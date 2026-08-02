//! The eight required perturbations, their playback records, and the Echo.
//!
//! `docs/field-framework/FRAMEWORK.md`'s Required perturbations section is what
//! is under test, and `docs/field-framework/ARCHITECTURE.md`'s Echo branch
//! beside it. The Field the readings run on is the circuit fixture, whose
//! recorded stored-Charge total of the inside `{2, 3, 4}` over eight steps is,
//! in whole units,
//!
//! ```text
//!   4, 8, 12, 16, 20, 16, 12, 8
//! ```
//!
//! Its range is 16 units, so the level margin is two and every four-unit step
//! reads as a direction: the recorded symbols are `+1 +1 +1 +1 -1 -1 -1`. Every
//! excess below is the arithmetic that series and one edit produce.
//!
//! Delayed replay runs on the drained fixture instead, because it is the one
//! kind that reads the exogenous schedule and the circuit has none.

use field_game_core::fx::ONE_UNIT;
use field_game_core::json::{parse, write_text, Json};
use field_game_core::perturb::{self, EchoTarget, Recomputed, Request};
use field_game_core::run::RAMP_UNITS;
use field_game_core::slate::TAU_DEFAULT;
use field_game_core::state::{RunState, Trace, FRAC_ONE};
use field_game_core::Session;

mod support;

use support::measure::{circuit, circuit_with_form, drained, played, recorded_units, sigma, view_of};

const WINDOW: u16 = 8;
const INSIDE: [u32; 3] = [2, 3, 4];

/// A ratio as the one rounding rule gives it: `num / den` times 65536, rounded
/// to nearest with halves upward. Every expected number below is written this
/// way so the arithmetic is visible rather than pasted.
fn frac(num: i64, den: i64) -> i64 {
    (i128::from(num) * i128::from(FRAC_ONE) * 2 + i128::from(den)) as i64 / (den * 2)
}

fn run_of(kind: &str, parameter: Option<u32>) -> perturb::PerturbationResult {
    let view = view_of(&INSIDE, WINDOW);
    let state = played(circuit(), usize::from(WINDOW), view.clone());
    perturb::run(
        &state,
        &view,
        &[],
        0,
        &sigma(),
        TAU_DEFAULT,
        Request::of(kind, parameter).expect("a kind of the closed set"),
    )
}

#[test]
fn the_fixture_records_the_series_every_excess_is_taken_against() {
    let view = view_of(&INSIDE, WINDOW);
    let state = played(circuit(), usize::from(WINDOW), view.clone());
    assert_eq!(recorded_units(&state, &INSIDE), vec![4, 8, 12, 16, 20, 16, 12, 8]);
}

#[test]
fn the_closed_kind_set_is_the_frameworks_eight() {
    assert_eq!(
        perturb::KINDS,
        [
            "boundary-severance",
            "route-removal",
            "component-substitution",
            "resolution-change",
            "window-change",
            "surround-change",
            "delayed-replay",
            "full-turnover",
        ],
    );
    assert!(Request::of("no-such-kind", None).is_none());
}

// ---------------------------------------------------------------------------
// The replaying kinds
// ---------------------------------------------------------------------------

#[test]
fn boundary_severance_removes_every_crossing_route_and_reads_the_whole_deviation() {
    // The crossing set is Route 1, which supplies the inside, and Route 4,
    // which takes from it. With both gone nothing enters and nothing leaves,
    // and the inside opens the window holding nothing: the replayed total is
    // zero at every step, so its symbols are all level and agree with the
    // recorded `+1 +1 +1 +1 -1 -1 -1` on none of the seven. The deviation is
    // 1 - 0 = 1, the baseline deviation is 0 because a replay with no edit
    // reproduces the window exactly, and the excess is the whole of it.
    let result = run_of(perturb::BOUNDARY_SEVERANCE, None);
    assert_eq!(result.parameter, None, "the kind takes no parameter");
    assert_eq!(result.reading.value, Some(i64::from(FRAC_ONE)));
    assert_eq!(result.reading.low, Some(i64::from(FRAC_ONE)));
    assert_eq!(result.reading.high, Some(i64::from(FRAC_ONE)));
    assert_eq!(result.samples.len(), 8);
    for sample in &result.samples {
        assert_eq!(sample.series, vec![0; usize::from(WINDOW)], "the playback record");
        assert_eq!(sample.base_series, None, "only delayed replay carries its own base");
    }
}

#[test]
fn route_removal_resolves_the_default_route_and_records_it() {
    // The default is the cyclic Route with an end in the inside carrying the
    // largest window flow, smallest identifier on equal flows. Routes 2 and 3
    // each carried eight units a step — 64 over the window — and Route 5 four a
    // step, so the tie is between 2 and 3 and the smaller identifier takes it.
    let result = run_of(perturb::ROUTE_REMOVAL, None);
    assert_eq!(result.parameter, Some(2), "a defaulted parameter is recorded resolved");
    // Without Route 2 the Charge Route 1 delivers piles up in Node 2 and
    // nothing reaches Node 4, so nothing leaves: the replayed total climbs
    // 8, 16, 24, 32, 40 and then stands, and its symbols are
    // `+1 +1 +1 +1 0 0 0`. Four of seven agree, so the deviation is 1 - 4/7
    // and the excess is the same against a baseline of 0.
    assert_eq!(result.reading.value, Some(i64::from(FRAC_ONE) - frac(4, 7)));
    assert_eq!(result.samples[0].series[0], 8 * ONE_UNIT);
    assert_eq!(result.samples[0].series[7], 40 * ONE_UNIT);
}

