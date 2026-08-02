//! The bounded queue of proposed changes, and the transaction around it.
//!
//! The V2 paid-plan union has four
//! variants, the payload of each, the preconditions each is validated against,
//! the cost of one entry, the queue depth of 6 entries, the conflict rules, and
//! the caps rule: a locked cap that would be crossed is the `capacity` error,
//! so a seventh entry is refused with that envelope rather than dropped and
//! rather than pushing an earlier entry out.
//!
//! Two readings of the state matter here and are kept apart by name. The **base
//! state** is the causal Field as it stands; the **projection** is the base
//! state with every earlier queued entry applied in
//! order, which is what an entry is validated against at queue time. A commit
//! rebuilds the projection from the base state and revalidates every entry into
//! it, so the transaction is all-or-nothing by construction: nothing reaches
//! the run until every entry has passed, and a refusal at any position leaves
//! the base state untouched.
//!
//! One entry costs one Impulse and nothing else spends Impulse at all, so the
//! cost a queue displays and the cost a commit spends are the same arithmetic
//! on the same count — the queue length. There is no second cost model to drift
//! from the first.

use crate::fault::{Code, Fault};
use crate::field::{self, RouteState, ROUTES_PER_RUN};
use crate::fx::ONE_UNIT;
use crate::json::{Json, Obj};
use crate::read;
use crate::state::{FieldState, Fx};

/// How many entries the queue holds at once.
pub const PLAN_QUEUE_DEPTH: usize = 6;

/// What one queued entry costs, in Impulse. Locked at one per entry, and
/// nothing else spends Impulse at all.
pub const PLAN_ENTRY_COST: u8 = 1;

/// The per-step capacity a Route opens at when a `connect` forms it, for a
/// Field standing no controlled Form to author one: 32 units, the capacity the
/// opening chapter's own circuit Routes carry.
///
/// The `connect` payload carries two Node identifiers and nothing else, so the
/// capacity of a Route the player forms is not in the command — it is a
/// property of the Field the change lands in. The named extension point is
/// taken: the controlled Form's authored `route_capacity` is what a Route it
/// forms carries, exactly as `route_reach` is authored per Form, and nothing
/// else about `connect` moves with it. This constant is what stands when no
/// Form does, which is a Field the `connect` precondition already refuses.
pub const CONNECTED_ROUTE_CAPACITY: Fx = 32 * ONE_UNIT;

/// Which end of a Route a redirect moves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RouteEnd {
    Tail,
    Head,
}

impl RouteEnd {
    pub fn name(self) -> &'static str {
        match self {
            RouteEnd::Tail => "tail",
            RouteEnd::Head => "head",
        }
    }
}

/// One proposed change. `op` is the tag.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlanCommand {
    Connect { from: u32, to: u32 },
    Redirect { route: u32, end: RouteEnd, to: u32 },
    Cut { route: u32 },
    ReshapeCompartment { members: Vec<u32> },
}

impl PlanCommand {
    /// Reads one entry out of a command body.
    ///
    /// The tag decides the shape, and every variant is held to exactly its own
    /// declared keys: a body carrying a key another variant declares is a body
    /// the sender got wrong, not a body to read the intersection of.
    pub fn read(value: &Json) -> Result<Self, Fault> {
        if !value.is_map() {
            return Err(Fault::field("plan"));
        }
        let op = read::text(value, "op")?;
        match op {
            "connect" => {
                read::exact_keys(value, "plan", &["from", "op", "to"])?;
                Ok(PlanCommand::Connect {
                    from: node_id(value, "from")?,
                    to: node_id(value, "to")?,
                })
            }
            "redirect" => {
                read::exact_keys(value, "plan", &["end", "op", "route", "to"])?;
                let end = match read::one_of(value, "end", &["tail", "head"])? {
                    0 => RouteEnd::Tail,
                    _ => RouteEnd::Head,
                };
                Ok(PlanCommand::Redirect {
                    route: node_id(value, "route")?,
                    end,
                    to: node_id(value, "to")?,
                })
            }
            "cut" => {
                read::exact_keys(value, "plan", &["op", "route"])?;
                Ok(PlanCommand::Cut { route: node_id(value, "route")? })
            }
            "reshape_compartment" => {
                read::exact_keys(value, "plan", &["members", "op"])?;
                // Ascending with no repeats is a precondition of the variant
                // rather than a shape rule, but `read::ids` holds both here
                // because a member list that is neither is not a set at all.
                let members =
                    read::ids(value, "members", field::NODES_PER_RUN, i64::from(u32::MAX))?;
                if members.is_empty() {
                    return Err(Fault::field("members"));
                }
                Ok(PlanCommand::ReshapeCompartment { members })
            }
            _ => Err(Fault::field("op")),
        }
    }

