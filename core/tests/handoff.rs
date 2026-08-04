//! The Handoff: control moving between Forms.
//!
//! `docs/field-framework/ARCHITECTURE.md`'s `The Handoff` locks the whole of it,
//! and this file reads that section as a contract:
//!
//! - **The mechanism.** `InspectRequest` target `handoff`, `parameter` the Form
//!   `id` taking control. Still mode only, immediate — no queue entry, no
//!   commit, no Impulse — and ignored outside `still` exactly as every other
//!   inspection is.
//! - **The refusals.** `not_found` for a Form this run does not hold;
//!   `validation` for a Form the chapter marks un-controllable and for a
//!   Handoff to the Form already controlled, because a no-op is refused rather
//!   than silently absorbed. A request naming no Form at all disagrees with its
//!   own target and is refused where it is read.
//! - **The boundary.** A Handoff is a state mutation no control schedule
//!   derives, so it is a membership boundary: the active window ends, the
//!   keyframe carries the moved flag, and no retained window spans it. What
//!   that buys is byte-equivalence, which is asserted both ways — an export
//!   round trip and a Quick Retry across the move.
//! - **The camera.** The frame's header camera layer is the controlled Form's,
//!   so it moves on the frame control moved on.
//! - **The linked group.** A Handoff inside a Chorus re-anchors the formation
//!   without deforming it: every station is the reference member's position
//!   plus the difference of the two authored offsets.
//!
//! The un-controllable refusal and the away-from-the-group reading both need a
//! chapter that places several Forms, and are read in
//! `core/tests/chapter_the_mesh.rs` against the chapter that authors them.

use field_game_core::field::{
    self, FieldLayer, FormState, LinkState, NodeKind, PhysicalCompartment, PortState,
};
use field_game_core::fx::{Vec2, ONE_UNIT};
use field_game_core::json::{parse, Json};
use field_game_core::state::{FieldState, InputConfig, Progress};
use field_game_core::Session;

mod support;

const KEY: &str = "00112233445566dd";

/// One rendered frame at the 60-frames-per-second target, in microseconds.
const FRAME_US: i64 = 16_667;

/// How long a ramp takes, in microseconds. Locked.
const RAMP_US: i64 = 250_000;

/// One frame of input, with every declared field present and no inspection.
fn frame(seq: u32, steps: u16, t_us: i64, toggle_still: bool) -> String {
    built(seq, steps, t_us, toggle_still, "null")
}

/// The same, carrying one inspect request verbatim.
fn asking(seq: u32, t_us: i64, inspect: &str) -> String {
    built(seq, 0, t_us, false, inspect)
}

/// One frame carrying a steer vector, so the Form under control takes on
/// velocity rather than standing where the chapter placed it. Every other
/// frame helper here holds the control released, which is the right reading
/// for a refusal but a vacuous one for anything that coasts.
fn steered(seq: u32, steps: u16, t_us: i64, steer_x: i16, steer_y: i16) -> String {
    format!(
        "{{\"advance_steps\":{steps},\"depth_key\":0,\"inspect\":null,\"pause\":false,\
          \"pulse_held\":false,\"pulse_release\":false,\"seq\":{seq},\"steer_x\":{steer_x},\
          \"steer_y\":{steer_y},\"t_us\":{t_us},\"toggle_still\":false,\"wheel\":0}}"
    )
}

/// The inspect body of a Handoff naming one Form.
fn handoff_of(id: i64) -> String {
    format!("{{\"kind\":null,\"parameter\":{id},\"target\":\"handoff\"}}")
}

fn built(seq: u32, steps: u16, t_us: i64, toggle_still: bool, inspect: &str) -> String {
    format!(
        "{{\"advance_steps\":{steps},\"depth_key\":0,\"inspect\":{inspect},\"pause\":false,\
          \"pulse_held\":false,\"pulse_release\":false,\"seq\":{seq},\"steer_x\":0,\
          \"steer_y\":0,\"t_us\":{t_us},\"toggle_still\":{toggle_still},\"wheel\":0}}"
    )
}

fn opened(form: &str) -> Session {
    let mut session = Session::new(&support::worker_init()).expect("versions agree");
    let answer = session.command(
        "init_run",
        &format!("{{\"form\":\"{form}\",\"mode\":\"new\",\"run_id\":\"{KEY}\"}}"),
    );
    assert!(answer.contains("\"ok\":true"), "a new run opens: {answer}");
    session
}

/// The body of a successful response, parsed.
fn body(answer: &str) -> Json {
    let parsed = parse(answer).expect("a response is canonical JSON");
    assert_eq!(parsed.get("ok"), Some(&Json::Bool(true)), "{answer}");
    parsed.get("body").expect("a successful response carries a body").clone()
}

fn refusal(answer: &str) -> String {
    let parsed = parse(answer).expect("a response is canonical JSON");
    assert_eq!(parsed.get("ok"), Some(&Json::Bool(false)), "{answer}");
    let error = parsed.get("error").expect("a refusal carries an envelope");
    error.get("code").and_then(Json::as_text).expect("a code").to_string()
}

fn int_of(value: &Json, key: &str) -> i64 {
    value.get(key).and_then(Json::as_int).expect("an integer field")
}

fn exported(session: &mut Session) -> String {
    let held = body(&session.command("export_run", "{}"));
    held.get("text").and_then(Json::as_text).expect("an export file").to_string()
}

