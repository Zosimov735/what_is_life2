//! The Mesh, driven end to end: the whole sixty-minute chapter.
//!
//! The chapter's three systems are several controlled Forms, shared routes, and
//! incomparable Views, and the file is organised around what each makes
//! provable:
//!
//! - **Several controlled Forms.** The chapter places three, and the two it
//!   does not open control on stand beside the two Ports the opening asks for.
//!   Control moves between them by the Handoff — Still Mode, one press, no
//!   Impulse — and the run that never hands control on has to steer the one
//!   Form the length of the field twice over.
//! - **Shared routes.** Both patterns return through one Route, and both can be
//!   drawn from one intake. The ascending Route pass is what makes the sharing
//!   tell: a Route earlier in the pass takes what the tail holds before a later
//!   one sees it, so one intake can supply one pattern and no more.
//! - **Incomparable Views.** The chapter authors two boundaries, one around
//!   each pattern, and the slate ranks them into the same tier with each better
//!   than the other somewhere. Choosing either changes which Forms the run is
//!   played through, what it is exposed to, and what the review reads. Nothing
//!   here declares one correct, and no test asserts one is.
//!
//! Two runs are driven, and they answer two different questions:
//!
//! - The **attentive run** carries a modelled first-time player, spending the
//!   named discovery allowances below. Its milestone table is what the sixty
//!   minutes are read against.
//! - The **authored floor** spends no allowance at all. It is the run the eight
//!   Forms and the two layouts are driven through.

use field_game_core::content::{self, CONTENT_VERSION};
use field_game_core::state::{Step, STEPS_PER_SECOND};
use field_game_core::Session;

mod support;

use support::campaign::{controlled, nearest_point, toward};

const KEY: &str = "00112233445566ee";

/// How many steps one scripted batch runs before the script re-aims.
const BATCH: u16 = 15;

/// How many steps a full Pulse charge takes, from the locked charging rule.
const FULL_CHARGE_STEPS: u32 = 32;

/// How long a ramp takes, in microseconds. Locked.
const RAMP_US: i64 = 250_000;

/// This chapter's place in the authored campaign.
const CHAPTER: u8 = 4;

/// What one phase asks of the run.
#[derive(Clone, Copy, Debug)]
enum Act {
    /// Steer toward a point in whole units.
    Toward(i64, i64),
    /// Steer at the nearest path point of a current, read off the Field.
    Follow(u16),
    /// Hold every input neutral.
    Rest,
    /// Hold the Pulse, steering toward a point.
    Charge(i64, i64),
    /// Release the Pulse.
    Release,
    /// Ask for one layer deeper, or one layer back up.
    Depth(i8),
    /// Hand control to the Form the identifier names.
    Hand(u8),
    /// Enter Still Mode, queue every plan body, commit, and ramp back out.
    Edit(&'static [&'static str]),
}

/// One phase of the script: what it does, how long it runs, and whether its
/// length is authored content or a modelled allowance.
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

/// Which intake the run draws the southern pattern from.
///
/// Both leave the pattern flowing, and the chapter asks for nothing else — so
/// the choice is a choice about the mesh rather than about the objective.
/// `Shared` draws it from the main intake, which is the Node Interference gives
/// first claim on the band; `Spare` opens the second intake and draws it from
/// there.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Layout {
    /// One intake supplies both patterns: the centralized mesh.
    Shared,
    /// Each pattern draws from its own intake.
    Spare,
}

impl Layout {
    /// The one Route the layout asks the player to draw.
    fn plan(self) -> &'static [&'static str] {
        match self {
            Layout::Shared => &["{\"plan\":{\"from\":4,\"op\":\"connect\",\"to\":8}}"],
            Layout::Spare => &["{\"plan\":{\"from\":5,\"op\":\"connect\",\"to\":8}}"],
        }
    }
}

