// Tests for the approved-lexicon and copy-catalog check.
//
// Run from the repository root:
//   node --test field_game/tools/
//
// Every fixture under tools/fixtures/ is a miniature field_game workspace with
// the same content/copy/catalog.json layout as the real one. The fixture tree is
// excluded from the default scan because it holds deliberately invalid input.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const CHECK = fileURLToPath(new URL('./lexicon-check.mjs', import.meta.url));
const FIXTURES = fileURLToPath(new URL('./fixtures/', import.meta.url));

/**
 * Runs the check and returns { status, report }.
 * A null root means the default scan of the real workspace.
 */
function run(fixtureName) {
  const args = ['--json'];
  if (fixtureName !== null) {
    args.push('--root', path.join(FIXTURES, fixtureName));
  }
  let status = 0;
  let stdout = '';
  try {
    stdout = execFileSync(process.execPath, [CHECK, ...args], {
      encoding: 'utf8',
      stdio: ['ignore', 'pipe', 'pipe'],
    });
  } catch (error) {
    status = error.status;
    stdout = error.stdout ?? '';
  }
  return { status, report: JSON.parse(stdout) };
}

/** Unique "code|file" pairs, sorted, so assertions stay independent of order. */
function found(report) {
  return [...new Set(report.violations.map((v) => `${v.code}|${v.file}`))].sort();
}

/** "code|file|line" triples, sorted, for assertions that pin the line too. */
function foundAtLines(report) {
  return report.violations.map((v) => `${v.code}|${v.file}|${v.line}`).sort();
}

test('accepts a workspace whose player-facing text all comes from the catalog', () => {
  const { status, report } = run('clean');
  assert.deepEqual(report.violations, [], 'clean fixture must report no violations');
  assert.equal(report.ok, true);
  assert.equal(status, 0, 'a clean workspace must exit 0');
  assert.ok(report.scannedFileCount >= 4, 'all fixture files must be scanned');
});

test('rejects uncatalogued player-facing text and representational identifiers', () => {
  const { status, report } = run('violations');
  assert.equal(status, 1, 'violations must exit 1');
  assert.equal(report.ok, false);
  assert.deepEqual(found(report), [
    'outside-workspace-import|src/outside.ts',
    'prohibited-term|src/representational-identifier.ts',
    'prohibited-term|src/scalar-value.ts',
    'uncatalogued-text|src/inline-attribute.tsx',
    'uncatalogued-text|src/inline-jsx.tsx',
    'unknown-copy-key|src/unknown-key.tsx',
  ]);
});

test('names the prohibited term and the line that carries it', () => {
  const { report } = run('violations');
  const hit = report.violations.find(
    (v) => v.code === 'prohibited-term' && v.file === 'src/representational-identifier.ts',
  );
  // lexicon-check: allow-term — asserting on the term the check must reject
  assert.equal(hit.term, 'creature');
  assert.equal(hit.line, 2);
  assert.equal(hit.category, 'representational');
});

test('rejects an identifier that collapses several readings into one value', () => {
  const { report } = run('violations');
  const hit = report.violations.find(
    (v) => v.code === 'prohibited-term' && v.file === 'src/scalar-value.ts',
  );
  assert.equal(hit.category, 'collapsed-value');
});

test('inflected forms of a prohibited stem are rejected as whole words', () => {
  const { report } = run('violations');
  const hit = report.violations.find(
    // lexicon-check: allow-term — asserting on the inflected term the check must reject
    (v) => v.code === 'prohibited-term' && v.term === 'surviving',
  );
  assert.ok(hit, 'the -ing inflection of a prohibited stem must be reported');
  assert.equal(hit.category, 'representational');
  assert.equal(hit.file, 'src/representational-identifier.ts');
});

test('escape markers suppress a violation on their own or the next line', () => {
  const { status, report } = run('allow-markers');
  assert.deepEqual(report.violations, [], 'marked lines must be accepted');
  assert.equal(status, 0);
});

test('a marker without a reason is reported and suppresses nothing', () => {
  const { status, report } = run('marker-reasons');
  assert.equal(status, 1);
  assert.deepEqual(foundAtLines(report), [
    'marker-missing-reason|src/unreasoned.tsx|4',
    'marker-missing-reason|src/unreasoned.tsx|8',
    'prohibited-term|src/unreasoned.tsx|5',
    'uncatalogued-text|src/unreasoned.tsx|9',
  ]);
});

