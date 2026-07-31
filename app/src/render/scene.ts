/**
 * The scene: one snapshot pair, projected into marks an engine can draw.
 *
 * Both engines — the PixiJS one and the Canvas2D fallback — draw the same
 * scene, so the two carry the same responsibilities and differ only in
 * fidelity. Everything that reads the snapshot happens here, once, and nothing
 * downstream of it touches a `FrameState`.
 *
 * The scene is a pure function of the pair of snapshots, the interpolation
 * fraction, the viewport, the motion profile, and the ephemera the renderer
 * derived from earlier snapshots. It holds no simulation state, and it never
 * writes back.
 *
 * Marks are pooled: one scene is filled and refilled in place for the life of
 * the renderer, so the marks themselves are allocated once and reused. What a
 * frame still allocates is bounded and short-lived, and worth listing rather
 * than claiming away:
 *
 * - two `Map`s of Node place and plane, one per snapshot of the pair, each
 *   bounded by the 256-Node cap;
 * - one `Map` of the members of the standing inside, grouped by plane, and one
 *   array per plane it reaches;
 * - the closure `place` and the small `{ x, y }` it answers with, one per
 *   projected point;
 * - the sorted copy the convex hull takes of one plane's members.
 *
 * All of it is released with the frame that made it; none of it grows with
 * time.
 * The pools are what keep the per-frame count flat as the Field fills.
 */

import {
  BACKDROP,
  BOUNDARY,
  BOUNDARY_PROPOSED,
  CANDIDATE,
  CANDIDATE_FOCUSED,
  CHARGE_CORE,
  CHARGE_HIGH,
  CHARGE_LOW,
  CUE_HARD,
  CUE_SOFT,
  CURRENT_BRIGHT,
  CURRENT_PLAIN,
  FORECAST,
  FORM_CONTROLLED,
  FORM_CONTROLLED_RING,
  FORM_PLAIN,
  HANDLE_BOUNDARY,
  HANDLE_PORT,
  HANDLE_ROUTE,
  HAZE,
  HAZE_TINT,
  mixTone,
  OVERLOAD,
  PORT_BASE,
  PRESSURE_TONES,
  ROUTE_BASE,
  ROUTE_CUT_QUEUED,
  ROUTE_FLOW,
  ROUTE_MOVE_QUEUED,
  ROUTE_PROPOSED,
  type Tone,
} from './palette';
import { Ephemera, FIELD_UNITS, portUnits } from './history';
import type { FramePort, FrameState } from '../../../worker/src/frame-state';

/**
 * The Route statuses that report a queued change rather than a standing Route:
 * one the queue would cut, one it would move an end of, and one it proposes
 * that no Route stands at.
 */
const QUEUED_STATUS: readonly number[] = [1, 3, 4];

/** The status a record carries when it is a proposal and not a Route at all. */
const PROPOSED_STATUS = 4;

/** How much of the Field the shorter side of the surface shows, in units. */
const VIEW_UNITS = 1400;

/** The widest a Form is drawn, in units, before its Charge lifts it. */
const FORM_UNITS = 26;

/** The widest a Port is drawn, in units. */
const PORT_UNITS = 15;

/** How wide the boundary's own edge is drawn, in units. */
const BOUNDARY_UNITS = 3;

/** How far one layer of separation pushes a plane away. */
const DEPTH_SCALE = 0.22;

/** How far one layer of separation in front pulls a plane forward. */
const NEAR_SCALE = 0.3;

/** How far above every other Form's mark the controlled one is held. */
const CONTROLLED_MARGIN = 1.4;

/** The eight Forms' drawn side counts, in the closed set's own order. */
const FORM_SIDES = [3, 6, 4, 5, 8, 7, 3, 9];

/** How many soft particles one band segment or flowing Route carries. */
const PARTICLES_PER_SEGMENT = 5;

/** The most particles a scene carries, whatever the Field holds. */
const PARTICLE_CAP = 384;

/** What the surface is, in device pixels and in the ratio it was sized at. */
export interface Viewport {
  width: number;
  height: number;
  dpr: number;
}

/**
 * The reduced-motion and trail settings `InputConfig` carries. `intensity` is
 * the raw `Frac` the configuration holds, 65536 being full.
 */
export interface MotionProfile {
  reducedMotion: boolean;
  trailIntensity: number;
}

/** The profile a renderer stands on until one is set. */
export const FULL_MOTION: MotionProfile = { reducedMotion: false, trailIntensity: 65536 };

/** A reused mark array: `count` is how many of `items` this frame filled. */
export class Pool<T> {
  readonly items: T[] = [];
  count = 0;

  constructor(private readonly make: () => T) {}

  next(): T {
    if (this.count === this.items.length) this.items.push(this.make());
    this.count += 1;
    return this.items[this.count - 1];
  }

  reset(): void {
    this.count = 0;
  }
}

/** A plane of haze standing between the camera and one layer of the Field. */
export interface HazeMark {
  /** Layers away from the camera's own: positive deeper, negative nearer. */
  depth: number;
  alpha: number;
  tone: Tone;
}

export interface PortMark {
  /** The Node this mark stands for, which is what a handle on it names. */
  node: number;
  x: number;
  y: number;
  radius: number;
  /** The closed Node-kind set's own order: 0 port, 1 reserve, 2 module, 3 form. */
  kind: number;
  tone: Tone;
  /** How far the soft bloom around the mark reaches, in device pixels. */
  bloom: number;
  /** 0 to 1: how much Charge this Node holds against the most any holds. */
  charge: number;
  overloaded: boolean;
  member: boolean;
  shell: boolean;
  /** Whether a queued change would make this Node a member. A preview. */
  proposedMember: boolean;
  /** 0 to 1: how lately a current delivered here. */
  delivered: number;
  /** 0 to 1: the Charge held in reserve, against this Node's own threshold. */
  reserve: number;
  alpha: number;
  /** Layers away from the camera's own; only a Form's own Node is ever off it. */
  depth: number;
}

export interface RouteMark {
  /** The Route this mark stands for, which is what a handle on it names. */
  route: number;
  x1: number;
  y1: number;
  x2: number;
  y2: number;
  tone: Tone;
  width: number;
  alpha: number;
  /** 0 to 1 of the Route's own capacity. */
  flow: number;
  /** 0 standing, 1 cut queued, 2 overloaded, 3 move queued, 4 proposed. */
  status: number;
  /**
   * Whether this mark is a queued change rather than a standing Route. Both
   * engines draw a preview broken rather than solid, so what the queue would do
   * never reads as something the Field already holds.
   */
  preview: boolean;
  /** Layers away from the camera's own: the shallower of its two ends. */
  depth: number;
}

export interface FormMark {
  x: number;
  y: number;
  radius: number;
  tone: Tone;
  ringTone: Tone;
  /** What the Form's own body is filled with. */
  bodyTone: Tone;
  controlled: boolean;
  focus: boolean;
  pulseCharging: boolean;
  /** 0 to 1 of the stored-Charge cap. */
  charge: number;
  sides: number;
  /** The direction the Form is travelling, in radians. */
  heading: number;
  alpha: number;
  depth: number;
}

export interface TrailMark {
  /** Device-pixel x, y pairs, oldest first. */
  points: number[];
  tone: Tone;
  alpha: number;
  width: number;
}

export interface CurrentMark {
  id: number;
  layer: number;
  bright: boolean;
  active: boolean;
  /** 0 to 1 of the locked strength cap. */
  strength: number;
  /** The current's own phase counter, as a fraction of a full turn. */
  phase: number;
  /** Device-pixel x, y pairs: the path the current runs, on its own plane. */
  points: number[];
  /** The band a Node has to stand within to receive, in device pixels. */
  width: number;
  tone: Tone;
  alpha: number;
  depth: number;
}

