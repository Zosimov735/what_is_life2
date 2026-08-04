//! The staged pressures: the closed set of six, their four stages, and the
//! schedule that admits them.
//!
//! `docs/field-framework/ARCHITECTURE.md` locks `PressureState` field by field
//! — the six machine ids, the four stage names, the four target kinds, and the
//! two invariants a payload is validated against: at most two active
//! (`queued: false`), at most one `primary`. It locks the save payload's
//! `pressures` list as active and queued together, ascending by `pressure` name
//! order within the closed set; the `pressure_changed` event carrying the whole
//! list after a change; the render snapshot's section 6; and the authored file
//! `content/pressures/<id>.json` as "stage thresholds, level curves as
//! piecewise-linear tables, targeting rules as data". The interior schema of
//! that file is delegated to this goal under `content_version`, and it is
//! written out in [`PressureContent`].
//!
//! **What this module carries.** The whole of what the documents lock: the
//! type, the ordering, the invariants, the admission limit, the stage machine
//! that derives a pressure's stage and level, the boundary settlement that
//! seats and removes, and the six effects of ARCHITECTURE.md's
//! `### Pressure effects` — Drain's scaling ([`drained`]), the drawing Noise
//! flow scale ([`noise_flow_scales`]), Flood's threshold ([`FloodPress`]),
//! Interference's redirection ([`Redirection`]), and the stage-entry
//! one-shots (Fracture's break, Flood's hold, Drift's move), which the run
//! applies between steps through [`Staged`]'s reports.
//!
//! **The determinism carriage.** A nonempty pressure list is a step input
//! exactly as the standing View's inside is, and ARCHITECTURE.md's displacement
//! extension point names the two permitted paths and no third: immutable within
//! a window with locked mutation boundaries, or recorded per step. This module
//! takes the first, and the rule is:
//!
//! - Within an active window the **membership** of the list is immutable —
//!   which pressures stand, and each one's `pressure`, `target`, `primary`,
//!   `queued`, and `start_step`. Every rule that changes membership ends the
//!   active window through the same mechanism a committed change does
//!   (`Run::end_window`): admission, activation, resolution and removal, and
//!   the Pulse's interference displacement.
//! - Within a window the only fields that move are `stage` and `level`, and
//!   both are **pure functions of `step - start_step`** and the pressure's own
//!   authored table ([`Schedule::reading`]): the machine writes the curve's
//!   own value every step, and the Pulse's press-back lives in the separate
//!   [`Displaced`] floor, whose write is itself a membership boundary. A
//!   replay derives stage and level from the immutable part and the step it
//!   is replaying, so the trace carries nothing extra and `Trace`'s locked
//!   three-key shape is untouched.
//!
//! That is what lets `replay_onto` and the trajectory keyframe carry take the
//! live list and reproduce the recorded window byte for byte: every retained
//! step ran under exactly the membership that stands now, and the two derived
//! fields are recomputed at the step being replayed rather than carried into
//! it.

use crate::fault::{Code, Fault};
use crate::json::{Arr, Json, Obj};
use crate::read;
use crate::rng::RngState;
use crate::state::{Frac, Step, FRAC_ONE};

/// The most pressures that may stand active at once.
pub const ACTIVE_CAP: usize = 2;

/// The most pressures that may stand primary at once.
pub const PRIMARY_CAP: usize = 1;

/// The most entries one chapter's authored schedule may carry.
pub const SCHEDULE_CAP: usize = 32;

/// The longest dwell one stage may be authored with, in steps: twenty minutes
/// at the locked 30 steps per second, which is the whole of the shortest
/// authored chapter.
pub const STAGE_STEPS_CAP: i64 = 36_000;

/// The closed pressure set of version 1, in the order ARCHITECTURE.md lists it.
/// The order is the ordinal the render snapshot's section 6 carries and the
/// order the save payload's list ascends by.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Pressure {
    Drain,
    Noise,
    Fracture,
    Flood,
    Interference,
    Drift,
}

/// The six machine ids, in closed-set order.
pub const PRESSURES: [&str; 6] = ["drain", "noise", "fracture", "flood", "interference", "drift"];

impl Pressure {
    pub fn name(self) -> &'static str {
        PRESSURES[self.ordinal() as usize]
    }

    pub fn ordinal(self) -> u8 {
        match self {
            Pressure::Drain => 0,
            Pressure::Noise => 1,
            Pressure::Fracture => 2,
            Pressure::Flood => 3,
            Pressure::Interference => 4,
            Pressure::Drift => 5,
        }
    }

    pub fn at(ordinal: usize) -> Option<Self> {
        [
            Pressure::Drain,
            Pressure::Noise,
            Pressure::Fracture,
            Pressure::Flood,
            Pressure::Interference,
            Pressure::Drift,
        ]
        .get(ordinal)
        .copied()
    }

    pub fn read(name: &str) -> Option<Self> {
        PRESSURES.iter().position(|held| *held == name).and_then(Pressure::at)
    }
}

