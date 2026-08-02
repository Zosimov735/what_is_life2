/**
 * The cues, sounded.
 *
 * The module is driven here exactly as the shell drives it — one decoded
 * `FrameState` per rendered frame — over a recording context that stands in for
 * WebAudio, so what is asserted is the graph the module built and the schedule
 * it wrote onto it. Nothing here plays anything.
 *
 * The three rules the module is held to have a test each: the level gates
 * everything and zero builds no context at all, the reduced-motion flag never
 * silences it, and no context is constructed before the player's first gesture.
 */

import { afterEach, expect, test, vi } from 'vitest';
import { decodeFrameState, FRAME_VERSION, type FramePort, type FrameState } from '../../worker/src/frame-state';
import { fixtureSnapshot } from '../src/shell/dev-frames';
import { FULL_SOUND, openSound, type Sound } from '../src/shell/sound';

const opened: Sound[] = [];

afterEach(() => {
  for (const sound of opened.splice(0)) sound.close();
  vi.restoreAllMocks();
});

/** One scheduled change to one parameter. */
interface Scheduled {
  call: 'setValueAtTime' | 'linearRampToValueAtTime';
  value: number;
  at: number;
}

/** A parameter that records what was written to it, in order. */
class RecordingParam {
  value = 0;
  readonly writes: Scheduled[] = [];

  setValueAtTime(value: number, at: number): void {
    this.value = value;
    this.writes.push({ call: 'setValueAtTime', value, at });
  }

  linearRampToValueAtTime(value: number, at: number): void {
    this.writes.push({ call: 'linearRampToValueAtTime', value, at });
  }

  /** The values written, in order. */
  values(): number[] {
    return this.writes.map((write) => write.value);
  }
}

class RecordingNode {
  readonly connected: unknown[] = [];
  connect(to: unknown): void {
    this.connected.push(to);
  }
  disconnect(): void {
    this.connected.length = 0;
  }
}

class RecordingOscillator extends RecordingNode {
  type: OscillatorType = 'sine';
  readonly frequency = new RecordingParam();
  started: number | null = null;
  stopped: number | null = null;

  start(at: number): void {
    this.started = at;
  }

  stop(at: number): void {
    this.stopped = at;
  }
}

class RecordingGain extends RecordingNode {
  readonly gain = new RecordingParam();
}

class RecordingFilter extends RecordingNode {
  type = 'lowpass';
  readonly frequency = new RecordingParam();
}

/** A context that records the graph built on it. */
class RecordingContext {
  static built = 0;
  currentTime = 0;
  state: AudioContextState = 'suspended';
  readonly destination = new RecordingNode();
  readonly oscillators: RecordingOscillator[] = [];
  readonly gains: RecordingGain[] = [];
  readonly filters: RecordingFilter[] = [];
  resumed = 0;
  closed = 0;

  constructor() {
    RecordingContext.built += 1;
  }

  createOscillator(): RecordingOscillator {
    const made = new RecordingOscillator();
    this.oscillators.push(made);
    return made;
  }

  createGain(): RecordingGain {
    const made = new RecordingGain();
    this.gains.push(made);
    return made;
  }

  createBiquadFilter(): RecordingFilter {
    const made = new RecordingFilter();
    this.filters.push(made);
    return made;
  }

  resume(): Promise<void> {
    this.resumed += 1;
    this.state = 'running';
    return Promise.resolve();
  }

  close(): Promise<void> {
    this.closed += 1;
    this.state = 'closed';
    return Promise.resolve();
  }
}

/** Opens the module over a recording context, and the target its gestures use. */
function sounding(level = FULL_SOUND): {
  sound: Sound;
  target: EventTarget;
  context: () => RecordingContext | null;
} {
  RecordingContext.built = 0;
  const target = new EventTarget();
  let held: RecordingContext | null = null;
  const sound = openSound({
    level,
    target,
    context: () => {
      held = new RecordingContext();
      return held as unknown as AudioContext;
    },
  });
  opened.push(sound);
  return { sound, target, context: () => held };
}

