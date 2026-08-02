//! The Rewrite, driven end to end: the whole seventy-minute chapter.
//!
//! The chapter's subject is a run that already stands and has to be changed
//! while it keeps running. Three systems carry it:
//!
//! - **Editable modules.** The two carriers between the intake and the junction
//!   are a module: a cluster with a role, which is to carry eight a step from
//!   the one to the other. Two spares stand below them, open and empty, and a
//!   standby store stands beside the junction. Nothing about a member is
//!   special — what is special is the role, and the role can be given to
//!   different members.
//! - **Dependencies.** Everything east of the junction runs on two Routes the
//!   junction sends along, three a step each. That pair is what every hold
//!   objective of the chapter names, so what the player is asked to preserve is
//!   never the module itself: it is what the module carries.
//! - **Substitution limits.** One queue holds six changes, each costs one
//!   Impulse, a completed objective gives three back, and the run may carry six.
//!   The finale's three rewrite paths cost four, five and two changes, so the
//!   Impulse the run carries out of the chapter is a reading of which path it
//!   took.
//!
//! **The final challenge** breaks the Route between the two carriers two
//! minutes in. From then the junction spends more than it takes in, and the
//! dependency stands about a hundred and sixty steps on what it banked. Three
//! rewrite paths are driven here, each of them completing the chapter, and each
//! leaving the run in a measurably different state:
//!
//! | path | what it does | changes |
//! |---|---|---|
//! | swap by parts | replaces one member, then the other, under the authored compartment | 4 |
//! | wholesale behind the standby | bridges with the standby, then replaces both at once and moves physical membership onto the replacements | 5 |
//! | relocation | leaves the module where it is and hangs the dependency on the standby | 2 |
//!
//! A fourth run is driven for contrast — the same swap by parts, begun after a
//! walk to look at the spares — and it is the one that loses continuity: the
//! junction empties before the first change lands, the dependency stops, and
//! the hold count starts again from nothing.
//!
//! **Two authored dependencies a later tweak could silently break**, both
//! asserted here with their reasons:
//!
//! - *The identifiers a drawn Route takes.* Identifiers are handed out in order
//!   and never reused, so the two outfalls are 8 and 9, the line the player
//!   draws again after Fracture is 10, and the finale's own changes start at 11.
//!   The swap-by-parts path redirects a Route it drew itself, which is only
//!   possible because that number is known. `the_drawn_routes_take_the_numbers_
//!   the_paths_are_written_against` pins it.
//! - *The pressure window.* `pressure_schedule` counts from the chapter's own
//!   opening, so Fracture's step is absolute; an objective is offered when a
//!   player reaches it, so its step is not. Four paths have to be reconciled
//!   rather than two, because a run that lets the optional test go arrives
//!   later than one that takes it.
//!   `the_pressure_lands_inside_the_objective_written_for_it` asserts the
//!   containment on all four and prints the binding window with its margins.

use field_game_core::content::{self, CONTENT_VERSION};
use field_game_core::fx::ONE_UNIT;
use field_game_core::json::Json;
use field_game_core::pressure::{Pressure, TargetKind};
use field_game_core::state::{Step, STEPS_PER_SECOND};
use field_game_core::Session;

mod support;

use support::campaign::{controlled, nearest_point, toward};

const KEY: &str = "00112233445566dd";

/// How many steps one scripted batch runs before the script re-aims.
const BATCH: u16 = 15;

/// How many steps a full Pulse charge takes, from the locked charging rule.
const FULL_CHARGE_STEPS: u32 = 32;

/// How long a ramp takes, in microseconds. Locked.
const RAMP_US: i64 = 250_000;

/// This chapter's place in the authored campaign.
const CHAPTER: u8 = 6;

/// The floor every four-path containment margin is held to, in steps. The
/// wave's own standard.
const PRESSURE_MARGIN: Step = 300;

/// The band point the run parks on for every hold after the opening. It is a
/// path point of the bright band, and it stands 282 units from the intake —
/// inside the shortest authored `route_reach` of the eight Forms, which is what
/// lets the line to the intake be drawn again from where the run is standing.
const BAND_STATION: (i64, i64) = (1960, 1620);

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
    /// Enter Still Mode, take a whole coordinate profile, and ramp back out
    /// without queueing anything.
    Read(&'static str),
}

/// One phase of the script: what it does, how long it runs, whether its length
/// is authored content or a modelled allowance, and the objective it holds
/// until.
///
/// **No phase commits anything.** Every queued change this chapter makes is a
/// reaction inside the driver rather than a step of the script, because every
/// one of them answers something the content raised — an objective in setback,
/// a Route the pressure broke, a Route the sequence broke — and the step each
/// of those lands on belongs to the content rather than to this file.
///
/// **`until` is what keeps the chapter's length the content's rather than this
/// file's.** A phase that holds a band or a pattern runs until the sequence has
/// moved past the objective it names, with `steps` as a cap that no run should
/// reach; a phase that crosses to somewhere runs its own length. Without it,
/// every hold would be a hand-tuned number and the difference between the floor
/// and an attentive first run would be a number this file chose rather than the
/// time the two runs actually spend.
struct Phase {
    label: &'static str,
    act: Act,
    steps: u32,
    allowance: bool,
    until: Option<&'static str>,
}

fn play(label: &'static str, act: Act, steps: u32) -> Phase {
    Phase { label, act, steps, allowance: false, until: None }
}

fn allow(label: &'static str, act: Act, steps: u32) -> Phase {
    Phase { label, act, steps, allowance: true, until: None }
}

/// A phase that holds until the sequence has moved past the named objective.
fn hold(label: &'static str, act: Act, until: &'static str, cap: u32) -> Phase {
    Phase { label, act, steps: cap, allowance: false, until: Some(until) }
}

fn read_profile(label: &'static str, named: &'static str) -> Phase {
    Phase { label, act: Act::Read(named), steps: 0, allowance: false, until: None }
}

fn connect(from: u32, to: u32) -> String {
    format!("{{\"plan\":{{\"from\":{from},\"op\":\"connect\",\"to\":{to}}}}}")
}

fn redirect(route: u32, end: &str, to: u32) -> String {
    format!("{{\"plan\":{{\"end\":\"{end}\",\"op\":\"redirect\",\"route\":{route},\"to\":{to}}}}}")
}

fn reshape(members: &[u32]) -> String {
    let written: Vec<String> = members.iter().map(|held| held.to_string()).collect();
    format!("{{\"plan\":{{\"members\":[{}],\"op\":\"reshape_compartment\"}}}}", written.join(","))
}

// ---------------------------------------------------------------------------
// The three rewrite paths
// ---------------------------------------------------------------------------

/// Which way the run answers the module's break.
///
/// Each of them completes the chapter, and each leaves the run standing in a
/// different state at the close. What differs is the shape of the change, not
/// the skill it takes to make it: the reach rule is a distance between two
/// Nodes rather than a distance from the Form, so every one of these can be
/// queued from the band the run is already holding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Path {
    /// Replace one carrier, then the other, leaving the authored compartment where
    /// it stands. Four changes, two commits.
    ByParts,
    /// Bridge the junction with the standby, then replace both carriers in one
    /// commit and move physical membership onto the replacements. Five changes.
    Wholesale,
    /// Leave the module where it is and hang the dependency on the standby
    /// instead. Two changes.
    Relocation,
    /// Swap by parts, begun after a walk out of the band to look at the spares.
    /// The one path that loses continuity.
    Unprepared,
}

/// The three paths the goal's done-when is about. `Unprepared` is the contrast
/// and is not one of them.
const PATHS: [Path; 3] = [Path::ByParts, Path::Wholesale, Path::Relocation];

impl Path {
    fn name(self) -> &'static str {
        match self {
            Path::ByParts => "by parts",
            Path::Wholesale => "wholesale",
            Path::Relocation => "relocation",
            Path::Unprepared => "unprepared",
        }
    }

    /// How many queued changes the path spends inside the final challenge.
    fn changes(self) -> u8 {
        match self {
            Path::ByParts | Path::Unprepared => 4,
            Path::Wholesale => 5,
            Path::Relocation => 2,
        }
    }
}

/// The Route the two outfalls take, in order: identifiers are handed out in
/// order and never reused, and the chapter authors seven.
const FIRST_DRAWN: u32 = 8;
/// The line to the intake, drawn again after Fracture breaks it.
const REDRAWN_LINE: u32 = 10;
/// The first identifier the final challenge's own changes take.
const FIRST_REWRITE: u32 = 11;

/// One driven run's record of what it edited, in order.
#[derive(Clone, Debug)]
struct Committed {
    step: Step,
    label: &'static str,
    entries: usize,
    impulse_after: u8,
}

