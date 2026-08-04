# What Is Life 2 — Form and Reality-of-Play Model

> Automation supersession: direct Form steering, manual Coupling input, and
> campaign minute-to-minute play below describe the implemented legacy runtime.
> D-019 through D-022 and `AUTOMATION_AND_CONTRACTS.md` own the target product:
> Forms are programmable mobile Components operated by frozen local policies.

Status: design authority for resolving the Form and resource model  
Date: 2026-08-02

## Decisive answer

Yes: the Form is the bright object the player steers.

It is a mobile commissioning chassis and one Component of the system. It is
not the entire generator, not the physical compartment, and not the thing that
is ultimately being claimed to be autonomous.

Two related objects must remain separate:

```text
GeneratorSpec g
= frozen local rules
+ explicitly encoded Component and topology constraints
+ declared addressed inputs and internal feedback policy

EmbodiedState S_t
= one or more Forms
+ current Components and Directed Routes
+ physical-compartment membership and leakage state
+ positions, stored resource, material, gates, and repair intermediates
```

During commissioning, the player pilots a Form to gather usable resource,
activate Components, place or connect structure, and diagnose failure. Every
player-authored topology and exact initial placement is either encoded in
`GeneratorSpec` or disclosed as initial side information. During hands-off
validation, direct steering, Pulses, and manual repair are disabled. A human or
adaptive external controller is a separate controller channel whose sensing,
memory, communication, and actuation must be represented; storing only its
action stream or hash does not account for that channel.

This resolves the central ambiguity in the mock-ups: the white polyhedron is a
thing in the world that moves. The luminous network around it is the larger
constructed system.

## The objects and their jobs

| Object | Literal meaning | Player interaction | Causal? |
|---|---|---|---:|
| Form | Steerable mobile chassis; also one Component | Steer, change depth, charge and release a coupling Pulse, use one chassis-specific ability | Yes |
| GeneratorSpec | Immutable encoded design and frozen local policy | Freeze, validate, archive, instantiate | Defines causal rules |
| Embodied generator state | Changing physical realization of one specification | Commission, operate, measure, disturb | Yes |
| Component | Discrete site with a type, operating state, stored resource, and local rules | Select, inspect, activate, connect, sometimes deploy | Yes |
| Directed Route | Transfer channel from one Component to another | Create, redirect, remove, inhibit | Yes |
| Stored resource, `Q` | Normalized available work held at a Component | Gather, store, transfer, consume | Yes |
| Supply Stream | External source that can deliver a finite amount of `Q` per step | Navigate into it; divert or delay it through declared tools | Yes |
| Medium motion | Velocity or force exerted by the environment | Steer against it or use it | Yes; immutable per Regime with chassis-specific drag coupling |
| Physical compartment | Authoritative causal membership with derived rendered geometry and a leakage coefficient | Reshape or breach as a paid intervention | Yes |
| Observation View | Passive protocol `V = (I, C, w, S)` over immutable history | Move aperture and change instruments freely | No |
| Intervention Budget | Meta-level limit on causal edits during an experiment | Spent only by declared physical edits | Governs allowed action; it is not `Q` |
| Typed material | Matter used to create or replace Components | Recruit, recycle, partition | Yes, embodied stock |
| Addressed signal | Decodable instruction delivered to a local rule | Schedule, delay, divert, decode | Yes, finite local record |
| Chosen code length | Serialized size under a declared canonical code | Read only in advanced analysis | Bits under that code; not automatically entropy or capacity |

Stored resource, matter, addressed signals, intervention budget, and
description cost are five different quantities. The game must not collapse
them into one glowing currency.

## What Charge means, and what flow means

The existing internal name `Charge` is retained in code for migration. The
player explanation is **stored usable resource**, symbol `Q`, measured in
Charge Units (`CU`). It behaves like a coarse available-work carrier.

