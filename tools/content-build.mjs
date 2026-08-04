#!/usr/bin/env node
// Computes the content hash the build embeds, and writes it where the worker
// statically imports it from.
//
// `docs/field-framework/ARCHITECTURE.md` (Authored content) locks the rule:
//
//   content_hash = SHA-256 over the concatenation of the manifest bytes and
//   every listed file's bytes in manifest order
//
// and locks that the build computes it and embeds it, that `init_run` reports
// it, and that a restore under a different hash sets `content_changed`.
//
// Manifest order is the order the manifest's own three lists give, read
// `chapters`, then `forms`, then `pressures`. Both halves of the pipeline read
// that order from this one module: the build writes the digest here, and the
// worker hands the same bytes to the core, which recomputes the digest over
// exactly them and refuses the content when the two disagree. A generated file
// that has gone stale therefore cannot ship quietly.
//
// Dependency-free, like every other module under `tools/`: node built-ins only,
// so it runs on a bare install and before anything is installed.
//
// `--check` also runs the geometry lint: the authored numbers are raw Q32.16,
// which is the only form canonical JSON admits and a hard form to author in, so
// the lint reads them back out in whole units and reports the shapes that
// matter — where a Node stands, how wide a current's band is against the gaps
// between its own path points, and how far apart a Route's endpoints are. It is
// an authoring aid for the seven chapters ahead, and it fails on the two shapes
// that are defects rather than choices: a point off the plane, and a current
// whose path is gapped wider than its band can cover.
//
//   node field_game/tools/content-build.mjs            write the generated file
//   node field_game/tools/content-build.mjs --check     report without writing
//   node field_game/tools/content-build.mjs --print     print the digest only
//   node field_game/tools/content-build.mjs --geometry  the lint alone
//
// Exit code 0 clean, 1 stale or a geometry defect under `--check`, 2 bad usage
// or bad content.

import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const TOOLS_DIR = path.dirname(fileURLToPath(import.meta.url));
const WORKSPACE = path.resolve(TOOLS_DIR, '..');

/** Where the authored content lives, relative to the workspace root. */
export const CONTENT_DIR = 'content';

/** The manifest, relative to the workspace root. */
export const MANIFEST_RELATIVE = path.join(CONTENT_DIR, 'manifest.json');

/** The automation-contract manifest, hashed after the legacy content files. */
export const CONTRACT_MANIFEST_RELATIVE = path.join(CONTENT_DIR, 'contracts', 'manifest.json');

/** Player copy resolved by contract presentation keys. */
export const COPY_CATALOG_RELATIVE = path.join(CONTENT_DIR, 'copy', 'catalog.json');

/** Where the generated digest is written, relative to the workspace root. */
export const GENERATED_RELATIVE = path.join('worker', 'build', 'content.json');

/** The three manifest lists, in the order the concatenation reads them. */
const LISTS = [
  ['chapters', 'chapters'],
  ['forms', 'forms'],
  ['pressures', 'pressures'],
];

/**
 * Every authored file the manifest lists, in manifest order, each as its
 * workspace-relative path and its bytes exactly as they sit on disk.
 */
