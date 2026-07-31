/**
 * Still Mode all the way through: a key, a frame, a mode, a paused Field.
 *
 * The pump is the shell's own, over the real worker and the real module, with
 * the animation frames handed out one at a time at exactly the render rate —
 * so the 250 ms the mode table states is read here in milliseconds rather than
 * in steps, and the commands Enter and Escape send are read as the session
 * sends them.
 *
 * The run these are played on is the development stand-in, opened the way the
 * local preview opens it, because a populated Field is what makes the surface
 * worth putting up at all.
 */

import '@vitest/web-worker';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { afterEach, beforeAll, expect, inject, test, vi } from 'vitest';
import type { CommandEnvelope, InputFrame } from '../../worker/src/protocol';
import { openSteering, type Steering } from '../src/shell/steering';
import { openStill, CANCEL_BINDING, COMMIT_BINDING, STILL_BINDING } from '../src/shell/still';
import { openCore, type CoreClient } from '../src/shell/worker-client';

const WORKER_ENTRY = new URL('../../worker/src/entry.ts', import.meta.url);
const WORKSPACE = inject('workspace');

/** The render target, in milliseconds: 60 frames a second, exactly. */
const FRAME_MS = 16.667;

/** How long a ramp takes, in milliseconds. Locked. */
const RAMP_MS = 250;

beforeAll(() => {
  vi.stubGlobal('fetch', async (target: URL | string) => {
    const { pathname } = new URL(String(target), 'http://localhost/');
    const bytes = await readFile(path.join(WORKSPACE, pathname));
    return new Response(bytes, { headers: { 'content-type': 'application/wasm' } });
  });
});

const opened: Worker[] = [];
const sources: { close: () => void }[] = [];
const restores: (() => void)[] = [];

afterEach(() => {
  for (const worker of opened.splice(0)) worker.terminate();
  for (const source of sources.splice(0)) source.close();
  for (const restore of restores.splice(0)) restore();
});

function press(target: EventTarget, code: string, down = true): void {
  target.dispatchEvent(
    Object.assign(new Event(down ? 'keydown' : 'keyup', { cancelable: true }), {
      code,
      repeat: false,
    }),
  );
}

/** Opens the preview on the development run, for the life of one test. */
function onStandInRun(): void {
  const held = window.location.search;
  window.history.replaceState({}, '', `${window.location.pathname}?field_run`);
  restores.push(() => window.history.replaceState({}, '', `${window.location.pathname}${held}`));
}

/**
 * The shell's own pump, over a real worker, with the animation frames handed
 * out one at a time at the render rate.
 */
