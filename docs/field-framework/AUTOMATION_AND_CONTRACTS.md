# What Is Life 2 — Automation and Contract Authority

Status: approved implementation contract
Established: 2026-08-03
Revised: 2026-08-04
## See PRODUCT_NORTH_STAR.md for the current and most up to date and concise canonical scope document.

This document contains historical correction and remains canonical for full scope, but current implementaion goals and north star drift resistance is found in PRODUCT_NORTH_STAR.md
## Product identity

The player builds a physical transport generator, authors local automation,
observes the resulting behavior, diagnoses weak designs, and evaluates function
against a declared qualification suite where editing and rescue are disabled.

The generator is the object of attachment and mastery. The game does not need
characters, moral choices, or a narrative campaign to supply motivation.

The repeatable loop is:

1. read one exact commissioning contract;
2. inspect the opening Field while paused;
3. edit topology, physical membership, and local policies;
4. run at 1x, 4x, or 16x wall-time rate;
5. pause on a bottleneck or failure and inspect authoritative evidence;
6. revise or reset without waiting through already-understood operation;
7. freeze the generator and initial assembly;
8. execute accelerated qualification without external rescue;
9. archive the result, compare designs, or instantiate the blueprint elsewhere.

## Authority states

| State | Simulation | Allowed authority |
|---|---:|---|
| Design | Paused | Topology, policy, physical-compartment, and initial-assembly edits |
| Commission | 1x, 4x, or 16x | Pause, inspect, revise, reset, compare, and observe |
| Qualify | Accelerated cold path | No causal edits or rescue; result inspection after execution |

Design pause is immediate. The former Still Mode ramp and direct-control input
grammar do not belong to the automation product.

## Frozen local policy

Every programmable Component has one ordered policy of at most eight rules and
one fallback action. Policies are part of `GeneratorSpec`. Runtime selection,
target, timer, and cooldown state are embodied state.

```rust
struct FrozenLocalPolicy {
    version: u16,
    components: Vec<ComponentPolicy>,
}

struct ComponentPolicy {
    address: PolicyAddress,
    rules: Vec<PolicyRule>,
    fallback: LocalAction,
}

struct PolicyRule {
    condition: LocalCondition,
    action: LocalAction,
}
```

One step evaluates policy inputs from one beginning-of-policy-phase snapshot.
Components evaluate by stable address. The first true rule selects one action.
Targets tie-break by distance and then stable identifier. Actions are admitted
and applied in stable address order. No render, View, shell state, pointer
position, hidden criterion value, or qualification-harness fact is readable.

### Initial conditions

- `always`;
- own Charge below or above a declared fraction of capacity;
- own operating margin below a declared quantity;
- local Supply absent, present, emitting, or quiet;
- compatible target inside Coupling range;
- attached Route enabled, blocked, or flowing below/above a declared rate;
- local overload or declared pressure signal detected;
- decoded deficit, material, or status signal received;
- local timer elapsed.

Remote stock and global topology are not direct sensors. A remote shortage must
be communicated through a locally emitted and decoded signal.

### Initial actions

- hold position;
- move toward locally detected Supply;
- move toward a compatible Port or decoded signal;
- change depth toward a locally detected target;
- Couple with one in-range target;
- open or close an owned local interface;
- enable, disable, throttle, or weight an outgoing Route;
- emit a typed local signal;
- activate one implemented chassis ability.

An action that its Component cannot physically perform is unavailable in the
editor and invalid in content. Chassis abilities remain finite, conserving,
and subject to their current material, Charge, range, and cooldown rules.

## Route automation

Every Route retains immutable endpoints and authored capacity beside embodied
automation state:

```rust
struct RouteControlState {
    enabled: bool,
    capacity_limit: Fx,
    allocation_weight: u16,
    controller: PolicyAddress,
}
```

The synchronous one-hop allocator remains conserving. A disabled Route requests
zero. A limit caps its request. When outgoing demand exceeds source stock,
accepted requests divide proportionally by positive allocation weight and round
down; retained residue stays at the source. Destination headroom resolution
then follows the existing proportional rule.

## Contract content

`content/contracts/<id>.json` is the authored source. A ContractSpec declares:

- opening embodied Field and initial assembly;
- available hardware and construction limits;
- available policy conditions and actions;
- commissioning regime and visible disturbance schedule;
- qualification regimes, schedules, seeds/trial count, and duration;
- one exact function-criterion vector;
- four independent grade-band tables;
- prerequisites and unlocks.

Contracts contain no serial objective list. Tutorial guidance is contextual and
non-blocking. A solved condition can end qualification immediately instead of
forcing the player to wait through unused authored time.

## Contract ladder

| Order | Contract | Required capability |
|---:|---|---|
| 1 | Intake | Detect Supply locally and acquire enough Charge to maintain one receiver |
| 2 | Transfer | Open an interface, Couple, and sustain one directed Route |
| 3 | Buffer | Bank Charge and bridge repeated periodic Supply gaps |
| 4 | Balance | Allocate one source across competing outputs without overload |
| 5 | Closure | Sustain circulation while paying upkeep and compartment loss |
| 6 | Interference | Use signals and fallback rules through noisy or interrupted Routes |
| 7 | Renewal | Detect local failure, recruit material, position, and reconnect |
| 8 | Transplant | Preserve one frozen GeneratorSpec across declared regimes |
| 9 | Holdout | Pass a suite-committed hands-off evaluation without post-seal editing or rescue |

Intake through Buffer target 8–12 minutes each. Later contracts target 15–25
minutes. Early qualification targets no more than 20 seconds of wall time;
advanced suites may reach 90 seconds but report decision-relevant progress at
least every two seconds. No required passive commissioning wait exceeds 20
seconds of wall time.

Implementation remains sequential, but player availability branches after
Buffer: Balance and Interference unlock together; Balance leads to Closure;
Interference leads to Renewal; retained Closure and Renewal passes unlock
Transplant; and Transplant unlocks Holdout. The ladder presents this as literal
engineering prerequisites, not a map or campaign sequence.

## Evaluation

Pass/fail comes only from the exact criterion vector. The result also reports:

- Throughput: useful delivered flow against declared demand;
- Resilience: pass rate and worst retained service across declared trials;
- Economy: upkeep, leakage, overload, material, and intervention use;
- Complexity: Components, Routes, policy rules, and canonical policy bytes.

Each contract authors grade bands for each axis. No aggregate score, weighted
sum, scientific claim, or hidden adjustment is permitted.

A failure report identifies the first violated criterion, exact simulation
step, directly affected object when one exists, preceding state transitions,
and the relevant retained trace. It labels inferred contributors as inference
rather than claiming unsupported causation.

## Blueprint and persistence boundary

`GeneratorSpec` contains Component kinds, topology constraints, addressed
inputs, Route controls, and frozen policies. `AssemblyTemplate` separately
contains positions, initial stocks, material, open states, physical membership,
and other embodied initial conditions. A `BlueprintRecord` retains both
identities without conflating them.

Browser indexes and canonical persisted result bytes live in IndexedDB.
Contract availability is a rebuildable versioned projection of retained
complete-pass result ids, never an independent writable completion flag.
Canonical generator, assembly, scenario, embodied-state, control, RNG, and
evidence hashes remain independent. Legacy V3 runs and archive evidence remain
importable. A legacy campaign run may be inspected or exported but is not
silently converted into a contract.

## Player surface

The active workspace uses the Field as its dominant surface:

- top: contract criteria, Run/Pause, speed, reset, and Qualify;
- center: causal Field and direct topology manipulation;
- right: exact selected-object state and ordered policy editor;
- bottom: pressure, policy, criterion, and failure timeline;
- full workspaces: contract ladder, blueprint archive, comparisons, and labs.

Policy previews draw sensor range, eligible targets, selected target, and
actuator effect over the Field. Route artwork separately communicates physical
connection, enabled state, requested flow, accepted flow, capacity, and closed
endpoints. Pointer input selects and edits; it never steers.

## Visual and audio target

The visual object is a biological machine operating under load. Gates move when
interfaces change, reservoirs fill with actual Charge, paths articulate at
accepted flow cadence, and physical failure appears only where authoritative
embodied state records it. Criterion violations remain evaluator evidence and
do not fabricate fracture or depletion. The active policy state is restrained
on the Field and exact in inspection.

Audio is derived from transfer cadence, switching, load, fracture, and recovery.
Decorative Pulse cues, character-like chirps, and narrative stingers are not
part of the automation product.

## State ownership during implementation

The migration must preserve one authoritative simulation path. Ownership is
split as follows:

| Owner | Frozen or durable data | Mutable data |
|---|---|---|
| Rust `ContractSpec` | hardware limits, available policy grammar, regimes, schedules, criteria, grade bands, prerequisites | no live trial state |
| Rust `GeneratorSpec` | Component kinds and addresses, topology constraints, addressed inputs, installed policies, declared Route controls | no positions, stocks, timers, or cooldowns |
| Rust embodied state | initial assembly identity | positions, Charge, material, interfaces, active Routes, policy timer/cooldown/target, and Route-control state |
| Worker | protocol version and run identity | fixed-step accumulator, selected wall-time rate, cold-job progress, and transferable frame buffers |
| React | copy catalog keys and durable browser record schemas | policy drafts, current selection, panel layout, filters, and noncausal View state |
| Renderer | shared projection rules and quality budgets | interpolation, particles, highlights, and other noncausal presentation state |

React may stage a policy draft, but only Rust may admit it into the frozen
generator. The renderer may show a projected target or actuator envelope, but
the projection never becomes simulation input. Wall-time rate is never written
to a save, hash, replay, generator, scenario, or criterion record.

## Detailed implementation sequence

The milestone ledger remains the status authority. This section defines the
deliverables and dependency boundaries for M-018 through M-025 so implementation
does not rely on chat context.

### Program shape and critical path

The implementation is one continuous vertical build, not a set of detached
features. The critical path is:

```text
M-018 authoritative automation
  -> M-019 authoring and diagnosis
    -> M-020 first playable contracts
      -> M-021 frozen qualification
        -> M-022 reusable engineering records
          -> M-023 advanced contracts
            -> M-025 default product and legacy isolation
```

M-024 is not postponed until the end. Each causal slice receives its visual and
audio stance when that slice is implemented, and M-024 closes with a single
cross-product integration pass. This prevents a visually attractive shell from
describing mechanics that do not exist and prevents finished mechanics from
remaining unreadable until a later reskin.

Every slice must deliver four connected layers before the next dependent slice
is treated as implemented:

1. **Authoritative behavior:** Rust owns the state, transition, conservation,
   identity, and deterministic conflict rules.
2. **Carriage:** save, replay, worker protocol, cold analysis, and exact
   inspection carry the same meaning without a TypeScript simulation mirror.
3. **Player operation:** the shell provides the necessary authoring, selection,
   command, and evidence surfaces with all player copy in the catalog.
4. **Physical expression:** both renderers and the audio system communicate the
   action, target, load, result, and failure using authoritative readings.

The rapid pass follows dependency order and may leave verification deferred,
but it may not replace an absent lower layer with a UI-only approximation. A
later milestone may depend on an earlier implementation boundary while that
earlier milestone still awaits the deferred verification gate.

### Player-experience target

The intended repeatable loop is short and mechanical:

1. Read one literal service contract and the available hardware constraints.
2. Assemble or revise topology and ordered local policies in paused Design.
3. Commission at a chosen rate and watch the machine operate under visible
   load.
4. Pause at the first weak margin, blocked transfer, or wrong policy choice.
5. Select the affected object, trace the commanding rule and upstream state,
   and make one deliberate revision.
6. Reset from a declared boundary and compare the new run with retained
   evidence.
7. Freeze the candidate and Qualify it without intervention.
8. Archive, branch, compare, or transplant the resulting engineering record.

The source of satisfaction is visible control over a constrained physical
system: a compact policy becomes coordinated movement and transfer; bottlenecks
have readable causes; and a successful hands-off run is a consequence of the
player's organization. There is no character framing, quest fiction, or
campaign dependency carrying the experience.

### Mechanistic product identity

The product fantasy is not that the machine is a character. It is that the
player can design a compact autonomous system, watch its local rules become
physical behavior, understand exactly why it succeeds or fails, and refine it
into a robust engineering object. The Field is both machine and instrument.

The normal experience therefore emphasizes four forms of satisfaction:

1. **Command satisfaction.** A short ordered policy visibly coordinates
   sensing, movement, switching, storage, and transfer. The player sees the
   exact selected rule and the affected physical relation immediately.
2. **Diagnostic satisfaction.** A bottleneck can be traced from criterion to
   object, accepted or rejected action, upstream state, and commanding rule.
   Failure becomes an answerable engineering question rather than a generic
   loss state.
3. **Optimization satisfaction.** After function is achieved, independent
   evidence reveals whether a design is faster, more tolerant, less lossy, or
   simpler. No aggregate score erases those tradeoffs.
4. **Robustness satisfaction.** Qualification, transplant, and Holdout show
   whether an organization survives conditions the player cannot rescue or
   tune against during execution.

The plan excludes substitute engagement devices that would weaken that core:

- no character, mascot, fable, quest-giver, or story-map wrapper;
- no direct steering minigame competing with policy authority;
- no unexplained upgrade stat, loot tier, or cumulative power curve;
- no decorative animation or sound that implies control, flow, damage, or
  recovery absent from authoritative state;
- no long passive timer used to manufacture tension after the relevant result
  is already determined;
- no single score, celebration overlay, or hidden completion flag standing in
  for engineering evidence.

### Decision cadence and time budget

The core loop is paced around prediction and correction, not simulation wait.
Targets are product budgets; authored step counts remain authoritative and wall
time acceleration is presentation-only.

| Loop segment | Target | Required information |
|---|---|---|
| read contract | 30-60 seconds | function, limits, opening, available primitives, criteria, and receipt in literal language |
| first policy | 60-120 seconds for an experienced player | ordered rules, local inputs, target rule, action range/cost, and draft preview |
| first consequence | under 3 seconds after Commission at a useful rate | active rule, target, physical action, admitted result or named no-op |
| informative failure | under 45 seconds in early contracts | one visible weak margin or wrong action with an addressed diagnosis path |
| pause and explain | under 20 seconds | criterion -> object -> event -> rule traversal without searching unrelated panels |
| revise and restart | under 30 seconds | local edit, canonical diff, named reset boundary, and retained prior evidence |
| qualification proof | normally under 20 seconds wall time | frozen identities, coarse progress, criterion resolution, grades, and first violation |
| contract solve | 8-12 minutes early; 15-25 minutes advanced | at least one prediction, one informative failure, one revision, and one hands-off proof |

Early contracts deliberately present a design that is close to working but has
one legible defect. The player must make a meaningful policy or control choice;
the opening must not solve itself, and it must not require rebuilding the whole
machine before the first causal relationship can be understood. Advanced
contracts increase interaction among known constraints rather than lengthening
timers or multiplying unfamiliar controls.

### Mechanical engagement model

The automation game succeeds only when authoring, operation, diagnosis, and
revision form a tight reasoning loop. More content cannot compensate for an
opaque machine, and visual polish cannot compensate for a policy whose result
arrives too late to connect with the player's decision.

Every contract and free-build workflow must support these six qualities:

1. **Prediction.** Before Commission, the player can state what the first
   selected rule should do and which object it should affect.
2. **Short consequence latency.** A committed change produces its first visible
   mechanical consequence within several authoritative steps. Longer service
   windows may prove reliability, but they do not delay initial feedback.
3. **Causal legibility.** The Field communicates sensing, target, actuation,
   transfer, and blockage as separate states. The inspector confirms the same
   account with literal values and identities.
4. **Consequential constraints.** Capacity, range, stock, headroom, loss,
   cooldown, and local information limits create tradeoffs that cannot be
   bypassed by a universal policy or direct rescue input.
5. **Revisable failure.** A failed attempt identifies an actionable boundary:
   policy ordering, target eligibility, physical topology, Route allocation,
   storage, or regime tolerance. Restart time stays short enough to test one
   changed hypothesis.
6. **Optimization after function.** Passing proves service. Independent grades,
   blueprint comparison, and transplant provide reasons to simplify, reduce
   loss, improve margin, or increase resilience without turning the game into
   one opaque score chase.

The minute-to-minute rhythm is therefore:

| Moment | Player question | Required product response | Failure to avoid |
|---|---|---|---|
| read | What function and limits matter? | literal criteria, hardware, environment, and unlocks | fictional brief or hidden threshold |
| predict | Which rule should fire first? | ordered policy, readable inputs, projected target and reach | editor detached from the Field |
| commit | What changed? | atomic generator diff and named reset boundary | partial installation or silent retuning |
| observe | Is the machine doing that? | immediate physical stance and exact active rule/outcome | decorative motion with no causal meaning |
| diagnose | Where did service diverge? | first weak margin/event, addressed object, preceding transitions | generic red failure state |
| revise | Which single hypothesis can I change? | local draft, comparison against installed state, rapid restart | destructive reset or long repeated wait |
| qualify | Does it work without me? | frozen accelerated trials and immutable result | live rescue, hidden criterion, or mutable pass |
| improve | Is this design better, and in what way? | independent evidence axes and structural/policy diff | aggregate score replacing engineering judgment |

### Implementation-state vocabulary

Milestone status and code presence are not interchangeable. Every bounded slice
uses the following vocabulary in status notes and handoffs:

| State | Meaning | What may depend on it |
|---|---|---|
| specified | behavior, ownership, schema, and player consequence are written | implementation planning only |
| infrastructure present | core types or transport paths exist, possibly without a complete player path | adjacent source work that does not assume usability |
| vertical slice present | one real authored case crosses Rust, carriage, shell, and physical expression | the next dependent implementation slice |
| integrated | all declared cases and normal entry paths use the slice; compatibility ownership is explicit | later milestones during the rapid pass |
| verification-ready | source reconciliation is complete and the deferred gate has a bounded evidence list | validation work after the hold is lifted |
| validated | fresh compile, behavior, protocol, migration, browser, visual, and human-speed evidence appropriate to the slice has passed | milestone completion/publication |

During the validation hold, work may advance through `integrated`. It may not be
described as `validated`, human-ready, ship-ready, or complete. A source review
that reconciles types and call sites is implementation work; executing builds,
tests, browser play, screenshots, timing runs, or acceptance scripts belongs to
the deferred verification gate.

### Slice delivery packet

Each bounded slice carries one implementation packet. The packet prevents a
mechanic from existing only in Rust, only in React, or only as visual theater:

1. **Authoritative contract:** state owner, legal transitions, deterministic
   ordering, conservation behavior, capability limits, and named no-ops.
2. **Canonical identity:** stable addresses, schema version, canonical bytes,
   hash participation, and branch/migration effect.
3. **Persistence carriage:** new-run default, save, restore, replay, keyframe,
   analysis clone, export/import, and legacy behavior.
4. **Worker carriage:** request, response, event, frame, scheduling, cold-job,
   and error boundaries with no second simulation in TypeScript.
5. **Player operation:** entry, selection, edit, commit, reset, inspection, and
   failure-recovery path with catalog-backed copy.
6. **Physical expression:** Field geometry, exact inspector reading, timeline
   event, audio stance when useful, and renderer/quality-tier ownership.
7. **Content use:** at least one authored opening state that makes the mechanic
   necessary and one failure state that makes its consequence readable.
8. **Deferred evidence list:** precise compile, behavior, migration, browser,
   visual, performance, and human-speed checks to run when validation resumes.

An implementation packet is incomplete when any changed canonical record lacks
a migration decision, any runtime state lacks an inspection path, any prominent
mark lacks an authoritative source, or any command can partially mutate the
generator after rejection.

### Cross-milestone data contracts

The following records are the stable handoff boundaries for the roadmap. Later
milestones extend them by version; they do not create competing representations.

