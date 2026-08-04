//! Authoritative functional criteria over exact completed-step records.
//!
//! A criterion specification is immutable authored input. Its runtime is the
//! smallest mutable carry required to evaluate rolling windows, recovery
//! grace, and the final hands-off interval. The evaluator consumes only the
//! authoritative Field, the exact records and ledger produced by that step,
//! and the complete external-control declaration supplied by the caller.

use crate::fault::Fault;
use crate::field::{
    Ledger, StepRecords, NODES_PER_RUN, NODE_CHARGE_CAP, ROUTES_PER_RUN, ROUTE_CAPACITY_CAP,
};
use crate::json::{Json, Obj, MAX_SAFE_INT};
use crate::read;
use crate::state::{ControlState, FieldState, Frac, Fx, Step, FRAC_ONE};

/// The longest rolling or hands-off interval accepted by one criterion: one
/// hour at the locked thirty simulation steps per second.
pub const CRITERION_INTERVAL_CAP: Step = 108_000;

/// One required Component and the minimum stored Charge at which it is
/// operating. The observable margin is `q - minimum_q`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComponentRequirement {
    node: u32,
    minimum_q: Fx,
}

impl ComponentRequirement {
    pub fn new(node: u32, minimum_q: Fx) -> Result<Self, Fault> {
        if node == 0 || !(0..=NODE_CHARGE_CAP).contains(&minimum_q) {
            return Err(Fault::field("component_requirements"));
        }
        Ok(Self { node, minimum_q })
    }

    pub fn node(&self) -> u32 {
        self.node
    }

    pub fn minimum_q(&self) -> Fx {
        self.minimum_q
    }

    fn read(value: &Json) -> Result<Self, Fault> {
        read::exact_keys(value, "component_requirements", &["minimum_q", "node"])?;
        Self::new(
            read::int(value, "node", 1, i64::from(u32::MAX))? as u32,
            read::int(value, "minimum_q", 0, NODE_CHARGE_CAP)?,
        )
    }

    fn written(&self) -> String {
        let mut out = String::new();
        let mut object = Obj::new(&mut out);
        object.int("minimum_q", self.minimum_q);
        object.int("node", i64::from(self.node));
        object.end();
        out
    }
}

/// Frozen success conditions for one run family.
///
/// Every required Route uses the same per-step rolling minimum floor. The mean
/// remains in the reading as diagnostic context, but cannot hide a failed
/// service step. Every
/// required Component carries its own minimum stored Charge. Leakage is
/// compared as exact accumulated integers over the same window; division is
/// used only to produce the displayed reading.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FunctionCriterionSpec {
    component_requirements: Vec<ComponentRequirement>,
    failure_grace_steps: Step,
    hands_off_steps: Step,
    leakage_ratio_ceiling: Frac,
    route_flow_floor: Fx,
    route_ids: Vec<u32>,
    window_steps: Step,
}

