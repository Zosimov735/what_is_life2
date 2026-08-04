# What Is Life 2 — Decision Log

Status: binding decisions  
Date established: 2026-08-02

Each entry records a settled rule. Later changes require a new dated entry that
explicitly supersedes the old one.

## D-001 — Physical compartment and observation View are separate objects

Status: accepted; highest implementation priority.

A physical compartment determines causal membership and the game's declared
leakage law; rendered geometry is derived from membership. An observation View selects and groups recorded state for
analysis. Moving or clearing a View must not alter leakage, delivery, motion,
topology, or any other physical behavior. Reshaping a compartment is an
intervention; changing a View is measurement.

## D-002 — External substitution is not autonomous renewal

Status: accepted.

Replacing a Component while automatically inheriting position and Routes tests
material identity under externally supplied organization. A genuine renewal
trial removes the failed Component and its connections; frozen local processes
must detect the failure, recruit typed material, position a replacement, and
reconnect it. Player rescue is disabled and recorded separately.

## D-003 — The Form is the steerable commissioning chassis

Status: accepted.

The Form is the bright object the player steers and one Component of the final
generator. It is not the generator, a character class, an autonomous system, or an
unexplained archetype. Every Form discloses comparable measured fields and an
ability whose implementation status is explicit.

## D-004 — Resource, matter, control, and information remain separate

Status: accepted.

Stored usable resource `Q` is measured in `CU`; Route flow is `Q` moving.
Intervention Budget, typed material, addressed signals, and description cost in
bits are different quantities. The UI may not collapse them into one currency
or imply that energy-like resource becomes matter or information for free.

## D-005 — Supply delivery and medium motion are separate laws

Status: accepted.

The current engine's `Current` delivers finite `Q` to nearby Components. It does
not advect or push the Form. Player language uses **Supply Stream** until a
separate medium-velocity field is implemented. The two may share artwork but
never a hidden causal rule.

## D-006 — A View is not the paper's coarse-graining

Status: accepted scientific correction.

The preprint's coarse-graining `C` is a defined projection or partition of the
underlying state space. Only a rigorously declared measurement grain/projection
in the game can be compared to `C`. View membership, time window, and surround
are analysis-protocol variables; hold them fixed when comparing entropy or
support across grains. The full View tuple must not be called `C`.

## D-007 — Generator specification and embodied state are separate

Status: accepted scientific correction.

An immutable `GeneratorSpec` contains the frozen local rules, declared
Component kinds, topology constraints, and addressed inputs supplied to a
trial. The embodied state `S_t` contains positions, inventories, active Routes,
compartment state, material, and other changing values. Player-authored initial
conditions and topology are counted explicitly; the game must not map the
entire changing generator state to the preprint's static genomic specification
`g`.

## D-008 — Small ensembles are validation samples, not entropy estimates

Status: accepted scientific correction.

Eight or twelve seeded runs can estimate pass rate and observed outcome range
for a game contract. They do not establish Shannon conditional entropy or
Hartley support without a declared state alphabet, sampling distribution,
estimator, uncertainty treatment, and sufficient sampling or enumeration.
Compile and Ensemble screens use terms such as observed variation, pass rate,
and support observed in this trial set unless those stronger requirements are
met.

## D-009 — Renewal cannot be driven by an omniscient job oracle

Status: accepted scientific correction.

A central `RenewalJob` may record an experiment and its outcome, but it may not
provide the failed identity, correct material source, target position, or
required reconnection plan to the local dynamics. Those facts must be sensed,
encoded, or discovered through local rules and available signals. Otherwise the
experimenter has supplied the organization the trial claims to recover.

## D-010 — Mock-ups own composition, not mechanics or terminology

Status: accepted.

The Number 2 images are authoritative for visual composition and art direction
only where the screen specification marks them canonical. Labels such as Flux,
Phase, Source, Echo, and Blind Proof are replaceable. Mechanics documents own
quantities, units, controls, and scientific meaning.

## D-011 — The Atlas is the target shell; the eight chapters are legacy content

Status: accepted.

The target product is not limited to an eight-chapter campaign. The current
chapters remain playable scenarios and regression material. The Atlas organizes
expeditions, Open Field, bench studies, Holdout, Renewal, inheritance assays, and
Archive challenges.

## D-012 — No TDD

Status: accepted and absolute.

Test-driven development is prohibited for this project. Product and systems
contracts may precede code. Implementation comes next. Tests and other checks
are added or updated only after the behavior exists, to validate that behavior.

## D-013 — Development is a continuous panel-gated milestone loop

Status: accepted.

Development proceeds one bounded milestone at a time in this order: canonical
contract, implementation, direct inspection or play, post-implementation
validation, publication to `main`, panel review, and selection of the next
milestone. Ordinary milestone transitions require no additional approval.
Completion evidence, the published commit, panel findings, and the selected
next milestone are recorded in `MILESTONES.md`. The loop pauses only under the
stop conditions in `DEVELOPMENT_LOOP.md`.

## D-014 — Remote development and static dual delivery share one WASM core

Status: accepted.