/// One coordinate profile the run stopped to read, and the readings the chapter
/// is about.
#[derive(Clone, Debug)]
struct Reading {
    named: &'static str,
    step: Step,
    /// The window the profile was actually read over: `min(w, t0,
    /// retained_span)`, which a commit clamps to nothing.
    window: u16,
    swap_range: Option<i64>,
    turnover: Option<i64>,
    separation: Option<i64>,
    self_support: Option<i64>,
    view_members: Vec<u32>,
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
    stages: Vec<String>,
    offered: Vec<(String, Step)>,
    /// The step the run entered this chapter at. Every figure the chapter is
    /// read by is measured from here, because the step counter is the
    /// campaign's and never restarts.
    opening_step: Step,
    steps: Step,
    anchor_step: Option<Step>,
    setbacks: Vec<Step>,
    recovery_step: Option<Step>,
    transition_step: Option<Step>,
    /// The step the authored break of the module was first read at.
    module_break_step: Option<Step>,
    /// The step Fracture's own break of the line was first read at.
    line_break_step: Option<Step>,
    /// The first batch after the line broke on which the dependency's two
    /// Routes were not both carrying — how long the run stands on what it had
    /// already banked, read once and never again.
    dependency_stopped_step: Option<Step>,
    /// The same reading taken from the module's own break, where the cut falls
    /// between the carriers and the junction and only the junction's bank is
    /// left to hold the dependency up.
    module_stopped_step: Option<Step>,
    /// Each pressure stage the run stood in, in order.
    pressure_steps: Vec<(Step, &'static str, String)>,
    /// Every commit the run made, in order.
    commits: Vec<Committed>,
    /// Every coordinate profile the run stopped to read.
    readings: Vec<Reading>,
    /// How many batches of the final challenge left the objective's own
    /// progress lower than the batch before it — the count restarting, which is
    /// what losing continuity is.
    continuity_breaks: usize,
    /// How many **batches** of the final challenge ended with the two Routes of
    /// the dependency not both carrying. The driver reads the Field once a
    /// batch, so this counts 15-step batches rather than steps, and a stop
    /// shorter than the gap between two readings is not counted at all.
    stopped_batches: usize,
    deep_spare_open: bool,
    /// Charge held across the physical compartment at the chapter's close, in
    /// whole units.
    compartment_units: i64,
    /// Charge the standby held at the close, in whole units.
    standby_units: i64,
    /// Charge the two carriers the chapter opens with held at the close.
    module_units: i64,
    /// Charge the store held at the close.
    store_units: i64,
    form_units: i64,
    forms: usize,
    /// Trail entries still standing in this chapter's Field at the latest
    /// pre-transition reading.
    pending_trails: usize,
    /// Whole-unit capacities of the Routes the player formed in this chapter,
    /// in Route identifier order.
    formed_route_capacities: Vec<i64>,
    routes: usize,
    /// The Routes standing at the close, tail and head, ascending.
    standing_routes: Vec<(u32, u32, u32)>,
    /// The standing Observation View at the close.
    view_members: Vec<u32>,
    /// The causal physical-compartment membership at the close.
    compartment_members: Vec<u32>,
    /// Impulse the run carries into the chapter after this one — read after the
    /// transition, because the completion that ends the chapter is itself the
    /// grant that pays for it. It is the one liability reading that is progress
    /// rather than Field.
    impulse: u8,
    /// What the objective stream left standing, after every `objective_changed`
    /// the run raised.
    standing: Vec<(Step, Vec<String>)>,
    reported: Vec<(String, String)>,
}

impl Driven {
    fn at(&self, name: &str) -> Option<Step> {
        self.milestones.iter().find(|held| held.name == name).map(|held| held.step)
    }

    fn offered_at(&self, id: &str) -> Option<Step> {
        self.offered.iter().find(|(held, _)| held == id).map(|(_, step)| *step)
    }

    /// The step the chapter completed at, measured from its own opening.
    fn span(&self) -> Step {
        self.transition_step.expect("the chapter completes") - self.opening_step
    }

    fn reading(&self, named: &str) -> Option<&Reading> {
        self.readings.iter().find(|held| held.named == named)
    }
}

fn record(driven: &mut Driven, events: Vec<(String, u32, Json)>) {
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
                if id.starts_with("objective.the_rewrite.") {
                    match driven.reported.iter_mut().find(|(held, _)| held == id) {
                        Some(entry) => entry.1 = stage.to_string(),
                        None => driven.reported.push((id.to_string(), stage.to_string())),
                    }
                    let live: Vec<String> = driven
                        .reported
                        .iter()
                        .filter(|(_, held)| held == "active" || held == "failed_recoverable")
                        .map(|(held, _)| held.clone())
                        .collect();
                    driven.standing.push((step, live));
                }
                if stage == "complete" {
                    driven.milestones.push(Milestone { name: id.to_string(), step });
                    driven.completed.push(id.to_string());
                }
                if stage == "failed_recoverable" {
                    driven.setbacks.push(step);
                }
                if stage == "active" && !driven.setbacks.is_empty() {
                    driven.recovery_step.get_or_insert(step);
                }
            }
            "checkpoint_written" => {
                driven.anchor_step.get_or_insert(step);
                driven.milestones.push(Milestone { name: "anchor".to_string(), step });
            }
            "chapter_changed" => {
                if body.get("chapter_index").and_then(|held| held.as_int())
                    == Some(i64::from(CHAPTER) + 1)
                {
                    driven.transition_step = Some(step);
                    driven.milestones.push(Milestone { name: "transition".to_string(), step });
                }
            }
            _ => {}
        }
    }
}

/// Sends one frame body, optionally carrying an inspection request.
fn frame(
    session: &mut Session,
    seq: &mut u32,
    steps: u16,
    steer: (i64, i64),
    held: bool,
    release: bool,
    depth_key: i8,
    t_us: i64,
    toggle: bool,
    inspect: &str,
) {
    let body = format!(
        "{{\"advance_steps\":{steps},\"depth_key\":{depth_key},\"inspect\":{inspect},\
         \"pause\":false,\"pulse_held\":{held},\"pulse_release\":{release},\
         \"seq\":{},\"steer_x\":{},\"steer_y\":{},\"t_us\":{t_us},\
         \"toggle_still\":{toggle},\"wheel\":0}}",
        *seq, steer.0, steer.1,
    );
    let answer = session.command("input_frame", &body);
    assert!(answer.contains("\"ok\":true"), "{answer}");
    *seq += 1;
}

/// A plain stepping frame.
fn step_frame(
    session: &mut Session,
    seq: &mut u32,
    steps: u16,
    steer: (i64, i64),
    held: bool,
    release: bool,
    depth_key: i8,
) {
    frame(session, seq, steps, steer, held, release, depth_key, 0, false, "null");
}

/// Enters Still Mode, queues every plan, commits, and ramps back out.
fn commit(session: &mut Session, seq: &mut u32, plans: &[String]) -> u8 {
    let opened = i64::from(*seq) * 1_000_000 + 10_000_000;
    frame(session, seq, 1, (0, 0), false, false, 0, opened, true, "null");
    assert_eq!(session.lifecycle(), "ramp_in");
    frame(session, seq, 0, (0, 0), false, false, 0, opened + RAMP_US, false, "null");
    assert_eq!(session.lifecycle(), "still", "the ramp completes into Still Mode");
    for plan in plans {
        let answer = session.command("queue_plan", plan);
        assert!(answer.contains("\"ok\":true"), "queueing {plan}: {answer}");
    }
    let answer = session.command("commit_plan", "{}");
    assert!(answer.contains("\"ok\":true"), "committing: {answer}");
    assert_eq!(session.lifecycle(), "ramp_out", "a commit is also the exit");
    let carried = session.run().expect("a run").state().progress.impulse;
    frame(session, seq, 0, (0, 0), false, false, 0, opened + 2 * RAMP_US, false, "null");
    assert_eq!(session.lifecycle(), "running", "the committed exit completes");
    carried
}

/// Enters Still Mode, asks for the whole ten-coordinate profile, and ramps back
/// out having changed nothing.
///
/// A profile is answered only in `still`, which is the surface this chapter is
/// about: the run stands, the readings are taken off the retained window, and
/// nothing moves while they are.
fn inspect(session: &mut Session, seq: &mut u32, named: &'static str) -> Reading {
    let opened = i64::from(*seq) * 1_000_000 + 10_000_000;
    frame(session, seq, 1, (0, 0), false, false, 0, opened, true, "null");
    frame(session, seq, 0, (0, 0), false, false, 0, opened + RAMP_US, false, "null");
    assert_eq!(session.lifecycle(), "still", "the ramp completes into Still Mode");
    let _ = support::campaign::raised(session);
    frame(
        session,
        seq,
        0,
        (0, 0),
        false,
        false,
        0,
        opened + RAMP_US,
        false,
        "{\"kind\":null,\"parameter\":null,\"target\":\"coordinates_full\"}",
    );
    let events = support::campaign::raised(session);
    let profile = events
        .iter()
        .find(|(name, _, _)| name == "review_ready")
        .map(|(_, _, body)| body.get("review").expect("a review").clone())
        .expect("the inspection answered");
    let held = profile.get("profile").expect("a profile");
    let value = |key: &str| -> Option<i64> {
        held.get(key).and_then(|entry| entry.get("value")).and_then(Json::as_int)
    };
    let state = session.run().expect("a run").state();
    let reading = Reading {
        named,
        step: state.now.step,
        window: state.effective_window(state.view.window),
        swap_range: value("swap_range"),
        turnover: value("turnover_tolerance"),
        separation: value("instruction_separation"),
        self_support: value("self_support"),
        view_members: state.view.inside.clone(),
    };
    frame(session, seq, 0, (0, 0), false, false, 0, opened + 2 * RAMP_US, true, "null");
    frame(session, seq, 0, (0, 0), false, false, 0, opened + 3 * RAMP_US, false, "null");
    assert_eq!(session.lifecycle(), "running", "the run stands again");
    reading
}

// ---------------------------------------------------------------------------
// The script
// ---------------------------------------------------------------------------

/// The two outfalls the dependency's own Nodes need, drawn in Still Mode: the
/// chapter's authored setback is that nothing takes Charge out of them.
fn outfalls() -> Vec<String> {
    vec![connect(6, 8), connect(7, 8)]
}

/// The changes one rewrite path commits, in the order it commits them.
fn rewrite(path: Path) -> Vec<Vec<String>> {
    match path {
        Path::ByParts | Path::Unprepared => vec![
            // The second carrier's role goes to the spare below it: a Route
            // from the first carrier to the spare, and the Route out of the old
            // member moved onto the new one.
            vec![connect(3, 10), redirect(5, "tail", 10)],
            // Then the first carrier's, the same way, on the Route the intake
            // already sends along and the Route the first commit drew.
            vec![redirect(2, "head", 9), redirect(FIRST_REWRITE, "tail", 9)],
        ],
        Path::Wholesale => vec![
            // The standby carries the junction while the module is replaced.
            vec![connect(11, 5)],
            // Both members at once, and the physical compartment follows them.
            vec![
                redirect(2, "head", 9),
                connect(9, 10),
                redirect(5, "tail", 10),
                reshape(&[2, 5, 9, 10]),
            ],
        ],
        Path::Relocation => vec![
            // The module is left where it stands. The dependency's own two
            // Routes keep their identifiers and hang on the standby instead.
            vec![redirect(6, "tail", 11), redirect(7, "tail", 11)],
        ],
    }
}

