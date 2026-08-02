# What Is Life 2 — Milestone Ledger

Status: canonical execution state
Updated: 2026-08-02

Exactly one milestone may be active. A completed entry is never rewritten to
hide a failed check, blocked platform gate, or panel dissent.

## Active

### M-002 — Causal compartment and passive View split V2

Selected by: science, game-design, and engine panel

Outcome: the physical compartment is the only source of leakage membership and
coefficient. An Observation View is an independent, free analysis selection
whose creation, movement, replacement, or clearing cannot change the simulated
trajectory.

Why now: the current transition reads `RunState.view.inside` as physical
membership, so observation changes the system being observed. It also copies a
selected Form's leakage value into the whole generator. Both violate D-001 and
block every credible Observe, Holdout, Archive, and Renewal feature.

Scope:

- add an authoritative `PhysicalCompartment` to causal `FieldState`;
- remove View or generic `inside` arguments from live, cached, replayed, and
  perturbed transition paths so the separation is compile-enforced;
- make physical-compartment membership and its leakage coefficient
  chapter/regime-authored rather than Form-authored;
- keep a compartment edit queued, paid, causal, and trace-ending;
- make View selection immediate, free, and unable to mutate causal state,
  Intervention Budget, the plan queue, or the retained trajectory;
- version content, saves, worker protocol, and frame layout together and provide
  one deterministic V1-to-V2 save migration;
- render the physical compartment as a thick material edge and the View as a
  thin violet aperture, with separate physical and observational readings;
- retain the dedicated Worker and Rust/WASM delivery architecture.

Non-goals:

- no complete `GeneratorSpec`/embodied-state migration;
- no typed material, local recruitment, or genuine autonomous Renewal;
- no synchronous Route allocation or segment-based Supply capture correction;
- no entropy, support, capacity, or external-category-sufficiency claim;
- no redesign of the legacy candidate-ranking procedures;
- no full Observe Bench, Atlas expansion, or broad Number 2 polish pass;
- no multiple compartments, material families, or editable permeability.

Completion gates:

- implementation precedes all new validation;
- no transition, cache, or physical replay API accepts a View or View member
  list;
- arbitrary View edits leave causal Field bytes, step records, leakage,
  random-stream position, retained span, and Intervention Budget unchanged;
- a committed compartment edit can change leakage but leaves the active View
  unchanged, and every Charge ledger remains exactly balanced;
- Thread and Ring open the same authored compartment under the same regime;
- a verified V1 save migrates deterministically and a V2 export/import remains
  byte-identical;
- the frame carries separate physical, proposed-physical, and View membership;
- direct play shows a free thin View moving independently of a paid thick
  compartment edge;
- the 32 KiB frame cap, 8 MiB save cap, 30-step-per-second simulation contract,
  and server-free static production boundary remain intact;
- exact results and documentation are published to `main`.

Panel resolution:

- Science required a View-blind transition and prohibited any claim that this
  implements the preprint's information threshold.
- Game design defined the playable contrast as harmless scanning versus
  consequential compartment work, with both objects visible at once.
- Engine required the separation to cross state, replay, content, save,
  protocol, and frame boundaries together rather than leaving a compatibility
  seam that could re-couple them.
- Game design proposed deferring Form-independent leakage ownership. Science
  and engine rejected that deferral because it would preserve a known false
  global coupling. M-002 therefore moves the provisional coefficient into the
  chapter/regime contract now; later tuning remains content-only.

Migration concern: a V1 run was produced while View and physical membership
were coupled. Migration copies that verified legacy membership into both V2
objects, clears stale analysis evidence, and labels its origin. It does not
pretend the earlier run supplied passive-observation evidence.

## Completed

| ID | Outcome | Main commit | Direct evidence | Validation | Panel review / next |
|---|---|---|---|---|---|
| M-001 | Reproducible remote-to-laptop static build baseline | [`9df748b`](https://github.com/Zosimov735/what_is_life2/commit/9df748ba76a83df566afaec9263b73b4b79448f4) | Clean Ubuntu 24.04 run [`30765055465`](https://github.com/Zosimov735/what_is_life2/actions/runs/30765055465); exact toolchain doctor; eight-file production artifact, 1,423,134 bytes unpacked and 472,476 bytes archived; worker bundle contains content hash `6aa41dc6…`; WASM is 662,243 bytes. macOS package, signing, launch, frame-time, memory, and energy evidence remain pending. | Release Rust/WASM build passed; app and test TypeScript passed; 534 Rust checks passed and 6 were ignored; 30 Vitest files / 313 checks passed; 35 tool checks passed and 76 lost-document checks remained explicitly skipped; lexicon checked 181 files with no violations. The inherited unoptimized core suite consumed about 25 minutes and is recorded as an iteration-cost issue. | Panel selected M-002, the compile-enforced physical-compartment / passive-View split. |
