//! Passive instruments over authoritative current and retained Field state.

use crate::fault::Fault;
use crate::field::{self, StepRecords, UPKEEP_PURPOSES};
use crate::json::{Json, Obj};
use crate::rank::Window;
use crate::read;
use crate::state::{FieldState, RunState, Fx, FRAC_ONE};

const INSTRUMENTS: [&str; 8] = [
    "stored_charge",
    "route_flow",
    "view_boundary_flow",
    "supply_uptake",
    "physical_leakage",
    "maintenance_allocation",
    "initial_stock_estimate",
    "response_lag",
];

struct HistoryStep {
    charge: Vec<(u32, Fx)>,
    flow: Vec<(u32, Fx)>,
    leakage: Vec<(u32, Fx)>,
    supply: Vec<(u32, Fx)>,
    upkeep: Vec<(u32, [Fx; UPKEEP_PURPOSES])>,
    input: Vec<(u32, Fx)>,
}

fn value(entries: &[(u32, Fx)], id: u32) -> Fx {
    entries
        .binary_search_by_key(&id, |(key, _)| *key)
        .ok()
        .map_or(0, |place| entries[place].1)
}

fn replayed(state: &RunState, requested: u16) -> (FieldState, Vec<HistoryStep>) {
    let window = Window::of(state, requested);
    let mut field = window.start.clone();
    let opening = field.clone();
    let mut pressures = window.pressures.clone();
    let mut history = Vec::with_capacity(window.len());
    for recorded in &window.steps {
        let input = field
            .currents
            .iter()
            .map(|current| {
                let gain = field
                    .layers
                    .iter()
                    .find(|layer| layer.layer == current.layer)
                    .map_or(FRAC_ONE, |layer| layer.gain);
                let ceiling = crate::fx::fixed_mul(
                    crate::fx::fixed_mul(current.strength, gain),
                    field::current_emission_scale(current),
                );
                (u32::from(current.id), ceiling)
            })
            .collect();
        let mut stream = recorded.rng;
        let outcome = field::advance_cached(
            &mut field,
            recorded.ctl,
            window.pointer_speed,
            &mut field::Staging {
                pressures: &mut pressures,
                schedule: &window.schedule,
                stream: &mut stream,
                medium: window.medium,
                supply_jitter: window.supply_jitter,
            },
            &window.cache,
        );
        history.push(HistoryStep {
            charge: outcome.records.q,
            flow: outcome.records.f,
            leakage: outcome
                .ledger
                .nodes
                .iter()
                .filter(|entry| entry.leakage != 0)
                .map(|entry| (entry.node, entry.leakage))
                .collect(),
            supply: outcome
                .ledger
                .nodes
                .iter()
                .filter(|entry| entry.supply != 0)
                .map(|entry| (entry.node, entry.supply))
                .collect(),
            upkeep: outcome
                .records
                .upkeep
                .iter()
                .map(|entry| (entry.node, entry.mix))
                .collect(),
            input,
        });
    }
    (opening, history)
}

/// Lag maximizing positive centered cross-covariance between one addressed
/// periodic input and the selected stored-Charge response. Ties choose the
/// shortest lag. The returned agreement is a bounded descriptive correlation,
/// not a significance claim.
fn response_lag(input: &[Fx], response: &[Fx], period: usize) -> (Fx, Fx) {
    let count = input.len().min(response.len());
    if count < 4 || period < 2 {
        return (0, 0);
    }
    let input_mean = mean(&input[..count]);
    let response_mean = mean(&response[..count]);
    let max_lag = period.saturating_sub(1).min(count / 2);
    let mut best_lag = 0usize;
    let mut best_average = i128::MIN;
    let mut best_covariance = 0i128;
    let mut best_bound = 0i128;
    for lag in 0..=max_lag {
        let pairs = count - lag;
        let mut covariance = 0i128;
        let mut bound = 0i128;
        for at in 0..pairs {
            let x = i128::from(input[at] - input_mean);
            let y = i128::from(response[at + lag] - response_mean);
            covariance += x * y;
            bound += x.abs() * y.abs();
        }
        let average = covariance / pairs as i128;
        if average > best_average {
            best_average = average;
            best_lag = lag;
            best_covariance = covariance;
            best_bound = bound;
        }
    }
    let agreement = if best_covariance <= 0 || best_bound == 0 {
        0
    } else {
        (best_covariance * i128::from(FRAC_ONE) / best_bound)
            .clamp(0, i128::from(FRAC_ONE)) as Fx
    };
    (best_lag as Fx * FRAC_ONE, agreement)
}

