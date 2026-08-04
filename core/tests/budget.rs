//! The caps fixture: the equality pin on the prepared step pass, and the
//! measurement harness the Still Mode analysis budget's table is taken from.
//!
//! The equality test runs with the suite. The measurements are ignored, because
//! a measurement is not an assertion; run them explicitly, in release:
//!
//! ```text
//! cargo test --manifest-path core/Cargo.toml --release --test budget -- --ignored --nocapture
//! ```

use field_game_core::field::{
    self, BoundaryState, CurrentState, FieldLayer, FormState, NodeKind, PhysicalCompartment,
    PortState, RouteState,
};
use field_game_core::fx::{Vec2, ONE_UNIT};
use field_game_core::rng::RngState;
use field_game_core::state::{ControlState, FieldState, TraceStep};

/// The caps: 256 Nodes, 512 Routes, 32 currents of 64 path points, 8 layers,
/// 8 Forms, a 60-step window — and, since the Form abilities landed, every
/// Node paying an authored upkeep and the Trail queue standing at its own cap
/// with an entry falling due on every step of the window. The budget is
/// measured over the widest Field a run can stand, and both are part of it.
fn caps_field() -> FieldState {
    let mut field = FieldState::opening();
    field.next_node_id = 257;
    field.next_route_id = 513;

    // 8 Form-kind Nodes, ids 1..=8, one per layer.
    let mut ports = Vec::new();
    for id in 1..=8u32 {
        ports.push(PortState {
            node: id,
            layer: (id - 1) as u8,
            pos: Vec2::units(1000 + i64::from(id) * 7, 1000 + i64::from(id) * 11),
            kind: NodeKind::Form,
            q: 16 * ONE_UNIT,
            open: true,
            upkeep_rate: ONE_UNIT / 4,
            capacity: 512 * ONE_UNIT,
        });
    }
    // 248 Port-kind Nodes spread over the 8 layers, 31 per layer, on a grid.
    for id in 9..=256u32 {
        let place = id - 9;
        let layer = (place % 8) as u8;
        let row = place / 8;
        ports.push(PortState {
            node: id,
            layer,
            pos: Vec2::units(200 + i64::from(row % 16) * 220, 200 + i64::from(row / 16) * 220),
            kind: NodeKind::Port,
            q: 100 * ONE_UNIT,
            open: true,
            upkeep_rate: ONE_UNIT / 4,
            capacity: 512 * ONE_UNIT,
        });
    }
    field.ports = ports;

    let mut routes = Vec::new();
    for id in 1..=512u32 {
        let tail = 9 + (id - 1) % 248;
        let head = 9 + id % 248;
        routes.push(RouteState {
            route: id,
            tail,
            head,
            capacity: 8 * ONE_UNIT,
            flow: 0,
            formed_step: 0,
        });
    }
    field.routes = routes;

    field.forms = (1..=8u8)
        .map(|id| FormState {
            id,
            form: field_game_core::run::FORMS[usize::from(id) - 1].to_string(),
            node: u32::from(id),
            controlled: id == 1,
            layer: id - 1,
            pos: Vec2::units(1000 + i64::from(id) * 7, 1000 + i64::from(id) * 11),
            vel: Vec2 { x: 0, y: 0 },
            charge: 16 * ONE_UNIT,
            reserve: 0,
            pulse_charge: 0,
            focus: false,
            route_reach: 256 * ONE_UNIT,
            forecast_depth: 0,
            steer_scale: field_game_core::state::FRAC_ONE,
            route_capacity: 32 * ONE_UNIT,
            link: None,
            trail: Some(field::TrailState {
                period: *field::TRAIL_PERIOD.start(),
                delay: *field::TRAIL_DELAY.start(),
                radius: field::TRAIL_RADIUS_CAP,
                magnitude: field::TRAIL_MAGNITUDE_CAP,
            }),
            junction: None,
        })
        .collect();

    // 32 currents, 4 per layer, each with the full 64 path points.
    let mut currents = Vec::new();
    for id in 0..32u16 {
        let layer = (id % 8) as u8;
        let path: Vec<Vec2> = (0..64)
            .map(|point| {
                Vec2::units(150 + i64::from(point) * 60, 150 + i64::from(id) * 90 % 3500)
            })
            .collect();
        currents.push(CurrentState {
            id: id + 1,
            layer,
            path,
            width: 300 * ONE_UNIT,
            strength: 16 * ONE_UNIT,
            duty: 65_536,
            period: 30,
            phase: 0,
            bright: true,
            active: true,
        });
    }
    field.currents = currents;

    field.layers = (0..8u8)
        .map(|layer| FieldLayer {
            layer,
            drain: ONE_UNIT / 4,
            noise: 0,
            gain: 32_768,
            current_ids: field
                .currents
                .iter()
                .filter(|current| current.layer == layer)
                .map(|current| current.id)
                .collect(),
            port_ids: field
                .ports
                .iter()
                .filter(|port| port.layer == layer && port.kind != NodeKind::Form)
                .map(|port| port.node)
                .collect(),
        })
        .collect();

    // The material compartment, rather than the observational View, owns the
    // widest 128-member leakage pass this budget fixture measures.
    field.physical_compartment = PhysicalCompartment {
        members: (9..=136u32).collect(),
        leak_per_exposed_contact_per_step: 4096,
    };
    field.boundaries = BoundaryState { drawn: Vec::new(), authored: Vec::new() };
    // The Trail queue standing at its cap, with an entry falling due on every
    // step of the measured window: the widest delivery pass a step can ask for.
    field.pending = (0..field::PENDING_TRAILS)
        .map(|index| field::PendingTrail {
            form: 1,
            layer: (index % 8) as u8,
            pos: Vec2::units(600 + (index as i64 % 16) * 180, 600 + (index as i64 / 16) * 180),
            due: 1 + (index as u32 % 60),
            magnitude: field::TRAIL_MAGNITUDE_CAP,
        })
        .collect();
    field
}