fn import_body(text: &str) -> String {
    let mut out = String::from("{\"text\":");
    field_game_core::json::write_text(&mut out, text);
    out.push('}');
    out
}

/// Which Form carries control.
fn controlled_id(session: &Session) -> u8 {
    session
        .run()
        .expect("a run")
        .state()
        .now
        .forms
        .iter()
        .find(|form| form.controlled)
        .expect("exactly one controlled Form")
        .id
}

/// Runs a few ordinary steps and then enters Still Mode, answering the next
/// free sequence number and the timestamp the entry completed at.
fn entered(session: &mut Session, steps: u16) -> (u32, i64) {
    let mut seq = 1u32;
    for _ in 0..4 {
        let answer = session.command("input_frame", &frame(seq, steps, i64::from(seq) * FRAME_US, false));
        assert!(answer.contains("\"ok\":true"), "{answer}");
        seq += 1;
    }
    let opened_at = i64::from(seq) * FRAME_US;
    session.command("input_frame", &frame(seq, 1, opened_at, true));
    assert_eq!(session.lifecycle(), "ramp_in");
    seq += 1;
    session.command("input_frame", &frame(seq, 0, opened_at + RAMP_US, false));
    assert_eq!(session.lifecycle(), "still", "the ramp completes into Still Mode");
    seq += 1;
    (seq, opened_at + RAMP_US)
}

// ---------------------------------------------------------------------------
// The request, and the three refusals
// ---------------------------------------------------------------------------

#[test]
fn a_handoff_names_the_form_taking_control_and_nothing_else() {
    let mut session = opened("chorus");
    let (seq, at) = entered(&mut session, 3);

    // A Handoff that names no Form disagrees with its own target: the
    // parameter is the Form id, so a null one is refused where it is read,
    // exactly as a perturbation with no kind is.
    assert_eq!(
        refusal(&session.command(
            "input_frame",
            &asking(seq, at + FRAME_US, "{\"kind\":null,\"parameter\":null,\"target\":\"handoff\"}"),
        )),
        "validation",
        "a Handoff naming no Form is refused",
    );
    // And a Handoff carrying a perturbation kind is refused for the same
    // reason from the other side.
    assert_eq!(
        refusal(&session.command(
            "input_frame",
            &asking(
                seq,
                at + FRAME_US,
                "{\"kind\":\"route-removal\",\"parameter\":2,\"target\":\"handoff\"}",
            ),
        )),
        "validation",
        "a Handoff carrying a kind is refused",
    );
    assert_eq!(controlled_id(&session), 1, "a refused request moves nothing");
}

#[test]
fn a_handoff_to_a_form_the_run_does_not_hold_is_not_found() {
    let mut session = opened("chorus");
    let (seq, at) = entered(&mut session, 3);
    let held = session.run().expect("a run").state().now.forms.len();
    assert_eq!(held, 4, "Chorus stands four Forms");

    let answer = session.command("input_frame", &asking(seq, at + FRAME_US, &handoff_of(9)));
    assert_eq!(refusal(&answer), "not_found", "no Form 9 stands: {answer}");
    assert!(answer.contains("\"id\":9"), "the refusal names what it looked for: {answer}");
    assert_eq!(controlled_id(&session), 1);
}

#[test]
fn a_handoff_to_the_form_already_controlled_is_refused_rather_than_absorbed() {
    let mut session = opened("chorus");
    let (seq, at) = entered(&mut session, 3);
    assert_eq!(controlled_id(&session), 1);

    assert_eq!(
        refusal(&session.command("input_frame", &asking(seq, at + FRAME_US, &handoff_of(1)))),
        "validation",
        "a no-op Handoff is refused, not silently absorbed",
    );
    assert_eq!(controlled_id(&session), 1, "and control has not moved");
}

#[test]
fn a_handoff_outside_still_mode_is_ignored_and_answers_nothing() {
    let mut session = opened("chorus");
    // A well-formed request while the Field is moving: read where it arrives,
    // and simply not answered.
    let answer = session.command(
        "input_frame",
        &built(1, 3, FRAME_US, false, &handoff_of(2)),
    );
    assert_eq!(int_of(&body(&answer), "steps_run"), 3, "the frame still ran its steps");
    assert_eq!(controlled_id(&session), 1, "control did not move outside Still Mode");
    assert!(session.take_events().find("review_ready").is_none(), "and nothing was reviewed");
}

// ---------------------------------------------------------------------------
// What a Handoff does
// ---------------------------------------------------------------------------

#[test]
fn a_handoff_moves_the_one_controlled_flag_immediately_and_for_free() {
    let mut session = opened("chorus");
    let (seq, at) = entered(&mut session, 3);
    let impulse = session.run().expect("a run").state().progress.impulse;
    let queued = session.run().expect("a run").queue().len();

    let answer = session.command("input_frame", &asking(seq, at + FRAME_US, &handoff_of(3)));
    assert!(answer.contains("\"ok\":true"), "{answer}");
    assert_eq!(int_of(&body(&answer), "steps_run"), 0, "a still run runs no step");
    assert_eq!(controlled_id(&session), 3, "control landed on the Form named");

    let run = session.run().expect("a run");
    assert_eq!(
        run.state().now.forms.iter().filter(|form| form.controlled).count(),
        1,
        "exactly one Form carries control at every step",
    );
    assert_eq!(run.state().progress.impulse, impulse, "a Handoff costs no Impulse");
    assert_eq!(run.queue().len(), queued, "and enters no queue");
    assert_eq!(session.lifecycle(), "still", "the run is where the request found it");
}