#[test]
fn route_removal_takes_the_route_a_caller_names() {
    let result = run_of(perturb::ROUTE_REMOVAL, Some(5));
    assert_eq!(result.parameter, Some(5), "the caller's Route, recorded as used");
}

#[test]
fn route_removal_is_unassigned_when_no_route_of_the_inside_carried() {
    // Node 6's one Route never carries, and it is the only Route with an end in
    // an inside of `{6}`.
    let view = view_of(&[6], WINDOW);
    let state = played(circuit(), usize::from(WINDOW), view_of(&INSIDE, WINDOW));
    let result = perturb::run(
        &state,
        &view,
        &[],
        0,
        &sigma(),
        TAU_DEFAULT,
        Request::of(perturb::ROUTE_REMOVAL, None).expect("a kind"),
    );
    assert_eq!(result.reading.value, None);
    assert_eq!(result.reading.reason, Some(perturb::NO_ROUTE));
    assert!(result.samples.is_empty(), "a kind that names no edit replays nothing");
}

#[test]
fn component_substitution_resolves_the_member_with_the_largest_mean_stored_charge() {
    // Node 2 held 4, 8, 12, 16, 20, 16, 12, 8 units over the window and Nodes 3
    // and 4 ended every step holding nothing, so the default is Node 2.
    let result = run_of(perturb::COMPONENT_SUBSTITUTION, None);
    assert_eq!(result.parameter, Some(2));
    // A `port` Node's kind holds nothing at creation and Node 2 held nothing at
    // the window start, so the replacement changes nothing at all and the
    // excess is honestly zero rather than absent.
    assert_eq!(result.reading.value, Some(0));
    assert_eq!(result.samples[0].series[0], 4 * ONE_UNIT);
}

#[test]
fn full_turnover_replaces_every_member_and_the_replacements_carry_their_kinds_charge() {
    // Node 4's kind is `reserve`, which holds 64 units at creation, so the
    // replayed inside opens 64 units above the recorded one and stays there:
    // 68, 72, 76, 80, 84, 80, 76, 72. A replay deviation reads directions, and
    // those are the recorded directions exactly, so the excess is zero — the
    // truthful reading of a turnover that moved the level and not the shape.
    let result = run_of(perturb::FULL_TURNOVER, None);
    assert_eq!(result.parameter, None, "the kind takes no parameter");
    assert_eq!(
        result.samples[0].series,
        vec![68, 72, 76, 80, 84, 80, 76, 72]
            .into_iter()
            .map(|units| units * ONE_UNIT)
            .collect::<Vec<i64>>(),
    );
    assert_eq!(result.reading.value, Some(0));
}

// ---------------------------------------------------------------------------
// The recomputing kinds
// ---------------------------------------------------------------------------

#[test]
fn resolution_change_re_reads_the_window_at_the_nearby_grains_without_replaying() {
    // The View's resolution is 1 and the inside holds three members, so the one
    // qualifying grain pair is (1, 2). In proximity order the members are 2, 3,
    // 4: at grain 1 the blocks are each member alone, and at grain 2 they are
    // `{2, 3}` and `{4}`. Nodes 3 and 4 end every step holding nothing, so
    // their series stand level; Node 2 carries the whole of the inside's total.
    //
    //   block {2}  against parent {2, 3}: the same series — agreement 1;
    //   block {3}  against parent {2, 3}: level against moving — agreement 0;
    //   block {4}  against parent {4}:    the same series — agreement 1.
    //
    // The pair value is the mean, 2/3, which is Scale Stability, and the
    // reading is one less it.
    let result = run_of(perturb::RESOLUTION_CHANGE, None);
    let stability = frac(2, 3);
    assert_eq!(result.reading.value, Some(i64::from(FRAC_ONE) - stability));
    assert_eq!(result.reading.samples, 1, "the kind takes one sample and runs no replay");
    assert!(result.samples.is_empty());
    assert!(result.streams.is_empty(), "no replay names a stream");
    let Some(Recomputed::Resolution { grains, pairs, blocks }) = result.recomputed else {
        panic!("the kind records the grains it used");
    };
    assert_eq!(grains, vec![1, 2]);
    assert_eq!(pairs, vec![(1, stability)], "the pair value keyed by its smaller grain");
    assert_eq!(blocks.len(), 2);
    assert_eq!(blocks[0].1.len(), 3, "three blocks at grain 1");
    assert_eq!(blocks[1].1.len(), 2, "two at grain 2");
}

#[test]
fn window_change_recomputes_the_halved_window_alone_when_the_trace_holds_no_more() {
    // The trajectory holds eight steps, so `2 * w` is more than it retains and
    // only `ceil(w / 2)` is recomputed. One recomputed window gives a point
    // confidence range.
    let result = run_of(perturb::WINDOW_CHANGE, None);
    let Some(Recomputed::Window { windows }) = &result.recomputed else {
        panic!("the kind records the windows it recomputed");
    };
    assert_eq!(windows.len(), 1);
    assert_eq!(windows[0].0, 4);
    assert_eq!(result.reading.low, result.reading.high, "a point range");
    // Cut Impact is the value that moves: over the whole window the severance
    // stops every circulating unit, and over the last four steps the severance
    // leaves Node 4 holding what the outlet would have taken, so the replayed
    // circulating flow is larger than the recorded one and the value clamps to
    // 0. That movement is the whole width of the range.
    assert_eq!(windows[0].1.cut_impact.value, Some(0));
    assert_eq!(windows[0].2, i64::from(FRAC_ONE));
    assert_eq!(result.reading.value, Some(i64::from(FRAC_ONE)));
    // Every stream the recomputation named is recorded, and each carries the
    // locked insertion.
    assert_eq!(result.streams.len(), 24, "three stochastic values, eight samples each");
    assert!(result.streams.iter().all(|name| name.contains("/perturbation/window/4/")));
}

