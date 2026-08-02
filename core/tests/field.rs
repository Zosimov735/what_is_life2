//! The Field model: its parts, its arithmetic, its ledger, and its step.
//!
//! The contracts under test are the ones `docs/field-framework/ARCHITECTURE.md`
//! and `docs/field-framework/FRAMEWORK.md` lock: the Q32.16 operations with
//! their per-operation rounding, the one distance and adjacency rule, the
//! capacity table, the Charge identity of a completed step — checked as exact
//! integer accounting with no tolerance anywhere — the trace's dense per-step
//! encoding and its caps, Drain as the per-layer difficulty parameter, and the
//! byte-equivalence of a Field advanced across several layers under a recorded
//! control schedule.
//!
//! ARCHITECTURE.md's `Field model rules` section adds the four rules that move
//! Charge, and each is tested against the locked text: Route flow as one
//! ascending pass with its `open` gate, source shortfall, destination headroom,
//! and multi-hop chain; physical-compartment leakage by exposure and its
//! per-contact coefficient, including the empty and whole-of-N edges; Node overload as an inflow throttle,
//! a quarter-of-the-excess decay, and memoryless recovery; and Current delivery
//! with its exact remainder split and its refusal at a full Node.
//!
//! One phase is still reserved and so is untested: upkeep, whose attribution
//! across five purposes is locked nowhere.

use field_game_core::field::{
    self, advance, pulse_radius, reach_ticks, BoundaryState, CurrentState,
    FieldLayer, FormState, Ledger, NodeKind, PhysicalCompartment, PortState, RouteState, StepOutcome, StepRecords,
    Unstaged, UpkeepRecord, CUE_CHARGE_GATHERED, CUE_INTERFERENCE_PUSHED, CUE_PORT_OPENED,
    CUE_PULSE_EMITTED, CURRENTS_PER_CHAPTER, CURRENT_STRENGTH_CAP, DRAWN_RETAINED,
    FORECAST_DEPTH_CAP, FORMS_PER_RUN, LAYERS_PER_CHAPTER, LEAK_FRAC_CAP, MAX_LAYER,
    CUES_PER_FRAME, NODES_PER_RUN, NODE_CHARGE_CAP, PLANE_SPAN, PULSE_CHARGE_STEP,
    PULSE_DISPLACE_SHARE,
    PULSE_GATHER_SHARE, PULSE_RADIUS_BASE, ROUTES_PER_RUN, ROUTE_CAPACITY_CAP, STEER_DAMPING,
    STEER_REACH, STEER_REST, STEER_STIFFNESS,
};
use field_game_core::fault::Code;
use field_game_core::pressure::{
    self as pressure, Pressure, PressureContent, PressureState, Schedule, Stage, StageRow, Target,
    TargetKind,
};
use field_game_core::fx::{
    adjacent, clamp01, distance, fixed_div, fixed_mul, isqrt, within, Vec2, ADJACENT_WITHIN,
    LAYER_SEPARATION, ONE_UNIT, STORED_BOUND,
};
use field_game_core::json::canonicalize;
use field_game_core::run::Run;
use field_game_core::state::{
    ControlState, FieldState, Frac, Fx, InputConfig, Progress, RunState, Trace,
    TraceStep, ViewDeclaration, FRAC_ONE,
};
use field_game_core::rng::trajectory_stream;

mod support;

const KEY: &str = "0123456789abcdef";

/// The save payload cap: 8 MiB of canonical bytes.
const SAVE_PAYLOAD_CAP: usize = 8 * 1024 * 1024;

/// How many steps the retained trajectory holds at its widest.
const RETAINED_STEPS: usize = 150;

/// The measured canonical size of a fully dense `TraceStep`, which
/// `docs/field-framework/ARCHITECTURE.md` records under Save version 2.
const DENSE_STEP_BYTES: usize = 49_092;

/// The measured canonical size of the worst-case save payload: the widest
/// retained trajectory over a Field at every cap, before the slate, the
/// pressures, and the Anchor records fill in. Recorded in the same section.
///
/// Every quantity of the fixture stands at its own cap; the booleans are not
/// caps, and their spelling moves this figure by 16 bytes either way, so the
/// equality pins one precisely stated fixture rather than a claim about the
/// widest way to spell a flag.
const WORST_PAYLOAD_BYTES: usize = 7_666_474;

// ---------------------------------------------------------------------------
// Fixtures. Balance values are the fixture's own; the shapes are locked.
// ---------------------------------------------------------------------------

fn layer(id: u8, drain_units: i64) -> FieldLayer {
    FieldLayer {
        layer: id,
        drain: drain_units * ONE_UNIT,
        // Deeper layers are authored with more distortion and larger rewards.
        noise: FRAC_ONE / 8 * i64::from(id + 1),
        gain: FRAC_ONE / 4 * i64::from(id + 1),
        current_ids: Vec::new(),
        port_ids: Vec::new(),
    }
}

fn port(node: u32, on_layer: u8, kind: NodeKind, q_units: i64, pos: Vec2) -> PortState {
    PortState {
        node,
        layer: on_layer,
        pos,
        kind,
        q: q_units * ONE_UNIT,
        // Kinds other than `port` are established open; a Port waits to be
        // opened, which is the goal that owns the Pulse.
        open: kind != NodeKind::Port,
        upkeep_rate: 0,
        capacity: 512 * ONE_UNIT,
    }
}

fn route(id: u32, tail: u32, head: u32, capacity_units: i64) -> RouteState {
    RouteState { route: id, tail, head, capacity: capacity_units * ONE_UNIT, flow: 0, formed_step: 0 }
}

fn form(id: u8, node: u32, on_layer: u8, pos: Vec2, charge_units: i64) -> FormState {
    FormState {
        id,
        form: "thread".to_string(),
        node,
        controlled: id == 1,
        layer: on_layer,
        pos,
        vel: Vec2::default(),
        charge: charge_units * ONE_UNIT,
        reserve: 0,
        pulse_charge: 0,
        focus: false,
        route_reach: 256 * ONE_UNIT,
        forecast_depth: 0,
        steer_scale: field_game_core::state::FRAC_ONE,
        route_capacity: 32 * ONE_UNIT,
        link: None,
        trail: None,
    }
}

fn current(id: u16, on_layer: u8, period: u16) -> CurrentState {
    CurrentState {
        id,
        layer: on_layer,
        path: vec![Vec2::units(0, 0), Vec2::units(1024, 1024)],
        width: 64 * ONE_UNIT,
        strength: 2 * ONE_UNIT,
        period,
        phase: 0,
        bright: id == 1,
        active: true,
    }
}

/// Assembles a Field from its parts, deriving what a chapter would declare
/// beside them: each layer's Port and current lists, and the next identifiers.
fn assembled(
    mut layers: Vec<FieldLayer>,
    ports: Vec<PortState>,
    routes: Vec<RouteState>,
    forms: Vec<FormState>,
    currents: Vec<CurrentState>,
) -> FieldState {
    for one in &mut layers {
        one.port_ids = ports
            .iter()
            .filter(|port| port.layer == one.layer && port.kind != NodeKind::Form)
            .map(|port| port.node)
            .collect();
        one.current_ids =
            currents.iter().filter(|c| c.layer == one.layer).map(|c| c.id).collect();
    }
    let mut field = FieldState::opening();
    field.next_node_id = ports.iter().map(|port| port.node).max().unwrap_or(0) + 1;
    field.next_route_id = routes.iter().map(|route| route.route).max().unwrap_or(0) + 1;
    field.layers = layers;
    field.ports = ports;
    field.routes = routes;
    field.forms = forms;
    field.currents = currents;
    field.physical_compartment = PhysicalCompartment {
        members: field.ports.iter().map(|port| port.node).collect(),
        leak_per_exposed_contact_per_step: 0,
    };
    field
}

/// Four layers, a Node on each, one controlled Form on the shallowest, two
/// Routes, and one current: the smallest Field that exercises several layers.
fn across_layers() -> FieldState {
    let ports = vec![
        port(1, 0, NodeKind::Form, 8, Vec2::units(2048, 2048)),
        port(2, 0, NodeKind::Port, 0, Vec2::units(2100, 2048)),
        port(3, 1, NodeKind::Reserve, 64, Vec2::units(1000, 1000)),
        port(4, 2, NodeKind::Module, 16, Vec2::units(1200, 1000)),
        port(5, 3, NodeKind::Port, 40, Vec2::units(1400, 1000)),
    ];
    let field = assembled(
        vec![layer(0, 0), layer(1, 1), layer(2, 2), layer(3, 4)],
        ports,
        vec![route(1, 3, 4, 16), route(2, 4, 5, 8)],
        vec![form(1, 1, 0, Vec2::units(2048, 2048), 8)],
        vec![current(1, 0, 30)],
    );
    field::validate(&field).expect("the fixture is a valid Field");
    field
}

/// The same Field with nothing in it but its layers: no Routes and no currents,
/// so Drain stands alone and each rule can be read on its own.
fn layers_only() -> FieldState {
    let base = across_layers();
    let field =
        assembled(base.layers, base.ports, Vec::new(), base.forms, Vec::new());
    field::validate(&field).expect("the fixture is a valid Field");
    field
}

/// One step of a Field using only its authoritative physical compartment.
fn step_of(field: &mut FieldState, control: ControlState) -> StepOutcome {
    advance(field, control, FRAC_ONE, &mut Unstaged::default().staging())
}

/// A run standing on a Field under a View of the whole of it, ready to be
/// advanced by frames.
fn standing(field: FieldState) -> Run {
    let inside: Vec<u32> = field.ports.iter().map(|port| port.node).collect();
    standing_under(field, inside)
}

/// A run standing on a Field under a named inside.
fn standing_under(field: FieldState, inside: Vec<u32>) -> Run {
    let view = ViewDeclaration { inside, ..ViewDeclaration::opening() };
    let mut run = Run::start(KEY, "thread", &support::content_hash()).expect("a valid key and Form open a run");
    run.establish_field(field, view).expect("the Field is established before the first step");
    run
}

/// One frame of input, with every declared field present.
fn frame(seq: u32, steps: u16, depth_key: i8) -> String {
    steered_frame_with(seq, steps, depth_key, 0, 0)
}

/// The same, steering.
fn steered_frame(seq: u32, steps: u16, steer_x: i16, steer_y: i16) -> String {
    steered_frame_with(seq, steps, 0, steer_x, steer_y)
}

fn steered_frame_with(seq: u32, steps: u16, depth_key: i8, steer_x: i16, steer_y: i16) -> String {
    pulsing_frame_with(seq, steps, depth_key, steer_x, steer_y, false, false, 0)
}

/// One frame carrying the Pulse: the control held, let go of, or neither.
fn pulsing_frame(seq: u32, steps: u16, held: bool, release: bool) -> String {
    pulsing_frame_with(seq, steps, 0, 0, 0, held, release, 0)
}

/// One frame turning the wheel, which the depth resolution thresholds.
fn wheeled_frame(seq: u32, steps: u16, wheel: i16) -> String {
    pulsing_frame_with(seq, steps, 0, 0, 0, false, false, wheel)
}

fn pulsing_frame_with(
    seq: u32,
    steps: u16,
    depth_key: i8,
    steer_x: i16,
    steer_y: i16,
    held: bool,
    release: bool,
    wheel: i16,
) -> String {
    format!(
        "{{\"advance_steps\":{steps},\"depth_key\":{depth_key},\"inspect\":null,\"pause\":false,\
          \"pulse_held\":{held},\"pulse_release\":{release},\"seq\":{seq},\"steer_x\":{steer_x},\
          \"steer_y\":{steer_y},\"t_us\":{stamp},\"toggle_still\":false,\"wheel\":{wheel}}}",
        stamp = i64::from(seq) * 33_333
    )
}

// ---------------------------------------------------------------------------
// The locked arithmetic.
// ---------------------------------------------------------------------------

#[test]
fn the_locked_operations_round_exactly_as_the_document_writes_them() {
    // fixed_mul shifts right by 16 arithmetically, so it rounds toward negative
    // infinity — the same magnitude rounds the other way when it is negative.
    assert_eq!(fixed_mul(ONE_UNIT, ONE_UNIT), ONE_UNIT);
    assert_eq!(fixed_mul(3, ONE_UNIT / 2), 1, "1.5 raw floors to 1");
    assert_eq!(fixed_mul(-3, ONE_UNIT / 2), -2, "-1.5 raw floors to -2, not -1");
    assert_eq!(fixed_mul(-1, ONE_UNIT), -1);
    assert_eq!(fixed_mul(0, STORED_BOUND), 0);

    // fixed_div truncates toward zero, which is the other rounding of the pair.
    assert_eq!(fixed_div(ONE_UNIT, ONE_UNIT), ONE_UNIT);
    assert_eq!(fixed_div(ONE_UNIT, 3 * ONE_UNIT), 21_845, "one third truncates down");
    assert_eq!(fixed_div(-ONE_UNIT, 3 * ONE_UNIT), -21_845, "and so does its negative");
    assert_eq!(fixed_div(-3 * ONE_UNIT, 2 * ONE_UNIT), -(ONE_UNIT + ONE_UNIT / 2));

    // isqrt is the largest s with s * s <= x, over the whole input width.
    assert_eq!(isqrt(0), 0);
    assert_eq!(isqrt(1), 1);
    assert_eq!(isqrt(2), 1);
    assert_eq!(isqrt(4), 2);
    assert_eq!(isqrt(8), 2);
    assert_eq!(isqrt(u128::from(u64::MAX) * u128::from(u64::MAX)), u64::MAX);
    assert_eq!(isqrt(u128::MAX), u64::MAX, "sixty-four result bits");
    for value in 0..2048u128 {
        let root = u128::from(isqrt(value));
        assert!(root * root <= value && (root + 1) * (root + 1) > value, "isqrt({value})");
    }

    assert_eq!(clamp01(-1), 0);
    assert_eq!(clamp01(0), 0);
    assert_eq!(clamp01(FRAC_ONE), FRAC_ONE);
    assert_eq!(clamp01(FRAC_ONE + 1), FRAC_ONE);
}

#[test]
fn distance_and_adjacency_follow_the_one_locked_rule() {
    let here = Vec2::units(1000, 1000);
    assert_eq!(distance(here, 0, here, 0), 0, "d(a, a) = 0");

    // A three-four-five triangle in whole units, exactly.
    let there = Vec2::units(1003, 1004);
    assert_eq!(distance(here, 0, there, 0), 5 * ONE_UNIT);
    assert_eq!(
        distance(there, 0, here, 0),
        distance(here, 0, there, 0),
        "the rule is symmetric"
    );

    // Adjacency is the 256-unit edge, inclusive, and one raw unit past it is not.
    assert_eq!(ADJACENT_WITHIN, 256 * ONE_UNIT);
    assert!(adjacent(Vec2::units(0, 0), 0, Vec2::units(256, 0), 0));
    assert!(!adjacent(Vec2::units(0, 1), 0, Vec2::units(256, 0), 0));

    // One layer of separation is 512 units, so nothing on another layer is ever
    // adjacent however close it stands on the plane.
    assert_eq!(LAYER_SEPARATION, 512 * ONE_UNIT);
    assert_eq!(distance(here, 0, here, 1), LAYER_SEPARATION);
    assert_eq!(distance(here, 0, here, 3), 3 * LAYER_SEPARATION);
    assert!(!adjacent(here, 0, here, 1));
}

#[test]
fn the_radius_predicate_decides_the_same_thing_as_the_locked_distance_rule() {
    // The current-delivery and adjacency rules ask whether a distance is at most
    // a radius, and deciding it without taking a root is what keeps a step
    // inside its budget. It has to be the same question on every input, so it is
    // swept against the locked rule across the boundary and past it.
    let origin = Vec2::units(1000, 1000);
    for radius in [0, 1, ONE_UNIT, 63 * ONE_UNIT, 64 * ONE_UNIT, ADJACENT_WITHIN, LAYER_SEPARATION]
    {
        for dx in [-3, -1, 0, 1, 2, 63, 64, 65, 100, 256, 257, 512] {
            for dy in [0, 1, 64, 255, 256] {
                for layer in [0u8, 1, 2] {
                    let other = Vec2::new(
                        origin.x + dx * ONE_UNIT,
                        origin.y + dy * ONE_UNIT,
                    );
                    assert_eq!(
                        within(origin, 0, other, layer, radius),
                        distance(origin, 0, other, layer) <= radius,
                        "radius {radius}, offset ({dx}, {dy}) units, layer {layer}"
                    );
                }
            }
        }
    }

    // And on the raw edge either side of a radius, where the flooring root and
    // the squared comparison could most easily disagree.
    for offset in -2..=2i64 {
        let edge = Vec2::new(origin.x + ADJACENT_WITHIN + offset, origin.y);
        assert_eq!(
            within(origin, 0, edge, 0, ADJACENT_WITHIN),
            distance(origin, 0, edge, 0) <= ADJACENT_WITHIN,
            "one raw unit at a time across the adjacency edge"
        );
    }
    assert!(!within(origin, 0, origin, 0, -1), "a radius below nothing holds nothing");
}

#[test]
fn each_node_kind_starts_with_its_locked_charge() {
    assert_eq!(NodeKind::Port.starting_charge(), 0);
    assert_eq!(NodeKind::Reserve.starting_charge(), 64 * ONE_UNIT);
    assert_eq!(NodeKind::Module.starting_charge(), 16 * ONE_UNIT);
    assert_eq!(NodeKind::Form.starting_charge(), 8 * ONE_UNIT);
    for kind in [NodeKind::Port, NodeKind::Reserve, NodeKind::Module, NodeKind::Form] {
        assert_eq!(NodeKind::read(kind.name()), Some(kind));
        assert!(kind.starting_charge() >= 0, "a starting Charge is nonnegative");
    }
    assert_eq!(NodeKind::read("circuit"), None, "the kind set is closed");
}

// ---------------------------------------------------------------------------
// Conservation.
// ---------------------------------------------------------------------------

/// Recomputes the locked identity from a step's records alone, as exact
/// integers: `q(n, t) = q(n, t-1) + inflow - outflow - upkeep + e(n, t)`.
fn identity_holds(before: &FieldState, after: &FieldState, records: &StepRecords) -> bool {
    after.ports.iter().zip(before.ports.iter()).all(|(now, then)| {
        let recorded = records
            .q
            .iter()
            .find(|(node, _)| *node == now.node)
            .map_or(0, |(_, value)| *value);
        if recorded != now.q {
            return false;
        }
        let inflow: i64 = records
            .f
            .iter()
            .filter(|(id, _)| after.routes.iter().any(|r| r.route == *id && r.head == now.node))
            .map(|(_, value)| *value)
            .sum();
        let outflow: i64 = records
            .f
            .iter()
            .filter(|(id, _)| after.routes.iter().any(|r| r.route == *id && r.tail == now.node))
            .map(|(_, value)| *value)
            .sum();
        let upkeep: i64 =
            records.upkeep.iter().filter(|u| u.node == now.node).map(|u| u.v).sum();
        let exogenous: i64 =
            records.e.iter().filter(|(node, _)| *node == now.node).map(|(_, v)| *v).sum();
        now.q == then.q + inflow - outflow - upkeep + exogenous
    })
}

