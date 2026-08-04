//! The ten-coordinate profile.
//!
//! `docs/field-framework/FRAMEWORK.md`'s Coordinate profile section is what is
//! under test. The Field these run on is the circuit fixture, built so the step
//! function's own arithmetic is short enough to carry out by hand: Route flow
//! and nothing else moves Charge, so the recorded stored-Charge total of the
//! inside `{2, 3, 4}` over eight steps is, in whole units,
//!
//! ```text
//!   4, 8, 12, 16, 20, 16, 12, 8
//! ```
//!
//! Node 1 supplies eight units a step for five steps and then stands empty;
//! Node 5 takes four a step throughout. Every expected number below is written
//! out as the arithmetic FRAMEWORK.md's formula produces on that series.

use field_game_core::coord;
use field_game_core::fx::ONE_UNIT;
use field_game_core::json::{parse, Json};
use field_game_core::slate::TAU_DEFAULT;
use field_game_core::state::{Surround, FRAC_ONE};

mod support;

use support::measure::{
    circuit, circuit_with_form, circuit_with_upkeep, played, recorded_units, sigma, view_of,
};

/// The window every reading below is taken over.
const WINDOW: u16 = 8;

/// The inside every reading below is taken of.
const INSIDE: [u32; 3] = [2, 3, 4];

fn profile() -> coord::CoordinateProfile {
    let view = view_of(&INSIDE, WINDOW);
    let state = played(circuit(), usize::from(WINDOW), view.clone());
    coord::of(&state, &view, TAU_DEFAULT)
}

fn whole_profile() -> coord::CoordinateProfile {
    let view = view_of(&INSIDE, WINDOW);
    let state = played(circuit(), usize::from(WINDOW), view.clone());
    coord::full(&state, &view, &sigma(), 0, TAU_DEFAULT)
}

#[test]
fn the_fixture_records_the_series_every_reading_is_taken_off() {
    let view = view_of(&INSIDE, WINDOW);
    let state = played(circuit(), usize::from(WINDOW), view.clone());
    assert_eq!(
        recorded_units(&state, &INSIDE),
        vec![4, 8, 12, 16, 20, 16, 12, 8],
        "the hand trace and the step function agree, or every number below is read off nothing",
    );
    assert_eq!(state.effective_window(WINDOW), WINDOW, "the clamp leaves the whole window");
}

// ---------------------------------------------------------------------------
// The graph-shape family: Swap Range and Reach
// ---------------------------------------------------------------------------

#[test]
fn swap_range_counts_the_members_that_are_not_articulation_members() {
    // The internal graph of `{2, 3, 4}`: Routes 2 (2→3), 3 (3→4), and 5 (4→2)
    // all carry over the window, and no member is adjacent to another, so the
    // graph is one triangle. A triangle has no articulation vertex, so all
    // three members are spare.
    assert_eq!(profile().swap_range.value, Some(3));
}

#[test]
fn swap_range_is_zero_for_an_inside_of_one_and_reads_per_component() {
    let view = view_of(&[3], WINDOW);
    let state = played(circuit(), usize::from(WINDOW), view_of(&INSIDE, WINDOW));
    // FRAMEWORK.md, verbatim: Swap Range is 0 when `|I| = 1`.
    assert_eq!(coord::of(&state, &view, TAU_DEFAULT).swap_range.value, Some(0));

    // Two members with no edge between them stand in two components of one
    // member each. Neither is an articulation member — a member alone in its
    // component never is — so both are spare, which is the reading the
    // per-component rule gives and the whole-graph rule would not.
    let split = view_of(&[2, 5], WINDOW);
    let held = coord::of(&state, &split, TAU_DEFAULT);
    assert_eq!(held.swap_range.value, Some(2));
}

