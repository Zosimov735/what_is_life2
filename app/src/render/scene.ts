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
  ASSEMBLY_DRAFT,
  ASSEMBLY_DRAFT_CURRENT,
  ASSEMBLY_DRAFT_MATERIAL,
  ASSEMBLY_DRAFT_ORIGIN,
  BACKDROP,
  CANDIDATE,
  CANDIDATE_FOCUSED,
  CHARGE_CORE,
  CHARGE_HIGH,
  CHARGE_LOW,
  CHARGE_SPARK,
  CUE_HARD,
  CUE_SOFT,
  CURRENT_BRIGHT,
  CURRENT_GLASS,
  CURRENT_PLAIN,
  FORECAST,
  FORM_CONTROLLED,
  FORM_CONTROLLED_RING,
  FORM_FACET,
  FORM_FACET_SHADOW,
  FORM_PLAIN,
  HANDLE_BOUNDARY,
  HANDLE_PORT,
  HANDLE_ROUTE,
  HAZE,
  HAZE_TINT,
  INTERVENTION_BREACH,
  INTERVENTION_CLAMP,
  INTERVENTION_DECOY,
  INTERVENTION_DELAY,
  INTERVENTION_SCRAMBLE,
  MATERIAL_CONDUCTOR,
  MATERIAL_JUNCTION,
  MATERIAL_BOUNDARY,
  mixTone,
  OVERLOAD,
  OBSERVATION_VIEW,
  PHYSICAL_COMPARTMENT,
  PHYSICAL_COMPARTMENT_PROPOSED,
  POLICY_ACTIVE,
  POLICY_BLOCKED,
  POLICY_IDLE,
  POLICY_TARGET,
  POLICY_PREVIEW,
  POLICY_WAIT,
  PORT_BASE,
  PRESSURE_TONES,
  ROUTE_BASE,
  ROUTE_AUTOMATION,
  ROUTE_AUTOMATION_DISABLED,
  ROUTE_CUT_QUEUED,
  ROUTE_FLOW,
  ROUTE_MOVE_QUEUED,
  ROUTE_PROPOSED,
  type Tone,
} from './palette';
import { renderQuality, type RenderQualityProfile } from './quality';
import { Ephemera, FIELD_UNITS, portUnits } from './history';
import type {
  FramePolicyRuntime,
  FramePort,
  FramePressure,
  FrameRouteTransferOutcome,
  FrameState,
} from '../../../worker/src/frame-state';
import type {
  EngineeringAssemblyDiffChange,
  EngineeringAssemblyCompatibilityDisposition,
  EngineeringAssemblyDraft,
  EngineeringAssemblyPreview,
  EngineeringRunTransitionPreview,
  EngineeringRunTransitionReceipt,
  EngineeringTransitionKind,
  PolicyPreview,
  PolicyTargetKind,
} from '../../../worker/src/protocol';

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

/** The physical compartment reads as material; the View as a thin aperture. */
const COMPARTMENT_EDGE_UNITS = 10;
const VIEW_EDGE_UNITS = 2;
const CANDIDATE_EDGE_UNITS = 2;

/** Field bits carried by one accepted assembly candidate mark. */
export const ASSEMBLY_DRAFT_FIELD = {
  position: 1 << 0,
  layer: 1 << 1,
  charge: 1 << 2,
  interface: 1 << 3,
  reserve: 1 << 4,
  blanks: 1 << 5,
  amount: 1 << 6,
  phase: 1 << 7,
  active: 1 << 8,
  members: 1 << 9,
  leakage: 1 << 10,
} as const;

/** How far one layer of separation pushes a plane away. */
const DEPTH_SCALE = 0.22;

/** How far one layer of separation in front pulls a plane forward. */
const NEAR_SCALE = 0.3;

/** How far above every other Form's mark the controlled one is held. */
const CONTROLLED_MARGIN = 1.4;

/** The eight chassis profiles, in the closed Form set's own order. */
const FORM_PROFILES = [
  { sides: 3, inset: 0.68, stretchX: 1.04, stretchY: 1.16, turn: -Math.PI / 2, core: 0.42 },
  { sides: 6, inset: 0.9, stretchX: 1.16, stretchY: 0.92, turn: Math.PI / 6, core: 0.5 },
  { sides: 4, inset: 0.7, stretchX: 0.84, stretchY: 1.2, turn: Math.PI / 4, core: 0.46 },
  { sides: 5, inset: 0.82, stretchX: 1.08, stretchY: 0.98, turn: -Math.PI / 2, core: 0.47 },
  { sides: 8, inset: 0.91, stretchX: 1, stretchY: 1, turn: Math.PI / 8, core: 0.56 },
  { sides: 7, inset: 0.76, stretchX: 1.12, stretchY: 0.94, turn: -Math.PI / 2, core: 0.45 },
  { sides: 3, inset: 0.48, stretchX: 0.74, stretchY: 1.34, turn: -Math.PI / 2, core: 0.36 },
  { sides: 9, inset: 0.78, stretchX: 1, stretchY: 1, turn: -Math.PI / 2, core: 0.48 },
] as const;

/** The most particles a scene carries, whatever the Field holds. */
const PARTICLE_CAP = 620;

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

/** Which Still Mode interaction register the chrome has selected. */
export type StillSurfaceTool = 'view' | 'compartment';

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

export interface MediumMark {
  active: boolean;
  dx: number;
  dy: number;
  speed: number;
  drag: number;
  collisionRadius: number;
  collisionResponse: number;
  alpha: number;
  offset: number;
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
  /** Whether this Component participates in Routes. Closed Ports stay hollow. */
  open: boolean;
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
  decoyReceiver: boolean;
  breached: boolean;
  /** 0 to 1: how lately a current delivered here. */
  delivered: number;
  /** 0 to 1: isolated Vault reserve against its finite reserve capacity. */
  reserve: number;
  /** Signed latest-frame change in isolated reserve, normalized to capacity. */
  reserveDelta: number;
  alpha: number;
  /** Layers away from the camera's own; only a Form's own Node is ever off it. */
  depth: number;
  /** A targeted pressure's local strength and register, or zero when unaffected. */
  stress: number;
  stressTone: Tone;
  stressCrisis: boolean;
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
  /** Exact latest-step demand and acceptance, normalized for visual stance. */
  requested: number;
  accepted: number;
  transferOutcome: FrameRouteTransferOutcome;
  /** 0 standing, 1 cut queued, 2 overloaded, 3 move queued, 4 proposed. */
  status: number;
  clamped: boolean;
  scrambled: boolean;
  automationEnabled: boolean;
  automationLimited: boolean;
  /** Exact endpoint gate state. A closed endpoint makes the Route dormant. */
  tailOpen: boolean;
  headOpen: boolean;
  gated: boolean;
  /**
   * Whether this mark is a queued change rather than a standing Route. Both
   * engines draw a preview broken rather than solid, so what the queue would do
   * never reads as something the Field already holds.
   */
  preview: boolean;
  /** Layers away from the camera's own: the shallower of its two ends. */
  depth: number;
  stress: number;
  stressTone: Tone;
  stressCrisis: boolean;
}

export interface FormMark {
  form: number;
  formOrdinal: number;
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
  /** Device-pixel pairs for the silhouette and inset chassis core. */
  outline: number[];
  core: number[];
  facetTone: Tone;
  facetShadow: Tone;
  /** The direction the Form is travelling, in radians. */
  heading: number;
  alpha: number;
  depth: number;
  stress: number;
  stressTone: Tone;
  stressCrisis: boolean;
}

/** One exact retained local-policy decision projected onto its physical owner. */
export interface PolicyMark {
  address: number;
  action: FramePolicyRuntime['action'];
  outcome: FramePolicyRuntime['outcome'];
  targetKind: FramePolicyRuntime['targetKind'];
  rule: number;
  x: number;
  y: number;
  radius: number;
  /** Selected action radius in device pixels; zero for non-range actions. */
  reach: number;
  /** Draft condition sensor radius in device pixels; zero for live runtime marks. */
  sensorReach: number;
  hasTarget: boolean;
  targetX: number;
  targetY: number;
  targetRadius: number;
  tone: Tone;
  targetTone: Tone;
  alpha: number;
  phase: number;
  /** Draft projection is visually distinct and never retained as runtime state. */
  preview: boolean;
}

export interface PolicyCandidateMark {
  id: number;
  kind: PolicyTargetKind;
  x: number;
  y: number;
  radius: number;
  selected: boolean;
  tone: Tone;
  alpha: number;
}

export type AssemblyDraftMarkKind =
  | 'component'
  | 'route'
  | 'form'
  | 'material'
  | 'current'
  | 'physical_compartment';

/**
 * One localized change in an accepted Design assembly preview.
 *
 * These marks are presentation-only. `fromX/fromY` point at the committed
 * opening, `x/y` at the normalized candidate, and no value is fed back into
 * simulation, inspection, or hit testing.
 */
export interface AssemblyDraftMark {
  kind: AssemblyDraftMarkKind;
  id: number;
  x: number;
  y: number;
  radius: number;
  fromX: number;
  fromY: number;
  fromRadius: number;
  displaced: boolean;
  fields: number;
  beforeLevel: number;
  afterLevel: number;
  count: number;
  active: boolean;
  open: boolean;
  phase: number;
  tone: Tone;
  originTone: Tone;
  alpha: number;
  depth: number;
  /** True only for a receipt-bound reconstruction companion. */
  companion: boolean;
  /** Rust-authored field stance for a transition companion, never simulation input. */
  compatibility: EngineeringAssemblyCompatibilityDisposition | null;
}

export type EngineeringTransitionCompanionStage =
  | 'none'
  | 'preview'
  | 'deenergize'
  | 'reconstruct'
  | 'settle'
  | 'locked';

/** Shell-held display input for one Rust-authored reconstruction boundary. */
export interface EngineeringTransitionCompanion {
  preview: EngineeringRunTransitionPreview;
  receipt: EngineeringRunTransitionReceipt | null;
  status: 'preview' | 'committed';
}

/** Parent geometry retained only long enough to express the accepted transition. */
export interface EngineeringTransitionOrigin {
  components: Array<{
    node: number;
    layer: number;
    x: number;
    y: number;
    charge: number;
    open: boolean;
  }>;
  currents: Array<{ id: number; active: boolean; phase: number }>;
  forms: Array<{ node: number; reserve: number }>;
  materials: Array<{
    material: number;
    layer: number;
    x: number;
    y: number;
    amount: number;
  }>;
  physicalCompartment: {
    members: number[];
    leakage: number;
  };
}

export function captureEngineeringTransitionOrigin(
  frame: FrameState | null,
): EngineeringTransitionOrigin | null {
  if (!frame) return null;
  const ports = new Map(frame.ports.map((port) => [port.node, port] as const));
  return {
    components: frame.ports.map((port) => ({
      node: port.node,
      layer: port.layer,
      x: port.x / 16,
      y: port.y / 16,
      charge: port.charge / 65_535,
      open: port.open,
    })),
    currents: frame.currents.map((current) => ({
      id: current.id,
      active: current.active,
      phase: current.phase / 65_535,
    })),
    forms: frame.forms.map((form) => ({
      node: form.id,
      reserve: (ports.get(form.id)?.reserve ?? 0) / 65_535,
    })),
    materials: frame.materials.map((material) => ({
      material: material.material,
      layer: material.layer,
      x: material.x / 16,
      y: material.y / 16,
      amount: material.amount,
    })),
    physicalCompartment: {
      members: frame.ports.filter((port) => port.member).map((port) => port.node).sort((a, b) => a - b),
      leakage: frame.header.leakPerExposedContactPerStep,
    },
  };
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
  diverted: boolean;
  delayed: boolean;
  emitting: boolean;
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
  stress: number;
  stressTone: Tone;
  stressCrisis: boolean;
}

/** A visible delivery path from a Supply Stream into one geometric recipient. */
export interface SupplyLinkMark {
  current: number;
  recipient: number;
  x1: number;
  y1: number;
  x2: number;
  y2: number;
  phase: number;
  width: number;
  tone: Tone;
  alpha: number;
}

/** Bit flags carried by one coupling target; an object may support several effects. */
export const COUPLING_EFFECT = { gather: 1, open: 2, suppress: 4 } as const;

/** One object the currently held coupling radius will actually affect. */
export interface CouplingTargetMark {
  x: number;
  y: number;
  radius: number;
  effects: number;
  phase: number;
  tone: Tone;
  alpha: number;
}

