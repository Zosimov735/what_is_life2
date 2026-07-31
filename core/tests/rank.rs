//! The four privilege values, the comparison over them, and the ranking.
//!
//! `docs/field-framework/FRAMEWORK.md`'s Privilege profile section is what is
//! under test. The Field these run on is one Field with several regions, each
//! built so that the step function's own arithmetic is short enough to carry
//! out by hand — Route flow and a layer's Drain, and nothing else moving
//! Charge. Every expected number below is written out as the arithmetic that
//! produces it, so a reader can check the framework's formula against the
//! fixture without running anything.
//!
//! The recorded window is played rather than written: the trace is what
//! [`field::advance`] produced from the window's start state, so a replay with
//! no edit reproduces it exactly and a deviation is a reading of the edit.

use field_game_core::field::{
    self, BoundaryState, FieldLayer, FormState, NodeKind, PortState, RouteState,
};
use field_game_core::fx::{Vec2, ONE_UNIT};
use field_game_core::rank;
use field_game_core::rng::RngState;
use field_game_core::slate::{
    self, Candidate, CandidateSlate, PrivilegeValue, Provenance, Source, BASELINE_SAMPLES,
    TAU_DEFAULT,
};
use field_game_core::state::{
    ControlState, FieldState, Frac, InputConfig, Progress, RunState, Surround, Trace,
    TraceStep, ViewDeclaration, FRAC_ONE,
};

const KEY: &str = "0123456789abcdef";
const NO_CONTENT: &str = "00000000000000000000000000000000000000000000000000000000000000ff";

/// The window every fixture is read over.
const WINDOW: usize = 8;

/// Every Route of the fixture carries at most this, per step.
const LINK: i64 = 8 * ONE_UNIT;

/// Layer 0's Drain, which is exactly one Route's capacity: a Node that takes
/// one Route's worth and sends it on ends the step holding nothing.
const DRAIN: i64 = 8 * ONE_UNIT;

// ---------------------------------------------------------------------------
// The Field the fixtures stand on
// ---------------------------------------------------------------------------

fn port(node: u32, layer: u8, x: i64, y: i64, q: i64) -> PortState {
    PortState {
        node,
        layer,
        pos: Vec2::units(x, y),
        kind: NodeKind::Port,
        q: q * ONE_UNIT,
        open: true,
        upkeep_rate: 0,
        // The overload threshold sits at the stored-Charge cap, so nothing in
        // the fixture ever overloads and no Node sheds a quarter of an excess.
        capacity: 4096 * ONE_UNIT,
    }
}

fn route(id: u32, tail: u32, head: u32, capacity: i64) -> RouteState {
    RouteState { route: id, tail, head, capacity, flow: 0, formed_step: 0 }
}

