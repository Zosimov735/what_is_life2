//! The authoritative state and its serialized form.
//!
//! `RunState`'s serialized form is the canonical V2 save payload — one live
//! shape, so the byte-equivalence contract and the save format cannot drift
//! apart. The V1 reader exists only as a verified one-way migration.
//! Everything here writes through
//! `crate::json`, so the canonical rules are applied in one place.
//!
//! What this holds is the runtime and the Field: the run key, the branch nonce,
//! the content hash, the live random state, the completed-step counter, the
//! retained trajectory, and the Field's own contents — layers, ports, routes,
//! forms, currents, the physical compartment, and the two candidate-Boundary
//! lists, whose shapes and step belong to [`crate::field`]. The View, progress,
//! slate, pressures, and Anchor records
//! belong to the modules that own them; the slate is [`crate::slate`]'s and
//! the staged pressures are [`crate::pressure`]'s. Every declared field is
//! present either way, because canonical JSON has no absent-versus-null
//! ambiguity.

use crate::fault::Fault;
use crate::field::{
    BoundaryState, CurrentState, FieldLayer, FormState, PendingTrail, PhysicalCompartment,
    PortState, RouteState, StepRecords, UpkeepRecord, CURRENTS_PER_CHAPTER, FORMS_PER_RUN,
    LAYERS_PER_CHAPTER, NODES_PER_RUN, PENDING_TRAILS, ROUTES_PER_RUN, UPKEEP_PURPOSES,
};
use crate::json::{Json, Obj};
use crate::read;
use crate::rng::RngState;
use crate::sha256;
use crate::slate::CandidateSlate;

/// A Q32.16 binary fixed-point quantity, raw. Nothing here is a float.
pub type Fx = i64;

/// `Fx` restricted to [0, 65536]: the raw form of [0, 1].
pub type Frac = i64;

/// The completed-step counter.
pub type Step = u32;

/// The save version this build reads and writes.
pub const SAVE_VERSION: i64 = 2;

/// The save payload's cap: 8 MiB of canonical bytes, validated at every write
/// and every export.
pub const SAVE_PAYLOAD_CAP: usize = 8 * 1024 * 1024;

/// How many completed steps one autosave interval spans: 30 s of play.
pub const AUTOSAVE_STEPS: Step = 900;

/// The two `auto` record slots a run's autosaves alternate between, so a fault
/// mid-write always leaves the other slot intact.
pub const AUTO_SLOTS: u32 = 2;

/// Anchor records per run. This is a cap on stored records; the payload's
/// checkpoint metadata is not capped, because the metadata of a pruned record
/// is kept.
pub const ANCHORS_PER_RUN: usize = 64;

/// The autosave slot a completed step derives: `floor(step / 900) mod 2`.
///
/// The suffix is a pure function of run state and of nothing else. It cannot
/// read the store's own write history, because the suffix is written into the
/// payload's checkpoint metadata: a slot chosen from what the store happens to
/// hold would put non-enumerated state into the bytes, and two sessions given
/// the same run key and the same frames would stop agreeing past the first
/// autosave. The two slots still alternate, so a fault mid-write always leaves
/// the other one intact.
pub fn auto_slot(step: Step) -> u32 {
    (step / AUTOSAVE_STEPS) % AUTO_SLOTS
}

/// Simulation rate: steps per second, exactly.
pub const STEPS_PER_SECOND: u32 = 30;

/// How far behind the current step the retained trajectory's keyframe sits
/// before it is carried forward, so the trace spans 120 to 150 steps.
pub const TRACE_LAG: u32 = 120;

/// The authored default declared window, in steps.
pub const DEFAULT_WINDOW: i64 = 45;

/// The raw `Frac` of 1: the unscaled setting.
pub const FRAC_ONE: Frac = 65536;

/// The Impulse a new run opens with.
pub const OPENING_IMPULSE: u8 = 3;

/// The most Impulse a run may carry at once.
pub const IMPULSE_CAP: u8 = 6;

/// What one completed layer objective gives back, clamped at the cap.
pub const IMPULSE_GRANT: u8 = 3;

/// The three closed surround rules, naming which non-members supply external
/// influence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Surround {
    Adjacent,
    Double,
    Whole,
}

impl Surround {
    pub fn name(self) -> &'static str {
        match self {
            Surround::Adjacent => "adjacent",
            Surround::Double => "double",
            Surround::Whole => "whole",
        }
    }
}

/// The four closed states an objective's `state` field carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjectiveStage {
    Hidden,
    Active,
    Complete,
    FailedRecoverable,
}

impl ObjectiveStage {
    pub fn name(self) -> &'static str {
        match self {
            ObjectiveStage::Hidden => "hidden",
            ObjectiveStage::Active => "active",
            ObjectiveStage::Complete => "complete",
            ObjectiveStage::FailedRecoverable => "failed_recoverable",
        }
    }
}

/// The control the step function consumed, recorded exactly as resolved.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct ControlState {
    pub steer_x: i16,
    pub steer_y: i16,
    pub pulse_held: bool,
    pub pulse_release: bool,
    pub depth_move: i8,
}

impl ControlState {
    fn read(value: &Json, key: &str) -> Result<Self, Fault> {
        let found = read::map(value, key)?;
        read::exact_keys(
            found,
            key,
            &["depth_move", "pulse_held", "pulse_release", "steer_x", "steer_y"],
        )?;
        Ok(ControlState {
            steer_x: read::int(found, "steer_x", -32_767, 32_767)? as i16,
            steer_y: read::int(found, "steer_y", -32_767, 32_767)? as i16,
            pulse_held: read::flag(found, "pulse_held")?,
            pulse_release: read::flag(found, "pulse_release")?,
            depth_move: read::int(found, "depth_move", -1, 1)? as i8,
        })
    }

