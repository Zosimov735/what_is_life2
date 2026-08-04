//! The fixed-step runtime.
//!
//! One run, advanced in steps of exactly 1/30 s of simulated time. The step
//! function consumes the control state of the `InputFrame` that asked for it,
//! advances the completed-step counter, records the step in the retained
//! trajectory, and carries the trajectory's keyframe forward. It reads no
//! clock: how wall time maps onto a step count is the accumulator's business,
//! and the accumulator lives in the worker. State is a pure function of the
//! run key, the branch nonce, the content hash, and the executed steps with
//! the control states they consumed — which is the whole of the
//! byte-equivalence contract.
//!
//! What one step moves is [`crate::field::advance`]'s business: the Field's own
//! parts, its Charge ledger, and the records the step carries. Nothing here
//! draws randomness yet, because no rule of the Field is stochastic until
//! pressures and currents supply exogenous terms; the stream position is
//! recorded with each step regardless, because that is what a replay reads.

use crate::fault::{Code, Fault};
use crate::field::{self, cap_fault, Cue, StepRecords};
use crate::frame::{self, Snapshot};
use crate::json::{is_hex, Json, Obj};
use crate::plan::{self, PlanCommand, PlanQueue, Projection, PLAN_QUEUE_DEPTH};
use crate::records::save_key;
use crate::rng::trajectory_stream;
use crate::sha256;
use crate::slate::CandidateSlate;
use crate::state::{
    CheckpointState, ControlState, FieldState, Frac, GeneratorSpec, InputConfig, Progress,
    RegimeSpec, ScenarioSpec,
    RecordKind, RunState, Trace, TraceStep, ViewDeclaration, AUTOSAVE_STEPS, SAVE_PAYLOAD_CAP,
};

/// The eight starting Forms, as their machine ids.
pub const FORMS: [&str; 8] =
    ["thread", "ring", "relay", "vault", "lens", "knot", "wake", "chorus"];

/// The most steps one `InputFrame` may ask for.
const MAX_ADVANCE_STEPS: i64 = 1800;

#[derive(Clone, Debug, PartialEq, Eq)]
struct RouteMechanismReading {
    route: u32,
    enabled: bool,
    capacity_limit: crate::state::Fx,
    allocation_weight: u16,
    requested: crate::state::Fx,
    accepted: crate::state::Fx,
    stance: &'static str,
}

#[derive(Clone, Debug)]
struct MechanismSnapshot {
    policies: Vec<crate::policy::PolicyRuntimeState>,
    policy_objects: Vec<(u32, &'static str, u32)>,
    interfaces: Vec<(u32, bool)>,
    routes: Vec<RouteMechanismReading>,
    reserves: Vec<(u8, u32, crate::state::Fx)>,
    supplies: Vec<(u16, bool)>,
}

impl MechanismSnapshot {
    fn of(field: &FieldState) -> Self {
        let policy_objects = field
            .policy_runtime
            .iter()
            .map(|runtime| {
                field.forms.iter().find(|form| form.node == runtime.address).map_or(
                    (runtime.address, "node", runtime.address),
                    |form| (runtime.address, "form", u32::from(form.id)),
                )
            })
            .collect();
        let interfaces = field.ports.iter().map(|port| (port.node, port.open)).collect();
        let routes = field
            .routes
            .iter()
            .map(|route| {
                let control = field
                    .route_controls
                    .iter()
                    .find(|control| control.route == route.route);
                let enabled = control.is_none_or(|control| control.enabled);
                let capacity_limit = control.map_or(route.capacity, |control| {
                    control.capacity_limit.min(route.capacity)
                });
                let allocation_weight = control.map_or(1, |control| control.allocation_weight);
                let runtime = field
                    .route_runtime
                    .iter()
                    .find(|runtime| runtime.route == route.route);
                let stance = if !enabled {
                    "disabled"
                } else if capacity_limit == 0 {
                    "closed"
                } else {
                    runtime.map_or("standing", |runtime| runtime.outcome.name())
                };
                RouteMechanismReading {
                    route: route.route,
                    enabled,
                    capacity_limit,
                    allocation_weight,
                    requested: runtime.map_or(0, |runtime| runtime.requested),
                    accepted: runtime.map_or(route.flow, |runtime| runtime.accepted),
                    stance,
                }
            })
            .collect();
        let supplies = field
            .currents
            .iter()
            .map(|current| (current.id, crate::field::current_emitting(current)))
            .collect();
        let reserves = field
            .forms
            .iter()
            .filter(|form| form.reserve > 0 || form.form == "vault")
            .map(|form| (form.id, form.node, form.reserve))
            .collect();
        Self {
            policies: field.policy_runtime.clone(),
            policy_objects,
            interfaces,
            routes,
            reserves,
            supplies,
        }
    }

    fn events_since(&self, before: &Self) -> Vec<String> {
        let mut events = Vec::new();
        for runtime in &self.policies {
            let changed = before
                .policies
                .iter()
                .find(|held| held.address == runtime.address)
                .is_none_or(|held| {
                    held.active_rule != runtime.active_rule
                        || held.active_action != runtime.active_action
                        || held.outcome != runtime.outcome
                        || held.target != runtime.target
                });
            if !changed {
                continue;
            }
            let mut body = String::new();
            let mut object = Obj::new(&mut body);
            object.text(
                "action",
                runtime.active_action.as_ref().map_or("none", |action| action.name()),
            );
            object.int("address", i64::from(runtime.address));
            object.text("kind", "policy");
            let (_, object_kind, object_id) = self
                .policy_objects
                .iter()
                .find(|(address, _, _)| *address == runtime.address)
                .copied()
                .unwrap_or((runtime.address, "node", runtime.address));
            object.int("object_id", i64::from(object_id));
            object.text("object_kind", object_kind);
            object.text("outcome", runtime.outcome.name());
            object.int("rule", i64::from(runtime.active_rule));
            let (target_kind, target) = runtime.target.parts();
            object.int_or_null("target", target);
            object.text("target_kind", target_kind);
            object.end();
            events.push(body);
        }
        for (node, open) in &self.interfaces {
            if before
                .interfaces
                .iter()
                .find(|(held, _)| held == node)
                .is_some_and(|(_, held)| held == open)
            {
                continue;
            }
            let mut body = String::new();
            let mut object = Obj::new(&mut body);
            object.text("kind", "interface");
            object.int("node", i64::from(*node));
            object.bool("open", *open);
            object.end();
            events.push(body);
        }
        for route in &self.routes {
            if before
                .routes
                .iter()
                .find(|held| held.route == route.route)
                .is_some_and(|held| {
                    held.enabled == route.enabled
                        && held.capacity_limit == route.capacity_limit
                        && held.allocation_weight == route.allocation_weight
                        && held.requested == route.requested
                        && held.accepted == route.accepted
                        && held.stance == route.stance
                })
            {
                continue;
            }
            let mut body = String::new();
            let mut object = Obj::new(&mut body);
            object.int("allocation_weight", i64::from(route.allocation_weight));
            object.int("accepted_flow", route.accepted);
            object.int("capacity_limit", route.capacity_limit);
            object.bool("enabled", route.enabled);
            object.text("kind", "route");
            object.int("requested_flow", route.requested);
            object.int("route", i64::from(route.route));
            object.text("state", route.stance);
            object.end();
            events.push(body);
        }
        for (current, emitting) in &self.supplies {
            if before
                .supplies
                .iter()
                .find(|(held, _)| held == current)
                .is_some_and(|(_, held)| held == emitting)
            {
                continue;
            }
            let mut body = String::new();
            let mut object = Obj::new(&mut body);
            object.int("current", i64::from(*current));
            object.bool("emitting", *emitting);
            object.text("kind", "supply");
            object.end();
            events.push(body);
        }
        for (form, node, closing) in &self.reserves {
            let opening = before
                .reserves
                .iter()
                .find(|(held, _, _)| held == form)
                .map_or(0, |(_, _, amount)| *amount);
            if opening == *closing {
                continue;
            }
            let mut body = String::new();
            let mut object = Obj::new(&mut body);
            object.int("closing", *closing);
            object.int("delta", closing.saturating_sub(opening));
            object.int("form", i64::from(*form));
            object.text("kind", "reserve");
            object.int("node", i64::from(*node));
            object.int("opening", opening);
            object.text("state", if closing > &opening { "banked" } else { "released" });
            object.end();
            events.push(body);
        }
        events
    }
}

/// Periodic exact Charge accounting for the commissioning evidence trail.
/// Transitions remain immediate above; continuous transfers are sampled on a
/// fixed core-step cadence so accelerated Commission does not flood the event
/// boundary with one record per mechanism per step.
fn charge_mechanism_event(step: u32, ledger: &crate::field::Ledger) -> Option<String> {
    let active = ledger.current != 0
        || ledger.gathered != 0
        || ledger.moved != 0
        || ledger.upkeep != 0
        || ledger.leakage != 0
        || ledger.drain != 0
        || ledger.opening != ledger.closing;
    let cadence = step == 1 || step % 15 == 0 || ledger.gathered != 0 && step % 5 == 0;
    if !active || !cadence {
        return None;
    }

    let dominant_node = ledger
        .nodes
        .iter()
        .map(|entry| {
            let activity = entry
                .inflow
                .unsigned_abs()
                .saturating_add(entry.outflow.unsigned_abs())
                .saturating_add(entry.upkeep.unsigned_abs())
                .saturating_add(entry.supply.unsigned_abs())
                .saturating_add(entry.leakage.unsigned_abs())
                .saturating_add(entry.exogenous.unsigned_abs());
            (activity, entry.node)
        })
        .max_by_key(|(activity, node)| (*activity, std::cmp::Reverse(*node)))
        .filter(|(activity, _)| *activity > 0)
        .map(|(_, node)| node);

    let mut out = String::new();
    let mut object = Obj::new(&mut out);
    object.int("accepted_supply", ledger.current);
    object.int("closing", ledger.closing);
    object.int("coupled_transfer", ledger.gathered);
    object.int("drain", ledger.drain);
    object.int_or_null("dominant_node", dominant_node.map(i64::from));
    object.text("kind", "charge");
    object.int("leakage", ledger.leakage);
    {
        let mut nodes = object.list("nodes");
        for entry in &ledger.nodes {
            if entry.inflow == 0
                && entry.outflow == 0
                && entry.upkeep == 0
                && entry.supply == 0
                && entry.leakage == 0
                && entry.exogenous == 0
                && entry.opening == entry.closing
            {
                continue;
            }
            let mut node = nodes.object();
            node.int("closing", entry.closing);
            node.int("exogenous", entry.exogenous);
            node.int("inflow", entry.inflow);
            node.int("leakage", entry.leakage);
            node.int("node", i64::from(entry.node));
            node.int("opening", entry.opening);
            node.int("outflow", entry.outflow);
            node.int("supply", entry.supply);
            node.int("upkeep", entry.upkeep);
            node.end();
        }
        nodes.end();
    }
    object.int("opening", ledger.opening);
    object.int("route_transfer", ledger.moved);
    object.int("upkeep", ledger.upkeep);
    object.end();
    Some(out)
}

/// Lens pays one Charge unit to read the chassis-local 192-unit neighborhood.
pub const LENS_SAMPLE_COST: crate::state::Fx = crate::fx::ONE_UNIT;
pub const LENS_SENSOR_RADIUS: crate::state::Fx = 192 * crate::fx::ONE_UNIT;
const LENS_FORECAST_TRIALS: usize = 8;

/// The steering component limit. The value −32768 is never sent.
const STEER_LIMIT: i64 = 32767;

/// The wheel delta sum limit, per frame.
const WHEEL_LIMIT: i64 = 3000;

/// Where the accumulated wheel delta triggers one depth change, and the bound
/// the accumulator is held to.
const WHEEL_TRIGGER: i32 = 480;

/// How many executed steps must pass before another depth change resolves.
const DEPTH_COOLDOWN: u8 = 15;

/// The full time scale, Q0.16. The header carries 16 bits, where 65536 has no
/// representation, so full speed is written as the widest value the field
/// holds; the ramps run over [0, 65535] there.
pub const FULL_SCALE: i64 = 65_536;

/// How long a ramp takes, in the accumulator's own microsecond-times-30 units:
/// the locked 250,000 µs of real time, which is 7,500,000 of them.
///
/// The unit is the accumulator's rather than plain microseconds so the ramp
/// clock and the step clock are counted in one currency: one step is exactly
/// [`RAMP_STEP_UNITS`] of it, and a ramp is exactly 7.5 of those — the 250 ms
/// read in steps, which is what a slowdown of that length costs a run at full
/// speed and less than that at a falling one.
pub const RAMP_UNITS: i64 = 7_500_000;

/// One step of the ramp clock: 1/30 s in the same units.
pub const RAMP_STEP_UNITS: i64 = 1_000_000;

/// The widest gap one frame may carry, in microseconds: the accumulator's own
/// locked clamp, applied here to the same timestamps, so the ramp clock and the
/// accumulator read one frame the same way.
const MAX_GAP_US: i64 = 250_000;

/// The mode the run is in, which is also its lifecycle state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Running,
    RampIn,
    Still,
    RampOut,
    Suspended,
    Ended,
}

impl Mode {
    pub fn name(self) -> &'static str {
        match self {
            Mode::Running => "running",
            Mode::RampIn => "ramp_in",
            Mode::Still => "still",
            Mode::RampOut => "ramp_out",
            Mode::Suspended => "suspended",
            Mode::Ended => "ended",
        }
    }

    /// The mode's number in the render snapshot's header.
    pub fn ordinal(self) -> u8 {
        match self {
            Mode::Running => 0,
            Mode::RampIn => 1,
            Mode::Still => 2,
            Mode::RampOut => 3,
            Mode::Suspended => 4,
            Mode::Ended => 5,
        }
    }
}

/// One frame of input, shell to worker, as the core reads it.
#[derive(Clone, Debug)]
pub struct InputFrame {
    pub seq: u32,
    pub t_us: i64,
    pub steer_x: i16,
    pub steer_y: i16,
    pub pulse_held: bool,
    pub pulse_release: bool,
    pub wheel: i16,
    pub depth_key: i8,
    pub toggle_still: bool,
    pub pause: bool,
    pub inspect: Option<InspectRequest>,
    pub advance_steps: Option<u16>,
}

/// One optional inspection a frame asks for, exactly as ARCHITECTURE.md shapes
/// it: a target, the perturbation kind when the target is a perturbation, and
/// the kind's parameter or null for its own resolved default.
///
/// A profile is inspected because it was asked for and never otherwise. The
/// request is processed only in `still` and ignored in every other mode, which
/// is what keeps ordinary play free of readings it never asked to see.
#[derive(Clone, Debug)]
pub struct InspectRequest {
    pub target: InspectTarget,
    pub kind: Option<String>,
    pub parameter: Option<u32>,
}

/// The four targets of the closed set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InspectTarget {
    /// The eight recorded-window coordinates.
    Coordinates,
    /// The whole profile: the eight, and the two the replays pay for.
    CoordinatesFull,
    /// One perturbation of the named kind.
    Perturbation,
    /// The Handoff: control moves to the Form the parameter names.
    ///
    /// The one target that changes the run rather than reading it. It rides
    /// this field because ARCHITECTURE.md's Handoff locks it here — the
    /// existing command surface, the strategy-layer register, no frame change
    /// and no new command — and because the `PlanCommand` union is closed at
    /// five variants. Like every other target it is answered only in `still`.
    Handoff,
}

impl InspectTarget {
    pub fn name(self) -> &'static str {
        match self {
            InspectTarget::Coordinates => "coordinates",
            InspectTarget::CoordinatesFull => "coordinates_full",
            InspectTarget::Perturbation => "perturbation",
            InspectTarget::Handoff => "handoff",
        }
    }
}

impl InspectRequest {
    /// Reads one request, held to the closed target set, the closed kind set,
    /// and the shape's own keys.
    fn read(value: &Json) -> Result<Self, Fault> {
        crate::read::exact_keys(value, "inspect", &["kind", "parameter", "target"])?;
        let target = match crate::read::one_of(
            value,
            "target",
            &["coordinates", "coordinates_full", "perturbation", "handoff"],
        )? {
            0 => InspectTarget::Coordinates,
            1 => InspectTarget::CoordinatesFull,
            2 => InspectTarget::Perturbation,
            _ => InspectTarget::Handoff,
        };
        let kind = match value.get("kind") {
            Some(Json::Null) => None,
            Some(Json::Text(name)) => Some(name.clone()),
            _ => return Err(Fault::field("kind")),
        };
        // A perturbation names its kind and the other two targets name none:
        // a request that disagrees with its own target is refused rather than
        // half read.
        match (target, &kind) {
            (InspectTarget::Perturbation, Some(name))
                if crate::perturb::KINDS.contains(&name.as_str()) => {}
            (InspectTarget::Perturbation, _) => return Err(Fault::field("kind")),
            (_, Some(_)) => return Err(Fault::field("kind")),
            (_, None) => {}
        }
        let parameter = match value.get("parameter") {
            Some(Json::Null) => None,
            Some(Json::Int(held)) if (0..=i64::from(u32::MAX)).contains(held) => {
                Some(*held as u32)
            }
            _ => return Err(Fault::field("parameter")),
        };
        // A Handoff names the Form taking control, so its parameter is the one
        // the target requires rather than one it may default: a request that
        // names no Form is a request that disagrees with its own target, read
        // here exactly as the kind cross-check above reads one.
        if target == InspectTarget::Handoff && parameter.is_none() {
            return Err(Fault::field("parameter"));
        }
        Ok(InspectRequest { target, kind, parameter })
    }
}

