//! The Loop, driven end to end: the whole fifty-minute chapter.
//!
//! The chapter's three systems are port routing, self-support, and overload,
//! and the file is organised around what each of them makes provable:
//!
//! - **Port routing.** Nothing on this field gives the run Charge. The one
//!   supply is a bright band the Form stands in, and the only way from the Form
//!   to the run is a Route the player draws in Still Mode. Four stores stand
//!   below the outfall, each an unopened Port with its own authored return into
//!   the intake, so the circuit can be closed four different ways and the
//!   choice is a choice about the circuit rather than about the trip to it.
//! - **Self-support.** Every Node of the run authors an `upkeep_rate` and pays
//!   it out of what it holds, every step, and so does every store. The circuit
//!   carries Charge back to the intake but makes none, so what keeps it
//!   standing is the band.
//! - **Overload.** The outfall has nowhere to send what arrives, so it carries
//!   more than it can hold and the objective reads `failed_recoverable` until
//!   it is given somewhere to go. That is the chapter's authored break, and
//!   closing the circuit is the recovery.
//!
//! Two runs are driven, and they answer two different questions:
//!
//! - The **attentive run** carries a modelled first-time player, spending the
//!   named discovery allowances below. Its milestone table is what the fifty
//!   minutes are read against.
//! - The **authored floor** spends no allowance at all. It is the run the eight
//!   Forms and the four route layouts are driven through, because what is being
//!   asked of them is whether the chapter completes, not how long a first run
//!   spends looking.
//!
//! **Two authored dependencies a later tweak could silently break**, both
//! asserted here with their reasons:
//!
//! - *The intake's Route identifiers.* The Route out of the intake carries a
//!   lower identifier than either Route into it, so the ascending Route pass
//!   empties the intake before it refills it. Flood's throttle term reads the
//!   target's Charge at the Route phase, where the intake therefore stands
//!   empty; what Flood does to it is shed, at the decay phase, on what the pass
//!   left.
//! - *The pressure window.* `pressure_schedule` counts from the chapter's own
//!   opening, so Flood's step is absolute; an objective is offered when a
//!   player reaches it, so its step is not. Four paths have to be reconciled
//!   rather than two, because the chapter authors an optional test and a run
//!   that lets it go arrives *later* than one that takes it: floor and
//!   attentive, each taking or skipping the spare.
//!   `the_pressures_land_inside_the_objectives_written_for_them` asserts the
//!   containment on all four and prints the binding window.

use field_game_core::content::{self, CONTENT_VERSION};
use field_game_core::fx::ONE_UNIT;
use field_game_core::pressure::{Pressure, TargetKind};
use field_game_core::state::{Step, STEPS_PER_SECOND};
use field_game_core::Session;

mod support;

use support::campaign::{controlled, nearest_point, toward};

const KEY: &str = "00112233445566cc";

/// How many steps one scripted batch runs before the script re-aims.
const BATCH: u16 = 15;

/// How many steps a full Pulse charge takes, from the locked charging rule.
const FULL_CHARGE_STEPS: u32 = 32;

/// How long a ramp takes, in microseconds. Locked.
const RAMP_US: i64 = 250_000;

/// This chapter's place in the authored campaign.
const CHAPTER: u8 = 2;

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

/// Which store the run closes its circuit onto.
///
/// Each store is a Port that has to be opened, stands within every Form's reach
/// of the outfall, and carries its own authored return Route into the intake.
/// What differs between them is the authored pair `(capacity, return capacity)`
/// — how much the store banks, and how fast it gives it back — so the choice is
/// a choice about the circuit rather than about the route to it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Layout {
    /// Node 6: 256 units banked, 24 a step returned.
    Near,
    /// Node 7: 640 units banked, 20 a step returned.
    Low,
    /// Node 8: 1280 units banked, 8 a step returned.
    Far,
    /// Nodes 7 and 8 together: two stores, two returns into one intake.
    Both,
}

/// The four layouts the variety runs are driven through.
const LAYOUTS: [Layout; 4] = [Layout::Near, Layout::Low, Layout::Far, Layout::Both];

/// How many of the four stand on the frontier — no other layout better than
/// them on every measure at once. Measured rather than assumed; see
/// `no_single_route_layout_dominates_the_driven_runs`.
const LIVE_LAYOUTS: usize = 3;

impl Layout {
    fn name(self) -> &'static str {
        match self {
            Layout::Near => "near",
            Layout::Low => "low",
            Layout::Far => "far",
            Layout::Both => "low+far",
        }
    }

    /// The stores this layout opens, each with its authored position in whole
    /// units.
    fn stores(self) -> &'static [(u32, i64, i64)] {
        match self {
            Layout::Near => &[(6, 2280, 2360)],
            Layout::Low => &[(7, 2500, 2610)],
            Layout::Far => &[(8, 2920, 2460)],
            Layout::Both => &[(7, 2500, 2610), (8, 2920, 2460)],
        }
    }

    /// The queued changes that close the circuit.
    fn closing(self) -> &'static [&'static str] {
        match self {
            Layout::Near => &["{\"plan\":{\"from\":5,\"op\":\"connect\",\"to\":6}}"],
            Layout::Low => &["{\"plan\":{\"from\":5,\"op\":\"connect\",\"to\":7}}"],
            Layout::Far => &["{\"plan\":{\"from\":5,\"op\":\"connect\",\"to\":8}}"],
            Layout::Both => &[
                "{\"plan\":{\"from\":5,\"op\":\"connect\",\"to\":7}}",
                "{\"plan\":{\"from\":5,\"op\":\"connect\",\"to\":8}}",
            ],
        }
    }
}

/// The one queued change that carries the supply from the Form into the run.
const SUPPLY_ROUTE: &[&str] = &["{\"plan\":{\"from\":1,\"op\":\"connect\",\"to\":3}}"];

