/**
 * The drags a paused Field takes, and the entries they queue.
 *
 * Nothing here starts a worker and nothing here validates a change: every
 * precondition is the core's, and what is under test is the other half — which
 * handle a pointer took hold of, and which entry of the locked union the drag
 * it completed proposes.
 *
 * The scene is the real one, projected by the real renderer from a snapshot, so
 * the places these drags reach for are the places the surface actually drew.
 */

import { afterEach, beforeEach, expect, test, vi } from 'vitest';
import { create_renderer, HANDLE_KIND, type Scene } from '../src/render';
import {
  CANDIDATE_BACK,
  CANDIDATE_FORWARD,
  CUT_BINDINGS,
  handleAt,
  openStillEdits,
  planFromDrag,
  proposedInside,
  standingInside,
} from '../src/shell/still-edits';
import type { PlanCommand } from '../../worker/src/protocol';
import type { FrameMode, FramePort, FrameState } from '../../worker/src/frame-state';

const WIDE = 1440;
const HIGH = 900;

/** A 2D context that answers everything and draws nothing. */
function quietContext(): CanvasRenderingContext2D {
  const held: Record<string, unknown> = {};
  const gradient = { addColorStop: () => {} };
  return new Proxy(held, {
    get(target, property) {
      const name = String(property);
      if (name === 'createRadialGradient' || name === 'createLinearGradient') {
        return () => gradient;
      }
      if (name in target) return target[name];
      return () => undefined;
    },
    set(target, property, value) {
      target[String(property)] = value;
      return true;
    },
  }) as unknown as CanvasRenderingContext2D;
}

beforeEach(() => {
  const quiet = quietContext();
  vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockImplementation(
    ((kind: string) =>
      kind === '2d' ? quiet : null) as typeof HTMLCanvasElement.prototype.getContext,
  );
});

afterEach(() => {
  vi.restoreAllMocks();
});

function port(over: Partial<FramePort> & { node: number }): FramePort {
  return {
    kind: 0,
    layer: 0,
    open: true,
    overloaded: false,
    member: false,
    shell: false,
    proposedMember: false,
    charge: 20_000,
    x: 2_048 * 16,
    y: 2_048 * 16,
    reserve: 0,
    ...over,
  };
}

/** A paused Field: four Ports, three of them inside, and one Route. */
function inspectable(mode: FrameMode = 'still'): FrameState {
  const ports = [
    port({ node: 1, member: true, x: 1_900 * 16, y: 2_000 * 16 }),
    port({ node: 2, member: true, x: 2_200 * 16, y: 2_000 * 16 }),
    port({ node: 3, member: true, x: 2_050 * 16, y: 2_250 * 16 }),
    port({ node: 4, x: 2_400 * 16, y: 2_400 * 16 }),
  ];
  return {
    header: {
      version: 1,
      flags: mode === 'still' ? 1 : 0,
      stillVisible: mode === 'still',
      dropped: false,
      reducedMotion: false,
      step: 10,
      timeScale: mode === 'still' ? 0 : 65_535,
      mode,
      cameraLayer: 0,
      impulse: 3,
      chapterIndex: 0,
      objectiveOrdinal: 0,
      sectionCount: 0,
    },
    forms: [],
    ports,
    routes: [{ route: 7, tail: 1, head: 2, flow: 30, status: 0, age: 40 }],
    currents: [],
    inside: ports.map((held) => held.member),
    pressures: [],
    cues: [],
    camera: null,
    overlay: [],
  };
}

/** The scene one snapshot projects to, from the real renderer. */
function projected(state: FrameState): Scene {
  const renderer = create_renderer(document.createElement('canvas'), 'canvas2d', {
    probeWebgl: () => false,
    startWebgl: async () => {
      throw new Error('the fallback is the engine under test');
    },
  });
  renderer.resize(WIDE, HIGH);
  renderer.set_motion_profile({ reducedMotion: true, trailIntensity: 65_536 });
  renderer.render(state, state, 0);
  const scene = renderer.scene();
  renderer.dispose();
  return scene;
}

