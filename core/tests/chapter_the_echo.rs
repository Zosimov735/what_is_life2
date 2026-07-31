//! The Echo, driven end to end: the whole fifty-five-minute chapter.
//!
//! What this file proves is the chapter's own claim. The Echo is built on one
//! relation and asks the player to read it off the surface rather than off a
//! sentence: **what the Form gathers now is what the ring turns on later.** A
//! Route runs from the Form's own Node to a store beyond the bright band, the
//! ring runs on what that store holds, and twice in the chapter the bright
//! current stops and the layer's loss is raised while the ring is being held.
//! The first stop is a rehearsal — short, early, and paid for in time. The
//! second is the chapter's final challenge, and it is carried by what was laid
//! down before it.
//!
//! Four runs are driven, and they answer four different questions:
//!
//! - The **attentive run** carries a modelled first-time player: it holds and
//!   travels at the speeds the Field allows and spends the named discovery
//!   allowances below looking for what to do next. Its milestone table is what
//!   the fifty-five minutes are read against.
//! - The **authored floor** spends no allowance at all — what a player who
//!   already knows the chapter would take. It is the run the eight Forms are
//!   driven through.
//! - The **withheld run** lets the optional test go, so it reaches the final
//!   challenge with no second line into the ring. It is what turns "the final
//!   challenge is carried by an earlier trace" from a claim into a
//!   measurement: it stands in exactly the same script and pays about ten
//!   thousand steps more.
//! - The **Trail run** lets the test go as well, and answers the break by
//!   standing over the ring instead: Wake's deposits come due sixty steps
//!   after they are left, inside the very stop they answer.
//!
//! The allowances are a model and are stated as one: they are the only part of
//! the timing that is not authored content, and every one is named in the phase
//! list so a reader can see exactly what is being assumed.
//!
//! **Why the breaks are authored as events.** A `pressure_schedule` counts from
//! the chapter's own opening, so its step is absolute; an objective is offered
//! when a player reaches it, so its step is not, and the spread across the four
//! paths this file drives — two allowance settings by two answers to one
//! optional test — is many thousands of steps. Events are objective-relative
//! and land at the same offset on every one of the four, which is why the
//! chapter's breaks are events and its one pressure is placed by measurement
//! into the span all four stand inside.

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

/// The chapter's own index in the campaign, and the one after it.
const CHAPTER: i64 = 3;

/// How often every Node's Charge is sampled, in steps.
const SAMPLE: Step = 900;

/// Where the near ring Node stands, in whole units — the point the Trail run
/// holds over, because a deposit reaches sixty-four units and this is the only
/// Node of the ring inside that reach from there.
const RING: (i64, i64) = (1900, 1990);

/// What a run does while the final challenge stands.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Stance {
    /// Stand in the bright band, which is where a run gathers when the band is
    /// running and where it stands idle when it is not.
    Light,
    /// Stand over the near ring Node. For Wake this leaves a deposit every
    /// fifteen steps that comes due sixty steps later; for every other Form it
    /// is the same standing still with nothing gathered.
    Ring,
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
    /// The plan bodies one Still Mode visit queues and commits. Empty for every
    /// stepping phase. It rides the phase rather than [`Act`], because `Act` is
    /// the shared closed set another chapter's driver matches exhaustively.
    plans: &'static [&'static str],
}

const fn play(label: &'static str, act: Act, steps: u32) -> Phase {
    Phase { label, act, steps, allowance: false, plans: &[] }
}

const fn allow(label: &'static str, act: Act, steps: u32) -> Phase {
    Phase { label, act, steps, allowance: true, plans: &[] }
}