/// The scripted run, phase by phase.
///
/// The positions are the authored content's own — the supply band, the four
/// Ports of the run, the stores below it, and the spare junction the optional
/// test asks for. Every phase that holds the supply follows the band's nearest
/// path point read off the Field rather than a coordinate off the file.
///
/// `layout` decides which store the circuit closes onto and `takes_the_test`
/// whether the spare is opened or let go; the rest of the script is identical
/// across all of them, so a difference between two driven runs is a difference
/// the authored content made.
fn script(layout: Layout, takes_the_test: bool) -> Vec<Phase> {
    let mut phases = vec![
        allow("read the surface", Act::Rest, 240),
        play("steer into the supply band", Act::Toward(1400, 1500), 240),
        play("hold the supply", Act::Follow(1), 5600),
        allow("try the controls", Act::Follow(1), 900),
        allow("look for what stands below", Act::Toward(1700, 1740), 900),
        play("cross to the intake", Act::Toward(1800, 2000), 480),
        play("charge", Act::Charge(1800, 2000), FULL_CHARGE_STEPS),
        play("release", Act::Release, 1),
        allow("look for the rest of the run", Act::Toward(1960, 1960), 900),
        play("cross to the turn", Act::Toward(2120, 1920), 360),
        play("charge", Act::Charge(2120, 1920), FULL_CHARGE_STEPS),
        play("release", Act::Release, 1),
        play("cross to the spur", Act::Toward(2440, 2000), 360),
        play("charge", Act::Charge(2440, 2000), FULL_CHARGE_STEPS),
        play("release", Act::Release, 1),
        play("cross to the outfall", Act::Toward(2620, 2280), 360),
        play("charge", Act::Charge(2620, 2280), FULL_CHARGE_STEPS),
        play("release", Act::Release, 1),
        allow("look for what the run is missing", Act::Toward(2200, 2140), 1800),
        play("back to the intake", Act::Toward(1800, 2000), 480),
        play("draw the supply route", Act::Edit(SUPPLY_ROUTE), 0),
        play("back to the band", Act::Toward(1800, 1500), 420),
        play("hold while the run starts", Act::Follow(1), 600),
        allow("read the stores", Act::Toward(2200, 2140), 1200),
        allow("look for where the run can send it", Act::Toward(2400, 2260), 2400),
    ];
    for (_, x, y) in layout.stores() {
        phases.push(play("cross to the store", Act::Toward(*x, *y), 720));
        phases.push(play("charge", Act::Charge(*x, *y), FULL_CHARGE_STEPS));
        phases.push(play("release", Act::Release, 1));
    }
    phases.push(play("close the circuit", Act::Edit(layout.closing()), 0));
    phases.push(play("back to the band", Act::Toward(2100, 1500), 780));
    phases.push(play("hold the supply", Act::Follow(1), 14_200));
    if takes_the_test {
        phases.extend([
            allow("look for the spare", Act::Toward(2700, 1800), 600),
            play("cross to the spare", Act::Toward(2900, 2080), 720),
            play("charge", Act::Charge(2900, 2080), FULL_CHARGE_STEPS),
            play("release", Act::Release, 1),
            play("back to the band", Act::Toward(2600, 1500), 720),
        ]);
    } else {
        phases.push(play("let the spare go", Act::Follow(1), 5600));
    }
    phases.extend([
        play("keep the circuit supplied", Act::Follow(1), 17_600),
        allow("look for what stopped", Act::Toward(2300, 1800), 1800),
        play("cross to the closed port", Act::Toward(2440, 2000), 720),
        play("charge", Act::Charge(2440, 2000), FULL_CHARGE_STEPS),
        play("release", Act::Release, 1),
        play("back to the band", Act::Toward(2300, 1500), 720),
        play("carry the run past the closure", Act::Follow(1), 14_200),
        play("sustain the circuit under Flood", Act::Follow(1), 24_000),
    ]);
    phases
}

/// One milestone the run reached, and the step it reached it at.
#[derive(Clone, Debug)]
struct Milestone {
    name: String,
    step: Step,
}

/// One reading taken while a pressure stood active.
#[derive(Clone, Debug)]
struct Sample {
    step: Step,
    stage: String,
    /// The Node the pressure's own hold stands on, and none before it holds
    /// one. Flood is authored with no target, so what it presses is whatever
    /// the run has been using.
    bound: Option<u32>,
    /// What the Node the hold stands on held, in whole units.
    target_units: i64,
    /// What the stores held together, in whole units.
    bank_units: i64,
}

