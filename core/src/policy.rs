//! Frozen local automation policy and deterministic decision projection.
//!
//! A policy is immutable generator data. Evaluation receives only causal Field
//! state local to the addressed Component and returns an action proposal; the
//! Field module owns application because it owns movement, Coupling, Route
//! allocation, signals, and the conserving ledger.

use crate::fault::Fault;
use crate::field::{current_emitting, CurrentState, NodeKind, PLANE_SPAN};
use crate::fx::{distance, within, within_segment, Vec2};
use crate::json::{Json, Obj};
use crate::read;
use crate::state::{FieldState, Frac, Fx, Step, FRAC_ONE};

pub const POLICY_VERSION: u16 = 2;
const POLICY_VERSION_OLDEST: u16 = 1;
pub const POLICY_RULES_PER_COMPONENT: usize = 8;
pub const POLICY_COMPONENTS_PER_GENERATOR: usize = crate::field::NODES_PER_RUN;
pub const POLICY_SENSOR_RADIUS_MAX: Fx = PLANE_SPAN;

/// The condition vocabulary contract content may unlock. These are the same
/// machine ids accepted by [`LocalCondition::read`]; content never gains a
/// second string-to-behavior table.
pub const POLICY_CONDITION_KINDS: [&str; 11] = [
    "always",
    "charge_below",
    "charge_above",
    "operating_margin_below",
    "supply",
    "target_in_range",
    "route_flow_below",
    "route_flow_above",
    "overloaded",
    "signal_present",
    "timer_elapsed",
];

/// The action vocabulary contract content may unlock. These are the same
/// machine ids accepted by [`LocalAction::read`].
pub const POLICY_ACTION_KINDS: [&str; 10] = [
    "hold",
    "seek_supply",
    "seek_port",
    "seek_signal",
    "change_depth",
    "couple",
    "set_interface",
    "set_route",
    "emit_signal",
    "use_ability",
];

pub fn condition_kind_known(kind: &str) -> bool {
    POLICY_CONDITION_KINDS.contains(&kind)
}