#[test]
fn window_change_recomputes_the_doubled_window_when_the_trace_holds_it() {
    let view = view_of(&INSIDE, WINDOW);
    let state = played(circuit(), 20, view.clone());
    assert_eq!(state.effective_window(WINDOW * 2), WINDOW * 2, "the trace holds 2w");
    let result = perturb::run(
        &state,
        &view,
        &[],
        0,
        &sigma(),
        TAU_DEFAULT,
        Request::of(perturb::WINDOW_CHANGE, None).expect("a kind"),
    );
    let Some(Recomputed::Window { windows }) = &result.recomputed else {
        panic!("the kind records the windows it recomputed");
    };
    assert_eq!(windows.iter().map(|held| held.0).collect::<Vec<u16>>(), vec![4, 16]);
    assert!(result.reading.low.unwrap() <= result.reading.value.unwrap());
    assert!(result.reading.value.unwrap() <= result.reading.high.unwrap());
    assert_eq!(result.streams.len(), 48, "two windows, three values, eight samples");
}

#[test]
fn surround_change_moves_one_place_along_the_cycle_and_records_both_values() {
    // The View declares `adjacent`, whose surround is Nodes 1 and 5. The next
    // rule in the cycle is `double`, which adds Node 6 — the non-member with a
    // Route into Node 1 — so the two rules genuinely name different sets here.
    let result = run_of(perturb::SURROUND_CHANGE, None);
    let Some(Recomputed::Surround { rule, old, new }) = &result.recomputed else {
        panic!("the kind records the rule it moved to");
    };
    assert_eq!(rule.name(), "double");
    assert!(old.value.is_some() && new.value.is_some(), "both values are assigned here");
    let moved = (new.value.unwrap() - old.value.unwrap()).abs();
    assert_eq!(result.reading.value, Some(moved), "the magnitude of the change");
    assert_eq!(result.reading.low, Some(moved), "and a point range at it");
    assert_eq!(result.reading.high, Some(moved));
    assert!(result.streams.iter().all(|name| name.contains("/perturbation/surround/double/")));
}

#[test]
fn surround_change_reads_zero_when_neither_value_is_assigned() {
    // An inside of one member leaves Shared Failure unassigned under every
    // rule, so neither value is assigned and the reading is 0 — the framework's
    // own third case.
    let view = view_of(&[2], WINDOW);
    let state = played(circuit(), usize::from(WINDOW), view_of(&INSIDE, WINDOW));
    let result = perturb::run(
        &state,
        &view,
        &[],
        0,
        &sigma(),
        TAU_DEFAULT,
        Request::of(perturb::SURROUND_CHANGE, None).expect("a kind"),
    );
    assert_eq!(result.reading.value, Some(0));
    let Some(Recomputed::Surround { old, new, .. }) = &result.recomputed else {
        panic!("the kind records both values");
    };
    assert_eq!(old.value, None);
    assert_eq!(new.value, None);
}

#[test]
fn surround_change_reads_one_when_exactly_one_value_is_assigned() {
    // The inside {1, 6}: its one `adjacent` non-member is Node 2 — Node 6's
    // only Route runs into the inside — so Shared Failure is unassigned under
    // the declared rule for want of a second surround Node. The `double` rule
    // adds Node 2's own neighbourhood, Nodes 3 and 4, and the recomputed value
    // is assigned. Exactly one of the two stands, and FRAMEWORK.md's own second
    // case says the reading is 1.
    let view = view_of(&[1, 6], WINDOW);
    let state = played(circuit(), usize::from(WINDOW), view_of(&INSIDE, WINDOW));
    let result = perturb::run(
        &state,
        &view,
        &[],
        0,
        &sigma(),
        TAU_DEFAULT,
        Request::of(perturb::SURROUND_CHANGE, None).expect("a kind"),
    );
    let Some(Recomputed::Surround { old, new, .. }) = &result.recomputed else {
        panic!("the kind records both values");
    };
    assert_eq!(old.value, None, "one surround Node is one too few");
    assert!(new.value.is_some(), "the widened rule reads a value");
    assert_eq!(result.reading.value, Some(i64::from(FRAC_ONE)));
    assert_eq!(result.reading.low, Some(i64::from(FRAC_ONE)), "a point range at the reading");
    assert_eq!(result.reading.high, Some(i64::from(FRAC_ONE)));
}

// ---------------------------------------------------------------------------
// Delayed replay
// ---------------------------------------------------------------------------

