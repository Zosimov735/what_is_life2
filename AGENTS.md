# field_game — standing instructions

Read this file before changing anything in this workspace. It holds the rules
that apply to every goal, not to any one of them.

Read in this order:

1. This file.
2. `docs/field-framework/LEXICON.md` — approved vocabulary and copy rules.
3. `docs/field-framework/SPEC.md` — the verbatim product specification.
4. `docs/field-framework/PLAN.md` — the goal ladder, one goal per agent.
5. Any other document under `docs/field-framework/` relevant to your goal.

## 1. Workspace boundary

Two directories may be created, modified, or deleted:

```
field_game/              the game
docs/field-framework/    the specification, framework, and vocabulary
```

**Nothing outside those two directories may be created, imported, or modified
by a workspace agent.** No file at the repository root, no sibling package, no
configuration elsewhere on the machine. If a goal appears to require a change
outside the boundary, stop and report it rather than reaching across.

This repository began empty, so no other package is present. The rule stands
anyway: it is the isolation rule, and the goal that eventually makes this game
the local launch target is the only one permitted to touch the root, and only
then because its own text says so.

`docs/field-framework/SPEC.md` and `PLAN.md` were supplied verbatim by the
project owner. Read them; do not edit them.

## 2. Isolation, in code

Nothing in `field_game/` may reach outside `field_game/`:

- No relative specifier that resolves above the workspace root.
- No absolute filesystem path, and no `file:` specifier.
- No build step that reads or writes outside the workspace.

The check enforces the first two for source and for Rust `include_str!`.

## 3. Approved vocabulary

The game uses no representational terminology. Use only the vocabulary in
`docs/field-framework/LEXICON.md` — in identifiers, comments, player-facing
copy, tests, API fields, error text, and documents alike. Two registers are
rejected outright: wording that depicts a thing from the world, and wording
that folds several readings into one number. LEXICON.md section 5 has the
detail and the reasoning; `tools/lexicon-data.json` has the list the check
reads.

Adding a noun or verb means adding it to LEXICON.md in the same change, with a
reason.

## 4. Copy catalog

Every player-facing string comes from one authored file:

```
content/copy/catalog.json
```

No component may write a player-facing string inline. Read it with the
accessor:

```
copy('objective.the_pull.follow_current')
```

Adding player-facing text means adding a catalog entry and referring to it by
key. LEXICON.md section 7 has the entry shape, the key format, the kinds and
their limits, the accessor contract, and the short list of strings that are not
player-facing copy — developer diagnostics, module specifiers, test names, and
technical values.

The accessor module does not exist yet; the goal that scaffolds the application
adds it. Keep the `copy('key')` call shape when you do, because that is the
shape the check looks for.

`index.html` is checked for vocabulary and for inline text. It may carry the
document title and nothing else a player reads: the check passes over the
title and reports every other text node the page holds. The neutral loading
surface is rendered from the catalog, not written into the page.

## 5. Running the checks

The whole suite is one command from `field_game/`:

```
npm test
```

It runs, in this order and stopping at the first failure:

| Step | Command | Covers |
|---|---|---|
| `test:core` | `cargo test --manifest-path core/Cargo.toml` | the Rust core |
| `test:units` | `vitest run` | the worker and the app, including the production-build check |
| `test:tools` | `node --test "tools/*.test.mjs"` | the check, the content build, and the document contract tests |
| `check:lexicon` | `node tools/lexicon-check.mjs` | wording, copy keys, and specifiers |

The other scripts, all run from `field_game/`:

```
npm install           once, and after any dependency change
npm run dev           the local preview on http://localhost:5173
npm run build         the content hash, the module, then the bundle in app/dist
npm run build:wasm    the module alone, into worker/wasm-pkg
npm run build:content the content hash, into worker/build/content.json
npm run typecheck     tsc over the worker, the app, and the tests
```

`npm test` builds the content hash and the module before the worker and app
tests, so a clean checkout needs no separate step. The build needs `wasm-pack`,
`cargo`, and the `wasm32-unknown-unknown` target.

`npm run typecheck` has two prerequisites the suite does not: the worker entry
imports the generated module and the generated content hash, so
`worker/wasm-pkg/` and `worker/build/` must exist. On a clean tree run
`npm run build:content && npm run build:wasm` (or `npm test`, or
`npm run build`) first.

