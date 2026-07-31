//! The authored opening sequence, driven end to end.
//!
//! The run below is scripted rather than played: every frame carries a control
//! state a test chose, and the whole of it runs through `advance_steps`, which
//! is the locked hook for asking the closed command set for an exact step
//! count. What it proves is that the authored content completes — that each of
//! the six objectives is reachable in the authored order, that the opening
//! layer produces no terminal failure, and that the first Anchor lands where
//! the sequence says it does.
//!
//! Two runs are driven, and they answer two different questions:
//!
//! - The **attentive run** carries a modelled first-time player: it holds and
//!   travels at the speeds the Field allows, and it spends the named discovery
//!   allowances below looking for what to do next. Its milestone table is what
//!   the twelve-minute mark is read against.
//! - The **direct run** spends no allowance at all. It is the authored floor —
//!   what a player who already knows the sequence would take — and it stands
//!   here so the two figures are never confused for one another.
//!
//! The allowances are a model and are stated as one: they are the only part of
//! the timing that is not authored content, and every one of them is named in
//! the phase list so a reader can see exactly what is being assumed.

use field_game_core::content::CONTENT_VERSION;
use field_game_core::fx::{isqrt, Vec2, ONE_UNIT};
use field_game_core::state::{ObjectiveStage, Step, STEPS_PER_SECOND};
use field_game_core::Session;

mod support;

const KEY: &str = "00112233445566aa";

/// The reach a fully deflected control names, in units: the shell's own
/// normalization, so a scripted frame carries what a pointer would.
const STEER_REACH_UNITS: i64 = 320;

/// The largest per-axis value of the normalized Q1.15 control.
const STEER_LIMIT: i64 = 32_767;

/// How many steps one scripted batch runs before the script re-aims.
const BATCH: u16 = 15;

/// How many steps a full Pulse charge takes, from the locked charging rule.
const FULL_CHARGE_STEPS: u16 = 32;

/// What one scripted phase asks of the run.
#[derive(Clone, Copy, Debug)]
enum Act {
    /// Steer toward a point, at the speed the distance to it names.
    Toward(i64, i64),
    /// Hold every input neutral.
    Rest,
    /// Hold the Pulse, steering toward a point.
    Charge(i64, i64),
    /// Release the Pulse.
    Release,
}

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

/// The scripted attentive run, phase by phase.
///
/// Positions are in whole units on the layer-0 plane and are the authored
/// content's own: the bright current's waypoints, the hold point the second
/// objective names, and the four Ports.
fn script() -> Vec<Phase> {
    vec![
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
        play("charge", Act::Charge(1520, 1992), FULL_CHARGE_STEPS as u32),
        play("release", Act::Release, 1),
        allow("look for the Port", Act::Toward(2200, 1900), 2760),
        play("cross to the Port", Act::Toward(2540, 1830), 150),
        play("charge", Act::Charge(2540, 1830), FULL_CHARGE_STEPS as u32),
        play("release", Act::Release, 1),
        allow("look for the rest of the loop", Act::Toward(2700, 1950), 1700),
        play("cross to the second Port", Act::Toward(2880, 2040), 200),
        play("charge", Act::Charge(2880, 2040), FULL_CHARGE_STEPS as u32),
        play("release", Act::Release, 1),
        play("cross to the third Port", Act::Toward(2570, 2270), 260),
        play("charge", Act::Charge(2570, 2270), FULL_CHARGE_STEPS as u32),
        play("release", Act::Release, 1),
        play("watch the loop run", Act::Toward(2700, 2100), 200),
        allow("look for what is carrying too much", Act::Toward(2900, 2200), 1700),
        play("cross to the relief Port", Act::Toward(3220, 2340), 220),
        play("charge", Act::Charge(3220, 2340), FULL_CHARGE_STEPS as u32),
        play("release", Act::Release, 1),
        play("carry the pattern", Act::Toward(2700, 2100), 7500),
    ]
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
    steps: Step,
    /// The step the first frame carrying something the player did ran to.
    first_input_step: Option<Step>,
    anchor_step: Option<Step>,
    collapse_step: Option<Step>,
    first_route_step: Option<Step>,
    first_pulse_step: Option<Step>,
    completed: Vec<String>,
    /// Every objective state the run ever stood in, so a terminal failure would
    /// be visible if one existed.
    stages: Vec<String>,
    /// What the run held when it ended, in whole units: the Charge stored
    /// across the standing inside, and the Charge the controlled Form's own
    /// Node stood with. The two readings the authored Form parameters move.
    inside_units: i64,
    form_units: i64,
    /// How many Forms stood in the Field.
    forms: usize,
}

impl Driven {
    fn at(&self, name: &str) -> Option<Step> {
        self.milestones.iter().find(|held| held.name == name).map(|held| held.step)
    }
}

