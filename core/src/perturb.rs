//! The eight required perturbations, their compact playback records, and the
//! Echo highlight one committed change leaves behind.
//!
//! `docs/field-framework/FRAMEWORK.md`'s Required perturbations section is the
//! whole of what this module does, carried out exactly as it is written: the
//! common replay frame and its eight samples, the per-kind operational edit,
//! the resolved default of every parameter a kind takes, the excess taken
//! against the shared baseline of the same sample number — and, for delayed
//! replay alone, against its own unshifted base — the median reading with its
//! [smallest, largest] confidence range, and the records every result carries.
//! `docs/field-framework/ARCHITECTURE.md` locks the serialized shapes
//! (`PerturbationResult` with the three recomputing kinds' own payloads, and
//! `EchoHighlight`), the `InspectRequest` surface they are asked for through,
//! and the on-demand budget they run inside.
//!
//! Four properties are load-bearing, and they are what the tests pin:
//!
//! - **Reproducible.** Identical inputs — the same Field, View, `sigma_V`,
//!   tolerance, kind, and parameters — reproduce an identical result, byte for
//!   byte. Every stream is a named split of `sigma_V`, every ratio goes through
//!   the one rounding rule [`crate::rank::frac_of`] holds, and no float appears.
//! - **Session-lived.** A result is never serialized into the run state and
//!   never enters a save payload. What stands after a restore is `sigma_V` and
//!   the resolved parameters, which is what makes the result reproducible rather
//!   than stored.
//! - **Replayed, never redrawn.** Every replay starts from the window's start
//!   state and drives control from the recorded `ctl` of each step — the locked
//!   replay-time input policy. A delayed replay shifts exogenous events only;
//!   the control schedule never shifts. A replay never touches the recorded
//!   trajectory.
//! - **Defaults resolved.** A kind whose parameter the caller left null records
//!   the parameter it actually used, not the null it was handed. Two kinds take
//!   no parameter at all and record null for it.
//!
//! What this module does not do is rank. No reading here enters the dominance
//! comparison, and no reading here is combined with any other.

use crate::field::{self, NodeKind, PortState, StepCache};
use crate::json::Obj;
use crate::rank::{
    self, agreement, baselines, frac_of, median, range_of, sample_stream_under, stream_name,
    symbols, symbols_under, Sets, Structure, Window,
};
use crate::rng::{Part, RngState};
use crate::slate::{
    written_value, Candidate, PrivilegeProfile, PrivilegeValue, Provenance, PRIVILEGE_VALUES,
    WINDOW_TOO_SHORT,
};
use crate::state::{FieldState, Frac, Fx, RunState, Surround, ViewDeclaration, FRAC_ONE};

/// The eight kinds, by the machine name FRAMEWORK.md gives each one's stream
/// and ARCHITECTURE.md gives each one's `InspectRequest`.
pub const BOUNDARY_SEVERANCE: &str = "boundary-severance";
pub const ROUTE_REMOVAL: &str = "route-removal";
pub const COMPONENT_SUBSTITUTION: &str = "component-substitution";
pub const RESOLUTION_CHANGE: &str = "resolution-change";
pub const WINDOW_CHANGE: &str = "window-change";
pub const SURROUND_CHANGE: &str = "surround-change";
pub const DELAYED_REPLAY: &str = "delayed-replay";
pub const FULL_TURNOVER: &str = "full-turnover";

/// The closed kind set, in the order FRAMEWORK.md defines them.
pub const KINDS: [&str; 8] = [
    BOUNDARY_SEVERANCE,
    ROUTE_REMOVAL,
    COMPONENT_SUBSTITUTION,
    RESOLUTION_CHANGE,
    WINDOW_CHANGE,
    SURROUND_CHANGE,
    DELAYED_REPLAY,
    FULL_TURNOVER,
];

/// The `EchoHighlight` kind a highlight carries when it derives from the
/// adopted candidate's evaluation record rather than from a perturbation.
pub const ECHO_EVALUATION: &str = "evaluation";

/// The stream part every perturbation stream carries after its candidate parts.
const PERTURBATION: &str = "perturbation";

/// How many samples the common replay frame takes.
const SAMPLES: usize = rank::SAMPLES;

/// Reasons a reading is unassigned, beyond the slate's own closed list.
///
/// Each names one of FRAMEWORK.md's own stated requirements: a window with too
/// few steps to compare (the shared `window-too-short`), no Route whose removal
/// the kind's default rule could name, no privilege value assigned at both
/// windows, and the Scale Stability requirement a resolution change inherits.
pub const NO_ROUTE: &str = "no-route-to-remove";
pub const NO_SHARED_WINDOW: &str = "no-shared-value";
pub const NO_MEMBER: &str = "no-member";

// ---------------------------------------------------------------------------
// The result record
// ---------------------------------------------------------------------------

/// One replayed sample, with the compact playback record it retained.
///
/// `series` is the replayed inside's stored-Charge series — one `Fx` per
/// replayed step, at most `w` of them — which is what ARCHITECTURE.md keeps
/// "retained for playback". `base_series` and `base_deviation` stand beside it
/// only for delayed replay, whose base is its own unshifted-schedule replay
/// rather than the shared baseline; for every other kind they are null and the
/// excess is taken against baseline sample `j`.
#[derive(Clone, Debug, Default)]
pub struct Sample {
    pub deviation: Option<Frac>,
    pub excess: Option<Frac>,
    pub series: Vec<Fx>,
    pub base_deviation: Option<Frac>,
    pub base_series: Option<Vec<Fx>>,
}

/// What a recomputing kind recomputed. Null for the five replaying kinds.
#[derive(Clone, Debug)]
pub enum Recomputed {
    /// Resolution change: the grains used, the qualifying pair values keyed by
    /// their smaller grain, and the block series at each grain.
    Resolution {
        grains: Vec<u16>,
        pairs: Vec<(u16, Frac)>,
        blocks: Vec<(u16, Vec<Vec<Fx>>)>,
    },
    /// Window change: each recomputed window with its four values and the
    /// largest movement they showed against the declared window.
    Window { windows: Vec<(u16, PrivilegeProfile, Frac)> },
    /// Surround change: the new rule, and Shared Failure under each rule.
    Surround { rule: Surround, old: PrivilegeValue, new: PrivilegeValue },
}