/// The script one run is driven by.
///
/// Every modelled assumption about a first-time player is an `allow` with a
/// label a reader can judge: the floor run skips them and the attentive run
/// spends them, and the difference between the two is the whole of the pacing
/// model.
fn script(layout: Layout, takes_the_test: bool) -> Vec<Phase> {
    let mut phases = vec![
        allow("read the surface", Act::Rest, 60),
        allow("try the controls", Act::Toward(1200, 1800), 1_200),
        allow("find the band", Act::Follow(1), 900),
        // 1 — the supply.
        play("stand in the band", Act::Follow(1), 5_600),
        // 2 — the northern store, opened from the Form standing beside it.
        allow("look for the other two Forms", Act::Follow(1), 2_400),
        play("take the northern Form", Act::Hand(2), 0),
        play("charge beside the northern store", Act::Charge(1450, 1300), FULL_CHARGE_STEPS),
        play("release", Act::Release, 1),
        // 3 — the southern store, the same way.
        play("take the southern Form", Act::Hand(3), 0),
        play("charge beside the southern store", Act::Charge(1750, 2500), FULL_CHARGE_STEPS),
        play("release", Act::Release, 1),
        // 4 — back to the steered Form, and hold the northern pattern.
        play("take the first Form back", Act::Hand(1), 0),
        allow("look for what the pattern needs", Act::Follow(1), 2_700),
        play("hold the northern pattern", Act::Follow(1), 34_000),
        // 5 — the spare intake, further east along the band.
        allow("look for the second intake", Act::Follow(1), 2_400),
        play("go to the spare intake", Act::Toward(2050, 2000), 600),
        play("charge beside it", Act::Charge(2050, 2000), FULL_CHARGE_STEPS),
        play("release", Act::Release, 1),
        // 6 — the southern pattern gets its way in.
        allow("look for the way in", Act::Rest, 2_400),
        play("draw the route", Act::Edit(layout.plan()), 0),
        play("let the route prime", Act::Follow(1), 120),
        // 7 — the depth.
        allow("look for what lies deeper", Act::Toward(1700, 2400), 1_800),
        play("go over the deep band", Act::Toward(1700, 2400), 700),
        play("descend", Act::Depth(1), 1),
        play("stand in the deep band", Act::Follow(2), 2_800),
    ];
    // 8 — the optional test.
    if takes_the_test {
        phases.push(play("stand beside the deep port", Act::Toward(1700, 2400), 400));
        phases.push(play("charge toward the deep port", Act::Charge(1700, 2440), FULL_CHARGE_STEPS));
        phases.push(play("release", Act::Release, 1));
        phases.push(play("back into the deep band", Act::Follow(2), 300));
    } else {
        phases.push(play("let the test stand for its span", Act::Follow(2), 5_600));
    }
    phases.extend([
        play("ascend", Act::Depth(-1), 1),
        allow("find the band again", Act::Follow(1), 1_200),
        // 9 — the final challenge, and the authored event inside it: the spare
        // intake closes, the southern pattern stops with it, and a Pulse
        // released beside it is the one thing that opens it again.
        play("hold both patterns", Act::Follow(1), 9_200),
        allow("look for what stopped", Act::Follow(1), 1_800),
        play("go back to the spare intake", Act::Toward(2050, 2000), 700),
        play("charge beside it", Act::Charge(2050, 2000), FULL_CHARGE_STEPS),
        play("release", Act::Release, 1),
        play("hold both patterns", Act::Follow(1), 60_000),
        play("hold both patterns", Act::Follow(1), 20_000),
    ]);
    phases
}