/// The four stages a pressure passes through, in the locked order the render
/// snapshot's stage byte numbers them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Stage {
    Signal,
    Pressure,
    Crisis,
    Resolution,
}

/// The four stage names, in closed-set order.
pub const STAGES: [&str; 4] = ["signal", "pressure", "crisis", "resolution"];

impl Stage {
    pub fn name(self) -> &'static str {
        STAGES[self.ordinal() as usize]
    }

    pub fn ordinal(self) -> u8 {
        match self {
            Stage::Signal => 0,
            Stage::Pressure => 1,
            Stage::Crisis => 2,
            Stage::Resolution => 3,
        }
    }

    pub fn at(ordinal: usize) -> Option<Self> {
        [Stage::Signal, Stage::Pressure, Stage::Crisis, Stage::Resolution].get(ordinal).copied()
    }

    pub fn read(name: &str) -> Option<Self> {
        STAGES.iter().position(|held| *held == name).and_then(Stage::at)
    }
}

/// What a pressure is aimed at, in the closed order the snapshot numbers it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TargetKind {
    None,
    Node,
    Route,
    Layer,
}

/// The four target-kind names, in closed-set order.
pub const TARGET_KINDS: [&str; 4] = ["none", "node", "route", "layer"];

impl TargetKind {
    pub fn name(self) -> &'static str {
        TARGET_KINDS[self.ordinal() as usize]
    }

    pub fn ordinal(self) -> u8 {
        match self {
            TargetKind::None => 0,
            TargetKind::Node => 1,
            TargetKind::Route => 2,
            TargetKind::Layer => 3,
        }
    }

    pub fn at(ordinal: usize) -> Option<Self> {
        [TargetKind::None, TargetKind::Node, TargetKind::Route, TargetKind::Layer]
            .get(ordinal)
            .copied()
    }

    pub fn read(name: &str) -> Option<Self> {
        TARGET_KINDS.iter().position(|held| *held == name).and_then(TargetKind::at)
    }
}

/// What a pressure is aimed at: the kind, and the identifier a kind that names
/// one carries. `none` carries no identifier and `node`, `route`, and `layer`
/// each carry theirs — the locked `{ "t": string, "id": u32|null }`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Target {
    pub kind: TargetKind,
    pub id: Option<u32>,
}

impl Target {
    /// The target of a pressure aimed at nothing in particular.
    pub fn none() -> Self {
        Target { kind: TargetKind::None, id: None }
    }

    pub fn read(value: &Json, key: &str) -> Result<Self, Fault> {
        let found = read::map(value, key)?;
        read::exact_keys(found, key, &["id", "t"])?;
        let kind = TargetKind::at(read::one_of(found, "t", &TARGET_KINDS)?)
            .ok_or_else(|| Fault::field("t"))?;
        let id = read::int_or_null(found, "id", 0, i64::from(u32::MAX))?.map(|held| held as u32);
        // A kind that names something carries an identifier and a kind that
        // names nothing carries none: the two are one reading, so a record
        // carrying half of it is refused rather than half-read.
        match (kind, id) {
            (TargetKind::None, None) => {}
            (TargetKind::None, Some(_)) => return Err(Fault::field("id")),
            (_, None) => return Err(Fault::field("id")),
            (_, Some(_)) => {}
        }
        Ok(Target { kind, id })
    }

    pub fn write(&self, out: &mut String) {
        let mut object = Obj::new(out);
        object.int_or_null("id", self.id.map(i64::from));
        object.text("t", self.kind.name());
        object.end();
    }
}

/// The Pulse's pressed-back floor: the stage it was recorded at, and the
/// level it pressed to. Read only while the pressure stands in that stage;
/// the write is a membership boundary, which is what makes the floor
/// derivable inside every retained window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Displaced {
    pub stage: Stage,
    pub level: Frac,
}

/// Flood's held working target, re-held at every stage entry when Flood is
/// authored with target `none`. Null for every other pressure and for a
/// Flood with an authored target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bound {
    pub kind: TargetKind,
    pub id: u32,
}

/// One staged pressure, exactly as ARCHITECTURE.md's `PressureState` locks it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PressureState {
    pub pressure: Pressure,
    pub stage: Stage,
    /// The stage machine's curve value for the completed step.
    pub level: Frac,
    pub primary: bool,
    pub queued: bool,
    pub start_step: Step,
    pub target: Target,
    pub displaced: Option<Displaced>,
    pub bound: Option<Bound>,
}

