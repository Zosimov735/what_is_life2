//! The deterministic fixed-step runtime.
//!
//! The contract under test is the one
//! `docs/field-framework/ARCHITECTURE.md` states: two serializations of the
//! run state produced at the same step from the same run key, branch nonce,
//! content hash, and frame sequence are byte-equivalent. Around it sit the
//! rest of this goal's runtime — the pause level, the completed-step counter,
//! the retained trajectory, and the bounded queue of proposed changes.

use field_game_core::fault::Code;
use field_game_core::json::canonicalize;
use field_game_core::plan::{PlanCommand, PlanQueue, PLAN_QUEUE_DEPTH};
use field_game_core::run::Run;
use field_game_core::state::{Trace, ViewDeclaration};
use field_game_core::Session;

mod support;

/// What the worker sends when it opens the core.

const KEY: &str = "0123456789abcdef";
const OTHER_KEY: &str = "fedcba9876543210";

/// The answer a valid frame gets: the exact step count, which the worker reads
/// and does not post — the frame event is what acknowledges the frame.
fn accepted(steps: u16) -> String {
    format!("{{\"body\":{{\"steps_run\":{steps}}},\"ok\":true}}")
}

/// One frame of input, with every declared field present.
fn frame(seq: u32, steer_x: i16, steps: u16, pause: bool) -> String {
    wheeled(seq, steer_x, steps, pause, 0, 0)
}