Development uses a Vite server in the remote workspace. Production is a static application
and has no required application server. Browser delivery serves `app/dist` from
a static host; the first macOS package uses Tauri 2 to host the same assets in
WKWebView. Both initially run the deterministic Rust/WASM core in the dedicated
Worker. A native desktop Rust host requires profiling evidence and a later
superseding decision; packaging convenience alone is not sufficient reason to
create a second simulation path.

## D-015 — Reach the playable baseline through rapid implementation slices

Status: accepted.

Until the documented gameplay and visual dependency chain is implemented far
enough to evaluate as a coherent product, milestone work prioritizes working
code and direct play over exhaustive regression. Each slice still receives a
production build, focused boundary checks, and browser inspection before
publication. Slow broad suites and evaluative studies are deferred to explicit
stabilization milestones; defects found during implementation are fixed as they
surface. This does not permit test-driven development or claims unsupported by
the implemented mechanics.

## D-016 — The Azure test host is private and serves versioned static builds

Status: accepted.

The first Azure test environment is a rebuildable Ubuntu VM that serves the
existing static `app/dist` artifact from nginx. Nginx listens on loopback and is
reached through an SSH tunnel; public web ingress is absent, and SSH is limited
to an explicit management address in both the Azure network security group and
the guest firewall. Builds occupy versioned release directories and activation
uses an atomic `current` symlink so rollback does not require rebuilding. Azure
does not become the source of truth and does not introduce an application
server or a second simulation runtime.

The VM remains deallocated except during active testing and has no automatic
start. A daily 05:00 UTC shutdown schedule is a backstop for forgotten test
sessions. The resource group carries a low monthly budget with staged alerts;
deleting and rebuilding the entire environment remains the zero-persistent-cost
path because the host contains no canonical data.

## D-017 — Coupling is an instrumented world action

Status: accepted after first-impression playtest.

Active play binds Coupling to `E`: hold to extend the true world-space radius
and release to apply the existing zero-`Q` Pulse. Shift is not an alias, and
pointer input remains available for inspection and interface controls. Before
release, every affected Port, stored-resource source, and suppressible
Interference target must be identifiable on the Field; the shell summarizes
only nonzero outcomes. Supply delivery is shown as directed filaments into its
actual geometric recipients, while a stable reservoir arc shows the controlled
Form's stored `Q`. These readings are presentation derived from authoritative
state and do not alter the existing gather, open, discharge, or suppression
rules.

## D-018 — Campaign position and Port wiring stay continuously legible

Status: accepted after human-playtest preparation.

The active Field carries a persistent campaign rail sourced from authoritative
chapter events and frame progress: chapter title, chapter number, campaign
count, objective number, objective count, campaign elapsed time, chapter
progress, and overall progress. Directed Routes render a permanent tail-to-head
notch. A Route with either endpoint Port closed is visibly dormant, broken, and
terminated by a gate mark at the exact closed endpoint; it becomes continuous
only when both endpoint gates are open. Coupling's Port locks are sourced from
the exact Port ids in the core's projected release, never reconstructed from
projected screen distance.

The local `?field_run` shortcut enters the authored campaign at Form selection.
The legacy no-campaign stand-in is isolated behind `?field_stand_in`, so a
playtest link cannot silently open a Field with no chapter or objective state.

## D-019 — Mechanistic automation is the primary game

Status: accepted; supersedes the player-role and product-structure portions of
D-003, D-011, D-017, and D-018.

The primary game is a deterministic automation and commissioning game. The
player designs topology, configures frozen local policies, observes operation,
diagnoses failure, and qualifies a generator after editing is locked. Narrative
chapters, inhabitants, moral choices, and character framing are not part of the
target loop. Existing chapter Fields may supply placements and disturbance
ideas, but their serial objective campaign is legacy content rather than a
parallel primary mode.

## D-020 — Forms are programmable mobile Components

Status: accepted; supersedes D-003 and the manual-control portion of D-017.

A Form is a mobile hardware profile and one Component of the generator. It is
not directly steered. Movement, depth changes, Coupling, interface actions,
signals, and chassis abilities are actuators selected by the Form's frozen
local policy. The shell sends no per-step steering, Pulse, wheel, or handoff
control during normal automation play.

Policies may read only owning-Component state, locally detectable Supply,
neighboring Route state, decoded local signals, available local material, and
state-carried timers. Target selection and conflict resolution are stable and
deterministic. A qualification harness may evaluate hidden facts but may not
expose them to a policy.

## D-021 — Contract completion and engineering grades remain separate

Status: accepted.

The contract ladder replaces campaign progression. A contract passes only by
its explicit function-criterion vector. Throughput, resilience, economy, and
complexity are reported as four independent engineering grades under authored
bands; they are never collapsed into one score and do not replace pass/fail.
Qualification freezes the GeneratorSpec, initial assembly declaration, regime,
input schedule, criteria, and control contract before accelerated trials begin.

## D-022 — Design, Commission, and Qualify are distinct authority states

Status: accepted.

Design is immediately paused and permits topology, compartment, and policy
editing. Commission runs the deterministic Field at a selected wall-time rate
and permits pause, inspection, revision, reset, and comparison. Qualify runs
the frozen generator under `frozen_feedback`, exposes no rescue control, and
returns evidence after execution. Simulation rate is Worker-owned presentation
state and never enters physics, save identity, generator identity, or scenario
identity.
