/**
 * The renderer, read the way the shell drives it.
 *
 * Nothing here starts a worker or a module: the renderer's whole input is a
 * decoded `FrameState` pair, so the tests hand it the snapshot the core's own
 * fixture pins and the development stand-in's populated Field, and read what it
 * made of them. What is under test is the scene the renderer builds — the shape
 * a snapshot takes on the surface — the fallback the WebGL failure path
 * engages, and the promise that the renderer writes nothing back.
 */

import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { afterEach, beforeEach, expect, inject, test, vi } from 'vitest';
import {
  decodeFrameState,
  FRAME_VERSION,
  FRAME_PRESSURE_IDS,
  type FrameCurrent,
  type FramePort,
  type FramePressure,
  type FrameState,
} from '../../worker/src/frame-state';
import { PRESSURE_TONES } from '../src/render/palette';
import { create_renderer } from '../src/render';
import { dashOf, type Engine } from '../src/render/engine';
import { fixtureSnapshot } from '../src/shell/dev-frames';

const WORKSPACE = inject('workspace');

/** The reference viewport the surface is sized to for these tests. */
const WIDE = 1440;
const HIGH = 900;

/** A 2D context that records what it was asked to draw. */
interface Recorded {
  context: CanvasRenderingContext2D;
  calls: Map<string, number>;
  arguments: Map<string, unknown[][]>;
  total: () => number;
}

function recordingContext(): Recorded {
  const calls = new Map<string, number>();
  const argumentsByCall = new Map<string, unknown[][]>();
  const held: Record<string, unknown> = {};
  const gradient = { addColorStop: () => {} };
  const bump = (name: string): void => {
    calls.set(name, (calls.get(name) ?? 0) + 1);
  };
  const context = new Proxy(held, {
    get(target, property) {
      const name = String(property);
      if (name === 'createRadialGradient' || name === 'createLinearGradient') {
        return () => {
          bump(name);
          return gradient;
        };
      }
      if (name in target) return target[name];
      return (...rest: unknown[]) => {
        bump(name);
        const held = argumentsByCall.get(name);
        if (held) held.push(rest);
        else argumentsByCall.set(name, [rest]);
        return rest.length === 0 ? undefined : undefined;
      };
    },
    set(target, property, value) {
      target[String(property)] = value;
      return true;
    },
  }) as unknown as CanvasRenderingContext2D;
  return {
    context,
    calls,
    arguments: argumentsByCall,
    total: () => [...calls.values()].reduce((sum, at) => sum + at, 0),
  };
}

let recorded: Recorded;

beforeEach(() => {
  recorded = recordingContext();
  vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockImplementation(
    ((kind: string) => (kind === '2d' ? recorded.context : null)) as typeof HTMLCanvasElement.prototype.getContext,
  );
});

afterEach(() => {
  vi.restoreAllMocks();
});

/** The snapshot the core's own test pins, read across the two languages. */
async function recordedSnapshot(): Promise<FrameState> {
  const hex = (
    await readFile(path.join(WORKSPACE, 'core', 'tests', 'fixtures', 'frame_state.hex'), 'utf8')
  ).trim();
  const bytes = new Uint8Array(hex.length / 2);
  for (let place = 0; place < bytes.length; place += 1) {
    bytes[place] = Number.parseInt(hex.slice(place * 2, place * 2 + 2), 16);
  }
  return decodeFrameState(bytes.buffer);
}

function surface(): HTMLCanvasElement {
  return document.createElement('canvas');
}

// ---------------------------------------------------------------------------
// Standing up against a recorded snapshot
// ---------------------------------------------------------------------------

test('the renderer draws the recorded snapshot with no worker behind it', async () => {
  const state = await recordedSnapshot();
  const renderer = create_renderer(surface(), 'canvas2d');
  await renderer.ready;
  renderer.resize(WIDE, HIGH);

  expect(renderer.kind()).toBe('canvas2d');
  renderer.render(state, state, 0);

  // The snapshot carries one Form, four Ports, one Route, and one current, and
  // every one of them reached the surface.
  const scene = renderer.scene();
  expect(scene.forms.count).toBe(state.forms.length);
  expect(scene.ports.count).toBe(state.ports.length);
  expect(scene.routes.count).toBe(state.routes.length);
  expect(scene.currents.count).toBe(state.currents.length);
  expect(recorded.total()).toBeGreaterThan(0);
  renderer.dispose();
});

test('the renderer writes nothing back into the snapshot it is handed', async () => {
  const state = await recordedSnapshot();
  const before = JSON.stringify(state);
  const renderer = create_renderer(surface(), 'canvas2d');
  renderer.resize(WIDE, HIGH);
  renderer.render(state, state, 0);
  renderer.render(state, state, 0.5);
  expect(JSON.stringify(state)).toBe(before);
  renderer.dispose();
});

// ---------------------------------------------------------------------------
// The scene's shape against the snapshot's own sections
// ---------------------------------------------------------------------------

test('every section of a populated snapshot maps onto the scene one to one', () => {
  const previous = fixtureSnapshot(600);
  const next = fixtureSnapshot(601);
  const renderer = create_renderer(surface(), 'canvas2d');
  renderer.resize(WIDE, HIGH);
  renderer.render(previous, next, 0);
  const scene = renderer.scene();

  expect(next.forms.length).toBe(4);
  expect(next.ports.length).toBe(22);
  expect(next.routes.length).toBe(12);
  expect(next.currents.length).toBe(3);
  expect(next.pressures.length).toBeGreaterThan(0);

  expect(scene.forms.count).toBe(next.forms.length);
  expect(scene.ports.count).toBe(next.ports.length);
  expect(scene.routes.count).toBe(next.routes.length);
  expect(scene.currents.count).toBe(next.currents.length);

  // The standing physical compartment and the passive View draw independently;
  // the material members remain the members the snapshot marks.
  expect(next.ports.filter((port) => port.member)).toHaveLength(6);
  expect(scene.boundaries.count).toBe(2);
  expect(scene.boundaries.items.find((mark) => mark.role === 'compartment')?.points.length)
    .toBeGreaterThanOrEqual(6);
  expect(scene.boundaries.items.find((mark) => mark.role === 'view')?.points.length)
    .toBeGreaterThanOrEqual(6);

  // A Field standing on more than one layer draws a plane of haze per layer
  // other than the camera's own.
  const layers = new Set([
    ...next.forms.map((form) => form.layer),
    ...next.currents.map((current) => current.layer),
  ]);
  expect(layers.size).toBeGreaterThan(1);
  expect(scene.hazes.count).toBeGreaterThan(0);
  for (let place = 0; place < scene.hazes.count; place += 1) {
    expect(scene.hazes.items[place].depth).not.toBe(0);
  }

  // The pressure reading reaches in from the edge.
  expect(scene.rim.level).toBeGreaterThan(0);
  renderer.dispose();
});

