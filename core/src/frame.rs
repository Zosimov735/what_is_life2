//! The compact render snapshot the worker sends with `frame` events.
//!
//! `docs/field-framework/ARCHITECTURE.md` locks this buffer field by field:
//! a 32-byte header at frozen offsets, a section table of 8-byte entries from
//! offset 32, and one locked record layout per section kind. It is
//! little-endian binary rather than canonical JSON, and it is render-only —
//! produced from fixed-point state by one-way lossy conversion, and never read
//! back into the simulation. Nothing here is authoritative, and nothing here
//! drives a rule.
//!
//! The compactness rule is the document's: a frame carries only what the
//! renderer and the chrome need every frame. Slate records, privilege values,
//! coordinates, perturbation results, boundary lists, the trajectory, and save
//! payloads cross on demand through command responses and `review_ready`
//! events, never here.
//!
//! One of the ten section kinds stands on state a later goal owns — the
//! camera — so this encoder writes the nine that stand on state the run
//! already holds: Forms, Ports, Routes, currents, the active observation
//! View's bitset, the staged pressures, the cues the step raised, the Still
//! Mode overlay, and the flat point array the currents' paths index into.
//! Physical-compartment membership and its queued preview ride as independent
//! Port flags; the View never supplies either of them.
//!
//! The overlay is the one section whose presence is a mode rather than a
//! count: the document says it is present only in `still`, so it is written
//! there and nowhere else, carrying however many window steps the standing
//! candidate's baseline envelope holds. A window the clamp left empty leaves
//! it empty, and an empty section is the honest reading of a surface that
//! stands with nothing to show — the alternative, leaving the section out,
//! would make the renderer read the mode instead of the frame.

use crate::field::{
    pulse_radius, reach_ticks, Cue, NodeKind, CUE_PULSE_EMITTED, CURRENT_STRENGTH_CAP,
    NODE_CHARGE_CAP,
};
use crate::fx::{adjacent, Vec2};
use crate::plan::{PlanCommand, PlanQueue};
use crate::run::{Mode, FORMS};
use crate::state::{FieldState, Fx, InputConfig, Progress};

/// The four ASCII bytes every snapshot opens with.
pub const MAGIC: &[u8; 4] = b"FGF1";

/// The snapshot format's own version, beside the protocol's.
pub const FRAME_VERSION: u16 = 2;

/// The header's width, in bytes. Every offset inside it is frozen.
pub const HEADER_BYTES: usize = 32;

/// One section table entry: kind, pad, count, offset.
pub const SECTION_ENTRY_BYTES: usize = 8;

/// The widest snapshot the timing contract allows to cross the boundary.
pub const FRAME_BUFFER_CAP: usize = 32 * 1024;

/// The locked section kinds. Only the nine written here are named as values;
/// the camera's arrives with the goal that owns its state.
const KIND_FORMS: u8 = 1;
const KIND_PORTS: u8 = 2;
const KIND_ROUTES: u8 = 3;
const KIND_CURRENTS: u8 = 4;
const KIND_VIEW: u8 = 5;
const KIND_PRESSURES: u8 = 6;
const KIND_CUES: u8 = 7;
const KIND_OVERLAY: u8 = 9;
const KIND_CURRENT_PATH: u8 = 10;

/// The record widths the document locks, in bytes.
const FORM_RECORD: usize = 24;
const PORT_RECORD: usize = 16;
const ROUTE_RECORD: usize = 16;
const CURRENT_RECORD: usize = 12;
const VIEW_RECORD: usize = 32;
const PRESSURE_RECORD: usize = 12;
const CUE_RECORD: usize = 8;
const OVERLAY_RECORD: usize = 8;
const PATH_RECORD: usize = 4;

/// The most path points one current carries into a frame.
pub const PATH_POINTS_CAP: usize = 32;

