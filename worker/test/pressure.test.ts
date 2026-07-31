/**
 * The staged pressures, across the worker boundary: the real module, the real
 * authored content, and the shared decoder reading section 6 out of the bytes
 * the core wrote.
 *
 * `the_pull` schedules Interference for step 900, and the authored table
 * carries it through signal, pressure, crisis, and resolution inside 1440
 * steps. It schedules a second entry far behind it — the Drift its final
 * challenge is built on — which stands queued through everything below and is
 * what the list holds once Interference has left it.
 * What these tests pin is the seam this goal turned real: the frame's
 * pressures section is core bytes rather than a development stand-in, the
 * `pressure_changed` event carries the full list after every change, and both
 * say the same thing the payload says.
 */

import '@vitest/web-worker';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { afterEach, beforeAll, expect, inject, test, vi } from 'vitest';
import {
  neutralFrame,
  PROTOCOL_VERSION,
  type EventEnvelope,
  type FrameEventBody,
  type InputFrame,
  type Payload,
  type PressureChanged,
  type PressureState,
  type ResponseEnvelope,
} from '../src/protocol';
import { decodeFrameState, FRAME_PRESSURE_IDS } from '../src/frame-state';

const WORKER_ENTRY = new URL('../src/entry.ts', import.meta.url);
const WORKSPACE = inject('workspace');

const KEY = 'aa5566bbccdd0011';

/** Where the authored schedule seats Interference, and its stage boundaries. */
const START = 900;
const CRISIS_AT = START + 90 + 240;
const SPENT_AT = CRISIS_AT + 120 + 90;

beforeAll(() => {
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
  command: (cmd: string, body: Payload) => Promise<ResponseEnvelope>;
  send: (frame: InputFrame) => Promise<FrameEventBody>;
  /** Every `pressure_changed` body seen so far, oldest first. */
  changes: () => PressureChanged[];
  close: () => void;
}

function openSession(): Session {
  const worker = new Worker(WORKER_ENTRY, { type: 'module' });
  opened.push(worker);

  const responses = new Map<number, (answer: ResponseEnvelope) => void>();
  const frames = new Map<number, (answer: FrameEventBody) => void>();
  const changes: PressureChanged[] = [];
  let nextId = 1;

  worker.addEventListener('message', (message) => {
    const data = (message as MessageEvent<ResponseEnvelope | EventEnvelope>).data;
    if ('re' in data) {
      const waiting = responses.get(data.re);
      responses.delete(data.re);
      waiting?.(data);
      return;
    }
    if (data.ev === 'pressure_changed') {
      changes.push(data.body as unknown as PressureChanged);
      return;
    }
    if (data.ev !== 'frame') return;
    const body = data.body as FrameEventBody;
    const waiting = frames.get(body.seq);
    frames.delete(body.seq);
    waiting?.(body);
  });

  return {
    command(cmd, body) {
      const id = nextId++;
      return new Promise((settle) => {
        responses.set(id, settle);
        worker.postMessage({ v: PROTOCOL_VERSION, id, cmd, body });
      });
    },
    send(frame) {
      const id = nextId++;
      return new Promise((settle, refuse) => {
        frames.set(frame.seq, settle);
        responses.set(id, (answer) => {
          frames.delete(frame.seq);
          refuse(new Error(`the frame was refused: ${JSON.stringify(answer)}`));
        });
        worker.postMessage({ v: PROTOCOL_VERSION, id, cmd: 'input_frame', body: frame });
      });
    },
    changes: () => changes,
    close() {
      worker.terminate();
    },
  };
}

/** A frame that advances an exact step count through the locked test hook. */
function stepped(seq: number, steps: number): InputFrame {
  return { ...neutralFrame(seq, seq * 16_667), advance_steps: steps };
}

/** Opens a run and advances it to an exact step, in frames the cap allows. */
async function openTo(session: Session, step: number): Promise<FrameEventBody> {
  const opening = await session.command('init_run', {
    mode: 'new',
    run_id: KEY,
    form: 'thread',
  });
  expect(opening.ok).toBe(true);
  let seq = 1;
  let at = 0;
  let last: FrameEventBody | null = null;
  while (at < step) {
    const batch = Math.min(step - at, 1_800);
    last = await session.send(stepped(seq, batch));
    at += batch;
    seq += 1;
  }
  expect(last).not.toBeNull();
  return last as FrameEventBody;
}

test('the frame carries the staged pressure as real core bytes the decoder reads', async () => {
  const session = openSession();
  const frame = await openTo(session, CRISIS_AT + 10);
  expect(frame.buffer).toBeDefined();
  const decoded = decodeFrameState(frame.buffer as ArrayBuffer);

  // Section 6, decoded by the shared decoder from the bytes the core wrote:
  // the active Interference pressure at its crisis stage, aimed at node 5, and
  // behind it the Drift the chapter queues for its final challenge. The list is
  // ascending in the closed set's own order, so Interference stands first.
  expect(decoded.pressures).toHaveLength(2);
  expect(decoded.pressures[1].queued).toBe(true);
  expect(FRAME_PRESSURE_IDS[decoded.pressures[1].ordinal]).toBe('drift');
  const pressure = decoded.pressures[0];
  expect(FRAME_PRESSURE_IDS[pressure.ordinal]).toBe('interference');
  expect(pressure.stage).toBe('crisis');
  expect(pressure.queued).toBe(false);
  expect(pressure.targetKind).toBe('node');
  expect(pressure.target).toBe(5);
  expect(pressure.level).toBe(58_982);
  session.close();
}, 30_000);

test('pressure_changed carries the full list at the seat, each stage, and the removal', async () => {
  const session = openSession();
  await openTo(session, SPENT_AT + 5);
  const changes = session.changes();

  // The seat at open, the admission, three stage turnovers, and the removal:
  // six changes, each carrying the whole list after it. The chapter's second
  // entry — the Drift its final challenge is built on — stands queued through
  // every one of them, so what the removal leaves is that entry rather than an
  // empty list.
  expect(changes.length).toBe(6);
  const shapes = changes.map((change) =>
    change.pressures.map((held: PressureState) => `${held.pressure}:${held.stage}:${held.queued}`),
  );
  expect(shapes).toEqual([
    ['interference:signal:true', 'drift:signal:true'],
    ['interference:signal:false', 'drift:signal:true'],
    ['interference:pressure:false', 'drift:signal:true'],
    ['interference:crisis:false', 'drift:signal:true'],
    ['interference:resolution:false', 'drift:signal:true'],
    ['drift:signal:true'],
  ]);

  // The locked invariants hold in every telling: at most two active, at most
  // one primary, and the seat the schedule asked for is the primary one.
  for (const change of changes) {
    expect(change.pressures.filter((held) => !held.queued).length).toBeLessThanOrEqual(2);
    expect(change.pressures.filter((held) => held.primary).length).toBeLessThanOrEqual(1);
  }
  expect(changes[1].pressures[0].primary).toBe(true);
  session.close();
}, 30_000);
