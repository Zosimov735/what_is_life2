//! Authored content: the bundle the worker hands over, and the chapter script.
//!
//! `docs/field-framework/ARCHITECTURE.md` (Authored content) locks where the
//! files live, what the manifest holds, and the rule the content hash follows:
//! SHA-256 over the concatenation of the manifest bytes and every listed file's
//! bytes in manifest order, computed by the build, embedded, and reported by
//! `init_run`. The worker imports those bytes statically and hands them across
//! with the digest the build wrote; what stands here recomputes the digest over
//! exactly the bytes it was handed and refuses the bundle when the two
//! disagree, so an embedded digest that has gone stale cannot pass quietly.
//! Nothing is fetched: the bundle arrives with the core, in the same call that
//! settles the version handshake.
//!
//! The locked chapter fields are the document's — `id`, `title_key`, `layers`,
//! `ports`, `routes`, `currents`, `authored_boundaries`, `objectives`,
//! `pressure_schedule`, `anchor_moments`, and `opening_view`. The document
//! leaves the interior of those fields to the goal that first reads them; this
//! is that goal for everything the opening sequence needs, and what it declares
//! is below, under `content_version` 1. Two fields beyond the locked minimum are
//! declared here for the same reason: `forms`, the chapter's placement of the
//! controlled Form.
//!
//! The Form file's interior is declared here for the same reason. The document
//! names what it holds — starting parameters, `route_reach`, `forecast_depth`
//! (only `lens` above 0 in version 1), `leak_frac`, upkeep rates, and
//! rule-changing abilities as data — and leaves the shape to the goal that
//! first reads it. Version 1 is exactly eight keys: `id`, `route_reach`,
//! `forecast_depth`, `leak_frac`, `upkeep_rate`, `capacity`, `reserve`, and
//! `abilities`. Each of the six quantities is copied into the state the run
//! stands on when the Field is established, and nothing anywhere branches on
//! which Form was selected: the Forms differ because their data differs, which
//! is the whole of what makes eight Forms eight rather than eight branches.
//!
//! Two of the eight decide something a chapter must be authored against, so the
//! policy is stated here rather than left in the numbers:
//!
//! - `route_reach` is how far apart the two ends of a Route the player forms
//!   may stand. It is measured between the Nodes, not from the Form, so it
//!   bounds the length of a link rather than the range of an act. A chapter has
//!   to place at least one pair of Nodes inside the shortest authored reach, or
//!   the Form carrying it can form nothing at all there; and any link a
//!   chapter's own objectives require has to stand inside that same shortest
//!   reach, or the chapter is one no Form completes. The locked distance rule
//!   adds 512 units per layer crossed, so a cross-layer link asks a reach that
//!   much wider again. `core/tests/forms.rs` reads the standing counts off the
//!   authored chapter and pins them, which is where the figures live.
//! - `capacity` is the Form's own overload threshold, and it is what a Form can
//!   stand on: above it a Node sheds a quarter of the excess every step. A
//!   chapter that asks a Form to carry more than the smallest authored
//!   threshold is asking something only some Forms can do.
//!
//! The objective script is the other half. It is a pure function of the state,
//! the step's own reading, and the authored condition, and every value it keeps
//! between steps lives in `progress` — the payload's own field — so a restore
//! lands exactly where the record was taken. That is why progress is carried as
//! the locked `Frac` and advanced by a derived per-step share rather than as a
//! step counter: a `Frac` round-trips through the payload exactly, and a
//! counter the payload could not carry would be a counter a restore could not
//! land on.
//!
//! The campaign is the sequence of those chapters. Three fields carry it, and
//! all three are declared here under `content_version` 1:
//!
//! - `optional` on an objective, which is the span an optional test stands for.
//!   A required objective authors `null` and blocks the chapter until it is
//!   met; an optional one authors a span in steps and is passed over when the
//!   span runs out, so the chapter carries on without it. The pass-over needs
//!   no field of its own, because the two the payload already holds decide it:
//!   an objective earlier in the authored order than the standing one, and not
//!   in `progress.complete`, is one the sequence has already passed over.
//! - `events`, the authored disruptions a chapter times against its own
//!   objectives. Each names the objective it lands during and how many steps
//!   after that objective was offered it falls due, so the trigger is a pure
//!   function of `progress.objective.started_step` and the step counter — both
//!   payload fields — and needs no state of its own either.
//! - `ending_key`, the copy-catalog key the run reports when the campaign ends
//!   on this chapter. Every chapter declares one, so a campaign truncated for
//!   testing still reaches an ending rather than stopping at a chapter that
//!   names none.
//! - `ending_marks`, the optional list that turns that one key into a family of
//!   them. A chapter that authors none reports `ending_key` exactly as before;
//!   a chapter that authors marks reports
//!   `<ending_key>.<form>.<mark>` — the Form the run opened on, and the first
//!   authored mark whose named objective the run completed. It is what lets one
//!   chapter close on an ending that names what this run built rather than on
//!   one sentence every run reads. See [`Chapter::ending_id`].

use crate::fault::{Code, Fault};
use crate::field::{
    self, BoundaryState, CurrentState, Cue, FieldLayer, FormState, LinkState, NodeKind, PortState,
    RouteState, TrailState, CUE_COLLAPSE, CUE_OBJECTIVE_COMPLETE, CUE_RECOVERY,
    CURRENTS_PER_CHAPTER, LAYERS_PER_CHAPTER, NODES_PER_RUN, ROUTES_PER_RUN,
};
use crate::fx::{within, Vec2, ONE_UNIT, STORED_BOUND};
use crate::json::{parse, Json};
use crate::read;
use crate::sha256;
use crate::state::{
    FieldState, Frac, Fx, ObjectiveStage, ObjectiveState, Progress, Step, Surround,
    ViewDeclaration, FRAC_ONE,
};

/// The content version this build reads.
pub const CONTENT_VERSION: i64 = 1;

/// The three manifest lists, in the order the concatenation reads them — the
/// same order `tools/content-build.mjs` hashes in.
const MANIFEST_LISTS: [&str; 3] = ["chapters", "forms", "pressures"];

/// How many files one manifest may list. The eight chapters, the eight Forms,
/// and the six pressures come to twenty-two; the cap stands well above that so
/// a manifest is bounded without being crowded, and it is named here rather
/// than borrowed from a cap that means something else.
pub const MANIFEST_FILES_CAP: usize = 64;

/// How many objectives one chapter may author. A chapter is a sequence a
/// player reads one line of at a time, so the cap is a readable count rather
/// than a machine one.
pub const OBJECTIVES_PER_CHAPTER: usize = 64;

/// The eight chapter ids, closed by the document.
pub const CHAPTER_IDS: [&str; 8] = [
    "the_pull",
    "the_edge",
    "the_loop",
    "the_echo",
    "the_mesh",
    "the_break",
    "the_rewrite",
    "the_quiet_edge",
];

/// One starting Form's authored parameters.
///
/// Every one of them is authored per Form and copied into the state the run
/// stands on when the Field is established: `route_reach`, `forecast_depth`,
/// and `steer_scale` onto the Form, `leak_frac` onto the run's Boundary, and
/// `upkeep_rate`, `capacity`, and `reserve` onto the Form's own Node. Which of
/// them a rule reads is the rule's business, and `reserve` is authored here all
/// the same though the rule that spends it is pending, because the parameter is
/// the Form's and only its rule is.
#[derive(Clone, Debug)]
pub struct FormContent {
    pub id: String,
    pub route_reach: Fx,
    pub forecast_depth: u16,
    pub leak_frac: Frac,
    pub upkeep_rate: Fx,
    pub capacity: Fx,
    pub reserve: Fx,
    /// The steering response this Form is authored with, as a scale on the
    /// locked steering reach: fast Forms author more than one, slow Forms less.
    pub steer_scale: Frac,
    /// What a Route this Form forms carries per step. The `connect` payload
    /// names two Nodes and no capacity, so the capacity is the Field's to
    /// supply, and this is the per-Form value the document names as the one
    /// variation that rule admits.
    pub route_capacity: Fx,
    /// The rule-changing abilities this Form authors, in authored order.
    pub abilities: Vec<Ability>,
}

/// The ability kinds version 1 knows, as their machine ids. A file naming any
/// other kind is content this build cannot apply, and is refused rather than
/// ignored — an ability that reached the Field as nothing would be a promise
/// the run does not keep.
pub const ABILITY_KINDS: [&str; 2] = ["linked_forms", "trail"];

/// How many Forms one `linked_forms` ability may stand beside the controlled
/// one: the Forms-per-Run cap less the controlled Form itself.
pub const LINKED_FORMS_CAP: usize = field::FORMS_PER_RUN - 1;

/// One rule-changing ability, authored as data rather than written into a
/// branch that names a Form.
///
/// The reader admits exactly the kinds above and every parameter each kind
/// declares, so what an ability may do is bounded by this type rather than by
/// what a caller remembers to check.
#[derive(Clone, Debug)]
pub enum Ability {
    /// Several Forms stand where a chapter places one: the controlled Form at
    /// the chapter's own placement, and one more at each authored offset from
    /// it, on the same layer and carrying the same parameters.
    ///
    /// `separation` is the distance past which the linked Forms stand apart
    /// rather than together: a Form past it accepts no delivery until it is
    /// back inside. Both numbers are copied onto every linked Form the ability
    /// stands up, because following and separation are rules of a step and a
    /// step reads state rather than content.
    LinkedForms { offsets: Vec<Vec2>, separation: Fx },
    /// The Form leaves a Trail entry every `period` steps where it stands; each
    /// entry falls due `delay` steps later and delivers `magnitude` to the
    /// Nodes inside `radius` of where it was left.
    Trail { period: u16, delay: u16, radius: Fx, magnitude: Fx },
}

/// What an objective's authored condition counts, and how a step that does not
/// meet it is read.
#[derive(Clone, Debug)]
pub enum Condition {
    /// The controlled Form stands in the named current's band. Cumulative.
    InCurrent { current: u16, steps: i64 },
    /// The controlled Form stands within `radius` of a point on a layer.
    HoldPosition { layer: u8, pos: Vec2, radius: Fx, steps: i64 },
    /// A controlled Form emitted a Pulse. Cumulative.
    PulseReleased { count: i64 },
    /// Every named Port stands open.
    PortsOpen { ports: Vec<u32> },
    /// Every named Route carried Charge on the same step.
    RoutesFlowing { routes: Vec<u32> },
    /// Every named Route carried Charge and no named Node stood overloaded.
    /// The one condition whose failure is read as a recoverable setback.
    PatternHeld { routes: Vec<u32>, nodes: Vec<u32>, steps: i64 },
}

impl Condition {
    /// How many qualifying steps, or occurrences, the condition asks for.
    pub fn target(&self) -> i64 {
        match self {
            Condition::InCurrent { steps, .. }
            | Condition::HoldPosition { steps, .. }
            | Condition::PatternHeld { steps, .. } => *steps,
            Condition::PulseReleased { count } => *count,
            Condition::PortsOpen { .. } | Condition::RoutesFlowing { .. } => 1,
        }
    }