The check also runs on its own, from the repository root and with no
dependencies installed:

```
node field_game/tools/lexicon-check.mjs
node field_game/tools/content-build.mjs            # once, on a clean checkout
node --test "field_game/tools/*.test.mjs"
```

The middle line is the one prerequisite the tools tests have: one of them reads
`worker/build/content.json` to check that the digest the worker imports is
current, and on a clean checkout nothing has written it yet. `npm test` and
`npm run build` both run it first, so this line is only needed when the tools
tests are run on their own.

Exit code 0 clean, 1 violations, 2 bad usage. `--json` gives a machine-readable
report; `--root <dir>` scans one directory as if it were the workspace;
`--help` explains both. Without `--root` the check scans all of `field_game/`
plus `docs/field-framework/` for wording.

`content-build.mjs` takes `--check` (report without writing, and lint the
authored geometry), `--geometry` (the lint alone, which reads the raw authored
numbers back out in whole units), and `--print`. Its exit codes are the same
three.

The checks and their tests are deliberately dependency-free and run on a bare
node install. They are wired into the scripts above unchanged, and must not be
replaced or weakened.

Run the suite before reporting any goal complete. A violation is a defect in
the change, not in the check; if you believe the check is genuinely wrong,
report that rather than working around it.

## 6. What is not scanned, and why

| Path | Reason |
|---|---|
| `tools/lexicon-data.json` | must spell out the terms the check rejects |
| `tools/fixtures/` | deliberately invalid input for the check's tests |
| `docs/field-framework/SPEC.md`, `PLAN.md` | supplied verbatim, quoted rather than authored |
| `node_modules/`, `target/`, `dist/`, `pkg/`, `wasm-pkg/`, `build/` | not authored here |
| `.git/` | version control metadata, not authored here |
| `.vite/` | bundler cache, not authored here |
| `package-lock.json`, `Cargo.lock` | generated |

Files under `tools/` and any `*.test.*` file are exempt from the inline-text
rule only, because they print developer-facing text by nature. Every
vocabulary rule still applies to them in full.

Adding an exclusion means adding a row here with a reason.

## 7. Goal discipline

- One goal per agent, in the order given in `PLAN.md`. Do not begin the next
  goal, however small it looks.
- Confirm the previous goal's acceptance criteria are present before starting.
  If a prerequisite is missing, stop and report it.
- Add or update focused tests for your goal. Run the narrow tests, the
  workspace test suite, and the production build once those exist.
- If visible behaviour changes, inspect the running result in a browser.
- Preserve unrelated changes in the working tree.
- **Do not stage and do not commit.** The orchestrator reviews and checkpoints.

## 8. Authoring a chapter

The campaign is eight chapters, and Stage F authors one per agent. Several of
those agents run at once, so each chapter owns a namespace and touches nothing
outside it. Everything shared is already wired for eight chapters — the
manifest lists all eight, the worker imports all eight, the test support module
embeds all eight — so a chapter agent replaces the placeholder standing in its
slot and edits no shared list at all.

**One chapter's namespace**, for a chapter with machine id `<id>`:

| What | Where | Shared? |
|---|---|---|
| The authored chapter | `content/chapters/<id>.json` | no — the whole file is yours |
| Copy keys | `<kind>.<id>.<name>` — `objective.the_edge.hold_inside`, `explanation.the_edge.hold_inside` | the catalog file is shared; your keys are not |
| The ending | `ending.<id>`, named by the chapter's own `ending_key` | as above |
| The chapter's name | `chapter.<id>` — already authored, closed set | do not edit |
| Tests | `core/tests/chapter_<id>.rs` | no — a new file per chapter |
| Manifest slot | the chapter's place in `manifest.json` `chapters` | already listed; do not reorder |

The key format is LEXICON.md's: a key opens with its kind, and the chapter id is
the segment after it. That is what keeps two chapters authored side by side from
naming one key, and it is why a merge of two chapter agents' work is mechanical:
each adds its own block of catalog entries and its own test file, and neither
moves a line the other wrote. **Add your keys as one contiguous block, appended
at the end of the entries object, in your chapter's own play order** — one block
per chapter, never interleaved with another chapter's. Two agents appending at
the same end of the file do meet there, so expect one add/add hunk; what the
scheme buys is that the hunk is the whole of it, because neither agent moves or
deletes a line the other wrote and the resolution is always to keep both blocks
in manifest order. Write
each objective and its own explanation as a pair, in play order, which is what
the opening chapter's block does; `chapter.<id>` and `ending.<id>` are authored
already and stay where they stand, so a chapter agent adds lines and deletes
none. The opening chapter's own keys predated the scheme and were migrated into
it by Goal 21, so `objective.the_pull.follow_current` and the rest are the shape
to copy.