/** The handle of one kind standing for one identifier. */
function handleFor(scene: Scene, kind: number, names: (mark: { node: number; route: number; end: number }) => boolean) {
  for (let place = 0; place < scene.handles.count; place += 1) {
    const mark = scene.handles.items[place];
    if (mark.kind === kind && names(mark)) return mark;
  }
  throw new Error('the surface drew no such handle');
}

// ---------------------------------------------------------------------------
// Taking hold of a handle
// ---------------------------------------------------------------------------

test('a handle is taken hold of where it was drawn, and only within reach', () => {
  const scene = projected(inspectable());
  const handle = handleFor(scene, HANDLE_KIND.port, (mark) => mark.node === 4);

  expect(handleAt(scene, { x: handle.x, y: handle.y })).toBe(handle);
  // Just inside the reach, and just outside it.
  const reach = handle.radius * 1.5;
  expect(handleAt(scene, { x: handle.x + reach - 1, y: handle.y })).toBe(handle);
  expect(handleAt(scene, { x: handle.x + reach + 4, y: handle.y })?.node).not.toBe(4);
  expect(handleAt(scene, { x: 4, y: 4 })).toBeNull();
});

test('a moving Field offers nothing to take hold of', () => {
  const scene = projected(inspectable('running'));
  expect(scene.handles.count).toBe(0);
  expect(handleAt(scene, { x: WIDE / 2, y: HIGH / 2 })).toBeNull();
});

test('the standing inside is read off the surface in ascending order', () => {
  expect(standingInside(projected(inspectable()))).toEqual([1, 2, 3]);
});

// ---------------------------------------------------------------------------
// What one drag proposes
// ---------------------------------------------------------------------------

test('a drag between two Port handles proposes a connection', () => {
  const scene = projected(inspectable());
  const from = handleFor(scene, HANDLE_KIND.port, (mark) => mark.node === 1);
  const to = handleFor(scene, HANDLE_KIND.port, (mark) => mark.node === 4);
  expect(planFromDrag(scene, from, to)).toEqual({ op: 'connect', from: 1, to: 4 });
  expect(planFromDrag(scene, from, from)).toBeNull();
  expect(planFromDrag(scene, from, null)).toBeNull();
});

test('a drag from a Route end proposes that end, moved', () => {
  const scene = projected(inspectable());
  const to = handleFor(scene, HANDLE_KIND.port, (mark) => mark.node === 4);
  const tail = handleFor(scene, HANDLE_KIND.route, (mark) => mark.route === 7 && mark.end === 0);
  const head = handleFor(scene, HANDLE_KIND.route, (mark) => mark.route === 7 && mark.end === 1);
  expect(planFromDrag(scene, tail, to)).toEqual({ op: 'redirect', route: 7, end: 'tail', to: 4 });
  expect(planFromDrag(scene, head, to)).toEqual({ op: 'redirect', route: 7, end: 'head', to: 4 });
});

test('a drag from a Boundary vertex proposes the member set it leaves', () => {
  const scene = projected(inspectable());
  const vertex = handleFor(scene, HANDLE_KIND.boundary, (mark) => mark.node === 1);
  const outside = handleFor(scene, HANDLE_KIND.port, (mark) => mark.node === 4);
  // The member the vertex is drawn around, replaced by the Node it was let go
  // over: the set the drag leaves, ascending.
  expect(planFromDrag(scene, vertex, outside)).toEqual({
    op: 'reshape_boundary',
    members: [2, 3, 4],
  });
  // Let go over a Node already inside, the dragged member simply leaves.
  const inside = handleFor(scene, HANDLE_KIND.port, (mark) => mark.node === 2);
  expect(planFromDrag(scene, vertex, inside)).toEqual({
    op: 'reshape_boundary',
    members: [2, 3],
  });
});

