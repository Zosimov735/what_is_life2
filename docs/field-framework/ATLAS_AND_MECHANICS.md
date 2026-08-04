# What Is Life 2 — Atlas and Mechanics Contract

> Automation supersession: D-019 through D-022 and
> `AUTOMATION_AND_CONTRACTS.md` replace the steerable campaign as the primary
> loop. Quantity, regime, causal, observation, and assay rules in this document
> remain authoritative.

Status: design authority for the next implementation pass  
Date: 2026-08-02  
<!-- lexicon-check: allow-term — exact title of the supplied scientific reference -->
Scientific reference: Kiiskinen, Kivinen, and Rivas, *Information-theoretic Limits on Programmatic Specification of Biological Systems*, bioRxiv preprint 2026.07.27.740886.

## Product thesis

The player builds a compact local generator that maintains a measurable function across different physical environments and randomized realizations, then tests whether the system continues without continuous player rescue.

The game is not a claim that a small Node-and-Route simulation reproduces real molecular organization. It is a disciplined artificial world for exploring coarse-grained organization, environmental compilation, causal intervention, and ensemble-level function.

The visual world may remain mysterious. The instruments may not. Every player-facing scientific term must answer four questions:

1. What quantity or state is this?
2. What rule changes it?
3. What can the player alter?
4. What measurement demonstrates success or failure?

## Council positions

The historical figures are used as analytical viewpoints, not as fictional quotations.

- The Schrödinger viewpoint keeps usable energy, inherited specification, environment, and stochasticity separate.
- The Feynman viewpoint rejects any noun that cannot be reduced to an operation, unit, or observable.
- The Crick and Watson viewpoint treats inherited structure as a generator specification, not as a microscopic trajectory script.
- The Alberts viewpoint requires genuine maintenance, degradation, recruitment, replacement, and local reassembly.
- The Todd Howard directorial viewpoint makes the Atlas the organizing world: conditions are discovered spatially, and the player carries a design into unfamiliar physical regimes.

## Four objects that must never be conflated

| Object | Meaning | May alter physics? |
|---|---|---:|
| Physical compartment | Persistent causal membership and permeability that determine leakage | Yes |
| Observation View | Passive selection, grouping scale, time window, and comparison neighborhood | No |
| External intervention | A player- or experimenter-supplied change to state, topology, input, or medium | Yes |
| Autonomous local process | A simulated rule by which the system detects, recruits, positions, and reconnects without direct rescue | Yes, through its authored local rule |

Changing a View is measurement. Reshaping a physical compartment is an intervention. They must use different state, controls, colors, costs, history, and replay baselines.

## Mapping to the preprint

| Preprint object | Game realization |
|---|---|
| Static system specification, `g` | Immutable `GeneratorSpec`: frozen local rules and every architecture detail explicitly encoded by the design |
| Exogenous physical input, `U_phys(t)` | All time-varying external physical input, including resource delivery, mechanical conditions, and signals |
| Addressed environmental message | Only the addressed, decodable portion of `U_phys(t)` contributes to environmental information capacity `C_E` |
| Runtime randomness, `W_t` | The modeled unresolved input stream; a PRNG seed merely indexes one realization |
| Compiler and substrate, `Phi` | Fixed lawset plus fixed substrate parameters within one trial family |
| Analysis protocol, `V = (I, C, w, S)` | Subsystem `I`, deterministic coarse-graining `C`, trajectory window `w`, and comparison neighborhood `S` |
| Induced trajectory, `X_C` | The trace after the declared state projection `C`, with `I`, `w`, and `S` held fixed for comparisons across grains |
| Gameplay Ensemble | Repeated validation runs under one declared `ScenarioSpec`; reports observed variation and pass results, not an information-theoretic threshold |

If an Atlas destination changes `Phi`, its results are conditioned on a
different background and are not pooled into one capacity comparison.

Capacity compatibility is not proof that a specification is sufficient. The
game must still require an implemented local causal generator that produces the
demanded function. Information-theoretic estimates remain disabled until the
state alphabet, trajectory distribution, canonical code, estimator,
uncertainty, and unseen-support treatment are declared. Gameplay Ensembles show
pass count, traces, median, observed range, and observed distinct outcomes.

