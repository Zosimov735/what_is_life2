/**
 * Still Mode in the shell: the three keys, and the surface a paused Field puts
 * up.
 *
 * Nothing here starts a worker. The key source is read the way the pump reads
 * it — one sample per emitted frame, one intent at a time — and the renderer is
 * handed decoded snapshots directly, which is its whole input. What the mode
 * itself does with any of it is the core's, and is pinned there.
 */

import { afterEach, beforeEach, expect, test, vi } from 'vitest';
import {
  CANCEL_BINDING,
  COMMIT_BINDING,
  openStill,
  STILL_BINDING,
  type Still,
} from '../src/shell/still';
import { create_renderer } from '../src/render';
import { startCanvas2dEngine } from '../src/render/canvas2d';
import { HANDLE_KIND } from '../src/render/scene';
import {
  FRAME_VERSION,
  type FrameMode,
  type FramePort,
  type FrameState,
} from '../../worker/src/frame-state';

/** The reference viewport the surface is sized to for these tests. */
const WIDE = 1440;
const HIGH = 900;

const sources: Still[] = [];

/**
 * A 2D context that answers everything and draws nothing. The surface under
 * test is the scene the renderer builds, and jsdom carries no canvas of its
 * own, so the engine is given one that says yes.
 */
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
    ((kind: string) => (kind === '2d' ? quiet : null)) as typeof HTMLCanvasElement.prototype.getContext,
  );
});

afterEach(() => {
  for (const source of sources.splice(0)) source.close();
  vi.restoreAllMocks();
});

function opened(interactive = 'button'): { source: Still; target: EventTarget } {
  const target = new EventTarget();
  const source = openStill({ target, interactive });
  sources.push(source);
  return { source, target };
}

function press(target: EventTarget, code: string, over?: Element, held: Partial<KeyboardEvent> = {}) {
  const event = Object.assign(new Event('keydown', { cancelable: true }), {
    code,
    repeat: false,
    ...held,
  });
  if (over) Object.defineProperty(event, 'target', { value: over });
  target.dispatchEvent(event);
  return event;
}

// ---------------------------------------------------------------------------
// The three keys
// ---------------------------------------------------------------------------

test('Space is an edge, carried by one frame and consumed by it', () => {
  const { source, target } = opened();
  expect(source.sample()).toEqual({ toggle_still: false });

  press(target, STILL_BINDING);
  expect(source.sample()).toEqual({ toggle_still: true });
  expect(source.sample()).toEqual({ toggle_still: false });

  // Two presses between two frames are one toggle, because the frame carries
  // a level of the edge rather than a count of it.
  press(target, STILL_BINDING);
  press(target, STILL_BINDING);
  expect(source.sample()).toEqual({ toggle_still: true });
  expect(source.sample()).toEqual({ toggle_still: false });
});

test('Enter and Escape queue one intent each, oldest spent first', () => {
  const { source, target } = opened();
  expect(source.takeIntent()).toBeNull();

  press(target, CANCEL_BINDING);
  press(target, COMMIT_BINDING);
  expect(source.waiting()).toBe(2);
  expect(source.takeIntent()).toBe('undo');
  expect(source.takeIntent()).toBe('commit');
  expect(source.takeIntent()).toBeNull();
  // And neither is a frame field: the frame carries the toggle and nothing
  // else this source holds.
  expect(source.sample()).toEqual({ toggle_still: false });
});

test('the three keys are consumed, so nothing scrolls or submits under them', () => {
  const { source, target } = opened();
  for (const code of [STILL_BINDING, COMMIT_BINDING, CANCEL_BINDING]) {
    expect(press(target, code).defaultPrevented).toBe(true);
  }
  source.clear();
});

test('a key aimed at a control belongs to the control', () => {
  const { source, target } = opened();
  const button = document.createElement('button');

  // A focused `Why?` answers Space and Enter itself; taking either would leave
  // a keyboard player with a control they cannot press.
  expect(press(target, STILL_BINDING, button).defaultPrevented).toBe(false);
  expect(press(target, COMMIT_BINDING, button).defaultPrevented).toBe(false);
  expect(source.sample()).toEqual({ toggle_still: false });
  expect(source.waiting()).toBe(0);
});

test('a shortcut and a key repeat are the platform own', () => {
  const { source, target } = opened();
  press(target, STILL_BINDING, undefined, { metaKey: true });
  press(target, STILL_BINDING, undefined, { ctrlKey: true });
  press(target, STILL_BINDING, undefined, { repeat: true });
  press(target, CANCEL_BINDING, undefined, { altKey: true });
  expect(source.sample()).toEqual({ toggle_still: false });
  expect(source.waiting()).toBe(0);
});

