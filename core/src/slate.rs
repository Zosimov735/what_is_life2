//! Candidate-View generation: the slate an evaluation stands on.
//!
//! `docs/field-framework/FRAMEWORK.md`'s Candidate slate section is the whole
//! of what this module does, carried out exactly as it is written: the seven
//! sources, intake, the three inside-axis variants and their permitting
//! conditions, the duplicate rule, the rotated offer sequence, and the record
//! the slate carries. `docs/field-framework/ARCHITECTURE.md` locks the
//! serialized shape (`CandidateSlate`, `Provenance`, `absent`,
//! `standing_intake`, `omitted`, `discarded`).
//!
//! Two properties are load-bearing, and they are what the tests pin:
//!
//! - **Deterministic.** Assembly draws no randomness at all. It is a pure
//!   function of the Field state, the standing View, the assembly ordinal, the
//!   previous assembly's evaluation step, `sigma_V`, and the tolerance — the
//!   locked reproducibility sentence — so the same inputs assemble the same
//!   slate byte for byte.
//! - **Bounded.** Every pass here is over the Field's own caps: 256 Nodes, 512
//!   Routes, a window of at most 60 steps, 32 retained drawn entries. The two
//!   tables the sources read — adjacency and links — are quadratic in the Node
//!   count and are built once; every rule then reads them rather than the
//!   Route list, so no pass is worse than quadratic.
//!
//! What this module does *not* do is rank. Assembly names the candidates and
//! nothing more; [`crate::rank`] reads the four privilege values, compares
//! them through their confidence ranges, and fills in the tiers, the dominance
//! relation, the baselines, and the tolerance-sensitivity flag. The record's
//! shape is here because the record is one shape, and the reader admits
//! exactly what this build writes — a payload carrying a value this build
//! could not re-serialize is one that would not come back out byte for byte.

use crate::fault::Fault;
use crate::json::{Json, Obj};
use crate::read;
use crate::rng::{Part, RngState};
use crate::state::{
    FieldState, Frac, Fx, RunState, Step, Surround, TraceStep, ViewDeclaration, FRAC_ONE,
};

/// The declared tolerance, `tau = 1/8` as a Q0.16 fraction. FRAMEWORK.md
/// declares the default and holds tau to `0 < tau <= 1/2`.
pub const TAU_DEFAULT: Frac = FRAC_ONE / 8;

/// The most candidates one slate holds. A slate of one is deficient.
pub const SLATE_CAP: usize = 5;

/// How many baseline replays an evaluated View takes. The count is locked here
/// because the record carries one deviation slot per sample from the first
/// assembly, even while every slot stands empty.
pub const BASELINE_SAMPLES: usize = 8;

/// How many provenances one entry can carry: one per source.
const SOURCES: usize = 8;

/// How many intake discards one record retains: the first 64 in offer order.
/// The stored lists can propose more — the drawn list holds 32 and the
/// authored list is bounded only by the Node cap — and a proposal past this
/// cap is dropped from the record with no marker. The truncation is silent,
/// and ARCHITECTURE.md declares it beside the record's shape.
const DISCARDS_RETAINED: usize = 64;

/// The reason an intake discard records.
const INTAKE_EMPTY: &str = "intake-empty";

/// The reason the standing View's fallback records, verbatim from FRAMEWORK.md.
const STANDING_VANISHED: &str = "standing-inside-vanished";

/// The reason a slate of one records.
const NO_ALTERNATIVE: &str = "no-alternative-candidate";

/// The stated reasons a privilege value is unassigned, ascending.
///
/// FRAMEWORK.md's minimum-data table is the whole of what puts a value in this
/// state, and each of these names one of its rows: an effective window with too
/// few steps for the procedure, no qualifying grain pair, too few members or
/// too few surround Nodes for Shared Failure, fewer than four defined samples,
/// and a window whose recorded circulating flow is zero. Every record stores
/// the reason, and the reader admits no other.
pub const UNASSIGNED_REASONS: [&str; 6] = [
    "few-members",
    "few-samples",
    "few-surround",
    "no-circulating-flow",
    "no-grain-pair",
    "window-too-short",
];

/// The effective window supplied fewer steps than the procedure needs.
pub const WINDOW_TOO_SHORT: &str = "window-too-short";
/// Neither of Scale Stability's two grain pairs lies on the ladder inside the
/// member count.
pub const NO_GRAIN_PAIR: &str = "no-grain-pair";
/// Shared Failure needs two members.
pub const FEW_MEMBERS: &str = "few-members";
/// Shared Failure needs two surround Nodes.
pub const FEW_SURROUND: &str = "few-surround";
/// A median over fewer than four defined samples is unassigned.
pub const FEW_SAMPLES: &str = "few-samples";
/// Cut Impact needs recorded circulating flow.
pub const NO_CIRCULATING_FLOW: &str = "no-circulating-flow";

/// The reasons the session-lived readings add: the coordinate profile's and the
/// perturbation kinds' own.
///
/// They stand beside [`UNASSIGNED_REASONS`] rather than inside it, and the
/// difference is what the reader admits: a coordinate profile and a
/// perturbation result are never serialized into a payload, so no payload may
/// carry one of these, and [`read_value`] keeps reading the slate's closed set
/// alone.
pub const MEASUREMENT_REASONS: [&str; 6] = [
    "no-member",
    "no-route-to-remove",
    "no-shared-value",
    "no-signature",
    "no-stored-charge",
    "no-upkeep",
];

/// The two tolerance recomputations a sensitivity flag can name.
pub const SENSITIVITY_HALF: &str = "half";
pub const SENSITIVITY_DOUBLE: &str = "double";

/// Where a candidate came from. Sources 1 to 4 are situational; 5 to 7 are the
/// three variants derived from the standing View; `standing` is seat 1.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Standing,
    Finer,
    Coarser,
    Lateral,
    Drawn,
    Clusters,
    Authored,
    Responses,
}

impl Source {
    pub fn name(self) -> &'static str {
        match self {
            Source::Standing => "standing",
            Source::Finer => "finer",
            Source::Coarser => "coarser",
            Source::Lateral => "lateral",
            Source::Drawn => "drawn",
            Source::Clusters => "clusters",
            Source::Authored => "authored",
            Source::Responses => "responses",
        }
    }

    fn read(name: &str) -> Option<Self> {
        Some(match name {
            "standing" => Source::Standing,
            "finer" => Source::Finer,
            "coarser" => Source::Coarser,
            "lateral" => Source::Lateral,
            "drawn" => Source::Drawn,
            "clusters" => Source::Clusters,
            "authored" => Source::Authored,
            "responses" => Source::Responses,
            _ => return None,
        })
    }

    /// A situational source's place in the omitted counts, which is also its
    /// place in the rotated cycle: 1 drawn, 2 clusters, 3 authored,
    /// 4 responses.
    fn slot(self) -> Option<usize> {
        match self {
            Source::Drawn => Some(0),
            Source::Clusters => Some(1),
            Source::Authored => Some(2),
            Source::Responses => Some(3),
            _ => None,
        }
    }
}

/// Why one candidate exists: the source and the detail that source records —
/// the drawn entry's position, the cluster's total weight, the authored
/// position, the total co-response count. The variants and the standing View
/// carry none.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Provenance {
    pub source: Source,
    pub detail: Option<i64>,
}

