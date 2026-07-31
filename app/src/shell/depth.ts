/**
 * Depth: the wheel, the two-finger vertical gesture, and the bracket keys —
 * from a device to the frame the worker reads.
 *
 * `docs/field-framework/ARCHITECTURE.md` freezes the two fields this source
 * fills and everything downstream of them. `InputFrame` carries `wheel`, the
 * raw wheel delta sum since the previous frame clamped to [−3000, 3000], and
 * `depth_key`, one of −1, 0, or +1. The core owns the thresholding: the delta
 * accumulates into `wheel_accum`, one depth change resolves at ±480, and a
 * 15-step cooldown holds off the next. Nothing here decides when depth changes;
 * this decides what the player asked for.
 *
 * **The deferral lives here, and has to.** A rendered frame executes no step
 * about half the time at the 60-frames-per-second target against 30 steps per
 * second, and the core resolves depth only on a frame that executes one — so a
 * press carried by a stepless frame is not consumed by it. This source
 * therefore offers a press again on the frames that follow, until the answer to
 * a frame that carried it says a step ran. The wheel needs no such re-offer:
 * its delta accumulates into `wheel_accum`, which is payload state and carries
 * the gesture forward by itself.
 *
 * The core must not hold the press instead, and that is the whole reason this
 * is here: `export_run` is reachable at any instant, the shell's own recovery
 * capture takes one on the first frame it sees, and a record written while the
 * core held an unrecorded press would be a record a restore could not land on —
 * the restored run and the run it came from would resolve different depth and
 * diverge with no byte to say why. Everything this source holds is spent into
 * the frames it fills, and the frames are the record.
 *
 * Three things this source is, and one it is not:
 *
 * - **The two-finger gesture is the wheel.** A trackpad's two-finger vertical
 *   swipe arrives as `wheel` events, on every platform this game targets. There
 *   is no separate gesture interface to read and none is used, so the trackpad
 *   and the mouse reach depth through one path and cannot drift apart.
 * - **The sign is the platform's.** `deltaY` is passed through as it arrives, so
 *   the direction that reads as downward under the player's own system setting —
 *   natural scrolling or not — is the direction that goes deeper. The locked
 *   rule reads `sign(wheel_accum)`, and +1 is one layer deeper.
 * - **A wheel over the play surface is consumed.** `preventDefault` is called on
 *   every wheel event over the surface, both axes, and the listener is
 *   registered non-passive so that call is honored: the page never scrolls, and
 *   no scroll-anchored gesture (rubber-banding, overscroll navigation, the
 *   horizontal swipe some platforms read as going back) can be started by
 *   steering. The one exception is the platform's zoom gesture, which arrives
 *   as a wheel with ctrl or meta held: it is left alone in both senses, so a
 *   player can still zoom the page and zooming never moves the Form. A wheel
 *   over the surrounding chrome is left entirely alone too — not consumed and
 *   not accumulated — so anything the shell ever puts beside the surface
 *   scrolls exactly as the platform scrolls it.
 * - **It is not a repeat.** One bracket press names one depth change. A held
 *   key repeats nothing and the platform's own key repeat is ignored, for the
 *   same reason the wheel has a threshold: depth is a difficulty choice, and a
 *   choice is made once per press.
 *
 * Nothing here reads a clock, and nothing here holds a device event for longer
 * than the one frame that carries it: a recorded trace of device events replays
 * to the same frames, and the frames to the same run.
 */

/**
 * The two depth fields of an `InputFrame`, ready to be spread into one.
 *
 * An object type rather than an interface, exactly as the steering and Pulse
 * pairs are: excess-property checking then guards what is spread into the
 * frame, and the implicit index signature is what lets it stand where a
 * `Payload` is wanted.
 */
export type DepthPair = {
  wheel: number;
  depth_key: number;
};

/** The widest delta sum one frame may carry, either way. Locked. */
export const WHEEL_LIMIT = 3_000;

/**
 * What one line and one page of a wheel delta are worth in CSS px.
 *
 * A wheel event reports its delta in one of three units, and which one is the
 * platform's business: pixels almost everywhere, lines on some Firefox builds,
 * pages on some remote desktops. The locked trigger is a distance, so the units
 * have to be one unit before they reach it — otherwise the same gesture would
 * need 30 notches on one browser and 3 on another. A line is a line of the
 * shell's own 16 px text; a page is 25 of them.
 */