#[test]
fn every_step_balances_the_charge_ledger_exactly() {
    // Every rule at once, for forty steps: Routes carrying Charge, a Drain at
    // each depth, a physical compartment whose exposed member leaks, a Node standing
    // over its threshold, and a current the Form can reach.
    let mut field = across_layers();
    field.physical_compartment.members = vec![1, 3];
    field.physical_compartment.leak_per_exposed_contact_per_step = LEAK_FRAC_CAP;
    field.ports[2].capacity = 8 * ONE_UNIT;
    field.ports[2].q = 40 * ONE_UNIT;
    field.currents[0].path = vec![Vec2::units(2048, 2048), Vec2::units(2100, 2048)];
    field.currents[0].strength = 6 * ONE_UNIT;
    field.layers[0].gain = FRAC_ONE;
    field::validate(&field).expect("the fixture is a valid Field");
    let mut sinks_total = 0i64;
    let mut sources_total = 0i64;
    let mut fired = (false, false, false, false);
    let opening_total: i64 = field.ports.iter().map(|port| port.q).sum();

    for step in 1..=40 {
        let before = field.clone();
        // A depth change every eighth step, so the controlled Form pays a
        // different layer's Drain across the run.
        let control = ControlState {
            depth_move: if step % 8 == 0 { 1 } else { 0 },
            ..ControlState::default()
        };
        let outcome = advance(&mut field, control, FRAC_ONE, &mut Unstaged::default().staging());
        let ledger = &outcome.ledger;

        assert_eq!(ledger.residual(), 0, "step {step} balances with no tolerance at all");
        assert!(ledger.balanced(), "step {step} balances per Node too");
        for node in &ledger.nodes {
            assert_eq!(node.residual(), 0, "Node {} balances at step {step}", node.node);
        }
        // The whole of the change in stored Charge is what the sinks took and
        // the sources gave, and nothing else.
        assert_eq!(
            ledger.closing - ledger.opening,
            ledger.sources() - ledger.sinks()
        );
        // The records alone reproduce the identity per Node, recomputed against
        // the state before and after the step.
        assert!(identity_holds(&before, &field, &outcome.records), "step {step} records");

        fired.0 |= ledger.drain > 0;
        fired.1 |= ledger.leakage > 0;
        fired.2 |= ledger.overload > 0;
        fired.3 |= ledger.current > 0;
        assert!(
            ledger.moved > 0 || ledger.sinks() > 0 || ledger.sources() > 0,
            "step {step} moved, took, or gave something"
        );
        sinks_total += ledger.sinks();
        sources_total += ledger.sources();
    }

    assert_eq!(fired, (true, true, true, true), "every sink and the source all fired");
    let closing_total: i64 = field.ports.iter().map(|port| port.q).sum();
    assert_eq!(
        closing_total - opening_total,
        sources_total - sinks_total,
        "every unit that entered or left the Field did so through a recorded entry"
    );
    assert!(sinks_total > 0 && sources_total > 0, "the fixture both gained and lost Charge");
}

#[test]
fn the_ledger_of_a_field_holding_nothing_balances_too() {
    let mut field = FieldState::opening();
    let outcome = step_of(&mut field, ControlState::default());
    assert_eq!(outcome.ledger.residual(), 0);
    assert_eq!(outcome.ledger.opening, 0);
    assert_eq!(outcome.ledger.closing, 0);
    assert!(outcome.records.q.is_empty(), "a zero-valued entry is never written");
    assert!(outcome.records.e.is_empty());
    assert!(outcome.records.f.is_empty());
    assert!(outcome.records.z.is_empty());
    assert_eq!(field.step, 1);
}

// ---------------------------------------------------------------------------
// Drain, the per-layer difficulty parameter.
// ---------------------------------------------------------------------------

#[test]
fn a_layers_drain_removes_charge_by_depth_and_never_below_zero() {
    let mut field = layers_only();
    let depth_of = |field: &FieldState, node: u32| {
        field.ports.iter().find(|port| port.node == node).expect("the Node stands").q
    };

    let outcome = step_of(&mut field, ControlState::default());
    // The shallowest layer is authored with no Drain, so nothing at that depth
    // loses anything; each deeper layer removes its own authored amount.
    assert_eq!(depth_of(&field, 1), 8 * ONE_UNIT, "layer 0 removes nothing");
    assert_eq!(depth_of(&field, 3), 63 * ONE_UNIT, "layer 1 removes one unit");
    assert_eq!(depth_of(&field, 4), 14 * ONE_UNIT, "layer 2 removes two units");
    assert_eq!(depth_of(&field, 5), 36 * ONE_UNIT, "layer 3 removes four units");
    assert_eq!(outcome.ledger.drain, 7 * ONE_UNIT);

    // The exogenous record carries the removal, signed, and nothing else does.
    let recorded: Vec<(u32, i64)> = outcome.records.e.clone();
    assert_eq!(
        recorded,
        vec![(3, -ONE_UNIT), (4, -2 * ONE_UNIT), (5, -4 * ONE_UNIT)],
        "ascending, and never a zero-valued entry"
    );

    // A Node is floored at nothing rather than driven negative, and the failure
    // indicator names it on the step it empties.
    for _ in 0..40 {
        let outcome = step_of(&mut field, ControlState::default());
        assert!(field.ports.iter().all(|port| port.q >= 0), "stored Charge is never negative");
        assert_eq!(outcome.ledger.residual(), 0);
    }
    assert_eq!(depth_of(&field, 4), 0, "the Module on layer 2 is emptied");
    let outcome = step_of(&mut field, ControlState::default());
    assert!(outcome.records.z.contains(&4), "a Node ending a step at nothing fails");
    assert!(!outcome.records.z.contains(&3), "one that still holds Charge does not");
    assert_eq!(outcome.ledger.residual(), 0, "the floored step balances");
}

#[test]
fn the_three_layer_parameters_are_carried_and_only_drain_removes_charge() {
    let field = across_layers();
    let deepest = field.layers.last().expect("four layers");
    assert_eq!(deepest.drain, 4 * ONE_UNIT);
    assert!(deepest.noise > field.layers[0].noise, "deeper layers distort more");
    assert!(deepest.gain > field.layers[0].gain, "and reward more");
    for one in &field.layers {
        assert!((0..=FRAC_ONE).contains(&one.noise), "noise is a fraction");
        assert!((0..=FRAC_ONE).contains(&one.gain), "gain is a fraction");
    }

    // Noise distorts routes and forecasts; it removes no Charge. A Field whose
    // Drain is nothing loses nothing at all, however its Routes move Charge
    // about inside it, because Route transfer is internal.
    let mut quiet = across_layers();
    for one in &mut quiet.layers {
        one.drain = 0;
    }
    let before: i64 = quiet.ports.iter().map(|port| port.q).sum();
    let mut moved_total = 0;
    for _ in 0..10 {
        let outcome = step_of(&mut quiet, ControlState::default());
        assert_eq!(outcome.ledger.drain, 0);
        assert_eq!(outcome.ledger.sinks(), 0, "no sink took anything");
        assert_eq!(outcome.ledger.sources(), 0, "and no current reaches these Nodes");
        assert_eq!(outcome.ledger.residual(), 0);
        moved_total += outcome.ledger.moved;
    }
    assert!(moved_total > 0, "Charge did move along the Routes");
    assert_eq!(quiet.ports.iter().map(|port| port.q).sum::<i64>(), before);
}

// ---------------------------------------------------------------------------
// Routes, and what is locked about their flow.
// ---------------------------------------------------------------------------

/// A chain of Nodes on one layer with no Drain, so Route flow reads alone.
/// `holds` gives each Node's opening Charge in units; the Nodes are Reserves,
/// which are established open, and each Route joins consecutive Nodes.
fn chain(holds: &[i64], capacity_units: i64) -> FieldState {
    let ports: Vec<PortState> = holds
        .iter()
        .enumerate()
        .map(|(index, units)| {
            port(
                index as u32 + 1,
                0,
                NodeKind::Reserve,
                *units,
                Vec2::units(100 + index as i64 * 600, 100),
            )
        })
        .collect();
    let routes: Vec<RouteState> = (1..holds.len())
        .map(|index| route(index as u32, index as u32, index as u32 + 1, capacity_units))
        .collect();
    // The Route-arithmetic fixture runs under zero noise: the Noise flow
    // scale has its own tests, and the pins here are the base rule's exact
    // amounts.
    let field = quiet(assembled(vec![layer(0, 0)], ports, routes, Vec::new(), Vec::new()));
    field::validate(&field).expect("the fixture is a valid Field");
    field
}

fn charge_of(field: &FieldState, node: u32) -> Fx {
    field.ports.iter().find(|port| port.node == node).expect("the Node stands").q
}

/// The same Field with every layer's noise authored to zero: a layer with
/// effective noise 0 draws nothing and keeps the whole flow scale, so the
/// tests that pin the base rules' exact arithmetic run under it. The Noise
/// rule has its own tests.
fn quiet(mut field: FieldState) -> FieldState {
    for layer in &mut field.layers {
        layer.noise = 0;
    }
    field
}

#[test]
fn a_route_moves_charge_tail_to_head_under_its_capacity_and_the_open_gate() {
    // Capacity is the amount when both endpoints hold enough and have room.
    let mut field = chain(&[64, 0], 16);
    let outcome = step_of(&mut field, ControlState::default());
    assert_eq!(charge_of(&field, 1), 48 * ONE_UNIT);
    assert_eq!(charge_of(&field, 2), 16 * ONE_UNIT);
    assert_eq!(field.routes[0].flow, 16 * ONE_UNIT, "the flow of the completed step");
    assert_eq!(outcome.records.f, vec![(1, 16 * ONE_UNIT)]);
    assert_eq!(outcome.ledger.moved, 16 * ONE_UNIT);
    assert_eq!(outcome.ledger.nodes[0].outflow, 16 * ONE_UNIT);
    assert_eq!(outcome.ledger.nodes[1].inflow, 16 * ONE_UNIT);
    assert_eq!(outcome.ledger.sinks(), 0, "Route transfer touches no sink");
    assert_eq!(outcome.ledger.sources(), 0, "and no source");
    assert_eq!(outcome.ledger.opening, outcome.ledger.closing, "it moves, it never creates");
    assert_eq!(outcome.ledger.residual(), 0);

    // The source shortfall: a tail holding less than the capacity sends all it
    // has and no more, and is left with nothing.
    let mut field = chain(&[5, 0], 16);
    let outcome = step_of(&mut field, ControlState::default());
    assert_eq!(charge_of(&field, 1), 0);
    assert_eq!(charge_of(&field, 2), 5 * ONE_UNIT);
    assert_eq!(outcome.records.z, vec![1], "the emptied tail failed the step");

    // The destination headroom: a head near the stored-Charge cap takes only
    // the room it has, and nothing overshoots the cap.
    let mut field = chain(&[64, 0], 16);
    field.ports[1].q = NODE_CHARGE_CAP - 3;
    field.ports[1].capacity = NODE_CHARGE_CAP;
    let outcome = step_of(&mut field, ControlState::default());
    assert_eq!(charge_of(&field, 2), NODE_CHARGE_CAP, "filled exactly to the cap");
    assert_eq!(charge_of(&field, 1), 64 * ONE_UNIT - 3);
    assert_eq!(outcome.ledger.moved, 3);
    assert!(field::within_caps(&field));

    // A full head takes nothing, and the zero writes no entry.
    let outcome = step_of(&mut field, ControlState::default());
    assert_eq!(outcome.ledger.moved, 0);
    assert!(outcome.records.f.is_empty(), "a Route that carried nothing records nothing");

    // The open gate: either endpoint closed and the Route moves nothing.
    for closed in [0, 1] {
        let mut field = chain(&[64, 0], 16);
        field.ports[closed].open = false;
        let outcome = step_of(&mut field, ControlState::default());
        assert_eq!(outcome.ledger.moved, 0, "a Route with a closed endpoint moves nothing");
        assert_eq!(charge_of(&field, 1), 64 * ONE_UNIT);
    }

    // A Route from a Node to itself takes Charge out and puts it back.
    let mut field = chain(&[64, 0], 16);
    field.routes[0].head = 1;
    let outcome = step_of(&mut field, ControlState::default());
    assert_eq!(charge_of(&field, 1), 64 * ONE_UNIT);
    assert_eq!(outcome.ledger.moved, 16 * ONE_UNIT);
    assert_eq!(outcome.ledger.residual(), 0);
}

#[test]
fn an_ascending_pass_of_routes_carries_charge_several_hops_in_one_step() {
    // Routes 1 and 2 stand in the order 1 → 2 → 3, so one pass carries Charge
    // the whole way: Route 2 reads what Route 1 has already moved.
    let mut field = chain(&[64, 0, 0], 16);
    let outcome = step_of(&mut field, ControlState::default());
    assert_eq!(charge_of(&field, 1), 48 * ONE_UNIT);
    assert_eq!(charge_of(&field, 2), 0, "what arrived left again in the same pass");
    assert_eq!(charge_of(&field, 3), 16 * ONE_UNIT);
    assert_eq!(outcome.ledger.moved, 32 * ONE_UNIT, "two hops moved 16 units each");
    assert_eq!(outcome.records.f, vec![(1, 16 * ONE_UNIT), (2, 16 * ONE_UNIT)]);
    assert_eq!(outcome.ledger.residual(), 0);

    // The same Routes numbered the other way round carry one hop per step,
    // which is what makes the pass order part of the contract.
    let mut field = chain(&[64, 0, 0], 16);
    field.routes[0] = route(1, 2, 3, 16);
    field.routes[1] = route(2, 1, 2, 16);
    let outcome = step_of(&mut field, ControlState::default());
    assert_eq!(charge_of(&field, 2), 16 * ONE_UNIT, "the later Route filled it after the earlier read");
    assert_eq!(charge_of(&field, 3), 0);
    assert_eq!(outcome.ledger.moved, 16 * ONE_UNIT);
}

#[test]
fn a_flow_and_a_capacity_outside_their_locked_bounds_are_refused() {
    let mut field = across_layers();
    field.routes[0].flow = field.routes[0].capacity;
    field::validate(&field).expect("a flow at capacity stands");
    field.routes[0].flow = field.routes[0].capacity + 1;
    assert_eq!(refusal(&field), Code::Validation, "a flow above capacity is refused");
    field.routes[0].flow = -1;
    assert_eq!(refusal(&field), Code::Validation, "and so is a negative flow");
    field.routes[0].flow = 0;
    field.routes[0].capacity = ROUTE_CAPACITY_CAP;
    field::validate(&field).expect("capacity at the cap stands");
    field.routes[0].capacity = ROUTE_CAPACITY_CAP + 1;
    assert_eq!(refusal(&field), Code::Capacity, "capacity past its locked cap is a cap crossing");
}

#[test]
fn an_overloaded_node_throttles_its_inflow_and_sheds_a_quarter_of_its_excess() {
    // The threshold is what overload is measured against, not a second cap on
    // stored Charge: standing above it is a valid Field, and that is the whole
    // of what being overloaded is.
    let mut field = chain(&[64, 0], 16);
    field.ports[1].capacity = 8 * ONE_UNIT;
    field.ports[1].q = 12 * ONE_UNIT;
    field::validate(&field).expect("stored Charge above the threshold stands");

    // Inflow throttle: the head is overloaded when the pass reaches it, so the
    // Route moves under half its capacity. Excess decay then takes a quarter of
    // what stands above the threshold, after the flow has landed.
    let outcome = step_of(&mut field, ControlState::default());
    assert_eq!(outcome.ledger.moved, 8 * ONE_UNIT, "capacity halved, exactly");
    // 12 + 8 = 20 units held against an 8-unit threshold: a 12-unit excess,
    // three units of which decay.
    assert_eq!(charge_of(&field, 2), 17 * ONE_UNIT);
    assert_eq!(outcome.ledger.overload, 3 * ONE_UNIT);
    assert_eq!(outcome.records.e, vec![(2, -3 * ONE_UNIT)]);
    assert_eq!(outcome.ledger.residual(), 0, "the sink balances the step exactly");

    // Tail overload never throttles: a congested Node sends Charge away at full
    // capacity, which is what lets a circuit recover.
    let mut field = chain(&[64, 0], 16);
    field.ports[0].capacity = 8 * ONE_UNIT;
    let outcome = step_of(&mut field, ControlState::default());
    assert_eq!(outcome.ledger.moved, 16 * ONE_UNIT, "the full capacity left the tail");

    // Recovery is memoryless: the moment a Node holds no more than its
    // threshold, neither effect touches it again.
    let mut field = chain(&[0, 9], 16);
    field.ports[1].capacity = 8 * ONE_UNIT;
    let mut decayed = Vec::new();
    for _ in 0..4 {
        let outcome = step_of(&mut field, ControlState::default());
        decayed.push(outcome.ledger.overload);
    }
    // An excess of one unit sheds a quarter, then the remainder of the quarter,
    // and stops once nothing stands above the threshold.
    assert_eq!(decayed[0], ONE_UNIT / 4);
    assert!(decayed[3] < decayed[0], "the excess shrinks toward the threshold");
    assert!(charge_of(&field, 2) > 8 * ONE_UNIT, "and never falls below it");

    // An excess below four raw units is below the rule's resolution and stands.
    let mut field = chain(&[0, 0], 16);
    field.ports[1].capacity = 8 * ONE_UNIT;
    field.ports[1].q = 8 * ONE_UNIT + 3;
    let outcome = step_of(&mut field, ControlState::default());
    assert_eq!(outcome.ledger.overload, 0, "a quarter of three raw units floors to nothing");
    assert_eq!(charge_of(&field, 2), 8 * ONE_UNIT + 3);
    assert!(outcome.records.e.is_empty(), "and no zero-valued entry is written");
}

#[test]
fn the_stored_charge_cap_is_a_cap_and_no_step_crosses_it() {
    let mut field = across_layers();
    field.ports[2].q = NODE_CHARGE_CAP;
    field::validate(&field).expect("stored Charge at its cap stands");
    field.ports[2].q = NODE_CHARGE_CAP + 1;
    assert_eq!(refusal(&field), Code::Capacity, "past the cap is a cap crossing");

    let mut field = across_layers();
    field.ports[2].q = NODE_CHARGE_CAP;
    for _ in 0..8 {
        step_of(&mut field, ControlState::default());
        assert!(field::within_caps(&field), "no quantity leaves its cap across a step");
    }
}

// ---------------------------------------------------------------------------
// Boundary leakage.
// ---------------------------------------------------------------------------

/// Five Nodes on one layer, no Drain, no Routes: exposure comes from adjacency
/// alone, which the positions below make exact. Nodes 1, 2, 3 stand within 256
/// units of each other; 4 stands beside 3 only; 5 stands alone.
fn exposed(leak_frac: Frac) -> FieldState {
    let ports = vec![
        port(1, 0, NodeKind::Reserve, 64, Vec2::units(1000, 1000)),
        port(2, 0, NodeKind::Reserve, 64, Vec2::units(1100, 1000)),
        port(3, 0, NodeKind::Reserve, 64, Vec2::units(1200, 1000)),
        port(4, 0, NodeKind::Reserve, 64, Vec2::units(1300, 1000)),
        port(5, 0, NodeKind::Reserve, 64, Vec2::units(3000, 3000)),
    ];
    let mut field = assembled(vec![layer(0, 0)], ports, Vec::new(), Vec::new(), Vec::new());
    field.physical_compartment.leak_per_exposed_contact_per_step = leak_frac;
    field::validate(&field).expect("the fixture is a valid Field");
    field
}