test('the controlled Form outsizes every other, however near theirs stand', () => {
  const renderer = create_renderer(surface(), 'canvas2d');
  renderer.resize(WIDE, HIGH);

  // The stand-in Field, where the others stand deeper: the easy case.
  renderer.render(fixtureSnapshot(600), fixtureSnapshot(601), 0);
  expectSteeredWidest(renderer.scene());

  // And the case the plane scaling can invert on its own: a Form four layers
  // nearer than the camera is drawn much larger, and a margin applied only at
  // the start would let it overtake the Form the player steers.
  const camera = fieldOf([]);
  camera.header.cameraLayer = 6;
  camera.forms = [
    {
      id: 1,
      formOrdinal: 0,
      layer: 6,
      controlled: true,
      focus: false,
      pulseCharging: false,
      separated: false,
      x: 2_048,
      y: 2_048,
      vx: 0,
      vy: 0,
      charge: 0,
      radius: 0,
    },
    {
      id: 2,
      formOrdinal: 1,
      layer: 0,
      controlled: false,
      focus: false,
      pulseCharging: false,
      separated: false,
      x: 2_248,
      y: 2_048,
      vx: 0,
      vy: 0,
      charge: 65_535,
      radius: 0,
    },
  ];
  renderer.render(camera, camera, 0);
  const scene = renderer.scene();
  expect(scene.forms.items[1].depth).toBe(-6);
  expectSteeredWidest(scene);
  expect(scene.forms.items[0].alpha).toBeGreaterThanOrEqual(scene.forms.items[1].alpha);
  renderer.dispose();
});

/** The controlled Form is the widest mark on the surface, by a margin. */
function expectSteeredWidest(scene: ReturnType<ReturnType<typeof create_renderer>['scene']>): void {
  const marks = scene.forms.items.slice(0, scene.forms.count);
  const steered = marks.find((mark) => mark.controlled);
  expect(steered).toBeDefined();
  for (const other of marks.filter((mark) => !mark.controlled)) {
    expect(steered?.radius ?? 0).toBeGreaterThan(other.radius);
  }
}

test('a Field with more in it draws more than a Field with less', async () => {
  const small = await recordedSnapshot();
  const large = fixtureSnapshot(300);

  const thin = create_renderer(surface(), 'canvas2d');
  thin.resize(WIDE, HIGH);
  thin.render(small, small, 0);
  const thinWork = recorded.total();
  thin.dispose();

  recorded = recordingContext();
  const wide = create_renderer(surface(), 'canvas2d');
  wide.resize(WIDE, HIGH);
  wide.render(large, large, 0);
  expect(recorded.total()).toBeGreaterThan(thinWork);
  wide.dispose();
});

// ---------------------------------------------------------------------------
// What the renderer derives from a pair of snapshots
// ---------------------------------------------------------------------------

test('a current is drawn along the path its own record names', async () => {
  // The recorded snapshot the core pins: one current, a two-point path, and a
  // width in whole units. What the renderer strokes is that path, projected.
  const state = await recordedSnapshot();
  expect(state.currents[0].path).toEqual([
    { x: 100, y: 100 },
    { x: 200, y: 300 },
  ]);
  expect(state.currents[0].width).toBe(16);

  const renderer = create_renderer(surface(), 'canvas2d');
  renderer.resize(WIDE, HIGH);
  renderer.render(state, state, 0);
  const mark = renderer.scene().currents.items[0];
  expect(mark.points).toHaveLength(state.currents[0].path.length * 2);
  expect(mark.width).toBeGreaterThan(2);

  // The two ends are as far apart on the surface as the path is in the Field,
  // to the zoom: the shape drawn is the shape the record carries.
  const zoom = renderer.scene().camera.zoom;
  const spread = Math.hypot(mark.points[2] - mark.points[0], mark.points[3] - mark.points[1]);
  expect(spread).toBeCloseTo(Math.hypot(100, 200) * zoom, 3);
  renderer.dispose();
});

test('a current with a path draws particles running along it', () => {
  const renderer = create_renderer(surface(), 'canvas2d');
  renderer.resize(WIDE, HIGH);
  renderer.render(fixtureSnapshot(199), fixtureSnapshot(200), 0);
  const scene = renderer.scene();
  expect(scene.currents.count).toBe(3);
  expect(scene.currents.items[0].points.length / 2).toBe(8);
  expect(scene.particles.count).toBeGreaterThan(0);
  renderer.dispose();
});

test('a Port whose Charge rose with no Route into it is lit as delivered', () => {
  const renderer = create_renderer(surface(), 'canvas2d');
  renderer.resize(WIDE, HIGH);
  let lit = 0;
  for (let step = 200; step < 230; step += 1) {
    renderer.render(fixtureSnapshot(step - 1), fixtureSnapshot(step), 0);
    const scene = renderer.scene();
    for (let place = 0; place < scene.ports.count; place += 1) {
      if (scene.ports.items[place].delivered > 0) lit += 1;
    }
  }
  expect(lit).toBeGreaterThan(0);
  renderer.dispose();
});

test('the surface interpolates between the two snapshots by the fraction given', () => {
  const previous = fixtureSnapshot(400);
  const next = fixtureSnapshot(401);
  const renderer = create_renderer(surface(), 'canvas2d');
  // Reduced motion places the camera outright rather than easing it, which is
  // what makes the interpolated place readable straight off the camera.
  renderer.set_motion_profile({ reducedMotion: true, trailIntensity: 0 });
  renderer.resize(WIDE, HIGH);

  expect(previous.forms[0].x).not.toBe(next.forms[0].x);

  renderer.render(previous, next, 0);
  expect(renderer.scene().camera.x).toBeCloseTo(previous.forms[0].x, 3);
  renderer.render(previous, next, 1);
  expect(renderer.scene().camera.x).toBeCloseTo(next.forms[0].x, 3);
  renderer.render(previous, next, 0.5);
  expect(renderer.scene().camera.x).toBeCloseTo((previous.forms[0].x + next.forms[0].x) / 2, 3);

  // The clock the surface animates on is simulated time, not wall time: it is
  // the step the snapshot names plus the fraction between two of them.
  expect(renderer.scene().clock).toBeCloseTo(next.header.step + 0.5, 6);
  renderer.dispose();
});