/// The fixture Field.
///
/// Two layers. Layer 0 drains one Route's capacity per step; layer 1 drains
/// nothing. Every Node stands 300 units from the next, which is past the
/// 256-unit adjacency radius, so the declared adjacency is empty and every
/// neighbourhood rule reads Routes alone.
///
/// ```text
/// layer 0, drain 8:
///   1  Form, far away, 1000 units, no Route
///   region A: 4 -> 2 -> 3 -> 4 is the circuit, 5 -> 2 never carries
///     2 (0)   3 (0)   4 (200)   5 (0)
///   region B: 7 -> 6 -> 7 is the circuit, 8 -> 6 and 9 -> 6 never carry
///     6 (0)   7 (200)   8 (0)   9 (0)
///
/// layer 1, drain 0:
///   region E: 11 -> 10 drips, 10 -> 12 -> 10 is the circuit, 13 -> 11 dead
///     10 (0)  11 (400)  12 (100)  13 (0)
///   the grain region: 18 -> 16 fills one member of four
///     14 (50) 15 (50) 16 (0) 17 (50) 18 (400) 19 (50) 20 (50)
///   the loop region: 21 carries its own Route at full capacity
///     21 (1000)  22 (200)  23 (200)
///   the turn region: 24 -> 25 runs dry after four steps
///     24 (32)  25 (0)  26 (50)
/// ```
fn fixture_field() -> FieldState {
    let mut field = FieldState::opening();
    field.next_node_id = 27;
    field.next_route_id = 18;

    let mut ports = vec![PortState {
        node: 1,
        layer: 0,
        pos: Vec2::units(3500, 3500),
        kind: NodeKind::Form,
        q: 1000 * ONE_UNIT,
        open: true,
        upkeep_rate: 0,
        capacity: 4096 * ONE_UNIT,
    }];
    // Region A and region B, on layer 0.
    for (node, x, q) in [(2, 300, 0), (3, 600, 0), (4, 900, 200), (5, 1200, 0)] {
        ports.push(port(node, 0, x, 300, q));
    }
    for (node, x, q) in [(6, 1500, 0), (7, 1800, 200), (8, 2100, 0), (9, 2400, 0)] {
        ports.push(port(node, 0, x, 300, q));
    }
    // Layer 1: region E, the grain region, the loop region.
    for (node, x, q) in [(10, 300, 0), (11, 600, 400), (12, 900, 100), (13, 1200, 0)] {
        ports.push(port(node, 1, x, 900, q));
    }
    for (node, x, q) in [
        (14, 1500, 50),
        (15, 1800, 50),
        (16, 2100, 0),
        (17, 2400, 50),
        (18, 2700, 400),
        (19, 3000, 50),
        (20, 3300, 50),
    ] {
        ports.push(port(node, 1, x, 900, q));
    }
    for (node, x, q) in [(21, 300, 1000), (22, 600, 200), (23, 900, 200)] {
        ports.push(port(node, 1, x, 1500, q));
    }
    // The turn region: Node 24 holds exactly four steps' worth for its one
    // Route, so Node 25's series climbs through the first half of an
    // eight-step window and stands level through the second — the two window
    // halves disagree, which is what a genuine confidence range is made of.
    for (node, x, q) in [(24, 300, 32), (25, 600, 0), (26, 900, 50)] {
        ports.push(port(node, 1, x, 2100, q));
    }
    field.ports = ports;

    field.routes = vec![
        // Region A: the circuit runs in one step because the Routes ascend in
        // the order the Charge travels.
        route(1, 4, 2, LINK),
        route(2, 2, 3, LINK),
        route(3, 3, 4, LINK),
        route(4, 5, 2, LINK),
        // Region B: the same, over two Nodes.
        route(5, 7, 6, LINK),
        route(6, 6, 7, LINK),
        route(7, 8, 6, LINK),
        route(8, 9, 6, LINK),
        // Region E: a one-way drip beside a two-Node circuit.
        route(9, 11, 10, LINK),
        route(10, 10, 12, LINK / 2),
        route(11, 12, 10, LINK / 2),
        route(12, 13, 11, LINK),
        // The grain region: one member fills, the rest stand.
        route(13, 18, 16, LINK),
        // The loop region: a Node's own Route at the locked capacity cap,
        // beside a circuit that runs out through a non-member.
        route(14, 21, 21, 256 * ONE_UNIT),
        route(15, 22, 23, LINK),
        route(16, 23, 22, LINK),
        // The turn region: one Route whose source runs dry mid-window.
        route(17, 24, 25, LINK),
    ];

    field.forms = vec![FormState {
        id: 1,
        form: "thread".to_string(),
        node: 1,
        controlled: true,
        layer: 0,
        pos: Vec2::units(3500, 3500),
        vel: Vec2 { x: 0, y: 0 },
        charge: 1000 * ONE_UNIT,
        reserve: 0,
        pulse_charge: 0,
        focus: false,
        route_reach: 256 * ONE_UNIT,
        forecast_depth: 0,
        steer_scale: field_game_core::state::FRAC_ONE,
        route_capacity: 32 * ONE_UNIT,
        link: None,
        trail: None,
    }];

    field.layers = (0..2u8)
        .map(|layer| FieldLayer {
            layer,
            drain: if layer == 0 { DRAIN } else { 0 },
            noise: 0,
            gain: 0,
            current_ids: Vec::new(),
            port_ids: field
                .ports
                .iter()
                .filter(|held| held.layer == layer && held.kind != NodeKind::Form)
                .map(|held| held.node)
                .collect(),
        })
        .collect();

    // No leakage: the standing View's boundary is not a source of exogenous
    // terms in this fixture, so every `e` a value reads is the layer's Drain.
    field.boundaries = BoundaryState { drawn: Vec::new(), authored: Vec::new(), leak_frac: 0 };
    field
}

/// Plays the Field forward and hands back the run it leaves: the keyframe at
/// step 0, the recorded steps after it, and the Field the last one produced.
fn played(steps: usize, view: ViewDeclaration) -> RunState {
    let keyframe = fixture_field();
    let mut now = keyframe.clone();
    let mut trace = Trace::opening(keyframe);
    for _ in 0..steps {
        let position = RngState { key: 0, ctr: 0, half: 0 };
        let outcome = field::advance(
            &mut now,
            ControlState::default(),
            &view.inside,
            FRAC_ONE,
            &mut field::Unstaged::default().staging(),
        );
        trace.steps.push_back(TraceStep {
            step: now.step,
            rng: position,
            ctl: ControlState::default(),
            records: outcome.records,
        });
    }
    RunState {
        run_id: KEY.to_string(),
        rng: field_game_core::rng::trajectory_stream(KEY, 0),
        content_hash: NO_CONTENT.to_string(),
        branch_nonce: 0,
        progress: Progress::opening(),
        now,
        trace,
        view,
        slate: None,
        input_config: InputConfig::default_config(),
        pressures: Vec::new(),
        schedule: Default::default(),
        anchors: Vec::new(),
    }
}