// ---------------------------------------------------------------------------
// The source over a surface
// ---------------------------------------------------------------------------

/** One pointer event at a place on the surface, in CSS pixels. */
function pointer(kind: string, x: number, y: number): Event {
  return Object.assign(new Event(kind, { bubbles: true, cancelable: true }), {
    clientX: x,
    clientY: y,
  });
}

function press(target: EventTarget, code: string): boolean {
  const event = Object.assign(new Event('keydown', { cancelable: true }), { code, repeat: false });
  target.dispatchEvent(event);
  return event.defaultPrevented;
}

/** A surface whose bounds are the origin, so device pixels are CSS pixels. */
function surfaceOf(scene: Scene): HTMLCanvasElement {
  const canvas = document.createElement('canvas');
  canvas.getBoundingClientRect = () =>
    ({ left: 0, top: 0, width: WIDE, height: HIGH }) as DOMRect;
  scene.dpr = 1;
  return canvas;
}

test('a completed drag on a paused Field queues the entry it proposes', () => {
  const scene = projected(inspectable());
  const surface = surfaceOf(scene);
  const keys = new EventTarget();
  const queued: PlanCommand[] = [];
  const edits = openStillEdits({
    surface,
    keys,
    scene: () => scene,
    paused: () => true,
    queue: (plan) => queued.push(plan),
  });

  const from = handleFor(scene, HANDLE_KIND.port, (mark) => mark.node === 1);
  const to = handleFor(scene, HANDLE_KIND.port, (mark) => mark.node === 4);
  surface.dispatchEvent(pointer('pointerdown', from.x, from.y));
  surface.dispatchEvent(pointer('pointerup', to.x, to.y));
  expect(queued).toEqual([{ op: 'connect', from: 1, to: 4 }]);

  // A drag that ends on nothing proposes nothing.
  surface.dispatchEvent(pointer('pointerdown', from.x, from.y));
  surface.dispatchEvent(pointer('pointerup', 4, 4));
  expect(queued).toHaveLength(1);
  edits.close();
});

test('a Route taken hold of is the Route the key takes away', () => {
  const scene = projected(inspectable());
  const surface = surfaceOf(scene);
  const keys = new EventTarget();
  const queued: PlanCommand[] = [];
  const edits = openStillEdits({
    surface,
    keys,
    scene: () => scene,
    paused: () => true,
    queue: (plan) => queued.push(plan),
  });

  // Nothing selected, nothing cut.
  expect(press(keys, CUT_BINDINGS[0])).toBe(false);
  expect(queued).toHaveLength(0);

  const handle = handleFor(scene, HANDLE_KIND.route, (mark) => mark.route === 7 && mark.end === 1);
  surface.dispatchEvent(pointer('pointerdown', handle.x, handle.y));
  surface.dispatchEvent(pointer('pointerup', handle.x, handle.y));
  expect(edits.selected()).toBe(7);
  expect(press(keys, CUT_BINDINGS[1])).toBe(true);
  expect(queued).toEqual([{ op: 'cut', route: 7 }]);
  // One press is one cut: the selection goes with it.
  expect(edits.selected()).toBe(0);
  expect(press(keys, CUT_BINDINGS[0])).toBe(false);
  expect(queued).toHaveLength(1);
  edits.close();
});

test('a moving Field takes no drag and no key at all', () => {
  const scene = projected(inspectable());
  const surface = surfaceOf(scene);
  const keys = new EventTarget();
  const queued: PlanCommand[] = [];
  const edits = openStillEdits({
    surface,
    keys,
    scene: () => scene,
    paused: () => false,
    queue: (plan) => queued.push(plan),
  });

  const from = handleFor(scene, HANDLE_KIND.port, (mark) => mark.node === 1);
  const to = handleFor(scene, HANDLE_KIND.port, (mark) => mark.node === 4);
  surface.dispatchEvent(pointer('pointerdown', from.x, from.y));
  surface.dispatchEvent(pointer('pointerup', to.x, to.y));
  press(keys, CUT_BINDINGS[0]);
  expect(queued).toHaveLength(0);
  expect(edits.selected()).toBe(0);

  // And a closed source lets go of everything it held.
  edits.close();
  surface.dispatchEvent(pointer('pointerdown', from.x, from.y));
  surface.dispatchEvent(pointer('pointerup', to.x, to.y));
  expect(queued).toHaveLength(0);
});