/// One of the four privilege values: the number, the confidence range that
/// contains it, how many samples it was taken from, and — when it is
/// unassigned — the stated reason and no numbers at all.
///
/// Nothing here is ever added to, averaged with, or weighed against another
/// value. The four stand apart, they are compared one at a time through their
/// confidence ranges, and the only ordering they may produce is the tier.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrivilegeValue {
    pub value: Option<Frac>,
    pub low: Option<Frac>,
    pub high: Option<Frac>,
    /// How many defined samples the value was taken from: 8 for a stochastic
    /// procedure that defined them all, 1 for a deterministic one, 0 for a
    /// value no replay was run for.
    pub samples: u16,
    /// The stated reason, exactly when the value is unassigned. One of
    /// [`UNASSIGNED_REASONS`].
    pub reason: Option<&'static str>,
}

impl PrivilegeValue {
    /// An assigned value and the confidence range around it.
    pub fn assigned(value: Frac, low: Frac, high: Frac, samples: u16) -> Self {
        debug_assert!(low <= value && value <= high, "a confidence range contains its value");
        PrivilegeValue { value: Some(value), low: Some(low), high: Some(high), samples, reason: None }
    }

    /// An unassigned value: no number, no range, and the reason it carries.
    pub fn unassigned(reason: &'static str) -> Self {
        debug_assert!(
            UNASSIGNED_REASONS.contains(&reason) || MEASUREMENT_REASONS.contains(&reason),
            "the reason set is closed",
        );
        PrivilegeValue { value: None, low: None, high: None, samples: 0, reason: Some(reason) }
    }

    pub fn is_assigned(&self) -> bool {
        self.value.is_some()
    }
}

/// The four values one candidate carries, never summed, averaged, weighted, or
/// otherwise combined — in this type, in any interface, or in anything derived
/// from them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrivilegeProfile {
    pub scale_stability: PrivilegeValue,
    pub shared_failure: PrivilegeValue,
    pub cut_impact: PrivilegeValue,
    pub boundary_sufficiency: PrivilegeValue,
}

impl PrivilegeProfile {
    /// The four unassigned, all naming one reason — the form a candidate
    /// carries between assembly and evaluation, and the form every candidate
    /// of a slate assembled under a clamped span of nothing keeps.
    pub fn unassigned(reason: &'static str) -> Self {
        PrivilegeProfile {
            scale_stability: PrivilegeValue::unassigned(reason),
            shared_failure: PrivilegeValue::unassigned(reason),
            cut_impact: PrivilegeValue::unassigned(reason),
            boundary_sufficiency: PrivilegeValue::unassigned(reason),
        }
    }

    /// The four, in the record's own ascending key order.
    pub fn each(&self) -> [&PrivilegeValue; 4] {
        [
            &self.boundary_sufficiency,
            &self.cut_impact,
            &self.scale_stability,
            &self.shared_failure,
        ]
    }
}

/// One candidate: the View it declares, every provenance that reached it, the
/// four values it was read under, the tier the ranking put it in, and the
/// baseline deviations its replays produced.
#[derive(Clone, Debug)]
pub struct Candidate {
    pub view: ViewDeclaration,
    pub provenance: Vec<Provenance>,
    pub privilege: PrivilegeProfile,
    /// The rank the nondominance ranking gives it: 1 for the nondominated set,
    /// r + 1 for the set that stands once tiers 1 through r are removed. A
    /// deficient slate runs no comparison, so its one candidate carries 0 —
    /// the number no tier has.
    pub tier: u8,
    /// The eight baseline replay deviations, and one empty slot per sample
    /// when the window is too short to compare direction symbols.
    pub baseline: [Option<Frac>; BASELINE_SAMPLES],
}

/// A proposed inside intake emptied, with the reason it was discarded.
#[derive(Clone, Debug)]
pub struct Discard {
    pub source: Source,
    pub detail: Option<i64>,
    pub reason: String,
}

/// A variant the Field did not permit, with the condition that failed.
#[derive(Clone, Debug)]
pub struct Absence {
    pub variant: Source,
    pub condition: String,
}

/// The evaluation record, as ARCHITECTURE.md serializes it.
#[derive(Clone, Debug)]
pub struct CandidateSlate {
    pub ordinal: u32,
    pub step: Step,
    pub sigma: RngState,
    pub tau: Frac,
    pub window_declared: u16,
    pub window_effective: u16,
    pub candidates: Vec<Candidate>,
    pub deficient: bool,
    pub deficiency_reason: Option<String>,
    /// Per situational source, in the order drawn, clusters, authored,
    /// responses: how many offers a full slate turned away.
    pub omitted: [u16; 4],
    /// Every proposed inside intake emptied, in offer order.
    pub discarded: Vec<Discard>,
    pub absent: Vec<Absence>,
    /// How many identifiers the standing inside's intersection with N removed.
    pub standing_removed: u16,
    /// Whether the standing inside vanished and the fallback replaced it.
    pub standing_fallback: bool,
    pub standing_reason: Option<String>,
    /// The Field's declared Forecast depth `a_F`.
    pub forecast_depth: u16,
    /// The dominance relation itself: every ordered pair of assembly positions
    /// where the first candidate dominates the second, ascending. The tiers
    /// derive from it and do not replace it.
    pub dominance: Vec<(u8, u8)>,
    /// Whether either tolerance recomputation changed the nondominated set.
    pub sensitivity: bool,
    /// Which recomputations changed it: `half`, `double`, or both.
    pub sensitivity_changed_at: Vec<&'static str>,
    /// The standing candidate's per-step baseline `q_I` envelope: the smallest
    /// and largest the eight baseline replays read at each window step. The
    /// only forward reading the game may show.
    pub forecast_envelope: Vec<(Fx, Fx)>,
}

