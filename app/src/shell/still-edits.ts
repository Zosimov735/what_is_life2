/**
 * The drags a paused Field takes, and the entries they queue.
 *
 * The four causal variants are exposed directly: dragging between Ports queues
 * a connection, dragging a Route endpoint redirects it, selecting a Route and
 * pressing Delete or Backspace queues a cut, and dragging a material handle
 * reshapes the physical compartment. Candidate keys move the passive View
 * immediately and never enter this queue.
 *
 * What a handle is, is not decided here. `docs/field-framework/ARCHITECTURE.md`
 * calls the inspection surface the `PlanCommand` union read back as places on
 * the Field, and the renderer is what puts those places on the surface: one
 * handle per Port, one at each end of every Route, one at every vertex of the
 * physical compartment. So this reads the scene the renderer drew and takes
 * hold of the handle the pointer is over — the same handle the player can see,
 * at the place it was actually drawn, rather than a second projection that
 * could disagree with it.
 *
 * Nothing here validates anything. Every precondition is the core's, checked
 * against the base state with every earlier queued entry applied, so a drag
 * that cannot stand is refused by the answer rather than predicted here. And
 * nothing here holds a queue: the queue lives in the worker, and what the tray
 * shows is what the worker last reported.
 */

import { HANDLE_KIND, type HandleMark, type Scene } from '../render';
import type { PlanCommand } from '../../../worker/src/protocol';

/** The keys that take a selected Route away, as the specification names them. */
export const CUT_BINDINGS: readonly string[] = ['Delete', 'Backspace'];

/**
 * The keys that walk the candidates of the standing slate, forward and back.
 *
 * The locked bindings name Space, Enter, Escape, the four steering keys, the
 * Pulse, the two depth keys, and the two cut keys, and none of them is a
 * cycle — so the arrows take it, documented here and in
 * `docs/field-framework/ARCHITECTURE.md`. Down and Right walk the presentation
 * order forward, Up and Left walk it back, and both wrap: the order is the
 * slate's own, assembly order, and every position is reachable either way.
 *
 * Tab remains available for the explicit tray controls; the arrow gesture is a
 * direct Field shortcut and ignores focused interactive chrome.
 *
 * A player who rebinds the steering keys onto the arrows collides with this
 * walk; the goal that owns key remapping (Goal 31) resolves that collision, as
 * it must for the cut keys already — a pre-existing pattern, left as it stands.
 */
export const CANDIDATE_FORWARD: readonly string[] = ['ArrowDown', 'ArrowRight'];
export const CANDIDATE_BACK: readonly string[] = ['ArrowUp', 'ArrowLeft'];

/** The two explicitly separate Still Mode tools. */
export type StillTool = 'view' | 'compartment';

/**
 * How far past its own radius a handle may be taken hold of, as a multiple of
 * that radius. A handle is small and a pointer is not exact; one and a half
 * radii is close enough to be deliberate and far enough to be reachable.
 */
const GRASP = 1.5;

/** One place on the surface, in device pixels — the scene's own units. */
export interface Spot {
  x: number;
  y: number;
}

/**
 * The handle nearest a place, and none when nothing is within reach of it.
 *
 * Ties go to the nearest, and then to the handle drawn later: a Route handle
 * sits a short way in from its end precisely so it does not stand under the
 * Port handle already there, and where two still overlap the one on top is the
 * one a player is aiming at.
 */
export function handleAt(scene: Scene, spot: Spot): HandleMark | null {
  let found: HandleMark | null = null;
  let nearest = Infinity;
  for (let place = 0; place < scene.handles.count; place += 1) {
    const handle = scene.handles.items[place];
    if (handle.alpha <= 0.01) continue;
    const dx = handle.x - spot.x;
    const dy = handle.y - spot.y;
    const distance = Math.sqrt(dx * dx + dy * dy);
    if (distance > handle.radius * GRASP) continue;
    if (distance <= nearest) {
      nearest = distance;
      found = handle;
    }
  }
  return found;
}

/**
 * The standing physical compartment, as the scene's own marks report it — the
 * causal membership the frame carried, read back in ascending Node order.
 */
export function standingCompartment(scene: Scene): number[] {
  return membersOf(scene, false);
}

/** Compatibility name; V2 treats this as physical compartment membership. */
export const standingInside = standingCompartment;

/**
 * The physical membership a queued change would leave, and none while the
 * queue proposes no compartment edit. The frame carries it beside the standing
 * physical membership, independently of the View bitset.
 */
export function proposedCompartment(scene: Scene): number[] | null {
  const held = membersOf(scene, true);
  return held.length > 0 ? held : null;
}