**What a chapter file declares** is locked in `docs/field-framework/
ARCHITECTURE.md` (Authored content, and The campaign span): `id`, `title_key`,
`ending_key`, `layers`, `forms`, `ports`, `routes`, `currents`,
`authored_boundaries`, `objectives` (each with `optional`), `anchor_moments`,
`events`, `pressure_schedule`, and `opening_view`. Numbers are integers only,
raw Q32.16 for quantities. `node tools/content-build.mjs --geometry` reads the
authored geometry back in whole units, which is the way to check a placement
without arithmetic in your head.

**What the campaign runner does with it**, so a chapter is authored against the
rules rather than around them:

- Objectives are offered one at a time in authored order. The chapter completes
  when the sequence has nothing left to offer, and completing it carries the run
  into the next chapter — or, on the last chapter, ends the campaign on its
  `ending_key`.
- An objective with `optional` set is a test the chapter does not require: it
  stands for the span it authors and is passed over when the span runs out.
- `pressure_schedule` counts `start_step` from the chapter's own opening, not
  from the run's step counter.
- `events` are timed against an objective of the same chapter — `at` steps after
  that objective was offered — so a disruption lands during the challenge it was
  written for. **An event whose objective completes before `at` never fires, and
  nothing reports that**: it is not a fault, it is the rule — a player who
  finished early does not meet the disruption written for the part they
  finished. Author `at` well inside the span the objective realistically takes,
  and read your chapter's own test for the step it actually landed on rather
  than assuming it did. The idiom for a disruption that should stand for the
  whole of an objective's phase is `at: 1`, the earliest span a chapter may
  author: it falls due on the first step after that objective is offered, and
  nothing but an objective completing in zero steps can outrun it.
- Every identifier a condition, an event, or a schedule entry names must be
  something the chapter placed, and the campaign-level checks refuse a chapter
  that repeats another's objective id, pays better nearer the surface, or fails
  to establish under any one of the eight Forms. The refusal names the field.

**What a chapter agent does not touch**: another chapter's file or keys, the
manifest's order, `core/src/`, the worker, or the shell. A chapter that seems to
need a new rule is a report back to the orchestrator, not a rule added in
passing — the runner reads authored data, and a chapter is authored data.

**The campaign driver has a slot, and it is the only line of it you touch.**
`core/tests/campaign.rs` plays every chapter through
`core/tests/support/campaign.rs`'s `play_chapter`, which dispatches on the
chapter's own id: `script_for(id)` names a phase list, or the chapter is driven
by advancing steps with every input neutral, bounded by the span its own
objectives author. The rest driver completes only a chapter a Form standing
still can complete — which is what every placeholder is. **A chapter that asks
for steering, a Pulse, a Port or depth appends exactly two things**: a
`fn <id>() -> Vec<Phase>` beside the others, and one arm in `script_for`'s
`match`. Both are additions at the end of what stands, so several agents
appending at once meet in an add/add hunk whose resolution is to keep every arm.
Nothing else in either file is yours, and no agent moves or deletes a line
another wrote.

## 9. Current state of the workspace

Goals 1 to 21 are complete: the boundary, the vocabulary, the copy catalog, the
check, the framework and architecture documents, the scaffolded application, the
deterministic fixed-step runtime, the headless Field model, the worker bridge,
the renderer, steering, the Pulse, depth, the authored opening sequence,
Still Mode, queued changes, candidate Views and the ranking that compares them,
the coordinate profile and perturbation playback, the pressure system, Form
selection, the campaign runner, and the first chapter authored whole. The
model holds the six parts, the fixed-point arithmetic, the capacity
table, the Charge ledger, the trace's per-step records, and the four rules that
move Charge — Route flow, Boundary leakage, Node overload, and Current delivery
— beside Drain and the depth a step consumes. The Node phase pays upkeep at
last: every Node pays what its own authored rate prices, ascending, into the one
sink, and version 1 attributes the whole payment to the first of the five locked
purposes. Two more movers stand beside the four: that upkeep, and the Charge a
Trail entry delivers when it comes due.