#[test]
fn boundary_leakage_takes_the_exposure_share_of_every_shell_member() {
    // The physical member set is {1, 2}. Node 1 is adjacent to 2 (a member) and 3 (a
    // non-member): one exposed link. Node 2 is adjacent to 1, 3, and 4 — 3 and 4
    // are non-members, so two exposed links.
    let mut field = exposed(LEAK_FRAC_CAP);
    field.physical_compartment.members = vec![1, 2];
    assert!(field::nodes_adjacent(&field, 2, 4), "200 units apart, inside the rule");
    assert!(!field::nodes_adjacent(&field, 1, 4), "300 units apart, outside it");
    let outcome = advance(&mut field, ControlState::default(), FRAC_ONE, &mut Unstaged::default().staging());

    // One eighth of 64 units, and two eighths of 64 units.
    assert_eq!(charge_of(&field, 1), 56 * ONE_UNIT);
    assert_eq!(charge_of(&field, 2), 48 * ONE_UNIT);
    assert_eq!(charge_of(&field, 3), 64 * ONE_UNIT, "a non-member leaks nothing");
    assert_eq!(charge_of(&field, 5), 64 * ONE_UNIT, "and neither does an unexposed Node");
    assert_eq!(outcome.ledger.leakage, 24 * ONE_UNIT);
    assert_eq!(
        outcome.records.e,
        vec![(1, -8 * ONE_UNIT), (2, -16 * ONE_UNIT)],
        "ascending, and carried as the exogenous term the framework already accounts"
    );
    assert_eq!(outcome.ledger.residual(), 0, "the sink balances the step exactly");

    // An interior member — one with no non-member neighbor — leaks nothing.
    let mut field = exposed(LEAK_FRAC_CAP);
    field.physical_compartment.members = vec![1, 2, 3, 4];
    let outcome = advance(&mut field, ControlState::default(), FRAC_ONE, &mut Unstaged::default().staging());
    assert_eq!(charge_of(&field, 1), 64 * ONE_UNIT, "Node 1 touches only members");
    assert_eq!(outcome.ledger.leakage, 0, "and 5 stands beside nobody at all");

    // A Route to a non-member exposes a member the same way an adjacency does,
    // and a neighbor reached both ways counts once.
    let mut field = exposed(LEAK_FRAC_CAP);
    field.physical_compartment.members = vec![1, 2, 3, 4];
    field.next_route_id = 3;
    field.routes = vec![route(1, 1, 5, 16), route(2, 5, 1, 16)];
    let outcome = advance(&mut field, ControlState::default(), FRAC_ONE, &mut Unstaged::default().staging());
    assert_eq!(outcome.ledger.leakage, 8 * ONE_UNIT, "one distinct non-member neighbor, once");
    assert_eq!(charge_of(&field, 1), 56 * ONE_UNIT);
}

#[test]
fn leakage_is_nothing_at_the_empty_and_whole_field_edges_and_never_more_than_is_held() {
    // An empty physical compartment: no member, so nothing leaks however wide
    // the parameter.
    let mut field = exposed(LEAK_FRAC_CAP);
    field.physical_compartment.members.clear();
    let before: Fx = field.ports.iter().map(|port| port.q).sum();
    let outcome = advance(&mut field, ControlState::default(), FRAC_ONE, &mut Unstaged::default().staging());
    assert_eq!(outcome.ledger.leakage, 0);
    assert_eq!(field.ports.iter().map(|port| port.q).sum::<Fx>(), before);

    // A physical compartment equal to the whole Field: no non-member exists, so no
    // member is exposed and nothing leaks.
    let mut field = exposed(LEAK_FRAC_CAP);
    field.physical_compartment.members = vec![1, 2, 3, 4, 5];
    let outcome = advance(&mut field, ControlState::default(), FRAC_ONE, &mut Unstaged::default().staging());
    assert_eq!(outcome.ledger.leakage, 0);
    assert_eq!(field.ports.iter().map(|port| port.q).sum::<Fx>(), before);

    // A parameter of nothing leaks nothing, exposed or not.
    let mut field = exposed(0);
    field.physical_compartment.members = vec![1, 2];
    let outcome = advance(&mut field, ControlState::default(), FRAC_ONE, &mut Unstaged::default().staging());
    assert_eq!(outcome.ledger.leakage, 0);

    // Exposure enough to reach the whole of what is held takes exactly that and
    // never more: the rate is held at one, so the Node empties and no more.
    let mut field = exposed(LEAK_FRAC_CAP);
    field.physical_compartment.members = vec![1];
    let outcome = advance(&mut field, ControlState::default(), FRAC_ONE, &mut Unstaged::default().staging());
    // Node 1 stands beside 2 and 3, two non-members at an eighth each.
    assert_eq!(charge_of(&field, 1), 48 * ONE_UNIT);
    assert_eq!(outcome.ledger.leakage, 16 * ONE_UNIT);

    let mut field = exposed(LEAK_FRAC_CAP);
    field.physical_compartment.members = vec![1];
    // Eight exposed links would be the whole of it; the rule holds the rate at
    // one, so the member empties in a single step and the step still balances.
    for extra in 0..6u32 {
        let node = 6 + extra;
        field.ports.push(port(node, 0, NodeKind::Reserve, 8, Vec2::units(1050, 1000)));
        field.next_node_id = node + 1;
    }
    field.layers[0].port_ids = (1..=11).collect();
    field::validate(&field).expect("a wider fixture is still a valid Field");
    let outcome = advance(&mut field, ControlState::default(), FRAC_ONE, &mut Unstaged::default().staging());
    assert_eq!(charge_of(&field, 1), 0, "eight eighths is the whole of it");
    assert_eq!(outcome.ledger.leakage, 64 * ONE_UNIT);
    assert!(outcome.records.z.contains(&1), "a Node that ends with nothing failed");
    assert_eq!(outcome.ledger.residual(), 0);

    // The parameter has a locked range, and past it the Field is refused.
    let mut field = exposed(LEAK_FRAC_CAP);
    field.physical_compartment.leak_per_exposed_contact_per_step = LEAK_FRAC_CAP + 1;
    assert_eq!(refusal(&field), Code::Validation);
    field.physical_compartment.leak_per_exposed_contact_per_step = -1;
    assert_eq!(refusal(&field), Code::Validation);
}

#[test]
fn a_compartment_edit_causes_the_first_leakage_divergence_and_both_ledgers_balance() {
    let mut standing = exposed(LEAK_FRAC_CAP);
    standing.physical_compartment.members = vec![1, 2, 3, 4];
    let mut reshaped = standing.clone();
    reshaped.physical_compartment.members = vec![1];

    let unchanged = advance(
        &mut standing,
        ControlState::default(),
        FRAC_ONE,
        &mut Unstaged::default().staging(),
    );
    let changed = advance(
        &mut reshaped,
        ControlState::default(),
        FRAC_ONE,
        &mut Unstaged::default().staging(),
    );

    assert_eq!(standing.step, 1);
    assert_eq!(reshaped.step, 1, "the divergence is on the first post-edit step");
    assert_eq!(unchanged.ledger.leakage, 0, "the standing compartment has no exposed member");
    assert_eq!(changed.ledger.leakage, 16 * ONE_UNIT, "the reshaped member has two contacts");
    assert_ne!(standing.written(), reshaped.written(), "the physical intervention changes state");
    assert_eq!(unchanged.ledger.residual(), 0);
    assert_eq!(changed.ledger.residual(), 0);
    assert!(unchanged.ledger.balanced() && changed.ledger.balanced());
}

// ---------------------------------------------------------------------------
// Current delivery.
// ---------------------------------------------------------------------------

#[test]
fn a_current_delivers_its_strength_scaled_by_gain_across_the_nodes_standing_in_it() {
    // Three Nodes on one layer, two of them inside the current's width of its
    // nearest path point, and a layer whose gain is the whole of the strength.
    let ports = vec![
        port(1, 0, NodeKind::Reserve, 0, Vec2::units(1000, 1000)),
        port(2, 0, NodeKind::Reserve, 0, Vec2::units(1040, 1000)),
        port(3, 0, NodeKind::Reserve, 0, Vec2::units(2000, 2000)),
    ];
    let mut whole_gain = layer(0, 0);
    whole_gain.gain = FRAC_ONE;
    let mut flow = current(1, 0, 30);
    flow.path = vec![Vec2::units(1000, 1000), Vec2::units(1010, 1000)];
    flow.width = 64 * ONE_UNIT;
    // An odd raw total, so the remainder has somewhere to go.
    flow.strength = 9 * ONE_UNIT + 1;
    let mut field =
        assembled(vec![whole_gain], ports, Vec::new(), Vec::new(), vec![flow]);
    field::validate(&field).expect("the fixture is a valid Field");

    let outcome = step_of(&mut field, ControlState::default());
    // An odd raw total across two recipients: the base share each, and the one
    // raw unit left over to the smaller identifier, so the shares sum exactly.
    let emitted = 9 * ONE_UNIT + 1;
    let base = emitted / 2;
    assert_eq!(charge_of(&field, 1), base + 1);
    assert_eq!(charge_of(&field, 2), base);
    assert_eq!(charge_of(&field, 3), 0, "a Node outside the width receives nothing");
    assert_eq!(charge_of(&field, 1) + charge_of(&field, 2), emitted, "the shares sum exactly");
    assert_eq!(outcome.ledger.current, emitted);
    assert_eq!(outcome.ledger.sources(), emitted);
    assert_eq!(outcome.records.e, vec![(1, base + 1), (2, base)]);
    assert_eq!(outcome.ledger.residual(), 0);

    // gain is the depth-scaling hook: a quarter of the gain delivers a quarter
    // of the strength, floored by the locked multiply.
    field.layers[0].gain = FRAC_ONE / 4;
    let outcome = step_of(&mut field, ControlState::default());
    assert_eq!(outcome.ledger.current, emitted / 4, "the locked multiply floors");

    // An inactive current delivers nothing, and its phase still turns.
    field.currents[0].active = false;
    let before = field.currents[0].phase;
    let outcome = step_of(&mut field, ControlState::default());
    assert_eq!(outcome.ledger.current, 0);
    assert_eq!(field.currents[0].phase, before + 1, "the phase counter is not the gate");
}

#[test]
fn the_vertex_rule_decides_who_stands_in_a_current_and_a_full_node_refuses() {
    let ports = vec![
        port(1, 0, NodeKind::Reserve, 0, Vec2::units(1000, 1064)),
        port(2, 0, NodeKind::Reserve, 0, Vec2::units(1000, 1000)),
        port(3, 1, NodeKind::Reserve, 0, Vec2::units(1000, 1000)),
    ];
    let mut whole_gain = layer(0, 0);
    whole_gain.gain = FRAC_ONE;
    let mut flow = current(1, 0, 30);
    flow.path = vec![Vec2::units(1000, 1000), Vec2::units(1001, 1000)];
    flow.width = 64 * ONE_UNIT;
    flow.strength = 8 * ONE_UNIT;
    let mut field = assembled(
        vec![whole_gain, layer(1, 0)],
        ports,
        Vec::new(),
        Vec::new(),
        vec![flow],
    );
    field::validate(&field).expect("the fixture is a valid Field");

    // Exactly at the width, a Node stands in the current.
    let outcome = step_of(&mut field, ControlState::default());
    assert_eq!(outcome.ledger.current, 8 * ONE_UNIT);
    assert_eq!(charge_of(&field, 1), 4 * ONE_UNIT, "64 units away is inside a 64-unit width");
    assert_eq!(charge_of(&field, 3), 0, "another layer never stands in it");

    // One raw unit further and it does not.
    let mut field = field;
    field.ports[0].pos = Vec2::new(1000 * ONE_UNIT, 1064 * ONE_UNIT + 1);
    field.forms.clear();
    let outcome = step_of(&mut field, ControlState::default());
    assert_eq!(outcome.ledger.current, 8 * ONE_UNIT, "all of it to the one recipient left");
    assert_eq!(outcome.records.e, vec![(2, 8 * ONE_UNIT)]);

    // Charge a full Node refuses is never emitted: the current delivers less,
    // and only what was accepted enters the ledger.
    field.ports[1].q = NODE_CHARGE_CAP - ONE_UNIT;
    field.ports[1].capacity = NODE_CHARGE_CAP;
    let outcome = step_of(&mut field, ControlState::default());
    assert_eq!(outcome.ledger.current, ONE_UNIT, "only the room it had");
    assert_eq!(charge_of(&field, 2), NODE_CHARGE_CAP);
    assert_eq!(outcome.ledger.residual(), 0);
    assert!(field::within_caps(&field));

    // A current nobody stands in emits nothing at all.
    let mut field = field;
    field.currents[0].width = 0;
    field.ports[0].pos = Vec2::units(3000, 3000);
    field.ports[1].pos = Vec2::units(2000, 2000);
    let outcome = step_of(&mut field, ControlState::default());
    assert_eq!(outcome.ledger.current, 0);
    assert_eq!(outcome.ledger.residual(), 0);

    // The strength cap is a capacity row, and past it the Field is refused.
    field.currents[0].strength = CURRENT_STRENGTH_CAP;
    field::validate(&field).expect("strength at its cap stands");
    field.currents[0].strength = CURRENT_STRENGTH_CAP + 1;
    assert_eq!(refusal(&field), Code::Capacity);
}

// ---------------------------------------------------------------------------
// Bounded values, at every cap at once.
// ---------------------------------------------------------------------------

fn refusal(field: &FieldState) -> Code {
    field::validate(field).expect_err("the Field is refused").code()
}

/// A Field standing at every locked cap: 8 layers, 256 Nodes, 512 Routes, 32
/// currents, 8 Forms, stored Charge at its cap, Route capacity at its cap, and
/// each layer's Drain at the widest value a stored quantity may carry.
fn at_every_cap() -> FieldState {
    let layers: Vec<FieldLayer> = (0..LAYERS_PER_CHAPTER as u8)
        .map(|id| FieldLayer {
            layer: id,
            drain: STORED_BOUND - 1,
            noise: FRAC_ONE,
            gain: FRAC_ONE,
            current_ids: Vec::new(),
            port_ids: Vec::new(),
        })
        .collect();

    let mut ports = Vec::new();
    for index in 0..NODES_PER_RUN {
        let node = index as u32 + 1;
        let on_layer = (index % LAYERS_PER_CHAPTER) as u8;
        // The last Nodes are the Forms', so every Form has one to stand on.
        let kind =
            if index >= NODES_PER_RUN - FORMS_PER_RUN { NodeKind::Form } else { NodeKind::Port };
        let pos = Vec2::units(((index % 16) * 256) as i64, ((index / 16) * 256) as i64);
        ports.push(PortState {
            node,
            layer: on_layer,
            pos,
            kind,
            q: NODE_CHARGE_CAP,
            open: true,
            upkeep_rate: 0,
            capacity: NODE_CHARGE_CAP,
        });
    }

    let routes: Vec<RouteState> = (0..ROUTES_PER_RUN)
        .map(|index| RouteState {
            route: index as u32 + 1,
            tail: (index % NODES_PER_RUN) as u32 + 1,
            head: ((index + 1) % NODES_PER_RUN) as u32 + 1,
            capacity: ROUTE_CAPACITY_CAP,
            flow: 0,
            formed_step: 0,
        })
        .collect();

    let forms: Vec<FormState> = (0..FORMS_PER_RUN)
        .map(|index| {
            let node = ports[NODES_PER_RUN - FORMS_PER_RUN + index].clone();
            FormState {
                id: index as u8 + 1,
                form: "chorus".to_string(),
                node: node.node,
                controlled: true,
                layer: node.layer,
                pos: node.pos,
                // The widest velocity a stored quantity may carry, in the
                // widest form it is written in — both components negative — and
                // the plane's own width then holds the position inside.
                vel: Vec2::new(-(STORED_BOUND - 1), -(STORED_BOUND - 1)),
                charge: node.q,
                reserve: NODE_CHARGE_CAP,
                pulse_charge: FRAC_ONE,
                focus: true,
                route_reach: STORED_BOUND - 1,
                forecast_depth: FORECAST_DEPTH_CAP,
                // The two carried abilities and the scale, each at the widest
                // value it may hold: what the payload figure below measures is
                // the widest a Form is ever written, and an ability standing
                // unread by the rules — every Form here is controlled — is
                // written exactly as one that is read.
                steer_scale: field::STEER_SCALE_HIGH,
                route_capacity: ROUTE_CAPACITY_CAP,
                link: Some(field::LinkState {
                    offset: Vec2::new(-(PLANE_SPAN - 1), -(PLANE_SPAN - 1)),
                    separation: STORED_BOUND - 1,
                }),
                trail: Some(field::TrailState {
                    period: *field::TRAIL_PERIOD.end(),
                    delay: *field::TRAIL_DELAY.end(),
                    radius: field::TRAIL_RADIUS_CAP,
                    magnitude: field::TRAIL_MAGNITUDE_CAP,
                }),
            }
        })
        .collect();

    let currents: Vec<CurrentState> = (0..CURRENTS_PER_CHAPTER)
        .map(|index| CurrentState {
            id: index as u16 + 1,
            layer: (index % LAYERS_PER_CHAPTER) as u8,
            path: (0..64).map(|point| Vec2::units(point * 64, point * 63)).collect(),
            width: STORED_BOUND - 1,
            strength: CURRENT_STRENGTH_CAP,
            period: 1,
            phase: 0,
            bright: index == 0,
            active: true,
        })
        .collect();

    let mut field = assembled(layers, ports, routes, forms, currents);
    field.physical_compartment.leak_per_exposed_contact_per_step = LEAK_FRAC_CAP;
    // The Trail queue at its own cap, every entry written as wide as one is
    // ever written: the furthest due step, the widest position, and the whole
    // magnitude an entry may carry.
    field.pending = (0..field::PENDING_TRAILS)
        .map(|index| field::PendingTrail {
            form: FORMS_PER_RUN as u8,
            layer: (index % LAYERS_PER_CHAPTER) as u8,
            pos: Vec2::new(PLANE_SPAN - 1, PLANE_SPAN - 1),
            due: u32::MAX,
            magnitude: field::TRAIL_MAGNITUDE_CAP,
        })
        .collect();
    field::validate(&field).expect("a Field at every cap is a valid Field");
    field
}

#[test]
fn no_quantity_leaves_its_locked_cap_under_adversarial_parameters() {
    let mut field = at_every_cap();
    assert_eq!(field.ports.len(), NODES_PER_RUN);
    assert_eq!(field.routes.len(), ROUTES_PER_RUN);
    assert_eq!(field.layers.len(), LAYERS_PER_CHAPTER);
    assert_eq!(field.currents.len(), CURRENTS_PER_CHAPTER);
    assert_eq!(field.forms.len(), FORMS_PER_RUN);

    // Overflow checks are on in every profile, so a quantity that wrapped would
    // trap here rather than pass.
    for step in 1..=12 {
        let outcome = step_of(&mut field, ControlState { depth_move: 1, ..ControlState::default() });
        assert_eq!(outcome.ledger.residual(), 0, "step {step} balances at every cap");
        assert!(field::within_caps(&field), "step {step} leaves nothing outside its cap");
        field::validate(&field).expect("a stepped Field is still a valid Field");
    }

    // At that Drain every Node is emptied in the pressure phase, and delivery
    // then refills it — delivery sits after the pressure phase, so what arrived
    // this step is exactly what each Node holds. Every Node was emptied first,
    // so every emitted unit found room: the delivered total is the whole of what
    // the 32 currents emitted, which is what makes the remainder split exact.
    let outcome = step_of(&mut field, ControlState::default());
    let emitted = fixed_mul(CURRENT_STRENGTH_CAP, FRAC_ONE);
    assert_eq!(
        outcome.ledger.current,
        emitted * CURRENTS_PER_CHAPTER as i64,
        "every raw unit emitted was delivered, with nothing lost to the split"
    );
    assert_eq!(outcome.records.q.len(), NODES_PER_RUN, "every Node records its Charge");
    assert!(outcome.records.z.is_empty(), "and none of them ended the step at nothing");
    // Each layer's four currents split across the Nodes standing on it, the
    // Forms among them — by now every controlled Form has descended to the
    // deepest layer, so the layer populations are not the ones authored.
    for port in &field.ports {
        let standing =
            field.ports.iter().filter(|other| other.layer == port.layer).count() as i64;
        let flows = field.currents.iter().filter(|c| c.layer == port.layer).count() as i64;
        let base = emitted / standing;
        assert!(
            (flows * base..=flows * (base + 1)).contains(&port.q),
            "Node {} on layer {} holds its share of {flows} currents",
            port.node,
            port.layer
        );
    }
    assert_eq!(outcome.ledger.residual(), 0);
    assert!(field::within_caps(&field));
}