    /// True when a step that does not meet the condition leaves what came
    /// before it standing, rather than starting the count again.
    fn cumulative(&self) -> bool {
        matches!(self, Condition::InCurrent { .. } | Condition::PulseReleased { .. })
    }

    /// The share of the whole one qualifying step carries, rounded up so the
    /// count always reaches the whole. The effective span in steps is therefore
    /// `ceil(65536 / share)`, which is the authored target to within one share.
    pub fn share(&self) -> Frac {
        let target = self.target().max(1);
        (FRAC_ONE + target - 1) / target
    }

    /// Reads one completed step against the condition.
    fn read_step(&self, reading: &StepReading<'_>) -> Outcome {
        match self {
            Condition::InCurrent { current, .. } => {
                let held = reading.field.currents.iter().find(|held| held.id == *current);
                let met = match (held, reading.controlled()) {
                    (Some(current), Some(form)) => stands_in(current, form.pos, form.layer),
                    _ => false,
                };
                Outcome { met, setback: false }
            }
            Condition::HoldPosition { layer, pos, radius, .. } => {
                let met = reading.controlled().is_some_and(|form| {
                    form.layer == *layer && within(form.pos, form.layer, *pos, *layer, *radius)
                });
                Outcome { met, setback: false }
            }
            Condition::PulseReleased { .. } => Outcome {
                met: reading.cues.iter().any(|cue| cue.kind == field::CUE_PULSE_EMITTED),
                setback: false,
            },
            Condition::PortsOpen { ports } => Outcome {
                met: ports.iter().all(|node| {
                    reading.field.ports.iter().any(|port| port.node == *node && port.open)
                }),
                setback: false,
            },
            Condition::RoutesFlowing { routes } => {
                Outcome { met: reading.flowing(routes), setback: false }
            }
            Condition::PatternHeld { routes, nodes, .. } => {
                let flowing = reading.flowing(routes);
                let strained = nodes.iter().any(|node| {
                    reading
                        .field
                        .ports
                        .iter()
                        .any(|port| port.node == *node && port.q > port.capacity)
                });
                // The authored break: the pattern is standing, and one of its
                // Nodes is carrying more than it can hold. Nothing about it is
                // terminal — the Charge above the threshold sheds, the state
                // says so, and the run carries on either way.
                Outcome { met: flowing && !strained, setback: strained }
            }
        }
    }
}

/// What one step said about one condition.
struct Outcome {
    met: bool,
    /// True while the authored recoverable setback stands.
    setback: bool,
}

/// True when a placed Node stands in a current's band: the vertex rule, the
/// same one Current delivery reads.
pub fn stands_in(current: &CurrentState, pos: Vec2, layer: u8) -> bool {
    current.layer == layer
        && current.path.iter().any(|point| within(pos, layer, *point, layer, current.width))
}

/// One completed step, as the script reads it.
pub struct StepReading<'a> {
    pub field: &'a FieldState,
    pub cues: &'a [Cue],
}

impl StepReading<'_> {
    fn controlled(&self) -> Option<&FormState> {
        self.field.forms.iter().find(|form| form.controlled)
    }

    fn flowing(&self, routes: &[u32]) -> bool {
        routes.iter().all(|named| {
            self.field.routes.iter().any(|route| route.route == *named && route.flow > 0)
        })
    }
}

/// One authored objective: the copy-catalog key the player reads it by, what
/// completes it, and — for an optional test — the span it stands for.
#[derive(Clone, Debug)]
pub struct Objective {
    pub id: String,
    pub condition: Condition,
    /// The span an optional test stands for, in steps, and none for an
    /// objective the chapter requires.
    ///
    /// A required objective blocks the chapter until it is met. An optional one
    /// is offered in its authored place like any other — one objective is
    /// visible at a time, and an optional test is that objective while it
    /// stands — and is passed over when its span runs out, which is what makes
    /// it optional: a player who takes it keeps what a completion is worth, and
    /// a player who does not loses only the time it stood.
    pub optional: Option<Step>,
}

/// The longest span an optional test may author, in steps: one hour of play at
/// the locked thirty steps a second. A test is a passage of a chapter rather
/// than a chapter of its own, and the cap says so.
pub const OPTIONAL_SPAN_CAP: i64 = 108_000;

/// How many authored events one chapter may hold. A chapter times its own
/// disruptions against its objectives, and the cap is a readable count of them
/// rather than a machine one.
pub const EVENTS_PER_CHAPTER: usize = 32;

/// How many ending marks one chapter may author. An ending is read once, at the
/// close of a campaign, so the cap is small on purpose: a chapter that wanted
/// more of them would be reporting a state rather than closing on one.
pub const ENDING_MARKS_PER_CHAPTER: usize = 8;

/// One ending variant a chapter may close on.
///
/// The mark is the last segment of the reported ending key. `objective` is the
/// completion that names it, and `None` is the catch-all — the mark a run
/// reaches when it completed none of the objectives the marks above it name.
/// The catch-all is authored last and is required of any non-empty list, which
/// is what makes every run reach one.
#[derive(Clone, Debug)]
pub struct EndingMark {
    pub mark: String,
    pub objective: Option<String>,
}

/// The authored-event kinds version 1 knows, as their machine ids.
///
/// The order is the reader's own — a kind is read by its place in this list —
/// so a kind is appended and never inserted.
pub const EVENT_KINDS: [&str; 4] =
    ["set_current_active", "set_layer_drain", "set_port_open", "set_route_cut"];

/// What one authored event does to the Field it lands in.
///
/// Each is a setter on authored Field state and nothing more: no Charge moves,
/// no ledger entry is written, and nothing is drawn from the stream. That is
/// what lets an event ride the boundary-settled carriage exactly as a Fracture
/// break and a Drift move do — the active window ends, the change lands between
/// two steps, and the trajectory restarts on the state it leaves.
#[derive(Clone, Debug)]
pub enum Effect {
    /// A Port opens or closes: the port closure a chapter times.
    PortOpen { node: u32, open: bool },
    /// A layer's per-Node loss is replaced: the drain spike a chapter times.
    LayerDrain { layer: u8, drain: Fx },
    /// A current stands or stops: the current deactivation a chapter times.
    CurrentActive { current: u16, active: bool },
    /// A Route is severed: the scripted route removal a chapter times.
    ///
    /// It takes the Route out of the Route set permanently, exactly as a
    /// committed cut and a Fracture break do, and by the same rule —
    /// identifiers are never reused, so `next_route_id` does not move and a
    /// queued plan naming the Route fails commit validation afterwards. A
    /// second event naming a Route already gone changes nothing and ends no
    /// window, which is the same reading every other kind gets when it finds
    /// the Field already as it asks.
    RouteCut { route: u32 },
}

/// One authored event: what it does, and when.
///
/// The when is an objective and a span, not a step of the run, because a run's
/// step counter is the campaign's and an author cannot know what it stands at
/// when a chapter opens. `at` steps after `objective` was offered, and while
/// that objective still stands, the event falls due — so a disruption lands
/// during the challenge it was written for, and a player who finishes early
/// never meets it. Both halves of the trigger are payload fields, so a restore
/// lands on the same reading and a replay repeats it.
#[derive(Clone, Debug)]
pub struct AuthoredEvent {
    pub objective: String,
    pub at: Step,
    pub effect: Effect,
}

impl AuthoredEvent {
    /// The cue this event raises against the Field as it stood before it, and
    /// none for a change the closed cue set has no reading of.
    ///
    /// A disruption is the Break the closed set already names — a Port closed,
    /// a layer's loss raised, a current stopped — and an opened Port is the cue
    /// a Port's opening already has. What restores something raises nothing: a
    /// current standing again is the absence of the disruption rather than an
    /// event of its own, and the closed cue set stays at twelve either way.
    pub fn cue_kind(&self, before: &FieldState) -> Option<u8> {
        match &self.effect {
            Effect::PortOpen { open: true, .. } => Some(field::CUE_PORT_OPENED),
            Effect::PortOpen { open: false, .. } => Some(field::CUE_BREAK),
            Effect::CurrentActive { active, .. } => (!*active).then_some(field::CUE_BREAK),
            Effect::LayerDrain { layer, drain } => before
                .layers
                .iter()
                .find(|held| held.layer == *layer)
                .filter(|held| *drain > held.drain)
                .map(|_| field::CUE_BREAK),
            // A severed Route is a Break like a closed Port: cue 5 is the
            // committed cut's own, and its locked payload is the Route in `a`
            // and the tail in `b`, which the event carriage's one-Node cue
            // cannot carry. So the closed set answers with the Break it
            // already names, standing at the tail.
            Effect::RouteCut { route } => before
                .routes
                .iter()
                .any(|held| held.route == *route)
                .then_some(field::CUE_BREAK),
        }
    }

    /// The Node this event's cue stands at, read against the Field as it stood
    /// before it: the Port it names, or the tail of the Route it severs, and
    /// none for an event that names a layer or a current, whose cue stands
    /// where the run stands.
    pub fn at_node(&self, before: &FieldState) -> Option<u32> {
        match &self.effect {
            Effect::PortOpen { node, .. } => Some(*node),
            Effect::RouteCut { route } => {
                before.routes.iter().find(|held| held.route == *route).map(|held| held.tail)
            }
            Effect::LayerDrain { .. } | Effect::CurrentActive { .. } => None,
        }
    }
}

/// One authored placement of a Form on the chapter's opening Field.
#[derive(Clone, Debug)]
pub struct FormPlacement {
    pub id: u8,
    pub node: u32,
    pub controlled: bool,
    /// Whether a Handoff may move control onto this Form.
    ///
    /// The authored seam ARCHITECTURE.md's Handoff locks: default true, and a
    /// placement that leaves the key out is controllable. It is validation
    /// data rather than step data — it gates a Handoff request at the moment
    /// it arrives and touches no step — so it is never serialized and adds no
    /// payload key. The authored cost lever is scarcity of controllable Forms
    /// rather than a price.
    pub controllable: bool,
    pub layer: u8,
    pub pos: Vec2,
}

/// One authored Node of the chapter's opening Field. Its stored Charge is its
/// kind's own starting Charge, which the document locks per kind.
#[derive(Clone, Debug)]
pub struct PortPlacement {
    pub node: u32,
    pub layer: u8,
    pub kind: NodeKind,
    pub pos: Vec2,
    pub capacity: Fx,
    pub upkeep_rate: Fx,
}

/// One authored Route of the chapter's opening Field.
#[derive(Clone, Debug)]
pub struct RoutePlacement {
    pub route: u32,
    pub tail: u32,
    pub head: u32,
    pub capacity: Fx,
}

/// One authored layer and its three difficulty parameters.
#[derive(Clone, Debug)]
pub struct LayerParameters {
    pub layer: u8,
    pub drain: Fx,
    pub noise: Frac,
    pub gain: Frac,
}

