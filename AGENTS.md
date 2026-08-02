# What Is Life 2 — contributor instructions

Read this file before changing the repository.

## Read order

1. `docs/field-framework/README.md`
2. `docs/field-framework/DECISIONS.md`
3. `docs/field-framework/WORKING_RULES.md`
4. `docs/field-framework/CODEBASE_STATE.md`
5. The owning product, systems, visual, or implementation document linked from
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

## Causal and scientific guardrails

- A physical compartment and an observation View are different objects.
- Observation code receives immutable history and cannot mutate causal state.
- External substitution and autonomous local renewal are different assays.
- Stored resource, typed material, addressed signals, external control, and
  code length remain separate quantities.
- A Supply Stream delivers resource; it does not imply medium motion.
- A Form is the steerable commissioning chassis, not the complete generator.
- Small gameplay Ensembles report observed variation and pass results, not
  Shannon entropy, Hartley support, or channel capacity.
- Mock-ups govern composition where marked canonical; mechanics documents
  govern terminology, units, controls, and causal meaning.

## Player-facing copy

Every player-facing string belongs in `content/copy/catalog.json` and is read
through the `copy('key')` accessor. Do not introduce inline player copy in
React, worker, or Rust source.

The machine-enforced vocabulary currently lives in
`tools/lexicon-data.json` and `tools/lexicon-check.mjs`. A rebuilt human
lexicon must be labeled as a reconstruction and reviewed before it supersedes
those rules.

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

## Commands

Run from the repository root:

```bash
npm install
npm run dev
npm run build:content
npm run build:wasm
npm run build
npm run typecheck
npm test
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