impl PressureState {
    /// The effective level: the curve level, floored by `min` against the
    /// displaced floor when one is set and its recorded stage is the current
    /// stage — the one named quantity every effect consumes, which is what
    /// gives a Pulse's press-back its mechanical effect.
    pub fn effective_level(&self) -> Frac {
        match self.displaced {
            Some(floor) if floor.stage == self.stage => self.level.min(floor.level),
            _ => self.level,
        }
    }
}

impl PressureState {
    /// The record a schedule entry stands as before its own step arrives:
    /// queued, at the opening stage, with no level yet.
    pub fn queued_at(pressure: Pressure, start_step: Step, primary: bool, target: Target) -> Self {
        PressureState {
            pressure,
            stage: Stage::Signal,
            level: 0,
            primary,
            queued: true,
            start_step,
            target,
            displaced: None,
            bound: None,
        }
    }

    pub fn read(value: &Json) -> Result<Self, Fault> {
        read::exact_keys(
            value,
            "pressures",
            &[
                "bound",
                "displaced",
                "level",
                "pressure",
                "primary",
                "queued",
                "stage",
                "start_step",
                "target",
            ],
        )?;
        let pressure = Pressure::at(read::one_of(value, "pressure", &PRESSURES)?)
            .ok_or_else(|| Fault::field("pressure"))?;
        let stage =
            Stage::at(read::one_of(value, "stage", &STAGES)?).ok_or_else(|| Fault::field("stage"))?;
        let displaced = match read::at(value, "displaced")? {
            Json::Null => None,
            held => {
                read::exact_keys(held, "displaced", &["level", "stage"])?;
                Some(Displaced {
                    stage: Stage::at(read::one_of(held, "stage", &STAGES)?)
                        .ok_or_else(|| Fault::field("stage"))?,
                    level: read::int(held, "level", 0, FRAC_ONE)?,
                })
            }
        };
        let bound = match read::at(value, "bound")? {
            Json::Null => None,
            held => {
                read::exact_keys(held, "bound", &["id", "t"])?;
                Some(Bound {
                    kind: TargetKind::at(read::one_of(held, "t", &TARGET_KINDS)?)
                        .ok_or_else(|| Fault::field("t"))?,
                    id: read::int(held, "id", 0, i64::from(u32::MAX))? as u32,
                })
            }
        };
        // The two writers are locked: only a displacement writes `displaced`,
        // and displacement presses only Interference; only Flood's hold
        // writes `bound`, and only when no target is authored. A payload
        // carrying either anywhere else is one this build cannot produce.
        if displaced.is_some() && pressure != Pressure::Interference {
            return Err(Fault::field("displaced"));
        }
        let target = Target::read(value, "target")?;
        if bound.is_some() && (pressure != Pressure::Flood || target.kind != TargetKind::None) {
            return Err(Fault::field("bound"));
        }
        Ok(PressureState {
            pressure,
            stage,
            level: read::int(value, "level", 0, FRAC_ONE)?,
            primary: read::flag(value, "primary")?,
            queued: read::flag(value, "queued")?,
            start_step: read::int(value, "start_step", 0, i64::from(u32::MAX))? as Step,
            target,
            displaced,
            bound,
        })
    }

    pub fn write(&self, out: &mut String) {
        let mut object = Obj::new(out);
        match self.bound {
            Some(bound) => {
                let mut held = String::new();
                {
                    let mut written = Obj::new(&mut held);
                    written.int("id", i64::from(bound.id));
                    written.text("t", bound.kind.name());
                    written.end();
                }
                object.raw("bound", &held);
            }
            None => {
                object.null("bound");
            }
        }
        match self.displaced {
            Some(floor) => {
                let mut held = String::new();
                {
                    let mut written = Obj::new(&mut held);
                    written.int("level", floor.level);
                    written.text("stage", floor.stage.name());
                    written.end();
                }
                object.raw("displaced", &held);
            }
            None => {
                object.null("displaced");
            }
        }
        object.int("level", self.level);
        object.text("pressure", self.pressure.name());
        object.bool("primary", self.primary);
        object.bool("queued", self.queued);
        object.text("stage", self.stage.name());
        object.int("start_step", i64::from(self.start_step));
        {
            let mut target = String::new();
            self.target.write(&mut target);
            object.raw("target", &target);
        }
        object.end();
    }

    pub fn written(&self) -> String {
        let mut out = String::new();
        self.write(&mut out);
        out
    }
}

/// The whole list as the payload and the `pressure_changed` event carry it:
/// active and queued together, ascending by `pressure` name order within the
/// closed set.
pub fn written_list(pressures: &[PressureState]) -> String {
    let mut out = String::new();
    let mut list = crate::json::Arr::new(&mut out);
    for pressure in pressures {
        list.raw(&pressure.written());
    }
    list.end();
    out
}