/// The scripted run, phase by phase.
///
/// The positions are the authored content's own — the bright band, the two
/// Nodes of the dependency, the store beyond them, the deep band and the Port
/// below it. Every phase that holds the band after the opening parks on
/// [`BAND_STATION`], which is the band point standing inside every Form's reach
/// of the intake: the line Fracture breaks has to be drawable from where the
/// run is holding, or the chapter would ask a player to choose between holding
/// the band and repairing it.
fn script(takes_the_test: bool) -> Vec<Phase> {
    let station = Act::Toward(BAND_STATION.0, BAND_STATION.1);
    let mut phases = vec![
        allow("read the run", Act::Rest, 600),
        play("steer into the bright band", Act::Toward(1400, 1620), 120),
        play("hold the band", Act::Follow(1), 4_000),
        allow("try the controls", Act::Follow(1), 1_200),
        allow("look along the run", Act::Toward(1800, 1740), 1_200),
        // Back in the band first: the run holds nothing while the Form is out
        // of it, and a profile read of an empty run reads an empty graph.
        play("back to the band", station, 1_200),
        read_profile("read the run standing", "the module carrying"),
        hold("hold the band", station, "follow_the_band", 30_000),
        // The two Nodes of the dependency, each opened by a Pulse beside it.
        play("cross to the first dependent", Act::Toward(3056, 1900), 2_400),
        play("charge", Act::Charge(3056, 1900), FULL_CHARGE_STEPS),
        play("release", Act::Release, 1),
        play("cross to the second dependent", Act::Toward(3056, 2120), 600),
        play("charge", Act::Charge(3056, 2120), FULL_CHARGE_STEPS),
        play("release", Act::Release, 1),
        allow("look for where it can go", Act::Toward(3260, 2010), 900),
        play("back to the band", station, 2_600),
        // The authored setback stands here: nothing takes Charge out of the two
        // Nodes of the dependency, so they carry more than they can hold and
        // the objective reads `failed_recoverable` until the outfalls are
        // drawn. The drawing is a reaction rather than a phase, because the
        // step the setback appears on belongs to the sequence.
        hold("hold the dependency", station, "hold_the_dependency", 50_000),
        // Depth.
        play("cross to the deep band", Act::Toward(2600, 2340), 1_800),
        play("down a layer", Act::Depth(1), 1),
        hold("hold the deep band", Act::Follow(2), "take_the_deep_band", 40_000),
    ];
    if takes_the_test {
        phases.extend([
            allow("look for the spare", Act::Toward(2660, 2420), 600),
            play("cross to the spare", Act::Toward(2660, 2420), 500),
            play("charge", Act::Charge(2660, 2420), FULL_CHARGE_STEPS),
            play("release", Act::Release, 1),
        ]);
    } else {
        phases.push(hold(
            "let the spare go",
            Act::Follow(2),
            "open_the_deep_spare",
            20_000,
        ));
    }
    phases.extend([
        play("back up a layer", Act::Depth(-1), 1),
        play("back to the band", station, 2_400),
        // Fracture builds through three stages and breaks the line at the
        // third. The repair is driven by the reaction below rather than by a
        // phase, because the step it falls due on is the pressure's and the
        // step a phase reaches is the player's.
        hold("carry the run", station, "carry_the_supply", 50_000),
        // The final challenge. The module's own Route breaks two minutes in,
        // and the rewrite is driven by the reaction below for the same reason.
        hold("hold the run through the rewrite", station, "rewrite_the_module", 70_000),
    ]);
    phases
}

/// How long the unprepared run spends out of the band looking at the spares
/// before it makes its first change. The band is the run's only supply, so the
/// walk costs the junction its inflow as well as the time.
const UNPREPARED_WALK: u32 = 900;

/// How long the run holds between the two commits of a two-commit path.
const SETTLING: u32 = 240;

/// How long the run holds after the last change before it reads the profile
/// again: a commit ends the active window, and the span regrows a step at a
/// time.
const REGROWING: u32 = 900;

/// How long an attentive first run stands in the authored setback before it
/// works out that the two Nodes of the dependency need somewhere to send what
/// arrives. The authored floor answers it on the step it appears.
const READING_THE_SETBACK: u32 = 2_400;

// ---------------------------------------------------------------------------
// The driver
// ---------------------------------------------------------------------------