impl FunctionCriterionSpec {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        route_ids: Vec<u32>,
        route_flow_floor: Fx,
        component_requirements: Vec<ComponentRequirement>,
        leakage_ratio_ceiling: Frac,
        window_steps: Step,
        failure_grace_steps: Step,
        hands_off_steps: Step,
    ) -> Result<Self, Fault> {
        let component_ids: Vec<u32> =
            component_requirements.iter().map(ComponentRequirement::node).collect();
        if route_ids.len() > ROUTES_PER_RUN
            || route_ids.first().is_some_and(|first| *first == 0)
            || !read::ascending(&route_ids)
            || route_ids.is_empty() != (route_flow_floor == 0)
            || component_requirements.is_empty()
            || component_requirements.len() > NODES_PER_RUN
            || !read::ascending(&component_ids)
            || !(0..=ROUTE_CAPACITY_CAP).contains(&route_flow_floor)
            || !(0..=FRAC_ONE).contains(&leakage_ratio_ceiling)
            || !(1..=CRITERION_INTERVAL_CAP).contains(&window_steps)
            || failure_grace_steps > CRITERION_INTERVAL_CAP
            || !(1..=CRITERION_INTERVAL_CAP).contains(&hands_off_steps)
        {
            return Err(Fault::field("function_criterion"));
        }
        Ok(Self {
            component_requirements,
            failure_grace_steps,
            hands_off_steps,
            leakage_ratio_ceiling,
            route_flow_floor,
            route_ids,
            window_steps,
        })
    }

    pub fn read(value: &Json, key: &str) -> Result<Self, Fault> {
        let found = read::map(value, key)?;
        Self::read_value(found, key)
    }

    pub fn read_value(found: &Json, key: &str) -> Result<Self, Fault> {
        read::exact_keys(
            found,
            key,
            &[
                "component_requirements",
                "failure_grace_steps",
                "hands_off_steps",
                "leakage_ratio_ceiling",
                "route_flow_floor",
                "route_ids",
                "window_steps",
            ],
        )?;
        let mut components = Vec::new();
        for entry in read::list(found, "component_requirements", NODES_PER_RUN)? {
            components.push(ComponentRequirement::read(entry)?);
        }
        Self::new(
            read::ids(found, "route_ids", ROUTES_PER_RUN, i64::from(u32::MAX))?,
            read::int(found, "route_flow_floor", 0, ROUTE_CAPACITY_CAP)?,
            components,
            read::int(found, "leakage_ratio_ceiling", 0, FRAC_ONE)?,
            read::int(found, "window_steps", 1, i64::from(CRITERION_INTERVAL_CAP))? as Step,
            read::int(found, "failure_grace_steps", 0, i64::from(CRITERION_INTERVAL_CAP))? as Step,
            read::int(found, "hands_off_steps", 1, i64::from(CRITERION_INTERVAL_CAP))? as Step,
        )
    }

    pub fn component_requirements(&self) -> &[ComponentRequirement] {
        &self.component_requirements
    }

    pub fn failure_grace_steps(&self) -> Step {
        self.failure_grace_steps
    }

    pub fn hands_off_steps(&self) -> Step {
        self.hands_off_steps
    }

    pub fn leakage_ratio_ceiling(&self) -> Frac {
        self.leakage_ratio_ceiling
    }

    pub fn route_flow_floor(&self) -> Fx {
        self.route_flow_floor
    }

    pub fn route_ids(&self) -> &[u32] {
        &self.route_ids
    }

    pub fn window_steps(&self) -> Step {
        self.window_steps
    }

    pub fn write(&self, out: &mut String) {
        let mut object = Obj::new(out);
        {
            let mut components = object.list("component_requirements");
            for requirement in &self.component_requirements {
                components.raw(&requirement.written());
            }
            components.end();
        }
        object.int("failure_grace_steps", i64::from(self.failure_grace_steps));
        object.int("hands_off_steps", i64::from(self.hands_off_steps));
        object.int("leakage_ratio_ceiling", self.leakage_ratio_ceiling);
        object.int("route_flow_floor", self.route_flow_floor);
        {
            let mut routes = object.list("route_ids");
            for route in &self.route_ids {
                routes.int(i64::from(*route));
            }
            routes.end();
        }
        object.int("window_steps", i64::from(self.window_steps));
        object.end();
    }

    pub fn written(&self) -> String {
        let mut out = String::new();
        self.write(&mut out);
        out
    }
}

/// Whether a criterion can still resolve, has passed, or exceeded its allowed
/// consecutive metric-failure grace.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CriterionStatus {
    Active,
    Passed,
    Failed,
}

impl CriterionStatus {
    pub fn name(self) -> &'static str {
        match self {
            CriterionStatus::Active => "active",
            CriterionStatus::Passed => "passed",
            CriterionStatus::Failed => "failed",
        }
    }

    fn read(value: &Json) -> Result<Self, Fault> {
        match read::one_of(value, "status", &["active", "failed", "passed"])? {
            0 => Ok(CriterionStatus::Active),
            1 => Ok(CriterionStatus::Failed),
            _ => Ok(CriterionStatus::Passed),
        }
    }
}

/// Exact authoritative inputs for one completed step.
///
/// `other_external_control` covers every recorded external act not represented
/// by `ControlState`, including ability use, handoff, intervention, or rescue.
pub struct CriterionStepInput<'a> {
    pub field: &'a FieldState,
    pub records: &'a StepRecords,
    pub ledger: &'a Ledger,
    pub control: ControlState,
    pub other_external_control: bool,
}

