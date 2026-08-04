//! The message layer of the core, in plain Rust.
//!
//! No binding type appears here, so `cargo test` exercises the same code the
//! browser runs. The version-13 command set carries thirty-two commands and
//! the error code set remains at twelve.
//!
//! What this answers is the whole command surface that stands before durable
//! persistence: the version handshake, `init_run` in both its modes,
//! `input_frame`, `export_run`, `import_run` with the locked import
//! validation, the two restores over the session's own record store, and the
//! three queued-change commands and the passive `set_focus` command, which are
//! valid only in `still` and answer the `state` envelope everywhere else.
//!
//! The three queued-change commands are whole: `queue_plan` reads one entry of
//! the locked union and hands it to the run, which validates it against the
//! projection every earlier entry has been applied to; `undo_plan` takes the
//! most recent entry back, and on an empty queue succeeds and changes nothing;
//! and `commit_plan` is a transaction — all of the queue or none of it — which
//! an empty queue passes through, spending nothing and leaving Still Mode by
//! the mode table's own committed exit.

use crate::content::{self, Chapter, Content};
use crate::fault::{ok_response, state_fault, Code, Fault};
use crate::json::{canonicalize, hex_bytes, parse, Json, Obj};
use crate::plan::PlanCommand;
use crate::read;
use crate::records::RecordStore;
use crate::run::{Mode, Run};
use crate::sha256;
use crate::state::{
    auto_slot, RecordKind, RegimeSpec, RunState, ScenarioSpec, EXPORT_FORMAT,
    SAVE_PAYLOAD_CAP, SAVE_VERSION,
};

/// The protocol version this build speaks.
pub const PROTOCOL_VERSION: u32 = 13;

/// The lifecycle state of a session that has not loaded a run.
const IDLE: &str = "idle";

/// The loaded lifecycle states, in mode-table order.
const LOADED: &[&str] = &[
    "running", "ramp_in", "still", "ramp_out", "suspended", "ended",
];

const SESSION_STATES: &[&str] = &[
    "idle", "running", "ramp_in", "still", "ramp_out", "suspended", "ended", "returned",
    "qualification_frozen",
];

const OPEN_STATES: &[&str] = &[
    "idle", "running", "ramp_in", "still", "ramp_out", "suspended", "ended", "returned",
];

const INSPECTION_STATES: &[&str] = &[
    "running", "ramp_in", "still", "ramp_out", "suspended", "ended",
    "qualification_frozen",
];

/// The thirty-two commands of version 13, each with the lifecycle states it is valid
/// in. The set is closed: an unknown command is a `protocol` fault.
const COMMANDS: [(&str, &[&str]); 32] = [
    ("list_contracts", SESSION_STATES),
    ("open_contract", OPEN_STATES),
    ("init_run", &["idle", "ended"]),
    ("input_frame", INSPECTION_STATES),
    ("queue_plan", &["still"]),
    ("undo_plan", &["still"]),
    ("commit_plan", &["still"]),
    ("set_focus", &["still"]),
    ("restore_checkpoint", LOADED),
    ("recover_branch", LOADED),
    ("export_run", INSPECTION_STATES),
    ("import_run", &["idle", "ended"]),
    ("reopen_archive", LOADED),
    ("run_analysis", LOADED),
    ("sample_instrument", INSPECTION_STATES),
    ("inspect_field", INSPECTION_STATES),
    ("compile_scenario", LOADED),
    ("run_scenario", &["still"]),
    ("sample_lens", &["still"]),
    ("renewal_trial", &["still"]),
    ("renewal_inventory", &["still"]),
    ("preview_design_patch", &["still"]),
    ("commit_design_patch", &["still"]),
    ("preview_commission_restart", &["still"]),
    ("preview_qualification_input", &["still", "qualification_frozen"]),
    ("freeze_qualification_request", &["still", "qualification_frozen"]),
    ("qualification_job", &["qualification_frozen"]),
    ("engineering_memory", &["still", "qualification_frozen", "returned"]),
    ("restart_commission", &["still"]),
    ("return_commission", &["still", "qualification_frozen"]),
    ("resume_commission", &["returned"]),
    ("set_local_policy", &["still"]),
];

#[derive(Clone)]
struct OpenComponent {
    node: u32,
    kind: crate::field::NodeKind,
    layer: u8,
    pos: crate::fx::Vec2,
    charge: i64,
    open: bool,
    upkeep_rate: i64,
    capacity: i64,
}

#[derive(Clone)]
struct OpenRoute {
    route: u32,
    tail: u32,
    head: u32,
    capacity: i64,
}

fn read_open_topology(
    draft: &Json,
) -> Result<
    (
        Vec<OpenComponent>,
        Vec<OpenRoute>,
        Vec<u32>,
        u8,
        crate::fx::Vec2,
    ),
    Fault,
> {
    let mut components = Vec::new();
    for value in read::list(draft, "components", crate::field::NODES_PER_RUN)? {
        read::exact_keys(
            value,
            "components",
            &[
                "capacity",
                "charge",
                "kind",
                "layer",
                "node",
                "open",
                "upkeep_rate",
                "x",
                "y",
            ],
        )?;
        let kind = crate::field::NodeKind::read(read::text(value, "kind")?)
            .filter(|kind| *kind != crate::field::NodeKind::Form)
            .ok_or_else(|| Fault::field("kind"))?;
        components.push(OpenComponent {
            node: read::int(value, "node", 1, i64::from(u32::MAX))? as u32,
            kind,
            layer: read::int(value, "layer", 0, i64::from(crate::field::MAX_LAYER))? as u8,
            pos: crate::fx::Vec2::new(
                read::int(value, "x", 0, crate::field::PLANE_SPAN - 1)?,
                read::int(value, "y", 0, crate::field::PLANE_SPAN - 1)?,
            ),
            charge: read::int(value, "charge", 0, crate::field::NODE_CHARGE_CAP)?,
            open: read::flag(value, "open")?,
            upkeep_rate: read::int(value, "upkeep_rate", 0, crate::fx::STORED_BOUND - 1)?,
            capacity: read::int(value, "capacity", 1, crate::field::NODE_CHARGE_CAP)?,
        });
    }
    let component_ids: Vec<u32> = components.iter().map(|component| component.node).collect();
    if !read::ascending(&component_ids) {
        return Err(Fault::field("components"));
    }

    let mut routes = Vec::new();
    for value in read::list(draft, "routes", crate::field::ROUTES_PER_RUN)? {
        read::exact_keys(value, "routes", &["capacity", "head", "route", "tail"])?;
        let route = OpenRoute {
            route: read::int(value, "route", 1, i64::from(u32::MAX))? as u32,
            tail: read::int(value, "tail", 1, i64::from(u32::MAX))? as u32,
            head: read::int(value, "head", 1, i64::from(u32::MAX))? as u32,
            capacity: read::int(value, "capacity", 1, crate::field::ROUTE_CAPACITY_CAP)?,
        };
        if route.tail == route.head {
            return Err(Fault::field("routes"));
        }
        routes.push(route);
    }
    let route_ids: Vec<u32> = routes.iter().map(|route| route.route).collect();
    if !read::ascending(&route_ids) {
        return Err(Fault::field("routes"));
    }
    let members = read::ids(
        draft,
        "compartment_members",
        crate::field::NODES_PER_RUN,
        i64::from(u32::MAX),
    )?;
    let supply_layer =
        read::int(draft, "supply_layer", 0, i64::from(crate::field::MAX_LAYER))? as u8;
    let supply_pos = crate::fx::Vec2::new(
        read::int(draft, "supply_x", 0, crate::field::PLANE_SPAN - 1)?,
        read::int(draft, "supply_y", 0, crate::field::PLANE_SPAN - 1)?,
    );
    Ok((components, routes, members, supply_layer, supply_pos))
}

fn apply_open_topology(field: &mut crate::state::FieldState, draft: &Json) -> Result<(), Fault> {
    let (components, routes, members, supply_layer, supply_pos) = read_open_topology(draft)?;
    if !field.layers.iter().any(|layer| layer.layer == supply_layer)
        || components
            .iter()
            .any(|component| !field.layers.iter().any(|layer| layer.layer == component.layer))
    {
        return Err(Fault::field("layer"));
    }
    if !components.is_empty() {
        field.ports.retain(|port| port.kind == crate::field::NodeKind::Form);
        if field.ports.len() + components.len() > crate::field::NODES_PER_RUN {
            return Err(Fault::field("components"));
        }
        for component in &components {
            if field.ports.iter().any(|port| port.node == component.node) {
                return Err(Fault::field("components"));
            }
            field.ports.push(crate::field::PortState {
                node: component.node,
                layer: component.layer,
                pos: component.pos,
                kind: component.kind,
                q: component.charge,
                open: component.open,
                upkeep_rate: component.upkeep_rate,
                capacity: component.capacity,
            });
        }
        field.ports.sort_by_key(|port| port.node);
        field.routes = routes
            .iter()
            .map(|route| crate::field::RouteState {
                route: route.route,
                tail: route.tail,
                head: route.head,
                capacity: route.capacity,
                flow: 0,
                formed_step: field.step,
            })
            .collect();
        for route in &field.routes {
            if !field.ports.iter().any(|port| port.node == route.tail)
                || !field.ports.iter().any(|port| port.node == route.head)
            {
                return Err(Fault::field("routes"));
            }
        }
        for layer in &mut field.layers {
            layer.port_ids = field
                .ports
                .iter()
                .filter(|port| {
                    port.layer == layer.layer && port.kind != crate::field::NodeKind::Form
                })
                .map(|port| port.node)
                .collect();
        }
        if members.iter().any(|member| !field.ports.iter().any(|port| port.node == *member)) {
            return Err(Fault::field("compartment_members"));
        }
        field.physical_compartment.members = members;
        field.boundaries.drawn.clear();
        field.boundaries.authored.clear();
        field.signals.clear();
        field.next_signal_id = 1;
        field.next_node_id = field
            .ports
            .iter()
            .map(|port| port.node)
            .max()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or_else(|| Fault::field("components"))?;
        field.next_route_id = field
            .routes
            .iter()
            .map(|route| route.route)
            .max()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or_else(|| Fault::field("routes"))?;
    }
    field.materials = read_open_materials(draft)?;

    if field.currents.is_empty() {
        field.currents.push(crate::field::CurrentState {
            id: 1,
            layer: supply_layer,
            path: vec![supply_pos],
            width: read::int(draft, "supply_width", 8 * 65_536, crate::fx::STORED_BOUND - 1)?,
            strength: read::int(draft, "supply_per_step", 0, crate::field::CURRENT_STRENGTH_CAP)?,
            duty: crate::state::FRAC_ONE,
            period: 120,
            phase: 0,
            bright: true,
            active: true,
        });
    } else {
        for current in &mut field.currents {
            current.layer = supply_layer;
            current.path = vec![supply_pos];
        }
    }
    for layer in &mut field.layers {
        layer.current_ids = field
            .currents
            .iter()
            .filter(|current| current.layer == layer.layer)
            .map(|current| current.id)
            .collect();
    }
    crate::field::validate(field)
}

fn read_open_materials(draft: &Json) -> Result<Vec<crate::field::MaterialState>, Fault> {
    let mut materials = Vec::new();
    for value in read::list(draft, "materials", crate::field::MATERIALS_PER_RUN)? {
        read::exact_keys(
            value,
            "materials",
            &["amount", "kind", "layer", "material", "x", "y"],
        )?;
        let kind = match read::one_of(
            value,
            "kind",
            &["junction_blank", "boundary_blank", "conductor"],
        )? {
            0 => crate::field::MaterialKind::JunctionBlank,
            1 => crate::field::MaterialKind::BoundaryBlank,
            _ => crate::field::MaterialKind::Conductor,
        };
        materials.push(crate::field::MaterialState {
            material: read::int(value, "material", 1, i64::from(u32::MAX))? as u32,
            kind,
            amount: read::int(value, "amount", 1, i64::from(u16::MAX))? as u16,
            layer: read::int(value, "layer", 0, i64::from(crate::field::MAX_LAYER))? as u8,
            pos: crate::fx::Vec2::new(
                read::int(value, "x", 0, crate::field::PLANE_SPAN - 1)?,
                read::int(value, "y", 0, crate::field::PLANE_SPAN - 1)?,
            ),
            claimed: false,
        });
    }
    let ids: Vec<u32> = materials.iter().map(|material| material.material).collect();
    if !read::ascending(&ids) {
        return Err(Fault::field("materials"));
    }
    Ok(materials)
}

fn read_open_intervention(draft: &Json) -> Result<(u32, Option<PlanCommand>), Fault> {
    let value = read::map(draft, "intervention")?;
    read::exact_keys(
        value,
        "intervention",
        &["amount", "duration", "onset", "target", "tool"],
    )?;
    let onset = read::int(value, "onset", 0, 1_800)? as u32;
    let duration = read::int(value, "duration", 1, 1_800)? as u16;
    let amount = read::int(value, "amount", 0, 100)?;
    let target = read::int(value, "target", 0, i64::from(u32::MAX))? as u32;
    let plan = match read::one_of(value, "tool", &["none", "blade", "clamp", "breach"])? {
        0 => None,
        1 => Some(PlanCommand::Cut { route: target }),
        2 => Some(PlanCommand::LimitRoute {
            route: target,
            retained_fraction: ((100 - amount).max(1) * crate::state::FRAC_ONE / 100)
                .clamp(1, crate::state::FRAC_ONE - 1),
            duration,
        }),
        _ => Some(PlanCommand::RaiseLeak {
            delta: (amount.max(1) * crate::field::LEAK_FRAC_CAP / 100).max(1),
            duration,
        }),
    };
    Ok((onset, plan))
}

fn transition_register_digest(kind: &str, canonical_value: &str) -> String {
    let mut definition = String::new();
    let mut object = Obj::new(&mut definition);
    object.text("kind", kind);
    object.raw("value", canonical_value);
    object.int("version", 1);
    object.end();
    hex_bytes(&sha256::digest(definition.as_bytes()))
}

fn transition_embodied_register_digest(
    kind: crate::engineering::EngineeringTransitionRegisterKind,
    field: &crate::state::FieldState,
) -> String {
    let mut value = String::new();
    let mut object = Obj::new(&mut value);
    object.text("embodied_hash", &field.embodied_hash());
    object.end();
    transition_register_digest(kind.name(), &value)
}

fn transition_register_addresses(
    kind: crate::engineering::EngineeringTransitionRegisterKind,
    field: &crate::state::FieldState,
) -> Vec<String> {
    use crate::engineering::EngineeringTransitionRegisterKind as Kind;
    let mut addresses = match kind {
        Kind::LivePositions => field
            .ports
            .iter()
            .map(|port| format!("component:{}", port.node))
            .chain(
                field
                    .materials
                    .iter()
                    .map(|material| format!("material:{}", material.material)),
            )
            .collect(),
        Kind::StoredCharge => field
            .ports
            .iter()
            .map(|port| format!("component:{}", port.node))
            .chain(
                field
                    .forms
                    .iter()
                    .map(|form| format!("form:{}", form.node)),
            )
            .chain(
                field
                    .materials
                    .iter()
                    .map(|material| format!("material:{}", material.material)),
            )
            .collect(),
        Kind::PolicyTimers => field
            .policy_runtime
            .iter()
            .map(|runtime| format!("component:{}", runtime.address))
            .collect(),
        Kind::ControllerState => field
            .policy_runtime
            .iter()
            .map(|runtime| format!("component:{}", runtime.address))
            .chain(
                field
                    .route_controls
                    .iter()
                    .map(|control| format!("route:{}", control.route)),
            )
            .chain(
                field
                    .forms
                    .iter()
                    .map(|form| format!("form:{}", form.node)),
            )
            .chain(
                field
                    .signals
                    .iter()
                    .map(|signal| format!("signal:{}", signal.signal)),
            )
            .collect(),
        Kind::EventWindow | Kind::ProvisionalCriteria => Vec::new(),
    };
    addresses.sort();
    addresses.dedup();
    addresses
}

fn engineering_transition_registers(
    state: &RunState,
    target: &crate::state::FieldState,
) -> Result<Vec<crate::engineering::EngineeringTransitionRegisterConsequence>, Fault> {
    use crate::engineering::{
        EngineeringTransitionRegisterConsequence as Register,
        EngineeringTransitionRegisterKind as Kind,
    };

    let mut registers = Vec::new();
    for kind in [
        Kind::LivePositions,
        Kind::StoredCharge,
        Kind::PolicyTimers,
        Kind::ControllerState,
    ] {
        let mut addresses = transition_register_addresses(kind, &state.now);
        addresses.extend(transition_register_addresses(kind, target));
        registers.push(Register::new(
            kind,
            &transition_embodied_register_digest(kind, &state.now),
            &transition_embodied_register_digest(kind, target),
            addresses,
        )?);
    }

    let target_trace = crate::state::Trace::opening(target.clone());
    let mut event_addresses: Vec<String> = state
        .trace
        .steps
        .iter()
        .map(|step| format!("step:{}", step.step))
        .collect();
    event_addresses.sort();
    event_addresses.dedup();
    registers.push(Register::new(
        Kind::EventWindow,
        &transition_register_digest(Kind::EventWindow.name(), &state.trace.written()),
        &transition_register_digest(Kind::EventWindow.name(), &target_trace.written()),
        event_addresses,
    )?);

    let before_criterion = state
        .criterion
        .as_ref()
        .map(crate::criterion::CriterionRuntime::written)
        .unwrap_or_else(|| "null".to_string());
    let after_criterion = state
        .scenario
        .criterion(state.progress.chapter_index)
        .map(|_| crate::criterion::CriterionRuntime::opening(0).written())
        .unwrap_or_else(|| "null".to_string());
    registers.push(Register::new(
        Kind::ProvisionalCriteria,
        &transition_register_digest(Kind::ProvisionalCriteria.name(), &before_criterion),
        &transition_register_digest(Kind::ProvisionalCriteria.name(), &after_criterion),
        state
            .criterion
            .as_ref()
            .map(|_| vec!["criterion:active".to_string()])
            .unwrap_or_default(),
    )?);
    Ok(registers)
}

fn push_transition_compatibility_issue(
    issues: &mut Vec<crate::engineering::EngineeringTransitionCompatibilityIssue>,
    address: Option<String>,
    code: crate::engineering::EngineeringCompatibilityCode,
    disposition: crate::engineering::EngineeringCompatibilityDisposition,
) -> Result<(), Fault> {
    let issue = crate::engineering::EngineeringTransitionCompatibilityIssue::new(
        address,
        code,
        disposition,
    )?;
    if !issues.contains(&issue) {
        issues.push(issue);
    }
    Ok(())
}

fn transition_contract_has_hardware(contract: &crate::content::ContractSpec, kind: &str) -> bool {
    contract.capabilities.hardware.iter().any(|held| held == kind)
}