fn view_of(inside: &[u32]) -> ViewDeclaration {
    ViewDeclaration {
        inside: inside.to_vec(),
        resolution: 1,
        window: 45,
        surround: Surround::Adjacent,
    }
}

/// A slate over the insides named, evaluated. The first is the standing View;
/// every other stands as an authored boundary would, which is a candidate like
/// any other — what is under test here is the reading, not the assembly.
fn evaluated(state: &RunState, insides: &[&[u32]]) -> CandidateSlate {
    let mut slate = CandidateSlate {
        ordinal: 0,
        step: state.now.step,
        sigma: slate::evaluation_stream(KEY, 0, 0),
        tau: TAU_DEFAULT,
        window_declared: state.view.window,
        window_effective: state.effective_window(state.view.window),
        candidates: insides
            .iter()
            .enumerate()
            .map(|(place, inside)| Candidate {
                view: view_of(inside),
                provenance: vec![Provenance {
                    source: if place == 0 { Source::Standing } else { Source::Authored },
                    detail: if place == 0 { None } else { Some(place as i64) },
                }],
                privilege: slate::PrivilegeProfile::unassigned(slate::WINDOW_TOO_SHORT),
                tier: 0,
                baseline: [None; BASELINE_SAMPLES],
            })
            .collect(),
        deficient: insides.len() < 2,
        deficiency_reason: (insides.len() < 2).then(|| "no-alternative-candidate".to_string()),
        omitted: [0; 4],
        discarded: Vec::new(),
        absent: Vec::new(),
        standing_removed: 0,
        standing_fallback: false,
        standing_reason: None,
        forecast_depth: 0,
        dominance: Vec::new(),
        sensitivity: false,
        sensitivity_changed_at: Vec::new(),
        forecast_envelope: Vec::new(),
    };
    rank::evaluate(state, &mut slate);
    slate
}

/// The four values of one candidate, as (value, low, high) or none.
fn values(slate: &CandidateSlate, position: usize) -> [Option<(Frac, Frac, Frac)>; 4] {
    let held = &slate.candidates[position - 1].privilege;
    let each = [
        &held.scale_stability,
        &held.shared_failure,
        &held.cut_impact,
        &held.boundary_sufficiency,
    ];
    each.map(|value| match (value.value, value.low, value.high) {
        (Some(number), Some(low), Some(high)) => Some((number, low, high)),
        _ => None,
    })
}

fn reasons(slate: &CandidateSlate, position: usize) -> [Option<&'static str>; 4] {
    let held = &slate.candidates[position - 1].privilege;
    [
        held.scale_stability.reason,
        held.shared_failure.reason,
        held.cut_impact.reason,
        held.boundary_sufficiency.reason,
    ]
}

fn tier_of(slate: &CandidateSlate, position: usize) -> u8 {
    slate.candidates[position - 1].tier
}

/// One whole: the raw `Frac` of 1.
const ONE: Frac = FRAC_ONE;
/// One half, one quarter, three quarters — the exact fractions the fixtures
/// were built to produce.
const HALF: Frac = FRAC_ONE / 2;
const THREE_QUARTERS: Frac = 3 * FRAC_ONE / 4;

// ---------------------------------------------------------------------------
// Domination: strict on all four
// ---------------------------------------------------------------------------

