/**
 * The Pulse, from a device to the frame the worker reads.
 *
 * What is under test is the locked half of the interaction: `pulse_held` as a
 * level and `pulse_release` as an edge consumed exactly once, E reaching them
 * through one source, pointer input remaining inspection-only, and the
 * focus-loss rule's safe release — a window that blurs mid-charge drops the
 * charge rather than emitting a Pulse the player never let go of.
 *
 * What the Field does with those frames is not under test here, because no
 * document locks it yet. The report for this goal enumerates what is missing.
 */

import { afterEach, expect, test, vi } from 'vitest';
import {
  neutralFrame,
  PROTOCOL_VERSION,
  type CommandEnvelope,
  type InputFrame,
  type ResponseEnvelope,
} from '../../worker/src/protocol';
import { openPulse, PULSE_BINDINGS, type Pulse } from '../src/shell/pulse';
import { openCore, type CoreClient } from '../src/shell/worker-client';
import type { Steering } from '../src/shell/steering';

const opened: Pulse[] = [];

afterEach(() => {
  for (const pulse of opened.splice(0)) pulse.close();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

/** A Pulse source over an event target of its own. */
function pulse(): { source: Pulse; target: EventTarget } {
  const target = new EventTarget();
  const source = openPulse({ target });
  opened.push(source);
  return { source, target };
}

/** Sends a primary-pointer event. Coupling must ignore it. */
function press(target: EventTarget, which = 0): void {
  target.dispatchEvent(Object.assign(new Event('pointerdown'), { button: which }));
}

/** Presses or releases a key by its code. */
function key(target: EventTarget, code: string, down = true, held: Partial<KeyboardEvent> = {}): void {
  target.dispatchEvent(
    Object.assign(new Event(down ? 'keydown' : 'keyup'), { code, repeat: false, ...held }),
  );
}

/** One frame's worth of the Pulse, as the whole `InputFrame` the pump sends. */
function framed(source: Pulse, seq = 1): InputFrame {
  return { ...neutralFrame(seq, 16_667), ...source.sample() };
}

// ---------------------------------------------------------------------------
// One explicit world-action key
// ---------------------------------------------------------------------------

test('E is the one Coupling binding', () => {
  expect(PULSE_BINDINGS).toEqual(['KeyE']);
});

test('a hold is a level and a release is an edge taken exactly once', () => {
  const { source, target } = pulse();
  expect(source.sample()).toEqual({ pulse_held: false, pulse_release: false });

  key(target, 'KeyE');
  expect(source.sample()).toEqual({ pulse_held: true, pulse_release: false });
  expect(source.sample()).toEqual({ pulse_held: true, pulse_release: false });
  expect(source.held()).toBe(1);

  key(target, 'KeyE', false);
  expect(source.sample()).toEqual({ pulse_held: false, pulse_release: true });
  // The edge is consumed by the one frame that carried it: a second frame does
  // not emit a second Pulse.
  expect(source.sample()).toEqual({ pulse_held: false, pulse_release: false });
  expect(source.held()).toBe(0);
});

test('a press and a release between two frames still carries its edge', () => {
  const { source, target } = pulse();
  key(target, 'KeyE');
  key(target, 'KeyE', false);
  // Nothing was held at either sampled instant, and a Pulse was let go of all
  // the same: the frame says exactly that.
  expect(source.sample()).toEqual({ pulse_held: false, pulse_release: true });
  expect(source.sample()).toEqual({ pulse_held: false, pulse_release: false });
});

test('duplicate keydown events remain one hold and one release', () => {
  const { source, target } = pulse();
  key(target, 'KeyE');
  key(target, 'KeyE');
  expect(source.held()).toBe(1);
  expect(source.sample()).toEqual({ pulse_held: true, pulse_release: false });

  key(target, 'KeyE', false);
  expect(source.sample()).toEqual({ pulse_held: false, pulse_release: true });
});

test('pointer input, Shift, and modified E never hold Coupling', () => {
  const { source, target } = pulse();

  // Both primary and secondary pointer input remain available to inspection.
  press(target);
  press(target, 2);
  expect(source.sample()).toEqual({ pulse_held: false, pulse_release: false });
  expect(source.held()).toBe(0);

  // Movement and the old Shift binding do nothing. A repeated or modified E is
  // also ignored because it is not a fresh world action.
  key(target, 'KeyW');
  key(target, 'ShiftLeft');
  key(target, 'KeyE', true, { repeat: true });
  key(target, 'KeyE', true, { metaKey: true });
  expect(source.held()).toBe(0);
  expect(source.sample()).toEqual({ pulse_held: false, pulse_release: false });
});

// ---------------------------------------------------------------------------
// Letting go
// ---------------------------------------------------------------------------

test('clearing lets a held Pulse go without emitting one', () => {
  const { source, target } = pulse();
  key(target, 'KeyE');
  expect(source.sample().pulse_held).toBe(true);

  source.clear();

  // The safe release: the hold is dropped and no edge is left behind, so the
  // neutral frame the locked focus-loss rule sends is neutral in both fields.
  expect(source.held()).toBe(0);
  expect(source.sample()).toEqual({ pulse_held: false, pulse_release: false });
  expect(source.sample()).toEqual({ pulse_held: false, pulse_release: false });

  // A release that arrives for a hold already dropped emits nothing either:
  // the key going up after the window came back is not a Pulse.
  key(target, 'KeyE', false);
  expect(source.sample()).toEqual({ pulse_held: false, pulse_release: false });

  // And the source still works afterwards.
  key(target, 'KeyE');
  expect(source.sample().pulse_held).toBe(true);
});

test('a pending release stands until the frame that carries it', () => {
  const { source, target } = pulse();
  key(target, 'KeyE');
  key(target, 'KeyE', false);
  key(target, 'KeyE');
  key(target, 'KeyE', false);
  // Two Pulses inside one frame are one edge: the frame carries what the shell
  // holds when it is sent, and no input is buffered beyond the latest frame.
  expect(source.sample()).toEqual({ pulse_held: false, pulse_release: true });
  expect(source.sample()).toEqual({ pulse_held: false, pulse_release: false });
});

// ---------------------------------------------------------------------------
// Determinism
// ---------------------------------------------------------------------------

test('a recorded trace of device events replays to the same frames', () => {
  const trace: ({ key: string; down: boolean } | { pointer: boolean } | { frame: true })[] = [];
  let seed = 11;
  const next = (): number => {
    seed = (seed * 1103515245 + 12345) % 2147483648;
    return seed / 2147483648;
  };
  for (let index = 0; index < 400; index += 1) {
    const roll = next();
    if (roll < 0.2) trace.push({ pointer: true });
    else if (roll < 0.4) trace.push({ key: 'KeyE', down: true });
    else if (roll < 0.6) trace.push({ key: 'KeyE', down: false });
    else trace.push({ frame: true });
  }

  const play = (): string => {
    const { source, target } = pulse();
    const frames: InputFrame[] = [];
    let seq = 1;
    for (const event of trace) {
      if ('key' in event) key(target, event.key, event.down);
      else if ('pointer' in event) press(target);
      else frames.push(framed(source, seq++));
    }
    return JSON.stringify(frames);
  };

  const once = play();
  expect(play()).toBe(once);
  expect(once).toContain('"pulse_release":true');
  expect(once).toContain('"pulse_held":true');
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

/** A steering source that holds nothing, so only the Pulse moves. */
function stillSteering(): Steering {
  return {
    sample: () => ({ steer_x: 0, steer_y: 0 }),
    clear: () => {},
    held: () => 0,
    close: () => {},
  };
}

/** Opens a client over a recorded worker, with the animation frames in hand. */
async function pumped(
  source: Pulse,
): Promise<{ client: CoreClient; tick: (at: number) => void }> {
  RecordingWorker.opened.length = 0;
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

  const client = openCore({ form: 'thread', steering: stillSteering(), pulse: source });
  RecordingWorker.opened[0].answer({
    v: PROTOCOL_VERSION,
    re: 1,
    ok: true,
    body: { protocol: PROTOCOL_VERSION },
  });
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

test('the pump carries one Pulse sample per frame', async () => {
  const { source, target } = pulse();
  const { client, tick } = await pumped(source);

  tick(16.667);
  key(target, 'KeyE');
  tick(33.334);
  tick(50);
  key(target, 'KeyE', false);
  tick(66.7);
  tick(83.4);

  const frames = framesSent();
  expect(frames.map((frame) => [frame.pulse_held, frame.pulse_release])).toEqual([
    [false, false],
    [true, false],
    [true, false],
    [false, true],
    [false, false],
  ]);
  client.close();
});

test('a blurred window drops a held Pulse rather than emitting one', async () => {
  const { source, target } = pulse();
  const { client, tick } = await pumped(source);

  key(target, 'KeyE');
  tick(16.667);
  expect(framesSent()[0].pulse_held).toBe(true);

  window.dispatchEvent(new Event('blur'));

  // The locked focus-loss rule: everything held is let go of and one neutral
  // frame carries the pause level. Neutral in both Pulse fields — the charge is
  // dropped, and no Pulse the player did not release is emitted.
  const frames = framesSent();
  expect(frames).toHaveLength(2);
  expect(frames[1].pause).toBe(true);
  expect(frames[1].pulse_held).toBe(false);
  expect(frames[1].pulse_release).toBe(false);
  expect(source.held()).toBe(0);

  // And the release that arrives once focus is back emits nothing either.
  window.dispatchEvent(new Event('focus'));
  key(target, 'KeyE', false);
  tick(100);
  const after = framesSent();
  expect(after).toHaveLength(3);
  expect(after[2].pulse_held).toBe(false);
  expect(after[2].pulse_release).toBe(false);
  client.close();
});