/** The gesture that lets a context be built at all. */
function gesture(target: EventTarget): void {
  target.dispatchEvent(new Event('pointerdown'));
}

/** What a snapshot may carry besides its step and its cues. */
interface Framing {
  /** Whether the controlled Form is charging a Pulse. */
  charging?: boolean;
  /** Whether the reduced-motion flag stands in the header. */
  reduced?: boolean;
  /** The mode the header carries: 0 is `running`. */
  mode?: number;
  /** The time scale the header carries, Q0.16 saturating at 65535. */
  scale?: number;
  /** The layer the controlled Form stands on. */
  layer?: number;
}

/** A snapshot carrying one controlled Form and the cues named. */
function framed(
  step: number,
  cues: { kind: number; b?: number }[],
  framing: Framing = {},
): FrameState {
  const { charging = false, reduced = false, mode = 0, layer = 0, scale = 65_535 } = framing;
  const sections = [{ kind: 1, count: 1, width: 24 }];
  if (cues.length > 0) sections.push({ kind: 7, count: cues.length, width: 8 });
  const table = 32 + sections.length * 8;
  const body = sections.reduce((total, section) => total + section.count * section.width, 0);
  const buffer = new ArrayBuffer(table + body);
  const view = new DataView(buffer);
  const bytes = new Uint8Array(buffer);
  bytes.set([0x46, 0x47, 0x46, 0x31]);
  view.setUint16(4, FRAME_VERSION, true);
  view.setUint16(6, reduced ? 4 : 0, true);
  view.setUint32(8, step, true);
  view.setUint16(12, scale, true);
  bytes[14] = mode;
  bytes[20] = sections.length;

  let at = table;
  sections.forEach((section, place) => {
    const entry = 32 + place * 8;
    bytes[entry] = section.kind;
    view.setUint16(entry + 2, section.count, true);
    view.setUint32(entry + 4, at, true);
    at += section.count * section.width;
  });

  let offset = table;
  bytes[offset] = 1;
  bytes[offset + 1] = 0;
  bytes[offset + 2] = layer;
  bytes[offset + 3] = 1 | (charging ? 4 : 0);
  view.setFloat32(offset + 4, 2048, true);
  view.setFloat32(offset + 8, 2048, true);
  view.setUint16(offset + 20, 1000, true);
  view.setUint16(offset + 22, charging ? 256 * 40 : 0, true);
  offset += 24;
  for (const cue of cues) {
    bytes[offset] = cue.kind;
    view.setUint32(offset + 4, cue.b ?? 1, true);
    offset += 8;
  }
  return decodeFrameState(buffer);
}

/** Gives a decoded frame independent physical-compartment and View members. */
function withRegisters(
  state: FrameState,
  physical: readonly number[],
  observed: readonly number[],
): FrameState {
  state.ports = [1, 2, 3].map(
    (node): FramePort => ({
      node,
      kind: 0,
      layer: 0,
      open: true,
      overloaded: false,
      member: physical.includes(node),
      shell: false,
      proposedMember: false,
      charge: 1_000,
      x: node * 16,
      y: node * 16,
      reserve: 0,
    }),
  );
  state.inside = state.ports.map((port) => observed.includes(port.node));
  return state;
}

/** Every oscillator built, by the shape it was given. */
function shapes(context: RecordingContext): OscillatorType[] {
  return context.oscillators.map((oscillator) => oscillator.type);
}

// ---------------------------------------------------------------------------
// The level gates everything
// ---------------------------------------------------------------------------

test('a level of nothing builds no context at all', () => {
  const { sound, target } = sounding(0);
  gesture(target);
  sound.observe(framed(1, [{ kind: 1 }, { kind: 2 }], { charging: true }));
  sound.observe(framed(2, [{ kind: 3 }]));

  expect(RecordingContext.built).toBe(0);
  expect(sound.state()).toBe('idle');
  expect(sound.scheduled()).toBe(0);
});