/** The true world-space coupling reach centered on the controlled Form. */
export interface CouplingMark {
  active: boolean;
  connected: boolean;
  x: number;
  y: number;
  radius: number;
  charge: number;
  phase: number;
  tone: Tone;
  alpha: number;
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
  /** Material compartment, passive View, or analysis candidate outline. */
  role: BoundaryRole;
  /** The member Node each point stands at, in the same order as the points. */
  nodes: number[];
  /**
   * Whether this is a physical compartment a queued `reshape_compartment`
   * proposes rather than the standing material edge. A passive View change
   * never sets this bit; it is drawn in its own register.
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
  /** Targeted pressure carried by this physical edge, never by a passive View. */
  stress: number;
  stressTone: Tone;
  stressCrisis: boolean;
}

/** The three hull registers, kept explicit so geometry never collapses them. */
export type BoundaryRole = 'compartment' | 'view' | 'candidate' | 'assembly-draft';

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
  /** 0 atmosphere, 1 renewal material, 2 conserving Wake cache. */
  subject: number;
  /** Material identifier for subject 1, cache ordinal for subject 2, otherwise 0. */
  id: number;
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
 * One handle: a place on the Field where a causal change can be started.
 *
 * Which places those are is the `PlanCommand` union read back as geometry, and
 * nothing more: a `connect` runs between two Ports, a `redirect` moves one end
 * of a Route, a `cut` takes a Route, and a `reshape_compartment` moves the
 * members enclosed by the standing material edge. So a Port carries a handle,
 * each Route carries one at each of its two ends, and every vertex of the
 * physical compartment carries one. Drawing them is this goal's; making them
 * draggable is the goal that owns queued changes, and the geometry is the same
 * either way.
 */