impl CandidateSlate {
    /// The record as canonical JSON.
    ///
    /// One slot stands reserved: the top-level `detail`, which the
    /// budget-overrun contract records a measured duration in and which is
    /// null in every record this build writes.
    pub fn write(&self, out: &mut String) {
        let mut object = Obj::new(out);
        {
            let mut absent = object.list("absent");
            for entry in &self.absent {
                let mut written = absent.object();
                written.text("condition", &entry.condition);
                written.text("variant", entry.variant.name());
                written.end();
            }
            absent.end();
        }
        {
            let mut candidates = object.list("candidates");
            for (place, candidate) in self.candidates.iter().enumerate() {
                let mut written = candidates.object();
                {
                    let mut baseline = written.object("baseline");
                    let mut deviations = baseline.list("deviations");
                    for held in candidate.baseline {
                        match held {
                            Some(deviation) => deviations.int(deviation),
                            None => deviations.raw("null"),
                        };
                    }
                    deviations.end();
                    baseline.end();
                }
                written.int("position", place as i64 + 1);
                {
                    let mut privilege = written.object("privilege");
                    for (name, value) in PRIVILEGE_VALUES.iter().zip(candidate.privilege.each()) {
                        privilege.raw(name, &written_value(value));
                    }
                    privilege.end();
                }
                {
                    let mut provenance = written.list("provenance");
                    for entry in &candidate.provenance {
                        let mut held = provenance.object();
                        held.int_or_null("detail", entry.detail);
                        held.text("source", entry.source.name());
                        held.end();
                    }
                    provenance.end();
                }
                written.int("tier", i64::from(candidate.tier));
                {
                    let mut view = String::new();
                    candidate.view.write(&mut view);
                    written.raw("view", &view);
                }
                written.end();
            }
            candidates.end();
        }
        match &self.deficiency_reason {
            Some(reason) => object.text("deficiency_reason", reason),
            None => object.null("deficiency_reason"),
        };
        object.bool("deficient", self.deficient);
        // Reserved for the budget-overrun contract: the goal that owns it
        // records the measured duration here on overrun, and until it lands
        // every record carries null — declared now so the record's key set
        // never moves under it.
        object.null("detail");
        {
            let mut discarded = object.list("discarded");
            for entry in &self.discarded {
                let mut written = discarded.object();
                written.int_or_null("detail", entry.detail);
                written.text("reason", &entry.reason);
                written.text("source", entry.source.name());
                written.end();
            }
            discarded.end();
        }
        {
            let mut dominance = object.list("dominance");
            for (first, second) in &self.dominance {
                let mut pair = dominance.object();
                pair.int("a", i64::from(*first));
                pair.int("b", i64::from(*second));
                pair.end();
            }
            dominance.end();
        }
        {
            let mut forecast = object.object("forecast");
            forecast.int("depth", i64::from(self.forecast_depth));
            {
                let mut envelope = forecast.list("envelope");
                for (low, high) in &self.forecast_envelope {
                    let mut step = envelope.object();
                    step.int("hi", *high);
                    step.int("lo", *low);
                    step.end();
                }
                envelope.end();
            }
            forecast.end();
        }
        {
            let mut omitted = object.object("omitted");
            omitted.int("authored", i64::from(self.omitted[2]));
            omitted.int("clusters", i64::from(self.omitted[1]));
            omitted.int("drawn", i64::from(self.omitted[0]));
            omitted.int("responses", i64::from(self.omitted[3]));
            omitted.end();
        }
        object.int("ordinal", i64::from(self.ordinal));
        {
            let mut sensitivity = object.object("sensitivity");
            {
                let mut changed = sensitivity.list("changed_at");
                for name in &self.sensitivity_changed_at {
                    changed.text(name);
                }
                changed.end();
            }
            sensitivity.bool("flag", self.sensitivity);
            sensitivity.end();
        }
        object.raw("sigma", &self.sigma.write());
        {
            let mut standing = object.object("standing_intake");
            standing.bool("fallback", self.standing_fallback);
            match &self.standing_reason {
                Some(reason) => standing.text("reason", reason),
                None => standing.null("reason"),
            };
            standing.int("removed", i64::from(self.standing_removed));
            standing.end();
        }
        object.int("step", i64::from(self.step));
        object.int("tau", self.tau);
        object.int("window_declared", i64::from(self.window_declared));
        object.int("window_effective", i64::from(self.window_effective));
        object.end();
    }

    pub fn written(&self) -> String {
        let mut out = String::new();
        self.write(&mut out);
        out
    }

