/**
 * The deterministic runtime, across the worker boundary.
 *
 * Two worker sessions given the same run key and the same frames must export
 * byte-equivalent state — the contract
 * `docs/field-framework/ARCHITECTURE.md` states, read here the way the shell
 * reads it: through the protocol envelopes, with no direct reach into the
 * module. The accumulator under test is the worker's own, so these runs are
 * driven by timestamps rather than by a step count.
 *
 * Sessions are opened one after another rather than side by side, because the
 * worker under test is the real entry and each session must be the only one
 * loading the module while it starts.
 */

import '@vitest/web-worker';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { afterEach, beforeAll, expect, inject, test, vi } from 'vitest';
import {
  neutralFrame,
  PROTOCOL_VERSION,
  type ErrorEnvelope,
  type EventEnvelope,
  type FrameEventBody,
  type InputFrame,
  type Payload,
  type ResponseEnvelope,
} from '../src/protocol';
import { CONTENT_HASH } from '../src/content';

const WORKER_ENTRY = new URL('../src/entry.ts', import.meta.url);
const WORKSPACE = inject('workspace');

/** One rendered frame at the 60-frames-per-second target, in microseconds. */
const FRAME_US = 16_667;

const KEY = '0123456789abcdef';
const OTHER_KEY = 'fedcba9876543210';

