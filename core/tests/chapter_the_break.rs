//! The Break, driven end to end: the whole sixty-minute chapter.
//!
//! The chapter's one subject is **Fracture and recovery**. Its systems are:
//!
//! - **A trunk the player draws.** Nothing carries Charge from the intake into
//!   the run until the player connects one in Still Mode, and there are two
//!   lines to connect it to. Whichever they connect is the Route the whole run
//!   then flows through, so the most-used Route on this field is the player's
//!   own choice rather than the author's.
//! - **A rehearsal.** Half way through, an authored `set_route_cut` severs the
//!   junction's own run into the core. The run starves, the junction carries
//!   more than it can hold, and the objective reads `failed_recoverable` until
//!   the player brings the line onto the core themselves. That is the chapter
//!   teaching the move it is about to demand.
//! - **The crisis.** Fracture is scheduled with **no target at all**, so the
//!   locked reading breaks the standing Route with the largest trailing flow —
//!   the trunk the player drew and has been running on. The severance is
//!   permanent.
//! - **Three recoveries.** Rerouting (one Route moved onto the intake), reserve
//!   use (the spare store, filled before the crisis, connected into the core),
//!   and deliberate separation (the Route into the tail cut and the inside
//!   reshaped to what the dim current alone can carry). Each completes the
//!   chapter; the run that
//!   does none of them does not.
//!
//! Two runs are driven, and they answer two different questions:
//!
//! - The **attentive run** carries a modelled first-time player, spending the
//!   named discovery allowances. Its milestone table is what the sixty minutes
//!   are read against.
//! - The **authored floor** spends no allowance at all. It is the run the eight
//!   Forms and the pre-crisis configurations are driven through.
//!
//! **The coverage claim.** `the_chapter_recovers_from_every_pre_crisis_
//! configuration` drives the enumerated configuration space — trunk choice ×
//! optional test × recovery — and `every_starting_form_completes_the_whole_
//! chapter` drives the Form axis. The partition argument is in the report: only
//! the trunk choice and the optional test change *what recovery is available*;
//! the Form and the allowance setting change *when* things happen, not what the
//! Field affords.

use field_game_core::content::{self, CONTENT_VERSION};
use field_game_core::fx::ONE_UNIT;
use field_game_core::pressure::{Pressure, TargetKind};
use field_game_core::state::{Step, STEPS_PER_SECOND};
use field_game_core::Session;

mod support;

use support::campaign::{controlled, nearest_point, toward};

const KEY: &str = "00112233445566bb";

/// How many steps one scripted batch runs before the script re-aims.
const BATCH: u16 = 15;

/// How many steps a full Pulse charge takes, from the locked charging rule.
const FULL_CHARGE_STEPS: u32 = 32;

/// How long a ramp takes, in microseconds. Locked.
const RAMP_US: i64 = 250_000;

/// This chapter's place in the authored campaign.
const CHAPTER: u8 = 5;

/// How long a driven run will wait for a chapter that never completes before it
/// gives up. Only the do-nothing baseline ever reaches it.
const GIVE_UP: Step = 125_000;

/// What one phase asks of the run.
#[derive(Clone, Debug)]
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
    /// Enter Still Mode, queue every plan body, commit, and ramp back out.
    Edit(Vec<String>),
}

/// What ends a phase early.
///
/// The joins between the script and the chapter are stated as readings of the
/// run rather than as step counts, because the two clocks are different: an
/// objective is offered when a player reaches it, and an authored event or a
/// scheduled pressure lands on a step of its own. A phase that ended on a count
/// would drift out of the beat it was written for the moment a span was retuned.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Until {
    /// Nothing: the phase runs its whole length.
    Never,
    /// The named objective — the tail of its id — has been offered.
    Offered(&'static str),
    /// The authored rehearsal cut has landed.
    Rehearsal,
    /// Fracture has taken the trunk.
    Cut,
}

/// One phase of the script: what it does, how long it runs at most, whether its
/// length is a modelled allowance, and what ends it early.
#[derive(Clone, Debug)]
struct Phase {
    label: &'static str,
    act: Act,
    steps: u32,
    allowance: bool,
    until: Until,
}

fn play(label: &'static str, act: Act, steps: u32) -> Phase {
    Phase { label, act, steps, allowance: false, until: Until::Never }
}

fn allow(label: &'static str, act: Act, steps: u32) -> Phase {
    Phase { label, act, steps, allowance: true, until: Until::Never }
}

/// A phase that ends on a reading of the run rather than on its own count.
fn hold(label: &'static str, act: Act, steps: u32, until: Until) -> Phase {
    Phase { label, act, steps, allowance: false, until }
}

// ---------------------------------------------------------------------------
// The configuration space
// ---------------------------------------------------------------------------

/// Which line the player connects the intake to — the pre-crisis choice that
/// decides which Route the run flows through, and so which Route the crisis
/// takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Trunk {
    /// The near line, node 3.
    Near,
    /// The far line, node 4.
    Far,
    /// Both, drawn at the same visit: two Impulse spent on redundancy.
    Both,
}

const TRUNKS: [Trunk; 3] = [Trunk::Near, Trunk::Far, Trunk::Both];

impl Trunk {
    fn name(self) -> &'static str {
        match self {
            Trunk::Near => "near",
            Trunk::Far => "far",
            Trunk::Both => "both",
        }
    }

    /// The queued change that carries the intake into the run.
    fn drawn(self) -> Vec<String> {
        match self {
            Trunk::Near => vec![connect(2, 3)],
            Trunk::Far => vec![connect(2, 4)],
            Trunk::Both => vec![connect(2, 3), connect(2, 4)],
        }
    }

    /// The queued change that answers the rehearsal: the line the player is
    /// running on is moved onto the core, past the junction the authored cut
    /// stranded.
    fn rehearsal(self) -> Vec<String> {
        match self {
            Trunk::Near => vec![redirect(1, "head", 5)],
            Trunk::Far => vec![redirect(3, "head", 5)],
            Trunk::Both => vec![redirect(1, "head", 5), redirect(3, "head", 5)],
        }
    }

    /// Where the line this trunk carries into stands, in whole units.
    fn line(self) -> (i64, i64) {
        match self {
            Trunk::Near | Trunk::Both => (1300, 1770),
            Trunk::Far => (1620, 1740),
        }
    }
}

/// What the player does once the crisis has taken the trunk.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Recovery {
    /// Nothing at all: the baseline the other three are read against.
    None,
    /// One Route moved: the line the run was on is taken off the lost trunk and
    /// put on the intake itself.
    Reroute,
    /// The spare store, filled before the crisis, connected into the core.
    Reserve,
    /// The Route into the tail cut and the inside reshaped: no new supply at
    /// all, and what is left runs on the dim current alone.
    Separate,
}

const RECOVERIES: [Recovery; 3] = [Recovery::Reroute, Recovery::Reserve, Recovery::Separate];

impl Recovery {
    fn name(self) -> &'static str {
        match self {
            Recovery::None => "nothing",
            Recovery::Reroute => "reroute",
            Recovery::Reserve => "reserve",
            Recovery::Separate => "separate",
        }
    }

    /// The queued changes this recovery commits, against the trunk the run was
    /// drawn with.
    fn plans(self, trunk: Trunk) -> Vec<String> {
        match self {
            Recovery::None => Vec::new(),
            Recovery::Reroute => match trunk {
                Trunk::Near | Trunk::Both => vec![redirect(1, "tail", 2)],
                Trunk::Far => vec![redirect(3, "tail", 2)],
            },
            Recovery::Reserve => vec![connect(9, 5)],
            Recovery::Separate => vec![cut(6), reshape(&[5, 6])],
        }
    }
}

fn connect(from: u32, to: u32) -> String {
    format!("{{\"plan\":{{\"from\":{from},\"op\":\"connect\",\"to\":{to}}}}}")
}

fn redirect(route: u32, end: &str, to: u32) -> String {
    format!("{{\"plan\":{{\"end\":\"{end}\",\"op\":\"redirect\",\"route\":{route},\"to\":{to}}}}}")
}

fn cut(route: u32) -> String {
    format!("{{\"plan\":{{\"op\":\"cut\",\"route\":{route}}}}}")
}

fn reshape(members: &[u32]) -> String {
    let written: Vec<String> = members.iter().map(|held| held.to_string()).collect();
    format!("{{\"plan\":{{\"members\":[{}],\"op\":\"reshape_boundary\"}}}}", written.join(","))
}

// ---------------------------------------------------------------------------
// The script
// ---------------------------------------------------------------------------