    /// Reads a record back out of a payload.
    ///
    /// Every slot is read as this build writes it, which is what the
    /// byte-equivalence contract asks of a reader: a value it admits is a value
    /// it can re-serialize to the same bytes. So the four privilege values are
    /// read whole — assigned with a confidence range that contains them, or
    /// unassigned with a reason from the closed set and no numbers at all —
    /// and the tier, the dominance relation, the sensitivity flag, the baseline
    /// deviations, and the forecast envelope with them. The one slot still held
    /// to a single value is the reserved `detail`, which no build writes yet.
    pub fn read(value: &Json, key: &str) -> Result<Self, Fault> {
        let found = read::map(value, key)?;
        read::exact_keys(
            found,
            key,
            &[
                "absent",
                "candidates",
                "deficiency_reason",
                "deficient",
                "detail",
                "discarded",
                "dominance",
                "forecast",
                "omitted",
                "ordinal",
                "sensitivity",
                "sigma",
                "standing_intake",
                "step",
                "tau",
                "window_declared",
                "window_effective",
            ],
        )?;

        let mut absent = Vec::new();
        for entry in read::list(found, "absent", 3)? {
            read::exact_keys(entry, "absent", &["condition", "variant"])?;
            let variant = match read::one_of(entry, "variant", &["coarser", "finer", "lateral"])? {
                0 => Source::Coarser,
                1 => Source::Finer,
                _ => Source::Lateral,
            };
            absent
                .push(Absence { variant, condition: read::text(entry, "condition")?.to_string() });
        }

        let mut candidates: Vec<Candidate> = Vec::new();
        for entry in read::list(found, "candidates", SLATE_CAP)? {
            read::exact_keys(
                entry,
                "candidates",
                &["baseline", "position", "privilege", "provenance", "tier", "view"],
            )?;
            let held = read::map(entry, "baseline")?;
            read::exact_keys(held, "baseline", &["deviations"])?;
            let read_back = read::list(held, "deviations", BASELINE_SAMPLES)?;
            if read_back.len() != BASELINE_SAMPLES {
                return Err(Fault::field("deviations"));
            }
            let mut baseline = [None; BASELINE_SAMPLES];
            for (slot, deviation) in baseline.iter_mut().zip(read_back.iter()) {
                *slot = match deviation {
                    Json::Null => None,
                    Json::Int(number) if (0..=FRAC_ONE).contains(number) => Some(*number),
                    _ => return Err(Fault::field("deviations")),
                };
            }
            // A window is either long enough to compare direction symbols or it
            // is not, so the eight slots are all filled or all empty.
            if baseline.iter().any(Option::is_some) && baseline.iter().any(Option::is_none) {
                return Err(Fault::field("deviations"));
            }
            if read::int(entry, "position", 1, SLATE_CAP as i64)? as usize != candidates.len() + 1 {
                return Err(Fault::field("position"));
            }
            let read_privilege = read::map(entry, "privilege")?;
            read::exact_keys(read_privilege, "privilege", &PRIVILEGE_VALUES)?;
            let privilege = PrivilegeProfile {
                boundary_sufficiency: read_value(read_privilege, "boundary_sufficiency")?,
                cut_impact: read_value(read_privilege, "cut_impact")?,
                scale_stability: read_value(read_privilege, "scale_stability")?,
                shared_failure: read_value(read_privilege, "shared_failure")?,
            };
            let tier = read::int(entry, "tier", 0, SLATE_CAP as i64)? as u8;
            let mut provenance = Vec::new();
            for held in read::list(entry, "provenance", SOURCES)? {
                read::exact_keys(held, "provenance", &["detail", "source"])?;
                let source = Source::read(read::text(held, "source")?)
                    .ok_or_else(|| Fault::field("source"))?;
                provenance.push(Provenance {
                    source,
                    detail: read::int_or_null(held, "detail", i64::MIN, i64::MAX)?,
                });
            }
            if provenance.is_empty() {
                return Err(Fault::field("provenance"));
            }
            let view = ViewDeclaration::read(entry, "view")?;
            if view.inside.is_empty() {
                return Err(Fault::field("inside"));
            }
            candidates.push(Candidate { view, provenance, privilege, tier, baseline });
        }
        if candidates.is_empty() {
            return Err(Fault::field("candidates"));
        }

        let mut discarded = Vec::new();
        for entry in read::list(found, "discarded", DISCARDS_RETAINED)? {
            read::exact_keys(entry, "discarded", &["detail", "reason", "source"])?;
            let source =
                Source::read(read::text(entry, "source")?).ok_or_else(|| Fault::field("source"))?;
            discarded.push(Discard {
                source,
                detail: read::int_or_null(entry, "detail", i64::MIN, i64::MAX)?,
                reason: read::text(entry, "reason")?.to_string(),
            });
        }

        // The relation is every ordered pair a candidate dominates another at,
        // ascending, with no pair naming one candidate twice over and no
        // candidate dominating itself.
        let mut dominance: Vec<(u8, u8)> = Vec::new();
        for entry in read::list(found, "dominance", SLATE_CAP * SLATE_CAP)? {
            read::exact_keys(entry, "dominance", &["a", "b"])?;
            let pair = (
                read::int(entry, "a", 1, candidates.len() as i64)? as u8,
                read::int(entry, "b", 1, candidates.len() as i64)? as u8,
            );
            if pair.0 == pair.1 || dominance.last().is_some_and(|held| *held >= pair) {
                return Err(Fault::field("dominance"));
            }
            dominance.push(pair);
        }
        if !read::at(found, "detail")?.is_null() {
            return Err(Fault::field("detail"));
        }

        let forecast = read::map(found, "forecast")?;
        read::exact_keys(forecast, "forecast", &["depth", "envelope"])?;
        let mut forecast_envelope = Vec::new();
        // One entry per window step, and the declared window is capped at 60,
        // so a longer list is a record this build never writes.
        for entry in
            read::list(forecast, "envelope", usize::from(*crate::field::WINDOW_RANGE.end()))?
        {
            read::exact_keys(entry, "envelope", &["hi", "lo"])?;
            let high = read::int(entry, "hi", i64::MIN, i64::MAX)?;
            let low = read::int(entry, "lo", i64::MIN, i64::MAX)?;
            if low > high {
                return Err(Fault::field("envelope"));
            }
            forecast_envelope.push((low, high));
        }

        let held = read::map(found, "sensitivity")?;
        read::exact_keys(held, "sensitivity", &["changed_at", "flag"])?;
        let mut sensitivity_changed_at: Vec<&'static str> = Vec::new();
        for entry in read::list(held, "changed_at", 2)? {
            let name = match entry.as_text() {
                Some(SENSITIVITY_HALF) => SENSITIVITY_HALF,
                Some(SENSITIVITY_DOUBLE) => SENSITIVITY_DOUBLE,
                _ => return Err(Fault::field("changed_at")),
            };
            if sensitivity_changed_at.contains(&name) {
                return Err(Fault::field("changed_at"));
            }
            sensitivity_changed_at.push(name);
        }
        let sensitivity = read::flag(held, "flag")?;
        // The flag is what the list says: a recomputation that changed the
        // nondominated set is named, and one that changed nothing is not.
        if sensitivity != !sensitivity_changed_at.is_empty() {
            return Err(Fault::field("sensitivity"));
        }

        let omitted = read::map(found, "omitted")?;
        read::exact_keys(omitted, "omitted", &["authored", "clusters", "drawn", "responses"])?;
        let counts = [
            read::int(omitted, "drawn", 0, i64::from(u16::MAX))? as u16,
            read::int(omitted, "clusters", 0, i64::from(u16::MAX))? as u16,
            read::int(omitted, "authored", 0, i64::from(u16::MAX))? as u16,
            read::int(omitted, "responses", 0, i64::from(u16::MAX))? as u16,
        ];

        let standing = read::map(found, "standing_intake")?;
        read::exact_keys(standing, "standing_intake", &["fallback", "reason", "removed"])?;
        let fallback = read::flag(standing, "fallback")?;
        let reason = match read::at(standing, "reason")? {
            Json::Null => None,
            _ => Some(read::text(standing, "reason")?.to_string()),
        };
        if fallback != (reason.as_deref() == Some(STANDING_VANISHED)) {
            return Err(Fault::field("standing_intake"));
        }

        let deficient = read::flag(found, "deficient")?;
        let deficiency_reason = match read::at(found, "deficiency_reason")? {
            Json::Null => None,
            _ => Some(read::text(found, "deficiency_reason")?.to_string()),
        };
        if deficient != (candidates.len() < 2) || deficient != deficiency_reason.is_some() {
            return Err(Fault::field("deficient"));
        }

        Ok(CandidateSlate {
            ordinal: read::int(found, "ordinal", 0, i64::from(u32::MAX))? as u32,
            step: read::int(found, "step", 0, i64::from(u32::MAX))? as Step,
            sigma: RngState::read(found, "sigma")?,
            tau: read::int(found, "tau", 1, FRAC_ONE / 2)?,
            window_declared: read::int(found, "window_declared", 0, i64::from(u16::MAX))? as u16,
            window_effective: read::int(found, "window_effective", 0, i64::from(u16::MAX))? as u16,
            candidates,
            deficient,
            deficiency_reason,
            omitted: counts,
            discarded,
            absent,
            standing_removed: read::int(standing, "removed", 0, i64::from(u16::MAX))? as u16,
            standing_fallback: fallback,
            standing_reason: reason,
            forecast_depth: read::int(forecast, "depth", 0, i64::from(u16::MAX))? as u16,
            dominance,
            sensitivity,
            sensitivity_changed_at,
            forecast_envelope,
        })
    }

    /// The candidates no candidate of the slate dominates: tier 1, read off the
    /// recorded relation. A deficient slate runs no comparison, so it has none.
    pub fn nondominated(&self) -> Vec<u8> {
        (1..=self.candidates.len() as u8)
            .filter(|_| !self.deficient)
            .filter(|position| !self.dominance.iter().any(|(_, held)| held == position))
            .collect()
    }
}

/// The four privilege values, in the ascending key order the record writes
/// them in.
pub(crate) const PRIVILEGE_VALUES: [&str; 4] =
    ["boundary_sufficiency", "cut_impact", "scale_stability", "shared_failure"];

/// One privilege value, as ARCHITECTURE.md shapes it: the confidence range,
/// the reason it is unassigned or null, the defined-sample count, and the
/// number itself.
pub(crate) fn written_value(value: &PrivilegeValue) -> String {
    let mut out = String::new();
    let mut object = Obj::new(&mut out);
    object.int_or_null("high", value.high);
    object.int_or_null("low", value.low);
    match value.reason {
        Some(reason) => object.text("reason", reason),
        None => object.null("reason"),
    };
    object.int("samples", i64::from(value.samples));
    object.int_or_null("value", value.value);
    object.end();
    out
}