#[test]
fn a_handoff_is_a_membership_boundary_that_ends_the_active_window() {
    let mut session = opened("chorus");
    let (seq, at) = entered(&mut session, 30);

    let before = session.run().expect("a run").state().trace.steps.len();
    assert!(before > 0, "the run stands with a retained window: {before} steps");
    let standing = session.run().expect("a run").state().now.step;

    let answer = session.command("input_frame", &asking(seq, at + FRAME_US, &handoff_of(2)));
    assert!(answer.contains("\"ok\":true"), "{answer}");

    let run = session.run().expect("a run");
    let trace = &run.state().trace;
    assert!(trace.steps.is_empty(), "the active window ended: no step is retained across it");
    assert_eq!(trace.start_step, standing, "the trajectory restarts where the Handoff landed");
    assert_eq!(
        trace.keyframe.forms.iter().find(|form| form.controlled).map(|form| form.id),
        Some(2),
        "the keyframe carries the moved flag, so every later replay steers the new Form",
    );
    assert_eq!(
        trace.keyframe.step,
        run.state().now.step,
        "a keyframe carried to this step stands where the run stands",
    );

    // And the teeth of it. The keyframe is carried forward by replaying the
    // steps that fall out of the retained window onto it, and a replay reads
    // the keyframe's own flags — so a window that spanned the Handoff would
    // carry a keyframe steering the Form control has left, for as long as the
    // run went on. Out of Still Mode and two hundred steps later, the two
    // readings still agree.
    let mut seq = seq + 1;
    session.command("input_frame", &frame(seq, 0, at + 2 * FRAME_US, true));
    seq += 1;
    session.command("input_frame", &frame(seq, 0, at + 2 * FRAME_US + RAMP_US, false));
    seq += 1;
    for _ in 0..10 {
        session.command("input_frame", &frame(seq, 30, at + i64::from(seq) * FRAME_US, false));
        seq += 1;
    }
    let run = session.run().expect("a run");
    assert!(run.state().now.step > standing + 120, "far enough for the keyframe to have moved");
    assert!(run.state().trace.start_step > standing, "the keyframe has been carried past it");
    assert_eq!(
        run.state().trace.keyframe.forms.iter().find(|form| form.controlled).map(|form| form.id),
        Some(2),
        "the carried keyframe still steers the Form control moved to",
    );
}

#[test]
fn a_run_carried_across_a_handoff_exports_and_imports_byte_for_byte() {
    let mut session = opened("chorus");
    let (mut seq, at) = entered(&mut session, 20);
    let answer = session.command("input_frame", &asking(seq, at + FRAME_US, &handoff_of(4)));
    assert!(answer.contains("\"ok\":true"), "{answer}");
    seq += 1;
    // Out of Still Mode, and on with the run under the Form control moved to.
    session.command("input_frame", &frame(seq, 0, at + 2 * FRAME_US, true));
    seq += 1;
    session.command("input_frame", &frame(seq, 0, at + 2 * FRAME_US + RAMP_US, false));
    assert_eq!(session.lifecycle(), "running");
    seq += 1;
    for _ in 0..6 {
        session.command("input_frame", &frame(seq, 20, at + i64::from(seq) * FRAME_US, false));
        seq += 1;
    }
    assert_eq!(controlled_id(&session), 4, "the run carried on under the Form it handed to");

    let file = exported(&mut session);
    let mut second = Session::new(&support::worker_init()).expect("versions agree");
    let landed = body(&second.command("import_run", &import_body(&file)));
    assert_eq!(
        int_of(&landed, "step"),
        i64::from(session.run().expect("a run").state().now.step),
    );
    assert_eq!(controlled_id(&second), 4, "the payload's flags are the record");

    // Byte-equal, not merely equivalent: the second export of the imported run
    // is the file the first one produced, past the one locked normalization.
    let again = exported(&mut second);
    let mut third = Session::new(&support::worker_init()).expect("versions agree");
    third.command("import_run", &import_body(&again));
    assert_eq!(exported(&mut third), again, "byte-equal across the Handoff");

    // And carrying on from the restore lands on the same bytes the source
    // does: the retained window the Handoff ended never spanned it.
    let mut carried_seq = seq;
    for _ in 0..4 {
        let stamp = at + i64::from(carried_seq) * FRAME_US;
        session.command("input_frame", &frame(carried_seq, 20, stamp, false));
        carried_seq += 1;
    }
    let source = session.run().expect("a run").state().payload();
    let mut restored_seq = 1u32;
    for _ in 0..4 {
        let stamp = at + i64::from(restored_seq) * FRAME_US;
        third.command("input_frame", &frame(restored_seq, 20, stamp, false));
        restored_seq += 1;
    }
    let after = third.run().expect("a run").state().payload();
    // The one post-restore normalization, and the only difference the resumed
    // run carries: `prev_assembly_step` in the live Field is set to the step
    // the restore returned to. The keyframe's copy is untouched, and the
    // Handoff's own boundary took that keyframe, so the substitution below
    // reaches exactly the live one.
    let returned_to = int_of(&landed, "step");
    let held = session
        .run()
        .expect("a run")
        .state()
        .now
        .prev_assembly_step
        .expect("a slate was assembled on the Still Mode entry");
    assert_ne!(i64::from(held), returned_to, "the two readings are distinguishable");
    assert_eq!(
        after.replace(
            &format!("\"prev_assembly_step\":{returned_to},"),
            &format!("\"prev_assembly_step\":{held},"),
        ),
        source,
        "a run resumed across a Handoff carries on byte for byte",
    );
}