#[test]
fn delayed_replay_shifts_the_exogenous_schedule_and_carries_two_series() {
    // The drained Field holds one Node of 60 units on a layer that removes six
    // a step, so its whole exogenous schedule is `-6` at every step and the
    // recorded totals are 54, 48, 42, 36, 30, 24, 18, 12.
    //
    // The default delay is `ceil(w / 4)` = 2. The base replay drives the
    // schedule unshifted and reproduces the window exactly. The shifted replay
    // has no event behind it for the first two steps, so the Node stands at 60
    // for both and then falls: 60, 60, 54, 48, 42, 36, 30, 24. The recorded
    // range is 42 units, so the margin is 5.25 and the level first step reads
    // as no direction against seven recorded falls: six of seven agree, the
    // deviation is 1 - 6/7, and the base deviation is 0.
    let view = view_of(&[1], WINDOW);
    let state = played(drained(), usize::from(WINDOW), view.clone());
    assert_eq!(recorded_units(&state, &[1]), vec![54, 48, 42, 36, 30, 24, 18, 12]);
    let result = perturb::run(
        &state,
        &view,
        &[],
        0,
        &sigma(),
        TAU_DEFAULT,
        Request::of(perturb::DELAYED_REPLAY, None).expect("a kind"),
    );
    assert_eq!(result.parameter, Some(2), "the resolved default delay");
    assert_eq!(result.reading.value, Some(i64::from(FRAC_ONE) - frac(6, 7)));
    for sample in &result.samples {
        assert_eq!(sample.base_deviation, Some(0), "the unshifted schedule is the base");
        let base = sample.base_series.as_ref().expect("the kind carries two series");
        assert_eq!(base[0], 54 * ONE_UNIT);
        assert_eq!(sample.series[0], 60 * ONE_UNIT, "no event stands behind the first step");
        assert_eq!(sample.series[1], 60 * ONE_UNIT);
        assert_eq!(sample.series[2], 54 * ONE_UNIT);
    }
    // Sixteen streams: one base and one shift per sample, each ending in the
    // locked final part.
    assert_eq!(result.streams.len(), 16);
    assert!(result.streams.iter().filter(|name| name.ends_with("/base")).count() == 8);
    assert!(result.streams.iter().filter(|name| name.ends_with("/shift")).count() == 8);
}

#[test]
fn delayed_replay_takes_the_delay_a_caller_names() {
    let view = view_of(&[1], WINDOW);
    let state = played(drained(), usize::from(WINDOW), view.clone());
    let result = perturb::run(
        &state,
        &view,
        &[],
        0,
        &sigma(),
        TAU_DEFAULT,
        Request::of(perturb::DELAYED_REPLAY, Some(3)).expect("a kind"),
    );
    assert_eq!(result.parameter, Some(3));
    assert_eq!(result.samples[0].series[2], 60 * ONE_UNIT, "three steps stand level now");
}

// ---------------------------------------------------------------------------
// The record, and reproducibility
// ---------------------------------------------------------------------------

#[test]
fn every_kind_reproduces_an_identical_record_from_identical_inputs() {
    for kind in perturb::KINDS {
        let first = run_of(kind, None).written();
        let second = run_of(kind, None).written();
        assert_eq!(first, second, "{kind} must reproduce its result in full");
    }
}

#[test]
fn a_different_root_random_state_is_the_only_thing_that_moves_a_stream() {
    let view = view_of(&INSIDE, WINDOW);
    let state = played(circuit(), usize::from(WINDOW), view.clone());
    let other = field_game_core::slate::evaluation_stream("fedcba9876543210", 0, 1);
    let held = perturb::run(
        &state,
        &view,
        &[],
        0,
        &other,
        TAU_DEFAULT,
        Request::of(perturb::BOUNDARY_SEVERANCE, None).expect("a kind"),
    );
    // The stream names are a function of the position and the kind, not of the
    // root, so they stand; the recorded root does not.
    assert_eq!(held.streams, run_of(perturb::BOUNDARY_SEVERANCE, None).streams);
    assert_ne!(held.sigma, sigma());
}

#[test]
fn a_restored_payload_reproduces_a_result_from_sigma_and_the_resolved_parameter() {
    // The locked Goal 3 adjudication, both halves: results are session-lived
    // and never serialized, and post-restore reproducibility comes from
    // `sigma_V` and the resolved parameters. So the payload is round-tripped
    // through the real import path, and the same kind asked with the recorded
    // resolved parameter under the recorded root reproduces the record byte
    // for byte — nothing of the result itself ever crossed.
    let view = view_of(&INSIDE, WINDOW);
    let state = played(circuit(), usize::from(WINDOW), view.clone());
    let before = perturb::run(
        &state,
        &view,
        &[],
        0,
        &sigma(),
        TAU_DEFAULT,
        Request::of(perturb::ROUTE_REMOVAL, None).expect("a kind"),
    );
    let resolved = before.parameter.expect("the default resolved") as u32;

    let mut session = imported(&state);
    let restored = session.run().expect("a run is loaded").state().clone();
    let after = perturb::run(
        &restored,
        &view,
        &[],
        0,
        &sigma(),
        TAU_DEFAULT,
        Request::of(perturb::ROUTE_REMOVAL, Some(resolved)).expect("a kind"),
    );
    assert_eq!(
        before.written(),
        after.written(),
        "the restored payload, the recorded sigma_V, and the resolved parameter reproduce the result",
    );
}

