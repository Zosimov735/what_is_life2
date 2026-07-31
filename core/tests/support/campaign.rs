//! Driving a whole campaign, chapter by chapter.
//!
//! **One chapter, one driver, chosen by the chapter's own id.** `script_for`
//! is the dispatch: a chapter that names a phase list there is played through
//! it, and a chapter that names none is driven by advancing steps with every
//! input neutral, bounded by the span its own objectives author. `play_chapter`
//! is what a caller uses, and it is the only thing a caller needs to know.
//!
//! The rest driver completes exactly the chapters a Form standing still
//! completes: a chapter that places the controlled Form inside its own bright
//! current and asks for time in it, which is what every placeholder is. That is
//! deliberate — what the campaign run tests is the runner, not the steering, and
//! a chapter that needs no steering tests the transition without the play in
//! front of it.
//!
//! **A chapter that asks for steering, a Pulse, a Port or depth needs its own
//! arm**, and adding one is the whole of the work: a `fn <id>() -> Vec<Phase>`
//! beside the others, and one line in `script_for`'s `match`. Both are
//! additions at the end of what stands, so several chapter agents appending at
//! once meet in an add/add hunk whose resolution is to keep every arm — no line
//! any of them wrote is moved or deleted. The phase lists are written out here
//! rather than shared with a chapter's own test file so the campaign run and a
//! chapter run can move apart without either dragging the other with it.

#![allow(dead_code)]

use field_game_core::content::{self, Chapter};
use field_game_core::json::{parse, Json};
use field_game_core::Session;

/// How many steps one driven batch runs before the script re-aims.
const BATCH: u16 = 15;

/// How many steps a full Pulse charge takes, from the locked charging rule.
const FULL_CHARGE_STEPS: u32 = 32;

/// What one phase asks of the run.
#[derive(Clone, Copy, Debug)]
pub enum Act {
    /// Steer toward a point, at the speed the distance to it names.
    Toward(i64, i64),
    /// Steer toward the nearest path point of a named current, read off the
    /// Field each batch. It is how a player follows a band that moves: the aim
    /// is where the band stands now, not where it was authored.
    Follow(u16),
    /// Hold every input neutral.
    Rest,
    /// Hold the Pulse, steering toward a point.
    Charge(i64, i64),
    /// Release the Pulse.
    Release,
    /// Ask for one layer deeper, or one layer back up.
    Depth(i8),
    /// Hand control to the Form the identifier names: a Still Mode visit that
    /// queues nothing, spends the two ramps of real time, and moves the
    /// `controlled` flag between steps.
    Hand(u8),
}

/// How long a ramp takes, in microseconds. Locked.
const RAMP_US: i64 = 250_000;

/// One phase: what it does, and for how many steps.
///
/// `plans` is empty for every phase that steps. A phase that carries plan
/// bodies is a queued-change phase instead: it enters Still Mode, queues them,
/// commits, and ramps back out, spending real time on the two ramps and no
/// simulated time at all. A chapter whose objectives ask for a Route the
/// player draws cannot be driven without one, and carrying it here rather than
/// as a variant of [`Act`] keeps the closed act set — which another chapter's
/// driver matches exhaustively — where it stands.
pub struct Phase {
    pub act: Act,
    pub steps: u32,
    pub plans: &'static [&'static str],
}

const fn play(act: Act, steps: u32) -> Phase {
    Phase { act, steps, plans: &[] }
}

/// A queued-change phase: the plan bodies one Still Mode visit commits.
const fn edit(plans: &'static [&'static str]) -> Phase {
    Phase { act: Act::Rest, steps: 0, plans }
}

/// The opening chapter, driven to its last objective. The positions are the
/// authored content's own: the bright current's waypoints, the point the hold
/// objective names, the five Ports, and the deep current the back half asks
/// for. The last phase follows the bright current rather than aiming at where
/// it was authored, because Drift moves it while the phase runs.
pub fn the_pull() -> Vec<Phase> {
    vec![
        play(Act::Toward(1000, 2032), 150),
        play(Act::Toward(1208, 2006), 260),
        play(Act::Toward(1416, 1993), 260),
        play(Act::Toward(1624, 1993), 300),
        play(Act::Toward(1832, 2006), 300),
        play(Act::Toward(2040, 2032), 300),
        play(Act::Toward(1520, 1992), 430),
        play(Act::Toward(1520, 1992), 2400),
        play(Act::Charge(1520, 1992), FULL_CHARGE_STEPS),
        play(Act::Release, 1),
        play(Act::Toward(2540, 1830), 400),
        play(Act::Charge(2540, 1830), FULL_CHARGE_STEPS),
        play(Act::Release, 1),
        play(Act::Toward(2880, 2040), 260),
        play(Act::Charge(2880, 2040), FULL_CHARGE_STEPS),
        play(Act::Release, 1),
        play(Act::Toward(2570, 2270), 260),
        play(Act::Charge(2570, 2270), FULL_CHARGE_STEPS),
        play(Act::Release, 1),
        play(Act::Toward(2700, 2100), 200),
        play(Act::Toward(3220, 2340), 300),
        play(Act::Charge(3220, 2340), FULL_CHARGE_STEPS),
        play(Act::Release, 1),
        play(Act::Toward(2700, 2100), 7600),
        // The back half: over the deep current, down a layer, and hold it.
        play(Act::Toward(1550, 2200), 900),
        play(Act::Depth(1), 1),
        play(Act::Follow(3), 1_100),
        // The optional test stands next, and this run lets it go: a driven
        // campaign passes over every test it is offered, which is what makes
        // the completed list the campaign's required objectives exactly.
        play(Act::Follow(3), 1_900),
        // Back up, and hold the bright current wherever Drift has taken it.
        play(Act::Depth(-1), 1),
        play(Act::Follow(1), 13_000),
    ]
}