/// One authored chapter.
#[derive(Clone, Debug)]
pub struct Chapter {
    pub id: String,
    pub title_key: String,
    /// The copy-catalog key the run reports when the campaign ends on this
    /// chapter. Only the last chapter of the manifest ever reaches it, and
    /// every chapter declares one so that a truncated campaign still ends.
    pub ending_key: String,
    /// The ending variants this chapter may close on, in authored order, and
    /// empty for a chapter that closes on `ending_key` alone.
    pub ending_marks: Vec<EndingMark>,
    /// The disruptions this chapter times against its own objectives, in
    /// authored order.
    pub events: Vec<AuthoredEvent>,
    pub layers: Vec<LayerParameters>,
    pub forms: Vec<FormPlacement>,
    pub ports: Vec<PortPlacement>,
    pub routes: Vec<RoutePlacement>,
    pub currents: Vec<CurrentState>,
    pub authored_boundaries: Vec<Vec<u32>>,
    pub objectives: Vec<Objective>,
    pub anchor_moments: Vec<String>,
    pub opening_view: ViewDeclaration,
    /// What the chapter requests of the pressure system, in authored order.
    /// The core admits per the locked limit; an entry the limit refuses stands
    /// queued until a seat opens.
    pub pressure_schedule: Vec<crate::pressure::ScheduleEntry>,
}

impl Chapter {
    /// The objective one catalog key names.
    pub fn objective(&self, id: &str) -> Option<&Objective> {
        self.objectives.iter().find(|held| held.id == id)
    }

    /// The copy-catalog key a completed campaign reports for this chapter,
    /// under the Form the run opened on and the objectives it completed.
    ///
    /// A chapter that authors no marks answers `ending_key`, which is every
    /// chapter authored before this reading and every chapter a truncated
    /// campaign ends on. A chapter that authors marks answers
    /// `<ending_key>.<form>.<mark>`, where the mark is the **first** authored
    /// one whose named objective stands in the completed list — authored order
    /// is therefore most specific first — and the catch-all closes the list, so
    /// the search always finds one.
    ///
    /// Both halves of the reading are payload fields: the Form is the run's
    /// own, and the completed list is the campaign's. Nothing here branches on
    /// which Form was selected — the identifier is a key segment the authored
    /// catalog answers for, exactly as the chapter's own id is.
    pub fn ending_id(&self, form: &str, complete: &[String]) -> String {
        let Some(found) = self.ending_marks.iter().find(|held| match &held.objective {
            Some(id) => complete.iter().any(|done| done == id),
            None => true,
        }) else {
            return self.ending_key.clone();
        };
        format!("{}.{}.{}", self.ending_key, form, found.mark)
    }

    /// Whether a Handoff may move control onto one Form of this chapter's
    /// Field, by the Form's identifier — and `None` for an identifier this
    /// chapter's Field does not account for.
    ///
    /// A Form the chapter placed answers with its own authored flag. A Form the
    /// selection stood up — the linked members of a `linked_forms` group, which
    /// take the identifiers above the highest the chapter authored — answers
    /// with the flag of the placement it stands from, which is the chapter's
    /// first: the group is one selection, and control moving inside it
    /// re-anchors the formation rather than reaching a Form the chapter never
    /// authored.
    ///
    /// An identifier neither of those accounts for is answered `None` rather
    /// than with the first placement's flag, which has nothing to do with it. A
    /// Field with no placements answers `None` to everything, as does one asked
    /// about an identifier below the placements it holds.
    pub fn controllable(&self, id: u8) -> Option<bool> {
        if let Some(placement) = self.forms.iter().find(|held| held.id == id) {
            return Some(placement.controllable);
        }
        let highest = self.forms.iter().map(|held| held.id).max()?;
        let first = self.forms.first()?;
        (id > highest).then_some(first.controllable)
    }

    /// Where one objective stands in the authored order.
    fn place_of(&self, id: &str) -> Option<usize> {
        self.objectives.iter().position(|held| held.id == id)
    }

    /// The objective the sequence offers under one reading of progress, and
    /// none when the chapter is complete.
    ///
    /// Two facts decide it, and both ride in the payload. The first is
    /// `progress.complete`: an objective already completed is behind the
    /// sequence. The second is the standing objective's own place: an
    /// **optional** objective standing earlier in the authored order than the
    /// one the run holds is one the sequence has already passed over, and is
    /// not offered again. A required objective is never passed over, so it
    /// stays offered until it is met however far the run has moved — which is
    /// what makes it required.
    pub fn offered(&self, progress: &Progress) -> Option<&Objective> {
        let standing = self.place_of(&progress.objective.id);
        self.objectives.iter().enumerate().find_map(|(place, held)| {
            if progress.complete.iter().any(|done| done == &held.id) {
                return None;
            }
            if held.optional.is_some() && standing.is_some_and(|at| place < at) {
                return None;
            }
            Some(held)
        })
    }

    /// The same reading, with one objective treated as already passed over.
    ///
    /// It answers the one question the standing reading cannot: what the
    /// sequence would offer if the optional test standing right now were let
    /// go of. The answer is what the pass-over installs, and a `None` is the
    /// pass-over that completes the chapter.
    fn offered_past(&self, progress: &Progress, passed: &str) -> Option<&Objective> {
        let standing = self.place_of(passed);
        self.objectives.iter().enumerate().find_map(|(place, held)| {
            if held.id == passed || progress.complete.iter().any(|done| done == &held.id) {
                return None;
            }
            if held.optional.is_some() && standing.is_some_and(|at| place < at) {
                return None;
            }
            Some(held)
        })
    }
}

/// The whole authored bundle a session holds.
#[derive(Clone, Debug)]
pub struct Content {
    pub hash: String,
    pub version: i64,
    pub chapters: Vec<Chapter>,
    pub forms: Vec<FormContent>,
    /// The six authored pressure tables the stage machine reads.
    pub pressures: crate::pressure::Schedule,
}

impl Content {
    pub fn chapter(&self, index: u8) -> Option<&Chapter> {
        self.chapters.get(usize::from(index))
    }

    pub fn form(&self, id: &str) -> Option<&FormContent> {
        self.forms.iter().find(|held| held.id == id)
    }
}

/// Reads the bundle the worker hands over at construction.
///
/// The shape is `{ "hash": hex64, "manifest": text, "files": [text] }`, with
/// the files in manifest order. Every failure here is the `content_invalid`
/// envelope, which is the one the document names for content that does not
/// validate at load.
pub fn read_bundle(value: &Json) -> Result<Content, Fault> {
    read::exact_keys(value, "content", &["files", "hash", "manifest"])
        .map_err(invalid)?;
    let hash = read::hex(value, "hash", 64).map_err(invalid)?.to_string();
    let manifest_text = read::text(value, "manifest").map_err(invalid)?;
    let files = read::list(value, "files", MANIFEST_FILES_CAP).map_err(invalid)?;

    // The locked rule, recomputed over exactly the bytes that arrived: the
    // manifest, then every listed file, in manifest order.
    let mut concatenated: Vec<u8> = manifest_text.as_bytes().to_vec();
    let mut texts = Vec::with_capacity(files.len());
    for entry in files {
        let text = entry.as_text().ok_or_else(|| Fault::because(Code::ContentInvalid, "files"))?;
        concatenated.extend_from_slice(text.as_bytes());
        texts.push(text);
    }
    if crate::json::hex_bytes(&sha256::digest(&concatenated)) != hash {
        return Err(Fault::because(Code::ContentInvalid, "content_hash"));
    }

    let manifest =
        parse(manifest_text).map_err(|reason| Fault::because(Code::ContentInvalid, reason))?;
    read::exact_keys(&manifest, "manifest", &["chapters", "content_version", "forms", "pressures"])
        .map_err(invalid)?;
    let version = read::int(&manifest, "content_version", CONTENT_VERSION, CONTENT_VERSION)
        .map_err(invalid)?;

    let mut listed: Vec<(usize, String)> = Vec::new();
    for (place, key) in MANIFEST_LISTS.iter().enumerate() {
        for entry in read::list(&manifest, key, MANIFEST_FILES_CAP).map_err(invalid)? {
            let id = entry.as_text().ok_or_else(|| Fault::because(Code::ContentInvalid, key))?;
            listed.push((place, id.to_string()));
        }
    }
    if listed.len() != texts.len() {
        return Err(Fault::because(Code::ContentInvalid, "files"));
    }

    let mut chapters = Vec::new();
    let mut forms = Vec::new();
    let mut tables = Vec::new();
    for ((list, id), text) in listed.iter().zip(texts.iter()) {
        let file = parse(text).map_err(|reason| Fault::because(Code::ContentInvalid, reason))?;
        match list {
            0 => {
                let chapter = read_chapter(&file)?;
                if &chapter.id != id {
                    return Err(Fault::because(Code::ContentInvalid, "chapters"));
                }
                chapters.push(chapter);
            }
            1 => {
                let form = read_form(&file)?;
                if &form.id != id {
                    return Err(Fault::because(Code::ContentInvalid, "forms"));
                }
                forms.push(form);
            }
            _ => {
                let table = crate::pressure::PressureContent::read(&file).map_err(invalid)?;
                if table.pressure.name() != id {
                    return Err(Fault::because(Code::ContentInvalid, "pressures"));
                }
                tables.push(table);
            }
        }
    }
    if chapters.is_empty() {
        return Err(Fault::because(Code::ContentInvalid, "chapters"));
    }
    let pressures = crate::pressure::Schedule::of(tables)?;
    // A chapter may request only a pressure the manifest authored a table for,
    // and, when it names a target at all, only at the target kind that table
    // declares: a schedule the tables cannot answer is content this build
    // cannot run.
    //
    // A target of kind `none` names nothing by declaration, so there is nothing
    // for the kind to agree with: it defers to the pressure's own locked
    // default — Flood's heaviest-throughput hold, Fracture's largest-trailing-
    // flow break — which is the reading ARCHITECTURE.md's pressure effects
    // authorise for every pressure that has one. Refusing it here would make
    // that reading unauthorable by any chapter.
    for chapter in &chapters {
        for entry in &chapter.pressure_schedule {
            let Some(table) = pressures.table(entry.pressure) else {
                return Err(Fault::because(Code::ContentInvalid, "pressure_schedule"));
            };
            if entry.target.kind != crate::pressure::TargetKind::None
                && table.target != entry.target.kind
            {
                return Err(Fault::because(Code::ContentInvalid, "pressure_schedule"));
            }
        }
    }
    let content = Content { hash, version, chapters, forms, pressures };
    campaign_valid(&content)?;
    Ok(content)
}

