//! The Edge, driven end to end: the roughly thirty-five-minute second chapter.
//!
//! The chapter's subject is the physical compartment — where its members end, what escapes
//! across it, and what redrawing it is worth. Its final challenge is to hold a
//! standing inside through three escalating breaks in the current that supplies
//! it, each longer and deeper than the one before.
//!
//! What this file proves:
//!
//! - the chapter completes, from the first transition to the second;
//! - every one of the eight starting Forms completes the whole of it, under one
//!   script that is not re-tuned per Form;
//! - the three escalating pulses land where they were authored, in every run,
//!   and each is longer and deeper than the one before it;
//! - **three distinct strategies complete the chapter**, which is the goal's own
//!   done-when: a reshaped compartment, a rerouted supply, and stored Charge. Each
//!   is a different set of player acts, each completes, and each leaves the run
//!   measurably different from the others and from the baseline;
//! - the optional test is passable and skippable;
//! - the chapter never stands in a state it cannot leave.
//!
//! Every run here starts at step 0 of the campaign and plays the opening chapter
//! first, through `support::campaign`'s own script, because this chapter is the
//! second: the run that enters it carries the Impulse, the completed list and
//! the step counter the first chapter left, and a run assembled any other way
//! would be answering a question about a state no player reaches.
//!
//! The **allowances** are the modelled first-run search, named one by one in the
//! phase list. The floor run spends none of them; the attentive run spends all
//! of them; the difference is the whole of the pacing model and is printed.

use field_game_core::content::{self, CONTENT_VERSION};
use field_game_core::fx::ONE_UNIT;
use field_game_core::state::{Progress, Step, STEPS_PER_SECOND};
use field_game_core::Session;

mod support;

use support::campaign::{nearest_point, toward};

const KEY: &str = "00112233445566aa";

/// How many steps one scripted batch runs before the script re-aims.
const BATCH: u16 = 15;

/// How many steps a full Pulse charge takes, from the locked charging rule.
const FULL_CHARGE_STEPS: u32 = 32;

/// One rendered frame at the 60-frames-per-second target, in microseconds: what
/// a Still Mode ramp is measured in.
const RAMP_US: i64 = 250_000;

/// The chapter this file is about, by its place in the manifest.
const CHAPTER: u8 = 1;

/// What one phase of the script asks of the run.
#[derive(Clone, Copy, Debug)]
enum Act {
    /// Steer toward a point in whole units.
    Toward(i64, i64),
    /// Steer toward the nearest path point of a named current, read off the
    /// Field each batch.
    Follow(u16),
    /// Hold every input neutral.
    Rest,
    /// Hold the Pulse, steering toward a point.
    Charge(i64, i64),
    /// Release the Pulse.
    Release,
    /// Ask for one layer deeper, or one layer back up.
    Depth(i8),
    /// Enter Still Mode, queue the named plan bodies, commit, and leave. The
    /// run stands fully paused throughout, so this costs no step at all — which
    /// is what makes the strategy layer free of the pacing model.
    Still(&'static [&'static str]),
}

/// One phase of the script: what it does, how long it runs, and whether its
/// length is authored content or a modelled first-run allowance.
struct Phase {
    label: &'static str,
    act: Act,
    steps: u32,
    allowance: bool,
}

const fn play(label: &'static str, act: Act, steps: u32) -> Phase {
    Phase { label, act, steps, allowance: false }
}

const fn allow(label: &'static str, act: Act, steps: u32) -> Phase {
    Phase { label, act, steps, allowance: true }
}

/// Which strategy a driven run plays. Every route through the chapter performs
/// the one physical-compartment edit the dedicated objective requires. The
/// baseline makes the smallest handle-reachable swap; the three after it are
/// the chapter's authored answers to its own final challenge, and the goal's
/// done-when is that each of them completes it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Strategy {
    /// The minimum required compartment swap, and the optional test let go.
    Baseline,
    /// The physical compartment expanded around the whole ring, so its
    /// members keep what would otherwise escape across the material edge.
    ExpandedCompartment,
    /// The supply Fracture cuts, formed again in Still Mode.
    RapidReroute,
    /// The deep store taken, and its Charge carried into the inside.
    StoredCharge,
}

impl Strategy {
    fn label(self) -> &'static str {
        match self {
            Strategy::Baseline => "baseline",
            Strategy::ExpandedCompartment => "expanded compartment",
            Strategy::RapidReroute => "rapid reroute",
            Strategy::StoredCharge => "stored charge",
        }
    }

    /// Whether the run takes the chapter's optional test.
    fn takes_the_test(self) -> bool {
        self == Strategy::StoredCharge
    }
}

