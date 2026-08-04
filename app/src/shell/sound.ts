/**
 * Sound: the Field's cues, synthesized.
 *
 * `docs/field-framework/SPEC.md` fixes the direction — nonrepresentational
 * sound, nothing depicted — and the offline rule fixes the means: every cue is
 * built here out of oscillators, gains, and one filter, so the bundle carries
 * no audio file and the game asks the network for nothing. What a cue says, it
 * says through pitch, length, and shape.
 *
 * What this module reads is exactly what the renderer reads: the decoded
 * `FrameState`. It owns nothing authoritative, writes nothing back, and drives
 * nothing — losing it costs the sound of a few cues and nothing else. The cues
 * themselves are the locked closed set of `docs/field-framework/ARCHITECTURE.md`
 * (`FrameState`, cue kinds), read by kind:
 *
 * | Reading | Source in the frame | Recipe |
 * |---|---|---|
 * | Coupling extending | the controlled Form's pulse-charging flag | two quiet sines, rising one octave while reach grows |
 * | Target acquired | a held preview changing from empty to effective | a brief soft resolving dyad |
 * | A Pulse emitted | cue 1 | a low sine body that expands and settles without a sharp transient |
 * | Charge gathered | cue 2 | a warm rising fifth, moving inward |
 * | A Port opened | cue 3 | a slow resolving dyad with a quiet upper glint |
 * | Interference pushed | cue 12 | a filtered triangle settling downward |
 * | An objective completed | cue 11 | two sines a fourth apart, 440 and 587 Hz, 300 ms, rising: the shape of arriving |
 * | An Anchor written | cue 7 | two sines an octave apart, 220 and 440 Hz, 620 ms, flat: the longest and steadiest cue there is |
 * | The pattern gave way | cue 8 | a triangle falling 300 → 150 Hz over 420 ms, under a filter: the one cue that sinks |
 * | And carried on | cue 9 | the same shape the other way, 150 → 300 Hz: recovery is the setback played back |
 * | A layer deeper | the controlled Form's layer, one greater | a triangle settling 260 → 130 Hz over 300 ms |
 * | A layer shallower | the controlled Form's layer, one less | the same length the other way, 130 → 260 Hz |
 * | Control moved to another Form | the controlled Form's identifier, changed | a triangle rising 349 → 466 Hz over 200 ms, its octave above it: a place taken up |
 * | Still Mode entered | the mode, moving into `ramp_in` | two sines a fifth apart, 392 → 294 Hz over 250 ms: the length of the ramp itself |
 * | Still Mode left | the mode, moving into `ramp_out` | the same shape the other way, 294 → 392 Hz |
 * | A pressure arriving | a pressure of section 6, newly active | a triangle rising 196 → 262 Hz over 220 ms, low: something starting |
 * | A pressure at its crisis | an active pressure's stage, newly `crisis` | two triangles a semitone apart, 220 and 233 Hz, 240 ms: the one cue that beats |
 * | A pressure resolving | an active pressure's stage, newly `resolution` — or its record leaving the frame | a sine settling 262 → 196 Hz over 260 ms: the arrival played back |
 *
 * The last four are the readings here that are not cues of the closed set.
 * A depth change is a change of state rather than an event of a step, and the
 * snapshot carries it plainly: the controlled Form's `layer`. Entering and
 * leaving Still Mode is the same kind of reading, carried by the header's own
 * mode byte. Both are read the way the renderer reads those fields for the
 * haze and the camera plane — derived from successive snapshots, owned by
 * nothing, and lost harmlessly whenever the module is rebuilt. Each pair is
 * one shape played both ways, which is what makes them tell apart without a
 * word for either, and the two pairs sit a register apart so a depth change
 * and a change of mode are never each other.
 *
 * Three rules the module is held to, each with its own test:
 *
 * - **The level gates everything.** `InputConfig.sound_level` is the locked
 *   setting, raw `Frac`, full at 65536. At 0 no `AudioContext` is ever
 *   constructed, so a player who wants silence gets an audio graph that does
 *   not exist rather than one turned down.
 * - **Reduced motion never silences it.** The two settings are separate
 *   channels of `InputConfig`, and a player who has asked for less movement has
 *   not asked for less sound. Nothing here reads the motion flag.
 * - **The autoplay policy is obeyed rather than worked around.** No context is
 *   constructed until the player's first gesture, which is also the gesture
 *   that presses the first Pulse, so the first cue a player can cause is the
 *   first cue that can sound. A context that will not resume is left alone and
 *   reported through `state()`; nothing is logged and nothing throws.
 *
 * The charging voice is the one thing here that sounds between cues, so it is
 * the one thing that could be left sounding. It is let go of twice over: on any
 * snapshot whose mode is not `running` — read before the step guard, because the
 * paused frame the locked focus-loss rule sends runs no step and so carries the
 * step the run already stood at — and on blur or a hidden tab directly, because
 * a hidden tab stops the surface's own loop and no snapshot may arrive at all
 * until it returns.
 */