#[test]
fn one_candidate_dominates_another_strictly_on_all_four() {
    // Region A's inside {2, 3} against region B's {6, 7}, over eight steps.
    //
    // Region A, per step: Route 1 moves 8 units from Node 4 into Node 2,
    // Route 2 moves them on to Node 3, Route 3 returns them to Node 4, and
    // Route 4 never carries because Node 5 holds nothing. Layer 0's Drain then
    // takes 8 units from every Node holding any, so Node 4 falls by 8 a step
    // (200, 192, … 136) and Nodes 2 and 3 end every step at exactly 0.
    //
    //   Scale Stability: both members' stored-Charge series are the constant 0,
    //     so every direction symbol is 0 and so is their block's; agreement is
    //     1 for both blocks, the one qualifying grain pair (1, 2) reads 1, and
    //     the value is 1.
    //   Shared Failure: the probe halves Node 4 to 100 units; the circuit runs
    //     unchanged, so both members end every step at 0 and fail, while of the
    //     surround {4, 5} only Node 5 does. chi(I) = 8·2·1 / ((2−1)·8·2) = 1 and
    //     chi(U) = 0 / ((2−1)·8) = 0, so SF = (1 − 0 + 1) / 2 = 1.
    //   Cut Impact: the recorded circulating flow is Routes 1, 2, 3 at 8 units
    //     over 8 steps = 192 units; severing the crossing set {1, 3, 4} leaves
    //     Node 2 with nothing to send, so no Route carries and CF = 0, giving
    //     (192 − 0) / 192 = 1.
    //   Boundary Sufficiency: both members are shell members, so there is no
    //     interior and no unaccounted influence: M = 8·(8 + 8) = 128 units,
    //     U_int = 0, BS = 1.
    //
    // Region B, per step: Route 5 moves 8 units from Node 7 into Node 6 and
    // Route 6 returns them, so Node 6 ends at 0 and Node 7 falls by 8 a step.
    //
    //   Scale Stability: Node 6 is the constant 0 (symbols 0) and Node 7 falls
    //     by 8 against a range of 56 units and a margin of 56/8 = 7, so its
    //     symbols are all −1; the parent block is their sum, which is Node 7's
    //     own series, so the first block agrees on none of the 7 steps and the
    //     second on all 7. The pair reads (0 + 1) / 2 = 1/2.
    //   Shared Failure: only Node 6 fails, so chi(I) = 0; both surround Nodes
    //     hold nothing and fail every step, so chi(U) = 8·2·1 / ((2−1)·8·2) = 1.
    //     SF = (0 − 1 + 1) / 2 = 0.
    //   Cut Impact: the crossing set is the two Routes that never carry, so
    //     severing them changes nothing and CF is unmoved: (128 − 128) / 128 = 0.
    //   Boundary Sufficiency: Node 6 is the shell and pays no Drain, the
    //     crossing Routes carry nothing, so M = 0; Node 7 is the interior and
    //     pays 8 units of Drain over 8 steps, so U_int = 64 units and BS = 0.
    let state = played(WINDOW, view_of(&[2, 3]));
    let slate = evaluated(&state, &[&[2, 3], &[6, 7]]);

    assert_eq!(
        values(&slate, 1),
        [Some((ONE, ONE, ONE)); 4],
        "region A reads 1 on all four, with the range a spread of nothing gives",
    );
    assert_eq!(
        values(&slate, 2),
        [Some((HALF, HALF, HALF)), Some((0, 0, 0)), Some((0, 0, 0)), Some((0, 0, 0))],
        "region B reads a half, then nothing, three times over",
    );

    // Better is `low_A - high_B > tau` on every one of the four, so the
    // dominance is strict everywhere both are measured.
    assert_eq!(slate.dominance, vec![(1, 2)], "and nothing dominates back");
    assert_eq!(tier_of(&slate, 1), 1);
    assert_eq!(tier_of(&slate, 2), 2, "a dominated candidate stands in the tier below");
    assert_eq!(slate.nondominated(), vec![1]);
    assert!(!slate.sensitivity, "a gap of a half stands through both recomputations");
}

// ---------------------------------------------------------------------------
// Ties: level values
// ---------------------------------------------------------------------------

#[test]
fn two_candidates_level_on_every_value_both_stand() {
    // {6, 8} reads what {2, 3} reads, by the same arithmetic on a different
    // circuit: both members end every step at 0, the surround holds one Node
    // that never fails and one that always does, severing the crossing set
    // stops the circuit, and there is no interior to leave anything
    // unaccounted. So the two are level on all four — neither is better on any
    // of them, because neither pair of ranges is separated at all — and
    // FRAMEWORK.md's own words apply: when A and B are level on every shared
    // value, neither dominates and both stand.
    let state = played(WINDOW, view_of(&[2, 3]));
    let slate = evaluated(&state, &[&[2, 3], &[6, 8]]);

    assert_eq!(values(&slate, 1), [Some((ONE, ONE, ONE)); 4]);
    assert_eq!(values(&slate, 2), [Some((ONE, ONE, ONE)); 4], "level, value for value");
    assert!(slate.dominance.is_empty(), "level everywhere is not dominance");
    assert_eq!(slate.nondominated(), vec![1, 2], "both stand");
    assert_eq!((tier_of(&slate, 1), tier_of(&slate, 2)), (1, 1));
}

// ---------------------------------------------------------------------------
// Multiple maximal candidates: an incomparable pair, both in tier 1
// ---------------------------------------------------------------------------

