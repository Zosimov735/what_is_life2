/**
 * The worker protocol, as `docs/field-framework/ARCHITECTURE.md` locks it.
 *
 * The command set (9) and the event set (7) are closed for version 1: nothing
 * else crosses the boundary between the shell and the worker. Beside the
 * message layer — the envelopes, the closed name sets, and the error envelope —
 * this file re-declares the public core types one to one with the Rust core, in
 * the order the document locks them. The types the goals that own them have not
 * reached yet are declared by those goals.
 */

/** The protocol version field, literally 1 everywhere in version 1. */
export const PROTOCOL_VERSION = 1;

/** The save version this build reads and writes. */
export const SAVE_VERSION = 1;

/** The nine commands the shell may send. */
export type CommandName =
  | 'init_run'
  | 'input_frame'
  | 'queue_plan'
  | 'undo_plan'
  | 'commit_plan'
  | 'restore_checkpoint'
  | 'recover_branch'
  | 'export_run'
  | 'import_run';

/** The seven events the worker may raise unsolicited. */
export type EventName =
  | 'frame'
  | 'objective_changed'
  | 'pressure_changed'
  | 'review_ready'
  | 'checkpoint_written'
  | 'chapter_changed'
  | 'run_completed';

/** The closed error code set. */
export type ErrorCode =
  | 'protocol'
  | 'state'
  | 'validation'
  | 'impulse'
  | 'capacity'
  | 'not_found'
  | 'save_corrupt'
  | 'save_version'
  | 'import_invalid'
  | 'content_invalid'
  | 'worker_restart'
  | 'internal';

/** Structured-clone-safe data: the only shape a message body may carry. */
export type Payload = Record<string, unknown>;

/**
 * A Q32.16 binary fixed-point quantity, raw: the value times 65536. Every
 * simulation quantity crosses as one of these, as a plain JSON integer, because
 * no floating-point number ever appears in a protocol body.
 */
export type Fx = number;

/** `Fx` restricted to [0, 65536]: the raw form of [0, 1]. */
export type Frac = number;

/** A position on a layer plane, raw per axis, inside [0, 4096) units. */
export interface Vec2 {
  x: Fx;
  y: Fx;
}

/** The closed Node-kind set of version 1. */
export type NodeKind = 'port' | 'reserve' | 'module' | 'form';

/** The eight starting Forms, as their machine ids. */
export type FormId =
  | 'thread'
  | 'ring'
  | 'relay'
  | 'vault'
  | 'lens'
  | 'knot'
  | 'wake'
  | 'chorus';

/**
 * The same eight, in the closed set's own order, for a surface that has to
 * offer them. The order is the set's and is not a ranking: nothing here, and
 * nothing that reads it, marks one Form as the one to take.
 */
export const FORM_IDS: readonly FormId[] = [
  'thread',
  'ring',
  'relay',
  'vault',
  'lens',
  'knot',
  'wake',
  'chorus',
];

/**
 * One layer and its three authored difficulty parameters: `drain` is Charge
 * removed per step by depth, `noise` distorts routes and forecasts, and `gain`
 * scales rewards. Lists ascending.
 */
export interface FieldLayer {
  layer: number;
  drain: Fx;
  noise: Frac;
  gain: Frac;
  current_ids: number[];
  port_ids: number[];
}

/** One Form. Every Form is also a Node, which `node` binds it to. */
export interface FormState {
  id: number;
  form: FormId;
  node: number;
  controlled: boolean;
  layer: number;
  pos: Vec2;
  /** In units per step. */
  vel: Vec2;
  charge: Fx;
  reserve: Fx;
  pulse_charge: Frac;
  focus: boolean;
  route_reach: Fx;
  /** What a Route this Form forms carries per step. */
  route_capacity: Fx;
  forecast_depth: number;
  /** The steering reach scale this Form was authored with, [16384, 262144]. */
  steer_scale: Frac;
  /** The station a linked Form holds, and the distance past which it stands separated. */
  link: { offset: Vec2; separation: Fx } | null;
  /** What a Form authoring a Trail leaves, and what a left entry does when it comes due. */
  trail: { period: number; delay: number; radius: Fx; magnitude: Fx } | null;
}