/// Reads the whole list back, holding it to the locked ordering and the two
/// locked invariants.
pub fn read_list(payload: &Json, key: &str) -> Result<Vec<PressureState>, Fault> {
    let mut found = Vec::new();
    for entry in read::list(payload, key, PRESSURES.len())? {
        found.push(PressureState::read(entry)?);
    }
    validate_list(&found).map_err(|fault| read::recode(fault, Code::Validation))?;
    Ok(found)
}

/// The two locked invariants, and the locked ordering that carries them.
///
/// ARCHITECTURE.md validates exactly two things of a pressure list — at most
/// two active, at most one primary — and locks the list as ascending by
/// `pressure` name order within the closed set. The ordering is what makes one
/// pressure stand at most once, so the three are checked together.
pub fn validate_list(pressures: &[PressureState]) -> Result<(), Fault> {
    let ordinals: Vec<u8> = pressures.iter().map(|held| held.pressure.ordinal()).collect();
    if !read::ascending(&ordinals) {
        return Err(Fault::field("pressures"));
    }
    if pressures.iter().filter(|held| !held.queued).count() > ACTIVE_CAP {
        return Err(Fault::field("pressures"));
    }
    if pressures.iter().filter(|held| held.primary).count() > PRIMARY_CAP {
        return Err(Fault::field("pressures"));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// The authored tables
// ---------------------------------------------------------------------------

/// One stage of one pressure's authored table: the level the pressure reads at
/// while it stands in that stage, and how many steps it stands there.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StageRow {
    pub stage: Stage,
    /// The level the pressure carries through this stage, `Frac` in [0, 65536].
    pub level: Frac,
    /// The dwell, in steps, `[1, STAGE_STEPS_CAP]`.
    pub steps: i64,
}

/// One authored pressure file, `content/pressures/<id>.json`.
///
/// ```text
/// { "id": "drain",
///   "target": "layer",
///   "stages": [ { "stage": "signal",     "level": 8192,  "steps": 90 },
///               { "stage": "pressure",   "level": 26214, "steps": 180 },
///               { "stage": "crisis",     "level": 52429, "steps": 120 },
///               { "stage": "resolution", "level": 13107, "steps": 90 } ] }
/// ```
///
/// `stages` is the level curve and the stage thresholds together: exactly the
/// four stages in the locked order, each with the level it carries and the
/// steps it stands. It is a piecewise-**constant** reading of ARCHITECTURE.md's
/// "level curves as piecewise-linear tables" — the degenerate case of one — and
/// the reason it is the constant one rather than the interpolated one is the
/// determinism carriage in this module's header: `level` moving every step
/// would make every step a mutation boundary and the retained span would never
/// regrow past 0. Which of the two the document wants is enumerated for the
/// amendment round rather than chosen here.
///
/// `target` is the one target kind this pressure may be aimed at. The
/// identifier comes from the chapter's schedule entry, so targeting is authored
/// end to end and no selection rule is inferred from the Field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PressureContent {
    pub pressure: Pressure,
    pub target: TargetKind,
    pub stages: [StageRow; STAGES.len()],
}

impl PressureContent {
    /// The dwell of one stage, in steps.
    pub fn steps(&self, stage: Stage) -> i64 {
        self.stages[stage.ordinal() as usize].steps
    }

    /// The level one stage carries.
    pub fn level(&self, stage: Stage) -> Frac {
        self.stages[stage.ordinal() as usize].level
    }

    /// How many steps the whole run of stages takes.
    pub fn span(&self) -> i64 {
        self.stages.iter().map(|row| row.steps).sum()
    }

    pub fn read(value: &Json) -> Result<Self, Fault> {
        read::exact_keys(value, "pressure", &["id", "stages", "target"])?;
        let pressure = Pressure::at(read::one_of(value, "id", &PRESSURES)?)
            .ok_or_else(|| Fault::field("id"))?;
        let target = TargetKind::at(read::one_of(value, "target", &TARGET_KINDS)?)
            .ok_or_else(|| Fault::field("target"))?;
        let rows = read::list(value, "stages", STAGES.len())?;
        if rows.len() != STAGES.len() {
            return Err(Fault::field("stages"));
        }
        let mut stages = [StageRow { stage: Stage::Signal, level: 0, steps: 1 }; STAGES.len()];
        for (place, entry) in rows.iter().enumerate() {
            read::exact_keys(entry, "stages", &["level", "stage", "steps"])?;
            let stage = Stage::at(read::one_of(entry, "stage", &STAGES)?)
                .ok_or_else(|| Fault::field("stage"))?;
            // The four stand in the locked order, so the table reads as the
            // curve it is rather than as four rows in any arrangement.
            if stage.ordinal() as usize != place {
                return Err(Fault::field("stages"));
            }
            stages[place] = StageRow {
                stage,
                level: read::int(entry, "level", 0, FRAC_ONE)?,
                steps: read::int(entry, "steps", 1, STAGE_STEPS_CAP)?,
            };
        }
        Ok(PressureContent { pressure, target, stages })
    }

