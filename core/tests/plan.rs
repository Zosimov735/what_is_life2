//! Transactional PlanCommands: the four causal variants, the queue that holds them,
//! and the commit that spends them.
//!
//! `docs/field-framework/ARCHITECTURE.md` locks the union field by field — the
//! payload of each variant, the preconditions each is validated against, the
//! cost of one entry, the queue's depth, the conflict rules, and the atomic
//! all-or-nothing commit. What is under test here is exactly that table, read
//! as a table, beside the three rules the goal that implements it had to lock:
//! where the Impulse lives, what a commit does to the active window, and when a
//! formed Route first carries.
//!
//! The Field most of these run on is a small one built here rather than the
//! authored chapter, and deliberately: the authored chapter's Ports stand
//! further apart than a Thread's `route_reach`, so no `connect` between two of
//! them can validate there at all. A Field whose Nodes are inside each other's
//! reach is what makes the preconditions readable one at a time.

use field_game_core::field::{
    BoundaryState, FieldLayer, FormState, NodeKind, PhysicalCompartment, PortState, RouteState,
};
use field_game_core::fx::{Vec2, ONE_UNIT};
use field_game_core::json::{parse, write_text, Json};
use field_game_core::plan::{
    self, PlanCommand, PlanQueue, Projection, RouteEnd, CONNECTED_ROUTE_CAPACITY, PLAN_QUEUE_DEPTH,
};
use field_game_core::state::{
    FieldState, GeneratorSpec, InputConfig, Progress, RunState, Surround, Trace, ViewDeclaration,
    IMPULSE_CAP, OPENING_IMPULSE,
};
use field_game_core::Session;

mod support;

const KEY: &str = "0123456789abcdef";

/// A content hash no build ever computed, so a run opened on this state runs no
/// authored sequence: the Field under test is the test's own, and an objective
/// script reading it would be reading a Field it was not authored for.
const NO_CONTENT: &str = "00000000000000000000000000000000000000000000000000000000000000ff";

/// One rendered frame at the 60-frames-per-second target, in microseconds.
const FRAME_US: i64 = 16_667;

/// How long a ramp takes, in microseconds. Locked.
const RAMP_US: i64 = 250_000;

// ---------------------------------------------------------------------------
// The Field these run on
// ---------------------------------------------------------------------------

/// A Field whose Nodes stand inside one Thread's reach of each other: one
/// layer, a controlled Form on Node 1, four Ports at 2 through 5, and two
/// Routes — 1: 2 → 3 and 2: 3 → 4.
///
/// Every Port is open, so a Route between any two of them carries the moment it
/// stands: the `open` gate is the participation rule, and a formed Route that
/// could not carry would say nothing about when it first does.
fn linked_field() -> FieldState {
    let mut field = FieldState::opening();
    field.next_node_id = 6;
    field.next_route_id = 3;
    field.layers = vec![FieldLayer {
        layer: 0,
        drain: 0,
        noise: 0,
        gain: 0,
        current_ids: Vec::new(),
        port_ids: vec![2, 3, 4, 5],
    }];
    field.ports = vec![
        // The Form's own Node, which mirrors the Form.
        port(1, NodeKind::Form, 1000, 1000, 8 * ONE_UNIT),
        port(2, NodeKind::Port, 1100, 1000, 64 * ONE_UNIT),
        port(3, NodeKind::Port, 1200, 1000, 0),
        port(4, NodeKind::Port, 1300, 1000, 0),
        // Far enough from every other Node to stand outside a Thread's reach.
        port(5, NodeKind::Port, 2000, 2000, 0),
    ];
    field.routes = vec![
        RouteState { route: 1, tail: 2, head: 3, capacity: 8 * ONE_UNIT, flow: 0, formed_step: 0 },
        RouteState { route: 2, tail: 3, head: 4, capacity: 8 * ONE_UNIT, flow: 0, formed_step: 0 },
    ];
    field.forms = vec![FormState {
        id: 1,
        form: "thread".to_string(),
        node: 1,
        controlled: true,
        layer: 0,
        pos: Vec2::units(1000, 1000),
        vel: Vec2 { x: 0, y: 0 },
        charge: 8 * ONE_UNIT,
        reserve: 0,
        pulse_charge: 0,
        focus: false,
        // 256 units, the authored Thread's own reach.
        route_reach: 256 * ONE_UNIT,
        forecast_depth: 0,
        steer_scale: field_game_core::state::FRAC_ONE,
        route_capacity: 32 * ONE_UNIT,
        link: None,
        trail: None,
    }];
    field.physical_compartment = PhysicalCompartment {
        members: vec![2, 3],
        leak_per_exposed_contact_per_step: 0,
    };
    field.boundaries = BoundaryState { drawn: Vec::new(), authored: Vec::new() };
    field
}