| Record | Authoritative owner | Identity and mutation rule | Primary consumers |
|---|---|---|---|
| `ContractSpec` | Rust content compiler | immutable, content-hashed; includes current capabilities, post-pass unlocks, opening, limits, schedules, criteria, grades, and prerequisites | ladder, run creation, qualification |
| `GeneratorSpec` | Rust run state | canonical/hashable; changes only through an admitted Design commit and creates a branch identity | policy runtime, reset, blueprint, qualification |
| `AssemblyTemplate` | Rust run boundary before qualification; blueprint reuse extends it in M-022 | canonical bytes/hash separate from generator; declares positions, stocks, material, interfaces, membership, and starting embodied state | restart, qualification, blueprint, transplant |
| `RunEnvelope` | Rust save/export boundary | explicit `AutomationContract`, `OpenField`, or `LegacyCampaign` kind plus schema/protocol identity; old semantics are preserved rather than inferred from save version | import dispatch, restore, legacy isolation |
| `AttemptRecord` | Rust canonical identity; browser persists opaque bytes and indexes | immutable attempt root addresses one contract opening and first generator/assembly pair | workbench, qualification source, lineage |
| `AttemptBranchRecord` | Rust canonical identity; browser persists opaque bytes and indexes | immutable branch id carries explicit parent id, operation provenance, generator/assembly outputs, evidence references, and RNG stream nonce; lineage is never inferred from nonce arithmetic | restart, commit, comparison, branch-back |
| `ComponentPolicy` | nested in `GeneratorSpec` | versioned, addressed, at most eight ordered rules plus fallback; disabled rows remain canonical | evaluator, editor, diff, complexity evidence |
| `PolicyDraft` | React workbench | ephemeral and based on one generator identity; never hashed, saved, replayed, or treated as active | editor and preview request |
| `PolicyPreview` | pure Rust preview response | ephemeral result against one paused snapshot and one draft; cannot actuate or mutate | Field preview and draft diagnostics |
| `PolicyRuntimeState` | embodied Rust state | mutable per step; never part of generator identity | frame/inspection, timeline, replay evidence |
| `RouteControlDefault` | chapter-addressed record nested in `GeneratorSpec` | immutable inside one generator revision; a complete current-chapter set is replaced only by an admitted Design commit | reset, diff, blueprint, qualification |
| `RouteControlState` | embodied Rust Field state | begins from the authored implicit opening or the committed chapter default; policy actions mutate only this live actuator | allocator, inspector, renderer, replay |
| `QualificationRequest` | Rust/worker cold boundary | immutable bundle of exact generator and assembly bytes, compiled criterion vector/hash, regime/schedule/control identities, source branch, build/protocol, RNG, and trial addresses | cold runner, result reproduction |
| `QualificationResult` | Rust result/Archive boundary | append-only evidence; cannot be rewritten by later grade or progression changes | result UI, progression, blueprint evidence |
| `BlueprintRecord` | IndexedDB archive boundary | immutable generator and assembly identities with explicit parent branch and linked evidence | clone, compare, transplant |
| `ContractProgressProjection` | deterministic Rust-owned derivation persisted/indexed by React | rebuildable versioned projection of retained complete-pass result ids; never an independently writable completion boolean | ladder and unlock filtering |

Three identity rules are non-negotiable:

- changing policy, topology, hardware, or Route defaults changes generator
  identity;
- changing starting position, stock, material, interface state, or physical
  membership changes assembly identity without rewriting generator identity;
- changing regime, schedule, criterion, seed, trial family, or build/protocol
  identity changes the qualification request and cannot reuse an earlier pass.

`ContractSpec` has a stricter compiler boundary than ordinary presentation
content. The source records may be concise, but the compiled record is complete
enough to create a run and a qualification request without consulting React or
legacy campaign progression:

| Contract section | Canonical meaning | Compiler obligation |
|---|---|---|
| identity | version, stable id, manifest order, content hash | one manifest entry and one exact source file; ids and order are unique and stable |
| presentation | title, function brief, success/failure, and contextual guidance copy keys | every key resolves in the copy catalog; no threshold or effect exists only in prose |
| prerequisites | earlier qualification records required for availability | references are acyclic, point only backward, and agree with `next_contract` ordering |
| capabilities | conditions, actions, hardware, instruments, regimes, and edit kinds available during this contract | every token belongs to a closed Rust vocabulary, resolves to one implemented capability owner, and is compatible with the opening hardware and construction limits |
| unlocks | conditions, actions, hardware, instruments, regimes, and next contract exposed after pass | every token belongs to a closed Rust vocabulary and has one capability owner |
| opening | regime, assembly declaration, generator baseline, visible schedule, and initial Route defaults | every addressed object/Route exists; opening generator and assembly compile to separate canonical identities |
| construction limits | total Components, total Routes, per-Component rules, material, stock, cost, and permitted edit kinds | limits have total-machine semantics and are never lower than the authored opening; player-added budgets use separate fields |
| commissioning | expected reasoning duration, first-consequence target, acceleration options, and maximum required wall wait | schedule is presentation-only where appropriate and never changes authoritative step duration |
| qualification | trial family, duration, hands-off window, retention, early termination, schedule/seed custody, and criterion vector | every criterion has units, comparison, addressed source, observation window, and pass aggregation rule |
| grades | Throughput, Resilience, Economy, and Complexity evidence bands | exactly four monotone authored bands per axis; grades cannot rewrite criterion pass/fail |

The initial source may refer to a legacy chapter/Form pair only through an
explicit `LegacyOpeningRef`. Rust resolves that reference into a canonical
contract opening during M-020A. The worker and shell receive the resulting
contract, generator, assembly, and regime identities; they do not receive a
chapter objective or derive contract progress from one. Once all nine openings
are authored directly, the adapter remains only for supported legacy imports.

The initial source vocabulary is closed before Intake is exposed. Adding an
action or condition requires a Rust capability implementation, admission rule,
inspection vocabulary, renderer stance when visible, copy, and schema-version
decision. Adding a JSON string alone never unlocks behavior.

### Command, preview, and carriage boundaries

Design editing uses revision-aware transactions. A draft begins against an
exact generator identity. Preview and commit both carry that base identity so a
stale editor cannot overwrite a newer accepted design.

The initial command contracts are:

```text
preview_design_patch(base_generator_hash, selected_address,
                     complete_policy, complete_route_defaults)
  -> preview(snapshot_step, candidates, selected_target, projected_action)
  -> rejected(field, physical_limit)
  -> stale_base(current_generator_hash)

commit_design_patch(base_generator_hash,
                    complete_policy, complete_route_defaults)
  -> committed(generator_hash, scenario_hash, canonical_policy,
               canonical_route_defaults, canonical_diff)
  -> rejected(diagnostics, retained_generator_hash)
  -> stale_base(current_generator_hash)
```

`preview_design_patch` runs the same Rust capability, local readability, target
ordering, and admission helpers used by live policy evaluation, but it performs
no actuation and writes no runtime state. The shell may add immediate structural
diagnostics such as an empty selector or value outside an input's representable
range, but it may not choose a target, infer a capability, or predict transfer.

`commit_design_patch` is available only in Design. The first integrated shape
carries the complete policy and complete Route-default set for the current
chapter. Each Route default names its stable Route, controller, enabled state,
capacity limit, and positive allocation weight. Later topology/assembly editors
join this transaction with explicit operations rather than creating a second
commit path. Rust validates the entire patch first, canonicalizes it once, and
either returns one new generator identity or retains the prior generator
byte-for-byte. A commit never advances a simulation step or silently restarts
the assembly. `Restart Commission` is a separate named command: it becomes
available only after acceptance and projects the committed defaults into the
contract's opening assembly.

The contract and qualification boundaries build on the same identity discipline:

```text
resolve_contract_catalog(retained_results)
  -> contracts(id, order, availability, reason, function, limits,
               capabilities, criteria, unlocks, opening_identity)

open_contract(contract_id, retained_results)
  -> opened(contract_identity, attempt_id, generator_hash, assembly_hash,
            regime_identity, canonical_route_defaults, authority=design)
  -> locked(missing_prerequisite_ids)

start_qualification(contract_identity, generator_hash, assembly_hash)
  -> accepted(qualification_request_id, frozen_input_summary)
  -> rejected(stale_or_incompatible_identity, diagnostics)

qualification_progress(request_id)
  -> progress(completed_trials, total_trials, resolved_trials)

qualification_finished(request_id)
  -> result(result_id, criterion_vector, grades, first_violation,
            trace_ids, unlock_receipt)
```

`resolve_contract_catalog` treats retained qualification results as evidence,
not booleans supplied by the shell. Rust checks result identity, content
identity, contract id, and complete criterion pass before returning an unlocked
contract. Before M-021 produces such results, Intake is the only normally
available contract and later records display their literal prerequisite.

The catalog command is valid before a run exists. `open_contract` is the first
normal command that creates RunState: it creates a new attempt identity and
returns Design authority. It does not carry embodied state from another
contract, silently install a sample policy, or count an old chapter objective.
`start_qualification` accepts only the current committed generator and declared
assembly identity. Once accepted, the request is immutable; cancellation may
end execution as an incomplete run but can never create a failed or passing
qualification record.

### Authority and navigation state machine

Normal automation navigation is a state machine over authoritative run and
evidence identities. A React route is never sufficient evidence that a branch
opened, closed, resumed, qualified, or progressed.

| State | Authoritative payload | Legal player transitions | Prohibited transition |
|---|---|---|---|
| Catalog idle | contract catalog plus retained result projection; no RunState | open an available contract, inspect a result/blueprint, import supported data | creating a hidden campaign or default contract run |
| Design | one open attempt branch, committed generator, assembly template, paused embodied state, optional local draft | preview/commit a draft, edit admitted topology, Restart Commission, Return, Qualify | advancing simulation or treating a draft as generator state |
| Commission | the same open branch plus advancing embodied state and addressed events | Pause to Design, arm presentation-only breakpoint, select/inspect, Return after closure | topology/policy edits, direct steering, qualification mutation |
| Returned | retained closed branch and no advancing live session | Resume as an explicit child branch, open another contract, inspect retained evidence | silently continuing the already closed branch |
| Qualifying | immutable request plus cold-job execution state | inspect coarse progress, cancel before a result where legal, leave and reopen the job | live Field command, policy edit, rescue, schedule disclosure to policy |
| Result | immutable request, per-trial artifacts, aggregate decision, grades, and trace indexes | inspect, branch to Design, save blueprint, compare, return to ladder | rewriting result, granting progress from an incomplete or failed record |
| Legacy inspection | labeled imported campaign envelope and original evidence | inspect, replay through the owned compatibility path, export | entering contract commands, qualification, blueprint, or progress projection |

The state transitions own these identity effects:

| Transition | Attempt effect | Branch effect | Evidence effect | Progress effect |
|---|---|---|---|---|
| Catalog -> Open contract | create one attempt root | create opening branch with no parent | none until the branch closes | none |
| Design -> Commission | retain attempt | retain branch | append addressed live events | none |
| Commission -> Pause | retain attempt | retain branch | retain latest event and criterion window | none |
| Design/Commission -> Restart | retain attempt | close current branch, create `restart` child, advance stream nonce | persist old branch as `restart` | none |
| Design -> admitted commit | retain attempt | close prior design branch and create `design_commit` child | retain old generator/diff boundary | none |
| Design/Commission -> Return | retain attempt | close current branch; no live continuation | persist branch as `returned` | none |
| Returned -> Resume | retain attempt | create `resume` child of returned branch with an explicit new stream ordinal | retain prior record unchanged | none |
| Any open branch -> Open another contract | close old attempt's branch; create new attempt | old branch `superseded`; new opening branch | persist old closure before replacing session | none |
| Design -> Qualify | retain attempt and source branch reference | current branch remains the source candidate and is no longer mutable through the request | persist immutable request and execution artifacts | only a later complete pass result can project progress |
| Result -> Branch to Design | create or retain the addressed attempt according to clone choice | create explicit result-derived child | result remains immutable | none |

`branch_nonce` is an RNG stream ordinal. It does not encode parentage, branch
depth, chronology, or closure. `parent_branch_id` and operation provenance are
mandatory for every non-opening automation branch. Migration may preserve an
unknown parent as unknown; it may not infer one by subtracting a nonce.

Evidence closure and child provenance remain separate vocabularies:

| Accepted command | Closed evidence reason | Child branch operation |
|---|---|---|
| Restart Commission | `restart` | `restart` |
| Commit policy or Route defaults | `superseded` | `design_commit` |
| Commit topology | `superseded` | `design_commit` with the canonical structural diff |
| Return to ladder | `returned` | none until an explicit resume |
| Resume returned attempt | no new closure; parent is already closed | `resume` |
| Open another contract | `superseded` | `opening` on a new attempt root |

The closure reason answers why the retained evidence window ended. The branch
operation answers how a new candidate was derived. They are not merged because
one old branch may be superseded by several kinds of admitted descendant.

### Durable transition protocols

Branch-closing commands use one prepare-command-persist sequence so the Archive
cannot claim that a rejected mutation replaced the branch:

1. **Prepare closure.** Snapshot the current canonical attempt and branch
   records, identities, opening/closing steps, exact embodied-state hash,
   bounded addressed event window, provisional criterion vector, generator
   diff, and intended closure reason. This is an in-memory candidate only.
2. **Execute command.** Ask Rust to admit and execute the restart, design commit,
   topology commit, return, resume, or contract replacement. Rust returns the
   complete post-transition identity or an exact rejection.
3. **Persist old branch.** Only after command acceptance, add the prepared
   closure, canonical attempt, and canonical branch records with immutable
   add-if-absent semantics. Repeating recovery of the same identity is
   idempotent and cannot alter canonical bytes.
4. **Publish new session.** Replace the shell identity, reset branch-local
   event buffers, register the new opening step, and refresh history. If durable
   storage fails, keep a volatile copy and expose degraded durability; never
   fabricate progression or silently discard the closure.

Return is the one transition that can persist before live-state disposal
because there is no following Rust mutation. Resume never reopens that closed
branch. It creates a child whose parent is the retained returned branch and
whose generator, assembly, and embodied boundary are named by the resume
command.

Qualification uses a separate append-only transaction:

1. Persist the canonical request and source identities as `prepared`.
2. Execute fresh trials through Rust and append addressed trial artifacts.
3. Persist the aggregate criterion decision only after all required trial
   resolutions or an authored monotone early-resolution proof exist.
4. Persist four independent grade vectors and labels after pass/fail is locked.
5. Mark the request `resolved` or `invalid_execution`; cancellation remains
   `incomplete` and creates neither pass nor failure evidence.
6. Rebuild the progression projection from immutable complete-pass result ids.
   Store the derivation version and ordered source ids so recovery can repeat
   the projection without creating a second completion authority.

Deleting a browser display row removes only a disposable index or explicit
user-owned record when the product supports deletion. It cannot turn a result
into another result, mutate a blueprint ancestor, or convert an incomplete job
into progress.

Runtime data is divided by cadence and purpose:

| Channel | Carries | Does not carry |
|---|---|---|
| frame | continuously visible compact state: position, stock/fill, active policy/action/outcome/target, Route enable/limit/request/acceptance | verbose diagnostics, full rule arrays, historical events |
| inspection | exact selected-object state and relationships at a requested step | continuously duplicated data for every unselected object |
| addressed event | transitions needed for timeline and first-violation evidence | per-frame animation samples or shell-only hover state |
| command response | canonical admission/rejection, identity change, and reset choices | optimistic mutation or partial success |
| cold-job progress | qualification request identity and coarse completed/remaining counts | normal live Field frames or hidden trial schedules |
| save/export | canonical durable specification, embodied state, control/RNG state, and supported evidence indexes | policy drafts, interpolation, panel state, or wall-time rate |

Frame sections remain append-only within a protocol version and declare record
size explicitly. Unknown optional sections may be skipped by a compatible
reader; changed meaning requires a new section kind or protocol version. Worker
decoders merge sections by stable identity and never synthesize a nonzero
capacity, enabled control, selected target, or action outcome when a section is
absent.

### Execution waves and merge order

The milestone numbers remain the authority, but implementation proceeds in
bounded waves so dependent infrastructure and its player-facing use arrive
together. A wave may start source work on its next item after the predecessor
reaches `vertical slice present`; it may not claim integration before every
listed handoff exists.

| Wave | Sequential mechanical work | Parallel M-024 work | End-of-wave player capability |
|---|---|---|---|
| A - command loop | close M-018 carriage and capability semantics; finish M-019C policy lifecycle and diagnostics | active policy, selected target, action outcome, Route control, live-versus-draft stance | author one valid policy, run it, and explain the exact result |
| B - diagnostic loop | M-019D preview, M-019E commit/reset queue, M-019F timeline/responsive workbench | sensor/actuator envelopes, event localization, stable desktop/narrow composition | predict, Commission, pause, trace, revise, and restart quickly |
| C - first service | M-020A schema/loader, then Intake, Transfer, and Buffer in order | acquisition, gates/request/acceptance, reservoir/input-phase grammar | commission three increasingly capable machines from the ladder |
| D - trust loop | M-021 freeze, cold runner, criteria, grades, failure evidence, and progression | sealed qualification instrument, criterion lock, result and trace stance | prove a frozen candidate and receive literal progression |
| E - engineering memory | M-022 blueprint record, resets, branch, compare, and transplant | real-projection thumbnails, structural/policy diff, lineage, regime comparison | preserve and compare design reasoning across attempts |
| F - systems ladder | M-023A through M-023F sequentially | contract-specific allocation, circulation, signal, renewal, transplant, and holdout states | solve the complete nine-contract systems ladder |
| G - product default | M-025 routing, campaign isolation, cleanup, guide, and normal-copy pass | final Number 2 integration, audio mix, loading/error/legacy states | enter and play only the automation product by default |

Within each wave, merge order is authoritative schema and behavior, persistence
and worker carriage, shell operation, physical expression, authored content,
then deferred-evidence documentation. Visual work may begin as soon as the
authoritative readings exist; it does not wait for the entire milestone.

### Revised packet contract

Every remaining packet is planned against one explicit input and produces one
handoff. The packet is not source-integrated until all required rows are
represented; validation remains a later and separately reported gate.

| Packet concern | Required plan detail | Source integration question |
|---|---|---|
| player hypothesis | the decision the player is expected to make and the competing viable choice | can the player predict a different consequence before committing? |
| authoritative delta | exact Rust-owned records, fields, transitions, ordering, and named no-ops added or changed | can React and the renderer be deleted without changing the outcome? |
| identity delta | hashes, stable addresses, parentage, schema/protocol versions, and migration behavior | can old and new records be distinguished without heuristics? |
| persistence delta | new-run, close, restart, save/restore, replay, export/import, recovery, and deletion behavior | can interruption recover without fabricating or overwriting evidence? |
| worker delta | commands, responses, events, frames, cold jobs, and legal authority states | is TypeScript carrying rather than recomputing the result? |
| operation flow | entry, selection, edit, commit, rejection, cancel, retry, return, and resume | does every command leave the player in a named state with a next action? |
| diagnosis flow | criterion, object, event, rule, upstream quantity, and retained comparison path | can the first actionable mismatch be reached without panel hunting? |
| physical stance | machine anatomy, motion, inspector, timeline, audio, low-quality fallback, and reduced modes | do all channels communicate the same authoritative state? |
| content obligation | opening, limits, first consequence, informative failure, retry boundary, criteria, grades, and receipt | does content require the mechanic without forcing one hidden solution? |
| handoff | exact record or capability the next packet may depend upon | can the successor start without inventing missing state or reading chat history? |

The source status terms are `specified`, `source present`, and `integrated`.
`Validated`, `published`, and `ready for human testing` remain unavailable while
the user-directed validation hold is active.

### Detailed remaining execution order

The revised order follows the shortest dependency path from the current
commissioning source to the full product:

1. **E-00/E-01 identity closeout.** Reconcile every constructor, migration,
   open, import, restart, commit, rebranch, reopen, and response path with exact
   versus migrated AssemblyTemplate stance and explicit branch provenance.
2. **E-02 immutable attempt storage.** Finish additive stores, canonical
   add-if-absent semantics, volatile parity, branch opening-step custody, and
   retained event/criterion capture.