#[test]
fn a_quick_retry_across_a_handoff_lands_on_the_flag_the_anchor_recorded() {
    let mut session = opened("chorus");
    // Far enough for the autosave cadence to have written a record, which is
    // what Quick Retry restores to.
    let mut seq = 1u32;
    for _ in 0..40 {
        session.command("input_frame", &frame(seq, 30, i64::from(seq) * FRAME_US, false));
        seq += 1;
    }
    let held: Vec<u32> = session
        .run()
        .expect("a run")
        .state()
        .anchors
        .iter()
        .map(|anchor| anchor.anchor_id)
        .collect();
    assert!(!held.is_empty(), "the run wrote a record to come back to: {held:?}");
    let anchor_id = *held.last().expect("the newest record");
    let recorded = controlled_id(&session);

    // Into Still Mode, hand control away, and back out.
    let opened_at = i64::from(seq) * FRAME_US;
    session.command("input_frame", &frame(seq, 1, opened_at, true));
    seq += 1;
    session.command("input_frame", &frame(seq, 0, opened_at + RAMP_US, false));
    seq += 1;
    let answer = session.command(
        "input_frame",
        &asking(seq, opened_at + RAMP_US + FRAME_US, &handoff_of(2)),
    );
    assert!(answer.contains("\"ok\":true"), "{answer}");
    assert_eq!(controlled_id(&session), 2);

    let answer = session.command("restore_checkpoint", &format!("{{\"anchor_id\":{anchor_id}}}"));
    assert!(answer.contains("\"ok\":true"), "{answer}");
    assert_eq!(
        controlled_id(&session),
        recorded,
        "a Quick Retry lands on the control the record was taken under",
    );
    assert_eq!(
        session.run().expect("a run").state().now.forms.iter().filter(|f| f.controlled).count(),
        1,
        "and exactly one Form carries it",
    );
}

// ---------------------------------------------------------------------------
// The camera, in the frame
// ---------------------------------------------------------------------------

/// A Field standing two Forms on two layers, one of them controlled.
fn two_layers(controlled: u8) -> FieldState {
    let node = |id: u32, layer: u8, x: i64| PortState {
        node: id,
        layer,
        pos: Vec2::units(x, 2000),
        kind: NodeKind::Form,
        q: 0,
        open: true,
        upkeep_rate: 0,
        capacity: 64 * ONE_UNIT,
    };
    let form = |id: u8, layer: u8, x: i64| FormState {
        id,
        form: "thread".to_string(),
        node: u32::from(id),
        controlled: id == controlled,
        layer,
        pos: Vec2::units(x, 2000),
        vel: Vec2::default(),
        charge: 0,
        reserve: 0,
        pulse_charge: 0,
        focus: false,
        route_reach: 64 * ONE_UNIT,
        forecast_depth: 8,
        steer_scale: 65_536,
        route_capacity: 8 * ONE_UNIT,
        link: None,
        trail: None,
        junction: None,
    };
    let mut field = FieldState::opening();
    field.ports = vec![node(1, 0, 1900), node(2, 1, 2100)];
    field.forms = vec![form(1, 0, 1900), form(2, 1, 2100)];
    field.layers = vec![
        FieldLayer {
            layer: 0,
            drain: 0,
            noise: 0,
            gain: 32_768,
            current_ids: Vec::new(),
            port_ids: vec![1],
        },
        FieldLayer {
            layer: 1,
            drain: 0,
            noise: 0,
            gain: 32_768,
            current_ids: Vec::new(),
            port_ids: vec![2],
        },
    ];
    field.next_node_id = 3;
    field.physical_compartment = PhysicalCompartment {
        members: vec![1, 2],
        leak_per_exposed_contact_per_step: 0,
    };
    field
}

#[test]
fn the_frames_camera_layer_follows_the_form_control_moved_to() {
    let progress = Progress::opening();
    let config = InputConfig::default_config();
    let layer_of = |controlled: u8| -> u8 {
        let field = two_layers(controlled);
        let encoded = field_game_core::frame::encode(&field_game_core::frame::Snapshot {
            field: &field,
            mode: field_game_core::run::Mode::Still,
            time_scale: 0,
            progress: &progress,
            objective_ordinal: 0,
            config: &config,
            cues: &[],
            pressures: &[],
            queue: &field_game_core::plan::PlanQueue::new(),
            view_inside: &[],
            forecast: &[],
            medium: field_game_core::field::MediumMotion::default(),
        });
        encoded[15]
    };
    // The camera layer is the controlled Form's, which is the one place the
    // Field already says where the view sits — so on the frame control moved
    // on, the authoritative target has moved with it.
    assert_eq!(layer_of(1), 0, "control on the shallow Form");
    assert_eq!(layer_of(2), 1, "and on the deep one, the camera is deep");
}

// ---------------------------------------------------------------------------
// The linked group
// ---------------------------------------------------------------------------

/// Every Form's position, by id.
fn stations(session: &Session) -> Vec<(u8, Vec2)> {
    session
        .run()
        .expect("a run")
        .state()
        .now
        .forms
        .iter()
        .map(|form| (form.id, form.pos))
        .collect()
}