export interface BoundaryMark {
  /**
   * Device-pixel x, y pairs, closing back on the first. One pair is a point
   * and two are a segment; both are drawn as a capsule of `width`.
   */
  points: number[];
  tone: Tone;
  alpha: number;
  /** How wide the edge is drawn, in device pixels. */
  width: number;
  /** The member Node each point stands at, in the same order as the points. */
  nodes: number[];
  /**
   * Whether this is the boundary a queued change proposes rather than the
   * standing one. A frame carrying a queued `reshape_boundary` or `set_focus`
   * puts up both, so the shape standing and the shape proposed are read
   * together.
   */
  proposed: boolean;
  /**
   * The candidate's assembly position, which is its presentation position, and
   * 0 when the mark is not a candidate's at all.
   *
   * A candidate outline is drawn from the slate the shell holds rather than
   * from the snapshot: the frame carries only what the renderer needs every
   * frame, and a slate record crosses on demand. What the snapshot supplies is
   * where each Node stands, which is what the outline is drawn through.
   */
  candidate: number;
  /** Whether the queue proposes adopting this candidate. */
  focused: boolean;
  /**
   * The tier the ranking put this candidate in, and 0 when the mark is not a
   * candidate's or the slate was never compared. The engines draw it as the
   * outline's own sparseness, which is what carries the grouping without a
   * figure and without a colour.
   */
  tier: number;
}

/**
 * One candidate of the standing slate, as the renderer takes it: the position
 * it stands at, the Nodes of its inside, and whether the queue proposes it.
 */
export interface CandidateOutline {
  position: number;
  members: readonly number[];
  focused: boolean;
  /** The tier the ranking put it in, and 0 on a slate no comparison ran on. */
  tier: number;
}

/**
 * One perturbation result's compact playback series, as the shell hands it to
 * the renderer.
 *
 * `members` is the result's own `view.inside`, ascending; `series` the played
 * sample's replayed stored-Charge series, one raw `Fx` per window step; `base`
 * the same sample's unshifted-schedule series for `delayed-replay` and null
 * for every other kind. The modulation scale is [smallest, largest] over
 * `series ∪ base`, computed where the marks are filled so both engines read
 * one normalization.
 */
export interface PlaybackReading {
  members: readonly number[];
  series: readonly number[];
  base: readonly number[] | null;
}

/**
 * One member mark of a playback reading: where the member stands, and how far
 * up the reading's own range the played step sits.
 *
 * `factor` and `base` are in [0, 1] against the reading's scale. What the
 * engines draw from them is motion — a ring that swells and falls around the
 * member's own mark, and a second, fainter ring for the base when one stands,
 * together, because the reading is their difference. No axis, no strip, and no
 * number is ever derived from these.
 */
export interface PlaybackMark {
  node: number;
  x: number;
  y: number;
  radius: number;
  factor: number;
  /** The base series' own factor at the same step, and none without one. */
  base: number | null;
  tone: Tone;
  alpha: number;
}

export interface ParticleMark {
  x: number;
  y: number;
  radius: number;
  tone: Tone;
  alpha: number;
}

export interface CueMark {
  x: number;
  y: number;
  radius: number;
  tone: Tone;
  alpha: number;
  width: number;
  /** Whether the cue reports something wrong. */
  hard: boolean;
  /** The closed cue set's own kind byte, 1 through 12. */
  kind: number;
}

/** The three things a paused Field offers to be taken hold of. */
export const HANDLE_KIND = { port: 0, route: 1, boundary: 2 } as const;

/**
 * One handle: a place on the Field a change can be started from.
 *
 * Which places those are is the `PlanCommand` union read back as geometry, and
 * nothing more: a `connect` runs between two Ports, a `redirect` moves one end
 * of a Route, a `cut` takes a Route, and a `reshape_boundary` moves the members
 * the standing inside is drawn around. So a Port carries a handle, each Route
 * carries one at each of its two ends, and every vertex of a drawn boundary
 * carries one. Drawing them is this goal's; making them draggable is the goal
 * that owns queued changes, and the geometry is the same either way.
 */
export interface HandleMark {
  x: number;
  y: number;
  /**
   * What the handle names: the Node a Port handle stands on, the member a
   * Boundary vertex is drawn around, or 0 for a Route handle, which names its
   * Route instead.
   *
   * A handle is the `PlanCommand` union read back as a place on the Field, so
   * it carries what the command it starts would name. Without that the surface
   * would draw places a change can be started from and leave the shell to guess
   * which change each one is.
   */
  node: number;
  /** The Route a Route handle stands on, and 0 for the other two kinds. */
  route: number;
  /** Which end of that Route the handle sits at: 0 the tail, 1 the head. */
  end: number;
  /** How far the handle reaches, in device pixels. */
  radius: number;
  /** `HANDLE_KIND`: 0 a Port, 1 a Route end, 2 a Boundary vertex. */
  kind: number;
  tone: Tone;
  alpha: number;
  /** How wide the handle's own outline is drawn, in device pixels. */
  width: number;
}

/**
 * The forecast overlay: the standing View's baseline `q_I` envelope over its
 * window, drawn as a band inside a strip at the foot of the surface.
 *
 * `count` is how many window steps the envelope carries, and it is 0 until the
 * goal that replays baselines computes one — the frame says so itself, because
 * the overlay section is present in `still` whether or not it holds anything.
 * A strip with nothing in it is drawn all the same: the surface Still Mode
 * offers is part of what Still Mode is, and an empty one says plainly that the
 * forward reading is not there yet rather than silently leaving it out.
 */
export interface ForecastMark {
  /** 0 to 1: how far the overlay has come up. Nothing is drawn at 0. */
  alpha: number;
  /** The strip the reading is drawn inside, in device pixels. */
  x: number;
  y: number;
  width: number;
  height: number;
  /** The envelope, as device-pixel x, y pairs along the low and high edges. */
  low: number[];
  high: number[];
  /** How many window steps the frame's envelope carried. */
  count: number;
  tone: Tone;
}

/** The pressure reading the edge of the surface carries. */
export interface RimMark {
  tone: Tone;
  /** 0 to 1: how far in from the edge the reading reaches. */
  level: number;
  /** 0 to 1: how hard the reading is beating. */
  beat: number;
  /** Whether a pressure stands at its crisis stage. */
  crisis: boolean;
}

export interface Scene {
  width: number;
  height: number;
  dpr: number;
  /** Where the camera sits, in units, and how many device pixels a unit is. */
  camera: { x: number; y: number; zoom: number; layer: number };
  /** Simulated time in steps, fractional between two of them. */
  clock: number;
  /**
   * 0 to 1: the time scale the snapshot reports. The header carries it as a
   * saturating Q0.16, so full speed reads 65535 and reads back as 1 here; a
   * ramp into Still Mode walks it down to 0.
   */
  timeScale: number;
  /**
   * 0 to 1: how far the surface has settled as the time scale falls. It is the
   * one reading a viewer gets that the Field has stopped moving on purpose.
   */
  stillness: number;
  /**
   * The Still Mode surface: how far it has come up, and whether the run has
   * arrived at the pause it is coming up for.
   *
   * `presence` is eased toward the mode's own target — 0 while running,
   * the falling or rising scale through the two ramps, 1 in `still` — because
   * a snapshot reaches the renderer only when a step ran or the mode changed,
   * and the end of an entry ramp runs no step. `paused` is the header's own
   * still-surface flag, which stands exactly while the run is fully still.
   */
  still: { presence: number; paused: boolean };
  reducedMotion: boolean;
  backdrop: Tone;
  hazes: Pool<HazeMark>;
  currents: Pool<CurrentMark>;
  routes: Pool<RouteMark>;
  boundaries: Pool<BoundaryMark>;
  /** One outline per candidate of the standing slate, drawn under them. */
  candidates: Pool<BoundaryMark>;
  ports: Pool<PortMark>;
  trails: Pool<TrailMark>;
  forms: Pool<FormMark>;
  particles: Pool<ParticleMark>;
  cues: Pool<CueMark>;
  handles: Pool<HandleMark>;
  /** The playback reading's member marks, and none while none is held. */
  playback: Pool<PlaybackMark>;
  forecast: ForecastMark;
  rim: RimMark;
}

