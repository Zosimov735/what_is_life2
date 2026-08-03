//! The staged pressures, driven end to end over the authored content.
//!
//! `the_pull` schedules one pressure — Interference, primary, aimed at
//! node 5, requested for step 900 — and the authored table gives it 90 steps
//! of signal, 240 of pressure, 120 of crisis, and 90 of resolution, so the
//! whole lifecycle sits inside the first 1440 steps of a run. The tests below
//! drive exactly that: the seat the schedule takes at open, the admission at
//! its step, the derived stages, the removal when the last stage is spent,
//! and the byte-equivalence contract through every one of those boundaries —
//! which is the determinism work ARCHITECTURE.md's displacement extension
//! point made mandatory the moment the list became nonempty.

use field_game_core::json::{parse, Json};
use field_game_core::pressure::{
    self, advance_pressures, Pressure, PressureContent, PressureState, Schedule, Stage, StageRow,
    Staged, Target, TargetKind, ACTIVE_CAP,
};
use field_game_core::state::AUTOSAVE_STEPS;
use field_game_core::Session;

mod support;

const KEY: &str = "5566aabbccdd0011";

/// Where the authored schedule puts Interference's seat.
const START: u32 = 900;

/// The authored stage boundaries, from `content/pressures/interference.json`:
/// signal 90, pressure 240, crisis 120, resolution 90.
const SIGNAL_UNTIL: u32 = START + 90;
const PRESSURE_UNTIL: u32 = SIGNAL_UNTIL + 240;
const CRISIS_UNTIL: u32 = PRESSURE_UNTIL + 120;
const SPENT_AT: u32 = CRISIS_UNTIL + 90;

/// Where the chapter's second entry stands, and when it is spent: Drift for the
/// final challenge, from `content/pressures/drift.json` — signal 150, pressure
/// 360, crisis 180, resolution 120.
const DRIFT_START: u32 = 25_333;
const DRIFT_SPENT_AT: u32 = DRIFT_START + 150 + 360 + 180 + 120;

fn opened(key: &str) -> Session {
    let mut session = Session::new(&support::worker_init()).expect("versions agree");
    let answer = session.command(
        "init_run",
        &format!("{{\"form\":\"thread\",\"mode\":\"new\",\"run_id\":\"{key}\"}}"),
    );
    assert!(answer.starts_with("{\"body\":"), "a new run opens: {answer}");
    session
}

/// One neutral frame of input, advancing an exact step count.
fn frame(seq: u32, steps: u16) -> String {
    format!(
        "{{\"advance_steps\":{steps},\"depth_key\":0,\"inspect\":null,\"pause\":false,\
          \"pulse_held\":false,\"pulse_release\":false,\"seq\":{seq},\"steer_x\":0,\
          \"steer_y\":0,\"t_us\":{stamp},\"toggle_still\":false,\"wheel\":0}}",
        stamp = i64::from(seq) * 33_333
    )
}

/// Advances a session to an exact step, in batches the frame cap allows.
fn advance_to(session: &mut Session, seq: &mut u32, step: u32) {
    while session.step() < step {
        let left = (step - session.step()).min(1_800) as u16;
        session.command("input_frame", &frame(*seq, left));
        *seq += 1;
    }
    assert_eq!(session.step(), step);
}

fn body(answer: &str) -> Json {
    let parsed = parse(answer).expect("a response is canonical JSON");
    assert_eq!(parsed.get("ok"), Some(&Json::Bool(true)), "{answer}");
    parsed.get("body").expect("a successful response carries a body").clone()
}

fn text_of(value: &Json, key: &str) -> String {
    value.get(key).and_then(Json::as_text).expect("a text field").to_string()
}

fn exported(session: &mut Session) -> String {
    text_of(&body(&session.command("export_run", "{}")), "text")
}

fn import_body(text: &str) -> String {
    let mut out = String::from("{\"text\":");
    field_game_core::json::write_text(&mut out, text);
    out.push('}');
    out
}

/// An export file with `field.now.prev_assembly_step` set to a step and the
/// digest recomputed — the bridge tests' own normalization, restated here so
/// a restored export compares against the file it was restored from. The
/// keyframe's copy of the field is untouched, exactly as the locked
/// normalization touches only the state the run stands on.
fn normalize_prev_assembly(file: &Json, step: i64) -> Json {
    normalize_prev_assembly_in(file, step, false)
}

