//! The Quiet Edge, driven end to end: the campaign's finale, and its endings.
//!
//! The chapter is one closed pattern and a supply that stops a piece at a time.
//! Three things carry it:
//!
//! - **The ring.** An intake, five arms, and a hold that carries back round to
//!   the intake. Every arm is fed eight a step and has nowhere to send it, so
//!   every arm needs a Route drawn out of it into the hold — five queued
//!   changes — or it stands over its own threshold and sheds a quarter of the
//!   excess each step. Once the five are drawn the pattern circulates in one
//!   Route pass and needs nothing from outside it.
//! - **The declining support.** Two currents deliver into the field, and the
//!   chapter stops both of them, one objective at a time, while raising the
//!   layer's own loss twice. By the last two objectives nothing outside the
//!   pattern delivers anything at all: what the ring carries is what the ring
//!   already held.
//! - **The arrival.** Impulse is the one reading that crosses a chapter
//!   boundary, and The Rewrite's three paths leave a run standing at six, five,
//!   or four of it. **Five Routes is the demand, and it binds before anything
//!   here completes**, so the three arrivals open the chapter three different
//!   ways: six draws the five and a sixth into the spare arm, five draws exactly
//!   the five, and four cannot draw them at all and takes the relief Port on the
//!   layer below — which costs no Impulse and about four thousand steps.
//!
//! **The endings.** Two optional tests stand at the top of the chapter, before
//! any completion has granted anything, and which of them a run completes is
//! what the chapter's `ending_marks` read. Eight Forms times three marks is
//! twenty-four endings, and every one of them is reached by playing the chapter
//! to its close rather than by being reported at it.
//!
//! **The independence test** is the last objective, and it is the campaign's
//! own thesis: `let_it_run` asks for 32,768 steps of a held pattern, and every
//! run driven here spends all of them with every input neutral — no steering, no
//! Pulse, no depth, no Still Mode — while Noise narrows what every Route on the
//! layer moves. The onboarding taught follow-the-current; this proves the player
//! no longer needs to.
//!
//! **How the runs get here, stated plainly.** Two preambles are used and they
//! answer different questions:
//!
//! - `the_three_rewrite_paths_carry_three_arrivals_into_the_finale` plays the
//!   **whole shipped campaign** — all seven chapters — and is what proves the
//!   arrivals are real and that every valid campaign path reaches a complete
//!   ending.
//! - Every other run stands the chapter up on a two-chapter manifest: a fixture
//!   chapter a resting run completes in about a thousand steps, then the finale.
//!   That leaves the run at the finale's opening carrying six Impulse, and an
//!   arrival of five or four is reached by committing one or two **no-op
//!   compartment reshapes** — a committed change that spends one Impulse and names
//!   exactly the members already standing, so the Field it leaves is the Field
//!   it found. It is an instrument, and it is used because twenty-four runs of
//!   the whole campaign would spend the suite's time on the seven chapters this
//!   file is not about.

use field_game_core::content::{self, Chapter, CONTENT_VERSION};
use field_game_core::fx::ONE_UNIT;
use field_game_core::json::Json;
use field_game_core::protocol::PROTOCOL_VERSION;
use field_game_core::state::{Step, SAVE_VERSION, STEPS_PER_SECOND};
use field_game_core::Session;

mod support;

use support::campaign::{controlled, nearest_point, toward, Driver};

/// A chapter a resting run completes in about a thousand steps, standing in the
/// first slot of the two-chapter manifest so the finale opens under a full
/// Impulse and nothing else. It authors nothing a player ever sees.
const CHAPTER_OPENING_FIXTURE: &str = include_str!("fixtures/chapter_opening.json");

/// The copy catalog, read here because the endings this chapter reaches are
/// catalog keys and a key no entry answers is an ending no player reads.
///
/// It is read as text rather than through the core's canonical parser: the
/// catalog is authored in play order, and canonical JSON asks for ascending
/// keys, so the parser refuses it by design.
const CATALOG: &str = include_str!("../../content/copy/catalog.json");

/// The entry one catalog key names, as the authored text between its key and
/// the close of its object, and none for a key the catalog does not answer.
fn catalog_entry(key: &str) -> Option<&'static str> {
    let opening = format!("\"{key}\": {{");
    let at = CATALOG.find(&opening)? + opening.len();
    let rest = &CATALOG[at..];
    let end = rest.find('}')?;
    Some(&rest[..end])
}

const KEY: &str = "00112233445566ee";

/// How many steps one driven batch runs before the script re-aims.
const BATCH: u16 = 15;

/// How many steps a full Pulse charge takes, from the locked charging rule.
const FULL_CHARGE_STEPS: u32 = 32;

/// This chapter's place in the shipped campaign.
const CHAPTER: u8 = 7;

/// The floor every four-path containment margin is held to, in steps. The
/// wave's own standard.
const PRESSURE_MARGIN: Step = 300;

/// The ring, and the Nodes the objectives name.
const ARMS: [u32; 5] = [3, 4, 5, 6, 7];
const INTAKE: u32 = 2;
const HOLD: u32 = 8;
const SPARE: u32 = 9;
const RELIEF: u32 = 10;
/// Every Node the chapter places, which is also the whole of its opening inside.
const EVERY_NODE: [u32; 10] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

/// Where the relief Port stands, in whole units.
const RELIEF_AT: (i64, i64) = (2900, 2900);
/// The band point a run parks on, in whole units.
const BAND_AT: (i64, i64) = (1500, 1500);

/// The chapter's own objectives, by their short names.
const CLOSE_THE_SPARE_ARM: &str = "objective.the_quiet_edge.close_the_spare_arm";
const CLOSE_THE_RING_AT_ONCE: &str = "objective.the_quiet_edge.close_the_ring_at_once";
const CLOSE_THE_RING: &str = "objective.the_quiet_edge.close_the_ring";
const TAKE_THE_DEEP_BAND: &str = "objective.the_quiet_edge.take_the_deep_band";
const HOLD_PAST_THE_DEEP_STOP: &str = "objective.the_quiet_edge.hold_past_the_deep_stop";
const CARRY_THE_CLOSED_FIELD: &str = "objective.the_quiet_edge.carry_the_closed_field";
const LET_IT_RUN: &str = "objective.the_quiet_edge.let_it_run";

/// The eight starting Forms, in the closed set's own order.
const FORMS: [&str; 8] =
    ["thread", "ring", "relay", "vault", "lens", "knot", "wake", "chorus"];

// ---------------------------------------------------------------------------
// The three arrivals
// ---------------------------------------------------------------------------

/// What a run carries into the finale, and therefore how it may open.
///
/// The figures are The Rewrite's own ladder, pinned there and re-pinned here by
/// `the_three_rewrite_paths_carry_three_arrivals_into_the_finale`: the
/// relocation carries six, the swap by parts five, and the wholesale four.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Arrival {
    /// Six Impulse — the relocation's. Affords the five arms and the spare.
    Six,
    /// Five Impulse — the swap by parts'. Affords the five arms exactly.
    Five,
    /// Four Impulse — the wholesale's. Affords four, and takes the relief.
    Four,
}

const ARRIVALS: [Arrival; 3] = [Arrival::Six, Arrival::Five, Arrival::Four];

impl Arrival {
    fn impulse(self) -> u8 {
        match self {
            Arrival::Six => 6,
            Arrival::Five => 5,
            Arrival::Four => 4,
        }
    }

    /// How many no-op reshapes stand the run at this arrival, from the six a
    /// fresh crossing leaves.
    fn spend(self) -> u8 {
        6 - self.impulse()
    }