import type { FrameState } from '../../../worker/src/frame-state';
import type { EngineeringTransitionKind } from '../../../worker/src/protocol';

/** The locked `InputConfig` default: full. */
export const FULL_SOUND = 65_536;

/** The closed cue set's own numbering, for the kinds this module names. */
const CUE_PULSE_EMITTED = 1;
const CUE_CHARGE_GATHERED = 2;
const CUE_PORT_OPENED = 3;
const CUE_ANCHOR_WRITTEN = 7;
const CUE_COLLAPSE = 8;
const CUE_RECOVERY = 9;
const CUE_OBJECTIVE_COMPLETE = 11;
const CUE_INTERFERENCE_PUSHED = 12;

/** The cue kinds that make a Pulse's release a Pulse that did something. */
const OUTCOME_CUES = [CUE_CHARGE_GATHERED, CUE_PORT_OPENED, CUE_INTERFERENCE_PUSHED];

/**
 * How loud the whole of it stands at full level. Cues are brief and the field
 * is quiet between them, so the headroom is generous on purpose.
 */
const MASTER_GAIN = 0.3;

/** One simulation step, in seconds: the ramp span while a hold is charging. */
const STEP_SECONDS = 1 / 30;

/** Where the charging voice starts and where holding carries it. */
const CHARGE_LOW_HZ = 108;
const CHARGE_HIGH_HZ = 216;

/**
 * How many steps of holding reach the top of that rise: the locked charging
 * rule's own count, so the tone is at the top of its range exactly when the
 * charge is full, and holding past full changes neither.
 */
const CHARGE_STEPS = 32;

/** How loud the charging voice stands, against the master gain. */
const CHARGE_GAIN = 0.055;

/**
 * The depth cue: where a change of layer starts and ends, how long it takes,
 * and how loud it stands.
 *
 * Longer and lower than any cue a step raises, because it says something about
 * where the Form now is rather than about something that happened to it, and
 * one shape played both ways: down from 260 Hz for a layer deeper, up to it for
 * a layer shallower.
 */
const DEPTH_HIGH_HZ = 260;
const DEPTH_LOW_HZ = 130;
const DEPTH_SECONDS = 0.3;
const DEPTH_GAIN = 0.34;

/**
 * The Still Mode cue: where entering starts and ends, how long it takes, and
 * how loud it stands.
 *
 * As long as the ramp it announces, so the sound and the slowdown are the same
 * quarter second, and a fifth above the depth cue's register so the two are
 * never mistaken for one another. Entering falls and leaving rises, which is
 * the same relation the depth pair keeps.
 */
const STILL_HIGH_HZ = 392;
const STILL_LOW_HZ = 294;
const STILL_SECONDS = 0.25;
const STILL_GAIN = 0.3;

/** The shortest a Still Mode cue is ever played, however little ramp is left. */
const STILL_FLOOR_SECONDS = 0.05;

/**
 * The cue a change of the active observation View sounds.
 *
 * It is derived from the snapshot like every other cue this module sounds —
 * the View members are in the frame's independent bitset, and the immediate
 * free `set_focus` command is what moves them. A physical-compartment commit
 * does not move this reading. The closed cue set carries no cue of its own for
 * a View change, and the frame
 * gains no field for one: this is the same reading the renderer already draws
 * the boundary from. A rising pair, briefly, in the boundary's own register
 * rather than the Pulse's or the depth pair's.
 */