#[test]
fn an_incomparable_pair_both_stand_in_tier_one() {
    // Region B's {6, 7} against region E's {10, 11}, which trade.
    //
    // Region E stands on layer 1, which drains nothing. Route 9 drips 8 units a
    // step from Node 11 into Node 10, and Routes 10 and 11 circulate 4 units
    // between Node 10 and Node 12, so Node 10 rises by 8 a step and Node 11
    // falls by 8 while their sum stands still.
    //
    //   Scale Stability: the two members' symbols are +1 and −1 against a
    //     parent block whose series is their constant sum, whose symbols are
    //     therefore all 0. Neither block agrees anywhere: the value is 0.
    //   Shared Failure: no member of region E ever ends a step at 0, so the
    //     co-failure index has no failure sum and every sample is undefined —
    //     the value is unassigned, and it drops out of the comparison for both.
    //   Cut Impact: the recorded circulating flow is Routes 10 and 11 at 4
    //     units over 8 steps = 64 units; severing the crossing set leaves only
    //     the one-way drip, which closes no circuit, so CF = 0 and the value
    //     is 1.
    //   Boundary Sufficiency: both members are shell members and layer 1 pays
    //     no Drain, so M = 8·(4 + 4) = 64 units and U_int = 0: the value is 1.
    //
    // So region B is better on Scale Stability and region E on both of the
    // others: each is better somewhere, neither dominates, and both stand.
    let state = played(WINDOW, view_of(&[6, 7]));
    let slate = evaluated(&state, &[&[6, 7], &[10, 11]]);

    assert_eq!(
        values(&slate, 1),
        [Some((HALF, HALF, HALF)), Some((0, 0, 0)), Some((0, 0, 0)), Some((0, 0, 0))],
    );
    assert_eq!(
        values(&slate, 2),
        [Some((0, 0, 0)), None, Some((ONE, ONE, ONE)), Some((ONE, ONE, ONE))],
    );
    assert_eq!(
        reasons(&slate, 2)[1],
        Some("few-samples"),
        "a co-failure index with no failure to read is unassigned, with its reason",
    );

    assert!(slate.dominance.is_empty(), "each better somewhere is not dominance either way");
    assert_eq!(slate.nondominated(), vec![1, 2], "multiple maximal candidates");
    assert_eq!((tier_of(&slate, 1), tier_of(&slate, 2)), (1, 1));
}

// ---------------------------------------------------------------------------
// Tolerance sensitivity
// ---------------------------------------------------------------------------

#[test]
fn a_comparison_the_tolerance_decides_is_flagged_sensitive() {
    // Two candidates that share exactly one assigned value, and are separated
    // on it by exactly a quarter.
    //
    // {19, 20} stand alone on layer 1: no Route, no adjacency, nothing to
    // observe but their own constant stored Charge. Both blocks and their
    // parent read 0 everywhere, so Scale Stability is 1; the surround is empty,
    // there is no circulating flow, and there is no traffic at all, so the
    // other three are unassigned.
    //
    // {14, 15, 16, 17} take Route 13's 8 units a step into Node 16 and stand
    // still otherwise. In proximity order the blocks are the four members and
    // the parents are {14, 15} — a constant sum, symbols 0 — and {16, 17},
    // which rises with Node 16. Three blocks agree with their parent and Node
    // 17, standing still under a rising parent, agrees on none: the pair reads
    // (1 + 1 + 1 + 0) / 4 = 3/4.
    //
    // The gap is 1 − 3/4 = 1/4. Better is a separation of more than tau:
    //   at tau = 1/8   1/4 > 1/8   yes, so {19, 20} dominates
    //   at tau/2 = 1/16  1/4 > 1/16  yes, unchanged
    //   at 2·tau = 1/4   1/4 > 1/4   no, so the two are level and both stand
    // The doubled tolerance changes the nondominated set, and that is what the
    // flag reports.
    let state = played(WINDOW, view_of(&[19, 20]));
    let slate = evaluated(&state, &[&[19, 20], &[14, 15, 16, 17]]);

    assert_eq!(values(&slate, 1)[0], Some((ONE, ONE, ONE)));
    assert_eq!(
        values(&slate, 2)[0],
        Some((THREE_QUARTERS, THREE_QUARTERS, THREE_QUARTERS)),
        "three blocks of four agree with their parent",
    );
    assert_eq!(
        reasons(&slate, 1),
        [None, Some("few-surround"), Some("no-circulating-flow"), Some("few-samples")],
        "an inside nothing reaches carries three different reasons",
    );

    assert_eq!(slate.dominance, vec![(1, 2)], "at the declared tolerance, one dominates");
    assert!(slate.sensitivity, "and the reading turns on the tolerance");
    assert_eq!(slate.sensitivity_changed_at, vec!["double"]);
}

// ---------------------------------------------------------------------------
// Unassigned results
// ---------------------------------------------------------------------------