    /// The ending mark this arrival reaches when it opens the chapter the way
    /// the arrival affords.
    fn mark(self) -> &'static str {
        match self {
            Arrival::Six => "spare",
            Arrival::Five => "whole",
            Arrival::Four => "steady",
        }
    }

    fn name(self) -> &'static str {
        match self {
            Arrival::Six => "six (relocation)",
            Arrival::Five => "five (by parts)",
            Arrival::Four => "four (wholesale)",
        }
    }
}

// ---------------------------------------------------------------------------
// The bundle these runs stand on
// ---------------------------------------------------------------------------

fn finale_manifest() -> String {
    "{\"chapters\":[\"the_pull\",\"the_quiet_edge\"],\"content_version\":2,\
     \"forms\":[\"thread\",\"ring\",\"relay\",\"vault\",\"lens\",\"knot\",\"wake\",\"chorus\"],\
     \"pressures\":[\"drain\",\"noise\",\"fracture\",\"flood\",\"interference\",\"drift\"]}"
        .to_string()
}

fn finale_files() -> Vec<String> {
    let mut listed = vec![
        CHAPTER_OPENING_FIXTURE.to_string(),
        support::CHAPTER_THE_QUIET_EDGE.to_string(),
    ];
    for form in [
        support::FORM_THREAD,
        support::FORM_RING,
        support::FORM_RELAY,
        support::FORM_VAULT,
        support::FORM_LENS,
        support::FORM_KNOT,
        support::FORM_WAKE,
        support::FORM_CHORUS,
    ] {
        listed.push(form.to_string());
    }
    for pressure in [
        support::PRESSURE_DRAIN,
        support::PRESSURE_NOISE,
        support::PRESSURE_FRACTURE,
        support::PRESSURE_FLOOD,
        support::PRESSURE_INTERFERENCE,
        support::PRESSURE_DRIFT,
    ] {
        listed.push(pressure.to_string());
    }
    listed
}

fn finale_init() -> String {
    format!(
        "{{\"content\":{},\"protocol\":{PROTOCOL_VERSION},\"save_version\":{SAVE_VERSION}}}",
        support::bundle_of(&finale_manifest(), &finale_files()),
    )
}

/// The shipped campaign, as the worker hands it across.
fn authored() -> content::Content {
    let bundle = field_game_core::json::parse(&support::bundle_with(&support::content_hash()))
        .expect("canonical");
    content::read_bundle(&bundle).expect("the content reads")
}

/// This chapter, as it is shipped.
fn chapter() -> Chapter {
    authored().chapter(CHAPTER).expect("The Quiet Edge").clone()
}

/// A step count as sim-minutes, to one decimal place, as a string.
fn minutes(step: Step) -> String {
    let tenths = i64::from(step) * 10 / (i64::from(STEPS_PER_SECOND) * 60);
    format!("{}.{}", tenths / 10, tenths % 10)
}

// ---------------------------------------------------------------------------
// The plan bodies
// ---------------------------------------------------------------------------

fn connect(from: u32, to: u32) -> String {
    format!("{{\"plan\":{{\"from\":{from},\"op\":\"connect\",\"to\":{to}}}}}")
}

/// A physical-compartment reshape naming exactly the members already standing:
/// a committed change that spends one Impulse and leaves the Field it found.
fn no_op_reshape() -> String {
    let written: Vec<String> = EVERY_NODE.iter().map(|held| held.to_string()).collect();
    format!("{{\"plan\":{{\"members\":[{}],\"op\":\"reshape_compartment\"}}}}", written.join(","))
}

/// One Route out of each arm into the hold, in arm order.
fn arm_outfalls() -> Vec<String> {
    ARMS.iter().map(|arm| connect(*arm, HOLD)).collect()
}

/// What supplies the spare arm, which already has its own way out.
fn spare_supply() -> String {
    connect(INTAKE, SPARE)
}

// ---------------------------------------------------------------------------
// One driven run
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
enum Act {
    Toward(i64, i64),
    Follow(u16),
    Rest,
    Charge(i64, i64),
    Release,
    Depth(i8),
}

/// One milestone the run reached, and the step it reached it at.
#[derive(Clone, Debug)]
struct Milestone {
    name: String,
    step: Step,
}

/// A sample of the ring, taken once a batch of the last two objectives.
#[derive(Clone, Copy, Debug)]
struct Sample {
    step: Step,
    ring: i64,
    carried: i64,
}

/// What one driven run recorded.
struct Driven {
    form: &'static str,
    #[allow(dead_code)]
    arrival: Arrival,
    milestones: Vec<Milestone>,
    completed: Vec<String>,
    offered: Vec<(String, Step)>,
    states: Vec<String>,
    setbacks: Vec<Step>,
    recovery_step: Option<Step>,
    opening_step: Step,
    end_step: Option<Step>,
    ending_id: Option<String>,
    continuation_unlocked: Option<bool>,
    anchors: usize,
    /// Every Impulse reading taken at a commit, in order: what was spent and
    /// what was left.
    commits: Vec<(Step, usize, u8)>,
    /// The step the relief Port was first read open, and none for a run that
    /// never opened it.
    relief_step: Option<Step>,
    /// Each pressure stage the run stood in, in order.
    pressure_steps: Vec<(Step, String, i64)>,
    /// What the ring carried, sampled once a batch of the final objective.
    samples: Vec<Sample>,
    /// The step the last objective was offered at, and the step it completed.
    hands_off: Option<(Step, Step)>,
    /// Every frame sent between those two steps carried nothing but neutral
    /// input; this counts them.
    neutral_frames: usize,
    /// Charge across the ring at the close, in whole units.
    ring_at_close: i64,
    /// What the ring carried per step at the close.
    carried_at_close: i64,
    /// The Routes standing at the close.
    routes: usize,
}

impl Driven {
    fn at(&self, name: &str) -> Option<Step> {
        self.milestones.iter().find(|held| held.name == name).map(|held| held.step)
    }

    fn offered_at(&self, id: &str) -> Option<Step> {
        self.offered.iter().find(|(held, _)| held == id).map(|(_, step)| *step)
    }

    /// The step the chapter closed at, measured from its own opening.
    fn span(&self) -> Step {
        self.end_step.expect("the chapter closes") - self.opening_step
    }

    fn took(&self, id: &str) -> bool {
        self.completed.iter().any(|held| held == id)
    }
}

fn record(driven: &mut Driven, events: Vec<(String, u32, Json)>) {
    for (name, step, body) in events {
        // Nothing before this chapter's own opening step is this chapter's.
        if step < driven.opening_step {
            continue;
        }
        match name.as_str() {
            "objective_changed" => {
                let objective = body.get("objective").expect("an objective");
                let id = objective.get("id").and_then(Json::as_text).unwrap_or("");
                let state = objective.get("state").and_then(Json::as_text).unwrap_or("");
                if !id.starts_with("objective.the_quiet_edge.") {
                    continue;
                }
                driven.states.push(state.to_string());
                if state == "active" && !driven.offered.iter().any(|(held, _)| held == id) {
                    driven.offered.push((id.to_string(), step));
                }
                if state == "complete" {
                    driven.milestones.push(Milestone { name: id.to_string(), step });
                    driven.completed.push(id.to_string());
                }
                if state == "failed_recoverable" {
                    driven.setbacks.push(step);
                }
                if state == "active" && !driven.setbacks.is_empty() {
                    driven.recovery_step.get_or_insert(step);
                }
            }
            "checkpoint_written" => {
                // The Anchor a transition writes stands on the opening step and
                // belongs to the boundary rather than to an authored moment.
                if step == driven.opening_step {
                    continue;
                }
                driven.anchors += 1;
                driven.milestones.push(Milestone { name: "anchor".to_string(), step });
            }
            "run_completed" => {
                driven.end_step = Some(step);
                driven.milestones.push(Milestone { name: "ending".to_string(), step });
                driven.ending_id =
                    body.get("ending_id").and_then(Json::as_text).map(str::to_string);
                driven.continuation_unlocked =
                    body.get("continuation_unlocked").and_then(Json::as_bool);
            }
            _ => {}
        }
    }
}