/// Drives one run: a named starting Form, a rewrite path, whether the modelled
/// allowances are spent, whether the optional test is taken, whether the run
/// ever holds a band at all, and whether it ever enters Still Mode to commit.
fn drive_full(
    form: &str,
    path: Path,
    allowances: bool,
    takes_the_test: bool,
    supplies: bool,
    edits: bool,
    repairs: bool,
) -> Driven {
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
        opening_step: 0,
        steps: 0,
        anchor_step: None,
        setbacks: Vec::new(),
        recovery_step: None,
        transition_step: None,
        module_break_step: None,
        line_break_step: None,
        dependency_stopped_step: None,
        module_stopped_step: None,
        pressure_steps: Vec::new(),
        commits: Vec::new(),
        readings: Vec::new(),
        continuity_breaks: 0,
        stopped_batches: 0,
        deep_spare_open: false,
        compartment_units: 0,
        standby_units: 0,
        module_units: 0,
        store_units: 0,
        form_units: 0,
        forms: 0,
        pending_trails: 0,
        formed_route_capacities: Vec::new(),
        routes: 0,
        standing_routes: Vec::new(),
        view_members: Vec::new(),
        compartment_members: Vec::new(),
        impulse: 0,
        standing: Vec::new(),
        reported: Vec::new(),
    };
    // The run opens in The Pull, so the six chapters before this one are played
    // by the shared campaign script and this file picks the run up at its own
    // opening step.
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
        "the run stands in The Rewrite",
    );
    driver.drain(&mut session);
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
    let opening = driven.opening_step;
    let carried: Vec<(String, u32, Json)> = driver
        .raised
        .iter()
        .filter(|(name, step, body)| {
            name == "objective_changed"
                && *step >= opening
                && body
                    .get("objective")
                    .and_then(|held| held.get("id"))
                    .and_then(|held| held.as_text())
                    .is_some_and(|id| id.starts_with("objective.the_rewrite."))
        })
        .cloned()
        .collect();
    record(&mut driven, carried);
    record(&mut driven, support::campaign::raised(&mut session));
    let mut seq = driver.seq;

    let mut module_route = true;
    let mut line_route = true;
    let mut last_progress: i64 = 0;
    // The two reactions: a player answers a break when it happens, and the step
    // it happens on belongs to the pressure and the sequence rather than to the
    // script. Everything the finale is measured by rides on these.
    let plans = rewrite(path);
    let mut repaired = false;
    let mut rewritten = 0usize;
    let mut read_broken = false;
    let mut read_settled = false;
    let mut walked = 0u32;
    let mut settling = 0u32;
    let mut drained = false;
    let mut read_setback = 0u32;
    for phase in script(takes_the_test) {
        if phase.allowance && !allowances {
            continue;
        }
        if driven.transition_step.is_some() {
            break;
        }
        match phase.act {
            Act::Read(named) => {
                let reading = inspect(&mut session, &mut seq, named);
                driven.readings.push(reading);
                record(&mut driven, support::campaign::raised(&mut session));
                continue;
            }
            _ => {}
        }
        let mut left = phase.steps;
        while left > 0 {
            if driven.transition_step.is_some() {
                break;
            }
            // A holding phase runs until the sequence has moved past the
            // objective it names, so the chapter's length is the content's.
            if let Some(until) = phase.until {
                if !session
                    .run()
                    .expect("a run")
                    .state()
                    .progress
                    .objective
                    .id
                    .ends_with(until)
                {
                    break;
                }
            }
            let at = controlled(&session);
            let walking = path == Path::Unprepared && !module_route && walked < UNPREPARED_WALK;
            // `supplies` false is the no-direct-play run: it plays the whole
            // script as far as the two outfalls — everything the chapter can be
            // given by editing — and then parks clear of every band instead of
            // holding one. The Form starts inside the bright band, so a run that
            // simply rests would still complete the opening; what this isolates
            // is the back half.
            let act = match (supplies, drained, walking) {
                (false, true, _) => Act::Toward(1400, 2400),
                (_, _, true) => Act::Toward(2400, 2200),
                _ => phase.act,
            };
            let (steer, held, release, depth) = match act {
                Act::Toward(x, y) => (toward(at, x, y), false, false, 0),
                Act::Follow(current) => {
                    let (x, y) = nearest_point(&session, current).unwrap_or((0, 0));
                    (toward(at, x, y), false, false, 0)
                }
                Act::Rest => ((0, 0), false, false, 0),
                Act::Charge(x, y) => (toward(at, x, y), true, false, 0),
                Act::Release => ((0, 0), false, true, 0),
                Act::Depth(way) => ((0, 0), false, false, way),
                Act::Read(_) => unreachable!("a profile reading is not a stepping act"),
            };
            let now = left.min(u32::from(BATCH)) as u16;
            step_frame(&mut session, &mut seq, now, steer, held, release, depth);
            left -= u32::from(now);

            record(&mut driven, support::campaign::raised(&mut session));

            let state = session.run().expect("a run").state();
            driven.steps = state.now.step;
            if driven.transition_step.is_some() {
                continue;
            }
            let module_now = state.now.routes.iter().any(|route| route.route == 4);
            if module_route && !module_now {
                driven.module_break_step = Some(state.now.step);
            }
            module_route = module_now;
            let line_now = state.now.routes.iter().any(|route| route.route == 1);
            if line_route && !line_now {
                driven.line_break_step = Some(state.now.step);
            }
            line_route = line_now;
            // How long what is banked upstream holds the dependency up, read
            // from the break rather than from the chapter's opening: the two
            // Routes carry only on a step the junction has something to send.
            if (driven.line_break_step.is_some() && driven.dependency_stopped_step.is_none())
                || (driven.module_break_step.is_some() && driven.module_stopped_step.is_none())
            {
                let carrying = [6u32, 7].iter().all(|named| {
                    state.now.routes.iter().any(|held| held.route == *named && held.flow > 0)
                });
                if !carrying {
                    if driven.line_break_step.is_some()
                        && driven.dependency_stopped_step.is_none()
                    {
                        driven.dependency_stopped_step = Some(state.now.step);
                    }
                    if driven.module_break_step.is_some() && driven.module_stopped_step.is_none() {
                        driven.module_stopped_step = Some(state.now.step);
                    }
                }
            }
            for pressure in &state.pressures {
                if pressure.queued {
                    continue;
                }
                let named = pressure.pressure.name();
                let stage = pressure.stage.name().to_string();
                if driven.pressure_steps.last().map(|held| (held.1, &held.2))
                    != Some((named, &stage))
                {
                    driven.pressure_steps.push((state.now.step, named, stage.clone()));
                }
            }
            // Continuity, measured on the final challenge alone: a hold count
            // that falls is a count that started again, which is the whole of
            // what "without losing continuity" asks about.
            if state.progress.objective.id.ends_with("rewrite_the_module") {
                let progress = state.progress.objective.progress;
                if progress < last_progress {
                    driven.continuity_breaks += 1;
                }
                last_progress = progress;
                let carrying = [6u32, 7]
                    .iter()
                    .all(|named| {
                        state.now.routes.iter().any(|held| held.route == *named && held.flow > 0)
                    });
                if !carrying {
                    driven.stopped_batches += 1;
                }
            }
            driven.deep_spare_open =
                driven.deep_spare_open || state.now.ports.iter().any(|p| p.node == 12 && p.open);
            driven.compartment_units = state
                .now
                .ports
                .iter()
                .filter(|port| state.now.physical_compartment.members.contains(&port.node))
                .map(|port| port.q / ONE_UNIT)
                .sum();
            driven.standby_units = charge_of(state, &[11]);
            driven.module_units = charge_of(state, &[3, 4]);
            driven.store_units = charge_of(state, &[8]);
            driven.form_units = state
                .now
                .forms
                .iter()
                .find(|form| form.controlled)
                .map_or(0, |form| form.charge / ONE_UNIT);
            driven.forms = state.now.forms.len();
            driven.pending_trails = state.now.pending.len();
            driven.formed_route_capacities = state
                .now
                .routes
                .iter()
                .filter(|route| route.formed_step >= driven.opening_step)
                .map(|route| route.capacity / ONE_UNIT)
                .collect();
            driven.routes = state.now.routes.len();
            driven.standing_routes =
                state.now.routes.iter().map(|r| (r.route, r.tail, r.head)).collect();
            driven.view_members = state.view.inside.clone();
            driven.compartment_members = state.now.physical_compartment.members.clone();

            // The reactions. A break is answered when it is read, and the two
            // of them are the only edits the chapter's back half makes.
            if walking {
                walked += u32::from(now);
                continue;
            }
            if settling > 0 {
                settling -= u32::from(now).min(settling);
                continue;
            }
            // The authored setback, answered: the two Nodes of the dependency
            // carry more than they can hold, and Still Mode is where they are
            // given somewhere to send it.
            if edits
                && !drained
                && state.progress.objective.state
                    == field_game_core::state::ObjectiveStage::FailedRecoverable
            {
                if allowances && read_setback < READING_THE_SETBACK {
                    read_setback += u32::from(now);
                    continue;
                }
                let before = state.now.step;
                let impulse = commit(&mut session, &mut seq, &outfalls());
                driven.commits.push(Committed {
                    step: before,
                    label: "draw the two outfalls",
                    entries: 2,
                    impulse_after: impulse,
                });
                record(&mut driven, support::campaign::raised(&mut session));
                drained = true;
                continue;
            }
            // The line runs from the Form's own Node, so drawing it again is
            // the one change in the chapter whose reach rule reads where the
            // Form is standing: the locked distance adds 512 units per layer,
            // and no authored `route_reach` covers that, so it cannot be drawn
            // from the deep band at all.
            let standing = state
                .now
                .forms
                .iter()
                .find(|form| form.controlled)
                .map_or(1, |form| form.layer);
            if edits && repairs && !repaired && !line_route && standing == 0 {
                let before = session.run().expect("a run").state().now.step;
                let impulse = commit(&mut session, &mut seq, &[connect(1, 2)]);
                driven.commits.push(Committed {
                    step: before,
                    label: "draw the line again",
                    entries: 1,
                    impulse_after: impulse,
                });
                record(&mut driven, support::campaign::raised(&mut session));
                repaired = true;
                continue;
            }
            if !module_route {
                if !read_broken {
                    driven.readings.push(inspect(&mut session, &mut seq, "the module broken"));
                    read_broken = true;
                    continue;
                }
                if edits && rewritten < plans.len() {
                    let before = session.run().expect("a run").state().now.step;
                    let impulse = commit(&mut session, &mut seq, &plans[rewritten]);
                    driven.commits.push(Committed {
                        step: before,
                        label: if rewritten == 0 { "rewrite" } else { "rewrite again" },
                        entries: plans[rewritten].len(),
                        impulse_after: impulse,
                    });
                    record(&mut driven, support::campaign::raised(&mut session));
                    rewritten += 1;
                    if rewritten < plans.len() {
                        settling = SETTLING;
                    } else {
                        // Read on the step the last change lands, before any
                        // batch runs: the commit clamped the retained span to
                        // nothing, and this is the reading that shows it.
                        driven
                            .readings
                            .push(inspect(&mut session, &mut seq, "the module rewritten"));
                        settling = REGROWING;
                    }
                    continue;
                }
                if !read_settled && (!edits || rewritten == plans.len()) {
                    driven.readings.push(inspect(&mut session, &mut seq, "the run settled"));
                    read_settled = true;
                    continue;
                }
            }
        }
    }
    // Read after the loop: the transition is raised on the step the last
    // objective completed, and that completion is what grants the Impulse the
    // next chapter opens with. Every Field reading above is taken before it,
    // because a run that has completed the chapter stands in the next one's
    // Field.
    driven.impulse = session.run().expect("a run").state().progress.impulse;
    driven
}

/// Charge held across a named set of Nodes, in whole units.
fn charge_of(state: &field_game_core::state::RunState, nodes: &[u32]) -> i64 {
    state
        .now
        .ports
        .iter()
        .filter(|port| nodes.contains(&port.node))
        .map(|port| port.q / ONE_UNIT)
        .sum()
}

/// The ordinary driven run: it holds the band, it edits, and it answers both
/// breaks.
fn drive_as(form: &str, path: Path, allowances: bool, takes_the_test: bool) -> Driven {
    drive_full(form, path, allowances, takes_the_test, true, true, true)
}

/// The same run, never drawing the line Fracture breaks again.
fn drive_no_repair() -> Driven {
    drive_full("thread", Path::ByParts, false, true, true, true, false)
}

/// The attentive first run: every allowance spent, the spare taken.
fn attentive() -> Driven {
    drive_as("thread", Path::ByParts, true, true)
}

/// The authored floor: no allowance spent, the spare taken.
fn floor() -> Driven {
    drive_as("thread", Path::ByParts, false, true)
}

/// The authored content, read as the worker hands it over.
fn authored() -> content::Content {
    let bundle = field_game_core::json::parse(&support::bundle_with(&support::content_hash()))
        .expect("canonical");
    content::read_bundle(&bundle).expect("the content reads")
}

/// The chapter this file is about.
fn chapter() -> content::Chapter {
    authored().chapter(CHAPTER).expect("The Rewrite").clone()
}

/// A step count as sim-minutes, to one decimal place, as a string.
fn minutes(step: Step) -> String {
    let tenths = i64::from(step) * 10 / (i64::from(STEPS_PER_SECOND) * 60);
    format!("{}.{}", tenths / 10, tenths % 10)
}

/// The objectives, in the authored order.
fn authored_order() -> Vec<String> {
    chapter().objectives.iter().map(|held| held.id.clone()).collect()
}

// ---------------------------------------------------------------------------
// The chapter, completed
// ---------------------------------------------------------------------------

#[test]
fn the_chapter_completes_and_carries_the_run_into_its_own_transition() {
    let run = attentive();

    println!(
        "\nThe Rewrite, attentive first run — milestone table (steps from the chapter's opening)"
    );
    println!("{:<52} {:>8} {:>8}", "milestone", "step", "minutes");
    for milestone in &run.milestones {
        let at = milestone.step - run.opening_step;
        println!("{:<52} {at:>8} {:>8}", milestone.name, minutes(at));
    }
    println!("offered at");
    for (id, step) in &run.offered {
        let at = step - run.opening_step;
        println!("  {id:<50} {at:>8} {:>8}", minutes(at));
    }
    for (name, step) in [
        ("Fracture breaks the line", run.line_break_step),
        ("the module's own Route breaks", run.module_break_step),
    ] {
        if let Some(step) = step {
            let at = step - run.opening_step;
            println!("{name:<52} {at:>8} {:>8}", minutes(at));
        }
    }
    println!("commits");
    for held in &run.commits {
        println!(
            "  {:<40} {:>8} {:>3} changes, Impulse after {:>2}",
            held.label,
            held.step - run.opening_step,
            held.entries,
            held.impulse_after,
        );
    }
    println!("recoverable setbacks at");
    for step in &run.setbacks {
        println!("  {:>8}", step - run.opening_step);
    }

    assert_eq!(
        run.completed,
        authored_order(),
        "every objective completes once each, in the authored order",
    );
    let transition = run.transition_step.expect("the chapter completes and the run carries on");
    assert_eq!(
        transition,
        run.at("objective.the_rewrite.rewrite_the_module").expect("the last"),
        "the transition lands on the step the final challenge completed",
    );
    assert_eq!(
        run.anchor_step,
        run.at("objective.the_rewrite.hold_the_dependency"),
        "the chapter's one Anchor is written at the step the dependency first held",
    );
}