- Inventory: `CU`.
- Transfer rate: `CU/step` or `CU/s`.
- Simulation rate: `30 steps/s`.
- Intervention Budget: separate integer points.
- Information: bits, shown only in analysis.
- Replacement material: typed units, separate from `CU`.

Flow does not cost Charge. Flow *is Charge moving*.

If a Route moves `3 CU` during one step, its tail loses `3 CU` and its head
gains `3 CU`. Internal Route transfer conserves `Q`. Supply adds `Q` from the
environment. Upkeep, physical leakage, dissipation, and overload loss remove
`Q` from the tracked system.

For Component `i`:

```text
Q_i(t + 1)
= Q_i(t)
+ external supply delivered to i
+ sum of incoming Route transfers
- sum of outgoing Route transfers
- upkeep
- physical-compartment leakage
- dissipation
- overload loss
+ internal recycling
```

The internal-recycling terms must sum to zero. Creating a replacement later
also consumes typed material; available work alone cannot become matter.

## What currently moves

The current Rust model does less than the Number 2 pictures imply:

- Forms move continuously in the plane and between discrete layers.
- Form position is mirrored into a `NodeKind::Form` Component.
- Other Components remain at authored positions.
- Directed Routes remain fixed graph connections.
- Stored resource moves algebraically between Component inventories; it is not
  simulated as individual particles.
- A `Current` is presently a finite **Supply Stream**. It distributes resource
  among Components within the exact point-to-polyline-segment capture width.
- A Supply Stream does not push, advect, or steer a Form. Regime-authored
  medium velocity is a separate force applied after steering.
- The existing Drift disturbance moves a Supply Stream's path.
- Wake entries remain at the location where they were left until their delayed
  effect becomes due.

Therefore, “ride the returning current” still means keeping the Form inside a
moving Supply Stream to receive resource. Environmental streaks instead show
the distinct medium-velocity field that changes Form motion without delivering
resource.

Target separation:

```rust
struct SupplyStream {
    path: Polyline,
    capture_width: Fx,
    max_delivery_q_per_step: Fx,
    schedule: SupplySchedule,
}

struct MediumVelocityField {
    regime_velocity: Vec2,
    drag: Frac,
    chassis_coupling: Map<FormKind, Frac>,
    component_collision_radius: Fx,
    collision_response: Frac,
}
```

The first system delivers resource. The second changes motion. They may occupy
the same visible channel but remain separate causal rules.

The current version implements a uniform regime-level sample rather than a
spatial grid. Each step applies `drag * chassis_coupling * (medium - velocity)`
after steering and before position integration. Open Field and Periodic
Transport are still; Crowded Medium, Vestige Pressure, and Holdout Atmosphere
carry distinct vectors, coupling strengths, and same-layer Component collision
envelopes. Collision uses deterministic axis-projected integer response after
position integration and before the Form's Component mirror is written.

## The Form's physical contract

Every selectable Form must disclose the same independently measured fields:

1. steering response under the actual spring/damping law;
2. stored-resource operating limit;
3. movement upkeep, if any;
4. global coupling-Pulse radius and direct effects until a per-Form radius is
   genuinely authored;
5. maximum construction span;
6. capacity of newly constructed Routes;
7. chassis-local permeability or loss;
8. chassis-specific ability;
9. whether that ability is implemented, analysis-only, or proposed.

The selected Form does not set the whole generator's physical-compartment
permeability. Permeability belongs to authoritative physical-compartment state;
Ring modifies only its own local loss. A Form may alter a material boundary
only through an explicit ability with a stated cost.

### Current and target Form contracts