/// One working target Flood held, and the reading that decided it.
///
/// The hold is made at every stage entry, off the retained trace as it stands
/// at that boundary. `ranked` is the same trailing-throughput ranking
/// recomputed by this file at the last batch before the entry, so what is
/// asserted is the rule rather than a Node identifier written down twice.
#[derive(Clone, Debug)]
struct Hold {
    step: Step,
    stage: String,
    bound: u32,
    ranked: Option<u32>,
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
    /// Every step at which the sequence entered a recoverable setback.
    setbacks: Vec<Step>,
    recovery_step: Option<Step>,
    transition_step: Option<Step>,
    /// The step the authored Port closure was first seen on.
    closure_step: Option<Step>,
    /// The step the authored drain spike was first seen on.
    spike_step: Option<Step>,
    /// Each pressure stage the run stood in, in order.
    pressure_steps: Vec<(Step, &'static str, String)>,
    /// Readings taken under Flood.
    samples: Vec<Sample>,
    /// Every working target Flood held, in order, with the ranking this file
    /// recomputed at the last batch before the entry that held it.
    holds: Vec<Hold>,
    /// The last `(stage, bound)` pair Flood was seen standing at, so a new one
    /// is read as a stage entry rather than as another sample of the same one.
    flood_seen: Option<(String, Option<u32>)>,
    /// The Node the run's own trailing flows made the heaviest, read at the
    /// last step before Flood took its seat — before the admission ended the
    /// window and left the trace with nothing to rank.
    heaviest: Option<u32>,
    /// The same reading, taken every batch of the final challenge.
    heaviest_now: Option<u32>,
    spare_open: bool,
    /// Charge held across the standing inside at the chapter's close, in whole
    /// units: the run's own reserve.
    reserve_units: i64,
    /// Charge held across the four stores at the close, in whole units.
    store_units: i64,
    form_units: i64,
    forms: usize,
    routes: usize,
    /// Trail entries still standing in this chapter's Field at the latest
    /// pre-transition reading.
    pending_trails: usize,
    /// Whole-unit capacities of the Routes the player formed in this chapter,
    /// in Route identifier order.
    formed_route_capacities: Vec<i64>,
    /// Impulse the run stood with at the close.
    impulse: u8,
    /// What the objective stream left standing, after every `objective_changed`
    /// the run raised: the step, and every one of this chapter's objectives in
    /// a live state on it.
    ///
    /// This is read off the reported stream rather than off `progress`, which
    /// holds one objective by construction and so could never disagree. What
    /// the shell draws is the stream, and the stream is what has to carry the
    /// one-at-a-time rule.
    standing: Vec<(Step, Vec<String>)>,
    /// The state the stream last reported for each of this chapter's
    /// objectives, which is what `standing` is derived from.
    reported: Vec<(String, String)>,
    /// The least the run held across the standing inside on any step Flood
    /// stood, in whole units.
    ///
    /// It is the one measure here that is neither authored nor a count of
    /// authored things: it is what the pressure actually cost this layout while
    /// it stood, and it is the axis on which a layout that banks little can beat
    /// one that banks a great deal.
    flood_low_reserve: i64,
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
                if id.starts_with("objective.the_loop.") {
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
///
/// The frames carry their own timestamps because a ramp is real time: the rest
/// of the script runs at `t_us` 0, so the gap the accumulator reads is clamped
/// to zero everywhere except here, and a ramp stands still until this asks it
/// to move.
fn edit(session: &mut Session, seq: &mut u32, plans: &[&str]) {
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

/// The Node the retained trace's own flows make the heaviest — the largest sum
/// of Route inflow plus outflow over the trailing steps, smallest NodeId on
/// equal sums.
///
/// It is the same reading the Flood hold makes, restated here so the chapter's
/// authored Flood target can be checked against what the run's records say
/// rather than against what this file believes.
fn heaviest_throughput(session: &Session) -> Option<u32> {
    let state = session.run().expect("a run").state();
    let ends: std::collections::BTreeMap<u32, (u32, u32)> =
        state.now.routes.iter().map(|route| (route.route, (route.tail, route.head))).collect();
    let steps = state.trace.steps.len();
    let mut sums: std::collections::BTreeMap<u32, i64> = std::collections::BTreeMap::new();
    for recorded in state.trace.steps.iter().skip(steps.saturating_sub(60)) {
        for (route, flow) in &recorded.records.f {
            let Some((tail, head)) = ends.get(route) else { continue };
            *sums.entry(*tail).or_insert(0) += flow;
            *sums.entry(*head).or_insert(0) += flow;
        }
    }
    let mut best: Option<(u32, i64)> = None;
    for port in &state.now.ports {
        let sum = sums.get(&port.node).copied().unwrap_or(0);
        if best.is_none_or(|(_, held)| sum > held) {
            best = Some((port.node, sum));
        }
    }
    best.map(|(node, _)| node)
}

/// Drives one run: a named starting Form, a route layout, whether the modelled
/// allowances are spent, whether the optional test is taken, and whether the
/// Form ever holds the supply band at all.
///
/// `supplies` false is the no-direct-play run: it draws every Route and opens
/// every Port the layout needs, and then rests where it stands instead of
/// holding the band. Nothing else about it differs.
fn drive_full(
    form: &str,
    layout: Layout,
    allowances: bool,
    takes_the_test: bool,
    supplies: bool,
    edits: bool,
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
        closure_step: None,
        spike_step: None,
        pressure_steps: Vec::new(),
        samples: Vec::new(),
        holds: Vec::new(),
        flood_seen: None,
        heaviest: None,
        heaviest_now: None,
        spare_open: false,
        reserve_units: 0,
        store_units: 0,
        form_units: 0,
        forms: 0,
        routes: 0,
        pending_trails: 0,
        formed_route_capacities: Vec::new(),
        impulse: 0,
        standing: Vec::new(),
        reported: Vec::new(),
        flood_low_reserve: i64::MAX,
    };
    // The run opens in The Pull, so the two chapters before this one are played
    // by the shared campaign script and this file picks the run up at its own
    // opening step.
    let mut driver = support::campaign::Driver::new();
    // Read once and played from: `authored()` parses the whole bundle, and a
    // chapter loop that calls it per chapter parses it per chapter.
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
        "the run stands in The Loop",
    );
    driver.drain(&mut session);
    // The chapter's own opening step, off the transition the shared driver
    // already drained: the run's step counter is the campaign's and the driver
    // may have run a batch past the boundary before it stopped.
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
    // The chapter's opening objective is offered on the transition step, which
    // the shared driver has already drained; it is read back off that list so
    // the record does not miss the one thing it opens with.
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
                    .is_some_and(|id| id.starts_with("objective.the_loop."))
        })
        .cloned()
        .collect();
    record(&mut driven, carried);
    record(&mut driven, support::campaign::raised(&mut session));
    let mut seq = driver.seq;