    /// One entry as the queue state carries it: the tagged union, `op` first
    /// among its keys wherever the canonical order puts it.
    pub fn written(&self) -> String {
        let mut out = String::new();
        let mut object = Obj::new(&mut out);
        match self {
            PlanCommand::Connect { from, to } => {
                object.int("from", i64::from(*from));
                object.text("op", "connect");
                object.int("to", i64::from(*to));
            }
            PlanCommand::Redirect { route, end, to } => {
                object.text("end", end.name());
                object.text("op", "redirect");
                object.int("route", i64::from(*route));
                object.int("to", i64::from(*to));
            }
            PlanCommand::Cut { route } => {
                object.text("op", "cut");
                object.int("route", i64::from(*route));
            }
            PlanCommand::ReshapeCompartment { members } => {
                let written: Vec<String> =
                    members.iter().map(|member| member.to_string()).collect();
                object.raw("members", &format!("[{}]", written.join(",")));
                object.text("op", "reshape_compartment");
            }
        }
        object.end();
        out
    }
}

fn node_id(value: &Json, key: &str) -> Result<u32, Fault> {
    Ok(read::int(value, key, 1, i64::from(u32::MAX))? as u32)
}

/// Why one entry was refused: the envelope its code names, and the machine
/// reason the refusal carries.
///
/// A commit's refusal is locked as `detail: { "position": n, "reason": … }`, so
/// the reason travels as its own value rather than inside a detail object that
/// would then have to be taken apart again. The queue-time refusal carries the
/// same reason without a position, because a queued entry is the only entry
/// there is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Refusal {
    pub code: Code,
    pub reason: &'static str,
}

impl Refusal {
    fn of(code: Code, reason: &'static str) -> Self {
        Refusal { code, reason }
    }

    pub(crate) fn missing(reason: &'static str) -> Self {
        Refusal::of(Code::NotFound, reason)
    }

    pub(crate) fn invalid(reason: &'static str) -> Self {
        Refusal::of(Code::Validation, reason)
    }

    /// The refusal a queued entry answers with.
    pub fn fault(self) -> Fault {
        let mut detail = String::new();
        let mut object = Obj::new(&mut detail);
        object.text("reason", self.reason);
        object.end();
        Fault::detailed(self.code, detail)
    }

    /// The refusal a commit answers with, naming the position that failed.
    pub fn positioned(self, position: usize) -> Fault {
        let mut detail = String::new();
        let mut object = Obj::new(&mut detail);
        object.int("position", position as i64);
        object.text("reason", self.reason);
        object.end();
        Fault::detailed(self.code, detail)
    }
}

/// The base state with some number of queued entries applied: what an entry is
/// validated against, and what a commit builds before it installs anything.
#[derive(Clone, Debug)]
pub struct Projection {
    pub field: FieldState,
}

impl Projection {
    /// The projection that has nothing applied to it yet.
    pub fn of(field: &FieldState) -> Self {
        Projection { field: field.clone() }
    }

    fn route(&self, route: u32) -> Option<&RouteState> {
        self.field.routes.iter().find(|held| held.route == route)
    }

    fn holds_node(&self, node: u32) -> bool {
        self.field.ports.iter().any(|port| port.node == node)
    }

    /// Whether a Route already stands from one Node to another, ignoring one
    /// Route by identifier — which is how a redirect asks whether the Route it
    /// is moving would land on top of another one rather than on top of itself.
    fn holds_link(&self, tail: u32, head: u32, besides: Option<u32>) -> bool {
        self.field
            .routes
            .iter()
            .any(|held| held.tail == tail && held.head == head && Some(held.route) != besides)
    }

    /// How far the controlled Form may reach to form or move a Route.
    fn route_reach(&self) -> Option<Fx> {
        self.field.forms.iter().find(|form| form.controlled).map(|form| form.route_reach)
    }