## Core physical quantities

### Stored Charge

Internal name: `Charge`  
Player explanation: usable stored resource  
Symbol: `Q`

`Q_n(t)` is the stored quantity at Node `n`.

```text
closing Q
= opening Q
+ supply delivery
+ incoming Route flow
- outgoing Route flow
- upkeep
- physical-compartment leakage
- overload loss
```

- Unit: Charge.
- Node range in the current core: `0–4096 Charge`.
- The current simulation runs at `30 steps/s`.
- Charge is not information and must never be displayed in bits.

### Supply Stream

Internal name: `Current`  
Player label: `Supply Stream`

A Supply Stream is an external delivery corridor defined by:

- path;
- width in field units;
- strength in Charge per step;
- layer;
- active state.

Displayed delivery rate should normally be converted to `Charge/s`. A Supply
Stream contributes specification information only when its modulation carries
distinct addressed and decodable messages. Its resource magnitude alone is not
a bit count.

### Node

Player label: `Component`

A Node is one storage, interface, processing, reserve, or controlled Form element. Its local instrument must expose:

- stored Charge;
- overload threshold;
- current inflow;
- Route inflow and outflow;
- upkeep;
- leakage;
- overload loss;
- open or closed state when applicable.

### Route

Player label: `Directed Route` or `Conduit`

A Route is a directed transfer connection from a tail Node to a head Node. Its flow is limited by:

- Route capacity;
- source stock;
- destination headroom;
- interface state;
- overload response;
- declared disturbance effects.

Display capacity and mean flow in `Charge/s`, with the averaging window stated.

### Boundary flow

Do not display `Flux` without qualification.

- Influx: sum of Route flow entering the observed selection.
- Efflux: sum of Route flow leaving the observed selection.
- Net boundary flow: influx minus efflux.
- Unit: `Charge/s`.
- Every value names its View and averaging window.

Example: `View V3 net boundary flow: +18 Charge/s over 1.5 s`.

### Physical compartment

Internal migration target: `PhysicalEdge` or `CompartmentBoundary`.

The version-2 graph compartment uses Component membership as authoritative;
rendered geometry is derived and cannot disagree with that membership. It
determines leakage regardless of which View is active. Its geometry is thick
and material in the Number 2 visual language.

In the retained abstraction, an exposed Component loses
`L_i = Q_i * min(1, kappa * e_i)` per step, where `e_i` is its number of exposed
external contacts and `kappa` has units of fraction per exposed external
contact per step. `kappa` is a game leakage coefficient, not a claim of physical
permeability derived from concentration, area, and molecular flux.

### Observation View

A View is a passive analysis protocol:

```text
V = (I, C, w, S)
```

- `I` selects the observed subsystem.
- `C` is a deterministic projection/partition of recorded causal state.
- `w` selects the trajectory window.
- `S` selects the comparison neighborhood.
- Changing it costs no intervention resource.
- Changing it records no physical edit.
- Changing it never changes leakage, flow, stored Charge, or future state.
- Its geometry is thin, violet, and explicitly noncausal.

### Phase and response lag

Current phase now controls an explicit integer duty-cycle window. `strength`
is the cycle-average ceiling. For a period `P` and duty `d`, the on-window is
`ceil(Pd)` steps and its multiplier is `P / ceil(Pd)`; the remaining steps emit
zero. Recipient headroom can still reject delivery, so accepted Supply remains
an empirical ledger quantity rather than a promise.

Each emitting Current then draws one symmetric multiplier from its addressed
trajectory stream within the RegimeSpec `supply_jitter` bound. Route Noise is
keyed by `(route_noise, layer, step)` and Supply by
`(supply_jitter, current, step)`. The interval is
descriptive modeled variability and preserves the authored mean; it is not a
measurement-error or molecular-fluctuation claim.

- phase: `0–1 cycle`, degrees, or radians;
- period: steps and seconds;
- response lag: the shortest retained lag maximizing positive centered
  cross-covariance between the named periodic Supply ceiling and selected
  stored Charge, reported with a bounded descriptive correlation.