#[test]
fn every_unassigned_reason_the_closed_set_names_is_reachable() {
    let state = played(WINDOW, view_of(&[19, 20]));

    // A short window: the clamp leaves nothing to observe, and FRAMEWORK.md
    // says every windowed procedure is then unassigned. This is the reading
    // right after an applying commit.
    let empty = played(0, view_of(&[2, 3]));
    let short = evaluated(&empty, &[&[2, 3], &[6, 7]]);
    assert_eq!(short.candidates[0].privilege.each().map(|held| held.reason), [
        Some("window-too-short"),
        Some("window-too-short"),
        Some("window-too-short"),
        Some("window-too-short"),
    ]);
    assert_eq!(
        short.candidates[0].baseline,
        [None; BASELINE_SAMPLES],
        "and no baseline replay ran, so every deviation slot stands empty",
    );
    assert!(short.dominance.is_empty(), "nothing is comparable when nothing is assigned");
    assert_eq!(short.nondominated(), vec![1, 2], "an empty shared set leaves both standing");

    // One member: Shared Failure needs two, and no grain pair fits inside a
    // member count of one.
    let alone = evaluated(&state, &[&[19], &[19, 20]]);
    assert_eq!(
        reasons(&alone, 1),
        [Some("no-grain-pair"), Some("few-members"), Some("no-circulating-flow"), Some("few-samples")],
    );

    // Two members with a surround of fewer than two: {19, 20} stand alone, so
    // their surround is empty, and Shared Failure is unassigned for the last
    // reason of the closed set this test had not yet reached.
    assert_eq!(reasons(&alone, 2)[1], Some("few-surround"));
}

// ---------------------------------------------------------------------------
// The self-loop, which no procedure special-cases
// ---------------------------------------------------------------------------

#[test]
fn a_full_capacity_self_loop_reads_cut_impact_low_rather_than_unassigned() {
    // Node 21 carries its own Route at the locked capacity cap, 256 units a
    // step, and Node 22 stands in a two-Node circuit that runs out through the
    // non-member Node 23. The inside is {21, 22}.
    //
    // A Route from a Node to itself is never in the crossing set — no
    // self-loop has exactly one endpoint in the inside — so severance never
    // removes it. Its flow enters the recorded circulating flow and every
    // severed one alike, cancels in the numerator, and inflates only the
    // denominator:
    //
    //   CF_rec = 8·256 + 8·8 + 8·8 = 2176 units
    //   CF_j   = 8·256            = 2048 units   (the circuit through 23 is cut)
    //   CI     = (2176 − 2048) / 2176 = 128 / 2176 = 1/17
    //
    // 1/17 of 65536 is 3855.06, which rounds to 3855. The reading is low, and
    // it is the truth about an inside that circulates through its own Route:
    // it is assigned, not unassigned, and no procedure special-cases it.
    let state = played(WINDOW, view_of(&[21, 22]));
    let slate = evaluated(&state, &[&[21, 22], &[19, 20]]);

    let cut = &slate.candidates[0].privilege.cut_impact;
    assert_eq!(cut.value, Some(3855), "one seventeenth, rounded to nearest");
    assert_eq!((cut.low, cut.high), (Some(3855), Some(3855)));
    assert_eq!(cut.reason, None, "assigned, and low, rather than unassigned");
    assert!(cut.value.unwrap() < TAU_DEFAULT, "and low enough that nothing is better than it");
}

// ---------------------------------------------------------------------------
// A genuine confidence range, and the comparison that consumes it
// ---------------------------------------------------------------------------