/// The Edge, driven to its own boundary: the second chapter, and the first one
/// whose objectives cannot be met by standing still.
///
/// The positions are the authored content's own, in whole units: the bright
/// current the reserve stands in, the two outer stores a Pulse opens, and the
/// deep current the back half asks for. The run takes no queued change at all —
/// the chapter's three strategies are alternates rather than requirements, and
/// what is being driven here is the runner — and it lets the optional test go,
/// which is what keeps a driven campaign's completed list the campaign's
/// required objectives exactly.
pub fn the_edge() -> Vec<Phase> {
    vec![
        play(Act::Toward(2000, 2000), 800),
        play(Act::Follow(1), 4_400),
        play(Act::Toward(1620, 2000), 260),
        // The two outer stores, each opened by a Pulse released beside it.
        play(Act::Toward(2060, 1620), 800),
        play(Act::Charge(2060, 1620), FULL_CHARGE_STEPS),
        play(Act::Release, 1),
        play(Act::Toward(2470, 2000), 1_000),
        play(Act::Charge(2470, 2000), FULL_CHARGE_STEPS),
        play(Act::Release, 1),
        // Standing clear of the band, so the inside takes the whole of what the
        // current delivers, and holding while it does.
        play(Act::Toward(2500, 2200), 8_500),
        play(Act::Toward(1620, 2200), 11_300),
        // Depth, and the optional test let go.
        play(Act::Toward(2080, 2380), 1_300),
        play(Act::Depth(1), 1),
        play(Act::Follow(3), 4_400),
        play(Act::Follow(3), 5_700),
        play(Act::Depth(-1), 1),
        // The final challenge: three stops of the current that fills it.
        play(Act::Toward(1620, 2200), 34_000),
    ]
}

/// The Rewrite, driven to its own boundary at the authored floor.
///
/// The chapter opens with a run already standing and asks for it to be changed
/// while it keeps running. Three of its beats cannot be driven by steering
/// alone, and each of them is a Still Mode commit here: the two Nodes of the
/// dependency have nowhere to send what arrives until an outfall is drawn to
/// each of them; Fracture breaks the line from the Form to the intake and only
/// a Route drawn again supplies the run; and the module's own Route breaks in
/// the final challenge.
///
/// **The rewrite this script takes is the relocation** — the dependency's two
/// Routes keep their identifiers and hang on the standby store instead of on
/// the junction the module carried to. It is the two-change path, and it is the
/// one whose changes are legal whenever they are queued rather than only after
/// the break, which is what makes it the right one for a driver whose phases
/// are step counts rather than reactions. `core/tests/chapter_the_rewrite.rs`
/// drives all three paths and the timing they turn on.
///
/// **The optional test is let go**, as every arm of this dispatch lets its own
/// go: what keeps a driven campaign's completed list the campaign's required
/// objectives exactly.
///
/// Two step counts are load-bearing and are stated rather than tuned. The line
/// is drawn again only after Fracture has broken it — a `connect` of a Route
/// that still stands is refused as a duplicate — so every phase before that
/// commit sums past the pressure's own step. And the Route runs from the Form's
/// own Node, so the phase before it brings the Form back up to the surface and
/// into the band: the locked distance adds 512 units per layer, which no
/// authored `route_reach` covers.
pub fn the_rewrite() -> Vec<Phase> {
    vec![
        play(Act::Toward(1400, 1620), 200),
        play(Act::Follow(1), 17_000),
        // The two Nodes of the dependency, each opened by a Pulse beside it.
        play(Act::Toward(3056, 1900), 2_400),
        play(Act::Charge(3056, 1900), FULL_CHARGE_STEPS),
        play(Act::Release, 1),
        play(Act::Toward(3056, 2120), 600),
        play(Act::Charge(3056, 2120), FULL_CHARGE_STEPS),
        play(Act::Release, 1),
        play(Act::Toward(1960, 1620), 2_600),
        // The authored setback: nothing takes Charge out of the two Nodes, so
        // they carry more than they can hold until an outfall is drawn.
        play(Act::Toward(1960, 1620), 1_200),
        edit(&[
            "{\"plan\":{\"from\":6,\"op\":\"connect\",\"to\":8}}",
            "{\"plan\":{\"from\":7,\"op\":\"connect\",\"to\":8}}",
        ]),
        play(Act::Toward(1960, 1620), 23_500),
        // Depth, and the optional test let go.
        play(Act::Toward(2600, 2340), 1_800),
        play(Act::Depth(1), 1),
        play(Act::Follow(2), 23_000),
        play(Act::Follow(2), 8_300),
        play(Act::Depth(-1), 1),
        // Back to the surface and into the band, where the line to the intake
        // stands inside every Form's reach. This phase runs past Fracture's own
        // step, which is what makes the commit after it legal.
        play(Act::Toward(1960, 1620), 6_000),
        edit(&["{\"plan\":{\"from\":1,\"op\":\"connect\",\"to\":2}}"]),
        play(Act::Toward(1960, 1620), 1_200),
        // The relocation: the dependency hangs on the standby, which stands
        // full, so the module's own break no longer reaches it.
        edit(&[
            "{\"plan\":{\"end\":\"tail\",\"op\":\"redirect\",\"route\":6,\"to\":11}}",
            "{\"plan\":{\"end\":\"tail\",\"op\":\"redirect\",\"route\":7,\"to\":11}}",
        ]),
        play(Act::Toward(1960, 1620), 80_000),
    ]
}