/// The scripted run through The Edge, phase by phase.
///
/// The positions are the authored content's own, in whole units: the bright
/// current's waypoints, the three Ports a Pulse opens, and the deep current the
/// back half asks for.
fn script(strategy: Strategy) -> Vec<Phase> {
    let mut phases = vec![
        // 1. The bright current, and the inside standing in it.
        allow("read the surface", Act::Rest, 240),
        play("cross to the bright current", Act::Toward(2000, 2000), 700),
        play("hold the bright current", Act::Follow(1), 4_300),
        // 2. The ring closes on its own once the third Node opens, which the
        //    chapter's own event does forty steps in.
        play("stand off the band", Act::Toward(1620, 2000), 260),
        // 3. The two outer stores.
        allow("look for what stands outside", Act::Toward(2060, 1700), 1_800),
        play("cross to the north store", Act::Toward(2060, 1620), 700),
        play("charge", Act::Charge(2060, 1620), FULL_CHARGE_STEPS),
        play("release", Act::Release, 1),
        play("cross to the east store", Act::Toward(2470, 2000), 900),
        play("charge", Act::Charge(2470, 2000), FULL_CHARGE_STEPS),
        play("release", Act::Release, 1),
        // 4. Hold the inside, standing clear of the band that fills it.
        allow("look for where it goes", Act::Toward(1900, 2200), 1_500),
        play("stand clear and hold", Act::Toward(2500, 2200), 8_400),
    ];
    // 6. Reshape the physical compartment: the dedicated tutorial edit.
    if strategy == Strategy::ExpandedCompartment {
        phases.push(play(
            "draw the edge around the whole of it",
            Act::Still(&["{\"members\":[2,3,4,5,6,7],\"op\":\"reshape_compartment\"}"]),
            0,
        ));
    } else {
        // Dragging the handle around Node 3 onto Node 4 leaves this exact set.
        // It is the smallest physical edit the direct manipulation can make.
        phases.push(play(
            "move one compartment handle",
            Act::Still(&["{\"members\":[2,4],\"op\":\"reshape_compartment\"}"]),
            0,
        ));
    }
    phases.push(allow("read what the edge costs", Act::Rest, 2_400));
    phases.push(play("hold the pattern", Act::Toward(1620, 2200), 11_200));
    if strategy == Strategy::RapidReroute {
        // The supply Fracture took, formed again: the pair stands 414 units
        // apart, which is inside this Form's own reach.
        phases.push(play("cross to the cut supply", Act::Toward(2060, 1700), 900));
        phases.push(play(
            "form the supply again",
            Act::Still(&["{\"from\":7,\"op\":\"connect\",\"to\":2}"]),
            0,
        ));
    }
    phases.extend([
        // 7. Depth.
        allow("look for what lies deeper", Act::Rest, 1_200),
        play("cross to above the deep current", Act::Toward(2080, 2380), 1_200),
        play("take the depth", Act::Depth(1), 1),
        play("hold the deep current", Act::Follow(3), 4_300),
    ]);
    // 8. The optional test, taken or let go.
    if strategy.takes_the_test() {
        phases.extend([
            allow("look for what stands down here", Act::Toward(2060, 2400), 300),
            play("cross to the deep store", Act::Toward(2060, 2400), 500),
            play("charge", Act::Charge(2060, 2400), FULL_CHARGE_STEPS),
            play("release", Act::Release, 1),
        ]);
    } else {
        phases.push(play("let the test go", Act::Follow(3), 5_600));
    }
    phases.extend([
        play("back to the surface", Act::Depth(-1), 1),
        // 9. The final challenge: three breaks in the current that supplies it.
        allow("read the field before it breaks", Act::Rest, 1_800),
        play("stand clear and hold the edge", Act::Toward(1620, 2200), 34_000),
    ]);
    phases
}

/// One milestone the run reached, and the step it reached it at.
#[derive(Clone, Debug)]
struct Milestone {
    name: String,
    step: Step,
}

/// What one driven run recorded, from the step it entered The Edge.
struct Driven {
    strategy: Strategy,
    milestones: Vec<Milestone>,
    completed: Vec<String>,
    stages: Vec<String>,
    offered: Vec<(String, Step)>,
    /// The step the run crossed into The Edge, and the step it left it.
    opened_step: Step,
    closed_step: Option<Step>,
    collapse_step: Option<Step>,
    recovery_step: Option<Step>,
    /// Every step at which the bright current stopped or stood again, with what
    /// it turned into.
    breaks: Vec<(Step, bool)>,
    /// Every step at which layer 0's loss changed, with what it became in
    /// sixteenths of a unit.
    losses: Vec<(Step, i64)>,
    /// The step the Fracture took the east supply away, if it did.
    fracture_step: Option<Step>,
    /// The lowest total the three ring Nodes held while the final challenge
    /// stood, in whole units, and the highest.
    edge_low: i64,
    edge_high: i64,
    /// How many **batches** of the final challenge the run was read at with a
    /// Route of the inside carrying nothing.
    ///
    /// The script drives in fifteen-step batches and reads the Field once at
    /// the end of each, so this is a count of readings and not a count of
    /// steps: at fifteen steps a batch it is at most a fifteenth of the steps
    /// the inside actually stood empty for. It is a comparison between runs
    /// driven by the same script and nothing else.
    edge_stalls: u32,
    /// Whether the deep store stood open when the chapter closed.
    deep_open: bool,
    /// What the run held as the chapter closed, in whole units.
    inside_units: i64,
    ring_units: i64,
    /// The standing View's inside as the chapter closed.
    inside: Vec<u32>,
    /// The causal physical-compartment membership as the chapter closed.
    compartment: Vec<u32>,
    /// What the standing slate carried at each Still Mode visit: the objective
    /// the line stood on, how many candidates the slate held, whether it was
    /// deficient, and the reason it named when it was.
    ///
    /// A deficient slate is one with no alternative to compare the standing
    /// View against. It is not a cosmetic state: the candidate walk the
    /// paused surface offers — the arrow keys, which propose a candidate and
    /// commit it with Enter — reads the slate and does **nothing at all** when
    /// it is deficient. A chapter that teaches reshaping at a moment when the
    /// slate is deficient is teaching an input path that is not there.
    slates: Vec<(String, usize, bool, Option<String>)>,
    /// The Impulse the run carried as the chapter closed, and what it spent.
    impulse: u8,
    spent: u8,
    /// How many Routes stood as the chapter closed.
    routes: usize,
    forms: usize,
}

impl Driven {
    fn at(&self, name: &str) -> Option<Step> {
        self.milestones.iter().find(|held| held.name == name).map(|held| held.step)
    }

    fn offered_at(&self, id: &str) -> Option<Step> {
        self.offered.iter().find(|(held, _)| held == id).map(|(_, step)| *step)
    }

    /// How long the chapter took, in steps.
    fn span(&self) -> Step {
        self.closed_step.expect("the chapter closed") - self.opened_step
    }
}