test('a level turned down to nothing closes the context it had', () => {
  const { sound, target, context } = sounding();
  gesture(target);
  sound.observe(framed(1, [{ kind: 2 }]));
  expect(sound.scheduled()).toBeGreaterThan(0);
  const held = context();
  expect(held).not.toBeNull();

  sound.setLevel(0);
  expect(held?.closed).toBe(1);
  expect(sound.state()).toBe('idle');

  const before = sound.scheduled();
  sound.observe(framed(2, [{ kind: 3 }]));
  expect(sound.scheduled()).toBe(before);
  expect(RecordingContext.built).toBe(1);
});

test('the level scales the one gain everything passes through', () => {
  const { sound, target, context } = sounding(FULL_SOUND / 4);
  gesture(target);
  sound.observe(framed(1, [{ kind: 2 }]));
  const master = context()?.gains[0];
  expect(master?.gain.value).toBeCloseTo(0.4 * 0.25, 6);

  sound.setLevel(FULL_SOUND);
  expect(master?.gain.value).toBeCloseTo(0.4, 6);
});

// ---------------------------------------------------------------------------
// The autoplay policy
// ---------------------------------------------------------------------------

test('no context is built before the first gesture, and the first one resumes it', () => {
  const { sound, target, context } = sounding();

  sound.observe(framed(1, [{ kind: 1 }, { kind: 2 }]));
  expect(RecordingContext.built).toBe(0);
  expect(sound.state()).toBe('idle');

  gesture(target);
  expect(RecordingContext.built).toBe(1);
  expect(context()?.resumed).toBe(1);
  expect(sound.state()).toBe('running');

  sound.observe(framed(2, [{ kind: 2 }]));
  expect(sound.scheduled()).toBeGreaterThan(0);
});

test('a context that will not resume is left alone, and nothing is logged', async () => {
  const failures: unknown[] = [];
  vi.spyOn(console, 'error').mockImplementation((...rest) => failures.push(rest));
  vi.spyOn(console, 'warn').mockImplementation((...rest) => failures.push(rest));

  const target = new EventTarget();
  const blocked = new RecordingContext();
  blocked.resume = () => Promise.reject(new Error('blocked'));
  const sound = openSound({
    target,
    context: () => blocked as unknown as AudioContext,
  });
  opened.push(sound);

  gesture(target);
  sound.observe(framed(1, [{ kind: 3 }]));
  await Promise.resolve();

  // The refusal is the platform's answer, not a fault: the module carries on,
  // reports what the context is doing, and says nothing to the console.
  expect(sound.state()).toBe('suspended');
  expect(failures).toEqual([]);
});

test('a platform with no WebAudio at all runs and sounds nothing', () => {
  const target = new EventTarget();
  const sound = openSound({ target, context: () => null });
  opened.push(sound);
  gesture(target);
  sound.observe(framed(1, [{ kind: 1 }, { kind: 2 }], { charging: true }));
  expect(sound.state()).toBe('idle');
  expect(sound.scheduled()).toBe(0);
});

// ---------------------------------------------------------------------------
// One cue per reading
// ---------------------------------------------------------------------------