#[test]
fn the_chapter_asks_for_about_seventy_minutes_and_the_floor_stands_under_it() {
    let floor = floor();
    let attentive = attentive();
    let floor_at = floor.span();
    let attentive_at = attentive.span();

    let spent: u32 = script(true)
        .iter()
        .filter(|phase| phase.allowance)
        .map(|phase| phase.steps)
        .sum();
    println!("\nThe Rewrite — the authored floor against the attentive first run");
    println!("  authored floor      {floor_at:>7} steps  {:>6} minutes", minutes(floor_at));
    println!(
        "  attentive first run {attentive_at:>7} steps  {:>6} minutes",
        minutes(attentive_at)
    );
    println!("modelled first-run allowances, {spent} steps in all:");
    for phase in script(true).iter().filter(|phase| phase.allowance) {
        println!("  {:<40} {:>6} steps", phase.label, phase.steps);
    }
    println!("\nthe authored floor — milestone table");
    println!("{:<52} {:>8} {:>8}", "milestone", "step", "minutes");
    for milestone in &floor.milestones {
        let at = milestone.step - floor.opening_step;
        println!("{:<52} {at:>8} {:>8}", milestone.name, minutes(at));
    }

    let minute = 60 * STEPS_PER_SECOND;
    assert!(
        attentive_at > 66 * minute && attentive_at < 74 * minute,
        "the attentive run completes the chapter in {attentive_at} steps ({} minutes), \
         outside the seventy-minute band",
        minutes(attentive_at),
    );
    assert!(
        floor_at > 56 * minute && floor_at < attentive_at,
        "the authored floor completes in {floor_at} steps ({} minutes), outside its band",
        minutes(floor_at),
    );
}

#[test]
fn every_starting_form_completes_the_whole_chapter() {
    let authored = authored_order();
    println!("\nthe eight Forms through the whole chapter");
    println!(
        "{:<8} {:>10} {:>8} {:>8} {:>8} {:>6} {:>5} {:>6} {:>8} {:>8} {:>8}",
        "form", "objectives", "steps", "minutes", "setbacks", "routes", "forms", "trails", "compartment", "standby", "store",
    );
    let mut evidence: Vec<(&str, usize, usize, Vec<i64>)> = Vec::new();
    for form in field_game_core::run::FORMS {
        let run = drive_as(form, Path::ByParts, false, true);
        assert_eq!(run.completed, authored, "{form} completes the whole chapter in order");
        let at = run.span();
        for stage in &run.stages {
            assert!(
                ["hidden", "active", "complete", "failed_recoverable"].contains(&stage.as_str()),
                "{form} stood in an unexpected objective state: {stage}",
            );
        }
        assert!(run.deep_spare_open, "{form} opened the deep spare the optional test names");
        println!(
            "{form:<8} {:>10} {at:>8} {:>8} {:>8} {:>6} {:>5} {:>6} {:>8} {:>8} {:>8}",
            run.completed.len(),
            minutes(at),
            run.setbacks.len(),
            run.routes,
            run.forms,
            run.pending_trails,
            run.compartment_units,
            run.standby_units,
            run.store_units,
        );
        evidence.push((
            form,
            run.forms,
            run.pending_trails,
            run.formed_route_capacities.clone(),
        ));
    }
    // Closing Charge may converge under the chapter's one causal physical-
    // compartment coefficient. Assert the chassis mechanisms this script
    // actually exercises instead of treating incidental final magnitudes as a
    // Form fingerprint: Chorus stands its linked group up, Wake leaves delayed
    // Trail entries, and Relay gives every Route the player forms twice the
    // ordinary capacity.
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
    assert_eq!(
        of("relay").3.as_slice(),
        [64, 64, 64, 64],
        "Relay doubles all four Routes this chapter asks the player to form",
    );
    for held in evidence.iter().filter(|held| held.0 != "relay") {
        assert_eq!(
            held.3.as_slice(),
            [32, 32, 32, 32],
            "{} forms all four Routes at the ordinary capacity",
            held.0,
        );
    }
}

// ---------------------------------------------------------------------------
// Both hands: direct control and Still Mode
// ---------------------------------------------------------------------------

#[test]
fn the_dependency_cannot_be_held_by_playing_alone() {
    let run = drive_full("thread", Path::ByParts, false, true, true, false, false);
    println!(
        "\nno Still Mode: {} objectives completed, {} Routes standing, transition {:?}",
        run.completed.len(),
        run.routes,
        run.transition_step,
    );
    assert_eq!(run.routes, 7, "no Route was drawn, so the seven authored ones stand alone");
    assert!(
        run.completed.contains(&"objective.the_rewrite.run_the_dependency".to_string()),
        "steering and the Pulse carry the run as far as starting the dependency",
    );
    assert!(
        !run.completed.contains(&"objective.the_rewrite.hold_the_dependency".to_string()),
        "and no further: nothing takes Charge out of the two Nodes, so they stay over their \
         own thresholds and the pattern never holds",
    );
    assert!(run.transition_step.is_none(), "the chapter does not complete");
}

#[test]
fn the_dependency_cannot_be_held_by_routing_alone() {
    // The second necessity, pinned: a run that opens both Nodes of the
    // dependency and draws both outfalls — everything the chapter can be given
    // by editing — and then lets go of the band cannot hold the pattern. The
    // run makes no Charge of its own: every unit it moves came in along the one
    // Route from the Form, and the Form gathers only while it stands in a band.
    let run = drive_full("thread", Path::ByParts, false, true, false, true, true);
    println!(
        "\nno direct play past the outfalls: {} objectives completed, transition {:?}, \
         compartment {} store {}",
        run.completed.len(),
        run.transition_step,
        run.compartment_units,
        run.store_units,
    );
    assert!(
        run.completed.contains(&"objective.the_rewrite.run_the_dependency".to_string()),
        "the routing itself lands: both Routes carry once the outfalls are drawn",
    );
    // The run stops at the fourth objective, not a later one: `hold_the_
    // dependency` is the first that asks for the pattern to be *held*, and
    // naming a later objective would pass for the wrong reason — every
    // objective after the fourth is out of reach once the fourth is.
    assert!(
        !run.completed.contains(&"objective.the_rewrite.hold_the_dependency".to_string()),
        "the pattern cannot be held once the Form stops carrying Charge to it: {:?}",
        run.completed,
    );
    assert!(run.transition_step.is_none(), "the chapter does not complete");
    assert_eq!(run.compartment_units, 0, "the physical compartment stands empty");
}

// ---------------------------------------------------------------------------
// The three rewrite paths
// ---------------------------------------------------------------------------