/** An empty scene, to be filled and refilled for the life of one renderer. */
export function createScene(): Scene {
  return {
    width: 0,
    height: 0,
    dpr: 1,
    camera: { x: FIELD_UNITS / 2, y: FIELD_UNITS / 2, zoom: 1, layer: 0 },
    clock: 0,
    timeScale: 1,
    stillness: 0,
    still: { presence: 0, paused: false },
    reducedMotion: false,
    backdrop: BACKDROP,
    hazes: new Pool<HazeMark>(() => ({ depth: 0, alpha: 0, tone: HAZE })),
    currents: new Pool<CurrentMark>(() => ({
      id: 0,
      layer: 0,
      bright: false,
      active: false,
      strength: 0,
      phase: 0,
      points: [],
      width: 1,
      tone: CURRENT_PLAIN,
      alpha: 1,
      depth: 0,
    })),
    routes: new Pool<RouteMark>(() => ({
      route: 0,
      x1: 0,
      y1: 0,
      x2: 0,
      y2: 0,
      tone: ROUTE_BASE,
      width: 1,
      alpha: 1,
      flow: 0,
      status: 0,
      preview: false,
      depth: 0,
    })),
    boundaries: new Pool<BoundaryMark>(() => ({
      points: [],
      nodes: [],
      tone: BOUNDARY,
      alpha: 1,
      width: 2,
      proposed: false,
      candidate: 0,
      focused: false,
      tier: 0,
    })),
    candidates: new Pool<BoundaryMark>(() => ({
      points: [],
      nodes: [],
      tone: CANDIDATE,
      alpha: 1,
      width: 2,
      proposed: false,
      candidate: 0,
      focused: false,
      tier: 0,
    })),
    ports: new Pool<PortMark>(() => ({
      node: 0,
      x: 0,
      y: 0,
      radius: 1,
      kind: 0,
      tone: PORT_BASE,
      bloom: 0,
      charge: 0,
      overloaded: false,
      member: false,
      shell: false,
      proposedMember: false,
      delivered: 0,
      reserve: 0,
      alpha: 1,
      depth: 0,
    })),
    trails: new Pool<TrailMark>(() => ({ points: [], tone: FORM_PLAIN, alpha: 1, width: 1 })),
    forms: new Pool<FormMark>(() => ({
      x: 0,
      y: 0,
      radius: 1,
      tone: FORM_PLAIN,
      ringTone: FORM_PLAIN,
      bodyTone: BACKDROP,
      controlled: false,
      focus: false,
      pulseCharging: false,
      charge: 0,
      sides: 6,
      heading: 0,
      alpha: 1,
      depth: 0,
    })),
    particles: new Pool<ParticleMark>(() => ({ x: 0, y: 0, radius: 1, tone: CURRENT_BRIGHT, alpha: 1 })),
    cues: new Pool<CueMark>(() => ({
      x: 0,
      y: 0,
      radius: 0,
      tone: CUE_SOFT,
      alpha: 1,
      width: 1,
      hard: false,
      kind: 0,
    })),
    handles: new Pool<HandleMark>(() => ({
      node: 0,
      route: 0,
      end: 0,
      x: 0,
      y: 0,
      radius: 1,
      kind: HANDLE_KIND.port,
      tone: HANDLE_PORT,
      alpha: 1,
      width: 1,
    })),
    playback: new Pool<PlaybackMark>(() => ({
      node: 0,
      x: 0,
      y: 0,
      radius: 1,
      factor: 0,
      base: null,
      tone: FORECAST,
      alpha: 1,
    })),
    forecast: {
      alpha: 0,
      x: 0,
      y: 0,
      width: 0,
      height: 0,
      low: [],
      high: [],
      count: 0,
      tone: FORECAST,
    },
    rim: { tone: PRESSURE_TONES[0], level: 0, beat: 0, crisis: false },
  };
}

function clamp01(value: number): number {
  return value < 0 ? 0 : value > 1 ? 1 : value;
}

function lerp(from: number, to: number, amount: number): number {
  return from + (to - from) * amount;
}

/** How much smaller, or larger, a plane that many layers away is drawn. */
export function depthScale(depth: number): number {
  return depth >= 0 ? 1 / (1 + DEPTH_SCALE * depth) : 1 + NEAR_SCALE * -depth;
}

/** How much of a plane that many layers away comes through the haze. */
export function depthAlpha(depth: number): number {
  // A floor, so a plane far off still reads as a plane rather than as nothing.
  return depth >= 0
    ? Math.max(0.34, 1 / (1 + 0.85 * depth))
    : Math.max(0.4, 1 / (1 + 0.6 * -depth));
}

/** A tone as it reads through that many layers of haze. */
function depthTone(tone: Tone, depth: number): Tone {
  return mixTone(tone, HAZE_TINT, Math.min(0.62, 0.24 * Math.abs(depth)));
}

/**
 * Fills the scene from one pair of snapshots.
 *
 * `previous` and `next` are the two most recent snapshots and `alpha` the
 * fraction of a step between them, exactly as the timing contract sets it, so
 * the surface lags the simulation by at most one step.
 */
export function projectScene(
  scene: Scene,
  previous: FrameState,
  next: FrameState,
  alpha: number,
  viewport: Viewport,
  profile: MotionProfile,
  ephemera: Ephemera,
  candidates: readonly CandidateOutline[] = [],
  playback: PlaybackReading | null = null,
  playbackClock = 0,
): Scene {
  for (const pool of [
    scene.hazes,
    scene.currents,
    scene.routes,
    scene.boundaries,
    scene.candidates,
    scene.ports,
    scene.trails,
    scene.forms,
    scene.particles,
    scene.cues,
    scene.handles,
    scene.playback,
  ]) {
    pool.reset();
  }

  const held = clamp01(alpha);
  const reduced = profile.reducedMotion || next.header.reducedMotion;
  scene.width = viewport.width;
  scene.height = viewport.height;
  scene.dpr = viewport.dpr;
  scene.clock = next.header.step + held;
  scene.timeScale = next.header.timeScale / 65535;
  scene.stillness = 1 - scene.timeScale;
  scene.still.paused = next.header.stillVisible;
  scene.still.presence = ephemera.easeStill(stillTarget(next), reduced);
  scene.reducedMotion = reduced;
  scene.backdrop = BACKDROP;

  const cameraLayer = next.camera?.targetLayer ?? next.header.cameraLayer;
  const steered = next.forms.find((form) => form.controlled);
  const middle = steered
    ? interpolated(previous, next, steered.id, held)
    : { x: FIELD_UNITS / 2, y: FIELD_UNITS / 2 };
  const wanted = next.camera ?? { x: middle.x, y: middle.y, zoom: 1 };
  // The camera follows the controlled Form, and on the frame a Handoff moved
  // control it snaps to the new one rather than easing across the Field: the
  // authoritative target moved at once, and the two Forms may stand anywhere.
  ephemera.easeCamera(wanted.x, wanted.y, cameraLayer, reduced, steered?.id ?? null);

  const shorter = Math.min(viewport.width, viewport.height);
  const zoom = ((shorter || 1) / VIEW_UNITS) * (wanted.zoom > 0 ? wanted.zoom : 1);
  scene.camera.x = ephemera.cameraX;
  scene.camera.y = ephemera.cameraY;
  scene.camera.zoom = zoom;
  scene.camera.layer = cameraLayer;

  const middleX = viewport.width / 2;
  const middleY = viewport.height / 2;
  const place = (x: number, y: number, depth: number): { x: number; y: number } => {
    const spread = zoom * depthScale(depth);
    return {
      x: middleX + (x - scene.camera.x) * spread,
      y: middleY + (y - scene.camera.y) * spread,
    };
  };

  fillHazes(scene, next, cameraLayer);
  fillCurrents(scene, next, place, zoom, cameraLayer);
  fillRoutes(scene, previous, next, place, zoom, held, cameraLayer);
  fillPorts(scene, previous, next, ephemera, place, zoom, held, cameraLayer);
  fillBoundary(scene, next, place, zoom, cameraLayer);
  fillCandidates(scene, next, place, zoom, cameraLayer, candidates);
  fillTrails(scene, ephemera, place, zoom, profile, reduced, cameraLayer);
  fillForms(scene, previous, next, place, zoom, held, cameraLayer);
  fillParticles(scene, reduced);
  fillCues(scene, next, ephemera, place, zoom);
  fillHandles(scene, zoom);
  fillPlayback(scene, playback, playbackClock);
  fillForecast(scene, next);
  fillRim(scene, next);
  return scene;
}