test('the time scale the header carries reads as the surface settling', async () => {
  const running = await recordedSnapshot();
  expect(running.header.timeScale).toBe(65_535);

  const renderer = create_renderer(surface(), 'canvas2d');
  renderer.resize(WIDE, HIGH);
  renderer.render(running, running, 0);
  // Full speed saturates at 65535, one short of the 65536 the simulation holds,
  // and the surface settles by nothing at all.
  expect(renderer.scene().timeScale).toBe(1);
  expect(renderer.scene().stillness).toBe(0);

  const halted = structuredClone(running);
  halted.header.timeScale = 0;
  renderer.render(halted, halted, 0);
  expect(renderer.scene().stillness).toBe(1);

  const ramping = structuredClone(running);
  ramping.header.timeScale = 32_768;
  renderer.render(ramping, ramping, 0);
  expect(renderer.scene().stillness).toBeCloseTo(0.5, 2);
  renderer.dispose();
});

/** Runs a stretch of the stand-in Field through a renderer. */
function run(renderer: ReturnType<typeof create_renderer>, from: number, to: number): void {
  for (let step = from; step < to; step += 1) {
    renderer.render(fixtureSnapshot(step - 1), fixtureSnapshot(step), 0);
  }
}

test('reduced motion shortens the trails and thins the particles', () => {
  const full = create_renderer(surface(), 'canvas2d');
  full.resize(WIDE, HIGH);
  run(full, 500, 560);
  const trailed = full.scene().trails.items[0].points.length / 2;
  const drifting = full.scene().particles.count;
  expect(full.scene().trails.count).toBeGreaterThan(0);
  expect(trailed).toBeGreaterThan(30);
  full.dispose();

  // Reduced motion at full trail intensity: the path the setting actually
  // takes in the shell, where intensity stays at its default and only the
  // reduced-motion flag moves.
  const eased = create_renderer(surface(), 'canvas2d');
  eased.resize(WIDE, HIGH);
  eased.set_motion_profile({ reducedMotion: true, trailIntensity: 65536 });
  run(eased, 500, 560);
  expect(eased.scene().trails.count).toBeGreaterThan(0);
  expect(eased.scene().trails.items[0].points.length / 2).toBeLessThan(trailed);
  expect(eased.scene().particles.count).toBeLessThan(drifting);
  eased.dispose();

  // Intensity at zero is the other half of the profile, and drops them.
  const bare = create_renderer(surface(), 'canvas2d');
  bare.resize(WIDE, HIGH);
  bare.set_motion_profile({ reducedMotion: true, trailIntensity: 0 });
  run(bare, 500, 560);
  expect(bare.scene().trails.count).toBe(0);
  bare.dispose();
});

// ---------------------------------------------------------------------------
// Shapes with no area, and planes
// ---------------------------------------------------------------------------

/** A Port record with everything but the fields a test cares about neutral. */
function port(over: Partial<FramePort> & { node: number }): FramePort {
  return {
    kind: 0,
    layer: 0,
    open: true,
    overloaded: false,
    member: false,
    shell: false,
    proposedMember: false,
    charge: 1_000,
    x: 0,
    y: 0,
    reserve: 0,
    ...over,
  };
}

/** A current record with no path, for a Field under test that needs none. */
function current(over: Partial<FrameCurrent> & { id: number }): FrameCurrent {
  return {
    layer: 0,
    active: true,
    bright: false,
    phase: 0,
    strength: 20_000,
    path: [],
    width: 16,
    ...over,
  };
}

/** A snapshot holding exactly the Ports given, and nothing else. */
function fieldOf(
  ports: FramePort[],
  currents: FrameCurrent[] = [],
  viewed: readonly number[] = [],
): FrameState {
  return {
    header: {
      version: FRAME_VERSION,
      flags: 0,
      stillVisible: false,
      dropped: false,
      reducedMotion: false,
      step: 10,
      timeScale: 65_535,
      mode: 'running',
      cameraLayer: 0,
      impulse: 3,
      chapterIndex: 0,
      objectiveOrdinal: 0,
      sectionCount: 2,
      leakPerExposedContactPerStep: 0,
    },
    forms: [],
    ports,
    routes: [],
    currents,
    inside: ports.map((one) => viewed.includes(one.node)),
    pressures: [],
    cues: [],
    camera: null,
    overlay: null,
  };
}

/** What both engines drew, for a Field that has to reach both of them. */
function drawnBy(kind: 'webgl' | 'canvas2d', state: FrameState): number {
  recorded = recordingContext();
  const renderer = create_renderer(surface(), kind, {
    probeWebgl: () => false,
    startWebgl: async () => {
      throw new Error('the fallback is the engine under test');
    },
  });
  renderer.resize(WIDE, HIGH);
  renderer.render(state, state, 0);
  const boundaries = renderer.scene().boundaries.count;
  renderer.dispose();
  return boundaries;
}

test('a physical compartment with no area still draws its edge, and the same one either way', () => {
  // Two members: the hull is a segment. Three in a line: the monotone chain
  // drops the middle one and the hull is a segment again. Both cases used to
  // vanish in one engine and draw in the other.
  const two = fieldOf([
    port({ node: 1, member: true, x: 1_000 * 16, y: 1_000 * 16 }),
    port({ node: 2, member: true, x: 1_200 * 16, y: 1_000 * 16 }),
  ]);
  const collinear = fieldOf([
    port({ node: 1, member: true, x: 1_000 * 16, y: 1_000 * 16 }),
    port({ node: 2, member: true, x: 1_100 * 16, y: 1_000 * 16 }),
    port({ node: 3, member: true, x: 1_200 * 16, y: 1_000 * 16 }),
  ]);
  const alone = fieldOf([port({ node: 1, member: true, x: 1_000 * 16, y: 1_000 * 16 })]);

  for (const state of [two, collinear, alone]) {
    for (const kind of ['canvas2d', 'webgl'] as const) {
      expect(drawnBy(kind, state)).toBe(1);
      // The engine drew something for it rather than passing over it.
      expect(recorded.total()).toBeGreaterThan(0);
    }
  }

  // And the mark itself is the degenerate shape, not a dropped one.
  const renderer = create_renderer(surface(), 'canvas2d');
  renderer.resize(WIDE, HIGH);
  renderer.render(collinear, collinear, 0);
  expect(renderer.scene().boundaries.items[0].points).toHaveLength(4);
  renderer.render(alone, alone, 0);
  expect(renderer.scene().boundaries.items[0].points).toHaveLength(2);
  expect(renderer.scene().boundaries.items[0].width).toBeGreaterThan(0);
  renderer.dispose();
});