test('letting go drops the toggle and every intent not yet spent', () => {
  const { source, target } = opened();
  press(target, STILL_BINDING);
  press(target, CANCEL_BINDING);
  source.clear();
  expect(source.sample()).toEqual({ toggle_still: false });
  expect(source.takeIntent()).toBeNull();
});

test('an exit asked for is one toggle, carried by the next frame', () => {
  const { source } = opened();
  source.exit();
  expect(source.sample()).toEqual({ toggle_still: true });
  expect(source.sample()).toEqual({ toggle_still: false });
});

// ---------------------------------------------------------------------------
// The inspection surface
// ---------------------------------------------------------------------------

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

/** A snapshot in one mode, holding exactly the Ports and Routes given. */
function fieldIn(
  mode: FrameMode,
  timeScale: number,
  ports: FramePort[],
  routes: FrameState['routes'] = [],
  overlay: FrameState['overlay'] = null,
): FrameState {
  return {
    header: {
      version: FRAME_VERSION,
      flags: mode === 'still' ? 1 : 0,
      stillVisible: mode === 'still',
      dropped: false,
      reducedMotion: false,
      step: 10,
      timeScale,
      mode,
      cameraLayer: 0,
      impulse: 3,
      chapterIndex: 0,
      objectiveOrdinal: 0,
      sectionCount: 0,
      leakPerExposedContactPerStep: 0,
    },
    forms: [],
    ports,
    routes,
    currents: [],
    inside: ports.map((held) => held.member),
    pressures: [],
    cues: [],
    camera: null,
    overlay,
  };
}

/** A Field with a Route, a boundary of three members, and a closed Port. */
function inspectable(mode: FrameMode, timeScale: number, overlay: FrameState['overlay'] = null) {
  const ports = [
    port({ node: 1, member: true, x: 1_900 * 16, y: 2_000 * 16 }),
    port({ node: 2, member: true, x: 2_200 * 16, y: 2_000 * 16 }),
    port({ node: 3, member: true, x: 2_050 * 16, y: 2_250 * 16 }),
    port({ node: 4, open: false, x: 2_400 * 16, y: 2_400 * 16 }),
  ];
  const routes = [{ route: 1, tail: 1, head: 2, flow: 30, status: 0, age: 40 }];
  return fieldIn(mode, timeScale, ports, routes, overlay);
}

/**
 * Draws one snapshot until the eased surface has settled, and answers with the
 * scene. Reduced motion arrives at once, which is what makes a test able to
 * read the settled reading rather than a frame of the way there.
 */
function drawn(kind: 'webgl' | 'canvas2d', state: FrameState) {
  const renderer = create_renderer(document.createElement('canvas'), kind, {
    probeWebgl: () => false,
    startWebgl: async () => {
      throw new Error('the fallback is the engine under test');
    },
  });
  renderer.resize(WIDE, HIGH);
  renderer.set_motion_profile({ reducedMotion: true, trailIntensity: 65_536 });
  renderer.render(state, state, 0);
  const scene = renderer.scene();
  const read = {
    presence: scene.still.presence,
    paused: scene.still.paused,
    handles: scene.handles.count,
    kinds: Array.from({ length: scene.handles.count }, (_, place) => scene.handles.items[place].kind),
    forecast: { alpha: scene.forecast.alpha, count: scene.forecast.count, low: [...scene.forecast.low] },
    portAlpha: scene.ports.items[3].alpha,
  };
  renderer.dispose();
  return read;
}

test('a moving Field offers no handles at all', () => {
  const read = drawn('canvas2d', inspectable('running', 65_535));
  expect(read.presence).toBe(0);
  expect(read.paused).toBe(false);
  expect(read.handles).toBe(0);
  expect(read.forecast.alpha).toBe(0);
});

test('a paused Field puts a handle on every Port, Route end, and Boundary vertex', () => {
  const read = drawn('canvas2d', inspectable('still', 0, []));
  expect(read.presence).toBe(1);
  expect(read.paused).toBe(true);

  // Four Ports, one Route with two ends, and a boundary of three members whose
  // hull is a triangle: four, two, and three.
  expect(read.kinds.filter((kind) => kind === HANDLE_KIND.port)).toHaveLength(4);
  expect(read.kinds.filter((kind) => kind === HANDLE_KIND.route)).toHaveLength(2);
  expect(read.kinds.filter((kind) => kind === HANDLE_KIND.boundary)).toHaveLength(3);
  expect(read.handles).toBe(9);
});