    fn write(&self, out: &mut String) {
        let mut object = Obj::new(out);
        object.int("depth_move", i64::from(self.depth_move));
        object.bool("pulse_held", self.pulse_held);
        object.bool("pulse_release", self.pulse_release);
        object.int("steer_x", i64::from(self.steer_x));
        object.int("steer_y", i64::from(self.steer_y));
        object.end();
    }
}

/// One completed step's records: the stream position before the step ran, the
/// control that step consumed, and the per-Node and per-Route records.
#[derive(Clone, Debug)]
pub struct TraceStep {
    pub step: Step,
    pub rng: RngState,
    pub ctl: ControlState,
    pub records: StepRecords,
}

impl TraceStep {
    /// Reads one recorded step out of a payload.
    fn read(value: &Json) -> Result<Self, Fault> {
        read::exact_keys(value, "steps", &["ctl", "e", "f", "q", "rng", "step", "upkeep", "z"])?;
        let mut upkeep = Vec::new();
        for entry in read::list(value, "upkeep", NODES_PER_RUN)? {
            read::exact_keys(entry, "upkeep", &["mix", "node", "v"])?;
            let shares = read::list(entry, "mix", UPKEEP_PURPOSES)?;
            if shares.len() != UPKEEP_PURPOSES {
                return Err(Fault::field("mix"));
            }
            let mut mix = [0 as Fx; UPKEEP_PURPOSES];
            for (place, share) in shares.iter().enumerate() {
                mix[place] = share.as_int().ok_or_else(|| Fault::field("mix"))?;
            }
            upkeep.push(UpkeepRecord {
                node: read::int(entry, "node", 0, i64::from(u32::MAX))? as u32,
                v: read::int(entry, "v", i64::MIN, i64::MAX)?,
                mix,
            });
        }
        Ok(TraceStep {
            step: read::int(value, "step", 0, i64::from(u32::MAX))? as Step,
            rng: RngState::read(value, "rng")?,
            ctl: ControlState::read(value, "ctl")?,
            records: StepRecords {
                q: keyed_read(value, "q", "node", NODES_PER_RUN)?,
                f: keyed_read(value, "f", "route", ROUTES_PER_RUN)?,
                upkeep,
                e: keyed_read(value, "e", "node", NODES_PER_RUN)?,
                z: read::ids(value, "z", NODES_PER_RUN, i64::from(u32::MAX))?,
            },
        })
    }

    fn write(&self, out: &mut String) {
        let mut object = Obj::new(out);
        {
            let mut control = String::new();
            self.ctl.write(&mut control);
            object.raw("ctl", &control);
        }
        // The exogenous term, the flow, the stored Charge, the upkeep with its
        // five-purpose split, and the failure indicator, all ascending. A
        // zero-valued entry is never written, so a step that moved nothing
        // records nothing.
        keyed(&mut object, "e", "node", &self.records.e);
        keyed(&mut object, "f", "route", &self.records.f);
        keyed(&mut object, "q", "node", &self.records.q);
        object.raw("rng", &self.rng.write());
        object.int("step", i64::from(self.step));
        {
            let mut upkeep = object.list("upkeep");
            for entry in &self.records.upkeep {
                let mut written = upkeep.object();
                {
                    let mut mix = written.list("mix");
                    for share in entry.mix {
                        mix.int(share);
                    }
                    mix.end();
                }
                written.int("node", i64::from(entry.node));
                written.int("v", entry.v);
                written.end();
            }
            upkeep.end();
        }
        {
            let mut failed = object.list("z");
            for node in &self.records.z {
                failed.int(i64::from(*node));
            }
            failed.end();
        }
        object.end();
    }

    /// The step as canonical JSON, which is what the encoding caps are measured
    /// against.
    pub fn written(&self) -> String {
        let mut out = String::new();
        self.write(&mut out);
        out
    }
}

/// Reads one identifier-keyed record list back, holding it ascending and to
/// the cap on how many entries the list can carry.
fn keyed_read(
    value: &Json,
    key: &str,
    name: &str,
    longest: usize,
) -> Result<Vec<(u32, Fx)>, Fault> {
    let items = read::list(value, key, longest)?;
    let mut entries = Vec::with_capacity(items.len());
    for item in items {
        read::exact_keys(item, key, &[name, "v"])?;
        entries.push((
            read::int(item, name, 0, i64::from(u32::MAX))? as u32,
            read::int(item, "v", i64::MIN, i64::MAX)?,
        ));
    }
    let ids: Vec<u32> = entries.iter().map(|(id, _)| *id).collect();
    if !read::ascending(&ids) {
        return Err(Fault::field(key));
    }
    Ok(entries)
}

/// One identifier-keyed record list: `[{ "<name>": id, "v": value }]`,
/// ascending, as every record list of a step is written.
fn keyed(object: &mut Obj<'_>, key: &str, name: &str, entries: &[(u32, Fx)]) {
    let mut list = object.list(key);
    for (id, value) in entries {
        let mut written = list.object();
        written.int(name, i64::from(*id));
        written.int("v", *value);
        written.end();
    }
    list.end();
}

/// The complete Field at one instant.
#[derive(Clone, Debug)]
pub struct FieldState {
    pub step: Step,
    pub next_node_id: u32,
    pub next_route_id: u32,
    pub assembly_ordinal: u32,
    pub prev_assembly_step: Option<Step>,
    /// The depth threshold accumulator, clamped and cleared on a trigger.
    pub wheel_accum: i32,
    /// Steps remaining before another depth change may be resolved.
    pub depth_cooldown: u8,
    /// Every list below is in ascending identifier order, which is the order
    /// every rule of the core iterates in.
    pub layers: Vec<FieldLayer>,
    pub ports: Vec<PortState>,
    pub routes: Vec<RouteState>,
    pub forms: Vec<FormState>,
    pub currents: Vec<CurrentState>,
    /// The material compartment used by leakage and every other causal edge
    /// rule. Observation Views are serialized separately on [`RunState`].
    pub physical_compartment: PhysicalCompartment,
    pub boundaries: BoundaryState,
    /// The Trail entries standing until they fall due, in deposit order —
    /// the one list of the Field whose order is not its identifiers', because
    /// what the cap drops is the oldest entry and nothing else says which that
    /// is.
    pub pending: Vec<PendingTrail>,
}