/// The scripted run, phase by phase.
///
/// The positions are the authored content's own — the supply band, the intake
/// standing in it, the two lines, the junction and core below them, and the
/// spare store off to the south-east. Every phase that holds the supply follows
/// the band's nearest path point read off the Field rather than a coordinate
/// off the file.
///
/// `trunk` decides which line the run is connected to, `takes_the_test` whether
/// the spare store is opened or let go, and `recovery` what the player does once
/// the crisis has taken the trunk. The rest of the script is identical across
/// all of them, so a difference between two driven runs is a difference the
/// player's own choices made.
///
/// `draws` false is the no-Still-Mode run: it opens every Port the chapter asks
/// for and holds the band for as long as the driver will let it, and queues
/// nothing at all. Nothing else about it differs.
fn script(trunk: Trunk, takes_the_test: bool, recovery: Recovery, draws: bool) -> Vec<Phase> {
    let (line_x, line_y) = trunk.line();
    let queued = |plans: Vec<String>| -> Act {
        Act::Edit(if draws { plans } else { Vec::new() })
    };
    let mut phases = vec![
        allow("read the surface", Act::Rest, 300),
        play("steer into the supply band", Act::Toward(1400, 1500), 300),
        hold("hold the supply", Act::Follow(1), 25_000, Until::Offered("open_the_intake")),
        allow("try the controls", Act::Follow(1), 900),
        // The intake stands in the band, so the Form is already beside it.
        play("charge at the intake", Act::Charge(1400, 1500), FULL_CHARGE_STEPS),
        play("release", Act::Release, 1),
        allow("look for what stands below", Act::Toward(1370, 1620), 900),
        play("cross to the near line", Act::Toward(1330, 1760), 420),
        play("charge", Act::Charge(1330, 1760), FULL_CHARGE_STEPS),
        play("release", Act::Release, 1),
        play("cross to the far line", Act::Toward(1600, 1745), 420),
        play("charge", Act::Charge(1600, 1745), FULL_CHARGE_STEPS),
        play("release", Act::Release, 1),
        allow("look for the rest of the run", Act::Toward(1520, 1800), 900),
        play("cross to the junction and the core", Act::Toward(1450, 1860), 420),
        play("charge", Act::Charge(1450, 1860), FULL_CHARGE_STEPS),
        play("release", Act::Release, 1),
        allow("look for what the run is missing", Act::Toward(1420, 1700), 1800),
        play("draw the trunk", queued(trunk.drawn()), 0),
        play("back to the band", Act::Toward(1400, 1500), 500),
        // The authored rehearsal cut lands 4,500 steps into this hold.
        hold("hold the run", Act::Follow(1), 30_000, Until::Rehearsal),
        play("carry the starved run", Act::Follow(1), 240),
        allow("look for what stopped", Act::Toward(line_x, line_y), 1_500),
        play("bring the line onto the core", queued(trunk.rehearsal()), 0),
        play("back to the band", Act::Toward(1400, 1500), 500),
        hold(
            "hold the run past the rehearsal",
            Act::Follow(1),
            40_000,
            Until::Offered("open_the_store"),
        ),
    ];
    if takes_the_test {
        phases.extend([
            allow("look for the spare store", Act::Toward(1600, 1900), 600),
            play("cross to the spare store", Act::Toward(1700, 2050), 900),
            play("charge", Act::Charge(1700, 2050), FULL_CHARGE_STEPS),
            play("release", Act::Release, 1),
            play("back to the band", Act::Toward(1400, 1500), 900),
        ]);
    } else {
        phases.push(play("let the spare store go", Act::Follow(1), 5_600));
    }
    phases.extend([
        hold(
            "carry the run",
            Act::Follow(1),
            40_000,
            Until::Offered("hold_past_the_break"),
        ),
        // The crisis is chapter-absolute, so this phase ends on the break itself
        // rather than on a step count: every path meets it where its own pace
        // put it.
        hold("hold until the trunk is taken", Act::Follow(1), 60_000, Until::Cut),
        allow("look for what broke", Act::Toward(1400, 1620), 1_500),
        allow("read what the run still has", Act::Toward(1450, 1800), 1_200),
        play("recover", queued(recovery.plans(trunk)), 0),
        play("back to the band", Act::Toward(1400, 1500), 500),
        play("hold past the break", Act::Follow(1), 30_000),
        play("hold past the break", Act::Follow(1), 30_000),
    ]);
    phases
}

// ---------------------------------------------------------------------------
// What a driven run records
// ---------------------------------------------------------------------------

/// One milestone the run reached, and the step it reached it at.
#[derive(Clone, Debug)]
struct Milestone {
    name: String,
    step: Step,
}

/// One reading taken while Fracture stood active.
#[derive(Clone, Debug)]
struct Sample {
    step: Step,
    stage: String,
    /// Every standing Route's identifier at the reading.
    routes: Vec<u32>,
}