    let mut spur_open = true;
    let mut closed = 0;
    for phase in script(layout, takes_the_test) {
        if phase.allowance && !allowances {
            continue;
        }
        if let Act::Edit(plans) = phase.act {
            if edits {
                edit(&mut session, &mut seq, plans);
                record(&mut driven, support::campaign::raised(&mut session));
            }
            closed += 1;
            continue;
        }
        let mut left = phase.steps;
        while left > 0 {
            // The run stops at the chapter's own boundary: what is being driven
            // is one chapter, and the steps past it belong to the next.
            if driven.transition_step.is_some() {
                break;
            }
            let at = controlled(&session);
            // The no-direct-play run draws every Route and opens every Port the
            // layout needs, and then rests where it stands: everything the
            // chapter can be given by editing, and nothing it can only be given
            // by playing.
            let act = if supplies || closed < 2 { phase.act } else { Act::Rest };
            let (steer, held, release) = match act {
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
            // What the Field itself says, read after the events so a batch that
            // crossed the chapter boundary is not read as this chapter's.
            if driven.transition_step.is_some() {
                continue;
            }
            let open_now = state.now.ports.iter().any(|port| port.node == 4 && port.open);
            if spur_open && !open_now {
                driven.closure_step = Some(state.now.step);
            }
            spur_open = open_now;
            if driven.spike_step.is_none()
                && state.now.layers.iter().any(|layer| layer.layer == 0 && layer.drain > 0)
            {
                driven.spike_step = Some(state.now.step);
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
                if pressure.pressure == Pressure::Flood {
                    if driven.heaviest.is_none() {
                        driven.heaviest = driven.heaviest_now;
                    }
                    let bound = pressure.bound.map(|held| held.id);
                    // A `(stage, bound)` pair this run has not stood at before
                    // is a stage entry: the hold is made on one, and on nothing
                    // else. `ranked` is the sample taken at the end of the
                    // previous batch, which is the newest reading of the trace
                    // this file can take before the boundary ends the window.
                    let seen = (stage.clone(), bound);
                    if driven.flood_seen.as_ref() != Some(&seen) {
                        if let Some(id) = bound {
                            driven.holds.push(Hold {
                                step: state.now.step,
                                stage: stage.clone(),
                                bound: id,
                                ranked: driven.heaviest_now,
                            });
                        }
                        driven.flood_seen = Some(seen);
                    }
                    let state = session.run().expect("a run").state();
                    let standing_reserve: i64 = state
                        .now
                        .ports
                        .iter()
                        .filter(|port| state.view.inside.contains(&port.node))
                        .map(|port| port.q / ONE_UNIT)
                        .sum();
                    driven.flood_low_reserve = driven.flood_low_reserve.min(standing_reserve);
                    driven.samples.push(Sample {
                        step: state.now.step,
                        stage,
                        bound,
                        target_units: bound.map_or(0, |node| {
                            state
                                .now
                                .ports
                                .iter()
                                .find(|port| port.node == node)
                                .map_or(0, |port| port.q / ONE_UNIT)
                        }),
                        bank_units: state
                            .now
                            .ports
                            .iter()
                            .filter(|port| (6..=9).contains(&port.node))
                            .map(|port| port.q / ONE_UNIT)
                            .sum(),
                    });
                }
            }
            // Sampled every batch of the final challenge, so a hold made at any
            // stage entry has a reading taken shortly before it to be read
            // against.
            if driven.offered.iter().any(|(id, _)| id.ends_with("close_and_sustain")) {
                driven.heaviest_now = heaviest_throughput(&session);
            }
            let state = session.run().expect("a run").state();
            driven.spare_open = state.now.ports.iter().any(|port| port.node == 9 && port.open);
            driven.reserve_units = state
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
                .filter(|port| (6..=9).contains(&port.node))
                .map(|port| port.q / ONE_UNIT)
                .sum();
            driven.form_units = state
                .now
                .forms
                .iter()
                .find(|form| form.controlled)
                .map_or(0, |form| form.charge / ONE_UNIT);
            driven.forms = state.now.forms.len();
            driven.routes = state.now.routes.len();
            driven.pending_trails = state.now.pending.len();
            driven.formed_route_capacities = state
                .now
                .routes
                .iter()
                .filter(|route| route.formed_step >= driven.opening_step)
                .map(|route| route.capacity / ONE_UNIT)
                .collect();
            driven.impulse = state.progress.impulse;
        }
    }

    if driven.flood_low_reserve == i64::MAX {
        driven.flood_low_reserve = 0;
    }
    driven
}

/// The ordinary driven run: it supplies and it edits.
fn drive_as(form: &str, layout: Layout, allowances: bool, takes_the_test: bool) -> Driven {
    drive_full(form, layout, allowances, takes_the_test, true, true)
}

/// The attentive first run: every allowance spent, the spare taken.
fn attentive() -> Driven {
    drive_as("thread", Layout::Near, true, true)
}

/// The authored floor: no allowance spent, the spare taken.
fn floor() -> Driven {
    drive_as("thread", Layout::Near, false, true)
}

/// The authored content, read as the worker hands it over.
fn authored() -> content::Content {
    let bundle = field_game_core::json::parse(&support::bundle_with(&support::content_hash()))
        .expect("canonical");
    content::read_bundle(&bundle).expect("the content reads")
}

/// The chapter this file is about.
fn chapter() -> content::Chapter {
    authored().chapter(CHAPTER).expect("The Loop").clone()
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
fn the_chapter_completes_and_carries_the_run_into_its_own_transition() {
    let run = attentive();

    // The milestone table, printed so a reader can check the marks against the
    // chapter's own pacing: `cargo test --test chapter_the_loop -- --nocapture`.
    println!("\nThe Loop, attentive first run — milestone table (steps from the chapter's opening)");
    println!("{:<48} {:>8} {:>8}", "milestone", "step", "minutes");
    for milestone in &run.milestones {
        let at = milestone.step - run.opening_step;
        println!("{:<48} {at:>8} {:>8}", milestone.name, minutes(at));
    }
    println!("offered at");
    for (id, step) in &run.offered {
        let at = step - run.opening_step;
        println!("  {id:<48} {at:>8} {:>8}", minutes(at));
    }
    for (name, step) in [
        ("the spur closes", run.closure_step),
        ("the layer's loss rises", run.spike_step),
    ] {
        if let Some(step) = step {
            let at = step - run.opening_step;
            println!("{name:<48} {at:>8} {:>8}", minutes(at));
        }
    }
    println!("recoverable setbacks at");
    for step in &run.setbacks {
        let at = step - run.opening_step;
        println!("  {at:>8} {:>8}", minutes(at));
    }

    assert_eq!(
        run.completed,
        authored_order(),
        "every objective completes once each, in the authored order",
    );
    let transition = run.transition_step.expect("the chapter completes and the run carries on");
    assert_eq!(
        transition,
        run.at("objective.the_loop.close_and_sustain").expect("the last"),
        "the transition lands on the step the final challenge completed",
    );
    assert_eq!(
        run.anchor_step,
        run.at("objective.the_loop.hold_the_run"),
        "the chapter's one Anchor is written at the step the circuit closed",
    );
}

#[test]
fn the_chapter_asks_for_about_fifty_minutes_and_the_floor_stands_under_it() {
    let floor = floor();
    let attentive = attentive();
    let floor_at = floor.span();
    let attentive_at = attentive.span();

    let spent: u32 = script(Layout::Near, true)
        .iter()
        .filter(|phase| phase.allowance)
        .map(|phase| phase.steps)
        .sum();
    println!("\nThe Loop — the authored floor against the attentive first run");
    println!("  authored floor      {floor_at:>7} steps  {:>6} minutes", minutes(floor_at));
    println!("  attentive first run {attentive_at:>7} steps  {:>6} minutes", minutes(attentive_at));
    println!("modelled first-run allowances, {spent} steps in all:");
    for phase in script(Layout::Near, true).iter().filter(|phase| phase.allowance) {
        println!("  {:<40} {:>6} steps", phase.label, phase.steps);
    }
    println!("\nthe authored floor — milestone table");
    println!("{:<48} {:>8} {:>8}", "milestone", "step", "minutes");
    for milestone in &floor.milestones {
        let at = milestone.step - floor.opening_step;
        println!("{:<48} {at:>8} {:>8}", milestone.name, minutes(at));
    }

    let minute = 60 * STEPS_PER_SECOND;
    // Fifty minutes is the chapter's own figure, and the attentive run is what
    // it is read against. The band is wide enough that a re-tuned phase moves a
    // number a reader can check rather than one that quietly drifts.
    assert!(
        attentive_at > 46 * minute && attentive_at < 54 * minute,
        "the attentive run completes the chapter in {attentive_at} steps ({} minutes), \
         outside the fifty-minute band",
        minutes(attentive_at),
    );
    assert!(
        floor_at > 36 * minute && floor_at < attentive_at,
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
        "{:<8} {:>10} {:>8} {:>8} {:>8} {:>6} {:>8} {:>6} {:>7}",
        "form", "objectives", "steps", "minutes", "setbacks", "forms", "reserve", "held", "bank",
    );
    let mut evidence: Vec<(&str, usize, usize, Vec<i64>)> = Vec::new();
    for form in field_game_core::run::FORMS {
        let run = drive_as(form, Layout::Near, false, true);
        assert_eq!(run.completed, authored, "{form} completes the whole chapter in order");
        let at = run.span();
        for stage in &run.stages {
            assert!(
                ["hidden", "active", "complete", "failed_recoverable"].contains(&stage.as_str()),
                "{form} stood in an unexpected objective state: {stage}",
            );
        }
        assert!(run.spare_open, "{form} opened the spare junction the optional test names");
        assert_eq!(run.routes, 9, "{form} drew the two Routes the circuit needs");
        println!(
            "{form:<8} {:>10} {at:>8} {:>8} {:>8} {:>6} {:>8} {:>6} {:>7}",
            run.completed.len(),
            minutes(at),
            run.setbacks.len(),
            run.forms,
            run.reserve_units,
            run.form_units,
            run.store_units,
        );
        evidence.push((
            form,
            run.forms,
            run.pending_trails,
            run.formed_route_capacities.clone(),
        ));
    }
    // Closing Charge is not required to encode chassis identity: every run is
    // under this chapter's one physical-compartment coefficient. Pin the three
    // mechanisms this script actually exercises instead. Chorus stands its
    // linked group up, Wake leaves delayed entries, and Relay authors twice the
    // capacity on each Route the player forms.
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
        [64, 64],
        "Relay doubles both player-formed Routes",
    );
    for held in evidence.iter().filter(|held| held.0 != "relay") {
        assert_eq!(
            held.3.as_slice(),
            [32, 32],
            "{} forms the two Routes at the ordinary capacity",
            held.0,
        );
    }
}

// ---------------------------------------------------------------------------
// Both hands: direct control and Still Mode
// ---------------------------------------------------------------------------

#[test]
fn the_circuit_cannot_be_closed_by_steering_alone() {
    // The first necessity, pinned: a run that plays the whole script but never
    // enters Still Mode opens every Port the chapter asks for and still cannot
    // start the run carrying Charge, because nothing on this field gives the
    // intake anything. The only Route from the supply to the run is one the
    // player draws.
    let run = drive_full("thread", Layout::Near, false, true, true, false);
    println!(
        "\nno Still Mode: {} objectives completed, {} Routes standing, transition {:?}",
        run.completed.len(),
        run.routes,
        run.transition_step,
    );
    assert_eq!(run.routes, 7, "no Route was drawn, so the seven authored ones stand alone");
    assert!(
        run.completed.contains(&"objective.the_loop.open_the_run".to_string()),
        "steering and the Pulse carry the run as far as the Ports",
    );
    assert!(
        !run.completed.contains(&"objective.the_loop.start_the_run".to_string()),
        "and no further: the run never carries Charge",
    );
    assert!(run.transition_step.is_none(), "the chapter does not complete");
}

#[test]
fn the_circuit_cannot_be_held_by_routing_alone() {
    // The second necessity, pinned: a run that draws every Route and opens
    // every Port, and then lets go of the supply band, cannot hold the pattern.
    // The circuit carries Charge back to the intake but makes none, and every
    // Node of it pays upkeep every step, so what is not carried in is not
    // there.
    let run = drive_full("thread", Layout::Near, false, true, false, true);
    println!(
        "\nno direct play past the closing: {} objectives completed, transition {:?}, \
         reserve {} bank {}",
        run.completed.len(),
        run.transition_step,
        run.reserve_units,
        run.store_units,
    );
    assert!(
        run.completed.contains(&"objective.the_loop.start_the_run".to_string()),
        "the routing itself lands: the run carries Charge once the Route is drawn",
    );
    // The run stops at the fifth objective, not the seventh: `hold_the_run` is
    // the first that asks for the pattern to be *held*, and a circuit that
    // makes no Charge cannot hold it. Naming a later objective would pass for
    // the wrong reason — every objective after the fifth is out of reach once
    // the fifth is.
    assert!(
        !run.completed.contains(&"objective.the_loop.hold_the_run".to_string()),
        "the circuit cannot be held once the Form stops carrying Charge to it: {:?}",
        run.completed,
    );
    assert!(run.transition_step.is_none(), "the chapter does not complete");
    assert_eq!(run.reserve_units, 0, "the run stands empty");
}

// ---------------------------------------------------------------------------
// The route layouts
// ---------------------------------------------------------------------------

#[test]
fn no_single_route_layout_dominates_the_driven_runs() {
    // The chapter's own success measures, and nothing composed out of them:
    // how long it took, what the run held across the standing inside at the
    // close, what the stores had banked, how many recoverable setbacks it stood
    // in, and the least the run held while Flood stood. A layout dominates
    // another when it is no worse on every one of them and better on one.
    //
    // The last two are what the first round was missing. `setbacks` was
    // collected and then left out of the relation, and every other measure was
    // a reading of authored quantities — so a layout could be worse to play and
    // still read as incomparable. `flood_low` is neither authored nor a count
    // of authored things: it is what the pressure cost this layout while it
    // stood.
    #[derive(Debug)]
    struct Reading {
        layout: Layout,
        steps: Step,
        reserve: i64,
        bank: i64,
        setbacks: usize,
        flood_low: i64,
    }

    let mut readings: Vec<Reading> = Vec::new();
    println!("\nthe route layouts, each driven through the whole chapter");
    println!(
        "{:<10} {:>8} {:>8} {:>9} {:>7} {:>9} {:>7} {:>10}",
        "layout", "steps", "minutes", "setbacks", "routes", "reserve", "bank", "flood low",
    );
    for layout in LAYOUTS {
        let run = drive_as("thread", layout, false, true);
        assert_eq!(
            run.completed,
            authored_order(),
            "{} completes the whole chapter in order",
            layout.name(),
        );
        let at = run.span();
        println!(
            "{:<10} {at:>8} {:>8} {:>9} {:>7} {:>9} {:>7} {:>10}",
            layout.name(),
            minutes(at),
            run.setbacks.len(),
            run.routes,
            run.reserve_units,
            run.store_units,
            run.flood_low_reserve,
        );
        readings.push(Reading {
            layout,
            steps: at,
            reserve: run.reserve_units,
            bank: run.store_units,
            setbacks: run.setbacks.len(),
            flood_low: run.flood_low_reserve,
        });
    }

    // Every layout completes, so completion separates none of them; what
    // separates them is the five below, none of them composed with another.
    let dominates = |a: &Reading, b: &Reading| {
        a.steps <= b.steps
            && a.reserve >= b.reserve
            && a.bank >= b.bank
            && a.setbacks <= b.setbacks
            && a.flood_low >= b.flood_low
            && (a.steps < b.steps
                || a.reserve > b.reserve
                || a.bank > b.bank
                || a.setbacks < b.setbacks
                || a.flood_low > b.flood_low)
    };

    // The same relation over the four measures a player can actually read at the
    // close — how long, what the run holds, what the stores hold, how many
    // setbacks. `flood_low` is a reading taken mid-pressure that no surface
    // shows, so a layout that stands on the frontier by that alone is not a
    // choice a player could be making.
    let readable = |a: &Reading, b: &Reading| {
        a.steps <= b.steps
            && a.reserve >= b.reserve
            && a.bank >= b.bank
            && a.setbacks <= b.setbacks
            && (a.steps < b.steps
                || a.reserve > b.reserve
                || a.bank > b.bank
                || a.setbacks < b.setbacks)
    };

    let frontier = |relation: &dyn Fn(&Reading, &Reading) -> bool| -> Vec<&'static str> {
        readings
            .iter()
            .filter(|first| {
                !readings.iter().any(|other| other.layout != first.layout && relation(other, first))
            })
            .map(|held| held.layout.name())
            .collect()
    };
    let live = frontier(&readable);
    println!("layouts no other dominates, on the four readable measures: {live:?}");
    println!("the same, with the mid-pressure low added: {:?}", frontier(&dominates));
    for first in &readings {
        let beaten: Vec<&str> = readings
            .iter()
            .filter(|other| other.layout != first.layout)
            .filter(|other| readable(first, other))
            .map(|other| other.layout.name())
            .collect();
        if !beaten.is_empty() {
            println!("  {} dominates {beaten:?}", first.layout.name());
        }
    }