/// What a campaign is held to, beyond what each of its chapters is.
///
/// A chapter reads on its own; a campaign is the sequence of them, and three
/// things can only be checked across that sequence. Each is refused with the
/// field named, exactly as a chapter's own refusals are, because an author
/// reading `objectives` learns where to look and an author reading a sentence
/// does not.
///
/// **What this can honestly claim, and what it cannot.** It claims that every
/// chapter loads, that the sequence is the closed set's own order, that no two
/// chapters name one objective, that every chapter's Field establishes under
/// every authored Form, and that each chapter's completion condition names
/// entities that chapter placed. It does not claim that a chapter is winnable:
/// whether a player can reach a point, open a Port, or hold a pattern under a
/// pressure is a question about strategies rather than about content, and the
/// goal that runs strategies against every chapter is the one that can answer
/// it. What is refused here is content no strategy could complete; what is not
/// refused is not thereby promised.
fn campaign_valid(content: &Content) -> Result<(), Fault> {
    // The sequence is the closed set's own order, with no chapter twice. A
    // campaign may be shorter than eight — a build under test authors what it
    // needs — but it may not be reordered, because the chapter a run carries
    // into is the next of this list and the list is what the manifest says.
    let mut expected = CHAPTER_IDS.iter();
    for chapter in &content.chapters {
        if !expected.any(|held| *held == chapter.id) {
            return Err(Fault::because(Code::ContentInvalid, "chapters"));
        }
    }
    // The run's completed list is the campaign's rather than one chapter's, and
    // the sequence reads it to decide what to offer. Two chapters naming one
    // objective would therefore open the second with that objective already
    // behind it — an objective a player never sees, and, when it is the
    // chapter's only required one, a chapter that completes itself the instant
    // it opens.
    let mut named: Vec<&str> = content
        .chapters
        .iter()
        .flat_map(|chapter| chapter.objectives.iter().map(|held| held.id.as_str()))
        .collect();
    named.sort_unstable();
    let held = named.len();
    named.dedup();
    if named.len() != held {
        return Err(Fault::because(Code::ContentInvalid, "objectives"));
    }

    for chapter in &content.chapters {
        // Deeper layers carry the greater loss and the greater reward. It is
        // the one balance rule the specification states as a direction rather
        // than a figure, so it is checked as a direction: a chapter that pays
        // better nearer the surface is a chapter whose depth choice is not a
        // choice, and a chapter that costs less deeper down is the same defect
        // read the other way.
        for pair in chapter.layers.windows(2) {
            let (above, below) = (&pair[0], &pair[1]);
            if below.gain < above.gain || below.drain < above.drain || below.noise < above.noise {
                return Err(Fault::because(Code::ContentInvalid, "layers"));
            }
        }
        // Every chapter establishes, under every Form the campaign authors.
        // This is the whole of the malformed-definition check that a load can
        // make: the Field the chapter opens, the View it opens under, and the
        // Forms an ability stands beside the controlled one, all held to the
        // same locked caps, ranges, and ordering rules a run is.
        for form in &content.forms {
            let (field, view) = establish(chapter, form)?;
            field::validate(&field).map_err(invalid)?;
            field::establishable(&field).map_err(invalid)?;
            field::establishable_view(&view, &field).map_err(invalid)?;
        }
    }
    Ok(())
}

fn invalid(fault: Fault) -> Fault {
    read::recode(fault, Code::ContentInvalid)
}

fn read_form(value: &Json) -> Result<FormContent, Fault> {
    read::exact_keys(
        value,
        "form",
        &[
            "abilities",
            "capacity",
            "forecast_depth",
            "id",
            "leak_frac",
            "reserve",
            "route_capacity",
            "route_reach",
            "steer_scale",
            "upkeep_rate",
        ],
    )
    .map_err(invalid)?;
    let id = read::text(value, "id").map_err(invalid)?.to_string();
    if !crate::run::FORMS.contains(&id.as_str()) {
        return Err(Fault::because(Code::ContentInvalid, "id"));
    }
    Ok(FormContent {
        abilities: read_abilities(value)?,
        id,
        route_reach: read::int(value, "route_reach", 0, STORED_BOUND - 1).map_err(invalid)?,
        forecast_depth: read::int(value, "forecast_depth", 0, i64::from(field::FORECAST_DEPTH_CAP))
            .map_err(invalid)? as u16,
        leak_frac: read::int(value, "leak_frac", 0, field::LEAK_FRAC_CAP).map_err(invalid)?,
        upkeep_rate: read::int(value, "upkeep_rate", 0, STORED_BOUND - 1).map_err(invalid)?,
        capacity: read::int(value, "capacity", 0, field::NODE_CHARGE_CAP).map_err(invalid)?,
        reserve: read::int(value, "reserve", 0, field::NODE_CHARGE_CAP).map_err(invalid)?,
        steer_scale: read::int(
            value,
            "steer_scale",
            field::STEER_SCALE_LOW,
            field::STEER_SCALE_HIGH,
        )
        .map_err(invalid)?,
        route_capacity: read::int(value, "route_capacity", 1, field::ROUTE_CAPACITY_CAP)
            .map_err(invalid)?,
    })
}

/// Reads one Form's authored abilities.
///
/// The kind is read first and decides the shape, so every parameter an ability
/// declares is read under the kind that declares it and nothing is read that
/// the kind does not name. A file may author at most one entry of each kind,
/// because two entries of one kind would be two answers to one question.
fn read_abilities(value: &Json) -> Result<Vec<Ability>, Fault> {
    let entries = read::list(value, "abilities", ABILITY_KINDS.len()).map_err(invalid)?;
    let mut abilities = Vec::with_capacity(entries.len());
    let mut authored = Vec::with_capacity(entries.len());
    for entry in entries {
        let kind = read::one_of(entry, "kind", &ABILITY_KINDS).map_err(invalid)?;
        if authored.contains(&kind) {
            return Err(Fault::because(Code::ContentInvalid, "abilities"));
        }
        authored.push(kind);
        abilities.push(match kind {
            0 => {
                read::exact_keys(entry, "abilities", &["kind", "offsets", "separation"])
                    .map_err(invalid)?;
                let held = read::list(entry, "offsets", LINKED_FORMS_CAP).map_err(invalid)?;
                if held.is_empty() {
                    return Err(Fault::because(Code::ContentInvalid, "offsets"));
                }
                let mut offsets = Vec::with_capacity(held.len());
                for offset in held {
                    let wrapped = Json::Map(vec![("offsets".to_string(), offset.clone())]);
                    let point = Vec2::read(&wrapped, "offsets").map_err(invalid)?;
                    // An offset is a displacement on the plane, so the plane's
                    // own width bounds it in either direction; where it lands
                    // is checked when the Field is established, against the
                    // placement it is measured from.
                    for axis in [point.x, point.y] {
                        if !(-(field::PLANE_SPAN - 1)..field::PLANE_SPAN).contains(&axis) {
                            return Err(Fault::because(Code::ContentInvalid, "offsets"));
                        }
                    }
                    offsets.push(point);
                }
                Ability::LinkedForms {
                    offsets,
                    separation: read::int(entry, "separation", 0, STORED_BOUND - 1)
                        .map_err(invalid)?,
                }
            }
            1 => {
                read::exact_keys(
                    entry,
                    "abilities",
                    &["delay", "kind", "magnitude", "period", "radius"],
                )
                .map_err(invalid)?;
                let steps = |key: &str, bounds: std::ops::RangeInclusive<u16>| -> Result<u16, Fault> {
                    Ok(read::int(entry, key, i64::from(*bounds.start()), i64::from(*bounds.end()))
                        .map_err(invalid)? as u16)
                };
                Ability::Trail {
                    period: steps("period", field::TRAIL_PERIOD)?,
                    delay: steps("delay", field::TRAIL_DELAY)?,
                    radius: read::int(entry, "radius", 0, field::TRAIL_RADIUS_CAP)
                        .map_err(invalid)?,
                    magnitude: read::int(entry, "magnitude", 0, field::TRAIL_MAGNITUDE_CAP)
                        .map_err(invalid)?,
                }
            }
            _ => return Err(Fault::because(Code::ContentInvalid, "kind")),
        });
    }
    Ok(abilities)
}

