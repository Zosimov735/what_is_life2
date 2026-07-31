/**
 * Steering, all the way through: a device, a frame, a step, a Form that moved.
 *
 * The two claims this goal is done by are read here at their widest. A recorded
 * trace of pointer movement replays to identical exports — the input pipeline
 * introduces nothing a second run would not reproduce. And the same logical
 * steering, named once by the cursor and once by the keys, produces the same
 * frames and therefore the same run, byte for byte.
 *
 * The run these are played on is the development stand-in, opened the way the
 * local preview opens it: `import_run` over an export the core validates like
 * any other. It is also what keeps that stand-in honest, because a payload the
 * core stopped accepting would fail here first.
 *
 * The worker is the real entry loading the real module, and sessions are opened
 * one after another rather than side by side.
 */

import '@vitest/web-worker';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { afterEach, beforeAll, expect, inject, test, vi } from 'vitest';
import {
  neutralFrame,
  PROTOCOL_VERSION,
  type CommandEnvelope,
  type EventEnvelope,
  type FrameEventBody,
  type InputFrame,
  type Payload,
  type ResponseEnvelope,
} from '../../worker/src/protocol';
import { decodeFrameState } from '../../worker/src/frame-state';
import { DEV_RUN_EXPORT } from '../src/shell/dev-run';
import {
  KEY_FALL_FRAMES,
  KEY_RISE_FRAMES,
  openSteering,
  SATURATION_RADIUS_PX,
  STEER_UNIT,
  type Steering,
} from '../src/shell/steering';
import { openCore, type CoreClient } from '../src/shell/worker-client';

const WORKER_ENTRY = new URL('../../worker/src/entry.ts', import.meta.url);
const WORKSPACE = inject('workspace');

/** One rendered frame at the 60-frames-per-second target, in microseconds. */
const FRAME_US = 16_667;

/** Where the stand-in Field puts the controlled Form, in raw units. */
const OPENING_POS = 2048 * 65_536;

beforeAll(() => {
  vi.stubGlobal('fetch', async (target: URL | string) => {
    const { pathname } = new URL(String(target), 'http://localhost/');
    const bytes = await readFile(path.join(WORKSPACE, pathname));
    return new Response(bytes, { headers: { 'content-type': 'application/wasm' } });
  });
});

const opened: Worker[] = [];
const sources: Steering[] = [];
const restores: (() => void)[] = [];

afterEach(() => {
  for (const worker of opened.splice(0)) worker.terminate();
  for (const source of sources.splice(0)) source.close();
  for (const restore of restores.splice(0)) restore();
});

interface Session {
  command: (cmd: string, body: Payload) => Promise<ResponseEnvelope>;
  send: (frame: InputFrame) => Promise<FrameEventBody>;
  close: () => void;
}

function openSession(): Session {
  const worker = new Worker(WORKER_ENTRY, { type: 'module' });
  opened.push(worker);
  const responses = new Map<number, (answer: ResponseEnvelope) => void>();
  const frames = new Map<number, (answer: FrameEventBody) => void>();
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
    frames.get(body.seq)?.(body);
    frames.delete(body.seq);
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
        responses.set(id, (answer) => {
          throw new Error(`a frame was refused: ${JSON.stringify(answer)}`);
        });
        worker.postMessage({ v: PROTOCOL_VERSION, id, cmd: 'input_frame', body: frame });
      });
    },
    close: () => worker.terminate(),
  };
}

/** A steering source over an event target of its own, middled on the origin. */
function steering(): { source: Steering; target: EventTarget } {
  const target = new EventTarget();
  const source = openSteering({ target, middle: () => ({ x: 0, y: 0 }) });
  sources.push(source);
  return { source, target };
}

function moveTo(target: EventTarget, x: number, y: number): void {
  target.dispatchEvent(Object.assign(new Event('pointermove'), { clientX: x, clientY: y }));
}

function press(target: EventTarget, code: string, down = true): void {
  target.dispatchEvent(Object.assign(new Event(down ? 'keydown' : 'keyup'), { code, repeat: false }));
}

/** Opens the development run and answers with the state it opened on. */
async function openStandInRun(session: Session): Promise<Payload> {
  const answer = await session.command('import_run', { text: DEV_RUN_EXPORT });
  expect(answer.ok).toBe(true);
  return answer.ok ? answer.body : {};
}