/**
 * The playback reading's member marks, filled over the Port marks the frame
 * already placed.
 *
 * The played window step is `floor(clock) mod count` — one window step per
 * 1/30 s, looping while the reading is held — and under reduced motion it is
 * the last window step, held: a still comparison of the played series against
 * its base, reduced rather than removed. `factor` and `base` are the step's
 * values read against the reading's own [smallest, largest] scale over
 * `series ∪ base`, so a flat series reads as a steady mid swell and the two
 * rings of a delayed replay are comparable by construction. Only members are
 * modulated: a Node outside the reading's inside takes no mark at all.
 */
function fillPlayback(scene: Scene, playback: PlaybackReading | null, clock: number): void {
  if (!playback || playback.series.length === 0) return;
  const count = playback.series.length;
  const step = scene.reducedMotion ? count - 1 : Math.floor(clock) % count;
  let lo = Infinity;
  let hi = -Infinity;
  for (const value of playback.series) {
    lo = Math.min(lo, value);
    hi = Math.max(hi, value);
  }
  for (const value of playback.base ?? []) {
    lo = Math.min(lo, value);
    hi = Math.max(hi, value);
  }
  const factorOf = (value: number): number => (hi > lo ? (value - lo) / (hi - lo) : 0.5);
  const factor = factorOf(playback.series[step]);
  const base =
    playback.base && step < playback.base.length ? factorOf(playback.base[step]) : null;
  // The presence gate is the still surface's own: the reading is offered only
  // while the run is still or in a ramp, and the marks come up with the
  // surface exactly as the handles do.
  const alpha = 0.85 * scene.still.presence;
  if (alpha <= 0.001) return;
  for (let place = 0; place < scene.ports.count; place += 1) {
    const port = scene.ports.items[place];
    if (!contains(playback.members, port.node)) continue;
    const mark = scene.playback.next();
    mark.node = port.node;
    mark.x = port.x;
    mark.y = port.y;
    mark.radius = port.radius;
    mark.factor = factor;
    mark.base = base;
    mark.tone = FORECAST;
    mark.alpha = alpha * port.alpha;
  }
}

/** Whether an ascending identifier list holds one, by binary search. */
function contains(list: readonly number[], node: number): boolean {
  let low = 0;
  let high = list.length - 1;
  while (low <= high) {
    const middle = (low + high) >> 1;
    if (list[middle] === node) return true;
    if (list[middle] < node) low = middle + 1;
    else high = middle - 1;
  }
  return false;
}

/**
 * Where the Still Mode surface is wanted, from the mode alone.
 *
 * The mode says which way the surface is going and the time scale says how far
 * along it is, so the two together read the ramps without the renderer holding
 * a clock: the entry ramp brings the surface up as the scale falls, the pause
 * holds it up, the exit ramp takes it down as the scale rises, and every other
 * mode has no surface at all. A suspended run is one of those — a window that
 * went away is not an inspection — which is why this reads the mode rather
 * than the settling the falling scale already carries.
 */
function stillTarget(next: FrameState): number {
  switch (next.header.mode) {
    case 'still':
      return 1;
    // The two ramps read the same way, and that is the point rather than a
    // coincidence: the surface stands as far up as the Field stands still,
    // whichever direction the scale is moving in.
    case 'ramp_in':
    case 'ramp_out':
      return clamp01(1 - next.header.timeScale / 65535);
    default:
      return 0;
  }
}

/** Where a Form stands between the two snapshots. */
function interpolated(
  previous: FrameState,
  next: FrameState,
  id: number,
  alpha: number,
): { x: number; y: number } {
  const to = next.forms.find((form) => form.id === id);
  if (!to) return { x: FIELD_UNITS / 2, y: FIELD_UNITS / 2 };
  const from = previous.forms.find((form) => form.id === id);
  if (!from) return { x: to.x, y: to.y };
  return { x: lerp(from.x, to.x, alpha), y: lerp(from.y, to.y, alpha) };
}

/**
 * One plane of haze per layer the snapshot names, so a Field standing on more
 * than one layer reads as depth rather than as a flat drawing. The camera's own
 * layer needs none: it is the plane in focus.
 *
 * These marks do not interleave with the entities: the engines composite the
 * planes behind the Field in one pass and the planes in front of it in another.
 * What actually carries the depth reading is per-mark — scale, alpha, and how
 * far a tone is mixed toward the haze — and the planes are the atmosphere those
 * marks are read through.
 */
function fillHazes(scene: Scene, next: FrameState, cameraLayer: number): void {
  const layers = new Set<number>();
  for (const form of next.forms) layers.add(form.layer);
  for (const current of next.currents) layers.add(current.layer);
  // Standing on a layer other than the first is itself depth: the layers above
  // are there whether or not anything the snapshot carries stands on them.
  for (let above = 0; above < cameraLayer; above += 1) layers.add(above);

  for (const layer of [...layers].sort((first, second) => second - first)) {
    const depth = layer - cameraLayer;
    if (depth === 0) continue;
    const mark = scene.hazes.next();
    mark.depth = depth;
    // A plane below reads as depth the plane above cannot: the haze between the
    // camera and it is what says the Field goes on down there.
    mark.alpha = clamp01(depth > 0 ? 0.18 + 0.19 * depth : 0.09 + 0.1 * -depth);
    mark.tone = HAZE;
  }
}

/**
 * One mark per current the snapshot carries, with the path it runs along.
 *
 * The record names its own points in the frame's flat array and the decoder
 * hands them over in units, so the renderer strokes the shape the encoder
 * wrote — decimated to the locked cap on the way in, endpoints kept — on the
 * plane the current stands on.
 */
function fillCurrents(
  scene: Scene,
  next: FrameState,
  place: Place,
  zoom: number,
  cameraLayer: number,
): void {
  for (const current of next.currents) {
    const depth = current.layer - cameraLayer;
    const mark = scene.currents.next();
    mark.id = current.id;
    mark.layer = current.layer;
    mark.bright = current.bright;
    mark.active = current.active;
    mark.strength = current.strength / 65535;
    // The phase counter has no period in the frame, so it is read as a plain
    // turn: what it drives is a look, and nothing reads it back.
    mark.phase = (current.phase % 1024) / 1024;
    mark.depth = depth;
    mark.alpha = current.active ? depthAlpha(depth) : depthAlpha(depth) * 0.3;
    mark.tone = depthTone(current.bright ? CURRENT_BRIGHT : CURRENT_PLAIN, depth);
    // The band a Node has to stand within to receive, drawn at the width the
    // rule uses, so what a viewer sees is what a Node has to reach.
    mark.width = Math.max(2, zoom * depthScale(depth) * Math.max(8, current.width) * 2);
    mark.points.length = 0;
    for (const point of current.path) {
      const screen = place(point.x, point.y, depth);
      mark.points.push(screen.x, screen.y);
    }
  }
}

type Place = (x: number, y: number, depth: number) => { x: number; y: number };

/**
 * Every Port's place and plane, by Node id, so a Route can find its two ends.
 * One `Map` per snapshot pair per frame: two allocations, bounded by the Node
 * cap, and short-lived by construction.
 */
