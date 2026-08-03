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