    fn write(&self, out: &mut String) {
        let mut object = Obj::new(out);
        object.text("id", self.pressure.name());
        {
            let mut stages = object.list("stages");
            for row in &self.stages {
                let mut written = String::new();
                let mut stage = Obj::new(&mut written);
                stage.int("level", row.level);
                stage.text("stage", row.stage.name());
                stage.int("steps", row.steps);
                stage.end();
                stages.raw(&written);
            }
            stages.end();
        }
        object.text("target", self.target.name());
        object.end();
    }

    fn written(&self) -> String {
        let mut out = String::new();
        self.write(&mut out);
        out
    }
}

/// One entry of a chapter's authored `pressure_schedule`.
///
/// ```text
/// { "pressure": "interference", "start_step": 300, "primary": true,
///   "target": { "t": "node", "id": 4 } }
/// ```
///
/// The schedule **requests**: the core admits per the locked limit, and an
/// entry the limit refuses stands queued until room appears.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScheduleEntry {
    pub pressure: Pressure,
    pub start_step: Step,
    pub primary: bool,
    pub target: Target,
}

impl ScheduleEntry {
    pub fn read(value: &Json) -> Result<Self, Fault> {
        read::exact_keys(value, "pressure_schedule", &["pressure", "primary", "start_step", "target"])?;
        let pressure = Pressure::at(read::one_of(value, "pressure", &PRESSURES)?)
            .ok_or_else(|| Fault::field("pressure"))?;
        Ok(ScheduleEntry {
            pressure,
            start_step: read::int(value, "start_step", 0, i64::from(u32::MAX))? as Step,
            primary: read::flag(value, "primary")?,
            target: Target::read(value, "target")?,
        })
    }
}

/// The six authored tables, and the one place a step reads them.
///
/// A run's content is fixed for its life — `content_hash` pins it and a restore
/// under a different one carries on with `content_changed` set — so this is a
/// step input of the same class as `input_config.pointer_speed`: immutable
/// while the run plays, and so outside what a window has to carry.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Schedule {
    tables: Vec<PressureContent>,
}

impl Schedule {
    /// The tables the manifest listed, held to one entry per pressure.
    pub fn of(mut tables: Vec<PressureContent>) -> Result<Self, Fault> {
        let ordinals: Vec<u8> = tables.iter().map(|table| table.pressure.ordinal()).collect();
        let mut sorted = ordinals.clone();
        sorted.sort_unstable();
        sorted.dedup();
        if sorted.len() != ordinals.len() {
            return Err(Fault::because(Code::ContentInvalid, "pressures"));
        }
        tables.sort_by_key(|table| table.pressure.ordinal());
        Ok(Schedule { tables })
    }

    /// Reads the frozen pressure tables carried by a generator specification.
    pub fn read(value: &Json, key: &str) -> Result<Self, Fault> {
        let mut tables = Vec::new();
        for entry in read::list(value, key, PRESSURES.len())? {
            tables.push(PressureContent::read(entry)?);
        }
        Schedule::of(tables).map_err(|fault| read::recode(fault, Code::Validation))
    }

    /// Writes the frozen tables in pressure ordinal order.
    pub fn written(&self) -> String {
        let mut out = String::new();
        let mut tables = Arr::new(&mut out);
        for table in &self.tables {
            tables.raw(&table.written());
        }
        tables.end();
        out
    }

    /// The authored table for one pressure, and none when the manifest listed
    /// no file for it.
    pub fn table(&self, pressure: Pressure) -> Option<&PressureContent> {
        self.tables.iter().find(|table| table.pressure == pressure)
    }

    pub fn is_empty(&self) -> bool {
        self.tables.is_empty()
    }

    /// The stage and level a pressure reads at a step, derived rather than
    /// carried: the whole of what moves inside an active window.
    ///
    /// `elapsed` steps after activation the pressure stands in the first stage
    /// whose cumulative dwell reaches past `elapsed`, and carries that stage's
    /// authored level. Past the last dwell it is spent, and the caller removes
    /// it — which is a change of membership, so it ends the window.
    pub fn reading(&self, pressure: &PressureState, step: Step) -> Reading {
        let Some(table) = self.table(pressure.pressure) else {
            // A pressure with no authored table has no curve to read, so it
            // carries no level and never advances. It cannot arise from
            // authored content — the schedule is validated against the tables —
            // and this is the reading of a payload that named one anyway.
            return Reading::Spent;
        };
        if step < pressure.start_step {
            return Reading::Standing { stage: Stage::Signal, level: 0 };
        }
        let mut elapsed = i64::from(step - pressure.start_step);
        for row in &table.stages {
            if elapsed < row.steps {
                return Reading::Standing { stage: row.stage, level: row.level };
            }
            elapsed -= row.steps;
        }
        Reading::Spent
    }
}