/** One deposited Trail entry, standing until the step it falls due on. */
export interface PendingTrail {
  form: number;
  layer: number;
  pos: Vec2;
  due: number;
  magnitude: Fx;
}

/** One current: an authored polyline flow, `strength` in Charge per step. */
export interface CurrentState {
  id: number;
  layer: number;
  path: Vec2[];
  width: Fx;
  strength: Fx;
  period: number;
  phase: number;
  bright: boolean;
  active: boolean;
}

/**
 * One Port, which is a Node of the Field. `q` is stored Charge for the completed
 * step, `upkeep_rate` is Charge per step the Node pays to keep participating,
 * and `capacity` is the overload threshold.
 */
export interface PortState {
  node: number;
  layer: number;
  pos: Vec2;
  kind: NodeKind;
  q: Fx;
  open: boolean;
  upkeep_rate: Fx;
  capacity: Fx;
}

/** One Route, directed tail to head. `capacity` caps the per-step flow. */
export interface RouteState {
  route: number;
  tail: number;
  head: number;
  capacity: Fx;
  flow: Fx;
  formed_step: number;
}

/**
 * The two Boundary lists — drawn most recent first, authored in authored order —
 * and the run's boundary-leakage parameter, the Charge fraction that escapes per
 * exposed link per step, in [0, 8192].
 */
export interface BoundaryState {
  drawn: { members: number[]; step: number }[];
  authored: { members: number[] }[];
  leak_frac: Frac;
}

/** The six pressures of the closed set, in ordinal order. */
export const PRESSURE_IDS = [
  'drain',
  'noise',
  'fracture',
  'flood',
  'interference',
  'drift',
] as const;

export type PressureId = (typeof PRESSURE_IDS)[number];

/** The four stages a pressure passes through, in order. */
export const PRESSURE_STAGES = ['signal', 'pressure', 'crisis', 'resolution'] as const;

export type PressureStage = (typeof PRESSURE_STAGES)[number];

/** What a pressure is aimed at. */
export interface PressureTarget {
  t: 'none' | 'node' | 'route' | 'layer';
  id: number | null;
}

/** The Pulse's pressed-back floor, read while the stage matches its record. */
export interface PressureDisplaced {
  stage: PressureStage;
  level: number;
}

/** Flood's held working target, null everywhere else. */
export interface PressureBound {
  t: 'none' | 'node' | 'route' | 'layer';
  id: number;
}

/** One staged pressure, exactly as ARCHITECTURE.md locks it. */
export interface PressureState {
  pressure: PressureId;
  stage: PressureStage;
  /** The stage machine's curve value for the completed step, raw Q0.16. */
  level: number;
  primary: boolean;
  queued: boolean;
  start_step: number;
  target: PressureTarget;
  displaced: PressureDisplaced | null;
  bound: PressureBound | null;
}

/** The `pressure_changed` body: the full list after the change. */
export interface PressureChanged {
  pressures: PressureState[];
}

/** The three closed surround rules. */
export type Surround = 'adjacent' | 'double' | 'whole';

/** The View tuple, unchanged from the framework. */
export interface ViewDeclaration {
  inside: number[];
  resolution: number;
  window: number;
  surround: Surround;
}

/** One stream position: a key and counter as fixed-width hex, and the half. */
export interface RngState {
  key: string;
  ctr: string;
  half: number;
}

/** The two kinds of record a run writes. */
export type RecordKind = 'anchor' | 'auto';

/**
 * Anchor metadata. The payload lives in the persistence record `save_key`
 * names, and `rng` is the trajectory position at the write, so a Quick Retry
 * restores the exact random state.
 */
export interface CheckpointState {
  anchor_id: number;
  step: number;
  chapter_index: number;
  objective_id: string;
  kind: RecordKind;
  save_key: string;
  rng: RngState;
  branch_nonce: number;
}

/** The four states an objective's `state` field carries. */
export type ObjectiveStage = 'hidden' | 'active' | 'complete' | 'failed_recoverable';