### Initial-state retention

Replace the ambiguous `Source` readout with `Initial-state retention`.

It is the fraction of the closing observed Charge attributable to the View
window's opening stock under the declared accounting model. A later source
ledger should separately display initial stock, each Supply Stream, delayed
effects, and crossing Routes.

## Observation instruments

| Current label | Player-facing replacement | Output |
|---|---|---|
| Flux | Boundary flow | Inflow, outflow, and net flow in Charge/s over a stated window |
| Phase | Response lag | Lag between a named periodic input and selected stored Charge; unavailable without a periodic Current |
| Source | Initial-state retention | Fraction of final selected Charge attributable to opening stock |
| Resolution | Measurement grain | Number of Components grouped into one observable block |
| Temporal shutter | Analysis window | The last `T` steps and seconds included in the calculation |
| Surround | Comparison neighborhood | One hop, two hops, or all nonmembers |

External-substitution tolerance is an interventional assay over cloned worlds,
not a passive observation instrument. Its result may be inspected here only
after the Intervention Bench has produced it.

Recommended coordinate names:

| Existing name | Replacement |
|---|---|
| Swap Range | Noncritical Components |
| Self-Support | Internal Maintenance Share |
| Throughput | Boundary Flow |
| Upkeep Mix | Maintenance Allocation |
| Reach | Active Circulation Span |
| Input Resolution | Distinct Boundary States |
| Horizon | Memory or Forecast Horizon |
| Source Trace | Initial-state Retention |
| Instruction Separation | Reconstruction Fidelity |
| Turnover Tolerance | External Substitution Tolerance |
| Scale Stability | Resolution Consistency |
| Shared Failure | Failure Correlation |
| Cut Impact | Disconnection Sensitivity |
| Boundary Sufficiency | Boundary Accounting |

## Intervention instruments

Every tool displays only parameters that physically apply. There is no universal row of `target`, `dose`, `width`, and `duration`.

| Tool body | Literal subtitle | Action and parameters |
|---|---|---|
| Blade | Remove Route | Route identifier, onset, permanent or replay-only |
| Clamp | Reduce capacity | Route identifier, inhibition percentage, duration |
| Scramble | Reduce routing fidelity | Network selection, misrouting probability, duration; total Charge conserved |
| Decoy | Divert supply | Supply identifier, capture fraction, receiving Node, duration |
| Delay | Shift input timing | Input identifier, delay in steps and seconds, onset |
| Replace | External substitution | Selected Components, replacement fraction, explicitly transferred properties |
| Breach | Raise leakage coefficient | Proposed member boundary, coefficient change per exposed external contact per step, duration |
| Transplant | Change Field regime | Destination regime, equilibration interval, retained state and inputs |

Measurement-grain, time-window, and comparison-neighborhood changes are observation-sensitivity checks, not interventions.

## External substitution versus autonomous renewal

### External substitution

The experimenter supplies a replacement and may explicitly transfer declared
properties. The current core transfers Component kind, layer and position, open
state, capacity, upkeep, Form association, analytical membership replacement,
and every incoming and outgoing Route endpoint. This is useful for testing
material identity under externally supplied organization, but it must be
labeled as external substitution and publish the complete transfer policy.

### Autonomous renewal

The later trial must remove or degrade a Component and require local rules to:

1. detect loss;
2. recruit or produce a replacement;
3. position it;
4. restore necessary connections;
5. recover the demanded function.

The trial reports:

- detection latency;
- recruitment latency;
- reconnection fraction;
- functional recovery time;
- resource cost;
- pass rate across independent trials.

No automatic topology inheritance and no player rescue are permitted.

## Forms as steerable commissioning chassis

The Form is the bright object the player steers. It is a mobile Component and
commissioning chassis, not the complete generator. The generator being tested
is the Form plus Components, Directed Routes, physical compartment, and frozen
local rules.

The player uses the Form to navigate Supply, activate Components, commission
connections, and operate a chassis-specific ability. Direct steering and Pulse
use are external control. They are disabled in hands-off validation or counted
in the supplied specification.