/// Writes what one batch of events said into the record.
fn record(driven: &mut Driven, events: Vec<(String, u32, field_game_core::json::Json)>) {
    for (name, step, body) in events {
        match name.as_str() {
            "objective_changed" => {
                let objective = body.get("objective").expect("an objective");
                let id = objective.get("id").and_then(|held| held.as_text()).unwrap_or("");
                if !id.starts_with("objective.the_edge.") && !id.is_empty() {
                    continue;
                }
                let stage = objective.get("state").and_then(|held| held.as_text()).unwrap_or("");
                driven.stages.push(stage.to_string());
                if stage == "active" && !driven.offered.iter().any(|(held, _)| held == id) {
                    driven.offered.push((id.to_string(), step));
                }
                if stage == "complete" {
                    driven.milestones.push(Milestone { name: id.to_string(), step });
                    driven.completed.push(id.to_string());
                }
                if stage == "failed_recoverable" {
                    driven.collapse_step.get_or_insert(step);
                }
                if stage == "active" && driven.collapse_step.is_some() {
                    driven.recovery_step.get_or_insert(step);
                }
            }
            "chapter_changed" => {
                match body.get("chapter_index").and_then(|held| held.as_int()) {
                    Some(index) if index == i64::from(CHAPTER) => {
                        driven.opened_step = step;
                        driven.milestones.push(Milestone { name: "opened".to_string(), step });
                    }
                    Some(index) if index == i64::from(CHAPTER) + 1 => {
                        driven.closed_step = Some(step);
                        driven.milestones.push(Milestone { name: "transition".to_string(), step });
                    }
                    _ => {}
                }
            }
            "checkpoint_written" => {
                driven.milestones.push(Milestone { name: "anchor".to_string(), step });
            }
            _ => {}
        }
    }
}

/// The whole of one driven run: the opening chapter played through the shared
/// script, then The Edge played through this file's own.
fn drive_as(form: &str, allowances: bool, strategy: Strategy) -> Driven {
    let mut session = Session::new(&support::worker_init()).expect("versions agree");
    let opened = session.command(
        "init_run",
        &format!("{{\"form\":\"{form}\",\"mode\":\"new\",\"run_id\":\"{KEY}\"}}"),
    );
    assert!(opened.contains("\"ok\":true"), "{opened}");

    let mut driven = Driven {
        strategy,
        milestones: Vec::new(),
        completed: Vec::new(),
        stages: Vec::new(),
        offered: Vec::new(),
        opened_step: 0,
        closed_step: None,
        collapse_step: None,
        recovery_step: None,
        breaks: Vec::new(),
        losses: Vec::new(),
        fracture_step: None,
        edge_low: i64::MAX,
        edge_high: 0,
        edge_stalls: 0,
        deep_open: false,
        inside_units: 0,
        ring_units: 0,
        inside: Vec::new(),
        compartment: Vec::new(),
        slates: Vec::new(),
        impulse: 0,
        spent: 0,
        routes: 0,
        forms: 0,
    };

    // The opening chapter, played by the shared script and no further: the run
    // that enters The Edge is the run a player brings to it.
    let mut driver = support::campaign::Driver::new();
    driver.drain(&mut session);
    support::campaign::play_chapter(
        &mut session,
        &mut driver,
        authored().chapter(0).expect("The Pull"),
    );
    for event in driver.raised.drain(..) {
        record(&mut driven, vec![event]);
    }
    assert_eq!(
        support::campaign::chapter_index(&session),
        CHAPTER,
        "{form}: the opening chapter completed and the run carried into The Edge",
    );

    let mut seq = driver.seq;
    let mut t_us = 1_000_000i64;
    let mut bright = true;
    let mut loss = layer_drain(&session, 0);
    let mut routes = route_ids(&session);
    let opening_impulse = impulse(&session);
    let mut spent = 0u8;

    for phase in script(strategy) {
        if phase.allowance && !allowances {
            continue;
        }
        if let Act::Still(plans) = phase.act {
            // Still Mode: two frames of real time to ramp in, the queue, the
            // commit, and two more to ramp out. No step runs while it stands.
            t_us += 1_000_000;
            session.command("input_frame", &still_frame(seq, t_us, true));
            seq += 1;
            t_us += RAMP_US;
            session.command("input_frame", &still_frame(seq, t_us, false));
            seq += 1;
            assert_eq!(session.lifecycle(), "still", "Still Mode opens");
            // The standing slate, read where it stands: entering Still Mode
            // assembles it and evaluates it, and this is the only place it
            // exists.
            {
                let state = session.run().expect("a run").state();
                let standing = state.progress.objective.id.clone();
                if let Some(slate) = &state.slate {
                    driven.slates.push((
                        standing,
                        slate.candidates.len(),
                        slate.deficient,
                        slate.deficiency_reason.clone(),
                    ));
                }
            }
            for plan in plans {
                let answer = session.command("queue_plan", &format!("{{\"plan\":{plan}}}"));
                assert!(answer.contains("\"ok\":true"), "{}: {answer}", strategy.label());
                spent += 1;
            }
            // Enter commits, and the commit is also the exit: the run is
            // already ramping out, so what is left is to let the ramp finish.
            let answer = session.command("commit_plan", "{}");
            assert!(answer.contains("\"ok\":true"), "{}: {answer}", strategy.label());
            t_us += RAMP_US;
            session.command("input_frame", &still_frame(seq, t_us, false));
            seq += 1;
            assert_eq!(session.lifecycle(), "running", "and the commit is the exit");
            record(&mut driven, support::campaign::raised(&mut session));
            continue;
        }

        let mut left = phase.steps;
        while left > 0 {
            if driven.closed_step.is_some() {
                break;
            }
            let at = support::campaign::controlled(&session);
            let (steer, held, release, depth) = match phase.act {
                Act::Toward(x, y) => (toward(at, x, y), false, false, 0),
                Act::Follow(current) => {
                    let (x, y) = nearest_point(&session, current).unwrap_or((0, 0));
                    (toward(at, x, y), false, false, 0)
                }
                Act::Rest => ((0, 0), false, false, 0),
                Act::Charge(x, y) => (toward(at, x, y), true, false, 0),
                Act::Release => ((0, 0), false, true, 0),
                Act::Depth(way) => ((0, 0), false, false, way),
                Act::Still(_) => unreachable!("handled above"),
            };
            let now = left.min(u32::from(BATCH)) as u16;
            t_us += 16_667 * i64::from(now);
            let body = format!(
                "{{\"advance_steps\":{now},\"depth_key\":{depth},\"inspect\":null,\
                 \"pause\":false,\"pulse_held\":{held},\"pulse_release\":{release},\
                 \"seq\":{seq},\"steer_x\":{},\"steer_y\":{},\"t_us\":{t_us},\
                 \"toggle_still\":false,\"wheel\":0}}",
                steer.0, steer.1,
            );
            let answer = session.command("input_frame", &body);
            assert!(answer.contains("\"ok\":true"), "{answer}");
            seq += 1;
            left -= u32::from(now);

            record(&mut driven, support::campaign::raised(&mut session));
            let crossed = driven.closed_step.is_some();
            let state = session.run().expect("a run").state();
            let step = state.now.step;

            // Everything read off the Field is read before the transition: a run
            // that completes the chapter stands in the next chapter's Field.
            if crossed {
                continue;
            }
            let standing = state
                .now
                .currents
                .iter()
                .find(|held| held.id == 1)
                .is_some_and(|held| held.active);
            if standing != bright {
                driven.breaks.push((step, standing));
                bright = standing;
            }
            let now_loss = state
                .now
                .layers
                .iter()
                .find(|held| held.layer == 0)
                .map_or(0, |held| held.drain);
            if now_loss != loss {
                driven.losses.push((step, now_loss * 16 / ONE_UNIT));
                loss = now_loss;
            }
            let standing_routes = state.now.routes.iter().map(|held| held.route).collect::<Vec<_>>();
            if driven.fracture_step.is_none() && standing_routes.len() < routes.len() {
                driven.fracture_step = Some(step);
            }
            routes = standing_routes;

            let ring: i64 = state
                .now
                .ports
                .iter()
                .filter(|port| [2, 3, 4].contains(&port.node))
                .map(|port| port.q / ONE_UNIT)
                .sum();
            if driven.offered_at("objective.the_edge.hold_the_edge").is_some() {
                driven.edge_low = driven.edge_low.min(ring);
                driven.edge_high = driven.edge_high.max(ring);
                let carrying = state
                    .now
                    .routes
                    .iter()
                    .filter(|held| [2, 3, 4].contains(&held.route))
                    .all(|held| held.flow > 0);
                if !carrying {
                    driven.edge_stalls += 1;
                }
            }
            driven.ring_units = ring;
            driven.deep_open = state.now.ports.iter().any(|port| port.node == 8 && port.open);
            driven.inside_units = state
                .now
                .ports
                .iter()
                .filter(|port| state.view.inside.contains(&port.node))
                .map(|port| port.q / ONE_UNIT)
                .sum();
            driven.inside = state.view.inside.clone();
            driven.compartment = state.now.physical_compartment.members.clone();
            driven.impulse = state.progress.impulse;
            driven.routes = state.now.routes.len();
            driven.forms = state.now.forms.len();
        }
    }

    driven.spent = spent;
    let _ = opening_impulse;
    if driven.edge_low == i64::MAX {
        driven.edge_low = 0;
    }
    driven
}