export function contentFiles(workspace = WORKSPACE) {
  const manifestPath = path.join(workspace, MANIFEST_RELATIVE);
  const manifestBytes = fs.readFileSync(manifestPath);
  let manifest;
  try {
    manifest = JSON.parse(manifestBytes.toString('utf8'));
  } catch (cause) {
    throw new Error(`the manifest does not parse: ${cause.message}`);
  }
  if (typeof manifest.content_version !== 'number') {
    throw new Error('the manifest needs a numeric content_version');
  }

  const files = [];
  for (const [key, directory] of LISTS) {
    const listed = manifest[key];
    if (!Array.isArray(listed)) {
      throw new Error(`the manifest needs a ${key} list`);
    }
    for (const id of listed) {
      if (typeof id !== 'string' || !/^[a-z][a-z0-9_]*$/.test(id)) {
        throw new Error(`${key} names an id that is not a machine id: ${String(id)}`);
      }
      const relative = path.join(CONTENT_DIR, directory, `${id}.json`);
      const absolute = path.join(workspace, relative);
      if (!fs.existsSync(absolute)) {
        throw new Error(`the manifest lists ${relative}, which is not there`);
      }
      files.push({ id, kind: directory, relative, bytes: fs.readFileSync(absolute) });
    }
  }
  const contractManifestPath = path.join(workspace, CONTRACT_MANIFEST_RELATIVE);
  const contractManifestBytes = fs.readFileSync(contractManifestPath);
  let contractManifest;
  try {
    contractManifest = JSON.parse(contractManifestBytes.toString('utf8'));
  } catch (cause) {
    throw new Error(`the contract manifest does not parse: ${cause.message}`);
  }
  if (typeof contractManifest.contract_version !== 'number'
      || !Array.isArray(contractManifest.contracts)) {
    throw new Error('the contract manifest needs contract_version and contracts');
  }
  const contractFiles = contractManifest.contracts.map((id) => {
    if (typeof id !== 'string' || !/^[a-z][a-z0-9_]*$/.test(id)) {
      throw new Error(`contracts names an id that is not a machine id: ${String(id)}`);
    }
    const relative = path.join(CONTENT_DIR, 'contracts', `${id}.json`);
    const absolute = path.join(workspace, relative);
    if (!fs.existsSync(absolute)) {
      throw new Error(`the contract manifest lists ${relative}, which is not there`);
    }
    return { id, kind: 'contracts', relative, bytes: fs.readFileSync(absolute) };
  });
  const copyCatalogPath = path.join(workspace, COPY_CATALOG_RELATIVE);
  if (!fs.existsSync(copyCatalogPath)) {
    throw new Error(`the copy catalog ${COPY_CATALOG_RELATIVE} is not there`);
  }
  const copyCatalogBytes = fs.readFileSync(copyCatalogPath);
  return {
    manifest,
    manifestBytes,
    manifestRelative: MANIFEST_RELATIVE,
    files,
    contractManifest,
    contractManifestBytes,
    contractFiles,
    copyCatalogBytes,
  };
}

/** The locked digest across legacy content, contracts, then player copy. */
export function contentHash(workspace = WORKSPACE) {
  const {
    manifestBytes,
    files,
    contractManifestBytes,
    contractFiles,
    copyCatalogBytes,
  } = contentFiles(workspace);
  const digest = crypto.createHash('sha256');
  digest.update(manifestBytes);
  for (const file of files) digest.update(file.bytes);
  digest.update(contractManifestBytes);
  for (const file of contractFiles) digest.update(file.bytes);
  digest.update(copyCatalogBytes);
  return digest.digest('hex');
}

/** One raw Q32.16 quantity, in whole units, rounded to one decimal. */
function units(raw) {
  return Math.round((raw / 65536) * 10) / 10;
}

/** The plane's own width in units: positions run over [0, 4096). */
export const PLANE_UNITS = 4096;

/** The narrowest band a current may author, in units. */
export const MIN_CURRENT_WIDTH_UNITS = 8;

/** The distance between two authored points, in units. */
function apart(first, second) {
  const dx = units(first.x) - units(second.x);
  const dy = units(first.y) - units(second.y);
  return Math.round(Math.hypot(dx, dy) * 10) / 10;
}

/**
 * Reads the authored geometry back in whole units.
 *
 * Returns `{ readings, defects }`: the readings are for a reader, the defects
 * are the two shapes that cannot be an authoring choice — a position outside
 * the plane, and a current whose own path points sit further apart than its
 * band is wide, which leaves a length of drawn band that delivers to nothing.
 */