#[test]
fn a_member_whose_removal_splits_its_component_is_not_spare() {
    // Nodes 1, 2, and 3 stand in a line: Route 1 joins 1 to 2 and Route 2 joins
    // 2 to 3, and no other internal Route or adjacency stands between them. So
    // Node 2 is the articulation member of that component and the other two are
    // spare.
    let view = view_of(&[1, 2, 3], WINDOW);
    let state = played(circuit(), usize::from(WINDOW), view_of(&INSIDE, WINDOW));
    assert_eq!(coord::of(&state, &view, TAU_DEFAULT).swap_range.value, Some(2));
}

#[test]
fn reach_is_the_widest_distance_inside_a_component_that_holds_a_cycle() {
    // The flow-positive graph's one cyclic component is `{2, 3, 4}` — Route 2,
    // Route 3, and Route 5 close the loop. Nodes 2 and 4 stand 600 units apart
    // on one layer, and the locked distance of that pair is 600 units.
    assert_eq!(profile().reach.value, Some(600 * ONE_UNIT));
}

#[test]
fn reach_is_zero_when_no_member_lies_in_a_component_with_a_cycle() {
    // Node 5 takes Charge and sends none: it stands alone in its own strongly
    // connected component, which holds no cycle.
    let view = view_of(&[5], WINDOW);
    let state = played(circuit(), usize::from(WINDOW), view_of(&INSIDE, WINDOW));
    assert_eq!(coord::of(&state, &view, TAU_DEFAULT).reach.value, Some(0));
}

// ---------------------------------------------------------------------------
// The aggregate family: Self-Support, Throughput, Upkeep Mix, Source Trace
// ---------------------------------------------------------------------------

#[test]
fn self_support_is_one_when_no_upkeep_was_required() {
    // FRAMEWORK.md, verbatim: when `UP_total = 0`, Self-Support is 1 — nothing
    // was required, so nothing was supplied from outside. The Node phase that
    // charges upkeep is still reserved, so this is the whole game's reading
    // today and it is honest rather than degenerate.
    assert_eq!(profile().self_support.value, Some(i64::from(FRAC_ONE)));
}

#[test]
fn throughput_reports_the_two_magnitudes_and_the_identity_beside_them() {
    let held = profile().throughput;
    // Route 1 carried eight units a step for five steps: 40 over a window of
    // eight is five a step. Route 4 carried four a step throughout: 32 over
    // eight is four a step.
    assert_eq!(held.in_rate, 5 * ONE_UNIT);
    assert_eq!(held.out_rate, 4 * ONE_UNIT);
    // The identity: every crossing Route with its own mean, sorted descending
    // by magnitude.
    assert_eq!(held.routes, vec![(1, 5 * ONE_UNIT), (4, 4 * ONE_UNIT)]);
    // The shell is Nodes 2 and 4 — the members with a crossing Route — and
    // neither took any exogenous term at all, so both read zero and the tie
    // goes to the smaller identifier.
    assert_eq!(held.shell, vec![(2, 0), (4, 0)]);
}

#[test]
fn upkeep_mix_is_unassigned_while_no_upkeep_falls_due() {
    let held = profile();
    assert_eq!(held.upkeep_mix, None);
    assert_eq!(held.upkeep_reason, Some(coord::NO_UPKEEP));
}

