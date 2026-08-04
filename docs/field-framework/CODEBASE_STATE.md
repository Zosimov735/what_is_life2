# What Is Life 2 — Current Codebase State

Status: automation rapid implementation; validation and human-readiness claims deferred
Baseline reviewed: working tree after `1ae447c`
Date: 2026-08-03

## What exists on the baseline `main`

The repository is a three-layer application:

```text
React shell and Pixi/Canvas renderer
          |
          v
Web Worker protocol and fixed-step scheduler
          |
          v
Rust/WASM deterministic simulation core
```

It includes:

- a deterministic 30-step-per-second runtime;
- fixed-point simulation quantities and explicit random state;
- Forms, Components, Ports, Directed Routes, Boundaries, Currents, layers, and
  a Charge ledger;
- steering, depth changes, the coupling Pulse, and Still Mode;
- queued structural changes with commit and undo;
- candidate Views, coordinate profiles, perturbations, and divergence data;
- six authored pressure families;
- eight selectable Forms;
- eight authored chapters and a campaign runner;
- save/export/import, checkpoints, branch recovery, and content hashing;
- a causal `PhysicalCompartment` separated from the passive Observation View;
- an authoritative immutable `ScenarioSpec` owning content identity, regime,
  pressure schedule, control contract, and per-chapter function criteria;
- a nested organization-only `GeneratorSpec` owning declared Component kinds
  and Route topology independently of broader scenario conditions;
- exact criterion runtime for rolling Route throughput, Component Charge
  margins, leakage/Supply ratio, recovery grace, and hands-off completion;
- immutable Atlas regime rules for Supply scale, dissipation, conductance
  noise, Route capacity, and physical-compartment leakage;
- five selectable Atlas regimes, including causal Periodic Transport and the sealed Holdout Atmosphere
  composed lawset;
- simultaneous one-hop Route allocation and segment-based Supply capture;
- conserving integer duty-cycle Supply cadence with visible emitting/quiet
  windows and a passive retained-history response-lag instrument gated on two
  complete input cycles;
- conserving typed upkeep records split across boundary, repair, replacement,
  movement, and reserve, with named passive-instrument and hover readings;
- regime-authored environmental velocity and drag, mechanically separate from
  Supply delivery, with chassis-specific coupling, replay parity, exact hover
  readings, and shared WebGL/Canvas directional marks;
- regime-authored same-layer Component collision envelopes with deterministic
  Form displacement/rebound and shared renderer/hover geometry;
- bounded regime-authored Supply variability drawn per emitting Current in a
  stable replayable order, giving Ensemble seeds actual runtime divergence;
- keyed common randomness for Route Noise and Supply variability, addressed by
  event, object, and step so paired interventions do not shift unrelated draws;
- conserving Wake caches, Vault reserve banking/discharge, and Ring-local
  leakage reduction;
- live local Component reconstitution driven by finite deficit signals, nearby
  donor Charge, junction stock, and conductor stock, with assembly loss in the
  exact ledger and no shell-selected repair target;
- Relay-authored construction reach and Route capacity, explicit Chorus linked
  formation/separation/handoff, paid local Lens sensing, and finite Knot
  junction deployment;
- React/Pixi WebGL rendering with a Canvas2D fallback;
- the Number 2 graphite Field treatment, authored texture, and separate physical
  and observational Still Mode instruments;
- distinct rendered chassis silhouettes for all eight Forms, separated Route
  topology, localized pressure stress, restrained Supply Currents, and explicit
  low/medium/high renderer budgets shared by WebGL and Canvas2D;
- instrumented Coupling on `E`, with a true world-space radius, target locks,
  directional gather/open/suppress animation, a compact nonzero-effect readout,
  and pointer input reserved for inspection;
- visible Supply-to-recipient filaments, controlled-Form reservoir arcs, and
  distinct dormant/open Port stances in both renderer tiers;
- exact Coupling Port locks from core-projected Port ids, dormant Route wiring
  with closed-end gate marks, and permanent tail-to-head direction notches;
- a persistent campaign rail with chapter title and position, objective
  position, campaign elapsed time, chapter progress, and overall progress;
- a player-facing `?field_run` shortcut that opens the authored campaign rather
  than the legacy no-campaign stand-in (`?field_stand_in` remains diagnostic);
- localized active-intervention marks for constrained and scrambled Routes,
  diverted and delayed Supply Streams, decoy receivers, and breached exposed
  compartment members, with exact authoritative inspection readings;
- a worker protocol that keeps the simulation off the UI thread;
- a second module worker that imports canonical runs and executes Rust/WASM
  divergence, ensemble, holdout, and inheritance analysis over cloned state;
