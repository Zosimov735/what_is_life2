/**
 * The Web Worker entry: the only place a WASM core instance exists.
 *
 * It owns the message envelopes, the fixed-step accumulator, and the `frame`
 * event, and translates all of it into the four core calls
 * `docs/field-framework/ARCHITECTURE.md` locks. The division is the document's:
 * the worker decides how wall time maps onto a step count, and the core owns
 * the state those steps produce. Persistence, the analysis job, and autosave
 * belong to the goals that own them.
 *
 * The worker sets no timer of its own. Steps run only when an `input_frame`
 * arrives, so a hidden tab stops stepping by construction.
 */

import init, { Core } from '../wasm-pkg/field_game_core.js';
import { contentBundle } from './content';
import {
  errorResponse,
  isCommandName,
  isCorrelationId,
  isEventName,
  PROTOCOL_VERSION,
  SAVE_VERSION,
  type CommandEnvelope,
  type ErrorEnvelope,
  type EventEnvelope,
  type EventName,
  type FrameEventBody,
  type InputFrame,
  type Payload,
  type ResponseEnvelope,
} from './protocol';

const scope = self as unknown as DedicatedWorkerGlobalScope;

/**
 * What the worker opens a core with: the versions it speaks, and the authored
 * content it imported. The bundle rides here because the locked WASM surface
 * has exactly four methods and this is the one that opens a session — nothing
 * is fetched, and `init_run`'s body keeps its three locked keys.
 */
function coreInit(): string {
  return JSON.stringify({
    content: contentBundle(),
    protocol: PROTOCOL_VERSION,
    save_version: SAVE_VERSION,
  });
}

/** Simulation rate: steps per second, exactly. */
const STEPS_PER_SECOND = 30;

/**
 * One step's worth of accumulator, in microseconds times 30 — so a second of
 * real time is exactly 30,000,000 units and 30 steps, with no rounding drift.
 */
const STEP_UNITS = 1_000_000;

/** At most this many catch-up steps run per rendered frame. */
const CATCH_UP_LIMIT = 6;

/** The widest step count the frame event's `u8` field carries. */
const MAX_REPORTED_STEPS = 255;

/** A gap longer than this is clamped, so a long one can never queue a burst. */
const MAX_GAP_US = 250_000;

/** The full time scale, Q0.16. */
const FULL_SCALE = 65_536;

/** How long a ramp takes, in microseconds of real time. Locked. */
const RAMP_US = 250_000;

/** Where the render snapshot's header carries the completed-step counter. */
const STEP_OFFSET = 8;

/** Where the render snapshot's header carries its flags. */
const FLAGS_OFFSET = 6;

/** Where the render snapshot's header carries the time scale. */
const SCALE_OFFSET = 12;

/** Where the render snapshot's header carries the mode. */
const MODE_OFFSET = 14;

/** The mode byte's own numbering, for the modes the accumulator reads. */
const MODE_RAMP_IN = 1;
const MODE_STILL = 2;
const MODE_RAMP_OUT = 3;
const MODE_SUSPENDED = 4;

/** The header flag for time the accumulator dropped rather than burst. */
const DROPPED_FLAG = 1 << 1;

/**
 * The three queued-change commands. A run that is still runs no step and does
 * not change mode, so nothing else would make the next frame carry a snapshot —
 * and the queue is drawn on the surface, as the previews the frame's own
 * sections hold. A successful one of these therefore marks the next frame as
 * one that carries its buffer, exactly as a mode change does.
 */
const QUEUE_COMMANDS: readonly string[] = ['queue_plan', 'undo_plan', 'commit_plan'];

/**
 * The commands that put a run somewhere other than where the scheduler last
 * left it, so the accumulator and the previous timestamp start over: opening a
 * run, importing one, and the two restores. Each lands in `running` with the
 * accumulator cleared, exactly as the restore contract locks.
 */
const RESCHEDULING: readonly string[] = [
  'init_run',
  'import_run',
  'restore_checkpoint',
  'recover_branch',
];

/** What a core call returns, before the envelope is put around it. */
type CoreAnswer = { ok: true; body: Payload } | { ok: false; error: ErrorEnvelope };