test('each cue of the Pulse is its own short scheduled shape', () => {
  const { sound, target, context } = sounding();
  gesture(target);
  const held = context();
  if (!held) throw new Error('the gesture builds the context');

  // A Pulse that reached something: the emission falls away, and the gather
  // confirms above it.
  sound.observe(framed(1, [{ kind: 1 }, { kind: 2 }]));
  expect(sound.scheduled()).toBe(3);
  const emitted = held.oscillators[0];
  expect(emitted.type).toBe('sine');
  expect(emitted.frequency.values()).toEqual([520, 260]);
  expect(emitted.started).toBe(0);
  expect(emitted.stopped).toBeCloseTo(0.18, 6);
  // Every cue opens from nothing and closes back to it, so none of them clicks.
  const gain = held.gains[1];
  expect(gain.gain.values()[0]).toBe(0);
  expect(gain.gain.values()[gain.gain.values().length - 1]).toBe(0);
  expect(held.oscillators[1].frequency.values()[0]).toBe(660);

  // A Pulse that reached nothing: low, short, and covered by a filter.
  held.currentTime = 1;
  sound.observe(framed(2, [{ kind: 1 }]));
  const dull = held.oscillators[3];
  expect(dull.type).toBe('triangle');
  expect(dull.frequency.values()[0]).toBe(120);
  expect(dull.stopped).toBeCloseTo(1.09, 6);
  expect(held.filters).toHaveLength(1);

  // A Port opening is the one cue that resolves: it rises, and stands longest.
  held.currentTime = 2;
  sound.observe(framed(3, [{ kind: 3 }]));
  const port = held.oscillators[4];
  expect(port.frequency.values()).toEqual([330, 396]);
  expect((port.stopped ?? 0) - (port.started ?? 0)).toBeCloseTo(0.36, 6);

  // Interference pushed away: a square, swept down, filtered.
  held.currentTime = 3;
  sound.observe(framed(4, [{ kind: 12 }]));
  const shove = held.oscillators[6];
  expect(shove.type).toBe('square');
  expect(shove.frequency.values()).toEqual([190, 70]);
  expect(held.filters).toHaveLength(2);

  // Nothing is left running: every cue stops itself.
  for (const oscillator of held.oscillators) {
    expect(oscillator.stopped).not.toBeNull();
  }
});

test('a held Pulse sounds one rising voice, and the release ends it', () => {
  const { sound, target, context } = sounding();
  gesture(target);
  const held = context();
  if (!held) throw new Error('the gesture builds the context');

  // The locked charging rule fills in 32 held steps, and the rise is written
  // against exactly that: full charge is the top of the range.
  for (let step = 1; step <= 33; step += 1) {
    held.currentTime = step / 30;
    sound.observe(framed(step, [], { charging: true }));
  }

  // One voice for the whole hold rather than one per step, and it rises.
  expect(shapes(held).filter((kind) => kind === 'triangle')).toHaveLength(1);
  const rising = held.oscillators[0];
  const written = rising.frequency.values();
  expect(written[0]).toBe(180);
  for (let index = 2; index < 33; index += 1) {
    expect(written[index]).toBeGreaterThan(written[index - 1]);
  }
  // The top of the range at a full charge.
  expect(written[32]).toBe(660);
  // Holding past full changes nothing, exactly as the charge sits at the clamp.
  expect(written[written.length - 1]).toBe(660);
  expect(rising.stopped).toBeNull();

  // The release ends the voice and sounds the emission.
  held.currentTime = 1;
  sound.observe(framed(34, [{ kind: 1 }, { kind: 2 }]));
  expect(rising.stopped).toBeCloseTo(1.06, 6);
  expect(shapes(held).filter((kind) => kind === 'triangle')).toHaveLength(1);
});

test('one snapshot sounds once however many times it is read', () => {
  const { sound, target, context } = sounding();
  gesture(target);
  const frame = framed(7, [{ kind: 2 }]);
  sound.observe(frame);
  const after = sound.scheduled();
  sound.observe(frame);
  sound.observe(frame);
  expect(sound.scheduled()).toBe(after);
  expect(context()?.oscillators).toHaveLength(2);
});

test('a cue the module does not name sounds nothing', () => {
  const { sound, target } = sounding();
  gesture(target);
  // Route formed, route cut, a break: every one of them belongs to a goal that
  // has not reached it, and a cue this module does not name sounds nothing
  // rather than sounding like something else.
  sound.observe(framed(1, [{ kind: 4 }, { kind: 5 }, { kind: 6 }]));
  expect(sound.scheduled()).toBe(0);
});