#[test]
fn one_quantity_past_its_cap_is_the_capacity_envelope() {
    let base = at_every_cap();

    let mut layers = base.clone();
    layers.layers.push(layer(MAX_LAYER, 0));
    assert_eq!(refusal(&layers), Code::Capacity, "a ninth layer");

    let mut nodes = base.clone();
    let last = nodes.ports.last().expect("Nodes stand").clone();
    nodes.next_node_id += 1;
    nodes.ports.push(PortState { node: last.node + 1, ..last });
    assert_eq!(refusal(&nodes), Code::Capacity, "a two-hundred-and-fifty-seventh Node");

    let mut routes = base.clone();
    let tail = routes.routes.last().expect("Routes stand").clone();
    routes.next_route_id += 1;
    routes.routes.push(RouteState { route: tail.route + 1, ..tail });
    assert_eq!(refusal(&routes), Code::Capacity, "a five-hundred-and-thirteenth Route");

    let mut currents = base.clone();
    let flow = currents.currents.last().expect("currents stand").clone();
    currents.currents.push(CurrentState { id: flow.id + 1, ..flow });
    assert_eq!(refusal(&currents), Code::Capacity, "a thirty-third current");

    let mut forms = base.clone();
    let standing = forms.forms.last().expect("Forms stand").clone();
    forms.forms.push(FormState { id: standing.id + 1, ..standing });
    assert_eq!(refusal(&forms), Code::Capacity, "a ninth Form");

    let mut drawn = base.clone();
    for step in 0..=DRAWN_RETAINED as u32 {
        drawn.boundaries.drawn.push(field_game_core::field::DrawnBoundary {
            members: vec![1, 2],
            step,
        });
    }
    assert_eq!(refusal(&drawn), Code::Capacity, "a thirty-third retained drawn Boundary");
}

#[test]
fn a_field_outside_its_locked_shape_is_refused() {
    // A crossed capacity row is the `capacity` envelope, an identifier naming
    // nothing is `not_found`, and every other shape, range, closed set, or
    // ordering failure is `validation`.
    let refusals: Vec<(&str, FieldState)> = vec![
        ("descending Nodes", {
            let mut field = across_layers();
            field.ports.swap(0, 1);
            field
        }),
        ("a Node on no layer", {
            let mut field = across_layers();
            field.ports[1].layer = 6;
            field
        }),
        // A gap in the layer list would put a Form one depth change away from a
        // layer that does not stand, so the list is contiguous from 0.
        ("a sparse layer list", {
            let ports = vec![
                port(1, 0, NodeKind::Form, 8, Vec2::units(1000, 1000)),
                port(2, 2, NodeKind::Reserve, 8, Vec2::units(1200, 1000)),
                port(3, 4, NodeKind::Reserve, 8, Vec2::units(1400, 1000)),
            ];
            assembled(
                vec![layer(0, 0), layer(2, 2), layer(4, 4)],
                ports,
                Vec::new(),
                vec![form(1, 1, 0, Vec2::units(1000, 1000), 8)],
                Vec::new(),
            )
        }),
        ("a layer list that does not open at 0", {
            let ports = vec![port(1, 1, NodeKind::Reserve, 8, Vec2::units(1000, 1000))];
            assembled(vec![layer(1, 1)], ports, Vec::new(), Vec::new(), Vec::new())
        }),
        ("a position outside the plane", {
            let mut field = across_layers();
            field.ports[1].pos = Vec2::new(PLANE_SPAN, 0);
            field
        }),
        ("a Route naming no Node", {
            let mut field = across_layers();
            field.routes[0].head = 99;
            field
        }),
        ("a Form on a Node of another kind", {
            let mut field = across_layers();
            field.forms[0].node = 2;
            field
        }),
        ("two Forms on one Node", {
            let mut field = across_layers();
            let standing = field.forms[0].clone();
            field.forms.push(FormState { id: standing.id + 1, ..standing });
            field
        }),
        ("a Form whose Node does not mirror it", {
            let mut field = across_layers();
            field.forms[0].pos = Vec2::units(1, 1);
            field
        }),
        ("a current path of sixty-five points", {
            let mut field = across_layers();
            field.currents[0].path = (0..65).map(|point| Vec2::units(point, point)).collect();
            field
        }),
        ("a current path of one point", {
            let mut field = across_layers();
            field.currents[0].path.truncate(1);
            field
        }),
        ("a current period of nothing", {
            let mut field = across_layers();
            field.currents[0].period = 0;
            field
        }),
        ("a phase outside its period", {
            let mut field = across_layers();
            field.currents[0].phase = 30;
            field
        }),
        ("a Form outside the closed set", {
            let mut field = across_layers();
            field.forms[0].form = "spiral".to_string();
            field
        }),
        ("a noise parameter past one", {
            let mut field = across_layers();
            field.layers[0].noise = FRAC_ONE + 1;
            field
        }),
        ("a drawn Boundary with no members", {
            let mut field = across_layers();
            field.boundaries.drawn.push(field_game_core::field::DrawnBoundary {
                members: Vec::new(),
                step: 0,
            });
            field
        }),
        ("a Boundary whose members descend", {
            let mut field = across_layers();
            field.boundaries.authored.push(vec![3, 1]);
            field
        }),
    ];

    for (reason, field) in refusals {
        let code = refusal(&field);
        assert!(
            matches!(code, Code::Validation | Code::NotFound),
            "{reason} is refused as validation or not_found, not {}",
            code.name()
        );
    }

    // A forecast depth past its range is a capacity row rather than a shape.
    let mut deep = across_layers();
    deep.forms[0].forecast_depth = FORECAST_DEPTH_CAP + 1;
    assert_eq!(refusal(&deep), Code::Capacity);

    // A Node of a kind other than `port` that is not open cannot be established,
    // though it is a valid Field for every other purpose.
    let mut closed = across_layers();
    closed.ports[2].open = false;
    field::validate(&closed).expect("closing a Node is not a shape failure");
    assert_eq!(
        field::establishable(&closed).expect_err("but it cannot be established").code(),
        Code::Validation
    );
    field::establishable(&across_layers()).expect("the fixture establishes");
}

// ---------------------------------------------------------------------------
// Movement: depth, position, and the adjacency a moving Form recomputes.
// ---------------------------------------------------------------------------

#[test]
fn a_controlled_form_moves_between_layers_and_may_always_retreat_upward() {
    let mut field = across_layers();
    let depth = |field: &FieldState| field.forms[0].layer;
    let node_depth =
        |field: &FieldState| field.ports.iter().find(|p| p.node == 1).expect("the Node").layer;

    assert_eq!(depth(&field), 0);
    for expected in 1..=3u8 {
        step_of(&mut field, ControlState { depth_move: 1, ..ControlState::default() });
        assert_eq!(depth(&field), expected);
        assert_eq!(node_depth(&field), expected, "the Form's Node stands at the same depth");
    }
    // The deepest authored layer is the floor of the descent.
    step_of(&mut field, ControlState { depth_move: 1, ..ControlState::default() });
    assert_eq!(depth(&field), 3);

    // Retreat upward is always allowed, and the shallowest layer is its ceiling.
    for expected in [2u8, 1, 0] {
        step_of(&mut field, ControlState { depth_move: -1, ..ControlState::default() });
        assert_eq!(depth(&field), expected);
    }
    step_of(&mut field, ControlState { depth_move: -1, ..ControlState::default() });
    assert_eq!(depth(&field), 0);

    // A Form that is not controlled stays where it is.
    let mut field = across_layers();
    field.forms[0].controlled = false;
    step_of(&mut field, ControlState { depth_move: 1, ..ControlState::default() });
    assert_eq!(depth(&field), 0);
}

#[test]
fn upward_retreat_stands_open_the_moment_the_cooldown_ends_at_every_depth() {
    // The same claim as above, read through the whole path a player's gesture
    // takes: frames, the wheel's accumulated threshold, the cooldown, and the
    // step that records the change. Nothing gates the way up differently from
    // the way down — the cooldown is symmetric, and it is the only thing that
    // ever holds either off.
    let mut run = standing(across_layers());
    let mut seq = 0;
    let mut play = |run: &mut Run, steps: u16, wheel: i16| {
        seq += 1;
        let body = wheeled_frame(seq, steps, wheel);
        run.input_frame(&field_game_core::json::parse(&body).expect("canonical"), None)
            .expect("accepted");
    };
    let depth = |run: &Run| run.state().now.forms[0].layer;

    // Three deliberate turns carry the Form to the deepest authored layer, each
    // waiting out the cooldown the one before it started.
    for expected in 1..=3u8 {
        play(&mut run, 1, 480);
        assert_eq!(depth(&run), expected, "the wheel carried the Form one layer deeper");
        assert_eq!(run.state().now.depth_cooldown, 14);
        play(&mut run, 14, 0);
        assert_eq!(run.state().now.depth_cooldown, 0, "and the cooldown ran out");
    }

    // At the deepest layer a further descent is clamped to the ladder — and it
    // still spends its cooldown, because the resolution is one rule and the
    // clamp is the Field's, not the resolution's.
    play(&mut run, 1, 480);
    assert_eq!(depth(&run), 3, "the deepest authored layer is the floor");
    assert_eq!(run.state().now.depth_cooldown, 14);

    // Inside that cooldown the way up is held off exactly as the way down was.
    play(&mut run, 1, -3000);
    assert_eq!(depth(&run), 3);
    assert_eq!(run.state().trace.steps.back().expect("a step").ctl.depth_move, 0);
    assert_eq!(run.state().now.wheel_accum, -480, "the accumulator holds the crossing");

    // The moment it ends, the retreat resolves: the accumulator was standing at
    // the trigger the whole time, and nothing else had to be done to be allowed
    // up. The upward change starts the same 15-step cooldown a downward one
    // does — the symmetry is the whole of the difference between them.
    play(&mut run, 13, 0);
    assert_eq!(run.state().now.depth_cooldown, 0);
    play(&mut run, 1, 0);
    assert_eq!(depth(&run), 2, "upward retreat, on the first step the cooldown allows");
    assert_eq!(run.state().trace.steps.back().expect("a step").ctl.depth_move, -1);
    assert_eq!(run.state().now.depth_cooldown, 14);
}

#[test]
fn depth_scales_the_reward_and_the_drain_a_player_chooses_between() {
    // The authored ladder is monotone in depth, which is what makes depth an
    // active difficulty choice rather than a place to stand: deeper layers
    // author more Drain and more gain — stronger pressure and stronger reward
    // — and more noise with them.
    let field = across_layers();
    for pair in field.layers.windows(2) {
        assert!(pair[1].drain > pair[0].drain, "a deeper layer drains more");
        assert!(pair[1].gain > pair[0].gain, "and rewards more");
        assert!(pair[1].noise > pair[0].noise, "and distorts more");
    }

    // Both halves of that choice, measured on one step. Two Nodes stand under
    // two currents of equal strength, one on the shallowest layer and one on
    // the deepest, so the only difference between what they receive and what
    // they lose is the depth they stand at.
    let ports = vec![
        port(1, 0, NodeKind::Reserve, 64, Vec2::units(1000, 1000)),
        port(2, 3, NodeKind::Reserve, 64, Vec2::units(1000, 1000)),
    ];
    let mut shallow_flow = current(1, 0, 30);
    shallow_flow.path = vec![Vec2::units(1000, 1000), Vec2::units(1001, 1000)];
    shallow_flow.strength = 16 * ONE_UNIT;
    let mut deep_flow = current(2, 3, 30);
    deep_flow.path = shallow_flow.path.clone();
    deep_flow.strength = shallow_flow.strength;
    let mut field = assembled(
        vec![layer(0, 0), layer(1, 1), layer(2, 2), layer(3, 4)],
        ports,
        Vec::new(),
        Vec::new(),
        vec![shallow_flow, deep_flow],
    );
    field::validate(&field).expect("the fixture is a valid Field");

    let before: Vec<i64> = field.ports.iter().map(|port| port.q).collect();
    let outcome = step_of(&mut field, ControlState::default());
    let gained = |place: usize, field: &FieldState| field.ports[place].q - before[place];

    // Delivery is scaled by the layer's gain, which is the locked depth-scaling
    // hook for rewards: a quarter of the strength at the top of the ladder and
    // the whole of it at the bottom.
    assert_eq!(outcome.ledger.current, 16 * ONE_UNIT / 4 + 16 * ONE_UNIT);
    // Drain is the other half, and it is paid at the same depth: the shallowest
    // layer authors none and the deepest four units a step.
    assert_eq!(outcome.ledger.drain, 4 * ONE_UNIT);
    assert_eq!(gained(0, &field), 4 * ONE_UNIT, "a quarter of the strength, and no Drain");
    assert_eq!(gained(1, &field), 16 * ONE_UNIT - 4 * ONE_UNIT, "all of it, less the Drain");
    assert!(gained(1, &field) > gained(0, &field), "depth is worth taking");
    assert_eq!(outcome.ledger.residual(), 0);
}

#[test]
fn every_depth_a_form_reaches_is_a_layer_that_stands() {
    // On a two-layer ladder the Form can only be on 0 or 1, and the contiguity
    // rule is what makes that true of every ladder: a depth change moves one
    // layer, and every layer between the ends stands.
    let ports = vec![
        port(1, 0, NodeKind::Form, 64, Vec2::units(1000, 1000)),
        port(2, 1, NodeKind::Reserve, 64, Vec2::units(1200, 1000)),
    ];
    let mut field = assembled(
        vec![layer(0, 0), layer(1, 3)],
        ports,
        Vec::new(),
        vec![form(1, 1, 0, Vec2::units(1000, 1000), 64)],
        Vec::new(),
    );
    field::validate(&field).expect("a short contiguous ladder is a valid Field");

    for (depth_move, expected) in [(1i8, 1u8), (1, 1), (-1, 0), (-1, 0), (1, 1)] {
        step_of(&mut field, ControlState { depth_move, ..ControlState::default() });
        assert_eq!(field.forms[0].layer, expected, "the Form stands on layer {expected}");
        // A depth the layer list does not declare would fail here, because the
        // Node mirrors the Form and validation reads the Node's layer.
        field::validate(&field).expect("every depth reached is a layer that stands");
    }

    // And the Drain of the depth it reached is the Drain that applies: the
    // deeper layer's three units, not the shallower layer's none.
    let outcome = step_of(&mut field, ControlState::default());
    assert_eq!(outcome.ledger.drain, 2 * 3 * ONE_UNIT, "both Nodes on layer 1 paid it");
    assert_eq!(outcome.ledger.residual(), 0);
}

#[test]
fn a_moving_form_advances_by_its_velocity_and_recomputes_its_adjacency() {
    let mut field = across_layers();
    // The Port at 2100 units stands 52 units from the Form, so they open
    // adjacent; the Form then moves away from it, one step at a time.
    assert!(field::nodes_adjacent(&field, 1, 2));
    assert_eq!(field::adjacency_of(&field, 1), vec![2]);
    assert_eq!(field::nodes_distance(&field, 1, 2), Some(52 * ONE_UNIT));

    // A coasting Form. ARCHITECTURE.md's Handoff locks what an uncontrolled
    // Form outside a linked group does: it consumes a **neutral control** —
    // steer (0, 0), no Pulse, no depth — through the same spring-damper, so it
    // slows under the locked damping and comes to rest rather than holding the
    // velocity it stands with. What phase 2 does is still what is under test:
    // whatever velocity the control phase leaves, the position advances by it
    // and the Node mirrors the Form.
    field.forms[0].controlled = false;
    field.forms[0].vel = Vec2::units(-100, 0);
    let mut carried = Vec2::units(-100, 0).x;
    let mut at = 2048 * ONE_UNIT;
    let mut faster_than = i64::MIN;
    for _ in 1..=48 {
        // The damper a neutral control leaves, then the integration — the two
        // phases in the order the step runs them.
        carried -= fixed_mul(STEER_DAMPING, carried);
        if carried.abs() < STEER_REST {
            carried = 0;
        }
        at += carried;
        step_of(&mut field, ControlState::default());
        assert_eq!(field.forms[0].vel.x, carried, "the neutral control damps the velocity");
        assert_eq!(field.forms[0].pos.x, at, "the position advances by that velocity");
        assert!(carried >= faster_than, "no step of the coast is faster than the one before it");
        faster_than = carried;
        let node = field.ports.iter().find(|port| port.node == 1).expect("the Node");
        assert_eq!(node.pos, field.forms[0].pos, "the Node mirrors the Form each step");
    }
    assert_eq!(field.forms[0].vel.x, 0, "the coast reaches the rest floor and stops");
    assert_eq!(
        field::nodes_distance(&field, 1, 2),
        Some(2100 * ONE_UNIT - field.forms[0].pos.x),
        "the distance is read off where the coast left the Form",
    );
    assert!(2100 * ONE_UNIT - field.forms[0].pos.x > ADJACENT_WITHIN, "past the adjacency");
    assert!(!field::nodes_adjacent(&field, 1, 2), "the coasting Form recomputed it");
    assert!(field::adjacency_of(&field, 1).is_empty());

    // The plane's locked width holds the position inside it. The damper takes
    // a quarter of the velocity before the integration reads it, so the widest
    // velocity a Field may carry crosses the plane over two steps rather than
    // one — which is what the two steps here are.
    for _ in 0..2 {
        field.forms[0].vel = Vec2::units(-4096, -4096);
        step_of(&mut field, ControlState::default());
    }
    assert_eq!(field.forms[0].pos, Vec2::new(0, 0));
    for _ in 0..2 {
        field.forms[0].vel = Vec2::units(4096, 4096);
        step_of(&mut field, ControlState::default());
    }
    assert_eq!(field.forms[0].pos, Vec2::new(PLANE_SPAN - 1, PLANE_SPAN - 1));
    assert!(field::within_caps(&field));
}

// ---------------------------------------------------------------------------
// Steering: the control state, spring-damped, into velocity.
//
// The shape is locked — one normalized Q1.15 vector, componentwise linear
// algebra, exact integer arithmetic — and the numbers are the reference feel
// the steering block in `core/src/field.rs` states. These read the numbers
// against that block, so a change to the feel has to move the statement of it
// too.
// ---------------------------------------------------------------------------

/// The widest control a frame may carry on one axis. The value −32768 is never
/// sent, so a whole deflection is one part in 32768 short of a whole.
const FULL_CONTROL: i16 = 32_767;

/// A control on the diagonal, at the widest magnitude the locked invariant
/// admits on both axes at once.
const DIAGONAL_CONTROL: i16 = 23_169;

/// The speed the three constants state between them: stiffness times reach,
/// over damping — 20 units per step, 600 per second.
const FULL_SPEED: Fx = 20 * ONE_UNIT;

fn steering(steer_x: i16, steer_y: i16) -> ControlState {
    ControlState { steer_x, steer_y, ..ControlState::default() }
}

/// One step under a control and a configured pointer speed.
fn step_at(field: &mut FieldState, control: ControlState, pointer_speed: Frac) -> StepOutcome {
    advance(field, control, pointer_speed, &mut Unstaged::default().staging())
}

/// The velocity a control settles a Form at, at a given pointer speed.
fn settled(control: ControlState, pointer_speed: Frac) -> Vec2 {
    let mut field = across_layers();
    let mut held = Vec2::default();
    for _ in 0..400 {
        step_at(&mut field, control, pointer_speed);
        held = field.forms[0].vel;
    }
    held
}

#[test]
fn a_control_held_at_full_deflection_approaches_one_locked_speed_and_never_passes_it() {
    assert_eq!(
        fixed_div(fixed_mul(STEER_STIFFNESS, STEER_REACH), STEER_DAMPING),
        FULL_SPEED,
        "the three constants state one terminal speed between them",
    );

    let mut field = across_layers();
    let mut readings = Vec::new();
    let mut previous = 0;
    for _ in 0..60 {
        step_at(&mut field, steering(FULL_CONTROL, 0), FRAC_ONE);
        let speed = field.forms[0].vel.x;
        assert!(speed >= previous, "the approach is monotone: a spring against a damper");
        assert!(speed <= FULL_SPEED, "and never passes the speed the control asks for");
        previous = speed;
        readings.push(speed);
    }

    // The settling the steering block states, read back off the run. Every
    // reading is short of the ideal by the control's own last part in 32768 and
    // by what the two floors take — together below the speed the model calls
    // rest, which is the yardstick for a difference that is not a difference.
    assert!(
        (FULL_SPEED / 4 - readings[0]).abs() <= STEER_REST,
        "a quarter of the way inside one step",
    );
    assert!(readings[3] * 100 / FULL_SPEED >= 68, "68% inside four steps, 133 ms");
    assert!(readings[10] * 100 / FULL_SPEED >= 95, "95% inside eleven, 367 ms");
    assert_eq!(readings[59], readings[58], "and it settles rather than creeping");
    // Short of the identity by the control's own last part in 32768 and by what
    // the two floors take, which together stay inside the rest threshold.
    assert!(FULL_SPEED - readings[59] <= STEER_REST, "settles on the speed it states");

    // Nothing but a controlled Form is steered.
    let mut drifting = across_layers();
    drifting.forms[0].controlled = false;
    for _ in 0..60 {
        step_at(&mut drifting, steering(FULL_CONTROL, 0), FRAC_ONE);
    }
    assert_eq!(drifting.forms[0].vel, Vec2::default(), "a drifting Form reads no control");
}