#[test]
fn the_record_carries_every_locked_key() {
    let written = run_of(perturb::ROUTE_REMOVAL, None).written();
    let parsed = parse(&written).expect("the result is canonical JSON");
    let Json::Map(entries) = &parsed else { panic!("the result is an object") };
    let keys: Vec<&str> = entries.iter().map(|(key, _)| key.as_str()).collect();
    assert_eq!(
        keys,
        vec![
            "kind",
            "parameter",
            "position",
            "provenance",
            "reading",
            "recomputed",
            "samples",
            "sigma",
            "step",
            "streams",
            "tau",
            "view",
        ],
    );
    // The five FRAMEWORK.md Records fields, each present: the View, the random
    // state and every stream name, the kind and its resolved parameters, the
    // tolerance, and the confidence range. The eight shared-baseline streams
    // stand first — their replays are what the excesses are taken against, so
    // they are streams this result used — then the kind's own eight.
    assert!(written.contains("\"tau\":8192"));
    assert!(written.contains("\"streams\":[\"candidate/0/baseline/1\""));
    assert!(written.contains("\"candidate/0/perturbation/route-removal/1\""));
    assert!(written.contains("\"parameter\":2"));
}

#[test]
fn a_window_too_short_to_compare_leaves_the_reading_unassigned_and_the_series_recorded() {
    let view = view_of(&INSIDE, 1);
    let state = played(circuit(), 1, view_of(&INSIDE, WINDOW));
    let result = perturb::run(
        &state,
        &view,
        &[],
        0,
        &sigma(),
        TAU_DEFAULT,
        Request::of(perturb::BOUNDARY_SEVERANCE, None).expect("a kind"),
    );
    assert_eq!(result.reading.value, None);
    assert_eq!(result.reading.reason, Some("window-too-short"));
    assert_eq!(result.samples.len(), 8, "the replayed series are still recorded");
    assert_eq!(result.samples[0].series.len(), 1);
}

// ---------------------------------------------------------------------------
// The Echo
// ---------------------------------------------------------------------------

#[test]
fn a_perturbation_backed_highlight_names_the_largest_excess_and_its_range() {
    let result = run_of(perturb::ROUTE_REMOVAL, None);
    let echo = result.highlight().expect("an assigned reading leaves a highlight");
    assert_eq!(echo.kind, perturb::ROUTE_REMOVAL);
    assert_eq!(echo.parameter, Some(2));
    assert_eq!(echo.excess, i64::from(FRAC_ONE) - frac(4, 7));
    assert_eq!(echo.target, EchoTarget::Route(2));
    assert!(echo.written().contains("\"t\":\"route\""));
}

#[test]
fn an_unassigned_reading_leaves_no_highlight() {
    let view = view_of(&[6], WINDOW);
    let state = played(circuit(), usize::from(WINDOW), view_of(&INSIDE, WINDOW));
    let result = perturb::run(
        &state,
        &view,
        &[],
        0,
        &sigma(),
        TAU_DEFAULT,
        Request::of(perturb::ROUTE_REMOVAL, None).expect("a kind"),
    );
    assert!(result.highlight().is_none(), "there is no excess to name");
}

// ---------------------------------------------------------------------------
// Through the bridge: the inspect surface and the Echo at the exit
// ---------------------------------------------------------------------------

/// The locked ramp span, in microseconds.
const RAMP_US: i64 = RAMP_UNITS / 30;

fn frame(seq: u32, steps: u16, toggle: bool, t_us: i64, inspect: &str) -> String {
    format!(
        "{{\"advance_steps\":{steps},\"depth_key\":0,\"inspect\":{inspect},\"pause\":false,\
          \"pulse_held\":false,\"pulse_release\":false,\"seq\":{seq},\"steer_x\":0,\
          \"steer_y\":0,\"t_us\":{t_us},\"toggle_still\":{toggle},\"wheel\":0}}"
    )
}

fn imported(state: &RunState) -> Session {
    let mut session = Session::new(&support::worker_init()).expect("versions agree");
    let mut import = String::from("{\"text\":");
    write_text(&mut import, &state.export_file());
    import.push('}');
    let answer = session.command("import_run", &import);
    assert!(answer.contains("\"ok\":true"), "{answer}");
    session
}

/// A session standing on the circuit, played forward and entered into Still
/// Mode — the play-reachable way to a standing slate and an inspectable View.
fn stilled() -> Session {
    let view = view_of(&INSIDE, WINDOW);
    let mut state = played(circuit(), 0, view.clone());
    state.trace = Trace::opening(state.now.clone());
    let mut session = imported(&state);
    // Eight steps, which is the declared window exactly: the inside is still
    // carrying when the pause reads it, so the default rules of the kinds have
    // something to name.
    session.command("input_frame", &frame(1, 8, false, 1_000_000, "null"));
    session.command("input_frame", &frame(2, 0, true, 1_100_000, "null"));
    session.command("input_frame", &frame(3, 0, false, 1_100_000 + RAMP_US, "null"));
    assert_eq!(session.lifecycle(), "still");
    session
}

/// The reviews one command raised, as (kind, body).
fn reviews(session: &mut Session) -> Vec<(String, Json)> {
    let raised = session.take_events();
    let Json::List(items) = parse(&raised).expect("canonical events") else {
        panic!("the events are a list: {raised}");
    };
    items
        .into_iter()
        .filter(|item| item.get("ev").and_then(Json::as_text) == Some("review_ready"))
        .map(|item| {
            let review = item.get("body").and_then(|body| body.get("review")).cloned();
            let review = review.expect("a review");
            let kind = review.get("kind").and_then(Json::as_text).unwrap_or("").to_string();
            (kind, review)
        })
        .collect()
}