#[test]
fn three_rewrite_paths_succeed_and_each_leaves_a_different_liability() {
    let authored = authored_order();
    println!("\nthe three rewrite paths, each driven through the whole chapter");
    println!(
        "{:<12} {:>8} {:>8} {:>8} {:>8} {:>7} {:>8} {:>8} {:>8} {:>8}",
        "path",
        "changes",
        "steps",
        "minutes",
        "Impulse",
        "routes",
        "standby",
        "module",
        "compartment",
        "breaks",
    );
    let mut readings: Vec<(Path, Driven)> = Vec::new();
    for path in PATHS {
        let run = drive_as("thread", path, false, true);
        assert_eq!(
            run.completed, authored,
            "{} completes the whole chapter in order",
            path.name(),
        );
        println!(
            "{:<12} {:>8} {:>8} {:>8} {:>8} {:>7} {:>8} {:>8} {:>8} {:>8}",
            path.name(),
            path.changes(),
            run.span(),
            minutes(run.span()),
            run.impulse,
            run.routes,
            run.standby_units,
            run.module_units,
            run.compartment_units,
            run.continuity_breaks,
        );
        readings.push((path, run));
    }
    for (path, run) in &readings {
        println!(
            "  {} physical compartment {:?}; View {:?}",
            path.name(),
            run.compartment_members,
            run.view_members,
        );
        println!("  {} Routes {:?}", path.name(), run.standing_routes);
    }

    for (path, run) in &readings {
        // The `changes` column is a declaration; the commits are what the run
        // actually queued. The two agree here or the column is decoration.
        // Only the rewrite's own commits count — the two outfalls and the
        // redrawn line answer the chapter before the final challenge.
        let queued: usize = run
            .commits
            .iter()
            .filter(|held| held.label.starts_with("rewrite"))
            .map(|held| held.entries)
            .sum();
        assert_eq!(
            usize::from(path.changes()),
            queued,
            "{}: the path declares {} changes inside the final challenge and the run \
             committed {queued}",
            path.name(),
            path.changes(),
        );
        assert_eq!(
            run.continuity_breaks,
            0,
            "{}: the hold count started again during the rewrite",
            path.name(),
        );
        assert!(run.transition_step.is_some(), "{} completes the chapter", path.name());
    }

    // And the three take the same time. Every reading in the liability table
    // below is taken at the chapter's close, so a path that ran longer would
    // have carried, drained and leaked for longer before it was read — and the
    // table would report as a difference between the paths what was a
    // difference in pace. Equal spans take that confound out of it: whatever
    // the table shows is the shape of the change and nothing else.
    let spans: Vec<Step> = readings.iter().map(|(_, run)| run.span()).collect();
    assert_eq!(
        spans,
        vec![120_239, 120_239, 120_239],
        "the three paths run the chapter for the same number of steps, so nothing in the \
         liability table is a reading of pace",
    );

    // The liabilities, each a reading of the payload the next chapter opens on.
    let of = |path: Path| -> &Driven {
        &readings.iter().find(|(held, _)| *held == path).expect("driven").1
    };
    let by_parts = of(Path::ByParts);
    let wholesale = of(Path::Wholesale);
    let relocation = of(Path::Relocation);

    println!("\nthe liability each path leaves, read off the payload the next chapter opens on");
    println!(
        "{:<12} {:>8} {:>7} {:>8} {:>9} {:>8} {:>11} {:<16}",
        "path", "Impulse", "routes", "standby", "abandoned", "compartment", "Swap Range", "physical members",
    );
    for (path, run) in &readings {
        let swap = run
            .reading("the run settled")
            .and_then(|held| held.swap_range)
            .expect("the run read its own profile after the rewrite");
        println!(
            "{:<12} {:>8} {:>7} {:>8} {:>9} {:>8} {:>11} {:?}",
            path.name(),
            run.impulse,
            run.routes,
            run.standby_units,
            run.module_units,
            run.compartment_units,
            swap,
            run.compartment_members,
        );
    }

    // **Impulse.** The three paths cost four, five and two changes inside one
    // objective, and the completion that closes the chapter grants three
    // clamped at six — so what the next chapter opens with is a reading of the
    // path taken and nothing else.
    let impulses = [by_parts.impulse, wholesale.impulse, relocation.impulse];
    assert_eq!(
        impulses,
        [5, 4, 6],
        "the Impulse ladder is the chapter's own arithmetic: six, less the changes \
         the path spent, plus the three the last completion grants, clamped at six",
    );

    // **The standby.** Only the wholesale path spends it as a bridge, and only
    // the relocation path hangs the dependency on it — both leave it empty, and
    // swapping by parts never touches it.
    assert!(
        by_parts.standby_units > 2_000,
        "swapping by parts leaves the standby full: {} units",
        by_parts.standby_units,
    );
    assert_eq!(wholesale.standby_units, 0, "the wholesale bridge spends the standby");
    assert_eq!(relocation.standby_units, 0, "and the relocation runs the dependency off it");

    // **What is abandoned.** Only the relocation path leaves the module
    // standing and still drawing from the intake: eight a step into a Node with
    // nowhere to send it, which it sheds. The other two move the Route the
    // intake sends along onto a replacement, so the old members drain to
    // nothing.
    assert_eq!(by_parts.module_units, 0, "the parts swap leaves the old members empty");
    assert_eq!(wholesale.module_units, 0, "and so does the wholesale replacement");
    assert!(
        relocation.module_units > 50,
        "the relocation leaves the old module holding {} units it cannot spend",
        relocation.module_units,
    );

    // **Standing Routes.** Eight, nine and ten: the relocation draws none at
    // all in the finale, the parts swap draws one, the wholesale draws two.
    assert_eq!(
        [by_parts.routes, wholesale.routes, relocation.routes],
        [9, 10, 8],
        "the three paths leave different numbers of Routes standing",
    );

    // **Compartment versus View.** The wholesale path moves physical members
    // onto the replacements. Swap Range still reads the independently authored
    // Observation View, so the physical intervention must not replace the
    // member list that instrument uses.
    let swap = |run: &Driven| -> i64 {
        run.reading("the run settled").and_then(|held| held.swap_range).expect("a reading")
    };
    let swap_values = [swap(by_parts), swap(wholesale), swap(relocation)];
    assert!(swap_values.iter().all(|held| *held >= 0), "all fixed Views remain readable");
    assert_eq!(
        wholesale.compartment_members,
        vec![2, 5, 9, 10],
        "the wholesale intervention moves physical membership onto the replacements",
    );
    assert_eq!(by_parts.compartment_members, vec![2, 3, 4, 5]);
    assert_eq!(relocation.compartment_members, vec![2, 3, 4, 5]);
    assert_eq!(
        wholesale.view_members, by_parts.view_members,
        "the physical edit does not replace the Observation View",
    );
    assert_eq!(relocation.view_members, by_parts.view_members);
    assert_eq!(by_parts.view_members, vec![2, 3, 4, 5]);

    // Every reading above is one axis, and no path is better than another on
    // every one of them at once: what the chapter offers is a choice rather
    // than a solution. The four measures are the ones the payload carries and a
    // player can read — Impulse, Routes standing, what the standby holds, what
    // the physical compartment holds — with more Impulse, fewer Routes, and more of
    // both stores counted as better.
    let dominates = |a: &Driven, b: &Driven| {
        a.impulse >= b.impulse
            && a.routes <= b.routes
            && a.standby_units >= b.standby_units
            && a.compartment_units >= b.compartment_units
            && (a.impulse > b.impulse
                || a.routes < b.routes
                || a.standby_units > b.standby_units
                || a.compartment_units > b.compartment_units)
    };
    for (path, run) in &readings {
        let beaten: Vec<&str> = readings
            .iter()
            .filter(|(other, _)| other != path)
            .filter(|(_, other)| dominates(run, other))
            .map(|(other, _)| other.name())
            .collect();
        println!("  {} dominates {beaten:?}", path.name());
        // Not "dominates fewer than all of them" — none of the three dominates
        // any other at all. The weaker reading passes on a table where one path
        // beats one other and loses to the third, which is a ranking with a
        // gap in it rather than the choice this chapter claims to offer.
        assert!(
            beaten.is_empty(),
            "{} dominates {beaten:?}: on these four measures the three paths are a choice \
             rather than an order, so no path may be no worse than another on every one",
            path.name(),
        );
    }
}

// ---------------------------------------------------------------------------
// Fracture, and the four paths its window has to hold on
// ---------------------------------------------------------------------------

#[test]
fn the_pressure_lands_inside_the_objective_written_for_it() {
    // `pressure_schedule` counts from the chapter's opening and an objective is
    // offered when a player reaches it, so the two are reconciled by the one
    // lever authoring has: the objective asks for long enough that the latest
    // path has reached it before the earliest has left it, and Fracture is
    // scheduled into the span every path stands inside. Four paths, because a
    // run that lets the optional test go arrives later than one that takes it.
    //
    // What has to be contained is the pressure's **last move** rather than its
    // last step: Fracture's one effect is the break at the crisis entry, which
    // falls at the sum of the dwells before it.
    let chapter = chapter();
    let fracture = chapter
        .pressure_schedule
        .iter()
        .find(|entry| entry.pressure == Pressure::Fracture)
        .expect("the chapter schedules Fracture");
    assert!(fracture.primary, "and it is the chapter's own headline pressure");
    assert_eq!(fracture.target.kind, TargetKind::Route, "aimed at a Route the chapter placed");
    assert_eq!(fracture.target.id, Some(1), "the line from the Form to the intake");

    let table = authored()
        .pressures
        .table(Pressure::Fracture)
        .expect("the manifest authors Fracture's table")
        .clone();
    // The dwells before the crisis entry, read off the authored table rather
    // than written down here, so a re-tuned table moves this window without
    // moving a figure in this file.
    let to_crisis: Step = table
        .stages
        .iter()
        .take_while(|stage| stage.stage != field_game_core::pressure::Stage::Crisis)
        .map(|stage| stage.steps as Step)
        .sum();
    let objective = "objective.the_rewrite.carry_the_supply";

    println!("\nthe objective Fracture is written for, on all four paths (steps from the opening)");
    println!(
        "{:<26} {:>10} {:>12} {:>10} {:>10} {:>9}",
        "path", "offered", "completed", "break", "opening", "closing",
    );
    let mut latest_offer = 0;
    let mut earliest_close = Step::MAX;
    let mut least_opening = Step::MAX;
    let mut least_closing = Step::MAX;
    for (named, allowances, takes) in [
        ("floor, spare taken", false, true),
        ("floor, spare let go", false, false),
        ("attentive, spare taken", true, true),
        ("attentive, spare let go", true, false),
    ] {
        let run = drive_as("thread", Path::ByParts, allowances, takes);
        let offered = run.offered_at(objective).expect("the objective is offered")
            - run.opening_step;
        let completed = run.at(objective).expect("and completed") - run.opening_step;
        let broke = run.line_break_step.expect("Fracture broke the line") - run.opening_step;
        let opening = fracture.start_step - offered;
        let closing = completed - (fracture.start_step + to_crisis);
        println!("{named:<26} {offered:>10} {completed:>12} {broke:>10} {opening:>10} {closing:>9}");
        latest_offer = latest_offer.max(offered);
        earliest_close = earliest_close.min(completed);
        least_opening = least_opening.min(opening);
        least_closing = least_closing.min(closing);
        assert!(
            fracture.start_step > offered && fracture.start_step + to_crisis < completed,
            "{named}: Fracture runs from {} to {}, outside the objective it was written for \
             ({offered} to {completed})",
            fracture.start_step,
            fracture.start_step + to_crisis,
        );
    }
    println!(
        "the binding window is {latest_offer} to {earliest_close}; Fracture runs {} to {}, \
         with margins {least_opening} and {least_closing}",
        fracture.start_step,
        fracture.start_step + to_crisis,
    );
    assert!(latest_offer < earliest_close, "the four paths leave no span they all stand inside");
    // A schedule that has drifted to the edge of one path reads exactly like
    // one that has not, so the margin is asserted rather than printed.
    assert!(
        least_opening >= PRESSURE_MARGIN && least_closing >= PRESSURE_MARGIN,
        "the tightest margins are {least_opening} and {least_closing}, under the {PRESSURE_MARGIN} \
         step floor this wave holds a schedule to",
    );
}