fn read_chapter(value: &Json) -> Result<Chapter, Fault> {
    // `ending_marks` is the one defaulted key of the chapter shape, read the way
    // a Form placement's `controllable` is: a chapter that leaves it out closes
    // on `ending_key` alone, which is how every chapter authored before the
    // reading behaves. Everything else is held to the shape's own keys, so a key
    // the shape never declared is still a load refusal naming the field.
    const DECLARED: [&str; 14] = [
        "anchor_moments",
        "authored_boundaries",
        "currents",
        "ending_key",
        "events",
        "forms",
        "id",
        "layers",
        "objectives",
        "opening_view",
        "ports",
        "pressure_schedule",
        "routes",
        "title_key",
    ];
    let mut declared: Vec<&str> = DECLARED.to_vec();
    if value.get("ending_marks").is_some() {
        declared.push("ending_marks");
    }
    read::exact_keys(value, "chapter", &declared).map_err(invalid)?;

    let id = read::text(value, "id").map_err(invalid)?.to_string();
    if !CHAPTER_IDS.contains(&id.as_str()) {
        return Err(Fault::because(Code::ContentInvalid, "id"));
    }
    let title_key = read::text(value, "title_key").map_err(invalid)?.to_string();
    if title_key != format!("chapter.{id}") {
        return Err(Fault::because(Code::ContentInvalid, "title_key"));
    }
    // The ending this chapter's completion reaches. It is a copy-catalog key of
    // the `ending` kind, so what a player reads at the close of a campaign is
    // authored copy like every other string, and the event carries the key
    // rather than the wording.
    let ending_key = read::text(value, "ending_key").map_err(invalid)?.to_string();
    if !is_key_of("ending.", &ending_key) {
        return Err(Fault::because(Code::ContentInvalid, "ending_key"));
    }
    // What the chapter requests of the pressure system, in authored order. One
    // entry per pressure at most: the list the run carries is ascending in the
    // closed set's own order and holds each pressure once.
    let mut pressure_schedule = Vec::new();
    for entry in
        read::list(value, "pressure_schedule", crate::pressure::SCHEDULE_CAP).map_err(invalid)?
    {
        pressure_schedule.push(crate::pressure::ScheduleEntry::read(entry).map_err(invalid)?);
    }
    {
        let mut named: Vec<u8> =
            pressure_schedule.iter().map(|entry| entry.pressure.ordinal()).collect();
        named.sort_unstable();
        let held = named.len();
        named.dedup();
        if named.len() != held {
            return Err(Fault::because(Code::ContentInvalid, "pressure_schedule"));
        }
        // At most one primary, over the whole schedule: the run list's locked
        // invariant counts queued and active together, so a schedule that
        // requests two primaries would seat a list no payload could carry.
        if pressure_schedule.iter().filter(|entry| entry.primary).count()
            > crate::pressure::PRIMARY_CAP
        {
            return Err(Fault::because(Code::ContentInvalid, "pressure_schedule"));
        }
    }

    let mut layers = Vec::new();
    for entry in read::list(value, "layers", LAYERS_PER_CHAPTER).map_err(invalid)? {
        read::exact_keys(entry, "layers", &["drain", "gain", "layer", "noise"]).map_err(invalid)?;
        layers.push(LayerParameters {
            layer: read::int(entry, "layer", 0, i64::from(field::MAX_LAYER)).map_err(invalid)? as u8,
            drain: read::int(entry, "drain", 0, STORED_BOUND - 1).map_err(invalid)?,
            noise: read::int(entry, "noise", 0, FRAC_ONE).map_err(invalid)?,
            gain: read::int(entry, "gain", 0, FRAC_ONE).map_err(invalid)?,
        });
    }

    let mut forms = Vec::new();
    for entry in read::list(value, "forms", field::FORMS_PER_RUN).map_err(invalid)? {
        // `controllable` is the one defaulted key of the authored shape: a
        // placement that leaves it out is controllable, which is what keeps
        // every chapter authored before the Handoff reading exactly as it did.
        // Everything else is held to the shape's own keys, so a key the shape
        // never declared is still a load refusal naming the field.
        let declared: &[&str] = match entry.get("controllable") {
            Some(_) => &["controllable", "controlled", "id", "layer", "node", "pos"],
            None => &["controlled", "id", "layer", "node", "pos"],
        };
        read::exact_keys(entry, "forms", declared).map_err(invalid)?;
        forms.push(FormPlacement {
            id: read::int(entry, "id", 0, i64::from(u8::MAX)).map_err(invalid)? as u8,
            node: read::int(entry, "node", 1, i64::from(u32::MAX)).map_err(invalid)? as u32,
            controlled: read::flag(entry, "controlled").map_err(invalid)?,
            controllable: match entry.get("controllable") {
                Some(_) => read::flag(entry, "controllable").map_err(invalid)?,
                None => true,
            },
            layer: read::int(entry, "layer", 0, i64::from(field::MAX_LAYER)).map_err(invalid)? as u8,
            pos: Vec2::read(entry, "pos").map_err(invalid)?,
        });
    }
    // Exactly one placement carries control, at every step, in version 1: the
    // frozen `InputFrame` carries one steer vector, one Pulse and one depth
    // intent, so simultaneous control has no input to consume. Several
    // *controllable* Forms with control moving among them is what several
    // controlled Forms playably means, and the Handoff is what moves it.
    // A chapter that places none is refused here rather than by the count
    // below, so the diagnostic names the empty list rather than the control
    // nothing could carry.
    if forms.is_empty() {
        return Err(Fault::because(Code::ContentInvalid, "forms"));
    }
    if forms.iter().filter(|placement| placement.controlled).count() != 1 {
        return Err(Fault::because(Code::ContentInvalid, "controlled"));
    }
    // At least one controllable Form per chapter, or the run stands with a
    // control no Handoff could ever move and the seam is authored shut.
    if !forms.iter().any(|placement| placement.controllable) {
        return Err(Fault::because(Code::ContentInvalid, "controllable"));
    }
    // Ascending, distinct identifiers, and distinct Nodes. The Field's own
    // validation asks for ascending Form ids, and the linked Forms a
    // `linked_forms` ability stands up take the identifiers above the highest
    // authored one — so the authored order is the Field's order and a
    // collision is content this build refuses rather than a Field it
    // establishes twice onto one identifier.
    if !read::ascending(&forms.iter().map(|held| held.id).collect::<Vec<u8>>()) {
        return Err(Fault::because(Code::ContentInvalid, "id"));
    }
    for (place, placement) in forms.iter().enumerate() {
        if forms.iter().skip(place + 1).any(|other| other.node == placement.node) {
            return Err(Fault::because(Code::ContentInvalid, "node"));
        }
    }

    let mut ports = Vec::new();
    for entry in read::list(value, "ports", NODES_PER_RUN).map_err(invalid)? {
        read::exact_keys(entry, "ports", &["capacity", "kind", "layer", "node", "pos", "upkeep_rate"])
            .map_err(invalid)?;
        let kind = NodeKind::read(read::text(entry, "kind").map_err(invalid)?)
            .ok_or_else(|| Fault::because(Code::ContentInvalid, "kind"))?;
        // A Form's Node is placed by the Form placement above, never twice.
        if kind == NodeKind::Form {
            return Err(Fault::because(Code::ContentInvalid, "kind"));
        }
        ports.push(PortPlacement {
            node: read::int(entry, "node", 1, i64::from(u32::MAX)).map_err(invalid)? as u32,
            layer: read::int(entry, "layer", 0, i64::from(field::MAX_LAYER)).map_err(invalid)? as u8,
            kind,
            pos: Vec2::read(entry, "pos").map_err(invalid)?,
            capacity: read::int(entry, "capacity", 0, field::NODE_CHARGE_CAP).map_err(invalid)?,
            upkeep_rate: read::int(entry, "upkeep_rate", 0, STORED_BOUND - 1)
                .map_err(invalid)?,
        });
    }

    let mut routes = Vec::new();
    for entry in read::list(value, "routes", ROUTES_PER_RUN).map_err(invalid)? {
        read::exact_keys(entry, "routes", &["capacity", "head", "route", "tail"])
            .map_err(invalid)?;
        routes.push(RoutePlacement {
            route: read::int(entry, "route", 1, i64::from(u32::MAX)).map_err(invalid)? as u32,
            tail: read::int(entry, "tail", 1, i64::from(u32::MAX)).map_err(invalid)? as u32,
            head: read::int(entry, "head", 1, i64::from(u32::MAX)).map_err(invalid)? as u32,
            capacity: read::int(entry, "capacity", 0, field::ROUTE_CAPACITY_CAP)
                .map_err(invalid)?,
        });
    }

    let mut currents = Vec::new();
    for entry in read::list(value, "currents", CURRENTS_PER_CHAPTER).map_err(invalid)? {
        read::exact_keys(
            entry,
            "currents",
            &["active", "bright", "id", "layer", "path", "period", "strength", "width"],
        )
        .map_err(invalid)?;
        let points = read::list(entry, "path", *field::CURRENT_PATH_POINTS.end())
            .map_err(invalid)?;
        let mut path = Vec::with_capacity(points.len());
        for point in points {
            let held = Json::Map(vec![("path".to_string(), point.clone())]);
            path.push(Vec2::read(&held, "path").map_err(invalid)?);
        }
        currents.push(CurrentState {
            id: read::int(entry, "id", 1, i64::from(u16::MAX)).map_err(invalid)? as u16,
            layer: read::int(entry, "layer", 0, i64::from(field::MAX_LAYER)).map_err(invalid)? as u8,
            path,
            // The drawn band promises what the catchment delivers, so a width
            // below the smallest one the surface can honestly draw is refused.
            width: read::int(entry, "width", MIN_CURRENT_WIDTH, STORED_BOUND - 1)
                .map_err(invalid)?,
            strength: read::int(entry, "strength", 0, field::CURRENT_STRENGTH_CAP)
                .map_err(invalid)?,
            period: read::int(entry, "period", 1, i64::from(u16::MAX)).map_err(invalid)? as u16,
            phase: 0,
            bright: read::flag(entry, "bright").map_err(invalid)?,
            active: read::flag(entry, "active").map_err(invalid)?,
        });
    }

    let mut authored_boundaries = Vec::new();
    for entry in read::list(value, "authored_boundaries", NODES_PER_RUN).map_err(invalid)? {
        read::exact_keys(entry, "authored_boundaries", &["members"]).map_err(invalid)?;
        authored_boundaries.push(
            read::ids(entry, "members", NODES_PER_RUN, i64::from(u32::MAX)).map_err(invalid)?,
        );
    }

    let placed = Placements {
        layers: &layers,
        ports: &ports,
        routes: &routes,
        currents: &currents,
        forms: &forms,
    };

    // Every schedule entry aims at something the chapter placed. The gap this
    // closes is a pressure aimed at a Node, Route, or layer that never stands:
    // a target that names nothing is not refused at the boundary, it simply
    // finds nothing there, so a chapter would run its primary pressure through
    // every stage and change nothing at all.
    for entry in &pressure_schedule {
        if !placed.target(entry.target) {
            return Err(unplaced("pressure_schedule"));
        }
    }

    let mut objectives = Vec::new();
    for entry in read::list(value, "objectives", OBJECTIVES_PER_CHAPTER).map_err(invalid)? {
        read::exact_keys(entry, "objectives", &["condition", "id", "optional"]).map_err(invalid)?;
        let id = read::text(entry, "id").map_err(invalid)?.to_string();
        if !is_key_of("objective.", &id) {
            return Err(Fault::because(Code::ContentInvalid, "id"));
        }
        objectives.push(Objective {
            id,
            condition: read_condition(read::at(entry, "condition").map_err(invalid)?, &placed)?,
            optional: read::int_or_null(entry, "optional", 1, OPTIONAL_SPAN_CAP)
                .map_err(invalid)?
                .map(|span| span as Step),
        });
    }
    if objectives.is_empty() {
        return Err(Fault::because(Code::ContentInvalid, "objectives"));
    }
    // A chapter is completed by meeting what it requires, so it has to require
    // something: a chapter of optional tests alone would complete itself by
    // standing still for the spans it authored.
    if objectives.iter().all(|held| held.optional.is_some()) {
        return Err(Fault::because(Code::ContentInvalid, "objectives"));
    }
    {
        let mut named: Vec<&str> = objectives.iter().map(|held| held.id.as_str()).collect();
        named.sort_unstable();
        let held = named.len();
        named.dedup();
        if named.len() != held {
            return Err(Fault::because(Code::ContentInvalid, "objectives"));
        }
    }

    let ending_marks = read_ending_marks(value, &objectives)?;

    let events = read_events(value, &objectives, &placed)?;

    let anchor_moments = read_keys(value, "anchor_moments", &objectives)?;

    let opening_view = read_view(value)?;

    Ok(Chapter {
        id,
        title_key,
        ending_key,
        ending_marks,
        events,
        layers,
        forms,
        ports,
        routes,
        currents,
        authored_boundaries,
        objectives,
        anchor_moments,
        opening_view,
        pressure_schedule,
    })
}