/** Compatibility name; V2 treats this as a physical causal preview. */
export const proposedInside = proposedCompartment;

function membersOf(scene: Scene, proposed: boolean): number[] {
  const held: number[] = [];
  for (let place = 0; place < scene.ports.count; place += 1) {
    const port = scene.ports.items[place];
    if (proposed ? port.proposedMember : port.member) held.push(port.node);
  }
  return held.sort((first, second) => first - second);
}

/**
 * The entry one completed drag proposes, and none for a drag that proposes
 * nothing.
 *
 * The four rules are the specification's, one per handle kind the drag started
 * on:
 *
 * - **Port to Port** — a connection, from the one taken hold of to the one let
 *   go over.
 * - **Route end to Port** — that end of that Route, moved to the Port.
 * - **Compartment vertex to Port** — the member the vertex is drawn around,
 *   replaced by the Port let go over. Dropping it on a Node already inside
 *   takes the dragged member out instead, because a set with the same Node
 *   twice is the same set: the member set is what the drag leaves, and intake
 *   is what the core does with it.
 * - Anything else — nothing. A drag that ends on empty space, or on a handle of
 *   a kind the rule does not name, proposes no change at all.
 */
export function planFromDrag(
  scene: Scene,
  from: HandleMark,
  to: HandleMark | null,
): PlanCommand | null {
  if (!to || to.kind !== HANDLE_KIND.port || to.node === 0) return null;
  if (from.kind === HANDLE_KIND.port) {
    if (from.node === 0 || from.node === to.node) return null;
    return { op: 'connect', from: from.node, to: to.node };
  }
  if (from.kind === HANDLE_KIND.route) {
    if (from.route === 0) return null;
    return { op: 'redirect', route: from.route, end: from.end === 0 ? 'tail' : 'head', to: to.node };
  }
  if (from.kind === HANDLE_KIND.boundary) {
    if (from.node === 0) return null;
    // The set the drag starts from is the one the queue would leave, not the
    // one standing: a second drag in the same pause builds on the first, which
    // is what the core validates the entry against — it projects every earlier
    // entry before it reads this one. Reading the standing inside instead would
    // propose a set that quietly undid the reshape already queued.
    const base = proposedCompartment(scene) ?? standingCompartment(scene);
    const members = base.filter((node) => node !== from.node);
    if (!members.includes(to.node)) members.push(to.node);
    if (members.length === 0) return null;
    return { op: 'reshape_compartment', members: members.sort((first, second) => first - second) };
  }
  return null;
}

export interface StillEditsOptions {
  /** The surface drags are read on. */
  surface: HTMLCanvasElement;
  /** Where the keys are read. The window, so a press anywhere is heard. */
  keys?: EventTarget;
  /** The scene the newest frame drew, read fresh on every event. */
  scene: () => Scene;
  /** Whether the run is paused, and so whether a drag means anything at all. */
  paused: () => boolean;
  /** Which explicit tool is active. Compartment by default for old callers. */
  tool?: () => StillTool;
  /** Where a proposed entry is sent. */
  queue: (plan: PlanCommand) => void;
  /**
   * The standing slate, as the keys need to read it: the ordinal a `set_focus`
   * names, how many candidates stand in it, and whether it is deficient. None
   * while the run stands under no slate, which is every moment before the
   * first entry into Still Mode.
   */
  slate?: () => SlateReading | null;
  /**
   * The candidate the active passive View matches, 1-based, and 0 while none
   * matches.
   *
   * It is read from the worker's active View rather than from the causal queue.
   */
  focused?: () => number;
  /** Moves the passive View immediately, outside the causal plan queue. */
  focus?: (slateOrdinal: number, position: number) => void;
  /**
   * Legacy causal undo sink retained for callers that also expose queue edits;
   * passive View movement never calls it.
   */
  undo?: () => void;
}

/** What the candidate keys read of the standing slate. */
export interface SlateReading {
  ordinal: number;
  count: number;
  deficient: boolean;
}

export interface StillEdits {
  /** The Route a press would take away, and none while none is selected. */
  selected: () => number;
  /** Lets go of the Route selected and of any drag in flight. */
  clear: () => void;
  /** Stops listening. */
  close: () => void;
}