3. **E-03 closure-aware navigation.** Route restart, return, resume, contract
   replacement, Design commit, and topology commit through the durable
   transition protocol above.
4. **E-04 diagnosis and comparison.** Add branch history, current-versus-parent
   margin/event alignment, breakpoint custody, and literal observed-delta
   language.
5. **E-05 reset ownership.** Bind restart to current committed GeneratorSpec
   plus exact contract AssemblyTemplate; expose every kept/restored identity.
6. **E-06 qualification preview.** Produce the complete non-mutating request
   summary and keep all pass/progression paths absent.
7. **Q-01 through Q-07.** Freeze, execute, resolve criteria, calculate separate
   grades, retain failure traces, persist results, and rebuild progression in
   that order.
8. **M-022A through M-022G.** Complete engineering records and assembly capture,
   then add guarded transitions, clone lineage, normalized diffs, compatibility,
   assembly adaptation, and comparative transplant evidence.
9. **M-023-00, A, C, B, D, E, F.** Establish shared evidence semantics, then
   deliver each contract as opening/capability, authoritative pressure and
   minimum projection, shared player loop, and fidelity refinement before its
   dependent contract begins.
10. **P-03, P-01, M-024X, P-04, P-02, P-05.** Establish legacy import ownership,
    make the ladder the normal startup root, land replacement surfaces, retire
    compatibility and old controls, then publish the automation playtest
    contract.

M-024 is attached to each numbered step. Visual implementation does not wait
for M-024X, and no mechanical packet hands off while its new state remains only
a text reading or a decorative approximation.

### Immediate implementation packets

The policy lifecycle, first three contracts, commissioning closure, frozen
qualification, independent grades, immutable results, and receipt-derived
progression are represented in source. The remaining critical path starts at
M-022 by completing assembly/blueprint transaction semantics and replacing the
partial reset mutation with preview/commit/recovery before clone, diff,
transplant, advanced-contract, or product-cutover work depends on it.

| Packet | Required input | Authoritative output | Player-visible output | Source-level completion boundary |
|---|---|---|---|---|
| C-01.R1 capability split | compiled ContractSpec v2 | separate `capabilities` envelope and post-pass `unlocks` receipt, each with closed vocabulary and dependency checks | ladder and editor distinguish "available now" from "earned after pass" | no current-contract menu or admission rule reads the unlock receipt; no receipt is shown as already installed |
| C-01.R2 opening identity | contract-local opening declaration | canonical `GeneratorSpec`, separate canonical `AssemblyTemplate`, regime identity, complete Route defaults, and new attempt id | exact opening identity and reset boundary in ladder/workbench | open/restart/qualification-preview responses carry all identities; changing assembly-only data does not alter generator identity |
| C-01.R3 catalog-first session | loaded content and retained progress evidence | idle catalog session with no RunState until `open_contract` | application opens directly on the ladder without Atlas, Form, chapter, or hidden legacy run creation | worker can list contracts while idle; only an explicit open creates the first contract attempt |
| M-020B1 Intake assembly | corrected opening compiler | one local collector, visible Supply, maintained receiver, exact initial stocks/interfaces/routes | literal opening annotations and selected-object readings | all criterion addresses and available capabilities resolve against the compiled opening |
| M-020B2 Intake policy | Intake capability envelope | local Supply sensing, move-toward, Couple, Charge threshold, fallback, and named no-ops through the existing evaluator | authorable minimal policy with preview and exact runtime decision | every exposed primitive has hardware, sensor, range, admission, inspection, and copy ownership |
| M-020B3 Intake causal stance | runtime/action/ledger events | addressed sense, target, move, contact, transfer, storage, upkeep, and loss evidence | distinct acquisition animation, receiver reservoir response, event trace, and local failure state | prominent Field states agree with inspector/timeline and do not infer admission or transfer in React |
| M-020B4 Intake guidance and submission preview | exact runtime states and compiled criterion | deterministic cue predicates and canonical qualification-input summary | dismissible object-addressed cues plus literal criteria/trial/grade/unlock preview | guidance never blocks control, advances progress, or substitutes for a criterion; Qualify remains non-mutating |
| M-020C Transfer | Intake handoff plus interface/Route capabilities | compiled endpoint ownership, switching, Route defaults, flow criterion, and sustained-service window | readable gate motion and separate requested, accepted, throttled, blocked, starved, and headroom-limited states | Transfer uses the same evaluator, attempt model, screen, submission preview, and reset path as Intake |
| M-020D Buffer | Transfer handoff plus periodic input/storage capability | compiled emitting/quiet schedule, finite reserve, complete-cycle criterion windows, leakage, and recovery evidence | readable phase, reservoir fill/discharge, service floor, leakage, and recovery across a full gap | Buffer uses the same contract/attempt/result-input schemas and adds no shell timer or private pass rule |
| M-020E commissioning closure | all three complete contract slices | durable attempt/branch index, restart boundary, retained commissioning evidence, and qualification request preview | enter, Commission, pause, diagnose, revise, retry, return, and compare attempts without losing prior evidence | every normal pre-qualification route works from the ladder; no pass, grade, receipt, or next-contract mutation exists yet |
| M-021 qualification | canonical request inputs from M-020E | immutable request, cold authoritative trial records, criterion decision, four typed grade records, first violation, trace ids, result, and rebuildable progression projection | freeze/execute/resolve/inspect flow with literal pass/fail and unlock receipt | availability derives only from retained complete-pass result ids after result persistence succeeds |
| M-022 engineering memory | stable generator/assembly/result identities | blueprint, lineage, compatibility, diff, reset, branch, and transplant records | named resets, structural/policy comparison, branch-to-Design, and regime transplant | no command silently changes generator, assembly, evidence, or compatibility status |
| M-023-00 evidence substrate | M-022 record and transition identities | Rust-owned typed metric/source/unit extensions, qualification artifacts, comparative evidence, and versioned addressed mechanism frame/event semantics | no standalone screen; existing inspector/result surfaces gain literal support for later evidence families | no advanced contract needs a shell-derived metric, private criterion source, or renderer-inferred mechanism state |
| M-023A-F systems ladder | M-022 records and shared qualifier | six additional ContractSpecs using one policy, attempt, result, progress, and blueprint system | Balance, Interference, Closure, Renewal, Transplant, and Holdout in prerequisite order with one new reasoning problem each | each contract has one complete opening-to-result path and its M-024 causal companion before the next contract depends on it |
| M-025 product default | complete nine-contract ladder | explicit automation startup graph and isolated legacy import graph | no campaign, steering, Pulse, Still, objective, or unexplained narrative term on a normal path | clean launch completes ladder -> Design -> Commission -> Qualify -> result -> blueprint/next-contract without entering legacy state |

Three plan corrections are binding:

1. **Capabilities are not unlocks.** `capabilities` describe exactly what the
   selected contract permits during Design and Commission. `unlocks` describe
   the receipt produced only after an immutable passing result. A capability may
   have been earned by a prerequisite, but its current availability is compiled
   explicitly and is never inferred in React from the prior receipt.
2. **A run is not an opening.** `ContractSpec` points to an immutable opening
   declaration. Opening creates a new `AttemptRecord` that addresses a frozen
   generator baseline and a separate assembly template. Restarting embodied
   state, committing a generator revision, or branching an attempt changes only
   the identity owned by that operation.
3. **The catalog is a valid session state.** Content inspection and progress
   resolution do not require an initialized legacy campaign run. The worker
   accepts catalog/open/import commands while idle; scheduling begins only after
   a contract or explicit legacy record creates a run.

The C-01.R source slice now represents those three corrections. Its assembly
identity is presently the canonical hash of the compiled opening and is carried
separately from the generator identity. M-022 still owns the stronger record
boundary: an explicit `AssemblyTemplate` schema whose assembly-only revisions
cannot change `GeneratorSpec`. Until then, the hash is an address and reset
anchor, not a claim that selective assembly persistence is complete.

M-020B used the following fixed internal merge order:

1. `M-020B1` makes the Intake opening and its addresses literal in the
   workbench: contract function, authority, attempt/generator/assembly identity,
   receiver, Supply, stocks, interface state, and current weakest criterion.
2. `M-020B2` constrains authoring to Intake hardware and contract capabilities,
   supplies a valid minimal starting policy, suppresses Route controls that the
   contract does not grant, and keeps canonical Rust admission decisive.
3. `M-020B3` projects sensed Current, selected target, approach, contact,
   accepted transfer, receiver storage, upkeep, and loss as distinct states
   across the Field, inspector, and addressed event trail.
4. `M-020B4` derives dismissible guidance from authoritative readings and shows
   the exact immutable qualification input that M-021 will later execute:
   trials, duration, grace, criteria, windows, grade bands, and receipt. The
   preview cannot run qualification or mutate progress.

M-020B is now represented at source level and hands off to M-020C. Its opening
is an explicit inclusion subset over the temporary chapter adapter, so catalog,
generator, assembly hash, open, restart, criteria, and the visible service chain
all address the same collector/receiver/reserve/Route/Current machine. Protocol
10 carries sampled exact Charge-ledger records for continuous mechanisms, while
accepted Supply also raises an authoritative addressed cue for the renderer.
No validation state advances with this handoff.

M-020C is now represented at source level and hands off to M-020D. Transfer's
opening retains exactly Current 1, Components 2/3/4/5, Routes 2/3/4, and the
Relay Form. Route runtime is a transient authoritative record rebuilt by every
step and intentionally omitted from canonical persistence: it carries the
post-control request, accepted transfer, and one closed causal outcome. This
keeps the generator/assembly save identity stable while giving the current
frame, exact inspector, and evidence trail the same latest-step answer.

The Transfer workbench names the complete supply-to-receiver chain, prepares
Open Interface policies for stationary Components, permits committed Route
defaults under the contract's Route actuator, localizes limiting guidance to
the authored accepted-flow Route, and includes the unchanged qualification
input preview. M-024C is integrated with that packet: tail, head, and midline
constraint marks distinguish source, destination, and capacity limits in both
renderers. Progress remains read-only and no qualification trial is executed.
The deferred evidence packet must later establish compilation, deterministic
replay, save/reopen behavior, browser composition, and human readability.

M-020D is now represented at source level and hands off to M-020E. Buffer's
compiled opening contains the Vault, Components 2/3/4/5, Routes 1/2/3, and
Current 1 under `periodic_transport`. The resolved opening applies the regime
before its generator/assembly identities and catalog facts are computed, and
contract Runs do not apply it a second time. The catalog therefore carries the
same 15-step emitting and 15-step quiet cycle that the Field executes.

The Vault's prepared policy seeks Supply during emission, returns to nearby
Components in quiet input, and releases isolated reserve in physical range.
Reserve level is exact inspection state; bank and release are addressed
mechanism events; the frame normalizes the reservoir arc to the Vault's finite
capacity; and both renderers add a distinct bank/release motion ring. Route
service uses the same request/acceptance outcomes as Transfer. The live rolling
criterion now enforces the contract's minimum accepted-flow aggregation rather
than allowing a favorable mean to conceal one failed service step, while still
reporting the mean for diagnosis. The workbench names the complete five-stage
cycle and derives phase, gate, bridge, receiver, and recovery guidance only
from authoritative readings. Progress and qualification remain read-only.

M-024 is attached at the packet level. Intake cannot hand off to Transfer until
its sensing, targeting, movement, contact, transfer, storage, upkeep, and loss
states are physically distinct. Transfer cannot hand off to Buffer until gate,
request, acceptance, throttling, source shortage, and headroom are distinct.
Buffer cannot hand off to qualification until input phase, reserve, service,
leakage, and recovery are distinct. This prevents the visual lane from becoming
a late cosmetic pass while still keeping all physical marks derived from the
authoritative scene projection.

### M-018 — Deterministic automation foundation

**Outcome:** every programmable Component operates from a persisted local policy
and every automated action is resolved by the authoritative Rust transition.

Implementation slices:

1. **Schema and ownership.** Add versioned `FrozenLocalPolicy`, addressed
   `ComponentPolicy`, embodied `PolicyRuntimeState`, and embodied
   `RouteControlState`. Install policies in `GeneratorSpec`; retain active rule,
   target, timer, and cooldown only in embodied state. Migrate legacy saves to
   an empty policy without inventing behavior.
2. **Snapshot and evaluation.** At the beginning of the policy phase, freeze the
   locally readable state. Evaluate Components by ascending stable address, use
   first-true-rule ordering, resolve targets by distance then identifier, and
   produce at most one proposal per Component.
3. **Admission and actuation.** Reject capability-incompatible authored actions.
   Apply admitted proposals by stable address through the existing movement,
   depth, Coupling, interface, Route, signal, and chassis-ability paths. A target
   that becomes unavailable before application produces a named no-op; it does
   not retarget from later state.
4. **Route control.** Reconcile one control record per active Route. Disabled
   Routes request zero; enabled Routes respect capacity limits; oversubscribed
   outgoing stock divides by positive weight and retains rounding residue at the
   source before destination-headroom resolution.
5. **Runtime bookkeeping.** Increment local timers once per authoritative step,
   reset a timer only when its elapsed rule fires, decrement ability cooldowns,
   and retain the last selected rule, target, and action outcome for inspection.
6. **Persistence and protocol.** Carry installed policy through open, restore,
   export, import, replay, keyframe, analysis clone, and worker response paths.
   Carry exact runtime readings through field inspection without enlarging the
   per-frame hot path unless a visible mark requires it.
7. **Manual-input boundary.** Stop opening steering, wheel, Pulse, and depth
   sources in normal automation play. Preserve legacy input decoding only for
   imported campaign runs and explicit diagnostics until M-025 removes the
   primary entry path.

Authoritative policy phase order:

| Phase | Required behavior | Retained evidence |
|---|---|---|
| 1. Snapshot | Read only the beginning-of-phase local state available to each address | policy-step identity and readable sensor values |
| 2. Match | Scan enabled rules from first to last; choose the first true rule or fallback | selected rule index and condition result |
| 3. Resolve | Resolve one target from the snapshot by eligibility, distance, then stable id | target kind, target id, and candidate reason |
| 4. Admit | Check hardware capability, ownership, range, stock, cooldown, and target validity | admitted action or named rejection |
| 5. Apply | Execute proposals by ascending Component address through existing mechanics | applied/no-effect/no-target/unavailable outcome |
| 6. Allocate | Resolve Route requests, source shortage, destination headroom, and residue conservingly | requested, accepted, throttled, and retained amounts |
| 7. Record | Advance timers/cooldowns and retain the exact decision for inspection and replay | action, outcome, timer, cooldown, target, rule |

The initial named outcome vocabulary is closed and inspectable: `idle`, `held`,
`applied`, `no_target`, `target_unavailable`, `wrong_layer`, `out_of_range`,
`no_effect`, `cooldown`, `capacity_reached`, and `unavailable`. The UI may
translate these through copy keys but may not merge them into an ambiguous
failure state.

The M-024 companion slice adds restrained policy-state marks to programmable
Components, explicit target relations, enabled/limited/disabled Route stances,
and action-result accents. Those marks are derived from the runtime record; the
renderer never repeats target selection or action admission.

Handoff to M-019: the core and protocol can answer, for any programmable
Component or controlled Route, "which rule selected what action against which
target, what happened, and why?" without reading renderer or shell state.

Completion condition: the same generator and initial assembly produce the same
policy decisions, targets, transfers, and identities after reset, save/restore,
and replay; every active action and no-op reason is inspectable; conservation
still passes through the existing ledger.

### M-019 — Design and Commission workbench

**Outcome:** the player can author and understand automation without touching a
direct-control input.

Implementation slices:

1. **Authority controls.** New runs open in Design. Commission starts
   immediately from the committed design. Pause returns immediately to Design.
   `1x`, `4x`, and `16x` alter only worker scheduling. Reset restores the
   contract opening assembly and last committed generator separately.
2. **Selection and exact state.** Pointer selection covers Forms, stationary
   Components, Routes, Supply Streams, compartments, and Views. The right rail
   reports literal capacity, stock, margin, interface, policy rule, timer,
   cooldown, target, action, Route limit, weight, requested flow, accepted flow,
   and any no-op reason that applies.
3. **Ordered policy editor.** The editor supports insert, delete, reorder, copy,
   disable, parameter edit, and fallback edit for at most eight rules. The
   available condition/action menus are filtered by contract unlocks, hardware
   capability, attachment, and ownership. Invalid drafts remain local and name
   the exact conflict before installation.
4. **Field preview.** Selecting a rule projects only its readable sensor area,
   eligible targets, deterministic selected target, owned interfaces, outgoing
   Routes, and actuator envelope. Preview labels distinguish a projected result
   from the currently active runtime result.
5. **Construction surface.** Topology, initial interface state, stock/material,
   and physical-compartment edits use one queue with exact cost and consequence,
   undo, discard, and atomic commit. Observation View edits remain free,
   immediate, and visibly noncausal.
6. **Evidence timeline.** The bottom band aligns Supply phase, pressure changes,
   policy transitions, Route state, criterion windows, interventions, and first
   failure. Selecting an event moves inspection to its object and exact step.
7. **Responsive interaction.** Desktop keeps Field, policy rail, and timeline
   simultaneously legible. Narrow layouts give the Field priority and expose
   editor/timeline as dedicated sheets without obscuring primary controls.
   Keyboard focus, pointer selection, and text labels remain complete; pointer
   motion never steers.

Authority-state behavior is explicit:

| Command or edit | Design | Commission | Qualify |
|---|---:|---:|---:|
| Edit topology, stock, material, interfaces, or policy | yes, queued | no; pause first | no |
| Move or resize a passive View | yes, immediate | yes, immediate | no live trial surface |
| Select and inspect an object | yes | yes | result evidence only |
| Advance authoritative steps | no | yes, at `1x`/`4x`/`16x` | cold runner only |
| Reset or branch | yes | yes, after pause | only from retained result |
| Rescue or inject direct control | no | no | no |

The workbench is organized around one selected causal object rather than a
generic settings panel:

- selecting a programmable Component opens its hardware envelope, live local
  readings, installed policy, runtime decision, and draft policy in one rail;
- selecting a Route opens physical endpoints, ownership, enabled state, limit,
  weight, requested flow, accepted flow, headroom, and attached interventions;
- selecting Supply opens its delivery radius, phase, possible delivery band,
  actual recipients, and next schedule change;
- selecting a compartment opens physical membership and loss only, while
  selecting a View opens observation membership and instrument controls only;
- selecting a timeline event preserves the current simulation step and moves
  selection to the addressed object without mutating the run.

The policy editor is a dense ordered table. Each row contains enabled state,
condition, parameters, action, target rule, and a drag/keyboard reorder handle.
Copy, delete, and disable use compact icon controls with tooltips. Invalid rows
remain visible with one literal diagnostic; committing is atomic, so a rejected
draft cannot partially alter the installed generator.

Reset language must name the retained boundary even before M-022 adds durable
blueprints:

| Action | Generator policy/topology | Assembly positions/stocks | Evidence |
|---|---|---|---|
| Restart Commission | keep last committed | restore current contract start | retain prior run |
| Discard draft | keep last committed | keep current paused state | unchanged |
| Commit and restart | install draft atomically | restore declared start | begin new run branch |

The M-024 companion slice provides sensor envelopes, eligible-target marks,
deterministic target emphasis, actuator reach, current-action stance, and
preview-action stance. Preview uses a distinct line treatment and never
masquerades as a live outcome.

Handoff to M-020: a player can create a valid local policy, install it, run it,
pause on one exact event, understand the causal chain, revise the draft, and
restart from a named boundary entirely inside the mechanistic workbench.

Completion condition: a player can build, commission, pause on a visible
bottleneck, identify the commanding rule and addressed physical object, revise it, and
resume without consulting campaign language or hidden state.

### M-020 — First contract content and commissioning

**Outcome:** Intake, Transfer, and Buffer form one complete commissioning
sequence from local sensing through sustained autonomous service and produce
canonical inputs for the qualification milestone.