    /// Whether two Nodes stand inside the controlled Form's reach of each other.
    fn within_reach(&self, from: u32, to: u32) -> Result<bool, Refusal> {
        let reach = self.route_reach().ok_or_else(|| Refusal::invalid("form"))?;
        let distance = field::nodes_distance(&self.field, from, to)
            .ok_or_else(|| Refusal::missing("node"))?;
        Ok(distance <= reach)
    }
}

/// Validates one entry against a projection, exactly as the locked table has it.
///
/// The same function runs at queue time and again at commit, on the projection
/// each is built from, because "revalidate every entry in order from the base
/// state" is the same question asked of a projection rebuilt from scratch. One
/// rule set, two callers: a precondition that held when the entry was queued
/// and would not hold at the commit refuses the whole commit.
pub fn check(
    plan: &PlanCommand,
    projected: &Projection,
) -> Result<(), Refusal> {
    match plan {
        PlanCommand::Connect { from, to } => {
            if !projected.holds_node(*from) || !projected.holds_node(*to) {
                return Err(Refusal::missing("node"));
            }
            if from == to {
                return Err(Refusal::invalid("self_link"));
            }
            if projected.holds_link(*from, *to, None) {
                return Err(Refusal::invalid("duplicate"));
            }
            if projected.field.routes.len() >= ROUTES_PER_RUN {
                return Err(Refusal::of(Code::Capacity, "routes_per_run"));
            }
            if !projected.within_reach(*from, *to)? {
                return Err(Refusal::invalid("reach"));
            }
            Ok(())
        }
        PlanCommand::Redirect { route, end, to } => {
            let Some(held) = projected.route(*route) else {
                return Err(Refusal::missing("route"));
            };
            if !projected.holds_node(*to) {
                return Err(Refusal::missing("node"));
            }
            let (tail, head) = match end {
                RouteEnd::Tail => (*to, held.head),
                RouteEnd::Head => (held.tail, *to),
            };
            if projected.holds_link(tail, head, Some(*route)) {
                return Err(Refusal::invalid("duplicate"));
            }
            if !projected.within_reach(tail, head)? {
                return Err(Refusal::invalid("reach"));
            }
            Ok(())
        }
        PlanCommand::Cut { route } => match projected.route(*route) {
            Some(_) => Ok(()),
            None => Err(Refusal::missing("route")),
        },
        PlanCommand::ReshapeCompartment { members } => {
            if members.is_empty() || !read::ascending(members) {
                return Err(Refusal::invalid("members"));
            }
            // Intake is the framework's: the proposed set is replaced by its
            // intersection with the current Node set, and an empty result is
            // refused. A member naming a Node that has vanished is dropped by
            // intake rather than refusing the whole entry.
            if intake(members, &projected.field).is_empty() {
                return Err(Refusal::invalid("members"));
            }
            Ok(())
        }
    }
}

/// Applies one entry to a projection. Only an entry that has just passed
/// [`check`] against this same projection reaches here.
pub fn apply(
    plan: &PlanCommand,
    projected: &mut Projection,
) -> Result<(), Refusal> {
    match plan {
        PlanCommand::Connect { from, to } => {
            // Identifiers are unique within a Run and never reused, so the
            // formed Route takes the next one and the list stays ascending by
            // construction. `formed_step` is the completed step it was formed
            // at: a commit runs only while the run is still, and a still run
            // runs no step, so the first pass a formed Route takes part in is
            // the step after the commit.
            let route = projected.field.next_route_id;
            projected.field.next_route_id += 1;
            // The capacity is the controlled Form's own, which is the one
            // per-Form variation this rule admits.
            let capacity = projected
                .field
                .forms
                .iter()
                .find(|form| form.controlled)
                .map_or(CONNECTED_ROUTE_CAPACITY, |form| form.route_capacity);
            projected.field.routes.push(RouteState {
                route,
                tail: *from,
                head: *to,
                capacity,
                flow: 0,
                formed_step: projected.field.step,
            });
            Ok(())
        }
        PlanCommand::Redirect { route, end, to } => {
            let Some(held) =
                projected.field.routes.iter_mut().find(|held| held.route == *route)
            else {
                return Err(Refusal::missing("route"));
            };
            // A redirect moves one end and nothing else: the Route keeps its
            // identifier, its capacity, the flow it recorded for the completed
            // step, and the step it was formed at, because it is the same Route
            // standing somewhere else.
            match end {
                RouteEnd::Tail => held.tail = *to,
                RouteEnd::Head => held.head = *to,
            }
            Ok(())
        }
        PlanCommand::Cut { route } => {
            let before = projected.field.routes.len();
            projected.field.routes.retain(|held| held.route != *route);
            if projected.field.routes.len() == before {
                return Err(Refusal::missing("route"));
            }
            // `next_route_id` does not move: identifiers are never reused, so a
            // cut leaves no gap to fill.
            Ok(())
        }
        PlanCommand::ReshapeCompartment { members } => {
            // The material compartment is replaced by the member set after
            // intake. Observation View state is not part of this projection.
            let taken = intake(members, &projected.field);
            if taken.is_empty() {
                return Err(Refusal::invalid("members"));
            }
            projected.field.physical_compartment.members = taken;
            Ok(())
        }
    }
}

