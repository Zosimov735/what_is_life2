/**
 * The Pulse, from a device to the frame the worker reads.
 *
 * What is under test is the locked half of the interaction: `pulse_held` as a
 * level and `pulse_release` as an edge consumed exactly once, the primary
 * button and Shift reaching them through one source, and the focus-loss rule's
 * safe release — a window that blurs mid-charge drops the charge rather than
 * emitting a Pulse the player never let go of.
 *
 * What the Field does with those frames is not under test here, because no
 * document locks it yet. The report for this goal enumerates what is missing.
 */

import { afterEach, expect, test, vi } from 'vitest';
import {
  neutralFrame,
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

/** Presses the primary button down. */
function press(target: EventTarget, which = 0): void {
  target.dispatchEvent(Object.assign(new Event('pointerdown'), { button: which }));
}

/** Lets the primary button up. */
function lift(target: EventTarget, which = 0): void {
  target.dispatchEvent(Object.assign(new Event('pointerup'), { button: which }));
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
// One control, whatever named it
// ---------------------------------------------------------------------------

test('the button and Shift produce byte-identical frames for the same Pulse', () => {
  for (const code of PULSE_BINDINGS) {
    const pointer = pulse();
    const keyboard = pulse();

    press(pointer.target);
    key(keyboard.target, code);
    // Three frames of holding, then the release, then a frame with nothing
    // held: the whole of one Pulse, read as the frames that cross.
    for (let seq = 1; seq <= 3; seq += 1) {
      expect(JSON.stringify(framed(keyboard.source, seq))).toBe(
        JSON.stringify(framed(pointer.source, seq)),
      );
    }
    lift(pointer.target);
    key(keyboard.target, code, false);
    expect(JSON.stringify(framed(keyboard.source, 4))).toBe(
      JSON.stringify(framed(pointer.source, 4)),
    );
    expect(JSON.stringify(framed(keyboard.source, 5))).toBe(
      JSON.stringify(framed(pointer.source, 5)),
    );
  }
});

test('a hold is a level and a release is an edge taken exactly once', () => {
  const { source, target } = pulse();
  expect(source.sample()).toEqual({ pulse_held: false, pulse_release: false });

  press(target);
  expect(source.sample()).toEqual({ pulse_held: true, pulse_release: false });
  expect(source.sample()).toEqual({ pulse_held: true, pulse_release: false });
  expect(source.held()).toBe(1);

  lift(target);
  expect(source.sample()).toEqual({ pulse_held: false, pulse_release: true });
  // The edge is consumed by the one frame that carried it: a second frame does
  // not emit a second Pulse.
  expect(source.sample()).toEqual({ pulse_held: false, pulse_release: false });
  expect(source.held()).toBe(0);
});

test('a press and a release between two frames still carries its edge', () => {
  const { source, target } = pulse();
  press(target);
  lift(target);
  // Nothing was held at either sampled instant, and a Pulse was let go of all
  // the same: the frame says exactly that.
  expect(source.sample()).toEqual({ pulse_held: false, pulse_release: true });
  expect(source.sample()).toEqual({ pulse_held: false, pulse_release: false });
});

test('two devices on the Pulse are one hold, and the last one to let go emits', () => {
  const { source, target } = pulse();
  press(target);
  key(target, 'ShiftLeft');
  expect(source.held()).toBe(2);
  expect(source.sample()).toEqual({ pulse_held: true, pulse_release: false });

  lift(target);
  // One device let go and the other is still holding: still charging, and no
  // Pulse yet.
  expect(source.sample()).toEqual({ pulse_held: true, pulse_release: false });
  key(target, 'ShiftLeft', false);
  expect(source.sample()).toEqual({ pulse_held: false, pulse_release: true });
});

test('nothing but the primary button and the bound keys holds the Pulse', () => {
  const { source, target } = pulse();

  // The secondary button: right-click is not used at all.
  press(target, 2);
  expect(source.sample()).toEqual({ pulse_held: false, pulse_release: false });
  expect(source.held()).toBe(0);

  // A key that is not bound, a repeat the platform sends while a key is down,
  // and a key arriving under another modifier — a platform shortcut rather
  // than a Pulse.
  key(target, 'KeyW');
  key(target, 'ShiftLeft', true, { repeat: true });
  key(target, 'ShiftLeft', true, { metaKey: true });
  expect(source.held()).toBe(0);
  expect(source.sample()).toEqual({ pulse_held: false, pulse_release: false });

  // And a secondary button coming up while the primary is held releases
  // nothing: the button named on a release is the one that changed.
  press(target);
  lift(target, 2);
  expect(source.sample()).toEqual({ pulse_held: true, pulse_release: false });
});

test('a cancelled pointer lets the Pulse go', () => {
  const { source, target } = pulse();
  press(target);
  target.dispatchEvent(new Event('pointercancel'));
  expect(source.sample()).toEqual({ pulse_held: false, pulse_release: true });
});

// ---------------------------------------------------------------------------
// Letting go
// ---------------------------------------------------------------------------

test('clearing lets a held Pulse go without emitting one', () => {
  const { source, target } = pulse();
  press(target);
  key(target, 'ShiftLeft');
  expect(source.sample().pulse_held).toBe(true);

  source.clear();

  // The safe release: the hold is dropped and no edge is left behind, so the
  // neutral frame the locked focus-loss rule sends is neutral in both fields.
  expect(source.held()).toBe(0);
  expect(source.sample()).toEqual({ pulse_held: false, pulse_release: false });
  expect(source.sample()).toEqual({ pulse_held: false, pulse_release: false });

  // A release that arrives for a hold already dropped emits nothing either:
  // the key going up after the window came back is not a Pulse.
  key(target, 'ShiftLeft', false);
  expect(source.sample()).toEqual({ pulse_held: false, pulse_release: false });

  // And the source still works afterwards.
  press(target);
  expect(source.sample().pulse_held).toBe(true);
});

test('a pending release stands until the frame that carries it', () => {
  const { source, target } = pulse();
  press(target);
  lift(target);
  press(target);
  lift(target);
  // Two Pulses inside one frame are one edge: the frame carries what the shell
  // holds when it is sent, and no input is buffered beyond the latest frame.
  expect(source.sample()).toEqual({ pulse_held: false, pulse_release: true });
  expect(source.sample()).toEqual({ pulse_held: false, pulse_release: false });
});

// ---------------------------------------------------------------------------
// Determinism
// ---------------------------------------------------------------------------

test('a recorded trace of device events replays to the same frames', () => {
  const trace: ({ down: boolean } | { key: string; down: boolean } | { frame: true })[] = [];
  let seed = 11;
  const next = (): number => {
    seed = (seed * 1103515245 + 12345) % 2147483648;
    return seed / 2147483648;
  };
  for (let index = 0; index < 400; index += 1) {
    const roll = next();
    if (roll < 0.2) trace.push({ down: true });
    else if (roll < 0.4) trace.push({ down: false });
    else if (roll < 0.5) trace.push({ key: 'ShiftLeft', down: true });
    else if (roll < 0.6) trace.push({ key: 'ShiftLeft', down: false });
    else trace.push({ frame: true });
  }

  const play = (): string => {
    const { source, target } = pulse();
    const frames: InputFrame[] = [];
    let seq = 1;
    for (const event of trace) {
      if ('key' in event) key(target, event.key, event.down);
      else if ('down' in event) (event.down ? press : lift)(target);
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

test('the pump carries one Pulse sample per frame', async () => {
  const { source, target } = pulse();
  const { client, tick } = await pumped(source);

  tick(16.667);
  press(target);
  tick(33.334);
  tick(50);
  lift(target);
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

  press(target);
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
  lift(target);
  tick(100);
  const after = framesSent();
  expect(after).toHaveLength(3);
  expect(after[2].pulse_held).toBe(false);
  expect(after[2].pulse_release).toBe(false);
  client.close();
});