impl CriterionStepInput<'_> {
    pub fn hands_off(&self) -> bool {
        self.control == ControlState::default() && !self.other_external_control
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct WindowSample {
    component_charges: Option<Vec<Option<Fx>>>,
    leakage: Fx,
    route_flows: Vec<Fx>,
    step: Step,
    supply: Fx,
}

impl WindowSample {
    fn from_input(spec: &FunctionCriterionSpec, input: &CriterionStepInput<'_>) -> Self {
        let component_charges = spec
            .component_requirements
            .iter()
            .map(|requirement| {
                input
                    .field
                    .ports
                    .iter()
                    .find(|port| port.node == requirement.node)
                    .map(|port| port.q)
            })
            .collect();
        let route_flows = spec
            .route_ids
            .iter()
            .map(|route| {
                input
                    .records
                    .f
                    .binary_search_by_key(route, |(id, _)| *id)
                    .ok()
                    .map_or(0, |place| input.records.f[place].1)
            })
            .collect();
        WindowSample {
            component_charges: Some(component_charges),
            leakage: input.ledger.leakage,
            route_flows,
            step: input.field.step,
            supply: input.ledger.current,
        }
    }

    fn read(value: &Json, spec: &FunctionCriterionSpec) -> Result<Self, Fault> {
        let component_charges = if value.get("component_charges").is_some() {
            read::exact_keys(
                value,
                "history",
                &["component_charges", "leakage", "route_flows", "step", "supply"],
            )?;
            match read::at(value, "component_charges")? {
                Json::Null => None,
                Json::List(raw) if raw.len() == spec.component_requirements.len() => {
                    let mut charges = Vec::with_capacity(raw.len());
                    for charge in raw {
                        charges.push(match charge {
                            Json::Null => None,
                            Json::Int(value) if (0..=NODE_CHARGE_CAP).contains(value) => Some(*value),
                            _ => return Err(Fault::field("component_charges")),
                        });
                    }
                    Some(charges)
                }
                _ => return Err(Fault::field("component_charges")),
            }
        } else {
            // Pre-Q-03 runtime samples did not retain addressed Component
            // values. They remain readable for ordinary resumed play, but a
            // qualification resolution rejects the missing exact evidence.
            read::exact_keys(value, "history", &["leakage", "route_flows", "step", "supply"])?;
            None
        };
        let raw_flows = read::list(value, "route_flows", ROUTES_PER_RUN)?;
        if raw_flows.len() != spec.route_ids.len() {
            return Err(Fault::field("route_flows"));
        }
        let mut route_flows = Vec::with_capacity(raw_flows.len());
        for flow in raw_flows {
            route_flows.push(
                flow.as_int()
                    .filter(|value| (0..=ROUTE_CAPACITY_CAP).contains(value))
                    .ok_or_else(|| Fault::field("route_flows"))?,
            );
        }
        Ok(WindowSample {
            component_charges,
            leakage: read::int(value, "leakage", 0, MAX_SAFE_INT)?,
            route_flows,
            step: read::int(value, "step", 1, i64::from(u32::MAX))? as Step,
            supply: read::int(value, "supply", 0, MAX_SAFE_INT)?,
        })
    }

    fn written(&self) -> String {
        let mut out = String::new();
        let mut object = Obj::new(&mut out);
        match &self.component_charges {
            Some(charges) => {
                let mut values = object.list("component_charges");
                for charge in charges {
                    match charge {
                        Some(value) => values.int(*value),
                        None => values.raw("null"),
                    };
                }
                values.end();
            }
            None => {
                object.null("component_charges");
            }
        };
        object.int("leakage", self.leakage);
        {
            let mut flows = object.list("route_flows");
            for flow in &self.route_flows {
                flows.int(*flow);
            }
            flows.end();
        }
        object.int("step", i64::from(self.step));
        object.int("supply", self.supply);
        object.end();
        out
    }
}

/// The mutable, canonical carry for one immutable criterion specification.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CriterionRuntime {
    failure_streak: Step,
    hands_off_streak: Step,
    history: Vec<WindowSample>,
    last_step: Step,
    resolved_step: Option<Step>,
    started_step: Step,
    status: CriterionStatus,
}

impl CriterionRuntime {
    pub fn opening(started_step: Step) -> Self {
        CriterionRuntime {
            failure_streak: 0,
            hands_off_streak: 0,
            history: Vec::new(),
            last_step: started_step,
            resolved_step: None,
            started_step,
            status: CriterionStatus::Active,
        }
    }