impl InputFrame {
    /// Reads and validates one frame. Every declared field is present, because
    /// canonical JSON has no absent-versus-null ambiguity.
    pub fn read(body: &Json) -> Result<Self, Fault> {
        if !body.is_map() {
            return Err(Fault::because(Code::Validation, "body_not_an_object"));
        }
        // Every declared field is present and no key beyond them: canonical
        // JSON has no absent-versus-null ambiguity, and a key the shape never
        // declared is a body the sender got wrong.
        crate::read::exact_keys(
            body,
            "body",
            &[
                "advance_steps",
                "depth_key",
                "inspect",
                "pause",
                "pulse_held",
                "pulse_release",
                "seq",
                "steer_x",
                "steer_y",
                "t_us",
                "toggle_still",
                "wheel",
            ],
        )?;
        let seq = read_int(body, "seq", 1, i64::from(u32::MAX))? as u32;
        // The shell rAF timestamp, in whole microseconds. The worker has
        // already resolved it into a step count by the time the core reads the
        // frame, so nothing here counts steps with it — but the mode table's
        // ramps are stated in real time, and this is the only real time the
        // core ever sees, so the ramp clock reads it under the accumulator's
        // own clamp.
        let t_us = read_int(body, "t_us", i64::MIN, i64::MAX)?;
        let steer_x = read_int(body, "steer_x", -STEER_LIMIT, STEER_LIMIT)? as i16;
        let steer_y = read_int(body, "steer_y", -STEER_LIMIT, STEER_LIMIT)? as i16;
        if i64::from(steer_x) * i64::from(steer_x) + i64::from(steer_y) * i64::from(steer_y)
            > STEER_LIMIT * STEER_LIMIT
        {
            return Err(Fault::field("steer_x"));
        }
        let pulse_held = read_bool(body, "pulse_held")?;
        let pulse_release = read_bool(body, "pulse_release")?;
        // The raw wheel delta sum since the previous frame, and the bracket
        // keys. The core thresholds both into one resolved depth change.
        let wheel = read_int(body, "wheel", -WHEEL_LIMIT, WHEEL_LIMIT)? as i16;
        let depth_key = read_int(body, "depth_key", -1, 1)? as i8;
        // The one edge that moves the mode: Space, as the locked bindings put
        // it. What it does is the mode table's business, resolved once per
        // frame beside the ramp it starts.
        let toggle_still = read_bool(body, "toggle_still")?;
        let pause = read_bool(body, "pause")?;
        // An inspect request is validated wherever it arrives and processed only
        // in `still`: a malformed body is the sender's fault whatever mode the
        // run stands in, and a well-formed one asked for while the Field is
        // moving is simply not answered.
        let inspect = match body.get("inspect") {
            Some(Json::Null) => None,
            Some(value) if value.is_map() => Some(InspectRequest::read(value)?),
            _ => return Err(Fault::field("inspect")),
        };
        let advance_steps = match body.get("advance_steps") {
            Some(Json::Null) => None,
            Some(Json::Int(value)) if (0..=MAX_ADVANCE_STEPS).contains(value) => {
                Some(*value as u16)
            }
            _ => return Err(Fault::field("advance_steps")),
        };
        Ok(InputFrame {
            seq,
            t_us,
            steer_x,
            steer_y,
            pulse_held,
            pulse_release,
            wheel,
            depth_key,
            toggle_still,
            pause,
            inspect,
            advance_steps,
        })
    }

    /// The control state a step consumes from this frame, under the depth
    /// change and the release edge already resolved for the batch.
    ///
    /// The two edges of a frame are resolved by the caller and the two levels
    /// are carried as they arrived: `steer_x`, `steer_y`, and `pulse_held` are
    /// levels, so every step of a batch consumes them, and a batch that covers
    /// 200 ms of held time charges 200 ms of held time.
    pub fn control(&self, depth_move: i8, pulse_release: bool) -> ControlState {
        ControlState {
            steer_x: self.steer_x,
            steer_y: self.steer_y,
            pulse_held: self.pulse_held,
            pulse_release,
            depth_move,
        }
    }

    /// Whether this frame asks only for the current frozen render snapshot.
    pub fn is_passive_snapshot(&self) -> bool {
        self.steer_x == 0
            && self.steer_y == 0
            && !self.pulse_held
            && !self.pulse_release
            && self.wheel == 0
            && self.depth_key == 0
            && !self.toggle_still
            && !self.pause
            && self.inspect.is_none()
            && self.advance_steps == Some(0)
    }
}

/// One loaded run: the authoritative state, the mode, the queue, and the cues
/// the newest frame's steps raised.
///
/// The cues are the one thing here that is not authoritative: they are
/// render-only, they are never serialized, and they last exactly one frame —
/// cleared as a frame arrives, filled by the steps it runs, and read by the
/// render snapshot that acknowledges it.
///
/// Nothing else here stands outside the payload, and the depth resolution is
/// held to that deliberately: a run carries on from the payload and the frames
/// that follow it, and from nothing else. An `export_run` is reachable at any
/// instant, so state the record could not carry would be state a restore could
/// not land on.
#[derive(Clone, Debug)]
pub struct Run {
    state: RunState,
    mode: Mode,
    /// Whether the active attempt branch has been closed by an explicit return
    /// to the contract ladder. Session authority only; the immutable browser
    /// evidence record owns durable closure and resume creates a new branch.
    returned: bool,
    form: String,
    last_seq: u32,
    queue: PlanQueue,
    cues: Vec<Cue>,
    /// The events the steps of the newest frame raised, canonical JSON, in the
    /// order their causes occurred. Drained by the session that posts them.
    events: Vec<String>,
    /// Records the authored sequence asked for, each taken at the exact step
    /// that asked: the `save_key`, the kind of record, and the payload beside
    /// them. The session owns the store, so it writes them; the payload is
    /// taken here so an Anchor names the state the completion stood at rather
    /// than the state the batch ended at.
    pending_records: Vec<(String, RecordKind, String)>,
    /// The 1-based position of the standing objective in the chapter's authored
    /// order, as the render snapshot's header carries it. Derived, render-only,
    /// and never serialized — the id it stands for is what the payload holds.
    objective_ordinal: u16,
    /// How far the standing ramp has come, in microsecond-times-30 units, and 0
    /// while no ramp stands.
    ///
    /// Session state rather than payload state, exactly as the mode itself is:
    /// a restore lands in `running` with the queue and the accumulator cleared,
    /// which is the locked answer to what a record does with a run that was
    /// mid-`still`. Nothing here is serialized, and nothing here changes a byte
    /// of the run — the ramp decides how wall time maps onto a step count, and
    /// that is the accumulator's own kind of state.
    ramp: i64,
    /// The timestamp the previous frame carried, and none before the first one
    /// or after a pause. The accumulator holds the same value in the worker,
    /// resolved from the same field of the same frames.
    t_prev_us: Option<i64>,
    /// The Echo a committed change left, waiting for the exit that shows it.
    ///
    /// ARCHITECTURE.md locks both ends of this: the highlight is derived at
    /// commit — from the perturbation the committed change matches, run against
    /// the pre-commit standing View, or from the adopted candidate's evaluation
    /// record — and the `review_ready` carrying it is emitted at Still Mode
    /// exit, when the ramp completes. So it is held here between the two, and
    /// like the mode and the ramp it is session state: no record carries a
    /// pending Echo, and a restore lands in `running` with none.
    pending_echo: Option<crate::perturb::EchoHighlight>,
    /// The mode a standing pause interrupted, and none while none stands.
    ///
    /// Session state beside the ramp, and never serialized for the same reason:
    /// a record carries no mode, so a restore still lands in `running` whatever
    /// this held. What it is for is the window that goes away mid-inspection —
    /// a blur is not a decision to stop reading the Field, and coming back to a
    /// moving one would spend the pause the player was standing in.
    interrupted: Option<Mode>,
}

impl Run {
    /// Opens a new run under a run key, a starting Form, and the content hash
    /// the build computed over the authored content this run stands on.
    pub fn start(run_id: &str, form: &str, content_hash: &str) -> Result<Self, Fault> {
        Self::start_in_regime(run_id, form, content_hash, "open_field")
    }

    pub fn start_in_regime(
        run_id: &str,
        form: &str,
        content_hash: &str,
        regime: &str,
    ) -> Result<Self, Fault> {
        let scenario = ScenarioSpec::commissioning(
            content_hash.to_string(),
            crate::pressure::Schedule::default(),
            RegimeSpec::named(regime)?,
            GeneratorSpec::empty(),
            Vec::new(),
        );
        Self::start_with_scenario(run_id, form, scenario)
    }

    /// Opens a new run under a fully frozen scenario specification.
    pub fn start_with_scenario(
        run_id: &str,
        form: &str,
        scenario: ScenarioSpec,
    ) -> Result<Self, Fault> {
        let kind = if scenario.contract_id().is_some() {
            crate::state::RunKind::AutomationContract
        } else {
            crate::state::RunKind::OpenField
        };
        Self::start_with_scenario_kind(run_id, form, scenario, kind)
    }

    /// Opens a new run with an explicit product-path discriminator.
    pub fn start_with_scenario_kind(
        run_id: &str,
        form: &str,
        scenario: ScenarioSpec,
        run_kind: crate::state::RunKind,
    ) -> Result<Self, Fault> {
        if !is_hex(run_id, 16) {
            return Err(Fault::field("run_id"));
        }
        if !FORMS.contains(&form) {
            return Err(Fault::field("form"));
        }
        if !is_hex(scenario.content_hash(), 64) {
            return Err(Fault::field("content_hash"));
        }
        // A new run opens on the first branch, so its stream is the trajectory
        // split of the run's own root.
        let branch_nonce = 0;
        let now = FieldState::opening();
        let criterion = scenario
            .criterion(0)
            .map(|_| crate::criterion::CriterionRuntime::opening(0));
        let (attempt, attempt_branch) = match run_kind {
            crate::state::RunKind::AutomationContract => {
                let contract_id = scenario
                    .contract_id()
                    .ok_or_else(|| Fault::field("run_kind"))?
                    .to_string();
                let assembly_hash = scenario
                    .assembly_template_hash()
                    .ok_or_else(|| Fault::field("assembly_template"))?
                    .to_string();
                let generator_hash = scenario.generator().specification_hash();
                let attempt = crate::state::AttemptRecord::new(
                    run_id.to_string(),
                    contract_id,
                    scenario.content_hash().to_string(),
                    generator_hash.clone(),
                    assembly_hash.clone(),
                    crate::state::AttemptSource::Opened,
                )?;
                let branch = crate::state::AttemptBranchRecord::new(
                    run_id.to_string(),
                    None,
                    crate::state::BranchOperation::Opening,
                    generator_hash,
                    assembly_hash,
                    branch_nonce,
                )?;
                (Some(attempt), Some(branch))
            }
            crate::state::RunKind::OpenField | crate::state::RunKind::LegacyCampaign => {
                if scenario.contract_id().is_some() {
                    return Err(Fault::field("run_kind"));
                }
                (None, None)
            }
        };
        let state = RunState {
            run_id: run_id.to_string(),
            run_kind,
            attempt,
            attempt_branch,
            rng: trajectory_stream(run_id, branch_nonce),
            scenario,
            criterion,
            branch_nonce,
            progress: Progress::opening(),
            qualification_request: None,
            trace: Trace::opening(now.clone()),
            now,
            view: ViewDeclaration::opening(),
            slate: None,
            input_config: InputConfig::default_config(),
            pressures: Vec::new(),
            anchors: Vec::new(),
        };
        Ok(Run {
            state,
            mode: Mode::Still,
            returned: false,
            form: form.to_string(),
            last_seq: 0,
            queue: PlanQueue::new(),
            cues: Vec::new(),
            events: Vec::new(),
            pending_records: Vec::new(),
            objective_ordinal: 0,
            ramp: 0,
            t_prev_us: None,
            pending_echo: None,
            interrupted: None,
        })
    }

    /// Opens a run on a state read back from a save payload.
    ///
    /// The one post-restore normalization the document locks is applied here
    /// and nowhere else: `prev_assembly_step` becomes the step the run returned
    /// to. The stored bytes are never edited, and the hash verification that
    /// admitted them ran before this, so identical restore inputs still yield
    /// byte-equivalent state.
    ///
    /// A successful editable restore lands in `running` with the time scale
    /// full and the plan queue cleared, whatever mode the run was in when it
    /// was written; the shell re-enters Still Mode explicitly if it wants it.
    /// A qualification-frozen restore lands in `still` because causal frames
    /// are closed and only passive snapshot carriage remains legal. The frame
    /// counter is restored in either case, while a fresh worker session numbers
    /// its frame sequence from 1.
    pub fn restore(mut state: RunState, form: &str) -> Result<Self, Fault> {
        state.coherent()?;
        state.now.prev_assembly_step = Some(state.now.step);
        let mode = if state.qualification_request.is_some() {
            Mode::Still
        } else {
            Mode::Running
        };
        Ok(Run {
            state,
            mode,
            returned: false,
            form: form.to_string(),
            last_seq: 0,
            queue: PlanQueue::new(),
            cues: Vec::new(),
            events: Vec::new(),
            pending_records: Vec::new(),
            objective_ordinal: 0,
            ramp: 0,
            t_prev_us: None,
            pending_echo: None,
            interrupted: None,
        })
    }

    /// Re-roots the run on a fresh branch: the nonce the caller supplies, and
    /// the trajectory stream at position zero of that branch's own stream. The
    /// physical state is untouched, which is what makes the readings re-draw
    /// rather than the Field change.
    pub fn rebranch(&mut self, branch_nonce: u32) -> Result<(), Fault> {
        let attempt_branch = self.descendant_attempt_branch(
            &self.state.scenario,
            branch_nonce,
            crate::state::BranchOperation::Rebranch,
        )?;
        self.state.branch_nonce = branch_nonce;
        self.state.rng = trajectory_stream(&self.state.run_id, branch_nonce);
        self.state.attempt_branch = attempt_branch;
        Ok(())
    }

    /// Continues one returned automation attempt as an explicit child branch.
    /// The embodied machine is unchanged; the retained trace and criterion
    /// window begin again at the addressed resume boundary.
    pub fn resume_commission(&mut self) -> Result<(), Fault> {
        if self.state.run_kind != crate::state::RunKind::AutomationContract {
            return Err(Fault::field("run_kind"));
        }
        if !self.returned {
            return Err(Fault::field("attempt_branch"));
        }
        let branch_nonce = self
            .state
            .branch_nonce
            .checked_add(1)
            .ok_or_else(|| Fault::field("branch_nonce"))?;
        let attempt_branch = self.descendant_attempt_branch(
            &self.state.scenario,
            branch_nonce,
            crate::state::BranchOperation::Resume,
        )?;
        self.end_window();
        self.state.branch_nonce = branch_nonce;
        self.state.rng = trajectory_stream(&self.state.run_id, branch_nonce);
        self.state.attempt_branch = attempt_branch;
        self.state.trace.keyframe = self.state.now.clone();
        self.state.trace.start_step = self.state.now.step;
        self.state.criterion = self
            .state
            .scenario
            .criterion(self.state.progress.chapter_index)
            .map(|_| crate::criterion::CriterionRuntime::opening(self.state.now.step));
        self.state.qualification_request = None;
        self.returned = false;
        Ok(())
    }

    /// Closes live authority for the current automation branch while the
    /// contract ladder owns navigation. No causal state or identity changes.
    pub fn return_commission(&mut self) -> Result<(), Fault> {
        if self.state.run_kind != crate::state::RunKind::AutomationContract {
            return Err(Fault::field("run_kind"));
        }
        if self.returned {
            return Err(Fault::field("attempt_branch"));
        }
        self.returned = true;
        Ok(())
    }

    /// Seals one exact qualification request against the current Commission
    /// branch. The machine does not advance and no trial is executed here.
    pub fn freeze_qualification_request(
        &mut self,
        request: crate::state::QualificationRequest,
    ) -> Result<(), Fault> {
        if self.state.run_kind != crate::state::RunKind::AutomationContract {
            return Err(Fault::field("run_kind"));
        }
        if self.returned || self.mode != Mode::Still {
            return Err(Fault::field("qualification_request"));
        }
        if let Some(existing) = &self.state.qualification_request {
            if existing.request_id() == request.request_id() {
                return Ok(());
            }
            return Err(Fault::field("qualification_request"));
        }
        self.state.qualification_request = Some(request);
        if let Err(fault) = self.state.coherent() {
            self.state.qualification_request = None;
            return Err(fault);
        }
        Ok(())
    }