test('degenerate physical, proposed, View, and candidate hulls keep distinct dash registers', () => {
  const state = fieldOf(
    [
      port({ node: 1, member: true, x: 1_000 * 16, y: 1_000 * 16 }),
      port({ node: 2, proposedMember: true, x: 1_200 * 16, y: 1_000 * 16 }),
      port({ node: 3, x: 1_400 * 16, y: 1_000 * 16 }),
      port({ node: 4, x: 1_600 * 16, y: 1_000 * 16 }),
    ],
    [],
    [3],
  );
  const renderer = create_renderer(surface(), 'canvas2d');
  renderer.resize(WIDE, HIGH);
  renderer.set_still_tool('view');
  renderer.set_candidates([{ position: 1, members: [4], focused: false, tier: 1 }]);
  renderer.render(state, state, 0);

  const patterns = (recorded.arguments.get('setLineDash') ?? []).map(([pattern]) => pattern);
  expect(patterns).toContainEqual([]);
  expect(patterns).toContainEqual([4, 6]);
  expect(patterns).toContainEqual([12, 8]);
  expect(patterns).toContainEqual([2, 8]);
  renderer.dispose();
});

test('a physical compartment reaching two planes draws one material edge on each', () => {
  const state = fieldOf([
    port({ node: 1, member: true, layer: 0, x: 1_000 * 16, y: 1_000 * 16 }),
    port({ node: 2, member: true, layer: 0, x: 1_300 * 16, y: 1_000 * 16 }),
    port({ node: 3, member: true, layer: 0, x: 1_150 * 16, y: 1_300 * 16 }),
    port({ node: 4, member: true, layer: 2, x: 2_000 * 16, y: 2_000 * 16 }),
    port({ node: 5, member: true, layer: 2, x: 2_300 * 16, y: 2_000 * 16 }),
  ]);
  const renderer = create_renderer(surface(), 'canvas2d');
  renderer.resize(WIDE, HIGH);
  renderer.render(state, state, 0);
  expect(renderer.scene().boundaries.count).toBe(2);
  renderer.dispose();
});

test('a Port is placed on the plane its own record names', () => {
  const state = fieldOf([
    port({ node: 1, layer: 0, x: 2_048 * 16, y: 2_048 * 16, charge: 30_000 }),
    port({ node: 2, layer: 3, x: 2_048 * 16, y: 2_048 * 16, charge: 30_000 }),
  ]);
  const renderer = create_renderer(surface(), 'canvas2d');
  renderer.resize(WIDE, HIGH);
  renderer.render(state, state, 0);
  const scene = renderer.scene();
  // Two Nodes at the same place in the Field, three planes apart: the deeper
  // one is drawn smaller and dimmer, which is the whole depth reading.
  expect(scene.ports.items[0].depth).toBe(0);
  expect(scene.ports.items[1].depth).toBe(3);
  expect(scene.ports.items[1].radius).toBeLessThan(scene.ports.items[0].radius);
  expect(scene.ports.items[1].alpha).toBeLessThan(scene.ports.items[0].alpha);
  renderer.dispose();
});

// ---------------------------------------------------------------------------
// The fallback
// ---------------------------------------------------------------------------

test('a WebGL engine that will not start engages the fallback, and it draws', async () => {
  const refused = vi.fn(async (): Promise<Engine> => {
    throw new Error('no WebGL context');
  });
  const reported = vi.spyOn(console, 'error').mockImplementation(() => {});

  const renderer = create_renderer(surface(), 'webgl', {
    probeWebgl: () => true,
    startWebgl: refused,
  });
  renderer.resize(WIDE, HIGH);

  expect(await renderer.ready).toBe('canvas2d');
  expect(refused).toHaveBeenCalledOnce();
  expect(reported).toHaveBeenCalledOnce();
  expect(renderer.kind()).toBe('canvas2d');

  const state = fixtureSnapshot(120);
  const before = recorded.total();
  renderer.render(state, state, 0);
  expect(recorded.total()).toBeGreaterThan(before);
  // The fallback carries the same responsibilities: everything the snapshot
  // holds still reaches the surface.
  const scene = renderer.scene();
  expect(scene.ports.count).toBe(state.ports.length);
  expect(scene.routes.count).toBe(state.routes.length);
  expect(scene.forms.count).toBe(state.forms.length);
  renderer.dispose();
});

test('a surface that could carry no WebGL context never asks for one', async () => {
  const untouched = vi.fn(async (): Promise<Engine> => {
    throw new Error('this should never be reached');
  });
  const renderer = create_renderer(surface(), 'webgl', {
    probeWebgl: () => false,
    startWebgl: untouched,
  });
  expect(await renderer.ready).toBe('canvas2d');
  expect(untouched).not.toHaveBeenCalled();
  renderer.dispose();
});

test('a renderer disposed before its engine arrives leaves nothing behind', async () => {
  let released = 0;
  const engine: Engine = {
    kind: 'webgl',
    resize: () => {},
    draw: () => {},
    dispose: () => {
      released += 1;
    },
  };
  const renderer = create_renderer(surface(), 'webgl', {
    probeWebgl: () => true,
    startWebgl: async () => engine,
  });
  renderer.dispose();
  await renderer.ready;
  expect(released).toBe(1);
  expect(renderer.kind()).toBeNull();
});

// ---------------------------------------------------------------------------
// The Pulse, as the surface reads it
// ---------------------------------------------------------------------------

/** The four cue kinds a Pulse raises, in the closed set's own numbering. */
const PULSE_EMITTED = 1;
const CHARGE_GATHERED = 2;
const PORT_OPENED = 3;
const INTERFERENCE_PUSHED = 12;

/**
 * A snapshot with one controlled Form, one Port for a cue to stand on, and the
 * cues named. `reach` is the Form record's own radius, Q8.8 units.
 */
function cued(
  step: number,
  kinds: number[],
  reach = 0,
  reduced = false,
): FrameState {
  const state = fieldOf([port({ node: 1, x: 2_048 * 16, y: 2_048 * 16 })]);
  state.header.step = step;
  state.header.reducedMotion = reduced;
  state.forms = [
    {
      id: 1,
      formOrdinal: 0,
      layer: 0,
      controlled: true,
      focus: false,
      pulseCharging: reach > 0,
      separated: false,
      x: 2_048,
      y: 2_048,
      vx: 0,
      vy: 0,
      charge: 20_000,
      radius: reach,
    },
  ];
  state.cues = kinds.map((kind) => ({ kind, name: null, a: 0, b: 1 }));
  return state;
}

