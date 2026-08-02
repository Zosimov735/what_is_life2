# What Is Life 2 — Continuous Development Loop

Status: binding execution contract
Date established: 2026-08-02

## Contract

Development advances through one bounded milestone at a time. When a milestone
is complete, publish it, reconvene the panel, select the next milestone, and
continue without waiting for another ordinary instruction.

GitHub `main` is canonical. Chat, an unpushed commit, a remote scratch file, and
a laptop-only artifact are not durable project memory.

## Panel

The panel reviews implemented evidence rather than intention:

- **Science** reviews causal distinctions, quantities, claims, assays, and
  alignment with the scientific reference.
- **Game design** reviews fun, comprehension, controls, feedback, pacing, and
  player-visible consequences.
- **Engine and production** review architecture, migrations, deterministic
  behavior, build and distribution, and laptop performance.

The panel selects exactly one next milestone. Dissent may be recorded, but the
selected milestone must have one bounded outcome, explicit non-goals, and
observable completion gates.

## Cycle

1. Synchronize with `main`, read the active milestone and its owning contracts,
   and preserve unrelated work.
2. Record the milestone contract in [the milestone ledger](MILESTONES.md):
   outcome, scope, non-goals, affected layers, direct-inspection plan,
   post-implementation checks, laptop or distribution budget, and migration or
   rollback concern.
3. Implement the approved behavior. Never write a test first or use a newly
   authored failing test to drive implementation.
4. Inspect or play the implemented result directly. Inspect deterministic
   traces for causal-core work, use the real interface for interactive work,
   capture the selected viewport for visual work, and launch the built artifact
   for distribution work.
5. Only after the behavior exists, add or update validation. Run the relevant
   compile, type, behavior, content, production-build, visual, and performance
   checks. A blocked check remains blocked and may not be reported as a pass.
6. Update the owning documents, decision log, codebase state, and milestone
   evidence. Mark a milestone complete only when its stated gates are met.
7. Commit explicit paths, publish code and canonical documentation to `main`,
   and verify the remote commit and scope.
8. Reconvene the panel using actual play, traces, failures, and performance.
   Select and record one next bounded milestone, publish that documentation
   checkpoint, and repeat.

## Remote and laptop evidence

Remote development may satisfy cross-platform implementation and build gates. A
macOS launch, package, frame-time, memory, thermal, or energy claim remains
pending until it runs on a macOS runner or the target laptop.

Every hot-path or distribution milestone preserves a clean clone-and-run path
and records its laptop-facing budget. A remote-only service is not an acceptable
production dependency.

## Pause conditions

The loop pauses only when:

- the user pauses, replaces, or materially redirects the goal;
- the next step requires new authority, credentials, or an irreversible action;
- a product or scientific choice would materially change an approved contract;
- remote `main` cannot be reconciled safely; or
- required platform evidence is unavailable and proceeding would require a
  false completion claim.

An ordinary defect, failed validation, or panel disagreement does not end the
loop. It produces a smaller corrective or unblocker milestone.