/// What a pressure reads at one step: where it stands, or that it is spent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reading {
    Standing { stage: Stage, level: Frac },
    Spent,
}

/// What one step's staging derived and what it left for the boundary.
///
/// The step function never mutates the list's membership: within a window
/// the membership is constant, `stage` and `level` are re-derived per step,
/// and everything that cannot be re-derived — a seat, a removal, a
/// displacement floor, a Flood hold, a Fracture break, a Drift move — is
/// applied by the caller **between steps**, after the active window has been
/// ended under the list its steps ran under. That is the committed-change
/// shape, and it is what makes every retained window replay byte-exact.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Staged {
    /// True when any pressure's `stage` turned over this step. Derived state,
    /// not a boundary; the surface still wants to know.
    pub stage: bool,
    /// The stage turnovers this step derived, `(ordinal, stage)` ascending.
    /// The stage-entry one-shots — Fracture's break at crisis entry, Flood's
    /// hold, Drift's move — fall due on exactly these, at the boundary.
    pub entered: Vec<(u8, Stage)>,
    /// The active pressures whose run of stages this step walked past the
    /// end, ascending ordinals: spent, and removed at the boundary.
    pub spent: Vec<u8>,
    /// The presses the Pulse made this step: the `displaced` floors to write
    /// at the boundary, ascending ordinals. The write is the membership
    /// boundary the locked text names; the step itself only derives it.
    pub pressed: Vec<(u8, Displaced)>,
}

impl Staged {
    /// True when the boundary has anything to apply or the shell anything to
    /// hear.
    pub fn changed(&self) -> bool {
        self.stage
            || !self.entered.is_empty()
            || !self.spent.is_empty()
            || !self.pressed.is_empty()
    }
}

/// Advances the staged pressures one step: the two derived fields, and the
/// reports the boundary acts on.
///
/// It is the first rule of the pressure phase, so Drain, the overload decay,
/// and Current delivery all read the list as this step staged it, and the
/// Pulse and the Route phase — which ran earlier — read it as the step
/// opened. Membership never moves here: a spent pressure is reported and
/// stands until the boundary removes it, and admission is the boundary's own
/// act, so every step of a window runs under one membership and a replay
/// re-derives everything this function writes.
///
/// Every branch is a pure integer function of the list, the step, and the
/// authored tables; nothing here draws.
pub fn advance_pressures(
    pressures: &mut [PressureState],
    schedule: &Schedule,
    step: Step,
) -> Staged {
    let mut staged = Staged::default();
    for pressure in pressures.iter_mut() {
        if pressure.queued {
            continue;
        }
        match schedule.reading(pressure, step) {
            Reading::Standing { stage, level } => {
                if pressure.stage != stage {
                    staged.stage = true;
                    pressure.stage = stage;
                    staged.entered.push((pressure.pressure.ordinal(), stage));
                }
                pressure.level = level;
            }
            Reading::Spent => {
                staged.spent.push(pressure.pressure.ordinal());
            }
        }
    }
    staged
}

/// What one boundary settlement did to the list.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Settled {
    /// True when the list changed — a removal, a seat, or a written floor —
    /// so the boundary ends the window and the shell is told.
    pub changed: bool,
    /// The seats taken, `(ordinal, opening stage)` ascending: each is a
    /// stage entry for the one-shot effects, exactly as a turnover is.
    pub admitted: Vec<(u8, Stage)>,
}