fn averaged_by_id(ids: &[u32], history: &[HistoryStep], select: fn(&HistoryStep) -> &[(u32, Fx)]) -> Vec<Fx> {
    if history.is_empty() {
        return vec![0; ids.len()];
    }
    ids.iter()
        .map(|id| history.iter().map(|step| value(select(step), *id)).sum::<Fx>() / history.len() as Fx)
        .collect()
}

fn upkeep_purpose_means(ids: &[u32], history: &[HistoryStep]) -> Vec<Fx> {
    let mut purposes = [0; UPKEEP_PURPOSES];
    if history.is_empty() {
        return purposes.to_vec();
    }
    for step in history {
        for (node, mix) in &step.upkeep {
            if ids.binary_search(node).is_ok() {
                for (total, value) in purposes.iter_mut().zip(mix) {
                    *total += *value;
                }
            }
        }
    }
    for value in &mut purposes {
        *value /= history.len() as Fx;
    }
    purposes.to_vec()
}

fn grouped(values: &[Fx], grain: usize) -> Vec<Fx> {
    values.chunks(grain).map(|chunk| chunk.iter().copied().sum()).collect()
}

fn grouped_mean(values: &[Fx], grain: usize) -> Vec<Fx> {
    values.chunks(grain).map(mean).collect()
}

fn agreement(values: &[Fx], grain: usize) -> Fx {
    let denominator: i128 = values.iter().map(|value| i128::from(*value).abs()).sum();
    if denominator == 0 {
        return FRAC_ONE;
    }
    let mut error = 0i128;
    for group in values.chunks(grain) {
        let representative = mean(group);
        error += group
            .iter()
            .map(|value| (i128::from(*value) - i128::from(representative)).abs())
            .sum::<i128>();
    }
    (FRAC_ONE - (error * i128::from(FRAC_ONE) / denominator) as Fx).clamp(0, FRAC_ONE)
}

fn mean(values: &[Fx]) -> Fx {
    if values.is_empty() { 0 } else { values.iter().copied().sum::<Fx>() / values.len() as Fx }
}

fn route_ids(field: &FieldState, inside: &[u32], crossing: bool) -> Vec<u32> {
    field
        .routes
        .iter()
        .filter(|route| {
            let tail = inside.binary_search(&route.tail).is_ok();
            let head = inside.binary_search(&route.head).is_ok();
            if crossing { tail != head } else { tail && head }
        })
        .map(|route| route.route)
        .collect()
}

fn current_records(field: &FieldState) -> StepRecords {
    StepRecords {
        q: field.ports.iter().filter(|port| port.q != 0).map(|port| (port.node, port.q)).collect(),
        f: field.routes.iter().filter(|route| route.flow != 0).map(|route| (route.route, route.flow)).collect(),
        ..StepRecords::default()
    }
}