/// What a Route record's `status` byte says about the Route it carries.
///
/// The first three are the locked set. The last two are the other two shapes a
/// queued change takes on a Route — an end the queue would move, and a link the
/// queue proposes but no Route stands at — and they are values of the same byte
/// for the same reason `cut queued` is: a preview is the queue read as places
/// on the Field, and the renderer reads the Field from this buffer alone.
const ROUTE_STANDING: u8 = 0;
const ROUTE_CUT_QUEUED: u8 = 1;
const ROUTE_OVERLOADED: u8 = 2;
const ROUTE_MOVE_QUEUED: u8 = 3;
const ROUTE_PROPOSED: u8 = 4;

/// A current's path as the frame carries it: at most `PATH_POINTS_CAP` points,
/// both endpoints kept and the interior taken on an even stride.
///
/// The rule is locked by the document rather than chosen here, so the shape the
/// renderer strokes and the shape the encoder wrote are the same shape. The
/// decimation is render-only and one-way: Current delivery reads the authored
/// path and never this one.
fn decimated(path: &[Vec2]) -> Vec<Vec2> {
    if path.len() <= PATH_POINTS_CAP {
        return path.to_vec();
    }
    let last = path.len() - 1;
    let mut kept = Vec::with_capacity(PATH_POINTS_CAP);
    kept.push(path[0]);
    for step in 1..PATH_POINTS_CAP {
        // Rounded so the stride divides the interior evenly and the final index
        // lands exactly on the authored last point.
        let place = (step * last * 2 + (PATH_POINTS_CAP - 1)) / ((PATH_POINTS_CAP - 1) * 2);
        kept.push(path[place.min(last)]);
    }
    kept
}

/// The closed Node-kind set, in the order the document lists it.
fn kind_ordinal(kind: NodeKind) -> u8 {
    match kind {
        NodeKind::Port => 0,
        NodeKind::Reserve => 1,
        NodeKind::Module => 2,
        NodeKind::Form => 3,
    }
}

/// A raw quantity as a Q0.16 fraction of a cap, saturating. The widest value
/// the field carries is 65535, so a quantity at its cap saturates there — the
/// same one-way truncation the header's time scale takes.
fn q0_16(value: Fx, cap: Fx) -> u16 {
    if cap <= 0 || value <= 0 {
        return 0;
    }
    let scaled = (i128::from(value) << 16) / i128::from(cap);
    scaled.min(i128::from(u16::MAX)) as u16
}

/// The same, as a Q0.8 fraction.
fn q0_8(value: Fx, cap: Fx) -> u8 {
    if cap <= 0 || value <= 0 {
        return 0;
    }
    let scaled = (i128::from(value) << 8) / i128::from(cap);
    scaled.min(i128::from(u8::MAX)) as u8
}

/// A raw `Fx` as the locked one-way `f32`: raw ÷ 65536, rounded to nearest.
/// The division is exact in `f64` for every raw value the caps allow, so the
/// only rounding is the one the narrowing performs.
fn one_way(value: Fx) -> f32 {
    ((value as f64) / 65536.0) as f32
}

/// Everything the encoder reads. All of it is state the Field already holds;
/// nothing is stored for the renderer's sake.
pub struct Snapshot<'a> {
    pub field: &'a FieldState,
    pub mode: Mode,
    pub time_scale: u16,
    /// The active passive-observation View. Physical membership is read only
    /// from `field.physical_compartment` below.
    pub view_inside: &'a [u32],
    pub queue: &'a PlanQueue,
    /// The cues the steps of this frame raised, oldest first.
    pub cues: &'a [Cue],
    pub config: &'a InputConfig,
    pub progress: &'a Progress,
    /// The staged pressures, active and queued together, in the list's own
    /// closed-set order.
    pub pressures: &'a [crate::pressure::PressureState],
    /// The 1-based position of the standing objective in the chapter's
    /// authored order, and 0 while none stands.
    pub objective_ordinal: u16,
    /// The standing View's baseline `q_I` envelope, one low and high per step
    /// of its window, and empty when the window has no step to replay.
    pub forecast: &'a [(Fx, Fx)],
}