/**
 * What one engine drew: the shapes it asked for, and where.
 *
 * Both engines are driven over the *same* projected scene and each is asked
 * what it emitted, so this compares the two engines rather than one engine
 * with itself. Neither is asked to rasterize: the Canvas2D engine gets a
 * context that records calls, and the PixiJS engine gets a renderer of the
 * test's own, its scene graph building geometry with no GPU behind it.
 *
 * The scene is stripped down to the marks under test first. The two engines
 * differ in fidelity everywhere else by design — soft gradients against
 * shaders, fewer particles — so comparing everything they draw would compare
 * the fidelity difference rather than the parity claim.
 */
interface Emitted {
  arcs: string[];
  rects: string[];
  polys: number;
  strokes: number;
}

/** A recorder shared by both engines, reading one path at a time. */
function recorder(): { emitted: Emitted; open: () => void; note: (call: string) => void; stroke: () => void } {
  const emitted: Emitted = { arcs: [], rects: [], polys: 0, strokes: 0 };
  let path: string[] = [];
  return {
    emitted,
    open: () => {
      path = [];
    },
    note: (call) => path.push(call),
    stroke: () => {
      for (const one of path) {
        if (one.startsWith('arc ')) emitted.arcs.push(one);
        if (one.startsWith('rect ')) emitted.rects.push(one);
      }
      if (path.some((one) => one === 'poly' || one === 'lineTo')) emitted.polys += 1;
      emitted.strokes += 1;
      path = [];
    },
  };
}

/** The scene one snapshot projects to, with every pool but the two under test emptied. */
function handlesOnly(state: FrameState) {
  const renderer = create_renderer(document.createElement('canvas'), 'canvas2d', {
    probeWebgl: () => false,
    startWebgl: async () => {
      throw new Error('the engines are driven directly here');
    },
  });
  renderer.resize(WIDE, HIGH);
  renderer.set_motion_profile({ reducedMotion: true, trailIntensity: 65_536 });
  renderer.render(state, state, 0);
  const scene = renderer.scene();
  for (const pool of [
    scene.hazes,
    scene.currents,
    scene.routes,
    scene.boundaries,
    scene.ports,
    scene.trails,
    scene.forms,
    scene.particles,
    scene.cues,
  ]) {
    pool.count = 0;
  }
  scene.stillness = 0;
  scene.rim.level = 0;
  renderer.dispose();
  return scene;
}

function emittedByCanvas2d(scene: ReturnType<typeof handlesOnly>): Emitted {
  const held = recorder();
  const context = new Proxy({} as Record<string, unknown>, {
    get(target, property) {
      const name = String(property);
      if (name === 'createRadialGradient' || name === 'createLinearGradient') {
        return () => ({ addColorStop: () => {} });
      }
      if (name in target) return target[name];
      return (...given: unknown[]) => {
        if (name === 'beginPath') held.open();
        if (name === 'arc') held.note(`arc ${given.slice(0, 3).join(',')}`);
        if (name === 'rect') held.note(`rect ${given.join(',')}`);
        if (name === 'moveTo' || name === 'lineTo' || name === 'closePath') held.note(name);
        if (name === 'stroke') held.stroke();
        if (name === 'fill') held.open();
        return undefined;
      };
    },
    set(target, property, value) {
      target[String(property)] = value;
      return true;
    },
  }) as unknown as CanvasRenderingContext2D;
  const surface = document.createElement('canvas');
  surface.getContext = (() => context) as unknown as typeof surface.getContext;
  startCanvas2dEngine(surface).draw(scene);
  return held.emitted;
}

async function emittedByWebgl(scene: ReturnType<typeof handlesOnly>): Promise<Emitted> {
  const held = recorder();
  const { WebglEngine } = await import('../src/render/webgl');
  const { Graphics } = await import('pixi.js');
  const noted: [string, (given: unknown[]) => void][] = [
    ['circle', (given) => held.note(`arc ${given.slice(0, 3).join(',')}`)],
    ['rect', (given) => held.note(`rect ${given.join(',')}`)],
    ['poly', () => held.note('poly')],
    ['moveTo', () => held.note('moveTo')],
    ['lineTo', () => held.note('lineTo')],
    ['stroke', () => held.stroke()],
    ['clear', () => held.open()],
    ['fill', () => held.open()],
  ];
  const spies = noted.map(([name, record]) =>
    vi.spyOn(Graphics.prototype, name as never).mockImplementation(function (
      this: unknown,
      ...given: unknown[]
    ) {
      record(given);
      return this as never;
    } as never),
  );
  // A renderer of the test's own: the engine builds its scene graph and asks
  // this to present it, and presenting is the one thing a GPU is needed for.
  const engine = new WebglEngine({ resize: () => {}, render: () => {}, destroy: () => {} } as never);
  engine.draw(scene);
  for (const spy of spies) spy.mockRestore();
  return held.emitted;
}