fn compile_transition_policy_action(
    issues: &mut Vec<crate::engineering::EngineeringTransitionCompatibilityIssue>,
    contract: &crate::content::ContractSpec,
    declared_components: &[(u32, crate::field::NodeKind)],
    declared_routes: &[(u32, u32, u32)],
    field: &crate::state::FieldState,
    address: u32,
    action: &crate::policy::LocalAction,
) -> Result<(), Fault> {
    use crate::engineering::{
        EngineeringCompatibilityCode as Code,
        EngineeringCompatibilityDisposition as Disposition,
    };

    let policy_address = Some(format!("policy:component:{address}"));
    if !contract.capabilities.actions.iter().any(|kind| kind == action.name()) {
        push_transition_compatibility_issue(
            issues,
            policy_address.clone(),
            Code::UnsupportedAction,
            Disposition::GeneratorEditRequired,
        )?;
    }

    let mobile = matches!(
        action,
        crate::policy::LocalAction::SeekSupply { .. }
            | crate::policy::LocalAction::SeekPort { .. }
            | crate::policy::LocalAction::SeekSignal { .. }
            | crate::policy::LocalAction::ChangeDepth { .. }
            | crate::policy::LocalAction::Couple { .. }
            | crate::policy::LocalAction::UseAbility
    );
    if mobile && !transition_contract_has_hardware(contract, "mobile_component") {
        push_transition_compatibility_issue(
            issues,
            Some(format!("component:{address}")),
            Code::MissingHardware,
            Disposition::GeneratorEditRequired,
        )?;
    }
    if mobile
        && (!declared_components
            .iter()
            .any(|(node, kind)| *node == address && *kind == crate::field::NodeKind::Form)
            || !field.forms.iter().any(|form| form.node == address))
    {
        push_transition_compatibility_issue(
            issues,
            Some(format!("component:{address}")),
            Code::MissingHardware,
            Disposition::HardIncompatibility,
        )?;
    }

    match action {
        crate::policy::LocalAction::Couple { radius } => {
            if !transition_contract_has_hardware(contract, "coupler") {
                push_transition_compatibility_issue(
                    issues,
                    Some(format!("component:{address}")),
                    Code::MissingHardware,
                    Disposition::GeneratorEditRequired,
                )?;
            }
            if *radius > crate::field::pulse_radius(crate::state::FRAC_ONE) {
                push_transition_compatibility_issue(
                    issues,
                    policy_address.clone(),
                    Code::GeneratorEditRequired,
                    Disposition::GeneratorEditRequired,
                )?;
            }
        }
        crate::policy::LocalAction::SetInterface { .. } => {
            if !transition_contract_has_hardware(contract, "interface_actuator") {
                push_transition_compatibility_issue(
                    issues,
                    Some(format!("component:{address}")),
                    Code::MissingHardware,
                    Disposition::GeneratorEditRequired,
                )?;
            }
        }
        crate::policy::LocalAction::SetRoute { route, capacity_limit, .. } => {
            if !transition_contract_has_hardware(contract, "route_actuator") {
                push_transition_compatibility_issue(
                    issues,
                    Some(format!("component:{address}")),
                    Code::MissingHardware,
                    Disposition::GeneratorEditRequired,
                )?;
            }
            let declared = declared_routes
                .iter()
                .find(|(held, _, _)| held == route)
                .copied();
            let embodied = field.routes.iter().find(|held| held.route == *route);
            if declared.is_none_or(|(_, tail, _)| tail != address)
                || embodied.is_none_or(|held| {
                    held.tail != address || *capacity_limit > held.capacity
                })
            {
                push_transition_compatibility_issue(
                    issues,
                    Some(format!("route:{route}")),
                    Code::InvalidRouteOwnership,
                    Disposition::GeneratorEditRequired,
                )?;
            }
        }
        crate::policy::LocalAction::UseAbility => {
            if !transition_contract_has_hardware(contract, "finite_reserve") {
                push_transition_compatibility_issue(
                    issues,
                    Some(format!("component:{address}")),
                    Code::MissingHardware,
                    Disposition::GeneratorEditRequired,
                )?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn compile_transition_policy_condition(
    issues: &mut Vec<crate::engineering::EngineeringTransitionCompatibilityIssue>,
    contract: &crate::content::ContractSpec,
    declared_routes: &[(u32, u32, u32)],
    field: &crate::state::FieldState,
    address: u32,
    condition: &crate::policy::LocalCondition,
) -> Result<(), Fault> {
    use crate::engineering::{
        EngineeringCompatibilityCode as Code,
        EngineeringCompatibilityDisposition as Disposition,
    };

    let policy_address = Some(format!("policy:component:{address}"));
    if !contract
        .capabilities
        .conditions
        .iter()
        .any(|kind| kind == condition.name())
    {
        push_transition_compatibility_issue(
            issues,
            policy_address.clone(),
            Code::UnsupportedCondition,
            Disposition::GeneratorEditRequired,
        )?;
    }
    if matches!(
        condition,
        crate::policy::LocalCondition::Supply { .. }
            | crate::policy::LocalCondition::TargetInRange { .. }
            | crate::policy::LocalCondition::SignalPresent { .. }
    ) && !transition_contract_has_hardware(contract, "local_sensor")
    {
        push_transition_compatibility_issue(
            issues,
            Some(format!("component:{address}")),
            Code::MissingHardware,
            Disposition::GeneratorEditRequired,
        )?;
    }
    if let crate::policy::LocalCondition::TargetInRange { radius } = condition {
        if *radius > crate::field::pulse_radius(crate::state::FRAC_ONE) {
            push_transition_compatibility_issue(
                issues,
                policy_address,
                Code::GeneratorEditRequired,
                Disposition::GeneratorEditRequired,
            )?;
        }
    }
    let route = match condition {
        crate::policy::LocalCondition::RouteFlowBelow { route, .. }
        | crate::policy::LocalCondition::RouteFlowAbove { route, .. } => Some(*route),
        _ => None,
    };
    if let Some(route) = route {
        if !transition_contract_has_hardware(contract, "route_actuator") {
            push_transition_compatibility_issue(
                issues,
                Some(format!("component:{address}")),
                Code::MissingHardware,
                Disposition::GeneratorEditRequired,
            )?;
        }
        let declared = declared_routes
            .iter()
            .find(|(held, _, _)| *held == route)
            .copied();
        let embodied = field.routes.iter().find(|held| held.route == route);
        if declared.is_none_or(|(_, tail, head)| tail != address && head != address)
            || embodied.is_none_or(|held| held.tail != address && held.head != address)
        {
            push_transition_compatibility_issue(
                issues,
                Some(format!("route:{route}")),
                Code::InvalidRouteOwnership,
                Disposition::GeneratorEditRequired,
            )?;
        }
    }
    Ok(())
}

fn transition_component_field_stance(
    issues: &[crate::engineering::EngineeringTransitionCompatibilityIssue],
    node: u32,
) -> (
    crate::engineering::EngineeringAssemblyCompatibilityDisposition,
    Option<crate::engineering::EngineeringCompatibilityCode>,
) {
    use crate::engineering::{
        EngineeringAssemblyCompatibilityDisposition as Disposition,
        EngineeringCompatibilityDisposition as IssueDisposition,
    };
    let address = format!("component:{node}");
    let issue = issues
        .iter()
        .find(|issue| issue.address.as_deref() == Some(address.as_str()));
    match issue {
        Some(issue) => (
            if issue.disposition == IssueDisposition::AssemblyAdaptation {
                Disposition::AdaptationRequired
            } else {
                Disposition::HardRefusal
            },
            Some(issue.code),
        ),
        None => (Disposition::RetainedByAddress, None),
    }
}

fn transition_compartment_members_value(members: &[u32]) -> String {
    let mut value = String::new();
    let mut object = Obj::new(&mut value);
    {
        let mut listed = object.list("members");
        for member in members {
            listed.int(i64::from(*member));
        }
        listed.end();
    }
    object.end();
    value
}

fn compile_revert_generator_compatibility(
    contract: &crate::content::ContractSpec,
    target_generator: &crate::state::GeneratorSpec,
    current_assembly: &crate::state::AssemblyTemplate,
    chapter_index: u8,
) -> Result<
    (
        Vec<crate::engineering::EngineeringAssemblyCompatibilityField>,
        Vec<crate::engineering::EngineeringTransitionCompatibilityIssue>,
    ),
    Fault,
> {
    use crate::engineering::{
        EngineeringAssemblyCompatibilityDisposition as FieldDisposition,
        EngineeringAssemblyCompatibilityField as CompatibilityField,
        EngineeringAssemblyCompatibilityFieldKind as FieldKind,
        EngineeringCompatibilityCode as Code,
        EngineeringCompatibilityDisposition as Disposition,
    };

    let field = current_assembly
        .field()
        .ok_or_else(|| Fault::field("assembly_template"))?;
    let draft = current_assembly
        .draft()
        .ok_or_else(|| Fault::field("assembly_template"))?;
    let mut issues = Vec::new();
    let declared_components = match target_generator.declared_components(chapter_index) {
        Some(components) => components,
        None => {
            push_transition_compatibility_issue(
                &mut issues,
                Some(format!("generator:chapter:{chapter_index}")),
                Code::GeneratorEditRequired,
                Disposition::GeneratorEditRequired,
            )?;
            Vec::new()
        }
    };
    let declared_routes = target_generator
        .declared_routes(chapter_index)
        .unwrap_or_default();

    if declared_components.len() > usize::from(contract.limits.max_components) {
        push_transition_compatibility_issue(
            &mut issues,
            Some("generator:components".to_string()),
            Code::GeneratorEditRequired,
            Disposition::GeneratorEditRequired,
        )?;
    }
    if declared_routes.len() > usize::from(contract.limits.max_routes) {
        push_transition_compatibility_issue(
            &mut issues,
            Some("generator:routes".to_string()),
            Code::GeneratorEditRequired,
            Disposition::GeneratorEditRequired,
        )?;
    }

    for (node, kind) in &declared_components {
        if field
            .ports
            .iter()
            .find(|port| port.node == *node)
            .is_none_or(|port| port.kind != *kind)
        {
            push_transition_compatibility_issue(
                &mut issues,
                Some(format!("component:{node}")),
                Code::MissingHardware,
                Disposition::HardIncompatibility,
            )?;
        }
    }
    for port in &field.ports {
        if !declared_components
            .iter()
            .any(|(node, kind)| *node == port.node && *kind == port.kind)
        {
            push_transition_compatibility_issue(
                &mut issues,
                Some(format!("component:{}", port.node)),
                Code::GeneratorEditRequired,
                Disposition::GeneratorEditRequired,
            )?;
        }
    }

    for (route, tail, head) in &declared_routes {
        if field
            .routes
            .iter()
            .find(|held| held.route == *route)
            .is_none_or(|held| held.tail != *tail || held.head != *head)
        {
            push_transition_compatibility_issue(
                &mut issues,
                Some(format!("route:{route}")),
                Code::InvalidRouteOwnership,
                Disposition::HardIncompatibility,
            )?;
        }
    }
    for route in &field.routes {
        if !declared_routes.iter().any(|(id, tail, head)| {
            *id == route.route && *tail == route.tail && *head == route.head
        }) {
            push_transition_compatibility_issue(
                &mut issues,
                Some(format!("route:{}", route.route)),
                Code::GeneratorEditRequired,
                Disposition::GeneratorEditRequired,
            )?;
        }
    }

    let route_defaults = target_generator.route_defaults(chapter_index);
    if !declared_routes.is_empty() && route_defaults.is_empty() {
        push_transition_compatibility_issue(
            &mut issues,
            Some("generator:route_defaults".to_string()),
            Code::GeneratorEditRequired,
            Disposition::GeneratorEditRequired,
        )?;
    }
    for (route, tail, _) in &declared_routes {
        let embodied = field.routes.iter().find(|held| held.route == *route);
        if route_defaults
            .iter()
            .find(|control| control.route == *route)
            .is_none_or(|control| {
                control.controller != *tail
                    || control.allocation_weight == 0
                    || embodied.is_none_or(|held| {
                        control.capacity_limit < 0 || control.capacity_limit > held.capacity
                    })
            })
        {
            push_transition_compatibility_issue(
                &mut issues,
                Some(format!("route:{route}")),
                Code::InvalidRouteOwnership,
                Disposition::GeneratorEditRequired,
            )?;
        }
    }

    for component in target_generator.local_policy().components() {
        if !declared_components
            .iter()
            .any(|(node, _)| *node == component.address)
            || !field
                .ports
                .iter()
                .any(|port| port.node == component.address)
        {
            push_transition_compatibility_issue(
                &mut issues,
                Some(format!("policy:component:{}", component.address)),
                Code::InvalidAddress,
                Disposition::GeneratorEditRequired,
            )?;
        }
        if component.rules.len() > usize::from(contract.limits.max_rules_per_component) {
            push_transition_compatibility_issue(
                &mut issues,
                Some(format!("policy:component:{}", component.address)),
                Code::GeneratorEditRequired,
                Disposition::GeneratorEditRequired,
            )?;
        }
        compile_transition_policy_action(
            &mut issues,
            contract,
            &declared_components,
            &declared_routes,
            field,
            component.address,
            &component.fallback,
        )?;
        for rule in &component.rules {
            compile_transition_policy_condition(
                &mut issues,
                contract,
                &declared_routes,
                field,
                component.address,
                &rule.condition,
            )?;
            compile_transition_policy_action(
                &mut issues,
                contract,
                &declared_components,
                &declared_routes,
                field,
                component.address,
                &rule.action,
            )?;
        }
    }

    if !target_generator.establishes_field(chapter_index, field) && issues.is_empty() {
        push_transition_compatibility_issue(
            &mut issues,
            Some("assembly:opening".to_string()),
            Code::RegimeIncompatibleAssembly,
            Disposition::HardIncompatibility,
        )?;
    }

    let mut fields = Vec::new();
    for component in &draft.components {
        let address = format!("component:{}", component.node);
        let (disposition, issue_code) =
            transition_component_field_stance(&issues, component.node);
        for (kind, value) in [
            (FieldKind::ComponentPosition, component.pos.written()),
            (FieldKind::ComponentLayer, component.layer.to_string()),
            (FieldKind::StoredCharge, component.q.to_string()),
            (FieldKind::InterfaceState, component.open.to_string()),
        ] {
            fields.push(CompatibilityField::new(
                &address,
                kind,
                disposition,
                &value,
                &value,
                issue_code,
            )?);
        }
    }
    for form in &draft.forms {
        let address = format!("form:{}", form.node);
        let (disposition, issue_code) = transition_component_field_stance(&issues, form.node);
        let reserve = form.reserve.to_string();
        fields.push(CompatibilityField::new(
            &address,
            FieldKind::FormReserve,
            disposition,
            &reserve,
            &reserve,
            issue_code,
        )?);
        let blanks = form
            .junction_blanks
            .map(|value| value.to_string())
            .unwrap_or_else(|| "null".to_string());
        fields.push(CompatibilityField::new(
            &address,
            FieldKind::JunctionBlanks,
            disposition,
            &blanks,
            &blanks,
            issue_code,
        )?);
    }
    for material in &draft.materials {
        let address = format!("material:{}", material.material);
        for (kind, value) in [
            (FieldKind::MaterialAmount, material.amount.to_string()),
            (FieldKind::MaterialPosition, material.pos.written()),
            (FieldKind::MaterialLayer, material.layer.to_string()),
        ] {
            fields.push(CompatibilityField::new(
                &address,
                kind,
                FieldDisposition::RetainedUnchanged,
                &value,
                &value,
                None,
            )?);
        }
    }
    for current in &draft.currents {
        let address = format!("current:{}", current.current);
        for (kind, value) in [
            (FieldKind::CurrentActive, current.active.to_string()),
            (FieldKind::CurrentPhase, current.phase.to_string()),
        ] {
            fields.push(CompatibilityField::new(
                &address,
                kind,
                FieldDisposition::RetainedUnchanged,
                &value,
                &value,
                None,
            )?);
        }
    }
    let compartment_address = "physical_compartment:opening";
    let members = transition_compartment_members_value(&draft.physical_compartment.members);
    fields.push(CompatibilityField::new(
        compartment_address,
        FieldKind::PhysicalCompartmentMembers,
        FieldDisposition::RetainedUnchanged,
        &members,
        &members,
        None,
    )?);
    let leakage = draft
        .physical_compartment
        .leak_per_exposed_contact_per_step
        .to_string();
    fields.push(CompatibilityField::new(
        compartment_address,
        FieldKind::PhysicalCompartmentLeakage,
        FieldDisposition::RetainedUnchanged,
        &leakage,
        &leakage,
        None,
    )?);

    Ok((fields, issues))
}

struct PendingEngineeringTransition {
    preview: crate::engineering::EngineeringTransitionPreview,
    target_field: crate::state::FieldState,
    target_scenario: crate::state::ScenarioSpec,
    target_view: crate::state::ViewDeclaration,
    authored_chapter: Chapter,
}

/// A session over the WASM boundary: the state a `Core` instance holds.
pub struct Session {
    run: Option<Run>,
    store: RecordStore,
    /// The authored content the worker handed over at construction, or the
    /// fault reading it produced. The document puts content validation at
    /// `init_run`, so a bundle that does not read is held here and answered
    /// there rather than refusing the session outright.
    content: Result<Content, Fault>,
    /// Events the session itself raised, beside the run's own.
    events: Vec<String>,
    /// The most recent read-only engineering transition candidate. It is
    /// disposable session state: commit re-derives every current guard and an
    /// accepted receipt moves into the authoritative child branch/save.
    engineering_transition: Option<PendingEngineeringTransition>,
}

impl Session {
    /// Completes the version handshake the worker opens.
    ///
    /// `init_json` carries the versions the worker speaks. Agreement on both
    /// is the whole of the handshake; a disagreement is returned as the
    /// `protocol` error envelope and the caller never gets a session.
    pub fn new(init_json: &str) -> Result<Self, String> {
        let spoken = parse(init_json).ok();
        let protocol = spoken.as_ref().and_then(|body| body.get("protocol")).and_then(Json::as_int);
        let save_version =
            spoken.as_ref().and_then(|body| body.get("save_version")).and_then(Json::as_int);
        if protocol != Some(i64::from(PROTOCOL_VERSION)) || save_version != Some(SAVE_VERSION) {
            let mut detail = String::new();
            let mut object = Obj::new(&mut detail);
            object.int("protocol", i64::from(PROTOCOL_VERSION));
            object.int("save_version", SAVE_VERSION);
            object.end();
            return Err(Fault::detailed(Code::Protocol, detail).write());
        }
        // The authored content arrives with the versions, in the one call the
        // locked WASM surface has for opening a core: the worker imports the
        // bytes statically and hands them across with the digest the build
        // embedded, and nothing is ever fetched.
        let content = match spoken.as_ref().and_then(|body| body.get("content")) {
            Some(value) => content::read_bundle(value),
            None => Err(Fault::because(Code::ContentInvalid, "content")),
        };
        Ok(Self {
            run: None,
            store: RecordStore::new(),
            content,
            events: Vec::new(),
            engineering_transition: None,
        })
    }

    /// The content hash this build reports, and the empty digest when no
    /// content read.
    fn content_hash(&self) -> String {
        match &self.content {
            Ok(content) => content.hash.clone(),
            Err(_) => hex_bytes(&sha256::digest(&[])),
        }
    }

    /// The chapter a loaded run stands on, and none when the run's own content
    /// hash is not this build's.
    ///
    /// A run restored under a different hash carries on — that is the locked
    /// behaviour, and `content_changed` says so — but the authored sequence
    /// does not run against a Field it was not authored for: the objective ids
    /// and the Nodes they name belong to the content the run was opened under.
    fn chapter(&self) -> Option<&Chapter> {
        let content = self.content.as_ref().ok()?;
        let run = self.run.as_ref()?;
        if run.state().scenario.content_hash() != content.hash {
            return None;
        }
        if let Some(contract_id) = run.state().scenario.contract_id() {
            let contract = content.contract(contract_id)?;
            return content
                .chapters
                .iter()
                .find(|chapter| chapter.id == contract.opening.chapter());
        }
        content.chapter(run.state().progress.chapter_index)
    }

    /// The lifecycle state, as the state error reports it: the mode of the
    /// loaded run, or idle before one is loaded.
    pub fn lifecycle(&self) -> &'static str {
        self.run.as_ref().map_or(IDLE, Run::lifecycle)
    }

    /// The completed-step counter of the loaded run, and 0 before one loads.
    pub fn step(&self) -> u32 {
        self.run.as_ref().map_or(0, Run::step)
    }

    /// Answers one command. The return value is the inner response — `ok` with
    /// a body, or `ok` false with an error envelope — and the worker wraps it
    /// in the message envelope with the correlation id.
    pub fn command(&mut self, kind: &str, body_json: &str) -> String {
        match self.answer(kind, body_json) {
            Ok(body) => ok_response(&body),
            Err(fault) => fault.response(),
        }
    }

    fn answer(&mut self, kind: &str, body_json: &str) -> Result<String, Fault> {
        let Some((_, valid_in)) = COMMANDS.iter().find(|(name, _)| *name == kind) else {
            let mut detail = String::new();
            let mut object = Obj::new(&mut detail);
            object.text("cmd", kind);
            object.end();
            return Err(Fault::detailed(Code::Protocol, detail));
        };

        let lifecycle = self.lifecycle();
        if !valid_in.contains(&lifecycle) {
            return Err(state_fault(lifecycle, valid_in));
        }

        let body = parse(body_json).map_err(|reason| Fault::because(Code::Validation, reason))?;
        if !body.is_map() {
            return Err(Fault::because(Code::Validation, "body_not_an_object"));
        }

        match kind {
            "list_contracts" => {
                let completed = if body.get("receipts").is_some() {
                    self.completed_contracts_from_receipts(&body, &["receipts"])?
                } else {
                    read::exact_keys(&body, "body", &[])?;
                    Vec::new()
                };
                self.content
                    .as_ref()
                    .map_err(Fault::clone)?
                    .contract_catalog_written(&completed)
            }
            "open_contract" => self.open_contract(&body),
            "init_run" => self.init_run(&body),
            "input_frame" => {
                if lifecycle == "qualification_frozen" {
                    let frame = crate::run::InputFrame::read(&body)?;
                    if !frame.is_passive_snapshot() {
                        return Err(Fault::field("qualification_request"));
                    }
                    let mut out = String::new();
                    let mut object = Obj::new(&mut out);
                    object.int("steps_run", 0);
                    object.end();
                    return Ok(out);
                }
                // Disjoint fields: the campaign is read out of the content and
                // the run is advanced, and neither borrow reaches the other.
                // The whole campaign crosses rather than one chapter of it,
                // because a step can be the one that carries the run into the
                // next chapter and the step after it belongs to that one.
                let campaign = match (self.content.as_ref(), self.run.as_ref()) {
                    (Ok(content), Some(run))
                        if run.state().scenario.content_hash() == content.hash
                            && run.state().scenario.contract_id().is_none() =>
                    {
                        Some(content)
                    }
                    _ => None,
                };
                let run = self.run.as_mut().ok_or_else(|| state_fault(IDLE, LOADED))?;
                let ran = run.input_frame(&body, campaign)?;
                self.store_pending();
                self.drain_events();
                self.autosave();
                // No response to a valid frame crosses the boundary: the frame
                // event the worker raises is its acknowledgement. The count
                // here is the core's own answer to the worker, exact where the
                // event's `u8` saturates.
                let mut out = String::new();
                let mut object = Obj::new(&mut out);
                object.int("steps_run", i64::from(ran));
                object.end();
                Ok(out)
            }
            "export_run" => {
                read::exact_keys(&body, "body", &[])?;
                self.loaded()?.export()
            }
            "import_run" => self.import_run(&body),
            "reopen_archive" => self.reopen_archive(&body),
            "run_analysis" => crate::analysis::run(self.loaded()?.state(), &body),
            "sample_instrument" => crate::instrument::sample(self.loaded()?.state(), &body),
            "inspect_field" => self.inspect_field(&body),
            "restore_checkpoint" => self.restore_by_id(&body, false),
            "recover_branch" => self.restore_by_id(&body, true),
            "queue_plan" => self.queue_plan(&body),
            "compile_scenario" => self.compile_scenario(&body),
            "run_scenario" => self.run_scenario(&body),
            "sample_lens" => {
                read::exact_keys(&body, "body", &[])?;
                self.loaded()?.sample_lens()
            }
            "renewal_trial" => self.renewal_trial(&body),
            "renewal_inventory" => {
                read::exact_keys(&body, "body", &[])?;
                self.renewal_inventory()
            }
            "set_focus" => self.set_focus(&body),
            "preview_design_patch" => self.preview_design_patch(&body),
            "commit_design_patch" => self.commit_design_patch(&body),
            "preview_commission_restart" => self.preview_commission_restart(&body),
            "preview_qualification_input" => self.preview_qualification_input(&body),
            "freeze_qualification_request" => self.freeze_qualification_request(&body),
            "qualification_job" => self.qualification_job(&body),
            "engineering_memory" => self.engineering_memory(&body),
            "restart_commission" => self.restart_commission(&body),
            "return_commission" => self.return_commission(&body),
            "resume_commission" => self.resume_commission(&body),
            "set_local_policy" => self.set_local_policy(&body),
            "undo_plan" => {
                read::exact_keys(&body, "body", &[])?;
                let run = self.loaded()?;
                // An empty queue succeeds and changes nothing; the shell reads
                // `remaining: 0` and leaves Still Mode on the second Escape.
                let remaining = run.undo_plan();
                let mut out = String::new();
                let mut object = Obj::new(&mut out);
                object.raw("queue", &run.queue().written(run.impulse()));
                object.int("remaining", remaining as i64);
                object.end();
                Ok(out)
            }
            "commit_plan" => {
                read::exact_keys(&body, "body", &[])?;
                let run = self.loaded()?;
                let applied = run.commit_plan()?;
                let mut out = String::new();
                let mut object = Obj::new(&mut out);
                write_run_identity(&mut object, run.state());
                object.int("applied", i64::from(applied));
                object.text(
                    "generator_spec_hash",
                    &run.state().scenario.generator().specification_hash(),
                );
                object.int("impulse", i64::from(run.impulse()));
                object.raw(
                    "local_policy",
                    &run.state().scenario.generator().local_policy().written(),
                );
                object.raw(
                    "route_defaults",
                    &run.state()
                        .scenario
                        .generator()
                        .route_defaults_written(run.state().progress.chapter_index),
                );
                object.text("scenario_hash", &run.state().scenario.scenario_hash());
                // The slate the run stands under once the commit has answered:
                // the one an applying commit just reassembled, the one already
                // standing when a commit applied nothing, and ordinal 0 for a
                // run standing under no slate at all. The `review_ready` the
                // commit raises carries the record itself, and rides after this
                // response as the locked event ordering has it.
                object.int(
                    "slate_ordinal",
                    run.standing_slate().map_or(0, |slate| i64::from(slate.ordinal)),
                );
                object.end();
                // The record the reassembly raised is drained here, after the
                // answer is built and before it is returned, so the caller
                // reads the response first and the `review_ready` after it.
                self.drain_events();
                Ok(out)
            }
            _ => Err(state_fault(lifecycle, valid_in)),
        }
    }

    /// Queues one proposed change.
    ///
    /// The body carries the tagged union and nothing else; the run holds the
    /// entry to the preconditions its variant declares, against the projection
    /// every earlier entry has been applied to, and to what the queue would
    /// then cost. A refused entry is not queued, and the queue the answer
    /// carries is the queue as it stands.
    fn queue_plan(&mut self, body: &Json) -> Result<String, Fault> {
        read::exact_keys(body, "body", &["plan"])?;
        let plan = PlanCommand::read(read::at(body, "plan")?)?;
        let run = self.loaded()?;
        run.queue_plan(plan)?;
        let mut out = String::new();
        let mut object = Obj::new(&mut out);
        object.raw("queue", &run.queue().written(run.impulse()));
        object.end();
        Ok(out)
    }

    fn set_local_policy(&mut self, body: &Json) -> Result<String, Fault> {
        read::exact_keys(body, "body", &["policy"])?;
        let policy = crate::policy::FrozenLocalPolicy::read(body, "policy")?;
        self.require_contract_policy(&policy)?;
        let canonical = policy.written();
        let run = self.loaded()?;
        run.install_local_policy(policy)?;
        let mut out = String::new();
        let mut object = Obj::new(&mut out);
        write_run_identity(&mut object, run.state());
        object.text("control", run.state().scenario.control().name());
        object.text(
            "generator_spec_hash",
            &run.state().scenario.generator().specification_hash(),
        );
        object.raw("local_policy", &canonical);
        object.raw(
            "route_defaults",
            &run.state()
                .scenario
                .generator()
                .route_defaults_written(run.state().progress.chapter_index),
        );
        object.text("scenario_hash", &run.state().scenario.scenario_hash());
        object.end();
        Ok(out)
    }

    fn require_generator_base(run: &Run, base: &str) -> Result<(), Fault> {
        let current = run.state().scenario.generator().specification_hash();
        if current == base {
            return Ok(());
        }
        let mut detail = String::new();
        let mut object = Obj::new(&mut detail);
        object.text("current_generator_hash", &current);
        object.text("reason", "stale_base");
        object.end();
        Err(Fault::detailed(Code::State, detail))
    }

    fn require_contract_policy(
        &self,
        policy: &crate::policy::FrozenLocalPolicy,
    ) -> Result<(), Fault> {
        let Some(run) = self.run.as_ref() else {
            return Ok(());
        };
        let Some(contract_id) = run.state().scenario.contract_id()
        else {
            return Ok(());
        };
        let contract = self
            .content
            .as_ref()
            .map_err(Fault::clone)?
            .contract(contract_id)
            .ok_or_else(|| Fault::because(Code::ContentInvalid, "contract_id"))?;
        let vocabulary_permitted = policy.permitted_by(
            &contract.capabilities.actions,
            &contract.capabilities.conditions,
            usize::from(contract.limits.max_rules_per_component),
        );
        let mobile_permitted = contract
            .capabilities
            .hardware
            .iter()
            .any(|capability| capability == "mobile_component");
        let stationary_permitted = contract.capabilities.hardware.iter().any(|capability| {
            capability == "interface_actuator" || capability == "route_actuator"
        });
        let addresses_permitted = policy.components().iter().all(|component| {
            if run
                .state()
                .now
                .forms
                .iter()
                .any(|form| form.node == component.address)
            {
                mobile_permitted
            } else {
                stationary_permitted
            }
        });
        if vocabulary_permitted && addresses_permitted {
            Ok(())
        } else {
            Err(Fault::field("capabilities"))
        }
    }

    fn require_contract_route_defaults(
        &self,
        route_defaults: &[crate::policy::RouteControlState],
    ) -> Result<(), Fault> {
        let Some(run) = self.run.as_ref() else {
            return Ok(());
        };
        let Some(contract_id) = run.state().scenario.contract_id() else {
            return Ok(());
        };
        let contract = self
            .content
            .as_ref()
            .map_err(Fault::clone)?
            .contract(contract_id)
            .ok_or_else(|| Fault::because(Code::ContentInvalid, "contract_id"))?;
        let route_actuation = contract
            .capabilities
            .hardware
            .iter()
            .any(|capability| capability == "route_actuator");
        let standing = run
            .state()
            .scenario
            .generator()
            .route_defaults(run.state().progress.chapter_index);
        if route_actuation || standing.as_slice() == route_defaults {
            Ok(())
        } else {
            Err(Fault::field("capabilities"))
        }
    }

    fn preview_design_patch(&mut self, body: &Json) -> Result<String, Fault> {
        read::exact_keys(
            body,
            "body",
            &["address", "base_generator_hash", "policy", "route_defaults"],
        )?;
        let address = read::int(body, "address", 1, i64::from(u32::MAX))? as u32;
        let base = read::hex(body, "base_generator_hash", 64)?.to_string();
        let policy = crate::policy::FrozenLocalPolicy::read(body, "policy")?;
        self.require_contract_policy(&policy)?;
        let route_defaults = Self::read_route_defaults(body)?;
        self.require_contract_route_defaults(&route_defaults)?;
        let run = self.loaded()?;
        Self::require_generator_base(run, &base)?;
        let preview = run.preview_local_policy(&policy, &route_defaults, address)?;
        let mut out = String::new();
        let mut object = Obj::new(&mut out);
        object.text("base_generator_hash", &base);
        object.raw("preview", &preview.written());
        object.int("snapshot_step", i64::from(run.step()));
        object.end();
        Ok(out)
    }

    fn commit_design_patch(&mut self, body: &Json) -> Result<String, Fault> {
        read::exact_keys(
            body,
            "body",
            &["base_generator_hash", "policy", "route_defaults"],
        )?;
        let base = read::hex(body, "base_generator_hash", 64)?.to_string();
        let policy = crate::policy::FrozenLocalPolicy::read(body, "policy")?;
        self.require_contract_policy(&policy)?;
        let canonical = policy.written();
        let route_defaults = Self::read_route_defaults(body)?;
        self.require_contract_route_defaults(&route_defaults)?;
        let run = self.loaded()?;
        Self::require_generator_base(run, &base)?;
        let canonical_diff = Self::design_diff(
            run.state().scenario.generator().local_policy(),
            &run.state()
                .scenario
                .generator()
                .route_defaults(run.state().progress.chapter_index),
            &policy,
            &route_defaults,
        );
        run.install_design_patch(policy, route_defaults)?;
        let mut out = String::new();
        let mut object = Obj::new(&mut out);
        write_run_identity(&mut object, run.state());
        object.text("base_generator_hash", &base);
        object.raw("canonical_diff", &canonical_diff);
        object.text("control", run.state().scenario.control().name());
        object.text(
            "generator_spec_hash",
            &run.state().scenario.generator().specification_hash(),
        );
        object.raw("local_policy", &canonical);
        object.raw(
            "route_defaults",
            &run.state()
                .scenario
                .generator()
                .route_defaults_written(run.state().progress.chapter_index),
        );
        object.text("scenario_hash", &run.state().scenario.scenario_hash());
        object.end();
        Ok(out)
    }

    fn read_route_defaults(body: &Json) -> Result<Vec<crate::policy::RouteControlState>, Fault> {
        let mut controls = Vec::new();
        for entry in read::list(body, "route_defaults", crate::field::ROUTES_PER_RUN)? {
            controls.push(crate::policy::RouteControlState::read(entry)?);
        }
        Ok(controls)
    }

    fn preview_commission_restart(&mut self, body: &Json) -> Result<String, Fault> {
        read::exact_keys(body, "body", &[])?;
        let run = self.loaded()?;
        let state = run.state();
        if state.run_kind != crate::state::RunKind::AutomationContract {
            return Err(Fault::field("run_kind"));
        }
        let contract_id = state
            .scenario
            .contract_id()
            .ok_or_else(|| Fault::field("contract_id"))?;
        let branch = state
            .attempt_branch
            .as_ref()
            .ok_or_else(|| Fault::field("attempt_branch"))?;
        let assembly = state
            .scenario
            .assembly_template()
            .filter(|template| template.is_exact())
            .ok_or_else(|| Fault::field("assembly_template"))?;
        let next_nonce = state
            .branch_nonce
            .checked_add(1)
            .ok_or_else(|| Fault::field("branch_nonce"))?;
        let generator = state.scenario.generator();

        let mut out = String::new();
        let mut object = Obj::new(&mut out);
        write_run_identity(&mut object, state);
        object.raw("assembly_template", &assembly.written());
        object.text("boundary", "contract_opening");
        object.text("content_hash", state.scenario.content_hash());
        object.text("contract_id", contract_id);
        object.text("current_embodied_state_hash", &state.now.embodied_hash());
        object.int("current_step", i64::from(state.now.step));
        object.raw("generator_spec", &generator.written());
        object.text("generator_spec_hash", &generator.specification_hash());
        object.text("predicted_operation", "restart");
        object.text("predicted_parent_branch_id", branch.branch_id());
        object.int("predicted_branch_nonce", i64::from(next_nonce));
        object.int("preview_version", 1);
        object.text("regime", state.scenario.regime().id());
        object.text("scenario_hash", &state.scenario.scenario_hash());
        {
            let mut consequences = object.object("consequences");
            consequences.bool("create_child_branch", true);
            consequences.bool("keep_generator", true);
            consequences.bool("restore_assembly", true);
            consequences.bool("retain_evidence", true);
            consequences.end();
        }
        object.end();
        Ok(out)
    }

    fn preview_qualification_input(&mut self, body: &Json) -> Result<String, Fault> {
        read::exact_keys(body, "body", &[])?;
        let content = self.content.as_ref().map_err(Fault::clone)?;
        let run = self.run.as_ref().ok_or_else(|| state_fault(IDLE, LOADED))?;
        let state = run.state();
        if state.run_kind != crate::state::RunKind::AutomationContract {
            return Err(Fault::field("run_kind"));
        }
        if state.scenario.content_hash() != content.hash {
            return Err(Fault::because(Code::ContentInvalid, "content_hash"));
        }
        let contract_id = state
            .scenario
            .contract_id()
            .ok_or_else(|| Fault::field("contract_id"))?;
        let contract = content
            .contract(contract_id)
            .ok_or_else(|| Fault::because(Code::ContentInvalid, "contract_id"))?;
        let assembly = state
            .scenario
            .assembly_template()
            .ok_or_else(|| Fault::field("assembly_template"))?;
        let generator = state.scenario.generator();

        let mut criteria_bytes = String::new();
        {
            let mut criterion_vector = Obj::new(&mut criteria_bytes);
            {
                let mut criteria = criterion_vector.list("criteria");
                for criterion in &contract.qualification.criteria {
                    criteria.raw(&criterion.written());
                }
                criteria.end();
            }
            criterion_vector.int(
                "failure_grace_steps",
                i64::from(contract.qualification.failure_grace_steps),
            );
            criterion_vector.int("version", 1);
            criterion_vector.end();
        }
        let criterion_hash = hex_bytes(&sha256::digest(criteria_bytes.as_bytes()));

        let mut schedule_bytes = String::new();
        {
            let mut schedule = Obj::new(&mut schedule_bytes);
            schedule.int("duration_steps", i64::from(contract.qualification.duration_steps));
            schedule.raw("pressure_schedule", &state.scenario.pressure_schedule().written());
            schedule.text("regime", state.scenario.regime().id());
            schedule.text("schedule_kind", "contract_qualification_v1");
            schedule.end();
        }
        let schedule_hash = hex_bytes(&sha256::digest(schedule_bytes.as_bytes()));

        let mut grades_bytes = String::new();
        {
            let mut grades = Obj::new(&mut grades_bytes);
            for (axis, evidence, values) in [
                (
                    "complexity",
                    "components_routes_rules_and_canonical_policy_bytes",
                    &contract.grade_bands.complexity,
                ),
                (
                    "economy",
                    "typed_upkeep_leakage_overload_material_and_intervention",
                    &contract.grade_bands.economy,
                ),
                (
                    "resilience",
                    "trial_pass_vector_and_worst_retained_service",
                    &contract.grade_bands.resilience,
                ),
                (
                    "throughput",
                    "useful_delivered_service_against_declared_demand",
                    &contract.grade_bands.throughput,
                ),
            ] {
                let mut grade = grades.object(axis);
                {
                    let mut bands = grade.list("bands");
                    for value in values {
                        bands.int(*value);
                    }
                    bands.end();
                }
                grade.text("evidence", evidence);
                grade.end();
            }
            grades.end();
        }

        let mut receipt_bytes = String::new();
        {
            let mut receipt = Obj::new(&mut receipt_bytes);
            {
                let mut actions = receipt.list("actions");
                for value in &contract.unlocks.actions {
                    actions.text(value);
                }
                actions.end();
            }
            {
                let mut conditions = receipt.list("conditions");
                for value in &contract.unlocks.conditions {
                    conditions.text(value);
                }
                conditions.end();
            }
            {
                let mut hardware = receipt.list("hardware");
                for value in &contract.unlocks.hardware {
                    hardware.text(value);
                }
                hardware.end();
            }
            match &contract.unlocks.next_contract {
                Some(next) => receipt.text("next_contract", next),
                None => receipt.null("next_contract"),
            };
            receipt.end();
        }

        let mut missing = Vec::new();
        if !assembly.is_exact() {
            missing.push("exact_assembly_template");
        }
        if state.attempt.is_none() {
            missing.push("attempt_record");
        }
        if state.attempt_branch.is_none() {
            missing.push("attempt_branch");
        }

        let mut input_bytes = String::new();
        {
            let mut input = Obj::new(&mut input_bytes);
            input.raw("assembly_template", &assembly.written());
            input.bool("assembly_template_exact", assembly.is_exact());
            input.text("assembly_template_hash", assembly.hash());
            match &state.attempt_branch {
                Some(branch) => input.raw("attempt_branch", &branch.written()),
                None => input.null("attempt_branch"),
            };
            match &state.attempt {
                Some(attempt) => input.text("attempt_id", attempt.attempt_id()),
                None => input.null("attempt_id"),
            };
            match &state.attempt {
                Some(attempt) => input.raw("attempt_record", &attempt.written()),
                None => input.null("attempt_record"),
            };
            match &state.attempt_branch {
                Some(branch) => input.text("branch_id", branch.branch_id()),
                None => input.null("branch_id"),
            };
            input.int("branch_nonce", i64::from(state.branch_nonce));
            match &state.attempt_branch {
                Some(branch) => input.text("branch_operation", branch.operation().name()),
                None => input.null("branch_operation"),
            };
            {
                let mut build = input.object("build");
                build.text("package", env!("CARGO_PKG_NAME"));
                build.text("version", env!("CARGO_PKG_VERSION"));
                build.end();
            }
            input.text("content_hash", state.scenario.content_hash());
            input.text("contract_id", contract_id);
            input.raw("criterion_vector", &criteria_bytes);
            input.text("criterion_vector_hash", &criterion_hash);
            input.raw("generator_spec", &generator.written());
            input.text("generator_spec_hash", &generator.specification_hash());
            input.raw("grade_axes", &grades_bytes);
            {
                let mut absent = input.list("missing_inputs");
                for value in &missing {
                    absent.text(value);
                }
                absent.end();
            }
            match &state.attempt_branch {
                Some(branch) => match branch.parent_branch_id() {
                    Some(parent) => input.text("parent_branch_id", parent),
                    None => input.null("parent_branch_id"),
                },
                None => input.null("parent_branch_id"),
            };
            {
                let mut procedure = input.object("procedure");
                procedure.text("control_contract", "hands_off");
                procedure.text("early_resolution", "none");
                procedure.int(
                    "progress_interval_steps",
                    i64::from(contract.qualification.duration_steps.min(60)),
                );
                procedure.text("retention", "criterion_windows_first_violation_terminal");
                procedure.text("rng_algorithm", "philox4x32_10_v1");
                procedure.raw("schedule", &schedule_bytes);
                procedure.text("schedule_hash", &schedule_hash);
                procedure.text("seed_custody", "request_hash_and_trial_address");
                procedure.int("suite_version", 1);
                {
                    let mut trials = procedure.list("trial_addresses");
                    for trial in 0..contract.qualification.trial_count {
                        let mut address = trials.object();
                        address.int("trial", i64::from(trial));
                        address.end();
                    }
                    trials.end();
                }
                procedure.int("trial_count", i64::from(contract.qualification.trial_count));
                procedure.end();
            }
            input.raw("prospective_receipt", &receipt_bytes);
            input.int("protocol_version", i64::from(PROTOCOL_VERSION));
            input.text("regime", state.scenario.regime().id());
            input.text("run_kind", state.run_kind.name());
            input.text("scenario_hash", &state.scenario.scenario_hash());
            input.int("schema_version", 1);
            input.end();
        }
        let preview_hash = hex_bytes(&sha256::digest(input_bytes.as_bytes()));

        let mut out = String::new();
        let mut object = Obj::new(&mut out);
        object.raw("input", &input_bytes);
        {
            let mut absent = object.list("missing_inputs");
            for value in &missing {
                absent.text(value);
            }
            absent.end();
        }
        object.text("preview_hash", &preview_hash);
        object.int("preview_version", 1);
        object.text("status", if missing.is_empty() { "complete" } else { "incomplete" });
        object.end();
        Ok(out)
    }

    fn freeze_qualification_request(&mut self, body: &Json) -> Result<String, Fault> {
        read::exact_keys(
            body,
            "body",
            &[
                "expected_assembly_hash",
                "expected_branch_id",
                "expected_branch_nonce",
                "expected_generator_hash",
                "expected_preview_hash",
            ],
        )?;
        let expected_assembly_hash = read::hex(body, "expected_assembly_hash", 64)?;
        let expected_branch_id = read::hex(body, "expected_branch_id", 64)?;
        let expected_branch_nonce =
            read::int(body, "expected_branch_nonce", 0, i64::from(u32::MAX))? as u32;
        let expected_generator_hash = read::hex(body, "expected_generator_hash", 64)?;
        let expected_preview_hash = read::hex(body, "expected_preview_hash", 64)?;

        // Rebuild the entire input from current Rust authority. The shell's
        // preview is a stale guard only and contributes no request bytes.
        let empty = Json::Map(Vec::new());
        let preview_written = self.preview_qualification_input(&empty)?;
        let preview = parse(&preview_written)
            .map_err(|reason| Fault::because(Code::Internal, reason))?;
        if read::one_of(&preview, "status", &["complete", "incomplete"])? != 0
            || read::hex(&preview, "preview_hash", 64)? != expected_preview_hash
        {
            return Err(Fault::field("qualification_preview"));
        }
        let input = read::at(&preview, "input")?.clone();
        if read::hex(&input, "assembly_template_hash", 64)? != expected_assembly_hash
            || read::hex(&input, "branch_id", 64)? != expected_branch_id
            || read::int(&input, "branch_nonce", 0, i64::from(u32::MAX))?
                != i64::from(expected_branch_nonce)
            || read::hex(&input, "generator_spec_hash", 64)? != expected_generator_hash
        {
            return Err(Fault::field("qualification_preview"));
        }

        let request = crate::state::QualificationRequest::from_input(input)?;
        let request_id = request.request_id().to_string();
        let request_written = request.written();
        let input_written = request.input_written();
        let out = {
            let run = self.loaded()?;
            run.freeze_qualification_request(request)?;
            let state = run.state();
            let mut out = String::new();
            let mut object = Obj::new(&mut out);
            let assembly = state
                .scenario
                .assembly_template()
                .ok_or_else(|| Fault::field("assembly_template"))?;
            object.bool("assembly_template_exact", assembly.is_exact());
            object.text("assembly_template_hash", assembly.hash());
            match &state.attempt_branch {
                Some(branch) => object.raw("attempt_branch", &branch.written()),
                None => object.null("attempt_branch"),
            };
            match &state.attempt {
                Some(attempt) => object.text("attempt_id", attempt.attempt_id()),
                None => object.null("attempt_id"),
            };
            match &state.attempt {
                Some(attempt) => object.raw("attempt_record", &attempt.written()),
                None => object.null("attempt_record"),
            };
            match &state.attempt_branch {
                Some(branch) => object.text("branch_id", branch.branch_id()),
                None => object.null("branch_id"),
            };
            object.int("branch_nonce", i64::from(state.branch_nonce));
            match &state.attempt_branch {
                Some(branch) => object.text("branch_operation", branch.operation().name()),
                None => object.null("branch_operation"),
            };
            object.text("content_hash", state.scenario.content_hash());
            object.text("contract_id", state.scenario.contract_id().unwrap_or_default());
            object.text("embodied_state_hash", &state.now.embodied_hash());
            object.text(
                "generator_spec_hash",
                &state.scenario.generator().specification_hash(),
            );
            object.raw("input", &input_written);
            match &state.attempt_branch {
                Some(branch) => match branch.parent_branch_id() {
                    Some(parent) => object.text("parent_branch_id", parent),
                    None => object.null("parent_branch_id"),
                },
                None => object.null("parent_branch_id"),
            };
            object.raw("qualification_request", &request_written);
            object.text("qualification_request_id", &request_id);
            object.text("request_id", &request_id);
            object.text("run_kind", state.run_kind.name());
            object.text("scenario_hash", &state.scenario.scenario_hash());
            object.text("status", "frozen_pending_persistence");
            object.end();
            out
        };
        Ok(out)
    }

    fn unlock_receipt_definition(
        contract: &content::ContractSpec,
        content_hash: &str,
        result_id: &str,
    ) -> String {
        let mut written = String::new();
        let mut receipt = Obj::new(&mut written);
        {
            let mut values = receipt.list("actions");
            for value in &contract.unlocks.actions {
                values.text(value);
            }
            values.end();
        }
        {
            let mut values = receipt.list("conditions");
            for value in &contract.unlocks.conditions {
                values.text(value);
            }
            values.end();
        }
        receipt.text("content_hash", content_hash);
        receipt.text("contract_id", &contract.id);
        {
            let mut values = receipt.list("hardware");
            for value in &contract.unlocks.hardware {
                values.text(value);
            }
            values.end();
        }
        match &contract.unlocks.next_contract {
            Some(next) => receipt.text("next_contract", next),
            None => receipt.null("next_contract"),
        };
        {
            let mut values = receipt.list("prerequisites");
            for value in &contract.prerequisites {
                values.text(value);
            }
            values.end();
        }
        receipt.text("result_id", result_id);
        receipt.int("version", 1);
        receipt.end();
        written
    }

    fn completed_contracts_from_receipts(
        &self,
        body: &Json,
        exact_keys: &[&str],
    ) -> Result<Vec<String>, Fault> {
        read::exact_keys(body, "body", exact_keys)?;
        let content = self.content.as_ref().map_err(Fault::clone)?;
        let mut completed = Vec::new();
        let mut receipt_ids = Vec::new();
        for receipt in read::list(body, "receipts", content.contracts.len().saturating_mul(64))? {
            read::exact_keys(receipt, "receipt", &["definition", "receipt_id"])?;
            let receipt_id = read::hex(receipt, "receipt_id", 64)?;
            if receipt_ids.iter().any(|held| held == receipt_id) {
                return Err(Fault::field("receipts"));
            }
            receipt_ids.push(receipt_id.to_string());
            let definition = read::map(receipt, "definition")?;
            read::exact_keys(
                definition,
                "receipt_definition",
                &[
                    "actions",
                    "conditions",
                    "content_hash",
                    "contract_id",
                    "hardware",
                    "next_contract",
                    "prerequisites",
                    "result_id",
                    "version",
                ],
            )?;
            if read::text(definition, "content_hash")? != content.hash
                || read::int(definition, "version", 1, 1)? != 1
            {
                return Err(Fault::field("receipts"));
            }
            let contract_id = read::text(definition, "contract_id")?;
            let contract = content
                .contract(contract_id)
                .ok_or_else(|| Fault::field("receipts"))?;
            let result_id = read::hex(definition, "result_id", 64)?;
            let read_values = |key: &str, cap: usize| -> Result<Vec<String>, Fault> {
                read::list(definition, key, cap)?
                    .iter()
                    .map(|value| match value {
                        Json::Text(value) => Ok(value.clone()),
                        _ => Err(Fault::field("receipts")),
                    })
                    .collect()
            };
            let next_contract = match read::at(definition, "next_contract")? {
                Json::Null => None,
                Json::Text(value) => Some(value.as_str()),
                _ => return Err(Fault::field("receipts")),
            };
            if read_values("actions", contract.unlocks.actions.len())?.as_slice()
                != contract.unlocks.actions.as_slice()
                || read_values("conditions", contract.unlocks.conditions.len())?
                    .as_slice() != contract.unlocks.conditions.as_slice()
                || read_values("hardware", contract.unlocks.hardware.len())?
                    .as_slice() != contract.unlocks.hardware.as_slice()
                || read_values("prerequisites", contract.prerequisites.len())?
                    .as_slice() != contract.prerequisites.as_slice()
                || next_contract != contract.unlocks.next_contract.as_deref()
            {
                return Err(Fault::field("receipts"));
            }
            let canonical = Self::unlock_receipt_definition(contract, &content.hash, result_id);
            if hex_bytes(&sha256::digest(canonical.as_bytes())) != receipt_id {
                return Err(Fault::field("receipts"));
            }
            if !completed.iter().any(|held| held == contract_id) {
                completed.push(contract_id.to_string());
            }
        }
        Ok(completed)
    }

    fn engineering_memory(&mut self, body: &Json) -> Result<String, Fault> {
        match read::text(body, "op")? {
            "assembly_draft" => self.engineering_assembly_draft(body),
            "preview_assembly" => self.preview_engineering_assembly(body),
            "commit_assembly" => self.commit_engineering_assembly(body),
            "capture" => self.capture_engineering_memory(body),
            "preview_transition" => self.preview_engineering_transition(body),
            "commit_transition" => self.commit_engineering_transition(body),
            "recover_transition" => self.recover_engineering_transition(body),
            _ => Err(Fault::field("op")),
        }
    }

    fn capture_engineering_memory(&mut self, body: &Json) -> Result<String, Fault> {
        read::exact_keys(body, "body", &["op", "source"])?;
        if read::one_of(body, "op", &["capture"])? != 0 {
            return Err(Fault::field("op"));
        }
        let source_body = read::map(body, "source")?;
        read::exact_keys(source_body, "source", &["kind", "result_id"])?;
        let source_kind = read::one_of(
            source_body,
            "kind",
            &["committed_design", "qualification_result"],
        )?;
        let source_result_id = match read::at(source_body, "result_id")? {
            Json::Null if source_kind == 0 => None,
            Json::Text(id) if source_kind == 1 && crate::json::is_hex(id, 64) => {
                Some(id.as_str())
            }
            _ => return Err(Fault::field("result_id")),
        };
        let lifecycle = self.lifecycle();
        if source_kind == 0 && lifecycle != "still"
            || source_kind == 1
                && lifecycle != "qualification_frozen"
                && lifecycle != "returned"
        {
            return Err(Fault::field("source"));
        }
        let run = self.run.as_ref().ok_or_else(|| state_fault(IDLE, LOADED))?;
        let state = run.state();
        if state.run_kind != crate::state::RunKind::AutomationContract {
            return Err(Fault::field("run_kind"));
        }
        let assembly = state
            .scenario
            .assembly_template()
            .filter(|template| template.is_exact())
            .ok_or_else(|| Fault::field("assembly_template"))?;
        let attempt_id = state
            .attempt
            .as_ref()
            .map(crate::state::AttemptRecord::attempt_id)
            .ok_or_else(|| Fault::field("attempt_record"))?;
        let branch_id = state
            .attempt_branch
            .as_ref()
            .map(crate::state::AttemptBranchRecord::branch_id)
            .ok_or_else(|| Fault::field("attempt_branch"))?;
        let contract_id = state
            .scenario
            .contract_id()
            .ok_or_else(|| Fault::field("contract_id"))?;
        let generator = state.scenario.generator();
        if source_kind == 1 {
            let request = state
                .qualification_request
                .as_ref()
                .ok_or_else(|| Fault::field("qualification_request"))?;
            let input = request.input();
            if read::hex(input, "attempt_id", 16)? != attempt_id
                || read::hex(input, "branch_id", 64)? != branch_id
                || read::hex(input, "assembly_template_hash", 64)? != assembly.hash()
                || read::hex(input, "generator_spec_hash", 64)?
                    != generator.specification_hash()
            {
                return Err(Fault::field("qualification_request"));
            }
        }
        let source = match source_result_id {
            Some(result_id) => {
                crate::engineering::RecordSource::result(attempt_id, branch_id, result_id)
            }
            None => crate::engineering::RecordSource::committed(attempt_id, branch_id),
        };
        let generator_record = crate::engineering::GeneratorRecordV2::new(
            generator,
            contract_id,
            state.scenario.content_hash(),
            &source,
        );
        let assembly_record = crate::engineering::AssemblyRecordV2::new(
            assembly,
            contract_id,
            state.scenario.content_hash(),
            &generator_record.generator_record_id,
            state.scenario.regime().id(),
            &source,
        );
        let mut derivation_edges = vec![crate::engineering::DerivationEdge {
            operation: crate::engineering::DerivationOperation::Capture,
            source_id: branch_id.to_string(),
            source_kind: crate::engineering::DerivationSourceKind::AttemptBranch,
        }];
        let mut evidence_links = Vec::new();
        let creation_reason = match source_result_id {
            Some(result_id) => {
                derivation_edges.push(crate::engineering::DerivationEdge {
                    operation: crate::engineering::DerivationOperation::Capture,
                    source_id: result_id.to_string(),
                    source_kind: crate::engineering::DerivationSourceKind::QualificationResult,
                });
                evidence_links.push(crate::engineering::EvidenceLink {
                    availability: crate::engineering::EvidenceAvailability::Available,
                    evidence_id: result_id.to_string(),
                    evidence_kind: crate::engineering::EvidenceKind::QualificationResult,
                    role: crate::engineering::EvidenceRole::SourceQualification,
                });
                crate::engineering::BlueprintCreationReason::ResultCapture
            }
            None => crate::engineering::BlueprintCreationReason::DesignCapture,
        };
        let blueprint = crate::engineering::BlueprintRecordV2::new(
            crate::engineering::BlueprintRecordInput {
                assembly_record_id: &assembly_record.assembly_record_id,
                contract_id,
                content_hash: state.scenario.content_hash(),
                creation_reason,
                derivation_edges: &derivation_edges,
                evidence_links: &evidence_links,
                generator_record_id: &generator_record.generator_record_id,
                parent_blueprint_id: None,
                source: &source,
            },
        );

        let mut out = String::new();
        let mut object = Obj::new(&mut out);
        object.raw("assembly_record", &assembly_record.written());
        object.raw("blueprint", &blueprint.written());
        object.raw("generator_record", &generator_record.written());
        object.text("status", "captured");
        object.int("version", 2);
        object.end();
        Ok(out)
    }

    fn engineering_assembly_draft(&self, body: &Json) -> Result<String, Fault> {
        read::exact_keys(body, "body", &["op"])?;
        if self.lifecycle() != "still" {
            return Err(Fault::field("authority"));
        }
        let run = self.run.as_ref().ok_or_else(|| state_fault(IDLE, LOADED))?;
        let state = run.state();
        if state.run_kind != crate::state::RunKind::AutomationContract {
            return Err(Fault::field("run_kind"));
        }
        let draft = state
            .scenario
            .assembly_template()
            .filter(|assembly| assembly.is_exact())
            .and_then(crate::state::AssemblyTemplate::draft)
            .ok_or_else(|| Fault::field("assembly_template"))?;
        let mut out = String::new();
        let mut object = Obj::new(&mut out);
        object.raw("assembly_draft", &draft.written());
        write_run_identity_prefix(&mut object, state);
        object.text(
            "generator_spec_hash",
            &state.scenario.generator().specification_hash(),
        );
        write_run_identity_suffix(&mut object, state);
        object.text("status", "ready");
        object.int("version", 1);
        object.end();
        Ok(out)
    }

    fn engineering_assembly_preview_parts(
        &self,
        draft: &crate::state::AssemblyDraft,
        expected_assembly_hash: &str,
        expected_attempt_id: &str,
        expected_branch_id: &str,
        expected_contract_id: &str,
        expected_generator_hash: &str,
        expected_run_kind: &str,
    ) -> Result<(
        crate::state::AssemblyTemplate,
        crate::state::AssemblyDraft,
        crate::engineering::AssemblyDiffV1,
        String,
    ), Fault> {
        if self.lifecycle() != "still" {
            return Err(Fault::field("authority"));
        }
        let run = self.run.as_ref().ok_or_else(|| state_fault(IDLE, LOADED))?;
        let state = run.state();
        let attempt_id = state
            .attempt
            .as_ref()
            .map(crate::state::AttemptRecord::attempt_id)
            .ok_or_else(|| Fault::field("attempt_record"))?;
        let current = state
            .scenario
            .assembly_template()
            .filter(|assembly| assembly.is_exact())
            .ok_or_else(|| Fault::field("assembly_template"))?;
        let branch_id = state
            .attempt_branch
            .as_ref()
            .map(crate::state::AttemptBranchRecord::branch_id)
            .ok_or_else(|| Fault::field("attempt_branch"))?;
        let generator_hash = state.scenario.generator().specification_hash();
        if state.run_kind.name() != expected_run_kind
            || state.scenario.contract_id() != Some(expected_contract_id)
            || attempt_id != expected_attempt_id
            || current.hash() != expected_assembly_hash
            || branch_id != expected_branch_id
            || generator_hash != expected_generator_hash
        {
            return Err(Fault::field("stale_engineering_base"));
        }
        let before = current
            .draft()
            .ok_or_else(|| Fault::field("assembly_template"))?;
        let candidate = run.preview_assembly_revision(draft)?;
        let candidate_draft = candidate
            .draft()
            .ok_or_else(|| Fault::field("assembly_template"))?;
        let diff = crate::engineering::AssemblyDiffV1::between(
            current.hash(),
            &before,
            candidate.hash(),
            &candidate_draft,
        );
        let mut preview_definition = String::new();
        let mut preview = Obj::new(&mut preview_definition);
        preview.text("candidate_assembly_hash", candidate.hash());
        preview.text("diff_id", &diff.diff_id);
        preview.text("expected_assembly_hash", expected_assembly_hash);
        preview.text("expected_attempt_id", expected_attempt_id);
        preview.text("expected_branch_id", expected_branch_id);
        preview.text("expected_contract_id", expected_contract_id);
        preview.text("expected_generator_hash", expected_generator_hash);
        preview.text("expected_run_kind", expected_run_kind);
        preview.int("version", 1);
        preview.end();
        let preview_id = hex_bytes(&sha256::digest(preview_definition.as_bytes()));
        Ok((candidate, candidate_draft, diff, preview_id))
    }

    fn preview_engineering_assembly(&self, body: &Json) -> Result<String, Fault> {
        read::exact_keys(
            body,
            "body",
            &[
                "draft",
                "expected_assembly_hash",
                "expected_attempt_id",
                "expected_branch_id",
                "expected_contract_id",
                "expected_generator_hash",
                "expected_run_kind",
                "op",
            ],
        )?;
        let draft = crate::state::AssemblyDraft::read(body, "draft")?;
        let expected_assembly_hash = read::hex(body, "expected_assembly_hash", 64)?;
        let expected_attempt_id = read::hex(body, "expected_attempt_id", 16)?;
        let expected_branch_id = read::hex(body, "expected_branch_id", 64)?;
        let expected_contract_id = read::text(body, "expected_contract_id")?;
        let expected_generator_hash = read::hex(body, "expected_generator_hash", 64)?;
        let expected_run_kind = read::text(body, "expected_run_kind")?;
        if expected_run_kind != "automation_contract" {
            return Err(Fault::field("expected_run_kind"));
        }
        let (candidate, candidate_draft, diff, preview_id) = self
            .engineering_assembly_preview_parts(
                &draft,
                expected_assembly_hash,
                expected_attempt_id,
                expected_branch_id,
                expected_contract_id,
                expected_generator_hash,
                expected_run_kind,
            )?;
        let state = self
            .run
            .as_ref()
            .ok_or_else(|| state_fault(IDLE, LOADED))?
            .state();
        let mut out = String::new();
        let mut object = Obj::new(&mut out);
        write_run_identity_prefix(&mut object, state);
        object.text("candidate_assembly_hash", candidate.hash());
        object.raw("candidate_assembly_template", &candidate.written());
        object.raw("candidate_draft", &candidate_draft.written());
        {
            let mut compatibility = object.object("compatibility");
            compatibility.bool("assembly_owned_only", true);
            compatibility.bool("generator_unchanged", true);
            compatibility.list("issues").end();
            compatibility.text("status", "compatible");
            compatibility.int("version", 1);
            compatibility.end();
        }
        object.raw("diff", &diff.written());
        object.text("generator_spec_hash", expected_generator_hash);
        write_run_identity_parent(&mut object, state);
        object.text("preview_id", &preview_id);
        write_run_identity_kind(&mut object, state);
        object.text("status", "accepted");
        object.int("version", 1);
        object.list("warnings").end();
        object.end();
        Ok(out)
    }

    fn commit_engineering_assembly(&mut self, body: &Json) -> Result<String, Fault> {
        read::exact_keys(
            body,
            "body",
            &[
                "draft",
                "expected_assembly_hash",
                "expected_attempt_id",
                "expected_branch_id",
                "expected_contract_id",
                "expected_generator_hash",
                "expected_preview_id",
                "expected_run_kind",
                "op",
            ],
        )?;
        let draft = crate::state::AssemblyDraft::read(body, "draft")?;
        let expected_assembly_hash = read::hex(body, "expected_assembly_hash", 64)?;
        let expected_attempt_id = read::hex(body, "expected_attempt_id", 16)?;
        let expected_branch_id = read::hex(body, "expected_branch_id", 64)?;
        let expected_contract_id = read::text(body, "expected_contract_id")?;
        let expected_generator_hash = read::hex(body, "expected_generator_hash", 64)?;
        let expected_preview_id = read::hex(body, "expected_preview_id", 64)?;
        let expected_run_kind = read::text(body, "expected_run_kind")?;
        if expected_run_kind != "automation_contract" {
            return Err(Fault::field("expected_run_kind"));
        }
        let (candidate, _candidate_draft, diff, preview_id) = self
            .engineering_assembly_preview_parts(
                &draft,
                expected_assembly_hash,
                expected_attempt_id,
                expected_branch_id,
                expected_contract_id,
                expected_generator_hash,
                expected_run_kind,
            )?;
        if preview_id != expected_preview_id {
            return Err(Fault::field("assembly_preview"));
        }
        let parent_attempt_id;
        let parent_branch_id;
        {
            let state = self
                .run
                .as_ref()
                .ok_or_else(|| state_fault(IDLE, LOADED))?
                .state();
            parent_attempt_id = state
                .attempt
                .as_ref()
                .map(crate::state::AttemptRecord::attempt_id)
                .ok_or_else(|| Fault::field("attempt_record"))?
                .to_string();
            parent_branch_id = state
                .attempt_branch
                .as_ref()
                .map(crate::state::AttemptBranchRecord::branch_id)
                .ok_or_else(|| Fault::field("attempt_branch"))?
                .to_string();
        }
        self.loaded()?.commit_assembly_revision(candidate)?;
        let state = self
            .run
            .as_ref()
            .ok_or_else(|| state_fault(IDLE, LOADED))?
            .state();
        let child_attempt_id = state
            .attempt
            .as_ref()
            .map(crate::state::AttemptRecord::attempt_id)
            .ok_or_else(|| Fault::field("attempt_record"))?;
        let child_branch_id = state
            .attempt_branch
            .as_ref()
            .map(crate::state::AttemptBranchRecord::branch_id)
            .ok_or_else(|| Fault::field("attempt_branch"))?;
        let assembly = state
            .scenario
            .assembly_template()
            .filter(|assembly| assembly.is_exact())
            .ok_or_else(|| Fault::field("assembly_template"))?;
        let contract_id = state
            .scenario
            .contract_id()
            .ok_or_else(|| Fault::field("contract_id"))?;
        let source = crate::engineering::RecordSource::committed(
            child_attempt_id,
            child_branch_id,
        );
        let generator_record = crate::engineering::GeneratorRecordV2::new(
            state.scenario.generator(),
            contract_id,
            state.scenario.content_hash(),
            &source,
        );
        let assembly_record = crate::engineering::AssemblyRecordV2::new(
            assembly,
            contract_id,
            state.scenario.content_hash(),
            &generator_record.generator_record_id,
            state.scenario.regime().id(),
            &source,
        );
        let reconstruction_digest = state.now.embodied_hash();
        let mut identities = String::new();
        {
            let mut list = crate::json::Arr::new(&mut identities);
            for (disposition, identity, kind) in [
                ("retained", child_attempt_id, "attempt"),
                ("detached", parent_branch_id.as_str(), "branch"),
                ("recreated", child_branch_id, "branch"),
                ("retained", expected_generator_hash, "generator"),
                ("detached", expected_assembly_hash, "assembly"),
                ("recreated", assembly.hash(), "assembly"),
            ] {
                let mut entry = list.object();
                entry.text("disposition", disposition);
                entry.text("identity", identity);
                entry.text("kind", kind);
                entry.int("version", 1);
                entry.end();
            }
            list.end();
        }
        let mut receipt_definition = String::new();
        {
            let mut receipt = Obj::new(&mut receipt_definition);
            receipt.text("child_attempt_id", child_attempt_id);
            receipt.text("child_branch_id", child_branch_id);
            receipt.raw("identities", &identities);
            receipt.text("operation", "assembly_commit");
            receipt.text("parent_attempt_id", &parent_attempt_id);
            receipt.text("parent_branch_id", &parent_branch_id);
            receipt.text("preview_id", &preview_id);
            receipt.text("reconstruction_digest", &reconstruction_digest);
            receipt.text("recovery_state", "accepted_unpersisted");
            receipt.int("version", 1);
            receipt.end();
        }
        let operation_id = hex_bytes(&sha256::digest(receipt_definition.as_bytes()));
        let mut transition_receipt = String::new();
        {
            let mut receipt = Obj::new(&mut transition_receipt);
            receipt.text("child_attempt_id", child_attempt_id);
            receipt.text("child_branch_id", child_branch_id);
            receipt.raw("identities", &identities);
            receipt.text("operation", "assembly_commit");
            receipt.text("operation_id", &operation_id);
            receipt.text("parent_attempt_id", &parent_attempt_id);
            receipt.text("parent_branch_id", &parent_branch_id);
            receipt.text("preview_id", &preview_id);
            receipt.text("reconstruction_digest", &reconstruction_digest);
            receipt.text("recovery_state", "accepted_unpersisted");
            receipt.int("version", 1);
            receipt.end();
        }
        let mut out = String::new();
        let mut object = Obj::new(&mut out);
        object.raw("assembly_record", &assembly_record.written());
        write_run_identity_prefix(&mut object, state);
        object.raw("diff", &diff.written());
        object.raw("generator_record", &generator_record.written());
        write_run_identity_parent(&mut object, state);
        object.text("previous_assembly_hash", expected_assembly_hash);
        object.text("previous_branch_id", &parent_branch_id);
        object.text("preview_id", &preview_id);
        write_run_identity_kind(&mut object, state);
        object.text("status", "committed");
        object.raw("transition_receipt", &transition_receipt);
        object.int("version", 1);
        object.end();
        Ok(out)
    }

    fn engineering_transition_refusal(
        operation: crate::engineering::EngineeringTransitionKind,
        code: crate::engineering::EngineeringTransitionRefusalCode,
        field: Option<&str>,
    ) -> String {
        crate::engineering::EngineeringTransitionRefusal {
            code,
            field: field.map(str::to_string),
            operation,
        }
        .written()
    }

    fn current_engineering_transition_guard(
        &self,
    ) -> Result<crate::engineering::EngineeringTransitionGuard, Fault> {
        let lifecycle = self.lifecycle();
        let run = self.run.as_ref().ok_or_else(|| state_fault(IDLE, LOADED))?;
        let state = run.state();
        let attempt = state
            .attempt
            .as_ref()
            .ok_or_else(|| Fault::field("attempt_record"))?;
        let branch = state
            .attempt_branch
            .as_ref()
            .ok_or_else(|| Fault::field("attempt_branch"))?;
        crate::engineering::EngineeringTransitionGuard::new(
            state
                .scenario
                .assembly_template_hash()
                .ok_or_else(|| Fault::field("assembly_template"))?,
            attempt.attempt_id(),
            branch.branch_id(),
            state.branch_nonce,
            state.scenario.content_hash(),
            state
                .scenario
                .contract_id()
                .ok_or_else(|| Fault::field("contract_id"))?,
            &state.now.embodied_hash(),
            &state.scenario.generator().specification_hash(),
            lifecycle,
            state.run_kind.name(),
            &state.scenario.scenario_hash(),
        )
    }

    fn transition_guard_refusal(
        expected: &crate::engineering::EngineeringTransitionGuard,
        current: &crate::engineering::EngineeringTransitionGuard,
    ) -> Option<(crate::engineering::EngineeringTransitionRefusalCode, &'static str)> {
        use crate::engineering::EngineeringTransitionRefusalCode as Refusal;
        if expected.run_kind != current.run_kind {
            Some((Refusal::WrongRunKind, "run_kind"))
        } else if expected.lifecycle != current.lifecycle {
            Some((Refusal::WrongLifecycle, "lifecycle"))
        } else if expected.contract_id != current.contract_id
            || expected.content_hash != current.content_hash
            || expected.scenario_hash != current.scenario_hash
        {
            Some((Refusal::StaleContract, "contract_id"))
        } else if expected.attempt_id != current.attempt_id {
            Some((Refusal::StaleAttempt, "attempt_id"))
        } else if expected.branch_id != current.branch_id
            || expected.branch_nonce != current.branch_nonce
        {
            Some((Refusal::StaleBranch, "branch_id"))
        } else if expected.generator_hash != current.generator_hash {
            Some((Refusal::StaleGenerator, "generator_hash"))
        } else if expected.assembly_hash != current.assembly_hash {
            Some((Refusal::StaleAssembly, "assembly_hash"))
        } else if expected.embodied_hash != current.embodied_hash {
            Some((Refusal::StalePreview, "embodied_hash"))
        } else {
            None
        }
    }

    fn preview_engineering_transition(&mut self, body: &Json) -> Result<String, Fault> {
        use crate::engineering::{
            EngineeringIdentityDisposition as Disposition, EngineeringIdentityKind as Kind,
            EngineeringTransitionIdentity as Identity, EngineeringTransitionKind as Operation,
            EngineeringTransitionRefusalCode as Refusal, EngineeringTransitionSource as Source,
        };

        let operation = Operation::read(body, "operation")?;
        match operation {
            Operation::RevertGenerator => read::exact_keys(
                body,
                "body",
                &["generator_record", "op", "operation"],
            )?,
            Operation::RestartAssembly | Operation::FullContractReset => {
                read::exact_keys(body, "body", &["op", "operation"])?
            }
        }
        if read::one_of(body, "op", &["preview_transition"])? != 0 {
            return Err(Fault::field("op"));
        }
        if self.lifecycle() != "still" {
            return Ok(Self::engineering_transition_refusal(
                operation,
                if self.lifecycle() == "qualification_frozen" {
                    Refusal::QualificationFrozen
                } else {
                    Refusal::WrongLifecycle
                },
                Some("lifecycle"),
            ));
        }
        let run_kind = self
            .run
            .as_ref()
            .ok_or_else(|| state_fault(IDLE, LOADED))?
            .state()
            .run_kind;
        if run_kind != crate::state::RunKind::AutomationContract {
            return Ok(Self::engineering_transition_refusal(
                operation,
                Refusal::WrongRunKind,
                Some("run_kind"),
            ));
        }

        let guard = self.current_engineering_transition_guard()?;
        let (
            current_scenario,
            current_assembly,
            current_view,
            authored_chapter,
            current_contract,
            chapter_index,
            current_regime_id,
        ) = {
            let content = self.content.as_ref().map_err(Fault::clone)?;
            if content.hash != guard.content_hash {
                return Ok(Self::engineering_transition_refusal(
                    operation,
                    Refusal::SourceUnavailable,
                    Some("content_hash"),
                ));
            }
            let run = self.run.as_ref().ok_or_else(|| state_fault(IDLE, LOADED))?;
            let state = run.state();
            let contract = content
                .contract(&guard.contract_id)
                .ok_or_else(|| Fault::field("contract_id"))?;
            let chapter = content
                .chapters
                .iter()
                .find(|chapter| chapter.id == contract.opening.chapter())
                .ok_or_else(|| Fault::field("opening"))?
                .clone();
            (
                state.scenario.clone(),
                state
                    .scenario
                    .assembly_template()
                    .filter(|assembly| assembly.is_exact())
                    .ok_or_else(|| Fault::field("assembly_template"))?
                    .clone(),
                state.view.clone(),
                chapter,
                contract.clone(),
                state.progress.chapter_index,
                state.scenario.regime().id().to_string(),
            )
        };

        let target = match operation {
            Operation::RestartAssembly => {
                let prepared = match self
                    .run
                    .as_ref()
                    .ok_or_else(|| state_fault(IDLE, LOADED))?
                    .preview_restart_assembly()
                {
                    Ok(field) => field,
                    Err(_) => {
                        return Ok(Self::engineering_transition_refusal(
                            operation,
                            Refusal::ReconstructionFailed,
                            Some("assembly_template"),
                        ))
                    }
                };
                (
                    Source::current_committed(),
                    current_scenario.clone(),
                    field,
                    current_view.clone(),
                    authored_chapter,
                    Vec::new(),
                    Vec::new(),
                )
            }
            Operation::RevertGenerator => {
                let generator_record = match crate::engineering::GeneratorRecordV2::read(
                    body,
                    "generator_record",
                ) {
                    Ok(record) => record,
                    Err(_) => {
                        return Ok(Self::engineering_transition_refusal(
                            operation,
                            Refusal::SourceCorrupt,
                            Some("generator_record"),
                        ))
                    }
                };
                if generator_record.contract_id != guard.contract_id
                    || generator_record.content_hash != guard.content_hash
                {
                    return Ok(Self::engineering_transition_refusal(
                        operation,
                        Refusal::SourceUnavailable,
                        Some("generator_record"),
                    ));
                }
                let target_generator = generator_record.generator_spec;
                let target_scenario = match current_scenario.with_generator(target_generator.clone()) {
                    Ok(scenario) => scenario,
                    Err(_) => {
                        return Ok(Self::engineering_transition_refusal(
                            operation,
                            Refusal::SourceCorrupt,
                            Some("generator_record"),
                        ))
                    }
                };
                let (compatibility_fields, mut compatibility_issues) =
                    compile_revert_generator_compatibility(
                        &current_contract,
                        &target_generator,
                        &current_assembly,
                        chapter_index,
                    )?;
                let fallback = current_assembly
                    .field()
                    .cloned()
                    .ok_or_else(|| Fault::field("assembly_template"))?;
                let field = if compatibility_issues.is_empty() {
                    match self
                        .run
                        .as_ref()
                        .ok_or_else(|| state_fault(IDLE, LOADED))?
                        .preview_scenario_reconstruction(&target_scenario, &current_view)
                    {
                        Ok(field) => field,
                        Err(_) => {
                            push_transition_compatibility_issue(
                                &mut compatibility_issues,
                                Some("assembly:opening".to_string()),
                                crate::engineering::EngineeringCompatibilityCode::RegimeIncompatibleAssembly,
                                crate::engineering::EngineeringCompatibilityDisposition::HardIncompatibility,
                            )?;
                            fallback
                        }
                    }
                } else {
                    fallback
                };
                (
                    Source::generator_record(&generator_record.generator_record_id)?,
                    target_scenario,
                    field,
                    current_view.clone(),
                    authored_chapter,
                    compatibility_fields,
                    compatibility_issues,
                )
            }
            Operation::FullContractReset => {
                let (view, scenario, chapter, content_hash) = {
                    let content = self.content.as_ref().map_err(Fault::clone)?;
                    let contract = content
                        .contract(&guard.contract_id)
                        .ok_or_else(|| Fault::field("contract_id"))?;
                    let chapter = content
                        .chapters
                        .iter()
                        .find(|chapter| chapter.id == contract.opening.chapter())
                        .ok_or_else(|| Fault::field("opening"))?
                        .clone();
                    let (field, view) = match contract.establish(content) {
                        Ok(opening) => opening,
                        Err(_) => {
                            return Ok(Self::engineering_transition_refusal(
                                operation,
                                Refusal::SourceUnavailable,
                                Some("opening"),
                            ))
                        }
                    };
                    let generator = crate::state::GeneratorSpec::for_field(&field).with_design(
                        0,
                        crate::policy::FrozenLocalPolicy::empty(),
                        field.route_controls.clone(),
                    )?;
                    let assembly = crate::state::AssemblyTemplate::from_field(&field);
                    let scenario = ScenarioSpec::for_contract(
                        content.hash.clone(),
                        contract.id.clone(),
                        assembly,
                        content.pressures.clone(),
                        RegimeSpec::named(contract.opening.regime())?,
                        generator,
                        Some(contract.function_criterion()?),
                    )?;
                    (view, scenario, chapter, content.hash.clone())
                };
                let field = match self
                    .run
                    .as_ref()
                    .ok_or_else(|| state_fault(IDLE, LOADED))?
                    .preview_scenario_reconstruction(&scenario, &view)
                {
                    Ok(prepared) => prepared,
                    Err(_) => {
                        return Ok(Self::engineering_transition_refusal(
                            operation,
                            Refusal::ReconstructionFailed,
                            Some("opening"),
                        ))
                    }
                };
                (
                    Source::authored_contract_opening(&content_hash)?,
                    scenario,
                    prepared,
                    view,
                    chapter,
                    Vec::new(),
                    Vec::new(),
                )
            }
        };
        let (
            source,
            target_scenario,
            target_field,
            target_view,
            authored_chapter,
            compatibility_fields,
            compatibility_issues,
        ) = target;
        let target_generator = target_scenario.generator().clone();
        let target_assembly = target_scenario
            .assembly_template()
            .filter(|assembly| assembly.is_exact())
            .ok_or_else(|| Fault::field("assembly_template"))?
            .clone();
        let target_generator_hash = target_generator.specification_hash();
        let target_assembly_hash = target_assembly.hash().to_string();
        let target_regime_id = target_scenario.regime().id().to_string();
        let target_scenario_hash = target_scenario.scenario_hash();
        let child_nonce = guard
            .branch_nonce
            .checked_add(1)
            .ok_or_else(|| Fault::field("branch_nonce"))?;
        let branch_operation = match operation {
            Operation::RestartAssembly => crate::state::BranchOperation::RestartAssembly,
            Operation::RevertGenerator => crate::state::BranchOperation::RevertGenerator,
            Operation::FullContractReset => crate::state::BranchOperation::FullContractReset,
        };
        let child_branch = crate::state::AttemptBranchRecord::new(
            guard.attempt_id.clone(),
            Some(guard.branch_id.clone()),
            branch_operation,
            target_generator_hash.clone(),
            target_assembly_hash.clone(),
            child_nonce,
        )?;
        let mut identities = vec![
            Identity::new(Disposition::Retained, Kind::Attempt, &guard.attempt_id)?,
            Identity::new(Disposition::Detached, Kind::Branch, &guard.branch_id)?,
            Identity::new(
                Disposition::Recreated,
                Kind::Branch,
                child_branch.branch_id(),
            )?,
        ];
        for (kind, before, after) in [
            (Kind::Generator, guard.generator_hash.as_str(), target_generator_hash.as_str()),
            (Kind::Assembly, guard.assembly_hash.as_str(), target_assembly_hash.as_str()),
        ] {
            if before == after {
                identities.push(Identity::new(Disposition::Retained, kind, after)?);
            } else {
                identities.push(Identity::new(Disposition::Detached, kind, before)?);
                identities.push(Identity::new(Disposition::Restored, kind, after)?);
            }
        }
        let registers = {
            let state = self
                .run
                .as_ref()
                .ok_or_else(|| state_fault(IDLE, LOADED))?
                .state();
            engineering_transition_registers(state, &target_field)?
        };
        let target_assembly_draft = target_assembly
            .draft()
            .ok_or_else(|| Fault::field("assembly_template"))?;
        let preview = crate::engineering::EngineeringTransitionPreview::new(
            operation,
            source,
            guard,
            &current_regime_id,
            &target_assembly_draft,
            &target_generator_hash,
            &target_assembly_hash,
            &target_regime_id,
            &target_scenario_hash,
            &target_field.embodied_hash(),
            identities,
            registers,
            compatibility_fields,
            compatibility_issues,
        )?;
        let written = preview.written();
        self.engineering_transition = Some(PendingEngineeringTransition {
            preview,
            target_field,
            target_scenario,
            target_view,
            authored_chapter,
        });
        let mut out = String::new();
        let mut object = Obj::new(&mut out);
        object.raw("preview", &written);
        object.text("status", "accepted");
        object.int("version", i64::from(crate::engineering::ENGINEERING_TRANSITION_VERSION));
        object.end();
        Ok(out)
    }

    fn commit_engineering_transition(&mut self, body: &Json) -> Result<String, Fault> {
        use crate::engineering::{
            EngineeringTransitionClosureReason as Closure,
            EngineeringTransitionKind as Operation,
            EngineeringTransitionReceipt as Receipt,
            EngineeringTransitionRefusalCode as Refusal,
        };

        read::exact_keys(body, "body", &["expected_guard", "op", "preview_id"])?;
        if read::one_of(body, "op", &["commit_transition"])? != 0 {
            return Err(Fault::field("op"));
        }
        let preview_id = read::hex(body, "preview_id", 64)?;
        let expected_guard = crate::engineering::EngineeringTransitionGuard::read(
            body,
            "expected_guard",
        )?;
        let Some(pending) = self.engineering_transition.as_ref() else {
            return Ok(Self::engineering_transition_refusal(
                Operation::RestartAssembly,
                Refusal::StalePreview,
                Some("preview_id"),
            ));
        };
        let operation = pending.preview.operation;
        if pending.preview.preview_id != preview_id || pending.preview.guard != expected_guard {
            return Ok(Self::engineering_transition_refusal(
                operation,
                Refusal::StalePreview,
                Some("preview_id"),
            ));
        }
        let current_guard = match self.current_engineering_transition_guard() {
            Ok(guard) => guard,
            Err(_) => {
                return Ok(Self::engineering_transition_refusal(
                    operation,
                    Refusal::WrongLifecycle,
                    Some("lifecycle"),
                ))
            }
        };
        if let Some((code, field)) = Self::transition_guard_refusal(&expected_guard, &current_guard)
        {
            return Ok(Self::engineering_transition_refusal(
                operation,
                code,
                Some(field),
            ));
        }
        if !pending.preview.commit_allowed || !pending.preview.compatibility_issues.is_empty() {
            return Ok(Self::engineering_transition_refusal(
                operation,
                Refusal::IncompatibleAssembly,
                Some("compatibility_fields"),
            ));
        }

        let pending = self
            .engineering_transition
            .take()
            .ok_or_else(|| Fault::field("transition_preview"))?;
        let before_generator_hash = current_guard.generator_hash.clone();
        let before_assembly_hash = current_guard.assembly_hash.clone();
        let parent_attempt_id = current_guard.attempt_id.clone();
        let parent_branch_id = current_guard.branch_id.clone();
        let result = match operation {
            Operation::RestartAssembly => self.loaded()?.restart_assembly(),
            Operation::RevertGenerator => self
                .loaded()?
                .revert_generator(
                    pending.target_field.clone(),
                    pending.target_view.clone(),
                    pending.target_scenario.clone(),
                ),
            Operation::FullContractReset => self.loaded()?.full_contract_reset(
                pending.target_field.clone(),
                pending.target_view.clone(),
                pending.target_scenario.clone(),
            ),
        };
        if result.is_err() {
            return Ok(Self::engineering_transition_refusal(
                operation,
                Refusal::ReconstructionFailed,
                Some("reconstruction"),
            ));
        }
        self.loaded()?.open_schedule(&pending.authored_chapter);

        let (
            child_attempt_id,
            child_branch_id,
            after_generator_hash,
            after_assembly_hash,
            after_regime_id,
            after_scenario_hash,
            reconstruction_digest,
        ) = {
            let state = self
                .run
                .as_ref()
                .ok_or_else(|| state_fault(IDLE, LOADED))?
                .state();
            (
                state
                    .attempt
                    .as_ref()
                    .map(crate::state::AttemptRecord::attempt_id)
                    .ok_or_else(|| Fault::field("attempt_record"))?
                    .to_string(),
                state
                    .attempt_branch
                    .as_ref()
                    .map(crate::state::AttemptBranchRecord::branch_id)
                    .ok_or_else(|| Fault::field("attempt_branch"))?
                    .to_string(),
                state.scenario.generator().specification_hash(),
                state
                    .scenario
                    .assembly_template_hash()
                    .ok_or_else(|| Fault::field("assembly_template"))?
                    .to_string(),
                state.scenario.regime().id().to_string(),
                state.scenario.scenario_hash(),
                state.now.embodied_hash(),
            )
        };
        let detached_evidence_ids = pending
            .preview
            .registers
            .iter()
            .map(|register| register.before_digest.clone())
            .collect();
        let receipt = Receipt::new(
            &pending.preview,
            &parent_attempt_id,
            &parent_branch_id,
            &child_attempt_id,
            &child_branch_id,
            &before_generator_hash,
            &before_assembly_hash,
            &after_generator_hash,
            &after_assembly_hash,
            &after_regime_id,
            &after_scenario_hash,
            if operation == Operation::RestartAssembly {
                Closure::Restart
            } else {
                Closure::Superseded
            },
            &reconstruction_digest,
            detached_evidence_ids,
        )?;
        self.loaded()?
            .attach_engineering_transition_receipt(receipt.clone())?;

        let (generator_record, assembly_record) = {
            let state = self
                .run
                .as_ref()
                .ok_or_else(|| state_fault(IDLE, LOADED))?
                .state();
            let source = crate::engineering::RecordSource::committed(
                &child_attempt_id,
                &child_branch_id,
            );
            let generator_record = crate::engineering::GeneratorRecordV2::new(
                state.scenario.generator(),
                &current_guard.contract_id,
                state.scenario.content_hash(),
                &source,
            );
            let assembly_record = crate::engineering::AssemblyRecordV2::new(
                state
                    .scenario
                    .assembly_template()
                    .ok_or_else(|| Fault::field("assembly_template"))?,
                &current_guard.contract_id,
                state.scenario.content_hash(),
                &generator_record.generator_record_id,
                state.scenario.regime().id(),
                &source,
            );
            (generator_record, assembly_record)
        };
        let run = self
            .run
            .as_ref()
            .ok_or_else(|| state_fault(IDLE, LOADED))?;
        let mut out = String::new();
        let mut object = Obj::new(&mut out);
        object.raw("assembly_record", &assembly_record.written());
        write_run_identity_prefix(&mut object, run.state());
        object.text("contract_id", &current_guard.contract_id);
        object.text("embodied_state_hash", &run.state().now.embodied_hash());
        object.raw("generator_record", &generator_record.written());
        object.text(
            "generator_spec_hash",
            &run.state().scenario.generator().specification_hash(),
        );
        object.raw(
            "local_policy",
            &run.state().scenario.generator().local_policy().written(),
        );
        write_run_identity_parent(&mut object, run.state());
        object.text("preview_id", &pending.preview.preview_id);
        object.text("regime", run.state().scenario.regime().id());
        object.raw(
            "route_defaults",
            &run.state()
                .scenario
                .generator()
                .route_defaults_written(run.state().progress.chapter_index),
        );
        write_run_identity_kind(&mut object, run.state());
        object.text("scenario_hash", &run.state().scenario.scenario_hash());
        object.text("status", "committed");
        object.raw("transition_receipt", &receipt.written());
        object.int("version", i64::from(crate::engineering::ENGINEERING_TRANSITION_VERSION));
        object.raw("view", &run.state().view.written());
        object.end();
        Ok(out)
    }

    fn recover_engineering_transition(&mut self, body: &Json) -> Result<String, Fault> {
        read::exact_keys(body, "body", &["op", "operation_id"])?;
        if read::one_of(body, "op", &["recover_transition"])? != 0 {
            return Err(Fault::field("op"));
        }
        let operation_id = read::hex(body, "operation_id", 64)?;
        let run = self
            .run
            .as_ref()
            .ok_or_else(|| state_fault(IDLE, LOADED))?;
        let receipt = run
            .state()
            .attempt_branch
            .as_ref()
            .and_then(crate::state::AttemptBranchRecord::transition_receipt);
        let Some(receipt) = receipt.filter(|receipt| receipt.operation_id == operation_id) else {
            let operation = receipt.map_or(
                crate::engineering::EngineeringTransitionKind::RestartAssembly,
                |receipt| receipt.operation,
            );
            return Ok(Self::engineering_transition_refusal(
                operation,
                crate::engineering::EngineeringTransitionRefusalCode::SourceUnavailable,
                Some("operation_id"),
            ));
        };
        let mut out = String::new();
        let mut object = Obj::new(&mut out);
        write_run_identity_prefix(&mut object, run.state());
        match run.state().scenario.contract_id() {
            Some(id) => object.text("contract_id", id),
            None => object.null("contract_id"),
        };
        object.text("embodied_state_hash", &run.state().now.embodied_hash());
        object.text(
            "generator_spec_hash",
            &run.state().scenario.generator().specification_hash(),
        );
        object.raw(
            "local_policy",
            &run.state().scenario.generator().local_policy().written(),
        );
        write_run_identity_parent(&mut object, run.state());
        object.text("regime", run.state().scenario.regime().id());
        object.raw(
            "route_defaults",
            &run.state()
                .scenario
                .generator()
                .route_defaults_written(run.state().progress.chapter_index),
        );
        write_run_identity_kind(&mut object, run.state());
        object.text("scenario_hash", &run.state().scenario.scenario_hash());
        object.text("status", "recovered");
        object.raw("transition_receipt", &receipt.written());
        object.int("version", i64::from(receipt.version));
        object.raw("view", &run.state().view.written());
        object.end();
        Ok(out)
    }

    fn qualification_job(&mut self, body: &Json) -> Result<String, Fault> {
        let operation = read::one_of(
            body,
            "op",
            &["prepare", "resolve", "trial", "grade", "trace", "result", "receipt"],
        )?;
        let (request_id, input, scenario, form, assembly) = {
            let run = self
                .run
                .as_ref()
                .ok_or_else(|| state_fault(IDLE, &["qualification_frozen"]))?;
            let request = run
                .state()
                .qualification_request
                .as_ref()
                .ok_or_else(|| Fault::field("qualification_request"))?;
            let request_id = read::hex(body, "request_id", 64)?;
            if request.request_id() != request_id {
                return Err(Fault::field("qualification_request"));
            }
            let assembly = run
                .state()
                .scenario
                .assembly_template()
                .and_then(|template| template.field())
                .cloned()
                .ok_or_else(|| Fault::field("assembly_template"))?;
            (
                request_id.to_string(),
                request.input().clone(),
                run.state().scenario.clone(),
                run.form().to_string(),
                assembly,
            )
        };
        let procedure = read::map(&input, "procedure")?;
        let trial_count = read::int(procedure, "trial_count", 1, 64)? as u16;
        let duration_steps = read::int(
            read::map(procedure, "schedule")?,
            "duration_steps",
            1,
            i64::from(u16::MAX),
        )? as u32;
        let progress_interval_steps = read::int(
            procedure,
            "progress_interval_steps",
            1,
            i64::from(u16::MAX),
        )? as u32;

        let mut job_definition = String::new();
        {
            let mut job = Obj::new(&mut job_definition);
            job.text("request_id", &request_id);
            job.int("runner_version", 3);
            job.end();
        }
        let job_id = hex_bytes(&sha256::digest(job_definition.as_bytes()));

        if operation == 0 {
            read::exact_keys(body, "body", &["op", "request_id"])?;
            let mut out = String::new();
            let mut object = Obj::new(&mut out);
            object.int("duration_steps", i64::from(duration_steps));
            object.text("job_id", &job_id);
            object.int("progress_interval_steps", i64::from(progress_interval_steps));
            object.text("request_id", &request_id);
            object.text("status", "queued");
            object.int("trial_count", i64::from(trial_count));
            object.int("version", 3);
            object.end();
            return Ok(out);
        }

        if operation == 1 {
            return self.qualification_resolution(
                body,
                &request_id,
                &job_id,
                trial_count,
                duration_steps,
                &scenario,
            );
        }

        if operation == 3 {
            return self.qualification_grades(
                body,
                &request_id,
                &job_id,
                trial_count,
                duration_steps,
                &scenario,
            );
        }

        if operation == 4 {
            return self.qualification_failure_trace(
                body,
                &request_id,
                &job_id,
                trial_count,
                duration_steps,
                &scenario,
            );
        }

        if operation == 5 {
            return self.qualification_result(
                body,
                &request_id,
                &job_id,
                trial_count,
                duration_steps,
                &input,
                &scenario,
            );
        }

        if operation == 6 {
            return self.qualification_receipt(
                body,
                &request_id,
                &job_id,
                trial_count,
                duration_steps,
                &input,
                &scenario,
            );
        }

        read::exact_keys(body, "body", &["job_id", "op", "request_id", "trial"])?;
        if read::hex(body, "job_id", 64)? != job_id {
            return Err(Fault::field("qualification_job"));
        }
        let trial = read::int(body, "trial", 0, i64::from(trial_count - 1))? as u16;
        let mut trial_definition = String::new();
        {
            let mut definition = Obj::new(&mut trial_definition);
            definition.text("request_id", &request_id);
            definition.int("trial", i64::from(trial));
            definition.int("version", 3);
            definition.end();
        }
        let trial_hash = hex_bytes(&sha256::digest(trial_definition.as_bytes()));
        let trial_run_id = &trial_hash[..16];
        let mut trial_run = Run::start_with_scenario_kind(
            trial_run_id,
            &form,
            scenario,
            crate::state::RunKind::AutomationContract,
        )?;
        trial_run.establish_field(assembly, crate::state::ViewDeclaration::opening())?;
        let initial_material_units: i64 = trial_run
            .state()
            .now
            .materials
            .iter()
            .map(|material| i64::from(material.amount))
            .sum();
        let material_units = |field: &crate::state::FieldState, kind: crate::field::MaterialKind| {
            field
                .materials
                .iter()
                .filter(|material| material.kind == kind)
                .map(|material| i64::from(material.amount))
                .sum::<i64>()
        };
        let initial_materials = [
            (
                crate::field::MaterialKind::BoundaryBlank,
                material_units(&trial_run.state().now, crate::field::MaterialKind::BoundaryBlank),
            ),
            (
                crate::field::MaterialKind::Conductor,
                material_units(&trial_run.state().now, crate::field::MaterialKind::Conductor),
            ),
            (
                crate::field::MaterialKind::JunctionBlank,
                material_units(&trial_run.state().now, crate::field::MaterialKind::JunctionBlank),
            ),
        ];
        let mut total_drain = 0_i64;
        let mut total_leakage = 0_i64;
        let mut total_moved = 0_i64;
        let mut total_overload = 0_i64;
        let mut total_renewal = 0_i64;
        let mut total_supply = 0_i64;
        let mut total_upkeep = 0_i64;
        let mut first_failure_payload: Option<String> = None;
        let mut first_failure_step: Option<u32> = None;
        let mut recent_events = std::collections::VecDeque::new();
        let mut recent_events_truncated = false;
        let mut first_failure_events = Vec::new();
        let mut first_failure_events_truncated = false;
        for _ in 0..duration_steps {
            let ledger = trial_run.analysis_step();
            for event in trial_run.take_events() {
                recent_events.push_back(event);
                if recent_events.len() > 192 {
                    recent_events.pop_front();
                    recent_events_truncated = true;
                }
            }
            for (total, value) in [
                (&mut total_drain, ledger.drain),
                (&mut total_leakage, ledger.leakage),
                (&mut total_moved, ledger.moved),
                (&mut total_overload, ledger.overload),
                (&mut total_renewal, ledger.renewal),
                (&mut total_supply, ledger.current),
                (&mut total_upkeep, ledger.upkeep),
            ] {
                *total = total
                    .checked_add(value)
                    .filter(|sum| *sum <= crate::json::MAX_SAFE_INT)
                    .ok_or_else(|| Fault::field("qualification_grade_evidence"))?;
            }
            if first_failure_payload.is_none()
                && trial_run
                    .state()
                    .criterion
                    .as_ref()
                    .is_some_and(|runtime| {
                        runtime.status() == crate::criterion::CriterionStatus::Failed
                    })
            {
                first_failure_step = Some(trial_run.state().now.step);
                first_failure_payload = Some(trial_run.payload()?);
                first_failure_events = recent_events.iter().cloned().collect();
                first_failure_events_truncated = recent_events_truncated;
            }
        }
        let terminal_events: Vec<String> = recent_events.into_iter().collect();
        let final_material_units: i64 = trial_run
            .state()
            .now
            .materials
            .iter()
            .map(|material| i64::from(material.amount))
            .sum();
        let mut grade_evidence = String::new();
        {
            let mut evidence = Obj::new(&mut grade_evidence);
            evidence.int("drain", total_drain);
            evidence.int("final_material_units", final_material_units);
            evidence.int("initial_material_units", initial_material_units);
            evidence.int("interventions", 0);
            evidence.int("leakage", total_leakage);
            {
                let mut materials = evidence.list("materials");
                for (kind, initial) in initial_materials {
                    let mut material = materials.object();
                    material.int("final", material_units(&trial_run.state().now, kind));
                    material.int("initial", initial);
                    material.text("kind", kind.name());
                    material.end();
                }
                materials.end();
            }
            evidence.int("moved", total_moved);
            evidence.int("overload", total_overload);
            evidence.int("renewal", total_renewal);
            evidence.int("supply", total_supply);
            evidence.int("upkeep", total_upkeep);
            evidence.int("version", 1);
            evidence.end();
        }
        let terminal_payload = trial_run.payload()?;
        let terminal_payload_hash = hex_bytes(&sha256::digest(terminal_payload.as_bytes()));
        let first_failure_payload_hash = first_failure_payload
            .as_ref()
            .map(|payload| hex_bytes(&sha256::digest(payload.as_bytes())));
        let terminal_embodied_state_hash = trial_run.state().now.embodied_hash();
        let criterion_written = trial_run
            .state()
            .criterion
            .as_ref()
            .map(|runtime| runtime.written());

        let mut artifact_definition = String::new();
        {
            let mut artifact = Obj::new(&mut artifact_definition);
            match &criterion_written {
                Some(criterion) => artifact.raw("criterion_runtime", criterion),
                None => artifact.null("criterion_runtime"),
            };
            artifact.int("duration_steps", i64::from(duration_steps));
            artifact.int("executed_steps", i64::from(duration_steps));
            {
                let mut events = artifact.list("first_failure_events");
                for event in &first_failure_events {
                    events.raw(event);
                }
                events.end();
            }
            artifact.bool(
                "first_failure_events_truncated",
                first_failure_events_truncated,
            );
            match &first_failure_payload {
                Some(payload) => artifact.text("first_failure_payload", payload),
                None => artifact.null("first_failure_payload"),
            };
            match &first_failure_payload_hash {
                Some(hash) => artifact.text("first_failure_payload_hash", hash),
                None => artifact.null("first_failure_payload_hash"),
            };
            artifact.int_or_null("first_failure_step", first_failure_step.map(i64::from));
            artifact.raw("grade_evidence", &grade_evidence);
            artifact.text("job_id", &job_id);
            artifact.text("request_id", &request_id);
            artifact.text("terminal_embodied_state_hash", &terminal_embodied_state_hash);
            {
                let mut events = artifact.list("terminal_events");
                for event in &terminal_events {
                    events.raw(event);
                }
                events.end();
            }
            artifact.bool("terminal_events_truncated", recent_events_truncated);
            artifact.text("terminal_payload_hash", &terminal_payload_hash);
            artifact.int("trial", i64::from(trial));
            artifact.int("version", 3);
            artifact.end();
        }
        let artifact_id = hex_bytes(&sha256::digest(artifact_definition.as_bytes()));

        let mut out = String::new();
        let mut object = Obj::new(&mut out);
        object.text("artifact_id", &artifact_id);
        match &criterion_written {
            Some(criterion) => object.raw("criterion_runtime", criterion),
            None => object.null("criterion_runtime"),
        };
        object.int("duration_steps", i64::from(duration_steps));
        object.int("executed_steps", i64::from(duration_steps));
        {
            let mut events = object.list("first_failure_events");
            for event in &first_failure_events {
                events.raw(event);
            }
            events.end();
        }
        object.bool(
            "first_failure_events_truncated",
            first_failure_events_truncated,
        );
        match &first_failure_payload {
            Some(payload) => object.text("first_failure_payload", payload),
            None => object.null("first_failure_payload"),
        };
        match &first_failure_payload_hash {
            Some(hash) => object.text("first_failure_payload_hash", hash),
            None => object.null("first_failure_payload_hash"),
        };
        object.int_or_null("first_failure_step", first_failure_step.map(i64::from));
        object.raw("grade_evidence", &grade_evidence);
        object.text("job_id", &job_id);
        object.text("request_id", &request_id);
        object.text("status", "completed");
        object.text("terminal_embodied_state_hash", &terminal_embodied_state_hash);
        {
            let mut events = object.list("terminal_events");
            for event in &terminal_events {
                events.raw(event);
            }
            events.end();
        }
        object.bool("terminal_events_truncated", recent_events_truncated);
        object.text("terminal_payload", &terminal_payload);
        object.text("terminal_payload_hash", &terminal_payload_hash);
        object.int("trial", i64::from(trial));
        object.int("version", 3);
        object.end();
        Ok(out)
    }

    fn qualification_receipt(
        &self,
        body: &Json,
        request_id: &str,
        job_id: &str,
        trial_count: u16,
        duration_steps: u32,
        request_input: &Json,
        frozen_scenario: &ScenarioSpec,
    ) -> Result<String, Fault> {
        read::exact_keys(
            body,
            "body",
            &[
                "artifacts",
                "failure_trace_id",
                "function_decision_id",
                "grade_ids",
                "job_id",
                "marker_id",
                "op",
                "request_id",
                "result_id",
            ],
        )?;
        let submitted_result_id = read::hex(body, "result_id", 64)?;
        let submitted_marker_id = read::hex(body, "marker_id", 64)?;
        let mut result_body_written = String::new();
        {
            let mut rebuilt = Obj::new(&mut result_body_written);
            {
                let mut artifacts = rebuilt.list("artifacts");
                for artifact in read::list(body, "artifacts", 64)? {
                    let mut written = String::new();
                    crate::json::write_value(&mut written, artifact)
                        .map_err(|_| Fault::field("qualification_artifacts"))?;
                    artifacts.raw(&written);
                }
                artifacts.end();
            }
            match read::at(body, "failure_trace_id")? {
                Json::Null => rebuilt.null("failure_trace_id"),
                Json::Text(id) if crate::json::is_hex(id, 64) => {
                    rebuilt.text("failure_trace_id", id)
                }
                _ => return Err(Fault::field("failure_trace_id")),
            };
            rebuilt.text(
                "function_decision_id",
                read::hex(body, "function_decision_id", 64)?,
            );
            {
                let mut ids = rebuilt.list("grade_ids");
                let supplied = read::list(body, "grade_ids", 4)?;
                if supplied.len() != 4 {
                    return Err(Fault::field("grade_ids"));
                }
                for id in supplied {
                    match id {
                        Json::Text(id) if crate::json::is_hex(id, 64) => ids.text(id),
                        _ => return Err(Fault::field("grade_ids")),
                    }
                }
                ids.end();
            }
            rebuilt.text("job_id", job_id);
            rebuilt.text("op", "result");
            rebuilt.text("request_id", request_id);
            rebuilt.end();
        }
        let result_body = parse(&result_body_written)
            .map_err(|_| Fault::field("qualification_receipt"))?;
        let group = self.qualification_result(
            &result_body,
            request_id,
            job_id,
            trial_count,
            duration_steps,
            request_input,
            frozen_scenario,
        )?;
        let group = parse(&group).map_err(|_| Fault::field("qualification_result"))?;
        let result = read::map(&group, "result")?;
        let marker = read::map(&group, "complete_marker")?;
        if read::hex(result, "result_id", 64)? != submitted_result_id
            || read::hex(marker, "marker_id", 64)? != submitted_marker_id
        {
            return Err(Fault::field("qualification_result"));
        }
        let result_definition = read::map(result, "definition")?;
        if read::one_of(result_definition, "outcome", &["passed"])? != 0 {
            return Err(Fault::field("qualification_result"));
        }
        let contract_id = read::text(result_definition, "contract_id")?;
        let content = self.content.as_ref().map_err(Fault::clone)?;
        if content.hash != read::text(result_definition, "content_hash")? {
            return Err(Fault::field("content_hash"));
        }
        let contract = content
            .contract(contract_id)
            .ok_or_else(|| Fault::field("contract_id"))?;
        let receipt_definition = Self::unlock_receipt_definition(
            contract,
            &content.hash,
            submitted_result_id,
        );
        let receipt_id = hex_bytes(&sha256::digest(receipt_definition.as_bytes()));
        let mut receipt_written = String::new();
        {
            let mut receipt = Obj::new(&mut receipt_written);
            receipt.raw("definition", &receipt_definition);
            receipt.text("receipt_id", &receipt_id);
            receipt.end();
        }
        let mut out = String::new();
        let mut object = Obj::new(&mut out);
        object.raw("receipt", &receipt_written);
        object.text("status", "derived");
        object.int("version", 1);
        object.end();
        Ok(out)
    }

    fn qualification_result(
        &self,
        body: &Json,
        request_id: &str,
        job_id: &str,
        trial_count: u16,
        duration_steps: u32,
        request_input: &Json,
        frozen_scenario: &ScenarioSpec,
    ) -> Result<String, Fault> {
        read::exact_keys(
            body,
            "body",
            &[
                "artifacts",
                "failure_trace_id",
                "function_decision_id",
                "grade_ids",
                "job_id",
                "op",
                "request_id",
            ],
        )?;
        if read::hex(body, "job_id", 64)? != job_id {
            return Err(Fault::field("qualification_job"));
        }
        let function_decision_id = read::hex(body, "function_decision_id", 64)?;
        let artifacts = read::list(body, "artifacts", 64)?;
        let submitted_grade_ids = read::list(body, "grade_ids", 4)?;
        if submitted_grade_ids.len() != 4 {
            return Err(Fault::field("grade_ids"));
        }
        let submitted_grade_ids = submitted_grade_ids
            .iter()
            .map(|value| {
                value
                    .as_text()
                    .filter(|id| crate::json::is_hex(id, 64))
                    .map(str::to_string)
                    .ok_or_else(|| Fault::field("grade_ids"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let submitted_trace_id = match read::at(body, "failure_trace_id")? {
            Json::Null => None,
            Json::Text(id) if crate::json::is_hex(id, 64) => Some(id.to_string()),
            _ => return Err(Fault::field("failure_trace_id")),
        };

        let build_body = |operation: &str| -> Result<Json, Fault> {
            let mut written = String::new();
            {
                let mut rebuilt = Obj::new(&mut written);
                {
                    let mut held = rebuilt.list("artifacts");
                    for artifact in artifacts {
                        let mut artifact_written = String::new();
                        crate::json::write_value(&mut artifact_written, artifact)
                            .map_err(|_| Fault::field("qualification_artifacts"))?;
                        held.raw(&artifact_written);
                    }
                    held.end();
                }
                rebuilt.text("function_decision_id", function_decision_id);
                rebuilt.text("job_id", job_id);
                rebuilt.text("op", operation);
                rebuilt.text("request_id", request_id);
                rebuilt.end();
            }
            parse(&written).map_err(|_| Fault::field("qualification_result"))
        };
        let grade_body = build_body("grade")?;
        let graded = self.qualification_grades(
            &grade_body,
            request_id,
            job_id,
            trial_count,
            duration_steps,
            frozen_scenario,
        )?;
        let graded = parse(&graded).map_err(|_| Fault::field("qualification_grades"))?;
        let grades = read::list(&graded, "grades", 4)?;
        let grade_ids = grades
            .iter()
            .map(|grade| read::hex(grade, "grade_id", 64).map(str::to_string))
            .collect::<Result<Vec<_>, _>>()?;
        if grade_ids != submitted_grade_ids {
            return Err(Fault::field("grade_ids"));
        }

        let trace_body = build_body("trace")?;
        let traced = self.qualification_failure_trace(
            &trace_body,
            request_id,
            job_id,
            trial_count,
            duration_steps,
            frozen_scenario,
        )?;
        let traced = parse(&traced).map_err(|_| Fault::field("qualification_failure_trace"))?;

        let mut resolution_body_written = String::new();
        {
            let mut rebuilt = Obj::new(&mut resolution_body_written);
            {
                let mut held = rebuilt.list("artifacts");
                for artifact in artifacts {
                    let mut artifact_written = String::new();
                    crate::json::write_value(&mut artifact_written, artifact)
                        .map_err(|_| Fault::field("qualification_artifacts"))?;
                    held.raw(&artifact_written);
                }
                held.end();
            }
            rebuilt.text("job_id", job_id);
            rebuilt.text("op", "resolve");
            rebuilt.text("request_id", request_id);
            rebuilt.end();
        }
        let resolution_body = parse(&resolution_body_written)
            .map_err(|_| Fault::field("qualification_resolution"))?;
        let resolved = self.qualification_resolution(
            &resolution_body,
            request_id,
            job_id,
            trial_count,
            duration_steps,
            frozen_scenario,
        )?;
        let resolved = parse(&resolved).map_err(|_| Fault::field("qualification_resolution"))?;
        let function = read::map(&resolved, "function_decision")?;
        if read::hex(function, "function_decision_id", 64)? != function_decision_id {
            return Err(Fault::field("function_decision_id"));
        }
        let function_definition = read::map(function, "definition")?;
        let passed = read::flag(function_definition, "passed")?;
        let failure_trace_id = match (passed, read::at(&traced, "failure_trace")?) {
            (true, Json::Null) if submitted_trace_id.is_none() => None,
            (false, Json::Map(_)) => {
                let trace = read::map(&traced, "failure_trace")?;
                let trace_id = read::hex(trace, "failure_trace_id", 64)?.to_string();
                if submitted_trace_id.as_deref() != Some(trace_id.as_str()) {
                    return Err(Fault::field("failure_trace_id"));
                }
                Some(trace_id)
            }
            _ => return Err(Fault::field("failure_trace_id")),
        };
        let criterion_decisions = read::list(&resolved, "criterion_decisions", 4_096)?;
        let criterion_decision_ids = criterion_decisions
            .iter()
            .map(|decision| read::hex(decision, "decision_id", 64).map(str::to_string))
            .collect::<Result<Vec<_>, _>>()?;
        let artifact_ids = artifacts
            .iter()
            .map(|artifact| read::hex(artifact, "artifact_id", 64).map(str::to_string))
            .collect::<Result<Vec<_>, _>>()?;
        let assembly_hash = frozen_scenario
            .assembly_template_hash()
            .ok_or_else(|| Fault::field("assembly_template"))?;
        let contract_id = frozen_scenario
            .contract_id()
            .ok_or_else(|| Fault::field("contract_id"))?;
        let mut build_written = String::new();
        crate::json::write_value(
            &mut build_written,
            read::map(request_input, "build")?,
        )
        .map_err(|_| Fault::field("qualification_request"))?;
        let mut result_definition = String::new();
        {
            let mut result = Obj::new(&mut result_definition);
            {
                let mut ids = result.list("artifact_ids");
                for id in &artifact_ids {
                    ids.text(id);
                }
                ids.end();
            }
            result.text("assembly_template_hash", assembly_hash);
            result.raw("build", &build_written);
            result.text("content_hash", frozen_scenario.content_hash());
            result.text("contract_id", contract_id);
            {
                let mut ids = result.list("criterion_decision_ids");
                for id in &criterion_decision_ids {
                    ids.text(id);
                }
                ids.end();
            }
            result.text("execution_status", "completed");
            match &failure_trace_id {
                Some(id) => result.text("failure_trace_id", id),
                None => result.null("failure_trace_id"),
            };
            result.text("function_decision_id", function_decision_id);
            result.text(
                "generator_spec_hash",
                &frozen_scenario.generator().specification_hash(),
            );
            {
                let mut ids = result.list("grade_ids");
                for id in &grade_ids {
                    ids.text(id);
                }
                ids.end();
            }
            result.text("job_id", job_id);
            result.text("outcome", if passed { "passed" } else { "failed" });
            result.int("protocol_version", i64::from(PROTOCOL_VERSION));
            result.text("request_id", request_id);
            result.text("scenario_hash", &frozen_scenario.scenario_hash());
            result.int("trial_count", i64::from(trial_count));
            result.int("version", 1);
            result.end();
        }
        let result_id = hex_bytes(&sha256::digest(result_definition.as_bytes()));
        let mut result_written = String::new();
        {
            let mut result = Obj::new(&mut result_written);
            result.raw("definition", &result_definition);
            result.text("result_id", &result_id);
            result.end();
        }
        let child_count = artifact_ids.len()
            + criterion_decision_ids.len()
            + grade_ids.len()
            + 3
            + usize::from(failure_trace_id.is_some());
        let mut marker_definition = String::new();
        {
            let mut marker = Obj::new(&mut marker_definition);
            marker.int("child_count", child_count as i64);
            marker.text("result_id", &result_id);
            marker.int("version", 1);
            marker.end();
        }
        let marker_id = hex_bytes(&sha256::digest(marker_definition.as_bytes()));
        let mut marker_written = String::new();
        {
            let mut marker = Obj::new(&mut marker_written);
            marker.raw("definition", &marker_definition);
            marker.text("marker_id", &marker_id);
            marker.end();
        }
        let mut out = String::new();
        let mut object = Obj::new(&mut out);
        object.raw("complete_marker", &marker_written);
        object.raw("result", &result_written);
        object.text("status", "complete");
        object.int("version", 1);
        object.end();
        Ok(out)
    }

    fn qualification_failure_trace(
        &self,
        body: &Json,
        request_id: &str,
        job_id: &str,
        trial_count: u16,
        duration_steps: u32,
        frozen_scenario: &ScenarioSpec,
    ) -> Result<String, Fault> {
        read::exact_keys(
            body,
            "body",
            &["artifacts", "function_decision_id", "job_id", "op", "request_id"],
        )?;
        if read::hex(body, "job_id", 64)? != job_id {
            return Err(Fault::field("qualification_job"));
        }
        let artifacts = read::list(body, "artifacts", 64)?;
        let mut resolution_body = String::new();
        {
            let mut rebuilt = Obj::new(&mut resolution_body);
            {
                let mut held = rebuilt.list("artifacts");
                for artifact in artifacts {
                    let mut written = String::new();
                    crate::json::write_value(&mut written, artifact)
                        .map_err(|_| Fault::field("qualification_artifacts"))?;
                    held.raw(&written);
                }
                held.end();
            }
            rebuilt.text("job_id", job_id);
            rebuilt.text("op", "resolve");
            rebuilt.text("request_id", request_id);
            rebuilt.end();
        }
        let resolution_body = parse(&resolution_body)
            .map_err(|_| Fault::field("qualification_artifacts"))?;
        let resolved = self.qualification_resolution(
            &resolution_body,
            request_id,
            job_id,
            trial_count,
            duration_steps,
            frozen_scenario,
        )?;
        let resolved = parse(&resolved).map_err(|_| Fault::field("qualification_resolution"))?;
        let function = read::map(&resolved, "function_decision")?;
        let function_decision_id = read::hex(function, "function_decision_id", 64)?;
        if read::hex(body, "function_decision_id", 64)? != function_decision_id {
            return Err(Fault::field("function_decision_id"));
        }
        let function_definition = read::map(function, "definition")?;
        if read::flag(function_definition, "passed")? {
            let mut out = String::new();
            let mut object = Obj::new(&mut out);
            object.null("failure_trace");
            object.text("job_id", job_id);
            object.text("request_id", request_id);
            object.text("status", "not_applicable");
            object.int("version", 1);
            object.end();
            return Ok(out);
        }

        let decisions = read::list(&resolved, "criterion_decisions", 4_096)?;
        let mut first: Option<(u32, u16, usize, &Json)> = None;
        for (position, decision) in decisions.iter().enumerate() {
            let definition = read::map(decision, "definition")?;
            if read::flag(definition, "passed")? {
                continue;
            }
            let candidate = (
                read::int(definition, "resolution_step", 1, i64::from(duration_steps))? as u32,
                read::int(definition, "trial", 0, i64::from(trial_count - 1))? as u16,
                position,
                decision,
            );
            if first.as_ref().map_or(true, |held| {
                (candidate.0, candidate.1, candidate.2) < (held.0, held.1, held.2)
            }) {
                first = Some(candidate);
            }
        }
        let (resolution_step, trial, _, decision) =
            first.ok_or_else(|| Fault::field("failure_trace"))?;
        let definition = read::map(decision, "definition")?;
        let artifact = artifacts
            .get(usize::from(trial))
            .ok_or_else(|| Fault::field("qualification_artifacts"))?;
        let first_failure_step = read::int_or_null(
            artifact,
            "first_failure_step",
            1,
            i64::from(duration_steps),
        )?
        .map(|step| step as u32);
        let (payload, payload_hash, event_key, events_truncated) = if first_failure_step
            == Some(resolution_step)
        {
            (
                read::text(artifact, "first_failure_payload")?,
                read::hex(artifact, "first_failure_payload_hash", 64)?,
                "first_failure_events",
                read::flag(artifact, "first_failure_events_truncated")?,
            )
        } else {
            (
                read::text(artifact, "terminal_payload")?,
                read::hex(artifact, "terminal_payload_hash", 64)?,
                "terminal_events",
                read::flag(artifact, "terminal_events_truncated")?,
            )
        };
        if hex_bytes(&sha256::digest(payload.as_bytes())) != payload_hash {
            return Err(Fault::field("failure_trace"));
        }
        let payload = parse(payload).map_err(|_| Fault::field("failure_trace"))?;
        let state = RunState::read(&payload)?;
        state.coherent()?;
        if state.now.step != resolution_step {
            return Err(Fault::field("failure_trace"));
        }
        let window_start_step = read::int(
            definition,
            "window_start_step",
            1,
            i64::from(resolution_step),
        )? as u32;
        let last_trace_step = state.trace.steps.back().map(|step| step.step);
        let trace_status = if state.trace.start_step <= window_start_step.saturating_sub(1)
            && last_trace_step == Some(resolution_step)
            && !events_truncated
        {
            "complete"
        } else {
            "incomplete"
        };
        let mut source = String::new();
        crate::json::write_value(&mut source, read::map(definition, "source")?)
            .map_err(|_| Fault::field("source"))?;
        let mut trace_definition = String::new();
        {
            let mut trace = Obj::new(&mut trace_definition);
            trace.text("artifact_id", read::hex(artifact, "artifact_id", 64)?);
            trace.text("criterion_decision_id", read::hex(decision, "decision_id", 64)?);
            trace.text("function_decision_id", function_decision_id);
            trace.text("inference_algorithm", "direct_records_only_v1");
            {
                let contributors = trace.list("inferred_contributors");
                contributors.end();
            }
            trace.text("job_id", job_id);
            {
                let mut events = trace.list("mechanism_events");
                for event in read::list(artifact, event_key, 192)? {
                    let event_step = read::int(event, "step", 0, i64::from(resolution_step))?;
                    if event_step >= i64::from(window_start_step) {
                        let mut written = String::new();
                        crate::json::write_value(&mut written, event)
                            .map_err(|_| Fault::field("mechanism_events"))?;
                        events.raw(&written);
                    }
                }
                events.end();
            }
            trace.text("payload_hash", payload_hash);
            trace.text("request_id", request_id);
            trace.int("resolution_step", i64::from(resolution_step));
            trace.raw("source", &source);
            trace.text("status", trace_status);
            trace.text("trace_keyframe_hash", &state.trace.keyframe.embodied_hash());
            trace.int("trace_start_step", i64::from(state.trace.start_step));
            {
                let mut steps = trace.list("trace_steps");
                for step in &state.trace.steps {
                    if step.step <= resolution_step {
                        steps.raw(&step.written());
                    }
                }
                steps.end();
            }
            trace.int("trial", i64::from(trial));
            trace.int("version", 1);
            trace.int("window_start_step", i64::from(window_start_step));
            trace.end();
        }
        let failure_trace_id = hex_bytes(&sha256::digest(trace_definition.as_bytes()));
        let mut failure_trace = String::new();
        {
            let mut trace = Obj::new(&mut failure_trace);
            trace.raw("definition", &trace_definition);
            trace.text("failure_trace_id", &failure_trace_id);
            trace.end();
        }
        let mut out = String::new();
        let mut object = Obj::new(&mut out);
        object.raw("failure_trace", &failure_trace);
        object.text("job_id", job_id);
        object.text("request_id", request_id);
        object.text("status", "traced");
        object.int("version", 1);
        object.end();
        Ok(out)
    }

    fn qualification_grades(
        &self,
        body: &Json,
        request_id: &str,
        job_id: &str,
        trial_count: u16,
        duration_steps: u32,
        frozen_scenario: &ScenarioSpec,
    ) -> Result<String, Fault> {
        read::exact_keys(
            body,
            "body",
            &["artifacts", "function_decision_id", "job_id", "op", "request_id"],
        )?;
        if read::hex(body, "job_id", 64)? != job_id {
            return Err(Fault::field("qualification_job"));
        }
        let artifacts = read::list(body, "artifacts", 64)?;
        let mut resolution_body = String::new();
        {
            let mut rebuilt = Obj::new(&mut resolution_body);
            {
                let mut held = rebuilt.list("artifacts");
                for artifact in artifacts {
                    let mut written = String::new();
                    crate::json::write_value(&mut written, artifact)
                        .map_err(|_| Fault::field("qualification_artifacts"))?;
                    held.raw(&written);
                }
                held.end();
            }
            rebuilt.text("job_id", job_id);
            rebuilt.text("op", "resolve");
            rebuilt.text("request_id", request_id);
            rebuilt.end();
        }
        let resolution_body = parse(&resolution_body)
            .map_err(|_| Fault::field("qualification_artifacts"))?;
        let resolved = self.qualification_resolution(
            &resolution_body,
            request_id,
            job_id,
            trial_count,
            duration_steps,
            frozen_scenario,
        )?;
        let resolved = parse(&resolved).map_err(|_| Fault::field("qualification_resolution"))?;
        let function = read::map(&resolved, "function_decision")?;
        let function_decision_id = read::hex(function, "function_decision_id", 64)?;
        if read::hex(body, "function_decision_id", 64)? != function_decision_id {
            return Err(Fault::field("function_decision_id"));
        }
        let content = self.content.as_ref().map_err(|fault| fault.clone())?;
        let contract = content
            .contract(
                frozen_scenario
                    .contract_id()
                    .ok_or_else(|| Fault::field("contract_id"))?,
            )
            .ok_or_else(|| Fault::field("contract_id"))?;
        let decisions = read::list(&resolved, "criterion_decisions", 4_096)?;
        let ratio = |numerator: i64, denominator: i64| -> Result<i64, Fault> {
            if numerator < 0 || denominator < 0 {
                return Err(Fault::field("qualification_grade_evidence"));
            }
            if denominator == 0 {
                return Ok(crate::state::FRAC_ONE);
            }
            Ok(i64::try_from(
                (i128::from(numerator.min(denominator)) * i128::from(crate::state::FRAC_ONE))
                    / i128::from(denominator),
            )
            .map_err(|_| Fault::field("qualification_grade_evidence"))?)
        };

        let mut service_relations = Vec::new();
        let mut passed_by_trial = vec![true; usize::from(trial_count)];
        for decision in decisions {
            let definition = read::map(decision, "definition")?;
            let trial = read::int(definition, "trial", 0, i64::from(trial_count - 1))? as usize;
            let passed = read::flag(definition, "passed")?;
            passed_by_trial[trial] &= passed;
            match read::text(definition, "metric")? {
                "stored_charge" | "accepted_flow" => {
                    let measured = read::int(definition, "measured", 0, crate::json::MAX_SAFE_INT)?;
                    let threshold = read::int(definition, "threshold", 1, crate::json::MAX_SAFE_INT)?;
                    service_relations.push((
                        read::hex(decision, "decision_id", 64)?.to_string(),
                        read::text(definition, "metric")?.to_string(),
                        ratio(measured, threshold)?,
                        trial as u16,
                    ));
                }
                "leakage_ratio" | "hands_off_steps" => {}
                _ => return Err(Fault::field("qualification_grade_evidence")),
            }
        }
        if service_relations.is_empty() {
            return Err(Fault::field("qualification_grade_evidence"));
        }
        let throughput_score = service_relations
            .iter()
            .map(|(_, _, relation, _)| *relation)
            .min()
            .ok_or_else(|| Fault::field("qualification_grade_evidence"))?;
        let passed_trials = passed_by_trial.iter().filter(|passed| **passed).count() as i64;
        let pass_fraction = ratio(passed_trials, i64::from(trial_count))?;
        let resilience_score = pass_fraction.min(throughput_score);

        let mut economy_trials = Vec::new();
        let mut economy_score = crate::state::FRAC_ONE;
        for (expected_trial, artifact) in artifacts.iter().enumerate() {
            if read::int(artifact, "trial", 0, i64::from(trial_count - 1))? as usize
                != expected_trial
            {
                return Err(Fault::field("qualification_artifacts"));
            }
            let evidence = read::map(artifact, "grade_evidence")?;
            read::exact_keys(
                evidence,
                "grade_evidence",
                &[
                    "drain",
                    "final_material_units",
                    "initial_material_units",
                    "interventions",
                    "leakage",
                    "materials",
                    "moved",
                    "overload",
                    "renewal",
                    "supply",
                    "upkeep",
                    "version",
                ],
            )?;
            if read::int(evidence, "version", 1, 1)? != 1 {
                return Err(Fault::field("grade_evidence"));
            }
            let drain = read::int(evidence, "drain", 0, crate::json::MAX_SAFE_INT)?;
            let final_material = read::int(
                evidence,
                "final_material_units",
                0,
                crate::json::MAX_SAFE_INT,
            )?;
            let initial_material = read::int(
                evidence,
                "initial_material_units",
                0,
                crate::json::MAX_SAFE_INT,
            )?;
            let interventions = read::int(
                evidence,
                "interventions",
                0,
                crate::json::MAX_SAFE_INT,
            )?;
            let leakage = read::int(evidence, "leakage", 0, crate::json::MAX_SAFE_INT)?;
            let materials = read::list(evidence, "materials", 3)?;
            if materials.len() != 3 {
                return Err(Fault::field("materials"));
            }
            let mut typed_material_retention = crate::state::FRAC_ONE;
            let mut typed_final_units = 0_i64;
            let mut typed_initial_units = 0_i64;
            for (expected_kind, material) in ["boundary_blank", "conductor", "junction_blank"]
                .into_iter()
                .zip(materials)
            {
                read::exact_keys(material, "materials", &["final", "initial", "kind"])?;
                if read::text(material, "kind")? != expected_kind {
                    return Err(Fault::field("materials"));
                }
                let final_units = read::int(material, "final", 0, crate::json::MAX_SAFE_INT)?;
                let initial_units = read::int(material, "initial", 0, crate::json::MAX_SAFE_INT)?;
                typed_final_units = typed_final_units
                    .checked_add(final_units)
                    .ok_or_else(|| Fault::field("materials"))?;
                typed_initial_units = typed_initial_units
                    .checked_add(initial_units)
                    .ok_or_else(|| Fault::field("materials"))?;
                let retained = if initial_units == 0 {
                    crate::state::FRAC_ONE
                } else {
                    ratio(final_units, initial_units)?
                };
                typed_material_retention = typed_material_retention.min(retained);
            }
            if typed_final_units != final_material || typed_initial_units != initial_material {
                return Err(Fault::field("materials"));
            }
            let moved = read::int(evidence, "moved", 0, crate::json::MAX_SAFE_INT)?;
            let overload = read::int(evidence, "overload", 0, crate::json::MAX_SAFE_INT)?;
            let renewal = read::int(evidence, "renewal", 0, crate::json::MAX_SAFE_INT)?;
            let supply = read::int(evidence, "supply", 0, crate::json::MAX_SAFE_INT)?;
            let upkeep = read::int(evidence, "upkeep", 0, crate::json::MAX_SAFE_INT)?;
            let losses = [drain, leakage, overload, renewal, upkeep]
                .into_iter()
                .try_fold(0_i64, |sum, value| sum.checked_add(value))
                .filter(|sum| *sum <= crate::json::MAX_SAFE_INT)
                .ok_or_else(|| Fault::field("qualification_grade_evidence"))?;
            let charge_efficiency = ratio(supply, supply.saturating_add(losses))?;
            let material_retention = typed_material_retention;
            let intervention_score = if interventions == 0 {
                crate::state::FRAC_ONE
            } else {
                0
            };
            economy_score = economy_score
                .min(charge_efficiency)
                .min(material_retention)
                .min(intervention_score);
            economy_trials.push((
                read::hex(artifact, "artifact_id", 64)?.to_string(),
                charge_efficiency,
                drain,
                final_material,
                initial_material,
                interventions,
                leakage,
                material_retention,
                moved,
                overload,
                renewal,
                supply,
                expected_trial as u16,
                upkeep,
            ));
        }

        let assembly = frozen_scenario
            .assembly_template()
            .and_then(|template| template.field())
            .ok_or_else(|| Fault::field("assembly_template"))?;
        let component_count = assembly.ports.len() as i64;
        let route_count = assembly.routes.len() as i64;
        let policy = frozen_scenario.generator().local_policy();
        let policy_component_count = policy.components().len() as i64;
        let rule_count = policy
            .components()
            .iter()
            .map(|component| component.rules.len() as i64)
            .sum::<i64>();
        let policy_bytes = policy.written().len() as i64;
        let headroom = |used: i64, maximum: i64| -> Result<i64, Fault> {
            if maximum <= 0 || used < 0 || used > maximum {
                return Err(Fault::field("qualification_grade_evidence"));
            }
            ratio(maximum - used, maximum)
        };
        let component_headroom = headroom(component_count, i64::from(contract.limits.max_components))?;
        let route_headroom = headroom(route_count, i64::from(contract.limits.max_routes))?;
        let maximum_rules = i64::from(contract.limits.max_components)
            * i64::from(contract.limits.max_rules_per_component);
        let rule_headroom = headroom(rule_count, maximum_rules.max(1))?;
        let complexity_score = component_headroom.min(route_headroom).min(rule_headroom);

        let mut throughput_evidence = String::new();
        {
            let mut evidence = Obj::new(&mut throughput_evidence);
            evidence.text("score_basis", "minimum_service_fulfillment_v1");
            {
                let mut relations = evidence.list("service_relations");
                for (decision_id, metric, fulfillment, trial) in &service_relations {
                    let mut relation = relations.object();
                    relation.text("criterion_decision_id", decision_id);
                    relation.int("fulfillment", *fulfillment);
                    relation.text("metric", metric);
                    relation.int("trial", i64::from(*trial));
                    relation.end();
                }
                relations.end();
            }
            evidence.end();
        }
        let mut resilience_evidence = String::new();
        {
            let mut evidence = Obj::new(&mut resilience_evidence);
            evidence.int("pass_fraction", pass_fraction);
            {
                let mut vector = evidence.list("passed_trials");
                for passed in &passed_by_trial {
                    vector.raw(if *passed { "true" } else { "false" });
                }
                vector.end();
            }
            evidence.text("score_basis", "minimum_pass_and_service_fraction_v1");
            evidence.int("worst_service_fulfillment", throughput_score);
            evidence.end();
        }
        let mut economy_evidence = String::new();
        {
            let mut evidence = Obj::new(&mut economy_evidence);
            evidence.text("score_basis", "minimum_charge_material_intervention_efficiency_v1");
            {
                let mut trials = evidence.list("trials");
                for (
                    artifact_id,
                    charge_efficiency,
                    drain,
                    final_material,
                    initial_material,
                    interventions,
                    leakage,
                    material_retention,
                    moved,
                    overload,
                    renewal,
                    supply,
                    trial,
                    upkeep,
                ) in &economy_trials
                {
                    let mut held = trials.object();
                    held.text("artifact_id", artifact_id);
                    held.int("charge_efficiency", *charge_efficiency);
                    held.int("drain", *drain);
                    held.int("final_material_units", *final_material);
                    held.int("initial_material_units", *initial_material);
                    held.int("interventions", *interventions);
                    held.int("leakage", *leakage);
                    held.int("material_retention", *material_retention);
                    held.int("moved", *moved);
                    held.int("overload", *overload);
                    held.int("renewal", *renewal);
                    held.int("supply", *supply);
                    held.int("trial", i64::from(*trial));
                    held.int("upkeep", *upkeep);
                    held.end();
                }
                trials.end();
            }
            evidence.end();
        }
        let mut complexity_evidence = String::new();
        {
            let mut evidence = Obj::new(&mut complexity_evidence);
            evidence.int("component_count", component_count);
            evidence.int("component_headroom", component_headroom);
            evidence.int("policy_bytes", policy_bytes);
            evidence.int("policy_component_count", policy_component_count);
            evidence.int("route_count", route_count);
            evidence.int("route_headroom", route_headroom);
            evidence.int("rule_count", rule_count);
            evidence.int("rule_headroom", rule_headroom);
            evidence.text("score_basis", "minimum_declared_capacity_headroom_v1");
            evidence.end();
        }

        let grade_inputs: [(&str, &[i64; 4], i64, &str); 4] = [
            (
                "throughput",
                &contract.grade_bands.throughput,
                throughput_score,
                &throughput_evidence,
            ),
            (
                "resilience",
                &contract.grade_bands.resilience,
                resilience_score,
                &resilience_evidence,
            ),
            (
                "economy",
                &contract.grade_bands.economy,
                economy_score,
                &economy_evidence,
            ),
            (
                "complexity",
                &contract.grade_bands.complexity,
                complexity_score,
                &complexity_evidence,
            ),
        ];
        let mut grades = Vec::new();
        for (axis, bands, score, evidence) in grade_inputs {
            let mut band_definition = String::new();
            {
                let mut definition = Obj::new(&mut band_definition);
                definition.text("axis", axis);
                {
                    let mut authored = definition.list("bands");
                    for threshold in bands {
                        authored.int(*threshold);
                    }
                    authored.end();
                }
                definition.text("formula", match axis {
                    "throughput" => "minimum_service_fulfillment_v1",
                    "resilience" => "minimum_pass_and_service_fraction_v1",
                    "economy" => "minimum_charge_material_intervention_efficiency_v1",
                    _ => "minimum_declared_capacity_headroom_v1",
                });
                definition.int("version", 1);
                definition.end();
            }
            let band_definition_hash = hex_bytes(&sha256::digest(band_definition.as_bytes()));
            let band = bands.iter().filter(|threshold| score >= **threshold).count() as i64;
            let mut definition = String::new();
            {
                let mut grade = Obj::new(&mut definition);
                grade.text("axis", axis);
                grade.int("band", band);
                grade.text("band_definition_hash", &band_definition_hash);
                {
                    let mut authored = grade.list("bands");
                    for threshold in bands {
                        authored.int(*threshold);
                    }
                    authored.end();
                }
                grade.raw("evidence", evidence);
                grade.text("function_decision_id", function_decision_id);
                grade.text("job_id", job_id);
                grade.text("request_id", request_id);
                grade.int("score", score);
                grade.text("status", "available");
                grade.int("version", 1);
                grade.end();
            }
            let grade_id = hex_bytes(&sha256::digest(definition.as_bytes()));
            let mut written = String::new();
            {
                let mut grade = Obj::new(&mut written);
                grade.raw("definition", &definition);
                grade.text("grade_id", &grade_id);
                grade.end();
            }
            grades.push(written);
        }
        let mut out = String::new();
        let mut object = Obj::new(&mut out);
        {
            let mut held = object.list("grades");
            for grade in grades {
                held.raw(&grade);
            }
            held.end();
        }
        object.text("job_id", job_id);
        object.text("request_id", request_id);
        object.text("status", "graded");
        object.int("version", 1);
        object.end();
        Ok(out)
    }

    fn qualification_resolution(
        &self,
        body: &Json,
        request_id: &str,
        job_id: &str,
        trial_count: u16,
        duration_steps: u32,
        frozen_scenario: &ScenarioSpec,
    ) -> Result<String, Fault> {
        read::exact_keys(
            body,
            "body",
            &["artifacts", "job_id", "op", "request_id"],
        )?;
        if read::hex(body, "job_id", 64)? != job_id {
            return Err(Fault::field("qualification_job"));
        }
        let artifacts = read::list(body, "artifacts", 64)?;
        if artifacts.len() != usize::from(trial_count) {
            return Err(Fault::field("qualification_artifacts"));
        }
        let content = self.content.as_ref().map_err(|fault| fault.clone())?;
        let contract_id = frozen_scenario
            .contract_id()
            .ok_or_else(|| Fault::field("contract_id"))?;
        let contract = content
            .contract(contract_id)
            .ok_or_else(|| Fault::field("contract_id"))?;
        let criterion_spec = contract.function_criterion()?;
        let expected_scenario_hash = frozen_scenario.scenario_hash();
        let expected_generator_hash = frozen_scenario.generator().specification_hash();
        let expected_assembly_hash = frozen_scenario
            .assembly_template_hash()
            .ok_or_else(|| Fault::field("assembly_template"))?;
        let mut decisions = Vec::new();
        let mut all_passed = true;

        for (expected_trial, artifact) in artifacts.iter().enumerate() {
            read::exact_keys(
                artifact,
                "qualification_artifact",
                &[
                    "artifact_id",
                    "criterion_runtime",
                    "duration_steps",
                    "executed_steps",
                    "first_failure_events",
                    "first_failure_events_truncated",
                    "first_failure_payload",
                    "first_failure_payload_hash",
                    "first_failure_step",
                    "grade_evidence",
                    "job_id",
                    "request_id",
                    "status",
                    "terminal_embodied_state_hash",
                    "terminal_events",
                    "terminal_events_truncated",
                    "terminal_payload",
                    "terminal_payload_hash",
                    "trial",
                    "version",
                ],
            )?;
            if read::hex(artifact, "job_id", 64)? != job_id
                || read::hex(artifact, "request_id", 64)? != request_id
                || read::one_of(artifact, "status", &["completed"])? != 0
                || read::int(artifact, "version", 3, 3)? != 3
                || read::int(
                    artifact,
                    "duration_steps",
                    i64::from(duration_steps),
                    i64::from(duration_steps),
                )? != i64::from(duration_steps)
                || read::int(
                    artifact,
                    "executed_steps",
                    i64::from(duration_steps),
                    i64::from(duration_steps),
                )? != i64::from(duration_steps)
            {
                return Err(Fault::field("qualification_artifact"));
            }
            let trial = read::int(artifact, "trial", 0, i64::from(trial_count - 1))? as u16;
            if usize::from(trial) != expected_trial {
                return Err(Fault::field("qualification_artifacts"));
            }
            let terminal_payload = read::text(artifact, "terminal_payload")?;
            if hex_bytes(&sha256::digest(terminal_payload.as_bytes()))
                != read::hex(artifact, "terminal_payload_hash", 64)?
            {
                return Err(Fault::field("terminal_payload_hash"));
            }
            let parsed = parse(terminal_payload)
                .map_err(|_| Fault::field("terminal_payload"))?;
            let terminal = RunState::read(&parsed)?;
            terminal.coherent()?;
            let mut trial_definition = String::new();
            {
                let mut definition = Obj::new(&mut trial_definition);
                definition.text("request_id", request_id);
                definition.int("trial", i64::from(trial));
                definition.int("version", 3);
                definition.end();
            }
            let trial_hash = hex_bytes(&sha256::digest(trial_definition.as_bytes()));
            let terminal_runtime = terminal
                .criterion
                .as_ref()
                .ok_or_else(|| Fault::field("criterion_runtime"))?;
            let runtime_written = terminal_runtime.written();
            let mut submitted_runtime = String::new();
            crate::json::write_value(
                &mut submitted_runtime,
                read::at(artifact, "criterion_runtime")?,
            )
            .map_err(|_| Fault::field("criterion_runtime"))?;
            if terminal.run_id != &trial_hash[..16]
                || terminal.run_kind != crate::state::RunKind::AutomationContract
                || terminal.scenario.scenario_hash() != expected_scenario_hash
                || terminal.scenario.generator().specification_hash() != expected_generator_hash
                || terminal.scenario.assembly_template_hash() != Some(expected_assembly_hash)
                || terminal.now.step != duration_steps
                || terminal.now.embodied_hash()
                    != read::hex(artifact, "terminal_embodied_state_hash", 64)?
                || terminal_runtime.last_step() != terminal.now.step
                || submitted_runtime != runtime_written
            {
                return Err(Fault::field("qualification_artifact"));
            }
            let mut artifact_definition = String::new();
            {
                let mut held = Obj::new(&mut artifact_definition);
                held.raw("criterion_runtime", &runtime_written);
                held.int("duration_steps", i64::from(duration_steps));
                held.int("executed_steps", i64::from(duration_steps));
                {
                    let mut events = held.list("first_failure_events");
                    for event in read::list(artifact, "first_failure_events", 192)? {
                        let mut written = String::new();
                        crate::json::write_value(&mut written, event)
                            .map_err(|_| Fault::field("first_failure_events"))?;
                        events.raw(&written);
                    }
                    events.end();
                }
                held.bool(
                    "first_failure_events_truncated",
                    read::flag(artifact, "first_failure_events_truncated")?,
                );
                match read::at(artifact, "first_failure_payload")? {
                    Json::Text(payload) => held.text("first_failure_payload", payload),
                    Json::Null => held.null("first_failure_payload"),
                    _ => return Err(Fault::field("first_failure_payload")),
                };
                match read::at(artifact, "first_failure_payload_hash")? {
                    Json::Text(hash) => held.text("first_failure_payload_hash", hash),
                    Json::Null => held.null("first_failure_payload_hash"),
                    _ => return Err(Fault::field("first_failure_payload_hash")),
                };
                held.int_or_null(
                    "first_failure_step",
                    read::int_or_null(
                        artifact,
                        "first_failure_step",
                        1,
                        i64::from(duration_steps),
                    )?,
                );
                let mut grade_evidence = String::new();
                crate::json::write_value(
                    &mut grade_evidence,
                    read::map(artifact, "grade_evidence")?,
                )
                .map_err(|_| Fault::field("grade_evidence"))?;
                held.raw("grade_evidence", &grade_evidence);
                held.text("job_id", job_id);
                held.text("request_id", request_id);
                held.text(
                    "terminal_embodied_state_hash",
                    read::hex(artifact, "terminal_embodied_state_hash", 64)?,
                );
                {
                    let mut events = held.list("terminal_events");
                    for event in read::list(artifact, "terminal_events", 192)? {
                        let mut written = String::new();
                        crate::json::write_value(&mut written, event)
                            .map_err(|_| Fault::field("terminal_events"))?;
                        events.raw(&written);
                    }
                    events.end();
                }
                held.bool(
                    "terminal_events_truncated",
                    read::flag(artifact, "terminal_events_truncated")?,
                );
                held.text(
                    "terminal_payload_hash",
                    read::hex(artifact, "terminal_payload_hash", 64)?,
                );
                held.int("trial", i64::from(trial));
                held.int("version", 3);
                held.end();
            }
            if hex_bytes(&sha256::digest(artifact_definition.as_bytes()))
                != read::hex(artifact, "artifact_id", 64)?
            {
                return Err(Fault::field("artifact_id"));
            }

            let validate_events = |key: &str, maximum_step: u32| -> Result<(), Fault> {
                let mut prior = 0_u32;
                for event in read::list(artifact, key, 192)? {
                    read::exact_keys(event, key, &["body", "ev", "step"])?;
                    read::map(event, "body")?;
                    read::text(event, "ev")?;
                    let step = read::int(event, "step", 0, i64::from(maximum_step))? as u32;
                    if step < prior {
                        return Err(Fault::field(key));
                    }
                    prior = step;
                }
                Ok(())
            };
            validate_events("terminal_events", duration_steps)?;

            let first_failure = match terminal_runtime.status() {
                crate::criterion::CriterionStatus::Failed => {
                    let payload = read::text(artifact, "first_failure_payload")?;
                    if hex_bytes(&sha256::digest(payload.as_bytes()))
                        != read::hex(artifact, "first_failure_payload_hash", 64)?
                    {
                        return Err(Fault::field("first_failure_payload_hash"));
                    }
                    let failure_step = read::int(
                        artifact,
                        "first_failure_step",
                        1,
                        i64::from(duration_steps),
                    )? as u32;
                    validate_events("first_failure_events", failure_step)?;
                    let payload = parse(payload)
                        .map_err(|_| Fault::field("first_failure_payload"))?;
                    let failure = RunState::read(&payload)?;
                    failure.coherent()?;
                    let failure_runtime = failure
                        .criterion
                        .as_ref()
                        .ok_or_else(|| Fault::field("criterion_runtime"))?;
                    if failure.run_id != terminal.run_id
                        || failure.run_kind != terminal.run_kind
                        || failure.scenario.scenario_hash() != expected_scenario_hash
                        || failure.scenario.generator().specification_hash()
                            != expected_generator_hash
                        || failure.scenario.assembly_template_hash() != Some(expected_assembly_hash)
                        || failure.now.step != failure_step
                        || failure_runtime.status()
                            != crate::criterion::CriterionStatus::Failed
                        || failure_runtime.resolved_step() != Some(failure_step)
                    {
                        return Err(Fault::field("first_failure_payload"));
                    }
                    Some(failure)
                }
                _ => {
                    if !matches!(read::at(artifact, "first_failure_payload")?, Json::Null)
                        || !matches!(
                            read::at(artifact, "first_failure_payload_hash")?,
                            Json::Null
                        )
                        || read::int_or_null(
                            artifact,
                            "first_failure_step",
                            1,
                            i64::from(duration_steps),
                        )?
                        .is_some()
                        || !read::list(artifact, "first_failure_events", 192)?.is_empty()
                        || read::flag(artifact, "first_failure_events_truncated")?
                    {
                        return Err(Fault::field("first_failure_payload"));
                    }
                    None
                }
            };

            let evidence_state = first_failure.as_ref().unwrap_or(&terminal);
            let runtime = evidence_state
                .criterion
                .as_ref()
                .ok_or_else(|| Fault::field("criterion_runtime"))?;
            let reading = runtime.current_reading(&criterion_spec, &evidence_state.now, true);
            if !reading.ready || reading.observed_steps != criterion_spec.window_steps() {
                return Err(Fault::field("criterion_evidence"));
            }
            for criterion in &contract.qualification.criteria {
                let measured = match (criterion.metric, criterion.source) {
                    (
                        crate::content::CriterionMetric::StoredCharge,
                        crate::content::CriterionSource::Component(node),
                    ) => runtime
                        .component_window_minimum(&criterion_spec, node)
                        .ok_or_else(|| Fault::field("criterion_evidence"))?,
                    (
                        crate::content::CriterionMetric::AcceptedFlow,
                        crate::content::CriterionSource::Route(route),
                    ) => reading
                        .routes
                        .iter()
                        .find(|held| held.route == route)
                        .map(|held| held.minimum)
                        .ok_or_else(|| Fault::field("criterion_evidence"))?,
                    (
                        crate::content::CriterionMetric::LeakageRatio,
                        crate::content::CriterionSource::Field,
                    ) => reading
                        .leakage
                        .ratio
                        .ok_or_else(|| Fault::field("criterion_evidence"))?,
                    (
                        crate::content::CriterionMetric::HandsOffSteps,
                        crate::content::CriterionSource::Field,
                    ) => i64::from(runtime.hands_off_streak()),
                    _ => return Err(Fault::field("criterion_evidence")),
                };
                let passed = match criterion.comparison {
                    crate::content::CriterionComparison::AtLeast => measured >= criterion.threshold,
                    crate::content::CriterionComparison::AtMost => measured <= criterion.threshold,
                };
                all_passed &= passed;
                let margin = match criterion.comparison {
                    crate::content::CriterionComparison::AtLeast => measured - criterion.threshold,
                    crate::content::CriterionComparison::AtMost => criterion.threshold - measured,
                };
                let (source_kind, source_id) = criterion.source.parts();
                let window_end_step = evidence_state.now.step;
                let window_start_step = window_end_step
                    .saturating_sub(criterion.window_steps)
                    .saturating_add(1);
                let mut definition = String::new();
                {
                    let mut decision = Obj::new(&mut definition);
                    decision.text("aggregation", criterion.aggregation.name());
                    decision.text("artifact_id", read::hex(artifact, "artifact_id", 64)?);
                    decision.text("comparison", criterion.comparison.name());
                    decision.text("criterion_id", &criterion.id);
                    decision.text("job_id", job_id);
                    decision.int("margin", margin);
                    decision.int("measured", measured);
                    decision.text("metric", criterion.metric.name());
                    decision.bool("passed", passed);
                    decision.text("request_id", request_id);
                    decision.int("resolution_step", i64::from(window_end_step));
                    {
                        let mut source = decision.object("source");
                        source.int_or_null("id", source_id.map(i64::from));
                        source.text("kind", source_kind);
                        source.end();
                    }
                    decision.text("status", if passed { "passed" } else { "failed" });
                    decision.int("threshold", criterion.threshold);
                    decision.int("trial", i64::from(trial));
                    decision.int("version", 1);
                    decision.int("window_end_step", i64::from(window_end_step));
                    decision.int("window_start_step", i64::from(window_start_step));
                    decision.int("window_steps", i64::from(criterion.window_steps));
                    decision.end();
                }
                let decision_id = hex_bytes(&sha256::digest(definition.as_bytes()));
                let mut written = String::new();
                {
                    let mut decision = Obj::new(&mut written);
                    decision.text("decision_id", &decision_id);
                    decision.raw("definition", &definition);
                    decision.end();
                }
                decisions.push((decision_id, definition, written));
            }
        }
        let mut function_definition = String::new();
        {
            let mut aggregate = Obj::new(&mut function_definition);
            {
                let mut ids = aggregate.list("criterion_decision_ids");
                for (decision_id, _, _) in &decisions {
                    ids.text(decision_id);
                }
                ids.end();
            }
            aggregate.text("job_id", job_id);
            aggregate.bool("passed", all_passed);
            aggregate.text("request_id", request_id);
            aggregate.text("status", if all_passed { "passed" } else { "failed" });
            aggregate.int("trial_count", i64::from(trial_count));
            aggregate.int("version", 1);
            aggregate.end();
        }
        let function_decision_id = hex_bytes(&sha256::digest(function_definition.as_bytes()));
        let mut function_written = String::new();
        {
            let mut aggregate = Obj::new(&mut function_written);
            aggregate.raw("definition", &function_definition);
            aggregate.text("function_decision_id", &function_decision_id);
            aggregate.end();
        }
        let mut out = String::new();
        let mut object = Obj::new(&mut out);
        {
            let mut held = object.list("criterion_decisions");
            for (_, _, written) in decisions {
                held.raw(&written);
            }
            held.end();
        }
        object.raw("function_decision", &function_written);
        object.text("job_id", job_id);
        object.text("request_id", request_id);
        object.text("status", "resolved");
        object.int("version", 1);
        object.end();
        Ok(out)
    }

    fn restart_commission(&mut self, body: &Json) -> Result<String, Fault> {
        read::exact_keys(
            body,
            "body",
            &[
                "expected_assembly_hash",
                "expected_branch_id",
                "expected_branch_nonce",
                "expected_generator_hash",
            ],
        )?;
        let expected_assembly_hash = read::hex(body, "expected_assembly_hash", 64)?;
        let expected_branch_id = read::hex(body, "expected_branch_id", 64)?;
        let expected_branch_nonce =
            read::int(body, "expected_branch_nonce", 0, i64::from(u32::MAX))? as u32;
        let expected_generator_hash = read::hex(body, "expected_generator_hash", 64)?;
        {
            let run = self.run.as_ref().ok_or_else(|| state_fault(IDLE, LOADED))?;
            let state = run.state();
            if state.run_kind != crate::state::RunKind::AutomationContract {
                return Err(Fault::field("run_kind"));
            }
            if state
                .attempt_branch
                .as_ref()
                .is_none_or(|branch| branch.branch_id() != expected_branch_id)
            {
                return Err(Fault::field("branch_id"));
            }
            if state.branch_nonce != expected_branch_nonce {
                return Err(Fault::field("branch_nonce"));
            }
            if state.scenario.generator().specification_hash() != expected_generator_hash {
                return Err(Fault::field("generator_spec_hash"));
            }
            if state.scenario.assembly_template_hash() != Some(expected_assembly_hash) {
                return Err(Fault::field("assembly_template_hash"));
            }
        }
        let (chapter, chapter_count, contract_run, field, view) = {
            let content = self.content.as_ref().map_err(Fault::clone)?;
            let run = self.run.as_ref().ok_or_else(|| state_fault(IDLE, LOADED))?;
            if run.state().scenario.content_hash() != content.hash {
                return Err(Fault::because(Code::ContentInvalid, "content_hash"));
            }
            let contract_run = run.state().scenario.contract_id().is_some();
            let chapter = match run.state().scenario.contract_id() {
                Some(contract_id) => {
                    let contract = content
                        .contract(contract_id)
                        .ok_or_else(|| Fault::because(Code::ContentInvalid, "contract_id"))?;
                    content
                        .chapters
                        .iter()
                        .find(|chapter| chapter.id == contract.opening.chapter())
                        .ok_or_else(|| Fault::because(Code::ContentInvalid, "opening"))?
                }
                None => content
                    .chapter(run.state().progress.chapter_index)
                    .ok_or_else(|| Fault::because(Code::ContentInvalid, "chapters"))?,
            }
            .clone();
            let (field, view) = match run.state().scenario.contract_id() {
                Some(contract_id) => {
                    let (_fallback, view) = content
                        .contract(contract_id)
                        .ok_or_else(|| Fault::because(Code::ContentInvalid, "contract_id"))?
                        .establish(content)?;
                    let field = run
                        .state()
                        .scenario
                        .assembly_template()
                        .and_then(crate::state::AssemblyTemplate::field)
                        .cloned()
                        .ok_or_else(|| Fault::field("assembly_template"))?;
                    (field, view)
                }
                None => {
                    let form = content
                        .form(run.form())
                        .ok_or_else(|| Fault::because(Code::ContentInvalid, "forms"))?;
                    content::establish(&chapter, form)?
                }
            };
            (chapter, if contract_run { 1 } else { content.chapters.len() }, contract_run, field, view)
        };

        let run = self.loaded()?;
        run.restart_commission(field, view)?;
        if !contract_run {
            run.open_chapter(&chapter, chapter_count);
        }
        run.open_schedule(&chapter);
        let mut out = String::new();
        let mut object = Obj::new(&mut out);
        write_run_identity(&mut object, run.state());
        match run.state().scenario.contract_id() {
            Some(id) => object.text("contract_id", id),
            None => object.null("contract_id"),
        };
        object.text("embodied_state_hash", &run.state().now.embodied_hash());
        object.text(
            "generator_spec_hash",
            &run.state().scenario.generator().specification_hash(),
        );
        object.raw(
            "local_policy",
            &run.state().scenario.generator().local_policy().written(),
        );
        object.raw(
            "route_defaults",
            &run.state()
                .scenario
                .generator()
                .route_defaults_written(run.state().progress.chapter_index),
        );
        object.text("scenario_hash", &run.state().scenario.scenario_hash());
        object.int("step", i64::from(run.state().now.step));
        object.raw("view", &run.state().view.written());
        object.end();
        self.drain_events();
        Ok(out)
    }

    fn resume_commission(&mut self, body: &Json) -> Result<String, Fault> {
        read::exact_keys(body, "body", &[])?;
        let run = self.loaded()?;
        run.resume_commission()?;
        let mut out = String::new();
        let mut object = Obj::new(&mut out);
        write_run_identity(&mut object, run.state());
        match run.state().scenario.contract_id() {
            Some(id) => object.text("contract_id", id),
            None => object.null("contract_id"),
        };
        object.text("embodied_state_hash", &run.state().now.embodied_hash());
        object.text(
            "generator_spec_hash",
            &run.state().scenario.generator().specification_hash(),
        );
        object.raw(
            "local_policy",
            &run.state().scenario.generator().local_policy().written(),
        );
        object.raw(
            "route_defaults",
            &run.state()
                .scenario
                .generator()
                .route_defaults_written(run.state().progress.chapter_index),
        );
        object.text("scenario_hash", &run.state().scenario.scenario_hash());
        object.int("step", i64::from(run.state().now.step));
        object.raw("view", &run.state().view.written());
        object.end();
        Ok(out)
    }

    fn return_commission(&mut self, body: &Json) -> Result<String, Fault> {
        read::exact_keys(body, "body", &[])?;
        let run = self.loaded()?;
        run.return_commission()?;
        let mut out = String::new();
        let mut object = Obj::new(&mut out);
        write_run_identity(&mut object, run.state());
        match run.state().scenario.contract_id() {
            Some(id) => object.text("contract_id", id),
            None => object.null("contract_id"),
        };
        object.text("embodied_state_hash", &run.state().now.embodied_hash());
        object.text(
            "generator_spec_hash",
            &run.state().scenario.generator().specification_hash(),
        );
        object.text("scenario_hash", &run.state().scenario.scenario_hash());
        object.int("step", i64::from(run.state().now.step));
        object.end();
        Ok(out)
    }

    fn inspect_field(&self, body: &Json) -> Result<String, Fault> {
        let historical = body.get("step").is_some();
        read::exact_keys(
            body,
            "body",
            if historical { &["id", "step", "target"] } else { &["id", "target"] },
        )?;
        let id = read::int(body, "id", 0, i64::from(u32::MAX))?;
        let target = read::text(body, "target")?;
        let mut request = String::new();
        let mut object = Obj::new(&mut request);
        object.int("id", id);
        object.text("target", target);
        object.end();
        let request = parse(&request).expect("the inspection request writer is canonical");
        let run = self.run.as_ref().ok_or_else(|| state_fault(IDLE, LOADED))?;
        if historical {
            let step = read::int(body, "step", 0, i64::from(run.step()))? as u32;
            let state = run.inspection_state_at(step).ok_or_else(|| Fault::field("step"))?;
            crate::field_inspect::inspect(&state, &request)
        } else {
            crate::field_inspect::inspect(run.state(), &request)
        }
    }

    fn design_diff(
        previous_policy: &crate::policy::FrozenLocalPolicy,
        previous_routes: &[crate::policy::RouteControlState],
        next_policy: &crate::policy::FrozenLocalPolicy,
        next_routes: &[crate::policy::RouteControlState],
    ) -> String {
        let mut changed_routes: Vec<u32> = previous_routes
            .iter()
            .map(|control| control.route)
            .chain(next_routes.iter().map(|control| control.route))
            .collect();
        changed_routes.sort_unstable();
        changed_routes.dedup();
        changed_routes.retain(|route| {
            previous_routes.iter().find(|control| control.route == *route)
                != next_routes.iter().find(|control| control.route == *route)
        });

        let mut out = String::new();
        let mut object = Obj::new(&mut out);
        object.bool("policy_changed", previous_policy != next_policy);
        {
            let mut routes = object.list("route_defaults_changed");
            for route in changed_routes {
                routes.int(i64::from(route));
            }
            routes.end();
        }
        object.end();
        out
    }

    /// Validates and freezes one constrained Open Field draft against the
    /// fixed discrete lawset. Physical scalars arrive as fixed-point integers;
    /// the core's canonicalizer and SHA-256 establish the compiled identity.
    fn compile_scenario(&self, body: &Json) -> Result<String, Fault> {
        read::exact_keys(body, "body", &["draft"])?;
        let draft = read::map(body, "draft")?;
        read::exact_keys(
            draft,
            "draft",
            &[
                "compartment_leak",
                "conductance_noise",
                "control",
                "components",
                "compartment_members",
                "criterion_duration",
                "criterion_failure_grace",
                "criterion_floor",
                "criterion_leakage_ceiling",
                "criterion_route_floor",
                "criterion_window",
                "dissipation_per_step",
                "form",
                "intervention",
                "lawset_id",
                "materials",
                "observation_resolution",
                "observation_window",
                "route_capacity_scale",
                "routes",
                "supply_per_step",
                "supply_layer",
                "supply_width",
                "supply_x",
                "supply_y",
                "trial_count",
            ],
        )?;
        read::one_of(
            draft,
            "lawset_id",
            &[
                "discrete-transport-v1",
                "discrete-transport-crowded-v1",
                "discrete-transport-vestige-v1",
                "discrete-transport-holdout-v1",
            ],
        )?;
        read::one_of(draft, "form", &crate::run::FORMS)?;
        read::one_of(draft, "control", &["recorded_open_loop", "hands_off"])?;
        read::int(draft, "supply_per_step", 0, crate::field::CURRENT_STRENGTH_CAP)?;
        read::int(draft, "supply_width", 8 * 65_536, crate::fx::STORED_BOUND - 1)?;
        read::int(draft, "dissipation_per_step", 0, crate::fx::STORED_BOUND - 1)?;
        read::int(draft, "conductance_noise", 0, crate::state::FRAC_ONE)?;
        read::int(draft, "route_capacity_scale", 8_192, 262_144)?;
        read::int(draft, "compartment_leak", 0, crate::field::LEAK_FRAC_CAP)?;
        read::int(draft, "observation_window", 15, 1_800)?;
        let resolution = read::int(draft, "observation_resolution", 1, 32)? as u32;
        if !resolution.is_power_of_two() {
            return Err(Fault::field("observation_resolution"));
        }
        read::int(draft, "criterion_floor", 0, crate::field::NODE_CHARGE_CAP)?;
        read::int(draft, "criterion_route_floor", 0, crate::field::ROUTE_CAPACITY_CAP)?;
        read::int(draft, "criterion_leakage_ceiling", 0, crate::state::FRAC_ONE)?;
        read::int(draft, "criterion_window", 1, 1_800)?;
        read::int(draft, "criterion_failure_grace", 0, 1_800)?;
        read::int(draft, "criterion_duration", 1, 1_800)?;
        read::int(draft, "trial_count", 1, 64)?;
        read_open_topology(draft)?;
        read_open_intervention(draft)?;

        let mut canonical = String::new();
        crate::json::write_value(&mut canonical, draft)
            .map_err(|reason| Fault::because(Code::Validation, reason))?;
        let scenario_hash = hex_bytes(&sha256::digest(canonical.as_bytes()));
        let mut out = String::new();
        let mut object = Obj::new(&mut out);
        object.text("canonical", &canonical);
        object.text("experiment_id", &format!("open-{}", &scenario_hash[..12]));
        object.text("scenario_hash", &scenario_hash);
        object.end();
        Ok(out)
    }

    /// Instantiates a compiled Open Field draft over the current chapter's
    /// authored topology and selected Form, then executes its trial family in
    /// cloned authoritative states. The live run remains unchanged.
    fn run_scenario(&mut self, body: &Json) -> Result<String, Fault> {
        self.compile_scenario(body)?;
        let draft = read::map(body, "draft")?;
        if read::text(draft, "control")? != "hands_off" {
            return Err(Fault::field("control"));
        }
        let supply = read::int(draft, "supply_per_step", 0, crate::field::CURRENT_STRENGTH_CAP)?;
        let supply_width = read::int(draft, "supply_width", 8 * 65_536, crate::fx::STORED_BOUND - 1)?;
        let dissipation = read::int(draft, "dissipation_per_step", 0, crate::fx::STORED_BOUND - 1)?;
        let noise = read::int(draft, "conductance_noise", 0, crate::state::FRAC_ONE)?;
        let route_scale = read::int(draft, "route_capacity_scale", 8_192, 262_144)?;
        let leak = read::int(draft, "compartment_leak", 0, crate::field::LEAK_FRAC_CAP)?;
        let observation_window = read::int(draft, "observation_window", 15, 1_800)? as u32;
        let observation_resolution = read::int(draft, "observation_resolution", 1, 32)? as usize;
        let criterion_floor = read::int(draft, "criterion_floor", 0, crate::field::NODE_CHARGE_CAP)?;
        let criterion_route_floor = read::int(
            draft,
            "criterion_route_floor",
            0,
            crate::field::ROUTE_CAPACITY_CAP,
        )?;
        let criterion_leakage_ceiling = read::int(
            draft,
            "criterion_leakage_ceiling",
            0,
            crate::state::FRAC_ONE,
        )?;
        let criterion_window = read::int(draft, "criterion_window", 1, 1_800)? as u32;
        let criterion_failure_grace =
            read::int(draft, "criterion_failure_grace", 0, 1_800)? as u32;
        let criterion_duration = read::int(draft, "criterion_duration", 1, 1_800)? as u32;
        let trial_count = read::int(draft, "trial_count", 1, 64)? as u32;
        let (intervention_onset, intervention) = read_open_intervention(draft)?;

        let mut canonical = String::new();
        crate::json::write_value(&mut canonical, draft)
            .map_err(|reason| Fault::because(Code::Validation, reason))?;
        let scenario_hash = hex_bytes(&sha256::digest(canonical.as_bytes()));
        let experiment_id = format!("open-{}", &scenario_hash[..12]);
        let chapter_index = self.loaded()?.state().progress.chapter_index;
        let content = self.content.as_ref().map_err(Fault::clone)?;
        let chapter = content
            .chapter(chapter_index)
            .ok_or_else(|| Fault::because(Code::ContentInvalid, "chapters"))?;
        let form = content
            .form(read::text(draft, "form")?)
            .ok_or_else(|| Fault::because(Code::ContentInvalid, "forms"))?;
        let (mut template, _) = content::establish(chapter, form)?;
        apply_open_topology(&mut template, draft)?;
        for current in &mut template.currents {
            current.strength = supply;
            current.width = supply_width;
        }
        for layer in &mut template.layers {
            layer.drain = dissipation;
            layer.noise = noise;
        }
        for route in &mut template.routes {
            route.capacity = crate::fx::fixed_mul(route.capacity, route_scale)
                .clamp(1, crate::field::ROUTE_CAPACITY_CAP);
            route.flow = route.flow.min(route.capacity);
        }
        template.physical_compartment.leak_per_exposed_contact_per_step = leak;
        template.route_clamps.clear();
        template.leak_breach = None;
        template.supply_decoys.clear();
        template.current_delays.clear();
        template.route_scramble = None;
        let embodied_state_hash = template.embodied_hash();
        let generator_hash = crate::state::GeneratorSpec::for_field(&template).specification_hash();
        let mut passed_count = 0u32;
        let mut trial_json = Vec::new();

        for trial in 0..trial_count {
            let mut field = template.clone();

            let mut route_ids: Vec<u32> = field.routes.iter().map(|route| route.route).collect();
            route_ids.sort_unstable();
            let mut component_requirements = Vec::new();
            for node in &field.physical_compartment.members {
                let Some(component) = field
                    .ports
                    .iter()
                    .find(|component| component.node == *node && component.kind != crate::field::NodeKind::Form)
                else {
                    continue;
                };
                component_requirements.push(crate::criterion::ComponentRequirement::new(
                    component.node,
                    criterion_floor,
                )?);
            }
            component_requirements.sort_by_key(|requirement| requirement.node());
            let criterion_spec = crate::criterion::FunctionCriterionSpec::new(
                route_ids,
                criterion_route_floor,
                component_requirements,
                criterion_leakage_ceiling,
                criterion_window,
                criterion_failure_grace,
                criterion_duration,
            )?;
            let mut criterion_runtime =
                crate::criterion::CriterionRuntime::opening(field.step);

            let seed_name = format!("{}:{trial}", scenario_hash);
            let mut staging = crate::field::Unstaged {
                stream: crate::rng::RngState::root(&seed_name),
                ..crate::field::Unstaged::default()
            };
            let mut minimum = i64::MAX;
            let mut samples = 0u32;
            let mut final_criterion = None;
            for elapsed in 1..=observation_window {
                let mut intervened = false;
                if elapsed.saturating_sub(1) == intervention_onset {
                    if let Some(plan) = &intervention {
                        let mut projection = crate::plan::Projection::of(&field);
                        crate::plan::check(plan, &projection)
                            .map_err(crate::plan::Refusal::fault)?;
                        crate::plan::apply(plan, &mut projection)
                            .map_err(crate::plan::Refusal::fault)?;
                        field = projection.field;
                        intervened = true;
                    }
                }
                let outcome = crate::field::advance(
                    &mut field,
                    crate::state::ControlState::default(),
                    crate::state::FRAC_ONE,
                    &mut staging.staging(),
                );
                final_criterion = Some(criterion_runtime.advance(
                    &criterion_spec,
                    crate::criterion::CriterionStepInput {
                        field: &field,
                        records: &outcome.records,
                        ledger: &outcome.ledger,
                        control: crate::state::ControlState::default(),
                        other_external_control: intervened,
                    },
                )?);
                let selected = field
                    .physical_compartment
                    .members
                    .iter()
                    .filter_map(|node| field.ports.iter().find(|port| port.node == *node))
                    .map(|port| port.q)
                    .sum::<i64>();
                minimum = minimum.min(selected);
                if elapsed as usize % observation_resolution == 0 {
                    samples += 1;
                }
            }
            let criterion = final_criterion.ok_or_else(|| Fault::field("criterion"))?;
            let passed = criterion.status == crate::criterion::CriterionStatus::Passed;
            if passed {
                passed_count += 1;
            }
            let mut written = String::new();
            let mut trial_object = Obj::new(&mut written);
            trial_object.raw("criterion", &criterion.written());
            trial_object.int("final_charge", field.ports.iter().map(|port| port.q).sum());
            trial_object.int("minimum_selected_charge", minimum.max(0));
            trial_object.bool("passed", passed);
            trial_object.int("samples", i64::from(samples));
            trial_object.int("seed", i64::from(trial));
            trial_object.int("sustained_steps", i64::from(criterion.hands_off_streak));
            trial_object.end();
            trial_json.push(written);
        }

        let mut out = String::new();
        let mut object = Obj::new(&mut out);
        object.text("control_contract", "hands_off");
        object.text("embodied_state_hash", &embodied_state_hash);
        object.text("experiment_id", &experiment_id);
        object.text("generator_hash", &generator_hash);
        object.int("passed", i64::from(passed_count));
        object.text("scenario_hash", &scenario_hash);
        {
            let mut trials = object.list("trials");
            for trial in &trial_json {
                trials.raw(trial);
            }
            trials.end();
        }
        object.end();
        Ok(out)
    }

    /// Runs one bounded local Renewal policy over a clone of the authoritative
    /// Field. The harness withholds and degrades the target; the policy then
    /// consumes only placed typed materials, local donor Charge, a decoded
    /// neighbor signal, and locally visible topology.
    fn renewal_trial(&mut self, body: &Json) -> Result<String, Fault> {
        read::exact_keys(body, "body", &["seed"])?;
        let seed = read::int(body, "seed", 0, i64::from(u32::MAX))? as u32;
        let (result, control_contract, embodied_state_hash, generator_hash, scenario_hash) = {
            let state = self.loaded()?.state();
            let scenario = state
                .scenario
                .with_control(crate::state::ControlContract::HandsOff);
            (
                crate::field::renewal_assay(&state.now, seed)?,
                scenario.control().name(),
                state.now.embodied_hash(),
                scenario.generator().specification_hash(),
                scenario.scenario_hash(),
            )
        };

        let mut out = String::new();
        let mut object = Obj::new(&mut out);
        object.text("control_contract", control_contract);
        object.int("detected_at", i64::from(result.detected_at));
        object.text("embodied_state_hash", &embodied_state_hash);
        object.int("failed_node", i64::from(result.failed_node));
        object.text("generator_hash", &generator_hash);
        object.int("material_cost", result.material_ids.len() as i64);
        {
            let mut list = object.list("material_ids");
            for material in &result.material_ids {
                list.int(i64::from(*material));
            }
            list.end();
        }
        object.bool("passed", result.passed);
        {
            let mut list = object.list("rebuilt_routes");
            for route in &result.rebuilt_routes {
                list.int(i64::from(*route));
            }
            list.end();
        }
        object.int("reconnected_at", i64::from(result.reconnected_at));
        object.int("reconnection", i64::from(result.reconnection));
        object.int("recovered_at", i64::from(result.recovered_at));
        object.int("recruited_at", i64::from(result.recruited_at));
        object.int_or_null("replacement_node", result.replacement_node.map(i64::from));
        object.int("resource_cost", result.resource_cost);
        object.text("scenario_hash", &scenario_hash);
        object.int("seed", i64::from(seed));
        object.int_or_null("signal_id", result.signal_id.map(i64::from));
        object.end();
        Ok(out)
    }

    /// Exposes the embodied local inputs a Renewal policy can actually sense:
    /// finite placed material and finite-lived local deficit signals. This is
    /// observation only and does not mutate or spend from the live run.
    fn renewal_inventory(&mut self) -> Result<String, Fault> {
        let field = &self.loaded()?.state().now;
        let mut out = String::new();
        let mut object = Obj::new(&mut out);
        {
            let mut materials = object.list("materials");
            for material in &field.materials {
                let mut written = materials.object();
                written.int("amount", i64::from(material.amount));
                written.bool("claimed", material.claimed);
                written.text("kind", material.kind.name());
                written.int("layer", i64::from(material.layer));
                written.int("material", i64::from(material.material));
                written.int("x", material.pos.x);
                written.int("y", material.pos.y);
                written.end();
            }
            materials.end();
        }
        {
            let mut signals = object.list("signals");
            for signal in &field.signals {
                let mut written = signals.object();
                written.int("emitted_step", i64::from(signal.emitted_step));
                written.int("expires_step", i64::from(signal.expires_step));
                written.int("layer", i64::from(signal.layer));
                written.int("signal", i64::from(signal.signal));
                written.int("source", i64::from(signal.source));
                written.int("strength", signal.strength);
                written.int("target", i64::from(signal.target));
                written.int("x", signal.pos.x);
                written.int("y", signal.pos.y);
                written.end();
            }
            signals.end();
        }
        object.int("step", i64::from(field.step));
        object.end();
        Ok(out)
    }

    /// Moves the passive observation View to one candidate of the standing
    /// slate immediately, or clears its member selection at position 0.
    ///
    /// This is deliberately not a `PlanCommand`: changing what is measured
    /// queues no causal edit, spends no Intervention, and never enters the
    /// transaction that can reshape the physical compartment. The run owns
    /// validation against the standing slate and returns the View it adopted.
    fn set_focus(&mut self, body: &Json) -> Result<String, Fault> {
        read::exact_keys(body, "body", &["position", "slate_ordinal"])?;
        let slate_ordinal =
            read::int(body, "slate_ordinal", 0, i64::from(u32::MAX))? as u32;
        // Position 0 clears only the passive View's member selection. Positive
        // positions remain 1-based seats in the standing candidate slate.
        let position = read::int(body, "position", 0, i64::from(u8::MAX))? as u8;
        let view = self.loaded()?.set_focus(slate_ordinal, position)?;
        let mut out = String::new();
        let mut object = Obj::new(&mut out);
        object.raw("view", &view.written());
        object.end();
        Ok(out)
    }

    /// Opens one authored automation contract directly into Design authority.
    /// The legacy chapter reference is resolved here and does not cross to the
    /// shell or become campaign progression.
    fn open_contract(&mut self, body: &Json) -> Result<String, Fault> {
        let completed = if body.get("receipts").is_some() {
            self.completed_contracts_from_receipts(
                body,
                &["contract_id", "receipts", "run_id"],
            )?
        } else {
            read::exact_keys(body, "body", &["contract_id", "run_id"])?;
            Vec::new()
        };
        let contract_id = read::text(body, "contract_id")?;
        let run_id = read::text(body, "run_id")?;
        let (contract, chapter, content_hash, pressures) = {
            let content = self.content.as_ref().map_err(Fault::clone)?;
            let contract = content
                .contract(contract_id)
                .ok_or_else(|| Fault::because(Code::NotFound, "contract_id"))?
                .clone();
            if contract
                .prerequisites
                .iter()
                .any(|required| !completed.contains(required))
            {
                return Err(Fault::because(Code::State, "contract_locked"));
            }
            let chapter = content
                .chapters
                .iter()
                .find(|held| held.id == contract.opening.chapter())
                .ok_or_else(|| Fault::because(Code::ContentInvalid, "opening"))?
                .clone();
            (
                contract,
                chapter,
                content.hash.clone(),
                content.pressures.clone(),
            )
        };
        let (field, view) = contract.establish(self.content.as_ref().map_err(Fault::clone)?)?;
        let regime = RegimeSpec::named(contract.opening.regime())?;
        let criterion = Some(contract.function_criterion()?);
        let generator = crate::state::GeneratorSpec::for_field(&field).with_design(
            0,
            crate::policy::FrozenLocalPolicy::empty(),
            field.route_controls.clone(),
        )?;
        let scenario = ScenarioSpec::for_contract(
            content_hash,
            contract.id.clone(),
            crate::state::AssemblyTemplate::from_field(&field),
            pressures,
            regime,
            generator,
            criterion,
        )?;
        let mut run = Run::start_with_scenario(run_id, contract.opening.form(), scenario)?;
        run.establish_field(field, view)
            .map_err(|fault| read::recode(fault, Code::ContentInvalid))?;
        run.open_schedule(&chapter);
        let state = run.state();
        self.store.note_run(
            &state.run_id,
            contract.opening.form(),
            state.progress.chapter_index,
            state.now.step,
            state.branch_nonce,
        );
        let answer = opened_body(&run, false, None);
        self.run = Some(run);
        self.drain_events();
        Ok(answer)
    }

    /// Opens a run. A new run takes its key from the shell — the only
    /// nondeterministic input a run ever takes — and a restore names a
    /// persistence record.
    fn init_run(&mut self, body: &Json) -> Result<String, Fault> {
        let mode = read::text(body, "mode")?;
        match mode {
            "new" => {
                let regime = match body.get("regime") {
                    Some(_) => {
                        read::exact_keys(body, "body", &["form", "mode", "regime", "run_id"])?;
                        read::text(body, "regime")?
                    }
                    None => {
                        read::exact_keys(body, "body", &["form", "mode", "run_id"])?;
                        "open_field"
                    }
                };
                let run_id = read::text(body, "run_id")?;
                let form = read::text(body, "form")?;
                if !crate::run::FORMS.contains(&form) {
                    return Err(Fault::field("form"));
                }
                let content = self.content.as_ref().map_err(Fault::clone)?;
                // The run key and the Form id are the body's, so they are held
                // to the body's own envelope before the content is asked for
                // anything: a Form outside the closed set is a validation fault
                // whatever the content authors.
                let chapter = content
                    .chapter(0)
                    .ok_or_else(|| Fault::because(Code::ContentInvalid, "chapters"))?;
                let authored = content
                    .form(form)
                    .ok_or_else(|| Fault::because(Code::ContentInvalid, "forms"))?;
                let (field, view) = content::establish(chapter, authored)?;
                let regime = RegimeSpec::named(regime)?;
                let criteria = content
                    .chapters
                    .iter()
                    .map(|chapter| {
                        let (mut field, _) = content::establish(chapter, authored)?;
                        regime.apply(&mut field);
                        commissioning_criterion(&field)
                    })
                    .collect::<Result<Vec<_>, Fault>>()?;
                let scenario = ScenarioSpec::for_content(
                    content.hash.clone(),
                    content.pressures.clone(),
                    regime,
                    &content.chapters,
                    criteria,
                );
                let mut run = Run::start_with_scenario_kind(
                    run_id,
                    form,
                    scenario,
                    crate::state::RunKind::LegacyCampaign,
                )?;
                run.establish_field(field, view)
                    .map_err(|fault| read::recode(fault, Code::ContentInvalid))?;
                run.open_chapter(chapter, content.chapters.len());
                run.open_schedule(chapter);
                let state = run.state();
                self.store.note_run(
                    &state.run_id,
                    form,
                    state.progress.chapter_index,
                    state.now.step,
                    state.branch_nonce,
                );
                let answer = opened_body(&run, false, None);
                self.run = Some(run);
                self.drain_events();
                Ok(answer)
            }
            "restore" => {
                read::exact_keys(body, "body", &["mode", "save_key"])?;
                let key = read::text(body, "save_key")?.to_string();
                let (state, changed, migrated_from) = self.recorded(&key)?;
                let form = self.store.row(&state.run_id).map_or(String::new(), |row| row.form.clone());
                let run = Run::restore(state, &form)?;
                self.note(&run);
                let answer = opened_body(&run, changed, migrated_from);
                self.run = Some(run);
                self.reopen_chapter();
                self.drain_events();
                Ok(answer)
            }
            _ => Err(Fault::field("mode")),
        }
    }

    /// Restores from a checkpoint the run's own metadata names.
    ///
    /// Quick Retry keeps the branch nonce and the recorded random state
    /// exactly. Branch Recovery restores the same physical state, then takes
    /// the nonce one past the largest the run has recorded and re-roots the
    /// trajectory stream, so stochastic readings legitimately re-draw. Both
    /// land in `running` with the plan queue cleared, and both apply the one
    /// post-restore normalization.
    fn restore_by_id(&mut self, body: &Json, branching: bool) -> Result<String, Fault> {
        read::exact_keys(body, "body", &["anchor_id"])?;
        let anchor_id = read::int(body, "anchor_id", 0, i64::from(u32::MAX))? as u32;
        let run = self.run.as_ref().ok_or_else(|| state_fault(IDLE, LOADED))?;
        let Some(anchor) = run.anchor(anchor_id) else {
            let mut detail = String::new();
            let mut object = Obj::new(&mut detail);
            object.int("anchor_id", i64::from(anchor_id));
            object.text("quantity", "anchor");
            object.end();
            return Err(Fault::detailed(Code::NotFound, detail));
        };
        let key = anchor.save_key.clone();
        let form = run.form().to_string();
        let of_run = run.state().run_id.clone();

        // A checkpoint of one run never restores another. The run key is inside
        // the `save_key` already, and one session holds one run, so this cannot
        // be reached today; it is checked rather than relied on, because the
        // metadata travels in a payload that an import can bring from anywhere.
        if self.store.record(&key).map(|record| record.run_id.as_str()) != Some(of_run.as_str()) {
            let mut detail = String::new();
            let mut object = Obj::new(&mut detail);
            object.text("quantity", "save_key");
            object.text("save_key", &key);
            object.end();
            return Err(Fault::detailed(Code::NotFound, detail));
        }

        let (state, _, migrated_from) = self.recorded(&key)?;
        let mut restored = Run::restore(state, &form)?;
        if branching {
            let nonce = self.store.nonce_high(restored.state().run_id.as_str()).saturating_add(1);
            restored.rebranch(nonce)?;
        }
        self.note(&restored);
        let answer = restored_body(&restored, migrated_from);
        self.run = Some(restored);
        // A restore lands the run on a step it has already been at, and the
        // objective that stood there is the one the record carried. The shell
        // learns which the same way it learns it on any other reopening — the
        // chapter's own events — because a restore that moved the run
        // backwards would otherwise leave the surface showing an objective the
        // run no longer stands on.
        self.reopen_chapter();
        self.drain_events();
        Ok(answer)
    }

    /// Raises the chapter's own events on a run that was reopened, so a fresh
    /// worker session tells the shell what it holds. Authored pressure tables
    /// remain in the restored generator specification and are never rebound
    /// from the current bundle. A run whose content hash moved receives no new
    /// chapter events, but retains the frozen rules needed to replay its own
    /// recorded trajectory.
    fn reopen_chapter(&mut self) {
        let Some(chapter) = self.chapter().cloned() else {
            return;
        };
        let contract_run = self
            .run
            .as_ref()
            .is_some_and(|run| run.state().scenario.contract_id().is_some());
        let chapter_count = self.content.as_ref().map_or(0, |content| content.chapters.len());
        if let Some(run) = self.run.as_mut() {
            if !contract_run {
                run.open_chapter(&chapter, chapter_count);
            }
            run.open_schedule(&chapter);
        }
    }

    /// Writes every record the authored sequence asked for, each with the
    /// payload taken at the step that asked for it.
    fn store_pending(&mut self) {
        let Some(run) = self.run.as_mut() else {
            return;
        };
        let written = run.take_records();
        if written.is_empty() {
            return;
        }
        let run_id = run.state().run_id.clone();
        for (key, kind, payload) in written {
            let digest = hex_bytes(&sha256::digest(payload.as_bytes()));
            self.store.write(key, &run_id, kind, payload, digest);
        }
        self.note_loaded();
    }

    /// Takes the events the loaded run raised into the session's own queue, in
    /// the order their causes occurred.
    fn drain_events(&mut self) {
        let Some(run) = self.run.as_mut() else {
            return;
        };
        let raised = run.take_events();
        self.events.extend(raised);
    }

    fn note_loaded(&mut self) {
        let Some(run) = self.run.as_ref() else {
            return;
        };
        let state = run.state();
        let noted = (
            state.run_id.clone(),
            run.form().to_string(),
            state.progress.chapter_index,
            state.now.step,
            state.branch_nonce,
        );
        self.store.note_run(&noted.0, &noted.1, noted.2, noted.3, noted.4);
    }

    /// Reads one stored record back, in the locked load order: parse; the save
    /// version at most this build's; re-serialize through the core and compare
    /// bytes and digest; then schema, enum, range, and cap validation. Any
    /// failure marks the record invalid.
    fn recorded(&self, key: &str) -> Result<(RunState, bool, Option<i64>), Fault> {
        let Some(record) = self.store.record(key) else {
            let mut detail = String::new();
            let mut object = Obj::new(&mut detail);
            object.text("quantity", "save_key");
            object.text("save_key", key);
            object.end();
            return Err(Fault::detailed(Code::NotFound, detail));
        };
        if record.save_version > SAVE_VERSION {
            return Err(Fault::because(Code::SaveVersion, "record"));
        }
        let (state, changed) = read_payload(
            &record.payload,
            &record.payload_sha256,
            record.save_version,
            Code::SaveCorrupt,
            &self.content_hash(),
        )?;
        // Migration provenance is a boundary fact about how these bytes were
        // opened, never causal run state and never part of the rewritten V3
        // payload. V3 records therefore omit it entirely.
        let migrated_from = (record.save_version < SAVE_VERSION).then_some(record.save_version);
        Ok((state, changed, migrated_from))
    }

    /// Imports a run from an export file.
    ///
    /// The locked order: the file is canonical bytes or it is refused; `format`
    /// matches; `save_version` at most this build's, and above it the
    /// `save_version` envelope rather than a guess; the payload's digest is
    /// re-verified through the core; then schema and caps. The run then loads
    /// exactly — same branch nonce, same random state — and the imported
    /// payload is written into the older autosave slot.
    fn import_run(&mut self, body: &Json) -> Result<String, Fault> {
        read::exact_keys(body, "body", &["text"])?;
        let text = read::text(body, "text")?.to_string();
        // Canonicalization is implemented once, in the core, and an export file
        // is canonical bytes: parsing and re-emitting has to give back exactly
        // what arrived. Key order, spacing, a duplicate key, a float, and a
        // number written any other way all fail here.
        let rewritten = canonicalize(&text)
            .map_err(|reason| Fault::because(Code::ImportInvalid, reason))?;
        if rewritten != text {
            return Err(Fault::because(Code::ImportInvalid, "not_canonical"));
        }
        let file = parse(&text).map_err(|reason| Fault::because(Code::ImportInvalid, reason))?;
        read::exact_keys(&file, "text", &["format", "payload", "payload_sha256", "save_version"])
            .map_err(|fault| read::recode(fault, Code::ImportInvalid))?;
        let format = read::text(&file, "format")
            .map_err(|fault| read::recode(fault, Code::ImportInvalid))?;
        if format != EXPORT_FORMAT {
            return Err(Fault::because(Code::ImportInvalid, "format"));
        }
        let version = read::int(&file, "save_version", 0, i64::from(u32::MAX))
            .map_err(|fault| read::recode(fault, Code::ImportInvalid))?;
        if version > SAVE_VERSION {
            return Err(Fault::because(Code::SaveVersion, "import"));
        }
        if !matches!(version, 1 | 2 | 3 | 4 | 5 | 6) && version != SAVE_VERSION {
            return Err(Fault::because(Code::ImportInvalid, "save_version"));
        }
        let digest = read::hex(&file, "payload_sha256", 64)
            .map_err(|fault| read::recode(fault, Code::ImportInvalid))?
            .to_string();
        let payload = read::at(&file, "payload")
            .map_err(|fault| read::recode(fault, Code::ImportInvalid))?;
        let mut bytes = String::new();
        crate::json::write_value(&mut bytes, payload)
            .map_err(|reason| Fault::because(Code::ImportInvalid, reason))?;
        // The cap is stated over the payload, so the payload is what is
        // measured — the export file is that plus its fixed wrapper, and a
        // payload exactly at the cap arrives in a file a little larger.
        if bytes.len() > SAVE_PAYLOAD_CAP {
            return Err(crate::field::cap_fault("save_payload", SAVE_PAYLOAD_CAP as i64));
        }

        let (state, _) = read_payload(
            &bytes,
            &digest,
            version,
            Code::ImportInvalid,
            &self.content_hash(),
        )?;
        let form = state
            .now
            .forms
            .iter()
            .find(|held| held.controlled)
            .or_else(|| state.now.forms.first())
            .map_or(String::new(), |held| held.form.clone());
        let mut run = Run::restore(state, &form)?;
        let run_id = run.state().run_id.clone();

        // The imported payload lands under the derived autosave key for the
        // step it carries, and that key's metadata entry is rewritten in place
        // so it describes the payload now standing there. Nothing is appended:
        // an import adds no history the run did not have. What is stored is the
        // loaded run's own payload, so the bytes and the metadata beside them
        // always describe the same moment.
        let key = run.rewrite_checkpoint(RecordKind::Auto, auto_slot(run.step()));
        let stored = run.payload()?;
        let stored_digest = hex_bytes(&sha256::digest(stored.as_bytes()));
        self.note(&run);
        self.store.write(key, &run_id, RecordKind::Auto, stored, stored_digest);

        let mut out = String::new();
        let mut object = Obj::new(&mut out);
        write_run_identity(&mut object, run.state());
        object.text("embodied_state_hash", &run.state().now.embodied_hash());
        object.text(
            "generator_spec_hash",
            &run.state().scenario.generator().specification_hash(),
        );
        object.raw(
            "local_policy",
            &run.state().scenario.generator().local_policy().written(),
        );
        if version < SAVE_VERSION {
            object.int("migrated_from", version);
        }
        object.raw(
            "route_defaults",
            &run.state()
                .scenario
                .generator()
                .route_defaults_written(run.state().progress.chapter_index),
        );
        object.text("run_id", &run.state().run_id);
        object.text("scenario_hash", &run.state().scenario.scenario_hash());
        object.int("step", i64::from(run.state().now.step));
        object.raw("view", &run.state().view.written());
        object.end();
        self.run = Some(run);
        self.reopen_chapter();
        self.drain_events();
        Ok(out)
    }

    /// Opens one durable Archive export as a fresh branch while another run is
    /// loaded. The import boundary verifies the archived bytes unchanged, then
    /// this boundary advances the highest recorded nonce and re-roots only the
    /// trajectory stream. The durable source record is never rewritten.
    fn reopen_archive(&mut self, body: &Json) -> Result<String, Fault> {
        self.import_run(body)?;
        let run_id = self.loaded()?.state().run_id.clone();
        let nonce = self
            .store
            .nonce_high(&run_id)
            .checked_add(1)
            .ok_or_else(|| crate::field::cap_fault("branch_nonce", i64::from(u32::MAX)))?;
        self.loaded()?.rebranch(nonce)?;
        self.note_loaded();

        let run = self.loaded()?;
        let mut out = String::new();
        let mut object = Obj::new(&mut out);
        write_run_identity(&mut object, run.state());
        object.text("embodied_state_hash", &run.state().now.embodied_hash());
        object.text(
            "generator_spec_hash",
            &run.state().scenario.generator().specification_hash(),
        );
        object.text("run_id", &run.state().run_id);
        object.text("scenario_hash", &run.state().scenario.scenario_hash());
        object.int("step", i64::from(run.state().now.step));
        object.raw("view", &run.state().view.written());
        object.end();
        Ok(out)
    }

    /// Writes the autosave record the cadence is due, if one is: one `auto`
    /// record every 900 completed steps. The metadata rides in the payload, so
    /// it is noted before the bytes are taken.
    fn autosave(&mut self) {
        let Some(run) = self.run.as_ref() else {
            return;
        };
        let due = run.autosave_intervals();
        if due == 0 || due <= self.last_auto_interval() {
            return;
        }
        // Never inside Still Mode. A due autosave runs at the exit, which is
        // what the next frame after the ramp completes is; no run reaches
        // `still` yet, so this guard stands unexercised until the goal that
        // owns Still Mode makes it reachable.
        if run.mode() == Mode::Still {
            return;
        }
        let run_id = run.state().run_id.clone();
        let slot = auto_slot(run.step());

        // A payload past its cap is refused rather than stored, and the
        // metadata note is rolled back with it so the two never disagree.
        let written = {
            let run = self.run.as_mut().expect("a run is loaded");
            let held = run.state().anchors.clone();
            let key = run.note_checkpoint(RecordKind::Auto, slot);
            match run.payload() {
                Ok(payload) => Some((key, payload)),
                Err(_) => {
                    run.set_anchors(held);
                    None
                }
            }
        };
        let Some((key, payload)) = written else {
            return;
        };
        let digest = hex_bytes(&sha256::digest(payload.as_bytes()));
        self.store.write(key, &run_id, RecordKind::Auto, payload, digest);
        let noted = {
            let run = self.run.as_ref().expect("a run is loaded");
            let state = run.state();
            (
                run.form().to_string(),
                state.progress.chapter_index,
                state.now.step,
                state.branch_nonce,
            )
        };
        self.store.note_run(&run_id, &noted.0, noted.1, noted.2, noted.3);
    }

    /// The autosave interval the newest `auto` record of the loaded run stands
    /// at, and 0 when none stands.
    fn last_auto_interval(&self) -> u32 {
        let Some(run) = self.run.as_ref() else {
            return 0;
        };
        run.state()
            .anchors
            .iter()
            .filter(|anchor| anchor.kind == RecordKind::Auto)
            .map(|anchor| anchor.step / crate::state::AUTOSAVE_STEPS)
            .max()
            .unwrap_or(0)
    }

    /// Refreshes the run's metadata row, which is what raises `nonce_high`.
    fn note(&mut self, run: &Run) {
        let state = run.state();
        self.store.note_run(
            &state.run_id,
            run.form(),
            state.progress.chapter_index,
            state.now.step,
            state.branch_nonce,
        );
    }

    fn loaded(&mut self) -> Result<&mut Run, Fault> {
        self.run.as_mut().ok_or_else(|| state_fault(IDLE, LOADED))
    }

    /// The loaded run, for a caller that reads its state directly rather than
    /// through a command.
    pub fn run(&self) -> Option<&Run> {
        self.run.as_ref()
    }

    /// The session's records, for a caller reading the store directly.
    pub fn store(&self) -> &RecordStore {
        &self.store
    }

    /// The render snapshot for the most recent step, as locked bytes. A
    /// session holding no run has no frame to hand out.
    pub fn frame_view(&self) -> Vec<u8> {
        self.run.as_ref().map(Run::frame_view).unwrap_or_default()
    }

    /// The events raised since the last call, canonical JSON, in the order
    /// their causes occurred. The campaign raises four of the seven —
    /// `objective_changed`, `checkpoint_written`, `chapter_changed`, and
    /// `run_completed` — the pressure system raises `pressure_changed`, the
    /// slate, Echo, and inspections raise `review_ready`, and the frame event
    /// is the worker's own.
    pub fn take_events(&mut self) -> String {
        let raised = std::mem::take(&mut self.events);
        format!("[{}]", raised.join(","))
    }
}

/// Reads one save payload back and holds it to the locked load order's last
/// two steps: the digest of exactly these bytes, then schema and caps. The
/// caller names the envelope a failure answers with.
fn read_payload(
    bytes: &str,
    digest: &str,
    declared_version: i64,
    code: Code,
    hash: &str,
) -> Result<(RunState, bool), Fault> {
    if hex_bytes(&sha256::digest(bytes.as_bytes())) != digest {
        return Err(Fault::because(code, "payload_sha256"));
    }
    let payload = parse(bytes).map_err(|reason| Fault::because(code, reason))?;
    let version = read::int(&payload, "save_version", 0, i64::from(u32::MAX))
        .map_err(|fault| read::recode(fault, code))?;
    if version != declared_version {
        return Err(Fault::because(code, "save_version"));
    }
    let state = match version {
        1 => RunState::migrate_v1(&payload),
        2 => RunState::migrate_v2(&payload),
        3 => RunState::migrate_v3(&payload),
        4 => RunState::migrate_v4(&payload),
        5 => RunState::migrate_v5(&payload),
        6 => RunState::migrate_v6(&payload),
        SAVE_VERSION => RunState::read(&payload),
        _ => Err(Fault::because(Code::SaveVersion, "payload")),
    }
    .map_err(|fault| read::recode(fault, code))?;
    state.coherent().map_err(|fault| read::recode(fault, code))?;
    // A restore under a different content hash continues, and says so; the
    // framework reproducibility of pre-restore records is no longer claimed.
    let changed = state.scenario.content_hash() != hash;
    Ok((state, changed))
}

/// The first commissioning contract: all authored Routes must sustain a shared
/// quarter-capacity floor, every charged non-Form Component in the physical
/// compartment must retain a quarter of its opening stock, leakage may consume
/// at most one eighth of accepted Supply, and the complete condition must stand
/// hands-off for three seconds after a one-second grace interval.
fn commissioning_criterion(
    field: &crate::state::FieldState,
) -> Result<Option<crate::criterion::FunctionCriterionSpec>, Fault> {
    let mut route_ids: Vec<u32> = field.routes.iter().map(|route| route.route).collect();
    route_ids.sort_unstable();
    let Some(route_floor) = field
        .routes
        .iter()
        .map(|route| route.capacity / 4)
        .min()
        .filter(|floor| *floor > 0)
    else {
        return Ok(None);
    };

    let mut components = Vec::new();
    for node in &field.physical_compartment.members {
        let Some(port) = field.ports.iter().find(|port| {
            port.node == *node && port.kind != crate::field::NodeKind::Form && port.q > 0
        }) else {
            continue;
        };
        components.push(crate::criterion::ComponentRequirement::new(
            port.node,
            (port.q / 4).max(1),
        )?);
    }
    components.sort_by_key(|component| component.node());
    if components.is_empty() {
        return Ok(None);
    }

    Ok(Some(crate::criterion::FunctionCriterionSpec::new(
        route_ids,
        route_floor,
        components,
        8_192,
        90,
        30,
        90,
    )?))
}

fn write_run_identity_prefix(object: &mut Obj<'_>, state: &RunState) {
    match state.scenario.assembly_template() {
        Some(template) => {
            object.bool("assembly_template_exact", template.is_exact());
            object.text("assembly_template_hash", template.hash());
        }
        None => {
            object.bool("assembly_template_exact", false);
            object.null("assembly_template_hash");
        }
    }
    match &state.attempt_branch {
        Some(branch) => object.raw("attempt_branch", &branch.written()),
        None => object.null("attempt_branch"),
    };
    match &state.attempt {
        Some(attempt) => {
            object.text("attempt_id", attempt.attempt_id());
            object.raw("attempt_record", &attempt.written());
        }
        None => {
            object.null("attempt_id");
            object.null("attempt_record");
        }
    };
    match &state.attempt_branch {
        Some(branch) => {
            object.text("branch_id", branch.branch_id());
            object.int("branch_nonce", i64::from(state.branch_nonce));
            object.text("branch_operation", branch.operation().name());
        }
        None => {
            object.null("branch_id");
            object.int("branch_nonce", i64::from(state.branch_nonce));
            object.null("branch_operation");
        }
    }
}

fn write_run_identity_parent(object: &mut Obj<'_>, state: &RunState) {
    match state
        .attempt_branch
        .as_ref()
        .and_then(crate::state::AttemptBranchRecord::parent_branch_id)
    {
        Some(parent) => object.text("parent_branch_id", parent),
        None => object.null("parent_branch_id"),
    };
}

fn write_run_identity_kind(object: &mut Obj<'_>, state: &RunState) {
    object.text("run_kind", state.run_kind.name());
}

/// The complete flat run-lineage projection in canonical key order.
fn write_run_identity(object: &mut Obj<'_>, state: &RunState) {
    write_run_identity_prefix(object, state);
    write_run_identity_parent(object, state);
    write_run_identity_kind(object, state);
}

fn opened_body(run: &Run, content_changed: bool, migrated_from: Option<i64>) -> String {
    let state = run.state();
    let mut out = String::new();
    let mut object = Obj::new(&mut out);
    write_run_identity(&mut object, state);
    object.int("chapter_index", i64::from(state.progress.chapter_index));
    object.bool("content_changed", content_changed);
    object.text("content_hash", state.scenario.content_hash());
    match state.scenario.contract_id() {
        Some(id) => object.text("contract_id", id),
        None => object.null("contract_id"),
    };
    object.text("embodied_state_hash", &state.now.embodied_hash());
    object.text(
        "generator_spec_hash",
        &state.scenario.generator().specification_hash(),
    );
    object.raw(
        "local_policy",
        &state.scenario.generator().local_policy().written(),
    );
    if let Some(version) = migrated_from {
        object.int("migrated_from", version);
    }
    object.int("protocol", i64::from(PROTOCOL_VERSION));
    match &state.qualification_request {
        Some(request) => {
            object.raw("qualification_request", &request.written());
            object.text("qualification_request_id", request.request_id());
        }
        None => {
            object.null("qualification_request");
            object.null("qualification_request_id");
        }
    };
    object.text("regime", state.scenario.regime().id());
    object.raw(
        "route_defaults",
        &state
            .scenario
            .generator()
            .route_defaults_written(state.progress.chapter_index),
    );
    object.text("run_id", &state.run_id);
    object.int("save_version", SAVE_VERSION);
    object.text("scenario_hash", &state.scenario.scenario_hash());
    object.int("step", i64::from(state.now.step));
    object.raw("view", &state.view.written());
    object.end();
    out
}

/// The answer both restores return.
fn restored_body(run: &Run, migrated_from: Option<i64>) -> String {
    let state = run.state();
    let mut out = String::new();
    let mut object = Obj::new(&mut out);
    write_run_identity(&mut object, state);
    match state.scenario.contract_id() {
        Some(id) => object.text("contract_id", id),
        None => object.null("contract_id"),
    };
    object.text("embodied_state_hash", &state.now.embodied_hash());
    object.text(
        "generator_spec_hash",
        &state.scenario.generator().specification_hash(),
    );
    object.raw(
        "local_policy",
        &state.scenario.generator().local_policy().written(),
    );
    if let Some(version) = migrated_from {
        object.int("migrated_from", version);
    }
    match &state.qualification_request {
        Some(request) => {
            object.raw("qualification_request", &request.written());
            object.text("qualification_request_id", request.request_id());
        }
        None => {
            object.null("qualification_request");
            object.null("qualification_request_id");
        }
    };
    object.raw(
        "route_defaults",
        &state
            .scenario
            .generator()
            .route_defaults_written(state.progress.chapter_index),
    );
    object.text("scenario_hash", &state.scenario.scenario_hash());
    object.int("step", i64::from(state.now.step));
    object.raw("view", &state.view.written());
    object.end();
    out
}