/// Drives one run on the opening Form. `allowances` decides whether the
/// modelled first-run allowances are spent or skipped.
fn drive(allowances: bool) -> Driven {
    drive_as("thread", allowances)
}

/// Drives one run on a named starting Form.
///
/// The script is the same script for every Form: what is being asked is
/// whether the authored sequence completes under each one, so the play must
/// not be re-tuned per Form or the answer would be about the tuning.
fn drive_as(form: &str, allowances: bool) -> Driven {
    let mut session = Session::new(&support::worker_init()).expect("versions agree");
    let opened = session.command(
        "init_run",
        &format!("{{\"form\":\"{form}\",\"mode\":\"new\",\"run_id\":\"{KEY}\"}}"),
    );
    assert!(opened.contains("\"ok\":true"), "{opened}");
    // The events a run opens with: the chapter, and the objective it stands on.
    let opening = events(&mut session);
    assert!(opening.iter().any(|(name, _, _)| name == "chapter_changed"));

    let mut driven = Driven {
        milestones: Vec::new(),
        steps: 0,
        first_input_step: None,
        anchor_step: None,
        collapse_step: None,
        first_route_step: None,
        first_pulse_step: None,
        completed: Vec::new(),
        stages: Vec::new(),
        inside_units: 0,
        form_units: 0,
        forms: 0,
    };
    let mut seq = 1u32;

    for phase in script() {
        if phase.allowance && !allowances {
            continue;
        }
        let mut left = phase.steps;
        while left > 0 {
            let run = session.run().expect("a run is loaded");
            let form = run
                .state()
                .now
                .forms
                .iter()
                .find(|form| form.controlled)
                .expect("the controlled Form");
            let (steer, held, release) = match phase.act {
                Act::Toward(x, y) => (toward(form.pos, x, y), false, false),
                Act::Rest => ((0, 0), false, false),
                Act::Charge(x, y) => (toward(form.pos, x, y), true, false),
                Act::Release => ((0, 0), false, true),
            };
            let run_now = left.min(u32::from(BATCH)) as u16;
            let body = format!(
                "{{\"advance_steps\":{run_now},\"depth_key\":0,\"inspect\":null,\"pause\":false,\
                 \"pulse_held\":{held},\"pulse_release\":{release},\"seq\":{seq},\
                 \"steer_x\":{},\"steer_y\":{},\"t_us\":0,\"toggle_still\":false,\"wheel\":0}}",
                steer.0, steer.1,
            );
            let answer = session.command("input_frame", &body);
            assert!(answer.contains("\"ok\":true"), "{answer}");
            // The same reading the shell's own telemetry takes: the first frame
            // carrying anything the player did, rather than the first frame.
            if steer != (0, 0) || held || release {
                let step = session.run().expect("a run").step();
                driven.first_input_step.get_or_insert(step);
            }
            seq += 1;
            left -= u32::from(run_now);
            if release {
                let step = session.run().expect("a run").step();
                driven.first_pulse_step.get_or_insert(step);
            }

            let state = session.run().expect("a run").state();
            driven.steps = state.now.step;
            if driven.first_route_step.is_none()
                && state.now.routes.iter().any(|route| route.flow > 0)
            {
                driven.first_route_step = Some(state.now.step);
            }

            for (name, step, body) in events(&mut session) {
                match name.as_str() {
                    "objective_changed" => {
                        let objective = body.get("objective").expect("an objective");
                        let id = objective.get("id").and_then(|held| held.as_text()).unwrap_or("");
                        let stage =
                            objective.get("state").and_then(|held| held.as_text()).unwrap_or("");
                        driven.stages.push(stage.to_string());
                        if stage == "complete" {
                            driven.milestones.push(Milestone { name: id.to_string(), step });
                            driven.completed.push(id.to_string());
                        }
                        if stage == "failed_recoverable" && driven.collapse_step.is_none() {
                            driven.collapse_step = Some(step);
                        }
                    }
                    "checkpoint_written" => {
                        driven.anchor_step.get_or_insert(step);
                        driven.milestones.push(Milestone { name: "anchor".to_string(), step });
                    }
                    _ => {}
                }
            }
        }
    }
    let ended = session.run().expect("a run").state();
    // The chapter's own inside, read off the standing View rather than named
    // here, so the reading follows the content.
    driven.inside_units = ended
        .now
        .ports
        .iter()
        .filter(|port| ended.view.inside.contains(&port.node))
        .map(|port| port.q / ONE_UNIT)
        .sum();
    driven.form_units = ended
        .now
        .forms
        .iter()
        .find(|form| form.controlled)
        .map_or(0, |form| form.charge / ONE_UNIT);
    driven.forms = ended.now.forms.len();
    driven
}