/// Encodes one snapshot.
pub fn encode(snapshot: &Snapshot<'_>) -> Vec<u8> {
    let field = snapshot.field;
    let members = physical_membership(
        field,
        &field.physical_compartment.members,
        snapshot.queue.proposed_compartment_members(),
    );
    // Every current's path, decimated once and laid end to end: a current's
    // record names where its own points start and how many it has.
    let paths: Vec<Vec<Vec2>> =
        field.currents.iter().map(|current| decimated(&current.path)).collect();
    let points: usize = paths.iter().map(Vec::len).sum();
    // The queue's proposals, resolved once: the section table counts them and
    // the section body writes them, and reading the queue twice for one frame
    // would be reading it twice for one answer.
    let proposed = previews(snapshot);

    // The overlay stands only in `still`, so a slate the run still remembers
    // puts no envelope on a moving frame: presence is the mode, and the count
    // is what the mode has to show.
    let still = snapshot.mode == Mode::Still;
    let envelope = if still { snapshot.forecast.len() } else { 0 };
    let sections: Vec<(u8, usize, usize)> = vec![
        (KIND_FORMS, field.forms.len(), FORM_RECORD),
        (KIND_PORTS, field.ports.len(), PORT_RECORD),
        (KIND_ROUTES, field.routes.len() + proposed.len(), ROUTE_RECORD),
        (KIND_CURRENTS, field.currents.len(), CURRENT_RECORD),
        // The bitset is one record, present whenever a Port stands to be a
        // member of it.
        (KIND_VIEW, usize::from(!field.ports.is_empty()), VIEW_RECORD),
        (KIND_PRESSURES, snapshot.pressures.len(), PRESSURE_RECORD),
        (KIND_CUES, snapshot.cues.len(), CUE_RECORD),
        (KIND_OVERLAY, envelope, OVERLAY_RECORD),
        (KIND_CURRENT_PATH, points, PATH_RECORD),
    ];
    // A section with nothing in it is left out, with the one exception the
    // document states as a mode rather than as a count: the overlay is present
    // exactly while the run is `still`, so a still frame carries the section
    // whether or not an envelope has been computed for it yet.
    let present: Vec<&(u8, usize, usize)> = sections
        .iter()
        .filter(|(kind, count, _)| *count > 0 || (*kind == KIND_OVERLAY && still))
        .collect();

    let table = HEADER_BYTES + present.len() * SECTION_ENTRY_BYTES;
    let mut out = vec![0u8; table];

    out[0..4].copy_from_slice(MAGIC);
    put_u16(&mut out, 4, FRAME_VERSION);
    let mut flags = 0u16;
    if snapshot.mode == Mode::Still {
        flags |= 1 << 0;
    }
    // Bit 1 is the dropped-time flag, which the worker sets: the accumulator
    // that drops the time is the worker's, not the core's.
    if snapshot.config.reduced_motion {
        flags |= 1 << 2;
    }
    put_u16(&mut out, 6, flags);
    put_u32(&mut out, 8, field.step);
    put_u16(&mut out, 12, snapshot.time_scale);
    out[14] = snapshot.mode.ordinal();
    // The camera itself arrives with the renderer; the layer it targets is the
    // controlled Form's, which is the one place the Field already says where
    // the view sits.
    out[15] = field.forms.iter().find(|form| form.controlled).map_or(0, |form| form.layer);
    // The Impulse is progress rather than Field, and the header's byte is the
    // same byte: the encoder reads it from where the run keeps it.
    out[16] = snapshot.progress.impulse;
    out[17] = snapshot.progress.chapter_index;
    put_u16(&mut out, 18, snapshot.objective_ordinal);
    out[20] = present.len() as u8;

    for (place, (kind, count, width)) in present.iter().enumerate() {
        let at = HEADER_BYTES + place * SECTION_ENTRY_BYTES;
        out[at] = *kind;
        out[at + 1] = 0;
        put_u16(&mut out, at + 2, *count as u16);
        let starts = out.len() as u32;
        put_u32(&mut out, at + 4, starts);
        let body = match *kind {
            KIND_FORMS => forms(snapshot),
            KIND_PORTS => ports(snapshot, &members),
            KIND_ROUTES => routes(snapshot, &proposed),
            KIND_CURRENTS => currents(snapshot, &paths),
            KIND_PRESSURES => pressures(snapshot),
            KIND_CUES => cues(snapshot),
            KIND_OVERLAY => overlay(snapshot),
            KIND_CURRENT_PATH => path_points(&paths),
            KIND_VIEW => view_bitset(field, snapshot.view_inside),
            _ => Vec::new(),
        };
        debug_assert_eq!(body.len(), count * width, "a section is its record width times its count");
        out.extend_from_slice(&body);
    }

    debug_assert!(out.len() <= FRAME_BUFFER_CAP, "the snapshot stays inside its locked cap");
    out
}