async function exported(session: Session): Promise<{ text: string; sha256: string }> {
  const answer = await session.command('export_run', {});
  expect(answer.ok).toBe(true);
  if (!answer.ok) throw new Error('an export was expected');
  return answer.body as unknown as { text: string; sha256: string };
}

/** The controlled Form, as an export file carries it. */
function controlled(text: string): { pos: { x: number; y: number }; vel: { x: number; y: number } } {
  const payload = JSON.parse(text).payload as {
    field: { now: { forms: { controlled: boolean; pos: { x: number; y: number }; vel: { x: number; y: number } }[] } };
  };
  const form = payload.field.now.forms.find((held) => held.controlled);
  if (!form) throw new Error('the stand-in run carries a controlled Form');
  return { pos: form.pos, vel: form.vel };
}

/** Plays a script of frames into a fresh run and exports what it left. */
async function played(script: InputFrame[]): Promise<{ text: string; sha256: string }> {
  const session = openSession();
  await openStandInRun(session);
  for (const frame of script) await session.send(frame);
  const file = await exported(session);
  session.close();
  return file;
}

// ---------------------------------------------------------------------------
// The feel, in the millisecond domain the numbers are stated in
// ---------------------------------------------------------------------------

/** The render target, in milliseconds: 60 frames a second, exactly. */
const FRAME_MS = 16.667;

/**
 * The shell's own pump, over a real worker, with the animation frames handed
 * out one at a time at exactly the render rate.
 *
 * This is what makes the feel readable in milliseconds rather than in steps:
 * the timestamps are the ones the pump would see at 60 frames a second, the
 * accumulator turns them into steps at its own rate, and the velocity comes
 * back from the core that integrated it.
 */
async function pumped(source: Steering): Promise<{
  client: CoreClient;
  sent: InputFrame[];
  tick: (frame: number) => Promise<void>;
}> {
  const waiting = new Map<number, FrameRequestCallback>();
  let handle = 0;
  const heldRequest = globalThis.requestAnimationFrame;
  const heldCancel = globalThis.cancelAnimationFrame;
  globalThis.requestAnimationFrame = ((run: FrameRequestCallback) => {
    handle += 1;
    waiting.set(handle, run);
    return handle;
  }) as typeof globalThis.requestAnimationFrame;
  globalThis.cancelAnimationFrame = ((held: number) => waiting.delete(held)) as typeof globalThis.cancelAnimationFrame;
  restores.push(() => {
    globalThis.requestAnimationFrame = heldRequest;
    globalThis.cancelAnimationFrame = heldCancel;
  });

  const sent: InputFrame[] = [];
  const client = openCore({
    form: 'thread',
    steering: source,
    spawn: () => {
      const worker = new Worker(WORKER_ENTRY, { type: 'module' });
      opened.push(worker);
      const post = worker.postMessage.bind(worker);
      worker.postMessage = (message: unknown, ...rest: unknown[]): void => {
        const envelope = message as CommandEnvelope;
        if (envelope?.cmd === 'input_frame') sent.push(envelope.body as unknown as InputFrame);
        (post as (...given: unknown[]) => void)(message, ...rest);
      };
      return worker;
    },
  });
  await client.ready;

  return {
    client,
    sent,
    async tick(frame: number) {
      const due = [...waiting.values()];
      waiting.clear();
      for (const run of due) run(frame * FRAME_MS);
      // The worker answers on the message queue, so one turn of it settles the
      // frame before the next one is handed out.
      await new Promise((settle) => setTimeout(settle, 0));
    },
  };
}

/** A steering source with a target and a middle of the test's own. */
function pointing(): { source: Steering; target: EventTarget } {
  const held = steering();
  return held;
}

/** The controlled Form as the newest snapshot carries it. */
function steered(client: CoreClient): { x: number; y: number; vx: number; vy: number } {
  const snapshot = client.snapshot();
  const form = snapshot?.forms.find((one) => one.controlled);
  if (!form) throw new Error('the development run carries a controlled Form');
  return { x: form.x, y: form.y, vx: form.vx, vy: form.vy };
}

/** Opens the preview on the development run, for the life of one test. */
function onStandInRun(): void {
  const held = window.location.search;
  window.history.replaceState({}, '', `${window.location.pathname}?field_run`);
  restores.push(() => window.history.replaceState({}, '', `${window.location.pathname}${held}`));
}