/// One frame that carries a Still Mode toggle, or the frame that lets a ramp
/// finish. No step runs on either.
fn still_frame(seq: u32, t_us: i64, toggle: bool) -> String {
    format!(
        "{{\"advance_steps\":0,\"depth_key\":0,\"inspect\":null,\"pause\":false,\
         \"pulse_held\":false,\"pulse_release\":false,\"seq\":{seq},\"steer_x\":0,\
         \"steer_y\":0,\"t_us\":{t_us},\"toggle_still\":{toggle},\"wheel\":0}}"
    )
}

fn layer_drain(session: &Session, layer: u8) -> i64 {
    session
        .run()
        .expect("a run")
        .state()
        .now
        .layers
        .iter()
        .find(|held| held.layer == layer)
        .map_or(0, |held| held.drain)
}

fn route_ids(session: &Session) -> Vec<u32> {
    session.run().expect("a run").state().now.routes.iter().map(|held| held.route).collect()
}

fn impulse(session: &Session) -> u8 {
    session.run().expect("a run").state().progress.impulse
}

/// The attentive first run: every allowance spent and only the required edit.
fn attentive() -> Driven {
    drive_as("thread", true, Strategy::Baseline)
}

/// The authored floor: no allowance spent.
fn floor() -> Driven {
    drive_as("thread", false, Strategy::Baseline)
}

/// The authored content, read as the worker hands it over.
fn authored() -> content::Content {
    let bundle = field_game_core::json::parse(&support::bundle_with(&support::content_hash()))
        .expect("canonical");
    content::read_bundle(&bundle).expect("the content reads")
}

/// The chapter this file is about.
fn chapter() -> content::Chapter {
    authored().chapter(CHAPTER).expect("The Edge").clone()
}

/// A step count as sim-minutes, to one decimal place.
fn minutes(step: Step) -> String {
    let tenths = i64::from(step) * 10 / (i64::from(STEPS_PER_SECOND) * 60);
    format!("{}.{}", tenths / 10, tenths % 10)
}

/// The nine objectives, in the authored order.
fn authored_order() -> Vec<String> {
    chapter().objectives.iter().map(|held| held.id.clone()).collect()
}

/// Prints one run's milestone table.
fn print_table(named: &str, run: &Driven) {
    println!("\nThe Edge, {named} — milestone table (chapter opened at {})", run.opened_step);
    println!("{:<46} {:>8} {:>8} {:>8}", "milestone", "step", "into", "minutes");
    for milestone in &run.milestones {
        println!(
            "{:<46} {:>8} {:>8} {:>8}",
            milestone.name,
            milestone.step,
            milestone.step.saturating_sub(run.opened_step),
            minutes(milestone.step.saturating_sub(run.opened_step)),
        );
    }
    println!("offered at");
    for (id, step) in &run.offered {
        println!("  {id:<44} {:>8}", step.saturating_sub(run.opened_step));
    }
    println!("the bright current");
    for (step, standing) in &run.breaks {
        println!(
            "  {:>8}   {}",
            step.saturating_sub(run.opened_step),
            if *standing { "stands again" } else { "stops" },
        );
    }
    println!("layer 0 loss, in sixteenths of a unit a step");
    for (step, held) in &run.losses {
        println!("  {:>8}   {held}", step.saturating_sub(run.opened_step));
    }
    if let Some(step) = run.fracture_step {
        println!("the supply was cut at {}", step.saturating_sub(run.opened_step));
    }
    println!(
        "ring low {} high {} units; the inside was read carrying nothing on {} batches of \
         the challenge",
        run.edge_low, run.edge_high, run.edge_stalls,
    );
}

// ---------------------------------------------------------------------------
// The chapter, completed
// ---------------------------------------------------------------------------

