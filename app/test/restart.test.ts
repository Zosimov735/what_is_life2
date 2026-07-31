/**
 * Worker restart recovery, driven through the shell's own client.
 *
 * The contract under test is the one `docs/field-framework/ARCHITECTURE.md`
 * locks: the shell detects the fault, terminates the worker, fails every
 * in-flight command locally with `worker_restart`, starts a fresh worker,
 * restores the run from the newest record it holds, surfaces the catalog
 * notice, and resumes. An unacknowledged command is never assumed applied.
 *
 * The workers here are the real entry loading the real module, and the fault is
 * a real one: the test ends the worker out from under the client and lets the
 * client discover it.
 */

import '@vitest/web-worker';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { afterEach, beforeAll, expect, inject, test, vi } from 'vitest';
import type { RunExported, RunOpened } from '../../worker/src/protocol';
import { openCore, RESUMED_NOTICE, type CoreClient } from '../src/shell/worker-client';

const WORKER_ENTRY = new URL('../../worker/src/entry.ts', import.meta.url);
const WORKSPACE = inject('workspace');

/** How long the locked contract lets a pending command go unanswered. */
const RESPONSE_LIMIT_MS = 2_000;

beforeAll(() => {
  vi.stubGlobal('fetch', async (target: URL | string) => {
    const { pathname } = new URL(String(target), 'http://localhost/');
    const bytes = await readFile(path.join(WORKSPACE, pathname));
    return new Response(bytes, { headers: { 'content-type': 'application/wasm' } });
  });
});

const clients: CoreClient[] = [];

afterEach(() => {
  for (const client of clients.splice(0)) client.close();
  vi.restoreAllMocks();
});

/** A client over real workers, with the workers it started recorded. */
function openRecorded(): { client: CoreClient; workers: Worker[] } {
  const workers: Worker[] = [];
  const client = openCore({
    form: 'thread',
    pump: false,
    spawn: () => {
      const worker = new Worker(WORKER_ENTRY, { type: 'module' });
      workers.push(worker);
      return worker;
    },
  });
  clients.push(client);
  return { client, workers };
}

/** The payload of the run as it stands now. */
async function payloadOf(client: CoreClient): Promise<Record<string, unknown>> {
  const answer = await client.command('export_run', {});
  expect(answer.ok).toBe(true);
  if (!answer.ok) throw new Error('an export was expected');
  return JSON.parse((answer.body as RunExported).text).payload as Record<string, unknown>;
}

function stepOf(payload: Record<string, unknown>): number {
  const field = payload.field as { now: { step: number } };
  return field.now.step;
}

test('a worker ended out from under the shell is replaced and the run resumes', async () => {
  vi.spyOn(console, 'error').mockImplementation(() => {});
  vi.spyOn(console, 'info').mockImplementation(() => {});
  const { client, workers } = openRecorded();

  const opening = await client.ready;
  expect(opening.ok).toBe(true);
  const runId = opening.ok ? (opening.body as RunOpened).run_id : '';
  expect(runId).toMatch(/^[0-9a-f]{16}$/);

  await client.step(40);
  expect(client.snapshot()?.header.step).toBe(40);

  // The shell holds a record to come back to, taken at the locked cadence.
  await vi.waitFor(() => expect(client.held()).not.toBeNull());
  const held = client.held();
  expect(held).not.toBeNull();
  if (!held) return;

  // The fault: the worker is ended, and the next command goes unanswered.
  workers[0].terminate();
  const lost = await client.command('export_run', {});
  expect(lost.ok).toBe(false);
  if (lost.ok) return;
  expect(lost.error.code).toBe('worker_restart');
  expect(lost.error.message_key).toBe(RESUMED_NOTICE);

  await vi.waitFor(() => expect(client.restarts()).toBe(1), { timeout: 20_000 });
  expect(workers).toHaveLength(2);
  expect(client.notices()).toContain(RESUMED_NOTICE);
  expect(client.recovering()).toBe(false);

  // The run came back: same key, same branch, the step the record was taken
  // at, and the exact random state that record carried.
  const resumed = await payloadOf(client);
  const recorded = JSON.parse(held.text).payload as Record<string, unknown>;
  expect(resumed.run_id).toBe(runId);
  expect(resumed.branch_nonce).toBe(recorded.branch_nonce);
  expect(resumed.rng).toEqual(recorded.rng);
  expect(stepOf(resumed)).toBe(stepOf(recorded));

  // And it runs on from there, on the fresh worker, with the frames numbered
  // from the start of the new session.
  const answer = await client.step(6);
  expect(answer).toHaveProperty('steps_run');
  expect(client.snapshot()?.header.step).toBe(stepOf(recorded) + 6);
  expect(stepOf(await payloadOf(client))).toBe(stepOf(recorded) + 6);
}, 60_000);