    /// Advances one hands-off analysis step through the exact live transition.
    /// The returned ledger is observational; the cloned run owns every change.
    pub fn analysis_step(&mut self) -> field::Ledger {
        let before = self.state.now.step;
        let ledger = self.step_once(ControlState::default(), None, false);
        debug_assert_eq!(self.state.now.step, before.saturating_add(1));
        ledger
    }

    /// Advances one cold-path step under the scenario's declared external
    /// control source. The external-control flag marks an intervention applied
    /// immediately before this step for the criterion's hands-off gate.
    pub fn analysis_step_with(
        &mut self,
        control: ControlState,
        other_external_control: bool,
    ) -> field::Ledger {
        let before = self.state.now.step;
        let ledger = self.step_once(control, None, other_external_control);
        debug_assert_eq!(self.state.now.step, before.saturating_add(1));
        ledger
    }

    /// Applies one typed intervention to a cloned analysis run without spending
    /// the live run's Impulse. Preconditions and mutation are the same plan
    /// projection used by Still Mode commits.
    pub fn analysis_apply(&mut self, plan: PlanCommand) -> Result<(), Fault> {
        let mut projected = crate::plan::Projection::of(&self.state.now);
        crate::plan::check(&plan, &projected).map_err(crate::plan::Refusal::fault)?;
        crate::plan::apply(&plan, &mut projected).map_err(crate::plan::Refusal::fault)?;
        self.end_window();
        self.state.now = projected.field;
        self.carry_keyframe();
        Ok(())
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    pub fn lifecycle(&self) -> &'static str {
        if self.returned {
            "returned"
        } else if self.state.qualification_request.is_some() {
            "qualification_frozen"
        } else {
            self.mode.name()
        }
    }

    pub fn state(&self) -> &RunState {
        &self.state
    }

    fn descendant_attempt_branch(
        &self,
        scenario: &ScenarioSpec,
        branch_nonce: u32,
        operation: crate::state::BranchOperation,
    ) -> Result<Option<crate::state::AttemptBranchRecord>, Fault> {
        if self.state.run_kind != crate::state::RunKind::AutomationContract {
            return Ok(None);
        }
        let attempt = self
            .state
            .attempt
            .as_ref()
            .ok_or_else(|| Fault::field("attempt_record"))?;
        let parent = self
            .state
            .attempt_branch
            .as_ref()
            .ok_or_else(|| Fault::field("attempt_branch"))?;
        let assembly_hash = scenario
            .assembly_template_hash()
            .ok_or_else(|| Fault::field("assembly_template"))?;
        crate::state::AttemptBranchRecord::new(
            attempt.attempt_id().to_string(),
            Some(parent.branch_id().to_string()),
            operation,
            scenario.generator().specification_hash(),
            assembly_hash.to_string(),
            branch_nonce,
        )
        .map(Some)
    }

    /// The starting Form this run opened with. Which Form stands in the Field,
    /// where, and with what parameters is authored content, so the Field a run
    /// opens with is established from a chapter rather than assumed here.
    pub fn form(&self) -> &str {
        &self.form
    }

    /// Establishes the Field a run stands on and the View it opens under,
    /// validating both against every locked cap, range, and ordering rule first.
    /// This is the seam authored content arrives through: the chapter's layers,
    /// Ports, Routes, Forms, currents, authored Boundary list, and opening View.
    ///
    /// The two arrive together because they are one chapter's content. The
    /// Field already carries its independent material compartment; the View is
    /// the passive observation declaration the chapter opens under.
    ///
    /// The trajectory's keyframe takes the same Field, because a keyframe holds
    /// the state as of its own step and this is the opening state of the run. A
    /// run that has already advanced holds a retained trajectory that the
    /// opening state cannot replace, so the Field is established before the
    /// first step or not at all.
    pub fn establish_field(
        &mut self,
        mut field: FieldState,
        view: ViewDeclaration,
    ) -> Result<(), Fault> {
        if self.state.scenario.contract_id().is_none() {
            self.state.scenario.regime().apply(&mut field);
        }
        let chapter_index = self.state.progress.chapter_index;
        let defaults = self.state.scenario.generator().route_defaults(chapter_index);
        if !defaults.is_empty() {
            Self::validate_route_defaults(&field, &defaults)?;
            field.route_controls = defaults;
        }
        field::validate(&field)?;
        field::establishable(&field)?;
        field::establishable_view(&view, &field)?;
        if !self
            .state
            .scenario
            .generator()
            .establishes_field(chapter_index, &field)
        {
            return Err(Fault::field("generator_spec"));
        }
        if field.step != 0 || self.state.now.step != 0 {
            return Err(Fault::field("step"));
        }
        self.state.trace = Trace::opening(field.clone());
        self.state.now = field;
        self.state.view = view;
        Ok(())
    }

    /// Re-roots Commission on the current contract's authored opening
    /// assembly while retaining the accepted generator and durable records.
    /// The branch advances only after the replacement Field and View have
    /// passed the same establishment and Route-default checks as a new run.
    pub fn restart_commission(
        &mut self,
        field: FieldState,
        view: ViewDeclaration,
    ) -> Result<(), Fault> {
        self.restart_commission_as(field, view, crate::state::BranchOperation::Restart)
    }

    fn prepared_assembly_field(
        &self,
        assembly: &crate::state::AssemblyTemplate,
    ) -> Result<FieldState, Fault> {
        self.prepared_scenario_assembly_field(&self.state.scenario, assembly, &self.state.view)
    }

    fn prepared_scenario_assembly_field(
        &self,
        scenario: &crate::state::ScenarioSpec,
        assembly: &crate::state::AssemblyTemplate,
        view: &ViewDeclaration,
    ) -> Result<FieldState, Fault> {
        if self.state.run_kind != crate::state::RunKind::AutomationContract {
            return Err(Fault::field("run_kind"));
        }
        let mut field = assembly
            .field()
            .cloned()
            .ok_or_else(|| Fault::field("assembly_template"))?;
        let chapter_index = self.state.progress.chapter_index;
        let defaults = scenario.generator().route_defaults(chapter_index);
        if !defaults.is_empty() {
            Self::validate_route_defaults(&field, &defaults)?;
            field.route_controls = defaults;
        }
        if !scenario.generator().local_policy().is_empty() {
            for form in &mut field.forms {
                form.controlled = false;
                form.focus = false;
                form.pulse_charge = 0;
            }
        }
        field::validate(&field)?;
        field::establishable(&field)?;
        field::establishable_view(view, &field)?;
        if !scenario.generator().establishes_field(chapter_index, &field) {
            return Err(Fault::field("generator_spec"));
        }
        Ok(field)
    }

    pub fn preview_restart_assembly(&self) -> Result<FieldState, Fault> {
        let assembly = self
            .state
            .scenario
            .assembly_template()
            .filter(|assembly| assembly.is_exact())
            .ok_or_else(|| Fault::field("assembly_template"))?;
        self.prepared_assembly_field(assembly)
    }

    pub fn restart_assembly(&mut self) -> Result<(), Fault> {
        let field = self.preview_restart_assembly()?;
        let view = self.state.view.clone();
        self.restart_commission_as(
            field,
            view,
            crate::state::BranchOperation::RestartAssembly,
        )
    }

    pub fn preview_generator_reconstruction(
        &self,
        generator: &crate::state::GeneratorSpec,
    ) -> Result<FieldState, Fault> {
        let assembly = self
            .state
            .scenario
            .assembly_template()
            .filter(|assembly| assembly.is_exact())
            .ok_or_else(|| Fault::field("assembly_template"))?;
        let scenario = self.state.scenario.with_generator(generator.clone())?;
        self.prepared_scenario_assembly_field(&scenario, assembly, &self.state.view)
    }

    pub fn preview_scenario_reconstruction(
        &self,
        scenario: &crate::state::ScenarioSpec,
        view: &ViewDeclaration,
    ) -> Result<FieldState, Fault> {
        let assembly = scenario
            .assembly_template()
            .filter(|assembly| assembly.is_exact())
            .ok_or_else(|| Fault::field("assembly_template"))?;
        self.prepared_scenario_assembly_field(scenario, assembly, view)
    }

    pub fn preview_assembly_revision(
        &self,
        draft: &crate::state::AssemblyDraft,
    ) -> Result<crate::state::AssemblyTemplate, Fault> {
        let current = self
            .state
            .scenario
            .assembly_template()
            .filter(|assembly| assembly.is_exact())
            .ok_or_else(|| Fault::field("assembly_template"))?;
        let candidate = current.adapted(draft)?;
        self.prepared_assembly_field(&candidate)?;
        Ok(candidate)
    }

    pub fn commit_assembly_revision(
        &mut self,
        assembly: crate::state::AssemblyTemplate,
    ) -> Result<(), Fault> {
        let field = self.prepared_assembly_field(&assembly)?;
        let view = self.state.view.clone();
        let prior = self.state.scenario.clone();
        self.state.scenario = prior.with_assembly_template(assembly)?;
        if let Err(fault) = self.restart_commission_as(
            field,
            view,
            crate::state::BranchOperation::AssemblyCommit,
        ) {
            self.state.scenario = prior;
            return Err(fault);
        }
        Ok(())
    }

    fn restart_commission_as(
        &mut self,
        mut field: FieldState,
        view: ViewDeclaration,
        operation: crate::state::BranchOperation,
    ) -> Result<(), Fault> {
        if self.state.run_kind != crate::state::RunKind::AutomationContract {
            return Err(Fault::field("run_kind"));
        }
        let chapter_index = self.state.progress.chapter_index;
        if self.state.scenario.contract_id().is_none() {
            self.state.scenario.regime().apply(&mut field);
        }
        let defaults = self.state.scenario.generator().route_defaults(chapter_index);
        if !defaults.is_empty() {
            Self::validate_route_defaults(&field, &defaults)?;
            field.route_controls = defaults;
        }
        if !self.state.scenario.generator().local_policy().is_empty() {
            for form in &mut field.forms {
                form.controlled = false;
                form.focus = false;
                form.pulse_charge = 0;
            }
        }
        field::validate(&field)?;
        field::establishable(&field)?;
        field::establishable_view(&view, &field)?;
        if !self
            .state
            .scenario
            .generator()
            .establishes_field(chapter_index, &field)
        {
            return Err(Fault::field("generator_spec"));
        }
        let branch_nonce = self
            .state
            .branch_nonce
            .checked_add(1)
            .ok_or_else(|| Fault::field("branch_nonce"))?;
        let attempt_branch = self.descendant_attempt_branch(
            &self.state.scenario,
            branch_nonce,
            operation,
        )?;
        let impulse = self.state.progress.impulse;

        self.end_window();
        self.state.branch_nonce = branch_nonce;
        self.state.rng = trajectory_stream(&self.state.run_id, branch_nonce);
        self.state.attempt_branch = attempt_branch;
        self.state.progress = Progress {
            chapter_index,
            objective: crate::state::ObjectiveState::hidden(),
            complete: Vec::new(),
            impulse,
        };
        self.state.criterion = self
            .state
            .scenario
            .criterion(chapter_index)
            .map(|_| crate::criterion::CriterionRuntime::opening(0));
        self.state.trace = Trace::opening(field.clone());
        self.state.now = field;
        self.state.view = view;
        self.state.qualification_request = None;
        self.state.slate = None;
        self.state.pressures.clear();
        self.mode = Mode::Still;
        self.queue = PlanQueue::new();
        self.cues.clear();
        self.events.clear();
        self.objective_ordinal = 0;
        self.ramp = 0;
        self.t_prev_us = None;
        self.pending_echo = None;
        self.interrupted = None;
        Ok(())
    }

    pub fn full_contract_reset(
        &mut self,
        field: FieldState,
        view: ViewDeclaration,
        scenario: crate::state::ScenarioSpec,
    ) -> Result<(), Fault> {
        let prior = self.state.scenario.clone();
        self.state.scenario = scenario;
        if let Err(fault) = self.restart_commission_as(
            field,
            view,
            crate::state::BranchOperation::FullContractReset,
        ) {
            self.state.scenario = prior;
            return Err(fault);
        }
        Ok(())
    }

    /// Opens the authored sequence on a chapter the run stands on: the chapter
    /// the shell is told about, and the objective the run holds when it starts.
    ///
    /// A run that opens on a state it has already advanced keeps whatever
    /// objective the record carried; only the events are raised again, because
    /// a fresh worker session has told the shell nothing.
    pub fn open_chapter(&mut self, chapter: &crate::content::Chapter, chapter_count: usize) {
        self.raise(
            "chapter_changed",
            &format!(
                "{{\"chapter_count\":{},\"chapter_index\":{},\"objective_count\":{},\"route_defaults\":{},\"title_key\":{},\"view\":{}}}",
                chapter_count,
                self.state.progress.chapter_index,
                chapter.objectives.len(),
                self.state
                    .scenario
                    .generator()
                    .route_defaults_written(self.state.progress.chapter_index),
                crate::fault::quoted(&chapter.title_key),
                self.state.view.written(),
            ),
        );
        crate::content::offer_opening(&mut self.state.progress, chapter, self.state.now.step);
        self.objective_ordinal =
            crate::content::objective_ordinal(Some(chapter), &self.state.progress);
        if !self.state.progress.objective.id.is_empty() {
            self.raise_objective(&self.state.progress.objective.written(), None);
        }
    }

    /// Seats the chapter's authored schedule as queued pressures.
    ///
    /// A schedule entry is a request: it stands in the list as `queued`, at the
    /// step the author named, and the stage machine admits it when its step
    /// comes and the limit leaves a seat. The list is built in the closed set's
    /// own order, which is the one order the payload and the event carry.
    ///
    /// Only a run at step 0 seats one, for the same reason only a run at step 0
    /// establishes a Field: a run that has advanced holds the list its own
    /// steps left, and a record carries it.
    pub fn open_schedule(&mut self, chapter: &crate::content::Chapter) {
        if self.state.now.step == 0 && self.state.pressures.is_empty() {
            self.seat_schedule(chapter, 0);
        }
        // A reopened run tells the fresh session what stands, exactly as the
        // chapter's own events are raised again on reopening: a restore that
        // lands mid-crisis would otherwise leave the surface showing nothing
        // where the run holds a staged pressure.
        if !self.state.pressures.is_empty() {
            self.raise_pressures();
        }
    }

    /// Seats one chapter's authored schedule, at the step the chapter opens on.
    ///
    /// An authored `start_step` is counted from the chapter's own opening,
    /// because a campaign's step counter is the run's and an author cannot know
    /// what it stands at when the third chapter opens. What the list carries is
    /// the step the run will admit at, which is the opening step plus the
    /// authored one — so the payload holds an absolute step, the stage machine
    /// reads it exactly as it always has, and the opening chapter, whose
    /// opening step is 0, is unchanged.
    fn seat_schedule(&mut self, chapter: &crate::content::Chapter, opening_step: u32) {
        let mut seated: Vec<crate::pressure::PressureState> = chapter
            .pressure_schedule
            .iter()
            .map(|entry| {
                crate::pressure::PressureState::queued_at(
                    entry.pressure,
                    opening_step.saturating_add(entry.start_step),
                    entry.primary,
                    entry.target,
                )
            })
            .collect();
        seated.sort_by_key(|held| held.pressure.ordinal());
        self.state.pressures = seated;
        // The opening boundary: an entry due for the chapter's first step is
        // admitted before it, through the same settlement every later
        // boundary uses — so a seat taken here is a stage entry exactly
        // as a turnover is, and its one-shots (Flood's hold, Drift's
        // move) land at the open rather than never. A chapter opens on an
        // empty window, which the settlement ends harmlessly.
        let mut cues = Vec::new();
        self.settle_pressures(&crate::pressure::Staged::default(), &mut cues);
        for cue in cues {
            field::raise_cue(&mut self.cues, cue);
        }
    }