#[test]
fn the_chapter_completes_and_carries_the_run_into_the_second_transition() {
    let run = attentive();
    print_table("attentive first run", &run);

    assert_eq!(
        run.completed,
        authored_order()
            .into_iter()
            .filter(|id| !id.ends_with("open_the_deep_store"))
            .collect::<Vec<_>>(),
        "every required objective completes once each, in the authored order",
    );
    let closed = run.closed_step.expect("the chapter completes and the run carries on");
    assert_eq!(
        closed,
        run.at("objective.the_edge.hold_the_edge").expect("the last"),
        "the transition lands on the step the final challenge completed",
    );
}

#[test]
fn the_chapter_asks_for_about_thirty_five_minutes_and_the_floor_stands_under_it() {
    let floor = floor();
    let attentive = attentive();
    print_table("authored floor", &floor);

    let spent: u32 = script(Strategy::Baseline)
        .iter()
        .filter(|phase| phase.allowance)
        .map(|phase| phase.steps)
        .sum();
    println!("\nThe Edge — the authored floor against the attentive first run");
    println!(
        "  authored floor      {:>7} steps  {:>6} minutes",
        floor.span(),
        minutes(floor.span()),
    );
    println!(
        "  attentive first run {:>7} steps  {:>6} minutes",
        attentive.span(),
        minutes(attentive.span()),
    );
    println!("modelled first-run allowances, {spent} steps in all:");
    for phase in script(Strategy::Baseline).iter().filter(|phase| phase.allowance) {
        println!("  {:<40} {:>6} steps", phase.label, phase.steps);
    }

    let minute = 60 * STEPS_PER_SECOND;
    assert!(
        attentive.span() > 30 * minute && attentive.span() < 40 * minute,
        "the attentive run takes {} steps ({} minutes), outside the thirty-five-minute band",
        attentive.span(),
        minutes(attentive.span()),
    );
    assert!(
        floor.span() > 18 * minute && floor.span() < attentive.span(),
        "the authored floor takes {} steps ({} minutes), outside its band",
        floor.span(),
        minutes(floor.span()),
    );
}

#[test]
fn every_starting_form_completes_the_whole_chapter() {
    // The acceptance bar, driven rather than argued: one script, eight Forms,
    // the eight required objectives completing in the authored order under each
    // of them. The script is not re-tuned per Form, or the answer would be
    // about the tuning rather than about the chapter.
    let required: Vec<String> = authored_order()
        .into_iter()
        .filter(|id| !id.ends_with("open_the_deep_store"))
        .collect();
    println!("\nthe eight Forms through the whole of The Edge");
    println!(
        "{:<8} {:>10} {:>9} {:>8} {:>8} {:>8} {:>7} {:>7}",
        "form", "objectives", "steps", "minutes", "ring low", "ring high", "batches", "inside",
    );
    for form in field_game_core::run::FORMS {
        let run = drive_as(form, false, Strategy::Baseline);
        assert_eq!(run.completed, required, "{form} completes the whole chapter in order");
        let span = run.span();
        for stage in &run.stages {
            assert!(
                ["hidden", "active", "complete", "failed_recoverable"].contains(&stage.as_str()),
                "{form} stood in an unexpected objective state: {stage}",
            );
        }
        println!(
            "{form:<8} {:>10} {span:>9} {:>8} {:>8} {:>8} {:>7} {:>7}",
            run.completed.len(),
            minutes(span),
            run.edge_low,
            run.edge_high,
            run.edge_stalls,
            run.inside_units,
        );
        assert_eq!(
            run.compartment,
            vec![2, 4],
            "{form} leaves the same required handle-reachable compartment reshape",
        );
    }
}

// ---------------------------------------------------------------------------
// The final challenge: three escalating pulses
// ---------------------------------------------------------------------------

#[test]
fn the_three_pulses_escalate_and_land_where_they_were_authored() {
    let chapter = chapter();
    let last = chapter.objectives.last().expect("a last objective").id.clone();

    // What the chapter authors: three stops of the bright current, each with a
    // raised layer-0 loss, each longer and deeper than the one before.
    let stops: Vec<(Step, bool)> = chapter
        .events
        .iter()
        .filter(|held| held.objective == last)
        .filter_map(|held| match held.effect {
            content::Effect::CurrentActive { current: 1, active } => Some((held.at, active)),
            _ => None,
        })
        .collect();
    assert_eq!(stops.len(), 6, "three stops, each with its own return");
    let spans: Vec<Step> = stops.chunks(2).map(|pair| pair[1].0 - pair[0].0).collect();
    assert_eq!(spans.len(), 3, "three pulses");
    assert!(spans[0] < spans[1] && spans[1] < spans[2], "each stop is longer: {spans:?}");

    let losses: Vec<i64> = chapter
        .events
        .iter()
        .filter(|held| held.objective == last)
        .filter_map(|held| match held.effect {
            content::Effect::LayerDrain { layer: 0, drain } => Some(drain),
            _ => None,
        })
        .collect();
    let raised: Vec<i64> = losses.chunks(2).map(|pair| pair[0]).collect();
    assert_eq!(raised.len(), 3);
    assert!(raised[0] < raised[1] && raised[1] < raised[2], "each is deeper: {raised:?}");

    // And they land, in every run: the events are objective-relative, so a run
    // that reached the challenge in twenty-five minutes and one that reached it
    // in thirty-five meet the same three at the same offsets.
    for (named, run) in [
        ("the authored floor", floor()),
        ("the attentive run", attentive()),
        ("the run that took the test", drive_as("thread", false, Strategy::StoredCharge)),
    ] {
        let offered = run.offered_at(&last).expect("the final challenge is offered");
        assert_eq!(run.breaks.len(), 6, "{named}: three stops and three returns");
        for (place, (step, standing)) in run.breaks.iter().enumerate() {
            assert_eq!(*standing, place % 2 == 1, "{named}: they alternate");
            // The script re-aims every `BATCH` steps, so what is observed is
            // the batch the change landed in rather than the step itself.
            let into = *step - offered;
            assert!(
                into >= stops[place].0 && into < stops[place].0 + Step::from(BATCH),
                "{named}: the stop {into} steps in is not the authored {}",
                stops[place].0,
            );
        }
        // The ring stalled while they stood, which is what makes them a
        // challenge rather than a decoration.
        assert!(
            run.edge_stalls > 0,
            "{named}: the ring carried through the whole challenge unbroken",
        );
    }
}

