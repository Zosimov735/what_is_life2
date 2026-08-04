//! Authoritative cold-path experiments over cloned run state.
//!
//! These jobs never mutate the loaded run. Each trial restores a clone of the
//! same canonical `RunState`, re-roots its trajectory stream with an addressed
//! seed, and advances through the ordinary Rust transition. Paired divergence
//! gives both arms the same stream; Holdout removes intervention and control;
//! inheritance copies the immutable specification and partitions only embodied
//! state before hands-off recovery.

use crate::fault::Fault;
use crate::field::{self, NodeKind};
use crate::json::{Json, Obj};
use crate::plan::PlanCommand;
use crate::read;
use crate::run::Run;
use crate::state::{ControlContract, ControlState, FieldState, RunState, Trace, FRAC_ONE};

const ANALYSIS_KINDS: [&str; 4] = ["divergence", "ensemble", "holdout", "inheritance"];
const ENSEMBLE_TRIALS: u32 = 12;
const HOLDOUT_TRIALS: u32 = 8;
const DIVERGENCE_STEPS: u32 = 48;
const INHERITANCE_STEPS: u32 = 90;

#[derive(Clone)]
struct Intervention {
    tool: String,
    target: u32,
    receiver: u32,
    transfer_mask: u8,
    destination: String,
    onset: u32,
    duration: u16,
    amount: i64,
}

#[derive(Clone)]
struct Scenario {
    id: String,
    window: u32,
    route_ids: Vec<u32>,
    intervention: Option<Intervention>,
    holdout_seed: u32,
    control: ControlContract,
}

struct TrialResult {
    seed: u32,
    value: i64,
    passed: bool,
    failure: &'static str,
    trace: Vec<i64>,
    criterion_status: &'static str,
    criterion: Option<crate::criterion::CriterionReading>,
}

pub fn run(state: &RunState, body: &Json) -> Result<String, Fault> {
    read::exact_keys(body, "body", &["kind", "scenario"])?;
    let kind = ANALYSIS_KINDS[read::one_of(body, "kind", &ANALYSIS_KINDS)?];
    let scenario = read_scenario(read::map(body, "scenario")?)?;
    match kind {
        "divergence" => divergence(state, &scenario),
        "ensemble" => ensemble(state, &scenario, ENSEMBLE_TRIALS, true),
        "holdout" => ensemble(state, &scenario, HOLDOUT_TRIALS, false),
        _ => inheritance(state, &scenario),
    }
}

fn read_scenario(value: &Json) -> Result<Scenario, Fault> {
    let observation = read::map(value, "observation")?;
    let intervention = match read::map_or_null(value, "intervention")? {
        None => None,
        Some(held) => Some(Intervention {
            tool: read::text(held, "tool")?.to_string(),
            target: read::int(held, "target", 0, i64::from(u32::MAX))? as u32,
            receiver: read::int(held, "receiver", 0, i64::from(u32::MAX))? as u32,
            transfer_mask: read::int(held, "transferMask", 0, 127)? as u8,
            destination: read::text(held, "destination")?.to_string(),
            onset: read::int(held, "onset", 0, 1_800)? as u32,
            duration: read::int(held, "duration", 1, 1_800)? as u16,
            amount: read::int(held, "amount", 0, 100)?,
        }),
    };
    Ok(Scenario {
        id: read::text(value, "id")?.to_string(),
        window: read::int(observation, "window", 15, 1_800)? as u32,
        route_ids: read::ids(value, "routeIds", field::ROUTES_PER_RUN, i64::from(u32::MAX))?,
        intervention,
        holdout_seed: value
            .get("holdoutSeed")
            .and_then(Json::as_int)
            .filter(|seed| (0..=i64::from(u32::MAX)).contains(seed))
            .unwrap_or(0) as u32,
        control: ControlContract::named(read::text(value, "control")?)?,
    })
}

fn form_of(state: &RunState) -> String {
    state
        .now
        .forms
        .iter()
        .find(|form| form.controlled)
        .or_else(|| state.now.forms.first())
        .map_or(String::new(), |form| form.form.clone())
}

fn cloned_run(state: &RunState, scenario: &Scenario, seed: u32) -> Result<Run, Fault> {
    let mut trial = state.clone();
    trial.scenario = trial.scenario.with_control(scenario.control);
    trial.criterion = trial
        .scenario
        .criterion(trial.progress.chapter_index)
        .map(|_| crate::criterion::CriterionRuntime::opening(trial.now.step));
    let mut run = Run::restore(trial, &form_of(state))?;
    run.rebranch(seed)?;
    Ok(run)
}

