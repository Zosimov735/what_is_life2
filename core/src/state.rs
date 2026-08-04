//! The authoritative state and its serialized form.
//!
//! `RunState`'s serialized form is the canonical V7 save payload — one live
//! shape, so the byte-equivalence contract and the save format cannot drift
//! apart. V1 and V2 readers exist only as verified one-way migrations.
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
    BoundaryState, CurrentDelay, CurrentState, FieldLayer, FormState, LeakBreach, LocalSignalState,
    MaterialState, NodeKind, PendingTrail, PhysicalCompartment, PortState, RouteClamp, RouteScramble,
    RouteState, RouteTransferRuntime,
    StepRecords, SupplyDecoy, UpkeepRecord, CURRENTS_PER_CHAPTER, CURRENT_DELAYS_PER_RUN,
    FORMS_PER_RUN, LAYERS_PER_CHAPTER,
    LOCAL_SIGNALS_PER_RUN, MATERIALS_PER_RUN, NODES_PER_RUN, PENDING_TRAILS, ROUTES_PER_RUN,
    ROUTE_CLAMPS, SUPPLY_DECOYS_PER_RUN, UPKEEP_PURPOSES,
};
use crate::json::{Arr, Json, Obj};
use crate::policy::{FrozenLocalPolicy, PolicyRuntimeState, RouteControlState};
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
pub const SAVE_VERSION: i64 = 7;

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