test('the pump reaches whole deflection in the milliseconds the ramp states', async () => {
  onStandInRun();
  const { source, target } = pointing();
  const { client, sent, tick } = await pumped(source);

  press(target, 'KeyD');
  for (let frame = 1; frame <= KEY_RISE_FRAMES; frame += 1) await tick(frame);

  // Eight rendered frames at the render rate is 133 ms, and that is where the
  // ramp is whole — the number the steering block states, read in the domain
  // it states it in.
  expect(sent).toHaveLength(KEY_RISE_FRAMES);
  expect(sent[KEY_RISE_FRAMES - 1].steer_x).toBe(STEER_UNIT);
  expect(sent[KEY_RISE_FRAMES - 1].t_us).toBe(Math.round(KEY_RISE_FRAMES * FRAME_MS * 1000));
  expect(sent[KEY_RISE_FRAMES - 1].t_us / 1000).toBeCloseTo(133.3, 1);
  expect(sent[0].steer_x).toBe(Math.round(STEER_UNIT / KEY_RISE_FRAMES));
  // Every frame the pump sends is a timed frame: the step hook's field is null,
  // so the accumulator decides the step count and not the shell.
  expect(sent.every((frame) => frame.advance_steps === null)).toBe(true);
  expect(sent.every((frame) => frame.pause === false)).toBe(true);
  client.close();
}, 120_000);

test('the spring reaches 95% of its speed in the milliseconds the block states', async () => {
  onStandInRun();
  const { source, target } = pointing();
  const { client, sent, tick } = await pumped(source);

  // A cursor at the saturation radius asks for the whole speed from the first
  // frame, so what is measured here is the spring alone.
  moveTo(target, SATURATION_RADIUS_PX, 0);
  for (let frame = 1; frame <= 23; frame += 1) await tick(frame);

  // Twenty-three frames at the render rate is 383 ms of wall time, of which the
  // accumulator has spent 367 ms — eleven steps, the number the block states.
  const consumed = sent[22].t_us - sent[0].t_us;
  expect(consumed).toBe(366_674);
  expect(client.snapshot()?.header.step).toBe(11);
  const terminal = 20;
  expect(steered(client).vx).toBeGreaterThanOrEqual(0.95 * terminal);
  expect(steered(client).vx).toBeLessThanOrEqual(terminal);

  // And the rate itself, in the same domain: one wall second of frames is
  // thirty steps.
  for (let frame = 24; frame <= 61; frame += 1) await tick(frame);
  expect(sent[60].t_us - sent[0].t_us).toBe(1_000_020);
  expect(client.snapshot()?.header.step).toBe(30);
  client.close();
}, 120_000);

test('the development run opens on a Field with a Form to steer', async () => {
  const session = openSession();
  const body = await openStandInRun(session);
  expect(body.step).toBe(0);

  const file = await exported(session);
  const form = controlled(file.text);
  expect(form.pos).toEqual({ x: OPENING_POS, y: OPENING_POS });
  expect(form.vel).toEqual({ x: 0, y: 0 });
  // Enough of a Field to steer around: Nodes on two planes, and a current.
  const now = JSON.parse(file.text).payload.field.now;
  expect(now.ports.length).toBeGreaterThan(4);
  expect(now.layers).toHaveLength(2);
  expect(now.currents.length).toBeGreaterThan(0);
}, 60_000);

test('a recorded trace of pointer movement replays to identical exports', async () => {
  // A gesture with a shape: out to the saturation radius, around, back through
  // the middle, and a few frames of the small jitter a trackpad leaves under a
  // resting hand.
  const { source, target } = steering();
  const script: InputFrame[] = [];
  for (let frame = 1; frame <= 90; frame += 1) {
    const turn = (frame / 90) * Math.PI * 2;
    const reach = frame < 60 ? 40 + frame * 5 : 6;
    const jitter = frame < 60 ? 0 : ((frame % 3) - 1) * 0.7;
    moveTo(target, Math.cos(turn) * reach + jitter, Math.sin(turn) * reach - jitter);
    script.push({ ...neutralFrame(frame, frame * FRAME_US), ...source.sample() });
  }
  const longest = Math.max(
    ...script.map((frame) => Math.hypot(frame.steer_x, frame.steer_y)),
  );
  expect(longest).toBeGreaterThan(30_000);
  expect(script.filter((frame) => frame.steer_x === 0 && frame.steer_y === 0).length)
    .toBeGreaterThan(0);

  const first = await played(script);
  const second = await played(script);

  expect(first.text).toBe(second.text);
  expect(first.sha256).toBe(second.sha256);
  expect(first.sha256).toMatch(/^[0-9a-f]{64}$/);

  // And the trace actually steered: the Form left where it stood, and the
  // frames under the rest radius left it drifting rather than parked.
  const form = controlled(first.text);
  expect(form.pos.x).not.toBe(OPENING_POS);
  expect(form.pos.y).not.toBe(OPENING_POS);
  expect(Math.abs(form.vel.x) + Math.abs(form.vel.y)).toBeGreaterThan(0);
}, 120_000);