/// A queued-change phase: no simulated time, two ramps of real time, and the
/// plan bodies committed between them.
const fn edit(label: &'static str, plans: &'static [&'static str]) -> Phase {
    Phase { label, act: Act::Rest, steps: 0, allowance: false, plans }
}

/// The second line, laid by hand instead of taken as the authored test: one
/// Route from the deep store straight into the store the ring runs on.
///
/// The two Nodes stand 656 units apart — 144 across the plane and one layer,
/// which the distance rule prices at 512 — so the connect is admitted only by a
/// Form whose `route_reach` reaches that far. Two of the eight do: Relay at
/// 1,088 and Knot at 704. It is the chapter's one reading of `route_reach` as a
/// live difference between Forms rather than as a constraint on authoring.
const SECOND_LINE: &[&str] = &["{\"from\":6,\"op\":\"connect\",\"to\":2}"];

/// The scripted run, phase by phase.
///
/// Positions are in whole units and are the authored content's own: the bright
/// band's own waypoints, the three Ports of the surface, the deep band, and the
/// Port the optional test names. The phases that hold a band aim at the band's
/// nearest path point, read off the Field each batch rather than off the file.
///
/// `takes_the_test` decides whether the run opens the spare line or lets its
/// span run out. `stance` decides what the run does under the final challenge.
/// All four combinations are driven, because all four are what the chapter is.
fn script(takes_the_test: bool, stance: Stance, lays_by_hand: bool) -> Vec<Phase> {
    let mut phases = vec![
        // The opening: the Form stands in the bright band already, and the
        // Route from its own Node carries what it gathers east to the store.
        allow("read the surface", Act::Rest, 120),
        play("hold the bright band", Act::Follow(1), 11_100),
        allow("look for what the line reaches", Act::Toward(1700, 2060), 1_500),
        play("cross to the near Port", Act::Toward(1900, 1990), 900),
        play("charge", Act::Charge(1900, 1990), FULL_CHARGE_STEPS),
        play("release", Act::Release, 1),
        allow("look for the second Port", Act::Toward(2010, 2160), 900),
        play("cross to the far Port", Act::Toward(2010, 2160), 400),
        play("charge", Act::Charge(2010, 2160), FULL_CHARGE_STEPS),
        play("release", Act::Release, 1),
        play("watch the ring turn", Act::Toward(2010, 2160), 200),
        allow("look for what is carrying too much", Act::Toward(2100, 1880), 2_400),
        play("cross to the Port beside it", Act::Toward(2100, 1880), 400),
        play("charge", Act::Charge(2100, 1880), FULL_CHARGE_STEPS),
        play("release", Act::Release, 1),
        play("back to the bright band", Act::Follow(1), 700),
        play("hold the ring", Act::Follow(1), 20_500),
        allow("read what the stop did", Act::Follow(1), 1_200),
        // The deep band: the second store has been standing in it since the
        // chapter opened, and this is where a run first sees it.
        allow("look for what lies deeper", Act::Rest, 900),
        play("cross to above the deep band", Act::Toward(1700, 2180), 900),
        play("take the depth", Act::Depth(1), 1),
        play("hold the deep band", Act::Follow(2), 16_700),
    ];
    if lays_by_hand {
        // The other way to a second line into the ring, and the one the
        // authored test does not offer: Still Mode, one connect, one Impulse.
        // The run lets the authored test go, so what carries the finale is the
        // Route the player drew and nothing else.
        phases.push(play("hold the deep band on", Act::Follow(2), 5_600));
        phases.push(edit("lay the second line by hand", SECOND_LINE));
    } else if takes_the_test {
        phases.extend([
            allow("look for what stands down here", Act::Toward(1860, 2250), 900),
            play("cross to the spare Port", Act::Toward(1880, 2250), 500),
            play("charge", Act::Charge(1880, 2250), FULL_CHARGE_STEPS),
            play("release", Act::Release, 1),
        ]);
    } else {
        // The other shape the same test stands in: the span runs out with the
        // Port still closed, and the chapter carries on without it.
        phases.push(play("let the line go", Act::Follow(2), 5_600));
    }
    phases.extend([
        play("back to the surface", Act::Depth(-1), 1),
        allow("find the bright band again", Act::Follow(1), 600),
        play("cross back to the bright band", Act::Follow(1), 700),
        play("fill the store again", Act::Follow(1), 17_000),
    ]);
    let held = match stance {
        Stance::Light => play("hold the ring through it", Act::Follow(1), 40_000),
        Stance::Ring => play("hold over the ring", Act::Toward(RING.0, RING.1), 40_000),
    };
    phases.push(held);
    phases
}

/// One milestone the run reached, and the step it reached it at.
#[derive(Clone, Debug)]
struct Milestone {
    name: String,
    step: Step,
}

/// One sample of the standing store, taken once a batch under the objective
/// named: the step, and what the store held in whole units.
#[derive(Clone, Copy, Debug)]
struct Sample {
    step: Step,
    units: i64,
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
    /// The step the run entered this chapter at, which every objective-relative
    /// reading below is taken against.
    entered_step: Option<Step>,
    /// Every step at which the bright band stopped, and every step at which it
    /// stood again.
    stopped: Vec<Step>,
    resumed: Vec<Step>,
    /// Every step at which the layer's loss was raised, and every step at which
    /// it was put back.
    spiked: Vec<Step>,
    settled: Vec<Step>,
    /// The store's Charge, sampled once a batch while the final challenge
    /// stood.
    store: Vec<Sample>,
    /// Every Node's Charge in whole units, sampled every `SAMPLE` steps: the
    /// chapter's own reading of what is retained and what is spent.
    held: Vec<(Step, Vec<(u32, i64)>)>,
    /// How many batch samples under the final challenge found a Route of the
    /// ring carrying nothing.
    stalled: u32,
    /// How many batch samples under the final challenge were taken at all.
    samples: u32,
    /// Whether the optional test's Port stood open when the chapter ended.
    spare_open: bool,
    /// The steps at which the chapter's one pressure was observed standing —
    /// admitted, not queued — sampled once a batch.
    pressed: Vec<Step>,
    /// The refusal the queue answered a plan body with, and none when every one
    /// of them was admitted.
    refused: Option<String>,
    /// Whether the run laid a Route of its own, and how many stood at the close.
    routes: usize,
    /// What the forecast overlay carried while the run stood still: how many
    /// steps of envelope the standing candidate's baselines left, and whether
    /// the slate was deficient. None when the run never entered Still Mode.
    forecast: Option<(usize, bool)>,
    /// What the narrow Route into the ring carried, sampled once a batch while
    /// pressure was placed under stood: the step, whether the pressure stood on
    /// it, the flow on route 4 raw, and what the store held in whole units.
    ///
    /// Route 4 is the narrowest Route into the ring and its flow runs at its
    /// authored capacity whenever the store holds anything, so it is the one
    /// Route on this Field a drawn flow scale can be read off. A pressure that
    /// narrows nothing is a pressure the chapter would be better off without,
    /// and this is what says which it is.
    ring_route: Vec<(Step, bool, i64, i64)>,
    /// What the run held when the chapter ended, in whole units: the Charge
    /// stored across the standing inside, and what the controlled Form's Node
    /// held.
    inside_units: i64,
    form_units: i64,
    forms: usize,
}

impl Driven {
    fn at(&self, name: &str) -> Option<Step> {
        self.milestones.iter().find(|held| held.name == name).map(|held| held.step)
    }

    fn offered_at(&self, id: &str) -> Option<Step> {
        self.offered.iter().find(|(held, _)| held == id).map(|(_, step)| *step)
    }

    /// How long the run spent on one objective: offered to completed.
    fn span_of(&self, id: &str) -> Option<Step> {
        Some(self.at(id)? - self.offered_at(id)?)
    }

    /// True when the run's objectives were offered in the authored order.
    fn progressed(&self) -> bool {
        let mut last = 0;
        for (id, step) in &self.offered {
            if !id.starts_with("objective.the_echo.") {
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

/// Writes what one batch of events said into the record.
fn record(driven: &mut Driven, events: Vec<(String, u32, field_game_core::json::Json)>) {
    for (name, step, body) in events {
        match name.as_str() {
            "objective_changed" => {
                let objective = body.get("objective").expect("an objective");
                let id = objective.get("id").and_then(|held| held.as_text()).unwrap_or("");
                let stage = objective.get("state").and_then(|held| held.as_text()).unwrap_or("");
                if !id.starts_with("objective.the_echo.") {
                    continue;
                }
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
                let opened = driven.entered_step.unwrap_or(Step::MAX);
                if step > opened && driven.transition_step.is_none() {
                    driven.anchor_step.get_or_insert(step);
                    driven.milestones.push(Milestone { name: "anchor".to_string(), step });
                }
            }
            "chapter_changed" => {
                match body.get("chapter_index").and_then(|held| held.as_int()) {
                    Some(index) if index == CHAPTER => {
                        driven.entered_step = Some(step);
                        driven.milestones.push(Milestone { name: "entered".to_string(), step });
                    }
                    Some(index) if index == CHAPTER + 1 => {
                        driven.transition_step = Some(step);
                        driven.milestones.push(Milestone { name: "transition".to_string(), step });
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
}

/// An empty record, before a run writes into it.
fn fresh_record() -> Driven {
    Driven {
        milestones: Vec::new(),
        completed: Vec::new(),
        stages: Vec::new(),
        offered: Vec::new(),
        steps: 0,
        anchor_step: None,
        collapse_step: None,
        recovery_step: None,
        transition_step: None,
        entered_step: None,
        stopped: Vec::new(),
        resumed: Vec::new(),
        spiked: Vec::new(),
        settled: Vec::new(),
        store: Vec::new(),
        held: Vec::new(),
        stalled: 0,
        samples: 0,
        spare_open: false,
        pressed: Vec::new(),
        refused: None,
        routes: 0,
        forecast: None,
        ring_route: Vec::new(),
        inside_units: 0,
        form_units: 0,
        forms: 0,
    }
}

/// Drives one run of the chapter: a named starting Form, whether the modelled
/// allowances are spent, whether the optional test is taken, and what the run
/// does under the final challenge.
///
/// The three chapters before this one are placeholders that complete by
/// standing where they open, so the run is carried to this chapter's opening
/// with neutral input and the script starts there.
fn drive_as(form: &str, allowances: bool, takes_the_test: bool, stance: Stance) -> Driven {
    drive_full(form, allowances, takes_the_test, stance, false)
}

/// The same, with the second line laid by hand in Still Mode rather than taken
/// as the authored test. `refused` carries the refusal the queue answered with,
/// and none when the connect was admitted — a Form whose reach does not stretch
/// to 656 units cannot lay it, which is the whole of what this variant reads.
fn drive_with_line(form: &str) -> (Driven, Option<String>) {
    let mut run = drive_full(form, false, false, Stance::Light, true);
    let refused = run.refused.take();
    (run, refused)
}

fn drive_full(
    form: &str,
    allowances: bool,
    takes_the_test: bool,
    stance: Stance,
    lays_by_hand: bool,
) -> Driven {
    let mut session = Session::new(&support::worker_init()).expect("versions agree");
    let opened = session.command(
        "init_run",
        &format!("{{\"form\":\"{form}\",\"mode\":\"new\",\"run_id\":\"{KEY}\"}}"),
    );
    assert!(opened.contains("\"ok\":true"), "{opened}");
    let mut driven = fresh_record();
    let mut driver = support::campaign::Driver::new();
    // Every chapter before this one, played through the shared dispatch: the
    // scripted ones by their own scripts, the placeholders by resting.
    for index in 0..CHAPTER {
        support::campaign::play_chapter(
            &mut session,
            &mut driver,
            authored().chapter(index as u8).expect("a chapter"),
        );
    }
    driver.drain(&mut session);
    // The chapter opens here. Everything below is this chapter's own — and the
    // events the carry raised are read rather than dropped, because the step
    // that carried the run into this chapter is also the step its opening
    // objective was offered on.
    record(&mut driven, std::mem::take(&mut driver.raised));
    record(&mut driven, support::campaign::raised(&mut session));
    driven.entered_step.get_or_insert(session.run().expect("a run").state().now.step);

    let last = "objective.the_echo.hold_through_the_echo";
    let mut seq = driver.seq;
    let mut band_active = true;
    let mut drain = layer_drain(&session, 0);
    for phase in script(takes_the_test, stance, lays_by_hand) {
        if phase.allowance && !allowances {
            continue;
        }
        if !phase.plans.is_empty() {
            if driven.transition_step.is_none() {
                still_edit(&mut session, &mut seq, phase.plans, &mut driven);
                record(&mut driven, support::campaign::raised(&mut session));
            }
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
            // Field a transition establishes is the next chapter's.
            let crossed = driven.transition_step.is_some();
            if crossed {
                break;
            }
            let step = state.now.step;
            let standing = state
                .now
                .currents
                .iter()
                .find(|held| held.id == 1)
                .is_some_and(|held| held.active);
            if standing != band_active {
                match standing {
                    false => driven.stopped.push(step),
                    true => driven.resumed.push(step),
                }
                band_active = standing;
            }
            let now_drain = layer_drain_state(&state, 0);
            if now_drain != drain {
                match now_drain > drain {
                    true => driven.spiked.push(step),
                    false => driven.settled.push(step),
                }
                drain = now_drain;
            }
            // The steps the chapter's pressure actually **stood**, which is
            // not the same as the steps the list was non-empty: a scheduled
            // entry sits queued in the list from the chapter's opening until
            // its own step admits it.
            let standing_pressure = state.pressures.iter().any(|held| !held.queued);
            if standing_pressure {
                driven.pressed.push(step);
            }
            if state.progress.objective.id == "objective.the_echo.hold_the_ring" {
                let flow = state
                    .now
                    .routes
                    .iter()
                    .find(|route| route.route == 4)
                    .map_or(0, |route| route.flow);
                let store = state
                    .now
                    .ports
                    .iter()
                    .find(|port| port.node == 2)
                    .map_or(0, |port| port.q / ONE_UNIT);
                driven.ring_route.push((step, standing_pressure, flow, store));
            }
            driven.spare_open = state.now.ports.iter().any(|port| port.node == 7 && port.open);
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
            driven.routes = state.now.routes.len();
            if driven.held.last().is_none_or(|(last, _)| step >= last + SAMPLE) {
                driven.held.push((
                    step,
                    state.now.ports.iter().map(|port| (port.node, port.q / ONE_UNIT)).collect(),
                ));
            }
            // The final challenge, sampled: what the store holds, and whether
            // every Route of the ring carried something on the step just run.
            if state.progress.objective.id == last
                && state.progress.objective.state == field_game_core::state::ObjectiveStage::Active
            {
                let units = state
                    .now
                    .ports
                    .iter()
                    .find(|port| port.node == 2)
                    .map_or(0, |port| port.q / ONE_UNIT);
                driven.store.push(Sample { step, units });
                driven.samples += 1;
                let turning = [4u32, 5, 6].iter().all(|named| {
                    state.now.routes.iter().any(|route| route.route == *named && route.flow > 0)
                });
                if !turning {
                    driven.stalled += 1;
                }
            }
        }
        if driven.transition_step.is_some() {
            break;
        }
    }

    driven
}

/// How long a Still Mode ramp takes, in microseconds. Locked.
const RAMP_US: i64 = 250_000;

/// Enters Still Mode, queues every plan body, commits, and ramps back out.
///
/// The frames carry their own timestamps because a ramp is real time: every
/// other frame the script sends carries `t_us` 0, so the gap the accumulator
/// reads clamps to zero and a ramp stands still until this asks it to move.
///
/// The visit is also where the **forecast overlay** stands: entering Still Mode
/// assembles the candidate slate and evaluates it, and the standing candidate's
/// baseline replays leave the envelope the paused surface draws. The reading is
/// taken here rather than asserted from the outside, because the mode is the
/// only place it exists.
fn still_edit(session: &mut Session, seq: &mut u32, plans: &[&str], driven: &mut Driven) {
    let opened = i64::from(*seq) * 1_000_000 + 10_000_000;
    let frame = |session: &mut Session, seq: &mut u32, steps: u16, t_us: i64, toggle: bool| {
        let body = format!(
            "{{\"advance_steps\":{steps},\"depth_key\":0,\"inspect\":null,\"pause\":false,\
             \"pulse_held\":false,\"pulse_release\":false,\"seq\":{seq},\"steer_x\":0,\
             \"steer_y\":0,\"t_us\":{t_us},\"toggle_still\":{toggle},\"wheel\":0}}",
        );
        let answer = session.command("input_frame", &body);
        assert!(answer.contains("\"ok\":true"), "{answer}");
        *seq += 1;
    };
    frame(session, seq, 1, opened, true);
    assert_eq!(session.lifecycle(), "ramp_in");
    frame(session, seq, 0, opened + RAMP_US, false);
    assert_eq!(session.lifecycle(), "still", "the ramp completes into Still Mode");

    // The overlay, read where it stands.
    if let Some(slate) = &session.run().expect("a run").state().slate {
        driven.forecast = Some((slate.forecast_envelope.len(), slate.deficient));
    }

    let mut admitted = true;
    for plan in plans {
        let answer = session.command("queue_plan", &format!("{{\"plan\":{plan}}}"));
        if !answer.contains("\"ok\":true") {
            driven.refused = Some(answer);
            admitted = false;
        }
    }
    if admitted {
        let answer = session.command("commit_plan", "{}");
        assert!(answer.contains("\"ok\":true"), "committing: {answer}");
        assert_eq!(session.lifecycle(), "ramp_out", "a commit is also the exit");
    } else {
        // Nothing was queued, so there is nothing to commit and the way out is
        // the toggle. A refused entry is the reading this run was driven for.
        frame(session, seq, 0, opened + RAMP_US + 1, true);
    }
    frame(session, seq, 0, opened + 2 * RAMP_US + 2, false);
    assert_eq!(session.lifecycle(), "running", "and the run stands again");
}

/// One layer's loss, as the Field stands.
fn layer_drain(session: &Session, layer: u8) -> i64 {
    layer_drain_state(&session.run().expect("a run").state(), layer)
}

fn layer_drain_state(state: &field_game_core::state::RunState, layer: u8) -> i64 {
    state.now.layers.iter().find(|held| held.layer == layer).map_or(0, |held| held.drain)
}

/// The attentive first run: every allowance spent, and the test taken.
fn attentive() -> Driven {
    drive_as("thread", true, true, Stance::Light)
}

/// The authored floor: no allowance spent, and the test taken.
fn floor() -> Driven {
    drive_as("thread", false, true, Stance::Light)
}

/// The same floor with the optional test let go: the run that reaches the final
/// challenge with no second line into the ring.
fn withheld() -> Driven {
    drive_as("thread", false, false, Stance::Light)
}

/// The authored content, read as the worker hands it over.
fn authored() -> content::Content {
    let bundle = field_game_core::json::parse(&support::bundle_with(&support::content_hash()))
        .expect("canonical");
    content::read_bundle(&bundle).expect("the content reads")
}

/// The chapter this file is about.
fn chapter() -> content::Chapter {
    authored().chapter(CHAPTER as u8).expect("the fourth chapter").clone()
}

/// A step count as sim-minutes, to one decimal place, as a string.
fn minutes(step: Step) -> String {
    let tenths = i64::from(step) * 10 / (i64::from(STEPS_PER_SECOND) * 60);
    format!("{}.{}", tenths / 10, tenths % 10)
}

/// The chapter's objectives, in the authored order.
fn authored_order() -> Vec<String> {
    chapter().objectives.iter().map(|held| held.id.clone()).collect()
}

/// The chapter's required objectives, in the authored order.
fn required_order() -> Vec<String> {
    chapter()
        .objectives
        .iter()
        .filter(|held| held.optional.is_none())
        .map(|held| held.id.clone())
        .collect()
}

/// The step one run spent inside this chapter, from its opening to the
/// transition past it.
fn chapter_span(run: &Driven) -> Step {
    run.transition_step.expect("the chapter completes")
        - run.entered_step.expect("and the run entered it")
}

/// Prints one run's milestone table, with every step given relative to the
/// chapter's own opening.
fn table(named: &str, run: &Driven) {
    let opened = run.entered_step.unwrap_or(0);
    println!("\nThe Echo — {named}");
    println!("{:<48} {:>8} {:>8}", "milestone", "step", "minutes");
    for milestone in &run.milestones {
        let step = milestone.step - opened;
        println!("{:<48} {step:>8} {:>8}", milestone.name, minutes(step));
    }
    for (name, step) in [
        ("first setback", run.collapse_step),
        ("standing again after it", run.recovery_step),
    ] {
        if let Some(step) = step {
            println!("{name:<48} {:>8} {:>8}", step - opened, minutes(step - opened));
        }
    }
    for (name, steps) in [
        ("the bright band stopped", &run.stopped),
        ("the bright band stood again", &run.resumed),
        ("the layer's loss was raised", &run.spiked),
        ("the layer's loss was put back", &run.settled),
    ] {
        let listed: Vec<String> = steps.iter().map(|step| (step - opened).to_string()).collect();
        println!("{name:<48} {}", listed.join(", "));
    }
    println!("offered at");
    for (id, step) in &run.offered {
        println!("  {id:<46} {:>8} {:>8}", step - opened, minutes(step - opened));
    }
    println!("what every Node held, every {SAMPLE} steps, in whole units");
    let nodes: Vec<u32> = run.held.first().map(|(_, held)| held.iter().map(|(node, _)| *node).collect()).unwrap_or_default();
    print!("{:>8}", "step");
    for node in &nodes {
        print!(" {:>7}", format!("node {node}"));
    }
    println!();
    for (step, held) in &run.held {
        print!("{:>8}", step - opened);
        for (_, units) in held {
            print!(" {units:>7}");
        }
        println!();
    }
}

// ---------------------------------------------------------------------------
// The chapter, completed
// ---------------------------------------------------------------------------

#[test]
fn the_chapter_completes_and_carries_the_run_into_the_chapter_after_it() {
    let run = attentive();
    table("attentive first run", &run);

    assert_eq!(
        run.completed,
        authored_order(),
        "every objective completes once each, in the authored order",
    );
    let transition = run.transition_step.expect("the chapter completes and the run carries on");
    assert_eq!(
        transition,
        run.at("objective.the_echo.hold_through_the_echo").expect("the last"),
        "the transition lands on the step the final challenge completed",
    );
    assert!(run.anchor_step.is_some(), "the chapter's own Anchor moment was written");
}

#[test]
fn the_chapter_asks_for_about_fifty_five_minutes_and_the_floor_stands_under_it() {
    let floor = floor();
    let attentive = attentive();
    let floor_at = chapter_span(&floor);
    let attentive_at = chapter_span(&attentive);

    let spent: u32 = script(true, Stance::Light, false)
        .iter()
        .filter(|phase| phase.allowance)
        .map(|phase| phase.steps)
        .sum();
    println!("\nThe Echo — the authored floor against the attentive first run");
    println!("  authored floor      {floor_at:>7} steps  {:>6} minutes", minutes(floor_at));
    println!("  attentive first run {attentive_at:>7} steps  {:>6} minutes", minutes(attentive_at));
    println!("modelled first-run allowances, {spent} steps in all:");
    for phase in script(true, Stance::Light, false).iter().filter(|phase| phase.allowance) {
        println!("  {:<40} {:>6} steps", phase.label, phase.steps);
    }
    table("the authored floor", &floor);
    println!("\nThe Echo — how long each objective stood, attentive run");
    for id in authored_order() {
        match attentive.span_of(&id) {
            Some(span) => println!("  {id:<48} {span:>7} {:>7}", minutes(span)),
            None => println!("  {id:<48} {:>7}", "let go"),
        }
    }

    // The fourth path, and the longest: every allowance spent **and** the
    // optional test let go. The chapter's own thesis is that declining the test
    // costs about ten thousand steps at the finale, so this path is not a
    // variant of the fifty-five minutes — it is the fifty-five minutes plus the
    // price of the decision, and it gets a band of its own rather than being
    // left out of the pacing model.
    let declined = drive_as("thread", true, false, Stance::Light);
    let declined_at = chapter_span(&declined);
    println!(
        "  attentive, test declined {declined_at:>7} steps  {:>6} minutes",
        minutes(declined_at),
    );

    let minute = 60 * STEPS_PER_SECOND;
    // Fifty-five minutes is the chapter's own figure, and it describes the
    // **prepared** path: the attentive run that takes the optional test. The
    // band is wide enough that a re-tuned phase moves a number a reader can
    // check rather than one that quietly drifts.
    assert!(
        attentive_at > 52 * minute && attentive_at < 58 * minute,
        "the attentive run completes the chapter in {attentive_at} steps ({} minutes), \
         outside the fifty-five-minute band the prepared path is read against",
        minutes(attentive_at),
    );
    assert!(
        floor_at > 44 * minute && floor_at < attentive_at,
        "the authored floor completes in {floor_at} steps ({} minutes), outside its band",
        minutes(floor_at),
    );
    // The unprepared path's own band. It is the chapter's longest honest run,
    // and it is banded rather than left to be discovered by a reviewer adding
    // the two figures together.
    assert!(
        declined_at > 58 * minute && declined_at < 66 * minute,
        "the attentive run that declined the test completes in {declined_at} steps \
         ({} minutes), outside the band the unprepared path is read against",
        minutes(declined_at),
    );
    assert!(
        declined_at > attentive_at,
        "declining the test cost nothing at all, which is not the chapter's own claim",
    );
}

#[test]
fn every_starting_form_completes_the_whole_chapter() {
    // The acceptance bar, driven rather than argued: the same script, the eight
    // Forms, and every objective completing in the authored order under each of
    // them — the final challenge included. The script must not be re-tuned per
    // Form, or the answer would be about the tuning.
    let authored = authored_order();
    println!("\nthe eight Forms through the whole chapter");
    println!(
        "{:<8} {:>10} {:>8} {:>8} {:>8} {:>6} {:>7} {:>6} {:>7}",
        "form", "objectives", "steps", "minutes", "setback", "forms", "inside", "held", "stalled",
    );
    let mut readings: Vec<(&str, i64, i64, usize)> = Vec::new();
    let mut lengths: Vec<Step> = Vec::new();
    for form in field_game_core::run::FORMS {
        let run = drive_as(form, false, true, Stance::Light);
        assert_eq!(run.completed, authored, "{form} completes the whole chapter in order");
        let at = chapter_span(&run);
        for stage in &run.stages {
            assert!(
                ["hidden", "active", "complete", "failed_recoverable"].contains(&stage.as_str()),
                "{form} stood in an unexpected objective state: {stage}",
            );
        }
        assert!(run.spare_open, "{form} opened the Port the optional test names");
        println!(
            "{form:<8} {:>10} {at:>8} {:>8} {:>8} {:>6} {:>7} {:>6} {:>7}",
            run.completed.len(),
            minutes(at),
            run.collapse_step
                .map(|step| (step - run.entered_step.unwrap_or(0)).to_string())
                .unwrap_or_else(|| "none".to_string()),
            run.forms,
            run.inside_units,
            run.form_units,
            run.stalled,
        );
        readings.push((form, run.inside_units, run.form_units, run.forms));
        lengths.push(at);
    }
    // The same script, eight different Fields at the end of it — but **not**
    // eight different readings, and the chapter says why rather than asserting
    // a count it does not mean. The opening View seals the Form's own Node
    // inside the boundary, so Leakage does nothing here and the Charge
    // arithmetic is Form-independent: six of the eight end identically **by
    // construction**, and the two that do not are the two whose authored
    // ability changes what the Field holds. The magnitudes are stated, so a
    // change that flattens either one fails here rather than reading as one
    // fewer distinct triple.
    let of = |form: &str| readings.iter().find(|held| held.0 == form).expect("a reading");
    let mut distinct: Vec<(i64, i64, usize)> =
        readings.iter().map(|held| (held.1, held.2, held.3)).collect();
    distinct.sort_unstable();
    distinct.dedup();
    assert_eq!(
        distinct.len(),
        3,
        "the eight Forms end the chapter on {} distinct readings, not the three the sealed \
         inside allows: the six alike, Wake's Trail, and Chorus's link",
        distinct.len(),
    );
    let alike: Vec<&str> = readings
        .iter()
        .filter(|held| held.0 != "wake" && held.0 != "chorus")
        .map(|held| held.0)
        .collect();
    assert_eq!(alike.len(), 6, "six Forms author neither a Trail nor a link");
    for form in &alike {
        assert_eq!(
            (of(form).1, of(form).3),
            (of("thread").1, of("thread").3),
            "{form} ends holding something other than what the sealed inside makes every \
             Form without an ability hold",
        );
    }
    // Chorus stands four Forms where every other stands one — the whole of its
    // authored `linked_forms`, and the only reading of it this chapter takes.
    assert_eq!(of("chorus").3, 4, "Chorus stands its linked Forms");
    assert!(
        readings.iter().all(|held| held.0 == "chorus" || held.3 == 1),
        "a Form that authors no link stood more than one Form",
    );
    // Wake's Trail delivers 16 units every fifteen steps into its own Node, and
    // the chapter is long enough that what it has accumulated by the close is
    // two orders of magnitude past what a Form without one holds. The stated
    // magnitude is 100×, against the 4× the first round asked for.
    assert!(
        of("wake").2 > of("thread").2 * 100,
        "Wake ends holding {} against Thread's {}, which is not a Trail coming due over a \
         fifty-minute chapter",
        of("wake").2,
        of("thread").2,
    );

    // And the chapter is not the same length for all of them. Vault authors the
    // smallest `steer_scale` of the eight, so it is the slowest to cross the
    // Field and the last to close — stated as an ordering and a magnitude
    // rather than as "at least two lengths stand".
    let mut spans: Vec<Step> = lengths.clone();
    spans.sort_unstable();
    let slowest = *lengths.iter().max().expect("eight readings");
    let quickest = *lengths.iter().min().expect("eight readings");
    let vault = lengths[field_game_core::run::FORMS.iter().position(|held| *held == "vault")
        .expect("Vault is authored")];
    assert_eq!(vault, slowest, "Vault authors the smallest steer_scale and is the last to close");
    assert!(
        slowest - quickest >= 20,
        "the eight Forms close within {} steps of each other, which is not a difference the \
         authored steering makes",
        slowest - quickest,
    );
}

// ---------------------------------------------------------------------------
// The chapter's own beat: an earlier trace against a later break
// ---------------------------------------------------------------------------

#[test]
fn the_authored_breaks_land_at_their_own_steps_on_every_path() {
    // The chapter's breaks are timed against objectives rather than against the
    // run's step counter, so they land at the same offset in a run that spent
    // every allowance and one that spent none, and in a run that took the
    // optional test and one that let it go. All four are driven, because the
    // optional test's span is itself a difference of several thousand steps
    // between two paths that are otherwise identical.
    let chapter = chapter();
    let stops: Vec<&content::AuthoredEvent> = chapter
        .events
        .iter()
        .filter(|event| {
            matches!(event.effect, content::Effect::CurrentActive { active: false, .. })
        })
        .collect();
    let spikes: Vec<&content::AuthoredEvent> = chapter
        .events
        .iter()
        .filter(|event| matches!(event.effect, content::Effect::LayerDrain { .. }))
        .collect();
    assert_eq!(stops.len(), 4, "the chapter stops the bright band four times");
    assert_eq!(spikes.len(), 8, "and raises and puts back the layer's loss four times");

    for (named, run) in [
        ("floor, test taken", floor()),
        ("floor, test let go", withheld()),
        ("attentive, test taken", attentive()),
        ("attentive, test let go", drive_as("thread", true, false, Stance::Light)),
    ] {
        let mut expected: Vec<Step> = stops
            .iter()
            .map(|event| run.offered_at(&event.objective).expect("the objective stood") + event.at)
            .collect();
        expected.sort_unstable();
        assert_eq!(run.stopped.len(), expected.len(), "{named}: a break did not land");
        // The step is read once a batch, so a landing is placed to within one
        // batch of the step the chapter names. What is exact is the offset from
        // the objective's own offer, which is what makes an event immune to the
        // spread between these four paths.
        for (landed, named_step) in run.stopped.iter().zip(expected.iter()) {
            let late = i64::from(*landed) - i64::from(*named_step);
            assert!(
                (0..i64::from(BATCH)).contains(&late),
                "{named}: a break landed {late} steps from the step the chapter names",
            );
        }
        assert_eq!(
            run.spiked.len(),
            4,
            "{named}: the layer's loss was raised {} times",
            run.spiked.len(),
        );
        assert_eq!(run.stopped, run.spiked, "{named}: each break is one stop and one raise");
        assert_eq!(run.resumed.len(), 4, "{named}: every break is put back");
        assert_eq!(run.settled.len(), 4, "{named}: and so is every raise");
        for (stopped, resumed) in run.stopped.iter().zip(run.resumed.iter()) {
            assert!(resumed > stopped, "{named}: a break was put back before it landed");
        }
    }
}

#[test]
fn the_final_challenge_is_carried_by_what_was_laid_down_before_it() {
    // The chapter's claim, measured. Two runs stand in the same script at the
    // same floor and differ in one thing: whether the optional test's Port was
    // opened, an act that happens more than twenty thousand steps before the
    // first break of the final challenge and is the only second line into the
    // ring the chapter places.
    //
    // The run that opened it holds the ring through every break without one
    // Route ever carrying nothing. The run that did not stands with the ring
    // stopped for hundreds of steps at a time, and the count the challenge
    // keeps starts again each time, which is what the difference in steps is.
    let last = "objective.the_echo.hold_through_the_echo";
    let taken = floor();
    let let_go = withheld();

    let taken_span = taken.span_of(last).expect("the final challenge completed");
    let let_go_span = let_go.span_of(last).expect("and completed on the other path too");
    println!("\nThe Echo — the final challenge, with the earlier line and without it");
    println!(
        "  the spare line opened   {taken_span:>7} steps  {:>6} minutes, \
         {} of {} samples with the ring stopped",
        minutes(taken_span),
        taken.stalled,
        taken.samples,
    );
    println!(
        "  the spare line let go   {let_go_span:>7} steps  {:>6} minutes, \
         {} of {} samples with the ring stopped",
        minutes(let_go_span),
        let_go.stalled,
        let_go.samples,
    );
    let low = |run: &Driven| {
        run.store
            .iter()
            .min_by_key(|held| held.units)
            .map_or((0, -1), |held| (held.step - run.entered_step.unwrap_or(0), held.units))
    };
    for (named, run) in [("with the line", &taken), ("without it", &let_go)] {
        let (step, units) = low(run);
        println!("  the store's low mark {named:<12} {units:>6} units, at step {step}");
    }

    assert_eq!(taken.stalled, 0, "the prepared run holds the ring through every break");
    assert!(
        let_go.stalled > 0,
        "the withheld run met no break it could not carry, so the chapter asks nothing",
    );
    assert!(
        let_go_span > taken_span + 6_000,
        "letting the line go cost {} steps, which is not a challenge the earlier act carries",
        let_go_span - taken_span,
    );
    // And the run that let it go still completes: the chapter's breaks are
    // recoverable, and nothing here is a run a player loses.
    assert!(let_go.transition_step.is_some(), "the chapter completes on the withheld path too");
    assert_eq!(let_go.completed, required_order(), "with every required objective behind it");
}

#[test]
fn a_trail_left_before_a_break_comes_due_inside_it() {
    // The second counter the chapter admits, and the one that is literally a
    // delayed trace: Wake leaves a deposit every fifteen steps at the position
    // it stands at, and each deposit comes due sixty steps later and gives its
    // whole magnitude to the Nodes within its reach. A Wake that stands over
    // the ring while the bright band is stopped is answering the break with
    // deposits it left before the break landed.
    //
    // The same stance is driven under Thread, which authors no Trail at all, so
    // what is measured is the Trail and not the standing.
    let wake = drive_as("wake", false, false, Stance::Ring);
    let thread = drive_as("thread", false, false, Stance::Ring);
    let last = "objective.the_echo.hold_through_the_echo";

    println!("\nThe Echo — standing over the ring through the final challenge");
    for (named, run) in [("wake", &wake), ("thread", &thread)] {
        println!(
            "  {named:<7} {:>7} steps, {} of {} samples with the ring stopped, \
             store low mark {} units",
            run.span_of(last).unwrap_or(0),
            run.stalled,
            run.samples,
            run.store.iter().map(|held| held.units).min().unwrap_or(-1),
        );
    }
    let trail = authored()
        .form("wake")
        .expect("Wake is authored")
        .abilities
        .iter()
        .find_map(|ability| match ability {
            content::Ability::Trail { delay, period, magnitude, .. } => {
                Some((*delay, *period, *magnitude))
            }
            content::Ability::LinkedForms { .. } => None,
        })
        .expect("Wake authors a Trail");
    println!(
        "  Wake's Trail: a deposit every {} steps, coming due {} steps later, carrying {} units",
        trail.1,
        trail.0,
        trail.2 / ONE_UNIT,
    );

    assert!(wake.transition_step.is_some(), "Wake completes the chapter from over the ring");
    assert_eq!(wake.stalled, 0, "and carries the ring through every break while it stands there");
    // The same standing under a Form that authors no Trail: the ring stops and
    // stays stopped, because nothing is arriving at all. It is not a run that is
    // lost — every other test here drives a Thread that completes the chapter by
    // standing in the bright band instead — it is the measurement that says the
    // Trail is what carried Wake.
    assert!(
        thread.stalled > wake.stalled,
        "Wake's Trail carried no more of the breaks than the same standing did: \
         {} against {}",
        wake.stalled,
        thread.stalled,
    );
    assert!(
        thread.transition_step.is_none(),
        "a Form with no Trail completed the challenge from a place that gathers nothing",
    );
}

#[test]
fn a_second_line_laid_by_hand_is_a_reach_the_forms_do_not_share() {
    // The chapter's third way into the ring, and the one the authored content
    // does not hand out: Still Mode, one connect from the deep store straight
    // into the store, one Impulse. The pair stands 656 units apart — 144 across
    // the plane and one layer, priced at 512 — so **which Forms can lay it is a
    // reading of `route_reach`**, which is otherwise a constraint on authoring
    // rather than a difference a player meets.
    //
    // It is also the chapter's one exercise of the **forecast overlay**: the
    // visit assembles the slate and evaluates it, and the standing candidate's
    // baseline replays leave the envelope the paused surface draws.
    let content = authored();
    let chapter = chapter();
    let apart = {
        let place = |node: u32| -> (i64, i64, u8) {
            let port = chapter.ports.iter().find(|held| held.node == node).expect("a placed Node");
            (port.pos.x, port.pos.y, port.layer)
        };
        let (from_x, from_y, from_layer) = place(6);
        let (to_x, to_y, to_layer) = place(2);
        let dx = (from_x - to_x) as i128;
        let dy = (from_y - to_y) as i128;
        let planar = field_game_core::fx::isqrt((dx * dx + dy * dy) as u128) as i64;
        planar + i64::from(from_layer.abs_diff(to_layer)) * 512 * ONE_UNIT
    };
    println!("\nThe Echo — the second line, laid by hand: {} units apart", apart / ONE_UNIT);
    println!("{:<8} {:>7} {:>9} {:>8} {:>8} {:>8}", "form", "reach", "admits", "routes", "stalled", "steps");

    let mut admitted = 0;
    let mut turned_away = 0;
    for form in field_game_core::run::FORMS {
        let reach = content.form(form).expect("an authored Form").route_reach;
        if reach < apart {
            // One Form whose reach does not stretch that far, driven and pinned:
            // the queue refuses the entry, and the refusal names the reason.
            let (run, refused) = drive_with_line(form);
            let refused = refused.unwrap_or_else(|| {
                panic!("{form} reaches {} units and laid a {} unit Route", reach / ONE_UNIT, apart / ONE_UNIT)
            });
            assert!(refused.contains("reach"), "{form}: the refusal names its reason: {refused}");
            println!(
                "{form:<8} {:>7} {:>9} {:>8} {:>8} {:>8}",
                reach / ONE_UNIT,
                "no",
                run.routes,
                run.stalled,
                chapter_span(&run),
            );
            assert_eq!(run.routes, chapter.routes.len(), "{form}: no Route was laid");
            turned_away += 1;
            continue;
        }
        let (run, refused) = drive_with_line(form);
        assert!(refused.is_none(), "{form}: the connect was refused: {refused:?}");
        println!(
            "{form:<8} {:>7} {:>9} {:>8} {:>8} {:>8}",
            reach / ONE_UNIT,
            "yes",
            run.routes,
            run.stalled,
            chapter_span(&run),
        );
        assert_eq!(
            run.routes,
            chapter.routes.len() + 1,
            "{form}: the hand-laid Route stands beside the authored ones",
        );
        assert!(run.transition_step.is_some(), "{form}: the chapter completes on this line");
        assert_eq!(
            run.stalled, 0,
            "{form}: the hand-laid line did not carry the ring through the breaks",
        );
        // The optional test was let go: what carried the finale is the Route the
        // player drew and nothing the chapter handed over.
        assert!(!run.spare_open, "{form}: the authored test was taken as well");
        // And the overlay stood while the line was laid.
        let (envelope, deficient) =
            run.forecast.expect("{form}: the run never entered Still Mode");
        println!("         forecast envelope {envelope} steps, slate deficient {deficient}");
        assert!(!deficient, "{form}: the slate offered no alternative to compare against");
        assert!(envelope > 0, "{form}: the forecast overlay carried no envelope at all");
        admitted += 1;
    }
    assert_eq!(admitted, 2, "two of the eight Forms reach far enough to lay it");
    assert_eq!(turned_away, 6, "and six do not");
}

#[test]
fn the_chapters_one_pressure_stands_inside_the_challenge_on_all_four_paths() {
    // A pressure counts from the chapter's own opening and an objective is
    // offered when a player reaches it, so the only honest way to place one is
    // to measure the span every path stands inside and put it there. The four
    // paths are two allowance settings by two answers to the optional test, and
    // the binding pair is the earliest completion against the latest offer.
    let chapter = chapter();
    let entry = chapter.pressure_schedule.first().expect("the chapter schedules one pressure");
    assert_eq!(entry.pressure, field_game_core::pressure::Pressure::Noise);
    assert_eq!(
        entry.target.kind,
        field_game_core::pressure::TargetKind::Layer,
        "aimed at a layer, which is the target the Noise rule reads",
    );
    assert_eq!(entry.target.id, Some(0), "the layer the ring stands on");
    // Noise is the one drawing rule, and it narrows Route flow: it adds its
    // effective level to the layer's own authored noise and every Route whose
    // tail stands on that layer moves under the narrowed capacity. A layer that
    // authors no noise still draws under a Noise pressure, but the chapter
    // authors one anyway, so the rule runs every step of the chapter and the
    // pressure raises what is already there rather than switching it on.
    let chapter_layer = chapter.layers.iter().find(|held| held.layer == 0).expect("layer 0");
    assert!(
        chapter_layer.noise > 0,
        "the targeted layer authors a base noise, so the drawing rule runs whether or not \
         the pressure stands",
    );
    // And the reading only means anything where flows saturate: the Route into
    // the ring and its long side run at capacity whenever the store holds anything,
    // which is the condition the tripwire names and the reason this chapter is
    // where the campaign's Noise pressure belongs.
    let into_ring =
        chapter.routes.iter().find(|held| held.route == 4).expect("the Route into the ring");
    let store = chapter.ports.iter().find(|held| held.node == 2).expect("the store");
    assert!(
        store.capacity > into_ring.capacity * 64,
        "the store outlasts that Route by a wide margin, which is what makes it saturate",
    );
    let content = authored();
    let table = content.pressures.table(entry.pressure).expect("the manifest authors the table");
    let length: i64 = table.stages.iter().map(|stage| stage.steps).sum();

    let held = "objective.the_echo.hold_the_ring";
    println!("\nThe Echo — where the one pressure stands, path by path");
    println!(
        "  scheduled at {} for {length} steps, chapter-relative",
        entry.start_step,
    );
    let mut latest_offer = 0i64;
    let mut earliest_completion = i64::MAX;
    for (named, run) in [
        ("floor, test taken", floor()),
        ("floor, test let go", withheld()),
        ("attentive, test taken", attentive()),
        ("attentive, test let go", drive_as("thread", true, false, Stance::Light)),
    ] {
        let opened = run.entered_step.expect("the chapter opened");
        let offered = i64::from(run.offered_at(held).expect("the beat stood") - opened);
        let completed = i64::from(run.at(held).expect("and completed") - opened);
        println!("  {named:<24} offered {offered:>7}, completed {completed:>7}");
        latest_offer = latest_offer.max(offered);
        earliest_completion = earliest_completion.min(completed);
    }
    let start = i64::from(entry.start_step);
    println!(
        "  the binding window is {latest_offer} to {earliest_completion}, \
         and the pressure runs {start} to {}",
        start + length,
    );
    // The binding pair, which is the whole of the correction the four paths are
    // driven for: the latest a path offers the beat against the earliest a path
    // completes it. A window read off one path alone would be wrong by the
    // difference between them.
    assert!(
        latest_offer < start,
        "the pressure starts at {start}, before the latest path offers the beat at {latest_offer}",
    );
    assert!(
        start + length < earliest_completion,
        "the pressure ends at {}, after the earliest path completes the beat at \
         {earliest_completion}",
        start + length,
    );
    // And it stands well inside on both sides rather than only just inside.
    assert!(
        start - latest_offer > 2_000 && earliest_completion - (start + length) > 2_000,
        "the pressure stands {} steps past the latest offer and {} steps short of the \
         earliest completion, which is too little margin to re-tune a phase against",
        start - latest_offer,
        earliest_completion - (start + length),
    );

    // Everything above is arithmetic over the authored schedule and the steps
    // the objectives were offered at. What follows is the pressure **observed
    // standing** in a driven run, which is the thing the arithmetic is a claim
    // about: a schedule entry the limit never seated, or one seated at another
    // step, would read exactly the same above and differently here.
    let run = floor();
    let opened = run.entered_step.expect("the chapter opened");
    assert!(
        !run.pressed.is_empty(),
        "the chapter's one pressure was never observed standing at all — the schedule was \
         read and nothing was seated",
    );
    let first = i64::from(run.pressed.first().copied().expect("a first reading") - opened);
    let last = i64::from(run.pressed.last().copied().expect("a last reading") - opened);
    println!("  observed standing from {first} to {last}, sampled once every {BATCH} steps");
    let batch = i64::from(BATCH);
    // The sampler reads the list once a batch, so the observed window is the
    // authored one to within one batch at each end. It is banded rather than
    // pinned, because the batch the reading falls in is the driver's and not
    // the chapter's.
    assert!(
        (start..start + batch).contains(&first),
        "the pressure was first seen standing at {first}, not inside the batch its own \
         admission step ({start}) falls in",
    );
    assert!(
        (start + length - batch..start + length + batch).contains(&last),
        "the pressure was last seen standing at {last}, not within one batch of the step \
         its own table spends it on ({})",
        start + length,
    );
    // One pressure, standing once: the run never saw it leave and come back.
    let breaks = run
        .pressed
        .windows(2)
        .filter(|pair| i64::from(pair[1] - pair[0]) > batch)
        .count();
    assert_eq!(breaks, 0, "the pressure stood in one unbroken span, not {}", breaks + 1);

    // And it has teeth. An inert pressure is worse than none: it runs its whole
    // schedule, draws a line on the surface, and changes nothing a player could
    // read. Noise narrows Route flow, so what says it bit is the ring's own
    // narrowest Route into it carrying less while it stood than it carries with the same
    // store behind it and the pressure gone.
    let carried = |pressed: bool| -> Vec<i64> {
        run.ring_route
            .iter()
            .filter(|(_, held, _, store)| *held == pressed && *store > 0)
            .map(|(_, _, flow, _)| *flow)
            .collect()
    };
    let under = carried(true);
    let clear = carried(false);
    assert!(!under.is_empty() && !clear.is_empty(), "both halves of the beat were sampled");
    let least = |held: &[i64]| held.iter().copied().min().unwrap_or(0);
    println!(
        "  the Route into the ring, raw: least {} under the pressure, least {} clear of it, \
         over {} \
         and {} samples",
        least(&under),
        least(&clear),
        under.len(),
        clear.len(),
    );
    assert!(
        least(&under) < least(&clear),
        "the Route into the ring carried no less under the pressure than clear of it — the \
         pressure \
         runs its whole schedule and narrows nothing",
    );
}

// ---------------------------------------------------------------------------
// The optional test, and the recoverable setback
// ---------------------------------------------------------------------------

#[test]
fn the_optional_test_is_a_test_of_foresight_that_is_passable_and_skippable() {
    let chapter = chapter();
    let tests: Vec<&content::Objective> =
        chapter.objectives.iter().filter(|held| held.optional.is_some()).collect();
    assert_eq!(tests.len(), 1, "the chapter authors one optional test");
    let test = tests[0].id.clone();
    // It asks for a Port on the deeper layer, which no Pulse from the surface
    // can reach: the locked reach tops out under 256 units and the distance
    // rule adds 512 for the layer.
    assert!(matches!(tests[0].condition, content::Condition::PortsOpen { .. }));
    // And it is a Port rather than a band or a place, which is what keeps a run
    // that stands still from passing it by accident.
    assert!(
        !matches!(
            tests[0].condition,
            content::Condition::InCurrent { .. } | content::Condition::HoldPosition { .. }
        ),
        "an optional test a standing Form meets is one no player ever chooses",
    );

    let taken = floor();
    assert!(taken.spare_open, "the test is passable");
    assert!(taken.completed.contains(&test), "and a taken test is a completed objective");
    assert!(taken.transition_step.is_some(), "and the chapter completes");

    let let_go = withheld();
    assert!(!let_go.spare_open, "the test is skippable");
    assert!(!let_go.completed.contains(&test), "a test that was not taken was not completed");
    assert!(let_go.transition_step.is_some(), "and the chapter completes without it");
    // The span it stood for is the authored one: the sequence installs what
    // follows it at `started_step + span`.
    let offered = let_go.offered_at(&test).expect("the test was offered");
    let after = let_go
        .offered_at("objective.the_echo.fill_before_the_echo")
        .expect("and the objective after it stands");
    assert_eq!(
        after,
        offered + tests[0].optional.expect("a span"),
        "the pass-over lands at the step the span names",
    );
    println!(
        "\nthe optional test: taken, the chapter runs {} steps; let go, {} steps",
        chapter_span(&taken),
        chapter_span(&let_go),
    );
}

#[test]
fn the_authored_setback_stands_and_the_chapter_carries_past_it() {
    // The chapter's recoverable setback, and the whole of what makes it
    // recoverable: the sequence stands in `failed_recoverable` while the near
    // Node carries more than its threshold, comes back to `active` once the
    // Charge has somewhere else to go, and completes past it. Nothing in the
    // closed state set is terminal, and the run never leaves `running`.
    let run = attentive();
    let opened = run.entered_step.expect("the chapter opened");
    let collapse = run.collapse_step.expect("the authored setback is reached");
    let recovery = run.recovery_step.expect("and the sequence stands again after it");
    assert!(recovery > collapse);
    let setback = run
        .stages
        .iter()
        .rposition(|stage| stage == "failed_recoverable")
        .expect("the setback stood");
    assert!(
        run.stages[setback + 1..].iter().any(|stage| stage == "active"),
        "the sequence stands active again after the last setback",
    );
    assert!(
        run.stages[setback + 1..].iter().any(|stage| stage == "complete"),
        "and completes past it",
    );
    let setbacks = run.stages.iter().filter(|stage| *stage == "failed_recoverable").count();
    println!(
        "\nthe chapter stood in a recoverable setback {setbacks} times, first at step {}",
        collapse - opened,
    );
    assert!(setbacks > 0);
    let minute = 60 * STEPS_PER_SECOND;
    assert!(
        collapse - opened <= 22 * minute,
        "the first recoverable setback is inside the chapter's first half, and was at {}",
        collapse - opened,
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
    println!("\nThe Echo — authored duration, by objective");
    for (id, target, span) in &spans {
        println!("  {id:<48} authored {target:>6}  spans {span:>6}");
    }
    println!("  {:<48} {:>15} {total:>12}", "in all", "");
    assert_eq!(spans.len(), 5, "five of the eight ask for time: {spans:?}");
    // The chapter's authored duration alone, before a step of travel or search:
    // a little over forty-three minutes of the fifty-five.
    assert!(
        total > 74_000 && total < 82_000,
        "the authored duration comes to {total} steps, outside the band the pacing was set in",
    );
    // Every objective that asks for time is an exact span of the `Frac`: the
    // authored count and the count the run keeps are the same number.
    for (id, target, span) in &spans {
        assert_eq!(target, span, "{id} authors a count the progress share cannot reach exactly");
    }
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
    let order = authored_order();
    for id in &order {
        assert!(run.offered_at(id).is_some(), "{id} was offered");
    }
    assert_eq!(run.completed, order);
    assert!(run.progressed());
}

#[test]
fn the_ring_is_reachable_under_every_authored_route_reach() {
    // The chapter's own reading of Goal 19's per-Form reach: every link the
    // required ring stands on is inside the shortest reach the eight Forms
    // author, so a chapter that later lets a player lay these links by hand
    // does not exclude a Form from its own ring. The two links that are not
    // part of the required ring are stated as well, so the exclusion is a
    // decision on the record rather than an accident of placement.
    let chapter = chapter();
    let content = authored();
    let shortest = content
        .forms
        .iter()
        .map(|form| form.route_reach)
        .min()
        .expect("eight Forms are authored");
    let place = |node: u32| -> (i64, i64, u8) {
        if let Some(port) = chapter.ports.iter().find(|held| held.node == node) {
            return (port.pos.x, port.pos.y, port.layer);
        }
        let form = chapter.forms.iter().find(|held| held.node == node).expect("a placed Node");
        (form.pos.x, form.pos.y, form.layer)
    };
    println!("\nThe Echo — every authored Route against the shortest authored reach");
    println!("  the shortest reach the eight Forms author: {} units", shortest / ONE_UNIT);
    let ring = [4u32, 5, 6];
    for route in &chapter.routes {
        let (tail_x, tail_y, tail_layer) = place(route.tail);
        let (head_x, head_y, head_layer) = place(route.head);
        let dx = (tail_x - head_x) as i128;
        let dy = (tail_y - head_y) as i128;
        let planar = field_game_core::fx::isqrt((dx * dx + dy * dy) as u128) as i64;
        let layers = i64::from(tail_layer.abs_diff(head_layer));
        let reach = planar + layers * 512 * ONE_UNIT;
        let inside = reach <= shortest;
        println!(
            "  route {:<2} {} to {}  {:>5} units{}  {}",
            route.route,
            route.tail,
            route.head,
            reach / ONE_UNIT,
            if layers > 0 { ", one layer apart" } else { "" },
            if inside { "inside every reach" } else { "outside the shortest" },
        );
        if ring.contains(&route.route) {
            assert!(
                inside,
                "route {} carries the required ring and stands {} units apart, past the \
                 shortest authored reach of {}",
                route.route,
                reach / ONE_UNIT,
                shortest / ONE_UNIT,
            );
        }
    }
}

#[test]
fn the_campaign_script_carries_this_chapter_with_its_optional_test_passed_over() {
    // The arm this chapter adds to `support::campaign::script_for`, driven the
    // way the campaign run drives one: phases with a stop at this chapter's own
    // boundary, no allowance spent, and the optional test let go. A driven
    // campaign passes over every test it is offered, so what this asserts is the
    // completed list the campaign-wide counts are derived from — every required
    // objective of this chapter and no test.
    let mut session = Session::new(&support::worker_init()).expect("versions agree");
    let opened = session.command(
        "init_run",
        &format!("{{\"form\":\"thread\",\"mode\":\"new\",\"run_id\":\"{KEY}\"}}"),
    );
    assert!(opened.contains("\"ok\":true"), "{opened}");
    let mut driver = support::campaign::Driver::new();
    for index in 0..CHAPTER {
        support::campaign::play_chapter(
            &mut session,
            &mut driver,
            authored().chapter(index as u8).expect("a chapter"),
        );
    }
    assert_eq!(
        support::campaign::chapter_index(&session) as i64,
        CHAPTER,
        "the run stands at the opening of this chapter",
    );
    let opened_at = session.run().expect("a run").state().now.step;

    let stop = |session: &Session| i64::from(support::campaign::chapter_index(session)) > CHAPTER;
    let script = support::campaign::script_for("the_echo").expect("this chapter answers with one");
    for phase in &script {
        if stop(&session) {
            break;
        }
        driver.phase_until(&mut session, phase, &stop);
    }
    assert!(stop(&session), "the chapter completes under the script the campaign run uses");

    let mut driven = fresh_record();
    driven.entered_step = Some(opened_at);
    record(&mut driven, std::mem::take(&mut driver.raised));
    assert_eq!(
        driven.completed,
        required_order(),
        "every required objective of this chapter completes, and no optional test",
    );
    let span = session.run().expect("a run").state().now.step - opened_at;
    println!(
        "\nThe Echo — the campaign's own script: {span} steps, {} minutes, test passed over",
        minutes(span),
    );
}