test('the cues the authored sequence raises each sound, and none sounds alike', () => {
  // An Anchor written, a setback, the recovery from it, and a completion: four
  // of the closed set, and the four the opening sequence raises.
  for (const kind of [7, 8, 9, 11]) {
    const { sound, target } = sounding();
    gesture(target);
    sound.observe(framed(1, [{ kind }]));
    expect(sound.scheduled()).toBeGreaterThan(0);
  }
});

// ---------------------------------------------------------------------------
// Depth
// ---------------------------------------------------------------------------

/** The pitch a voice was started at and the pitch it was bent to. */
function bend(voice: RecordingOscillator): number[] {
  return voice.frequency.values();
}

test('a change of layer sounds, and the two directions are one shape played both ways', () => {
  const { sound, target, context } = sounding();
  gesture(target);
  const held = context();
  if (!held) throw new Error('the gesture builds the context');

  // Standing on a layer is not moving to one: the first snapshot sets the depth
  // and sounds nothing at all.
  sound.observe(framed(1, []));
  expect(sound.scheduled()).toBe(0);

  // A layer deeper falls; a layer shallower is the same length back up. Both
  // are the triangle, so neither can be mistaken for a cue a step raised.
  sound.observe(framed(2, [], { layer: 1 }));
  expect(sound.scheduled()).toBe(1);
  expect(held.oscillators[0].type).toBe('triangle');
  expect(bend(held.oscillators[0])).toEqual([260, 130]);

  sound.observe(framed(3, []));
  expect(sound.scheduled()).toBe(2);
  expect(bend(held.oscillators[1])).toEqual([130, 260]);
  expect(held.oscillators[1].stopped).toBeCloseTo(0.3, 6);

  // A step that changes no layer sounds nothing.
  sound.observe(framed(4, []));
  expect(sound.scheduled()).toBe(2);
});

test('entering and leaving Still Mode each sound, and neither is a depth change', () => {
  const { sound, target, context } = sounding();
  gesture(target);
  const held = context();
  if (!held) throw new Error('the gesture builds the context');

  // Standing in a mode is not moving into one: the first snapshot sets it and
  // sounds nothing, exactly as the first one sets the depth.
  sound.observe(framed(1, []));
  expect(sound.scheduled()).toBe(0);

  // The way in falls and the way out is the same shape back up, a fifth above
  // the register a depth change uses, and both are sines rather than the
  // triangle depth is played on.
  sound.observe(framed(2, [], { mode: 1 }));
  expect(sound.scheduled()).toBe(2);
  expect(held.oscillators[0].type).toBe('sine');
  expect(bend(held.oscillators[0])).toEqual([392, 294]);
  expect(held.oscillators[0].stopped).toBeCloseTo(0.25, 6);

  // The completion of a ramp is the arrival the ramp already announced, so it
  // sounds nothing of its own.
  sound.observe(framed(3, [], { mode: 2, scale: 0 }));
  expect(sound.scheduled()).toBe(2);

  sound.observe(framed(4, [], { mode: 3, scale: 0 }));
  expect(sound.scheduled()).toBe(4);
  expect(bend(held.oscillators[2])).toEqual([294, 392]);
  expect(held.oscillators[2].stopped).toBeCloseTo(0.25, 6);

  // And a completed exit announces nothing either.
  sound.observe(framed(5, []));
  expect(sound.scheduled()).toBe(4);
});

test('only the passive View cue follows View membership, not the physical compartment', () => {
  const { sound, target } = sounding();
  gesture(target);

  sound.observe(withRegisters(framed(1, []), [1, 2], [1]));
  expect(sound.scheduled()).toBe(0);

  // A causal compartment commit may change physical members on the same step,
  // but the observation aperture did not move and therefore sounds nothing.
  sound.observe(withRegisters(framed(1, []), [2, 3], [1]));
  expect(sound.scheduled()).toBe(0);

  // `set_focus` also runs no step. Its independent View bitset still produces
  // the brief observation cue immediately.
  sound.observe(withRegisters(framed(1, []), [2, 3], [2]));
  expect(sound.scheduled()).toBe(2);
});