export function geometry(workspace = WORKSPACE) {
  const { files } = contentFiles(workspace);
  const readings = [];
  const defects = [];
  for (const file of files.filter((held) => held.kind === 'chapters')) {
    const chapter = JSON.parse(file.bytes.toString('utf8'));
    readings.push(`${file.relative}`);

    const placed = new Map();
    for (const port of chapter.ports ?? []) {
      placed.set(port.node, port);
      const [x, y] = [units(port.pos.x), units(port.pos.y)];
      readings.push(
        `  node ${port.node} ${port.kind} layer ${port.layer} at (${x}, ${y}) ` +
          `threshold ${units(port.capacity)}`,
      );
      if (x < 0 || x >= PLANE_UNITS || y < 0 || y >= PLANE_UNITS) {
        defects.push(`node ${port.node} stands at (${x}, ${y}), outside the plane`);
      }
    }
    for (const form of chapter.forms ?? []) {
      placed.set(form.node, form);
      const [x, y] = [units(form.pos.x), units(form.pos.y)];
      readings.push(`  form ${form.id} on node ${form.node} layer ${form.layer} at (${x}, ${y})`);
      if (x < 0 || x >= PLANE_UNITS || y < 0 || y >= PLANE_UNITS) {
        defects.push(`form ${form.id} stands at (${x}, ${y}), outside the plane`);
      }
    }
    for (const route of chapter.routes ?? []) {
      const tail = placed.get(route.tail);
      const head = placed.get(route.head);
      const span = tail && head ? apart(tail.pos, head.pos) : null;
      readings.push(
        `  route ${route.route} ${route.tail} to ${route.head} ` +
          `carrying ${units(route.capacity)} a step, ${span ?? '?'} apart`,
      );
    }
    for (const current of chapter.currents ?? []) {
      const width = units(current.width);
      let widest = 0;
      for (let index = 1; index < current.path.length; index += 1) {
        widest = Math.max(widest, apart(current.path[index - 1], current.path[index]));
      }
      readings.push(
        `  current ${current.id} layer ${current.layer} band ${width} wide, ` +
          `${current.path.length} points, widest gap ${widest}, ` +
          `${units(current.strength)} a step${current.bright ? ', bright' : ''}`,
      );
      if (width < MIN_CURRENT_WIDTH_UNITS) {
        defects.push(`current ${current.id} authors a band ${width} wide, below the drawn minimum`);
      }
      // The recipient rule measures to the nearest path point, never to the
      // segment between two, so a gap wider than twice the band leaves drawn
      // band between the points that delivers to nothing standing in it.
      if (widest > width * 2) {
        defects.push(
          `current ${current.id} has a ${widest}-unit gap against a ${width}-unit band; ` +
            'densify the path or widen the band',
        );
      }
    }
    for (const objective of chapter.objectives ?? []) {
      const held = objective.condition ?? {};
      if (held.kind !== 'hold_position') continue;
      const [x, y] = [units(held.pos.x), units(held.pos.y)];
      readings.push(
        `  ${objective.id} holds within ${units(held.radius)} of (${x}, ${y}) on layer ${held.layer}`,
      );
    }
  }
  return { readings, defects };
}

/** The generated module's own bytes, canonical and newline-terminated. */
function generated(hash, version) {
  return `{"content_hash":"${hash}","content_version":${version}}\n`;
}

/** Writes the generated file, and the ignore rule that keeps it out of history. */
function write(workspace, text) {
  const target = path.join(workspace, GENERATED_RELATIVE);
  fs.mkdirSync(path.dirname(target), { recursive: true });
  // The directory ignores itself, exactly as the generated module directory
  // does: nothing here is authored, and none of it belongs in history.
  fs.writeFileSync(path.join(path.dirname(target), '.gitignore'), '*\n');
  fs.writeFileSync(target, text);
  return target;
}

function main(argv) {
  const options = { check: false, print: false, geometry: false };
  for (const argument of argv) {
    if (argument === '--check') options.check = true;
    else if (argument === '--print') options.print = true;
    else if (argument === '--geometry') options.geometry = true;
    else if (argument === '--help') {
      process.stdout.write(
        'Computes the content hash the build embeds.\n\n' +
          '  --check     report whether the generated file is current, and lint the geometry\n' +
          '  --print     print the digest only\n' +
          '  --geometry  the geometry lint alone\n',
      );
      return 0;
    } else {
      process.stderr.write(`content-build: unknown argument ${argument}\n`);
      return 2;
    }
  }

  let bundle;
  try {
    bundle = contentFiles(WORKSPACE);
  } catch (cause) {
    process.stderr.write(`content-build: ${cause.message}\n`);
    return 2;
  }
  const hash = contentHash(WORKSPACE);
  const text = generated(hash, bundle.manifest.content_version);

  if (options.print) {
    process.stdout.write(`${hash}\n`);
    return 0;
  }
  if (options.geometry || options.check) {
    const read = geometry(WORKSPACE);
    for (const line of read.readings) process.stdout.write(`${line}\n`);
    for (const defect of read.defects) process.stderr.write(`content-build: ${defect}\n`);
    if (read.defects.length > 0) return 1;
    if (options.geometry && !options.check) return 0;
  }
  const target = path.join(WORKSPACE, GENERATED_RELATIVE);
  if (options.check) {
    const current = fs.existsSync(target) && fs.readFileSync(target, 'utf8') === text;
    if (!current) {
      process.stderr.write(`content-build: ${GENERATED_RELATIVE} is not current\n`);
      return 1;
    }
    process.stdout.write(`content-build: ${GENERATED_RELATIVE} is current (${hash})\n`);
    return 0;
  }
  write(WORKSPACE, text);
  process.stdout.write(
    `content-build: ${bundle.files.length + 1} content files hashed to ${hash}\n`,
  );
  return 0;
}

if (import.meta.url === `file://${process.argv[1]}`) {
  process.exit(main(process.argv.slice(2)));
}