/// The FRAMEWORK.md result record, as ARCHITECTURE.md shapes it.
#[derive(Clone, Debug)]
pub struct PerturbationResult {
    pub view: ViewDeclaration,
    pub provenance: Vec<Provenance>,
    pub position: u8,
    pub sigma: RngState,
    pub streams: Vec<String>,
    pub kind: &'static str,
    /// The resolved parameter: what the kind actually used, never the null a
    /// caller handed it. Null only for the two kinds that take none.
    pub parameter: Option<i64>,
    pub tau: Frac,
    pub reading: PrivilegeValue,
    pub samples: Vec<Sample>,
    pub recomputed: Option<Recomputed>,
    pub step: u32,
}

impl PerturbationResult {
    pub fn write(&self, out: &mut String) {
        let mut object = Obj::new(out);
        object.text("kind", self.kind);
        object.int_or_null("parameter", self.parameter);
        object.int("position", i64::from(self.position));
        {
            let mut provenance = object.list("provenance");
            for entry in &self.provenance {
                let mut held = provenance.object();
                held.int_or_null("detail", entry.detail);
                held.text("source", entry.source.name());
                held.end();
            }
            provenance.end();
        }
        object.raw("reading", &written_value(&self.reading));
        match &self.recomputed {
            None => {
                object.null("recomputed");
            }
            Some(Recomputed::Resolution { grains, pairs, blocks }) => {
                let mut held = object.object("recomputed");
                {
                    let mut written = held.list("blocks");
                    for (grain, series) in blocks {
                        let mut entry = written.object();
                        entry.int("grain", i64::from(*grain));
                        {
                            let mut rows = entry.list("series");
                            for row in series {
                                rows.raw(&written_series(row));
                            }
                            rows.end();
                        }
                        entry.end();
                    }
                    written.end();
                }
                {
                    let mut written = held.list("grains");
                    for grain in grains {
                        written.int(i64::from(*grain));
                    }
                    written.end();
                }
                {
                    let mut written = held.list("pairs");
                    for (grain, value) in pairs {
                        let mut entry = written.object();
                        entry.int("grain", i64::from(*grain));
                        entry.int("value", *value);
                        entry.end();
                    }
                    written.end();
                }
                held.end();
            }
            Some(Recomputed::Window { windows }) => {
                let mut held = object.object("recomputed");
                let mut written = held.list("windows");
                for (width, profile, deviation) in windows {
                    let mut entry = written.object();
                    entry.int("deviation", *deviation);
                    {
                        let mut privilege = entry.object("privilege");
                        for (name, value) in PRIVILEGE_VALUES.iter().zip(profile.each()) {
                            privilege.raw(name, &written_value(value));
                        }
                        privilege.end();
                    }
                    entry.int("w", i64::from(*width));
                    entry.end();
                }
                written.end();
                held.end();
            }
            Some(Recomputed::Surround { rule, old, new }) => {
                let mut held = object.object("recomputed");
                held.raw("new", &written_value(new));
                held.raw("old", &written_value(old));
                held.text("rule", rule.name());
                held.end();
            }
        };
        {
            let mut samples = object.list("samples");
            for sample in &self.samples {
                let mut held = samples.object();
                held.int_or_null("base_deviation", sample.base_deviation);
                match &sample.base_series {
                    Some(series) => {
                        let mut values = held.list("base_series");
                        for value in series {
                            values.int(*value);
                        }
                        values.end();
                    }
                    None => {
                        held.null("base_series");
                    }
                }
                held.int_or_null("deviation", sample.deviation);
                held.int_or_null("excess", sample.excess);
                {
                    let mut values = held.list("series");
                    for value in &sample.series {
                        values.int(*value);
                    }
                    values.end();
                }
                held.end();
            }
            samples.end();
        }
        object.raw("sigma", &self.sigma.write());
        object.int("step", i64::from(self.step));
        {
            let mut streams = object.list("streams");
            for name in &self.streams {
                streams.text(name);
            }
            streams.end();
        }
        object.int("tau", self.tau);
        {
            let mut view = String::new();
            self.view.write(&mut view);
            object.raw("view", &view);
        }
        object.end();
    }

    pub fn written(&self) -> String {
        let mut out = String::new();
        self.write(&mut out);
        out
    }

    /// The Echo this result leaves: the largest excess deviation it found, and
    /// the confidence range of the reading that found it.
    ///
    /// FRAMEWORK.md's Echo rule says the highlight names the largest excess
    /// deviation in the result, and ARCHITECTURE.md says the three values a
    /// perturbation-backed highlight carries are that excess and its confidence
    /// range. A result whose reading is unassigned leaves no highlight: there
    /// is no excess to name, and a zero would be a reading the record does not
    /// hold.
    pub fn highlight(&self) -> Option<EchoHighlight> {
        let (value, low, high) = match (self.reading.value, self.reading.low, self.reading.high) {
            (Some(value), Some(low), Some(high)) => (value, low, high),
            _ => return None,
        };
        let largest = self
            .samples
            .iter()
            .filter_map(|sample| sample.excess)
            .max()
            .unwrap_or(value);
        Some(EchoHighlight {
            kind: self.kind,
            parameter: self.parameter,
            excess: largest,
            low,
            high,
            target: match self.kind {
                ROUTE_REMOVAL => EchoTarget::Route(self.parameter.unwrap_or(0) as u32),
                COMPONENT_SUBSTITUTION => EchoTarget::Node(self.parameter.unwrap_or(0) as u32),
                _ => EchoTarget::None,
            },
        })
    }
}