/// Reads one privilege value back, held to the two shapes it can take: a
/// number inside a confidence range that contains it, or no numbers at all and
/// a reason from the closed set.
fn read_value(value: &Json, key: &str) -> Result<PrivilegeValue, Fault> {
    let found = read::map(value, key)?;
    read::exact_keys(found, key, &["high", "low", "reason", "samples", "value"])?;
    let high = read::int_or_null(found, "high", 0, FRAC_ONE)?;
    let low = read::int_or_null(found, "low", 0, FRAC_ONE)?;
    let held = read::int_or_null(found, "value", 0, FRAC_ONE)?;
    let samples = read::int(found, "samples", 0, BASELINE_SAMPLES as i64)? as u16;
    let reason = match read::at(found, "reason")? {
        Json::Null => None,
        _ => {
            let name = read::text(found, "reason")?;
            Some(
                *UNASSIGNED_REASONS
                    .iter()
                    .find(|held| **held == name)
                    .ok_or_else(|| Fault::field("reason"))?,
            )
        }
    };
    match (held, low, high, reason) {
        (Some(number), Some(low), Some(high), None) if low <= number && number <= high => {
            Ok(PrivilegeValue::assigned(number, low, high, samples))
        }
        (None, None, None, Some(reason)) if samples == 0 => {
            Ok(PrivilegeValue::unassigned(reason))
        }
        _ => Err(Fault::field(key)),
    }
}

/// `sigma_V` for the evaluation at one assembly ordinal:
/// `split(sigma_run, "evaluation", n)`, exactly as ARCHITECTURE.md's roots and
/// partitioning locks it.
pub fn evaluation_stream(run_id: &str, branch_nonce: u32, ordinal: u32) -> RngState {
    crate::rng::run_stream(run_id, branch_nonce)
        .split(&[Part::Name("evaluation"), Part::Number(i64::from(ordinal))])
}

/// The tables every source reads, computed once per assembly.
///
/// Each is a pass over the Field at its own cap, and every list is indexed by
/// the Node's place in the ascending Node list rather than by its identifier,
/// so every ordering rule in the section — "ascending identifier order" — is
/// the natural order of these arrays.
struct Tables {
    /// The current Node set N, ascending.
    nodes: Vec<u32>,
    /// The declared adjacency, as a square table over Node places.
    adjacency: Vec<bool>,
    /// Whether a Route stands between two Node places, in either direction.
    links: Vec<bool>,
    /// Every Route as (tail place, head place, window flow `Fw(r)`).
    routes: Vec<(usize, usize, Fx)>,
    /// Stored Charge per Node place over the window's steps.
    q: Vec<Vec<Fx>>,
    /// The failure indicator per Node place over the window's steps.
    z: Vec<Vec<bool>>,
    /// The effective window: how many steps the tables above cover.
    window: usize,
    tau: Frac,
}

impl Tables {
    fn build(field: &FieldState, window: &[&TraceStep], tau: Frac) -> Self {
        let nodes: Vec<u32> = field.ports.iter().map(|port| port.node).collect();
        let count = nodes.len();

        let mut adjacency = vec![false; count * count];
        for first in 0..count {
            for second in (first + 1)..count {
                let a = &field.ports[first];
                let b = &field.ports[second];
                if crate::fx::adjacent(a.pos, a.layer, b.pos, b.layer) {
                    adjacency[first * count + second] = true;
                    adjacency[second * count + first] = true;
                }
            }
        }

        // Window flow per Route, over the window's steps. A Route the window
        // recorded but the Field no longer holds is not a Route of S, and a
        // Route formed inside the window simply carries the steps it ran.
        let mut flows = vec![0 as Fx; field.routes.len()];
        for step in window {
            for (route, moved) in &step.records.f {
                if let Ok(place) = field.routes.binary_search_by_key(route, |held| held.route) {
                    flows[place] = flows[place].saturating_add(*moved);
                }
            }
        }
        let mut links = vec![false; count * count];
        let mut routes = Vec::with_capacity(field.routes.len());
        for (route, flow) in field.routes.iter().zip(flows.iter()) {
            let (Ok(tail), Ok(head)) =
                (nodes.binary_search(&route.tail), nodes.binary_search(&route.head))
            else {
                continue;
            };
            links[tail * count + head] = true;
            links[head * count + tail] = true;
            routes.push((tail, head, *flow));
        }

        let mut q = vec![vec![0 as Fx; window.len()]; count];
        let mut z = vec![vec![false; window.len()]; count];
        for (index, step) in window.iter().enumerate() {
            for (node, stored) in &step.records.q {
                if let Ok(place) = nodes.binary_search(node) {
                    q[place][index] = *stored;
                }
            }
            for node in &step.records.z {
                if let Ok(place) = nodes.binary_search(node) {
                    z[place][index] = true;
                }
            }
        }

        Tables { nodes, adjacency, links, routes, q, z, window: window.len(), tau }
    }

    fn count(&self) -> usize {
        self.nodes.len()
    }

    /// Whether two Node places stand in the declared structural neighborhood,
    /// which is the adjacency relation and nothing else.
    fn adjacent(&self, first: usize, second: usize) -> bool {
        self.adjacency[first * self.count() + second]
    }

    /// Whether a Route stands between two Node places, in either direction.
    fn linked(&self, first: usize, second: usize) -> bool {
        self.links[first * self.count() + second]
    }

    /// The surround set U(V) of an inside under a surround rule: the named set
    /// of non-members, computed at the evaluation step from the current Route
    /// set and the declared adjacency.
    fn surround(&self, inside: &[bool], rule: Surround) -> Vec<bool> {
        match rule {
            Surround::Whole => (0..self.count()).map(|place| !inside[place]).collect(),
            Surround::Adjacent => (0..self.count())
                .map(|place| !inside[place] && self.touches(place, inside))
                .collect(),
            Surround::Double => {
                let near: Vec<bool> = (0..self.count())
                    .map(|place| !inside[place] && self.touches(place, inside))
                    .collect();
                (0..self.count())
                    .map(|place| {
                        !inside[place] && (near[place] || self.touches(place, &near))
                    })
                    .collect()
            }
        }
    }

    /// Whether a Node has a Route to or from a marked set, or a declared
    /// adjacency to it.
    fn touches(&self, place: usize, marked: &[bool]) -> bool {
        (0..self.count()).any(|other| {
            marked[other]
                && other != place
                && (self.adjacent(place, other) || self.linked(place, other))
        })
    }

    /// The shell of an inside: members with a crossing Route or a declared
    /// adjacency to a non-member.
    fn shell(&self, inside: &[bool]) -> Vec<bool> {
        (0..self.count())
            .map(|place| {
                inside[place]
                    && (0..self.count()).any(|other| {
                        !inside[other]
                            && (self.adjacent(place, other) || self.linked(place, other))
                    })
            })
            .collect()
    }

    /// The internal attachment `A_in(n)`: the sum of `Fw(r)` over Routes with
    /// both endpoints in the inside that touch n.
    fn internal_attachment(&self, inside: &[bool]) -> Vec<Fx> {
        let mut found = vec![0 as Fx; self.count()];
        for (tail, head, flow) in &self.routes {
            if !inside[*tail] || !inside[*head] {
                continue;
            }
            found[*tail] = found[*tail].saturating_add(*flow);
            if tail != head {
                found[*head] = found[*head].saturating_add(*flow);
            }
        }
        found
    }

    /// The boundary attachment `A_bd(m)`: the sum of `Fw(r)` over Routes
    /// between a non-member and any member, either direction.
    fn boundary_attachment(&self, inside: &[bool]) -> Vec<Fx> {
        let mut found = vec![0 as Fx; self.count()];
        for (tail, head, flow) in &self.routes {
            if inside[*tail] && !inside[*head] {
                found[*head] = found[*head].saturating_add(*flow);
            } else if inside[*head] && !inside[*tail] {
                found[*tail] = found[*tail].saturating_add(*flow);
            }
        }
        found
    }

