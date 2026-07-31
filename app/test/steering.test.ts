/**
 * Steering, from a device to the frame the worker reads.
 *
 * The contracts under test are the ones `docs/field-framework/ARCHITECTURE.md`
 * locks — the pointer read relative to the surface's middle, the normalization
 * `min(1, r / 320)`, the keyboard's unit axes with their ±23170 diagonal, the
 * clamp that never sends −32768, and the magnitude the vector is held to — and
 * the one this goal is done by: mouse, trackpad, and keyboard produce identical
 * normalized `InputFrame` messages for equivalent logical steering.
 *
 * That last one is read here as byte-identical JSON, on the frame as a whole
 * rather than on the pair inside it, because the frame is what crosses.
 */

import { afterEach, expect, test, vi } from 'vitest';
import { neutralFrame, type CommandEnvelope, type InputFrame, type ResponseEnvelope } from '../../worker/src/protocol';
import { openCore, type CoreClient } from '../src/shell/worker-client';
import {
  KEY_DIAGONAL,
  KEY_FALL_FRAMES,
  KEY_RISE_FRAMES,
  openSteering,
  REST_RADIUS_PX,
  SATURATION_RADIUS_PX,
  steerFrom,
  STEER_UNIT,
  type Steering,
} from '../src/shell/steering';

/** The value the protocol never carries, and the reader refuses. */
const NEVER_SENT = -32_768;

const opened: Steering[] = [];