/**
 * The single visible objective's state. `id` is the objective's copy-catalog
 * key and is the empty string exactly while `state` is `hidden` and none has
 * been offered; at most one objective is active at a time.
 */
export interface ObjectiveState {
  id: string;
  state: ObjectiveStage;
  progress: Frac;
  target: Fx | null;
  started_step: number;
  completed_step: number | null;
}

/**
 * The tagged union of proposed changes, exactly five variants; `op` is the tag.
 *
 * Every variant costs one Impulse and nothing else spends Impulse at all. What
 * each is validated against is the core's — the preconditions are locked per
 * variant and checked against the base state with every earlier queued entry
 * applied — so nothing here validates anything: this is the shape the body
 * carries, one to one with the Rust union.
 */
export type PlanCommand =
  | { op: 'connect'; from: number; to: number }
  | { op: 'redirect'; route: number; end: 'tail' | 'head'; to: number }
  | { op: 'cut'; route: number }
  | { op: 'reshape_boundary'; members: number[] }
  | { op: 'set_focus'; slate_ordinal: number; position: number };

/** One entry of the queue, as every queued-change response carries it. */
export interface QueueEntry {
  /** Where it stands in the queue, from 0. */
  position: number;
  plan: PlanCommand;
  /** What this entry costs, in Impulse. One, per the locked table. */
  cost: number;
  /**
   * Whether another entry of the queue touches the same Route or proposes the
   * same endpoint pair. A conflict informs the display and invalidates nothing
   * by itself.
   */
  conflict: boolean;
}

/**
 * The queue of proposed changes: what stands in it, what it costs, and the
 * Impulse standing before and after it.
 *
 * `impulse_after` is the prediction the tray shows, and a commit spends exactly
 * `cost_total` — one number arrived at once, so what is displayed and what is
 * spent cannot drift.
 */
export interface QueueState {
  entries: QueueEntry[];
  cost_total: number;
  impulse: number;
  impulse_after: number;
}

/** The success body of `queue_plan`. */
export interface PlanQueued extends Payload {
  queue: QueueState;
}

/** The success body of `undo_plan`: the queue, and how many entries remain. */
export interface PlanUndone extends Payload {
  queue: QueueState;
  remaining: number;
}

/** The success body of `commit_plan`. */
export interface PlanCommitted extends Payload {
  applied: number;
  impulse: number;
  slate_ordinal: number;
}

/** The request body of `init_run`, in its two modes. */
export type InitRunBody =
  | { mode: 'new'; run_id: string; form: FormId }
  | { mode: 'restore'; save_key: string };

/** The success body of `init_run`, in either mode. */
export interface RunOpened extends Payload {
  protocol: number;
  save_version: number;
  run_id: string;
  branch_nonce: number;
  step: number;
  chapter_index: number;
  view: ViewDeclaration;
  content_hash: string;
  content_changed: boolean;
}

/** The success body both restores answer with. */
export interface RunRestored extends Payload {
  step: number;
  branch_nonce: number;
  view: ViewDeclaration;
}

/** The success body of `export_run`. */
export interface RunExported extends Payload {
  text: string;
  sha256: string;
  filename_hint: string;
}

/** The success body of `import_run`. */
export interface RunImported extends Payload {
  run_id: string;
  step: number;
  branch_nonce: number;
}

/**
 * One frame of input, one per rendered frame, shell to worker.
 *
 * `seq` strictly increases from 1. `t_us` is the shell's animation-frame
 * timestamp in whole microseconds, and is the only wall clock the simulation
 * ever sees. Steering is normalized to Q1.15, the same shape from pointer,
 * trackpad, and keyboard alike. `advance_steps` replaces the accumulator with
 * exactly that many steps, which is how a deterministic test asks for a step
 * count inside the closed command set.
 */
export interface InputFrame extends Payload {
  seq: number;
  t_us: number;
  steer_x: number;
  steer_y: number;
  pulse_held: boolean;
  pulse_release: boolean;
  wheel: number;
  depth_key: number;
  toggle_still: boolean;
  pause: boolean;
  inspect: InspectRequest | null;
  advance_steps: number | null;
}

