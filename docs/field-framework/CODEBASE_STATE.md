# What Is Life 2 — Current Codebase State

Status: dated implementation snapshot  
Baseline reviewed: `f187f6e`
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
- a causal `PhysicalCompartment` separated from the passive Observation View;
- React/Pixi WebGL rendering with a Canvas2D fallback;
- the Number 2 graphite Field treatment, authored texture, and separate physical
  and observational Still Mode instruments;
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
- Physical-compartment membership and leakage are chapter-authored causal state.
- Observation View membership is immediate analysis state and does not enter the
  transition, replay, perturbation, budget, or physical-plan paths.
- Frame V2 carries physical, proposed-physical, and View membership separately,
  plus the physical leakage coefficient used by the Still instrumentation.
- Component replacement can inherit externally supplied organization; genuine
  local recruitment and reconnection do not yet exist.

## Known simulation defects relative to the target

1. Immutable generator rules and embodied changing state are not yet separated
   into an explicit `GeneratorSpec` and `FieldState` ownership boundary.
2. Route updates occur sequentially by identifier, so identifier order can
   affect same-step multi-hop transfer.
3. Supply capture uses distance to authored path vertices rather than the
   nearest point on each path segment.
4. Wake can create delayed resource without first transferring it from a named
   source.
5. Lens does not yet run the cloned-state forecast its player promise requires.
6. Several Form distinctions are authored values without complete defining
   abilities.
7. The quantity model lacks typed replacement material and addressed signals.
8. Player steering, Pulses, rescue, and handoff need a unified external-control
   record for hands-off claims.

## Product mismatch

The current build launches into a visually overhauled Form selection and Field,
then runs the linear chapter campaign. It does not yet contain the target Atlas
shell, regime catalog, typed Observe and Intervention benches, analysis job
protocol, Holdout dashboard, Archive, or genuine local Renewal.

A local Atlas prototype and texture were created during the design session but
were intentionally excluded from the canonical documentation checkpoint
because the worktree also contained unrelated unfinished UI changes. Its dated
verification record is preserved in
[Atlas prototype QA](qa/ATLAS_PROTOTYPE_QA.md).

## Build and verification state

- Release Rust/WASM and Vite production builds pass locally.
- TypeScript checks pass.
- Focused M-002 validation passes 99 Vitest checks, 30 bridge checks, and the
  budget assertion; six measurement-only budget checks remain ignored.
- The lexicon check passes over 181 files and 281 catalog entries.
- Desktop and 319 × 699 direct-play captures are recorded in `design-qa.md`.
- Exhaustive campaign regression was explicitly stopped during the slow chapter
  simulations to prioritize rapid implementation of the remaining milestones.
- Legacy framework and architecture document contract checks remain broken
  because their source documents were never committed and could not be
  recovered.

## Dependency backlog

The selected current step lives in [the milestone ledger](MILESTONES.md). This
list records causal dependency order rather than a second active plan.

1. Separate immutable `GeneratorSpec` from embodied generator state.
2. Migrate save, protocol, frame, content, and rendering boundaries explicitly.
3. Make transfer and capture geometry physically order-independent.
4. Correct Form abilities and resource accounting.
5. Integrate the Atlas against one real regime.
6. Add passive instruments, typed interventions, and cold-path analysis jobs.
7. Add Holdout and Renewal only after external control and local information
   are represented honestly.