/// What one driven run recorded.
struct Driven {
    trunk: Trunk,
    recovery: Recovery,
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
    /// Every step at which the sequence entered a recoverable setback.
    setbacks: Vec<Step>,
    recovery_step: Option<Step>,
    transition_step: Option<Step>,
    /// The step the authored `set_route_cut` was first seen on, and the Route
    /// set before and after it.
    rehearsal_step: Option<Step>,
    /// The step Fracture's own break was first seen on, and what it took.
    cut_step: Option<Step>,
    cut_route: Option<u32>,
    /// Every step at which Fracture was observed standing.
    pressed: Vec<Step>,
    /// The stages Fracture was observed standing in, in order.
    pressure_steps: Vec<(Step, String)>,
    /// Readings taken under Fracture.
    samples: Vec<Sample>,
    /// The Route this file's own recomputation of the trailing-flow ranking
    /// answered at the last batch before the break.
    ranked: Option<u32>,
    /// The same ranking with the runner-up's sum, for the margin.
    ranked_margin: Option<(u32, i64, u32, i64)>,
    /// The outlet's flow, sampled every batch of the final challenge, in raw
    /// units, with whether the crisis had landed by then.
    outlet: Vec<(Step, bool, i64)>,
    /// Whether the spare store stood open.
    store_open: bool,
    /// What the standing inside held at the close, in whole units.
    inside_units: i64,
    /// The standing inside at the close.
    inside: Vec<u32>,
    /// What the spare store held at the close, in whole units.
    store_units: i64,
    /// What the far store held at the close, in whole units.
    bank_units: i64,
    /// What the run leaked on the last batch, recomputed by the Boundary
    /// leakage rule off the Field, in raw units.
    leak_now: i64,
    form_units: i64,
    forms: usize,
    routes: usize,
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
}

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
                if id.starts_with("objective.the_break.") {
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

/// Sends one frame body.
fn frame(
    session: &mut Session,
    seq: &mut u32,
    steps: u16,
    steer: (i64, i64),
    held: bool,
    release: bool,
    t_us: i64,
    toggle: bool,
) {
    let body = format!(
        "{{\"advance_steps\":{steps},\"depth_key\":0,\"inspect\":null,\
         \"pause\":false,\"pulse_held\":{held},\"pulse_release\":{release},\
         \"seq\":{},\"steer_x\":{},\"steer_y\":{},\"t_us\":{t_us},\
         \"toggle_still\":{toggle},\"wheel\":0}}",
        *seq, steer.0, steer.1,
    );
    let answer = session.command("input_frame", &body);
    assert!(answer.contains("\"ok\":true"), "{answer}");
    *seq += 1;
}

/// Enters Still Mode, queues every plan, commits, and ramps back out.
fn edit(session: &mut Session, seq: &mut u32, plans: &[String]) {
    let opened = i64::from(*seq) * 1_000_000 + 10_000_000;
    frame(session, seq, 1, (0, 0), false, false, opened, true);
    assert_eq!(session.lifecycle(), "ramp_in");
    frame(session, seq, 0, (0, 0), false, false, opened + RAMP_US, false);
    assert_eq!(session.lifecycle(), "still", "the ramp completes into Still Mode");
    for plan in plans {
        let answer = session.command("queue_plan", plan);
        assert!(answer.contains("\"ok\":true"), "queueing {plan}: {answer}");
    }
    let answer = session.command("commit_plan", "{}");
    assert!(answer.contains("\"ok\":true"), "committing: {answer}");
    assert_eq!(session.lifecycle(), "ramp_out", "a commit is also the exit");
    frame(session, seq, 0, (0, 0), false, false, opened + 2 * RAMP_US, false);
    assert_eq!(session.lifecycle(), "running", "the committed exit completes");
}

/// The standing Route with the largest trailing flow — the sum of `f(r, t)`
/// over the trace's retained steps up to 60, smallest RouteId on equal sums,
/// and none when no standing Route moved anything.
///
/// It is the same reading Fracture's break makes, restated here so the chapter's
/// authored `none` target can be checked against what the run's own records say
/// rather than against what this file believes.
fn trailing_flow_route(session: &Session) -> Option<(u32, i64, u32, i64)> {
    let state = session.run().expect("a run").state();
    let steps = state.trace.steps.len();
    let mut sums: std::collections::BTreeMap<u32, i64> = std::collections::BTreeMap::new();
    for recorded in state.trace.steps.iter().skip(steps.saturating_sub(60)) {
        for (route, flow) in &recorded.records.f {
            *sums.entry(*route).or_insert(0) += flow;
        }
    }
    let mut ranked: Vec<(u32, i64)> = state
        .now
        .routes
        .iter()
        .map(|route| (route.route, sums.get(&route.route).copied().unwrap_or(0)))
        .filter(|(_, sum)| *sum > 0)
        .collect();
    // Descending by sum, ascending by identifier on a tie — the rule's own
    // ordering.
    ranked.sort_by(|first, second| second.1.cmp(&first.1).then(first.0.cmp(&second.0)));
    match ranked.as_slice() {
        [] => None,
        [only] => Some((only.0, only.1, only.0, only.1)),
        [first, second, ..] => Some((first.0, first.1, second.0, second.1)),
    }
}

/// What the run leaks this step, recomputed off the Field by the Boundary
/// leakage rule: every member of the standing inside with a crossing Route or a
/// declared adjacency to a non-member loses the fraction its exposure earns.
fn leak_now(session: &Session) -> i64 {
    use field_game_core::fx::{adjacent, fixed_mul};
    let state = session.run().expect("a run").state();
    let inside = &state.view.inside;
    let leak_frac = state.now.boundaries.leak_frac;
    let mut total = 0;
    for member in inside {
        let Some(port) = state.now.ports.iter().find(|held| held.node == *member) else {
            continue;
        };
        let mut neighbours: std::collections::BTreeSet<u32> = std::collections::BTreeSet::new();
        for route in &state.now.routes {
            if route.tail == *member && !inside.contains(&route.head) {
                neighbours.insert(route.head);
            }
            if route.head == *member && !inside.contains(&route.tail) {
                neighbours.insert(route.tail);
            }
        }
        for other in &state.now.ports {
            if inside.contains(&other.node) || other.node == *member {
                continue;
            }
            if adjacent(port.pos, port.layer, other.pos, other.layer) {
                neighbours.insert(other.node);
            }
        }
        if neighbours.is_empty() {
            continue;
        }
        let rate = (neighbours.len() as i64 * leak_frac).min(65_536);
        total += fixed_mul(port.q, rate);
    }
    total
}

/// Drives one run: a named starting Form, the trunk the player draws, whether
/// the modelled allowances are spent, whether the optional test is taken, and
/// what the player does once the crisis has taken the trunk.
fn drive_full(
    form: &str,
    trunk: Trunk,
    allowances: bool,
    takes_the_test: bool,
    recovery: Recovery,
    draws: bool,
) -> Driven {
    let mut session = Session::new(&support::worker_init()).expect("versions agree");
    let opened = session.command(
        "init_run",
        &format!("{{\"form\":\"{form}\",\"mode\":\"new\",\"run_id\":\"{KEY}\"}}"),
    );
    assert!(opened.contains("\"ok\":true"), "{opened}");
    let mut driven = Driven {
        trunk,
        recovery,
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
        rehearsal_step: None,
        cut_step: None,
        cut_route: None,
        pressed: Vec::new(),
        pressure_steps: Vec::new(),
        samples: Vec::new(),
        ranked: None,
        ranked_margin: None,
        outlet: Vec::new(),
        store_open: false,
        inside_units: 0,
        inside: Vec::new(),
        store_units: 0,
        bank_units: 0,
        leak_now: 0,
        form_units: 0,
        forms: 0,
        routes: 0,
        impulse: 0,
        standing: Vec::new(),
        reported: Vec::new(),
    };
    // The run opens in The Pull, so the five chapters before this one are
    // played by the shared campaign script and this file picks the run up at
    // its own opening step.
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
        "the run stands in The Break",
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
    let carried: Vec<(String, u32, field_game_core::json::Json)> = driver
        .raised
        .iter()
        .filter(|(name, step, body)| {
            name == "objective_changed"
                && *step >= opening
                && body
                    .get("objective")
                    .and_then(|held| held.get("id"))
                    .and_then(|held| held.as_text())
                    .is_some_and(|id| id.starts_with("objective.the_break."))
        })
        .cloned()
        .collect();
    record(&mut driven, carried);
    record(&mut driven, support::campaign::raised(&mut session));
    let mut seq = driver.seq;

    // The Route set as the chapter opened: eight authored Routes. A Route that
    // leaves it is either the rehearsal's authored cut or Fracture's own break,
    // and the two are told apart by which Route went.
    let mut standing_routes: Vec<u32> = session
        .run()
        .expect("a run")
        .state()
        .now
        .routes
        .iter()
        .map(|held| held.route)
        .collect();
    let mut ranked_now: Option<(u32, i64, u32, i64)> = None;

    'phases: for phase in script(trunk, takes_the_test, recovery, draws) {
        if phase.allowance && !allowances {
            continue;
        }
        if let Act::Edit(plans) = &phase.act {
            if plans.is_empty() {
                continue;
            }
            if driven.transition_step.is_some() {
                continue;
            }
            edit(&mut session, &mut seq, plans);
            record(&mut driven, support::campaign::raised(&mut session));
            continue;
        }
        let reached = |driven: &Driven| match phase.until {
            Until::Never => false,
            Until::Offered(tail) => driven.offered.iter().any(|(id, _)| id.ends_with(tail)),
            Until::Rehearsal => driven.rehearsal_step.is_some(),
            Until::Cut => driven.cut_step.is_some(),
        };
        if reached(&driven) {
            continue;
        }
        let mut left = phase.steps;
        while left > 0 {
            if driven.transition_step.is_some() {
                break;
            }
            if driven.steps > driven.opening_step + GIVE_UP {
                break 'phases;
            }
            if reached(&driven) {
                break;
            }
            let at = controlled(&session);
            let (steer, held, release) = match phase.act {
                Act::Toward(x, y) => (toward(at, x, y), false, false),
                Act::Follow(current) => {
                    let (x, y) = nearest_point(&session, current).unwrap_or((0, 0));
                    (toward(at, x, y), false, false)
                }
                Act::Rest => ((0, 0), false, false),
                Act::Charge(x, y) => (toward(at, x, y), true, false),
                Act::Release => ((0, 0), false, true),
                Act::Edit(_) => unreachable!("an edit is not a stepping act"),
            };
            let now = left.min(u32::from(BATCH)) as u16;
            frame(&mut session, &mut seq, now, steer, held, release, 0, false);
            left -= u32::from(now);

            record(&mut driven, support::campaign::raised(&mut session));

            let state = session.run().expect("a run").state();
            driven.steps = state.now.step;
            if driven.transition_step.is_some() {
                continue;
            }
            let routes: Vec<u32> = state.now.routes.iter().map(|held| held.route).collect();
            for gone in &standing_routes {
                if routes.contains(gone) {
                    continue;
                }
                // The authored rehearsal takes Route 5 and nothing else; the
                // first Route to leave after that is Fracture's own break. A
                // Route the player cuts afterwards — the separation recovery's
                // own act — is neither, and must not overwrite either reading.
                if *gone == 5 && driven.cut_step.is_none() {
                    driven.rehearsal_step.get_or_insert(state.now.step);
                } else if driven.cut_step.is_none() {
                    driven.cut_step = Some(state.now.step);
                    driven.cut_route = Some(*gone);
                    driven.ranked = ranked_now.map(|held| held.0);
                    driven.ranked_margin = ranked_now;
                }
            }
            standing_routes = routes;
            for pressure in &state.pressures {
                if pressure.queued || pressure.pressure != Pressure::Fracture {
                    continue;
                }
                driven.pressed.push(state.now.step);
                let stage = pressure.stage.name().to_string();
                if driven.pressure_steps.last().map(|held| held.1.clone()) != Some(stage.clone()) {
                    driven.pressure_steps.push((state.now.step, stage.clone()));
                }
                driven.samples.push(Sample {
                    step: state.now.step,
                    stage,
                    routes: state.now.routes.iter().map(|held| held.route).collect(),
                });
            }
            // The ranking recomputed every batch, so the reading taken at the
            // last batch before the break is the newest one this file can take
            // before the boundary ends the window and leaves the trace with
            // nothing to rank.
            ranked_now = trailing_flow_route(&session);
            let outlet = state
                .now
                .routes
                .iter()
                .find(|held| held.route == 7)
                .map_or(0, |held| held.flow);
            driven.outlet.push((state.now.step, driven.cut_step.is_some(), outlet));
            driven.store_open =
                state.now.ports.iter().any(|port| port.node == 9 && port.open);
            driven.inside = state.view.inside.clone();
            driven.inside_units = state
                .now
                .ports
                .iter()
                .filter(|port| state.view.inside.contains(&port.node))
                .map(|port| port.q / ONE_UNIT)
                .sum();
            driven.store_units = state
                .now
                .ports
                .iter()
                .find(|port| port.node == 9)
                .map_or(0, |port| port.q / ONE_UNIT);
            driven.bank_units = state
                .now
                .ports
                .iter()
                .find(|port| port.node == 7)
                .map_or(0, |port| port.q / ONE_UNIT);
            driven.form_units = state
                .now
                .forms
                .iter()
                .find(|form| form.controlled)
                .map_or(0, |form| form.charge / ONE_UNIT);
            driven.forms = state.now.forms.len();
            driven.routes = state.now.routes.len();
            driven.impulse = state.progress.impulse;
            driven.leak_now = leak_now(&session);
        }
    }
    driven
}

/// The ordinary driven run: it draws its trunk and it recovers.
fn drive_as(form: &str, trunk: Trunk, allowances: bool, takes_the_test: bool) -> Driven {
    drive_full(form, trunk, allowances, takes_the_test, Recovery::Reroute, true)
}