/// One compact playback series, as canonical JSON: a flat array of raw `Fx`,
/// one entry per replayed step.
fn written_series(series: &[Fx]) -> String {
    let mut out = String::new();
    let mut values = crate::json::Arr::new(&mut out);
    for value in series {
        values.int(*value);
    }
    values.end();
    out
}

/// What an Echo points at on the Field: a Node, a Route, or nowhere.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EchoTarget {
    None,
    Node(u32),
    Route(u32),
}

impl EchoTarget {
    fn name(self) -> &'static str {
        match self {
            EchoTarget::None => "none",
            EchoTarget::Node(_) => "node",
            EchoTarget::Route(_) => "route",
        }
    }

    fn id(self) -> Option<i64> {
        match self {
            EchoTarget::None => None,
            EchoTarget::Node(id) | EchoTarget::Route(id) => Some(i64::from(id)),
        }
    }
}

/// The one short causal highlight a committed change leaves.
///
/// The framework supplies the readings and the game supplies the wording: this
/// record carries the kind, the resolved parameter, and the three numbers, and
/// the shell chooses one catalog line by the kind. Nothing here is a report.
#[derive(Clone, Debug)]
pub struct EchoHighlight {
    pub kind: &'static str,
    pub parameter: Option<i64>,
    pub excess: Frac,
    pub low: Frac,
    pub high: Frac,
    pub target: EchoTarget,
}

impl EchoHighlight {
    pub fn write(&self, out: &mut String) {
        let mut object = Obj::new(out);
        object.int("excess", self.excess);
        object.int("high", self.high);
        object.text("kind", self.kind);
        object.int("low", self.low);
        object.int_or_null("parameter", self.parameter);
        {
            let mut target = object.object("target");
            target.int_or_null("id", self.target.id());
            target.text("t", self.target.name());
            target.end();
        }
        object.end();
    }

    pub fn written(&self) -> String {
        let mut out = String::new();
        self.write(&mut out);
        out
    }
}

/// The Echo of an adopted candidate: the largest of its eight recorded baseline
/// deviations, and their [smallest, largest] spread, with the slate ordinal as
/// the parameter.
///
/// This is the branch ARCHITECTURE.md locks for a committed reshape or
/// adoption: those commits run no perturbation at all, and the highlight comes
/// from the evaluation record instead. A candidate with no defined baseline
/// deviation leaves no highlight.
pub fn evaluation_highlight(candidate: &Candidate, ordinal: u32) -> Option<EchoHighlight> {
    let held: Vec<Frac> = candidate.baseline.iter().flatten().copied().collect();
    let low = *held.iter().min()?;
    let high = *held.iter().max()?;
    Some(EchoHighlight {
        kind: ECHO_EVALUATION,
        parameter: Some(i64::from(ordinal)),
        excess: high,
        low,
        high,
        target: EchoTarget::None,
    })
}

// ---------------------------------------------------------------------------
// Shared replay machinery
// ---------------------------------------------------------------------------

/// The recorded window as every deviation reads it: the inside's stored-Charge
/// series, the one margin that series gives, and its direction symbols.
pub(crate) struct Recorded {
    #[allow(dead_code)]
    pub series: Vec<Fx>,
    pub margin: Fx,
    pub symbols: Vec<i8>,
}

impl Recorded {
    pub(crate) fn of(window: &Window<'_>, structure: &Structure, sets: &Sets, tau: Frac) -> Self {
        let series: Vec<Fx> = window
            .steps
            .iter()
            .map(|step| rank::sum_marked(&step.records.q, &structure.nodes, &sets.member))
            .collect();
        let margin = range_of(&series);
        let symbols = symbols(&series, tau);
        Recorded { series, margin, symbols }
    }

    /// The replay deviation of a replayed series against this recorded one:
    /// `1 - agreement`, both series read under the one margin the recorded
    /// series gives. Unassigned when there is no step to compare.
    pub(crate) fn deviation(&self, replayed: &[Fx], tau: Frac) -> Option<Frac> {
        agreement(&self.symbols, &symbols_under(replayed, self.margin, tau))
            .map(|held| FRAC_ONE - held)
    }
}

/// A Field prepared for replay: the state a replay starts from, the pass
/// prepared for its causal shape, and the member identifiers the observation's
/// series is summed over.
pub(crate) struct Prepared {
    pub start: FieldState,
    pub cache: StepCache,
    pub members: Vec<u32>,
}

impl Prepared {
    /// The window's own start state, unedited.
    pub(crate) fn plain(window: &Window<'_>, sets: &Sets) -> Self {
        Prepared {
            start: window.start.clone(),
            cache: window.cache.clone(),
            members: sets.members.clone(),
        }
    }

    /// The start state with a set of Routes removed for the whole replay, and
    /// the pass prepared for the shape that leaves.
    pub(crate) fn without(window: &Window<'_>, sets: &Sets, routes: &[u32]) -> Self {
        let mut start = window.start.clone();
        start.routes.retain(|held| !routes.contains(&held.route));
        let cache = StepCache::of(&start);
        Prepared {
            start,
            cache,
            members: sets.members.clone(),
        }
    }

