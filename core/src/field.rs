//! The Field model: its parts, its locked caps, and one step of it.
//!
//! `docs/field-framework/ARCHITECTURE.md` locks the six parts field by field —
//! `FieldLayer`, `FormState`, `CurrentState`, `PortState`, `RouteState`,
//! `PhysicalCompartment`, and `BoundaryState` — together with the capacity
//! table every quantity is held to, the Node-kind set and its starting Charge, and the distance and
//! adjacency rules of `crate::fx`. `docs/field-framework/FRAMEWORK.md` item 6
//! locks the records a completed step carries and the identity they satisfy:
//!
//! ```text
//! q(n, t) = q(n, t - 1)
//!         + sum of f(r, t) over Routes r with head n
//!         - sum of f(r, t) over Routes r with tail n
//!         - upkeep(n, t) + e(n, t)
//! ```
//!
//! The ledger below carries that identity per Node and in aggregate, as exact
//! integer accounting: every step's deltas sum to zero across it, sinks
//! included, with no tolerance anywhere.
//!
//! ARCHITECTURE.md's `Field model rules` section locks the five rules that move
//! Charge, and each is implemented here verbatim at the phase the locked step
//! order gives it: the Pulse, the one player-triggered rule, charging and
//! emitting from where the Form now stands; Route flow as one ascending pass
//! with an `open` gate, a source-shortfall term and a destination-headroom term;
//! physical-compartment leakage over material shell members by exposure and
//! `leak_per_exposed_contact_per_step`; Node overload as an inflow throttle and
//! a quarter-of-the-excess decay with memoryless recovery; and Current delivery split across the Nodes
//! standing in a current with an exact remainder. The Pressure effects scale
//! and steer these five and add no sixth mover; the Noise flow scale among
//! them is the one drawing rule, at its locked drawing point — the start of
//! the Route phase, layers ascending — and every rule names its ledger
//! entries.
//!
//! One phase stays reserved: the Node phase pays upkeep, and the attribution of
//! upkeep across its five locked purposes is still locked nowhere, so no upkeep
//! falls due and the trace writes no upkeep entry.

use crate::fault::{Code, Fault};
use crate::fx::{adjacent, distance, fixed_mul, hold, within, Vec2, ONE_UNIT, STORED_BOUND};
use crate::json::{Json, Obj};
use crate::pressure::{self, PressureState, Pressure, Schedule, TargetKind};
use crate::read;
use crate::rng::RngState;
use crate::run::FORMS;
use crate::state::{ControlState, FieldState, Frac, Fx, Step, FRAC_ONE};

/// Nodes per Run. A Form is a Node too, so the cap covers Ports and Forms
/// together.
pub const NODES_PER_RUN: usize = 256;

/// Routes per Run.
pub const ROUTES_PER_RUN: usize = 512;

/// Layers per chapter.
pub const LAYERS_PER_CHAPTER: usize = 8;

/// Currents per chapter.
pub const CURRENTS_PER_CHAPTER: usize = 32;

/// Forms per Run.
pub const FORMS_PER_RUN: usize = 8;

/// The largest layer identifier: layers run over [0, 7].
pub const MAX_LAYER: u8 = 7;

/// Stored Charge per Node: 4096 units.
pub const NODE_CHARGE_CAP: Fx = 4096 * ONE_UNIT;

/// Route capacity: 256 units per step.
pub const ROUTE_CAPACITY_CAP: Fx = 256 * ONE_UNIT;

/// Current strength: 256 units per step.
pub const CURRENT_STRENGTH_CAP: Fx = 256 * ONE_UNIT;

/// The widest boundary-leakage parameter: 1/8 per exposed link per step.
pub const LEAK_FRAC_CAP: Frac = 8192;

/// The share of a Node's excess that decays each step it stands overloaded:
/// one quarter.
pub const OVERLOAD_DECAY: Frac = 16_384;

// ---------------------------------------------------------------------------
// The Pulse.
//
// `docs/field-framework/ARCHITECTURE.md`'s `The Pulse` rule locks every number
// below, and none of them is chosen here. A held control charges by a fixed
// share per step; the reach is derived from the charge and never stored; an
// emission gathers, opens, and displaces in that order and consumes the charge.
// ---------------------------------------------------------------------------

/// What one held step adds to a Form's Pulse charge: full in 32 held steps,
/// about 1.07 s, with a nine-step tap reaching 18432.
pub const PULSE_CHARGE_STEP: Frac = 2_048;

/// The reach of an empty release: 8 units.
pub const PULSE_RADIUS_BASE: Fx = 8 * ONE_UNIT;

/// What each raw unit of charge adds to the reach. At full charge the reach is
/// 192 units, which the locked distance rule's 512-unit layer term keeps on the
/// Form's own layer by construction.
pub const PULSE_RADIUS_PER_CHARGE: Fx = 184;

/// The share of what a source holds that one emission gathers: one quarter.
pub const PULSE_GATHER_SHARE: Frac = 16_384;

/// The share of an interference-class level one emission displaces: one
/// quarter.
pub const PULSE_DISPLACE_SHARE: Frac = 16_384;

/// The most cues one frame carries; the oldest is dropped past it.
pub const CUES_PER_FRAME: usize = 16;

/// The cue kinds the Pulse raises, in the closed set's own numbering.
pub const CUE_PULSE_EMITTED: u8 = 1;
pub const CUE_CHARGE_GATHERED: u8 = 2;
pub const CUE_PORT_OPENED: u8 = 3;
pub const CUE_INTERFERENCE_PUSHED: u8 = 12;

/// The cue kinds the authored sequence raises, from the same closed set: the
/// Anchor a completion writes, the setback the authored break stands as, the
/// recovery from it, the completion itself, and — for the disruptions a chapter
/// times against its own objectives — the Break the closed set already names.
pub const CUE_ROUTE_CUT: u8 = 5;
pub const CUE_BREAK: u8 = 6;
pub const CUE_ANCHOR_WRITTEN: u8 = 7;
pub const CUE_COLLAPSE: u8 = 8;
pub const CUE_RECOVERY: u8 = 9;
pub const CUE_OBJECTIVE_COMPLETE: u8 = 11;

/// The reach an emission at this charge has, raw. Derived on demand and never
/// stored, so no state and no serialized byte carries it.
pub fn pulse_radius(pulse_charge: Frac) -> Fx {
    PULSE_RADIUS_BASE + PULSE_RADIUS_PER_CHARGE * pulse_charge
}

/// A reach as the render snapshot carries it: Q8.8 units, which is the raw
/// value divided by 256, flooring, saturating at the field's width.
pub fn reach_ticks(radius: Fx) -> u16 {
    (radius >> 8).clamp(0, i64::from(u16::MAX)) as u16
}

/// One cue a step raised: render-only, never authoritative, and never
/// serialized. `b` is the Node the cue stands at; what `a` carries is locked
/// per kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cue {
    pub kind: u8,
    pub a: u16,
    pub b: u32,
}

/// The step inputs this goal adds, in one place: the standing pressures, the
/// authored tables the stage machine reads, and the trajectory stream position
/// the step draws from.
///
/// What keeps pressure regeneration exact is locked in
/// [`crate::pressure`]'s own header: membership is immutable inside a window
/// and every rule that changes it ends the window, while `stage` and `level`
/// are pure functions of `step - start_step` and the authored table, recomputed at the step being
/// replayed. So a replay hands this the live list and reproduces the recorded
/// window byte for byte.
///
/// The stream is the step's supplied stream, consumed at ARCHITECTURE.md's
/// one locked drawing point: the Noise flow scale draws from it at the start
/// of the Route phase, layers ascending, one word per layer with positive
/// effective noise. A live step hands the trajectory stream and its position
/// advances with the draws; a regeneration hands the recorded per-step
/// position and re-draws exactly what the live step drew; a framework sample
/// hands its own named stream and legitimately diverges.
pub struct Staging<'a> {
    pub pressures: &'a mut Vec<PressureState>,
    pub schedule: &'a Schedule,
    pub stream: &'a mut RngState,
}

/// A step with nothing staged, owned by the caller.
///
/// A caller with no pressure list still threads the seam rather than passing a
/// null one, so there is exactly one shape of a step and no second path through
/// the phase order.
#[derive(Clone, Debug, Default)]
pub struct Unstaged {
    pub pressures: Vec<PressureState>,
    pub schedule: Schedule,
    pub stream: RngState,
}

impl Unstaged {
    pub fn staging(&mut self) -> Staging<'_> {
        Staging {
            pressures: &mut self.pressures,
            schedule: &self.schedule,
            stream: &mut self.stream,
        }
    }
}

/// The width of a layer plane per axis: positions run over [0, 4096) units.
pub const PLANE_SPAN: Fx = 4096 * ONE_UNIT;

// ---------------------------------------------------------------------------
// The steering feel.
//
// `docs/field-framework/ARCHITECTURE.md` locks the shape of steering and none
// of its numbers. It locks that the control arrives as one normalized Q1.15
// vector however it was produced, that the response is componentwise linear
// algebra with no transcendental function anywhere, and that every operation is
// exact integer arithmetic. The four values below are chosen, and they are
// chosen here, in one place, beside the shell's own three in
// `app/src/shell/steering.ts`: together those seven are the reference feel, and
// every device reaches the Field through them because every device reaches it
// through this one control state.
//
// The rule is a spring against a damper — the plainest form that carries both a
// heading and an inertia:
//
// ```text
// offset = STEER_REACH * pointer_speed * control   the point the control names
// pull   = STEER_STIFFNESS * offset                the spring
// drag   = STEER_DAMPING * vel                     the damper
// vel   += pull - drag
// ```
//
// Nothing overshoots: the spring pulls against the velocity's own balance
// rather than against a stored acceleration, so a Form converges on the speed
// the control asks for and never past it. Steering that overshoots its heading
// reads as loose rather than as fluid, and the line this serves — SPEC.md's
// spring damping, so trackpad movement feels fluid rather than twitchy — asks
// for the smoothing, not for the wobble.
//
// What the three numbers come to at the default pointer speed, which is the
// feel a reader should be able to check against the game:
//
// | Reading | Value |
// |---|---|
// | Speed at full deflection | `STIFFNESS * REACH / DAMPING` = 20 units per step, 600 per second |
// | Of that, reached in 1 step | 25%, so the first frame of a gesture already moves |
// | Reached in 4 steps (133 ms) | 68% |
// | Reached in 11 steps (367 ms) | 95% |
// | Crossing the drawn view, 1400 units | 2.3 s |
// | Crossing a whole layer plane, 4096 units | 6.8 s |
//
// The damping coefficient's own window: at 1 the approach is exact in one step,
// above 1 it alternates about the speed it is approaching, and at 2 or beyond
// it diverges. Below about 1/16 it would take longer to settle than a gesture
// lasts. A quarter per step is well inside the settling half of that window and
// is what the 133 ms above is.
// ---------------------------------------------------------------------------

/// The point a fully deflected control names, measured from the Form: 320
/// units. With the stiffness and the damping below it is one half of the
/// terminal-speed identity rather than a place the Form is ever put — a
/// steered Form approaches the speed the offset asks for, and never arrives at
/// the offset itself.
pub const STEER_REACH: Fx = 320 * ONE_UNIT;

/// The spring constant: 1/64 of the offset, as a velocity, per step.
pub const STEER_STIFFNESS: Frac = 1_024;

/// The damping coefficient: a quarter of the standing velocity per step.
pub const STEER_DAMPING: Frac = 16_384;

/// The raw speed below which a Form under no control stands at rest. The
/// damper floors, so the last raw unit or two of a decaying velocity would
/// otherwise stand forever and a parked Form would never quite stop. A
/// thousandth of a unit per step is three hundredths of a unit per second: far
/// below anything the surface can draw, and reached only under a control of
/// exactly zero, so no motion the player asked for is ever squared off.
pub const STEER_REST: Fx = ONE_UNIT / 1024;

/// Drawn Boundary entries retained, most recent first.
pub const DRAWN_RETAINED: usize = 32;

/// The path point count a current is authored with.
pub const CURRENT_PATH_POINTS: std::ops::RangeInclusive<usize> = 2..=64;

/// Forecast depth, in steps, authored per Form.
pub const FORECAST_DEPTH_CAP: u16 = 30;

/// The steering response scale a Form authors: a quarter of the reference
/// speed to four times it, around a default of one. It scales the steering
/// reach and nothing else, so terminal speed moves with it and the settling
/// feel does not.
pub const STEER_SCALE_LOW: Frac = 16_384;
pub const STEER_SCALE_HIGH: Frac = 262_144;

/// Trail entries waiting to fall due, at most. A deposit past the cap drops
/// the oldest entry, so the queue is bounded whatever a run does.
pub const PENDING_TRAILS: usize = 64;

/// How often a Trail-authoring Form deposits, in steps.
pub const TRAIL_PERIOD: std::ops::RangeInclusive<u16> = 5..=60;

/// How long a deposited entry waits before it falls due, in steps.
pub const TRAIL_DELAY: std::ops::RangeInclusive<u16> = 30..=300;

/// How far a due entry reaches for its recipients.
pub const TRAIL_RADIUS_CAP: Fx = 256 * ONE_UNIT;

/// How much Charge one due entry delivers.
pub const TRAIL_MAGNITUDE_CAP: Fx = 64 * ONE_UNIT;

/// The five purposes every unit of upkeep is attributed to, in the locked
/// order: boundary, repair, replacement, movement, reserve.
pub const UPKEEP_PURPOSES: usize = 5;

/// The closed Node-kind set of version 1, each with its starting Charge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeKind {
    Port,
    Reserve,
    Module,
    Form,
}

