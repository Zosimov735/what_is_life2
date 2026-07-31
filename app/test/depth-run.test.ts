/**
 * Depth, all the way through: a gesture, a frame, a step, a Form one layer
 * deeper.
 *
 * The claims this goal is done by are read here at their widest, against the
 * real worker and the real module. Ordinary trackpad noise changes nothing; a
 * deliberate turn changes depth exactly once and then holds off; the way back up
 * is open the moment that hold-off ends; the bracket keys reach the same change
 * through the same path; and a recorded trace with wheel noise in it replays to
 * identical exports, which is what makes difficulty selection deterministic
 * under recorded inputs.
 *
 * The rendered frames matter here rather than only the steps: at the 60 frames
 * per second target against 30 steps per second about half of them execute no
 * step at all, and a gesture that lands on one of those is deferred to the first
 * frame that does — never dropped. That is the carry-forward decision this goal
 * locked, read in the domain a player feels it in.
 *
 * The run is the development stand-in, opened the way the local preview opens
 * it: `import_run` over an export the core validates like any other. It stands
 * on two layers, which is the whole ladder a depth change has to move on.
 */

import '@vitest/web-worker';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { afterEach, beforeAll, expect, inject, test, vi } from 'vitest';
import {
  PROTOCOL_VERSION,
  type CommandEnvelope,
  type EventEnvelope,
  type FrameEventBody,
  type InputFrame,
  type Payload,
  type ResponseEnvelope,
} from '../../worker/src/protocol';
import { DEV_RUN_EXPORT } from '../src/shell/dev-run';
import { openDepth, type Depth } from '../src/shell/depth';
import { openCore, type CoreClient } from '../src/shell/worker-client';

const WORKER_ENTRY = new URL('../../worker/src/entry.ts', import.meta.url);
const WORKSPACE = inject('workspace');

/** The render target, in milliseconds: 60 frames a second, exactly. */
const FRAME_MS = 16.667;

/** How many executed steps the locked cooldown holds the next change off for. */
const COOLDOWN_STEPS = 15;

/** How far the accumulated delta must reach for one depth change. Locked. */
const TRIGGER_PX = 480;

beforeAll(() => {
  vi.stubGlobal('fetch', async (target: URL | string) => {
    const { pathname } = new URL(String(target), 'http://localhost/');
    const bytes = await readFile(path.join(WORKSPACE, pathname));
    return new Response(bytes, { headers: { 'content-type': 'application/wasm' } });
  });
});

const opened: Worker[] = [];
const sources: Depth[] = [];
const restores: (() => void)[] = [];

