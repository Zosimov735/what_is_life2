# What Is Life 2

What Is Life 2 is an experimental systems game about building a compact local
generator, carrying it through different physical regimes, measuring what it
actually does, disturbing it, and learning whether it can keep a declared
function without continuous player rescue.

The current repository contains a deterministic Rust/WASM simulation, Web
Worker bridge, React/Pixi interface, eight authored legacy chapters, eight
steerable Forms, perturbation analysis, Still Mode, persistence records, and a
campaign runner. The target product replaces the chapter-first shell with a
navigable Atlas of physical regimes and makes every instrument, intervention,
resource, and success criterion quantitatively legible.

## Canonical documentation

Start with [the documentation index](docs/field-framework/README.md). It
contains the recovered product outline, settled decisions, current codebase
state, exact Form model, Atlas and mechanics contract, all 21 Number 2 mock-ups,
implementation pseudocode, and the scientific reference used during design.

The legacy `LEXICON.md`, `SPEC.md`, `PLAN.md`, `FRAMEWORK.md`, and
`ARCHITECTURE.md` files cited by older source comments were never committed to
this repository and could not be recovered. Their status is recorded explicitly
in [Legacy contract status](docs/field-framework/LEGACY_CONTRACT_STATUS.md); no
new document pretends to be a verbatim copy of them.

## Project rule: no TDD

This project does not use test-driven development. Implement behavior from the
approved product and systems contracts first, then update and run validation.
Tests verify an implementation after it exists; they do not define the design
in advance.

## Local commands

```bash
npm install
npm run dev
npm test
npm run build
```

The complete suite and production build require Rust, Cargo, `wasm-pack`, and
the `wasm32-unknown-unknown` target. See [AGENTS.md](AGENTS.md) for the current
workspace rules and detailed command notes.