    /// Settles the pressure list at the step boundary and applies the
    /// stage-entry one-shots: removals of what the step reported spent, the
    /// pressed floors, due admissions, Fracture's break at crisis entry,
    /// Flood's hold of a working target, and Drift's move of the targeted
    /// layer's current paths. Answers whether the list itself changed.
    ///
    /// Everything rides the committed-change machinery: the selections read
    /// the retained trace as it stands, the active window ends before
    /// anything applies — so every recorded step replays under the list, the
    /// Route set, and the geometry it actually ran under — and the trajectory
    /// restarts on the state the boundary leaves, exactly as a committed cut
    /// restarts it.
    fn settle_pressures(&mut self, staged: &crate::pressure::Staged, cues: &mut Vec<Cue>) -> bool {
        use crate::pressure::{Bound, Pressure, Stage, TargetKind};

        // What the settlement would do, learned on a copy so a boundary with
        // nothing to apply ends nothing: a due admission the limit refuses is
        // not a change, and neither is a step with no report at all.
        let next_step = self.state.now.step + 1;
        let would = {
            let mut copy = self.state.pressures.clone();
            crate::pressure::settle_boundary(
                &mut copy,
                self.state.scenario.pressure_schedule(),
                &staged.spent,
                &staged.pressed,
                next_step,
            )
        };

        // The stage entries the one-shots fall due on: the turnovers the
        // step derived, and the opening stage of every seat the settlement
        // will take.
        let mut entered: Vec<(u8, Stage)> = staged.entered.clone();
        entered.extend(would.admitted.iter().copied());

        // The selections, off the trace as it stands — the retained steps up
        // to 60, which within one window all name standing Routes.
        let mut breaks: Vec<u32> = Vec::new();
        let mut drifts: Vec<(u32, crate::state::Fx, u32)> = Vec::new();
        let mut holds: Vec<(u8, u32)> = Vec::new();
        for (ordinal, stage) in &entered {
            let Some(pressure) = self
                .state
                .pressures
                .iter()
                .find(|held| held.pressure.ordinal() == *ordinal)
            else {
                continue;
            };
            match pressure.pressure {
                Pressure::Fracture if *stage == Stage::Crisis => {
                    let selected = match pressure.target.kind {
                        TargetKind::Route => pressure
                            .target
                            .id
                            .filter(|route| {
                                self.state.now.routes.iter().any(|held| held.route == *route)
                            }),
                        TargetKind::None => self.trailing_flow_route(),
                        _ => None,
                    };
                    // A missing target — already cut, or no Route with
                    // positive trailing flow — breaks nothing, and the stages
                    // walk on.
                    if let Some(route) = selected {
                        breaks.push(route);
                    }
                }
                Pressure::Flood if pressure.target.kind == TargetKind::None => {
                    if let Some(node) = self.heaviest_throughput() {
                        holds.push((*ordinal, node));
                    }
                }
                Pressure::Drift => {
                    if let (TargetKind::Layer, Some(layer)) =
                        (pressure.target.kind, pressure.target.id)
                    {
                        // The level the entered stage carries, floored by a
                        // matching displaced floor — the effective level at
                        // the entry.
                        let held = self
                            .state
                            .scenario
                            .pressure_schedule()
                            .table(pressure.pressure)
                            .map_or(0, |table| table.level(*stage));
                        let floored = match pressure.displaced {
                            Some(floor) if floor.stage == *stage => held.min(floor.level),
                            _ => held,
                        };
                        let delta = crate::fx::fixed_mul(floored, 16_777_216);
                        if delta > 0 {
                            drifts.push((layer, delta, pressure.start_step % 4));
                        }
                    }
                }
                _ => {}
            }
        }

        let structural = !breaks.is_empty() || !drifts.is_empty();
        if !would.changed && !structural && holds.is_empty() {
            return false;
        }

        // The committed-change shape: the window ends under the old list and
        // the old Field, the changes land between steps, and the trajectory
        // restarts on the state they leave.
        self.end_window();
        let settled = crate::pressure::settle_boundary(
            &mut self.state.pressures,
            self.state.scenario.pressure_schedule(),
            &staged.spent,
            &staged.pressed,
            next_step,
        );
        debug_assert_eq!(settled, would, "a settlement applies what its own copy learned");
        for route in &breaks {
            let tail = self
                .state
                .now
                .routes
                .iter()
                .find(|held| held.route == *route)
                .map_or(0, |held| held.tail);
            self.state.now.routes.retain(|held| held.route != *route);
            // Cue 5, the same cue a committed cut raises, with the locked
            // payload: `a` the Route's own identifier, saturating, and `b`
            // the tail's Node.
            cues.push(Cue {
                kind: field::CUE_ROUTE_CUT,
                a: (*route).min(u32::from(u16::MAX)) as u16,
                b: tail,
            });
        }
        for (layer, delta, direction) in &drifts {
            drift_paths(&mut self.state.now, *layer, *delta, *direction);
        }
        for (ordinal, node) in &holds {
            if let Some(pressure) = self
                .state
                .pressures
                .iter_mut()
                .find(|held| !held.queued && held.pressure.ordinal() == *ordinal)
            {
                pressure.bound = Some(Bound { kind: TargetKind::Node, id: *node });
            }
        }
        self.state.trace.keyframe = self.state.now.clone();
        self.state.trace.start_step = self.state.now.step;
        settled.changed || !holds.is_empty()
    }

    /// The standing Route with the largest trailing flow — the sum of
    /// `f(r, t)` over the trace's retained steps up to 60 — smallest RouteId
    /// on equal sums, and none when no standing Route moved anything.
    fn trailing_flow_route(&self) -> Option<u32> {
        let steps = self.state.trace.steps.len();
        let recent = self.state.trace.steps.iter().skip(steps.saturating_sub(60));
        let mut sums: std::collections::BTreeMap<u32, i64> = std::collections::BTreeMap::new();
        for recorded in recent {
            for (route, flow) in &recorded.records.f {
                *sums.entry(*route).or_insert(0) += flow;
            }
        }
        let mut best: Option<(u32, i64)> = None;
        for route in &self.state.now.routes {
            let sum = sums.get(&route.route).copied().unwrap_or(0);
            if sum <= 0 {
                continue;
            }
            // Ascending RouteId, strictly-greater keeps the smallest on ties.
            if best.is_none_or(|(_, held)| sum > held) {
                best = Some((route.route, sum));
            }
        }
        best.map(|(route, _)| route)
    }

    /// The heaviest-throughput Node — the largest sum of Route inflow plus
    /// outflow over the trace's retained steps up to 60 — smallest NodeId on
    /// equal sums. Every recorded flow inside one window names a standing
    /// Route, so the endpoints resolve against the Route set as it stands.
    fn heaviest_throughput(&self) -> Option<u32> {
        let ends: std::collections::BTreeMap<u32, (u32, u32)> = self
            .state
            .now
            .routes
            .iter()
            .map(|route| (route.route, (route.tail, route.head)))
            .collect();
        let steps = self.state.trace.steps.len();
        let recent = self.state.trace.steps.iter().skip(steps.saturating_sub(60));
        let mut sums: std::collections::BTreeMap<u32, i64> = std::collections::BTreeMap::new();
        for recorded in recent {
            for (route, flow) in &recorded.records.f {
                let Some((tail, head)) = ends.get(route) else {
                    continue;
                };
                *sums.entry(*tail).or_insert(0) += flow;
                *sums.entry(*head).or_insert(0) += flow;
            }
        }
        let mut best: Option<(u32, i64)> = None;
        for port in &self.state.now.ports {
            let sum = sums.get(&port.node).copied().unwrap_or(0);
            // Ascending NodeId, strictly-greater keeps the smallest on ties.
            if best.is_none_or(|(_, held)| sum > held) {
                best = Some((port.node, sum));
            }
        }
        best.map(|(node, _)| node)
    }

    /// Tells the shell the whole list after a change, which is what the locked
    /// `pressure_changed` body carries.
    fn raise_pressures(&mut self) {
        let written = crate::pressure::written_list(&self.state.pressures);
        self.raise("pressure_changed", &format!("{{\"pressures\":{written}}}"));
    }

    /// The events the newest command raised, oldest first, and nothing after.
    pub fn take_events(&mut self) -> Vec<String> {
        std::mem::take(&mut self.events)
    }

    /// The records the authored sequence asked for, each with the kind it is
    /// and the payload taken at the step that asked.
    pub fn take_records(&mut self) -> Vec<(String, RecordKind, String)> {
        std::mem::take(&mut self.pending_records)
    }

    /// The identifier the next Anchor record takes, which is also its
    /// `save_key` suffix.
    fn next_anchor_id(&self) -> u32 {
        self.state.anchors.iter().map(|anchor| anchor.anchor_id).max().unwrap_or(0) + 1
    }

    fn raise(&mut self, name: &str, body: &str) {
        self.events.push(format!(
            "{{\"body\":{body},\"ev\":{},\"step\":{}}}",
            crate::fault::quoted(name),
            self.state.now.step,
        ));
    }

    fn raise_objective(&mut self, objective: &str, previous: Option<&str>) {
        let previous = match previous {
            Some(id) => crate::fault::quoted(id),
            None => "null".to_string(),
        };
        self.raise(
            "objective_changed",
            &format!("{{\"objective\":{objective},\"previous_id\":{previous}}}"),
        );
    }

    /// The completed-step counter.
    pub fn step(&self) -> u32 {
        self.state.now.step
    }

    pub fn queue(&self) -> &PlanQueue {
        &self.queue
    }

    /// Compatibility adapter for the former policy-only command. A caller
    /// crossing this boundary still produces one complete Design revision by
    /// retaining the current chapter's committed Route defaults, or freezing
    /// its embodied controls when migrating a legacy generator.
    pub fn install_local_policy(
        &mut self,
        policy: crate::policy::FrozenLocalPolicy,
    ) -> Result<(), Fault> {
        let chapter_index = self.state.progress.chapter_index;
        let mut route_defaults =
            self.state.scenario.generator().route_defaults(chapter_index);
        if route_defaults.is_empty() {
            route_defaults = self.state.now.route_controls.clone();
        }
        self.install_design_patch(policy, route_defaults)
    }

    /// Installs one complete Design revision. Policy and Route defaults are
    /// validated against the same paused Field before any retained state or
    /// generator identity changes.
    pub fn install_design_patch(
        &mut self,
        policy: crate::policy::FrozenLocalPolicy,
        route_defaults: Vec<crate::policy::RouteControlState>,
    ) -> Result<(), Fault> {
        Self::validate_local_policy(&self.state.now, &policy)?;
        Self::validate_route_defaults(&self.state.now, &route_defaults)?;
        let chapter_index = self.state.progress.chapter_index;
        let generator = self.state.scenario.generator().with_design(
            chapter_index,
            policy,
            route_defaults.clone(),
        )?;
        self.install_generator_revision(
            generator,
            route_defaults,
            crate::state::BranchOperation::DesignCommit,
        )
    }

    pub fn revert_generator(
        &mut self,
        field: FieldState,
        view: ViewDeclaration,
        scenario: crate::state::ScenarioSpec,
    ) -> Result<(), Fault> {
        let prior = self.state.scenario.clone();
        self.state.scenario = scenario;
        if let Err(fault) = self.restart_commission_as(
            field,
            view,
            crate::state::BranchOperation::RevertGenerator,
        ) {
            self.state.scenario = prior;
            return Err(fault);
        }
        Ok(())
    }

    pub fn clone_blueprint_generator(
        &mut self,
        generator: crate::state::GeneratorSpec,
    ) -> Result<(), Fault> {
        let route_defaults = generator.route_defaults(self.state.progress.chapter_index);
        self.install_generator_revision(
            generator,
            route_defaults,
            crate::state::BranchOperation::CloneBlueprint,
        )
    }

    fn install_generator_revision(
        &mut self,
        generator: crate::state::GeneratorSpec,
        route_defaults: Vec<crate::policy::RouteControlState>,
        operation: crate::state::BranchOperation,
    ) -> Result<(), Fault> {
        Self::validate_local_policy(&self.state.now, generator.local_policy())?;
        Self::validate_route_defaults(&self.state.now, &route_defaults)?;
        let chapter_index = self.state.progress.chapter_index;
        if !generator.accepts_field(chapter_index, &self.state.now) {
            return Err(Fault::field("generator_spec"));
        }
        let scenario = self.state.scenario.with_generator(generator)?;
        let attempt_branch = self.descendant_attempt_branch(
            &scenario,
            self.state.branch_nonce,
            operation,
        )?;
        let mut projected = self.state.now.clone();
        projected.route_controls = route_defaults;
        projected.policy_runtime.clear();
        if !scenario.generator().local_policy().is_empty() {
            for form in &mut projected.forms {
                form.controlled = false;
                form.focus = false;
                form.pulse_charge = 0;
            }
        }
        field::validate(&projected)?;

        self.end_window();
        self.state.scenario = scenario;
        self.state.attempt_branch = attempt_branch;
        self.state.now = projected;
        self.state.trace.keyframe = self.state.now.clone();
        self.state.trace.start_step = self.state.now.step;
        Ok(())
    }

    pub fn attach_engineering_transition_receipt(
        &mut self,
        receipt: crate::engineering::EngineeringTransitionReceipt,
    ) -> Result<(), Fault> {
        self.state
            .attempt_branch
            .as_mut()
            .ok_or_else(|| Fault::field("attempt_branch"))?
            .attach_transition_receipt(receipt)
    }

    /// Projects one complete policy against the current paused embodied
    /// snapshot without applying an action or changing any retained state.
    pub fn preview_local_policy(
        &self,
        policy: &crate::policy::FrozenLocalPolicy,
        route_defaults: &[crate::policy::RouteControlState],
        address: u32,
    ) -> Result<crate::policy::PolicyPreview, Fault> {
        let mut projected = self.state.now.clone();
        Self::validate_route_defaults(&projected, route_defaults)?;
        projected.route_controls = route_defaults.to_vec();
        Self::validate_local_policy(&projected, policy)?;
        crate::policy::preview(&projected, policy, address)
            .ok_or_else(|| Fault::field("address"))
    }

    fn validate_route_defaults(
        field: &FieldState,
        defaults: &[crate::policy::RouteControlState],
    ) -> Result<(), Fault> {
        let routes: Vec<u32> = field.routes.iter().map(|route| route.route).collect();
        let controlled: Vec<u32> = defaults.iter().map(|control| control.route).collect();
        if controlled != routes {
            return Err(Fault::field("route_defaults"));
        }
        for control in defaults {
            let route = field
                .routes
                .iter()
                .find(|route| route.route == control.route)
                .ok_or_else(|| Fault::field("route"))?;
            if control.controller != route.tail {
                return Err(Fault::field("controller"));
            }
            if control.capacity_limit < 0 || control.capacity_limit > route.capacity {
                return Err(Fault::field("capacity_limit"));
            }
            if control.allocation_weight == 0 {
                return Err(Fault::field("allocation_weight"));
            }
        }
        Ok(())
    }

    fn validate_local_policy(
        field: &FieldState,
        policy: &crate::policy::FrozenLocalPolicy,
    ) -> Result<(), Fault> {
        for component in policy.components() {
            if !field.ports.iter().any(|port| port.node == component.address) {
                return Err(Fault::field("address"));
            }
            for rule in &component.rules {
                Self::validate_policy_action(field, component.address, &rule.action)?;
                Self::validate_policy_condition(field, component.address, &rule.condition)?;
            }
            Self::validate_policy_action(field, component.address, &component.fallback)?;
        }
        Ok(())
    }

    fn validate_policy_condition(
        field: &FieldState,
        address: u32,
        condition: &crate::policy::LocalCondition,
    ) -> Result<(), Fault> {
        if let crate::policy::LocalCondition::TargetInRange { radius } = condition {
            if *radius > crate::field::pulse_radius(crate::state::FRAC_ONE) {
                return Err(Fault::field("radius"));
            }
        }
        let route = match condition {
            crate::policy::LocalCondition::RouteFlowBelow { route, .. }
            | crate::policy::LocalCondition::RouteFlowAbove { route, .. } => Some(*route),
            _ => None,
        };
        if let Some(route) = route {
            let attached = field.routes.iter().any(|held| {
                held.route == route && (held.tail == address || held.head == address)
            });
            if !attached {
                return Err(Fault::field("route"));
            }
        }
        Ok(())
    }

    fn validate_policy_action(
        field: &FieldState,
        address: u32,
        action: &crate::policy::LocalAction,
    ) -> Result<(), Fault> {
        if let crate::policy::LocalAction::Couple { radius } = action {
            if *radius > crate::field::pulse_radius(crate::state::FRAC_ONE) {
                return Err(Fault::field("radius"));
            }
        }
        if let crate::policy::LocalAction::SetRoute { route, capacity_limit, .. } = action {
            let Some(held) = field
                .routes
                .iter()
                .find(|held| held.route == *route && held.tail == address)
            else {
                return Err(Fault::field("route"));
            };
            if *capacity_limit > held.capacity {
                return Err(Fault::field("capacity_limit"));
            }
        }
        let mobile_action = matches!(
            action,
            crate::policy::LocalAction::SeekSupply { .. }
                | crate::policy::LocalAction::SeekPort { .. }
                | crate::policy::LocalAction::SeekSignal { .. }
                | crate::policy::LocalAction::ChangeDepth { .. }
                | crate::policy::LocalAction::Couple { .. }
                | crate::policy::LocalAction::UseAbility
        );
        if mobile_action && !field.forms.iter().any(|form| form.node == address) {
            return Err(Fault::field("action"));
        }
        Ok(())
    }

