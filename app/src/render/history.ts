/**
 * The renderer's own ephemera, derived from successive snapshots.
 *
 * Trails, the Charge a current has just delivered, the live cues, and the
 * camera's easing are all read out of the snapshots the renderer is handed and
 * held nowhere else. None of it is authoritative: losing it — a fallback
 * engaging, a worker restarting, a surface being rebuilt — costs a few frames
 * of trail and nothing more, which is why the renderer may hold it at all.
 *
 * Two derivations are worth stating, because neither is written in the
 * snapshot and both are read straight out of it:
 *
 * - **Accepted Supply.** Cue 13 is raised by the authoritative Current phase
 *   only after one addressed Node accepts Charge. The renderer retains only
 *   its short visual decay; Coupling, Route inflow, and other Charge increases
 *   cannot be mistaken for accepted environmental Supply.
 */

import type { FramePort, FrameState } from '../../../worker/src/frame-state';

/** How many trail samples one Form keeps. About four seconds at the step rate. */
const TRAIL_SAMPLES = 120;

/**
 * How long a delivery keeps a Port lit, in steps, once the rise stops. A
 * current delivers every step it is active, but Drain and the Node's own upkeep
 * pull the other way, so a Port standing in a current shows a rise on some
 * steps and not on others. Holding the reading for a second of play is what
 * makes the band a band rather than a flicker.
 */
const DELIVERY_DECAY_STEPS = 30;

/** How long a cue stands, in steps. */
const CUE_STEPS = 18;
const CUE_SUPPLY_ACCEPTED = 13;

/** How much of the way the camera closes on its target each step. */
const CAMERA_EASE = 0.22;

/**
 * How many rendered frames one ramp spans, at the render target: 250,000 µs of
 * real time against 60 frames a second.
 */
const RAMP_FRAMES = Math.ceil(250_000 / 16_667);

/**
 * How much of the way the Still Mode surface closes on where it is wanted,
 * each rendered frame.
 *
 * The surface comes up with the ramp, and the ramp is carried by the time
 * scale in a snapshot's header — but a snapshot rides a frame event only when
 * a step ran or the mode changed, and the last stretch of an entry ramp runs
 * no step at all. So the target is read from the snapshots that do arrive and
 * closed on here, once per rendered frame, which is what makes the 250 ms read
 * as 250 ms rather than as four jumps and a snap.
 *
 * The rate is tied to the ramp's own frame count rather than chosen by eye:
 * the surface closes to within a fiftieth of wherever the newest snapshot put
 * it in a third of a ramp, which leaves it within a twentieth of the ramp's
 * own reading at the moment the ramp completes. A slower rate lags a moving
 * target by more than that, and an overlay still fading in over a Field that
 * has already stopped reads as lag rather than as settling.
 */
const STILL_EASE = 1 - Math.pow(0.02, 3 / RAMP_FRAMES);

/** The Field's extent per axis, in units; the camera opens at its middle. */
export const FIELD_UNITS = 4096;

/** One Form's recent positions, oldest first once the ring has filled. */
export interface Trail {
  /** Positions in units, as x, y pairs. */
  readonly points: Float32Array;
  /** How many pairs are written. */
  count: number;
  /** Where the next pair goes. */
  head: number;
  /** The layer the Form stood on when the newest sample was taken. */
  layer: number;
  controlled: boolean;
  /** The step the newest sample was taken at. */
  step: number;
}

/** One cue still standing, with the step it opened at. */
export interface LiveCue {
  kind: number;
  a: number;
  b: number;
  step: number;
  /**
   * How far the controlled Form's own radius reached at the step this cue
   * opened, in units, and 0 when the frame carried none.
   *
   * A cue stands for longer than the step that raised it, and what a Pulse
   * reached is a property of the moment it was emitted rather than of the
   * frame being drawn — so it is captured here, once, with the cue. That is the
   * same derivation as every other reading in this file: read out of the
   * snapshot, held nowhere authoritative, and lost harmlessly.
   */
  reach: number;
}

/** A Port's position in units, which the record carries as units times 16. */
export function portUnits(port: FramePort): { x: number; y: number } {
  return { x: port.x / 16, y: port.y / 16 };
}

/**
 * Everything the renderer derives from the snapshots it has been handed. One
 * of these belongs to one renderer and is dropped with it.
 */
export class Ephemera {
  /** One trail per Form id. */
  readonly trails = new Map<number, Trail>();

  /** The step a Node last took Charge from a current at, by Node id. */
  readonly delivered = new Map<number, number>();

  /** The cues still standing, oldest first. */
  readonly cues: LiveCue[] = [];

  /** Where the camera sits, in units. */
  cameraX = FIELD_UNITS / 2;
  cameraY = FIELD_UNITS / 2;
  cameraLayer = 0;

  /** How far the Still Mode surface has come up, 0 to 1. */
  stillPresence = 0;

  /** The newest step this has read, and −1 before the first snapshot. */
  private seen = -1;

  /** Whether the camera has been placed at all. */
  private placed = false;