#[test]
fn a_released_control_drifts_a_form_to_rest_rather_than_stopping_it() {
    let mut field = across_layers();
    for _ in 0..60 {
        step_at(&mut field, steering(DIAGONAL_CONTROL, DIAGONAL_CONTROL), FRAC_ONE);
    }
    let moving = field.forms[0].vel;
    assert!(moving.x > 0 && moving.y > 0, "the Form is under way");

    let mut previous = moving.x;
    let mut at_rest = None;
    for step in 1..=60u32 {
        step_at(&mut field, ControlState::default(), FRAC_ONE);
        let speed = field.forms[0].vel.x;
        assert!(speed < previous || speed == 0, "every step decays, and none of them snaps");
        if step == 1 {
            assert!(speed * 2 > moving.x, "a released control keeps most of its speed for a frame");
        }
        previous = speed;
        if field.forms[0].vel == Vec2::default() && at_rest.is_none() {
            at_rest = Some(step);
        }
    }
    let reached = at_rest.expect("a released control comes to rest");
    assert!(reached <= 40, "inside a second and a third of play, not eventually: {reached} steps");
    assert_eq!(field.forms[0].vel, Vec2::default(), "and exactly to rest, not almost");
}

#[test]
fn the_control_carries_its_heading_componentwise() {
    // Equal deflection on both axes is one heading at 45 degrees, and the sign
    // of each component is its own axis's.
    let turned = settled(steering(-DIAGONAL_CONTROL, DIAGONAL_CONTROL), FRAC_ONE);
    assert!(turned.x < 0 && turned.y > 0);
    // `fixed_mul` floors toward negative infinity, so the damper takes a raw
    // unit more from the negative axis than from the positive one until the two
    // balance. They part by a few raw units, which is below the speed the model
    // calls rest.
    assert!((turned.x + turned.y).abs() <= STEER_REST, "one heading, componentwise: {turned:?}");

    // And the response is linear in the control: half the deflection is half
    // the speed, which is what makes cursor distance a speed rather than a
    // switch.
    let half = settled(steering(FULL_CONTROL / 2, 0), FRAC_ONE);
    let whole = settled(steering(FULL_CONTROL, 0), FRAC_ONE);
    assert!((whole.x - 2 * half.x).abs() <= STEER_REST, "linear: {half:?} against {whole:?}");
}

#[test]
fn the_configured_pointer_speed_scales_the_speed_a_control_asks_for() {
    // The locked range is a quarter of the reference speed to four times it.
    let slowest = settled(steering(FULL_CONTROL, 0), 16_384);
    let default = settled(steering(FULL_CONTROL, 0), FRAC_ONE);
    let fastest = settled(steering(FULL_CONTROL, 0), 262_144);

    assert!((default.x - FULL_SPEED).abs() <= STEER_REST);
    assert!((slowest.x * 4 - FULL_SPEED).abs() <= 4 * STEER_REST, "a quarter: {slowest:?}");
    assert!((fastest.x - 4 * FULL_SPEED).abs() <= 4 * STEER_REST, "four times: {fastest:?}");
}

#[test]
fn a_steering_schedule_replays_to_the_same_field_and_the_same_bytes() {
    // A schedule that turns rather than one that settles: the spring is in
    // motion for most of it, which is where a replay would part company with a
    // live run if the phase read anything the trace does not carry.
    let turning = |seq: u32| -> (i16, i16) {
        match seq % 4 {
            0 => (FULL_CONTROL, 0),
            1 => (-DIAGONAL_CONTROL, DIAGONAL_CONTROL),
            2 => (0, 0),
            _ => (DIAGONAL_CONTROL, -DIAGONAL_CONTROL),
        }
    };
    let play = || -> Run {
        let mut run = standing(across_layers());
        for seq in 1..=200u32 {
            let (steer_x, steer_y) = turning(seq);
            let body = steered_frame(seq, 1, steer_x, steer_y);
            run.input_frame(&field_game_core::json::parse(&body).expect("canonical"), None)
                .expect("accepted");
        }
        run
    };

    let run = play();
    let now = &run.state().now;
    assert_ne!(now.forms[0].pos, Vec2::units(2048, 2048), "the schedule moved the Form");
    assert_ne!(now.forms[0].vel, Vec2::default(), "and left it under way");

    // The retained trajectory regenerates the live Field exactly, spring and
    // all: the control schedule the trace records is the whole of the phase's
    // input.
    let trace = &run.state().trace;
    let mut carried = trace.keyframe.clone();
    for recorded in &trace.steps {
        advance(&mut carried, recorded.ctl, FRAC_ONE, &mut Unstaged::default().staging());
    }
    assert_eq!(carried.written(), now.written(), "the replay lands on the live Field");

    // And the same run key under the same frames serializes to the same bytes.
    assert_eq!(play().state().payload(), run.state().payload(), "byte-equivalent");
}

#[test]
fn every_current_advances_one_phase_per_step_modulo_its_period() {
    let mut field = across_layers();
    assert_eq!(field.currents[0].period, 30);
    for step in 1..=31u16 {
        step_of(&mut field, ControlState::default());
        assert_eq!(field.currents[0].phase, step % 30);
    }
    assert_eq!(field.currents[0].phase, 1, "the phase wrapped exactly once");
}

#[test]
fn the_drawn_boundary_list_keeps_the_most_recent_entries_and_no_more() {
    let mut boundaries = BoundaryState::default();
    for step in 1..=40u32 {
        boundaries.record_drawn(vec![step, step + 1], step);
    }
    assert_eq!(boundaries.drawn.len(), DRAWN_RETAINED);
    assert_eq!(boundaries.drawn[0].step, 40, "most recent first");
    assert_eq!(boundaries.drawn[DRAWN_RETAINED - 1].step, 40 - DRAWN_RETAINED as u32 + 1);
}

// ---------------------------------------------------------------------------
// Deterministic updates, across the whole runtime.
// ---------------------------------------------------------------------------

/// Plays a recorded control schedule over a Field on several layers and returns
/// the canonical bytes of the run state at the end of it.
fn played(field: FieldState, script: &[(u16, i8)]) -> String {
    let inside: Vec<u32> = field.ports.iter().map(|port| port.node).collect();
    played_under(field, inside, script)
}

/// The same, under a named passive View, which takes part in the payload bytes
/// but never in the Field's leakage arithmetic.
fn played_under(field: FieldState, inside: Vec<u32>, script: &[(u16, i8)]) -> String {
    let mut run = standing_under(field, inside);
    for (index, (steps, depth_key)) in script.iter().enumerate() {
        let body = frame(index as u32 + 1, *steps, *depth_key);
        let parsed = field_game_core::json::parse(&body).expect("the frame is canonical");
        run.input_frame(&parsed, None).expect("the frame is accepted");
    }
    run.state().payload()
}

#[test]
fn a_field_on_several_layers_serializes_the_same_bytes_under_the_same_schedule() {
    // A schedule that moves the Form between layers and lets Drain take Charge
    // at each depth it reaches.
    let script = [(1u16, 1i8), (3, 0), (1, 1), (6, 0), (1, -1), (4, 0), (1, 1), (2, 0)];
    let once = played(across_layers(), &script);
    let again = played(across_layers(), &script);

    assert_eq!(once, again, "byte-equivalent, not merely equivalent");
    assert_eq!(canonicalize(&once).expect("the payload is canonical"), once);
    assert!(once.contains("\"step\":19"), "nineteen steps ran: {once}");
    // The Field is in the payload: its parts, its Charge, and its depth.
    assert!(once.contains("\"form\":\"thread\""), "{once}");
    assert!(once.contains("\"kind\":\"reserve\""), "{once}");
    assert!(once.contains("\"drain\":262144"), "{once}");

    // A different schedule over the same Field diverges, and so does a
    // different Field under the same schedule.
    let other = played(across_layers(), &[(19u16, 0i8)]);
    assert_ne!(once, other, "the depth the Form reached is part of the state");

    // The richer state repeats byte for byte too: a Field where all four rules
    // move Charge every step, including the Field's physical leakage rule.
    let script = [(1u16, 1i8), (4, 0), (1, 1), (5, 0), (1, -1), (3, 0)];
    let rich = played_under(whole_field(), vec![1, 3], &script);
    assert_eq!(rich, played_under(whole_field(), vec![1, 3], &script));
    assert_eq!(canonicalize(&rich).expect("the payload is canonical"), rich);
    assert!(rich.contains("\"leak_per_exposed_contact_per_step\":8192"), "{rich}");
    assert!(rich.contains("\"flow\":"), "the Routes carried their flow into the trace");

    // The active View is serialized observation metadata, so the whole payload
    // differs even though the physical Field and its leakage do not.
    assert_ne!(rich, played_under(whole_field(), vec![1, 3, 4], &script));
    // And so is the leakage parameter itself.
    let mut quiet = whole_field();
    quiet.physical_compartment.leak_per_exposed_contact_per_step = 0;
    assert_ne!(rich, played_under(quiet, vec![1, 3], &script));

    let mut deeper = across_layers();
    deeper.forms[0].layer = 1;
    deeper.ports[0].layer = 1;
    let deeper = assembled(
        deeper.layers.clone(),
        deeper.ports.clone(),
        deeper.routes.clone(),
        deeper.forms.clone(),
        deeper.currents.clone(),
    );
    assert_ne!(once, played(deeper, &script));
}

/// The done-when Field: a controlled Form on the shallowest layer with a current
/// running through it, two Routes carrying its Charge down to Nodes on deeper
/// layers, and Drain rising with depth.
///
/// Node 1 is the Form, on layer 0 inside the bright current. Node 2 is a Port on
/// layer 0, closed, standing beside the Form. Nodes 3 and 4 are a Reserve on
/// layer 1 and a Module on layer 2, and the two Routes run 1 → 3 → 4 in
/// ascending order, so one pass carries Charge both hops.
fn whole_field() -> FieldState {
    let ports = vec![
        port(1, 0, NodeKind::Form, 0, Vec2::units(1000, 1000)),
        port(2, 0, NodeKind::Port, 0, Vec2::units(1100, 1000)),
        port(3, 1, NodeKind::Reserve, 0, Vec2::units(1200, 1000)),
        port(4, 2, NodeKind::Module, 0, Vec2::units(1400, 1000)),
    ];
    let mut shallow = layer(0, 0);
    shallow.gain = FRAC_ONE;
    let mut bright = current(1, 0, 30);
    bright.path = vec![Vec2::units(1000, 1000), Vec2::units(1000, 1064)];
    bright.width = 64 * ONE_UNIT;
    bright.strength = 12 * ONE_UNIT;
    let mut field = assembled(
        vec![shallow, layer(1, 1), layer(2, 2), layer(3, 4)],
        ports,
        vec![route(1, 1, 3, 8), route(2, 3, 4, 8)],
        vec![form(1, 1, 0, Vec2::units(1000, 1000), 0)],
        vec![bright],
    );
    field.physical_compartment.members = vec![1];
    field.physical_compartment.leak_per_exposed_contact_per_step = LEAK_FRAC_CAP;
    // Zero noise, for the same reason the chain fixture is quiet: what these
    // tests pin is the other rules' exact arithmetic.
    let field = quiet(field);
    field::validate(&field).expect("the fixture is a valid Field");
    field
}

#[test]
fn arbitrary_view_changes_leave_field_trace_leakage_and_impulse_unchanged() {
    let run = standing_under(whole_field(), vec![1]);
    let mut first = run.state().clone();
    let mut second = first.clone();
    second.view = ViewDeclaration {
        inside: vec![2, 3, 4],
        resolution: 8,
        window: 30,
        surround: field_game_core::state::Surround::Double,
    };
    let opening_impulse = first.progress.impulse;
    let controls = [
        ControlState::default(),
        ControlState { steer_x: 12_000, ..ControlState::default() },
        ControlState { depth_move: 1, ..ControlState::default() },
    ];

    for control in controls {
        let left = advance(
            &mut first.now,
            control,
            FRAC_ONE,
            &mut Unstaged::default().staging(),
        );
        let right = advance(
            &mut second.now,
            control,
            FRAC_ONE,
            &mut Unstaged::default().staging(),
        );
        let step = first.now.step;
        first.trace.steps.push_back(TraceStep {
            step,
            rng: Default::default(),
            ctl: control,
            records: left.records.clone(),
        });
        second.trace.steps.push_back(TraceStep {
            step,
            rng: Default::default(),
            ctl: control,
            records: right.records.clone(),
        });

        assert_eq!(left.ledger.leakage, right.ledger.leakage);
        assert_eq!(left.records.e, right.records.e, "View never enters the leakage record");
        assert_eq!(left.ledger.residual(), 0);
        assert_eq!(right.ledger.residual(), 0);
        assert_eq!(first.now.written(), second.now.written(), "the Fields stay byte-identical");
        let left_trace: Vec<String> = first.trace.steps.iter().map(TraceStep::written).collect();
        let right_trace: Vec<String> = second.trace.steps.iter().map(TraceStep::written).collect();
        assert_eq!(left_trace, right_trace, "the retained causal trace is identical");
        assert_eq!(first.progress.impulse, opening_impulse);
        assert_eq!(second.progress.impulse, opening_impulse);
    }
    assert_eq!(first.trace.keyframe.written(), second.trace.keyframe.written());
    assert_ne!(first.view, second.view, "only observation metadata differs");
}

#[test]
fn a_headless_form_accumulates_moves_loses_and_routes_charge_across_several_layers() {
    // The physical compartment contains the Form alone, so it is an exposed
    // material shell member and leaks — the fourth rule, beside the other three.
    let mut run = standing_under(whole_field(), vec![1]);
    let play = |run: &mut Run, seq: u32, steps: u16, depth_key: i8| {
        let body = frame(seq, steps, depth_key);
        run.input_frame(&field_game_core::json::parse(&body).expect("canonical"), None)
            .expect("accepted");
    };

    // Six steps on the shallowest layer: the current delivers Charge to the
    // Form, and the Routes carry it on to the Nodes below.
    play(&mut run, 1, 6, 0);
    let after_gathering = run.state().now.clone();
    let held = |field: &FieldState, node: u32| charge_of(field, node);
    assert!(held(&after_gathering, 1) > 0, "the Form accumulated Charge from the current");
    assert!(held(&after_gathering, 4) > 0, "and routed some of it down two layers");
    assert_eq!(held(&after_gathering, 2), 0, "the closed Port took none of it");
    // The Reserve between them ends every step empty because both Routes stand
    // in one ascending pass: what arrives along Route 1 leaves along Route 2 in
    // the same step, which is the multi-hop behavior the pass order locks.
    let records = run.last_records().expect("a step was recorded");
    assert_eq!(
        records.f,
        vec![(1, 8 * ONE_UNIT), (2, 8 * ONE_UNIT)],
        "both Routes carried their capacity"
    );
    assert_eq!(held(&after_gathering, 3), 0, "so the Node between them holds nothing");
    assert_eq!(
        after_gathering.forms[0].charge,
        held(&after_gathering, 1),
        "a Form's Charge is its own Node's stored Charge"
    );

    // Three depth changes, one per frame, then twelve steps at the bottom: the
    // Form moves across every layer and pays the Drain of each depth it reaches.
    for (offset, seq) in (2..=4u32).enumerate() {
        play(&mut run, seq, 1, 1);
        assert_eq!(run.state().now.forms[0].layer, offset as u8 + 1, "the Form descended");
    }
    play(&mut run, 5, 12, 0);

    let now = &run.state().now;
    assert_eq!(now.forms[0].layer, 3, "and stands on the deepest authored layer");
    assert_eq!(now.step, 21);
    assert!(
        held(now, 1) < held(&after_gathering, 1),
        "away from the current it accumulates nothing and loses Charge instead"
    );
    assert_eq!(
        now.ports.iter().find(|port| port.node == 1).expect("the Form's Node").layer,
        3,
        "the Form's Node stands where the Form does"
    );

    // Every rule contributed, and every step of the run balanced exactly.
    let mut totals = (0i64, 0i64, 0i64, 0i64);
    let mut replayed = whole_field();
    for recorded in &run.state().trace.steps {
        let outcome = advance(&mut replayed, recorded.ctl, FRAC_ONE, &mut Unstaged::default().staging());
        assert_eq!(outcome.ledger.residual(), 0, "step {} balances", recorded.step);
        assert!(outcome.ledger.balanced(), "per Node too");
        totals.0 += outcome.ledger.moved;
        totals.1 += outcome.ledger.drain;
        totals.2 += outcome.ledger.leakage;
        totals.3 += outcome.ledger.current;
    }
    assert!(totals.0 > 0, "Charge routed");
    assert!(totals.1 > 0, "Charge lost to depth");
    assert!(totals.2 > 0, "Charge leaked across the boundary");
    assert!(totals.3 > 0, "Charge accumulated from the current");
    assert_eq!(
        replayed.written(),
        now.written(),
        "and the whole schedule replays to the same Field, byte for byte"
    );
}

#[test]
fn the_keyframe_carried_forward_replays_to_the_live_field() {
    let mut run = standing(across_layers());
    // Bracket keys resolve depth without the wheel, so the accumulator and the
    // cooldown stay at nothing and the whole state is Field content.
    for seq in 1..=200u32 {
        let depth_key = if seq % 50 == 0 { 1 } else { 0 };
        let body = frame(seq, 1, depth_key);
        run.input_frame(&field_game_core::json::parse(&body).expect("canonical"), None)
            .expect("accepted");
    }

    let trace = &run.state().trace;
    assert_eq!(trace.start_step, 60);
    assert_eq!(trace.steps.len(), 140);

    let mut carried = trace.keyframe.clone();
    for recorded in &trace.steps {
        let outcome = advance(&mut carried, recorded.ctl, FRAC_ONE, &mut Unstaged::default().staging());
        assert_eq!(carried.step, recorded.step);
        assert_eq!(outcome.ledger.residual(), 0);
    }
    assert_eq!(
        carried.written(),
        run.state().now.written(),
        "the retained trajectory regenerates the live Field exactly"
    );
}

// ---------------------------------------------------------------------------
// The trace's dense encoding.
// ---------------------------------------------------------------------------

/// The densest step the encoding allows: every Node and every Route recording
/// its widest value, with the upkeep split summing to its own total exactly.
fn densest_step(step: u32) -> TraceStep {
    let share = (STORED_BOUND - 1) / 5;
    let mix = [share, share, share, share, STORED_BOUND - 1 - 4 * share];
    assert_eq!(mix.iter().sum::<i64>(), STORED_BOUND - 1, "the split sums to its total");

    TraceStep {
        step,
        rng: trajectory_stream(KEY, 0),
        ctl: ControlState {
            steer_x: -32_767,
            steer_y: 32_767,
            pulse_held: true,
            pulse_release: true,
            depth_move: -1,
        },
        records: StepRecords {
            q: (1..=NODES_PER_RUN as u32).map(|node| (node, NODE_CHARGE_CAP)).collect(),
            f: (1..=ROUTES_PER_RUN as u32).map(|route| (route, ROUTE_CAPACITY_CAP)).collect(),
            upkeep: (1..=NODES_PER_RUN as u32)
                .map(|node| UpkeepRecord { node, v: STORED_BOUND - 1, mix })
                .collect(),
            e: (1..=NODES_PER_RUN as u32).map(|node| (node, -NODE_CHARGE_CAP)).collect(),
            z: (1..=NODES_PER_RUN as u32).collect(),
        },
    }
}