/// Reads the ending variants a chapter may close on.
///
/// The list is optional — a chapter that leaves the key out closes on
/// `ending_key` alone, which is every chapter authored before this reading —
/// and a chapter that authors it is held to four rules, each of which exists to
/// stop an ending that no run reaches or that two runs share:
///
/// 1. Every mark is one lowercase key segment, because it is appended to a
///    catalog key and a key of the wrong shape names no entry.
/// 2. No two marks are the same, or two readings of a run would report one
///    ending.
/// 3. Every named objective is one of this chapter's own, exactly as an event's
///    is: a mark named by an objective the chapter never authors is a variant
///    no run could reach.
/// 4. Exactly one entry is the catch-all, and it stands last. Without it a run
///    that completed nothing the marks name reaches no mark at all; anywhere
///    but last, every entry below it is unreachable.
fn read_ending_marks(value: &Json, objectives: &[Objective]) -> Result<Vec<EndingMark>, Fault> {
    let Some(entries) = value.get("ending_marks") else {
        return Ok(Vec::new());
    };
    let entries = match entries {
        Json::List(items) if items.len() <= ENDING_MARKS_PER_CHAPTER => items,
        _ => return Err(Fault::because(Code::ContentInvalid, "ending_marks")),
    };
    let mut marks = Vec::new();
    for entry in entries {
        let declared: &[&str] = match entry.get("objective") {
            Some(_) => &["mark", "objective"],
            None => &["mark"],
        };
        read::exact_keys(entry, "ending_marks", declared).map_err(invalid)?;
        let mark = read::text(entry, "mark").map_err(invalid)?.to_string();
        if !is_key_of("ending.", &format!("ending.{mark}")) {
            return Err(Fault::because(Code::ContentInvalid, "mark"));
        }
        let objective = match entry.get("objective") {
            Some(_) => {
                let id = read::text(entry, "objective").map_err(invalid)?.to_string();
                if !objectives.iter().any(|held| held.id == id) {
                    return Err(unplaced("objective"));
                }
                Some(id)
            }
            None => None,
        };
        marks.push(EndingMark { mark, objective });
    }
    if marks.is_empty() {
        return Err(Fault::because(Code::ContentInvalid, "ending_marks"));
    }
    {
        let mut named: Vec<&str> = marks.iter().map(|held| held.mark.as_str()).collect();
        named.sort_unstable();
        let held = named.len();
        named.dedup();
        if named.len() != held {
            return Err(Fault::because(Code::ContentInvalid, "mark"));
        }
    }
    if marks.iter().filter(|held| held.objective.is_none()).count() != 1
        || marks.last().is_some_and(|held| held.objective.is_some())
    {
        return Err(Fault::because(Code::ContentInvalid, "ending_marks"));
    }
    Ok(marks)
}

/// Reads the disruptions a chapter times against its own objectives.
///
/// The kind is read first and decides the shape, exactly as an ability's is, so
/// every parameter is read under the kind that declares it. What each names is
/// checked against what the chapter placed, because an event aimed at a Port,
/// layer, or current that never stands is a disruption that never lands — and a
/// chapter whose crisis quietly does nothing is the defect this reader exists
/// to refuse.
fn read_events(
    value: &Json,
    objectives: &[Objective],
    placed: &Placements<'_>,
) -> Result<Vec<AuthoredEvent>, Fault> {
    let mut events = Vec::new();
    for entry in read::list(value, "events", EVENTS_PER_CHAPTER).map_err(invalid)? {
        let kind = read::one_of(entry, "kind", &EVENT_KINDS).map_err(invalid)?;
        let objective = read::text(entry, "objective").map_err(invalid)?.to_string();
        if !objectives.iter().any(|held| held.id == objective) {
            return Err(unplaced("objective"));
        }
        // The span is at least one step, so an event never falls due on the
        // same boundary that offered the objective it is timed against.
        let at = read::int(entry, "at", 1, OPTIONAL_SPAN_CAP).map_err(invalid)? as Step;
        let effect = match kind {
            0 => {
                read::exact_keys(entry, "events", &["active", "at", "current", "kind", "objective"])
                    .map_err(invalid)?;
                let current =
                    read::int(entry, "current", 1, i64::from(u16::MAX)).map_err(invalid)? as u16;
                if !placed.current(current) {
                    return Err(unplaced("current"));
                }
                Effect::CurrentActive { current, active: read::flag(entry, "active").map_err(invalid)? }
            }
            1 => {
                read::exact_keys(entry, "events", &["at", "drain", "kind", "layer", "objective"])
                    .map_err(invalid)?;
                let layer =
                    read::int(entry, "layer", 0, i64::from(field::MAX_LAYER)).map_err(invalid)? as u8;
                if !placed.layer(layer) {
                    return Err(unplaced("layer"));
                }
                Effect::LayerDrain {
                    layer,
                    drain: read::int(entry, "drain", 0, STORED_BOUND - 1).map_err(invalid)?,
                }
            }
            2 => {
                read::exact_keys(entry, "events", &["at", "kind", "node", "objective", "open"])
                    .map_err(invalid)?;
                let node = read::int(entry, "node", 1, i64::from(u32::MAX)).map_err(invalid)? as u32;
                // Only a Port waits on anything: every other kind is
                // established open and the open gate is what a Port is for, so
                // an event that closed one of the others would close something
                // the rules never let a player open again.
                if !placed.closable_port(node) {
                    return Err(unplaced("node"));
                }
                Effect::PortOpen { node, open: read::flag(entry, "open").map_err(invalid)? }
            }
            _ => {
                read::exact_keys(entry, "events", &["at", "kind", "objective", "route"])
                    .map_err(invalid)?;
                let route =
                    read::int(entry, "route", 1, i64::from(u32::MAX)).map_err(invalid)? as u32;
                // A Route the chapter never placed is a cut that severs
                // nothing: the same gap every other kind's target check closes.
                // A Route a player draws cannot be named — an author cannot
                // know its identifier — so what a chapter may sever is what it
                // placed.
                if !placed.route(route) {
                    return Err(unplaced("route"));
                }
                Effect::RouteCut { route }
            }
        };
        events.push(AuthoredEvent { objective, at, effect });
    }
    Ok(events)
}

/// The narrowest current a chapter may author, raw: eight units. The band the
/// renderer strokes is the width, and a narrower band would draw a promise the
/// catchment does not keep.
pub const MIN_CURRENT_WIDTH: Fx = 8 * ONE_UNIT;

/// A list of objective keys, each of which must name an authored objective.
fn read_keys(value: &Json, key: &str, objectives: &[Objective]) -> Result<Vec<String>, Fault> {
    let mut found = Vec::new();
    for entry in read::list(value, key, OBJECTIVES_PER_CHAPTER).map_err(invalid)? {
        let id = entry.as_text().ok_or_else(|| Fault::because(Code::ContentInvalid, key))?;
        if !objectives.iter().any(|held| held.id == id) {
            return Err(Fault::because(Code::ContentInvalid, key));
        }
        found.push(id.to_string());
    }
    Ok(found)
}

fn read_view(value: &Json) -> Result<ViewDeclaration, Fault> {
    let found = read::map(value, "opening_view").map_err(invalid)?;
    read::exact_keys(found, "opening_view", &["inside", "resolution", "surround", "window"])
        .map_err(invalid)?;
    let surround = match read::one_of(found, "surround", &["adjacent", "double", "whole"])
        .map_err(invalid)?
    {
        0 => Surround::Adjacent,
        1 => Surround::Double,
        _ => Surround::Whole,
    };
    Ok(ViewDeclaration {
        inside: read::ids(found, "inside", NODES_PER_RUN, i64::from(u32::MAX)).map_err(invalid)?,
        resolution: read::int(found, "resolution", 0, i64::from(u16::MAX)).map_err(invalid)? as u16,
        window: read::int(found, "window", 0, i64::from(u16::MAX)).map_err(invalid)? as u16,
        surround,
    })
}

/// What a chapter has placed, for the cross-checks a condition is held to.
///
/// Every identifier a condition names is checked against these at read time, so
/// a typo is refused at load with the field named rather than standing as a
/// condition that can never be met — which would read to a player as a sequence
/// that has quietly stopped.
struct Placements<'a> {
    layers: &'a [LayerParameters],
    ports: &'a [PortPlacement],
    routes: &'a [RoutePlacement],
    currents: &'a [CurrentState],
    forms: &'a [FormPlacement],
}

impl Placements<'_> {
    /// True when a Node id names something the chapter placed: a Port of any
    /// kind, or the Node a Form stands on.
    fn node(&self, id: u32) -> bool {
        self.ports.iter().any(|held| held.node == id)
            || self.forms.iter().any(|held| held.node == id)
    }

    /// True when a Node id names a Port that waits to be opened. Every other
    /// kind is established open, so naming one would be a condition that is met
    /// before the player does anything.
    fn closable_port(&self, id: u32) -> bool {
        self.ports.iter().any(|held| held.node == id && held.kind == NodeKind::Port)
    }

    fn route(&self, id: u32) -> bool {
        self.routes.iter().any(|held| held.route == id)
    }

    fn current(&self, id: u16) -> bool {
        self.currents.iter().any(|held| held.id == id)
    }

    fn layer(&self, id: u8) -> bool {
        self.layers.iter().any(|held| held.layer == id)
    }

    /// True when a pressure's target names something the chapter placed. A
    /// target of kind `none` names nothing by declaration and is always
    /// reachable: the pressure holds its own working target at each stage
    /// entry, which is the locked behaviour for the two that admit it.
    fn target(&self, target: crate::pressure::Target) -> bool {
        use crate::pressure::TargetKind;
        match (target.kind, target.id) {
            (TargetKind::None, _) => true,
            (TargetKind::Node, Some(id)) => self.node(id),
            (TargetKind::Route, Some(id)) => self.route(id),
            (TargetKind::Layer, Some(id)) => {
                u8::try_from(id).is_ok_and(|layer| self.layer(layer))
            }
            (_, None) => false,
        }
    }
}

/// Refuses a condition whose named identifier the chapter never placed. The
/// detail names the field, which is the whole of the diagnostic an author
/// needs: the condition's own key, and nothing about the run.
fn unplaced(field: &str) -> Fault {
    Fault::because(Code::ContentInvalid, field)
}

/// The closed condition set of `content_version` 1.
fn read_condition(value: &Json, placed: &Placements<'_>) -> Result<Condition, Fault> {
    let kind = read::text(value, "kind").map_err(invalid)?;
    let steps = |value: &Json| read::int(value, "steps", 1, FRAC_ONE).map_err(invalid);
    match kind {
        "in_current" => {
            read::exact_keys(value, "condition", &["current", "kind", "steps"]).map_err(invalid)?;
            let current =
                read::int(value, "current", 1, i64::from(u16::MAX)).map_err(invalid)? as u16;
            if !placed.current(current) {
                return Err(unplaced("current"));
            }
            Ok(Condition::InCurrent { current, steps: steps(value)? })
        }
        "hold_position" => {
            read::exact_keys(value, "condition", &["kind", "layer", "pos", "radius", "steps"])
                .map_err(invalid)?;
            let layer =
                read::int(value, "layer", 0, i64::from(field::MAX_LAYER)).map_err(invalid)? as u8;
            if !placed.layer(layer) {
                return Err(unplaced("layer"));
            }
            let pos = Vec2::read(value, "pos").map_err(invalid)?;
            // A point off the plane is a point no Form can ever stand at.
            if !(0..field::PLANE_SPAN).contains(&pos.x) || !(0..field::PLANE_SPAN).contains(&pos.y)
            {
                return Err(unplaced("pos"));
            }
            Ok(Condition::HoldPosition {
                layer,
                pos,
                radius: read::int(value, "radius", 1, STORED_BOUND - 1).map_err(invalid)?,
                steps: steps(value)?,
            })
        }
        "pulse_released" => {
            read::exact_keys(value, "condition", &["count", "kind"]).map_err(invalid)?;
            Ok(Condition::PulseReleased {
                count: read::int(value, "count", 1, FRAC_ONE).map_err(invalid)?,
            })
        }
        "ports_open" => {
            read::exact_keys(value, "condition", &["kind", "ports"]).map_err(invalid)?;
            let ports = nonempty(value, "ports", NODES_PER_RUN)?;
            if !ports.iter().all(|id| placed.closable_port(*id)) {
                return Err(unplaced("ports"));
            }
            Ok(Condition::PortsOpen { ports })
        }
        "routes_flowing" => {
            read::exact_keys(value, "condition", &["kind", "routes"]).map_err(invalid)?;
            let routes = nonempty(value, "routes", ROUTES_PER_RUN)?;
            if !routes.iter().all(|id| placed.route(*id)) {
                return Err(unplaced("routes"));
            }
            Ok(Condition::RoutesFlowing { routes })
        }
        "pattern_held" => {
            read::exact_keys(value, "condition", &["kind", "nodes", "routes", "steps"])
                .map_err(invalid)?;
            let routes = nonempty(value, "routes", ROUTES_PER_RUN)?;
            if !routes.iter().all(|id| placed.route(*id)) {
                return Err(unplaced("routes"));
            }
            let nodes = nonempty(value, "nodes", NODES_PER_RUN)?;
            if !nodes.iter().all(|id| placed.node(*id)) {
                return Err(unplaced("nodes"));
            }
            Ok(Condition::PatternHeld { routes, nodes, steps: steps(value)? })
        }
        _ => Err(Fault::because(Code::ContentInvalid, "kind")),
    }
}