/// The events one command raised, as (name, step, body).
fn events(session: &mut Session) -> Vec<(String, Step, field_game_core::json::Json)> {
    let raised = session.take_events();
    let parsed = field_game_core::json::parse(&raised).expect("canonical events");
    let field_game_core::json::Json::List(items) = parsed else {
        panic!("the events are a list: {raised}");
    };
    items
        .into_iter()
        .map(|item| {
            let name = item.get("ev").and_then(|held| held.as_text()).unwrap_or("").to_string();
            let step = item.get("step").and_then(|held| held.as_int()).unwrap_or(0) as Step;
            let body = item.get("body").cloned().unwrap_or(field_game_core::json::Json::Null);
            (name, step, body)
        })
        .collect()
}

/// The normalized control that steers a Form toward a point, in whole units.
///
/// It is the shell's own rule — the direction to the point, at the magnitude
/// `min(1, r / 320)` — so a scripted frame carries exactly what a pointer over
/// the surface would.
fn toward(from: Vec2, x: i64, y: i64) -> (i64, i64) {
    let dx = x * ONE_UNIT - from.x;
    let dy = y * ONE_UNIT - from.y;
    let distance = isqrt((i128::from(dx) * i128::from(dx) + i128::from(dy) * i128::from(dy)) as u128)
        as i64;
    if distance == 0 {
        return (0, 0);
    }
    let reach = STEER_REACH_UNITS * ONE_UNIT;
    let magnitude = (STEER_LIMIT * distance / reach).min(STEER_LIMIT);
    let mut steer_x = dx * magnitude / distance;
    let mut steer_y = dy * magnitude / distance;
    // The locked magnitude cap is normative, and the reader that enforces it
    // refuses a frame that crosses it by a single raw unit.
    while steer_x * steer_x + steer_y * steer_y > STEER_LIMIT * STEER_LIMIT {
        steer_x -= steer_x.signum();
        steer_y -= steer_y.signum();
    }
    (steer_x, steer_y)
}

/// A step count as sim-minutes, to one decimal place, as a string.
fn minutes(step: Step) -> String {
    let tenths = i64::from(step) * 10 / (i64::from(STEPS_PER_SECOND) * 60);
    format!("{}.{}", tenths / 10, tenths % 10)
}

#[test]
fn every_starting_form_completes_the_authored_sequence_and_reaches_its_anchor() {
    // The done-when of Form selection, driven rather than argued: the same
    // script, the eight Forms, and the six objectives completing in the
    // authored order under each of them. The runs are the authored floor —
    // no modelled allowance spent — because what is being asked is whether
    // the sequence completes at all under each Form, not how long a first-time
    // player spends looking for it.
    let authored: Vec<String> = vec![
        "objective.the_pull.follow_current".to_string(),
        "objective.the_pull.hold_center".to_string(),
        "objective.the_pull.release_pulse".to_string(),
        "objective.the_pull.open_port".to_string(),
        "objective.the_pull.close_loop".to_string(),
        "objective.the_pull.carry_pattern".to_string(),
    ];

    println!("\nthe eight Forms through the authored sequence");
    println!(
        "{:<8} {:>10} {:>7} {:>8} {:>8} {:>6} {:>7} {:>6}",
        "form", "objectives", "anchor", "minutes", "setback", "forms", "inside", "held",
    );
    let mut readings: Vec<(&str, i64, i64, usize)> = Vec::new();
    for form in field_game_core::run::FORMS {
        let run = drive_as(form, false);
        assert_eq!(run.completed, authored, "{form} completes the authored sequence in order");
        let anchor = run.anchor_step.unwrap_or_else(|| panic!("{form} reaches its first Anchor"));
        for stage in &run.stages {
            assert!(
                ["hidden", "active", "complete", "failed_recoverable"].contains(&stage.as_str()),
                "{form} stood in an unexpected objective state: {stage}",
            );
        }
        println!(
            "{form:<8} {:>10} {anchor:>7} {:>8} {:>8} {:>6} {:>7} {:>6}",
            run.completed.len(),
            minutes(anchor),
            run.collapse_step.map(|step| step.to_string()).unwrap_or_else(|| "none".to_string()),
            run.forms,
            run.inside_units,
            run.form_units,
        );
        readings.push((form, run.inside_units, run.form_units, run.forms));
    }

    // The same script, eight different Fields at the end of it: what a Form's
    // authored parameters are for. Every Form differs from every other in at
    // least one of the three readings, which is the distinctness claim stated
    // as an assertion rather than as a table a reader has to compare by eye.
    for (place, first) in readings.iter().enumerate() {
        for second in &readings[place + 1..] {
            assert!(
                (first.1, first.2, first.3) != (second.1, second.2, second.3),
                "{} and {} end the same run holding the same readings",
                first.0,
                second.0,
            );
        }
    }
}

#[test]
fn the_authored_sequence_completes_in_its_authored_order() {
    let run = drive(true);
    assert_eq!(
        run.completed,
        vec![
            "objective.the_pull.follow_current",
            "objective.the_pull.hold_center",
            "objective.the_pull.release_pulse",
            "objective.the_pull.open_port",
            "objective.the_pull.close_loop",
            "objective.the_pull.carry_pattern",
        ],
        "the six objectives complete once each, in the authored order",
    );
}