  /**
   * Which Form carried control when the camera was last placed, and none
   * before the first snapshot. A Handoff moves it, and the camera follows the
   * new Form at once rather than easing across the Field: ARCHITECTURE.md's
   * Handoff says the authoritative target snaps on that frame, and a Form on
   * the far side of the Field is not somewhere the view travelled to.
   */
  private steering: number | null = null;

  /**
   * Reads one pair of snapshots. Everything derived from a completed step is
   * computed once per step rather than once per rendered frame, so a display
   * running at twice the step rate does the work once.
   */
  observe(_previous: FrameState, next: FrameState): void {
    const step = next.header.step;
    if (step === this.seen) return;
    // A restart, a restore, or a branch can move the step backwards. What was
    // derived from a history that no longer stands is dropped rather than
    // carried into one it does not belong to.
    if (step < this.seen) this.clear();
    this.seen = step;

    for (const form of next.forms) {
      this.sample(form.id, form.x, form.y, form.layer, form.controlled, step);
    }
    for (const [id] of this.trails) {
      if (!next.forms.some((form) => form.id === id)) this.trails.delete(id);
    }

    // The Form record carries its radius as Q8.8 units.
    const reach = (next.forms.find((form) => form.controlled)?.radius ?? 0) / 256;
    for (const cue of next.cues) {
      if (cue.kind === CUE_SUPPLY_ACCEPTED) this.delivered.set(cue.b, step);
      this.cues.push({ kind: cue.kind, a: cue.a, b: cue.b, step, reach });
    }
    for (const [node, at] of this.delivered) {
      if (step - at > DELIVERY_DECAY_STEPS) this.delivered.delete(node);
    }
    while (this.cues.length > 0 && step - this.cues[0].step > CUE_STEPS) this.cues.shift();
  }

  /**
   * Moves the camera one step of the way toward where it is wanted — or puts
   * it there outright on the first placement, under reduced motion, and on the
   * frame a Handoff moved control.
   */
  easeCamera(
    x: number,
    y: number,
    layer: number,
    reducedMotion: boolean,
    steering: number | null = null,
  ): void {
    const handed = steering !== null && this.steering !== null && steering !== this.steering;
    if (steering !== null) this.steering = steering;
    if (!this.placed || reducedMotion || handed) {
      this.cameraX = x;
      this.cameraY = y;
      this.placed = true;
    } else {
      this.cameraX += (x - this.cameraX) * CAMERA_EASE;
      this.cameraY += (y - this.cameraY) * CAMERA_EASE;
    }
    this.cameraLayer = layer;
  }

  /**
   * Moves the Still Mode surface toward where the newest snapshot puts it, and
   * answers where it now stands.
   *
   * Reduced motion arrives there at once: the surface coming up is motion, and
   * a player who asked for less of it has asked for this too. What it says is
   * unchanged either way — the handles are up or they are not.
   */
  easeStill(wanted: number, reducedMotion: boolean): number {
    const held = wanted < 0 ? 0 : wanted > 1 ? 1 : wanted;
    if (reducedMotion) {
      this.stillPresence = held;
      return held;
    }
    this.stillPresence += (held - this.stillPresence) * STILL_EASE;
    // The two ends are settled on rather than approached forever, so a surface
    // that is up is all the way up and one that is down draws nothing at all.
    if (Math.abs(held - this.stillPresence) < 0.005) this.stillPresence = held;
    return this.stillPresence;
  }

  /** How lit a Node's delivery still reads, 1 at the rise and 0 once it is old. */
  deliveryStrength(node: number, clock: number): number {
    const at = this.delivered.get(node);
    if (at === undefined) return 0;
    const age = clock - at;
    if (age <= 0) return 1;
    if (age >= DELIVERY_DECAY_STEPS) return 0;
    return 1 - age / DELIVERY_DECAY_STEPS;
  }

  /** How far through its life a cue is, 0 at the open and 1 at the close. */
  cueAge(cue: LiveCue, clock: number): number {
    const age = (clock - cue.step) / CUE_STEPS;
    return age < 0 ? 0 : age > 1 ? 1 : age;
  }

  /** Drops everything derived. The next snapshot rebuilds it. */
  clear(): void {
    this.steering = null;
    this.trails.clear();
    this.delivered.clear();
    this.cues.length = 0;
    this.seen = -1;
    this.placed = false;
    this.stillPresence = 0;
  }

  private sample(
    id: number,
    x: number,
    y: number,
    layer: number,
    controlled: boolean,
    step: number,
  ): void {
    let trail = this.trails.get(id);
    if (!trail) {
      trail = {
        points: new Float32Array(TRAIL_SAMPLES * 2),
        count: 0,
        head: 0,
        layer,
        controlled,
        step,
      };
      this.trails.set(id, trail);
    }
    trail.points[trail.head * 2] = x;
    trail.points[trail.head * 2 + 1] = y;
    trail.head = (trail.head + 1) % TRAIL_SAMPLES;
    if (trail.count < TRAIL_SAMPLES) trail.count += 1;
    trail.layer = layer;
    trail.controlled = controlled;
    trail.step = step;
  }

}
