# What Is Life 2 — Working Rules

Status: contributor process authority  
Date: 2026-08-02

## Rule one: no TDD, ever

Do not write tests first. Do not use failing tests to discover or drive the
design. Do not frame implementation as making a newly authored test pass.

The required order is:

1. read the owning product, systems, and decision documents;
2. implement the behavior;
3. inspect the behavior directly when it is visible or interactive;
4. add or update validation after implementation;
5. run the relevant checks and report their actual results.

Specifications, pseudocode, diagrams, and causal contracts are not tests and
may precede implementation.

## Canonical memory

- Durable product decisions belong in `docs/field-framework/`.
- Visual sources belong in `docs/field-framework/assets/` with a manifest.
- Scientific sources belong in `docs/field-framework/references/` with license
  and status notes.
- A decision discussed only in chat is not canonical.
- Do not cite a lost document as if its original wording were known.

## Change discipline

- Preserve unrelated worktree changes.
- Stage explicit paths only in a mixed worktree.
- Keep passive observation code unable to mutate causal state.
- State whether a document describes current implementation, target behavior,
  or historical evidence.
- Mark proposed mechanics as proposed until the simulation enacts them.
- Use literal names, quantities, units, targets, and outcomes for player-facing
  instruments.
- Treat the mock-up text as provisional unless a mechanics document adopts it.

## Validation

Validation should match the change:

- documents: link, asset, heading, and terminology checks;
- Rust core: focused compile and behavioral checks after implementation;
- worker protocol: typecheck and bridge checks after implementation;
- React/Pixi UI: typecheck, interaction checks, production bundle, and direct
  browser inspection after implementation;
- visual work: same-viewport comparison against the selected Number 2 source;
- performance: laptop-class frame-time and memory measurements, not an
  assumption from desktop hardware.

When a toolchain or browser is unavailable, report the blocked check plainly.
Do not replace it with a claim of completion.