The Form screen must show steering response, operating limit, upkeep, coupling
radius, construction span, capacity of newly commissioned Routes, chassis-local
loss if it exists, and the actual special mechanism. It must name incomplete
mechanics rather than awarding them descriptive advantages.

The selected Form must not determine the whole generator's physical-compartment
permeability. That property belongs to the physical compartment. Likewise,
Wake's delayed delivery must conserve stored resource or name an external
Supply source, and Lens must run a real cloned-state forecast rather than gain
an analysis result from its authored horizon.

The authoritative object model, all eight chassis contracts, player controls,
and migration decisions are in `FORM_AND_PLAY_MODEL.md`.

## Atlas contract

The Atlas replaces the chapter list as the main progression surface. Each
destination declares a fixed lawset and substrate-parameter set `Phi`, plus a
separate schedule of exogenous physical inputs `U_phys(t)`. Results from
destinations with different `Phi` values remain separate trial families.

A destination panel states:

- technical identifier;
- medium and transport rule;
- input schedule;
- Route conductance rule;
- dissipation rate;
- runtime-noise level;
- mixing or response time;
- available interventions;
- functional success criterion;
- present implementation status.

Recommended destination language:

- Steady Transport;
- Periodic Transport;
- Branching Transport Network;
- Crowded Diffusive Medium;
- Reactive Porous Medium;
- Advected Supply Field;
- High-Turnover Compartment.

Atmospheric names may remain as quiet subtitles, but never replace the technical identifier.

### Reference destination

`Regime 01 — Steady Transport`

- Supply rate: `2.00 Charge/step` on the opening layer.
- Supply width: `64 field units`.
- Background loss: `0.00 Charge/step` on the opening layer.
- Noise fraction: `0.125`.
- Network: `7 Nodes, 6 Routes`.
- Default observation window: `45 steps / 1.50 s`.
- Later disturbance: Route redirection and supply-path displacement.

The Atlas may start every destination whose catalog status is `implemented`.
Each such destination is backed by a named immutable Rust `RegimeSpec`; a
catalog entry remains `Model Pending` whenever no corresponding causal lawset
exists.

## Keyboard and pointer contract

### Atlas

- Mouse click: select destination.
- Left and Right Arrow: change destination.
- Enter: open an available destination.

### Active play

- WASD or Arrow keys: requested heading and speed. Pointer motion never steers.
- Hold E: extend the coupling radius.
- Release E: apply the coupling Pulse.
- Wheel or bracket keys: change depth.
- Space: enter or leave Still Mode.
- Handoff control: select which linked Form receives direct steering.

Pointer input is reserved for Field inspection, the Why control, Still Mode
handles, and laboratory controls. It never begins a Pulse.

The Pulse preview must identify affected objects in world space and summarize
its predicted direct effects before release:

```text
Radius: 96 units
Gates opened: 2
Charge transferred: 34
Supply diversion: 40% -> 30%
```

### Still Mode

- Drag Component to Component: queue a directed Route.
- Drag a Route endpoint: redirect that endpoint.
- Select a Route and Delete: queue removal.
- Drag a physical-compartment handle: queue a causal boundary edit.
- Use the separate violet View tool: change observation at no intervention cost.
- Enter: apply queued physical interventions.
- Escape: undo newest physical intervention; Escape again leaves Still Mode.

## Operational objective grammar

Every objective includes:

- named observable;
- target range or threshold;
- duration;
- allowed recovery interval;
- whether player control is permitted;
- number of trials and pass count when applicable.

Bad: `Find a description that holds.`

Good:

> Using the declared nested partitions and a fixed subsystem, 45-step window,
> and comparison set, choose the coarsest grain whose outlet-flow trace has
> normalized absolute-error agreement of at least `0.90` with the Component-level
> trace on the held-out interval.

For samples `k` in that held-out interval:

```text
agreement(C)
= clamp(1 - sum_k |y_C(k) - y_ref(k)|
          / max(epsilon, sum_k |y_ref(k)|), 0, 1)
```