    /// The start state with each named member replaced.
    ///
    /// FRAMEWORK.md's replacement rule, one sentence at a time: a fresh Node of
    /// the same kind holding its kind's starting Charge; the replacement takes
    /// over the member's Routes and declared adjacency; it carries a fresh
    /// identifier; and it takes the member's place in the inside. The declared
    /// adjacency is positional in this Field — `adj` is a distance test — so a
    /// replacement standing where the member stood is adjacent to exactly what
    /// the member was adjacent to, and taking over the adjacency is taking over
    /// the place.
    pub(crate) fn substituting(window: &Window<'_>, sets: &Sets, replaced: &[u32]) -> Self {
        let mut start = window.start.clone();
        let mut fresh: Vec<(u32, u32)> = Vec::with_capacity(replaced.len());
        for node in replaced {
            let Some(place) = start.ports.iter().position(|port| port.node == *node) else {
                continue;
            };
            let id = start.next_node_id;
            start.next_node_id = start.next_node_id.saturating_add(1);
            let held = &start.ports[place];
            let taken = PortState {
                node: id,
                layer: held.layer,
                pos: held.pos,
                kind: held.kind,
                q: held.kind.starting_charge().min(held.capacity),
                open: held.open,
                upkeep_rate: held.upkeep_rate,
                capacity: held.capacity,
            };
            start.ports[place] = taken;
            fresh.push((*node, id));
        }
        for (old, new) in &fresh {
            for route in start.routes.iter_mut() {
                if route.tail == *old {
                    route.tail = *new;
                }
                if route.head == *old {
                    route.head = *new;
                }
            }
            // A Form whose Node was replaced follows the replacement: a Form and
            // its Node are one placed thing, and a Form left naming a Node that
            // no longer stands would mirror nothing.
            for form in start.forms.iter_mut() {
                if form.node == *old {
                    form.node = *new;
                    form.charge = NodeKind::Form.starting_charge();
                }
            }
            for layer in start.layers.iter_mut() {
                for node in layer.port_ids.iter_mut() {
                    if node == old {
                        *node = *new;
                    }
                }
                layer.port_ids.sort_unstable();
            }
            for member in start.physical_compartment.members.iter_mut() {
                if member == old {
                    *member = *new;
                }
            }
        }
        // Every list a Field declares is ascending by identifier, and a fresh
        // identifier does not land where the one it replaced stood.
        start.ports.sort_by_key(|port| port.node);
        let members: Vec<u32> = {
            let mut held: Vec<u32> = sets
                .members
                .iter()
                .map(|node| {
                    fresh
                        .iter()
                        .find(|(old, _)| old == node)
                        .map(|(_, new)| *new)
                        .unwrap_or(*node)
                })
                .collect();
            held.sort_unstable();
            held
        };
        // Component substitution is an explicit cloned intervention. If the
        // replaced material stood inside the physical compartment, its fresh
        // identifier takes that same physical seat; the observation set above
        // is remapped independently.
        start.physical_compartment.members.sort_unstable();
        let cache = StepCache::of(&start);
        Prepared { start, cache, members }
    }
}

/// Replays the window from a prepared start state and hands back the replayed
/// inside's stored-Charge series — the compact playback record.
///
/// `drive` runs after each step, before the series is read, and is handed the
/// records that step produced. It is where the two schedule-driven procedures
/// put the recorded exogenous terms and crossing flows back onto the Field, and
/// it is a no-op for every ordinary edit.
pub(crate) fn replay_series(
    window: &Window<'_>,
    prepared: &Prepared,
    stream: RngState,
    mut drive: impl FnMut(usize, &mut FieldState, &crate::field::StepRecords),
) -> Vec<Fx> {
    // The sample's parent stream passes through to event-addressed Route Noise
    // and Supply variability. Unaffected object/step addresses remain paired
    // even when this prepared world differs structurally.
    let mut stream = stream;
    let mut scratch = window.pressures.clone();
    let mut state = prepared.start.clone();
    let mut series = Vec::with_capacity(window.len());
    for recorded in window.steps.iter() {
        let outcome = field::advance_cached(
            &mut state,
            recorded.ctl,
            window.pointer_speed,
            &mut field::Staging {
                pressures: &mut scratch,
                schedule: &window.schedule,
                stream: &mut stream,
                medium: window.medium,
                supply_jitter: window.supply_jitter,
            },
            &prepared.cache,
        );
        drive(series.len(), &mut state, &outcome.records);
        series.push(stored_over(&state, &prepared.members));
    }
    series
}

/// The stored Charge a named set of Nodes holds, read off a Field state.
pub(crate) fn stored_over(state: &FieldState, members: &[u32]) -> Fx {
    state
        .ports
        .iter()
        .filter(|port| members.binary_search(&port.node).is_ok())
        .map(|port| port.q)
        .sum()
}

/// Moves one Node's stored Charge by a signed amount, held inside the Field's
/// own bound, and gives every Form the stored Charge of its own Node again.
///
/// This is how a schedule is driven: the step function has already run, and the
/// correction puts the recorded term where the computed one stood. The bound is
/// the Field's, not this module's — a Node never holds less than nothing or
/// more than the stored-Charge cap, whatever a schedule asks.
pub(crate) fn move_stored(state: &mut FieldState, node: u32, by: Fx) {
    // The Port list is ascending by identifier — Field validation says so — and
    // a schedule drives one correction per recorded entry per replayed step, so
    // the search has to be the binary one or the pass is quadratic in the Node
    // count for every driven step.
    let Ok(place) = state.ports.binary_search_by_key(&node, |port| port.node) else {
        return;
    };
    let held = &mut state.ports[place];
    held.q = (held.q + by).clamp(0, crate::fx::STORED_BOUND - 1);
    let q = held.q;
    for form in state.forms.iter_mut() {
        if form.node == node {
            form.charge = q;
        }
    }
}

// ---------------------------------------------------------------------------
// The request, and the run
// ---------------------------------------------------------------------------

/// One perturbation asked for: the kind, and the parameter the caller named or
/// null for the kind's own resolved default.
#[derive(Clone, Copy, Debug)]
pub struct Request {
    pub kind: &'static str,
    pub parameter: Option<u32>,
}

impl Request {
    /// The kind named by a machine name, and none for a name outside the closed
    /// set.
    pub fn of(kind: &str, parameter: Option<u32>) -> Option<Self> {
        KINDS.iter().find(|held| **held == kind).map(|held| Request { kind: held, parameter })
    }
}