The content compiler loads `content/contracts/manifest.json` and one canonical
JSON file per contract. It validates stable identifiers, prerequisites,
current capabilities, post-pass unlocks, opening assembly, hardware limits,
policy grammar, commissioning schedule, qualification suite, criterion vector,
and four grade-band tables. Compiled content contributes to the existing
content hash.

M-020A is implemented as a source pipeline, not as one large shell feature:

1. **Read ordered bytes.** The content build emits the contract manifest and
   exact contract file bytes in manifest order. Rust rejects extra, missing,
   duplicate, or reordered records and includes all bytes in the content hash.
2. **Parse exact records.** `core/src/content.rs` owns closed-key parsing,
   version checks, bounds, canonical integer units, and copy-key references.
   Unknown required semantics fail compilation instead of being ignored.
3. **Resolve vocabularies and availability.** Action and condition ids resolve
   to the same Rust enums used by policy parsing and capability admission.
   Hardware ids resolve to explicit sensor, actuator, storage, interface,
   movement, material, and Route-control capabilities. The current contract's
   `capabilities` are compiled separately from its post-pass `unlocks`; editor
   menus and admission use only the former, while result/progression uses only
   the latter.
4. **Resolve the opening.** A temporary legacy-opening adapter resolves the
   authored chapter/Form/regime reference, then emits a contract-local opening
   assembly and generator baseline. The resulting run has contract identity and
   no normal-path campaign objective.
5. **Resolve criteria.** Component and Route addresses, units, comparison
   operators, windows, grace, trial aggregation, and first-violation ownership
   are compiled once. The shell receives display-ready summaries, not executable
   criterion logic.
6. **Resolve progression.** Manifest order, prerequisites, next-contract links,
   and unlock receipts form one acyclic ladder. Current capabilities may be a
   cumulative authored envelope, but are not inferred by replaying receipts in
   React. Before qualification exists, only Intake is available and later
   contracts report the missing retained result.
7. **Expose protocol summaries.** One idle-safe catalog response returns each
   contract's availability, literal function, opening identity, current
   capabilities, limits, criteria, expected duration, and post-pass unlock
   receipt. A separate open command creates the first normal RunState, assigns
   the attempt, generator, assembly, and regime identities, and returns Design
   authority.
8. **Build the ladder workspace.** The shell renders the canonical summaries,
   keeps the selected contract and its details in one workspace, and opens the
   workbench without constructing a second content model.
9. **Complete copy and stance.** Every status, limit, criterion, lock reason,
   entry command, and opening annotation resolves through the catalog. Visual
   state distinguishes available, selected, locked, retained-pass, and active
   attempt without fictional map or campaign treatment.

Ambiguous schema fields are resolved before Intake becomes a normal route.
Component and Route limits mean total active machine counts, so the authored
opening must fit them; any player-addition budget is a separate field. Grade
tables use the product's four axes, `throughput`, `resilience`, `economy`, and
`complexity`; temporary reliability/efficiency/reserve names are not carried
into the public schema. Criterion thresholds use explicit fixed-point units and
comparators rather than relying on a field name such as `minimum_q` to imply
both.

Contract slices:

1. **Intake.** The opening Field contains one mobile Component, one visible
   Supply Stream, and one receiver whose operating margin is the service target.
   The contract exposes local Supply sensing, approach, and Coupling. Its
   criterion reports receiver Charge floor, maintenance window, and any Supply
   lost outside usable range. The expected realization is that detection,
   movement, transfer, and stored Charge are separate events.
2. **Transfer.** The opening adds one owned interface and one directed Route.
   The contract exposes interface and Route actions. Its criterion reports
   accepted Route flow, receiver margin, endpoint state, and uninterrupted
   service duration. The expected realization is that physical connection,
   enabled control, requested flow, accepted flow, and endpoint headroom are
   distinct states.
3. **Buffer.** The opening adds periodic Supply and finite storage. The contract
   exposes Charge thresholds, Supply emitting/quiet state, local timer, and
   storage behavior. Its criterion spans complete input cycles and reports
   service through every declared quiet interval, minimum reserve, leakage, and
   final hands-off duration. The expected realization is that a successful
   average transfer can still fail at a predictable gap.
4. **Contract shell.** The ladder shows exact available hardware and policy
   primitives before entry, preserves Commission attempts, previews the
   qualification criteria and unlock receipt, and permits retry or redesign
   without deleting prior evidence. Actual pass records and unlock mutation are
   owned by M-021.

The first three contracts use the same screen and policy grammar while changing
one principal engineering problem at a time:

| Contract | Opening machine | New expressive power | Commissioning problem | Qualification evidence | Primary visible failure |
|---|---|---|---|---|---|
| Intake | one mobile Component, one Supply Stream, one maintained receiver | local Supply state, move-toward-Supply, Couple | convert intermittent local detection into reliable acquisition and delivery | receiver Charge floor, sustained operating margin, usable Supply capture, hands-off interval | the mobile Component searches, arrives late, Couples out of range, or leaves the receiver below margin |
| Transfer | Intake hardware plus one owned interface and one directed Route | interface action, Route enable, local target selection | coordinate endpoint state and transfer without confusing connection with accepted service | minimum accepted flow, endpoint availability, receiver margin, uninterrupted service window | a Route is physically present but closed, disabled, throttled, source-starved, or destination-blocked |
| Buffer | Transfer machine plus finite storage and periodic Supply | Charge thresholds, emitting/quiet state, timer, reserve behavior | build stock during input windows and spend it across complete gaps | minimum reserve, service in every quiet interval, leakage ceiling, final hands-off interval | average flow passes while one quiet interval drains storage below the service floor |

Each contract has five authored presentation records in addition to its causal
specification:

1. **Brief:** one literal function statement, criterion vector, declared
   environment, available hardware, and unlocked policy primitives.
2. **Opening annotations:** names and units attached to the few objects already
   present in the Field; no modal tour and no fictional nouns.
3. **Context cues:** optional, dismissible cues triggered by observed states such
   as no rule installed, repeated `no_target`, closed endpoint, or exhausted
   reserve. Cues point to the exact object and reading.
4. **Result explanation:** every criterion receives pass/fail, measured value,
   required relation, retained window, and addressed object where applicable.
5. **Unlock receipt:** exact hardware, condition, action, instrument, or contract
   gained from a qualification pass. There is no invisible stat increase.

Pacing is authored around action and diagnosis rather than waiting. A new
policy should produce a visible consequence within several authoritative steps;
the first useful criterion window should complete within roughly one minute of
Commission at `1x`; accelerated reset-and-retry should take seconds; and no
required passive wall-time wait may exceed 20 seconds. The 8-12 minute target
includes reading, authoring, at least one informative failure, revision, and
qualification.

The M-024 companion slice gives each contract a distinct physical composition
without changing the shared grammar: Intake emphasizes sensing and directed
acquisition, Transfer emphasizes gates and request-versus-acceptance cadence,
and Buffer emphasizes reservoir level and emitting/quiet phase. Contract color
accents may aid recognition, but state remains legible through shape, motion,
texture, and exact text.

Each contract authors literal quantities and time windows in content; no React
threshold or tutorial-only completion flag may advance progression. Contextual
guidance may point at a visible object or reading, but it never blocks editing
or substitutes for the criterion.

#### M-020E Commission loop closure

M-020E turns the three contract slices into one repeatable pre-qualification
workflow. The Commission loop is complete only when leaving or replacing a
branch has an explicit evidence result.

Before the durable index, Rust adds the minimum canonical records needed by
qualification: `AssemblyTemplate`, `RunEnvelope`, `AttemptRecord`, and
`AttemptBranchRecord`. The browser persists their canonical bytes and maintains
rebuildable indexes; it does not invent lineage or authoritative hashes.

The durable browser index adds an immutable commissioning evidence record with
at least these fields:

- schema version and stable evidence id derived from canonical attempt and
  branch identities;
- contract, attempt, branch, explicit parent branch, branch operation, RNG
  stream nonce, run kind, generator, assembly, scenario, regime, protocol, and
  content identities;
- opening step, closing step, closing embodied-state hash, and diagnostic
  wall-clock record time;
- closure reason: `restart`, `returned`, or `superseded`;
- exact bounded addressed mechanism events retained by the live session,
  including first changed physical consequence and first weak margin;
- generator diff and named restart boundary when the branch descends from an
  earlier candidate;
- latest complete provisional criterion-margin vector and the current
  contract's qualification-input summary identity.

Wall-clock time is diagnostic metadata only. It does not participate in replay,
hashes, criteria, grades, progression, or deterministic comparison.

Branch-closing behavior is explicit:

| Player command | Record before transition | Generator after transition | Assembly after transition | Branch effect | Progress effect |
|---|---|---|---|---|---|
| Restart Commission | current branch as `restart` | retain committed generator | restore contract assembly boundary | explicit child branch; stream nonce advances | none |
| Return to ladder | current branch as `returned` | retain only through the addressed attempt record | no live assembly required | current branch closes | none |
| Open another contract/attempt | current branch as `superseded` | create from selected contract opening | create from selected contract opening | new attempt id, opening branch | none |
| Commit Design revision | retain prior generator identity in current evidence | replace atomically with admitted generator | keep current template until named restart | explicit child branch and new generator identity | none |
| Compare attempt | no record mutation | no mutation | no mutation | no change | none |

The workbench presents retained evidence as engineering comparison, not as a
chronological social feed. Each row exposes contract, attempt, branch, closure,
step span, generator and assembly references, criterion status, event count,
and the first differing addressed event when compared with the current branch.
Selecting a row can project its affected objects and relations using the same
scene geometry, but it cannot restore a lossy shell snapshot as causal state.

Commission adds optional causal breakpoints without changing simulation
semantics. A breakpoint addresses an object, rule, event kind, named outcome,
or criterion-margin crossing. The worker pauses immediately after the matching
authoritative event is retained, selects the affected object and commanding
rule, and opens the preceding event window. Breakpoints affect presentation
rate only, are absent from generator/assembly/criterion identity, and are
unavailable during Qualify.

The authority rail shows the complete provisional criterion-margin vector:
measured value, required relation, bound, retained-window progress, trend, and
source object. The weakest margin receives emphasis, but every row is labeled
as Commission evidence and cannot be mistaken for a retained qualification
pass. Contextual guidance escalates from addressed event/object, to literal
local mismatch, to opening the relevant editor or inspector. Only one cue may
be active; triggers, repetition thresholds, cooldowns, and dismissal are
content-owned, and a cue never supplies a complete policy or topology.

The disabled Qualify boundary is intentionally prominent. It shows the exact
candidate and trial inputs already available, then states that M-021 must freeze
them into an immutable request before execution. It does not imply that the
candidate has passed, is ready, or owns an unlock.

The M-024 companion stance makes branch state physical without adding fiction:
the active branch has one stable authority marker; retained branches use quiet
technical traces; restarting reconstructs embodied fill and interface state
while the retained generator structure remains visually fixed; comparison
highlights changed objects and accepted consequences only. Navigation does not
use a story map, transition scene, character reaction, or campaign metaphor.

M-020E closes with two deliberately separate command boundaries.

#### Restart Commission boundary

`preview_commission_restart` is read-only and authoritative. It is valid only
for a paused automation-contract candidate whose active branch has not already
closed. It returns one `CommissionRestartPreview`:

```text
CommissionRestartPreview
  |- contract / attempt / current branch / current parent
  |- current branch operation / current branch nonce / current step
  |- retained generator canonical bytes and hash
  |- restored contract AssemblyTemplate canonical bytes and hash
  |- scenario / regime / run-kind / content / protocol identities
  |- predicted operation = restart
  |- predicted parent = current branch
  |- predicted next nonce
  |- boundary label = contract opening assembly + committed generator
  `- consequence vector = keep generator / restore assembly /
                          retain evidence / create child branch
