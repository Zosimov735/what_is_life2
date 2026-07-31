// Contract test for docs/field-framework/ARCHITECTURE.md.
//
// ARCHITECTURE.md freezes module ownership and every cross-module interface:
// core types, the PlanCommand union, the worker protocol, save version 1,
// error envelopes, and the determinism and serialization rules. Later goals
// scaffold and implement exactly what it locks, so this test pins the locked
// inventory and the load-bearing phrases another goal depends on. It is a
// contract test on the document, not a prose-quality test.
//
// Run from the repository root:
//   node --test "field_game/tools/*.test.mjs"

import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import { fileURLToPath } from 'node:url';

const DOC_PATH = fileURLToPath(
  new URL('../../docs/field-framework/ARCHITECTURE.md', import.meta.url),
);
const DATA_PATH = fileURLToPath(new URL('./lexicon-data.json', import.meta.url));

const text = fs.existsSync(DOC_PATH) ? fs.readFileSync(DOC_PATH, 'utf8') : '';
const lines = text.split('\n');
const data = JSON.parse(fs.readFileSync(DATA_PATH, 'utf8'));

/**
 * Returns the body of the section opened by the exact heading line, up to the
 * next heading of the same or shallower depth. Empty string when absent.
 * Whitespace runs are collapsed so a locked phrase is found across a Markdown
 * line wrap.
 */