    pub fn read(
        value: &Json,
        key: &str,
        spec: &FunctionCriterionSpec,
    ) -> Result<Self, Fault> {
        let found = read::map(value, key)?;
        read::exact_keys(
            found,
            key,
            &[
                "failure_streak",
                "hands_off_streak",
                "history",
                "last_step",
                "resolved_step",
                "started_step",
                "status",
            ],
        )?;
        let mut history = Vec::new();
        for entry in read::list(found, "history", spec.window_steps as usize)? {
            history.push(WindowSample::read(entry, spec)?);
        }
        let runtime = CriterionRuntime {
            failure_streak: read::int(found, "failure_streak", 0, i64::from(u32::MAX))? as Step,
            hands_off_streak: read::int(found, "hands_off_streak", 0, i64::from(u32::MAX))? as Step,
            history,
            last_step: read::int(found, "last_step", 0, i64::from(u32::MAX))? as Step,
            resolved_step: read::int_or_null(found, "resolved_step", 0, i64::from(u32::MAX))?
                .map(|step| step as Step),
            started_step: read::int(found, "started_step", 0, i64::from(u32::MAX))? as Step,
            status: CriterionStatus::read(found)?,
        };
        runtime.coherent(spec)?;
        Ok(runtime)
    }

    pub fn status(&self) -> CriterionStatus {
        self.status
    }

    pub fn started_step(&self) -> Step {
        self.started_step
    }

    pub fn last_step(&self) -> Step {
        self.last_step
    }

    pub fn resolved_step(&self) -> Option<Step> {
        self.resolved_step
    }

    pub fn failure_streak(&self) -> Step {
        self.failure_streak
    }

    pub fn hands_off_streak(&self) -> Step {
        self.hands_off_streak
    }

    /// The exact minimum retained Charge for one addressed requirement.
    /// `None` means the runtime predates addressed Component history or one
    /// sample did not contain the required Component; Q-03 must reject either
    /// condition instead of substituting the terminal value.
    pub fn component_window_minimum(
        &self,
        spec: &FunctionCriterionSpec,
        node: u32,
    ) -> Option<Fx> {
        if self.history.len() != spec.window_steps as usize {
            return None;
        }
        let position = spec
            .component_requirements
            .iter()
            .position(|requirement| requirement.node == node)?;
        self.history
            .iter()
            .map(|sample| sample.component_charges.as_ref()?.get(position).copied().flatten())
            .collect::<Option<Vec<_>>>()?
            .into_iter()
            .min()
    }

    pub fn current_reading(
        &self,
        spec: &FunctionCriterionSpec,
        field: &FieldState,
        hands_off: bool,
    ) -> CriterionReading {
        self.reading(spec, field, hands_off)
    }

    pub fn advance(
        &mut self,
        spec: &FunctionCriterionSpec,
        input: CriterionStepInput<'_>,
    ) -> Result<CriterionReading, Fault> {
        if input.field.step != self.last_step.saturating_add(1) {
            return Err(Fault::field("criterion_step"));
        }
        self.history.push(WindowSample::from_input(spec, &input));
        if self.history.len() > spec.window_steps as usize {
            self.history.remove(0);
        }
        self.last_step = input.field.step;

        let mut reading = self.reading(spec, input.field, input.hands_off());
        if self.status == CriterionStatus::Active && reading.ready {
            if reading.all_metrics_met {
                self.failure_streak = 0;
                self.hands_off_streak = if reading.hands_off {
                    self.hands_off_streak.saturating_add(1)
                } else {
                    0
                };
            } else {
                self.failure_streak = self.failure_streak.saturating_add(1);
                self.hands_off_streak = 0;
            }
            if self.failure_streak > spec.failure_grace_steps {
                self.status = CriterionStatus::Failed;
                self.resolved_step = Some(self.last_step);
            } else if self.hands_off_streak >= spec.hands_off_steps {
                self.status = CriterionStatus::Passed;
                self.resolved_step = Some(self.last_step);
            }
        }
        reading.failure_streak = self.failure_streak;
        reading.failure_grace_remaining = spec.failure_grace_steps.saturating_sub(self.failure_streak);
        reading.hands_off_streak = self.hands_off_streak;
        reading.hands_off_remaining = spec.hands_off_steps.saturating_sub(self.hands_off_streak);
        reading.status = self.status;
        Ok(reading)
    }