pub const REGIME_IDS: [&str; 5] = [
    "open_field",
    "periodic_transport",
    "crowded_medium",
    "vestige_pressure",
    "holdout_atmosphere",
];

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
    pub fn read(value: &Json, key: &str) -> Result<Self, Fault> {
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
    pub next_signal_id: u32,
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
    /// Last-step Route allocation evidence. It is derived afresh by advance,
    /// omitted from canonical persistence, and used only by inspection,
    /// mechanism events, and the render snapshot.
    pub route_runtime: Vec<RouteTransferRuntime>,
    /// Persistent local actuator settings, ascending by Route id.
    pub route_controls: Vec<RouteControlState>,
    /// Temporary paid Route ceilings, ascending by Route id.
    pub route_clamps: Vec<RouteClamp>,
    /// Temporary paid increase to material-boundary leakage.
    pub leak_breach: Option<LeakBreach>,
    pub forms: Vec<FormState>,
    pub currents: Vec<CurrentState>,
    pub materials: Vec<MaterialState>,
    pub signals: Vec<LocalSignalState>,
    /// Persistent local decision clocks and selected targets, ascending by address.
    pub policy_runtime: Vec<PolicyRuntimeState>,
    pub supply_decoys: Vec<SupplyDecoy>,
    pub current_delays: Vec<CurrentDelay>,
    pub route_scramble: Option<RouteScramble>,
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
            next_signal_id: 1,
            assembly_ordinal: 0,
            prev_assembly_step: None,
            wheel_accum: 0,
            depth_cooldown: 0,
            layers: Vec::new(),
            ports: Vec::new(),
            routes: Vec::new(),
            route_runtime: Vec::new(),
            route_controls: Vec::new(),
            route_clamps: Vec::new(),
            leak_breach: None,
            forms: Vec::new(),
            currents: Vec::new(),
            materials: Vec::new(),
            signals: Vec::new(),
            policy_runtime: Vec::new(),
            supply_decoys: Vec::new(),
            current_delays: Vec::new(),
            route_scramble: None,
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
        let has_route_clamps = found.get("route_clamps").is_some();
        let has_interventions = has_route_clamps && found.get("leak_breach").is_some();
        let has_renewal = has_interventions
            && found.get("materials").is_some()
            && found.get("signals").is_some()
            && found.get("next_signal_id").is_some();
        let has_decoys = has_renewal && found.get("supply_decoys").is_some();
        let has_delays = has_decoys && found.get("current_delays").is_some();
        let has_scramble = has_delays && found.get("route_scramble").is_some();
        let has_policy_runtime = has_scramble
            && found.get("policy_runtime").is_some()
            && found.get("route_controls").is_some();
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
        } else if has_policy_runtime {
            &[
                "assembly_ordinal",
                "boundaries",
                "current_delays",
                "currents",
                "depth_cooldown",
                "forms",
                "layers",
                "leak_breach",
                "materials",
                "next_node_id",
                "next_route_id",
                "next_signal_id",
                "pending",
                "physical_compartment",
                "policy_runtime",
                "ports",
                "prev_assembly_step",
                "route_clamps",
                "route_controls",
                "route_scramble",
                "routes",
                "signals",
                "step",
                "supply_decoys",
                "wheel_accum",
            ]
        } else if has_scramble {
            &[
                "assembly_ordinal",
                "boundaries",
                "current_delays",
                "currents",
                "depth_cooldown",
                "forms",
                "layers",
                "leak_breach",
                "materials",
                "next_node_id",
                "next_route_id",
                "next_signal_id",
                "pending",
                "physical_compartment",
                "ports",
                "prev_assembly_step",
                "route_clamps",
                "route_scramble",
                "routes",
                "signals",
                "step",
                "supply_decoys",
                "wheel_accum",
            ]
        } else if has_delays {
            &[
                "assembly_ordinal",
                "boundaries",
                "current_delays",
                "currents",
                "depth_cooldown",
                "forms",
                "layers",
                "leak_breach",
                "materials",
                "next_node_id",
                "next_route_id",
                "next_signal_id",
                "pending",
                "physical_compartment",
                "ports",
                "prev_assembly_step",
                "route_clamps",
                "routes",
                "signals",
                "step",
                "supply_decoys",
                "wheel_accum",
            ]
        } else if has_decoys {
            &[
                "assembly_ordinal",
                "boundaries",
                "currents",
                "depth_cooldown",
                "forms",
                "layers",
                "leak_breach",
                "materials",
                "next_node_id",
                "next_route_id",
                "next_signal_id",
                "pending",
                "physical_compartment",
                "ports",
                "prev_assembly_step",
                "route_clamps",
                "routes",
                "signals",
                "step",
                "supply_decoys",
                "wheel_accum",
            ]
        } else if has_renewal {
            &[
                "assembly_ordinal",
                "boundaries",
                "currents",
                "depth_cooldown",
                "forms",
                "layers",
                "leak_breach",
                "materials",
                "next_node_id",
                "next_route_id",
                "next_signal_id",
                "pending",
                "physical_compartment",
                "ports",
                "prev_assembly_step",
                "route_clamps",
                "routes",
                "signals",
                "step",
                "wheel_accum",
            ]
        } else if has_interventions {
            &[
                "assembly_ordinal",
                "boundaries",
                "currents",
                "depth_cooldown",
                "forms",
                "layers",
                "leak_breach",
                "next_node_id",
                "next_route_id",
                "pending",
                "physical_compartment",
                "ports",
                "prev_assembly_step",
                "route_clamps",
                "routes",
                "step",
                "wheel_accum",
            ]
        } else if has_route_clamps {
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
                "route_clamps",
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
        let mut route_clamps = Vec::new();
        if has_route_clamps {
            for entry in read::list(found, "route_clamps", ROUTE_CLAMPS)? {
                route_clamps.push(RouteClamp::read(entry)?);
            }
        }
        let leak_breach = if has_interventions {
            match read::map_or_null(found, "leak_breach")? {
                Some(value) => Some(LeakBreach::read(value)?),
                None => None,
            }
        } else {
            None
        };
        let mut materials = Vec::new();
        let mut signals = Vec::new();
        let mut supply_decoys = Vec::new();
        let mut current_delays = Vec::new();
        let mut policy_runtime = Vec::new();
        let mut route_controls = Vec::new();
        if has_renewal {
            for entry in read::list(found, "materials", MATERIALS_PER_RUN)? {
                materials.push(MaterialState::read(entry)?);
            }
            for entry in read::list(found, "signals", LOCAL_SIGNALS_PER_RUN)? {
                signals.push(LocalSignalState::read(entry)?);
            }
        }
        if has_policy_runtime {
            for entry in read::list(found, "policy_runtime", NODES_PER_RUN)? {
                policy_runtime.push(PolicyRuntimeState::read(entry)?);
            }
            for entry in read::list(found, "route_controls", ROUTES_PER_RUN)? {
                route_controls.push(RouteControlState::read(entry)?);
            }
        } else {
            route_controls = routes
                .iter()
                .map(|route| RouteControlState::opening(route.route, route.tail, route.capacity))
                .collect();
        }
        if has_decoys {
            for entry in read::list(found, "supply_decoys", SUPPLY_DECOYS_PER_RUN)? {
                supply_decoys.push(SupplyDecoy::read(entry)?);
            }
        }
        if has_delays {
            for entry in read::list(found, "current_delays", CURRENT_DELAYS_PER_RUN)? {
                current_delays.push(CurrentDelay::read(entry)?);
            }
        }
        let route_scramble = if has_scramble {
            match read::map_or_null(found, "route_scramble")? {
                Some(value) => Some(RouteScramble::read(value)?),
                None => None,
            }
        } else {
            None
        };
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
            route_clamps,
            leak_breach,
            materials,
            signals,
            supply_decoys,
            current_delays,
            route_scramble,
            step: read::int(found, "step", 0, i64::from(u32::MAX))? as Step,
            next_node_id: read::int(found, "next_node_id", 1, i64::from(u32::MAX))? as u32,
            next_route_id: read::int(found, "next_route_id", 1, i64::from(u32::MAX))? as u32,
            next_signal_id: if has_renewal {
                read::int(found, "next_signal_id", 1, i64::from(u32::MAX))? as u32
            } else {
                1
            },
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
            route_runtime: Vec::new(),
            route_controls,
            forms,
            currents,
            policy_runtime,
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
            let mut delays = object.list("current_delays");
            for delay in &self.current_delays {
                let mut written = String::new();
                delay.write(&mut written);
                delays.raw(&written);
            }
            delays.end();
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
        match &self.leak_breach {
            Some(breach) => {
                let mut written = String::new();
                breach.write(&mut written);
                object.raw("leak_breach", &written);
            }
            None => {
                object.null("leak_breach");
            }
        }
        {
            let mut materials = object.list("materials");
            for material in &self.materials {
                let mut written = String::new();
                material.write(&mut written);
                materials.raw(&written);
            }
            materials.end();
        }
        object.int("next_node_id", i64::from(self.next_node_id));
        object.int("next_route_id", i64::from(self.next_route_id));
        object.int("next_signal_id", i64::from(self.next_signal_id));
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
            let mut runtime = object.list("policy_runtime");
            for state in &self.policy_runtime {
                runtime.raw(&state.written());
            }
            runtime.end();
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
            let mut route_clamps = object.list("route_clamps");
            for clamp in &self.route_clamps {
                let mut written = String::new();
                clamp.write(&mut written);
                route_clamps.raw(&written);
            }
            route_clamps.end();
        }
        {
            let mut controls = object.list("route_controls");
            for control in &self.route_controls {
                controls.raw(&control.written());
            }
            controls.end();
        }
        match &self.route_scramble {
            Some(scramble) => {
                let mut written = String::new();
                scramble.write(&mut written);
                object.raw("route_scramble", &written);
            }
            None => {
                object.null("route_scramble");
            }
        }
        {
            let mut routes = object.list("routes");
            for route in &self.routes {
                let mut written = String::new();
                route.write(&mut written);
                routes.raw(&written);
            }
            routes.end();
        }
        {
            let mut signals = object.list("signals");
            for signal in &self.signals {
                let mut written = String::new();
                signal.write(&mut written);
                signals.raw(&written);
            }
            signals.end();
        }
        object.int("step", i64::from(self.step));
        {
            let mut decoys = object.list("supply_decoys");
            for decoy in &self.supply_decoys {
                let mut written = String::new();
                decoy.write(&mut written);
                decoys.raw(&written);
            }
            decoys.end();
        }
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

    /// SHA-256 identity of embodied causal state only. Scenario, generator,
    /// observation View, progress, RNG, and retained history are deliberately
    /// outside this digest.
    pub fn embodied_hash(&self) -> String {
        crate::json::hex_bytes(&sha256::digest(self.written().as_bytes()))
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

    pub fn written(&self) -> String {
        let mut out = String::new();
        self.write(&mut out);
        out
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
                ("pulse", "KeyE".to_string()),
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
pub struct RegimeSpec {
    id: String,
    supply_scale: Frac,
    supply_duty: Frac,
    dissipation_delta: Fx,
    noise_delta: Frac,
    route_capacity_scale: Frac,
    leak_delta: Frac,
    medium_x: Fx,
    medium_y: Fx,
    medium_drag: Frac,
    supply_jitter: Frac,
    collision_radius: Fx,
    collision_response: Frac,
}

impl RegimeSpec {
    pub fn named(id: &str) -> Result<Self, Fault> {
        let values = match id {
            "open_field" => (FRAC_ONE, FRAC_ONE, 0, 0, FRAC_ONE, 0, 0, 0, 0, 0, 0, 0),
            "periodic_transport" => (FRAC_ONE, 32_768, 0, 2_048, FRAC_ONE, 0, 0, 0, 0, 2_048, 0, 0),
            "crowded_medium" => (49_152, FRAC_ONE, 4_096, 8_192, 49_152, 2_048, 8 * crate::fx::ONE_UNIT, 3 * crate::fx::ONE_UNIT, 24_576, 8_192, 52 * crate::fx::ONE_UNIT, 32_768),
            "vestige_pressure" => (32_768, FRAC_ONE, 8_192, 16_384, 40_960, 8_192, -2 * crate::fx::ONE_UNIT, crate::fx::ONE_UNIT, 12_288, 12_288, 40 * crate::fx::ONE_UNIT, 40_960),
            "holdout_atmosphere" => (FRAC_ONE, FRAC_ONE, 2_048, 4_096, FRAC_ONE, 1_024, crate::fx::ONE_UNIT, -crate::fx::ONE_UNIT, 8_192, 16_384, 32 * crate::fx::ONE_UNIT, 24_576),
            _ => return Err(Fault::field("regime")),
        };
        Ok(RegimeSpec {
            id: id.to_string(),
            supply_scale: values.0,
            supply_duty: values.1,
            dissipation_delta: values.2,
            noise_delta: values.3,
            route_capacity_scale: values.4,
            leak_delta: values.5,
            medium_x: values.6,
            medium_y: values.7,
            medium_drag: values.8,
            supply_jitter: values.9,
            collision_radius: values.10,
            collision_response: values.11,
        })
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    fn read(value: &Json, key: &str) -> Result<Self, Fault> {
        let found = read::map(value, key)?;
        let has_duty = found.get("supply_duty").is_some();
        let has_medium = found.get("medium_drag").is_some();
        let has_jitter = found.get("supply_jitter").is_some();
        let has_collision = found.get("collision_radius").is_some();
        let current = [
                "collision_radius",
                "collision_response",
                "dissipation_delta",
                "id",
                "leak_delta",
                "medium_drag",
                "medium_x",
                "medium_y",
                "noise_delta",
                "route_capacity_scale",
                "supply_duty",
                "supply_jitter",
                "supply_scale",
        ];
        let jitter_only = [
            "dissipation_delta",
            "id",
            "leak_delta",
            "medium_drag",
            "medium_x",
            "medium_y",
            "noise_delta",
            "route_capacity_scale",
            "supply_duty",
            "supply_jitter",
            "supply_scale",
        ];
        let medium_only = [
            "dissipation_delta",
            "id",
            "leak_delta",
            "medium_drag",
            "medium_x",
            "medium_y",
            "noise_delta",
            "route_capacity_scale",
            "supply_duty",
            "supply_scale",
        ];
        let duty_only = [
            "dissipation_delta",
            "id",
            "leak_delta",
            "noise_delta",
            "route_capacity_scale",
            "supply_duty",
            "supply_scale",
        ];
        let legacy = [
            "dissipation_delta",
            "id",
            "leak_delta",
            "noise_delta",
            "route_capacity_scale",
            "supply_scale",
        ];
        let keys = if has_collision { &current[..] } else if has_jitter { &jitter_only[..] } else if has_medium { &medium_only[..] } else if has_duty { &duty_only[..] } else { &legacy[..] };
        read::exact_keys(found, key, keys)?;
        let regime = read::one_of(found, "id", &REGIME_IDS)?;
        let named = RegimeSpec::named(REGIME_IDS[regime])?;
        let held = RegimeSpec {
            id: named.id,
            supply_scale: read::int(found, "supply_scale", 0, FRAC_ONE)?,
            supply_duty: if has_duty {
                read::int(found, "supply_duty", 1, FRAC_ONE)?
            } else {
                FRAC_ONE
            },
            dissipation_delta: read::int(found, "dissipation_delta", 0, i64::MAX)?,
            noise_delta: read::int(found, "noise_delta", 0, FRAC_ONE)?,
            route_capacity_scale: read::int(found, "route_capacity_scale", 0, FRAC_ONE)?,
            leak_delta: read::int(found, "leak_delta", 0, FRAC_ONE)?,
            medium_x: if has_medium { read::int(found, "medium_x", -crate::field::PLANE_SPAN, crate::field::PLANE_SPAN)? } else { named.medium_x },
            medium_y: if has_medium { read::int(found, "medium_y", -crate::field::PLANE_SPAN, crate::field::PLANE_SPAN)? } else { named.medium_y },
            medium_drag: if has_medium { read::int(found, "medium_drag", 0, FRAC_ONE)? } else { named.medium_drag },
            supply_jitter: if has_jitter { read::int(found, "supply_jitter", 0, FRAC_ONE)? } else { named.supply_jitter },
            collision_radius: if has_collision { read::int(found, "collision_radius", 0, crate::field::PLANE_SPAN)? } else { named.collision_radius },
            collision_response: if has_collision { read::int(found, "collision_response", 0, FRAC_ONE)? } else { named.collision_response },
        };
        if held.supply_scale != named.supply_scale
            || held.supply_duty != named.supply_duty
            || held.dissipation_delta != named.dissipation_delta
            || held.noise_delta != named.noise_delta
            || held.route_capacity_scale != named.route_capacity_scale
            || held.leak_delta != named.leak_delta
            || held.medium_x != named.medium_x
            || held.medium_y != named.medium_y
            || held.medium_drag != named.medium_drag
            || held.supply_jitter != named.supply_jitter
            || held.collision_radius != named.collision_radius
            || held.collision_response != named.collision_response
        {
            return Err(Fault::field("regime"));
        }
        Ok(held)
    }

    fn write(&self, out: &mut String) {
        let mut object = Obj::new(out);
        object.int("collision_radius", self.collision_radius);
        object.int("collision_response", self.collision_response);
        object.int("dissipation_delta", self.dissipation_delta);
        object.text("id", &self.id);
        object.int("leak_delta", self.leak_delta);
        object.int("medium_drag", self.medium_drag);
        object.int("medium_x", self.medium_x);
        object.int("medium_y", self.medium_y);
        object.int("noise_delta", self.noise_delta);
        object.int("route_capacity_scale", self.route_capacity_scale);
        object.int("supply_duty", self.supply_duty);
        object.int("supply_jitter", self.supply_jitter);
        object.int("supply_scale", self.supply_scale);
        object.end();
    }

    fn scale(value: Fx, factor: Frac) -> Fx {
        value.saturating_mul(factor) / FRAC_ONE
    }

    pub fn medium_motion(&self) -> crate::field::MediumMotion {
        crate::field::MediumMotion {
            velocity: crate::fx::Vec2::new(self.medium_x, self.medium_y),
            drag: self.medium_drag,
            collision_radius: self.collision_radius,
            collision_response: self.collision_response,
        }
    }

    pub fn supply_jitter(&self) -> Frac {
        self.supply_jitter
    }

    pub fn apply(&self, field: &mut FieldState) {
        for layer in &mut field.layers {
            layer.drain = layer
                .drain
                .saturating_add(self.dissipation_delta)
                .min(crate::fx::STORED_BOUND - 1);
            layer.noise = (layer.noise.saturating_add(self.noise_delta)).min(FRAC_ONE);
        }
        for current in &mut field.currents {
            current.strength = Self::scale(current.strength, self.supply_scale);
            current.duty = self.supply_duty;
        }
        for route in &mut field.routes {
            route.capacity = Self::scale(route.capacity, self.route_capacity_scale);
        }
        field.physical_compartment.leak_per_exposed_contact_per_step = field
            .physical_compartment
            .leak_per_exposed_contact_per_step
            .saturating_add(self.leak_delta)
            .min(crate::field::LEAK_FRAC_CAP);
    }
}

/// One immutable component declaration from authored chapter content.
///
/// Runtime placement, charge, capacity, upkeep, and open state remain on the
/// embodied [`FieldState`]. The specification retains only identity and kind,
/// which are the local-rule selectors a transition may consult but never edit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComponentDeclaration {
    node: u32,
    kind: NodeKind,
}

impl ComponentDeclaration {
    fn read(value: &Json) -> Result<Self, Fault> {
        read::exact_keys(value, "components", &["kind", "node"])?;
        let kind = NodeKind::read(read::text(value, "kind")?)
            .ok_or_else(|| Fault::field("kind"))?;
        Ok(ComponentDeclaration {
            node: read::int(value, "node", 1, i64::from(u32::MAX))? as u32,
            kind,
        })
    }

    fn written(&self) -> String {
        let mut out = String::new();
        let mut object = Obj::new(&mut out);
        object.text("kind", self.kind.name());
        object.int("node", i64::from(self.node));
        object.end();
        out
    }
}

/// One authored route constraint. The live route, including its current
/// capacity, flow, and availability, remains in the Field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RouteConstraint {
    route: u32,
    tail: u32,
    head: u32,
}

impl RouteConstraint {
    fn read(value: &Json) -> Result<Self, Fault> {
        read::exact_keys(value, "routes", &["head", "route", "tail"])?;
        Ok(RouteConstraint {
            route: read::int(value, "route", 1, i64::from(u32::MAX))? as u32,
            tail: read::int(value, "tail", 1, i64::from(u32::MAX))? as u32,
            head: read::int(value, "head", 1, i64::from(u32::MAX))? as u32,
        })
    }

    fn written(&self) -> String {
        let mut out = String::new();
        let mut object = Obj::new(&mut out);
        object.int("head", i64::from(self.head));
        object.int("route", i64::from(self.route));
        object.int("tail", i64::from(self.tail));
        object.end();
        out
    }
}

/// The immutable local-rule and topology declaration for one chapter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChapterGeneratorSpec {
    chapter_index: u8,
    components: Vec<ComponentDeclaration>,
    routes: Vec<RouteConstraint>,
}