#[test]
fn an_inspect_request_answers_with_the_coordinate_profile() {
    let mut session = stilled();
    reviews(&mut session);
    let answer = session.command(
        "input_frame",
        &frame(
            4,
            0,
            false,
            2_000_000,
            "{\"kind\":null,\"parameter\":null,\"target\":\"coordinates\"}",
        ),
    );
    assert!(!answer.contains("\"ok\":false"), "{answer}");
    let raised = reviews(&mut session);
    assert_eq!(raised.len(), 1);
    assert_eq!(raised[0].0, "coordinates");
    let profile = raised[0].1.get("profile").expect("the profile");
    assert!(profile.get("swap_range").is_some());
    // The two replay-based coordinates stand null until they are asked for.
    assert_eq!(profile.get("instruction_separation"), Some(&Json::Null));
    assert_eq!(profile.get("turnover_tolerance"), Some(&Json::Null));
}

#[test]
fn a_full_request_runs_the_two_replay_based_coordinates() {
    let mut session = stilled();
    reviews(&mut session);
    session.command(
        "input_frame",
        &frame(
            4,
            0,
            false,
            2_000_000,
            "{\"kind\":null,\"parameter\":null,\"target\":\"coordinates_full\"}",
        ),
    );
    let raised = reviews(&mut session);
    let profile = raised[0].1.get("profile").expect("the profile");
    assert_ne!(profile.get("instruction_separation"), Some(&Json::Null));
    assert_ne!(profile.get("turnover_tolerance"), Some(&Json::Null));
}

#[test]
fn an_inspect_request_answers_with_one_perturbation() {
    let mut session = stilled();
    reviews(&mut session);
    session.command(
        "input_frame",
        &frame(
            4,
            0,
            false,
            2_000_000,
            "{\"kind\":\"route-removal\",\"parameter\":null,\"target\":\"perturbation\"}",
        ),
    );
    let raised = reviews(&mut session);
    assert_eq!(raised.len(), 1);
    assert_eq!(raised[0].0, "perturbation");
    let result = raised[0].1.get("result").expect("the result");
    assert_eq!(result.get("kind").and_then(Json::as_text), Some("route-removal"));
    assert!(result.get("parameter").and_then(Json::as_int).is_some(), "resolved, not null");
}

#[test]
fn a_request_outside_still_mode_is_ignored_and_a_malformed_one_is_refused() {
    let view = view_of(&INSIDE, WINDOW);
    let mut state = played(circuit(), 0, view.clone());
    state.trace = Trace::opening(state.now.clone());
    let mut session = imported(&state);
    // A running run answers nothing at all: ordinary play is free of readings
    // it never asked for.
    let answer = session.command(
        "input_frame",
        &frame(
            1,
            4,
            false,
            1_000_000,
            "{\"kind\":null,\"parameter\":null,\"target\":\"coordinates\"}",
        ),
    );
    assert!(!answer.contains("\"ok\":false"), "{answer}");
    assert!(reviews(&mut session).is_empty(), "a moving Field raises no review");

    // A kind outside the closed set is a body the sender got wrong, wherever
    // it arrives.
    let refused = session.command(
        "input_frame",
        &frame(
            2,
            0,
            false,
            1_100_000,
            "{\"kind\":\"no-such-kind\",\"parameter\":null,\"target\":\"perturbation\"}",
        ),
    );
    assert!(refused.contains("\"ok\":false"), "{refused}");
    let missing = session.command(
        "input_frame",
        &frame(3, 0, false, 1_200_000, "{\"kind\":null,\"parameter\":null,\"target\":\"nope\"}"),
    );
    assert!(missing.contains("\"ok\":false"), "{missing}");
}

#[test]
fn a_committed_cut_leaves_a_route_removal_echo_shown_at_the_exit() {
    let mut session = stilled();
    reviews(&mut session);
    let queued = session.command("queue_plan", "{\"plan\":{\"op\":\"cut\",\"route\":2}}");
    assert!(queued.contains("\"ok\":true"), "{queued}");
    let committed = session.command("commit_plan", "{}");
    assert!(committed.contains("\"ok\":true"), "{committed}");
    // The commit's own review is the reassembled slate; the Echo waits for the
    // exit, exactly as the event ordering locks.
    let after_commit = reviews(&mut session);
    assert_eq!(after_commit.len(), 1);
    assert_eq!(after_commit[0].0, "slate");

    assert_eq!(session.lifecycle(), "ramp_out");
    session.command("input_frame", &frame(4, 0, false, 3_000_000, "null"));
    session.command("input_frame", &frame(5, 0, false, 3_000_000 + RAMP_US, "null"));
    assert_eq!(session.lifecycle(), "running");
    let raised = reviews(&mut session);
    let echo = raised
        .iter()
        .find(|(kind, _)| kind == "echo")
        .map(|(_, body)| body.get("echo").expect("the highlight").clone())
        .expect("the exit raises the Echo");
    assert_eq!(echo.get("kind").and_then(Json::as_text), Some("route-removal"));
    assert_eq!(echo.get("parameter").and_then(Json::as_int), Some(2));
    let target = echo.get("target").expect("a target");
    assert_eq!(target.get("t").and_then(Json::as_text), Some("route"));
    assert_eq!(target.get("id").and_then(Json::as_int), Some(2));
}