/// Compartment intake: the proposed material set intersected with the current
/// Node set, ascending.
fn intake(members: &[u32], field: &FieldState) -> Vec<u32> {
    members
        .iter()
        .copied()
        .filter(|member| field.ports.iter().any(|port| port.node == *member))
        .collect()
}

/// One queued entry, with the two keys the locked conflict rules compare.
///
/// Both keys are resolved when the queue is built rather than when it is read,
/// because both are readings of the projection the entry stands in: the Route a
/// `connect` would take is the identifier the Field hands out at that point in
/// the queue, and the endpoints a `redirect` proposes are the pair the Route
/// would stand at after the entries before it. Reading them later — from a
/// frame, from a response — would be reading them against a different state.
#[derive(Clone, Debug)]
pub struct QueuedPlan {
    pub plan: PlanCommand,
    /// The Route the entry names, or the identifier a `connect` would take.
    /// None for the two variants that name no Route at all.
    pub route: Option<u32>,
    /// The endpoints the entry proposes, tail then head, and none for an entry
    /// that proposes no link.
    pub pair: Option<(u32, u32)>,
    /// Whether the entry would take the Route it names away.
    pub cut: bool,
}

/// The queue itself: at most `PLAN_QUEUE_DEPTH` entries, in the order they
/// were queued.
#[derive(Clone, Debug, Default)]
pub struct PlanQueue {
    entries: Vec<QueuedPlan>,
    /// The physical member set the queue proposes, and none while no entry
    /// reshapes the material compartment. Resolved beside the entries' own
    /// keys because it is a reading of the whole projection.
    proposed_compartment_members: Option<Vec<u32>>,
}

impl PlanQueue {
    pub fn new() -> Self {
        PlanQueue { entries: Vec::new(), proposed_compartment_members: None }
    }

    /// The material members the queue would leave, and none while it proposes
    /// no compartment reshape.
    pub fn proposed_compartment_members(&self) -> Option<&[u32]> {
        self.proposed_compartment_members.as_deref()
    }

    /// Queues one entry. A full queue refuses the entry with the `capacity`
    /// envelope: nothing is dropped and nothing is replaced.
    pub fn push(&mut self, entry: PlanCommand) -> Result<usize, Fault> {
        if self.entries.len() >= PLAN_QUEUE_DEPTH {
            let mut detail = String::new();
            let mut object = Obj::new(&mut detail);
            object.int("cap", PLAN_QUEUE_DEPTH as i64);
            object.text("quantity", "plan_queue_depth");
            object.end();
            return Err(Fault::detailed(Code::Capacity, detail));
        }
        self.entries.push(QueuedPlan { plan: entry, route: None, pair: None, cut: false });
        Ok(self.entries.len())
    }

    /// Removes the most recent entry. An empty queue succeeds and changes
    /// nothing.
    pub fn undo(&mut self) -> Option<PlanCommand> {
        self.entries.pop().map(|held| held.plan)
    }

    /// Empties the queue, as a commit and every state restore do.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.proposed_compartment_members = None;
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The queued entries, in the order they were queued.
    pub fn queued(&self) -> &[QueuedPlan] {
        &self.entries
    }

    /// The proposed changes alone, for a caller that reads the union rather
    /// than the keys resolved beside it.
    pub fn entries(&self) -> Vec<PlanCommand> {
        self.entries.iter().map(|held| held.plan.clone()).collect()
    }

    /// The projection the queue stands on: the base state with every entry
    /// applied in order.
    pub fn projected(
        &self,
        field: &FieldState,
    ) -> Projection {
        let mut held = Projection::of(field);
        for entry in &self.entries {
            // An entry in the queue passed this same apply when it was queued,
            // and nothing moves the base state while the run is still, so a
            // refusal here is a defect rather than a fault in any input.
            let _ = apply(&entry.plan, &mut held);
        }
        held
    }