/** The eight perturbation kinds, by the machine name the core reads. */
export type PerturbationKind =
  | 'boundary-severance'
  | 'route-removal'
  | 'component-substitution'
  | 'resolution-change'
  | 'window-change'
  | 'surround-change'
  | 'delayed-replay'
  | 'full-turnover';

export const PERTURBATION_KINDS: readonly PerturbationKind[] = [
  'boundary-severance',
  'route-removal',
  'component-substitution',
  'resolution-change',
  'window-change',
  'surround-change',
  'delayed-replay',
  'full-turnover',
];

/**
 * One optional inspection a frame asks for.
 *
 * `target` names what to read: the eight recorded-window coordinates, the whole
 * profile with the two the replays pay for, or one perturbation. `kind` is the
 * perturbation's own machine name and null for every other target; `parameter`
 * is the kind's parameter, or null for the kind's own resolved default. The
 * core answers one only while the run is `still` and ignores it otherwise,
 * which is what keeps ordinary play free of readings nobody asked for.
 *
 * `handoff` is the one target that changes the run rather than reading it: it
 * moves control to the Form `parameter` names, immediately and for no Impulse,
 * with `kind` null. ARCHITECTURE.md's `The Handoff` puts it here rather than in
 * the plan queue — the command set is closed and the queue's five variants are
 * SPEC-closed, and a Handoff changes no Route, no Boundary and no View. It
 * refuses on the `input_frame` error path: `not_found` for a Form the run does
 * not hold, `validation` for one the chapter marks un-controllable and for a
 * Handoff to the Form already controlled.
 */
export interface InspectRequest extends Payload {
  target: 'coordinates' | 'coordinates_full' | 'perturbation' | 'handoff';
  kind: PerturbationKind | null;
  parameter: number | null;
}

/**
 * A frame holding nothing down. Every declared field is present, because the
 * protocol has no absent-versus-null ambiguity.
 */
export function neutralFrame(seq: number, timestampUs: number): InputFrame {
  return {
    seq,
    t_us: timestampUs,
    steer_x: 0,
    steer_y: 0,
    pulse_held: false,
    pulse_release: false,
    wheel: 0,
    depth_key: 0,
    toggle_still: false,
    pause: false,
    inspect: null,
    advance_steps: null,
  };
}

/** The body of a `frame` event. `buffer` is the render snapshot, transferred. */
export interface FrameEventBody extends Payload {
  seq: number;
  steps_run: number;
  remainder_us: number;
  dropped: boolean;
  buffer?: ArrayBuffer;
}

/**
 * The one error shape used everywhere. `message_key` is a copy-catalog key for
 * a fault a player is shown and null for a developer-only one; `detail` is
 * machine-readable and is never shown to a player.
 */
export interface ErrorEnvelope {
  code: ErrorCode;
  message_key: string | null;
  detail: Payload | null;
}

/** Shell to worker. `id` is a per-worker-session counter, strictly increasing. */
export interface CommandEnvelope {
  v: number;
  id: number;
  cmd: CommandName;
  body: Payload;
}

/** Worker to shell, exactly one per command; `re` echoes the command's `id`. */
export type ResponseEnvelope =
  | { v: number; re: number; ok: true; body: Payload }
  | { v: number; re: number; ok: false; error: ErrorEnvelope };

/** Worker to shell, unsolicited. */
export interface EventEnvelope {
  v: number;
  ev: EventName;
  step: number;
  body: Payload;
}

/** The nine commands as a value, for membership tests. */
export const COMMAND_NAMES: readonly CommandName[] = [
  'init_run',
  'input_frame',
  'queue_plan',
  'undo_plan',
  'commit_plan',
  'restore_checkpoint',
  'recover_branch',
  'export_run',
  'import_run',
];

/** True when a value is one of the nine commands. */
export function isCommandName(value: unknown): value is CommandName {
  return typeof value === 'string' && (COMMAND_NAMES as readonly string[]).includes(value);
}