/// One Form's position and one Form's velocity, by identifier.
fn placed_at(session: &Session, id: u8) -> Vec2 {
    stations(session).into_iter().find(|held| held.0 == id).expect("the Form stands").1
}

fn moving_at(session: &Session, id: u8) -> Vec2 {
    session
        .run()
        .expect("a run")
        .state()
        .now
        .forms
        .iter()
        .find(|form| form.id == id)
        .expect("the Form stands")
        .vel
}

#[test]
fn a_handoff_inside_a_chorus_re_anchors_the_formation_without_deforming_it() {
    let mut session = opened("chorus");
    let authored: Vec<(u8, Option<LinkState>)> = session
        .run()
        .expect("a run")
        .state()
        .now
        .forms
        .iter()
        .map(|form| (form.id, form.link))
        .collect();
    assert_eq!(authored.len(), 4, "Chorus stands four Forms");
    assert!(authored[0].1.is_none(), "the anchor carries no link of its own");
    assert!(authored[1..].iter().all(|(_, link)| link.is_some()), "the members carry theirs");

    // Settled into formation, then control moves inside the group.
    let mut seq = 1u32;
    for _ in 0..30 {
        session.command("input_frame", &frame(seq, 30, i64::from(seq) * FRAME_US, false));
        seq += 1;
    }
    let before = stations(&session);
    let spread = |held: &[(u8, Vec2)]| -> Vec<(u8, u8, i64, i64)> {
        let mut out = Vec::new();
        for (place, (first, at)) in held.iter().enumerate() {
            for (second, other) in held.iter().skip(place + 1) {
                out.push((*first, *second, other.x - at.x, other.y - at.y));
            }
        }
        out
    };
    let held_spread = spread(&before);

    let opened_at = i64::from(seq) * FRAME_US;
    session.command("input_frame", &frame(seq, 1, opened_at, true));
    seq += 1;
    session.command("input_frame", &frame(seq, 0, opened_at + RAMP_US, false));
    seq += 1;
    let answer = session.command(
        "input_frame",
        &asking(seq, opened_at + RAMP_US + FRAME_US, &handoff_of(3)),
    );
    assert!(answer.contains("\"ok\":true"), "{answer}");
    seq += 1;
    assert_eq!(controlled_id(&session), 3);
    assert_eq!(stations(&session), before, "a still run moved nothing");

    // Back out, and let the formation settle around its new reference member
    // with nothing steering it anywhere.
    session.command("input_frame", &frame(seq, 0, opened_at + RAMP_US + 2 * FRAME_US, true));
    seq += 1;
    session.command("input_frame", &frame(seq, 0, opened_at + 2 * RAMP_US + 2 * FRAME_US, false));
    assert_eq!(session.lifecycle(), "running");
    seq += 1;
    for _ in 0..40 {
        session.command("input_frame", &frame(seq, 30, opened_at + i64::from(seq) * FRAME_US, false));
        seq += 1;
    }

    // The formation is the one the offsets author, measured from the new
    // reference member: station(m) = reference + (offset(m) − offset(reference)),
    // which is exactly the spread the group stood in before control moved.
    assert_eq!(
        spread(&stations(&session)),
        held_spread,
        "the formation re-anchored without deforming",
    );
    // And the anchor, which now holds a station rather than steering, has not
    // drifted away: the separated flag is defined for it under the same rule.
    let apart = field::separated_forms(&session.run().expect("a run").state().now);
    assert!(apart.iter().all(|held| !*held), "no member stands separated");
}

// ---------------------------------------------------------------------------
// A chapter that places several Forms
// ---------------------------------------------------------------------------

/// The opening chapter's bytes with two more Form placements standing beside
/// its own: one controllable Form a layer down, and one the chapter marks
/// un-controllable.
///
/// The edit is made here rather than in the shipped content because what is
/// under test is the engine's reading of the seam rather than any chapter's
/// authoring of it — the chapter that authors several controllable Forms in
/// earnest is The Mesh, and its own file reads them there.
fn several_placements() -> Vec<String> {
    const EXTRA: &str = ",\n    { \"id\": 5, \"node\": 20, \"controlled\": false, \
        \"layer\": 1, \"pos\": { \"x\": 49807360, \"y\": 157286400 } },\n    \
        { \"id\": 6, \"node\": 21, \"controlled\": false, \"controllable\": false, \
        \"layer\": 0, \"pos\": { \"x\": 58982400, \"y\": 157286400 } }";
    let mut listed: Vec<String> = support::files().iter().map(|file| file.to_string()).collect();
    listed[0] = with_placements(&listed[0], EXTRA);
    listed
}

/// One chapter's bytes with more placements inserted into its `forms` list.
///
/// The insertion point is derived from the key rather than matched against the
/// placement the chapter happens to author, so re-authoring a chapter's opening
/// Form never breaks this file.
fn with_placements(chapter: &str, extra: &str) -> String {
    let opens = chapter.find("\"forms\": [").expect("a chapter authors a forms list");
    let closes = opens + chapter[opens..].find(']').expect("the list closes");
    format!("{}{}{}", &chapter[..closes], extra, &chapter[closes..])
}

fn opened_on_several(form: &str) -> Session {
    let mut session = Session::new(&support::worker_init_of(&several_placements()))
        .expect("versions agree");
    let answer = session.command(
        "init_run",
        &format!("{{\"form\":\"{form}\",\"mode\":\"new\",\"run_id\":\"{KEY}\"}}"),
    );
    assert!(answer.contains("\"ok\":true"), "a widened chapter opens a run: {answer}");
    session
}

