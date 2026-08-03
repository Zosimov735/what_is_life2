# What Is Life 2 — Milestone Ledger

Status: canonical execution state
Updated: 2026-08-02

Exactly one milestone may be active. A completed entry is never rewritten to
hide a failed check, blocked platform gate, or panel dissent.

## Active

### M-003 — Immutable GeneratorSpec and embodied state split

Selected by: science, game-design, and engine panel

Outcome: frozen generator rules and declared topology constraints live in an
immutable `GeneratorSpec`; positions, inventories, active Routes, compartment
state, and other changing quantities remain in embodied `FieldState`.

Why now: M-002 removed the View from causal state, but authored rules and
changing state still share broad structures and initialization paths. That
ambiguity blocks honest inheritance, Holdout, Renewal, and specification-cost
work and makes later mechanic changes harder to reason about.

Scope:

- introduce an immutable `GeneratorSpec` owned by authoritative Rust state;
- move frozen local rules, declared Component kinds, topology constraints, and
  addressed trial inputs into the specification;
- keep positions, inventories, active Routes, compartment state, and other
  changing quantities in embodied state;
- make transition, replay, perturbation, save, and worker paths consume the
  split explicitly;
- preserve deterministic V2 save compatibility while versioning any necessary
  V3 boundary once, end to end;
- expose only player-meaningful specification and embodied readings in the UI.

Non-goals:

- no typed material, local recruitment, or genuine autonomous Renewal;
- no synchronous Route allocation or segment-based Supply capture correction;
- no entropy, capacity, or information-threshold claim;
- no Atlas integration or broad candidate-ranking redesign;
- no second simulation path outside the existing Rust/WASM Worker architecture.

Completion gates:

- implementation and direct inspection precede validation;
- immutable specification data cannot be changed through embodied transition or
  plan APIs;
- replay and perturbation preserve one frozen specification while changing only
  embodied state and explicit external inputs;
- save/export/import remains deterministic across the split;
- direct play continues through one representative chapter without changing
  the established visual and control surface;
- focused build, type, protocol, migration, and direct-play checks pass;
- exact results and documentation are published to `main`.

Panel resolution:

- Science requires the split before any genomic-specification analogy.
- Game design requires no interruption to the playable Field while the internal
  ownership model changes.
- Engine requires one explicit migration boundary and compile-visible ownership
  rather than parallel compatibility structures.

## Completed

| ID | Outcome | Main commit | Direct evidence | Validation | Panel review / next |
|---|---|---|---|---|---|
| M-002 | Physical Compartment is causal and paid; Observation View is independent, immediate, free, and passive. The Number 2 Still surface now renders both with distinct readings and authored field texture. | [`f187f6e`](https://github.com/Zosimov735/what_is_life2/commit/f187f6e) | Direct play selected Thread, entered Still Mode, switched from View to Physical Compartment, preserved budget 3 / queued edits 0 / cost 0, and produced no browser warnings or errors. Same-state source/implementation evidence is recorded in `design-qa.md` and `design-qa-comparison.png`; narrow 319 × 699 inspection found no region collisions after correction. | Release WASM and Vite production builds passed; TypeScript passed; focused validation passed 99 Vitest checks, 30 bridge checks, 1 budget assertion with 6 measurement checks ignored, and lexicon validation over 181 files / 281 entries. A later full suite passed build, typecheck, bridge, budget, campaign, and canonical JSON sections before the user explicitly stopped exhaustive validation in favor of rapid milestone implementation during the slow chapter simulations. | Panel selected M-003, the immutable `GeneratorSpec` / embodied-state split. Exhaustive campaign regression is deferred until the gameplay baseline is implemented far enough to evaluate. |
| M-001 | Reproducible remote-to-laptop static build baseline | [`9df748b`](https://github.com/Zosimov735/what_is_life2/commit/9df748ba76a83df566afaec9263b73b4b79448f4) | Clean Ubuntu 24.04 run [`30765055465`](https://github.com/Zosimov735/what_is_life2/actions/runs/30765055465); exact toolchain doctor; eight-file production artifact, 1,423,134 bytes unpacked and 472,476 bytes archived; worker bundle contains content hash `6aa41dc6…`; WASM is 662,243 bytes. macOS package, signing, launch, frame-time, memory, and energy evidence remain pending. | Release Rust/WASM build passed; app and test TypeScript passed; 534 Rust checks passed and 6 were ignored; 30 Vitest files / 313 checks passed; 35 tool checks passed and 76 lost-document checks remained explicitly skipped; lexicon checked 181 files with no violations. The inherited unoptimized core suite consumed about 25 minutes and is recorded as an iteration-cost issue. | Panel selected M-002, the compile-enforced physical-compartment / passive-View split. |
