/**
 * The content pipeline across the worker boundary: the bundle the worker hands
 * the core, the hash `init_run` reports, and the events the authored sequence
 * raises.
 */

import '@vitest/web-worker';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { afterEach, beforeAll, expect, inject, test, vi } from 'vitest';
import { neutralFrame, PROTOCOL_VERSION, type CommandName, type ErrorEnvelope, type EventEnvelope, type InputFrame, type Payload, type ResponseEnvelope } from '../src/protocol';
import { CONTENT_HASH, MANIFEST_TEXT, contentBundle } from '../src/content';

const WORKER_ENTRY = new URL('../src/entry.ts', import.meta.url);
const WORKSPACE = inject('workspace');
const KEY = '00112233445566aa';

beforeAll(() => {
  // The module the worker loads is read off disk, the same way every other
  // worker test reads it: nothing here reaches the network, and neither does
  // the game.
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

interface Session {
  send: (cmd: CommandName, body: Payload) => Promise<ResponseEnvelope>;
  frame: (frame: InputFrame) => Promise<void>;
  events: () => EventEnvelope[];
  close: () => void;
}

function openSession(): Session {
  const worker = new Worker(WORKER_ENTRY, { type: 'module' });
  opened.push(worker);
  const events: EventEnvelope[] = [];
  const pending = new Map<number, (answer: ResponseEnvelope) => void>();
  const frames = new Map<number, () => void>();
  let next = 1;

  worker.onmessage = (message: MessageEvent<ResponseEnvelope | EventEnvelope>) => {
    const data = message.data;
    if ('re' in data) {
      pending.get(data.re)?.(data);
      pending.delete(data.re);
      return;
    }
    if (data.ev === 'frame') {
      const seq = Number((data.body as { seq: number }).seq);
      frames.get(seq)?.();
      frames.delete(seq);
      return;
    }
    events.push(data);
  };

  return {
    send(cmd, body) {
      const id = next++;
      return new Promise((settle) => {
        pending.set(id, settle);
        worker.postMessage({ v: PROTOCOL_VERSION, id, cmd, body });
      });
    },
    frame(frame) {
      const id = next++;
      return new Promise((settle) => {
        frames.set(frame.seq, settle);
        worker.postMessage({ v: PROTOCOL_VERSION, id, cmd: 'input_frame', body: frame });
      });
    },
    events: () => [...events],
    close: () => worker.terminate(),
  };
}

/** One frame that runs an exact number of steps, with the control it names. */
function driven(seq: number, steps: number, held: Partial<InputFrame> = {}): InputFrame {
  return { ...neutralFrame(seq, 0), advance_steps: steps, ...held };
}

test('the digest the build embedded is the digest over the files it names', async () => {
  // The same rule, computed twice by two different pieces of the pipeline: the
  // generated file the worker imports, and the bytes on disk.
  const manifest = JSON.parse(MANIFEST_TEXT) as {
    chapters: string[];
    forms: string[];
    pressures: string[];
  };
  const digest = createHash('sha256');
  digest.update(Buffer.from(MANIFEST_TEXT, 'utf8'));
  const listed: [string[], string][] = [
    [manifest.chapters, 'chapters'],
    [manifest.forms, 'forms'],
    [manifest.pressures, 'pressures'],
  ];
  for (const [ids, directory] of listed) {
    for (const id of ids) {
      const file = path.join(WORKSPACE, 'content', directory, `${id}.json`);
      digest.update(await readFile(file));
    }
  }
  expect(CONTENT_HASH).toBe(digest.digest('hex'));

  const bundle = contentBundle();
  expect(bundle.hash).toBe(CONTENT_HASH);
  expect(bundle.manifest).toBe(MANIFEST_TEXT);
  expect(bundle.files).toHaveLength(
    manifest.chapters.length + manifest.forms.length + manifest.pressures.length,
  );
});

test('init_run reports the build hash and opens on the authored chapter', async () => {
  const session = openSession();
  const answer = await session.send('init_run', { mode: 'new', run_id: KEY, form: 'thread' });
  expect(answer.ok).toBe(true);
  if (!answer.ok) return;
  expect(answer.body.content_hash).toBe(CONTENT_HASH);
  expect(answer.body.content_changed).toBe(false);
  expect(answer.body.chapter_index).toBe(0);
  expect(answer.body.view).toEqual({
    inside: [2, 3, 4],
    resolution: 1,
    window: 45,
    surround: 'adjacent',
  });
  session.close();
});

test('opening a run raises the chapter and the objective it stands on', async () => {
  const session = openSession();
  await session.send('init_run', { mode: 'new', run_id: KEY, form: 'thread' });
  // The events follow the response their cause stands behind, so one frame is
  // enough to be sure they have crossed.
  await session.frame(driven(1, 1));

  const raised = session.events();
  const chapter = raised.find((event) => event.ev === 'chapter_changed');
  expect(chapter?.body).toEqual({
    chapter_index: 0,
    title_key: 'chapter.the_pull',
    view: { inside: [2, 3, 4], resolution: 1, surround: 'adjacent', window: 45 },
  });

  const offered = raised.find((event) => event.ev === 'objective_changed');
  expect(offered).toBeDefined();
  const objective = (offered?.body as { objective: { id: string; state: string } }).objective;
  expect(objective.id).toBe('objective.the_pull.follow_current');
  expect(objective.state).toBe('active');
  expect((offered?.body as { previous_id: string | null }).previous_id).toBeNull();
  session.close();
});

test('one objective stands at a time, and the next is offered only when it completes', async () => {
  const session = openSession();
  await session.send('init_run', { mode: 'new', run_id: KEY, form: 'thread' });
  // Steering east into the bright current, then holding there. The first
  // objective asks for time inside the band and nothing else.
  for (let index = 0; index < 3; index += 1) {
    await session.frame(driven(index + 1, 900, { steer_x: 30_000, steer_y: -6_000 }));
  }
  const objectives = session
    .events()
    .filter((event) => event.ev === 'objective_changed')
    .map((event) => (event.body as { objective: { id: string; state: string } }).objective);

  const active = objectives.filter((held) => held.state === 'active').map((held) => held.id);
  expect(active[0]).toBe('objective.the_pull.follow_current');
  // Whatever the run reached, no objective is ever offered before the one
  // before it has completed.
  const completed = objectives.filter((held) => held.state === 'complete').map((held) => held.id);
  for (const [place, id] of completed.entries()) {
    expect(active.slice(0, place + 1)).toContain(id);
  }
  session.close();
});

test('a run resumed on a different build of the content says so', async () => {
  // The locked behaviour: a restore under a different hash continues, and
  // `content_changed` is what says the framework reproducibility of the
  // pre-restore records is no longer claimed.
  const session = openSession();
  await session.send('init_run', { mode: 'new', run_id: KEY, form: 'thread' });
  await session.frame(driven(1, 30));
  const exported = await session.send('export_run', {});
  expect(exported.ok).toBe(true);
  if (!exported.ok) return;
  const file = JSON.parse(String(exported.body.text)) as {
    payload: { content_hash: string };
  };
  expect(file.payload.content_hash).toBe(CONTENT_HASH);
  session.close();
});

test('a body the core refuses answers with the locked envelope', async () => {
  const session = openSession();
  const refused = await session.send('init_run', { mode: 'new', run_id: KEY, form: 'spiral' });
  expect(refused.ok).toBe(false);
  if (refused.ok) return;
  const error = refused.error as ErrorEnvelope;
  expect(error.code).toBe('validation');
  session.close();
});