impl NodeKind {
    pub fn name(self) -> &'static str {
        match self {
            NodeKind::Port => "port",
            NodeKind::Reserve => "reserve",
            NodeKind::Module => "module",
            NodeKind::Form => "form",
        }
    }

    /// The kind's name as a body carries it.
    pub fn read(name: &str) -> Option<Self> {
        match name {
            "port" => Some(NodeKind::Port),
            "reserve" => Some(NodeKind::Reserve),
            "module" => Some(NodeKind::Module),
            "form" => Some(NodeKind::Form),
            _ => None,
        }
    }

    /// The nonnegative Charge a fresh Node of this kind holds at creation:
    /// `port` 0, `reserve` 64, `module` 16, `form` 8 units.
    pub fn starting_charge(self) -> Fx {
        match self {
            NodeKind::Port => 0,
            NodeKind::Reserve => 64 * ONE_UNIT,
            NodeKind::Module => 16 * ONE_UNIT,
            NodeKind::Form => 8 * ONE_UNIT,
        }
    }
}

/// One layer and its three difficulty parameters.
///
/// `drain` is Charge removed per step by depth, `noise` distorts routes and
/// forecasts, and `gain` scales rewards. All three are authored per layer;
/// deeper layers are authored with larger values, which is what makes depth an
/// active difficulty choice. Only `drain` acts on Charge, so only `drain` is
/// applied by a step here: `noise` is read by the goals that own routing
/// distortion and forecasts, and `gain` by the goal that owns rewards.
#[derive(Clone, Debug)]
pub struct FieldLayer {
    pub layer: u8,
    pub drain: Fx,
    pub noise: Frac,
    pub gain: Frac,
    pub current_ids: Vec<u16>,
    pub port_ids: Vec<u32>,
}

impl FieldLayer {
    /// Reads one layer out of a payload. Shape, keys, and types are held here;
    /// every range and cap is held by [`validate`] over the whole Field, so one
    /// rule answers for a restored Field and a live one alike.
    pub fn read(value: &Json) -> Result<Self, Fault> {
        read::exact_keys(
            value,
            "layers",
            &["current_ids", "drain", "gain", "layer", "noise", "port_ids"],
        )?;
        let current_ids = read::ids(value, "current_ids", CURRENTS_PER_CHAPTER, i64::from(u16::MAX))?;
        Ok(FieldLayer {
            layer: read::int(value, "layer", 0, i64::from(MAX_LAYER))? as u8,
            drain: read::int(value, "drain", i64::MIN, i64::MAX)?,
            noise: read::int(value, "noise", i64::MIN, i64::MAX)?,
            gain: read::int(value, "gain", i64::MIN, i64::MAX)?,
            current_ids: current_ids.into_iter().map(|id| id as u16).collect(),
            port_ids: read::ids(value, "port_ids", NODES_PER_RUN, i64::from(u32::MAX))?,
        })
    }

    pub fn write(&self, out: &mut String) {
        let mut object = Obj::new(out);
        {
            let mut ids = object.list("current_ids");
            for id in &self.current_ids {
                ids.int(i64::from(*id));
            }
            ids.end();
        }
        object.int("drain", self.drain);
        object.int("gain", self.gain);
        object.int("layer", i64::from(self.layer));
        object.int("noise", self.noise);
        {
            let mut ids = object.list("port_ids");
            for id in &self.port_ids {
                ids.int(i64::from(*id));
            }
            ids.end();
        }
        object.end();
    }
}

/// What a linked Form stands in: the station it holds, as an offset from the
/// Form that is steered, and the distance past which it stands separated.
///
/// Both are the `linked_forms` ability's own authored numbers, copied into the
/// state when the Field is established for the reason `steer_scale` is —
/// following and separation are rules of a step, a step reads state and never
/// content, and a content change may not re-time a recorded run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LinkState {
    pub offset: Vec2,
    pub separation: Fx,
}

/// What a Trail-authoring Form deposits: how often, how long an entry waits,
/// how far a due entry reaches, and how much Charge it carries. Authored by
/// the `trail` ability and carried in the state for the same reason.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TrailState {
    pub period: u16,
    pub delay: u16,
    pub radius: Fx,
    pub magnitude: Fx,
}

/// One deposited Trail entry, standing until the step it falls due on.
///
/// The entry carries what the delivery needs of the moment it was left —
/// where, on which layer, when it comes due, and how much — and names its Form
/// for the one thing it does not carry, the reach that Form authored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PendingTrail {
    pub form: u8,
    pub layer: u8,
    pub pos: Vec2,
    pub due: Step,
    pub magnitude: Fx,
}

impl PendingTrail {
    pub fn read(value: &Json) -> Result<Self, Fault> {
        read::exact_keys(value, "pending", &["due", "form", "layer", "magnitude", "pos"])?;
        Ok(PendingTrail {
            form: read::int(value, "form", 0, i64::from(u8::MAX))? as u8,
            layer: read::int(value, "layer", 0, i64::from(u8::MAX))? as u8,
            pos: Vec2::read(value, "pos")?,
            due: read::int(value, "due", 0, i64::from(u32::MAX))? as Step,
            magnitude: read::int(value, "magnitude", i64::MIN, i64::MAX)?,
        })
    }

    pub fn write(&self, out: &mut String) {
        let mut object = Obj::new(out);
        object.int("due", i64::from(self.due));
        object.int("form", i64::from(self.form));
        object.int("layer", i64::from(self.layer));
        object.int("magnitude", self.magnitude);
        object.raw("pos", &self.pos.written());
        object.end();
    }
}

/// One Form: a Node of the Field that also carries a position, a velocity, and
/// the parameters its authored Form data supplies.
#[derive(Clone, Debug)]
pub struct FormState {
    pub id: u8,
    pub form: String,
    pub node: u32,
    pub controlled: bool,
    pub layer: u8,
    pub pos: Vec2,
    pub vel: Vec2,
    pub charge: Fx,
    pub reserve: Fx,
    pub pulse_charge: Frac,
    pub focus: bool,
    pub route_reach: Fx,
    pub forecast_depth: u16,
    pub steer_scale: Frac,
    /// What a Route this Form forms carries per step, copied from its authored
    /// data at run start like the reach the same command reads.
    pub route_capacity: Fx,
    /// The `linked_forms` ability's station and separation, for a Form one
    /// selection stands beside the Form it steers; none for every other Form.
    pub link: Option<LinkState>,
    /// The `trail` ability's parameters; none for every Form that authors no
    /// Trail.
    pub trail: Option<TrailState>,
}

impl FormState {
    pub fn read(value: &Json) -> Result<Self, Fault> {
        read::exact_keys(
            value,
            "forms",
            &[
                "charge",
                "controlled",
                "focus",
                "forecast_depth",
                "form",
                "id",
                "layer",
                "link",
                "node",
                "pos",
                "pulse_charge",
                "reserve",
                "route_capacity",
                "route_reach",
                "steer_scale",
                "trail",
                "vel",
            ],
        )?;
        let link = match read::map_or_null(value, "link")? {
            Some(held) => {
                read::exact_keys(held, "link", &["offset", "separation"])?;
                Some(LinkState {
                    offset: Vec2::read(held, "offset")?,
                    separation: read::int(held, "separation", i64::MIN, i64::MAX)?,
                })
            }
            None => None,
        };
        let trail = match read::map_or_null(value, "trail")? {
            Some(held) => {
                read::exact_keys(held, "trail", &["delay", "magnitude", "period", "radius"])?;
                Some(TrailState {
                    period: read::int(held, "period", 0, i64::from(u16::MAX))? as u16,
                    delay: read::int(held, "delay", 0, i64::from(u16::MAX))? as u16,
                    radius: read::int(held, "radius", i64::MIN, i64::MAX)?,
                    magnitude: read::int(held, "magnitude", i64::MIN, i64::MAX)?,
                })
            }
            None => None,
        };
        Ok(FormState {
            steer_scale: read::int(value, "steer_scale", i64::MIN, i64::MAX)?,
            route_capacity: read::int(value, "route_capacity", i64::MIN, i64::MAX)?,
            link,
            trail,
            id: read::int(value, "id", 0, i64::from(u8::MAX))? as u8,
            form: read::text(value, "form")?.to_string(),
            node: read::int(value, "node", 0, i64::from(u32::MAX))? as u32,
            controlled: read::flag(value, "controlled")?,
            layer: read::int(value, "layer", 0, i64::from(u8::MAX))? as u8,
            pos: Vec2::read(value, "pos")?,
            vel: Vec2::read(value, "vel")?,
            charge: read::int(value, "charge", i64::MIN, i64::MAX)?,
            reserve: read::int(value, "reserve", i64::MIN, i64::MAX)?,
            pulse_charge: read::int(value, "pulse_charge", i64::MIN, i64::MAX)?,
            focus: read::flag(value, "focus")?,
            route_reach: read::int(value, "route_reach", i64::MIN, i64::MAX)?,
            forecast_depth: read::int(value, "forecast_depth", 0, i64::from(u16::MAX))? as u16,
        })
    }

    pub fn write(&self, out: &mut String) {
        let mut object = Obj::new(out);
        object.int("charge", self.charge);
        object.bool("controlled", self.controlled);
        object.bool("focus", self.focus);
        object.int("forecast_depth", i64::from(self.forecast_depth));
        object.text("form", &self.form);
        object.int("id", i64::from(self.id));
        object.int("layer", i64::from(self.layer));
        match &self.link {
            Some(held) => {
                let mut link = object.object("link");
                link.raw("offset", &held.offset.written());
                link.int("separation", held.separation);
                link.end();
            }
            None => {
                object.null("link");
            }
        }
        object.int("node", i64::from(self.node));
        object.raw("pos", &self.pos.written());
        object.int("pulse_charge", self.pulse_charge);
        object.int("reserve", self.reserve);
        object.int("route_capacity", self.route_capacity);
        object.int("route_reach", self.route_reach);
        object.int("steer_scale", self.steer_scale);
        match &self.trail {
            Some(held) => {
                let mut trail = object.object("trail");
                trail.int("delay", i64::from(held.delay));
                trail.int("magnitude", held.magnitude);
                trail.int("period", i64::from(held.period));
                trail.int("radius", held.radius);
                trail.end();
            }
            None => {
                object.null("trail");
            }
        }
        object.raw("vel", &self.vel.written());
        object.end();
    }
}

/// One current: an authored polyline flow on a layer, with `strength` in Charge
/// per step and a phase that advances 1 per step modulo its period.
#[derive(Clone, Debug)]
pub struct CurrentState {
    pub id: u16,
    pub layer: u8,
    pub path: Vec<Vec2>,
    pub width: Fx,
    pub strength: Fx,
    pub period: u16,
    pub phase: u16,
    pub bright: bool,
    pub active: bool,
}

impl CurrentState {
    pub fn read(value: &Json) -> Result<Self, Fault> {
        read::exact_keys(
            value,
            "currents",
            &[
                "active", "bright", "id", "layer", "path", "period", "phase", "strength", "width",
            ],
        )?;
        let points = read::list(value, "path", *CURRENT_PATH_POINTS.end())?;
        let mut path = Vec::with_capacity(points.len());
        for (place, _) in points.iter().enumerate() {
            // Each point is read through the same shape rule as any position:
            // a map of exactly the two raw axes.
            let point = Json::Map(vec![("path".to_string(), points[place].clone())]);
            path.push(Vec2::read(&point, "path")?);
        }
        Ok(CurrentState {
            id: read::int(value, "id", 0, i64::from(u16::MAX))? as u16,
            layer: read::int(value, "layer", 0, i64::from(u8::MAX))? as u8,
            path,
            width: read::int(value, "width", i64::MIN, i64::MAX)?,
            strength: read::int(value, "strength", i64::MIN, i64::MAX)?,
            period: read::int(value, "period", 0, i64::from(u16::MAX))? as u16,
            phase: read::int(value, "phase", 0, i64::from(u16::MAX))? as u16,
            bright: read::flag(value, "bright")?,
            active: read::flag(value, "active")?,
        })
    }

    pub fn write(&self, out: &mut String) {
        let mut object = Obj::new(out);
        object.bool("active", self.active);
        object.bool("bright", self.bright);
        object.int("id", i64::from(self.id));
        object.int("layer", i64::from(self.layer));
        {
            let mut path = object.list("path");
            for point in &self.path {
                path.raw(&point.written());
            }
            path.end();
        }
        object.int("period", i64::from(self.period));
        object.int("phase", i64::from(self.phase));
        object.int("strength", self.strength);
        object.int("width", self.width);
        object.end();
    }
}

/// One Port: a Node of the Field. Form-kind Nodes appear here too, with their
/// position mirroring their Form's each step.
#[derive(Clone, Debug)]
pub struct PortState {
    pub node: u32,
    pub layer: u8,
    pub pos: Vec2,
    pub kind: NodeKind,
    pub q: Fx,
    pub open: bool,
    pub upkeep_rate: Fx,
    pub capacity: Fx,
}

impl PortState {
    pub fn read(value: &Json) -> Result<Self, Fault> {
        read::exact_keys(
            value,
            "ports",
            &["capacity", "kind", "layer", "node", "open", "pos", "q", "upkeep_rate"],
        )?;
        let kind = NodeKind::read(read::text(value, "kind")?).ok_or_else(|| Fault::field("kind"))?;
        Ok(PortState {
            node: read::int(value, "node", 0, i64::from(u32::MAX))? as u32,
            layer: read::int(value, "layer", 0, i64::from(u8::MAX))? as u8,
            pos: Vec2::read(value, "pos")?,
            kind,
            q: read::int(value, "q", i64::MIN, i64::MAX)?,
            open: read::flag(value, "open")?,
            upkeep_rate: read::int(value, "upkeep_rate", i64::MIN, i64::MAX)?,
            capacity: read::int(value, "capacity", i64::MIN, i64::MAX)?,
        })
    }

    pub fn write(&self, out: &mut String) {
        let mut object = Obj::new(out);
        object.int("capacity", self.capacity);
        object.text("kind", self.kind.name());
        object.int("layer", i64::from(self.layer));
        object.int("node", i64::from(self.node));
        object.bool("open", self.open);
        object.raw("pos", &self.pos.written());
        object.int("q", self.q);
        object.int("upkeep_rate", self.upkeep_rate);
        object.end();
    }
}

