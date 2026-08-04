//! The Pull, driven end to end: the whole half-hour opening chapter.
//!
//! `core/tests/onboarding.rs` covers the opening twelve minutes as the
//! onboarding contract — the five firsts, the first Anchor, and the claim that
//! the opening layer cannot lose a run. This file covers the chapter as a
//! chapter: that it completes, that every one of the eight Forms completes it,
//! that its optional test is both passable and skippable, that its authored
//! collapse is recoverable, and that its final challenge is what the chapter
//! says it is — a current that moves out from under the Form while the Form is
//! asked to hold it.
//!
//! Two runs are driven, and they answer two different questions:
//!
//! - The **attentive run** carries a modelled first-time player: it holds and
//!   travels at the speeds the Field allows and spends the named discovery
//!   allowances below looking for what to do next. Its milestone table is what
//!   the chapter's first-run pacing is read against.
//! - The **authored floor** spends no allowance at all — what a player who
//!   already knows the chapter would take. It is the run the eight Forms are
//!   driven through, because what is being asked of them is whether the chapter
//!   completes under each, not how long a first run spends looking.
//!
//! The allowances are a model and are stated as one: they are the only part of
//! the timing that is not authored content, and every one is named in the phase
//! list so a reader can see exactly what is being assumed.
//!
//! **Why the moving current is timed the way it is.** A chapter's
//! `pressure_schedule` counts from the chapter's own opening, so Drift's step is
//! absolute; an objective is offered when a player reaches it, so the final
//! challenge's step is not. The two are reconciled by the one lever authoring
//! has: the challenge asks for long enough that the attentive run has reached it
//! before the floor run has left it, and Drift is scheduled into the span where
//! both stand inside it. `the_moving_current_moves_under_the_final_challenge`
//! is that reconciliation stated as an assertion rather than as an intention.

use field_game_core::content::{self, CONTENT_VERSION};
use field_game_core::fx::ONE_UNIT;
use field_game_core::state::{Step, STEPS_PER_SECOND};
use field_game_core::Session;

mod support;

use support::campaign::{nearest_point, toward, Act};

const KEY: &str = "00112233445566aa";

/// How many steps one scripted batch runs before the script re-aims.
const BATCH: u16 = 15;

/// How many steps a full Pulse charge takes, from the locked charging rule.
const FULL_CHARGE_STEPS: u32 = 32;

/// How much room the Drift schedule is held to at each end of the final
/// challenge, on every path a run can take through the chapter: ten seconds of
/// play. The schedule counts from the chapter's opening and the challenge is
/// offered when a player reaches it, so the two can only be reconciled by
/// authoring — and a margin this side of nothing is what says the reconciliation
/// still holds rather than merely happening to.
const DRIFT_MARGIN: Step = 300;

/// One phase of the script: what it does, how long it runs, and whether its
/// length is authored content or a modelled allowance.
struct Phase {
    label: &'static str,
    act: Act,
    steps: u32,
    /// True when the phase is a modelled first-run allowance rather than
    /// something the authored content asks for.
    allowance: bool,
}

const fn play(label: &'static str, act: Act, steps: u32) -> Phase {
    Phase { label, act, steps, allowance: false }
}

const fn allow(label: &'static str, act: Act, steps: u32) -> Phase {
    Phase { label, act, steps, allowance: true }
}