/// One run of the chapter, standing on the two-chapter manifest.
struct Run {
    session: Session,
    driver: Driver,
    driven: Driven,
    /// How much of the driver's own event log this run has already read. The
    /// driver drains the session on every frame it sends, so the events are
    /// there and not in the session.
    seen: usize,
    /// The Routes the required close names, read off the content once.
    supplies: Vec<u32>,
}

impl Run {
    fn impulse(&self) -> u8 {
        self.session.run().expect("a run").state().progress.impulse
    }

    fn step(&self) -> Step {
        self.session.run().expect("a run").step()
    }

    fn objective(&self) -> String {
        self.session.run().expect("a run").state().progress.objective.id.clone()
    }

    /// Charge held across a set of Nodes, in whole units.
    fn charge(&self, nodes: &[u32]) -> i64 {
        let state = self.session.run().expect("a run").state();
        state
            .now
            .ports
            .iter()
            .filter(|port| nodes.contains(&port.node))
            .map(|port| port.q / ONE_UNIT)
            .sum()
    }

    /// What the five arms of the pattern carried on the step just run, raw.
    fn carried(&self) -> i64 {
        let state = self.session.run().expect("a run").state();
        state
            .now
            .routes
            .iter()
            .filter(|route| self.supplies.contains(&route.route))
            .map(|route| route.flow)
            .sum()
    }

    fn relief_open(&self) -> bool {
        let state = self.session.run().expect("a run").state();
        state.now.ports.iter().any(|port| port.node == RELIEF && port.open)
    }

    fn ended(&self) -> bool {
        self.session.lifecycle() == "ended"
    }

    fn drain(&mut self) {
        let fresh: Vec<(String, u32, Json)> =
            self.driver.raised[self.seen..].iter().cloned().collect();
        self.seen = self.driver.raised.len();
        record(&mut self.driven, fresh);
    }

    fn commit(&mut self, plans: &[String]) {
        let borrowed: Vec<&str> = plans.iter().map(String::as_str).collect();
        let step = self.step();
        self.driver.commit_edit(&mut self.session, &borrowed);
        self.drain();
        let left = self.impulse();
        self.driven.commits.push((step, plans.len(), left));
    }

    /// Runs a number of steps, aiming as the act says, stopping at the ending.
    ///
    /// `neutral` counts the frames sent while the last objective stands, which
    /// is what the independence test is read off.
    fn run(&mut self, act: Act, steps: u32) {
        let mut left = steps;
        while left > 0 && !self.ended() {
            let now = left.min(u32::from(BATCH)) as u16;
            let form = controlled(&self.session);
            let (steer, held, release, depth) = match act {
                Act::Toward(x, y) => (toward(form, x, y), false, false, 0),
                Act::Follow(current) => {
                    let (x, y) = nearest_point(&self.session, current).unwrap_or((0, 0));
                    (toward(form, x, y), false, false, 0)
                }
                Act::Rest => ((0, 0), false, false, 0),
                Act::Charge(x, y) => (toward(form, x, y), true, false, 0),
                Act::Release => ((0, 0), false, true, 0),
                Act::Depth(way) => ((0, 0), false, false, way),
            };
            let standing = self.objective();
            self.driver.depth_frame(&mut self.session, now, steer, held, release, depth);
            self.drain();
            if standing == LET_IT_RUN {
                let neutral = steer == (0, 0) && !held && !release && depth == 0;
                assert!(neutral, "the last objective is driven with every input neutral");
                self.driven.neutral_frames += 1;
                let step = self.step();
                let ring = self.charge(&EVERY_NODE);
                let carried = self.carried();
                self.driven.samples.push(Sample { step, ring, carried });
            } else if standing == CARRY_THE_CLOSED_FIELD {
                let step = self.step();
                let ring = self.charge(&EVERY_NODE);
                let carried = self.carried();
                self.driven.samples.push(Sample { step, ring, carried });
            }
            if self.driven.relief_step.is_none() && self.relief_open() {
                self.driven.relief_step = Some(self.step());
            }
            // The pressure, read off the run rather than off an event: the
            // locked `pressure_changed` body carries the whole list, and what
            // this file needs is the stage the run stood in.
            for pressure in &self.session.run().expect("a run").state().pressures {
                let named = pressure.stage.name().to_string();
                let level = pressure.level;
                let last = self.driven.pressure_steps.last().map(|held| held.1.clone());
                if last.as_deref() != Some(named.as_str()) {
                    self.driven.pressure_steps.push((self.session.run().expect("a run").step(), named, level));
                }
            }
            left -= u32::from(now);
        }
    }

    /// The same, running until the sequence has moved past a named objective.
    fn until(&mut self, act: Act, id: &str, cap: u32) {
        let mut left = cap;
        while left > 0 && !self.ended() && self.objective() == id {
            self.run(act, u32::from(BATCH));
            left = left.saturating_sub(u32::from(BATCH));
        }
        assert!(
            self.ended() || self.objective() != id,
            "{} moved past {id} inside its cap; step={} charge={} carried={} first_sample={:?} last_sample={:?}",
            self.driven.form,
            self.step(),
            self.charge(&EVERY_NODE),
            self.carried(),
            self.driven.samples.first(),
            self.driven.samples.last(),
        );
    }
}

/// The route identifiers the five arms are carried by, read off the chapter.
fn chapter_supply_routes() -> Vec<u32> {
    let chapter = chapter();
    let objective = chapter.objective(CLOSE_THE_RING).expect("the required close");
    match &objective.condition {
        content::Condition::PatternHeld { routes, .. } => routes.clone(),
        _ => panic!("the close is a held pattern"),
    }
}

/// A fresh record for one driven run.
fn fresh_driven(form: &'static str, arrival: Arrival, opening_step: Step) -> Driven {
    Driven {
        form,
        arrival,
        milestones: Vec::new(),
        completed: Vec::new(),
        offered: Vec::new(),
        states: Vec::new(),
        setbacks: Vec::new(),
        recovery_step: None,
        opening_step,
        end_step: None,
        ending_id: None,
        continuation_unlocked: None,
        anchors: 0,
        commits: Vec::new(),
        relief_step: None,
        pressure_steps: Vec::new(),
        samples: Vec::new(),
        hands_off: None,
        neutral_frames: 0,
        ring_at_close: 0,
        carried_at_close: 0,
        routes: 0,
    }
}

/// Opens a run standing at the finale's opening, under one Form and one arrival.
fn at_the_finale(form: &'static str, arrival: Arrival) -> Run {
    let mut session = Session::new(&finale_init()).expect("versions agree");
    let opened = session.command(
        "init_run",
        &format!("{{\"form\":\"{form}\",\"mode\":\"new\",\"run_id\":\"{KEY}\"}}"),
    );
    assert!(opened.contains("\"ok\":true"), "{opened}");
    let mut driver = Driver::new();
    support::campaign::carry_on(&mut session, &mut driver, 8_000);
    assert_eq!(
        support::campaign::chapter_index(&session),
        1,
        "the fixture chapter completed and the finale opened",
    );
    // The step the chapter opened at is the step its own `chapter_changed`
    // was raised on, not the step the driver's last batch happened to end at:
    // the rest driver runs fifteen steps at a time and the transition lands
    // inside one of them.
    let opening_step = driver
        .raised
        .iter()
        .rev()
        .find(|(name, _, body)| {
            name == "chapter_changed"
                && body.get("chapter_index").and_then(Json::as_int) == Some(1)
        })
        .map(|(_, step, _)| *step)
        .expect("the run entered the chapter");
    assert_eq!(
        session.run().expect("a run").state().progress.impulse,
        6,
        "a crossing leaves the run carrying six",
    );
    let driven = fresh_driven(form, arrival, opening_step);
    let mut run = Run { session, driver, driven, seen: 0, supplies: chapter_supply_routes() };
    run.drain();
    for _ in 0..arrival.spend() {
        run.commit(&[no_op_reshape()]);
    }
    assert_eq!(run.impulse(), arrival.impulse(), "the arrival {}", arrival.name());
    // The instrument changed neither independent object: physical membership
    // and the Observation View both remain at their authored starting data.
    let state = run.session.run().expect("a run").state();
    assert_eq!(
        state.now.physical_compartment.members,
        EVERY_NODE.to_vec(),
        "a no-op compartment reshape leaves physical membership unchanged",
    );
    assert_eq!(state.view.inside, EVERY_NODE.to_vec(), "and cannot replace the View");
    run
}

