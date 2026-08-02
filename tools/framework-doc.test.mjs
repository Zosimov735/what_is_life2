// Contract test for docs/field-framework/FRAMEWORK.md.
//
// FRAMEWORK.md is the sole game-framework authority: later goals implement its
// procedures verbatim. This test pins the document's required structure and the
// locked elements another goal depends on, so an edit cannot silently drop one.
// It is a contract test on the document, not a prose-quality test.
//
// Run from the repository root:
//   node --test "tools/*.test.mjs"

import { test as nodeTest } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import { fileURLToPath } from 'node:url';

const DOC_PATH = fileURLToPath(
  new URL('../docs/field-framework/FRAMEWORK.md', import.meta.url),
);

const text = fs.existsSync(DOC_PATH) ? fs.readFileSync(DOC_PATH, 'utf8') : '';
const lines = text.split('\n');

// The original source document was never committed and could not be recovered.
// Preserve this contract inventory as historical evidence, but do not fail the
// active suite until a deliberately reconstructed, versioned replacement exists.
const test = text.length > 0 ? nodeTest : nodeTest.skip;

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
  '## Core declaration',
  '## Candidate slate',
  '## Privilege profile',
  '## Coordinate profile',
  '## Required perturbations',
  '## Declared limitations',
  '## Game-facing interpretations',
];

const PRIVILEGE_VALUES = [
  'Scale Stability',
  'Shared Failure',
  'Cut Impact',
  'Boundary Sufficiency',
];

const COORDINATES = [
  'Swap Range',
  'Self-Support',
  'Throughput',
  'Upkeep Mix',
  'Reach',
  'Input Resolution',
  'Horizon',
  'Source Trace',
  'Instruction Separation',
  'Turnover Tolerance',
];

const PERTURBATIONS = [
  'Boundary severance',
  'Route removal',
  'Component substitution',
  'Resolution change',
  'Window change',
  'Surround change',
  'Delayed replay',
  'Full component turnover',
];

// Distinctive fragments of the six declared limitations from SPEC.md.
const LIMITATIONS = [
  'Candidate creation is an authored rule',
  'The four privilege components are an authored inventory',
  'Windows, surrounds, tolerances, and coordinate inventories are declared choices',
  'Multiple nondominated Views are expected',
  'comparison and counterfactual play, not a universal category',
  'Campaign thresholds are game rules',
];

// Fields every perturbation result must record, from SPEC.md.
const RESULT_FIELDS = ['View', 'random state', 'perturbation', 'tolerance', 'confidence range'];

// Construction vocabulary the document must never use. The framework document
// stays free of any binding to one construction of the game.
const CONSTRUCTION_WORDS = ['rust', 'react', 'worker', 'renderer', 'json', 'indexeddb'];

test('FRAMEWORK.md exists and is substantial', () => {
  assert.ok(text.length > 0, 'docs/field-framework/FRAMEWORK.md must exist');
  assert.ok(lines.length > 100, 'the framework document must be a full specification');
});

test('every required section heading is present exactly once', () => {
  for (const heading of REQUIRED_SECTIONS) {
    const count = lines.filter((line) => line.trim() === heading).length;
    assert.equal(count, 1, `expected exactly one "${heading}" heading, found ${count}`);
  }
});

test('the core declaration states the evaluation and the View tuple', () => {
  const body = sectionBody('## Core declaration');
  assert.match(body, /F\(S, V\)/, 'the evaluation notation F(S, V) must be declared');
  assert.ok(
    body.includes('inside, resolution, window, surround'),
    'the four View components must be named together in order',
  );
  assert.ok(
    body.includes('never assigns an intrinsic category to an isolated object'),
    'the no-intrinsic-category rule must be stated',
  );
});

test('the candidate slate locks size, sources, and ordering', () => {
  const body = sectionBody('## Candidate slate');
  assert.ok(body.includes('two to five'), 'the two-to-five slate size must be stated');
  const sources = [
    'Player-drawn boundaries',
    'Strong route clusters',
    'Existing physical boundaries',
    'Repeated shared responses',
    'finer',
    'coarser',
    'laterally shifted',
  ];
  for (const source of sources) {
    assert.ok(body.includes(source), `candidate source "${source}" must appear`);
  }
  assert.ok(
    body.includes('assembly order'),
    'a deterministic presentation ordering rule must be locked',
  );
  assert.ok(body.includes('fewer than five'), 'the cap of five must bound the fill');
  assert.ok(
    body.includes('max(2, ceil((w0 - 1) / 8))'),
    'the shared-response repeat threshold must be locked',
  );
});

test('the candidate slate locks rotation, freshness, and reachability', () => {
  const body = sectionBody('## Candidate slate');
  assert.ok(body.includes('assembly ordinal'), 'the assembly ordinal input must be declared');
  assert.ok(body.includes('rotated'), 'the rotated fill order must be locked');
  assert.ok(body.includes('fresh'), 'the fresh drawn-entry rule must be locked');
  assert.ok(
    body.includes('is at or after the'),
    'freshness must be at-or-after: a still-mode drag shares the step of the assembly that opened its session',
  );
  assert.ok(
    !body.includes('is later than the'),
    'the strictly-after freshness wording must not stand beside the at-or-after rule',
  );
  assert.ok(
    body.includes('shares the step of the assembly that opened its session'),
    'the reason the comparison is inclusive must be stated',
  );
  assert.ok(
    body.includes('permanently starved'),
    'a reachability guarantee for every source must be stated',
  );
  assert.ok(
    body.includes('at most one seat') && body.includes('counted omitted at most once'),
    'a candidate offered by more than one source must be bounded to one seat and one omission',
  );
  assert.ok(
    body.includes("every later source's provenance still reaches the record"),
    'the merged-provenance reading must stand beside the reachability sentence',
  );
});