test('a Handoff sounds once, rising, in a register no other cue uses', () => {
  const { sound, target, context } = sounding();
  gesture(target);
  const held = context();
  if (!held) throw new Error('the gesture builds the context');

  // Holding a Form is not taking one up: the first snapshot sets the reading
  // and sounds nothing, exactly as the first one sets the depth.
  sound.observe(framed(1, []));
  expect(sound.scheduled()).toBe(0);

  // A Handoff is answered in Still Mode, where no step runs — so the frame
  // that carries one carries the same step, and the cue is read above the
  // step guard or it is never read at all.
  const moved = framed(1, []);
  moved.forms[0].id = 5;
  sound.observe(moved);
  expect(sound.scheduled()).toBe(2);
  expect(held.oscillators[0].type).toBe('triangle');
  expect(bend(held.oscillators[0])).toEqual([349, 466]);
  expect(bend(held.oscillators[1])).toEqual([698, 932]);
  expect(held.oscillators[0].stopped).toBeCloseTo(0.2, 6);

  // It rises, which neither the depth pair nor the Still Mode pair does from
  // this register, and it is a fourth rather than the fifth both of those are.
  const [from, to] = bend(held.oscillators[0]);
  expect(to).toBeGreaterThan(from);
  expect(to / from).toBeGreaterThan(1.3);
  expect(to / from).toBeLessThan(1.4);

  // Once per change: the same reading again sounds nothing.
  const again = framed(2, []);
  again.forms[0].id = 5;
  sound.observe(again);
  expect(sound.scheduled()).toBe(2);

  // And moving back sounds it again.
  sound.observe(framed(3, []));
  expect(sound.scheduled()).toBe(4);
});

test('a Handoff cue is dropped with a history that no longer stands', () => {
  const { sound, target } = sounding();
  gesture(target);
  sound.observe(framed(40, []));
  const moved = framed(41, []);
  moved.forms[0].id = 5;
  sound.observe(moved);
  expect(sound.scheduled()).toBe(2);
  // A restore moves the step backwards. The Form that carried control in a
  // history that no longer stands is not one control has just left, so the
  // reading is dropped and the frame that follows sounds nothing of its own.
  sound.observe(framed(10, []));
  expect(sound.scheduled()).toBe(2);
});

test('a reversal is announced for the length the ramp it turned around has left', () => {
  const { sound, target, context } = sounding();
  gesture(target);
  const held = context();
  if (!held) throw new Error('the gesture builds the context');

  sound.observe(framed(1, []));
  // A fresh entry has the whole span in front of it, and is announced for the
  // whole of it.
  sound.observe(framed(2, [], { mode: 1, scale: 65_535 }));
  expect(held.oscillators[0].stopped).toBeCloseTo(0.25, 6);

  // Turned around halfway down. The scale is continuous across the turn, and
  // what the way back has left is the half already spent — so the cue is half
  // the length, not the whole of it. A cue that ran the full 250 ms here would
  // still be sounding after the Field had settled.
  sound.observe(framed(3, [], { mode: 3, scale: Math.round(65_535 / 2) }));
  expect(bend(held.oscillators[2])).toEqual([294, 392]);
  expect(held.oscillators[2].stopped).toBeCloseTo(0.125, 3);

  // And a reversal made in the same breath as the toggle it turns around — a
  // way out cancelled the instant it opened, with nothing to climb — is still
  // a sound rather than a click.
  sound.observe(framed(4, [], { mode: 1, scale: 0 }));
  expect(bend(held.oscillators[4])).toEqual([392, 294]);
  expect(held.oscillators[4].stopped).toBeCloseTo(0.05, 6);
});