    /// The members of a marked set, as identifiers, ascending.
    fn members(&self, marked: &[bool]) -> Vec<u32> {
        (0..self.count()).filter(|place| marked[*place]).map(|place| self.nodes[place]).collect()
    }

    /// The pair weight table of source 2: `W(a, b)` over unordered Node pairs,
    /// as a square table over Node places.
    ///
    /// A pair is two distinct Nodes, so a Route from a Node to itself enters no
    /// pair weight: the enumeration has no `{a, a}` to put it in, and the
    /// section admits only components of two or more Nodes, which no self-loop
    /// can supply on its own. FRAMEWORK.md names a self-loop explicitly where
    /// it means one to count (Cut Impact's cycle rule) and does not name one
    /// here. The reading is isolated in this one function so a settled
    /// amendment moves one place.
    fn pair_weights(&self) -> Vec<Fx> {
        let count = self.count();
        let mut weights = vec![0 as Fx; count * count];
        for (tail, head, flow) in &self.routes {
            if tail == head {
                continue;
            }
            weights[tail * count + head] = weights[tail * count + head].saturating_add(*flow);
            weights[head * count + tail] = weights[head * count + tail].saturating_add(*flow);
        }
        weights
    }
}

/// The connected components, of two or more Nodes, of a graph over Node
/// places, each with the total of the edge quantity over its internal pairs.
///
/// The total is taken over every internal pair rather than over the graph's
/// own edges, which is the reading source 4 states outright — "descending
/// total of `co` over internal pairs" — and source 2 states in the same words.
fn components(count: usize, edges: &[(usize, usize)], weight: &[i64]) -> Vec<(Vec<usize>, i64)> {
    // The edge list is turned into neighbour lists first, so the walk below
    // costs one pass over the edges rather than one pass per member: the
    // response graph of a wide inside can hold every pair, and a walk that
    // rescanned the list per member would be cubic in the Node count.
    let mut near: Vec<Vec<usize>> = vec![Vec::new(); count];
    for (a, b) in edges {
        near[*a].push(*b);
        near[*b].push(*a);
    }
    let mut seen = vec![false; count];
    let mut found: Vec<Vec<usize>> = Vec::new();
    for place in 0..count {
        if seen[place] || near[place].is_empty() {
            continue;
        }
        let mut members = vec![place];
        seen[place] = true;
        let mut next = 0;
        while next < members.len() {
            let held = members[next];
            next += 1;
            for other in &near[held] {
                if !seen[*other] {
                    seen[*other] = true;
                    members.push(*other);
                }
            }
        }
        members.sort_unstable();
        found.push(members);
    }
    found
        .into_iter()
        .filter(|members| members.len() >= 2)
        .map(|members| {
            let mut total: i64 = 0;
            for (place, first) in members.iter().enumerate() {
                for second in members.iter().skip(place + 1) {
                    total = total.saturating_add(weight[first * count + second]);
                }
            }
            (members, total)
        })
        .collect()
}

/// Orders proposed insides descending by their total, and on equal totals by
/// the smallest identifier the component holds — which, over ascending member
/// lists, is its first member.
fn order_by_total(mut found: Vec<(Vec<usize>, i64)>) -> Vec<(Vec<usize>, i64)> {
    found.sort_by(|first, second| second.1.cmp(&first.1).then(first.0[0].cmp(&second.0[0])));
    found
}

/// One proposed inside on its way into the offer sequence, with every
/// provenance that reached it: a candidate offered by more than one source is
/// one offer, and the later sources' provenances merge onto it by the
/// duplicate rule rather than taking a second turn.
#[derive(Clone)]
struct Offer {
    members: Vec<u32>,
    provenance: Vec<Provenance>,
}

/// Assembles the slate for one evaluation.
///
/// The inputs are exactly FRAMEWORK.md's: the Field state, the standing View,
/// the assembly ordinal, the previous assembly's evaluation step, `sigma_V`,
/// and the tolerance. Nothing else is read, and nothing here draws.
pub fn assemble(state: &RunState) -> CandidateSlate {
    let field = &state.now;
    let ordinal = field.assembly_ordinal;
    let declared = state.view.window;
    let effective = state.effective_window(declared);
    let window: Vec<&TraceStep> = state
        .trace
        .steps
        .iter()
        .skip(state.trace.steps.len().saturating_sub(usize::from(effective)))
        .collect();
    let tables = Tables::build(field, &window, TAU_DEFAULT);

    // The standing inside passes intake first. When it empties, either because
    // the player cleared it or because every named Node vanished, the local
    // fallback `<N, rho0, w0, u0>` seeds this evaluation before anything else.
    // This never writes the fallback back into the active View.
    let taken = intake(&state.view.inside, &tables.nodes);
    let standing_removed = (state.view.inside.len() - taken.len()) as u16;
    let standing_fallback = taken.is_empty();
    let inside = if standing_fallback { tables.nodes.clone() } else { taken };
    let standing = ViewDeclaration {
        inside: inside.clone(),
        resolution: state.view.resolution,
        window: declared,
        surround: state.view.surround,
    };

    let mut slate = CandidateSlate {
        ordinal,
        step: field.step,
        sigma: evaluation_stream(&state.run_id, state.branch_nonce, ordinal),
        tau: TAU_DEFAULT,
        window_declared: declared,
        window_effective: effective,
        candidates: vec![fresh_candidate(
            standing.clone(),
            vec![Provenance { source: Source::Standing, detail: None }],
        )],
        deficient: false,
        deficiency_reason: None,
        omitted: [0; 4],
        discarded: Vec::new(),
        absent: Vec::new(),
        dominance: Vec::new(),
        sensitivity: false,
        sensitivity_changed_at: Vec::new(),
        forecast_envelope: Vec::new(),
        standing_removed,
        standing_fallback,
        standing_reason: standing_fallback.then(|| STANDING_VANISHED.to_string()),
        forecast_depth: field
            .forms
            .iter()
            .find(|form| form.controlled)
            .map_or(0, |form| form.forecast_depth),
    };

    let mut mask = vec![false; tables.count()];
    for member in &inside {
        if let Ok(place) = tables.nodes.binary_search(member) {
            mask[place] = true;
        }
    }

    // Step 2: the permitted variants, in the order finer, coarser, lateral.
    for (variant, proposed, condition) in variants(&tables, &mask) {
        match proposed {
            Some(members) => append(
                &mut slate,
                &standing,
                members,
                Provenance { source: variant, detail: None },
            ),
            None => {
                slate.absent.push(Absence { variant, condition: condition.to_string() });
            }
        }
    }

    // Step 3: the offer sequence — fresh drawn entries first, then the four
    // situational sources interleaved in the rotated cycle.
    let offers = offer_sequence(state, &tables, &mask, &mut slate);

    // Step 4: process it in order. An offer equal to an entry already in the
    // slate merges every provenance it carries; otherwise it is appended
    // while a seat stands open, and counted as omitted once none does — once
    // per offer, for the source that offered it first, however many sources
    // reached it.
    for offer in offers {
        let view = ViewDeclaration { inside: offer.members, ..standing.clone() };
        if let Some(held) = slate.candidates.iter_mut().find(|entry| same_view(&entry.view, &view))
        {
            for provenance in offer.provenance {
                if !held.provenance.contains(&provenance) {
                    held.provenance.push(provenance);
                }
            }
        } else if slate.candidates.len() < SLATE_CAP {
            slate.candidates.push(fresh_candidate(view, offer.provenance));
        } else if let Some(slot) = offer.provenance.first().and_then(|held| held.source.slot()) {
            slate.omitted[slot] = slate.omitted[slot].saturating_add(1);
        }
    }

    if slate.candidates.len() < 2 {
        slate.deficient = true;
        slate.deficiency_reason = Some(NO_ALTERNATIVE.to_string());
    }
    slate
}