afterEach(() => {
  for (const worker of opened.splice(0)) worker.terminate();
  for (const source of sources.splice(0)) source.close();
  for (const restore of restores.splice(0)) restore();
  document.body.innerHTML = '';
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

/** Opens the preview on the development run, for the life of one test. */
function onStandInRun(): void {
  const held = window.location.search;
  window.history.replaceState({}, '', `${window.location.pathname}?field_run`);
  restores.push(() => window.history.replaceState({}, '', `${window.location.pathname}${held}`));
}

/** The play surface, as the shell builds it. */
function surface(): HTMLCanvasElement {
  const canvas = document.createElement('canvas');
  canvas.className = 'field';
  document.body.append(canvas);
  return canvas;
}

/** Turns the wheel over the play surface. */
function turn(over: Element, deltaY: number): void {
  over.dispatchEvent(new WheelEvent('wheel', { deltaY, bubbles: true, cancelable: true }));
}

/** Presses or releases a key by its code. */
function press(code: string, down = true): void {
  window.dispatchEvent(Object.assign(new Event(down ? 'keydown' : 'keyup'), { code, repeat: false }));
}

/**
 * The shell's own pump over a real worker, with the animation frames handed out
 * one at a time at exactly the render rate — the same harness the steering run
 * is read through, carrying a depth source instead of a steering one.
 */
async function pumped(): Promise<{
  client: CoreClient;
  source: Depth;
  canvas: HTMLCanvasElement;
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
  globalThis.cancelAnimationFrame = ((held: number) =>
    waiting.delete(held)) as typeof globalThis.cancelAnimationFrame;
  restores.push(() => {
    globalThis.requestAnimationFrame = heldRequest;
    globalThis.cancelAnimationFrame = heldCancel;
  });

  const source = openDepth();
  sources.push(source);
  const canvas = surface();
  const sent: InputFrame[] = [];
  const client = openCore({
    form: 'thread',
    depth: source,
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
    source,
    canvas,
    sent,
    async tick(frame: number) {
      const due = [...waiting.values()];
      waiting.clear();
      for (const run of due) run(frame * FRAME_MS);
      await new Promise((settle) => setTimeout(settle, 0));
    },
  };
}

/** The depth the controlled Form stands at, as the newest snapshot carries it. */
function depthOf(client: CoreClient): number {
  const form = client.snapshot()?.forms.find((one) => one.controlled);
  if (!form) throw new Error('the development run carries a controlled Form');
  return form.layer;
}

/** Plays a script of frames into a fresh stand-in run and exports what it left. */
async function played(script: InputFrame[]): Promise<{ text: string; sha256: string }> {
  const session = openSession();
  const opening = await session.command('import_run', { text: DEV_RUN_EXPORT });
  expect(opening.ok).toBe(true);
  for (const frame of script) await session.send(frame);
  const answer = await session.command('export_run', {});
  expect(answer.ok).toBe(true);
  if (!answer.ok) throw new Error('an export was expected');
  session.close();
  return answer.body as unknown as { text: string; sha256: string };
}

/** The controlled Form's depth, as an export file carries it. */
function depthInFile(text: string): number {
  const payload = JSON.parse(text).payload as {
    field: { now: { forms: { controlled: boolean; layer: number }[] } };
  };
  const form = payload.field.now.forms.find((one) => one.controlled);
  if (!form) throw new Error('the stand-in run carries a controlled Form');
  return form.layer;
}

test('ordinary trackpad noise never changes depth', async () => {
  onStandInRun();
  const { client, canvas, sent, tick } = await pumped();

  // Two seconds of the small deltas either way a hand resting on a trackpad
  // leaves. The sum wanders and never reaches the trigger, so nothing resolves.
  const noise = [7, -6, 9, -11, 4, -3, 12, -14, 5, -2, 8, -9];
  for (let frame = 1; frame <= 120; frame += 1) {
    turn(canvas, noise[frame % noise.length]);
    await tick(frame);
  }

  expect(depthOf(client)).toBe(0);
  expect(client.snapshot()?.header.step).toBeGreaterThan(50);
  expect(sent.some((frame) => frame.wheel !== 0)).toBe(true);
  client.close();
}, 120_000);

test('a deliberate turn crossing on a frame that runs no step is deferred, not lost', async () => {
  onStandInRun();
  const { client, canvas, sent, tick } = await pumped();

  // The first two rendered frames of a session execute no step: the first has
  // no previous timestamp to measure a gap against, and one frame at the render
  // rate is half a step. The whole gesture lands on them.
  turn(canvas, TRIGGER_PX);
  await tick(1);
  await tick(2);
  expect(sent[0].wheel).toBe(TRIGGER_PX);
  // No step has run, so no snapshot has been sent at all: the frame event
  // carries one exactly when a step ran or the mode changed.
  expect(client.snapshot()).toBeNull();

  // The first frame that does execute a step resolves it. Under a rule that
  // dropped it instead, this frame would have found the accumulator cleared and
  // the cooldown started for a change no step ever recorded.
  await tick(3);
  expect(client.snapshot()?.header.step).toBe(1);
  expect(depthOf(client)).toBe(1);
  client.close();
}, 120_000);

test('one turn is one change, the cooldown holds the next off, and the way up is open when it ends', async () => {
  onStandInRun();
  const { client, canvas, tick } = await pumped();
  let frame = 0;
  const run = async (frames: number, delta = 0): Promise<void> => {
    for (let index = 0; index < frames; index += 1) {
      frame += 1;
      if (delta !== 0) turn(canvas, delta);
      await tick(frame);
    }
  };

  // A deliberate turn, and one turn only: the whole gesture is one change even
  // though the frames after it keep turning the wheel the same way.
  await run(6, 120);
  expect(depthOf(client)).toBe(1);
  const changed = client.snapshot()?.header.step ?? 0;

  // Inside the cooldown nothing resolves, however hard the wheel is turned —
  // and the way back up is held off by exactly the same rule as the way down,
  // which is the only thing that ever holds it off at all.
  await run(10, -600);
  expect(depthOf(client)).toBe(1);
  expect((client.snapshot()?.header.step ?? 0) - changed).toBeLessThan(COOLDOWN_STEPS);

  // The moment it ends, the retreat resolves: the accumulator has been standing
  // at the trigger the whole time, and nothing else had to be done to be
  // allowed up.
  await run(24);
  expect((client.snapshot()?.header.step ?? 0) - changed).toBeGreaterThanOrEqual(COOLDOWN_STEPS);
  expect(depthOf(client)).toBe(0);
  client.close();
}, 120_000);

test('the bracket keys reach the same change through the same path', async () => {
  onStandInRun();
  const { client, sent, tick } = await pumped();
  let frame = 0;
  const run = async (frames: number): Promise<void> => {
    for (let index = 0; index < frames; index += 1) {
      frame += 1;
      await tick(frame);
    }
  };

  // Two frames of settling first, so the press lands on a run that is stepping.
  await run(3);
  press('BracketRight');
  await run(3);
  expect(depthOf(client)).toBe(1);
  // The press may be offered by more than one frame — a frame that runs no step
  // does not consume it — but it is offered until a step takes it and not after.
  const offered = sent.filter((one) => one.depth_key === 1).length;
  expect(offered).toBeGreaterThanOrEqual(1);

  // Holding the key names nothing more: one press is one change, and the source
  // stops offering it the moment a step has taken it.
  await run(20);
  expect(depthOf(client)).toBe(1);
  expect(sent.filter((one) => one.depth_key === 1)).toHaveLength(offered);

  // And the way up is a press of the other bracket, at any time: no cooldown
  // stands after a bracket change, so the retreat is immediate.
  press('BracketRight', false);
  press('BracketLeft');
  await run(3);
  expect(depthOf(client)).toBe(0);
  // Two presses, two changes, and nothing offered once both were taken.
  const asked = sent.filter((one) => one.depth_key !== 0).length;
  await run(6);
  expect(sent.filter((one) => one.depth_key !== 0)).toHaveLength(asked);
  expect(depthOf(client)).toBe(0);
  client.close();
}, 120_000);

test('a recorded trace of wheel noise and gestures replays to identical exports', async () => {
  onStandInRun();
  const { client, canvas, sent, tick } = await pumped();

  // A trace with everything in it: noise either way, two deliberate turns in
  // opposite directions with the cooldown between them, and a bracket press.
  const noise = [5, -4, 6, -7, 3, -2];
  for (let frame = 1; frame <= 120; frame += 1) {
    if (frame >= 10 && frame < 16) turn(canvas, 90);
    else if (frame >= 70 && frame < 76) turn(canvas, -90);
    else turn(canvas, noise[frame % noise.length]);
    if (frame === 100) press('BracketRight');
    await tick(frame);
  }
  client.close();

  const trace = sent.map((frame) => ({ ...frame }));
  expect(trace).toHaveLength(120);
  expect(trace.some((frame) => frame.wheel !== 0)).toBe(true);
  expect(trace.some((frame) => frame.depth_key !== 0)).toBe(true);

  const first = await played(trace);
  const second = await played(trace);
  expect(first.text).toBe(second.text);
  expect(first.sha256).toBe(second.sha256);
  expect(first.sha256).toMatch(/^[0-9a-f]{64}$/);

  // The trace did move the Form between layers, so the bytes it repeats are
  // bytes a depth change is part of: the same frames with the wheel taken out
  // of them land somewhere else entirely.
  const quiet = await played(trace.map((frame) => ({ ...frame, wheel: 0, depth_key: 0 })));
  expect(quiet.text).not.toBe(first.text);
  expect(depthInFile(quiet.text)).toBe(0);
}, 120_000);