/// One Route, directed tail to head. `flow` is `f(r, t)` for the most recent
/// completed step and `capacity` caps it.
#[derive(Clone, Debug)]
pub struct RouteState {
    pub route: u32,
    pub tail: u32,
    pub head: u32,
    pub capacity: Fx,
    pub flow: Fx,
    pub formed_step: Step,
}

impl RouteState {
    pub fn read(value: &Json) -> Result<Self, Fault> {
        read::exact_keys(
            value,
            "routes",
            &["capacity", "flow", "formed_step", "head", "route", "tail"],
        )?;
        Ok(RouteState {
            route: read::int(value, "route", 0, i64::from(u32::MAX))? as u32,
            tail: read::int(value, "tail", 0, i64::from(u32::MAX))? as u32,
            head: read::int(value, "head", 0, i64::from(u32::MAX))? as u32,
            capacity: read::int(value, "capacity", i64::MIN, i64::MAX)?,
            flow: read::int(value, "flow", i64::MIN, i64::MAX)?,
            formed_step: read::int(value, "formed_step", 0, i64::from(u32::MAX))? as Step,
        })
    }

    pub fn write(&self, out: &mut String) {
        let mut object = Obj::new(out);
        object.int("capacity", self.capacity);
        object.int("flow", self.flow);
        object.int("formed_step", i64::from(self.formed_step));
        object.int("head", i64::from(self.head));
        object.int("route", i64::from(self.route));
        object.int("tail", i64::from(self.tail));
        object.end();
    }
}

/// One drawn Boundary entry: the member set of a completed Handle drag, and the
/// step it was recorded at.
#[derive(Clone, Debug)]
pub struct DrawnBoundary {
    pub members: Vec<u32>,
    pub step: Step,
}

/// The two candidate-Boundary lists — drawn entries most recent first, capped
/// at the retained count, and the authored list in authored order.
///
/// These lists are observation metadata and candidate seeds. They do not
/// determine physical membership or leakage; [`PhysicalCompartment`] does.
#[derive(Clone, Debug, Default)]
pub struct BoundaryState {
    pub drawn: Vec<DrawnBoundary>,
    pub authored: Vec<Vec<u32>>,
}

impl BoundaryState {
    /// Reads the two candidate-Boundary lists out of a payload.
    pub fn read(value: &Json, key: &str) -> Result<Self, Fault> {
        let found = read::map(value, key)?;
        read::exact_keys(found, key, &["authored", "drawn"])?;

        Self::read_lists(found)
    }

    /// Reads the version-1 candidate lists and returns the causal leakage value
    /// that used to be stored beside them. Save migration moves that value to
    /// [`PhysicalCompartment`] and never exposes this shape to live V2 state.
    pub(crate) fn read_v1(value: &Json, key: &str) -> Result<(Self, Frac), Fault> {
        let found = read::map(value, key)?;
        read::exact_keys(found, key, &["authored", "drawn", "leak_frac"])?;
        let boundaries = Self::read_lists(found)?;
        let leak_frac = read::int(found, "leak_frac", i64::MIN, i64::MAX)?;
        Ok((boundaries, leak_frac))
    }

    fn read_lists(found: &Json) -> Result<Self, Fault> {
        let mut authored = Vec::new();
        for entry in read::list(found, "authored", NODES_PER_RUN)? {
            read::exact_keys(entry, "authored", &["members"])?;
            authored.push(read::ids(entry, "members", NODES_PER_RUN, i64::from(u32::MAX))?);
        }

        let mut drawn = Vec::new();
        for entry in read::list(found, "drawn", DRAWN_RETAINED)? {
            read::exact_keys(entry, "drawn", &["members", "step"])?;
            drawn.push(DrawnBoundary {
                members: read::ids(entry, "members", NODES_PER_RUN, i64::from(u32::MAX))?,
                step: read::int(entry, "step", 0, i64::from(u32::MAX))? as Step,
            });
        }

        Ok(BoundaryState { drawn, authored })
    }

    /// Appends a completed drag to the front, dropping the oldest entry past
    /// the retained count.
    pub fn record_drawn(&mut self, members: Vec<u32>, step: Step) {
        self.drawn.insert(0, DrawnBoundary { members, step });
        self.drawn.truncate(DRAWN_RETAINED);
    }

    pub fn write(&self, out: &mut String) {
        let mut object = Obj::new(out);
        {
            let mut authored = object.list("authored");
            for members in &self.authored {
                let mut entry = authored.object();
                let mut ids = entry.list("members");
                for member in members {
                    ids.int(i64::from(*member));
                }
                ids.end();
                entry.end();
            }
            authored.end();
        }
        {
            let mut drawn = object.list("drawn");
            for entry in &self.drawn {
                let mut written = drawn.object();
                {
                    let mut ids = written.list("members");
                    for member in &entry.members {
                        ids.int(i64::from(*member));
                    }
                    ids.end();
                }
                written.int("step", i64::from(entry.step));
                written.end();
            }
            drawn.end();
        }
        object.end();
    }
}

/// The material compartment that participates in the Field's causal rules.
///
/// `members` is the ascending set of Nodes physically contained by the
/// compartment. `leak_per_exposed_contact_per_step` is a raw [`Frac`]: each
/// distinct crossing contact earns this fraction of a member's held Charge per
/// step, capped at the whole. Neither value is derived from the active View.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PhysicalCompartment {
    pub members: Vec<u32>,
    pub leak_per_exposed_contact_per_step: Frac,
}

impl PhysicalCompartment {
    pub fn read(value: &Json, key: &str) -> Result<Self, Fault> {
        let found = read::map(value, key)?;
        read::exact_keys(
            found,
            key,
            &["leak_per_exposed_contact_per_step", "members"],
        )?;
        Ok(PhysicalCompartment {
            members: read::ids(found, "members", NODES_PER_RUN, i64::from(u32::MAX))?,
            leak_per_exposed_contact_per_step: read::int(
                found,
                "leak_per_exposed_contact_per_step",
                i64::MIN,
                i64::MAX,
            )?,
        })
    }

    pub fn write(&self, out: &mut String) {
        let mut object = Obj::new(out);
        object.int(
            "leak_per_exposed_contact_per_step",
            self.leak_per_exposed_contact_per_step,
        );
        {
            let mut members = object.list("members");
            for member in &self.members {
                members.int(i64::from(*member));
            }
            members.end();
        }
        object.end();
    }
}

/// One Node's share of the locked identity for one step, in raw `Fx`.
#[derive(Clone, Copy, Debug, Default)]
pub struct NodeLedger {
    pub node: u32,
    pub opening: Fx,
    pub inflow: Fx,
    pub outflow: Fx,
    pub upkeep: Fx,
    pub exogenous: Fx,
    pub closing: Fx,
}

impl NodeLedger {
    /// The identity's residual for this Node, which is exactly zero.
    pub fn residual(&self) -> Fx {
        self.closing - (self.opening + self.inflow - self.outflow - self.upkeep + self.exogenous)
    }
}

/// The whole step's Charge accounting: the stored total before and after, what
/// moved along Routes, and every named sink and source beside them.
#[derive(Clone, Debug, Default)]
pub struct Ledger {
    pub opening: Fx,
    pub closing: Fx,
    /// Charge moved along Routes. Route transfer is internal, so it leaves the
    /// stored total unchanged and never enters the residual.
    pub moved: Fx,
    /// Charge a Pulse gathered. A transfer like `moved`: it moves Charge from
    /// its sources to the emitting Form's Node and creates none, so it leaves
    /// the stored total unchanged and never enters the residual either.
    pub gathered: Fx,
    /// Charge paid as upkeep: a sink.
    pub upkeep: Fx,
    /// Charge removed by a layer's Drain: a sink.
    pub drain: Fx,
    /// Charge that escaped across the physical compartment's edge: a sink.
    pub leakage: Fx,
    /// Charge an overloaded Node shed from its excess: a sink.
    pub overload: Fx,
    /// Charge the currents delivered: a source. Only Charge a Node accepted is
    /// counted, because Charge a full Node refuses is never emitted.
    pub current: Fx,
    /// Charge a due Trail entry delivered: a source, counted the same way and
    /// at the step the entry comes due, never at the step it was left.
    pub wake: Fx,
    pub nodes: Vec<NodeLedger>,
}

impl Ledger {
    /// Every sink of the step: Charge that left the Field.
    pub fn sinks(&self) -> Fx {
        self.upkeep + self.drain + self.leakage + self.overload
    }

    /// Every source of the step: Charge that entered the Field.
    pub fn sources(&self) -> Fx {
        self.current + self.wake
    }

    /// The residual of the whole step, which is exactly zero: the stored total
    /// moves by what the sources gave less what the sinks took, and by nothing
    /// else.
    pub fn residual(&self) -> Fx {
        self.closing - self.opening - self.sources() + self.sinks()
    }

    pub fn balanced(&self) -> bool {
        self.residual() == 0 && self.nodes.iter().all(|node| node.residual() == 0)
    }
}

/// One Node's upkeep for one step, split across the five purposes.
#[derive(Clone, Copy, Debug)]
pub struct UpkeepRecord {
    pub node: u32,
    pub v: Fx,
    pub mix: [Fx; UPKEEP_PURPOSES],
}

/// The records one completed step carries, ascending, with zero-valued entries
/// left out exactly as the trace's encoding requires.
#[derive(Clone, Debug, Default)]
pub struct StepRecords {
    pub q: Vec<(u32, Fx)>,
    pub f: Vec<(u32, Fx)>,
    pub upkeep: Vec<UpkeepRecord>,
    pub e: Vec<(u32, Fx)>,
    pub z: Vec<u32>,
}

/// What one step produced: its records, its accounting, and the cues it
/// raised.
///
/// The cues are render-only. They are not part of the trace, not part of any
/// payload, and a replay's are dropped: what a regeneration reproduces is the
/// state, and a cue is a thing the surface says about a step rather than a
/// thing the step holds.
#[derive(Clone, Debug)]
pub struct StepOutcome {
    pub records: StepRecords,
    pub ledger: Ledger,
    pub cues: Vec<Cue>,
    /// What this step's staging did to the pressure list. `membership` is the
    /// locked mutation boundary: the caller that owns a window ends it, because
    /// no window may span a change no step can re-derive.
    pub staged: crate::pressure::Staged,
}

/// What a run of steps over one unchanging Field shape can be told in advance.
///
/// Three rules of a step read relations that a step cannot change: which place
/// a Route's two ends stand at, which Nodes stand in a current, and which
/// non-members a member is exposed to. Recomputing them per step is what the
/// live path does, and at one step per frame it costs nothing worth naming. A
/// windowed replay runs thousands of steps over one shape, and there the same
/// three passes are the whole cost — the Still Mode analysis budget's own
/// arithmetic says so, per rule.
///
/// So a replay prepares them once and reads them per step. **The only thing a
/// step moves is a Form**: [`mirror_form_nodes`] writes a position and a layer
/// onto the Nodes the Form list names and onto nothing else, no rule adds or
/// removes a Node, a Route, or a current, and a current's path never moves. So
/// everything a Node that no Form names takes part in is fixed for the life of
/// the cache, and everything a Form-moved Node takes part in is read again each
/// step. Nothing here is an approximation and nothing is dropped: the cached
/// pass and the uncached pass produce the same recipients, the same exposure,
/// and the same bytes, which is what the equality test on a caps fixture pins.
///
/// A cache is valid exactly while the Field's shape stands: the Node list, the
/// Route list, the current list, the non-moving positions, and the standing
/// physical-compartment membership it was built against. A commit changes the
/// shape, so a commit builds a fresh one; a replay never changes it, because every edit a replay applies
/// is applied before the first replayed step.
#[derive(Clone, Debug)]
pub struct StepCache {
    /// Per Route, in the Route list's order: the places its tail and head stand
    /// at, and none for a Route whose ends name no Node.
    route_ends: Vec<Option<(usize, usize)>>,
    /// Per Form, in the Form list's order: the place its Node stands at.
    form_places: Vec<Option<usize>>,
    /// The places of the Nodes a Form moves, ascending.
    moving: Vec<usize>,
    /// Per current: the places of the Nodes standing in it that cannot move.
    current_fixed: Vec<Vec<usize>>,
    /// Per current: the box outside which a Node cannot stand in it, as
    /// `(low x, low y, high x, high y)`. A moving Node outside the box is
    /// outside every path point's width, because a squared distance below
    /// `(width + 1)^2` puts each axis inside `width + 1` on its own.
    current_box: Vec<(Fx, Fx, Fx, Fx)>,
    /// The physical-compartment leakage pass, prepared.
    leak: LeakCache,
}

/// The leakage pass, prepared: the members, the exposure that cannot change,
/// and the neighbours that must be read again each step.
#[derive(Clone, Debug, Default)]
struct LeakCache {
    /// Member places, ascending — the order the rule pays in.
    members: Vec<usize>,
    /// Per member: how many distinct non-member neighbours are fixed.
    fixed: Vec<u32>,
    /// Per member: the non-member places whose adjacency is read again each
    /// step. None of them is counted in `fixed`, so no neighbour is counted
    /// twice.
    read_again: Vec<Vec<usize>>,
}

impl StepCache {
    /// Prepares the three passes for a Field, including its material
    /// compartment's leakage relations.
    pub fn of(field: &FieldState) -> Self {
        let count = field.ports.len();
        let route_ends = field
            .routes
            .iter()
            .map(|route| match (place_of(field, route.tail), place_of(field, route.head)) {
                (Some(tail), Some(head)) => Some((tail, head)),
                _ => None,
            })
            .collect();
        let form_places =
            field.forms.iter().map(|form| place_of(field, form.node)).collect::<Vec<_>>();
        let mut moves = vec![false; count];
        for place in form_places.iter().flatten() {
            moves[*place] = true;
        }
        let moving: Vec<usize> = (0..count).filter(|place| moves[*place]).collect();

        let mut current_fixed = Vec::with_capacity(field.currents.len());
        let mut current_box = Vec::with_capacity(field.currents.len());
        for index in 0..field.currents.len() {
            let current = &field.currents[index];
            current_fixed.push(
                (0..count)
                    .filter(|place| !moves[*place])
                    .filter(|place| stands_in(&field.ports[*place], current))
                    .collect(),
            );
            let (mut low_x, mut low_y) = (Fx::MAX, Fx::MAX);
            let (mut high_x, mut high_y) = (Fx::MIN, Fx::MIN);
            for point in &current.path {
                low_x = low_x.min(point.x);
                low_y = low_y.min(point.y);
                high_x = high_x.max(point.x);
                high_y = high_y.max(point.y);
            }
            let width = current.width.max(0);
            current_box.push((
                low_x.saturating_sub(width),
                low_y.saturating_sub(width),
                high_x.saturating_add(width),
                high_y.saturating_add(width),
            ));
        }

        StepCache {
            leak: prepare_leakage(field, &moves),
            route_ends,
            form_places,
            moving,
            current_fixed,
            current_box,
        }
    }

