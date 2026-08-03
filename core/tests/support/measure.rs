//! The Fields the coordinate and perturbation tests read.
//!
//! Both are built so the step function's own arithmetic is short enough to
//! carry out by hand: Route flow and nothing else moves Charge in the circuit,
//! and a layer's Drain and nothing else moves it in the drained one. Every
//! expected number in the tests that read them is written out as the arithmetic
//! that produces it, so a reader can check FRAMEWORK.md's formula against the
//! fixture without running anything.
//!
//! The recorded window is played rather than written: the trace is what
//! [`field::advance`] produced from the opening state, so a replay with no edit
//! reproduces it exactly and a deviation is a reading of the edit.

use field_game_core::field::{
    self, BoundaryState, FieldLayer, NodeKind, PhysicalCompartment, PortState, RouteState,
};
use field_game_core::fx::{Vec2, ONE_UNIT};
use field_game_core::state::{
    ControlState, FieldState, GeneratorSpec, InputConfig, Progress, RunState, Surround, Trace,
    TraceStep, ViewDeclaration, FRAC_ONE,
};

pub const KEY: &str = "0123456789abcdef";
pub const NO_CONTENT: &str =
    "00000000000000000000000000000000000000000000000000000000000000ff";

/// One Route's capacity in the circuit, in raw `Fx`.
pub const LINK: i64 = 8 * ONE_UNIT;

/// The narrow Route that carries Charge out of the inside.
pub const OUTLET: i64 = 4 * ONE_UNIT;

fn port(node: u32, layer: u8, x: i64, kind: NodeKind, q: i64) -> PortState {
    PortState {
        node,
        layer,
        // Every Node stands 300 units from the next, which is past the
        // 256-unit adjacency radius, so the declared adjacency is empty and
        // every neighbourhood rule reads Routes alone.
        pos: Vec2::units(x, 300),
        kind,
        q: q * ONE_UNIT,
        open: true,
        upkeep_rate: 0,
        // The overload threshold sits at the stored-Charge cap, so nothing here
        // ever overloads and no Node sheds a quarter of an excess.
        capacity: 4096 * ONE_UNIT,
    }
}

fn route(id: u32, tail: u32, head: u32, capacity: i64) -> RouteState {
    RouteState { route: id, tail, head, capacity, flow: 0, formed_step: 0 }
}

/// **The circuit.** One layer, no Drain, six Nodes, six Routes.
///
/// ```text
///   1 (port, 40)  --r1-->  2 (port, 0)  --r2-->  3 (port, 0)  --r3-->  4 (reserve, 0)
///                              ^                                          |
///                              |                                          | r4 (cap 4)
///                              +---------------- r5 (cap 8) --------------+--> 5 (port, 0)
///   6 (port, 0)  --r6-->  1
/// ```
///
/// The inside every test reads is `{2, 3, 4}`. Node 1 supplies it, Node 5 takes
/// from it, and Node 6 stands beyond Node 1 with a Route that never carries —
/// which is what makes the `adjacent` surround `{1, 5}` and the `double`
/// surround `{1, 5, 6}`, so a surround change is a change.
///
/// The recorded stored-Charge total of the inside, in whole units, over the
/// first eight steps is `4, 8, 12, 16, 20, 16, 12, 8`: Node 1 supplies eight
/// units a step for five steps and then stands empty, and Node 5 takes four a
/// step throughout.
pub fn circuit() -> FieldState {
    let mut field = FieldState::opening();
    field.next_node_id = 7;
    field.next_route_id = 7;
    field.ports = vec![
        port(1, 0, 300, NodeKind::Port, 40),
        port(2, 0, 600, NodeKind::Port, 0),
        port(3, 0, 900, NodeKind::Port, 0),
        // A `reserve` Node, so a substitution that replaces it puts 64 units of
        // starting Charge where it stood and the edit is visible.
        port(4, 0, 1200, NodeKind::Reserve, 0),
        port(5, 0, 1500, NodeKind::Port, 0),
        port(6, 0, 1800, NodeKind::Port, 0),
    ];
    field.routes = vec![
        route(1, 1, 2, LINK),
        route(2, 2, 3, LINK),
        route(3, 3, 4, LINK),
        // The outlet runs before the recycle, so Node 4 pays it first.
        route(4, 4, 5, OUTLET),
        route(5, 4, 2, LINK),
        route(6, 6, 1, LINK),
    ];
    field.layers = vec![FieldLayer {
        layer: 0,
        drain: 0,
        noise: 0,
        gain: 0,
        current_ids: Vec::new(),
        port_ids: vec![1, 2, 3, 4, 5, 6],
    }];
    field.physical_compartment = PhysicalCompartment {
        members: vec![2, 3, 4],
        leak_per_exposed_contact_per_step: 0,
    };
    field.boundaries = BoundaryState { drawn: Vec::new(), authored: Vec::new() };
    field
}