| Form | Real current distinction | Player reality | Current causal implementation |
|---|---|---|---|
| Thread | `2x` commanded offset entering the steering spring; construction span `448`; operating limit `2048 CU` | High-acceleration, high-steady-speed survey chassis under the current law | Doubled steering response with the shared damping law and ordinary paid construction |
| Ring | Construction span `384`; operating limit `1536 CU`; local leakage reduction | Compact chassis intended for retention | Lower loss applies only to the Ring Form Component, never the generator boundary |
| Relay | Construction span `1088`; new Route capacity `64 CU/step`; operating limit `1024 CU` | Builds long, high-throughput connections while carrying little reserve | Commissioning enforces endpoint reach and stamps Relay Route capacity |
| Vault | `0.5x` steering; operating limit `4096 CU`; authored reserve `768 CU` | Slow storage chassis | Excess stock banks into an isolated reserve and discharges conservatively to nearby Components |
| Lens | Forecast horizon `30 steps / 1 s`; operating limit `256 CU`; sensor radius `192` | Local-sensing chassis with a bounded projection from currently sensed state | A paid `1 CU` packet builds eight local belief-Field realizations without remote or hidden input state |
| Knot | Operating limit `2560 CU`; upkeep `0.25 CU/step`; construction span `704` | Junction-building chassis | Four typed blanks support finite paid junction deployment with persistent upkeep |
| Wake | Conserving cache: up to `16 CU`, due after `60` steps within radius `64` | Cache-laying logistics chassis | Deposits transfer stock out of Wake and release only retained stock to local recipients |
| Chorus | Three linked Forms with separation limit `256` | Distributed chassis with direct-control handoff | Formation following, separation, supply eligibility, and handoff are explicit state |

Forms with unimplemented defining abilities should remain visibly unavailable
or carry an explicit `Ability Pending` status. They should not compete through
descriptive promises that the simulation does not enact.

## Concrete chassis abilities

These are the implemented mechanics that make each chassis legible. Values
require tuning only after the causal rule
exists.

### Thread

- Player steers the most responsive chassis.
- Its doubled commanded offset changes acceleration and steady speed under the
  existing spring while leaving the damping time constant unchanged.
- It may commission a Route only when both selected endpoints are within its
  working radius and their separation is at most `448` units.
- There is no hidden “thread bonus.” Its advantage is precise movement and fast
  commissioning.

### Ring

- The Ring's own Component loses less resource through its local shell.
- It does not lower leakage for unrelated Components.
- Its short `384`-unit construction span forces compact layouts.
- Its value is sustaining operation across supply gaps with a tightly built system.

### Relay

- Routes commissioned by Relay receive a `64 CU/step` ceiling rather than the
  normal `32 CU/step` ceiling.
- The selected endpoints may be at most `1088` units apart and must be locally
  reachable during commissioning.
- Its low `1024 CU` operating limit creates a real overload/storage tradeoff.

### Vault

- Excess supply can be transferred into an isolated `768 CU` reserve.
- Pressing the Form ability key opens a reserve-discharge preview.
- Releasing transfers a declared amount from the reserve to selected nearby
  Components. The ledger records equal reserve loss and recipient gain.
- Discharge never creates resource.

```text
requested = slider or held-key amount
released = min(requested, reserve_q, recipient_headroom)
reserve_q -= released
recipient_q += released
```

### Lens

Lens changes what the player can sense and project, not the laws of the Field.

- Hold the ability key to capture one chassis-local sensor packet.
- The packet contains only fields within the declared sensor radius and input
  schedule already revealed to the player.
- Clone a belief state constructed from that packet, not the omniscient
  authoritative world.
- Run `30` steps with neutral future control and show a range across modeled
  unresolved local state.
- Render only locally supported ghost positions, Route flows, and predicted
  limit crossings; fade or widen the display as uncertainty grows.
- Charge `1 CU` for each local sensor packet.
- Hidden Holdout conditions and unsensed remote state are never revealed.

```text
packet = sense_local_state(authoritative_state, lens_sensor_contract)
preview = build_belief_ensemble(packet)
for step in 1..=30:
    advance_each(preview, neutral_control, known_inputs, forecast_rng)
render_noncausal_local_range(preview)
```