export interface HandleMark {
  x: number;
  y: number;
  /**
   * What the handle names: the Node a Port handle stands on, the member a
   * Compartment vertex is drawn around, or 0 for a Route handle, which names its
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
  /** `HANDLE_KIND`: 0 a Port, 1 a Route end, 2 a Compartment vertex. */
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
  /** Whether the dominant reading also has a local target on the Field. */
  localized: boolean;
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
  /** Effect and mark budgets selected for this frame. */
  quality: RenderQualityProfile;
  /** Persistent marks plus this tier's moving-particle allowance. */
  particleLimit: number;
  backdrop: Tone;
  medium: MediumMark;
  hazes: Pool<HazeMark>;
  currents: Pool<CurrentMark>;
  supplyLinks: Pool<SupplyLinkMark>;
  routes: Pool<RouteMark>;
  boundaries: Pool<BoundaryMark>;
  /** One outline per candidate of the standing slate, drawn under them. */
  candidates: Pool<BoundaryMark>;
  /** Candidate physical-compartment hulls from accepted assembly previews. */
  assemblyBoundaries: Pool<BoundaryMark>;
  ports: Pool<PortMark>;
  trails: Pool<TrailMark>;
  forms: Pool<FormMark>;
  policies: Pool<PolicyMark>;
  policyCandidates: Pool<PolicyCandidateMark>;
  /** Localized accepted assembly changes in the noncausal draft register. */
  assemblyDrafts: Pool<AssemblyDraftMark>;
  /** One noncausal preview or receipt-bound reconstruction sequence. */
  engineeringTransition: {
    active: boolean;
    operation: EngineeringTransitionKind | null;
    stage: EngineeringTransitionCompanionStage;
    phase: number;
    tone: Tone;
    originTone: Tone;
    authorityTone: Tone;
    commitAllowed: boolean;
  };
  coupling: CouplingMark;
  couplingTargets: Pool<CouplingTargetMark>;
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
    quality: renderQuality(),
    particleLimit: 320,
    backdrop: BACKDROP,
    medium: { active: false, dx: 1, dy: 0, speed: 0, drag: 0, collisionRadius: 0, collisionResponse: 0, alpha: 0, offset: 0, tone: CURRENT_GLASS },
    hazes: new Pool<HazeMark>(() => ({ depth: 0, alpha: 0, tone: HAZE })),
    currents: new Pool<CurrentMark>(() => ({
      id: 0,
      layer: 0,
      bright: false,
      active: false,
      diverted: false,
      delayed: false,
      emitting: false,
      strength: 0,
      phase: 0,
      points: [],
      width: 1,
      tone: CURRENT_PLAIN,
      alpha: 1,
      depth: 0,
      stress: 0,
      stressTone: PRESSURE_TONES[0],
      stressCrisis: false,
    })),
    supplyLinks: new Pool<SupplyLinkMark>(() => ({
      current: 0,
      recipient: 0,
      x1: 0,
      y1: 0,
      x2: 0,
      y2: 0,
      phase: 0,
      width: 1,
      tone: CURRENT_GLASS,
      alpha: 0,
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
      requested: 0,
      accepted: 0,
      transferOutcome: 'standing',
      status: 0,
      clamped: false,
      scrambled: false,
      automationEnabled: true,
      automationLimited: false,
      tailOpen: false,
      headOpen: false,
      gated: true,
      preview: false,
      depth: 0,
      stress: 0,
      stressTone: PRESSURE_TONES[0],
      stressCrisis: false,
    })),
    boundaries: new Pool<BoundaryMark>(() => ({
      points: [],
      nodes: [],
      tone: PHYSICAL_COMPARTMENT,
      alpha: 1,
      width: 2,
      role: 'compartment',
      proposed: false,
      candidate: 0,
      focused: false,
      tier: 0,
      stress: 0,
      stressTone: PRESSURE_TONES[0],
      stressCrisis: false,
    })),
    candidates: new Pool<BoundaryMark>(() => ({
      points: [],
      nodes: [],
      tone: CANDIDATE,
      alpha: 1,
      width: 2,
      role: 'candidate',
      proposed: false,
      candidate: 0,
      focused: false,
      tier: 0,
      stress: 0,
      stressTone: PRESSURE_TONES[0],
      stressCrisis: false,
    })),
    assemblyBoundaries: new Pool<BoundaryMark>(() => ({
      points: [],
      nodes: [],
      tone: ASSEMBLY_DRAFT,
      alpha: 0,
      width: 2,
      role: 'assembly-draft',
      proposed: true,
      candidate: 0,
      focused: false,
      tier: 0,
      stress: 0,
      stressTone: PRESSURE_TONES[0],
      stressCrisis: false,
    })),
    ports: new Pool<PortMark>(() => ({
      node: 0,
      x: 0,
      y: 0,
      radius: 1,
      kind: 0,
      open: false,
      tone: PORT_BASE,
      bloom: 0,
      charge: 0,
      overloaded: false,
      member: false,
      shell: false,
      proposedMember: false,
      decoyReceiver: false,
      breached: false,
      delivered: 0,
      reserve: 0,
      reserveDelta: 0,
      alpha: 1,
      depth: 0,
      stress: 0,
      stressTone: PRESSURE_TONES[0],
      stressCrisis: false,
    })),
    trails: new Pool<TrailMark>(() => ({ points: [], tone: FORM_PLAIN, alpha: 1, width: 1 })),
    forms: new Pool<FormMark>(() => ({
      form: 0,
      formOrdinal: 0,
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
      outline: [],
      core: [],
      facetTone: FORM_CONTROLLED,
      facetShadow: HAZE,
      heading: 0,
      alpha: 1,
      depth: 0,
      stress: 0,
      stressTone: PRESSURE_TONES[0],
      stressCrisis: false,
    })),
    policies: new Pool<PolicyMark>(() => ({
      address: 0,
      action: 'none',
      outcome: 'idle',
      targetKind: 'none',
      rule: -1,
      x: 0,
      y: 0,
      radius: 1,
      reach: 0,
      sensorReach: 0,
      hasTarget: false,
      targetX: 0,
      targetY: 0,
      targetRadius: 1,
      tone: POLICY_IDLE,
      targetTone: POLICY_TARGET,
      alpha: 0,
      phase: 0,
      preview: false,
    })),
    policyCandidates: new Pool<PolicyCandidateMark>(() => ({
      id: 0,
      kind: 'none',
      x: 0,
      y: 0,
      radius: 1,
      selected: false,
      tone: POLICY_PREVIEW,
      alpha: 0,
    })),
    assemblyDrafts: new Pool<AssemblyDraftMark>(() => ({
      kind: 'component',
      id: 0,
      x: 0,
      y: 0,
      radius: 1,
      fromX: 0,
      fromY: 0,
      fromRadius: 1,
      displaced: false,
      fields: 0,
      beforeLevel: 0,
      afterLevel: 0,
      count: 0,
      active: false,
      open: false,
      phase: 0,
      tone: ASSEMBLY_DRAFT,
      originTone: ASSEMBLY_DRAFT_ORIGIN,
      alpha: 0,
      depth: 0,
      companion: false,
      compatibility: null,
    })),
    engineeringTransition: {
      active: false,
      operation: null,
      stage: 'none',
      phase: 0,
      tone: ASSEMBLY_DRAFT,
      originTone: ASSEMBLY_DRAFT_ORIGIN,
      authorityTone: ASSEMBLY_DRAFT_MATERIAL,
      commitAllowed: false,
    },
    coupling: {
      active: false,
      connected: false,
      x: 0,
      y: 0,
      radius: 0,
      charge: 0,
      phase: 0,
      tone: FORM_CONTROLLED_RING,
      alpha: 0,
    },
    couplingTargets: new Pool<CouplingTargetMark>(() => ({
      x: 0,
      y: 0,
      radius: 0,
      effects: 0,
      phase: 0,
      tone: FORM_CONTROLLED_RING,
      alpha: 0,
    })),
    particles: new Pool<ParticleMark>(() => ({
      subject: 0,
      id: 0,
      x: 0,
      y: 0,
      radius: 1,
      tone: CURRENT_BRIGHT,
      alpha: 1,
    })),
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
    rim: { tone: PRESSURE_TONES[0], level: 0, beat: 0, crisis: false, localized: false },
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

interface PressureStyledMark {
  stress: number;
  stressTone: Tone;
  stressCrisis: boolean;
}

/** The visual force of one authored pressure stage, independent of its target. */
function pressureLevel(pressure: FramePressure): number {
  const stage =
    pressure.stage === 'crisis'
      ? 1
      : pressure.stage === 'pressure'
        ? 0.62
        : pressure.stage === 'resolution'
          ? 0.18
          : 0.32;
  return clamp01((pressure.level / 65535) * stage + (pressure.stage === 'crisis' ? 0.25 : 0));
}

/**
 * Applies only the strongest pressure that actually reaches a rendered subject.
 * A targetless pressure is intentionally global; addressed pressures stay on
 * their Node, Route, or layer instead of recolouring the whole Field.
 */
function applyPressure(
  next: FrameState,
  mark: PressureStyledMark,
  layer: number,
  node: number | null = null,
  route: number | null = null,
  relatedNodes: readonly number[] = [],
): void {
  mark.stress = 0;
  mark.stressTone = PRESSURE_TONES[0];
  mark.stressCrisis = false;
  for (const pressure of next.pressures) {
    if (pressure.queued) continue;
    const reaches =
      pressure.targetKind === 'none' ||
      (pressure.targetKind === 'layer' && pressure.target === layer) ||
      (pressure.targetKind === 'node' &&
        (pressure.target === node || relatedNodes.includes(pressure.target))) ||
      (pressure.targetKind === 'route' &&
        (pressure.target === route ||
          (relatedNodes.length > 0 &&
            next.routes.some(
              (candidate) =>
                candidate.route === pressure.target &&
                (relatedNodes.includes(candidate.tail) || relatedNodes.includes(candidate.head)),
            ))));
    if (!reaches) continue;
    const reading = pressureLevel(pressure);
    if (reading <= mark.stress) continue;
    mark.stress = reading;
    mark.stressTone = PRESSURE_TONES[pressure.ordinal % PRESSURE_TONES.length];
    mark.stressCrisis = pressure.stage === 'crisis';
  }
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
  tool: StillSurfaceTool = 'compartment',
  policyPreview: PolicyPreview | null = null,
  assemblyPreview: EngineeringAssemblyPreview | null = null,
  engineeringTransition: EngineeringTransitionCompanion | null = null,
  engineeringTransitionOrigin: EngineeringTransitionOrigin | null = null,
  engineeringTransitionClock = 0,
): Scene {
  for (const pool of [
    scene.hazes,
    scene.currents,
    scene.supplyLinks,
    scene.routes,
    scene.boundaries,
    scene.candidates,
    scene.assemblyBoundaries,
    scene.ports,
    scene.trails,
    scene.forms,
    scene.policies,
    scene.policyCandidates,
    scene.assemblyDrafts,
    scene.couplingTargets,
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
  scene.engineeringTransition.active = false;
  scene.engineeringTransition.operation = null;
  scene.engineeringTransition.stage = 'none';
  scene.engineeringTransition.phase = 0;
  scene.engineeringTransition.commitAllowed = false;
  scene.quality = renderQuality();
  scene.particleLimit = scene.quality.movingParticles;
  scene.backdrop = BACKDROP;
  const medium = next.mediumMotion;
  const mediumSpeed = medium ? Math.hypot(medium.vx, medium.vy) : 0;
  scene.medium.active = medium !== null && medium.drag > 0 && mediumSpeed > 0;
  scene.medium.dx = mediumSpeed > 0 && medium ? medium.vx / mediumSpeed : 1;
  scene.medium.dy = mediumSpeed > 0 && medium ? medium.vy / mediumSpeed : 0;
  scene.medium.speed = mediumSpeed;
  scene.medium.drag = medium ? medium.drag / 65535 : 0;
  scene.medium.collisionRadius = medium?.collisionRadius ?? 0;
  scene.medium.collisionResponse = medium ? medium.collisionResponse / 65535 : 0;
  scene.medium.alpha = scene.medium.active ? 0.055 + scene.medium.drag * 0.12 : 0;
  scene.medium.offset = reduced ? 0 : (scene.clock * mediumSpeed * 0.85) % 96;
  scene.medium.tone = CURRENT_GLASS;

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
  if (tool === 'view') fillCandidates(scene, next, place, zoom, cameraLayer, candidates);
  fillTrails(scene, ephemera, place, zoom, profile, reduced, cameraLayer);
  fillForms(scene, previous, next, place, zoom, held, cameraLayer);
  fillPolicyRuntime(scene, next, place, zoom, cameraLayer);
  fillPolicyPreview(scene, next, policyPreview, place, zoom, cameraLayer);
  fillAssemblyPreview(scene, assemblyPreview, place, zoom, cameraLayer);
  fillEngineeringTransition(
    scene,
    engineeringTransition,
    engineeringTransitionOrigin,
    engineeringTransitionClock,
    place,
    zoom,
    cameraLayer,
  );
  fillSupplyLinks(scene, next);
  fillCoupling(scene, next, zoom);
  fillEmbodiedRenewal(scene, next, place, zoom, cameraLayer);
  scene.particleLimit = Math.min(PARTICLE_CAP, scene.particles.count + scene.quality.movingParticles);
  fillParticles(scene, reduced);
  fillCues(scene, next, ephemera, place, zoom);
  fillHandles(scene, zoom, tool);
  fillPlayback(scene, playback, playbackClock);
  fillForecast(scene, next);
  fillRim(scene, next);
  return scene;
}

/**
 * Renewal stock is physical Field state, so it is projected in the world and
 * on its own layer rather than represented only by the laboratory inventory.
 * Local evidence uses the existing cue grammar: a quiet ring whose remaining
 * lifetime and radius expose how much usable signal is still present.
 */
function fillEmbodiedRenewal(
  scene: Scene,
  next: FrameState,
  place: Place,
  zoom: number,
  cameraLayer: number,
): void {
  const materialTones = [MATERIAL_JUNCTION, MATERIAL_BOUNDARY, MATERIAL_CONDUCTOR] as const;
  const materialLimit = scene.quality.persistentMarks;

  for (const material of next.materials) {
    if (material.claimed || scene.particles.count >= materialLimit) continue;
    const depth = material.layer - cameraLayer;
    const scale = zoom * depthScale(depth);
    const screen = place(material.x / 16, material.y / 16, depth);
    const mark = scene.particles.next();
    mark.subject = 1;
    mark.id = material.material;
    mark.x = screen.x;
    mark.y = screen.y;
    mark.radius = Math.max(2.4, scale * (5.5 + Math.min(4, material.amount / 512)));
    mark.tone = depthTone(materialTones[material.kind] ?? MATERIAL_CONDUCTOR, depth);
    mark.alpha = (0.72 + 0.2 * Math.min(1, material.amount / 1024)) * depthAlpha(depth);
  }

  for (const cache of next.wakeCaches) {
    if (scene.particles.count >= materialLimit) break;
    const depth = cache.layer - cameraLayer;
    const scale = zoom * depthScale(depth);
    const screen = place(cache.x / 16, cache.y / 16, depth);
    const retained = clamp01(cache.charge / (16 * 65_536));
    const waiting = clamp01(cache.remaining / 60);
    const mark = scene.particles.next();
    mark.subject = 2;
    mark.id = cache.ordinal;
    mark.x = screen.x;
    mark.y = screen.y;
    mark.radius = Math.max(4, scale * (7 + retained * 6));
    mark.tone = depthTone(CHARGE_SPARK, depth);
    mark.alpha = (0.68 + retained * 0.28) * depthAlpha(depth);
    const ring = scene.cues.next();
    ring.x = screen.x;
    ring.y = screen.y;
    ring.radius = Math.max(mark.radius * 1.5, scale * (cache.radius / 65_536) * 0.12);
    ring.tone = depthTone(CHARGE_CORE, depth);
    ring.alpha = (0.18 + 0.42 * (1 - waiting)) * depthAlpha(depth);
    ring.width = Math.max(1, scale * 1.8);
    ring.hard = false;
    ring.kind = 0;
  }

  for (const signal of next.localSignals) {
    const depth = signal.layer - cameraLayer;
    const scale = zoom * depthScale(depth);
    const screen = place(signal.x / 16, signal.y / 16, depth);
    const strength = signal.strength / 255;
    const lifetime = clamp01(signal.remaining / 180);
    const mark = scene.cues.next();
    mark.x = screen.x;
    mark.y = screen.y;
    mark.radius = Math.max(6, scale * (18 + strength * 38));
    mark.tone = depthTone(OBSERVATION_VIEW, depth);
    mark.alpha = (0.22 + 0.5 * strength) * (0.3 + 0.7 * lifetime) * depthAlpha(depth);
    mark.width = Math.max(1, scale * (1.5 + strength * 2.5));
    mark.hard = false;
    mark.kind = 0;
  }
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
    mark.diverted = current.diverted;
    mark.delayed = current.delayed;
    mark.emitting = current.emitting;
    mark.strength = current.strength / 65535;
    // The phase counter has no period in the frame, so it is read as a plain
    // turn: what it drives is a look, and nothing reads it back.
    mark.phase = (current.phase % 1024) / 1024;
    mark.depth = depth;
    mark.alpha = current.active
      ? depthAlpha(depth) * (current.emitting ? 1 : 0.28)
      : depthAlpha(depth) * 0.18;
    applyPressure(next, mark, current.layer);
    const standingTone = current.delayed
      ? INTERVENTION_DELAY
      : current.diverted
        ? INTERVENTION_DECOY
        : current.bright
          ? CURRENT_BRIGHT
          : CURRENT_PLAIN;
    const currentTone = depthTone(standingTone, depth);
    mark.tone = mixTone(currentTone, depthTone(mark.stressTone, depth), mark.stress * 0.2);
    // The band a Node has to stand within to receive, drawn at the width the
    // rule uses, so what a viewer sees is what a Node has to reach.
    mark.width = Math.max(
      4,
      zoom * depthScale(depth) * Math.max(10, current.width) * (current.bright ? 2.35 : 1.9),
    );
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
  const gateByNode = new Map(next.ports.map((port) => [port.node, port.open]));
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
    mark.requested = route.capacity > 0 ? clamp01(route.requested / route.capacity) : 0;
    mark.accepted = route.capacity > 0 ? clamp01(route.accepted / route.capacity) : 0;
    mark.transferOutcome = route.transferOutcome;
    mark.status = route.status;
    mark.clamped = route.clamped;
    mark.scrambled = route.scrambled;
    mark.automationEnabled = route.automationEnabled;
    mark.automationLimited = route.automationLimited;
    mark.tailOpen = gateByNode.get(route.tail) ?? false;
    mark.headOpen = gateByNode.get(route.head) ?? false;
    mark.gated = !mark.tailOpen || !mark.headOpen;
    mark.preview = QUEUED_STATUS.includes(route.status);
    mark.depth = depth;
    applyPressure(next, mark, Math.min(tail.layer, head.layer), null, route.route, [route.tail, route.head]);
    mark.width = Math.max(1.2, zoom * depthScale(depth) * (2.4 + 7.6 * flow));
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
    } else if (!mark.automationEnabled) {
      mark.tone = depthTone(ROUTE_AUTOMATION_DISABLED, depth);
      mark.alpha = 0.5 * fade;
      mark.width = Math.max(1, zoom * depthScale(depth) * 1.9);
    } else if (mark.gated) {
      mark.tone = depthTone(mixTone(ROUTE_BASE, HAZE, 0.68), depth);
      mark.alpha = 0.3 * fade;
      mark.width = Math.max(1, zoom * depthScale(depth) * 1.8);
    } else if (mark.transferOutcome === 'source_starved') {
      mark.tone = depthTone(mixTone(ROUTE_BASE, POLICY_BLOCKED, 0.72), depth);
      mark.alpha = 0.72 * fade;
    } else if (mark.transferOutcome === 'destination_headroom') {
      mark.tone = depthTone(mixTone(ROUTE_BASE, CHARGE_HIGH, 0.68), depth);
      mark.alpha = 0.78 * fade;
    } else if (mark.transferOutcome === 'capacity_throttled') {
      mark.tone = depthTone(mixTone(ROUTE_BASE, ROUTE_AUTOMATION, 0.7), depth);
      mark.alpha = 0.76 * fade;
    } else if (mark.transferOutcome === 'closed') {
      mark.tone = depthTone(ROUTE_AUTOMATION_DISABLED, depth);
      mark.alpha = 0.4 * fade;
      mark.width = Math.max(1, zoom * depthScale(depth) * 1.8);
    } else {
      mark.tone = depthTone(mixTone(ROUTE_BASE, ROUTE_FLOW, clamp01(flow * 1.4)), depth);
      mark.alpha = (0.4 + 0.48 * clamp01(flow * 1.6)) * fade;
    }
    if (route.clamped) {
      mark.tone = mixTone(mark.tone, depthTone(INTERVENTION_CLAMP, depth), 0.72);
      mark.width = Math.max(1, mark.width * 0.72);
    }
    if (route.scrambled) {
      mark.tone = mixTone(mark.tone, depthTone(INTERVENTION_SCRAMBLE, depth), 0.66);
    }
    if (mark.automationEnabled && mark.automationLimited) {
      mark.tone = mixTone(mark.tone, depthTone(ROUTE_AUTOMATION, depth), 0.52);
      mark.width = Math.max(1, mark.width * 0.82);
    }
    mark.tone = mixTone(mark.tone, depthTone(mark.stressTone, depth), mark.stress * 0.38);
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
    const charge = clamp01(port.charge / 65_535);
    const visibleCharge = Math.sqrt(charge);
    const mark = scene.ports.next();
    mark.node = port.node;
    mark.x = screen.x;
    mark.y = screen.y;
    mark.kind = port.kind;
    mark.open = port.open;
    mark.charge = charge;
    mark.overloaded = port.overloaded;
    mark.member = port.member;
    mark.shell = port.shell;
    mark.proposedMember = port.proposedMember;
    mark.decoyReceiver = port.decoyReceiver;
    mark.breached = port.breached;
    mark.reserve = port.reserve / 65535;
    mark.reserveDelta = (port.reserve - (from?.reserve ?? port.reserve)) / 65535;
    mark.delivered = ephemera.deliveryStrength(port.node, scene.clock);
    mark.depth = depth;
    applyPressure(next, mark, port.layer, port.node);
    const scaled = depthScale(depth) * (port.kind === FORM_NODE_KIND ? 0.62 : 1);
    mark.radius = zoom * PORT_UNITS * (0.46 + 0.66 * visibleCharge) * scaled;
    mark.bloom = mark.radius * (2.1 + 4.6 * charge + 1.2 * mark.delivered);
    // Ports are made prominent while the Still Mode surface is up. A closed
    // Port is drawn faintly during play, because what it is doing is nothing;
    // in Still Mode it is a place a Route can end and a Pulse can open, so it
    // is lifted toward the legibility an open one already has.
    const legible = port.open ? 1 : 0.42 + 0.48 * scene.still.presence;
    mark.alpha = legible * depthAlpha(depth);
    const warm = mixTone(CHARGE_LOW, CHARGE_HIGH, charge);
    const portTone = depthTone(
      port.overloaded ? OVERLOAD : mixTone(PORT_BASE, warm, clamp01(0.2 + 0.8 * charge)),
      depth,
    );
    const intervenedTone = port.breached
      ? mixTone(portTone, depthTone(INTERVENTION_BREACH, depth), 0.62)
      : port.decoyReceiver
        ? mixTone(portTone, depthTone(INTERVENTION_DECOY, depth), 0.5)
        : portTone;
    mark.tone = mixTone(intervenedTone, depthTone(mark.stressTone, depth), mark.stress * 0.42);
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
  physicalHulls(scene, next, place, zoom, cameraLayer, false);
  // The physical compartment a queued causal change proposes. Passive View
  // changes never raise these flags and never appear as construction lines.
  if (next.ports.some((port) => port.proposedMember)) {
    physicalHulls(scene, next, place, zoom, cameraLayer, true);
  }

  // The active observation View is an independent bitset. It is drawn after
  // the material edge so its thin violet aperture remains legible where the
  // two cross, but it never supplies physical membership or shell flags.
  const viewed = new Set<number>();
  next.inside.forEach((held, index) => {
    if (held && next.ports[index]) viewed.add(next.ports[index].node);
  });
  if (viewed.size > 0) {
    hullsInto(
      scene.boundaries,
      next,
      place,
      zoom,
      cameraLayer,
      (port) => viewed.has(port.node),
      {
        tone: OBSERVATION_VIEW,
        weight: 0.94,
        role: 'view',
        proposed: false,
        candidate: 0,
        focused: false,
        tier: 0,
      },
    );
  }
}

/**
 * One closed hull per plane, over the members of either the standing inside or
 * the one a queued change proposes.
 */
function physicalHulls(
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
      tone: proposed ? PHYSICAL_COMPARTMENT_PROPOSED : PHYSICAL_COMPARTMENT,
      weight: proposed ? 0.8 : 0.92,
      role: 'compartment',
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
      role: 'candidate',
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
  role: BoundaryRole;
  proposed: boolean;
  candidate: number;
  focused: boolean;
  tier: number;
}

/** The one hull pass the compartment, View, and candidate outlines share. */
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
    if (style.role === 'compartment' && !style.proposed) {
      applyPressure(next, mark, layer, null, null, mark.nodes);
    } else {
      mark.stress = 0;
      mark.stressTone = PRESSURE_TONES[0];
      mark.stressCrisis = false;
    }
    mark.tone = mixTone(style.tone, mark.stressTone, mark.stress * 0.34);
    mark.alpha = style.weight * depthAlpha(depth);
    const units =
      style.role === 'compartment'
        ? COMPARTMENT_EDGE_UNITS
        : style.role === 'view'
          ? VIEW_EDGE_UNITS
          : CANDIDATE_EDGE_UNITS;
    mark.width =
      Math.max(style.role === 'compartment' ? 3 : 1.5, zoom * depthScale(depth) * units) *
      (1 + mark.stress * 0.18);
    mark.role = style.role;
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

/** Builds one pooled chassis silhouette and inset core for both render engines. */
function fillFormChassis(mark: FormMark): void {
  const profile = FORM_PROFILES[mark.formOrdinal % FORM_PROFILES.length];
  const stableTurn = ((mark.form % 12) / 12) * Math.PI * 2;
  const turn = profile.turn + (mark.controlled ? mark.heading : stableTurn);
  const points = profile.sides * 2;
  mark.outline.length = 0;
  mark.core.length = 0;

  for (let point = 0; point < points; point += 1) {
    const angle = turn + (point / points) * Math.PI * 2;
    const tooth = point % 2 === 0 ? 1 : profile.inset;
    const identity = 1 + Math.sin((mark.form + 1) * (point + 1) * 1.73) * 0.028;
    mark.outline.push(
      mark.x + Math.cos(angle) * mark.radius * tooth * profile.stretchX * identity,
      mark.y + Math.sin(angle) * mark.radius * tooth * profile.stretchY * identity,
    );
  }

  for (let corner = 0; corner < profile.sides; corner += 1) {
    const angle = turn + (corner / profile.sides) * Math.PI * 2;
    mark.core.push(
      mark.x + Math.cos(angle) * mark.radius * profile.core * profile.stretchX,
      mark.y + Math.sin(angle) * mark.radius * profile.core * profile.stretchY,
    );
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
    mark.form = form.id;
    mark.formOrdinal = form.formOrdinal % FORM_PROFILES.length;
    mark.x = screen.x;
    mark.y = screen.y;
    mark.controlled = form.controlled;
    mark.focus = form.focus;
    mark.pulseCharging = form.pulseCharging;
    mark.charge = charge;
    mark.sides = FORM_PROFILES[mark.formOrdinal].sides;
    mark.heading = Math.hypot(form.vx, form.vy) > 0.001 ? Math.atan2(form.vy, form.vx) : 0;
    mark.depth = depth;
    mark.alpha = depthAlpha(depth);
    applyPressure(next, mark, form.layer);
    // The controlled Form is the largest and the brightest mark on the surface,
    // by a margin no other mark closes: which one the player steers has to be
    // answerable at a glance and without a label.
    const extent = FORM_UNITS * (form.controlled ? 3.45 : 1.02) * (0.86 + 0.42 * charge);
    mark.radius = zoom * extent * depthScale(depth);
    mark.tone = depthTone(form.controlled ? FORM_CONTROLLED : FORM_PLAIN, depth);
    const ringTone = depthTone(
      form.controlled ? FORM_CONTROLLED_RING : mixTone(FORM_PLAIN, CHARGE_CORE, 0.38 * charge),
      depth,
    );
    mark.ringTone = mixTone(ringTone, depthTone(mark.stressTone, depth), mark.stress * 0.32);
    const bodyTone = mixTone(0x0d1014, HAZE_TINT, Math.min(0.55, 0.2 * Math.abs(depth)));
    mark.bodyTone = mixTone(bodyTone, mark.stressTone, mark.stress * 0.16);
    mark.facetTone = depthTone(form.controlled ? FORM_FACET : FORM_PLAIN, depth);
    mark.facetShadow = depthTone(FORM_FACET_SHADOW, depth);
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
  for (let place = 0; place < scene.forms.count; place += 1) {
    fillFormChassis(scene.forms.items[place]);
  }
}

function policyOutcomeTone(outcome: FramePolicyRuntime['outcome']): Tone {
  switch (outcome) {
    case 'applied':
      return POLICY_ACTIVE;
    case 'cooldown':
    case 'capacity_reached':
    case 'no_effect':
      return POLICY_WAIT;
    case 'no_target':
    case 'target_unavailable':
    case 'wrong_layer':
    case 'out_of_range':
    case 'unavailable':
      return POLICY_BLOCKED;
    case 'idle':
    case 'held':
    default:
      return POLICY_IDLE;
  }
}

function currentPolicyTarget(mark: CurrentMark): { x: number; y: number; radius: number } | null {
  const pairs = mark.points.length / 2;
  if (pairs === 0) return null;
  const point = Math.min(pairs - 1, Math.floor(pairs / 2));
  return {
    x: mark.points[point * 2],
    y: mark.points[point * 2 + 1],
    radius: Math.max(5, mark.width * 0.36),
  };
}

/**
 * Projects the core-retained policy decision without repeating policy logic.
 * Target lookup resolves an already-selected identity only; missing geometry
 * remains a visible no-target/outcome stance rather than triggering retargeting.
 */
function fillPolicyRuntime(
  scene: Scene,
  next: FrameState,
  place: Place,
  zoom: number,
  cameraLayer: number,
): void {
  const ports = new Map<number, PortMark>();
  const routes = new Map<number, RouteMark>();
  const currents = new Map<number, CurrentMark>();
  for (let index = 0; index < scene.ports.count; index += 1) {
    const mark = scene.ports.items[index];
    ports.set(mark.node, mark);
  }
  for (let index = 0; index < scene.routes.count; index += 1) {
    const mark = scene.routes.items[index];
    if (!mark.preview) routes.set(mark.route, mark);
  }
  for (let index = 0; index < scene.currents.count; index += 1) {
    const mark = scene.currents.items[index];
    currents.set(mark.id, mark);
  }

  for (const runtime of next.policies) {
    const source = ports.get(runtime.address);
    if (!source) continue;
    const mark = scene.policies.next();
    mark.address = runtime.address;
    mark.action = runtime.action;
    mark.outcome = runtime.outcome;
    mark.targetKind = runtime.targetKind;
    mark.rule = runtime.rule;
    mark.x = source.x;
    mark.y = source.y;
    mark.radius = Math.max(source.radius * 2.05, scene.dpr * 8);
    const selectedRadius = runtime.radius > 0
      ? Math.max(mark.radius, (runtime.radius / 65_536) * zoom * depthScale(source.depth))
      : 0;
    const sensing = runtime.action === 'seek_supply'
      || runtime.action === 'seek_port'
      || runtime.action === 'seek_signal';
    mark.sensorReach = sensing ? selectedRadius : 0;
    mark.reach = sensing ? 0 : selectedRadius;
    mark.hasTarget = false;
    mark.targetX = source.x;
    mark.targetY = source.y;
    mark.targetRadius = Math.max(source.radius, scene.dpr * 5);
    mark.tone = depthTone(policyOutcomeTone(runtime.outcome), source.depth);
    mark.targetTone = depthTone(POLICY_TARGET, source.depth);
    mark.alpha = source.alpha * (runtime.action === 'none' ? 0.34 : 0.8);
    mark.phase = scene.reducedMotion ? 0.25 : (scene.clock * 0.035 + runtime.address * 0.071) % 1;
    mark.preview = false;

    let target: { x: number; y: number; radius: number; tone: Tone } | null = null;
    if (runtime.targetKind === 'node') {
      const held = ports.get(runtime.target);
      if (held) target = { x: held.x, y: held.y, radius: held.radius * 1.65, tone: held.tone };
    } else if (runtime.targetKind === 'route') {
      const held = routes.get(runtime.target);
      if (held) {
        target = {
          x: (held.x1 + held.x2) / 2,
          y: (held.y1 + held.y2) / 2,
          radius: Math.max(scene.dpr * 6, held.width * 2.2),
          tone: held.tone,
        };
      }
    } else if (runtime.targetKind === 'current') {
      const held = currents.get(runtime.target);
      const at = held ? currentPolicyTarget(held) : null;
      if (held && at) target = { ...at, tone: held.tone };
    } else if (runtime.targetKind === 'signal') {
      const held = next.localSignals.find((signal) => signal.signal === runtime.target);
      if (held) {
        const depth = held.layer - cameraLayer;
        const at = place(held.x / 16, held.y / 16, depth);
        target = {
          x: at.x,
          y: at.y,
          radius: Math.max(scene.dpr * 6, zoom * depthScale(depth) * 9),
          tone: MATERIAL_CONDUCTOR,
        };
      }
    }
    if (target) {
      mark.hasTarget = true;
      mark.targetX = target.x;
      mark.targetY = target.y;
      mark.targetRadius = target.radius;
      mark.targetTone = mixTone(depthTone(target.tone, source.depth), POLICY_TARGET, 0.68);
    }
  }
}

function previewGeometry(
  scene: Scene,
  next: FrameState,
  kind: PolicyTargetKind,
  id: number,
  place: Place,
  zoom: number,
  cameraLayer: number,
): { x: number; y: number; radius: number; tone: Tone; depth: number } | null {
  if (kind === 'node') {
    for (let index = 0; index < scene.ports.count; index += 1) {
      const mark = scene.ports.items[index];
      if (mark.node === id) {
        return {
          x: mark.x,
          y: mark.y,
          radius: mark.radius * 1.65,
          tone: mark.tone,
          depth: mark.depth,
        };
      }
    }
  }
  if (kind === 'route') {
    for (let index = 0; index < scene.routes.count; index += 1) {
      const mark = scene.routes.items[index];
      if (mark.route === id && !mark.preview) {
        return {
          x: (mark.x1 + mark.x2) / 2,
          y: (mark.y1 + mark.y2) / 2,
          radius: Math.max(scene.dpr * 6, mark.width * 2.2),
          tone: mark.tone,
          depth: mark.depth,
        };
      }
    }
  }
  if (kind === 'current') {
    for (let index = 0; index < scene.currents.count; index += 1) {
      const mark = scene.currents.items[index];
      if (mark.id === id) {
        const at = currentPolicyTarget(mark);
        if (at) return { ...at, tone: mark.tone, depth: mark.depth };
      }
    }
  }
  if (kind === 'signal') {
    const signal = next.localSignals.find((held) => held.signal === id);
    if (signal) {
      const depth = signal.layer - cameraLayer;
      const at = place(signal.x / 16, signal.y / 16, depth);
      return {
        x: at.x,
        y: at.y,
        radius: Math.max(scene.dpr * 6, zoom * depthScale(depth) * 9),
        tone: MATERIAL_CONDUCTOR,
        depth,
      };
    }
  }
  return null;
}

/** Projects the pure Rust draft result; no targeting or admission is repeated here. */
function fillPolicyPreview(
  scene: Scene,
  next: FrameState,
  preview: PolicyPreview | null,
  place: Place,
  zoom: number,
  cameraLayer: number,
): void {
  if (!preview) return;
  let source: PortMark | null = null;
  for (let index = 0; index < scene.ports.count; index += 1) {
    const held = scene.ports.items[index];
    if (held.node === preview.address) {
      source = held;
      break;
    }
  }
  if (!source) return;

  for (const candidate of preview.candidates) {
    if (candidate.id === null || candidate.kind === 'none') continue;
    const geometry = previewGeometry(
      scene,
      next,
      candidate.kind,
      candidate.id,
      place,
      zoom,
      cameraLayer,
    );
    if (!geometry) continue;
    const mark = scene.policyCandidates.next();
    mark.id = candidate.id;
    mark.kind = candidate.kind;
    mark.x = geometry.x;
    mark.y = geometry.y;
    mark.radius = Math.max(geometry.radius, scene.dpr * 5);
    mark.selected = candidate.kind === preview.target_kind && candidate.id === preview.target;
    mark.tone = depthTone(mark.selected ? POLICY_TARGET : POLICY_PREVIEW, geometry.depth);
    mark.alpha = source.alpha * (mark.selected ? 0.9 : 0.44);
  }

  const mark = scene.policies.next();
  mark.address = preview.address;
  mark.action = preview.action.kind;
  const needsTarget = preview.action.kind === 'seek_supply'
    || preview.action.kind === 'seek_port'
    || preview.action.kind === 'seek_signal'
    || preview.action.kind === 'couple';
  mark.outcome = needsTarget && preview.target_kind === 'none'
    ? 'no_target'
    : 'idle';
  mark.targetKind = preview.target_kind;
  mark.rule = preview.rule;
  mark.x = source.x;
  mark.y = source.y;
  mark.radius = Math.max(source.radius * 2.35, scene.dpr * 9);
  mark.reach = preview.action_radius > 0
    ? Math.max(mark.radius, (preview.action_radius / 65_536) * zoom * depthScale(source.depth))
    : 0;
  mark.sensorReach = preview.sensor_radius > 0
    ? Math.max(mark.radius, (preview.sensor_radius / 65_536) * zoom * depthScale(source.depth))
    : 0;
  mark.hasTarget = false;
  mark.targetX = source.x;
  mark.targetY = source.y;
  mark.targetRadius = Math.max(source.radius, scene.dpr * 5);
  mark.tone = depthTone(POLICY_PREVIEW, source.depth);
  mark.targetTone = depthTone(POLICY_TARGET, source.depth);
  mark.alpha = source.alpha * 0.88;
  mark.phase = scene.reducedMotion ? 0.2 : (scene.clock * 0.025 + preview.address * 0.047) % 1;
  mark.preview = true;
  if (preview.target !== null && preview.target_kind !== 'none') {
    const target = previewGeometry(
      scene,
      next,
      preview.target_kind,
      preview.target,
      place,
      zoom,
      cameraLayer,
    );
    if (target) {
      mark.hasTarget = true;
      mark.targetX = target.x;
      mark.targetY = target.y;
      mark.targetRadius = target.radius;
      mark.targetTone = mixTone(depthTone(target.tone, source.depth), POLICY_TARGET, 0.72);
    }
  }
}

type AssemblyPayload = Record<string, unknown>;

function assemblyPayload(
  change: EngineeringAssemblyDiffChange,
  side: 'before' | 'after',
): AssemblyPayload {
  const value = change[side];
  return value && typeof value === 'object' && !Array.isArray(value)
    ? value as AssemblyPayload
    : {};
}

function assemblyNumber(value: AssemblyPayload, key: string): number {
  const found = value[key];
  return typeof found === 'number' && Number.isFinite(found) ? found : 0;
}

function assemblyFlag(value: AssemblyPayload, key: string): boolean {
  return value[key] === true;
}

function assemblyPosition(value: AssemblyPayload): { x: number; y: number } {
  const found = value.pos;
  if (!found || typeof found !== 'object' || Array.isArray(found)) return { x: 0, y: 0 };
  const position = found as AssemblyPayload;
  return { x: assemblyNumber(position, 'x'), y: assemblyNumber(position, 'y') };
}

function assemblyMembers(value: AssemblyPayload): number[] {
  const found = value.members;
  if (!Array.isArray(found)) return [];
  return found.filter((member): member is number => Number.isInteger(member));
}

function sameMembers(left: readonly number[], right: readonly number[]): boolean {
  return left.length === right.length && left.every((member, index) => member === right[index]);
}

function scenePortRadius(scene: Scene, node: number, fallback: number): number {
  for (let index = 0; index < scene.ports.count; index += 1) {
    const port = scene.ports.items[index];
    if (port.node === node) return Math.max(fallback, port.radius * 1.55);
  }
  return fallback;
}

function sceneFormRadius(scene: Scene, node: number, fallback: number): number {
  for (let index = 0; index < scene.forms.count; index += 1) {
    const form = scene.forms.items[index];
    if (form.form === node) return Math.max(fallback, form.radius * 1.48);
  }
  return scenePortRadius(scene, node, fallback);
}

function pointAlong(points: readonly number[], fraction: number): { x: number; y: number } | null {
  if (points.length < 2) return null;
  if (points.length === 2) return { x: points[0], y: points[1] };
  const lengths: number[] = [];
  let total = 0;
  for (let index = 2; index < points.length; index += 2) {
    const length = Math.hypot(points[index] - points[index - 2], points[index + 1] - points[index - 1]);
    lengths.push(length);
    total += length;
  }
  if (total <= 0) return { x: points[0], y: points[1] };
  let wanted = clamp01(fraction) * total;
  for (let index = 0; index < lengths.length; index += 1) {
    const length = lengths[index];
    if (wanted <= length || index === lengths.length - 1) {
      const along = length <= 0 ? 0 : wanted / length;
      const point = index * 2;
      return {
        x: points[point] + (points[point + 2] - points[point]) * along,
        y: points[point + 1] + (points[point + 3] - points[point + 1]) * along,
      };
    }
    wanted -= length;
  }
  return { x: points[points.length - 2], y: points[points.length - 1] };
}

function fillAssemblyBoundary(
  scene: Scene,
  draft: EngineeringAssemblyDraft,
  before: AssemblyPayload,
  after: AssemblyPayload,
  place: Place,
  zoom: number,
  cameraLayer: number,
  companion = false,
  alpha = 0.82,
  forceFields = false,
  compatibility: EngineeringAssemblyCompatibilityDisposition | null = null,
): void {
  const afterMembers = assemblyMembers(after);
  const beforeMembers = assemblyMembers(before);
  const candidateComponents = new Map(
    draft.components.map((component) => [component.node, component] as const),
  );
  const planes = new Map<number, { x: number; y: number; node: number }[]>();
  const anchors: Array<{ x: number; y: number }> = [];
  for (const node of afterMembers) {
    const component = candidateComponents.get(node);
    if (!component) continue;
    const point = {
      x: component.pos.x / 65_536,
      y: component.pos.y / 65_536,
      node,
    };
    const held = planes.get(component.layer);
    if (held) held.push(point);
    else planes.set(component.layer, [point]);
    anchors.push(place(point.x, point.y, component.layer - cameraLayer));
  }

  for (const [layer, members] of [...planes].sort((left, right) => right[0] - left[0])) {
    const depth = layer - cameraLayer;
    const hull = members.length <= 2 ? members : convexHull(members);
    const boundary = scene.assemblyBoundaries.next();
    boundary.points.length = 0;
    boundary.nodes.length = 0;
    for (const member of hull) {
      const at = place(member.x, member.y, depth);
      boundary.points.push(at.x, at.y);
      boundary.nodes.push(member.node);
    }
    boundary.tone = depthTone(
      compatibility === 'hard_refusal'
        ? OVERLOAD
        : compatibility === 'adaptation_required'
          ? ASSEMBLY_DRAFT_MATERIAL
          : ASSEMBLY_DRAFT,
      depth,
    );
    boundary.alpha = alpha * depthAlpha(depth);
    boundary.width = Math.max(2, zoom * depthScale(depth) * 4.2);
    boundary.role = 'assembly-draft';
    boundary.proposed = true;
    boundary.candidate = 0;
    boundary.focused = true;
    boundary.tier = 0;
    boundary.stress = 0;
    boundary.stressTone = PRESSURE_TONES[0];
    boundary.stressCrisis = false;
  }

  if (anchors.length === 0) {
    for (const node of beforeMembers) {
      const port = nextPort(scene, node);
      if (port) anchors.push({ x: port.x, y: port.y });
    }
  }
  const x = anchors.length > 0
    ? anchors.reduce((sum, anchor) => sum + anchor.x, 0) / anchors.length
    : scene.width / 2;
  const y = anchors.length > 0
    ? anchors.reduce((sum, anchor) => sum + anchor.y, 0) / anchors.length
    : scene.height / 2;
  const mark = scene.assemblyDrafts.next();
  mark.kind = 'physical_compartment';
  mark.id = 0;
  mark.x = x;
  mark.y = y;
  mark.radius = Math.max(scene.dpr * 11, zoom * 16);
  mark.fromX = x;
  mark.fromY = y;
  mark.fromRadius = mark.radius;
  mark.displaced = false;
  mark.fields = forceFields
    ? ASSEMBLY_DRAFT_FIELD.members | ASSEMBLY_DRAFT_FIELD.leakage
    : (sameMembers(beforeMembers, afterMembers) ? 0 : ASSEMBLY_DRAFT_FIELD.members)
      | (assemblyNumber(before, 'leak_per_exposed_contact_per_step')
        === assemblyNumber(after, 'leak_per_exposed_contact_per_step')
        ? 0
        : ASSEMBLY_DRAFT_FIELD.leakage);
  mark.beforeLevel = clamp01(assemblyNumber(before, 'leak_per_exposed_contact_per_step') / 65_536);
  mark.afterLevel = clamp01(assemblyNumber(after, 'leak_per_exposed_contact_per_step') / 65_536);
  mark.count = afterMembers.length;
  mark.active = true;
  mark.open = false;
  mark.phase = 0;
  mark.tone = ASSEMBLY_DRAFT;
  mark.originTone = ASSEMBLY_DRAFT_ORIGIN;
  mark.alpha = alpha;
  mark.depth = 0;
  mark.companion = companion;
  mark.compatibility = compatibility;
}

function nextPort(scene: Scene, node: number): PortMark | null {
  for (let index = 0; index < scene.ports.count; index += 1) {
    const port = scene.ports.items[index];
    if (port.node === node) return port;
  }
  return null;
}

/** Projects only Rust-accepted normalized assembly changes into the draft register. */
function fillAssemblyPreview(
  scene: Scene,
  preview: EngineeringAssemblyPreview | null,
  place: Place,
  zoom: number,
  cameraLayer: number,
): void {
  if (!preview || preview.status !== 'accepted') return;
  const componentDrafts = new Map(
    preview.candidate_draft.components.map((component) => [component.node, component] as const),
  );
  const compartmentMembers = new Set(preview.candidate_draft.physical_compartment.members);
  const compartmentGeometryChanged = preview.diff.definition.changes.some((change) => {
    if (change.kind !== 'component') return false;
    const before = assemblyPayload(change, 'before');
    const after = assemblyPayload(change, 'after');
    if (!compartmentMembers.has(assemblyNumber(after, 'node'))) return false;
    const beforePosition = assemblyPosition(before);
    const afterPosition = assemblyPosition(after);
    return beforePosition.x !== afterPosition.x
      || beforePosition.y !== afterPosition.y
      || assemblyNumber(before, 'layer') !== assemblyNumber(after, 'layer');
  });
  let compartmentProjected = false;

  for (const change of preview.diff.definition.changes) {
    const before = assemblyPayload(change, 'before');
    const after = assemblyPayload(change, 'after');
    if (change.kind === 'physical_compartment') {
      fillAssemblyBoundary(
        scene,
        preview.candidate_draft,
        before,
        after,
        place,
        zoom,
        cameraLayer,
      );
      compartmentProjected = true;
      continue;
    }

    const mark = scene.assemblyDrafts.next();
    mark.kind = change.kind;
    mark.id = change.kind === 'current'
      ? assemblyNumber(after, 'current')
      : change.kind === 'material'
        ? assemblyNumber(after, 'material')
        : assemblyNumber(after, 'node');
    mark.fields = 0;
    mark.beforeLevel = 0;
    mark.afterLevel = 0;
    mark.count = 0;
    mark.active = true;
    mark.open = false;
    mark.phase = (mark.id * 0.037) % 1;
    mark.tone = ASSEMBLY_DRAFT;
    mark.originTone = ASSEMBLY_DRAFT_ORIGIN;
    mark.alpha = 0.9;
    mark.depth = 0;
    mark.companion = false;
    mark.compatibility = null;

    if (change.kind === 'component') {
      const beforePosition = assemblyPosition(before);
      const afterPosition = assemblyPosition(after);
      const beforeLayer = assemblyNumber(before, 'layer');
      const afterLayer = assemblyNumber(after, 'layer');
      const depth = afterLayer - cameraLayer;
      const originDepth = beforeLayer - cameraLayer;
      const at = place(afterPosition.x / 65_536, afterPosition.y / 65_536, depth);
      const from = place(beforePosition.x / 65_536, beforePosition.y / 65_536, originDepth);
      mark.x = at.x;
      mark.y = at.y;
      mark.fromX = from.x;
      mark.fromY = from.y;
      mark.radius = scenePortRadius(scene, mark.id, Math.max(scene.dpr * 9, zoom * depthScale(depth) * 18));
      mark.fromRadius = Math.max(scene.dpr * 6, mark.radius * 0.64);
      mark.displaced = beforePosition.x !== afterPosition.x
        || beforePosition.y !== afterPosition.y
        || beforeLayer !== afterLayer;
      if (beforePosition.x !== afterPosition.x || beforePosition.y !== afterPosition.y) {
        mark.fields |= ASSEMBLY_DRAFT_FIELD.position;
      }
      if (beforeLayer !== afterLayer) mark.fields |= ASSEMBLY_DRAFT_FIELD.layer;
      if (assemblyNumber(before, 'q') !== assemblyNumber(after, 'q')) {
        mark.fields |= ASSEMBLY_DRAFT_FIELD.charge;
      }
      if (assemblyFlag(before, 'open') !== assemblyFlag(after, 'open')) {
        mark.fields |= ASSEMBLY_DRAFT_FIELD.interface;
      }
      mark.beforeLevel = clamp01(assemblyNumber(before, 'q') / (4096 * 65_536));
      mark.afterLevel = clamp01(assemblyNumber(after, 'q') / (4096 * 65_536));
      mark.open = assemblyFlag(after, 'open');
      mark.tone = depthTone(ASSEMBLY_DRAFT, depth);
      mark.originTone = depthTone(ASSEMBLY_DRAFT_ORIGIN, originDepth);
      mark.alpha *= depthAlpha(depth);
      mark.depth = depth;
      continue;
    }

    if (change.kind === 'form') {
      const component = componentDrafts.get(mark.id);
      const port = nextPort(scene, mark.id);
      const depth = component ? component.layer - cameraLayer : port?.depth ?? 0;
      const at = component
        ? place(component.pos.x / 65_536, component.pos.y / 65_536, depth)
        : port
          ? { x: port.x, y: port.y }
          : { x: scene.width / 2, y: scene.height / 2 };
      mark.x = at.x;
      mark.y = at.y;
      mark.fromX = at.x;
      mark.fromY = at.y;
      mark.radius = sceneFormRadius(scene, mark.id, Math.max(scene.dpr * 13, zoom * depthScale(depth) * 24));
      mark.fromRadius = mark.radius;
      mark.displaced = false;
      if (assemblyNumber(before, 'reserve') !== assemblyNumber(after, 'reserve')) {
        mark.fields |= ASSEMBLY_DRAFT_FIELD.reserve;
      }
      if (before.junction_blanks !== after.junction_blanks) {
        mark.fields |= ASSEMBLY_DRAFT_FIELD.blanks;
      }
      mark.beforeLevel = clamp01(assemblyNumber(before, 'reserve') / (4096 * 65_536));
      mark.afterLevel = clamp01(assemblyNumber(after, 'reserve') / (4096 * 65_536));
      mark.count = after.junction_blanks === null ? 0 : assemblyNumber(after, 'junction_blanks');
      mark.tone = depthTone(ASSEMBLY_DRAFT, depth);
      mark.originTone = depthTone(ASSEMBLY_DRAFT_ORIGIN, depth);
      mark.alpha *= depthAlpha(depth);
      mark.depth = depth;
      continue;
    }

    if (change.kind === 'material') {
      const beforePosition = assemblyPosition(before);
      const afterPosition = assemblyPosition(after);
      const beforeLayer = assemblyNumber(before, 'layer');
      const afterLayer = assemblyNumber(after, 'layer');
      const depth = afterLayer - cameraLayer;
      const originDepth = beforeLayer - cameraLayer;
      const at = place(afterPosition.x / 65_536, afterPosition.y / 65_536, depth);
      const from = place(beforePosition.x / 65_536, beforePosition.y / 65_536, originDepth);
      mark.x = at.x;
      mark.y = at.y;
      mark.fromX = from.x;
      mark.fromY = from.y;
      mark.radius = Math.max(scene.dpr * 10, zoom * depthScale(depth) * (12 + Math.min(8, assemblyNumber(after, 'amount') / 256)));
      mark.fromRadius = Math.max(scene.dpr * 5, mark.radius * 0.55);
      mark.displaced = beforePosition.x !== afterPosition.x
        || beforePosition.y !== afterPosition.y
        || beforeLayer !== afterLayer;
      if (beforePosition.x !== afterPosition.x || beforePosition.y !== afterPosition.y) {
        mark.fields |= ASSEMBLY_DRAFT_FIELD.position;
      }
      if (beforeLayer !== afterLayer) mark.fields |= ASSEMBLY_DRAFT_FIELD.layer;
      if (assemblyNumber(before, 'amount') !== assemblyNumber(after, 'amount')) {
        mark.fields |= ASSEMBLY_DRAFT_FIELD.amount;
      }
      mark.beforeLevel = clamp01(assemblyNumber(before, 'amount') / 1024);
      mark.afterLevel = clamp01(assemblyNumber(after, 'amount') / 1024);
      mark.count = Math.min(6, Math.ceil(Math.log2(assemblyNumber(after, 'amount') + 1)));
      mark.tone = depthTone(ASSEMBLY_DRAFT_MATERIAL, depth);
      mark.originTone = depthTone(ASSEMBLY_DRAFT_ORIGIN, originDepth);
      mark.alpha *= depthAlpha(depth);
      mark.depth = depth;
      continue;
    }

    const current = [...scene.currents.items.slice(0, scene.currents.count)]
      .find((candidate) => candidate.id === mark.id);
    const beforePhaseRaw = assemblyNumber(before, 'phase');
    const afterPhaseRaw = assemblyNumber(after, 'phase');
    const beforePhase = (beforePhaseRaw % 1024) / 1024;
    const afterPhase = (afterPhaseRaw % 1024) / 1024;
    const at = current ? pointAlong(current.points, afterPhase) : null;
    const from = current ? pointAlong(current.points, beforePhase) : null;
    mark.x = at?.x ?? scene.width / 2;
    mark.y = at?.y ?? scene.height / 2;
    mark.fromX = from?.x ?? mark.x;
    mark.fromY = from?.y ?? mark.y;
    mark.radius = Math.max(scene.dpr * 12, (current?.width ?? scene.dpr * 5) * 0.72);
    mark.fromRadius = Math.max(scene.dpr * 5, mark.radius * 0.5);
    mark.displaced = beforePhaseRaw !== afterPhaseRaw;
    if (beforePhaseRaw !== afterPhaseRaw) mark.fields |= ASSEMBLY_DRAFT_FIELD.phase;
    if (assemblyFlag(before, 'active') !== assemblyFlag(after, 'active')) {
      mark.fields |= ASSEMBLY_DRAFT_FIELD.active;
    }
    mark.beforeLevel = clamp01(beforePhase);
    mark.afterLevel = clamp01(afterPhase);
    mark.active = assemblyFlag(after, 'active');
    mark.phase = afterPhase;
    mark.tone = depthTone(ASSEMBLY_DRAFT_CURRENT, current?.depth ?? 0);
    mark.originTone = depthTone(ASSEMBLY_DRAFT_ORIGIN, current?.depth ?? 0);
    mark.alpha *= depthAlpha(current?.depth ?? 0);
    mark.depth = current?.depth ?? 0;
  }

  if (compartmentGeometryChanged && !compartmentProjected) {
    const compartment = preview.candidate_draft.physical_compartment as unknown as AssemblyPayload;
    fillAssemblyBoundary(
      scene,
      preview.candidate_draft,
      compartment,
      compartment,
      place,
      zoom,
      cameraLayer,
    );
  }
}

function transitionSequence(
  status: EngineeringTransitionCompanion['status'],
  clock: number,
  reducedMotion: boolean,
): { stage: EngineeringTransitionCompanionStage; phase: number; movement: number; alpha: number } {
  if (status === 'preview') return { stage: 'preview', phase: 1, movement: 1, alpha: 0.72 };
  if (reducedMotion || clock >= 1) return { stage: 'locked', phase: 1, movement: 1, alpha: 0.82 };
  const progress = clamp01(clock);
  if (progress < 0.24) {
    const phase = progress / 0.24;
    return { stage: 'deenergize', phase, movement: 0, alpha: 0.88 - phase * 0.3 };
  }
  if (progress < 0.72) {
    const phase = (progress - 0.24) / 0.48;
    return {
      stage: 'reconstruct',
      phase,
      movement: phase * phase * (3 - 2 * phase),
      alpha: 0.58 + phase * 0.38,
    };
  }
  const phase = (progress - 0.72) / 0.28;
  return { stage: 'settle', phase, movement: 1, alpha: 0.96 - phase * 0.14 };
}

function transitionTones(operation: EngineeringTransitionKind): {
  tone: Tone;
  origin: Tone;
  authority: Tone;
} {
  if (operation === 'revert_generator') {
    return { tone: ASSEMBLY_DRAFT_MATERIAL, origin: ASSEMBLY_DRAFT_ORIGIN, authority: CURRENT_GLASS };
  }
  if (operation === 'full_contract_reset') {
    return { tone: CURRENT_GLASS, origin: ASSEMBLY_DRAFT_ORIGIN, authority: CHARGE_SPARK };
  }
  return { tone: ASSEMBLY_DRAFT, origin: ASSEMBLY_DRAFT_ORIGIN, authority: ASSEMBLY_DRAFT_MATERIAL };
}

function transitionCompatibility(
  definition: EngineeringRunTransitionPreview['definition'],
  address: string,
): EngineeringAssemblyCompatibilityDisposition | null {
  const issueAddresses = new Set([address]);
  if (address.startsWith('component:')) issueAddresses.add(`policy:${address}`);
  const issues = definition.compatibility_issues.filter((issue) => (
    issue.address !== null
    && (issueAddresses.has(issue.address)
      || address === 'physical_compartment:opening'
        && (issue.address.startsWith('assembly:') || issue.address.startsWith('generator:')))
  ));
  if (issues.some((issue) => issue.disposition !== 'assembly_adaptation')) {
    return 'hard_refusal';
  }
  if (issues.some((issue) => issue.disposition === 'assembly_adaptation')) {
    return 'adaptation_required';
  }
  const fields = definition.compatibility_fields.filter((field) => field.address === address);
  if (fields.some((field) => field.disposition === 'hard_refusal')) return 'hard_refusal';
  if (fields.some((field) => field.disposition === 'adaptation_required')) {
    return 'adaptation_required';
  }
  if (fields.some((field) => field.disposition === 'retained_by_address')) {
    return 'retained_by_address';
  }
  if (fields.some((field) => field.disposition === 'retained_unchanged')) {
    return 'retained_unchanged';
  }
  return null;
}

function coherentTransitionReceipt(
  preview: EngineeringRunTransitionPreview,
  receipt: EngineeringRunTransitionReceipt | null,
): receipt is EngineeringRunTransitionReceipt {
  if (
    !receipt
    || receipt.version !== 5
    || receipt.preview_id !== preview.preview_id
    || receipt.operation !== preview.definition.operation
    || receipt.after_generator_hash !== preview.definition.target_generator_hash
    || receipt.after_assembly_hash !== preview.definition.target_assembly_hash
    || receipt.reconstruction_digest !== preview.definition.reconstruction_digest
    || receipt.before_regime_id !== preview.definition.current_regime_id
    || receipt.before_scenario_hash !== preview.definition.guard.scenario_hash
    || receipt.after_regime_id !== preview.definition.target_regime_id
    || receipt.after_scenario_hash !== preview.definition.target_scenario_hash
    || !receipt.compatibility_fields
  ) return false;
  return JSON.stringify(receipt.compatibility_fields)
      === JSON.stringify(preview.definition.compatibility_fields)
    && JSON.stringify(receipt.compatibility_issues)
      === JSON.stringify(preview.definition.compatibility_issues)
    && JSON.stringify(receipt.identities) === JSON.stringify(preview.definition.identities)
    && JSON.stringify(receipt.registers) === JSON.stringify(preview.definition.registers)
    && JSON.stringify(receipt.source) === JSON.stringify(preview.definition.source);
}

function fillEngineeringTransition(
  scene: Scene,
  companion: EngineeringTransitionCompanion | null,
  origin: EngineeringTransitionOrigin | null,
  clock: number,
  place: Place,
  zoom: number,
  cameraLayer: number,
): void {
  if (!companion || !origin) return;
  if (
    companion.status === 'committed'
    && !coherentTransitionReceipt(companion.preview, companion.receipt)
  ) return;
  const definition = companion.preview.definition;
  const draft = definition.target_assembly_draft;
  const sequence = transitionSequence(companion.status, clock, scene.reducedMotion);
  const tones = transitionTones(definition.operation);
  scene.engineeringTransition.active = true;
  scene.engineeringTransition.operation = definition.operation;
  scene.engineeringTransition.stage = sequence.stage;
  scene.engineeringTransition.phase = sequence.phase;
  scene.engineeringTransition.tone = tones.tone;
  scene.engineeringTransition.originTone = tones.origin;
  scene.engineeringTransition.authorityTone = tones.authority;
  scene.engineeringTransition.commitAllowed = definition.commit_allowed;

  const origins = new Map(origin.components.map((component) => [component.node, component] as const));
  const targets = new Map(draft.components.map((component) => [component.node, component] as const));
  for (const component of draft.components) {
    const before = origins.get(component.node);
    const fromDepth = (before?.layer ?? component.layer) - cameraLayer;
    const depth = component.layer - cameraLayer;
    const from = place(before?.x ?? component.pos.x / 65_536, before?.y ?? component.pos.y / 65_536, fromDepth);
    const target = place(component.pos.x / 65_536, component.pos.y / 65_536, depth);
    const mark = scene.assemblyDrafts.next();
    mark.kind = 'component';
    mark.id = component.node;
    mark.x = lerp(from.x, target.x, sequence.movement);
    mark.y = lerp(from.y, target.y, sequence.movement);
    mark.fromX = from.x;
    mark.fromY = from.y;
    mark.radius = scenePortRadius(scene, component.node, Math.max(scene.dpr * 9, zoom * depthScale(depth) * 18));
    mark.fromRadius = scenePortRadius(scene, component.node, Math.max(scene.dpr * 6, mark.radius * 0.64));
    mark.displaced = Math.abs(from.x - target.x) > scene.dpr
      || Math.abs(from.y - target.y) > scene.dpr
      || fromDepth !== depth;
    mark.fields = ASSEMBLY_DRAFT_FIELD.position
      | ASSEMBLY_DRAFT_FIELD.layer
      | ASSEMBLY_DRAFT_FIELD.charge
      | ASSEMBLY_DRAFT_FIELD.interface;
    mark.beforeLevel = before?.charge ?? 0;
    mark.afterLevel = clamp01(component.q / (4096 * 65_536));
    mark.count = 0;
    mark.active = true;
    mark.open = component.open;
    mark.phase = sequence.phase;
    mark.tone = depthTone(sequence.stage === 'deenergize' ? tones.origin : tones.tone, depth);
    mark.originTone = depthTone(tones.origin, fromDepth);
    mark.alpha = sequence.alpha * depthAlpha(depth);
    mark.depth = depth;
    mark.companion = true;
    mark.compatibility = transitionCompatibility(definition, `component:${component.node}`);
  }

  const projectedRoutes = new Set<number>();
  for (const issue of definition.compatibility_issues) {
    if (!issue.address?.startsWith('route:')) continue;
    const routeId = Number(issue.address.slice('route:'.length));
    if (!Number.isInteger(routeId) || projectedRoutes.has(routeId)) continue;
    const route = scene.routes.items.slice(0, scene.routes.count)
      .find((candidate) => candidate.id === routeId);
    if (!route) continue;
    projectedRoutes.add(routeId);
    const at = pointAlong(route.points, 0.5);
    const mark = scene.assemblyDrafts.next();
    mark.kind = 'route';
    mark.id = routeId;
    mark.x = at.x;
    mark.y = at.y;
    mark.fromX = at.x;
    mark.fromY = at.y;
    mark.radius = Math.max(scene.dpr * 11, route.width * 1.8);
    mark.fromRadius = mark.radius;
    mark.displaced = false;
    mark.fields = 0;
    mark.beforeLevel = 0;
    mark.afterLevel = 0;
    mark.count = 0;
    mark.active = true;
    mark.open = true;
    mark.phase = sequence.phase;
    mark.tone = tones.tone;
    mark.originTone = tones.origin;
    mark.alpha = sequence.alpha * depthAlpha(route.depth);
    mark.depth = route.depth;
    mark.companion = true;
    mark.compatibility = transitionCompatibility(definition, issue.address);
  }

  const formOrigins = new Map(origin.forms.map((form) => [form.node, form] as const));
  for (const form of draft.forms) {
    const component = targets.get(form.node);
    const beforeComponent = origins.get(form.node);
    const depth = (component?.layer ?? 0) - cameraLayer;
    const fromDepth = (beforeComponent?.layer ?? component?.layer ?? 0) - cameraLayer;
    const target = component
      ? place(component.pos.x / 65_536, component.pos.y / 65_536, depth)
      : { x: scene.width / 2, y: scene.height / 2 };
    const from = beforeComponent
      ? place(beforeComponent.x, beforeComponent.y, fromDepth)
      : target;
    const mark = scene.assemblyDrafts.next();
    mark.kind = 'form';
    mark.id = form.node;
    mark.x = lerp(from.x, target.x, sequence.movement);
    mark.y = lerp(from.y, target.y, sequence.movement);
    mark.fromX = from.x;
    mark.fromY = from.y;
    mark.radius = sceneFormRadius(scene, form.node, Math.max(scene.dpr * 13, zoom * depthScale(depth) * 24));
    mark.fromRadius = mark.radius;
    mark.displaced = Math.abs(from.x - target.x) > scene.dpr || Math.abs(from.y - target.y) > scene.dpr;
    mark.fields = ASSEMBLY_DRAFT_FIELD.reserve | ASSEMBLY_DRAFT_FIELD.blanks;
    mark.beforeLevel = formOrigins.get(form.node)?.reserve ?? 0;
    mark.afterLevel = clamp01(form.reserve / (4096 * 65_536));
    mark.count = form.junction_blanks ?? 0;
    mark.active = true;
    mark.open = true;
    mark.phase = sequence.phase;
    mark.tone = depthTone(sequence.stage === 'deenergize' ? tones.origin : tones.authority, depth);
    mark.originTone = depthTone(tones.origin, fromDepth);
    mark.alpha = sequence.alpha * depthAlpha(depth);
    mark.depth = depth;
    mark.companion = true;
    mark.compatibility = transitionCompatibility(definition, `form:${form.node}`);
  }

  const materialOrigins = new Map(origin.materials.map((material) => [material.material, material] as const));
  for (const material of draft.materials) {
    const before = materialOrigins.get(material.material);
    const depth = material.layer - cameraLayer;
    const fromDepth = (before?.layer ?? material.layer) - cameraLayer;
    const from = place(before?.x ?? material.pos.x / 65_536, before?.y ?? material.pos.y / 65_536, fromDepth);
    const target = place(material.pos.x / 65_536, material.pos.y / 65_536, depth);
    const mark = scene.assemblyDrafts.next();
    mark.kind = 'material';
    mark.id = material.material;
    mark.x = lerp(from.x, target.x, sequence.movement);
    mark.y = lerp(from.y, target.y, sequence.movement);
    mark.fromX = from.x;
    mark.fromY = from.y;
    mark.radius = Math.max(scene.dpr * 10, zoom * depthScale(depth) * (12 + Math.min(8, material.amount / 256)));
    mark.fromRadius = Math.max(scene.dpr * 5, mark.radius * 0.55);
    mark.displaced = Math.abs(from.x - target.x) > scene.dpr
      || Math.abs(from.y - target.y) > scene.dpr
      || fromDepth !== depth;
    mark.fields = ASSEMBLY_DRAFT_FIELD.position | ASSEMBLY_DRAFT_FIELD.layer | ASSEMBLY_DRAFT_FIELD.amount;
    mark.beforeLevel = clamp01((before?.amount ?? 0) / 1024);
    mark.afterLevel = clamp01(material.amount / 1024);
    mark.count = Math.min(6, Math.ceil(Math.log2(material.amount + 1)));
    mark.active = true;
    mark.open = true;
    mark.phase = sequence.phase;
    mark.tone = depthTone(sequence.stage === 'deenergize' ? tones.origin : ASSEMBLY_DRAFT_MATERIAL, depth);
    mark.originTone = depthTone(tones.origin, fromDepth);
    mark.alpha = sequence.alpha * depthAlpha(depth);
    mark.depth = depth;
    mark.companion = true;
    mark.compatibility = transitionCompatibility(definition, `material:${material.material}`);
  }

  const currentOrigins = new Map(origin.currents.map((current) => [current.id, current] as const));
  for (const target of draft.currents) {
    const before = currentOrigins.get(target.current);
    const current = scene.currents.items.slice(0, scene.currents.count)
      .find((candidate) => candidate.id === target.current);
    const beforePhase = before?.phase ?? 0;
    const afterPhase = (target.phase % 1024) / 1024;
    const from = current ? pointAlong(current.points, beforePhase) : null;
    const at = current ? pointAlong(current.points, afterPhase) : null;
    const mark = scene.assemblyDrafts.next();
    mark.kind = 'current';
    mark.id = target.current;
    mark.fromX = from?.x ?? at?.x ?? scene.width / 2;
    mark.fromY = from?.y ?? at?.y ?? scene.height / 2;
    mark.x = lerp(mark.fromX, at?.x ?? mark.fromX, sequence.movement);
    mark.y = lerp(mark.fromY, at?.y ?? mark.fromY, sequence.movement);
    mark.radius = Math.max(scene.dpr * 12, (current?.width ?? scene.dpr * 5) * 0.72);
    mark.fromRadius = Math.max(scene.dpr * 5, mark.radius * 0.5);
    mark.displaced = beforePhase !== afterPhase;
    mark.fields = ASSEMBLY_DRAFT_FIELD.phase | ASSEMBLY_DRAFT_FIELD.active;
    mark.beforeLevel = beforePhase;
    mark.afterLevel = afterPhase;
    mark.count = 0;
    mark.active = target.active;
    mark.open = true;
    mark.phase = afterPhase;
    mark.tone = sequence.stage === 'deenergize' ? tones.origin : ASSEMBLY_DRAFT_CURRENT;
    mark.originTone = tones.origin;
    mark.alpha = sequence.alpha * depthAlpha(current?.depth ?? 0);
    mark.depth = current?.depth ?? 0;
    mark.companion = true;
    mark.compatibility = transitionCompatibility(definition, `current:${target.current}`);
  }

  const beforeCompartment = {
    members: origin.physicalCompartment.members,
    leak_per_exposed_contact_per_step: origin.physicalCompartment.leakage,
  } as unknown as AssemblyPayload;
  const afterCompartment = draft.physical_compartment as unknown as AssemblyPayload;
  fillAssemblyBoundary(
    scene,
    draft,
    beforeCompartment,
    afterCompartment,
    place,
    zoom,
    cameraLayer,
    true,
    sequence.alpha,
    true,
    transitionCompatibility(definition, 'physical_compartment:opening'),
  );
}

/** Closest point on one rendered polyline, used only for visual relationships. */
function closestOnPath(points: readonly number[], x: number, y: number): { x: number; y: number; distance: number } | null {
  if (points.length < 2) return null;
  if (points.length === 2) {
    return { x: points[0], y: points[1], distance: Math.hypot(x - points[0], y - points[1]) };
  }
  let closest = { x: points[0], y: points[1], distance: Infinity };
  for (let point = 2; point < points.length; point += 2) {
    const x1 = points[point - 2];
    const y1 = points[point - 1];
    const x2 = points[point];
    const y2 = points[point + 1];
    const dx = x2 - x1;
    const dy = y2 - y1;
    const span = dx * dx + dy * dy;
    const along = span === 0 ? 0 : clamp01(((x - x1) * dx + (y - y1) * dy) / span);
    const at = { x: x1 + dx * along, y: y1 + dy * along };
    const distance = Math.hypot(x - at.x, y - at.y);
    if (distance < closest.distance) closest = { ...at, distance };
  }
  return closest;
}

/**
 * Supply is legible as a directed transfer, not merely as a glowing band.
 * These links are derived from current and recipient geometry and never feed
 * back into delivery.
 */
function fillSupplyLinks(scene: Scene, next: FrameState): void {
  const rawByNode = new Map(next.ports.map((port) => [port.node, port]));
  for (let currentPlace = 0; currentPlace < scene.currents.count; currentPlace += 1) {
    const current = scene.currents.items[currentPlace];
    if (!current.active || !current.emitting || current.points.length < 2) continue;
    const capture = current.width / (current.bright ? 2.35 : 1.9);
    for (let portPlace = 0; portPlace < scene.ports.count; portPlace += 1) {
      const port = scene.ports.items[portPlace];
      const raw = rawByNode.get(port.node);
      if (!raw || raw.layer !== current.layer || raw.charge >= 65_535 || port.delivered <= 0) continue;
      const source = closestOnPath(current.points, port.x, port.y);
      if (!source || source.distance > capture) continue;
      const link = scene.supplyLinks.next();
      link.current = current.id;
      link.recipient = port.node;
      link.x1 = source.x;
      link.y1 = source.y;
      link.x2 = port.x;
      link.y2 = port.y;
      link.phase = scene.reducedMotion ? 0.65 : (scene.clock * 0.16 + port.node * 0.11) % 1;
      link.width = Math.max(0.8, scene.dpr * (current.bright ? 1.35 : 0.95));
      link.tone = current.bright ? CURRENT_GLASS : current.tone;
      link.alpha = current.alpha * (0.3 + 0.5 * port.delivered);
    }
  }
}

/** The distance between a Form and a projected mark. */
const markDistance = (form: FormMark, x: number, y: number): number => Math.hypot(x - form.x, y - form.y);

/**
 * Projects the authoritative aggregate Pulse preview back onto the concrete
 * objects its radius reaches. Exact totals remain the core's; target identity
 * is a render-only consequence of the same layer, distance, gate, and stock
 * readings already present in the frame.
 */
function fillCoupling(scene: Scene, next: FrameState, zoom: number): void {
  const preview = next.pulsePreview;
  const form = scene.forms.items.slice(0, scene.forms.count).find((held) => held.controlled);
  scene.coupling.active = Boolean(preview && form);
  if (!preview || !form) {
    scene.coupling.alpha = 0;
    return;
  }

  const spread = zoom * depthScale(form.depth);
  const radius = Math.max(1, (preview.radius / 65_536) * spread);
  const effectTotal = preview.gathered
    + preview.reserveReleased
    + preview.openedPorts
    + preview.displacedPressures;
  scene.coupling.connected = effectTotal > 0;
  scene.coupling.x = form.x;
  scene.coupling.y = form.y;
  scene.coupling.radius = radius;
  scene.coupling.charge = clamp01(((preview.radius / 65_536) - 8) / 184);
  scene.coupling.phase = scene.reducedMotion ? 0.25 : (scene.clock * 0.06) % 1;
  scene.coupling.tone = scene.coupling.connected ? ROUTE_PROPOSED : FORM_CONTROLLED_RING;
  scene.coupling.alpha = form.alpha * (scene.coupling.connected ? 0.92 : 0.62);

  const markByNode = new Map<number, PortMark>();
  for (let place = 0; place < scene.ports.count; place += 1) {
    markByNode.set(scene.ports.items[place].node, scene.ports.items[place]);
  }
  const rawForm = next.ports
    .filter((port) => port.kind === FORM_NODE_KIND && port.layer - scene.camera.layer === form.depth)
    .map((port) => ({ port, mark: markByNode.get(port.node) }))
    .filter((entry): entry is { port: FramePort; mark: PortMark } => Boolean(entry.mark))
    .sort((a, b) => markDistance(form, a.mark.x, a.mark.y) - markDistance(form, b.mark.x, b.mark.y))[0]?.port;
  let headroom = Math.max(0, 65_535 - (rawForm?.charge ?? 0));
  const exactOpenPorts = new Set(next.pulseOpenPorts);
  const targets = new Map<string, CouplingTargetMark>();

  const addTarget = (
    key: string,
    x: number,
    y: number,
    targetRadius: number,
    effect: number,
    tone: Tone,
  ): void => {
    let target = targets.get(key);
    if (!target) {
      target = scene.couplingTargets.next();
      target.x = x;
      target.y = y;
      target.radius = Math.max(5 * scene.dpr, targetRadius);
      target.effects = 0;
      target.phase = scene.coupling.phase;
      target.tone = tone;
      target.alpha = 0.9 * form.alpha;
      targets.set(key, target);
    }
    target.effects |= effect;
    target.tone = target.effects === effect ? tone : mixTone(target.tone, tone, 0.5);
  };

  for (const raw of next.ports) {
    const port = markByNode.get(raw.node);
    if (!port || port.depth !== form.depth || markDistance(form, port.x, port.y) > radius) continue;
    if (raw.kind !== FORM_NODE_KIND && raw.charge > 0 && headroom > 0) {
      const take = Math.min(Math.ceil(raw.charge / 4), headroom);
      if (take > 0) {
        addTarget(`node:${raw.node}`, port.x, port.y, port.radius * 2.2, COUPLING_EFFECT.gather, CHARGE_HIGH);
        headroom -= take;
      }
    }
    if (raw.kind === 0 && !raw.open && exactOpenPorts.has(raw.node)) {
      addTarget(`node:${raw.node}`, port.x, port.y, port.radius * 2.5, COUPLING_EFFECT.open, CURRENT_GLASS);
    }
  }

  for (const pressure of next.pressures) {
    if (pressure.queued || pressure.ordinal !== 4) continue;
    if (pressure.targetKind === 'node') {
      const port = markByNode.get(pressure.target);
      if (port && port.depth === form.depth && markDistance(form, port.x, port.y) <= radius) {
        addTarget(`node:${pressure.target}`, port.x, port.y, port.radius * 2.8, COUPLING_EFFECT.suppress, PRESSURE_TONES[4]);
      }
    } else if (pressure.targetKind === 'route') {
      const route = scene.routes.items.slice(0, scene.routes.count).find((held) => held.route === pressure.target);
      if (!route) continue;
      const reachesTail = markDistance(form, route.x1, route.y1) <= radius;
      const reachesHead = markDistance(form, route.x2, route.y2) <= radius;
      if (!reachesTail && !reachesHead) continue;
      addTarget(
        `route:${pressure.target}`,
        (route.x1 + route.x2) / 2,
        (route.y1 + route.y2) / 2,
        Math.max(8 * scene.dpr, route.width * 3),
        COUPLING_EFFECT.suppress,
        PRESSURE_TONES[4],
      );
    }
  }
}

/**
 * Soft particles, drifting along the currents and the Routes that carry flow.
 * Their positions are a function of the clock rather than of a store, so they
 * hold nothing between frames and stop dead when the time scale does.
 */
function fillParticles(scene: Scene, reduced: boolean): void {
  const perSegment = reduced ? Math.min(2, scene.quality.particlesPerSegment) : scene.quality.particlesPerSegment;
  const drift = reduced ? 0 : scene.clock * 0.07;

  for (let place = 0; place < scene.currents.count; place += 1) {
    const current = scene.currents.items[place];
    if (!current.active || !current.emitting) continue;
    const pairs = current.points.length / 2;
    for (let leg = 1; leg < pairs; leg += 1) {
      for (let index = 0; index < perSegment; index += 1) {
        if (scene.particles.count >= scene.particleLimit) return;
        const along = (index / perSegment + drift) % 1;
        const mark = scene.particles.next();
        mark.subject = 0;
        mark.id = 0;
        mark.x = lerp(current.points[(leg - 1) * 2], current.points[leg * 2], along);
        mark.y = lerp(current.points[(leg - 1) * 2 + 1], current.points[leg * 2 + 1], along);
        mark.radius = Math.max(1.2, current.width * (current.bright ? 0.12 : 0.09));
        mark.tone = current.bright ? CURRENT_GLASS : current.tone;
        mark.alpha = current.alpha * (current.bright ? 0.85 : 0.5) * Math.sin(Math.PI * along) ** 0.5;
      }
    }
  }

  for (let place = 0; place < scene.routes.count; place += 1) {
    const route = scene.routes.items[place];
    if (route.flow <= 0) continue;
    const count = Math.max(1, Math.round(perSegment * route.flow));
    for (let index = 0; index < count; index += 1) {
      if (scene.particles.count >= scene.particleLimit) return;
      const along = (index / count + drift * (0.4 + route.flow)) % 1;
      const mark = scene.particles.next();
      mark.subject = 0;
      mark.id = 0;
      mark.x = lerp(route.x1, route.x2, along);
      mark.y = lerp(route.y1, route.y2, along);
      mark.radius = Math.max(1.1, route.width * 0.72);
      mark.tone = route.status === 2 ? OVERLOAD : CHARGE_SPARK;
      mark.alpha = 0.58 + 0.42 * route.flow;
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
      if (scene.particles.count >= scene.particleLimit) break;
      const turn = ((index + 0.5) / shape.particles) * Math.PI * 2 + cue.b;
      const speck = scene.particles.next();
      speck.subject = 0;
      speck.id = 0;
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
function fillHandles(scene: Scene, zoom: number, tool: StillSurfaceTool): void {
  const presence = scene.still.presence;
  if (presence <= 0.001 || tool !== 'compartment') return;
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
    // The standing physical compartment carries the handles; its queued
    // proposal and the passive View are readings, not places to take hold of.
    if (boundary.proposed || boundary.role !== 'compartment') continue;
    // A vertex sits exactly where its member's own Port stands, so its handle
    // is pushed a little way outward from the middle of the shape — the same
    // reason a Route's handles sit in from its ends. Two handles in one place
    // is one handle a player cannot take hold of, and the two mean different
    // changes: the Port starts a connection and the vertex reshapes the
    // physical compartment.
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
  let localized = false;
  for (const pressure of next.pressures) {
    if (pressure.queued) continue;
    const reading = pressureLevel(pressure);
    if (reading <= level) continue;
    level = reading;
    tone = PRESSURE_TONES[pressure.ordinal % PRESSURE_TONES.length];
    crisis = pressure.stage === 'crisis';
    localized = pressure.targetKind !== 'none';
  }
  scene.rim.level = level;
  scene.rim.tone = tone;
  scene.rim.crisis = crisis;
  scene.rim.localized = localized;
  scene.rim.beat = crisis && !scene.reducedMotion ? 0.5 + 0.5 * Math.sin(scene.clock * 0.55) : 0;
}

/** How wide a Form's own ring is drawn, in device pixels. */
export function formRingWidth(mark: FormMark): number {
  return Math.max(1.8, mark.radius * 0.16);
}