    /// The Nodes standing in one current, ascending by place: the fixed ones
    /// merged with the moved ones read again for this step.
    fn recipients_into(&self, field: &FieldState, index: usize, out: &mut Vec<usize>) {
        out.clear();
        let current = &field.currents[index];
        let (low_x, low_y, high_x, high_y) = self.current_box[index];
        let fixed = &self.current_fixed[index];
        out.reserve(fixed.len() + self.moving.len());
        let mut next = 0;
        for place in &self.moving {
            let run = next;
            while next < fixed.len() && fixed[next] < *place {
                next += 1;
            }
            out.extend_from_slice(&fixed[run..next]);
            let port = &field.ports[*place];
            // The box first, which no Node inside the width can be outside of,
            // and then the path itself for the ones it does not settle.
            if port.pos.x >= low_x
                && port.pos.x <= high_x
                && port.pos.y >= low_y
                && port.pos.y <= high_y
                && stands_in(port, current)
            {
                out.push(*place);
            }
        }
        out.extend_from_slice(&fixed[next..]);
    }
}

/// The recipient rule, for one Node and one current: the same vertex rule
/// [`standing_in`] states, read one Node at a time.
fn stands_in(port: &PortState, current: &CurrentState) -> bool {
    port.layer == current.layer
        && current
            .path
            .iter()
            .any(|point| within(port.pos, port.layer, *point, port.layer, current.width))
}

/// Splits every member's exposure into the part that cannot change and the
/// neighbours that must be read again each step.
///
/// A member and a non-member joined by a Route are joined for the life of the
/// cache, whichever of them moves, because no step adds or removes a Route.
/// Adjacency is a distance, so it is fixed exactly when neither end moves. The
/// two are kept apart by one bitset per member, so a neighbour reached both
/// ways is still counted once — the rule counts distinct non-member neighbours,
/// and that is what makes the cached exposure the same number as the uncached
/// one.
fn prepare_leakage(field: &FieldState, moves: &[bool]) -> LeakCache {
    let count = field.ports.len();
    let members: Vec<usize> = {
        let mut found: Vec<usize> = field
            .physical_compartment
            .members
            .iter()
            .filter_map(|node| place_of(field, *node))
            .collect();
        found.sort_unstable();
        found.dedup();
        found
    };
    if members.is_empty() || count == 0 {
        return LeakCache::default();
    }
    let mut is_member = vec![false; count];
    for place in &members {
        is_member[*place] = true;
    }
    let words = count.div_ceil(64);
    let mut fixed_bits = vec![0u64; members.len() * words];
    let mut slot_of = vec![usize::MAX; count];
    for (slot, place) in members.iter().enumerate() {
        slot_of[*place] = slot;
    }
    let mark = |slot: usize, other: usize, bits: &mut Vec<u64>| {
        bits[slot * words + other / 64] |= 1u64 << (other % 64);
    };
    for route in &field.routes {
        let (Some(tail), Some(head)) = (place_of(field, route.tail), place_of(field, route.head))
        else {
            continue;
        };
        if is_member[tail] && !is_member[head] {
            mark(slot_of[tail], head, &mut fixed_bits);
        }
        if is_member[head] && !is_member[tail] {
            mark(slot_of[head], tail, &mut fixed_bits);
        }
    }
    let mut fixed = Vec::with_capacity(members.len());
    let mut read_again = Vec::with_capacity(members.len());
    for (slot, place) in members.iter().enumerate() {
        let (pos, layer) = (field.ports[*place].pos, field.ports[*place].layer);
        if !moves[*place] {
            for other in 0..count {
                if is_member[other] || moves[other] {
                    continue;
                }
                if adjacent(pos, layer, field.ports[other].pos, field.ports[other].layer) {
                    mark(slot, other, &mut fixed_bits);
                }
            }
        }
        let held = &fixed_bits[slot * words..(slot + 1) * words];
        fixed.push(held.iter().map(|word| word.count_ones()).sum());
        // A member that moves has every non-member's adjacency to read again;
        // one that stands still has only the non-members that move. Neither
        // list holds a neighbour a Route already counted.
        read_again.push(
            (0..count)
                .filter(|other| !is_member[*other])
                .filter(|other| moves[*place] || moves[*other])
                .filter(|other| held[other / 64] & (1u64 << (other % 64)) == 0)
                .collect(),
        );
    }
    LeakCache { members, fixed, read_again }
}

/// Advances the Field one step under the control state the step consumes, the
/// [`Staging`] that carries the standing pressures and the stream.
///
/// The material compartment is part of the Field itself. The staged pressure
/// list is immutable in membership between the boundaries
/// [`crate::pressure`] names, with `stage` and `level` derived at the step being
/// replayed; it is not recorded per step and does not have to be.
///
/// The nine phases are the locked step order, and draws occur at the one
/// locked drawing point of version 1 — the Noise flow scale, at the Route
/// phase's start, layers ascending:
///
/// 1. the resolved depth change moves every controlled Form, and the same
///    control state sets every controlled Form's velocity, spring-damped;
/// 2. positions advance by velocity, which is declared in units per step;
/// 3. Form-kind Nodes take their Form's position and layer;
/// 4. the Pulse phase: charging and emission, from where the Form now stands,
///    so Charge an emission gathers can move along Routes in this same step;
/// 5. the Node phase, which pays upkeep — still reserved, because the
///    attribution of upkeep across its five locked purposes is locked nowhere,
///    and the trace cannot record an upkeep entry without it;
/// 6. the Route phase: one ascending pass of Route flow;
/// 7. the pressure phase: Drain, then compartment leakage, then overload decay,
///    each reading the Charge the previous rule left;
/// 8. the current phase: delivery, then every phase counter advances;
/// 9. the records and the ledger.
///
/// Delivery sits after the pressure phase, so Charge delivered this step is not
/// drained, leaked, or decayed until the next one.
pub fn advance(
    field: &mut FieldState,
    control: ControlState,
    pointer_speed: Frac,
    staging: &mut Staging<'_>,
) -> StepOutcome {
    advance_over(field, control, pointer_speed, staging, None)
}

/// The same step, over a [`StepCache`] prepared for this Field's causal shape.
///
/// It is the same rules reading the same relations, told rather than recomputed
/// — the results are identical by construction and pinned as identical by test.
/// A caller that replays a window uses this; a caller that runs one live step
/// uses [`advance`], because preparing the cache costs more than one step saves.
pub fn advance_cached(
    field: &mut FieldState,
    control: ControlState,
    pointer_speed: Frac,
    staging: &mut Staging<'_>,
    cache: &StepCache,
) -> StepOutcome {
    advance_over(field, control, pointer_speed, staging, Some(cache))
}

fn advance_over(
    field: &mut FieldState,
    control: ControlState,
    pointer_speed: Frac,
    staging: &mut Staging<'_>,
    cache: Option<&StepCache>,
) -> StepOutcome {
    field.step += 1;

    let mut ledger = Ledger {
        opening: field.ports.iter().map(|port| port.q).sum(),
        nodes: field
            .ports
            .iter()
            .map(|port| NodeLedger { node: port.node, opening: port.q, ..NodeLedger::default() })
            .collect(),
        ..Ledger::default()
    };
    let mut cues = Vec::new();

    change_depth(field, control.depth_move);
    steer_forms(field, control, pointer_speed);
    advance_positions(field);
    mirror_form_nodes(field, cache);
    // Every position and layer of the step stands settled here, so this is
    // where a linked Form's separation is read and where a Trail entry is left.
    let blocked = separated_places(field);
    deposit_trails(field);
    let pressed = pulse_phase(field, control, staging.pressures, &mut ledger, &mut cues);
    // The Node phase: every Node pays the upkeep it was authored with, before
    // Drain, so the two sinks never race for the last of what a Node holds.
    apply_upkeep(field, &mut ledger);
    // The Route phase reads the pressure list as the step opened — the
    // locked reading point for the Noise flow scale, drawn at the phase's
    // own start from the step's supplied stream, and for Flood's throttle
    // term. A stage turnover therefore reaches this phase one step later,
    // deterministically.
    let scales = pressure::noise_flow_scales(
        &field.layers,
        staging.pressures,
        staging.schedule,
        field.step,
        staging.stream,
    );
    let opened_press = pressure::FloodPress::opened(staging.pressures, staging.schedule, field.step);
    move_route_charge(field, &mut ledger, cache, &scales, &opened_press);
    // The pressure phase opens with the stage machine, so Drain, leakage,
    // the overload decay, and Current delivery all read the list as this
    // step staged it, and the Pulse of phase 4 read it as the previous step
    // left it — which is the list the frame the player pulsed at was written
    // from. The stage entries the machine reports carry the stage-entry
    // one-shots — Fracture's break, Flood's hold, Drift's move — which ride
    // the committed-change machinery between steps, applied by the caller.
    let mut staged = pressure::advance_pressures(staging.pressures, staging.schedule, field.step);
    staged.pressed = pressed;
    apply_drain(field, &mut ledger, staging.pressures);
    apply_leakage(field, &mut ledger, cache);
    decay_overload(field, &mut ledger, &pressure::FloodPress::of(staging.pressures));
    deliver_currents(
        field,
        &mut ledger,
        cache,
        &pressure::Redirection::of(staging.pressures),
        &blocked,
    );
    // The Trail entries that come due this step deliver after the currents
    // have, in the same phase and by the same split.
    deliver_trails(field, &mut ledger, &blocked);
    advance_currents(field);
    mirror_form_charge(field, cache);

    ledger.closing = field.ports.iter().map(|port| port.q).sum();
    for (entry, port) in ledger.nodes.iter_mut().zip(field.ports.iter()) {
        entry.closing = port.q;
    }
    debug_assert!(
        ledger.balanced(),
        "every step's Charge deltas sum to zero across the ledger, sinks included",
    );

    let records = record(field, &ledger);
    StepOutcome { records, ledger, cues, staged }
}

/// The Pulse phase: charging, and the emission a release makes.
///
/// It runs after the Form has moved and its Node has taken its position, so a
/// Pulse acts from where the Form now stands, and before the Route phase, so
/// Charge it gathers can move along Routes in the same step. Controlled Forms
/// are processed in ascending Form id, which the Form list is validated to be
/// in; a Form the player does not steer holds no control and neither charges
/// nor emits.
///
/// The three cases a step falls into are the locked ones: a release emits with
/// the charge as it stands and returns it to 0, a hold adds one step's share up
/// to the whole, and a step that does neither zeroes a partial charge — which
/// is how the locked focus-loss neutral frame discards a fumbled hold without
/// emitting anything.
fn pulse_phase(
    field: &mut FieldState,
    control: ControlState,
    pressures: &[PressureState],
    ledger: &mut Ledger,
    cues: &mut Vec<Cue>,
) -> Vec<(u8, crate::pressure::Displaced)> {
    let mut pressed: Vec<(u8, crate::pressure::Displaced)> = Vec::new();
    for index in 0..field.forms.len() {
        if !field.forms[index].controlled {
            continue;
        }
        // Focus is written by the hold and by nothing else, and changes no
        // dynamics: the steering response is the same held or not.
        field.forms[index].focus = control.pulse_held;

        if !control.pulse_release {
            let form = &mut field.forms[index];
            form.pulse_charge = if control.pulse_held {
                (form.pulse_charge + PULSE_CHARGE_STEP).min(FRAC_ONE)
            } else {
                0
            };
            continue;
        }

        // The emission consumes the charge as it stands; the increment never
        // applies on this step.
        let radius = pulse_radius(field.forms[index].pulse_charge);
        field.forms[index].pulse_charge = 0;
        let (node, pos, layer) =
            (field.forms[index].node, field.forms[index].pos, field.forms[index].layer);

        // Gathering, then port activation, then interference displacement.
        let gathered = gather_charge(field, node, pos, layer, radius, ledger);
        let opened = open_ports(field, pos, layer, radius);
        // The real list, since Goal 18: `RunState.pressures` is what stands
        // here, so a Pulse that reaches an active Interference pressure's
        // target takes a quarter off its level exactly as the locked contract
        // says. The reduction is the one mutation of the list a step takes that
        // no replay can derive from the step number alone, so a step that makes
        // one is a mutation boundary and the caller ends the window on it.
        let displaced = displace_interference(field, pressures, pos, layer, radius, &mut pressed);

        // Every cue an emission raises lands in this same step, which is the
        // locked signal that tells a Pulse that did something from one that did
        // nothing: an outcome cue beside the emission, or the emission alone.
        raise_cue(cues, Cue { kind: CUE_PULSE_EMITTED, a: reach_ticks(radius), b: node });
        if gathered > 0 {
            // The gathered total in 1/16-unit ticks, flooring and saturating.
            let ticks = (gathered >> 12).clamp(0, i64::from(u16::MAX)) as u16;
            raise_cue(cues, Cue { kind: CUE_CHARGE_GATHERED, a: ticks, b: node });
        }
        for port in opened {
            raise_cue(cues, Cue { kind: CUE_PORT_OPENED, a: 0, b: port });
        }
        if displaced > 0 {
            raise_cue(cues, Cue { kind: CUE_INTERFERENCE_PUSHED, a: displaced, b: node });
        }
    }
    pressed
}