    pub fn write(&self, out: &mut String) {
        let mut object = Obj::new(out);
        object.int("failure_streak", i64::from(self.failure_streak));
        object.int("hands_off_streak", i64::from(self.hands_off_streak));
        {
            let mut history = object.list("history");
            for sample in &self.history {
                history.raw(&sample.written());
            }
            history.end();
        }
        object.int("last_step", i64::from(self.last_step));
        object.int_or_null("resolved_step", self.resolved_step.map(i64::from));
        object.int("started_step", i64::from(self.started_step));
        object.text("status", self.status.name());
        object.end();
    }

    pub fn written(&self) -> String {
        let mut out = String::new();
        self.write(&mut out);
        out
    }

    fn coherent(&self, spec: &FunctionCriterionSpec) -> Result<(), Fault> {
        if self.last_step < self.started_step
            || (self.last_step == self.started_step) != self.history.is_empty()
            || self.history.len() > spec.window_steps as usize
        {
            return Err(Fault::field("criterion_runtime"));
        }
        if !self.history.is_empty() {
            let first = self.last_step - self.history.len() as Step + 1;
            if self
                .history
                .iter()
                .enumerate()
                .any(|(place, sample)| sample.step != first + place as Step)
            {
                return Err(Fault::field("history"));
            }
        }
        let resolved = self.resolved_step.is_some_and(|step| {
            step >= self.started_step && step <= self.last_step
        });
        let status_ok = match self.status {
            CriterionStatus::Active => {
                self.resolved_step.is_none()
                    && self.failure_streak <= spec.failure_grace_steps
                    && self.hands_off_streak < spec.hands_off_steps
            }
            CriterionStatus::Passed => {
                resolved && self.hands_off_streak >= spec.hands_off_steps
            }
            CriterionStatus::Failed => {
                resolved && self.failure_streak > spec.failure_grace_steps
            }
        };
        if !status_ok {
            return Err(Fault::field("status"));
        }
        Ok(())
    }

    fn reading(
        &self,
        spec: &FunctionCriterionSpec,
        field: &FieldState,
        hands_off: bool,
    ) -> CriterionReading {
        let ready = self.history.len() == spec.window_steps as usize;
        let routes = spec
            .route_ids
            .iter()
            .enumerate()
            .map(|(place, route)| {
                let total = checked_sum(self.history.iter().map(|sample| sample.route_flows[place]));
                let minimum = if ready {
                    self.history
                        .iter()
                        .map(|sample| sample.route_flows[place])
                        .min()
                        .unwrap_or(0)
                } else {
                    0
                };
                let met = ready && minimum >= spec.route_flow_floor;
                RouteCriterionReading {
                    floor: spec.route_flow_floor,
                    mean: if ready { total / i64::from(spec.window_steps) } else { 0 },
                    minimum,
                    met,
                    route: *route,
                    total,
                    window_steps: spec.window_steps,
                }
            })
            .collect::<Vec<_>>();
        let components = spec
            .component_requirements
            .iter()
            .map(|requirement| {
                let found = field.ports.iter().find(|port| port.node == requirement.node);
                let exact_minimum = self.component_window_minimum(spec, requirement.node);
                let charge = exact_minimum.unwrap_or_else(|| found.map_or(0, |port| port.q));
                ComponentCriterionReading {
                    charge,
                    margin: charge - requirement.minimum_q,
                    met: ready
                        && exact_minimum.is_some()
                        && charge >= requirement.minimum_q,
                    minimum_q: requirement.minimum_q,
                    node: requirement.node,
                    present: exact_minimum.is_some(),
                }
            })
            .collect::<Vec<_>>();
        let leakage = checked_sum(self.history.iter().map(|sample| sample.leakage));
        let supply = checked_sum(self.history.iter().map(|sample| sample.supply));
        let (ratio, ratio_met) = leakage_reading(leakage, supply, spec.leakage_ratio_ceiling);
        let leakage = LeakageCriterionReading {
            ceiling: spec.leakage_ratio_ceiling,
            leakage,
            met: ready && ratio_met,
            ratio,
            supply,
        };
        let all_metrics_met = ready
            && routes.iter().all(|route| route.met)
            && components.iter().all(|component| component.met)
            && leakage.met;
        CriterionReading {
            all_metrics_met,
            components,
            failure_grace_remaining: spec.failure_grace_steps.saturating_sub(self.failure_streak),
            failure_streak: self.failure_streak,
            hands_off,
            hands_off_remaining: spec.hands_off_steps.saturating_sub(self.hands_off_streak),
            hands_off_streak: self.hands_off_streak,
            leakage,
            observed_steps: self.history.len() as Step,
            ready,
            routes,
            status: self.status,
            step: field.step,
        }
    }
}