#[test]
fn one_objective_stands_at_a_time_and_none_of_them_fails_terminally() {
    let run = drive(true);
    // The closed state set has no terminal failure in it, and a run reaches
    // only the four states it declares: `hidden` before an objective is offered
    // and after the last one completes, and the three that carry on between.
    for stage in &run.stages {
        assert!(
            ["hidden", "active", "complete", "failed_recoverable"].contains(&stage.as_str()),
            "an unexpected objective state: {stage}",
        );
    }
    let setback = run
        .stages
        .iter()
        .rposition(|stage| stage == "failed_recoverable")
        .expect("the authored break is reached");
    // Recovery is a state after the last setback, not merely somewhere in the
    // run: what makes the break recoverable is that the sequence carried on
    // past the last time it stood.
    assert!(
        run.stages[setback + 1..].iter().any(|stage| stage == "active"),
        "the sequence stands active again after the last setback: {:?}",
        &run.stages[setback..],
    );
    assert!(
        run.stages[setback + 1..].iter().any(|stage| stage == "complete"),
        "and completes past it",
    );
    // One objective at a time, to the end of the opening sequence and past it:
    // the completion of the sixth objective hands over to the seventh, which is
    // where The Pull's back half begins. The line clears only where there is
    // nothing more to offer at all, which is the end of the campaign, so what
    // stands at the end of this script is an active objective rather than an
    // empty line.
    assert_eq!(run.stages.last().map(String::as_str), Some("active"));
    assert_eq!(
        run.stages.iter().rev().nth(1).map(String::as_str),
        Some("complete"),
        "and it stands because the objective before it completed",
    );
}

#[test]
fn the_first_anchor_lands_at_about_sim_minute_twelve() {
    let run = drive(true);
    let anchor = run.anchor_step.expect("the sequence writes its first Anchor");

    // The milestone table, printed so a reader can check the marks against the
    // spec's own: `cargo test -- --nocapture`.
    println!("\nattentive first run — milestone table");
    println!("{:<34} {:>7} {:>8}", "milestone", "step", "minutes");
    for milestone in &run.milestones {
        println!(
            "{:<34} {:>7} {:>8}",
            milestone.name,
            milestone.step,
            minutes(milestone.step),
        );
    }
    for (name, step) in [
        ("first input", run.first_input_step),
        ("first pulse", run.first_pulse_step),
        ("first route", run.first_route_step),
        ("first collapse", run.collapse_step),
        ("first anchor", run.anchor_step),
    ] {
        if let Some(step) = step {
            println!("{name:<34} {step:>7} {:>8}", minutes(step));
        }
    }

    let twelve = 12 * 60 * STEPS_PER_SECOND;
    let ten = 10 * 60 * STEPS_PER_SECOND;
    assert!(
        anchor > ten && anchor < twelve + twelve / 10,
        "the first Anchor lands at about sim-minute twelve, and landed at {anchor} \
         ({} minutes)",
        minutes(anchor),
    );
}

#[test]
fn the_onboarding_marks_land_where_the_specification_puts_them() {
    let run = drive(true);
    let minute = |count: u32| count * 60 * STEPS_PER_SECOND;

    // Movement works from the first frame: the first objective is offered at
    // step 0 and the control state of frame 1 moves the Form.
    assert!(
        run.at("objective.the_pull.follow_current").expect("the first objective completes") < minute(2),
        "the opening objective completes inside the first two minutes",
    );
    let route = run.first_route_step.expect("a Route carries Charge");
    assert!(route <= minute(5), "the first Route is completed by minute five, and was at {route}");
    let collapse = run.collapse_step.expect("the authored break stands");
    assert!(
        collapse <= minute(8),
        "the first recoverable collapse is by minute eight, and was at {collapse}",
    );
    assert!(
        run.anchor_step.expect("an Anchor") <= minute(12) + minute(1),
        "the first Anchor is by about minute twelve",
    );
}