- passive Observe instruments computed on demand from authoritative Rust Field
  quantities rather than decoded render-frame approximations;
- on-demand Component, Directed Route, Supply Stream, compartment, passive
  View, and typed-material hover inspection backed by exact Rust state and
  retained flow records rather than lossy renderer values;
- a live coupling-Pulse instrument projected through the same gather, open,
  Vault-discharge, and disturbance-displacement helpers as release, with exact
  radius, transfer, interface, diversion, and cost readings;
- authoritative Form-chassis hover readings for inventory, operating limit,
  velocity, control, and implemented ability state, plus visible conserving
  Wake caches with retained Charge, release time, and delivery radius;
- durable IndexedDB Archive records with canonical payloads and evidence;
- a selectable spatial Archive lineage surface with branch reopening, record
  removal, protocol-bound evidence vectors, and direct Generator/Scenario/
  embodied-state/control comparison between records;
- a constrained Open Field scenario compiler with direct Component, Route,
  physical-compartment, and Supply authoring over cloned trial templates;
- provenance-complete cold analysis, Renewal, and Open Field result envelopes
  carrying control, GeneratorSpec, ScenarioSpec, and embodied-state identities.

## What the current engine really does

- A Form moves continuously in two dimensions and across discrete layers.
- The controlled Form's position is mirrored into a Form-kind Component.
- Other Components and graph topology remain authored unless an explicit plan
  changes them.
- Route transfer is algebraic rather than particle-based.
- A `Current` supplies finite Charge to nearby Components; it does not push the
  Form.
- Drift displaces that Supply path.
- Physical-compartment membership and leakage are chapter-authored causal state.
- Observation View membership is immediate analysis state and does not enter the
  transition, replay, perturbation, budget, or physical-plan paths.
- Frame V3 carries physical, proposed-physical, and View membership separately,
  embodied material stock, and expiring local signal evidence. Passive Still
  instruments read the canonical Field directly through `sample_instrument`.
- Cold Divergence, Ensemble, Holdout, and Batch Inheritance jobs restore cloned
  `RunState` in a dedicated WASM worker and use the same Rust transition and
  plan projection as live play. The main worker provides the same command as a
  fallback; neither path synthesizes trials in TypeScript.
- Lens forecast ranges are produced by eight local belief-Field realizations in
  Rust after the paid sensor capture; React only projects the returned packet.
- Holdout sealing records a randomized local suite identity, candidate hash,
  version hash, and addressed Rust seed before execution. Archive records carry
  the lawset/protocol/RNG and reproducibility metadata needed to interpret a
  branch without deriving it from display state.
- Renewal assays consume persistent typed material and local donor Charge to
  recruit a replacement and reconnect locally observed Routes in cloned Fields.
- The live Number 2 Field projects unclaimed Renewal stock as depth-aware
  material glints and local evidence as finite-lifetime signal rings; the
  Renewal bench reads the same embodied inventory before launch.
- Each authored chapter now has an optional frozen function criterion. The
  mutable criterion runtime retains only its exact rolling samples, streaks,
  status, and resolution step; a chapter transition selects the next frozen
  contract and resets only that runtime window.
- The shell receives criterion evidence through `criterion_changed`, caches no
  inferred simulation values, and renders a compact contract rail only while
  the Field is running.
- Cold analysis now honors its declared control contract. Recorded-open-loop
  families replay the retained authoritative control sequence through the same
  Rust transition; hands-off families use neutral control, and intervention
  steps are explicitly excluded from hands-off criterion streaks.
- Divergence, Ensemble, Holdout, and inheritance result payloads carry the
  actual control contract, nested generator hash, and frozen scenario hash.
  Holdout seals source those identities from run-open/restore rather than using
  a bench-local scenario ID as a generator surrogate.
- Open Field applies the exact FunctionCriterion evaluator to every cloned
  trial and derives its organization-only Generator identity from the frozen
  experiment template. Renewal results retain the live scenario, generator,
  control, and source embodied identity for every seed.
- Frame V3 intervention flags reuse reserved record bits, preserving the frame
  width and cap. Both renderers consume one shared scene projection; low quality
  retains geometric bars, dashes, and arcs without bloom or extra particles.

## Remaining simulation defects relative to the target

1. Crowded Medium, Vestige Pressure, and Holdout Atmosphere combine explicit
   transport, dissipation, leakage, noise, uniform medium motion, and Component
   collision laws; they do not yet add memory, reaction, spatial velocity grids,
   or arbitrary conversion laws.
2. Holdout suites now persist sealed, evaluated, contaminated, and retired
   administration locally; server-independent secret custody remains absent.