impl ChapterGeneratorSpec {
    fn from_content(chapter_index: u8, chapter: &crate::content::Chapter) -> Self {
        let mut components: Vec<ComponentDeclaration> = chapter
            .ports
            .iter()
            .map(|port| ComponentDeclaration { node: port.node, kind: port.kind })
            .chain(chapter.forms.iter().map(|form| ComponentDeclaration {
                node: form.node,
                kind: NodeKind::Form,
            }))
            .collect();
        components.sort_by_key(|component| component.node);

        let mut routes: Vec<RouteConstraint> = chapter
            .routes
            .iter()
            .map(|route| RouteConstraint {
                route: route.route,
                tail: route.tail,
                head: route.head,
            })
            .collect();
        routes.sort_by_key(|route| route.route);

        ChapterGeneratorSpec { chapter_index, components, routes }
    }

    fn from_field(chapter_index: u8, field: &FieldState) -> Self {
        let mut components: Vec<ComponentDeclaration> = field
            .ports
            .iter()
            .map(|component| ComponentDeclaration {
                node: component.node,
                kind: component.kind,
            })
            .collect();
        components.sort_by_key(|component| component.node);
        let mut routes: Vec<RouteConstraint> = field
            .routes
            .iter()
            .map(|route| RouteConstraint {
                route: route.route,
                tail: route.tail,
                head: route.head,
            })
            .collect();
        routes.sort_by_key(|route| route.route);
        ChapterGeneratorSpec { chapter_index, components, routes }
    }

    fn read(value: &Json) -> Result<Self, Fault> {
        read::exact_keys(value, "chapters", &["chapter_index", "components", "routes"])?;
        let mut components = Vec::new();
        for entry in read::list(value, "components", NODES_PER_RUN)? {
            components.push(ComponentDeclaration::read(entry)?);
        }
        let component_ids: Vec<u32> = components.iter().map(|component| component.node).collect();
        if !read::ascending(&component_ids) {
            return Err(Fault::field("components"));
        }

        let mut routes = Vec::new();
        for entry in read::list(value, "routes", ROUTES_PER_RUN)? {
            routes.push(RouteConstraint::read(entry)?);
        }
        let route_ids: Vec<u32> = routes.iter().map(|route| route.route).collect();
        if !read::ascending(&route_ids) {
            return Err(Fault::field("routes"));
        }

        Ok(ChapterGeneratorSpec {
            chapter_index: read::int(value, "chapter_index", 0, i64::from(u8::MAX))? as u8,
            components,
            routes,
        })
    }

    fn coherent(&self) -> Result<(), Fault> {
        for route in &self.routes {
            if route.tail == route.head
                || !self.components.iter().any(|component| component.node == route.tail)
                || !self.components.iter().any(|component| component.node == route.head)
            {
                return Err(Fault::field("routes"));
            }
        }
        Ok(())
    }

    fn accepts(&self, field: &FieldState) -> bool {
        self.components.iter().all(|declared| {
            field
                .ports
                .iter()
                .any(|component| component.node == declared.node && component.kind == declared.kind)
        }) && self.routes.iter().all(|declared| {
            field.routes.iter().find(|route| route.route == declared.route).is_none_or(|route| {
                route.tail == declared.tail && route.head == declared.head
            })
        })
    }

    fn establishes(&self, field: &FieldState) -> bool {
        self.accepts(field)
            && self.routes.iter().all(|declared| {
                field.routes.iter().any(|route| {
                    route.route == declared.route
                        && route.tail == declared.tail
                        && route.head == declared.head
                })
            })
    }

    fn written(&self) -> String {
        let mut out = String::new();
        let mut object = Obj::new(&mut out);
        object.int("chapter_index", i64::from(self.chapter_index));
        {
            let mut components = object.list("components");
            for component in &self.components {
                components.raw(&component.written());
            }
            components.end();
        }
        {
            let mut routes = object.list("routes");
            for route in &self.routes {
                routes.raw(&route.written());
            }
            routes.end();
        }
        object.end();
        out
    }
}

#[derive(Clone, Debug)]
pub struct GeneratorSpec {
    chapters: Vec<ChapterGeneratorSpec>,
    local_policy: FrozenLocalPolicy,
    route_defaults: Vec<RouteControlDefault>,
}

/// One chapter-addressed Route control frozen into a generator revision.
///
/// The matching [`RouteControlState`] in the Field is the embodied actuator:
/// policy actions may change it while a Commission runs. This record remains
/// unchanged and is projected back into the Field when that chapter is
/// established or its committed Design is installed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RouteControlDefault {
    chapter_index: u8,
    control: RouteControlState,
}

impl RouteControlDefault {
    fn read(value: &Json) -> Result<Self, Fault> {
        read::exact_keys(value, "route_defaults", &["chapter_index", "control"])?;
        Ok(Self {
            chapter_index: read::int(value, "chapter_index", 0, i64::from(u8::MAX))? as u8,
            control: RouteControlState::read(read::at(value, "control")?)?,
        })
    }

    fn written(&self) -> String {
        let mut out = String::new();
        let mut object = Obj::new(&mut out);
        object.int("chapter_index", i64::from(self.chapter_index));
        object.raw("control", &self.control.written());
        object.end();
        out
    }
}

impl GeneratorSpec {
    pub fn empty() -> Self {
        GeneratorSpec {
            chapters: Vec::new(),
            local_policy: FrozenLocalPolicy::empty(),
            route_defaults: Vec::new(),
        }
    }

    /// Freezes the authored organization and local-rule selectors.
    pub fn for_content(chapters: &[crate::content::Chapter]) -> Self {
        let chapters = chapters
            .iter()
            .enumerate()
            .map(|(index, chapter)| ChapterGeneratorSpec::from_content(index as u8, chapter))
            .collect();
        GeneratorSpec {
            chapters,
            local_policy: FrozenLocalPolicy::empty(),
            route_defaults: Vec::new(),
        }
    }

    /// Freezes a standalone experiment template as one organization-only
    /// chapter, independent of its embodied stocks and physical law values.
    pub fn for_field(field: &FieldState) -> Self {
        GeneratorSpec {
            chapters: vec![ChapterGeneratorSpec::from_field(0, field)],
            local_policy: FrozenLocalPolicy::empty(),
            route_defaults: Vec::new(),
        }
    }

    pub fn chapters(&self) -> &[ChapterGeneratorSpec] {
        &self.chapters
    }

    /** Exact Component addresses and kinds owned by one generator chapter. */
    pub fn declared_components(
        &self,
        chapter_index: u8,
    ) -> Option<Vec<(u32, crate::field::NodeKind)>> {
        self.chapters.get(usize::from(chapter_index)).map(|chapter| {
            chapter
                .components
                .iter()
                .map(|component| (component.node, component.kind))
                .collect()
        })
    }

    /** Exact Route ids and endpoints owned by one generator chapter. */
    pub fn declared_routes(&self, chapter_index: u8) -> Option<Vec<(u32, u32, u32)>> {
        self.chapters.get(usize::from(chapter_index)).map(|chapter| {
            chapter
                .routes
                .iter()
                .map(|route| (route.route, route.tail, route.head))
                .collect()
        })
    }

    pub fn local_policy(&self) -> &FrozenLocalPolicy {
        &self.local_policy
    }

    /// The committed opening controls for one chapter. An empty result is a
    /// legacy/implicit declaration: the authored Field's opening controls
    /// remain authoritative until the first complete Design commit.
    pub fn route_defaults(&self, chapter_index: u8) -> Vec<RouteControlState> {
        self.route_defaults
            .iter()
            .filter_map(|default| {
                (default.chapter_index == chapter_index).then_some(default.control)
            })
            .collect()
    }

    pub fn route_defaults_written(&self, chapter_index: u8) -> String {
        let mut out = String::new();
        let mut list = Arr::new(&mut out);
        for control in self.route_defaults(chapter_index) {
            list.raw(&control.written());
        }
        list.end();
        out
    }

    pub fn with_local_policy(&self, local_policy: FrozenLocalPolicy) -> Result<Self, Fault> {
        let mut specification = self.clone();
        specification.local_policy = local_policy;
        specification.coherent()?;
        Ok(specification)
    }

    /// Replaces the complete policy and one chapter's complete Route defaults
    /// as one generator revision.
    pub fn with_design(
        &self,
        chapter_index: u8,
        local_policy: FrozenLocalPolicy,
        route_controls: Vec<RouteControlState>,
    ) -> Result<Self, Fault> {
        let mut specification = self.clone();
        specification.local_policy = local_policy;
        specification
            .route_defaults
            .retain(|default| default.chapter_index != chapter_index);
        specification.route_defaults.extend(route_controls.into_iter().map(|control| {
            RouteControlDefault { chapter_index, control }
        }));
        specification
            .route_defaults
            .sort_by_key(|default| (default.chapter_index, default.control.route));
        specification.coherent()?;
        Ok(specification)
    }

    /// Re-freezes the organization produced by one Design commit while
    /// retaining every other contract chapter and the installed local policy.
    pub fn with_field(&self, chapter_index: u8, field: &FieldState) -> Result<Self, Fault> {
        let mut specification = self.clone();
        let place = usize::from(chapter_index);
        if place == specification.chapters.len() {
            specification
                .chapters
                .push(ChapterGeneratorSpec::from_field(chapter_index, field));
        } else if let Some(chapter) = specification.chapters.get_mut(place) {
            *chapter = ChapterGeneratorSpec::from_field(chapter_index, field);
        } else {
            return Err(Fault::field("chapter_index"));
        }
        specification
            .route_defaults
            .retain(|default| default.chapter_index != chapter_index);
        specification.route_defaults.extend(field.route_controls.iter().copied().map(
            |control| RouteControlDefault { chapter_index, control },
        ));
        specification
            .route_defaults
            .sort_by_key(|default| (default.chapter_index, default.control.route));
        specification.coherent()?;
        Ok(specification)
    }

    /// Holds an embodied chapter to the identities and local-rule selectors
    /// frozen in this specification. Empty legacy declarations remain
    /// migration-compatible; current specifications declare every chapter.
    pub fn accepts_field(&self, chapter_index: u8, field: &FieldState) -> bool {
        if self.chapters.is_empty() {
            return true;
        }
        self.chapters
            .get(usize::from(chapter_index))
            .is_some_and(|chapter| chapter.accepts(field))
    }

    pub fn establishes_field(&self, chapter_index: u8, field: &FieldState) -> bool {
        if self.chapters.is_empty() {
            return true;
        }
        self.chapters
            .get(usize::from(chapter_index))
            .is_some_and(|chapter| chapter.establishes(field))
    }

    pub fn specification_hash(&self) -> String {
        crate::json::hex_bytes(&sha256::digest(self.definition_written().as_bytes()))
    }

    fn coherent(&self) -> Result<(), Fault> {
        self.local_policy.coherent()?;
        let indices: Vec<u8> = self.chapters.iter().map(|chapter| chapter.chapter_index).collect();
        if !indices.iter().enumerate().all(|(place, index)| usize::from(*index) == place) {
            return Err(Fault::field("chapters"));
        }
        for chapter in &self.chapters {
            chapter.coherent()?;
        }
        if !self.route_defaults.windows(2).all(|pair| {
            (pair[0].chapter_index, pair[0].control.route)
                < (pair[1].chapter_index, pair[1].control.route)
        }) {
            return Err(Fault::field("route_defaults"));
        }
        for chapter in &self.chapters {
            let defaults: Vec<&RouteControlDefault> = self
                .route_defaults
                .iter()
                .filter(|default| default.chapter_index == chapter.chapter_index)
                .collect();
            if defaults.is_empty() {
                continue;
            }
            let default_routes: Vec<u32> =
                defaults.iter().map(|default| default.control.route).collect();
            let declared_routes: Vec<u32> =
                chapter.routes.iter().map(|route| route.route).collect();
            if default_routes != declared_routes
                || defaults.iter().any(|default| {
                    chapter.routes.iter().find(|route| route.route == default.control.route)
                        .is_none_or(|route| route.tail != default.control.controller)
                })
            {
                return Err(Fault::field("route_defaults"));
            }
        }
        if self.route_defaults.iter().any(|default| {
            self.chapters
                .get(usize::from(default.chapter_index))
                .is_none_or(|chapter| chapter.chapter_index != default.chapter_index)
        }) {
            return Err(Fault::field("route_defaults"));
        }
        Ok(())
    }