#[test]
fn the_analysis_slate_remains_available_but_does_not_define_the_compartment() {
    // Authored and drawn Boundary candidates remain analysis seeds. They may
    // give the View alternatives to compare, but the paid compartment edit is
    // queued independently and cannot commit a new View as a side effect.
    let run = drive_as("thread", false, Strategy::ExpandedCompartment);
    println!("\nThe Edge — the standing slate, read at each Still Mode visit");
    println!("{:<40} {:>11} {:>11} {:>22}", "objective", "candidates", "deficient", "reason");
    for (standing, candidates, deficient, reason) in &run.slates {
        println!(
            "{standing:<40} {candidates:>11} {deficient:>11} {:>22}",
            reason.clone().unwrap_or_else(|| "—".to_string()),
        );
    }

    let (standing, candidates, deficient, reason) =
        run.slates.first().expect("the run entered Still Mode once").clone();
    assert_eq!(
        standing, "objective.the_edge.draw_the_edge",
        "the visit stands under the objective the copy teaches, and not another",
    );
    assert!(
        !deficient,
        "the chapter's analysis slate is deficient ({reason:?})",
    );
    assert!(
        candidates >= 2,
        "the slate holds {candidates} candidate(s), so there is nothing to walk to",
    );
    assert!(
        run.slates.iter().all(|(_, _, held, _)| !held),
        "a later visit found the slate deficient: {:?}",
        run.slates,
    );
    assert_eq!(run.inside, vec![2, 3], "the Observation View remains at its authored start");
    assert_eq!(
        run.compartment,
        vec![2, 3, 4, 5, 6, 7],
        "the independent physical intervention commits its own members",
    );
}

#[test]
fn the_compartment_objective_progresses_only_after_a_physical_reshape() {
    let content = authored();
    let chapter = content.chapter(CHAPTER).expect("The Edge");
    let form = content.forms.first().expect("a Form");
    let (mut field, _) = content::establish(chapter, form).expect("the Field stands");
    let objective = chapter
        .objectives
        .iter()
        .find(|held| held.id == "objective.the_edge.draw_the_edge")
        .expect("the compartment objective");
    assert!(matches!(
        &objective.condition,
        content::Condition::CompartmentReshaped { steps: 1 },
    ));

    let mut progress = Progress::opening();
    progress.chapter_index = CHAPTER;
    progress.complete = chapter
        .objectives
        .iter()
        .take_while(|held| held.id != objective.id)
        .map(|held| held.id.clone())
        .collect();
    progress.complete.sort();
    let mut cues = Vec::new();

    for step in 1..=30 {
        field.step = step;
        let reading = content::StepReading { field: &field, cues: &[] };
        content::advance_objectives(&mut progress, &field, chapter, &reading, &mut cues);
    }
    assert_eq!(progress.objective.id, objective.id);
    assert_eq!(progress.objective.progress, 0, "an unchanged compartment earns no progress");

    field.physical_compartment.members = vec![2, 4];
    field.step += 1;
    let reading = content::StepReading { field: &field, cues: &[] };
    content::advance_objectives(&mut progress, &field, chapter, &reading, &mut cues);
    assert_eq!(
        progress.objective.progress,
        objective.condition.share(),
        "a changed physical membership advances the dedicated objective",
    );
}

#[test]
fn the_chapter_absolute_pressures_land_inside_the_objectives_they_were_written_for() {
    // A `pressure_schedule` counts from the chapter's own opening, which is
    // absolute; an objective is offered when a player reaches it, which is
    // not. The two are reconciled by measuring the window on **every** path a
    // player can take through the chapter — the floor and the attentive run,
    // each taking and each letting go of the optional test — and asserting the
    // binding one. A run that declines a test arrives later than one that takes
    // it whenever the span it stands for is longer than taking it costs, so the
    // skip paths are measured rather than assumed to be the same.
    let chapter = chapter();
    let authored = authored();
    let drain = chapter
        .pressure_schedule
        .iter()
        .find(|entry| entry.pressure == field_game_core::pressure::Pressure::Drain)
        .expect("the chapter schedules Drain as its primary");
    assert!(drain.primary, "Drain is the chapter's headline pressure");
    assert_eq!(drain.target.kind, field_game_core::pressure::TargetKind::Layer);
    assert_eq!(drain.target.id, Some(0), "the layer the inside stands on");
    let fracture = chapter
        .pressure_schedule
        .iter()
        .find(|entry| entry.pressure == field_game_core::pressure::Pressure::Fracture)
        .expect("the chapter schedules Fracture");
    assert!(!fracture.primary, "one primary over the whole schedule is the locked limit");
    assert_eq!(fracture.target.kind, field_game_core::pressure::TargetKind::Route);

    let table = authored
        .pressures
        .table(field_game_core::pressure::Pressure::Drain)
        .expect("Drain's authored table");
    let drain_ends = drain.start_step + table.span() as Step;
    let breaking = authored
        .pressures
        .table(field_game_core::pressure::Pressure::Fracture)
        .expect("Fracture's authored table");
    // The break is the one-shot at crisis entry: the two dwells before it.
    let cut_at = fracture.start_step
        + (breaking.steps(field_game_core::pressure::Stage::Signal)
            + breaking.steps(field_game_core::pressure::Stage::Pressure)) as Step;

    println!("\nThe Edge — the two chapter-absolute pressures, on all four paths");
    println!(
        "  Drain    stands {} to {} — inside the opening objective",
        drain.start_step, drain_ends,
    );
    println!("  Fracture cuts at {cut_at}");
    println!(
        "{:<28} {:>10} {:>10} {:>10} {:>10}",
        "path", "depth from", "depth to", "margin in", "margin out",
    );
    let first = chapter.objectives.first().expect("a first").id.clone();
    let depth = "objective.the_edge.take_the_depth";
    let mut tightest = Step::MAX;
    for (named, allowances, strategy) in [
        ("floor, test let go", false, Strategy::Baseline),
        ("attentive, test let go", true, Strategy::Baseline),
        ("floor, test taken", false, Strategy::StoredCharge),
        ("attentive, test taken", true, Strategy::StoredCharge),
    ] {
        let run = drive_as("thread", allowances, strategy);
        let into = |step: Step| step - run.opened_step;

        // The opening objective holds the whole of Drain's run.
        let opens = into(run.offered_at(&first).expect("offered"));
        let closes = into(run.at(&first).expect("completed"));
        assert!(
            opens <= drain.start_step && drain_ends < closes,
            "{named}: Drain runs {} to {drain_ends}, outside the opening objective ({opens} to {closes})",
            drain.start_step,
        );

        // The reshape objective completes on the first post-commit step; the
        // following depth objective holds the Fracture's cut while the player
        // observes the reshaped compartment under load.
        let from = into(run.offered_at(depth).expect("offered"));
        let to = into(run.at(depth).expect("completed"));
        let cut = into(run.fracture_step.expect("the supply was cut"));
        // The one-shot rides the step boundary and the script reads the Field
        // once a batch, so what is observed is the batch it landed in.
        assert!(
            cut >= cut_at && cut - cut_at <= Step::from(BATCH),
            "{named}: the cut landed {cut} steps in, not within one batch of its \
             authored {cut_at}",
        );
        println!("{named:<28} {from:>10} {to:>10} {:>10} {:>10}", cut - from, to - cut);
        assert!(
            cut > from && cut < to,
            "{named}: the cut at {cut} stands outside {from} to {to}",
        );
        tightest = tightest.min((cut - from).min(to - cut));
    }
    println!("the binding margin on any path is {tightest} steps");
    assert!(
        tightest > 900,
        "the tightest margin around the cut is {tightest} steps, which is under half a \
         minute of a first run's own pace",
    );
}