/// The same normalization, optionally applied to the trajectory keyframe's
/// copy of the field too: a boundary — a settled pressure list exactly as a
/// committed change — restarts the keyframe as a clone of `now`, so a run
/// that restored before a boundary carries the normalized value in both
/// copies while a run that never restored carries it in neither.
fn normalize_prev_assembly_in(file: &Json, step: i64, keyframe_too: bool) -> Json {
    let mut rebuilt = file.clone();
    let Json::Map(pairs) = &mut rebuilt else {
        panic!("an export file is an object");
    };
    for (key, value) in pairs.iter_mut() {
        if key == "payload" {
            let Json::Map(payload) = value else { panic!("a payload is an object") };
            for (name, part) in payload.iter_mut() {
                if name != "field" {
                    continue;
                }
                let Json::Map(field) = part else { panic!("a field is an object") };
                for (which, state) in field.iter_mut() {
                    if which == "now" {
                        let Json::Map(now) = state else { panic!("a state is an object") };
                        for (held, at) in now.iter_mut() {
                            if held == "prev_assembly_step" {
                                *at = Json::Int(step);
                            }
                        }
                    }
                    if keyframe_too && which == "trace" {
                        let Json::Map(trace) = state else { panic!("a trace is an object") };
                        for (part, inside) in trace.iter_mut() {
                            if part != "keyframe" {
                                continue;
                            }
                            let Json::Map(now) = inside else { panic!("a state is an object") };
                            for (held, at) in now.iter_mut() {
                                if held == "prev_assembly_step" {
                                    *at = Json::Int(step);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    let Json::Map(pairs) = &mut rebuilt else { unreachable!() };
    let payload = pairs
        .iter()
        .find(|(key, _)| key == "payload")
        .map(|(_, value)| value.clone())
        .expect("a payload");
    let mut bytes = String::new();
    field_game_core::json::write_value(&mut bytes, &payload).expect("canonical");
    let digest =
        field_game_core::json::hex_bytes(&field_game_core::sha256::digest(bytes.as_bytes()));
    for (key, value) in pairs.iter_mut() {
        if key == "payload_sha256" {
            *value = Json::Text(digest.clone());
        }
    }
    rebuilt
}

/// The staged Interference a session's run holds, cloned out.
///
/// `the_pull` schedules two entries — Interference for the opening sequence and
/// Drift for its final challenge, far later — and this file is about the first
/// of them, so the entry is found by kind rather than by its place in the list.
fn the_pressure(session: &Session) -> PressureState {
    let held = &session.run().expect("a run is loaded").state().pressures;
    held.iter()
        .find(|entry| entry.pressure == Pressure::Interference)
        .expect("the_pull schedules Interference")
        .clone()
}

// ---------------------------------------------------------------------------
// Start, progress, resolve
// ---------------------------------------------------------------------------

#[test]
fn the_authored_schedule_is_seated_queued_and_the_event_says_so() {
    let mut session = opened(KEY);
    let seated = the_pressure(&session);
    assert_eq!(seated.pressure, Pressure::Interference);
    assert!(seated.queued, "an entry stands queued until its step comes");
    assert!(seated.primary);
    assert_eq!(seated.stage, Stage::Signal);
    assert_eq!(seated.level, 0, "no level before activation");
    assert_eq!(seated.start_step, START);
    assert_eq!(seated.target.kind, TargetKind::Node);
    assert_eq!(seated.target.id, Some(5));

    // The shell was told with the locked event body: the full list after the
    // change.
    let events = session.take_events();
    assert!(
        events.contains("\"ev\":\"pressure_changed\""),
        "the seat is a change of the list: {events}"
    );
    assert!(events.contains("\"pressures\":[{"), "the body carries the whole list: {events}");

    // And the payload carries the queued entry, in the locked shape. The list
    // is ascending in the closed set's own order, so Interference stands first
    // and the chapter's Drift entry stands behind it.
    let payload = session.run().expect("a run is loaded").state().payload();
    assert!(
        payload.contains(
            "\"pressures\":[{\"bound\":null,\"displaced\":null,\"level\":0,\
             \"pressure\":\"interference\",\"primary\":true,\
             \"queued\":true,\"stage\":\"signal\",\"start_step\":900,\
             \"target\":{\"id\":5,\"t\":\"node\"}},"
        ),
        "{payload}"
    );
}

#[test]
fn a_pressure_starts_progresses_resolves_and_leaves_through_its_authored_stages() {
    let mut session = opened(KEY);
    let mut seq = 1;

    // Before its step: queued, whatever the frame count. The seat is taken
    // at the boundary before the step it acts in, so the last step the list
    // shows it queued after is two before its start.
    advance_to(&mut session, &mut seq, START - 2);
    assert!(the_pressure(&session).queued);

    // The boundary at the close of step 899 admits it for step 900 — it acts
    // first at its authored `start_step`, which is what activating at
    // `start_step` means — and the seat takes the signal stage's own level.
    advance_to(&mut session, &mut seq, START - 1);
    let seated = the_pressure(&session);
    assert!(!seated.queued, "the limit leaves a seat, so the boundary admits it");
    assert_eq!(seated.start_step, START, "an on-time seat keeps its authored step");
    advance_to(&mut session, &mut seq, START);
    let active = the_pressure(&session);
    assert!(!active.queued);
    assert_eq!(active.stage, Stage::Signal);
    assert_eq!(active.level, 16_384);
    session.take_events();

    // Each boundary turns the derived stage over, at the authored step.
    advance_to(&mut session, &mut seq, SIGNAL_UNTIL - 1);
    assert_eq!(the_pressure(&session).stage, Stage::Signal);
    advance_to(&mut session, &mut seq, SIGNAL_UNTIL);
    let pressing = the_pressure(&session);
    assert_eq!(pressing.stage, Stage::Pressure);
    assert_eq!(pressing.level, 39_322);
    let events = session.take_events();
    assert!(events.contains("pressure_changed"), "a stage turnover is told: {events}");

    advance_to(&mut session, &mut seq, PRESSURE_UNTIL);
    let crisis = the_pressure(&session);
    assert_eq!(crisis.stage, Stage::Crisis);
    assert_eq!(crisis.level, 58_982);

    advance_to(&mut session, &mut seq, CRISIS_UNTIL);
    let resolving = the_pressure(&session);
    assert_eq!(resolving.stage, Stage::Resolution);
    assert_eq!(resolving.level, 13_107);

    // Past the last stage the pressure is spent and leaves the list, and the
    // event carries what the removal left: the chapter's other entry, still
    // queued for the step far ahead that it was scheduled for.
    session.take_events();
    advance_to(&mut session, &mut seq, SPENT_AT);
    let left = session.run().expect("a run is loaded").state().pressures.clone();
    assert!(
        !left.iter().any(|held| held.pressure == Pressure::Interference),
        "a spent pressure is removed: {left:?}",
    );
    assert_eq!(left.len(), 1, "and what stands is the chapter's queued Drift: {left:?}");
    assert!(left[0].queued);
    let events = session.take_events();
    assert!(
        !events.contains("\"pressure\":\"interference\""),
        "the shortened list is told: {events}",
    );
}

#[test]
fn admission_and_removal_end_the_retained_window_and_a_stage_turnover_does_not() {
    let mut session = opened(KEY);
    let mut seq = 1;

    // Admission is a change of membership, applied at the boundary that
    // closes step 899 — the step before the pressure acts — so the retained
    // trajectory restarts on the state that boundary leaves.
    advance_to(&mut session, &mut seq, START + 5);
    let state = session.run().expect("a run is loaded").state();
    assert_eq!(
        state.trace.start_step,
        START - 1,
        "the window ended at the admission boundary and regrows from it"
    );

    // A stage turnover is derived state: the window did not end at 990. By
    // step 1150 the ordinary carriage has moved the keyframe past the
    // turnover — which it could only do if the turnover ended nothing, since
    // an ended window would restart the trajectory there and hold it.
    advance_to(&mut session, &mut seq, SIGNAL_UNTIL + 160);
    let state = session.run().expect("a run is loaded").state();
    assert!(
        state.trace.start_step > SIGNAL_UNTIL,
        "the ordinary lag passed the turnover without a restart: {}",
        state.trace.start_step
    );

    // Removal at 1440 is a change of membership again.
    advance_to(&mut session, &mut seq, SPENT_AT + 3);
    let state = session.run().expect("a run is loaded").state();
    assert_eq!(state.trace.start_step, SPENT_AT, "the window ended at the removal");
}

// ---------------------------------------------------------------------------
// Byte-equivalence through the lifecycle
// ---------------------------------------------------------------------------

#[test]
fn the_same_key_and_frames_agree_through_onset_crisis_and_resolution() {
    let played = |key: &str| -> String {
        let mut session = opened(key);
        let mut seq = 1;
        advance_to(&mut session, &mut seq, SPENT_AT + 30);
        session.run().expect("a run is loaded").state().payload()
    };
    let once = played(KEY);
    let again = played(KEY);
    assert_eq!(once, again, "byte-equivalent through the whole staged lifecycle");
    assert!(
        !once.contains("\"pressure\":\"interference\""),
        "and the lifecycle completed, leaving the list without it",
    );
}

#[test]
fn an_export_mid_crisis_imports_and_replays_to_the_same_bytes() {
    let mut source = opened(KEY);
    let mut seq = 1;
    advance_to(&mut source, &mut seq, PRESSURE_UNTIL + 40);
    assert_eq!(the_pressure(&source).stage, Stage::Crisis);
    let file = exported(&mut source);

    // The import validation accepts the staged list it once refused, and the
    // re-export differs only by the locked post-restore normalization.
    let mut imported = Session::new(&support::worker_init()).expect("versions agree");
    let answer = body(&imported.command("import_run", &import_body(&file)));
    assert_eq!(text_of(&answer, "run_id"), KEY);
    assert_eq!(the_pressure(&imported).stage, Stage::Crisis, "the crisis stands restored");
    let again = exported(&mut imported);
    let normalized = normalize_prev_assembly(
        &parse(&file).expect("canonical"),
        i64::from(PRESSURE_UNTIL + 40),
    );
    assert_eq!(
        parse(&again).expect("canonical"),
        normalized,
        "the re-export differs only by the normalization"
    );

    // Played on the same frames, the imported run and the source run walk the
    // same bytes through the crisis boundary and the removal.
    let drive = |session: &mut Session, from: u32| {
        let mut seq = from;
        advance_to(session, &mut seq, SPENT_AT + 10);
        session.run().expect("a run is loaded").state().payload()
    };
    let source_on = drive(&mut source, seq);
    let imported_on = drive(&mut imported, 1);
    // The imported run carries the one normalized value; everything else of
    // the two payloads is byte-identical, which the parsed comparison under
    // the same normalization states exactly.
    // Both runs crossed the removal boundary, which restarts the keyframe as
    // a clone of `now` — so the imported run carries the normalized value in
    // both copies, and the source in neither; the comparison normalizes both.
    let with_normalized = {
        let wrapped = format!(
            "{{\"payload\":{source_on},\"payload_sha256\":\"{:064}\"}}", 0
        );
        let rebuilt = normalize_prev_assembly_in(
            &parse(&wrapped).expect("canonical"),
            i64::from(PRESSURE_UNTIL + 40),
            true,
        );
        rebuilt.get("payload").expect("a payload").clone()
    };
    assert_eq!(
        with_normalized,
        parse(&imported_on).expect("canonical"),
        "byte-equivalent through resolution and removal"
    );
}

#[test]
fn quick_retry_across_a_pressure_stage_change_is_byte_equivalent() {
    // The autosave cadence writes at step 900, which is the admission step:
    // the recorded payload carries the freshly active pressure.
    let script = |session: &mut Session, from: u32| -> String {
        let mut seq = from;
        advance_to(session, &mut seq, PRESSURE_UNTIL + 20);
        session.run().expect("a run is loaded").state().payload()
    };

    let mut straight = opened(KEY);
    let mut seq = 1;
    advance_to(&mut straight, &mut seq, AUTOSAVE_STEPS);
    let anchor_id = {
        let anchors = &straight.run().expect("a run is loaded").state().anchors;
        assert_eq!(anchors[0].step, AUTOSAVE_STEPS);
        anchors[0].anchor_id
    };
    let played_on = script(&mut straight, seq);

    // Retried from the record at 900 and driven by the same frames, the run
    // walks back through signal, pressure, and into crisis on the same bytes,
    // less only the one locked normalization the restore applies.
    let mut retried = opened(KEY);
    let mut seq = 1;
    advance_to(&mut retried, &mut seq, AUTOSAVE_STEPS);
    retried.command("restore_checkpoint", &format!("{{\"anchor_id\":{anchor_id}}}"));
    assert_eq!(the_pressure(&retried).stage, Stage::Signal, "the record held the fresh seat");
    let replayed = script(&mut retried, seq);
    assert_eq!(
        replayed.replace(
            &format!("\"prev_assembly_step\":{AUTOSAVE_STEPS}"),
            "\"prev_assembly_step\":null"
        ),
        played_on,
        "Quick Retry across the stage change reproduces the bytes"
    );
}

// ---------------------------------------------------------------------------
// The limits, and the stage machine's own rules
// ---------------------------------------------------------------------------

/// A table whose four stages carry one level for authored dwells.
fn table(pressure: Pressure, target: TargetKind, dwell: i64, level: i64) -> PressureContent {
    PressureContent {
        pressure,
        target,
        stages: [
            StageRow { stage: Stage::Signal, level, steps: dwell },
            StageRow { stage: Stage::Pressure, level, steps: dwell },
            StageRow { stage: Stage::Crisis, level, steps: dwell },
            StageRow { stage: Stage::Resolution, level, steps: dwell },
        ],
    }
}

fn queued(pressure: Pressure, start_step: u32, primary: bool) -> PressureState {
    PressureState::queued_at(pressure, start_step, primary, Target::none())
}

#[test]
fn at_most_two_stand_active_and_a_refused_entry_waits_for_a_seat() {
    let schedule = Schedule::of(vec![
        table(Pressure::Drain, TargetKind::None, 10, 30_000),
        table(Pressure::Noise, TargetKind::None, 1_000, 30_000),
        table(Pressure::Fracture, TargetKind::None, 1_000, 30_000),
    ])
    .expect("one table per pressure");
    let mut pressures = vec![
        queued(Pressure::Drain, 5, false),
        queued(Pressure::Noise, 5, false),
        queued(Pressure::Fracture, 5, false),
    ];

    // At the boundary before step 5 the first two of the closed set take the
    // two seats; the third request is refused and stands queued — never
    // dropped. The seat takes the opening stage's own curve level.
    let settled = pressure::settle_boundary(&mut pressures, &schedule, &[], &[], 5);
    assert!(settled.changed);
    assert_eq!(
        settled.admitted,
        vec![(Pressure::Drain.ordinal(), Stage::Signal), (Pressure::Noise.ordinal(), Stage::Signal)]
    );
    let active: Vec<Pressure> = pressures
        .iter()
        .filter(|held| !held.queued)
        .map(|held| held.pressure)
        .collect();
    assert_eq!(active, vec![Pressure::Drain, Pressure::Noise]);
    assert_eq!(active.len(), ACTIVE_CAP);
    assert!(pressures[2].queued, "the limit admits per seat, and the refusal queues");
    assert_eq!(pressures[0].level, 30_000, "a seat takes the opening curve level");

    // Drain spends its 40 steps: the step that walks past the end reports it,
    // the boundary removes it, and the freed seat goes to the waiting entry —
    // whose stage clock starts at its admission rather than at its request,
    // the rebase that keeps a late admission from arriving mid-stage.
    let staged = advance_pressures(&mut pressures, &schedule, 45);
    assert_eq!(staged.spent, vec![Pressure::Drain.ordinal()]);
    let settled = pressure::settle_boundary(&mut pressures, &schedule, &staged.spent, &[], 46);
    assert!(settled.changed, "a removal and an admission both change the list");
    assert_eq!(pressures.len(), 2, "the spent pressure left");
    let fracture = pressures
        .iter()
        .find(|held| held.pressure == Pressure::Fracture)
        .expect("the waiting entry");
    assert!(!fracture.queued);
    assert_eq!(fracture.start_step, 46, "admission rebases the stage clock");
    assert_eq!(fracture.stage, Stage::Signal);
    assert_eq!(fracture.level, 30_000);
}

#[test]
fn at_most_one_stands_primary_and_a_second_waits_for_the_seat() {
    let schedule = Schedule::of(vec![
        table(Pressure::Drain, TargetKind::None, 10, 30_000),
        table(Pressure::Noise, TargetKind::None, 1_000, 30_000),
    ])
    .expect("one table per pressure");
    let mut pressures =
        vec![queued(Pressure::Drain, 0, true), queued(Pressure::Noise, 0, true)];

    pressure::settle_boundary(&mut pressures, &schedule, &[], &[], 1);
    assert!(!pressures[0].queued, "the first primary takes the seat");
    assert!(pressures[1].queued, "the second waits for it despite a free active seat");

    // The primary seat frees when Drain spends, and the waiting primary takes
    // it at the same boundary.
    let staged = advance_pressures(&mut pressures, &schedule, 41);
    assert_eq!(staged.spent, vec![Pressure::Drain.ordinal()]);
    pressure::settle_boundary(&mut pressures, &schedule, &staged.spent, &[], 42);
    assert_eq!(pressures.len(), 1);
    assert!(!pressures[0].queued);
    assert!(pressures[0].primary);
    assert_eq!(pressures[0].pressure, Pressure::Noise);
}

#[test]
fn a_stage_turnover_is_reported_without_a_boundary_and_a_press_is_deferred_to_one() {
    let schedule =
        Schedule::of(vec![table(Pressure::Drift, TargetKind::None, 10, 20_000)]).expect("one table");
    let mut pressures = vec![PressureState {
        queued: false,
        ..queued(Pressure::Drift, 1, false)
    }];
    advance_pressures(&mut pressures, &schedule, 1);

    // Crossing into the second stage: a stage change, derived in the step,
    // with nothing for the boundary to apply.
    let staged = advance_pressures(&mut pressures, &schedule, 11);
    assert_eq!(
        staged,
        Staged {
            stage: true,
            entered: vec![(5, Stage::Pressure)],
            spent: Vec::new(),
            pressed: Vec::new(),
        }
    );
    assert_eq!(pressures[0].stage, Stage::Pressure);

    // Standing inside a stage: nothing at all.
    let staged = advance_pressures(&mut pressures, &schedule, 12);
    assert_eq!(staged, Staged::default());

    // A pressed floor is applied by the boundary, not by the step: the write
    // lands after the window ends, which is what keeps it derivable inside
    // every retained window.
    let floor = pressure::Displaced { stage: Stage::Pressure, level: 15_000 };
    let settled = pressure::settle_boundary(
        &mut pressures,
        &schedule,
        &[],
        &[(Pressure::Drift.ordinal(), floor)],
        13,
    );
    assert!(settled.changed);
    assert_eq!(pressures[0].displaced, Some(floor));
}

// ---------------------------------------------------------------------------
// The locked shape, read and refused
// ---------------------------------------------------------------------------

/// A pressures list parsed out of canonical text.
fn read_list_of(text: &str) -> Result<Vec<PressureState>, field_game_core::fault::Fault> {
    let wrapped = format!("{{\"pressures\":{text}}}");
    let parsed = parse(&wrapped).expect("the fixture parses");
    pressure::read_list(&parsed, "pressures")
}

const ENTRY: &str = "{\"bound\":null,\"displaced\":null,\"level\":100,\"pressure\":\"drain\",\
    \"primary\":false,\"queued\":false,\"stage\":\"signal\",\"start_step\":0,\
    \"target\":{\"id\":null,\"t\":\"none\"}}";

fn entry_of(pressure: &str, queued: bool, primary: bool) -> String {
    ENTRY
        .replace("\"drain\"", &format!("\"{pressure}\""))
        .replace("\"queued\":false", &format!("\"queued\":{queued}"))
        .replace("\"primary\":false", &format!("\"primary\":{primary}"))
}

#[test]
fn the_two_locked_invariants_and_the_ordering_are_validated() {
    // Three active is past the locked limit.
    let three = format!(
        "[{},{},{}]",
        entry_of("drain", false, false),
        entry_of("noise", false, false),
        entry_of("fracture", false, false)
    );
    assert!(read_list_of(&three).is_err(), "at most two active");

    // Two primary is past the locked limit, active or not.
    let primaries = format!(
        "[{},{}]",
        entry_of("drain", false, true),
        entry_of("noise", true, true)
    );
    assert!(read_list_of(&primaries).is_err(), "at most one primary");

    // The list ascends in the closed set's own order, each pressure at most
    // once.
    let unordered = format!(
        "[{},{}]",
        entry_of("noise", true, false),
        entry_of("drain", true, false)
    );
    assert!(read_list_of(&unordered).is_err(), "closed-set order");
    let doubled = format!(
        "[{},{}]",
        entry_of("drain", true, false),
        entry_of("drain", false, false)
    );
    assert!(read_list_of(&doubled).is_err(), "one seat per pressure");

    // A target kind that names something carries its identifier, and one that
    // names nothing carries none.
    let half = ENTRY.replace("{\"id\":null,\"t\":\"none\"}", "{\"id\":null,\"t\":\"node\"}");
    assert!(read_list_of(&format!("[{half}]")).is_err(), "a named kind carries an id");
    let extra = ENTRY.replace("{\"id\":null,\"t\":\"none\"}", "{\"id\":7,\"t\":\"none\"}");
    assert!(read_list_of(&format!("[{extra}]")).is_err(), "none carries none");

    // The well-formed single entry reads back and re-serializes byte for
    // byte.
    let list = read_list_of(&format!("[{ENTRY}]")).expect("a valid list reads");
    assert_eq!(pressure::written_list(&list), format!("[{ENTRY}]"));
}

// ---------------------------------------------------------------------------
// The render snapshot's section 6
// ---------------------------------------------------------------------------

#[test]
fn the_frame_carries_the_staged_pressures_in_the_locked_record_layout() {
    let mut session = opened(KEY);
    let mut seq = 1;
    advance_to(&mut session, &mut seq, PRESSURE_UNTIL + 1);
    assert_eq!(the_pressure(&session).stage, Stage::Crisis);

    let bytes = session.frame_view();
    assert_eq!(&bytes[0..4], b"FGF1");
    let sections = bytes[20] as usize;
    let mut found = None;
    for place in 0..sections {
        let at = 32 + place * 8;
        if bytes[at] == 6 {
            let count = u16::from_le_bytes([bytes[at + 2], bytes[at + 3]]);
            let offset = u32::from_le_bytes([
                bytes[at + 4],
                bytes[at + 5],
                bytes[at + 6],
                bytes[at + 7],
            ]) as usize;
            found = Some((count, offset));
        }
    }
    let (count, at) = found.expect("section 6 stands while a pressure does");
    // Two entries: the Interference this file drives, and the Drift the
    // chapter queues for its final challenge. The list is ascending in the
    // closed set's own order, so the record read below is the first of them.
    assert_eq!(count, 2);
    assert_eq!(bytes[at], 4, "interference is ordinal 4 of the closed set");
    assert_eq!(bytes[at + 1], 2, "crisis is stage 2");
    assert_eq!(bytes[at + 2], 1, "a node target is kind 1");
    assert_eq!(bytes[at + 3], 0, "active, not queued");
    let level = u16::from_le_bytes([bytes[at + 4], bytes[at + 5]]);
    assert_eq!(i64::from(level), 58_982, "the level as Q0.16");
    assert_eq!(u16::from_le_bytes([bytes[at + 6], bytes[at + 7]]), 0, "the zero pad");
    let target = u32::from_le_bytes([bytes[at + 8], bytes[at + 9], bytes[at + 10], bytes[at + 11]]);
    assert_eq!(target, 5, "the target's own identifier");

    // Once every lifecycle the chapter schedules is spent, the section leaves
    // the frame: the run is carried past the Drift the chapter queues for its
    // final challenge, so what is read here is the emptied list rather than a
    // shortened one.
    advance_to(&mut session, &mut seq, DRIFT_SPENT_AT + 1);
    assert!(session.run().expect("a run is loaded").state().pressures.is_empty());
    let bytes = session.frame_view();
    let sections = bytes[20] as usize;
    assert!(
        (0..sections).all(|place| bytes[32 + place * 8] != 6),
        "an empty section is left out"
    );
}

// ---------------------------------------------------------------------------
// Ranking over a pressure-bearing Field
// ---------------------------------------------------------------------------

#[test]
fn a_drawing_noise_pressure_spreads_the_confidence_ranges() {
    // The first non-degenerate confidence range a locked rule produces. The
    // Noise flow scale is the one drawing rule of version 1: the recorded
    // window drew its scales from the trajectory stream, every framework
    // sample replays under its own named stream and re-draws them — the
    // locked forecast distortion, with no second arithmetic — and where the
    // drawn scale narrows a Route at capacity the replayed windows genuinely
    // diverge. So the eight samples of a procedure finally disagree, and the
    // range [smallest, largest] opens.
    //
    // The Field is built to make the scale count: a full reserve behind a
    // small-capacity Route, so every step's flow is the drawn scale of the
    // capacity rather than the shortfall, under an active Noise pressure
    // whose level stands on top of a zero authored base — the pressure alone
    // is what makes the layer draw.
    use field_game_core::field::{
        BoundaryState, FieldLayer, FormState, PhysicalCompartment, PortState, RouteState,
        Unstaged,
    };
    use field_game_core::fx::{Vec2, ONE_UNIT};
    use field_game_core::rng::RngState;
    use field_game_core::state::{
        ControlState, FieldState, Progress, Trace, TraceStep, ViewDeclaration, FRAC_ONE,
    };

    let port_at = |node: u32, q_units: i64, x: i64| PortState {
        node,
        layer: 0,
        pos: Vec2::units(x, 1000),
        kind: field_game_core::field::NodeKind::Reserve,
        q: q_units * ONE_UNIT,
        capacity: 4_096 * ONE_UNIT,
        upkeep_rate: 0,
        open: true,
    };
    let mut keyframe = FieldState::opening();
    keyframe.layers = vec![FieldLayer {
        layer: 0,
        drain: 0,
        noise: 0,
        gain: 0,
        current_ids: Vec::new(),
        port_ids: vec![1, 2, 3],
    }];
    keyframe.ports =
        vec![port_at(1, 2_000, 1000), port_at(2, 500, 1600), port_at(3, 500, 2200)];
    // Two closed circuits: one internal to the standing inside {1, 2}, one
    // crossing to the outside Node 3. The recorded circulating flow is the
    // internal circuit's; severing the crossings leaves that circuit running
    // at the drawn scale of its capacity — which is where the draws narrow
    // the flows, and why the eight severance replays finally disagree.
    keyframe.routes = vec![
        RouteState { route: 1, tail: 1, head: 2, capacity: 16 * ONE_UNIT, flow: 0, formed_step: 0 },
        RouteState { route: 2, tail: 2, head: 1, capacity: 16 * ONE_UNIT, flow: 0, formed_step: 0 },
        RouteState { route: 3, tail: 1, head: 3, capacity: 16 * ONE_UNIT, flow: 0, formed_step: 0 },
        RouteState { route: 4, tail: 3, head: 1, capacity: 16 * ONE_UNIT, flow: 0, formed_step: 0 },
    ];
    keyframe.forms = Vec::<FormState>::new();
    keyframe.physical_compartment = PhysicalCompartment {
        members: vec![1, 2],
        leak_per_exposed_contact_per_step: 0,
    };
    keyframe.boundaries = BoundaryState { drawn: Vec::new(), authored: Vec::new() };
    keyframe.next_node_id = 4;
    keyframe.next_route_id = 5;
    field_game_core::field::validate(&keyframe).expect("the fixture is a valid Field");

    let bearing = PressureState {
        pressure: Pressure::Noise,
        stage: Stage::Pressure,
        level: 39_322,
        primary: true,
        queued: false,
        start_step: 0,
        target: Target { kind: TargetKind::Layer, id: Some(0) },
        displaced: None,
        bound: None,
    };
    let schedule = Schedule::of(vec![table(Pressure::Noise, TargetKind::Layer, 36_000, 39_322)])
        .expect("one table per pressure");

    let view = ViewDeclaration {
        inside: vec![1, 2],
        resolution: 1,
        window: 45,
        surround: field_game_core::state::Surround::Adjacent,
    };
    let mut now = keyframe.clone();
    let mut trace = Trace::opening(keyframe);
    let mut stream = field_game_core::rng::trajectory_stream("00aa00aa00aa00aa", 0);
    let mut staged = Unstaged {
        pressures: vec![bearing.clone()],
        schedule: schedule.clone(),
        stream: RngState::default(),
    };
    staged.stream = stream;
    for _ in 0..60 {
        let position = staged.stream;
        let outcome = field_game_core::field::advance(
            &mut now,
            ControlState::default(),
            FRAC_ONE,
            &mut staged.staging(),
        );
        trace.steps.push_back(TraceStep {
            step: now.step,
            rng: position,
            ctl: ControlState::default(),
            records: outcome.records,
        });
    }
    stream = staged.stream;
    let state = field_game_core::state::RunState {
        run_id: "00aa00aa00aa00aa".to_string(),
        rng: stream,
        spec: field_game_core::state::GeneratorSpec::new(support::content_hash(), schedule),
        branch_nonce: 0,
        progress: Progress::opening(),
        now,
        trace,
        view,
        slate: None,
        input_config: field_game_core::state::InputConfig::default_config(),
        pressures: vec![bearing],
        anchors: Vec::new(),
    };

    let mut slate = field_game_core::slate::assemble(&state);
    field_game_core::rank::evaluate(&state, &mut slate);
    let mut assigned = 0;
    let mut spread = 0;
    for candidate in &slate.candidates {
        for value in [
            &candidate.privilege.scale_stability,
            &candidate.privilege.shared_failure,
            &candidate.privilege.cut_impact,
            &candidate.privilege.boundary_sufficiency,
        ] {
            if let (Some(low), Some(high)) = (value.low, value.high) {
                assigned += 1;
                if low < high {
                    spread += 1;
                }
            }
        }
    }
    assert!(assigned > 0, "the evaluation assigned values over the pressure-bearing Field");
    assert!(
        spread > 0,
        "a drawing rule under the sample streams opens at least one confidence range",
    );

    // And the range that opened is Cut Impact's on the standing candidate:
    // its eight severance replays each re-draw the internal circuit's flow
    // scales from their own sample streams, so the stopped share genuinely
    // varies — the value stays a median with [smallest, largest] around it,
    // never a bare number.
    let standing = &slate.candidates[0].privilege.cut_impact;
    let (low, high) = (standing.low.expect("assigned"), standing.high.expect("assigned"));
    assert!(low < high, "the severance samples disagree: [{low}, {high}]");
    let median = standing.value.expect("assigned");
    assert!((low..=high).contains(&median), "the median sits inside its own range");
}

// ---------------------------------------------------------------------------
// The six effects, each read against its locked arithmetic
// ---------------------------------------------------------------------------

use field_game_core::field::{
    self as field_mod, BoundaryState, CurrentState, FieldLayer, NodeKind, PhysicalCompartment,
    PortState, RouteState, Unstaged,
};
use field_game_core::fx::{fixed_mul, Vec2, ONE_UNIT};
use field_game_core::rng::RngState;
use field_game_core::state::{ControlState, FieldState, FRAC_ONE};

/// A one-layer Field with the named reserves and routes, every Node open.
fn effect_field(
    holds: &[(u32, i64, i64, i64)],
    routes: &[(u32, u32, u32, i64)],
) -> FieldState {
    let mut field = FieldState::opening();
    field.layers = vec![FieldLayer {
        layer: 0,
        drain: 0,
        noise: 0,
        gain: 0,
        current_ids: Vec::new(),
        port_ids: holds.iter().map(|(node, ..)| *node).collect(),
    }];
    field.ports = holds
        .iter()
        .map(|(node, units, x, y)| PortState {
            node: *node,
            layer: 0,
            pos: Vec2::units(*x, *y),
            kind: NodeKind::Reserve,
            q: units * ONE_UNIT,
            capacity: 4_096 * ONE_UNIT,
            upkeep_rate: 0,
            open: true,
        })
        .collect();
    field.routes = routes
        .iter()
        .map(|(route, tail, head, capacity)| RouteState {
            route: *route,
            tail: *tail,
            head: *head,
            capacity: capacity * ONE_UNIT,
            flow: 0,
            formed_step: 0,
        })
        .collect();
    field.physical_compartment = PhysicalCompartment {
        members: field.ports.iter().map(|port| port.node).collect(),
        leak_per_exposed_contact_per_step: 0,
    };
    field.boundaries = BoundaryState { drawn: Vec::new(), authored: Vec::new() };
    field.next_node_id = holds.iter().map(|(node, ..)| *node).max().unwrap_or(0) + 1;
    field.next_route_id = routes.iter().map(|(route, ..)| *route).max().unwrap_or(0) + 1;
    field_game_core::field::validate(&field).expect("the fixture is a valid Field");
    field
}

/// One active pressure of a kind, at a stage and level, aimed as named.
fn active(pressure: Pressure, stage: Stage, level: i64, target: Target) -> PressureState {
    PressureState {
        pressure,
        stage,
        level,
        primary: false,
        queued: false,
        start_step: 0,
        target,
        displaced: None,
        bound: None,
    }
}

/// A staging whose schedule holds the pressure standing in one long stage, so
/// the machine derives the same stage and level every step of the test.
fn staged_with(pressure: PressureState) -> Unstaged {
    let held = table(pressure.pressure, pressure.target.kind, 36_000, pressure.level);
    Unstaged {
        schedule: Schedule::of(vec![held]).expect("one table per pressure"),
        pressures: vec![pressure],
        stream: RngState::default(),
    }
}

#[test]
fn drain_scales_the_targeted_layer_through_the_one_drain_sink() {
    // Two layers, each with a Node holding 64 units under a 1-unit drain.
    let mut field = effect_field(&[(1, 64, 500, 500), (2, 64, 1500, 500)], &[]);
    field.layers[0].drain = ONE_UNIT;
    field.layers.push(FieldLayer {
        layer: 1,
        drain: ONE_UNIT,
        noise: 0,
        gain: 0,
        current_ids: Vec::new(),
        port_ids: vec![2],
    });
    let place = field.ports.iter().position(|port| port.node == 2).expect("node 2");
    field.ports[place].layer = 1;
    field.layers[0].port_ids = vec![1];
    field_game_core::field::validate(&field).expect("still valid");

    // Drain at level 32768 — half — scales the targeted layer to one and a
    // half units; the untargeted layer keeps its authored loss.
    let mut staged = staged_with(active(
        Pressure::Drain,
        Stage::Pressure,
        32_768,
        Target { kind: TargetKind::Layer, id: Some(0) },
    ));
    let outcome = field_mod::advance(
        &mut field,
        ControlState::default(),
        FRAC_ONE,
        &mut staged.staging(),
    );
    let expected = ONE_UNIT + fixed_mul(ONE_UNIT, 32_768);
    let held = |field: &FieldState, node: u32| {
        field.ports.iter().find(|port| port.node == node).expect("the Node").q
    };
    assert_eq!(held(&field, 1), 64 * ONE_UNIT - expected, "the targeted layer, scaled");
    assert_eq!(held(&field, 2), 63 * ONE_UNIT, "the other layer, untouched");
    assert_eq!(
        outcome.ledger.drain,
        expected + ONE_UNIT,
        "one sink absorbs both layers' losses"
    );
    assert_eq!(outcome.ledger.residual(), 0, "and the step balances exactly");
}

#[test]
fn noise_narrows_route_flow_by_the_drawn_scale_and_zero_noise_draws_nothing() {
    // A full reserve behind a small Route: the flow is capacity-limited, so
    // the drawn scale is exactly what the Route moves.
    let opening = effect_field(&[(1, 2_000, 500, 500), (2, 0, 1100, 500)], &[(1, 1, 2, 16)]);

    // Zero effective noise: no draw, the whole scale, and the stream does not
    // move — which is what keeps every pre-Noise byte pin standing.
    let mut quiet = opening.clone();
    let mut unstaged = Unstaged::default();
    let before = unstaged.stream;
    field_mod::advance(&mut quiet, ControlState::default(), FRAC_ONE, &mut unstaged.staging());
    assert_eq!(unstaged.stream, before, "a run without noise consumes no words");
    assert_eq!(quiet.routes[0].flow, 16 * ONE_UNIT, "and the flow is the whole capacity");

    // An active Noise pressure targeting the layer draws one word and narrows
    // the flow to `fixed_mul(capacity, 65536 - fixed_mul(noise_eff, j))`.
    let mut noisy = opening.clone();
    let mut staged = staged_with(active(
        Pressure::Noise,
        Stage::Pressure,
        39_322,
        Target { kind: TargetKind::Layer, id: Some(0) },
    ));
    let mut foretold = staged.stream;
    let word = foretold.draw(65_537) as i64;
    let outcome = field_mod::advance(
        &mut noisy,
        ControlState::default(),
        FRAC_ONE,
        &mut staged.staging(),
    );
    assert_eq!(staged.stream, foretold, "one draw per noisy layer per step");
    let scale = FRAC_ONE - fixed_mul(39_322, word);
    let expected = fixed_mul(16 * ONE_UNIT, scale);
    assert_eq!(noisy.routes[0].flow, expected, "the drawn scale narrows the capacity");
    assert!(noisy.routes[0].flow < 16 * ONE_UNIT, "and the drawn scale narrowed this step");
    assert_eq!(outcome.ledger.moved, expected);
    assert_eq!(outcome.ledger.residual(), 0, "narrowing moves no Charge of its own");
}

#[test]
fn flood_lowers_the_targeted_threshold_for_decay_and_the_throttle() {
    // Node 2 holds 100 units against a 128-unit capacity: not overloaded by
    // the base rule. Flood at full level halves the threshold to 64, so the
    // excess above 64 decays and the Route into it moves under half capacity.
    let mut field = effect_field(&[(1, 2_000, 500, 500), (2, 100, 1100, 500)], &[(1, 1, 2, 16)]);
    let place = field.ports.iter().position(|port| port.node == 2).expect("node 2");
    field.ports[place].capacity = 128 * ONE_UNIT;
    let mut staged = staged_with(active(
        Pressure::Flood,
        Stage::Crisis,
        FRAC_ONE,
        Target { kind: TargetKind::Node, id: Some(2) },
    ));
    let outcome = field_mod::advance(
        &mut field,
        ControlState::default(),
        FRAC_ONE,
        &mut staged.staging(),
    );
    // The throttle read the list as the step opened: half of 16.
    assert_eq!(field.routes[0].flow, 8 * ONE_UNIT, "the inflow throttle held it to half");
    // Decay read the staged list: held 108 against an effective 64, so a
    // quarter of the 44-unit excess decayed.
    let expected = fixed_mul((108 - 64) * ONE_UNIT, 16_384);
    assert_eq!(outcome.ledger.overload, expected, "the existing overload sink absorbs it");
    assert_eq!(outcome.ledger.residual(), 0);

    // The end-of-step flags consult the same lowered threshold: the frame
    // reads the Node as overloaded though it stands below its capacity.
    let snapshot = field_game_core::frame::encode(&field_game_core::frame::Snapshot {
        field: &field,
        mode: field_game_core::run::Mode::Running,
        time_scale: 65_535,
        view_inside: &[],
        queue: &field_game_core::plan::PlanQueue::new(),
        cues: &[],
        config: &field_game_core::state::InputConfig::default_config(),
        progress: &field_game_core::state::Progress::opening(),
        pressures: &staged.pressures,
        objective_ordinal: 0,
        forecast: &[],
    });
    let sections = snapshot[20] as usize;
    let mut overloaded = None;
    for place in 0..sections {
        let at = 32 + place * 8;
        if snapshot[at] == 2 {
            let offset = u32::from_le_bytes([
                snapshot[at + 4],
                snapshot[at + 5],
                snapshot[at + 6],
                snapshot[at + 7],
            ]) as usize;
            // The second Port record: node 2.
            overloaded = Some(snapshot[offset + 16 + 5] & (1 << 1) != 0);
        }
    }
    assert_eq!(overloaded, Some(true), "the flag reads the lowered threshold");
}

#[test]
fn interference_redirects_a_share_of_every_same_layer_emission_to_its_target() {
    // One current over Node 1, its band far from Node 3 — the competing path.
    let mut field = effect_field(&[(1, 0, 500, 500), (2, 0, 650, 500), (3, 0, 3_000, 3_000)], &[]);
    field.layers[0].gain = FRAC_ONE;
    field.layers[0].current_ids = vec![1];
    field.currents = vec![CurrentState {
        id: 1,
        layer: 0,
        path: vec![Vec2::units(400, 500), Vec2::units(700, 500)],
        width: 100 * ONE_UNIT,
        strength: 64 * ONE_UNIT,
        period: 30,
        phase: 0,
        bright: false,
        active: true,
    }];
    field_game_core::field::validate(&field).expect("still valid");

    // At level 65536 the redirected share is half: 32 units to the target,
    // and the geometric recipients split the other 32 by the locked
    // remainder rule.
    let mut staged = staged_with(active(
        Pressure::Interference,
        Stage::Crisis,
        FRAC_ONE,
        Target { kind: TargetKind::Node, id: Some(3) },
    ));
    let outcome = field_mod::advance(
        &mut field,
        ControlState::default(),
        FRAC_ONE,
        &mut staged.staging(),
    );
    let held = |field: &FieldState, node: u32| {
        field.ports.iter().find(|port| port.node == node).expect("the Node").q
    };
    assert_eq!(held(&field, 3), 32 * ONE_UNIT, "the target's first claim, off the band");
    assert_eq!(held(&field, 1), 16 * ONE_UNIT, "the remainder splits geometrically");
    assert_eq!(held(&field, 2), 16 * ONE_UNIT);
    assert_eq!(outcome.ledger.current, 64 * ONE_UNIT, "everything inside the current source");
    assert_eq!(outcome.ledger.residual(), 0, "conservation holds through the redirection");

    // Short headroom refuses the rest, never emitted — the delivery rule's
    // own clamp, inside the same conservation.
    let mut nearly = effect_field(&[(1, 0, 500, 500), (2, 0, 650, 500), (3, 0, 3_000, 3_000)], &[]);
    nearly.layers[0].gain = FRAC_ONE;
    nearly.layers[0].current_ids = vec![1];
    nearly.currents = field.currents.clone();
    let place = nearly.ports.iter().position(|port| port.node == 3).expect("node 3");
    nearly.ports[place].q = 4_090 * ONE_UNIT;
    let mut staged = staged_with(active(
        Pressure::Interference,
        Stage::Crisis,
        FRAC_ONE,
        Target { kind: TargetKind::Node, id: Some(3) },
    ));
    let outcome = field_mod::advance(
        &mut nearly,
        ControlState::default(),
        FRAC_ONE,
        &mut staged.staging(),
    );
    assert_eq!(held(&nearly, 3), 4_096 * ONE_UNIT, "delivered only to the headroom");
    assert_eq!(
        outcome.ledger.current,
        6 * ONE_UNIT + 32 * ONE_UNIT,
        "the refused share was never emitted"
    );
    assert_eq!(outcome.ledger.residual(), 0);
}

/// A run restored over an effect fixture, standing under a View of Node 1,
/// with one staged pressure and its table bound.
fn run_with(
    field: FieldState,
    pressure: PressureState,
    dwell: i64,
) -> field_game_core::run::Run {
    use field_game_core::state::{InputConfig, Progress, Surround, Trace, ViewDeclaration};
    let held = table(pressure.pressure, pressure.target.kind, dwell, pressure.level);
    let state = field_game_core::state::RunState {
        run_id: "00bb00bb00bb00bb".to_string(),
        rng: field_game_core::rng::trajectory_stream("00bb00bb00bb00bb", 0),
        spec: field_game_core::state::GeneratorSpec::new(
            support::content_hash(),
            Schedule::of(vec![held]).expect("one table per pressure"),
        ),
        branch_nonce: 0,
        progress: Progress::opening(),
        now: field.clone(),
        trace: Trace::opening(field),
        view: ViewDeclaration {
            inside: vec![1],
            resolution: 1,
            window: 45,
            surround: Surround::Adjacent,
        },
        slate: None,
        input_config: InputConfig::default_config(),
        pressures: vec![pressure],
        anchors: Vec::new(),
    };
    state.coherent().expect("the fixture is a coherent run");
    field_game_core::run::Run::restore(state, "thread").expect("the run restores")
}

/// Advances a restored run by exact steps through the locked test hook.
fn run_steps(run: &mut field_game_core::run::Run, seq: u32, steps: u16) {
    let body = format!(
        "{{\"advance_steps\":{steps},\"depth_key\":0,\"inspect\":null,\"pause\":false,\
          \"pulse_held\":false,\"pulse_release\":false,\"seq\":{seq},\"steer_x\":0,\
          \"steer_y\":0,\"t_us\":{stamp},\"toggle_still\":false,\"wheel\":0}}",
        stamp = i64::from(seq) * 33_333
    );
    run.input_frame(&parse(&body).expect("canonical"), None).expect("the frame is accepted");
}

#[test]
fn fracture_breaks_the_heaviest_route_at_crisis_entry_and_the_window_ends() {
    // Route 1 carries 16 units a step, Route 2 carries 4: the trailing-flow
    // fallback selects Route 1, smallest identifier on ties never reached.
    let field = effect_field(
        &[(1, 3_000, 500, 500), (2, 0, 1100, 500), (3, 0, 1700, 500)],
        &[(1, 1, 2, 16), (2, 1, 3, 4)],
    );
    // Signal and pressure stages of two steps each: crisis entry lands at
    // step 4, and the boundary that closes it breaks the Route.
    let mut run = run_with(
        field,
        active(Pressure::Fracture, Stage::Signal, 30_000, Target::none()),
        2,
    );
    run_steps(&mut run, 1, 3);
    assert_eq!(run.state().now.routes.len(), 2, "nothing breaks before the crisis");

    run_steps(&mut run, 2, 1);
    let state = run.state();
    assert_eq!(state.now.step, 4);
    assert_eq!(state.now.routes.len(), 1, "the crisis entry broke one Route");
    assert_eq!(state.now.routes[0].route, 2, "the heaviest trailing flow was Route 1's");
    assert_eq!(
        state.trace.start_step, 4,
        "the break rides the committed-cut machinery: the window ended"
    );
    assert_eq!(
        state.trace.keyframe.routes.len(),
        1,
        "and the trajectory restarts on the state the break leaves"
    );

    // Permanent: resolution restores nothing, and the run plays on — recovery
    // is the player's own rerouting.
    run_steps(&mut run, 3, 40);
    assert_eq!(run.state().now.routes.len(), 1);
    run.state().coherent().expect("the broken Field is a coherent run");

    // An authored target breaks exactly the Route it names, once.
    let again = effect_field(
        &[(1, 3_000, 500, 500), (2, 0, 1100, 500), (3, 0, 1700, 500)],
        &[(1, 1, 2, 16), (2, 1, 3, 4)],
    );
    let mut aimed = run_with(
        again,
        active(
            Pressure::Fracture,
            Stage::Signal,
            30_000,
            Target { kind: TargetKind::Route, id: Some(2) },
        ),
        2,
    );
    run_steps(&mut aimed, 1, 6);
    assert_eq!(aimed.state().now.routes.len(), 1);
    assert_eq!(aimed.state().now.routes[0].route, 1, "the authored target broke");
}

/// The authored bundle with one edit made to one listed file's bytes, read
/// back through the whole of `read_bundle`.
fn schedule_edited(
    place: usize,
    from: &str,
    to: &str,
) -> Result<field_game_core::content::Content, field_game_core::fault::Fault> {
    let mut files: Vec<String> = support::files().iter().map(|file| file.to_string()).collect();
    let held = files[place].replace(from, to);
    assert_ne!(held, files[place], "the edit found something to change");
    files[place] = held;
    let bundle = support::bundle_of(support::MANIFEST, &files);
    field_game_core::content::read_bundle(&parse(&bundle).expect("canonical"))
}

/// The Node the trace's own retained flows make the heaviest — the same
/// reading the Flood hold makes, restated here so what is asserted is the rule
/// rather than a Node identifier written down twice.
fn heaviest_throughput(state: &field_game_core::state::RunState) -> Option<u32> {
    let ends: std::collections::BTreeMap<u32, (u32, u32)> =
        state.now.routes.iter().map(|route| (route.route, (route.tail, route.head))).collect();
    let steps = state.trace.steps.len();
    let mut sums: std::collections::BTreeMap<u32, i64> = std::collections::BTreeMap::new();
    for recorded in state.trace.steps.iter().skip(steps.saturating_sub(60)) {
        for (route, flow) in &recorded.records.f {
            let Some((tail, head)) = ends.get(route) else { continue };
            *sums.entry(*tail).or_insert(0) += flow;
            *sums.entry(*head).or_insert(0) += flow;
        }
    }
    let mut best: Option<(u32, i64)> = None;
    for port in &state.now.ports {
        let sum = sums.get(&port.node).copied().unwrap_or(0);
        if best.is_none_or(|(_, held)| sum > held) {
            best = Some((port.node, sum));
        }
    }
    best.map(|(node, _)| node)
}

#[test]
fn a_schedule_entry_aimed_at_nothing_loads_and_holds_the_locked_default() {
    // The two pressures whose locked reading admits `none` — Flood's
    // heaviest-throughput hold and Fracture's largest-trailing-flow break —
    // are authorable only if the schedule validation admits an entry whose
    // target kind is `none` whatever kind the pressure's own table declares.
    // A `none` entry names nothing by declaration and defers to the pressure's
    // own default, so there is nothing for the kind to agree with.
    for aimed in [
        "{ \"pressure\": \"flood\", \"start_step\": 900, \"primary\": true, \
          \"target\": { \"t\": \"none\", \"id\": null } }",
        "{ \"pressure\": \"fracture\", \"start_step\": 900, \"primary\": true, \
          \"target\": { \"t\": \"none\", \"id\": null } }",
    ] {
        let content = schedule_edited(
            0,
            "{ \"pressure\": \"interference\", \"start_step\": 900, \"primary\": true, \
             \"target\": { \"t\": \"node\", \"id\": 5 } }",
            aimed,
        )
        .expect("a schedule entry aimed at nothing loads");
        let entry = content
            .chapter(0)
            .expect("the opening chapter")
            .pressure_schedule
            .first()
            .expect("the edited entry");
        assert_eq!(entry.target.kind, TargetKind::None);
        assert_eq!(entry.target.id, None, "a kind that names nothing carries no identifier");
    }

    // What the guard still refuses is what it was written for: an entry that
    // names a target of the wrong kind for the pressure it aims. Flood reads a
    // Node and a layer is not one.
    let fault = schedule_edited(
        0,
        "{ \"pressure\": \"interference\", \"start_step\": 900, \"primary\": true, \
         \"target\": { \"t\": \"node\", \"id\": 5 } }",
        "{ \"pressure\": \"flood\", \"start_step\": 900, \"primary\": true, \
          \"target\": { \"t\": \"layer\", \"id\": 0 } }",
    )
    .expect_err("a Flood aimed at a layer is refused");
    assert_eq!(fault.code(), field_game_core::fault::Code::ContentInvalid);
    assert_eq!(fault.detail().expect("a diagnostic"), "{\"reason\":\"pressure_schedule\"}");

    // And the hold itself, at the stage entries it is made on: two Routes into
    // one Node, so the heaviest-throughput Node is neither Route's tail and a
    // fallback that bound the smallest identifier would bind Node 1 instead.
    let field = effect_field(
        &[(1, 3_000, 500, 500), (2, 0, 1_100, 500), (3, 3_000, 1_700, 500)],
        &[(1, 1, 2, 4), (2, 3, 2, 16)],
    );
    let mut run = run_with(field, active(Pressure::Flood, Stage::Signal, 30_000, Target::none()), 4);
    run_steps(&mut run, 1, 3);
    assert_eq!(run.state().pressures[0].bound, None, "no stage has been entered yet");
    // The ranking is read here, one step short of the entry: the hold is a
    // membership boundary and ends the active window, so the trace it ranked is
    // gone by the time the hold stands.
    let ranked = heaviest_throughput(&run.state()).expect("the trace ranks a Node");
    assert_eq!(ranked, 2, "the Node both Routes carry into, not the smallest identifier");

    // The turnover into the second stage is a stage entry, and the hold is made
    // on it, off the trace as it stood.
    run_steps(&mut run, 2, 1);
    let state = run.state();
    assert_eq!(state.pressures[0].stage, Stage::Pressure);
    let held = state.pressures[0].bound.expect("the stage entry held a working target");
    assert_eq!(held.kind, TargetKind::Node);
    assert_eq!(held.id, ranked, "and it is the rule's own reading of the run's own records");
    assert_eq!(state.trace.start_step, state.now.step, "the hold ended the active window");

    // Re-held at every stage entry: the next one makes the hold again, off the
    // trace the window that ended left behind.
    run_steps(&mut run, 3, 3);
    let ranked = heaviest_throughput(&run.state()).expect("the new window ranks one too");
    run_steps(&mut run, 4, 1);
    let state = run.state();
    assert_eq!(state.pressures[0].stage, Stage::Crisis);
    let again = state.pressures[0].bound.expect("the next stage entry held one too");
    assert_eq!(again.id, ranked);
    state.coherent().expect("a Flood aimed at nothing leaves a coherent run");
}

#[test]
fn drift_moves_the_targeted_layers_current_paths_at_every_stage_entry() {
    // One current whose band covers Node 2; Drift at level 32768 moves its
    // path 128 units along +x — `start_step mod 4` is 0 — at each entry.
    let mut field = effect_field(&[(1, 100, 500, 500), (2, 0, 1100, 500)], &[]);
    field.layers[0].gain = FRAC_ONE;
    field.layers[0].current_ids = vec![1];
    field.currents = vec![CurrentState {
        id: 1,
        layer: 0,
        path: vec![Vec2::units(1000, 500), Vec2::units(1200, 500)],
        width: 150 * ONE_UNIT,
        strength: 8 * ONE_UNIT,
        period: 30,
        phase: 0,
        bright: false,
        active: true,
    }];
    field_game_core::field::validate(&field).expect("still valid");

    let mut run = run_with(
        field,
        active(
            Pressure::Drift,
            Stage::Signal,
            32_768,
            Target { kind: TargetKind::Layer, id: Some(0) },
        ),
        2,
    );
    // The first turnover — signal into pressure — lands at step 2, and its
    // boundary moves the paths.
    run_steps(&mut run, 1, 1);
    assert_eq!(run.state().now.currents[0].path[0], Vec2::units(1000, 500), "not yet");
    run_steps(&mut run, 2, 1);
    let state = run.state();
    let moved = fixed_mul(32_768, 16_777_216);
    assert_eq!(moved, 128 * ONE_UNIT, "a half level drifts 128 units");
    assert_eq!(state.now.currents[0].path[0].x, 1000 * ONE_UNIT + moved);
    assert_eq!(state.now.currents[0].path[1].x, 1200 * ONE_UNIT + moved);
    assert_eq!(state.now.currents[0].path[0].y, 500 * ONE_UNIT, "geometry drifts on one axis");
    assert_eq!(state.trace.start_step, 2, "each move is a membership boundary");
    assert_eq!(
        state.trace.keyframe.currents[0].path[0].x,
        1000 * ONE_UNIT + moved,
        "the keyframe carries the moved geometry"
    );

    // The crisis entry at step 4 moves it again, and the run stays coherent:
    // within any window no path ever moves, which is the budget table's own
    // preparation invariant.
    run_steps(&mut run, 3, 2);
    assert_eq!(
        run.state().now.currents[0].path[0].x,
        1000 * ONE_UNIT + 2 * moved,
        "one move per stage entry"
    );
    run.state().coherent().expect("the drifted Field is a coherent run");
}

#[test]
fn a_seat_taken_at_the_opening_boundary_fires_its_stage_entry_one_shots() {
    // A chapter authoring Flood-with-none and Drift for the run's first step:
    // the opening boundary admits both, and a seat is a stage entry exactly
    // as a turnover is — so Flood's hold and Drift's move land at the open,
    // before the first step runs, not never.
    use field_game_core::state::{Surround, ViewDeclaration};

    let mut field = effect_field(
        &[(1, 100, 500, 500), (2, 0, 1100, 500)],
        &[(1, 1, 2, 4)],
    );
    field.layers[0].gain = FRAC_ONE;
    field.layers[0].current_ids = vec![1];
    field.currents = vec![CurrentState {
        id: 1,
        layer: 0,
        path: vec![Vec2::units(1000, 500), Vec2::units(1200, 500)],
        width: 150 * ONE_UNIT,
        strength: 8 * ONE_UNIT,
        period: 30,
        phase: 0,
        bright: false,
        active: true,
    }];
    field_game_core::field::validate(&field).expect("still valid");

    let mut run = field_game_core::run::Run::start(
        "00dd00dd00dd00dd",
        "thread",
        &support::content_hash(),
    )
    .expect("a run opens");
    run.establish_field(
        field,
        ViewDeclaration {
            inside: vec![1],
            resolution: 1,
            window: 45,
            surround: Surround::Adjacent,
        },
    )
    .expect("the Field is establishable");
    run.set_schedule(
        Schedule::of(vec![
            table(Pressure::Flood, TargetKind::None, 36_000, FRAC_ONE),
            table(Pressure::Drift, TargetKind::Layer, 36_000, 32_768),
        ])
        .expect("one table per pressure"),
    );
    let chapter = field_game_core::content::Chapter {
        id: "the_pull".to_string(),
        title_key: "chapter.the_pull".to_string(),
        ending_key: "ending.the_pull".to_string(),
        ending_marks: Vec::new(),
        events: Vec::new(),
        layers: Vec::new(),
        forms: Vec::new(),
        ports: Vec::new(),
        routes: Vec::new(),
        currents: Vec::new(),
        physical_compartment: Default::default(),
        authored_boundaries: Vec::new(),
        objectives: Vec::new(),
        anchor_moments: Vec::new(),
        opening_view: ViewDeclaration {
            inside: vec![1],
            resolution: 1,
            window: 45,
            surround: Surround::Adjacent,
        },
        pressure_schedule: vec![
            pressure::ScheduleEntry {
                pressure: Pressure::Flood,
                start_step: 0,
                primary: false,
                target: Target::none(),
            },
            pressure::ScheduleEntry {
                pressure: Pressure::Drift,
                start_step: 1,
                primary: false,
                target: Target { kind: TargetKind::Layer, id: Some(0) },
            },
        ],
    };
    run.open_schedule(&chapter);

    // Both seats taken at the opening boundary, for the run's first step.
    let state = run.state();
    let flood = state
        .pressures
        .iter()
        .find(|held| held.pressure == Pressure::Flood)
        .expect("the Flood seat");
    assert!(!flood.queued);
    assert_eq!(flood.start_step, 1, "admitted for the first step");

    // The hold fired: with an empty trace every throughput sum is zero, and
    // the smallest NodeId is the heaviest, so `bound` names Node 1.
    assert_eq!(
        flood.bound,
        Some(pressure::Bound { kind: TargetKind::Node, id: 1 }),
        "Flood's hold lands at the opening boundary"
    );

    // The move fired: Drift's signal entry drifts the layer's current 128
    // units along +y — `start_step mod 4` is 1.
    let drift = state
        .pressures
        .iter()
        .find(|held| held.pressure == Pressure::Drift)
        .expect("the Drift seat");
    assert!(!drift.queued);
    let moved = fixed_mul(32_768, 16_777_216);
    assert_eq!(
        state.now.currents[0].path[0],
        Vec2::units(1000, 500 + 128),
        "Drift's move lands at the opening boundary"
    );
    assert_eq!(state.now.currents[0].path[0].y, 500 * ONE_UNIT + moved);
    assert_eq!(
        state.trace.keyframe.currents[0].path[0].y,
        500 * ONE_UNIT + moved,
        "and the keyframe carries the moved geometry"
    );

    // The run plays on from the settled opening and stays coherent.
    run_steps(&mut run, 1, 3);
    run.state().coherent().expect("the settled opening is a coherent run");
}

/// A run restored over an effect fixture with one staged pressure and an
/// authored table whose four stages carry their own levels — the shape that
/// makes an opened reading differ per stage.
fn run_with_table(
    field: FieldState,
    pressure: PressureState,
    content: PressureContent,
) -> field_game_core::run::Run {
    use field_game_core::state::{InputConfig, Progress, Surround, Trace, ViewDeclaration};
    let state = field_game_core::state::RunState {
        run_id: "00ee00ee00ee00ee".to_string(),
        rng: field_game_core::rng::trajectory_stream("00ee00ee00ee00ee", 0),
        spec: field_game_core::state::GeneratorSpec::new(
            support::content_hash(),
            Schedule::of(vec![content]).expect("one table per pressure"),
        ),
        branch_nonce: 0,
        progress: Progress::opening(),
        now: field.clone(),
        trace: Trace::opening(field),
        view: ViewDeclaration {
            inside: vec![1],
            resolution: 1,
            window: 45,
            surround: Surround::Adjacent,
        },
        slate: None,
        input_config: InputConfig::default_config(),
        pressures: vec![pressure],
        anchors: Vec::new(),
    };
    state.coherent().expect("the fixture is a coherent run");
    field_game_core::run::Run::restore(state, "thread").expect("the run restores")
}

/// A four-stage table with its own level per stage: what makes the opened
/// reading of one step differ from the reading a later stage carries.
fn staged_table(pressure: Pressure, target: TargetKind, dwell: i64, levels: [i64; 4]) -> PressureContent {
    PressureContent {
        pressure,
        target,
        stages: [
            StageRow { stage: Stage::Signal, level: levels[0], steps: dwell },
            StageRow { stage: Stage::Pressure, level: levels[1], steps: dwell },
            StageRow { stage: Stage::Crisis, level: levels[2], steps: dwell },
            StageRow { stage: Stage::Resolution, level: levels[3], steps: dwell },
        ],
    }
}

#[test]
fn a_noise_seating_on_a_saturating_route_replays_byte_exact_through_its_lifecycle() {
    // The Echo's shape: a Noise pressure over a Route whose flow saturates,
    // walked from its seat through every stage boundary and its removal. The
    // opened reading of the seating step is the signal level; a window
    // replayed at the removal must re-derive exactly that reading — never the
    // list's current stage — or the drawn scale narrows a different capacity
    // than the live step drew and the keyframe diverges.
    let build = || {
        let field =
            effect_field(&[(1, 3_000, 500, 500), (2, 0, 1100, 500)], &[(1, 1, 2, 8)]);
        run_with_table(
            field,
            PressureState::queued_at(
                Pressure::Noise,
                6,
                false,
                Target { kind: TargetKind::Layer, id: Some(0) },
            ),
            staged_table(
                Pressure::Noise,
                TargetKind::Layer,
                3,
                [20_000, 30_000, 50_000, 10_000],
            ),
        )
    };

    // Twice, from the same key and frames: the lifecycle — seat at the
    // boundary before step 6, three turnovers, spent at 18 — replays inside
    // the run (the removal boundary ends the window over the seating step)
    // and the two runs walk the same bytes.
    let played = |mut run: field_game_core::run::Run| -> String {
        let mut seq = 1;
        for _ in 0..24 {
            run_steps(&mut run, seq, 1);
            seq += 1;
        }
        assert!(
            run.state().pressures.is_empty(),
            "the lifecycle completed and the removal boundary replayed it"
        );
        run.state().payload()
    };
    let once = played(build());
    let again = played(build());
    assert_eq!(once, again, "byte-equivalent through seat, stages, and removal");

    // Export mid-lifecycle — crisis standing — and replay on: the restored
    // run and the source walk the same bytes across the later boundaries.
    let mut source = build();
    let mut seq = 1;
    for _ in 0..13 {
        run_steps(&mut source, seq, 1);
        seq += 1;
    }
    assert_eq!(source.state().pressures[0].stage, Stage::Crisis);
    let payload = source.state().payload();
    let parsed = parse(&payload).expect("canonical");
    let mut imported = field_game_core::run::Run::restore(
        field_game_core::state::RunState::read(&parsed).expect("the payload reads"),
        "thread",
    )
    .expect("the run restores");
    imported.set_schedule(
        Schedule::of(vec![staged_table(
            Pressure::Noise,
            TargetKind::Layer,
            3,
            [20_000, 30_000, 50_000, 10_000],
        )])
        .expect("one table per pressure"),
    );
    let drive = |run: &mut field_game_core::run::Run, from: u32| -> String {
        let mut seq = from;
        for _ in 0..10 {
            run_steps(run, seq, 1);
            seq += 1;
        }
        run.state().payload()
    };
    let source_on = drive(&mut source, seq);
    let imported_on = drive(&mut imported, 1);
    // Every restore stamps prev_assembly_step with the step it returned to —
    // the fixture's own construction included (a restore at step 0) — and a
    // later boundary's keyframe clone carries the stamp into both copies, so
    // both sides normalize their own stamp before comparing. Everything else
    // is byte-identical, which is the defect under test.
    let normalize = |payload: &str, stamp: &str| {
        payload.replace(
            &format!("\"prev_assembly_step\":{stamp}"),
            "\"prev_assembly_step\":null",
        )
    };
    assert_eq!(
        normalize(&source_on, "0"),
        normalize(&imported_on, "13"),
        "an import mid-crisis replays to the source's own bytes"
    );

    // Quick Retry's shape: restore an earlier snapshot — taken before the
    // seating step — and drive the same frames; the retried run reproduces
    // the straight run's bytes across the seat and every boundary after it.
    let mut straight = build();
    let mut seq = 1;
    for _ in 0..4 {
        run_steps(&mut straight, seq, 1);
        seq += 1;
    }
    let snapshot = straight.state().payload();
    let straight_on = drive(&mut straight, seq);
    let mut retried = field_game_core::run::Run::restore(
        field_game_core::state::RunState::read(&parse(&snapshot).expect("canonical"))
            .expect("the payload reads"),
        "thread",
    )
    .expect("the run restores");
    retried.set_schedule(
        Schedule::of(vec![staged_table(
            Pressure::Noise,
            TargetKind::Layer,
            3,
            [20_000, 30_000, 50_000, 10_000],
        )])
        .expect("one table per pressure"),
    );
    let retried_on = drive(&mut retried, 1);
    assert_eq!(
        normalize(&retried_on, "4"),
        normalize(&straight_on, "0"),
        "a retry from before the seat replays across it to the same bytes"
    );
}

#[test]
fn a_flood_seating_over_a_throttled_route_replays_byte_exact() {
    // The same class, read at the same phase: Flood's throttle term consults
    // the lowered threshold with the list as the step opened. The head Node
    // fills toward its threshold while the stages turn over, so a replay that
    // read a later stage's level would throttle a different step than the
    // live run did.
    let build = || {
        let mut field =
            effect_field(&[(1, 3_000, 500, 500), (2, 100, 1100, 500)], &[(1, 1, 2, 16)]);
        let place = field.ports.iter().position(|port| port.node == 2).expect("node 2");
        field.ports[place].capacity = 128 * ONE_UNIT;
        field_game_core::field::validate(&field).expect("still valid");
        run_with_table(
            field,
            PressureState::queued_at(
                Pressure::Flood,
                6,
                false,
                Target { kind: TargetKind::Node, id: Some(2) },
            ),
            staged_table(
                Pressure::Flood,
                TargetKind::Node,
                3,
                [20_000, 35_000, 60_000, 10_000],
            ),
        )
    };
    let played = |mut run: field_game_core::run::Run| -> String {
        let mut seq = 1;
        for _ in 0..24 {
            run_steps(&mut run, seq, 1);
            seq += 1;
        }
        assert!(run.state().pressures.is_empty(), "the lifecycle completed");
        run.state().payload()
    };
    let once = played(build());
    let again = played(build());
    assert_eq!(once, again, "byte-equivalent through seat, stages, and removal");
}

#[test]
fn a_retained_window_spanning_a_noise_turnover_replays_byte_exact() {
    // The Loop's facet: no seat inside any window — the pressure stands
    // active from the run's first step — and the defect is the turnover
    // alone. With 45-step stages, the ordinary keyframe carry replays spans
    // that cross the turnovers at 45, 90, and 135 while the live list stands
    // at a later stage; each replayed step must re-derive the reading that
    // stood as it opened, or the drawn scale narrows a different capacity
    // than the live step drew.
    let build = || {
        let field =
            effect_field(&[(1, 3_000, 500, 500), (2, 0, 1100, 500)], &[(1, 1, 2, 8)]);
        run_with_table(
            field,
            active(
                Pressure::Noise,
                Stage::Signal,
                20_000,
                Target { kind: TargetKind::Layer, id: Some(0) },
            ),
            staged_table(
                Pressure::Noise,
                TargetKind::Layer,
                45,
                [20_000, 30_000, 50_000, 10_000],
            ),
        )
    };
    let played = |mut run: field_game_core::run::Run| -> String {
        let mut seq = 1;
        // Past the spent removal at 180: the carry replays the turnover spans
        // on the way, and the removal boundary replays the whole retained
        // window across the crisis and resolution turnovers.
        for _ in 0..38 {
            run_steps(&mut run, seq, 5);
            seq += 1;
        }
        assert!(run.state().pressures.is_empty(), "the lifecycle completed");
        run.state().payload()
    };
    let once = played(build());
    let again = played(build());
    assert_eq!(once, again, "byte-equivalent across every replayed turnover");
}

#[test]
fn a_flood_threshold_flip_across_a_mid_window_turnover_replays_byte_exact() {
    // The same facet on Flood's throttle: the head Node fills toward its
    // threshold while a turnover moves the stage level, so the step at which
    // the throttle first fires depends on which stage's threshold the Route
    // phase read — the reading as the step opened, never the stage the list
    // stands at when a window is replayed.
    let build = || {
        let mut field =
            effect_field(&[(1, 3_000, 500, 500), (2, 60, 1100, 500)], &[(1, 1, 2, 4)]);
        let place = field.ports.iter().position(|port| port.node == 2).expect("node 2");
        field.ports[place].capacity = 128 * ONE_UNIT;
        field_game_core::field::validate(&field).expect("still valid");
        run_with_table(
            field,
            active(
                Pressure::Flood,
                Stage::Signal,
                10_000,
                Target { kind: TargetKind::Node, id: Some(2) },
            ),
            staged_table(
                Pressure::Flood,
                TargetKind::Node,
                45,
                [10_000, 35_000, 60_000, 5_000],
            ),
        )
    };
    let played = |mut run: field_game_core::run::Run| -> String {
        let mut seq = 1;
        for _ in 0..38 {
            run_steps(&mut run, seq, 5);
            seq += 1;
        }
        assert!(run.state().pressures.is_empty(), "the lifecycle completed");
        run.state().payload()
    };
    let once = played(build());
    let again = played(build());
    assert_eq!(once, again, "byte-equivalent across the throttle's own flip");
}
