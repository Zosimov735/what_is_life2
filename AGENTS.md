# What Is Life 2 — contributor instructions

Read this file before changing the repository.

## Read order

1. `docs/field-framework/README.md`
2. `docs/field-framework/DECISIONS.md`
3. `docs/field-framework/PRODUCT_NORTH_STAR.md`
4. `docs/field-framework/CORE_FUN_LOOP_DESIGN.md`
5. `docs/field-framework/FIRST_SYNTHETIC_CELL_DESIGN.md`
6. `docs/field-framework/DEVELOPMENT_LOOP.md`
7. `docs/field-framework/MILESTONES.md`
8. `docs/field-framework/WORKING_RULES.md`
9. `docs/field-framework/CODEBASE_STATE.md`
10. The owning product, systems, visual, or implementation document linked from
    the canonical index.

The old `LEXICON.md`, `SPEC.md`, `PLAN.md`, `FRAMEWORK.md`, and
`ARCHITECTURE.md` files cited by legacy comments were never committed and could
not be recovered. Do not invent their original wording. Their status and the
archived v1 instructions are under `docs/field-framework/legacy/` and
`docs/field-framework/LEGACY_CONTRACT_STATUS.md`.

## Repository boundary

The repository root is the complete workspace. Code, authored content,
documentation, tools, and assets must remain inside it. Nothing may import,
read, or write a required build input above the repository root.

Preserve unrelated worktree changes. In a mixed worktree, stage explicit paths
only.

## Rule one: no TDD, ever

Test-driven development is prohibited.

The order is:

1. read the approved product and systems contract;
2. implement the behavior;
3. inspect visible behavior directly when applicable;
4. add or update validation after the implementation exists;
5. run the relevant checks and report exact results.

Do not author a failing test to drive design or implementation.

## Continuous execution

Follow `docs/field-framework/DEVELOPMENT_LOOP.md`. Complete one bounded
milestone, inspect it directly, validate it only after implementation, publish
the verified scope to `main`, reconvene the panel, record the next milestone,
and continue. Do not wait for approval between ordinary milestones. Panel
findings, evidence, and remote commit identifiers belong in
`docs/field-framework/MILESTONES.md`.

Exactly one milestone may be active. A new product specification does not
silently replace the active milestone. Record the bounded packet and its
dependency in the ledger before implementation begins.

## Core fun-loop guardrails

- The player creates artificial organisms from biological machines; internal
  engineering records remain the substrate, not the ordinary-play fantasy.
- The first player-facing organism is the entire bounded synthetic cell, not
  one Form or one Component.
- A Form is programmable mobile cellular machinery inside the generator, not
  the complete cell.
- Player-facing automation is taught as local biological response or
  regulation, not source-code programming.
- Symptoms appear before explanation; exact inspection and evidence then expose
  the causal relationship and editable boundary.
- The first retained adaptation is metabolic buffering and starvation
  tolerance produced by ordinary resource, reserve, transport, and response
  mechanics.
- Do not add population, reproduction, ecosystem, or evolution claims before
  their causal variation, inheritance, selection, and scale-specific mechanics
  exist.
- Do not add advanced systems to compensate for an unclear single-cell loop.
- Long-running persistence may create history and delayed consequences, but
  passive elapsed time may not replace decisions or turn the first slice into
  an idle game.
- Biological labels must expand to literal mechanics and may not imply a
  one-to-one real-cell model.

## Causal and scientific guardrails

- A physical compartment and an observation View are different objects.
- Observation code receives immutable history and cannot mutate causal state.
- External substitution and autonomous local renewal are different assays.
- Stored resource, typed material, addressed signals, external control, and
  code length remain separate quantities.
- A Supply Stream delivers resource; it does not imply medium motion.
- A Form is a programmable mobile Component, not the complete generator.
- Small gameplay Ensembles report observed variation and pass results, not
  Shannon entropy, Hartley support, or channel capacity.
- Mock-ups govern composition where marked canonical; mechanics documents
  govern terminology, units, controls, and causal meaning.
- The game is an artificial coarse-grained world, not a biological research
  tool or predictor.
- Terms such as genome, reproduction, and evolution are reserved for mechanics
  that support their causal meaning.

## Player-facing copy

Every player-facing string belongs in `content/copy/catalog.json` and is read
through the `copy('key')` accessor. Do not introduce inline player copy in
React, worker, or Rust source.

The machine-enforced vocabulary currently lives in
`tools/lexicon-data.json` and `tools/lexicon-check.mjs`. A rebuilt human
lexicon must be labeled as a reconstruction and reviewed before it supersedes
those rules.

Player language has three layers:

1. compact biological label for ordinary play;
2. expanded literal explanation with exact consequence;
3. internal technical identity for export, recovery, and advanced diagnostics.

Do not expose schema, hash, protocol, canonical-byte, journal, or stale-guard
language as the primary first-cell interface merely because those records are
necessary internally.

## Architecture

```text
React shell and Pixi/Canvas renderer
          |
          v
Web Worker protocol and fixed-step scheduler
          |
          v
Rust/WASM deterministic simulation core
```

- Rust owns authoritative causal state, deterministic transitions, save
  payloads, and analysis procedures.
- The worker owns fixed-step scheduling, protocol coordination, transferables,
  and long-job orchestration.
- React owns screens, controls, accessibility, instruments, and durable browser
  storage.
- Pixi/Canvas owns rendering derived from snapshots; renderer state never
  changes simulation outcomes.
- Biological presentation is a projection over this authority. It does not
  create a cell-specific simulation in TypeScript.

Remote development uses Vite. Production is a static artifact. The first macOS
package is a thin Tauri 2 host for the same Worker/WASM build and does not start
a production web server. Do not move the core to native desktop IPC without a
representative profile and an explicit superseding decision. See
`docs/field-framework/PLATFORM_AND_DELIVERY.md`.

## Commands

Run from the repository root:

```bash
nvm use
npm ci
npm run doctor
npm run dev
npm run build:content
npm run build:wasm:dev
npm run build:wasm
npm run build
npm run typecheck
npm test
npm run verify
node tools/lexicon-check.mjs
```

The full suite and build require Rust, Cargo, `wasm-pack`, and the
`wasm32-unknown-unknown` target. If a prerequisite, browser, or capture tool is
unavailable, report the blocked check instead of claiming completion.

## Canonical memory

Any durable product decision, new mock-up, scientific source, implementation
status change, or migration decision must be committed under
`docs/field-framework/` in the same change or an immediately preceding
documentation checkpoint. A chat-only decision is not canonical.
