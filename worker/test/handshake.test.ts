/**
 * The worker test: the real entry, running in a worker context, loading the
 * built module and answering the version handshake.
 *
 * Every answer here comes back through the WASM core, so a passing run is
 * proof the module loaded.
 */

import '@vitest/web-worker';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { afterEach, beforeAll, expect, inject, test, vi } from 'vitest';
import { PROTOCOL_VERSION, type CommandEnvelope, type ResponseEnvelope } from '../src/protocol';

const WORKER_ENTRY = new URL('../src/entry.ts', import.meta.url);
const WORKSPACE = inject('workspace');

beforeAll(() => {
  // No static server runs here, so one stands in: the module's own loader asks
  // for a workspace path and gets those bytes from disk, with the type a
  // server would send.
  vi.stubGlobal('fetch', async (target: URL | string) => {
    const { pathname } = new URL(String(target), 'http://localhost/');
    const bytes = await readFile(path.join(WORKSPACE, pathname));
    return new Response(bytes, { headers: { 'content-type': 'application/wasm' } });
  });
});

const opened: Worker[] = [];

afterEach(() => {
  for (const worker of opened.splice(0)) worker.terminate();
});

function openWorker(): Worker {
  const worker = new Worker(WORKER_ENTRY, { type: 'module' });
  opened.push(worker);
  return worker;
}

function answerOf(worker: Worker, command: unknown): Promise<ResponseEnvelope> {
  return new Promise((settle) => {
    worker.addEventListener(
      'message',
      (message) => settle((message as MessageEvent<ResponseEnvelope>).data),
      { once: true },
    );
    worker.postMessage(command);
  });
}

test('the worker loads the module and opens a run on the run key it is given', async () => {
  const worker = openWorker();
  const handshake: CommandEnvelope = {
    v: PROTOCOL_VERSION,
    id: 1,
    cmd: 'init_run',
    body: { mode: 'new', run_id: '00112233445566aa', form: 'thread' },
  };

  const response = await answerOf(worker, handshake);

  expect(response.v).toBe(PROTOCOL_VERSION);
  expect(response.re).toBe(1);
  expect(response.ok).toBe(true);
  if (!response.ok) return;
  expect(response.body.protocol).toBe(PROTOCOL_VERSION);
  expect(response.body.save_version).toBe(1);
  expect(response.body.run_id).toBe('00112233445566aa');
  expect(response.body.step).toBe(0);
  expect(response.body.branch_nonce).toBe(0);
  expect(response.body.content_changed).toBe(false);
  expect(response.body.content_hash).toMatch(/^[0-9a-f]{64}$/);
});

test('a run key the core does not read is refused', async () => {
  const worker = openWorker();
  const response = await answerOf(worker, {
    v: PROTOCOL_VERSION,
    id: 1,
    cmd: 'init_run',
    body: { mode: 'new', run_id: 'not-a-run-key', form: 'thread' },
  } satisfies CommandEnvelope);

  expect(response.ok).toBe(false);
  if (response.ok) return;
  expect(response.error.code).toBe('validation');
  expect(response.error.detail).toEqual({ field: 'run_id' });
});

test('a command outside its lifecycle states is answered by the core', async () => {
  const worker = openWorker();
  const response = await answerOf(worker, {
    v: PROTOCOL_VERSION,
    id: 1,
    cmd: 'queue_plan',
    body: {},
  } satisfies CommandEnvelope);

  expect(response.ok).toBe(false);
  if (response.ok) return;
  expect(response.error.code).toBe('state');
  expect(response.error.detail).toEqual({ actual: 'idle', expected: ['still'] });
});

test('a message in another protocol version is refused, with its id echoed', async () => {
  const worker = openWorker();
  const response = await answerOf(worker, { v: 2, id: 7, cmd: 'init_run', body: {} });

  expect(response.v).toBe(PROTOCOL_VERSION);
  expect(response.re).toBe(7);
  expect(response.ok).toBe(false);
  if (response.ok) return;
  expect(response.error.code).toBe('protocol');
});

test('a message that is not an envelope is refused against correlation zero', async () => {
  const worker = openWorker();
  const response = await answerOf(worker, 'init_run');

  expect(response.re).toBe(0);
  expect(response.ok).toBe(false);
  if (response.ok) return;
  expect(response.error.code).toBe('protocol');
});

test('a repeated correlation id is refused', async () => {
  const worker = openWorker();
  await answerOf(worker, {
    v: PROTOCOL_VERSION,
    id: 1,
    cmd: 'init_run',
    body: {},
  } satisfies CommandEnvelope);
  const repeated = await answerOf(worker, {
    v: PROTOCOL_VERSION,
    id: 1,
    cmd: 'init_run',
    body: {},
  } satisfies CommandEnvelope);

  expect(repeated.ok).toBe(false);
  if (repeated.ok) return;
  expect(repeated.error.code).toBe('protocol');
  expect(repeated.error.detail).toEqual({ reason: 'correlation' });
});