The bridge carries the whole command surface that stands before durable
persistence: `init_run` in both modes, `input_frame` with the pause level, the
Still Mode toggle, and the step hook, `export_run`, `import_run` with the locked
import validation, `restore_checkpoint` and `recover_branch` over the records the
autosave cadence writes, and the three queued-change commands, valid only in
`still`. `undo_plan` and `commit_plan` are whole; what `queue_plan` puts in the
queue is the goal that owns queued changes, so until it lands nothing can be
queued and both of the others stand over an empty one. Persistence records live
in memory for the life of one core; the goal that owns persistence moves them to
IndexedDB and adds the shell's read API.

Still Mode is reachable and is the strategy layer's own surface. Space toggles
it: the time scale falls 65536 → 0 over 250,000 µs of real time, the run stands
fully paused with no step running and no input read whatever a frame asks for,
and Space again takes the same quarter second back. Space during either ramp
reverses it, mirroring the position so the scale is continuous across the turn
and a cancel costs only the time already spent. A pause remembers the mode it
interrupted — a window that blurs mid-inspection comes back to the same paused
Field, and one that blurs mid-ramp comes back moving, because a ramp is real
time and a suspended run spends none. Enter commits — which is also the exit —
Escape undoes, and an Escape that removes nothing is the press that leaves. The
paused Field puts up handles on every Port, every Route end, and every Boundary
vertex, lifts closed Ports to legibility, and stands the forecast overlay's own
strip, empty until baseline replays produce an envelope for it. The tray is
React chrome in a corner with nothing focusable in it; the mode, the queue, and
the Impulse it shows all come from the worker. `docs/field-framework/
ARCHITECTURE.md`'s Still Mode entry and exit section locks the rules the mode
table was silent on.

Authored content is real, and it is how a run starts. `content/manifest.json`
lists the chapters, Forms, and pressures in play order; the build hashes the
manifest bytes and every listed file's bytes in manifest order into
`worker/build/content.json`; the worker imports those files as their own bytes
and hands them to the core with the digest; and the core recomputes the digest
over exactly what it was handed and refuses the bundle when the two disagree, so
a generated digest that has gone stale cannot ship. `init_run` reports the hash,
a run records it, and a restore under a different one carries on with
`content_changed` set. All eight chapters are listed and load — `the_pull`, the
opening sequence, and seven placeholders of different lengths standing in the
slots Stage F authors — and all eight Forms carry their own authored parameters.

The campaign runner is what turns those eight files into one run. A chapter
completes when its sequence has nothing more to offer; the transition is settled
between two steps, on the same carriage a committed change and a pressure
boundary ride: the active window ends under the Field the recorded steps ran
under, the next chapter's Field and opening View are established from its own
authored content under the Form the run opened on, and the trajectory restarts
on what that leaves. The run carries across — its key, its branch, its random
state, its Impulse, its completed objectives, its Anchor metadata, and the step
counter, which is the campaign's and never restarts — and the Field does not. An
Anchor is written at every transition and an autosave record beside it, so a
Quick Retry across a boundary lands at the opening of the chapter the run
entered. The last chapter's completion clears the objective line and raises
`run_completed` with the ending its own content names, and the run stands in
`ended`, where it takes no further step. Chapters author optional tests, which
stand for a span and are passed over rather than blocking, and timed events —
a Port closed, a layer's loss raised, a current stopped — landing at a step
counted from the objective they were written for. `docs/field-framework/
ARCHITECTURE.md`'s campaign span section locks the whole of it, including the
campaign-level validation a bundle is held to at load.