/// The script one chapter is driven by, and none for a chapter a resting run
/// completes on its own.
///
/// This is the wave's own dispatch: each chapter agent adds its chapter's phase
/// list above and one arm here, and nothing else in the shared driver moves.
/// A placeholder chapter has no arm and is driven by advancing steps alone,
/// which is what the placeholders were authored for.
pub fn script_for(chapter: &str) -> Option<Vec<Phase>> {
    match chapter {
        "the_pull" => Some(the_pull()),
        "the_edge" => Some(the_edge()),
        "the_echo" => Some(the_echo()),
        "the_loop" => Some(the_loop()),
        "the_mesh" => Some(the_mesh()),

        "the_break" => Some(the_break()),
        "the_rewrite" => Some(the_rewrite()),
        "the_quiet_edge" => Some(the_quiet_edge()),
        _ => None,
    }
}

/// One driven session: the frame counter it stands at, and every event it has
/// raised, in the order they were raised.
pub struct Driver {
    pub seq: u32,
    pub raised: Vec<(String, u32, Json)>,
}

impl Driver {
    pub fn new() -> Self {
        Driver { seq: 1, raised: Vec::new() }
    }

    /// Runs one phase, re-aiming every batch, and stopping early when the
    /// caller's own reading of the session says to.
    ///
    /// The stop is what keeps a driven chapter inside its own chapter: the
    /// phases are written to overshoot a little, so that a run which reaches an
    /// objective late still reaches it, and a run that reaches it early would
    /// otherwise spend the overshoot in the chapter after it.
    pub fn phase_until(
        &mut self,
        session: &mut Session,
        phase: &Phase,
        stop: &dyn Fn(&Session) -> bool,
    ) {
        // A queued-change phase is not a stepping phase: it spends real time on
        // the two ramps and no simulated time at all, so it runs whole or not
        // at all.
        if !phase.plans.is_empty() {
            if !stop(session) {
                self.commit_edit(session, phase.plans);
            }
            return;
        }
        // A Handoff is not a stepping phase either: it is answered in Still
        // Mode, where no step runs, so it spends the two ramps and no
        // simulated time at all.
        if let Act::Hand(id) = phase.act {
            if !stop(session) {
                self.hand_control(session, id);
            }
            return;
        }
        let mut left = phase.steps;
        while left > 0 {
            if stop(session) {
                return;
            }
            let form = controlled(session);
            let (steer, held, release, depth) = match phase.act {
                Act::Toward(x, y) => (toward(form, x, y), false, false, 0),
                Act::Follow(current) => {
                    let (x, y) = nearest_point(session, current).unwrap_or((0, 0));
                    (toward(form, x, y), false, false, 0)
                }
                Act::Rest => ((0, 0), false, false, 0),
                Act::Charge(x, y) => (toward(form, x, y), true, false, 0),
                Act::Release => ((0, 0), false, true, 0),
                Act::Depth(way) => ((0, 0), false, false, way),
                // Answered above: a Handoff spends no simulated time.
                Act::Hand(_) => ((0, 0), false, false, 0),
            };
            let now = left.min(u32::from(BATCH)) as u16;
            self.depth_frame(session, now, steer, held, release, depth);
            left -= u32::from(now);
        }
    }