export const WHEEL_LINE_PX = 16;
export const WHEEL_PAGE_PX = 25 * WHEEL_LINE_PX;

/**
 * The keys depth is bound to: the locked `InputConfig` defaults `BracketLeft`
 * for the way up and `BracketRight` for the way down, which is also the order
 * they sit on the keyboard in. A remapping surface reads the configured binding
 * instead, which the goal that owns accessibility adds once `InputConfig`
 * crosses to the shell; until then these are the codes the core defaults to,
 * written where the shell can see them.
 */
export const DEPTH_BINDINGS: Readonly<Record<string, number>> = {
  BracketLeft: -1,
  BracketRight: 1,
};

/**
 * The selector naming the play surface, whose wheel events are consumed. The
 * element the shell mounts is asserted against it, so the two cannot drift.
 */
export const PLAY_SURFACE = 'canvas.field';

export interface DepthOptions {
  /** Where the listeners are attached. Replaced in tests. */
  target?: EventTarget;
  /** What counts as the play surface. Replaced in tests. */
  surface?: string;
}

export interface Depth {
  /**
   * The depth fields for the next frame, and the one place the wheel sum is
   * consumed: called exactly once per emitted `InputFrame`.
   *
   * A press is offered rather than consumed. It is offered on one frame at a
   * time — the frame that carries it is the frame that may resolve it, and
   * offering it again before that frame is answered could have two frames in
   * flight carrying the same press and two steps resolving it.
   */
  sample: () => DepthPair;
  /**
   * The answer to the frame that carried the offered press, as its steps_run.
   * A frame that executed a step consumed the press, so the hold is let go of;
   * a frame that executed none did not, so the next frame offers it again.
   *
   * Called exactly once per offered frame, by the client that owns the frame
   * numbers — including for a frame that was refused or lost, which counts as
   * no step and so keeps the press.
   */
  settle: (stepsRun: number) => void;
  /**
   * Lets go of everything held: the wheel delta not yet carried, the press not
   * yet resolved, and the keys themselves.
   *
   * The locked focus-loss rule clears every held input and sends one neutral
   * `InputFrame` with the pause level. A gesture half made when the window went
   * away is not finished on its return, and a suspended run reads nothing, so
   * an offer outstanding at that moment is dropped rather than left waiting for
   * an answer that a stopped pump will never bring.
   */
  clear: () => void;
  /** How many depth keys are held. A diagnostic, and what a test reads. */
  held: () => number;
  /** Stops listening, and lets go of what was held. */
  close: () => void;
}

/**
 * Opens a depth source over the wheel and the keyboard.
 *
 * The wheel listener stands on the same target the keys do rather than on the
 * surface element, because the surface is built and rebuilt by the component
 * that draws on it while this source stands for the life of the session. Which
 * events belong to the surface is read off the event itself.
 */