fn port(node: u32, kind: NodeKind, x: i64, y: i64, q: i64) -> PortState {
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

fn linked_view() -> ViewDeclaration {
    ViewDeclaration { inside: vec![2, 3], resolution: 1, window: 45, surround: Surround::Adjacent }
}

/// A run standing on that Field, carrying the Impulse the caller names.
///
/// The state is built here rather than played into, because the Impulse a run
/// carries is what the cost rules are read against and a run has to have been
/// somewhere to carry six of it.
fn state_with(impulse: u8) -> RunState {
    let field = linked_field();
    let mut progress = Progress::opening();
    progress.impulse = impulse;
    RunState {
        run_id: KEY.to_string(),
        rng: field_game_core::rng::trajectory_stream(KEY, 0),
        spec: GeneratorSpec::new(NO_CONTENT.to_string(), Default::default()),
        branch_nonce: 0,
        progress,
        trace: Trace::opening(field.clone()),
        now: field,
        view: linked_view(),
        slate: None,
        input_config: InputConfig::default_config(),
        pressures: Vec::new(),
        anchors: Vec::new(),
    }
}

/// A session standing on that state, in `still`, ready to take entries.
///
/// The state reaches the session the one way the closed command set admits:
/// `import_run` over the export file the state writes, which is exactly how the
/// local preview opens a Field of its own.
fn stilled(impulse: u8) -> (Session, u32) {
    let mut session = Session::new(&support::worker_init()).expect("versions agree");
    let file = state_with(impulse).export_file();
    let mut body = String::from("{\"text\":");
    write_text(&mut body, &file);
    body.push('}');
    let answer = session.command("import_run", &body);
    assert!(answer.contains("\"ok\":true"), "the state imports: {answer}");
    session.command("input_frame", &toggling_at(1, 0, 1_000_000));
    session.command("input_frame", &at(2, 0, 1_000_000 + RAMP_US));
    assert_eq!(session.lifecycle(), "still");
    (session, 3)
}

// ---------------------------------------------------------------------------
// Frames and answers
// ---------------------------------------------------------------------------

fn at(seq: u32, steps: u16, t_us: i64) -> String {
    built(seq, steps, false, t_us)
}

fn toggling_at(seq: u32, steps: u16, t_us: i64) -> String {
    built(seq, steps, true, t_us)
}

fn built(seq: u32, steps: u16, toggle_still: bool, t_us: i64) -> String {
    format!(
        "{{\"advance_steps\":{steps},\"depth_key\":0,\"inspect\":null,\"pause\":false,\
          \"pulse_held\":false,\"pulse_release\":false,\"seq\":{seq},\"steer_x\":0,\
          \"steer_y\":0,\"t_us\":{t_us},\"toggle_still\":{toggle_still},\"wheel\":0}}"
    )
}

fn body(answer: &str) -> Json {
    let parsed = parse(answer).expect("a response is canonical JSON");
    assert_eq!(parsed.get("ok"), Some(&Json::Bool(true)), "{answer}");
    parsed.get("body").expect("a successful response carries a body").clone()
}

/// The code and the reason a refusal names.
fn refusal(answer: &str) -> (String, String) {
    let parsed = parse(answer).expect("a response is canonical JSON");
    assert_eq!(parsed.get("ok"), Some(&Json::Bool(false)), "{answer}");
    let error = parsed.get("error").expect("a refusal carries an envelope");
    let code = error.get("code").and_then(Json::as_text).expect("a code").to_string();
    let reason = error
        .get("detail")
        .and_then(|detail| detail.get("reason"))
        .and_then(Json::as_text)
        .unwrap_or_default()
        .to_string();
    (code, reason)
}

fn int_of(value: &Json, key: &str) -> i64 {
    value.get(key).and_then(Json::as_int).expect("an integer field")
}

/// Queues one entry, as the shell's own drag would.
fn queue(session: &mut Session, plan: &str) -> String {
    session.command("queue_plan", &format!("{{\"plan\":{plan}}}"))
}

/// The queue state a response carries.
fn queue_state(answer: &str) -> Json {
    body(answer).get("queue").expect("a queued-change answer carries the queue").clone()
}

fn entries(queue: &Json) -> Vec<Json> {
    match queue.get("entries") {
        Some(Json::List(held)) => held.clone(),
        _ => panic!("the queue state carries a list of entries"),
    }
}

/// The routes of the loaded run, as (route, tail, head).
fn routes(session: &Session) -> Vec<(u32, u32, u32)> {
    session
        .run()
        .expect("a run is loaded")
        .state()
        .now
        .routes
        .iter()
        .map(|route| (route.route, route.tail, route.head))
        .collect()
}

fn inside(session: &Session) -> Vec<u32> {
    session.run().expect("a run is loaded").state().view.inside.clone()
}

fn physical_members(session: &Session) -> Vec<u32> {
    session
        .run()
        .expect("a run is loaded")
        .state()
        .now
        .physical_compartment
        .members
        .clone()
}

fn payload(session: &Session) -> String {
    session.run().expect("a run is loaded").payload().expect("inside the cap")
}

// ---------------------------------------------------------------------------
// The union, read out of a body
// ---------------------------------------------------------------------------

#[test]
fn every_variant_reads_its_own_payload_and_nothing_else() {
    let read = |text: &str| PlanCommand::read(&parse(text).expect("canonical"));

    assert_eq!(
        read("{\"from\":2,\"op\":\"connect\",\"to\":3}").expect("connect reads"),
        PlanCommand::Connect { from: 2, to: 3 },
    );
    assert_eq!(
        read("{\"end\":\"head\",\"op\":\"redirect\",\"route\":1,\"to\":4}").expect("redirect reads"),
        PlanCommand::Redirect { route: 1, end: RouteEnd::Head, to: 4 },
    );
    assert_eq!(read("{\"op\":\"cut\",\"route\":2}").expect("cut reads"), PlanCommand::Cut { route: 2 });
    assert_eq!(
        read("{\"members\":[2,3,4],\"op\":\"reshape_compartment\"}").expect("reshape reads"),
        PlanCommand::ReshapeCompartment { members: vec![2, 3, 4] },
    );

    // A tag outside the four, a key another variant declares, a missing key,
    // and a member list that is not a set.
    assert!(read("{\"op\":\"replace\"}").is_err(), "the union is closed at four");
    assert!(read("{\"op\":\"set_focus\",\"position\":2,\"slate_ordinal\":7}").is_err(), "observation is not a plan");
    assert!(read("{\"from\":2,\"op\":\"connect\",\"route\":1,\"to\":3}").is_err(), "an extra key");
    assert!(read("{\"op\":\"connect\",\"to\":3}").is_err(), "a missing key");
    assert!(read("{\"members\":[3,3],\"op\":\"reshape_compartment\"}").is_err(), "a repeat");
    assert!(read("{\"members\":[4,2],\"op\":\"reshape_compartment\"}").is_err(), "out of order");
    assert!(read("{\"members\":[],\"op\":\"reshape_compartment\"}").is_err(), "empty");
    assert!(read("{\"end\":\"middle\",\"op\":\"redirect\",\"route\":1,\"to\":4}").is_err(), "an end");
}

#[test]
fn a_body_that_is_not_the_union_at_all_is_refused() {
    let (mut session, _) = stilled(OPENING_IMPULSE);
    assert_eq!(refusal(&queue(&mut session, "{}")).0, "validation", "no tag");
    assert_eq!(refusal(&session.command("queue_plan", "{}")).0, "validation", "no plan");
    assert_eq!(refusal(&queue(&mut session, "7")).0, "validation", "not an object");
}

// ---------------------------------------------------------------------------
// One variant at a time, against its locked preconditions
// ---------------------------------------------------------------------------

#[test]
fn connect_validates_both_nodes_the_self_link_the_duplicate_and_the_reach() {
    let (mut session, _) = stilled(OPENING_IMPULSE);

    assert_eq!(
        refusal(&queue(&mut session, "{\"from\":2,\"op\":\"connect\",\"to\":9}")),
        ("not_found".to_string(), "node".to_string()),
        "a Node that names nothing",
    );
    assert_eq!(
        refusal(&queue(&mut session, "{\"from\":2,\"op\":\"connect\",\"to\":2}")),
        ("validation".to_string(), "self_link".to_string()),
        "from and to are the same Node",
    );
    assert_eq!(
        refusal(&queue(&mut session, "{\"from\":2,\"op\":\"connect\",\"to\":3}")),
        ("validation".to_string(), "duplicate".to_string()),
        "Route 1 already stands from 2 to 3",
    );
    assert_eq!(
        refusal(&queue(&mut session, "{\"from\":2,\"op\":\"connect\",\"to\":5}")),
        ("validation".to_string(), "reach".to_string()),
        "Node 5 stands outside the controlled Form's reach",
    );

    // The other direction of a standing Route is not a duplicate: a Route is
    // directed, and 3 → 2 is a Route the Field does not hold.
    let answer = queue(&mut session, "{\"from\":3,\"op\":\"connect\",\"to\":2}");
    assert_eq!(entries(&queue_state(&answer)).len(), 1, "the one entry that passed");
}

#[test]
fn redirect_validates_the_route_the_node_the_duplicate_and_the_reach() {
    let (mut session, _) = stilled(OPENING_IMPULSE);

    assert_eq!(
        refusal(&queue(&mut session, "{\"end\":\"head\",\"op\":\"redirect\",\"route\":9,\"to\":4}")),
        ("not_found".to_string(), "route".to_string()),
    );
    assert_eq!(
        refusal(&queue(&mut session, "{\"end\":\"head\",\"op\":\"redirect\",\"route\":1,\"to\":9}")),
        ("not_found".to_string(), "node".to_string()),
    );
    // Route 2 stands 3 → 4; moving Route 1's head to 4 would put 2 → 4 on the
    // Field, which no Route holds, so that one passes. Moving Route 2's tail to
    // 2 would put 2 → 4 there too, and after the first entry it is a duplicate.
    assert_eq!(
        refusal(&queue(&mut session, "{\"end\":\"head\",\"op\":\"redirect\",\"route\":1,\"to\":5}")),
        ("validation".to_string(), "reach".to_string()),
        "the resulting endpoints stand further apart than the reach",
    );
    body(&queue(&mut session, "{\"end\":\"head\",\"op\":\"redirect\",\"route\":1,\"to\":4}"));
    assert_eq!(
        refusal(&queue(&mut session, "{\"end\":\"tail\",\"op\":\"redirect\",\"route\":2,\"to\":2}")),
        ("validation".to_string(), "duplicate".to_string()),
        "against the projection the first entry left",
    );
}

#[test]
fn cut_validates_the_route_and_nothing_else() {
    let (mut session, _) = stilled(OPENING_IMPULSE);
    assert_eq!(
        refusal(&queue(&mut session, "{\"op\":\"cut\",\"route\":9}")),
        ("not_found".to_string(), "route".to_string()),
    );
    body(&queue(&mut session, "{\"op\":\"cut\",\"route\":1}"));
    // The same Route again: the projection no longer holds it.
    assert_eq!(
        refusal(&queue(&mut session, "{\"op\":\"cut\",\"route\":1}")),
        ("not_found".to_string(), "route".to_string()),
    );
}

#[test]
fn reshape_compartment_takes_the_member_set_through_intake_without_moving_the_view() {
    let (mut session, _) = stilled(OPENING_IMPULSE);

    // A member naming a Node that stands nowhere is dropped by intake rather
    // than refusing the entry; a set that intake empties is refused.
    assert_eq!(
        refusal(&queue(&mut session, "{\"members\":[41,42],\"op\":\"reshape_compartment\"}")),
        ("validation".to_string(), "members".to_string()),
    );
    body(&queue(&mut session, "{\"members\":[3,4,42],\"op\":\"reshape_compartment\"}"));
    body(&session.command("commit_plan", "{}"));
    assert_eq!(physical_members(&session), vec![3, 4], "the material intersection, ascending");
    assert_eq!(inside(&session), vec![2, 3], "a physical edit never changes observation");
}

#[test]
fn set_focus_is_refused_when_the_ordinal_names_no_slate() {
    // Observation is its own free protocol command, never a queued causal edit.
    let (mut session, _) = stilled(OPENING_IMPULSE);
    assert_eq!(
        refusal(&session.command("set_focus", "{\"position\":1,\"slate_ordinal\":7}")),
        ("not_found".to_string(), "slate".to_string()),
    );
    assert_eq!(entries(&queue_state(&session.command("undo_plan", "{}"))).len(), 0);
}

// ---------------------------------------------------------------------------
// The queue: costs, the caps, and what an entry is validated against
// ---------------------------------------------------------------------------

#[test]
fn every_entry_costs_one_impulse_and_the_queue_reports_what_it_would_leave() {
    let (mut session, _) = stilled(IMPULSE_CAP);

    let first = queue_state(&queue(&mut session, "{\"op\":\"cut\",\"route\":1}"));
    assert_eq!(int_of(&first, "cost_total"), 1);
    assert_eq!(int_of(&first, "impulse"), 6);
    assert_eq!(int_of(&first, "impulse_after"), 5);
    let held = entries(&first);
    assert_eq!(int_of(&held[0], "cost"), 1, "one Impulse an entry, and nothing else spends it");
    assert_eq!(int_of(&held[0], "position"), 0);
    assert_eq!(held[0].get("conflict"), Some(&Json::Bool(false)));

    let second = queue_state(&queue(&mut session, "{\"op\":\"cut\",\"route\":2}"));
    assert_eq!(int_of(&second, "cost_total"), 2);
    assert_eq!(int_of(&second, "impulse_after"), 4);
}

#[test]
fn a_queue_that_would_cost_more_impulse_than_the_run_carries_is_refused() {
    let (mut session, _) = stilled(2);
    body(&queue(&mut session, "{\"op\":\"cut\",\"route\":1}"));
    body(&queue(&mut session, "{\"op\":\"cut\",\"route\":2}"));
    let answer = queue(&mut session, "{\"from\":3,\"op\":\"connect\",\"to\":2}");
    let parsed = parse(&answer).expect("canonical");
    let error = parsed.get("error").expect("a refusal");
    assert_eq!(error.get("code").and_then(Json::as_text), Some("impulse"));
    assert_eq!(int_of(error.get("detail").expect("a detail"), "cost"), 3);
    assert_eq!(int_of(error.get("detail").expect("a detail"), "impulse"), 2);
    assert_eq!(entries(&queue_state(&session.command("undo_plan", "{}"))).len(), 1, "and not queued");
}

#[test]
fn the_seventh_entry_crosses_the_locked_depth_rather_than_the_impulse() {
    // Both caps stand at six, so the order the two checks run in decides which
    // envelope a seventh entry answers with. The depth is checked first, and
    // deliberately: it is the fact about the queue, and it would otherwise be
    // an envelope nothing could ever reach.
    let (mut session, _) = stilled(IMPULSE_CAP);
    let plans = [
        "{\"op\":\"cut\",\"route\":1}",
        "{\"op\":\"cut\",\"route\":2}",
        "{\"from\":3,\"op\":\"connect\",\"to\":2}",
        "{\"from\":4,\"op\":\"connect\",\"to\":2}",
        "{\"from\":4,\"op\":\"connect\",\"to\":3}",
        "{\"members\":[2,3],\"op\":\"reshape_compartment\"}",
    ];
    for plan in plans {
        body(&queue(&mut session, plan));
    }
    assert_eq!(PLAN_QUEUE_DEPTH, 6);
    let answer = queue(&mut session, "{\"members\":[2,4],\"op\":\"reshape_compartment\"}");
    let parsed = parse(&answer).expect("canonical");
    let error = parsed.get("error").expect("a refusal");
    assert_eq!(error.get("code").and_then(Json::as_text), Some("capacity"));
    assert_eq!(
        error.get("detail").and_then(|detail| detail.get("quantity")).and_then(Json::as_text),
        Some("plan_queue_depth"),
    );
}

#[test]
fn an_undo_takes_the_most_recent_entry_and_the_cost_falls_with_it() {
    let (mut session, _) = stilled(IMPULSE_CAP);
    body(&queue(&mut session, "{\"op\":\"cut\",\"route\":1}"));
    body(&queue(&mut session, "{\"op\":\"cut\",\"route\":2}"));

    let answer = body(&session.command("undo_plan", "{}"));
    assert_eq!(int_of(&answer, "remaining"), 1);
    let held = answer.get("queue").expect("the queue rides with it");
    assert_eq!(int_of(held, "cost_total"), 1);
    assert_eq!(int_of(held, "impulse_after"), 5);
    assert_eq!(
        entries(held)[0].get("plan").and_then(|plan| plan.get("route")).and_then(Json::as_int),
        Some(1),
        "the entry that was queued first is the one that remains",
    );
}

// ---------------------------------------------------------------------------
// Conflicts
// ---------------------------------------------------------------------------

#[test]
fn two_entries_that_touch_one_route_are_both_flagged() {
    let (mut session, _) = stilled(IMPULSE_CAP);
    body(&queue(&mut session, "{\"end\":\"head\",\"op\":\"redirect\",\"route\":1,\"to\":4}"));
    let held = queue_state(&queue(&mut session, "{\"op\":\"cut\",\"route\":1}"));
    let flagged: Vec<bool> = entries(&held)
        .iter()
        .map(|entry| entry.get("conflict") == Some(&Json::Bool(true)))
        .collect();
    assert_eq!(flagged, vec![true, true]);

    // A conflict informs the display and invalidates nothing by itself: the
    // commit applies both, in order.
    let applied = body(&session.command("commit_plan", "{}"));
    assert_eq!(int_of(&applied, "applied"), 2);
    assert_eq!(routes(&session), vec![(2, 3, 4)], "the redirected Route was then cut");
}

#[test]
fn two_entries_that_propose_one_endpoint_pair_are_both_flagged() {
    // The pair a redirect proposes is resolved in the projection it stands in,
    // so a later `connect` of the same link is read against the same pair — and
    // the cut between them is what makes both entries stand at once.
    let (mut session, _) = stilled(IMPULSE_CAP);
    body(&queue(&mut session, "{\"end\":\"tail\",\"op\":\"redirect\",\"route\":2,\"to\":2}"));
    body(&queue(&mut session, "{\"op\":\"cut\",\"route\":2}"));
    let held = queue_state(&queue(&mut session, "{\"from\":2,\"op\":\"connect\",\"to\":4}"));
    let flagged: Vec<bool> = entries(&held)
        .iter()
        .map(|entry| entry.get("conflict") == Some(&Json::Bool(true)))
        .collect();
    assert_eq!(flagged, vec![true, true, true], "two on the pair, and two on the Route");
}

#[test]
fn an_undo_resolves_the_conflict_keys_again() {
    let (mut session, _) = stilled(IMPULSE_CAP);
    body(&queue(&mut session, "{\"end\":\"head\",\"op\":\"redirect\",\"route\":1,\"to\":4}"));
    body(&queue(&mut session, "{\"op\":\"cut\",\"route\":1}"));
    let held = body(&session.command("undo_plan", "{}"));
    let queue = held.get("queue").expect("the queue rides with it");
    assert_eq!(
        entries(queue)[0].get("conflict"),
        Some(&Json::Bool(false)),
        "one entry conflicts with nothing",
    );
}

// ---------------------------------------------------------------------------
// The commit: all or none, and the cost it spends
// ---------------------------------------------------------------------------

#[test]
fn the_commit_spends_exactly_the_total_the_queue_predicted() {
    // A mixed queue: one of each kind that can stand together, the tray's
    // prediction read before the commit, and the commit's own answer read
    // after. The two are the same number, which is the goal's own done-when.
    let (mut session, _) = stilled(IMPULSE_CAP);
    body(&queue(&mut session, "{\"from\":3,\"op\":\"connect\",\"to\":2}"));
    body(&queue(&mut session, "{\"op\":\"cut\",\"route\":2}"));
    let predicted = queue_state(&queue(&mut session, "{\"members\":[2,3],\"op\":\"reshape_compartment\"}"));
    let per_entry: Vec<i64> = entries(&predicted).iter().map(|entry| int_of(entry, "cost")).collect();
    assert_eq!(per_entry, vec![1, 1, 1]);
    assert_eq!(int_of(&predicted, "cost_total"), 3);
    assert_eq!(int_of(&predicted, "impulse"), 6);
    assert_eq!(int_of(&predicted, "impulse_after"), 3);

    let applied = body(&session.command("commit_plan", "{}"));
    assert_eq!(int_of(&applied, "applied"), 3);
    assert_eq!(
        int_of(&applied, "impulse"),
        int_of(&predicted, "impulse_after"),
        "what the tray predicted is what the commit spent",
    );
    assert_eq!(
        session.run().expect("a run is loaded").state().progress.impulse as i64,
        3,
        "and the run carries it",
    );
    assert_eq!(routes(&session), vec![(1, 2, 3), (3, 3, 2)], "the cut is gone and the link stands");
    assert_eq!(inside(&session), vec![2, 3]);
}

#[test]
fn a_refused_entry_is_not_queued_and_leaves_no_trace_of_itself() {
    let (mut session, _) = stilled(OPENING_IMPULSE);
    for plan in [
        "{\"from\":2,\"op\":\"connect\",\"to\":9}",
        "{\"from\":2,\"op\":\"connect\",\"to\":2}",
        "{\"op\":\"cut\",\"route\":9}",
        "{\"members\":[41],\"op\":\"reshape_compartment\"}",
    ] {
        assert!(queue(&mut session, plan).contains("\"ok\":false"), "{plan}");
    }
    // Nothing was queued, nothing was applied, and — the one a refused entry
    // could plausibly have left behind — the refused reshape recorded no drag,
    // because the drawn list is written after the entry has passed.
    assert_eq!(int_of(&body(&session.command("undo_plan", "{}")), "remaining"), 0);
    assert_eq!(routes(&session), vec![(1, 2, 3), (2, 3, 4)]);
    assert_eq!(inside(&session), vec![2, 3]);
    assert!(session.run().expect("a run is loaded").state().now.boundaries.drawn.is_empty());
}

#[test]
fn a_queue_with_one_invalid_entry_applies_none_of_itself() {
    // The transaction, read at the level it is made: every entry is applied to
    // a projection of the base state and the projection is installed only once
    // the last one has passed. A refusal at any position therefore drops a
    // projection — there is no half-applied state to roll back, and the
    // position and reason the commit answers with are the failing entry's own.
    let state = state_with(IMPULSE_CAP);
    let mut projected = Projection::of(&state.now);
    // The whole payload, byte for byte: what the transaction may not touch is
    // not "the Routes" or "the View" but every byte of the run.
    let held = state.payload();

    let entries = [
        PlanCommand::Cut { route: 1 },
        // Route 1 has gone by the time this is revalidated.
        PlanCommand::Redirect { route: 1, end: RouteEnd::Head, to: 4 },
        PlanCommand::Cut { route: 2 },
    ];
    let mut refused = None;
    for (position, entry) in entries.iter().enumerate() {
        match plan::check(entry, &projected)
            .and_then(|()| plan::apply(entry, &mut projected))
        {
            Ok(()) => continue,
            Err(refusal) => {
                refused = Some((position, refusal));
                break;
            }
        }
    }
    let (position, refusal) = refused.expect("the second entry cannot stand");
    assert_eq!(position, 1);
    assert_eq!(refusal.reason, "route");
    assert_eq!(state.payload(), held, "no byte of the base state was written to");

    let written = refusal.positioned(position).write();
    let parsed = parse(&written).expect("canonical");
    let detail = parsed.get("detail").expect("a refusal carries a detail");
    assert_eq!(int_of(detail, "position"), 1);
    assert_eq!(detail.get("reason").and_then(Json::as_text), Some("route"));
    assert_eq!(parsed.get("code").and_then(Json::as_text), Some("not_found"));
}

#[test]
fn a_commit_is_an_exit_and_an_empty_one_spends_nothing() {
    let (mut session, _) = stilled(OPENING_IMPULSE);
    let answer = body(&session.command("commit_plan", "{}"));
    assert_eq!(int_of(&answer, "applied"), 0);
    assert_eq!(int_of(&answer, "impulse"), i64::from(OPENING_IMPULSE));
    assert_eq!(int_of(&answer, "slate_ordinal"), 0);
    assert_eq!(session.lifecycle(), "ramp_out");
}

// ---------------------------------------------------------------------------
// What a commit does to the Field, and to the window
// ---------------------------------------------------------------------------

#[test]
fn a_formed_route_opens_at_the_locked_capacity_and_carries_from_the_next_step() {
    let (mut session, _) = stilled(IMPULSE_CAP);
    body(&queue(&mut session, "{\"from\":2,\"op\":\"connect\",\"to\":4}"));
    body(&session.command("commit_plan", "{}"));

    let formed = {
        let state = session.run().expect("a run is loaded").state();
        state.now.routes.iter().find(|route| route.route == 3).expect("the formed Route").clone()
    };
    assert_eq!(formed.capacity, CONNECTED_ROUTE_CAPACITY);
    assert_eq!(formed.flow, 0, "it has carried nothing, because no step has run");
    assert_eq!(formed.formed_step, 0, "the completed step the commit landed on");

    // The exit ramp, and then one step: the pass at step 1 reads every Route
    // standing when the step began, the formed one among them.
    session.command("input_frame", &at(3, 0, 1_000_000 + 2 * RAMP_US));
    assert_eq!(session.lifecycle(), "running");
    session.command("input_frame", &at(4, 1, 1_000_000 + 2 * RAMP_US + FRAME_US));
    let carried = session
        .run()
        .expect("a run is loaded")
        .state()
        .now
        .routes
        .iter()
        .find(|route| route.route == 3)
        .expect("the formed Route")
        .flow;
    assert!(carried > 0, "the first pass after the commit carries along it");
}

#[test]
fn a_commit_that_applies_a_change_ends_the_active_window() {
    // The locked leakage rule leaves two paths and no third, and this is the
    // first: a commit lands on a window boundary. The trajectory restarts at
    // the commit's own step, so no window a replay reads spans a change no step
    // recorded.
    let mut session = Session::new(&support::worker_init()).expect("versions agree");
    let file = state_with(IMPULSE_CAP).export_file();
    let mut import = String::from("{\"text\":");
    write_text(&mut import, &file);
    import.push('}');
    body(&session.command("import_run", &import));

    session.command("input_frame", &at(1, 200, 1_000_000));
    let before = session.run().expect("a run is loaded").state().trace.clone();
    assert_eq!(before.start_step, 60, "the retained span puts the keyframe 120 to 150 steps back");
    assert_eq!(before.steps.len(), 140);

    session.command("input_frame", &toggling_at(2, 0, 1_100_000));
    session.command("input_frame", &at(3, 0, 1_100_000 + RAMP_US));
    assert_eq!(session.lifecycle(), "still");
    body(&queue(&mut session, "{\"members\":[2,4],\"op\":\"reshape_compartment\"}"));
    body(&session.command("commit_plan", "{}"));

    let after = session.run().expect("a run is loaded").state().trace.clone();
    assert_eq!(after.start_step, 200, "the trajectory restarts at the commit");
    assert!(after.steps.is_empty(), "and holds no step that ran under the inside it left");
    assert_eq!(after.keyframe.step, 200);
    assert_eq!(
        after.keyframe.written(),
        session.run().expect("a run is loaded").state().now.written(),
        "the keyframe is the state the commit leaves",
    );
    assert_eq!(physical_members(&session), vec![2, 4]);
    assert_eq!(inside(&session), vec![2, 3], "the observation View is unchanged");
}

#[test]
fn a_run_carrying_a_committed_reshape_exports_and_restores_byte_for_byte() {
    let mut session = Session::new(&support::worker_init()).expect("versions agree");
    let file = state_with(IMPULSE_CAP).export_file();
    let mut import = String::from("{\"text\":");
    write_text(&mut import, &file);
    import.push('}');
    body(&session.command("import_run", &import));

    session.command("input_frame", &at(1, 200, 1_000_000));
    session.command("input_frame", &toggling_at(2, 0, 1_100_000));
    session.command("input_frame", &at(3, 0, 1_100_000 + RAMP_US));
    body(&queue(&mut session, "{\"from\":2,\"op\":\"connect\",\"to\":4}"));
    body(&queue(&mut session, "{\"members\":[2,4],\"op\":\"reshape_compartment\"}"));
    body(&session.command("commit_plan", "{}"));
    session.command("input_frame", &at(4, 0, 1_100_000 + 2 * RAMP_US));
    session.command("input_frame", &at(5, 40, 1_400_000));

    // Export, restore, export again: the second and third files are the same
    // bytes, so a run that carries a committed reshape restores to itself. The
    // first file differs from them in one field and one only — the previous
    // assembly step, which every restore normalizes to the step it returned to.
    let first = exported(&mut session);
    let mut fresh = restored(&first);
    let second = exported(&mut fresh);
    let mut again = restored(&second);
    let third = exported(&mut again);
    assert_eq!(second, third, "a run restored around a commit is the same run");
    assert_eq!(payload(&fresh), payload(&again));
    assert_eq!(
        payload(&session).replacen("\"prev_assembly_step\":200", "\"prev_assembly_step\":240", 1),
        payload(&fresh),
        "and the one field a restore normalizes is the only one that moved",
    );

    // What the commit did is in the bytes: it formed the Route, replaced the
    // material compartment, preserved the passive View, and restarted the
    // trajectory at the commit.
    assert_eq!(routes(&fresh), routes(&session));
    assert_eq!(
        physical_members(&fresh),
        vec![2, 4],
        "the committed compartment restores",
    );
    assert_eq!(
        inside(&fresh),
        vec![2, 3],
        "the passive View is preserved across the restore",
    );
    assert_eq!(
        fresh.run().expect("a run is loaded").state().trace.start_step,
        200,
        "the window the commit ended is where the retained trajectory begins",
    );
    assert!(
        fresh.run().expect("a run is loaded").queue().is_empty(),
        "and a restore lands with the queue cleared",
    );
}

/// The export file a session writes.
fn exported(session: &mut Session) -> String {
    body(&session.command("export_run", "{}"))
        .get("text")
        .and_then(Json::as_text)
        .expect("an export file")
        .to_string()
}

/// A fresh session holding the run one export file carries.
fn restored(text: &str) -> Session {
    let mut fresh = Session::new(&support::worker_init()).expect("versions agree");
    let mut import = String::from("{\"text\":");
    write_text(&mut import, text);
    import.push('}');
    body(&fresh.command("import_run", &import));
    fresh
}

#[test]
fn an_export_taken_mid_still_with_a_queue_restores_with_the_queue_cleared() {
    let (mut session, _) = stilled(IMPULSE_CAP);
    body(&queue(&mut session, "{\"op\":\"cut\",\"route\":1}"));
    body(&queue(&mut session, "{\"members\":[2,4],\"op\":\"reshape_compartment\"}"));

    let exported = body(&session.command("export_run", "{}"));
    let text = exported.get("text").and_then(Json::as_text).expect("an export file").to_string();
    let mut fresh = Session::new(&support::worker_init()).expect("versions agree");
    let mut import = String::from("{\"text\":");
    write_text(&mut import, &text);
    import.push('}');
    body(&fresh.command("import_run", &import));

    assert_eq!(fresh.lifecycle(), "running", "a record carries no mode");
    assert!(fresh.run().expect("a run is loaded").queue().is_empty(), "and no queue");
    assert_eq!(
        fresh.run().expect("a run is loaded").state().now.routes.len(),
        2,
        "nothing the queue proposed was applied",
    );
    assert_eq!(
        fresh.run().expect("a run is loaded").state().now.boundaries.drawn.len(),
        1,
        "and the drag the player completed is still recorded",
    );
}

#[test]
fn a_completed_drag_is_recorded_whether_or_not_the_change_is_committed() {
    // The one locked queue-time side effect, and the one thing an undo does not
    // take back: the drawn list holds what the player drew, and source 1 of
    // candidate assembly reads that list rather than the commits.
    let (mut session, _) = stilled(IMPULSE_CAP);
    body(&queue(&mut session, "{\"members\":[2,4],\"op\":\"reshape_compartment\"}"));
    body(&session.command("undo_plan", "{}"));

    let drawn = session.run().expect("a run is loaded").state().now.boundaries.drawn.clone();
    assert_eq!(drawn.len(), 1, "the drag was recorded when it was queued");
    assert_eq!(drawn[0].members, vec![2, 4]);
    assert_eq!(drawn[0].step, 0, "at the step the run stood on");
    assert_eq!(inside(&session), vec![2, 3], "and the standing inside never moved");
}

// ---------------------------------------------------------------------------
// Determinism
// ---------------------------------------------------------------------------

#[test]
fn a_run_with_queued_and_committed_edits_replays_byte_for_byte() {
    // Commits are step input: they arrive as commands between frames, and the
    // same run key, the same frames, and the same commands produce the same
    // bytes twice.
    let played = || -> String {
        let mut session = Session::new(&support::worker_init()).expect("versions agree");
        let file = state_with(IMPULSE_CAP).export_file();
        let mut import = String::from("{\"text\":");
        write_text(&mut import, &file);
        import.push('}');
        body(&session.command("import_run", &import));
        session.command("input_frame", &at(1, 60, 1_000_000));
        session.command("input_frame", &toggling_at(2, 0, 1_050_000));
        session.command("input_frame", &at(3, 0, 1_050_000 + RAMP_US));
        body(&queue(&mut session, "{\"op\":\"cut\",\"route\":1}"));
        body(&queue(&mut session, "{\"from\":3,\"op\":\"connect\",\"to\":2}"));
        body(&queue(&mut session, "{\"members\":[2,4],\"op\":\"reshape_compartment\"}"));
        body(&session.command("undo_plan", "{}"));
        body(&queue(&mut session, "{\"members\":[3,4],\"op\":\"reshape_compartment\"}"));
        body(&session.command("commit_plan", "{}"));
        session.command("input_frame", &at(4, 0, 1_050_000 + 2 * RAMP_US));
        session.command("input_frame", &at(5, 45, 1_400_000));
        payload(&session)
    };
    assert_eq!(played(), played());
}

// ---------------------------------------------------------------------------
// Impulse: where it lives, and what gives it back
// ---------------------------------------------------------------------------

#[test]
fn the_impulse_is_carried_in_progress_and_the_frame_header_reads_it_there() {
    let (session, _) = stilled(5);
    let state = session.run().expect("a run is loaded").state();
    assert_eq!(state.progress.impulse, 5);
    // The payload carries it under progress and nowhere else.
    let payload = state.payload();
    assert!(payload.contains("\"complete\":[],\"impulse\":5,\"objective\""), "{payload}");
    assert_eq!(payload.matches("\"impulse\"").count(), 1, "one field, in one place");
    assert_eq!(session.frame_view()[16], 5, "and the header's byte reads it there");
}

#[test]
fn a_completed_layer_objective_gives_three_impulse_back_and_the_cap_holds() {
    // The authored sequence is what knows a completion happened, and progress is
    // what it writes into. The first objective of the opening chapter asks the
    // Form to stand in the bright current; steering into it and holding is the
    // whole of the run this needs.
    let mut session = Session::new(&support::worker_init()).expect("versions agree");
    let answer = session.command(
        "init_run",
        &format!("{{\"form\":\"thread\",\"mode\":\"new\",\"run_id\":\"{KEY}\"}}"),
    );
    assert!(answer.contains("\"ok\":true"), "{answer}");
    assert_eq!(
        session.run().expect("a run is loaded").state().progress.impulse,
        OPENING_IMPULSE,
        "a new run opens at three",
    );

    let mut seq = 1u32;
    let mut completed = 0;
    while seq < 400 {
        let form = {
            let state = session.run().expect("a run is loaded").state();
            state.now.forms.iter().find(|form| form.controlled).expect("a Form").clone()
        };
        let (steer_x, steer_y) = toward(form.pos, 1520, 1992);
        let body = format!(
            "{{\"advance_steps\":15,\"depth_key\":0,\"inspect\":null,\"pause\":false,\
              \"pulse_held\":false,\"pulse_release\":false,\"seq\":{seq},\"steer_x\":{steer_x},\
              \"steer_y\":{steer_y},\"t_us\":0,\"toggle_still\":false,\"wheel\":0}}"
        );
        session.command("input_frame", &body);
        seq += 1;
        completed = session.run().expect("a run is loaded").state().progress.complete.len();
        if completed > 0 {
            break;
        }
    }
    assert!(completed > 0, "the opening objective completes");
    let state = session.run().expect("a run is loaded").state();
    assert_eq!(state.progress.impulse, OPENING_IMPULSE + 3, "three back, once");
    assert_eq!(session.frame_view()[16], OPENING_IMPULSE + 3);

    // The clamp, read directly: a second grant cannot carry the run past six.
    let mut progress = state.progress.clone();
    progress.grant_impulse();
    assert_eq!(progress.impulse, IMPULSE_CAP);
    progress.grant_impulse();
    assert_eq!(progress.impulse, IMPULSE_CAP, "and stays there");
}

/// The steering that aims a Form at a point, as the onboarding run's does.
fn toward(from: Vec2, x: i64, y: i64) -> (i64, i64) {
    let dx = x * ONE_UNIT - from.x;
    let dy = y * ONE_UNIT - from.y;
    let distance = field_game_core::fx::isqrt(
        (i128::from(dx) * i128::from(dx) + i128::from(dy) * i128::from(dy)) as u128,
    ) as i64;
    if distance == 0 {
        return (0, 0);
    }
    let limit = 32_767i64;
    let magnitude = (limit * distance / (320 * ONE_UNIT)).min(limit);
    let mut steer_x = dx * magnitude / distance;
    let mut steer_y = dy * magnitude / distance;
    while steer_x * steer_x + steer_y * steer_y > limit * limit {
        steer_x -= steer_x.signum();
        steer_y -= steer_y.signum();
    }
    (steer_x, steer_y)
}

// ---------------------------------------------------------------------------
// Previews
// ---------------------------------------------------------------------------

/// The routes section of the newest snapshot, as (route, tail, head, status).
fn frame_routes(session: &Session) -> Vec<(u32, u32, u32, u8)> {
    let view = session.frame_view();
    let mut found = Vec::new();
    for place in 0..usize::from(view[20]) {
        let at = 32 + place * 8;
        if view[at] != 3 {
            continue;
        }
        let count = usize::from(u16::from_le_bytes([view[at + 2], view[at + 3]]));
        let offset =
            u32::from_le_bytes([view[at + 4], view[at + 5], view[at + 6], view[at + 7]]) as usize;
        for record in 0..count {
            let start = offset + record * 16;
            let read = |from: usize| {
                u32::from_le_bytes([
                    view[from],
                    view[from + 1],
                    view[from + 2],
                    view[from + 3],
                ])
            };
            found.push((read(start), read(start + 4), read(start + 8), view[start + 13]));
        }
    }
    found
}

/// The port flags of the newest snapshot, in record order.
fn frame_port_flags(session: &Session) -> Vec<u8> {
    let view = session.frame_view();
    let mut found = Vec::new();
    for place in 0..usize::from(view[20]) {
        let at = 32 + place * 8;
        if view[at] != 2 {
            continue;
        }
        let count = usize::from(u16::from_le_bytes([view[at + 2], view[at + 3]]));
        let offset =
            u32::from_le_bytes([view[at + 4], view[at + 5], view[at + 6], view[at + 7]]) as usize;
        for record in 0..count {
            found.push(view[offset + record * 16 + 5]);
        }
    }
    found
}

#[test]
fn the_frame_carries_the_queue_as_previews_and_drops_them_with_it() {
    let (mut session, _) = stilled(IMPULSE_CAP);
    assert_eq!(
        frame_routes(&session),
        vec![(1, 2, 3, 0), (2, 3, 4, 0)],
        "a queue with nothing in it previews nothing",
    );

    body(&queue(&mut session, "{\"op\":\"cut\",\"route\":1}"));
    body(&queue(&mut session, "{\"from\":3,\"op\":\"connect\",\"to\":2}"));
    body(&queue(&mut session, "{\"end\":\"tail\",\"op\":\"redirect\",\"route\":2,\"to\":2}"));
    assert_eq!(
        frame_routes(&session),
        vec![
            // The standing Routes, each carrying what the queue would do to it.
            (1, 2, 3, 1),
            (2, 3, 4, 3),
            // One proposal per entry that proposes a link, past the standing
            // Routes: the identifier it would take, and where it would stand.
            (3, 3, 2, 4),
            (2, 2, 4, 4),
        ],
    );

    // The material membership a queued reshape proposes, beside the standing one.
    let standing = frame_port_flags(&session);
    assert!(standing.iter().all(|flags| flags & 16 == 0), "no compartment is proposed yet");
    body(&queue(&mut session, "{\"members\":[3,4],\"op\":\"reshape_compartment\"}"));
    let proposed = frame_port_flags(&session);
    let member = |place: usize| proposed[place] & 4 != 0;
    let wanted = |place: usize| proposed[place] & 16 != 0;
    assert!(member(1) && !wanted(1), "Node 2 is physical and would be left out");
    assert!(member(2) && wanted(2), "Node 3 is physical and would stay");
    assert!(!member(3) && wanted(3), "Node 4 is non-member and would be taken in");

    body(&session.command("undo_plan", "{}"));
    assert!(
        frame_port_flags(&session).iter().all(|flags| flags & 16 == 0),
        "an undone reshape proposes nothing",
    );
    body(&session.command("commit_plan", "{}"));
    assert_eq!(
        frame_routes(&session),
        vec![(2, 2, 4, 0), (3, 3, 2, 0)],
        "a committed queue is state, and previews nothing",
    );
}

// ---------------------------------------------------------------------------
// The queue itself, without a session around it
// ---------------------------------------------------------------------------

#[test]
fn a_queue_holds_its_depth_and_no_more() {
    let mut queue = PlanQueue::new();
    for route in 1..=PLAN_QUEUE_DEPTH as u32 {
        queue.push(PlanCommand::Cut { route }).expect("inside the depth");
    }
    assert_eq!(queue.len(), PLAN_QUEUE_DEPTH);
    assert!(queue.push(PlanCommand::Cut { route: 9 }).is_err(), "and refuses a seventh");
    assert_eq!(queue.cost_total(), PLAN_QUEUE_DEPTH as u8, "one Impulse an entry");
    queue.undo();
    assert_eq!(queue.len(), PLAN_QUEUE_DEPTH - 1);
    queue.clear();
    assert!(queue.is_empty());
    assert!(queue.undo().is_none(), "an undo on an empty queue changes nothing");
}

#[test]
fn the_locked_run_state_reads_a_progress_carrying_its_impulse() {
    let payload = state_with(4).payload();
    let parsed = parse(&payload).expect("canonical");
    let restored = RunState::read(&parsed).expect("the payload reads back");
    assert_eq!(restored.progress.impulse, 4);
    restored.coherent().expect("and stands");

    // A payload naming an Impulse past the cap is refused.
    let broken = payload.replace("\"impulse\":4", "\"impulse\":9");
    let parsed = parse(&broken).expect("canonical");
    assert!(RunState::read(&parsed).is_err(), "the cap is held at the reader");
}

// ---------------------------------------------------------------------------
// The window a commit leaves
// ---------------------------------------------------------------------------

#[test]
fn an_applying_commit_leaves_a_retained_span_of_zero_and_a_window_clamped_to_it() {
    // Ending a window is half a rule until the procedures that read one are
    // told what to read. The locked clamp is `w_eff = min(w, t0,
    // retained_span)`, and the span is what an applying commit restarts: right
    // after one there is nothing to observe, so every windowed reading is
    // unassigned until the span regrows.
    let mut session = Session::new(&support::worker_init()).expect("versions agree");
    let file = state_with(IMPULSE_CAP).export_file();
    let mut import = String::from("{\"text\":");
    write_text(&mut import, &file);
    import.push('}');
    body(&session.command("import_run", &import));

    // A young run is clamped by its own age, which is FRAMEWORK's own term.
    session.command("input_frame", &at(1, 3, 1_000_000));
    {
        let state = session.run().expect("a run is loaded").state();
        assert_eq!(state.retained_span(), 3);
        assert_eq!(state.effective_window(45), 3, "min(w, t0, span) — the run is three steps old");
    }

    session.command("input_frame", &at(2, 197, 1_100_000));
    {
        let state = session.run().expect("a run is loaded").state();
        assert_eq!(state.now.step, 200);
        assert_eq!(state.retained_span(), 140, "the retained span the trace holds");
        assert_eq!(state.effective_window(45), 45, "the declared window is the smallest term");
    }

    session.command("input_frame", &toggling_at(3, 0, 1_200_000));
    session.command("input_frame", &at(4, 0, 1_200_000 + RAMP_US));
    assert_eq!(session.lifecycle(), "still");
    body(&queue(&mut session, "{\"members\":[2,4],\"op\":\"reshape_compartment\"}"));
    body(&session.command("commit_plan", "{}"));

    {
        let state = session.run().expect("a run is loaded").state();
        assert!(state.trace.steps.is_empty(), "the trajectory restarts at the commit");
        assert_eq!(state.retained_span(), 0, "so there is nothing retained to observe");
        assert_eq!(
            state.effective_window(45),
            0,
            "and every windowed procedure is unassigned, which w_eff 0 is how the \
             framework says so",
        );
    }

    // The span regrows a step at a time, and the window with it.
    session.command("input_frame", &at(5, 0, 1_200_000 + 2 * RAMP_US));
    assert_eq!(session.lifecycle(), "running");
    session.command("input_frame", &at(6, 10, 1_800_000));
    {
        let state = session.run().expect("a run is loaded").state();
        assert_eq!(state.retained_span(), 10);
        assert_eq!(state.effective_window(45), 10, "the span is now the smallest term");
    }
    session.command("input_frame", &at(7, 40, 2_400_000));
    {
        let state = session.run().expect("a run is loaded").state();
        assert_eq!(state.retained_span(), 50);
        assert_eq!(state.effective_window(45), 45, "and the declared window is back");
    }
}

#[test]
fn an_empty_commit_ends_no_window() {
    let mut session = Session::new(&support::worker_init()).expect("versions agree");
    let file = state_with(IMPULSE_CAP).export_file();
    let mut import = String::from("{\"text\":");
    write_text(&mut import, &file);
    import.push('}');
    body(&session.command("import_run", &import));
    session.command("input_frame", &at(1, 60, 1_000_000));
    session.command("input_frame", &toggling_at(2, 0, 1_100_000));
    session.command("input_frame", &at(3, 0, 1_100_000 + RAMP_US));

    let before = session.run().expect("a run is loaded").state().retained_span();
    body(&session.command("commit_plan", "{}"));
    let after = session.run().expect("a run is loaded").state();
    assert_eq!(after.retained_span(), before, "an empty commit changes nothing to end");
    assert_eq!(after.effective_window(45), 45);
}

// ---------------------------------------------------------------------------
// set_focus, the free observational protocol command
// ---------------------------------------------------------------------------

#[test]
fn set_focus_is_immediate_free_and_outside_the_causal_plan_queue() {
    let (mut session, _) = stilled(IMPULSE_CAP);
    body(&queue(&mut session, "{\"op\":\"cut\",\"route\":1}"));
    let (ordinal, position, expected) = {
        let run = session.run().expect("a run is loaded");
        let slate = run.standing_slate().expect("entry assembled a slate");
        let (place, candidate) = slate
            .candidates
            .iter()
            .enumerate()
            .find(|(_, candidate)| candidate.view != run.state().view)
            .expect("the fixture offers a distinct observation View");
        (slate.ordinal, place as u8 + 1, candidate.view.clone())
    };
    let before_field = session.run().expect("a run").state().now.written();
    let before_keyframe = session.run().expect("a run").state().trace.keyframe.written();
    let before_span = session.run().expect("a run").state().trace.steps.len();
    let before_step = session.step();
    let before_impulse = session.run().expect("a run").state().progress.impulse;
    let before_queue = session.run().expect("a run").queue().len();

    let answer = body(&session.command(
        "set_focus",
        &format!("{{\"position\":{position},\"slate_ordinal\":{ordinal}}}"),
    ));
    assert_eq!(answer.get("view"), Some(&parse(&expected.written()).expect("canonical View")));
    let run = session.run().expect("a run is loaded");
    assert_eq!(run.state().view, expected, "the candidate is active immediately");
    assert_eq!(run.state().now.written(), before_field, "the Field is untouched");
    assert_eq!(run.state().trace.keyframe.written(), before_keyframe, "the trace is untouched");
    assert_eq!(run.state().trace.steps.len(), before_span);
    assert_eq!(run.step(), before_step);
    assert_eq!(run.state().progress.impulse, before_impulse, "observation costs no Impulse");
    assert_eq!(run.queue().len(), before_queue, "the causal queue is unchanged");
    assert_eq!(session.lifecycle(), "still", "no commit or ramp was started");
}

#[test]
fn set_focus_position_zero_clears_only_view_members_and_round_trips() {
    let (mut session, _) = stilled(IMPULSE_CAP);
    body(&queue(&mut session, "{\"op\":\"cut\",\"route\":1}"));
    let (ordinal, before_view) = {
        let run = session.run().expect("a run is loaded");
        (
            run.standing_slate().expect("entry assembled a slate").ordinal,
            run.state().view.clone(),
        )
    };
    let before_field = session.run().expect("a run").state().now.written();
    let before_keyframe = session.run().expect("a run").state().trace.keyframe.written();
    let before_records: Vec<String> = session
        .run()
        .expect("a run")
        .state()
        .trace
        .steps
        .iter()
        .map(field_game_core::state::TraceStep::written)
        .collect();
    let before_rng = session.run().expect("a run").state().rng;
    let before_span = session.run().expect("a run").state().retained_span();
    let before_impulse = session.run().expect("a run").state().progress.impulse;
    let before_queue = session.run().expect("a run").queue().len();

    let answer = body(&session.command(
        "set_focus",
        &format!("{{\"position\":0,\"slate_ordinal\":{ordinal}}}"),
    ));
    let cleared = answer.get("view").expect("the cleared View");
    assert_eq!(cleared.get("inside"), Some(&Json::List(Vec::new())));
    assert_eq!(
        cleared.get("resolution").and_then(Json::as_int),
        Some(i64::from(before_view.resolution))
    );
    assert_eq!(
        cleared.get("window").and_then(Json::as_int),
        Some(i64::from(before_view.window))
    );
    assert_eq!(
        cleared.get("surround").and_then(Json::as_text),
        Some(before_view.surround.name())
    );

    let run = session.run().expect("a run is loaded");
    assert!(run.state().view.inside.is_empty());
    assert_eq!(run.state().view.resolution, before_view.resolution);
    assert_eq!(run.state().view.window, before_view.window);
    assert_eq!(run.state().view.surround, before_view.surround);
    assert_eq!(
        run.state().now.written(),
        before_field,
        "clearing observation moves no Field byte"
    );
    assert_eq!(run.state().trace.keyframe.written(), before_keyframe);
    assert_eq!(
        run.state()
            .trace
            .steps
            .iter()
            .map(field_game_core::state::TraceStep::written)
            .collect::<Vec<_>>(),
        before_records,
    );
    assert_eq!(run.state().rng, before_rng);
    assert_eq!(run.state().retained_span(), before_span);
    assert_eq!(run.state().progress.impulse, before_impulse);
    assert_eq!(run.queue().len(), before_queue);
    run.state().coherent().expect("a cleared live View is coherent");

    let file = exported(&mut session);
    let restored = restored(&file);
    let state = restored.run().expect("the cleared run restores").state();
    assert!(
        state.view.inside.is_empty(),
        "the cleared selection remains across the save boundary"
    );
    assert_eq!(state.view.resolution, before_view.resolution);
    assert_eq!(state.view.window, before_view.window);
    assert_eq!(state.view.surround, before_view.surround);
    state.coherent().expect("the restored cleared View is coherent");
}

#[test]
fn a_run_that_took_the_grant_exports_and_restores_byte_for_byte() {
    // The grant crosses a save. It is written into progress, and progress is a
    // payload field, so a run that regained Impulse and then went through an
    // export and an import carries the Impulse it earned rather than the one it
    // opened with — on the authored content, where the grant actually happens.
    let mut session = Session::new(&support::worker_init()).expect("versions agree");
    let opened = session.command(
        "init_run",
        &format!("{{\"form\":\"thread\",\"mode\":\"new\",\"run_id\":\"{KEY}\"}}"),
    );
    assert!(opened.contains("\"ok\":true"), "{opened}");

    let mut seq = 1u32;
    while seq < 400 {
        let form = {
            let state = session.run().expect("a run is loaded").state();
            state.now.forms.iter().find(|form| form.controlled).expect("a Form").clone()
        };
        let (steer_x, steer_y) = toward(form.pos, 1520, 1992);
        let body = format!(
            "{{\"advance_steps\":15,\"depth_key\":0,\"inspect\":null,\"pause\":false,\
              \"pulse_held\":false,\"pulse_release\":false,\"seq\":{seq},\"steer_x\":{steer_x},\
              \"steer_y\":{steer_y},\"t_us\":0,\"toggle_still\":false,\"wheel\":0}}"
        );
        session.command("input_frame", &body);
        seq += 1;
        if !session.run().expect("a run is loaded").state().progress.complete.is_empty() {
            break;
        }
    }
    let granted = session.run().expect("a run is loaded").state().progress.impulse;
    assert_eq!(granted, OPENING_IMPULSE + 3, "the run took the grant");

    let first = exported(&mut session);
    let mut fresh = restored(&first);
    let second = exported(&mut fresh);
    let mut again = restored(&second);
    let third = exported(&mut again);

    assert_eq!(second, third, "a run that took the grant restores to itself");
    assert_eq!(payload(&fresh), payload(&again));
    assert_eq!(
        fresh.run().expect("a run is loaded").state().progress.impulse,
        granted,
        "and carries the Impulse it earned rather than the one it opened with",
    );
    assert_eq!(fresh.frame_view()[16], granted, "which the header reads where it lives");
    // The one field a restore normalizes is the only one that moved.
    let step = session.run().expect("a run is loaded").state().now.step;
    assert_eq!(
        payload(&session)
            .replacen("\"prev_assembly_step\":null", &format!("\"prev_assembly_step\":{step}"), 1),
        payload(&fresh),
    );
}