test('a marker naming its reason still suppresses both kinds of violation', () => {
  const { report } = run('marker-reasons');
  assert.deepEqual(
    report.violations.filter((v) => v.file === 'src/reasoned.tsx'),
    [],
    'a reasoned marker must keep suppressing the line it covers',
  );
});

test('an apostrophe in markup text does not hide the text node', () => {
  const { status, report } = run('markup-text');
  assert.equal(status, 1);
  assert.ok(
    report.violations.some(
      (v) =>
        v.code === 'uncatalogued-text' && v.file === 'src/apostrophe.tsx' && v.line === 3,
    ),
    'text carrying an apostrophe must still be reported',
  );
});

test('literal text beside an expression is checked, and a quoted tag is not', () => {
  const { report } = run('markup-text');
  assert.deepEqual(foundAtLines(report), [
    'uncatalogued-text|src/apostrophe.tsx|3',
    'uncatalogued-text|src/mixed-children.tsx|7',
    'uncatalogued-text|src/mixed-children.tsx|8',
  ]);
});

test('a conditional branch is not a text node', () => {
  const { report } = run('markup-text');
  assert.deepEqual(
    report.violations.filter((v) => v.file === 'src/conditional.tsx'),
    [],
    'ternary, guard, generic, and fragment idioms hold no markup text',
  );
});

test('consecutive generic-typed members in a type body are not text nodes', () => {
  const { report } = run('markup-text');
  assert.deepEqual(
    report.violations.filter((v) => v.file === 'src/generic-members.tsx'),
    [],
    'a member ending at ">" beside one opening at "<" is a declaration, not a tag pair',
  );
});

test('inline text in a page is reported, and the document title is not', () => {
  const { status, report } = run('html-text');
  assert.equal(status, 1);
  // Sorted as text, so lines 10 and 12 precede line 9. Line 12 is the text
  // written after a passed-over element closes.
  assert.deepEqual(foundAtLines(report), [
    'uncatalogued-text|index.html|10',
    'uncatalogued-text|index.html|12',
    'uncatalogued-text|index.html|9',
  ]);
});

test('rejects a catalog that breaks the writing rules', () => {
  const { status, report } = run('bad-catalog');
  assert.equal(status, 1);
  const catalogFile = 'content/copy/catalog.json';
  assert.deepEqual(found(report), [
    `catalog-entry-invalid|${catalogFile}`,
    `catalog-key-format|${catalogFile}`,
    `instruction-too-long|${catalogFile}`,
    `label-spelling|${catalogFile}`,
  ]);
  // The key format is the strictest of the three that state it — LEXICON.md,
  // this check, and the core's own reader of authored keys — so a segment that
  // opens with a digit is refused here as it is there. The clean fixture's own
  // `objective.the_edge.hold_inside` is the accepting half of the same pin: a
  // per-chapter namespace passes, and a segment that opens with a digit does
  // not.
  const keys = report.violations
    .filter((one) => one.code === 'catalog-key-format')
    .map((one) => one.message);
  assert.ok(
    keys.some((message) => message.includes('objective.the_edge.2nd_hold')),
    'a key segment opening with a digit is a key-format violation',
  );
});

test('reports a missing catalog instead of blaming every key that uses it', () => {
  const { status, report } = run('no-catalog');
  assert.equal(status, 1);
  assert.deepEqual(found(report), ['catalog-missing|content/copy/catalog.json']);
});

test('the real field_game workspace and framework documents pass the check', () => {
  const { status, report } = run(null);
  assert.deepEqual(
    report.violations,
    [],
    'the committed workspace must satisfy its own rules',
  );
  assert.equal(status, 0);
  assert.ok(report.catalog.entryCount > 0, 'the real catalog must hold entries');
});

test('--help explains the invocation and exits 0', () => {
  const stdout = execFileSync(process.execPath, [CHECK, '--help'], { encoding: 'utf8' });
  assert.match(stdout, /--root/);
  assert.match(stdout, /--json/);
});

test('--root without a directory is bad usage, not a silent default scan', () => {
  for (const args of [['--root'], ['--root='], ['--root', '--json']]) {
    let status = 0;
    let stdout = '';
    try {
      stdout = execFileSync(process.execPath, [CHECK, ...args], {
        encoding: 'utf8',
        stdio: ['ignore', 'pipe', 'pipe'],
      });
    } catch (error) {
      status = error.status;
      stdout = error.stdout ?? '';
    }
    assert.equal(status, 2, `${args.join(' ')} must exit 2`);
    assert.equal(stdout, '', `${args.join(' ')} must not report a scan`);
  }
});