/// A schedule that moves everything the prepared pass reads again: the
/// controlled Form is steered around the plane and pushed between layers, so
/// Form-moved Nodes enter and leave currents, adjacency, and the standing
/// inside's exposure over the run of steps.
fn schedule(step: usize) -> ControlState {
    let turn = (step % 8) as i16;
    ControlState {
        steer_x: (turn - 4) * 8000,
        steer_y: (4 - turn) * 8000,
        pulse_held: step % 5 == 0,
        pulse_release: step % 5 == 4,
        depth_move: match step % 20 {
            3 => 1,
            13 => -1,
            _ => 0,
        },
    }
}

#[test]
fn the_prepared_pass_and_the_plain_pass_leave_the_same_bytes() {
    // The optimization the Still Mode analysis budget rests on is exact, and
    // this is what says so: at the caps, over a full window, under a schedule
    // that moves a Form through every relation the cache prepares, the two
    // passes leave the same Field and record the same step — byte for byte,
    // through the canonical writer, which is the only equality that matters.
    let field = caps_field();
    field::validate(&field).expect("the caps fixture is a valid Field");
    let inside: Vec<u32> = (9..=136u32).collect();

    let mut plain = field.clone();
    let mut prepared = field.clone();
    let cache = field::StepCache::of(&prepared);
    for step in 1..=60usize {
        let control = schedule(step);
        let first = field::advance(
            &mut plain,
            control,
            65_536,
            &mut field::Unstaged::default().staging(),
        );
        let second = field::advance_cached(
            &mut prepared,
            control,
            65_536,
            &mut field::Unstaged::default().staging(),
            &cache,
        );
        assert_eq!(plain.written(), prepared.written(), "the Field, at step {step}");
        assert_eq!(
            recorded(step, control, first),
            recorded(step, control, second),
            "the records, at step {step}",
        );
    }
}

/// One step's records as the trajectory writes them, which is the canonical
/// form the equality is taken over.
fn recorded(step: usize, control: ControlState, outcome: field::StepOutcome) -> String {
    TraceStep {
        step: step as u32,
        rng: RngState { key: 0, ctr: 0, half: 0 },
        ctl: control,
        records: outcome.records,
    }
    .written()
}