pub fn sample(state: &RunState, body: &Json) -> Result<String, Fault> {
    read::exact_keys(
        body,
        "body",
        &["inside", "instrument", "resolution", "surround", "window"],
    )?;
    let instrument = INSTRUMENTS[read::one_of(body, "instrument", &INSTRUMENTS)?];
    let inside = read::ids(body, "inside", field::NODES_PER_RUN, i64::from(u32::MAX))?;
    let grain = read::int(body, "resolution", 1, 32)? as usize;
    if !grain.is_power_of_two() {
        return Err(Fault::field("resolution"));
    }
    let requested = read::int(body, "window", 1, 180)? as u16;
    read::one_of(body, "surround", &["adjacent", "double", "whole"])?;

    let (opening, mut history) = replayed(state, requested);
    if history.is_empty() {
        let records = current_records(&state.now);
        history.push(HistoryStep {
            charge: records.q,
            flow: records.f,
            leakage: field::passive_leakage(&state.now),
            supply: Vec::new(),
            upkeep: state
                .now
                .ports
                .iter()
                .filter_map(|port| {
                    let due = port.q.min(port.upkeep_rate);
                    (due != 0).then_some((port.node, field::upkeep_allocation(port.kind, due)))
                })
                .collect(),
            input: state
                .now
                .currents
                .iter()
                .map(|current| {
                    (
                        u32::from(current.id),
                        crate::fx::fixed_mul(
                            current.strength,
                            field::current_emission_scale(current),
                        ),
                    )
                })
                .collect(),
        });
    }

    let (primary, samples, provenance, retained) = match instrument {
        "route_flow" => {
            let ids = route_ids(&state.now, &inside, false);
            let values = averaged_by_id(&ids, &history, |step| &step.flow);
            (values.iter().copied().sum(), grouped(&values, grain), "recorded_route_ledger", agreement(&values, grain))
        }
        "view_boundary_flow" => {
            let crossing = route_ids(&state.now, &inside, true);
            let trace: Vec<Fx> = history
                .iter()
                .map(|step| {
                    state
                        .now
                        .routes
                        .iter()
                        .filter(|route| crossing.binary_search(&route.route).is_ok())
                        .map(|route| {
                            let flow = value(&step.flow, route.route);
                            if inside.binary_search(&route.head).is_ok() { flow } else { -flow }
                        })
                        .sum()
                })
                .collect();
            (mean(&trace), grouped_mean(&trace, grain), "recorded_route_ledger", agreement(&trace, grain))
        }
        "supply_uptake" => {
            let values = averaged_by_id(&inside, &history, |step| &step.supply);
            (values.iter().copied().sum(), grouped(&values, grain), "replayed_supply_ledger", agreement(&values, grain))
        }
        "physical_leakage" => {
            let values = averaged_by_id(&inside, &history, |step| &step.leakage);
            (values.iter().copied().sum(), grouped(&values, grain), "replayed_leakage_ledger", agreement(&values, grain))
        }
        "maintenance_allocation" => {
            let values = upkeep_purpose_means(&inside, &history);
            (
                values.iter().copied().sum(),
                values,
                "recorded_typed_upkeep_ledger",
                FRAC_ONE,
            )
        }
        "initial_stock_estimate" => {
            let closing = history.last().map(|step| &step.charge[..]).unwrap_or(&[]);
            let values: Vec<Fx> = inside
                .iter()
                .map(|node| {
                    let initial = opening
                        .ports
                        .iter()
                        .find(|port| port.node == *node)
                        .map_or(0, |port| port.q);
                    let final_charge = value(closing, *node);
                    if final_charge == 0 { 0 } else { initial.min(final_charge) * FRAC_ONE / final_charge }
                })
                .collect();
            (mean(&values), grouped_mean(&values, grain), "opening_stock_upper_bound_estimate", agreement(&values, grain))
        }
        "response_lag" => {
            let periodic = opening
                .currents
                .iter()
                .filter(|current| current.duty < FRAC_ONE)
                .min_by_key(|current| current.id);
            let response: Vec<Fx> = history
                .iter()
                .map(|step| inside.iter().map(|node| value(&step.charge, *node)).sum())
                .collect();
            let samples = grouped_mean(&response, grain);
            match periodic {
                Some(current) => {
                    let input: Vec<Fx> = history
                        .iter()
                        .map(|step| value(&step.input, u32::from(current.id)))
                        .collect();
                    if history.len() < usize::from(current.period) * 2 {
                        (0, samples, "insufficient_periodic_cycles", 0)
                    } else {
                        let (lag, correlation) = response_lag(&input, &response, usize::from(current.period));
                        (lag, samples, "periodic_supply_charge_cross_covariance", correlation)
                    }
                }
                None => (0, samples, "periodic_input_unavailable", 0),
            }
        }
        _ => {
            let values = averaged_by_id(&inside, &history, |step| &step.charge);
            (values.iter().copied().sum(), grouped(&values, grain), "recorded_charge_state", agreement(&values, grain))
        }
    };
    let secondary = samples.iter().copied().min().unwrap_or(0);

    let mut out = String::new();
    let mut object = Obj::new(&mut out);
    object.int("agreement", retained);
    object.int("effectiveWindow", history.len() as i64);
    object.text("instrument", instrument);
    object.int("primary", primary);
    object.text("provenance", provenance);
    {
        let mut list = object.list("samples");
        for value in samples {
            list.int(value);
        }
        list.end();
    }
    object.int("secondary", secondary);
    object.int("step", i64::from(state.now.step));
    object.int("targetCount", inside.len() as i64);
    object.end();
    Ok(out)
}