/// Which Ports are members of the physical compartment, which of those are
/// exposed, and which membership a queued physical edit proposes. These are
/// causal render flags and never read the observation View.
struct PhysicalMembership {
    member: Vec<bool>,
    shell: Vec<bool>,
    /// Which Ports the queue would make physical members, and none flagged
    /// while the queue proposes no compartment edit.
    proposed: Vec<bool>,
}

fn physical_membership(
    field: &FieldState,
    members: &[u32],
    proposed: Option<&[u32]>,
) -> PhysicalMembership {
    let mut member = vec![false; field.ports.len()];
    for node in members {
        if let Ok(place) = field.ports.binary_search_by_key(node, |port| port.node) {
            member[place] = true;
        }
    }
    let mut wanted = vec![false; field.ports.len()];
    for node in proposed.unwrap_or_default() {
        if let Ok(place) = field.ports.binary_search_by_key(node, |port| port.node) {
            wanted[place] = true;
        }
    }
    let mut shell = vec![false; field.ports.len()];
    for route in &field.routes {
        let (Ok(tail), Ok(head)) = (
            field.ports.binary_search_by_key(&route.tail, |port| port.node),
            field.ports.binary_search_by_key(&route.head, |port| port.node),
        ) else {
            continue;
        };
        if member[tail] != member[head] {
            shell[if member[tail] { tail } else { head }] = true;
        }
    }
    for place in 0..field.ports.len() {
        if !member[place] || shell[place] {
            continue;
        }
        let (pos, layer) = (field.ports[place].pos, field.ports[place].layer);
        shell[place] = field.ports.iter().enumerate().any(|(other, port)| {
            !member[other] && adjacent(pos, layer, port.pos, port.layer)
        });
    }
    PhysicalMembership { member, shell, proposed: wanted }
}

fn forms(snapshot: &Snapshot<'_>) -> Vec<u8> {
    let mut out = Vec::with_capacity(snapshot.field.forms.len() * FORM_RECORD);
    // The separated reading, taken by the one function the delivery rules take
    // it from, so the flag a player sees and the delivery a Node is passed over
    // for are the same fact — under every Handoff, control inside the linked
    // group or away from it.
    let apart = crate::field::separated_forms(snapshot.field);
    for (place, form) in snapshot.field.forms.iter().enumerate() {
        out.push(form.id);
        out.push(FORMS.iter().position(|name| *name == form.form).unwrap_or(0) as u8);
        out.push(form.layer);
        let mut flags = 0u8;
        if form.controlled {
            flags |= 1 << 0;
        }
        if form.focus {
            flags |= 1 << 1;
        }
        if form.pulse_charge > 0 {
            flags |= 1 << 2;
        }
        // Separated: a member of the linked group standing further from the
        // reference member than its authored separation admits, which is
        // exactly the Form whose Node the delivery rules pass over this step.
        if apart[place] {
            flags |= 1 << 3;
        }
        out.push(flags);
        for value in [form.pos.x, form.pos.y, form.vel.x, form.vel.y] {
            out.extend_from_slice(&one_way(value).to_le_bytes());
        }
        out.extend_from_slice(&q0_16(form.charge, NODE_CHARGE_CAP).to_le_bytes());
        // The Pulse's reach, in Q8.8 units: the emitted reach on the step an
        // emission raised its cue, the reach so far while a hold is charging,
        // and 0 at rest. It is derived from the charge rather than stored, and
        // on the emission step the charge is already spent — so the emitted
        // value is read from the emission's own cue, which carries exactly it
        // and stands for exactly this frame. One quantity, one source.
        let emitted = snapshot
            .cues
            .iter()
            .find(|cue| cue.kind == CUE_PULSE_EMITTED && cue.b == form.node)
            .map(|cue| cue.a);
        let reach = match emitted {
            Some(carried) => carried,
            None if form.pulse_charge > 0 => reach_ticks(pulse_radius(form.pulse_charge)),
            None => 0,
        };
        out.extend_from_slice(&reach.to_le_bytes());
    }
    out
}