impl FieldState {
    /// The Field a run opens with: no Nodes, no Routes, nothing assembled.
    pub fn opening() -> Self {
        FieldState {
            step: 0,
            // Identifiers are unique within a Run, never reused, and strictly
            // increasing from 1, so the first one handed out is 1.
            next_node_id: 1,
            next_route_id: 1,
            assembly_ordinal: 0,
            prev_assembly_step: None,
            wheel_accum: 0,
            depth_cooldown: 0,
            layers: Vec::new(),
            ports: Vec::new(),
            routes: Vec::new(),
            forms: Vec::new(),
            currents: Vec::new(),
            physical_compartment: PhysicalCompartment::default(),
            boundaries: BoundaryState::default(),
            pending: Vec::new(),
        }
    }

    /// Reads a whole Field out of a payload. Shape, keys, and types are held
    /// here; every range, cap, closed set, and ordering rule is held by
    /// [`crate::field::validate`] afterwards, so a restored Field passes
    /// exactly the checks a live one does.
    fn read(value: &Json, key: &str) -> Result<Self, Fault> {
        Self::read_version(value, key, None)
    }

    /// Reads the old V1 Field shape during the single supported migration.
    /// The then-active View supplies the physical members because V1 had no
    /// independent material-compartment object.
    fn read_v1(value: &Json, key: &str, legacy_inside: &[u32]) -> Result<Self, Fault> {
        Self::read_version(value, key, Some(legacy_inside))
    }

    fn read_version(
        value: &Json,
        key: &str,
        legacy_inside: Option<&[u32]>,
    ) -> Result<Self, Fault> {
        let found = read::map(value, key)?;
        let keys: &[&str] = if legacy_inside.is_some() {
            &[
                "assembly_ordinal",
                "boundaries",
                "currents",
                "depth_cooldown",
                "forms",
                "layers",
                "next_node_id",
                "next_route_id",
                "pending",
                "ports",
                "prev_assembly_step",
                "routes",
                "step",
                "wheel_accum",
            ]
        } else {
            &[
                "assembly_ordinal",
                "boundaries",
                "currents",
                "depth_cooldown",
                "forms",
                "layers",
                "next_node_id",
                "next_route_id",
                "pending",
                "physical_compartment",
                "ports",
                "prev_assembly_step",
                "routes",
                "step",
                "wheel_accum",
            ]
        };
        read::exact_keys(found, key, keys)?;
        let mut layers = Vec::new();
        for entry in read::list(found, "layers", LAYERS_PER_CHAPTER)? {
            layers.push(FieldLayer::read(entry)?);
        }
        let mut ports = Vec::new();
        for entry in read::list(found, "ports", NODES_PER_RUN)? {
            ports.push(PortState::read(entry)?);
        }
        let mut routes = Vec::new();
        for entry in read::list(found, "routes", ROUTES_PER_RUN)? {
            routes.push(RouteState::read(entry)?);
        }
        let mut forms = Vec::new();
        for entry in read::list(found, "forms", FORMS_PER_RUN)? {
            forms.push(FormState::read(entry)?);
        }
        let mut currents = Vec::new();
        for entry in read::list(found, "currents", CURRENTS_PER_CHAPTER)? {
            currents.push(CurrentState::read(entry)?);
        }
        let mut pending = Vec::new();
        for entry in read::list(found, "pending", PENDING_TRAILS)? {
            pending.push(PendingTrail::read(entry)?);
        }
        let (boundaries, physical_compartment) = match legacy_inside {
            Some(members) => {
                let (boundaries, leak_frac) = BoundaryState::read_v1(found, "boundaries")?;
                (
                    boundaries,
                    PhysicalCompartment {
                        members: members.to_vec(),
                        leak_per_exposed_contact_per_step: leak_frac,
                    },
                )
            }
            None => (
                BoundaryState::read(found, "boundaries")?,
                PhysicalCompartment::read(found, "physical_compartment")?,
            ),
        };
        Ok(FieldState {
            pending,
            step: read::int(found, "step", 0, i64::from(u32::MAX))? as Step,
            next_node_id: read::int(found, "next_node_id", 1, i64::from(u32::MAX))? as u32,
            next_route_id: read::int(found, "next_route_id", 1, i64::from(u32::MAX))? as u32,
            assembly_ordinal: read::int(found, "assembly_ordinal", 0, i64::from(u32::MAX))? as u32,
            prev_assembly_step: read::int_or_null(
                found,
                "prev_assembly_step",
                0,
                i64::from(u32::MAX),
            )?
            .map(|step| step as Step),
            wheel_accum: read::int(found, "wheel_accum", -480, 480)? as i32,
            depth_cooldown: read::int(found, "depth_cooldown", 0, 15)? as u8,
            layers,
            ports,
            routes,
            forms,
            currents,
            physical_compartment,
            boundaries,
        })
    }