    /// Takes one frame of input: resolves the pause level and the mode, then
    /// runs exactly the steps the frame asks for, each consuming this frame's
    /// control state.
    ///
    /// The order inside one frame is the mode table's, and is worth stating
    /// because three things want the same instant:
    ///
    /// 1. **The pause level.** `pause: true` suspends the run whatever mode it
    ///    stood in, which is the table's own trigger, and the frame resolves
    ///    nothing else at all.
    /// 2. **The standing ramp**, advanced by the real time this frame carries,
    ///    and completed into `still` or `running` when it reaches its span.
    /// 3. **The toggle**, applied last, so a frame that starts a ramp spends
    ///    its own elapsed time at the scale it arrived under. The worker's
    ///    accumulator resolves that same frame at that same scale, which is
    ///    what keeps the two readings of one frame together.
    ///
    /// A run in `still` runs no step however many the frame asks for: the mode
    /// table calls it fully paused, and that is what makes direct movement
    /// inert there rather than merely unlikely.
    pub fn input_frame(
        &mut self,
        body: &Json,
        campaign: Option<&crate::content::Content>,
    ) -> Result<u32, Fault> {
        let frame = InputFrame::read(body)?;
        if frame.seq <= self.last_seq {
            let mut detail = String::new();
            let mut object = Obj::new(&mut detail);
            object.int("accepted", i64::from(self.last_seq));
            object.text("field", "seq");
            object.int("seq", i64::from(frame.seq));
            object.end();
            return Err(Fault::detailed(Code::Validation, detail));
        }
        self.last_seq = frame.seq;
        // A cue lasts one frame. The buffer is cleared as the frame that will
        // carry it arrives, filled by the steps that frame runs, and read by
        // the render snapshot that acknowledges it — a paused frame runs no
        // step and so carries none.
        self.cues.clear();

        if frame.pause {
            // A level pause suspends the simulation: the frame injects no
            // steps at all, accumulates nothing, and resolves nothing, which is
            // what makes a pause byte-neutral. The shell lets go of what it was
            // holding on the same rule, so nothing it had half offered is
            // played back into the run on its return.
            //
            // The mode table names one trigger for `suspended` and puts no
            // source state on it, so a pause takes a still run too — and the
            // release puts the run back in the mode the pause interrupted. A
            // blur is not a decision to stop reading the Field.
            //
            // A standing ramp is discarded rather than remembered, so a run
            // suspended mid-ramp releases into `running`: a ramp is a span of
            // real time, and a suspended run spends none of it. The remembered
            // mode is noted only on the edge, so a second paused frame does not
            // overwrite it with `suspended`.
            if self.mode != Mode::Suspended {
                self.interrupted = Some(match self.mode {
                    Mode::Still => Mode::Still,
                    _ => Mode::Running,
                });
            }
            self.mode = Mode::Suspended;
            self.ramp = 0;
            self.t_prev_us = None;
            return Ok(0);
        }
        if self.mode == Mode::Suspended {
            self.mode = self.interrupted.take().unwrap_or(Mode::Running);
        }

        let _elapsed = self.elapsed(&frame);
        if frame.toggle_still {
            self.toggle_still();
        }

        // Depth is resolved once per frame, before the catch-up loop, and only
        // on a frame that executes a step. A still run executes none, and
        // neither does an ended one: `run_completed` is the last thing a
        // campaign does, and a run that ran a step after it would be a run
        // carrying on past its own ending.
        let asked = if matches!(self.mode, Mode::Still | Mode::Ended) {
            0
        } else {
            frame.advance_steps.unwrap_or(0)
        };
        // A still run resolves no depth and accumulates no wheel delta either.
        //
        // The two are one guard on the mode rather than on the step count,
        // because they are different rules. A *frame* that runs no step defers
        // its depth change and keeps accumulating the wheel toward it, which is
        // the locked deferral and stands untouched. A *run* that is paused
        // takes no input at all — and `wheel_accum` is a payload field, so a
        // wheel turned over a Field being read would otherwise change the bytes
        // an export carries and be spent the moment the run started moving
        // again.
        let depth_move = if matches!(self.mode, Mode::Still | Mode::Ended) {
            0
        } else {
            self.resolve_depth(&frame, asked > 0)
        };
        let mut ran = 0u16;
        for index in 0..asked {
            // The first executed step of the batch records both resolved edges
            // and every later one records neither, so one frame yields at most
            // one depth change and at most one Pulse however many steps it
            // runs. The Pulse's release carries the depth idiom verbatim, which
            // is what bounds how much one release can gather.
            let first = index == 0;
            let control =
                frame.control(if first { depth_move } else { 0 }, first && frame.pulse_release);
            self.step_once(control, campaign, false);
            ran += 1;
            // A campaign that ended mid-batch runs no further step of it: the
            // frame asked for steps of a run that was still going, and the
            // ending is where the asking stops being answered. What the frame
            // reports is what it ran, so the count and the trajectory agree.
            if self.mode == Mode::Ended {
                break;
            }
        }
        // The optional inspection, answered last, so a frame that also moved
        // the mode reads the Field the mode left. A request outside `still` is
        // ignored exactly as the document says, and answers nothing at all.
        //
        // A Handoff is the one target that can refuse, and it refuses on the
        // `input_frame` error path. A still run runs no step, so the frame that
        // carries a refused Handoff ran nothing to take back: `asked` is 0 in
        // `still` by the mode table, and the refusal leaves the run exactly
        // where the frame found it.
        if self.mode == Mode::Still {
            if let Some(request) = frame.inspect {
                self.inspect(&request, campaign)?;
            }
        }
        Ok(u32::from(ran))
    }

    /// Moves control to one Form of the standing Field: the Handoff, whole.
    ///
    /// Immediate and free. The `controlled` flag leaves the Form it stood on
    /// and lands on the named one between steps, while the simulation stands at
    /// time scale 0 — no queue entry, no commit, and no Impulse, because
    /// SPEC.md's cost sentence prices changes to the Field and a Handoff
    /// changes no Route, no Boundary and no View.
    ///
    /// **It is a membership boundary.** A Handoff is a state mutation no
    /// control schedule derives, so the active window ends here exactly as a
    /// committed change ends one: every retained step is replayed onto the
    /// keyframe under the flags it actually ran under, and the trajectory
    /// restarts on the state the Handoff leaves. Without it a regeneration
    /// inside a window spanning this instant would steer the wrong Form for
    /// every step before it.
    ///
    /// The three refusals are the document's: `not_found` for a Form this run
    /// does not hold, and `validation` for a Form the chapter marks
    /// un-controllable or for a Handoff to the Form already controlled — a
    /// no-op is refused rather than silently absorbed.
    fn handoff(
        &mut self,
        id: u8,
        campaign: Option<&crate::content::Content>,
    ) -> Result<(), Fault> {
        let Some(place) = self.state.now.forms.iter().position(|form| form.id == id) else {
            let mut detail = String::new();
            let mut object = Obj::new(&mut detail);
            object.text("field", "form");
            object.int("id", i64::from(id));
            object.end();
            return Err(Fault::detailed(Code::NotFound, detail));
        };
        if self.state.now.forms[place].controlled {
            return Err(Fault::field("parameter"));
        }
        // The authored seam, read against the chapter the run stands in. It is
        // content-derived and never serialized, so it is read here — at the
        // moment the request arrives — rather than carried on the Form.
        //
        // With no chapter to read it against — a run carried on under content
        // whose hash has moved, which is what `content_changed` reports — there
        // is no authored gate, and the shape's own default stands rather than
        // its inverse: `controllable` absent means true. Refusing every Handoff
        // here would leave such a run unable to move control at all, and the
        // standing rule for changed content is that the run carries on.
        let allowed = match campaign
            .and_then(|content| content.chapter(self.state.progress.chapter_index))
        {
            Some(chapter) => chapter.controllable(id) == Some(true),
            None => true,
        };
        if !allowed {
            return Err(Fault::field("parameter"));
        }
        self.end_window();
        for form in &mut self.state.now.forms {
            form.controlled = false;
        }
        self.state.now.forms[place].controlled = true;
        // The keyframe is the state the boundary leaves, exactly as a commit
        // takes it: the moved flag is in the record every later replay starts
        // from, and no retained step spans the move.
        self.state.trace.keyframe = self.state.now.clone();
        self.state.trace.start_step = self.state.now.step;
        Ok(())
    }

    /// Answers one inspection: the coordinate profile, one perturbation of
    /// the standing View, or the Handoff.
    ///
    /// Both run outside a slate — slate position 0, the outside-a-slate
    /// context — because what is inspected is the View the run stands under
    /// rather than a candidate of the standing slate. `sigma_V` is the standing
    /// evaluation's own root when one stands, so a reading taken here and the
    /// slate beside it name their streams from the same root; with no slate
    /// assembled yet it is the root the next assembly would declare, which is
    /// what makes an inspection reproducible from the payload alone.
    fn inspect(
        &mut self,
        request: &InspectRequest,
        campaign: Option<&crate::content::Content>,
    ) -> Result<(), Fault> {
        // The Handoff stands before the reading guard: it moves control rather
        // than reading a View, so a Field with no Node and a run holding no
        // inside are not what refuses it.
        if request.target == InspectTarget::Handoff {
            let named = request.parameter.filter(|held| *held <= u32::from(u8::MAX));
            let Some(id) = named else {
                return Err(Fault::field("parameter"));
            };
            return self.handoff(id as u8, campaign);
        }
        if self.state.now.ports.is_empty() || self.state.view.inside.is_empty() {
            return Ok(());
        }
        let sigma = self.evaluation_sigma();
        let tau = self.tolerance();
        let view = self.state.view.clone();
        match request.target {
            InspectTarget::Coordinates => {
                let profile = crate::coord::of(&self.state, &view, tau);
                self.raise(
                    "review_ready",
                    &format!(
                        "{{\"review\":{{\"kind\":\"coordinates\",\"profile\":{}}}}}",
                        profile.written()
                    ),
                );
            }
            InspectTarget::CoordinatesFull => {
                let profile = crate::coord::full(&self.state, &view, &sigma, 0, tau);
                self.raise(
                    "review_ready",
                    &format!(
                        "{{\"review\":{{\"kind\":\"coordinates\",\"profile\":{}}}}}",
                        profile.written()
                    ),
                );
            }
            InspectTarget::Perturbation => {
                let Some(name) = request.kind.as_deref() else { return Ok(()) };
                let Some(asked) = crate::perturb::Request::of(name, request.parameter) else {
                    return Ok(());
                };
                let result = crate::perturb::run(
                    &self.state,
                    &view,
                    &[],
                    0,
                    &sigma,
                    tau,
                    asked,
                );
                self.raise(
                    "review_ready",
                    &format!(
                        "{{\"review\":{{\"kind\":\"perturbation\",\"result\":{}}}}}",
                        result.written()
                    ),
                );
            }
            // Answered above, before the reading guard.
            InspectTarget::Handoff => {}
        }
        Ok(())
    }

    /// The `sigma_V` an on-demand reading names its streams from: the standing
    /// evaluation's own root, and the root the next assembly would declare when
    /// no slate stands yet.
    fn evaluation_sigma(&self) -> crate::rng::RngState {
        match &self.state.slate {
            Some(slate) => slate.sigma,
            None => crate::slate::evaluation_stream(
                &self.state.run_id,
                self.state.branch_nonce,
                self.state.now.assembly_ordinal,
            ),
        }
    }

    /// The declared tolerance a reading runs under: the standing evaluation's,
    /// and the declared default before the first one.
    fn tolerance(&self) -> Frac {
        self.state.slate.as_ref().map_or(crate::slate::TAU_DEFAULT, |slate| slate.tau)
    }

    /// How much of a ramp this frame carries, in microsecond-times-30 units:
    /// the real time between this frame and the one before it, under the
    /// accumulator's own clamp and its own first-frame reading of zero.
    ///
    /// **Every frame is read this way, `advance_steps` or not.** The document
    /// says the test hook ignores `t_us` and runs exactly the steps it names,
    /// and that is a rule about the step count, which is all the hook replaces:
    /// the worker resolves every frame it forwards into a step count and fills
    /// `advance_steps` with it, so a core that read the field as "this frame
    /// carries no real time" would read every frame of ordinary play that way.
    /// The mode table states its ramps in real time, `t_us` is the only real
    /// time the core ever sees, and the arithmetic here is the accumulator's
    /// own on the same field of the same frames — so the ramp the core resolves
    /// and the scale the worker spends it at cannot come apart.
    fn elapsed(&mut self, frame: &InputFrame) -> i64 {
        let gap = match self.t_prev_us {
            Some(before) => frame.t_us.saturating_sub(before).clamp(0, MAX_GAP_US),
            None => 0,
        };
        self.t_prev_us = Some(frame.t_us);
        gap * 30
    }

    /// Carries a standing ramp forward, and completes it when it reaches its
    /// span: `ramp_in` into `still`, `ramp_out` into `running`. Nothing else
    /// moves a ramp, and nothing moves one that is not standing.
    fn advance_ramp(&mut self, elapsed: i64) {
        if !matches!(self.mode, Mode::RampIn | Mode::RampOut) {
            return;
        }
        self.ramp = self.ramp.saturating_add(elapsed.max(0)).min(RAMP_UNITS);
        if self.ramp < RAMP_UNITS {
            return;
        }
        self.ramp = 0;
        let entered = self.mode == Mode::RampIn;
        self.mode = if entered { Mode::Still } else { Mode::Running };
        // Entry into Still Mode is one of the two moments a slate is
        // assembled, and the analysis budget puts it exactly here: the frame
        // the entry ramp reaches scale 0 on, with the last completed step as
        // the evaluation step.
        if entered {
            self.assemble_slate();
            return;
        }
        // The other end of the Echo: the `review_ready` carrying the highlight
        // is emitted at Still Mode exit, when the ramp completes. One committed
        // change leaves one highlight, and it is shown once.
        if let Some(echo) = self.pending_echo.take() {
            self.raise(
                "review_ready",
                &format!("{{\"review\":{{\"kind\":\"echo\",\"echo\":{}}}}}", echo.written()),
            );
        }
    }

    /// Moves immediately between Design and Commission authority.
    ///
    /// The former presentation ramp made pausing feel like an actuator. In the
    /// automation product a pause is an authority boundary, so it takes effect
    /// on the frame that asks for it and consumes no simulation time.
    fn toggle_still(&mut self) {
        match self.mode {
            Mode::Running | Mode::RampOut => {
                self.mode = Mode::Still;
                self.ramp = 0;
                self.assemble_slate();
            }
            Mode::Still | Mode::RampIn => {
                self.mode = Mode::Running;
                self.ramp = 0;
                if let Some(echo) = self.pending_echo.take() {
                    self.raise(
                        "review_ready",
                        &format!("{{\"review\":{{\"kind\":\"echo\",\"echo\":{}}}}}", echo.written()),
                    );
                }
            }
            // A suspended run reads no toggle, because a frame carrying the
            // pause level resolves nothing else; an ended one moves no more.
            Mode::Suspended | Mode::Ended => {}
        }
    }

    /// Thresholds the wheel and the bracket keys into one depth change.
    ///
    /// The accumulator is held to the trigger distance and cleared when it
    /// fires; the cooldown holds off the next change for 15 executed steps.
    /// Both are authoritative state, because the change they resolve is
    /// recorded in the trajectory and a restore has to land mid-cooldown
    /// exactly where it left off.
    ///
    /// `executing` is whether this frame runs a step at all. A frame that runs
    /// none resolves no depth change and consumes no bracket press; the wheel
    /// delta it carries still accumulates, into the payload field that already
    /// holds it. Without that the resolution would fire into a batch with no
    /// step to record it and the change would be lost — the accumulator cleared
    /// and the cooldown started for a depth change that never happened. A
    /// rendered frame runs no step about half the time at the 60-frames-per-
    /// second target against 30 steps per second, so the loss would not be an
    /// edge: it would be every other gesture.
    ///
    /// The half of the deferral this does not do is the shell's, and belongs
    /// there: a press a stepless frame carried is offered again on the frames
    /// that follow, until one that executes a step consumes it. Holding it here
    /// instead would be state outside the payload — and an `export_run` taken
    /// between the two frames would then produce a record a restore could not
    /// land on, diverging from the run it was taken from with no byte to say
    /// why. The wheel needs no re-offer for exactly the reason this must not
    /// hold the press: `wheel_accum` is payload state already.
    ///
    /// Everything the rule does on a frame that executes a step is unchanged,
    /// the cooldown included: a bracket direction that arrives while the
    /// cooldown stands is spent against it.
    fn resolve_depth(&mut self, frame: &InputFrame, executing: bool) -> i8 {
        let field = &mut self.state.now;
        field.wheel_accum =
            (field.wheel_accum + i32::from(frame.wheel)).clamp(-WHEEL_TRIGGER, WHEEL_TRIGGER);
        if !executing || field.depth_cooldown > 0 {
            return 0;
        }
        if field.wheel_accum.abs() >= WHEEL_TRIGGER {
            let direction = if field.wheel_accum > 0 { 1 } else { -1 };
            field.wheel_accum = 0;
            field.depth_cooldown = DEPTH_COOLDOWN;
            return direction;
        }
        frame.depth_key
    }