fn recorded_controls(state: &RunState, scenario: &Scenario) -> Vec<ControlState> {
    if scenario.control != ControlContract::RecordedOpenLoop {
        return Vec::new();
    }
    state.trace.steps.iter().map(|step| step.ctl).collect()
}

fn control_at(controls: &[ControlState], elapsed: u32) -> ControlState {
    if controls.is_empty() {
        ControlState::default()
    } else {
        controls[elapsed as usize % controls.len()]
    }
}

fn provenance(state: &RunState, scenario: &Scenario) -> (String, String) {
    let frozen = state.scenario.with_control(scenario.control);
    (frozen.generator().specification_hash(), frozen.scenario_hash())
}

fn throughput(field: &FieldState) -> i64 {
    field
        .routes
        .iter()
        .map(|route| {
            if route.capacity <= 0 {
                0
            } else {
                (route.flow.saturating_mul(255) / route.capacity).clamp(0, 255)
            }
        })
        .sum()
}

fn stored(field: &FieldState) -> i64 {
    field.ports.iter().map(|port| port.q).sum::<i64>()
        + field.forms.iter().map(|form| form.reserve).sum::<i64>()
        + field.pending.iter().map(|entry| entry.magnitude).sum::<i64>()
}

fn trailing_mean(values: &[i64]) -> i64 {
    let width = values.len().min(8);
    if width == 0 {
        return 0;
    }
    values[values.len() - width..].iter().sum::<i64>() / width as i64
}

fn median(values: &[i64]) -> i64 {
    let mut ordered = values.to_vec();
    ordered.sort_unstable();
    let middle = ordered.len() / 2;
    if ordered.len() % 2 == 0 {
        (ordered[middle - 1] + ordered[middle]) / 2
    } else {
        ordered[middle]
    }
}

fn plan_for(intervention: &Intervention, scenario: &Scenario) -> Result<PlanCommand, Fault> {
    let plan = match intervention.tool.as_str() {
        "blade" => PlanCommand::Cut { route: intervention.target },
        "clamp" => PlanCommand::LimitRoute {
            route: intervention.target,
            retained_fraction: ((100 - intervention.amount).max(1) * FRAC_ONE / 100).clamp(1, FRAC_ONE - 1),
            duration: intervention.duration,
        },
        "scramble" => PlanCommand::ScrambleRoutes {
            routes: scenario.route_ids.clone(),
            probability: (intervention.amount.max(1) * FRAC_ONE / 100).clamp(1, FRAC_ONE - 1),
            duration: intervention.duration,
        },
        "decoy" => PlanCommand::DivertSupply {
            current: u16::try_from(intervention.target).map_err(|_| Fault::field("target"))?,
            receiver: intervention.receiver,
            capture_fraction: (intervention.amount.max(1) * FRAC_ONE / 100).clamp(1, FRAC_ONE - 1),
            duration: intervention.duration,
        },
        "delay" => PlanCommand::DelaySupply {
            current: u16::try_from(intervention.target).map_err(|_| Fault::field("target"))?,
            duration: intervention.duration,
        },
        "replace" => PlanCommand::ReplaceComponent {
            node: intervention.target,
            transfer_mask: intervention.transfer_mask.max(1),
        },
        "breach" => PlanCommand::RaiseLeak {
            delta: (intervention.amount.max(1) * 8_192 / 100).max(1),
            duration: intervention.duration,
        },
        "transplant" => PlanCommand::Transplant { regime: intervention.destination.clone() },
        _ => return Err(Fault::field("tool")),
    };
    Ok(plan)
}

fn apply_intervention(run: &mut Run, scenario: &Scenario) -> Result<(), Fault> {
    let Some(intervention) = &scenario.intervention else {
        return Ok(());
    };
    run.analysis_apply(plan_for(intervention, scenario)?)
}