/** One event the core raised, before the envelope is put around it. */
interface CoreEvent {
  ev: string;
  step: number;
  body: Payload;
}

/** What the accumulator resolved one frame into. */
interface Schedule {
  body: Payload;
  dropped: boolean;
  remainderUs: number;
}

/** The correlation id of the most recently accepted command. */
let lastAccepted = 0;

/** The accumulator, in microseconds times 30. */
let accumulator = 0;

/** The previous frame's timestamp, and none before the first frame. */
let previousStamp: number | null = null;

/** The step counter and mode the most recent frame event reported. */
let lastStep = 0;
let lastMode = -1;

/**
 * Whether the queue of proposed changes has moved since the last frame carried
 * a snapshot. A still run runs no step, so this is what puts the preview of a
 * queued change in front of the renderer.
 */
let queueMoved = false;

/**
 * The time scale the most recent snapshot's header reported, Q0.16.
 *
 * The core owns the mode and the ramp that moves this; the header is where it
 * says so, and it is re-read after every frame, so nothing here is a second
 * copy of anything. What is held is the value at the start of the interval the
 * next frame covers, which is exactly what the accumulator needs to update
 * from the elapsed time before spending it.
 */
let lastScale = FULL_SCALE;

/**
 * Loads the module and opens a core. A version the core does not speak throws
 * here, so no session exists to answer with.
 */
const opened: Promise<Core> = init().then(() => {
  const core = new Core(coreInit());
  console.info(
    `field_game worker: core opened, protocol ${PROTOCOL_VERSION}, save version ${SAVE_VERSION}`,
  );
  return core;
});

scope.onmessage = (message: MessageEvent<unknown>) => {
  void answer(message.data);
};

/**
 * Answers one message. Exactly one response is posted per command, except for
 * a valid `input_frame`, which its `frame` event acknowledges instead.
 */
async function answer(message: unknown): Promise<void> {
  const fault = envelopeFault(message, lastAccepted);
  if (fault) {
    post(errorResponse(correlationOf(message), fault));
    return;
  }

  const command = message as CommandEnvelope;
  lastAccepted = command.id;

  try {
    const core = await opened;
    const frame = command.cmd === 'input_frame' ? (command.body as InputFrame) : null;
    const held = { accumulator, previousStamp };
    const schedule = frame ? resolve(frame) : null;
    const body = schedule ? schedule.body : (command.body ?? {});
    const answered = JSON.parse(core.command(command.cmd, JSON.stringify(body))) as CoreAnswer;

    if (!answered.ok) {
      // A refused frame runs nothing, so the time it was going to spend is
      // still owed: the accumulator goes back where it was.
      accumulator = held.accumulator;
      previousStamp = held.previousStamp;
      post(errorResponse(command.id, answered.error));
      return;
    }
    if (frame && schedule) {
      raiseFrame(core, frame, schedule, Number(answered.body.steps_run ?? 0));
      raiseCoreEvents(core);
      return;
    }
    if (RESCHEDULING.includes(command.cmd)) {
      restart(core);
    }
    if (QUEUE_COMMANDS.includes(command.cmd)) {
      queueMoved = true;
    }
    post({ v: PROTOCOL_VERSION, re: command.id, ok: true, body: answered.body });
    // Events follow the response their cause stands behind, which is the order
    // the document locks: a command answers first, then what it raised.
    raiseCoreEvents(core);
  } catch (cause) {
    // A trap or a defect never crosses the boundary as a thrown value, and the
    // worker never terminates itself.
    console.error('field_game worker: command failed', cause);
    post(
      errorResponse(command.id, {
        code: 'internal',
        message_key: 'notice.run_resumed',
        detail: null,
      }),
    );
  }
}

/**
 * Resolves one frame into the step count the core runs, by the locked
 * accumulator. Owed time past the catch-up limit is discarded and flagged,
 * never burst.
 */