/// One candidate as assembly leaves it: the View, its provenance, and the four
/// values still to be read.
///
/// Assembly and evaluation are two passes over one record. Assembly names the
/// candidates; [`crate::rank::evaluate`] reads them, and it is the only thing
/// that ever assigns a value, a tier, a deviation, or a place in the dominance
/// relation. The form left here is the one an evaluation that can read nothing
/// leaves standing — every value unassigned for the one reason a window of no
/// steps gives — so a record is never half-shaped, whichever pass last touched
/// it.
fn fresh_candidate(view: ViewDeclaration, provenance: Vec<Provenance>) -> Candidate {
    Candidate {
        view,
        provenance,
        privilege: PrivilegeProfile::unassigned(WINDOW_TOO_SHORT),
        tier: 0,
        baseline: [None; BASELINE_SAMPLES],
    }
}

/// Appends a proposed inside under the duplicate rule: a View already in the
/// slate takes the new provenance and no new entry is added.
fn append(
    slate: &mut CandidateSlate,
    standing: &ViewDeclaration,
    members: Vec<u32>,
    provenance: Provenance,
) {
    let view = ViewDeclaration { inside: members, ..standing.clone() };
    match slate.candidates.iter_mut().find(|entry| same_view(&entry.view, &view)) {
        Some(held) => {
            if !held.provenance.contains(&provenance) {
                held.provenance.push(provenance);
            }
        }
        None => slate.candidates.push(fresh_candidate(view, vec![provenance])),
    }
}

/// Two candidate Views are the same View exactly when all four components are
/// equal: the same inside as a set, the same resolution, the same window, and
/// the same surround rule.
fn same_view(first: &ViewDeclaration, second: &ViewDeclaration) -> bool {
    first.inside == second.inside
        && first.resolution == second.resolution
        && first.window == second.window
        && first.surround == second.surround
}

/// The framework's intake: the intersection with the current Node set,
/// ascending, with duplicates removed.
fn intake(proposed: &[u32], nodes: &[u32]) -> Vec<u32> {
    let mut found: Vec<u32> =
        proposed.iter().copied().filter(|node| nodes.binary_search(node).is_ok()).collect();
    found.sort_unstable();
    found.dedup();
    found
}

/// The three inside-axis variants, in the order the assembly appends them,
/// each with the inside it proposes or the permitting condition that failed.
fn variants(tables: &Tables, inside: &[bool]) -> Vec<(Source, Option<Vec<u32>>, &'static str)> {
    let count = tables.count();
    let size = inside.iter().filter(|held| **held).count();
    let attachment = tables.internal_attachment(inside);
    let mut found = Vec::new();

    // Finer: permitted exactly when |I0| >= 2. The interior is the member set
    // minus the shell; when it is nonempty and differs from I0 it is the finer
    // inside, and otherwise the weakest member is dropped — the first member in
    // ascending `(A_in, identifier)` order.
    if size >= 2 {
        let shell = tables.shell(inside);
        let interior: Vec<bool> = (0..count).map(|place| inside[place] && !shell[place]).collect();
        let held = interior.iter().filter(|held| **held).count();
        if held > 0 && held < size {
            found.push((Source::Finer, Some(tables.members(&interior)), ""));
        } else {
            let weakest = (0..count)
                .filter(|place| inside[*place])
                .min_by_key(|place| (attachment[*place], tables.nodes[*place]))
                .expect("an inside of two or more members holds a weakest one");
            let mut without = inside.to_vec();
            without[weakest] = false;
            found.push((Source::Finer, Some(tables.members(&without)), ""));
        }
    } else {
        found.push((Source::Finer, None, "|I0| < 2"));
    }

    // Coarser: I0 plus every non-member with a Route to or from a member or a
    // declared adjacency to a member, permitted exactly when that differs.
    let near = tables.surround(inside, Surround::Adjacent);
    let wider: Vec<bool> = (0..count).map(|place| inside[place] || near[place]).collect();
    if wider.iter().filter(|held| **held).count() > size {
        found.push((Source::Coarser, Some(tables.members(&wider)), ""));
    } else {
        found.push((Source::Coarser, None, "C1 = I0"));
    }

    // Laterally shifted: the weakest k' members out, the k' most attached
    // qualifying non-members in.
    let boundary = tables.boundary_attachment(inside);
    let qualifying: Vec<usize> =
        (0..count).filter(|place| !inside[*place] && boundary[*place] > 0).collect();
    let shift = 1.max(size.div_ceil(4)).min(qualifying.len()).min(size.saturating_sub(1));
    if shift >= 1 {
        let mut out: Vec<usize> = (0..count).filter(|place| inside[*place]).collect();
        out.sort_by_key(|place| (attachment[*place], tables.nodes[*place]));
        let mut into = qualifying;
        into.sort_by_key(|place| (-boundary[*place], tables.nodes[*place]));
        let mut shifted = inside.to_vec();
        for place in out.iter().take(shift) {
            shifted[*place] = false;
        }
        for place in into.iter().take(shift) {
            shifted[*place] = true;
        }
        found.push((Source::Lateral, Some(tables.members(&shifted)), ""));
    } else {
        found.push((Source::Lateral, None, "k' = 0"));
    }

    found
}