function placesByNode(state: FrameState): Map<number, { x: number; y: number; layer: number }> {
  const found = new Map<number, { x: number; y: number; layer: number }>();
  for (const port of state.ports) {
    const at = portUnits(port);
    found.set(port.node, { x: at.x, y: at.y, layer: port.layer });
  }
  return found;
}

function fillRoutes(
  scene: Scene,
  previous: FrameState,
  next: FrameState,
  place: Place,
  zoom: number,
  alpha: number,
  cameraLayer: number,
): void {
  const now = placesByNode(next);
  const before = placesByNode(previous);
  const at = (node: number): { x: number; y: number; layer: number } | null => {
    const to = now.get(node);
    if (!to) return null;
    const from = before.get(node);
    if (!from) return to;
    return { x: lerp(from.x, to.x, alpha), y: lerp(from.y, to.y, alpha), layer: to.layer };
  };

  for (const route of next.routes) {
    const tail = at(route.tail);
    const head = at(route.head);
    if (!tail || !head) continue;
    // Each end is drawn where its own Node is drawn, so a Route that crosses
    // planes joins the two marks rather than floating between them. The mark
    // reads at the shallower of the two, which is the plane it is nearest.
    const tailDepth = tail.layer - cameraLayer;
    const headDepth = head.layer - cameraLayer;
    const depth = Math.min(tailDepth, headDepth);
    const from = place(tail.x, tail.y, tailDepth);
    const to = place(head.x, head.y, headDepth);
    const flow = route.flow / 255;
    const mark = scene.routes.next();
    mark.route = route.route;
    mark.x1 = from.x;
    mark.y1 = from.y;
    mark.x2 = to.x;
    mark.y2 = to.y;
    mark.flow = flow;
    mark.status = route.status;
    mark.preview = QUEUED_STATUS.includes(route.status);
    mark.depth = depth;
    mark.width = Math.max(1, zoom * depthScale(depth) * (2.6 + 5.4 * flow));
    const fade = depthAlpha(depth);
    if (route.status === 2) {
      mark.tone = depthTone(OVERLOAD, depth);
      mark.alpha = 0.92 * fade;
    } else if (route.status === 1) {
      mark.tone = depthTone(ROUTE_CUT_QUEUED, depth);
      mark.alpha = 0.55 * fade;
    } else if (route.status === 4) {
      // A link the queue proposes: no Route stands here, so the mark carries
      // no flow reading and is drawn at the width a Route carrying nothing is.
      mark.tone = depthTone(ROUTE_PROPOSED, depth);
      mark.alpha = 0.8 * fade;
      mark.width = Math.max(1, zoom * depthScale(depth) * 2.6);
    } else if (route.status === 3) {
      // A standing Route an entry would move an end of: it is still carrying,
      // and the preview beside it is where it would go.
      mark.tone = depthTone(ROUTE_MOVE_QUEUED, depth);
      mark.alpha = 0.7 * fade;
    } else {
      mark.tone = depthTone(mixTone(ROUTE_BASE, ROUTE_FLOW, clamp01(flow * 1.4)), depth);
      mark.alpha = (0.34 + 0.42 * clamp01(flow * 1.6)) * fade;
    }
  }
}

/**
 * The Charge reading. Charge crosses as a fraction of the locked cap, and a
 * Field early in a run sits far below it, so the marks are scaled against the
 * most any Node on the surface holds: what a viewer reads is where Charge has
 * gathered, which is a comparison rather than an absolute.
 */
function fillPorts(
  scene: Scene,
  previous: FrameState,
  next: FrameState,
  ephemera: Ephemera,
  place: Place,
  zoom: number,
  alpha: number,
  cameraLayer: number,
): void {
  let widest = 0;
  for (const port of next.ports) widest = Math.max(widest, port.charge);
  const before = new Map<number, FramePort>();
  for (const port of previous.ports) before.set(port.node, port);

  for (const port of next.ports) {
    const to = portUnits(port);
    const from = before.get(port.node);
    const x = from ? lerp(from.x / 16, to.x, alpha) : to.x;
    const y = from ? lerp(from.y / 16, to.y, alpha) : to.y;
    // The record names the plane its Node stands on, Form-kind Nodes included:
    // a Form's own Node carries its Form's layer, so the two never come apart.
    const depth = port.layer - cameraLayer;
    const screen = place(x, y, depth);
    const share = widest > 0 ? Math.sqrt(port.charge / widest) : 0;
    const charge = clamp01(0.12 + 0.88 * share);
    const mark = scene.ports.next();
    mark.node = port.node;
    mark.x = screen.x;
    mark.y = screen.y;
    mark.kind = port.kind;
    mark.charge = charge;
    mark.overloaded = port.overloaded;
    mark.member = port.member;
    mark.shell = port.shell;
    mark.proposedMember = port.proposedMember;
    mark.reserve = port.reserve / 65535;
    mark.delivered = ephemera.deliveryStrength(port.node, scene.clock);
    mark.depth = depth;
    const scaled = depthScale(depth) * (port.kind === FORM_NODE_KIND ? 0.62 : 1);
    mark.radius = zoom * PORT_UNITS * (0.42 + 0.58 * charge) * scaled;
    mark.bloom = mark.radius * (1.6 + 3.4 * charge);
    // Ports are made prominent while the Still Mode surface is up. A closed
    // Port is drawn faintly during play, because what it is doing is nothing;
    // in Still Mode it is a place a Route can end and a Pulse can open, so it
    // is lifted toward the legibility an open one already has.
    const legible = port.open ? 1 : 0.42 + 0.48 * scene.still.presence;
    mark.alpha = legible * depthAlpha(depth);
    const warm = mixTone(CHARGE_LOW, CHARGE_HIGH, charge);
    mark.tone = depthTone(
      port.overloaded ? OVERLOAD : mixTone(PORT_BASE, warm, clamp01(0.2 + 0.8 * charge)),
      depth,
    );
  }
}

/** The closed Node-kind set's ordinal for a Form's own Node. */
const FORM_NODE_KIND = 3;

/**
 * The standing View's boundary: the closed shape the members of the inside sit
 * within, one per plane the inside reaches. The hull is the smallest convex
 * shape holding every member of that plane, which is what makes "inside" read
 * as a place rather than as a list.
 *
 * An inside can be degenerate — one member, two members, or three in a line —
 * and the hull of any of those is a point or a segment. It is drawn all the
 * same, as a capsule of the boundary's own width, so the reading never
 * disappears just because the shape has no area. Both engines take the same
 * marks and treat one and two points identically.
 */
function fillBoundary(
  scene: Scene,
  next: FrameState,
  place: Place,
  zoom: number,
  cameraLayer: number,
): void {
  hullsOf(scene, next, place, zoom, cameraLayer, false);
  // The boundary a queued change proposes, drawn beside the standing one and in
  // the queue's own register. A frame whose queue proposes no View raises the
  // flag on no Port at all, so nothing is drawn and nothing is walked.
  if (next.ports.some((port) => port.proposedMember)) {
    hullsOf(scene, next, place, zoom, cameraLayer, true);
  }
}

/**
 * One closed hull per plane, over the members of either the standing inside or
 * the one a queued change proposes.
 */
function hullsOf(
  scene: Scene,
  next: FrameState,
  place: Place,
  zoom: number,
  cameraLayer: number,
  proposed: boolean,
): void {
  hullsInto(
    scene.boundaries,
    next,
    place,
    zoom,
    cameraLayer,
    (port) => (proposed ? port.proposedMember : port.member),
    {
      // One tone for the boundary itself, whatever stands on its shell: the
      // shell members carry their own mark, and a boundary that changed colour
      // with them would read as a different boundary.
      tone: proposed ? BOUNDARY_PROPOSED : BOUNDARY,
      weight: proposed ? 0.8 : 0.92,
      proposed,
      candidate: 0,
      focused: false,
      tier: 0,
    },
  );
}