    // No layout is better than every other on every measure — the goal's own
    // done-when.
    for first in &readings {
        let beaten = readings
            .iter()
            .filter(|other| other.layout != first.layout)
            .filter(|other| dominates(first, other))
            .count();
        assert!(
            beaten < readings.len() - 1,
            "{} is no worse than every other layout on every measure",
            first.layout.name(),
        );
    }
    // And the frontier holds more than one layout: at least one pair trades a
    // faster close against a fuller run. `LIVE_LAYOUTS` is the count this
    // chapter claims, asserted as a floor — a re-tuned store that drops the
    // frontier below it fails here rather than degrading quietly.
    assert!(
        live.len() >= LIVE_LAYOUTS,
        "only {} of the four route layouts stand on the frontier: {live:?}",
        live.len(),
    );
    let incomparable = readings.iter().any(|first| {
        readings.iter().any(|other| {
            other.layout != first.layout && !dominates(first, other) && !dominates(other, first)
        })
    });
    assert!(incomparable, "every layout is ordered against every other: {readings:?}");
}

// ---------------------------------------------------------------------------
// Flood, and the most-used route
// ---------------------------------------------------------------------------

#[test]
fn flood_holds_the_node_the_runs_own_records_make_the_heaviest() {
    // The chapter's headline pressure targets **what the run uses**, not a Node
    // the author picked: it is scheduled with no target at all, and the locked
    // reading holds the heaviest-throughput Node at every stage entry. That is
    // the goal's own sentence — "Flood targets the most-used route" — as a rule
    // rather than as a coincidence of placement, and it is why this is driven
    // through all four route layouts: each of them uses a different store, and
    // a hold that followed the authoring rather than the play would answer the
    // same Node in all four.
    let chapter = chapter();
    let flood = chapter
        .pressure_schedule
        .iter()
        .find(|entry| entry.pressure == Pressure::Flood)
        .expect("the chapter schedules Flood");
    assert_eq!(flood.target.kind, TargetKind::None, "aimed at nothing the author named");
    assert_eq!(flood.target.id, None, "a kind that names nothing carries no identifier");
    assert!(flood.primary, "and it is the chapter's own headline pressure");

    println!("\nFlood's working target, held at every stage entry, in each layout");
    println!(
        "{:<10} {:>8} {:<12} {:>7} {:>8}",
        "layout", "step", "stage", "held", "ranked",
    );
    for layout in LAYOUTS {
        let run = drive_as("thread", layout, false, true);
        assert!(
            !run.holds.is_empty(),
            "{}: Flood stood its whole life and held no working target",
            layout.name(),
        );
        for hold in &run.holds {
            println!(
                "{:<10} {:>8} {:<12} {:>7} {:>8}",
                layout.name(),
                hold.step - run.opening_step,
                hold.stage,
                hold.bound,
                format!("{:?}", hold.ranked),
            );
        }
        for hold in &run.holds {
            assert_eq!(
                Some(hold.bound),
                hold.ranked,
                "{}: the hold at the {} entry stands on node {}, which is not the Node this \
                 run's own trailing flows make the heaviest",
                layout.name(),
                hold.stage,
                hold.bound,
            );
        }
        // Every stage of the pressure's life makes a hold, so the working
        // target follows the run rather than being fixed at the admission.
        let stages: std::collections::BTreeSet<&str> =
            run.holds.iter().map(|hold| hold.stage.as_str()).collect();
        assert!(
            stages.len() >= 3,
            "{}: the hold was made at {} of the four stage entries: {:?}",
            layout.name(),
            stages.len(),
            stages,
        );
    }

    // And it bites: what the hold stands on sheds while the pressure stands,
    // and what the circuit had banked pays for it.
    let run = attentive();
    println!("\nFlood over the final challenge");
    println!("{:>8} {:<12} {:>7} {:>8} {:>8}", "step", "stage", "held", "target", "bank");
    for sample in run.samples.iter().step_by(8) {
        println!(
            "{:>8} {:<12} {:>7} {:>8} {:>8}",
            sample.step - run.opening_step,
            sample.stage,
            format!("{:?}", sample.bound),
            sample.target_units,
            sample.bank_units,
        );
    }
    let opening = run.samples.first().expect("Flood stood").bank_units;
    let lowest = run.samples.iter().map(|sample| sample.bank_units).min().expect("a reading");
    assert!(
        lowest < opening,
        "the bank stood at {opening} when Flood arrived and never fell below it",
    );
    assert!(
        run.samples.iter().any(|sample| sample.stage == "crisis"),
        "the pressure ran its whole course inside the final challenge",
    );
}