#[test]
fn upkeep_readings_over_a_synthetic_schedule_check_the_two_formulas_by_hand() {
    // The Node phase that charges upkeep is still reserved, so the step
    // function writes no upkeep record — and the two formulas that read one
    // would otherwise be exercised only by their degenerate cases. The window
    // here is written by hand instead: FRAMEWORK.md's aggregates read the
    // recorded window, not the rule that recorded it, so a synthetic trace is
    // a legitimate window for the recorded-window coordinates.
    //
    // Four steps over the circuit's shape, inside {2, 3, 4}:
    //
    //   f(r1) = 1 unit at steps 1 and 2   (crossing, into the inside)
    //   f(r4) = 1 unit at step 1          (crossing, out of the inside)
    //   upkeep(2) = 1 unit at steps 1..3, attributed to purposes 1, 2, 3
    //   q(2) = 4 units throughout
    //
    // Self-Support: UP_total = 3; NetImport = max(0, 2 - 1) = 1;
    // (3 - 1) / 3 = 2/3 -> 43691 raw, rounded to nearest.
    //
    // Upkeep Mix: shares 1/3 each for the first three purposes. Each floors to
    // 21845 raw with an equal remainder, and 65536 - 3 * 21845 = 1 unit is
    // left; largest-remainder rounding gives it to the earliest of the tied
    // purposes, so the shares are [21846, 21845, 21845, 0, 0] and sum to one.
    //
    // Throughput: in 2 units over 4 steps = 32768 raw per step; out 1 over 4 =
    // 16384; itemized [(r1, 32768), (r4, 16384)], descending.
    let keyframe = circuit();
    let mut now = keyframe.clone();
    now.step = 4;
    let mut trace = field_game_core::state::Trace::opening(keyframe);
    for step in 1..=4u32 {
        let mut records = field_game_core::field::StepRecords {
            q: vec![(2, 4 * ONE_UNIT)],
            f: Vec::new(),
            upkeep: Vec::new(),
            e: Vec::new(),
            z: Vec::new(),
        };
        if step <= 2 {
            records.f.push((1, ONE_UNIT));
        }
        if step == 1 {
            records.f.push((4, ONE_UNIT));
        }
        if step <= 3 {
            let mut mix = [0 as i64; 5];
            mix[step as usize - 1] = ONE_UNIT;
            records.upkeep.push(field_game_core::field::UpkeepRecord {
                node: 2,
                v: ONE_UNIT,
                mix,
            });
        }
        trace.steps.push_back(field_game_core::state::TraceStep {
            step,
            rng: field_game_core::rng::RngState { key: 0, ctr: 0, half: 0 },
            ctl: field_game_core::state::ControlState::default(),
            records,
        });
    }
    let view = view_of(&INSIDE, 4);
    let state = field_game_core::state::RunState {
        run_id: support::measure::KEY.to_string(),
        rng: field_game_core::rng::trajectory_stream(support::measure::KEY, 0),
        scenario: field_game_core::state::ScenarioSpec::legacy(
            support::measure::NO_CONTENT.to_string(),
        ),
        criterion: None,
        branch_nonce: 0,
        progress: field_game_core::state::Progress::opening(),
        now,
        trace,
        view: view.clone(),
        slate: None,
        input_config: field_game_core::state::InputConfig::default_config(),
        pressures: Vec::new(),
        anchors: Vec::new(),
    };
    assert_eq!(state.effective_window(4), 4);
    let held = coord::of(&state, &view, TAU_DEFAULT);
    let two_thirds = ((2i128 * i128::from(FRAC_ONE) * 2 + 3) / 6) as i64;
    assert_eq!(held.self_support.value, Some(two_thirds));
    assert_eq!(held.upkeep_mix, Some([21846, 21845, 21845, 0, 0]));
    let shares = held.upkeep_mix.expect("assigned");
    assert_eq!(shares.iter().sum::<i64>(), i64::from(FRAC_ONE), "the five sum to exactly one");
    assert_eq!(held.upkeep_reason, None);
    assert_eq!(held.throughput.in_rate, ONE_UNIT / 2);
    assert_eq!(held.throughput.out_rate, ONE_UNIT / 4);
    assert_eq!(
        held.throughput.routes,
        vec![(1, ONE_UNIT / 2), (4, ONE_UNIT / 4)],
        "the itemized identity, descending by magnitude",
    );
}

#[test]
fn source_trace_is_zero_for_an_inside_that_started_empty() {
    // The window opens at step 0, where every member holds nothing, so the
    // retained share of what was inside at the window start is nothing: the
    // inside runs entirely on recurring supply from the surround.
    assert_eq!(profile().source_trace.value, Some(0));
}