    fn write(&self, out: &mut String) {
        let mut object = Obj::new(out);
        object.int("assembly_ordinal", i64::from(self.assembly_ordinal));
        {
            let mut boundaries = String::new();
            self.boundaries.write(&mut boundaries);
            object.raw("boundaries", &boundaries);
        }
        {
            let mut currents = object.list("currents");
            for current in &self.currents {
                let mut written = String::new();
                current.write(&mut written);
                currents.raw(&written);
            }
            currents.end();
        }
        object.int("depth_cooldown", i64::from(self.depth_cooldown));
        {
            let mut forms = object.list("forms");
            for form in &self.forms {
                let mut written = String::new();
                form.write(&mut written);
                forms.raw(&written);
            }
            forms.end();
        }
        {
            let mut layers = object.list("layers");
            for layer in &self.layers {
                let mut written = String::new();
                layer.write(&mut written);
                layers.raw(&written);
            }
            layers.end();
        }
        object.int("next_node_id", i64::from(self.next_node_id));
        object.int("next_route_id", i64::from(self.next_route_id));
        {
            let mut pending = object.list("pending");
            for entry in &self.pending {
                let mut written = String::new();
                entry.write(&mut written);
                pending.raw(&written);
            }
            pending.end();
        }
        {
            let mut physical_compartment = String::new();
            self.physical_compartment.write(&mut physical_compartment);
            object.raw("physical_compartment", &physical_compartment);
        }
        {
            let mut ports = object.list("ports");
            for port in &self.ports {
                let mut written = String::new();
                port.write(&mut written);
                ports.raw(&written);
            }
            ports.end();
        }
        object.int_or_null("prev_assembly_step", self.prev_assembly_step.map(i64::from));
        {
            let mut routes = object.list("routes");
            for route in &self.routes {
                let mut written = String::new();
                route.write(&mut written);
                routes.raw(&written);
            }
            routes.end();
        }
        object.int("step", i64::from(self.step));
        object.int("wheel_accum", i64::from(self.wheel_accum));
        object.end();
    }

    /// The Field as canonical JSON. Two Fields are the same Field exactly when
    /// these bytes agree, which is what a keyframe carried forward is checked
    /// against.
    pub fn written(&self) -> String {
        let mut out = String::new();
        self.write(&mut out);
        out
    }
}

/// The retained trajectory: a keyframe and the recorded steps after it.
#[derive(Clone, Debug)]
pub struct Trace {
    pub start_step: Step,
    pub keyframe: FieldState,
    pub steps: std::collections::VecDeque<TraceStep>,
}

impl Trace {
    /// The trajectory a run opens with: the opening state, nothing recorded.
    pub fn opening(keyframe: FieldState) -> Self {
        Trace { start_step: 0, keyframe, steps: std::collections::VecDeque::new() }
    }

    /// The furthest back the keyframe may sit for a given completed step:
    /// `max(0, 30 * floor((step - 120) / 30))`.
    ///
    /// It is a floor rather than an equality, because a commit that applies a
    /// change ends the active window and restarts the retained trajectory at
    /// the step it lands on — a change no step recorded may not sit inside a
    /// window a replay reads. A run therefore holds a shorter trace for the
    /// 120 to 150 steps after a commit and the ordinary lag from then on.
    pub fn start_for(step: Step) -> Step {
        let lag = i64::from(step) - i64::from(TRACE_LAG);
        let rate = i64::from(STEPS_PER_SECOND);
        // Floor division, which for a negative lag is the multiple below it.
        let blocks = lag.div_euclid(rate);
        (blocks * rate).max(0) as Step
    }

    /// The widest retained span, in recorded steps: the trace spans 120 to 150
    /// steps once the run is old enough.
    pub const RETAINED_STEPS: usize = 150;

    /// Reads the retained trajectory out of a payload.
    fn read(value: &Json, key: &str) -> Result<Self, Fault> {
        Self::read_version(value, key, None)
    }

    fn read_v1(value: &Json, key: &str, legacy_inside: &[u32]) -> Result<Self, Fault> {
        Self::read_version(value, key, Some(legacy_inside))
    }

    fn read_version(
        value: &Json,
        key: &str,
        legacy_inside: Option<&[u32]>,
    ) -> Result<Self, Fault> {
        let found = read::map(value, key)?;
        read::exact_keys(found, key, &["keyframe", "start_step", "steps"])?;
        let mut steps = std::collections::VecDeque::new();
        for entry in read::list(found, "steps", Trace::RETAINED_STEPS)? {
            steps.push_back(TraceStep::read(entry)?);
        }
        Ok(Trace {
            start_step: read::int(found, "start_step", 0, i64::from(u32::MAX))? as Step,
            keyframe: match legacy_inside {
                Some(inside) => FieldState::read_v1(found, "keyframe", inside)?,
                None => FieldState::read(found, "keyframe")?,
            },
            steps,
        })
    }

    fn write(&self, out: &mut String) {
        let mut object = Obj::new(out);
        {
            let mut keyframe = String::new();
            self.keyframe.write(&mut keyframe);
            object.raw("keyframe", &keyframe);
        }
        object.int("start_step", i64::from(self.start_step));
        {
            let mut steps = object.list("steps");
            for recorded in &self.steps {
                let mut written = String::new();
                recorded.write(&mut written);
                steps.raw(&written);
            }
            steps.end();
        }
        object.end();
    }
}

/// The View tuple, unchanged from the framework.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ViewDeclaration {
    pub inside: Vec<u32>,
    pub resolution: u16,
    pub window: u16,
    pub surround: Surround,
}

impl ViewDeclaration {
    /// The View a run stands on before a chapter is loaded, which the
    /// document locks exactly:
    /// `{ "inside": [], "resolution": 1, "window": 45, "surround":
    /// "adjacent" }`. A player-cleared live View may also have an empty inside;
    /// unlike this opening placeholder, it retains the live View's other three
    /// measurement parameters.
    pub fn opening() -> Self {
        ViewDeclaration {
            inside: Vec::new(),
            resolution: 1,
            window: DEFAULT_WINDOW as u16,
            surround: Surround::Adjacent,
        }
    }