/// The modelled allowances of a first run, each one a step count this file
/// chose rather than the content did.
///
/// The floor run skips them; the attentive run spends them, and the difference
/// between the two is the whole of the pacing model. Each is named so a reader
/// can judge it, and **each one really does lengthen the chapter** — this
/// chapter's objectives are hold counts that run whether the Form is doing
/// anything or not, so an allowance spent beside a pattern that is already
/// holding costs nothing at all. The three that stand are the three that tell:
/// the opening delay pushes the whole sequence back, and both deep-band
/// allowances are steps spent outside a band whose count only runs inside it.
const ALLOWANCES: [(&str, u32); 3] = [
    ("a first look at the ring before drawing anything", 600),
    ("finding the way down to the deep band", 3_000),
    ("drifting out of the deep band and back", 2_400),
];

/// Drives the chapter under one Form, from one arrival, with or without the
/// modelled allowances of a first run.
fn drive_as(form: &'static str, arrival: Arrival, allowances: bool) -> Driven {
    let mut run = at_the_finale(form, arrival);
    play_finale(&mut run, arrival, allowances);
    run.driven
}

/// Plays the chapter from its opening to the campaign's ending, over a run that
/// is already standing in it.
fn play_finale(run: &mut Run, arrival: Arrival, allowances: bool) {
    let form = run.driven.form;
    let spend = |run: &mut Run, place: usize| {
        if allowances {
            run.run(Act::Follow(1), ALLOWANCES[place].1);
        }
    };

    // The opening, which is the whole of the strategic beat: what this arrival
    // can afford before anything here completes.
    spend(run, 0);
    let mut plans = arm_outfalls();
    if arrival == Arrival::Six {
        plans.insert(0, spare_supply());
    }
    let affordable = usize::from(run.impulse()).min(plans.len());
    let taken: Vec<String> = plans.into_iter().take(affordable).collect();
    run.commit(&taken);
    assert_eq!(run.impulse(), 0, "the opening spends the whole arrival");

    if arrival == Arrival::Four {
        // The slower opening: the four arms it could afford are drawn, and the
        // fifth carries through the relief Port on the layer below, which costs
        // no Impulse and the time it takes to get there and back.
        run.run(Act::Toward(RELIEF_AT.0, RELIEF_AT.1), 4_000);
        run.run(Act::Depth(1), 1);
        run.run(Act::Toward(RELIEF_AT.0, RELIEF_AT.1), 300);
        run.run(Act::Charge(RELIEF_AT.0, RELIEF_AT.1), FULL_CHARGE_STEPS);
        run.run(Act::Release, 1);
        assert!(run.relief_open(), "{form} opened the relief Port");
        run.run(Act::Depth(-1), 1);
        run.run(Act::Toward(BAND_AT.0, BAND_AT.1), 3_400);
    }

    run.until(Act::Follow(1), CLOSE_THE_SPARE_ARM, 12_000);
    run.until(Act::Follow(1), CLOSE_THE_RING_AT_ONCE, 12_000);
    run.until(Act::Follow(1), CLOSE_THE_RING, 30_000);

    // The deep band. An attentive run spends longer finding the way down and
    // longer out of the band once it is there, and both of those genuinely
    // lengthen the chapter: `in_current` counts only the steps spent inside.
    if allowances {
        run.run(Act::Toward(RELIEF_AT.0, RELIEF_AT.1), ALLOWANCES[1].1);
    }
    run.run(Act::Toward(RELIEF_AT.0, RELIEF_AT.1), 3_600);
    run.run(Act::Depth(1), 1);
    if allowances {
        // Out of the band and back, which the count really does feel: `in_current`
        // is cumulative but only over the steps spent *inside*. Standing still at
        // the descent point would not be an allowance at all — that point is a
        // path point of the deep band, so a resting Form there keeps counting.
        run.run(Act::Toward(RELIEF_AT.0, RELIEF_AT.1 - 400), ALLOWANCES[2].1);
    }
    run.until(Act::Follow(2), TAKE_THE_DEEP_BAND, 60_000);
    run.run(Act::Depth(-1), 1);
    run.run(Act::Toward(BAND_AT.0, BAND_AT.1), 3_000);

    run.until(Act::Follow(1), HOLD_PAST_THE_DEEP_STOP, 50_000);
    run.until(Act::Follow(1), CARRY_THE_CLOSED_FIELD, 50_000);

    // The independence test: every input neutral for the whole of it.
    run.until(Act::Rest, LET_IT_RUN, 60_000);
    assert!(run.ended(), "{form} reached the campaign's ending");
    run.drain();
    let offered = run.driven.offered_at(LET_IT_RUN).expect("the last objective was offered");
    let ended = run.driven.end_step.expect("the campaign ended");
    run.driven.hands_off = Some((offered, ended));
    run.driven.ring_at_close = run.charge(&EVERY_NODE);
    run.driven.carried_at_close = run.carried();
    run.driven.routes = run.session.run().expect("a run").state().now.routes.len();
}

fn floor(arrival: Arrival) -> Driven {
    drive_as("thread", arrival, false)
}

fn attentive(arrival: Arrival) -> Driven {
    drive_as("thread", arrival, true)
}

// ---------------------------------------------------------------------------
// The chapter, completed
// ---------------------------------------------------------------------------

#[test]
fn the_chapter_completes_and_closes_the_campaign_on_its_own_ending() {
    let run = attentive(Arrival::Six);

    println!(
        "\nThe Quiet Edge, attentive first run from the six-Impulse arrival — milestones \
         (steps from the chapter's opening)"
    );
    println!("{:<56} {:>8} {:>8}", "milestone", "step", "minutes");
    for milestone in &run.milestones {
        let at = milestone.step - run.opening_step;
        println!("{:<56} {at:>8} {:>8}", milestone.name, minutes(at));
    }

    let chapter = chapter();
    let required: Vec<&str> = chapter
        .objectives
        .iter()
        .filter(|held| held.optional.is_none())
        .map(|held| held.id.as_str())
        .collect();
    for id in &required {
        assert!(run.took(id), "{id} was completed");
    }
    assert!(run.end_step.is_some(), "the campaign ended on this chapter");
    assert_eq!(run.anchors, chapter.anchor_moments.len(), "one Anchor per authored moment");
    assert_eq!(
        run.ending_id.as_deref(),
        Some("ending.the_quiet_edge.thread.spare"),
        "the ending the Form and the choice resolve to",
    );
    // A truncated campaign reaches its own ending and unlocks nothing.
    assert_eq!(run.continuation_unlocked, Some(false));
}