#[test]
fn source_trace_is_one_for_an_inside_nothing_is_removed_from() {
    // Node 6 holds nothing and carries nothing, so it is not the example. Node
    // 1 opens the window holding 40 units and is drained by its own Route, so
    // its share falls. Read over a window of one step, the removal is one
    // step's outflow of eight units against an opening of 40: the retained
    // share is 32/40 of the 32 units it ends the step with.
    let view = view_of(&[1], 1);
    let state = played(circuit(), 1, view_of(&INSIDE, WINDOW));
    let held = coord::of(&state, &view, TAU_DEFAULT);
    // old(1) = 40 * (40 - 8) / 40 = 32, and q_I(t0) = 32, so the reading is 1.
    assert_eq!(held.source_trace.value, Some(i64::from(FRAC_ONE)));
}

#[test]
fn source_trace_is_unassigned_when_the_inside_ends_the_window_holding_nothing() {
    // Node 3 is emptied by Route 3 in the same step it is filled by Route 2, so
    // it ends every step holding nothing and there is no whole to take a share
    // of.
    let view = view_of(&[3], WINDOW);
    let state = played(circuit(), usize::from(WINDOW), view_of(&INSIDE, WINDOW));
    let held = coord::of(&state, &view, TAU_DEFAULT);
    assert_eq!(held.source_trace.value, None);
    assert_eq!(held.source_trace.reason, Some(coord::NO_STORED_CHARGE));
}

// ---------------------------------------------------------------------------
// The series family: Input Resolution and Horizon
// ---------------------------------------------------------------------------

#[test]
fn input_resolution_counts_the_distinct_quantized_surround_signatures() {
    // The signature holds four positions: Route 1 and Route 4 — the crossing
    // set, ascending — then Nodes 2 and 4, the shell, ascending. Route 4 never
    // moves off four units a step and neither shell member takes an exogenous
    // term, so three positions stand level and quantize to 0 throughout. Route
    // 1 carried eight units for five steps and nothing for three, which is two
    // levels. So the window held exactly two distinguishable states.
    assert_eq!(profile().input_resolution.value, Some(2));
}

#[test]
fn input_resolution_is_unassigned_when_the_signature_has_no_position() {
    // Node 6 has a Route to Node 1 and none to any member, so an inside of the
    // whole circuit but for it has no crossing Route and an empty shell — no
    // position at all to quantize.
    let view = view_of(&[1, 2, 3, 4, 5, 6], WINDOW);
    let state = played(circuit(), usize::from(WINDOW), view_of(&INSIDE, WINDOW));
    let held = coord::of(&state, &view, TAU_DEFAULT);
    assert_eq!(held.input_resolution.value, None);
    assert_eq!(held.input_resolution.reason, Some(coord::NO_SIGNATURE));
}

#[test]
fn horizon_is_the_largest_lag_whose_agreement_reaches_the_threshold() {
    // The series' range is 20 - 4 = 16 units, so the level margin is two units
    // and every four-unit step reads as a direction: the symbols are
    // `+1 +1 +1 +1 -1 -1 -1`. The threshold is `1/2 + tau` = 0.625.
    //
    //   lag 1: five of six pairs agree — 0.833 — which qualifies;
    //   lag 2: three of five — 0.6 — which does not;
    //   lag 3 and above: fewer still.
    //
    // So the retained span is 1, the declared Forecast depth is 0 because the
    // fixture stands no Form, and Horizon is the larger of the two.
    assert_eq!(profile().horizon.value, Some(1));
}

#[test]
fn horizon_is_the_larger_of_the_retained_span_and_the_declared_forecast_depth() {
    // The Field's declared Forecast depth `a_F` is the controlled Form's
    // `forecast_depth` — the rule `route_reach` already stands under — and
    // Horizon is `max(retained span, a_F)`. The Form stands far from the
    // circuit and is steered by nothing, so the recorded series and its
    // retained span of 1 are exactly the plain fixture's; the declared depth
    // of 6 is what the reading takes.
    let view = view_of(&INSIDE, WINDOW);
    let state = played(circuit_with_form(6), usize::from(WINDOW), view.clone());
    assert_eq!(recorded_units(&state, &INSIDE), vec![4, 8, 12, 16, 20, 16, 12, 8]);
    assert_eq!(coord::of(&state, &view, TAU_DEFAULT).horizon.value, Some(6));

    // And a declared depth below the retained span changes nothing: the
    // retained 1 is the larger.
    let shallow = played(circuit_with_form(0), usize::from(WINDOW), view.clone());
    assert_eq!(coord::of(&shallow, &view, TAU_DEFAULT).horizon.value, Some(1));
}