fn time_slate(label: &str, field: &FieldState, _inside: &[u32]) {
    for cached in [false, true] {
        let replays = 160;
        let steps = 60;
        let start = std::time::Instant::now();
        let mut sink: i64 = 0;
        // One preparation for the whole slate: the shape does not change
        // across the samples that share it, which is how the evaluation uses
        // it.
        let cache = field::StepCache::of(field);
        for _ in 0..replays {
            let mut state = field.clone();
            for _ in 0..steps {
                let outcome = if cached {
                    field::advance_cached(
                        &mut state,
                        ControlState::default(),
                        65_536,
                        &mut field::Unstaged::default().staging(),
                        &cache,
                    )
                } else {
                    field::advance(
                        &mut state,
                        ControlState::default(),
                        65_536,
                        &mut field::Unstaged::default().staging(),
                    )
                };
                sink = sink.wrapping_add(outcome.ledger.closing);
            }
        }
        let elapsed = start.elapsed();
        println!(
            "{label:26} {:8} {:>10.2?} total  {:>9.3?} per step  [sink {sink}]",
            if cached { "cached" } else { "uncached" },
            elapsed,
            elapsed / (replays * steps) as u32,
        );
    }
}

/// The whole job the budget is stated over: one caps-scale run, a slate of
/// five candidates over wide insides, assembled and evaluated end to end.
#[test]
#[ignore = "measurement, not an assertion: run in release with --nocapture"]
fn whole_slate_job() {
    for window in [60u16, 45] {
        whole_slate_job_at(window);
    }
}

fn whole_slate_job_at(window: u16) {
    use field_game_core::state::{
        InputConfig, Progress, RunState, Surround, Trace, TraceStep, ViewDeclaration,
    };

    let keyframe = caps_field();
    let inside: Vec<u32> = (9..=136u32).collect();
    let mut now = keyframe.clone();
    let mut trace = Trace::opening(keyframe);
    // A full 60-step window, played under the moving schedule.
    for step in 1..=60usize {
        let control = schedule(step);
        let outcome = field::advance(
            &mut now,
            control,
            65_536,
            &mut field::Unstaged::default().staging(),
        );
        trace.steps.push_back(TraceStep {
            step: now.step,
            rng: RngState { key: 0, ctr: 0, half: 0 },
            ctl: control,
            records: outcome.records,
        });
    }
    // Four wide authored boundaries, so the slate fills its five seats with
    // insides at the scale the caps allow.
    now.boundaries.authored = vec![
        (9..=200u32).collect(),
        (20..=180u32).collect(),
        (9..=120u32).collect(),
        (30..=250u32).collect(),
    ];
    let state = RunState {
        run_id: "0123456789abcdef".to_string(),
        rng: field_game_core::rng::trajectory_stream("0123456789abcdef", 0),
        scenario: field_game_core::state::ScenarioSpec::legacy("00".repeat(32)),
        criterion: None,
        branch_nonce: 0,
        progress: Progress::opening(),
        now,
        trace,
        view: ViewDeclaration { inside, resolution: 1, window, surround: Surround::Adjacent },
        slate: None,
        input_config: InputConfig::default_config(),
        pressures: Vec::new(),
        anchors: Vec::new(),
    };

    let start = std::time::Instant::now();
    let mut slate = field_game_core::slate::assemble(&state);
    let assembled = start.elapsed();
    field_game_core::rank::evaluate(&state, &mut slate);
    let evaluated = start.elapsed();
    let written = slate.written();
    let recorded = start.elapsed();
    println!(
        "whole slate job: {} candidates, window {} — assembly {:?}, evaluation {:?}, record {:?}, total {:?} ({} bytes)",
        slate.candidates.len(),
        slate.window_effective,
        assembled,
        evaluated - assembled,
        recorded - evaluated,
        recorded,
        written.len(),
    );
}