/**
 * One outline per candidate of the standing slate, per plane.
 *
 * The members come from the slate the shell holds rather than from the
 * snapshot — a slate record crosses on demand and never per frame — and where
 * each of those Nodes stands comes from the snapshot, which is the one place
 * positions live. A candidate naming a Node this frame does not carry is drawn
 * without it rather than not drawn: the outline is a reading of the Field in
 * front of the player.
 */
function fillCandidates(
  scene: Scene,
  next: FrameState,
  place: Place,
  zoom: number,
  cameraLayer: number,
  candidates: readonly CandidateOutline[],
): void {
  for (const candidate of candidates) {
    const members = new Set(candidate.members);
    if (members.size === 0) continue;
    hullsInto(scene.candidates, next, place, zoom, cameraLayer, (port) => members.has(port.node), {
      tone: candidate.focused ? CANDIDATE_FOCUSED : CANDIDATE,
      weight: candidate.focused ? 0.85 : 0.55,
      proposed: false,
      candidate: candidate.position,
      focused: candidate.focused,
      tier: candidate.tier,
    });
  }
}

/** How a filled hull reads: its tone, its weight, and what kind of mark it is. */
interface HullStyle {
  tone: Tone;
  weight: number;
  proposed: boolean;
  candidate: number;
  focused: boolean;
  tier: number;
}

/** The one hull pass both the standing boundary and a candidate outline take. */
function hullsInto(
  pool: Pool<BoundaryMark>,
  next: FrameState,
  place: Place,
  zoom: number,
  cameraLayer: number,
  keep: (port: FramePort) => boolean,
  style: HullStyle,
): void {
  // Each member carries the Node it stands for through the hull, so a handle on
  // a vertex names the member it is drawn around rather than a bare position.
  const planes = new Map<number, { x: number; y: number; node: number }[]>();
  for (const port of next.ports) {
    if (!keep(port)) continue;
    const at = { ...portUnits(port), node: port.node };
    const held = planes.get(port.layer);
    if (held) held.push(at);
    else planes.set(port.layer, [at]);
  }

  for (const [layer, members] of [...planes].sort((first, second) => second[0] - first[0])) {
    const depth = layer - cameraLayer;
    // Two points are their own hull, and a hull of three in a line comes back
    // as two: the monotone chain drops a point that turns through nothing.
    const hull = members.length <= 2 ? members : convexHull(members);
    const mark = pool.next();
    mark.points.length = 0;
    mark.nodes.length = 0;
    for (const point of hull) {
      const screen = place(point.x, point.y, depth);
      mark.points.push(screen.x, screen.y);
      mark.nodes.push(point.node);
    }
    mark.tone = style.tone;
    mark.alpha = style.weight * depthAlpha(depth);
    mark.width = Math.max(2, zoom * depthScale(depth) * BOUNDARY_UNITS);
    mark.proposed = style.proposed;
    mark.candidate = style.candidate;
    mark.tier = style.tier;
    mark.focused = style.focused;
  }
}

/** Andrew's monotone chain, counter-clockwise, first point repeated nowhere. */
function convexHull<T extends { x: number; y: number }>(points: T[]): T[] {
  const sorted = [...points].sort((first, second) =>
    first.x === second.x ? first.y - second.y : first.x - second.x,
  );
  const cross = (
    origin: { x: number; y: number },
    first: { x: number; y: number },
    second: { x: number; y: number },
  ): number =>
    (first.x - origin.x) * (second.y - origin.y) - (first.y - origin.y) * (second.x - origin.x);

  const lower: T[] = [];
  for (const point of sorted) {
    while (lower.length >= 2 && cross(lower[lower.length - 2], lower[lower.length - 1], point) <= 0) {
      lower.pop();
    }
    lower.push(point);
  }
  const upper: T[] = [];
  for (let index = sorted.length - 1; index >= 0; index -= 1) {
    const point = sorted[index];
    while (upper.length >= 2 && cross(upper[upper.length - 2], upper[upper.length - 1], point) <= 0) {
      upper.pop();
    }
    upper.push(point);
  }
  lower.pop();
  upper.pop();
  return lower.concat(upper);
}

function fillTrails(
  scene: Scene,
  ephemera: Ephemera,
  place: Place,
  zoom: number,
  profile: MotionProfile,
  reduced: boolean,
  cameraLayer: number,
): void {
  const intensity = clamp01(profile.trailIntensity / 65536);
  if (intensity <= 0) return;
  const kept = Math.round((reduced ? 0.25 : 1) * intensity * 120);
  if (kept < 4) return;

  for (const trail of ephemera.trails.values()) {
    if (trail.count < 4) continue;
    const depth = trail.layer - cameraLayer;
    const mark = scene.trails.next();
    mark.points.length = 0;
    const total = Math.min(trail.count, kept);
    const capacity = trail.points.length / 2;
    for (let back = total - 1; back >= 0; back -= 1) {
      const slot = (trail.head - 1 - back + capacity * 2) % capacity;
      const screen = place(trail.points[slot * 2], trail.points[slot * 2 + 1], depth);
      mark.points.push(screen.x, screen.y);
    }
    mark.tone = depthTone(trail.controlled ? FORM_CONTROLLED_RING : FORM_PLAIN, depth);
    mark.alpha = (trail.controlled ? 0.85 : 0.38) * depthAlpha(depth) * intensity;
    mark.width = Math.max(1, zoom * (trail.controlled ? 10 : 5));
  }
}

function fillForms(
  scene: Scene,
  previous: FrameState,
  next: FrameState,
  place: Place,
  zoom: number,
  alpha: number,
  cameraLayer: number,
): void {
  for (const form of next.forms) {
    const depth = form.layer - cameraLayer;
    const at = interpolated(previous, next, form.id, alpha);
    const screen = place(at.x, at.y, depth);
    const charge = form.charge / 65535;
    const mark = scene.forms.next();
    mark.x = screen.x;
    mark.y = screen.y;
    mark.controlled = form.controlled;
    mark.focus = form.focus;
    mark.pulseCharging = form.pulseCharging;
    mark.charge = charge;
    mark.sides = FORM_SIDES[form.formOrdinal % FORM_SIDES.length];
    mark.heading = Math.atan2(form.vy, form.vx);
    mark.depth = depth;
    mark.alpha = depthAlpha(depth);
    // The controlled Form is the largest and the brightest mark on the surface,
    // by a margin no other mark closes: which one the player steers has to be
    // answerable at a glance and without a label.
    const extent = FORM_UNITS * (form.controlled ? 1.95 : 0.95) * (0.82 + 0.35 * charge);
    mark.radius = zoom * extent * depthScale(depth);
    mark.tone = depthTone(form.controlled ? FORM_CONTROLLED : FORM_PLAIN, depth);
    mark.ringTone = depthTone(
      form.controlled ? FORM_CONTROLLED_RING : mixTone(FORM_PLAIN, CHARGE_CORE, 0.3 * charge),
      depth,
    );
    mark.bodyTone = mixTone(0x0d1014, HAZE_TINT, Math.min(0.55, 0.2 * Math.abs(depth)));
  }

  // Which Form the player steers has to be answerable at a glance, and the
  // per-plane scaling can undo that on its own: a Form several layers nearer
  // than the camera is drawn larger, and enough layers of that outgrow even the
  // controlled Form's own margin. So the controlled Form is floored above every
  // other mark rather than merely started above them — in size and in how much
  // of it comes through the haze.
  let steered: FormMark | null = null;
  let widest = 0;
  let brightest = 0;
  for (let place = 0; place < scene.forms.count; place += 1) {
    const mark = scene.forms.items[place];
    if (mark.controlled) {
      steered = mark;
      continue;
    }
    widest = Math.max(widest, mark.radius);
    brightest = Math.max(brightest, mark.alpha);
  }
  if (steered) {
    steered.radius = Math.max(steered.radius, widest * CONTROLLED_MARGIN);
    steered.alpha = Math.max(steered.alpha, Math.min(1, brightest * CONTROLLED_MARGIN));
  }
}