#[test]
fn the_pressures_land_inside_the_objectives_written_for_them() {
    // `pressure_schedule` counts from the chapter's opening and an objective is
    // offered when a player reaches it, so the two are reconciled by the one
    // lever authoring has: the challenge asks for long enough that the latest
    // path has reached it before the earliest has left it, and Flood is
    // scheduled into the span every path stands inside. Four paths, because a
    // run that lets the optional test go arrives later than one that takes it.
    let chapter = chapter();
    let flood = chapter
        .pressure_schedule
        .iter()
        .find(|entry| entry.pressure == Pressure::Flood)
        .expect("the chapter schedules Flood");
    // How long the pressure stands is its authored table's own sum, read off
    // the content rather than written down here: a re-tuned table moves this
    // window without moving a figure in this file.
    let life = authored()
        .pressures
        .table(Pressure::Flood)
        .expect("the manifest authors Flood's table")
        .span() as Step;
    let last = chapter.objectives.last().expect("a last objective").id.clone();

    println!("\nthe final challenge, on all four paths (steps from the chapter's opening)");
    println!("{:<24} {:>10} {:>12}", "path", "offered", "completed");
    let mut latest_offer = 0;
    let mut earliest_close = Step::MAX;
    for (named, allowances, takes) in [
        ("floor, spare taken", false, true),
        ("floor, spare let go", false, false),
        ("attentive, spare taken", true, true),
        ("attentive, spare let go", true, false),
    ] {
        let run = drive_as("thread", Layout::Near, allowances, takes);
        let offered = run.offered_at(&last).expect("the final challenge is offered")
            - run.opening_step;
        let completed = run.at(&last).expect("and completed") - run.opening_step;
        println!("{named:<24} {offered:>10} {completed:>12}");
        latest_offer = latest_offer.max(offered);
        earliest_close = earliest_close.min(completed);
        assert!(
            flood.start_step > offered && flood.start_step + life < completed,
            "{named}: Flood runs from {} to {}, outside the challenge it was written for \
             ({offered} to {completed})",
            flood.start_step,
            flood.start_step + life,
        );
    }
    println!(
        "the binding window is {latest_offer} to {earliest_close}; Flood runs {} to {}",
        flood.start_step,
        flood.start_step + life,
    );
    assert!(
        latest_offer < earliest_close,
        "the four paths leave no span they all stand inside",
    );
}