fn trial(
    state: &RunState,
    scenario: &Scenario,
    seed: u32,
    admit_intervention: bool,
) -> Result<TrialResult, Fault> {
    let controls = recorded_controls(state, scenario);
    let mut run = cloned_run(state, scenario, seed)?;
    let opening_stored = stored(&run.state().now);
    let opening_throughput = throughput(&run.state().now);
    let threshold = (opening_throughput * 72 / 100).max(4);
    let steps = scenario.window.clamp(24, 180);
    let mut trace = Vec::with_capacity(steps as usize);
    let mut leakage = 0i64;
    let mut supplied = 0i64;
    for elapsed in 0..steps {
        let intervened = admit_intervention
            && scenario.intervention.as_ref().is_some_and(|held| held.onset == elapsed);
        if intervened {
            apply_intervention(&mut run, scenario)?;
        }
        let ledger = run.analysis_step_with(control_at(&controls, elapsed), intervened);
        leakage = leakage.saturating_add(ledger.leakage);
        supplied = supplied.saturating_add(ledger.current);
        trace.push(throughput(&run.state().now));
    }
    let value = trailing_mean(&trace);
    let final_stored = stored(&run.state().now);
    let criterion = run
        .state()
        .scenario
        .criterion(run.state().progress.chapter_index)
        .zip(run.state().criterion.as_ref())
        .map(|(spec, runtime)| {
            runtime.current_reading(
                spec,
                &run.state().now,
                scenario.control == ControlContract::HandsOff,
            )
        });
    let threshold_passed = value >= threshold && final_stored >= opening_stored / 2;
    let criterion_status = criterion.as_ref().map_or("unavailable", |reading| reading.status.name());
    let passed = if admit_intervention {
        threshold_passed
    } else {
        run.state().criterion.as_ref().map_or(threshold_passed, |runtime| {
            runtime.status() == crate::criterion::CriterionStatus::Passed
        })
    };
    let failure = if passed {
        "none"
    } else if criterion.as_ref().is_some_and(|reading| !reading.leakage.met) {
        "leakage"
    } else if criterion
        .as_ref()
        .is_some_and(|reading| reading.components.iter().any(|component| !component.met))
    {
        "reserve"
    } else if criterion
        .as_ref()
        .is_some_and(|reading| reading.routes.iter().any(|route| !route.met))
    {
        "throughput"
    } else if leakage > supplied / 3 {
        "leakage"
    } else if final_stored < opening_stored * 3 / 5 {
        "reserve"
    } else {
        "throughput"
    };
    Ok(TrialResult { seed, value, passed, failure, trace, criterion_status, criterion })
}

fn seed_for(scenario: &Scenario, ordinal: u32, holdout: bool) -> u32 {
    let mut hash = if holdout { 0x9e37_79b9 } else { 0x811c_9dc5 };
    for byte in scenario.id.bytes() {
        hash ^= u32::from(byte);
        hash = hash.wrapping_mul(16_777_619);
    }
    hash ^= if holdout { scenario.holdout_seed.rotate_left(13) } else { 0 };
    hash.wrapping_add(ordinal.wrapping_mul(65_537))
}

fn ensemble(
    state: &RunState,
    scenario: &Scenario,
    count: u32,
    admit_intervention: bool,
) -> Result<String, Fault> {
    let holdout = !admit_intervention;
    let mut trials = Vec::with_capacity(count as usize);
    for ordinal in 0..count {
        trials.push(trial(state, scenario, seed_for(scenario, ordinal, holdout), admit_intervention)?);
    }
    let values: Vec<i64> = trials.iter().map(|trial| trial.value).collect();
    let (generator_hash, scenario_hash) = provenance(state, scenario);
    let mut out = String::new();
    let mut object = Obj::new(&mut out);
    object.text("controlContract", scenario.control.name());
    object.text("generatorHash", &generator_hash);
    object.int("high", *values.iter().max().unwrap_or(&0));
    object.int("low", *values.iter().min().unwrap_or(&0));
    object.int("median", median(&values));
    object.int("passCount", trials.iter().filter(|trial| trial.passed).count() as i64);
    object.text("scenarioHash", &scenario_hash);
    {
        let mut list = object.list("trials");
        for trial in &trials {
            let mut written = String::new();
            let mut held = Obj::new(&mut written);
            match &trial.criterion {
                Some(criterion) => held.raw("criterion", &criterion.written()),
                None => held.null("criterion"),
            };
            held.text("criterionStatus", trial.criterion_status);
            held.text("failure", trial.failure);
            held.bool("passed", trial.passed);
            held.int("seed", i64::from(trial.seed));
            {
                let mut trace = held.list("trace");
                for value in &trial.trace {
                    trace.int(*value);
                }
                trace.end();
            }
            held.int("value", trial.value);
            held.end();
            list.raw(&written);
        }
        list.end();
    }
    object.end();
    Ok(out)
}