#[test]
fn the_densest_step_and_a_full_trajectory_stay_inside_the_locked_caps() {
    let dense = densest_step(1);
    let written = dense.written();
    assert_eq!(canonicalize(&written).expect("the step is canonical"), written);
    // The encoder is deterministic, so the document's measured figure is an
    // equality rather than a bound: a change to any record's shape moves it, and
    // the document and this pin are updated together or not at all.
    assert_eq!(written.len(), DENSE_STEP_BYTES, "the locked dense-step figure");

    // The whole save payload, at the widest trajectory the trace retains and a
    // Field at every cap, inside the locked 8 MiB.
    let field = at_every_cap();
    let mut trace = Trace::opening(field.clone());
    trace.start_step = 0;
    for step in 1..=RETAINED_STEPS as u32 {
        trace.steps.push_back(densest_step(step));
    }
    let state = RunState {
        run_id: KEY.to_string(),
        rng: trajectory_stream(KEY, 0),
        content_hash: support::content_hash(),
        branch_nonce: 0,
        progress: Progress::opening(),
        now: field,
        trace,
        view: ViewDeclaration::opening(),
        slate: None,
        input_config: InputConfig::default_config(),
        pressures: Vec::new(),
        schedule: Default::default(),
        // Checkpoint metadata is measured beside the slate and the pressures,
        // in the headroom the document leaves past this figure.
        anchors: Vec::new(),
    };
    let payload = state.payload();
    assert_eq!(canonicalize(&payload).expect("the payload is canonical"), payload);
    assert_eq!(payload.len(), WORST_PAYLOAD_BYTES, "the locked worst-case payload figure");
    assert!(
        payload.len() < SAVE_PAYLOAD_CAP,
        "the densest retained trajectory is {} bytes, and the cap is {SAVE_PAYLOAD_CAP}",
        payload.len()
    );
    // The headroom the document states, to the tenth of a percent.
    let spare = (SAVE_PAYLOAD_CAP - payload.len()) * 1000 / SAVE_PAYLOAD_CAP;
    assert_eq!(spare, 86, "8.6% of the cap stands spare");
}

#[test]
fn a_step_records_its_lists_ascending_and_omits_every_zero() {
    let mut field = across_layers();
    field.ports[1].q = 0;
    let outcome = step_of(&mut field, ControlState::default());

    let nodes: Vec<u32> = outcome.records.q.iter().map(|(node, _)| *node).collect();
    assert!(nodes.windows(2).all(|pair| pair[0] < pair[1]), "ascending: {nodes:?}");
    assert!(!nodes.contains(&2), "a Node holding nothing writes no stored-Charge entry");
    assert!(outcome.records.q.iter().all(|(_, value)| *value != 0));
    assert!(outcome.records.e.iter().all(|(_, value)| *value != 0));
    let failed: Vec<u32> = outcome.records.z.clone();
    assert!(failed.windows(2).all(|pair| pair[0] < pair[1]), "ascending: {failed:?}");
    assert_eq!(failed, vec![2], "only the Node that ended the step at nothing");
}

#[test]
fn a_field_and_its_view_are_established_before_the_first_step_and_validated_on_the_way_in() {
    let whole = |field: &FieldState| ViewDeclaration {
        inside: field.ports.iter().map(|port| port.node).collect(),
        ..ViewDeclaration::opening()
    };

    let mut run = Run::start(KEY, "thread", &support::content_hash()).expect("a run opens");
    let mut refused = across_layers();
    refused.ports[0].q = NODE_CHARGE_CAP + 1;
    let view = whole(&refused);
    assert_eq!(
        run.establish_field(refused, view).expect_err("an invalid Field is refused").code(),
        Code::Capacity
    );
    assert!(run.state().now.ports.is_empty(), "a refusal establishes nothing");
    assert!(run.state().view.inside.is_empty(), "and leaves the opening View standing");

    // The View is held to its own locked shape: an empty or descending inside, a
    // grain off the ladder, a window outside its range, and a member naming no
    // Node are each refused.
    // The inside has no row of the capacity table, so it fails as a shape; the
    // grain and the window restate rows of it, so the table's envelope governs
    // them even though the type restates them as ranges.
    let refusals: Vec<(&str, ViewDeclaration, Code)> = vec![
        ("an empty inside", ViewDeclaration::opening(), Code::Validation),
        (
            "a descending inside",
            ViewDeclaration { inside: vec![3, 1], ..ViewDeclaration::opening() },
            Code::Validation,
        ),
        (
            "a grain off the ladder",
            ViewDeclaration { inside: vec![1], resolution: 3, ..ViewDeclaration::opening() },
            Code::Capacity,
        ),
        (
            "a window past its range",
            ViewDeclaration { inside: vec![1], window: 61, ..ViewDeclaration::opening() },
            Code::Capacity,
        ),
        (
            "a window below its range",
            ViewDeclaration { inside: vec![1], window: 0, ..ViewDeclaration::opening() },
            Code::Capacity,
        ),
    ];
    for (reason, view, expected) in refusals {
        assert_eq!(
            run.establish_field(across_layers(), view).expect_err(reason).code(),
            expected,
            "{reason}"
        );
    }
    let absent = ViewDeclaration { inside: vec![1, 99], ..ViewDeclaration::opening() };
    assert_eq!(
        run.establish_field(across_layers(), absent).expect_err("a member naming no Node").code(),
        Code::NotFound
    );

    let field = across_layers();
    let view = whole(&field);
    run.establish_field(field, view).expect("a valid Field and View are established");
    assert_eq!(run.state().now.ports.len(), 5);
    assert_eq!(run.state().view.inside, vec![1, 2, 3, 4, 5]);
    assert_eq!(run.state().trace.keyframe.written(), run.state().now.written());

    // A run that has already advanced holds a trajectory the opening state
    // cannot replace.
    let body = frame(1, 1, 0);
    run.input_frame(&field_game_core::json::parse(&body).expect("canonical"), None).expect("accepted");
    let field = across_layers();
    let view = whole(&field);
    assert_eq!(
        run.establish_field(field, view).expect_err("too late").code(),
        Code::Validation
    );
}

#[test]
fn the_ledger_carries_every_term_of_the_locked_identity() {
    let mut field = layers_only();
    let outcome = step_of(&mut field, ControlState::default());
    let ledger: &Ledger = &outcome.ledger;

    assert_eq!(ledger.nodes.len(), field.ports.len());
    for (entry, port) in ledger.nodes.iter().zip(field.ports.iter()) {
        assert_eq!(entry.node, port.node, "the ledger is in ascending Node order");
        assert_eq!(entry.closing, port.q);
    }
    assert_eq!(ledger.opening, 128 * ONE_UNIT, "8 + 0 + 64 + 16 + 40 units stood in the Field");
    assert_eq!(ledger.closing, 121 * ONE_UNIT);
    assert_eq!(ledger.drain, 7 * ONE_UNIT);
    assert_eq!(ledger.upkeep, 0, "no upkeep falls due while its attribution is unlocked");
    assert_eq!(ledger.leakage, 0, "the physical leakage coefficient is zero");
    assert_eq!(ledger.overload, 0, "and nothing holds more than its threshold");
    assert_eq!(ledger.current, 0, "this fixture carries no current");
    assert_eq!(ledger.moved, 0, "and no Route");
    assert_eq!(ledger.sinks(), 7 * ONE_UNIT);
    assert_eq!(ledger.sources(), 0);
    assert_eq!(ledger.residual(), 0);

    // Every sink and source at once, on a Field that carries all four rules.
    let mut whole = across_layers();
    whole.physical_compartment.members = vec![1, 3];
    whole.physical_compartment.leak_per_exposed_contact_per_step = LEAK_FRAC_CAP;
    whole.ports[1].q = 32 * ONE_UNIT;
    whole.ports[1].capacity = 8 * ONE_UNIT;
    whole.currents[0].path = vec![Vec2::units(2048, 2048), Vec2::units(2100, 2048)];
    field::validate(&whole).expect("the fixture is a valid Field");
    let outcome = advance(&mut whole, ControlState::default(), FRAC_ONE, &mut Unstaged::default().staging());
    let ledger = &outcome.ledger;
    assert!(ledger.moved > 0, "a Route carried Charge");
    assert!(ledger.drain > 0, "a layer's Drain took some");
    assert!(ledger.leakage > 0, "an exposed member leaked some");
    assert!(ledger.overload > 0, "an overloaded Node shed some");
    assert!(ledger.current > 0, "and a current delivered some");
    assert_eq!(ledger.residual(), 0, "the whole of it balances to nothing");
    assert!(ledger.balanced(), "per Node as well as in total");
}


// ---------------------------------------------------------------------------
// The Pulse, as far as the documents lock it.
// ---------------------------------------------------------------------------

/// One press-and-release of the Pulse, as the frames the shell sends carry it:
/// `pulse_held` on every frame of the hold and `pulse_release` on the one that
/// lets go. The shape is `InputFrame`'s and `ControlState`'s, both frozen.
fn one_pulse(hold: u32) -> Vec<(bool, bool)> {
    let mut script: Vec<(bool, bool)> = (0..hold).map(|_| (true, false)).collect();
    script.push((false, true));
    script.push((false, false));
    script
}

/// Plays a schedule of Pulse frames over a Field and hands back the run.
fn pulsed(field: FieldState, inside: Vec<u32>, script: &[(bool, bool)]) -> Run {
    let mut run = standing_under(field, inside);
    for (index, (held, release)) in script.iter().enumerate() {
        let body = pulsing_frame(index as u32 + 1, 1, *held, *release);
        run.input_frame(&field_game_core::json::parse(&body).expect("canonical"), None)
            .expect("accepted");
    }
    run
}

#[test]
fn a_frame_carrying_the_pulse_is_read_into_the_control_state_it_records() {
    let script = one_pulse(4);
    let run = pulsed(whole_field(), vec![1], &script);

    // `ControlState` carries both fields, so the trace records the Pulse the
    // step consumed exactly as it records the steering and the depth change.
    let recorded: Vec<(bool, bool)> = run
        .state()
        .trace
        .steps
        .iter()
        .map(|step| (step.ctl.pulse_held, step.ctl.pulse_release))
        .collect();
    assert_eq!(recorded, script, "every step recorded the control it consumed");

    // The emission consumed the charge and the step after it held nothing, so
    // the Form ends at rest with no charge and no focus.
    assert_eq!(run.state().now.forms[0].pulse_charge, 0, "the emission consumed the charge");
    assert!(!run.state().now.forms[0].focus, "and the hold that wrote focus is over");
}

/// The same Field, with the closed Port beside the Form holding Charge, so a
/// Pulse that reaches it gathers as well as opens it.
fn whole_field_with_a_source() -> FieldState {
    let mut field = whole_field();
    let place = field.ports.iter().position(|port| port.node == 2).expect("the closed Port");
    field.ports[place].q = 200 * ONE_UNIT;
    field::validate(&field).expect("a Port holding Charge is a valid one");
    field
}

#[test]
fn a_schedule_carrying_pulses_balances_every_step_and_replays_byte_exact() {
    // The Field where all five locked rules move Charge, with an exposed
    // physical member, the Pulse held across it and released inside reach of a
    // closed Port that holds Charge: conservation has to hold through the
    // gathering transfer exactly as through every other rule.
    let script = one_pulse(20);
    let run = pulsed(whole_field_with_a_source(), vec![1], &script);
    assert_eq!(run.state().now.step, script.len() as u32);

    // The emission gathered a quarter of what that Port held and opened it.
    let opened = run.state().now.ports.iter().find(|port| port.node == 2).expect("the Port");
    assert!(opened.open, "the Pulse opened the Port it reached");
    assert!(opened.q < 200 * ONE_UNIT, "and gathered a quarter of what it held");

    let mut totals = (0i64, 0i64, 0i64, 0i64);
    let mut gathered = 0i64;
    let mut replayed = whole_field_with_a_source();
    for recorded in &run.state().trace.steps {
        let outcome = advance(&mut replayed, recorded.ctl, FRAC_ONE, &mut Unstaged::default().staging());
        // Exact zero residual, in total and per Node, with no tolerance
        // anywhere — including on the step that carried the release.
        assert_eq!(outcome.ledger.residual(), 0, "step {} balances", recorded.step);
        assert!(outcome.ledger.balanced(), "per Node too");
        totals.0 += outcome.ledger.moved;
        totals.1 += outcome.ledger.drain;
        totals.2 += outcome.ledger.leakage;
        totals.3 += outcome.ledger.current;
        gathered += outcome.ledger.gathered;
        // The gathering transfer creates and destroys nothing, so it never
        // enters the residual and never touches a sink or a source.
        assert_eq!(outcome.ledger.sources(), outcome.ledger.current);
    }
    assert!(totals.0 > 0 && totals.1 > 0 && totals.2 > 0 && totals.3 > 0, "every rule ran");
    assert!(gathered > 0, "and the Pulse gathered");
    assert_eq!(gathered, fixed_mul(200 * ONE_UNIT, PULSE_GATHER_SHARE), "a quarter, once");
    assert_eq!(
        replayed.written(),
        run.state().now.written(),
        "the recorded schedule replays to the same Field, byte for byte",
    );

    // The same schedule twice is the same bytes, and the Pulse is part of what
    // the bytes stand on: a run that pulsed and a run that did not diverge,
    // because the control schedule the trace records drives the Field.
    let again = pulsed(whole_field_with_a_source(), vec![1], &script);
    assert_eq!(again.state().payload(), run.state().payload(), "byte-equivalent");
    let quiet: Vec<(bool, bool)> = script.iter().map(|_| (false, false)).collect();
    let untouched = pulsed(whole_field_with_a_source(), vec![1], &quiet);
    assert_ne!(
        untouched.state().payload(),
        run.state().payload(),
        "the Pulse fields are recorded, and now the Field moves under them",
    );
    assert_ne!(untouched.state().now.written(), run.state().now.written());
}

#[test]
fn the_keyframe_carries_a_pulse_carrying_schedule_forward_exactly() {
    // Long enough that the retained span moves and the keyframe is carried by
    // replaying steps that held the Pulse.
    let mut run = standing(across_layers());
    for seq in 1..=200u32 {
        let through = seq % 20;
        let body = pulsing_frame(seq, 1, through < 12, through == 12);
        run.input_frame(&field_game_core::json::parse(&body).expect("canonical"), None)
            .expect("accepted");
    }

    let trace = &run.state().trace;
    assert_eq!(trace.start_step, 60, "the keyframe moved with the retained span");
    assert!(
        trace.steps.iter().any(|step| step.ctl.pulse_release),
        "and the span it kept holds releases",
    );

    let mut carried = trace.keyframe.clone();
    for recorded in &trace.steps {
        let outcome = advance(&mut carried, recorded.ctl, FRAC_ONE, &mut Unstaged::default().staging());
        assert_eq!(outcome.ledger.residual(), 0);
    }
    assert_eq!(
        carried.written(),
        run.state().now.written(),
        "the retained trajectory regenerates the live Field exactly",
    );
}

// ---------------------------------------------------------------------------
// The Pulse, rule by rule.
// ---------------------------------------------------------------------------

/// A Field built for the Pulse: a controlled Form holding nothing, with three
/// sources around it on its own plane inside a full charge's reach of 192
/// units, one source past that reach, and one directly below it on the next
/// layer — which the locked distance rule's 512-unit layer term puts out of
/// reach whatever the charge.
fn pulse_field() -> FieldState {
    let ports = vec![
        port(1, 0, NodeKind::Form, 0, Vec2::units(2000, 2000)),
        // 100 units away, closed: a source and an activation at once.
        port(2, 0, NodeKind::Port, 64, Vec2::units(2100, 2000)),
        // 150 units away.
        port(3, 0, NodeKind::Reserve, 40, Vec2::units(2000, 2150)),
        // 190 units away: inside a full charge and outside most of one.
        port(4, 0, NodeKind::Module, 20, Vec2::units(2000, 1810)),
        // 300 units away: outside every reach the rule can produce.
        port(5, 0, NodeKind::Port, 80, Vec2::units(2300, 2000)),
        // Directly below, so only the layer term stands between them.
        port(6, 1, NodeKind::Reserve, 100, Vec2::units(2000, 2000)),
    ];
    // Zero noise, for the same reason the chain fixture is quiet: what these
    // tests pin is the Pulse's exact arithmetic.
    let field = quiet(assembled(
        vec![layer(0, 0), layer(1, 0)],
        ports,
        Vec::new(),
        vec![form(1, 1, 0, Vec2::units(2000, 2000), 0)],
        Vec::new(),
    ));
    field::validate(&field).expect("the fixture is a valid Field");
    field
}

/// A control that holds the Pulse, releases it, or neither.
fn pulsing(held: bool, release: bool) -> ControlState {
    ControlState { pulse_held: held, pulse_release: release, ..ControlState::default() }
}

/// Runs `count` steps holding the Pulse, and hands back the charge reached.
fn charged_for(field: &mut FieldState, count: u32) -> Frac {
    for _ in 0..count {
        step_of(field, pulsing(true, false));
    }
    field.forms[0].pulse_charge
}

#[test]
fn a_held_control_charges_by_the_locked_share_and_a_release_consumes_it() {
    let mut field = pulse_field();

    // One held step is one locked share, and the rise is linear in held steps.
    assert_eq!(charged_for(&mut field, 1), PULSE_CHARGE_STEP);
    assert_eq!(charged_for(&mut field, 8), 9 * PULSE_CHARGE_STEP, "a nine-step tap");
    assert_eq!(9 * PULSE_CHARGE_STEP, 18_432, "the figure the locked text states");

    // Full in 32 held steps, and it sits there rather than climbing past.
    let mut field = pulse_field();
    assert_eq!(charged_for(&mut field, 32), FRAC_ONE, "full in 32 held steps");
    assert_eq!(charged_for(&mut field, 10), FRAC_ONE, "and holds at the whole");

    // The hold writes focus, and nothing else does.
    assert!(field.forms[0].focus, "a held step is a focused step");
    step_of(&mut field, ControlState::default());
    assert!(!field.forms[0].focus);

    // A step that neither holds nor releases zeroes a partial charge: the
    // locked focus-loss neutral frame discards a fumbled hold.
    let mut field = pulse_field();
    charged_for(&mut field, 6);
    step_of(&mut field, ControlState::default());
    assert_eq!(field.forms[0].pulse_charge, 0, "a fumbled hold is discarded, not emitted");

    // And an emission consumes the charge as it stands, without adding to it
    // first, whether or not the releasing frame still reports the hold.
    for release_holding in [false, true] {
        let mut field = pulse_field();
        let before = charged_for(&mut field, 5);
        assert_eq!(before, 5 * PULSE_CHARGE_STEP);
        let outcome = step_of(&mut field, pulsing(release_holding, true));
        assert_eq!(field.forms[0].pulse_charge, 0, "the emission consumed it");
        let emitted = outcome.cues.iter().find(|cue| cue.kind == CUE_PULSE_EMITTED);
        assert_eq!(
            emitted.expect("a release emits").a,
            reach_ticks(pulse_radius(before)),
            "the reach is the charge as it stood, with no increment on this step",
        );
    }
}