Lens now pays `1 CU` for an authoritative local sensor packet inside a
192-unit radius. Rust partitions a local belief Field to the sensed Nodes,
internal Routes, local materials, signals, caches, and known Supply geometry,
then runs eight addressed 30-step neutral-control realizations. The laboratory
receives only sensed identities and low/expected/high local Charge trajectories;
it does not receive remote topology or hidden Holdout inputs and does not
establish generator self-prediction.

### Knot

- The ability deploys a junction Component at the Form's current position.
- Knot carries four typed junction blanks. Deployment consumes one blank,
  `32 CU` of stored resource, and one Intervention Budget point.
- The deployed junction holds up to `64 CU` and pays `0.25 CU/step` in upkeep.
- The junction accepts several short Directed Routes but pays continuing
  upkeep.
- The player gains local routing density at a persistent resource cost.

### Wake

Wake deposits a physical resource cache; “deposit” is not a metaphor.

- Press the ability key at the desired position.
- Transfer up to `16 CU` from Wake's Form Component into a new cache record.
- The cache remains visible and clickable.
- After `60` steps, it opens and distributes its retained resource among open
  Components within `64` units.
- If nothing can receive it, the cache remains or dissipates according to the
  selected Field law; it never creates new resource.

```text
amount = min(16 CU, wake_form.q)
wake_form.q -= amount
pending_cache.push(position, layer, due_step + 60, amount)

when cache is due:
    recipients = open Components within 64 units
    distribute cache.q subject to recipient headroom
    cache.q -= amount_delivered
```

The existing automatic deposit every `15` steps may remain as an optional
chassis policy, but it must still transfer resource out of Wake and be shown to
the player.

### Chorus

- The run begins with several linked Form Components.
- One receives direct steering.
- The others follow authored relative stations through the same motion law.
- The ability key cycles direct control among eligible members.
- A member outside the `256`-unit separation limit stops receiving shared
  supply and raises a visible separation warning.

This is coordinated embodiment supplied by the selected chassis. It is not
spontaneous multicellular organization.

## The coupling Pulse

The current Pulse has three direct effects:

1. it transfers one quarter of stored resource from eligible nearby
   non-Form Components into the controlled Form;
2. it opens closed Port Components inside its radius;
3. it reduces a reachable Supply Diversion disturbance.

Holding `E` increases radius. Releasing `E` applies the Pulse. The current
Pulse spends no stored resource. The interface must not imply otherwise.
Pulse radius is currently global rather than a per-Form catalog value.

During the hold, the renderer draws the true radius around the controlled Form,
locks every affected object, and distinguishes inward resource transfer, Port
activation, and Interference suppression through direction, hue, and motion.
The shell reports only nonzero aggregate outcomes. A missed release remains a
valid zero-cost action but cannot satisfy an instructional objective.

Before release, the preview must state exactly what will happen:

```text
Coupling radius: 96 units
Resource transferred to Form: 34 CU
Ports opened: 2
Supply diversion: 40% -> 30%
Actuation cost: 0 CU in current model
```

Because a Pulse can rescue a system, every Pulse is part of the external
control record. A hands-off trial disables it.

## Components and click targets

The Number 2 Field should have a strict interaction grammar:

| Visible object | Pointer behavior during play | Pointer behavior during Still Mode |
|---|---|---|
| Form | Hover shows `Q`, operating limit, velocity, current ability state; click focuses | Click selects; linked Form selection offers control handoff |
| Component | Hover shows type, `Q`, operating limit, inflow, outflow, upkeep, leakage | Drag to another Component proposes a Directed Route |
| Directed Route | Hover shows direction, capacity, last-step flow, rolling mean | Click selects; drag endpoint proposes redirect; Delete proposes removal |
| Supply Stream | Hover shows delivery ceiling, width, schedule, current recipients | Select as target only for a compatible intervention |
| Physical compartment | Hover shows members and leakage coefficient per exposed external contact per step | Drag a material handle proposes a paid physical edit |
| View aperture | Hover states that it is passive | Drag changes observed region for free |
| Wake cache | Hover shows retained `Q` and time until release | May be selected only if a declared tool can affect it |
| Disturbance | Hover shows target, level, onset, and exact modified rule | Can be selected only by a compatible response or experiment |

