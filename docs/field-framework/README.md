# What Is Life 2 — Canonical Documentation

Status: canonical index  
Established: 2026-08-02

This directory is the durable project memory for What Is Life 2. A decision is
not canonical merely because it appeared in a conversation or mock-up. It must
be represented here and merged to the repository's `main` branch.

## Authority order

When documents overlap, read them in this order:

1. [Decision log](DECISIONS.md) — settled corrections and non-negotiable
   distinctions.
2. [Product outline](PRODUCT_OUTLINE.md) — what the game promises and why it is
   fun.
3. [Atlas and mechanics contract](ATLAS_AND_MECHANICS.md) — regimes,
   quantities, instruments, experiments, objectives, and validation.
4. [Form and reality-of-play model](FORM_AND_PLAY_MODEL.md) — exact meaning of
   the steerable Form, resource flow, abilities, click targets, and causal
   model.
5. [Number 2 mock-up pseudocode](NUMBER_2_MOCKUP_PSEUDOCODE.md) — visual and
   implementation contract for every retained mock-up.
6. [Current codebase state](CODEBASE_STATE.md) — dated implementation truth and
   known mismatches.
7. [Working rules](WORKING_RULES.md) — contributor process, including the
   absolute no-TDD rule.

If a visual label conflicts with a mechanics document, the visual composition
remains and the label does not. If a target document conflicts with the dated
codebase state, the former describes the destination and the latter describes
what exists today.

## Preserved evidence

- [Number 2 asset manifest](assets/number-2/README.md) and all 21 source
  mock-ups.
- [Scientific reference snapshot](references/README.md), including the attached
  CC-BY 4.0 bioRxiv preprint.
- [Atlas prototype QA record](qa/ATLAS_PROTOTYPE_QA.md).
- [Legacy contract status](LEGACY_CONTRACT_STATUS.md), recording documents that
  could not be recovered instead of silently recreating them.

## Current implementation priority

The first causal migration is to introduce a physical compartment independent
of any observation View. The simulation transition must never read a View to
decide leakage, membership, permeability, or any other physical behavior.

After that:

1. separate immutable generator specification from embodied runtime state;
2. make Route transfer order-independent;
3. correct Supply capture geometry and resource accounting;
4. expose measurable Form contracts;
5. integrate the Atlas and first regime;
6. add passive instruments and typed counterfactual experiments;
7. build Ensemble, Holdout, and genuine local Renewal on the corrected causal
   model, and test whether the function continues.

## Editing rule

Change the narrowest owning document. Add a dated decision when a settled rule
changes. Do not edit a mock-up to smuggle in a new mechanic, and do not infer a
scientific claim from an attractive visualization.
