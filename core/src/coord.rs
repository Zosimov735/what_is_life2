//! The ten-coordinate profile: descriptive readings, none of which is ever
//! combined with any other.
//!
//! `docs/field-framework/FRAMEWORK.md`'s Coordinate profile section is the
//! whole of what this module does, carried out exactly as it is written: Swap
//! Range with its per-connected-component evaluation, Self-Support, Throughput
//! with its itemized record, Upkeep Mix, Reach, Input Resolution, Horizon with
//! its `w_eff >= 1` assignment rule, Source Trace, Instruction Separation, and
//! Turnover Tolerance with its `(phi, agreement)` ladder.
//! `docs/field-framework/ARCHITECTURE.md` locks the serialized shape
//! (`CoordinateProfile`), the fixed-point representation of each coordinate,
//! and the two-request split — the eight recorded-window coordinates on a
//! `coordinates` request, the two replay-based ones only on `coordinates_full`.
//!
//! **No coordinate joins a composite.** There is no scalar anywhere in this
//! module: ten readings stand separately, none enters the dominance comparison,
//! and nothing here produces a figure that stands for a View as a whole. A
//! reading that cannot be taken is unassigned with its stated reason, never a
//! zero standing in for one.
//!
//! Two properties are load-bearing:
//!
//! - **Windowed.** Every reading runs under the effective window
//!   `w_eff = min(w, t0, retained_span)`, so right after an applying commit the
//!   windowed readings are honestly unassigned and the span regrows a step at a
//!   time. Swap Range and Reach still read the Field's own shape, which is what
//!   the framework asks of them.
//! - **Integer.** No float appears. Fractions are raw `Frac` — the quantity
//!   times 65536 — and every ratio goes through the one rounding rule
//!   [`crate::rank::frac_of`] holds, so identical inputs produce an identical
//!   record byte for byte.

use crate::field::{StepCache, UPKEEP_PURPOSES};
use crate::json::Obj;
use crate::perturb::{Prepared, Recorded};
use crate::rank::{self, agreement, frac_of, median, symbols, symbols_under, Sets, Structure, Window};
use crate::rng::{Part, RngState};
use crate::slate::{written_value, PrivilegeValue, WINDOW_TOO_SHORT};
use crate::state::{Frac, Fx, RunState, ViewDeclaration, FRAC_ONE};

/// How many samples the two replay-based coordinates take.
const SAMPLES: usize = 8;

/// Reasons a coordinate is unassigned, each one FRAMEWORK.md's own.
pub const NO_UPKEEP: &str = "no-upkeep";
pub const NO_SIGNATURE: &str = "no-signature";
pub const NO_STORED_CHARGE: &str = "no-stored-charge";

/// The stream names FRAMEWORK.md gives the two replay-based coordinates.
pub const STREAM_INSTRUCTION_SEPARATION: &str = "instruction-separation";
pub const STREAM_TURNOVER: &str = "turnover";

/// One reading: a number, or no number at all and the stated reason.
#[derive(Clone, Copy, Debug)]
pub struct Reading {
    pub value: Option<i64>,
    pub reason: Option<&'static str>,
}

impl Reading {
    fn at(value: i64) -> Self {
        Reading { value: Some(value), reason: None }
    }

    fn none(reason: &'static str) -> Self {
        Reading { value: None, reason: Some(reason) }
    }

    fn write(&self, out: &mut Obj<'_>, key: &str) {
        let mut held = out.object(key);
        match self.reason {
            Some(reason) => held.text("reason", reason),
            None => held.null("reason"),
        };
        held.int_or_null("value", self.value);
        held.end();
    }
}

/// Throughput's itemized record: the two magnitudes, and the identity beside
/// them — every crossing Route's mean flow per step, and every shell member's
/// mean exogenous magnitude per step.
#[derive(Clone, Debug, Default)]
pub struct Throughput {
    pub in_rate: Fx,
    pub out_rate: Fx,
    pub routes: Vec<(u32, Fx)>,
    pub shell: Vec<(u32, Fx)>,
}

/// Turnover Tolerance's reading and the ladder it was read off.
#[derive(Clone, Debug)]
pub struct Turnover {
    pub value: Option<Frac>,
    pub reason: Option<&'static str>,
    pub pairs: Option<Vec<(Frac, Frac)>>,
}