#[test]
fn the_pulse_reach_is_derived_from_the_charge_and_never_stored() {
    // The two ends of the locked formula, exactly.
    assert_eq!(pulse_radius(0), 8 * ONE_UNIT, "an empty release reaches 8 units");
    assert_eq!(pulse_radius(FRAC_ONE), 192 * ONE_UNIT, "a full charge reaches 192");
    assert_eq!(pulse_radius(FRAC_ONE / 2), PULSE_RADIUS_BASE + 184 * 32_768);
    // Nothing rounds: the reach is exact integer arithmetic on raw values.
    for charge in [0, 1, 2048, 18_432, 32_768, 65_535, FRAC_ONE] {
        assert_eq!(pulse_radius(charge), 524_288 + 184 * charge);
    }

    // Nothing stores it. The Form's serialized shape is what it was before the
    // Pulse: no radius field, so no byte of a payload carries one.
    let mut field = pulse_field();
    charged_for(&mut field, 3);
    let mut written = String::new();
    field.forms[0].write(&mut written);
    assert!(!written.contains("radius"), "{written}");
    assert!(written.contains("\"pulse_charge\":6144"), "{written}");
}

#[test]
fn a_pulse_reaches_no_further_than_its_own_layer() {
    // Node 6 stands directly below the Form: same position, one layer down. The
    // locked distance rule adds 512 units for that layer, and the widest reach
    // the charging rule can produce is 192, so it can never be gathered from —
    // confinement by construction rather than by a rule of its own.
    let mut field = pulse_field();
    let below = charge_of(&field, 6);
    charged_for(&mut field, 32);
    let outcome = step_of(&mut field, pulsing(false, true));
    assert_eq!(charge_of(&field, 6), below, "the layer below is out of reach at full charge");
    assert!(outcome.ledger.gathered > 0, "while its own layer is not");
    assert!(pulse_radius(FRAC_ONE) < LAYER_SEPARATION, "192 units against 512 of separation");
}

#[test]
fn an_emission_gathers_a_quarter_of_every_source_within_reach() {
    let mut field = pulse_field();
    let before: Vec<Fx> = (1..=6).map(|node| charge_of(&field, node)).collect();
    charged_for(&mut field, 32);
    let outcome = step_of(&mut field, pulsing(false, true));

    // A quarter of what each source within reach held, flooring, and nothing
    // from the two out of reach. The closed Port gives like any other source:
    // `open` does not gate gathering.
    let quarter = |units: i64| fixed_mul(units * ONE_UNIT, PULSE_GATHER_SHARE);
    assert_eq!(charge_of(&field, 2), 64 * ONE_UNIT - quarter(64), "the closed Port gave a quarter");
    assert_eq!(charge_of(&field, 3), 40 * ONE_UNIT - quarter(40));
    assert_eq!(charge_of(&field, 4), 20 * ONE_UNIT - quarter(20));
    assert_eq!(charge_of(&field, 5), before[4], "300 units away: out of reach");
    assert_eq!(charge_of(&field, 6), before[5], "a layer down: out of reach");

    // Every unit of it landed on the emitting Form's own Node, and the Form
    // reads its Node's Charge.
    let gathered = quarter(64) + quarter(40) + quarter(20);
    assert_eq!(outcome.ledger.gathered, gathered, "the ledger's transfer total");
    assert_eq!(charge_of(&field, 1), gathered);
    assert_eq!(field.forms[0].charge, gathered, "a Form's Charge is its own Node's");

    // A transfer, not a source: the stored total is what it was.
    let opening: Fx = before.iter().sum();
    let closing: Fx = (1..=6).map(|node| charge_of(&field, node)).sum();
    assert_eq!(opening, closing, "gathering creates and destroys nothing");
    assert_eq!(outcome.ledger.residual(), 0, "and the whole step balances exactly");
    assert!(outcome.ledger.balanced(), "per Node too");
    assert_eq!(outcome.ledger.sources(), 0, "gathering is no source");
    assert_eq!(outcome.ledger.sinks(), 0, "and no sink");

    // No Form takes from itself or from another Form.
    let mut two = pulse_field();
    two.forms.push(form(2, 7, 0, Vec2::units(2010, 2000), 30));
    two.ports.push(port(7, 0, NodeKind::Form, 30, Vec2::units(2010, 2000)));
    two.next_node_id = 8;
    field::validate(&two).expect("two Forms stand");
    charged_for(&mut two, 32);
    step_of(&mut two, pulsing(false, true));
    assert_eq!(charge_of(&two, 7), 30 * ONE_UNIT, "a Form-kind Node is never a source");
}

#[test]
fn gathering_stops_at_the_emitting_nodes_own_headroom() {
    let mut field = pulse_field();
    // The Form's Node stands one unit below the stored-Charge cap, and three
    // sources within reach hold far more than that unit between them.
    let place = field.ports.iter().position(|port| port.node == 1).expect("the Form's Node");
    field.ports[place].q = NODE_CHARGE_CAP - ONE_UNIT;
    field.ports[place].capacity = NODE_CHARGE_CAP;
    field.forms[0].charge = NODE_CHARGE_CAP - ONE_UNIT;
    field::validate(&field).expect("a nearly full Node is a valid one");

    charged_for(&mut field, 32);
    let outcome = step_of(&mut field, pulsing(false, true));

    assert_eq!(charge_of(&field, 1), NODE_CHARGE_CAP, "filled to the cap and no further");
    assert_eq!(outcome.ledger.gathered, ONE_UNIT, "only the headroom moved");
    // The first source in ascending order gave that unit; the pass stopped
    // before the others, so they are untouched.
    assert_eq!(charge_of(&field, 2), 64 * ONE_UNIT - ONE_UNIT);
    assert_eq!(charge_of(&field, 3), 40 * ONE_UNIT, "the pass stopped at headroom 0");
    assert_eq!(charge_of(&field, 4), 20 * ONE_UNIT);
    assert!(field::within_caps(&field), "no quantity left its cap");
    assert_eq!(outcome.ledger.residual(), 0);
}

#[test]
fn an_emission_opens_every_closed_port_within_reach_and_leaves_it_open() {
    let mut field = pulse_field();
    assert!(!field.ports[1].open, "node 2 stands closed");
    assert!(!field.ports[4].open, "and so does node 5, out of reach");

    charged_for(&mut field, 32);
    let outcome = step_of(&mut field, pulsing(false, true));

    assert!(field.ports[1].open, "the Port within reach opened");
    assert!(!field.ports[4].open, "the Port out of reach did not");
    // Opening costs nothing: no Charge moved for it, and no ledger entry was
    // written for it.
    let opened: Vec<u32> = outcome
        .cues
        .iter()
        .filter(|cue| cue.kind == CUE_PORT_OPENED)
        .map(|cue| cue.b)
        .collect();
    assert_eq!(opened, vec![2]);

    // One-way: a second Pulse changes nothing about it, and raises no cue for
    // a Port that is already open.
    charged_for(&mut field, 32);
    let again = step_of(&mut field, pulsing(false, true));
    assert!(field.ports[1].open, "nothing in version 1 closes an open Port");
    assert!(!again.cues.iter().any(|cue| cue.kind == CUE_PORT_OPENED), "and none opened twice");

    // A Reserve or a Module is established open and is never an activation:
    // only a Port kind waits to be opened.
    assert!(field.ports.iter().all(|port| port.kind == NodeKind::Port || port.open));
}

#[test]
fn an_emission_displaces_a_quarter_of_every_interference_level_within_reach() {
    let field = pulse_field();
    let at = |kind: TargetKind, id: u32| PressureState {
        pressure: Pressure::Interference,
        stage: Stage::Pressure,
        level: 40_000,
        primary: false,
        queued: false,
        start_step: 0,
        target: Target {
            kind,
            id: if kind == TargetKind::None { None } else { Some(id) },
        },
        displaced: None,
        bound: None,
    };
    let mut pressures = vec![
        at(TargetKind::Node, 2),                       // within reach
        at(TargetKind::Node, 5),                       // out of reach
        at(TargetKind::Node, 6),                       // a layer down
        at(TargetKind::Layer, 0),                      // never displaceable
        at(TargetKind::None, 0),                       // never displaceable
        PressureState { queued: true, ..at(TargetKind::Node, 2) },
        PressureState { pressure: Pressure::Drain, ..at(TargetKind::Node, 2) },
    ];
    let radius = pulse_radius(FRAC_ONE);
    let mut pressed = Vec::new();
    let reduced = field::displace_interference(
        &field,
        &pressures,
        Vec2::units(2000, 2000),
        0,
        radius,
        &mut pressed,
    );

    assert_eq!(reduced, 1, "one pressure stood within reach, active, and interference");
    // The press is collected for the boundary rather than written in the
    // step: the floor carries the current stage and a quarter off the
    // effective level.
    assert_eq!(
        pressed,
        vec![(
        Pressure::Interference.ordinal(),
        pressure::Displaced {
            stage: Stage::Pressure,
            level: 40_000 - fixed_mul(40_000, PULSE_DISPLACE_SHARE),
        },
    )]
    );
    for held in &pressures {
        assert_eq!(held.displaced, None, "the step itself writes nothing");
        assert_eq!(held.level, 40_000, "and the curve level is untouched");
    }

    // A Route target is displaced when either of its endpoints is within reach.
    let mut routed = across_layers();
    routed.forms[0].pos = Vec2::units(2048, 2048);
    let pressures = vec![at(TargetKind::Route, 1)];
    // Route 1 runs 3 → 4, both of them layers below: out of reach.
    let mut pressed = Vec::new();
    assert_eq!(
        field::displace_interference(
            &routed,
            &pressures,
            Vec2::units(2048, 2048),
            0,
            radius,
            &mut pressed,
        ),
        0,
    );
    // A Route with an endpoint beside the Form is displaced by it.
    routed.routes.push(RouteState {
        route: 3,
        tail: 2,
        head: 3,
        capacity: 8 * ONE_UNIT,
        flow: 0,
        formed_step: 0,
    });
    routed.next_route_id = 4;
    let pressures = vec![at(TargetKind::Route, 3)];
    assert_eq!(
        field::displace_interference(
            &routed,
            &pressures,
            Vec2::units(2048, 2048),
            0,
            radius,
            &mut pressed,
        ),
        1,
        "node 2 stands 52 units from the Form, so the Route it ends at is in reach",
    );

    // A level of nothing presses to nothing, and never below zero.
    let empty = vec![PressureState { level: 0, ..at(TargetKind::Node, 2) }];
    let mut floors = Vec::new();
    assert_eq!(
        field::displace_interference(
            &field,
            &empty,
            Vec2::units(2000, 2000),
            0,
            radius,
            &mut floors,
        ),
        1,
        "a zero level within reach is still pressed, to zero",
    );
    assert_eq!(floors[0].1.level, 0);

    // And the live call displaces the staged list the step is handed: an
    // emission within reach of an active Interference pressure's target
    // reduces it and raises cue 12 with the count, and the step reports the
    // membership boundary so the caller ends the window on it. The staging
    // carries an authored table whose opening stage holds the fixture's level
    // for longer than the step, so the stage machine leaves the displaced
    // level standing — the smaller of the two — rather than lifting it back.
    let mut live = pulse_field();
    charged_for(&mut live, 32);
    let table = PressureContent {
        pressure: Pressure::Interference,
        target: TargetKind::Node,
        stages: [
            StageRow { stage: Stage::Signal, level: 40_000, steps: 36_000 },
            StageRow { stage: Stage::Pressure, level: 40_000, steps: 1 },
            StageRow { stage: Stage::Crisis, level: 40_000, steps: 1 },
            StageRow { stage: Stage::Resolution, level: 40_000, steps: 1 },
        ],
    };
    let mut staged = Unstaged {
        pressures: vec![PressureState { stage: Stage::Signal, ..at(TargetKind::Node, 2) }],
        schedule: Schedule::of(vec![table]).expect("one table per pressure"),
        ..Default::default()
    };
    let outcome = advance(
        &mut live,
        pulsing(false, true),
        FRAC_ONE,
        &mut staged.staging(),
    );
    let pushed = outcome
        .cues
        .iter()
        .find(|cue| cue.kind == CUE_INTERFERENCE_PUSHED)
        .expect("the emission displaced a pressure");
    assert_eq!(pushed.a, 1, "one pressure was reduced");
    // The step derives the press and leaves the write to the boundary: the
    // list itself is untouched until the caller settles it, which is what
    // keeps every retained window replaying under the list it ran under.
    assert_eq!(staged.pressures[0].displaced, None);
    assert_eq!(
        outcome.staged.pressed,
        vec![(
            Pressure::Interference.ordinal(),
            pressure::Displaced {
                stage: Stage::Signal,
                level: 40_000 - fixed_mul(40_000, PULSE_DISPLACE_SHARE),
            },
        )],
        "the floor rides the outcome for the boundary to write",
    );
}

#[test]
fn an_emission_raises_its_cues_in_the_same_step_with_the_locked_payloads() {
    let mut field = pulse_field();
    let charge = charged_for(&mut field, 32);
    let outcome = step_of(&mut field, pulsing(false, true));

    // Cue 1 is the emission and the outcome cues ride the same step, which is
    // the locked signal that a Pulse did something.
    let kinds: Vec<u8> = outcome.cues.iter().map(|cue| cue.kind).collect();
    assert_eq!(kinds, vec![CUE_PULSE_EMITTED, CUE_CHARGE_GATHERED, CUE_PORT_OPENED]);
    let cue = |kind: u8| *outcome.cues.iter().find(|cue| cue.kind == kind).expect("the cue");

    // `b` is the Node the cue stands at, and `a` is locked per kind.
    assert_eq!(cue(CUE_PULSE_EMITTED).b, 1, "the emitting Form's Node");
    assert_eq!(cue(CUE_PULSE_EMITTED).a, reach_ticks(pulse_radius(charge)));
    assert_eq!(cue(CUE_PULSE_EMITTED).a, 192 * 256, "192 units in Q8.8");
    assert_eq!(cue(CUE_CHARGE_GATHERED).b, 1);
    assert_eq!(
        i64::from(cue(CUE_CHARGE_GATHERED).a),
        outcome.ledger.gathered >> 12,
        "the gathered total in 1/16-unit ticks",
    );
    assert_eq!(cue(CUE_PORT_OPENED).b, 2, "the Port that opened");
    assert_eq!(cue(CUE_PORT_OPENED).a, 0);

    // A release that reaches nothing raises the emission alone — which is what
    // makes a failed Pulse legible as one.
    let mut empty = pulse_field();
    let outcome = step_of(&mut empty, pulsing(false, true));
    assert_eq!(outcome.cues.len(), 1);
    assert_eq!(outcome.cues[0].kind, CUE_PULSE_EMITTED);
    assert_eq!(outcome.cues[0].a, 8 * 256, "8 units of reach, and nothing inside it");
    assert_eq!(outcome.ledger.gathered, 0);

    // A step that is not an emission raises nothing at all.
    assert!(step_of(&mut empty, pulsing(true, false)).cues.is_empty());
    assert!(step_of(&mut empty, ControlState::default()).cues.is_empty());
}

#[test]
fn the_pulse_acts_from_where_the_form_now_stands_and_before_the_routes_run() {
    // The Pulse phase sits after the kinematics: a Form moving toward a source
    // reaches what it can reach at the end of its own movement rather than at
    // the start of it. Node 4 stands 195 units from where this Form begins the
    // step — outside even a full charge's 192 — and 187.5 units from where it
    // ends it.
    let mut field = pulse_field();
    field.forms[0].pos = Vec2::units(2000, 2005);
    field.ports[0].pos = Vec2::units(2000, 2005);
    field.forms[0].pulse_charge = FRAC_ONE;
    field.forms[0].vel = Vec2::new(0, -10 * ONE_UNIT);
    field::validate(&field).expect("a moving Form is a valid one");
    let outcome = step_of(&mut field, pulsing(false, true));
    // The damper took its quarter before the position advanced, so the Form
    // travelled 7.5 of the 10 units it was carrying.
    assert_eq!(
        field.forms[0].pos,
        Vec2::new(2000 * ONE_UNIT, 1997 * ONE_UNIT + ONE_UNIT / 2),
        "the Form moved first",
    );
    assert!(
        charge_of(&field, 4) < 20 * ONE_UNIT,
        "and the Pulse reached the source that only its new place is near",
    );
    assert!(outcome.ledger.gathered > 0);

    // And it sits before the Route phase: Charge an emission gathers can move
    // along a Route in the same step.
    let mut field = pulse_field();
    field.routes.push(RouteState {
        route: 1,
        tail: 1,
        head: 3,
        capacity: 4 * ONE_UNIT,
        flow: 0,
        formed_step: 0,
    });
    field.next_route_id = 2;
    field::validate(&field).expect("a Field with one Route");
    charged_for(&mut field, 32);
    let outcome = step_of(&mut field, pulsing(false, true));
    assert_eq!(outcome.records.f, vec![(1, 4 * ONE_UNIT)], "the Route carried its capacity");
    assert!(outcome.ledger.moved > 0, "in the same step the Pulse gathered");
    assert_eq!(outcome.ledger.residual(), 0);
}

#[test]
fn the_release_edge_is_taken_by_the_first_executed_step_of_a_batch() {
    // The depth idiom, verbatim: one frame emits at most one Pulse however many
    // catch-up steps it runs, which is what bounds one release's gathering.
    let mut run = standing_under(pulse_field(), vec![1]);
    let play = |run: &mut Run, seq: u32, steps: u16, held: bool, release: bool| {
        let body = pulsing_frame(seq, steps, held, release);
        run.input_frame(&field_game_core::json::parse(&body).expect("canonical"), None)
            .expect("accepted");
    };

    play(&mut run, 1, 32, true, false);
    assert_eq!(run.state().now.forms[0].pulse_charge, FRAC_ONE, "a batch charges every step");
    play(&mut run, 2, 4, false, true);

    let released: Vec<bool> = run
        .state()
        .trace
        .steps
        .iter()
        .skip(32)
        .map(|step| step.ctl.pulse_release)
        .collect();
    assert_eq!(released, vec![true, false, false, false], "the first executed step, and no other");

    // One release, so one emission: the sources gave a quarter once rather than
    // four times.
    let quarter = fixed_mul(64 * ONE_UNIT, PULSE_GATHER_SHARE);
    assert_eq!(charge_of(&run.state().now, 2), 64 * ONE_UNIT - quarter);
}

#[test]
fn a_frame_at_its_cue_cap_never_drops_the_emission_the_rest_is_read_against() {
    // Fifteen closed Ports and a source, all within reach of one emission: the
    // emission, the gather, and fifteen activations are seventeen cues against
    // a cap of sixteen.
    let mut ports = vec![port(1, 0, NodeKind::Form, 0, Vec2::units(2000, 2000))];
    for index in 0..15u32 {
        let turn = index as f64 * 0.4;
        ports.push(port(
            index + 2,
            0,
            NodeKind::Port,
            // One of them holds Charge, so the emission gathers as well.
            if index == 0 { 40 } else { 0 },
            Vec2::units(
                2000 + (60.0 * turn.cos()) as i64,
                2000 + (60.0 * turn.sin()) as i64,
            ),
        ));
    }
    let mut field = assembled(
        vec![layer(0, 0)],
        ports,
        Vec::new(),
        vec![form(1, 1, 0, Vec2::units(2000, 2000), 0)],
        Vec::new(),
    );
    field::validate(&field).expect("the fixture is a valid Field");

    charged_for(&mut field, 32);
    let outcome = step_of(&mut field, pulsing(false, true));

    assert_eq!(outcome.cues.len(), CUES_PER_FRAME, "the frame stands at its cap");
    // The one cue that is never dropped is the emission: the frame's own reach
    // is read from it, and an outcome cue beside it is the whole of the signal
    // that the Pulse did something.
    assert_eq!(outcome.cues[0].kind, CUE_PULSE_EMITTED);
    assert_eq!(outcome.cues.iter().filter(|cue| cue.kind == CUE_PULSE_EMITTED).count(), 1);
    assert_eq!(
        outcome.cues.iter().filter(|cue| cue.kind == CUE_PORT_OPENED).count(),
        15,
        "every activation stands; the oldest cue of another kind went",
    );
    assert!(
        !outcome.cues.iter().any(|cue| cue.kind == CUE_CHARGE_GATHERED),
        "and the oldest of those was the gather",
    );
    // The Field itself is untouched by the dropped cue: every Port opened.
    assert!(field.ports.iter().all(|port| port.open));
    assert!(outcome.ledger.gathered > 0, "and the gather it stopped reporting still happened");
    assert_eq!(outcome.ledger.residual(), 0);
}