/// Settles the list at a step boundary, for the step about to run: removes
/// what the completed step reported spent, writes the floors the Pulse
/// pressed, and admits what is due — in that order, so a freed seat can be
/// taken at the same boundary.
///
/// Admission, in the closed set's own order, which is the list's order: a
/// queued pressure is due when `start_step <= next_step`, and takes a seat if
/// the limit leaves one. An entry the limit refuses stays queued and tries
/// again at every boundary until room appears — the schedule requests, the
/// core admits per the limit, and nothing an author scheduled is silently
/// dropped. A seat rebases `start_step` to the first step the pressure acts
/// in, so a pressure the limit held back starts at its signal rather than
/// arriving mid-stage; an on-time seat writes the value it already holds.
/// The seat takes the opening stage's own curve level, so the list a boundary
/// leaves already reads as the step about to run will read it.
///
/// The caller ends the active window before applying what this settles: every
/// change here is a membership boundary, and no retained step may be replayed
/// under a list it did not run under.
pub fn settle_boundary(
    pressures: &mut Vec<PressureState>,
    schedule: &Schedule,
    spent: &[u8],
    pressed: &[(u8, Displaced)],
    next_step: Step,
) -> Settled {
    let mut settled = Settled::default();

    let before = pressures.len();
    pressures.retain(|pressure| {
        pressure.queued || !spent.contains(&pressure.pressure.ordinal())
    });
    settled.changed |= pressures.len() != before;

    for (ordinal, floor) in pressed {
        if let Some(pressure) = pressures
            .iter_mut()
            .find(|held| !held.queued && held.pressure.ordinal() == *ordinal)
        {
            pressure.displaced = Some(*floor);
            settled.changed = true;
        }
    }

    for place in 0..pressures.len() {
        if !pressures[place].queued || pressures[place].start_step > next_step {
            continue;
        }
        let active = pressures.iter().filter(|held| !held.queued).count();
        if active >= ACTIVE_CAP {
            continue;
        }
        if pressures[place].primary
            && pressures.iter().filter(|held| held.primary && !held.queued).count() >= PRIMARY_CAP
        {
            continue;
        }
        pressures[place].queued = false;
        pressures[place].start_step = next_step;
        if let Reading::Standing { stage, level } =
            schedule.reading(&pressures[place], next_step)
        {
            pressures[place].stage = stage;
            pressures[place].level = level;
        }
        settled.admitted.push((pressures[place].pressure.ordinal(), Stage::Signal));
        settled.changed = true;
    }
    settled
}

// ---------------------------------------------------------------------------
// The continuous effects, as the step's phases read them
// ---------------------------------------------------------------------------
//
// ARCHITECTURE.md's Pressure effects section locks the six rules and their
// reading points: the Noise flow scale and Flood's throttle term read the
// list as the step opened (they run in the Route phase, before the stage
// machine), the Pulse reads it in its own phase, and everything at or after
// the machine — Drain scaling, overload decay, Current delivery — reads the
// list as this step staged it. The stage-entry one-shots (Fracture's break,
// Flood's hold, Drift's move) ride the committed-change machinery between
// steps and live with the run, not here.

/// The active pressure of one kind, and none when it does not stand: the one
/// seat per pressure the list ordering enforces makes this at most one.
pub fn active_of(pressures: &[PressureState], kind: Pressure) -> Option<&PressureState> {
    pressures.iter().find(|held| !held.queued && held.pressure == kind)
}

/// The effective level a pressure stood at as a step OPENED — the previous
/// step's staging — derived, never carried.
///
/// The locked reading points say the Route phase reads the list as the step
/// opened, and "as the step opened" is the previous step's derivation:
/// `reading(max(start_step, step - 1))`, floored by a stage-matching
/// `displaced`. It is re-derived here as a pure function of the immutable
/// membership and the step number rather than read off the list's `stage`
/// and `level` fields, because a replayed window carries the fields as they
/// stand NOW — a later stage's values — while the live step read the values
/// its predecessor derived. A seating step reads its own opening stage (the
/// boundary seated it with exactly that reading), a mid-window step reads
/// its predecessor's, and a live step and every replay of it therefore read
/// the same opened values by construction, whatever the clone's fields hold.
pub fn opened_level(pressure: &PressureState, schedule: &Schedule, step: Step) -> Frac {
    let at = step.saturating_sub(1).max(pressure.start_step);
    match schedule.reading(pressure, at) {
        Reading::Standing { stage, level } => match pressure.displaced {
            Some(floor) if floor.stage == stage => level.min(floor.level),
            _ => level,
        },
        // A pressure past its run of stages contributes nothing while it
        // waits for the boundary that removes it.
        Reading::Spent => 0,
    }
}

/// The per-layer flow scales the Noise rule draws, one entry per layer index,
/// at the start of the Route phase: layers ascending, one addressed draw per layer
/// whose effective noise is above zero, from `(route_noise, layer, step)`.
///
/// `noise_eff(L) = min(65536, noise(L) + level_eff)`: the layer's authored
/// base, plus an active Noise pressure's effective level when it targets L,
/// additive with saturation. The scale is `65536 - fixed_mul(noise_eff, j)`
/// with `j = draw(stream, 65537)`, flooring; a layer with effective noise 0
/// draws nothing and keeps the whole scale, so a run without noise consumes
/// no event draw and every pre-Noise byte pin stands. The drawn scale moves no
/// Charge of its own: it only narrows what Route flow may move.
pub fn noise_flow_scales(
    layers: &[crate::field::FieldLayer],
    pressures: &[PressureState],
    schedule: &Schedule,
    step: Step,
    stream: &mut RngState,
) -> [Frac; crate::field::LAYERS_PER_CHAPTER] {
    let mut scales = [FRAC_ONE; crate::field::LAYERS_PER_CHAPTER];
    let noise = active_of(pressures, Pressure::Noise);
    for layer in layers {
        let base = layer.noise;
        let added = match noise {
            Some(held)
                if held.target.kind == TargetKind::Layer
                    && held.target.id == Some(u32::from(layer.layer)) =>
            {
                // The reading as the step opened, derived — the seam the
                // replay carriage rests on, since this is the rule that
                // draws.
                opened_level(held, schedule, step)
            }
            _ => 0,
        };
        let effective = (base + added).min(FRAC_ONE);
        if effective <= 0 {
            continue;
        }
        let word = stream.addressed("route_noise", u32::from(layer.layer), step, 65_537) as Frac;
        scales[usize::from(layer.layer)] =
            FRAC_ONE - crate::fx::fixed_mul(effective, word);
    }
    scales
}