#[test]
fn the_chapter_asks_for_about_seventy_minutes_and_the_floor_stands_under_it() {
    let mut rows: Vec<(String, Step)> = Vec::new();
    for arrival in ARRIVALS {
        let floor = floor(arrival);
        let attentive = attentive(arrival);
        rows.push((format!("{} floor", arrival.name()), floor.span()));
        rows.push((format!("{} attentive", arrival.name()), attentive.span()));
    }
    println!("\nThe Quiet Edge — how long the chapter asks for");
    println!("{:<28} {:>9} {:>9}", "path", "steps", "minutes");
    for (name, span) in &rows {
        println!("{name:<28} {span:>9} {:>9}", minutes(*span));
    }
    let longest = rows.iter().map(|(_, span)| *span).max().expect("a row");
    let shortest = rows.iter().map(|(_, span)| *span).min().expect("a row");
    // Seventy minutes is 126,000 steps at the locked thirty a second. The floor
    // stands under it and the attentive run is near it; the band is stated
    // rather than pinned to a step.
    assert!(shortest > 90_000, "the floor is a real chapter: {shortest}");
    assert!(shortest < 126_000, "and it stands under seventy minutes: {shortest}");
    assert!(longest > 108_000, "the attentive run is past an hour: {longest}");
    assert!(longest < 145_000, "and inside eighty minutes: {longest}");
}

// ---------------------------------------------------------------------------
// The gate: five queued entries, before anything grants
// ---------------------------------------------------------------------------

#[test]
fn five_queued_entries_is_the_demand_and_it_binds_before_the_first_grant() {
    // The demand: the required close names five Routes out of five arms, and
    // each of them is a Route the chapter does not place. Read off the content
    // rather than written here.
    let chapter = chapter();
    let close = chapter.objective(CLOSE_THE_RING).expect("the required close");
    let content::Condition::PatternHeld { nodes, .. } = &close.condition else {
        panic!("the close is a held pattern");
    };
    assert_eq!(nodes, &ARMS.to_vec(), "five arms, and every one of them needs a way out");
    // Four of the five are placed with nothing at all carrying out of them, so
    // the only way to give them one is a Route drawn. The fifth has one placed
    // Route out of it, to the relief Port on the layer below — and that Port is
    // established closed, so it carries nothing until a Pulse opens it.
    let placed_out = |arm: u32| -> Vec<u32> {
        chapter.routes.iter().filter(|route| route.tail == arm).map(|route| route.head).collect()
    };
    for arm in [3, 4, 5, 6] {
        assert!(placed_out(arm).is_empty(), "arm {arm} has nothing carrying out of it");
    }
    assert_eq!(placed_out(7), vec![RELIEF], "the fifth arm's one placed way out");
    let relief = chapter
        .ports
        .iter()
        .find(|port| port.node == RELIEF)
        .expect("the relief Port is placed");
    assert_eq!(relief.kind, field_game_core::field::NodeKind::Port, "and it is a Port");
    assert_eq!(relief.layer, 1, "standing on the layer below");

    // The gate, driven: at each arrival, how many of the six entries the opening
    // asks for can be committed before anything here has completed.
    println!("\nThe Quiet Edge — the opening, before any completion grants anything");
    println!("{:<20} {:>8} {:>10} {:>12} {:>12}", "arrival", "Impulse", "afforded", "spare arm", "ring at once");
    let mut afforded = Vec::new();
    for arrival in ARRIVALS {
        let run = floor(arrival);
        // Nothing had completed when the opening was committed.
        let (step, entries, _left) = run.commits[usize::from(arrival.spend())];
        assert!(
            run.milestones.iter().all(|held| held.name == "anchor" || held.step > step),
            "the opening is committed before anything completes",
        );
        afforded.push(entries);
        println!(
            "{:<20} {:>8} {:>10} {:>12} {:>12}",
            arrival.name(),
            arrival.impulse(),
            entries,
            run.took(CLOSE_THE_SPARE_ARM),
            run.took(CLOSE_THE_RING_AT_ONCE),
        );
        match arrival {
            Arrival::Six => {
                assert!(run.took(CLOSE_THE_SPARE_ARM), "six affords the spare arm as well");
                assert!(run.took(CLOSE_THE_RING_AT_ONCE));
            }
            Arrival::Five => {
                assert!(!run.took(CLOSE_THE_SPARE_ARM), "five cannot afford a sixth entry");
                assert!(run.took(CLOSE_THE_RING_AT_ONCE), "five affords the ring exactly");
            }
            Arrival::Four => {
                assert!(!run.took(CLOSE_THE_SPARE_ARM));
                assert!(
                    !run.took(CLOSE_THE_RING_AT_ONCE),
                    "four cannot draw five Routes and cannot reach the relief in time",
                );
            }
        }
    }
    assert_eq!(afforded, vec![6, 5, 4], "the three arrivals afford three different openings");
}

#[test]
fn the_wholesale_arrival_has_an_opening_and_it_is_slower() {
    // The four-Impulse arrival's honest alternative, measured: it draws the four
    // arms it can afford and opens the relief Port on the layer below, which
    // costs no Impulse and the time it takes to get there.
    let run = floor(Arrival::Four);
    let opened = run.relief_step.expect("the relief Port was opened") - run.opening_step;
    let closed = run.at(CLOSE_THE_RING).expect("the ring closed") - run.opening_step;

    // The five-Impulse arrival never needs it.
    let quick = floor(Arrival::Five);
    assert!(quick.relief_step.is_none(), "five Impulse never opens the relief");
    let quick_closed = quick.at(CLOSE_THE_RING).expect("the ring closed") - quick.opening_step;

    println!("\nThe Quiet Edge — the two openings");
    println!("  four Impulse: relief opened at {opened}, ring closed at {closed}");
    println!("  five Impulse: ring closed at {quick_closed}");
    assert!(
        closed > quick_closed,
        "the slower opening is slower: {closed} against {quick_closed}",
    );

    // And the reason it cannot take the test the five-Impulse arrival takes: the
    // relief is not open until well past the step the test would have to be
    // held from. The margin is stated rather than assumed.
    let chapter = chapter();
    let test = chapter.objective(CLOSE_THE_RING_AT_ONCE).expect("the test");
    let span = test.optional.expect("an optional test");
    let held = content::effective_steps(&test.condition) as Step;
    let earlier = chapter.objective(CLOSE_THE_SPARE_ARM).expect("the first test");
    let earlier_span = earlier.optional.expect("an optional test");
    let latest = earlier_span + span - held;
    println!(
        "  the test could be taken only by a run holding the ring from step {latest}; \
         the relief opens at {opened}, {} steps later",
        opened - latest,
    );
    assert!(
        opened > latest + PRESSURE_MARGIN,
        "and the margin is real: {opened} against {latest}",
    );
}

// ---------------------------------------------------------------------------
// Eight Forms, three arrivals, twenty-four endings
// ---------------------------------------------------------------------------