    /// Advances a number of steps with every input neutral.
    pub fn rest(&mut self, session: &mut Session, steps: u32) {
        self.rest_until(session, steps, &|_| false);
    }

    /// The same, stopping early when the caller's reading says to.
    pub fn rest_until(
        &mut self,
        session: &mut Session,
        steps: u32,
        stop: &dyn Fn(&Session) -> bool,
    ) {
        let mut left = steps;
        while left > 0 {
            if stop(session) {
                return;
            }
            let now = left.min(u32::from(BATCH)) as u16;
            self.frame(session, now, (0, 0), false, false);
            left -= u32::from(now);
        }
    }

    /// Sends one frame and drains the events it raised.
    pub fn frame(
        &mut self,
        session: &mut Session,
        steps: u16,
        steer: (i64, i64),
        held: bool,
        release: bool,
    ) {
        self.depth_frame(session, steps, steer, held, release, 0);
    }

    /// The same, carrying a depth press: −1 one layer up, +1 one layer down.
    pub fn depth_frame(
        &mut self,
        session: &mut Session,
        steps: u16,
        steer: (i64, i64),
        held: bool,
        release: bool,
        depth_key: i8,
    ) {
        let body = format!(
            "{{\"advance_steps\":{steps},\"depth_key\":{depth_key},\"inspect\":null,\
             \"pause\":false,\"pulse_held\":{held},\"pulse_release\":{release},\"seq\":{},\
             \"steer_x\":{},\"steer_y\":{},\"t_us\":0,\"toggle_still\":false,\"wheel\":0}}",
            self.seq, steer.0, steer.1,
        );
        let answer = session.command("input_frame", &body);
        assert!(answer.contains("\"ok\":true"), "{answer}");
        self.seq += 1;
        self.drain(session);
    }

    /// The same, carrying a timestamp and the Still Mode toggle.
    ///
    /// Every other frame this driver sends carries `t_us` 0, so the gap the
    /// accumulator reads is clamped to zero and a ramp stands still; the frames
    /// an edit sends carry their own increasing stamps, which is the only real
    /// time a driven run ever spends.
    pub fn stamped_frame(
        &mut self,
        session: &mut Session,
        steps: u16,
        t_us: i64,
        toggle_still: bool,
    ) {
        let body = format!(
            "{{\"advance_steps\":{steps},\"depth_key\":0,\"inspect\":null,\"pause\":false,\
             \"pulse_held\":false,\"pulse_release\":false,\"seq\":{},\"steer_x\":0,\
             \"steer_y\":0,\"t_us\":{t_us},\"toggle_still\":{toggle_still},\"wheel\":0}}",
            self.seq,
        );
        let answer = session.command("input_frame", &body);
        assert!(answer.contains("\"ok\":true"), "{answer}");
        self.seq += 1;
        self.drain(session);
    }

    /// Enters Still Mode, hands control to one Form, and ramps back out.
    ///
    /// The request rides the frame's `inspect` field, which is where
    /// ARCHITECTURE.md's Handoff puts it: still-mode only, immediate, no queue
    /// entry and no Impulse.
    pub fn hand_control(&mut self, session: &mut Session, id: u8) {
        let opened = i64::from(self.seq) * 1_000_000 + 10_000_000;
        // No step at all, on any frame of the visit: a Handoff is answered
        // between steps and spends the two ramps of real time and nothing else.
        self.stamped_frame(session, 0, opened, true);
        assert_eq!(session.lifecycle(), "ramp_in");
        self.stamped_frame(session, 0, opened + RAMP_US, false);
        assert_eq!(session.lifecycle(), "still", "the ramp completes into Still Mode");
        let body = format!(
            "{{\"advance_steps\":0,\"depth_key\":0,\"inspect\":{{\"kind\":null,\
             \"parameter\":{id},\"target\":\"handoff\"}},\"pause\":false,\
             \"pulse_held\":false,\"pulse_release\":false,\"seq\":{},\"steer_x\":0,\
             \"steer_y\":0,\"t_us\":{},\"toggle_still\":false,\"wheel\":0}}",
            self.seq,
            opened + RAMP_US + 1,
        );
        let answer = session.command("input_frame", &body);
        assert!(answer.contains("\"ok\":true"), "handing control to {id}: {answer}");
        self.seq += 1;
        self.drain(session);
        self.stamped_frame(session, 0, opened + RAMP_US + 2, true);
        self.stamped_frame(session, 0, opened + 2 * RAMP_US + 2, false);
        assert_eq!(session.lifecycle(), "running", "the exit completes");
    }

