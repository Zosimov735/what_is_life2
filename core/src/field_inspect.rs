//! On-demand local inspection over authoritative Field state and retained flow records.

use crate::fault::{Code, Fault};
use crate::json::{Json, Obj};
use crate::read;
use crate::state::{Fx, RunState};

fn recorded_flow(records: &crate::field::StepRecords, route: u32) -> Fx {
    records
        .f
        .binary_search_by_key(&route, |(id, _)| *id)
        .ok()
        .map_or(0, |place| records.f[place].1)
}

fn retained_steps(state: &RunState) -> Vec<&crate::state::TraceStep> {
    let count = usize::from(state.view.window).min(state.trace.steps.len());
    state
        .trace
        .steps
        .iter()
        .skip(state.trace.steps.len().saturating_sub(count))
        .collect()
}

fn route_mean(state: &RunState, route: u32) -> Fx {
    let steps = retained_steps(state);
    if steps.is_empty() {
        return state
            .now
            .routes
            .iter()
            .find(|held| held.route == route)
            .map_or(0, |held| held.flow);
    }
    steps
        .iter()
        .map(|step| recorded_flow(&step.records, route))
        .sum::<Fx>()
        / steps.len() as Fx
}

fn node_mean(state: &RunState, node: u32, incoming: bool) -> Fx {
    let route_ids: Vec<u32> = state
        .now
        .routes
        .iter()
        .filter(|route| if incoming { route.head == node } else { route.tail == node })
        .map(|route| route.route)
        .collect();
    let steps = retained_steps(state);
    if steps.is_empty() {
        return state
            .now
            .routes
            .iter()
            .filter(|route| route_ids.binary_search(&route.route).is_ok())
            .map(|route| route.flow)
            .sum();
    }
    steps
        .iter()
        .map(|step| route_ids.iter().map(|route| recorded_flow(&step.records, *route)).sum::<Fx>())
        .sum::<Fx>()
        / steps.len() as Fx
}

fn missing(kind: &str, id: u32) -> Fault {
    let mut detail = String::new();
    let mut object = Obj::new(&mut detail);
    object.int("id", i64::from(id));
    object.text("target", kind);
    object.end();
    Fault::detailed(Code::NotFound, detail)
}

struct AutomationReading {
    action: Option<crate::policy::LocalAction>,
    automated: bool,
    cooldown: u16,
    outcome: crate::policy::PolicyOutcome,
    rule: i16,
    target: Option<i64>,
    target_kind: &'static str,
    timer: u32,
}

fn automation_reading(state: &RunState, address: u32) -> AutomationReading {
    let automated = state
        .scenario
        .generator()
        .local_policy()
        .components()
        .iter()
        .any(|component| component.address == address);
    let runtime = automated
        .then(|| {
            state
                .now
                .policy_runtime
                .iter()
                .find(|runtime| runtime.address == address)
        })
        .flatten();
    let (target_kind, target) = runtime
        .map(|runtime| runtime.target.parts())
        .unwrap_or(("none", None));
    AutomationReading {
        action: runtime.and_then(|runtime| runtime.active_action.clone()),
        automated,
        cooldown: runtime.map_or(0, |runtime| runtime.cooldown),
        outcome: runtime.map_or(crate::policy::PolicyOutcome::Idle, |runtime| runtime.outcome),
        rule: runtime.map_or(-1, |runtime| runtime.active_rule),
        target,
        target_kind,
        timer: runtime.map_or(0, |runtime| runtime.timer),
    }
}