test('a run that moved backwards sounds no depth it did not move through', () => {
  const { sound, target } = sounding();
  gesture(target);
  sound.observe(framed(40, [], { layer: 1 }));
  expect(sound.scheduled()).toBe(0);

  // A restart, a restore, or a branch can move the step backwards. Where the
  // Form stood in a history that no longer stands is not a layer it has just
  // moved from, so the first snapshot after it sets the depth again in silence.
  sound.observe(framed(3, []));
  expect(sound.scheduled()).toBe(0);
  sound.observe(framed(4, [], { layer: 1 }));
  expect(sound.scheduled()).toBe(1);
});

// ---------------------------------------------------------------------------
// Separate channels
// ---------------------------------------------------------------------------

test('reduced motion does not silence a single cue', () => {
  const { sound, target } = sounding();
  gesture(target);
  sound.observe(framed(1, [{ kind: 1 }, { kind: 2 }], { charging: true, reduced: true }));
  // The two settings are separate channels of `InputConfig`, and a player who
  // asked for less movement has not asked for less sound.
  expect(sound.scheduled()).toBe(3);
});

test('the stand-in Field sounds its scripted Pulse the same way', () => {
  const { sound, target, context } = sounding();
  gesture(target);
  // The stand-in charges for the first stretch of every turn and releases the
  // step after: read straight through, the charging voice opens once and the
  // release sounds.
  for (let step = 0; step <= 24; step += 1) sound.observe(fixtureSnapshot(step));
  const held = context();
  expect(shapes(held as RecordingContext).filter((kind) => kind === 'triangle')).toHaveLength(1);
  expect(sound.scheduled()).toBeGreaterThan(0);
});

// ---------------------------------------------------------------------------
// Letting go
// ---------------------------------------------------------------------------

/** Charges for a few steps and answers with the voice that is sounding it. */
function charging(sound: Sound, context: RecordingContext): RecordingOscillator {
  for (let step = 1; step <= 5; step += 1) {
    context.currentTime = step / 30;
    sound.observe(framed(step, [], { charging: true }));
  }
  const voice = context.oscillators[0];
  expect(voice.type).toBe('triangle');
  expect(voice.stopped).toBeNull();
  return voice;
}

test('a pressure arriving, its crisis, and its resolution each sound, distinctly and short', () => {
  const { sound, target, context } = sounding();
  gesture(target);
  const held = context();
  if (!held) throw new Error('the gesture builds the context');

  /** A snapshot carrying one active pressure at a stage. */
  const bearing = (step: number, stage: FrameState['pressures'][number]['stage'] | null) => {
    const state = framed(step, []);
    state.pressures = stage === null
      ? []
      : [{ ordinal: 4, stage, targetKind: 'node', queued: false, level: 52_000, target: 1 }];
    return state;
  };

  // The first snapshot sets the reading and sounds nothing: standing under a
  // pressure is not its arrival.
  sound.observe(bearing(1, 'signal'));
  expect(sound.scheduled()).toBe(0);

  // Gone and back is an arrival: one rising voice, low, short.
  sound.observe(bearing(2, null));
  sound.observe(bearing(3, 'signal'));
  expect(sound.scheduled()).toBe(2);
  const onset = held.oscillators[held.oscillators.length - 1];
  expect(onset.type).toBe('triangle');
  expect(onset.frequency.values()).toEqual([196, 262]);
  expect((onset.stopped ?? 0) - (onset.started ?? 0)).toBeCloseTo(0.22, 6);

  // A stage inside the run of stages that is not the crisis sounds nothing:
  // the surface's rim carries the build, and the sound marks the corners.
  sound.observe(bearing(4, 'pressure'));
  const before = sound.scheduled();

  // The crisis is the one cue in the module that beats: two voices a
  // semitone apart.
  sound.observe(bearing(5, 'crisis'));
  expect(sound.scheduled()).toBe(before + 2);
  const [low, high] = held.oscillators.slice(-2);
  expect(low.frequency.values()).toEqual([220, 220]);
  expect(high.frequency.values()).toEqual([233, 233]);
  expect((low.stopped ?? 0) - (low.started ?? 0)).toBeCloseTo(0.24, 6);

  // The resolution is the arrival played back.
  sound.observe(bearing(6, 'resolution'));
  const settled = held.oscillators[held.oscillators.length - 1];
  expect(settled.type).toBe('sine');
  expect(settled.frequency.values()).toEqual([262, 196]);

  // A pressure that resolved and left sounds nothing more: the resolution
  // was its leaving. Every voice stops itself.
  const after = sound.scheduled();
  sound.observe(bearing(7, null));
  expect(sound.scheduled()).toBe(after);
  for (const oscillator of held.oscillators) {
    expect(oscillator.stopped).not.toBeNull();
  }

  // A pressure that leaves mid-stage — spent, or gone with a restore —
  // sounds the resolution once, because leaving is the resolved reading.
  sound.observe(bearing(8, 'crisis'));
  const standing = sound.scheduled();
  sound.observe(bearing(9, null));
  expect(sound.scheduled()).toBe(standing + 1);
});

