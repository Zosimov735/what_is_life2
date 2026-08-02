//! The four privilege values, and the ranking they produce without ever being
//! combined.
//!
//! `docs/field-framework/FRAMEWORK.md`'s Privilege profile section is the whole
//! of what this module does, carried out exactly as it is written: Scale
//! Stability, Shared Failure, Cut Impact, and Boundary Sufficiency, each
//! normalized to [0, 1] and each carrying a confidence range; the comparison
//! that reads those ranges and never the bare numbers; dominance,
//! nondominance, ties, and incomparability; the iterated tiers; and the
//! tolerance sensitivity that recomputes the comparison alone.
//! `docs/field-framework/ARCHITECTURE.md` locks the serialized shape the
//! results are written into and the analysis budget they are computed inside.
//!
//! **The four are never summed, averaged, weighted, or otherwise combined.**
//! There is no scalar anywhere in this module: a candidate carries four values
//! and a tier, the tier is the rank of a set rather than a figure derived from
//! the values, and nothing here produces a number that stands for a candidate
//! as a whole.
//!
//! Three properties are load-bearing:
//!
//! - **Windowed.** Every value runs under the effective window
//!   `w_eff = min(w, t0, retained_span)` — FRAMEWORK.md's clamp with the
//!   retained-span term ARCHITECTURE.md adds — so right after an applying
//!   commit there is nothing to observe and every value is honestly
//!   unassigned. The span regrows a step at a time and the values return.
//! - **Replayed under the recorded schedule.** Every windowed replay starts
//!   from the window's start state and drives control from the recorded `ctl`
//!   of each step, which is the locked replay-time input policy. A replay never
//!   touches the recorded trajectory.
//! - **Integer.** No float appears here. Fractions are raw `Frac` — the
//!   quantity times 65536 — and every ratio goes through [`frac_of`], which
//!   rounds to nearest with halves upward on `i128` intermediates. Identical
//!   inputs therefore produce an identical record, byte for byte.
//!
//! One rule of version 1 draws: the Noise flow scale, at its locked drawing
//! point in the step function. Every sample below names its stream as the
//! locked split of `sigma_V` — [`sample_stream`] is that split — and passes
//! it through the replay boundary into the step function, which carries it
//! beside the staged pressures as [`field::Staging`]. Where effective noise
//! stands, each sample's replay re-draws the flow scales from its own stream
//! — the locked forecast distortion, one rule seen through the replay
//! carriage — so the eight samples of a procedure genuinely disagree and the
//! confidence ranges are real spreads. Where no noise stands, nothing draws,
//! every sample agrees, and the ranges are points, exactly as before.

use crate::field::{self, StepCache, StepRecords};
use crate::fx::fixed_mul;
use crate::rng::{Part, RngState};
use crate::slate::{
    CandidateSlate, PrivilegeProfile, PrivilegeValue, BASELINE_SAMPLES, FEW_MEMBERS, FEW_SAMPLES,
    FEW_SURROUND, NO_CIRCULATING_FLOW, NO_GRAIN_PAIR, SENSITIVITY_DOUBLE, SENSITIVITY_HALF,
    WINDOW_TOO_SHORT,
};
use crate::state::{
    FieldState, Frac, Fx, RunState, Surround, TraceStep, ViewDeclaration, FRAC_ONE,
};

/// The resolution ladder every grain stands on.
const LADDER: [u16; 9] = [1, 2, 4, 8, 16, 32, 64, 128, 256];

/// How many samples each stochastic procedure takes.
pub(crate) const SAMPLES: usize = 8;

/// The fewest defined samples a median may be taken over.
const DEFINED_MINIMUM: usize = 4;

/// The stream names FRAMEWORK.md gives the sample streams of this section.
pub const STREAM_BASELINE: &str = "baseline";
pub const STREAM_SHARED_FAILURE: &str = "shared-failure";
pub const STREAM_CUT_IMPACT: &str = "cut-impact";
pub const STREAM_BOUNDARY_SUFFICIENCY: &str = "boundary-sufficiency";

/// One sample's stream: `split(sigma_V, "candidate", c, <name>, j)`, exactly as
/// FRAMEWORK.md names it, with `c` the candidate's assembly position (1-based)
/// and `j` the sample number from 1.
pub fn sample_stream(sigma: &RngState, position: u8, name: &str, sample: usize) -> RngState {
    sample_stream_under(sigma, position, &[], name, sample)
}