#[test]
fn a_series_that_turns_mid_window_produces_a_genuine_range_the_comparison_reads() {
    // The window-split range, exercised with three parts that disagree — no
    // hand-built values anywhere: every number below is produced by the
    // procedure over the played window.
    //
    // The turn region: Node 24 holds 32 units, exactly four steps' worth for
    // its one 8-unit Route, so Node 25's stored-Charge series over the
    // eight-step window is [8, 16, 24, 32, 32, 32, 32, 32] — climbing through
    // the first half, level through the second. Node 26 stands constant at 50.
    // The inside is {25, 26}, resolution 1, one qualifying grain pair (1, 2).
    //
    // Full window: Node 25's range is 24 units, its margin 24/8 = 3, and the
    // deltas +8,+8,+8,0,0,0,0 read +1,+1,+1,0,0,0,0. Node 26 reads all 0. The
    // parent block is their sum, same deltas as 25, same symbols. Block 25
    // agrees with the parent on 7/7 = 65536; block 26 on the four 0s, 4/7 →
    // round(4·65536/7) = round(37449.14) = 37449. The pair value is the mean:
    // (65536 + 37449) / 2 = 51492.5, halves upward → 51493.
    //
    // First half [8, 16, 24, 32]: margin 12/8, symbols +1,+1,+1 against the
    // constant's 0,0,0 — agreements 3/3 and 0/3, mean 32768.
    // Second half [32, 32, 32, 32]: every series level, every symbol 0 —
    // agreements 3/3 and 3/3, mean 65536.
    //
    // So the three parts are 51493, 32768, and 65536: the value is the
    // full-window part, the range is [smallest, largest] of the three, and the
    // containment is strict on both sides — a swapped bound at either end
    // would fail this pin.
    let state = played(WINDOW, view_of(&[25, 26]));
    let slate = evaluated(&state, &[&[25, 26], &[19, 20]]);

    let range = &slate.candidates[0].privilege.scale_stability;
    assert_eq!(range.value, Some(51493), "the full-window part");
    assert_eq!(range.low, Some(32768), "the first half, where the halves disagree");
    assert_eq!(range.high, Some(65536), "the second half");
    assert!(
        range.low < range.value && range.value < range.high,
        "the range contains the value strictly, so both bounds are load-bearing",
    );

    // And the comparison consumes that produced range. {19, 20} read Scale
    // Stability 1 with the point range a window of constants gives; the bare
    // numbers stand 65536 − 51493 = 14043 apart, which is more than tau — a
    // comparison of numbers would separate them. The comparison reads ranges:
    // low_B − high_A = 65536 − 65536 = 0 is no separation at all, so the two
    // are level, neither dominates, and both stand in tier 1.
    let point = &slate.candidates[1].privilege.scale_stability;
    assert_eq!(
        (point.low, point.value, point.high),
        (Some(65536), Some(65536), Some(65536)),
    );
    assert!(
        point.value.unwrap() - range.value.unwrap() > TAU_DEFAULT,
        "the bare numbers are separated by more than the tolerance",
    );
    assert!(!rank::better(point, range, TAU_DEFAULT), "but the produced ranges overlap");
    assert!(!rank::better(range, point, TAU_DEFAULT));
    assert!(slate.dominance.is_empty(), "so neither dominates");
    assert_eq!((tier_of(&slate, 1), tier_of(&slate, 2)), (1, 1), "and both stand");
    assert!(!slate.sensitivity, "an overlap of ranges is level at every recomputed tolerance");
}

#[test]
fn a_ranked_slate_rides_the_export_file_byte_for_byte() {
    // The round trip, extended through ranking: a slate with assigned values,
    // a genuine confidence range, a nonempty dominance relation, and a filled
    // envelope is written into the payload, read back, and re-serialized to
    // the same bytes — and the export file it rides repeats exactly.
    let mut state = played(WINDOW, view_of(&[2, 3]));
    let slate = evaluated(&state, &[&[2, 3], &[6, 7], &[25, 26]]);
    // {2, 3} dominates {6, 7} strictly, and {25, 26} dominates it too: level on
    // Scale Stability (32768 against 32768 is no separation), better on
    // Boundary Sufficiency, unassigned elsewhere and so excluded.
    assert_eq!(slate.dominance, vec![(1, 2), (3, 2)], "the record under test is a ranked one");
    let ranged = &slate.candidates[2].privilege.scale_stability;
    assert!(ranged.low < ranged.high, "and carries a range wider than a point");
    let written = slate.written();
    state.slate = Some(slate);

    let payload = state.payload();
    let file = state.export_file();
    let parsed = field_game_core::json::parse(&payload).expect("the payload parses");
    let back = RunState::read(&parsed).expect("the reader admits the ranked record");
    back.coherent().expect("and the state it rides in is coherent");
    assert_eq!(back.payload(), payload, "the payload re-serializes byte for byte");
    assert_eq!(back.export_file(), file, "and so does the export file");
    assert_eq!(
        back.slate.as_ref().expect("the slate rode the payload").written(),
        written,
    );
}

// ---------------------------------------------------------------------------
// The record, the ranking surface, and reproducibility
// ---------------------------------------------------------------------------