    /// Enters Still Mode, queues every plan body, commits, and ramps back out.
    pub fn commit_edit(&mut self, session: &mut Session, plans: &[&str]) {
        let opened = i64::from(self.seq) * 1_000_000 + 10_000_000;
        self.stamped_frame(session, 1, opened, true);
        assert_eq!(session.lifecycle(), "ramp_in");
        self.stamped_frame(session, 0, opened + RAMP_US, false);
        assert_eq!(session.lifecycle(), "still", "the ramp completes into Still Mode");
        for plan in plans {
            let answer = session.command("queue_plan", plan);
            assert!(answer.contains("\"ok\":true"), "queueing {plan}: {answer}");
        }
        let answer = session.command("commit_plan", "{}");
        assert!(answer.contains("\"ok\":true"), "committing: {answer}");
        self.stamped_frame(session, 0, opened + 2 * RAMP_US, false);
        assert_eq!(session.lifecycle(), "running", "the committed exit completes");
    }

    /// Takes what the session has raised since the last call.
    pub fn drain(&mut self, session: &mut Session) {
        for event in raised(session) {
            self.raised.push(event);
        }
    }

    /// Every event of one name, in order.
    pub fn of(&self, name: &str) -> Vec<&(String, u32, Json)> {
        self.raised.iter().filter(|(held, _, _)| held == name).collect()
    }
}

impl Default for Driver {
    fn default() -> Self {
        Self::new()
    }
}

/// The chapter the run stands in.
pub fn chapter_index(session: &Session) -> u8 {
    session.run().expect("a run").state().progress.chapter_index
}

/// The phase list one chapter is driven by, and none for a chapter the rest
/// driver completes.
///
/// How long one chapter driven by the rest driver asks for, in steps, plus the
/// slack a boundary costs: every required objective's own span, and every
/// optional test's span, because a rest-driven run takes none of the tests and
/// every one of them is passed over.
pub fn span_of(chapter: &Chapter) -> u32 {
    chapter
        .objectives
        .iter()
        .map(|held| match held.optional {
            Some(span) => span,
            None => content::effective_steps(&held.condition) as u32,
        })
        .sum::<u32>()
        + 60
}

/// Plays one chapter to its own boundary and no further, by whichever driver
/// its id asks for: its own scripted phase list, or the rest driver bounded by
/// the span the chapter authors.
///
/// The run stops the moment it stands in the chapter after this one, or the
/// moment the campaign ends on it, so a phase list written to overshoot a
/// little never spends the overshoot in the chapter that follows.
pub fn play_chapter(session: &mut Session, driver: &mut Driver, chapter: &Chapter) {
    let standing = chapter_index(session);
    let stop = move |session: &Session| {
        chapter_index(session) > standing || session.lifecycle() == "ended"
    };
    match script_for(&chapter.id) {
        Some(phases) => {
            for phase in phases {
                if stop(session) {
                    break;
                }
                driver.phase_until(session, &phase, &stop);
            }
            assert!(stop(session), "{} completes under its own driven script", chapter.id);
        }
        None => {
            driver.rest_until(session, span_of(chapter), &stop);
            assert!(stop(session), "{} completes inside the span it authors", chapter.id);
        }
    }
}

/// Advances until the run stands in the chapter after the one it is in, or
/// until the campaign ends. `cap` bounds how long it will wait.
///
/// The rest driver on its own, for a caller that is resuming part way through a
/// chapter rather than playing one from its opening.
pub fn carry_on(session: &mut Session, driver: &mut Driver, cap: u32) {
    let standing = chapter_index(session);
    let stop = move |session: &Session| {
        chapter_index(session) > standing || session.lifecycle() == "ended"
    };
    driver.rest_until(session, cap, &stop);
    assert!(stop(session), "the chapter completes inside the span it authors");
}

/// The path point of one current standing nearest the controlled Form, in
/// whole units, and none when the Field holds no such current.
///
/// It is read off the Field rather than off the authored file, which is what
/// makes a driven run able to follow a band Drift has moved: the aim is the
/// band's own geometry as it stands this step.
pub fn nearest_point(session: &Session, current: u16) -> Option<(i64, i64)> {
    use field_game_core::fx::ONE_UNIT;
    let at = controlled(session);
    let state = session.run().expect("a run").state();
    let held = state.now.currents.iter().find(|held| held.id == current)?;
    held.path
        .iter()
        .min_by_key(|point| {
            let dx = i128::from(point.x - at.x);
            let dy = i128::from(point.y - at.y);
            dx * dx + dy * dy
        })
        .map(|point| (point.x / ONE_UNIT, point.y / ONE_UNIT))
}

/// Where the controlled Form stands, raw.
pub fn controlled(session: &Session) -> field_game_core::fx::Vec2 {
    session
        .run()
        .expect("a run")
        .state()
        .now
        .forms
        .iter()
        .find(|form| form.controlled)
        .expect("the controlled Form")
        .pos
}