#[test]
fn horizon_is_assigned_whenever_the_effective_window_holds_a_step() {
    // FRAMEWORK.md's minimum-data table, verbatim: Horizon is assigned whenever
    // `w_eff >= 1`, and the retained span is 0 — not unassigned — when `w < 3`.
    let view = view_of(&INSIDE, 1);
    let state = played(circuit(), 1, view_of(&INSIDE, WINDOW));
    assert_eq!(coord::of(&state, &view, TAU_DEFAULT).horizon.value, Some(0));
}

// ---------------------------------------------------------------------------
// The replay family: Instruction Separation and Turnover Tolerance
// ---------------------------------------------------------------------------

#[test]
fn the_two_replay_based_coordinates_are_null_until_they_are_asked_for() {
    let held = profile();
    assert!(held.instruction_separation.is_none());
    assert!(held.turnover_tolerance.is_none());
    assert!(held.written().contains("\"instruction_separation\":null"));
    assert!(held.written().contains("\"turnover_tolerance\":null"));

    let whole = whole_profile();
    assert!(whole.instruction_separation.is_some());
    assert!(whole.turnover_tolerance.is_some());
}

#[test]
fn instruction_separation_reads_the_agreement_of_the_rebuilt_inside() {
    // The descriptor rebuilds `{2, 3, 4}` with Routes 2, 3, and 5 among them,
    // and drives Route 1 and Route 4 from the recorded schedule. Route 1's
    // eight units a step arrive as recorded; Route 4's four a step are asked of
    // a Node that holds nothing when the schedule asks, and a Node never holds
    // less than nothing, so nothing leaves. The rebuilt total therefore climbs
    // 8, 16, 24, 32, 40 and then stands, against a recorded series that climbs
    // and falls: the symbols agree on four of seven steps.
    let held = whole_profile().instruction_separation.expect("the replays ran");
    let four_sevenths = ((4i128 * i128::from(FRAC_ONE) * 2 + 7) / 14) as i64;
    assert_eq!(held.value, Some(four_sevenths));
    assert_eq!(held.low, Some(four_sevenths));
    assert_eq!(held.high, Some(four_sevenths));
    assert_eq!(held.samples, 8);
}

#[test]
fn turnover_tolerance_records_the_whole_ladder_and_the_largest_qualifying_step() {
    // Every member holds nothing at the window start, and Node 4's kind holds
    // 64 units at creation — so a replacement changes the level the inside
    // stands at without changing the direction it moves in. The replay
    // deviation reads directions, so every replacement fraction agrees fully,
    // and the largest qualifying fraction is the whole of the inside.
    let held = whole_profile().turnover_tolerance.expect("the replays ran");
    assert_eq!(held.value, Some(FRAC_ONE));
    let pairs = held.pairs.expect("the ladder is recorded");
    assert_eq!(pairs.len(), 8, "one (phi, agreement) pair per replacement fraction");
    for (index, (phi, agreed)) in pairs.iter().enumerate() {
        assert_eq!(*phi, FRAC_ONE * (index as i64 + 1) / 8, "the ladder is j / 8");
        assert_eq!(*agreed, FRAC_ONE, "every replayed direction series matches");
    }
}

#[test]
fn the_replay_based_coordinates_are_unassigned_on_a_window_too_short_to_compare() {
    let view = view_of(&INSIDE, 1);
    let state = played(circuit(), 1, view_of(&INSIDE, WINDOW));
    let held = coord::full(&state, &view, &sigma(), 0, TAU_DEFAULT);
    let separation = held.instruction_separation.expect("the request ran");
    assert_eq!(separation.value, None);
    assert_eq!(separation.reason, Some("window-too-short"));
    let turnover = held.turnover_tolerance.expect("the request ran");
    assert_eq!(turnover.value, None);
    assert_eq!(turnover.pairs, None);
}