/// Runs one perturbation over one View of one run state.
///
/// `position` is the candidate's assembly position, 1-based, or 0 for a View
/// evaluated outside a slate — which is every on-demand inspection and every
/// post-commit Echo. `sigma` is the evaluation's own `sigma_V`.
pub fn run(
    state: &RunState,
    view: &ViewDeclaration,
    provenance: &[Provenance],
    position: u8,
    sigma: &RngState,
    tau: Frac,
    request: Request,
) -> PerturbationResult {
    let effective = state.effective_window(view.window);
    let structure = Structure::of(&state.now);
    let window = Window::of(state, effective);
    let sets = Sets::of(&structure, view);
    let mut result = PerturbationResult {
        view: view.clone(),
        provenance: provenance.to_vec(),
        position,
        sigma: *sigma,
        streams: Vec::new(),
        kind: request.kind,
        parameter: None,
        tau,
        reading: PrivilegeValue::unassigned(WINDOW_TOO_SHORT),
        samples: Vec::new(),
        recomputed: None,
        step: state.now.step,
    };
    match request.kind {
        RESOLUTION_CHANGE => resolution_change(&window, &structure, &sets, view, tau, &mut result),
        WINDOW_CHANGE => {
            window_change(state, &structure, view, sigma, position, tau, effective, &mut result)
        }
        SURROUND_CHANGE => {
            surround_change(&window, &structure, &sets, view, sigma, position, tau, &mut result)
        }
        DELAYED_REPLAY => delayed_replay(
            &window,
            &structure,
            &sets,
            sigma,
            position,
            tau,
            request.parameter,
            &mut result,
        ),
        _ => replaying(
            &window,
            &structure,
            &sets,
            sigma,
            position,
            tau,
            request,
            &mut result,
        ),
    }
    result
}

/// The common replay frame: the five kinds whose reading is a median excess
/// over eight edited replays.
#[allow(clippy::too_many_arguments)]
fn replaying(
    window: &Window<'_>,
    structure: &Structure,
    sets: &Sets,
    sigma: &RngState,
    position: u8,
    tau: Frac,
    request: Request,
    result: &mut PerturbationResult,
) {
    // `split(sigma_V, "candidate", c, "perturbation", <kind name>, j)`, so the
    // insertion is the one part `"perturbation"` and the kind is the sample
    // stream's own name.
    let extra = [Part::Name(PERTURBATION)];
    // The edit, and the parameter it resolved to. A kind that cannot name one
    // records its reason and replays nothing at all.
    let prepared = match request.kind {
        BOUNDARY_SEVERANCE => {
            // Every crossing Route of the inside, removed for the whole replay.
            // Exogenous events are untouched. Parameters: none.
            Some(Prepared::without(window, sets, &sets.severed))
        }
        ROUTE_REMOVAL => match resolve_route(window, structure, sets, request.parameter) {
            Some(route) => {
                result.parameter = Some(i64::from(route));
                Some(Prepared::without(window, sets, &[route]))
            }
            None => {
                result.reading = PrivilegeValue::unassigned(NO_ROUTE);
                None
            }
        },
        COMPONENT_SUBSTITUTION => {
            match resolve_member(window, structure, sets, request.parameter) {
                Some(node) => {
                    result.parameter = Some(i64::from(node));
                    Some(Prepared::substituting(window, sets, &[node]))
                }
                None => {
                    result.reading = PrivilegeValue::unassigned(NO_MEMBER);
                    None
                }
            }
        }
        FULL_TURNOVER => {
            // Every member at once, each as in Component substitution.
            // Parameters: none.
            Some(Prepared::substituting(window, sets, &sets.members.clone()))
        }
        _ => None,
    };
    let Some(prepared) = prepared else { return };

    let recorded = Recorded::of(window, structure, sets, tau);
    // The shared baselines the excesses are taken against. Their replays run
    // here, so their streams are streams this result used, and FRAMEWORK.md's
    // Records paragraph asks for every full stream name used — the eight
    // baseline names are recorded first, then the kind's own eight.
    let (baseline, _) = baselines(window, structure, sets, sigma, position, &[], tau);
    for sample in 1..=SAMPLES {
        result.streams.push(stream_name(position, &[], rank::STREAM_BASELINE, sample));
    }
    let mut excesses: Vec<Frac> = Vec::new();
    for sample in 1..=SAMPLES {
        let stream = sample_stream_under(sigma, position, &extra, request.kind, sample);
        result.streams.push(stream_name(position, &extra, request.kind, sample));
        let series = replay_series(window, &prepared, stream, |_, _, _| {});
        let deviation = recorded.deviation(&series, tau);
        let excess = excess_of(deviation, baseline[sample - 1]);
        if let Some(held) = excess {
            excesses.push(held);
        }
        result.samples.push(Sample {
            deviation,
            excess,
            series,
            base_deviation: None,
            base_series: None,
        });
    }
    result.reading = reading_of(excesses);
}

/// The excess of one edited sample against its base: `clamp01(dev_edit -
/// dev_base)`, and unassigned when either deviation is.
fn excess_of(edited: Option<Frac>, base: Option<Frac>) -> Option<Frac> {
    match (edited, base) {
        (Some(edited), Some(base)) => Some((edited - base).clamp(0, FRAC_ONE)),
        _ => None,
    }
}

/// The median excess and its [smallest, largest] confidence range, or the
/// unassigned reading a window too short to compare gives.
fn reading_of(mut excesses: Vec<Frac>) -> PrivilegeValue {
    if excesses.is_empty() {
        return PrivilegeValue::unassigned(WINDOW_TOO_SHORT);
    }
    let count = excesses.len() as u16;
    excesses.sort_unstable();
    let low = excesses[0];
    let high = excesses[excesses.len() - 1];
    PrivilegeValue::assigned(median(&excesses), low, high, count)
}