/** The seven events as a value, for membership tests. */
export const EVENT_NAMES: readonly EventName[] = [
  'frame',
  'objective_changed',
  'pressure_changed',
  'review_ready',
  'checkpoint_written',
  'chapter_changed',
  'run_completed',
];

/** True when a value is one of the seven events. */
export function isEventName(value: unknown): value is EventName {
  return typeof value === 'string' && (EVENT_NAMES as readonly string[]).includes(value);
}

/** The body of an `objective_changed` event. */
export interface ObjectiveChanged extends Payload {
  objective: ObjectiveState;
  previous_id: string | null;
}

/** The body of a `checkpoint_written` event. */
export interface CheckpointWritten extends Payload {
  anchor: CheckpointState;
}

/**
 * Where one candidate of a slate came from, and the detail that source
 * records: the drawn entry's position, the cluster's total weight, the
 * authored position, the total co-response count — and none for the standing
 * View and the three variants.
 */
export interface Provenance extends Payload {
  source:
    | 'standing'
    | 'finer'
    | 'coarser'
    | 'lateral'
    | 'drawn'
    | 'clusters'
    | 'authored'
    | 'responses';
  detail: number | null;
}

/**
 * One of the four privilege values a candidate carries.
 *
 * `value` is the number and `low`/`high` the confidence range that contains
 * it, all as raw `Frac` — the quantity times 65536. An unassigned value
 * carries no number and no range at all, and `reason` says why. The four are
 * never summed, averaged, weighted, or otherwise combined, here or anywhere a
 * reader of this type takes them.
 */
export interface PrivilegeValue extends Payload {
  value: number | null;
  low: number | null;
  high: number | null;
  samples: number;
  reason: string | null;
}

/** The four values, each standing on its own. */
export interface PrivilegeProfile extends Payload {
  scale_stability: PrivilegeValue;
  shared_failure: PrivilegeValue;
  cut_impact: PrivilegeValue;
  boundary_sufficiency: PrivilegeValue;
}

/**
 * One candidate of the standing slate.
 *
 * `position` is the assembly position, 1-based, and presentation order is
 * assembly order. `tier` is the rank the nondominance ranking gives it — 1 for
 * the nondominated set, r + 1 for the set that stands once tiers 1 through r
 * are removed — and 0 on a deficient slate, which is not compared at all.
 */
export interface SlateCandidate extends Payload {
  position: number;
  view: ViewDeclaration;
  provenance: Provenance[];
  tier: number;
  privilege: PrivilegeProfile;
  baseline: { deviations: (number | null)[] };
}

/**
 * The evaluation record, as the core serializes it. The shell reads the
 * ordinal a `set_focus` names, whether it is deficient, the candidates in
 * presentation order with their provenance, four values, and tier, and the
 * tolerance-sensitivity flag — and passes the rest through untouched.
 */
export interface CandidateSlate extends Payload {
  ordinal: number;
  step: number;
  candidates: SlateCandidate[];
  deficient: boolean;
  deficiency_reason: string | null;
  /** The declared window, and the one the clamp left of it. */
  window_declared: number;
  window_effective: number;
  /** Every ordered pair of positions where the first dominates the second. */
  dominance: { a: number; b: number }[];
  /** Whether either tolerance recomputation changed the nondominated set. */
  sensitivity: { flag: boolean; changed_at: string[] };
}

/**
 * One coordinate reading: a number, or no number at all and the stated reason.
 *
 * The unit is the coordinate's own, exactly as FRAMEWORK.md declares it — a
 * count, a raw `Frac`, or a raw `Fx` in distance units — and a reader never
 * combines one with another.
 */
export interface CoordinateReading extends Payload {
  value: number | null;
  reason: string | null;
}

/**
 * The ten-coordinate profile of one View at one step.
 *
 * **No coordinate is combined** with any other, with a privilege value, or with
 * anything else into a single overall value: the ten are reported separately or
 * not at all, here and in every reader of this type. `instruction_separation`
 * and `turnover_tolerance` are the two the replays pay for, and they stand null
 * until a `coordinates_full` request runs them.
 */