## Product mismatch

The approved target has changed to a mechanistic automation game. The current
runtime still accepts direct Form steering, Pulse, depth, handoff, and Still
Mode inputs; `GeneratorSpec` still contains topology without player-authored
local policies; chapter objectives still drive primary run progression; and the
shell has no contract workbench, policy editor, accelerated qualification
transition, independent engineering grades, or reusable assembly blueprint.

M-017 establishes the superseding product authority. M-018 begins the runtime
migration at the deterministic Rust policy boundary.

The current working tree launches into the Number 2 Atlas, carries the selected
implemented regime into authoritative run initialization, opens the measured
Form catalog, and exposes Observe, Intervention, Divergence, Ensemble, Holdout,
Archive, Renewal, Batch Inheritance, and Open Field benches from Still Mode.
The legacy chapter campaign remains the authored commissioning content behind
those surfaces.

The active Field now carries a persistent campaign position rail. It names the
chapter and objective, shows both counts, reports the authoritative campaign
elapsed clock, and separates chapter progress from overall campaign progress.
The [human playtest guide](PLAYTEST_GUIDE.md) remains the detailed campaign map:
it names expected time bands, every objective's advancement condition, and the
visual/audio mark that corresponds to each noun.

The current Atlas and Form surfaces are no longer a detached prototype. Their
Number 2 constellation, measured Form silhouettes, regime contract rail, and
responsive selection composition are integrated into the shell working tree.
The dated prototype record remains preserved in
[Atlas prototype QA](qa/ATLAS_PROTOTYPE_QA.md) as historical evidence only.

## Build and verification state

The rapid implementation pass has crossed its human-playtest readiness gate.
The current working tree has passed the production build, TypeScript, Vitest,
Rust bridge, chapter, campaign, tool-contract, lexicon, and browser checks
listed in the milestone ledger. The campaign now completes all 23 regression
cases, The Loop completes all 15 chapter checks, and The Quiet Edge carries all
three continuity paths through 32,768 neutral-input finale steps with Charge
remaining.

Desktop 1280 × 720 and narrow 319 × 699 evidence, same-state mock-up
comparisons, keyboard steering, pointer-accessible Why, Pulse isolation, Still
Mode, and all laboratory destinations are recorded in `design-qa.md`.

Legacy framework and architecture document contract checks remain skipped
because their source documents were never committed and could not be
recovered. Those 76 explicit skips do not conceal executable product failures.

## Implementation roadmap state

The selected current step and sequential rapid queue live in
[the milestone ledger](MILESTONES.md). M-003F through M-016 implementation,
the first M-018/M-019 local-policy and workbench vertical path, and the C-01
contract compiler/catalog/ladder vertical path are represented in the working
tree. Protocol 12 can list contracts while no RunState exists, open a
contract-addressed Design run; contract schema 2 separates current capabilities
from post-pass receipts; open/restart/recovery carries contract, attempt,
generator, assembly, scenario, and regime identity;
Intake, Transfer, and Buffer have versioned authored records and generic
criterion declarations; Save V6 and archive schema 4 retain an immutable
qualification request; and the shell has a catalog-backed Number 2 ladder.

This is source state, not current validation evidence. C-01.R is represented;
the current assembly hash is a separate compiled-opening address. Revised
M-020E packet E-00 now owns the minimal canonical `AssemblyTemplate`, explicit
branch lineage, and `RunKind` required before qualification; M-022 owns reusable
blueprint naming, selective assembly adaptation, and compatibility. M-020B Intake
is represented with a contract-owned filtered opening, constrained authoring,
exact Charge-ledger evidence, causal Field states, contextual guidance, and a
non-mutating qualification-input preview. M-020C Transfer is represented with
its contract-local source/staging/interface/receiver topology, exact
requested-versus-accepted Route allocation evidence, literal workbench state,
and dual-renderer constraint marks. M-020D Buffer is represented with its
resolved periodic regime, exact supply cycle, Vault reserve events and
reservoir stance, minimum service-flow window, and contract-local Buffer
machine. M-020E is now the active Commission-loop packet: branch identity,
attempt retention, navigation closure, diagnosis comparison, named restart,
and the disabled qualification handoff. M-021 then introduces immutable
qualification results and the only progression derivation path. M-022 through
M-025 remain decomposed into source-owned packets in the canonical plan, with
M-024 visual/audio work paired to each mechanical packet rather than deferred
as a reskin. The user-requested validation hold remains in effect for the
automation queue; no build, test, browser, migration, replay, or human-play
claim is made for M-018 onward. The two scientific limitations above remain
explicit.
