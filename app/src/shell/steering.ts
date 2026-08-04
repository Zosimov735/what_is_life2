/**
 * Steering: keyboard input into one normalized control.
 *
 * `docs/field-framework/ARCHITECTURE.md` locks the shape of this mapping and
 * two of its numbers. The normalization is `magnitude = min(1, r / 320)` with
 * r in CSS px; keyboard
 * steering uses unit axes, whose diagonal components the same section writes as
 * ±23170 in Q1.15; each component crosses clamped to [−32767, 32767] with the
 * value −32768 never sent; and the vector's own magnitude is at most 32767
 * raw. WASD and the arrow keys both produce that same normalized form.
 *
 * A held key names an offset from the Form's place on the surface, and
 * `steerFrom` turns that offset into the control. The offset walks out to the
 * saturation radius over a few frames and returns on release.
 *
 * Three of the seven reference-feel numbers are here — the rest radius, and the
 * two ramp spans. The other four are the spring the core integrates them
 * against, in the steering block of `core/src/field.rs`. Change one of the
 * seven and the Field steers differently for every device at once, which is the
 * point of there being seven rather than one set per device.
 *
 * Nothing here reads a clock. A ramp advances once per emitted frame, so a
 * recorded trace of device events replays to the same frames, and the frames to
 * the same run.
 */

/**
 * The offset at which the control saturates, in CSS px. Locked: at or beyond
 * it the control is whole, and inside it the magnitude is the fraction of the
 * way out.
 */
export const SATURATION_RADIUS_PX = 320;

/** The widest component a frame may carry. The value −32768 is never sent. */
export const STEER_UNIT = 32_767;

/**
 * The radius inside which an input offset asks for nothing, in CSS px: a
 * fortieth of the saturation radius.
 *
 * It is small enough that
 * the step at its edge — a fortieth of the speed range — is below what the
 * surface shows, and the spring smooths even that.
 */
export const REST_RADIUS_PX = 8;

/** How many rendered frames a held key takes to reach full deflection. */
export const KEY_RISE_FRAMES = 8;

/** How many rendered frames a released key takes to come back to nothing. */
export const KEY_FALL_FRAMES = 10;

/**
 * The ramp's own span: the least common multiple of the two frame counts, so
 * both walk it in whole steps and the ramp is exact integer state rather than
 * an accumulating fraction.
 */
const KEY_RAMP_SPAN = 40;

/** What one frame adds to the ramp while a direction key is held. */
const KEY_RAMP_RISE = KEY_RAMP_SPAN / KEY_RISE_FRAMES;

/** What one frame takes off it while none is. */
const KEY_RAMP_FALL = KEY_RAMP_SPAN / KEY_FALL_FRAMES;

/**
 * The diagonal component of a whole keyboard deflection, as the locked text
 * writes it: ±23169 in Q1.15 — the largest per-axis pair the magnitude rule
 * admits, since 2 × 23169² fits under 32767² and 2 × 23170² does not.
 *
 * The keyboard reaches it rather than declaring it: a unit diagonal offset
 * through `steerFrom` rounds to 23170, and the magnitude clamp there holds the
 * pair to this. The constant is what the check against the document reads.
 */
export const KEY_DIAGONAL = 23_169;

/**
 * The keys each direction is bound to: the locked defaults of `InputConfig`,
 * beside the arrow keys the same specification names as the fallback. Remapping
 * reads the configured bindings instead, which the goal that owns accessibility
 * adds once `InputConfig` crosses to the shell; until then these are the same
 * codes the core defaults to, written where the shell can see them.
 */
const BINDINGS: Readonly<Record<string, { x: number; y: number }>> = {
  KeyW: { x: 0, y: -1 },
  KeyS: { x: 0, y: 1 },
  KeyA: { x: -1, y: 0 },
  KeyD: { x: 1, y: 0 },
  ArrowUp: { x: 0, y: -1 },
  ArrowDown: { x: 0, y: 1 },
  ArrowLeft: { x: -1, y: 0 },
  ArrowRight: { x: 1, y: 0 },
};

/**
 * The two steering fields of an `InputFrame`, ready to be spread into one.
 *
 * Written as an object type rather than an interface so it carries exactly two
 * fields and no third: excess-property checking then guards what is spread into
 * the frozen frame, and the implicit index signature an object type takes is
 * what lets it stand where the protocol's `Payload` is wanted.
 */
export type SteerPair = {
  steer_x: number;
  steer_y: number;
};

/**
 * The largest integer whose square is at most `value`, exactly. `Math.sqrt` is
 * correctly rounded, so the correction below runs at most once, and the result
 * is the integer root on every engine rather than whatever a rounding left.
 */
function integerRoot(value: number): number {
  let root = Math.floor(Math.sqrt(value));
  while (root > 0 && root * root > value) root -= 1;
  while ((root + 1) * (root + 1) <= value) root += 1;
  return root;
}

/**
 * Holds a pair inside the locked magnitude invariant: at most 32767 raw, on
 * every heading rather than only on the axes.
 *
 * The scale is taken on the exact integer root and the walk that follows closes
 * the last raw unit the truncation leaves, so the answer is the same on every
 * engine and the reader on the other side of the boundary never refuses it.
 */
function fitted(x: number, y: number): SteerPair {
  let held_x = Math.max(-STEER_UNIT, Math.min(STEER_UNIT, x));
  let held_y = Math.max(-STEER_UNIT, Math.min(STEER_UNIT, y));
  const limit = STEER_UNIT * STEER_UNIT;
  if (held_x * held_x + held_y * held_y > limit) {
    const root = integerRoot(held_x * held_x + held_y * held_y);
    held_x = Math.trunc((held_x * STEER_UNIT) / root);
    held_y = Math.trunc((held_y * STEER_UNIT) / root);
    while (held_x * held_x + held_y * held_y > limit) {
      held_x -= Math.sign(held_x);
      held_y -= Math.sign(held_y);
    }
  }
  return { steer_x: held_x, steer_y: held_y };
}