fn nonempty(value: &Json, key: &str, longest: usize) -> Result<Vec<u32>, Fault> {
    let found = read::ids(value, key, longest, i64::from(u32::MAX)).map_err(invalid)?;
    if found.is_empty() {
        return Err(Fault::because(Code::ContentInvalid, key));
    }
    Ok(found)
}

/// The Forms one Form's abilities stand beside the controlled one: the Form
/// identifier and the placement of each, in authored order.
///
/// The offsets are measured from the chapter's own placement, so where the
/// linked Forms stand is the chapter's geometry and the Form's data together,
/// and an offset that would put one off the plane is content this build
/// refuses rather than a position it quietly clamps.
fn linked_placements(
    placement: &FormPlacement,
    highest: u8,
    form: &FormContent,
) -> Result<Vec<(u8, Vec2, LinkState)>, Fault> {
    let mut placed: Vec<(u8, Vec2, LinkState)> = Vec::new();
    for ability in &form.abilities {
        match ability {
            Ability::LinkedForms { offsets, separation } => {
                for offset in offsets {
                    // The identifiers carry on past the highest the chapter
                    // authored rather than past the anchor's own, so a chapter
                    // that places several Forms cannot collide with the ones
                    // its selection stands up. A chapter that places one is
                    // unchanged: its one placement is the highest.
                    let id = highest
                        .checked_add(placed.len() as u8 + 1)
                        .ok_or_else(|| Fault::because(Code::ContentInvalid, "abilities"))?;
                    let pos = Vec2::new(placement.pos.x + offset.x, placement.pos.y + offset.y);
                    if !(0..field::PLANE_SPAN).contains(&pos.x)
                        || !(0..field::PLANE_SPAN).contains(&pos.y)
                    {
                        return Err(Fault::because(Code::ContentInvalid, "offsets"));
                    }
                    // The offset is the station this Form holds for the life of
                    // the run, so it is carried rather than read back off the
                    // opening placement: the two agree only until the set moves.
                    placed.push((id, pos, LinkState { offset: *offset, separation: *separation }));
                }
            }
            Ability::Trail { .. } => {}
        }
    }
    Ok(placed)
}

/// True for a copy-catalog key of one kind.
///
/// The name after the kind may carry dots, which is what the per-chapter
/// namespace is written in: `objective.the_edge.hold_inside` opens with its
/// kind, as `docs/field-framework/LEXICON.md` requires, and reserves the rest
/// of the key to one chapter, so two chapters authored side by side cannot
/// collide.
///
/// What is admitted here is exactly LEXICON.md's stated key format,
/// `^[a-z][a-z0-9]*(\.[a-z][a-z0-9_]*)+$`, with the kind read as the literal
/// prefix the caller names: every segment after it opens with a lowercase
/// letter and carries only lowercase letters, digits, and underscores, so an
/// empty segment and a trailing dot are both refused. The document, the copy
/// check, and this reader are written to one pattern deliberately — a key this
/// admits and the check refuses would be authored content that loads and does
/// not ship.
fn is_key_of(kind: &str, id: &str) -> bool {
    let Some(name) = id.strip_prefix(kind) else {
        return false;
    };
    !name.is_empty()
        && name.split('.').all(|segment| {
            segment.starts_with(|first: char| first.is_ascii_lowercase())
                && segment
                    .chars()
                    .all(|held| held.is_ascii_lowercase() || held.is_ascii_digit() || held == '_')
        })
}

/// Builds the Field a chapter opens on, and the View it opens under.
///
/// Every quantity is authored except the stored Charge each Node starts with,
/// which is its kind's own — the document locks that per kind, so a chapter
/// does not restate it and cannot disagree with it.
///
/// The selected Form is read here and nowhere else, and it is read as data:
/// its parameters become the Field's own quantities, and its abilities shape
/// what stands in the Field. Nothing below asks which Form was selected — a
/// branch on a Form's name is the thing this seam exists to make unnecessary.
pub fn establish(
    chapter: &Chapter,
    form: &FormContent,
) -> Result<(FieldState, ViewDeclaration), Fault> {
    let placement = chapter.forms.first().ok_or_else(|| Fault::because(Code::ContentInvalid, "forms"))?;
    let highest = chapter.forms.iter().map(|held| held.id).max().unwrap_or(placement.id);
    let linked = linked_placements(placement, highest, form)?;

    let mut ports: Vec<PortState> = chapter
        .ports
        .iter()
        .map(|held| PortState {
            node: held.node,
            layer: held.layer,
            pos: held.pos,
            kind: held.kind,
            q: held.kind.starting_charge(),
            // Every Node kind other than `port` is established open: the open
            // gate is the participation rule, and only a Port waits.
            open: held.kind != NodeKind::Port,
            upkeep_rate: held.upkeep_rate,
            capacity: held.capacity,
        })
        .collect();
    // Every authored placement is a Form of the Field and a Node of it, in the
    // chapter's own order. A chapter that places one is exactly what it was.
    for held in &chapter.forms {
        ports.push(PortState {
            node: held.node,
            layer: held.layer,
            pos: held.pos,
            kind: NodeKind::Form,
            q: NodeKind::Form.starting_charge(),
            open: true,
            upkeep_rate: form.upkeep_rate,
            capacity: form.capacity,
        });
    }

    // The Trail a Form authors is carried by every Form of the selection, so a
    // linked Form leaves the same Trail the steered one does; the link a Form
    // stands in is carried by the linked Forms alone, because the Form the
    // chapter placed is the one they follow.
    let trail = form.abilities.iter().find_map(|ability| match ability {
        Ability::Trail { period, delay, radius, magnitude } => {
            Some(TrailState { period: *period, delay: *delay, radius: *radius, magnitude: *magnitude })
        }
        Ability::LinkedForms { .. } => None,
    });

    // The run's stored reserve stands with the Form the chapter opened control
    // on, and every other placement opens holding none — the same rule a linked
    // Form already stands under, for the same reason: the reserve is the run's
    // rather than each Form's.
    let mut forms: Vec<FormState> = chapter
        .forms
        .iter()
        .map(|held| FormState {
            id: held.id,
            form: form.id.clone(),
            node: held.node,
            controlled: held.controlled,
            layer: held.layer,
            pos: held.pos,
            vel: Vec2::default(),
            charge: NodeKind::Form.starting_charge(),
            reserve: if held.controlled { form.reserve } else { 0 },
            pulse_charge: 0,
            focus: false,
            route_reach: form.route_reach,
            forecast_depth: form.forecast_depth,
            steer_scale: form.steer_scale,
            route_capacity: form.route_capacity,
            link: None,
            trail,
        })
        .collect();

    // Each linked Form is a Form of the Field like any other: its own Node,
    // its own place in the two lists, the same authored parameters, and the
    // control the chapter placed on exactly one of them. The Node identifiers
    // carry on past the chapter's own, so a chapter that authors no link and a
    // chapter read for a Form that authors none establish identical Fields.
    // The stored reserve is the run's rather than each Form's, so it stands
    // with the controlled Form and a linked one opens holding none.
    let mut next_node = ports.iter().map(|port| port.node).max().unwrap_or(0) + 1;
    for (id, pos, link) in linked {
        ports.push(PortState {
            node: next_node,
            layer: placement.layer,
            pos,
            kind: NodeKind::Form,
            q: NodeKind::Form.starting_charge(),
            open: true,
            upkeep_rate: form.upkeep_rate,
            capacity: form.capacity,
        });
        forms.push(FormState {
            id,
            form: form.id.clone(),
            node: next_node,
            controlled: false,
            layer: placement.layer,
            pos,
            vel: Vec2::default(),
            charge: NodeKind::Form.starting_charge(),
            reserve: 0,
            pulse_charge: 0,
            focus: false,
            route_reach: form.route_reach,
            forecast_depth: form.forecast_depth,
            steer_scale: form.steer_scale,
            route_capacity: form.route_capacity,
            link: Some(link),
            trail,
        });
        next_node += 1;
    }
    ports.sort_by_key(|port| port.node);

    let routes: Vec<RouteState> = chapter
        .routes
        .iter()
        .map(|held| RouteState {
            route: held.route,
            tail: held.tail,
            head: held.head,
            capacity: held.capacity,
            flow: 0,
            formed_step: 0,
        })
        .collect();

    let currents = chapter.currents.clone();

    // A layer's lists name what stands on it: its currents, and its Ports other
    // than the Form-kind Nodes, which carry their own layer and move.
    let layers: Vec<FieldLayer> = chapter
        .layers
        .iter()
        .map(|held| FieldLayer {
            layer: held.layer,
            drain: held.drain,
            noise: held.noise,
            gain: held.gain,
            current_ids: currents
                .iter()
                .filter(|current| current.layer == held.layer)
                .map(|current| current.id)
                .collect(),
            port_ids: ports
                .iter()
                .filter(|port| port.layer == held.layer && port.kind != NodeKind::Form)
                .map(|port| port.node)
                .collect(),
        })
        .collect();

    let next_node_id = ports.iter().map(|port| port.node).max().unwrap_or(0) + 1;
    let next_route_id = routes.iter().map(|route| route.route).max().unwrap_or(0) + 1;

    let field = FieldState {
        step: 0,
        next_node_id,
        next_route_id,
        assembly_ordinal: 0,
        prev_assembly_step: None,
        wheel_accum: 0,
        depth_cooldown: 0,
        layers,
        ports,
        routes,
        forms,
        currents,
        boundaries: BoundaryState {
            drawn: Vec::new(),
            authored: chapter.authored_boundaries.clone(),
            // The run's boundary-leakage parameter is copied from the selected
            // Form at run start, which is what makes the boundary a Form's own
            // promise rather than a layer's.
            leak_frac: form.leak_frac,
        },
        // A run opens with no Trail standing: the first entry is left by the
        // first step whose count the authored period names.
        pending: Vec::new(),
    };
    Ok((field, chapter.opening_view.clone()))
}