test('the same steering named by the keys and by the cursor is the same run', async () => {
  // One gesture: a whole deflection along one axis, held and then released.
  // The keys name it by their ramp, and the cursor names it by standing where
  // the ramp stands — which is the same offset, from the two devices.
  const held = 12;
  const released = 14;
  const keyboard = steering();
  const cursor = steering();
  const byKey: InputFrame[] = [];
  const byCursor: InputFrame[] = [];

  press(keyboard.target, 'KeyD');
  for (let frame = 1; frame <= held; frame += 1) {
    moveTo(cursor.target, SATURATION_RADIUS_PX * Math.min(1, frame / KEY_RISE_FRAMES), 0);
    byKey.push({ ...neutralFrame(frame, frame * FRAME_US), ...keyboard.source.sample() });
    byCursor.push({ ...neutralFrame(frame, frame * FRAME_US), ...cursor.source.sample() });
  }
  press(keyboard.target, 'KeyD', false);
  for (let frame = 1; frame <= released; frame += 1) {
    const seq = held + frame;
    moveTo(cursor.target, SATURATION_RADIUS_PX * Math.max(0, 1 - frame / KEY_FALL_FRAMES), 0);
    byKey.push({ ...neutralFrame(seq, seq * FRAME_US), ...keyboard.source.sample() });
    byCursor.push({ ...neutralFrame(seq, seq * FRAME_US), ...cursor.source.sample() });
  }

  // Identical normalized messages, which is the whole of the claim: not
  // equivalent, not close, the same bytes.
  expect(JSON.stringify(byKey)).toBe(JSON.stringify(byCursor));
  // With the ramp in it rather than a switch: the deflection climbs and falls.
  expect(byKey[0].steer_x).toBeGreaterThan(0);
  expect(byKey[0].steer_x).toBeLessThan(byKey[held - 1].steer_x);
  expect(byKey[byKey.length - 1].steer_x).toBe(0);

  const fromKeys = await played(byKey);
  const fromCursor = await played(byCursor);
  expect(fromKeys.text).toBe(fromCursor.text);
  expect(fromKeys.sha256).toBe(fromCursor.sha256);

  // And the run moved: one axis, the direction the key names, and the Form
  // still drifting when the control let go.
  const form = controlled(fromKeys.text);
  expect(form.pos.x).toBeGreaterThan(OPENING_POS);
  expect(form.pos.y).toBe(OPENING_POS);
  expect(form.vel.x).toBeGreaterThan(0);
}, 120_000);

// ---------------------------------------------------------------------------
// The Pulse, all the way through: a device, a frame, a step, a Field that moved
// ---------------------------------------------------------------------------

/** Where the stand-in Field's closed Port stands, in units. */
const CLOSED_PORT = { node: 2, x: 1760, y: 1880 };

/** The Field, as an export file carries it. */
function fieldOf(text: string): {
  ports: { node: number; q: number; open: boolean }[];
  forms: {
    controlled: boolean;
    pulse_charge: number;
    focus: boolean;
    charge: number;
    pos: { x: number; y: number };
  }[];
} {
  return (JSON.parse(text) as { payload: { field: { now: ReturnType<typeof fieldOf> } } }).payload
    .field.now;
}