fn policy_capabilities(state: &RunState, address: u32, mobile: bool) -> String {
    let attached: Vec<_> = state
        .now
        .routes
        .iter()
        .filter(|route| route.tail == address || route.head == address)
        .collect();
    let has_attached = !attached.is_empty();
    let has_outgoing = attached.iter().any(|route| route.tail == address);
    let mut out = String::new();
    let mut object = Obj::new(&mut out);
    {
        let mut actions = object.list("actions");
        actions.text("hold");
        if mobile {
            actions.text("seek_supply");
            actions.text("seek_port");
            actions.text("seek_signal");
            actions.text("change_depth");
            actions.text("couple");
        }
        actions.text("set_interface");
        if has_outgoing {
            actions.text("set_route");
        }
        actions.text("emit_signal");
        if mobile {
            actions.text("use_ability");
        }
        actions.end();
    }
    {
        let mut routes = object.list("attached_routes");
        for route in attached {
            let mut held = routes.object();
            held.int("capacity", route.capacity);
            held.bool("outgoing", route.tail == address);
            held.int("route", i64::from(route.route));
            held.end();
        }
        routes.end();
    }
    {
        let mut conditions = object.list("conditions");
        for condition in [
            "always",
            "charge_below",
            "charge_above",
            "operating_margin_below",
            "supply",
            "target_in_range",
        ] {
            conditions.text(condition);
        }
        if has_attached {
            conditions.text("route_flow_below");
            conditions.text("route_flow_above");
        }
        conditions.text("overloaded");
        conditions.text("signal_present");
        conditions.text("timer_elapsed");
        conditions.end();
    }
    object.int(
        "coupling_radius_max",
        crate::field::pulse_radius(crate::state::FRAC_ONE),
    );
    object.bool("mobile", mobile);
    object.int("route_weight_max", i64::from(u16::MAX));
    object.int("sensor_radius_max", crate::policy::POLICY_SENSOR_RADIUS_MAX);
    object.int("signal_strength_max", crate::fx::STORED_BOUND - 1);
    object.end();
    out
}