// ---------------------------------------------------------------------------
// The record
// ---------------------------------------------------------------------------

#[test]
fn the_record_carries_every_locked_key_and_no_combined_figure() {
    let written = whole_profile().written();
    let parsed = parse(&written).expect("the profile is canonical JSON");
    let Json::Map(entries) = &parsed else { panic!("the profile is an object") };
    let keys: Vec<&str> = entries.iter().map(|(key, _)| key.as_str()).collect();
    assert_eq!(
        keys,
        vec![
            "horizon",
            "input_resolution",
            "instruction_separation",
            "reach",
            "self_support",
            "source_trace",
            "step",
            "swap_range",
            "throughput",
            "turnover_tolerance",
            "upkeep_mix",
            "view",
        ],
        "the ten coordinates, the step, and the View — and nothing that folds them together",
    );
    // The whole point of the type, asserted structurally: the record's key set
    // is exactly the ten readings beside the two facts that name what was read,
    // so a figure derived from several of them has nowhere in the record to
    // stand. The equality above is the assertion; this is what it means.
    assert_eq!(keys.len(), 12);
}

#[test]
fn identical_inputs_produce_an_identical_record() {
    let view = view_of(&INSIDE, WINDOW);
    let first = played(circuit(), usize::from(WINDOW), view.clone());
    let second = played(circuit(), usize::from(WINDOW), view.clone());
    assert_eq!(
        coord::full(&first, &view, &sigma(), 0, TAU_DEFAULT).written(),
        coord::full(&second, &view, &sigma(), 0, TAU_DEFAULT).written(),
        "the same Field, View, sigma_V, and tolerance read the same profile, byte for byte",
    );
}

#[test]
fn a_window_the_clamp_leaves_empty_reads_the_shape_and_nothing_windowed() {
    // Right after an applying commit the retained span is 0, so every windowed
    // reading is honestly unassigned — and the two that read the Field's own
    // shape rather than its window still stand.
    let view = view_of(&INSIDE, WINDOW);
    let mut state = played(circuit(), usize::from(WINDOW), view.clone());
    state.trace.start_step = state.now.step;
    state.trace.steps.clear();
    assert_eq!(state.effective_window(WINDOW), 0);
    let held = coord::of(&state, &view, TAU_DEFAULT);
    assert_eq!(held.horizon.value, None);
    assert_eq!(held.source_trace.value, None);
    assert_eq!(held.input_resolution.value, None);
    assert_eq!(held.throughput.in_rate, 0);
    // No Route carried over an empty window and no member is adjacent to
    // another, so the internal graph has no edge at all: three components of
    // one member each, none of them an articulation member, and all three
    // spare. That is the per-component rule read literally — the whole-graph
    // reading would have called a disconnected inside a spare-less one.
    assert_eq!(held.swap_range.value, Some(3));
    assert_eq!(held.reach.value, Some(0), "and no component holds a cycle");
}

#[test]
fn a_surround_rule_changes_the_shell_and_the_readings_that_read_it() {
    // The `whole` rule names every non-member, which changes the surround but
    // not the shell — the shell is a property of the members, not of the rule —
    // so Throughput's itemized identity stands where it stood.
    let view = view_of(&INSIDE, WINDOW);
    let state = played(circuit(), usize::from(WINDOW), view.clone());
    let wide = field_game_core::state::ViewDeclaration {
        surround: Surround::Whole,
        ..view.clone()
    };
    assert_eq!(
        coord::of(&state, &view, TAU_DEFAULT).throughput.shell,
        coord::of(&state, &wide, TAU_DEFAULT).throughput.shell,
    );
}

// ---------------------------------------------------------------------------
// Self-Support and Upkeep Mix, on a Field that pays
// ---------------------------------------------------------------------------