/** The cue marks the newest frame drew, by their kind. */
function cueMarks(
  renderer: ReturnType<typeof create_renderer>,
): Map<number, { radius: number; tone: number; alpha: number; width: number }> {
  const found = new Map<number, { radius: number; tone: number; alpha: number; width: number }>();
  const scene = renderer.scene();
  for (let place = 0; place < scene.cues.count; place += 1) {
    const mark = scene.cues.items[place];
    found.set(mark.kind, {
      radius: mark.radius,
      tone: mark.tone,
      alpha: mark.alpha,
      width: mark.width,
    });
  }
  return found;
}

test('each cue of the Pulse reads as its own shape, hue, and direction', () => {
  const renderer = create_renderer(surface(), 'canvas2d');
  renderer.resize(WIDE, HIGH);
  const kinds = [PULSE_EMITTED, CHARGE_GATHERED, PORT_OPENED, INTERFERENCE_PUSHED];

  const opening = cued(10, kinds, 256 * 80);
  renderer.render(opening, opening, 0);
  const first = cueMarks(renderer);
  expect(first.size).toBe(kinds.length);

  // No two of them carry the same hue: what a cue says, it says by colour and
  // motion, and two readings never compete for one hue.
  expect(new Set([...first.values()].map((mark) => mark.tone)).size).toBe(kinds.length);

  const later = cued(19, [], 0);
  renderer.render(later, later, 0);
  const aged = cueMarks(renderer);

  // A Pulse opens outward from the Form, and Interference pushed opens further
  // and harder; Charge gathered closes inward, because what moved came in.
  expect(aged.get(PULSE_EMITTED)!.radius).toBeGreaterThan(first.get(PULSE_EMITTED)!.radius);
  expect(aged.get(PORT_OPENED)!.radius).toBeGreaterThan(first.get(PORT_OPENED)!.radius);
  expect(aged.get(INTERFERENCE_PUSHED)!.radius).toBeGreaterThan(
    aged.get(PULSE_EMITTED)!.radius,
  );
  expect(aged.get(CHARGE_GATHERED)!.radius).toBeLessThan(first.get(CHARGE_GATHERED)!.radius);

  // Every one of them fades as it goes, so the shape and the fading say the
  // same thing twice.
  for (const kind of kinds) {
    expect(aged.get(kind)!.alpha).toBeLessThan(first.get(kind)!.alpha);
  }

  // And a cue past its life is gone rather than standing.
  const gone = cued(40, [], 0);
  renderer.render(gone, gone, 0);
  expect(renderer.scene().cues.count).toBe(0);
  renderer.dispose();
});

/**
 * One active pressure of the frame's section 6, at any ordinal and stage.
 */
function pressureOf(
  ordinal: number,
  stage: FramePressure['stage'],
  level = 52_000,
  queued = false,
): FramePressure {
  return { ordinal, stage, targetKind: 'node', queued, level, target: 1 };
}

test('the rim carries one hue per pressure, in the closed set\'s own order', () => {
  // The doc-anchored mapping, ledgered since Goal 8: PRESSURE_TONES is one
  // hue per pressure of the closed set — drain, noise, fracture, flood,
  // interference, drift — indexed by the ordinal the frame's section 6
  // carries, which is the closed set's own order. One tone per id, no reuse,
  // and the palette and the decoder agree on how many pressures there are.
  expect(PRESSURE_TONES).toHaveLength(FRAME_PRESSURE_IDS.length);
  expect(new Set(PRESSURE_TONES).size).toBe(PRESSURE_TONES.length);
  expect(FRAME_PRESSURE_IDS).toEqual([
    'drain',
    'noise',
    'fracture',
    'flood',
    'interference',
    'drift',
  ]);

  // The reading on the surface follows the mapping: for every ordinal, the
  // rim takes exactly that pressure's tone.
  for (let ordinal = 0; ordinal < FRAME_PRESSURE_IDS.length; ordinal += 1) {
    const renderer = create_renderer(surface(), 'canvas2d');
    renderer.resize(WIDE, HIGH);
    const state = fieldOf([port({ node: 1, x: 2_048 * 16, y: 2_048 * 16 })]);
    state.pressures = [pressureOf(ordinal, 'pressure')];
    renderer.render(state, state, 0);
    const rim = renderer.scene().rim;
    expect(rim.tone).toBe(PRESSURE_TONES[ordinal]);
    expect(rim.level).toBeGreaterThan(0);
    expect(rim.crisis).toBe(false);
    renderer.dispose();
  }
});

test('a crisis beats, a queued pressure reads as nothing, and reduced motion stills the beat', () => {
  const drawn = (pressures: FramePressure[], reduced = false) => {
    const renderer = create_renderer(surface(), 'canvas2d');
    renderer.resize(WIDE, HIGH);
    const state = fieldOf([port({ node: 1, x: 2_048 * 16, y: 2_048 * 16 })]);
    state.header.reducedMotion = reduced;
    state.pressures = pressures;
    renderer.render(state, state, 1 / 3);
    const rim = { ...renderer.scene().rim };
    renderer.dispose();
    return rim;
  };

  // A pressure at its crisis stage is the one reading that beats.
  const crisis = drawn([pressureOf(4, 'crisis')]);
  expect(crisis.crisis).toBe(true);
  expect(crisis.tone).toBe(PRESSURE_TONES[4]);
  expect(crisis.beat).toBeGreaterThan(0);

  // Reduced motion keeps the reading and stills the movement, exactly as it
  // does for trails and particles: the hue and the depth stand, the beat does
  // not.
  const stilled = drawn([pressureOf(4, 'crisis')], true);
  expect(stilled.crisis).toBe(true);
  expect(stilled.level).toBe(crisis.level);
  expect(stilled.beat).toBe(0);

  // A queued pressure is a seat requested, not a pressure standing: the rim
  // reads nothing from it.
  const waiting = drawn([pressureOf(0, 'signal', 52_000, true)]);
  expect(waiting.level).toBe(0);

  // Two active pressures: the further reading wins the rim, which is the at
  // most-two limit read as a surface rule — one reading at a time, the worse
  // one.
  const paired = drawn([pressureOf(0, 'signal', 30_000), pressureOf(4, 'crisis', 52_000)]);
  expect(paired.tone).toBe(PRESSURE_TONES[4]);
});