    /// One step of exactly 1/30 s of simulated time.
    fn step_once(
        &mut self,
        control: ControlState,
        campaign: Option<&crate::content::Content>,
        other_external_control: bool,
    ) -> field::Ledger {
        // The chapter the run stands in, read out of the campaign the session
        // holds. It is read per step rather than per frame because a step can
        // be the one that carries the run into the next chapter, and the next
        // step of the same batch belongs to that one.
        let chapter = campaign.and_then(|content| {
            content.chapter(self.state.progress.chapter_index)
        });
        let mechanism_before = MechanismSnapshot::of(&self.state.now);
        let criterion_before = self.state.criterion.as_ref().map(|runtime| runtime.status());
        // The stream position before the step ran, which is what a replay of
        // this step reads. Nothing stochastic exists yet, so the position is
        // recorded and not advanced: the goals that add exogenous terms to
        // pressures and currents draw in the locked order.
        let position = self.state.rng;
        // Physical membership and its leakage coefficient live in the Field.
        // The active View is observation metadata and is deliberately absent
        // from this causal transition call.
        let local_policy = self.state.scenario.generator().local_policy().clone();
        let outcome = if local_policy.is_empty() {
            field::advance(
                &mut self.state.now,
                control,
                self.state.input_config.pointer_speed,
                &mut field::Staging {
                    pressures: &mut self.state.pressures,
                    schedule: self.state.scenario.pressure_schedule(),
                    stream: &mut self.state.rng,
                    medium: self.state.scenario.regime().medium_motion(),
                    supply_jitter: self.state.scenario.regime().supply_jitter(),
                },
            )
        } else {
            field::advance_programmed(
                &mut self.state.now,
                ControlState::default(),
                self.state.input_config.pointer_speed,
                &local_policy,
                &mut field::Staging {
                    pressures: &mut self.state.pressures,
                    schedule: self.state.scenario.pressure_schedule(),
                    stream: &mut self.state.rng,
                    medium: self.state.scenario.regime().medium_motion(),
                    supply_jitter: self.state.scenario.regime().supply_jitter(),
                },
            )
        };
        debug_assert!(
            outcome.ledger.balanced(),
            "the step's Charge deltas sum to zero across the ledger",
        );
        for body in MechanismSnapshot::of(&self.state.now).events_since(&mechanism_before) {
            self.raise("mechanism_event", &body);
        }
        let ledger = outcome.ledger.clone();
        if let Some(body) = charge_mechanism_event(self.state.now.step, &ledger) {
            self.raise("mechanism_event", &body);
        }
        let criterion_event = match (
            self.state
                .scenario
                .criterion(self.state.progress.chapter_index),
            self.state.criterion.as_mut(),
        ) {
            (Some(spec), Some(runtime)) => {
                let before = runtime.status();
                match runtime.advance(
                    spec,
                    crate::criterion::CriterionStepInput {
                        field: &self.state.now,
                        records: &outcome.records,
                        ledger: &outcome.ledger,
                        control,
                        other_external_control,
                    },
                ) {
                    Ok(reading)
                        if reading.status != before
                            || reading.observed_steps == 1
                            || self.state.now.step % 15 == 0 =>
                    {
                        Some(format!("{{\"criterion\":{}}}", reading.written()))
                    }
                    Ok(_) => None,
                    Err(_) => {
                        debug_assert!(false, "criterion runtime accepts contiguous live steps");
                        None
                    }
                }
            }
            _ => None,
        };
        if let Some(body) = criterion_event {
            self.raise("criterion_changed", &body);
        }
        let criterion_after = self.state.criterion.as_ref().map(|runtime| runtime.status());
        if criterion_after != criterion_before {
            if let Some(status) = criterion_after {
                let mut body = String::new();
                let mut object = Obj::new(&mut body);
                object.int(
                    "chapter_index",
                    i64::from(self.state.progress.chapter_index),
                );
                object.text("kind", "criterion");
                object.text("status", status.name());
                object.end();
                self.raise("mechanism_event", &body);
                if status == crate::criterion::CriterionStatus::Failed {
                    let mut failure = String::new();
                    let mut object = Obj::new(&mut failure);
                    object.int(
                        "chapter_index",
                        i64::from(self.state.progress.chapter_index),
                    );
                    object.text("kind", "failure");
                    object.text("source", "criterion");
                    object.end();
                    self.raise("mechanism_event", &failure);
                }
            }
        }
        // The authored sequence reads the step the Field just took, and writes
        // only into `progress` — the payload's own field — so what it keeps
        // between steps is what a restore lands on. It runs after the step and
        // before the step's cues are raised into the frame, so a completion's
        // own cue arrives in the same frame as the step that earned it.
        let mut cues = outcome.cues;
        let mut anchoring = false;
        let mut closing = false;
        if let Some(chapter) = chapter {
            // The step's own cues are what the script reads — a Pulse it
            // released is a cue and nothing else — and the cues it raises are
            // appended, so the two lists are held apart while it runs.
            let raised = cues.clone();
            let reading =
                crate::content::StepReading { field: &self.state.now, cues: &raised };
            let script = crate::content::advance_objectives(
                &mut self.state.progress,
                &self.state.now,
                chapter,
                &reading,
                &mut cues,
            );
            for (objective, previous) in &script.changed {
                self.raise_objective(&objective.written(), previous.as_deref());
            }
            if !script.changed.is_empty() {
                self.objective_ordinal =
                    crate::content::objective_ordinal(Some(chapter), &self.state.progress);
            }
            anchoring = script.anchor;
            closing = script.chapter_complete;
        }
        self.state.trace.steps.push_back(TraceStep {
            step: self.state.now.step,
            rng: position,
            ctl: control,
            records: outcome.records,
        });
        // The depth cooldown is counted in executed steps, floored at 0.
        self.state.now.depth_cooldown = self.state.now.depth_cooldown.saturating_sub(1);
        self.carry_keyframe();
        // The staging's boundary: everything the completed step derived but
        // could not apply — a removal, a pressed floor, a due admission, a
        // stage-entry one-shot — lands here, between steps, through the
        // committed-change shape: the active window ends first, under the
        // list and the Field every retained step actually ran under, and the
        // changes land on the state the ended window leaves. A stage turnover
        // alone is derived state, ends nothing, and only tells the shell.
        let told = self.settle_pressures(&outcome.staged, &mut cues);
        if told || outcome.staged.stage {
            self.raise_pressures();
        }
        // The campaign's own boundary, and the authored events that stand in
        // the same place: both land between two steps, after the step that
        // asked for them is closed and before the next one opens, through the
        // carriage a committed change and a pressure settlement already use.
        // A step that closed a chapter runs no event of it — the events of the
        // chapter the run has just entered are that chapter's, and its opening
        // objective has only just been offered.
        let mut carried = false;
        if closing {
            carried = self.close_chapter(campaign, &mut cues);
        } else if let Some(chapter) = chapter {
            self.settle_events(chapter, &mut cues);
        }
        field::synchronize_automation_state(&mut self.state.now);
        // The Anchor is written after the step is closed, and for the same
        // reason a record is checked before it is stored: the payload it holds
        // is the whole run state, and the run state is only coherent once the
        // step it just took is in the trajectory and the keyframe has been
        // carried past it. The step it records is still this one.
        //
        // A chapter transition writes one too, whatever the chapter's own
        // Anchor moments name: the document writes Anchors at major objective
        // completions **and chapter transitions**, and the payload it holds is
        // the chapter the run has entered — so a Quick Retry across the
        // boundary lands at the opening of the new chapter rather than at the
        // end of the old one.
        if anchoring || carried {
            self.write_anchor(&mut cues);
        }
        // The frame's own cap, under the same locked overflow policy the step
        // uses: what a frame drops is never the emission every other cue of it
        // is read against.
        for cue in cues {
            field::raise_cue(&mut self.cues, cue);
        }
        ledger
    }

    /// Closes the chapter the run has just completed: the next chapter opens,
    /// or the campaign ends on this one. Answers whether a chapter transition
    /// happened, which is what asks for the Anchor beside it.
    ///
    /// **What a transition carries, and what it does not.** The run carries:
    /// its key, its branch nonce, its trajectory stream position, its content
    /// hash, its Impulse, its completed objectives, its Anchor metadata, its
    /// input configuration, and its step counter — which is the campaign's and
    /// never restarts, because the autosave cadence, the trajectory, and every
    /// record are counted in it. The Field does not: the chapter that opens
    /// establishes its own Nodes, Routes, currents, layers, and Boundary from
    /// its own authored content, under the Form the run was opened on, exactly
    /// as the opening chapter did. So the standing View becomes the new
    /// chapter's authored opening View, the evaluation record is let go of —
    /// its candidates name Nodes that no longer stand — and the pressure list
    /// is replaced by the new chapter's schedule, seated at this step.
    ///
    /// **Why it is settled between steps.** The whole of it rides the carriage
    /// a committed change and a pressure boundary already use: the active
    /// window ends first, under the Field and the list every recorded step
    /// actually ran under, and the trajectory restarts on the state the
    /// transition leaves. So no retained step is ever replayed against a Field
    /// it did not run under, and no record can be taken mid-transition — a
    /// command answers between frames, and this lands between two steps.
    fn close_chapter(
        &mut self,
        campaign: Option<&crate::content::Content>,
        cues: &mut Vec<Cue>,
    ) -> bool {
        let Some(content) = campaign else {
            return false;
        };
        let next = self.state.progress.chapter_index.saturating_add(1);
        let Some(chapter) = content.chapter(next) else {
            self.complete_run(content);
            return false;
        };
        let Some(form) = content.form(&self.form) else {
            return false;
        };
        let Ok((mut field, view)) = crate::content::establish(chapter, form) else {
            // Content that does not establish was refused at load, so this is
            // unreachable through a validated bundle. It is answered rather
            // than trusted: a run that could not open the next chapter stands
            // where it is, with its sequence complete and its line clear.
            return false;
        };
        self.state.scenario.regime().apply(&mut field);
        let defaults = self.state.scenario.generator().route_defaults(next);
        if !defaults.is_empty() {
            if Self::validate_route_defaults(&field, &defaults).is_err() {
                return false;
            }
            field.route_controls = defaults;
        }
        let at = self.state.now.step;
        field.step = at;
        // The assembly ordinal counts the evaluations of one Run rather than of
        // one chapter, so it carries: two records of one run never share an
        // ordinal, and the streams they name stay distinct across the boundary.
        field.assembly_ordinal = self.state.now.assembly_ordinal;
        field.prev_assembly_step = Some(at);
        if !self.state.scenario.generator().establishes_field(next, &field)
            || field::validate(&field).is_err()
            || field::establishable(&field).is_err()
            || field::establishable_view(&view, &field).is_err()
        {
            return false;
        }

        self.end_window();
        self.state.now = field;
        self.state.view = view;
        self.state.slate = None;
        self.state.trace.keyframe = self.state.now.clone();
        self.state.trace.start_step = at;
        self.state.progress.chapter_index = next;
        self.state.criterion = self
            .state
            .scenario
            .criterion(next)
            .map(|_| crate::criterion::CriterionRuntime::opening(at));
        self.raise("criterion_changed", "{\"criterion\":null}");
        self.state.progress.objective = crate::state::ObjectiveState::hidden();
        self.state.pressures.clear();
        self.seat_schedule(chapter, at);
        if !self.state.pressures.is_empty() {
            self.raise_pressures();
        }
        // The chapter's own opening: the event that tells the shell which
        // chapter stands, and the first objective of it.
        self.open_chapter(chapter, content.chapters.len());
        // One autosave record on every `chapter_changed`, which is the locked
        // cadence read literally. It stands beside the Anchor the caller
        // writes, and costs one payload: the two are different kinds of record
        // and the fallback order reads them in different places.
        self.write_auto();
        let node = self
            .state
            .now
            .forms
            .iter()
            .find(|form| form.controlled)
            .map_or(0, |form| form.node);
        cues.push(Cue { kind: field::CUE_OBJECTIVE_COMPLETE, a: 0, b: node });
        true
    }

    /// Ends the campaign on the chapter the run stands in.
    ///
    /// The ending the run reaches is the chapter's own, resolved through
    /// [`crate::content::Chapter::ending_id`] against the Form the run opened on
    /// and the objectives it completed — the chapter's plain `ending_key` where
    /// the chapter authors no marks, and one of its authored variants where it
    /// does. The continuation is unlocked exactly when the campaign the run
    /// completed authored the whole closed set of chapters — a campaign
    /// truncated for testing reaches its own ending and unlocks nothing, because
    /// completing four chapters is not completing the campaign.
    fn complete_run(&mut self, content: &crate::content::Content) {
        let index = self.state.progress.chapter_index;
        let Some(chapter) = content.chapter(index) else {
            return;
        };
        let unlocked = content.chapters.len() == crate::content::CHAPTER_IDS.len();
        let ending = crate::fault::quoted(
            &chapter.ending_id(&self.form, &self.state.progress.complete),
        );
        // The line clears first: the campaign has nothing more to offer, and
        // the hidden state is the shape the payload declares for exactly that.
        // A chapter that hands on to another never reaches it, because the
        // chapter it hands to offers its own objective on the same step.
        if !self.state.progress.objective.id.is_empty() {
            let previous = self.state.progress.objective.id.clone();
            self.state.progress.objective = crate::state::ObjectiveState::hidden();
            let written = self.state.progress.objective.written();
            self.raise_objective(&written, Some(&previous));
            self.objective_ordinal = 0;
        }
        self.raise(
            "run_completed",
            &format!(
                "{{\"chapter_index\":{index},\"continuation_unlocked\":{unlocked},\
                 \"ending_id\":{ending}}}",
            ),
        );
        // One autosave record on `run_completed`, the other half of the locked
        // cadence sentence, taken before the mode moves so the record holds the
        // run that finished rather than one that has stopped answering.
        self.write_auto();
        self.mode = Mode::Ended;
    }

    /// Applies the authored events that fall due at this step boundary.
    ///
    /// An event is a step input like a pressure's stage entry: it changes
    /// authored Field state between two steps, through the same carriage — the
    /// active window ends under the Field the recorded steps ran under, the
    /// change lands, and the trajectory restarts on what it leaves. Nothing
    /// here draws, moves Charge, or writes a ledger entry, so the step function
    /// and every rule of it are untouched.
    fn settle_events(&mut self, chapter: &crate::content::Chapter, cues: &mut Vec<Cue>) {
        let due = crate::content::events_due(chapter, &self.state.progress, self.state.now.step);
        if due.is_empty() {
            return;
        }
        // Learned on a copy first, exactly as a pressure settlement is: an
        // event that finds the Field already the way it asks for ends no
        // window, because nothing changed.
        let mut copy = self.state.now.clone();
        let would = due
            .iter()
            .fold(false, |moved, event| crate::content::apply_event(&mut copy, event) || moved);
        if !would {
            return;
        }
        self.end_window();
        let standing = self
            .state
            .now
            .forms
            .iter()
            .find(|form| form.controlled)
            .map_or(0, |form| form.node);
        for event in due {
            // Both readings are taken against the Field as it stood before the
            // event: a severed Route names its own tail, and the tail is gone
            // the moment the cut applies.
            let kind = event.cue_kind(&self.state.now);
            let at = event.at_node(&self.state.now).unwrap_or(standing);
            if !crate::content::apply_event(&mut self.state.now, event) {
                continue;
            }
            // The cue the closed set already has for what the event did, at the
            // Node it stands at: the Port it names, the tail of the Route it
            // severs, or where the run stands.
            if let Some(kind) = kind {
                cues.push(Cue { kind, a: 0, b: at });
            }
        }
        self.state.trace.keyframe = self.state.now.clone();
        self.state.trace.start_step = self.state.now.step;
    }

    /// Writes one autosave record at the step the campaign asked for one.
    ///
    /// The metadata is noted first, because it rides in the payload the record
    /// stores, and a payload past its cap is refused with the note rolled back
    /// so the two never disagree — the same order the Anchor path uses.
    fn write_auto(&mut self) {
        let held = self.state.anchors.clone();
        let key = self.note_checkpoint(RecordKind::Auto, crate::state::auto_slot(self.state.now.step));
        match self.payload() {
            Ok(payload) => self.pending_records.push((key, RecordKind::Auto, payload)),
            Err(_) => self.state.anchors = held,
        }
    }

    /// Writes an Anchor at the step the authored sequence asked for one.
    ///
    /// The metadata is noted first, because it rides in the payload the record
    /// stores, and the payload is taken here rather than at the end of the
    /// batch so the record names the state the completion stood at. A payload
    /// past its cap is refused, and the note is rolled back with it so the two
    /// never disagree.
    fn write_anchor(&mut self, cues: &mut Vec<Cue>) {
        let held = self.state.anchors.clone();
        let key = self.note_checkpoint(RecordKind::Anchor, self.next_anchor_id());
        let Ok(payload) = self.payload() else {
            self.state.anchors = held;
            return;
        };
        let written = self
            .state
            .anchors
            .iter()
            .find(|anchor| anchor.save_key == key)
            .map(CheckpointState::written);
        self.pending_records.push((key, RecordKind::Anchor, payload));
        if let Some(anchor) = written {
            self.raise("checkpoint_written", &format!("{{\"anchor\":{anchor}}}"));
        }
        let node = self
            .state
            .now
            .forms
            .iter()
            .find(|form| form.controlled)
            .map_or(0, |form| form.node);
        cues.push(Cue { kind: field::CUE_ANCHOR_WRITTEN, a: 0, b: node });
    }