#[test]
fn a_commit_that_cuts_every_crossing_route_leaves_a_boundary_severance_echo() {
    let mut session = stilled();
    reviews(&mut session);
    // The standing View of this run is the circuit's opening View, whose
    // crossing set the commit takes whole.
    let crossing: Vec<u32> = {
        let run = session.run().expect("a run is loaded");
        let inside = run.state().view.inside.clone();
        run.state()
            .now
            .routes
            .iter()
            .filter(|route| {
                inside.contains(&route.tail) != inside.contains(&route.head)
            })
            .map(|route| route.route)
            .collect()
    };
    assert!(!crossing.is_empty(), "the standing View has a boundary to sever");
    for route in &crossing {
        let queued =
            session.command("queue_plan", &format!("{{\"plan\":{{\"op\":\"cut\",\"route\":{route}}}}}"));
        assert!(queued.contains("\"ok\":true"), "{queued}");
    }
    assert!(session.command("commit_plan", "{}").contains("\"ok\":true"));
    reviews(&mut session);
    session.command("input_frame", &frame(4, 0, false, 3_000_000, "null"));
    session.command("input_frame", &frame(5, 0, false, 3_000_000 + RAMP_US, "null"));
    let raised = reviews(&mut session);
    let echo = raised
        .iter()
        .find(|(kind, _)| kind == "echo")
        .map(|(_, body)| body.get("echo").expect("the highlight").clone())
        .expect("the exit raises the Echo");
    assert_eq!(echo.get("kind").and_then(Json::as_text), Some("boundary-severance"));
    assert_eq!(echo.get("parameter"), Some(&Json::Null), "the kind takes no parameter");
}

#[test]
fn passive_focus_never_queues_or_commits_an_echo() {
    let mut session = stilled();
    reviews(&mut session);
    let (ordinal, count) = {
        let run = session.run().expect("a run is loaded");
        let slate = run.standing_slate().expect("a slate stands");
        (slate.ordinal, slate.candidates.len())
    };
    assert!(count >= 2, "a slate to adopt from");
    let focused = session.command(
        "set_focus",
        &format!("{{\"position\":2,\"slate_ordinal\":{ordinal}}}"),
    );
    assert!(focused.contains("\"ok\":true"), "{focused}");
    assert!(session.run().expect("a run").queue().is_empty(), "focus is outside the plan queue");
    // An empty commit only exits Still Mode. It applies no causal change and
    // therefore has no post-commit Echo to carry.
    assert!(session.command("commit_plan", "{}").contains("\"ok\":true"));
    reviews(&mut session);
    session.command("input_frame", &frame(4, 0, false, 3_000_000, "null"));
    session.command("input_frame", &frame(5, 0, false, 3_000_000 + RAMP_US, "null"));
    let raised = reviews(&mut session);
    assert!(
        raised.iter().all(|(kind, _)| kind != "echo"),
        "passive observation is not presented as a physical intervention",
    );
}

#[test]
fn a_committed_compartment_reshape_never_borrows_a_view_candidates_record() {
    let mut session = stilled();
    reviews(&mut session);
    // The finer candidate is an inside-axis variant: a different inside under
    // the standing resolution, window, and surround — exactly the components a
    // reshape keeps — so reshaping to its member set installs its View and the
    // pre-commit record that evaluated it is the one the highlight reads.
    let (ordinal, members) = {
        let run = session.run().expect("a run is loaded");
        let slate = run.standing_slate().expect("a slate stands");
        let standing = &slate.candidates[0].view.inside;
        let other = slate
            .candidates
            .iter()
            .find(|held| held.view.inside != *standing)
            .expect("an inside-axis variant to reshape to");
        (slate.ordinal, other.view.inside.clone())
    };
    let list: Vec<String> = members.iter().map(|node| node.to_string()).collect();
    let queued = session.command(
        "queue_plan",
        &format!(
            "{{\"plan\":{{\"members\":[{}],\"op\":\"reshape_compartment\"}}}}",
            list.join(",")
        ),
    );
    assert!(queued.contains("\"ok\":true"), "{queued}");
    assert!(session.command("commit_plan", "{}").contains("\"ok\":true"));
    reviews(&mut session);
    session.command("input_frame", &frame(4, 0, false, 3_000_000, "null"));
    session.command("input_frame", &frame(5, 0, false, 3_000_000 + RAMP_US, "null"));
    let raised = reviews(&mut session);
    assert!(
        raised.iter().all(|(kind, _)| kind != "echo"),
        "a View evaluation is not evidence for a physical compartment edit (slate {ordinal})",
    );
}

#[test]
fn a_committed_reshape_no_record_evaluated_leaves_no_highlight() {
    let mut session = stilled();
    reviews(&mut session);
    // A member set no candidate of the pre-commit slate declares: no record
    // evaluated it, so there is no highlight — honest, rather than another
    // record's reading under its name.
    let members = {
        let run = session.run().expect("a run is loaded");
        let slate = run.standing_slate().expect("a slate stands");
        let taken: Vec<Vec<u32>> =
            slate.candidates.iter().map(|held| held.view.inside.clone()).collect();
        [vec![2u32], vec![4u32], vec![2u32, 4u32], vec![3u32, 4u32]]
            .into_iter()
            .find(|set| !taken.contains(set))
            .expect("a set outside a slate of at most five")
    };
    let list: Vec<String> = members.iter().map(|node| node.to_string()).collect();
    let queued = session.command(
        "queue_plan",
        &format!(
            "{{\"plan\":{{\"members\":[{}],\"op\":\"reshape_compartment\"}}}}",
            list.join(",")
        ),
    );
    assert!(queued.contains("\"ok\":true"), "{queued}");
    assert!(session.command("commit_plan", "{}").contains("\"ok\":true"));
    reviews(&mut session);
    session.command("input_frame", &frame(4, 0, false, 3_000_000, "null"));
    session.command("input_frame", &frame(5, 0, false, 3_000_000 + RAMP_US, "null"));
    let raised = reviews(&mut session);
    assert!(
        raised.iter().all(|(kind, _)| kind != "echo"),
        "no candidate evaluated the reshaped View, so nothing is highlighted",
    );
}