/// Adds one cue, holding the frame's locked count.
///
/// The overflow drops the oldest cue of a kind other than the emission's own,
/// and an emission's cue only when nothing else stands: the frame's forms
/// record reads its reach from that cue, and a cue 1 beside an outcome cue is
/// the whole of the signal that tells a Pulse that did something from one that
/// did nothing, so dropping it would leave both unreadable.
pub fn raise_cue(cues: &mut Vec<Cue>, cue: Cue) {
    cues.push(cue);
    if cues.len() <= CUES_PER_FRAME {
        return;
    }
    let dropped = cues
        .iter()
        .position(|held| held.kind != CUE_PULSE_EMITTED)
        .unwrap_or(0);
    cues.remove(dropped);
}

/// Gathers a quarter of what every source within reach holds, into the emitting
/// Form's own Node.
///
/// Sources are the Nodes of kind `port`, `reserve`, and `module` — never a
/// Form-kind Node, so no Form takes from itself or from another Form — holding
/// Charge, within the reach under the locked distance rule, in ascending Node
/// order. `open` does not gate this: a closed Port gives Charge like any other
/// source, and the same emission may open it.
///
/// Each take is held to the emitting Node's remaining headroom as earlier
/// sources of the same emission have left it, and the pass stops the moment
/// that headroom is gone. The Charge moves as paired exogenous terms, so
/// FRAMEWORK.md's per-Node identity holds at both ends and the two sum to zero:
/// the ledger records the transfer total and touches no sink and no source.
fn gather_charge(
    field: &mut FieldState,
    node: u32,
    pos: Vec2,
    layer: u8,
    radius: Fx,
    ledger: &mut Ledger,
) -> Fx {
    let Some(destination) = place_of(field, node) else {
        debug_assert!(false, "a Form's Node is validated to stand in the Port list");
        return 0;
    };
    let mut total = 0;
    for place in 0..field.ports.len() {
        let source = &field.ports[place];
        if source.kind == NodeKind::Form || source.q <= 0 {
            continue;
        }
        if !within(pos, layer, source.pos, source.layer, radius) {
            continue;
        }
        let headroom = NODE_CHARGE_CAP - field.ports[destination].q;
        if headroom <= 0 {
            break;
        }
        let take = fixed_mul(field.ports[place].q, PULSE_GATHER_SHARE).min(headroom);
        if take <= 0 {
            continue;
        }
        field.ports[place].q -= take;
        field.ports[destination].q += take;
        ledger.nodes[place].exogenous -= take;
        ledger.nodes[destination].exogenous += take;
        ledger.gathered += take;
        total += take;
    }
    total
}

/// Opens every closed Port within reach, in ascending Node order.
///
/// No Charge is spent and no ledger entry is written: activation is a change of
/// participation, not of quantity. The base rule is one-way — nothing in
/// version 1 closes an open Port — and a Pulse that reaches no closed Port
/// raises no cue, which is what makes a failed release read as a release that
/// did nothing.
fn open_ports(field: &mut FieldState, pos: Vec2, layer: u8, radius: Fx) -> Vec<u32> {
    let mut opened = Vec::new();
    for port in field.ports.iter_mut() {
        if port.kind != NodeKind::Port || port.open {
            continue;
        }
        if !within(pos, layer, port.pos, port.layer, radius) {
            continue;
        }
        port.open = true;
        opened.push(port.node);
    }
    opened
}

/// Presses back every active interference-class pressure whose target stands
/// within reach, through its `displaced` floor, and answers how many were
/// pressed.
///
/// Targets `layer` and `none` are never displaceable, a `node` target is
/// pressed when that Node is within reach, and a `route` target when either
/// endpoint is. With `v` the pressure's effective level at the emission,
/// `displaced` becomes `{ the current stage, v - fixed_mul(v, 16384) }` — a
/// quarter off, flooring, never below 0 because the decrement never exceeds
/// `v`, and repeated Pulses press an already-displaced pressure further, each
/// from the current effective level. The write is a membership boundary: the
/// floor is collected here and written by the caller at the step boundary,
/// after the active window has ended, so no retained step is ever replayed
/// under a floor it did not run under. Displacement moves no Charge and is
/// outside the ledger.
pub fn displace_interference(
    field: &FieldState,
    pressures: &[PressureState],
    pos: Vec2,
    layer: u8,
    radius: Fx,
    pressed: &mut Vec<(u8, crate::pressure::Displaced)>,
) -> u16 {
    let reaches = |node: u32| -> bool {
        node_of(field, node)
            .is_some_and(|target| within(pos, layer, target.pos, target.layer, radius))
    };
    let mut reduced: u16 = 0;
    for pressure in pressures.iter() {
        if pressure.queued || pressure.pressure != Pressure::Interference {
            continue;
        }
        let named = pressure.target.id.unwrap_or_default();
        let within_reach = match pressure.target.kind {
            TargetKind::Node => reaches(named),
            TargetKind::Route => field
                .routes
                .iter()
                .find(|route| route.route == named)
                .is_some_and(|route| reaches(route.tail) || reaches(route.head)),
            TargetKind::None | TargetKind::Layer => false,
        };
        if !within_reach {
            continue;
        }
        // A later Pulse of the same emission batch presses from the floor an
        // earlier one collected, so repeated presses compound exactly as the
        // boundary will write them.
        let ordinal = pressure.pressure.ordinal();
        let held = pressed
            .iter()
            .rev()
            .find(|(other, _)| *other == ordinal)
            .map(|(_, floor)| {
                if floor.stage == pressure.stage {
                    pressure.level.min(floor.level)
                } else {
                    pressure.effective_level()
                }
            })
            .unwrap_or_else(|| pressure.effective_level());
        pressed.push((
            ordinal,
            crate::pressure::Displaced {
                stage: pressure.stage,
                level: held - fixed_mul(held, PULSE_DISPLACE_SHARE),
            },
        ));
        reduced = reduced.saturating_add(1);
    }
    reduced
}

/// Moves every controlled Form by the resolved depth change, held inside the
/// authored layer range, and takes every linked Form with it on the same step.
/// Retreat upward is always allowed, and a Field with no layers has no depth to
/// change.
///
/// A linked Form follows the layer its controlled Form ends the change on
/// rather than the change itself, so a move the range clamps clamps the set
/// together: the alternative is a descent that separates a Chorus by a layer's
/// own 512 units without the player having steered anywhere.
fn change_depth(field: &mut FieldState, depth_move: i8) {
    if depth_move == 0 || field.layers.is_empty() {
        return;
    }
    let shallowest = field.layers.first().expect("a nonempty layer list").layer;
    let deepest = field.layers.last().expect("a nonempty layer list").layer;
    for form in &mut field.forms {
        if !form.controlled {
            continue;
        }
        let wanted = i16::from(form.layer) + i16::from(depth_move);
        form.layer = wanted.clamp(i16::from(shallowest), i16::from(deepest)) as u8;
    }
    // The linked group follows its reference member's layer, which is the
    // controlled member's while the group holds one and its anchor's while it
    // does not: an uncontrolled Form takes no depth intent, so a group whose
    // control has been handed away stays where the anchor stands.
    let Some(reference) = reference_place(field) else {
        return;
    };
    let carried = field.forms[reference].layer;
    for place in 0..field.forms.len() {
        if place == reference || !in_group(field, place) {
            continue;
        }
        field.forms[place].layer = carried;
    }
}

/// The anchor of the linked group: the Form the group's authored offsets are
/// measured from, and none where the Field stands no linked Form at all.
///
/// A `linked_forms` ability stands its members up around the chapter's first
/// placement, and the identifiers it takes carry on past the highest the
/// chapter authored — so the anchor is the lowest Form identifier carrying no
/// link of its own. A chapter that places one Form and a selection that links
/// none both leave this reading exactly where it stood.
fn anchor_place(field: &FieldState) -> Option<usize> {
    if !field.forms.iter().any(|form| form.link.is_some()) {
        return None;
    }
    field
        .forms
        .iter()
        .enumerate()
        .filter(|(_, form)| form.link.is_none())
        .min_by_key(|(_, form)| form.id)
        .map(|(place, _)| place)
}

/// Whether one Form stands in the linked group: a member carrying the link, or
/// the anchor the members' offsets are measured from.
fn in_group(field: &FieldState, place: usize) -> bool {
    field.forms[place].link.is_some() || anchor_place(field) == Some(place)
}

/// The member of the linked group the rest of it follows: the controlled
/// member while the group holds one, and the group's anchor while it does not.
///
/// ARCHITECTURE.md's Handoff locks it this way so that a Handoff *inside* the
/// group re-anchors the formation without deforming it, and a Handoff *away*
/// from the group leaves it holding formation around its anchor while the
/// anchor coasts.
fn reference_place(field: &FieldState) -> Option<usize> {
    let anchor = anchor_place(field)?;
    if field.forms[anchor].controlled {
        return Some(anchor);
    }
    Some(
        field
            .forms
            .iter()
            .position(|form| form.controlled && form.link.is_some())
            .unwrap_or(anchor),
    )
}

/// The authored offset one group member holds from the group's anchor. The
/// anchor's own offset is (0, 0), because it is the placement every other
/// offset was measured from.
fn station_offset(field: &FieldState, place: usize) -> Vec2 {
    match field.forms[place].link {
        Some(link) => link.offset,
        None => Vec2::default(),
    }
}

/// The Nodes of linked Forms standing further from the member they follow than
/// their authored separation admits, as one flag per Port place.
///
/// Separation is measured to the **reference member**, so the flag stays
/// defined under every Handoff — and a Handoff to a distant Form can separate a
/// group on the spot, which is a consequence rather than a defect.
///
/// Separation is read after the kinematics phase, where every position and
/// layer of the step stands settled, and it costs delivery rather than Charge:
/// a separated Form's Node is passed over by every delivery source, and a Form
/// back inside its separation is delivered to again at once.
fn separated_places(field: &FieldState) -> Vec<bool> {
    let mut blocked = vec![false; field.ports.len()];
    for (place, apart) in separated_forms(field).iter().enumerate() {
        if !*apart {
            continue;
        }
        let node = field.forms[place].node;
        if let Some(at) = field.ports.iter().position(|port| port.node == node) {
            blocked[at] = true;
        }
    }
    blocked
}

/// Which Forms of the linked group stand separated from its reference member,
/// one flag per Form place.
///
/// This is the one reading: [`separated_places`] projects it onto the Port list
/// for the delivery rules, and the render snapshot's forms-record flags byte
/// carries the same bit. Keeping them one function is what makes the flag a
/// player sees and the delivery a Node is passed over for the same fact.
pub fn separated_forms(field: &FieldState) -> Vec<bool> {
    let mut apart = vec![false; field.forms.len()];
    let Some(reference) = reference_place(field) else {
        return apart;
    };
    // The group's authored separation: every member carries the same value, so
    // the reference member's own is the group's, and the anchor — which
    // carries no link — is measured against the reference member's.
    let Some(separation) = group_separation(field, reference) else {
        return apart;
    };
    let (held, on) = (field.forms[reference].pos, field.forms[reference].layer);
    for place in 0..field.forms.len() {
        if place == reference || !in_group(field, place) {
            continue;
        }
        let form = &field.forms[place];
        apart[place] = distance(held, on, form.pos, form.layer) > separation;
    }
    apart
}

/// Where the linked group's reference member stands, for a caller outside this
/// module that needs the same reading the delivery rules take.
pub fn reference_of(field: &FieldState) -> Option<(Vec2, u8)> {
    reference_place(field).map(|place| (field.forms[place].pos, field.forms[place].layer))
}

/// The separation the linked group stands under: the reference member's own
/// where it carries a link, and otherwise the first member's — every member of
/// one group carries the same authored value.
fn group_separation(field: &FieldState, reference: usize) -> Option<Fx> {
    if let Some(link) = field.forms[reference].link {
        return Some(link.separation);
    }
    field.forms.iter().find_map(|form| form.link.map(|link| link.separation))
}

/// The steering reach one Form stands under: the locked reach, the player's
/// configured scale, and the Form's own authored scale, composed at this one
/// flooring point and nowhere else.
fn scaled_reach(pointer_speed: Frac, steer_scale: Frac) -> Fx {
    fixed_mul(STEER_REACH, fixed_mul(pointer_speed, steer_scale))
}