    /// The records of the most recent completed step, for a caller that reads
    /// the trajectory rather than the Field.
    pub fn last_records(&self) -> Option<&StepRecords> {
        self.state.trace.steps.back().map(|recorded| &recorded.records)
    }

    /// Reconstructs a read-only state at one retained timeline step. This is
    /// used only by exact inspection; it never replaces the live state or
    /// writes events, records, or runtime inputs.
    pub fn inspection_state_at(&self, step: u32) -> Option<RunState> {
        if step == self.state.now.step {
            return Some(self.state.clone());
        }
        if step < self.state.trace.start_step || step > self.state.now.step {
            return None;
        }
        let mut field = self.state.trace.keyframe.clone();
        let cache = field::StepCache::of(&field);
        for recorded in self.state.trace.steps.iter().filter(|recorded| recorded.step <= step) {
            replay_onto(
                &mut field,
                recorded,
                self.state.input_config.pointer_speed,
                &self.state.pressures,
                self.state.scenario.pressure_schedule(),
                self.state.scenario.regime().medium_motion(),
                self.state.scenario.regime().supply_jitter(),
                self.state.scenario.generator().local_policy(),
                &cache,
            );
        }
        if field.step != step {
            return None;
        }
        let mut state = self.state.clone();
        state.now = field;
        state.trace.steps.retain(|recorded| recorded.step <= step);
        Some(state)
    }

    /// Carries the trajectory's keyframe forward to where the retained span
    /// puts it, by replaying the recorded steps it passes over. A regeneration
    /// draws no fresh randomness and reads no live input.
    fn carry_keyframe(&mut self) {
        let wanted = Trace::start_for(self.state.now.step);
        if self.state.trace.start_step >= wanted {
            return;
        }
        // One cache for the whole carry: the Field's shape does not change
        // while it runs, because a commit is what changes a shape and a commit
        // ends the window before it does.
        let cache = field::StepCache::of(&self.state.trace.keyframe);
        while self.state.trace.start_step < wanted {
            let Some(recorded) = self.state.trace.steps.pop_front() else {
                debug_assert!(false, "the retained span holds every step after its keyframe");
                break;
            };
            replay_onto(
                &mut self.state.trace.keyframe,
                &recorded,
                self.state.input_config.pointer_speed,
                &self.state.pressures,
                self.state.scenario.pressure_schedule(),
                self.state.scenario.regime().medium_motion(),
                self.state.scenario.regime().supply_jitter(),
                self.state.scenario.generator().local_policy(),
                &cache,
            );
            self.state.trace.start_step = recorded.step;
        }
    }

    /// The evaluation record itself, for the caller that reports it.
    pub fn standing_slate(&self) -> Option<&CandidateSlate> {
        self.state.slate.as_ref()
    }

    /// Selects one candidate's observation View immediately and for free, or
    /// clears only its member selection at position 0.
    ///
    /// This changes measurement metadata only. It deliberately does not touch
    /// the Field, trace, completed step, plan queue, Impulse, slate, or active
    /// window. Physical compartment reshaping is the paid plan operation.
    pub fn set_focus(
        &mut self,
        slate_ordinal: u32,
        position: u8,
    ) -> Result<ViewDeclaration, Fault> {
        let Some(slate) = self.state.slate.as_ref() else {
            return Err(plan::Refusal::missing("slate").fault());
        };
        if slate.ordinal != slate_ordinal {
            return Err(plan::Refusal::missing("slate").fault());
        }
        // Position 0 is the passive clear operation, not a candidate seat. It
        // retains the rest of the active measurement protocol and is available
        // even when the slate is deficient, because it adopts no evidence from
        // that slate. The ordinal still has to name the record that stands.
        if position == 0 {
            let mut view = self.state.view.clone();
            view.inside.clear();
            self.state.view = view.clone();
            return Ok(view);
        }
        if slate.deficient {
            return Err(plan::Refusal::invalid("deficient").fault());
        }
        let Some(candidate) = slate.candidates.get(usize::from(position) - 1) else {
            return Err(plan::Refusal::invalid("position").fault());
        };
        let view = candidate.view.clone();
        self.state.view = view.clone();
        Ok(view)
    }

    /// Assembles the slate for one evaluation and raises the record.
    ///
    /// FRAMEWORK.md's Game-facing interpretations name the two moments: a slate
    /// is assembled on entry into Still Mode and again after every committed
    /// change, and each assembly increments the Run's assembly ordinal. Entry
    /// means the moment the entry ramp reaches scale 0, which the analysis
    /// budget locks as the trigger, and the evaluation step is the last
    /// completed step.
    ///
    /// A Field with no Node is the one case no evaluation may run in: both the
    /// selected members and the fallback seed are empty, so there is no
    /// candidate to evaluate. That pre-chapter placeholder differs from a
    /// player-cleared View over a populated Field, whose slate uses the local
    /// whole-Field fallback without reselecting the active View.
    fn assemble_slate(&mut self) {
        if self.state.now.ports.is_empty() {
            return;
        }
        let mut slate = crate::slate::assemble(&self.state);
        // The second half of one job: assembly names the candidates, and the
        // evaluation reads the four values, compares them through their
        // confidence ranges, and fills in the tiers, the dominance relation,
        // the baselines, and the tolerance-sensitivity flag. The order is the
        // analysis budget's own.
        crate::rank::evaluate(&self.state, &mut slate);
        // The ordinal counts the slates assembled earlier in this Run, so it
        // advances after the assembly that read it; the previous assembly's
        // evaluation step becomes this one's, which is what the next
        // assembly's freshness rule reads.
        self.state.now.assembly_ordinal = self.state.now.assembly_ordinal.saturating_add(1);
        self.state.now.prev_assembly_step = Some(slate.step);
        let written = slate.written();
        self.state.slate = Some(slate);
        self.raise("review_ready", &format!("{{\"review\":{{\"kind\":\"slate\",\"slate\":{written}}}}}"));
    }

    /// Queues one proposed change.
    ///
    /// The order the checks run in is the order the refusals are worth making
    /// in. The queue's own depth comes first, because a full queue is a fact
    /// about the queue rather than about the entry — and because with the
    /// Impulse cap and the queue depth both at six, the Impulse check would
    /// otherwise answer every seventh entry and the `capacity` envelope the
    /// caps rule locks would never be reachable. Then the entry itself, against
    /// the projection every earlier entry has been applied to. Then the cost,
    /// which is what the whole queue would spend with this entry in it.
    pub fn queue_plan(&mut self, plan: PlanCommand) -> Result<usize, Fault> {
        if self.queue.len() >= PLAN_QUEUE_DEPTH {
            return Err(cap_fault("plan_queue_depth", PLAN_QUEUE_DEPTH as i64));
        }
        let projected = self.queue.projected(&self.state.now);
        plan::check(&plan, &projected).map_err(plan::Refusal::fault)?;

        let cost = (self.queue.len() as u8 + 1) * crate::plan::PLAN_ENTRY_COST;
        if cost > self.impulse() {
            let mut detail = String::new();
            let mut object = Obj::new(&mut detail);
            object.int("cost", i64::from(cost));
            object.int("impulse", i64::from(self.impulse()));
            object.end();
            return Err(Fault::detailed(Code::Impulse, detail));
        }

        // The one locked queue-time side effect: a completed Boundary Handle
        // drag appends its member set to the front of the drawn list with the
        // current step, whether or not the change is later committed. It is the
        // player's own act, and source 1 of candidate assembly reads the list
        // rather than the commits.
        if let PlanCommand::ReshapeCompartment { members } = &plan {
            self.state.now.boundaries.record_drawn(members.clone(), self.state.now.step);
        }
        let queued = self.queue.push(plan)?;
        self.queue.rebuild(&self.state.now);
        Ok(queued)
    }

    /// Removes the most recent queued change; an empty queue changes nothing.
    ///
    /// What a drag recorded in the drawn boundary list is not taken back with
    /// it: the list holds every completed drag whether or not the change is
    /// committed, so an undo that removes a `reshape_compartment` removes the
    /// proposal and leaves the record of the act.
    pub fn undo_plan(&mut self) -> usize {
        self.queue.undo();
        self.queue.rebuild(&self.state.now);
        self.queue.len()
    }

    /// Pays for one Lens-local sensor packet. The paid observation ends the
    /// preceding trace window before Charge changes, then builds a bounded
    /// local belief ensemble. The shell receives sensed identities and the
    /// aggregate forecast, never remote topology or hidden pressure schedules.
    pub fn sample_lens(&mut self) -> Result<String, Fault> {
        let form_place = self
            .state
            .now
            .forms
            .iter()
            .position(|form| form.controlled && form.form == "lens" && form.forecast_depth > 0)
            .ok_or_else(|| Fault::field("lens"))?;
        let form = &self.state.now.forms[form_place];
        let port_place = self
            .state
            .now
            .ports
            .iter()
            .position(|port| port.node == form.node)
            .ok_or_else(|| Fault::field("lens_node"))?;
        if self.state.now.ports[port_place].q < LENS_SAMPLE_COST {
            return Err(Fault::because(Code::Validation, "lens_charge"));
        }

        let origin = form.pos;
        let layer = form.layer;
        let horizon = form.forecast_depth;
        let nodes: Vec<u32> = self
            .state
            .now
            .ports
            .iter()
            .filter(|port| {
                crate::fx::distance(origin, layer, port.pos, port.layer) <= LENS_SENSOR_RADIUS
            })
            .map(|port| port.node)
            .collect();
        let routes: Vec<u32> = self
            .state
            .now
            .routes
            .iter()
            .filter(|route| nodes.contains(&route.tail) && nodes.contains(&route.head))
            .map(|route| route.route)
            .collect();

        self.end_window();
        self.state.now.ports[port_place].q -= LENS_SAMPLE_COST;
        self.state.now.forms[form_place].charge -= LENS_SAMPLE_COST;
        self.state.slate = None;
        self.state.trace.keyframe = self.state.now.clone();
        self.state.trace.start_step = self.state.now.step;

        let forecast = self.lens_forecast(&nodes, &routes, origin, layer, horizon);

        let mut out = String::new();
        let mut object = Obj::new(&mut out);
        object.int("cost", LENS_SAMPLE_COST);
        object.int("horizon", i64::from(horizon));
        {
            let mut listed = object.list("node_ids");
            for node in &nodes {
                listed.int(i64::from(*node));
            }
            listed.end();
        }
        {
            let mut listed = object.list("points");
            for (step, low, expected, high) in forecast {
                let mut written = String::new();
                let mut point = Obj::new(&mut written);
                point.int("expected", expected);
                point.int("high", high);
                point.int("low", low);
                point.int("step", i64::from(step));
                point.end();
                listed.raw(&written);
            }
            listed.end();
        }
        {
            let mut listed = object.list("route_ids");
            for route in &routes {
                listed.int(i64::from(*route));
            }
            listed.end();
        }
        object.int("sensor_radius", LENS_SENSOR_RADIUS);
        object.end();
        Ok(out)
    }

    fn lens_forecast(
        &self,
        nodes: &[u32],
        routes: &[u32],
        origin: crate::fx::Vec2,
        layer: u8,
        horizon: u16,
    ) -> Vec<(u32, crate::state::Fx, crate::state::Fx, crate::state::Fx)> {
        let mut local = self.state.now.clone();
        local.ports.retain(|port| nodes.binary_search(&port.node).is_ok());
        local.routes.retain(|route| routes.binary_search(&route.route).is_ok());
        local.forms.retain(|form| nodes.binary_search(&form.node).is_ok());
        local.physical_compartment.members.retain(|node| nodes.binary_search(node).is_ok());
        local.route_clamps.retain(|clamp| routes.binary_search(&clamp.route).is_ok());
        let empty_scramble = if let Some(scramble) = &mut local.route_scramble {
            scramble.routes.retain(|route| routes.binary_search(route).is_ok());
            scramble.routes.is_empty()
        } else {
            false
        };
        if empty_scramble {
            local.route_scramble = None;
        }
        local.supply_decoys.retain(|decoy| nodes.binary_search(&decoy.receiver).is_ok());
        local.signals.retain(|signal| {
            nodes.binary_search(&signal.source).is_ok()
                && nodes.binary_search(&signal.target).is_ok()
        });
        local.materials.retain(|material| {
            crate::fx::distance(origin, layer, material.pos, material.layer) <= LENS_SENSOR_RADIUS
        });
        local.pending.retain(|cache| {
            crate::fx::distance(origin, layer, cache.pos, cache.layer) <= LENS_SENSOR_RADIUS
        });
        for held in &mut local.layers {
            held.port_ids.retain(|node| nodes.binary_search(node).is_ok());
        }

        let mut trials = vec![vec![0; usize::from(horizon)]; LENS_FORECAST_TRIALS];
        for (ordinal, series) in trials.iter_mut().enumerate() {
            let mut field = local.clone();
            let cache = field::StepCache::of(&field);
            let mut pressures = Vec::new();
            let schedule = crate::pressure::Schedule::default();
            let mut stream = trajectory_stream(
                &self.state.run_id,
                self.state
                    .branch_nonce
                    .wrapping_add(0x4c45_4e53)
                    .wrapping_add(ordinal as u32),
            );
            for value in series {
                let mut staging = field::Staging {
                    pressures: &mut pressures,
                    schedule: &schedule,
                    stream: &mut stream,
                    medium: self.state.scenario.regime().medium_motion(),
                    supply_jitter: self.state.scenario.regime().supply_jitter(),
                };
                let policy = self.state.scenario.generator().local_policy();
                if policy.is_empty() {
                    field::advance_cached(
                        &mut field,
                        ControlState::default(),
                        self.state.input_config.pointer_speed,
                        &mut staging,
                        &cache,
                    );
                } else {
                    field::advance_cached_programmed(
                        &mut field,
                        ControlState::default(),
                        self.state.input_config.pointer_speed,
                        policy,
                        &mut staging,
                        &cache,
                    );
                }
                *value = field.ports.iter().map(|port| port.q).sum();
            }
        }

        (0..usize::from(horizon))
            .map(|place| {
                let values: Vec<crate::state::Fx> = trials.iter().map(|trial| trial[place]).collect();
                let low = *values.iter().min().unwrap_or(&0);
                let high = *values.iter().max().unwrap_or(&0);
                let expected = values.iter().sum::<crate::state::Fx>() / values.len() as i64;
                (self.state.now.step.saturating_add(place as u32 + 1), low, expected, high)
            })
            .collect()
    }

    /// The canonical save payload, held to its locked cap. The cap is checked
    /// at every write and every export, so a payload that crossed it would be
    /// refused rather than stored.
    pub fn payload(&self) -> Result<String, Fault> {
        let payload = self.state.payload();
        if payload.len() > SAVE_PAYLOAD_CAP {
            return Err(cap_fault("save_payload", SAVE_PAYLOAD_CAP as i64));
        }
        Ok(payload)
    }

    /// The export file, its hash, and the name the shell offers.
    pub fn export(&self) -> Result<String, Fault> {
        // The cap is stated over the payload, and the export file wraps it, so
        // the payload is what is measured.
        self.payload()?;
        let text = self.state.export_file();
        let hash = crate::json::hex_bytes(&sha256::digest(text.as_bytes()));
        let mut out = String::new();
        let mut object = Obj::new(&mut out);
        object.text("embodied_state_hash", &self.state.now.embodied_hash());
        object.text("filename_hint", &self.state.filename_hint());
        object.text("sha256", &hash);
        object.text("text", &text);
        object.end();
        Ok(out)
    }

    /// How many autosave intervals the run has completed: one every 900
    /// completed steps, which is 30 s of play.
    pub fn autosave_intervals(&self) -> u32 {
        self.state.now.step / AUTOSAVE_STEPS
    }

    /// One checkpoint's metadata as it stands right now, under a given
    /// identifier and key.
    fn checkpoint(&self, anchor_id: u32, kind: RecordKind, save_key: String) -> CheckpointState {
        CheckpointState {
            anchor_id,
            step: self.state.now.step,
            chapter_index: self.state.progress.chapter_index,
            objective_id: self.state.progress.objective.id.clone(),
            kind,
            save_key,
            rng: self.state.rng,
            branch_nonce: self.state.branch_nonce,
        }
    }

    /// Records one checkpoint's metadata against a record slot, and hands back
    /// the `save_key` the record is written under.
    ///
    /// One entry stands per key. A write that replaces a record replaces that
    /// key's entry in place, keeping its identifier, so no entry ever names a
    /// record holding a payload it was not written beside; a key with no entry
    /// takes one past the largest identifier the run has handed out. Both are
    /// pure functions of the run's own history, so a replay repeats them.
    pub fn note_checkpoint(&mut self, kind: RecordKind, slot: u32) -> String {
        let key = save_key(&self.state.run_id, kind, slot);
        match self.state.anchors.iter().position(|anchor| anchor.save_key == key) {
            Some(place) => {
                let held = self.state.anchors[place].anchor_id;
                let entry = self.checkpoint(held, kind, key.clone());
                self.state.anchors[place] = entry;
            }
            None => {
                let next =
                    self.state.anchors.iter().map(|anchor| anchor.anchor_id).max().unwrap_or(0) + 1;
                let entry = self.checkpoint(next, kind, key.clone());
                self.state.anchors.push(entry);
            }
        }
        key
    }