/// The events a session has raised, parsed.
pub fn raised(session: &mut Session) -> Vec<(String, u32, Json)> {
    let text = session.take_events();
    let parsed = parse(&text).expect("canonical events");
    let Json::List(entries) = parsed else {
        return Vec::new();
    };
    entries
        .into_iter()
        .map(|entry| {
            let name = entry.get("ev").and_then(Json::as_text).expect("a name").to_string();
            let step = entry.get("step").and_then(Json::as_int).expect("a step") as u32;
            let body = entry.get("body").cloned().expect("a body");
            (name, step, body)
        })
        .collect()
}

/// The steering that aims at a point in whole units, at the speed the distance
/// to it names — the shell's own normalization, so a driven frame carries what
/// a pointer would.
pub fn toward(from: field_game_core::fx::Vec2, x: i64, y: i64) -> (i64, i64) {
    use field_game_core::fx::{isqrt, ONE_UNIT};
    /// The reach a fully deflected control names, in units.
    const STEER_REACH_UNITS: i64 = 320;
    /// The largest per-axis value of the normalized Q1.15 control.
    const STEER_LIMIT: i64 = 32_767;
    let dx = x * ONE_UNIT - from.x;
    let dy = y * ONE_UNIT - from.y;
    let span =
        isqrt((i128::from(dx) * i128::from(dx) + i128::from(dy) * i128::from(dy)) as u128) as i64;
    if span == 0 {
        return (0, 0);
    }
    let reach = STEER_REACH_UNITS * ONE_UNIT;
    let magnitude = (STEER_LIMIT * span / reach).min(STEER_LIMIT);
    let mut steer_x = dx * magnitude / span;
    let mut steer_y = dy * magnitude / span;
    // The locked magnitude cap is normative, and the reader that enforces it
    // refuses a frame that crosses it by a single raw unit.
    while steer_x * steer_x + steer_y * steer_y > STEER_LIMIT * STEER_LIMIT {
        steer_x -= steer_x.signum();
        steer_y -= steer_y.signum();
    }
    (steer_x, steer_y)
}

// ---------------------------------------------------------------------------
// The Echo — chapter 4
// ---------------------------------------------------------------------------

/// The Echo, driven to its own boundary at the authored floor.
///
/// The chapter is a store beyond the bright band, a ring that turns on what the
/// store holds, and three stretches where the bright band stops and the layer's
/// loss is raised while the ring is being held. The script gathers, opens the
/// ring's two Ports, opens the Port beside the Node that carries more than it
/// can hold, holds the ring through the first stop, takes the depth, lets the
/// optional test's span run out, comes back up, fills the store again, and
/// holds the ring through the three stops of the final challenge.
///
/// **Letting the test go is deliberate.** A driven campaign passes over every
/// test it is offered, which is what makes the completed list the campaign's
/// required objectives exactly. The Echo's one optional test names a Port, so a
/// Form that stands still never meets it by accident — but the pass-over here is
/// this script's own choice rather than a property of the condition, and the
/// final challenge is about ten thousand steps longer for it. The phases are
/// sized for that longer path.
pub fn the_echo() -> Vec<Phase> {
    vec![
        play(Act::Follow(1), 11_100),
        play(Act::Toward(1900, 1990), 900),
        play(Act::Charge(1900, 1990), FULL_CHARGE_STEPS),
        play(Act::Release, 1),
        play(Act::Toward(2010, 2160), 400),
        play(Act::Charge(2010, 2160), FULL_CHARGE_STEPS),
        play(Act::Release, 1),
        play(Act::Toward(2010, 2160), 200),
        play(Act::Toward(2100, 1880), 400),
        play(Act::Charge(2100, 1880), FULL_CHARGE_STEPS),
        play(Act::Release, 1),
        play(Act::Follow(1), 700),
        play(Act::Follow(1), 20_500),
        play(Act::Toward(1700, 2180), 900),
        play(Act::Depth(1), 1),
        play(Act::Follow(2), 16_700),
        // The optional test stands here, and this run lets it go.
        play(Act::Follow(2), 5_600),
        play(Act::Depth(-1), 1),
        play(Act::Follow(1), 700),
        play(Act::Follow(1), 17_000),
        play(Act::Follow(1), 40_000),
    ]
}