/// The ten coordinates of one View at one step.
#[derive(Clone, Debug)]
pub struct CoordinateProfile {
    pub view: ViewDeclaration,
    pub step: u32,
    pub swap_range: Reading,
    pub self_support: Reading,
    pub throughput: Throughput,
    pub upkeep_mix: Option<[Frac; UPKEEP_PURPOSES]>,
    pub upkeep_reason: Option<&'static str>,
    pub reach: Reading,
    pub input_resolution: Reading,
    pub horizon: Reading,
    pub source_trace: Reading,
    /// Null until a `coordinates_full` request runs the replays.
    pub instruction_separation: Option<PrivilegeValue>,
    /// Null until a `coordinates_full` request runs the replays.
    pub turnover_tolerance: Option<Turnover>,
}

impl CoordinateProfile {
    pub fn write(&self, out: &mut String) {
        let mut object = Obj::new(out);
        self.horizon.write(&mut object, "horizon");
        self.input_resolution.write(&mut object, "input_resolution");
        match &self.instruction_separation {
            Some(value) => object.raw("instruction_separation", &written_value(value)),
            None => object.null("instruction_separation"),
        };
        self.reach.write(&mut object, "reach");
        self.self_support.write(&mut object, "self_support");
        self.source_trace.write(&mut object, "source_trace");
        object.int("step", i64::from(self.step));
        self.swap_range.write(&mut object, "swap_range");
        {
            let mut held = object.object("throughput");
            held.int("in_rate", self.throughput.in_rate);
            held.int("out_rate", self.throughput.out_rate);
            {
                let mut routes = held.list("routes");
                for (route, rate) in &self.throughput.routes {
                    let mut entry = routes.object();
                    entry.int("rate", *rate);
                    entry.int("route", i64::from(*route));
                    entry.end();
                }
                routes.end();
            }
            {
                let mut shell = held.list("shell");
                for (node, rate) in &self.throughput.shell {
                    let mut entry = shell.object();
                    entry.int("node", i64::from(*node));
                    entry.int("rate", *rate);
                    entry.end();
                }
                shell.end();
            }
            held.end();
        }
        match &self.turnover_tolerance {
            None => {
                object.null("turnover_tolerance");
            }
            Some(turnover) => {
                let mut held = object.object("turnover_tolerance");
                match &turnover.pairs {
                    Some(pairs) => {
                        let mut written = held.list("pairs");
                        for (phi, agreed) in pairs {
                            let mut entry = written.object();
                            entry.int("agreement", *agreed);
                            entry.int("phi", *phi);
                            entry.end();
                        }
                        written.end();
                    }
                    None => {
                        held.null("pairs");
                    }
                }
                match turnover.reason {
                    Some(reason) => held.text("reason", reason),
                    None => held.null("reason"),
                };
                held.int_or_null("value", turnover.value);
                held.end();
            }
        }
        {
            let mut held = object.object("upkeep_mix");
            match self.upkeep_reason {
                Some(reason) => held.text("reason", reason),
                None => held.null("reason"),
            };
            match &self.upkeep_mix {
                Some(shares) => {
                    let mut written = held.list("value");
                    for share in shares {
                        written.int(*share);
                    }
                    written.end();
                }
                None => {
                    held.null("value");
                }
            }
            held.end();
        }
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
}

/// Reads the eight recorded-window coordinates of a View.
///
/// The two replay-based coordinates are null here and are filled in by
/// [`full`], which is the split ARCHITECTURE.md locks between the `coordinates`
/// and `coordinates_full` inspect requests: a reading that costs replays is not
/// taken until it is asked for.
pub fn of(state: &RunState, view: &ViewDeclaration, tau: Frac) -> CoordinateProfile {
    let structure = Structure::of(&state.now);
    let window = Window::of(state, state.effective_window(view.window));
    let sets = Sets::of(&structure, view);
    of_over(state, view, tau, &structure, &window, &sets)
}

/// The eight recorded-window coordinates, over parts the caller already built.
///
/// [`of`] and [`full`] both come through here, so the window-start regeneration
/// and the two derived tables are built once per request rather than once per
/// half of one.
fn of_over(
    state: &RunState,
    view: &ViewDeclaration,
    tau: Frac,
    structure: &Structure,
    window: &Window<'_>,
    sets: &Sets,
) -> CoordinateProfile {
    let steps = window.len();

    // Every per-step aggregate one pass over the window supplies, so the eight
    // recorded-window coordinates cost one walk of the trace rather than eight.
    let mut flows = vec![0 as Fx; structure.routes.len()];
    let mut inflow: i128 = 0;
    let mut outflow: i128 = 0;
    let mut upkeep_total: i128 = 0;
    let mut upkeep_mix = [0i128; UPKEEP_PURPOSES];
    let mut shell_magnitude = vec![0i128; structure.count()];
    let mut series: Vec<Fx> = Vec::with_capacity(steps);
    let mut removal: Vec<Fx> = Vec::with_capacity(steps);
    for step in &window.steps {
        rank::add_flows(&step.records.f, structure, &mut flows);
        let mut into = 0i128;
        let mut out = 0i128;
        let mut place = 0;
        for (route, moved) in &step.records.f {
            while place < structure.routes.len() && structure.routes[place].0 < *route {
                place += 1;
            }
            if place < structure.routes.len()
                && structure.routes[place].0 == *route
                && sets.crossing[place]
            {
                let (_, tail, head) = structure.routes[place];
                if sets.member[head] {
                    into += i128::from(*moved);
                } else if sets.member[tail] {
                    out += i128::from(*moved);
                }
            }
        }
        inflow += into;
        outflow += out;
        let mut paid = 0i128;
        for entry in &step.records.upkeep {
            let Ok(held) = structure.nodes.binary_search(&entry.node) else { continue };
            if !sets.member[held] {
                continue;
            }
            paid += i128::from(entry.v);
            for (purpose, share) in entry.mix.iter().enumerate() {
                upkeep_mix[purpose] += i128::from(*share);
            }
        }
        upkeep_total += paid;
        // Source Trace's removal total, and the shell magnitudes Throughput
        // itemizes, both read the same exogenous records.
        let mut negative = 0i128;
        rank::walk(&step.records.e, &structure.nodes, |held, value| {
            if sets.member[held] && value < 0 {
                negative += i128::from(-value);
            }
            if sets.shell[held] {
                shell_magnitude[held] += i128::from(value.abs());
            }
        });
        removal.push((out + paid + negative).min(i128::from(Fx::MAX)) as Fx);
        series.push(rank::sum_marked(&step.records.q, &structure.nodes, &sets.member));
    }

    CoordinateProfile {
        view: view.clone(),
        step: state.now.step,
        swap_range: swap_range(structure, sets, &flows),
        self_support: self_support(upkeep_total, inflow, outflow),
        throughput: throughput(structure, sets, &flows, &shell_magnitude, steps),
        upkeep_mix: mix_shares(upkeep_total, &upkeep_mix),
        upkeep_reason: if upkeep_total > 0 { None } else { Some(NO_UPKEEP) },
        reach: reach(structure, sets, &flows),
        input_resolution: input_resolution(window, structure, sets, tau),
        horizon: horizon(&series, &state.now, tau),
        source_trace: source_trace(window, structure, sets, &series, &removal),
        instruction_separation: None,
        turnover_tolerance: None,
    }
}

/// The whole profile: the eight recorded-window coordinates, and the two
/// replay-based ones a `coordinates_full` request pays for.
pub fn full(
    state: &RunState,
    view: &ViewDeclaration,
    sigma: &RngState,
    position: u8,
    tau: Frac,
) -> CoordinateProfile {
    let structure = Structure::of(&state.now);
    let window = Window::of(state, state.effective_window(view.window));
    let sets = Sets::of(&structure, view);
    let mut profile = of_over(state, view, tau, &structure, &window, &sets);
    profile.instruction_separation =
        Some(instruction_separation(&window, &structure, &sets, sigma, position, tau));
    profile.turnover_tolerance =
        Some(turnover_tolerance(&window, &structure, &sets, sigma, position, tau));
    profile
}

// ---------------------------------------------------------------------------
// Swap Range
// ---------------------------------------------------------------------------

/// **Swap Range** — how many members can change while preserving their effect.
///
/// The internal graph has the members as its vertices and an undirected edge
/// between two members joined by an internal Route with positive window flow or
/// by a declared adjacency. **The graph is judged one connected component at a
/// time**, so an inside already standing in several parts is not read as having
/// no member to spare: a member is an articulation member exactly when removing
/// it splits the remaining members of its own component into two or more
/// connected components, and a member alone in its component is never one. Swap
/// Range is the count of members that are not articulation members, and 0 when
/// the inside holds one member.
fn swap_range(structure: &Structure, sets: &Sets, flows: &[Fx]) -> Reading {
    let members: Vec<usize> =
        (0..structure.count()).filter(|place| sets.member[*place]).collect();
    if members.len() <= 1 {
        return Reading::at(0);
    }
    let mut slot = vec![usize::MAX; structure.count()];
    for (index, place) in members.iter().enumerate() {
        slot[*place] = index;
    }
    let count = members.len();
    let mut near: Vec<Vec<usize>> = vec![Vec::new(); count];
    let mut joined = vec![false; count * count];
    let join = |a: usize, b: usize, near: &mut Vec<Vec<usize>>, joined: &mut Vec<bool>| {
        if a == b || joined[a * count + b] {
            return;
        }
        joined[a * count + b] = true;
        joined[b * count + a] = true;
        near[a].push(b);
        near[b].push(a);
    };
    for (place, (_, tail, head)) in structure.routes.iter().enumerate() {
        if flows[place] <= 0 || !sets.member[*tail] || !sets.member[*head] {
            continue;
        }
        join(slot[*tail], slot[*head], &mut near, &mut joined);
    }
    let whole = structure.count();
    for (first, a) in members.iter().enumerate() {
        for (second, b) in members.iter().enumerate() {
            if first < second && structure.adjacency[a * whole + b] {
                join(first, second, &mut near, &mut joined);
            }
        }
    }
    let cut = articulation(count, &near);
    Reading::at((count - cut.iter().filter(|held| **held).count()) as i64)
}

/// The articulation vertices of an undirected graph, by Hopcroft and Tarjan's
/// lowlink rule, walked iteratively — the Node cap is 256 and a recursive walk
/// would put that depth on the stack.
///
/// Each connected component is walked from its own root, which is what makes
/// the reading per component: a root is an articulation vertex exactly when it
/// has two or more children in its own walk, and a vertex alone in its
/// component has none.
fn articulation(count: usize, near: &[Vec<usize>]) -> Vec<bool> {
    const UNVISITED: usize = usize::MAX;
    let mut depth = vec![UNVISITED; count];
    let mut low = vec![0usize; count];
    let mut parent = vec![UNVISITED; count];
    let mut children = vec![0usize; count];
    let mut cut = vec![false; count];
    let mut next = 0usize;
    let mut work: Vec<(usize, usize)> = Vec::new();
    for root in 0..count {
        if depth[root] != UNVISITED {
            continue;
        }
        depth[root] = next;
        low[root] = next;
        next += 1;
        work.push((root, 0));
        while let Some((at, edge)) = work.pop() {
            if edge < near[at].len() {
                work.push((at, edge + 1));
                let other = near[at][edge];
                if depth[other] == UNVISITED {
                    parent[other] = at;
                    children[at] += 1;
                    depth[other] = next;
                    low[other] = next;
                    next += 1;
                    work.push((other, 0));
                } else if other != parent[at] {
                    low[at] = low[at].min(depth[other]);
                }
                continue;
            }
            let up = parent[at];
            if up != UNVISITED {
                low[up] = low[up].min(low[at]);
                // A non-root vertex is an articulation vertex when one of its
                // children can reach no further back than the vertex itself.
                if parent[up] != UNVISITED && low[at] >= depth[up] {
                    cut[up] = true;
                }
            }
        }
        // A root is an articulation vertex exactly when its own walk gave it
        // two or more children.
        cut[root] = children[root] >= 2;
    }
    cut
}

// ---------------------------------------------------------------------------
// Self-Support, Throughput, Upkeep Mix
// ---------------------------------------------------------------------------

/// **Self-Support** — how much required upkeep is supplied from inside.
///
/// `clamp01((UP_total - NetImport) / UP_total)` with
/// `NetImport = max(0, sum of (in - out))`. When `UP_total = 0` the reading is
/// 1: nothing was required, so nothing was supplied from outside.
fn self_support(upkeep_total: i128, inflow: i128, outflow: i128) -> Reading {
    if upkeep_total == 0 {
        return Reading::at(i64::from(FRAC_ONE));
    }
    let net_import = (inflow - outflow).max(0);
    Reading::at(i64::from(frac_of((upkeep_total - net_import).max(0), upkeep_total)))
}

/// **Throughput** — the identity and magnitude of Charge entering and leaving.
///
/// The magnitudes are the window totals per step; the identity is the itemized
/// record beside them, each list sorted descending by magnitude and ascending by
/// identifier on equal magnitudes. A window with no step has no rate to report
/// and reports zero rather than a division by nothing.
fn throughput(
    structure: &Structure,
    sets: &Sets,
    flows: &[Fx],
    shell_magnitude: &[i128],
    steps: usize,
) -> Throughput {
    if steps == 0 {
        return Throughput::default();
    }
    let per_step = |total: i128| -> Fx {
        let span = steps as i128;
        let rounded = if total >= 0 {
            (total * 2 + span) / (span * 2)
        } else {
            -((-total * 2 + span) / (span * 2))
        };
        rounded as Fx
    };
    let mut into = 0i128;
    let mut out = 0i128;
    let mut routes: Vec<(u32, Fx)> = Vec::new();
    for (place, (route, tail, head)) in structure.routes.iter().enumerate() {
        if !sets.crossing[place] {
            continue;
        }
        let window_flow = i128::from(flows[place]);
        if sets.member[*head] {
            into += window_flow;
        } else if sets.member[*tail] {
            out += window_flow;
        }
        routes.push((*route, per_step(window_flow)));
    }
    let mut shell: Vec<(u32, Fx)> = (0..structure.count())
        .filter(|place| sets.shell[*place])
        .map(|place| (structure.nodes[place], per_step(shell_magnitude[place])))
        .collect();
    routes.sort_by(|first, second| second.1.cmp(&first.1).then(first.0.cmp(&second.0)));
    shell.sort_by(|first, second| second.1.cmp(&first.1).then(first.0.cmp(&second.0)));
    Throughput { in_rate: per_step(into), out_rate: per_step(out), routes, shell }
}

/// **Upkeep Mix** — the boundary, repair, replacement, movement, and reserve
/// shares, each the window total attributed to that purpose over `UP_total`.
///
/// The five are held to summing to exactly one by largest-remainder rounding,
/// which ARCHITECTURE.md locks: each share takes its floor, and the units left
/// over go to the largest remainders, ties to the earlier purpose. Unassigned
/// when `UP_total = 0`.
fn mix_shares(total: i128, mix: &[i128; UPKEEP_PURPOSES]) -> Option<[Frac; UPKEEP_PURPOSES]> {
    if total <= 0 {
        return None;
    }
    let mut shares = [0 as Frac; UPKEEP_PURPOSES];
    let mut remainders: Vec<(i128, usize)> = Vec::with_capacity(UPKEEP_PURPOSES);
    let mut given: i128 = 0;
    for (purpose, held) in mix.iter().enumerate() {
        let scaled = held * i128::from(FRAC_ONE);
        let floor = scaled / total;
        shares[purpose] = floor as Frac;
        given += floor;
        remainders.push((scaled - floor * total, purpose));
    }
    let mut left = i128::from(FRAC_ONE) - given;
    remainders.sort_by(|first, second| second.0.cmp(&first.0).then(first.1.cmp(&second.1)));
    for (_, purpose) in remainders {
        if left <= 0 {
            break;
        }
        shares[purpose] += 1;
        left -= 1;
    }
    Some(shares)
}

// ---------------------------------------------------------------------------
// Reach
// ---------------------------------------------------------------------------

/// **Reach** — the distance crossed by a closed route.
///
/// On the flow-positive graph and its strongly connected components — Cut
/// Impact's own rule — Reach is the largest `d(a, b)` over pairs where a is a
/// member and b is any Node in the same component as a, that component
/// containing a cycle. It is 0 when no member lies in a component with a cycle.
fn reach(structure: &Structure, sets: &Sets, flows: &[Fx]) -> Reading {
    let present = vec![true; structure.routes.len()];
    let (component, cyclic) = crate::perturb::flow_components(structure, flows, &present);
    let count = structure.count();
    let mut largest: Fx = 0;
    let mut found = false;
    for a in 0..count {
        if !sets.member[a] || component[a] >= cyclic.len() || !cyclic[component[a]] {
            continue;
        }
        found = true;
        for b in 0..count {
            if component[b] != component[a] {
                continue;
            }
            let (first, first_layer) = structure.placed[a];
            let (second, second_layer) = structure.placed[b];
            let held = crate::fx::distance(first, first_layer, second, second_layer);
            largest = largest.max(held);
        }
    }
    Reading::at(if found { i64::from(largest) } else { 0 })
}

// ---------------------------------------------------------------------------
// Input Resolution
// ---------------------------------------------------------------------------

/// **Input Resolution** — how many distinguishable surround states the window
/// held.
///
/// The signature at a window step is `f(r, t)` for every crossing Route in
/// ascending Route order, then `e(m, t)` for every shell member in ascending
/// identifier order. Each position is quantized on its own window range with
/// tau as the bin width, and the reading is the count of distinct quantized
/// vectors. Unassigned when the signature has no positions — no crossing Route
/// and an empty shell.
fn input_resolution(
    window: &Window<'_>,
    structure: &Structure,
    sets: &Sets,
    tau: Frac,
) -> Reading {
    let routes: Vec<usize> =
        (0..structure.routes.len()).filter(|place| sets.crossing[*place]).collect();
    let members: Vec<usize> =
        (0..structure.count()).filter(|place| sets.shell[*place]).collect();
    let width = routes.len() + members.len();
    if width == 0 {
        return Reading::none(NO_SIGNATURE);
    }
    if window.len() == 0 {
        return Reading::none(WINDOW_TOO_SHORT);
    }
    // The raw signature, one row per window step.
    let mut rows: Vec<Vec<Fx>> = Vec::with_capacity(window.len());
    for step in &window.steps {
        let mut row = vec![0 as Fx; width];
        let mut place = 0;
        for (route, moved) in &step.records.f {
            while place < structure.routes.len() && structure.routes[place].0 < *route {
                place += 1;
            }
            if place < structure.routes.len() && structure.routes[place].0 == *route {
                if let Some(slot) = routes.iter().position(|held| *held == place) {
                    row[slot] = *moved;
                }
            }
        }
        rank::walk(&step.records.e, &structure.nodes, |held, value| {
            if let Some(slot) = members.iter().position(|at| *at == held) {
                row[routes.len() + slot] = value;
            }
        });
        rows.push(row);
    }
    // Each position quantized on its own window range: the level is 0 where the
    // position never moved, and otherwise the bin the value falls in, with tau
    // times the range as the bin width. The division is taken on the scaled
    // numerator so nothing is lost to a floored bin width.
    let mut levels: Vec<Vec<i64>> = Vec::with_capacity(rows.len());
    let mut low = vec![Fx::MAX; width];
    let mut high = vec![Fx::MIN; width];
    for row in &rows {
        for (slot, value) in row.iter().enumerate() {
            low[slot] = low[slot].min(*value);
            high[slot] = high[slot].max(*value);
        }
    }
    for row in &rows {
        let mut held = Vec::with_capacity(width);
        for (slot, value) in row.iter().enumerate() {
            let span = i128::from(high[slot] - low[slot]);
            if span == 0 {
                held.push(0);
                continue;
            }
            let bin = span * i128::from(tau);
            let level = (i128::from(*value - low[slot]) * i128::from(FRAC_ONE)) / bin;
            held.push(level as i64);
        }
        levels.push(held);
    }
    let mut distinct: Vec<Vec<i64>> = Vec::new();
    for held in levels {
        if !distinct.contains(&held) {
            distinct.push(held);
        }
    }
    Reading::at(distinct.len() as i64)
}

// ---------------------------------------------------------------------------
// Horizon
// ---------------------------------------------------------------------------

/// **Horizon** — retained or anticipated time.
///
/// The retained span is the largest lag L in `1 ..= w - 2` whose lag agreement
/// — the agreement of the `q_I` direction symbol at step t with the symbol at
/// step `t - L`, over the steps where both exist — reaches `1/2 + tau`, and 0
/// when none qualifies or `w < 3`. Horizon is the larger of that span and the
/// declared Forecast depth. It is assigned whenever `w_eff >= 1`: a window too
/// short to hold a lag has a retained span of 0, not an unassigned one.
fn horizon(series: &[Fx], now: &crate::state::FieldState, tau: Frac) -> Reading {
    // The Field's declared Forecast depth is the controlled Form's, which is
    // where authored content puts it; a Field standing with no controlled Form
    // declares none.
    let declared = now
        .forms
        .iter()
        .find(|form| form.controlled)
        .map_or(0u16, |form| form.forecast_depth);
    if series.is_empty() {
        return Reading::none(WINDOW_TOO_SHORT);
    }
    let held = symbols(series, tau);
    let threshold = FRAC_ONE / 2 + tau;
    let mut retained = 0u16;
    if series.len() >= 3 {
        for lag in 1..=(series.len() - 2) {
            if lag >= held.len() {
                break;
            }
            let compared = held.len() - lag;
            let same = (lag..held.len()).filter(|at| held[*at] == held[at - lag]).count();
            if frac_of(same as i128, compared as i128) >= threshold {
                retained = lag as u16;
            }
        }
    }
    Reading::at(i64::from(retained.max(declared)))
}

// ---------------------------------------------------------------------------
// Source Trace
// ---------------------------------------------------------------------------

/// **Source Trace** — contribution from earlier state versus recurring surround.
///
/// The retained share of the Charge that was inside at the window start, tracked
/// step by step with stored Charge treated as fully mixed:
/// `old(t) = old(t - 1) * max(0, 1 - REM(t) / q_I(t - 1))`, and 0 wherever
/// `q_I(t - 1)` is. The reading is `old(t0) / q_I(t0)`, and it is unassigned
/// when the inside ends the window holding nothing.
fn source_trace(
    window: &Window<'_>,
    structure: &Structure,
    sets: &Sets,
    series: &[Fx],
    removal: &[Fx],
) -> Reading {
    if series.is_empty() {
        return Reading::none(WINDOW_TOO_SHORT);
    }
    let opening: Fx = window
        .start
        .ports
        .iter()
        .filter(|port| {
            structure
                .nodes
                .binary_search(&port.node)
                .map(|place| sets.member[place])
                .unwrap_or(false)
        })
        .map(|port| port.q)
        .sum();
    let mut old = i128::from(opening);
    let mut previous = i128::from(opening);
    for (index, held) in series.iter().enumerate() {
        if previous > 0 {
            let kept = (previous - i128::from(removal[index])).max(0);
            // Round to nearest with halves upward, the one rounding rule.
            old = (old * kept * 2 + previous) / (previous * 2);
        } else {
            old = 0;
        }
        previous = i128::from(*held);
    }
    let last = i128::from(series[series.len() - 1]);
    if last <= 0 {
        return Reading::none(NO_STORED_CHARGE);
    }
    Reading::at(i64::from(frac_of(old.min(last), last)))
}

// ---------------------------------------------------------------------------
// Instruction Separation
// ---------------------------------------------------------------------------

/// **Instruction Separation** — whether a portable description can rebuild the
/// arrangement.
///
/// The View's descriptor is extracted — each member's kind, its stored Charge at
/// the window start, and the internal Routes among members with their endpoints
/// — and instantiated in a cleared context: the members and their internal
/// Routes and nothing else, with every crossing Route and every shell member's
/// exogenous term driven from the recorded schedule while everything else runs
/// under the step function. The sample is the agreement of the rebuilt inside's
/// stored-Charge symbol series with the recorded one, both under the recorded
/// series' margin. The value is the median of the eight, and the confidence
/// range is [smallest, largest]. Unassigned when `w < 2`.
fn instruction_separation(
    window: &Window<'_>,
    structure: &Structure,
    sets: &Sets,
    sigma: &RngState,
    position: u8,
    tau: Frac,
) -> PrivilegeValue {
    if window.len() < 2 {
        return PrivilegeValue::unassigned(WINDOW_TOO_SHORT);
    }
    let recorded = Recorded::of(window, structure, sets, tau);
    let rebuilt = descriptor_context(window, structure, sets);
    // The schedule the cleared context is driven from: every crossing Route's
    // recorded flow, and every shell member's recorded exogenous term.
    let crossings: Vec<(u32, usize, usize)> = structure
        .routes
        .iter()
        .enumerate()
        .filter(|(place, _)| sets.crossing[*place])
        .map(|(_, held)| *held)
        .collect();
    let mut samples: Vec<Frac> = Vec::new();
    for sample in 1..=SAMPLES {
        let stream = sigma.split(&[
            Part::Name("candidate"),
            Part::Number(i64::from(position)),
            Part::Name(STREAM_INSTRUCTION_SEPARATION),
            Part::Number(sample as i64),
        ]);
        let series =
            crate::perturb::replay_series(window, &rebuilt, stream, |index, state, records| {
                let step = window.steps[index];
                // Every shell member's own term, put back as recorded.
                for (node, value) in &records.e {
                    if structure
                        .nodes
                        .binary_search(node)
                        .map(|place| sets.shell[place])
                        .unwrap_or(false)
                    {
                        crate::perturb::move_stored(state, *node, -*value);
                    }
                }
                rank::walk(&step.records.e, &structure.nodes, |place, value| {
                    if sets.shell[place] {
                        crate::perturb::move_stored(state, structure.nodes[place], value);
                    }
                });
                // Every crossing Route's recorded flow, delivered to the member
                // end it moved to or taken from the member end it moved from.
                for (route, tail, head) in &crossings {
                    let moved = step
                        .records
                        .f
                        .iter()
                        .find(|(held, _)| held == route)
                        .map_or(0, |(_, value)| *value);
                    if moved == 0 {
                        continue;
                    }
                    if sets.member[*head] {
                        crate::perturb::move_stored(state, structure.nodes[*head], moved);
                    }
                    if sets.member[*tail] {
                        crate::perturb::move_stored(state, structure.nodes[*tail], -moved);
                    }
                }
            });
        let Some(agreed) =
            agreement(&recorded.symbols, &symbols_under(&series, recorded.margin, tau))
        else {
            return PrivilegeValue::unassigned(WINDOW_TOO_SHORT);
        };
        samples.push(agreed);
    }
    samples.sort_unstable();
    let low = samples[0];
    let high = samples[samples.len() - 1];
    PrivilegeValue::assigned(median(&samples), low, high, SAMPLES as u16)
}

/// The cleared context a descriptor is instantiated in: the members standing
/// where they stood with the stored Charge the window opened on, the internal
/// Routes among them, and nothing else at all — no non-member, no crossing
/// Route, and no current.
///
/// The layers stay, because a layer is a declaration of the context rather than
/// a member of it and the step function reads one for every Node it advances.
/// Nothing crosses the rebuilt inside's boundary, so nothing leaks: with every
/// Node a member, no member is exposed to a non-member, which is the honest
/// reading of a context with nothing outside it.
fn descriptor_context(window: &Window<'_>, structure: &Structure, sets: &Sets) -> Prepared {
    let mut start = window.start.clone();
    let member_of = |node: u32| {
        structure.nodes.binary_search(&node).map(|place| sets.member[place]).unwrap_or(false)
    };
    start.ports.retain(|port| member_of(port.node));
    start.routes.retain(|route| member_of(route.tail) && member_of(route.head));
    start.forms.retain(|form| member_of(form.node));
    start.currents.clear();
    for layer in start.layers.iter_mut() {
        layer.current_ids.clear();
        layer.port_ids.retain(|node| member_of(*node));
    }
    let members = sets.members.clone();
    // This is a physically cleared synthetic context, not a changed View: its
    // material compartment is explicitly the set of remaining Components.
    start.physical_compartment.members = members.clone();
    let cache = StepCache::of(&start);
    Prepared { start, cache, members }
}

// ---------------------------------------------------------------------------
// Turnover Tolerance
// ---------------------------------------------------------------------------

/// **Turnover Tolerance** — how much component replacement preserves continuity.
///
/// For `j = 1..8` the replacement fraction is `phi_j = j / 8` and the replaced
/// count is `k_j = ceil(phi_j * |I|)`; the replaced members are drawn by the
/// shuffle rule under the kind's `"draw"` stream, replaced exactly as Component
/// substitution replaces one, and the window is replayed under the `"run"`
/// stream. The reading is the largest `phi_j` whose agreement reaches
/// `1 - 2 * tau`, and 0 when none does. Every `(phi_j, a_j)` pair is recorded.
/// Unassigned when `w < 2`.
fn turnover_tolerance(
    window: &Window<'_>,
    structure: &Structure,
    sets: &Sets,
    sigma: &RngState,
    position: u8,
    tau: Frac,
) -> Turnover {
    if window.len() < 2 {
        return Turnover { value: None, reason: Some(WINDOW_TOO_SHORT), pairs: None };
    }
    let recorded = Recorded::of(window, structure, sets, tau);
    let members = sets.members.len();
    let threshold = FRAC_ONE - 2 * tau;
    let mut pairs: Vec<(Frac, Frac)> = Vec::with_capacity(SAMPLES);
    let mut best: Frac = 0;
    for sample in 1..=SAMPLES {
        let phi = (FRAC_ONE * sample as Frac) / SAMPLES as Frac;
        // `k_j = ceil(phi_j * |I|)`, taken on the exact fraction `j / 8` rather
        // than on its rounded raw form, so the ladder is the framework's own.
        let replaced_count = (sample * members).div_ceil(SAMPLES);
        let mut draw = sigma.split(&[
            Part::Name("candidate"),
            Part::Number(i64::from(position)),
            Part::Name(STREAM_TURNOVER),
            Part::Number(sample as i64),
            Part::Name("draw"),
        ]);
        let drawn = shuffled(&sets.members, &mut draw, replaced_count);
        let prepared = crate::perturb::Prepared::substituting(window, sets, &drawn);
        let run = sigma.split(&[
            Part::Name("candidate"),
            Part::Number(i64::from(position)),
            Part::Name(STREAM_TURNOVER),
            Part::Number(sample as i64),
            Part::Name("run"),
        ]);
        let series = crate::perturb::replay_series(window, &prepared, run, |_, _, _| {});
        let agreed = agreement(&recorded.symbols, &symbols_under(&series, recorded.margin, tau))
            .unwrap_or(0);
        pairs.push((phi, agreed));
        if agreed >= threshold {
            best = best.max(phi);
        }
    }
    Turnover { value: Some(best), reason: None, pairs: Some(pairs) }
}

/// FRAMEWORK.md's shuffle rule, and the first `k` it leaves: the items in
/// ascending identifier order, then `i` from `L - 1` down to 1 exchanging
/// `a[i]` with `a[draw(stream, i + 1)]`.
fn shuffled(items: &[u32], stream: &mut RngState, take: usize) -> Vec<u32> {
    let mut held = items.to_vec();
    held.sort_unstable();
    let length = held.len();
    for index in (1..length).rev() {
        let other = stream.draw(index as u64 + 1) as usize;
        held.swap(index, other);
    }
    held.truncate(take.min(length));
    held.sort_unstable();
    held
}