#[test]
fn a_handoff_to_a_form_the_chapter_marks_un_controllable_is_refused() {
    let mut session = opened_on_several("thread");
    let ids: Vec<u8> = session
        .run()
        .expect("a run")
        .state()
        .now
        .forms
        .iter()
        .map(|form| form.id)
        .collect();
    assert_eq!(ids, vec![1, 5, 6], "three placements stand, in the authored order");
    let (seq, at) = entered(&mut session, 3);

    // The Form the chapter authored `controllable: false` on. It stands, so
    // this is not `not_found`; the chapter refuses it, so it is `validation`.
    assert_eq!(
        refusal(&session.command("input_frame", &asking(seq, at + FRAME_US, &handoff_of(6)))),
        "validation",
        "an un-controllable Form is refused",
    );
    assert_eq!(controlled_id(&session), 1, "and control has not moved");

    // The one beside it, authored with no flag at all, is controllable: absent
    // means true, which is what keeps every chapter authored before the
    // Handoff reading exactly as it did. (A refused frame has spent its own
    // sequence number, so the next one carries the next.)
    let answer = session.command("input_frame", &asking(seq + 1, at + 2 * FRAME_US, &handoff_of(5)));
    assert!(answer.contains("\"ok\":true"), "{answer}");
    assert_eq!(controlled_id(&session), 5);
}

/// The reserve one frame draws at each Node, by Node identifier — the ports
/// section of the frame the shell reads, decoded through its own section table.
fn drawn_reserves(session: &Session) -> Vec<(u32, u16)> {
    const PORT_RECORD: usize = 16;
    let view = session.frame_view();
    for place in 0..usize::from(view[20]) {
        let entry = 32 + place * 8;
        // The ports section is kind 2 of the closed set the encoder writes.
        if view[entry] != 2 {
            continue;
        }
        let count = usize::from(u16::from_le_bytes([view[entry + 2], view[entry + 3]]));
        let at = u32::from_le_bytes([
            view[entry + 4],
            view[entry + 5],
            view[entry + 6],
            view[entry + 7],
        ]) as usize;
        return (0..count)
            .map(|held| {
                let record = at + held * PORT_RECORD;
                let node = u32::from_le_bytes([
                    view[record],
                    view[record + 1],
                    view[record + 2],
                    view[record + 3],
                ]);
                (node, u16::from_le_bytes([view[record + 12], view[record + 13]]))
            })
            .collect();
    }
    panic!("a frame carries a ports section");
}

/// The Node one Form of the run stands on.
fn node_of(session: &Session, id: u8) -> u32 {
    session
        .run()
        .expect("a run")
        .state()
        .now
        .forms
        .iter()
        .find(|form| form.id == id)
        .expect("the Form stands")
        .node
}

#[test]
fn the_run_reserve_is_drawn_at_the_form_carrying_control() {
    // The stored reserve is the run's rather than any one Form's: `establish`
    // stands the whole of it with the Form the chapter opened control on and
    // opens every other placement holding none. So the frame draws it at the
    // Node of the Form control stands on *now*, and a Handoff carries the
    // reading with it rather than leaving it beside a Form nobody is steering.
    let mut session = opened_on_several("vault");
    let opened_on = node_of(&session, 1);
    let taking = node_of(&session, 5);

    let before = drawn_reserves(&session);
    let read = |drawn: &[(u32, u16)], node: u32| -> u16 {
        drawn.iter().find(|held| held.0 == node).expect("the Node stands").1
    };
    let held = read(&before, opened_on);
    assert!(held > 0, "the run opens holding a reserve: {before:?}");
    assert!(
        before.iter().all(|(node, drawn)| *node == opened_on || *drawn == 0),
        "and holds it in one place: {before:?}",
    );

    let (seq, at) = entered(&mut session, 3);
    let answer = session.command("input_frame", &asking(seq, at + FRAME_US, &handoff_of(5)));
    assert!(answer.contains("\"ok\":true"), "{answer}");

    let after = drawn_reserves(&session);
    assert_eq!(read(&after, taking), held, "the reserve is drawn where control stands now");
    assert_eq!(read(&after, opened_on), 0, "and no longer where the chapter opened it");
}

#[test]
fn a_handoff_under_content_whose_hash_has_moved_reads_the_authored_default() {
    // `controllable` is validation data rather than step data: content-derived,
    // never serialized, read at the moment the request arrives. So a run
    // carried on under content whose hash has moved has no authored gate to
    // read at all — and what stands then is the shape's own default, absent
    // means true, rather than its inverse. The standing rule for changed
    // content is that the run carries on, and a run that can never move
    // control has not carried on.
    let mut session = opened("chorus");
    let ids: Vec<u8> = session
        .run()
        .expect("a run")
        .state()
        .now
        .forms
        .iter()
        .map(|form| form.id)
        .collect();
    assert_eq!(ids.len(), 4, "Chorus stands four Forms");
    let file = exported(&mut session);

    // A session whose content differs by a chapter's bytes, reading the same
    // file back: the run carries on under a hash that is not this session's.
    let widened = several_placements();
    let digest = parse(&support::bundle_of(support::MANIFEST, &widened))
        .expect("canonical")
        .get("hash")
        .and_then(Json::as_text)
        .expect("a digest")
        .to_string();
    let mut carried = Session::new(&support::worker_init_of(&widened)).expect("versions agree");
    let answer = carried.command("import_run", &import_body(&file));
    assert!(answer.contains("\"ok\":true"), "{answer}");
    assert_ne!(
        carried.run().expect("a run").state().scenario.content_hash(),
        digest,
        "the run stands under content whose hash has moved",
    );

    let (seq, at) = entered(&mut carried, 3);
    let taking = i64::from(ids[1]);
    let answer = carried.command("input_frame", &asking(seq, at + FRAME_US, &handoff_of(taking)));
    assert!(answer.contains("\"ok\":true"), "a Handoff still moves control: {answer}");
    assert_eq!(controlled_id(&carried), ids[1]);
}