/// The on-demand jobs the budget states beside the slate: one coordinate
/// profile, the whole profile with its two replay-based coordinates, and each
/// of the eight perturbation kinds, all at the caps and a full 60-step window.
///
/// ARCHITECTURE.md's own figures are the ones to read these against: the worst
/// perturbation is a window change at `w = 60` — 24 replays over 30 and 120
/// steps, 3,600 replayed steps, about 21.3 ms — the two replay-based
/// coordinates together are 16 replays over 60 steps, about 5.7 ms, and the
/// post-commit Echo perturbation is 8 replays, about 2.8 ms.
#[test]
#[ignore = "measurement, not an assertion: run in release with --nocapture"]
fn on_demand_jobs() {
    use field_game_core::state::{
        InputConfig, Progress, RunState, Surround, Trace, TraceStep, ViewDeclaration,
    };

    let keyframe = caps_field();
    let inside: Vec<u32> = (9..=136u32).collect();
    let mut now = keyframe.clone();
    let mut trace = Trace::opening(keyframe);
    // 120 steps, so a window change at `w = 60` finds the doubled window too —
    // which is the worst on-demand job the budget names.
    for step in 1..=120usize {
        let control = schedule(step);
        let outcome = field::advance(
            &mut now,
            control,
            65_536,
            &mut field::Unstaged::default().staging(),
        );
        trace.steps.push_back(TraceStep {
            step: now.step,
            rng: RngState { key: 0, ctr: 0, half: 0 },
            ctl: control,
            records: outcome.records,
        });
    }
    let view = ViewDeclaration {
        inside: inside.clone(),
        resolution: 1,
        window: 60,
        surround: Surround::Adjacent,
    };
    let state = RunState {
        run_id: "0123456789abcdef".to_string(),
        rng: field_game_core::rng::trajectory_stream("0123456789abcdef", 0),
        scenario: field_game_core::state::ScenarioSpec::legacy("00".repeat(32)),
        criterion: None,
        branch_nonce: 0,
        progress: Progress::opening(),
        now,
        trace,
        view: view.clone(),
        slate: None,
        input_config: InputConfig::default_config(),
        pressures: Vec::new(),
        anchors: Vec::new(),
    };
    let sigma = field_game_core::slate::evaluation_stream("0123456789abcdef", 0, 1);
    let tau = field_game_core::slate::TAU_DEFAULT;

    let start = std::time::Instant::now();
    let eight = field_game_core::coord::of(&state, &view, tau);
    let recorded_only = start.elapsed();
    let whole = field_game_core::coord::full(&state, &view, &sigma, 0, tau);
    let with_replays = start.elapsed();
    println!(
        "coordinates (eight)       {recorded_only:>12.3?}  ({} bytes)",
        eight.written().len(),
    );
    println!(
        "coordinates_full (ten)    {:>12.3?}  ({} bytes)",
        with_replays - recorded_only,
        whole.written().len(),
    );

    for kind in field_game_core::perturb::KINDS {
        let start = std::time::Instant::now();
        let result = field_game_core::perturb::run(
            &state,
            &view,
            &[],
            0,
            &sigma,
            tau,
            field_game_core::perturb::Request::of(kind, None).expect("a kind"),
        );
        let elapsed = start.elapsed();
        println!(
            "{kind:25} {elapsed:>12.3?}  ({} bytes, reading {:?})",
            result.written().len(),
            result.reading.value,
        );
    }
}

#[test]
#[ignore = "measurement, not an assertion: run in release with --nocapture"]
fn per_rule_breakdown() {
    let full = caps_field();
    field::validate(&full).expect("the caps fixture is a valid Field");
    let inside: Vec<u32> = (9..=136u32).collect();

    time_slate("everything", &full, &inside);

    let mut without_currents = full.clone();
    without_currents.currents.clear();
    for layer in &mut without_currents.layers {
        layer.current_ids.clear();
    }
    time_slate("no currents", &without_currents, &inside);

    let mut without_leak = full.clone();
    without_leak.physical_compartment.leak_per_exposed_contact_per_step = 0;
    time_slate("no leakage", &without_leak, &inside);

    let mut without_routes = full.clone();
    without_routes.routes.clear();
    time_slate("no routes", &without_routes, &inside);

    let mut bare = without_currents.clone();
    bare.physical_compartment.leak_per_exposed_contact_per_step = 0;
    bare.routes.clear();
    time_slate("no currents/leak/routes", &bare, &inside);
}

#[test]
#[ignore = "measurement, not an assertion: run in release with --nocapture"]
fn worst_case_slate_timing() {
    let field = caps_field();
    field::validate(&field).expect("the caps fixture is a valid Field");
    // 128 members: the widest leakage pass a 256-Node Field can ask for.
    let inside: Vec<u32> = (9..=136u32).collect();

    let replays = 160;
    let steps = 60;
    let start = std::time::Instant::now();
    let mut sink: i64 = 0;
    for _ in 0..replays {
        let mut state = field.clone();
        for _ in 0..steps {
            let outcome = field::advance(
                &mut state,
                ControlState::default(),
                65_536,
                &mut field::Unstaged::default().staging(),
            );
            sink = sink.wrapping_add(outcome.ledger.closing);
        }
    }
    let elapsed = start.elapsed();
    println!(
        "worst-case slate: {replays} replays x {steps} steps = {} replayed steps in {:?} ({:?} per step) [sink {sink}]",
        replays * steps,
        elapsed,
        elapsed / (replays * steps) as u32,
    );
}