    /// Reads the View tuple out of a payload. `resolution` and `window`
    /// restate rows of the locked capacity table, so their envelope is checked
    /// where a View is established; the shape is checked here.
    pub fn read(value: &Json, key: &str) -> Result<Self, Fault> {
        let found = read::map(value, key)?;
        read::exact_keys(found, key, &["inside", "resolution", "surround", "window"])?;
        let surround = match read::one_of(found, "surround", &["adjacent", "double", "whole"])? {
            0 => Surround::Adjacent,
            1 => Surround::Double,
            _ => Surround::Whole,
        };
        Ok(ViewDeclaration {
            inside: read::ids(found, "inside", NODES_PER_RUN, i64::from(u32::MAX))?,
            resolution: read::int(found, "resolution", 0, i64::from(u16::MAX))? as u16,
            window: read::int(found, "window", 0, i64::from(u16::MAX))? as u16,
            surround,
        })
    }

    pub fn write(&self, out: &mut String) {
        let mut object = Obj::new(out);
        {
            let mut inside = object.list("inside");
            for node in &self.inside {
                inside.int(i64::from(*node));
            }
            inside.end();
        }
        object.int("resolution", i64::from(self.resolution));
        object.text("surround", self.surround.name());
        object.int("window", i64::from(self.window));
        object.end();
    }

    pub fn written(&self) -> String {
        let mut out = String::new();
        self.write(&mut out);
        out
    }
}

/// The single visible objective's state.
#[derive(Clone, Debug)]
pub struct ObjectiveState {
    pub id: String,
    pub state: ObjectiveStage,
    pub progress: Frac,
    pub target: Option<Fx>,
    pub started_step: Step,
    pub completed_step: Option<Step>,
}

impl ObjectiveState {
    /// No objective is offered before a chapter authors one: the hidden
    /// state, whose id is the empty string exactly while that holds.
    pub fn hidden() -> Self {
        ObjectiveState {
            id: String::new(),
            state: ObjectiveStage::Hidden,
            progress: 0,
            target: None,
            started_step: 0,
            completed_step: None,
        }
    }

    fn read(value: &Json, key: &str) -> Result<Self, Fault> {
        let found = read::map(value, key)?;
        read::exact_keys(
            found,
            key,
            &["completed_step", "id", "progress", "started_step", "state", "target"],
        )?;
        let state = match read::one_of(
            found,
            "state",
            &["hidden", "active", "complete", "failed_recoverable"],
        )? {
            0 => ObjectiveStage::Hidden,
            1 => ObjectiveStage::Active,
            2 => ObjectiveStage::Complete,
            _ => ObjectiveStage::FailedRecoverable,
        };
        let id = read::text(found, "id")?.to_string();
        // The id is the empty string exactly when no objective has yet been
        // offered, which is exactly the hidden state.
        if id.is_empty() != (state == ObjectiveStage::Hidden) {
            return Err(Fault::field("id"));
        }
        Ok(ObjectiveState {
            id,
            state,
            progress: read::int(found, "progress", 0, FRAC_ONE)?,
            target: read::int_or_null(found, "target", i64::MIN, i64::MAX)?,
            started_step: read::int(found, "started_step", 0, i64::from(u32::MAX))? as Step,
            completed_step: read::int_or_null(found, "completed_step", 0, i64::from(u32::MAX))?
                .map(|step| step as Step),
        })
    }

    /// The objective as canonical JSON, which is what the `objective_changed`
    /// event carries and what the payload holds.
    pub fn written(&self) -> String {
        let mut out = String::new();
        self.write(&mut out);
        out
    }

    fn write(&self, out: &mut String) {
        let mut object = Obj::new(out);
        object.int_or_null("completed_step", self.completed_step.map(i64::from));
        object.text("id", &self.id);
        object.int("progress", self.progress);
        object.int("started_step", i64::from(self.started_step));
        object.text("state", self.state.name());
        object.int_or_null("target", self.target);
        object.end();
    }
}

/// Chapter and objective progress, and the Impulse the run carries.
///
/// The Impulse lives here rather than on the Field, and the reason is replay
/// safety. A layer objective's completion gives three Impulse back, and the
/// authored sequence is the one thing that knows a completion happened: it runs
/// after a step and writes into progress, which no replay re-runs. A quantity
/// the script writes into `FieldState` would be a quantity the trajectory
/// keyframe carry could not reproduce — the carry replays the step function and
/// nothing else — so it would either be lost by every regeneration or granted
/// twice by one. Progress is the field a script may write without touching the
/// keyframe, which is exactly what the grant needs.
#[derive(Clone, Debug)]
pub struct Progress {
    pub chapter_index: u8,
    pub objective: ObjectiveState,
    pub complete: Vec<String>,
    /// What a queued change is paid for out of, in [0, `IMPULSE_CAP`].
    pub impulse: u8,
}

impl Progress {
    pub fn opening() -> Self {
        Progress {
            chapter_index: 0,
            objective: ObjectiveState::hidden(),
            complete: Vec::new(),
            impulse: OPENING_IMPULSE,
        }
    }

    /// Gives back what one completed layer objective is worth, held at the cap.
    pub fn grant_impulse(&mut self) {
        self.impulse = self.impulse.saturating_add(IMPULSE_GRANT).min(IMPULSE_CAP);
    }

    fn read(value: &Json, key: &str) -> Result<Self, Fault> {
        let found = read::map(value, key)?;
        read::exact_keys(found, key, &["chapter_index", "complete", "impulse", "objective"])?;
        let mut complete = Vec::new();
        // The ascending list of completed objective ids.
        for entry in read::list(found, "complete", NODES_PER_RUN)? {
            complete.push(entry.as_text().ok_or_else(|| Fault::field("complete"))?.to_string());
        }
        if !read::ascending(&complete) {
            return Err(Fault::field("complete"));
        }
        Ok(Progress {
            chapter_index: read::int(found, "chapter_index", 0, 7)? as u8,
            objective: ObjectiveState::read(found, "objective")?,
            complete,
            impulse: read::int(found, "impulse", 0, i64::from(IMPULSE_CAP))? as u8,
        })
    }