/**
 * Soft particles, drifting along the currents and the Routes that carry flow.
 * Their positions are a function of the clock rather than of a store, so they
 * hold nothing between frames and stop dead when the time scale does.
 */
function fillParticles(scene: Scene, reduced: boolean): void {
  const perSegment = reduced ? 2 : PARTICLES_PER_SEGMENT;
  const drift = reduced ? 0 : scene.clock * 0.07;

  for (let place = 0; place < scene.currents.count; place += 1) {
    const current = scene.currents.items[place];
    if (!current.active) continue;
    const pairs = current.points.length / 2;
    for (let leg = 1; leg < pairs; leg += 1) {
      for (let index = 0; index < perSegment; index += 1) {
        if (scene.particles.count >= PARTICLE_CAP) return;
        const along = (index / perSegment + drift) % 1;
        const mark = scene.particles.next();
        mark.x = lerp(current.points[(leg - 1) * 2], current.points[leg * 2], along);
        mark.y = lerp(current.points[(leg - 1) * 2 + 1], current.points[leg * 2 + 1], along);
        mark.radius = Math.max(1, current.width * 0.09);
        mark.tone = current.tone;
        mark.alpha = current.alpha * (current.bright ? 0.85 : 0.5) * Math.sin(Math.PI * along) ** 0.5;
      }
    }
  }

  for (let place = 0; place < scene.routes.count; place += 1) {
    const route = scene.routes.items[place];
    if (route.flow <= 0) continue;
    const count = Math.max(1, Math.round(perSegment * route.flow));
    for (let index = 0; index < count; index += 1) {
      if (scene.particles.count >= PARTICLE_CAP) return;
      const along = (index / count + drift * (0.4 + route.flow)) % 1;
      const mark = scene.particles.next();
      mark.x = lerp(route.x1, route.x2, along);
      mark.y = lerp(route.y1, route.y2, along);
      mark.radius = Math.max(1, route.width * 0.62);
      mark.tone = route.status === 2 ? OVERLOAD : CHARGE_HIGH;
      mark.alpha = 0.5 + 0.45 * route.flow;
    }
  }
}

/** The cue kinds that report something wrong rather than something done. */
const HARD_CUES = new Set([5, 6, 8]);

/** The four cue kinds a Pulse raises, in the closed set's own numbering. */
const CUE_PULSE_EMITTED = 1;
const CUE_CHARGE_GATHERED = 2;
const CUE_PORT_OPENED = 3;
const CUE_INTERFERENCE_PUSHED = 12;

/**
 * How far a cue's own ring reaches when the frame carried no radius for it, in
 * units.
 *
 * A Pulse carries its reach: the `FrameState` Form record holds it in Q8.8
 * units, and the cue captured it at the step it was raised, so the ring expands
 * to what the emission actually reached. This stands in only where a frame
 * carried none — a cue raised at a Node the controlled Form was not standing
 * at, or a snapshot written before the reach was filled — so that a reading is
 * never lost for want of a number.
 */
const PULSE_EXTENT_UNITS = 96;

/**
 * How each cue of the Pulse reads: how far it reaches, which way it travels,
 * what hue carries it, and how many soft particles go with it.
 *
 * The four are told apart by motion and hue rather than by a label. A Pulse
 * opens outward in the Form's own hue; Charge gathered closes inward in the
 * hue Charge is drawn in, because what moved is Charge and it moved toward the
 * Form; a Port opening lifts the Port's own hue toward Charge and holds, which
 * is exactly what opening one does; and Interference pushed carries
 * Interference's own hue outward, hard and fast, because it is a shove.
 */
interface CueShape {
  /** How far the ring reaches at the end of its life, in units. */
  extent: number;
  /** True when the ring closes inward rather than opening outward. */
  inward: boolean;
  tone: Tone;
  /** How wide the ring is drawn, in units, before its life thins it. */
  width: number;
  /** How many soft particles travel with the ring. */
  particles: number;
  alpha: number;
}

function cueShape(kind: number, captured: number): CueShape | null {
  const reach = captured > 0 ? captured : PULSE_EXTENT_UNITS;
  switch (kind) {
    case CUE_PULSE_EMITTED:
      return {
        extent: reach,
        inward: false,
        tone: FORM_CONTROLLED_RING,
        width: 5,
        particles: 0,
        alpha: 0.9,
      };
    case CUE_CHARGE_GATHERED:
      return {
        extent: reach * 0.8,
        inward: true,
        tone: CHARGE_HIGH,
        width: 3,
        particles: 7,
        alpha: 0.8,
      };
    case CUE_PORT_OPENED:
      return {
        extent: 46,
        inward: false,
        tone: mixTone(PORT_BASE, CHARGE_HIGH, 0.75),
        width: 7,
        particles: 5,
        alpha: 0.95,
      };
    case CUE_INTERFERENCE_PUSHED:
      return {
        extent: reach * 1.15,
        inward: false,
        tone: PRESSURE_TONES[4],
        width: 6,
        particles: 6,
        alpha: 0.9,
      };
    default:
      return null;
  }
}

function fillCues(
  scene: Scene,
  next: FrameState,
  ephemera: Ephemera,
  place: Place,
  zoom: number,
): void {
  const positions = placesByNode(next);
  for (const cue of ephemera.cues) {
    const age = ephemera.cueAge(cue, scene.clock);
    if (age >= 1) continue;
    const at = positions.get(cue.b);
    const depth = at ? at.layer - scene.camera.layer : 0;
    const screen = at
      ? place(at.x, at.y, depth)
      : { x: scene.width / 2, y: scene.height / 2 };
    const shape = cueShape(cue.kind, cue.reach);
    const mark = scene.cues.next();
    mark.x = screen.x;
    mark.y = screen.y;
    mark.kind = cue.kind;

    if (!shape) {
      const hard = HARD_CUES.has(cue.kind);
      mark.radius = zoom * (hard ? 40 : 26) * (0.3 + 2.4 * age);
      mark.tone = hard ? CUE_HARD : CUE_SOFT;
      mark.alpha = (1 - age) * (hard ? 0.95 : 0.7);
      mark.width = Math.max(1, zoom * (hard ? 6 : 3) * (1 - age));
      mark.hard = hard;
      continue;
    }

    // A ring that opens travels from a point out to the reach; one that closes
    // starts at the reach and arrives. Either way it fades as it goes, so the
    // shape and the fading say the same thing twice.
    const spread = zoom * depthScale(depth);
    const along = shape.inward ? 1 - age : 0.12 + 0.88 * age;
    mark.radius = spread * shape.extent * along;
    mark.tone = depthTone(shape.tone, depth);
    mark.alpha = shape.alpha * (1 - age) * depthAlpha(depth);
    mark.width = Math.max(1, spread * shape.width * (1 - 0.6 * age));
    mark.hard = cue.kind === CUE_INTERFERENCE_PUSHED;

    // The particles ride the ring, so a gather streams inward and a shove
    // throws outward. They are motion and nothing else, so reduced motion
    // drops them and the ring alone carries the reading.
    if (scene.reducedMotion || shape.particles === 0) continue;
    for (let index = 0; index < shape.particles; index += 1) {
      if (scene.particles.count >= PARTICLE_CAP) break;
      const turn = ((index + 0.5) / shape.particles) * Math.PI * 2 + cue.b;
      const speck = scene.particles.next();
      speck.x = mark.x + Math.cos(turn) * mark.radius;
      speck.y = mark.y + Math.sin(turn) * mark.radius;
      speck.radius = Math.max(1, spread * 3.4 * (1 - 0.5 * age));
      speck.tone = shape.tone;
      speck.alpha = mark.alpha;
    }
  }
}

/** How far in from the end of a Route its own handle sits, as a fraction. */
const ROUTE_HANDLE_ALONG = 0.16;