afterEach(() => {
  for (const steering of opened.splice(0)) steering.close();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

/** A steering source over an event target of its own, with a fixed middle. */
function steering(): { source: Steering; target: EventTarget } {
  const target = new EventTarget();
  const source = openSteering({ target, middle: () => ({ x: 0, y: 0 }) });
  opened.push(source);
  return { source, target };
}

/** Moves the cursor to an offset from the middle. */
function moveTo(target: EventTarget, x: number, y: number): void {
  target.dispatchEvent(Object.assign(new Event('pointermove'), { clientX: x, clientY: y }));
}

/** Presses or releases a key by its code. */
function press(target: EventTarget, code: string, down = true): void {
  target.dispatchEvent(Object.assign(new Event(down ? 'keydown' : 'keyup'), { code, repeat: false }));
}

/** One frame's worth of steering, as the whole `InputFrame` the pump sends. */
function framed(source: Steering, seq = 1): InputFrame {
  return { ...neutralFrame(seq, 16_667), ...source.sample() };
}

/** Samples `count` frames and returns the pair from each. */
function sampled(source: Steering, count: number): { steer_x: number; steer_y: number }[] {
  return Array.from({ length: count }, () => source.sample());
}

// ---------------------------------------------------------------------------
// The locked normalization
// ---------------------------------------------------------------------------

test('the magnitude is the locked fraction of the saturation radius', () => {
  // At the radius and beyond it the control is whole; inside it, the fraction
  // of the way out. `min(1, r / 320)`, and nothing else.
  expect(steerFrom(SATURATION_RADIUS_PX, 0)).toEqual({ steer_x: STEER_UNIT, steer_y: 0 });
  expect(steerFrom(SATURATION_RADIUS_PX * 4, 0)).toEqual({ steer_x: STEER_UNIT, steer_y: 0 });
  expect(steerFrom(-SATURATION_RADIUS_PX, 0)).toEqual({ steer_x: -STEER_UNIT, steer_y: 0 });
  expect(steerFrom(0, SATURATION_RADIUS_PX / 2).steer_y).toBe(Math.round(STEER_UNIT / 2));
  expect(steerFrom(0, SATURATION_RADIUS_PX / 4).steer_y).toBe(Math.round(STEER_UNIT / 4));

  // Inside the rest radius the cursor asks for nothing rather than for a
  // fraction of nothing, and the edge of it is exactly where that stops.
  expect(steerFrom(REST_RADIUS_PX, 0)).toEqual({ steer_x: 0, steer_y: 0 });
  expect(steerFrom(0, 0)).toEqual({ steer_x: 0, steer_y: 0 });
  expect(steerFrom(REST_RADIUS_PX + 1, 0).steer_x).toBeGreaterThan(0);
});

test('no offset produces a vector the frame reader would refuse', () => {
  // The locked invariant: each component inside [−32767, 32767] with −32768
  // never sent, and the vector's own magnitude at most 32767 — the same
  // squared check the core makes of every frame.
  const limit = STEER_UNIT * STEER_UNIT;
  for (let degrees = 0; degrees < 360; degrees += 1) {
    const turn = (degrees * Math.PI) / 180;
    for (const radius of [9, 40, 160, 319, 320, 321, 1000, 5000]) {
      const { steer_x, steer_y } = steerFrom(
        Math.cos(turn) * radius,
        Math.sin(turn) * radius,
      );
      expect(Number.isInteger(steer_x) && Number.isInteger(steer_y)).toBe(true);
      expect(steer_x).toBeGreaterThan(NEVER_SENT);
      expect(steer_y).toBeGreaterThan(NEVER_SENT);
      expect(Math.abs(steer_x)).toBeLessThanOrEqual(STEER_UNIT);
      expect(Math.abs(steer_y)).toBeLessThanOrEqual(STEER_UNIT);
      expect(steer_x * steer_x + steer_y * steer_y).toBeLessThanOrEqual(limit);
    }
  }
});

test('the heading the offset names is the heading the pair carries', () => {
  // Every quadrant, and the axes between them: the sign of each component is
  // the sign of its own axis, and equal offsets give equal components.
  expect(steerFrom(300, 300).steer_x).toBe(steerFrom(300, 300).steer_y);
  expect(steerFrom(-300, 300)).toEqual({
    steer_x: -steerFrom(300, 300).steer_x,
    steer_y: steerFrom(300, 300).steer_y,
  });
  const shallow = steerFrom(200, 50);
  expect(shallow.steer_x).toBeGreaterThan(shallow.steer_y * 3);
});

// ---------------------------------------------------------------------------
// One control, whatever named it
// ---------------------------------------------------------------------------

test('the cursor and the keys produce byte-identical frames for the same steering', () => {
  const cursor = steering();
  const keyboard = steering();

  // Whole deflection along one axis: the cursor at the saturation radius, and
  // a key held until its ramp is full.
  moveTo(cursor.target, SATURATION_RADIUS_PX, 0);
  press(keyboard.target, 'KeyD');
  sampled(keyboard.source, KEY_RISE_FRAMES - 1);
  expect(JSON.stringify(framed(keyboard.source))).toBe(JSON.stringify(framed(cursor.source)));

  // Half of it: the cursor at half the radius, and the key released long
  // enough for the ramp to walk halfway back.
  moveTo(cursor.target, SATURATION_RADIUS_PX / 2, 0);
  press(keyboard.target, 'KeyD', false);
  sampled(keyboard.source, KEY_FALL_FRAMES / 2 - 1);
  expect(JSON.stringify(framed(keyboard.source, 2))).toBe(JSON.stringify(framed(cursor.source, 2)));

  // And the arrow keys are the same keys: the fallback names the same offset.
  const arrows = steering();
  press(arrows.target, 'ArrowRight');
  sampled(arrows.source, KEY_RISE_FRAMES - 1);
  moveTo(cursor.target, SATURATION_RADIUS_PX, 0);
  expect(JSON.stringify(framed(arrows.source, 3))).toBe(JSON.stringify(framed(cursor.source, 3)));
});

test('a diagonal is the locked pair, on either device, held to the magnitude', () => {
  const cursor = steering();
  const keyboard = steering();
  press(keyboard.target, 'KeyD');
  press(keyboard.target, 'KeyS');
  sampled(keyboard.source, KEY_RISE_FRAMES - 1);
  moveTo(cursor.target, SATURATION_RADIUS_PX, SATURATION_RADIUS_PX);

  const held = keyboard.source.sample();
  expect(JSON.stringify(held)).toBe(JSON.stringify(cursor.source.sample()));

  // The document writes the diagonal as ±23169: the largest per-axis pair the
  // magnitude rule admits, because 2 × 23169² fits under 32767² and 2 × 23170²
  // does not. The constant states the rule, and both sides of the comparison
  // that decides it are read here.
  expect(held.steer_x).toBe(KEY_DIAGONAL);
  expect(held.steer_y).toBe(KEY_DIAGONAL);
  expect(KEY_DIAGONAL * KEY_DIAGONAL * 2).toBeLessThanOrEqual(STEER_UNIT * STEER_UNIT);
  expect((KEY_DIAGONAL + 1) * (KEY_DIAGONAL + 1) * 2).toBeGreaterThan(STEER_UNIT * STEER_UNIT);
});

test('the rounding is symmetric about zero, at the half-unit boundary too', () => {
  // Half the saturation radius lands exactly on .5 — 16383.5 — which is where
  // a half-up rounding would give one magnitude steering right and another
  // steering left. Steering that is not sign-symmetric is a defect however
  // small, so the rounding goes away from zero on both sides.
  const right = steerFrom(SATURATION_RADIUS_PX / 2, 0);
  const left = steerFrom(-SATURATION_RADIUS_PX / 2, 0);
  expect(right.steer_x).toBe(16_384);
  expect(left.steer_x).toBe(-16_384);
  for (const reach of [12, 40, 97, 160, 200.5, 319]) {
    const out = steerFrom(reach, reach / 3);
    const back = steerFrom(-reach, -reach / 3);
    expect(back.steer_x).toBe(-out.steer_x);
    expect(back.steer_y).toBe(-out.steer_y);
  }
});

test('the rest radius is a hard edge, and the step across it is stated', () => {
  // The locked normalization is `min(1, r / 320)` and nothing rescales it, so
  // the control steps from nothing to its value at the radius rather than
  // easing in. The step is deliberate: rescaling the remaining range would be a
  // different rule from the locked one.
  expect(steerFrom(7.9, 0)).toEqual({ steer_x: 0, steer_y: 0 });
  const across = steerFrom(8.1, 0);
  expect(across.steer_x).toBe(829);
  // What that comes to at the far end: a fortieth of the speed range, which is
  // half a unit per step, and the spring takes four steps to reach even that.
  expect(across.steer_x / STEER_UNIT).toBeLessThan(1 / 39);
});

test('opposed keys cancel and the cursor is what is left', () => {
  const { source, target } = steering();
  press(target, 'KeyA');
  press(target, 'KeyD');
  expect(source.sample()).toEqual({ steer_x: 0, steer_y: 0 });
  expect(source.held()).toBe(2);

  moveTo(target, 0, SATURATION_RADIUS_PX);
  expect(source.sample()).toEqual({ steer_x: 0, steer_y: STEER_UNIT });
});

// ---------------------------------------------------------------------------
// The ramp
// ---------------------------------------------------------------------------

test('a held key ramps up over its locked span and a released one decays', () => {
  const { source, target } = steering();
  press(target, 'KeyW');

  const rising = sampled(source, KEY_RISE_FRAMES).map((pair) => Math.abs(pair.steer_y));
  expect(rising[0]).toBe(Math.round(STEER_UNIT / KEY_RISE_FRAMES));
  for (let frame = 1; frame < rising.length; frame += 1) {
    expect(rising[frame]).toBeGreaterThan(rising[frame - 1]);
  }
  expect(rising[KEY_RISE_FRAMES - 1]).toBe(STEER_UNIT);
  // And it stays there rather than climbing past the saturation radius.
  expect(sampled(source, 4).every((pair) => pair.steer_y === -STEER_UNIT)).toBe(true);

  press(target, 'KeyW', false);
  const falling = sampled(source, KEY_FALL_FRAMES).map((pair) => Math.abs(pair.steer_y));
  // Not a snap: the first frame after the release still carries most of it.
  expect(falling[0] * 10).toBeGreaterThan(STEER_UNIT * 8);
  for (let frame = 1; frame < falling.length; frame += 1) {
    expect(falling[frame]).toBeLessThan(falling[frame - 1]);
  }
  expect(falling[KEY_FALL_FRAMES - 1]).toBe(0);
  expect(source.held()).toBe(0);
});

test('a ramp advances once per frame and not once per event', () => {
  // The ramp is the shell's only state between frames, and it moves only when
  // a frame is emitted: a recorded trace of device events therefore replays to
  // the same frames however the events were spaced.
  const { source, target } = steering();
  press(target, 'KeyD');
  for (let repeat = 0; repeat < 20; repeat += 1) {
    target.dispatchEvent(Object.assign(new Event('keydown'), { code: 'KeyD', repeat: true }));
    moveTo(target, 0, 0);
  }
  expect(source.sample()).toEqual({
    steer_x: Math.round(STEER_UNIT / KEY_RISE_FRAMES),
    steer_y: 0,
  });
});

// ---------------------------------------------------------------------------
// Letting go
// ---------------------------------------------------------------------------

test('clearing lets go of the cursor, the keys, and the ramp between them', () => {
  const { source, target } = steering();
  moveTo(target, SATURATION_RADIUS_PX, SATURATION_RADIUS_PX);
  press(target, 'KeyW');
  sampled(source, KEY_RISE_FRAMES);
  expect(source.sample()).not.toEqual({ steer_x: 0, steer_y: 0 });

  source.clear();

  expect(source.held()).toBe(0);
  expect(source.sample()).toEqual({ steer_x: 0, steer_y: 0 });
  // Nothing decays back in from what was held: the ramp went with the keys.
  expect(sampled(source, 8).every((pair) => pair.steer_x === 0 && pair.steer_y === 0)).toBe(true);
  // And the source still works afterwards.
  press(target, 'KeyD');
  expect(source.sample().steer_x).toBeGreaterThan(0);
});

test('a cursor that leaves the window stops steering and the keys carry on', () => {
  const { source, target } = steering();
  moveTo(target, SATURATION_RADIUS_PX, 0);
  press(target, 'KeyW');
  sampled(source, KEY_RISE_FRAMES);
  expect(source.sample().steer_x).toBeGreaterThan(0);

  document.dispatchEvent(new Event('pointerleave'));

  // The offset the cursor left behind would have steered for as long as it
  // stayed away. The keys are untouched: the window still has focus and still
  // receives their release, so none of them can be left held.
  const after = source.sample();
  expect(after.steer_x).toBe(0);
  expect(after.steer_y).toBe(-STEER_UNIT);
  expect(source.held()).toBe(1);
});

// ---------------------------------------------------------------------------
// Determinism
// ---------------------------------------------------------------------------

test('a recorded trace of device events replays to the same frames', () => {
  // The mapping reads no clock and keeps no state a second run would not
  // rebuild: the same events in the same order against the same frame
  // boundaries give the same frames, byte for byte.
  const trace: (
    | { move: [number, number] }
    | { down: string }
    | { up: string }
    | { frame: true }
  )[] = [];
  let seed = 7;
  const next = (): number => {
    seed = (seed * 1103515245 + 12345) % 2147483648;
    return seed / 2147483648;
  };
  for (let step = 0; step < 400; step += 1) {
    const roll = next();
    if (roll < 0.35) trace.push({ move: [next() * 900 - 450, next() * 900 - 450] });
    else if (roll < 0.45) trace.push({ down: ['KeyW', 'KeyA', 'KeyS', 'KeyD'][Math.floor(next() * 4)] });
    else if (roll < 0.55) trace.push({ up: ['KeyW', 'KeyA', 'KeyS', 'KeyD'][Math.floor(next() * 4)] });
    else trace.push({ frame: true });
  }

  const play = (): string => {
    const { source, target } = steering();
    const frames: InputFrame[] = [];
    let seq = 1;
    for (const event of trace) {
      if ('move' in event) moveTo(target, event.move[0], event.move[1]);
      else if ('down' in event) press(target, event.down);
      else if ('up' in event) press(target, event.up, false);
      else frames.push(framed(source, seq++));
    }
    return JSON.stringify(frames);
  };

  const once = play();
  expect(play()).toBe(once);
  expect(once).toContain('"steer_x":');
});

// ---------------------------------------------------------------------------
// Into the frames the pump sends
// ---------------------------------------------------------------------------

/** A worker that records what it was sent and answers when the test says so. */
class RecordingWorker {
  static opened: RecordingWorker[] = [];
  onmessage: ((message: MessageEvent<ResponseEnvelope>) => void) | null = null;
  onerror: ((failure: unknown) => void) | null = null;
  readonly sent: CommandEnvelope[] = [];

  constructor() {
    RecordingWorker.opened.push(this);
  }

  postMessage(command: CommandEnvelope): void {
    this.sent.push(command);
  }

  terminate(): void {}

  answer(response: ResponseEnvelope): void {
    this.onmessage?.({ data: response } as MessageEvent<ResponseEnvelope>);
  }
}

/** A steering source a test drives directly, in place of the devices. */
function scripted(pairs: { steer_x: number; steer_y: number }[]): Steering & { cleared: number } {
  let at = 0;
  const source = {
    cleared: 0,
    sample: () => pairs[Math.min(at++, pairs.length - 1)],
    clear() {
      source.cleared += 1;
    },
    held: () => 0,
    close: () => {},
  };
  return source;
}

/** Opens a client over a recorded worker, with the animation frames in hand. */
async function pumped(steer: Steering): Promise<{ client: CoreClient; tick: (at: number) => void }> {
  RecordingWorker.opened.length = 0;
  // Animation frames the test hands out one at a time, and a cancellation that
  // really cancels — the pump stopping is half of what the pause rule is.
  const waiting = new Map<number, FrameRequestCallback>();
  let handle = 0;
  vi.stubGlobal('requestAnimationFrame', (run: FrameRequestCallback) => {
    handle += 1;
    waiting.set(handle, run);
    return handle;
  });
  vi.stubGlobal('cancelAnimationFrame', (held: number) => waiting.delete(held));
  vi.stubGlobal('Worker', RecordingWorker);
  vi.spyOn(console, 'info').mockImplementation(() => {});

  const client = openCore({ form: 'thread', steering: steer });
  RecordingWorker.opened[0].answer({ v: 1, re: 1, ok: true, body: { protocol: 1 } });
  await client.ready;
  return {
    client,
    tick(at: number) {
      const due = [...waiting.values()];
      waiting.clear();
      for (const run of due) run(at);
    },
  };
}

/** The frames one worker was sent. */
function framesSent(): InputFrame[] {
  return RecordingWorker.opened[0].sent
    .filter((command) => command.cmd === 'input_frame')
    .map((command) => command.body as unknown as InputFrame);
}

test('the pump carries one steering sample per frame, and numbers them', async () => {
  const steer = scripted([
    { steer_x: 12_000, steer_y: -4_000 },
    { steer_x: 32_767, steer_y: 0 },
    { steer_x: 0, steer_y: 0 },
  ]);
  const { client, tick } = await pumped(steer);

  tick(16.667);
  tick(33.334);
  tick(50);

  const frames = framesSent();
  expect(frames).toHaveLength(3);
  expect(frames.map((frame) => [frame.steer_x, frame.steer_y])).toEqual([
    [12_000, -4_000],
    [32_767, 0],
    [0, 0],
  ]);
  expect(frames.map((frame) => frame.seq)).toEqual([1, 2, 3]);
  expect(frames.every((frame) => frame.pause === false)).toBe(true);
  // The timestamp is the animation frame's own, in whole microseconds.
  expect(frames[1].t_us).toBe(33_334);
  client.close();
});

/** Reports the document hidden for as long as the returned handle stands. */
function hidden(): { release: () => void } {
  const held = Object.getOwnPropertyDescriptor(Document.prototype, 'visibilityState');
  Object.defineProperty(document, 'visibilityState', {
    configurable: true,
    get: () => 'hidden',
  });
  return {
    release() {
      delete (document as unknown as Record<string, unknown>).visibilityState;
      if (held) Object.defineProperty(Document.prototype, 'visibilityState', held);
    },
  };
}

test('a hidden tab lets go of everything and sends one neutral paused frame', async () => {
  const steer = scripted([{ steer_x: 0, steer_y: -32_767 }]);
  const { client, tick } = await pumped(steer);

  tick(16.667);
  expect(framesSent()).toHaveLength(1);

  const held = hidden();
  document.dispatchEvent(new Event('visibilitychange'));

  // The same locked rule as blur, from the other trigger: everything held is
  // let go of, one neutral frame carries the pause level, and the pump stops.
  expect(steer.cleared).toBeGreaterThan(0);
  const frames = framesSent();
  expect(frames).toHaveLength(2);
  expect(frames[1].pause).toBe(true);
  expect([frames[1].steer_x, frames[1].steer_y]).toEqual([0, 0]);
  tick(50);
  expect(framesSent()).toHaveLength(2);

  held.release();
  document.dispatchEvent(new Event('visibilitychange'));
  tick(66.7);
  const after = framesSent();
  expect(after).toHaveLength(3);
  expect(after[2].pause).toBe(false);
  client.close();
});

test('a window that blurs while already hidden still lets go of what it holds', async () => {
  const steer = scripted([{ steer_x: 32_767, steer_y: 0 }]);
  const { client, tick } = await pumped(steer);
  tick(16.667);

  const held = hidden();
  document.dispatchEvent(new Event('visibilitychange'));
  const clearedWhenHidden = steer.cleared;
  const framesWhenHidden = framesSent().length;
  expect(clearedWhenHidden).toBeGreaterThan(0);

  // The pause level is already held, so the pause path returns without doing
  // anything — which is exactly why the clear does not live inside it. A key
  // pressed between the two events would otherwise still be held on return.
  window.dispatchEvent(new Event('blur'));

  expect(steer.cleared).toBeGreaterThan(clearedWhenHidden);
  expect(framesSent()).toHaveLength(framesWhenHidden);
  held.release();
  client.close();
});

test('a blurred window lets go of everything and sends one neutral paused frame', async () => {
  const steer = scripted([{ steer_x: 32_767, steer_y: 0 }]);
  const { client, tick } = await pumped(steer);

  tick(16.667);
  expect(framesSent()).toHaveLength(1);

  window.dispatchEvent(new Event('blur'));

  // Every held input is let go of before the frame is built, so the frame the
  // locked rule sends is neutral because nothing is held rather than because
  // the field was overwritten.
  expect(steer.cleared).toBeGreaterThan(0);
  const frames = framesSent();
  expect(frames).toHaveLength(2);
  expect(frames[1].pause).toBe(true);
  expect([frames[1].steer_x, frames[1].steer_y]).toEqual([0, 0]);
  expect(frames[1].pulse_held).toBe(false);
  expect(frames[1].pulse_release).toBe(false);

  // And the pump has stopped: steering while paused sends nothing at all.
  tick(50);
  tick(66.7);
  expect(framesSent()).toHaveLength(2);

  // Focus returns and the pump comes back, numbering on from where it was.
  window.dispatchEvent(new Event('focus'));
  tick(83.4);
  const after = framesSent();
  expect(after).toHaveLength(3);
  expect(after[2].pause).toBe(false);
  expect(after[2].seq).toBe(3);
  client.close();
});