#[test]
fn the_frame_the_handoff_rides_carries_the_camera_to_the_new_form() {
    let mut session = opened_on_several("thread");
    let (seq, at) = entered(&mut session, 3);
    assert_eq!(session.frame_view()[15], 0, "the camera stands on the controlled Form's layer");

    let answer = session.command("input_frame", &asking(seq, at + FRAME_US, &handoff_of(5)));
    assert!(answer.contains("\"ok\":true"), "{answer}");
    assert_eq!(
        session.frame_view()[15],
        1,
        "on the frame control moved on, the camera moved with it",
    );
}

#[test]
fn a_handoff_away_from_the_linked_group_leaves_it_holding_formation() {
    let mut session = opened_on_several("chorus");
    let ids: Vec<u8> = session
        .run()
        .expect("a run")
        .state()
        .now
        .forms
        .iter()
        .map(|form| form.id)
        .collect();
    assert_eq!(
        ids,
        vec![1, 5, 6, 7, 8, 9],
        "the linked Forms take the identifiers above the highest the chapter authored",
    );

    let spread = |held: &[(u8, Vec2)]| -> Vec<(i64, i64)> {
        held.windows(2).map(|pair| (pair[1].1.x - pair[0].1.x, pair[1].1.y - pair[0].1.y)).collect()
    };
    let members = |held: &[(u8, Vec2)]| -> Vec<(u8, Vec2)> {
        held.iter().copied().filter(|(id, _)| matches!(id, 1 | 7 | 8 | 9)).collect()
    };
    // The formation as the chapter placed it: every member on its authored
    // offset from the anchor. That is the spread the group settles back onto
    // once the anchor has come to rest, and the one this test holds it to.
    let held_spread = spread(&members(&stations(&session)));

    let mut seq = 1u32;
    // A steer held all the way into the Handoff. A released control makes this
    // reading vacuous — nothing is under way, and a velocity that never decays
    // stands indistinguishable from one already at rest.
    for _ in 0..60 {
        session.command("input_frame", &steered(seq, 1, i64::from(seq) * FRAME_US, 9_000, 0));
        seq += 1;
    }
    let under_way = moving_at(&session, 1);
    assert_eq!(
        under_way,
        Vec2::new(360_000, 0),
        "the anchor carries the held steer's velocity into the Handoff",
    );
    let left_at = placed_at(&session, 1);

    // Control goes to a Form standing outside the group entirely.
    let opened_at = i64::from(seq) * FRAME_US;
    session.command("input_frame", &frame(seq, 0, opened_at, true));
    seq += 1;
    session.command("input_frame", &frame(seq, 0, opened_at + RAMP_US, false));
    seq += 1;
    let answer =
        session.command("input_frame", &asking(seq, opened_at + RAMP_US + FRAME_US, &handoff_of(5)));
    assert!(answer.contains("\"ok\":true"), "{answer}");
    seq += 1;
    session.command("input_frame", &frame(seq, 0, opened_at + RAMP_US + 2 * FRAME_US, true));
    seq += 1;
    session.command(
        "input_frame",
        &frame(seq, 0, opened_at + 2 * RAMP_US + 2 * FRAME_US, false),
    );
    seq += 1;
    for _ in 0..40 {
        session.command("input_frame", &frame(seq, 30, opened_at + i64::from(seq) * FRAME_US, false));
        seq += 1;
    }

    // The anchor coasts to rest under the locked damping rather than carrying
    // the velocity it was handed off under for the whole of the run.
    assert_eq!(moving_at(&session, 1), Vec2::default(), "the anchor came to rest");
    // And the coast is short: the damper takes a quarter of the velocity every
    // step, so the whole of it is the geometric tail rather than open-ended
    // drift. Measured at 16.5 units on this steer — pinned to a band so the
    // reading stays a coast rather than a journey.
    let coasted = placed_at(&session, 1).x - left_at.x;
    assert!(
        (16 * ONE_UNIT..18 * ONE_UNIT).contains(&coasted),
        "the anchor coasts the damper's tail and stops: {coasted} raw",
    );

    // The group settles back onto the authored spread around its own anchor,
    // to within the station deadband: a member closer to its station than
    // `reach / 32768` — 640 raw under the locked reach and the default scale —
    // names a control that truncates to zero and parks there, so a spread
    // reading, being a difference of two such, stands within twice that. The
    // widest departure measured here is 638 raw, a hundredth of one unit.
    const STATION_DEADBAND: i64 = 640;
    let carried = spread(&members(&stations(&session)));
    assert_eq!(carried.len(), held_spread.len(), "the same members stand");
    for (settled, authored) in carried.iter().zip(held_spread.iter()) {
        assert!(
            (settled.0 - authored.0).abs() <= 2 * STATION_DEADBAND
                && (settled.1 - authored.1).abs() <= 2 * STATION_DEADBAND,
            "the group holds its formation around its own anchor while control \
             is away: settled {settled:?} against the authored {authored:?}",
        );
    }
    let apart = field::separated_forms(&session.run().expect("a run").state().now);
    assert!(apart.iter().all(|held| !*held), "and nothing separated from it");
}

