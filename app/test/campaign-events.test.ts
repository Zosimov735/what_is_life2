/**
 * What the shell makes of the campaign's two events.
 *
 * The division the goal turns on is that no chapter rule lives in the shell:
 * the worker reports which chapter the run stands in and which ending it
 * reached, and the shell holds what it was told. So the worker here is a stub
 * that raises exactly those events, and what is under test is the one thing the
 * client derives from them — that a chapter closes when the run enters the one
 * after it, and not when a reopening or a restore reports a chapter again.
 */

import { afterEach, expect, test } from 'vitest';
import { PROTOCOL_VERSION } from '../../worker/src/protocol';
import { openCore, type CoreClient } from '../src/shell/worker-client';

/** A worker that answers every command and raises what a test tells it to. */
interface Stub {
  worker: Worker;
  raise: (ev: string, body: Record<string, unknown>) => void;
}

const clients: CoreClient[] = [];

afterEach(() => {
  for (const client of clients.splice(0)) client.close();
});

function stubWorker(): Stub {
  const held: { onmessage: ((message: MessageEvent<unknown>) => void) | null } = {
    onmessage: null,
  };
  const worker = {
    set onmessage(handler: (message: MessageEvent<unknown>) => void) {
      held.onmessage = handler;
    },
    set onerror(_handler: unknown) {},
    postMessage(envelope: { id: number }) {
      // Every command answers, so the client's own timers never fire.
      queueMicrotask(() => {
        held.onmessage?.({
          data: { v: PROTOCOL_VERSION, re: envelope.id, ok: true, body: {} },
        } as MessageEvent<unknown>);
      });
    },
    terminate() {},
  } as unknown as Worker;
  return {
    worker,
    raise(ev, body) {
      held.onmessage?.({
        data: { v: PROTOCOL_VERSION, ev, step: 0, body },
      } as unknown as MessageEvent<unknown>);
    },
  };
}

async function opened(): Promise<{ client: CoreClient; stub: Stub }> {
  const stub = stubWorker();
  const client = openCore({ form: 'thread', pump: false, spawn: () => stub.worker });
  clients.push(client);
  await client.ready;
  return { client, stub };
}

test('the chapter the run stands in is the one the worker last reported', async () => {
  const { client, stub } = await opened();
  expect(client.chapter()).toBeNull();
  stub.raise('chapter_changed', { chapter_index: 0, title_key: 'chapter.the_pull' });
  expect(client.chapter()?.title_key).toBe('chapter.the_pull');
  // The opening chapter closes nothing: a run has to have been somewhere.
  expect(client.review()).toBeNull();
});

test('a chapter closes when the run enters the one after it', async () => {
  const { client, stub } = await opened();
  stub.raise('chapter_changed', { chapter_index: 0, title_key: 'chapter.the_pull' });
  stub.raise('chapter_changed', { chapter_index: 1, title_key: 'chapter.the_edge' });
  expect(client.chapter()?.title_key).toBe('chapter.the_edge');
  expect(client.review()?.title_key).toBe('chapter.the_pull');
  expect(client.review()?.chapter_index).toBe(0);

  // Shown once: the surface lets go of it, and it does not come back.
  client.clearReview();
  expect(client.review()).toBeNull();
});

test('a reopened run and a restore that moved backwards close no chapter', async () => {
  const { client, stub } = await opened();
  stub.raise('chapter_changed', { chapter_index: 2, title_key: 'chapter.the_loop' });
  expect(client.review()).toBeNull();

  // A reopening reports the chapter the run stands in, again.
  stub.raise('chapter_changed', { chapter_index: 2, title_key: 'chapter.the_loop' });
  expect(client.review()).toBeNull();

  // And a restore can report an earlier one.
  stub.raise('chapter_changed', { chapter_index: 1, title_key: 'chapter.the_edge' });
  expect(client.chapter()?.chapter_index).toBe(1);
  expect(client.review()).toBeNull();
});

test('the ending arrives with the key the worker named', async () => {
  const { client, stub } = await opened();
  expect(client.ending()).toBeNull();
  stub.raise('run_completed', {
    ending_id: 'ending.the_quiet_edge',
    chapter_index: 7,
    continuation_unlocked: true,
  });
  expect(client.ending()?.ending_id).toBe('ending.the_quiet_edge');
  expect(client.ending()?.continuation_unlocked).toBe(true);
});
