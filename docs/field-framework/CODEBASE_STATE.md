# What Is Life 2 — Current Codebase State

Status: dated implementation snapshot  
Baseline reviewed: `caee15f5c452a4a1d5b2afe66595e73fee0a8d88`  
Date: 2026-08-02

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
- React/Pixi WebGL rendering with a Canvas2D fallback;
- a worker protocol that keeps the simulation off the UI thread.

## What the current engine really does

- A Form moves continuously in two dimensions and across discrete layers.
- The controlled Form's position is mirrored into a Form-kind Component.
- Other Components and graph topology remain authored unless an explicit plan
  changes them.
- Route transfer is algebraic rather than particle-based.
- A `Current` supplies finite Charge to nearby Components; it does not push the
  Form.
- Drift displaces that Supply path.
- Form selection currently copies several parameters into run state, including
  a leakage fraction that incorrectly affects the global Boundary.
- The standing View's `inside` set currently participates in leakage, so
  observation and physical membership are causally entangled.
- Component replacement can inherit externally supplied organization; genuine
  local recruitment and reconnection do not yet exist.

## Known simulation defects relative to the target

1. Physical compartment membership and observation View are the same causal
   data path.
2. Selected Form leakage can change the whole generator Boundary.
3. Route updates occur sequentially by identifier, so identifier order can
   affect same-step multi-hop transfer.
4. Supply capture uses distance to authored path vertices rather than the
   nearest point on each path segment.
5. Wake can create delayed resource without first transferring it from a named
   source.
6. Lens does not yet run the cloned-state forecast its player promise requires.
7. Several Form distinctions are authored values without complete defining
   abilities.
8. The quantity model lacks typed replacement material and addressed signals.
9. Player steering, Pulses, rescue, and handoff need a unified external-control
   record for hands-off claims.

## Product mismatch

The baseline launches into Form selection and a linear chapter campaign. It
does not contain the target Atlas shell, regime catalog, typed Observe and
Intervention benches, analysis job protocol, Holdout dashboard, Archive, or
genuine local Renewal.

A local Atlas prototype and texture were created during the design session but
were intentionally excluded from the canonical documentation checkpoint
because the worktree also contained unrelated unfinished UI changes. Its dated
verification record is preserved in
[Atlas prototype QA](qa/ATLAS_PROTOTYPE_QA.md).

## Build and verification state at recovery

- TypeScript app and test checks for the local Atlas prototype passed.
- Twenty-two focused shell and Form-selection checks passed.
- The lexicon check passed with 306 catalog entries.
- Production bundle verification was blocked because generated WASM and the
  Rust/`wasm-pack` toolchain were unavailable in that environment.
- Same-viewport browser capture was blocked because no browser binary was
  available.
- Legacy framework and architecture document contract checks remain broken
  because their source documents were never committed and could not be
  recovered.

## Dependency backlog

The selected current step lives in [the milestone ledger](MILESTONES.md). This
list records causal dependency order rather than a second active plan.

1. Add a `PhysicalCompartment` to authoritative Rust state and remove every
   causal dependency on `ViewDeclaration`.
2. Separate immutable `GeneratorSpec` from embodied generator state.
3. Migrate save, protocol, frame, content, and rendering boundaries explicitly.
4. Make transfer and capture geometry physically order-independent.
5. Correct Form abilities and resource accounting.
6. Integrate the Atlas against one real regime.
7. Add passive instruments, typed interventions, and cold-path analysis jobs.
8. Add Holdout and Renewal only after external control and local information
   are represented honestly.