    fn write(&self, out: &mut String) {
        let mut object = Obj::new(out);
        object.int("chapter_index", i64::from(self.chapter_index));
        {
            let mut complete = object.list("complete");
            for id in &self.complete {
                complete.text(id);
            }
            complete.end();
        }
        object.int("impulse", i64::from(self.impulse));
        {
            let mut objective = String::new();
            self.objective.write(&mut objective);
            object.raw("objective", &objective);
        }
        object.end();
    }
}

/// The input configuration, as the shell reads and writes it.
#[derive(Clone, Debug)]
pub struct InputConfig {
    pub bindings: [(&'static str, String); 10],
    pub pointer_speed: Frac,
    pub reduced_motion: bool,
    pub trail_intensity: Frac,
    pub sound_level: Frac,
}

impl InputConfig {
    /// The locked default bindings, and the unscaled setting for each of the
    /// three scales the configuration carries.
    pub fn default_config() -> Self {
        InputConfig {
            bindings: [
                ("ascend", "BracketLeft".to_string()),
                ("cancel", "Escape".to_string()),
                ("commit", "Enter".to_string()),
                ("descend", "BracketRight".to_string()),
                ("down", "KeyS".to_string()),
                ("left", "KeyA".to_string()),
                ("pulse", "ShiftLeft".to_string()),
                ("right", "KeyD".to_string()),
                ("still", "Space".to_string()),
                ("up", "KeyW".to_string()),
            ],
            pointer_speed: FRAC_ONE,
            reduced_motion: false,
            trail_intensity: FRAC_ONE,
            sound_level: FRAC_ONE,
        }
    }

    fn read(value: &Json, key: &str) -> Result<Self, Fault> {
        let found = read::map(value, key)?;
        read::exact_keys(
            found,
            key,
            &["bindings", "pointer_speed", "reduced_motion", "sound_level", "trail_intensity"],
        )?;
        let declared = read::map(found, "bindings")?;
        let mut bindings = InputConfig::default_config().bindings;
        let names: Vec<&str> = bindings.iter().map(|(name, _)| *name).collect();
        read::exact_keys(declared, "bindings", &names)?;
        for (name, code) in &mut bindings {
            *code = read::text(declared, name)?.to_string();
        }
        Ok(InputConfig {
            bindings,
            pointer_speed: read::int(found, "pointer_speed", 16_384, 262_144)?,
            reduced_motion: read::flag(found, "reduced_motion")?,
            trail_intensity: read::int(found, "trail_intensity", 0, FRAC_ONE)?,
            sound_level: read::int(found, "sound_level", 0, FRAC_ONE)?,
        })
    }

    fn write(&self, out: &mut String) {
        let mut object = Obj::new(out);
        {
            let mut bindings = object.object("bindings");
            for (name, code) in &self.bindings {
                bindings.text(name, code);
            }
            bindings.end();
        }
        object.int("pointer_speed", self.pointer_speed);
        object.bool("reduced_motion", self.reduced_motion);
        object.int("sound_level", self.sound_level);
        object.int("trail_intensity", self.trail_intensity);
        object.end();
    }
}

/// The two kinds of record a run writes: one the campaign script writes at a
/// major moment, and one the autosave cadence writes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecordKind {
    Anchor,
    Auto,
}

impl RecordKind {
    pub fn name(self) -> &'static str {
        match self {
            RecordKind::Anchor => "anchor",
            RecordKind::Auto => "auto",
        }
    }
}

/// Anchor metadata: the payload itself lives in the persistence record the
/// `save_key` names, and `rng` is the trajectory position at the write, so a
/// Quick Retry restores the exact random state.
#[derive(Clone, Debug)]
pub struct CheckpointState {
    pub anchor_id: u32,
    pub step: Step,
    pub chapter_index: u8,
    pub objective_id: String,
    pub kind: RecordKind,
    pub save_key: String,
    pub rng: RngState,
    pub branch_nonce: u32,
}

impl CheckpointState {
    fn read(value: &Json) -> Result<Self, Fault> {
        read::exact_keys(
            value,
            "anchors",
            &[
                "anchor_id",
                "branch_nonce",
                "chapter_index",
                "kind",
                "objective_id",
                "rng",
                "save_key",
                "step",
            ],
        )?;
        let kind = match read::one_of(value, "kind", &["anchor", "auto"])? {
            0 => RecordKind::Anchor,
            _ => RecordKind::Auto,
        };
        Ok(CheckpointState {
            anchor_id: read::int(value, "anchor_id", 1, i64::from(u32::MAX))? as u32,
            step: read::int(value, "step", 0, i64::from(u32::MAX))? as Step,
            chapter_index: read::int(value, "chapter_index", 0, 7)? as u8,
            objective_id: read::text(value, "objective_id")?.to_string(),
            kind,
            save_key: read::text(value, "save_key")?.to_string(),
            rng: RngState::read(value, "rng")?,
            branch_nonce: read::int(value, "branch_nonce", 0, i64::from(u32::MAX))? as u32,
        })
    }

    pub fn write(&self, out: &mut String) {
        let mut object = Obj::new(out);
        object.int("anchor_id", i64::from(self.anchor_id));
        object.int("branch_nonce", i64::from(self.branch_nonce));
        object.int("chapter_index", i64::from(self.chapter_index));
        object.text("kind", self.kind.name());
        object.text("objective_id", &self.objective_id);
        object.raw("rng", &self.rng.write());
        object.text("save_key", &self.save_key);
        object.int("step", i64::from(self.step));
        object.end();
    }

    pub fn written(&self) -> String {
        let mut out = String::new();
        self.write(&mut out);
        out
    }
}