test('a second boundary drag builds on the set the queue would leave', () => {
  // The core validates each entry against the projection every earlier entry
  // has been applied to, so a drag made while a reshape is already queued has
  // to start from what that reshape leaves. Starting from the standing inside
  // would propose a set that quietly undid it.
  const state = inspectable();
  // A reshape standing in the queue: Node 1 dropped, Node 4 taken in.
  state.ports = state.ports.map((port) => ({
    ...port,
    proposedMember: port.node !== 1,
  }));
  const scene = projected(state);

  expect(standingInside(scene)).toEqual([1, 2, 3]);
  expect(proposedInside(scene)).toEqual([2, 3, 4]);

  const vertex = handleFor(scene, HANDLE_KIND.boundary, (mark) => mark.node === 2);
  const outside = handleFor(scene, HANDLE_KIND.port, (mark) => mark.node === 4);
  // Member 2 replaced by 4, over the proposed set: 4 is already in it, so the
  // drag leaves [3, 4] rather than restoring the 1 the queue had dropped.
  expect(planFromDrag(scene, vertex, outside)).toEqual({
    op: 'reshape_boundary',
    members: [3, 4],
  });
  // And with nothing queued, the standing set is what a drag builds on.
  expect(proposedInside(projected(inspectable()))).toBeNull();
});

test('a release outside the surface leaves no drag armed', () => {
  // A drag left armed is a drag the next press completes, out of two gestures
  // the player never joined.
  const scene = projected(inspectable());
  const surface = surfaceOf(scene);
  const keys = new EventTarget();
  const queued: PlanCommand[] = [];
  const edits = openStillEdits({
    surface,
    keys,
    scene: () => scene,
    paused: () => true,
    queue: (plan) => queued.push(plan),
  });

  const from = handleFor(scene, HANDLE_KIND.port, (mark) => mark.node === 1);
  const to = handleFor(scene, HANDLE_KIND.port, (mark) => mark.node === 4);
  surface.dispatchEvent(pointer('pointerdown', from.x, from.y));
  // The release happens somewhere the surface never sees.
  keys.dispatchEvent(new Event('pointerup'));
  // The next press and release are one gesture of their own, and they are not
  // joined to the one that got away.
  surface.dispatchEvent(pointer('pointerup', to.x, to.y));
  expect(queued).toHaveLength(0);

  // A cancelled pointer disarms the same way.
  surface.dispatchEvent(pointer('pointerdown', from.x, from.y));
  keys.dispatchEvent(new Event('pointercancel'));
  surface.dispatchEvent(pointer('pointerup', to.x, to.y));
  expect(queued).toHaveLength(0);

  // And an ordinary drag still completes: the surface's own release runs first.
  surface.dispatchEvent(pointer('pointerdown', from.x, from.y));
  surface.dispatchEvent(pointer('pointerup', to.x, to.y));
  expect(queued).toEqual([{ op: 'connect', from: 1, to: 4 }]);
  edits.close();
});