/**
 * Rounds away from zero, so the same offset mirrored gives the same component
 * mirrored. `Math.round` breaks a half toward positive infinity, which at an
 * exact half — half the saturation radius lands on 16383.5 — would make
 * steering left a raw unit weaker than steering right. Steering that is not
 * sign-symmetric is a defect however small it is.
 */
function rounded(value: number): number {
  const magnitude = Math.round(Math.abs(value));
  return value < 0 ? -magnitude : magnitude;
}

/**
 * The one normalization: an offset in CSS px, into the control the frame
 * carries.
 *
 * `magnitude = min(1, r / 320)` is the locked rule, and the rest radius is the
 * one thing added to it — inside that radius the control is nothing rather than
 * a fraction of nothing. Nothing rescales what is left: the edge of the rest
 * radius is a step from nothing to a fortieth of the range, which is half a
 * unit per step of speed the spring takes four steps to reach, and rescaling
 * the remainder would be a different rule from the locked one.
 *
 * Every arithmetic step is an operation IEEE 754 specifies exactly, so the same
 * offset gives the same pair on every engine and on every run.
 */
export function steerFrom(dx: number, dy: number): SteerPair {
  const reach = Math.sqrt(dx * dx + dy * dy);
  if (!(reach > REST_RADIUS_PX)) return { steer_x: 0, steer_y: 0 };
  const magnitude = Math.min(1, reach / SATURATION_RADIUS_PX);
  const scale = (magnitude * STEER_UNIT) / reach;
  return fitted(rounded(dx * scale), rounded(dy * scale));
}

/** Where the surface's middle stands, in CSS px inside the viewport. */
export interface Middle {
  x: number;
  y: number;
}

export interface SteeringOptions {
  /**
   * Kept for compatibility with callers that supplied the old pointer center.
   * Pointer position no longer contributes to steering.
   */
  middle?: () => Middle;
  /** Where the listeners are attached. Replaced in tests. */
  target?: EventTarget;
}

export interface Steering {
  /**
   * The control for the next frame, and the one place a ramp advances: called
   * exactly once per emitted `InputFrame`.
   */
  sample: () => SteerPair;
  /**
   * Clears every held key and the ramp between them. The locked focus-loss
   * rule calls this before the shell sends
   * its one neutral frame, beside the same call on the Pulse source, which
   * lets a held Pulse go without emitting one.
   */
  clear: () => void;
  /** How many direction keys are held. A diagnostic, and what a test reads. */
  held: () => number;
  /** Stops listening. */
  close: () => void;
}

/**
 * Opens keyboard steering. The pointer remains entirely available for Field
 * inspection and shell controls; moving or parking it never moves the Form.
 */
export function openSteering(options: SteeringOptions = {}): Steering {
  const viewport = typeof window === 'undefined' ? null : window;
  const target = options.target ?? viewport;
  /** Which direction keys are held, by their `KeyboardEvent.code`. */
  const keys = new Set<string>();
  /** The direction the keys last named, which a released ramp walks back on. */
  let heading: Middle = { x: 0, y: 0 };
  /** How far the keyboard's own offset has ramped, over the span. */
  let ramp = 0;

  const onKeyDown = (event: Event): void => {
    const key = event as KeyboardEvent;
    // A shortcut is the platform's, not a direction: the key that comes with a
    // modifier held is never added, so its release clears nothing.
    if (key.metaKey || key.ctrlKey || key.altKey) return;
    if (key.repeat || !(key.code in BINDINGS)) return;
    keys.add(key.code);
  };

  const onKeyUp = (event: Event): void => {
    keys.delete((event as KeyboardEvent).code);
  };

  if (target) {
    target.addEventListener('keydown', onKeyDown);
    target.addEventListener('keyup', onKeyUp);
  }

  /**
   * The offset the keyboard names, in CSS px, at the ramp it stands at.
   *
   * A held direction walks the offset out along its own axis to the saturation
   * radius over the rise. A released one keeps its heading while the ramp walks back:
   * the offset decays rather than snapping, and the spring on the other side of
   * the boundary carries the rest of the way to rest.
   */
  function keyed(): Middle {
    let x = 0;
    let y = 0;
    for (const code of keys) {
      x += BINDINGS[code].x;
      y += BINDINGS[code].y;
    }
    // Opposed keys cancel, which reads as no direction rather than as a
    // sideways one, and the ramp walks back as if both were released.
    const held = x !== 0 || y !== 0;
    if (held) {
      heading = { x, y };
      ramp = Math.min(KEY_RAMP_SPAN, ramp + KEY_RAMP_RISE);
    } else {
      ramp = Math.max(0, ramp - KEY_RAMP_FALL);
      if (ramp === 0) heading = { x: 0, y: 0 };
    }
    if (ramp === 0) return { x: 0, y: 0 };
    const length = Math.sqrt(heading.x * heading.x + heading.y * heading.y);
    const reach = (SATURATION_RADIUS_PX * ramp) / KEY_RAMP_SPAN;
    return { x: (heading.x / length) * reach, y: (heading.y / length) * reach };
  }

  return {
    sample() {
      const keyboard = keyed();
      return steerFrom(keyboard.x, keyboard.y);
    },
    clear() {
      keys.clear();
      heading = { x: 0, y: 0 };
      ramp = 0;
    },
    held: () => keys.size,
    close() {
      if (target) {
        target.removeEventListener('keydown', onKeyDown);
        target.removeEventListener('keyup', onKeyUp);
      }
    },
  };
}