    pub fn read(value: &Json, key: &str) -> Result<Self, Fault> {
        let found = read::map(value, key)?;
        let has_local_policy = found.get("local_policy").is_some();
        let has_route_defaults = found.get("route_defaults").is_some();
        read::exact_keys(
            found,
            key,
            if has_local_policy && has_route_defaults {
                &["chapters", "local_policy", "route_defaults", "specification_hash"]
            } else if has_local_policy {
                &["chapters", "local_policy", "specification_hash"]
            } else if has_route_defaults {
                &["chapters", "route_defaults", "specification_hash"]
            } else {
                &["chapters", "specification_hash"]
            },
        )?;
        let mut chapters = Vec::new();
        for entry in read::list(found, "chapters", usize::from(u8::MAX) + 1)? {
            chapters.push(ChapterGeneratorSpec::read(entry)?);
        }
        let mut route_defaults = Vec::new();
        if has_route_defaults {
            for entry in read::list(
                found,
                "route_defaults",
                ROUTES_PER_RUN * (usize::from(u8::MAX) + 1),
            )? {
                route_defaults.push(RouteControlDefault::read(entry)?);
            }
        }
        let spec = GeneratorSpec {
            chapters,
            local_policy: if has_local_policy {
                FrozenLocalPolicy::read(found, "local_policy")?
            } else {
                FrozenLocalPolicy::empty()
            },
            route_defaults,
        };
        spec.coherent()?;
        if has_local_policy
            && has_route_defaults
            && read::hex(found, "specification_hash", 64)? != spec.specification_hash()
        {
            return Err(Fault::field("specification_hash"));
        }
        Ok(spec)
    }

    fn write_shape(&self, out: &mut String, include_hash: bool) {
        let mut object = Obj::new(out);
        {
            let mut chapters = object.list("chapters");
            for chapter in &self.chapters {
                chapters.raw(&chapter.written());
            }
            chapters.end();
        }
        object.raw("local_policy", &self.local_policy.written());
        {
            let mut defaults = object.list("route_defaults");
            for default in &self.route_defaults {
                defaults.raw(&default.written());
            }
            defaults.end();
        }
        if include_hash {
            object.text("specification_hash", &self.specification_hash());
        }
        object.end();
    }

    fn definition_written(&self) -> String {
        let mut out = String::new();
        self.write_shape(&mut out, false);
        out
    }

    fn write(&self, out: &mut String) {
        self.write_shape(out, true);
    }

    /// Complete canonical GeneratorSpec bytes, including its specification
    /// hash, for immutable command previews and retained engineering records.
    pub fn written(&self) -> String {
        let mut out = String::new();
        self.write(&mut out);
        out
    }
}

/// The declared control source for one frozen scenario.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControlContract {
    Commissioning,
    RecordedOpenLoop,
    FrozenFeedback,
    HandsOff,
}

const CONTROL_CONTRACTS: [&str; 4] = [
    "commissioning",
    "recorded_open_loop",
    "frozen_feedback",
    "hands_off",
];

impl ControlContract {
    pub fn name(self) -> &'static str {
        CONTROL_CONTRACTS[match self {
            ControlContract::Commissioning => 0,
            ControlContract::RecordedOpenLoop => 1,
            ControlContract::FrozenFeedback => 2,
            ControlContract::HandsOff => 3,
        }]
    }

    pub fn named(name: &str) -> Result<Self, Fault> {
        CONTROL_CONTRACTS
            .iter()
            .position(|candidate| *candidate == name)
            .map(|place| match place {
                0 => ControlContract::Commissioning,
                1 => ControlContract::RecordedOpenLoop,
                2 => ControlContract::FrozenFeedback,
                _ => ControlContract::HandsOff,
            })
            .ok_or_else(|| Fault::field("control"))
    }

    fn read(value: &Json, key: &str) -> Result<Self, Fault> {
        Ok(match read::one_of(value, key, &CONTROL_CONTRACTS)? {
            0 => ControlContract::Commissioning,
            1 => ControlContract::RecordedOpenLoop,
            2 => ControlContract::FrozenFeedback,
            _ => ControlContract::HandsOff,
        })
    }
}

/// The product path one durable run belongs to.
///
/// This is carried explicitly because save version cannot distinguish a legacy
/// campaign record from an automation contract or an Open Field experiment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunKind {
    AutomationContract,
    OpenField,
    LegacyCampaign,
}

const RUN_KINDS: [&str; 3] = ["automation_contract", "open_field", "legacy_campaign"];

impl RunKind {
    pub fn name(self) -> &'static str {
        RUN_KINDS[match self {
            RunKind::AutomationContract => 0,
            RunKind::OpenField => 1,
            RunKind::LegacyCampaign => 2,
        }]
    }

    fn read(value: &Json, key: &str) -> Result<Self, Fault> {
        Ok(match read::one_of(value, key, &RUN_KINDS)? {
            0 => RunKind::AutomationContract,
            1 => RunKind::OpenField,
            _ => RunKind::LegacyCampaign,
        })
    }
}