#[test]
#[ignore = "measurement, not an assertion: run in release with --nocapture"]
fn pressure_effect_rows() {
    // The three effect rows of the budget table, measured as deltas over the
    // caps fixture: the stage machine's walk over a full list, the Noise flow
    // scale's eight draws, and the Flood and Interference riders on the Route
    // and current phases.
    use field_game_core::pressure::{
        Pressure, PressureContent, PressureState, Schedule, Stage, StageRow, Target, TargetKind,
    };

    let full = caps_field();
    field::validate(&full).expect("the caps fixture is a valid Field");
    let inside: Vec<u32> = (9..=136u32).collect();

    let table = |pressure: Pressure, target: TargetKind| PressureContent {
        pressure,
        target,
        stages: [
            StageRow { stage: Stage::Signal, level: 30_000, steps: 36_000 },
            StageRow { stage: Stage::Pressure, level: 30_000, steps: 36_000 },
            StageRow { stage: Stage::Crisis, level: 30_000, steps: 36_000 },
            StageRow { stage: Stage::Resolution, level: 30_000, steps: 36_000 },
        ],
    };
    let seat = |pressure: Pressure, queued: bool, target: Target| PressureState {
        pressure,
        stage: Stage::Signal,
        level: 30_000,
        primary: false,
        queued,
        start_step: 100_000,
        target,
        displaced: None,
        bound: None,
    };

    // The prepared pass, which is the pass the budget table measures: one
    // StepCache per shape, read by every replayed step.
    let time = |label: &str, field: &FieldState, staged: &mut field::Unstaged| {
        let replays = 160;
        let steps = 60;
        let cache = field::StepCache::of(field);
        let start = std::time::Instant::now();
        let mut sink: i64 = 0;
        for _ in 0..replays {
            let mut state = field.clone();
            let mut scratch = field::Unstaged {
                pressures: staged.pressures.clone(),
                schedule: staged.schedule.clone(),
                stream: staged.stream,
            };
            for _ in 0..steps {
                let outcome = field::advance_cached(
                    &mut state,
                    ControlState::default(),
                    65_536,
                    &mut scratch.staging(),
                    &cache,
                );
                sink = sink.wrapping_add(outcome.ledger.closing);
            }
        }
        let spent = start.elapsed();
        println!(
            "{label}: {:.2} us per step (sink {sink})",
            spent.as_secs_f64() * 1e6 / f64::from(replays * steps),
        );
    };

    // The reference: no list, no draws.
    time("bare", &full, &mut field::Unstaged::default());

    // The stage machine over a full list: six seats, two active, no draws and
    // no effect aimed at anything.
    let mut listed = field::Unstaged {
        pressures: vec![
            seat(Pressure::Drain, false, Target { kind: TargetKind::Layer, id: Some(7) }),
            seat(Pressure::Noise, true, Target { kind: TargetKind::Layer, id: Some(7) }),
            seat(Pressure::Fracture, true, Target::none()),
            seat(Pressure::Flood, false, Target { kind: TargetKind::Node, id: Some(9) }),
            seat(Pressure::Interference, true, Target { kind: TargetKind::Node, id: Some(9) }),
            seat(Pressure::Drift, true, Target { kind: TargetKind::Layer, id: Some(7) }),
        ],
        schedule: Schedule::of(vec![
            table(Pressure::Drain, TargetKind::Layer),
            table(Pressure::Noise, TargetKind::Layer),
            table(Pressure::Fracture, TargetKind::None),
            table(Pressure::Flood, TargetKind::Node),
            table(Pressure::Interference, TargetKind::Node),
            table(Pressure::Drift, TargetKind::Layer),
        ])
        .expect("one table per pressure"),
        stream: Default::default(),
    };
    time("stage machine, full list", &full, &mut listed);

    // The Noise flow scale: every layer noisy, eight draws per step, every
    // Route's capacity scaled.
    let mut noisy_field = full.clone();
    for layer in &mut noisy_field.layers {
        layer.noise = 8_192;
    }
    time("noise scale, 8 layers drawing", &noisy_field, &mut field::Unstaged::default());

    // The Flood and Interference riders, active over the full Route and
    // current phases.
    let mut riding = field::Unstaged {
        pressures: vec![
            seat(Pressure::Flood, false, Target { kind: TargetKind::Node, id: Some(9) }),
            seat(
                Pressure::Interference,
                false,
                Target { kind: TargetKind::Node, id: Some(9) },
            ),
        ],
        schedule: Schedule::of(vec![
            table(Pressure::Flood, TargetKind::Node),
            table(Pressure::Interference, TargetKind::Node),
        ])
        .expect("one table per pressure"),
        stream: Default::default(),
    };
    time("flood and interference riders", &full, &mut riding);
}