fn divergence(state: &RunState, scenario: &Scenario) -> Result<String, Fault> {
    let seed = seed_for(scenario, 0, false);
    let controls = recorded_controls(state, scenario);
    let mut baseline = cloned_run(state, scenario, seed)?;
    let mut changed = cloned_run(state, scenario, seed)?;
    let opening_stored = stored(&baseline.state().now);
    let threshold = (throughput(&baseline.state().now) * 72 / 100).max(4);
    let onset = scenario.intervention.as_ref().map_or(0, |held| held.onset);
    let mut points = Vec::new();
    let mut first = None;
    let mut outlet = None;
    let mut reserve = None;
    let mut criterion = None;
    let mut below = 0u32;
    for elapsed in 0..DIVERGENCE_STEPS {
        let intervened = scenario.intervention.is_some() && elapsed == onset;
        if intervened {
            apply_intervention(&mut changed, scenario)?;
        }
        let control = control_at(&controls, elapsed);
        baseline.analysis_step_with(control, false);
        changed.analysis_step_with(control, intervened);
        let ordinary = throughput(&baseline.state().now);
        let altered = throughput(&changed.state().now);
        let step = changed.state().now.step;
        if ordinary != altered && first.is_none() {
            first = Some(step);
        }
        if altered < threshold {
            below += 1;
            outlet.get_or_insert(step);
            if below >= 5 {
                criterion.get_or_insert(step);
            }
        } else {
            below = 0;
        }
        if stored(&changed.state().now) < opening_stored * 3 / 5 {
            reserve.get_or_insert(step);
        }
        points.push((step, ordinary, altered));
    }
    let fallback = state.now.step.saturating_add(DIVERGENCE_STEPS);
    let (generator_hash, scenario_hash) = provenance(state, scenario);
    let mut out = String::new();
    let mut object = Obj::new(&mut out);
    object.text("controlContract", scenario.control.name());
    object.int("criterionStep", i64::from(criterion.unwrap_or(fallback)));
    object.int("firstStep", i64::from(first.unwrap_or(fallback)));
    object.text("generatorHash", &generator_hash);
    object.int("outletFloorStep", i64::from(outlet.unwrap_or(fallback)));
    {
        let mut list = object.list("points");
        for (step, ordinary, altered) in points {
            let mut written = String::new();
            let mut point = Obj::new(&mut written);
            point.int("baseline", ordinary);
            point.int("changed", altered);
            point.int("step", i64::from(step));
            point.end();
            list.raw(&written);
        }
        list.end();
    }
    object.int("reserveFloorStep", i64::from(reserve.unwrap_or(fallback)));
    object.text("scenarioHash", &scenario_hash);
    object.end();
    Ok(out)
}

fn partition_field(source: &FieldState, parity: usize) -> FieldState {
    let mut field = source.clone();
    let retained: Vec<u32> = source
        .ports
        .iter()
        .filter(|port| port.kind == NodeKind::Form || port.node as usize % 2 == parity)
        .map(|port| port.node)
        .collect();
    field.ports.retain(|port| retained.binary_search(&port.node).is_ok());
    for port in &mut field.ports {
        if port.kind == NodeKind::Form {
            port.q = if parity == 0 { (port.q + 1) / 2 } else { port.q / 2 };
        }
    }
    field.routes.retain(|route| {
        retained.binary_search(&route.tail).is_ok() && retained.binary_search(&route.head).is_ok()
    });
    let route_ids: Vec<u32> = field.routes.iter().map(|route| route.route).collect();
    field.route_clamps.retain(|clamp| route_ids.binary_search(&clamp.route).is_ok());
    field.route_controls.retain(|control| route_ids.binary_search(&control.route).is_ok());
    field.policy_runtime.retain(|runtime| retained.binary_search(&runtime.address).is_ok());
    crate::field::synchronize_automation_state(&mut field);
    field.route_scramble = None;
    field.physical_compartment.members.retain(|node| retained.binary_search(node).is_ok());
    if field.physical_compartment.members.is_empty() {
        if let Some(form) = field.forms.iter().find(|form| form.controlled).or_else(|| field.forms.first()) {
            field.physical_compartment.members.push(form.node);
        }
    }
    field.materials.retain(|material| material.material as usize % 2 == parity);
    field.signals.retain(|signal| {
        retained.binary_search(&signal.source).is_ok() && retained.binary_search(&signal.target).is_ok()
    });
    field.supply_decoys.retain(|decoy| retained.binary_search(&decoy.receiver).is_ok());
    for layer in &mut field.layers {
        layer.port_ids = field
            .ports
            .iter()
            .filter(|port| port.layer == layer.layer && port.kind != NodeKind::Form)
            .map(|port| port.node)
            .collect();
    }
    for form in &mut field.forms {
        if let Some(port) = field.ports.iter().find(|port| port.node == form.node) {
            form.charge = port.q;
            form.reserve = if parity == 0 { (form.reserve + 1) / 2 } else { form.reserve / 2 };
        }
    }
    field
}