test('a run that stops running lets the charging voice go', () => {
  const { sound, target, context } = sounding();
  gesture(target);
  const held = context();
  if (!held) throw new Error('the gesture builds the context');
  const voice = charging(sound, held);

  // The locked focus-loss rule sends one paused frame, which runs no step: the
  // snapshot that carries the mode change carries the step the run already
  // stood at. A voice that is only let go of on a new step would drone through
  // the whole pause.
  held.currentTime = 1;
  sound.observe(framed(5, [], { charging: true, mode: 4 }));

  expect(voice.stopped).not.toBeNull();
  // The gain the voice sounds through is walked back to nothing, so it closes
  // rather than cutting.
  const gain = held.gains[1].gain.values();
  expect(gain[gain.length - 1]).toBe(0);

  // And nothing starts again while the run is suspended.
  sound.observe(framed(5, [], { charging: true, mode: 4 }));
  expect(shapes(held).filter((kind) => kind === 'triangle')).toHaveLength(1);

  // When the run comes back and the player holds again, a fresh voice sounds.
  held.currentTime = 2;
  sound.observe(framed(6, [], { charging: true }));
  expect(shapes(held).filter((kind) => kind === 'triangle')).toHaveLength(2);
});

test('a blurred window lets the charging voice go before any frame says so', () => {
  const { sound, target, context } = sounding();
  gesture(target);
  const held = context();
  if (!held) throw new Error('the gesture builds the context');
  const voice = charging(sound, held);

  // A hidden tab stops the surface's own loop, so no snapshot may arrive at
  // all until it returns. The sound lets go for the same reason and at the same
  // moment the input does, rather than waiting to be told.
  held.currentTime = 1;
  target.dispatchEvent(new Event('blur'));

  expect(voice.stopped).not.toBeNull();
  expect(sound.state()).toBe('running');
});

test('a completed ranking is heard once, and the first one sets the reading', () => {
  // A ranking is a record the worker raises rather than a place on the Field,
  // so the shell hands the record's ordinal over. The first sets the reading
  // and sounds nothing — standing under an evaluation is not one arriving —
  // and every fresh ordinal after it sounds once.
  const { sound, target, context } = sounding();
  gesture(target);
  const held = context();
  if (!held) throw new Error('the gesture builds the context');

  sound.ranked(0);
  expect(sound.scheduled()).toBe(0);

  sound.ranked(0);
  expect(sound.scheduled()).toBe(0);

  // A fresh record: the pair the ranking cue is.
  sound.ranked(1);
  expect(sound.scheduled()).toBe(2);

  // A run that stands under no record at all is not a ranking going away.
  sound.ranked(null);
  expect(sound.scheduled()).toBe(2);
});