    /// Rewrites the metadata entry for one record slot, and hands back the
    /// `save_key`. Unlike a checkpoint note, this appends nothing: an import
    /// adds no history the run did not already have, and only corrects an entry
    /// that would otherwise name a record whose payload has been replaced.
    pub fn rewrite_checkpoint(&mut self, kind: RecordKind, slot: u32) -> String {
        let key = save_key(&self.state.run_id, kind, slot);
        if let Some(place) = self.state.anchors.iter().position(|anchor| anchor.save_key == key) {
            let held = self.state.anchors[place].anchor_id;
            let entry = self.checkpoint(held, kind, key.clone());
            self.state.anchors[place] = entry;
        }
        key
    }

    /// The checkpoint metadata one identifier names.
    pub fn anchor(&self, anchor_id: u32) -> Option<&CheckpointState> {
        self.state.anchors.iter().find(|anchor| anchor.anchor_id == anchor_id)
    }

    /// Puts the checkpoint metadata back where it stood, for a write that was
    /// refused after it was noted.
    pub fn set_anchors(&mut self, anchors: Vec<CheckpointState>) {
        self.state.anchors = anchors;
    }

    /// The render snapshot: the locked header, section table, and one section
    /// per part of the Field that stands.
    pub fn frame_view(&self) -> Vec<u8> {
        frame::encode(&Snapshot {
            field: &self.state.now,
            mode: self.mode,
            time_scale: self.time_scale(),
            view_inside: &self.state.view.inside,
            queue: &self.queue,
            cues: &self.cues,
            config: &self.state.input_config,
            progress: &self.state.progress,
            pressures: &self.state.pressures,
            objective_ordinal: self.objective_ordinal,
            forecast: self.forecast(),
            medium: self.state.scenario.regime().medium_motion(),
        })
    }

    /// The standing View's baseline `q_I` envelope over its window, which the
    /// Still Mode overlay draws and which is the only forward reading shown.
    ///
    /// It is the spread the standing candidate's eight baseline replays read at
    /// each window step: the smallest and the largest `q_I` any of them stood
    /// at. A run under no slate has none, and so does one whose window the
    /// clamp left empty — the section is written all the same while the run is
    /// `still`, carrying the count it has, because the surface reads the frame
    /// rather than the mode.
    fn forecast(&self) -> &[(crate::state::Fx, crate::state::Fx)] {
        match &self.state.slate {
            Some(slate)
                if slate
                    .candidates
                    .first()
                    .is_some_and(|candidate| candidate.view == self.state.view) =>
            {
                &slate.forecast_envelope
            }
            None => &[],
            Some(_) => &[],
        }
    }

    /// The time scale, as the header's field carries it.
    ///
    /// The mode table runs the scale from 0 to 65536 and the header holds it
    /// in 16 bits, where 65536 has no representation; full speed is therefore
    /// written as the widest value the field carries, and the ramps run over
    /// [0, 65535] with it. The buffer is render-only and produced by one-way
    /// conversion, so nothing reads this back into the simulation.
    ///
    /// The ramp is linear in the elapsed time it has taken, which is what the
    /// table says it is: falling 65536 → 0 over `ramp_in`, rising 0 → 65536
    /// over `ramp_out`.
    fn time_scale(&self) -> u16 {
        let raw = match self.mode {
            Mode::Running => FULL_SCALE,
            Mode::RampIn => (RAMP_UNITS - self.ramp) * FULL_SCALE / RAMP_UNITS,
            Mode::RampOut => self.ramp * FULL_SCALE / RAMP_UNITS,
            // `still` holds the scale at 0, and a suspended or ended run
            // advances nothing either.
            _ => 0,
        };
        raw.clamp(0, i64::from(u16::MAX)) as u16
    }

    /// The Impulse the run carries, which every queued change is paid for out
    /// of and which the render snapshot's header reports.
    pub fn impulse(&self) -> u8 {
        self.state.progress.impulse
    }

    /// Commits the queued changes and exits Still Mode.
    ///
    /// The locked rule is all-or-nothing: revalidate every entry in order from
    /// the base state, require the Impulse the whole queue costs, then apply
    /// every entry, deduct the cost, and clear the queue. A refusal at any
    /// position applies nothing at all, keeps the queue, and reports the
    /// position and the reason.
    ///
    /// Nothing is applied to the run until every entry has passed, because
    /// every entry is applied to a projection of the base state first and the
    /// projection is installed only once the last one has passed. A refusal
    /// therefore drops a projection rather than rolling anything back: there is
    /// no half-applied state to undo, and no order of failures that could leave
    /// one.
    ///
    /// The cost is the queue's own — one Impulse per entry — so the total the
    /// tray predicted and the total spent here are the same arithmetic on the
    /// same count.
    pub fn commit_plan(&mut self) -> Result<u8, Fault> {
        let entries = self.queue.entries();
        let cost = entries.len() as u32;
        if u32::from(self.impulse()) < cost {
            let mut detail = String::new();
            let mut object = Obj::new(&mut detail);
            object.int("cost", i64::from(cost));
            object.int("impulse", i64::from(self.impulse()));
            object.end();
            return Err(Fault::detailed(Code::Impulse, detail));
        }

        // Revalidation, entry by entry from the base state. A queue is built
        // while the run is still, and a still run runs no step, so the base
        // state cannot have moved under it — but the entries can disagree with
        // each other, and an entry that stood when it was queued can fail
        // against a projection the entries before it have changed.
        let mut projected = Projection::of(&self.state.now);
        for (position, entry) in entries.iter().enumerate() {
            plan::check(entry, &projected)
                .map_err(|refusal| refusal.positioned(position))?;
            plan::apply(entry, &mut projected)
                .map_err(|refusal| refusal.positioned(position))?;
        }
        if !entries.is_empty() {
            Self::validate_local_policy(
                &projected.field,
                self.state.scenario.generator().local_policy(),
            )?;
            let generator = self
                .state
                .scenario
                .generator()
                .with_field(self.state.progress.chapter_index, &projected.field)?;
            let scenario = self.state.scenario.with_generator(generator)?;
            let attempt_branch = self.descendant_attempt_branch(
                &scenario,
                self.state.branch_nonce,
                crate::state::BranchOperation::DesignCommit,
            )?;
            // The Echo is derived here, before anything is applied, whichever
            // branch it takes: a perturbation replays the pre-commit window,
            // which the commit is about to end, and an evaluation highlight
            // reads the pre-commit slate, whose baselines the reassembly is
            // about to replace with the unassigned ones a span of 0 gives.
            // Either reading taken after the commit would read nothing.
            self.pending_echo = self.echo_of(&entries);
            // The active window ends before the causal change takes effect.
            // The keyframe carry therefore replays every retained step under
            // the material compartment and route topology it actually ran
            // under, then restarts on the Field the commit leaves.
            self.end_window();
            self.state.now = projected.field;
            self.state.scenario = scenario;
            self.state.attempt_branch = attempt_branch;
            field::synchronize_automation_state(&mut self.state.now);
            self.state.progress.impulse -= cost as u8;
            // The second of the two assembly moments: a committed change
            // reassembles the slate, under the span the commit just clamped.
            // The retained span is 0 at this instant, so the effective window
            // is 0 and every windowed source of the assembly yields nothing —
            // which is the honest reading of a Field nothing has been observed
            // of yet, not a fault. It runs before the keyframe is taken so the
            // keyframe is the whole state the commit leaves, the ordinal this
            // assembly advanced among it.
            self.assemble_slate();
            self.state.trace.keyframe = self.state.now.clone();
            self.state.trace.start_step = self.state.now.step;
        }
        self.queue.clear();
        // Design edits remain in Design authority. Commissioning starts only
        // when the operator explicitly runs the generator.
        self.mode = Mode::Still;
        self.ramp = 0;
        if let Some(echo) = self.pending_echo.take() {
            self.raise(
                "review_ready",
                &format!("{{\"review\":{{\"kind\":\"echo\",\"echo\":{}}}}}", echo.written()),
            );
        }
        Ok(entries.len() as u8)
    }

    /// The Echo one committed queue leaves: one branch on the last committed
    /// entry, taken before the commit applies anything, with no fall-through
    /// between its arms.
    ///
    /// FRAMEWORK.md's Echo list binds the committed changes to their sources,
    /// and ARCHITECTURE.md carries the branch verbatim:
    ///
    /// - a committed **cut** of one Route runs the `route-removal` perturbation
    ///   with that Route as its parameter — or `boundary-severance` when the
    ///   committed queue took every crossing Route of the pre-commit standing
    ///   View. It replays the pre-commit window, which is the only window there
    ///   is: the commit ends it. A cut whose reading is unassigned leaves no
    ///   highlight, and — because the branch is one match — never another
    ///   arm's.
    /// - a committed **compartment reshape** has no candidate-View evaluation
    ///   to borrow and therefore leaves no highlight until a typed physical
    ///   compartment counterfactual exists;
    /// - a committed **connect or redirect**, which the framework's list does
    ///   not bind, reads the standing candidate — seat 1 — because the
    ///   standing View is what those commits leave adopted.
    /// - a committed **replacement**, when later goals add the rewrite flow,
    ///   reads from `component-substitution` with that member as its
    ///   parameter; until that flow exists no entry queues one.
    ///
    fn echo_of(&self, entries: &[PlanCommand]) -> Option<crate::perturb::EchoHighlight> {
        use crate::perturb::{Request, BOUNDARY_SEVERANCE, ROUTE_REMOVAL};
        match entries.last()? {
            PlanCommand::Cut { route } => {
                // Every Route the committed queue cuts, against the crossing
                // set of the physical compartment standing before it: a commit that takes the
                // last one of them is a severance of the boundary rather than
                // the removal of one Route.
                let cut: Vec<u32> = entries
                    .iter()
                    .filter_map(|entry| match entry {
                        PlanCommand::Cut { route } => Some(*route),
                        _ => None,
                    })
                    .collect();
                let crossing = self.crossing_routes();
                let severed =
                    !crossing.is_empty() && crossing.iter().all(|held| cut.contains(held));
                let request = if severed {
                    // The existing boundary-severance assay is parameterized
                    // by an observation View. Once that View and the material
                    // compartment differ, borrowing it would replay a
                    // different crossing set from the physical edit. Leave no
                    // Echo until a typed compartment-severance assay exists.
                    if self.state.view.inside.as_slice()
                        != self.state.now.physical_compartment.members.as_slice()
                    {
                        return None;
                    }
                    Request::of(BOUNDARY_SEVERANCE, None)?
                } else {
                    Request::of(ROUTE_REMOVAL, Some(*route))?
                };
                crate::perturb::run(
                    &self.state,
                    &self.state.view,
                    &[],
                    0,
                    &self.evaluation_sigma(),
                    self.tolerance(),
                    request,
                )
                .highlight()
            }
            // A physical reshape has no candidate-View evaluation to borrow.
            // It leaves no Echo until a typed compartment counterfactual is
            // implemented.
            PlanCommand::ReshapeCompartment { .. }
            | PlanCommand::DeployJunction
            | PlanCommand::LimitRoute { .. }
            | PlanCommand::RaiseLeak { .. }
            | PlanCommand::DivertSupply { .. }
            | PlanCommand::ReplaceComponent { .. }
            | PlanCommand::Transplant { .. }
            | PlanCommand::DelaySupply { .. }
            | PlanCommand::ScrambleRoutes { .. } => None,
            PlanCommand::Connect { .. } | PlanCommand::Redirect { .. } => {
                let slate = self.state.slate.as_ref()?;
                crate::perturb::evaluation_highlight(slate.candidates.first()?, slate.ordinal)
            }
        }
    }

    /// The crossing set of the physical compartment: the Routes with exactly
    /// one material endpoint inside it.
    fn crossing_routes(&self) -> Vec<u32> {
        let inside = &self.state.now.physical_compartment.members;
        self.state
            .now
            .routes
            .iter()
            .filter(|route| {
                inside.binary_search(&route.tail).is_ok()
                    != inside.binary_search(&route.head).is_ok()
            })
            .map(|route| route.route)
            .collect()
    }

    /// Carries the trajectory's keyframe to the step the run stands on and
    /// drops the steps it passed over.
    ///
    /// This is the ordinary keyframe carry run to a boundary rather than to the
    /// retained span's own start, and it is what "a commit ends the active
    /// window" means for the trace: every retained step is replayed under the
    /// causal Field that produced it, and none is left behind to be replayed
    /// under a later physical edit.
    fn end_window(&mut self) {
        let cache = field::StepCache::of(&self.state.trace.keyframe);
        while let Some(recorded) = self.state.trace.steps.pop_front() {
            replay_onto(
                &mut self.state.trace.keyframe,
                &recorded,
                self.state.input_config.pointer_speed,
                &self.state.pressures,
                self.state.scenario.pressure_schedule(),
                self.state.scenario.regime().medium_motion(),
                self.state.scenario.regime().supply_jitter(),
                self.state.scenario.generator().local_policy(),
                &cache,
            );
            self.state.trace.start_step = recorded.step;
        }
        debug_assert_eq!(
            self.state.trace.start_step, self.state.now.step,
            "the retained span holds every step after its keyframe",
        );
        debug_assert!(
            self.state
                .trace
                .keyframe
                .ports
                .iter()
                .zip(self.state.now.ports.iter())
                .all(|(carried, standing)| carried.node == standing.node
                    && carried.q == standing.q),
            "a keyframe carried to this step stands where the run stands",
        );
    }
}

/// Drifts every path point of every current on one layer by `delta` raw along
/// one of the four axis directions — `(+x, +y, -x, -y)` indexed by the
/// pressure's `start_step mod 4` — each coordinate clamped into the plane.
/// Geometry only: no Charge moves and nothing enters the ledger; what drifts
/// is where delivery lands.
fn drift_paths(field: &mut crate::state::FieldState, layer: u32, delta: crate::state::Fx, direction: u32) {
    let clamp = |value: crate::state::Fx| value.clamp(0, 268_435_455);
    for current in &mut field.currents {
        if u32::from(current.layer) != layer {
            continue;
        }
        for point in &mut current.path {
            match direction {
                0 => point.x = clamp(point.x + delta),
                1 => point.y = clamp(point.y + delta),
                2 => point.x = clamp(point.x - delta),
                _ => point.y = clamp(point.y - delta),
            }
        }
    }
}

/// Replays one recorded step onto a stored state, under the recorded control
/// schedule: the step function runs again on the control the step consumed,
/// which reproduces the same records because the step is a pure function of the
/// state and that control.
///
/// A regeneration draws no fresh randomness and reads no live input.
/// `wheel_accum` and `depth_cooldown` are carried forward unchanged, as the
/// document says: they are input-resolution state, not Field content, and no
/// replay reads them, because every replay drives control from the recorded
/// schedule rather than resolving input again.
fn replay_onto(
    state: &mut FieldState,
    recorded: &TraceStep,
    pointer_speed: Frac,
    pressures: &[crate::pressure::PressureState],
    schedule: &crate::pressure::Schedule,
    medium: field::MediumMotion,
    supply_jitter: crate::state::Frac,
    policy: &crate::policy::FrozenLocalPolicy,
    cache: &field::StepCache,
) {
    // The pressure list a replayed step runs under is the live list: no window
    // spans a change of membership, because every rule that changes one ends
    // the window, so the membership the run stands with now is the membership
    // every retained step ran under. The clone's `stage` and `level` fields
    // may stand at a LATER stage than the step being replayed — and that is
    // fine, because no phase reads them as history: the opened readings
    // (the Noise scale, Flood's throttle) are derived per step by
    // `pressure::opened_level`, and every staged reading follows the stage
    // machine's own re-derivation for the replayed step.
    let mut carried = pressures.to_vec();
    let mut stream = recorded.rng;
    let mut staging = field::Staging {
        pressures: &mut carried,
        schedule,
        stream: &mut stream,
        medium,
        supply_jitter,
    };
    if policy.is_empty() {
        field::advance_cached(state, recorded.ctl, pointer_speed, &mut staging, cache);
    } else {
        field::advance_cached_programmed(
            state,
            ControlState::default(),
            pointer_speed,
            policy,
            &mut staging,
            cache,
        );
    }
    debug_assert_eq!(state.step, recorded.step, "a replayed step lands on its own step");
}

fn read_int(body: &Json, key: &str, low: i64, high: i64) -> Result<i64, Fault> {
    body.get(key)
        .and_then(Json::as_int)
        .filter(|value| (low..=high).contains(value))
        .ok_or_else(|| Fault::field(key))
}

fn read_bool(body: &Json, key: &str) -> Result<bool, Fault> {
    body.get(key).and_then(Json::as_bool).ok_or_else(|| Fault::field(key))
}