#[test]
fn fracture_seats_itself_and_the_break_it_makes_has_teeth() {
    // Two things, and they are different claims. **Seating**: the pressure is
    // admitted, stands through its four stages, and is the one primary the
    // schedule names. **Teeth**: what it does costs the run something a
    // measurement can name — here, the line the whole run is supplied by is
    // gone, and the dependency stops carrying inside a few hundred steps
    // unless it is drawn again.
    let run = attentive();
    println!("\nFracture, stage by stage (steps from the chapter's opening)");
    println!("{:<12} {:>10}", "stage", "step");
    for (step, named, stage) in &run.pressure_steps {
        println!("{:<12} {:>10}  {named}", stage, step - run.opening_step);
    }
    let stages: std::collections::BTreeSet<&str> =
        run.pressure_steps.iter().map(|held| held.2.as_str()).collect();
    assert!(
        stages.contains("signal") && stages.contains("pressure") && stages.contains("crisis"),
        "Fracture stood through its own stages: {stages:?}",
    );
    assert!(
        run.pressure_steps.iter().all(|held| held.1 == "fracture"),
        "and it is the only pressure the chapter admits",
    );

    // The teeth: a run that answers the break keeps the dependency, and one
    // that never answers it does not. `no_repair` plays the identical script
    // and simply never draws the line again.
    let broke = run.line_break_step.expect("Fracture broke the line") - run.opening_step;
    let repair = run
        .commits
        .iter()
        .find(|held| held.label == "draw the line again")
        .expect("the run answered it");
    println!(
        "the line broke at {broke} and was drawn again at {}",
        repair.step - run.opening_step,
    );
    let unanswered = drive_no_repair();
    // How long the run stands on what it banked, measured rather than modelled:
    // the intake, the two carriers and the junction all still hold Charge when
    // Fracture takes the line, and the dependency goes on carrying out of them.
    // This is a different figure from the module break's, where the cut falls
    // between the carriers and the junction and only the junction's own bank is
    // left — the copy for the two objectives says so separately.
    let broke_unanswered =
        unanswered.line_break_step.expect("Fracture broke the line") - unanswered.opening_step;
    let stood = unanswered
        .dependency_stopped_step
        .map(|held| held - unanswered.opening_step - broke_unanswered);
    println!(
        "left unanswered: {} objectives completed, transition {:?}; the dependency stood \
         {stood:?} steps past the break",
        unanswered.completed.len(),
        unanswered.transition_step,
    );
    // The figure `explanation.the_rewrite.carry_the_supply` states, pinned so
    // the sentence and the Field move together. Sampled once a batch, so the
    // true stop stands inside the 15 steps before this reading: measured 105.
    let stood = stood.expect("the dependency stopped once nothing supplied it");
    assert!(
        (75..=135).contains(&stood),
        "the dependency stood {stood} steps past Fracture's break, which is not the hundred \
         the copy for carry_the_supply says it stands",
    );
    assert!(
        unanswered
            .completed
            .contains(&"objective.the_rewrite.take_the_deep_band".to_string()),
        "the run got as far as the deep band before the break",
    );
    assert!(
        !unanswered
            .completed
            .contains(&"objective.the_rewrite.carry_the_supply".to_string()),
        "and no further: nothing supplies the run once the line is gone, so the pattern \
         cannot be held for the span the objective names: {:?}",
        unanswered.completed,
    );
    assert!(unanswered.transition_step.is_none(), "the chapter does not complete");
}

#[test]
fn a_rewrite_begun_too_late_is_the_one_that_loses_continuity() {
    // The claim the goal's own sentence makes — "without losing continuity" —
    // is only worth something if continuity can be lost. It can: once the
    // module's own Route is cut the junction has **no supply path at all**, and
    // it spends six a step out of the thousand it banked. So what the late run
    // pays for is the *delay* and nothing else. Walking out of the band is how
    // this test spends the delay, not why it costs: after the cut the junction
    // takes nothing in wherever the Form stands, and a run that stood still in
    // the band for the same nine hundred steps would arrive to find exactly the
    // same stopped dependency. The hold count starts again from nothing, and
    // the chapter takes longer for it.
    //
    // The unprepared run makes the *same four changes* as the parts swap. What
    // differs is only when it makes them, which is what isolates continuity
    // from the shape of the rewrite.
    let prepared = drive_as("thread", Path::ByParts, false, true);
    let late = drive_as("thread", Path::Unprepared, false, true);
    println!("\nthe same rewrite, made at once and made after a walk");
    println!(
        "{:<12} {:>9} {:>8} {:>8} {:>10} {:>10} {:>9} {:>14}",
        "run", "changes", "commits", "steps", "restarts", "stopped", "Impulse", "stood past cut",
    );
    for (named, run, path) in
        [("at once", &prepared, Path::ByParts), ("after a walk", &late, Path::Unprepared)]
    {
        println!(
            "{named:<12} {:>9} {:>8} {:>8} {:>10} {:>10} {:>9} {:>14}",
            path.changes(),
            run.commits.iter().filter(|held| held.label.starts_with("rewrite")).count(),
            run.span(),
            run.continuity_breaks,
            run.stopped_batches,
            run.impulse,
            match (run.module_break_step, run.module_stopped_step) {
                (Some(cut), Some(stopped)) => format!("{}", stopped - cut),
                _ => "never".to_string(),
            },
        );
    }
    assert_eq!(prepared.continuity_breaks, 0, "the prepared rewrite never restarts the count");
    assert_eq!(prepared.stopped_batches, 0, "and the dependency never stops carrying");
    assert!(
        late.continuity_breaks > 0,
        "the late rewrite made the same changes and lost nothing: {:?}",
        late.continuity_breaks,
    );
    assert!(
        late.stopped_batches > 0,
        "the dependency never stopped carrying on the late run",
    );
    // The other figure the copy states — this one for `rewrite_the_module`,
    // where the cut leaves the junction with no supply path at all and only its
    // own bank to spend. Measured 76 steps, sampled once a batch, so the true
    // stop stands inside the 15 steps before it.
    let stood = late.module_stopped_step.expect("the dependency stopped")
        - late.module_break_step.expect("the module's Route was cut");
    assert!(
        (50..=105).contains(&stood),
        "the dependency stood {stood} steps past the module's break, which is not the \
         seventy the copy for rewrite_the_module says it stands",
    );
    assert!(
        late.span() > prepared.span(),
        "the late run took {} steps against the prepared run's {}",
        late.span(),
        prepared.span(),
    );
    assert!(late.transition_step.is_some(), "and it still completes the chapter");
    assert_eq!(
        late.impulse, prepared.impulse,
        "the two runs spend the same changes, so the Impulse they carry out is the same: \
         what the walk cost is the continuity, not the budget",
    );
}

// ---------------------------------------------------------------------------
// The still surface, which is what the chapter teaches to read
// ---------------------------------------------------------------------------

#[test]
fn the_still_surface_reads_the_run_and_a_commit_ends_the_window_it_reads_over() {
    // The chapter's own instrument. Every reading below is taken through the
    // locked surface — a `coordinates_full` inspection answered in Still Mode
    // and nowhere else — at three moments the chapter authors: the module
    // carrying, the module broken, and the module rewritten.
    //
    // Two properties are what the chapter is teaching, and they pull opposite
    // ways:
    //
    // - **Swap Range reads the Field's own shape**, so it answers at every one
    //   of the three. It is the reading that says whether the members of the
    //   standing inside are joined to each other by anything that is carrying.
    // - **The two replay-based readings run over the retained window**, and a
    //   commit clamps that window to nothing. So the moment after a rewrite
    //   they are honestly unassigned, and the span regrows a step at a time.
    //   Reading before the change is the only way to read at all.
    let run = attentive();
    println!("\nthe run read through the still surface, at the chapter's four moments");
    println!(
        "{:<24} {:>9} {:>8} {:>12} {:>12} {:>14} {:>14}  {}",
        "moment", "step", "window", "Swap Range", "Turnover", "Separation", "Self-Support", "inside",
    );
    for reading in &run.readings {
        println!(
            "{:<24} {:>9} {:>8} {:>12} {:>12} {:>14} {:>14}  {:?}",
            reading.named,
            reading.step - run.opening_step,
            reading.window,
            format!("{:?}", reading.swap_range),
            format!("{:?}", reading.turnover),
            format!("{:?}", reading.separation),
            format!("{:?}", reading.self_support),
            reading.view_members,
        );
    }
    assert_eq!(run.readings.len(), 4, "the run stopped to read four times");

    let carrying = run.reading("the module carrying").expect("the first reading");
    // The module is a chain: the intake, two carriers, the junction, joined by
    // Routes that are carrying. Only the two ends of a chain can be spared, so
    // Swap Range is two — and it is two because the members stand further apart
    // than the locked adjacency, so what joins them in the reading is the
    // carrying rather than the placement.
    assert_eq!(
        carrying.swap_range,
        Some(2),
        "a chain of four spares its two ends and no more",
    );
    assert!(
        carrying.turnover.is_some(),
        "the flows saturate, so Turnover Tolerance has a window to replay over",
    );
    assert!(
        carrying.separation.is_some(),
        "and so does Instruction Separation",
    );
    // Self-Support is unassigned only when nothing was required, so a reading
    // at all says the Nodes of the run pay upkeep. That it reads zero says the
    // rest: what pays for the run arrives from outside the View, which is
    // the Form standing in the band and nothing else.
    assert_eq!(
        carrying.self_support,
        Some(0),
        "the run's upkeep is paid entirely out of what the surround supplies",
    );

    // Taken on the step the last change lands, before a batch has run.
    let rewritten = run.reading("the module rewritten").expect("the third reading");
    assert!(
        rewritten.window < 2,
        "a commit clamps the retained span, so the window a profile reads over is {} here",
        rewritten.window,
    );
    assert_eq!(
        rewritten.turnover,
        None,
        "and a window under two steps has nothing for a replay to replay over",
    );
    assert_eq!(rewritten.separation, None, "the same for Instruction Separation");
    assert!(
        rewritten.swap_range.is_some(),
        "Swap Range reads the Field's own shape rather than the window, so it still answers",
    );

    // And the span regrows: nine hundred steps later the same request answers
    // whole again. Re-accumulation is the cost of committing, and it is paid in
    // steps rather than in Impulse.
    let settled = run.reading("the run settled").expect("the fourth reading");
    assert!(
        settled.window >= 2,
        "the retained span regrew to {} steps",
        settled.window,
    );
    assert!(settled.turnover.is_some(), "so Turnover Tolerance answers again");
    assert!(settled.separation.is_some(), "and so does Instruction Separation");
    assert_eq!(
        settled.swap_range,
        Some(4),
        "the Observation View was left where it stood while the components changed, so the \
         four it names are joined to each other by nothing that carries",
    );
}

// ---------------------------------------------------------------------------
// The authored breaks, the optional test, and the setback
// ---------------------------------------------------------------------------