#[test]
fn every_starting_form_reaches_a_complete_ending_from_every_arrival() {
    println!("\nThe Quiet Edge — eight Forms, three arrivals, twenty-four driven endings");
    println!(
        "{:<8} {:<18} {:>9} {:>8} {:>7} {:>8}  {}",
        "form", "arrival", "steps", "minutes", "ring", "carried", "ending"
    );
    let mut reached: Vec<String> = Vec::new();
    for form in FORMS {
        for arrival in ARRIVALS {
            let run = drive_as(form, arrival, false);
            let ending = run.ending_id.clone().expect("an ending");
            println!(
                "{form:<8} {:<18} {:>9} {:>8} {:>7} {:>8}  {ending}",
                arrival.name(),
                run.span(),
                minutes(run.span()),
                run.ring_at_close,
                run.carried_at_close / ONE_UNIT,
                ending = ending,
            );
            assert_eq!(
                ending,
                format!("ending.the_quiet_edge.{form}.{}", arrival.mark()),
                "the ending names this Form and this choice",
            );
            let entry =
                catalog_entry(&ending).unwrap_or_else(|| panic!("{ending} is authored"));
            assert!(entry.contains("\"kind\": \"ending\""), "{ending} is of the ending kind");
            assert!(run.ring_at_close > 0, "{form} closed with the pattern still standing");
            reached.push(ending);
        }
    }
    assert_eq!(reached.len(), 24, "eight Forms times three continuity choices");
    let mut distinct = reached.clone();
    distinct.sort();
    distinct.dedup();
    assert_eq!(distinct.len(), 24, "and every one of them a different ending");

    // Distinct copy, not just distinct keys.
    let mut wording: Vec<&str> =
        reached.iter().map(|key| catalog_entry(key).expect("wording")).collect();
    wording.sort();
    wording.dedup();
    assert_eq!(wording.len(), 24, "twenty-four endings, twenty-four sentences");
}

// ---------------------------------------------------------------------------
// The three rewrite paths, played through the whole campaign
// ---------------------------------------------------------------------------

#[test]
fn the_three_rewrite_paths_carry_three_arrivals_into_the_finale() {
    // The whole shipped campaign, three times, differing only in how much the
    // run spends before the finale's first completion. What this proves is the
    // two things the finale stands on: the arrivals really are 6, 5 and 4, and
    // every one of them reaches a complete ending — the chapter's own done-when.
    //
    // The Rewrite's own three paths are driven in
    // `core/tests/chapter_the_rewrite.rs`, which pins the ladder `[5, 4, 6]`
    // there. The shared campaign arm takes the relocation, so a run played by it
    // arrives carrying six; the other two arrivals are reached by the same
    // instrument the rest of this file uses — a no-op compartment reshape, spent
    // at the finale's opening, before anything here completes.
    let content = authored();
    let mut arrivals = Vec::new();
    let mut endings = Vec::new();
    for arrival in ARRIVALS {
        let mut session = Session::new(&support::worker_init()).expect("versions agree");
        let opened = session.command(
            "init_run",
            &format!("{{\"form\":\"thread\",\"mode\":\"new\",\"run_id\":\"{KEY}\"}}"),
        );
        assert!(opened.contains("\"ok\":true"), "{opened}");
        let mut driver = Driver::new();
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
            "the run stands in the finale",
        );
        assert_eq!(
            session.run().expect("a run").state().progress.impulse,
            6,
            "the shared arm takes The Rewrite's relocation, which carries six",
        );
        let opening_step = driver
            .raised
            .iter()
            .rev()
            .find(|(name, _, body)| {
                name == "chapter_changed"
                    && body.get("chapter_index").and_then(Json::as_int)
                        == Some(i64::from(CHAPTER))
            })
            .map(|(_, step, _)| *step)
            .expect("the run entered the chapter");
        let driven = fresh_driven("thread", arrival, opening_step);
        let mut run = Run { session, driver, driven, seen: 0, supplies: chapter_supply_routes() };
        run.drain();
        for _ in 0..arrival.spend() {
            run.commit(&[no_op_reshape()]);
        }
        assert_eq!(run.impulse(), arrival.impulse());
        play_finale(&mut run, arrival, false);
        assert_eq!(run.session.lifecycle(), "ended", "{} reached the ending", arrival.name());
        assert_eq!(
            run.driven.continuation_unlocked,
            Some(true),
            "the whole closed set was authored, so the continuation unlocks",
        );
        arrivals.push(arrival.impulse());
        endings.push(run.driven.ending_id.clone().expect("an ending"));
    }
    println!("\nThe Quiet Edge — three continuity arrivals, played through the whole campaign");
    for (place, ending) in endings.iter().enumerate() {
        println!("  arrived carrying {}, ended on {ending}", arrivals[place]);
    }
    assert_eq!(arrivals, vec![6, 5, 4], "the three arrivals the finale reads");
    assert_eq!(
        endings,
        vec![
            "ending.the_quiet_edge.thread.spare".to_string(),
            "ending.the_quiet_edge.thread.whole".to_string(),
            "ending.the_quiet_edge.thread.steady".to_string(),
        ],
        "and three different endings, reached by playing to the close",
    );
}

// ---------------------------------------------------------------------------
// The closed field, and the declining support
// ---------------------------------------------------------------------------

#[test]
fn the_field_closes_and_the_support_declines_one_objective_at_a_time() {
    let chapter = chapter();
    // Every authored event stands for the whole of an objective's phase: the
    // `at: 1` idiom, which nothing but a zero-step completion can outrun.
    for event in &chapter.events {
        assert_eq!(event.at, 1, "{} stands for its whole phase", event.objective);
    }
    // The arc: two currents stopped and the layer's loss raised twice, in
    // authored order, each against a later objective than the one before it.
    let order: Vec<usize> = chapter
        .events
        .iter()
        .map(|event| {
            chapter
                .objectives
                .iter()
                .position(|held| held.id == event.objective)
                .expect("an objective of this chapter")
        })
        .collect();
    assert!(
        order.windows(2).all(|pair| pair[0] <= pair[1]),
        "the arc runs one way through the chapter: {order:?}",
    );
    println!("\nThe Quiet Edge — the declining support, as authored");
    for event in &chapter.events {
        println!("  {} at {}: {:?}", event.objective.rsplit('.').next().unwrap_or(""), event.at, event.effect);
    }

    // Nothing outside the pattern delivers by the last two objectives, and the
    // ring holds anyway. Measured on all three arrivals.
    println!("\n{:<20} {:>10} {:>10} {:>10} {:>9}", "arrival", "ring open", "ring close", "spent", "carried");
    for arrival in ARRIVALS {
        let run = floor(arrival);
        let (offered, _) = run.hands_off.expect("the last objective stood");
        let opening = run
            .samples
            .first()
            .copied()
            .expect("the closed field was sampled");
        let closing = run.samples.last().copied().expect("a sample");
        println!(
            "{:<20} {:>10} {:>10} {:>10} {:>9}",
            arrival.name(),
            opening.ring,
            closing.ring,
            opening.ring - closing.ring,
            run.carried_at_close / ONE_UNIT,
        );
        assert!(closing.ring > 0, "the pattern is still standing at the close");
        assert!(
            opening.ring > closing.ring,
            "and the closed field spends what it stands on",
        );
        assert!(offered > run.opening_step, "the last objective is the last of them");
    }
}

#[test]
fn the_pattern_continues_without_direct_control() {
    // The campaign's own thesis, driven: the last objective is held for its
    // whole authored span with every input neutral.
    let chapter = chapter();
    let last = chapter.objectives.last().expect("an objective");
    assert_eq!(last.id, LET_IT_RUN, "the chapter ends on the hands-off span");
    assert!(last.optional.is_none(), "and it is required");
    let asked = content::effective_steps(&last.condition) as Step;

    println!("\nThe Quiet Edge — the independence test");
    println!(
        "{:<20} {:>10} {:>10} {:>8} {:>10} {:>10}",
        "arrival", "offered", "completed", "steps", "frames", "ring left"
    );
    for arrival in ARRIVALS {
        let run = floor(arrival);
        let (offered, ended) = run.hands_off.expect("the last objective stood");
        let span = ended - offered;
        println!(
            "{:<20} {:>10} {:>10} {:>8} {:>10} {:>10}",
            arrival.name(),
            offered - run.opening_step,
            ended - run.opening_step,
            span,
            run.neutral_frames,
            run.ring_at_close,
        );
        // Every frame of the span was neutral — the driver asserts it as it
        // sends them — and there were enough of them to cover the whole span.
        assert!(run.neutral_frames > 0, "the span was driven");
        assert!(
            run.neutral_frames >= (span / u32::from(BATCH)) as usize,
            "every frame of the span is one of the neutral ones: {} against {span}",
            run.neutral_frames,
        );
        // The count opens on the step after the offer, so the span from the
        // offer to the completion is the authored target less that one step.
        assert_eq!(
            span + 1,
            asked,
            "the pattern held for the whole authored span: {span} against {asked}",
        );
        assert!(run.ring_at_close > 0, "and the ring is still carrying at the close");
    }
}