#[test]
fn the_authored_events_land_at_their_own_steps_in_every_path() {
    // The chapter's two events are timed against objectives rather than against
    // the run's step counter, so they land at the same offset in a run that
    // took fifty minutes to reach them and one that took forty.
    let chapter = chapter();
    assert_eq!(chapter.events.len(), 2, "the chapter authors two events");
    println!("\nthe two authored events, and the step each was first read at");
    println!("{:<20} {:<16} {:>9} {:>7} {:>8} {:>6}", "path", "event", "offered", "due", "read", "late");
    for (named, run) in [("the authored floor", floor()), ("the attentive run", attentive())] {
        for event in &chapter.events {
            let offered = run.offered_at(&event.objective).expect("the objective was offered");
            let (kind, landed) = match event.effect {
                content::Effect::PortOpen { .. } => ("the Port closes", run.closure_step),
                content::Effect::LayerDrain { .. } => ("the loss rises", run.spike_step),
                content::Effect::CurrentActive { .. } | content::Effect::RouteCut { .. } => {
                    ("no reading", None)
                }
            };
            let landed = landed.expect("the event landed");
            let due = offered + event.at;
            println!(
                "{named:<20} {kind:<16} {:>9} {:>7} {:>8} {:>6}",
                offered - run.opening_step,
                due - run.opening_step,
                landed - run.opening_step,
                landed - due,
            );
            // What is exact is the trigger: `at` steps after the objective was
            // offered, on one boundary. What is *read* here is not, and the
            // slack is the sampler's, not the rule's — this file drives in
            // fifteen-step batches and reads the Field once at the end of each,
            // so a landing is placed to within one batch of its own step. The
            // measured slack, on both paths, is two steps for the Port closure
            // and thirteen for the drain spike — where each due step happens to
            // fall inside its batch, not anything the rule does. The band is
            // the batch width and not the measurement, so a re-timed phase
            // moves the reading without failing the test for the wrong reason.
            assert!(
                landed >= due && landed < due + Step::from(BATCH),
                "{named}: the event was read at {landed}, not inside the batch its own step \
                 ({due}) falls in",
            );
        }
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
    // It asks for a Pulse released beside a Port a long way from everything the
    // chapter requires, so a Form standing still never meets it: the shape an
    // optional test has to have, or a run that does nothing would pass it.
    assert!(matches!(tests[0].condition, content::Condition::PortsOpen { .. }));

    let taken = drive_as("thread", Layout::Near, false, true);
    assert!(taken.spare_open, "the test is passable");
    assert!(taken.completed.contains(&test), "and a taken test is a completed objective");
    assert!(taken.transition_step.is_some(), "and the chapter completes");

    let let_go = drive_as("thread", Layout::Near, false, false);
    assert!(!let_go.spare_open, "the test is skippable");
    assert!(!let_go.completed.contains(&test), "a test that was not taken was not completed");
    assert!(let_go.transition_step.is_some(), "and the chapter completes without it");
    // The span it stood for is the authored one: the sequence installs what
    // follows it at `started_step + span`.
    let offered = let_go.offered_at(&test).expect("the test was offered");
    let after = let_go
        .offered_at("objective.the_loop.carry_the_circuit")
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
fn the_authored_collapse_stands_and_the_chapter_carries_past_it() {
    // The chapter's authored break: the outfall has nowhere to send what
    // arrives, so it carries more than it can hold and the objective reads
    // `failed_recoverable` while it does. Closing the circuit carries it
    // through. Nothing in the closed state set is terminal and the run never
    // leaves `running`.
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
    let minute = 60 * STEPS_PER_SECOND;
    let opening = collapse - run.opening_step;
    assert!(
        opening <= 12 * minute,
        "the first recoverable collapse is by minute twelve, and was at {opening}",
    );
    // Every recoverable setback the chapter has is one of these two: the
    // outfall with nowhere to send what arrives, and the Port the authored
    // event closes under the run.
    assert!(run.setbacks.len() >= 2, "the closure is a setback of its own: {:?}", run.setbacks);
}

// ---------------------------------------------------------------------------
// What the chapter authors
// ---------------------------------------------------------------------------

#[test]
fn the_runs_own_nodes_pay_upkeep_every_step_they_hold_anything() {
    // Self-support is a rule here rather than a word: every Node of the run
    // authors an `upkeep_rate`, the Node phase spends it out of what the Node
    // holds, and the coordinate profile reads the payment where it has always
    // looked for it. What is asserted is that the payment is real — a profile
    // that found none reports the reason instead — and the Self-Support
    // reading is printed beside it, because a run supplied from outside is a
    // run whose net import stands against its own upkeep.
    use field_game_core::coord;
    use field_game_core::slate::TAU_DEFAULT;

    let chapter = chapter();
    let inside = chapter.opening_view.inside.clone();
    for node in &inside {
        let port = chapter.ports.iter().find(|held| held.node == *node).expect("a placed Node");
        assert!(port.upkeep_rate > 0, "node {node} of the run authors no upkeep");
    }

    let mut session = Session::new(&support::worker_init()).expect("versions agree");
    let opened = session.command(
        "init_run",
        &format!("{{\"form\":\"thread\",\"mode\":\"new\",\"run_id\":\"{KEY}\"}}"),
    );
    assert!(opened.contains("\"ok\":true"), "{opened}");
    let mut driver = support::campaign::Driver::new();
    // Read once and played from: `authored()` parses the whole bundle, and a
    // chapter loop that calls it per chapter parses it per chapter.
    let content = authored();
    for index in 0..CHAPTER {
        support::campaign::play_chapter(
            &mut session,
            &mut driver,
            content.chapter(index).expect("a chapter"),
        );
    }
    let mut seq = driver.seq;

    let mut readings: Vec<(&str, Option<i64>, bool)> = Vec::new();
    let mut closed = 0;
    for phase in script(Layout::Near, false) {
        if phase.allowance {
            continue;
        }
        if let Act::Edit(plans) = phase.act {
            edit(&mut session, &mut seq, plans);
            closed += 1;
            continue;
        }
        let mut left = if closed >= 2 { phase.steps.min(3_000) } else { phase.steps };
        while left > 0 {
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
                Act::Edit(_) => unreachable!(),
            };
            let now = left.min(u32::from(BATCH)) as u16;
            frame(&mut session, &mut seq, now, steer, held, release, 0, false);
            left -= u32::from(now);
        }
        if closed == 1 && readings.is_empty() && phase.label == "hold while the run starts" {
            let state = session.run().expect("a run").state();
            let profile = coord::of(state, &state.view, TAU_DEFAULT);
            readings.push((
                "supplied, not yet closed",
                profile.self_support.value,
                profile.upkeep_reason.is_none(),
            ));
        }
        if closed >= 2 && readings.len() == 1 {
            let state = session.run().expect("a run").state();
            let profile = coord::of(state, &state.view, TAU_DEFAULT);
            readings.push((
                "closed",
                profile.self_support.value,
                profile.upkeep_reason.is_none(),
            ));
            break;
        }
    }
    println!("\nthe standing inside, read as a coordinate profile");
    println!("{:<28} {:>14} {:>10}", "when", "Self-Support", "upkeep paid");
    for (named, value, paid) in &readings {
        println!("{named:<28} {:>14} {paid:>10}", format!("{value:?}"));
    }
    assert_eq!(readings.len(), 2, "both readings were taken");
    for (named, _, paid) in &readings {
        assert!(*paid, "{named}: the profile found no upkeep at all across the run's Nodes");
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
    let total: i64 = spans.iter().map(|(_, _, span)| span).sum();
    println!("\nThe Loop — authored duration, by objective");
    for (id, target, span) in &spans {
        println!("  {id:<48} authored {target:>6}  spans {span:>6}");
    }
    println!("  {:<48} {:>15} {total:>12}", "in all", "");
    assert_eq!(spans.len(), 5, "five of the nine ask for time: {spans:?}");
    // The chapter's authored duration alone, before a step of travel, a Pulse,
    // or a queued change: a little under forty minutes of the fifty.
    assert!(
        total > 66_000 && total < 74_000,
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
    assert_eq!(
        run.standing.iter().filter(|(_, standing)| standing.len() == 1).count(),
        run.standing.len() - run.standing.iter().filter(|(_, s)| s.is_empty()).count(),
        "every reading is either one objective live or none",
    );
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
    assert!(run.progressed());
}

impl Driven {
    /// True when the run's objectives were offered in the authored order.
    fn progressed(&self) -> bool {
        let mut last = 0;
        for (id, step) in &self.offered {
            if !id.starts_with("objective.the_loop.") {
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


#[test]
fn the_shared_campaign_script_carries_the_chapter_to_its_own_boundary() {
    // The campaign run drives every chapter from `support::campaign`'s own
    // phase list rather than from this file's, and The Loop is not
    // rest-completable: the supply Route and the closing Route are queued
    // changes and the four Ports of the run are opened by Pulses. This is the
    // arm that chapter contributes, driven end to end so that a campaign run
    // reaching it finds a script that works.
    let mut session = Session::new(&support::worker_init()).expect("versions agree");
    let opened = session.command(
        "init_run",
        &format!("{{\"form\":\"thread\",\"mode\":\"new\",\"run_id\":\"{KEY}\"}}"),
    );
    assert!(opened.contains("\"ok\":true"), "{opened}");
    let mut driver = support::campaign::Driver::new();
    // Read once and played from: `authored()` parses the whole bundle, and a
    // chapter loop that calls it per chapter parses it per chapter.
    let content = authored();
    for index in 0..CHAPTER {
        support::campaign::play_chapter(
            &mut session,
            &mut driver,
            content.chapter(index).expect("a chapter"),
        );
    }
    assert_eq!(support::campaign::chapter_index(&session), CHAPTER);

    let script = support::campaign::script_for("the_loop").expect("the chapter authors a script");
    let stop = |session: &Session| support::campaign::chapter_index(session) > CHAPTER;
    for phase in &script {
        if stop(&session) {
            break;
        }
        driver.phase_until(&mut session, phase, &stop);
    }
    let stalled = session.run().expect("a run").state();
    assert!(
        stop(&session),
        "the shared campaign script completes The Loop and carries the run past it; \
         objective={} progress={}/{} routes={:?} nodes={:?}",
        stalled.progress.objective.id,
        stalled.progress.objective.progress,
        stalled.progress.objective.target.unwrap_or_default(),
        stalled
            .now
            .routes
            .iter()
            .map(|route| {
                (
                    route.route,
                    route.tail,
                    route.head,
                    route.flow / ONE_UNIT,
                    route.capacity / ONE_UNIT,
                )
            })
            .collect::<Vec<_>>(),
        stalled
            .now
            .ports
            .iter()
            .filter(|port| [2, 4, 5, 6].contains(&port.node))
            .map(|port| (port.node, port.q / ONE_UNIT, port.capacity / ONE_UNIT))
            .collect::<Vec<_>>(),
    );
    // And it lets the optional test go, which is what keeps a driven campaign's
    // completed list the campaign's required objectives exactly.
    let state = session.run().expect("a run").state();
    assert!(
        !state.progress.complete.iter().any(|id| id == "objective.the_loop.open_the_spare"),
        "the shared script passes over the optional test rather than taking it",
    );
}