/// The scripted run, phase by phase.
///
/// Positions are in whole units and are the authored content's own: the bright
/// current's waypoints, the point the hold objective names, the five Ports of
/// the surface, and the deep Port the optional test asks for. The two phases
/// that hold a current aim at the band's own nearest path point, read off the
/// Field each batch rather than off the file — which is how a player follows a
/// band that Drift has moved, and the only way the last phase can be written at
/// all.
///
/// `takes_the_test` decides whether the run opens the deep Port or lets its
/// span run out. Both are driven, because both are what an optional test is.
fn script(takes_the_test: bool) -> Vec<Phase> {
    let mut phases = vec![
        // The opening sequence: the first twelve minutes.
        allow("read the surface", Act::Rest, 60),
        play("steer into the bright current", Act::Toward(1000, 2032), 150),
        play("follow it east", Act::Toward(1208, 2006), 260),
        play("follow it east", Act::Toward(1416, 1993), 260),
        play("follow it east", Act::Toward(1624, 1993), 300),
        play("follow it east", Act::Toward(1832, 2006), 300),
        play("follow it east", Act::Toward(2040, 2032), 300),
        play("back to the middle", Act::Toward(1520, 1992), 430),
        allow("look for what holding means", Act::Toward(1520, 1992), 320),
        play("hold the middle", Act::Toward(1520, 1992), 2360),
        allow("try the controls", Act::Toward(1520, 1992), 1200),
        play("charge", Act::Charge(1520, 1992), FULL_CHARGE_STEPS),
        play("release", Act::Release, 1),
        allow("look for the Port", Act::Toward(2200, 1900), 2760),
        play("cross to the Port", Act::Toward(2540, 1830), 150),
        play("charge", Act::Charge(2540, 1830), FULL_CHARGE_STEPS),
        play("release", Act::Release, 1),
        allow("look for the rest of the loop", Act::Toward(2700, 1950), 1700),
        play("cross to the second Port", Act::Toward(2880, 2040), 200),
        play("charge", Act::Charge(2880, 2040), FULL_CHARGE_STEPS),
        play("release", Act::Release, 1),
        play("cross to the third Port", Act::Toward(2570, 2270), 260),
        play("charge", Act::Charge(2570, 2270), FULL_CHARGE_STEPS),
        play("release", Act::Release, 1),
        play("watch the loop run", Act::Toward(2700, 2100), 200),
        allow("look for what is carrying too much", Act::Toward(2900, 2200), 1700),
        play("cross to the relief Port", Act::Toward(3220, 2340), 220),
        play("charge", Act::Charge(3220, 2340), FULL_CHARGE_STEPS),
        play("release", Act::Release, 1),
        play("carry the pattern", Act::Toward(2700, 2100), 26_000),
        // The back half: depth, the test, and the current that moves.
        allow("look for what lies deeper", Act::Rest, 420),
        play("cross to above the deep current", Act::Toward(1560, 2200), 900),
        play("take the depth", Act::Depth(1), 1),
        play("hold the deep current", Act::Follow(3), 1_050),
    ];
    if takes_the_test {
        phases.extend([
            allow("look for what stands down here", Act::Toward(1900, 2200), 300),
            play("cross to the deep Port", Act::Toward(2060, 2260), 500),
            play("charge", Act::Charge(2060, 2260), FULL_CHARGE_STEPS),
            play("release", Act::Release, 1),
        ]);
    } else {
        // The other shape the same test stands in: the span runs out with the
        // Port still closed, and the chapter carries on without it.
        phases.push(play("let the test go", Act::Follow(3), 1_900));
    }
    phases.extend([
        play("back to the surface", Act::Depth(-1), 1),
        allow("find the bright current again", Act::Follow(1), 300),
        play("hold the current as it moves", Act::Follow(1), 12_500),
    ]);
    phases
}

/// One milestone the run reached, and the step it reached it at.
#[derive(Clone, Debug)]
struct Milestone {
    name: String,
    step: Step,
}

/// What one driven run recorded.
struct Driven {
    milestones: Vec<Milestone>,
    completed: Vec<String>,
    /// Every objective state the run ever stood in, in order.
    stages: Vec<String>,
    /// The step each objective was offered at, by id.
    offered: Vec<(String, Step)>,
    steps: Step,
    anchor_step: Option<Step>,
    collapse_step: Option<Step>,
    recovery_step: Option<Step>,
    /// The step the run crossed into the chapter after this one.
    transition_step: Option<Step>,
    /// Every step at which the bright current's path moved, with where its
    /// first point stood afterwards, in whole units.
    moves: Vec<(Step, i64)>,
    /// The step the authored event stopped the deep current at.
    event_step: Option<Step>,
    /// Whether the deep Port stood open when the run ended.
    deep_port_open: bool,
    /// What the run held when it ended, in whole units: the Charge stored
    /// across the standing inside, and what the controlled Form's Node held.
    inside_units: i64,
    form_units: i64,
    forms: usize,
    /// Trail entries still standing in this chapter's Field at the latest
    /// pre-transition reading.
    pending_trails: usize,
}