#[test]
fn a_committed_connection_reads_the_standing_candidates_record() {
    // `connect` and `redirect` read the controlled Form's `route_reach`, so
    // this branch needs the fixture that stands one — far from the circuit, so
    // every recorded series is unchanged.
    let view = view_of(&INSIDE, WINDOW);
    let mut state = played(circuit_with_form(0), 0, view.clone());
    state.trace = Trace::opening(state.now.clone());
    let mut session = imported(&state);
    session.command("input_frame", &frame(1, 8, false, 1_000_000, "null"));
    session.command("input_frame", &frame(2, 0, true, 1_100_000, "null"));
    session.command("input_frame", &frame(3, 0, false, 1_100_000 + RAMP_US, "null"));
    assert_eq!(session.lifecycle(), "still");
    reviews(&mut session);
    let ordinal = {
        let run = session.run().expect("a run is loaded");
        run.standing_slate().expect("a slate stands").ordinal
    };
    // A connection the Field admits: 5 to 3, 600 units apart, no standing
    // Route between them, inside the Form's 4000-unit reach.
    let queued =
        session.command("queue_plan", "{\"plan\":{\"from\":5,\"op\":\"connect\",\"to\":3}}");
    assert!(queued.contains("\"ok\":true"), "{queued}");
    assert!(session.command("commit_plan", "{}").contains("\"ok\":true"));
    reviews(&mut session);
    session.command("input_frame", &frame(4, 0, false, 3_000_000, "null"));
    session.command("input_frame", &frame(5, 0, false, 3_000_000 + RAMP_US, "null"));
    let raised = reviews(&mut session);
    // The framework's Echo list does not bind these two ops; the locked path
    // is the evaluation record of the standing candidate — seat 1 — because
    // the standing View is what the commit leaves adopted.
    let echo = raised
        .iter()
        .find(|(kind, _)| kind == "echo")
        .map(|(_, body)| body.get("echo").expect("the highlight").clone())
        .expect("a committed connection leaves the standing candidate's Echo");
    assert_eq!(echo.get("kind").and_then(Json::as_text), Some("evaluation"));
    assert_eq!(echo.get("parameter").and_then(Json::as_int), Some(i64::from(ordinal)));
}

#[test]
fn a_cut_whose_reading_is_unassigned_leaves_no_highlight_and_no_other_arms() {
    // A session entered into Still Mode with nothing recorded: the effective
    // window is 0, the cut's perturbation reading is unassigned, and the Echo
    // branch is one match — so the exit raises nothing at all, and never the
    // evaluation arm's highlight under the cut's name.
    let view = view_of(&INSIDE, WINDOW);
    let mut state = played(circuit(), 0, view.clone());
    state.trace = Trace::opening(state.now.clone());
    let mut session = imported(&state);
    session.command("input_frame", &frame(1, 0, true, 1_000_000, "null"));
    session.command("input_frame", &frame(2, 0, false, 1_000_000 + RAMP_US, "null"));
    assert_eq!(session.lifecycle(), "still");
    reviews(&mut session);
    let queued = session.command("queue_plan", "{\"plan\":{\"op\":\"cut\",\"route\":2}}");
    assert!(queued.contains("\"ok\":true"), "{queued}");
    assert!(session.command("commit_plan", "{}").contains("\"ok\":true"));
    reviews(&mut session);
    session.command("input_frame", &frame(3, 0, false, 3_000_000, "null"));
    session.command("input_frame", &frame(4, 0, false, 3_000_000 + RAMP_US, "null"));
    assert_eq!(session.lifecycle(), "running");
    let raised = reviews(&mut session);
    assert!(
        raised.iter().all(|(kind, _)| kind != "echo"),
        "an unassigned reading leaves no highlight, and no other arm's either",
    );
}

#[test]
fn a_perturbation_result_never_enters_the_payload() {
    let mut session = stilled();
    session.command(
        "input_frame",
        &frame(
            4,
            0,
            false,
            2_000_000,
            "{\"kind\":\"route-removal\",\"parameter\":null,\"target\":\"perturbation\"}",
        ),
    );
    session.command(
        "input_frame",
        &frame(
            5,
            0,
            false,
            2_100_000,
            "{\"kind\":null,\"parameter\":null,\"target\":\"coordinates_full\"}",
        ),
    );
    let answer = session.command("export_run", "{}");
    // Results are session-lived: what stands after a restore is `sigma_V` and
    // the resolved parameters, and the reading is taken again from those.
    for name in [
        "route-removal",
        "swap_range",
        "turnover_tolerance",
        "instruction_separation",
        "base_series",
    ] {
        assert!(!answer.contains(name), "a payload carries no {name}");
    }
}