beforeAll(() => {
  // No static server runs here, so one stands in: the module's own loader asks
  // for a workspace path and gets those bytes from disk.
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

/** What a frame comes back as: the event that acknowledges it, or a refusal. */
type FrameAnswer = { frame: FrameEventBody } | { refused: ErrorEnvelope };

/** A worker session, correlated the way the shell correlates one. */
interface Session {
  command: (cmd: string, body: Payload) => Promise<ResponseEnvelope>;
  send: (frame: InputFrame) => Promise<FrameAnswer>;
  close: () => void;
}

function openSession(): Session {
  const worker = new Worker(WORKER_ENTRY, { type: 'module' });
  opened.push(worker);

  const responses = new Map<number, (answer: ResponseEnvelope) => void>();
  const frames = new Map<number, (answer: FrameAnswer) => void>();
  let nextId = 1;

  worker.addEventListener('message', (message) => {
    const data = (message as MessageEvent<ResponseEnvelope | EventEnvelope>).data;
    if ('re' in data) {
      const waiting = responses.get(data.re);
      responses.delete(data.re);
      waiting?.(data);
      return;
    }
    if (data.ev !== 'frame') return;
    const body = data.body as FrameEventBody;
    const waiting = frames.get(body.seq);
    frames.delete(body.seq);
    waiting?.({ frame: body });
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
      return new Promise((settle) => {
        frames.set(frame.seq, settle);
        // A refused frame answers with a response instead of a frame event.
        responses.set(id, (answer) => {
          frames.delete(frame.seq);
          settle({ refused: answer.ok ? ({} as ErrorEnvelope) : answer.error });
        });
        worker.postMessage({ v: PROTOCOL_VERSION, id, cmd: 'input_frame', body: frame });
      });
    },
    close() {
      worker.terminate();
    },
  };
}

async function openRun(session: Session, key: string): Promise<void> {
  const opening = await session.command('init_run', { mode: 'new', run_id: key, form: 'thread' });
  expect(opening.ok).toBe(true);
}

async function exported(session: Session): Promise<Payload> {
  const answer = await session.command('export_run', {});
  expect(answer.ok).toBe(true);
  return answer.ok ? answer.body : {};
}

/** The frames of a run played at the render rate. */
function rendered(count: number): InputFrame[] {
  return Array.from({ length: count }, (_, index) => neutralFrame(index + 1, (index + 1) * FRAME_US));
}

/** Opens one session, plays a script, exports, and ends the session. */
async function playThrough(
  key: string,
  script: InputFrame[],
): Promise<{ body: Payload; ran: number[] }> {
  const session = openSession();
  await openRun(session, key);
  const ran: number[] = [];
  for (const frame of script) {
    const answer = await session.send(frame);
    expect(answer).toHaveProperty('frame');
    if ('frame' in answer) ran.push(answer.frame.steps_run);
  }
  const body = await exported(session);
  session.close();
  return { body, ran };
}

test('two sessions under the same run key and the same frames export the same bytes', async () => {
  const script = rendered(12);
  const first = await playThrough(KEY, script);
  const second = await playThrough(KEY, script);

  expect(first.body.text).toBe(second.body.text);
  expect(first.body.sha256).toBe(second.body.sha256);
  expect(first.body.filename_hint).toBe(second.body.filename_hint);
  expect(String(first.body.sha256)).toMatch(/^[0-9a-f]{64}$/);
  expect(first.ran).toEqual(second.ran);

  // The accumulator ran the simulation at its own rate, not the render rate.
  const total = first.ran.reduce((sum, steps) => sum + steps, 0);
  expect(total).toBeGreaterThan(0);
  expect(total).toBeLessThan(script.length);
  const exportedFile = JSON.parse(String(first.body.text));
  expect(exportedFile.format).toBe('field-game-run');
  expect(exportedFile.save_version).toBe(1);
  expect(exportedFile.payload.field.now.step).toBe(total);
  expect(exportedFile.payload.run_id).toBe(KEY);

  // The Field crosses the boundary as its six locked parts, and a run now
  // stands on the chapter the manifest lists, so each part carries what the
  // chapter authored — every declared field is always present either way.
  const field = exportedFile.payload.field.now;
  for (const part of ['layers', 'ports', 'routes', 'forms', 'currents'] as const) {
    expect(Array.isArray(field[part])).toBe(true);
    expect(field[part].length).toBeGreaterThan(0);
  }
  // The boundary-leakage parameter is copied from the selected Form at run
  // start, and the authored Boundary list is the chapter's own.
  expect(field.boundaries.drawn).toEqual([]);
  expect(field.boundaries.authored).toEqual([{ members: [2, 3, 4] }]);
  expect(field.boundaries.leak_frac).toBeGreaterThan(0);
  // The content hash is the digest the build embedded, and the run records it.
  expect(exportedFile.payload.content_hash).toBe(CONTENT_HASH);
});

test('a different run key exports different bytes', async () => {
  const script = rendered(6);
  const first = await playThrough(KEY, script);
  const second = await playThrough(OTHER_KEY, script);

  expect(first.body.text).not.toBe(second.body.text);
  expect(first.body.sha256).not.toBe(second.body.sha256);
});

test('a different frame sequence exports different bytes', async () => {
  const straight = await playThrough(KEY, rendered(6));
  const steered = await playThrough(
    KEY,
    rendered(6).map((frame) => ({ ...frame, steer_x: 4_096 })),
  );

  expect(straight.body.text).not.toBe(steered.body.text);
});

test('the frame event acknowledges a frame and carries the snapshot when a step ran', async () => {
  const session = openSession();
  await openRun(session, KEY);

  const seen: FrameEventBody[] = [];
  for (const frame of rendered(4)) {
    const answer = await session.send(frame);
    expect(answer).toHaveProperty('frame');
    if ('frame' in answer) seen.push(answer.frame);
  }

  expect(seen.map((frame) => frame.seq)).toEqual([1, 2, 3, 4]);
  let carried = 0;
  for (const frame of seen) {
    expect(frame.dropped).toBe(false);
    expect(frame.remainder_us).toBeGreaterThanOrEqual(0);
    expect(frame.remainder_us).toBeLessThan(1_000_000 / 30);
    if (frame.steps_run === 0 && frame.buffer === undefined) continue;
    carried += 1;
    const bytes = new Uint8Array(frame.buffer as ArrayBuffer);
    expect(bytes.length).toBeGreaterThan(32);
    expect(String.fromCharCode(...bytes.subarray(0, 4))).toBe('FGF1');
    const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
    expect(view.getUint16(4, true)).toBe(1);
    expect(view.getUint8(14)).toBe(0);
  }
  expect(carried).toBeGreaterThan(0);
});

test('a paused frame injects no steps and holds the counter still', async () => {
  const session = openSession();
  await openRun(session, KEY);

  for (const frame of rendered(4)) await session.send(frame);
  const before = await exported(session);

  for (let index = 0; index < 4; index += 1) {
    const seq = 5 + index;
    const answer = await session.send({ ...neutralFrame(seq, seq * FRAME_US), pause: true });
    expect(answer).toHaveProperty('frame');
    if ('frame' in answer) {
      expect(answer.frame.steps_run).toBe(0);
      expect(answer.frame.remainder_us).toBe(0);
    }
  }

  const after = await exported(session);
  expect(after.text).toBe(before.text);
  expect(after.sha256).toBe(before.sha256);
});

test('a long gap runs the catch-up limit and flags the time it dropped', async () => {
  const session = openSession();
  await openRun(session, KEY);

  await session.send(neutralFrame(1, 1_000_000));
  const answer = await session.send(neutralFrame(2, 1_000_000 + 10_000_000));

  expect(answer).toHaveProperty('frame');
  if (!('frame' in answer)) return;
  expect(answer.frame.steps_run).toBe(6);
  expect(answer.frame.dropped).toBe(true);
  expect(answer.frame.remainder_us).toBe(33_333);
});

test('a batch wider than the event field saturates it and still runs in full', async () => {
  const session = openSession();
  await openRun(session, KEY);

  // The event's `steps_run` is a u8; the run itself is not bounded by it.
  const answer = await session.send({ ...neutralFrame(1, FRAME_US), advance_steps: 300 });
  expect(answer).toHaveProperty('frame');
  if ('frame' in answer) expect(answer.frame.steps_run).toBe(255);

  const body = await exported(session);
  expect(JSON.parse(String(body.text)).payload.field.now.step).toBe(300);
  expect(body.filename_hint).toBe(`field-run-${KEY}-step-300.json`);
});

test('the payload carries the depth resolution state a restore has to land on', async () => {
  const session = openSession();
  await openRun(session, KEY);

  // Two turns of the wheel cross the trigger, so the run is left inside the
  // cooldown with the accumulator cleared.
  await session.send({ ...neutralFrame(1, FRAME_US), wheel: 300, advance_steps: 1 });
  await session.send({ ...neutralFrame(2, 2 * FRAME_US), wheel: 300, advance_steps: 1 });

  const now = JSON.parse(String((await exported(session)).text)).payload.field.now;
  expect(now.wheel_accum).toBe(0);
  expect(now.depth_cooldown).toBe(14);
});

test('a recorded trace with wheel noise in it replays byte for byte', async () => {
  // The accumulator under test is the worker's own, so this trace is driven by
  // timestamps: at the render rate against 30 steps per second about half these
  // frames execute no step at all, which is exactly where a depth resolution
  // can be lost. Two runs of the same recorded frames must agree byte for byte,
  // and the changes the noise resolves must be the two the gestures asked for.
  const noise = [6, -5, 8, -9, 3, -4, 11, -12];
  // A turn is a run of deltas the same way: the sum crosses the trigger part
  // way through it and the rest of the turn holds it there until a frame runs
  // a step. Between the two turns there is nothing but noise, for longer than
  // the cooldown the first one started.
  const turning = (index: number): number => {
    if (index >= 8 && index < 14) return 90;
    if (index >= 48) return -90;
    return noise[index % 8];
  };
  const script = rendered(60).map((frame, index) => ({ ...frame, wheel: turning(index) }));

  const first = await playThrough(KEY, script);
  const second = await playThrough(KEY, script);
  expect(first.body.text).toBe(second.body.text);
  expect(first.body.sha256).toBe(second.body.sha256);
  expect(first.ran).toEqual(second.ran);
  // The trace is a real one: some of its frames ran no step.
  expect(first.ran.some((steps) => steps === 0)).toBe(true);

  const moves = (JSON.parse(String(first.body.text)) as {
    payload: { field: { trace: { steps: { ctl: { depth_move: number } }[] } } };
  }).payload.field.trace.steps.map((step) => step.ctl.depth_move);
  expect(moves.filter((change) => change !== 0)).toEqual([1, -1]);

  // And the noise alone resolves nothing, which is what the threshold is for.
  const quiet = await playThrough(
    KEY,
    rendered(60).map((frame, index) => ({ ...frame, wheel: noise[index % 8] })),
  );
  const quietMoves = (JSON.parse(String(quiet.body.text)) as {
    payload: { field: { trace: { steps: { ctl: { depth_move: number } }[] } } };
  }).payload.field.trace.steps.map((step) => step.ctl.depth_move);
  expect(quietMoves.every((change) => change === 0)).toBe(true);
});

test('a frame the core refuses answers with an error and runs nothing', async () => {
  const session = openSession();
  await openRun(session, KEY);

  for (const frame of rendered(2)) await session.send(frame);
  const before = await exported(session);

  const stale = await session.send(neutralFrame(1, 3 * FRAME_US));
  expect(stale).toHaveProperty('refused');
  if ('refused' in stale) expect(stale.refused.code).toBe('validation');

  expect((await exported(session)).text).toBe(before.text);
});

// ---------------------------------------------------------------------------
// The Pulse, through the accumulator
// ---------------------------------------------------------------------------

/** The control every recorded step of an exported run consumed. */
function recordedControl(body: Payload): { pulse_held: boolean; pulse_release: boolean }[] {
  const file = JSON.parse(String(body.text)) as {
    payload: { field: { trace: { steps: { ctl: { pulse_held: boolean; pulse_release: boolean } }[] } } };
  };
  return file.payload.field.trace.steps.map((step) => step.ctl);
}

test('a run that pulsed and a run that did not export different bytes', async () => {
  // One press held across the middle of the run and let go of near its end,
  // sent the way the pump sends it: `pulse_held` on every frame of the hold and
  // `pulse_release` on the frame that lets go.
  const script = rendered(12).map((frame, index) => ({
    ...frame,
    pulse_held: index >= 2 && index < 8,
    pulse_release: index === 8,
  }));
  const pulsed = await playThrough(KEY, script);
  const quiet = await playThrough(KEY, rendered(12));

  expect(pulsed.body.text).not.toBe(quiet.body.text);
  expect(pulsed.ran).toEqual(quiet.ran);

  // The same frames twice are the same bytes: the Pulse is part of the recorded
  // control schedule, and a recorded schedule replays byte-exact.
  const again = await playThrough(KEY, script);
  expect(again.body.text).toBe(pulsed.body.text);
  expect(again.body.sha256).toBe(pulsed.body.sha256);

  const control = recordedControl(pulsed.body);
  expect(control.some((step) => step.pulse_held)).toBe(true);
  expect(control.filter((step) => step.pulse_release)).toHaveLength(1);
  expect(recordedControl(quiet.body).every((step) => !step.pulse_held)).toBe(true);
});

test('a catch-up frame emits one Pulse and charges every step of the batch', async () => {
  // The locked accumulator runs up to six catch-up steps from one frame, each
  // consuming that frame's control state. The release carries the depth idiom:
  // the first executed step of the batch records it and every later step
  // records false, so one frame emits at most one Pulse however many steps it
  // runs. The hold is a level and is recorded on every step of the batch, so a
  // batch covering 200 ms of held time charges 200 ms of held time.
  const session = openSession();
  await openRun(session, KEY);
  // A 100 ms gap is three steps of catch-up, all of them holding.
  await session.send({ ...neutralFrame(1, 0), pulse_held: true });
  await session.send({ ...neutralFrame(2, 100_000), pulse_held: true });
  const held = recordedControl(await exported(session));
  expect(held.length).toBe(3);
  expect(held.every((step) => step.pulse_held)).toBe(true);

  // The next batch releases: one release, on the first executed step.
  await session.send({ ...neutralFrame(3, 200_000), pulse_held: false, pulse_release: true });
  const body = await exported(session);
  session.close();

  const control = recordedControl(body);
  expect(control.length).toBe(6);
  expect(control.slice(3).map((step) => step.pulse_release)).toEqual([true, false, false]);
});