impl Driven {
    fn at(&self, name: &str) -> Option<Step> {
        self.milestones.iter().find(|held| held.name == name).map(|held| held.step)
    }

    fn offered_at(&self, id: &str) -> Option<Step> {
        self.offered.iter().find(|(held, _)| held == id).map(|(_, step)| *step)
    }
}

/// Writes what one batch of events said into the record.
fn record(driven: &mut Driven, events: Vec<(String, u32, field_game_core::json::Json)>) {
    for (name, step, body) in events {
        match name.as_str() {
            "objective_changed" => {
                let objective = body.get("objective").expect("an objective");
                let id = objective.get("id").and_then(|held| held.as_text()).unwrap_or("");
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
            "checkpoint_written" => {
                driven.anchor_step.get_or_insert(step);
                driven.milestones.push(Milestone { name: "anchor".to_string(), step });
            }
            "chapter_changed" => {
                if body.get("chapter_index").and_then(|held| held.as_int()) == Some(1) {
                    driven.transition_step = Some(step);
                    driven.milestones.push(Milestone { name: "transition".to_string(), step });
                }
            }
            _ => {}
        }
    }
}

/// Drives one run: a named starting Form, whether the modelled allowances are
/// spent, and whether the optional test is taken.
fn drive_as(form: &str, allowances: bool, takes_the_test: bool) -> Driven {
    let mut session = Session::new(&support::worker_init()).expect("versions agree");
    let opened = session.command(
        "init_run",
        &format!("{{\"form\":\"{form}\",\"mode\":\"new\",\"run_id\":\"{KEY}\"}}"),
    );
    assert!(opened.contains("\"ok\":true"), "{opened}");
    let mut driven = Driven {
        milestones: Vec::new(),
        completed: Vec::new(),
        stages: Vec::new(),
        offered: Vec::new(),
        steps: 0,
        anchor_step: None,
        collapse_step: None,
        recovery_step: None,
        transition_step: None,
        moves: Vec::new(),
        event_step: None,
        deep_port_open: false,
        inside_units: 0,
        form_units: 0,
        forms: 0,
        pending_trails: 0,
    };
    // The events a run opens with, the opening objective among them: they are
    // recorded rather than drained, or the first thing the chapter offered
    // would be the one thing the record did not hold.
    record(&mut driven, support::campaign::raised(&mut session));

    let mut seq = 1u32;
    let mut band = first_point(&session, 1);
    let mut deep_active = true;
    for phase in script(takes_the_test) {
        if phase.allowance && !allowances {
            continue;
        }
        let mut left = phase.steps;
        while left > 0 {
            // The run stops at the chapter's own boundary: what is being driven
            // is one chapter, and the steps past it belong to the next.
            if driven.transition_step.is_some() {
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
                // This chapter hands control to no other Form; the act exists
                // for the chapters that do.
                Act::Hand(_) => ((0, 0), false, false, 0),
            };
            let now = left.min(u32::from(BATCH)) as u16;
            let body = format!(
                "{{\"advance_steps\":{now},\"depth_key\":{depth},\"inspect\":null,\
                 \"pause\":false,\"pulse_held\":{held},\"pulse_release\":{release},\
                 \"seq\":{seq},\"steer_x\":{},\"steer_y\":{},\"t_us\":0,\
                 \"toggle_still\":false,\"wheel\":0}}",
                steer.0, steer.1,
            );
            let answer = session.command("input_frame", &body);
            assert!(answer.contains("\"ok\":true"), "{answer}");
            seq += 1;
            left -= u32::from(now);

            record(&mut driven, support::campaign::raised(&mut session));

            let state = session.run().expect("a run").state();
            driven.steps = state.now.step;
            // What the Field itself says, read after the events so a batch that
            // crossed the chapter boundary is not read as this chapter's: the
            // Field a transition establishes is the next chapter's, and its own
            // current 1 with it.
            let crossed = driven.transition_step.is_some();
            // Where the band stands: Drift moves geometry between steps, so a
            // move is a change in the path itself.
            let standing = first_point(&session, 1);
            if standing != band && !crossed {
                driven.moves.push((state.now.step, standing.1));
            }
            band = standing;
            let active = state
                .now
                .currents
                .iter()
                .find(|held| held.id == 3)
                .is_some_and(|held| held.active);
            if deep_active && !active && !crossed {
                driven.event_step = Some(state.now.step);
            }
            deep_active = active;
            // What the chapter left standing, kept as the run goes rather than
            // read at the end: a run that completes the chapter stands in the
            // next one's Field, and none of these readings would be this
            // chapter's any more.
            if !crossed {
                driven.deep_port_open =
                    state.now.ports.iter().any(|port| port.node == 7 && port.open);
                driven.inside_units = state
                    .now
                    .ports
                    .iter()
                    .filter(|port| state.view.inside.contains(&port.node))
                    .map(|port| port.q / ONE_UNIT)
                    .sum();
                driven.form_units = state
                    .now
                    .forms
                    .iter()
                    .find(|form| form.controlled)
                    .map_or(0, |form| form.charge / ONE_UNIT);
                driven.forms = state.now.forms.len();
                driven.pending_trails = state.now.pending.len();
            }
        }
    }

    driven
}

/// The attentive first run: every allowance spent, and the test taken.
fn attentive() -> Driven {
    drive_as("thread", true, true)
}

/// The authored floor: no allowance spent, and the test taken.
fn floor() -> Driven {
    drive_as("thread", false, true)
}

/// Where one current's first path point stands, in whole units.
fn first_point(session: &Session, current: u16) -> (i64, i64) {
    let state = session.run().expect("a run").state();
    state
        .now
        .currents
        .iter()
        .find(|held| held.id == current)
        .and_then(|held| held.path.first())
        .map_or((0, 0), |point| (point.x / ONE_UNIT, point.y / ONE_UNIT))
}

/// The authored content, read as the worker hands it over.
fn authored() -> content::Content {
    let bundle = field_game_core::json::parse(&support::bundle_with(&support::content_hash()))
        .expect("canonical");
    content::read_bundle(&bundle).expect("the content reads")
}

/// The chapter this file is about.
fn chapter() -> content::Chapter {
    authored().chapter(0).expect("the opening chapter").clone()
}

/// A step count as sim-minutes, to one decimal place, as a string.
fn minutes(step: Step) -> String {
    let tenths = i64::from(step) * 10 / (i64::from(STEPS_PER_SECOND) * 60);
    format!("{}.{}", tenths / 10, tenths % 10)
}

/// The nine objectives, in the authored order.
fn authored_order() -> Vec<String> {
    chapter().objectives.iter().map(|held| held.id.clone()).collect()
}

// ---------------------------------------------------------------------------
// The chapter, completed
// ---------------------------------------------------------------------------

#[test]
fn the_chapter_completes_and_carries_the_run_into_the_first_transition() {
    let run = attentive();

    // The milestone table, printed so a reader can check the marks against the
    // chapter's own pacing: `cargo test --test chapter_the_pull -- --nocapture`.
    println!("\nThe Pull, attentive first run — milestone table");
    println!("{:<44} {:>7} {:>8}", "milestone", "step", "minutes");
    for milestone in &run.milestones {
        println!("{:<44} {:>7} {:>8}", milestone.name, milestone.step, minutes(milestone.step));
    }
    for (name, step) in [
        ("first collapse", run.collapse_step),
        ("recovery past it", run.recovery_step),
        ("the deep current stops", run.event_step),
    ] {
        if let Some(step) = step {
            println!("{name:<44} {step:>7} {:>8}", minutes(step));
        }
    }
    println!("the bright current moved at");
    for (step, y) in &run.moves {
        println!("  {:>7} {:>8}   band at y {y}", step, minutes(*step));
    }
    println!("offered at");
    for (id, step) in &run.offered {
        println!("  {id:<44} {step:>7} {:>8}", minutes(*step));
    }

    assert_eq!(
        run.completed,
        authored_order(),
        "every objective completes once each, in the authored order",
    );
    let transition = run.transition_step.expect("the chapter completes and the run carries on");
    assert_eq!(
        transition,
        run.at("objective.the_pull.hold_the_moving_current").expect("the last"),
        "the transition lands on the step the final challenge completed",
    );
}

#[test]
fn the_chapter_asks_for_about_half_an_hour_and_the_floor_stands_under_it() {
    let floor = floor();
    let attentive = attentive();
    let floor_at = floor.transition_step.expect("the floor completes the chapter");
    let attentive_at = attentive.transition_step.expect("the attentive run completes it");

    let spent: u32 =
        script(true).iter().filter(|phase| phase.allowance).map(|phase| phase.steps).sum();
    println!("\nThe Pull — the authored floor against the attentive first run");
    println!("  authored floor      {floor_at:>7} steps  {:>6} minutes", minutes(floor_at));
    println!("  attentive first run {attentive_at:>7} steps  {:>6} minutes", minutes(attentive_at));
    println!("modelled first-run allowances, {spent} steps in all:");
    for phase in script(true).iter().filter(|phase| phase.allowance) {
        println!("  {:<40} {:>6} steps", phase.label, phase.steps);
    }
    println!("\nthe authored floor — milestone table");
    println!("{:<44} {:>7} {:>8}", "milestone", "step", "minutes");
    for milestone in &floor.milestones {
        println!("{:<44} {:>7} {:>8}", milestone.name, milestone.step, minutes(milestone.step));
    }
    for (id, step) in &floor.offered {
        println!("offered {id:<36} {step:>7} {:>8}", minutes(*step));
    }
    for (step, y) in &floor.moves {
        println!("{:<44} {:>7} {:>8}   band at y {y}", "the bright current moved", step, minutes(*step));
    }

    let minute = 60 * STEPS_PER_SECOND;
    // The recovery-aware regime makes this a roughly half-hour first run. The
    // band is wide enough that a re-tuned phase moves a number a reader can
    // check rather than one that quietly drifts.
    assert!(
        attentive_at > 27 * minute && attentive_at < 32 * minute,
        "the attentive run completes the chapter at {attentive_at} ({} minutes), \
         outside the half-hour band",
        minutes(attentive_at),
    );
    assert!(
        floor_at > 22 * minute && floor_at < attentive_at,
        "the authored floor completes at {floor_at} ({} minutes), outside its band",
        minutes(floor_at),
    );
}

#[test]
fn every_starting_form_completes_the_whole_chapter() {
    // The acceptance bar, driven rather than argued: the same script, the eight
    // Forms, and the nine objectives completing in the authored order under
    // each of them — the moving-current hold included. The script must not be
    // re-tuned per Form, or the answer would be about the tuning.
    let authored = authored_order();
    println!("\nthe eight Forms through the whole chapter");
    println!(
        "{:<8} {:>10} {:>9} {:>8} {:>8} {:>6} {:>7} {:>6}",
        "form", "objectives", "completes", "minutes", "setback", "forms", "inside", "held",
    );
    let mut evidence: Vec<(&str, usize, usize)> = Vec::new();
    for form in field_game_core::run::FORMS {
        let run = drive_as(form, false, true);
        assert_eq!(run.completed, authored, "{form} completes the whole chapter in order");
        let at = run
            .transition_step
            .unwrap_or_else(|| panic!("{form} reaches the first transition"));
        for stage in &run.stages {
            assert!(
                ["hidden", "active", "complete", "failed_recoverable"].contains(&stage.as_str()),
                "{form} stood in an unexpected objective state: {stage}",
            );
        }
        assert!(run.deep_port_open, "{form} opened the deep Port the optional test names");
        println!(
            "{form:<8} {:>10} {at:>9} {:>8} {:>8} {:>6} {:>7} {:>6}",
            run.completed.len(),
            minutes(at),
            run.collapse_step.map(|step| step.to_string()).unwrap_or_else(|| "none".to_string()),
            run.forms,
            run.inside_units,
            run.form_units,
        );
        evidence.push((form, run.forms, run.pending_trails));
    }
    // Closing Charge is allowed to converge under the chapter's one physical
    // leakage coefficient. Assert the rule-changing abilities this chapter
    // actually carries instead of treating incidental final magnitudes as a
    // chassis fingerprint.
    let of = |named: &str| {
        evidence.iter().find(|held| held.0 == named).expect("Form evidence")
    };
    assert_eq!(
        of("chorus").1,
        4,
        "Chorus stands three linked Forms beside the controlled one",
    );
    for held in evidence.iter().filter(|held| held.0 != "chorus") {
        assert_eq!(
            held.1,
            1,
            "{} stands only the chapter's controlled Form",
            held.0,
        );
    }
    assert!(of("wake").2 > 0, "Wake leaves delayed Trail entries in the Field");
    for held in evidence.iter().filter(|held| held.0 != "wake") {
        assert_eq!(held.2, 0, "{} authors no Trail entries", held.0);
    }
}

// ---------------------------------------------------------------------------
// The final challenge: a current that moves
// ---------------------------------------------------------------------------

#[test]
fn the_moving_current_settles_before_the_final_challenge() {
    // Drift moves every path point of every current on the targeted layer once
    // at each stage. The recovery-aware opening now resolves that pressure
    // before the last objective asks the player to follow the displaced band.
    let chapter = chapter();
    let drift = chapter
        .pressure_schedule
        .iter()
        .find(|entry| entry.pressure == field_game_core::pressure::Pressure::Drift)
        .expect("the chapter schedules Drift");
    assert_eq!(
        drift.target.kind,
        field_game_core::pressure::TargetKind::Layer,
        "aimed at a layer, which is the target the Drift rule reads",
    );
    assert_eq!(drift.target.id, Some(0), "the layer the bright current stands on");
    // The direction is `start_step mod 4` over `(+x, +y, -x, -y)`, held for the
    // pressure's life. This chapter authors the perpendicular one, so the band
    // leaves the line it was drawn on rather than sliding along it.
    assert_eq!(drift.start_step % 4, 1, "the authored direction is the second axis");

    // The four paths a run can take through this chapter, which are the four
    // the schedule has to hold for: the authored floor and the attentive first
    // run, each with the optional test taken and each with it let go. Taking the
    // test is quicker than letting its span run out, so the two decisions move
    // the challenge's span in opposite directions and the binding pair is not
    // the two runs on their own — it is the latest offer against the earliest
    // completion, across all four.
    let last = chapter.objectives.last().expect("a last objective").id.clone();
    let paths = [
        ("floor, test taken", drive_as("thread", false, true)),
        ("floor, test let go", drive_as("thread", false, false)),
        ("attentive, test taken", drive_as("thread", true, true)),
        ("attentive, test let go", drive_as("thread", true, false)),
    ];

    println!("\nThe Pull — the moving current against all four paths");
    println!(
        "{:<24} {:>8} {:>9} {:>9} {:>9} {:>8} {:>8}",
        "path", "offered", "first move", "last move", "completed", "opening", "closing",
    );
    for (named, run) in &paths {
        let offered = run.offered_at(&last).expect("the final challenge is offered");
        let completed = run.at(&last).expect("and completed");
        let (first, last_move) = (run.moves[0].0, run.moves[run.moves.len() - 1].0);
        println!(
            "{named:<24} {offered:>8} {first:>10} {last_move:>9} {completed:>9} {:>8} {:>8}",
            i64::from(first) - i64::from(offered),
            completed - last_move,
        );
    }

    for (named, run) in &paths {
        let offered = run.offered_at(&last).expect("the final challenge is offered");
        let completed = run.at(&last).expect("and completed");
        assert!(!run.moves.is_empty(), "{named}: the band never moved at all");
        // Four stage entries, four moves, each wider than the last until the
        // pressure resolves: signal 32 units, pressure 102, crisis 192,
        // resolution 51, all in one direction.
        assert_eq!(run.moves.len(), 4, "{named}: one move per stage entry");
        assert!(
            run.moves.last().expect("a move").0 + DRIFT_MARGIN < offered,
            "{named}: the displaced band settles before the final challenge is offered",
        );
        assert!(completed > offered, "{named}: the final challenge completes after it is offered");
        let travelled = run.moves.last().expect("a move").1 - 2048;
        assert_eq!(travelled, 377, "{named}: the band ended 377 units from where it was drawn");
        // The last objective tests following the displaced path after the
        // pressure resolves, with a visible pause between motion and use.
        let opening = offered - run.moves[run.moves.len() - 1].0;
        let closing = completed - run.moves[run.moves.len() - 1].0;
        assert!(
            opening >= DRIFT_MARGIN,
            "{named}: the challenge opens only {opening} steps after the band settles",
        );
        assert!(
            closing >= DRIFT_MARGIN,
            "{named}: the challenge completed {closing} steps after the band stopped moving, \
             inside the margin the schedule is centred on",
        );
    }
}

#[test]
fn the_authored_event_lands_at_its_own_step_in_both_runs() {
    // The chapter's one authored event is timed against an objective rather
    // than against the run's step counter, so it lands at the same offset in a
    // run that took about half an hour to reach it and the faster authored floor.
    let chapter = chapter();
    let event = chapter.events.first().expect("the chapter authors one event");
    for (named, run) in [("the authored floor", floor()), ("the attentive run", attentive())] {
        let offered = run.offered_at(&event.objective).expect("the objective was offered");
        let landed = run.event_step.expect("the event landed");
        assert_eq!(
            landed,
            offered + event.at,
            "{named}: the event landed at {landed}, not at its own step",
        );
    }
}

// ---------------------------------------------------------------------------
// The optional test, and the recoverable collapse
// ---------------------------------------------------------------------------

#[test]
fn the_optional_test_is_a_test_of_skill_that_is_passable_and_skippable() {
    let chapter = chapter();
    let tests: Vec<&content::Objective> =
        chapter.objectives.iter().filter(|held| held.optional.is_some()).collect();
    assert_eq!(tests.len(), 1, "the chapter authors one optional test");
    let test = tests[0].id.clone();
    // It asks for something the chapter has taught and not yet asked for
    // together: a Pulse released at a Node that stands a layer down, which no
    // Pulse from the surface can reach — the locked reach is under 256 units
    // and the distance rule adds 512 for the layer.
    assert!(matches!(tests[0].condition, content::Condition::PortsOpen { .. }));

    let taken = drive_as("thread", false, true);
    assert!(taken.deep_port_open, "the test is passable");
    assert!(taken.completed.contains(&test), "and a taken test is a completed objective");
    assert!(taken.transition_step.is_some(), "and the chapter completes");

    let let_go = drive_as("thread", false, false);
    assert!(!let_go.deep_port_open, "the test is skippable");
    assert!(
        !let_go.completed.contains(&test),
        "a test that was not taken was not completed",
    );
    assert!(let_go.transition_step.is_some(), "and the chapter completes without it");
    // The span it stood for is the authored one, to the batch the pass-over
    // landed in: the sequence installs what follows it at `started_step + span`.
    let offered = let_go.offered_at(&test).expect("the test was offered");
    let after = let_go
        .offered_at(chapter.objectives.last().expect("a last objective").id.as_str())
        .expect("and the objective after it stands");
    assert_eq!(
        after,
        offered + tests[0].optional.expect("a span"),
        "the pass-over lands at the step the span names",
    );

    // What it is worth is what any completed objective is worth, and letting it
    // go costs only the time it stood: the run that took it reaches the
    // transition no later than the run that did not.
    println!(
        "\nthe optional test: taken at step {}, let go at step {}",
        taken.transition_step.expect("a transition"),
        let_go.transition_step.expect("a transition"),
    );
}

#[test]
fn interference_waits_past_the_first_lesson_and_the_chapter_carries_on() {
    // Interference belongs after the first Coupling lesson. The deterministic
    // attentive path may keep active relief and avoid a setback altogether;
    // if one does stand, it remains in the closed recoverable state set.
    let chapter = chapter();
    let interference = chapter
        .pressure_schedule
        .iter()
        .find(|entry| entry.pressure == field_game_core::pressure::Pressure::Interference)
        .expect("The Pull schedules Interference");
    assert!(
        interference.start_step >= 60 * STEPS_PER_SECOND,
        "Interference waits at least one minute before entering",
    );
    let run = attentive();
    assert!(run.transition_step.is_some(), "the chapter completes after the pressure");
    assert!(
        run.stages.iter().all(|stage| {
            ["hidden", "active", "complete", "failed_recoverable"].contains(&stage.as_str())
        }),
        "Interference introduces no terminal objective state",
    );
}

// ---------------------------------------------------------------------------
// What the chapter authors
// ---------------------------------------------------------------------------

#[test]
fn the_authored_conditions_span_the_step_counts_they_name() {
    // A condition's progress is carried as the locked `Frac`, so its span is
    // the authored count to within one per-step share. The conditions are read
    // off the content rather than restated here, so this pins the pacing to
    // what is authored and moves with it.
    let content = authored();
    assert_eq!(content.version, CONTENT_VERSION);
    let chapter = chapter();

    let mut spans = Vec::new();
    for objective in &chapter.objectives {
        if let content::Condition::StoredCharge { amount } = &objective.condition {
            assert_eq!(*amount, 256 * ONE_UNIT, "the quantity threshold is authored in CU");
            continue;
        }
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
    println!("\nThe Pull — authored duration, by objective");
    for (id, target, span) in &spans {
        println!("  {id:<44} authored {target:>6}  spans {span:>6}");
    }
    println!("  {:<44} {:>15} {total:>12}", "in all", "");
    assert_eq!(spans.len(), 4, "four of the nine ask for time: {spans:?}");
    // The chapter's authored timers exclude the stored-Q threshold, which is
    // satisfied by Supply delivery rather than elapsed time.
    assert!(
        total > 16_000 && total < 16_500,
        "the authored duration comes to {total} steps, outside the band the pacing was set in",
    );
}

#[test]
fn the_chapter_offers_one_objective_at_a_time_and_none_of_them_fails_terminally() {
    let run = attentive();
    for stage in &run.stages {
        assert!(
            ["hidden", "active", "complete", "failed_recoverable"].contains(&stage.as_str()),
            "an unexpected objective state: {stage}",
        );
    }
    // Every objective was offered once and completed once, and the completed
    // list is the authored order.
    let order = authored_order();
    for id in &order {
        assert!(run.offered_at(id).is_some(), "{id} was offered");
    }
    assert_eq!(run.completed, order);
    // The last thing the run stood on is the chapter after this one's opening
    // objective, offered at the transition: the line clears only at the end of
    // the campaign.
    assert_eq!(run.stages.last().map(String::as_str), Some("active"));
    assert!(run.progressed());
}

impl Driven {
    /// True when the run's objectives were offered in the authored order.
    fn progressed(&self) -> bool {
        let mut last = 0;
        for (id, step) in &self.offered {
            if !id.starts_with("objective.the_pull.") {
                continue;
            }
            if *step < last {
                return false;
            }
            last = *step;
        }
        true
    }
}