fn ports(snapshot: &Snapshot<'_>, members: &PhysicalMembership) -> Vec<u8> {
    let field = snapshot.field;
    // The end-of-step overload flags consult Flood's lowered threshold with
    // the staged list, which is the list this snapshot carries.
    let press = crate::pressure::FloodPress::of(snapshot.pressures);
    // The stored reserve is the run's rather than any one Form's — `content`'s
    // `establish` stands the whole of it with the Form the chapter opened
    // control on and opens every other placement holding none — so it is drawn
    // on the Node of the Form carrying control *now*. A Handoff moves control
    // without moving the quantity, and reading it off the Form it was
    // established on would leave it drawn beside a Form the player is no longer
    // steering. Summed rather than found, because "the run's" is what the rule
    // says and one Form holding it is how the rule is kept.
    let reserve: Fx = field.forms.iter().map(|form| form.reserve).sum();
    let carrying = field.forms.iter().find(|form| form.controlled).map(|form| form.node);
    let mut out = Vec::with_capacity(field.ports.len() * PORT_RECORD);
    for (place, port) in field.ports.iter().enumerate() {
        out.extend_from_slice(&port.node.to_le_bytes());
        out.push(kind_ordinal(port.kind));
        let mut flags = 0u8;
        if port.open {
            flags |= 1 << 0;
        }
        if port.q > press.threshold(port.node, port.capacity) {
            flags |= 1 << 1;
        }
        if members.member[place] {
            flags |= 1 << 2;
        }
        if members.shell[place] {
            flags |= 1 << 3;
        }
        // The physical membership the queue proposes, which is a preview and
        // not standing state: the two flags together say whether a Node would
        // be taken in, left out, or left where it is. A frame whose queue
        // proposes no compartment edit raises none of them.
        if members.proposed[place] {
            flags |= 1 << 4;
        }
        out.push(flags);
        out.extend_from_slice(&q0_16(port.q, NODE_CHARGE_CAP).to_le_bytes());
        // Position in units times 16, which is the raw value shifted down 12.
        out.extend_from_slice(&((port.pos.x >> 12) as u16).to_le_bytes());
        out.extend_from_slice(&((port.pos.y >> 12) as u16).to_le_bytes());
        // The Charge held in reserve, as a fraction of this Node's threshold,
        // drawn at the one Node the run's reserve stands at: the controlled
        // Form's.
        let held = if carrying == Some(port.node) { reserve } else { 0 };
        out.extend_from_slice(&q0_16(held, port.capacity).to_le_bytes());
        // The layer the Node stands on, so the renderer places it on its own
        // plane. A Form-kind Node carries the layer of the Form it mirrors.
        out.push(port.layer);
        out.push(0);
    }
    out
}