fn inheritance_child(
    state: &RunState,
    scenario: &Scenario,
    parity: usize,
) -> Result<(i64, i64, i64, Option<u32>, i64, bool, Vec<u32>, Vec<u32>), Fault> {
    let field = partition_field(&state.now, parity);
    let components = field.ports.len() as i64;
    let routes = field.routes.len() as i64;
    let component_ids = field.ports.iter().map(|port| port.node).collect();
    let route_ids = field.routes.iter().map(|route| route.route).collect();
    let initial_charge = stored(&field);
    let threshold = (throughput(&state.now) * 36 / 100).max(2);
    let mut child_state = state.clone();
    child_state.now = field.clone();
    child_state.trace = Trace {
        start_step: field.step,
        keyframe: field,
        steps: std::collections::VecDeque::new(),
    };
    child_state.view.inside.retain(|node| child_state.now.ports.iter().any(|port| port.node == *node));
    let controls = recorded_controls(state, scenario);
    let mut run = cloned_run(
        &child_state,
        scenario,
        seed_for(scenario, parity as u32, false),
    )?;
    let mut held = 0u32;
    let mut recovered = None;
    let mut final_value = 0;
    for elapsed in 1..=INHERITANCE_STEPS {
        run.analysis_step_with(control_at(&controls, elapsed - 1), false);
        final_value = throughput(&run.state().now);
        if final_value >= threshold {
            held += 1;
            if held >= 10 && recovered.is_none() {
                recovered = Some(elapsed);
            }
        } else {
            held = 0;
        }
    }
    let margin = final_value - threshold;
    let passed = recovered.is_some() && margin >= 0;
    Ok((components, routes, initial_charge, recovered, margin, passed, component_ids, route_ids))
}

fn inheritance(state: &RunState, scenario: &Scenario) -> Result<String, Fault> {
    let first = inheritance_child(state, scenario, 0)?;
    let second = inheritance_child(state, scenario, 1)?;
    let (generator_hash, scenario_hash) = provenance(state, scenario);
    let mut out = String::new();
    let mut object = Obj::new(&mut out);
    {
        let mut children = object.list("children");
        for (id, held) in [("A", first), ("B", second)] {
            let mut written = String::new();
            let mut child = Obj::new(&mut written);
            {
                let mut components = child.list("componentIds");
                for node in &held.6 {
                    components.int(i64::from(*node));
                }
                components.end();
            }
            child.int("criterionMargin", held.4);
            child.text("id", id);
            child.int("inheritedComponents", held.0);
            child.int("inheritedRoutes", held.1);
            child.int("initialCharge", held.2);
            child.bool("passed", held.5);
            match held.3 {
                Some(step) => child.int("recoveredAt", i64::from(step)),
                None => child.null("recoveredAt"),
            };
            {
                let mut routes = child.list("routeIds");
                for route in &held.7 {
                    routes.int(i64::from(*route));
                }
                routes.end();
            }
            child.end();
            children.raw(&written);
        }
        children.end();
    }
    object.text("controlContract", scenario.control.name());
    object.text("copiedSpecification", &state.scenario.generator().specification_hash());
    object.text("generatorHash", &generator_hash);
    object.text("partition", "alternating_components");
    object.text("scenarioHash", &scenario_hash);
    object.int("sourceComponents", state.now.ports.len() as i64);
    object.int("sourceRoutes", state.now.routes.len() as i64);
    object.end();
    Ok(out)
}