pub fn inspect(state: &RunState, body: &Json) -> Result<String, Fault> {
    read::exact_keys(body, "body", &["id", "target"])?;
    let id = read::int(body, "id", 0, i64::from(u32::MAX))? as u32;
    let target = read::text(body, "target")?;
    let window = retained_steps(state).len() as i64;
    let mut out = String::new();
    let mut object = Obj::new(&mut out);
    match target {
        "form" => {
            let form = state
                .now
                .forms
                .iter()
                .find(|form| u32::from(form.id) == id)
                .ok_or_else(|| missing(target, id))?;
            let node = state
                .now
                .ports
                .iter()
                .find(|node| node.node == form.node)
                .ok_or_else(|| missing("node", form.node))?;
            let (ability, available, value, limit, count, due) = match form.form.as_str() {
                "thread" => ("responsive_steering", form.controlled, form.steer_scale, 2 * 65_536, 0, 0),
                "ring" => ("local_retention", true, 0, 0, 0, 0),
                "relay" => ("extended_commissioning", true, form.route_capacity, form.route_reach, 0, 0),
                "vault" => (
                    "reserve_discharge",
                    form.reserve > 0,
                    form.reserve,
                    crate::field::VAULT_RESERVE_CAPACITY,
                    0,
                    0,
                ),
                "lens" => (
                    "local_sensor",
                    node.q >= crate::run::LENS_SAMPLE_COST,
                    crate::run::LENS_SAMPLE_COST,
                    crate::run::LENS_SENSOR_RADIUS,
                    0,
                    0,
                ),
                "knot" => {
                    let blanks = form.junction.map_or(0, |junction| junction.blanks);
                    let deploy_cost = form.junction.map_or(0, |junction| junction.deploy_cost);
                    let capacity = form.junction.map_or(0, |junction| junction.capacity);
                    (
                        "junction_deployment",
                        blanks > 0 && node.q >= deploy_cost,
                        deploy_cost,
                        capacity,
                        i64::from(blanks),
                        0,
                    )
                }
                "wake" => {
                    let pending: Vec<_> = state.now.pending.iter().filter(|cache| cache.form == form.id).collect();
                    let due = pending
                        .iter()
                        .map(|cache| cache.due.saturating_sub(state.now.step))
                        .min()
                        .map_or(0, i64::from);
                    (
                        "conserving_cache",
                        node.q > 0,
                        form.trail.map_or(0, |trail| trail.magnitude),
                        form.trail.map_or(0, |trail| trail.radius),
                        pending.len() as i64,
                        due,
                    )
                }
                "chorus" => (
                    "control_handoff",
                    state.now.forms.len() > 1,
                    0,
                    state
                        .now
                        .forms
                        .iter()
                        .find_map(|member| member.link.map(|link| link.separation))
                        .unwrap_or(0),
                    state.now.forms.len() as i64,
                    0,
                ),
                _ => ("unavailable", false, 0, 0, 0, 0),
            };
            let medium = state.scenario.regime().medium_motion();
            let automation = automation_reading(state, form.node);
            object.text("ability", ability);
            object.bool("ability_available", available);
            object.int("ability_count", count);
            object.int("ability_due", due);
            object.int("ability_limit", limit);
            object.int("ability_value", value);
            object.bool("automated", automation.automated);
            object.int("capacity", node.capacity);
            object.bool("controlled", form.controlled);
            object.int("id", i64::from(id));
            object.text("kind", &form.form);
            object.int("layer", i64::from(form.layer));
            object.int("medium_collision_radius", medium.collision_radius);
            object.int("medium_collision_response", medium.collision_response);
            object.int("medium_coupling", crate::field::form_medium_coupling(&form.form));
            object.int("medium_drag", medium.drag);
            object.int("medium_vx", medium.velocity.x);
            object.int("medium_vy", medium.velocity.y);
            object.int("node", i64::from(form.node));
            match &automation.action {
                Some(action) => {
                    object.raw("policy_action", &action.written());
                }
                None => {
                    object.null("policy_action");
                }
            }
            object.int("policy_cooldown", i64::from(automation.cooldown));
            object.text("policy_outcome", automation.outcome.name());
            object.int("policy_rule", i64::from(automation.rule));
            object.int_or_null("policy_target", automation.target);
            object.text("policy_target_kind", automation.target_kind);
            object.int("policy_timer", i64::from(automation.timer));
            object.raw("policy_capabilities", &policy_capabilities(state, form.node, true));
            object.int("q", node.q);
            object.text("target", "form");
            object.int("vx", form.vel.x);
            object.int("vy", form.vel.y);
        }
        "node" => {
            let node = state.now.ports.iter().find(|port| port.node == id).ok_or_else(|| missing(target, id))?;
            let leakage = crate::field::passive_leakage(&state.now)
                .into_iter()
                .find(|(held, _)| *held == id)
                .map_or(0, |(_, value)| value);
            let automation = automation_reading(state, id);
            {
                let mut interventions = object.list("active_interventions");
                for decoy in state.now.supply_decoys.iter().filter(|decoy| decoy.receiver == id) {
                    let mut reading = interventions.object();
                    reading.int("capture_fraction", decoy.capture_fraction);
                    reading.int("current", i64::from(decoy.current));
                    reading.text("kind", "decoy_receiver");
                    reading.int("remaining", i64::from(decoy.until_step.saturating_sub(state.now.step)));
                    reading.end();
                }
                interventions.end();
            }
            object.bool("automated", automation.automated);
            object.int("capacity", node.capacity);
            object.int("current_leakage", leakage);
            object.int("id", i64::from(id));
            object.int("inflow_mean", node_mean(state, id, true));
            object.text("kind", node.kind.name());
            object.int("layer", i64::from(node.layer));
            object.bool("open", node.open);
            object.int("outflow_mean", node_mean(state, id, false));
            match &automation.action {
                Some(action) => {
                    object.raw("policy_action", &action.written());
                }
                None => {
                    object.null("policy_action");
                }
            }
            object.int("policy_cooldown", i64::from(automation.cooldown));
            object.text("policy_outcome", automation.outcome.name());
            object.int("policy_rule", i64::from(automation.rule));
            object.int_or_null("policy_target", automation.target);
            object.text("policy_target_kind", automation.target_kind);
            object.int("policy_timer", i64::from(automation.timer));
            object.raw("policy_capabilities", &policy_capabilities(state, id, false));
            object.int("q", node.q);
            object.text("target", "node");
            {
                let mut mix = object.list("upkeep_mix");
                for value in crate::field::upkeep_allocation(node.kind, node.upkeep_rate) {
                    mix.int(value);
                }
                mix.end();
            }
            object.int("upkeep_rate", node.upkeep_rate);
            object.int("window", window);
        }
        "route" => {
            let route = state.now.routes.iter().find(|held| held.route == id).ok_or_else(|| missing(target, id))?;
            let control = state.now.route_controls.iter().find(|control| control.route == id);
            {
                let mut interventions = object.list("active_interventions");
                for clamp in state.now.route_clamps.iter().filter(|clamp| clamp.route == id) {
                    let mut reading = interventions.object();
                    reading.text("kind", "clamp");
                    reading.int("original_capacity", clamp.original_capacity);
                    reading.int("remaining", i64::from(clamp.until_step.saturating_sub(state.now.step)));
                    reading.end();
                }
                if let Some(scramble) = state
                    .now
                    .route_scramble
                    .as_ref()
                    .filter(|scramble| scramble.routes.binary_search(&id).is_ok())
                {
                    let mut reading = interventions.object();
                    reading.text("kind", "scramble");
                    reading.int("probability", scramble.probability);
                    reading.int("remaining", i64::from(scramble.until_step.saturating_sub(state.now.step)));
                    reading.end();
                }
                interventions.end();
            }
            object.int(
                "allocation_weight",
                control.map_or(1, |held| i64::from(held.allocation_weight)),
            );
            object.int("capacity", route.capacity);
            object.int("capacity_limit", control.map_or(route.capacity, |held| held.capacity_limit));
            object.int("controller", control.map_or(i64::from(route.tail), |held| i64::from(held.controller)));
            object.bool("enabled", control.is_none_or(|held| held.enabled));
            let transfer = state
                .now
                .route_runtime
                .iter()
                .find(|runtime| runtime.route == route.route);
            object.int("accepted_flow", transfer.map_or(route.flow, |held| held.accepted));
            object.int("flow", route.flow);
            object.int("formed_step", i64::from(route.formed_step));
            object.int("head", i64::from(route.head));
            object.int("id", i64::from(id));
            object.int("mean_flow", route_mean(state, id));
            object.text(
                "outcome",
                transfer.map_or("standing", |held| held.outcome.name()),
            );
            object.int("requested_flow", transfer.map_or(0, |held| held.requested));
            object.int("tail", i64::from(route.tail));
            object.text("target", "route");
            object.int("window", window);
        }
        "current" => {
            let place = state.now.currents.iter().position(|held| u32::from(held.id) == id).ok_or_else(|| missing(target, id))?;
            let current = &state.now.currents[place];
            let gain = state
                .now
                .layers
                .iter()
                .find(|layer| layer.layer == current.layer)
                .map_or(crate::state::FRAC_ONE, |layer| layer.gain);
            let cycle_mean = crate::fx::fixed_mul(current.strength, gain);
            let scheduled = crate::fx::fixed_mul(cycle_mean, crate::field::current_emission_scale(current));
            let jitter = state.scenario.regime().supply_jitter();
            object.bool("active", current.active);
            {
                let mut interventions = object.list("active_interventions");
                for decoy in state.now.supply_decoys.iter().filter(|decoy| u32::from(decoy.current) == id) {
                    let mut reading = interventions.object();
                    reading.int("capture_fraction", decoy.capture_fraction);
                    reading.text("kind", "decoy");
                    reading.int("receiver", i64::from(decoy.receiver));
                    reading.int("remaining", i64::from(decoy.until_step.saturating_sub(state.now.step)));
                    reading.end();
                }
                for delay in state.now.current_delays.iter().filter(|delay| u32::from(delay.current) == id) {
                    let mut reading = interventions.object();
                    reading.text("kind", "delay");
                    reading.int("remaining", i64::from(delay.until_step.saturating_sub(state.now.step)));
                    reading.end();
                }
                interventions.end();
            }
            object.int("ceiling_high", crate::fx::fixed_mul(scheduled, crate::state::FRAC_ONE + jitter));
            object.int("ceiling_low", crate::fx::fixed_mul(scheduled, crate::state::FRAC_ONE - jitter));
            object.int("cycle_mean", cycle_mean);
            object.int("duty", current.duty);
            object.bool("emitting", crate::field::current_emitting(current));
            object.int("id", i64::from(id));
            object.int("instantaneous_ceiling", scheduled);
            object.int("layer", i64::from(current.layer));
            object.int("on_steps", i64::from(crate::field::current_on_steps(current)));
            object.int("period", i64::from(current.period));
            object.int("phase", i64::from(current.phase));
            {
                let mut recipients = object.list("recipients");
                for node in crate::field::standing_in(&state.now, place)
                    .into_iter()
                    .map(|held| state.now.ports[held].node)
                {
                    recipients.int(i64::from(node));
                }
                recipients.end();
            }
            object.int("strength", current.strength);
            object.text("target", "current");
            object.int("variability", jitter);
            object.int("width", current.width);
        }
        "compartment" => {
            let leakage: Fx = crate::field::passive_leakage(&state.now)
                .into_iter()
                .map(|(_, value)| value)
                .sum();
            {
                let mut interventions = object.list("active_interventions");
                if let Some(breach) = state.now.leak_breach {
                    let mut reading = interventions.object();
                    reading.text("kind", "breach");
                    reading.int("original_coefficient", breach.original_coefficient);
                    reading.int("remaining", i64::from(breach.until_step.saturating_sub(state.now.step)));
                    reading.end();
                }
                interventions.end();
            }
            object.int("current_leakage", leakage);
            object.int("leak_fraction", state.now.physical_compartment.leak_per_exposed_contact_per_step);
            {
                let mut members = object.list("members");
                for node in &state.now.physical_compartment.members {
                    members.int(i64::from(*node));
                }
                members.end();
            }
            object.text("target", "compartment");
        }
        "view" => {
            {
                let mut members = object.list("members");
                for node in &state.view.inside {
                    members.int(i64::from(*node));
                }
                members.end();
            }
            object.int("resolution", i64::from(state.view.resolution));
            object.text("surround", state.view.surround.name());
            object.text("target", "view");
            object.int("window", i64::from(state.view.window));
        }
        "material" => {
            let material = state.now.materials.iter().find(|held| held.material == id).ok_or_else(|| missing(target, id))?;
            object.int("amount", i64::from(material.amount));
            object.bool("claimed", material.claimed);
            object.int("id", i64::from(id));
            object.text("kind", material.kind.name());
            object.int("layer", i64::from(material.layer));
            object.text("target", "material");
        }
        "cache" => {
            let cache = state
                .now
                .pending
                .get(id as usize)
                .ok_or_else(|| missing(target, id))?;
            let radius = state
                .now
                .forms
                .iter()
                .find(|form| form.id == cache.form)
                .and_then(|form| form.trail)
                .map_or(0, |trail| trail.radius);
            object.int("form", i64::from(cache.form));
            object.int("id", i64::from(id));
            object.int("layer", i64::from(cache.layer));
            object.int("q", cache.magnitude);
            object.int("radius", radius);
            object.int("release_in", i64::from(cache.due.saturating_sub(state.now.step)));
            object.text("target", "cache");
        }
        _ => return Err(Fault::field("target")),
    }
    object.end();
    Ok(out)
}
