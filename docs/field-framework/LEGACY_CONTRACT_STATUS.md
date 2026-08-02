# Legacy Contract Status

Status: recovery record  
Date: 2026-08-02

## Missing files

Older instructions, source comments, and documentation-contract checks cite
five files:

- `LEXICON.md`
- `SPEC.md`
- `PLAN.md`
- `FRAMEWORK.md`
- `ARCHITECTURE.md`

They were not present in the local workspace, the `what_is_life2` repository's
tracked history, its remote branches, or the related `what_is_life` repository.
The current Git history begins with a README-only commit followed by the
complete game import; the five files were never part of either tracked tree.

## What was recoverable

- The complete current source and authored content.
- `AGENTS.md`, which quotes some historical rules and describes completed work.
- Machine-readable vocabulary enforcement in `tools/lexicon-data.json` and
  `tools/lexicon-check.mjs`.
- Document contract checks that reveal portions of the old framework and
  architecture inventories.
- Three new Number 2 design documents produced from direct code inspection,
  panel review, and the supplied preprint.
- All 21 Number 2 mock-ups and the supplied scientific reference.

## Canonical treatment

The missing files are **lost legacy authorities**, not silently reconstructed
ones. Code and tests may be used to write a new versioned architecture or
lexicon, but that work must be labeled as a reconstruction and reviewed against
the implementation. It must not claim to reproduce the original wording.

Until that migration is complete:

- current source and authored content are implementation truth for v1;
- the documents indexed by [README.md](README.md) are target product authority;
- [DECISIONS.md](DECISIONS.md) resolves known scientific and systems conflicts;
- legacy documentation contract checks are known broken checks, not evidence
  that the new product documents are incomplete.

## Required follow-up

1. Replace stale root instructions with a concise repository-local contributor
   contract.
2. Write a new versioned architecture from code plus approved Number 2 changes.
3. Rebuild a human-readable lexicon from the machine rules and new literal
   player language.
4. Retire or replace contract checks that require unavailable originals.
5. Preserve this recovery record so a future contributor does not counterfeit
   the missing documents again.