/**
 * The cue a Handoff sounds: control has moved to another Form.
 *
 * Derived from the snapshot like every other reading here that is not a cue of
 * the closed set — which Form carries the `controlled` flag is in the frame's
 * forms record, and a Handoff is exactly what moves it. It sits between the
 * depth pair and the Still Mode pair, in a register neither uses, and it rises:
 * a Handoff is a place taken up rather than a place left. It is read above the
 * step guard, because the frame that carries a Handoff runs no step at all.
 *
 * A rising fourth, in triangle, between the depth pair below it and the Still
 * Mode pair above: no other cue rises a fourth and none stands in this
 * register, so a Handoff is never a depth change, an adoption, or an exit.
 */
const HANDOFF_LOW_HZ = 349;
const HANDOFF_HIGH_HZ = 466;
const HANDOFF_SECONDS = 0.2;
const HANDOFF_GAIN = 0.28;

const ADOPT_LOW_HZ = 330;
const ADOPT_HIGH_HZ = 495;
const ADOPT_SECONDS = 0.18;
const ADOPT_GAIN = 0.26;

/**
 * The cue a completed ranking sounds.
 *
 * This one is not read off a snapshot, and it cannot be: a ranking is a record
 * the worker raises with `review_ready`, not a place on the Field, so the
 * frame carries nothing that moves when one completes. The shell hands the
 * record's ordinal over instead, and the cue sounds once per ordinal — the
 * same once-per-change discipline every other cue here keeps, on the one
 * reading that says a fresh evaluation stands.
 *
 * It is a short falling pair, two octaves clear of the adoption cue below it,
 * so a ranking arriving and a View being adopted are never each other — and a
 * commit sounds both, in the order they happen.
 */
/**
 * The pressure cues' own register: below every Pulse cue and above the depth
 * pair, so a pressure arriving, beating, and resolving is never a thing the
 * player did. The onset rises, the resolution is the onset played back, and
 * the crisis is the one cue in the module that beats — two voices a semitone
 * apart, which is the audible shape of the rim's own beat.
 */
const PRESSURE_ONSET_LOW_HZ = 196;
const PRESSURE_ONSET_HIGH_HZ = 262;
const PRESSURE_CRISIS_HZ = 220;
const PRESSURE_CRISIS_BEAT_HZ = 233;

const RANK_LOW_HZ = 880;
const RANK_HIGH_HZ = 740;
const RANK_SECONDS = 0.12;
const RANK_GAIN = 0.2;

/** What a cue may sound like, and what the module reports about itself. */
export type SoundState = 'idle' | 'running' | 'suspended' | 'closed';

export interface SoundOptions {
  /** The raw `Frac` from `InputConfig`. Full by default, silent at 0. */
  level?: number;
  /**
   * How a context is made. Replaced in tests, and answering null is how a
   * platform without WebAudio is handled: the module runs and sounds nothing.
   */
  context?: () => AudioContext | null;
  /** Where the first-gesture listeners are attached. Replaced in tests. */
  target?: EventTarget;
}

export interface EngineeringSoundTransition {
  commitAllowed: boolean;
  operation: EngineeringTransitionKind;
  previewId: string;
  status: 'preview' | 'committed';
}

export interface Sound {
  /**
   * Reads one snapshot, once per rendered frame. Work is done once per
   * completed step, so a display running at twice the step rate schedules each
   * cue once.
   */
  observe: (next: FrameState) => void;
  /**
   * Reads the ordinal of the evaluation record the run stands under. A fresh
   * one sounds the ranking cue; the first sets the reading and sounds nothing,
   * exactly as the first snapshot sets the depth and sounds nothing.
   */
  ranked: (ordinal: number | null) => void;
  /** Sounds only an addressed refusal or an accepted reconstruction boundary. */
  transition: (next: EngineeringSoundTransition | null) => void;
  /** Takes a new `sound_level`. Zero closes the context and builds no other. */
  setLevel: (level: number) => void;
  /** What the context is doing, and `idle` before there is one. */
  state: () => SoundState;
  /** How many cues have been scheduled. A diagnostic, and what a test reads. */
  scheduled: () => number;
  /** Releases the context and stops listening. */
  close: () => void;
}

