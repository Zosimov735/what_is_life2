/** Reports whether this checkout is using the pinned build toolchain. */

import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const REPO = fileURLToPath(new URL('..', import.meta.url));
const packageJson = JSON.parse(fs.readFileSync(path.join(REPO, 'package.json'), 'utf8'));
const rustToolchain = fs.readFileSync(path.join(REPO, 'rust-toolchain.toml'), 'utf8');
const rustChannel = rustToolchain.match(/^channel = "([^"]+)"$/m)?.[1];
if (!rustChannel) throw new Error('rust-toolchain.toml must declare an exact channel');

const EXPECTED = {
  node: fs.readFileSync(path.join(REPO, '.nvmrc'), 'utf8').trim(),
  npm: packageJson.packageManager.replace(/^npm@/, ''),
  rust: rustChannel,
  wasmPack: packageJson.devDependencies['wasm-pack'],
  target: 'wasm32-unknown-unknown',
};

const failures = [];

function command(binary, args = []) {
  const result = spawnSync(binary, args, {
    cwd: REPO,
    encoding: 'utf8',
    shell: process.platform === 'win32',
  });
  return result.status === 0 ? result.stdout.trim() : null;
}

function expect(label, actual, wanted, hint) {
  const ok = actual !== null && actual.includes(wanted);
  console.log(`${ok ? 'ok' : '!!'} ${label}: ${actual ?? 'missing'} (expected ${wanted})`);
  if (!ok) failures.push(`${label}: ${hint}`);
}

expect('Node', process.versions.node, EXPECTED.node, `run nvm use ${EXPECTED.node}`);
expect('npm', command('npm', ['--version']), EXPECTED.npm, `use npm ${EXPECTED.npm}`);
expect(
  'Rust',
  command('rustc', ['--version']),
  `rustc ${EXPECTED.rust}`,
  'install rustup; rust-toolchain.toml will select the pinned toolchain',
);
expect('Cargo', command('cargo', ['--version']), 'cargo', 'install Rust through rustup');
expect(
  'wasm-pack',
  command('wasm-pack', ['--version']),
  `wasm-pack ${EXPECTED.wasmPack}`,
  'run npm ci to install the pinned local wasm-pack command',
);

const targets = command('rustup', ['target', 'list', '--installed']);
expect(
  'WASM target',
  targets,
  EXPECTED.target,
  `run rustup target add ${EXPECTED.target}`,
);

if (failures.length > 0) {
  console.error('\nToolchain repair:');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log('\nPinned development toolchain is ready.');