/// What the script asked for on one step, beyond what it wrote into the state.
#[derive(Clone, Debug, Default)]
pub struct ScriptOutcome {
    /// The objective states whose id or stage changed, with the id that stood
    /// before each — the body of an `objective_changed` event.
    pub changed: Vec<(ObjectiveState, Option<String>)>,
    /// True when this step completed an objective the chapter names an Anchor
    /// moment at.
    pub anchor: bool,
    /// True when this step left the chapter with nothing more to offer: every
    /// required objective complete, and every optional test either taken or
    /// passed over. It is the chapter's completion condition, and the caller
    /// carries the run into the next chapter or ends the campaign on it.
    pub chapter_complete: bool,
}

/// Advances the authored objective sequence by one completed step.
///
/// One objective is active at a time and they are offered in authored order:
/// the active one is the first the run has not completed, so which objective
/// stands is derived from `progress.complete` and never stored twice. Progress
/// is the locked `Frac`, advanced by the condition's own per-step share, so the
/// whole of what the script keeps between steps rides in the payload.
pub fn advance_objectives(
    progress: &mut Progress,
    field: &FieldState,
    chapter: &Chapter,
    reading: &StepReading<'_>,
    cues: &mut Vec<Cue>,
) -> ScriptOutcome {
    let mut outcome = ScriptOutcome::default();
    let step = field.step;

    // The optional test's own clock, read before anything else this step: an
    // optional objective that has stood for the span it authored is passed
    // over, and the sequence carries on from what stands after it. Nothing is
    // written into the completed list — a test that was not taken was not
    // completed — and nothing needs to be, because the standing objective
    // moving past it is what the sequence reads next step.
    if passing_over(chapter, progress, step) {
        let passed = progress.objective.id.clone();
        match chapter.offered_past(progress, &passed) {
            Some(next) => {
                progress.objective = offered_state(next, step);
                outcome.changed.push((progress.objective.clone(), Some(passed)));
            }
            None => {
                // The passed-over test was the last thing the chapter had to
                // offer, so letting it go is what completes the chapter.
                progress.objective = ObjectiveState::hidden();
                outcome.changed.push((progress.objective.clone(), Some(passed)));
                outcome.chapter_complete = true;
                // What a close is worth, paid once and whatever closed it. A
                // chapter closed by a completion is paid by that completion —
                // the locked grant every completed objective carries — and a
                // chapter closed by letting a test go has no completion on the
                // step to pay it, so the close grants. Both readings are three,
                // clamped at the cap, and neither pays twice.
                progress.grant_impulse();
                return outcome;
            }
        }
    }

    let Some(objective) = chapter.offered(progress) else {
        // The chapter has nothing more to offer, so the line clears: one
        // objective is visible at a time, and none is one of the counts that
        // rule allows. The hidden state is the shape the payload declares for
        // exactly this — no objective offered, and an empty id beside it.
        if progress.objective.state != ObjectiveStage::Hidden {
            let previous = progress.objective.id.clone();
            progress.objective = ObjectiveState::hidden();
            outcome.changed.push((progress.objective.clone(), Some(previous)));
        }
        outcome.chapter_complete = true;
        return outcome;
    };

    // Offering the next objective: the one place an objective's id changes.
    if progress.objective.id != objective.id
        || progress.objective.state == ObjectiveStage::Complete
    {
        let previous = if progress.objective.id.is_empty() {
            None
        } else {
            Some(progress.objective.id.clone())
        };
        progress.objective = offered_state(objective, step);
        outcome.changed.push((progress.objective.clone(), previous));
    }

    let read = objective.condition.read_step(reading);
    let held = &mut progress.objective;
    if read.met {
        held.progress = (held.progress + objective.condition.share()).min(FRAC_ONE);
    } else if !objective.condition.cumulative() {
        held.progress = 0;
    }

    // The authored setback, and the recovery from it. Neither is terminal: the
    // state set carries no terminal failure, and the run continues either way.
    let stage = if read.setback {
        ObjectiveStage::FailedRecoverable
    } else {
        ObjectiveStage::Active
    };
    if stage != held.state {
        held.state = stage;
        cues.push(Cue {
            kind: if stage == ObjectiveStage::FailedRecoverable { CUE_COLLAPSE } else { CUE_RECOVERY },
            a: 0,
            b: reading.controlled().map_or(0, |form| form.node),
        });
        outcome.changed.push((held.clone(), Some(held.id.clone())));
    }

    if held.progress < FRAC_ONE {
        return outcome;
    }

    held.state = ObjectiveStage::Complete;
    held.completed_step = Some(step);
    let done = held.id.clone();
    outcome.changed.push((held.clone(), Some(done.clone())));
    cues.push(Cue { kind: CUE_OBJECTIVE_COMPLETE, a: 0, b: reading.controlled().map_or(0, |form| form.node) });

    // The completed list stays ascending, which is what the payload declares.
    let place = progress.complete.partition_point(|held| held < &done);
    progress.complete.insert(place, done.clone());

    // The locked recovery: a completed layer objective gives three Impulse
    // back, clamped at the cap. Every authored objective of a chapter is one of
    // that layer's objectives — the chapter's list is the sequence a player
    // works through on the layer, and no authored field distinguishes a subset
    // of it — so every completion grants, and the clamp is what keeps a run
    // that completes several in a row inside the six it may carry. It fires
    // once per objective because an objective completes once: the id joins the
    // completed list on the same step, and the sequence offers the next.
    //
    // The grant is written into progress, which no replay re-runs, so a
    // regeneration neither loses it nor gives it twice.
    progress.grant_impulse();

    outcome.anchor = chapter.anchor_moments.iter().any(|held| held == &done);
    // The completion that leaves the chapter with nothing more to offer is the
    // chapter's own completion. It is read here rather than waited for on the
    // next step, so the transition lands on the step that earned it and the
    // Anchor beside it names that step.
    outcome.chapter_complete = chapter.offered(progress).is_none();
    outcome
}

/// True when the standing objective is an optional test whose span has run out
/// on this step.
///
/// The reading is an equality on the step counter rather than a comparison, so
/// it is true on exactly one step: the one that carries the run past the span.
/// A test already met is not passed over — a completion is what it was for —
/// and neither is one the run is not standing on.
fn passing_over(chapter: &Chapter, progress: &Progress, step: Step) -> bool {
    if progress.objective.state == ObjectiveStage::Complete {
        return false;
    }
    let Some(objective) = chapter.objective(&progress.objective.id) else {
        return false;
    };
    let Some(span) = objective.optional else {
        return false;
    };
    progress.objective.started_step.saturating_add(span) == step
}

/// One objective as it stands the moment it is offered.
fn offered_state(objective: &Objective, step: Step) -> ObjectiveState {
    ObjectiveState {
        id: objective.id.clone(),
        state: ObjectiveStage::Active,
        progress: 0,
        target: Some(objective.condition.target()),
        started_step: step,
        completed_step: None,
    }
}

/// Offers the objective a chapter opens on, before a single step has run.
///
/// The opening objective is the first the sequence offers, and it is offered at
/// establishment rather than on the first step so a player reads what to do
/// before anything moves. A run that already stands on one keeps it.
pub fn offer_opening(progress: &mut Progress, chapter: &Chapter, step: Step) -> bool {
    if !progress.objective.id.is_empty() {
        return false;
    }
    let Some(objective) = chapter.offered(progress) else {
        return false;
    };
    progress.objective = offered_state(objective, step);
    true
}

/// The authored events that fall due at this step boundary, in authored order.
///
/// Both halves of the trigger are payload state: the step the standing
/// objective was offered at, and the step counter. The reading is an equality,
/// so an event lands on exactly one boundary, and it is read against the
/// objective the run stands on, so an event written for a challenge a player
/// finished early never lands at all.
pub fn events_due<'a>(
    chapter: &'a Chapter,
    progress: &Progress,
    step: Step,
) -> Vec<&'a AuthoredEvent> {
    let standing = &progress.objective;
    if standing.id.is_empty() || standing.state == ObjectiveStage::Complete {
        return Vec::new();
    }
    chapter
        .events
        .iter()
        .filter(|event| {
            event.objective == standing.id
                && standing.started_step.saturating_add(event.at) == step
        })
        .collect()
}

/// Applies one authored event to the Field, and answers whether it changed
/// anything.
///
/// Every arm is a setter on authored Field state: no Charge moves, nothing is
/// drawn, and no ledger entry is written. An event naming something the Field
/// no longer holds changes nothing and says so, which is the same reading a
/// Fracture takes of a Route that has already been cut.
pub fn apply_event(field: &mut FieldState, event: &AuthoredEvent) -> bool {
    match &event.effect {
        Effect::PortOpen { node, open } => field
            .ports
            .iter_mut()
            .find(|port| port.node == *node)
            .is_some_and(|port| std::mem::replace(&mut port.open, *open) != *open),
        Effect::LayerDrain { layer, drain } => field
            .layers
            .iter_mut()
            .find(|held| held.layer == *layer)
            .is_some_and(|held| std::mem::replace(&mut held.drain, *drain) != *drain),
        Effect::CurrentActive { current, active } => field
            .currents
            .iter_mut()
            .find(|held| held.id == *current)
            .is_some_and(|held| std::mem::replace(&mut held.active, *active) != *active),
        // The Route leaves the set and `next_route_id` does not move:
        // identifiers are never reused, so a cut leaves no gap to fill. This is
        // the committed cut's own rule, restated where the event applies it.
        Effect::RouteCut { route } => {
            let before = field.routes.len();
            field.routes.retain(|held| held.route != *route);
            field.routes.len() != before
        }
    }
}

/// The ordinal the render snapshot's header carries: the 1-based position of
/// the standing objective in the chapter's authored order, and 0 when none
/// stands.
pub fn objective_ordinal(chapter: Option<&Chapter>, progress: &Progress) -> u16 {
    let Some(chapter) = chapter else {
        return 0;
    };
    if progress.objective.id.is_empty() {
        return 0;
    }
    chapter
        .objectives
        .iter()
        .position(|held| held.id == progress.objective.id)
        .map_or(0, |place| (place + 1).min(usize::from(u16::MAX)) as u16)
}

/// Where an objective's own explanation stands in the copy catalog: the same
/// name under the `explanation` kind. The shell derives it the same way.
pub fn explanation_key(objective_id: &str) -> Option<String> {
    objective_id.strip_prefix("objective.").map(|name| format!("explanation.{name}"))
}

/// The step count one authored target actually spans, which is the target to
/// within one share.
pub fn effective_steps(condition: &Condition) -> i64 {
    let share = condition.share();
    (FRAC_ONE + share - 1) / share
}