/// Builds the offer sequence: every fresh drawn entry first, most recent
/// first, then the four situational sources interleaved in the rotated cycle
/// that starts at source `1 + (n mod 4)`.
///
/// Intake runs on every proposal here, and a proposal it empties is discarded
/// with its reason rather than offered. A candidate already offered is skipped
/// rather than offered twice — which is what keeps a fresh drawn entry from
/// being offered again when its own source's turn comes.
fn offer_sequence(
    state: &RunState,
    tables: &Tables,
    inside: &[bool],
    slate: &mut CandidateSlate,
) -> Vec<Offer> {
    let field = &state.now;
    let ordinal = field.assembly_ordinal;

    let drawn = collect(
        field
            .boundaries
            .drawn
            .iter()
            .enumerate()
            .map(|(place, entry)| (entry.members.clone(), Source::Drawn, Some(place as i64 + 1))),
        tables,
        slate,
    );
    let clusters = collect(
        strong_clusters(tables)
            .into_iter()
            .map(|(members, total)| (members, Source::Clusters, Some(total))),
        tables,
        slate,
    );
    let authored = collect(
        field.boundaries.authored.iter().enumerate().map(|(place, members)| {
            (members.clone(), Source::Authored, Some(place as i64 + 1))
        }),
        tables,
        slate,
    );
    let responses = collect(
        shared_responses(tables, inside, state.view.surround)
            .into_iter()
            .map(|(members, total)| (members, Source::Responses, Some(total))),
        tables,
        slate,
    );

    let mut sequence: Vec<Offer> = Vec::new();

    // A drawn entry is fresh when its recorded step is at or after the
    // evaluation step of the previous assembly — after a state restore, at or
    // after the step the restore returned to — or when this is the Run's
    // first assembly. At or after rather than strictly after, because a
    // still-mode drag necessarily shares the step of the assembly that opened
    // its session: a still run runs no step. The detail carries the entry's
    // own position in the list, so a discarded entry shifts nothing.
    for offer in &drawn {
        let place = offer.provenance[0].detail.unwrap_or_default() as usize;
        let Some(entry) = field.boundaries.drawn.get(place.saturating_sub(1)) else {
            continue;
        };
        if ordinal == 0 || field.prev_assembly_step.is_none_or(|before| entry.step >= before) {
            merge_or_push(&mut sequence, offer);
        }
    }

    let streams = [drawn, clusters, authored, responses];
    let mut next = [0usize; 4];
    let lead = (ordinal % 4) as usize;
    loop {
        let mut moved = false;
        for turn in 0..4 {
            let source = (lead + turn) % 4;
            while next[source] < streams[source].len() {
                let offer = &streams[source][next[source]];
                next[source] += 1;
                // A candidate already offered takes no second turn and no
                // second seat, and is never counted omitted a second time —
                // but its provenance is not dropped: it merges onto the offer
                // that stands, and reaches the slate with it through step 4's
                // duplicate branch. The source goes on to its next candidate
                // in the same turn.
                if merge_or_push(&mut sequence, offer) {
                    moved = true;
                    break;
                }
            }
        }
        if !moved {
            break;
        }
    }
    sequence
}

/// Appends an offer to the sequence, or — when its members are already
/// offered — merges its provenance onto the standing offer and appends
/// nothing. True exactly when a new offer was appended.
fn merge_or_push(sequence: &mut Vec<Offer>, offer: &Offer) -> bool {
    if let Some(held) = sequence.iter_mut().find(|held| held.members == offer.members) {
        for provenance in &offer.provenance {
            if !held.provenance.contains(provenance) {
                held.provenance.push(provenance.clone());
            }
        }
        return false;
    }
    sequence.push(offer.clone());
    true
}

/// Runs intake over one source's proposals, recording the discards.
fn collect(
    proposals: impl Iterator<Item = (Vec<u32>, Source, Option<i64>)>,
    tables: &Tables,
    slate: &mut CandidateSlate,
) -> Vec<Offer> {
    let mut found = Vec::new();
    for (members, source, detail) in proposals {
        let taken = intake(&members, &tables.nodes);
        if taken.is_empty() {
            if slate.discarded.len() < DISCARDS_RETAINED {
                slate.discarded.push(Discard {
                    source,
                    detail,
                    reason: INTAKE_EMPTY.to_string(),
                });
            }
            continue;
        }
        found.push(Offer { members: taken, provenance: vec![Provenance { source, detail }] });
    }
    found
}

/// Source 2: the strong route clusters, each with its total internal weight.
fn strong_clusters(tables: &Tables) -> Vec<(Vec<u32>, i64)> {
    let count = tables.count();
    let weights = tables.pair_weights();
    let mut positive: Vec<Fx> = Vec::new();
    for first in 0..count {
        for second in (first + 1)..count {
            let weight = weights[first * count + second];
            if weight > 0 {
                positive.push(weight);
            }
        }
    }
    // When no weight is positive the source yields nothing.
    if positive.is_empty() {
        return Vec::new();
    }
    positive.sort_unstable();
    // The median, kept in doubled units so an even count needs no division: a
    // pair is strong when `2 * W >= 2 * theta`, and the doubled median of an
    // even count is the sum of the two middle values.
    let middle = positive.len() / 2;
    let doubled = if positive.len() % 2 == 1 {
        positive[middle].saturating_mul(2)
    } else {
        positive[middle - 1].saturating_add(positive[middle])
    };
    let mut edges = Vec::new();
    for first in 0..count {
        for second in (first + 1)..count {
            let weight = weights[first * count + second];
            if weight > 0 && weight.saturating_mul(2) >= doubled {
                edges.push((first, second));
            }
        }
    }
    order_by_total(components(count, &edges, &weights))
        .into_iter()
        .map(|(members, total)| {
            (members.iter().map(|place| tables.nodes[*place]).collect(), total)
        })
        .collect()
}

/// Source 4: the repeated shared responses, each with its total co-response
/// count over internal pairs.
fn shared_responses(tables: &Tables, inside: &[bool], rule: Surround) -> Vec<(Vec<u32>, i64)> {
    // Only Nodes of `I0 union U(V0)` are considered, and a window of fewer
    // than three steps yields nothing.
    let count = tables.count();
    if tables.window < 3 {
        return Vec::new();
    }
    let near = tables.surround(inside, rule);
    let considered: Vec<usize> =
        (0..count).filter(|place| inside[*place] || near[*place]).collect();

    // A Node responds at a response step when it failed there, or when its
    // stored Charge moved by more than its own level margin. The margin is
    // `tau` times the range of that Node's series over the window, and the
    // comparison is scaled rather than divided so nothing rounds.
    //
    // One Node's response steps are held as the bits of one word. The declared
    // window is capped at 60 steps, so a window has at most 59 response steps
    // and every series fits: the co-response count of a pair is then one
    // bitwise and and one count of set bits, rather than a pass over the
    // window per pair.
    let steps = tables.window - 1;
    debug_assert!(steps <= 64, "the locked window cap keeps a response series inside one word");
    let mut responds = vec![0u64; count];
    for place in &considered {
        let series = &tables.q[*place];
        let low = series.iter().copied().min().unwrap_or(0);
        let high = series.iter().copied().max().unwrap_or(0);
        let margin = (high - low).saturating_mul(tables.tau);
        let mut held = 0u64;
        for index in 1..tables.window.min(65) {
            let moved = (series[index] - series[index - 1]).abs().saturating_mul(FRAC_ONE);
            if tables.z[*place][index] || moved > margin {
                held |= 1u64 << (index - 1);
            }
        }
        responds[*place] = held;
    }

    // The co-response count over every considered pair, and the repeat
    // threshold `theta_r = max(2, ceil((w0 - 1) / 8))`.
    let mut co = vec![0i64; count * count];
    let threshold = 2.max(steps.div_ceil(8)) as i64;
    let mut edges = Vec::new();
    for (place, first) in considered.iter().enumerate() {
        for second in considered.iter().skip(place + 1) {
            let together = i64::from((responds[*first] & responds[*second]).count_ones());
            co[first * count + second] = together;
            co[second * count + first] = together;
            if together >= threshold {
                edges.push((*first, *second));
            }
        }
    }
    order_by_total(components(count, &edges, &co))
        .into_iter()
        .map(|(members, total)| {
            (members.iter().map(|place| tables.nodes[*place]).collect(), total)
        })
        .collect()
}