#[test]
fn the_authored_break_of_the_module_lands_at_its_own_step_on_every_path() {
    // The chapter's two events are timed against an objective rather than
    // against the run's step counter, so they land at the same offset in a run
    // that took seventy minutes to reach them and one that took sixty.
    let chapter = chapter();
    assert_eq!(chapter.events.len(), 2, "the chapter authors two events");
    let cut = chapter
        .events
        .iter()
        .find(|held| matches!(held.effect, content::Effect::RouteCut { .. }))
        .expect("one of them severs the module's own Route");
    assert!(
        matches!(cut.effect, content::Effect::RouteCut { route: 4 }),
        "and the Route it severs is the one between the two carriers",
    );
    println!("\nthe module's own break, and the step each run read it at");
    println!("{:<24} {:>10} {:>8} {:>8} {:>6}", "path", "offered", "due", "read", "late");
    for (named, run) in [("the authored floor", floor()), ("the attentive run", attentive())] {
        let offered = run.offered_at(&cut.objective).expect("the objective was offered");
        let landed = run.module_break_step.expect("the Route was severed");
        let due = offered + cut.at;
        println!(
            "{named:<24} {:>10} {:>8} {:>8} {:>6}",
            offered - run.opening_step,
            due - run.opening_step,
            landed - run.opening_step,
            landed - due,
        );
        // What is exact is the trigger: `at` steps after the objective was
        // offered, on one boundary. What is read here is not, and the slack is
        // the sampler's — this file drives in fifteen-step batches and reads the
        // Field once at the end of each — so a landing is placed to within one
        // batch of its own step.
        assert!(
            landed >= due && landed < due + Step::from(BATCH),
            "{named}: the break was read at {landed}, not inside the batch its own step \
             ({due}) falls in",
        );
    }
    // The cut is permanent: nothing in the chapter brings that Route back, and
    // no run ends with it standing.
    let run = floor();
    assert!(
        !run.standing_routes.iter().any(|(route, _, _)| *route == 4),
        "the severed Route stands again at the close: {:?}",
        run.standing_routes,
    );
}

#[test]
fn the_optional_test_is_a_test_of_skill_that_is_passable_and_skippable() {
    let chapter = chapter();
    let tests: Vec<&content::Objective> =
        chapter.objectives.iter().filter(|held| held.optional.is_some()).collect();
    assert_eq!(tests.len(), 1, "the chapter authors one optional test");
    let test = tests[0].id.clone();
    // It asks for a Pulse released beside a Port that stands below the deep
    // band, further than the longest Pulse reaches — so a Form standing in the
    // band never meets it by accident, which is the shape an optional test has
    // to have or a run that does nothing would pass it.
    assert!(matches!(tests[0].condition, content::Condition::PortsOpen { .. }));

    let taken = drive_as("thread", Path::ByParts, false, true);
    assert!(taken.deep_spare_open, "the test is passable");
    assert!(taken.completed.contains(&test), "and a taken test is a completed objective");
    assert!(taken.transition_step.is_some(), "and the chapter completes");

    let let_go = drive_as("thread", Path::ByParts, false, false);
    assert!(!let_go.deep_spare_open, "the test is skippable");
    assert!(!let_go.completed.contains(&test), "a test that was not taken was not completed");
    assert!(let_go.transition_step.is_some(), "and the chapter completes without it");
    // The span it stood for is the authored one: the sequence installs what
    // follows it at `started_step + span`.
    let offered = let_go.offered_at(&test).expect("the test was offered");
    let after = let_go
        .offered_at("objective.the_rewrite.carry_the_supply")
        .expect("and the objective after it stands");
    assert_eq!(
        after,
        offered + tests[0].optional.expect("a span"),
        "the pass-over lands at the step the span names",
    );
    println!(
        "\nthe optional test: taken, the chapter takes {} steps; let go, {} steps",
        taken.span(),
        let_go.span(),
    );
}

#[test]
fn the_authored_setback_stands_and_the_chapter_carries_past_it() {
    // The chapter's authored break: the two Nodes of the dependency have
    // nowhere to send what arrives, so they carry more than they can hold and
    // the objective reads `failed_recoverable` while they do. Drawing an
    // outfall from each of them carries it through. Nothing in the closed state
    // set is terminal and the run never leaves `running`.
    let run = attentive();
    let collapse = *run.setbacks.first().expect("the authored break is reached");
    let recovery = run.recovery_step.expect("and the sequence stands again after it");
    assert!(recovery > collapse);
    let setback = run
        .stages
        .iter()
        .rposition(|stage| stage == "failed_recoverable")
        .expect("the break stood");
    assert!(
        run.stages[setback + 1..].iter().any(|stage| stage == "active"),
        "the sequence stands active again after the last setback",
    );
    assert!(
        run.stages[setback + 1..].iter().any(|stage| stage == "complete"),
        "and completes past it",
    );
    println!(
        "\nthe chapter stood in a recoverable setback {} times, and in no other state",
        run.setbacks.len(),
    );
    assert_eq!(
        run.setbacks.len(),
        1,
        "the chapter authors one setback, and it stands once: {:?}",
        run.setbacks,
    );
    let minute = 60 * STEPS_PER_SECOND;
    let opening = collapse - run.opening_step;
    assert!(
        opening <= 14 * minute,
        "the recoverable setback is by minute fourteen, and was at {opening}",
    );
}

// ---------------------------------------------------------------------------
// What the chapter authors
// ---------------------------------------------------------------------------

#[test]
fn the_drawn_routes_take_the_numbers_the_paths_are_written_against() {
    // Identifiers are handed out in order and never reused, which is what lets
    // one of the rewrite paths move an end of a Route it drew itself. The
    // chapter authors seven, so the two outfalls are 8 and 9, the line drawn
    // again after Fracture is 10, and the finale's own changes start at 11.
    // This is the least discoverable authored dependency in the chapter: a
    // Route added to the content, or an outfall drawn in a different order,
    // silently moves every number the finale is written against.
    let chapter = chapter();
    assert_eq!(chapter.routes.len(), 7, "the chapter places seven Routes");
    let run = drive_as("thread", Path::ByParts, false, true);
    let held = |route: u32| -> (u32, u32) {
        run.standing_routes
            .iter()
            .find(|(id, _, _)| *id == route)
            .map(|(_, tail, head)| (*tail, *head))
            .unwrap_or_else(|| panic!("route {route} stands at the close: {:?}", run.standing_routes))
    };
    println!("\nthe Routes standing at the close: {:?}", run.standing_routes);
    assert_eq!(held(FIRST_DRAWN), (6, 8), "the first outfall");
    assert_eq!(held(FIRST_DRAWN + 1), (7, 8), "the second outfall");
    assert_eq!(held(REDRAWN_LINE), (1, 2), "the line to the intake, drawn again");
    assert_eq!(
        held(FIRST_REWRITE),
        (9, 10),
        "and the finale's own first Route, after the second commit moved its tail",
    );
    // The two Routes the chapter's hold objectives name keep their own
    // identifiers through every change, which is what makes the pattern's
    // identity persist across the substitution rather than being rebuilt.
    for route in [6u32, 7] {
        assert!(
            run.standing_routes.iter().any(|(id, _, _)| *id == route),
            "the dependency's Route {route} stands at the close under its own identifier",
        );
    }
}

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
    let optional: i64 =
        chapter.objectives.iter().filter_map(|held| held.optional).map(i64::from).sum();
    let total: i64 = spans.iter().map(|(_, _, span)| span).sum::<i64>() + optional;
    println!("\nThe Rewrite — authored duration, by objective");
    for (id, target, span) in &spans {
        println!("  {id:<52} authored {target:>6}  spans {span:>6}");
    }
    println!("  {:<52} {:>15} {optional:>12}", "the optional test's own span", "");
    println!("  {:<52} {:>15} {total:>12}", "in all", "");
    assert_eq!(spans.len(), 5, "five of the eight ask for time: {spans:?}");
    // The chapter's authored duration alone, before a step of travel, a Pulse,
    // or a queued change: a little under sixty-six minutes of the seventy.
    assert!(
        total > 114_000 && total < 124_000,
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
    // One at a time, asserted rather than assumed: the objective line carries
    // exactly one of this chapter's objectives on every step the chapter
    // stands, so a run that ever held two would fail here rather than pass for
    // having held each of them at some point.
    for (step, standing) in &run.standing {
        assert!(
            standing.len() <= 1,
            "the stream left {} of this chapter's objectives live at once at step {step}: \
             {standing:?}",
            standing.len(),
        );
    }
    assert!(
        run.standing.iter().any(|(_, standing)| standing.len() == 1),
        "the stream never left an objective of this chapter live at all",
    );
    let order = authored_order();
    for id in &order {
        assert!(run.offered_at(id).is_some(), "{id} was offered");
    }
    assert_eq!(run.completed, order);
    // The last thing the run stood on is the next chapter's opening objective,
    // offered at the transition: the line clears only at the end of the
    // campaign.
    assert_eq!(run.stages.last().map(String::as_str), Some("active"));
    let mut last = 0;
    for (id, step) in &run.offered {
        if !id.starts_with("objective.the_rewrite.") {
            continue;
        }
        assert!(*step >= last, "{id} was offered out of the authored order");
        last = *step;
    }
}

#[test]
fn the_shared_campaign_script_carries_the_chapter_to_its_own_boundary() {
    // The campaign run drives every chapter from `support::campaign`'s own
    // phase list rather than from this file's, and The Rewrite is not
    // rest-completable: the two outfalls, the line Fracture breaks, and the
    // rewrite itself are all queued changes, and the two Nodes of the
    // dependency are opened by Pulses. This is the arm that chapter
    // contributes, driven end to end so that a campaign run reaching it finds
    // a script that works.
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
    assert_eq!(support::campaign::chapter_index(&session), CHAPTER);

    let script =
        support::campaign::script_for("the_rewrite").expect("the chapter authors a script");
    let stop = |session: &Session| support::campaign::chapter_index(session) > CHAPTER;
    for phase in &script {
        if stop(&session) {
            break;
        }
        driver.phase_until(&mut session, phase, &stop);
    }
    assert!(
        stop(&session),
        "the shared campaign script completes The Rewrite and carries the run past it",
    );
    // And it lets the optional test go, which is what keeps a driven campaign's
    // completed list the campaign's required objectives exactly.
    let state = session.run().expect("a run").state();
    assert!(
        !state
            .progress
            .complete
            .iter()
            .any(|id| id == "objective.the_rewrite.open_the_deep_spare"),
        "the shared script passes over the optional test rather than taking it",
    );
}