/// Route removal's parameter, resolved.
///
/// FRAMEWORK.md's default, in order: the cyclic Route with tail or head in the
/// inside carrying the largest window flow, smallest identifier on equal flows;
/// when no cyclic Route has an end in the inside, the Route with an end in the
/// inside carrying the largest positive window flow, smallest identifier on
/// equal flows; and when no Route with an end in the inside has positive window
/// flow, no Route at all — the kind's readings are unassigned with that reason.
fn resolve_route(
    window: &Window<'_>,
    structure: &Structure,
    sets: &Sets,
    asked: Option<u32>,
) -> Option<u32> {
    if let Some(route) = asked {
        return structure
            .routes
            .iter()
            .find(|(held, _, _)| *held == route)
            .map(|(held, _, _)| *held);
    }
    let mut flows = vec![0 as Fx; structure.routes.len()];
    for step in &window.steps {
        rank::add_flows(&step.records.f, structure, &mut flows);
    }
    let present = vec![true; structure.routes.len()];
    let cyclic = cyclic_routes(structure, &flows, &present);
    let touching = |tail: usize, head: usize| sets.member[tail] || sets.member[head];
    let mut best: Option<(Fx, u32)> = None;
    for (place, (route, tail, head)) in structure.routes.iter().enumerate() {
        if !cyclic[place] || !touching(*tail, *head) {
            continue;
        }
        let flow = flows[place];
        if best.is_none_or(|(held, id)| flow > held || (flow == held && *route < id)) {
            best = Some((flow, *route));
        }
    }
    if let Some((_, route)) = best {
        return Some(route);
    }
    for (place, (route, tail, head)) in structure.routes.iter().enumerate() {
        if flows[place] <= 0 || !touching(*tail, *head) {
            continue;
        }
        let flow = flows[place];
        if best.is_none_or(|(held, id)| flow > held || (flow == held && *route < id)) {
            best = Some((flow, *route));
        }
    }
    best.map(|(_, route)| route)
}

/// Which Routes are cyclic on the window's flow-positive graph — Cut Impact's
/// own rule, read here for Route removal's default and for Reach.
pub(crate) fn cyclic_routes(
    structure: &Structure,
    flows: &[Fx],
    present: &[bool],
) -> Vec<bool> {
    let (component, cyclic) = flow_components(structure, flows, present);
    structure
        .routes
        .iter()
        .enumerate()
        .map(|(place, (_, tail, head))| {
            present[place]
                && flows[place] > 0
                && component[*tail] == component[*head]
                && cyclic[component[*tail]]
        })
        .collect()
}

/// The strongly connected components of the flow-positive graph, and which of
/// them contain a cycle — two or more Nodes, or a Route from a Node to itself.
pub(crate) fn flow_components(
    structure: &Structure,
    flows: &[Fx],
    present: &[bool],
) -> (Vec<usize>, Vec<bool>) {
    let count = structure.count();
    let mut near: Vec<Vec<usize>> = vec![Vec::new(); count];
    let mut loops = vec![false; count];
    for (place, (_, tail, head)) in structure.routes.iter().enumerate() {
        if !present[place] || flows[place] <= 0 {
            continue;
        }
        near[*tail].push(*head);
        if tail == head {
            loops[*tail] = true;
        }
    }
    let (component, found) = rank::components(count, &near);
    let mut size = vec![0usize; found];
    let mut cyclic = vec![false; found];
    for place in 0..count {
        if component[place] < found {
            size[component[place]] += 1;
            if loops[place] {
                cyclic[component[place]] = true;
            }
        }
    }
    for held in 0..found {
        if size[held] >= 2 {
            cyclic[held] = true;
        }
    }
    (component, cyclic)
}

/// Component substitution's parameter, resolved: the member with the largest
/// mean stored Charge over the window, smallest identifier on equal means.
///
/// The mean is compared as its own total, because every member's total is taken
/// over the same window: dividing each by the same step count would not move
/// one past another, and comparing totals keeps the comparison exact.
pub(crate) fn resolve_member(
    window: &Window<'_>,
    structure: &Structure,
    sets: &Sets,
    asked: Option<u32>,
) -> Option<u32> {
    if let Some(node) = asked {
        return sets.members.iter().copied().find(|held| *held == node);
    }
    let count = structure.count();
    let mut totals = vec![0i128; count];
    for step in &window.steps {
        rank::walk(&step.records.q, &structure.nodes, |place, value| {
            totals[place] += i128::from(value);
        });
    }
    let mut best: Option<(i128, u32)> = None;
    for place in 0..count {
        if !sets.member[place] {
            continue;
        }
        let node = structure.nodes[place];
        let total = totals[place];
        if best.is_none_or(|(held, id)| total > held || (total == held && node < id)) {
            best = Some((total, node));
        }
    }
    best.map(|(_, node)| node)
}

// ---------------------------------------------------------------------------
// Resolution change
// ---------------------------------------------------------------------------