/// Authored rules pinned for the life of a run.
///
/// The fields stay private so embodied transitions cannot rewrite the content
/// identity or pressure schedule in place. A restored run receives the same
/// content-pinned specification from the session that opened it.
#[derive(Clone, Debug)]
pub struct GeneratorSpec {
    content_hash: String,
    schedule: crate::pressure::Schedule,
}

impl GeneratorSpec {
    pub fn new(content_hash: String, schedule: crate::pressure::Schedule) -> Self {
        GeneratorSpec {
            content_hash,
            schedule,
        }
    }

    pub fn content_hash(&self) -> &str {
        &self.content_hash
    }

    pub fn schedule(&self) -> &crate::pressure::Schedule {
        &self.schedule
    }
}

/// The root authoritative aggregate, whose serialized form is exactly the
/// current save payload.
#[derive(Clone, Debug)]
pub struct RunState {
    pub run_id: String,
    pub rng: RngState,
    pub spec: GeneratorSpec,
    pub branch_nonce: u32,
    pub progress: Progress,
    pub now: FieldState,
    pub trace: Trace,
    pub view: ViewDeclaration,
    /// The evaluation record the run stands under, and none before the first
    /// slate is assembled. A `set_focus` names a position in it, and it rides
    /// the payload so that a restored run stands under the slate it was
    /// written beside.
    pub slate: Option<CandidateSlate>,
    pub input_config: InputConfig,
    /// The staged pressures, active and queued together, ascending by
    /// `pressure` name order within the closed set.
    ///
    /// It is a step input the moment it is nonempty, and what keeps a
    /// regeneration exact is [`crate::pressure`]'s carriage rule rather than a
    /// wider trace: membership is immutable inside a window, every rule that
    /// changes it ends the window, and `stage` and `level` are derived at the
    /// step being replayed.
    pub pressures: Vec<crate::pressure::PressureState>,
    /// Checkpoint metadata, ascending by `anchor_id`. The payloads themselves
    /// live in the persistence records these name.
    pub anchors: Vec<CheckpointState>,
}

impl RunState {
    /// Reads a whole save payload back.
    ///
    /// The payload is exactly the twelve locked keys, nothing more, and each is
    /// read through the strict readers. `pressures` is read whole since Goal
    /// 18: the list this build refused before is exactly the list it now
    /// re-serializes, held to the locked ordering and the two locked invariants
    /// — at most two active, at most one primary. `slate` is read whole too —
    /// null before the first assembly, and otherwise the evaluation record,
    /// itself held to the parts this build computes.
    pub fn read(payload: &Json) -> Result<Self, Fault> {
        Self::read_version(payload, SAVE_VERSION)
    }

    /// Deterministically migrates one verified canonical V1 payload into the
    /// V2 in-memory model. The importer verifies the original bytes and digest
    /// before calling this; this reader then holds the entire V1 shape to its
    /// schema, moves each Field's former boundary coefficient plus the active
    /// View's members into an authoritative physical compartment, and discards
    /// the legacy slate after validating it because its readings were produced
    /// under the superseded causal model.
    pub fn migrate_v1(payload: &Json) -> Result<Self, Fault> {
        Self::read_version(payload, 1)
    }

    fn read_version(payload: &Json, version: i64) -> Result<Self, Fault> {
        read::exact_keys(
            payload,
            "payload",
            &[
                "anchors",
                "branch_nonce",
                "content_hash",
                "field",
                "input_config",
                "pressures",
                "progress",
                "rng",
                "run_id",
                "save_version",
                "slate",
                "view",
            ],
        )?;
        if read::int(payload, "save_version", version, version)? != version {
            return Err(Fault::field("save_version"));
        }
        let parsed_slate = match read::at(payload, "slate")? {
            Json::Null => None,
            _ => Some(CandidateSlate::read(payload, "slate")?),
        };
        let slate = if version == 1 { None } else { parsed_slate };
        let pressures = crate::pressure::read_list(payload, "pressures")?;
        let view = ViewDeclaration::read(payload, "view")?;

        let field = read::map(payload, "field")?;
        read::exact_keys(field, "field", &["now", "trace"])?;

        // The checkpoint metadata list is not capped: the 64-record cap is a cap
        // on stored records, and the metadata of a pruned record is kept. The
        // payload's own 8 MiB cap is what bounds this list.
        let mut anchors = Vec::new();
        for entry in read::list(payload, "anchors", usize::MAX)? {
            anchors.push(CheckpointState::read(entry)?);
        }
        let ids: Vec<u32> = anchors.iter().map(|anchor| anchor.anchor_id).collect();
        if !read::ascending(&ids) {
            return Err(Fault::field("anchors"));
        }

        let (now, trace) = if version == 1 {
            (
                FieldState::read_v1(field, "now", &view.inside)?,
                Trace::read_v1(field, "trace", &view.inside)?,
            )
        } else {
            (FieldState::read(field, "now")?, Trace::read(field, "trace")?)
        };

        let content_hash = read::hex(payload, "content_hash", 64)?.to_string();
        Ok(RunState {
            run_id: read::hex(payload, "run_id", 16)?.to_string(),
            rng: RngState::read(payload, "rng")?,
            spec: GeneratorSpec::new(content_hash, crate::pressure::Schedule::default()),
            branch_nonce: read::int(payload, "branch_nonce", 0, i64::from(u32::MAX))? as u32,
            progress: Progress::read(payload, "progress")?,
            now,
            trace,
            view,
            slate,
            input_config: InputConfig::read(payload, "input_config")?,
            pressures,
            anchors,
        })
    }