Form selection is the game's first surface, and it stands before there is a run:
the eight Forms in the closed set's own order, one catalog promise each, no mark
on any of them and no figure anywhere on the surface, keyboard-operable with one
tab stop and the arrow keys inside it. The Form taken is `init_run`'s own field,
so it is part of the run and part of what a run records. What a Form is, is its
authored data — `route_reach`, `forecast_depth`, `leak_frac`, `upkeep_rate`,
`capacity`, `reserve`, and `abilities` — copied into the state the run stands on
when the Field is established: onto the Form, onto the run's Boundary, and onto
the Form's own Node. Nothing in the core or the shell branches on which Form was
selected. Two ability kinds are read: `linked_forms`, which stands several Forms
of one selection in the Field with exactly one of them steered, the rest holding
station on a derived control and accepting no delivery while they stand past
their authored separation; and `trail`, which leaves an entry every authored
period and delivers it where it was left, the authored delay later. A Form's
`steer_scale` is what makes one fast and another slow, and `upkeep_rate` is what
its Nodes pay each step. `reserve` is authored and the rule that spends it
stands reserved.

The opening sequence runs from that content: six objectives in authored order,
one visible at a time, each completing on an authored condition from a closed
set of six — standing in a named current, holding near a point, releasing a
Pulse, opening named Ports, named Routes carrying Charge, and the pattern held
without a Node standing over its threshold. The last of those is the authored
break: a loop Node carries more than it can hold, the objective reads
`failed_recoverable` while it does, and opening the relief Port beyond it
carries the pattern through. The opening layer authors no Drain and the closed
objective-state set carries no terminal failure, so layer 0 cannot lose a run.
That completion is the chapter's one Anchor moment, and the record is written at
the step it completes. The shell shows the objective, and `Why?` holds the
longer explanation — one button, no modal, the field moving behind it. Local
telemetry records the five firsts of the onboarding contract in the shell and
sends them nowhere.

The Pull carries on past it, and is the whole first chapter: three more
objectives take the run from that Anchor to the first transition. The player
descends a layer for the deeper current, is offered an optional test — the Port
that stands at that current's far end, which only a Pulse released beside it at
depth can reach — and comes back up for the final challenge, which is to hold
the bright current while Drift moves it out from under the Form. The chapter
schedules that Drift on layer 0 and authors one timed event against the final
challenge, and it asks for about twenty minutes of a first run against fourteen
and a half of a run that already knows it. `core/tests/chapter_the_pull.rs`
drives both, and the eight Forms through the whole of it.

```
field_game/
  AGENTS.md                     this file
  package.json                  workspace root: scripts, workspaces worker and app
  vitest.config.ts              the worker and app test run
  vitest.global-setup.ts        builds the content hash and the module first
  tsconfig.test.json            type-checks the tests
  content/                      the authored content
    manifest.json               what is authored, in play order
    chapters/<id>.json          the eight chapters, in play order
    forms/<id>.json             the eight starting Forms, parameters and abilities
    copy/catalog.json           the single copy catalog
  core/                         the Rust crate field_game_core
    Cargo.toml
    src/json.rs                 canonical JSON, the one canonicalizer
    src/read.rs                 the strict readers every incoming value goes through
    src/rng.rs                  the explicit random state
    src/sha256.rs               the digest canonical bytes are hashed with
    src/state.rs                the run state and the save payload
    src/content.rs              the authored bundle, the objective script, and the campaign
    src/coord.rs                the ten-coordinate profile
    src/field.rs                the Field's parts, its caps, and one step of it
    src/frame.rs                the render snapshot's locked byte layout
    src/fx.rs                   the fixed-point arithmetic, distance, adjacency
    src/perturb.rs              the eight perturbations, and the Echo highlight
    src/records.rs              the session's persistence records
    src/run.rs                  the fixed-step runtime
    src/plan.rs                 the bounded queue of proposed changes
    src/fault.rs                the one error envelope
    src/protocol.rs             the command surface
    src/lib.rs                  the WASM surface
    tests/                      the core tests, and the render snapshot's fixture
    tests/chapter_<id>.rs       one authored chapter, driven end to end
  worker/                       the npm package field-game-worker
    src/content.ts              the authored content, statically imported
    src/protocol.ts             envelopes and closed name sets
    src/frame-state.ts          the render snapshot's decoder
    src/entry.ts                the Web Worker entry and the accumulator
    test/                       the worker tests
    build/                      generated by tools/content-build.mjs, never scanned
    wasm-pkg/                   generated by wasm-pack, never scanned
  app/                          the npm package field-game-app
    index.html  vite.config.ts
    src/shell/                  the React shell and the worker client
      FormSelect.tsx            the opening selection: eight Forms, one promise each
      Objective.tsx             the one visible objective, and Why?
      ChapterReview.tsx         the chapter that closed, and the campaign's ending
      Inspect.tsx               the optional coordinate profile, and the Echo
      StillTray.tsx             the queued-change tray, and nothing focusable
      still.ts                  Space, Enter, Escape: the mode and its commands
      telemetry.ts              the five firsts, local and sent nowhere
      steering.ts               every device, into one normalized control
      dev-frames.ts             a development stand-in Field, never shipped
      dev-run.ts                a development stand-in run, never shipped
    src/render/                 the renderer, and everything drawn on the canvas
      index.ts                  create_renderer, the locked interface
      scene.ts                  one snapshot pair, projected into marks
      history.ts                trails, deliveries, cues: the renderer's ephemera
      palette.ts                the graphite direction, as numbers
      engine.ts                 what the two engines have in common
      webgl.ts                  the PixiJS engine
      canvas2d.ts               the fallback engine
    test/                       the shell, renderer, and production-build checks
  tools/                        the check, its tests, and the document tests
```