// ---------------------------------------------------------------------------
// The done-when: three distinct strategies
// ---------------------------------------------------------------------------

#[test]
fn three_distinct_strategies_complete_the_chapter() {
    // The goal's own done-when. Each of the three is a different set of player
    // acts against the same authored chapter — a reshaped compartment, a supply
    // formed again after Fracture took it, and the deep store taken and carried
    // in — and each completes the chapter.
    let required: Vec<String> = authored_order()
        .into_iter()
        .filter(|id| !id.ends_with("open_the_deep_store"))
        .collect();
    let runs: Vec<Driven> = [
        Strategy::ExpandedCompartment,
        Strategy::RapidReroute,
        Strategy::StoredCharge,
    ]
    .into_iter()
    .map(|strategy| drive_as("thread", false, strategy))
    .collect();
    let baseline = floor();

    println!("\nThe Edge — four ways through the same chapter");
    println!(
        "{:<18} {:>8} {:>8} {:>8} {:>7} {:>7} {:>7} {:>22}",
        "strategy", "steps", "ring low", "ring high", "batches", "spent", "routes", "compartment",
    );
    for run in std::iter::once(&baseline).chain(runs.iter()) {
        println!(
            "{:<18} {:>8} {:>8} {:>8} {:>7} {:>7} {:>7} {:>22}",
            run.strategy.label(),
            run.span(),
            run.edge_low,
            run.edge_high,
            run.edge_stalls,
            run.spent,
            run.routes,
            format!("{:?}", run.compartment),
        );
    }

    for run in &runs {
        let named = run.strategy.label();
        assert!(run.closed_step.is_some(), "{named}: the chapter completes");
        let mut done = run.completed.clone();
        done.retain(|id| !id.ends_with("open_the_deep_store"));
        assert_eq!(done, required, "{named}: every required objective, in order");
    }

    // Each acted on the Field in its own way, and the Field says so.
    let strong = &runs[0];
    let reroute = &runs[1];
    let stored = &runs[2];
    assert_eq!(
        strong.compartment,
        vec![2, 3, 4, 5, 6, 7],
        "the physical compartment was expanded",
    );
    assert_eq!(
        baseline.compartment,
        vec![2, 4],
        "the baseline made the minimum required physical reshape",
    );
    assert_ne!(baseline.compartment, strong.compartment, "and the baseline's was not");
    assert_eq!(
        baseline.inside, strong.inside,
        "the paid physical edit leaves the Observation View unchanged",
    );
    assert!(
        reroute.routes > baseline.routes,
        "the reroute stands a Route the baseline does not: {} against {}",
        reroute.routes,
        baseline.routes,
    );
    assert!(stored.deep_open, "the stored-charge run took the deep store");
    assert!(!baseline.deep_open, "and the baseline let it go");

    // What each of them is **worth**, pinned. The first round asserted that
    // each run acted differently on the Field — a reshaped compartment, a Route the
    // baseline does not stand, a Port the baseline let go — and then described
    // the benefit in prose. A strategy whose acts land and whose benefit
    // vanished would have passed. These are the three the report claims, one
    // per strategy, each against the baseline.
    assert!(
        strong.edge_high > baseline.edge_high,
        "the expanded compartment holds {} against the baseline's {}, so cutting the exposure \
         bought its members nothing",
        strong.edge_high,
        baseline.edge_high,
    );
    assert!(
        reroute.edge_stalls < baseline.edge_stalls,
        "the rerouted supply left the inside carrying nothing on {} batches against the \
         baseline's {}, so forming the cut Route again bought nothing",
        reroute.edge_stalls,
        baseline.edge_stalls,
    );
    assert!(
        stored.edge_stalls < baseline.edge_stalls,
        "the stored charge left the inside carrying nothing on {} batches against the \
         baseline's {}, so the deep store bought nothing",
        stored.edge_stalls,
        baseline.edge_stalls,
    );
    // What the stored-charge run's shorter chapter is **not**. It is the only
    // one of the four that takes the optional test, and the script spends 5,600
    // steps letting the test go against about 830 taking it — so the difference
    // in chapter length is the shape of the two scripted paths, not a benefit
    // the stored Charge bought. The benefit the stored Charge bought is the
    // batch count above, and that is what is asserted.
    println!(
        "  the stored-charge run closes {} steps sooner, which is the optional-test path's \
         own length and not the strategy's benefit",
        baseline.span().saturating_sub(stored.span()),
    );
    assert!(
        baseline.spent == 1 && strong.spent == 1 && stored.spent == 1 && reroute.spent == 2,
        "every path spends one Intervention on the required reshape and the reroute spends one more",
    );

    // And they are three different runs, not one run three times: what each
    // leaves in the ring through the challenge is its own.
    let readings = [
        ("baseline", baseline.edge_low, baseline.edge_high, baseline.edge_stalls),
        ("expanded compartment", strong.edge_low, strong.edge_high, strong.edge_stalls),
        ("rapid reroute", reroute.edge_low, reroute.edge_high, reroute.edge_stalls),
        ("stored charge", stored.edge_low, stored.edge_high, stored.edge_stalls),
    ];
    for (place, first) in readings.iter().enumerate() {
        for second in &readings[place + 1..] {
            assert!(
                (first.1, first.2, first.3) != (second.1, second.2, second.3),
                "{} and {} are the same run under two names",
                first.0,
                second.0,
            );
        }
    }
}