test('the hard cue kinds are exactly the doc\'s own three, and read hard', () => {
  // Doc-anchored, ledgered since Goal 8: of the closed cue set, the kinds
  // that report something wrong rather than something done are 5 (route
  // cut), 6 (break occurred), and 8 (collapse). They open in the hard hue
  // and wider; every other unshaped kind reads soft.
  const HARD = [5, 6, 8];
  const SOFT = [4, 7, 9, 10, 11];
  const renderer = create_renderer(surface(), 'canvas2d');
  renderer.resize(WIDE, HIGH);
  const state = cued(10, [...HARD, ...SOFT]);
  renderer.render(state, state, 0);
  const marks = cueMarks(renderer);
  const scene = renderer.scene();
  const hardOf = new Map<number, boolean>();
  for (let place = 0; place < scene.cues.count; place += 1) {
    const mark = scene.cues.items[place];
    hardOf.set(mark.kind, mark.hard);
  }
  for (const kind of HARD) {
    expect(hardOf.get(kind), `cue ${kind} reads hard`).toBe(true);
  }
  for (const kind of SOFT) {
    expect(hardOf.get(kind), `cue ${kind} reads soft`).toBe(false);
  }
  // The two registers are two hues, and each register is one hue.
  const hardTones = new Set(HARD.map((kind) => marks.get(kind)!.tone));
  const softTones = new Set(SOFT.map((kind) => marks.get(kind)!.tone));
  expect(hardTones.size).toBe(1);
  expect(softTones.size).toBe(1);
  expect([...hardTones][0]).not.toBe([...softTones][0]);
  renderer.dispose();
});

test("a Pulse's ring expands to the reach the frame carried", () => {
  const renderer = create_renderer(surface(), 'canvas2d');
  renderer.resize(WIDE, HIGH);

  // The Form record carries the radius as Q8.8 units, and the ring reaches it.
  const wide = cued(10, [PULSE_EMITTED], 256 * 160);
  renderer.render(wide, wide, 0);
  const reached = cueMarks(renderer).get(PULSE_EMITTED)!.radius;

  const narrow = cued(10, [PULSE_EMITTED], 256 * 40);
  const other = create_renderer(surface(), 'canvas2d');
  other.resize(WIDE, HIGH);
  other.render(narrow, narrow, 0);
  expect(cueMarks(other).get(PULSE_EMITTED)!.radius).toBeLessThan(reached);

  // A frame carrying no radius — which is every frame the core sends until the
  // rule that fills it is locked — still draws a ring rather than nothing.
  const bare = cued(10, [PULSE_EMITTED], 0);
  const third = create_renderer(surface(), 'canvas2d');
  third.resize(WIDE, HIGH);
  third.render(bare, bare, 0);
  expect(cueMarks(third).get(PULSE_EMITTED)!.radius).toBeGreaterThan(0);

  renderer.dispose();
  other.dispose();
  third.dispose();
});

test('the particles a cue carries are motion, and reduced motion drops them', () => {
  const moving = create_renderer(surface(), 'canvas2d');
  moving.resize(WIDE, HIGH);
  const state = cued(10, [CHARGE_GATHERED, INTERFERENCE_PUSHED], 256 * 80);
  moving.render(state, state, 0);
  const spread = moving.scene().particles.count;
  expect(spread).toBeGreaterThan(0);

  const still = create_renderer(surface(), 'canvas2d');
  still.resize(WIDE, HIGH);
  const quiet = cued(10, [CHARGE_GATHERED, INTERFERENCE_PUSHED], 256 * 80, true);
  still.render(quiet, quiet, 0);
  expect(still.scene().particles.count).toBe(0);
  // The ring itself stands either way: the reading holds whatever the setting.
  expect(still.scene().cues.count).toBe(2);

  moving.dispose();
  still.dispose();
});

test('the stand-in Field raises the Pulse cues the renderer draws', () => {
  const renderer = create_renderer(surface(), 'canvas2d');
  renderer.resize(WIDE, HIGH);
  const kinds = new Set<number>();
  for (let step = 0; step <= 260; step += 1) {
    const state = fixtureSnapshot(step);
    renderer.render(state, state, 0);
    for (const cue of state.cues) kinds.add(cue.kind);
  }
  // A release that reached something, a release that reached nothing, a Port
  // opening, and Interference pushed away: the four readings the Pulse has to
  // be legible through.
  expect(kinds.has(PULSE_EMITTED)).toBe(true);
  expect(kinds.has(CHARGE_GATHERED)).toBe(true);
  expect(kinds.has(PORT_OPENED)).toBe(true);
  expect(kinds.has(INTERFERENCE_PUSHED)).toBe(true);
  renderer.dispose();
});

test('every candidate of a slate draws its own outline, on both engines', () => {
  // The candidates are not in the snapshot: a slate record crosses on demand,
  // and what the renderer is handed is the inside each candidate declares. The
  // positions come from the frame, which is the one place they live.
  const state = fieldOf([
    port({ node: 1, member: true, x: 1_000 * 16, y: 1_000 * 16 }),
    port({ node: 2, member: true, x: 1_300 * 16, y: 1_000 * 16 }),
    port({ node: 3, x: 1_150 * 16, y: 1_300 * 16 }),
    port({ node: 4, x: 1_400 * 16, y: 1_400 * 16 }),
  ]);
  const slate = [
    { position: 1, members: [1, 2], focused: false, tier: 1 },
    { position: 2, members: [2, 3, 4], focused: true, tier: 2 },
  ];

  for (const kind of ['canvas2d', 'webgl'] as const) {
    const renderer = create_renderer(surface(), kind, {
      probeWebgl: () => false,
      startWebgl: async () => {
        throw new Error('the fallback is the engine under test');
      },
    });
    renderer.resize(WIDE, HIGH);
    renderer.set_still_tool('view');
    renderer.set_candidates(slate);
    renderer.render(state, state, 0);
    const scene = renderer.scene();
    // One outline per candidate, beside the physical compartment.
    expect(scene.candidates.count).toBe(2);
    expect(scene.boundaries.count).toBe(1);
    expect(scene.candidates.items[0].nodes).toEqual([1, 2]);
    expect(scene.candidates.items[0].candidate).toBe(1);
    expect(scene.candidates.items[0].focused).toBe(false);
    expect(scene.candidates.items[1].candidate).toBe(2);
    expect(scene.candidates.items[1].focused).toBe(true);
    // The focused one is the brighter of the two, and neither is the tone the
    // standing boundary carries.
    expect(scene.candidates.items[1].alpha).toBeGreaterThan(scene.candidates.items[0].alpha);
    expect(scene.candidates.items[0].tone).not.toBe(scene.boundaries.items[0].tone);
    expect(scene.candidates.items[1].tone).not.toBe(scene.candidates.items[0].tone);
    // The tier reads on the outline itself, without a colour and without a
    // figure: a deeper tier is drawn sparser, and both engines read the one
    // rule that says so.
    expect(scene.candidates.items.map((mark) => mark.tier).slice(0, 2)).toEqual([1, 2]);
    expect(dashOf(scene.candidates.items[0])[1]).toBeLessThan(
      dashOf(scene.candidates.items[1])[1],
    );
    expect(scene.boundaries.items[0].role).toBe('compartment');
    expect(dashOf(scene.boundaries.items[0])).toEqual([]);
    // And the engine drew them: both engines take the same marks.
    expect(recorded.total()).toBeGreaterThan(0);
    renderer.dispose();
  }
});