pub const ASSEMBLY_OWNED_FIELDS: [&str; 8] = [
    "component_opening_state",
    "component_placement",
    "current_phase",
    "form_reserve",
    "interface_state",
    "material_placement_and_stock",
    "physical_compartment",
    "stored_charge",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AssemblyComponentDraft {
    pub node: u32,
    pub layer: u8,
    pub pos: crate::fx::Vec2,
    pub q: Fx,
    pub open: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AssemblyFormDraft {
    pub node: u32,
    pub reserve: Fx,
    pub junction_blanks: Option<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AssemblyMaterialDraft {
    pub material: u32,
    pub amount: u16,
    pub layer: u8,
    pub pos: crate::fx::Vec2,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AssemblyCurrentDraft {
    pub current: u16,
    pub active: bool,
    pub phase: u16,
}

/// The complete player-editable projection of assembly-owned opening state.
///
/// Lists are complete and address-sorted. Omitting an object therefore cannot
/// silently delete it, and generator-owned hardware/topology never enters this
/// draft at all.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssemblyDraft {
    pub components: Vec<AssemblyComponentDraft>,
    pub currents: Vec<AssemblyCurrentDraft>,
    pub forms: Vec<AssemblyFormDraft>,
    pub materials: Vec<AssemblyMaterialDraft>,
    pub physical_compartment: PhysicalCompartment,
}

impl AssemblyDraft {
    pub fn read(value: &Json, key: &str) -> Result<Self, Fault> {
        let found = read::map(value, key)?;
        read::exact_keys(
            found,
            key,
            &["components", "currents", "forms", "materials", "physical_compartment", "version"],
        )?;
        if read::int(found, "version", 1, 1)? != 1 {
            return Err(Fault::field(key));
        }

        let mut components = Vec::new();
        for entry in read::list(found, "components", NODES_PER_RUN)? {
            read::exact_keys(entry, "components", &["layer", "node", "open", "pos", "q"])?;
            components.push(AssemblyComponentDraft {
                node: read::int(entry, "node", 1, i64::from(u32::MAX))? as u32,
                layer: read::int(entry, "layer", 0, i64::from(u8::MAX))? as u8,
                pos: crate::fx::Vec2::read(entry, "pos")?,
                q: read::int(entry, "q", 0, crate::field::NODE_CHARGE_CAP)?,
                open: read::flag(entry, "open")?,
            });
        }
        if !read::ascending(&components.iter().map(|item| item.node).collect::<Vec<_>>()) {
            return Err(Fault::field("components"));
        }

        let mut currents = Vec::new();
        for entry in read::list(found, "currents", CURRENTS_PER_CHAPTER)? {
            read::exact_keys(entry, "currents", &["active", "current", "phase"])?;
            currents.push(AssemblyCurrentDraft {
                current: read::int(entry, "current", 0, i64::from(u16::MAX))? as u16,
                active: read::flag(entry, "active")?,
                phase: read::int(entry, "phase", 0, i64::from(u16::MAX))? as u16,
            });
        }
        if !read::ascending(&currents.iter().map(|item| item.current).collect::<Vec<_>>()) {
            return Err(Fault::field("currents"));
        }

        let mut forms = Vec::new();
        for entry in read::list(found, "forms", FORMS_PER_RUN)? {
            read::exact_keys(entry, "forms", &["junction_blanks", "node", "reserve"])?;
            forms.push(AssemblyFormDraft {
                node: read::int(entry, "node", 1, i64::from(u32::MAX))? as u32,
                reserve: read::int(entry, "reserve", 0, crate::field::NODE_CHARGE_CAP)?,
                junction_blanks: read::int_or_null(
                    entry,
                    "junction_blanks",
                    0,
                    i64::from(u8::MAX),
                )?
                .map(|value| value as u8),
            });
        }
        if !read::ascending(&forms.iter().map(|item| item.node).collect::<Vec<_>>()) {
            return Err(Fault::field("forms"));
        }

        let mut materials = Vec::new();
        for entry in read::list(found, "materials", MATERIALS_PER_RUN)? {
            read::exact_keys(entry, "materials", &["amount", "layer", "material", "pos"])?;
            materials.push(AssemblyMaterialDraft {
                material: read::int(entry, "material", 1, i64::from(u32::MAX))? as u32,
                amount: read::int(entry, "amount", 0, i64::from(u16::MAX))? as u16,
                layer: read::int(entry, "layer", 0, i64::from(u8::MAX))? as u8,
                pos: crate::fx::Vec2::read(entry, "pos")?,
            });
        }
        if !read::ascending(&materials.iter().map(|item| item.material).collect::<Vec<_>>()) {
            return Err(Fault::field("materials"));
        }

        Ok(Self {
            components,
            currents,
            forms,
            materials,
            physical_compartment: PhysicalCompartment::read(found, "physical_compartment")?,
        })
    }

    pub fn written(&self) -> String {
        let mut out = String::new();
        let mut object = Obj::new(&mut out);
        {
            let mut components = object.list("components");
            for component in &self.components {
                let mut entry = components.object();
                entry.int("layer", i64::from(component.layer));
                entry.int("node", i64::from(component.node));
                entry.bool("open", component.open);
                entry.raw("pos", &component.pos.written());
                entry.int("q", component.q);
                entry.end();
            }
            components.end();
        }
        {
            let mut currents = object.list("currents");
            for current in &self.currents {
                let mut entry = currents.object();
                entry.bool("active", current.active);
                entry.int("current", i64::from(current.current));
                entry.int("phase", i64::from(current.phase));
                entry.end();
            }
            currents.end();
        }
        {
            let mut forms = object.list("forms");
            for form in &self.forms {
                let mut entry = forms.object();
                entry.int_or_null("junction_blanks", form.junction_blanks.map(i64::from));
                entry.int("node", i64::from(form.node));
                entry.int("reserve", form.reserve);
                entry.end();
            }
            forms.end();
        }
        {
            let mut materials = object.list("materials");
            for material in &self.materials {
                let mut entry = materials.object();
                entry.int("amount", i64::from(material.amount));
                entry.int("layer", i64::from(material.layer));
                entry.int("material", i64::from(material.material));
                entry.raw("pos", &material.pos.written());
                entry.end();
            }
            materials.end();
        }
        let mut compartment = String::new();
        self.physical_compartment.write(&mut compartment);
        object.raw("physical_compartment", &compartment);
        object.int("version", 1);
        object.end();
        out
    }
}

/// The immutable embodied opening that a Commission restart reconstructs.
///
/// V2 records normalize away live-only runtime state and explicitly declare
/// their owned fields. Migrated V4 contract saves retain their former hash-only
/// address and are deliberately not qualification-ready because their original
/// opening bytes were never stored.
#[derive(Clone, Debug)]
pub struct AssemblyTemplate {
    field: Option<FieldState>,
    hash: String,
    version: u8,
}

impl AssemblyTemplate {
    fn normalize_opening(field: &FieldState) -> FieldState {
        let mut opening = field.clone();
        opening.step = 0;
        opening.assembly_ordinal = 0;
        opening.prev_assembly_step = None;
        opening.wheel_accum = 0;
        opening.depth_cooldown = 0;
        opening.route_runtime.clear();
        opening.route_clamps.clear();
        opening.leak_breach = None;
        opening.signals.clear();
        opening.next_signal_id = 1;
        opening.policy_runtime.clear();
        opening.supply_decoys.clear();
        opening.current_delays.clear();
        opening.route_scramble = None;
        opening.pending.clear();
        opening.boundaries.drawn.clear();
        for route in &mut opening.routes {
            route.flow = 0;
            route.formed_step = 0;
        }
        opening.route_controls = opening
            .routes
            .iter()
            .map(|route| RouteControlState::opening(route.route, route.tail, route.capacity))
            .collect();
        for form in &mut opening.forms {
            form.controlled = false;
            form.focus = false;
            form.pulse_charge = 0;
            form.vel = crate::fx::Vec2::default();
            if let Some(port) = opening.ports.iter().find(|port| port.node == form.node) {
                form.layer = port.layer;
                form.pos = port.pos;
                form.charge = port.q;
            }
        }
        for material in &mut opening.materials {
            material.claimed = false;
        }
        opening
    }

    pub fn from_field(field: &FieldState) -> Self {
        let field = Self::normalize_opening(field);
        let hash = field.embodied_hash();
        AssemblyTemplate { field: Some(field), hash, version: 2 }
    }

    fn migrated(hash: String) -> Self {
        AssemblyTemplate { field: None, hash, version: 1 }
    }

    pub fn hash(&self) -> &str {
        &self.hash
    }

    pub fn field(&self) -> Option<&FieldState> {
        self.field.as_ref()
    }

    pub fn is_exact(&self) -> bool {
        self.field.is_some()
    }

    pub fn version(&self) -> u8 {
        self.version
    }

    pub fn draft(&self) -> Option<AssemblyDraft> {
        let field = self.field.as_ref()?;
        let mut components = field
            .ports
            .iter()
            .map(|port| AssemblyComponentDraft {
                node: port.node,
                layer: port.layer,
                pos: port.pos,
                q: port.q,
                open: port.open,
            })
            .collect::<Vec<_>>();
        components.sort_by_key(|item| item.node);
        let mut currents = field
            .currents
            .iter()
            .map(|current| AssemblyCurrentDraft {
                current: current.id,
                active: current.active,
                phase: current.phase,
            })
            .collect::<Vec<_>>();
        currents.sort_by_key(|item| item.current);
        let mut forms = field
            .forms
            .iter()
            .map(|form| AssemblyFormDraft {
                node: form.node,
                reserve: form.reserve,
                junction_blanks: form.junction.as_ref().map(|junction| junction.blanks),
            })
            .collect::<Vec<_>>();
        forms.sort_by_key(|item| item.node);
        let mut materials = field
            .materials
            .iter()
            .map(|material| AssemblyMaterialDraft {
                material: material.material,
                amount: material.amount,
                layer: material.layer,
                pos: material.pos,
            })
            .collect::<Vec<_>>();
        materials.sort_by_key(|item| item.material);
        Some(AssemblyDraft {
            components,
            currents,
            forms,
            materials,
            physical_compartment: field.physical_compartment.clone(),
        })
    }

    pub fn adapted(&self, draft: &AssemblyDraft) -> Result<Self, Fault> {
        let source = self.field.as_ref().ok_or_else(|| Fault::field("assembly_template"))?;
        let mut field = source.clone();
        let component_ids = field.ports.iter().map(|port| port.node).collect::<Vec<_>>();
        let draft_component_ids = draft.components.iter().map(|item| item.node).collect::<Vec<_>>();
        let mut source_form_ids = field.forms.iter().map(|form| form.node).collect::<Vec<_>>();
        source_form_ids.sort_unstable();
        let draft_form_ids = draft.forms.iter().map(|item| item.node).collect::<Vec<_>>();
        let material_ids = field.materials.iter().map(|item| item.material).collect::<Vec<_>>();
        let draft_material_ids = draft.materials.iter().map(|item| item.material).collect::<Vec<_>>();
        let current_ids = field.currents.iter().map(|item| item.id).collect::<Vec<_>>();
        let draft_current_ids = draft.currents.iter().map(|item| item.current).collect::<Vec<_>>();
        if component_ids != draft_component_ids
            || source_form_ids != draft_form_ids
            || material_ids != draft_material_ids
            || current_ids != draft_current_ids
        {
            return Err(Fault::field("assembly_draft"));
        }

        for component in &draft.components {
            let port = field
                .ports
                .iter_mut()
                .find(|port| port.node == component.node)
                .ok_or_else(|| Fault::field("components"))?;
            port.layer = component.layer;
            port.pos = component.pos;
            port.q = component.q;
            port.open = component.open;
            if let Some(form) = field.forms.iter_mut().find(|form| form.node == component.node) {
                form.layer = component.layer;
                form.pos = component.pos;
                form.charge = component.q;
            }
        }
        for form_draft in &draft.forms {
            let form = field
                .forms
                .iter_mut()
                .find(|form| form.node == form_draft.node)
                .ok_or_else(|| Fault::field("forms"))?;
            form.reserve = form_draft.reserve;
            match (&mut form.junction, form_draft.junction_blanks) {
                (Some(junction), Some(blanks)) => junction.blanks = blanks,
                (None, None) => {}
                _ => return Err(Fault::field("junction_blanks")),
            }
        }
        for material_draft in &draft.materials {
            let material = field
                .materials
                .iter_mut()
                .find(|material| material.material == material_draft.material)
                .ok_or_else(|| Fault::field("materials"))?;
            material.amount = material_draft.amount;
            material.layer = material_draft.layer;
            material.pos = material_draft.pos;
            material.claimed = false;
        }
        for current_draft in &draft.currents {
            let current = field
                .currents
                .iter_mut()
                .find(|current| current.id == current_draft.current)
                .ok_or_else(|| Fault::field("currents"))?;
            if current.period == 0 && current_draft.phase != 0
                || current.period > 0 && current_draft.phase >= current.period
            {
                return Err(Fault::field("phase"));
            }
            current.active = current_draft.active;
            current.phase = current_draft.phase;
        }
        field.physical_compartment = draft.physical_compartment.clone();
        field = Self::normalize_opening(&field);
        crate::field::validate(&field)?;
        crate::field::establishable(&field)?;
        Ok(Self::from_field(&field))
    }

    pub fn read(value: &Json, key: &str) -> Result<Option<Self>, Fault> {
        match read::at(value, key)? {
            Json::Null => Ok(None),
            Json::Map(_) => {
                let found = read::map(value, key)?;
                let version = read::int(found, "version", 1, 2)? as u8;
                read::exact_keys(
                    found,
                    key,
                    if version == 1 {
                        &["field", "hash", "version"]
                    } else {
                        &["field", "hash", "owned_fields", "version"]
                    },
                )?;
                if version == 2 {
                    let owned = read::list(found, "owned_fields", ASSEMBLY_OWNED_FIELDS.len())?;
                    if owned.len() != ASSEMBLY_OWNED_FIELDS.len()
                        || owned.iter().zip(ASSEMBLY_OWNED_FIELDS).any(|(found, expected)| {
                            !matches!(found, Json::Text(value) if value == expected)
                        })
                    {
                        return Err(Fault::field("owned_fields"));
                    }
                }
                if version != 1 && version != 2 {
                    return Err(Fault::field("assembly_template"));
                }
                let field = match read::at(found, "field")? {
                    Json::Null => None,
                    _ => Some(FieldState::read(found, "field")?),
                };
                let hash = read::hex(found, "hash", 64)?.to_string();
                if field.as_ref().is_some_and(|held| held.embodied_hash() != hash) {
                    return Err(Fault::field("assembly_template"));
                }
                Ok(Some(AssemblyTemplate { field, hash, version }))
            }
            _ => Err(Fault::field("assembly_template")),
        }
    }

    pub fn written(&self) -> String {
        let mut out = String::new();
        let mut object = Obj::new(&mut out);
        match &self.field {
            Some(field) => object.raw("field", &field.written()),
            None => object.null("field"),
        };
        object.text("hash", &self.hash);
        if self.version >= 2 {
            let mut fields = object.list("owned_fields");
            for field in ASSEMBLY_OWNED_FIELDS {
                fields.text(field);
            }
            fields.end();
        }
        object.int("version", i64::from(self.version));
        object.end();
        out
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttemptSource {
    Opened,
    Migrated,
}

impl AttemptSource {
    fn name(self) -> &'static str {
        match self {
            AttemptSource::Opened => "opened",
            AttemptSource::Migrated => "migrated",
        }
    }

    fn read(value: &Json, key: &str) -> Result<Self, Fault> {
        Ok(match read::one_of(value, key, &["opened", "migrated"])? {
            0 => AttemptSource::Opened,
            _ => AttemptSource::Migrated,
        })
    }
}

/// The immutable root of one contract attempt.
#[derive(Clone, Debug)]
pub struct AttemptRecord {
    attempt_id: String,
    contract_id: String,
    content_hash: String,
    opening_generator_hash: String,
    opening_assembly_hash: String,
    source: AttemptSource,
}

impl AttemptRecord {
    pub fn new(
        attempt_id: String,
        contract_id: String,
        content_hash: String,
        opening_generator_hash: String,
        opening_assembly_hash: String,
        source: AttemptSource,
    ) -> Result<Self, Fault> {
        if attempt_id.len() != 16 || !attempt_id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(Fault::field("attempt_id"));
        }
        if contract_id.is_empty()
            || !contract_id.starts_with(|first: char| first.is_ascii_lowercase())
            || !contract_id
                .chars()
                .all(|held| held.is_ascii_lowercase() || held.is_ascii_digit() || held == '_')
        {
            return Err(Fault::field("contract_id"));
        }
        for (field, value) in [
            ("content_hash", &content_hash),
            ("opening_generator_hash", &opening_generator_hash),
            ("opening_assembly_hash", &opening_assembly_hash),
        ] {
            if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                return Err(Fault::field(field));
            }
        }
        Ok(AttemptRecord {
            attempt_id,
            contract_id,
            content_hash,
            opening_generator_hash,
            opening_assembly_hash,
            source,
        })
    }

    pub fn attempt_id(&self) -> &str {
        &self.attempt_id
    }

    pub fn source(&self) -> AttemptSource {
        self.source
    }

    fn read(value: &Json, key: &str) -> Result<Option<Self>, Fault> {
        match read::at(value, key)? {
            Json::Null => Ok(None),
            Json::Map(_) => {
                let found = read::map(value, key)?;
                read::exact_keys(
                    found,
                    key,
                    &[
                        "attempt_id",
                        "content_hash",
                        "contract_id",
                        "opening_assembly_hash",
                        "opening_generator_hash",
                        "source",
                        "version",
                    ],
                )?;
                if read::int(found, "version", 1, 1)? != 1 {
                    return Err(Fault::field("attempt_record"));
                }
                Self::new(
                    read::text(found, "attempt_id")?.to_string(),
                    read::text(found, "contract_id")?.to_string(),
                    read::hex(found, "content_hash", 64)?.to_string(),
                    read::hex(found, "opening_generator_hash", 64)?.to_string(),
                    read::hex(found, "opening_assembly_hash", 64)?.to_string(),
                    AttemptSource::read(found, "source")?,
                )
                .map(Some)
            }
            _ => Err(Fault::field("attempt_record")),
        }
    }

    pub fn written(&self) -> String {
        let mut out = String::new();
        let mut object = Obj::new(&mut out);
        object.text("attempt_id", &self.attempt_id);
        object.text("content_hash", &self.content_hash);
        object.text("contract_id", &self.contract_id);
        object.text("opening_assembly_hash", &self.opening_assembly_hash);
        object.text("opening_generator_hash", &self.opening_generator_hash);
        object.text("source", self.source.name());
        object.int("version", 1);
        object.end();
        out
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BranchOperation {
    Opening,
    DesignCommit,
    AssemblyCommit,
    Restart,
    RestartAssembly,
    RevertGenerator,
    FullContractReset,
    CloneBlueprint,
    Rebranch,
    Resume,
    Migrated,
}

const BRANCH_OPERATIONS: [&str; 11] = [
    "opening",
    "design_commit",
    "assembly_commit",
    "restart",
    "restart_assembly",
    "revert_generator",
    "full_contract_reset",
    "clone_blueprint",
    "rebranch",
    "resume",
    "migrated",
];

impl BranchOperation {
    pub fn name(self) -> &'static str {
        BRANCH_OPERATIONS[match self {
            BranchOperation::Opening => 0,
            BranchOperation::DesignCommit => 1,
            BranchOperation::AssemblyCommit => 2,
            BranchOperation::Restart => 3,
            BranchOperation::RestartAssembly => 4,
            BranchOperation::RevertGenerator => 5,
            BranchOperation::FullContractReset => 6,
            BranchOperation::CloneBlueprint => 7,
            BranchOperation::Rebranch => 8,
            BranchOperation::Resume => 9,
            BranchOperation::Migrated => 10,
        }]
    }

    fn read(value: &Json, key: &str) -> Result<Self, Fault> {
        Ok(match read::one_of(value, key, &BRANCH_OPERATIONS)? {
            0 => BranchOperation::Opening,
            1 => BranchOperation::DesignCommit,
            2 => BranchOperation::AssemblyCommit,
            3 => BranchOperation::Restart,
            4 => BranchOperation::RestartAssembly,
            5 => BranchOperation::RevertGenerator,
            6 => BranchOperation::FullContractReset,
            7 => BranchOperation::CloneBlueprint,
            8 => BranchOperation::Rebranch,
            9 => BranchOperation::Resume,
            _ => BranchOperation::Migrated,
        })
    }
}

/// One immutable descendant in a contract attempt's explicit lineage.
#[derive(Clone, Debug)]
pub struct AttemptBranchRecord {
    attempt_id: String,
    branch_id: String,
    parent_branch_id: Option<String>,
    operation: BranchOperation,
    generator_hash: String,
    assembly_hash: String,
    branch_nonce: u32,
    transition_receipt: Option<crate::engineering::EngineeringTransitionReceipt>,
}

impl AttemptBranchRecord {
    pub fn new(
        attempt_id: String,
        parent_branch_id: Option<String>,
        operation: BranchOperation,
        generator_hash: String,
        assembly_hash: String,
        branch_nonce: u32,
    ) -> Result<Self, Fault> {
        if attempt_id.len() != 16 || !attempt_id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(Fault::field("attempt_id"));
        }
        if parent_branch_id.as_ref().is_some_and(|id| {
            id.len() != 64 || !id.bytes().all(|byte| byte.is_ascii_hexdigit())
        }) {
            return Err(Fault::field("parent_branch_id"));
        }
        for (field, value) in [
            ("generator_hash", &generator_hash),
            ("assembly_hash", &assembly_hash),
        ] {
            if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                return Err(Fault::field(field));
            }
        }
        let mut branch = AttemptBranchRecord {
            attempt_id,
            branch_id: String::new(),
            parent_branch_id,
            operation,
            generator_hash,
            assembly_hash,
            branch_nonce,
            transition_receipt: None,
        };
        branch.branch_id = crate::json::hex_bytes(&sha256::digest(
            branch.definition_written().as_bytes(),
        ));
        Ok(branch)
    }

    pub fn branch_id(&self) -> &str {
        &self.branch_id
    }

    pub fn parent_branch_id(&self) -> Option<&str> {
        self.parent_branch_id.as_deref()
    }

    pub fn operation(&self) -> BranchOperation {
        self.operation
    }

    pub fn transition_receipt(
        &self,
    ) -> Option<&crate::engineering::EngineeringTransitionReceipt> {
        self.transition_receipt.as_ref()
    }

    pub fn attach_transition_receipt(
        &mut self,
        receipt: crate::engineering::EngineeringTransitionReceipt,
    ) -> Result<(), Fault> {
        if !receipt.coherent_child(
            &self.attempt_id,
            &self.branch_id,
            &self.generator_hash,
            &self.assembly_hash,
        ) {
            return Err(Fault::field("transition_receipt"));
        }
        if self.transition_receipt.as_ref().is_some_and(|held| held != &receipt) {
            return Err(Fault::field("transition_receipt"));
        }
        self.transition_receipt = Some(receipt);
        Ok(())
    }

    fn read(value: &Json, key: &str) -> Result<Option<Self>, Fault> {
        match read::at(value, key)? {
            Json::Null => Ok(None),
            Json::Map(_) => {
                let found = read::map(value, key)?;
                let version = read::int(found, "version", 1, 2)?;
                let keys: &[&str] = if version == 1 {
                    &[
                        "assembly_hash",
                        "attempt_id",
                        "branch_id",
                        "branch_nonce",
                        "generator_hash",
                        "operation",
                        "parent_branch_id",
                        "version",
                    ]
                } else {
                    &[
                        "assembly_hash",
                        "attempt_id",
                        "branch_id",
                        "branch_nonce",
                        "generator_hash",
                        "operation",
                        "parent_branch_id",
                        "transition_receipt",
                        "version",
                    ]
                };
                read::exact_keys(found, key, keys)?;
                if !matches!(version, 1 | 2) {
                    return Err(Fault::field("attempt_branch"));
                }
                let parent_branch_id = match read::at(found, "parent_branch_id")? {
                    Json::Null => None,
                    Json::Text(_) => Some(read::hex(found, "parent_branch_id", 64)?.to_string()),
                    _ => return Err(Fault::field("parent_branch_id")),
                };
                let mut branch = Self::new(
                    read::text(found, "attempt_id")?.to_string(),
                    parent_branch_id,
                    BranchOperation::read(found, "operation")?,
                    read::hex(found, "generator_hash", 64)?.to_string(),
                    read::hex(found, "assembly_hash", 64)?.to_string(),
                    read::int(found, "branch_nonce", 0, i64::from(u32::MAX))? as u32,
                )?;
                if read::hex(found, "branch_id", 64)? != branch.branch_id {
                    return Err(Fault::field("branch_id"));
                }
                if version == 2 {
                    match read::at(found, "transition_receipt")? {
                        Json::Null => {}
                        Json::Map(_) => branch.attach_transition_receipt(
                            crate::engineering::EngineeringTransitionReceipt::read(
                                found,
                                "transition_receipt",
                            )?,
                        )?,
                        _ => return Err(Fault::field("transition_receipt")),
                    }
                }
                Ok(Some(branch))
            }
            _ => Err(Fault::field("attempt_branch")),
        }
    }

    fn write_identity_shape(&self, include_id: bool) -> String {
        let mut out = String::new();
        let mut object = Obj::new(&mut out);
        object.text("assembly_hash", &self.assembly_hash);
        object.text("attempt_id", &self.attempt_id);
        if include_id {
            object.text("branch_id", &self.branch_id);
        }
        object.int("branch_nonce", i64::from(self.branch_nonce));
        object.text("generator_hash", &self.generator_hash);
        object.text("operation", self.operation.name());
        match &self.parent_branch_id {
            Some(parent) => object.text("parent_branch_id", parent),
            None => object.null("parent_branch_id"),
        };
        object.int("version", 1);
        object.end();
        out
    }

    fn definition_written(&self) -> String {
        self.write_identity_shape(false)
    }

    pub fn written(&self) -> String {
        let mut out = String::new();
        let mut object = Obj::new(&mut out);
        object.text("assembly_hash", &self.assembly_hash);
        object.text("attempt_id", &self.attempt_id);
        object.text("branch_id", &self.branch_id);
        object.int("branch_nonce", i64::from(self.branch_nonce));
        object.text("generator_hash", &self.generator_hash);
        object.text("operation", self.operation.name());
        match &self.parent_branch_id {
            Some(parent) => object.text("parent_branch_id", parent),
            None => object.null("parent_branch_id"),
        };
        match &self.transition_receipt {
            Some(receipt) => object.raw("transition_receipt", &receipt.written()),
            None => object.null("transition_receipt"),
        };
        object.int("version", 2);
        object.end();
        out
    }
}

/// One immutable qualification request frozen from the complete Q-01 input.
///
/// The nested input is the authoritative E-06 bundle. The request identity is
/// the SHA-256 address of that bundle under this request schema, so storage
/// retries converge on the same bytes and a later live edit cannot be smuggled
/// into an existing request id.
#[derive(Clone, Debug)]
pub struct QualificationRequest {
    input: Json,
    request_id: String,
}

impl QualificationRequest {
    pub fn from_input(input: Json) -> Result<Self, Fault> {
        let map = match &input {
            Json::Map(_) => &input,
            _ => return Err(Fault::field("qualification_input")),
        };
        read::exact_keys(
            map,
            "qualification_input",
            &[
                "assembly_template",
                "assembly_template_exact",
                "assembly_template_hash",
                "attempt_branch",
                "attempt_id",
                "attempt_record",
                "branch_id",
                "branch_nonce",
                "branch_operation",
                "build",
                "content_hash",
                "contract_id",
                "criterion_vector",
                "criterion_vector_hash",
                "generator_spec",
                "generator_spec_hash",
                "grade_axes",
                "missing_inputs",
                "parent_branch_id",
                "procedure",
                "prospective_receipt",
                "protocol_version",
                "regime",
                "run_kind",
                "scenario_hash",
                "schema_version",
            ],
        )?;
        if read::int(map, "schema_version", 1, 1)? != 1
            || !read::list(map, "missing_inputs", 8)?.is_empty()
            || !read::flag(map, "assembly_template_exact")?
            || read::one_of(map, "run_kind", &["automation_contract"])? != 0
            || read::int(
                map,
                "protocol_version",
                i64::from(crate::protocol::PROTOCOL_VERSION),
                i64::from(crate::protocol::PROTOCOL_VERSION),
            )? != i64::from(crate::protocol::PROTOCOL_VERSION)
        {
            return Err(Fault::field("qualification_input"));
        }
        let assembly = AssemblyTemplate::read(map, "assembly_template")?
            .filter(|template| template.is_exact())
            .ok_or_else(|| Fault::field("qualification_input"))?;
        let generator = GeneratorSpec::read(map, "generator_spec")?;
        let attempt = AttemptRecord::read(map, "attempt_record")?
            .ok_or_else(|| Fault::field("qualification_input"))?;
        let branch = AttemptBranchRecord::read(map, "attempt_branch")?
            .ok_or_else(|| Fault::field("qualification_input"))?;
        if read::hex(map, "assembly_template_hash", 64)? != assembly.hash()
            || read::hex(map, "generator_spec_hash", 64)? != generator.specification_hash()
            || read::hex(map, "attempt_id", 16)? != attempt.attempt_id()
            || read::hex(map, "branch_id", 64)? != branch.branch_id()
            || read::int(map, "branch_nonce", 0, i64::from(u32::MAX))?
                != i64::from(branch.branch_nonce)
            || read::text(map, "branch_operation")? != branch.operation.name()
            || attempt.contract_id != read::text(map, "contract_id")?
            || attempt.content_hash != read::hex(map, "content_hash", 64)?
            || branch.attempt_id != attempt.attempt_id
            || branch.generator_hash != generator.specification_hash()
            || branch.assembly_hash != assembly.hash
        {
            return Err(Fault::field("qualification_input"));
        }
        match (&branch.parent_branch_id, read::at(map, "parent_branch_id")?) {
            (Some(expected), Json::Text(found)) if expected == found => {}
            (None, Json::Null) => {}
            _ => return Err(Fault::field("qualification_input")),
        }
        let canonical_hash = |value: &Json| -> Result<String, Fault> {
            let mut written = String::new();
            crate::json::write_value(&mut written, value)
                .map_err(|_| Fault::field("qualification_input"))?;
            Ok(crate::json::hex_bytes(&sha256::digest(written.as_bytes())))
        };
        if canonical_hash(read::at(map, "criterion_vector")?)?
            != read::hex(map, "criterion_vector_hash", 64)?
        {
            return Err(Fault::field("qualification_input"));
        }
        let procedure = read::map(map, "procedure")?;
        if canonical_hash(read::at(procedure, "schedule")?)?
            != read::hex(procedure, "schedule_hash", 64)?
        {
            return Err(Fault::field("qualification_input"));
        }
        let mut request = QualificationRequest {
            input,
            request_id: String::new(),
        };
        request.request_id = crate::json::hex_bytes(&sha256::digest(
            request.definition_written().as_bytes(),
        ));
        Ok(request)
    }

    fn read(value: &Json, key: &str) -> Result<Option<Self>, Fault> {
        match read::at(value, key)? {
            Json::Null => Ok(None),
            Json::Map(_) => {
                let found = read::map(value, key)?;
                read::exact_keys(found, key, &["input", "request_id", "version"])?;
                if read::int(found, "version", 1, 1)? != 1 {
                    return Err(Fault::field("qualification_request"));
                }
                let request = Self::from_input(read::at(found, "input")?.clone())?;
                if read::hex(found, "request_id", 64)? != request.request_id {
                    return Err(Fault::field("qualification_request"));
                }
                Ok(Some(request))
            }
            _ => Err(Fault::field("qualification_request")),
        }
    }

    fn write_shape(&self, include_id: bool) -> String {
        let mut input = String::new();
        crate::json::write_value(&mut input, &self.input)
            .expect("stored qualification input is canonical JSON");
        let mut out = String::new();
        let mut object = Obj::new(&mut out);
        object.raw("input", &input);
        if include_id {
            object.text("request_id", &self.request_id);
        }
        object.int("version", 1);
        object.end();
        out
    }

    fn definition_written(&self) -> String {
        self.write_shape(false)
    }

    pub fn input(&self) -> &Json {
        &self.input
    }

    pub fn input_written(&self) -> String {
        let mut out = String::new();
        crate::json::write_value(&mut out, &self.input)
            .expect("stored qualification input is canonical JSON");
        out
    }

    pub fn request_id(&self) -> &str {
        &self.request_id
    }

    pub fn written(&self) -> String {
        self.write_shape(true)
    }
}

/// Everything frozen about one trial that is not embodied Field state.
///
/// The generator is nested deliberately: its hash identifies organization and
/// local policy, while the scenario hash also accounts for physical regime,
/// exogenous pressure inputs, control source, and authored content identity.
#[derive(Clone, Debug)]
pub struct ScenarioSpec {
    assembly_template: Option<AssemblyTemplate>,
    content_hash: String,
    contract_id: Option<String>,
    control: ControlContract,
    criteria: Vec<Option<crate::criterion::FunctionCriterionSpec>>,
    generator: GeneratorSpec,
    pressure_schedule: crate::pressure::Schedule,
    regime: RegimeSpec,
}

impl ScenarioSpec {
    pub fn commissioning(
        content_hash: String,
        pressure_schedule: crate::pressure::Schedule,
        regime: RegimeSpec,
        generator: GeneratorSpec,
        criteria: Vec<Option<crate::criterion::FunctionCriterionSpec>>,
    ) -> Self {
        ScenarioSpec {
            assembly_template: None,
            content_hash,
            contract_id: None,
            control: ControlContract::Commissioning,
            criteria,
            generator,
            pressure_schedule,
            regime,
        }
    }

    pub fn for_content(
        content_hash: String,
        pressure_schedule: crate::pressure::Schedule,
        regime: RegimeSpec,
        chapters: &[crate::content::Chapter],
        criteria: Vec<Option<crate::criterion::FunctionCriterionSpec>>,
    ) -> Self {
        Self::commissioning(
            content_hash,
            pressure_schedule,
            regime,
            GeneratorSpec::for_content(chapters),
            criteria,
        )
    }

    pub fn for_contract(
        content_hash: String,
        contract_id: String,
        assembly_template: AssemblyTemplate,
        pressure_schedule: crate::pressure::Schedule,
        regime: RegimeSpec,
        generator: GeneratorSpec,
        criterion: Option<crate::criterion::FunctionCriterionSpec>,
    ) -> Result<Self, Fault> {
        if contract_id.is_empty()
            || !contract_id.starts_with(|first: char| first.is_ascii_lowercase())
            || !contract_id
                .chars()
                .all(|held| held.is_ascii_lowercase() || held.is_ascii_digit() || held == '_')
        {
            return Err(Fault::field("contract_id"));
        }
        let scenario = ScenarioSpec {
            assembly_template: Some(assembly_template),
            content_hash,
            contract_id: Some(contract_id),
            control: ControlContract::Commissioning,
            criteria: vec![criterion],
            generator,
            pressure_schedule,
            regime,
        };
        scenario.coherent()?;
        Ok(scenario)
    }

    pub fn legacy(content_hash: String) -> Self {
        Self::commissioning(
            content_hash,
            crate::pressure::Schedule::default(),
            RegimeSpec::named("open_field").expect("the default regime is closed"),
            GeneratorSpec::empty(),
            Vec::new(),
        )
    }

    pub fn content_hash(&self) -> &str {
        &self.content_hash
    }

    pub fn assembly_template_hash(&self) -> Option<&str> {
        self.assembly_template.as_ref().map(AssemblyTemplate::hash)
    }

    pub fn assembly_template(&self) -> Option<&AssemblyTemplate> {
        self.assembly_template.as_ref()
    }

    pub fn contract_id(&self) -> Option<&str> {
        self.contract_id.as_deref()
    }

    pub fn control(&self) -> ControlContract {
        self.control
    }

    pub fn with_control(&self, control: ControlContract) -> Self {
        let mut scenario = self.clone();
        scenario.control = control;
        scenario
    }

    pub fn with_generator(&self, generator: GeneratorSpec) -> Result<Self, Fault> {
        let mut scenario = self.clone();
        scenario.generator = generator;
        scenario.coherent()?;
        Ok(scenario)
    }

    pub fn with_assembly_template(
        &self,
        assembly_template: AssemblyTemplate,
    ) -> Result<Self, Fault> {
        if self.contract_id.is_none() || !assembly_template.is_exact() {
            return Err(Fault::field("assembly_template"));
        }
        let mut scenario = self.clone();
        scenario.assembly_template = Some(assembly_template);
        scenario.coherent()?;
        Ok(scenario)
    }

    pub fn generator(&self) -> &GeneratorSpec {
        &self.generator
    }

    pub fn criterion(&self, chapter_index: u8) -> Option<&crate::criterion::FunctionCriterionSpec> {
        self.criteria
            .get(usize::from(chapter_index))
            .and_then(Option::as_ref)
    }

    pub fn pressure_schedule(&self) -> &crate::pressure::Schedule {
        &self.pressure_schedule
    }

    pub fn regime(&self) -> &RegimeSpec {
        &self.regime
    }

    pub fn scenario_hash(&self) -> String {
        crate::json::hex_bytes(&sha256::digest(self.definition_written().as_bytes()))
    }

    fn coherent(&self) -> Result<(), Fault> {
        self.generator.coherent()?;
        if self.contract_id.is_some() != self.assembly_template.is_some() {
            return Err(Fault::field("assembly_template"));
        }
        if !self.generator.chapters.is_empty() && self.criteria.len() != self.generator.chapters.len() {
            return Err(Fault::field("criteria"));
        }
        Ok(())
    }

    fn read(value: &Json, key: &str) -> Result<Self, Fault> {
        let found = read::map(value, key)?;
        let legacy_contract_shape = found.get("contract_id").is_none();
        let current_assembly_shape = found.get("assembly_template").is_some();
        let legacy_assembly_shape = !current_assembly_shape;
        let mut declared = vec![
            "content_hash",
            "control",
            "criteria",
            "generator",
            "pressure_schedule",
            "regime",
            "scenario_hash",
        ];
        if !legacy_contract_shape {
            declared.push("contract_id");
        }
        if current_assembly_shape {
            declared.push("assembly_template");
        } else if found.get("assembly_template_hash").is_some() {
            declared.push("assembly_template_hash");
        }
        read::exact_keys(
            found,
            key,
            &declared,
        )?;
        let contract_id = match found.get("contract_id") {
            None | Some(Json::Null) => None,
            Some(Json::Text(id))
                if !id.is_empty()
                    && id.starts_with(|first: char| first.is_ascii_lowercase())
                    && id.chars().all(|held| {
                        held.is_ascii_lowercase() || held.is_ascii_digit() || held == '_'
                    }) =>
            {
                Some(id.clone())
            }
            _ => return Err(Fault::field("contract_id")),
        };
        let assembly_template = if current_assembly_shape {
            AssemblyTemplate::read(found, "assembly_template")?
        } else {
            match found.get("assembly_template_hash") {
                None | Some(Json::Null) => None,
                Some(Json::Text(hash))
                    if hash.len() == 64 && hash.bytes().all(|byte| byte.is_ascii_hexdigit()) =>
                {
                    Some(AssemblyTemplate::migrated(hash.clone()))
                }
                _ => return Err(Fault::field("assembly_template_hash")),
            }
        };
        let scenario = ScenarioSpec {
            assembly_template,
            content_hash: read::hex(found, "content_hash", 64)?.to_string(),
            contract_id,
            control: ControlContract::read(found, "control")?,
            criteria: read::list(found, "criteria", usize::from(u8::MAX) + 1)?
                .iter()
                .map(|entry| match entry {
                    Json::Null => Ok(None),
                    _ => crate::criterion::FunctionCriterionSpec::read_value(entry, "criteria")
                        .map(Some),
                })
                .collect::<Result<Vec<_>, Fault>>()?,
            generator: GeneratorSpec::read(found, "generator")?,
            pressure_schedule: crate::pressure::Schedule::read(found, "pressure_schedule")?,
            regime: RegimeSpec::read(found, "regime")?,
        };
        scenario.coherent()?;
        let legacy_generator = read::map(found, "generator")?.get("local_policy").is_none();
        if !legacy_generator
            && !legacy_contract_shape
            && !legacy_assembly_shape
            && read::hex(found, "scenario_hash", 64)? != scenario.scenario_hash()
        {
            return Err(Fault::field("scenario_hash"));
        }
        Ok(scenario)
    }

    fn write_shape(&self, out: &mut String, include_hash: bool) {
        let mut object = Obj::new(out);
        match &self.assembly_template {
            Some(template) => object.raw("assembly_template", &template.written()),
            None => object.null("assembly_template"),
        };
        object.text("content_hash", &self.content_hash);
        match &self.contract_id {
            Some(id) => object.text("contract_id", id),
            None => object.null("contract_id"),
        };
        object.text("control", self.control.name());
        {
            let mut criteria = object.list("criteria");
            for criterion in &self.criteria {
                match criterion {
                    Some(criterion) => criteria.raw(&criterion.written()),
                    None => criteria.raw("null"),
                };
            }
            criteria.end();
        }
        let mut generator = String::new();
        self.generator.write(&mut generator);
        object.raw("generator", &generator);
        object.raw("pressure_schedule", &self.pressure_schedule.written());
        let mut regime = String::new();
        self.regime.write(&mut regime);
        object.raw("regime", &regime);
        if include_hash {
            object.text("scenario_hash", &self.scenario_hash());
        }
        object.end();
    }

    fn definition_written(&self) -> String {
        let mut out = String::new();
        self.write_shape(&mut out, false);
        out
    }

    fn write(&self, out: &mut String) {
        self.write_shape(out, true);
    }
}

/// The root authoritative aggregate, whose serialized form is exactly the
/// current save payload.
#[derive(Clone, Debug)]
pub struct RunState {
    pub run_id: String,
    pub run_kind: RunKind,
    pub attempt: Option<AttemptRecord>,
    pub attempt_branch: Option<AttemptBranchRecord>,
    pub rng: RngState,
    pub scenario: ScenarioSpec,
    pub criterion: Option<crate::criterion::CriterionRuntime>,
    pub branch_nonce: u32,
    pub progress: Progress,
    pub qualification_request: Option<QualificationRequest>,
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
    /// The V3 payload is exactly the thirteen locked keys, nothing more, and each is
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

    /// Deterministically migrates the former V2 payload. V2 did not retain a
    /// separate opening declaration, so its oldest retained keyframe becomes
    /// the disclosed initial side information of the migrated run.
    pub fn migrate_v2(payload: &Json) -> Result<Self, Fault> {
        Self::read_version(payload, 2)
    }

    /// Migrates the topology-only V3 GeneratorSpec by installing an empty
    /// local policy without inventing automation behavior.
    pub fn migrate_v3(payload: &Json) -> Result<Self, Fault> {
        Self::read_version(payload, 3)
    }

    /// Migrates the first automation-policy payload. V4 carried only an
    /// assembly hash and branch nonce, so contract records are marked migrated
    /// and cannot claim exact qualification input or inferred parentage.
    pub fn migrate_v4(payload: &Json) -> Result<Self, Fault> {
        Self::read_version(payload, 4)
    }

    /// Migrates the first exact attempt/branch payload. V5 predates immutable
    /// qualification requests, so it opens with no sealed request.
    pub fn migrate_v5(payload: &Json) -> Result<Self, Fault> {
        Self::read_version(payload, 5)
    }

    /// Migrates the V6 qualification payload. V6 predates transition receipts
    /// on branch records, so every imported branch begins with no accepted
    /// engineering-transition receipt rather than inferring one from its label.
    pub fn migrate_v6(payload: &Json) -> Result<Self, Fault> {
        Self::read_version(payload, 6)
    }

    fn read_version(payload: &Json, version: i64) -> Result<Self, Fault> {
        let keys: &[&str] = if version >= 6 {
            &[
                "anchors",
                "attempt_branch",
                "attempt_record",
                "branch_nonce",
                "criterion_runtime",
                "field",
                "input_config",
                "pressures",
                "progress",
                "qualification_request",
                "rng",
                "run_id",
                "run_kind",
                "save_version",
                "scenario_spec",
                "slate",
                "view",
            ]
        } else if version >= 5 {
            &[
                "anchors",
                "attempt_branch",
                "attempt_record",
                "branch_nonce",
                "criterion_runtime",
                "field",
                "input_config",
                "pressures",
                "progress",
                "rng",
                "run_id",
                "run_kind",
                "save_version",
                "scenario_spec",
                "slate",
                "view",
            ]
        } else if version >= 3 {
            &[
                "anchors",
                "branch_nonce",
                "criterion_runtime",
                "field",
                "input_config",
                "pressures",
                "progress",
                "rng",
                "run_id",
                "save_version",
                "scenario_spec",
                "slate",
                "view",
            ]
        } else {
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
            ]
        };
        read::exact_keys(payload, "payload", keys)?;
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

        let scenario = if version >= 3 {
            ScenarioSpec::read(payload, "scenario_spec")?
        } else {
            ScenarioSpec::legacy(read::hex(payload, "content_hash", 64)?.to_string())
        };
        let run_id = read::hex(payload, "run_id", 16)?.to_string();
        let branch_nonce =
            read::int(payload, "branch_nonce", 0, i64::from(u32::MAX))? as u32;
        let run_kind = if version >= 5 {
            RunKind::read(payload, "run_kind")?
        } else if scenario.contract_id().is_some() {
            RunKind::AutomationContract
        } else {
            RunKind::LegacyCampaign
        };
        let (attempt, attempt_branch) = if version >= 5 {
            (
                AttemptRecord::read(payload, "attempt_record")?,
                AttemptBranchRecord::read(payload, "attempt_branch")?,
            )
        } else if let (Some(contract_id), Some(assembly_hash)) =
            (scenario.contract_id(), scenario.assembly_template_hash())
        {
            let generator_hash = scenario.generator().specification_hash();
            let attempt = AttemptRecord::new(
                run_id.clone(),
                contract_id.to_string(),
                scenario.content_hash().to_string(),
                generator_hash.clone(),
                assembly_hash.to_string(),
                AttemptSource::Migrated,
            )?;
            let branch = AttemptBranchRecord::new(
                run_id.clone(),
                None,
                BranchOperation::Migrated,
                generator_hash,
                assembly_hash.to_string(),
                branch_nonce,
            )?;
            (Some(attempt), Some(branch))
        } else {
            (None, None)
        };
        let progress = Progress::read(payload, "progress")?;
        let qualification_request = if version >= 6 {
            QualificationRequest::read(payload, "qualification_request")?
        } else {
            None
        };
        let no_criterion = Json::Null;
        let criterion_value = if version >= 3 {
            read::at(payload, "criterion_runtime")?
        } else {
            &no_criterion
        };
        let criterion = match (scenario.criterion(progress.chapter_index), criterion_value) {
            (Some(_spec), Json::Null) => {
                return Err(Fault::field("criterion_runtime"));
            }
            (Some(spec), _) => Some(crate::criterion::CriterionRuntime::read(
                payload,
                "criterion_runtime",
                spec,
            )?),
            (None, Json::Null) => None,
            (None, _) => return Err(Fault::field("criterion_runtime")),
        };
        Ok(RunState {
            run_id,
            run_kind,
            attempt,
            attempt_branch,
            rng: RngState::read(payload, "rng")?,
            scenario,
            criterion,
            branch_nonce,
            progress,
            qualification_request,
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
        match &self.attempt_branch {
            Some(branch) => object.raw("attempt_branch", &branch.written()),
            None => object.null("attempt_branch"),
        };
        match &self.attempt {
            Some(attempt) => object.raw("attempt_record", &attempt.written()),
            None => object.null("attempt_record"),
        };
        object.int("branch_nonce", i64::from(self.branch_nonce));
        match &self.criterion {
            Some(runtime) => object.raw("criterion_runtime", &runtime.written()),
            None => object.null("criterion_runtime"),
        };
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
        match &self.qualification_request {
            Some(request) => object.raw("qualification_request", &request.written()),
            None => object.null("qualification_request"),
        };
        object.raw("rng", &self.rng.write());
        object.text("run_id", &self.run_id);
        object.text("run_kind", self.run_kind.name());
        object.int("save_version", SAVE_VERSION);
        {
            let mut scenario = String::new();
            self.scenario.write(&mut scenario);
            object.raw("scenario_spec", &scenario);
        }
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
        self.scenario.coherent()?;
        match self.run_kind {
            RunKind::AutomationContract => {
                let contract_id = self
                    .scenario
                    .contract_id()
                    .ok_or_else(|| Fault::field("run_kind"))?;
                let assembly = self
                    .scenario
                    .assembly_template()
                    .ok_or_else(|| Fault::field("assembly_template"))?;
                let attempt = self
                    .attempt
                    .as_ref()
                    .ok_or_else(|| Fault::field("attempt_record"))?;
                let branch = self
                    .attempt_branch
                    .as_ref()
                    .ok_or_else(|| Fault::field("attempt_branch"))?;
                if attempt.attempt_id != self.run_id
                    || attempt.contract_id != contract_id
                    || attempt.content_hash != self.scenario.content_hash
                    || attempt.opening_assembly_hash != assembly.hash
                    || (attempt.source == AttemptSource::Opened && !assembly.is_exact())
                    || branch.attempt_id != attempt.attempt_id
                    || branch.generator_hash != self.scenario.generator.specification_hash()
                    || branch.assembly_hash != assembly.hash
                    || branch.branch_nonce != self.branch_nonce
                {
                    return Err(Fault::field("attempt_branch"));
                }
                if let Some(request) = &self.qualification_request {
                    let input = request.input();
                    if read::hex(input, "branch_id", 64)? != branch.branch_id
                        || read::int(input, "branch_nonce", 0, i64::from(u32::MAX))?
                            != i64::from(self.branch_nonce)
                        || read::hex(input, "generator_spec_hash", 64)?
                            != self.scenario.generator.specification_hash()
                        || read::hex(input, "assembly_template_hash", 64)? != assembly.hash
                        || read::hex(input, "content_hash", 64)? != self.scenario.content_hash
                        || read::text(input, "contract_id")? != contract_id
                        || read::hex(input, "scenario_hash", 64)?
                            != self.scenario.scenario_hash()
                    {
                        return Err(Fault::field("qualification_request"));
                    }
                }
            }
            RunKind::OpenField | RunKind::LegacyCampaign => {
                if self.scenario.contract_id().is_some()
                    || self.attempt.is_some()
                    || self.attempt_branch.is_some()
                    || self.qualification_request.is_some()
                {
                    return Err(Fault::field("run_kind"));
                }
            }
        }
        if self.scenario.criterion(self.progress.chapter_index).is_some()
            != self.criterion.is_some()
        {
            return Err(Fault::field("criterion_runtime"));
        }
        crate::field::validate(&self.now)?;
        crate::field::validate(&self.trace.keyframe)?;
        if !self.scenario.generator.accepts_field(self.progress.chapter_index, &self.now)
            || !self
                .scenario
                .generator
                .accepts_field(self.progress.chapter_index, &self.trace.keyframe)
        {
            return Err(Fault::field("generator_spec"));
        }
        let policy_addresses: Vec<u32> = self
            .scenario
            .generator
            .local_policy()
            .components()
            .iter()
            .map(|component| component.address)
            .collect();
        if [&self.now, &self.trace.keyframe].iter().any(|field| {
            field
                .policy_runtime
                .iter()
                .any(|runtime| policy_addresses.binary_search(&runtime.address).is_err())
        }) {
            return Err(Fault::field("policy_runtime"));
        }
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