No click target may mean something different merely because a poetic label is
active.

In the current core, `Port`, `Reserve`, and `Module` are mostly labels with
different starting resource and interface state. A Module does not yet convert
or process resource. Player copy must not imply a biochemical function until a
distinct local rule exists.

## Reality of play

### Second-to-second

1. Steer the Form toward a Supply Stream, inactive Component, or failing part
   of the network.
2. Watch a small set of literal readings: Form inventory, required Route flow,
   Component operating margin, and time remaining before a criterion fails.
3. Hold and release E to apply Coupling: transfer existing resource, open
   interfaces, or push back a compatible disturbance.
4. Decide whether to keep piloting or stop and make a structural change.

### Minute-to-minute

1. Enter Still Mode.
2. Queue a Route, redirect, removal, or physical-compartment edit.
3. Read the exact predicted target and cost.
4. Commit the intervention.
5. Move the passive View and inspect the result without changing the system.
6. Make a prediction and run one controlled counterfactual.
7. Use Divergence Replay to see where the altered run first departs from the
   baseline.

### Session-to-session

1. Freeze the complete generator and its local rules.
2. Run an ensemble under varied unresolved state or runtime randomness.
3. Lock direct control and perform Holdout Validation.
4. Archive the successful design, failed branches, interventions, and evidence.
5. Carry the same generator into a different Atlas Field regime.

The fun is not in protecting a vague glowing center. It is in operating a
legible physical system, predicting how it will fail, making a constrained
change, and eventually trusting it without direct rescue.

## Formal causal model

```rust
struct ScenarioSpec {
    phi: PhysicalBackground,
    generator: GeneratorSpec,
    initial_state_distribution: InitialStateDistribution,
    exogenous_inputs: Vec<PhysicalInputSchedule>,
    control_contract: ControlContract,
    criterion: FunctionCriterion,
    intervention_plan: InterventionPlan,
    analysis_protocols: Vec<ViewDeclaration>,
}

struct GeneratorSpec {
    component_kinds: Vec<ComponentKindSpec>,
    topology_constraints: TopologySpec,
    local_policy: FrozenLocalPolicy,
    addressed_inputs: Vec<AddressedInputSpec>,
}

struct CausalWorld {
    scenario_hash: Hash,
    embodied: EmbodiedState,
    supplies: Vec<SupplyStream>,
    disturbances: Vec<DisturbanceState>,
    rng: RngState,
}

struct EmbodiedState {
    forms: Vec<FormState>,
    components: Vec<ComponentState>,
    routes: Vec<RouteState>,
    compartment: PhysicalCompartment,
    material_pools: Vec<MaterialPool>,
    local_timers: Vec<LocalTimer>,
}

struct PhysicalCompartment {
    members: Vec<ComponentId>,
    leak_per_exposed_contact_per_step: Frac,
}

struct ObservationWorkspace {
    views: Vec<ViewDeclaration>,
    active_view: ViewId,
    instruments: Vec<InstrumentRequest>,
}
```

One step is causal and does not accept a View:

```text
X_(t+1) = transition(lawset, X_t, scheduled_input_t,
                     explicit_control_t, runtime_randomness_t)
```

Observation happens afterward:

```text
Y_t = observe(recorded_history, active_view, instrument)
```

Target step order:

```rust
fn advance_world(world, scheduled_input, explicit_control, random_draws) {
    apply_medium_forces(world);
    apply_form_control(world, explicit_control.steering);
    integrate_component_motion(world);
    apply_commissioning_actions(world, explicit_control.actions);
    apply_frozen_local_rules_through_local_interfaces(world);
    allocate_route_transfers_synchronously(world);
    apply_supply_delivery(world, scheduled_input);
    apply_compartment_exchange(world);
    pay_upkeep_and_apply_dissipation(world);
    advance_disturbances(world, random_draws);
    record_complete_ledger(world);
}

fn observe(history, view, instrument) -> Reading {
    // Pure function. No mutable reference to CausalWorld is accepted.
}
```