/// The Loop, driven to its own boundary at the authored floor.
///
/// The chapter is not rest-completable and cannot be: nothing on its field
/// gives the run Charge, so the supply Route and the closing Route are both
/// changes the player queues in Still Mode, and the four Ports of the run are
/// opened by Pulses. The layout below is the near store — the one every Form
/// reaches — and the optional spare junction is let go rather than taken, so a
/// driven campaign passes over every test it is offered.
///
/// `core/tests/chapter_the_loop.rs` drives the same chapter with the modelled
/// first-run allowances and through four route layouts; this is the floor.
pub fn the_loop() -> Vec<Phase> {
    vec![
        play(Act::Toward(1400, 1500), 240),
        play(Act::Follow(1), 5600),
        play(Act::Toward(1800, 2000), 480),
        play(Act::Charge(1800, 2000), FULL_CHARGE_STEPS),
        play(Act::Release, 1),
        play(Act::Toward(2120, 1920), 360),
        play(Act::Charge(2120, 1920), FULL_CHARGE_STEPS),
        play(Act::Release, 1),
        play(Act::Toward(2440, 2000), 360),
        play(Act::Charge(2440, 2000), FULL_CHARGE_STEPS),
        play(Act::Release, 1),
        play(Act::Toward(2620, 2280), 360),
        play(Act::Charge(2620, 2280), FULL_CHARGE_STEPS),
        play(Act::Release, 1),
        play(Act::Toward(1800, 2000), 480),
        edit(&["{\"plan\":{\"from\":1,\"op\":\"connect\",\"to\":3}}"]),
        play(Act::Toward(1800, 1500), 420),
        play(Act::Follow(1), 600),
        play(Act::Toward(2280, 2360), 720),
        play(Act::Charge(2280, 2360), FULL_CHARGE_STEPS),
        play(Act::Release, 1),
        edit(&["{\"plan\":{\"from\":5,\"op\":\"connect\",\"to\":6}}"]),
        play(Act::Toward(2100, 1500), 780),
        play(Act::Follow(1), 14_200),
        // The spare junction stands for its span and is let go.
        play(Act::Follow(1), 5600),
        play(Act::Follow(1), 17_600),
        // The authored event closes one Port of the run; only a Pulse reopens it.
        play(Act::Toward(2440, 2000), 720),
        play(Act::Charge(2440, 2000), FULL_CHARGE_STEPS),
        play(Act::Release, 1),
        play(Act::Toward(2300, 1500), 720),
        play(Act::Follow(1), 14_200),
        play(Act::Follow(1), 24_000),
    ]
}


/// The Mesh, driven to its own boundary at the authored floor.
///
/// The chapter places three Forms and opens control on one of them. The two
/// Ports its opening asks for stand beside the other two, so the script hands
/// control on rather than steering the length of the field twice — which is
/// what the chapter is about. The southern pattern is drawn from the spare
/// intake rather than from the main one: both leave it flowing, and this is the
/// layout the driven campaign takes. The optional deep Port is let go, which is
/// what keeps a driven campaign's completed list the campaign's required
/// objectives exactly.
pub fn the_mesh() -> Vec<Phase> {
    vec![
        play(Act::Follow(1), 5_600),
        play(Act::Hand(2), 0),
        play(Act::Charge(1450, 1300), FULL_CHARGE_STEPS),
        play(Act::Release, 1),
        play(Act::Hand(3), 0),
        play(Act::Charge(1750, 2500), FULL_CHARGE_STEPS),
        play(Act::Release, 1),
        play(Act::Hand(1), 0),
        play(Act::Follow(1), 34_000),
        play(Act::Toward(2050, 2000), 600),
        play(Act::Charge(2050, 2000), FULL_CHARGE_STEPS),
        play(Act::Release, 1),
        edit(&["{\"plan\":{\"from\":5,\"op\":\"connect\",\"to\":8}}"]),
        play(Act::Follow(1), 120),
        play(Act::Toward(1700, 2400), 700),
        play(Act::Depth(1), 1),
        play(Act::Follow(2), 2_800),
        // The optional deep Port stands for its span and is let go.
        play(Act::Follow(2), 5_600),
        play(Act::Depth(-1), 1),
        play(Act::Follow(1), 9_200),
        // The authored event closes the spare intake, and the southern pattern
        // stops with it. A Pulse released beside it is the one thing that
        // opens it again.
        play(Act::Toward(2050, 2000), 700),
        play(Act::Charge(2050, 2000), FULL_CHARGE_STEPS),
        play(Act::Release, 1),
        play(Act::Follow(1), 60_000),
        play(Act::Follow(1), 20_000),
    ]
}

// ---------------------------------------------------------------------------
// The Break — chapter 6
// ---------------------------------------------------------------------------