/** How wide a handle is drawn, in units, before its own mark's size lifts it. */
const HANDLE_UNITS = 9;

/**
 * The handles Still Mode puts on the Field: one per Port, one at each end of
 * every Route, and one at every vertex of a drawn boundary.
 *
 * Every one of them is read off marks this scene has already projected, so
 * nothing is placed twice and nothing is allocated to place it. A Route's two
 * handles sit a little way in from its ends rather than exactly on them,
 * because exactly on them is where the Port's own handle already is, and two
 * handles in one place is one handle a player cannot take hold of.
 *
 * They come up with the surface and go down with it; at rest there are none.
 */
function fillHandles(scene: Scene, zoom: number): void {
  const presence = scene.still.presence;
  if (presence <= 0.001) return;
  const radius = Math.max(3, zoom * HANDLE_UNITS);

  for (let place = 0; place < scene.ports.count; place += 1) {
    const port = scene.ports.items[place];
    const mark = scene.handles.next();
    mark.x = port.x;
    mark.y = port.y;
    mark.node = port.node;
    mark.route = 0;
    mark.end = 0;
    mark.kind = HANDLE_KIND.port;
    mark.radius = Math.max(radius, port.radius * 1.5);
    mark.tone = HANDLE_PORT;
    mark.alpha = 0.8 * presence * depthAlpha(port.depth);
    mark.width = Math.max(1, radius * 0.22);
  }

  for (let place = 0; place < scene.routes.count; place += 1) {
    const route = scene.routes.items[place];
    // A proposal is not a place a change starts from: nothing stands there yet,
    // so the queue's own preview carries no handle of its own.
    if (route.status === PROPOSED_STATUS) continue;
    for (const [end, along] of [
      [0, ROUTE_HANDLE_ALONG],
      [1, 1 - ROUTE_HANDLE_ALONG],
    ] as const) {
      const mark = scene.handles.next();
      mark.x = lerp(route.x1, route.x2, along);
      mark.y = lerp(route.y1, route.y2, along);
      mark.node = 0;
      mark.route = route.route;
      mark.end = end;
      mark.kind = HANDLE_KIND.route;
      mark.radius = radius * 0.72;
      mark.tone = HANDLE_ROUTE;
      // A handle reads through the same haze the mark it belongs to does, as
      // the Port and Boundary handles already do: a Route two layers down is
      // drawn faint, and a handle standing at full strength over it would say
      // the plane it is on does not matter.
      mark.alpha = 0.85 * presence * depthAlpha(route.depth);
      mark.width = Math.max(1, radius * 0.2);
    }
  }

  for (let place = 0; place < scene.boundaries.count; place += 1) {
    const boundary = scene.boundaries.items[place];
    // The standing boundary carries the handles; the one a queued change
    // proposes is a reading rather than a place to take hold of.
    if (boundary.proposed) continue;
    // A vertex sits exactly where its member's own Port stands, so its handle
    // is pushed a little way outward from the middle of the shape — the same
    // reason a Route's handles sit in from its ends. Two handles in one place
    // is one handle a player cannot take hold of, and the two mean different
    // changes: the Port starts a connection and the vertex reshapes the View.
    let middleX = 0;
    let middleY = 0;
    const corners = boundary.points.length / 2;
    for (let point = 0; point + 1 < boundary.points.length; point += 2) {
      middleX += boundary.points[point] / corners;
      middleY += boundary.points[point + 1] / corners;
    }
    for (let point = 0; point + 1 < boundary.points.length; point += 2) {
      const mark = scene.handles.next();
      const outX = boundary.points[point] - middleX;
      const outY = boundary.points[point + 1] - middleY;
      const span = Math.sqrt(outX * outX + outY * outY);
      const push = span > 0.001 ? (radius * 1.6) / span : 0;
      mark.x = boundary.points[point] + outX * push;
      mark.y = boundary.points[point + 1] + outY * push;
      mark.node = boundary.nodes[point / 2] ?? 0;
      mark.route = 0;
      mark.end = 0;
      mark.kind = HANDLE_KIND.boundary;
      mark.radius = radius * 0.9;
      mark.tone = HANDLE_BOUNDARY;
      mark.alpha = 0.9 * presence * boundary.alpha;
      mark.width = Math.max(1, radius * 0.24);
    }
  }
}

/** How much of the surface the forecast strip spans, and how tall it stands. */
const FORECAST_SPAN = 0.46;
const FORECAST_HEIGHT = 0.1;

/** How far in from the foot of the surface the strip sits. */
const FORECAST_INSET = 0.06;

/**
 * The forecast overlay: the standing View's baseline `q_I` envelope, drawn as
 * a band inside a strip at the foot of the surface.
 *
 * The frame says whether the overlay stands at all — the section is present
 * exactly while the run is `still` — and how many window steps it carries.
 * That is none until the goal that replays baselines computes one, so what is
 * drawn today is the strip and nothing inside it. The reading is deliberately
 * not stood in for: a band drawn from anything but replayed baselines would be
 * a forward reading the framework never made.
 */
function fillForecast(scene: Scene, next: FrameState): void {
  const forecast = scene.forecast;
  forecast.low.length = 0;
  forecast.high.length = 0;
  forecast.count = 0;
  // The overlay is the still surface's own, so it stands only where the frame
  // says the run is still — not through the ramps, which the handles carry.
  forecast.alpha = next.overlay === null ? 0 : scene.still.presence;
  if (forecast.alpha <= 0.001) return;

  const width = scene.width * FORECAST_SPAN;
  const height = scene.height * FORECAST_HEIGHT;
  forecast.x = (scene.width - width) / 2;
  forecast.y = scene.height - height - scene.height * FORECAST_INSET;
  forecast.width = width;
  forecast.height = height;
  forecast.tone = FORECAST;

  const envelope = next.overlay ?? [];
  forecast.count = envelope.length;
  if (envelope.length === 0) return;

  // The band is drawn against the largest reading it carries, which is what
  // every other Charge reading on the surface is drawn against: what a viewer
  // reads is the shape of the spread rather than an absolute.
  let widest = 0;
  for (const step of envelope) widest = Math.max(widest, step.hi, step.lo);
  const span = envelope.length > 1 ? envelope.length - 1 : 1;
  for (let place = 0; place < envelope.length; place += 1) {
    const x = forecast.x + (place / span) * width;
    const scale = (value: number): number =>
      forecast.y + height - (widest > 0 ? clamp01(value / widest) : 0) * height;
    forecast.low.push(x, scale(envelope[place].lo));
    forecast.high.push(x, scale(envelope[place].hi));
  }
}

/**
 * The pressure reading: how far in from the edge of the surface it reaches,
 * and how hard it beats. A pressure at its crisis stage is the one reading
 * that beats, which is what makes something being wrong legible without a word
 * for it.
 */
function fillRim(scene: Scene, next: FrameState): void {
  let level = 0;
  let tone = PRESSURE_TONES[0];
  let crisis = false;
  for (const pressure of next.pressures) {
    if (pressure.queued) continue;
    const stage = pressure.stage === 'crisis' ? 1 : pressure.stage === 'pressure' ? 0.6 : 0.3;
    const reading = clamp01((pressure.level / 65535) * stage + (pressure.stage === 'crisis' ? 0.25 : 0));
    if (reading <= level) continue;
    level = reading;
    tone = PRESSURE_TONES[pressure.ordinal % PRESSURE_TONES.length];
    crisis = pressure.stage === 'crisis';
  }
  scene.rim.level = level;
  scene.rim.tone = tone;
  scene.rim.crisis = crisis;
  scene.rim.beat = crisis && !scene.reducedMotion ? 0.5 + 0.5 * Math.sin(scene.clock * 0.55) : 0;
}

/** How wide a Form's own ring is drawn, in device pixels. */
export function formRingWidth(mark: FormMark): number {
  return Math.max(1.8, mark.radius * 0.16);
}