function resolve(frame: InputFrame): Schedule {
  // The step count the shell supplied replaces the whole block: the timestamp
  // is ignored and exactly that many steps run.
  if (typeof frame.advance_steps === 'number') {
    return { body: frame, dropped: false, remainderUs: Math.floor(accumulator / STEPS_PER_SECOND) };
  }

  const stamp = typeof frame.t_us === 'number' ? frame.t_us : 0;
  const gap = previousStamp === null ? 0 : Math.min(Math.max(stamp - previousStamp, 0), MAX_GAP_US);
  previousStamp = stamp;

  if (frame.pause === true) {
    // The pause level suspends the run, and the accumulator clears on the edge
    // and on every mode change into a paused state. The previous timestamp
    // clears with it: the frame that releases the pause is the first frame of
    // the run again, and the first frame reads a gap of zero. Keeping the
    // stamp would hand the release frame the whole length of the pause, which
    // the clamp would turn into a full catch-up burst and a dropped flag —
    // time a suspended run never owed.
    accumulator = 0;
    previousStamp = null;
    return { body: { ...frame, advance_steps: 0 }, dropped: false, remainderUs: 0 };
  }

  accumulator += Math.floor((gap * STEPS_PER_SECOND * scaleOver(gap)) / FULL_SCALE);

  let ran = 0;
  while (accumulator >= STEP_UNITS && ran < CATCH_UP_LIMIT) {
    accumulator -= STEP_UNITS;
    ran += 1;
  }
  let dropped = false;
  if (accumulator >= STEP_UNITS) {
    accumulator = STEP_UNITS - 1;
    dropped = true;
  }

  return {
    body: { ...frame, advance_steps: ran },
    dropped,
    remainderUs: Math.floor(accumulator / STEPS_PER_SECOND),
  };
}

/**
 * The time scale this frame's elapsed time is spent at, Q0.16.
 *
 * The locked rule is that a ramp updates the scale from the real time elapsed
 * *before* the accumulator spends it, so the scale here is the one the header
 * reported carried forward over this frame's own gap at the ramp's own rate —
 * 65536 over 250,000 µs, falling on the way in and rising on the way out.
 * Reading the header rather than holding a ramp of its own is what keeps the
 * two ends of this together: the core owns the ramp, and every frame re-seeds
 * this from what the core says it has become.
 *
 * A frame that arrives while the run is not ramping is spent at the plain
 * reading: full while running, nothing at all while still.
 */
function scaleOver(gapUs: number): number {
  const moved = Math.floor((gapUs * FULL_SCALE) / RAMP_US);
  if (lastMode === MODE_RAMP_IN) return Math.max(0, lastScale - moved);
  if (lastMode === MODE_RAMP_OUT) return Math.min(FULL_SCALE, lastScale + moved);
  if (lastMode === MODE_STILL || lastMode === MODE_SUSPENDED) return 0;
  return FULL_SCALE;
}

/**
 * Raises the `frame` event that acknowledges a valid frame. The render
 * snapshot rides along exactly when a step ran, the mode changed, control moved
 * (a Handoff), or a queued-change command moved the plan queue — and it is the
 * one transferable that ever crosses the boundary.
 */
function raiseFrame(core: Core, frame: InputFrame, schedule: Schedule, ran: number): void {
  const view = core.frame_view();
  const step = readStep(view);
  const mode = view.length > MODE_OFFSET ? view[MODE_OFFSET] : -1;
  const changed = mode !== lastMode;
  lastStep = step;
  lastMode = mode;
  lastScale = readScale(view);
  // The accumulator clears on every mode change into `still` or `suspended`,
  // and with it the remainder this frame would have interpolated across: a run
  // that has stopped on purpose is not part way between two steps.
  let remainderUs = schedule.remainderUs;
  if (mode === MODE_STILL || mode === MODE_SUSPENDED) {
    accumulator = 0;
    remainderUs = 0;
  }
  // The dropped-time flag is the accumulator's, and the accumulator is the
  // worker's: the core cannot know how much owed time this frame discarded.
  if (schedule.dropped && view.length >= FLAGS_OFFSET + 2) {
    view[FLAGS_OFFSET] |= DROPPED_FLAG;
  }

  const body: FrameEventBody = {
    seq: frame.seq,
    // The event's field is a `u8`. The catch-up cap of 6 keeps a timed frame
    // far below it; a larger `advance_steps` batch saturates here, and the
    // core's own answer to the command carries the exact count.
    steps_run: Math.min(ran, MAX_REPORTED_STEPS),
    remainder_us: remainderUs,
    dropped: schedule.dropped,
  };
  // Control moved (a Handoff): the request rode this frame's `inspect` field
  // and the core answered it between steps, in `still`, where no step runs and
  // no mode changes — so without this condition the frame that moved the
  // `controlled` flag would carry no buffer and the renderer would draw the
  // Form control had left until the next thing to happen happened.
  const handoff = frame.inspect != null && frame.inspect.target === 'handoff';
  if (ran > 0 || changed || queueMoved || handoff) {
    queueMoved = false;
    body.buffer = view.buffer as ArrayBuffer;
    raise('frame', step, body, [body.buffer]);
    return;
  }
  raise('frame', step, body, []);
}