test('a self-loop is one Node\'s own Route, and every consequence is stated', () => {
  const shared = sectionBody('## Core declaration');
  assert.ok(
    shared.includes("that Node's own Route, never an edge between two Nodes"),
    'the self-loop principle must be stated in Shared procedures',
  );
  const slate = sectionBody('## Candidate slate');
  assert.ok(
    slate.includes('enters no pair weight, reaches no median, forms no strong edge'),
    'source 2 must state that a self-loop enters no pair weight',
  );
  assert.ok(
    slate.includes('constitutes no cluster'),
    'source 2 must state that a self-loop constitutes no cluster',
  );
  assert.ok(
    slate.includes('exactly once — never twice for its two endpoints'),
    'A_in must state that a member self-loop contributes once, naming the double-count trap',
  );
  const cut = sectionBody('### Cut Impact');
  assert.ok(
    cut.includes('or a Route from a Node to itself'),
    "Cut Impact's cycle parenthetical must stand unchanged",
  );
  assert.ok(
    cut.includes('never in the crossing set X'),
    'Cut Impact must note that a self-loop is never severed',
  );
  assert.ok(
    cut.includes('cancels in the numerator') && cut.includes('inflates only the denominator'),
    'the arithmetic consequence of an unsevered self-loop must be stated',
  );
  assert.ok(
    cut.includes('no procedure special-cases it'),
    'the truthful-low reading must be declared, so the ranking goal does not special-case it',
  );
});

test('the core declaration locks tolerance, windows, and range construction', () => {
  const body = sectionBody('## Core declaration');
  assert.ok(body.includes('default is 1/8'), 'the tolerance default must be locked');
  assert.ok(
    body.includes('effective window'),
    'a rule for a trajectory shorter than the window must be locked',
  );
  assert.ok(
    body.includes('the full-window part is one of the three'),
    'the window-split range must contain its value by construction',
  );
});

test('each privilege value has a named definition normalized to [0, 1]', () => {
  for (const name of PRIVILEGE_VALUES) {
    const body = sectionBody(`### ${name}`);
    assert.ok(body.length > 0, `a "### ${name}" subsection must exist`);
    assert.ok(body.includes('[0, 1]'), `${name} must state its [0, 1] range`);
  }
});

test('the privilege profile states the no-collapse rule and nondominance', () => {
  const body = sectionBody('## Privilege profile');
  assert.ok(
    body.includes('never summed, averaged, weighted, or otherwise combined'),
    'the no-collapse rule must be stated in those words',
  );
  assert.ok(body.includes('dominates'), 'the dominance rule must be defined');
  assert.ok(body.includes('nondominated'), 'the nondominated outcome must be defined');
  assert.ok(body.includes('confidence range'), 'comparisons must account for confidence ranges');
  assert.ok(body.includes('unassigned'), 'unassigned-value comparison behavior must be defined');
  assert.ok(
    body.includes('low_A - high_B > tau'),
    'the range-separation comparison inequality must be locked',
  );
  assert.ok(
    body.includes('min(1, 4 * tau)'),
    'the probe strength must be locked in the profile',
  );
  assert.ok(
    body.includes(
      "the same S, standing View, assembly ordinal, previous assembly's evaluation step",
    ),
    'the reproducibility input list must cover every assembly input',
  );
});

test('all ten coordinates are defined and none joins a universal value', () => {
  for (const name of COORDINATES) {
    const body = sectionBody(`### ${name}`);
    assert.ok(body.length > 0, `a "### ${name}" subsection must exist`);
  }
  const body = sectionBody('## Coordinate profile');
  assert.ok(
    body.includes('No coordinate is combined'),
    'the coordinate no-combination rule must be stated',
  );
});

test('all eight perturbation kinds are defined', () => {
  for (const name of PERTURBATIONS) {
    const body = sectionBody(`### ${name}`);
    assert.ok(body.length > 0, `a "### ${name}" subsection must exist`);
  }
});

test('perturbation results record the five required fields', () => {
  const body = sectionBody('## Required perturbations');
  const start = body.indexOf('**Records.**');
  const end = body.indexOf('**Reproducibility.**');
  assert.ok(start !== -1 && end > start, 'a Records paragraph must exist');
  const records = body.slice(start, end);
  for (const field of RESULT_FIELDS) {
    assert.ok(records.includes(field), `the Records paragraph must name the ${field}`);
  }
});

test('stochastic procedures state their sample count and stream partition', () => {
  const body = sectionBody('## Privilege profile') + sectionBody('## Required perturbations');
  assert.ok(body.includes('8 samples'), 'the locked sample count must be stated');
  assert.ok(body.includes('split('), 'the random-state partition must be stated');
});

test('all six declared limitations are stated', () => {
  const body = sectionBody('## Declared limitations');
  for (const fragment of LIMITATIONS) {
    assert.ok(body.includes(fragment), `limitation "${fragment}" must be stated`);
  }
});

test('the game-facing section maps the required play concepts', () => {
  const body = sectionBody('## Game-facing interpretations');
  for (const concept of ['Still Mode', 'Anchor', 'Echo', 'Campaign thresholds']) {
    assert.ok(body.includes(concept), `the ${concept} interpretation must appear`);
  }
});

test('the document never names a construction technology', () => {
  for (const word of CONSTRUCTION_WORDS) {
    const pattern = new RegExp(`\\b${word}\\b`, 'i');
    assert.ok(!pattern.test(text), `"${word}" must not appear in the framework document`);
  }
});