What runs today: the shell opens a worker, the worker loads the module and the
authored content and opens a run on the key it generates, and the shell shows the catalog's notice
until it answers and then a full-screen canvas the renderer draws on. An
animation-frame pump posts one `InputFrame` per rendered frame, the accumulator
turns those into fixed steps, and each frame comes back as a `frame` event
carrying the render snapshot the shell decodes. Each of those frames carries the
steering the shell holds: the cursor's offset from the middle of the surface, or
the direction the held keys name walked out over a few frames, normalized to one
Q1.15 vector by the one function both devices reach it through. The core springs
that vector into the controlled Form's velocity — a pull toward the point the
control names against a damper on the speed it stands at — and the position
phase integrates it. Blur, a hidden tab, and a cursor leaving the window each
let go of what is held. The renderer takes the two most
recent snapshots and the fraction between them and draws the Field: the
controlled Form and its trail, the currents, the Charge each Node holds, the
Routes and what they carry, the standing View's boundary, the haze between
layers, and the pressure reading at the edge. PixiJS draws it on WebGL, and a
Canvas2D engine draws the same scene at reduced fidelity when WebGL will not
start. Behind that canvas the core advances the run, records each step's control
state and stream position, and serializes the whole run state to canonical bytes
that repeat exactly for the same run key and frames. Blur and a hidden tab send
the pause level and stop the pump; a worker that faults is replaced and the run
picked up from the newest record the shell holds.

The renderer owns no state a run depends on. Its whole input is the decoded
snapshot pair, the viewport, and the motion profile; its trails, particles, and
cues are derived from successive snapshots and are lost, harmlessly, whenever
the surface is rebuilt.

A run stands on the chapter the manifest lists, established through
`Run::establish_field`, which takes the Field and the View it opens under: which
Nodes, Routes, Forms, and currents stand where is authored content, and the
worker hands that content to the core when it opens one. An established Field
advances with the run: its Forms move
between layers on the depth a step consumes, positions advance by velocity,
Form-kind Nodes take their Form's place, Routes carry Charge tail to head in one
ascending pass, each Node loses its layer's Drain, exposed members of the
standing inside leak, overloaded Nodes throttle their inflow and shed a quarter
of their excess, currents deliver to the Nodes standing in them, and each step's
records and Charge ledger are written into the retained trajectory. There is no
authored gameplay content, no captured input, and no durable persistence; the
goals that own those add them.

Until authored content arrives, a development build has two stand-ins for a
populated Field, each reached by a query marker on the local preview, each
behind `import.meta.env.DEV` by a dynamic import, and each dropped from the
production bundle along with the branch that names it — which the
production-build check reads the emitted files to prove.

`?field_fixture` draws `app/src/shell/dev-frames.ts`: scripted snapshots in the
locked byte layout, read back through the shared decoder, so the renderer sees
exactly what the worker would send it. Nothing behind it reaches the core, so
nothing behind it can be steered.

`?field_run` opens `app/src/shell/dev-run.ts` instead: an export file carrying a
small Field with one controlled Form, which the session opens with `import_run`
rather than `init_run`. It carries the content hash of a build with no content
at all, so a session opened on it reports `content_changed` and runs no authored
sequence — it is a place to steer with nothing scripted around it, which is what
makes it useful for reading input and rendering on their own. The two markers do
different things and are not combined.