/// The Node Flood presses, and the threshold it presses it to: while a Flood
/// pressure stands active, its target — the authored Node, or the Node its
/// `bound` holds — overloads at
/// `capacity - fixed_mul(capacity, fixed_mul(level_eff, 32768))`, at most
/// half off at full level, flooring twice. Every other Node keeps its own
/// `capacity`.
#[derive(Clone, Copy, Debug, Default)]
pub struct FloodPress {
    pub node: Option<u32>,
    /// `fixed_mul(level_eff, 32768)`: the share of the capacity taken off.
    pub share: Frac,
}

impl FloodPress {
    /// Reads the press off the staged list — the reading decay and the
    /// end-of-step flags consult, whose `stage` and `level` the machine has
    /// just derived for this step.
    pub fn of(pressures: &[PressureState]) -> Self {
        let Some(flood) = active_of(pressures, Pressure::Flood) else {
            return FloodPress::default();
        };
        FloodPress {
            node: Self::aimed(flood),
            share: crate::fx::fixed_mul(flood.effective_level(), 32_768),
        }
    }

    /// The press as the step OPENED — the Route-phase throttle's reading —
    /// with the level derived by [`opened_level`] rather than read off the
    /// list's fields, for exactly the reason the Noise scale derives its own:
    /// a replayed window carries the fields of a later stage.
    pub fn opened(pressures: &[PressureState], schedule: &Schedule, step: Step) -> Self {
        let Some(flood) = active_of(pressures, Pressure::Flood) else {
            return FloodPress::default();
        };
        FloodPress {
            node: Self::aimed(flood),
            share: crate::fx::fixed_mul(opened_level(flood, schedule, step), 32_768),
        }
    }

    /// The Node the press stands on: the authored target, or the held bound —
    /// both membership-class, immutable inside a window.
    fn aimed(flood: &PressureState) -> Option<u32> {
        match flood.target.kind {
            TargetKind::Node => flood.target.id,
            TargetKind::None => flood.bound.map(|bound| bound.id),
            _ => None,
        }
    }

    /// The overload threshold one Node stands under.
    pub fn threshold(&self, node: u32, capacity: crate::state::Fx) -> crate::state::Fx {
        if self.node == Some(node) {
            capacity - crate::fx::fixed_mul(capacity, self.share)
        } else {
            capacity
        }
    }
}

/// The redirection Interference presses on Current delivery: the target Node
/// and `fixed_mul(level_eff, 32768)` — the share of each same-layer current's
/// emission it claims first, at most half at full level.
#[derive(Clone, Copy, Debug, Default)]
pub struct Redirection {
    pub node: Option<u32>,
    pub share: Frac,
}

impl Redirection {
    /// Reads the redirection off the staged list, which is the list Current
    /// delivery reads.
    pub fn of(pressures: &[PressureState]) -> Self {
        let Some(held) = active_of(pressures, Pressure::Interference) else {
            return Redirection::default();
        };
        if held.target.kind != TargetKind::Node {
            return Redirection::default();
        }
        Redirection {
            node: held.target.id,
            share: crate::fx::fixed_mul(held.effective_level(), 32_768),
        }
    }
}

/// The Drain scaling: the targeted layer's per-Node loss becomes
/// `drain + fixed_mul(drain, level_eff)` — flooring, at most double at full
/// level — through the locked Drain rule, still entering the one `drain`
/// sink; other layers are untouched, and level 0 changes nothing.
pub fn drained(layer: u8, drain: crate::state::Fx, pressures: &[PressureState]) -> crate::state::Fx {
    match active_of(pressures, Pressure::Drain) {
        Some(held)
            if held.target.kind == TargetKind::Layer
                && held.target.id == Some(u32::from(layer)) =>
        {
            drain + crate::fx::fixed_mul(drain, held.effective_level())
        }
        _ => drain,
    }
}