/// The circuit with an upkeep rate authored on each of the three members, and
/// a stock for them to pay it out of.
///
/// The Nodes and the Routes are the plain circuit's, so the crossing Routes are
/// the same two: `1 -> 2` carries in at its capacity of eight a step while Node
/// 1 stands supplied, and `4 -> 5` carries out at its own four. What this adds
/// is the one thing the Node phase reads and a stored stock large enough that
/// every member pays its rate in full — which is what makes Self-Support a
/// reading of the supply rather than of an empty Node.
pub fn circuit_with_upkeep(rate_units: i64, stock_units: i64) -> FieldState {
    let mut field = circuit();
    for held in &mut field.ports {
        if [2, 3, 4].contains(&held.node) {
            held.upkeep_rate = rate_units * ONE_UNIT;
            held.q = stock_units * ONE_UNIT;
        }
        if held.node == 1 {
            held.q = 200 * ONE_UNIT;
        }
    }
    field
}

/// The circuit with a controlled Form standing far from it.
///
/// The Form's Node is number 7, Form-kind, at (3500, 3500) — past the 256-unit
/// adjacency radius of every circuit Node, in no current, on the drainless
/// layer, and steered by nothing (every fixture control is neutral) — so every
/// recorded series of the circuit is unchanged and every hand trace in the
/// tests stands. What the Form adds is the two authored-per-Form quantities a
/// reading needs a controlled Form for: `route_reach`, which the `connect` and
/// `redirect` preconditions measure against, and `forecast_depth`, which is
/// the Field's declared `a_F`.
pub fn circuit_with_form(forecast_depth: u16) -> FieldState {
    let mut field = circuit();
    field.next_node_id = 8;
    field.ports.push(PortState {
        node: 7,
        layer: 0,
        pos: Vec2::units(3500, 3500),
        kind: NodeKind::Form,
        q: 8 * ONE_UNIT,
        open: true,
        upkeep_rate: 0,
        capacity: 4096 * ONE_UNIT,
    });
    field.layers[0].port_ids = vec![1, 2, 3, 4, 5, 6];
    field.forms = vec![field_game_core::field::FormState {
        id: 1,
        form: "thread".to_string(),
        node: 7,
        controlled: true,
        layer: 0,
        pos: Vec2::units(3500, 3500),
        vel: Vec2 { x: 0, y: 0 },
        charge: 8 * ONE_UNIT,
        reserve: 0,
        pulse_charge: 0,
        focus: false,
        route_reach: 4000 * ONE_UNIT,
        forecast_depth,
        steer_scale: field_game_core::state::FRAC_ONE,
        route_capacity: 32 * ONE_UNIT,
        link: None,
        trail: None,
    }];
    field
}

/// **The drained Field.** One Node on a layer that drains six units a step, and
/// no Route at all.
///
/// The whole of its exogenous schedule is that Drain: `e(1, t) = -6` at every
/// step while the Node holds at least six. Nothing else moves Charge, so a
/// delay applied to that schedule is the only thing a shifted replay changes.
pub fn drained() -> FieldState {
    let mut field = FieldState::opening();
    field.next_node_id = 2;
    field.next_route_id = 1;
    field.ports = vec![port(1, 0, 300, NodeKind::Port, 60)];
    field.routes = Vec::new();
    field.layers = vec![FieldLayer {
        layer: 0,
        drain: 6 * ONE_UNIT,
        noise: 0,
        gain: 0,
        current_ids: Vec::new(),
        port_ids: vec![1],
    }];
    field.physical_compartment = PhysicalCompartment {
        members: vec![1],
        leak_per_exposed_contact_per_step: 0,
    };
    field.boundaries = BoundaryState { drawn: Vec::new(), authored: Vec::new() };
    field
}

/// A View over an inside, with the declared window and the `adjacent` surround.
pub fn view_of(inside: &[u32], window: u16) -> ViewDeclaration {
    ViewDeclaration {
        inside: inside.to_vec(),
        resolution: 1,
        window,
        surround: Surround::Adjacent,
    }
}

/// Plays a Field forward and hands back the run it leaves: the keyframe at step
/// 0, the recorded steps after it, and the Field the last one produced.
pub fn played(opening: FieldState, steps: usize, view: ViewDeclaration) -> RunState {
    let keyframe = opening;
    let mut now = keyframe.clone();
    let mut trace = Trace::opening(keyframe);
    for _ in 0..steps {
        let position = field_game_core::rng::RngState { key: 0, ctr: 0, half: 0 };
        let outcome = field::advance(
            &mut now,
            ControlState::default(),
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
        spec: GeneratorSpec::new(NO_CONTENT.to_string(), Default::default()),
        branch_nonce: 0,
        progress: Progress::opening(),
        now,
        trace,
        view,
        slate: None,
        input_config: InputConfig::default_config(),
        pressures: Vec::new(),
        anchors: Vec::new(),
    }
}

/// The stored-Charge total of an inside over the recorded window, in whole
/// units — the series every hand-checked number below is read off.
pub fn recorded_units(state: &RunState, inside: &[u32]) -> Vec<i64> {
    state
        .trace
        .steps
        .iter()
        .map(|step| {
            step.records
                .q
                .iter()
                .filter(|(node, _)| inside.contains(node))
                .map(|(_, value)| value)
                .sum::<i64>()
                / ONE_UNIT
        })
        .collect()
}

/// `sigma_V` for a test evaluation: the same split a slate declares.
pub fn sigma() -> field_game_core::rng::RngState {
    field_game_core::slate::evaluation_stream(KEY, 0, 1)
}