/// The other half of the control phase: the control state this step consumes
/// sets every controlled Form's velocity, spring-damped, and phase 2 then
/// integrates it. The steering feel block above states the rule and its
/// numbers.
///
/// Only a controlled Form is steered. A Form the player does not steer keeps
/// the velocity it stands with, which is what makes it a drifting Form rather
/// than an unsteered one.
///
/// `pointer_speed` is the player's configured scale on the offset the control
/// names, raw [16384, 262144] around a default of 65536 — a quarter of the
/// reference speed to four times it. It is the one step input the trace does
/// not carry, and what keeps a regeneration exact is locked in
/// ARCHITECTURE.md's byte-equivalence contract rather than left to be inferred:
/// `input_config` is immutable for the life of a run in version 1, because no
/// command of the closed set changes it. The goal that adds a settings surface
/// — accessibility, PLAN.md Goal 31 — has the two paths that lock leaves, and
/// no third: end the active window on a change, or record the value each window
/// ran under.
fn steer_forms(field: &mut FieldState, control: ControlState, pointer_speed: Frac) {
    // The control crosses as Q1.15 over [-32767, 32767] and a raw `Frac` is
    // Q0.16, so widening one to the other is a single shift and exact. The
    // vector's own magnitude is at most 32767, which the shell holds it to and
    // the frame reader checks, so the offset below is inside the reach on every
    // heading rather than only on the axes.
    let released = control.steer_x == 0 && control.steer_y == 0;
    for place in 0..field.forms.len() {
        if !field.forms[place].controlled {
            continue;
        }
        let reach = scaled_reach(pointer_speed, field.forms[place].steer_scale);
        let named = |component: i16| fixed_mul(reach, Fx::from(component) << 1);
        spring(
            &mut field.forms[place],
            named(control.steer_x),
            named(control.steer_y),
            released,
        );
    }

    // Then the rest of the linked group, ascending Form id, each on the control
    // its own station names. The derived control is a pure function of the
    // state the controlled Forms have just left, so the recorded control stays
    // the player's alone and byte-equivalence is untouched.
    //
    // A station is the **reference member's** position plus the difference of
    // the two authored offsets, so a Handoff inside the group re-anchors the
    // formation without deforming it: with control on the anchor, whose own
    // offset is (0, 0), this is exactly the reading that stood before.
    let Some(reference) = reference_place(field) else {
        return uncontrolled_coast(field);
    };
    let anchor = field.forms[reference].pos;
    let held_offset = station_offset(field, reference);
    for place in 0..field.forms.len() {
        if place == reference {
            // The reference names no station of its own. While it carries
            // control the loop above has already steered it; while it does not
            // — control having moved away from the group, leaving the anchor
            // as reference — it consumes the same neutral control every other
            // uncontrolled Form does, and coasts to rest under the locked
            // damping. Skipping it outright froze its velocity instead, and the
            // formation followed it out of the field.
            if !field.forms[place].controlled {
                spring(&mut field.forms[place], 0, 0, true);
            }
            continue;
        }
        if !in_group(field, place) {
            // Outside the group, an uncontrolled Form consumes a neutral
            // control through this same spring-damper: it coasts to rest under
            // the locked damping and adds no kinematics of its own.
            if !field.forms[place].controlled {
                spring(&mut field.forms[place], 0, 0, true);
            }
            continue;
        }
        if field.forms[place].controlled {
            continue;
        }
        let offset = station_offset(field, place);
        let reach = scaled_reach(pointer_speed, field.forms[place].steer_scale);
        // A Form standing at its station names no control at all, so the rest
        // reads exactly as a released control does and the damper parks it.
        let named = |away: Fx| -> i16 {
            if reach <= 0 {
                return 0;
            }
            // Truncating toward zero, which is what integer division does and
            // what keeps a station approached from either side symmetric.
            ((away * 32768) / reach).clamp(-32767, 32767) as i16
        };
        let station = Vec2::new(
            anchor.x + offset.x - held_offset.x,
            anchor.y + offset.y - held_offset.y,
        );
        let steer_x = named(station.x - field.forms[place].pos.x);
        let steer_y = named(station.y - field.forms[place].pos.y);
        let held = |component: i16| fixed_mul(reach, Fx::from(component) << 1);
        spring(
            &mut field.forms[place],
            held(steer_x),
            held(steer_y),
            steer_x == 0 && steer_y == 0,
        );
    }
}

/// Every uncontrolled Form outside a linked group, consuming a neutral control
/// — steer (0, 0), no Pulse, no depth intent — through the same spring-damper.
///
/// It coasts to rest under the locked damping and adds no kinematics: this is
/// the whole of what an uncontrolled Form does when it stands in no formation,
/// and it is per Form rather than global, which is what lets a chapter place
/// several Forms and hand control among them.
fn uncontrolled_coast(field: &mut FieldState) {
    for place in 0..field.forms.len() {
        if field.forms[place].controlled {
            continue;
        }
        spring(&mut field.forms[place], 0, 0, true);
    }
}

/// One Form's velocity under one control: the locked spring against the locked
/// damper, and the rest floor a released control brings.
fn spring(form: &mut FormState, offset_x: Fx, offset_y: Fx, released: bool) {
    form.vel.x += fixed_mul(STEER_STIFFNESS, offset_x) - fixed_mul(STEER_DAMPING, form.vel.x);
    form.vel.y += fixed_mul(STEER_STIFFNESS, offset_y) - fixed_mul(STEER_DAMPING, form.vel.y);
    if released {
        if form.vel.x.abs() < STEER_REST {
            form.vel.x = 0;
        }
        if form.vel.y.abs() < STEER_REST {
            form.vel.y = 0;
        }
    }
}

/// Advances each Form's position by its velocity, which is declared in units
/// per step. A controlled Form's velocity is the steering phase's; a drifting
/// Form's is its own. The plane's locked width is held here, because a position
/// outside [0, 4096) units is outside the range the type declares.
fn advance_positions(field: &mut FieldState) {
    for form in &mut field.forms {
        form.pos.x = hold(form.pos.x + form.vel.x, 0, PLANE_SPAN - 1);
        form.pos.y = hold(form.pos.y + form.vel.y, 0, PLANE_SPAN - 1);
    }
}

/// Gives every Form-kind Node its Form's position and layer, which is what
/// makes a moving Form recompute its adjacency each step.
fn mirror_form_nodes(field: &mut FieldState, cache: Option<&StepCache>) {
    let placed: Vec<(u32, Vec2, u8)> =
        field.forms.iter().map(|form| (form.node, form.pos, form.layer)).collect();
    for (index, (node, pos, layer)) in placed.into_iter().enumerate() {
        let found = match cache {
            Some(held) => held.form_places[index],
            None => field.ports.iter().position(|port| port.node == node),
        };
        if let Some(place) = found {
            field.ports[place].pos = pos;
            field.ports[place].layer = layer;
        }
    }
}

/// Gives every Form the stored Charge of its own Node. The ledger's quantity is
/// the Node's `q`, because the framework's identity is stated over Nodes; a
/// Form's `charge` is that same stored Charge as its Form reads it.
fn mirror_form_charge(field: &mut FieldState, cache: Option<&StepCache>) {
    let held: Vec<Option<Fx>> = field
        .forms
        .iter()
        .enumerate()
        .map(|(index, form)| match cache {
            Some(cached) => cached.form_places[index].map(|place| field.ports[place].q),
            None => field
                .ports
                .iter()
                .find(|port| port.node == form.node)
                .map(|port| port.q),
        })
        .collect();
    for (form, found) in field.forms.iter_mut().zip(held) {
        if let Some(q) = found {
            form.charge = q;
        }
    }
}

/// Where a Node stands in the ascending Port list, which is also its place in
/// the ledger. The list is validated ascending, so this is a binary search.
fn place_of(field: &FieldState, node: u32) -> Option<usize> {
    field.ports.binary_search_by_key(&node, |port| port.node).ok()
}

/// Moves Charge along every Route, in one pass in ascending Route order.
///
/// Each Route reads the Charge the earlier Routes of the same pass have already
/// moved, so a chain of Routes in ascending order carries Charge several hops in
/// one step — the locked behavior, and what lets a closed circuit sustain
/// itself. Every term is exact integer arithmetic on raw values: the `open` gate
/// is the participation rule, `avail` is the source shortfall, `room` is the
/// destination's headroom under the stored-Charge cap, and the head's overload
/// halves the Route's capacity for this pass. Route transfer moves Charge and
/// never creates or destroys it, so it touches no sink and no source.
fn move_route_charge(
    field: &mut FieldState,
    ledger: &mut Ledger,
    cache: Option<&StepCache>,
    scales: &[Frac; LAYERS_PER_CHAPTER],
    press: &pressure::FloodPress,
) {
    for index in 0..field.routes.len() {
        let capacity = field.routes[index].capacity;
        let ends = match cache {
            Some(held) => held.route_ends[index],
            None => {
                let route = &field.routes[index];
                match (place_of(field, route.tail), place_of(field, route.head)) {
                    (Some(tail), Some(head)) => Some((tail, head)),
                    _ => None,
                }
            }
        };
        let Some((from, to)) = ends else {
            debug_assert!(false, "a Route's endpoints are validated to name Nodes");
            continue;
        };

        let moved = if !field.ports[from].open || !field.ports[to].open {
            0
        } else {
            // Tail overload never throttles: sending Charge away from a
            // congested Node stays allowed at full capacity, which is what lets
            // a circuit recover. Flood lowers the targeted head's threshold
            // for this test with the list as the step opened, and the Noise
            // flow scale of the tail's layer narrows the capacity; the three
            // compose by taking the smallest.
            let head = &field.ports[to];
            let throttled = if head.q > press.threshold(head.node, head.capacity) {
                capacity >> 1
            } else {
                capacity
            };
            let scaled = fixed_mul(capacity, scales[usize::from(field.ports[from].layer)]);
            let effective = throttled.min(scaled);
            let avail = field.ports[from].q;
            let room = NODE_CHARGE_CAP - field.ports[to].q;
            effective.min(avail).min(room).max(0)
        };

        field.routes[index].flow = moved;
        if moved == 0 {
            continue;
        }
        // A Route from a Node to itself takes Charge out and puts it back, which
        // the two records carry and the identity absorbs.
        field.ports[from].q -= moved;
        field.ports[to].q += moved;
        ledger.nodes[from].outflow += moved;
        ledger.nodes[to].inflow += moved;
        ledger.moved += moved;
    }
}

/// Leaks Charge across the material compartment's physical edge.
///
/// Every physical member that is a shell member — one with a
/// crossing Route or a declared adjacency to a non-member — loses the fraction
/// its exposure earns: `x(n)` distinct non-member neighbors at
/// `leak_per_exposed_contact_per_step` each, held at the whole of what it
/// holds. Because the rate never passes 65536, the flooring `fixed_mul` cannot
/// take more than the Node has, so no clamp stands between the two. An empty
/// compartment and one equal to the whole of N both leak nothing.
///
/// What leaks is recorded as part of the Node's exogenous term, which is how the
/// framework's Boundary Sufficiency already accounts for it.
fn apply_leakage(
    field: &mut FieldState,
    ledger: &mut Ledger,
    cache: Option<&StepCache>,
) {
    let leak_frac = field.physical_compartment.leak_per_exposed_contact_per_step;
    if leak_frac == 0
        || field.physical_compartment.members.is_empty()
        || field.ports.is_empty()
    {
        return;
    }
    if let Some(held) = cache {
        // The prepared pass: the exposure that cannot change, plus the
        // neighbours a moved Node put in or took out of reach this step.
        let leak = &held.leak;
        for (slot, place) in leak.members.iter().enumerate() {
            let (pos, layer) = (field.ports[*place].pos, field.ports[*place].layer);
            let mut exposure = leak.fixed[slot];
            for other in &leak.read_again[slot] {
                if adjacent(pos, layer, field.ports[*other].pos, field.ports[*other].layer) {
                    exposure += 1;
                }
            }
            if exposure == 0 {
                continue;
            }
            let rate = (i64::from(exposure) * leak_frac).min(FRAC_ONE);
            let port = &mut field.ports[*place];
            let leaked = fixed_mul(port.q, rate);
            if leaked == 0 {
                continue;
            }
            port.q -= leaked;
            ledger.nodes[*place].exogenous -= leaked;
            ledger.leakage += leaked;
        }
        return;
    }
    // A compartment may name a Node that has since vanished; intake reconciles
    // that, and a name that stands for nothing leaks nothing.
    let members: Vec<usize> = field
        .physical_compartment
        .members
        .iter()
        .filter_map(|node| place_of(field, *node))
        .collect();
    if members.is_empty() {
        return;
    }

    let mut is_member = vec![false; field.ports.len()];
    for place in &members {
        is_member[*place] = true;
    }

    // One bitset of distinct non-member neighbors per member, so a neighbor
    // reached by both a Route and an adjacency is counted once.
    let words = field.ports.len().div_ceil(64);
    let mut neighbors = vec![0u64; members.len() * words];
    let mut slot_of = vec![usize::MAX; field.ports.len()];
    for (slot, place) in members.iter().enumerate() {
        slot_of[*place] = slot;
    }
    let mark = |slot: usize, other: usize, neighbors: &mut Vec<u64>| {
        neighbors[slot * words + other / 64] |= 1u64 << (other % 64);
    };

    // A Route between a member and a non-member, in either direction.
    for route in &field.routes {
        let (Some(tail), Some(head)) = (place_of(field, route.tail), place_of(field, route.head))
        else {
            continue;
        };
        if is_member[tail] && !is_member[head] {
            mark(slot_of[tail], head, &mut neighbors);
        }
        if is_member[head] && !is_member[tail] {
            mark(slot_of[head], tail, &mut neighbors);
        }
    }

    // A declared adjacency between a member and a non-member.
    for (slot, place) in members.iter().enumerate() {
        let (pos, layer) = (field.ports[*place].pos, field.ports[*place].layer);
        for other in 0..field.ports.len() {
            if is_member[other] {
                continue;
            }
            if adjacent(pos, layer, field.ports[other].pos, field.ports[other].layer) {
                mark(slot, other, &mut neighbors);
            }
        }
    }

    // Ascending Node order: the compartment is ascending, and a Node's place in
    // the Port list ascends with its identifier.
    let mut ordered: Vec<(usize, usize)> = members.iter().copied().enumerate().collect();
    ordered.sort_by_key(|(_, place)| *place);
    for (slot, place) in ordered {
        let exposure: u32 =
            neighbors[slot * words..(slot + 1) * words].iter().map(|word| word.count_ones()).sum();
        if exposure == 0 {
            continue;
        }
        let rate = (i64::from(exposure) * leak_frac).min(FRAC_ONE);
        let port = &mut field.ports[place];
        let leak = fixed_mul(port.q, rate);
        debug_assert!(leak <= port.q, "a rate inside one cannot take more than is held");
        if leak == 0 {
            continue;
        }
        port.q -= leak;
        ledger.nodes[place].exogenous -= leak;
        ledger.leakage += leak;
    }
}

/// Sheds a quarter of every overloaded Node's excess, floored.
///
/// This runs after Drain and leakage, on the Charge those left. An excess below
/// four raw units is below the rule's resolution and stands. Recovery is
/// memoryless: the moment a Node holds no more than its threshold, nothing here
/// touches it again.
fn decay_overload(field: &mut FieldState, ledger: &mut Ledger, press: &pressure::FloodPress) {
    for (place, port) in field.ports.iter_mut().enumerate() {
        // Flood lowers the targeted Node's threshold for this rule with the
        // list as this step staged it; what the lowered threshold sheds flows
        // through the same `overload` sink, and no Charge is injected.
        let threshold = press.threshold(port.node, port.capacity);
        if port.q <= threshold {
            continue;
        }
        let decay = fixed_mul(port.q - threshold, OVERLOAD_DECAY);
        if decay == 0 {
            continue;
        }
        port.q -= decay;
        ledger.nodes[place].exogenous -= decay;
        ledger.overload += decay;
    }
}

