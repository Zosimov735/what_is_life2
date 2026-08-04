/**
 * Coupling: hold E to extend the Form's reach, then release to emit.
 *
 * `docs/field-framework/ARCHITECTURE.md` freezes the two fields this source
 * fills. `InputFrame` carries `pulse_held` and `pulse_release`, one frame per
 * rendered frame, and `ControlState` records both with every step the trace
 * holds. `pulse_held` is a level — the control is held at the moment the frame
 * is taken — and `pulse_release` is an edge — the control was let go of since
 * the previous frame. The input is deliberately keyboard-only during active
 * play: pointer movement and clicks remain available for inspection and chrome,
 * while E is the one explicit world action.
 *
 * They reach the frame through one source, for the same reason steering does:
 * a key names a hold, and the edge is consumed once by the next sampled frame.
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
 * E is a deliberate action key rather than a movement modifier. Shift is not a
 * hidden alias: one visible verb has one default key.
 */
export const PULSE_BINDINGS: readonly string[] = ['KeyE'];

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
 * Opens the keyboard coupling source. Pointer input is inspection-only.
 */
export function openPulse(options: PulseOptions = {}): Pulse {
  const viewport = typeof window === 'undefined' ? null : window;
  const target = options.target ?? viewport;

  /** Which devices hold the Pulse, by their own name. */
  const holders = new Set<string>();
  /** Whether the last of them let go since the previous frame. */
  let released = false;

  /** Takes the keyboard hold. */
  function hold(holder: string): void {
    holders.add(holder);
  }

  /**
   * Lets the hold go and raises one release edge.
   */
  function letGo(holder: string): void {
    if (!holders.delete(holder)) return;
    if (holders.size === 0) released = true;
  }

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
      target.removeEventListener('keydown', onKeyDown);
      target.removeEventListener('keyup', onKeyUp);
    },
  };
}