/// One driven run that draws its trunk, for a caller that names no other choice.
fn drive_with(form: &str, trunk: Trunk, allowances: bool, takes: bool, recovery: Recovery)
    -> Driven
{
    drive_full(form, trunk, allowances, takes, recovery, true)
}

/// The attentive first run: every allowance spent, the spare store taken.
fn attentive() -> Driven {
    drive_as("thread", Trunk::Near, true, true)
}

/// The authored floor: no allowance spent, the spare store taken.
fn floor() -> Driven {
    drive_as("thread", Trunk::Near, false, true)
}

/// The authored content, read as the worker hands it over.
fn authored() -> content::Content {
    let bundle = field_game_core::json::parse(&support::bundle_with(&support::content_hash()))
        .expect("canonical");
    content::read_bundle(&bundle).expect("the content reads")
}

/// The chapter this file is about.
fn chapter() -> content::Chapter {
    authored().chapter(CHAPTER).expect("The Break").clone()
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


/// The required objectives, which is the authored order without the one test.
fn required_order() -> Vec<String> {
    authored_order().into_iter().filter(|id| !id.ends_with("open_the_store")).collect()
}

/// How long Fracture stands, read off the authored table rather than written
/// down here: a re-tuned table moves every window in this file without moving a
/// figure in it.
fn pressure_life() -> Step {
    authored()
        .pressures
        .table(Pressure::Fracture)
        .expect("the manifest authors Fracture's table")
        .span() as Step
}

/// The one schedule entry the chapter authors.
fn scheduled() -> field_game_core::pressure::ScheduleEntry {
    chapter()
        .pressure_schedule
        .iter()
        .find(|entry| entry.pressure == Pressure::Fracture)
        .expect("the chapter schedules Fracture")
        .clone()
}

#[test]
fn every_route_the_chapter_asks_a_player_to_draw_is_inside_every_forms_reach() {
    // Pure arithmetic over what is authored: the placements the chapter writes
    // and the `route_reach` each of the eight Forms writes. The chapter's own
    // scripts draw three Routes — the two trunks and the reserve's — and the
    // eight-Form completion test drives every Form through them, so a chapter
    // that placed one Node a little further away would fail there as a mystery
    // rather than here as a measurement.
    let held = authored();
    let chapter = chapter();
    let placed = |node: u32| -> (field_game_core::fx::Vec2, u8) {
        chapter
            .ports
            .iter()
            .find(|port| port.node == node)
            .map(|port| (port.pos, port.layer))
            .or_else(|| {
                chapter
                    .forms
                    .iter()
                    .find(|form| form.node == node)
                    .map(|form| (form.pos, form.layer))
            })
            .expect("the chapter places the Node")
    };
    let apart = |from: u32, to: u32| -> i64 {
        let (first, first_layer) = placed(from);
        let (second, second_layer) = placed(to);
        field_game_core::fx::distance(first, first_layer, second, second_layer)
    };

    // The narrowest reach of the eight, which is the one that binds.
    let mut reaches: Vec<(&str, i64)> = field_game_core::run::FORMS
        .iter()
        .map(|id| (*id, held.form(id).expect("an authored Form").route_reach))
        .collect();
    reaches.sort_by_key(|held| held.1);
    let (narrowest, reach) = reaches[0];
    println!("\nthe reach the chapter's own Routes are drawn under");
    for (id, held) in &reaches {
        println!("  {id:<8} {:>8.1} units", *held as f64 / ONE_UNIT as f64);
    }
    assert_eq!(narrowest, "ring", "Ring authors the narrowest reach");

    // The three the scripts draw: `Trunk::Near`, `Trunk::Far`, and the
    // reserve's recovery, each named beside its own distance.
    let drawn = [("the near trunk", 2u32, 3u32), ("the far trunk", 2, 4), ("the reserve", 9, 5)];
    let mut widest = (0i64, "");
    for (label, from, to) in drawn {
        let distance = apart(from, to);
        println!(
            "  {label:<16} connect({from}, {to}) {:>8.1} units",
            distance as f64 / ONE_UNIT as f64,
        );
        assert!(
            distance <= reach,
            "{label}: connect({from}, {to}) spans {distance} raw, past {narrowest}'s {reach}",
        );
        if distance > widest.0 {
            widest = (distance, label);
        }
    }
    // And which of the three binds, so the margin is a reading rather than an
    // impression: the reserve's Route is the long one, and it clears Ring's
    // reach by 27.9 units.
    assert_eq!(widest.1, "the reserve", "the reserve's Route is the longest the scripts draw");
    let margin = reach - widest.0;
    println!(
        "  the binding pair clears the narrowest reach by {:.1} units",
        margin as f64 / ONE_UNIT as f64,
    );
    assert!(
        (27 * ONE_UNIT..29 * ONE_UNIT).contains(&margin),
        "the binding margin is {margin} raw, which is not the 27.9 units measured",
    );
}

/// Prints one run's milestone table.
fn milestones(named: &str, run: &Driven) {
    println!("\n{named} — milestone table (steps from the chapter's opening)");
    println!("{:<52} {:>8} {:>8}", "milestone", "step", "minutes");
    for milestone in &run.milestones {
        let at = milestone.step - run.opening_step;
        println!("{:<52} {at:>8} {:>8}", milestone.name, minutes(at));
    }
    for (name, step) in [
        ("the junction's Route is severed", run.rehearsal_step),
        ("the recoverable setback stands", run.setbacks.first().copied()),
        ("the sequence stands again", run.recovery_step),
        ("Fracture takes the trunk", run.cut_step),
    ] {
        if let Some(step) = step {
            let at = step - run.opening_step;
            println!("{name:<52} {at:>8} {:>8}", minutes(at));
        }
    }
}

// ---------------------------------------------------------------------------
// The chapter, completed
// ---------------------------------------------------------------------------

#[test]
fn the_chapter_completes_and_carries_the_run_into_its_own_transition() {
    assert_eq!(CONTENT_VERSION, 1);
    let run = attentive();

    milestones("The Break, attentive first run", &run);
    println!("offered at");
    for (id, step) in &run.offered {
        let at = step - run.opening_step;
        println!("  {id:<50} {at:>8} {:>8}", minutes(at));
    }

    assert_eq!(
        run.completed,
        authored_order(),
        "every objective completes once each, in the authored order",
    );
    let transition = run.transition_step.expect("the chapter completes and the run carries on");
    assert_eq!(
        transition,
        run.at("objective.the_break.hold_past_the_break").expect("the last"),
        "the transition lands on the step the final challenge completed",
    );
    assert_eq!(
        run.anchor_step,
        run.at("objective.the_break.hold_the_run"),
        "the chapter's one Anchor is written at the step the run first held",
    );
}

#[test]
fn the_chapter_asks_for_about_sixty_minutes_and_the_floor_stands_under_it() {
    let floor = floor();
    let attentive = attentive();
    let floor_at = floor.span();
    let attentive_at = attentive.span();

    let spent: u32 = script(Trunk::Near, true, Recovery::Reroute, true)
        .iter()
        .filter(|phase| phase.allowance)
        .map(|phase| phase.steps)
        .sum();
    println!("\nThe Break — the authored floor against the attentive first run");
    println!("  authored floor      {floor_at:>7} steps  {:>6} minutes", minutes(floor_at));
    println!("  attentive first run {attentive_at:>7} steps  {:>6} minutes", minutes(attentive_at));
    println!("modelled first-run allowances, {spent} steps in all:");
    for phase in
        script(Trunk::Near, true, Recovery::Reroute, true).iter().filter(|phase| phase.allowance)
    {
        println!("  {:<40} {:>6} steps", phase.label, phase.steps);
    }
    milestones("the authored floor", &floor);

    let minute = 60 * STEPS_PER_SECOND;
    // Sixty minutes is the chapter's own figure, and the attentive run is what
    // it is read against. The two runs converge after the crisis rather than
    // before it: Fracture's step is chapter-absolute, so a path that reached the
    // final challenge earlier simply waits longer inside it, and the whole of
    // the difference between these two figures is the allowance the attentive
    // run spends working out what to do once the trunk is gone.
    assert!(
        attentive_at > 55 * minute && attentive_at < 65 * minute,
        "the attentive run completes the chapter in {attentive_at} steps ({} minutes), \
         outside the sixty-minute band",
        minutes(attentive_at),
    );
    assert!(
        floor_at > 44 * minute && floor_at < attentive_at,
        "the authored floor completes in {floor_at} steps ({} minutes), outside its band",
        minutes(floor_at),
    );
}

#[test]
fn every_starting_form_completes_the_whole_chapter() {
    // The acceptance bar, driven rather than argued: the same script, the eight
    // Forms, and the nine objectives completing in the authored order under
    // each of them. The script must not be re-tuned per Form, or the answer
    // would be about the tuning.
    let authored = authored_order();
    println!("\nthe eight Forms through the whole chapter");
    println!(
        "{:<8} {:>10} {:>8} {:>8} {:>8} {:>6} {:>7} {:>9} {:>8}",
        "form", "objectives", "steps", "minutes", "setbacks", "forms", "inside", "leak/step",
        "broke",
    );
    let mut readings: Vec<(&str, i64, i64, usize)> = Vec::new();
    for form in field_game_core::run::FORMS {
        let run = drive_as(form, Trunk::Near, false, true);
        assert_eq!(run.completed, authored, "{form} completes the whole chapter in order");
        let at = run.span();
        for stage in &run.stages {
            assert!(
                ["hidden", "active", "complete", "failed_recoverable"].contains(&stage.as_str()),
                "{form} stood in an unexpected objective state: {stage}",
            );
        }
        assert!(run.store_open, "{form} opened the store the optional test names");
        assert_eq!(
            run.cut_route,
            Some(9),
            "{form}: Fracture took the Route the player drew, and not an authored one",
        );
        assert_eq!(run.setbacks.len(), 1, "{form} met the chapter's one setback exactly once");
        println!(
            "{form:<8} {:>10} {at:>8} {:>8} {:>8} {:>6} {:>7} {:>9} {:>8}",
            run.completed.len(),
            minutes(at),
            run.setbacks.len(),
            run.forms,
            run.inside_units,
            run.leak_now,
            format!("{:?}", run.cut_route),
        );
        readings.push((form, run.inside_units, run.leak_now, run.forms));
    }
    // The same script, eight different Fields at the end of it: what a Form's
    // authored parameters are for. `leak_frac` is the parameter this chapter's
    // Boundary is read through, and every Form authors a different one, so no
    // two of the eight leak the same.
    let distinct: std::collections::BTreeSet<(i64, i64, usize)> =
        readings.iter().map(|held| (held.1, held.2, held.3)).collect();
    assert_eq!(
        distinct.len(),
        readings.len(),
        "the eight Forms end the chapter holding only {} distinct readings: {readings:?}",
        distinct.len(),
    );
}

// ---------------------------------------------------------------------------
// The crisis: what Fracture takes, and why
// ---------------------------------------------------------------------------

#[test]
fn fracture_breaks_the_route_the_runs_own_records_make_the_most_used() {
    // The goal's own sentence — "remove the player's most-used route" — as a
    // rule rather than as a coincidence of placement. The chapter schedules
    // Fracture with no target at all, and the locked reading breaks the standing
    // Route with the largest trailing flow. That is why this is driven through
    // every trunk the player can draw: each of them makes a different Route the
    // one the run flows through, and a break that followed the authoring rather
    // than the play would answer the same Route in all of them.
    let entry = scheduled();
    assert_eq!(entry.target.kind, TargetKind::None, "aimed at nothing the author named");
    assert_eq!(entry.target.id, None, "a kind that names nothing carries no identifier");
    assert!(entry.primary, "and it is the chapter's own headline pressure");
    // Every Route the chapter places carries a lower identifier than the first
    // one a player can draw, so "the Route the crisis took was drawn rather than
    // authored" is a statement about identifiers and not about a guess.
    let authored_routes = chapter().routes.len() as u32;
    assert_eq!(authored_routes, 8, "the chapter places eight Routes");

    println!("\nwhat Fracture took, and what this file's own ranking made the most used");
    println!(
        "{:<8} {:>8} {:>7} {:>8} {:>14} {:>8} {:>14}",
        "trunk", "step", "took", "ranked", "its trailing sum", "runner", "its trailing sum",
    );
    for trunk in TRUNKS {
        let run = drive_with("thread", trunk, false, true, Recovery::Reroute);
        let took = run.cut_route.expect("Fracture broke a Route");
        let (ranked, first, runner, second) =
            run.ranked_margin.expect("the ranking was taken before the break");
        println!(
            "{:<8} {:>8} {:>7} {:>8} {:>14} {:>8} {:>14}",
            trunk.name(),
            run.cut_step.expect("a step") - run.opening_step,
            took,
            ranked,
            first,
            runner,
            second,
        );
        assert_eq!(
            took, ranked,
            "{}: Fracture took Route {took}, which is not the Route this run's own trailing \
             flows make the largest",
            trunk.name(),
        );
        assert!(
            took > authored_routes,
            "{}: Fracture took Route {took}, which the chapter authored — the break has to \
             fall on a Route the player drew",
            trunk.name(),
        );
        // And it is a decisive reading rather than a coin toss between two
        // Routes carrying the same. The two trunks of the redundant
        // configuration do carry the same, and there the locked rule's own
        // tie-break — the smaller identifier — is what decides it.
        match trunk {
            Trunk::Both => {
                assert_eq!(first, second, "both trunks carry the same");
                assert!(runner > took, "the smaller identifier is the one taken");
            }
            // The margin is authored rather than lucky, and this is where it
            // comes from: the trunk's head is Node 7, a module the chapter
            // gives a 4096-unit capacity and a 16-per-step upkeep — the
            // heaviest of any Node it places. A head that saturates stops
            // accepting flow, and a trunk that stops carrying is a trunk whose
            // trailing sum falls back among the rest; Node 7 burns what
            // arrives, so the trunk keeps carrying for the whole of the
            // trailing span and the ranking stays decisive.
            _ => assert!(
                first > second * 2,
                "{}: the trunk's trailing sum {first} is not decisively above the next \
                 Route's {second} — check Node 7's upkeep still burns what the trunk \
                 delivers rather than letting the module saturate",
                trunk.name(),
            ),
        }
        // The break is permanent: the Route is gone for the rest of the chapter.
        for sample in run.samples.iter().filter(|held| held.step > run.cut_step.unwrap()) {
            assert!(
                !sample.routes.contains(&took),
                "Route {took} stood again at step {}, in the {} stage — the break is not \
                 permanent",
                sample.step,
                sample.stage,
            );
        }
        assert!(
            !run.inside.is_empty(),
            "{}: the run stood on a Field at the close",
            trunk.name(),
        );
    }
}

#[test]
fn the_pressure_stands_inside_the_objective_it_was_written_for_on_all_four_paths() {
    // `pressure_schedule` counts from the chapter's opening and an objective is
    // offered when a player reaches it, so the two are reconciled by the one
    // lever authoring has: the final challenge asks for long enough that the
    // latest path has reached it before the earliest has left it, and Fracture
    // is scheduled into the span every path stands inside. Four paths, because a
    // run that lets the optional store go arrives later than one that takes it.
    let entry = scheduled();
    let life = pressure_life();
    let last = "objective.the_break.hold_past_the_break";

    println!("\nthe final challenge, on all four paths (steps from the chapter's opening)");
    println!("{:<26} {:>10} {:>12}", "path", "offered", "completed");
    let mut latest_offer = 0;
    let mut earliest_close = Step::MAX;
    for (named, allowances, takes) in [
        ("floor, store taken", false, true),
        ("floor, store let go", false, false),
        ("attentive, store taken", true, true),
        ("attentive, store let go", true, false),
    ] {
        let run = drive_with("thread", Trunk::Near, allowances, takes, Recovery::Reroute);
        let offered =
            run.offered_at(last).expect("the final challenge is offered") - run.opening_step;
        let completed = run.at(last).expect("and completed") - run.opening_step;
        println!("{named:<26} {offered:>10} {completed:>12}");
        latest_offer = latest_offer.max(offered);
        earliest_close = earliest_close.min(completed);
        assert!(
            entry.start_step > offered && entry.start_step + life < completed,
            "{named}: Fracture runs from {} to {}, outside the challenge it was written for \
             ({offered} to {completed})",
            entry.start_step,
            entry.start_step + life,
        );
    }
    let opening_margin = entry.start_step - latest_offer;
    let closing_margin = earliest_close - (entry.start_step + life);
    println!(
        "the binding window is {latest_offer} to {earliest_close}; Fracture runs {} to {}, \
         {opening_margin} steps clear of the latest offer and {closing_margin} clear of the \
         earliest completion",
        entry.start_step,
        entry.start_step + life,
    );
    assert!(latest_offer < earliest_close, "the four paths leave no span they all stand inside");
    // A schedule that has drifted to the edge of one path reads exactly like one
    // that has not, so the margin is asserted with a floor rather than only the
    // containment.
    assert!(
        opening_margin > 300 && closing_margin > 300,
        "the margins are {opening_margin} and {closing_margin} steps, too little to re-tune \
         a phase against",
    );
}

#[test]
fn the_pressure_seats_and_it_has_teeth() {
    // Everything the window test asserts is arithmetic over the authored
    // schedule and the steps the objectives were offered at. What follows is the
    // pressure **observed standing** in a driven run, which is the thing that
    // arithmetic is a claim about, and then what it did.
    let entry = scheduled();
    let life = pressure_life();
    let run = floor();
    let opening = run.opening_step;
    assert!(
        !run.pressed.is_empty(),
        "the chapter's one pressure was never observed standing at all — the schedule was \
         read and nothing was seated",
    );
    let first = run.pressed.first().copied().expect("a first reading") - opening;
    let last = run.pressed.last().copied().expect("a last reading") - opening;
    println!(
        "\nFracture observed standing from {first} to {last}, sampled once every {BATCH} steps"
    );
    for (step, stage) in &run.pressure_steps {
        println!("  {:<12} entered at {:>8}", stage, step - opening);
    }
    let batch = Step::from(BATCH);
    // The sampler reads the list once a batch, so the observed window is the
    // authored one to within one batch at each end. It is banded rather than
    // pinned, because the batch the reading falls in is the driver's and not
    // the chapter's.
    assert!(
        (entry.start_step..entry.start_step + batch).contains(&first),
        "the pressure was first seen standing at {first}, not inside the batch its own \
         admission step ({}) falls in",
        entry.start_step,
    );
    assert!(
        (entry.start_step + life - batch..entry.start_step + life + batch).contains(&last),
        "the pressure was last seen standing at {last}, not within one batch of the step its \
         own table spends it on ({})",
        entry.start_step + life,
    );
    // One pressure, standing once: the run never saw it leave and come back.
    // The tolerance is two batches rather than one because the run answers the
    // break with a Still Mode visit while the pressure still stands, and the
    // three frames a visit sends are not sampled — a gap in the readings, and
    // not a gap in the pressure.
    let breaks = run.pressed.windows(2).filter(|pair| pair[1] - pair[0] > 2 * batch).count();
    assert_eq!(breaks, 0, "the pressure stood in one unbroken span, not {}", breaks + 1);
    let widest = run.pressed.windows(2).map(|pair| pair[1] - pair[0]).max().unwrap_or(0);
    println!("  the widest gap between two readings is {widest} steps");
    // All four stages of its life were walked, and the break fell on the crisis
    // entry and on nothing else.
    let stages: Vec<&str> = run.pressure_steps.iter().map(|held| held.1.as_str()).collect();
    assert_eq!(
        stages,
        vec!["signal", "pressure", "crisis", "resolution"],
        "the pressure walked its whole run of stages, in order",
    );
    let crisis = run
        .pressure_steps
        .iter()
        .find(|held| held.1 == "crisis")
        .map(|held| held.0)
        .expect("a crisis entry");
    let cut = run.cut_step.expect("Fracture broke a Route");
    assert!(
        cut.abs_diff(crisis) <= u32::from(BATCH),
        "the break landed at {cut} and the crisis was entered at {crisis}: the one-shot falls \
         on the crisis entry and nowhere else",
    );

    // And it has teeth. An inert pressure is worse than none: it runs its whole
    // schedule, draws a line on the surface, and changes nothing a player could
    // read. What says this one bit is that the run which answers it in no way at
    // all stops carrying and does not finish the chapter.
    let idle = drive_with("thread", Trunk::Near, false, true, Recovery::None);
    // The stretch either side of the break, rather than the whole chapter: the
    // opening minutes carry nothing because the run has not been built yet, and
    // the rehearsal's own severance stops the outlet in the middle, so a reading
    // taken over the whole chapter would say nothing about this break.
    const EITHER_SIDE: Step = 3_000;
    let outlet = |run: &Driven, after: bool| -> Vec<i64> {
        let cut = run.cut_step.expect("a break");
        run.outlet
            .iter()
            .filter(|(step, _, _)| {
                if after {
                    *step > cut && *step <= cut + EITHER_SIDE
                } else {
                    *step < cut && *step >= cut - EITHER_SIDE
                }
            })
            .map(|(_, _, flow)| *flow)
            .collect()
    };
    let before = outlet(&idle, false);
    let after = outlet(&idle, true);
    assert!(!before.is_empty() && !after.is_empty(), "both halves of the beat were sampled");
    let least = |held: &[i64]| held.iter().copied().min().unwrap_or(0);
    // The two sides are read in opposite directions, so they need opposite
    // aggregates. Before the break the claim is that the outlet never stopped,
    // and the minimum is exactly that claim. After it the claim is that the
    // outlet mostly stopped — and a minimum below a threshold is bought by one
    // dip, which is a reading about the quietest sample rather than about the
    // outlet. So the after side is read as the whole it carried and as its
    // ninetieth percentile, neither of which one sample can move. Measured: it
    // carries 10,121,200 raw over the after side against 104,093,322 over the
    // before side — 9.7% of the throughput — and its ninetieth percentile
    // falls from 523,544 raw a step to 24,000. The tail does come back: the
    // top one sample in twenty stands where the before side does, which is why
    // no reading here claims the outlet went silent.
    let at_percentile = |held: &[i64], of: usize| -> i64 {
        let mut sorted = held.to_vec();
        sorted.sort_unstable();
        sorted[(sorted.len() - 1) * of / 100]
    };
    println!(
        "  the core's outlet over the {EITHER_SIDE} steps either side of the break, raw: \
         before it over {} samples, least {}, p90 {}, whole {}; \
         after it over {} samples, least {}, p50 {}, p90 {}, p95 {}, most {}, whole {}",
        before.len(),
        least(&before),
        at_percentile(&before, 90),
        before.iter().sum::<i64>(),
        after.len(),
        least(&after),
        at_percentile(&after, 50),
        at_percentile(&after, 90),
        at_percentile(&after, 95),
        after.iter().max().copied().unwrap_or(0),
        after.iter().sum::<i64>(),
    );
    assert!(
        least(&before) > 7 * ONE_UNIT,
        "the outlet carried at least {} raw a step before the break, which is under the seven \
         whole units the run was built to carry",
        least(&before),
    );
    let carried = |held: &[i64]| held.iter().sum::<i64>();
    assert!(
        carried(&after) * 5 < carried(&before),
        "the outlet carried {} raw over the {EITHER_SIDE} steps after the break against {} \
         over the {EITHER_SIDE} before it — the break has to take most of the throughput \
         rather than one sample of it",
        carried(&after),
        carried(&before),
    );
    assert!(
        at_percentile(&after, 90) < ONE_UNIT / 2,
        "nine readings in ten after the break have to stand below half a unit a step for the \
         break to have taken the Route the run was using; the ninetieth percentile is {} raw \
         against the {} the run held before it",
        at_percentile(&after, 90),
        at_percentile(&before, 90),
    );
    assert!(
        idle.transition_step.is_none(),
        "the run that answered the break in no way finished the chapter anyway, in {} steps",
        idle.steps - idle.opening_step,
    );
    assert!(
        !idle.completed.contains(&"objective.the_break.hold_past_the_break".to_string()),
        "the final challenge completed without any answer to the break",
    );
}

// ---------------------------------------------------------------------------
// The done-when: recoverable from every valid pre-crisis configuration
// ---------------------------------------------------------------------------

#[test]
fn the_chapter_recovers_from_every_pre_crisis_configuration() {
    // The goal's done-when, driven. The pre-crisis configuration space is
    // enumerated as the two choices a player makes that change **what the Field
    // affords afterwards**: which line the trunk was drawn to (near, far, or
    // both), and whether the optional store was opened. Each of those is crossed
    // with each of the three recovery families the goal names. The Form axis and
    // the allowance axis are driven elsewhere, because neither changes what the
    // Field affords — only when things happen; the partition argument is in the
    // report.
    let required = required_order();
    println!("\nthe pre-crisis configuration space, and what each recovery does with it");
    println!(
        "{:<8} {:>7} {:<10} {:>10} {:>8} {:>7} {:>7} {:>9}",
        "trunk", "store", "recovery", "completed", "steps", "routes", "store", "leak/step",
    );
    let mut carried = 0;
    for trunk in TRUNKS {
        for takes in [true, false] {
            for recovery in RECOVERIES {
                let run = drive_with("thread", trunk, false, takes, recovery);
                println!(
                    "{:<8} {:>7} {:<10} {:>10} {:>8} {:>7} {:>7} {:>9}",
                    trunk.name(),
                    if takes { "taken" } else { "let go" },
                    recovery.name(),
                    run.completed.len(),
                    run.transition_step.map_or(0, |step| step - run.opening_step),
                    run.routes,
                    run.store_units,
                    run.leak_now,
                );
                // A store that was never opened is not a reserve: the Route the
                // player draws to it stands, and carries nothing, because a
                // Route carries nothing while either of its ends is closed.
                // That is the one place in the space where a family is
                // unavailable, and the configuration is still recoverable —
                // by either of the other two, driven in the rows beside it.
                // The redundant configuration is the exception: it drew a second
                // trunk before the crisis, so it carries on whatever is done
                // afterwards, and an unavailable answer costs it nothing.
                let available =
                    takes || recovery != Recovery::Reserve || trunk == Trunk::Both;
                if available {
                    let done: Vec<String> = run
                        .completed
                        .iter()
                        .filter(|id| !id.ends_with("open_the_store"))
                        .cloned()
                        .collect();
                    assert_eq!(
                        done,
                        required,
                        "{} trunk, store {}, {}: the chapter did not complete",
                        trunk.name(),
                        if takes { "taken" } else { "let go" },
                        recovery.name(),
                    );
                    assert!(
                        run.transition_step.is_some(),
                        "{} trunk, store {}, {}: the run did not carry into the next chapter",
                        trunk.name(),
                        if takes { "taken" } else { "let go" },
                        recovery.name(),
                    );
                    carried += 1;
                } else {
                    assert!(
                        run.transition_step.is_none(),
                        "{} trunk: connecting a store that was never opened carried the \
                         chapter anyway",
                        trunk.name(),
                    );
                }
            }
        }
    }
    assert_eq!(
        carried, 16,
        "sixteen of the eighteen driven configurations carry the chapter; {carried} did. \
         The two that do not are the single-trunk configurations that let the store go and \
         then reach for it, and each of those is carried by the two answers beside it",
    );

    // And the redundant configuration is its own answer: a player who spent the
    // second Impulse before the crisis has already done the rerouting, so the
    // run carries on with no further act at all. The two single-trunk
    // configurations do not.
    println!("\nthe same three configurations with no answer to the break at all");
    for trunk in TRUNKS {
        let run = drive_with("thread", trunk, false, true, Recovery::None);
        let carried = run.transition_step.is_some();
        println!(
            "  {:<8} completed {:>2}  carried on: {carried}",
            trunk.name(),
            run.completed.len(),
        );
        match trunk {
            Trunk::Both => assert!(
                carried,
                "the redundant configuration needed a further act after the break",
            ),
            _ => assert!(
                !carried,
                "{}: the chapter carried past the break with no answer to it",
                trunk.name(),
            ),
        }
    }
}

#[test]
fn each_recovery_is_worth_something_the_baseline_is_not() {
    // What each family is **worth**, pinned against the run that answers the
    // break in no way at all. Completion alone is not the measure: three
    // recoveries that all completed and whose benefits had vanished would pass a
    // completion test. Each of these is a measure the layout does not trivially
    // control, and the setbacks are printed beside the benefits.
    let baseline = drive_with("thread", Trunk::Near, false, true, Recovery::None);
    let runs: Vec<Driven> = RECOVERIES
        .into_iter()
        .map(|recovery| drive_with("thread", Trunk::Near, false, true, recovery))
        .collect();

    // A batch on which the core's outlet moved less than one whole unit: the
    // run standing still in all but name. Counted rather than thresholded on
    // zero, because a stranded line goes on trickling raw units into the core
    // for a long time without ever carrying anything a player could use.
    let stalled = |run: &Driven| -> usize {
        let cut = run.cut_step.expect("a break");
        run.outlet.iter().filter(|(step, _, flow)| *step > cut && *flow < ONE_UNIT).count()
    };
    let carried_after = |run: &Driven| -> i64 {
        let cut = run.cut_step.expect("a break");
        run.outlet.iter().filter(|(step, _, _)| *step > cut).map(|(_, _, flow)| *flow).sum()
    };

    println!(
        "\nThe Break — four answers to the same break, all on the {} trunk",
        baseline.trunk.name(),
    );
    println!(
        "{:<10} {:>9} {:>9} {:>9} {:>7} {:>7} {:>9} {:>7} {:<18}",
        "answer", "completed", "stalls", "carried", "routes", "store", "leak/step", "impulse",
        "inside",
    );
    for run in std::iter::once(&baseline).chain(runs.iter()) {
        println!(
            "{:<10} {:>9} {:>9} {:>9} {:>7} {:>7} {:>9} {:>7} {:<18}",
            run.recovery.name(),
            run.transition_step.map_or(0, |step| step - run.opening_step),
            stalled(run),
            carried_after(run),
            run.routes,
            run.store_units,
            run.leak_now,
            run.impulse,
            format!("{:?}", run.inside),
        );
    }

    let reroute = &runs[0];
    let reserve = &runs[1];
    let separate = &runs[2];
    assert_eq!(reroute.recovery, Recovery::Reroute);
    assert_eq!(reserve.recovery, Recovery::Reserve);
    assert_eq!(separate.recovery, Recovery::Separate);

    // Each of them acted on the Field in its own way, and the Field says so.
    assert_eq!(
        reroute.routes, baseline.routes,
        "the reroute formed no Route: it moved one end of one that stood",
    );
    assert!(
        reserve.routes > baseline.routes,
        "the reserve run stands a Route the baseline does not: {} against {}",
        reserve.routes,
        baseline.routes,
    );
    assert!(
        separate.routes < baseline.routes,
        "the separated run stands fewer Routes than the baseline: {} against {}",
        separate.routes,
        baseline.routes,
    );
    assert!(
        separate.inside.len() < baseline.inside.len(),
        "the separated run's standing inside is not smaller: {:?} against {:?}",
        separate.inside,
        baseline.inside,
    );

    // And what each is worth, one measure per family, each strictly better than
    // the baseline.
    for run in &runs {
        assert!(
            stalled(run) < stalled(&baseline),
            "{}: the core's outlet carried nothing on {} sampled steps after the break, \
             against the baseline's {} — the answer bought nothing",
            run.recovery.name(),
            stalled(run),
            stalled(&baseline),
        );
        assert!(
            carried_after(run) > carried_after(&baseline),
            "{}: the outlet carried {} raw after the break against the baseline's {}",
            run.recovery.name(),
            carried_after(run),
            carried_after(&baseline),
        );
    }
    // The reserve is the only one that spends the store, and the separation is
    // the only one that leaves the run leaking less than it did — the exposure
    // the reshaped inside sheds is the whole of that difference.
    assert_eq!(reserve.store_units, 0, "the reserve run spent the store it had banked");
    assert!(
        reroute.store_units > 0 && separate.store_units > 0,
        "the other two answers left the store untouched",
    );
    assert!(
        separate.leak_now * 8 < baseline.leak_now,
        "the separated run leaks {} raw a step against the baseline's {}: shedding the \
         stranded Nodes from the inside bought less than an eighth",
        separate.leak_now,
        baseline.leak_now,
    );
    assert!(
        reroute.leak_now >= baseline.leak_now / 2 && reserve.leak_now >= baseline.leak_now / 2,
        "the two answers that keep the whole inside leak about what the baseline leaks",
    );

    // They are four different runs, not one run under four names.
    let readings = [
        ("nothing", stalled(&baseline), baseline.routes, baseline.store_units, baseline.leak_now),
        ("reroute", stalled(reroute), reroute.routes, reroute.store_units, reroute.leak_now),
        ("reserve", stalled(reserve), reserve.routes, reserve.store_units, reserve.leak_now),
        ("separate", stalled(separate), separate.routes, separate.store_units, separate.leak_now),
    ];
    for (place, first) in readings.iter().enumerate() {
        for second in &readings[place + 1..] {
            assert!(
                (first.1, first.2, first.3, first.4) != (second.1, second.2, second.3, second.4),
                "{} and {} are the same run under two names",
                first.0,
                second.0,
            );
        }
    }
}

// ---------------------------------------------------------------------------
// The authored events, the setback, and the optional test
// ---------------------------------------------------------------------------

#[test]
fn the_authored_events_land_at_their_own_steps_on_every_path() {
    // The chapter's two events are timed against an objective rather than
    // against the run's step counter, so they land at the same offset in a run
    // that took sixty minutes to reach them and one that took fifty.
    let chapter = chapter();
    assert_eq!(chapter.events.len(), 2, "the chapter authors two events");
    for event in &chapter.events {
        assert_eq!(
            event.objective, "objective.the_break.hold_the_run",
            "both are written against the hold they disrupt",
        );
    }
    let severance = chapter
        .events
        .iter()
        .find(|event| matches!(event.effect, content::Effect::RouteCut { .. }))
        .expect("the chapter authors a scripted route cut");
    let content::Effect::RouteCut { route } = severance.effect else {
        unreachable!("matched above");
    };
    assert_eq!(route, 5, "the scripted cut takes the junction's own Route into the core");

    println!("\nthe authored severance, and the step each path first read it at");
    println!("{:<22} {:>9} {:>7} {:>8} {:>6}", "path", "offered", "due", "read", "late");
    for (named, run) in [("the authored floor", floor()), ("the attentive run", attentive())] {
        let offered =
            run.offered_at(&severance.objective).expect("the objective was offered");
        let landed = run.rehearsal_step.expect("the cut landed");
        println!(
            "{named:<22} {:>9} {:>7} {:>8} {:>6}",
            offered - run.opening_step,
            severance.at,
            landed - run.opening_step,
            landed - offered - severance.at,
        );
        // The event is read on the batch the driver next samples, so it is
        // within one batch of the step it was authored for.
        assert!(
            landed >= offered + severance.at
                && landed - offered - severance.at <= Step::from(BATCH),
            "{named}: the cut was read {} steps after the step it was authored for",
            landed - offered - severance.at,
        );
    }
}

#[test]
fn the_authored_setback_stands_and_the_chapter_carries_past_it() {
    // The chapter's one recoverable collapse, and its only one. The scripted
    // severance leaves the junction with a Route in and none out; it fills, and
    // `pattern_held` reads a Node over its threshold as the authored setback.
    // Nothing about it is terminal: the sequence stands again the moment the
    // line is brought onto the core.
    let run = floor();
    assert_eq!(
        run.setbacks.len(),
        1,
        "the chapter enters a recoverable setback exactly once, at {:?}",
        run.setbacks,
    );
    let setback = run.setbacks[0];
    let cut = run.rehearsal_step.expect("the scripted cut landed");
    let recovered = run.recovery_step.expect("the sequence stood again");
    println!(
        "\nthe cut at {}, the setback at {} ({} steps later), the sequence standing again at \
         {} ({} steps after the setback)",
        cut - run.opening_step,
        setback - run.opening_step,
        setback - cut,
        recovered - run.opening_step,
        recovered - setback,
    );
    assert!(setback > cut, "the setback follows the cut that caused it");
    assert!(recovered > setback, "and the sequence stands again after it");
    assert!(
        run.transition_step.is_some_and(|step| step > recovered),
        "the chapter completes past the setback",
    );
    // It is the junction that fills, and the objective it fails is the one the
    // cut was written against.
    let chapter = chapter();
    let held = chapter
        .objectives
        .iter()
        .find(|objective| objective.id.ends_with("hold_the_run"))
        .expect("the hold");
    let content::Condition::PatternHeld { nodes, .. } = &held.condition else {
        panic!("the hold is a pattern");
    };
    assert!(nodes.contains(&10), "the objective names the junction, so its filling is read");
    let junction = chapter.ports.iter().find(|port| port.node == 10).expect("the junction");
    assert!(
        junction.upkeep_rate > 0,
        "the junction pays upkeep, which is what lets it come back under its own threshold \
         once nothing is carrying into it",
    );
}

#[test]
fn the_optional_test_is_a_reach_that_is_passable_and_skippable() {
    let chapter = chapter();
    let tests: Vec<&content::Objective> =
        chapter.objectives.iter().filter(|held| held.optional.is_some()).collect();
    assert_eq!(tests.len(), 1, "the chapter authors one optional test");
    let store = tests[0];
    assert_eq!(store.id, "objective.the_break.open_the_store");
    assert_eq!(store.optional, Some(5462), "and it stands for the span it authors");
    // It is a Port, standing alone, well past every other Node the chapter asks
    // a Pulse of — so no Pulse the sequence requires opens it by accident.
    let content::Condition::PortsOpen { ports } = &store.condition else {
        panic!("the test names a Port");
    };
    assert_eq!(ports, &vec![9]);

    let taken = drive_with("thread", Trunk::Near, false, true, Recovery::Reroute);
    let let_go = drive_with("thread", Trunk::Near, false, false, Recovery::Reroute);
    println!("\nthe optional store, taken and let go");
    println!(
        "  taken:  open {}, banked {} units, chapter {} steps, {} objectives",
        taken.store_open,
        taken.store_units,
        taken.span(),
        taken.completed.len(),
    );
    println!(
        "  let go: open {}, banked {} units, chapter {} steps, {} objectives",
        let_go.store_open,
        let_go.store_units,
        let_go.span(),
        let_go.completed.len(),
    );
    assert!(taken.store_open, "the driven run that takes it opens the store");
    assert!(!let_go.store_open, "and the run that lets it go never does");
    assert!(taken.completed.contains(&store.id), "taking it completes it");
    assert!(!let_go.completed.contains(&store.id), "letting it go passes it over");
    assert_eq!(
        let_go.completed.len() + 1,
        taken.completed.len(),
        "and the only difference in what completed is the test itself",
    );
    // What taking it is worth: a store that is full when the break lands. What
    // letting it go costs: only the time it stood, and one of the three answers
    // to the break.
    assert!(
        taken.store_units > 4000,
        "the store taken here is full by the close: {} units",
        taken.store_units,
    );
    assert_eq!(let_go.store_units, 0, "and one never opened banks nothing");
}

// ---------------------------------------------------------------------------
// The authored content, read back
// ---------------------------------------------------------------------------

#[test]
fn the_authored_conditions_span_the_step_counts_they_name() {
    // Progress is a `Frac`, so a target of N actually spans
    // `ceil(65536 / ceil(65536 / N))`. Every authored count is checked against
    // what it will really take, and held to within the share one step carries.
    let chapter = chapter();
    println!("\nthe authored conditions, and the spans they really take");
    println!("{:<52} {:>8} {:>10} {:>7}", "objective", "authored", "effective", "drift");
    for objective in &chapter.objectives {
        let named = objective.condition.target();
        let effective = content::effective_steps(&objective.condition);
        println!(
            "{:<52} {named:>8} {effective:>10} {:>7}",
            objective.id,
            effective - named,
        );
        assert!(
            (effective - named).abs() <= named / 32 + 1,
            "{}: authored {named}, spans {effective}",
            objective.id,
        );
    }
    // The chapter's shape, read off the file rather than restated: nine
    // objectives, one of them optional, one Anchor moment, one pressure.
    assert_eq!(chapter.objectives.len(), 9);
    assert_eq!(chapter.objectives.iter().filter(|held| held.optional.is_some()).count(), 1);
    assert_eq!(chapter.anchor_moments, vec!["objective.the_break.hold_the_run".to_string()]);
    assert_eq!(chapter.pressure_schedule.len(), 1);
    assert_eq!(chapter.layers.len(), 1, "the chapter stands on one layer");
    // The dim current over the core stands stopped at the open, so the core
    // banks nothing before the run reaches it and the objective that asks the
    // run to carry cannot be met by standing still.
    let dim = chapter.currents.iter().find(|held| held.id == 2).expect("the dim current");
    assert!(!dim.active, "the dim current is stopped at the open");
    assert!(!dim.bright, "and it is not the bright band");
}

#[test]
fn the_chapter_offers_one_objective_at_a_time_and_none_of_them_fails_terminally() {
    let run = floor();
    // Read off the reported stream rather than off `progress`, which holds one
    // objective by construction and so could never disagree. What the shell
    // draws is the stream, and the stream is what has to carry the rule.
    for (step, live) in &run.standing {
        assert!(
            live.len() <= 1,
            "at step {} the stream left {} of this chapter's objectives standing: {live:?}",
            step - run.opening_step,
            live.len(),
        );
    }
    assert!(
        run.standing.iter().any(|(_, live)| live.len() == 1),
        "and at least one of those readings had an objective standing",
    );
    for stage in &run.stages {
        assert!(
            ["hidden", "active", "complete", "failed_recoverable"].contains(&stage.as_str()),
            "the chapter stood in an objective state outside the closed set: {stage}",
        );
    }
    assert!(
        !run.stages.iter().any(|stage| stage == "failed"),
        "no objective of this chapter fails terminally",
    );
}

#[test]
fn the_shared_campaign_script_carries_the_chapter_to_its_own_boundary() {
    // The chapter's arm in the shared driver, driven as `core/tests/campaign.rs`
    // drives it: the whole campaign up to and including this chapter, with every
    // optional test passed over. It is a different script from this file's, so
    // it is a second reading of the same authored chapter.
    let mut session = Session::new(&support::worker_init()).expect("versions agree");
    let opened = session.command(
        "init_run",
        &format!("{{\"form\":\"thread\",\"mode\":\"new\",\"run_id\":\"{KEY}\"}}"),
    );
    assert!(opened.contains("\"ok\":true"), "{opened}");
    let mut driver = support::campaign::Driver::new();
    let content = authored();
    for index in 0..=CHAPTER {
        support::campaign::play_chapter(
            &mut session,
            &mut driver,
            content.chapter(index).expect("a chapter"),
        );
    }
    assert_eq!(
        support::campaign::chapter_index(&session),
        CHAPTER + 1,
        "the shared script carries the run past The Break",
    );
    let done = session.run().expect("a run").state().progress.complete.clone();
    for id in required_order() {
        assert!(done.contains(&id), "the shared script completed {id}");
    }
    assert!(
        !done.contains(&"objective.the_break.open_the_store".to_string()),
        "and it passed the optional test over, which is what a driven campaign does",
    );
}

#[test]
fn the_run_cannot_be_started_without_a_route_the_player_draws() {
    // The chapter's own thesis, pinned where it can fail: the Route the crisis
    // takes is one the player made, so a run that never enters Still Mode has
    // no trunk for the crisis to take — and, before that, no run at all. This
    // one plays the whole script, opens every Port the chapter asks for, and
    // holds the band for as long as the driver will let it, queueing nothing.
    let idle = drive_full("thread", Trunk::Near, false, true, Recovery::Reroute, false);
    println!(
        "\nno Still Mode: {} objectives completed, {} Routes standing, {} steps driven, \
         transition {:?}",
        idle.completed.len(),
        idle.routes,
        idle.steps - idle.opening_step,
        idle.transition_step,
    );
    for id in &idle.completed {
        println!("  {id}");
    }
    // The four Ports the sequence asks a Pulse of do open, so what stops the run
    // is not the steering and not the Pulse.
    let opened = required_order()
        .into_iter()
        .take_while(|id| !id.ends_with("start_the_run"))
        .collect::<Vec<String>>();
    assert_eq!(
        idle.completed, opened,
        "the run opened every Port the chapter names and then stopped at the one objective \
         that asks for a Route",
    );
    assert_eq!(
        idle.routes,
        chapter().routes.len(),
        "and it stands exactly the Routes the chapter authored, and not one more",
    );
    assert!(
        idle.transition_step.is_none(),
        "the chapter cannot be completed by steering and the Pulse alone",
    );
    assert!(idle.cut_step.is_none(), "and Fracture found no Route worth breaking");
    assert!(
        idle.steps - idle.opening_step > 100_000,
        "the run was driven well past the step the floor completes the chapter at",
    );
}