test('an in-flight command lost to a fault is never assumed applied', async () => {
  vi.spyOn(console, 'error').mockImplementation(() => {});
  vi.spyOn(console, 'info').mockImplementation(() => {});
  const { client, workers } = openRecorded();
  await client.ready;
  await client.step(10);
  await vi.waitFor(() => expect(client.held()).not.toBeNull());

  // Two commands are in flight when the worker goes; both fail locally rather
  // than being left to settle or assumed to have run.
  workers[0].terminate();
  const started = Date.now();
  const [first, second] = await Promise.all([
    client.command('export_run', {}),
    client.command('export_run', {}),
  ]);
  expect(Date.now() - started).toBeGreaterThanOrEqual(RESPONSE_LIMIT_MS - 100);
  for (const answer of [first, second]) {
    expect(answer.ok).toBe(false);
    if (!answer.ok) expect(answer.error.code).toBe('worker_restart');
  }
  await vi.waitFor(() => expect(client.restarts()).toBe(1), { timeout: 20_000 });
}, 60_000);

test('a worker reporting an error is replaced without waiting for a timeout', async () => {
  vi.spyOn(console, 'error').mockImplementation(() => {});
  vi.spyOn(console, 'info').mockImplementation(() => {});
  const { client, workers } = openRecorded();
  await client.ready;
  await client.step(3);
  await vi.waitFor(() => expect(client.held()).not.toBeNull());

  const reported = workers[0].onerror;
  expect(typeof reported).toBe('function');
  reported?.call(workers[0], new Event('error') as ErrorEvent);
  expect(client.recovering()).toBe(true);

  await vi.waitFor(() => expect(client.restarts()).toBe(1), { timeout: 20_000 });
  expect(workers).toHaveLength(2);
  const resumed = await payloadOf(client);
  expect(stepOf(resumed)).toBe(3);
}, 60_000);

test('a session with nothing held opens a fresh run and claims no resumed one', async () => {
  vi.spyOn(console, 'error').mockImplementation(() => {});
  vi.spyOn(console, 'info').mockImplementation(() => {});
  const { client, workers } = openRecorded();
  await client.ready;
  // No frame has been acknowledged, so no record has been taken.
  expect(client.held()).toBeNull();

  workers[0].onerror?.call(workers[0], new Event('error') as ErrorEvent);
  await vi.waitFor(() => expect(client.restarts()).toBe(1), { timeout: 20_000 });

  const opened = await payloadOf(client);
  expect(stepOf(opened)).toBe(0);
  expect(String(opened.run_id)).toMatch(/^[0-9a-f]{16}$/);
  // A fresh run is not a resumed one, and the shell says nothing that claims
  // otherwise: what a player who lost a run outright is shown belongs to the
  // goal that owns persistence and its recovery surface.
  expect(client.notices()).toEqual([]);
}, 60_000);

test('a held record the fresh worker refuses opens a run without claiming a resumed one', async () => {
  vi.spyOn(console, 'error').mockImplementation(() => {});
  vi.spyOn(console, 'info').mockImplementation(() => {});
  const { client, workers } = openRecorded();
  await client.ready;
  await client.step(4);
  await vi.waitFor(() => expect(client.held()).not.toBeNull());

  // The held record is corrupted where the shell holds it, so the fresh worker
  // refuses the import and the run cannot come back.
  const held = client.held();
  if (!held) return;
  held.text = held.text.replace('"payload_sha256":"', '"payload_sha256":"0');

  workers[0].onerror?.call(workers[0], new Event('error') as ErrorEvent);
  await vi.waitFor(() => expect(client.restarts()).toBe(1), { timeout: 20_000 });

  expect(client.notices()).toEqual([]);
  expect(stepOf(await payloadOf(client))).toBe(0);
}, 60_000);

test('the pump leaves nothing behind: every acknowledged frame clears its bookkeeping', async () => {
  vi.spyOn(console, 'info').mockImplementation(() => {});
  const { client } = openRecorded();
  await client.ready;
  await vi.waitFor(() => expect(client.inflight()).toBe(0));

  for (let sent = 0; sent < 40; sent += 1) {
    await client.step(1);
  }
  expect(client.snapshot()?.header.step).toBe(40);

  // The correlation held for the refusal path is cleared by the frame event
  // that acknowledges the frame, not only by a refusal or a timeout, so a pump
  // running at the render rate grows nothing without bound.
  await vi.waitFor(() => expect(client.inflight()).toBe(0));
}, 60_000);