test('a slate handed over between frames reaches the surface without one', () => {
  // A still run runs no step, so no snapshot need follow a slate: the setter
  // redraws, exactly as a resize does.
  const state = fieldOf([
    port({ node: 1, member: true, x: 1_000 * 16, y: 1_000 * 16 }),
    port({ node: 2, x: 1_300 * 16, y: 1_000 * 16 }),
  ]);
  const renderer = create_renderer(surface(), 'canvas2d');
  renderer.resize(WIDE, HIGH);
  renderer.render(state, state, 0);
  expect(renderer.scene().candidates.count).toBe(0);
  renderer.set_still_tool('view');
  renderer.set_candidates([{ position: 1, members: [1, 2], focused: false, tier: 1 }]);
  expect(renderer.scene().candidates.count).toBe(1);
  // And a slate taken away leaves nothing behind.
  renderer.set_candidates([]);
  expect(renderer.scene().candidates.count).toBe(0);
  renderer.dispose();
});

test('physical, proposed-physical, and View hulls remain separate render registers', () => {
  const ports = [
    port({ node: 1, member: true, x: 1_000 * 16, y: 1_000 * 16 }),
    port({ node: 2, member: true, proposedMember: true, x: 1_300 * 16, y: 1_000 * 16 }),
    port({ node: 3, member: true, proposedMember: true, x: 1_150 * 16, y: 1_300 * 16 }),
    port({ node: 4, proposedMember: true, x: 1_450 * 16, y: 1_300 * 16 }),
  ];
  const state = fieldOf(ports, [], [1, 2, 4]);
  const renderer = create_renderer(surface(), 'canvas2d');
  renderer.resize(WIDE, HIGH);
  renderer.render(state, state, 0);
  const marks = renderer.scene().boundaries.items.slice(0, renderer.scene().boundaries.count);
  const physical = marks.find((mark) => mark.role === 'compartment' && !mark.proposed);
  const proposed = marks.find((mark) => mark.role === 'compartment' && mark.proposed);
  const view = marks.find((mark) => mark.role === 'view');

  expect([...(physical?.nodes ?? [])].sort((a, b) => a - b)).toEqual([1, 2, 3]);
  expect([...(proposed?.nodes ?? [])].sort((a, b) => a - b)).toEqual([2, 3, 4]);
  expect([...(view?.nodes ?? [])].sort((a, b) => a - b)).toEqual([1, 2, 4]);
  expect(physical!.width).toBeGreaterThan(view!.width);
  expect(dashOf(physical!)).toEqual([]);
  expect(dashOf(proposed!)).toEqual([4, 6]);
  expect(dashOf(view!)).toEqual([12, 8]);
  renderer.dispose();
});

/** A still-surface frame over the same ports, for the playback readings. */
function stillFieldOf(ports: FramePort[]): FrameState {
  const state = fieldOf(ports);
  state.header.mode = 'still';
  state.header.timeScale = 0;
  state.header.stillVisible = true;
  return state;
}

test('a playback reading draws motion on the members, on both engines', async () => {
  // The reading is not in the snapshot: a perturbation result crosses on
  // demand, the shell holds it, and the renderer is handed the played sample's
  // series — exactly as the slate crosses. Where a member stands is still the
  // frame's own.
  const state = stillFieldOf([
    port({ node: 2, member: true, x: 1_000 * 16, y: 1_000 * 16 }),
    port({ node: 3, member: true, x: 1_300 * 16, y: 1_000 * 16 }),
    port({ node: 5, x: 1_150 * 16, y: 1_300 * 16 }),
  ]);
  const reading = {
    members: [2, 3],
    series: [4 * 65536, 8 * 65536, 12 * 65536],
    base: [4 * 65536, 6 * 65536, 8 * 65536],
  };

  // The Canvas2D engine, through the renderer that owns the scene fill.
  const renderer = create_renderer(surface(), 'canvas2d');
  renderer.resize(WIDE, HIGH);
  renderer.render(state, state, 0);
  expect(renderer.scene().playback.count).toBe(0);
  const before = recorded.total();
  renderer.set_playback(reading);
  const scene = renderer.scene();
  // One mark per member, each riding its own Port's place, both series read
  // against one scale: at the first window step the played series sits at its
  // own floor and the base beside it, together — the reading is their
  // difference.
  expect(scene.playback.count).toBe(2);
  expect(scene.playback.items.slice(0, 2).map((mark) => mark.node)).toEqual([2, 3]);
  const played = scene.playback.items[0];
  const anchor = scene.ports.items.find((held) => held.node === 2);
  expect(played.x).toBe(anchor?.x);
  expect(played.y).toBe(anchor?.y);
  expect(played.factor).toBe(0);
  expect(played.base).toBe(0);
  expect(played.alpha).toBeGreaterThan(0);
  // And the setter redrew: a reading arrives between frames, and a still run
  // runs no step to carry it to the surface.
  expect(recorded.total()).toBeGreaterThan(before);
  renderer.dispose();

  // The WebGL engine, over the same scene: the marks are the same marks, and
  // the rings it emits are the shared swell rule's own radii.
  const { WebglEngine } = await import('../src/render/webgl');
  const { Graphics } = await import('pixi.js');
  const circles: number[] = [];
  const spy = vi
    .spyOn(Graphics.prototype, 'circle')
    .mockImplementation(function (this: unknown, ...given: unknown[]) {
      circles.push(given[2] as number);
      return this as never;
    } as never);
  const engine = new WebglEngine({
    resize: () => {},
    render: () => {},
    destroy: () => {},
  } as never);
  engine.draw(scene);
  spy.mockRestore();
  const { playbackSwell } = await import('../src/render/engine');
  for (const mark of scene.playback.items.slice(0, scene.playback.count)) {
    expect(circles).toContain(playbackSwell(mark.radius, mark.factor));
    expect(circles).toContain(playbackSwell(mark.radius, mark.base ?? 0));
  }
});