#[test]
fn self_support_reads_what_the_inside_paid_against_what_it_took_in() {
    // The two coordinates FRAMEWORK.md defines over upkeep were unassigned
    // while nothing was priced. The Node phase prices them now, so this is the
    // first Field that reads them: the plain circuit with one unit a step
    // authored on each of the three members.
    //
    // The formula is FRAMEWORK.md's own, and every term below is read off the
    // recorded steps rather than restated: Self-Support is
    // `clamp01((UP_total - NetImport) / UP_total)` with
    // `NetImport = max(0, sum of (in - out))` over the window.
    let view = view_of(&INSIDE, WINDOW);
    let state = played(circuit_with_upkeep(8, 200), usize::from(WINDOW), view.clone());

    let paid: i64 = state
        .trace
        .steps
        .iter()
        .flat_map(|step| step.records.upkeep.iter())
        .filter(|entry| INSIDE.contains(&entry.node))
        .map(|entry| entry.v)
        .sum();
    // Three members, eight steps, eight units each, and a stock large enough
    // that every one of them paid in full: 3 x 8 x 8 = 192 units.
    assert_eq!(paid, 192 * ONE_UNIT, "the window's whole upkeep");

    // The crossing Routes are 1 → 2 in and 4 → 5 out: what the window imported
    // net is the first less the second, and it is what the reading discounts.
    let carried = |route: u32| -> i64 {
        state
            .trace
            .steps
            .iter()
            .flat_map(|step| step.records.f.iter())
            .filter(|(held, _)| *held == route)
            .map(|(_, flow)| *flow)
            .sum()
    };
    let net_import = carried(1) - carried(4);
    // Eight units a step in over the window and four a step out: 64 - 32 = 32.
    assert_eq!(carried(1), 64 * ONE_UNIT, "the inlet carried at its capacity throughout");
    assert_eq!(carried(4), 32 * ONE_UNIT, "and the outlet at its own");
    assert_eq!(net_import, 32 * ONE_UNIT);

    // So the reading is `(192 - 32) / 192` = 5/6, which in Q0.16 floors to
    // 54613: five sixths of what the inside paid, it paid out of itself.
    let expected = ((paid - net_import).max(0) * i64::from(FRAC_ONE)) / paid;
    assert_eq!(expected, 54_613);
    let held = coord::of(&state, &view, TAU_DEFAULT);
    assert_eq!(held.self_support.value, Some(expected), "the formula on the recorded terms");
    assert!(held.self_support.value < Some(i64::from(FRAC_ONE)), "and it is a real reading now");
    assert_eq!(held.self_support.reason, None);
}

#[test]
fn upkeep_mix_reads_the_one_purpose_version_1_attributes_it_to() {
    // Every payment is attributed whole to the boundary in version 1, so the
    // five shares are one and four zeroes — and they sum to exactly one, which
    // is the largest-remainder rule's own guarantee holding trivially.
    let view = view_of(&INSIDE, WINDOW);
    let state = played(circuit_with_upkeep(8, 200), usize::from(WINDOW), view.clone());
    let held = coord::of(&state, &view, TAU_DEFAULT);

    assert_eq!(held.upkeep_mix, Some([FRAC_ONE, 0, 0, 0, 0]));
    assert_eq!(held.upkeep_reason, None, "a Field that pays reports no reason");
    let shares: i64 = held.upkeep_mix.expect("a reading").iter().map(|share| i64::from(*share)).sum();
    assert_eq!(shares, i64::from(FRAC_ONE), "the five shares are exactly one");

    // A Field that prices nothing keeps the reading FRAMEWORK.md gives it: no
    // upkeep was required, so none was attributed, and Self-Support is 1.
    let quiet = played(circuit(), usize::from(WINDOW), view.clone());
    let unpriced = coord::of(&quiet, &view, TAU_DEFAULT);
    assert_eq!(unpriced.upkeep_mix, None);
    assert_eq!(unpriced.self_support.value, Some(i64::from(FRAC_ONE)));
}
