//! Still Mode: the mode table, the ramps, and the queued-change commands.
//!
//! `docs/field-framework/ARCHITECTURE.md` locks the mode table, the 250 ms the
//! two ramps take, the lifecycle states the three queued-change commands are
//! valid in, and the overlay section the render snapshot carries only while a
//! run is `still`. What is under test here is exactly that table, read as a
//! table: every transition it names, and the two it does not.
//!
//! The ramps are stated in real time, so every frame here carries a timestamp
//! that means something: the gap between one frame and the next is what moves
//! a ramp, under the accumulator's own clamp, whether or not the frame also
//! names a step count.

use field_game_core::frame;
use field_game_core::fx::ONE_UNIT;
use field_game_core::json::{parse, Json};
use field_game_core::plan::PlanQueue;
use field_game_core::run::{Mode, RAMP_UNITS};
use field_game_core::state::{FieldState, InputConfig, Progress};
use field_game_core::Session;

mod support;

const KEY: &str = "0123456789abcdef";

/// One rendered frame at the 60-frames-per-second target, in microseconds.
const FRAME_US: i64 = 16_667;

/// How long a ramp takes, in microseconds. Locked.
const RAMP_US: i64 = 250_000;

/// One frame of input at the render rate, with every declared field present.
fn frame(seq: u32, steps: u16) -> String {
    at(seq, steps, i64::from(seq) * FRAME_US)
}

/// One frame at a timestamp of the caller's own naming.
fn at(seq: u32, steps: u16, t_us: i64) -> String {
    built(seq, Some(steps), false, false, t_us, 0)
}

/// The same, carrying the toggle.
fn toggling_at(seq: u32, steps: u16, t_us: i64) -> String {
    built(seq, Some(steps), true, false, t_us, 0)
}

fn built(
    seq: u32,
    steps: Option<u16>,
    toggle_still: bool,
    pause: bool,
    t_us: i64,
    wheel: i16,
) -> String {
    let advance = match steps {
        Some(count) => count.to_string(),
        None => "null".to_string(),
    };
    format!(
        "{{\"advance_steps\":{advance},\"depth_key\":0,\"inspect\":null,\"pause\":{pause},\
          \"pulse_held\":false,\"pulse_release\":false,\"seq\":{seq},\"steer_x\":0,\
          \"steer_y\":0,\"t_us\":{t_us},\"toggle_still\":{toggle_still},\"wheel\":{wheel}}}"
    )
}