#[test]
fn the_authored_floor_is_shorter_than_the_attentive_run() {
    // The same script with none of the modelled allowances spent: what the
    // authored content alone asks for, which is the floor the twelve minutes
    // sits above.
    let floor = drive(false);
    let attentive = drive(true);
    let anchor = floor.anchor_step.expect("the floor reaches its Anchor too");
    println!(
        "\nauthored floor: first Anchor at step {anchor} ({} minutes); \
         attentive run at step {} ({} minutes)",
        minutes(anchor),
        attentive.anchor_step.expect("an Anchor"),
        minutes(attentive.anchor_step.expect("an Anchor")),
    );
    let spent: u32 = script().iter().filter(|phase| phase.allowance).map(|phase| phase.steps).sum();
    println!("modelled first-run allowances, {spent} steps in all:");
    for phase in script().iter().filter(|phase| phase.allowance) {
        println!("  {:<40} {:>6} steps", phase.label, phase.steps);
    }
    assert!(
        anchor < attentive.anchor_step.expect("an Anchor"),
        "the allowances are the difference between the two",
    );
    assert_eq!(floor.completed.len(), 6, "and the floor completes the whole sequence");

    // The floor is banded the same way the attentive run is, so a change to the
    // authored durations moves a figure a reader can check rather than one that
    // quietly drifts. Below the band the sequence has stopped asking for time;
    // above it the allowances have stopped being what the twelve minutes rests
    // on.
    let minute = 60 * STEPS_PER_SECOND;
    assert!(
        anchor > 6 * minute && anchor < 9 * minute,
        "the authored floor reaches its Anchor at {anchor} ({} minutes), outside its band",
        minutes(anchor),
    );
}

