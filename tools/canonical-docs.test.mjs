// Post-implementation guard for the repository-local canonical documentation.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const REPO = fileURLToPath(new URL('..', import.meta.url));
const DOC_ROOT = path.join(REPO, 'docs', 'field-framework');

const CORE_DOCUMENTS = [
  'README.md',
  'PRODUCT_OUTLINE.md',
  'DECISIONS.md',
  'DEVELOPMENT_LOOP.md',
  'MILESTONES.md',
  'PLATFORM_AND_DELIVERY.md',
  'WORKING_RULES.md',
  'CODEBASE_STATE.md',
  'ATLAS_AND_MECHANICS.md',
  'FORM_AND_PLAY_MODEL.md',
  'NUMBER_2_MOCKUP_PSEUDOCODE.md',
  'LEGACY_CONTRACT_STATUS.md',
];

const MOCKUPS = [
  'active-commissioning.png',
  'archive.png',
  'atlas-texture.png',
  'atlas.png',
  'autonomous-renewal.png',
  'crowded-medium.png',
  'disturbance-grammar.png',
  'divergence-replay.png',
  'ensemble-overlay.png',
  'holdout-atmosphere-study.png',
  'holdout-validation.png',
  'intervention-bench.png',
  'life-cycle.png',
  'measurement-grain.png',
  'observe-bench.png',
  'open-field-equation-study.png',
  'open-field-refined.png',
  'physical-compartment-and-view.png',
  'select-form.png',
  'still-mode.png',
  'vestige-pressure-study.png',
];

function walk(directory) {
  const files = [];
  for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
    const absolute = path.join(directory, entry.name);
    if (entry.isDirectory()) files.push(...walk(absolute));
    else files.push(absolute);
  }
  return files;
}

test('canonical documents are repository-local and substantial', () => {
  for (const relative of CORE_DOCUMENTS) {
    const absolute = path.join(DOC_ROOT, relative);
    assert.ok(fs.existsSync(absolute), `${relative} must exist`);
    assert.ok(fs.statSync(absolute).size > 500, `${relative} must be substantial`);
  }

  const allText = walk(path.join(REPO, 'docs'))
    .filter((file) => file.endsWith('.md'))
    .map((file) => fs.readFileSync(file, 'utf8'))
    .join('\n');
  assert.ok(!allText.includes('/workspace/'), 'canonical docs must not contain scratch paths');
  assert.ok(!allText.includes('sandbox:'), 'canonical docs must not contain sandbox links');
});

test('all 21 Number 2 mock-ups are present and referenced', () => {
  const assetRoot = path.join(DOC_ROOT, 'assets', 'number-2');
  const actual = fs.readdirSync(assetRoot).filter((name) => name.endsWith('.png')).sort();
  assert.deepEqual(actual, [...MOCKUPS].sort());

  const screenSpec = fs.readFileSync(
    path.join(DOC_ROOT, 'NUMBER_2_MOCKUP_PSEUDOCODE.md'),
    'utf8',
  );
  const manifest = fs.readFileSync(path.join(assetRoot, 'README.md'), 'utf8');
  for (const mockup of MOCKUPS) {
    assert.ok(screenSpec.includes(`assets/number-2/${mockup}`), `${mockup} missing from screen spec`);
    assert.ok(manifest.includes(`(${mockup})`), `${mockup} missing from asset manifest`);
  }
});

test('relative Markdown links resolve', () => {
  const markdownFiles = walk(path.join(REPO, 'docs')).filter((file) => file.endsWith('.md'));
  const linkPattern = /!?\[[^\]]*\]\(([^)]+)\)/g;

  for (const file of markdownFiles) {
    const text = fs.readFileSync(file, 'utf8');
    for (const match of text.matchAll(linkPattern)) {
      const target = match[1].trim();
      if (!target || target.startsWith('#') || /^[a-z][a-z0-9+.-]*:/i.test(target)) continue;
      const withoutFragment = target.split('#', 1)[0];
      const absolute = path.resolve(path.dirname(file), withoutFragment);
      assert.ok(
        fs.existsSync(absolute),
        `${path.relative(REPO, file)} links to missing ${target}`,
      );
    }
  }
});

test('no-TDD rule and lost-contract status are explicit', () => {
  const workingRules = fs.readFileSync(path.join(DOC_ROOT, 'WORKING_RULES.md'), 'utf8');
  const rootInstructions = fs.readFileSync(path.join(REPO, 'AGENTS.md'), 'utf8');
  const recovery = fs.readFileSync(path.join(DOC_ROOT, 'LEGACY_CONTRACT_STATUS.md'), 'utf8');

  assert.match(workingRules, /no TDD, ever/i);
  assert.match(rootInstructions, /Test-driven development is prohibited/);
  for (const missing of ['LEXICON.md', 'SPEC.md', 'PLAN.md', 'FRAMEWORK.md', 'ARCHITECTURE.md']) {
    assert.ok(recovery.includes(`\`${missing}\``), `${missing} recovery status must be recorded`);
  }
});

test('continuous execution and static Mac delivery are canonical', () => {
  const loop = fs.readFileSync(path.join(DOC_ROOT, 'DEVELOPMENT_LOOP.md'), 'utf8');
  const milestones = fs.readFileSync(path.join(DOC_ROOT, 'MILESTONES.md'), 'utf8');
  const platform = fs.readFileSync(path.join(DOC_ROOT, 'PLATFORM_AND_DELIVERY.md'), 'utf8');

  assert.match(loop, /publish it, reconvene the panel/i);
  assert.match(loop, /Never write a test first/i);
  assert.match(loop, /GitHub `main` is canonical/i);
  assert.match(milestones, /^## Active$/m);
  assert.match(milestones, /M-001 — Reproducible remote-to-laptop build baseline/);
  assert.match(platform, /Tauri 2 as a thin static host/i);
  assert.match(platform, /requires no application server/i);
  assert.match(platform, /dedicated module Web Worker/i);
});