fn opened(key: &str) -> Session {
    let mut session = Session::new(&support::worker_init()).expect("versions agree");
    let answer = session.command(
        "init_run",
        &format!("{{\"form\":\"thread\",\"mode\":\"new\",\"run_id\":\"{key}\"}}"),
    );
    assert!(answer.starts_with("{\"body\":"), "a new run opens: {answer}");
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

/// The time scale the newest render snapshot reports.
fn scale(session: &Session) -> u16 {
    let view = session.frame_view();
    u16::from_le_bytes([view[12], view[13]])
}

/// Whether the newest snapshot's header raises the still-surface flag.
fn still_flag(session: &Session) -> bool {
    let view = session.frame_view();
    u16::from_le_bytes([view[6], view[7]]) & 1 != 0
}

/// The record count of one section kind in the newest snapshot, and none when
/// the frame carries no section of that kind at all.
fn section(session: &Session, kind: u8) -> Option<u16> {
    let view = session.frame_view();
    for place in 0..usize::from(view[20]) {
        let at = 32 + place * 8;
        if view[at] == kind {
            return Some(u16::from_le_bytes([view[at + 2], view[at + 3]]));
        }
    }
    None
}

/// Drives a session from `running` into `still` by the shortest path there is:
/// one toggling frame, then one frame a whole ramp later.
///
/// The second frame carries the locked span exactly, which is also where the
/// accumulator's gap clamp sits — the widest real time one frame may carry is
/// the length of one ramp.
fn entered(session: &mut Session, from_seq: u32) -> i64 {
    let opened_at = i64::from(from_seq) * FRAME_US;
    session.command("input_frame", &toggling_at(from_seq, 1, opened_at));
    assert_eq!(session.lifecycle(), "ramp_in");
    session.command("input_frame", &at(from_seq + 1, 8, opened_at + RAMP_US));
    assert_eq!(session.lifecycle(), "still");
    opened_at + RAMP_US
}

// ---------------------------------------------------------------------------
// The mode table
// ---------------------------------------------------------------------------

#[test]
fn the_toggle_opens_the_ramp_and_the_ramp_completes_into_still() {
    let mut session = opened(KEY);
    assert_eq!(session.lifecycle(), "running");
    assert_eq!(scale(&session), 65535, "full speed saturates the header field");

    // The frame carrying the toggle spends its own time at the scale it
    // arrived under, and opens the ramp at zero.
    session.command("input_frame", &toggling_at(1, 1, 1_000_000));
    assert_eq!(session.lifecycle(), "ramp_in");
    assert_eq!(session.step(), 1, "the toggling frame runs the steps it asked for");
    assert_eq!(scale(&session), 65535, "the ramp opens at full and falls from there");

    // A frame most of the way through the span leaves the ramp standing.
    session.command("input_frame", &at(2, 1, 1_000_000 + RAMP_US - FRAME_US));
    assert_eq!(session.lifecycle(), "ramp_in", "the span is not crossed until it is crossed");
    let spent = (RAMP_US - FRAME_US) * 30;
    let wanted = ((RAMP_UNITS - spent) * 65_536 / RAMP_UNITS) as u16;
    assert_eq!(scale(&session), wanted, "the scale falls linearly with the elapsed time");
    assert_eq!(session.step(), 2, "a ramping run still steps");

    // The next crosses it, and the completion is the one thing that puts a run
    // in `still`.
    session.command("input_frame", &at(3, 1, 1_000_000 + RAMP_US));
    assert_eq!(session.lifecycle(), "still");
    assert_eq!(scale(&session), 0, "Still Mode holds the time scale at 0");
    assert_eq!(session.step(), 2, "the frame that completes the ramp runs no step");
}

#[test]
fn the_ramp_runs_over_the_locked_span_of_real_time() {
    let mut session = opened(KEY);
    session.command("input_frame", &toggling_at(1, 0, 1_000_000));
    assert_eq!(session.lifecycle(), "ramp_in");

    // Halfway through the span, halfway down the scale.
    session.command("input_frame", &at(2, 0, 1_125_000));
    assert_eq!(session.lifecycle(), "ramp_in");
    assert_eq!(scale(&session), 32_768, "125,000 µs is half of the locked 250,000");

    // One microsecond short of the span, and then across it.
    session.command("input_frame", &at(3, 0, 1_249_999));
    assert_eq!(session.lifecycle(), "ramp_in", "the span is not crossed until it is crossed");
    session.command("input_frame", &at(4, 0, 1_250_000));
    assert_eq!(session.lifecycle(), "still");
}

#[test]
fn a_toggle_during_a_ramp_reverses_it_and_the_scale_does_not_jump() {
    let mut session = opened(KEY);
    session.command("input_frame", &toggling_at(1, 0, 1_000_000));
    assert_eq!(session.lifecycle(), "ramp_in");

    // A fifth of the way down.
    session.command("input_frame", &at(2, 0, 1_000_000 + RAMP_US / 5));
    let turned_at = scale(&session);
    assert_eq!(turned_at, ((RAMP_UNITS - RAMP_US / 5 * 30) * 65_536 / RAMP_UNITS) as u16);

    // The turn itself moves the scale nowhere: the position mirrors, and the
    // scale is a linear function of what a ramp has left to run.
    session.command("input_frame", &toggling_at(3, 0, 1_000_000 + RAMP_US / 5));
    assert_eq!(session.lifecycle(), "ramp_out");
    assert_eq!(scale(&session), turned_at, "a reversal is continuous in the scale");

    // And the way back costs only the time the way in had spent: a fifth.
    session.command("input_frame", &at(4, 0, 1_000_000 + RAMP_US / 5 + RAMP_US / 5 - 1));
    assert_eq!(session.lifecycle(), "ramp_out", "one microsecond short of the fifth");
    session.command("input_frame", &at(5, 0, 1_000_000 + RAMP_US / 5 + RAMP_US / 5));
    assert_eq!(session.lifecycle(), "running");
    assert_eq!(scale(&session), 65535);
}

#[test]
fn a_reversal_made_at_once_costs_nothing_and_a_reversal_reverses_again() {
    let mut session = opened(KEY);
    session.command("input_frame", &toggling_at(1, 0, 1_000_000));
    assert_eq!(session.lifecycle(), "ramp_in");

    // A cancel in the same breath: the entry had spent nothing, so the exit
    // has nothing to spend and the next frame is back at full speed.
    session.command("input_frame", &toggling_at(2, 0, 1_000_000));
    assert_eq!(session.lifecycle(), "ramp_out");
    assert_eq!(scale(&session), 65535, "and the scale never left full");
    session.command("input_frame", &at(3, 0, 1_000_001));
    assert_eq!(session.lifecycle(), "running");

    // A reversal is a ramp like any other, so it reverses too. Halfway down,
    // turned, then turned back: the second turn mirrors the first.
    session.command("input_frame", &toggling_at(4, 0, 2_000_000));
    session.command("input_frame", &at(5, 0, 2_000_000 + RAMP_US / 2));
    let half = scale(&session);
    session.command("input_frame", &toggling_at(6, 0, 2_000_000 + RAMP_US / 2));
    assert_eq!(session.lifecycle(), "ramp_out");
    session.command("input_frame", &toggling_at(7, 0, 2_000_000 + RAMP_US / 2));
    assert_eq!(session.lifecycle(), "ramp_in");
    assert_eq!(scale(&session), half, "two turns at one instant stand where one did");
    session.command("input_frame", &at(8, 0, 2_000_000 + RAMP_US));
    assert_eq!(session.lifecycle(), "still", "and the half it had left completed it");
}

#[test]
fn the_ramp_out_completes_into_running_and_the_scale_rises() {
    let mut session = opened(KEY);
    let now = entered(&mut session, 1);

    session.command("input_frame", &toggling_at(3, 0, now + FRAME_US));
    assert_eq!(session.lifecycle(), "ramp_out");
    assert_eq!(scale(&session), 0, "the way out opens where the way in ended");

    session.command("input_frame", &at(4, 1, now + FRAME_US + RAMP_US / 2));
    assert_eq!(session.lifecycle(), "ramp_out");
    let half = (RAMP_US / 2) * 30;
    assert_eq!(scale(&session), (half * 65_536 / RAMP_UNITS) as u16, "and rises linearly");

    session.command("input_frame", &at(5, 1, now + FRAME_US + RAMP_US));
    assert_eq!(session.lifecycle(), "running");
    assert_eq!(scale(&session), 65535, "a completed exit stands at full speed again");
}

#[test]
fn a_pause_remembers_a_still_run_and_the_release_puts_it_back() {
    // A window that goes away mid-inspection has not decided to stop reading
    // the Field, so the mode the pause interrupted is what the release returns
    // to — and the surface comes back with it, on the frame that returns.
    let mut session = opened(KEY);
    let now = entered(&mut session, 1);
    let held = session.step();

    session.command("input_frame", &built(3, Some(0), false, true, now + FRAME_US, 0));
    assert_eq!(session.lifecycle(), "suspended");
    // A level pause stays a level pause, and holding it overwrites nothing.
    session.command("input_frame", &built(4, Some(0), false, true, now + 2 * FRAME_US, 0));
    assert_eq!(session.lifecycle(), "suspended");

    session.command("input_frame", &at(5, 1, now + 3 * FRAME_US));
    assert_eq!(session.lifecycle(), "still", "the release lands back in the pause it left");
    assert_eq!(scale(&session), 0);
    assert!(still_flag(&session), "and the surface is up on the frame that returned");
    assert_eq!(
        section(&session, 9),
        Some(1),
        "overlay and all, carrying the one window step the run has stood through",
    );
    assert_eq!(session.step(), held, "and no step ran on the way through");
}

#[test]
fn a_pause_that_interrupts_a_ramp_discards_it_and_releases_into_running() {
    let mut session = opened(KEY);
    session.command("input_frame", &toggling_at(1, 1, 1_000_000));
    session.command("input_frame", &at(2, 1, 1_000_000 + RAMP_US - FRAME_US));
    assert_eq!(session.lifecycle(), "ramp_in");

    session.command("input_frame", &built(3, Some(0), false, true, 1_300_000, 0));
    assert_eq!(session.lifecycle(), "suspended");
    session.command("input_frame", &at(4, 1, 1_400_000));
    assert_eq!(session.lifecycle(), "running", "a ramp is real time, and a pause spends none");
    assert_eq!(scale(&session), 65535);
}

#[test]
fn a_pause_that_interrupts_the_way_out_releases_into_running_too() {
    let mut session = opened(KEY);
    let now = entered(&mut session, 1);
    session.command("input_frame", &toggling_at(3, 0, now + FRAME_US));
    assert_eq!(session.lifecycle(), "ramp_out");

    session.command("input_frame", &built(4, Some(0), false, true, now + 2 * FRAME_US, 0));
    session.command("input_frame", &at(5, 1, now + 3 * FRAME_US));
    assert_eq!(session.lifecycle(), "running", "the way out was a ramp, and it went with it");
}

// ---------------------------------------------------------------------------
// What a still run does with a frame
// ---------------------------------------------------------------------------

#[test]
fn a_still_run_executes_no_step_however_many_a_frame_asks_for() {
    let mut session = opened(KEY);
    let now = entered(&mut session, 1);
    let before = session.step();

    let answer = body(&session.command("input_frame", &at(3, 30, now + FRAME_US)));
    assert_eq!(int_of(&answer, "steps_run"), 0, "the simulation is fully paused");
    assert_eq!(session.step(), before);
}

#[test]
fn a_frame_a_still_run_takes_changes_no_byte_of_it() {
    let mut session = opened(KEY);
    entered(&mut session, 1);
    let before = session.run().expect("a run is loaded").payload().expect("inside the cap");

    // Steering, a held Pulse, a release, a wheel gesture, and a bracket press:
    // every direct control there is, carried by a frame a still run reads.
    //
    // The wheel is the one that has to be said out loud. The locked deferral
    // rule accumulates a wheel delta on a frame that runs no step, because the
    // gesture it is part of will be resolved by a later frame that does — but
    // that rule is about a frame, and this is about a run. A still run takes no
    // input at all, so a wheel turned while the Field is being read is neither
    // resolved nor banked against the exit, and `wheel_accum` is a payload
    // field: banking it would make an inspection change the bytes a record
    // carries.
    let steered = format!(
        "{{\"advance_steps\":4,\"depth_key\":1,\"inspect\":null,\"pause\":false,\
          \"pulse_held\":true,\"pulse_release\":true,\"seq\":3,\"steer_x\":32000,\
          \"steer_y\":0,\"t_us\":400000,\"toggle_still\":false,\"wheel\":900}}"
    );
    body(&session.command("input_frame", &steered));

    let after = session.run().expect("a run is loaded").payload().expect("inside the cap");
    assert_eq!(before, after, "direct movement is inert while the run is paused");
}

/// A payload with the record an assembly writes taken back out of it: the
/// slate itself, and the two Field fields that count assemblies.
///
/// Entry into Still Mode assembles a slate at the same instant the ramp
/// completes, so the payload legitimately moves on that frame. What a frame
/// *banks* is a question about the input it carried, and this is how a test
/// asks it: everything but the assembly, byte for byte. The slate is cut by
/// its own key and the payload's last `view` — the one key written after it —
/// because a candidate carries a `view` of its own and only the outermost one
/// closes the record.
fn without_assembly(payload: &str) -> String {
    let opens = payload.find("\"slate\":").expect("the payload carries a slate key");
    let closes = payload.rfind(",\"view\":").expect("the payload ends with the standing View");
    let mut held = String::with_capacity(payload.len());
    held.push_str(&payload[..opens]);
    held.push_str("\"slate\":null");
    held.push_str(&payload[closes..]);
    held.replacen("\"assembly_ordinal\":1", "\"assembly_ordinal\":0", 1)
        .replacen("\"prev_assembly_step\":1,", "\"prev_assembly_step\":null,", 1)
}

#[test]
fn the_frame_that_completes_the_entry_ramp_banks_nothing_either() {
    // The frame that completes an entry ramp is the last one the shell filled
    // while the Field was still moving, so it carries whatever was held at
    // that moment — and by the time its input is read the run is `still`. The
    // guard is on the mode rather than on the step count precisely so that
    // this frame is covered by it.
    let mut session = opened(KEY);
    session.command("input_frame", &toggling_at(1, 1, 1_000_000));
    let before = session.run().expect("a run is loaded").payload().expect("inside the cap");

    let completing = built(2, Some(4), false, false, 1_000_000 + RAMP_US, 900);
    body(&session.command("input_frame", &completing));
    assert_eq!(session.lifecycle(), "still");
    let after = session.run().expect("a run is loaded").payload().expect("inside the cap");
    assert_eq!(
        before,
        without_assembly(&after),
        "the frame that stopped the Field spent none of what it held",
    );

    // What that frame did write is the slate the entry assembles, at the last
    // completed step, and the counters that name it.
    let state = session.run().expect("a run is loaded").state();
    assert_eq!(state.now.wheel_accum, 0, "and banked no wheel delta against the exit");
    assert_eq!(state.now.assembly_ordinal, 1, "one slate has been assembled");
    assert_eq!(state.now.prev_assembly_step, Some(state.now.step));
    let slate = session.run().expect("a run is loaded").standing_slate().expect("a slate stands");
    assert_eq!(slate.ordinal, 0, "the Run's first slate is ordinal 0");
    assert_eq!(slate.step, 1, "and its evaluation step is the last completed step");
}

#[test]
fn the_still_frame_carries_the_overlay_section_and_the_still_flag() {
    let mut session = opened(KEY);
    assert!(!still_flag(&session), "a running frame raises no still-surface flag");
    assert_eq!(section(&session, 9), None, "and carries no overlay section");

    let now = entered(&mut session, 1);
    assert!(still_flag(&session));
    assert_eq!(
        section(&session, 9),
        Some(1),
        "the overlay is present in still, one envelope entry per window step",
    );

    session.command("input_frame", &toggling_at(3, 0, now + FRAME_US));
    assert_eq!(session.lifecycle(), "ramp_out");
    assert_eq!(section(&session, 9), None, "and present only in still");
}

#[test]
fn a_filled_envelope_encodes_at_the_locked_record_layout() {
    // The overlay now carries the standing candidate's baseline envelope, and
    // this pins the record layout it crosses in, on a buffer built by hand so
    // the layout is asserted on its own terms rather than through a run: one
    // low and high per window step, `f32` little-endian, in the same one-way
    // conversion every other derived quantity crosses as.
    let field = FieldState::opening();
    let config = InputConfig::default_config();
    let progress = Progress::opening();
    let queue = PlanQueue::new();
    let envelope: Vec<(i64, i64)> = vec![(ONE_UNIT / 2, ONE_UNIT * 3), (0, ONE_UNIT)];
    let encoded = frame::encode(&frame::Snapshot {
        field: &field,
        mode: Mode::Still,
        time_scale: 0,
        view_inside: &[],
        queue: &queue,
        cues: &[],
        config: &config,
        progress: &progress,
        pressures: &[],
        objective_ordinal: 0,
        forecast: &envelope,
    });

    // The header raises the still-surface flag, and the section table names
    // the overlay with the count the envelope holds.
    assert_eq!(u16::from_le_bytes([encoded[6], encoded[7]]) & 1, 1);
    let mut found = None;
    for place in 0..usize::from(encoded[20]) {
        let at = 32 + place * 8;
        if encoded[at] == 9 {
            found = Some((
                u16::from_le_bytes([encoded[at + 2], encoded[at + 3]]),
                u32::from_le_bytes([
                    encoded[at + 4],
                    encoded[at + 5],
                    encoded[at + 6],
                    encoded[at + 7],
                ]) as usize,
            ));
        }
    }
    let (count, offset) = found.expect("a still frame carries the overlay section");
    assert_eq!(count, 2, "one record per window step");

    for (place, (low, high)) in envelope.iter().enumerate() {
        let at = offset + place * 8;
        let read = |from: usize| {
            f32::from_le_bytes([encoded[from], encoded[from + 1], encoded[from + 2], encoded[from + 3]])
        };
        assert_eq!(read(at), (*low as f64 / 65536.0) as f32, "the low of step {place}");
        assert_eq!(read(at + 4), (*high as f64 / 65536.0) as f32, "the high of step {place}");
    }
}

// ---------------------------------------------------------------------------
// The three queued-change commands
// ---------------------------------------------------------------------------

#[test]
fn the_queued_change_commands_are_valid_only_in_still() {
    let mut session = opened(KEY);
    for command in ["queue_plan", "undo_plan", "commit_plan"] {
        assert_eq!(refusal(&session.command(command, "{}")), "state", "{command} while running");
    }

    session.command("input_frame", &toggling_at(1, 1, 1_000_000));
    for command in ["queue_plan", "undo_plan", "commit_plan"] {
        assert_eq!(refusal(&session.command(command, "{}")), "state", "{command} while ramping");
    }

    session.command("input_frame", &at(2, 1, 1_000_000 + RAMP_US));
    assert_eq!(session.lifecycle(), "still");
    body(&session.command("undo_plan", "{}"));
}

#[test]
fn an_undo_on_an_empty_queue_succeeds_and_reports_nothing_remaining() {
    let mut session = opened(KEY);
    entered(&mut session, 1);

    let answer = body(&session.command("undo_plan", "{}"));
    assert_eq!(int_of(&answer, "remaining"), 0);
    let queue = answer.get("queue").expect("the queue state rides with it");
    assert_eq!(int_of(queue, "cost_total"), 0);
    assert_eq!(int_of(queue, "impulse"), 3, "the opening Impulse");
    assert_eq!(int_of(queue, "impulse_after"), 3, "an empty queue costs nothing");
    assert_eq!(queue.get("entries"), Some(&Json::List(Vec::new())));
    assert_eq!(session.lifecycle(), "still", "an undo is not an exit");
}

#[test]
fn an_empty_commit_applies_nothing_spends_nothing_and_leaves_still_mode() {
    let mut session = opened(KEY);
    let now = entered(&mut session, 1);

    let answer = body(&session.command("commit_plan", "{}"));
    assert_eq!(int_of(&answer, "applied"), 0);
    assert_eq!(int_of(&answer, "impulse"), 3, "nothing was queued, so nothing was spent");
    // The field names the slate the run stands under, and the entry that
    // opened this session assembled the Run's first — ordinal 0. An empty
    // commit reassembles nothing, so that is the ordinal it answers with; the
    // re-entry test below is where the number visibly moves.
    assert_eq!(int_of(&answer, "slate_ordinal"), 0, "the standing slate is the entry's");
    assert_eq!(session.lifecycle(), "ramp_out", "a committed exit is an exit");

    session.command("input_frame", &at(4, 1, now + RAMP_US));
    assert_eq!(session.lifecycle(), "running");
}

#[test]
fn an_empty_commit_after_a_reentry_answers_the_standing_ordinal() {
    // Entry assembles ordinal 0; the empty commit reassembles nothing and
    // leaves it standing; re-entry assembles ordinal 1; and the next empty
    // commit answers that — the response names the slate the run stands
    // under, never a literal zero.
    let mut session = opened(KEY);
    entered(&mut session, 1);
    let first = body(&session.command("commit_plan", "{}"));
    assert_eq!(int_of(&first, "slate_ordinal"), 0);

    // The committed exit opened a ramp of its own; a full ramp of real time
    // passes before the next frame, so it completes on arrival.
    let reopened = FRAME_US + 2 * RAMP_US;
    session.command("input_frame", &at(3, 1, reopened));
    assert_eq!(session.lifecycle(), "running", "the committed exit completed");
    session.command("input_frame", &toggling_at(4, 1, reopened + FRAME_US));
    session.command("input_frame", &at(5, 8, reopened + FRAME_US + RAMP_US));
    assert_eq!(session.lifecycle(), "still");

    let again = body(&session.command("commit_plan", "{}"));
    assert_eq!(int_of(&again, "applied"), 0);
    assert_eq!(int_of(&again, "slate_ordinal"), 1, "the re-entry's own slate stands");
}

#[test]
fn a_body_the_queue_cannot_read_is_refused_rather_than_half_read() {
    let mut session = opened(KEY);
    entered(&mut session, 1);

    // The entry reader belongs to the goal that owns queued changes; until it
    // lands, a well-shaped body is refused rather than queued unvalidated.
    assert_eq!(refusal(&session.command("queue_plan", "{\"plan\":{}}")), "validation");
    assert_eq!(refusal(&session.command("queue_plan", "{}")), "validation");
    let answer = body(&session.command("undo_plan", "{}"));
    assert_eq!(int_of(&answer, "remaining"), 0, "and nothing was queued by the attempt");
}

// ---------------------------------------------------------------------------
// What a still run does about the rest of the runtime
// ---------------------------------------------------------------------------

#[test]
fn a_restore_issued_mid_still_lands_in_running_with_the_queue_cleared() {
    let mut session = opened(KEY);
    entered(&mut session, 1);
    assert_eq!(session.lifecycle(), "still");

    let exported = body(&session.command("export_run", "{}"));
    let text = exported.get("text").and_then(Json::as_text).expect("an export file").to_string();

    let mut fresh = Session::new(&support::worker_init()).expect("versions agree");
    let mut import = String::from("{\"text\":");
    field_game_core::json::write_text(&mut import, &text);
    import.push('}');
    body(&fresh.command("import_run", &import));
    assert_eq!(fresh.lifecycle(), "running", "a record carries no mode into the run it opens");
    assert_eq!(scale(&fresh), 65535);
    assert!(fresh.run().expect("a run is loaded").queue().is_empty());
}

#[test]
fn a_checkpoint_restore_issued_mid_still_lands_in_running_too() {
    // The other restore path, and the one the locked rule names first: Quick
    // Retry over the run's own Anchor metadata, issued from `still`, which is
    // one of the loaded states every restore is valid in.
    let mut session = opened(KEY);
    // The opening chapter writes its Anchor at the completing objective, which
    // is further off than this test wants to play, so the record this restores
    // from is the autosave the cadence writes at the first interval.
    session.command("input_frame", &frame(1, 900));
    let anchors = session.run().expect("a run is loaded").state().anchors.clone();
    let anchor_id = anchors.first().expect("the cadence wrote a record").anchor_id;

    let now = entered(&mut session, 2);
    assert_eq!(session.lifecycle(), "still");
    session.command("input_frame", &at(4, 0, now + FRAME_US));

    let answer = body(&session.command("restore_checkpoint", &format!("{{\"anchor_id\":{anchor_id}}}")));
    assert_eq!(int_of(&answer, "step"), 900);
    assert_eq!(session.lifecycle(), "running", "a restore lands in running, whatever it left");
    assert_eq!(scale(&session), 65535, "with the time scale full");
    assert!(session.run().expect("a run is loaded").queue().is_empty(), "and the queue cleared");
    assert_eq!(section(&session, 9), None, "and no overlay, because nothing is still");

    // And the run carries on from there: the ramp went with the mode.
    session.command("input_frame", &at(5, 1, now + 2 * FRAME_US));
    assert_eq!(session.lifecycle(), "running");
    assert_eq!(session.step(), 901);
}

#[test]
fn a_recorded_trace_through_an_entry_and_an_exit_replays_byte_for_byte() {
    // Mode transitions are step input: they arrive on frames, and the frames
    // are what the trace records. So the same run key and the same frames —
    // toggle, ramp, pause, exit, and all — serialize to the same bytes twice,
    // which is the byte-equivalence contract read through Still Mode.
    let script: Vec<String> = vec![
        at(1, 3, 1_000_000),
        toggling_at(2, 2, 1_016_667),
        at(3, 1, 1_100_000),
        at(4, 1, 1_266_667),
        at(5, 4, 1_283_334),
        toggling_at(6, 0, 1_300_000),
        at(7, 2, 1_400_000),
        at(8, 3, 1_560_000),
        at(9, 5, 1_600_000),
    ];
    let played = |key: &str| -> String {
        let mut session = opened(key);
        let mut modes = Vec::new();
        for held in &script {
            session.command("input_frame", held);
            modes.push(session.lifecycle().to_string());
        }
        // The script really does pass through every mode of the table it can
        // reach, so what is compared is a trace that went somewhere.
        assert!(modes.contains(&"ramp_in".to_string()), "{modes:?}");
        assert!(modes.contains(&"still".to_string()), "{modes:?}");
        assert!(modes.contains(&"ramp_out".to_string()), "{modes:?}");
        assert_eq!(modes.last().map(String::as_str), Some("running"));
        session.run().expect("a run is loaded").payload().expect("inside the cap")
    };
    assert_eq!(played(KEY), played(KEY));
}

#[test]
fn an_autosave_due_inside_still_mode_waits_for_the_exit() {
    let mut session = opened(KEY);
    // One step short of the first autosave interval — the toggling frame runs
    // the last of them — and then into Still Mode.
    session.command("input_frame", &frame(1, 898));
    let now = entered(&mut session, 2);
    assert_eq!(session.step(), 899);

    let stored = session.store().of_run(KEY).len();
    session.command("input_frame", &at(4, 30, now + FRAME_US));
    assert_eq!(session.step(), 899, "a still run runs no step, so none is due");
    assert_eq!(
        session.store().of_run(KEY).len(),
        stored,
        "and nothing is written inside Still Mode",
    );

    session.command("input_frame", &toggling_at(5, 0, now + 2 * FRAME_US));
    assert_eq!(session.lifecycle(), "ramp_out");
    session.command("input_frame", &at(6, 8, now + 2 * FRAME_US + RAMP_US));
    assert_eq!(session.lifecycle(), "running");
    assert_eq!(session.step(), 907, "the exit spends the steps the frame asked for");
    assert!(
        session.store().of_run(KEY).len() > stored,
        "the exit is where the due write happens",
    );
}
