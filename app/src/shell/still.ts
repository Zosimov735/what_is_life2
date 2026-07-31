/**
 * Still Mode's three keys — from a device to the frame and the commands the
 * worker reads.
 *
 * `docs/field-framework/SPEC.md` names them and `InputConfig`'s locked
 * bindings spell them: Space toggles Still Mode, Enter commits the queued
 * changes, and Escape removes the most recent one — a second Escape, on a
 * queue with nothing left in it, exits without committing.
 *
 * The three do not reach the worker the same way, and that division is the
 * protocol's rather than a choice made here:
 *
 * - **Space is a frame field.** `InputFrame.toggle_still` is an edge, consumed
 *   exactly once by the one frame that carries it, exactly as the Pulse's
 *   release edge is. What it does is the mode table's business.
 * - **Enter and Escape are commands.** `commit_plan` and `undo_plan` are two
 *   of the nine, valid only in `still`, and each takes an answer the shell
 *   reads — which is how the second Escape knows it is the second one: the
 *   undo reports what remains, and a queue with nothing remaining is a queue
 *   the next Escape leaves rather than empties.
 *
 * So this source holds an edge for the frame and a short queue of intents for
 * the client to spend, and decides nothing about either. Nothing here reads a
 * clock, and nothing here reads the mode: which intents are worth sending is
 * the client's, because the client is what holds the snapshots.
 *
 * One rule about focus, and it is the reason the keys are read here at all
 * rather than on the surface element: a key aimed at a control the shell put
 * on the page belongs to that control. `Why?` is a button, and a button that
 * has focus answers Space and Enter itself; taking either out from under it
 * would leave a keyboard player with a control they cannot press. So a key
 * event whose target is an interactive element is left entirely alone.
 */

/** The one Still Mode field of an `InputFrame`, ready to be spread into one. */
export type StillPair = {
  toggle_still: boolean;
};

/** What Enter and Escape ask for, as the client spends them. */
export type StillIntent = 'commit' | 'undo';

/**
 * The keys the three are bound to: the locked `InputConfig` defaults `Space`,
 * `Enter`, and `Escape`. A remapping surface reads the configured binding
 * instead, which the goal that owns accessibility adds once `InputConfig`
 * crosses to the shell; until then these are the codes the core defaults to,
 * written where the shell can see them.
 */
export const STILL_BINDING = 'Space';
export const COMMIT_BINDING = 'Enter';
export const CANCEL_BINDING = 'Escape';

/**
 * How many intents wait at once. Two is enough for the one sequence that
 * produces them faster than frames answer — Escape, Escape — and a third
 * press before either is answered is a press the player cannot have meant.
 */
const INTENT_DEPTH = 2;

/**
 * Elements that answer a key themselves, and so are never read from.
 *
 * A negative `tabindex` is the one exclusion: it names an element that can be
 * focused programmatically but is not in the tab order and answers no key of
 * its own, so a key that happened to land on one is still the mode's.
 */
const INTERACTIVE =
  'a[href], button, input, select, textarea, [contenteditable], [tabindex]:not([tabindex="-1"])';

export interface StillOptions {
  /** Where the listeners are attached. Replaced in tests. */
  target?: EventTarget;
  /** What counts as a control that answers its own keys. Replaced in tests. */
  interactive?: string;
}

export interface Still {
  /**
   * The Still Mode field for the next frame, and the one place the toggle edge
   * is consumed: called exactly once per emitted `InputFrame`.
   */
  sample: () => StillPair;
  /** The oldest intent still waiting, and none while none waits. */
  takeIntent: () => StillIntent | null;
  /** Asks for one exit, as the second Escape does once a queue is empty. */
  exit: () => void;
  /**
   * Drops every intent not yet spent, keeping whatever toggle is waiting.
   *
   * An intent is only ever worth sending while the run is paused, because
   * `commit_plan` and `undo_plan` are valid in `still` and nowhere else. So an
   * intent still waiting as Still Mode is left is an intent that would be
   * spent into the *next* inspection — a second Escape pressed to leave one
   * pause ejecting the player from the one after it. It goes with the mode,
   * and the toggle does not: a toggle pressed as the mode changes is a
   * reversal, and reversals are the one thing a ramp answers.
   */
  dropIntents: () => void;
  /**
   * Lets go of everything held: the toggle not yet carried and every intent
   * not yet spent.
   *
   * The locked focus-loss rule clears every held input and sends one neutral
   * `InputFrame` with the pause level. A run that was inspecting is suspended
   * by that frame, so an intent outstanding at that moment would be a command
   * sent into a state that no longer admits it.
   */
  clear: () => void;
  /** How many intents wait. A diagnostic, and what a test reads. */
  waiting: () => number;
  /** Stops listening, and lets go of what was held. */
  close: () => void;
}

/** Opens a Still Mode source over the keyboard. */
export function openStill(options: StillOptions = {}): Still {
  const viewport = typeof window === 'undefined' ? null : window;
  const target = options.target ?? viewport;
  const interactive = options.interactive ?? INTERACTIVE;

  /** Whether a toggle is waiting for a frame to carry it. */
  let toggling = false;
  /** The intents waiting to be spent, oldest first. */
  const intents: StillIntent[] = [];

  /** Whether a key event belongs to a control that answers it itself. */
  function ownedByAControl(event: KeyboardEvent): boolean {
    const at = event.target;
    return at instanceof Element && at.closest(interactive) !== null;
  }

  function want(intent: StillIntent): void {
    if (intents.length >= INTENT_DEPTH) return;
    intents.push(intent);
  }

  const onKeyDown = (event: Event): void => {
    const key = event as KeyboardEvent;
    // A shortcut is the platform's: a key that arrives with a modifier held is
    // never taken. The platform's own key repeat is not taken either — one
    // press is one toggle, one commit, or one undo.
    if (key.metaKey || key.ctrlKey || key.altKey || key.repeat) return;
    if (key.code !== STILL_BINDING && key.code !== COMMIT_BINDING && key.code !== CANCEL_BINDING) {
      return;
    }
    if (ownedByAControl(key)) return;
    // Space scrolls a page and Enter submits a form. Neither is what the key
    // means here, and both are the platform's default rather than a control's,
    // so the event is consumed where it is taken.
    if (key.cancelable) key.preventDefault();
    if (key.code === STILL_BINDING) {
      toggling = true;
      return;
    }
    want(key.code === COMMIT_BINDING ? 'commit' : 'undo');
  };

  if (target) {
    target.addEventListener('keydown', onKeyDown);
  }

  /** Lets go of everything held. One function, so `clear` and `close` agree. */
  function letGoOfHeld(): void {
    toggling = false;
    intents.length = 0;
  }

  return {
    sample() {
      const pair: StillPair = { toggle_still: toggling };
      toggling = false;
      return pair;
    },
    takeIntent: () => intents.shift() ?? null,
    exit() {
      toggling = true;
    },
    dropIntents() {
      intents.length = 0;
    },
    clear: letGoOfHeld,
    waiting: () => intents.length,
    close() {
      letGoOfHeld();
      if (!target) return;
      target.removeEventListener('keydown', onKeyDown);
    },
  };
}