// ---------------------------------------------------------------------------
// The content the seam is authored in
// ---------------------------------------------------------------------------

/// The reason one edited campaign is refused for, read through the whole of
/// `read_bundle`. Every edit below is made to the fixture chapter, whose
/// literals exist to be broken.
fn refused(edit: &dyn Fn(&str) -> String) -> String {
    let mut files = support::fixture_files();
    let place = support::FIXTURE_PLACE;
    let held = edit(&files[place]);
    assert_ne!(held, files[place], "the edit found something to change");
    files[place] = held;
    let bundle = support::bundle_of(support::MANIFEST, &files);
    let fault = field_game_core::content::read_bundle(&parse(&bundle).expect("canonical"))
        .expect_err("the edited campaign is refused");
    assert_eq!(fault.code(), field_game_core::fault::Code::ContentInvalid);
    fault.detail().expect("a named diagnostic").to_string()
}

#[test]
fn the_authored_seam_answers_only_for_the_forms_its_field_accounts_for() {
    let bundle = support::bundle_of(support::MANIFEST, &several_placements());
    let content = field_game_core::content::read_bundle(&parse(&bundle).expect("canonical"))
        .expect("the widened campaign reads");
    let chapter = content.chapter(0).expect("a chapter");

    // The placements the chapter authored, each on its own flag.
    assert_eq!(chapter.controllable(1), Some(true), "the placement carrying no flag at all");
    assert_eq!(chapter.controllable(5), Some(true));
    assert_eq!(chapter.controllable(6), Some(false), "the placement authored un-controllable");

    // The linked members a `linked_forms` selection stands up take the
    // identifiers above the highest authored one, and answer with the flag of
    // the placement they stand from.
    assert_eq!(chapter.controllable(7), Some(true));

    // And an identifier the Field accounts for neither way answers None rather
    // than borrowing the flag of a placement that has nothing to do with it.
    assert_eq!(chapter.controllable(0), None);
    assert_eq!(chapter.controllable(3), None);
}

#[test]
fn a_chapter_that_leaves_no_form_controllable_is_refused_at_load() {
    // A chapter whose control no Handoff could ever move has authored the seam
    // shut, and that is content this build refuses rather than a run it opens.
    assert_eq!(
        refused(&|chapter: &str| chapter
            .replace("\"controlled\": true", "\"controlled\": true, \"controllable\": false")),
        "{\"reason\":\"controllable\"}",
    );
}

#[test]
fn a_chapter_that_places_two_controlled_forms_is_refused_at_load() {
    // Exactly one Form carries control at every step in version 1: the frozen
    // `InputFrame` has one steer vector to consume.
    assert_eq!(
        refused(&|chapter: &str| with_placements(
            chapter,
            ",\n    { \"id\": 5, \"node\": 20, \"controlled\": true, \"layer\": 0, \
             \"pos\": { \"x\": 58982400, \"y\": 157286400 } }",
        )),
        "{\"reason\":\"controlled\"}",
    );
}

#[test]
fn a_chapter_that_places_two_forms_out_of_order_is_refused_at_load() {
    // The Field's own validation asks for ascending Form identifiers, and the
    // linked Forms a selection stands up take the ones above the highest
    // authored — so the authored order is the Field's order.
    assert_eq!(
        refused(&|chapter: &str| with_placements(
            chapter,
            ",\n    { \"id\": 0, \"node\": 20, \"controlled\": false, \"layer\": 0, \
             \"pos\": { \"x\": 58982400, \"y\": 157286400 } }",
        )),
        "{\"reason\":\"id\"}",
    );
}

#[test]
fn a_handoff_and_a_handoff_back_leave_the_formation_where_it_stood() {
    let mut session = opened("chorus");
    let mut seq = 1u32;
    for _ in 0..30 {
        session.command("input_frame", &frame(seq, 30, i64::from(seq) * FRAME_US, false));
        seq += 1;
    }
    let before = stations(&session);

    let hand = |session: &mut Session, seq: &mut u32, id: i64| {
        let opened_at = i64::from(*seq) * FRAME_US;
        session.command("input_frame", &frame(*seq, 0, opened_at, true));
        *seq += 1;
        session.command("input_frame", &frame(*seq, 0, opened_at + RAMP_US, false));
        *seq += 1;
        let answer = session.command(
            "input_frame",
            &asking(*seq, opened_at + RAMP_US + FRAME_US, &handoff_of(id)),
        );
        assert!(answer.contains("\"ok\":true"), "{answer}");
        *seq += 1;
        session.command("input_frame", &frame(*seq, 0, opened_at + RAMP_US + 2 * FRAME_US, true));
        *seq += 1;
        session.command(
            "input_frame",
            &frame(*seq, 0, opened_at + 2 * RAMP_US + 2 * FRAME_US, false),
        );
        *seq += 1;
    };
    hand(&mut session, &mut seq, 2);
    hand(&mut session, &mut seq, 1);
    assert_eq!(controlled_id(&session), 1, "control came back");
    assert_eq!(
        stations(&session),
        before,
        "no simulated step ran, so nothing moved on either visit",
    );
}
