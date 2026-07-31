/**
 * The Pulse: press and hold, then release — from a device to the frame the
 * worker reads.
 *
 * `docs/field-framework/ARCHITECTURE.md` freezes the two fields this source
 * fills. `InputFrame` carries `pulse_held` and `pulse_release`, one frame per
 * rendered frame, and `ControlState` records both with every step the trace
 * holds. `pulse_held` is a level — the control is held at the moment the frame
 * is taken — and `pulse_release` is an edge — the control was let go of since
 * the previous frame. The `InputConfig` defaults bind the Pulse to `ShiftLeft`,
 * and `docs/field-framework/SPEC.md` names the two devices: holding the primary
 * button focuses the Form's pull and releasing it emits a Pulse, with Shift
 * charging a Pulse and releasing Shift emitting it.
 *
 * They reach the frame through one source, for the same reason steering does:
 * a device names a hold, and the hold is one control however many devices are
 * on it. The frame a press produces is therefore the same frame whichever
 * device pressed, which is what the parity test reads.
 *
 * Nothing here reads a clock and nothing here decides what a Pulse does. The
 * edge is consumed exactly once, by the one frame that carries it, so a
 * recorded trace of device events replays to the same frames; what the Field
 * makes of those frames is the core's, and the rules it would need are the ones
 * this goal reports as unlocked.
 */

/**
 * The two Pulse fields of an `InputFrame`, ready to be spread into one.
 *
 * An object type rather than an interface, exactly as the steering pair is:
 * excess-property checking then guards what is spread into the frame, and the
 * implicit index signature is what lets it stand where a `Payload` is wanted.
 */
export type PulsePair = {
  pulse_held: boolean;
  pulse_release: boolean;
};

/**
 * The keys the Pulse is bound to: the locked `InputConfig` default `ShiftLeft`,
 * and the other Shift beside it. SPEC.md names the fallback as Shift rather
 * than as one of them, and a player pressing the right-hand Shift is pressing
 * Shift — the same reason the arrow keys stand beside WASD in steering. A
 * remapping surface reads the configured binding instead, which the goal that
 * owns accessibility adds once `InputConfig` crosses to the shell.
 */
export const PULSE_BINDINGS: readonly string[] = ['ShiftLeft', 'ShiftRight'];

/**
 * The primary pointer button. Right-click is not used at all, so no other
 * button holds the Pulse and no other button releases it.
 */
const PRIMARY_BUTTON = 0;

/** What the pointer's own hold is called among the holders. */
const POINTER_HOLDER = 'pointer';

export interface PulseOptions {
  /** Where the listeners are attached. Replaced in tests. */
  target?: EventTarget;
}

export interface Pulse {
  /**
   * The Pulse fields for the next frame, and the one place the release edge is
   * consumed: called exactly once per emitted `InputFrame`.
   */
  sample: () => PulsePair;
  /**
   * Lets go of the Pulse without emitting one — the safe release.
   *
   * The locked focus-loss rule clears every held input and sends one neutral
   * `InputFrame` with the pause level, and a neutral frame carries both fields
   * false. So a window that blurs mid-charge drops the charge rather than
   * emitting a Pulse the player never released: the pending edge goes with the
   * hold, and the frame that follows says nothing was held and nothing was let
   * go of.
   */
  clear: () => void;
  /** How many devices hold the Pulse. A diagnostic, and what a test reads. */
  held: () => number;
  /** Stops listening. */
  close: () => void;
}

/**
 * Opens a Pulse source over the pointer and the keyboard.
 *
 * No pointer lock is taken and no button beyond the primary one is read.
 */
export function openPulse(options: PulseOptions = {}): Pulse {
  const viewport = typeof window === 'undefined' ? null : window;
  const target = options.target ?? viewport;

  /** Which devices hold the Pulse, by their own name. */
  const holders = new Set<string>();
  /** Whether the last of them let go since the previous frame. */
  let released = false;

  /** Takes a hold. Two devices on it at once is one hold, not two. */
  function hold(holder: string): void {
    holders.add(holder);
  }

  /**
   * Lets one hold go. The edge is raised when the last holder lets go, so
   * pressing Shift while the button is held and letting one of them go carries
   * on charging rather than emitting.
   */
  function letGo(holder: string): void {
    if (!holders.delete(holder)) return;
    if (holders.size === 0) released = true;
  }

  const onPointerDown = (event: Event): void => {
    const at = event as PointerEvent;
    if (at.button !== PRIMARY_BUTTON) return;
    hold(POINTER_HOLDER);
  };

  const onPointerUp = (event: Event): void => {
    const at = event as PointerEvent;
    // `button` on a release names the button that changed, so a secondary
    // button coming up while the primary is held releases nothing.
    if (at.type === 'pointerup' && at.button !== PRIMARY_BUTTON) return;
    letGo(POINTER_HOLDER);
  };

  const onKeyDown = (event: Event): void => {
    const key = event as KeyboardEvent;
    // A shortcut is the platform's, not a Pulse: a key that arrives with
    // another modifier held is never taken, so its release lets nothing go.
    if (key.metaKey || key.ctrlKey || key.altKey) return;
    if (key.repeat || !PULSE_BINDINGS.includes(key.code)) return;
    hold(key.code);
  };

  const onKeyUp = (event: Event): void => {
    letGo((event as KeyboardEvent).code);
  };

  if (target) {
    target.addEventListener('pointerdown', onPointerDown, { passive: true });
    target.addEventListener('pointerup', onPointerUp, { passive: true });
    target.addEventListener('pointercancel', onPointerUp, { passive: true });
    target.addEventListener('keydown', onKeyDown);
    target.addEventListener('keyup', onKeyUp);
  }

  return {
    sample() {
      // A press and a release between two frames still carries its edge: the
      // frame reports nothing held and one release, which is a Pulse let go of
      // the moment it was taken.
      const pair: PulsePair = { pulse_held: holders.size > 0, pulse_release: released };
      released = false;
      return pair;
    },
    clear() {
      holders.clear();
      released = false;
    },
    held: () => holders.size,
    close() {
      if (!target) return;
      target.removeEventListener('pointerdown', onPointerDown);
      target.removeEventListener('pointerup', onPointerUp);
      target.removeEventListener('pointercancel', onPointerUp);
      target.removeEventListener('keydown', onKeyDown);
      target.removeEventListener('keyup', onKeyUp);
    },
  };
}