test('the two engines emit the same handles and the same strip', async () => {
  const scene = handlesOnly(inspectable('still', 0, []));
  const canvas = emittedByCanvas2d(scene);
  const webgl = await emittedByWebgl(scene);

  // Four Port handles are rings, two Route handles are squares, and three
  // Boundary handles are four-sided paths, with the forecast strip's own
  // bracket beside them — the same shapes at the same places from both
  // engines, which is the parity claim itself rather than a claim about the
  // scene they were both projected from.
  expect(canvas.arcs).toHaveLength(4);
  expect(canvas.rects).toHaveLength(2);
  expect(canvas.strokes).toBe(scene.handles.count + 1);
  expect(webgl.arcs).toEqual(canvas.arcs);
  expect(webgl.rects).toEqual(canvas.rects);
  expect(webgl.polys).toBe(canvas.polys);
  expect(webgl.strokes).toBe(canvas.strokes);
});

test('the two engines project the same scene from the same snapshot', () => {
  const state = inspectable('still', 0, []);
  const canvas = drawn('canvas2d', state);
  const webgl = drawn('webgl', state);
  expect(webgl.handles).toBe(canvas.handles);
  expect(webgl.kinds).toEqual(canvas.kinds);
  expect(webgl.forecast).toEqual(canvas.forecast);
});

test('the surface comes up with the ramp rather than at the end of it', () => {
  const early = drawn('canvas2d', inspectable('ramp_in', 49_000));
  const late = drawn('canvas2d', inspectable('ramp_in', 8_000));
  expect(early.presence).toBeGreaterThan(0);
  expect(late.presence).toBeGreaterThan(early.presence);
  expect(late.presence).toBeLessThan(1);
  expect(late.handles).toBe(9);
  // The overlay is the pause's own, so it waits for the pause.
  expect(late.forecast.alpha).toBe(0);
});

test('the surface finishes arriving about when the ramp finishes running', () => {
  // With motion on, the surface eases toward the mode's target rather than
  // snapping to it, because the last stretch of an entry ramp runs no step and
  // so carries no snapshot. The rate is tied to the ramp's own frame count, so
  // a surface that is still fading in over a Field that has already stopped is
  // the failure this pins.
  const renderer = create_renderer(document.createElement('canvas'), 'canvas2d', {
    probeWebgl: () => false,
    startWebgl: async () => {
      throw new Error('the fallback is the engine under test');
    },
  });
  renderer.resize(WIDE, HIGH);
  renderer.set_motion_profile({ reducedMotion: false, trailIntensity: 65_536 });

  // One ramp, at the render rate: the scale falls linearly over the frames a
  // 250,000 µs span covers at 60 frames a second.
  const frames = Math.ceil(250_000 / 16_667);
  for (let frame = 1; frame <= frames; frame += 1) {
    const scale = Math.round(65_535 * (1 - frame / frames));
    const state = inspectable('ramp_in', scale);
    renderer.render(state, state, 0);
  }
  const arriving = renderer.scene().still.presence;
  expect(arriving).toBeGreaterThan(0.9);
  expect(arriving).toBeLessThanOrEqual(1);

  // And the pause itself settles the reading rather than approaching it
  // forever, so a surface that is up is all the way up.
  const still = inspectable('still', 0, []);
  for (let frame = 0; frame < 4; frame += 1) renderer.render(still, still, 0);
  expect(renderer.scene().still.presence).toBe(1);
  renderer.dispose();
});

test('a closed Port is made prominent while the surface is up', () => {
  const moving = drawn('canvas2d', inspectable('running', 65_535));
  const paused = drawn('canvas2d', inspectable('still', 0, []));
  expect(paused.portAlpha).toBeGreaterThan(moving.portAlpha);
});

test('a suspended run is not an inspection, and puts no surface up', () => {
  const read = drawn('canvas2d', inspectable('suspended', 0));
  expect(read.presence).toBe(0);
  expect(read.handles).toBe(0);
});

test('the forecast overlay draws its chrome with nothing in it, and its band with something', () => {
  const empty = drawn('canvas2d', inspectable('still', 0, []));
  expect(empty.forecast.alpha).toBe(1);
  expect(empty.forecast.count).toBe(0);
  expect(empty.forecast.low).toHaveLength(0);

  const filled = drawn(
    'canvas2d',
    inspectable('still', 0, [
      { lo: 1, hi: 2 },
      { lo: 1.5, hi: 3 },
      { lo: 0.5, hi: 2.5 },
    ]),
  );
  expect(filled.forecast.count).toBe(3);
  expect(filled.forecast.low).toHaveLength(6);
});