/// What one driven run recorded.
#[derive(Default)]
struct Driven {
    /// Every objective completion, in order: the catalog id and the step.
    milestones: Vec<(String, Step)>,
    /// The step each objective was offered at, in order.
    offered: Vec<(String, Step)>,
    completed: Vec<String>,
    opening_step: Step,
    transition_step: Option<Step>,
    anchor_steps: Vec<Step>,
    setbacks: Vec<(String, Step)>,
    recovery_step: Option<Step>,
    /// The states every objective stood in, in order.
    states: Vec<String>,
    /// Steps on which every Route of both patterns carried Charge.
    both_standing: u32,
    /// Steps on which the southern pattern carried nothing at all.
    south_stalled: u32,
    /// The stage entries Interference reported.
    stages: Vec<(String, Step)>,
    /// The steps Interference actually **stood** at, which is not the steps the
    /// list held an entry: a scheduled entry sits queued in the list from the
    /// chapter's opening until its own step admits it.
    pressed: Vec<Step>,
    /// The step Interference first stood active at.
    interference_step: Option<Step>,
    /// Whether the deep Port was opened.
    deep_open: bool,
    /// What the run held at the transition, read before it crossed.
    impulse: u8,
    forms: usize,
    /// Every allowance the script spent, in the order it spent them.
    allowances: Vec<(&'static str, u32)>,
    probe_at: u32,
    /// Steps the final challenge stood for, and what stood inside it.
    challenge_steps: u32,
    both_in_challenge: u32,
    strained_in_challenge: u32,
}

/// The routes each pattern is held by, as the chapter authors them.
const NORTH: [u32; 4] = [1, 2, 3, 6];
const SOUTH: [u32; 3] = [4, 5, 6];

fn authored() -> content::Content {
    let bundle = support::bundle_with(&support::content_hash());
    let parsed = field_game_core::json::parse(&bundle).expect("canonical");
    let content = content::read_bundle(&parsed).expect("the shipped campaign loads");
    assert_eq!(content.version, CONTENT_VERSION);
    content
}

fn record(driven: &mut Driven, events: &[(String, u32, field_game_core::json::Json)]) {
    use field_game_core::json::Json;
    for (name, step, body) in events {
        match name.as_str() {
            "objective_changed" => {
                let objective = body.get("objective").expect("an objective");
                let id = objective.get("id").and_then(Json::as_text).unwrap_or("").to_string();
                let state = objective.get("state").and_then(Json::as_text).unwrap_or("");
                // This chapter's own sequence and no other: the run plays four
                // chapters to reach this one, and their objectives are theirs.
                if !id.starts_with("objective.the_mesh.") {
                    continue;
                }
                driven.states.push(state.to_string());
                match state {
                    "active" => {
                        if !driven.offered.iter().any(|(held, _)| held == &id) {
                            driven.offered.push((id.clone(), *step));
                        } else if driven.recovery_step.is_none() && !driven.setbacks.is_empty() {
                            driven.recovery_step = Some(*step);
                        }
                    }
                    "complete" => {
                        if !driven.completed.contains(&id) {
                            driven.completed.push(id.clone());
                            driven.milestones.push((id, *step));
                        }
                    }
                    "failed_recoverable" => driven.setbacks.push((id.clone(), *step)),
                    _ => {}
                }
            }
            "checkpoint_written" => driven.anchor_steps.push(*step),
            "chapter_changed" => {
                let index = body.get("chapter_index").and_then(Json::as_int).unwrap_or(-1);
                if index == i64::from(CHAPTER) {
                    driven.opening_step = *step;
                } else if index > i64::from(CHAPTER) && driven.transition_step.is_none() {
                    driven.transition_step = Some(*step);
                }
            }
            "pressure_changed" => {
                if let Some(Json::List(held)) = body.get("pressures") {
                    for entry in held {
                        if entry.get("pressure").and_then(Json::as_text) != Some("interference") {
                            continue;
                        }
                        if entry.get("queued").and_then(Json::as_bool) == Some(true) {
                            continue;
                        }
                        let stage =
                            entry.get("stage").and_then(Json::as_text).unwrap_or("").to_string();
                        if driven.interference_step.is_none() {
                            driven.interference_step = Some(*step);
                        }
                        if driven.stages.last().map(|(held, _)| held.as_str()) != Some(&stage) {
                            driven.stages.push((stage, *step));
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

/// Whether every Route of a list carried Charge on the step just completed.
fn flowing(session: &Session, routes: &[u32]) -> bool {
    let state = session.run().expect("a run").state();
    routes.iter().all(|named| {
        state.now.routes.iter().any(|route| route.route == *named && route.flow > 0)
    })
}

/// Drives one whole run of the chapter, from the campaign's opening.
fn drive(form: &str, layout: Layout, allowances: bool, takes_the_test: bool) -> Driven {
    let mut session = Session::new(&support::worker_init()).expect("versions agree");
    let opened = session.command(
        "init_run",
        &format!("{{\"form\":\"{form}\",\"mode\":\"new\",\"run_id\":\"{KEY}\"}}"),
    );
    assert!(opened.contains("\"ok\":true"), "{opened}");
    let mut driven = Driven::default();
    let mut driver = support::campaign::Driver::new();
    let content = authored();
    for index in 0..CHAPTER {
        support::campaign::play_chapter(
            &mut session,
            &mut driver,
            content.chapter(index).expect("a chapter"),
        );
    }
    assert_eq!(
        support::campaign::chapter_index(&session),
        CHAPTER,
        "the run stands in The Mesh",
    );
    driver.drain(&mut session);
    record(&mut driven, &driver.raised);
    driven.opening_step = driver
        .raised
        .iter()
        .rev()
        .find(|(name, _, body)| {
            name == "chapter_changed"
                && body.get("chapter_index").and_then(|held| held.as_int())
                    == Some(i64::from(CHAPTER))
        })
        .map(|(_, step, _)| *step)
        .expect("the run entered the chapter");
    driven.anchor_steps.clear();
    driven.stages.clear();
    driver.raised.clear();

    let mut raised = Vec::new();
    for phase in script(layout, takes_the_test) {
        if driven.transition_step.is_some() {
            break;
        }
        if phase.allowance && !allowances {
            continue;
        }
        if phase.allowance {
            driven.allowances.push((phase.label, phase.steps));
        }
        match phase.act {
            Act::Hand(id) => {
                let before = session.run().expect("a run").state().now.step;
                driver.hand_control(&mut session, id);
                raised.append(&mut std::mem::take(&mut driver.raised));
                record(&mut driven, &raised);
                raised.clear();
                assert_eq!(
                    session.run().expect("a run").state().now.step,
                    before,
                    "a Handoff spends no simulated time",
                );
                continue;
            }
            Act::Edit(plans) => {
                driver.commit_edit(&mut session, plans);
                raised.append(&mut std::mem::take(&mut driver.raised));
                record(&mut driven, &raised);
                raised.clear();
                continue;
            }
            _ => {}
        }
        let mut left = phase.steps;
        while left > 0 {
            if driven.transition_step.is_some() {
                break;
            }
            let at = controlled(&session);
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
                Act::Hand(_) | Act::Edit(_) => ((0, 0), false, false, 0),
            };
            let now = left.min(u32::from(BATCH)) as u16;
            driver.depth_frame(&mut session, now, steer, held, release, depth);
            left -= u32::from(now);
            // Read before the transition: a run that has completed the chapter
            // stands in the next chapter's Field.
            let standing = support::campaign::chapter_index(&session) == CHAPTER;
            if standing {
                if std::env::var("MESH_PROBE").is_ok() && driven.probe_at % 40 == 0 {
                    let state = session.run().expect("a run").state();
                    let q: Vec<(u32, i64, i64)> = state
                        .now
                        .ports
                        .iter()
                        .map(|port| (port.node, port.q / 65536, port.capacity / 65536))
                        .collect();
                    let f: Vec<(u32, i64)> = state
                        .now
                        .routes
                        .iter()
                        .map(|route| (route.route, route.flow))
                        .collect();
                    println!("step {} q {q:?} f {f:?}", state.now.step);
                }
                driven.probe_at += 1;
                let north = flowing(&session, &NORTH);
                let south = flowing(&session, &SOUTH);
                if north && south {
                    driven.both_standing += u32::from(now);
                }
                if !south {
                    driven.south_stalled += u32::from(now);
                }
                let state = session.run().expect("a run").state();
                driven.impulse = state.progress.impulse;
                driven.forms = state.now.forms.len();
                // The seat itself, read off the run rather than off an event:
                // the pressure stands when the list holds it unqueued.
                if state.pressures.iter().any(|held| {
                    !held.queued && held.pressure == field_game_core::pressure::Pressure::Interference
                }) {
                    driven.pressed.push(state.now.step);
                }
                driven.deep_open =
                    state.now.ports.iter().any(|port| port.node == 11 && port.open);
                // The challenge's own instruments, taken only while it stands.
                if driven.offered.iter().any(|(id, _)| id.ends_with("hold_both_patterns"))
                    && !driven.completed.iter().any(|id| id.ends_with("hold_both_patterns"))
                {
                    driven.challenge_steps += u32::from(now);
                    if north && south {
                        driven.both_in_challenge += u32::from(now);
                    }
                    let over = [4u32, 5, 8].iter().any(|node| {
                        state
                            .now
                            .ports
                            .iter()
                            .any(|port| port.node == *node && port.q > port.capacity)
                    });
                    if over {
                        driven.strained_in_challenge += u32::from(now);
                    }
                }
            }
            raised.append(&mut std::mem::take(&mut driver.raised));
            record(&mut driven, &raised);
            raised.clear();
        }
    }
    driver.drain(&mut session);
    raised.append(&mut std::mem::take(&mut driver.raised));
    record(&mut driven, &raised);
    driven
}

/// The chapter's own step count, from its opening.
fn since(driven: &Driven, step: Step) -> Step {
    step.saturating_sub(driven.opening_step)
}

fn minutes(steps: Step) -> f64 {
    f64::from(steps) / f64::from(STEPS_PER_SECOND) / 60.0
}

fn print_table(label: &str, driven: &Driven) {
    println!("\n=== {label} ===");
    println!("opening step {}", driven.opening_step);
    for (id, step) in &driven.milestones {
        println!(
            "  {:<46} {:>7} / {:>5.1} min",
            id.trim_start_matches("objective.the_mesh."),
            since(driven, *step),
            minutes(since(driven, *step)),
        );
    }
    if let Some(step) = driven.transition_step {
        println!(
            "  {:<46} {:>7} / {:>5.1} min",
            "transition",
            since(driven, step),
            minutes(since(driven, step)),
        );
    }
    println!(
        "  both patterns standing {} steps; the southern one stalled {} steps",
        driven.both_standing, driven.south_stalled,
    );
    println!("  setbacks {} anchors {}", driven.setbacks.len(), driven.anchor_steps.len());
    for (id, step) in &driven.setbacks {
        println!(
            "    setback in {:<40} {:>7}",
            id.trim_start_matches("objective.the_mesh."),
            since(driven, *step),
        );
    }
    println!("  interference stages {:?}", driven.stages);
    if !driven.allowances.is_empty() {
        let total: u32 = driven.allowances.iter().map(|(_, steps)| steps).sum();
        println!("  allowances, {total} steps in all:");
        for (label, steps) in &driven.allowances {
            println!("    {label:<40} {steps:>6}");
        }
    }
}

#[test]
fn the_chapter_completes_and_carries_the_run_into_its_own_transition() {
    let floor = drive("thread", Layout::Spare, false, false);
    print_table("the authored floor, the spare intake", &floor);
    let attentive = drive("thread", Layout::Spare, true, false);
    print_table("the attentive run, the spare intake", &attentive);

    let required: Vec<String> = authored()
        .chapter(CHAPTER)
        .expect("the chapter")
        .objectives
        .iter()
        .filter(|held| held.optional.is_none())
        .map(|held| held.id.clone())
        .collect();
    assert_eq!(floor.completed, required, "every required objective, in authored order");
    assert!(floor.transition_step.is_some(), "the floor reaches the transition");
    assert!(attentive.transition_step.is_some(), "and so does the attentive run");
}

#[test]
fn the_chapter_asks_for_about_sixty_minutes_and_the_floor_stands_under_it() {
    let floor = drive("thread", Layout::Spare, false, false);
    let attentive = drive("thread", Layout::Spare, true, false);
    let floor_at = since(&floor, floor.transition_step.expect("the floor completes"));
    let held = since(&attentive, attentive.transition_step.expect("the attentive run completes"));
    println!(
        "floor {floor_at} steps / {:.1} min; attentive {held} steps / {:.1} min",
        minutes(floor_at),
        minutes(held),
    );
    assert!(
        minutes(held) > 57.0 && minutes(held) < 63.0,
        "the attentive run asks for about sixty minutes: {:.1}",
        minutes(held),
    );
    assert!(
        minutes(floor_at) > 36.0 && floor_at < held,
        "the authored floor stands under it and above two thirds of it: {:.1}",
        minutes(floor_at),
    );
    let asked: u32 = authored()
        .chapter(CHAPTER)
        .expect("the chapter")
        .objectives
        .iter()
        .filter(|held| held.optional.is_none())
        .map(|held| content::effective_steps(&held.condition) as u32)
        .sum();
    println!("the authored conditions alone ask for {asked} steps / {:.1} min", minutes(asked));
    assert!(asked < floor_at, "the floor is the authored duration plus the travel it takes");
}

#[test]
fn every_starting_form_completes_the_whole_chapter() {
    // One script, never re-tuned per Form, at the authored floor.
    println!("\nform     objectives  completes  minutes  both  stalled  setbacks  forms");
    for form in ["thread", "ring", "relay", "vault", "lens", "knot", "wake", "chorus"] {
        let driven = drive(form, Layout::Spare, false, false);
        let at = driven
            .transition_step
            .map(|step| since(&driven, step))
            .unwrap_or_else(|| {
                println!("{form}: no transition; completed {:?}", driven.completed);
                0
            });
        println!(
            "{form:<8} {:>10} {:>10} {:>8.1} {:>5} {:>8} {:>9} {:>6}",
            driven.completed.len(),
            at,
            minutes(at),
            driven.both_standing,
            driven.south_stalled,
            driven.setbacks.len(),
            driven.forms,
        );
        assert!(driven.transition_step.is_some(), "{form}: the chapter completes");
        assert_eq!(driven.completed.len(), 8, "{form}: every required objective");
        assert!(driven.both_standing > 20_000, "{form}: both patterns stood for a long while");
        // Chorus in a multi-Form chapter is the flagship case: the chapter
        // places three Forms and the selection stands three more beside the
        // first, which is six of the eight a run may hold.
        let wanted = if form == "chorus" { 6 } else { 3 };
        assert_eq!(driven.forms, wanted, "{form}: the Forms the chapter and the selection stand");
    }
}

#[test]
fn centralizing_the_mesh_on_one_intake_costs_the_southern_pattern() {
    // The teeth, measured rather than asserted from the layout: the same
    // script, the same Form, the same allowances, and one line of difference —
    // which intake the southern pattern is drawn from.
    let spare = drive("thread", Layout::Spare, false, false);
    let shared = drive("thread", Layout::Shared, false, false);
    println!(
        "\nlayout   both standing  southern stalled  completed  transition\n\
         spare    {:>13}  {:>16}  {:>9}  {:?}\n\
         shared   {:>13}  {:>16}  {:>9}  {:?}",
        spare.both_standing,
        spare.south_stalled,
        spare.completed.len(),
        spare.transition_step.map(|step| since(&spare, step)),
        shared.both_standing,
        shared.south_stalled,
        shared.completed.len(),
        shared.transition_step.map(|step| since(&shared, step)),
    );
    let share = |driven: &Driven, held: u32| -> f64 {
        if driven.challenge_steps == 0 {
            return 0.0;
        }
        f64::from(held) * 100.0 / f64::from(driven.challenge_steps)
    };
    println!(
        "layout   challenge steps  both standing  a named Node over its threshold\n\
         spare    {:>15}  {:>12.1}%  {:>28.1}%\n\
         shared   {:>15}  {:>12.1}%  {:>28.1}%",
        spare.challenge_steps,
        share(&spare, spare.both_in_challenge),
        share(&spare, spare.strained_in_challenge),
        shared.challenge_steps,
        share(&shared, shared.both_in_challenge),
        share(&shared, shared.strained_in_challenge),
    );
    // The teeth, and they are the mesh's own rather than the pressure's alone.
    // A centralized mesh leaves the spare intake standing in the band with
    // nowhere to send what it takes, so it carries more than it can hold for
    // most of the challenge — and the challenge names it. Interference is what
    // makes centralizing look right: it gives the main intake first claim on
    // everything the band delivers, so one intake looks like enough. It is
    // not, and the run that took that reading does not finish the chapter.
    assert!(
        share(&shared, shared.strained_in_challenge) > 80.0,
        "the centralized mesh stands over its threshold for most of the challenge: {:.1}%",
        share(&shared, shared.strained_in_challenge),
    );
    assert!(
        share(&spare, spare.strained_in_challenge) < 10.0,
        "and the two-intake mesh only for the window the authored event opens: {:.1}%",
        share(&spare, spare.strained_in_challenge),
    );
    // Both patterns do turn under the centralized mesh — the drawn Route
    // carries what the main intake has left after the northern pattern has
    // taken its share, and that is more than nothing. What it costs is the
    // spare intake, which keeps taking from the band with nowhere to send it:
    // the challenge names that Node, so the pattern reads as held and strained
    // at once, and a strained step is not a step the challenge counts.
    assert!(
        shared.challenge_steps > spare.challenge_steps,
        "the centralized run spends longer inside a challenge it never completes",
    );
    assert!(
        shared.transition_step.is_none(),
        "the centralized run does not reach the chapter's own transition inside the script",
    );
    assert!(spare.transition_step.is_some(), "and the two-intake run does");
}

#[test]
fn the_authored_event_stands_once_and_the_chapter_carries_past_it() {
    let floor = drive("thread", Layout::Spare, false, false);
    assert_eq!(floor.setbacks.len(), 1, "the chapter authors one collapse, and it stands once");
    let (id, step) = &floor.setbacks[0];
    assert_eq!(id, "objective.the_mesh.hold_both_patterns");
    let offered = floor
        .offered
        .iter()
        .find(|(held, _)| held == id)
        .map(|(_, step)| *step)
        .expect("the challenge was offered");
    // The event is objective-relative, so it lands at the same offset on every
    // path however long the run took to reach the challenge.
    println!(
        "the spare intake closed {} steps into the challenge; the setback stood at {}",
        step.saturating_sub(offered),
        since(&floor, *step),
    );
    assert!(floor.recovery_step.is_some(), "and the run came back from it");
    assert!(
        floor.transition_step.is_some_and(|held| held > *step),
        "and carried on past it to the transition",
    );
    // Only the four locked objective states are ever stood in.
    for state in &floor.states {
        assert!(
            ["active", "complete", "failed_recoverable", "hidden"].contains(&state.as_str()),
            "an objective stood in {state}",
        );
    }
}

#[test]
fn the_optional_test_is_a_test_of_skill_that_is_passable_and_skippable() {
    let taken = drive("thread", Layout::Spare, false, true);
    let let_go = drive("thread", Layout::Spare, false, false);
    assert!(taken.deep_open, "the deep Port opens for a run that takes the test");
    assert!(!let_go.deep_open, "and stands closed for one that does not");
    assert!(
        taken.completed.contains(&"objective.the_mesh.open_the_deep_port".to_string()),
        "taking it completes it",
    );
    assert!(
        !let_go.completed.contains(&"objective.the_mesh.open_the_deep_port".to_string()),
        "letting it go passes it over",
    );
    // Letting it go is slower than taking it, which is what makes the pacing
    // window below have two ends from two different paths.
    let taken_at = since(&taken, taken.transition_step.expect("completes"));
    let let_go_at = since(&let_go, let_go.transition_step.expect("completes"));
    println!("taken {taken_at}; let go {let_go_at}");
    assert!(taken_at < let_go_at, "taking the test is quicker than waiting it out");
}

#[test]
fn interference_lands_inside_the_final_challenge_on_every_path() {
    // The four paths: floor and attentive, each taking the optional test and
    // letting it go. `pressure_schedule.start_step` counts from the chapter's
    // own opening and an objective is offered when a player reaches it, so the
    // window is `[max over paths of offered, min over paths of completed]` and
    // its two ends come from two different paths.
    let mut window = (0u32, u32::MAX);
    let mut margins = (u32::MAX, u32::MAX);
    println!("\npath                      offered  first stage  last stage  completed  open  close");
    for (label, allowances, takes) in [
        ("floor, test taken", false, true),
        ("floor, test let go", false, false),
        ("attentive, test taken", true, true),
        ("attentive, test let go", true, false),
    ] {
        let driven = drive("thread", Layout::Spare, allowances, takes);
        let id = "objective.the_mesh.hold_both_patterns".to_string();
        let offered = since(
            &driven,
            driven.offered.iter().find(|(held, _)| *held == id).expect("offered").1,
        );
        let completed = since(
            &driven,
            driven.milestones.iter().find(|(held, _)| *held == id).expect("completed").1,
        );
        let stages: Vec<u32> = driven.stages.iter().map(|(_, step)| since(&driven, *step)).collect();
        assert!(!stages.is_empty(), "{label}: Interference seated and staged");
        let first = *stages.first().expect("a stage");
        let last = *stages.last().expect("a stage");
        println!(
            "{label:<24} {offered:>8} {first:>12} {last:>11} {completed:>10} \
             {:>5} {:>6}",
            first.saturating_sub(offered),
            completed.saturating_sub(last),
        );
        assert!(first > offered, "{label}: Interference arrives inside the challenge");
        assert!(last < completed, "{label}: and resolves inside it");

        // It **seats**, which is not the same as being staged: the list held it
        // unqueued on the run's own steps, and every one of those steps stands
        // inside the challenge too. A schedule that only ever queued would
        // print the same stage table as this one.
        assert!(!driven.pressed.is_empty(), "{label}: Interference stood, not merely staged");
        let stood_first = since(&driven, *driven.pressed.first().expect("a first reading"));
        let stood_last = since(&driven, *driven.pressed.last().expect("a last reading"));
        assert!(
            stood_first > offered && stood_last < completed,
            "{label}: Interference stood only inside the challenge, \
             [{stood_first}, {stood_last}] against [{offered}, {completed}]",
        );
        // And the stages it reported walk the closed set's own order, once
        // each: a schedule that re-entered a stage, or reported them out of
        // order, is a schedule that did not run its window through.
        let walked: Vec<usize> = driven
            .stages
            .iter()
            .map(|(held, _)| {
                field_game_core::pressure::STAGES
                    .iter()
                    .position(|name| name == held)
                    .expect("a stage of the closed set")
            })
            .collect();
        assert!(
            walked.windows(2).all(|pair| pair[0] < pair[1]),
            "{label}: the stages walk the closed set's order: {:?}",
            driven.stages,
        );
        window.0 = window.0.max(offered);
        window.1 = window.1.min(completed);
        margins.0 = margins.0.min(first - offered);
        margins.1 = margins.1.min(completed - last);
    }
    println!("the binding window is [{}, {}]", window.0, window.1);
    println!("the tightest margins are {} opening and {} closing", margins.0, margins.1);
    // A schedule that has drifted to the edge of one path reads exactly like
    // one that has not, so the margin is asserted rather than printed.
    assert!(margins.0 >= 300, "the opening margin holds its floor: {}", margins.0);
    assert!(margins.1 >= 300, "the closing margin holds its floor: {}", margins.1);
}

#[test]
fn the_chapter_offers_one_objective_at_a_time_and_none_of_them_fails_terminally() {
    let driven = drive("thread", Layout::Spare, true, true);
    // Every objective the chapter authors was offered, in the authored order,
    // and one at a time: the offered list is a prefix-ordered walk of the
    // chapter's own sequence.
    let authored_ids: Vec<String> = authored()
        .chapter(CHAPTER)
        .expect("the chapter")
        .objectives
        .iter()
        .map(|held| held.id.clone())
        .collect();
    let offered: Vec<String> = driven
        .offered
        .iter()
        .map(|(id, _)| id.clone())
        .filter(|id| id.starts_with("objective.the_mesh."))
        .collect();
    assert_eq!(offered, authored_ids, "every objective, once, in the authored order");
    assert!(!driven.states.iter().any(|state| state == "failed_terminal"));
}

/// Drives the chapter to a point inside its final challenge, enters Still Mode
/// so a slate is assembled, and answers the run standing there.
fn stood_in_the_challenge(layout: Layout) -> Session {
    let mut session = Session::new(&support::worker_init()).expect("versions agree");
    let opened = session.command(
        "init_run",
        &format!("{{\"form\":\"thread\",\"mode\":\"new\",\"run_id\":\"{KEY}\"}}"),
    );
    assert!(opened.contains("\"ok\":true"), "{opened}");
    let mut driver = support::campaign::Driver::new();
    let content = authored();
    for index in 0..CHAPTER {
        support::campaign::play_chapter(
            &mut session,
            &mut driver,
            content.chapter(index).expect("a chapter"),
        );
    }
    let mut standing = String::new();
    for phase in script(layout, false) {
        if standing.ends_with("hold_both_patterns") {
            break;
        }
        match phase.act {
            Act::Hand(id) => {
                driver.hand_control(&mut session, id);
                continue;
            }
            Act::Edit(plans) => {
                driver.commit_edit(&mut session, plans);
                continue;
            }
            _ => {}
        }
        let mut left = phase.steps;
        while left > 0 {
            let at = controlled(&session);
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
                Act::Hand(_) | Act::Edit(_) => ((0, 0), false, false, 0),
            };
            let now = left.min(u32::from(BATCH)) as u16;
            driver.depth_frame(&mut session, now, steer, held, release, depth);
            left -= u32::from(now);
            standing =
                session.run().expect("a run").state().progress.objective.id.clone();
            if standing.ends_with("hold_both_patterns") {
                break;
            }
        }
    }
    assert!(standing.ends_with("hold_both_patterns"), "the run stands in the final challenge");
    // Another eight thousand steps, so the challenge has been standing a while
    // and both patterns have been turning under the window the slate reads.
    for _ in 0..540 {
        let (x, y) = nearest_point(&session, 1).unwrap_or((0, 0));
        let at = controlled(&session);
        driver.frame(&mut session, BATCH, toward(at, x, y), false, false);
    }
    // Into Still Mode, which is where a slate is assembled.
    let opened_at = i64::from(driver.seq) * 1_000_000 + 10_000_000;
    driver.stamped_frame(&mut session, 0, opened_at, true);
    driver.stamped_frame(&mut session, 0, opened_at + RAMP_US, false);
    assert_eq!(session.lifecycle(), "still");
    session
}

#[test]
fn the_slate_offers_two_maximal_views_and_declares_neither_correct() {
    use field_game_core::rank::dominates;
    let session = stood_in_the_challenge(Layout::Spare);
    let run = session.run().expect("a run").clone();
    let slate = run.standing_slate().expect("a slate is assembled on the Still Mode entry");

    println!("\ntier  inside            provenance   BS      CI      SS      SF");
    let name = |value: &field_game_core::slate::PrivilegeValue| -> String {
        match value.value {
            Some(held) => format!("{:.3}", held as f64 / 65536.0),
            None => "  --  ".to_string(),
        }
    };
    for candidate in &slate.candidates {
        let sources: Vec<&str> =
            candidate.provenance.iter().map(|held| held.source.name()).collect();
        println!(
            "{:>4}  {:<16}  {:<11}  {}  {}  {}  {}",
            candidate.tier,
            format!("{:?}", candidate.view.inside),
            sources.join(","),
            name(&candidate.privilege.boundary_sufficiency),
            name(&candidate.privilege.cut_impact),
            name(&candidate.privilege.scale_stability),
            name(&candidate.privilege.shared_failure),
        );
    }

    // Several maximal candidates: FRAMEWORK.md's tier 1 is the nondominated
    // set, and all of it remains a valid strategic choice.
    let maximal: Vec<&field_game_core::slate::Candidate> =
        slate.candidates.iter().filter(|held| held.tier == 1).collect();
    assert!(
        maximal.len() >= 2,
        "the chapter's Field leaves more than one maximal View: {} in tier 1",
        maximal.len(),
    );

    // Incomparable, and not identical either. FRAMEWORK.md's own reading:
    // candidates level on every shared assigned value, or each better than the
    // other somewhere, both stand — and a value unassigned for either is
    // excluded from their comparison. What is asserted here is that the
    // maximal set holds Views that differ in what they hold and in what can be
    // read of them, and that the ranking puts none of them above another.
    let mut differing = None;
    for first in &maximal {
        for second in &maximal {
            if std::ptr::eq(*first, *second) || first.view.inside == second.view.inside {
                continue;
            }
            let a = first.privilege.each();
            let b = second.privilege.each();
            let apart = a.iter().zip(b.iter()).any(|(one, other)| {
                one.is_assigned() != other.is_assigned() || one.value != other.value
            });
            if apart {
                differing = Some((first.view.inside.clone(), second.view.inside.clone()));
            }
        }
    }
    let (one, other) = differing.expect(
        "two maximal Views hold different Nodes and read differently",
    );
    println!("the choice stands between {one:?} and {other:?}");

    // And the tiers carry content. Reading `!dominates` across tier 1 says
    // nothing — tier 1 *is* the nondominated set, so the reading is the filter
    // restated — and the assertion that means something is the one below it:
    // every candidate the ranking put lower is dominated by one standing above
    // it. That is what makes tier 1 a choice between incomparable Views rather
    // than a listing of everything the slate assembled.
    let lower: Vec<&field_game_core::slate::Candidate> =
        slate.candidates.iter().filter(|held| held.tier > 1).collect();
    assert!(!lower.is_empty(), "the ranking separates something: {} below tier 1", lower.len());
    for held in &lower {
        assert!(
            slate.candidates.iter().any(|above| above.tier < held.tier
                && dominates(&above.privilege, &held.privilege, slate.tau)),
            "the candidate on {:?} stands in tier {} because something above it dominates it",
            held.view.inside,
            held.tier,
        );
    }

    // **Controls.** Which Forms the adopted View centres differ: the Nodes one
    // holds stand beside one Form of the chapter and the Nodes the other holds
    // beside another, so the run played under either is played through a
    // different Form — and moving control to it is the Handoff.
    let field = &run.state().now;
    let nearest = |inside: &[u32]| -> Vec<u8> {
        let mut held: Vec<u8> = Vec::new();
        for node in inside {
            let Some(port) = field.ports.iter().find(|port| port.node == *node) else {
                continue;
            };
            let closest = field
                .forms
                .iter()
                .min_by_key(|form| {
                    field_game_core::fx::distance(form.pos, form.layer, port.pos, port.layer)
                })
                .map(|form| form.id);
            if let Some(id) = closest {
                if !held.contains(&id) {
                    held.push(id);
                }
            }
        }
        held.sort_unstable();
        held
    };
    let (one_forms, other_forms) = (nearest(&one), nearest(&other));
    println!("  {one:?} is centred on Forms {one_forms:?}");
    println!("  {other:?} is centred on Forms {other_forms:?}");
    assert_ne!(
        one_forms, other_forms,
        "the two choices are tended through different Forms",
    );
    // The two readings themselves, pinned as an unordered pair so the search
    // above may report the differing candidates either way round: one choice
    // reaches both Forms of the challenge, the other only the one it stands
    // beside — and the Form it leaves out is the Form a Handoff moves to.
    let mut readings = [one_forms, other_forms];
    readings.sort();
    assert_eq!(
        readings,
        [vec![2u8], vec![2u8, 3u8]],
        "the wider choice is centred on the Form the narrower one leaves out",
    );

    // **Risk.** What each choice exposes: the Routes that cross its boundary
    // are what a severance would take, and the two Views cross different ones.
    let crossing = |inside: &[u32]| -> Vec<u32> {
        field
            .routes
            .iter()
            .filter(|route| inside.contains(&route.tail) != inside.contains(&route.head))
            .map(|route| route.route)
            .collect()
    };
    let (one_cross, other_cross) = (crossing(&one), crossing(&other));
    println!("  {one:?} crosses {one_cross:?}; {other:?} crosses {other_cross:?}");
    assert_ne!(one_cross, other_cross, "the two choices expose different Routes");

    // **The review.** The review surface reads the adopted View: the coordinate
    // profile is taken against the standing View and nothing else, so the two
    // choices are read differently by the surface that reports them.
    assert_ne!(
        profile_under(&run, &one),
        profile_under(&run, &other),
        "the review reads the adopted View",
    );

    // Nothing in the record names one correct: there is no scalar total, and
    // the slate carries no recommendation of any kind.
    let written = slate.written();
    for named in ["\"best\"", "\"recommended\"", "\"total\""] {
        assert!(!written.contains(named), "the record carries no {named} of any kind");
    }
    // The two collapsed-value words the lexicon refuses to see written are
    // assembled rather than spelled, so this file may check for what a record
    // must never carry without carrying it itself.
    for named in [
        format!("\"{}\"", "sc".to_string() + "ore"),
        format!("\"{}\"", "rat".to_string() + "ing"),
    ] {
        assert!(!written.contains(&named), "the record carries no {named} of any kind");
    }
}

/// The coordinate profile the review surface reads under one View.
///
/// It is the same function the `coordinates` inspection answers with, called on
/// the same standing state with the View swapped: the reading is taken against
/// the adopted View and against nothing else, which is what "the review reads
/// the adopted View" means.
fn profile_under(run: &field_game_core::run::Run, inside: &[u32]) -> String {
    let state = run.state();
    let view = field_game_core::state::ViewDeclaration {
        inside: inside.to_vec(),
        ..state.view.clone()
    };
    field_game_core::coord::of(state, &view, field_game_core::slate::TAU_DEFAULT).written()
}