test('reduced motion holds the last window step as a still comparison', () => {
  const state = stillFieldOf([port({ node: 2, member: true, x: 1_000 * 16, y: 1_000 * 16 })]);
  state.header.reducedMotion = true;
  const reading = {
    members: [2],
    series: [0, 32768, 65536],
    base: [0, 16384, 65536],
  };
  const renderer = create_renderer(surface(), 'canvas2d');
  renderer.resize(WIDE, HIGH);
  renderer.render(state, state, 0);
  renderer.set_playback(reading);
  // The reading is reduced, never removed: the mark stands at the last window
  // step — the played series at its ceiling, the base beside it — and stands
  // there however the clock moves, a still comparison rather than a loop.
  const mark = renderer.scene().playback.items[0];
  expect(renderer.scene().playback.count).toBe(1);
  expect(mark.factor).toBe(1);
  expect(mark.base).toBe(1);
  renderer.render(state, state, 0);
  expect(renderer.scene().playback.items[0].factor).toBe(1);
  renderer.dispose();
});

test('nothing outside the reading members is modulated', () => {
  const state = stillFieldOf([
    port({ node: 2, member: true, x: 1_000 * 16, y: 1_000 * 16 }),
    port({ node: 3, member: true, x: 1_300 * 16, y: 1_000 * 16 }),
    port({ node: 5, x: 1_150 * 16, y: 1_300 * 16 }),
    port({ node: 9, x: 1_400 * 16, y: 1_400 * 16 }),
  ]);
  const renderer = create_renderer(surface(), 'canvas2d');
  renderer.resize(WIDE, HIGH);
  renderer.render(state, state, 0);
  renderer.set_playback({ members: [3], series: [65536, 32768], base: null });
  const scene = renderer.scene();
  // One mark, on the one member the reading names — a Node outside the
  // reading's inside takes no modulation at all, member of the standing View
  // or not — and a kind with no base series draws no second ring.
  expect(scene.playback.count).toBe(1);
  expect(scene.playback.items[0].node).toBe(3);
  expect(scene.playback.items[0].base).toBeNull();
  renderer.dispose();
});

test('a running surface carries no playback, structurally', () => {
  // The shell's gate: the offer is null in ordinary play, whatever record is
  // held. The renderer's own half of the rule is that a null reading leaves
  // nothing in the scene — no mark, and nothing for either engine to draw.
  const state = fieldOf([port({ node: 2, member: true, x: 1_000 * 16, y: 1_000 * 16 })]);
  const renderer = create_renderer(surface(), 'canvas2d');
  renderer.resize(WIDE, HIGH);
  renderer.set_playback({ members: [2], series: [65536], base: null });
  renderer.render(state, state, 0);
  // A running frame eases the still surface toward absence, and the reading
  // rides the surface: with the shell's gate passing null nothing stands at
  // all.
  renderer.set_playback(null);
  expect(renderer.scene().playback.count).toBe(0);
  renderer.render(state, state, 0);
  expect(renderer.scene().playback.count).toBe(0);
  renderer.dispose();
});

// ---------------------------------------------------------------------------
// The Handoff, in both engines
// ---------------------------------------------------------------------------

/** Two Forms standing far apart, with control on the one the caller names. */
function twoForms(controlled: number): FrameState {
  const state = fieldOf([]);
  state.forms = [1, 2].map((id) => ({
    id,
    formOrdinal: 0,
    layer: 0,
    controlled: id === controlled,
    focus: false,
    pulseCharging: false,
    separated: false,
    x: id === 1 ? 1_000 : 3_000,
    y: 2_048,
    vx: 0,
    vy: 0,
    charge: 0,
    radius: 0,
  }));
  return state;
}

test('the controlled mark moves to the Form a Handoff handed control to, in both engines', () => {
  for (const kind of ['canvas2d', 'webgl'] as const) {
    recorded = recordingContext();
    const renderer = create_renderer(surface(), kind, {
      probeWebgl: () => false,
      startWebgl: async () => {
        throw new Error('the fallback is the engine under test');
      },
    });
    renderer.resize(WIDE, HIGH);

    const before = twoForms(1);
    renderer.render(before, before, 0);
    // `FormMark` carries no identifier: the marks stand in the frame's own
    // order, so place 0 is Form 1 and place 1 is Form 2.
    const first = renderer.scene().forms.items.slice(0, renderer.scene().forms.count);
    expect(first.map((mark) => mark.controlled)).toEqual([true, false]);
    const held = { ...first[0] };

    // The same Field with the flag moved: every reading the controlled flag
    // decides moves with it, and the engine drew the frame either way.
    const after = twoForms(2);
    renderer.render(after, after, 0);
    const marks = renderer.scene().forms.items.slice(0, renderer.scene().forms.count);
    expect(marks.map((mark) => mark.controlled)).toEqual([false, true]);
    expectSteeredWidest(renderer.scene());
    expect(marks[0].radius).toBeLessThan(held.radius);
    expect(marks[1].radius).toBeGreaterThan(marks[0].radius);
    expect(marks[0].tone).not.toEqual(held.tone);
    expect(marks[0].ringTone).not.toEqual(held.ringTone);
    expect(marks[1].tone).toEqual(held.tone);
    expect(marks[1].ringTone).toEqual(held.ringTone);
    expect(recorded.total()).toBeGreaterThan(0);
    renderer.dispose();
  }
});

test('the camera snaps to the Form control moved to rather than easing across', () => {
  const renderer = create_renderer(surface(), 'canvas2d');
  renderer.resize(WIDE, HIGH);

  // Settled on the first Form: the first frame places the camera outright, and
  // a second frame of the same Field leaves it there.
  const before = twoForms(1);
  renderer.render(before, before, 0);
  renderer.render(before, before, 0);
  expect(renderer.scene().camera.x).toBeCloseTo(1_000, 3);

  // A Handoff moves the target 2,000 units. An eased camera would arrive a
  // fraction of the way; the snap arrives.
  const after = twoForms(2);
  renderer.render(after, after, 0);
  expect(renderer.scene().camera.x).toBeCloseTo(3_000, 3);

  // And an ordinary move of the same Form still eases: the snap is the
  // Handoff's, not every camera change's.
  const moved = twoForms(2);
  moved.forms[1].x = 1_000;
  renderer.render(moved, moved, 0);
  const eased = renderer.scene().camera.x;
  expect(eased).toBeLessThan(3_000);
  expect(eased).toBeGreaterThan(1_000);
  renderer.dispose();
});