// ---------------------------------------------------------------------------
// The optional test, and what the chapter cannot do
// ---------------------------------------------------------------------------

#[test]
fn the_optional_test_is_a_test_of_skill_that_is_passable_and_skippable() {
    let chapter = chapter();
    let tests: Vec<&content::Objective> =
        chapter.objectives.iter().filter(|held| held.optional.is_some()).collect();
    assert_eq!(tests.len(), 1, "the chapter authors one optional test");
    let test = tests[0].id.clone();
    // It asks for a Pulse released at a Node a layer down, which no Pulse from
    // the surface reaches: the locked reach is under 256 units and the distance
    // rule adds 512 for the layer. A resting Form cannot satisfy it, which is
    // what keeps a driven campaign's pass-over honest.
    assert!(matches!(tests[0].condition, content::Condition::PortsOpen { .. }));

    let taken = drive_as("thread", false, Strategy::StoredCharge);
    assert!(taken.deep_open, "the test is passable");
    assert!(taken.completed.contains(&test), "and a taken test is a completed objective");
    assert!(taken.closed_step.is_some(), "and the chapter completes");

    let let_go = floor();
    assert!(!let_go.deep_open, "the test is skippable");
    assert!(!let_go.completed.contains(&test), "a test not taken was not completed");
    assert!(let_go.closed_step.is_some(), "and the chapter completes without it");

    let offered = let_go.offered_at(&test).expect("the test was offered");
    let after = let_go
        .offered_at(chapter.objectives.last().expect("a last").id.as_str())
        .expect("and the objective after it stands");
    assert_eq!(
        after,
        offered + tests[0].optional.expect("a span"),
        "the pass-over lands at the step the span names",
    );
}

#[test]
fn the_chapter_offers_one_objective_at_a_time_and_stands_in_nothing_it_cannot_leave() {
    // Why the guard below can hold at all, asserted off the content rather than
    // argued in a ledger. `pattern_held` reads a setback as `q > capacity`, and
    // every Node this chapter names in a `pattern_held` set authors `capacity`
    // at the locked per-Node ceiling — so no Node in any named set can ever
    // carry more than it can hold, and the chapter's difficulty is starvation
    // rather than flood. The guard that follows is otherwise vacuous: it can
    // only ever pass. This turns it into a tripwire — an authored capacity
    // lowered below the ceiling fails **here**, naming the Node, rather than
    // leaving the guard to pass over a setback the chapter cannot leave.
    let chapter = chapter();
    let mut guarded = 0;
    for objective in &chapter.objectives {
        let content::Condition::PatternHeld { nodes, .. } = &objective.condition else {
            continue;
        };
        for node in nodes {
            let port = chapter.ports.iter().find(|held| held.node == *node).expect("a placed Node");
            assert_eq!(
                port.capacity,
                field_game_core::field::NODE_CHARGE_CAP,
                "{}: node {node} authors a capacity under the locked ceiling, so it can carry \
                 more than it can hold and the chapter has a setback it may not be able to \
                 leave",
                objective.id,
            );
            guarded += 1;
        }
    }
    assert!(guarded > 0, "the chapter names no pattern to hold");
    println!("\nThe Edge — {guarded} Node readings across the chapter's held patterns, every \
              one at the locked per-Node ceiling");

    let run = attentive();
    for stage in &run.stages {
        assert!(
            ["hidden", "active", "complete", "failed_recoverable"].contains(&stage.as_str()),
            "an unexpected objective state: {stage}",
        );
    }
    // Every setback the chapter reached was left: the last state the chapter
    // stood in is never the recoverable failure.
    if let Some(place) = run.stages.iter().rposition(|stage| stage == "failed_recoverable") {
        assert!(
            run.stages[place + 1..].iter().any(|stage| stage == "complete"),
            "the chapter completed past its last setback",
        );
    }
    let order = authored_order();
    for id in &order {
        if id.ends_with("open_the_deep_store") {
            continue;
        }
        assert!(run.offered_at(id).is_some(), "{id} was offered");
    }
    let mut last = 0;
    for (id, step) in &run.offered {
        assert!(*step >= last, "{id} was offered out of order");
        last = *step;
    }
}

#[test]
fn the_authored_conditions_span_the_step_counts_they_name() {
    // A condition's progress is carried as the locked `Frac`, so its span is the
    // authored count to within one per-step share. The conditions are read off
    // the content rather than restated here.
    let content = authored();
    assert_eq!(content.version, CONTENT_VERSION);
    let chapter = chapter();

    let mut spans = Vec::new();
    for objective in &chapter.objectives {
        let target = objective.condition.target();
        let span = content::effective_steps(&objective.condition);
        assert!(span <= target, "{} spans {span} past its authored {target}", objective.id);
        assert!(
            target - span <= target / 32 + 1,
            "{} spans {span}, too far short of its authored {target}",
            objective.id,
        );
        if target > 1 {
            spans.push((objective.id.clone(), target, span));
        }
    }
    let total: i64 = spans.iter().map(|(_, _, span)| span).sum();
    println!("\nThe Edge — authored duration, by objective");
    for (id, target, span) in &spans {
        println!("  {id:<46} authored {target:>6}  spans {span:>6}");
    }
    println!("  {:<46} {:>15} {total:>12}", "in all", "");
    assert!(
        total > 40_000 && total < 50_000,
        "the authored duration comes to {total} steps, outside the band the pacing was set in",
    );
}
