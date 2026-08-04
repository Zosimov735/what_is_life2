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
3. [Automation and contract authority](AUTOMATION_AND_CONTRACTS.md) — the
   primary loop, local-policy grammar, contract ladder, qualification, and
   blueprint boundary.
4. [Atlas and mechanics contract](ATLAS_AND_MECHANICS.md) — regimes,
   quantities, instruments, experiments, objectives, and validation.
5. [Form and reality-of-play model](FORM_AND_PLAY_MODEL.md) — exact meaning of
   the steerable Form, resource flow, abilities, click targets, and causal
   model.
6. [Number 2 mock-up pseudocode](NUMBER_2_MOCKUP_PSEUDOCODE.md) — visual and
   implementation contract for every retained mock-up.
7. [Platform and delivery](PLATFORM_AND_DELIVERY.md) — remote, static-web, and
   macOS runtime and packaging boundaries.
8. [Current codebase state](CODEBASE_STATE.md) — dated implementation truth and
   known mismatches.
9. [Continuous development loop](DEVELOPMENT_LOOP.md) — binding milestone,
   publication, and panel-review cycle.
10. [Milestone ledger](MILESTONES.md) — the sole authority for the one active
   implementation milestone and its evidence.
11. [Working rules](WORKING_RULES.md) — contributor process, including the
   absolute no-TDD rule.

If a visual label conflicts with a mechanics document, the visual composition
remains and the label does not. If a target document conflicts with the dated
codebase state, the former describes the destination and the latter describes
what exists today.

## Preserved evidence

- [Number 2 asset manifest](assets/number-2/README.md) and all 21 source
  mock-ups.
- [Human playtest guide](PLAYTEST_GUIDE.md), including the eight-chapter clock,
  objective-by-objective progression, the complete visual and sound dictionary,
  controls, expected causal results, and high-risk bug-report areas.
- [Scientific reference snapshot](references/README.md), including the attached
  CC-BY 4.0 bioRxiv preprint.
- [Atlas prototype QA record](qa/ATLAS_PROTOTYPE_QA.md).
- [Legacy contract status](LEGACY_CONTRACT_STATUS.md), recording documents that
  could not be recovered instead of silently recreating them.

## Active implementation milestone

[The milestone ledger](MILESTONES.md) is the sole authority for what is active
now. Implementation sequences in the mechanics contract, mock-up pseudocode,
and dated codebase state are dependency backlogs; they do not compete with the
one selected milestone.

After a milestone is implemented, inspected, validated, and published, the
science, game-design, and engine panel reviews its evidence and selects the next
bounded milestone.

## Editing rule

Change the narrowest owning document. Add a dated decision when a settled rule
changes. Do not edit a mock-up to smuggle in a new mechanic, and do not infer a
scientific claim from an attractive visualization.