```

The preview does not assign the future branch id, advance the RNG stream,
export or close evidence, reconstruct state, or alter selection. The branch id
is assigned only by accepted authoritative execution so a dismissed preview
cannot reserve identity or create a gap in lineage.

The player confirmation shows current and restored generator/assembly
identities, the current parent branch, predicted next stream nonce, and the
four consequence rows. Confirming runs one ordered transaction:

1. export and bind the exact closing embodied hash to a prepared immutable
   attempt-evidence record;
2. submit `restart_commission` against the previewed current branch, generator,
   assembly, and nonce base;
3. reject without closure if any preview identity is stale;
4. reconstruct embodied state from the contract AssemblyTemplate while keeping
   the current committed GeneratorSpec;
5. advance the stream nonce, create one explicit Restart child branch, reset
   branch-local trace and criterion windows, and return the resulting identity;
6. add the prepared parent evidence and lineage records immutably only after
   authoritative acceptance;
7. clear stale event/criterion selections and return to Design on the child.

IndexedDB failure after authoritative acceptance is a degraded-durability
state, not grounds to roll back causal state. The record remains in the
in-session volatile index, the player is warned that reload durability is not
confirmed, and another write may only add the same canonically equivalent id.
An id collision with different canonical content is an integrity fault.

Restart failure handling is literal:

| Failure | Branch effect | Evidence effect | Player recovery |
|---|---|---|---|
| unsupported run kind or non-paused authority | none | none | return to the named required authority |
| branch already returned/superseded | none | none | resume or open the current active attempt |
| stale generator, assembly, branch, or nonce base | none | none | refresh preview and review changed identities |
| authoritative reconstruction rejection | none | none | inspect the exact field/content fault |
| durable archive unavailable after accepted restart | child exists | volatile immutable parent record retained | continue with visible degraded durability or leave data local to the session |
| conflicting immutable evidence id | child exists | conflict retained as integrity fault; no overwrite | stop branch-changing commands until the conflict is addressed |

The restart visual transition represents reconstruction, not rewind. Runtime
flow, policy decision marks, and transient events withdraw; fill, material,
positions, gates, and interfaces resolve to the template; generator structure
stays fixed; and the authority rail changes to the new child branch. Reduced
motion performs the same semantic transition without interpolation.

#### Qualification-input boundary

`preview_qualification_input` is read-only and authoritative. It is valid only
for a paused automation candidate. Rust combines the current run envelope and
compiled ContractSpec into canonical `QualificationInputPreview` bytes and a
preview hash. It does not create the immutable request owned by Q-01.

The preview has six sections whose order is stable across contracts:

1. **Candidate identity:** run kind, contract, content, attempt, source branch,
   parent branch, branch operation/nonce, generator, assembly, scenario,
   regime/lawset, build, and protocol.
2. **Frozen candidate data:** complete canonical GeneratorSpec and
   AssemblyTemplate bytes plus exact-or-migrated assembly stance.
3. **Procedure:** qualification-suite version, control contract, schedule
   identity/canonical bytes, duration, trial addresses/count, RNG algorithm,
   seed-custody rule, early-resolution predicates, progress cadence, and
   retention policy.
4. **Function criteria:** every compiled address, unit, comparator, bound,
   window, grace, aggregation, trial aggregation, and vector hash.
5. **Independent engineering axes:** exact Throughput, Resilience, Economy, and
   Complexity evidence definitions and authored monotone grade bands, with no
   aggregate score.
6. **Prospective receipt:** literal post-pass capabilities and prerequisites,
   labeled as unearned and incapable of mutating availability.

Preview status is `complete`, `incomplete`, or `stale`. `Incomplete` includes a
closed list of missing or incompatible addressed inputs. `Stale` is a shell
presentation state applied when any displayed candidate identity changes; the
old preview is discarded rather than submitted. `Complete` means only that
Q-01 can freeze the candidate without consulting unaddressed mutable state. It
does not mean pass, likely pass, qualified, sealed, or unlocked.

The command is referentially stable for identical authoritative inputs and has
no step, RNG, branch, policy, topology, assembly, archive, result, receipt, or
progression side effect. The shell may format its data but may not fill missing
fields, derive hidden schedule values, substitute catalog summaries for
canonical bytes, or calculate a preview hash.

The disabled Qualify surface shows `incomplete`, `inputs complete`, or `stale`
and the exact next boundary: Q-01 will freeze a new immutable request. It keeps
the Field visibly editable, uses no result color or seal, and provides no
execution control in M-020E.

Handoff to M-021: Intake, Transfer, and Buffer each have a canonical opening
assembly, legal policy envelope, commissioning schedule, qualification trial
family, exact criterion vector, grade bands, progress identity, and complete
player-facing copy. The shell can enter, Commission, retry, and submit a fully
addressed qualification request without consulting chapter or objective state;
it does not yet claim a pass or unlock the next contract.

### M-021 — Qualification and engineering grades

**Outcome:** qualification is a reproducible evaluation against a declared
suite, not a faster version of commissioning with hidden rescue controls, and
its retained pass is the sole authority from which contract availability is
derived.

#### Q-01 source packet: immutable freeze

Save V6 adds nullable `qualification_request` authority to `RunState`. A V5
record migrates with that field absent; migration never infers a request from a
paused branch or a preview. The request contains the complete canonical E-06
input plus `request_id = SHA-256({ input, version })`. On read, Rust rechecks
the exact input key set, empty missing-input vector, exact assembly bytes/hash,
generator bytes/hash, attempt and branch records, parent and nonce, run kind,
protocol identity, criterion-vector hash, and schedule hash before accepting
the request into state.

Protocol 12 adds `freeze_qualification_request`. The shell sends only stale
guards from the displayed preview: preview, assembly, branch, nonce, and
generator hashes. Rust rebuilds the input from current content and RunState,
requires complete status and exact guard equality, constructs the request, and
then installs it without advancing a step, consuming RNG, changing embodied
state, or starting a trial. Repeating the command with the same request is
idempotent; a different request is refused. The lifecycle becomes
`qualification_frozen`, in which causal frames and every Design, Commission,
restart, navigation, construction, policy, Route, and analysis command are
closed. Passive zero-step snapshot carriage, exact inspection, export, input
reinspection, and identical freeze retry remain legal.

Archive schema 4 adds `qualification_requests`, keyed by request id and indexed
by contract, attempt, branch, and storage time. Writes are add-if-absent and
compare canonical request content on an existing id. IndexedDB failure retains
the same record in volatile immutable storage, marks persistence degraded, and
offers retry; a conflicting id never overwrites either record. The source
Commission branch is archived as `qualified` only after the authoritative
freeze succeeds.

The sealed Workbench shows the request id, source branch, exact candidate and
procedure, durable/degraded persistence, and `execution not started`. It
withdraws editing and commissioning chrome while leaving the physical machine
unchanged and inspectable. Q-01 creates no job, trial, criterion decision,
grade, trace, result, receipt, contract availability, or progression record.
Those remain Q-02 through Q-07.

#### Q-02 source packet: addressed cold execution

Protocol 12 also carries the single `qualification_job` boundary. The browser
side uses `start`, `dispatch`, and `cancel` orchestration operations, but the
worker translates execution into the narrower Rust operations `prepare` and
`trial`. This keeps scheduling and cancellation outside simulation authority:
the worker may choose only the next missing trial address, while Rust validates
the sealed request and reconstructs and advances the fresh trial.

`prepare` derives one deterministic version-3 job id from the request id and
runner version, and returns the declared trial count, duration, and coarse
progress interval. The shell must append this queued job before asking the
worker to dispatch it. `trial` accepts only that job id, request id, and one
declared trial address. Rust derives the trial run identity from request plus
address, clones the frozen authored scenario, restores the exact assembly and
generator-bound form, starts a fresh `automation_contract` run, and advances
the full hands-off schedule through `analysis_step`. No live Field command,
steering input, rescue control, TypeScript physics, or shell criterion decision
enters the trial.

Each completed version-3 trial returns one immutable terminal artifact containing its
job/request/address identities, requested and executed duration, terminal
embodied-state hash, terminal Save V6 payload and payload hash, and the retained
Rust criterion runtime. The version-3 artifact also retains the exact trial
ledger totals for Supply, moved Charge, upkeep, drain, leakage, overload,
renewal, intervention count, and opening/closing material units by material
kind. It retains the exact first latched-failure Save payload when one exists,
plus bounded addressed mechanism-event windows for first failure and terminal
state. Q-02 stores those values as evidence only. It does not
translate any relation into pass, failure, grade, receipt, or progression; Q-03
owns that decision from the complete addressed artifact family.

The worker emits only `qualification_progress`: current address, completed
addresses, terminal artifact when one closes, and the exact job status. A
cancel request changes the job to `cancel_requested`; a synchronous Rust trial
already in progress may finish, and cancellation becomes terminal only at the
next address boundary. A core refusal becomes `invalid_execution`, never a
failed-function claim. A worker replacement or page reload converts a formerly
queued/running lease to `interrupted`; retained immutable artifacts remain and
resume supplies their addresses so only missing deterministic trials run.

Archive schema 5 adds `qualification_jobs` and `qualification_trials` beside
the schema-4 request store. Job authority fields are immutable while status and
the sorted completed-address projection may advance. Trial artifacts use
add-if-absent byte-equivalence. Persistence is serialized terminal artifacts
first and job projection second; a completed job cannot be written unless all
its delivered artifacts are already retained. Request durability and execution
durability have separate degraded states and retry operations, preventing a
successful request rewrite from hiding a failed trial write.

The sealed Workbench displays job identity, exact status, retained artifact
count, and one addressed trial matrix. It offers prepare/start, deterministic
resume, queued dispatch retry, or cancellation only where the job lifecycle
allows. It deliberately presents no evaluator locks, pass/fail banner, grade,
failure trace, result record, or unlock. Those remain Q-03 through Q-07.

#### Q-03 source packet: independent criterion decision

Q-03 extends the retained Rust criterion window rather than evaluating terminal
payloads in TypeScript. Every new `WindowSample` now carries the addressed
Charge value or absence for each required Component in addition to exact Route
flow, Supply, leakage, and step. This closes the authored `minimum stored
charge over window` evidence gap. Runtime samples written before this packet
remain readable for resumed ordinary play, but their missing Component history
is represented as unavailable and makes qualification resolution invalid; the
resolver never substitutes the terminal Charge or a threshold value.

The worker accepts a `resolve` operation only as orchestration and forwards the
complete terminal artifact family to Rust under the existing
`qualification_job` command. Rust rebuilds the deterministic job and trial
identities, requires exactly one ascending artifact for every declared address,
rehashes every terminal payload and artifact definition, reads and checks the
full Save V6 state, and requires exact scenario, generator, assembly, run-kind,
duration, terminal embodied state, and criterion-runtime equality. Missing,
duplicate, reordered, corrupt, mismatched, undefined, or pre-Q-03 evidence is a
resolution refusal and changes the job projection to `invalid_execution`; it
cannot become failed function.

For each trial and authored criterion, Rust resolves the declared source,
metric, aggregation, comparison, threshold, retained window, measured value,
margin, and boolean relation. Stored Charge uses the exact retained minimum;
accepted flow uses the addressed rolling minimum; leakage uses the exact
retained ratio and rejects an undefined ratio; hands-off service uses the
authoritative terminal streak. Each deterministic criterion-decision id hashes
its complete definition, including artifact, request, job, trial, source, and
window addresses.

Only after every per-trial relation is known does Rust create the deterministic
function decision referencing their ordered ids. `passed` means every authored
relation passed in every required trial; otherwise the independent function
decision is `failed`. Neither status contains or consults an engineering grade,
result-complete marker, receipt, contract availability, or progression state.

Archive schema 6 adds immutable `qualification_criterion_decisions` and
`qualification_function_decisions`. The shell appends every criterion decision
before the aggregate function decision. Reload exposes an aggregate only when
all referenced decision ids are present; an interrupted partial group remains
resolvable idempotently from the retained terminal artifacts. The Workbench
shows the literal function boundary, aggregate identity, and every trial
relation with measured value, threshold, window, and status. Q-04 through Q-07
still own independent grades, failure trace, complete result grouping, and
progression projection.

#### Q-04 source packet: independent engineering grades

Q-04 consumes only the complete version-3 artifact family and the retained
Q-03 function-decision id. Rust reruns Q-03 artifact validation and decision
derivation before grading; the supplied function id is a stale guard and must
equal the recomputed identity. This prevents a shell, archive index, or visible
pass banner from selecting the evidence or changing functional status.

The four axes use separate versioned monotone rules and preserve their typed
inputs:

1. **Throughput** retains every stored-Charge and accepted-flow service
   decision by trial, converts each measured/required pair to a bounded
   dimensionless fulfillment fraction, and uses the minimum fraction. Charge
   and flow quantities are never added to each other.
2. **Resilience** retains the complete trial-pass vector, its pass fraction,
   and the worst service-fulfillment fraction. Its score is the minimum of the
   two dimensionless fractions, a conjunctive floor rather than a weighted
   average.
3. **Economy** references every immutable trial artifact and retains exact
   same-unit Charge ledgers plus material-kind inventories and intervention
   count. Charge efficiency, per-kind material retention, and hands-off
   intervention stance are dimensionless independently; the axis takes their
   minimum across trials and never prices unlike resources into one sum.
4. **Complexity** retains Component, Route, policy-Component, rule, and
   canonical-policy-byte counts. Its band score is the minimum declared-limit
   headroom for Components, Routes, and rules. Policy bytes remain visible raw
   evidence and do not enter an unauthored normalization.

Each axis hashes a band-definition record containing its axis, exact authored
four-band vector, formula identity, and version. Its immutable grade record
then retains that hash, full evidence vector, score, reached band `0..4`,
request/job/function identities, and its own deterministic id. No axis reads
another grade; there is no combined score, compensation, weighting, or grade
effect on function pass/fail.

Archive schema 7 adds `qualification_grades`. Grade children are appended only
after the complete function decision and use add-if-absent byte equivalence.
Reload exposes grades only when exactly four distinct axes all reference the
standing complete function decision; a partial family remains hidden and can
be regenerated idempotently from artifacts. The Workbench presents four
stable instruments, exact bands and evidence descriptions, and the literal
`No aggregate score` boundary. Q-05 through Q-07 still own failure trace,
complete result grouping, and progression projection.

#### Q-05 source packet: first-violation trace

Version-3 cold execution closes the trace gap that a terminal payload cannot:
when the authoritative criterion runtime first latches `failed`, the runner
retains that exact Save V6 payload, its hash and step, and the bounded mechanism
events standing before it. It also retains the terminal event window for a
relation that fails only at the full-schedule boundary. The artifact identity
hashes all of these fields; Q-03 revalidates their ordering, payload identity,
scenario/generator/assembly custody, runtime status, resolved step, and event
addressing before any later packet consumes them.

Q-05 accepts only a retained failed function-decision id and the complete
artifact family. Rust reruns Q-03, chooses the failed relation with the earliest
resolution step, then trial address, then authored criterion order, and selects
the matching first-failure or terminal payload. Passing function returns
`not_applicable` and creates no failure record.

The deterministic `QualificationFailureTrace` retains the exact artifact,
criterion decision, function decision, request, job, trial, directly addressed
source, violation step, required-window start, selected payload hash, trace
keyframe hash/start, canonical trajectory steps, and mechanism events. Policy
rule/action/outcome transitions are included through the Rust mechanism event
stream; continuous Charge, flow, upkeep, exogenous, and failure records remain
in trajectory steps. The trace status is `complete` only when the retained
keyframe/step span covers the criterion window, the violation step is present,
and the mechanism-event buffer was not truncated. Otherwise the same failed
function decision receives `incomplete`; it is not reclassified as invalid
execution.

Contributor inference is an explicit empty vector under
`direct_records_only_v1`. Q-05 identifies observed sequence and adjacency but
does not claim root cause, rank contributors, or fabricate a controlled
counterfactual. A later paired-analysis feature must create a separately
versioned inference record before any contributor language appears.

Archive schema 8 adds immutable `qualification_failure_traces`, at most one
deterministic trace per job/function decision. The Workbench names the first
criterion, trial, step, exact measured relation, retained trajectory/event
counts, complete/incomplete stance, and `None; direct records only` inference
boundary. Q-06 consumes that trace into complete result grouping and Q-07 alone
may derive progression from a complete passing group.

#### Q-06 source packet: immutable result group

Q-06 adds a final Rust `result` operation to the sealed qualification job
protocol. The shell supplies the complete terminal artifact family, standing
function-decision id, the four ordered grade ids, and the applicable
first-violation trace id or explicit absence. These values are stale guards,
not result authority: Rust reruns Q-03 resolution, Q-04 grading, and Q-05 trace
selection over the artifacts and refuses any missing, reordered, mismatched, or
inapplicable child.

The addressed `QualificationResult` binds ordered artifact, criterion-decision,
and grade ids; the function decision and optional failure trace; request and job
ids; completed execution stance and passed/failed functional outcome; trial
count; contract, scenario, generator, assembly, and content hashes; and exact
core package, build version, and protocol version. A failed-function result
requires the deterministic Q-05 trace, including an incomplete trace stance
when the exact retained window is unavailable. A passing result requires trace
absence. Canceled, interrupted, and invalid-execution jobs cannot enter result
assembly and remain distinct job terminal states.

Rust separately addresses a `QualificationCompleteMarker` over the result id,
exact referenced-authority child count, and marker version. The marker counts
the request, job, terminal artifacts, criterion decisions, function decision,
four independent grades, and optional failure trace; it does not count itself
or pretend to be a progression receipt.

Archive schema 9 adds immutable `qualification_results` and
`qualification_result_markers`. Persistence writes all earlier children first,
then the result add-if-absent, and publishes the marker last. Byte-identical
retry converges; any existing-id byte conflict is a corruption refusal. A
result row without its marker remains hidden and can be regenerated from the
same children. Reload exposes a group only when the marker points to the result
and every referenced request/job/artifact/decision/grade/trace identity is
present under the expected function outcome and child count.

The Workbench presents the immutable result as a technical evidence instrument:
functional outcome, result and marker ids, referenced record count, execution
status, build, and protocol. It contains no aggregate grade, reward treatment,
unlock, or contract-availability claim. Q-07 alone may consume marker-complete
passing result ids to derive receipts and progression projection.

#### Q-07 source packet: receipt-derived progression

Q-07 adds a final `receipt` operation inside the sealed qualification job
surface. It accepts the standing result and marker ids plus the same artifacts,
function decision, ordered grades, and trace guard used by Q-06. Rust reruns the
complete Q-06 assembly and requires exact result and marker identity before it
will read the outcome. Failed-function results, canceled/interrupted jobs, and
invalid execution cannot produce a receipt.

For a passing result, Rust reads the source contract from the standing content
bundle and emits one deterministic `QualificationUnlockReceipt`. Its canonical
definition contains the source result, content and contract ids, authored
prerequisites, exact hardware/condition/action unlock vectors, declared next
contract, and receipt schema version. The shell contributes none of these
capability or graph values. The receipt id hashes the complete definition, so a
later content version or different passing result creates a different receipt
instead of rewriting prior evidence.

Archive schema 10 adds immutable `qualification_unlock_receipts`. The receipt
is written only after its source result marker. The in-memory progression cache
is rebuilt at startup by scanning receipts and retaining only those whose
passing result, completeness marker, request, job, every trial artifact, every
criterion decision, function decision, all four grade records, and optional
trace remain present. Missing or corrupt source records remove that edge on the
next rebuild; no historical `completed: true` value survives independently.

`list_contracts` and `open_contract` now receive the eligible receipt family.
Rust revalidates each receipt against the current content hash, closed authored
unlock vectors, prerequisite vector, next-contract declaration, result address,
and receipt hash. It derives completed, available, locked, and missing-
prerequisite catalog states from the resulting contract-id set. Opening a
contract with prerequisites requires that same validated set, so visual ladder
availability and command admission cannot diverge.

The result workspace exposes a literal `derive receipt and update contract
availability` operation only for complete passing results. It shows receipt,
source result, completed contract, next contract, and exact newly represented
capabilities. A failed result explicitly states that no capability or contract
becomes available. Returning to the ladder is allowed after a failed complete
result or after a passing receipt has been retained. No animation, score,
campaign flag, or mutable progress boolean participates in this boundary.

Implementation slices:

1. **Freeze.** Qualify canonicalizes and hashes ContractSpec, GeneratorSpec,
   complete AssemblyTemplate bytes, regime/schedule, compiled criterion vector,
   control contract, and trial addresses. Entering Qualify closes every causal
   edit path and records the explicit source branch.
2. **Cold runner.** A dedicated worker job restores fresh authoritative Rust
   trials, advances at accelerated rate without streaming normal render frames,
   reports coarse progress, and can stop a trial early only when a retained
   monotone bound proves all remaining inputs incapable of changing its
   resolution.
3. **Criterion resolution.** Pass requires every declared criterion across the
   required trial family. The result retains per-trial status, exact resolution
   step, retained trace identity, and the immutable inputs needed to reproduce
   the result.
4. **Independent grades.** Throughput retains useful delivered service and
   demand; Resilience retains pass vector and worst service; Economy retains
   typed resource, upkeep, leakage, overload, material, and intervention
   evidence; Complexity retains Component, Route, rule, and canonical-policy
   byte counts. Contract-authored monotone predicates map each typed vector to
   its axis band without summing unlike quantities.
5. **Failure evidence.** The result names the first violated criterion, exact
   step, directly addressed object, preceding observed state changes, and
   retained trace. Any suggested contributor is labeled as inference; causal
   attribution requires a controlled paired trial. The UI provides a direct
   branch-back-to-Design action but never mutates the frozen result.
6. **Progression projection.** Persist request, trial artifacts, immutable
   result, and projection receipt in a recoverable ordered transaction. Derive
   availability from retained complete-pass result ids and a versioned
   projection rather than writing an independent completion boolean. Reveal the
   literal unlock receipt authored by the contract. Replaying, improving
   grades, deleting a display index, or recovering an interrupted write cannot
   revoke or fabricate a recorded pass.

The frozen request is an addressed engineering bundle, not a pointer to mutable
browser state. It contains at minimum:

- contract id, schema version, content hash, and exact compiled criterion
  vector/version/hash actually executed;
- generator bytes/hash and separate complete assembly-template bytes/hash;
- regime/lawset identity, canonical schedule bytes or suite-committed input
  artifact sufficient for reproduction, control contract,
  qualification-suite version, and trial addresses; Holdout inputs remain
  withheld from policy and normal UI without being omitted from retained
  evidence;
- RNG algorithm/version, root seed custody, build/protocol identity, and source
  blueprint branch;
- the authored early-pass, early-fail, duration, retention, and progress-report
  rules used by the cold runner.

The result record contains the immutable request identity plus one record per
trial: pass/fail, resolution reason, resolution step, criterion readings,
four-axis evidence inputs, trace identity, first violation, and directly
addressed objects. It also stores the aggregate criterion decision and four
independent authored grade labels. It does not store an aggregate score or a
mutable "best" result that can overwrite older evidence.

Qualification has four visible phases:

1. **Freeze:** the shell shows the exact generator, assembly, regime, compiled
   criteria, and trial count becoming immutable.
2. **Execute:** ordinary Field controls withdraw; the worker reports a
   trial-by-criterion matrix with queued, executing, resolved, and early-stopped
   states at a coarse cadence. Cancellation yields no result or progression.
3. **Resolve:** the criterion vector locks before grades are calculated, so a
   strong grade cannot turn a failed function into a pass.
4. **Inspect:** the result opens on criterion status, then independent grades,
   then the first violation and trace. Branch to Design creates a descendant and
   leaves the frozen result untouched.

The M-024 companion slice makes the authority change visible without pretending
the machine itself changed: editing handles and preview marks withdraw,
instrument chrome becomes sealed, progress is restrained, and pass/fail
resolves from the evaluator rail rather than a celebratory overlay. Criterion
violation does not produce physical fracture unless a separate embodied event
exists. Audio uses one brief freeze motif and distinct pass, violation, and
invalid-execution events derived from result state.

Handoff to M-022: every qualifying candidate and result can be addressed by
stable identities, reopened for exact inspection, and linked to a new editable
branch without mutating the source candidate or evidence.

Completion condition: qualification can be reproduced from its record, cannot
receive shell control, and returns pass/fail plus four separate grades with no
aggregate score.

### M-022 — Blueprints, reset, comparison, and transplant

**Outcome:** successful organization becomes a reusable engineering object
without conflating frozen design and embodied initial conditions.

Implementation slices:

1. **M-022A record authority and migration.** Establish the complete immutable
   V2 record family, content-addressed child stores, typed derivation and
   evidence relationships, separate mutable display metadata, exact V1 custody,
   resumable migration journal, and volatile-storage parity before a later
   packet writes V2 references.
2. **M-022B Design assembly transaction and exact capture.** Expose the complete
   assembly-owned opening declaration as a Design-only draft. Rust previews
   canonicalization, compatibility, reconstruction, identities, and exact diff;
   guarded commit creates a child branch without changing generator bytes. Save
   Blueprint consumes only that committed authority or an immutable result
   source and never silently captures live Commission state.
3. **M-022C named transitions.** Restart Assembly restores the current committed
   generator/assembly pair. Revert Generator restores selected immutable design
   while preserving only assembly fields admitted by compatibility. Full
   Contract Reset restores both authored opening records. Each uses separate
   preview, commit, receipt, persistence, and recovery behavior.
4. **M-022D clone lineage.** Clone creates a new editable attempt with true
   branch parentage plus typed derivation from a blueprint, result, request, or
   retained branch. It copies exact generator/assembly authority, inherits no
   qualification claim, and never overwrites ancestor evidence.
5. **M-022E engineering diff.** Side-by-side comparison first normalizes each
   source into generator-design, initial-assembly, and observed-evidence
   projections, then aligns hardware, topology, policy rules/bytes, Route
   controls, stock/material, identities, criteria, four grades, and failure
   evidence. Equal, changed, added, removed, unaligned, and unavailable remain
   explicit; a selected difference can seed one descendant hypothesis branch.
6. **M-022F transplant compile and adaptation.** A transplant is read-only
   compatibility analysis, Design-only adaptation of assembly-owned fields,
   and explicit destination instantiation. It keeps one frozen
   `GeneratorSpec`, retains incompatibilities even when no run is created, and
   exits the unchanged-generator claim before any policy retuning.
7. **M-022G comparative qualification.** After ordinary source and destination
   results are complete, Rust derives one `ComparativeQualificationRecord`
   naming controlled and uncontrolled differences, matched identities, and the
   first addressed divergence. It never recomputes, rewrites, or replaces either
   result and never participates in progression as a private paired qualifier.


The persistence model uses explicit record relationships:

```text
BlueprintRecord
  |- generator_identity -> canonical GeneratorSpec
  |- assembly_identity  -> canonical AssemblyTemplate
  |- parent_blueprint   -> optional immutable blueprint ancestor
  |- derivation_edges   -> typed branch/request/result/blueprint sources
  |- source_contract    -> authored ContractSpec identity
  |- qualification_ids  -> zero or more immutable result records
  |- lineage authority  -> creation reason and content/schema versions
  `- display metadata   -> mutable name, tags, timestamps, and thumbnail
```

True attempt-branch parentage and cross-record derivation are separate. A child
may descend from the current branch while also recording that the operation was
initiated from a blueprint, request, or result. One overloaded `parent_id`
cannot represent both facts without losing lineage meaning.

The engineering-memory protocol is split into explicit operations and records:

| Record or command | Authority | Required contents |
|---|---|---|
| `EngineeringTransitionPreview` | Rust, read-only | operation, expected current identities, retained/restored/recreated/detached identities, compatibility issues, preview id |
| `commit_engineering_transition` | Rust mutation | preview id, expected branch/generator/assembly guards, accepted operation; returns one transition receipt and new run identity |
| `EngineeringTransitionReceipt` | Rust record, browser durable | operation id, parent/child lineage, old/new identities, reconstruction result, recovery state |
| `DerivationEdge` | Rust record | typed source kind/id and operation, separate from actual parent branch and parent blueprint |
| `EngineeringDiffReport` | Rust derived record | typed left/right subjects, design/assembly/evidence projections, aligned changes, unavailable/unaligned sections, schema versions |
| `CompatibilityReport` | Rust derived record | source/destination generator, assembly, contract, and regime identities; typed issues; permitted assembly-only adaptations; byte-identical-generator result |
| `AssemblyAdaptationRecord` | Rust record | source assembly, destination assembly, exact declared changes, compatibility report, destination lineage |
| `ComparativeQualificationRecord` | Rust derived record | ordinary source/destination result ids, controlled and uncontrolled differences, matched identities, and first observed divergence |