#[test]
fn the_opening_sequences_conditions_span_the_step_counts_they_name() {
    // A condition's progress is carried as the locked `Frac`, so its span is
    // the authored count to within one per-step share. The conditions are read
    // off the content rather than restated here, so this pins the pacing to
    // what is authored and moves with it.
    //
    // The opening sequence is the part of the chapter this file is about: the
    // objectives up to and including the one the chapter names an Anchor
    // moment at, which is where the twelve minutes are read. What The Pull
    // authors past it belongs to `chapter_the_pull.rs`.
    let bundle =
        field_game_core::json::parse(&support::bundle_with(&support::content_hash()))
            .expect("canonical");
    let content = field_game_core::content::read_bundle(&bundle).expect("the content reads");
    assert_eq!(content.version, CONTENT_VERSION);
    let chapter = content.chapter(0).expect("the opening chapter");
    let closes = chapter.anchor_moments.first().expect("the chapter names an Anchor moment");
    let opening = chapter
        .objectives
        .iter()
        .position(|held| &held.id == closes)
        .expect("the Anchor moment names an authored objective")
        + 1;
    assert_eq!(opening, 6, "the opening sequence is six objectives long");

    let mut spans = Vec::new();
    for objective in chapter.objectives.iter().take(opening) {
        let target = objective.condition.target();
        let span = field_game_core::content::effective_steps(&objective.condition);
        // The span never overshoots the authored target and never falls more
        // than one share short of it, which is the whole of the rounding the
        // `Frac` carries.
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
    // The three that carry authored duration, and what they come to in steps.
    assert_eq!(spans.len(), 3, "three of the six ask for time: {spans:?}");
    let total: i64 = spans.iter().map(|(_, _, span)| span).sum();
    println!("\nauthored duration, by objective");
    for (id, target, span) in &spans {
        println!("  {id:<36} authored {target:>5}  spans {span:>5}");
    }
    println!("  {:<36} {:>14} {total:>11}", "in all", "");
    assert!(
        total > 9_000 && total < 14_000,
        "the authored duration comes to {total} steps, outside the band the pacing was set in",
    );
}

#[test]
fn objective_progress_carries_through_an_export_and_an_import_exactly() {
    // The whole of what the script keeps between steps rides in `progress`,
    // which is a payload field, so a run that is exported mid-objective and
    // read back stands exactly where it stood. That is why progress is carried
    // as the locked `Frac` and advanced by a derived per-step share rather than
    // as a step counter the payload has no room for.
    let mut session = Session::new(&support::worker_init()).expect("versions agree");
    session.command(
        "init_run",
        &format!("{{\"form\":\"thread\",\"mode\":\"new\",\"run_id\":\"{KEY}\"}}"),
    );
    // Into the bright current, and part way through the first objective.
    for seq in 1..=4u32 {
        let form = session
            .run()
            .expect("a run")
            .state()
            .now
            .forms
            .iter()
            .find(|form| form.controlled)
            .expect("the controlled Form")
            .pos;
        let (steer_x, steer_y) = toward(form, 1520, 1992);
        let body = format!(
            "{{\"advance_steps\":200,\"depth_key\":0,\"inspect\":null,\"pause\":false,\
             \"pulse_held\":false,\"pulse_release\":false,\"seq\":{seq},\"steer_x\":{steer_x},\
             \"steer_y\":{steer_y},\"t_us\":0,\"toggle_still\":false,\"wheel\":0}}",
        );
        session.command("input_frame", &body);
    }
    let held = session.run().expect("a run").state().progress.objective.clone();
    assert_eq!(held.state, ObjectiveStage::Active);
    assert!(held.progress > 0, "the first objective is part way through");
    assert!(held.progress < 65_536);
    let payload = session.run().expect("a run").state().payload();

    let exported = session.command("export_run", "{}");
    let text = field_game_core::json::parse(&exported)
        .expect("canonical")
        .get("body")
        .and_then(|body| body.get("text"))
        .and_then(|held| held.as_text())
        .expect("an export file")
        .to_string();

    let mut fresh = Session::new(&support::worker_init()).expect("versions agree");
    let quoted = field_game_core::fault::quoted(&text);
    let answer = fresh.command("import_run", &format!("{{\"text\":{quoted}}}"));
    assert!(answer.contains("\"ok\":true"), "{answer}");
    let back = fresh.run().expect("a run").state();
    assert_eq!(back.progress.objective.id, held.id);
    assert_eq!(back.progress.objective.progress, held.progress, "exactly, not nearly");
    assert_eq!(back.progress.objective.target, held.target);
    // The payload is byte-equivalent but for the one post-restore
    // normalization the document locks: every restore sets the Field's own
    // `prev_assembly_step` to the step it returned to. The trajectory's
    // keyframe keeps what it recorded, so only the first of the two moves.
    let normalized =
        payload.replacen("\"prev_assembly_step\":null", "\"prev_assembly_step\":800", 1);
    assert_eq!(back.payload(), normalized, "and the payload is byte-equivalent");
}

#[test]
fn the_first_anchor_restores_to_the_step_the_sequence_completed_at() {
    // The Anchor is written at the step the completing objective completed at,
    // not at the end of the batch that ran it, so a restore lands on the moment
    // the sequence named.
    let mut session = Session::new(&support::worker_init()).expect("versions agree");
    session.command(
        "init_run",
        &format!("{{\"form\":\"thread\",\"mode\":\"new\",\"run_id\":\"{KEY}\"}}"),
    );
    let mut seq = 1u32;
    let mut anchor: Option<(u32, Step)> = None;
    for phase in script() {
        let mut left = phase.steps;
        while left > 0 {
            let form = session
                .run()
                .expect("a run")
                .state()
                .now
                .forms
                .iter()
                .find(|form| form.controlled)
                .expect("the controlled Form")
                .pos;
            let (steer, held, release) = match phase.act {
                Act::Toward(x, y) => (toward(form, x, y), false, false),
                Act::Rest => ((0, 0), false, false),
                Act::Charge(x, y) => (toward(form, x, y), true, false),
                Act::Release => ((0, 0), false, true),
            };
            let run_now = left.min(u32::from(BATCH)) as u16;
            let body = format!(
                "{{\"advance_steps\":{run_now},\"depth_key\":0,\"inspect\":null,\"pause\":false,\
                 \"pulse_held\":{held},\"pulse_release\":{release},\"seq\":{seq},\
                 \"steer_x\":{},\"steer_y\":{},\"t_us\":0,\"toggle_still\":false,\"wheel\":0}}",
                steer.0, steer.1,
            );
            session.command("input_frame", &body);
            seq += 1;
            left -= u32::from(run_now);
            for (name, step, body) in events(&mut session) {
                if name != "checkpoint_written" {
                    continue;
                }
                let held = body.get("anchor").expect("the metadata");
                assert_eq!(held.get("kind").and_then(|kind| kind.as_text()), Some("anchor"));
                // The completion that writes this Anchor is the chapter's own
                // Anchor moment, which stands part way through The Pull rather
                // than at its close: the objective the run is on at the step it
                // was written is that completed objective, and the next is
                // offered on the step after.
                assert_eq!(
                    held.get("objective_id").and_then(|id| id.as_text()),
                    Some("objective.the_pull.carry_pattern"),
                    "the Anchor names the objective the run stands on at the step it was written",
                );
                assert_eq!(held.get("step").and_then(|held| held.as_int()), Some(i64::from(step)));
                anchor = Some((
                    held.get("anchor_id").and_then(|id| id.as_int()).expect("an id") as u32,
                    step,
                ));
            }
        }
    }
    let (anchor_id, at) = anchor.expect("the sequence writes its first Anchor");
    // The identifier space is shared with the autosave cadence's own records —
    // the payload's metadata carries both kinds — so what makes this the first
    // Anchor is that it is the only entry of that kind.
    let written: Vec<&field_game_core::state::CheckpointState> = session
        .run()
        .expect("a run")
        .state()
        .anchors
        .iter()
        .filter(|held| held.kind == field_game_core::state::RecordKind::Anchor)
        .collect();
    assert_eq!(written.len(), 1, "one Anchor, and it is the run's first");
    assert_eq!(written[0].anchor_id, anchor_id);

    let restored = session.command("restore_checkpoint", &format!("{{\"anchor_id\":{anchor_id}}}"));
    assert!(restored.contains("\"ok\":true"), "{restored}");
    let run = session.run().expect("a run is loaded");
    assert_eq!(run.step(), at, "the run lands on the step the Anchor was written at");
    assert_eq!(
        run.state().progress.complete.len(),
        6,
        "with the whole authored sequence behind it",
    );
}

#[test]
fn the_opening_objective_stands_before_a_single_step_runs() {
    let mut session = Session::new(&support::worker_init()).expect("versions agree");
    session.command(
        "init_run",
        &format!("{{\"form\":\"thread\",\"mode\":\"new\",\"run_id\":\"{KEY}\"}}"),
    );
    let raised = events(&mut session);
    let offered = raised
        .iter()
        .find(|(name, _, _)| name == "objective_changed")
        .expect("the run opens with an objective");
    let objective = offered.2.get("objective").expect("an objective");
    assert_eq!(
        objective.get("id").and_then(|held| held.as_text()),
        Some("objective.the_pull.follow_current"),
    );
    assert_eq!(objective.get("state").and_then(|held| held.as_text()), Some("active"));
    let run = session.run().expect("a run is loaded");
    assert_eq!(run.state().progress.objective.state, ObjectiveStage::Active);
    assert_eq!(run.step(), 0, "and it stands before a single step has run");
}

#[test]
fn the_worst_run_the_opening_layer_admits_still_carries_on() {
    // The adversarial run: never enter the bright current, take the deeper
    // layer and stand out of its current until every unit of Charge has
    // drained, then come back up. Nothing here is play — it is the shape a
    // terminal failure would have to take if one existed, driven deliberately
    // so that a later rule which ended a run on lost Charge would fail this
    // test rather than pass unnoticed. "The opening layer cannot produce a
    // terminal failure" is then a guarded claim rather than one argued from the
    // absence of a rule.
    let mut session = Session::new(&support::worker_init()).expect("versions agree");
    session.command(
        "init_run",
        &format!("{{\"form\":\"thread\",\"mode\":\"new\",\"run_id\":\"{KEY}\"}}"),
    );
    let opening = session.run().expect("a run").state().now.forms[0].charge;
    assert!(opening > 0, "a Form opens holding Charge");

    let mut seq = 1u32;
    let mut send = |session: &mut Session, seq: &mut u32, steps: u16, depth: i8, aim: (i64, i64)| {
        let form = session
            .run()
            .expect("a run")
            .state()
            .now
            .forms
            .iter()
            .find(|form| form.controlled)
            .expect("the controlled Form")
            .pos;
        let steer = toward(form, aim.0, aim.1);
        let body = format!(
            "{{\"advance_steps\":{steps},\"depth_key\":{depth},\"inspect\":null,\
             \"pause\":false,\"pulse_held\":false,\"pulse_release\":false,\"seq\":{seq},\
             \"steer_x\":{},\"steer_y\":{},\"t_us\":0,\"toggle_still\":false,\"wheel\":0}}",
            steer.0, steer.1,
        );
        let answer = session.command("input_frame", &body);
        assert!(answer.contains("\"ok\":true"), "{answer}");
        *seq += 1;
    };

    // A far corner of the opening layer, well outside every authored band.
    let corner = (240, 3800);
    for _ in 0..40 {
        send(&mut session, &mut seq, 30, 0, corner);
    }
    assert_eq!(session.lifecycle(), "running", "standing still ends nothing");

    // Down one layer, where Drain stands, and away from its current.
    send(&mut session, &mut seq, 1, 1, corner);
    for _ in 0..200 {
        send(&mut session, &mut seq, 30, 0, corner);
    }
    let deep = session.run().expect("a run").state().now.forms[0].clone();
    assert_eq!(deep.layer, 1, "the run took the deeper layer");
    assert_eq!(deep.charge, 0, "and every unit of Charge drained away");

    // Nothing ended, nothing is unrecoverable, and the sequence still stands
    // where it stood: the first objective, active, waiting to be met.
    assert_eq!(session.lifecycle(), "running");
    let state = session.run().expect("a run").state();
    assert_eq!(state.progress.objective.id, "objective.the_pull.follow_current");
    assert_eq!(state.progress.objective.state, ObjectiveStage::Active);
    assert!(state.progress.complete.is_empty());
    let raised = events(&mut session);
    assert!(
        !raised.iter().any(|(name, _, _)| name == "run_completed"),
        "no ending is reached by losing Charge",
    );

    // And the run recovers by doing the thing the objective asks for: back up
    // to the opening layer, into the bright current, and the Charge returns.
    send(&mut session, &mut seq, 1, -1, (1520, 1992));
    for _ in 0..80 {
        send(&mut session, &mut seq, 30, 0, (1520, 1992));
    }
    let back = session.run().expect("a run").state().now.forms[0].clone();
    assert_eq!(back.layer, 0);
    assert!(back.charge > 0, "the opening layer gives Charge back, and takes none");
    assert!(
        session.run().expect("a run").state().progress.objective.progress > 0,
        "and the sequence carries on from where it stood",
    );
}

#[test]
fn a_restore_puts_the_shell_back_on_the_objective_the_record_carried() {
    // A restore lands the run on a step it has already been at. The objective
    // that stood there is the one the record carried, and the shell is told so
    // the same way it is told on any other reopening — otherwise a restore that
    // moved the run backwards would leave the surface showing an objective the
    // run no longer stands on.
    let bundle =
        field_game_core::json::parse(&support::bundle_with(&support::content_hash()))
            .expect("canonical");
    let content = field_game_core::content::read_bundle(&bundle).expect("the content reads");
    let order: Vec<String> = content
        .chapter(0)
        .expect("a chapter")
        .objectives
        .iter()
        .map(|held| held.id.clone())
        .collect();

    let mut session = Session::new(&support::worker_init()).expect("versions agree");
    session.command(
        "init_run",
        &format!("{{\"form\":\"thread\",\"mode\":\"new\",\"run_id\":\"{KEY}\"}}"),
    );
    events(&mut session);
    let ordinal_at = |session: &Session| -> u16 {
        let view = session.frame_view();
        u16::from_le_bytes([view[18], view[19]])
    };

    // Far enough that a later objective stands and the autosave cadence has
    // written records behind it.
    let mut seq = 1u32;
    for phase in script() {
        if session.run().expect("a run").state().progress.complete.len() >= 3 {
            break;
        }
        let mut left = phase.steps;
        while left > 0 {
            let form = session
                .run()
                .expect("a run")
                .state()
                .now
                .forms
                .iter()
                .find(|form| form.controlled)
                .expect("the controlled Form")
                .pos;
            let (steer, held, release) = match phase.act {
                Act::Toward(x, y) => (toward(form, x, y), false, false),
                Act::Rest => ((0, 0), false, false),
                Act::Charge(x, y) => (toward(form, x, y), true, false),
                Act::Release => ((0, 0), false, true),
            };
            let run_now = left.min(u32::from(BATCH)) as u16;
            let body = format!(
                "{{\"advance_steps\":{run_now},\"depth_key\":0,\"inspect\":null,\
                 \"pause\":false,\"pulse_held\":{held},\"pulse_release\":{release},\
                 \"seq\":{seq},\"steer_x\":{},\"steer_y\":{},\"t_us\":0,\
                 \"toggle_still\":false,\"wheel\":0}}",
                steer.0, steer.1,
            );
            session.command("input_frame", &body);
            seq += 1;
            left -= u32::from(run_now);
        }
    }
    // One more frame, so the objective that completed last has handed over to
    // the one after it: an objective is offered on the step after the one it
    // follows completes.
    let body = format!(
        "{{\"advance_steps\":1,\"depth_key\":0,\"inspect\":null,\"pause\":false,\
         \"pulse_held\":false,\"pulse_release\":false,\"seq\":{seq},\"steer_x\":0,\
         \"steer_y\":0,\"t_us\":0,\"toggle_still\":false,\"wheel\":0}}",
    );
    session.command("input_frame", &body);
    events(&mut session);
    let standing = session.run().expect("a run").state().progress.objective.id.clone();
    let at_ordinal = ordinal_at(&session);
    assert_eq!(
        at_ordinal as usize,
        order.iter().position(|id| id == &standing).expect("an authored objective") + 1,
        "the snapshot's ordinal is the standing objective's place in the authored order",
    );

    // The oldest record the run still holds. Its metadata says which objective
    // stood when it was written, and it is read now rather than remembered, so
    // the autosave slots recycling behind it cannot make this stale.
    let older = session
        .run()
        .expect("a run")
        .state()
        .anchors
        .iter()
        .min_by_key(|held| held.step)
        .expect("the autosave cadence has written a record")
        .clone();
    assert!(older.step < session.run().expect("a run").step(), "and it is behind the run");
    assert_ne!(older.objective_id, standing, "across an objective boundary");

    let restored =
        session.command("restore_checkpoint", &format!("{{\"anchor_id\":{}}}", older.anchor_id));
    assert!(restored.contains("\"ok\":true"), "{restored}");
    assert_eq!(session.run().expect("a run").step(), older.step);
    assert_eq!(
        session.run().expect("a run").state().progress.objective.id,
        older.objective_id,
        "the run stands on the objective the record carried",
    );
    assert_eq!(
        ordinal_at(&session) as usize,
        order.iter().position(|id| id == &older.objective_id).expect("an authored objective") + 1,
        "and the snapshot's ordinal moved back with it",
    );
    assert!(ordinal_at(&session) < at_ordinal, "backwards, not forwards");

    let raised = events(&mut session);
    let offered = raised
        .iter()
        .find(|(name, _, _)| name == "objective_changed")
        .expect("the shell is told which objective the restored run stands on");
    assert_eq!(
        offered.2.get("objective").and_then(|held| held.get("id")).and_then(|id| id.as_text()),
        Some(older.objective_id.as_str()),
    );
    assert!(
        raised.iter().any(|(name, _, _)| name == "chapter_changed"),
        "and which chapter it stands in",
    );
}