#[test]
fn two_controlled_forms_emit_in_ascending_form_id() {
    // One source between two controlled Forms, both emitting in the same step:
    // the ascending Form order decides who reaches it first, and the cues come
    // out in that order too.
    let mut field = pulse_field();
    field.forms.push(FormState {
        id: 2,
        controlled: true,
        ..form(2, 7, 0, Vec2::units(2200, 2000), 0)
    });
    field.ports.push(port(7, 0, NodeKind::Form, 0, Vec2::units(2200, 2000)));
    field.next_node_id = 8;
    field.layers[0].port_ids = vec![2, 3, 4, 5];
    field::validate(&field).expect("two controlled Forms stand");

    // Node 2 stands 100 units from each of them, so both reach it and the
    // ascending order decides what is left for the second.
    let source = charge_of(&field, 2);
    charged_for(&mut field, 32);
    let outcome = step_of(&mut field, pulsing(false, true));

    let quarter = |held: Fx| fixed_mul(held, PULSE_GATHER_SHARE);
    let first = quarter(source);
    let second = quarter(source - first);
    assert!(second > 0 && second < first, "the higher id took a quarter of what was left");
    assert_eq!(charge_of(&field, 2), source - first - second, "the shared source gave twice");
    // What each Form ended with is the sum of its own quarters: the first
    // reaches nodes 2, 3, and 4, the second reaches node 2 and node 5.
    assert_eq!(
        charge_of(&field, 1),
        first + quarter(40 * ONE_UNIT) + quarter(20 * ONE_UNIT),
        "the lower Form id gathered first, and from everything in its own reach",
    );
    assert_eq!(charge_of(&field, 7), second + quarter(80 * ONE_UNIT));

    // The cues come out in the same order: everything the first Form raised,
    // then everything the second did.
    let raised: Vec<(u8, u32)> = outcome.cues.iter().map(|cue| (cue.kind, cue.b)).collect();
    assert_eq!(
        raised,
        vec![
            (CUE_PULSE_EMITTED, 1),
            (CUE_CHARGE_GATHERED, 1),
            (CUE_PORT_OPENED, 2),
            (CUE_PULSE_EMITTED, 7),
            (CUE_CHARGE_GATHERED, 7),
            (CUE_PORT_OPENED, 5),
        ],
        "the first Form's emission and outcomes, then the second's",
    );
    assert_eq!(outcome.ledger.residual(), 0);
    assert!(outcome.ledger.balanced(), "per Node too");
}

// ---------------------------------------------------------------------------
// The four Form ability rules
// ---------------------------------------------------------------------------

/// A Field of one Form and one Port on the same layer, with nothing that moves
/// Charge but the rules under test.
fn ability_field(upkeep_units: i64) -> FieldState {
    let mut ports = vec![
        port(1, 0, NodeKind::Form, 8, Vec2::units(1000, 1000)),
        port(2, 0, NodeKind::Port, 40, Vec2::units(1010, 1000)),
        port(3, 0, NodeKind::Port, 40, Vec2::units(1030, 1000)),
    ];
    for held in &mut ports {
        held.upkeep_rate = upkeep_units * ONE_UNIT;
    }
    let field = assembled(
        vec![layer(0, 0)],
        ports,
        Vec::new(),
        vec![form(1, 1, 0, Vec2::units(1000, 1000), 8)],
        Vec::new(),
    );
    field::validate(&field).expect("the fixture is a valid Field");
    field
}

#[test]
fn every_node_pays_the_upkeep_it_was_authored_with_into_the_one_sink() {
    // Three Nodes, each authored at two units a step: the Form's own Node holds
    // eight and pays two, and the two Ports hold forty and pay two. Nothing
    // else moves Charge in this Field, so the whole step is the payment.
    let mut field = ability_field(2);
    let outcome = step_of(&mut field, ControlState::default());

    assert_eq!(charge_of(&field, 1), 6 * ONE_UNIT, "the Form's own Node paid its two");
    assert_eq!(charge_of(&field, 2), 38 * ONE_UNIT);
    assert_eq!(charge_of(&field, 3), 38 * ONE_UNIT);
    assert_eq!(outcome.ledger.upkeep, 6 * ONE_UNIT, "three Nodes at two units each");
    assert_eq!(outcome.ledger.sinks(), 6 * ONE_UNIT, "and the sink is the only one that moved");
    assert_eq!(outcome.ledger.residual(), 0, "the step balances");
    assert!(outcome.ledger.balanced(), "and balances per Node");

    // The record: what each Node paid, ascending, attributed whole to the first
    // of the five purposes, which is what version 1 locks.
    let paid: Vec<(u32, i64, [i64; 5])> =
        outcome.records.upkeep.iter().map(|entry| (entry.node, entry.v, entry.mix)).collect();
    assert_eq!(
        paid,
        vec![
            (1, 2 * ONE_UNIT, [2 * ONE_UNIT, 0, 0, 0, 0]),
            (2, 2 * ONE_UNIT, [2 * ONE_UNIT, 0, 0, 0, 0]),
            (3, 2 * ONE_UNIT, [2 * ONE_UNIT, 0, 0, 0, 0]),
        ],
    );
    assert!(outcome.records.z.is_empty(), "every Node paid in full, so none failed");
}

#[test]
fn a_node_that_cannot_pay_its_upkeep_pays_what_it_holds_and_stands_as_failed() {
    // No debt: a Node holding less than its rate pays the whole of what it
    // holds and records the failure indicator FRAMEWORK.md locks for exactly
    // this branch. The Node that ends the step at nothing records it too, which
    // is the branch that has always been live.
    let mut field = ability_field(0);
    field.ports[0].upkeep_rate = 20 * ONE_UNIT;
    field.ports[1].upkeep_rate = 4 * ONE_UNIT;
    let outcome = step_of(&mut field, ControlState::default());

    assert_eq!(charge_of(&field, 1), 0, "it paid the eight it had against a rate of twenty");
    assert_eq!(charge_of(&field, 2), 36 * ONE_UNIT);
    assert_eq!(outcome.ledger.upkeep, 12 * ONE_UNIT);
    assert_eq!(outcome.records.z, vec![1], "the Node that could not pay in full");
    assert_eq!(outcome.records.upkeep[0].v, 8 * ONE_UNIT, "what it paid, not what it owed");
    assert_eq!(outcome.ledger.residual(), 0);
    assert!(outcome.ledger.balanced());
}

/// The same Field with a Trail authored on its one Form.
fn trailing_field(period: u16, delay: u16, radius_units: i64, magnitude_units: i64) -> FieldState {
    let mut field = ability_field(0);
    field.forms[0].trail = Some(field::TrailState {
        period,
        delay,
        radius: radius_units * ONE_UNIT,
        magnitude: magnitude_units * ONE_UNIT,
    });
    field::validate(&field).expect("the fixture is a valid Field");
    field
}

#[test]
fn a_trail_is_left_on_the_steps_its_period_names_and_moves_nothing_when_it_is() {
    let mut field = trailing_field(5, 30, 32, 12);
    let opening: Vec<i64> = field.ports.iter().map(|port| port.q).collect();

    for step in 1..=5 {
        let outcome = step_of(&mut field, ControlState::default());
        assert_eq!(outcome.ledger.residual(), 0, "step {step} balances");
        assert_eq!(outcome.ledger.wake, 0, "a deposit delivers nothing");
    }
    // One entry, left on the fifth step, due thirty steps later, carrying the
    // magnitude its Form authored — and no Charge moved to leave it.
    assert_eq!(field.pending.len(), 1);
    assert_eq!(field.pending[0].due, 35);
    assert_eq!(field.pending[0].form, 1);
    assert_eq!(field.pending[0].magnitude, 12 * ONE_UNIT);
    assert_eq!(field.ports.iter().map(|port| port.q).collect::<Vec<i64>>(), opening);
}

#[test]
fn a_due_trail_delivers_its_whole_magnitude_by_the_locked_split() {
    // Two Ports and the Form's own Node stand inside the reach, so a magnitude
    // of thirteen units splits four, four, four with one raw unit over: the
    // first recipient in ascending NodeId takes it, exactly as a current's
    // delivery does.
    let mut field = trailing_field(5, 30, 64, 13);
    let mut delivered = 0;
    let mut at = 0;
    for step in 1..=35 {
        let outcome = step_of(&mut field, ControlState::default());
        assert_eq!(outcome.ledger.residual(), 0, "step {step} balances");
        assert!(outcome.ledger.balanced(), "and balances per Node");
        if outcome.ledger.wake > 0 {
            delivered = outcome.ledger.wake;
            at = step;
        }
    }
    assert_eq!(at, 35, "the entry left on step 5 came due thirty steps later");
    assert_eq!(delivered, 13 * ONE_UNIT, "the whole magnitude reached the Field");
    let base = 13 * ONE_UNIT / 3;
    assert_eq!(charge_of(&field, 1), 8 * ONE_UNIT + base + 1, "the first takes the odd raw unit");
    assert_eq!(charge_of(&field, 2), 40 * ONE_UNIT + base);
    assert_eq!(charge_of(&field, 3), 40 * ONE_UNIT + base);
    // The queue holds only what is still standing: the entries left on steps
    // 10 through 35, and not the one that came due.
    assert!(field.pending.iter().all(|entry| entry.due > field.step));
    assert_eq!(field.pending.len(), 6);
}

#[test]
fn the_trail_queue_holds_its_cap_and_drops_the_oldest_entry_past_it() {
    // One Form cannot reach the cap: the widest ratio its authored bounds admit
    // is a deposit every five steps against a delay of three hundred, which is
    // sixty entries standing. Three Forms leaving one each is what the cap is
    // for, and what it does is hold the newest sixty-four.
    let mut field = trailing_field(5, 300, 8, 4);
    for id in 2..=3u8 {
        let mut held = form(id, u32::from(id), 0, Vec2::units(1000 + i64::from(id), 1000), 8);
        held.controlled = false;
        held.trail = field.forms[0].trail;
        field.ports[usize::from(id) - 1].kind = NodeKind::Form;
        field.ports[usize::from(id) - 1].pos = held.pos;
        field.ports[usize::from(id) - 1].q = held.charge;
        field.forms.push(held);
    }
    field.layers[0].port_ids = Vec::new();
    field::validate(&field).expect("three Forms of one Field is a valid Field");

    for _ in 0..(5 * 30) {
        step_of(&mut field, ControlState::default());
    }
    assert_eq!(field.pending.len(), field::PENDING_TRAILS, "the queue stands at its cap");
    let due: Vec<u32> = field.pending.iter().map(|entry| entry.due).collect();
    assert!(due.windows(2).all(|pair| pair[0] <= pair[1]), "deposit order, oldest first");
    assert_eq!(
        *due.last().expect("an entry"),
        field.step + 300 - (field.step % 5),
        "the newest entry is the one the last deposit left",
    );
    // What the cap dropped is the oldest: nothing standing is older than the
    // newest entry less the depth the cap holds.
    assert!(
        due[0] > field.step,
        "and every entry still standing is still to come",
    );
}

#[test]
fn a_trail_that_comes_due_where_nothing_stands_delivers_nothing_and_leaves() {
    // The reach is the whole of what decides a recipient: an entry left where
    // no Node stands within it delivers nothing, and leaves the queue all the
    // same rather than standing forever.
    let mut field = trailing_field(5, 30, 1, 12);
    field.forms[0].pos = Vec2::units(3000, 3000);
    field.ports[0].pos = field.forms[0].pos;
    for step in 1..=35 {
        let outcome = step_of(&mut field, ControlState::default());
        // The Form's own Node stands where the entry was left, so it is the one
        // recipient: the two Ports are far outside a one-unit reach.
        assert_eq!(outcome.ledger.residual(), 0, "step {step} balances");
    }
    assert_eq!(charge_of(&field, 2), 40 * ONE_UNIT, "nothing reached the Ports");
    assert!(field.pending.iter().all(|entry| entry.due > field.step));
}

/// A Field standing a Chorus: one steered Form and one linked to it, with the
/// link authored at the offset and separation the caller names.
fn linked_pair(offset: Vec2, separation_units: i64) -> FieldState {
    let mut ports = vec![
        port(1, 0, NodeKind::Form, 8, Vec2::units(1000, 1000)),
        port(2, 0, NodeKind::Form, 8, Vec2::units(1000 + offset.x / ONE_UNIT, 1000)),
    ];
    ports[1].pos = Vec2::new(ports[0].pos.x + offset.x, ports[0].pos.y + offset.y);
    let mut leader = form(1, 1, 0, ports[0].pos, 8);
    let mut linked = form(2, 2, 0, ports[1].pos, 8);
    leader.form = "chorus".to_string();
    linked.form = "chorus".to_string();
    linked.controlled = false;
    linked.link =
        Some(field::LinkState { offset, separation: separation_units * ONE_UNIT });
    let field = assembled(
        vec![layer(0, 0)],
        ports,
        Vec::new(),
        vec![leader, linked],
        vec![current(1, 0, 1)],
    );
    field::validate(&field).expect("the fixture is a valid Field");
    field
}

#[test]
fn a_linked_form_follows_the_station_its_link_authors() {
    // The leader is steered away and the linked Form follows on its own derived
    // control, which is a station rather than a place: it trails by a bounded
    // distance while the leader runs, and closes onto the station when the
    // leader stops. It is never steered, and it holds no control of its own.
    let offset = Vec2::units(-40, 0);
    let mut field = linked_pair(offset, 256);
    let away = |field: &FieldState| -> i64 {
        let leader = field.forms[0].pos;
        let station = Vec2::new(leader.x + offset.x, leader.y + offset.y);
        (field.forms[1].pos.x - station.x).abs() + (field.forms[1].pos.y - station.y).abs()
    };

    let control = ControlState { steer_x: 32767, ..ControlState::default() };
    let mut running: Vec<i64> = Vec::new();
    for _ in 0..90 {
        step_of(&mut field, control);
        running.push(away(&field));
    }
    assert!(field.forms[1].pos.x > 1000 * ONE_UNIT, "the linked Form moved, and was never steered");
    // The trail settles rather than growing without bound: a follower at the
    // leader's own terminal speed holds a fixed distance behind its station.
    let settled = running[89] - running[88];
    assert!(settled >= 0 && settled < ONE_UNIT / 16, "the lag settles: {settled} raw a step");
    // What it settles at is the reach itself: a follower matching the leader's
    // terminal speed has to name a full-deflection control, and a full
    // deflection is exactly one reach of station. So a Chorus running flat out
    // stands past its own authored separation — which is the vulnerability the
    // promise names, arrived at from the two rules rather than added to them.
    let lag = running[89] / ONE_UNIT;
    assert!((315..=321).contains(&lag), "and it settles at the reach: {lag} units");

    // The leader stops, and the follower closes on its station and parks: a
    // Form standing at its station names no control at all, so the damper
    // floors what is left rather than leaving it drifting.
    for _ in 0..120 {
        step_of(&mut field, ControlState::default());
    }
    assert!(away(&field) < ONE_UNIT, "it stands within a unit of its station: {}", away(&field));
    // And what is left of its motion is under the rest floor the steering rule
    // calls still — a thousandth of a unit a step, which no surface can draw.
    assert!(field.forms[1].vel.x.abs() < field::STEER_REST, "{:?}", field.forms[1].vel);
    assert!(field.forms[1].vel.y.abs() < field::STEER_REST, "{:?}", field.forms[1].vel);
}

#[test]
fn a_linked_form_takes_the_depth_change_on_the_step_its_leader_takes_it() {
    let mut field = linked_pair(Vec2::units(-40, 0), 256);
    field.layers = vec![layer(0, 0), layer(1, 1)];
    field.layers[0].port_ids = Vec::new();
    field.layers[1].port_ids = Vec::new();
    field.layers[0].current_ids = vec![1];
    step_of(&mut field, ControlState { depth_move: 1, ..ControlState::default() });
    assert_eq!(field.forms[0].layer, 1, "the steered Form descended");
    assert_eq!(field.forms[1].layer, 1, "and the linked Form is on the same layer, the same step");
}

#[test]
fn a_separated_linked_form_accepts_no_delivery_until_it_is_back_inside() {
    // The linked Form is placed past its authored separation, so the current
    // delivering to both Forms delivers to one of them; nothing is a sink for
    // the share it refuses, because the share is never emitted to it.
    let mut apart = linked_pair(Vec2::units(-40, 0), 8);
    apart.forms[1].pos = Vec2::units(600, 1000);
    apart.ports[1].pos = apart.forms[1].pos;
    // Standing still: the derived control would close the gap, so the follow is
    // held off by giving the linked Form the whole step at its own place.
    apart.forms[1].link = Some(field::LinkState { offset: Vec2::units(-400, 0), separation: 8 * ONE_UNIT });
    let outcome = step_of(&mut apart, ControlState::default());
    assert!(outcome.ledger.current > 0, "the current delivered to what stands in it");
    let separated = apart.forms[1].node;
    assert_eq!(
        outcome.records.e.iter().find(|(node, _)| *node == separated),
        None,
        "the separated Form's Node took no exogenous share at all",
    );
    assert_eq!(outcome.ledger.residual(), 0, "and no sink stands for what it refused");
    assert!(outcome.ledger.balanced());

    // The same Field with the linked Form standing inside its separation: the
    // Node is a recipient again at once, with no latch in between.
    let mut together = linked_pair(Vec2::units(-8, -8), 256);
    let outcome = step_of(&mut together, ControlState::default());
    let linked = together.forms[1].node;
    assert!(
        outcome.records.e.iter().any(|(node, _)| *node == linked),
        "a Form inside its separation is delivered to",
    );
}

#[test]
fn the_steering_scale_a_form_authors_moves_its_terminal_speed_and_nothing_else() {
    // Two Fields, one control, and the scales at the ends of the locked range.
    // Terminal speed is linear in the reach the scale moves, so the fast Form
    // covers four times the ground of the slow one, while the settling share
    // per step — the damper's own quarter — is the same for both.
    let speed = |scale: i64| -> (i64, i64) {
        let mut field = ability_field(0);
        field.forms[0].steer_scale = scale;
        let control = ControlState { steer_x: 32767, ..ControlState::default() };
        let mut first = 0;
        for step in 1..=64 {
            step_of(&mut field, control);
            if step == 1 {
                first = field.forms[0].vel.x;
            }
        }
        (field.forms[0].vel.x, first)
    };
    let (fast, fast_first) = speed(field::STEER_SCALE_HIGH);
    let (slow, slow_first) = speed(field::STEER_SCALE_LOW);
    let (default, default_first) = speed(field_game_core::state::FRAC_ONE);

    // The locked identity `STIFFNESS * REACH / DAMPING` is twenty units a step
    // at the reference reach, and the flooring composition lands a hair under
    // it: eighty at the ceiling, twenty at the default, five at the floor.
    println!("terminal speeds, raw: slow {slow} default {default} fast {fast}");
    assert_eq!(default, 1_310_680, "the reference terminal speed stands, in raw units");
    assert_eq!(fast, 4 * default, "four times it at the ceiling");
    // A quarter of it at the floor, to the raw unit or two the composed floors
    // lose at the small end — the composition floors twice, and 8 raw units is
    // an eight-thousandth of one unit.
    assert!((default - slow * 4).abs() <= 16, "a quarter at the floor: {slow} against {default}");
    assert_eq!(default / ONE_UNIT, 19, "which is the locked twenty units a step, floored");
    assert_eq!(fast / ONE_UNIT, 79, "and the locked eighty at the ceiling, floored");
    // The first step of a gesture reaches the same share of the terminal speed
    // whatever the scale: the feel is the damper's, and the scale is not.
    assert_eq!(fast_first, 4 * default_first, "the same share of a larger speed");
    assert!((default_first - slow_first * 4).abs() <= 16, "and of a smaller one");
}