`recover_engineering_transition` resolves a lost response or interrupted browser
write from the transition receipt carried by the authoritative save. React may
retry add-if-absent persistence and active-pointer movement; it may not rerun or
infer the reset, clone, diff, or compatibility decision.

The shell never offers a generic Reset command because each reset boundary has
a different engineering meaning:

| Command | Frozen generator | Assembly template | Live embodied state | Qualification evidence |
|---|---|---|---|---|
| Restart Assembly | keep current | keep current | reconstruct from template | preserve and detach from new run |
| Revert Generator | restore selected blueprint | preserve only declared compatible fields | reconstruct | preserve on original ancestor |
| Full Contract Reset | restore authored opening | restore authored opening | reconstruct | preserve as historical evidence |
| Clone Branch | copy into new editable identity | copy into new editable identity | optional fresh reconstruction | reference only; never inherit the claim |
| Transplant | keep selected generator exactly | instantiate a declared destination assembly | new regime state | no carryover qualification |

Compatibility is resolved before reconstruction. Missing hardware, unsupported
policy actions, invalid addresses, illegal Route ownership, absent material,
and regime-incompatible assembly declarations are listed separately. The player
must either adapt the assembly, choose a compatible destination, or branch the
generator; the system never deletes a rule or adjusts a threshold silently.

Comparison is an engineering diff, not a pair of summary cards. It aligns
Components by stable address, Routes by endpoints and owner, policy rules by
ordered position, and assembly objects by declared identity. Added, removed,
changed, and equal records receive distinct line treatments. Criteria, grade
evidence, first violation, and provenance remain below the design diff so the
player can connect a structural change to an observed result without implying
causation beyond the retained evidence.

The comparison ends in an experiment action. The player selects one changed
subsystem or evidence discrepancy, branches from either source, and carries the
comparison id plus selected addresses into Design as context. The branch remains
fully editable, but the source records and comparison are immutable and no
causal claim is created merely because two values differ.

The M-024 companion slice gives blueprint lineage a restrained technical
language: full machine thumbnails use the real scene projection, changed
topology and policy relations are overlaid geometrically, and regime transplant
shows the destination medium around the unchanged machine rather than using
fictional location art.

Handoff to M-023: advanced contracts can unlock against durable qualification
records, instantiate known generator/assembly pairs, create explicit branches,
and compare failures across regimes without identity drift.

Completion condition: a player can reset, clone, compare, and transplant a
design while every identity boundary and evidence relationship remains visible.

#### M-022 implementation specification

M-022 is executed as seven ordered packets. Record authority lands before any
new operation writes those records. Assembly commit lands before reset because
reset must reconstruct a trusted generator/assembly pair. Clone lands before
comparison because a comparison action must have a valid descendant target.
Transplant lands after ordinary diff and before comparative qualification so
the latter can remain derived evidence over two normal results.

```text
M-022A record authority and migration
  -> M-022B Design assembly transaction and exact blueprint capture
    -> M-022C named transition preview / commit / recovery
      -> M-022D clone into a new editable attempt
        -> M-022E normalized engineering diff and hypothesis branch
          -> M-022F compatibility compile and destination adaptation
            -> M-022G comparative qualification record
```

M-024 accompanies every arrow. A record or transition is not handed off when
the only way to understand it is a hash, a noun, or a prose status message.

##### Canonical field ownership

The target assembly editor is comprehensive but deliberately narrow. It presents
every field the assembly owns and no field owned by the generator, live runtime,
or evidence system.

| Ownership register | Fields | Identity effect | Legal writer |
|---|---|---|---|
| generator design | Component hardware/profile, topology, Route endpoints and ownership, complete frozen policies, Route control defaults, construction declarations | changes `GeneratorSpec` and creates a generator revision | admitted Design generator commit only |
| opening assembly | Component opening position/layer/stored Charge/interface stance, Form reserve and opening blanks, typed material stock and placement, Current opening phase/active stance, physical-compartment declaration, explicit starting attachment state when the schema supports it | changes `AssemblyTemplate` without changing generator bytes | admitted Design assembly commit or exact authored/result source |
| live embodied runtime | current position after motion, current stock after transfer/upkeep/loss, timers, active rule/target/outcome, accepted flow, damage in Commission, temporary interface state, signals, disturbances, velocities, caches, criterion windows | changes embodied save/checkpoint identity only | deterministic Rust step or admitted Commission action |
| immutable evidence | requests, trials, criteria, grades, first violations, results, receipts, traces, comparison and compatibility reports | creates append-only evidence identities | Rust qualification/derivation boundary |
| display metadata | name, tags, notes, timestamps, selected thumbnail, workspace sort/filter state | never changes generator, assembly, blueprint, branch, request, result, or compatibility identity | browser metadata owner |

The assembly schema is a canonical opening declaration, not an arbitrary
`FieldState` snapshot. Canonicalization clears step counters, transient policy
runtime, accepted/requested flow, active disturbances, draw-only geometry,
selection, View state, live damage, movement velocity, pending actions, and
other Commission residue before hashing. Every V2 record declares its schema
version and complete ordered ownership vocabulary so an older reader cannot
silently omit a newly added assembly field.

##### V2 record family

All immutable ids are content addresses over canonical Rust bytes. A browser
store key may repeat that id but may not replace it with a timestamp, display
name, array index, or generated UUID. Add-if-absent accepts exact duplicate
bytes and rejects an id/byte conflict.

| Record | Required immutable contents | Explicit exclusions and mutation rule |
|---|---|---|
| `EngineeringGeneratorRecordV2` | record/schema version, generator id/hash, complete canonical `GeneratorSpec`, source contract/content/protocol identities | no assembly, live state, grade, pass, or display metadata; immutable |
| `EngineeringAssemblyRecordV2` | record/schema version, assembly id/hash, complete canonical `AssemblyTemplate`, ordered owned-field vocabulary, compatible generator/contract/regime identities when known, source authority | no policy/runtime/evidence/display data; immutable |
| `BlueprintRecordV2` | blueprint id, child generator-record id, child assembly-record id, source contract, source attempt/branch, optional parent blueprint, creation reason, typed derivation edges, typed evidence links, schema/content/protocol identities | no embedded mutable name/tags/thumbnail and no inferred pass claim; immutable |
| `DerivationEdge` | source kind and id, operation kind, destination kind and id, parent branch where applicable | does not replace true attempt/branch parentage; immutable |
| `EvidenceLink` | evidence role, exact request/result/trace/comparison id, availability stance, authored schema identity | a missing target stays an unavailable link rather than becoming an inferred result; immutable except through a new parent record |
| `EngineeringTransitionReceipt` | operation id/kind, preview id, prior and child run identities, retained/restored/recreated/detached identities, reconstruction digest, lineage, recovery state | never reused for another operation and never rewritten to hide interruption; append-only recovery facts may refer to it |
| `EngineeringDiffReport` | subject types/ids, normalization versions, design/assembly/evidence sections, aligned rows, unavailable/unaligned sections, selected hypothesis context when saved | deterministic derived data; sources remain immutable |
| `CompatibilityReport` | source generator/assembly/contract/regime, destination contract/regime, exact issue list, permitted assembly adaptations, unchanged-generator verdict | read-only analysis; refusal is retained evidence, not an error to discard |
| `AssemblyAdaptationRecord` | accepted report id, source/destination assembly ids, exact changed fields, destination attempt/branch, unchanged generator id | no generator retuning; immutable |
| `ComparativeQualificationRecord` | source/destination result ids and provenance, controlled/uncontrolled differences, matched identities, first addressed divergence, comparability verdict | does not recompute, merge, or replace either ordinary result; immutable |
| `EngineeringDisplayMetadata` | target immutable record id, user name, tags, notes, thumbnail asset/version, created/updated display times | replaceable; excluded from all causal and evidence hashes |
| `EngineeringMigrationJournal` | archive schema from/to, phase, last completed key, copied/conflicted/unsupported counts, current refusal or recovery detail | browser operational record; resumable and removable only after the new stores are durably readable |

`BlueprintRecordV1` remains byte-preserved and readable. A V1 adapter may expose
an unavailable parent, derivation, or evidence-role section, but it may not
invent those facts or rewrite the stored V1 bytes into V2 in place. Promotion
to V2 is an explicit child capture with a derivation edge to the V1 record.

##### Protocol operations and guards

The existing `engineering_memory` command becomes an operation envelope rather
than a capture/reset switch. Every response carries protocol/content identity
and one typed operation result. Every Rust mutation includes the complete
stale-base guard set: run kind, lifecycle authority, contract id, attempt id,
branch id, generator id, assembly id, and preview id. The shell transition
envelope separately carries the browser-owned expected pointer generation.

| Operation | Legal authority | Rust behavior | Mutation and response |
|---|---|---|---|
| `assembly_draft` | Design | project the exact committed assembly into a complete editable draft and declare every owned field | read-only; returns base identities, draft schema, capabilities, and immutable-field summary |
| `preview_assembly` | Design | canonicalize the full draft, check exact address sets/types/ranges, compile compatibility, reconstruct a candidate opening on cloned state, and derive exact diff/preview id | read-only; returns accepted candidate or complete typed refusals; no branch or record writes |
| `commit_assembly` | Design | recheck preview and base guards, reconstruct the accepted opening, update scenario assembly authority, and create an assembly-commit child branch | mutates once; returns child run identity, canonical records, diff, and transition receipt for durable recovery |
| `capture_blueprint` | Design or returned immutable-result authority | resolve an exact committed generator/assembly source, validate typed evidence links, and emit V2 child records plus blueprint | no live Commission capture; returns immutable authority records before optional browser metadata |
| `preview_transition` | Design, paused Commission, frozen-result/returned as allowed by operation | calculate one Restart Assembly, Revert Generator, or Full Contract Reset consequence and compatibility report | read-only; returns exact retained/restored/recreated/detached identities and preview id |
| `commit_transition` | same source authority as accepted preview | recheck guards, perform exactly one reconstruction, increment lineage, and retain a receipt in the authoritative save | mutates once; no generic reset fallback |
| `recover_transition` | catalog/resume recovery or active session | resolve operation id from current save/branch receipts without replaying the transition | read-only apart from explicit acknowledgement; returns authoritative accepted/refused/unknown state |
| `clone_source` | catalog, result, blueprint, retained branch, or explicit return boundary | resolve immutable source, create a new attempt/root branch, copy exact generator/assembly authority, attach derivation, clear qualification slate | creates a separate attempt; never edits the source/current attempt implicitly |
| `derive_diff` | any nonmutating workspace authority | normalize two resolvable subjects and derive aligned sections | read-only; optional display/hypothesis metadata is browser-owned |
| `preview_transplant` | catalog/Design/engineering workspace | compile source against destination contract/regime and enumerate legal assembly-only changes | read-only; returns retained compatibility report even when incompatible |
| `commit_transplant` | Design with accepted current report | recheck report/base guards, adapt assembly-owned fields, prove unchanged generator bytes, instantiate destination child attempt | mutates once; returns compatibility/adaptation/lineage records |
| `derive_comparative` | result/engineering workspace | compare two ordinary complete results without running trials | read-only derivation; returns comparative record for add-if-absent persistence |

Typed refusals share one closed vocabulary with catalog copy. At minimum they
distinguish wrong authority, stale preview, stale branch, missing source,
unsupported schema, immutable-record conflict, generator mismatch, assembly
address mismatch, illegal owned field, unavailable evidence, incompatible
hardware, unsupported action, invalid Route ownership, missing material,
regime incompatibility, reconstruction refusal, incomplete result, and
uncontrolled comparison input.

##### Durable operation state machine

Browser durability follows the same state machine for assembly commit, named
transition, clone, and transplant. The exact child records differ; the ordering
does not.

1. **Previewed.** React holds the Rust preview and complete expected identities
   in memory. No canonical record or active pointer changes.
2. **Prepared.** React constructs the prior-branch closure and operation journal
   entry in memory. A prepared operation is not represented as accepted.
3. **Accepted in Rust.** The guarded commit reconstructs successfully and Rust
   returns an operation id, receipt, canonical child authority, and new save.
   A refusal leaves the previous run byte-for-byte authoritative.
4. **Prior branch retained.** React add-if-absent persists the prior save,
   closure, branch record, event/criterion custody, and any source authority
   needed to reopen the reasoning history.
5. **Child authority published.** React persists new generator/assembly records
   where applicable, transition/compatibility/adaptation records, child attempt
   and branch, checkpoint/save, then immutable blueprint/comparison records.
6. **Pointer moved.** `ActiveSessionPointer` is replaced only after every record
   it references is readable. Pointer generation increases exactly once.
7. **Metadata completed.** Names, tags, notes, thumbnails, and saved hypothesis
   context are written last and may be retried without changing operation
   authority.
8. **Recovered or acknowledged.** On reload, the journal and Rust-carried
   receipt determine whether to retry missing identical writes, publish an
   already-created inactive child, restore the prior pointer, or expose a
   literal manual recovery choice. React never reruns a mutating command to
   infer whether it previously succeeded.

The journal uses explicit states `prepared`, `accepted_unpersisted`,
`prior_retained`, `child_published`, `pointer_moved`, `complete`, `refused`, and
`recovery_required`. Cancellation is legal only before Rust acceptance. After
acceptance, the UI may leave the workspace, but persistence/recovery ownership
continues by operation id.

##### Packet implementation detail

**M-022A record authority and migration**

1. Freeze V2 JSON/canonical writers in Rust and matching discriminated TypeScript
   readers; use versioned unions rather than widening V1 interfaces in place.
2. Add independent stores/indexes for generator, assembly, blueprint authority,
   display metadata, derivation/evidence links where not nested, transitions,
   diffs, compatibility, adaptations, comparative results, and migration
   journal. Increment archive schema only after all upgrade handlers exist.
3. Upgrade by opening new stores first, enumerating old keys deterministically,
   copying/indexing exact records, recording unsupported rows, and leaving old
   stores readable until the journal reaches `complete`.
4. Implement the identical operation order in volatile storage, including byte
   conflict refusal and joined-record reads, so degraded storage never changes
   engineering semantics.
5. Expose record schema, source authority, unavailable relationships, migration
   state, and conflicts literally in the engineering workspace. V1 remains
   read-only; V2 creation always produces a child record.

Handoff: M-022B can persist a generator and assembly before it creates the first
V2 blueprint, and an interruption cannot orphan an invisible mutable claim.

**M-022B Design assembly transaction and exact blueprint capture**

1. Populate the full draft from the committed `AssemblyTemplate`; no partial
   patch defaults to current Commission state. Display the base branch,
   generator, assembly, contract, and regime identities beside the draft.
2. Author positions/layers, opening Charge/interface stance, Form reserve and
   blanks, material stock/placement, Current opening phase/active stance, and
   physical compartment through controls appropriate to each value. Generator
   fields remain inspectable but locked in this mode.
3. Preview in Rust. Require exact object/address coverage, stable ordering,
   finite/ranged values, hardware compatibility, physical establishability,
   and generator-byte equality. Return normalized values, warnings, refusals,
   before/after ids, and an address-level diff.
4. Render the candidate only in the noncausal draft register. The live Field,
   inspector, event history, and current branch remain unchanged until commit.
5. Commit by the durable state machine. Begin the child in Design at its exact
   reconstructed opening and retain the prior branch as immutable history.
6. Capture V2 Blueprint from that committed child or from an immutable result's
   exact source identities. Persist authority first, then accept user metadata
   and a thumbnail generated from the shared real scene projection.

Handoff: every reset, clone, and transplant can address an exact reusable
generator/assembly pair whose origin is explicit and whose live-state exclusions
are inspectable.

**M-022C named engineering transitions**

1. Replace `reset` string dispatch with a typed operation union and separate
   preview/commit/recover responses.
2. Restart Assembly keeps the current committed generator and assembly,
   reconstructs embodied state, detaches live evidence from the new branch, and
   reports the reset step/checkpoint boundary.
3. Revert Generator selects an immutable ancestor generator, compiles the
   current assembly against it, preserves only fields the compatibility report
   explicitly admits, and refuses when reconstruction would require guessing.
4. Full Contract Reset restores authored opening generator and assembly while
   retaining all prior branches and evidence as history.
5. Give each operation distinct confirmation copy, reconstruction motion, and
   result summary. A single Reset label or animation is prohibited.

Handoff: transition receipts and child branches are trustworthy enough for
direct clone and history navigation.

**M-022D clone lineage**

1. Resolve blueprint, result, or branch source through immutable records and
   refuse unresolved, corrupt, unsupported, or incompatible sources.
2. Create a new `AttemptRecord`; create its root branch from exact source
   generator/assembly bytes; record true parentage separately from source
   derivation; start with no qualification request, result, receipt, or seal.
3. Persist the child before moving the pointer. An interrupted pointer write
   leaves a labeled inactive attempt that can be opened or discarded without
   deleting shared ancestors.
4. Open directly in Design with source context visible. Name/tags may be copied
   as a suggestion, but metadata never implies inherited authority.

Handoff: comparison and transplant can always branch into a clean editable
attempt rather than mutating an evidence-bearing source.

**M-022E engineering diff and hypothesis branch**

1. Resolve each subject into three independent projections: generator design,
   declared initial assembly, and observed evidence.
2. Align Components by stable address; Routes by owner/endpoints; policies by
   owning Component and ordered rule position; material/stocks by type and
   address; criteria/grades/failures by typed source and observation window.
3. Emit `equal`, `added`, `removed`, `changed`, `unaligned`, and `unavailable`
   states. Never collapse a missing evidence section into equality.
4. Connect a selected row to both real-scene projections, exact inspector
   readings, relevant events/criteria, and lineage. Structural and policy
   differences use different geometry and never recolor the entire Field.
5. Let the player choose either source plus one hypothesis context and create an
   M-022D descendant. The hypothesis is navigation metadata, not a causal claim.

Handoff: one focused engineering question can be revised and commissioned with
the source comparison retained.

**M-022F transplant compile and destination adaptation**

1. Select frozen source blueprint/result and destination contract/regime without
   creating a run. Rust emits compatibility across hardware, policy actions,
   addresses, Route ownership, material, assembly placement, compartment, and
   regime law.
2. Separate hard incompatibility, legal assembly adaptation, and generator edit
   required. Only the middle category enters the assembly draft; the last exits
   the byte-identical transplant flow into an ordinary descendant.
3. Preview the destination machine using unchanged generator bytes and the
   adapted opening assembly. Show source/destination regime laws and every
   controlled/uncontrolled opening difference.
4. Commit through the durable state machine and retain the compatibility report
   even if adaptation or instantiation is refused.

Handoff: the destination attempt has explicit regime and assembly lineage and
can use ordinary Commission and qualification without a transplant-specific
simulation path.

**M-022G comparative qualification**

1. Require two complete ordinary results and resolve their requests, generator,
   assembly, regime, schedule, seed family, criteria, build/protocol, and trace
   identities.
2. Prove or refuse the narrow unchanged-generator claim. Partition remaining
   inputs into matched, controlled-different, uncontrolled-different, and
   unavailable sets.
3. Align criterion and engineering-grade evidence without averaging or replacing
   either result. Locate the first addressed divergence in accepted action or
   physical consequence when comparable traces exist.
4. Persist the comparative record after both source results and offer an
   M-022D descendant action from either side. Progression continues to consume
   only ordinary complete passing results.

Handoff: M-023E can teach regime dependence using ordinary engine, result, and
progression systems.

##### Engineering workspace and M-024 companion

Engineering Memory is one task workspace with stable modes, not a collection of
unrelated dialogs:

| Mode | Dominant surface | Right rail | Bottom evidence band | Required non-happy states |
|---|---|---|---|---|
| Browse | real-projection machine plus lineage context | immutable identities, source authority, metadata | linked results, criteria, grades, first failure | empty Archive, V1 read-only, missing evidence, migration in progress |
| Edit Assembly | live committed machine plus unmistakable draft overlay | complete assembly controls and locked generator summary | exact before/after assembly diff and compatibility | invalid value, missing address, stale base, incompatible draft, refused reconstruction |
| Preview Transition | retained boundary and destination reconstruction side by side | operation-specific kept/restored/recreated/detached list | branch closure and evidence consequences | stale preview, unavailable ancestor, incompatible assembly, recovery required |
| Compare | aligned real projections with selected changed relation | design/assembly/evidence filters and exact row detail | criteria, grades, first divergence, hypothesis context | unaligned schema, unavailable source, incomparable evidence |
| Transplant | unchanged machine framed by source/destination regime stances | compatibility issues and legal assembly adaptations | controlled/uncontrolled differences and later result links | hard incompatibility, generator edit required, refused destination, incomplete results |

The M-024 companion uses one shared semantic projection across inspector,
scene, WebGL, Canvas2D, timeline, audio, thumbnail, and result surfaces:

- blueprint thumbnails are captured from the real scene at a declared state and
  quality tier; they never substitute atmosphere for machine identity;
- assembly drafts use outline/texture/handles reserved for noncausal preview and
  cannot resemble live movement, transfer, damage, or accepted actions;
- named transitions animate only the objects/relations actually reconstructed
  and then settle into the accepted opening state;
- lineage is compact technical notation with explicit parent/derivation roles,
  not narrative geography;
- design, assembly, and evidence differences keep separate line/shape registers;
  color is reinforcement rather than the only distinction;
- transplant preserves the machine silhouette and generator relationships while
  the environment, assembly changes, and affected margins carry the difference;
- audio is event-derived and sparse: metadata/cursor actions stay quiet,
  preview and commit are distinct, refusal never resembles physical failure,
  and accepted reconstruction receives one restrained transition cue.

Every compact label expands to a literal explanation of subject, source
authority, operation, expected identity consequence, evidence consequence, and
available next action. Player-facing strings remain in `content/copy/catalog.json`.

##### Source ownership and packet handoff

| Layer | Primary owners | M-022 responsibility |
|---|---|---|
| Rust state/canonical data | `core/src/state.rs`, `core/src/run.rs` | V2 schemas, field ownership, canonical bytes/hashes, reconstruction, attempt/branch lineage |
| Rust command/derivation | `core/src/protocol.rs` and focused engineering modules when extraction reduces real complexity | operation envelope, previews, stale guards, receipts, diff, compatibility, comparative derivation |
| worker carriage | `worker/src/protocol.ts`, `worker/src/entry.ts` | versioned unions, exact operation/result transport, no identity or compatibility inference |
| browser durability | `app/src/shell/archive.ts` | additive schema, immutable child stores, metadata store, migration/operation journal, volatile parity, joined reads |
| shell coordination | `app/src/shell/worker-client.ts`, `app/src/shell/App.tsx` | prepared closure, guarded commit, ordered persistence, active pointer, recovery routing |
| task workspace | `app/src/shell/AutomationWorkbench.tsx` and its focused engineering components/CSS | browse/edit/preview/compare/transplant modes, literal states, keyboard navigation, no direct steering |
| shared projection | `app/src/render/scene.ts`, `app/src/render/index.ts`, WebGL/Canvas2D, inspection and audio owners | real thumbnails, draft register, reconstruction/diff/transplant geometry, renderer parity |
| authored language | `content/copy/catalog.json` | commands, consequences, refusals, recovery, unavailable evidence, compatibility and comparison explanations |

A packet reaches source-level `integrated` only when its canonical records,
worker carriage, browser persistence/recovery, normal workspace entry and exit,
copy, and M-024 projection all use the same identities and closed outcome
vocabulary. During the validation hold this does not imply compilation,
migration success, browser correctness, visual fidelity, human comprehension,
publication, or readiness; those claims remain deferred to the recorded gate.

### M-023 — Advanced contract ladder

**Outcome:** six later contracts combine the first-slice primitives into
progressively harder autonomous generators without introducing a second rules
language.

Before the first advanced opening is authored, `M-023-00 Evidence substrate`
extends the closed Rust metric/source/unit schema, qualification artifacts, and
addressed mechanism event vocabulary for per-output service, reserve,
purpose-typed upkeep, overload, signal and failover timing, typed material use,
reconstruction, and comparative cross-regime evidence. This packet also assigns
one versioned semantic projection owner shared by inspector, frame/event
projection, renderers, timeline, audio, and result surfaces. TypeScript may
format and select this evidence; it may not derive new authority from it.

Implementation order:

1. **Balance.** One finite source serves competing outputs. The contract unlocks
   Route limits and allocation weights, then measures per-output service,
   retained source stock, overload, and proportional behavior under shortage.
2. **Interference.** Declared Route noise, interruption, or misleading local
   status forces use of typed signals, timers, and ordered fallback rules. The
   contract never grants global fault state to policy code.
3. **Closure.** A circulating topology must service itself while paying typed
   upkeep and physical-compartment loss. The contract makes leakage, reserve,
   opening-stock retention, and weakest operating margin jointly legible.
4. **Renewal.** A local deficit signal, nearby typed material, donor Charge, and
   locally observable attachment data must recruit, position, and reconnect a
   replacement. External substitution remains a separate comparison assay.
5. **Transplant.** The same frozen GeneratorSpec is commissioned and qualified
   in multiple declared regimes. Policy changes create a new generator branch;
   assembly adaptation is recorded separately.
6. **Holdout.** A sealed candidate runs a suite-committed family under
   hands-off control. The suite reveals criteria and aggregate condition family
   before sealing, while schedules and seeds are withheld from policy and normal
   UI. Post-seal edits create an explicitly unsealed descendant rather than
   contaminating or rewriting the immutable sealed candidate.

#### M-023A - Balance

- **Opening:** one finite source, two receivers with different service demand,
  two outgoing Routes, and enough total capacity to make policy and allocation
  choices matter.
- **New power:** per-Route capacity limit and positive allocation weight;
  local source-stock and receiver-status signals already learned remain usable.
- **Pressure:** supply or demand changes create a shortage window in which both
  outputs cannot receive their unconstrained requests.
- **Cadence target:** one 30-36 displayed-second Commission episode begins with
  stable service, enters shortage within 8-12 seconds, recovers, and introduces
  a shorter second shortage. Revisions change allocation behavior rather than
  waiting through a longer schedule.
- **Design question:** which service floor is inviolable, which output can
  absorb variation, and how should retained stock be allocated before the next
  input window?
- **Criterion evidence:** per-output minimum and mean service, shortage-window
  allocation, source residue, overload, and uninterrupted critical-service
  duration. Passing requires every authored output floor, not only total flow.
- **Readable failure:** one branch starves while another consumes excess,
  residue accumulates behind a limit, or both Routes request more than the
  available source. Requested, accepted, and retained amounts remain visually
  distinct.

#### M-023B - Closure

- **Opening:** a small circulating topology inside a causal physical
  compartment, finite opening stock, typed upkeep obligations, leakage, and no
  free external replenishment sufficient to mask imbalance.
- **New power:** policy access to local operating margin and declared status
  signals across a cycle; no global closure sensor is introduced.
- **Pressure:** compartment loss and upkeep continually remove usable resource,
  so circulation alone cannot be mistaken for conservation.
- **Cadence target:** one 36-45 displayed-second loop exposes the first reserve
  crisis within 12 seconds, with upkeep draws and continuous declared leakage
  frequent enough that reserve placement and throttling decisions become visible
  during the same Commission attempt.
- **Claim boundary:** Closure means finite-horizon loop service under declared
  upkeep and leakage. It is not thermodynamic closure and does not imply that
  resource is created or perfectly retained.
- **Design question:** can local control keep every required Component above its
  operating margin while minimizing loss and preserving enough reserve to
  absorb timing variation?
- **Criterion evidence:** weakest operating margin, paid upkeep by purpose,
  leakage, reserve floor, opening-stock retention, and sustained circulation.
- **Readable failure:** the first depleted Component and its upstream loss path
  are emphasized; the loop does not display a healthy circulation motif when
  accepted flow has fallen to zero.

#### M-023C - Interference

- **Opening:** a functioning transfer network with declared Route noise,
  temporary interruption, and a local status-signal channel.
- **New power:** typed signal emission/decoding, elapsed timers, and ordered
  fallback rules. Policies still cannot read global fault identity or future
  schedule.
- **Pressure:** the normal Route becomes unreliable long enough that immediate
  retry, premature failover, and delayed failover produce different outcomes.
- **Cadence target:** one 36-40 displayed-second episode contains baseline,
  disturbance, recovery, and a second disturbance family; the first local
  symptom appears inside 10 seconds so timeout and fallback revisions can be
  evaluated without passive waiting.
- **Disturbance separation:** transfer interruption, Route allocation noise,
  sensor error, and signal corruption are distinct authored disturbances with
  separate events. A contract may compose them, but never labels one as
  another or hides their ownership behind a generic interference amount.
- **Design question:** which local evidence is sufficient to declare a fault,
  how long should the machine wait, and how does it return to normal operation
  without oscillation?
- **Criterion evidence:** service retained during disturbance, failover delay,
  false-switch count, recovery time, emitted/decoded signal records, and
  post-recovery stability.
- **Readable failure:** blocked, noisy, and disabled are separate Route stances;
  signal origin, propagation, decode, timer state, selected fallback, and
  recovery are traceable in order.

#### M-023D - Renewal

- **Opening:** one replaceable required Component, locally stocked compatible
  material, donor Charge, and attachment information available only through
  local relations.
- **New power:** deficit detection, typed material/status signals, local
  recruitment, placement, and reconnection actions using the established policy
  grammar and existing conserving renewal mechanics.
- **Pressure:** the required Component is degraded or removed after stable
  operation. The qualification harness knows the withheld target but does not
  expose it to policies.
- **Cadence target:** stable service is established for 10-12 displayed seconds,
  then one eligible Component is removed and the machine receives 15-25 seconds
  to detect, recruit, place, reconnect, and restore service. Later attempts vary
  failed identity, donor margin, and material location through authored inputs.
- **Information boundary:** every policy-visible repair input records its local
  provenance: heartbeat timeout, missing expected neighbor, decoded local
  message, locally sensed compatible material, or frozen local attachment
  address. A deficit signal may not encode the failed identity, required
  material, destination, or reconnection plan as an implicit repair oracle.
- **Design question:** can the machine detect loss, marshal finite material and
  resource, place a compatible replacement, and recover service without an
  external repair oracle?
- **Criterion evidence:** detection delay, donor expenditure, typed material
  use, assembly loss, replacement identity, local reconnections, recovery time,
  and restored service window.
- **Readable failure:** missing signal, wrong material, insufficient Charge,
  unreachable placement, incomplete reconnection, and service non-recovery are
  distinct outcomes. External substitution remains labeled as a comparison
  assay and cannot satisfy autonomous Renewal.

#### M-023E - Transplant

- **Opening:** one qualified blueprint and at least two declared destination
  regimes whose transport, dissipation, leakage, motion, or schedule differs.
- **New power:** no new hidden capability; the challenge exercises blueprint
  transplant, compatibility reporting, and existing local-policy primitives.
- **Pressure:** a design tuned to one regime encounters changed physical
  relationships while its GeneratorSpec remains byte-identical.
- **Cadence target:** paired 24-36 displayed-second source and destination runs
  expose one regime-owned difference inside 8 seconds. Matched-variable status
  remains literal; comparisons do not synchronize conditions that are declared
  different.
- **Design question:** which behavior belongs to the frozen organization, which
  depends on assembly choice, and which assumption was specific to the source
  regime?
- **Criterion evidence:** identical generator hash, destination assembly hash,
  regime identity, per-regime criterion vector, independent grades, and first
  divergence from the source behavior.
- **Comparison matrix:** the result lists unchanged GeneratorSpec fields,
  assembly differences, regime-law differences, matched or unmatched schedules
  and seeds, and the first observed divergence. It does not label a divergence
  as regime-caused when assembly or schedule also changed.
- **Readable failure:** the comparison aligns the same Component and Route
  addresses across regimes and localizes the first different accepted action or
  physical consequence. A policy edit creates a new branch and exits the frozen
  transplant claim.

#### M-023F - Holdout

- **Opening:** one qualified candidate, a visible condition family and criterion
  contract, a versioned sealed suite, and no live Commission controls.
- **Authority records:** Rust owns `HoldoutSuiteSpec`, public suite commitment,
  `CandidateSeal`, addressed `HoldoutExecution`, and append-only retirement or
  invalidity facts. The worker owns execution leases, withheld trial custody,
  cancellation/resumption, and coarse progress. React persists and presents
  those records but never generates a suite seed, seal hash, status, or pass.
- **New power:** sealing and evidence custody, not stronger hardware or policy.
- **Pressure:** suite-committed schedules and seeds sample declared operating
  variation; they are withheld from policy and normal UI, not claimed to be
  cryptographically secret on a static local client. The player cannot tune
  the sealed candidate against individual trials after the seal.
- **Cadence target:** candidate comparison and sealing are the deliberate work;
  the cold suite targets 8-12 addressed trials and a coarse result within 90
  wall seconds. A failed seal returns to a short descendant revision loop rather
  than exposing rescue control or a longer hidden wait.
- **Design question:** is the candidate's local organization robust enough to
  satisfy the declared function outside the commissioning traces used to tune
  it?
- **Criterion evidence:** seal identity, candidate hash, suite version, trial
  addresses, pass vector, worst retained service, first violation, trace
  identities, and sealed/unsealed lineage without disclosing withheld schedules
  through the normal UI.
- **Readable failure:** the result distinguishes a legitimate failed trial,
  invalid execution, unsealed descendant, and retired suite. Later edits never
  contaminate or rewrite the immutable sealed candidate; they create an
  explicitly unsealed descendant. A failed seal branches back to Design and
  exposes no rescue control.

The advanced contracts target 15-25 minutes because the reasoning space grows,
not because timers become longer. Each introduces one unfamiliar systems
problem, reuses prior primitives, places the first informative disturbance
early, and keeps reset-to-consequence latency short enough for deliberate
iteration.

The authored pacing budget is 2-3 minutes to read the opening and inspect the
machine, 4-6 minutes for the first design, 2-4 minutes for the first Commission
episode, 4-6 minutes for diagnosis and one focused revision, and 1-2 minutes for
qualification setup and result reading. These are experience targets rather
than pass rules. All actual schedules remain authored in deterministic
simulation steps, the first informative consequence targets 8-12 displayed
seconds, and no intended learning beat relies on more than 20 seconds of passive
wall-time waiting.

The M-024 companion work ships contract by contract: allocation stance with
Balance, circulation/loss stance with Closure, signal/failover stance with
Interference, damage/material/reconstruction stance with Renewal, same-machine
regime comparison with Transplant, and sealed instrument presentation with
Holdout.

Every contract adds one primary systems insight, reuses prior policy
primitives, introduces no invisible upgrade statistic, and remains replayable
from the same authoritative engine.

Handoff to M-025: all nine contracts use one manifest, authority model, policy
grammar, qualification runner, result schema, progression projection, and blueprint
system. No advanced contract requires campaign progression, direct control, or
a contract-specific simulation loop.

### M-024 — Mechanistic visual and audio fidelity

**Outcome:** every consequential state has a beautiful, coherent physical stance
and every prominent visual or sound is grounded in authoritative state.

This is a parallel implementation lane during M-018 through M-023, followed by
one integration pass after those mechanics are present:

1. **State grammar.** Define four distinct visual registers: embodied physical
   state, policy/controller state, evaluator evidence, and noncausal
   draft/interaction preview. Map idle, sensing, selected target, moving,
   Coupling, switching, requesting, accepted transfer, throttled, blocked,
   overloaded, physically failed, criterion-violating, recovering, frozen, and
   retained-result states without treating evaluator metadata as machine
   physics. Component, Route, Port, Supply, compartment, policy, and criterion
   marks remain distinguishable without relying on color alone.
2. **Machine models and materials.** Upgrade each hardware silhouette with
   readable storage, gate, sensor, actuator, attachment, and damage regions.
   Preserve the Number 2 graphite, translucent gel, filament, mineral haze, and
   luminous-resource direction while using actual source or generated raster
   assets for material detail rather than approximate UI drawings.
3. **Motion language.** Reservoir fill follows exact Charge; gates articulate on
   interface state; Route cadence follows accepted flow; sensing and policy
   target marks stay restrained; overload, fracture, and recovery remain local
   to affected relations. Decorative motion may add texture but never imply
   nonexistent transfer or control.
4. **Workbench composition.** Criteria and authority remain at the top, exact
   object/policy state at the right, causal history at the bottom, and the Field
   dominant. Dense panels use restrained borders and compact type; transient
   guidance expands in place without turning the workspace into a card stack.
5. **Audio system.** Replace Pulse and narrative cues with motifs driven by
   actual transfer cadence, interface switching, rising load, blockage,
   fracture, qualification freeze, failure, and recovery. Repetition is
   rate-limited, intensity follows measured values, and every motif has a
   reduced/muted path.
6. **Renderer parity and budgets.** WebGL and Canvas2D consume one scene
   projection. Low quality preserves causal geometry and exact state while
   reducing particles, bloom, texture layers, and interpolation work. Desktop
   and narrow compositions retain stable controls and readable labels.

The visual language is a causal legend carried by the machine itself:

| State | Required physical cue | Exact reading | Audio role |
|---|---|---|---|
| sensing | bounded scan or receptor activity anchored to the owning Component | sensor kind, range, readable candidates | normally silent |
| target selected | one stable relation from owner to deterministic target | target kind/id and selecting rule | subtle lock only on change |
| moving | chassis articulation and wake aligned to authoritative velocity | velocity, layer, target, action outcome | load follows movement expenditure |
| Coupling | actuator extension to true range and contact at the affected object | target, range, transferred amount or no-op reason | short contact/transfer motif |
| interface switching | mechanical gate movement at the exact endpoint | open/closed, owner, commanding rule | discrete open and close transients |
| Route requesting | static thin directional chevrons in a separate request channel; no moving resource mark | request and capacity | silent |
| accepted transfer | luminous cadence from tail to head proportional to accepted flow | accepted flow and destination headroom | cadence follows accepted transfer |
| throttled | continuous Route with a visible constriction/control collar | request, limit, accepted amount, controller | filtered transfer tone |
| blocked | broken or stalled stance at the exact blocking endpoint | blocking state and zero/partial acceptance | brief obstruction event, then quiet |
| overload | local strain at the overloaded object and connected relation only | load, capacity, margin | rising load with strict rate limit |
| embodied failure | loss of normal internal activity and localized fracture/depletion only when an authoritative physical event exists | failure event, affected object, quantities | single physical-failure event |
| criterion violation | evaluator rail localizes the violated relation while the machine keeps its actual physical stance | criterion, step, object, measured relation | single restrained violation event |
| recovering | staged material arrival, reconnection, and returning transfer | recovery stage and remaining deficit | sparse reconstruction events |
| retained qualification result | sealed instrument frame around an unchanged retained machine projection | result id and criterion vector | brief resolution motif |

Object models remain identifiable at gameplay scale. Each Component reserves
stable regions for storage, sensing, actuation, attachment, and damage so
dynamic state never shifts the silhouette or label layout. Ports visibly expose
their gate and ownership. Routes reserve separate channels for physical
connection, direction, control state, requested transfer, and accepted
transfer. Supply Stream artwork remains visually distinct from environmental
medium motion.

The first complete machine-model pass uses these silhouette contracts. Raster
material masks and authored textures provide surface detail; renderer geometry
owns causal articulation and does not approximate machinery with DOM/CSS art.