/// **Resolution change** — no replay. The recorded window is re-read at the
/// nearby grains: Scale Stability's qualifying grain pairs and pair values, and
/// the block series at each grain. The reading is `1 - mean(pair values)`, its
/// confidence range the window-split range of that reading, sample count 1, and
/// it is unassigned exactly when Scale Stability is unassigned.
fn resolution_change(
    window: &Window<'_>,
    structure: &Structure,
    sets: &Sets,
    view: &ViewDeclaration,
    tau: Frac,
    result: &mut PerturbationResult,
) {
    if window.len() < 2 {
        result.reading = PrivilegeValue::unassigned(WINDOW_TOO_SHORT);
        return;
    }
    let pairs = rank::grain_pairs(view.resolution, sets.members.len());
    if pairs.is_empty() {
        result.reading = PrivilegeValue::unassigned(crate::slate::NO_GRAIN_PAIR);
        return;
    }
    let places: Vec<usize> = (0..structure.count()).filter(|place| sets.member[*place]).collect();
    let order = rank::proximity_order(structure, &places);
    let mut q = vec![vec![0 as Fx; window.len()]; order.len()];
    let mut slot_of = vec![usize::MAX; structure.count()];
    for (slot, place) in order.iter().enumerate() {
        slot_of[*place] = slot;
    }
    for (step, recorded) in window.steps.iter().enumerate() {
        rank::walk(&recorded.records.q, &structure.nodes, |place, value| {
            if slot_of[place] != usize::MAX {
                q[slot_of[place]][step] = value;
            }
        });
    }
    let slots: Vec<usize> = (0..order.len()).collect();
    let stability = |from: usize, to: usize| {
        rank::scale_stability_over(&slots, &q, &pairs, tau, from, to)
    };
    let Some(value) = stability(0, window.len()) else {
        result.reading = PrivilegeValue::unassigned(WINDOW_TOO_SHORT);
        return;
    };
    // The reading is one less the mean of the qualifying pair values: a pattern
    // that reads the same at the nearby grains is one a resolution change moves
    // least, so the reading rises exactly as the stability falls.
    let reading = FRAC_ONE - value;
    let half = window.len() / 2;
    let split = if window.len() >= 4 {
        match (stability(0, half), stability(half, window.len())) {
            (Some(first), Some(second)) => Some((FRAC_ONE - first, FRAC_ONE - second)),
            _ => None,
        }
    } else {
        None
    };
    result.reading = match split {
        Some((first, second)) => PrivilegeValue::assigned(
            reading,
            reading.min(first).min(second),
            reading.max(first).max(second),
            1,
        ),
        None => PrivilegeValue::assigned(reading, reading, reading, 1),
    };
    // The grains used, the pair values keyed by their smaller grain, and the
    // block series at each grain — the record the document locks for this kind.
    let mut grains: Vec<u16> = Vec::new();
    for (small, large) in &pairs {
        for grain in [*small, *large] {
            if !grains.contains(&grain) {
                grains.push(grain);
            }
        }
    }
    grains.sort_unstable();
    let mut values: Vec<(u16, Frac)> = Vec::new();
    for (small, large) in &pairs {
        let blocks = rank::block_series(&slots, &q, usize::from(*small), 0, window.len());
        let parents = rank::block_series(&slots, &q, usize::from(*large), 0, window.len());
        let mut total: i128 = 0;
        for (index, block) in blocks.iter().enumerate() {
            let parent = &parents[index / 2];
            let held = agreement(&symbols(block, tau), &symbols(parent, tau))
                .expect("a window of two or more steps has a step to compare");
            total += i128::from(held);
        }
        values.push((
            *small,
            frac_of(total, blocks.len() as i128 * i128::from(FRAC_ONE)),
        ));
    }
    let blocks: Vec<(u16, Vec<Vec<Fx>>)> = grains
        .iter()
        .map(|grain| {
            (*grain, rank::block_series(&slots, &q, usize::from(*grain), 0, window.len()))
        })
        .collect();
    result.recomputed = Some(Recomputed::Resolution { grains, pairs: values, blocks });
}

// ---------------------------------------------------------------------------
// Window change
// ---------------------------------------------------------------------------

/// **Window change** — the four privilege values, recomputed at `w' = ceil(w /
/// 2)` and, when the trajectory holds at least `2 * w` completed steps, at
/// `w' = 2 * w`.
///
/// Every stream inside such a recomputation inserts `"perturbation", "window",
/// w'` directly after its candidate parts, which is what
/// [`sample_stream_under`] is for. The deviation of one recomputed window is
/// the largest movement over the values assigned at both windows; the reading
/// is the largest of those, and the confidence range is [smallest, largest]
/// over them — a point range when only one window was recomputed.
#[allow(clippy::too_many_arguments)]
fn window_change(
    state: &RunState,
    structure: &Structure,
    view: &ViewDeclaration,
    sigma: &RngState,
    position: u8,
    tau: Frac,
    effective: u16,
    result: &mut PerturbationResult,
) {
    let base_window = Window::of(state, effective);
    let sets = Sets::of(structure, view);
    let declared = rank::profile(&base_window, structure, &sets, view, sigma, position, &[], tau);

    let mut widths: Vec<u16> = Vec::new();
    let half = effective.div_ceil(2);
    if half >= 1 && half != effective {
        widths.push(half);
    }
    // The doubled window is recomputed only when the trajectory actually holds
    // `2 * w` completed steps; the clamp is what decides that, and asking for
    // more than it leaves would recompute the same window under a different
    // name.
    let doubled = effective.saturating_mul(2);
    if doubled > effective && state.effective_window(doubled) == doubled {
        widths.push(doubled);
    }

    let mut deviations: Vec<Frac> = Vec::new();
    let mut windows: Vec<(u16, PrivilegeProfile, Frac)> = Vec::new();
    for width in widths {
        let extra = [
            Part::Name(PERTURBATION),
            Part::Name("window"),
            Part::Number(i64::from(width)),
        ];
        for name in [
            rank::STREAM_SHARED_FAILURE,
            rank::STREAM_CUT_IMPACT,
            rank::STREAM_BOUNDARY_SUFFICIENCY,
        ] {
            for sample in 1..=SAMPLES {
                result.streams.push(stream_name(position, &extra, name, sample));
            }
        }
        let held = Window::of(state, width);
        let profile =
            rank::profile(&held, structure, &sets, view, sigma, position, &extra, tau);
        let mut largest: Option<Frac> = None;
        for (at, other) in declared.each().into_iter().zip(profile.each()) {
            let (Some(first), Some(second)) = (at.value, other.value) else { continue };
            let moved = (second - first).abs();
            largest = Some(largest.map_or(moved, |held| held.max(moved)));
        }
        if let Some(moved) = largest {
            deviations.push(moved);
        }
        windows.push((width, profile, largest.unwrap_or(0)));
    }
    result.recomputed = Some(Recomputed::Window { windows });
    if deviations.is_empty() {
        result.reading = PrivilegeValue::unassigned(NO_SHARED_WINDOW);
        return;
    }
    let low = *deviations.iter().min().expect("a nonempty list has a smallest");
    let high = *deviations.iter().max().expect("a nonempty list has a largest");
    result.reading = PrivilegeValue::assigned(high, low, high, deviations.len() as u16);
}

// ---------------------------------------------------------------------------
// Surround change
// ---------------------------------------------------------------------------

