//! Candidate-View generation: the seven sources, the variants, the rotation,
//! and the record the slate carries.
//!
//! `docs/field-framework/FRAMEWORK.md`'s Candidate slate section is what is
//! under test, read as the procedure it is written as. The Field these run on
//! is built here rather than played into, for the reason the assembly's own
//! reproducibility sentence gives: assembly is a pure function of the Field
//! state, the standing View, the assembly ordinal, the previous assembly's
//! evaluation step, `sigma_V`, and the tolerance, so a state built to name each
//! of those exactly is the readable way to ask what it does with them.
//!
//! Every Node of that Field stands 300 units from the next, which is past the
//! 256-unit adjacency radius: the declared adjacency is therefore empty and
//! every neighbourhood rule under test — the surround set, the shell, the
//! coarser variant — reads Routes alone. Adjacency has its own test beside
//! them.

use field_game_core::field::{
    BoundaryState, DrawnBoundary, FieldLayer, FormState, NodeKind, PortState, RouteState,
    StepRecords, NODES_PER_RUN, ROUTES_PER_RUN,
};
use field_game_core::fx::{Vec2, ONE_UNIT};
use field_game_core::json::{parse, write_text, Json};
use field_game_core::rng::RngState;
use field_game_core::slate::{self, CandidateSlate, Source, SLATE_CAP};
use field_game_core::state::{
    ControlState, FieldState, InputConfig, Progress, RunState, Step, Surround, Trace, TraceStep,
    ViewDeclaration,
};
use field_game_core::Session;

mod support;

const KEY: &str = "0123456789abcdef";

/// A content hash no build ever computed, so a run opened on this state runs no
/// authored sequence.
const NO_CONTENT: &str = "00000000000000000000000000000000000000000000000000000000000000ff";

/// How long a ramp takes, in microseconds. Locked.
const RAMP_US: i64 = 250_000;

/// 100 units of Charge, the swing the response rule is read against.
const SWING: i64 = 100 * ONE_UNIT;

// ---------------------------------------------------------------------------
// The Field these run on
// ---------------------------------------------------------------------------