/// One exact required-Route reading over the current complete rolling window.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RouteCriterionReading {
    pub floor: Fx,
    pub mean: Fx,
    pub minimum: Fx,
    pub met: bool,
    pub route: u32,
    pub total: Fx,
    pub window_steps: Step,
}

/// One exact required-Component operating reading.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComponentCriterionReading {
    pub charge: Fx,
    pub margin: Fx,
    pub met: bool,
    pub minimum_q: Fx,
    pub node: u32,
    pub present: bool,
}

/// Leakage over Supply for the current complete rolling window. `ratio` is
/// null exactly when Supply is zero and leakage is nonzero; that case fails.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LeakageCriterionReading {
    pub ceiling: Frac,
    pub leakage: Fx,
    pub met: bool,
    pub ratio: Option<Frac>,
    pub supply: Fx,
}

/// The complete post-step reading returned by [`CriterionRuntime::advance`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CriterionReading {
    pub all_metrics_met: bool,
    pub components: Vec<ComponentCriterionReading>,
    pub failure_grace_remaining: Step,
    pub failure_streak: Step,
    pub hands_off: bool,
    pub hands_off_remaining: Step,
    pub hands_off_streak: Step,
    pub leakage: LeakageCriterionReading,
    pub observed_steps: Step,
    pub ready: bool,
    pub routes: Vec<RouteCriterionReading>,
    pub status: CriterionStatus,
    pub step: Step,
}

impl CriterionReading {
    pub fn write(&self, out: &mut String) {
        let mut object = Obj::new(out);
        object.bool("all_metrics_met", self.all_metrics_met);
        {
            let mut components = object.list("components");
            for reading in &self.components {
                let mut entry = components.object();
                entry.int("charge", reading.charge);
                entry.int("margin", reading.margin);
                entry.bool("met", reading.met);
                entry.int("minimum_q", reading.minimum_q);
                entry.int("node", i64::from(reading.node));
                entry.bool("present", reading.present);
                entry.end();
            }
            components.end();
        }
        object.int("failure_grace_remaining", i64::from(self.failure_grace_remaining));
        object.int("failure_streak", i64::from(self.failure_streak));
        object.bool("hands_off", self.hands_off);
        object.int("hands_off_remaining", i64::from(self.hands_off_remaining));
        object.int("hands_off_streak", i64::from(self.hands_off_streak));
        {
            let mut leakage = object.object("leakage");
            leakage.int("ceiling", self.leakage.ceiling);
            leakage.int("leakage", self.leakage.leakage);
            leakage.bool("met", self.leakage.met);
            leakage.int_or_null("ratio", self.leakage.ratio);
            leakage.int("supply", self.leakage.supply);
            leakage.end();
        }
        object.int("observed_steps", i64::from(self.observed_steps));
        object.bool("ready", self.ready);
        {
            let mut routes = object.list("routes");
            for reading in &self.routes {
                let mut entry = routes.object();
                entry.int("floor", reading.floor);
                entry.int("mean", reading.mean);
                entry.int("minimum", reading.minimum);
                entry.bool("met", reading.met);
                entry.int("route", i64::from(reading.route));
                entry.int("total", reading.total);
                entry.int("window_steps", i64::from(reading.window_steps));
                entry.end();
            }
            routes.end();
        }
        object.text("status", self.status.name());
        object.int("step", i64::from(self.step));
        object.end();
    }

    pub fn written(&self) -> String {
        let mut out = String::new();
        self.write(&mut out);
        out
    }
}

fn checked_sum(values: impl Iterator<Item = Fx>) -> Fx {
    values.fold(0, |sum, value| {
        sum.checked_add(value).expect("a bounded criterion window stays inside i64")
    })
}

fn leakage_reading(leakage: Fx, supply: Fx, ceiling: Frac) -> (Option<Frac>, bool) {
    if supply == 0 {
        return if leakage == 0 { (Some(0), true) } else { (None, false) };
    }
    let ratio = Fx::try_from((i128::from(leakage) << 16) / i128::from(supply))
        .expect("a bounded leakage ratio stays inside i64");
    let met = i128::from(leakage) * i128::from(FRAC_ONE)
        <= i128::from(supply) * i128::from(ceiling);
    (Some(ratio), met)
}