    /// The canonical bytes of the save payload — the whole of the
    /// byte-equivalence contract.
    pub fn payload(&self) -> String {
        let mut out = String::new();
        let mut object = Obj::new(&mut out);
        {
            let mut anchors = object.list("anchors");
            for anchor in &self.anchors {
                anchors.raw(&anchor.written());
            }
            anchors.end();
        }
        object.int("branch_nonce", i64::from(self.branch_nonce));
        object.text("content_hash", self.spec.content_hash());
        {
            let mut field = object.object("field");
            {
                let mut now = String::new();
                self.now.write(&mut now);
                field.raw("now", &now);
            }
            {
                let mut trace = String::new();
                self.trace.write(&mut trace);
                field.raw("trace", &trace);
            }
            field.end();
        }
        {
            let mut config = String::new();
            self.input_config.write(&mut config);
            object.raw("input_config", &config);
        }
        // Active and queued pressures together, ascending in the closed set's
        // own order — the one order the reader accepts and the event carries.
        object.raw("pressures", &crate::pressure::written_list(&self.pressures));
        {
            let mut progress = String::new();
            self.progress.write(&mut progress);
            object.raw("progress", &progress);
        }
        object.raw("rng", &self.rng.write());
        object.text("run_id", &self.run_id);
        object.int("save_version", SAVE_VERSION);
        // The evaluation record is null before the first assembly.
        match &self.slate {
            Some(slate) => object.raw("slate", &slate.written()),
            None => object.null("slate"),
        };
        {
            let mut view = String::new();
            self.view.write(&mut view);
            object.raw("view", &view);
        }
        object.end();
        out
    }

    /// Holds a state read back from a payload to the invariants a state built
    /// by playing forward cannot leave.
    ///
    /// Shape and ranges are the readers' business; this is the structure between
    /// the parts: the Field and the trajectory's keyframe each pass the whole
    /// Field validation, the keyframe sits exactly where the retained span puts
    /// it, the recorded steps run contiguously from it to the completed step,
    /// and the standing View is a valid passive measurement protocol, including
    /// the empty selection a player-cleared View carries.
    pub fn coherent(&self) -> Result<(), Fault> {
        crate::field::validate(&self.now)?;
        crate::field::validate(&self.trace.keyframe)?;
        if self.now.physical_compartment != self.trace.keyframe.physical_compartment {
            return Err(Fault::field("physical_compartment"));
        }
        // The keyframe sits where the retained span puts it, or later: a commit
        // that applied a change restarted the trajectory at its own step, and
        // the span grows back from there.
        if self.trace.start_step < Trace::start_for(self.now.step)
            || self.trace.start_step > self.now.step
        {
            return Err(Fault::field("start_step"));
        }
        if self.trace.keyframe.step != self.trace.start_step {
            return Err(Fault::field("keyframe"));
        }
        if self.trace.steps.len() as i64 != i64::from(self.now.step - self.trace.start_step) {
            return Err(Fault::field("steps"));
        }
        for (place, recorded) in self.trace.steps.iter().enumerate() {
            if recorded.step != self.trace.start_step + place as Step + 1 {
                return Err(Fault::field("steps"));
            }
        }
        // A live View may be cleared: no selected Node is still a complete
        // passive measurement protocol once resolution, window, and surround
        // are retained. Chapter establishment has the separate, stricter rule
        // that authored opening Views are nonempty.
        crate::field::validate_view(&self.view, &self.now)?;
        // The two locked pressure invariants and the ordering that carries
        // them. A payload read through `RunState::read` has passed these
        // already; a state built by playing forward passes them because the
        // stage machine is the only rule that seats a pressure and it admits
        // per the limit.
        crate::pressure::validate_list(&self.pressures)?;
        Ok(())
    }

    /// How many completed steps the retained trajectory holds behind the step
    /// the run stands on.
    ///
    /// It is the ordinary 120 to 150 once a run is old enough, less while a run
    /// is young, and 0 at the instant an applying commit ends the active window
    /// and restarts the trajectory on the state it left.
    pub fn retained_span(&self) -> Step {
        self.now.step - self.trace.start_step
    }

    /// The effective window a procedure reads, for a declared window `w`.
    ///
    /// `docs/field-framework/ARCHITECTURE.md` locks it under Boundary leakage as
    /// `w_eff = min(w, t0, retained_span)`, which is FRAMEWORK.md's own
    /// `min(w, t0)` with one term added: a window may not reach past what the
    /// trajectory retains, because a commit that applied a change restarted the
    /// trajectory and nothing before that point may be replayed under the Field
    /// the commit left.
    ///
    /// The consequence is FRAMEWORK.md's, unchanged and needing no amendment:
    /// at `w_eff = 0` there are no steps to observe and every windowed procedure
    /// is unassigned, and below the minimum a procedure names it is unassigned
    /// there too. Right after an applying commit that is exactly the reading —
    /// nothing has been observed of the Field the commit made yet — and the
    /// span regrows one step at a time from there.
    /// The three terms are written out rather than reduced: the retained span
    /// never reaches past the completed step, so the `t0` term is already
    /// subsumed by it, and keeping it visible is what makes this line and the
    /// locked expression read as the same rule.
    pub fn effective_window(&self, declared: u16) -> u16 {
        let span = self.retained_span().min(self.now.step);
        u16::try_from(span).unwrap_or(u16::MAX).min(declared)
    }

    /// The export file: the payload, its hash, and the format marker.
    pub fn export_file(&self) -> String {
        let payload = self.payload();
        let hash = crate::json::hex_bytes(&sha256::digest(payload.as_bytes()));
        let mut out = String::new();
        let mut object = Obj::new(&mut out);
        object.text("format", EXPORT_FORMAT);
        object.raw("payload", &payload);
        object.text("payload_sha256", &hash);
        object.int("save_version", SAVE_VERSION);
        object.end();
        out
    }

    /// The name the shell offers when the player saves an export.
    pub fn filename_hint(&self) -> String {
        format!("field-run-{}-step-{}.json", self.run_id, self.now.step)
    }
}

/// The marker every export file opens with.
pub const EXPORT_FORMAT: &str = "field-game-run";