/// One layer, a controlled Form on Node 1 standing well away from everything,
/// Ports 2 through 8 in a line 300 units apart, and five Routes:
/// 1: 2 → 3, 2: 3 → 4, 3: 5 → 6, 4: 6 → 7, and 5: 2 → 8.
///
/// The first four carry; the fifth never does. That is what separates the two
/// neighbourhood readings the section uses: `C1` takes in every non-member a
/// Route reaches, carrying or not, and the lateral variant's qualifying set
/// takes in only those with positive window flow.
fn spread_field() -> FieldState {
    let mut field = FieldState::opening();
    field.next_node_id = 9;
    field.next_route_id = 6;
    field.layers = vec![FieldLayer {
        layer: 0,
        drain: 0,
        noise: 0,
        gain: 0,
        current_ids: Vec::new(),
        port_ids: vec![2, 3, 4, 5, 6, 7, 8],
    }];
    field.ports = vec![
        placed(1, NodeKind::Form, 3500, 3500, 8 * ONE_UNIT),
        port(2, NodeKind::Port, 600, 300),
        port(3, NodeKind::Port, 900, 300),
        port(4, NodeKind::Port, 1200, 300),
        port(5, NodeKind::Port, 1500, 300),
        port(6, NodeKind::Port, 1800, 300),
        port(7, NodeKind::Port, 2100, 300),
        port(8, NodeKind::Port, 2400, 300),
    ];
    field.routes = vec![
        route(1, 2, 3),
        route(2, 3, 4),
        route(3, 5, 6),
        route(4, 6, 7),
        route(5, 2, 8),
    ];
    field.forms = vec![FormState {
        id: 1,
        form: "thread".to_string(),
        node: 1,
        controlled: true,
        layer: 0,
        pos: Vec2::units(3500, 3500),
        vel: Vec2 { x: 0, y: 0 },
        charge: 8 * ONE_UNIT,
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
    field.boundaries = BoundaryState { drawn: Vec::new(), authored: Vec::new(), leak_frac: 0 };
    field
}

fn port(node: u32, kind: NodeKind, x: i64, y: i64) -> PortState {
    placed(node, kind, x, y, 0)
}

/// A Port carrying stored Charge. A Form's own Node mirrors the Form, its
/// stored Charge among the three things they share.
fn placed(node: u32, kind: NodeKind, x: i64, y: i64, q: i64) -> PortState {
    PortState {
        node,
        layer: 0,
        pos: Vec2::units(x, y),
        kind,
        q,
        open: true,
        upkeep_rate: 0,
        capacity: 512 * ONE_UNIT,
    }
}

fn route(route: u32, tail: u32, head: u32) -> RouteState {
    RouteState { route, tail, head, capacity: 8 * ONE_UNIT, flow: 0, formed_step: 0 }
}

/// The six recorded steps the window reads.
///
/// Route flow: the two Routes among Nodes 2, 3, and 4 each move 100 units on
/// the first recorded step and nothing after, so both window flows are 100,
/// the median of the positive weights is 100, and both pairs are strong — one
/// cluster, over {2, 3, 4}. The other three Routes never carry.
///
/// Stored Charge: Nodes 2 and 4 swing between 0 and 100 units on every step and
/// Node 3 holds steady, so 2 and 4 respond at every response step and 3 at
/// none. The co-response count of the one pair is 5, past the threshold of 2.
fn recorded_steps() -> Vec<TraceStep> {
    (1..=6)
        .map(|step| {
            let mut records = StepRecords::default();
            if step == 1 {
                records.f = vec![(1, 100 * ONE_UNIT), (2, 100 * ONE_UNIT)];
            }
            // The swing is on the odd steps, which puts a change on every step
            // after the first — the response steps.
            let swinging = if step % 2 == 0 { 0 } else { SWING };
            if swinging > 0 {
                records.q.push((2, swinging));
            }
            records.q.push((3, SWING));
            if swinging > 0 {
                records.q.push((4, swinging));
            }
            records.q.sort_by_key(|(node, _)| *node);
            TraceStep {
                step,
                rng: RngState { key: 0, ctr: 0, half: 0 },
                ctl: ControlState::default(),
                records,
            }
        })
        .collect()
}

/// A run standing on that Field under a View the caller names.
///
/// The completed step is the number of recorded steps, so the retained span
/// and the trajectory agree exactly as a played run's do — which is what makes
/// the effective window the clamp's own reading.
fn state_of(mut field: FieldState, view: ViewDeclaration, steps: Vec<TraceStep>) -> RunState {
    let mut keyframe = field.clone();
    keyframe.step = 0;
    // A keyframe holds the state as of its own step, so it carries no drag
    // recorded after it.
    keyframe.boundaries.drawn.clear();
    field.step = steps.len() as Step;
    let mut trace = Trace::opening(keyframe);
    for step in steps {
        trace.steps.push_back(step);
    }
    RunState {
        run_id: KEY.to_string(),
        rng: field_game_core::rng::trajectory_stream(KEY, 0),
        content_hash: NO_CONTENT.to_string(),
        branch_nonce: 0,
        progress: Progress::opening(),
        now: field,
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

/// The whole fixture: the spread Field, the six recorded steps, a standing
/// inside of {2, 3}, one drawn entry over {5, 6} and one authored entry over
/// {6, 7}.
fn fixture() -> RunState {
    let mut field = spread_field();
    field.boundaries.drawn = vec![DrawnBoundary { members: vec![5, 6], step: 4 }];
    field.boundaries.authored = vec![vec![6, 7]];
    state_of(field, view_of(&[2, 3]), recorded_steps())
}

/// The inside each candidate declares, in assembly order.
fn insides(slate: &CandidateSlate) -> Vec<Vec<u32>> {
    slate.candidates.iter().map(|entry| entry.view.inside.clone()).collect()
}

/// The provenance of one candidate, as (source name, detail) pairs.
fn provenance(slate: &CandidateSlate, position: usize) -> Vec<(String, Option<i64>)> {
    slate.candidates[position - 1]
        .provenance
        .iter()
        .map(|held| (held.source.name().to_string(), held.detail))
        .collect()
}

/// Where one source's candidate landed, or none when it reached no seat.
fn seat_of(slate: &CandidateSlate, source: Source) -> Option<usize> {
    slate.candidates.iter().position(|entry| {
        entry.provenance.iter().any(|held| held.source == source)
    })
}

// ---------------------------------------------------------------------------
// Seat 1, the variants, and the conditions that keep them out
// ---------------------------------------------------------------------------

#[test]
fn the_slate_opens_with_the_standing_view_and_the_three_variants() {
    let slate = slate::assemble(&fixture());

    // Seat 1 is the standing View, post-intake, with `standing` provenance.
    assert_eq!(insides(&slate)[0], vec![2, 3]);
    assert_eq!(provenance(&slate, 1)[0], ("standing".to_string(), None));

    // Then the permitted variants, in the order finer, coarser, laterally
    // shifted. The interior of {2, 3} is empty — Node 2 reaches Node 8 and
    // Node 3 reaches Node 4 — so the finer rule drops the weakest member, and
    // the two are level on internal attachment, so the smaller identifier goes.
    assert_eq!(insides(&slate)[1], vec![3], "finer");
    assert_eq!(provenance(&slate, 2), vec![("finer".to_string(), None)]);
    // C1 takes in every non-member a Route reaches, the one that never carries
    // among them.
    assert_eq!(insides(&slate)[2], vec![2, 3, 4, 8], "coarser");
    assert_eq!(provenance(&slate, 3), vec![("coarser".to_string(), None)]);
    // The lateral shift takes the weakest member out and the most attached
    // qualifying non-member in; Node 8 does not qualify, because the Route to
    // it carries nothing.
    assert_eq!(insides(&slate)[3], vec![3, 4], "laterally shifted");
    assert_eq!(provenance(&slate, 4), vec![("lateral".to_string(), None)]);

    assert!(slate.absent.is_empty(), "the Field permits all three");
    assert_eq!(slate.window_declared, 45);
    assert_eq!(slate.window_effective, 6, "the retained span clamps the window");
    assert_eq!(slate.step, 6, "the evaluation step is the last completed step");
    assert_eq!(slate.tau, 8192, "the declared tolerance, 1/8");
    assert_eq!(slate.sigma, slate::evaluation_stream(KEY, 0, 0), "sigma_V is the ordinal's split");
}

#[test]
fn the_finer_variant_takes_the_interior_when_one_stands() {
    // An inside with an interior: Node 3 reaches only members, and Node 4
    // reaches nothing outside, so the interior is {3, 4} and differs from I0.
    let slate = slate::assemble(&state_of(spread_field(), view_of(&[2, 3, 4]), recorded_steps()));
    assert_eq!(insides(&slate)[1], vec![3, 4], "the interior is the finer inside");
}

#[test]
fn each_absent_variant_records_the_condition_that_failed() {
    // |I0| < 2 keeps the finer variant out, and takes the lateral one with it:
    // `k'` is held below the member count, and one member has none to spare.
    let single = slate::assemble(&state_of(spread_field(), view_of(&[3]), recorded_steps()));
    let conditions: Vec<(String, String)> = single
        .absent
        .iter()
        .map(|held| (held.variant.name().to_string(), held.condition.clone()))
        .collect();
    assert_eq!(
        conditions,
        vec![
            ("finer".to_string(), "|I0| < 2".to_string()),
            ("lateral".to_string(), "k' = 0".to_string()),
        ],
    );
    assert_eq!(insides(&single)[1], vec![2, 3, 4], "and the coarser one still stands");

    // C1 = I0 keeps the coarser variant out: the whole of N reaches nothing
    // outside itself.
    let whole = slate::assemble(&state_of(
        spread_field(),
        view_of(&[1, 2, 3, 4, 5, 6, 7, 8]),
        recorded_steps(),
    ));
    let coarser: Vec<String> = whole
        .absent
        .iter()
        .filter(|held| held.variant == Source::Coarser)
        .map(|held| held.condition.clone())
        .collect();
    assert_eq!(coarser, vec!["C1 = I0".to_string()]);
    // And so does the lateral variant, for its own reason: no non-member is
    // left to take in.
    assert!(whole.absent.iter().any(|held| held.variant == Source::Lateral));
}

// ---------------------------------------------------------------------------
// The four situational sources
// ---------------------------------------------------------------------------

#[test]
fn every_situational_source_reaches_a_slate_and_records_why() {
    // Four candidates stand before the situational sources are offered, so one
    // seat is open per assembly. Over the four assemblies of one rotation each
    // source takes that seat exactly once, which is also how each source's own
    // proposal and detail is read here.
    let mut found = Vec::new();
    for ordinal in 0..4 {
        let mut state = fixture();
        state.now.assembly_ordinal = ordinal;
        // Past the drawn entry's own step, so nothing is fresh and the leader
        // of the rotated cycle is the first offer.
        state.now.prev_assembly_step = Some(6);
        let slate = slate::assemble(&state);
        assert_eq!(slate.candidates.len(), SLATE_CAP, "the slate fills");
        found.push((insides(&slate)[4].clone(), provenance(&slate, 5)));
    }

    assert_eq!(
        found[0],
        (vec![5, 6], vec![("drawn".to_string(), Some(1))]),
        "source 1, with the drawn entry's own position",
    );
    assert_eq!(
        found[1],
        (vec![2, 3, 4], vec![("clusters".to_string(), Some(200 * ONE_UNIT))]),
        "source 2, with the cluster's total internal weight",
    );
    assert_eq!(
        found[2],
        (vec![6, 7], vec![("authored".to_string(), Some(1))]),
        "source 3, with the authored position",
    );
    assert_eq!(
        found[3],
        (vec![2, 4], vec![("responses".to_string(), Some(5))]),
        "source 4, with the total co-response count",
    );
}

#[test]
fn the_rotation_gives_each_source_the_lead_once_in_four_assemblies() {
    // The cycle starts at source `1 + (n mod 4)`, so the leader moves with the
    // ordinal and comes back round on the fifth assembly.
    let leaders: Vec<Source> = (0..5)
        .map(|ordinal| {
            let mut state = fixture();
            state.now.assembly_ordinal = ordinal;
            state.now.prev_assembly_step = Some(6);
            let slate = slate::assemble(&state);
            slate.candidates[4].provenance[0].source
        })
        .collect();
    assert_eq!(
        leaders,
        vec![Source::Drawn, Source::Clusters, Source::Authored, Source::Responses, Source::Drawn],
    );

    // A source that reached no seat is counted as omitted, per source.
    let mut state = fixture();
    state.now.assembly_ordinal = 1;
    state.now.prev_assembly_step = Some(6);
    let slate = slate::assemble(&state);
    assert_eq!(slate.omitted, [1, 0, 1, 1], "drawn, clusters, authored, responses");
}

/// The played history both freshness tests stand on: enter Still Mode, drag
/// a boundary around {5, 6} — recorded at queue time, and kept through the
/// undo — and commit a cut, which reassembles the slate.
fn dragged_and_committed() -> Session {
    let mut state = fixture();
    state.now.boundaries.drawn.clear();
    let mut session = played_into_still(state);
    body(&session.command(
        "queue_plan",
        "{\"plan\":{\"members\":[5,6],\"op\":\"reshape_boundary\"}}",
    ));
    body(&session.command("undo_plan", "{}"));
    body(&session.command("queue_plan", "{\"plan\":{\"op\":\"cut\",\"route\":4}}"));
    body(&session.command("commit_plan", "{}"));
    session
}

#[test]
fn a_drag_made_in_still_mode_leads_the_very_next_assembly() {
    // The play-reachable shape of the freshness rule. A still run runs no
    // step, so a drag is recorded at the very step the assembly that opened
    // its session evaluated at — fresh is at or after, never strictly after —
    // and the reassembly a commit runs offers the player's own act ahead of
    // the rotated leader.
    let mut session = dragged_and_committed();
    let run = session.run().expect("a run is loaded");
    let slate = run.standing_slate().expect("the commit reassembled");
    assert_eq!(slate.ordinal, 1);
    assert_eq!(
        run.state().now.boundaries.drawn[0].step,
        slate.step,
        "the drag shares the step of the assembly that opened its session",
    );

    // At n = 1 source 1 stands last in the rotated cycle — clusters, authored,
    // responses, then drawn — so without freshness the drawn entry could only
    // follow the authored one. Fresh, it precedes the leader.
    let drawn = seat_of(slate, Source::Drawn).expect("the drag reached the slate");
    let authored = seat_of(slate, Source::Authored).expect("and the authored entry beside it");
    assert!(drawn < authored, "the player's own act stands ahead of the rotated cycle");
    assert_eq!(slate.candidates[drawn].view.inside, vec![5, 6]);
}

#[test]
fn a_state_restore_judges_freshness_from_the_step_it_returned_to() {
    // The same played history, exported twice: once at the very step the drag
    // was recorded at, and once forty steps later. A restore unsets the
    // previous assembly's evaluation step and supplies the step it returned
    // to, so the first restore reads the drag as fresh and the second as
    // stale — and the slate the re-entry assembles says which, in the order
    // its seats filled.
    let mut session = dragged_and_committed();
    let at_the_step = exported(&mut session);
    session.command("input_frame", &frame(4, 0, false, 1_100_000 + 2 * RAMP_US));
    assert_eq!(session.lifecycle(), "running", "the committed exit completed");
    session.command("input_frame", &frame(5, 40, false, 2_000_000));
    let later = exported(&mut session);

    let fresh = reentered(&at_the_step);
    let drawn = seat_of(&fresh, Source::Drawn).expect("the drag reached the slate");
    let authored = seat_of(&fresh, Source::Authored).expect("the authored entry beside it");
    assert!(drawn < authored, "returned to the drag's own step, the drag is fresh and leads");
    assert_eq!(fresh.candidates[drawn].view.inside, vec![5, 6]);

    let stale = reentered(&later);
    let drawn = seat_of(&stale, Source::Drawn).expect("the entry still rotates in");
    let authored = seat_of(&stale, Source::Authored).expect("behind the leader");
    assert!(
        authored < drawn,
        "returned past the drag's step, nothing leads and the cycle order stands",
    );
}

#[test]
fn a_self_loop_route_forms_no_cluster_and_carries_no_pair_weight() {
    // A Route from a Node to itself is that Node's own Route, never an edge
    // between two Nodes — the locked principle — and this fixture is built so
    // the two readings disagree. The pair weights are 100, 250, and 300, an
    // odd count whose median is 250, so both the 250 pair and the 300 pair
    // are strong. A reading that counted the loop's 900 among them would read
    // an even count with a doubled median of 550 and drop the 250 pair; under
    // the locked reading the loop moves nothing, and the 250 cluster is what
    // says so.
    let mut field = spread_field();
    field.routes = vec![
        route(1, 2, 3),
        route(2, 3, 4),
        route(3, 5, 6),
        RouteState {
            route: 4,
            tail: 5,
            head: 5,
            capacity: 8 * ONE_UNIT,
            flow: 0,
            formed_step: 0,
        },
    ];
    let mut steps: Vec<TraceStep> = recorded_steps();
    for step in &mut steps {
        step.records.f.clear();
        step.records.q.clear();
        step.records.z.clear();
    }
    steps[0].records.f = vec![
        (1, 100 * ONE_UNIT),
        (2, 250 * ONE_UNIT),
        (3, 300 * ONE_UNIT),
        (4, 900 * ONE_UNIT),
    ];
    let mut state = state_of(field, view_of(&[2, 3]), steps);
    // Ordinal 1 leads with source 2, and nothing is fresh.
    state.now.assembly_ordinal = 1;
    state.now.prev_assembly_step = Some(7);
    let slate = slate::assemble(&state);

    // The strongest cluster takes the open seat, and the 250 cluster — which
    // the counting reading would have dropped — equals the lateral variant,
    // so its provenance merges there: the discriminating evidence, twice over.
    assert_eq!(insides(&slate)[4], vec![5, 6]);
    assert_eq!(provenance(&slate, 5), vec![("clusters".to_string(), Some(300 * ONE_UNIT))]);
    assert_eq!(insides(&slate)[3], vec![3, 4], "the lateral variant");
    assert_eq!(
        provenance(&slate, 4),
        vec![("lateral".to_string(), None), ("clusters".to_string(), Some(250 * ONE_UNIT))],
        "the 250 pair is strong under the locked median of 250",
    );
    assert_eq!(slate.omitted, [0, 0, 0, 0]);
    // And the loop constitutes no cluster of its own: no candidate stands on
    // Node 5 alone, however much its own Route carries.
    assert!(slate.candidates.iter().all(|held| held.view.inside != vec![5]));
}

#[test]
fn a_members_self_loop_counts_once_in_its_internal_attachment() {
    // `A_in(n)` sums Routes with both endpoints in the inside that touch n. A
    // member's self-loop has both endpoints inside and touches its one Node,
    // and contributes its window flow exactly once — the locked sentence. The
    // fixture is built so once and twice disagree: counted once, Node 2
    // carries 100 + 100 = 200 and is the weakest member, so the finer inside
    // is [3, 4]; counted twice it would carry 300, Node 4's 250 would be the
    // weakest, and the finer inside would read [2, 3].
    let mut field = spread_field();
    field.routes = vec![
        route(1, 2, 3),
        route(2, 3, 4),
        RouteState {
            route: 3,
            tail: 2,
            head: 2,
            capacity: 8 * ONE_UNIT,
            flow: 0,
            formed_step: 0,
        },
    ];
    let mut steps: Vec<TraceStep> = recorded_steps();
    for step in &mut steps {
        step.records.f.clear();
        step.records.q.clear();
        step.records.z.clear();
    }
    steps[0].records.f =
        vec![(1, 100 * ONE_UNIT), (2, 250 * ONE_UNIT), (3, 100 * ONE_UNIT)];
    let slate = slate::assemble(&state_of(field, view_of(&[2, 3, 4]), steps));

    assert_eq!(insides(&slate)[1], vec![3, 4], "the weakest member counted its loop once");
    // Nothing reaches outside this inside, so the other two variants record
    // their failed conditions — and the 250 cluster equals the finer inside,
    // merging by the duplicate rule.
    let conditions: Vec<(String, String)> = slate
        .absent
        .iter()
        .map(|held| (held.variant.name().to_string(), held.condition.clone()))
        .collect();
    assert_eq!(
        conditions,
        vec![
            ("coarser".to_string(), "C1 = I0".to_string()),
            ("lateral".to_string(), "k' = 0".to_string()),
        ],
    );
    assert_eq!(
        provenance(&slate, 2),
        vec![("finer".to_string(), None), ("clusters".to_string(), Some(250 * ONE_UNIT))],
    );
    assert_eq!(slate.candidates.len(), 2, "two candidates, and the slate is not deficient");
    assert!(!slate.deficient);
}

#[test]
fn the_repeat_threshold_climbs_with_the_window() {
    // Twenty-five recorded steps are twenty-four response steps, so
    // `theta_r = max(2, ceil(24 / 8))` takes the ceiling branch and reads 3 —
    // the short-window fixtures elsewhere read the floor of 2. Nodes 2 and 3
    // step up together three times and Nodes 4 and 5 twice: two co-responses
    // reach the short window's threshold and not this one.
    let mut field = spread_field();
    field.boundaries.drawn.clear();
    field.boundaries.authored.clear();
    let rises: [(u32, &[Step]); 4] =
        [(2, &[5, 10, 15]), (3, &[5, 10, 15]), (4, &[7, 12]), (5, &[7, 12])];
    let steps: Vec<TraceStep> = (1..=25)
        .map(|step| {
            let mut records = StepRecords::default();
            for (node, risen) in rises {
                let held = risen.iter().filter(|at| **at <= step).count() as i64;
                if held > 0 {
                    records.q.push((node, held * SWING));
                }
            }
            TraceStep {
                step,
                rng: RngState { key: 0, ctr: 0, half: 0 },
                ctl: ControlState::default(),
                records,
            }
        })
        .collect();
    let mut state = state_of(field, view_of(&[2, 3, 4, 5]), steps);
    // Ordinal 3 leads with source 4, and nothing else has anything to offer.
    state.now.assembly_ordinal = 3;
    state.now.prev_assembly_step = Some(26);
    let slate = slate::assemble(&state);

    assert_eq!(slate.window_effective, 25);
    let seat = seat_of(&slate, Source::Responses).expect("the three-count pair is a component");
    assert_eq!(slate.candidates[seat].view.inside, vec![2, 3]);
    assert_eq!(provenance(&slate, seat + 1), vec![("responses".to_string(), Some(3))]);
    // The two-count pair reaches no edge under the raised threshold: no
    // candidate stands on {4, 5}, seated or merged.
    assert!(slate.candidates.iter().all(|held| held.view.inside != vec![4, 5]));
    assert!(slate
        .candidates
        .iter()
        .all(|held| held.provenance.iter().filter(|p| p.source == Source::Responses).count() <= 1));
}

// ---------------------------------------------------------------------------
// Intake, the fallback, and the duplicate rule
// ---------------------------------------------------------------------------

#[test]
fn intake_drops_vanished_members_and_discards_a_proposal_it_empties() {
    let mut field = spread_field();
    // A drawn entry naming Nodes that have gone, one that names some of them,
    // and an authored entry naming nothing that stands.
    field.boundaries.drawn = vec![
        DrawnBoundary { members: vec![40, 41], step: 4 },
        DrawnBoundary { members: vec![5, 6, 42], step: 4 },
    ];
    field.boundaries.authored = vec![vec![80, 81]];
    let slate = slate::assemble(&state_of(field, view_of(&[2, 3]), recorded_steps()));

    // The proposal intake emptied is discarded with its reason, and the one it
    // only narrowed is offered as the intersection.
    let discarded: Vec<(String, Option<i64>, String)> = slate
        .discarded
        .iter()
        .map(|held| (held.source.name().to_string(), held.detail, held.reason.clone()))
        .collect();
    assert_eq!(
        discarded,
        vec![
            ("drawn".to_string(), Some(1), "intake-empty".to_string()),
            ("authored".to_string(), Some(1), "intake-empty".to_string()),
        ],
    );
    assert_eq!(insides(&slate)[4], vec![5, 6], "the intersection with the Node set, ascending");
    assert_eq!(provenance(&slate, 5), vec![("drawn".to_string(), Some(2))]);
}

#[test]
fn the_standing_inside_passes_intake_and_falls_back_when_it_vanishes() {
    // A standing inside that has lost members is reduced, and the reduction is
    // noted.
    let reduced = slate::assemble(&state_of(spread_field(), view_of(&[2, 3, 44]), recorded_steps()));
    assert_eq!(insides(&reduced)[0], vec![2, 3]);
    assert_eq!(reduced.standing_removed, 1);
    assert!(!reduced.standing_fallback);
    assert_eq!(reduced.standing_reason, None);

    // A standing inside that has vanished is replaced by `<N, rho0, w0, u0>`
    // before anything else, with the recorded reason.
    let gone = slate::assemble(&state_of(spread_field(), view_of(&[44, 45]), recorded_steps()));
    assert_eq!(insides(&gone)[0], vec![1, 2, 3, 4, 5, 6, 7, 8], "the whole of N");
    assert_eq!(gone.standing_removed, 2);
    assert!(gone.standing_fallback);
    assert_eq!(gone.standing_reason.as_deref(), Some("standing-inside-vanished"));
    assert_eq!(provenance(&gone, 1), vec![("standing".to_string(), None)]);
    assert_eq!(gone.candidates[0].view.resolution, 1, "and keeps the other three components");
    assert_eq!(gone.candidates[0].view.window, 45);
}

#[test]
fn one_inside_offered_by_two_sources_takes_one_seat_with_both_provenances() {
    // The drawn list and the authored list both propose {5, 6}: one offer,
    // one seat, and the entry carries why it exists twice over. The later
    // source takes no second turn in the cycle, and nothing is counted
    // omitted for it — an omission is counted once per offer, for the source
    // that offered it first.
    let mut field = spread_field();
    field.boundaries.drawn = vec![DrawnBoundary { members: vec![5, 6], step: 4 }];
    field.boundaries.authored = vec![vec![5, 6]];
    let slate = slate::assemble(&state_of(field, view_of(&[2, 3]), recorded_steps()));

    let seat = seat_of(&slate, Source::Drawn).expect("the shared inside is seated");
    assert_eq!(slate.candidates[seat].view.inside, vec![5, 6]);
    assert_eq!(
        provenance(&slate, seat + 1),
        vec![("drawn".to_string(), Some(1)), ("authored".to_string(), Some(1))],
        "one candidate, both reasons",
    );
    assert_eq!(seat_of(&slate, Source::Authored), Some(seat), "one seat, not two");
    assert_eq!(
        slate.candidates.iter().filter(|held| held.view.inside == vec![5, 6]).count(),
        1,
    );
    // The slate stood full when the cluster and response sources rotated in,
    // so each counts one omission — and the merged authored offer counts none.
    assert_eq!(slate.omitted, [0, 1, 0, 1], "drawn, clusters, authored, responses");
}

#[test]
fn a_view_already_in_the_slate_takes_the_new_provenance_and_no_new_seat() {
    // The authored boundary is the standing inside itself, so the duplicate
    // rule merges rather than adding — and the seat it would have taken stays
    // open for the next source.
    let mut field = spread_field();
    field.boundaries.authored = vec![vec![2, 3]];
    let slate = slate::assemble(&state_of(field, view_of(&[2, 3]), recorded_steps()));
    assert_eq!(
        provenance(&slate, 1),
        vec![("standing".to_string(), None), ("authored".to_string(), Some(1))],
    );
    assert_eq!(seat_of(&slate, Source::Authored), Some(0));
    assert_eq!(slate.omitted[2], 0, "a merged offer is not an omitted one");
}

// ---------------------------------------------------------------------------
// The slate's own invariants
// ---------------------------------------------------------------------------

#[test]
fn every_slate_holds_two_to_five_candidates_unless_it_is_deficient() {
    let mut states = vec![
        fixture(),
        state_of(spread_field(), view_of(&[3]), recorded_steps()),
        state_of(spread_field(), view_of(&[1, 2, 3, 4, 5, 6, 7, 8]), recorded_steps()),
        state_of(spread_field(), view_of(&[44]), recorded_steps()),
        state_of(spread_field(), view_of(&[2, 3]), Vec::new()),
    ];
    for ordinal in 0..4u32 {
        let mut held = fixture();
        held.now.assembly_ordinal = ordinal;
        held.now.prev_assembly_step = Some(6);
        states.push(held);
    }
    for state in &states {
        let slate = slate::assemble(state);
        assert!(
            (2..=SLATE_CAP).contains(&slate.candidates.len()),
            "a slate holds two to five candidates, not {}",
            slate.candidates.len(),
        );
        assert!(!slate.deficient);
        // Every entry is a distinct View, and every one records why it exists.
        for (place, candidate) in slate.candidates.iter().enumerate() {
            assert!(!candidate.provenance.is_empty(), "position {}", place + 1);
            assert!(!candidate.view.inside.is_empty());
        }
        let mut insides = insides(&slate);
        insides.sort();
        let held = insides.len();
        insides.dedup();
        assert_eq!(insides.len(), held, "no two entries declare the same View");
    }
}

#[test]
fn a_slate_that_ends_with_one_entry_is_deficient_and_names_its_reason() {
    // A Field with one Node in the inside, nothing attached to it, and no
    // stored boundary to propose: the standing View takes seat 1, all three
    // variants fail their conditions, and no source has anything to offer.
    let mut field = spread_field();
    field.routes.clear();
    field.boundaries.drawn.clear();
    field.boundaries.authored.clear();
    let slate = slate::assemble(&state_of(field, view_of(&[5]), recorded_steps()));

    assert_eq!(slate.candidates.len(), 1);
    assert!(slate.deficient);
    assert_eq!(slate.deficiency_reason.as_deref(), Some("no-alternative-candidate"));
    assert_eq!(slate.absent.len(), 3, "all three variants failed");
}

#[test]
fn a_window_the_clamp_left_empty_yields_no_windowed_source() {
    // Right after a commit the retained span is 0, so the effective window is
    // 0: the sources that read the window — the clusters' flows and the shared
    // responses — yield nothing, and the slate is what the Field alone
    // supports. That is the degenerate reading the clamp names, not a fault.
    let slate = slate::assemble(&state_of(spread_field(), view_of(&[2, 3]), Vec::new()));
    assert_eq!(slate.window_effective, 0);
    assert_eq!(seat_of(&slate, Source::Clusters), None);
    assert_eq!(seat_of(&slate, Source::Responses), None);
    // The variants that read no flow still stand: `C1` is a Route-set reading.
    assert_eq!(insides(&slate)[2], vec![2, 3, 4, 8], "coarser");
    // And the lateral variant is out, because no non-member carries.
    assert!(slate.absent.iter().any(|held| held.variant == Source::Lateral));
}

#[test]
fn assembly_is_deterministic_and_the_record_round_trips() {
    let state = fixture();
    let first = slate::assemble(&state).written();
    let second = slate::assemble(&state).written();
    assert_eq!(first, second, "the same inputs assemble the same slate");

    // The record goes into the payload and comes back out of it byte for byte.
    let mut carried = fixture();
    carried.slate = Some(slate::assemble(&state));
    let payload = carried.payload();
    let parsed = parse(&payload).expect("the payload parses");
    let read = RunState::read(&parsed).expect("and reads back");
    assert_eq!(read.payload(), payload, "the slate rides the payload");
    assert_eq!(read.slate.expect("a slate stands").written(), first);

    // And the whole export file round-trips through the bridge.
    let file = carried.export_file();
    let mut session = Session::new(&support::worker_init()).expect("versions agree");
    let mut import = String::from("{\"text\":");
    write_text(&mut import, &file);
    import.push('}');
    let answer = session.command("import_run", &import);
    assert!(answer.contains("\"ok\":true"), "{answer}");
    let held = session.run().expect("a run is loaded").standing_slate().expect("a slate");
    assert_eq!(held.written(), first);
}

#[test]
fn the_record_carries_the_shape_the_document_locks() {
    let slate = slate::assemble(&fixture());
    let written = slate.written();
    let parsed = parse(&written).expect("the record parses");
    let Json::Map(pairs) = &parsed else { panic!("the record is an object") };
    let keys: Vec<&str> = pairs.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(
        keys,
        vec![
            "absent",
            "candidates",
            "deficiency_reason",
            "deficient",
            "detail",
            "discarded",
            "dominance",
            "forecast",
            "omitted",
            "ordinal",
            "sensitivity",
            "sigma",
            "standing_intake",
            "step",
            "tau",
            "window_declared",
            "window_effective",
        ],
    );
    // Assembly names the candidates and reads nothing: the four values stand
    // unassigned, the tier is 0 — the number no tier has — and the relation is
    // empty, until `rank::evaluate` runs over the same record. Nothing in the
    // record is a combined figure, at either stage.
    assert!(written.contains("\"tier\":0"));
    assert!(written.contains("\"dominance\":[]"));
    assert!(written.contains("\"reason\":\"window-too-short\""));
    assert!(written.contains("\"deviations\":[null,null,null,null,null,null,null,null]"));
    assert!(written.contains("\"changed_at\":[],\"flag\":false"));
    // The reserved slot for the budget-overrun contract, null until that
    // goal lands, so the record's key set never moves under it.
    assert!(written.contains("\"detail\":null,\"discarded\""));
}

// ---------------------------------------------------------------------------
// Through the bridge: assemble, adopt, commit
// ---------------------------------------------------------------------------

fn body(answer: &str) -> Json {
    let parsed = parse(answer).expect("the answer parses");
    assert_eq!(parsed.get("ok"), Some(&Json::Bool(true)), "{answer}");
    parsed.get("body").expect("a body").clone()
}

/// The events one command raised, as (name, body).
fn events(session: &mut Session) -> Vec<(String, Json)> {
    let raised = session.take_events();
    let Json::List(items) = parse(&raised).expect("canonical events") else {
        panic!("the events are a list: {raised}");
    };
    items
        .into_iter()
        .map(|item| {
            let name = item.get("ev").and_then(Json::as_text).unwrap_or("").to_string();
            (name, item.get("body").cloned().unwrap_or(Json::Null))
        })
        .collect()
}

/// The slate records the `review_ready` events of one command carried.
fn reviews(session: &mut Session) -> Vec<Json> {
    events(session)
        .into_iter()
        .filter(|(name, _)| name == "review_ready")
        .map(|(_, body)| body.get("review").cloned().expect("a review"))
        .collect()
}

fn frame(seq: u32, steps: u16, toggle: bool, t_us: i64) -> String {
    format!(
        "{{\"advance_steps\":{steps},\"depth_key\":0,\"inspect\":null,\"pause\":false,\
          \"pulse_held\":false,\"pulse_release\":false,\"seq\":{seq},\"steer_x\":0,\
          \"steer_y\":0,\"t_us\":{t_us},\"toggle_still\":{toggle},\"wheel\":0}}"
    )
}

/// Opens a session on a state rewound to step 0, plays forty steps, and
/// enters Still Mode — the play-reachable way into a standing slate.
fn played_into_still(mut state: RunState) -> Session {
    state.now.step = 0;
    // The Field opens at step 0, so any drag it carries was recorded there.
    for entry in &mut state.now.boundaries.drawn {
        entry.step = 0;
    }
    state.trace = Trace::opening(state.now.clone());
    let mut session = imported(&state.export_file());
    session.command("input_frame", &frame(1, 40, false, 1_000_000));
    session.command("input_frame", &frame(2, 0, true, 1_100_000));
    session.command("input_frame", &frame(3, 0, false, 1_100_000 + RAMP_US));
    assert_eq!(session.lifecycle(), "still");
    session
}

/// A fresh session opened on an export file.
fn imported(file: &str) -> Session {
    let mut session = Session::new(&support::worker_init()).expect("versions agree");
    let mut import = String::from("{\"text\":");
    write_text(&mut import, file);
    import.push('}');
    let answer = session.command("import_run", &import);
    assert!(answer.contains("\"ok\":true"), "{answer}");
    session
}

/// The export file a session writes.
fn exported(session: &mut Session) -> String {
    let answer = body(&session.command("export_run", "{}"));
    answer.get("text").and_then(Json::as_text).expect("the file").to_string()
}

/// Enters Still Mode on a freshly imported session and hands back the slate
/// the entry assembled.
fn reentered(file: &str) -> CandidateSlate {
    let mut session = imported(file);
    session.command("input_frame", &frame(1, 0, true, 1_000_000));
    session.command("input_frame", &frame(2, 0, false, 1_000_000 + RAMP_US));
    assert_eq!(session.lifecycle(), "still");
    session.run().expect("a run is loaded").standing_slate().expect("a slate stands").clone()
}

/// A session standing on the fixture Field, entered into Still Mode.
fn stilled() -> Session {
    played_into_still(fixture())
}

#[test]
fn entering_still_raises_the_slate_and_adopting_a_candidate_commits_it() {
    let mut session = stilled();

    // The entry ramp reaching scale 0 assembles the Run's first slate and
    // raises it as the review the shell reads.
    let raised = reviews(&mut session);
    assert_eq!(raised.len(), 1, "one review, on the frame the ramp completed");
    assert_eq!(raised[0].get("kind").and_then(Json::as_text), Some("slate"));
    let record = raised[0].get("slate").expect("the record");
    assert_eq!(record.get("ordinal").and_then(Json::as_int), Some(0));

    let standing = {
        let run = session.run().expect("a run is loaded");
        run.standing_slate().expect("a slate stands").clone()
    };
    assert!(standing.candidates.len() >= 2, "a slate to adopt from");
    let adopted = standing.candidates[1].view.clone();
    assert_ne!(adopted.inside, standing.candidates[0].view.inside);

    // Adoption is a queued change like any other, and the commit installs the
    // candidate's whole View.
    let queued = session.command(
        "queue_plan",
        "{\"plan\":{\"op\":\"set_focus\",\"position\":2,\"slate_ordinal\":0}}",
    );
    assert!(queued.contains("\"ok\":true"), "{queued}");
    let committed = body(&session.command("commit_plan", "{}"));
    assert_eq!(committed.get("applied").and_then(Json::as_int), Some(1));

    let run = session.run().expect("a run is loaded");
    assert_eq!(run.state().view.inside, adopted.inside, "the standing View is the candidate's");
    assert_eq!(run.state().retained_span(), 0, "the commit ended the active window");
    assert_eq!(run.state().effective_window(45), 0, "so the clamp reads nothing");

    // The commit reassembled the slate under that clamp: a new ordinal, the
    // adopted inside in seat 1, and an empty window.
    let reassembled = run.standing_slate().expect("a slate stands");
    assert_eq!(reassembled.ordinal, 1);
    assert_eq!(reassembled.step, run.state().now.step);
    assert_eq!(reassembled.window_effective, 0);
    assert_eq!(reassembled.candidates[0].view.inside, adopted.inside);
    assert_eq!(committed.get("slate_ordinal").and_then(Json::as_int), Some(1));
}

#[test]
fn the_commit_raises_the_reassembled_slate_after_its_own_answer() {
    let mut session = stilled();
    reviews(&mut session);
    session.command("queue_plan", "{\"plan\":{\"op\":\"cut\",\"route\":1}}");
    session.command("commit_plan", "{}");
    let raised = reviews(&mut session);
    assert_eq!(raised.len(), 1, "a committed change reassembles the slate");
    assert_eq!(
        raised[0].get("slate").and_then(|held| held.get("ordinal")).and_then(Json::as_int),
        Some(1),
    );
}

#[test]
fn an_empty_commit_reassembles_nothing_and_names_the_slate_that_stands() {
    let mut session = stilled();
    reviews(&mut session);
    let answer = body(&session.command("commit_plan", "{}"));
    assert_eq!(answer.get("applied").and_then(Json::as_int), Some(0));
    assert_eq!(
        answer.get("slate_ordinal").and_then(Json::as_int),
        Some(0),
        "the slate the run stands under",
    );
    assert!(reviews(&mut session).is_empty(), "an empty commit changes nothing to reassemble");
}

#[test]
fn a_run_that_assembled_and_adopted_replays_byte_for_byte() {
    let mut session = stilled();
    session.command(
        "queue_plan",
        "{\"plan\":{\"op\":\"set_focus\",\"position\":2,\"slate_ordinal\":0}}",
    );
    session.command("commit_plan", "{}");

    let first = body(&session.command("export_run", "{}"));
    let file = first.get("text").and_then(Json::as_text).expect("the file").to_string();
    let mut import = String::from("{\"text\":");
    write_text(&mut import, &file);
    import.push('}');

    let mut fresh = Session::new(&support::worker_init()).expect("versions agree");
    assert!(fresh.command("import_run", &import).contains("\"ok\":true"));
    let second = body(&fresh.command("export_run", "{}"));
    assert_eq!(
        first.get("text").and_then(Json::as_text),
        second.get("text").and_then(Json::as_text),
        "a run carrying a slate restores to itself",
    );
}

// ---------------------------------------------------------------------------
// The budget
// ---------------------------------------------------------------------------

/// A Field at every cap the assembly's passes are bounded by: 256 Nodes, 512
/// Routes, a 60-step window, and the retained count of drawn entries.
fn at_every_cap() -> RunState {
    let mut field = FieldState::opening();
    field.next_node_id = NODES_PER_RUN as u32 + 1;
    field.next_route_id = ROUTES_PER_RUN as u32 + 1;
    field.layers = vec![FieldLayer {
        layer: 0,
        drain: 0,
        noise: 0,
        gain: 0,
        current_ids: Vec::new(),
        port_ids: (2..=NODES_PER_RUN as u32).collect(),
    }];
    field.ports = (1..=NODES_PER_RUN as u32)
        .map(|node| {
            let kind = if node == 1 { NodeKind::Form } else { NodeKind::Port };
            // Sixteen to a row, 100 units apart, so every Node has neighbours
            // inside the adjacency radius and the tables are dense.
            port(node, kind, 100 * i64::from(node % 16), 100 * i64::from(node / 16))
        })
        .collect();
    field.routes = (1..=ROUTES_PER_RUN as u32)
        .map(|id| {
            let tail = 1 + (id % NODES_PER_RUN as u32);
            let head = 1 + ((id * 7) % NODES_PER_RUN as u32);
            route(id, tail, head.max(1))
        })
        .collect();
    field.forms = spread_field().forms;
    field.boundaries.drawn = (0..32)
        .map(|place| DrawnBoundary {
            members: (2..=NODES_PER_RUN as u32).step_by(2).collect(),
            step: 60 - place,
        })
        .collect();
    field.boundaries.authored = vec![(2..=64).collect()];
    field.step = 60;

    let steps: Vec<TraceStep> = (1..=60)
        .map(|step| {
            let mut records = StepRecords::default();
            records.f = (1..=ROUTES_PER_RUN as u32)
                .map(|id| (id, i64::from(id % 7) * ONE_UNIT))
                .collect();
            records.q = (1..=NODES_PER_RUN as u32)
                .map(|node| (node, i64::from((node + step) % 5) * ONE_UNIT))
                .collect();
            records.z = (1..=NODES_PER_RUN as u32).filter(|node| (node + step) % 3 == 0).collect();
            TraceStep {
                step,
                rng: RngState { key: 0, ctr: 0, half: 0 },
                ctl: ControlState::default(),
                records,
            }
        })
        .collect();

    let view = ViewDeclaration {
        inside: (2..=128).collect(),
        resolution: 1,
        window: 60,
        surround: Surround::Double,
    };
    state_of(field, view, steps)
}

#[test]
fn assembly_at_every_cap_stays_inside_its_share_of_the_budget() {
    let state = at_every_cap();
    let slate = slate::assemble(&state);
    assert_eq!(slate.window_effective, 60, "the widest declared window");
    assert_eq!(slate.candidates.len(), SLATE_CAP, "and a full slate");

    // The measurement is reported rather than pinned tightly: the budget is a
    // performance target verified in the goal that owns it, and this test run
    // is an unoptimized build. What is asserted is the order of magnitude the
    // deterministic ceiling allows, so a pass that turned quadratic in the
    // window or cubic in the Node count would fail here rather than in a
    // browser.
    let started = std::time::Instant::now();
    let rounds = 10;
    for _ in 0..rounds {
        let held = slate::assemble(&state);
        assert_eq!(held.candidates.len(), SLATE_CAP);
    }
    let each = started.elapsed() / rounds;
    println!("assembly at every cap: {each:?} per assembly");
    // The unoptimized run measures about 15 ms and the release build about
    // 1 ms, against the 15 ms deterministic ceiling the budget leaves for the
    // assembly sources and the record beside them.
    assert!(
        each < std::time::Duration::from_millis(50),
        "assembly took {each:?}, past the share of the budget it is bounded by",
    );
}

/// The step a synthetic trace records. Kept beside the fixture so a reader can
/// see that nothing here draws.
#[allow(dead_code)]
fn recorded_step_count(state: &RunState) -> Step {
    state.now.step
}

#[test]
fn a_commit_reassembles_under_a_span_of_nothing_and_the_values_return_with_it() {
    // The window clamp, through the Goal 14 seam and out the other side. A
    // commit that applies a change ends the active window and restarts the
    // retained trajectory on the state it leaves, so the reassembly it runs
    // reads an effective window of 0 — and FRAMEWORK.md says every windowed
    // procedure is then unassigned. That is the honest reading of a Field
    // nothing has been observed of yet, and it is not a fault.
    let mut session = dragged_and_committed();
    let run = session.run().expect("a run is loaded");
    let after = run.standing_slate().expect("the commit reassembled").clone();
    assert_eq!(after.window_effective, 0, "the commit clamped the span to nothing");
    for candidate in &after.candidates {
        for value in candidate.privilege.each() {
            assert_eq!(value.value, None, "no number");
            assert_eq!((value.low, value.high), (None, None), "and no range");
            assert_eq!(value.reason, Some("window-too-short"), "with the stated reason");
        }
        assert_eq!(
            candidate.baseline,
            [None; slate::BASELINE_SAMPLES],
            "and no baseline replay ran",
        );
    }
    assert!(after.dominance.is_empty(), "nothing is comparable when nothing is assigned");
    assert!(!after.sensitivity, "and no tolerance decides a comparison that did not run");

    // The span regrows a step at a time, and the values come back with it: play
    // the run on and enter Still Mode again.
    session.command("input_frame", &frame(4, 0, false, 1_100_000 + 2 * RAMP_US));
    assert_eq!(session.lifecycle(), "running", "the committed exit completed");
    session.command("input_frame", &frame(5, 30, false, 2_000_000));
    session.command("input_frame", &frame(6, 0, true, 2_100_000));
    session.command("input_frame", &frame(7, 0, false, 2_100_000 + RAMP_US));
    assert_eq!(session.lifecycle(), "still");
    let back = session
        .run()
        .expect("a run is loaded")
        .standing_slate()
        .expect("the re-entry assembled")
        .clone();
    assert_eq!(back.window_effective, 30, "the span the run has stood through since");
    assert!(
        back.candidates
            .iter()
            .any(|held| held.privilege.each().iter().any(|value| value.is_assigned())),
        "and a value the window can carry is read again",
    );
    assert!(
        back.candidates.iter().all(|held| held.tier >= 1),
        "every candidate of a compared slate stands in a tier",
    );
}
