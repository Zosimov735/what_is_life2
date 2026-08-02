# What Is Life 2 — Milestone Ledger

Status: canonical execution state
Updated: 2026-08-02

Exactly one milestone may be active. A completed entry is never rewritten to
hide a failed check, blocked platform gate, or panel dissent.

## Active

### M-001 — Reproducible remote-to-laptop build baseline

Selected by: science, game-design, and engine panel

Outcome: a clean checkout has a pinned, documented path that builds the
authored content, Rust/WASM core, worker, and production application. The same
static application is ready to become the payload of a lightweight macOS shell.

Why now: direct play, the causal-state migration, visual inspection,
performance work, and later packaging cannot be evaluated reliably without a
reproducible build.

Scope:

- pin Node, npm, Rust, the WASM target, and `wasm-pack`;
- make clean-checkout development preparation automatic and incremental;
- make the production build use locked dependency graphs;
- add remote validation for the complete static artifact;
- establish the browser/static-web/Tauri delivery boundary in
  [Platform and delivery](PLATFORM_AND_DELIVERY.md);
- preserve the Worker-separated deterministic Rust/WASM architecture.

Non-goals:

- no new regime or Form mechanic;
- no causal-state migration;
- no Tauri scaffold, native-Rust IPC migration, signing, or notarization;
- no visual redesign.

Completion gates:

- implementation precedes all new validation;
- a clean remote job installs pinned tools and produces `app/dist`;
- content, Rust, WASM, worker, and frontend checks pass against the pinned
  dependency graphs;
- the generated artifact contains the worker, WASM module, content, and app
  entry point;
- macOS package and performance status are explicitly recorded as pending;
- exact results and documentation are published to `main`.

## Completed

| ID | Outcome | Main commit | Direct evidence | Validation | Panel review / next |
|---|---|---|---|---|---|