/// The same, turning the wheel and the bracket keys.
fn wheeled(seq: u32, steer_x: i16, steps: u16, pause: bool, wheel: i16, depth_key: i8) -> String {
    format!(
        "{{\"advance_steps\":{steps},\"depth_key\":{depth_key},\"inspect\":null,\"pause\":{pause},\
          \"pulse_held\":false,\"pulse_release\":false,\"seq\":{seq},\"steer_x\":{steer_x},\
          \"steer_y\":0,\"t_us\":{stamp},\"toggle_still\":false,\"wheel\":{wheel}}}",
        stamp = i64::from(seq) * 33_333
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

/// Runs a script of (steering, steps, paused) frames and returns the canonical
/// bytes of the run state at the end of it.
fn played(key: &str, script: &[(i16, u16, bool)]) -> String {
    let mut session = opened(key);
    for (index, (steer, steps, paused)) in script.iter().enumerate() {
        let seq = index as u32 + 1;
        let ran = if *paused { 0 } else { *steps };
        assert_eq!(
            session.command("input_frame", &frame(seq, *steer, *steps, *paused)),
            accepted(ran)
        );
    }
    session.run().expect("a run is loaded").state().payload()
}

/// A script of `count` frames, each asking for one step.
fn steady(count: u32, steer: i16) -> Vec<(i16, u16, bool)> {
    (0..count).map(|_| (steer, 1, false)).collect()
}

#[test]
fn the_same_run_key_and_the_same_frames_serialize_to_the_same_bytes() {
    let script = [(0, 1, false), (12_000, 2, false), (-4_500, 0, false), (300, 6, false)];
    let once = played(KEY, &script);
    let again = played(KEY, &script);

    assert_eq!(once, again, "byte-equivalent, not merely equivalent");
    assert_eq!(
        canonicalize(&once).expect("the payload is canonical"),
        once,
        "the payload reads back as the bytes it was written as"
    );
    assert!(once.contains("\"step\":9"), "nine steps ran: {once}");
}

#[test]
fn a_different_run_key_diverges() {
    let script = steady(5, 0);
    assert_ne!(played(KEY, &script), played(OTHER_KEY, &script));
}

#[test]
fn a_different_frame_sequence_diverges() {
    // The same key, the same step count, one differing control state.
    let straight = played(KEY, &steady(5, 0));
    let steered = played(KEY, &[(0, 1, false), (900, 1, false), (0, 1, false), (0, 1, false), (0, 1, false)]);
    assert_ne!(straight, steered);

    // And the same key with a different number of steps.
    assert_ne!(played(KEY, &steady(5, 0)), played(KEY, &steady(6, 0)));
}

#[test]
fn pausing_injects_no_steps_and_changes_no_byte() {
    let unpaused = steady(8, 250);
    let mut interrupted: Vec<(i16, u16, bool)> = Vec::new();
    for (position, frame) in unpaused.iter().enumerate() {
        // Three real-time intervals spent paused, each asking for a step that
        // a suspended run never runs.
        if position % 3 == 0 {
            interrupted.push((250, 1, true));
        }
        interrupted.push(*frame);
    }
    assert_eq!(interrupted.len(), unpaused.len() + 3);

    assert_eq!(played(KEY, &unpaused), played(KEY, &interrupted));
}

#[test]
fn the_frame_counter_rises_only_on_executed_steps_and_never_falls() {
    let mut session = opened(KEY);
    let script = [(0, 2, false), (0, 3, true), (0, 0, false), (0, 4, false), (0, 1, true), (0, 2, false)];
    let mut counted = 0u32;
    let mut seen = Vec::new();

    for (index, (steer, steps, paused)) in script.iter().enumerate() {
        let before = session.step();
        let ran = if *paused { 0 } else { u32::from(*steps) };
        assert_eq!(
            session.command("input_frame", &frame(index as u32 + 1, *steer, *steps, *paused)),
            accepted(ran as u16)
        );
        let after = session.step();
        assert!(after >= before, "the counter never falls");
        assert_eq!(after - before, ran, "a suspended run injects no steps");
        counted += ran;
        seen.push(after);
    }

    assert_eq!(session.step(), counted);
    assert_eq!(seen, vec![2, 2, 2, 6, 6, 8]);
    assert_eq!(session.lifecycle(), "running", "the last frame released the pause");
}

#[test]
fn the_pause_level_moves_the_lifecycle_state_and_releasing_it_moves_it_back() {
    let mut session = opened(KEY);
    assert_eq!(session.lifecycle(), "running");
    session.command("input_frame", &frame(1, 0, 1, true));
    assert_eq!(session.lifecycle(), "suspended");
    session.command("input_frame", &frame(2, 0, 1, true));
    assert_eq!(session.lifecycle(), "suspended", "the pause is a level, not an edge");
    session.command("input_frame", &frame(3, 0, 1, false));
    assert_eq!(session.lifecycle(), "running");
    assert_eq!(session.step(), 1);
}

#[test]
fn the_retained_trajectory_spans_the_locked_window() {
    assert_eq!(Trace::start_for(0), 0);
    assert_eq!(Trace::start_for(119), 0);
    assert_eq!(Trace::start_for(149), 0);
    assert_eq!(Trace::start_for(150), 30);
    assert_eq!(Trace::start_for(179), 30);
    assert_eq!(Trace::start_for(180), 60);

    let mut session = opened(KEY);
    for seq in 1..=200u32 {
        session.command("input_frame", &frame(seq, 0, 1, false));
    }
    let run = session.run().expect("a run is loaded");
    let trace = &run.state().trace;
    assert_eq!(run.step(), 200);
    assert_eq!(trace.start_step, 60);
    assert_eq!(trace.keyframe.step, 60, "the keyframe holds the state as of its own step");
    assert_eq!(trace.steps.len(), 140, "120 to 150 steps are retained");
    assert_eq!(trace.steps.front().expect("a recorded step").step, 61);
    assert_eq!(trace.steps.back().expect("a recorded step").step, 200);
}

#[test]
fn the_stream_position_is_recorded_with_every_step() {
    let mut session = opened(KEY);
    for seq in 1..=3u32 {
        session.command("input_frame", &frame(seq, 0, 1, false));
    }
    let run = session.run().expect("a run is loaded");

    // The Noise flow scale draws one word per layer with positive effective
    // noise per step — the one drawing rule — and `the_pull` authors noise on
    // both of its layers, so each step consumes exactly two words from the
    // trajectory stream. The trace records the position before each step, so
    // a replay of any recorded step re-draws exactly what the live step drew.
    let mut expected = field_game_core::rng::trajectory_stream(KEY, 0);
    for recorded in &run.state().trace.steps {
        assert_eq!(recorded.rng, expected, "the position before the step is recorded");
        expected.draw(65_537);
        expected.draw(65_537);
    }
    assert_eq!(run.state().rng, expected, "and the live position is past the last draw");
}

#[test]
fn a_repeated_or_stale_frame_number_is_refused() {
    let mut session = opened(KEY);
    assert_eq!(session.command("input_frame", &frame(4, 0, 1, false)), accepted(1));
    for stale in [1, 4] {
        let refused = session.command("input_frame", &frame(stale, 0, 1, false));
        assert!(refused.contains("\"code\":\"validation\""), "{refused}");
        assert!(refused.contains("\"field\":\"seq\""), "{refused}");
    }
    assert_eq!(session.step(), 1, "a refused frame runs nothing");
}

#[test]
fn a_frame_outside_its_locked_ranges_is_refused() {
    let mut session = opened(KEY);
    let refusals = [
        // The steering component −32768 is never sent.
        ("steer_x", frame(1, 0, 1, false).replace("\"steer_x\":0", "\"steer_x\":-32768")),
        // The steering vector's magnitude is at most 32767 raw.
        (
            "steer_x",
            frame(1, 0, 1, false)
                .replace("\"steer_x\":0", "\"steer_x\":32000")
                .replace("\"steer_y\":0", "\"steer_y\":32000"),
        ),
        ("wheel", frame(1, 0, 1, false).replace("\"wheel\":0", "\"wheel\":3001")),
        ("depth_key", frame(1, 0, 1, false).replace("\"depth_key\":0", "\"depth_key\":2")),
        (
            "advance_steps",
            frame(1, 0, 1, false).replace("\"advance_steps\":1", "\"advance_steps\":1801"),
        ),
        ("seq", frame(1, 0, 1, false).replace("\"seq\":1", "\"seq\":0")),
        ("pause", frame(1, 0, 1, false).replace("\"pause\":false", "\"pause\":1")),
    ];

    for (field, body) in refusals {
        let refused = session.command("input_frame", &body);
        assert!(refused.contains("\"code\":\"validation\""), "{field}: {refused}");
        assert!(refused.contains(&format!("\"field\":\"{field}\"")), "{field}: {refused}");
    }

    // A missing field is refused too: every declared field is always present.
    let short = "{\"seq\":1,\"t_us\":0}";
    assert!(session.command("input_frame", short).contains("\"code\":\"validation\""));
    assert_eq!(session.step(), 0);
}

#[test]
fn a_frame_carrying_a_float_is_refused_before_it_is_read() {
    let mut session = opened(KEY);
    let refused =
        session.command("input_frame", &frame(1, 0, 1, false).replace("\"t_us\":33333", "\"t_us\":33333.5"));
    assert!(refused.contains("\"code\":\"validation\""), "{refused}");
    assert!(refused.contains("\"reason\":\"float\""), "{refused}");
}

#[test]
fn the_queue_of_proposed_changes_is_bounded_and_refuses_rather_than_drops() {
    let mut queue = PlanQueue::new();
    assert_eq!(PLAN_QUEUE_DEPTH, 6);
    assert!(queue.is_empty());

    for position in 1..=PLAN_QUEUE_DEPTH as u32 {
        let queued = queue
            .push(PlanCommand::Cut { route: position })
            .expect("the queue holds six entries");
        assert_eq!(queued, position as usize);
    }

    let refused = queue.push(PlanCommand::Cut { route: 7 }).expect_err("the seventh is refused");
    assert_eq!(refused.code(), Code::Capacity);
    assert!(refused.write().contains("\"quantity\":\"plan_queue_depth\""));
    assert_eq!(queue.len(), PLAN_QUEUE_DEPTH, "a refusal leaves the queue as it was");
    assert_eq!(
        queue.entries().first(),
        Some(&PlanCommand::Cut { route: 1 }),
        "nothing already queued is dropped to make room"
    );

    // Undo removes the most recent entry, and an empty queue changes nothing.
    for remaining in (0..PLAN_QUEUE_DEPTH).rev() {
        queue.undo();
        assert_eq!(queue.len(), remaining);
    }
    assert!(queue.undo().is_none());
    assert!(queue.is_empty());
}

#[test]
fn a_run_validates_every_entry_before_it_queues_one() {
    // A run that has not established a Field holds no Route, so every entry
    // naming one names nothing. What the queue does with entries that do stand
    // — the depth, the cost, and the preconditions one at a time — is pinned in
    // the transaction tests, on a Field built to be edited.
    let mut run = Run::start(KEY, "thread", &support::content_hash()).expect("a valid key and Form open a run");
    assert_eq!(
        run.queue_plan(PlanCommand::Cut { route: 1 }).expect_err("nothing to cut").code(),
        Code::NotFound
    );
    assert!(run.queue().is_empty(), "and a refused entry is not queued");
    assert_eq!(run.undo_plan(), 0, "an undo on an empty queue changes nothing");
}

#[test]
fn a_run_opens_only_under_a_run_key_and_a_form_of_the_closed_set() {
    assert!(Run::start(KEY, "thread", &support::content_hash()).is_ok());
    assert!(Run::start(KEY, "chorus", &support::content_hash()).is_ok());
    for refused in ["0123456789ABCDEF", "0123", "0123456789abcdeg", ""] {
        assert_eq!(
            Run::start(refused, "thread", &support::content_hash()).expect_err("a bad key is refused").code(),
            Code::Validation
        );
    }
    assert_eq!(
        Run::start(KEY, "spiral", &support::content_hash()).expect_err("a Form outside the set is refused").code(),
        Code::Validation
    );
}

#[test]
fn the_export_file_carries_the_payload_and_its_hash() {
    let mut session = opened(KEY);
    session.command("input_frame", &frame(1, 0, 2, false));
    let exported = session.command("export_run", "{}");

    let payload = session.run().expect("a run is loaded").state().payload();
    let hash = field_game_core::json::hex_bytes(&field_game_core::sha256::digest(payload.as_bytes()));
    assert!(exported.contains("\\\"format\\\":\\\"field-game-run\\\""), "{exported}");
    assert!(exported.contains(&format!("\\\"payload_sha256\\\":\\\"{hash}\\\"")), "{exported}");
    assert!(
        exported.contains(&format!("\"filename_hint\":\"field-run-{KEY}-step-2.json\"")),
        "{exported}"
    );
}

#[test]
fn two_sessions_export_the_same_bytes_for_the_same_key_and_frames() {
    let script = steady(4, 700);
    let mut first = opened(KEY);
    let mut second = opened(KEY);
    for (index, (steer, steps, paused)) in script.iter().enumerate() {
        let body = frame(index as u32 + 1, *steer, *steps, *paused);
        first.command("input_frame", &body);
        second.command("input_frame", &body);
    }

    assert_eq!(first.command("export_run", "{}"), second.command("export_run", "{}"));
}

#[test]
fn the_render_snapshot_carries_the_locked_header() {
    let mut session = opened(KEY);
    session.command("input_frame", &frame(1, 0, 3, false));
    let view = session.frame_view();

    assert_eq!(&view[0..4], b"FGF1");
    assert_eq!(u16::from_le_bytes([view[4], view[5]]), 1);
    assert_eq!(u32::from_le_bytes([view[8], view[9], view[10], view[11]]), 3);
    assert_eq!(
        u16::from_le_bytes([view[12], view[13]]),
        65535,
        "full speed saturates the header's 16 bits, where 65536 has no place"
    );
    assert_eq!(view[14], 0, "the mode is running");
    assert_eq!(view[16], 3, "a new run opens with three Impulse");
    assert!(view[20] > 0, "the authored chapter's own parts stand");

    session.command("input_frame", &frame(2, 0, 1, true));
    let paused = session.frame_view();
    assert_eq!(paused[14], 4, "the mode is suspended");
    assert_eq!(u16::from_le_bytes([paused[12], paused[13]]), 0, "a suspended run does not advance");
    assert_eq!(u32::from_le_bytes([paused[8], paused[9], paused[10], paused[11]]), 3);
}

#[test]
fn the_standing_view_before_a_chapter_is_the_locked_shape() {
    // The empty inside is the one case the document allows, and it is the View
    // a run holds before a chapter is established. A run now opens on the
    // authored chapter, so what a session answers with is that chapter's own
    // opening View instead — the empty shape stands only where no chapter has
    // been established at all.
    let empty = ViewDeclaration::opening();
    assert_eq!(
        empty.written(),
        "{\"inside\":[],\"resolution\":1,\"surround\":\"adjacent\",\"window\":45}",
    );
    let mut session = Session::new(&support::worker_init()).expect("versions agree");
    let answer = session.command(
        "init_run",
        &format!("{{\"form\":\"thread\",\"mode\":\"new\",\"run_id\":\"{KEY}\"}}"),
    );
    let authored = "{\"inside\":[2,3,4],\"resolution\":1,\"surround\":\"adjacent\",\"window\":45}";
    assert!(answer.contains(&format!("\"view\":{authored}")), "{answer}");
    let payload = session.run().expect("a run is loaded").state().payload();
    assert!(payload.ends_with(&format!("\"view\":{authored}}}")), "{payload}");
}

#[test]
fn a_new_run_opens_with_the_locked_defaults() {
    let payload = played(KEY, &[]);
    assert!(payload.contains("\"impulse\":3"), "{payload}");
    assert!(payload.contains("\"branch_nonce\":0"), "{payload}");
    assert!(payload.contains("\"chapter_index\":0"), "{payload}");
    assert!(payload.contains("\"pointer_speed\":65536"), "{payload}");
    assert!(payload.contains("\"sound_level\":65536"), "{payload}");
    assert!(payload.contains("\"trail_intensity\":65536"), "{payload}");
    assert!(payload.contains("\"reduced_motion\":false"), "{payload}");
    // An objective's id is empty exactly while it is hidden and none has been
    // offered. A run now opens on the authored chapter, which offers its first
    // objective at establishment, so what a new run's payload carries is that
    // objective rather than the hidden shape.
    assert!(
        payload.contains(
            "\"id\":\"objective.the_pull.follow_current\",\"progress\":0,\"started_step\":0,\
             \"state\":\"active\""
        ),
        "the opening objective stands from the first frame: {payload}"
    );
    assert!(
        !payload.contains("\"state\":\"hidden\""),
        "and nothing is left hidden behind it: {payload}"
    );
}

#[test]
fn the_wheel_resolves_one_depth_change_and_then_holds_off() {
    let mut session = opened(KEY);
    // Below the trigger, the accumulator carries and nothing resolves.
    session.command("input_frame", &wheeled(1, 0, 1, false, 300, 0));
    let run = session.run().expect("a run is loaded");
    assert_eq!(run.state().now.wheel_accum, 300);
    assert_eq!(run.state().now.depth_cooldown, 0);
    assert_eq!(run.state().trace.steps.back().expect("a step").ctl.depth_move, 0);

    // Crossing it resolves one change, clears the accumulator, and starts the
    // cooldown, which the executed step then counts down.
    session.command("input_frame", &wheeled(2, 0, 1, false, 300, 0));
    let run = session.run().expect("a run is loaded");
    assert_eq!(run.state().now.wheel_accum, 0);
    assert_eq!(run.state().now.depth_cooldown, 14);
    assert_eq!(run.state().trace.steps.back().expect("a step").ctl.depth_move, 1);

    // Inside the cooldown nothing resolves, however hard the wheel turns.
    for seq in 3..=6u32 {
        session.command("input_frame", &wheeled(seq, 0, 1, false, 3000, 1));
        let recorded = session.run().expect("a run is loaded");
        assert_eq!(recorded.state().trace.steps.back().expect("a step").ctl.depth_move, 0);
    }
    let run = session.run().expect("a run is loaded");
    assert_eq!(run.state().now.depth_cooldown, 10);
    assert_eq!(run.state().now.wheel_accum, 480, "the accumulator is held at the trigger");

    // Once it runs out the held accumulator fires again, downward this time
    // after the wheel is turned the other way.
    session.command("input_frame", &wheeled(7, 0, 10, false, 0, 0));
    let run = session.run().expect("a run is loaded");
    assert_eq!(run.state().now.depth_cooldown, 0);
    session.command("input_frame", &wheeled(8, 0, 1, false, -3000, 0));
    let run = session.run().expect("a run is loaded");
    assert_eq!(run.state().trace.steps.back().expect("a step").ctl.depth_move, -1);
}

#[test]
fn one_frame_yields_at_most_one_depth_change_however_many_steps_it_runs() {
    let mut session = opened(KEY);
    session.command("input_frame", &wheeled(1, 0, 6, false, 480, 0));
    let run = session.run().expect("a run is loaded");
    let recorded: Vec<i8> =
        run.state().trace.steps.iter().map(|step| step.ctl.depth_move).collect();
    assert_eq!(recorded, vec![1, 0, 0, 0, 0, 0], "only the first step of the batch carries it");
    assert_eq!(run.state().now.depth_cooldown, 9, "six executed steps counted the cooldown down");
}

#[test]
fn the_bracket_keys_resolve_only_outside_the_cooldown() {
    let mut session = opened(KEY);
    session.command("input_frame", &wheeled(1, 0, 1, false, 0, 1));
    let run = session.run().expect("a run is loaded");
    assert_eq!(run.state().trace.steps.back().expect("a step").ctl.depth_move, 1);
    assert_eq!(run.state().now.depth_cooldown, 0, "a bracket key starts no cooldown");

    // A wheel trigger does, and it holds the bracket keys off too.
    session.command("input_frame", &wheeled(2, 0, 1, false, 480, 0));
    session.command("input_frame", &wheeled(3, 0, 1, false, 0, -1));
    let run = session.run().expect("a run is loaded");
    assert_eq!(run.state().trace.steps.back().expect("a step").ctl.depth_move, 0);
}

#[test]
fn a_frame_that_runs_no_step_holds_its_depth_change_for_the_first_frame_that_does() {
    // Half the rendered frames of a run at the 60-frames-per-second target run
    // no step at all against 30 steps per second, so a resolution that fired
    // into a stepless batch would lose every other gesture — and would clear
    // the accumulator and start the cooldown for a change that never happened.
    // The locked rule defers instead: a frame that executes no step resolves no
    // depth change and consumes no press, while the wheel delta it carries
    // still accumulates — into the payload field that already holds it, which
    // is why the wheel needs nothing else to carry the gesture forward.
    let mut session = opened(KEY);
    session.command("input_frame", &wheeled(1, 0, 0, false, 480, 0));
    let run = session.run().expect("a run is loaded");
    assert_eq!(run.state().now.wheel_accum, 480, "the accumulator holds the crossing");
    assert_eq!(run.state().now.depth_cooldown, 0, "and no cooldown started");
    assert_eq!(run.state().now.step, 0, "because no step ran");

    // The first frame that does execute one resolves it, exactly once.
    session.command("input_frame", &wheeled(2, 0, 2, false, 0, 0));
    let run = session.run().expect("a run is loaded");
    let recorded: Vec<i8> =
        run.state().trace.steps.iter().map(|step| step.ctl.depth_move).collect();
    assert_eq!(recorded, vec![1, 0], "only the first step of the batch carries it");
    assert_eq!(run.state().now.wheel_accum, 0);
    assert_eq!(run.state().now.depth_cooldown, 13, "two executed steps counted it down");
}

#[test]
fn a_bracket_press_a_stepless_frame_carries_is_not_kept_by_the_run() {
    let mut session = opened(KEY);
    let before = session.run().expect("a run is loaded").state().payload();

    // A frame that executes no step resolves no depth change and keeps no press:
    // the run holds nothing of it, and the payload it would be exported to says
    // so — not one byte moved. Offering the press again is the shell's half of
    // the deferral, because the shell's whole output is the recorded frames and
    // a record taken here has to be one a restore can carry on from.
    session.command("input_frame", &wheeled(1, 0, 0, false, 0, 1));
    let run = session.run().expect("a run is loaded");
    assert_eq!(run.state().now.step, 0, "nothing ran");
    assert_eq!(run.state().payload(), before, "and nothing was kept");

    // A frame after it that carries no press resolves none.
    session.command("input_frame", &wheeled(2, 0, 1, false, 0, 0));
    let run = session.run().expect("a run is loaded");
    assert_eq!(run.state().trace.steps.back().expect("a step").ctl.depth_move, 0);

    // Offered again on a frame that executes a step, it resolves — once, and
    // starting no cooldown, which is what makes the way back up immediate.
    session.command("input_frame", &wheeled(3, 0, 1, false, 0, 1));
    let run = session.run().expect("a run is loaded");
    assert_eq!(run.state().trace.steps.back().expect("a step").ctl.depth_move, 1);
    assert_eq!(run.state().now.depth_cooldown, 0, "a bracket key starts no cooldown");

    session.command("input_frame", &wheeled(4, 0, 1, false, 0, 0));
    let run = session.run().expect("a run is loaded");
    assert_eq!(run.state().trace.steps.back().expect("a step").ctl.depth_move, 0);
}

#[test]
fn a_paused_frame_resolves_no_depth_and_accumulates_none() {
    let mut session = opened(KEY);
    session.command("input_frame", &wheeled(1, 0, 1, false, 300, 0));
    let run = session.run().expect("a run is loaded");
    let held = run.state().payload();
    assert_eq!(run.state().now.wheel_accum, 300);

    // A suspended run consumes no input at all: the wheel turns and the
    // brackets go down against a run that is not reading them, and the
    // accumulator that would decide the next change is left exactly where the
    // pause found it. That is what makes a pause byte-neutral, and the
    // accumulator is in the payload, so it would not be if the wheel were read.
    for seq in 2..=5u32 {
        session.command("input_frame", &wheeled(seq, 0, 1, true, 3000, 1));
    }
    let run = session.run().expect("a run is loaded");
    assert_eq!(run.state().now.wheel_accum, 300, "nothing accumulated while suspended");
    assert_eq!(run.state().now.depth_cooldown, 0);
    assert_eq!(run.state().payload(), held, "not one byte moved");

    // A press a paused frame carries is read by nothing and kept by nothing, so
    // the frame that ends the pause resolves it only if it carries it. What the
    // shell was holding when the window went away is let go of there, on the
    // same locked rule that drops a fumbled Pulse hold.
    session.command("input_frame", &wheeled(6, 0, 1, true, 0, 1));
    session.command("input_frame", &wheeled(7, 0, 1, false, 0, 0));
    let run = session.run().expect("a run is loaded");
    assert_eq!(run.state().trace.steps.back().expect("a step").ctl.depth_move, 0);
}

#[test]
fn a_noisy_wheel_schedule_serializes_the_same_bytes_twice() {
    // Trackpad noise, as a schedule: small deltas either way, frames that run
    // no step among them, and two deliberate gestures that do cross the
    // trigger. The bytes are the same both times, which is what makes a
    // recorded input trace with wheel noise in it replay exactly.
    let script: [(i16, u16); 16] = [
        // Noise: small deltas either way, some of them on frames that run
        // nothing. None of it reaches the trigger.
        (12, 1),
        (-8, 0),
        (17, 1),
        (-21, 0),
        (9, 1),
        // A deliberate turn, its crossing landing on a frame that runs no step.
        (240, 0),
        (240, 1),
        // More noise while the cooldown counts itself down.
        (-9, 1),
        (6, 0),
        (-11, 1),
        (0, 12),
        // And a deliberate turn back the other way.
        (-260, 0),
        (-260, 1),
        (7, 1),
        (-5, 0),
        (0, 1),
    ];
    let bytes = |key: &str| -> String {
        let mut session = opened(key);
        for (index, (wheel, steps)) in script.iter().enumerate() {
            session.command(
                "input_frame",
                &wheeled(index as u32 + 1, 0, *steps, false, *wheel, 0),
            );
        }
        let run = session.run().expect("a run is loaded");
        let moves: Vec<i8> =
            run.state().trace.steps.iter().map(|step| step.ctl.depth_move).collect();
        // Two gestures, two changes: the noise between them resolved nothing.
        assert_eq!(moves.iter().filter(|change| **change != 0).count(), 2, "{moves:?}");
        assert_eq!(moves.iter().sum::<i8>(), 0, "one down and one back up: {moves:?}");
        run.state().payload()
    };

    let once = bytes(KEY);
    assert_eq!(once, bytes(KEY), "byte-equivalent, not merely equivalent");
    assert_eq!(canonicalize(&once).expect("the payload is canonical"), once);
    assert_ne!(once, bytes(OTHER_KEY), "and the run key is still part of the bytes");
}

#[test]
fn a_run_stopped_mid_cooldown_serializes_the_same_bytes_twice() {
    // Both runs are left inside the cooldown, so the payload has to carry the
    // accumulator and the remaining cooldown for the bytes to repeat.
    let script = [(300i16, 1u16, 0i8), (300, 2, 0), (0, 1, 0), (120, 1, 1)];
    let bytes = |key: &str| -> String {
        let mut session = opened(key);
        for (index, (wheel, steps, depth_key)) in script.iter().enumerate() {
            session.command(
                "input_frame",
                &wheeled(index as u32 + 1, 0, *steps, false, *wheel, *depth_key),
            );
        }
        session.run().expect("a run is loaded").state().payload()
    };

    let once = bytes(KEY);
    let again = bytes(KEY);
    assert_eq!(once, again);
    assert!(once.contains("\"depth_cooldown\":11"), "{once}");
    assert!(once.contains("\"wheel_accum\":120"), "{once}");
    assert_eq!(canonicalize(&once).expect("the payload is canonical"), once);

    // And a different wheel schedule diverges, which is what makes those two
    // fields part of the contract rather than decoration.
    let mut other = opened(KEY);
    for (index, (_, steps, depth_key)) in script.iter().enumerate() {
        other.command(
            "input_frame",
            &wheeled(index as u32 + 1, 0, *steps, false, 0, *depth_key),
        );
    }
    assert_ne!(once, other.run().expect("a run is loaded").state().payload());
}