`FrozenLocalPolicy` may read only the owning Component state, neighboring Route
state, locally available material, decoded local signals, and state-carried
timers. The experiment harness may know a failed target for evaluation but never
exposes its identifier, desired position, material source, or desired topology
to the policy.

Version 2 uses one-step synchronous Route transport:

1. snapshot all source stocks, destination headroom, gates, and capacities;
2. compute every Route request from that snapshot;
3. if a source is oversubscribed, scale all its requests by the same ratio and
   round each down, leaving fixed-point residue at the source;
4. if a destination is oversubscribed, scale all accepted incoming requests by
   the same ratio and round each down, leaving rejected amounts at their
   sources;
5. apply all accepted transfers simultaneously at the step boundary, so
   received resource cannot travel again until the next step.

Supply delivery uses the same proportional, round-down rule across eligible
recipients after Route transfer; rejected delivery remains in the external
Supply allocation or is dissipated according to the versioned regime rule. No
identifier receives a rounding remainder. This allocator, update order, capture
geometry, and one-step latency are part of `Phi` and must be versioned.

## Commissioning versus proof

The player contributes substantial organization while piloting and drawing
Routes. The game should celebrate that as engineering, not mislabel it as
self-organization.

Three experiment classes must remain explicit:

| Class | Direct control | What it can establish |
|---|---:|---|
| Commissioning trial | Allowed and recorded | The player can build and operate the generator |
| External substitution trial | Frozen replay; replacement properties declared | Function tolerates material replacement under supplied organization |
| Autonomous renewal trial | Disabled | Frozen local rules detect, recruit, position, reconnect, and restore function |

The autonomous-renewal trial removes the failed Component and its Routes. It
does not inherit position or topology automatically. The replacement begins as
typed material or an unpositioned spare and must be recruited by local rules.

## Example function contract

Candidate contract for a three-Component circulation:

> For 30 seconds, Routes A→B, B→C, and C→A must each carry at least `20 CU/s`
> in every rolling one-second interval. No required Component may remain below
> `10 CU` for more than `8 consecutive steps / 0.267 s`.
> Physical-compartment leakage over each rolling one-second interval must remain
> below `15%` of delivered Supply in that interval; an interval with zero
> delivered Supply fails if leakage is nonzero and otherwise records `0%`.
> At least `10 of 12` sealed hands-off trials must pass.

This contract tells the player exactly what to steer toward, what to build,
what to measure, and what must continue after control is withdrawn.

## Required migration decisions

1. Move standing physical membership and permeability out of
   `ViewDeclaration` and into authoritative Field/Generator state.
2. Stop copying selected Form `leak_frac` into the whole system's boundary.
3. Rename renderer and UI `Current` to `Supply Stream` until medium motion is a
   separate causal system.
4. Keep `FormState.charge` a read-only mirror of its authoritative Component
   inventory, or remove the duplicate field.
5. Make Route transfer synchronous or expose intentional transport substeps;
   Route identifier order must not alter the physical outcome invisibly.
6. Use point-to-segment distance for Supply capture instead of distance to
   sampled path vertices.
7. Replace Wake's unaccounted delayed source with a conserving cache or a named
   environmental Supply allocation.
8. Implement Lens forecast from cloned-state simulation; never grant a result
   from the authored horizon value itself.
9. Add typed material and addressed signals before making molecular-renewal or
   information claims.
10. Record steering, Pulses, handoffs, edits, and rescues as external control.
11. Separate immutable `GeneratorSpec` from embodied state and encode or
    disclose player-authored topology and exact initial conditions.
12. Replace sequential Route flow with the versioned proportional synchronous
    allocator and one-step latency defined above.
13. Keep Pulse radius global until per-Form content and physics support it.
14. Restrict Lens to local sensing or keep it as a laboratory tool.
