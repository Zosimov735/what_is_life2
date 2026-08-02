/**
 * Clean-checkout preparation for the development server.
 *
 * Authored content is cheap and deterministic, so it is regenerated each time.
 * Development WASM is rebuilt only when it is absent or older than a Rust,
 * Cargo, or toolchain input. Production and validation still use the release
 * build explicitly.
 */

import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const REPO = fileURLToPath(new URL('..', import.meta.url));
const WASM_OUTPUTS = [
  path.join(REPO, 'worker', 'wasm-pkg', 'field_game_core.js'),
  path.join(REPO, 'worker', 'wasm-pkg', 'field_game_core_bg.wasm'),
];
const RUST_INPUTS = [
  path.join(REPO, 'core', 'Cargo.toml'),
  path.join(REPO, 'core', 'Cargo.lock'),
  path.join(REPO, 'package.json'),
  path.join(REPO, 'rust-toolchain.toml'),
  ...walk(path.join(REPO, 'core', 'src')),
];

function walk(directory) {
  return fs.readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const absolute = path.join(directory, entry.name);
    return entry.isDirectory() ? walk(absolute) : [absolute];
  });
}

function run(script) {
  const result = spawnSync('npm', ['run', script], {
    cwd: REPO,
    shell: process.platform === 'win32',
    stdio: 'inherit',
  });
  if (result.status !== 0) process.exit(result.status ?? 1);
}

function wasmIsFresh() {
  if (WASM_OUTPUTS.some((file) => !fs.existsSync(file))) return false;
  const newestInput = Math.max(...RUST_INPUTS.map((file) => fs.statSync(file).mtimeMs));
  const oldestOutput = Math.min(...WASM_OUTPUTS.map((file) => fs.statSync(file).mtimeMs));
  return oldestOutput >= newestInput;
}

run('build:content');
if (wasmIsFresh()) {
  console.log('Development WASM is current.');
} else {
  console.log('Development WASM is missing or stale; rebuilding it.');
  run('build:wasm:dev');
}