#[test]
#[ignore = "measurement, not an assertion: run in release with --nocapture"]
fn trail_entry_rows() {
    // The Trail row of the budget table, measured as a delta over the caps
    // fixture: the queue walk every step, and the pass a due entry makes over
    // the Node list for its recipients. The worst case is the queue standing at
    // its cap with an entry coming due every step, which is what the second
    // measurement below stands up.
    let bare = caps_field();
    field::validate(&bare).expect("the caps fixture is a valid Field");
    let inside: Vec<u32> = (9..=136u32).collect();

    let time = |label: &str, field: &FieldState| {
        let replays = 160;
        let steps = 60;
        let cache = field::StepCache::of(field);
        let start = std::time::Instant::now();
        let mut sink: i64 = 0;
        for _ in 0..replays {
            let mut state = field.clone();
            for _ in 0..steps {
                let outcome = field::advance_cached(
                    &mut state,
                    ControlState::default(),
                    65_536,
                    &mut field::Unstaged::default().staging(),
                    &cache,
                );
                sink = sink.wrapping_add(outcome.ledger.closing);
            }
        }
        let spent = start.elapsed();
        println!(
            "{label}: {:.2} us per step (sink {sink})",
            spent.as_secs_f64() * 1e6 / f64::from(replays * steps),
        );
    };

    // One pass to warm the caches, so the first measurement is not the one
    // that pays for them.
    time("warm-up", &bare);
    time("bare", &bare);

    // Every Form leaving a Trail on the tightest period its bounds admit, the
    // queue standing at its cap, and an entry coming due on every step of the
    // measured window: the walk and the recipient pass together.
    let mut standing = bare.clone();
    for form in &mut standing.forms {
        form.trail = Some(field::TrailState {
            period: *field::TRAIL_PERIOD.start(),
            delay: *field::TRAIL_DELAY.start(),
            radius: field::TRAIL_RADIUS_CAP,
            magnitude: field::TRAIL_MAGNITUDE_CAP,
        });
    }
    standing.pending = (0..field::PENDING_TRAILS)
        .map(|index| field::PendingTrail {
            form: 1,
            layer: 0,
            pos: standing.forms[0].pos,
            due: standing.step + 1 + (index as u32 % 60),
            magnitude: field::TRAIL_MAGNITUDE_CAP,
        })
        .collect();
    field::validate(&standing).expect("a Field with a full queue is a valid Field");
    time("trail entries: queue at cap, one due a step", &standing);

    // The two rows the first line of the table absorbed: every Node paying its
    // authored upkeep, and every Form but one following a station.
    let mut paying = bare.clone();
    for port in &mut paying.ports {
        port.upkeep_rate = ONE_UNIT / 4;
    }
    field::validate(&paying).expect("a Field that pays is a valid Field");
    time("upkeep: every Node paying", &paying);

    let mut following = bare.clone();
    for (place, form) in following.forms.iter_mut().enumerate() {
        if place == 0 {
            continue;
        }
        form.controlled = false;
        form.link = Some(field::LinkState {
            offset: Vec2::units(place as i64 * 8, place as i64 * 8),
            separation: 64 * ONE_UNIT,
        });
    }
    field::validate(&following).expect("a Field that follows is a valid Field");
    time("linked following: seven Forms on stations", &following);
}