/** The default context factory: none at all where WebAudio does not stand. */
function platformContext(): AudioContext | null {
  if (typeof AudioContext === 'undefined') return null;
  return new AudioContext();
}

/**
 * Opens the sound module.
 *
 * Nothing is constructed here: the first gesture builds the context, and only
 * if the level asks for one.
 */
export function openSound(options: SoundOptions = {}): Sound {
  const make = options.context ?? platformContext;
  const viewport = typeof window === 'undefined' ? null : window;
  const target = options.target ?? viewport;

  let level = options.level ?? FULL_SOUND;
  let context: AudioContext | null = null;
  let master: GainNode | null = null;
  /** The voice a held Pulse sounds through, and none while nothing is held. */
  let charging: {
    voices: Array<{ oscillator: OscillatorNode; gain: GainNode; ratio: number }>;
    since: number;
    connected: boolean;
  } | null = null;
  /** The newest step read, and −1 before the first snapshot. */
  let seen = -1;
  /** The depth the controlled Form stood at, and none before the first. */
  let depth: number | null = null;
  /** The mode the newest snapshot reported, and none before the first. */
  let mode: string | null = null;
  let transitionReading: string | null = null;
  /** The active passive View the newest snapshot carried, and none before the first. */
  let inside: number | null = null;
  /** Which Form carried control, and none before the first snapshot. */
  let steering: number | null = null;
  /**
   * The stage each active pressure last stood at, by ordinal, and none before
   * the first snapshot. Read the way the depth is read: derived from
   * successive snapshots, owned by nothing, and dropped with a history that no
   * longer stands.
   */
  let standing: Map<number, string> | null = null;
  /** The ordinal of the record the run stands under, and none before the first. */
  let ranked: number | null = null;
  let scheduled = 0;
  let closed = false;
  /** Whether the player has done anything the autoplay policy counts. */
  let gestured = false;

  /**
   * The context, built on the first gesture that asks for one.
   *
   * A context built before a gesture is a context the browser suspends and
   * warns about, so none is built before one. The gesture that presses the
   * first Pulse is the gesture that opens this, which is why the first cue a
   * player can cause is never the one that is lost.
   */
  function opened(): AudioContext | null {
    if (closed || level <= 0 || !gestured) return null;
    if (context) return context;
    const made = make();
    if (!made) return null;
    context = made;
    master = made.createGain();
    master.gain.value = weight();
    master.connect(made.destination);
    return context;
  }

  /** How loud the master gain stands at the level held. */
  function weight(): number {
    return MASTER_GAIN * Math.min(1, Math.max(0, level / FULL_SOUND));
  }

  /**
   * Asks a suspended context to resume. A refusal is the platform's answer and
   * not a fault: it is swallowed here, and `state()` still reports `suspended`.
   */
  function resume(held: AudioContext): void {
    if (held.state !== 'suspended') return;
    void held.resume().catch(() => {});
  }

  const onGesture = (): void => {
    gestured = true;
    const held = opened();
    if (held) resume(held);
  };

  /** Lets go of what is sounding, for the same reason the input lets go. */
  const onLetGo = (): void => stopCharging();

  const onVisibility = (): void => {
    if (typeof document !== 'undefined' && document.visibilityState === 'hidden') stopCharging();
  };

  if (target) {
    target.addEventListener('pointerdown', onGesture, { passive: true });
    target.addEventListener('keydown', onGesture, { passive: true });
    target.addEventListener('blur', onLetGo, { passive: true });
  }
  if (typeof document !== 'undefined') {
    document.addEventListener('visibilitychange', onVisibility);
  }

  /** One oscillator into one gain, into the master. */
  function voice(
    kind: OscillatorType,
    hz: number,
  ): { held: AudioContext; oscillator: OscillatorNode; gain: GainNode } | null {
    const held = opened();
    if (!held || !master) return null;
    resume(held);
    const oscillator = held.createOscillator();
    const gain = held.createGain();
    oscillator.type = kind;
    oscillator.frequency.setValueAtTime(hz, held.currentTime);
    gain.gain.setValueAtTime(0, held.currentTime);
    oscillator.connect(gain);
    gain.connect(master);
    return { held, oscillator, gain };
  }

  /**
   * One short cue: a tone that opens quickly, falls away, and stops.
   *
   * `to` bends the pitch over the cue's own length, which is what tells the
   * four apart without a word for any of them: a Pulse that did something
   * falls, a shove falls further and harder, a Port opening rises.
   */
  function tone(
    kind: OscillatorType,
    from: number,
    to: number,
    seconds: number,
    loudness: number,
    filtered = false,
  ): void {
    const made = voice(kind, from);
    if (!made) return;
    const { held, oscillator, gain } = made;
    const at = held.currentTime;
    const open = Math.min(0.012, seconds / 4);
    if (filtered) {
      // The dull cue: everything above the fundamental taken off, so what is
      // left has no edge to it at all.
      const filter = held.createBiquadFilter();
      filter.type = 'lowpass';
      filter.frequency.setValueAtTime(from * 2, at);
      oscillator.disconnect();
      oscillator.connect(filter);
      filter.connect(gain);
    }
    oscillator.frequency.linearRampToValueAtTime(to, at + seconds);
    gain.gain.linearRampToValueAtTime(loudness, at + open);
    gain.gain.linearRampToValueAtTime(0, at + seconds);
    oscillator.start(at);
    oscillator.stop(at + seconds);
    scheduled += 1;
  }

  /** Two tones a fifth apart, the second quieter: one cue, not two. */
  function pair(from: number, to: number, seconds: number, loudness: number): void {
    tone('sine', from, to, seconds, loudness);
    tone('sine', from * 1.5, to * 1.5, seconds * 0.86, loudness * 0.45);
  }

  function soundTransition(next: EngineeringSoundTransition | null): void {
    if (!next) {
      transitionReading = null;
      return;
    }
    const reading = `${next.previewId}:${next.status}:${next.commitAllowed ? 'ready' : 'refused'}`;
    if (reading === transitionReading) return;
    transitionReading = reading;
    if (next.status === 'preview') {
      if (!next.commitAllowed) tone('triangle', 116, 96, 0.18, 0.12, true);
      return;
    }
    switch (next.operation) {
      case 'restart_assembly':
        pair(196, 196, 0.22, 0.13);
        break;
      case 'revert_generator':
        pair(196, 247, 0.28, 0.14);
        break;
      case 'full_contract_reset':
        pair(164, 246, 0.36, 0.15);
        tone('sine', 328, 369, 0.42, 0.06, true);
        break;
    }
  }

  /** Starts, carries, or stops the voice a held Pulse sounds through. */
  function carryCharging(holding: boolean, step: number, connected: boolean): void {
    if (!holding) {
      stopCharging();
      return;
    }
    if (!charging) {
      const ratios = [1, 1.5];
      const voices: Array<{ oscillator: OscillatorNode; gain: GainNode; ratio: number }> = [];
      for (const [place, ratio] of ratios.entries()) {
        const made = voice('sine', CHARGE_LOW_HZ * ratio);
        if (!made) continue;
        const at = made.held.currentTime;
        made.gain.gain.linearRampToValueAtTime(CHARGE_GAIN * (place === 0 ? 1 : 0.46), at + 0.12);
        made.oscillator.start(at);
        voices.push({ oscillator: made.oscillator, gain: made.gain, ratio });
      }
      if (voices.length === 0) return;
      charging = { voices, since: step, connected };
      return;
    }
    const held = context;
    if (!held) return;
    // The rise is read off the steps the hold has stood for, so it carries the
    // duration rather than the wall clock, and a frame that runs no step moves
    // it nowhere.
    const through = Math.min(1, (step - charging.since) / CHARGE_STEPS);
    const wanted = CHARGE_LOW_HZ + (CHARGE_HIGH_HZ - CHARGE_LOW_HZ) * through;
    for (const voice of charging.voices) {
      voice.oscillator.frequency.linearRampToValueAtTime(
        wanted * voice.ratio,
        held.currentTime + STEP_SECONDS,
      );
    }
    if (connected && !charging.connected) {
      pair(330, 392, 0.13, 0.14);
    }
    charging.connected = connected;
  }

  function stopCharging(): void {
    if (!charging || !context) {
      charging = null;
      return;
    }
    const at = context.currentTime;
    for (const voice of charging.voices) {
      voice.gain.gain.linearRampToValueAtTime(0, at + 0.09);
      voice.oscillator.stop(at + 0.1);
    }
    charging = null;
  }

  /**
   * Sounds a change of layer, in the direction it went.
   *
   * The first snapshot with a controlled Form in it sets the depth and sounds
   * nothing: arriving somewhere is not moving there. A layer that moves by more
   * than one — a restore, a branch, a Field established under the module — is
   * one cue in the direction it moved, not one per layer crossed.
   */
  function carryDepth(layer: number | undefined): void {
    if (layer === undefined) {
      depth = null;
      return;
    }
    const before = depth;
    depth = layer;
    if (before === null || before === layer) return;
    if (layer > before) tone('triangle', DEPTH_HIGH_HZ, DEPTH_LOW_HZ, DEPTH_SECONDS, DEPTH_GAIN);
    else tone('triangle', DEPTH_LOW_HZ, DEPTH_HIGH_HZ, DEPTH_SECONDS, DEPTH_GAIN);
  }

  /**
   * Sounds a change of mode, in the direction it went and for as long as the
   * ramp it announces has left to run.
   *
   * Only the two ramps are announced, and each only as it opens: a ramp is the
   * transition, and its completion is the arrival the ramp already said was
   * coming. The first snapshot sets the mode and sounds nothing, exactly as the
   * first one sets the depth and sounds nothing.
   *
   * The length is read off the same header the mode came from, because a ramp
   * does not always open with the whole 250 ms in front of it: a toggle during
   * a ramp reverses it and mirrors its position, so cancelling an entry that
   * was a fifth of the way down leaves a fifth of a ramp to climb back. The
   * scale is a linear function of what a ramp has left, so the remainder is
   * read straight out of it — the whole of it on a fresh entry or exit, and
   * the mirrored part on a reversal. A cue that ran the full length over a
   * ramp that did not would still be sounding after the Field had settled.
   */
  function carryMode(next: string, scale: number): void {
    const before = mode;
    mode = next;
    if (before === null || before === next) return;
    if (next !== 'ramp_in' && next !== 'ramp_out') return;
    // `ramp_in` falls from full, so what it has left is the scale itself;
    // `ramp_out` rises to full, so what it has left is the rest of the way.
    const held = Math.min(1, Math.max(0, scale / 65_535));
    const remaining = next === 'ramp_in' ? held : 1 - held;
    // A floor, so a reversal made in the same breath as the toggle it turns
    // around is still a sound rather than a click.
    const seconds = Math.max(STILL_FLOOR_SECONDS, STILL_SECONDS * remaining);
    const from = next === 'ramp_in' ? STILL_HIGH_HZ : STILL_LOW_HZ;
    const to = next === 'ramp_in' ? STILL_LOW_HZ : STILL_HIGH_HZ;
    tone('sine', from, to, seconds, STILL_GAIN);
    tone('sine', from * 1.5, to * 1.5, seconds * 0.86, STILL_GAIN * 0.45);
  }

  /**
   * Sounds a Handoff: control standing on a Form it did not stand on before.
   *
   * The first snapshot sets the reading and sounds nothing — holding a Form is
   * not taking it up. A frame carrying no Form at all leaves the reading where
   * it stood, so a snapshot taken before a chapter is established sounds
   * nothing and forgets nothing.
   */
  function carryControl(next: FrameState): void {
    const held = next.forms.find((form) => form.controlled);
    if (!held) return;
    const before = steering;
    steering = held.id;
    if (before === null || before === held.id) return;
    tone('triangle', HANDOFF_LOW_HZ, HANDOFF_HIGH_HZ, HANDOFF_SECONDS, HANDOFF_GAIN);
    tone(
      'triangle',
      HANDOFF_LOW_HZ * 2,
      HANDOFF_HIGH_HZ * 2,
      HANDOFF_SECONDS * 0.8,
      HANDOFF_GAIN * 0.4,
    );
  }

  /**
   * Sounds a change of the passive observation View.
   *
   * The first snapshot sets the reading and sounds nothing, exactly as the
   * first one sets the depth and sounds nothing — standing somewhere is not
   * moving there. A history that no longer stands drops it with the rest.
   */
  function carryInside(next: FrameState): void {
    let held = 0;
    for (let place = 0; place < next.ports.length; place += 1) {
      if (next.inside[place]) held = (held * 31 + next.ports[place].node) | 0;
    }
    const before = inside;
    inside = held;
    if (before === null || before === held) return;
    pair(ADOPT_LOW_HZ, ADOPT_HIGH_HZ, ADOPT_SECONDS, ADOPT_GAIN);
  }

  /**
   * Sounds the stages of the staged pressures: an arrival, a crisis, and a
   * resolution, each once per change, each distinct, each short.
   *
   * The reading is the frame's own section 6, exactly as the renderer reads
   * it for the rim: which pressures stand active, and at which stage. The
   * first snapshot sets the reading and sounds nothing. A pressure that
   * leaves the frame mid-stage — spent before its resolution was seen, or
   * gone with a restore — sounds the resolution cue once, because leaving is
   * the resolved reading of it.
   */
  function carryPressures(next: FrameState): void {
    const now = new Map<number, string>();
    for (const pressure of next.pressures) {
      if (!pressure.queued) now.set(pressure.ordinal, pressure.stage);
    }
    const before = standing;
    standing = now;
    if (before === null) return;
    for (const [ordinal, stage] of now) {
      const held = before.get(ordinal);
      if (held === stage) continue;
      if (held === undefined) {
        tone('triangle', PRESSURE_ONSET_LOW_HZ, PRESSURE_ONSET_HIGH_HZ, 0.22, 0.32);
      } else if (stage === 'crisis') {
        tone('triangle', PRESSURE_CRISIS_HZ, PRESSURE_CRISIS_HZ, 0.24, 0.36);
        tone('triangle', PRESSURE_CRISIS_BEAT_HZ, PRESSURE_CRISIS_BEAT_HZ, 0.24, 0.36);
      } else if (stage === 'resolution') {
        tone('sine', PRESSURE_ONSET_HIGH_HZ, PRESSURE_ONSET_LOW_HZ, 0.26, 0.3);
      }
    }
    for (const ordinal of before.keys()) {
      if (before.get(ordinal) === 'resolution') continue;
      if (!now.has(ordinal)) {
        tone('sine', PRESSURE_ONSET_HIGH_HZ, PRESSURE_ONSET_LOW_HZ, 0.26, 0.3);
      }
    }
  }

  function sound(cues: readonly { kind: number }[]): void {
    const outcome = cues.some((cue) => OUTCOME_CUES.includes(cue.kind));
    for (const cue of cues) {
      switch (cue.kind) {
        case CUE_PULSE_EMITTED:
          // A low, rounded body makes the expansion audible without the former
          // high downward whistle. Outcomes add their own local confirmation.
          tone('sine', outcome ? 104 : 88, outcome ? 62 : 72, outcome ? 0.3 : 0.14, outcome ? 0.28 : 0.18, true);
          if (outcome) tone('sine', 208, 124, 0.24, 0.1, true);
          break;
        case CUE_CHARGE_GATHERED:
          pair(247, 330, 0.19, 0.22);
          break;
        case CUE_PORT_OPENED:
          pair(220, 294, 0.44, 0.28);
          tone('sine', 440, 588, 0.34, 0.1);
          break;
        case CUE_INTERFERENCE_PUSHED:
          tone('triangle', 146, 92, 0.22, 0.18, true);
          break;
        case CUE_OBJECTIVE_COMPLETE:
          // An objective arriving: a fourth, rising, and short enough to sit
          // under whatever the step it landed on was already saying.
          pair(440, 587, 0.3, 0.4);
          break;
        case CUE_ANCHOR_WRITTEN:
          // The longest and steadiest cue in the set, and the only one that
          // does not bend: an Anchor is a place the run can be returned to.
          pair(220, 220, 0.62, 0.34);
          break;
        case CUE_COLLAPSE:
          // The pattern giving way: the one cue that sinks, and covered, so it
          // reads as something coming apart rather than something arriving.
          tone('triangle', 300, 150, 0.42, 0.36, true);
          break;
        case CUE_RECOVERY:
          // The same shape played back the other way.
          tone('triangle', 150, 300, 0.42, 0.36, true);
          break;
        default:
          // Every other cue of the closed set belongs to a goal that has not
          // reached it. A cue this module does not name sounds nothing rather
          // than sounding like something else.
          break;
      }
    }
  }

  return {
    observe(next) {
      if (closed || level <= 0) return;
      // A run that is not running holds nothing. This stands before the step
      // guard on purpose: the frame that carries a mode change runs no step, so
      // a voice let go of only on a new step would sound through the whole
      // pause.
      if (next.header.mode !== 'running') stopCharging();
      // The mode is read here for the same reason, and it is the whole reason
      // the reading is not inside the step guard: the frame that completes a
      // ramp runs no step at all, and neither does any frame of a still run.
      carryMode(next.header.mode, next.header.timeScale);
      // The passive View is read on the same frames and for the same reason:
      // `set_focus` runs no step, so its change would never be heard from
      // inside the step guard below. So is the Form control stands
      // on: a Handoff is answered in `still`, where no step runs and no mode
      // changes, so the frame that carries one would otherwise be silent.
      carryInside(next);
      // A restart, a restore, or a branch can move the step backwards, and the
      // Form that carried control in a history that no longer stands is not one
      // control has just left. The reading is dropped here rather than in the
      // block below, because this cue is read above the step guard and the
      // block below runs after it.
      if (next.header.step < seen) steering = null;
      carryControl(next);
      const step = next.header.step;
      if (step === seen) return;
      // A restart, a restore, or a branch can move the step backwards. What was
      // held for a history that no longer stands is dropped with it, the depth
      // included: where the Form stood in a history that no longer stands is
      // not a layer it has just moved from.
      if (step < seen) {
        stopCharging();
        depth = null;
        inside = null;
        ranked = null;
        standing = null;
        mode = next.header.mode;
      }
      seen = step;
      const steered = next.forms.find((form) => form.controlled);
      const preview = next.pulsePreview;
      const connected = Boolean(preview && (
        preview.gathered > 0
        || preview.reserveReleased > 0
        || preview.openedPorts > 0
        || preview.displacedPressures > 0
      ));
      carryCharging(steered?.pulseCharging === true, step, connected);
      carryDepth(steered?.layer);
      carryPressures(next);
      sound(next.cues);
    },
    ranked(ordinal) {
      if (closed || level <= 0 || ordinal === null) return;
      const before = ranked;
      ranked = ordinal;
      if (before === null || before === ordinal) return;
      pair(RANK_LOW_HZ, RANK_HIGH_HZ, RANK_SECONDS, RANK_GAIN);
    },
    transition(next) {
      if (closed) return;
      if (!next) {
        soundTransition(null);
        return;
      }
      if (level <= 0) return;
      soundTransition(next);
    },
    setLevel(next) {
      level = Math.max(0, next);
      if (level <= 0) {
        stopCharging();
        void context?.close().catch(() => {});
        context = null;
        master = null;
        return;
      }
      if (master) master.gain.value = weight();
    },
    state() {
      if (!context) return 'idle';
      return context.state as SoundState;
    },
    scheduled: () => scheduled,
    close() {
      closed = true;
      stopCharging();
      if (target) {
        target.removeEventListener('pointerdown', onGesture);
        target.removeEventListener('keydown', onGesture);
        target.removeEventListener('blur', onLetGo);
      }
      if (typeof document !== 'undefined') {
        document.removeEventListener('visibilitychange', onVisibility);
      }
      void context?.close().catch(() => {});
      context = null;
      master = null;
    },
  };
}
