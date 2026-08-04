/**
 * The production-build check: the locked build chain runs, and what it writes
 * is a self-contained static bundle holding the module.
 */

import { execFileSync } from 'node:child_process';
import { existsSync, readdirSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { expect, inject, test } from 'vitest';
import { DEV_RUN_EXPORT } from '../src/shell/dev-run';

const WORKSPACE = inject('workspace');
const BUNDLE = path.join(WORKSPACE, 'app', 'dist');

/**
 * The addresses allowed to appear anywhere in the bundle, and why none is a
 * request. The first are XML namespace names, which are identifiers rather
 * than addresses; the second is the link in the one error message the minified
 * build keeps; the third is the renderer library's console greeting, which the
 * renderer switches off at initialization and which the build therefore never
 * prints. Anything else fails, so a dependency that starts reaching for the
 * network has to be decided about rather than shipped — and so does a build
 * that quietly ships development sources, which carry a long list of their own
 * links.
 */
const ALLOWED_LINKS = [
  'http://www.w3.org/',
  'https://reactjs.org/docs/error-decoder.html',
  'http://www.pixijs.com/',
];

/** Every file under a directory. */
function filesUnder(root: string): string[] {
  const found: string[] = [];
  for (const entry of readdirSync(root, { withFileTypes: true, recursive: true })) {
    if (entry.isFile()) found.push(path.join(entry.parentPath, entry.name));
  }
  return found;
}

test(
  'the production build writes a self-contained bundle holding the module',
  () => {
    // The test runner marks its own environment, and a build that inherits
    // that mark resolves development sources. Dropping it is what makes this
    // the same build a shell runs.
    const { NODE_ENV: runnerEnvironment, ...environment } = process.env;
    expect(runnerEnvironment).not.toBe('production');
    execFileSync('npm', ['run', 'build'], { cwd: WORKSPACE, stdio: 'pipe', env: environment });

    const page = path.join(BUNDLE, 'index.html');
    expect(existsSync(page)).toBe(true);

    const files = filesUnder(BUNDLE);
    expect(files.filter((file) => file.endsWith('.wasm'))).toHaveLength(1);
    expect(files.some((file) => file.endsWith('.js'))).toBe(true);

    // Offline operation, read over every emitted file rather than the page
    // alone: nothing anywhere loads from another origin, and every remaining
    // mention of one is on the pinned list above.
    const loaded: string[] = [];
    const mentioned = new Set<string>();
    for (const file of files) {
      if (!/\.(?:css|html|js|json|map|txt)$/i.test(file)) continue;
      const text = readFileSync(file, 'utf8');
      for (const [reference] of text.matchAll(
        /(?:src|href)\s*=\s*["']?(?:https?:)?\/\/[^\s"'>]*|url\(\s*["']?(?:https?:)?\/\/[^\s"')]*|(?:import|fetch)\(\s*["'](?:https?:)?\/\/[^\s"']*/g,
      )) {
        loaded.push(`${path.basename(file)}: ${reference}`);
      }
      for (const [link] of text.matchAll(/https?:\/\/[^\s"'`)\\]*/g)) {
        if (!ALLOWED_LINKS.some((allowed) => link.startsWith(allowed))) {
          mentioned.add(`${path.basename(file)}: ${link}`);
        }
      }
    }
    expect(loaded).toEqual([]);
    expect([...mentioned]).toEqual([]);

    // The two development stand-ins — the scripted Field the renderer reads and
    // the run the core opens from — are each reached behind
    // `import.meta.env.DEV` by a dynamic import, so a production build drops
    // both branches and both modules. That is the mechanism; this is the proof.
    // Both halves of each are checked, because either could be left behind
    // without the other: the marker the branch reads, and anything the module
    // itself is made of.
    const shipped: string[] = [];
    for (const file of files) {
      if (file.endsWith('.wasm')) continue;
      const text = readFileSync(file, 'utf8');
      for (const trace of [
        'field_fixture',
        'fixtureFrames',
        'fixtureSnapshot',
        'fixtureSeek',
        'field_run',
        'DEV_RUN_EXPORT',
        // A run of the development run's own bytes, read from the module
        // rather than written here, so the two cannot part company: its
        // payload digest appears nowhere else.
        JSON.parse(DEV_RUN_EXPORT).payload_sha256 as string,
      ]) {
        if (text.includes(trace)) shipped.push(`${path.basename(file)}: ${trace}`);
      }
    }
    expect(shipped).toEqual([]);

    // And no emitted chunk is either module: a bundler that split one out
    // rather than dropping it leaves a file named after it.
    for (const stand of ['dev-frames', 'dev-run']) {
      expect(files.filter((file) => path.basename(file).startsWith(stand))).toEqual([]);
    }
  },
  600_000,
);