/// **Surround change** — the View's surround rule moves one place along the
/// cycle `adjacent` to `double` to `whole` to `adjacent`, and Shared Failure is
/// recomputed under the new rule.
///
/// The reading is the magnitude of the change when both values are assigned, 1
/// when exactly one is, and 0 when neither is; its confidence range is the point
/// range at the reading, and both values' own ranges are recorded beside it.
#[allow(clippy::too_many_arguments)]
fn surround_change(
    window: &Window<'_>,
    structure: &Structure,
    sets: &Sets,
    view: &ViewDeclaration,
    sigma: &RngState,
    position: u8,
    tau: Frac,
    result: &mut PerturbationResult,
) {
    let rule = match view.surround {
        Surround::Adjacent => Surround::Double,
        Surround::Double => Surround::Whole,
        Surround::Whole => Surround::Adjacent,
    };
    let old = rank::shared_failure(window, structure, sets, sigma, position, &[], tau);
    let shifted = ViewDeclaration { surround: rule, ..view.clone() };
    let other = Sets::of(structure, &shifted);
    let extra =
        [Part::Name(PERTURBATION), Part::Name("surround"), Part::Name(rule.name())];
    for sample in 1..=SAMPLES {
        result.streams.push(stream_name(
            position,
            &extra,
            rank::STREAM_SHARED_FAILURE,
            sample,
        ));
    }
    let new = rank::shared_failure(window, structure, &other, sigma, position, &extra, tau);
    let reading = match (old.value, new.value) {
        (Some(first), Some(second)) => (second - first).abs(),
        (Some(_), None) | (None, Some(_)) => FRAC_ONE,
        (None, None) => 0,
    };
    result.reading = PrivilegeValue::assigned(reading, reading, reading, 1);
    result.recomputed = Some(Recomputed::Surround { rule, old, new });
}

// ---------------------------------------------------------------------------
// Delayed replay
// ---------------------------------------------------------------------------

/// **Delayed replay** — the one kind that pins the exogenous schedule instead
/// of letting it re-draw, and the one kind that carries two series per sample.
///
/// Both replays drive every exogenous event from the recorded window's
/// schedule: the base at the step it was recorded at, and the shift `s` steps
/// later, with events pushed past the window's end dropped. **The control
/// schedule never shifts** — that is locked, and it is why the shift is applied
/// to the recorded exogenous term alone and to nothing the step consumed.
///
/// The excess of sample j is `clamp01(deviation of shifted j - deviation of
/// baseline j)`, both against the recorded window, so this kind's base is its
/// own unshifted replay rather than the shared baseline.
#[allow(clippy::too_many_arguments)]
fn delayed_replay(
    window: &Window<'_>,
    structure: &Structure,
    sets: &Sets,
    sigma: &RngState,
    position: u8,
    tau: Frac,
    asked: Option<u32>,
    result: &mut PerturbationResult,
) {
    // The delay in steps. Default: `s = ceil(w / 4)`.
    let delay = usize::from(asked.map_or_else(
        || (window.len() as u16).div_ceil(4),
        |held| held.min(u32::from(u16::MAX)) as u16,
    ));
    result.parameter = Some(delay as i64);
    let extra = [Part::Name(PERTURBATION)];
    let recorded = Recorded::of(window, structure, sets, tau);
    // The recorded exogenous term of every Node, per window step: the schedule
    // both replays drive from.
    let schedule: Vec<Vec<(u32, Fx)>> =
        window.steps.iter().map(|step| step.records.e.clone()).collect();
    let prepared = Prepared::plain(window, sets);
    let mut excesses: Vec<Frac> = Vec::new();
    for sample in 1..=SAMPLES {
        let base_stream = sigma.split(&[
            Part::Name("candidate"),
            Part::Number(i64::from(position)),
            Part::Name(PERTURBATION),
            Part::Name(DELAYED_REPLAY),
            Part::Number(sample as i64),
            Part::Name("base"),
        ]);
        let shift_stream = sigma.split(&[
            Part::Name("candidate"),
            Part::Number(i64::from(position)),
            Part::Name(PERTURBATION),
            Part::Name(DELAYED_REPLAY),
            Part::Number(sample as i64),
            Part::Name("shift"),
        ]);
        result.streams.push(format!(
            "{}/base",
            stream_name(position, &extra, DELAYED_REPLAY, sample)
        ));
        result.streams.push(format!(
            "{}/shift",
            stream_name(position, &extra, DELAYED_REPLAY, sample)
        ));
        let base_series =
            replay_series(window, &prepared, base_stream, |index, state, records| {
                drive_schedule(state, records, &schedule, index, 0);
            });
        let series =
            replay_series(window, &prepared, shift_stream, |index, state, records| {
                drive_schedule(state, records, &schedule, index, delay);
            });
        let base_deviation = recorded.deviation(&base_series, tau);
        let deviation = recorded.deviation(&series, tau);
        let excess = excess_of(deviation, base_deviation);
        if let Some(held) = excess {
            excesses.push(held);
        }
        result.samples.push(Sample {
            deviation,
            excess,
            series,
            base_deviation,
            base_series: Some(base_series),
        });
    }
    result.reading = reading_of(excesses);
}

/// Puts the recorded exogenous schedule back onto a Field after one replayed
/// step: the term the step computed for itself is taken off, and the term
/// recorded `delay` steps earlier is put in its place.
///
/// A correction rather than a substitution is what the Field admits: the step
/// function has already run and written its own term, so the recorded term is
/// reached by moving the difference. An unshifted drive takes the same term off
/// and puts the same term back, so a Field whose step reproduces its recording
/// is left exactly where it stood — which is what makes the base replay a base
/// rather than a second edit.
///
/// Events shifted past the window's end are dropped, because the replay runs
/// exactly `w` steps and never reads a schedule entry past the last one, and a
/// step with no recorded term behind it takes none at all.
fn drive_schedule(
    state: &mut FieldState,
    records: &crate::field::StepRecords,
    schedule: &[Vec<(u32, Fx)>],
    index: usize,
    delay: usize,
) {
    for (node, value) in &records.e {
        move_stored(state, *node, -*value);
    }
    if let Some(entries) = index.checked_sub(delay).and_then(|held| schedule.get(held)) {
        for (node, value) in entries {
            move_stored(state, *node, *value);
        }
    }
}