// ---------------------------------------------------------------------------
// The pressure: it seats, and it has teeth
// ---------------------------------------------------------------------------

#[test]
fn the_pressure_lands_inside_the_objective_written_for_it() {
    // `pressure_schedule.start_step` is chapter-absolute; an objective is
    // offered when a player reaches it. The paths are enumerated rather than
    // assumed: floor and attentive, from each of the three arrivals, plus the
    // shape the shared campaign arm drives — a six-Impulse run that lets both
    // opening tests go, which is the slowest way through the opening.
    let chapter = chapter();
    let entry = chapter.pressure_schedule.first().expect("one pressure");
    let table = authored().pressures.table(entry.pressure).expect("a table").clone();
    let life: Step = table.stages.iter().map(|stage| stage.steps as Step).sum();

    let mut window: Vec<(String, Step, Step)> = Vec::new();
    for arrival in ARRIVALS {
        for (label, allowances) in [("floor", false), ("attentive", true)] {
            let run = drive_as("thread", arrival, allowances);
            let (offered, ended) = run.hands_off.expect("the last objective stood");
            window.push((
                format!("{} {label}", arrival.name()),
                offered - run.opening_step,
                ended - run.opening_step,
            ));
        }
    }

    let latest = window.iter().map(|(_, offered, _)| *offered).max().expect("a path");
    let earliest = window.iter().map(|(_, _, ended)| *ended).min().expect("a path");
    println!("\nThe Quiet Edge — the window Noise has to land inside, on six paths");
    println!("{:<28} {:>10} {:>10}", "path", "offered", "completed");
    for (name, offered, ended) in &window {
        println!("{name:<28} {offered:>10} {ended:>10}");
    }
    println!(
        "  window [{latest}, {earliest}], {} steps wide; the pressure occupies \
         [{}, {}] and lives {life} steps",
        earliest - latest,
        entry.start_step,
        entry.start_step + life,
    );
    assert!(latest + PRESSURE_MARGIN <= entry.start_step, "the near margin");
    assert!(
        entry.start_step + life + PRESSURE_MARGIN <= earliest,
        "the far margin",
    );
    println!(
        "  margins: {} steps at the near end, {} at the far",
        entry.start_step - latest,
        earliest - entry.start_step - life,
    );
}

#[test]
fn noise_seats_itself_and_narrows_what_the_ring_carries() {
    let run = floor(Arrival::Five);
    // It seats: the pressure is pressed, and its stages were read.
    let stages: Vec<&str> = run.pressure_steps.iter().map(|(_, stage, _)| stage.as_str()).collect();
    println!("\nThe Quiet Edge — Noise, as the run met it");
    for (step, stage, level) in &run.pressure_steps {
        println!("  {} {stage} level {level}", step - run.opening_step);
    }
    assert!(!stages.is_empty(), "the pressure seated and staged");
    assert!(stages.contains(&"crisis"), "and reached its crisis");

    // It has teeth: what the five arms carry per step is measurably narrower
    // while the pressure stands than it is before the pressure seats.
    let entry = chapter().pressure_schedule.first().expect("one pressure").clone();
    let seated = run.opening_step + entry.start_step;
    let table = authored().pressures.table(entry.pressure).expect("a table").clone();
    let life: Step = table.stages.iter().map(|stage| stage.steps as Step).sum();
    let mean = |samples: &[Sample]| -> i64 {
        if samples.is_empty() {
            return 0;
        }
        samples.iter().map(|held| held.carried).sum::<i64>() / samples.len() as i64
    };
    let before: Vec<Sample> =
        run.samples.iter().copied().filter(|held| held.step < seated).collect();
    let during: Vec<Sample> = run
        .samples
        .iter()
        .copied()
        .filter(|held| held.step >= seated && held.step < seated + life)
        .collect();
    let after: Vec<Sample> =
        run.samples.iter().copied().filter(|held| held.step >= seated + life).collect();
    let (quiet, pressed, released) = (mean(&before), mean(&during), mean(&after));
    println!(
        "  the five arms carried {} raw a step before it, {} while it stood, {} after",
        quiet, pressed, released,
    );
    assert!(!before.is_empty() && !during.is_empty(), "both sides were sampled");
    assert!(pressed < quiet, "Noise narrowed what the ring carries");
    let narrowing = (quiet - pressed) * 100 / quiet.max(1);
    println!("  a narrowing of {narrowing}%");
    assert!(narrowing >= 10, "and the narrowing is a real one: {narrowing}%");
    assert!(released > pressed, "and the ring carries more again once it passes");
}

// ---------------------------------------------------------------------------
// The authored shapes
// ---------------------------------------------------------------------------

#[test]
fn the_authored_setback_stands_and_the_chapter_carries_past_it() {
    // An arm with nowhere to send what arrives stands over its own threshold
    // inside about sixty steps, and `pattern_held` reads that as a recoverable
    // setback. A run that closes the ring on its opening step never meets it —
    // which is the point of closing it — so the runs read here are the
    // attentive ones, which look at the field for six hundred steps first.
    for arrival in ARRIVALS {
        let run = attentive(arrival);
        assert!(!run.setbacks.is_empty(), "{} met the setback", arrival.name());
        assert!(run.recovery_step.is_some(), "and carried past it");
        assert!(
            !run.states.iter().any(|state| state == "failed"),
            "nothing in this chapter fails terminally",
        );
        println!(
            "{}: {} setbacks, recovered at {}",
            arrival.name(),
            run.setbacks.len(),
            run.recovery_step.expect("a recovery") - run.opening_step,
        );
    }
}

#[test]
fn the_optional_tests_are_passable_and_skippable() {
    let chapter = chapter();
    let tests: Vec<&content::Objective> =
        chapter.objectives.iter().filter(|held| held.optional.is_some()).collect();
    assert_eq!(tests.len(), 2, "the chapter authors two tests, and they are its first two");
    assert_eq!(tests[0].id, CLOSE_THE_SPARE_ARM);
    assert_eq!(tests[1].id, CLOSE_THE_RING_AT_ONCE);
    assert_eq!(chapter.objectives[0].id, CLOSE_THE_SPARE_ARM, "offered first");
    assert_eq!(chapter.objectives[1].id, CLOSE_THE_RING_AT_ONCE, "and second");
    assert!(
        chapter.objectives.last().expect("an objective").optional.is_none(),
        "and neither of them is last, so neither closes the chapter",
    );

    // Both ways, driven: taken by the arrival that affords them, let go by the
    // one that does not.
    let took = floor(Arrival::Six);
    let let_go = floor(Arrival::Four);
    assert!(took.took(CLOSE_THE_SPARE_ARM) && took.took(CLOSE_THE_RING_AT_ONCE));
    assert!(!let_go.took(CLOSE_THE_SPARE_ARM) && !let_go.took(CLOSE_THE_RING_AT_ONCE));
    // A test let go is passed over at its own span, and the objective after it
    // stands.
    for (place, test) in tests.iter().enumerate() {
        let offered = let_go.offered_at(&test.id).expect("it was offered");
        let next = let_go
            .offered_at(&chapter.objectives[place + 1].id)
            .expect("the one after it stood");
        assert_eq!(
            next - offered,
            test.optional.expect("a span"),
            "{} was passed over at its own span",
            test.id,
        );
    }
}