`y_ref` is the declared Component-level outlet-flow trace, `y_C` is the trace
after the declared nested partition and aggregation rule, and `epsilon` is the
fixed nonzero denominator guard stored in the experiment. The grouping,
aggregation, target trace, fitting interval, and held-out interval are frozen
before evaluation.

Bad: `Conserve connections and navigate without collapse.`

Good:

> For 30 seconds, keep at least three named Routes at or above mean flow
> `24 Charge/s` in every rolling one-second window while following the displaced
> Supply Stream. No required Component may remain below its operating threshold
> for more than 30 consecutive steps.

Bad: `Missed signal cadence: delta t greater than window.`

Good:

> Supply arrived 0.40 s after the Component's 0.30 s acceptance interval.

## Divergence replay

`Echo` becomes `Divergence Replay` on the technical layer.

It is a paired replay from one Anchor with the same initial state and one
open-loop recorded control sequence. Modeled random draws use keyed common
randomness addressed by `(event kind, object id, step)`, so an intervention
cannot shift every later draw by changing draw order. The intervention is the
only changed condition. A frozen internal feedback policy is part of
`GeneratorSpec` and may respond differently when state diverges.

The display may identify the first recorded divergence. It may not call that event the complete cause unless the intermediate links were separately tested.

Example timeline:

- `18.4 s` — Route C12 removed.
- `20.1 s` — outlet flow fell below `8 Charge/s`.
- `21.3 s` — reserve R3 fell below `20%`.
- `24.0 s` — functional criterion failed.

## Ensemble experiment

An Ensemble holds constant:

- generator specification;
- Field regime;
- addressed input schedule;
- either a neutral/open-loop control sequence or one frozen internal feedback
  policy declared as part of `GeneratorSpec`;
- macroscopic initial condition.

It varies modeled runtime randomness or samples from a declared unresolved
initial-state distribution. A PRNG seed indexes one modeled realization; it is
not a measure of random bits consumed. A seed supplied to reproduce an exact
trajectory is recorded as side information.

The display includes:

- individual realization traces;
- median;
- observed range or quantile interval;
- failure-mode counts;
- pass fraction;
- measurement grain and View.

Eight trials are a useful game preview. Their min-to-max band is an observed range, not a confidence interval and not proof of rare-failure performance.

## Compile and holdout validation

Open Field compiles one explicit `ScenarioSpec`:

1. fixed lawset and substrate parameters, `Phi`;
2. immutable `GeneratorSpec`, `g`;
3. initial-state distribution and any player-authored initial side information;
4. complete exogenous physical-input schedule, `U_phys`, with its addressed
   decodable portion identified separately;
5. modeled runtime-noise process, `W`;
6. control mode and any open-loop control sequence;
7. function-criterion vector, intervention plan, and analysis protocols.

`Compile 8 Trials` means:

1. freeze and hash the complete `ScenarioSpec`;
2. disclose player-authored topology and exact initial conditions as encoded
   design or side information;
3. run eight modeled realizations;
4. apply passive analysis protocols;
5. report pass count, individual traces, median, observed range, and observed
   distinct outcomes.

It makes no Shannon, Hartley, support-cost, or capacity claim. Canonical
serialized length is a chosen code length, not automatically entropy or channel
capacity, and a hash length is never description cost.

Holdout Validation locks physical editing before revealing the schedule. The
suite is sealed and version-hashed before design evaluation, selected
independently of the candidate, and retired or marked contaminated after it has
been used repeatedly for optimization. Human/adaptive external control is
disabled; if admitted in another experiment, its sensing, memory,
communication, and actuation enlarge the external controller channel rather
than becoming free because an action hash was stored.

Example demand:

> For 60 seconds, each outlet's rolling one-second mean must remain between
> `8` and `12 Charge/s`; total stored Charge must remain between `400` and
> `1200 Charge`; no outlet may stay below `8 Charge/s` for more than 60
> consecutive steps; and at least 7 of 8 sealed trials must pass.

## Scientific correction ledger

The following ledger records both implemented corrections and the limitations
that still block proof-grade claims:

1. Physical-compartment membership is now independent from passive View
   membership. It remains a graph-contact leakage abstraction rather than a
   geometric boundary microphysics model.
2. Candidate View selection is now free and passive. A View protocol remains a
   modeled observation operation rather than the paper's coarse-graining itself.
3. Full turnover is external substitution with automatic topology transfer.
4. Player steering, Pulses, and rescue add addressed external information.
5. Charge is a resource and cannot stand in for information.
6. The current mesoscale model does not simulate molecular microstates.
7. Capacity compatibility does not establish causal sufficiency.
8. Shannon and Hartley quantities must remain separate.
9. Eight-run ranges are not confidence intervals.
10. Runtime randomness now includes bounded per-emission Supply variability in
    every implemented Regime, in addition to Route conductance Noise. Ensemble
    ranges remain descriptive for the declared model and finite addressed seeds.
11. Route execution now uses a simultaneous proportional one-hop allocator;
    intentional multi-hop transport therefore requires later simulation steps.
    A one-hop occupancy overshoot with an active outgoing Route is transport in
    flight. Pattern failure is reserved for overload with no active relief.
12. Supply membership now uses exact integer point-to-polyline-segment distance
    over the authored path rather than sampled vertices.
13. Maintenance allocation now preserves the exact paid sink across five
    structural purposes. The version-1 attribution is role-based rather than a
    molecular maintenance model, so it supports gameplay accounting but not a
    biochemical maintenance claim.
14. Response-lag evidence requires enough retained periodic cycles; an absent
    periodic Current or fewer than two complete retained cycles returns distinct
    unavailable provenance and establishes no lag.
15. The scientific reference is a preprint and has not undergone peer review.
16. A View protocol is not identical to the paper's coarse-graining `C`.
17. Small gameplay Ensembles do not estimate Shannon entropy or Hartley support.
18. Player-authored topology and exact initial conditions are specification or
    side information, not free organization.
19. PRNG seeds index modeled realizations and do not measure random-bit use.
20. A global repair controller may not supply target identity, position,
    material source, or desired topology to local renewal rules. Live local
    reconstitution now acts only through emitted deficit signals, nearby donor
    stock, nearby typed material, and standing attached Routes. The cloned
    full-turnover assay remains the stronger identity-replacement test.

## Implementation sequence

1. Add a real physical-compartment state independent of View state.
2. Move View editing to a passive, free observation layer.
3. Separate immutable `GeneratorSpec` from embodied state and declare all
   initial side information.
4. Implement the Number 2 Atlas shell with one honest playable destination and
   explicit model-pending destinations.
5. Replace vague Form promises with complete parameter tables and implementation-status disclosures.
6. Rename existing measurements and separate observation sensitivity from interventions.
7. Implement exact local ledgers and hover inspection. The current Field
   resolves Component, Directed Route, Supply Stream, physical-compartment,
   passive-View, and typed-material targets through the on-demand
   `inspect_field` Rust command. The pressure register expands to its exact
   stage, level, target, onset, and authored rule explanation.
8. Define and version a simultaneous transport allocator, Supply sharing,
   rejection, update order, and Route latency; correct segment-based capture.
9. Build paired Divergence Replay with keyed common randomness.
10. Add runtime variability and descriptive Ensemble analysis.
11. Add Open Field compilation and sealed Holdout Validation.
12. Add autonomous degradation, recruitment, positioning, reconnection, and renewal using only locally available state and signals.

## Number 2 performance contract

The visual target is achievable on a laptop if dense atmosphere is not simulated literally.

- Atlas dense filaments and particles are a precomposed texture or cached render texture.
- Typography and instruments are live DOM elements.
- Only selection rings and a small number of local cues animate.
- Internal render resolution is capped near `2.6 million pixels`.
- Atlas animation targets `30 fps` and stops when idle or hidden.
- No full-screen fluid solver or repeated blur pass.
- No more than four full-screen compositing passes.
- Field particle counts remain bounded.
- Retina device-pixel ratio is capped rather than accepted without limit.

The Number 2 art should carry wonder. The measurement layer should carry precision.