export function openDepth(options: DepthOptions = {}): Depth {
  const viewport = typeof window === 'undefined' ? null : window;
  const target = options.target ?? viewport;
  const surface = options.surface ?? PLAY_SURFACE;

  /**
   * The delta sum since the previous frame, in CSS px, fraction and all: a
   * trackpad reports sub-pixel deltas, and a frame carries whole units, so the
   * fraction is kept here rather than truncated away one frame at a time.
   */
  let carried = 0;
  /** The direction the newest press named, and 0 once a step has consumed it. */
  let pressed = 0;
  /** Whether a frame carrying that press is waiting to be answered. */
  let offered = false;
  /** Which depth keys are held, by their `KeyboardEvent.code`. */
  const keys = new Set<string>();

  /** Whether an event stands over the play surface. */
  function overSurface(event: Event): boolean {
    const at = event.target;
    return at instanceof Element && at.closest(surface) !== null;
  }

  /** One wheel event's vertical delta, in CSS px whatever unit it arrived in. */
  function pixels(event: WheelEvent): number {
    if (event.deltaMode === 1) return event.deltaY * WHEEL_LINE_PX;
    if (event.deltaMode === 2) return event.deltaY * WHEEL_PAGE_PX;
    return event.deltaY;
  }

  const onWheel = (event: Event): void => {
    const turn = event as WheelEvent;
    // A wheel carrying a platform modifier is the platform's own gesture — the
    // zoom — and not a depth change. It is left alone entirely: not consumed,
    // so the player can still zoom the page, and not accumulated, so zooming
    // never moves the Form.
    if (turn.ctrlKey || turn.metaKey) return;
    if (!overSurface(turn)) return;
    // The whole event is consumed, both axes, and only the vertical part is
    // read. The horizontal part is deliberately swallowed rather than left to
    // the platform: on macOS a shift-held wheel and a two-finger sideways swipe
    // both arrive here as `deltaX` with no `deltaY`, and both would otherwise
    // rubber-band the page or take a browser back through its history under a
    // player who was steering. Swallowing what it does not read is the point —
    // over the play surface, a wheel does depth or it does nothing.
    if (turn.cancelable) turn.preventDefault();
    carried += pixels(turn);
  };

  const onKeyDown = (event: Event): void => {
    const key = event as KeyboardEvent;
    // A shortcut is the platform's, not a depth change: a key that arrives with
    // a modifier held is never taken, so its release lets nothing go. The
    // platform's own key repeat is not taken either — one press is one change.
    if (key.metaKey || key.ctrlKey || key.altKey) return;
    if (key.repeat || !(key.code in DEPTH_BINDINGS)) return;
    keys.add(key.code);
    // The direction is the net of what is held, so both brackets held at once
    // ask for nothing rather than for the one pressed second. A press made
    // while an earlier one is still waiting on its answer takes a frame of its
    // own rather than queueing behind it: two presses are two changes, which is
    // what a player who pressed twice asked for.
    pressed = direction();
    offered = false;
  };

  const onKeyUp = (event: Event): void => {
    keys.delete((event as KeyboardEvent).code);
  };

  /** The direction the held keys name: −1 up, +1 down, 0 for both or neither. */
  function direction(): number {
    let named = 0;
    for (const code of keys) named += DEPTH_BINDINGS[code];
    return Math.sign(named);
  }

  /** Lets go of everything held. One function, so `clear` and `close` agree. */
  function letGoOfHeld(): void {
    carried = 0;
    pressed = 0;
    offered = false;
    keys.clear();
  }

  if (target) {
    // Not passive: the whole point of the listener is that it may consume the
    // event, and a passive listener's `preventDefault` is ignored with a
    // console warning. Wheel listeners on the window default to passive on
    // every browser that has the optimization, so this is stated rather than
    // left to the default.
    target.addEventListener('wheel', onWheel, { passive: false });
    target.addEventListener('keydown', onKeyDown);
    target.addEventListener('keyup', onKeyUp);
  }

  return {
    sample() {
      // Whole units cross and the fraction stays: a slow trackpad gesture that
      // reports a third of a pixel a frame still reaches the trigger, and it
      // reaches it exactly where the deltas say it does. What the locked clamp
      // cuts off is dropped rather than carried forward — a single frame past
      // the limit is already six times the trigger distance, and carrying the
      // excess would turn one flick into a second change a frame later.
      const whole = Math.trunc(carried);
      carried -= whole;
      const wheel = Math.max(-WHEEL_LIMIT, Math.min(WHEEL_LIMIT, whole));
      // A press stands until a step consumes it, and is offered on one frame at
      // a time: the frame this fills is the one whose answer decides it.
      const key = offered ? 0 : pressed;
      if (key !== 0) offered = true;
      return { wheel, depth_key: key };
    },
    settle(stepsRun) {
      if (!offered) return;
      offered = false;
      if (stepsRun > 0) pressed = 0;
    },
    clear: letGoOfHeld,
    held: () => keys.size,
    close() {
      // Nothing outlives the source: a press still standing when the session
      // ends is not carried into whatever opens next.
      letGoOfHeld();
      if (!target) return;
      target.removeEventListener('wheel', onWheel);
      target.removeEventListener('keydown', onKeyDown);
      target.removeEventListener('keyup', onKeyUp);
    },
  };
}