pub fn action_kind_known(kind: &str) -> bool {
    POLICY_ACTION_KINDS.contains(&kind)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SupplySense {
    Absent,
    Present,
    Emitting,
    Quiet,
}

impl SupplySense {
    fn name(self) -> &'static str {
        match self {
            SupplySense::Absent => "absent",
            SupplySense::Present => "present",
            SupplySense::Emitting => "emitting",
            SupplySense::Quiet => "quiet",
        }
    }

    fn read(value: &Json, key: &str) -> Result<Self, Fault> {
        Ok(match read::one_of(value, key, &["absent", "present", "emitting", "quiet"])? {
            0 => SupplySense::Absent,
            1 => SupplySense::Present,
            2 => SupplySense::Emitting,
            _ => SupplySense::Quiet,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LocalCondition {
    Always,
    ChargeBelow { fraction: Frac },
    ChargeAbove { fraction: Frac },
    OperatingMarginBelow { amount: Fx },
    Supply { state: SupplySense, radius: Fx },
    TargetInRange { radius: Fx },
    RouteFlowBelow { route: u32, flow: Fx },
    RouteFlowAbove { route: u32, flow: Fx },
    Overloaded,
    SignalPresent { radius: Fx },
    TimerElapsed { steps: Step },
}

impl LocalCondition {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Always => "always",
            Self::ChargeBelow { .. } => "charge_below",
            Self::ChargeAbove { .. } => "charge_above",
            Self::OperatingMarginBelow { .. } => "operating_margin_below",
            Self::Supply { .. } => "supply",
            Self::TargetInRange { .. } => "target_in_range",
            Self::RouteFlowBelow { .. } => "route_flow_below",
            Self::RouteFlowAbove { .. } => "route_flow_above",
            Self::Overloaded => "overloaded",
            Self::SignalPresent { .. } => "signal_present",
            Self::TimerElapsed { .. } => "timer_elapsed",
        }
    }

    pub fn read(value: &Json) -> Result<Self, Fault> {
        if !value.is_map() {
            return Err(Fault::field("condition"));
        }
        let kind = read::text(value, "kind")?;
        match kind {
            "always" => {
                read::exact_keys(value, "condition", &["kind"])?;
                Ok(Self::Always)
            }
            "charge_below" | "charge_above" => {
                read::exact_keys(value, "condition", &["fraction", "kind"])?;
                let fraction = read::int(value, "fraction", 0, FRAC_ONE)?;
                Ok(if kind == "charge_below" {
                    Self::ChargeBelow { fraction }
                } else {
                    Self::ChargeAbove { fraction }
                })
            }
            "operating_margin_below" => {
                read::exact_keys(value, "condition", &["amount", "kind"])?;
                Ok(Self::OperatingMarginBelow {
                    amount: read::int(value, "amount", 0, crate::fx::STORED_BOUND - 1)?,
                })
            }
            "supply" => {
                read::exact_keys(value, "condition", &["kind", "radius", "state"])?;
                Ok(Self::Supply {
                    state: SupplySense::read(value, "state")?,
                    radius: read::int(value, "radius", 0, POLICY_SENSOR_RADIUS_MAX)?,
                })
            }
            "target_in_range" => {
                read::exact_keys(value, "condition", &["kind", "radius"])?;
                Ok(Self::TargetInRange {
                    radius: read::int(value, "radius", 0, POLICY_SENSOR_RADIUS_MAX)?,
                })
            }
            "route_flow_below" | "route_flow_above" => {
                read::exact_keys(value, "condition", &["flow", "kind", "route"])?;
                let route = read::int(value, "route", 1, i64::from(u32::MAX))? as u32;
                let flow = read::int(value, "flow", 0, crate::field::ROUTE_CAPACITY_CAP)?;
                Ok(if kind == "route_flow_below" {
                    Self::RouteFlowBelow { route, flow }
                } else {
                    Self::RouteFlowAbove { route, flow }
                })
            }
            "overloaded" => {
                read::exact_keys(value, "condition", &["kind"])?;
                Ok(Self::Overloaded)
            }
            "signal_present" => {
                read::exact_keys(value, "condition", &["kind", "radius"])?;
                Ok(Self::SignalPresent {
                    radius: read::int(value, "radius", 0, POLICY_SENSOR_RADIUS_MAX)?,
                })
            }
            "timer_elapsed" => {
                read::exact_keys(value, "condition", &["kind", "steps"])?;
                Ok(Self::TimerElapsed {
                    steps: read::int(value, "steps", 1, i64::from(u16::MAX))? as Step,
                })
            }
            _ => Err(Fault::field("kind")),
        }
    }

    pub fn written(&self) -> String {
        let mut out = String::new();
        let mut object = Obj::new(&mut out);
        match self {
            Self::Always => {
                object.text("kind", "always");
            }
            Self::ChargeBelow { fraction } => {
                object.int("fraction", *fraction);
                object.text("kind", "charge_below");
            }
            Self::ChargeAbove { fraction } => {
                object.int("fraction", *fraction);
                object.text("kind", "charge_above");
            }
            Self::OperatingMarginBelow { amount } => {
                object.int("amount", *amount);
                object.text("kind", "operating_margin_below");
            }
            Self::Supply { state, radius } => {
                object.text("kind", "supply");
                object.int("radius", *radius);
                object.text("state", state.name());
            }
            Self::TargetInRange { radius } => {
                object.text("kind", "target_in_range");
                object.int("radius", *radius);
            }
            Self::RouteFlowBelow { route, flow } => {
                object.int("flow", *flow);
                object.text("kind", "route_flow_below");
                object.int("route", i64::from(*route));
            }
            Self::RouteFlowAbove { route, flow } => {
                object.int("flow", *flow);
                object.text("kind", "route_flow_above");
                object.int("route", i64::from(*route));
            }
            Self::Overloaded => {
                object.text("kind", "overloaded");
            }
            Self::SignalPresent { radius } => {
                object.text("kind", "signal_present");
                object.int("radius", *radius);
            }
            Self::TimerElapsed { steps } => {
                object.text("kind", "timer_elapsed");
                object.int("steps", i64::from(*steps));
            }
        }
        object.end();
        out
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LocalAction {
    Hold,
    SeekSupply { radius: Fx },
    SeekPort { radius: Fx },
    SeekSignal { radius: Fx },
    ChangeDepth { direction: i8 },
    Couple { radius: Fx },
    SetInterface { open: bool },
    SetRoute {
        route: u32,
        enabled: bool,
        capacity_limit: Fx,
        allocation_weight: u16,
    },
    EmitSignal { strength: Fx },
    UseAbility,
}

impl LocalAction {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Hold => "hold",
            Self::SeekSupply { .. } => "seek_supply",
            Self::SeekPort { .. } => "seek_port",
            Self::SeekSignal { .. } => "seek_signal",
            Self::ChangeDepth { .. } => "change_depth",
            Self::Couple { .. } => "couple",
            Self::SetInterface { .. } => "set_interface",
            Self::SetRoute { .. } => "set_route",
            Self::EmitSignal { .. } => "emit_signal",
            Self::UseAbility => "use_ability",
        }
    }

    pub fn read(value: &Json) -> Result<Self, Fault> {
        if !value.is_map() {
            return Err(Fault::field("action"));
        }
        let kind = read::text(value, "kind")?;
        match kind {
            "hold" => {
                read::exact_keys(value, "action", &["kind"])?;
                Ok(Self::Hold)
            }
            "seek_supply" | "seek_port" | "seek_signal" | "couple" => {
                read::exact_keys(value, "action", &["kind", "radius"])?;
                let radius = read::int(value, "radius", 0, POLICY_SENSOR_RADIUS_MAX)?;
                Ok(match kind {
                    "seek_supply" => Self::SeekSupply { radius },
                    "seek_port" => Self::SeekPort { radius },
                    "seek_signal" => Self::SeekSignal { radius },
                    _ => Self::Couple { radius },
                })
            }
            "change_depth" => {
                read::exact_keys(value, "action", &["direction", "kind"])?;
                Ok(Self::ChangeDepth {
                    direction: read::int(value, "direction", -1, 1)? as i8,
                })
            }
            "set_interface" => {
                read::exact_keys(value, "action", &["kind", "open"])?;
                Ok(Self::SetInterface { open: read::flag(value, "open")? })
            }
            "set_route" => {
                read::exact_keys(
                    value,
                    "action",
                    &["allocation_weight", "capacity_limit", "enabled", "kind", "route"],
                )?;
                Ok(Self::SetRoute {
                    route: read::int(value, "route", 1, i64::from(u32::MAX))? as u32,
                    enabled: read::flag(value, "enabled")?,
                    capacity_limit: read::int(
                        value,
                        "capacity_limit",
                        0,
                        crate::field::ROUTE_CAPACITY_CAP,
                    )?,
                    allocation_weight: read::int(
                        value,
                        "allocation_weight",
                        1,
                        i64::from(u16::MAX),
                    )? as u16,
                })
            }
            "emit_signal" => {
                read::exact_keys(value, "action", &["kind", "strength"])?;
                Ok(Self::EmitSignal {
                    strength: read::int(value, "strength", 1, crate::fx::STORED_BOUND - 1)?,
                })
            }
            "use_ability" => {
                read::exact_keys(value, "action", &["kind"])?;
                Ok(Self::UseAbility)
            }
            _ => Err(Fault::field("kind")),
        }
    }

    pub fn written(&self) -> String {
        let mut out = String::new();
        let mut object = Obj::new(&mut out);
        match self {
            Self::Hold => {
                object.text("kind", "hold");
            }
            Self::SeekSupply { radius } => {
                object.text("kind", "seek_supply");
                object.int("radius", *radius);
            }
            Self::SeekPort { radius } => {
                object.text("kind", "seek_port");
                object.int("radius", *radius);
            }
            Self::SeekSignal { radius } => {
                object.text("kind", "seek_signal");
                object.int("radius", *radius);
            }
            Self::ChangeDepth { direction } => {
                object.int("direction", i64::from(*direction));
                object.text("kind", "change_depth");
            }
            Self::Couple { radius } => {
                object.text("kind", "couple");
                object.int("radius", *radius);
            }
            Self::SetInterface { open } => {
                object.text("kind", "set_interface");
                object.bool("open", *open);
            }
            Self::SetRoute {
                route,
                enabled,
                capacity_limit,
                allocation_weight,
            } => {
                object.int("allocation_weight", i64::from(*allocation_weight));
                object.int("capacity_limit", *capacity_limit);
                object.bool("enabled", *enabled);
                object.text("kind", "set_route");
                object.int("route", i64::from(*route));
            }
            Self::EmitSignal { strength } => {
                object.text("kind", "emit_signal");
                object.int("strength", *strength);
            }
            Self::UseAbility => {
                object.text("kind", "use_ability");
            }
        }
        object.end();
        out
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PolicyRule {
    pub enabled: bool,
    pub condition: LocalCondition,
    pub action: LocalAction,
}

impl PolicyRule {
    fn read(value: &Json) -> Result<Self, Fault> {
        let enabled = if value.get("enabled").is_some() {
            read::exact_keys(value, "rules", &["action", "condition", "enabled"])?;
            read::flag(value, "enabled")?
        } else {
            read::exact_keys(value, "rules", &["action", "condition"])?;
            true
        };
        Ok(Self {
            enabled,
            condition: LocalCondition::read(read::at(value, "condition")?)?,
            action: LocalAction::read(read::at(value, "action")?)?,
        })
    }

    fn written(&self) -> String {
        let mut out = String::new();
        let mut object = Obj::new(&mut out);
        object.raw("action", &self.action.written());
        object.raw("condition", &self.condition.written());
        object.bool("enabled", self.enabled);
        object.end();
        out
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComponentPolicy {
    pub address: u32,
    pub rules: Vec<PolicyRule>,
    pub fallback: LocalAction,
}

impl ComponentPolicy {
    fn read(value: &Json) -> Result<Self, Fault> {
        read::exact_keys(value, "components", &["address", "fallback", "rules"])?;
        let mut rules = Vec::new();
        for entry in read::list(value, "rules", POLICY_RULES_PER_COMPONENT)? {
            rules.push(PolicyRule::read(entry)?);
        }
        Ok(Self {
            address: read::int(value, "address", 1, i64::from(u32::MAX))? as u32,
            rules,
            fallback: LocalAction::read(read::at(value, "fallback")?)?,
        })
    }

    fn written(&self) -> String {
        let mut out = String::new();
        let mut object = Obj::new(&mut out);
        object.int("address", i64::from(self.address));
        object.raw("fallback", &self.fallback.written());
        {
            let mut rules = object.list("rules");
            for rule in &self.rules {
                rules.raw(&rule.written());
            }
            rules.end();
        }
        object.end();
        out
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrozenLocalPolicy {
    version: u16,
    components: Vec<ComponentPolicy>,
}

impl FrozenLocalPolicy {
    pub fn empty() -> Self {
        Self { version: POLICY_VERSION, components: Vec::new() }
    }

    pub fn new(mut components: Vec<ComponentPolicy>) -> Result<Self, Fault> {
        components.sort_by_key(|component| component.address);
        let policy = Self { version: POLICY_VERSION, components };
        policy.coherent()?;
        Ok(policy)
    }

    pub fn components(&self) -> &[ComponentPolicy] {
        &self.components
    }

    pub fn is_empty(&self) -> bool {
        self.components.is_empty()
    }

    pub fn permitted_by(
        &self,
        actions: &[String],
        conditions: &[String],
        max_rules_per_component: usize,
    ) -> bool {
        self.components.iter().all(|component| {
            component.rules.len() <= max_rules_per_component
                && actions.iter().any(|kind| kind == component.fallback.name())
                && component.rules.iter().all(|rule| {
                    conditions.iter().any(|kind| kind == rule.condition.name())
                        && actions.iter().any(|kind| kind == rule.action.name())
                })
        })
    }

    pub fn read(value: &Json, key: &str) -> Result<Self, Fault> {
        let found = read::map(value, key)?;
        read::exact_keys(found, key, &["components", "version"])?;
        let mut components = Vec::new();
        for entry in read::list(found, "components", POLICY_COMPONENTS_PER_GENERATOR)? {
            components.push(ComponentPolicy::read(entry)?);
        }
        read::int(
            found,
            "version",
            i64::from(POLICY_VERSION_OLDEST),
            i64::from(POLICY_VERSION),
        )?;
        let policy = Self { version: POLICY_VERSION, components };
        policy.coherent()?;
        Ok(policy)
    }

    pub fn written(&self) -> String {
        let mut out = String::new();
        let mut object = Obj::new(&mut out);
        {
            let mut components = object.list("components");
            for component in &self.components {
                components.raw(&component.written());
            }
            components.end();
        }
        object.int("version", i64::from(self.version));
        object.end();
        out
    }

    pub fn coherent(&self) -> Result<(), Fault> {
        if self.version != POLICY_VERSION || self.components.len() > POLICY_COMPONENTS_PER_GENERATOR {
            return Err(Fault::field("local_policy"));
        }
        let addresses: Vec<u32> = self.components.iter().map(|component| component.address).collect();
        if !read::ascending(&addresses)
            || self.components.iter().any(|component| {
                component.address == 0 || component.rules.len() > POLICY_RULES_PER_COMPONENT
            })
        {
            return Err(Fault::field("local_policy"));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PolicyTarget {
    None,
    Node(u32),
    Route(u32),
    Current(u16),
    Signal(u32),
}

impl PolicyTarget {
    pub(crate) fn parts(self) -> (&'static str, Option<i64>) {
        match self {
            Self::None => ("none", None),
            Self::Node(id) => ("node", Some(i64::from(id))),
            Self::Route(id) => ("route", Some(i64::from(id))),
            Self::Current(id) => ("current", Some(i64::from(id))),
            Self::Signal(id) => ("signal", Some(i64::from(id))),
        }
    }

    fn read(kind: &str, id: Option<i64>) -> Result<Self, Fault> {
        match (kind, id) {
            ("none", None) => Ok(Self::None),
            ("node", Some(id)) if (1..=i64::from(u32::MAX)).contains(&id) => {
                Ok(Self::Node(id as u32))
            }
            ("route", Some(id)) if (1..=i64::from(u32::MAX)).contains(&id) => {
                Ok(Self::Route(id as u32))
            }
            ("current", Some(id)) if (0..=i64::from(u16::MAX)).contains(&id) => {
                Ok(Self::Current(id as u16))
            }
            ("signal", Some(id)) if (1..=i64::from(u32::MAX)).contains(&id) => {
                Ok(Self::Signal(id as u32))
            }
            _ => Err(Fault::field("target")),
        }
    }
}

/// The exact physical result of the last selected local action.
///
/// These values distinguish a policy that selected no target, lost its target
/// before application, reached a layer or range boundary, or simply requested
/// an already-standing state. Inspection reads this retained result; it never
/// guesses from the next step's conditions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PolicyOutcome {
    Idle,
    Held,
    Applied,
    NoTarget,
    TargetUnavailable,
    WrongLayer,
    OutOfRange,
    NoEffect,
    Cooldown,
    CapacityReached,
    Unavailable,
}

impl PolicyOutcome {
    pub fn name(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Held => "held",
            Self::Applied => "applied",
            Self::NoTarget => "no_target",
            Self::TargetUnavailable => "target_unavailable",
            Self::WrongLayer => "wrong_layer",
            Self::OutOfRange => "out_of_range",
            Self::NoEffect => "no_effect",
            Self::Cooldown => "cooldown",
            Self::CapacityReached => "capacity_reached",
            Self::Unavailable => "unavailable",
        }
    }

    fn read(value: &Json, key: &str) -> Result<Self, Fault> {
        Ok(match read::one_of(
            value,
            key,
            &[
                "idle",
                "held",
                "applied",
                "no_target",
                "target_unavailable",
                "wrong_layer",
                "out_of_range",
                "no_effect",
                "cooldown",
                "capacity_reached",
                "unavailable",
            ],
        )? {
            0 => Self::Idle,
            1 => Self::Held,
            2 => Self::Applied,
            3 => Self::NoTarget,
            4 => Self::TargetUnavailable,
            5 => Self::WrongLayer,
            6 => Self::OutOfRange,
            7 => Self::NoEffect,
            8 => Self::Cooldown,
            9 => Self::CapacityReached,
            _ => Self::Unavailable,
        })
    }
}

/// Embodied execution state for one addressed Component.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PolicyRuntimeState {
    pub address: u32,
    pub active_action: Option<LocalAction>,
    /// Zero-based rule, or -1 when the fallback action was selected.
    pub active_rule: i16,
    pub outcome: PolicyOutcome,
    pub target: PolicyTarget,
    pub timer: Step,
    pub cooldown: u16,
}

impl PolicyRuntimeState {
    pub fn opening(address: u32) -> Self {
        Self {
            address,
            active_action: None,
            active_rule: -1,
            outcome: PolicyOutcome::Idle,
            target: PolicyTarget::None,
            timer: 0,
            cooldown: 0,
        }
    }

    pub fn read(value: &Json) -> Result<Self, Fault> {
        let expanded = value.get("active_action").is_some() || value.get("outcome").is_some();
        read::exact_keys(
            value,
            "policy_runtime",
            if expanded {
                &[
                    "active_action",
                    "active_rule",
                    "address",
                    "cooldown",
                    "outcome",
                    "target",
                    "target_kind",
                    "timer",
                ]
            } else {
                &["active_rule", "address", "cooldown", "target", "target_kind", "timer"]
            },
        )?;
        let target = match value.get("target") {
            Some(Json::Null) => None,
            Some(Json::Int(id)) => Some(*id),
            _ => return Err(Fault::field("target")),
        };
        let active_action = if expanded {
            match value.get("active_action") {
                Some(Json::Null) => None,
                Some(action) => Some(LocalAction::read(action)?),
                None => return Err(Fault::field("active_action")),
            }
        } else {
            None
        };
        Ok(Self {
            active_action,
            active_rule: read::int(value, "active_rule", -1, 7)? as i16,
            address: read::int(value, "address", 1, i64::from(u32::MAX))? as u32,
            cooldown: read::int(value, "cooldown", 0, i64::from(u16::MAX))? as u16,
            outcome: if expanded {
                PolicyOutcome::read(value, "outcome")?
            } else {
                PolicyOutcome::Idle
            },
            target: PolicyTarget::read(read::text(value, "target_kind")?, target)?,
            timer: read::int(value, "timer", 0, i64::from(u32::MAX))? as Step,
        })
    }

    pub fn written(&self) -> String {
        let mut out = String::new();
        let mut object = Obj::new(&mut out);
        let (target_kind, target) = self.target.parts();
        match &self.active_action {
            Some(action) => {
                object.raw("active_action", &action.written());
            }
            None => {
                object.null("active_action");
            }
        }
        object.int("active_rule", i64::from(self.active_rule));
        object.int("address", i64::from(self.address));
        object.int("cooldown", i64::from(self.cooldown));
        object.text("outcome", self.outcome.name());
        object.int_or_null("target", target);
        object.text("target_kind", target_kind);
        object.int("timer", i64::from(self.timer));
        object.end();
        out
    }
}

/// Embodied actuator state for one directed Route.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RouteControlState {
    pub route: u32,
    pub enabled: bool,
    pub capacity_limit: Fx,
    pub allocation_weight: u16,
    pub controller: u32,
}

impl RouteControlState {
    pub fn opening(route: u32, controller: u32, capacity_limit: Fx) -> Self {
        Self { route, enabled: true, capacity_limit, allocation_weight: 1, controller }
    }

    pub fn read(value: &Json) -> Result<Self, Fault> {
        read::exact_keys(
            value,
            "route_controls",
            &["allocation_weight", "capacity_limit", "controller", "enabled", "route"],
        )?;
        Ok(Self {
            allocation_weight: read::int(value, "allocation_weight", 1, i64::from(u16::MAX))?
                as u16,
            capacity_limit: read::int(value, "capacity_limit", 0, crate::field::ROUTE_CAPACITY_CAP)?,
            controller: read::int(value, "controller", 1, i64::from(u32::MAX))? as u32,
            enabled: read::flag(value, "enabled")?,
            route: read::int(value, "route", 1, i64::from(u32::MAX))? as u32,
        })
    }

    pub fn written(&self) -> String {
        let mut out = String::new();
        let mut object = Obj::new(&mut out);
        object.int("allocation_weight", i64::from(self.allocation_weight));
        object.int("capacity_limit", self.capacity_limit);
        object.int("controller", i64::from(self.controller));
        object.bool("enabled", self.enabled);
        object.int("route", i64::from(self.route));
        object.end();
        out
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PolicyDecision {
    pub address: u32,
    pub rule: i16,
    pub action: LocalAction,
    pub target: PolicyTarget,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PolicyPreviewCandidate {
    pub target: PolicyTarget,
    pub distance: Fx,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PolicyPreview {
    pub address: u32,
    pub rule: i16,
    pub condition: Option<LocalCondition>,
    pub action: LocalAction,
    pub target: PolicyTarget,
    pub sensor_radius: Fx,
    pub action_radius: Fx,
    pub candidates: Vec<PolicyPreviewCandidate>,
}

impl PolicyPreview {
    pub fn written(&self) -> String {
        let mut out = String::new();
        let mut object = Obj::new(&mut out);
        object.raw("action", &self.action.written());
        object.int("action_radius", self.action_radius);
        object.int("address", i64::from(self.address));
        {
            let mut candidates = object.list("candidates");
            for candidate in &self.candidates {
                let (kind, id) = candidate.target.parts();
                let mut held = candidates.object();
                held.int("distance", candidate.distance);
                held.int_or_null("id", id);
                held.text("kind", kind);
                held.end();
            }
            candidates.end();
        }
        match &self.condition {
            Some(condition) => object.raw("condition", &condition.written()),
            None => object.null("condition"),
        };
        object.int("rule", i64::from(self.rule));
        object.int("sensor_radius", self.sensor_radius);
        let (target_kind, target) = self.target.parts();
        object.int_or_null("target", target);
        object.text("target_kind", target_kind);
        object.end();
        out
    }
}

fn component_position(field: &FieldState, address: u32) -> Option<(Vec2, u8)> {
    field
        .ports
        .iter()
        .find(|port| port.node == address)
        .map(|port| (port.pos, port.layer))
}

fn current_is_local(current: &CurrentState, pos: Vec2, layer: u8, radius: Fx) -> bool {
    if current.layer != layer {
        return false;
    }
    current.path.windows(2).any(|segment| {
        within_segment(pos, segment[0], segment[1], radius.saturating_add(current.width))
    }) || current.path.first().is_some_and(|point| {
        within(pos, layer, *point, layer, radius.saturating_add(current.width))
    })
}

fn condition_met(field: &FieldState, address: u32, condition: &LocalCondition) -> bool {
    let Some(port) = field.ports.iter().find(|port| port.node == address) else {
        return false;
    };
    match condition {
        LocalCondition::Always => true,
        LocalCondition::ChargeBelow { fraction } => {
            port.q < crate::fx::fixed_mul(port.capacity, *fraction)
        }
        LocalCondition::ChargeAbove { fraction } => {
            port.q >= crate::fx::fixed_mul(port.capacity, *fraction)
        }
        LocalCondition::OperatingMarginBelow { amount } => port.q < *amount,
        LocalCondition::Supply { state, radius } => {
            let local: Vec<&CurrentState> = field
                .currents
                .iter()
                .filter(|current| current_is_local(current, port.pos, port.layer, *radius))
                .collect();
            match state {
                SupplySense::Absent => local.iter().all(|current| !current.active),
                SupplySense::Present => local.iter().any(|current| current.active),
                SupplySense::Emitting => local.iter().any(|current| current_emitting(current)),
                SupplySense::Quiet => local.iter().any(|current| current.active && !current_emitting(current)),
            }
        }
        LocalCondition::TargetInRange { radius } => field.ports.iter().any(|other| {
            other.node != address
                && other.kind != NodeKind::Form
                && within(port.pos, port.layer, other.pos, other.layer, *radius)
        }),
        LocalCondition::RouteFlowBelow { route, flow } => field.routes.iter().any(|held| {
            held.route == *route
                && (held.tail == address || held.head == address)
                && held.flow < *flow
        }),
        LocalCondition::RouteFlowAbove { route, flow } => field.routes.iter().any(|held| {
            held.route == *route
                && (held.tail == address || held.head == address)
                && held.flow >= *flow
        }),
        LocalCondition::Overloaded => port.q > port.capacity,
        LocalCondition::SignalPresent { radius } => field.signals.iter().any(|signal| {
            within(port.pos, port.layer, signal.pos, signal.layer, *radius)
        }),
        LocalCondition::TimerElapsed { steps } => field
            .policy_runtime
            .iter()
            .find(|runtime| runtime.address == address)
            .is_some_and(|runtime| runtime.timer >= *steps),
    }
}

fn nearest_current(field: &FieldState, address: u32, radius: Fx) -> Option<u16> {
    let (pos, layer) = component_position(field, address)?;
    field
        .currents
        .iter()
        .filter(|current| current.active && current_is_local(current, pos, layer, radius))
        .filter_map(|current| {
            current
                .path
                .iter()
                .map(|point| distance(pos, layer, *point, current.layer))
                .min()
                .map(|span| (span, current.id))
        })
        .min()
        .map(|(_, id)| id)
}

fn nearest_port(field: &FieldState, address: u32, radius: Fx) -> Option<u32> {
    let (pos, layer) = component_position(field, address)?;
    field
        .ports
        .iter()
        .filter(|other| {
            other.node != address
                && other.kind != NodeKind::Form
                && within(pos, layer, other.pos, other.layer, radius)
        })
        .map(|other| (distance(pos, layer, other.pos, other.layer), other.node))
        .min()
        .map(|(_, node)| node)
}

fn nearest_signal(field: &FieldState, address: u32, radius: Fx) -> Option<u32> {
    let (pos, layer) = component_position(field, address)?;
    field
        .signals
        .iter()
        .filter(|signal| within(pos, layer, signal.pos, signal.layer, radius))
        .map(|signal| (distance(pos, layer, signal.pos, signal.layer), signal.signal))
        .min()
        .map(|(_, signal)| signal)
}

fn target_for(field: &FieldState, address: u32, action: &LocalAction) -> PolicyTarget {
    match action {
        LocalAction::SeekSupply { radius } => nearest_current(field, address, *radius)
            .map(PolicyTarget::Current)
            .unwrap_or(PolicyTarget::None),
        LocalAction::SeekPort { radius } => nearest_port(field, address, *radius)
            .map(PolicyTarget::Node)
            .unwrap_or(PolicyTarget::None),
        LocalAction::SeekSignal { radius } => nearest_signal(field, address, *radius)
            .map(PolicyTarget::Signal)
            .unwrap_or(PolicyTarget::None),
        LocalAction::Couple { radius } => nearest_port(field, address, *radius)
            .map(PolicyTarget::Node)
            .unwrap_or(PolicyTarget::None),
        LocalAction::SetRoute { route, .. } => PolicyTarget::Route(*route),
        _ => PolicyTarget::None,
    }
}

fn selected_rule<'a>(
    field: &FieldState,
    component: &'a ComponentPolicy,
) -> (i16, Option<&'a LocalCondition>, &'a LocalAction) {
    match component
        .rules
        .iter()
        .enumerate()
        .find(|(_, rule)| {
            rule.enabled && condition_met(field, component.address, &rule.condition)
        })
    {
        Some((index, rule)) => (index as i16, Some(&rule.condition), &rule.action),
        None => (-1, None, &component.fallback),
    }
}

fn condition_radius(condition: Option<&LocalCondition>) -> Fx {
    match condition {
        Some(LocalCondition::Supply { radius, .. })
        | Some(LocalCondition::TargetInRange { radius })
        | Some(LocalCondition::SignalPresent { radius }) => *radius,
        _ => 0,
    }
}

fn action_radius(action: &LocalAction) -> Fx {
    match action {
        LocalAction::SeekSupply { radius }
        | LocalAction::SeekPort { radius }
        | LocalAction::SeekSignal { radius }
        | LocalAction::Couple { radius } => *radius,
        _ => 0,
    }
}

fn preview_candidates(
    field: &FieldState,
    address: u32,
    action: &LocalAction,
) -> Vec<PolicyPreviewCandidate> {
    let Some((pos, layer)) = component_position(field, address) else {
        return Vec::new();
    };
    let mut candidates = match action {
        LocalAction::SeekSupply { radius } => field
            .currents
            .iter()
            .filter(|current| current.active && current_is_local(current, pos, layer, *radius))
            .filter_map(|current| {
                current
                    .path
                    .iter()
                    .map(|point| distance(pos, layer, *point, current.layer))
                    .min()
                    .map(|distance| PolicyPreviewCandidate {
                        target: PolicyTarget::Current(current.id),
                        distance,
                    })
            })
            .collect(),
        LocalAction::SeekPort { radius } | LocalAction::Couple { radius } => field
            .ports
            .iter()
            .filter(|other| {
                other.node != address
                    && other.kind != NodeKind::Form
                    && within(pos, layer, other.pos, other.layer, *radius)
            })
            .map(|other| PolicyPreviewCandidate {
                target: PolicyTarget::Node(other.node),
                distance: distance(pos, layer, other.pos, other.layer),
            })
            .collect(),
        LocalAction::SeekSignal { radius } => field
            .signals
            .iter()
            .filter(|signal| within(pos, layer, signal.pos, signal.layer, *radius))
            .map(|signal| PolicyPreviewCandidate {
                target: PolicyTarget::Signal(signal.signal),
                distance: distance(pos, layer, signal.pos, signal.layer),
            })
            .collect(),
        LocalAction::SetRoute { route, .. } => vec![PolicyPreviewCandidate {
            target: PolicyTarget::Route(*route),
            distance: 0,
        }],
        _ => Vec::new(),
    };
    candidates.sort_by_key(|candidate| {
        let (kind, id) = candidate.target.parts();
        (candidate.distance, kind, id.unwrap_or(-1))
    });
    candidates
}

pub fn preview(
    field: &FieldState,
    policy: &FrozenLocalPolicy,
    address: u32,
) -> Option<PolicyPreview> {
    let component = policy.components().iter().find(|held| held.address == address)?;
    if !field.ports.iter().any(|port| port.node == address) {
        return None;
    }
    let (rule, condition, action) = selected_rule(field, component);
    let target = target_for(field, address, action);
    Some(PolicyPreview {
        address,
        rule,
        condition: condition.cloned(),
        action: action.clone(),
        target,
        sensor_radius: condition_radius(condition),
        action_radius: action_radius(action),
        candidates: preview_candidates(field, address, action),
    })
}

pub fn decide(field: &FieldState, policy: &FrozenLocalPolicy) -> Vec<PolicyDecision> {
    policy
        .components()
        .iter()
        .filter(|component| field.ports.iter().any(|port| port.node == component.address))
        .map(|component| {
            let (rule, _, selected) = selected_rule(field, component);
            let action = selected.clone();
            let target = target_for(field, component.address, &action);
            PolicyDecision { address: component.address, rule, action, target }
        })
        .collect()
}

/// Reconciles execution state to the frozen policy and advances local clocks at
/// the start of one policy phase.
pub fn prepare_runtime(field: &mut FieldState, policy: &FrozenLocalPolicy) {
    let addresses: Vec<u32> = policy.components().iter().map(|component| component.address).collect();
    field.policy_runtime.retain(|runtime| addresses.binary_search(&runtime.address).is_ok());
    for address in addresses {
        if field.policy_runtime.binary_search_by_key(&address, |runtime| runtime.address).is_err() {
            field.policy_runtime.push(PolicyRuntimeState::opening(address));
        }
    }
    field.policy_runtime.sort_by_key(|runtime| runtime.address);
    for runtime in &mut field.policy_runtime {
        runtime.timer = runtime.timer.saturating_add(1);
        runtime.cooldown = runtime.cooldown.saturating_sub(1);
    }
}

/// Commits the decision facts that the completed step must carry into replay,
/// inspection, export, and qualification.
pub fn commit_runtime(
    field: &mut FieldState,
    policy: &FrozenLocalPolicy,
    decisions: &[PolicyDecision],
    outcomes: &[(u32, PolicyOutcome)],
) {
    for decision in decisions {
        let Some(runtime) = field
            .policy_runtime
            .iter_mut()
            .find(|runtime| runtime.address == decision.address)
        else {
            continue;
        };
        runtime.active_action = Some(decision.action.clone());
        runtime.active_rule = decision.rule;
        runtime.outcome = outcomes
            .iter()
            .find(|(address, _)| *address == decision.address)
            .map_or(PolicyOutcome::Unavailable, |(_, outcome)| *outcome);
        runtime.target = decision.target;
        if decision.rule >= 0 {
            let elapsed = policy
                .components()
                .iter()
                .find(|component| component.address == decision.address)
                .and_then(|component| component.rules.get(decision.rule as usize))
                .is_some_and(|rule| {
                    matches!(&rule.condition, LocalCondition::TimerElapsed { .. })
                });
            if elapsed {
                runtime.timer = 0;
            }
        }
        if matches!(&decision.action, LocalAction::UseAbility)
            && runtime.outcome == PolicyOutcome::Applied
        {
            runtime.cooldown = 15;
        }
    }
}