async function pumped(): Promise<{
  client: CoreClient;
  sent: InputFrame[];
  keys: EventTarget;
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

  // One target for every key, so the steering and the Still Mode source read
  // the same presses a player's keyboard would put in front of both.
  const keys = new EventTarget();
  const steering: Steering = openSteering({ target: keys, middle: () => ({ x: 0, y: 0 }) });
  const still = openStill({ target: keys });
  sources.push(steering, still);

  const sent: InputFrame[] = [];
  const client = openCore({
    form: 'thread',
    steering,
    still,
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
  sources.push({ close: () => client.close() });

  return {
    client,
    sent,
    keys,
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

/** Runs frames until the client reports a mode, and answers with how many. */
async function until(
  client: CoreClient,
  tick: (frame: number) => Promise<void>,
  mode: string,
  from: number,
  cap = 60,
): Promise<number> {
  for (let frame = from; frame < from + cap; frame += 1) {
    await tick(frame);
    if (client.mode() === mode) return frame;
  }
  throw new Error(`the run never reached ${mode}, and stood in ${client.mode()}`);
}

test('Space slows the field over the locked 250 ms and pauses it', async () => {
  onStandInRun();
  const { client, tick, keys } = await pumped();

  let frame = 1;
  for (; frame <= 6; frame += 1) await tick(frame);
  expect(client.mode()).toBe('running');

  press(keys, STILL_BINDING);
  await tick(frame);
  const opened = frame;
  frame += 1;
  expect(client.mode()).toBe('ramp_in');

  const arrived = await until(client, tick, 'still', frame);
  // The frame the ramp completes on is the first one a whole 250 ms after the
  // frame that opened it, which at the render rate is the sixteenth.
  const span = (arrived - opened) * FRAME_MS;
  expect(span).toBeGreaterThanOrEqual(RAMP_MS);
  expect(span).toBeLessThan(RAMP_MS + FRAME_MS);

  // A paused Field stands: the step counter holds however many frames follow.
  const step = client.snapshot()?.header.step;
  for (let held = arrived + 1; held <= arrived + 6; held += 1) await tick(held);
  expect(client.snapshot()?.header.step).toBe(step);
  expect(client.mode()).toBe('still');
});

test('direct movement is disabled while the run is paused', async () => {
  onStandInRun();
  const { client, sent, tick, keys } = await pumped();

  // A steering key held from before the toggle and never let go of.
  press(keys, 'KeyD');
  let frame = 1;
  for (; frame <= 10; frame += 1) await tick(frame);
  expect(sent[sent.length - 1].steer_x).toBeGreaterThan(0);

  press(keys, STILL_BINDING);
  await tick(frame);
  frame += 1;
  const arrived = await until(client, tick, 'still', frame);

  const before = client.snapshot();
  for (let held = arrived + 1; held <= arrived + 8; held += 1) await tick(held);
  const paused = sent.slice(-6);
  expect(paused).not.toHaveLength(0);
  for (const one of paused) {
    expect(one.steer_x).toBe(0);
    expect(one.steer_y).toBe(0);
    expect(one.pulse_held).toBe(false);
    expect(one.wheel).toBe(0);
    expect(one.depth_key).toBe(0);
  }
  // And the Form the key was steering has not moved.
  const steered = client.snapshot()?.forms.find((one) => one.controlled);
  const held = before?.forms.find((one) => one.controlled);
  expect(steered?.x).toBe(held?.x);
  expect(steered?.vx).toBe(held?.vx);
});

test('Escape on an empty queue leaves without committing', async () => {
  onStandInRun();
  const { client, tick, keys } = await pumped();

  let frame = 1;
  for (; frame <= 4; frame += 1) await tick(frame);
  press(keys, STILL_BINDING);
  await tick(frame);
  frame += 1;
  const arrived = await until(client, tick, 'still', frame);
  frame = arrived + 1;

  // Escape asks the queue to give back its newest entry. Nothing is queued
  // yet, so this press is the second Escape by the only thing that makes one:
  // it removed nothing. The answer carries the queue all the same, which is
  // what the tray shows.
  press(keys, CANCEL_BINDING);
  await tick(frame);
  frame += 1;
  await tick(frame);
  frame += 1;
  expect(client.queue()).toEqual({
    entries: [],
    cost_total: 0,
    impulse: 3,
    impulse_after: 3,
  });
  expect(client.mode()).toBe('ramp_out');

  // And leaving applies nothing: the exit is a ramp, and the run comes back
  // moving with the Impulse it went in with.
  const back = await until(client, tick, 'running', frame);
  expect(back).toBeGreaterThan(frame);
  expect(client.snapshot()?.header.impulse).toBe(3);
});

test('a window that went away mid-inspection comes back to the same pause', async () => {
  onStandInRun();
  const { client, tick, keys } = await pumped();

  let frame = 1;
  for (; frame <= 4; frame += 1) await tick(frame);
  press(keys, STILL_BINDING);
  await tick(frame);
  frame += 1;
  const arrived = await until(client, tick, 'still', frame);
  frame = arrived + 1;
  const step = client.snapshot()?.header.step;

  // The locked focus-loss rule: one neutral frame carrying the pause level,
  // and the pump stops until the window returns.
  client.pause(true);
  await new Promise((settle) => setTimeout(settle, 0));
  expect(client.mode()).toBe('suspended');

  // The release rides one frame, and the surface is back on it: a blur is not
  // a decision to stop reading the Field.
  client.pause(false);
  await tick(frame);
  frame += 1;
  expect(client.mode()).toBe('still');
  expect(client.snapshot()?.header.stillVisible).toBe(true);
  // The overlay comes back with the pause it interrupted, carrying the
  // standing candidate's baseline envelope over the clamped window.
  expect(client.snapshot()?.overlay).not.toBeNull();
  expect(client.snapshot()?.header.step).toBe(step);
});

test('a window that went away mid-ramp comes back to a moving Field', async () => {
  onStandInRun();
  const { client, tick, keys } = await pumped();

  let frame = 1;
  for (; frame <= 4; frame += 1) await tick(frame);
  press(keys, STILL_BINDING);
  await tick(frame);
  frame += 1;
  await tick(frame);
  frame += 1;
  expect(client.mode()).toBe('ramp_in');

  client.pause(true);
  await new Promise((settle) => setTimeout(settle, 0));
  expect(client.mode()).toBe('suspended');

  // A ramp is a span of real time, and a suspended run spends none of it, so
  // the half-run ramp is discarded rather than resumed.
  client.pause(false);
  await tick(frame);
  frame += 1;
  await tick(frame);
  expect(client.mode()).toBe('running');
});

test('an Escape that left one inspection does not eject the player from the next', async () => {
  onStandInRun();
  const { client, tick, keys } = await pumped();

  let frame = 1;
  for (; frame <= 4; frame += 1) await tick(frame);
  press(keys, STILL_BINDING);
  await tick(frame);
  frame += 1;
  const arrived = await until(client, tick, 'still', frame);
  frame = arrived + 1;

  // The taught gesture, pressed as a player presses it: twice, faster than the
  // frames answer. The first leaves; the second has nowhere to go.
  press(keys, CANCEL_BINDING);
  press(keys, CANCEL_BINDING);
  for (let held = 0; held < 3; held += 1) {
    await tick(frame);
    frame += 1;
  }
  frame = (await until(client, tick, 'running', frame)) + 1;

  // Straight back in. The second Escape must not be waiting for it.
  press(keys, STILL_BINDING);
  await tick(frame);
  frame += 1;
  const again = await until(client, tick, 'still', frame);
  for (let held = again + 1; held <= again + 6; held += 1) await tick(held);
  expect(client.mode()).toBe('still');
});

test('Enter commits and the commit is the exit', async () => {
  onStandInRun();
  const { client, tick, keys } = await pumped();

  let frame = 1;
  for (; frame <= 4; frame += 1) await tick(frame);
  press(keys, STILL_BINDING);
  await tick(frame);
  frame += 1;
  const arrived = await until(client, tick, 'still', frame);
  frame = arrived + 1;

  press(keys, COMMIT_BINDING);
  await tick(frame);
  frame += 1;
  await tick(frame);
  frame += 1;
  // Nothing was queued, so nothing was applied and no Impulse was spent — and
  // the commit is still an exit, which is the mode table's own second trigger
  // into the way out.
  expect(client.mode()).toBe('ramp_out');
  expect(client.queue().impulse).toBe(3);
  expect(client.snapshot()?.header.impulse).toBe(3);

  await until(client, tick, 'running', frame);
});

test('a queued change reaches the worker, shows its cost, and commits', async () => {
  // The shell's own half of the transaction, over the real worker: the client
  // sends the entry, holds the queue the answer carried, and Enter spends
  // exactly the total that queue predicted.
  onStandInRun();
  const { client, tick, keys } = await pumped();

  let frame = 1;
  for (; frame <= 4; frame += 1) await tick(frame);
  press(keys, STILL_BINDING);
  await tick(frame);
  frame += 1;
  const arrived = await until(client, tick, 'still', frame);
  frame = arrived + 1;

  const queued = await client.queuePlan({ op: 'cut', route: 1 });
  expect(queued.ok).toBe(true);
  expect(client.queue().entries).toHaveLength(1);
  expect(client.queue().entries[0].cost).toBe(1);
  expect(client.queue().cost_total).toBe(1);
  expect(client.queue().impulse_after).toBe(2);

  // The preview reaches the surface without a step running, because a still run
  // runs none: the frame that follows a queued change carries its snapshot.
  await tick(frame);
  frame += 1;
  const previewed = client.snapshot()?.routes.find((route) => route.route === 1);
  expect(previewed?.status).toBe(1);

  // An entry the core refuses is an answer rather than a queue that grew.
  const refused = await client.queuePlan({ op: 'cut', route: 1 });
  expect(refused.ok).toBe(false);
  expect(client.queue().entries).toHaveLength(1);

  const predicted = client.queue().cost_total;
  press(keys, COMMIT_BINDING);
  await tick(frame);
  frame += 1;
  await tick(frame);
  frame += 1;
  expect(client.mode()).toBe('ramp_out');
  expect(client.queue().entries).toHaveLength(0);
  expect(client.queue().impulse).toBe(3 - predicted);

  const back = await until(client, tick, 'running', frame);
  expect(client.snapshot()?.header.impulse).toBe(3 - predicted);
  expect(client.snapshot()?.routes.some((route) => route.route === 1)).toBe(false);
  expect(back).toBeGreaterThan(frame);
});