#[test]
fn the_authored_conditions_span_the_step_counts_they_name() {
    // Progress is a `Frac`, so a target of N spans `ceil(65536 / ceil(65536 / N))`.
    // Every count this chapter authors divides exactly.
    for objective in &chapter().objectives {
        let asked = objective.condition.target();
        let spans = content::effective_steps(&objective.condition);
        assert_eq!(spans, asked, "{} spans exactly what it names", objective.id);
    }
}

#[test]
fn the_chapter_offers_one_objective_at_a_time_and_none_of_them_fails_terminally() {
    let run = floor(Arrival::Six);
    let chapter = chapter();
    // The offers came in the authored order, and no objective was offered twice.
    let order: Vec<&str> = run.offered.iter().map(|(id, _)| id.as_str()).collect();
    let authored_order: Vec<&str> =
        chapter.objectives.iter().map(|held| held.id.as_str()).collect();
    assert_eq!(order, authored_order, "one at a time, in the authored order");
    assert!(!run.states.iter().any(|state| state == "failed"));
}

#[test]
fn the_shared_campaign_script_carries_the_chapter_to_its_own_boundary() {
    // The dispatch's own arm, over the shipped campaign, ending the run.
    let content = authored();
    let mut session = Session::new(&support::worker_init()).expect("versions agree");
    let opened = session.command(
        "init_run",
        &format!("{{\"form\":\"vault\",\"mode\":\"new\",\"run_id\":\"{KEY}\"}}"),
    );
    assert!(opened.contains("\"ok\":true"), "{opened}");
    let mut driver = Driver::new();
    for index in 0..content.chapters.len() {
        support::campaign::play_chapter(
            &mut session,
            &mut driver,
            content.chapter(index as u8).expect("a chapter"),
        );
    }
    assert_eq!(session.lifecycle(), "ended");
    let completed = driver.of("run_completed");
    let ending = completed
        .last()
        .and_then(|(_, _, body)| body.get("ending_id").and_then(Json::as_text))
        .expect("an ending");
    println!("\nthe shared arm, under Vault: {ending}");
    assert_eq!(ending, "ending.the_quiet_edge.vault.steady");
    // And the arm let both tests go, which is what keeps a driven campaign's
    // completed list the campaign's required objectives exactly.
    let done = session.run().expect("a run").state().progress.complete.clone();
    for objective in &content.chapter(CHAPTER).expect("the finale").objectives {
        assert_eq!(
            done.iter().any(|held| held == &objective.id),
            objective.optional.is_none(),
            "{}",
            objective.id,
        );
    }
}

#[test]
fn the_chapter_reads_as_it_is_authored() {
    let chapter = chapter();
    assert_eq!(chapter.id, "the_quiet_edge");
    assert_eq!(chapter.ending_key, "ending.the_quiet_edge");
    // Three marks, most specific first, with the catch-all last.
    let marks: Vec<&str> = chapter.ending_marks.iter().map(|held| held.mark.as_str()).collect();
    assert_eq!(marks, vec!["spare", "whole", "steady"]);
    assert!(chapter.ending_marks.last().expect("a mark").objective.is_none());
    // The resolution is a pure function of the Form and the completed list, and
    // this is the whole of the grid it produces.
    for form in FORMS {
        assert_eq!(
            chapter.ending_id(form, &[CLOSE_THE_SPARE_ARM.to_string()]),
            format!("ending.the_quiet_edge.{form}.spare"),
        );
        assert_eq!(
            chapter.ending_id(form, &[CLOSE_THE_RING_AT_ONCE.to_string()]),
            format!("ending.the_quiet_edge.{form}.whole"),
        );
        assert_eq!(
            chapter.ending_id(form, &[]),
            format!("ending.the_quiet_edge.{form}.steady"),
        );
        // A run that took both reads the first mark, which is the more specific.
        assert_eq!(
            chapter.ending_id(
                form,
                &[CLOSE_THE_RING_AT_ONCE.to_string(), CLOSE_THE_SPARE_ARM.to_string()],
            ),
            format!("ending.the_quiet_edge.{form}.spare"),
        );
    }
    let _ = CONTENT_VERSION;
}

// ---------------------------------------------------------------------------
// What the amendment refuses
// ---------------------------------------------------------------------------

/// The bundle with this chapter's own bytes edited, and the digest recomputed
/// over exactly what the edit leaves.
///
/// Every refusal below edits **this chapter's** file and no other, which is
/// what the decoupling rule asks for: no literal another chapter's author may
/// not move is reached for.
fn refused(from: &str, to: &str) -> String {
    let edited = support::CHAPTER_THE_QUIET_EDGE.replace(from, to);
    assert_ne!(edited, support::CHAPTER_THE_QUIET_EDGE, "the edit reached something: {from}");
    let mut listed: Vec<String> =
        support::files().iter().map(|file| file.to_string()).collect();
    listed[usize::from(CHAPTER)] = edited;
    let bundle = support::bundle_of(support::MANIFEST, &listed);
    let parsed = field_game_core::json::parse(&bundle).expect("canonical");
    let fault = content::read_bundle(&parsed).expect_err("the edited chapter is refused");
    assert_eq!(fault.code(), field_game_core::fault::Code::ContentInvalid);
    fault.detail().expect("a named diagnostic").to_string()
}

#[test]
fn a_mark_that_is_not_a_key_segment_is_refused() {
    let detail = refused("\"mark\": \"spare\"", "\"mark\": \"Spare\"");
    assert_eq!(detail, "{\"reason\":\"mark\"}");
}

#[test]
fn two_marks_the_same_are_refused() {
    let detail = refused("\"mark\": \"whole\"", "\"mark\": \"spare\"");
    assert!(detail.contains("\"reason\":\"mark\""), "{detail}");
}

#[test]
fn a_mark_naming_an_objective_this_chapter_never_authors_is_refused() {
    let detail = refused(
        "\"objective\": \"objective.the_quiet_edge.close_the_spare_arm\"",
        "\"objective\": \"objective.the_quiet_edge.close_the_spare\"",
    );
    assert!(detail.contains("\"reason\":\"objective\""), "{detail}");
}

#[test]
fn a_mark_list_with_no_catch_all_is_refused() {
    // Without one, a run that completed none of the named objectives would
    // reach no mark at all.
    let detail = refused(
        "{\n      \"mark\": \"steady\"\n    }",
        "{\n      \"mark\": \"steady\",\n      \"objective\": \
         \"objective.the_quiet_edge.close_the_ring\"\n    }",
    );
    assert!(detail.contains("\"reason\":\"ending_marks\""), "{detail}");
}

#[test]
fn a_catch_all_that_is_not_last_is_refused() {
    // Everything below it would be unreachable.
    let detail = refused(
        "{\n      \"mark\": \"spare\",\n      \"objective\": \
         \"objective.the_quiet_edge.close_the_spare_arm\"\n    }",
        "{\n      \"mark\": \"spare\"\n    }",
    );
    assert!(detail.contains("\"reason\":\"ending_marks\""), "{detail}");
}

#[test]
fn a_chapter_that_authors_no_marks_closes_on_its_key_as_it_stands() {
    // The defaulted key, read as the amendment declares it: the shipped
    // chapters before this one author none, and every one of them answers its
    // own `ending_key`.
    let content = authored();
    for index in 0..CHAPTER {
        let chapter = content.chapter(index).expect("a chapter");
        assert!(chapter.ending_marks.is_empty(), "{} authors no marks", chapter.id);
        assert_eq!(
            chapter.ending_id("thread", &[]),
            chapter.ending_key,
            "{} closes on its key as it stands",
            chapter.id,
        );
    }
}