test('the surface keeps the pointer for the life of a drag', () => {
  const scene = projected(inspectable());
  const surface = surfaceOf(scene);
  const captured: number[] = [];
  surface.setPointerCapture = (id: number) => captured.push(id);
  const edits = openStillEdits({
    surface,
    keys: new EventTarget(),
    scene: () => scene,
    paused: () => true,
    queue: () => {},
  });

  const from = handleFor(scene, HANDLE_KIND.port, (mark) => mark.node === 1);
  surface.dispatchEvent(
    Object.assign(new Event('pointerdown', { bubbles: true, cancelable: true }), {
      clientX: from.x,
      clientY: from.y,
      pointerId: 7,
    }),
  );
  expect(captured).toEqual([7]);
  edits.close();
});


test('the arrows walk the slate and each step proposes the candidate it lands on', () => {
  const scene = projected(inspectable());
  const surface = surfaceOf(scene);
  const keys = new EventTarget();
  const queued: PlanCommand[] = [];
  let undone = 0;
  // The focus is read from the queue, exactly as the shell reads it: the
  // newest entry when it is a `set_focus`, and 0 otherwise.
  const focused = (): number => {
    const newest = queued[queued.length - 1];
    return newest && newest.op === 'set_focus' ? newest.position : 0;
  };
  const edits = openStillEdits({
    surface,
    keys,
    scene: () => scene,
    paused: () => true,
    queue: (plan) => queued.push(plan),
    slate: () => ({ ordinal: 3, count: 3, deficient: false }),
    focused,
    undo: () => {
      undone += 1;
      queued.pop();
    },
  });

  // The first step forward proposes the first candidate of the presentation
  // order, and takes nothing back: nothing was proposed yet.
  expect(press(keys, CANDIDATE_FORWARD[0])).toBe(true);
  expect(queued).toEqual([{ op: 'set_focus', slate_ordinal: 3, position: 1 }]);
  expect(undone).toBe(0);

  // Every step after replaces the proposal rather than stacking a second one.
  press(keys, CANDIDATE_FORWARD[1]);
  expect(queued).toEqual([{ op: 'set_focus', slate_ordinal: 3, position: 2 }]);
  expect(undone).toBe(1);
  press(keys, CANDIDATE_FORWARD[0]);
  press(keys, CANDIDATE_FORWARD[0]);
  expect(queued).toEqual([{ op: 'set_focus', slate_ordinal: 3, position: 1 }]);
  expect(undone).toBe(3);

  // And back walks the other way, wrapping through the same order.
  press(keys, CANDIDATE_BACK[0]);
  expect(queued).toEqual([{ op: 'set_focus', slate_ordinal: 3, position: 3 }]);
  press(keys, CANDIDATE_BACK[1]);
  expect(queued).toEqual([{ op: 'set_focus', slate_ordinal: 3, position: 2 }]);
  edits.close();
});

test('the arrows propose nothing without a slate to walk', () => {
  const scene = projected(inspectable());
  const surface = surfaceOf(scene);
  const keys = new EventTarget();
  const queued: PlanCommand[] = [];
  let standing: { ordinal: number; count: number; deficient: boolean } | null = null;
  let running = false;
  const edits = openStillEdits({
    surface,
    keys,
    scene: () => scene,
    paused: () => !running,
    queue: (plan) => queued.push(plan),
    slate: () => standing,
    focused: () => 0,
  });

  // No slate stands: the key is not even consumed, so it reaches whatever else
  // the page does with it.
  expect(press(keys, CANDIDATE_FORWARD[0])).toBe(false);
  expect(queued).toHaveLength(0);

  // A deficient slate is not adopted from, whatever position is named.
  standing = { ordinal: 0, count: 1, deficient: true };
  expect(press(keys, CANDIDATE_FORWARD[0])).toBe(false);
  expect(queued).toHaveLength(0);

  // And a moving Field takes no proposal at all.
  standing = { ordinal: 0, count: 2, deficient: false };
  running = true;
  expect(press(keys, CANDIDATE_FORWARD[0])).toBe(false);
  expect(queued).toHaveLength(0);

  running = false;
  expect(press(keys, CANDIDATE_FORWARD[0])).toBe(true);
  expect(queued).toHaveLength(1);
  edits.close();
});