#[test]
fn the_four_values_are_never_collapsed_into_one() {
    // The record carries four values, a confidence range around each, a tier,
    // and the dominance relation the tier derives from — and no figure that
    // stands for a candidate as a whole. This is the shape check the
    // no-collapse rule reduces to at the boundary.
    let state = played(WINDOW, view_of(&[2, 3]));
    let slate = evaluated(&state, &[&[2, 3], &[6, 7]]);
    let written = slate.written();

    for name in ["boundary_sufficiency", "cut_impact", "scale_stability", "shared_failure"] {
        assert!(written.contains(&format!("\"{name}\":{{")), "{name} stands on its own");
    }
    assert!(written.contains("\"dominance\":[{\"a\":1,\"b\":2}]"));
    assert!(written.contains("\"tier\":1"));
    assert!(written.contains("\"tier\":2"));
    // Every ranked candidate's baseline deviations are recorded, and they are
    // zero: a replay with no edit reproduces the recorded window exactly.
    assert!(written.contains("\"deviations\":[0,0,0,0,0,0,0,0]"));

    let parsed = field_game_core::json::parse(&written).expect("the record parses");
    let held = CandidateSlate::read(
        &field_game_core::json::parse(&format!("{{\"slate\":{written}}}"))
            .expect("wrapped"),
        "slate",
    )
    .expect("the reader admits what this build writes");
    assert_eq!(held.written(), written, "and re-serializes to the same bytes");
    assert!(parsed.get("detail").is_some_and(field_game_core::json::Json::is_null));
}

#[test]
fn the_same_six_inputs_produce_the_same_ranked_record() {
    // The locked reproducibility sentence, extended through the ranking: the
    // same state, standing View, assembly ordinal, previous assembly step,
    // `sigma_V`, and tolerance reproduce the record exactly.
    let state = played(WINDOW, view_of(&[2, 3]));
    let first = evaluated(&state, &[&[2, 3], &[6, 7], &[10, 11]]);
    let second = evaluated(&state, &[&[2, 3], &[6, 7], &[10, 11]]);
    assert_eq!(first.written(), second.written());

    // And the streams are the locked splits, one per sample.
    let sigma = slate::evaluation_stream(KEY, 0, 0);
    assert_eq!(
        rank::sample_stream(&sigma, 1, rank::STREAM_SHARED_FAILURE, 3),
        sigma.split(&[
            field_game_core::rng::Part::Name("candidate"),
            field_game_core::rng::Part::Number(1),
            field_game_core::rng::Part::Name("shared-failure"),
            field_game_core::rng::Part::Number(3),
        ]),
    );
}

#[test]
fn the_forecast_envelope_is_the_standing_candidates_baseline_spread() {
    // The only forward reading the game may show: the smallest and largest
    // `q_I` the eight baseline replays stood at, one pair per window step. The
    // replays draw nothing, so the spread is a point at each step — and the
    // point is the recorded total, because a replay with no edit reproduces
    // the window.
    let state = played(WINDOW, view_of(&[7]));
    let slate = evaluated(&state, &[&[7], &[6, 7]]);
    assert_eq!(slate.forecast_envelope.len(), WINDOW);
    // Node 7 falls by 8 units a step from 200: 192, 184, … 136.
    let expected: Vec<(i64, i64)> = (1..=WINDOW)
        .map(|step| {
            let held = (200 - 8 * step as i64) * ONE_UNIT;
            (held, held)
        })
        .collect();
    assert_eq!(slate.forecast_envelope, expected);
}

#[test]
fn a_deficient_slate_is_not_compared_and_keeps_the_tier_no_tier_has() {
    // No comparison runs on a slate of one and nothing is adopted from one,
    // but the four values are still computed and recorded for the standing
    // View — FRAMEWORK.md's deficient-slate rule, read literally.
    let state = played(WINDOW, view_of(&[2, 3]));
    let slate = evaluated(&state, &[&[2, 3]]);
    assert!(slate.deficient);
    assert_eq!(values(&slate, 1), [Some((ONE, ONE, ONE)); 4], "the values are still read");
    assert_eq!(tier_of(&slate, 1), 0, "and the tier is the number no tier has");
    assert!(slate.dominance.is_empty());
    assert!(slate.nondominated().is_empty(), "nothing is adopted from a deficient slate");
}

#[test]
fn the_comparison_reads_ranges_rather_than_the_numbers_inside_them() {
    // Two values whose numbers differ by more than the tolerance but whose
    // ranges overlap are level, which is the whole reason the comparison reads
    // ranges at all.
    let wide = PrivilegeValue::assigned(ONE, 0, ONE, 8);
    let narrow = PrivilegeValue::assigned(0, 0, 0, 8);
    assert!(
        !rank::better(&wide, &narrow, TAU_DEFAULT),
        "a low of 0 against a high of 0 is a separation of nothing",
    );
    let clear = PrivilegeValue::assigned(ONE, ONE, ONE, 8);
    assert!(rank::better(&clear, &narrow, TAU_DEFAULT));
    assert!(!rank::better(&narrow, &clear, TAU_DEFAULT));

    // An unassigned value is excluded from the comparison rather than counted
    // against either candidate.
    let missing = PrivilegeValue::unassigned("few-samples");
    assert!(!rank::better(&clear, &missing, TAU_DEFAULT));
    assert!(!rank::better(&missing, &clear, TAU_DEFAULT));
}