/** Opens the drag source over one surface. */
export function openStillEdits(options: StillEditsOptions): StillEdits {
  const { surface, scene, paused, queue } = options;
  const tool = options.tool ?? (() => 'compartment');
  const keys = options.keys ?? (typeof window === 'undefined' ? null : window);

  /** The handle a drag started on, and none while no drag is in flight. */
  let held: HandleMark | null = null;
  /** The Route a press would cut, and 0 while none is selected. */
  let selected = 0;

  /**
   * Where an event happened, in the scene's own device pixels.
   *
   * The scene is filled in device pixels and a pointer event is in CSS pixels,
   * so the two meet at the ratio the surface was sized at — the same ratio the
   * renderer took its viewport from.
   */
  function spotOf(event: PointerEvent): Spot {
    const bounds = surface.getBoundingClientRect();
    const dpr = scene().dpr || 1;
    return { x: (event.clientX - bounds.left) * dpr, y: (event.clientY - bounds.top) * dpr };
  }

  const onDown = (event: Event): void => {
    if (!paused() || tool() !== 'compartment') return;
    const pointer = event as PointerEvent;
    // The surface keeps the pointer for the life of the drag, so a release
    // outside it still arrives here and still completes — or, landing on
    // nothing, still proposes nothing. Not every environment implements
    // capture, which is why the window-level release below stands beside it.
    if (typeof surface.setPointerCapture === 'function' && pointer.pointerId !== undefined) {
      try {
        surface.setPointerCapture(pointer.pointerId);
      } catch {
        // A pointer that has already been released cannot be captured, and a
        // drag that cannot be captured is a drag the release below disarms.
      }
    }
    const found = handleAt(scene(), spotOf(pointer));
    held = found;
    // Taking hold of a Route's end selects that Route, which is what makes the
    // press that cuts it mean something: the specification's own sequence is
    // selecting a Route and then pressing the key.
    selected = found && found.kind === HANDLE_KIND.route ? found.route : 0;
  };

  const onUp = (event: Event): void => {
    const from = held;
    held = null;
    if (!from || !paused() || tool() !== 'compartment') return;
    const pointer = event as PointerEvent;
    const plan = planFromDrag(scene(), from, handleAt(scene(), spotOf(pointer)));
    if (plan) queue(plan);
  };

  /**
   * The release that happened somewhere else.
   *
   * A drag left armed is a drag the next press completes, which would queue a
   * change out of two gestures the player never joined. The surface's own
   * handler runs first when the release is on it — it takes `held` and this
   * sees nothing — so this only ever fires for a release the surface did not
   * get, and all it does is let go.
   */
  const onReleaseElsewhere = (): void => {
    held = null;
  };

  const onKeyDown = (event: Event): void => {
    const key = event as KeyboardEvent;
    const target = key.target as { closest?: (selector: string) => Element | null } | null;
    if (target?.closest?.('button, input, select, textarea, [contenteditable="true"]')) return;
    if (key.metaKey || key.ctrlKey || key.altKey || key.repeat) return;
    if (!paused()) return;
    if (CUT_BINDINGS.includes(key.code)) {
      if (tool() !== 'compartment') return;
      if (selected === 0) return;
      // Backspace navigates on some platforms and Delete does nothing; neither
      // is what the key means here, so the event is consumed where it is taken.
      if (key.cancelable) key.preventDefault();
      queue({ op: 'cut', route: selected });
      selected = 0;
      return;
    }
    const forward = CANDIDATE_FORWARD.includes(key.code);
    if (!forward && !CANDIDATE_BACK.includes(key.code)) return;
    if (tool() !== 'view') return;
    walkCandidates(key, forward ? 1 : -1);
  };

  /**
   * Walks the slate's presentation order by one and immediately observes the
   * candidate it lands on.
   *
   * Focusing is passive observation: the top-level command applies immediately,
   * costs nothing, and leaves every causal queue entry exactly where it stood.
   */
  function walkCandidates(key: KeyboardEvent, step: 1 | -1): void {
    const held = options.slate?.() ?? null;
    if (!held || held.deficient || held.count < 1) return;
    if (key.cancelable) key.preventDefault();
    const standing = options.focused?.() ?? 0;
    const next =
      standing === 0
        ? step === 1
          ? 1
          : held.count
        : ((standing - 1 + step + held.count) % held.count) + 1;
    options.focus?.(held.ordinal, next);
  }

  surface.addEventListener('pointerdown', onDown);
  surface.addEventListener('pointerup', onUp);
  keys?.addEventListener('keydown', onKeyDown);
  keys?.addEventListener('pointerup', onReleaseElsewhere);
  keys?.addEventListener('pointercancel', onReleaseElsewhere);

  return {
    selected: () => selected,
    clear() {
      held = null;
      selected = 0;
    },
    close() {
      held = null;
      selected = 0;
      surface.removeEventListener('pointerdown', onDown);
      surface.removeEventListener('pointerup', onUp);
      keys?.removeEventListener('keydown', onKeyDown);
      keys?.removeEventListener('pointerup', onReleaseElsewhere);
      keys?.removeEventListener('pointercancel', onReleaseElsewhere);
    },
  };
}