/**
 * Posts every event the core raised since the previous call, in the order their
 * causes occurred. The core owns which events a step raises; the worker only
 * puts the locked envelope around each one.
 */
function raiseCoreEvents(core: Core): void {
  let raised: CoreEvent[];
  try {
    raised = JSON.parse(core.take_events()) as CoreEvent[];
  } catch (cause) {
    console.error('field_game worker: the core events did not parse', cause);
    return;
  }
  for (const event of raised) {
    if (!isEventName(event.ev)) continue;
    raise(event.ev, event.step, event.body, []);
  }
}

/** Puts the scheduler back where a freshly opened run leaves it. */
function restart(core: Core): void {
  accumulator = 0;
  previousStamp = null;
  // A restore lands with the plan queue cleared, so a mark left by the run that
  // was open would put a buffer on a frame for a queue that no longer stands.
  queueMoved = false;
  const view = core.frame_view();
  lastStep = readStep(view);
  lastMode = view.length > MODE_OFFSET ? view[MODE_OFFSET] : -1;
  lastScale = readScale(view);
}

/** The completed-step counter, from the render snapshot's header. */
function readStep(view: Uint8Array): number {
  if (view.length < STEP_OFFSET + 4) return lastStep;
  return new DataView(view.buffer, view.byteOffset, view.byteLength).getUint32(STEP_OFFSET, true);
}

/**
 * The time scale, from the render snapshot's header.
 *
 * The header holds it in 16 bits, where full speed saturates at 65535; a
 * running run is read as the whole 65536 rather than as that truncation, so
 * the one place the buffer is lossy does not leak into the accumulator.
 */
function readScale(view: Uint8Array): number {
  if (view.length < SCALE_OFFSET + 2) return lastScale;
  const held = new DataView(view.buffer, view.byteOffset, view.byteLength).getUint16(
    SCALE_OFFSET,
    true,
  );
  return held === 65_535 ? FULL_SCALE : held;
}

/**
 * The locked reading of a malformed or misplaced message. A fault here is
 * answered before the core is consulted at all.
 */
function envelopeFault(message: unknown, accepted: number): ErrorEnvelope | null {
  if (typeof message !== 'object' || message === null) {
    return protocolFault('not_an_object');
  }
  const envelope = message as Partial<CommandEnvelope>;
  if (envelope.v !== PROTOCOL_VERSION) return protocolFault('version');
  if (!isCorrelationId(envelope.id) || envelope.id <= accepted) {
    return protocolFault('correlation');
  }
  if (!isCommandName(envelope.cmd)) return protocolFault('command');
  return null;
}

function protocolFault(reason: string): ErrorEnvelope {
  return { code: 'protocol', message_key: null, detail: { reason } };
}

/** The `id` to echo: the message's own when it is a `u32`, and 0 otherwise. */
function correlationOf(message: unknown): number {
  const id = (message as Partial<CommandEnvelope> | null)?.id;
  return isCorrelationId(id) ? id : 0;
}

function post(response: ResponseEnvelope): void {
  scope.postMessage(response);
}

function raise(name: EventName, step: number, body: Payload, transfer: Transferable[]): void {
  const event: EventEnvelope = { v: PROTOCOL_VERSION, ev: name, step, body };
  if (transfer.length > 0) {
    scope.postMessage(event, transfer);
    return;
  }
  scope.postMessage(event);
}