/// Delivers each active current's Charge to the Nodes standing in it.
///
/// A current emits its `strength` scaled by its layer's `gain` — the locked
/// depth-scaling hook for rewards — split across its recipients with an exact
/// remainder, so the shares sum to what was emitted and nothing is lost to
/// rounding. Charge a full Node refuses is never emitted, so only what a Node
/// accepted enters the ledger. Nothing gates delivery on `open`, and Form-kind
/// Nodes receive like any other, which is how steering into a current gathers
/// Charge.
/// The Node phase: every Node pays the upkeep its own authored rate prices.
///
/// `pay = min(q(n), upkeep_rate(n))`, ascending NodeId, flooring and never
/// below zero — there is no debt, so a Node that cannot pay in full pays what
/// it holds and stands as having failed for the step, which is FRAMEWORK.md's
/// own could-not-pay branch of the failure indicator. The payment is a sink:
/// it enters the `upkeep` term of the Node's ledger and of the step's, and
/// never the exogenous term, so Self-Support and Upkeep Mix read it where they
/// have always looked for it.
fn apply_upkeep(field: &mut FieldState, ledger: &mut Ledger) {
    for (place, port) in field.ports.iter_mut().enumerate() {
        if port.upkeep_rate <= 0 {
            continue;
        }
        let pay = port.q.min(port.upkeep_rate);
        if pay <= 0 {
            continue;
        }
        port.q -= pay;
        ledger.nodes[place].upkeep += pay;
        ledger.upkeep += pay;
    }
}

/// Leaves a Trail entry for every Form whose authored ability declares one, on
/// the steps its period names, at the position and layer the Form now stands
/// at.
///
/// A deposit moves no Charge: nothing leaves the Form, nothing enters a Node,
/// and no ledger term is touched — which is what keeps the delivery's own two
/// terms inside the one step they apply on. The queue is bounded, and a
/// deposit past the cap drops the oldest entry standing, so a long run costs
/// exactly as much as a short one.
fn deposit_trails(field: &mut FieldState) {
    let step = field.step;
    let left: Vec<PendingTrail> = field
        .forms
        .iter()
        .filter_map(|form| {
            let trail = form.trail?;
            if trail.period == 0 || step % Step::from(trail.period) != 0 {
                return None;
            }
            Some(PendingTrail {
                form: form.id,
                layer: form.layer,
                pos: form.pos,
                due: step.saturating_add(Step::from(trail.delay)),
                magnitude: trail.magnitude,
            })
        })
        .collect();
    for entry in left {
        if field.pending.len() >= PENDING_TRAILS {
            field.pending.remove(0);
        }
        field.pending.push(entry);
    }
}

/// Delivers every Trail entry that comes due this step, and leaves the queue
/// holding only what is still standing.
///
/// A due entry gives its whole magnitude to the Nodes on its own layer inside
/// the reach its Form authored, split by the same remainder rule a current
/// delivers by — the base share to every recipient, one raw unit more to the
/// first of them in ascending NodeId — clamped by each recipient's headroom,
/// with Charge a full Node refuses never emitted. Ledger: the `wake` source
/// grows by what was accepted, at this step and no other. An entry that comes
/// due where nothing stands delivers nothing and leaves the queue all the
/// same.
fn deliver_trails(field: &mut FieldState, ledger: &mut Ledger, blocked: &[bool]) {
    if field.pending.is_empty() {
        return;
    }
    let step = field.step;
    let due: Vec<PendingTrail> =
        field.pending.iter().copied().filter(|entry| entry.due <= step).collect();
    field.pending.retain(|entry| entry.due > step);
    for entry in due {
        let Some(reach) = field
            .forms
            .iter()
            .find(|form| form.id == entry.form)
            .and_then(|form| form.trail)
            .map(|trail| trail.radius)
        else {
            continue;
        };
        let standing: Vec<usize> = field
            .ports
            .iter()
            .enumerate()
            .filter(|(place, port)| {
                !blocked[*place]
                    && port.layer == entry.layer
                    && within(port.pos, port.layer, entry.pos, entry.layer, reach)
            })
            .map(|(place, _)| place)
            .collect();
        if standing.is_empty() || entry.magnitude <= 0 {
            continue;
        }
        let count = standing.len() as i64;
        let base = entry.magnitude / count;
        let remainder = entry.magnitude % count;
        for (order, place) in standing.into_iter().enumerate() {
            let share = base + i64::from((order as i64) < remainder);
            let port = &mut field.ports[place];
            let delivered = share.min(NODE_CHARGE_CAP - port.q);
            if delivered <= 0 {
                continue;
            }
            port.q += delivered;
            ledger.nodes[place].exogenous += delivered;
            ledger.wake += delivered;
        }
    }
}

fn deliver_currents(
    field: &mut FieldState,
    ledger: &mut Ledger,
    cache: Option<&StepCache>,
    redirection: &pressure::Redirection,
    blocked: &[bool],
) {
    let mut gain_of = [0 as Frac; LAYERS_PER_CHAPTER];
    for layer in &field.layers {
        gain_of[usize::from(layer.layer)] = layer.gain;
    }
    // An active Interference pressure gives its target Node first claim on
    // every same-layer current's emission — the competing path. The target
    // need not stand inside the current's width: that distance is the
    // redirection.
    let seized = redirection.node.and_then(|node| place_of(field, node));
    // One buffer for the whole phase, refilled per current rather than
    // allocated per current.
    let mut standing: Vec<usize> = Vec::new();

    for index in 0..field.currents.len() {
        let (active, layer, strength) = {
            let current = &field.currents[index];
            (current.active, current.layer, current.strength)
        };
        if !active {
            continue;
        }
        let emitted = fixed_mul(strength, gain_of[usize::from(layer)]);
        if emitted == 0 {
            continue;
        }

        // The redirected share leaves first, delivered against the target's
        // headroom — short headroom refuses the rest, never emitted, which is
        // the delivery rule's own clamp — and the remainder splits among the
        // geometric recipients by the locked remainder rule. Everything stays
        // inside the `current` source and delivery's exact conservation.
        let mut split = emitted;
        if let Some(place) = seized {
            if field.ports[place].layer == layer {
                let redirected = fixed_mul(emitted, redirection.share);
                split = emitted - redirected;
                // A separated linked Form's Node accepts nothing, the
                // redirected share included: it refuses exactly as short
                // headroom refuses, and refused Charge is never emitted.
                let port = &mut field.ports[place];
                let delivered = match blocked[place] {
                    true => 0,
                    false => redirected.min(NODE_CHARGE_CAP - port.q),
                };
                if delivered > 0 {
                    port.q += delivered;
                    ledger.nodes[place].exogenous += delivered;
                    ledger.current += delivered;
                }
            }
        }
        if split == 0 {
            continue;
        }

        match cache {
            Some(held) => held.recipients_into(field, index, &mut standing),
            None => standing = standing_in(field, index),
        }
        // A separated Node is not a recipient, so the current simply delivers
        // to fewer of them and the split is taken over what is left.
        standing.retain(|place| !blocked[*place]);
        if standing.is_empty() {
            continue;
        }

        // The split: every recipient takes the base share, and the first
        // `remainder` of them in ascending Node order take one raw unit more.
        let count = standing.len() as i64;
        let base = split / count;
        let remainder = split % count;
        for (order, place) in standing.iter().copied().enumerate() {
            let share = base + i64::from((order as i64) < remainder);
            let port = &mut field.ports[place];
            let delivered = share.min(NODE_CHARGE_CAP - port.q);
            if delivered <= 0 {
                continue;
            }
            port.q += delivered;
            ledger.nodes[place].exogenous += delivered;
            ledger.current += delivered;
        }
    }
}

/// Where the Nodes standing in one current sit in the Port list, ascending.
///
/// The recipient rule is the vertex rule: a Node stands in the current when its
/// distance to the nearest path point is at most the current's width, never
/// measured to the segment between two points. "The nearest point is within the
/// width" is the same statement as "some point is within the width", so the scan
/// stops at the first one.
fn standing_in(field: &FieldState, current: usize) -> Vec<usize> {
    let current = &field.currents[current];
    field
        .ports
        .iter()
        .enumerate()
        .filter(|(_, port)| port.layer == current.layer)
        .filter(|(_, port)| {
            current
                .path
                .iter()
                .any(|point| within(port.pos, port.layer, *point, port.layer, current.width))
        })
        .map(|(place, _)| place)
        .collect()
}

/// Removes each layer's Drain from every Node at that depth, floored by what
/// the Node holds, because stored Charge is never negative. Drain is a
/// pressure, so what it removes is part of the Node's exogenous term.
fn apply_drain(field: &mut FieldState, ledger: &mut Ledger, pressures: &[PressureState]) {
    let mut drain_of = [0 as Fx; LAYERS_PER_CHAPTER];
    for layer in &field.layers {
        // An active Drain pressure scales the targeted layer's loss —
        // `drain + fixed_mul(drain, level_eff)`, at most double — through
        // this same rule and into the same single sink; other layers are
        // untouched.
        drain_of[usize::from(layer.layer)] =
            pressure::drained(layer.layer, layer.drain, pressures);
    }
    for (entry, port) in ledger.nodes.iter_mut().zip(field.ports.iter_mut()) {
        let wanted = drain_of[usize::from(port.layer)];
        let removed = wanted.min(port.q);
        if removed == 0 {
            continue;
        }
        port.q -= removed;
        entry.exogenous -= removed;
        ledger.drain += removed;
    }
}

/// Advances every current's phase one step, modulo its period.
fn advance_currents(field: &mut FieldState) {
    for current in &mut field.currents {
        debug_assert!(current.period >= 1, "a current's period is validated nonzero");
        current.phase = (current.phase + 1) % current.period;
    }
}

/// Builds the step's records: ascending, and never a zero-valued entry.
fn record(field: &FieldState, ledger: &Ledger) -> StepRecords {
    // The lists are sized to what the step can hold rather than grown into,
    // because a windowed replay builds one of these per replayed step and the
    // regrowth is the whole of what that costs.
    let mut records = StepRecords {
        q: Vec::with_capacity(field.ports.len()),
        e: Vec::with_capacity(field.ports.len()),
        f: Vec::with_capacity(field.routes.len()),
        ..StepRecords::default()
    };
    for (entry, port) in ledger.nodes.iter().zip(field.ports.iter()) {
        if port.q != 0 {
            records.q.push((port.node, port.q));
        }
        if entry.exogenous != 0 {
            records.e.push((port.node, entry.exogenous));
        }
        // What the Node paid, attributed across the five purposes. Version 1
        // attributes the whole payment to the first of them, the boundary,
        // which is what the locked rule states and what Upkeep Mix reads; a
        // richer split is authored data a later goal adds. A Node that paid
        // nothing writes no entry, exactly as the zero-omission rule requires.
        if entry.upkeep != 0 {
            let mut mix = [0 as Fx; UPKEEP_PURPOSES];
            mix[0] = entry.upkeep;
            records.upkeep.push(UpkeepRecord { node: port.node, v: entry.upkeep, mix });
        }
        // The failure indicator is 1 exactly when the Node ends the step with
        // no stored Charge or could not pay its full upkeep — the second
        // branch being real since the Node phase pays.
        if port.q == 0 || entry.upkeep < port.upkeep_rate {
            records.z.push(port.node);
        }
    }
    for route in &field.routes {
        if route.flow != 0 {
            records.f.push((route.route, route.flow));
        }
    }
    records
}

/// True when two Nodes of the Field are adjacent under the one locked rule.
/// Nothing stores adjacency: it is computed from current positions, so a Form
/// that moves recomputes its own each step.
pub fn nodes_adjacent(field: &FieldState, first: u32, second: u32) -> bool {
    match (node_of(field, first), node_of(field, second)) {
        (Some(a), Some(b)) => adjacent(a.pos, a.layer, b.pos, b.layer),
        _ => false,
    }
}

/// The locked distance between two Nodes of the Field.
pub fn nodes_distance(field: &FieldState, first: u32, second: u32) -> Option<Fx> {
    let a = node_of(field, first)?;
    let b = node_of(field, second)?;
    Some(distance(a.pos, a.layer, b.pos, b.layer))
}

/// Every Node adjacent to the named one, ascending. Recomputed on every call,
/// which is what the moving-Form rule requires.
pub fn adjacency_of(field: &FieldState, node: u32) -> Vec<u32> {
    let Some(from) = node_of(field, node) else {
        return Vec::new();
    };
    field
        .ports
        .iter()
        .filter(|port| port.node != node && adjacent(from.pos, from.layer, port.pos, port.layer))
        .map(|port| port.node)
        .collect()
}

fn node_of(field: &FieldState, node: u32) -> Option<&PortState> {
    field.ports.iter().find(|port| port.node == node)
}

/// A cap crossing, in the shape the caps rule locks: the `capacity` envelope
/// naming the quantity and its cap.
pub fn cap_fault(quantity: &str, cap: i64) -> Fault {
    let mut detail = String::new();
    let mut object = Obj::new(&mut detail);
    object.int("cap", cap);
    object.text("quantity", quantity);
    object.end();
    Fault::detailed(Code::Capacity, detail)
}

/// Holds a quantity to a row of the locked capacity table, where a crossing is
/// the `capacity` envelope rather than a validation fault.
fn capped(value: Fx, cap: Fx, quantity: &str) -> Result<(), Fault> {
    if (0..=cap).contains(&value) {
        Ok(())
    } else {
        Err(cap_fault(quantity, cap))
    }
}

/// An identifier naming nothing.
fn missing(field: &str, id: u32) -> Fault {
    let mut detail = String::new();
    let mut object = Obj::new(&mut detail);
    object.int("id", i64::from(id));
    object.text("quantity", field);
    object.end();
    Fault::detailed(Code::NotFound, detail)
}

fn ranged(value: Fx, low: Fx, high: Fx, field: &str) -> Result<(), Fault> {
    if (low..=high).contains(&value) {
        Ok(())
    } else {
        Err(Fault::field(field))
    }
}