/// The links the queue proposes, as records the renderer can draw: the Route
/// identifier the change stands under, and the endpoints it would stand at.
///
/// A queued change is not state — nothing here has been committed, and a
/// refused commit applies none of it — so a preview is written into the frame
/// beside the Field rather than into the Field. It rides the routes section
/// because that is where the locked layout already carries a queued change: the
/// `cut queued` status is one, and these are the other two.
fn previews(snapshot: &Snapshot<'_>) -> Vec<(u32, u32, u32)> {
    snapshot
        .queue
        .queued()
        .iter()
        .filter_map(|entry| match (entry.route, entry.pair) {
            (Some(route), Some((tail, head))) => Some((route, tail, head)),
            _ => None,
        })
        .collect()
}

fn routes(snapshot: &Snapshot<'_>, proposed: &[(u32, u32, u32)]) -> Vec<u8> {
    // The same staged-list threshold the port flags consult.
    let press = crate::pressure::FloodPress::of(snapshot.pressures);
    let field = snapshot.field;
    let queued: Vec<u32> = snapshot
        .queue
        .queued()
        .iter()
        .filter(|entry| entry.cut)
        .filter_map(|entry| entry.route)
        .collect();
    // A Route an entry proposes to move is drawn where it stands and again
    // where it would go: the standing record keeps its own status, and the
    // preview carries the proposed endpoints.
    let moved: Vec<u32> = snapshot
        .queue
        .queued()
        .iter()
        .filter(|entry| matches!(entry.plan, PlanCommand::Redirect { .. }))
        .filter_map(|entry| entry.route)
        .collect();
    let mut out = Vec::with_capacity((field.routes.len() + proposed.len()) * ROUTE_RECORD);
    for route in &field.routes {
        out.extend_from_slice(&route.route.to_le_bytes());
        out.extend_from_slice(&route.tail.to_le_bytes());
        out.extend_from_slice(&route.head.to_le_bytes());
        out.push(q0_8(route.flow, route.capacity));
        let overloaded = field
            .ports
            .binary_search_by_key(&route.head, |port| port.node)
            .map(|place| {
                let held = &field.ports[place];
                held.q > press.threshold(held.node, held.capacity)
            })
            .unwrap_or(false);
        out.push(if queued.contains(&route.route) {
            ROUTE_CUT_QUEUED
        } else if moved.contains(&route.route) {
            ROUTE_MOVE_QUEUED
        } else if overloaded {
            ROUTE_OVERLOADED
        } else {
            ROUTE_STANDING
        });
        let age = field.step.saturating_sub(route.formed_step);
        out.extend_from_slice(&(age.min(u32::from(u16::MAX)) as u16).to_le_bytes());
    }
    for (route, tail, head) in proposed.iter().copied() {
        out.extend_from_slice(&route.to_le_bytes());
        out.extend_from_slice(&tail.to_le_bytes());
        out.extend_from_slice(&head.to_le_bytes());
        // A proposal carries nothing and has stood for no step: no flow, no
        // age. What it is, the status says.
        out.push(0);
        out.push(ROUTE_PROPOSED);
        out.extend_from_slice(&0u16.to_le_bytes());
    }
    out
}

fn currents(snapshot: &Snapshot<'_>, paths: &[Vec<Vec2>]) -> Vec<u8> {
    let mut out = Vec::with_capacity(snapshot.field.currents.len() * CURRENT_RECORD);
    let mut at = 0usize;
    for (place, current) in snapshot.field.currents.iter().enumerate() {
        out.extend_from_slice(&current.id.to_le_bytes());
        out.push(current.layer);
        let mut flags = 0u8;
        if current.active {
            flags |= 1 << 0;
        }
        if current.bright {
            flags |= 1 << 1;
        }
        out.push(flags);
        out.extend_from_slice(&current.phase.to_le_bytes());
        out.extend_from_slice(&q0_16(current.strength, CURRENT_STRENGTH_CAP).to_le_bytes());
        // Where this current's own points start in the flat array, and how
        // many of them there are.
        let count = paths[place].len();
        out.extend_from_slice(&(at as u16).to_le_bytes());
        out.push(count as u8);
        // The band a Node has to stand within to receive, in whole units.
        out.push((current.width >> 16).clamp(0, 255) as u8);
        at += count;
    }
    out
}