test('a Pulse gathers, opens a Port, and says so in the frame the shell decodes', async () => {
  const session = openSession();
  await openStandInRun(session);
  const before = fieldOf((await exported(session)).text);
  const port = before.ports.find((one) => one.node === CLOSED_PORT.node);
  expect(port?.open).toBe(false);
  expect(port?.q).toBeGreaterThan(0);

  // Steer to the closed Port, then stand and charge beside it, then release.
  // The steering is the locked normalized pair aimed along the offset between
  // the two; the Pulse fields are what a held button sends.
  let seq = 1;
  const frameOf = (steer: [number, number], held: boolean, release = false): InputFrame => ({
    ...neutralFrame(seq, seq++ * FRAME_US),
    steer_x: steer[0],
    steer_y: steer[1],
    pulse_held: held,
    pulse_release: release,
    advance_steps: 1,
  });

  const toward: [number, number] = [-28_300, -16_500];
  for (let frame = 0; frame < 20; frame += 1) await session.send(frameOf(toward, false));
  // Full in exactly 32 held steps, coasting to rest as it charges.
  let carried: FrameEventBody | null = null;
  for (let frame = 0; frame < 32; frame += 1) carried = await session.send(frameOf([0, 0], true));
  const charging = fieldOf((await exported(session)).text);
  expect(charging.forms[0].pulse_charge).toBe(65_536);
  expect(charging.forms[0].focus).toBe(true);
  // The Form stands inside a full reach of the Port, which is what makes the
  // release below a release that reaches something.
  const at = charging.forms[0].pos;
  const reach = Math.hypot(at.x / 65_536 - CLOSED_PORT.x, at.y / 65_536 - CLOSED_PORT.y);
  expect(reach).toBeLessThan(192);

  carried = await session.send(frameOf([0, 0], false, true));
  const file = (await exported(session)).text;
  const after = fieldOf(file);

  // The Field moved: the Port opened and gave a quarter of what it held, and
  // the Form's own Node holds that quarter.
  const opened = after.ports.find((one) => one.node === CLOSED_PORT.node);
  const gathered = Math.floor((port!.q * 16_384) / 65_536);
  expect(opened?.open).toBe(true);
  expect(after.forms[0].charge).toBe(before.forms[0].charge + gathered);
  expect(after.forms[0].pulse_charge).toBe(0);
  // And what the Port kept left it down the Route it was closed to a moment
  // ago: the Pulse phase runs before the Route phase, so a Port opened by an
  // emission participates in the same step it opened in.
  const route = JSON.parse(file).payload.field.now.routes[0] as {
    tail: number;
    capacity: number;
  };
  expect(route.tail).toBe(CLOSED_PORT.node);
  expect(opened?.q).toBe(port!.q - gathered - route.capacity);

  // And the frame the shell decodes says all of it: the emission with its
  // reach, the gather, and the Port that opened — every one of them in the
  // same frame, which is the signal the surface and the sound read.
  expect(carried?.buffer).toBeDefined();
  const snapshot = decodeFrameState(carried!.buffer!);
  const kinds = snapshot.cues.map((cue) => cue.kind);
  expect(kinds).toContain(1);
  expect(kinds).toContain(2);
  expect(kinds).toContain(3);
  const emitted = snapshot.cues.find((cue) => cue.kind === 1);
  expect(emitted?.a).toBe(192 * 256);
  expect(snapshot.forms[0].radius).toBe(192 * 256);
  expect(snapshot.cues.find((cue) => cue.kind === 3)?.b).toBe(CLOSED_PORT.node);
  expect(snapshot.ports.find((one) => one.node === CLOSED_PORT.node)?.open).toBe(true);
  session.close();
}, 120_000);

test('a release that reaches nothing says only that a Pulse was emitted', async () => {
  const session = openSession();
  await openStandInRun(session);
  const before = fieldOf((await exported(session)).text);
  // Standing where the run opens, nothing is within even a full reach: the
  // nearest Node is 333 units away and 192 is as far as a Pulse goes.
  let seq = 1;
  const still = (held: boolean, release = false): InputFrame => ({
    ...neutralFrame(seq, seq++ * FRAME_US),
    pulse_held: held,
    pulse_release: release,
    advance_steps: 1,
  });
  for (let frame = 0; frame < 32; frame += 1) await session.send(still(true));
  const carried = await session.send(still(false, true));
  const after = fieldOf((await exported(session)).text);
  session.close();

  const snapshot = decodeFrameState(carried.buffer!);
  expect(snapshot.cues.map((cue) => cue.kind)).toEqual([1]);
  expect(snapshot.cues[0].a).toBe(192 * 256);
  expect(after.forms[0].pulse_charge).toBe(0);
  // A Pulse that reaches nothing takes nothing and opens nothing: the Form's
  // own Node holds exactly what it held, and both closed Ports are still
  // closed. What the rest of the Field did, it did under the other four rules.
  expect(after.forms[0].charge).toBe(before.forms[0].charge);
  for (const node of [2, 5]) {
    expect(after.ports.find((one) => one.node === node)?.open).toBe(false);
  }
}, 120_000);