/// The same stream with parts inserted directly after the candidate parts.
///
/// Two perturbation kinds recompute a privilege value under a changed
/// declaration and FRAMEWORK.md names their streams by insertion rather than by
/// replacement: a window change inserts `"perturbation", "window", w'` and a
/// surround change inserts `"perturbation", "surround", <new rule>`, each
/// directly after `"candidate", c` and before the value's own name and sample
/// number. `extra` is that insertion, empty for every ordinary sample, so one
/// function names every stream this crate ever splits for a sample.
pub fn sample_stream_under(
    sigma: &RngState,
    position: u8,
    extra: &[Part<'_>],
    name: &str,
    sample: usize,
) -> RngState {
    let mut parts: Vec<Part<'_>> =
        Vec::with_capacity(4 + extra.len());
    parts.push(Part::Name("candidate"));
    parts.push(Part::Number(i64::from(position)));
    parts.extend_from_slice(extra);
    parts.push(Part::Name(name));
    parts.push(Part::Number(sample as i64));
    sigma.split(&parts)
}

/// A full stream name as a record stores it: the split parts joined with `/`,
/// integers in decimal — ARCHITECTURE.md's serialization of `streams`.
pub fn stream_name(position: u8, extra: &[Part<'_>], name: &str, sample: usize) -> String {
    let mut out = format!("candidate/{position}");
    for part in extra {
        match part {
            Part::Name(held) => {
                out.push('/');
                out.push_str(held);
            }
            Part::Number(held) => {
                out.push('/');
                out.push_str(&held.to_string());
            }
        }
    }
    out.push('/');
    out.push_str(name);
    out.push('/');
    out.push_str(&sample.to_string());
    out
}

/// A ratio as a `Frac`: `num / den` times 65536, rounded to nearest with halves
/// upward, held inside [0, 1].
///
/// One rounding rule, in one place, is what makes every value below
/// hand-checkable: a fraction `a / b` reads as `round(a * 65536 / b)`.
pub(crate) fn frac_of(num: i128, den: i128) -> Frac {
    debug_assert!(den > 0 && num >= 0, "a normalized reading is a nonnegative part of a whole");
    let scaled = num * i128::from(FRAC_ONE);
    let rounded = (scaled * 2 + den) / (den * 2);
    (rounded as Frac).clamp(0, FRAC_ONE)
}

/// The median of a list of `Frac`s: the middle one of an odd count, the mean of
/// the two middle ones of an even count. The caller sorts.
pub(crate) fn median(sorted: &[Frac]) -> Frac {
    let count = sorted.len();
    debug_assert!(count > 0, "a median is taken over at least one value");
    if count % 2 == 1 {
        sorted[count / 2]
    } else {
        // The mean of two, rounded to nearest with halves upward, which is the
        // one rounding rule this module uses everywhere.
        (sorted[count / 2 - 1] + sorted[count / 2] + 1) / 2
    }
}

/// The value, and the confidence range, a list of defined samples gives:
/// the median, and [smallest, largest].
pub(crate) fn from_samples(mut defined: Vec<Frac>) -> PrivilegeValue {
    if defined.len() < DEFINED_MINIMUM {
        return PrivilegeValue::unassigned(FEW_SAMPLES);
    }
    let count = defined.len() as u16;
    defined.sort_unstable();
    let low = defined[0];
    let high = defined[defined.len() - 1];
    PrivilegeValue::assigned(median(&defined), low, high, count)
}

// ---------------------------------------------------------------------------
// The Field, read as the framework's relations
// ---------------------------------------------------------------------------

/// The relations of the Field at the evaluation step, computed once and read by
/// every candidate.
///
/// The surround set U(V) is "the named set computed at the evaluation step from
/// the current Route set R and the declared adjacency", and the crossing set,
/// the shell, and the interior are read from the same two relations at the same
/// instant. So they are taken once, here, and the same sets are used for the
/// recorded window and for every replayed one — a replay is compared with the
/// record like for like, which it could not be if the sets moved under it.
pub(crate) struct Structure {
    /// The Node set N, ascending: the Port list's own order.
    pub(crate) nodes: Vec<u32>,
    /// The declared adjacency, as a square table over Node places.
    pub(crate) adjacency: Vec<bool>,
    /// Whether a Route stands between two Node places, in either direction.
    pub(crate) links: Vec<bool>,
    /// Every Route as (identifier, tail place, head place), in Route order.
    pub(crate) routes: Vec<(u32, usize, usize)>,
    /// Each Node's layer and position, for the proximity order's distances.
    pub(crate) placed: Vec<(crate::fx::Vec2, u8)>,
}

impl Structure {
    pub(crate) fn of(field: &FieldState) -> Self {
        let nodes: Vec<u32> = field.ports.iter().map(|port| port.node).collect();
        let placed: Vec<(crate::fx::Vec2, u8)> =
            field.ports.iter().map(|port| (port.pos, port.layer)).collect();
        let count = nodes.len();
        let mut adjacency = vec![false; count * count];
        for first in 0..count {
            for second in (first + 1)..count {
                if crate::fx::adjacent(placed[first].0, placed[first].1, placed[second].0, placed[second].1)
                {
                    adjacency[first * count + second] = true;
                    adjacency[second * count + first] = true;
                }
            }
        }
        let mut links = vec![false; count * count];
        let mut routes = Vec::with_capacity(field.routes.len());
        for route in &field.routes {
            let (Ok(tail), Ok(head)) =
                (nodes.binary_search(&route.tail), nodes.binary_search(&route.head))
            else {
                continue;
            };
            links[tail * count + head] = true;
            links[head * count + tail] = true;
            routes.push((route.route, tail, head));
        }
        Structure { nodes, adjacency, links, routes, placed }
    }

    pub(crate) fn count(&self) -> usize {
        self.nodes.len()
    }

    /// Whether a Node has a Route to or from a marked set, or a declared
    /// adjacency to it.
    pub(crate) fn touches(&self, place: usize, marked: &[bool]) -> bool {
        let count = self.count();
        (0..count).any(|other| {
            marked[other]
                && other != place
                && (self.adjacency[place * count + other] || self.links[place * count + other])
        })
    }
}

/// One candidate's sets, read off the Structure at the evaluation step.
pub(crate) struct Sets {
    /// Per Node place: a member of the inside.
    pub(crate) member: Vec<bool>,
    /// Per Node place: in the surround set U(V).
    pub(crate) surround: Vec<bool>,
    /// Per Node place: a member with a crossing Route or a declared adjacency
    /// to a non-member.
    pub(crate) shell: Vec<bool>,
    /// Per Route place: exactly one endpoint in the inside.
    pub(crate) crossing: Vec<bool>,
    /// The member Node identifiers, ascending.
    pub(crate) members: Vec<u32>,
    /// The identifiers of `I union U`, ascending: what the Shared Failure probe
    /// takes its fraction from.
    pub(crate) probed: Vec<u32>,
    /// The Route identifiers of the crossing set, ascending: what a severance
    /// removes.
    pub(crate) severed: Vec<u32>,
}

impl Sets {
    pub(crate) fn of(structure: &Structure, view: &ViewDeclaration) -> Self {
        let count = structure.count();
        let mut member = vec![false; count];
        for node in &view.inside {
            if let Ok(place) = structure.nodes.binary_search(node) {
                member[place] = true;
            }
        }
        let surround: Vec<bool> = match view.surround {
            Surround::Whole => (0..count).map(|place| !member[place]).collect(),
            Surround::Adjacent => (0..count)
                .map(|place| !member[place] && structure.touches(place, &member))
                .collect(),
            Surround::Double => {
                let near: Vec<bool> = (0..count)
                    .map(|place| !member[place] && structure.touches(place, &member))
                    .collect();
                (0..count)
                    .map(|place| {
                        !member[place] && (near[place] || structure.touches(place, &near))
                    })
                    .collect()
            }
        };
        let shell: Vec<bool> = (0..count)
            .map(|place| {
                member[place]
                    && (0..count).any(|other| {
                        !member[other]
                            && (structure.adjacency[place * count + other]
                                || structure.links[place * count + other])
                    })
            })
            .collect();
        let crossing: Vec<bool> = structure
            .routes
            .iter()
            .map(|(_, tail, head)| member[*tail] != member[*head])
            .collect();
        let members: Vec<u32> =
            (0..count).filter(|place| member[*place]).map(|place| structure.nodes[place]).collect();
        let probed: Vec<u32> = (0..count)
            .filter(|place| member[*place] || surround[*place])
            .map(|place| structure.nodes[place])
            .collect();
        let severed: Vec<u32> = structure
            .routes
            .iter()
            .zip(crossing.iter())
            .filter(|(_, held)| **held)
            .map(|((route, _, _), _)| *route)
            .collect();
        Sets { member, surround, shell, crossing, members, probed, severed }
    }

    pub(crate) fn surround_count(&self) -> usize {
        self.surround.iter().filter(|held| **held).count()
    }
}

// ---------------------------------------------------------------------------
// The window, and the replays over it
// ---------------------------------------------------------------------------

/// Everything one evaluation reads of the Run: the window's recorded steps,
/// the state the window starts at, and the remaining step inputs a replay
/// carries. Physical membership is already part of that Field state.
pub(crate) struct Window<'a> {
    /// The window's recorded steps, oldest first.
    pub(crate) steps: Vec<&'a TraceStep>,
    /// The Field as of step `t0 - w_eff`, regenerated from the trajectory's
    /// keyframe under the recorded control schedule.
    pub(crate) start: FieldState,
    pub(crate) pointer_speed: Frac,
    /// The prepared step pass for the window's own shape, built once and read
    /// by every replay that does not change that shape — which is every one
    /// but a severance. A probe moves stored Charge and nothing structural, so
    /// the same preparation stands for it; the eight samples of one procedure
    /// and the four procedures alike share this.
    pub(crate) cache: StepCache,
    /// The staged pressures the recorded steps ran under. Membership is
    /// immutable inside a window — every rule that changes it ends the window —
    /// so the live list's membership is the membership of every recorded step,
    /// and a replay hands each step a scratch copy whose `stage` and `level`
    /// the stage machine re-derives at that step.
    pub(crate) pressures: Vec<crate::pressure::PressureState>,
    /// The authored tables the stage machine reads: content, identical for the
    /// live step and every replay of it.
    pub(crate) schedule: crate::pressure::Schedule,
}

impl<'a> Window<'a> {
    pub(crate) fn of(state: &'a RunState, effective: u16) -> Self {
        let held: Vec<&TraceStep> = state.trace.steps.iter().collect();
        let start_at = held.len().saturating_sub(usize::from(effective));
        let pointer_speed = state.input_config.pointer_speed;
        let mut start = state.trace.keyframe.clone();
        let cache = StepCache::of(&start);
        let mut scratch = state.pressures.clone();
        for recorded in &held[..start_at] {
            let mut stream = recorded.rng;
            field::advance_cached(
                &mut start,
                recorded.ctl,
                pointer_speed,
                &mut field::Staging {
                    pressures: &mut scratch,
                    schedule: &state.schedule,
                    stream: &mut stream,
                },
                &cache,
            );
        }
        let cache = StepCache::of(&start);
        Window {
            steps: held[start_at..].to_vec(),
            start,
            pointer_speed,
            cache,
            pressures: state.pressures.clone(),
            schedule: state.schedule.clone(),
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.steps.len()
    }
}

/// What a replay applies before its first step.
pub(crate) enum Edit<'a> {
    /// No edit: the baseline replays and Boundary Sufficiency's samples.
    None,
    /// Shared Failure's probe: this fraction of every named Node's stored
    /// Charge, removed.
    Probe { fraction: Frac, nodes: &'a [u32] },
    /// Cut Impact's severance: these Routes removed for the whole replay.
    Sever { routes: &'a [u32] },
}

/// Replays the window from its start state under an edit, handing each step's
/// records to a watcher.
///
/// The control comes from the recorded schedule and physical membership from
/// the replayed Field. Under a Field with no effective noise a replay with no edit
/// reproduces the recorded window exactly; where noise stands, the sample's
/// own stream re-draws the flow scales — the locked forecast distortion, one
/// rule seen through the replay carriage — so baselines legitimately spread
/// and the confidence ranges widen. Either way every sample of one procedure
/// runs under its own named stream, and a deviation reads the edit plus the
/// distortion the drawn scales carry, exactly as the document states it.
///
/// `stream` is the sample's own stream, named by [`sample_stream`], and since
/// Goal 18 it is passed through to the step function rather than discarded at
/// this boundary: each replayed step reads the sample's stream at the position
/// the previous replayed step left. No rule of version 1 draws yet, so the
/// position does not move and the eight samples of one procedure still agree —
/// the honest reading of a Field with no stochastic rule — but the parameter
/// is threaded, and the rule that draws lands with no change of shape here.
pub(crate) fn replay(
    window: &Window<'_>,
    edit: &Edit<'_>,
    cache: &StepCache,
    stream: RngState,
    mut watch: impl FnMut(usize, &StepRecords),
) {
    let mut stream = stream;
    let mut scratch = window.pressures.clone();
    let mut state = window.start.clone();
    match edit {
        Edit::None => {}
        Edit::Probe { fraction, nodes } => {
            for port in state.ports.iter_mut() {
                if nodes.binary_search(&port.node).is_ok() {
                    port.q -= fixed_mul(port.q, *fraction);
                }
            }
            // A Form and its Node are one placed thing, and the Node's stored
            // Charge is what the Form reads as its own.
            let stored: Vec<(u32, Fx)> =
                state.ports.iter().map(|port| (port.node, port.q)).collect();
            for form in state.forms.iter_mut() {
                if let Some((_, q)) = stored.iter().find(|(node, _)| *node == form.node) {
                    form.charge = *q;
                }
            }
        }
        Edit::Sever { routes } => {
            // The severed list is ascending — it was read off the Route list in
            // the list's own order — so each Route asks one binary search.
            state.routes.retain(|held| routes.binary_search(&held.route).is_err());
        }
    }
    for (index, recorded) in window.steps.iter().enumerate() {
        let outcome = field::advance_cached(
            &mut state,
            recorded.ctl,
            window.pointer_speed,
            &mut field::Staging {
                pressures: &mut scratch,
                schedule: &window.schedule,
                stream: &mut stream,
            },
            cache,
        );
        watch(index, &outcome.records);
    }
}

/// The Field a severance leaves, and the pass prepared for it. A severance is
/// the one edit that changes the shape, so it is the one that needs its own
/// preparation — built once for the eight samples that share it.
pub(crate) fn severed_cache(window: &Window<'_>, routes: &[u32]) -> StepCache {
    let mut state = window.start.clone();
    state.routes.retain(|held| routes.binary_search(&held.route).is_err());
    StepCache::of(&state)
}

/// Walks a step's record list and the ascending Node list together. Both are
/// ascending, so this is one pass rather than a search per entry.
pub(crate) fn walk(entries: &[(u32, Fx)], nodes: &[u32], mut found: impl FnMut(usize, Fx)) {
    let mut place = 0;
    for (id, value) in entries {
        while place < nodes.len() && nodes[place] < *id {
            place += 1;
        }
        if place < nodes.len() && nodes[place] == *id {
            found(place, *value);
        }
    }
}

/// The same walk over a list of identifiers alone, which is what `z` is.
pub(crate) fn walk_ids(entries: &[u32], nodes: &[u32], mut found: impl FnMut(usize)) {
    let mut place = 0;
    for id in entries {
        while place < nodes.len() && nodes[place] < *id {
            place += 1;
        }
        if place < nodes.len() && nodes[place] == *id {
            found(place);
        }
    }
}

/// The sum of a step's record over a marked set of Node places.
pub(crate) fn sum_marked(entries: &[(u32, Fx)], nodes: &[u32], marked: &[bool]) -> Fx {
    let mut total = 0;
    walk(entries, nodes, |place, value| {
        if marked[place] {
            total += value;
        }
    });
    total
}

// ---------------------------------------------------------------------------
// Direction symbols, agreement, and the replay deviation
// ---------------------------------------------------------------------------

/// The direction symbols of a series under its own level margin.
///
/// `Delta = v(t) - v(t - 1)`; the symbol is +1 when `Delta > delta`, -1 when
/// `Delta < -delta`, and 0 otherwise, with `delta = tau * (range of the
/// series)`. Both sides are scaled by 65536 rather than the margin being
/// rounded, so nothing is lost to a floor.
pub(crate) fn symbols_under(series: &[Fx], margin_range: Fx, tau: Frac) -> Vec<i8> {
    let margin = margin_range.saturating_mul(tau);
    series
        .windows(2)
        .map(|pair| {
            let moved = (pair[1] - pair[0]).saturating_mul(FRAC_ONE);
            if moved > margin {
                1
            } else if moved < -margin {
                -1
            } else {
                0
            }
        })
        .collect()
}

/// A series' own range: largest less smallest.
pub(crate) fn range_of(series: &[Fx]) -> Fx {
    let low = series.iter().copied().min().unwrap_or(0);
    let high = series.iter().copied().max().unwrap_or(0);
    high - low
}

/// The symbols of a series under the margin its own range gives.
pub(crate) fn symbols(series: &[Fx], tau: Frac) -> Vec<i8> {
    symbols_under(series, range_of(series), tau)
}

/// The agreement of two symbol series: the fraction of steps on which the
/// symbols are equal, and none when there is no step to compare.
pub(crate) fn agreement(first: &[i8], second: &[i8]) -> Option<Frac> {
    debug_assert_eq!(first.len(), second.len(), "two series over the same steps");
    if first.is_empty() {
        return None;
    }
    let same = first.iter().zip(second.iter()).filter(|(a, b)| a == b).count();
    Some(frac_of(same as i128, first.len() as i128))
}

// ---------------------------------------------------------------------------
// Scale Stability
// ---------------------------------------------------------------------------

/// The proximity order of an inside: start at the smallest identifier, then
/// repeatedly append the not-yet-listed member nearest by the declared distance
/// to the most recently appended one, smallest identifier on equal distances.
///
/// The nearest is found without taking a root for every pair. The locked
/// distance is `isqrt(plane squared) + 512 units per layer of separation`, so
/// among candidates at one layer separation the order of the squared plane
/// distance is the order of the distance itself; the root is taken once per
/// layer separation present rather than once per candidate. The answer is the
/// same number the pairwise rule gives, and the pass is what keeps Scale
/// Stability inside the analysis budget at the Node cap.
pub(crate) fn proximity_order(structure: &Structure, members: &[usize]) -> Vec<usize> {
    let mut left: Vec<usize> = members.to_vec();
    left.sort_unstable();
    let mut order = Vec::with_capacity(left.len());
    if left.is_empty() {
        return order;
    }
    order.push(left.remove(0));
    // The plane distance of a squared separation, and every square that reads
    // the same distance: `isqrt` floors, so two different squares can name one
    // distance, and the tie among them goes to the smallest identifier exactly
    // as the rule says.
    let squares_of = |square: u128| -> (Fx, u128) {
        let root = crate::fx::isqrt(square);
        (root as Fx, u128::from(root + 1) * u128::from(root + 1))
    };
    while !left.is_empty() {
        let from = *order.last().expect("the order opened with a member");
        let (pos, layer) = structure.placed[from];
        let square_to = |place: usize| -> (usize, u128) {
            let (other, other_layer) = structure.placed[place];
            let dx = i128::from(pos.x) - i128::from(other.x);
            let dy = i128::from(pos.y) - i128::from(other.y);
            (usize::from(layer.abs_diff(other_layer)), (dx * dx + dy * dy) as u128)
        };
        // One pass for the smallest square per layer separation.
        let mut smallest: [Option<u128>; 8] = [None; 8];
        for place in &left {
            let (separation, square) = square_to(*place);
            let held = &mut smallest[separation];
            if held.is_none_or(|found| square < found) {
                *held = Some(square);
            }
        }
        // The distance each separation's nearest reads, and the square below
        // which a member of that separation reads the same distance.
        let mut reach: [Option<(Fx, u128)>; 8] = [None; 8];
        let mut nearest: Option<Fx> = None;
        for (separation, held) in smallest.iter().enumerate() {
            let Some(square) = held else { continue };
            let (plane, bound) = squares_of(*square);
            let found = plane + crate::fx::LAYER_SEPARATION * separation as Fx;
            reach[separation] = Some((found, bound));
            if nearest.is_none_or(|best| found < best) {
                nearest = Some(found);
            }
        }
        let nearest = nearest.expect("a nonempty remainder holds a nearest member");
        // One more pass for the smallest identifier at that distance, which is
        // the first of the ascending remainder that reaches it.
        let index = left
            .iter()
            .position(|place| {
                let (separation, square) = square_to(*place);
                reach[separation].is_some_and(|(found, bound)| found == nearest && square < bound)
            })
            .expect("the nearest distance was read off a member of the remainder");
        order.push(left.remove(index));
    }
    order
}

/// The grain pairs Scale Stability considers: `(rho / 2, rho)` and
/// `(rho, 2 * rho)`, each qualifying when both grains lie on the ladder and
/// both are at most the member count.
pub(crate) fn grain_pairs(resolution: u16, members: usize) -> Vec<(u16, u16)> {
    let qualifies = |grain: u16| LADDER.contains(&grain) && usize::from(grain) <= members;
    let mut found = Vec::new();
    for (small, large) in [(resolution / 2, resolution), (resolution, resolution * 2)] {
        if small > 0 && qualifies(small) && qualifies(large) {
            found.push((small, large));
        }
    }
    found
}

/// The block series at one grain: the sum of stored Charge over each block's
/// members, over the steps the caller names.
pub(crate) fn block_series(order: &[usize], q: &[Vec<Fx>], grain: usize, from: usize, to: usize) -> Vec<Vec<Fx>> {
    order
        .chunks(grain)
        .map(|block| {
            (from..to)
                .map(|step| block.iter().map(|slot| q[*slot][step]).sum::<Fx>())
                .collect()
        })
        .collect()
}

/// Scale Stability over one part of the window: the mean of the qualifying
/// pair values, each the mean of its blocks' agreements with their parents.
pub(crate) fn scale_stability_over(
    order: &[usize],
    q: &[Vec<Fx>],
    pairs: &[(u16, u16)],
    tau: Frac,
    from: usize,
    to: usize,
) -> Option<Frac> {
    if to - from < 2 {
        return None;
    }
    let mut values = Vec::new();
    for (small, large) in pairs {
        let blocks = block_series(order, q, usize::from(*small), from, to);
        let parents = block_series(order, q, usize::from(*large), from, to);
        if blocks.is_empty() {
            continue;
        }
        let mut total: i128 = 0;
        for (index, block) in blocks.iter().enumerate() {
            // The parent of block j at grain g is block ceil(j / 2) at grain
            // 2g, and the blocks here are 0-based, so block index i has parent
            // index i / 2.
            let parent = &parents[index / 2];
            let held = agreement(&symbols(block, tau), &symbols(parent, tau))
                .expect("a part of two or more steps has a step to compare");
            total += i128::from(held);
        }
        values.push(frac_of(total, blocks.len() as i128 * i128::from(FRAC_ONE)) as i128);
    }
    if values.is_empty() {
        return None;
    }
    let count = values.len() as i128;
    Some(frac_of(values.iter().sum::<i128>(), count * i128::from(FRAC_ONE)))
}

// ---------------------------------------------------------------------------
// Cut Impact's circulating flow
// ---------------------------------------------------------------------------

/// The strongly connected components of a graph over Node places, by Tarjan's
/// method, iteratively — the Node cap is 256 and a recursive walk would put
/// that depth on the stack.
pub(crate) fn components(count: usize, near: &[Vec<usize>]) -> (Vec<usize>, usize) {
    const UNVISITED: usize = usize::MAX;
    let mut index = vec![UNVISITED; count];
    let mut low = vec![0usize; count];
    let mut on_stack = vec![false; count];
    let mut stack: Vec<usize> = Vec::new();
    let mut component = vec![UNVISITED; count];
    let mut next = 0;
    let mut found = 0;
    let mut work: Vec<(usize, usize)> = Vec::new();

    for root in 0..count {
        if index[root] != UNVISITED {
            continue;
        }
        index[root] = next;
        low[root] = next;
        next += 1;
        stack.push(root);
        on_stack[root] = true;
        work.push((root, 0));
        while let Some((node, edge)) = work.pop() {
            if edge < near[node].len() {
                work.push((node, edge + 1));
                let other = near[node][edge];
                if index[other] == UNVISITED {
                    index[other] = next;
                    low[other] = next;
                    next += 1;
                    stack.push(other);
                    on_stack[other] = true;
                    work.push((other, 0));
                } else if on_stack[other] {
                    low[node] = low[node].min(index[other]);
                }
                continue;
            }
            if let Some((parent, _)) = work.last() {
                low[*parent] = low[*parent].min(low[node]);
            }
            if low[node] == index[node] {
                loop {
                    let held = stack.pop().expect("a root closes over its own component");
                    on_stack[held] = false;
                    component[held] = found;
                    if held == node {
                        break;
                    }
                }
                found += 1;
            }
        }
    }
    (component, found)
}

/// The circulating flow of a window: the sum of window flows over the cyclic
/// Routes with tail or head in the inside.
///
/// A Route is cyclic when its tail and head lie in one strongly connected
/// component of the flow-positive graph and that component contains a cycle —
/// two or more Nodes, or a Route from a Node to itself. A self-loop is
/// therefore counted here exactly as FRAMEWORK.md says it is: it is never in
/// the crossing set, so a severance never removes it, its flow enters the
/// recorded total and every severed total alike, it cancels in the numerator,
/// and it inflates only the denominator. An inside that circulates through its
/// own loops reads a low Cut Impact, and that reading is the truth about it.
pub(crate) fn circulating(
    structure: &Structure,
    member: &[bool],
    flows: &[Fx],
    present: &[bool],
) -> Fx {
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
    let (component, found) = components(count, &near);
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
    let mut total = 0;
    for (place, (_, tail, head)) in structure.routes.iter().enumerate() {
        if !present[place] || !(member[*tail] || member[*head]) {
            continue;
        }
        if component[*tail] == component[*head] && cyclic[component[*tail]] {
            total += flows[place];
        }
    }
    total
}

// ---------------------------------------------------------------------------
// The evaluation
// ---------------------------------------------------------------------------

/// Reads the four values of every candidate of a slate, compares them, and
/// fills in the tiers, the dominance relation, the sensitivity flag, the
/// baseline deviations, and the standing candidate's forecast envelope.
///
/// The order is the analysis budget's: per candidate in assembly order —
/// baselines, Shared Failure, Cut Impact, Boundary Sufficiency, Scale
/// Stability — then the comparison, then tolerance sensitivity, which is a
/// recomputation of the comparison alone and runs no new replay.
pub fn evaluate(state: &RunState, slate: &mut CandidateSlate) {
    let tau = slate.tau;
    let structure = Structure::of(&state.now);
    let window = Window::of(state, slate.window_effective);

    for position in 0..slate.candidates.len() {
        let sets = Sets::of(&structure, &slate.candidates[position].view);
        let sigma = slate.sigma;
        let place = position as u8 + 1;
        let (baseline, envelope) = baselines(&window, &structure, &sets, &sigma, place, &[], tau);
        slate.candidates[position].baseline = baseline;
        if position == 0 {
            slate.forecast_envelope = envelope;
        }
        slate.candidates[position].privilege = profile(
            &window,
            &structure,
            &sets,
            &slate.candidates[position].view,
            &sigma,
            place,
            &[],
            tau,
        );
    }

    rank(slate, tau);
}

/// The four values of one View over one window, in the analysis budget's own
/// order — Shared Failure, Cut Impact, Boundary Sufficiency, Scale Stability.
///
/// `extra` is the stream insertion the two recomputing perturbation kinds name
/// (see [`sample_stream_under`]); it is empty for an ordinary evaluation, so a
/// slate and a window-change recomputation run the same code over different
/// streams rather than two implementations of one procedure.
#[allow(clippy::too_many_arguments)]
pub(crate) fn profile(
    window: &Window<'_>,
    structure: &Structure,
    sets: &Sets,
    view: &ViewDeclaration,
    sigma: &RngState,
    position: u8,
    extra: &[Part<'_>],
    tau: Frac,
) -> PrivilegeProfile {
    if window.len() < 1 {
        // FRAMEWORK.md, verbatim: when `w_eff = 0` there are no steps to
        // observe and every windowed procedure is unassigned. This is the
        // reading right after an applying commit, and it is honest rather
        // than degenerate — the span regrows a step at a time.
        return PrivilegeProfile::unassigned(WINDOW_TOO_SHORT);
    }
    PrivilegeProfile {
        shared_failure: shared_failure(window, structure, sets, sigma, position, extra, tau),
        cut_impact: cut_impact(window, structure, sets, sigma, position, extra),
        boundary_sufficiency: boundary_sufficiency(window, structure, sets, sigma, position, extra),
        scale_stability: scale_stability(window, structure, sets, view, tau),
    }
}

/// The eight baseline replays of one View, and the envelope they draw.
///
/// They are computed once per evaluated View and shared by every procedure that
/// names them — the deviation of an edited replay is taken against the baseline
/// sample of the same number, which is the locked excess rule the perturbation
/// kinds read.
pub(crate) fn baselines(
    window: &Window<'_>,
    structure: &Structure,
    sets: &Sets,
    sigma: &RngState,
    position: u8,
    extra: &[Part<'_>],
    tau: Frac,
) -> ([Option<Frac>; BASELINE_SAMPLES], Vec<(Fx, Fx)>) {
    let mut deviations = [None; BASELINE_SAMPLES];
    let mut envelope: Vec<(Fx, Fx)> = Vec::new();
    if window.len() == 0 {
        return (deviations, envelope);
    }
    let recorded: Vec<Fx> = window
        .steps
        .iter()
        .map(|step| sum_marked(&step.records.q, &structure.nodes, &sets.member))
        .collect();
    let margin = range_of(&recorded);
    let recorded_symbols = symbols(&recorded, tau);
    envelope = vec![(Fx::MAX, Fx::MIN); window.len()];
    for sample in 1..=BASELINE_SAMPLES {
        let mut replayed = vec![0 as Fx; window.len()];
        replay(
            window,
            &Edit::None,
            &window.cache,
            sample_stream_under(sigma, position, extra, STREAM_BASELINE, sample),
            |index, records| {
                replayed[index] = sum_marked(&records.q, &structure.nodes, &sets.member);
            },
        );
        for (step, held) in replayed.iter().enumerate() {
            envelope[step].0 = envelope[step].0.min(*held);
            envelope[step].1 = envelope[step].1.max(*held);
        }
        // The replay deviation reads both series under the one margin the
        // recorded series gives, which is what makes it a reading of the
        // replay rather than of two different scales.
        deviations[sample - 1] = agreement(
            &recorded_symbols,
            &symbols_under(&replayed, margin, tau),
        )
        .map(|held| FRAC_ONE - held);
    }
    (deviations, envelope)
}

/// **Scale Stability** — does the inside's pattern read the same at nearby
/// grains? Deterministic; no random draws; read off the recorded window.
pub(crate) fn scale_stability(
    window: &Window<'_>,
    structure: &Structure,
    sets: &Sets,
    view: &ViewDeclaration,
    tau: Frac,
) -> PrivilegeValue {
    if window.len() < 2 {
        return PrivilegeValue::unassigned(WINDOW_TOO_SHORT);
    }
    let pairs = grain_pairs(view.resolution, sets.members.len());
    if pairs.is_empty() {
        return PrivilegeValue::unassigned(NO_GRAIN_PAIR);
    }
    let places: Vec<usize> =
        (0..structure.count()).filter(|place| sets.member[*place]).collect();
    let order = proximity_order(structure, &places);
    // Stored Charge per member, in proximity order, over the window's steps.
    let mut q = vec![vec![0 as Fx; window.len()]; order.len()];
    let mut slot_of = vec![usize::MAX; structure.count()];
    for (slot, place) in order.iter().enumerate() {
        slot_of[*place] = slot;
    }
    for (step, recorded) in window.steps.iter().enumerate() {
        walk(&recorded.records.q, &structure.nodes, |place, value| {
            if slot_of[place] != usize::MAX {
                q[slot_of[place]][step] = value;
            }
        });
    }
    let slots: Vec<usize> = (0..order.len()).collect();
    let Some(value) = scale_stability_over(&slots, &q, &pairs, tau, 0, window.len()) else {
        return PrivilegeValue::unassigned(WINDOW_TOO_SHORT);
    };
    // The window-split range: the value on the full window, on the first
    // floor(w / 2) steps, and on the rest. The full-window part is one of the
    // three, so the range contains the value by construction.
    let half = window.len() / 2;
    let parts = if window.len() >= 4 {
        match (
            scale_stability_over(&slots, &q, &pairs, tau, 0, half),
            scale_stability_over(&slots, &q, &pairs, tau, half, window.len()),
        ) {
            (Some(first), Some(second)) => Some((first, second)),
            _ => None,
        }
    } else {
        None
    };
    match parts {
        Some((first, second)) => PrivilegeValue::assigned(
            value,
            value.min(first).min(second),
            value.max(first).max(second),
            1,
        ),
        None => PrivilegeValue::assigned(value, value, value, 1),
    }
}

/// **Shared Failure** — do the members fail together more strongly than the
/// surround does? Stochastic: 8 probe samples.
pub(crate) fn shared_failure(
    window: &Window<'_>,
    structure: &Structure,
    sets: &Sets,
    sigma: &RngState,
    position: u8,
    extra: &[Part<'_>],
    tau: Frac,
) -> PrivilegeValue {
    if sets.members.len() < 2 {
        return PrivilegeValue::unassigned(FEW_MEMBERS);
    }
    if sets.surround_count() < 2 {
        return PrivilegeValue::unassigned(FEW_SURROUND);
    }
    // The probe strength: the fraction `min(1, 4 * tau)` of stored Charge.
    let fraction = (tau.saturating_mul(4)).min(FRAC_ONE);
    let members = sets.members.len() as i128;
    let surround = sets.surround_count() as i128;
    let mut defined = Vec::new();
    for sample in 1..=SAMPLES {
        // `chi(A) = sum C(C - 1) / ((|A| - 1) * sum C)`, accumulated over the
        // replayed window as the samples come.
        let (mut inside_pairs, mut inside_total) = (0i128, 0i128);
        let (mut outside_pairs, mut outside_total) = (0i128, 0i128);
        replay(
            window,
            &Edit::Probe { fraction, nodes: &sets.probed },
            &window.cache,
            sample_stream_under(sigma, position, extra, STREAM_SHARED_FAILURE, sample),
            |_, records| {
                let (mut held_in, mut held_out) = (0i128, 0i128);
                walk_ids(&records.z, &structure.nodes, |place| {
                    if sets.member[place] {
                        held_in += 1;
                    } else if sets.surround[place] {
                        held_out += 1;
                    }
                });
                inside_pairs += held_in * (held_in - 1);
                inside_total += held_in;
                outside_pairs += held_out * (held_out - 1);
                outside_total += held_out;
            },
        );
        // The sample is undefined when either chi is: a failure sum of zero
        // leaves the co-failure index with no denominator.
        if inside_total == 0 || outside_total == 0 {
            continue;
        }
        let inside = frac_of(inside_pairs, (members - 1) * inside_total);
        let outside = frac_of(outside_pairs, (surround - 1) * outside_total);
        // `SF_j = (chi_I - chi_U + 1) / 2`, in [0, 1].
        defined.push((inside - outside + FRAC_ONE + 1) / 2);
    }
    from_samples(defined)
}

/// **Cut Impact** — how much of the circulating Charge that touches the inside
/// stops when the boundary is severed? Stochastic: 8 severed samples.
pub(crate) fn cut_impact(
    window: &Window<'_>,
    structure: &Structure,
    sets: &Sets,
    sigma: &RngState,
    position: u8,
    extra: &[Part<'_>],
) -> PrivilegeValue {
    let present = vec![true; structure.routes.len()];
    let mut recorded = vec![0 as Fx; structure.routes.len()];
    for step in &window.steps {
        add_flows(&step.records.f, structure, &mut recorded);
    }
    let total = circulating(structure, &sets.member, &recorded, &present);
    if total <= 0 {
        return PrivilegeValue::unassigned(NO_CIRCULATING_FLOW);
    }
    if sets.severed.is_empty() {
        // Nothing crosses: the inside is the whole of N, or nothing connects
        // it. Severance removes nothing, so nothing stops, and no replay runs.
        return PrivilegeValue::assigned(0, 0, 0, 0);
    }
    // The Routes a severance leaves standing, in the Structure's own order —
    // the replayed Field holds only these, and the flows of the others are
    // zero by construction.
    let kept: Vec<bool> = structure
        .routes
        .iter()
        .map(|(route, _, _)| sets.severed.binary_search(route).is_err())
        .collect();
    let cut = severed_cache(window, &sets.severed);
    let mut samples = Vec::with_capacity(SAMPLES);
    for sample in 1..=SAMPLES {
        let mut flows = vec![0 as Fx; structure.routes.len()];
        replay(
            window,
            &Edit::Sever { routes: &sets.severed },
            &cut,
            sample_stream_under(sigma, position, extra, STREAM_CUT_IMPACT, sample),
            |_, records| add_flows(&records.f, structure, &mut flows),
        );
        let severed = circulating(structure, &sets.member, &flows, &kept);
        let stopped = (total - severed).max(0);
        samples.push(frac_of(i128::from(stopped), i128::from(total)));
    }
    samples.sort_unstable();
    let low = samples[0];
    let high = samples[samples.len() - 1];
    PrivilegeValue::assigned(median(&samples), low, high, SAMPLES as u16)
}

/// Adds one step's Route records into a window-flow total per Route place.
pub(crate) fn add_flows(entries: &[(u32, Fx)], structure: &Structure, into: &mut [Fx]) {
    let mut place = 0;
    for (route, moved) in entries {
        while place < structure.routes.len() && structure.routes[place].0 < *route {
            place += 1;
        }
        if place < structure.routes.len() && structure.routes[place].0 == *route {
            into[place] += moved;
        }
    }
}

/// **Boundary Sufficiency** — does traffic across the boundary account for the
/// surround's effect on the inside? Stochastic: 8 samples, no edit.
pub(crate) fn boundary_sufficiency(
    window: &Window<'_>,
    structure: &Structure,
    sets: &Sets,
    sigma: &RngState,
    position: u8,
    extra: &[Part<'_>],
) -> PrivilegeValue {
    let interior: Vec<bool> = (0..structure.count())
        .map(|place| sets.member[place] && !sets.shell[place])
        .collect();
    let mut defined = Vec::new();
    for sample in 1..=SAMPLES {
        let (mut accounted, mut unaccounted) = (0i128, 0i128);
        replay(
            window,
            &Edit::None,
            &window.cache,
            sample_stream_under(sigma, position, extra, STREAM_BOUNDARY_SUFFICIENCY, sample),
            |_, records| {
                let mut place = 0;
                for (route, moved) in &records.f {
                    while place < structure.routes.len() && structure.routes[place].0 < *route {
                        place += 1;
                    }
                    if place < structure.routes.len()
                        && structure.routes[place].0 == *route
                        && sets.crossing[place]
                    {
                        accounted += i128::from(*moved);
                    }
                }
                walk(&records.e, &structure.nodes, |held, value| {
                    if sets.shell[held] {
                        accounted += i128::from(value.abs());
                    } else if interior[held] {
                        unaccounted += i128::from(value.abs());
                    }
                });
            },
        );
        // Undefined when there is no boundary traffic and no exogenous
        // influence at all — the zero-traffic case, which is skipped.
        if accounted + unaccounted == 0 {
            continue;
        }
        defined.push(frac_of(accounted, accounted + unaccounted));
    }
    from_samples(defined)
}

// ---------------------------------------------------------------------------
// Comparison, dominance, tiers, and tolerance sensitivity
// ---------------------------------------------------------------------------

/// **Better.** One candidate is better than another on one value exactly when
/// `low_A - high_B > tau` for that value's confidence ranges. Bare numbers are
/// never compared; a value unassigned for either candidate is excluded.
pub fn better(first: &PrivilegeValue, second: &PrivilegeValue, tau: Frac) -> bool {
    match (first.low, second.high) {
        (Some(low), Some(high)) => low - high > tau,
        _ => false,
    }
}

/// **Dominance.** A dominates B exactly when their shared assigned set is
/// nonempty, B is better on none of it, and A is better on at least one of it.
/// This is the only dominance rule: no worse everywhere that both are measured,
/// better somewhere.
pub fn dominates(first: &PrivilegeProfile, second: &PrivilegeProfile, tau: Frac) -> bool {
    let mut shared = 0;
    let mut ahead = 0;
    for (a, b) in first.each().into_iter().zip(second.each()) {
        if !a.is_assigned() || !b.is_assigned() {
            continue;
        }
        shared += 1;
        if better(b, a, tau) {
            return false;
        }
        if better(a, b, tau) {
            ahead += 1;
        }
    }
    shared > 0 && ahead > 0
}

/// The nondominated set of a subset of the slate: the positions no other
/// position of the subset dominates.
fn nondominated(profiles: &[PrivilegeProfile], left: &[usize], tau: Frac) -> Vec<usize> {
    left.iter()
        .copied()
        .filter(|held| {
            !left
                .iter()
                .any(|other| other != held && dominates(&profiles[*other], &profiles[*held], tau))
        })
        .collect()
}

/// Compares the slate and records what the comparison found: the dominance
/// relation itself, the tiers derived from it, and the tolerance-sensitivity
/// flag.
///
/// A deficient slate is not compared — no comparison runs on one and nothing is
/// adopted from one — so its one candidate keeps tier 0, the number no tier
/// has, and the relation stays empty.
fn rank(slate: &mut CandidateSlate, tau: Frac) {
    if slate.deficient {
        return;
    }
    let profiles: Vec<PrivilegeProfile> =
        slate.candidates.iter().map(|held| held.privilege.clone()).collect();
    let count = profiles.len();

    for first in 0..count {
        for second in 0..count {
            if first != second && dominates(&profiles[first], &profiles[second], tau) {
                slate.dominance.push((first as u8 + 1, second as u8 + 1));
            }
        }
    }
    slate.dominance.sort_unstable();

    // Tier 1 is the nondominated set; tier r + 1 is the nondominated set of
    // what stands once tiers 1 through r are removed. Within a tier the
    // candidates keep assembly order, which the record's own order already is.
    let mut left: Vec<usize> = (0..count).collect();
    let mut tier: u8 = 1;
    while !left.is_empty() {
        let held = nondominated(&profiles, &left, tau);
        debug_assert!(!held.is_empty(), "a finite dominance relation has a nondominated set");
        for position in &held {
            slate.candidates[*position].tier = tier;
        }
        left.retain(|position| !held.contains(position));
        tier += 1;
    }

    // Tolerance sensitivity: the comparison stage alone, recomputed at half and
    // at double the declared tolerance, each held inside (0, 1/2]. The same
    // samples, the same values, the same confidence ranges — sensitivity
    // measures the reading, never new probes.
    let whole: Vec<usize> = (0..count).collect();
    let standing = nondominated(&profiles, &whole, tau);
    for (name, other) in [
        (SENSITIVITY_HALF, (tau / 2).clamp(1, FRAC_ONE / 2)),
        (SENSITIVITY_DOUBLE, (tau * 2).clamp(1, FRAC_ONE / 2)),
    ] {
        if nondominated(&profiles, &whole, other) != standing {
            slate.sensitivity_changed_at.push(name);
        }
    }
    slate.sensitivity = !slate.sensitivity_changed_at.is_empty();
}