/// The staged pressures, one locked 12-byte record each: ordinal, stage,
/// target kind, the queued flag, the level as Q0.16 of the level range, two
/// zero bytes, and the target's identifier — 0 for a target that names none,
/// exactly as a cue's `b` is the Node it stands at and 0 where none does.
///
/// `level` is a `Frac` over [0, 65536] and the field is a u16, so the one-way
/// saturation is the header's own time-scale truncation: 65536 encodes as
/// 65535, confined to the render-only buffer.
fn pressures(snapshot: &Snapshot<'_>) -> Vec<u8> {
    let mut out = Vec::with_capacity(snapshot.pressures.len() * PRESSURE_RECORD);
    for pressure in snapshot.pressures {
        out.push(pressure.pressure.ordinal());
        out.push(pressure.stage.ordinal());
        out.push(pressure.target.kind.ordinal());
        out.push(u8::from(pressure.queued));
        // The effective level — the curve floored by a Pulse's press-back —
        // which is the level the reading on the surface answers to.
        let level = pressure.effective_level().clamp(0, i64::from(u16::MAX)) as u16;
        out.extend_from_slice(&level.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&pressure.target.id.unwrap_or(0).to_le_bytes());
    }
    out
}

/// The cues this frame's steps raised, in the order they were raised. `b` is
/// the Node the cue stands at and what `a` carries is locked per kind.
fn cues(snapshot: &Snapshot<'_>) -> Vec<u8> {
    let mut out = Vec::with_capacity(snapshot.cues.len() * CUE_RECORD);
    for cue in snapshot.cues {
        out.push(cue.kind);
        out.push(0);
        out.extend_from_slice(&cue.a.to_le_bytes());
        out.extend_from_slice(&cue.b.to_le_bytes());
    }
    out
}

/// The Still Mode overlay: the standing View's baseline `q_I` envelope, one
/// low and high per step of its window, in the same one-way `f32` every other
/// derived quantity crosses as.
fn overlay(snapshot: &Snapshot<'_>) -> Vec<u8> {
    let mut out = Vec::with_capacity(snapshot.forecast.len() * OVERLAY_RECORD);
    for (low, high) in snapshot.forecast {
        out.extend_from_slice(&one_way(*low).to_le_bytes());
        out.extend_from_slice(&one_way(*high).to_le_bytes());
    }
    out
}

/// The flat point array every current's path indexes into, in the same
/// quantization a Port position uses: units times 16, per axis.
fn path_points(paths: &[Vec<Vec2>]) -> Vec<u8> {
    let mut out = Vec::with_capacity(paths.iter().map(Vec::len).sum::<usize>() * PATH_RECORD);
    for path in paths {
        for point in path {
            out.extend_from_slice(&((point.x >> 12) as u16).to_le_bytes());
            out.extend_from_slice(&((point.y >> 12) as u16).to_le_bytes());
        }
    }
    out
}

/// One 32-byte bitset: bit i is set when Port record i is a member of the
/// active passive-observation View. The Node cap is 256, which is exactly 32
/// bytes of bits. Physical membership is deliberately not consulted here.
fn view_bitset(field: &FieldState, inside: &[u32]) -> Vec<u8> {
    let mut out = vec![0u8; VIEW_RECORD];
    for node in inside {
        if let Ok(place) = field.ports.binary_search_by_key(node, |port| port.node) {
            out[place / 8] |= 1 << (place % 8);
        }
    }
    out
}

fn put_u16(out: &mut [u8], at: usize, value: u16) {
    out[at..at + 2].copy_from_slice(&value.to_le_bytes());
}

fn put_u32(out: &mut [u8], at: usize, value: u32) {
    out[at..at + 4].copy_from_slice(&value.to_le_bytes());
}