/// The Break, driven to its own boundary at the authored floor.
///
/// The chapter cannot be rest-completed and cannot be driven without Still
/// Mode: nothing carries Charge from the intake into the run until the player
/// draws a Route, the four Ports of the run are opened by Pulses, and the
/// chapter's two severances — the authored rehearsal and Fracture's own break —
/// are both answered by moving one end of a Route. This script takes the near
/// line, answers both severances by rerouting, and lets the optional store go,
/// so a driven campaign passes over every test it is offered.
///
/// `core/tests/chapter_the_break.rs` drives the same chapter through both other
/// trunks, all three recoveries, and the eight Forms; this is the floor.
pub fn the_break() -> Vec<Phase> {
    vec![
        play(Act::Toward(1400, 1500), 300),
        play(Act::Follow(1), 17_000),
        play(Act::Charge(1400, 1500), FULL_CHARGE_STEPS),
        play(Act::Release, 1),
        play(Act::Toward(1330, 1760), 420),
        play(Act::Charge(1330, 1760), FULL_CHARGE_STEPS),
        play(Act::Release, 1),
        play(Act::Toward(1600, 1745), 420),
        play(Act::Charge(1600, 1745), FULL_CHARGE_STEPS),
        play(Act::Release, 1),
        play(Act::Toward(1450, 1860), 420),
        play(Act::Charge(1450, 1860), FULL_CHARGE_STEPS),
        play(Act::Release, 1),
        edit(&["{\"plan\":{\"from\":2,\"op\":\"connect\",\"to\":3}}"]),
        play(Act::Toward(1400, 1500), 500),
        // The authored rehearsal takes the junction's own Route into the core
        // 4,500 steps into the hold; the line is moved onto the core instead.
        play(Act::Follow(1), 5_000),
        edit(&["{\"plan\":{\"end\":\"head\",\"op\":\"redirect\",\"route\":1,\"to\":5}}"]),
        play(Act::Toward(1400, 1500), 500),
        play(Act::Follow(1), 22_500),
        // The store stands for its span and is let go.
        play(Act::Follow(1), 5_600),
        play(Act::Follow(1), 22_400),
        // Fracture's break is chapter-absolute; this phase carries the run to
        // it, and the line is then moved onto the intake itself.
        play(Act::Follow(1), 12_000),
        edit(&["{\"plan\":{\"end\":\"tail\",\"op\":\"redirect\",\"route\":1,\"to\":2}}"]),
        play(Act::Toward(1400, 1500), 500),
        play(Act::Follow(1), 30_000),
    ]
}

// ---------------------------------------------------------------------------
// The Quiet Edge — chapter 8, the finale
// ---------------------------------------------------------------------------

/// The Quiet Edge, driven to the campaign's own ending at the authored floor.
///
/// The chapter is one closed pattern — an intake, five arms, and a hold that
/// carries back round — standing on external support that stops a piece at a
/// time. Nothing here can be driven by steering alone: every arm needs a Route
/// out of it, and the five Routes are five queued changes.
///
/// **Both opening tests are let go**, as every arm of this dispatch lets its own
/// go, and letting them go is deliberate rather than incidental: the ring is not
/// closed until the second of them has stood down, which is what keeps a driven
/// campaign's completed list the campaign's required objectives exactly. The
/// two tests are the chapter's continuity reading — a run that closes the ring
/// inside their spans reaches a different ending — so a script that closed it
/// early would take a test the campaign run is supposed to pass over.
///
/// **The last objective is driven with every input neutral.** That is the whole
/// point of the chapter and it is why the last phase is `Act::Rest`: the pattern
/// carries itself once it is closed, and the campaign run proves it by letting
/// go for the whole of the final span.
///
/// `core/tests/chapter_the_quiet_edge.rs` drives the same chapter from all three
/// arrivals and through the eight Forms; this is the floor.
pub fn the_quiet_edge() -> Vec<Phase> {
    vec![
        // The two opening tests stand for their own spans and are let go.
        play(Act::Follow(1), 4_200),
        // The ring, closed: one Route out of each arm into the hold.
        edit(&[
            "{\"plan\":{\"from\":3,\"op\":\"connect\",\"to\":8}}",
            "{\"plan\":{\"from\":4,\"op\":\"connect\",\"to\":8}}",
            "{\"plan\":{\"from\":5,\"op\":\"connect\",\"to\":8}}",
            "{\"plan\":{\"from\":6,\"op\":\"connect\",\"to\":8}}",
            "{\"plan\":{\"from\":7,\"op\":\"connect\",\"to\":8}}",
        ]),
        play(Act::Follow(1), 18_000),
        // The deep band, over the relief Port this run never needs to open.
        play(Act::Toward(2900, 2900), 3_600),
        play(Act::Depth(1), 1),
        play(Act::Follow(2), 24_000),
        play(Act::Depth(-1), 1),
        play(Act::Toward(1500, 1500), 3_000),
        play(Act::Follow(1), 24_000),
        play(Act::Follow(1), 24_000),
        // The closed field, and then the hands off it: every input neutral for
        // the whole of the last objective.
        play(Act::Rest, 40_000),
        play(Act::Rest, 8_000),
    ]
}