    /// Resolves every entry's conflict keys against the base state.
    ///
    /// Called whenever the queue changes, so the keys are never stale: an undo
    /// that removes a `connect` moves the identifier every later `connect`
    /// would take, and an entry's proposed pair is read in the projection it
    /// actually stands in.
    pub fn rebuild(
        &mut self,
        field: &FieldState,
    ) {
        let mut held = Projection::of(field);
        for place in 0..self.entries.len() {
            let plan = self.entries[place].plan.clone();
            let (route, pair, cut) = match &plan {
                PlanCommand::Connect { from, to } => {
                    (Some(held.field.next_route_id), Some((*from, *to)), false)
                }
                PlanCommand::Redirect { route, end, to } => {
                    let standing = held.route(*route).map(|held| (held.tail, held.head));
                    let pair = standing.map(|(tail, head)| match end {
                        RouteEnd::Tail => (*to, head),
                        RouteEnd::Head => (tail, *to),
                    });
                    (Some(*route), pair, false)
                }
                PlanCommand::Cut { route } => (Some(*route), None, true),
                PlanCommand::ReshapeCompartment { .. } => (None, None, false),
            };
            self.entries[place].route = route;
            self.entries[place].pair = pair;
            self.entries[place].cut = cut;
            let _ = apply(&plan, &mut held);
        }
        let proposes_a_compartment = self
            .entries
            .iter()
            .any(|entry| matches!(entry.plan, PlanCommand::ReshapeCompartment { .. }));
        self.proposed_compartment_members = proposes_a_compartment
            .then_some(held.field.physical_compartment.members);
    }

    /// Which entries stand in conflict with another entry of the queue.
    ///
    /// The locked inventory is two rules: two entries that touch the same Route,
    /// and two entries that propose the same endpoint pair. Both flag both
    /// entries, and neither invalidates anything by itself — a conflict informs
    /// the display, and the transaction is decided by the preconditions.
    ///
    /// A `connect` cannot collide with a standing Route by identifier, because
    /// the identifier it would take is one the Run has never handed out; what it
    /// can collide with is another entry proposing the same link, which is the
    /// second rule and is why the second rule is not the first one restated.
    pub fn conflicts(&self) -> Vec<bool> {
        let mut flagged = vec![false; self.entries.len()];
        for first in 0..self.entries.len() {
            for second in (first + 1)..self.entries.len() {
                let a = &self.entries[first];
                let b = &self.entries[second];
                let same_route = a.route.is_some() && a.route == b.route;
                let same_pair = a.pair.is_some() && a.pair == b.pair;
                if same_route || same_pair {
                    flagged[first] = true;
                    flagged[second] = true;
                }
            }
        }
        flagged
    }

    /// What the whole queue costs, one Impulse per entry.
    pub fn cost_total(&self) -> u8 {
        (self.entries.len() as u8) * PLAN_ENTRY_COST
    }

    /// The queue state every queued-change response carries, and the one thing
    /// the tray displays: the entries in the order they were queued, what each
    /// costs, what the queue costs together, and the Impulse standing before
    /// and after it.
    ///
    /// The total is the sum of the per-entry costs and the commit spends that
    /// same total, so what the tray predicts and what a commit takes are one
    /// number arrived at once.
    pub fn written(&self, impulse: u8) -> String {
        let cost_total = self.cost_total();
        let conflicts = self.conflicts();
        let entries: Vec<String> = self
            .entries
            .iter()
            .enumerate()
            .map(|(position, entry)| {
                let mut out = String::new();
                let mut object = Obj::new(&mut out);
                object.bool("conflict", conflicts[position]);
                object.int("cost", i64::from(PLAN_ENTRY_COST));
                object.raw("plan", &entry.plan.written());
                object.int("position", position as i64);
                object.end();
                out
            })
            .collect();
        let mut out = String::new();
        let mut object = Obj::new(&mut out);
        object.int("cost_total", i64::from(cost_total));
        object.raw("entries", &format!("[{}]", entries.join(",")));
        object.int("impulse", i64::from(impulse));
        object.int("impulse_after", i64::from(impulse) - i64::from(cost_total));
        object.end();
        out
    }
}