/// True when the identifiers are strictly ascending, which every
/// identifier-keyed list of the Field is.
fn ascending<T: Ord + Copy>(ids: &[T]) -> bool {
    ids.windows(2).all(|pair| pair[0] < pair[1])
}

/// Validates a whole Field against every locked cap, range, closed set, and
/// ordering rule. A crossed cap is the `capacity` envelope, an identifier
/// naming nothing is `not_found`, and everything else is `validation`.
pub fn validate(field: &FieldState) -> Result<(), Fault> {
    if field.layers.len() > LAYERS_PER_CHAPTER {
        return Err(cap_fault("layers_per_chapter", LAYERS_PER_CHAPTER as i64));
    }
    if field.ports.len() > NODES_PER_RUN {
        return Err(cap_fault("nodes_per_run", NODES_PER_RUN as i64));
    }
    if field.routes.len() > ROUTES_PER_RUN {
        return Err(cap_fault("routes_per_run", ROUTES_PER_RUN as i64));
    }
    if field.currents.len() > CURRENTS_PER_CHAPTER {
        return Err(cap_fault("currents_per_chapter", CURRENTS_PER_CHAPTER as i64));
    }
    if field.forms.len() > FORMS_PER_RUN {
        return Err(cap_fault("forms_per_run", FORMS_PER_RUN as i64));
    }
    if field.boundaries.drawn.len() > DRAWN_RETAINED {
        return Err(cap_fault("drawn_boundaries_retained", DRAWN_RETAINED as i64));
    }
    // The physical leakage parameter is a range rather than a capacity row.
    ranged(
        field.physical_compartment.leak_per_exposed_contact_per_step,
        0,
        LEAK_FRAC_CAP,
        "leak_per_exposed_contact_per_step",
    )?;

    // The layer list is contiguous from 0: a chapter declaring n layers declares
    // exactly the layers 0 through n − 1. A depth change moves a Form one layer
    // at a time, so a gap would put a Form on a layer that does not stand.
    let layer_ids: Vec<u8> = field.layers.iter().map(|layer| layer.layer).collect();
    let contiguous: Vec<u8> = (0..field.layers.len() as u8).collect();
    if layer_ids != contiguous {
        return Err(Fault::field("layers"));
    }
    for layer in &field.layers {
        if layer.layer > MAX_LAYER {
            return Err(Fault::field("layer"));
        }
        ranged(layer.drain, 0, STORED_BOUND - 1, "drain")?;
        ranged(layer.noise, 0, FRAC_ONE, "noise")?;
        ranged(layer.gain, 0, FRAC_ONE, "gain")?;
        if !ascending(&layer.current_ids) || !ascending(&layer.port_ids) {
            return Err(Fault::field("layer"));
        }
    }

    let node_ids: Vec<u32> = field.ports.iter().map(|port| port.node).collect();
    if !ascending(&node_ids) {
        return Err(Fault::field("ports"));
    }
    if field.physical_compartment.members.len() > NODES_PER_RUN {
        return Err(cap_fault("nodes_per_run", NODES_PER_RUN as i64));
    }
    if !ascending(&field.physical_compartment.members) {
        return Err(Fault::field("physical_compartment"));
    }
    if !field.ports.is_empty() && field.physical_compartment.members.is_empty() {
        return Err(Fault::field("physical_compartment"));
    }
    for member in &field.physical_compartment.members {
        if !node_ids.contains(member) {
            return Err(missing("node", *member));
        }
    }
    for port in &field.ports {
        if port.node == 0 || i64::from(port.node) >= i64::from(field.next_node_id) {
            return Err(Fault::field("node"));
        }
        if !layer_ids.contains(&port.layer) {
            return Err(missing("layer", u32::from(port.layer)));
        }
        ranged(port.pos.x, 0, PLANE_SPAN - 1, "pos")?;
        ranged(port.pos.y, 0, PLANE_SPAN - 1, "pos")?;
        capped(port.q, NODE_CHARGE_CAP, "stored_charge_per_node")?;
        ranged(port.upkeep_rate, 0, STORED_BOUND - 1, "upkeep_rate")?;
        // The overload threshold is a Charge, so the stored-Charge cap bounds
        // it. Stored Charge above it is what overload means, and is allowed.
        capped(port.capacity, NODE_CHARGE_CAP, "stored_charge_per_node")?;
    }

    let route_ids: Vec<u32> = field.routes.iter().map(|route| route.route).collect();
    if !ascending(&route_ids) {
        return Err(Fault::field("routes"));
    }
    for route in &field.routes {
        if route.route == 0 || i64::from(route.route) >= i64::from(field.next_route_id) {
            return Err(Fault::field("route"));
        }
        for end in [route.tail, route.head] {
            if !node_ids.contains(&end) {
                return Err(missing("node", end));
            }
        }
        capped(route.capacity, ROUTE_CAPACITY_CAP, "route_capacity")?;
        ranged(route.flow, 0, route.capacity, "flow")?;
        if route.formed_step > field.step {
            return Err(Fault::field("formed_step"));
        }
    }

    let form_ids: Vec<u8> = field.forms.iter().map(|form| form.id).collect();
    if !ascending(&form_ids) {
        return Err(Fault::field("forms"));
    }
    for form in &field.forms {
        if !FORMS.contains(&form.form.as_str()) {
            return Err(Fault::field("form"));
        }
        // A Node is one thing, so no two Forms stand on the same one.
        if field.forms.iter().filter(|other| other.node == form.node).count() > 1 {
            return Err(Fault::field("node"));
        }
        let Some(node) = node_of(field, form.node) else {
            return Err(missing("node", form.node));
        };
        if node.kind != NodeKind::Form {
            return Err(Fault::field("kind"));
        }
        // A Form and its Node are one placed thing: the Node mirrors the Form.
        if node.layer != form.layer || node.pos != form.pos || node.q != form.charge {
            return Err(Fault::field("node"));
        }
        ranged(form.vel.x, -(STORED_BOUND - 1), STORED_BOUND - 1, "vel")?;
        ranged(form.vel.y, -(STORED_BOUND - 1), STORED_BOUND - 1, "vel")?;
        ranged(form.reserve, 0, NODE_CHARGE_CAP, "reserve")?;
        ranged(form.pulse_charge, 0, FRAC_ONE, "pulse_charge")?;
        ranged(form.route_reach, 0, STORED_BOUND - 1, "route_reach")?;
        if form.forecast_depth > FORECAST_DEPTH_CAP {
            return Err(cap_fault("forecast_depth", i64::from(FORECAST_DEPTH_CAP)));
        }
        ranged(form.steer_scale, STEER_SCALE_LOW, STEER_SCALE_HIGH, "steer_scale")?;
        capped(form.route_capacity, ROUTE_CAPACITY_CAP, "route_capacity")?;
        if let Some(link) = form.link {
            // An offset is a displacement on the plane, so the plane's own
            // width bounds it either way; where it lands is the position rule's
            // business and is held there.
            ranged(link.offset.x, -(PLANE_SPAN - 1), PLANE_SPAN - 1, "offset")?;
            ranged(link.offset.y, -(PLANE_SPAN - 1), PLANE_SPAN - 1, "offset")?;
            ranged(link.separation, 0, STORED_BOUND - 1, "separation")?;
        }
        if let Some(trail) = form.trail {
            if !TRAIL_PERIOD.contains(&trail.period) {
                return Err(Fault::field("period"));
            }
            if !TRAIL_DELAY.contains(&trail.delay) {
                return Err(Fault::field("delay"));
            }
            capped(trail.radius, TRAIL_RADIUS_CAP, "radius")?;
            capped(trail.magnitude, TRAIL_MAGNITUDE_CAP, "magnitude")?;
        }
    }

    // The Trail entries standing: bounded, placed, and still to come. An entry
    // due at or before the step this Field stands at is one this build cannot
    // have written — a due entry delivers and leaves the queue on its own step
    // — so a payload carrying one is refused rather than delivered late.
    if field.pending.len() > PENDING_TRAILS {
        return Err(cap_fault("pending", PENDING_TRAILS as i64));
    }
    for entry in &field.pending {
        if !field.forms.iter().any(|form| form.id == entry.form) {
            return Err(missing("form", u32::from(entry.form)));
        }
        if !layer_ids.contains(&entry.layer) {
            return Err(missing("layer", u32::from(entry.layer)));
        }
        ranged(entry.pos.x, 0, PLANE_SPAN - 1, "pos")?;
        ranged(entry.pos.y, 0, PLANE_SPAN - 1, "pos")?;
        capped(entry.magnitude, TRAIL_MAGNITUDE_CAP, "magnitude")?;
        if entry.due <= field.step {
            return Err(Fault::field("due"));
        }
    }

    let current_ids: Vec<u16> = field.currents.iter().map(|current| current.id).collect();
    if !ascending(&current_ids) {
        return Err(Fault::field("currents"));
    }
    for current in &field.currents {
        if !layer_ids.contains(&current.layer) {
            return Err(missing("layer", u32::from(current.layer)));
        }
        if !CURRENT_PATH_POINTS.contains(&current.path.len()) {
            return Err(Fault::field("path"));
        }
        for point in &current.path {
            ranged(point.x, 0, PLANE_SPAN - 1, "path")?;
            ranged(point.y, 0, PLANE_SPAN - 1, "path")?;
        }
        ranged(current.width, 0, STORED_BOUND - 1, "width")?;
        capped(current.strength, CURRENT_STRENGTH_CAP, "current_strength")?;
        if current.period == 0 || current.phase >= current.period {
            return Err(Fault::field("period"));
        }
    }

    // A layer's lists name what stands on it: its currents, and its Ports other
    // than the Form-kind Nodes, which carry their own layer and move between
    // them.
    for layer in &field.layers {
        let standing: Vec<u32> = field
            .ports
            .iter()
            .filter(|port| port.layer == layer.layer && port.kind != NodeKind::Form)
            .map(|port| port.node)
            .collect();
        if layer.port_ids != standing {
            return Err(Fault::field("port_ids"));
        }
        let flowing: Vec<u16> = field
            .currents
            .iter()
            .filter(|current| current.layer == layer.layer)
            .map(|current| current.id)
            .collect();
        if layer.current_ids != flowing {
            return Err(Fault::field("current_ids"));
        }
    }

    for entry in &field.boundaries.drawn {
        if entry.members.is_empty() || !ascending(&entry.members) {
            return Err(Fault::field("members"));
        }
        if entry.step > field.step {
            return Err(Fault::field("step"));
        }
    }
    for members in &field.boundaries.authored {
        if members.is_empty() || !ascending(members) {
            return Err(Fault::field("members"));
        }
    }

    Ok(())
}

/// Checks what a Field must hold at the moment it is established, beyond what
/// [`validate`] holds for its whole life: Node kinds other than `port` are
/// established open, because the `open` gate of the Route flow rule is the
/// participation rule and only a Port waits to be opened.
pub fn establishable(field: &FieldState) -> Result<(), Fault> {
    for port in &field.ports {
        if port.kind != NodeKind::Port && !port.open {
            return Err(Fault::field("open"));
        }
    }
    Ok(())
}

/// The resolution ladder, which every View's grain stands on.
pub const RESOLUTION_LADDER: [u16; 9] = [1, 2, 4, 8, 16, 32, 64, 128, 256];

/// The declared window's range, in steps.
pub const WINDOW_RANGE: std::ops::RangeInclusive<u16> = 1..=60;

/// Checks any passive View against the Field it observes.
///
/// An empty inside is a valid cleared observation: it selects no Node and has no
/// physical consequence. Nonempty member lists remain ascending and every
/// member must name a Node. Resolution and window are valid whether or not the
/// observation currently selects anything, so a cleared View can be saved and
/// restored without losing the rest of its measurement protocol.
pub fn validate_view(
    view: &crate::state::ViewDeclaration,
    field: &FieldState,
) -> Result<(), Fault> {
    if !ascending(&view.inside) {
        return Err(Fault::field("inside"));
    }
    for member in &view.inside {
        if !field.ports.iter().any(|port| port.node == *member) {
            return Err(missing("node", *member));
        }
    }
    if !RESOLUTION_LADDER.contains(&view.resolution) {
        return Err(cap_fault("resolution", i64::from(*RESOLUTION_LADDER.last().expect("a ladder"))));
    }
    if !WINDOW_RANGE.contains(&view.window) {
        return Err(cap_fault("declared_window", i64::from(*WINDOW_RANGE.end())));
    }
    Ok(())
}

/// Checks the View a chapter opens with against the Field it declares an inside
/// of. Chapter authors must provide a nonempty opening selection even though a
/// player may later clear that passive selection. At establishment nothing has
/// vanished yet, so every member names a Node.
///
/// Resolution and the declared window restate rows of the locked capacity table,
/// so crossing either is the `capacity` envelope; `inside` has no row of its own
/// and fails as `validation`.
pub fn establishable_view(
    view: &crate::state::ViewDeclaration,
    field: &FieldState,
) -> Result<(), Fault> {
    if view.inside.is_empty() {
        return Err(Fault::field("inside"));
    }
    validate_view(view, field)
}

/// Checks the caps a step could cross, which nothing in a valid Field's step
/// may leave: every stored quantity inside its locked cap.
pub fn within_caps(field: &FieldState) -> bool {
    field.ports.iter().all(|port| (0..=NODE_CHARGE_CAP).contains(&port.q))
        && field.routes.iter().all(|route| {
            (0..=route.capacity).contains(&route.flow) && route.capacity <= ROUTE_CAPACITY_CAP
        })
        && field.forms.iter().all(|form| {
            (0..PLANE_SPAN).contains(&form.pos.x)
                && (0..PLANE_SPAN).contains(&form.pos.y)
                && (0..=NODE_CHARGE_CAP).contains(&form.charge)
                && (0..=FRAC_ONE).contains(&form.pulse_charge)
                && form.layer <= MAX_LAYER
        })
        && field.currents.iter().all(|current| current.phase < current.period)
}