| Hardware | Stable silhouette and readable regions |
|---|---|
| Thread | narrow axial chassis, forward receptor fork, small central reservoir, and two aligned attachments |
| Ring | annular load-bearing body, reservoir visible through the center, radial Ports, and segmented local shell |
| Relay | long asymmetric truss, extended actuator beam, small reservoir, and strongly separated input/output attachments |
| Vault | broad faceted tank, dominant reservoir chamber, discharge gate, and fixed reserve graduations |
| Lens | forward aperture and receptor stack, narrow body, visible sensor origin, and directional range stance |
| Knot | compact multi-Port hub, radial attachment collars, carried-material rack, and deployed-junction interface |
| Wake | directional chassis with physically separate cache canisters and distinct occupied/released cache stances |
| Chorus | mechanically linked bodies with explicit couplers, one controller indicator, and separately readable reservoirs |

Stationary receiver, reserve, interface, module, and junction Components use the
same region grammar with distinct outer geometry. Every silhouette remains
identifiable in grayscale, at low quality, without a label, and while its
dynamic regions are empty.

The Number 2 material direction is interpreted as engineered life rather than
character art: graphite structure, translucent material volumes, mineral
particulate, fine conducting filaments, restrained luminous transport, and
localized stress. The palette uses several functional families rather than one
dominant hue: neutral structure, cool resource transfer, distinct sensor and
control accents, warm capacity pressure, and high-contrast failure. Color is
never the only carrier of state.

Animation follows exact state transitions:

- reservoirs interpolate toward authoritative Charge over roughly 100-160 ms
  while their exact label updates immediately;
- gate animation begins only after the interface transition is accepted, uses a
  120-180 ms articulation, and then holds its accepted state;
- flow particles are decorative samples of accepted flow and disappear at zero;
- sensor scans stay inside implemented range and do not touch ineligible
  objects;
- target links change only when the runtime target changes;
- failure propagation follows retained affected relations and never spreads as
  a generic full-screen effect;
- qualification progress is a cold-job instrument and does not fake a live
  rendered trial.

At `4x`, repeated events aggregate into approximately 100 ms display windows.
At `16x`, they aggregate into approximately 250 ms windows and emphasize stable
envelope readings over individual particles. Reduced motion removes repeated
sweeps, wake trails, and spatial fracture movement while preserving final gate,
target, transfer, damage, and evaluator geometry.

Workbench layout is stable under changing content. The top contract/authority
band, dominant Field, right inspection/editor rail, and bottom evidence band use
explicit responsive tracks and minimum sizes. At narrow widths, the Field and
authority controls remain in the primary view while inspector, editor, and
timeline become full-height sheets with clear return controls. Long object ids,
rules, criteria, and diagnostic text wrap without covering controls.

Audio is mixed as information, not ambience. It uses `events`, `machinery`,
`UI`, and `master` buses. Repeating sounds are generated
from aggregated state changes rather than one oscillator per object. Transfer
cadence groups nearby events, load intensity follows normalized authoritative
margin, and failure/recovery cues are sparse enough to remain learnable. Audio
settings expose master, events, continuous machinery, reduced motion/sound, and
mute; all critical state remains visually and textually available.

Transfer articulation is capped per nearby machine cluster, interface open and
close use distinct dry transients, blockage emits once and then remains quiet,
and priority physical failure or first-violation events briefly duck continuous
machinery. Preview and selection sounds are spectrally separate from physical
events. Muting audio removes no mechanical information.

Parallel pairing with the mechanical queue:

| Mechanical slice | Visual/audio companion delivered in the same pass |
|---|---|
| M-018 | policy stance, exact target relation, action outcome, Route controller/limit/enabled state |
| M-019 | live-versus-preview grammar, selection, editor focus, event-to-object trace, responsive workbench |
| M-020 | Intake sensing/acquisition, Transfer gates/request/acceptance, Buffer reservoir/phase |
| M-020E | Design/Commission/paused authority, branch lineage, causal breakpoints, provisional margin rail, restart reconstruction, and disabled Qualify boundary |
| M-021 | qualification freeze, coarse trial instrument, criterion resolution, first-violation trace |
| M-022 | blueprint thumbnails from real projection, structural/policy diff, lineage, regime transplant |
| M-023 | allocation, circulation/loss, signal/failover, renewal, cross-regime, and sealed-suite stances |
| M-025 | removal of campaign/Pulse cues, default ladder composition, final copy and icon consistency |

#### Rendering hierarchy

The Field uses a stable six-layer hierarchy. The layers are ordered by causal
importance, not by ornamental effect:

1. **Environment:** substrate, depth, physical compartment boundaries, Supply
   delivery regions, medium direction, and declared pressure. This layer moves
   slowly and stays below machine contrast.
2. **Machine body:** Component silhouette, Port placement, Route structure,
   attachment points, material regions, and persistent damage. These features
   remain identifiable while idle and at low quality.
3. **Resource state:** reservoir fill, accepted Route flow, Supply receipt,
   material movement, upkeep draw, and leakage. Resource marks use exact
   quantities or normalized physical capacity.
4. **Automation state:** sensor envelope, eligible set, selected target,
   commanding rule, actuator reach, Route controller, and live outcome. Only
   the selected or recently changed policy receives high contrast.
5. **Exception state:** throttling, blockage, overload, depletion, fracture,
   criterion risk, and recovery. Exceptions localize to the affected object or
   relation instead of tinting the entire workspace.
6. **Interaction state:** hover, selection, draft preview, edit handles, focus,
   event trace, and exact labels. Interaction marks are visually distinct from
   live automation so editing cannot be mistaken for operation.

Selection may raise the contrast of a lower layer, but it never changes layer
order or hides the machine's physical state. Bloom, haze, particles, and noise
are decorative sublayers and are removed before any causal layer under quality
or performance pressure.

#### Component model specification

Every Component model is a gameplay diagram embodied as machinery. Each model
must define these slots even when a particular chassis leaves one dormant:

| Slot | Required representation | State source |
|---|---|---|
| body | stable chassis silhouette and depth stance | Component kind, position, collision envelope |
| reservoir | bounded interior volume with exact fill and capacity markers | stored Charge and capacity |
| maintenance | restrained periodic draw at the consuming region | upkeep ledger and next allocation |
| sensor | physical receptor plus bounded active envelope | available sensor, range, current rule |
| actuator | articulated region and true effect reach | action kind, range, target, outcome |
| Ports | fixed attachment/gate locations with ownership | projected Ports and interface state |
| material | discrete carried/installed blanks or conductors | typed material inventory |
| damage | persistent localized loss or fracture region | embodied failure/recovery state |
| policy | compact state indicator linked to inspection, not a floating badge | active rule/action/outcome |

Forms and stationary Components share this grammar but keep distinct
silhouettes. A mobile chassis communicates movement capability before it moves.
A storage chassis makes reserve visible at ordinary zoom. A sensor-specialized
chassis makes receptor range readable without opening the editor. A junction
or transfer body shows Ports and flow ownership as physical attachments rather
than abstract list entries.

Routes use five separate channels: persistent physical path, tail-to-head
direction, controller/enable stance, requested transfer, and accepted transfer.
Requested flow cannot look identical to accepted flow. A closed endpoint,
disabled controller, source shortage, capacity limit, and destination headroom
must produce visibly different localized stances because they require different
player revisions.

#### Motion and transition timing

Animations communicate a state transition and then settle. They do not loop at
high salience merely because an object exists.

| Transition | Start condition | Settle behavior | Prohibited implication |
|---|---|---|---|
| sense | rule begins reading that sensor | low-contrast bounded scan while selected | detecting objects outside range |
| acquire target | runtime target identity changes | one short lock transition, then stable relation | retargeting every render frame |
| move | authoritative velocity leaves zero | articulation follows velocity and expenditure | motion toward a draft-only target |
| Couple | admitted action begins | extend, contact/result accent, retract or hold by state | transfer before contact/admission |
| switch interface | authoritative open state changes | gate moves to accepted state and stays there | preview opening a live endpoint |
| transfer | accepted flow becomes positive | cadence follows accepted amount and stops at zero | particles for requested-only flow |
| throttle/block | accepted/requested relation crosses named state | constriction or obstruction holds while condition persists | generic flashing disconnected from the cause |
| fail/recover | addressed failure/recovery stage changes | one localized transition, then persistent state | full-screen damage propagation |
| qualify | immutable request is accepted | editing withdraws and cold instrument takes focus | visualizing unseen hidden trials as live Field truth |

Animation timing is simulation-aware but wall-time-rate tolerant. At `4x` and
`16x`, presentation may aggregate repeated events and shorten transitions; it
may not reorder events, invent intermediate states, or make a zero-duration
state appear causally significant. Reduced-motion mode replaces spatial sweeps
and repeated pulses with stable state changes and concise transitions.

#### Workspace composition budgets

The Number 2 composition remains a dense instrument surface. It is not expanded
into a landing page or a collection of floating cards.

| Region | Desktop responsibility | Narrow responsibility | Stability rule |
|---|---|---|---|
| top authority band | contract function, weakest criterion, mode, Run/Pause, rate, reset, Qualify | function/status plus compact mode and rate controls | controls keep fixed hit areas and never wrap over the Field |
| Field | dominant causal machine and direct selection/edit geometry | dominant view with selection retained | reserves a minimum playable aspect and never shrinks for verbose guidance |
| right rail | selected-object readings, installed policy, draft editor, diagnostics | dedicated full-height inspector/editor sheet | long rules and ids wrap; tool controls remain fixed-size |
| bottom evidence band | aligned causal events, criteria, first violation, trace navigation | dedicated timeline sheet or compact current-event strip | dynamic events do not resize the Field |
| full workspaces | ladder, results, blueprints, comparison, labs | single-column task-specific workspace | sections are unframed bands; only repeated records use cards |

Copy has two densities: a compact operational label and an expandable literal
explanation. The compact layer never leaves a mechanic as an unexplained noun.
The expanded layer may be verbose and must include the affected object, input,
action, expected result, units, and no-op cause when those facts exist.

#### Asset and material pipeline

Visual assets are registered by gameplay slot rather than generated as generic
atmosphere. Each asset record states source, license/ownership, intended model
slot, native dimensions, crop, contrast range, quality tiers, and fallback.

- retained Number 2 sources govern composition, contrast, material direction,
  and density;
- generated raster assets are used for graphite, translucent material,
  mineral, membrane, damage, and filament texture only after the target slot is
  measured;
- icons come from the project's selected icon library and receive tooltips when
  their command is not universally familiar;
- no image or texture carries a mechanical state that is absent from the exact
  renderer scene data;
- Canvas2D may simplify texture and lighting, but it preserves silhouette,
  reservoir, gate, direction, target, outcome, and failure semantics.

#### Audio behavior specification

Audio events are aggregated from authoritative transitions and mixed by
information priority:

| Priority | Event family | Behavior |
|---:|---|---|
| 1 | first violation, fracture, qualification resolution | one unmistakable sparse transient with cooldown |
| 2 | interface switch, blockage onset, recovery stage | localized event cue on state change only |
| 3 | accepted transfer, movement load, sustained pressure | grouped continuous bed scaled by measured activity and margin |
| 4 | selection, editor commit, preview | quiet UI feedback that never resembles a physical action |

Pulse audio, character-like chirps, narrative stingers, one-sound-per-particle,
and constant warning loops are excluded. Muting audio never removes required
information; reduced-sound mode keeps only priority-one and essential
priority-two transitions at restrained level.

Asset production is slot-driven. Existing Number 2 sources are used as the
composition reference; generated raster materials or model texture assets are
created only after their rendered dimensions, crop, contrast, and quality-tier
role are specified. Meaningful machinery is not replaced by decorative CSS art,
handmade icon substitutes, or atmospheric assets that obscure state.

Completion condition: a first-time player can identify what each major object
is doing, which policy is commanding it, where resource is moving, and why a
failure propagated before opening the exact inspector; the inspector then
confirms the same causal account.

### M-025 — Default contract runtime and legacy retirement

**Outcome:** every normal entry path opens the contract ladder and no campaign
concept competes with the automation product.

Implementation slices:

1. Probe save bytes without restoring or autosaving, publish a V1-through-current
   format/kind support matrix, keep supported campaign saves importable for
   read-only inspection/export/regression tooling, and preserve unsupported or
   newer bytes as diagnosable opaque recovery data when possible. Label campaign
   records as legacy and never convert objectives into contract progress.
2. Make contract selection, hardware selection, Design, Commission, Qualify,
   result, and blueprint actions the default navigation path after import
   dispatch has established whether the session is normal or legacy.
3. Integrate ladder, workbench, qualification, result, engineering-memory,
   comparison, transplant, loading, refusal, recovery, and legacy surfaces under
   one Number 2 shell before deleting any surface that still owns a route.
4. Remove campaign rails, objective counters, chapter progression, steering
   prompts, Pulse prompts, Still Mode language, and direct-control bindings from
   normal copy and layout. Isolate or remove dead ramp, campaign, and
   compatibility code only after every required import boundary and replacement
   surface has an explicit owner.
5. Replace the campaign playtest guide with a complete automation guide covering
   every screen, visual object, authority state, policy primitive, expected
   action result, contract criterion, grade, failure report, reset boundary,
   blueprint action, and likely feature-versus-defect ambiguity.

The default route becomes:

```text
Contract ladder
  -> Contract brief and hardware envelope
    -> Design
      -> Commission <-> pause/revise/reset
        -> Qualify
          -> Result
            -> Archive / clone / compare / transplant / next contract
```

Normal navigation contains only the surfaces needed by that route plus Open
Field and analysis benches unlocked from engineering records. The Contract
Ladder is the startup root. Atlas is an optional regime-selection surface
reached from a contract, transplant, or Open Field flow; it is never startup
navigation or a story map. Forms appear only as
hardware profiles. Design replaces Still Mode with different authority
semantics. The policy-selected Couple action replaces direct Pulse control, and
exact inspection/evidence replaces the campaign Why control. Historical Still,
Pulse, and Why events keep their original legacy meaning; they are never
renamed or reinterpreted as automation records.

Legacy handling is explicit at every boundary:

- `probe_import` verifies bytes, digest, format, save version, protocol, and
  explicit run kind before normal run creation, restore, or autosave;
- import dispatch chooses exactly one automation, Open Field, supported legacy,
  unsupported/newer, corrupt, or opaque-exportable graph while authority remains
  catalog idle;
- a campaign save opens in a labeled legacy inspector with its original chapter,
  objective, direct-control, and event records intact;
- legacy runs can be replayed, inspected, and exported through isolated
  compatibility ownership but cannot unlock contracts or produce blueprints;
- normal copy, routing, worker inputs, audio, and renderer overlays do not load
  campaign concepts;
- compatibility code is removed only after all supported legacy data has an
  owned read path and unsupported data produces a literal import result.

Normal startup uses a versioned browser-owned `ActiveSessionPointer` containing
run kind, contract, attempt, branch, checkpoint/save key, content identity,
protocol identity, and pointer generation. The pointer is written only after
its referenced records and cleared only after persisted closure or explicit
abandonment. Rust rechecks every referenced identity on Resume. A stale pointer
becomes labeled recovery data; it never silently creates, opens, or deletes a
run.

The automation playtest guide is a product contract rather than a glossary. It
must include:

1. the complete default route and the purpose, permitted authority, entry, and
   exit of every screen;
2. a visual dictionary for every Component region, Port, Route stance, Supply
   stance, compartment, View, material, signal, pressure, policy, criterion,
   failure, and qualification mark;
3. every initial condition and action with readable inputs, target rule,
   physical effect, resource/material cost, cooldown, success result, and named
   no-op outcomes;
4. each contract's opening state, newly available primitives, expected
   mechanical realization, criterion vector, qualification behavior, common
   failed designs, and timing target;
5. the difference among draft, committed generator, live assembly, reset,
   branch, blueprint, qualification record, transplant, seal lineage, and
   legacy import;
6. a feature-versus-defect table for deterministic tie-breaks, quantization,
   Route rounding residue, headroom throttling, timers, accelerated display,
   hidden Holdout inputs, View noncausality, and unavailable actions;
7. a bug-report template that captures contract, mode, step, selected object,
   rule/action/outcome, generator and assembly identities, expected criterion,
   observed reading, reproduction actions, and attached export/trace.
8. 10-minute comprehension, 30-minute contract, and 60-minute
   progression/resume human session scripts with observation fields, stop
   conditions, and the exact evidence to retain; these scripts are not executed
   until the user lifts the validation and playthrough hold;
9. a labeled legacy appendix containing historical campaign controls, semantics,
   import support, and replay limitations so the normal guide contains no
   campaign framing.

The final copy pass removes poetic stand-ins for measurable objects and states.
Names may be distinctive, but every command, status, contract, failure, and
result can expand to a literal explanation with units and causal consequence.
No old narrative noun remains as the only explanation of a mechanic.

The `M-024X Product-surface closeout` travels beside M-025: default routes,
empty/loading/error states, imported-legacy state, all nine contracts, blueprint
surfaces, and qualification results receive one consistent composition, icon,
typography, motion, and sound pass as their replacement routes land. P-04 may
remove campaign chrome only after each affected state has that visible owner.

Completion condition: the game can be understood and played end to end without
encountering a chapter, objective, steering, Pulse, or unexplained campaign
noun, while existing evidence remains recoverable through the labeled legacy
path.

## Delivery risks and containment

The roadmap deliberately reuses a large existing simulation and presentation
surface. The main risks are boundary failures, not lack of feature ideas:

| Risk | Earliest warning | Containment in the implementation plan |
|---|---|---|
| policy schema changes silently alter old runs | a legacy run gains rules or a hash changes without an explicit migration | old saves default to no policy; migration version and canonical bytes are handled before editor features |
| TypeScript becomes a second automation engine | preview or result differs from exact runtime and no Rust record explains it | TypeScript may stage drafts and project geometry only; admission, target choice, transfer, criteria, and grades stay in Rust |
| frame growth damages the hot path | visible sections duplicate inspection payloads or update at unnecessary cadence | frames carry only continuously visible exact marks; detailed history and diagnostics use addressed events/inspection |
| editor permits impossible policies | install repeatedly fails for choices the UI presented as valid | capability and contract envelopes filter menus; local diagnostics mirror the canonical reason vocabulary; Rust remains final authority |
| visual polish overstates causality | flow, sensing, damage, or target effects appear without authoritative state | every prominent mark names its scene field and inspector reading; decoration is removed first under uncertainty |
| WebGL and Canvas diverge | a state is readable in one renderer and absent in the other | both consume one scene projection and share the causal-state acceptance matrix |
| contract content hard-codes shell logic | React contains thresholds, unlocks, schedules, or progression conditions | all contract quantities and progression inputs compile from versioned content into `ContractSpec` |
| qualification can be rescued or rewritten | live commands reach a cold trial or later grades mutate pass/fail | freeze closes causal commands; immutable requests/results and append-only evidence precede progression mutation |
| reset destroys reasoning history | a restart overwrites the prior candidate or evidence | generator, assembly, branch, attempt, and result identities stay separate; reset commands name their retained boundary |
| IndexedDB migration loses archive lineage | old records cannot be opened after blueprint schema changes | additive stores/versioned records, pre-migration read path, explicit legacy type, and no silent conversion |
| advanced contracts multiply special cases | a contract introduces a private loop, action, score, or progression path | one manifest, policy grammar, qualifier, result schema, and blueprint system remain mandatory for all nine contracts |
| fidelity work turns into a late reskin | new mechanics remain text-only until M-024 closeout | every mechanical slice includes its causal visual/audio companion before the next content dependency |

When a risk materializes, the correction remains inside the owning boundary.
It does not justify a second simulation, a hidden UI threshold, a fake renderer
state, or a contract-specific exception to identity and progression rules.

## Deferred verification gate

Implementation may proceed through the queue while the user-requested validation
hold is active, but no milestone may be represented as validated or published
without fresh evidence. When the hold is lifted, verification proceeds from the
smallest causal boundary outward: direct state inspection, Rust compile and
focused behavior checks, protocol/type checks, content checks, production build,
save/replay/migration checks, qualification reproducibility, browser interaction,
same-viewport Number 2 comparison, human-speed contract play, and laptop-facing
performance budgets. Failures produce bounded corrections; they do not justify
a parallel simulation path or silent criterion changes.