function sectionBody(heading) {
  const depth = heading.match(/^#+/)[0].length;
  const start = lines.findIndex((line) => line.trim() === heading);
  if (start === -1) return '';
  let end = lines.length;
  for (let index = start + 1; index < lines.length; index += 1) {
    const match = lines[index].match(/^(#+)\s/);
    if (match && match[1].length <= depth) {
      end = index;
      break;
    }
  }
  return lines.slice(start + 1, end).join('\n').replace(/\s+/g, ' ');
}

const REQUIRED_SECTIONS = [
  '## Locked stack and targets',
  '## Module ownership and workspace layout',
  '## Runtime topology and timing',
  '## Public core types',
  '## Field model rules',
  '## Worker protocol',
  '## Error envelope',
  '## Determinism and serialization',
  '## Still Mode entry, exit, and the inspection surface',
  '## Still Mode analysis budget',
  '## Save version 1 and persistence',
  '## Authored content',
  '## Offline operation and build chain',
];

// The seventeen public core types locked by SPEC.md.
const CORE_TYPES = [
  'RunState',
  'FrameState',
  'FieldLayer',
  'FormState',
  'CurrentState',
  'PortState',
  'RouteState',
  'BoundaryState',
  'ViewDeclaration',
  'CandidateSlate',
  'PrivilegeProfile',
  'CoordinateProfile',
  'PressureState',
  'ObjectiveState',
  'CheckpointState',
  'InputFrame',
  'PlanCommand',
];

const PLAN_VARIANTS = ['connect', 'redirect', 'cut', 'reshape_boundary', 'set_focus'];

const WORKER_COMMANDS = [
  'init_run',
  'input_frame',
  'queue_plan',
  'undo_plan',
  'commit_plan',
  'restore_checkpoint',
  'recover_branch',
  'export_run',
  'import_run',
];

const WORKER_EVENTS = [
  'frame',
  'objective_changed',
  'pressure_changed',
  'review_ready',
  'checkpoint_written',
  'chapter_changed',
  'run_completed',
];

// Distinctive fragments of the eight save-version-1 fields from SPEC.md.
const SAVE_FIELDS = [
  'Run identifier and random state',
  'Branch nonce',
  'Chapter and objective progress',
  'Complete Field and Form state',
  'Active View and candidate slate',
  'Queued pressures',
  'Input configuration',
  'Anchor metadata',
];

// The locked top-level keys of the save payload.
const SAVE_KEYS = [
  'save_version',
  'run_id',
  'rng',
  'branch_nonce',
  'progress',
  'field',
  'view',
  'slate',
  'pressures',
  'input_config',
  'anchors',
];

// The closed error-code set for version 1.
const ERROR_CODES = [
  'protocol',
  'state',
  'validation',
  'impulse',
  'capacity',
  'not_found',
  'save_corrupt',
  'save_version',
  'import_invalid',
  'content_invalid',
  'worker_restart',
  'internal',
];

/** Lowercase machine id for a canonical closed-set name: "The Pull" -> "the_pull". */
function machineId(name) {
  return name.toLowerCase().replace(/[^a-z0-9]+/g, '_');
}

test('ARCHITECTURE.md exists and is substantial', () => {
  assert.ok(text.length > 0, 'docs/field-framework/ARCHITECTURE.md must exist');
  assert.ok(lines.length > 400, 'the architecture document must be a full interface freeze');
});

test('every required section heading is present exactly once', () => {
  for (const heading of REQUIRED_SECTIONS) {
    const count = lines.filter((line) => line.trim() === heading).length;
    assert.equal(count, 1, `expected exactly one "${heading}" heading, found ${count}`);
  }
});

test('FRAMEWORK.md is named as the upstream authority, carried unchanged', () => {
  assert.ok(text.includes('FRAMEWORK.md'), 'the upstream authority must be named');
  assert.ok(
    text.includes('unchanged'),
    'framework semantics must be carried across the interfaces unchanged',
  );
});

test('all seventeen public core types have their own subsection', () => {
  for (const name of CORE_TYPES) {
    const count = lines.filter((line) => line.trim() === `### ${name}`).length;
    assert.equal(count, 1, `expected exactly one "### ${name}" subsection, found ${count}`);
  }
});

test('the PlanCommand union locks its five variants, costs, and atomicity', () => {
  const body = sectionBody('### PlanCommand');
  for (const variant of PLAN_VARIANTS) {
    assert.ok(body.includes(`\`${variant}\``), `PlanCommand variant "${variant}" must appear`);
  }
  assert.ok(
    body.includes('costs 1 Impulse'),
    'the Impulse cost of every variant must be locked',
  );
  const queue = sectionBody('## Worker protocol') + body;
  assert.ok(
    queue.includes('all-or-nothing'),
    'the atomic all-or-nothing commit rule must be stated',
  );
});

test('all nine worker commands are locked in the protocol section', () => {
  const body = sectionBody('## Worker protocol');
  for (const command of WORKER_COMMANDS) {
    assert.ok(body.includes(`\`${command}\``), `worker command "${command}" must appear`);
  }
  assert.ok(
    body.includes('closed'),
    'the command set must be declared closed for version 1',
  );
});

test('all seven worker events are locked in the protocol section', () => {
  const body = sectionBody('## Worker protocol');
  for (const event of WORKER_EVENTS) {
    assert.ok(body.includes(`\`${event}\``), `worker event "${event}" must appear`);
  }
});

test('the protocol locks version field, correlation, transferables, and faults', () => {
  const body = sectionBody('## Worker protocol');
  assert.ok(body.includes('"v": 1') || body.includes('`v`'), 'a protocol version field must be locked');
  assert.ok(body.includes('correlation'), 'a correlation id rule must be locked');
  assert.ok(body.includes('transferable'), 'the transferable payload policy must be locked');
  assert.ok(body.includes('malformed'), 'malformed-message behavior must be locked');
  assert.ok(body.includes('restart'), 'the worker restart recovery contract must be locked');
});

test('the error envelope locks one shape and the closed code set', () => {
  const body = sectionBody('## Error envelope');
  assert.ok(body.includes('`code`'), 'the code field must be locked');
  assert.ok(body.includes('`message_key`'), 'the copy-catalog message key field must be locked');
  assert.ok(body.includes('`detail`'), 'the detail field must be locked');
  for (const code of ERROR_CODES) {
    assert.ok(body.includes(`\`${code}\``), `error code "${code}" must appear`);
  }
  assert.ok(body.includes('closed'), 'the code set must be declared closed for version 1');
});

test('the save section names all eight fields and the locked payload keys', () => {
  const body = sectionBody('## Save version 1 and persistence');
  for (const field of SAVE_FIELDS) {
    assert.ok(body.includes(field), `save field "${field}" must be named`);
  }
  for (const key of SAVE_KEYS) {
    assert.ok(body.includes(`\`${key}\``), `save payload key "${key}" must be locked`);
  }
  assert.ok(body.includes('`field_game`'), 'the IndexedDB database name must be locked');
  for (const store of ['runs', 'records', 'profile']) {
    assert.ok(body.includes(`\`${store}\``), `object store "${store}" must be locked`);
  }
  assert.ok(body.includes('autosave'), 'the autosave cadence must be locked');
  assert.ok(body.includes('corruption'), 'the corruption fallback must be locked');
  assert.ok(body.includes('migration'), 'the migration envelope must be locked');
  assert.ok(body.includes('Quick Retry'), 'exact Quick Retry must be bound');
  assert.ok(body.includes('Branch Recovery'), 'divergent Branch Recovery must be bound');
  assert.ok(
    body.includes('`prev_assembly_step`') && body.includes('the step it returned to'),
    'the post-restore normalization of the previous assembly step must be locked',
  );
  assert.ok(
    body.includes('`nonce_high`'),
    'the monotone branch-nonce record must be locked in the runs store',
  );
  assert.ok(
    body.includes('7,664,520') && body.includes('8 MiB'),
    'the worst-case payload size arithmetic and its fit under the cap must be stated',
  );
});

test('the deterministic random state names its algorithm and streams', () => {
  const body = sectionBody('## Determinism and serialization');
  assert.ok(body.includes('Philox2x64-10'), 'the generator must be named');
  assert.ok(body.includes('FNV-1a-64'), 'the split absorption hash must be named');
  assert.ok(body.includes('SplitMix64'), 'the split finalizer must be named');
  assert.ok(body.includes('sigma_V'), 'the evaluation root must be bound');
  for (const stream of ['"branch"', '"trajectory"', '"evaluation"'] ) {
    assert.ok(body.includes(stream), `the ${stream} stream partition must be locked`);
  }
});

test('the canonical serialization rules are stated', () => {
  const body = sectionBody('## Determinism and serialization');
  assert.ok(
    body.includes('ascending UTF-8 byte order'),
    'the object key ordering rule must be locked',
  );
  assert.ok(
    body.includes('No floating-point number'),
    'the integers-only number rule must be locked',
  );
  assert.ok(body.includes('SHA-256'), 'the payload hash must be locked');
  assert.ok(body.includes('byte-equivalent'), 'the byte-equivalence contract must be stated');
});

test('the numeric policy is pinned to fixed point with no system libm', () => {
  const body = sectionBody('## Determinism and serialization');
  assert.ok(body.includes('Q32.16'), 'the fixed-point format must be locked');
  assert.ok(body.includes('`i64`'), 'the storage type must be locked');
  assert.ok(body.includes('`fixed_mul`'), 'the multiplication rule must be locked');
  assert.ok(body.includes('`fixed_div`'), 'the division rule must be locked');
  assert.ok(body.includes('`isqrt`'), 'the integer square root must be locked');
  assert.ok(body.includes('system libm'), 'the no-system-libm rule must be stated');
});

test('the fixed-step accumulator is locked with catch-up and hidden-tab rules', () => {
  const body = sectionBody('## Determinism and serialization');
  assert.ok(body.includes('at most 6'), 'the maximum catch-up steps must be locked');
  assert.ok(body.includes('hidden'), 'the hidden-tab behavior must be locked');
  assert.ok(body.includes('acc30'), 'the exact accumulator arithmetic must be locked');
});

test('the depth resolution locks its thresholds, its cooldown, and its deferral', () => {
  // The wheel's accumulated threshold and the cooldown are what keep ordinary
  // trackpad noise from changing depth; the deferral is what keeps a deliberate
  // gesture from being lost on one of the many rendered frames that execute no
  // step at all. The deferral has two halves, in two modules, and both are
  // locked here because a rule split across a boundary is one rule.
  const body = sectionBody('## Determinism and serialization');
  assert.ok(body.includes('wheel_accum'), 'the wheel accumulator must be named');
  assert.ok(body.includes('480'), 'the trigger distance must be a number');
  assert.ok(body.includes('depth_cooldown = 15'), 'the cooldown must be a number of steps');
  assert.ok(
    body.includes(
      'A frame that executes no step resolves no depth change and consumes no press; ' +
        'the wheel delta it carries still accumulates',
    ),
    'the zero-step rule must be locked as a sentence, and must say what a stepless frame does move',
  );
  assert.ok(
    body.includes('half of all frames execute no step'),
    'the reason the deferral is not an edge case must be stated',
  );
  assert.ok(
    body.includes('one frame yields at most one depth change'),
    'the once-per-frame rule must stand beside the deferral',
  );

  // The shell's half: the re-offer, and why the core may not hold the press
  // instead of it.
  assert.ok(
    body.includes('offers an unconsumed press again') && body.includes('steps_run > 0'),
    'the shell re-offer contract must be locked, with the answer that ends it',
  );
  assert.ok(
    body.includes('one press is offered by one frame at a time'),
    'the re-offer must be bounded so two frames cannot resolve one press twice',
  );
  assert.ok(
    body.includes('The core holds nothing of a press between frames'),
    'the no-hidden-state rule must be stated as the byte-equivalence rule it is',
  );
  assert.ok(
    body.includes('`export_run` is reachable at any instant'),
    'the reason the core may hold nothing must be the record, not taste',
  );
  assert.ok(
    body.includes('The wheel needs no re-offer') && body.includes('payload state'),
    'why the wheel needs no re-offer must be stated: its accumulator is in the payload',
  );

  // The wheel capture the shell fills the field from, including the one
  // gesture the page keeps.
  assert.ok(
    body.includes('the page never scrolls'),
    'consuming the wheel over the play surface must be locked',
  );
  assert.ok(
    body.includes('`ctrl` or `meta`'),
    'the platform-zoom exception to that must be locked with it',
  );
  assert.ok(
    body.includes('16 px a line, 400 px a page'),
    'the delta-mode normalization must be numbers',
  );
});

test('the timing contract pins 30, 60, 8 ms p95, and 100 ms', () => {
  const body = sectionBody('## Runtime topology and timing');
  assert.ok(body.includes('30 fixed steps per second'), 'the simulation rate must be pinned');
  assert.ok(body.includes('60 rendered frames per second'), 'the render target must be pinned');
  assert.ok(body.includes('8 ms'), 'the step budget must be pinned');
  assert.ok(body.includes('p95'), 'the step budget percentile must be pinned');
  assert.ok(body.includes('100 ms'), 'the Still Mode analysis budget must be pinned');
});

test('the mode table carries every trigger the two ramps answer', () => {
  const body = sectionBody('## Runtime topology and timing');
  for (const trigger of [
    '`toggle_still` while `running`',
    '`toggle_still` while `ramp_out`',
    '`toggle_still` or committed exit while `still`',
    '`toggle_still` while `ramp_in`',
    'pause released from a run suspended in `still`',
    'the mode the pause interrupted is remembered; a standing ramp is discarded',
  ]) {
    assert.ok(body.includes(trigger), `the mode table must name ${trigger}`);
  }
});

test('Still Mode entry and exit lock the rules the mode table is silent on', () => {
  const body = sectionBody('## Still Mode entry, exit, and the inspection surface');
  assert.ok(
    body.includes('clamp(t_us - t_prev_us, 0, 250000)'),
    'the ramp clock must state the arithmetic it reads a frame with',
  );
  assert.ok(
    body.includes('a test must advance `t_us` to advance a ramp'),
    'the consequence for tests must be stated where the rule is',
  );
  assert.ok(
    body.includes('`ramp := RAMP_UNITS - ramp`'),
    'a reversal must state the mirror it takes',
  );
  assert.ok(
    body.includes('A pause remembers the mode it interrupted'),
    'what a pause does to a still run must be answered',
  );
  assert.ok(
    body.includes('a standing ramp is discarded rather than remembered'),
    'what a pause does to a standing ramp must be answered',
  );
  assert.ok(
    body.includes('the core runs no step there'),
    'a still run must be stated to run no step',
  );
  assert.ok(
    body.includes('Present exactly while the mode is `still`'),
    'the overlay section presence rule must be stated',
  );
  for (const locked of ['"applied": 0', '"slate_ordinal": 0']) {
    assert.ok(body.includes(locked), `an empty commit must lock ${locked}`);
  }
});

test('the Still Mode budget states the replay arithmetic', () => {
  const body = sectionBody('## Still Mode analysis budget');
  assert.ok(body.includes('8 baselines'), 'the per-View baseline count must appear');
  assert.ok(body.includes('32 replays'), 'the per-candidate replay total must appear');
  assert.ok(body.includes('160'), 'the worst-case slate replay count must appear');
  assert.ok(body.includes('9,600'), 'the worst-case replayed step count must appear');
  assert.ok(body.includes('100 ms'), 'the budget must be restated');
  assert.ok(body.includes('reference machine'), 'the reference machine assumption must be stated');
  assert.ok(body.includes('capped at 60 steps'), 'the window cap lever must be pinned');
  assert.ok(
    body.includes('default window is 45 steps'),
    'the default window lever must be pinned',
  );
  assert.ok(body.includes('74.9 ms'), 'the worst-case total must round upward, not down');
  assert.ok(body.includes('25.1 ms'), 'the margin must round downward, not up');
  assert.ok(body.includes('57.1 ms'), 'the default-window total must be pinned beside it');
  assert.ok(
    body.includes('prices upkeep') && body.includes('Trail queue'),
    'the re-measured block must say what the widened caps fixture stands',
  );
});

test('the budget model is per rule, and its shape covers a super-linear one', () => {
  // The model's shape is the amendment: a single element-count term hid a rule
  // whose cost is a product of three caps, and the figure was wrong by a
  // factor of thirty because of it. The table is what keeps that from
  // happening again, so the table's own rule is what this pins.
  const body = sectionBody('## Still Mode analysis budget');
  for (const rule of ['Route flow', 'Boundary leakage', 'Current delivery']) {
    assert.ok(body.includes(rule), `the per-rule table must carry a row for ${rule}`);
  }
  assert.ok(
    body.includes('Shape of its term'),
    'each row must state the shape of its own term, not only a number',
  );
  assert.ok(
    body.includes('quadratic in N') && body.includes('product of three caps'),
    'a super-linear rule must say so in its shape',
  );
  assert.ok(
    body.includes('A rule added without a row is'),
    'the model must state what an undeclared rule does to the figure',
  );
  assert.ok(
    body.includes('No approximate prefilter is permitted'),
    'the preparation must be locked as exact rather than approximate',
  );
  assert.ok(
    body.includes('conservative by construction'),
    'the one prefilter used must state why it is exact',
  );
  assert.ok(
    body.includes('1.84 s'),
    'the measured overrun the amendment answers must be recorded',
  );
});

test('the ownership map locks the six modules and the import rule', () => {
  const body = sectionBody('## Module ownership and workspace layout');
  for (const module of ['`core/`', '`worker/`', '`app/`', '`content/`', '`tools/`', '`app/src/render/`']) {
    assert.ok(body.includes(module), `module ${module} must appear in the ownership map`);
  }
  assert.ok(body.includes('may import'), 'the import rule must be stated');
  assert.ok(body.includes('Nothing imports `app/`'), 'the terminal-module rule must be stated');
});

test('the toolchain bindings are pinned', () => {
  const body = sectionBody('## Offline operation and build chain');
  assert.ok(body.includes('wasm-pack'), 'wasm-pack must be named');
  assert.ok(body.includes('0.15.0'), 'the wasm-pack version must be pinned');
  assert.ok(body.includes('--target web'), 'the wasm target must be pinned');
  assert.ok(body.includes('Vite'), 'the bundler must be named');
  assert.ok(body.includes('Vitest'), 'the TS test runner must be named');
  assert.ok(body.includes('cargo test'), 'the core test runner must be named');
  assert.ok(body.includes('no network'), 'offline operation must be locked');
});

test('no runtime code is added by this document', () => {
  assert.ok(
    text.includes('no runtime code'),
    'the document must state that it adds no runtime code',
  );
});

test('the trace records consumed control state and the replay input policy', () => {
  const body = sectionBody('## Public core types');
  assert.ok(body.includes('ControlState'), 'a per-step control record must be defined');
  assert.ok(body.includes('"ctl"'), 'TraceStep must carry the consumed control state');
  assert.ok(
    body.includes('recorded control schedule'),
    'window-start regeneration and framework replays must drive control from the record',
  );
});

test('the FrameState header is offset-locked to its declared size', () => {
  const body = sectionBody('### FrameState');
  assert.ok(body.includes('32 bytes'), 'the header size must be declared');
  assert.ok(body.includes('21–31'), 'the pad bytes must be explicit');
  assert.ok(body.includes('11 bytes'), 'the pad length must close the 28-versus-32 gap');
  assert.ok(
    body.includes('byte offset 32'),
    'the section-table start offset must be explicit',
  );
});

test('the slate record carries every intake outcome and the assembly moments', () => {
  const body = sectionBody('### CandidateSlate');
  assert.ok(body.includes('"discarded"'), 'an intake discard must have a slot of its own');
  assert.ok(
    body.includes('intake-empty'),
    'the locked reason an intake discard records must be carried verbatim',
  );
  assert.ok(
    body.includes('no-alternative-candidate'),
    'the locked deficiency reason must be carried verbatim',
  );
  assert.ok(
    body.includes('`omitted` counts candidates a full') && body.includes('never became'),
    'the two counts must be told apart',
  );
  // The closed reason set an unassigned privilege value carries, each a row of
  // FRAMEWORK.md's minimum-data table.
  for (const reason of [
    'window-too-short',
    'no-grain-pair',
    'few-members',
    'few-surround',
    'few-samples',
    'no-circulating-flow',
  ]) {
    assert.ok(body.includes(reason), `the unassigned reason ${reason} must be named`);
  }
  assert.ok(
    body.includes('two shapes and the reader admits no'),
    'a privilege value must be locked to a number with a range, or no numbers at all',
  );
  assert.ok(
    body.includes('at most the first 64 entries') && body.includes('the truncation is silent'),
    'the discard cap and its silent truncation must be declared rather than left to be found',
  );
  assert.ok(
    body.includes('"detail": null') && body.includes('overrun'),
    'the reserved detail slot must be declared now, so the record key set never moves later',
  );
  assert.ok(
    body.includes('on entry into Still Mode') && body.includes('after every committed change'),
    'the two assembly moments must be stated',
  );
  assert.ok(
    body.includes('advances it'),
    'the assembly ordinal must be stated to advance after the assembly that read it',
  );
});

test('the still surface locks how a slate is shown and walked', () => {
  const body = sectionBody('### The slate on the still surface');
  assert.ok(body.includes('presentation order'), 'the listing order must be stated');
  assert.ok(
    body.includes('never one bar for the four, never a total, never an\naverage')
      || body.includes('never one bar for the four, never a total, never an average'),
    'the no-collapsed-value rule must be restated for the ranked candidate list',
  );
  assert.ok(
    body.includes('drawn as its confidence range'),
    'a value must be shown as the range the comparison reads',
  );
  assert.ok(
    body.includes('drawn as an absence'),
    'an unassigned value must be shown as an absence rather than a zero',
  );
  assert.ok(body.includes('The tier groups the list'), 'the tier must group rather than rate');
  assert.ok(
    body.includes('Tolerance sensitivity is a flag'),
    'the sensitivity warning must be a flag rather than a figure',
  );
  assert.ok(
    body.includes('A completed ranking is heard'),
    'the ranking cue must be locked with the surface it belongs to',
  );
  assert.ok(
    body.includes('ArrowDown') && body.includes('ArrowUp'),
    'the candidate bindings must be named',
  );
  assert.ok(body.includes('Tab is deliberately unused'), 'the Tab decision must be stated');
  assert.ok(
    body.includes('replaces the proposal'),
    'walking must be stated to replace rather than stack a proposal',
  );
  assert.ok(body.includes('dash pattern'), 'the engines must be given a colour-free reading');
});

test('the coordinate representations lock the forecast depth and the cleared context', () => {
  const body = sectionBody('## Public core types');
  assert.ok(
    body.includes("The Field's declared\nForecast depth `a_F` is the controlled Form's `forecast_depth`") ||
      body.includes("declared Forecast depth `a_F` is the controlled Form's `forecast_depth`"),
    'a_F must be assigned to the controlled Form, the rule route_reach already stands under',
  );
  assert.ok(
    body.includes('no controlled Form declares 0'),
    'a Field with no controlled Form must declare depth 0',
  );
  assert.ok(
    body.includes('the members alone'),
    'the cleared context must be the members alone with their stored Charge and internal Routes',
  );
  assert.ok(
    body.includes('a declaration\nis not a participant') ||
      body.includes('a declaration is not a participant'),
    'the layers must stand as declarations whose parameters still run',
  );
  assert.ok(
    body.includes('a Node never holds less than nothing'),
    'the schedule correction must be held inside the Field\'s own bound',
  );
});

test('the coordinate surface is locked default-hidden and outside the tray', () => {
  const body = sectionBody('### The coordinate profile on the still surface (locked, Goal 17)');
  assert.ok(body.length > 0, 'the inspection surface must have its own subsection');
  assert.ok(
    body.includes('default-hidden and asked for'),
    'a profile must be optional by construction rather than by layout',
  );
  assert.ok(
    body.includes('its own region beside the tray'),
    'the tray must stay unfocusable, so the opt-in control stands outside it',
  );
  assert.ok(
    body.includes('renders no part of it'),
    'ordinary play must be stated to carry no part of the surface',
  );
  assert.ok(
    body.includes('never as a zero'),
    'an unassigned coordinate must be an absence rather than a zero',
  );
  assert.ok(
    body.includes('no row for a figure derived from several of them'),
    'the no-collapsed-value rule must be restated for the ten coordinates',
  );
  assert.ok(
    body.includes('a second request'),
    'the two replay-based coordinates must stay behind their own request',
  );
  assert.ok(
    body.includes('one short line from the copy catalog')
      || body.includes('one short line\nfrom the copy catalog'),
    'the Echo must be catalog copy rather than a report',
  );
  assert.ok(
    body.includes('shows **no\nnumber at all**') || body.includes('shows **no number at all**'),
    'the Echo must show no number',
  );
});

test('playback of a perturbation result is locked as motion, never a chart', () => {
  const body = sectionBody('### Playback of a perturbation result (locked, Goal 17)');
  assert.ok(body.length > 0, 'the playback subsection must exist');
  assert.ok(
    body.includes('never as a chart'),
    'the forbidden shape must be named',
  );
  assert.ok(
    body.includes('No frame section, and no widened record'),
    'the frame and the record must stand untouched',
  );
  assert.ok(
    body.includes('`set_playback(reading | null)`'),
    'the renderer interface addition must be named',
  );
  assert.ok(
    body.includes('No axes, no\n   strip, no number') ||
      body.includes('No axes, no strip, no number'),
    'the drawing must carry no axes, strip, or number',
  );
  assert.ok(
    body.includes('largest excess, smallest\n   sample number') ||
      body.includes('largest excess, smallest sample number'),
    'the played sample must be the one the Echo names',
  );
  assert.ok(
    body.includes('one window step per 1/30 s'),
    'the playback clock must be locked',
  );
  assert.ok(
    body.includes('Reduced motion holds the last window step'),
    'reduced motion must hold a still comparison rather than remove the reading',
  );
  assert.ok(
    body.includes('`running` sets null'),
    'ordinary play must carry no playback',
  );
  assert.ok(
    body.includes('Both engines draw it from the shared scene'),
    'both engines must carry the reading',
  );
});

test('the on-demand budget states what its figures count and what was measured', () => {
  const body = sectionBody('## Still Mode analysis budget');
  assert.ok(body.includes('On-demand jobs each get the same 100 ms ceiling'));
  assert.ok(
    body.includes('| `window-change` |'),
    'every on-demand job must carry a measured figure beside its prediction',
  );
  for (const kind of [
    'boundary-severance',
    'route-removal',
    'component-substitution',
    'resolution-change',
    'window-change',
    'surround-change',
    'delayed-replay',
    'full-turnover',
  ]) {
    assert.ok(body.includes(`\`${kind}\``), `the ${kind} job must be measured`);
  }
  assert.ok(
    body.includes('replay cost'),
    'the three predicted figures must say which term they are',
  );
  assert.ok(
    body.includes('on_demand_jobs'),
    'the harness the measured column comes from must be named',
  );
});

test('the slate record carries dominance, absent variants, and intake notes', () => {
  const body = sectionBody('### CandidateSlate');
  assert.ok(body.includes('"dominance"'), 'the dominance relation itself must be recorded');
  assert.ok(body.includes('"absent"'), 'variant absences must be recorded');
  assert.ok(
    body.includes('failed'),
    'each absent variant must carry its failed permitting condition',
  );
  assert.ok(body.includes('"standing_intake"'), 'the standing-inside intake note must exist');
  assert.ok(
    body.includes('standing-inside-vanished'),
    'the fallback reason must be carried verbatim',
  );
});

test('perturbation results lock their edge shapes', () => {
  const body = sectionBody('## Public core types');
  assert.ok(
    body.includes('"base_deviation"') && body.includes('"base_series"'),
    'delayed replay must carry its own base sample beside the shifted one',
  );
  assert.ok(
    body.includes('resolved default'),
    'defaulted parameters must be recorded resolved',
  );
  assert.ok(
    body.includes('five kinds that take no caller parameter'),
    'the null-parameter rule must name all five no-caller-parameter kinds',
  );
  for (const kind of [
    '`resolution-change`',
    '`window-change`',
    '`surround-change`',
  ]) {
    assert.ok(
      body.includes(kind),
      `the amended null-parameter rule must name ${kind}`,
    );
  }
  assert.ok(
    body.includes('"grains"') && body.includes('"windows"'),
    'the recomputed payload shapes must be locked per kind',
  );
  for (const reason of [
    '`no-member`',
    '`no-route-to-remove`',
    '`no-shared-value`',
    '`no-signature`',
    '`no-stored-charge`',
    '`no-upkeep`',
  ]) {
    assert.ok(
      body.includes(reason),
      `the session-lived reason ${reason} must be documented with the records that carry it`,
    );
  }
  assert.ok(
    body.includes('the save reader admits the slate') ||
      body.includes("the save reader admits the slate's own reason set alone"),
    'the session-lived reasons must be scoped out of the payload',
  );
});

test('the echo source branches on the committed change', () => {
  const protocol = sectionBody('## Worker protocol');
  assert.ok(
    protocol.includes("adopted candidate's evaluation record"),
    'a committed reshape must read from the adopted candidate, not a perturbation',
  );
  assert.ok(
    protocol.includes('**pre-commit** slate'),
    'the evaluation highlight must read the pre-commit slate — the post-commit record is clamped to nothing',
  );
  assert.ok(
    protocol.includes('no fall-through'),
    'the derivation must be one branch on the last committed entry with no fall-through',
  );
  assert.ok(
    protocol.includes('no highlight at all when no candidate evaluated'),
    'a reshape to an unevaluated View must leave no highlight rather than another record\'s',
  );
  assert.ok(
    protocol.includes('seat 1'),
    'connect and redirect must read the standing candidate',
  );
  const types = sectionBody('## Public core types');
  assert.ok(
    types.includes('`evaluation`'),
    'EchoHighlight must carry the evaluation-record kind',
  );
});

test('steering clamps to Q1.15 and mid-still restores land in running', () => {
  const types = sectionBody('### InputFrame');
  assert.ok(types.includes('32767'), 'the i16 steering clamp must be explicit');
  const protocol = sectionBody('## Worker protocol');
  assert.ok(
    protocol.includes('plan queue cleared'),
    'a restore issued mid-still must land in running with the queue cleared',
  );
});

test('the keyboard diagonal is the pair the magnitude rule admits', () => {
  // Two locked rules met one raw unit apart, and the document says which
  // governs: the magnitude cap is normative, and the diagonal is the largest
  // per-axis pair that fits under it.
  const body = sectionBody('## Runtime topology and timing');
  assert.ok(body.includes('±23169'), 'the diagonal components must be the admitted pair');
  assert.ok(
    !body.includes('±23170'),
    'the superseded diagonal must not stand beside the one that governs',
  );
  assert.ok(
    body.includes('2 × 23169²') && body.includes('2 × 23170²'),
    'both sides of the comparison that decides it must be stated',
  );
  assert.ok(
    body.includes('magnitude cap is normative'),
    'which of the two rules governs must be stated outright',
  );
  assert.ok(
    body.includes('round(2^15/√2)'),
    'the superseded value must be explained rather than merely dropped',
  );
});

test('the input configuration is locked immutable for the life of a run', () => {
  // The step function reads `pointer_speed`, which the trace does not carry.
  // What keeps a regeneration exact is stated here rather than by widening the
  // trace — the same shape of argument the standing inside stands on.
  const body = sectionBody('## Determinism and serialization');
  assert.ok(
    body.includes('`input_config` is immutable for the life of a run'),
    'the immutability of the input configuration must be locked',
  );
  assert.ok(
    body.includes('no command changes it'),
    'the reason it is immutable — nothing in the closed command set writes it — must be stated',
  );
  assert.ok(
    body.includes('configuration standing at replay time'),
    'which configuration a keyframe carry replays under must be locked',
  );
  assert.ok(
    body.includes('two permitted paths and no third'),
    'the goal that adds a settings surface must be left the same two paths',
  );
});

test('the four field model rules are locked with exact arithmetic', () => {
  for (const heading of [
    '### Step order',
    '### Route flow',
    '### Boundary leakage',
    '### Node overload',
    '### Current delivery',
  ]) {
    const count = lines.filter((line) => line.trim() === heading).length;
    assert.equal(count, 1, `expected exactly one "${heading}" subsection, found ${count}`);
  }

  const order = sectionBody('### Step order');
  for (const phase of ['Route phase', 'pressure phase', 'current phase']) {
    assert.ok(order.includes(phase), `the step order must place the ${phase}`);
  }

  const flow = sectionBody('### Route flow');
  assert.ok(
    flow.includes('single pass in ascending `route` order'),
    'the flow rule must lock its pass order',
  );
  assert.ok(flow.includes('`open`'), 'the open gate must be locked');
  assert.ok(flow.includes('min('), 'the flow amount must be a locked min');
  assert.ok(flow.includes('room'), 'the destination-cap term must be locked');
  assert.ok(flow.includes('`moved`'), 'the flow rule must name its ledger entries');

  const leak = sectionBody('### Boundary leakage');
  assert.ok(leak.includes('`leak_frac`'), 'the leakage parameter must be named');
  assert.ok(leak.includes('[0, 8192]'), 'the leakage parameter range must be locked');
  assert.ok(leak.includes('shell'), 'leakage must act on the standing inside’s shell');
  assert.ok(leak.includes('`fixed_mul`'), 'the leakage arithmetic must be locked');
  assert.ok(leak.includes('sink `leakage`'), 'the leakage sink must be named');
  assert.ok(
    leak.includes('exogenous term') && leak.includes('Boundary Sufficiency'),
    'the FRAMEWORK.md absorption reading must be stated',
  );
  // The rule reads the standing inside, which is a step input the trace does
  // not carry, so the document must lock what keeps a regeneration exact.
  assert.ok(
    leak.includes('immutable between View commits'),
    'the standing inside must be locked immutable between View commits',
  );
  assert.ok(
    leak.includes('ends the active window'),
    'a View commit must be locked to end the window it changes',
  );
  assert.ok(
    leak.includes('interface item 11'),
    'the cleared-context rebuild must be pinned to the declared View',
  );

  const overload = sectionBody('### Node overload');
  assert.ok(overload.includes('16384'), 'the excess decay constant must be a number');
  assert.ok(overload.includes('>> 1'), 'the inflow throttle must be an exact halving');
  assert.ok(overload.includes('sink `overload`'), 'the overload sink must be named');
  assert.ok(overload.includes('no latch'), 'recovery must be memoryless and stated');

  const delivery = sectionBody('### Current delivery');
  assert.ok(delivery.includes('`width`'), 'the recipient rule must use the locked width');
  assert.ok(delivery.includes('ascending NodeId'), 'the recipient order must be locked');
  assert.ok(delivery.includes('remainder'), 'the exact integer split must be locked');
  assert.ok(delivery.includes('source `current`'), 'the delivery source must be named');
  assert.ok(delivery.includes('`gain`'), 'delivery must consume the layer gain parameter');

  const model = sectionBody('## Field model rules');
  assert.ok(
    model.includes('ledger') && model.includes('exactly zero'),
    'the section must restate exact integer conservation over its ledger entries',
  );
  assert.ok(model.includes('2^46'), 'the overflow-safety bound must be restated');

  const boundary = sectionBody('### BoundaryState');
  assert.ok(boundary.includes('"leak_frac"'), 'BoundaryState must carry the parameter field');

  const scalars = sectionBody('### Scalar conventions and locked ranges');
  assert.ok(scalars.includes('Current strength'), 'the current-strength cap must join the table');
});

test('the type sections lock layer contiguity and the caps-table envelope', () => {
  // A depth change moves a Form one layer at a time, so a gap in the layer list
  // would put it on a layer that does not stand.
  const layers = sectionBody('### FieldLayer');
  assert.ok(
    layers.includes('contiguous'),
    'the layer list must be locked contiguous so every depth lands on a layer',
  );

  // Two of the View's components restate rows of the capacity table, and one
  // envelope has to govern both statements of the same cap.
  const view = sectionBody('### ViewDeclaration');
  assert.ok(
    view.includes('capacity table') && view.includes('`capacity` error'),
    'the caps table must be named as the envelope authority for restated ranges',
  );
});

test('the Pulse model rules are locked with exact arithmetic', () => {
  const count = lines.filter((line) => line.trim() === '### The Pulse').length;
  assert.equal(count, 1, `expected exactly one "### The Pulse" subsection, found ${count}`);

  const intro = sectionBody('## Field model rules');
  assert.ok(
    intro.includes('The five rules'),
    'the only-dynamics sentence must now count the Pulse',
  );

  const order = sectionBody('### Step order');
  assert.ok(order.includes('The Pulse phase'), 'the step order must place the Pulse phase');

  const pulse = sectionBody('### The Pulse');
  assert.ok(pulse.includes('2048'), 'the charging increment must be a number');
  assert.ok(pulse.includes('min(65536'), 'the charge clamp must be locked');
  assert.ok(
    pulse.includes('neither holds nor releases'),
    'the fumbled-hold reset must be locked',
  );
  assert.ok(
    pulse.includes('first executed step') && pulse.includes('records false'),
    'the release edge must carry the depth batch idiom',
  );
  assert.ok(pulse.includes('524288 + 184'), 'the radius formula must be locked');
  assert.ok(pulse.includes('Q8.8'), 'the frame radius encoding must be restated');
  assert.ok(pulse.includes('never stored'), 'the radius must stay derived state');
  assert.ok(pulse.includes('16384'), 'the gathering and displacement fraction must be a number');
  assert.ok(pulse.includes('`reserve`, or `module`'), 'the gathering source kinds must be closed');
  assert.ok(pulse.includes('headroom'), 'the destination cap must bound gathering');
  assert.ok(pulse.includes('`gathered`'), 'the gathering ledger transfer must be named');
  assert.ok(
    pulse.includes('no sink and no source'),
    'gathering must be a conserving transfer',
  );
  assert.ok(pulse.includes('ascending NodeId'), 'the per-source order must be locked');
  assert.ok(pulse.includes('one-way'), 'base port activation must be one-way');
  assert.ok(pulse.includes('moves no Charge'), 'displacement must stay outside the ledger');
  assert.ok(pulse.includes('extension point'), 'the Goal 18 extension point must be named');
  assert.ok(pulse.includes('same step'), 'outcome cues must land with their cue 1');
  assert.ok(pulse.includes('saturating'), 'cue payload saturation must be locked');
  assert.ok(
    pulse.includes('steering response is unchanged'),
    'focus must be written by the hold without a feel change',
  );
  // The hazard the goal that stages pressures has to see: a pressure list is a
  // step input the moment one stands, and a replay that does not reproduce a
  // displacement breaks the byte-equivalence contract.
  assert.ok(
    pulse.includes('membership boundary') && pulse.includes('derivable'),
    'the displacement must be a window boundary whose floor replays derive',
  );
  assert.ok(
    pulse.includes('byte-equivalence'),
    'the replay constraint must say what it protects',
  );
  // The one-step reach of a press, blessed in so many words: the floor lands
  // at the boundary that closes the pressing step, so the same step's
  // delivery reads the unpressed level and the press reaches every effect
  // from the next step — the reading-points idiom, stated at the write.
  assert.ok(
    pulse.includes('the boundary that closes the pressing') &&
      pulse.includes('reads the unpressed level') &&
      pulse.includes('from the next step'),
    'the press must reach the effects one step later, at the boundary',
  );
  // The opened reading is a derivation, not a carried field: the clause that
  // makes the Route-phase reads identical live and replayed, whatever stage
  // the list stands at when a window is replayed.
  const effects = sectionBody('### Pressure effects');
  assert.ok(
    effects.includes('derived, never') &&
      effects.includes('reading(max(start_step, step − 1))') &&
      effects.includes('not the list\'s stored fields'),
    'the opened reading must be derived, never carried',
  );
  // A frame at its cue cap must not drop the one cue every other cue of an
  // emission is read against.
  assert.ok(
    pulse.includes('overflow') && pulse.includes('cue 1'),
    'the cue overflow policy must resolve what happens to an emission cue',
  );
});

test('the six pressure effect rules are locked with stage scaling and draws', () => {
  const count = lines.filter((line) => line.trim() === '### Pressure effects').length;
  assert.equal(count, 1, `expected exactly one "### Pressure effects" subsection, found ${count}`);

  const effects = sectionBody('### Pressure effects');
  assert.ok(effects.includes('effective level'), 'one named level quantity must drive every effect');
  assert.ok(
    effects.includes('as the step opened'),
    'the phase-6 reading point must be locked against the phase-7 stage machine',
  );
  assert.ok(effects.includes('piecewise-constant'), 'the v1 level-curve semantics must be closed');
  assert.ok(
    effects.includes('never carries `primary`'),
    'the section-6 primary question must be settled',
  );
  assert.ok(effects.includes('drain + fixed_mul(drain'), 'the Drain scaling formula must be locked');
  assert.ok(effects.includes('noise(L) + level_eff'), 'the noise composition must be locked additive');
  assert.ok(effects.includes('one draw per layer'), 'the Noise draw count must be bounded');
  assert.ok(effects.includes('flow scale'), 'the drawn quantity must be named');
  assert.ok(
    effects.includes('confidence ranges'),
    'forecast distortion must be the sample-stream re-draw',
  );
  assert.ok(effects.includes('crisis entry'), 'Fracture must fire at crisis entry');
  assert.ok(effects.includes('smallest RouteId'), 'the Fracture fallback tie-break must be locked');
  assert.ok(effects.includes('cue 5'), 'the break must ride the existing cut cue');
  assert.ok(effects.includes('ends the active window'), 'a break must be a window boundary');
  assert.ok(
    effects.includes('fixed_mul(level_eff, 32768)'),
    'the Flood threshold cut must be locked',
  );
  assert.ok(effects.includes('heaviest'), 'the Flood holding rule must define heavily used');
  assert.ok(effects.includes('competing'), 'the Interference mechanic must name the competing path');
  assert.ok(
    effects.includes('`current` source'),
    'Interference redirection must stay inside the delivery source',
  );
  assert.ok(effects.includes('16777216'), 'the Drift move magnitude must be a number');
  assert.ok(effects.includes('stage entry'), 'Drift must move only at stage boundaries');
  assert.ok(
    effects.includes('preparation'),
    'Drift must preserve the budget preparation invariant',
  );

  const pressure = sectionBody('### PressureState');
  assert.ok(pressure.includes('"displaced"'), 'the displaced floor must join PressureState');
  assert.ok(pressure.includes('"bound"'), 'the Flood holding must join PressureState');

  const pulse = sectionBody('### The Pulse');
  assert.ok(
    pulse.includes('`displaced`'),
    'the displacement contract must route through the displaced floor',
  );

  const intro = sectionBody('## Field model rules');
  assert.ok(
    intro.includes('the one drawing rule'),
    'the no-randomness intro sentence must be amended for Noise',
  );

  const rng = sectionBody('## Determinism and serialization');
  assert.ok(
    rng.includes('start of the Route phase'),
    'the within-step draw order must name the actual drawing point',
  );

  const budget = sectionBody('## Still Mode analysis budget');
  assert.ok(
    budget.includes('Noise flow scale'),
    'the per-rule budget table must gain the Noise row',
  );
});

test('the four Form ability rules are locked with exact arithmetic', () => {
  const count = lines.filter((line) => line.trim() === '### Form abilities').length;
  assert.equal(count, 1, `expected exactly one "### Form abilities" subsection, found ${count}`);

  const body = sectionBody('### Form abilities');

  // Thread fast / Vault slow: one authored scale, core-side, frame-invariant.
  assert.ok(body.includes('`steer_scale`'), 'the steering scale parameter must be named');
  assert.ok(body.includes('[16384, 262144]'), 'the steering scale bound must be locked');
  assert.ok(
    body.includes('fixed_mul(pointer_speed, steer_scale)'),
    'the composition with the player setting must be one flooring point',
  );
  assert.ok(
    body.includes('never on the recorded frames'),
    'the InputFrame invariance must be stated',
  );

  // Knot: the reserved Node phase pays at last.
  assert.ok(
    body.includes('min(q(n), upkeep_rate(n))'),
    'the upkeep payment arithmetic must be locked',
  );
  assert.ok(body.includes('`upkeep` sink'), 'upkeep must enter the existing sink');
  assert.ok(
    body.includes('whole to `boundary`'),
    'the version-1 five-purpose attribution must be locked',
  );
  assert.ok(body.includes('z(n, t) = 1'), 'the could-not-pay indicator branch must activate');
  assert.ok(
    body.includes('Self-Support') && body.includes('Upkeep Mix'),
    'the two coordinates the rule makes real must be named with semantics untouched',
  );

  // Wake: Trail entries.
  assert.ok(body.includes('"pending"'), 'the pending queue must join FieldState');
  assert.ok(body.includes('capped at 64 entries'), 'the queue cap must be locked');
  assert.ok(body.includes('falls due'), 'the maturity rule must be locked');
  assert.ok(body.includes('source `wake`'), 'the delivery source must be named');
  assert.ok(
    body.includes('moves nothing'),
    'the deposit step must move no Charge so no terms straddle steps',
  );

  // Chorus: following, depth, separation, and the named seam.
  assert.ok(
    body.includes('32768) / reach_raw'),
    'the derived-control arithmetic must be locked',
  );
  assert.ok(body.includes('ascending Form id'), 'the following order must be locked');
  assert.ok(body.includes('follows the depth change'), 'linked depth must be locked');
  assert.ok(
    body.includes('accepts no delivery'),
    'separation must cost delivery, suppression not sink',
  );
  assert.ok(
    body.includes('named and now locked'),
    'the Goal 25 control-handoff seam must point at its locked rule',
  );

  // Figures discipline.
  assert.ok(
    body.includes('re-measured'),
    'the moved payload figure must be re-measured by the resuming implementer',
  );

  const form = sectionBody('### FormState');
  assert.ok(form.includes('"steer_scale"'), 'FormState must carry the scale');
  assert.ok(form.includes('"link"'), 'FormState must carry the link ability');
  assert.ok(form.includes('"trail"'), 'FormState must carry the trail ability');
  assert.ok(
    form.includes('"route_capacity"'),
    'FormState must carry what a Route it forms carries',
  );
  assert.ok(
    form.includes('non-null exactly on the Forms a `linked_forms` ability stands beside'),
    'the link carrier must say where it is non-null',
  );
  assert.ok(
    form.includes('non-null on every Form of a selection whose Form authors `trail`'),
    'the trail carrier must say where it is non-null',
  );
  assert.ok(
    form.includes('measured with both carriers at their widest'),
    'the payload figures must be stated as measured with the carriers widest',
  );

  // Item 2: the shipped upkeep figures, not the drafted ones.
  assert.ok(
    body.includes('quarter of a unit per step') && body.includes('sixteenth'),
    'the authored upkeep liabilities must be the shipped figures',
  );

  // Item 3: the measured payload growth.
  assert.ok(
    body.includes('0.18%'),
    'the payload growth must be the measured figure',
  );

  const aux = sectionBody('### Auxiliary shapes');
  assert.ok(aux.includes('"pending"'), 'FieldState must carry the pending Trail entries');

  const order = sectionBody('### Step order');
  assert.ok(
    order.includes('pays its authored upkeep'),
    'the reserved Node phase must pay at last',
  );

  const intro = sectionBody('## Field model rules');
  assert.ok(
    intro.includes('ability movers'),
    'the only-dynamics sentence must count the two ability movers',
  );

  const budget = sectionBody('## Still Mode analysis budget');
  assert.ok(
    budget.includes('Trail entries'),
    'the per-rule budget table must gain the Trail row',
  );
});

test('the control Handoff is locked for The Mesh', () => {
  const count = lines.filter((line) => line.trim() === '### The Handoff').length;
  assert.equal(count, 1, `expected exactly one "### The Handoff" subsection, found ${count}`);

  const body = sectionBody('### The Handoff');
  assert.ok(
    body.includes('`handoff`'),
    'the mechanism must be the locked inspect target on the existing command surface',
  );
  assert.ok(
    body.includes('Exactly one Form carries'),
    'single control must be locked as the version-1 invariant',
  );
  assert.ok(
    body.includes('frozen'),
    'the single-steer-vector justification must name the frozen frame',
  );
  assert.ok(body.includes('no Impulse'), 'the cost decision must be explicit');
  assert.ok(
    body.includes('ends the active window'),
    'a handoff must be a membership boundary',
  );
  assert.ok(
    body.includes('neutral control'),
    'the un-controlled motion rule must be locked',
  );
  assert.ok(
    body.includes('smallest Form id'),
    'a linked group without a controlled member needs its reference member',
  );
  assert.ok(body.includes('`controllable`'), 'the authorable seam must be named');
  assert.ok(body.includes('default true'), 'the controllable default must be locked');
  assert.ok(
    body.includes('never serialized'),
    'the controllable flag must stay content-derived with no payload cost',
  );
  assert.ok(
    body.includes('`not_found`') && body.includes('`validation`'),
    'the refusal reasons must be locked',
  );
  assert.ok(body.includes('snaps'), 'the camera transition must be locked');
  assert.ok(
    body.includes('`InputFrame` sequence'),
    'the handoff must sit inside the enumerated byte-equivalence inputs',
  );
  assert.ok(
    body.includes('already carries'),
    'the frame must be verified to move nothing but the existing bits',
  );
  // The budget note states the term that dominates rather than the one the
  // move itself walks: a Handoff ends the active window, and the keyframe
  // carry that ends it is the cost.
  assert.ok(
    body.includes('keyframe carry'),
    'the budget note must name the term that dominates a Handoff',
  );
  assert.ok(
    body.includes('150 recorded steps'),
    'the budget note must bound the carry by the retained span',
  );

  const aux = sectionBody('### Auxiliary shapes');
  assert.ok(
    aux.includes('"handoff"'),
    'InspectRequest must gain the handoff target',
  );

  const abilities = sectionBody('### Form abilities');
  assert.ok(
    abilities.includes('The Handoff'),
    'the named seam must now point at the locked rule',
  );

  const protocol = sectionBody('## Worker protocol');
  assert.ok(
    protocol.includes('control moved'),
    'the frame buffer-presence rule must cover a handoff inside still mode',
  );
});

test('the save-size arithmetic reflects the measured worst case', () => {
  const body = sectionBody('## Save version 1 and persistence');
  assert.ok(body.includes('49,092'), 'the measured dense TraceStep size must be stated');
  assert.ok(body.includes('7,664,520'), 'the measured worst-case payload must be stated');
  assert.ok(body.includes('8.6%'), 'the true headroom must replace the old estimate');
  // Both figures are pinned as equalities in the core, so the document and the
  // pins move together or not at all.
  assert.ok(body.includes('exact'), 'the figures must be stated as exact, not approximate');
});

test('the autosave suffix is derived from run state, not from the store', () => {
  const body = sectionBody('## Save version 1 and persistence');
  assert.ok(
    body.includes('floor(step / 900) mod 2'),
    'the derived autosave suffix must be locked as an expression over run state',
  );
  assert.ok(
    body.includes('pure function of the enumerated inputs'),
    'the reason the suffix is derived — byte-equivalence — must be stated',
  );
  assert.ok(
    body.includes('derived autosave key') && body.includes('rewritten in place'),
    'import must be bound to the same derived key and the in-place metadata rewrite',
  );
});

test('checkpoint metadata covers both record kinds and is not capped', () => {
  const types = sectionBody('## Public core types');
  assert.ok(
    types.includes('metadata of both record kinds'),
    'the anchors list must be locked as carrying Anchors and autosaves alike',
  );
  assert.ok(
    types.includes('One entry stands per `save_key`'),
    'the one-entry-per-key rule must be locked',
  );
  assert.ok(
    types.includes('The list is not capped'),
    'the 64 cap must be locked to records rather than to payload metadata',
  );
});

test('restart recovery names its interim source and scopes the source of truth', () => {
  const body = sectionBody('## Worker protocol');
  assert.ok(
    body.includes('`import_run` with') && body.includes('no record outlives the worker'),
    'the pre-persistence restart source must be locked as import_run over the held export',
  );
  assert.ok(
    body.includes('Once the goal that owns\npersistence lands'.replace(/\s+/g, ' ')) ||
      body.includes('Once the goal that owns persistence lands'),
    'the only-source-of-truth sentence must be scoped to the persistence goal',
  );
  assert.ok(
    body.includes('no notice claims a resumed one'),
    'a fresh run after a fault must be locked as surfacing no resumed notice',
  );
});

test('the corruption fallback selection is locked to the shell', () => {
  const body = sectionBody('## Save version 1 and persistence');
  assert.ok(
    body.includes('Selecting the fallback is the shell'),
    'fallback selection must be locked to the shell',
  );
  assert.ok(
    body.includes('`init_run`\'s success body has no'.replace(/\s+/g, ' ')) ||
      body.includes("`init_run`'s success body has no"),
    'the reason it cannot ride one command must be stated',
  );
  assert.ok(
    body.includes('carries\n`notice.anchor_recovered`'.replace(/\s+/g, ' ')) ||
      body.includes('carries `notice.anchor_recovered`'),
    'the notice must be locked to the save_corrupt message key',
  );
  assert.ok(
    body.includes('including when\nthe newest was absent'.replace(/\s+/g, ' ')) ||
      body.includes('including when the newest was absent'),
    'the absent-newer case must be locked into when the notice is surfaced',
  );
});

test('mentioned closed name sets are complete and never contradicted', () => {
  const sets = [
    ['chapters', data.canonical.chapters],
    ['pressures', data.canonical.pressures],
    ['forms', data.canonical.forms],
  ];
  for (const [label, names] of sets) {
    const mentioned = names.filter((name) => text.includes(name));
    if (mentioned.length === 0) continue;
    for (const name of names) {
      assert.ok(
        text.includes(name),
        `${label}: "${name}" is missing while others of its closed set are mentioned`,
      );
    }
    // The machine ids the document derives from the closed set must match the
    // canonical spelling exactly: "The Pull" yields the_pull, and so on.
    for (const name of names) {
      const id = machineId(name);
      assert.ok(
        text.includes(id),
        `${label}: machine id "${id}" must be derived from the canonical name`,
      );
    }
  }
});

test('the transactional queue locks what the union table left silent', () => {
  // Goal 14 could not implement the union without answering each of these, and
  // an answer left unwritten is one the next goal would have to guess.
  const body = sectionBody('### PlanCommand');
  assert.ok(
    body.includes('32 units per step'),
    'the capacity a formed Route opens at must be locked',
  );
  assert.ok(
    body.includes("does not take part in step T's Route flow pass"),
    'when a formed Route first carries must be locked',
  );
  assert.ok(
    body.includes('A redirect moves one end and nothing else'),
    'what a redirect keeps must be locked',
  );
  assert.ok(
    body.includes('Two entries sharing either key are both flagged'),
    'the conflict keys must be locked',
  );
  assert.ok(
    body.includes('they supersede, and the later one wins'),
    'what two entries replacing the standing View do instead of conflicting must be stated',
  );
  assert.ok(
    body.includes('Every completed objective of a chapter grants +3'),
    'which completions grant Impulse must be locked',
  );
  assert.ok(
    body.includes('Depth first, then the') && body.includes('then the cost'),
    'the order the queue refuses in must be locked, because both caps stand at six',
  );
});

test('the Impulse is carried in progress, and the reason is replay safety', () => {
  const types = sectionBody('### RunState');
  assert.ok(
    types.includes('Impulse lives in `progress`'),
    'where the Impulse is carried must be locked',
  );
  assert.ok(
    types.includes('the carry replays the step function and nothing else'),
    'the replay-safety argument for the relocation must be stated',
  );
  const field = sectionBody('## Public core types');
  assert.ok(
    field.includes('"complete": [string], "impulse": u8'),
    'the progress shape must carry the Impulse',
  );
});

test('a commit that applies a change ends the active window', () => {
  const rules = sectionBody('### Boundary leakage');
  assert.ok(
    rules.includes('Goal 14 took the first path'),
    'which of the two permitted paths was taken must be recorded',
  );
  assert.ok(
    rules.includes('for every commit that applies anything'),
    'the rule must be stated at the width it was implemented',
  );
  assert.ok(
    rules.includes('no window ever spans a change no step recorded'),
    'the property the rule buys must be stated',
  );
  const types = sectionBody('## Public core types');
  assert.ok(
    types.includes('`start_step` <= step') || types.includes('`start_step <= step`'),
    'the keyframe bound must be a floor with a ceiling rather than an equality',
  );
});

test('previews of queued changes ride the frame, and say so', () => {
  const body = sectionBody('### FrameState');
  assert.ok(
    body.includes('Previews of queued changes'),
    'the preview channel must be locked in the render snapshot',
  );
  assert.ok(
    body.includes('3 move queued, 4 proposed'),
    'the two added route statuses must be locked',
  );
  assert.ok(body.includes('queued member'), 'the proposed-membership flag must be locked');
  const protocol = sectionBody('## Worker protocol');
  assert.ok(
    protocol.includes('the plan queue') && protocol.includes('previews'),
    'when a frame carries its buffer must cover a queue that moved',
  );
});

test('the window clamp after an applying commit is stated, with its consequences', () => {
  // The architecture-level realization of FRAMEWORK.md's "a View commit ends
  // the active window": the retained span restarts at 0 and every windowed
  // procedure clamps to it. FRAMEWORK.md is untouched — its own unassigned and
  // short-window rules already say what a degenerate span produces.
  const rules = sectionBody('### Boundary leakage');
  assert.ok(
    rules.includes('w_eff = min(w, t0, retained_span)'),
    'the clamped effective window must be stated as an expression',
  );
  assert.ok(
    rules.includes('retained span restarts at 0'),
    'what the span is immediately after an applying commit must be stated',
  );
  assert.ok(
    rules.includes('FRAMEWORK.md is untouched'),
    'the amendment must say that the upstream document needs no change',
  );
  assert.ok(
    rules.includes('unassigned'),
    'the degenerate aftermath — unassigned windowed values — must be named',
  );
  assert.ok(
    rules.includes('Anchor written'),
    'the Anchor contract implication of the clamp must be stated',
  );
  const plan = sectionBody('### PlanCommand');
  assert.ok(
    plan.includes('under the clamped span'),
    'the slate a commit reassembles must be reconciled with the clamp',
  );
});

test('the queue defers its refusal codes to the per-precondition rules', () => {
  const body = sectionBody('### PlanCommand');
  assert.ok(
    !body.includes('An invalid entry is rejected with `validation` and is not queued'),
    'the blanket validation sentence must not stand beside the per-precondition rules',
  );
  assert.ok(
    body.includes('rejected with the envelope its own precondition names'),
    'queueing must defer to the codes the preconditions name',
  );
  assert.ok(
    body.includes('`form`'),
    'the missing controlled-Form reason must be enumerated',
  );
});

test('what queue-time validation rests on, and when supersession is right', () => {
  const body = sectionBody('### PlanCommand');
  assert.ok(
    body.includes('no rule moves base state in `still`'),
    'the condition queue-time validation rests on must be locked',
  );
  assert.ok(
    body.includes('must revalidate at commit'),
    'the obligation a later goal inherits must be stated',
  );
  assert.ok(
    body.includes('source 1') && body.includes('an undo does not take that record back'),
    'the reason two superseding View entries are not a conflict must be stated',
  );
});

test('the routes section says its identifiers are not unique', () => {
  const body = sectionBody('### FrameState');
  assert.ok(
    body.includes('not unique'),
    'a proposal sharing a standing Route identifier must be called out',
  );
  assert.ok(
    body.includes('status'),
    'consumers must be told to read the status beside the identifier',
  );
});

test('the formed-step rule names the guarantee it actually rests on', () => {
  const body = sectionBody('### PlanCommand');
  assert.ok(
    !body.includes('the core asserts it rather than guarding it'),
    'the rule must not claim an assertion the core does not make',
  );
  assert.ok(
    body.includes('before the Route pass runs'),
    'the step-counter ordering the rule rests on must be named',
  );
  assert.ok(
    body.includes('`formed_step` above the completed step'),
    'the validation that holds the field must be named',
  );
});

test('the keyframe floor names what it cannot check', () => {
  const body = sectionBody('## Public core types');
  assert.ok(
    body.includes('a trace shorter than the retained span cannot be told'),
    'the limit of the floor — a truncated trace imports silently — must be named',
  );
});

test('the campaign span locks its schedule validation and its event kinds', () => {
  const count = lines.filter((line) => line.trim() === '## The campaign span (locked, Goal 20)')
    .length;
  assert.equal(count, 1, `expected exactly one campaign-span heading, found ${count}`);
  const body = sectionBody('## The campaign span (locked, Goal 20)');

  // A schedule entry that names no target defers to the pressure's own locked
  // default, so the kind check has nothing to agree with and must not refuse it.
  assert.ok(
    body.includes('when it names a target at all'),
    'the schedule kind check must be stated as conditional on a target being named',
  );
  assert.ok(
    body.includes('kind `none` names nothing by declaration'),
    'the none-admitting form of the schedule validation must be stated',
  );
  assert.ok(
    body.includes('heaviest-throughput hold') && body.includes('largest-trailing-flow'),
    'the two locked defaults a none target defers to must be named',
  );

  // The authored-event kinds are a closed set, and every one of them must be
  // in the table a chapter author reads.
  for (const kind of [
    '`set_port_open`',
    '`set_layer_drain`',
    '`set_current_active`',
    '`set_route_cut`',
  ]) {
    assert.ok(body.includes(kind), `the authored-event kind ${kind} must stand in the table`);
  }
  assert.ok(
    body.includes('`next_route_id` does not move'),
    'a severed Route must be held to the committed cut\'s own identifier rule',
  );
  assert.ok(
    body.includes('at the tail of the') && body.includes('Cue 5'),
    'the cue a severed Route raises, and why it is not cue 5, must be stated',
  );
});