export interface CoordinateProfile extends Payload {
  view: ViewDeclaration;
  step: number;
  swap_range: CoordinateReading;
  self_support: CoordinateReading;
  throughput: {
    in_rate: number;
    out_rate: number;
    routes: { route: number; rate: number }[];
    shell: { node: number; rate: number }[];
  };
  upkeep_mix: { value: number[] | null; reason: string | null };
  reach: CoordinateReading;
  input_resolution: CoordinateReading;
  horizon: CoordinateReading;
  source_trace: CoordinateReading;
  instruction_separation: PrivilegeValue | null;
  turnover_tolerance: {
    value: number | null;
    reason: string | null;
    pairs: { phi: number; agreement: number }[] | null;
  } | null;
}

/**
 * One replayed sample of a perturbation, with the compact playback record it
 * retained.
 *
 * `series` is the replayed inside's stored-Charge series, one raw `Fx` per
 * replayed step. `base_series` and `base_deviation` stand beside it only for
 * `delayed-replay`, whose base is its own unshifted-schedule replay; for every
 * other kind they are null and the excess is taken against the shared baseline
 * of the same sample number.
 */
export interface PerturbationSample extends Payload {
  deviation: number | null;
  excess: number | null;
  series: number[];
  base_deviation: number | null;
  base_series: number[] | null;
}

/**
 * One perturbation result, as the core records it.
 *
 * `parameter` always carries the value the kind actually used — a defaulted
 * parameter is stored resolved — and is null only for the two kinds that take
 * none. Results are session-lived: they never enter a save payload, and what
 * reproduces one after a restore is `sigma` with the resolved parameter.
 */
export interface PerturbationResult extends Payload {
  view: ViewDeclaration;
  provenance: Provenance[];
  position: number;
  sigma: Payload;
  streams: string[];
  kind: PerturbationKind;
  parameter: number | null;
  tau: number;
  reading: PrivilegeValue;
  samples: PerturbationSample[];
  recomputed: Payload | null;
  step: number;
}

/**
 * The one short causal highlight a committed change leaves.
 *
 * `kind` is a perturbation machine name, or `evaluation` when the highlight
 * derives from the adopted candidate's evaluation record. The three values are
 * the largest excess deviation and its confidence range; the shell chooses the
 * catalog wording and shows no number at all.
 */
export interface EchoHighlight extends Payload {
  kind: PerturbationKind | 'evaluation';
  parameter: number | null;
  excess: number;
  low: number;
  high: number;
  target: { t: 'node' | 'route' | 'none'; id: number | null };
}

/** The body of a `review_ready` event: one of the four reviews. */
export interface ReviewReady extends Payload {
  review:
    | { kind: 'slate'; slate: CandidateSlate }
    | { kind: 'echo'; echo: EchoHighlight }
    | { kind: 'coordinates'; profile: CoordinateProfile }
    | { kind: 'perturbation'; result: PerturbationResult }
    | { kind: string };
}

/** The body of a `chapter_changed` event. */
export interface ChapterChanged extends Payload {
  chapter_index: number;
  title_key: string;
}

/**
 * The body of a `run_completed` event: the ending the campaign closed on.
 *
 * `ending_id` is the copy-catalog key the last chapter authored, so the shell
 * reads the wording from the catalog by the key the worker names and never
 * chooses one. `continuation_unlocked` is true exactly when the campaign the
 * run completed authored the whole closed set of chapters.
 */
export interface RunCompleted extends Payload {
  ending_id: string;
  chapter_index: number;
  continuation_unlocked: boolean;
}

/** True when a value is a `u32`, the width of `id` and `re`. */
export function isCorrelationId(value: unknown): value is number {
  return typeof value === 'number' && Number.isInteger(value) && value >= 0 && value <= 0xffffffff;
}

/** Builds the error response for a command that could not be served. */
export function errorResponse(re: number, error: ErrorEnvelope): ResponseEnvelope {
  return { v: PROTOCOL_VERSION, re, ok: false, error };
}
