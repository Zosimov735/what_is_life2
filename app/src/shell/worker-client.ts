/**
 * The worker client: the shell's whole view of the simulation.
 *
 * Every fact the shell learns arrives through the protocol envelopes. It never
 * imports the generated module and never builds a core itself, so the import
 * graph the architecture locks holds: the shell reaches the worker package and
 * the copy catalog, and nothing else.
 *
 * What lives here is the shell's half of the boundary — correlation, the
 * animation-frame pump that turns wall time into one `InputFrame` per rendered
 * frame, the pause level the visibility rules send, and the restart recovery
 * contract. The worker owns the accumulator and the core owns the state those
 * frames produce.
 *
 * Restart recovery, as `docs/field-framework/ARCHITECTURE.md` locks it: the
 * shell detects the fault, terminates the worker, fails every in-flight command
 * locally with `worker_restart`, starts a fresh worker, restores the run, and
 * surfaces the catalog notice before resuming the pump. The record the fresh
 * worker restores from is the newest the shell holds, taken at the locked
 * autosave cadence, so at most one interval of unpaused play is lost.
 */

import {
  neutralFrame,
  PROTOCOL_VERSION,
  type CommandName,
  type CommandEnvelope,
  type ErrorEnvelope,
  type EventEnvelope,
  type FormId,
  type FrameEventBody,
  type InputFrame,
  type Payload,
  type PlanCommand,
  type QueueState,
  type ResponseEnvelope,
  type RunExported,
  type RunOpened,
} from '../../../worker/src/protocol';
import type {
  CandidateSlate,
  ChapterChanged,
  CoordinateProfile,
  EchoHighlight,
  InspectRequest,
  ObjectiveState,
  PerturbationResult,
  PressureState,
  ReviewReady,
  RunCompleted,
} from '../../../worker/src/protocol';
import { decodeFrameState, type FrameState } from '../../../worker/src/frame-state';
import { openDepth, type Depth } from './depth';
import { openPulse, type Pulse } from './pulse';
import { openSteering, type Steering } from './steering';
import { openStill, type Still } from './still';
import { openTelemetry, type Recorder, type Telemetry } from './telemetry';

/** The correlation id of the first command of a worker session. */
const HANDSHAKE_ID = 1;

/**
 * The query marker a local preview opens the development run with, in place of
 * the empty Field a new run stands on until authored content arrives. Reached
 * only in a development build; `dev-run.ts` says what it carries and why the
 * command that opens it is the one it is.
 */
const DEV_RUN_MARKER = 'field_run';

/** How long a pending command may go unanswered before the worker is faulted. */
const RESPONSE_LIMIT_MS = 2_000;

/** How many completed steps one autosave interval spans. */
const AUTOSAVE_STEPS = 900;

/** The catalog notice a resumed run surfaces. */
export const RESUMED_NOTICE = 'notice.run_resumed';

/** A queue with nothing in it, which is what the tray shows before one opens. */
const EMPTY_QUEUE: QueueState = {
  entries: [],
  cost_total: 0,
  impulse: 0,
  impulse_after: 0,
};

/** What the shell holds so a fresh worker can pick the run back up. */
interface Recovery {
  text: string;
  step: number;
}

/**
 * The two most recent snapshots and the fraction of a step between them.
 *
 * `alpha` is the timing contract's own: `remainder_us × 30 ÷ 1,000,000`, taken
 * from every `frame` event whether or not a step ran, so the surface advances
 * smoothly between two snapshots and lags the simulation by at most one step.
 */
export interface FramePair {
  previous: FrameState | null;
  next: FrameState | null;
  alpha: number;
}

export interface CoreClient {
  /** Settles once the worker has answered in protocol version 1. */
  ready: Promise<ResponseEnvelope>;
  /** Sends one command and settles with the response envelope. */
  command: (cmd: CommandName, body: Payload) => Promise<ResponseEnvelope>;
  /** The most recently decoded render snapshot, and none before the first. */
  snapshot: () => FrameState | null;
  /**
   * The pair the renderer draws between. The same object each call, refreshed
   * in place, so reading it once per rendered frame allocates nothing.
   */
  frames: () => FramePair;
  /** Holds or releases the pause level the next frame carries. */
  pause: (held: boolean) => void;
  /**
   * Sends one frame now, outside the pump, and settles when it is answered.
   * The frame carries the steering, the Pulse, and the depth gesture held at
   * the moment it is sent, and taking them advances the keyboard ramp and
   * consumes a pending release and a pending press exactly as an animation
   * frame would.
   */
  step: (steps?: number) => Promise<FrameEventBody | ErrorEnvelope>;
  /** The export file the shell is holding for a restart, and none before one. */
  held: () => Recovery | null;
  /** How many times this session has replaced its worker. */
  restarts: () => number;
  /** True while a faulted worker is being replaced and the run reopened. */
  recovering: () => boolean;
  /**
   * How many messages are still awaiting an answer: commands, frames, and the
   * correlations held for the refusal path. A diagnostic, and the one way a
   * test can see that the pump leaves nothing behind.
   */
  inflight: () => number;
  /** The catalog keys this session has surfaced, most recent last. */
  notices: () => string[];
  /**
   * The objective the run stands on, and none before one is offered. It
   * arrives on `objective_changed` and on nothing else: the frame carries the
   * objective's ordinal, and its wording is the shell's to read from the
   * catalog by the key the event names.
   */
  objective: () => ObjectiveState | null;
  /**
   * The staged pressures as the worker last told them, active and queued
   * together in the closed set's own order, and empty before the first
   * `pressure_changed`. The event carries the full list after every change —
   * a seat taken, a stage turned over, a removal — and a reopened run raises
   * it again, so a fresh session is told what stands.
   */
  pressures: () => PressureState[];
  /**
   * The mode the newest snapshot reports, and none before the first one. The
   * shell shows the Still Mode surface from this and from nothing else: the
   * mode is the worker's, and a shell holding one of its own could disagree
   * with it.
   */
  mode: () => FrameState['header']['mode'] | null;
  /**
   * The queue of proposed changes as the worker last reported it. It moves
   * only on a queued-change response, because those are the only messages that
   * carry one.
   */
  queue: () => QueueState;
  /**
   * The evaluation record the run stands under, and none before the first
   * slate is assembled.
   *
   * It arrives on `review_ready` and on nothing else: a slate record crosses
   * on demand rather than per frame, so the shell holds the one the worker
   * last raised. A restart drops it, because the record belongs to the session
   * that raised it and the fresh one assembles its own on the next entry into
   * Still Mode.
   */
  slate: () => CandidateSlate | null;
  /**
   * The coordinate profile the worker last answered with, and none until one is
   * asked for.
   *
   * Profiles are optional: nothing requests one on its own, so a session whose
   * player never opens the inspection surface never holds a reading and the
   * surface has nothing to show. It arrives on `review_ready` like every other
   * record and crosses on demand rather than per frame.
   */
  profile: () => CoordinateProfile | null;
  /** The perturbation result the worker last answered with. */
  perturbation: () => PerturbationResult | null;
  /**
   * The Echo the newest committed change left, and none before one.
   *
   * It arrives at Still Mode exit, when the ramp completes, which is where the
   * event ordering puts it: one committed change leaves one highlight.
   */
  echo: () => EchoHighlight | null;
  /** Forgets the standing Echo, which is what showing it once means. */
  clearEcho: () => void;
  /**
   * The chapter the run stands in, as the worker last reported it, and none
   * before the first `chapter_changed`.
   *
   * It arrives on that event and on nothing else. Which chapter follows which
   * is the core's own sequencing, and the shell holds what it was told rather
   * than a sequence of its own: no chapter rule lives here.
   */
  chapter: () => ChapterChanged | null;
  /**
   * The chapter that closed, and none until one has.
   *
   * It is the chapter the worker reported before the one the run stands in
   * now, and it is set only when the run moved forwards by exactly one — a
   * reopened run reports the chapter it is in without having closed one, and a
   * restore that moved backwards is not a transition either.
   */
  review: () => ChapterChanged | null;
  /** Forgets the standing chapter review, which is what showing it once means. */
  clearReview: () => void;
  /**
   * The ending the campaign closed on, and none before `run_completed`.
   *
   * `ending_id` is a copy-catalog key: the wording is the catalog's and which
   * key it is belongs to the chapter the campaign ended on.
   */
  ending: () => RunCompleted | null;
  /**
   * Asks for one inspection, carried by the next frame the pump sends.
   *
   * It is the only way a reading is ever taken: the core answers an inspect
   * request and nothing else, and only while the run is `still`.
   */
  inspect: (request: InspectRequest) => void;
  /**
   * Queues one proposed change, and settles with the response.
   *
   * The entry is validated by the core against the base state with every
   * earlier entry applied, so a refusal is an answer rather than something to
   * predict here: the shell sends what the player did and shows what came back.
   * Valid only while the run is `still`, exactly as the command is.
   */
  queuePlan: (plan: PlanCommand) => Promise<ResponseEnvelope>;
  /**
   * Takes the newest queued entry back, and settles with the response.
   *
   * It is the same `undo_plan` the second Escape spends, reached directly:
   * walking the candidates of a slate replaces the focus it proposed rather
   * than stacking a second one, and taking one back is how a replacement is
   * made out of the two commands the closed set has.
   */
  undoPlan: () => Promise<ResponseEnvelope>;
  /** The local telemetry marks this run has reached. */
  telemetry: () => Telemetry;
  /** Runs when a snapshot, a notice, or a restart changes what is shown. */
  watch: (observer: () => void) => () => void;
  /** Ends the worker session. */
  close: () => void;
}

export interface CoreOptions {
  /**
   * The Form the run opens on, which the player chose on the opening surface.
   *
   * It has no default. A session is opened on a Form — the choice is part of
   * `init_run` and part of what the run records — and a default here would be
   * a Form the game recommended by standing behind the option.
   */
  form: FormId;
  /** How a worker is started. Replaced in tests. */
  spawn?: () => Worker;
  /** Whether the animation-frame pump runs. Off in tests that drive frames. */
  pump?: boolean;
  /** Where the steering comes from. Replaced in tests. */
  steering?: Steering;
  /** Where the Pulse comes from. Replaced in tests. */
  pulse?: Pulse;
  /** Where the depth gesture comes from. Replaced in tests. */
  depth?: Depth;
  /** Where the Still Mode keys come from. Replaced in tests. */
  still?: Still;
  /** Where the local telemetry is recorded. Replaced in tests. */
  telemetry?: Recorder;
}

/** Starts a worker session on the chosen Form and sends the first command. */
export function openCore(options: CoreOptions): CoreClient {
  const form = options.form;
  const spawn = options.spawn ?? defaultWorker;
  const pumping = options.pump ?? true;
  // A source the caller supplied is the caller's to close; only one opened
  // here is closed with the session.
  const steering = options.steering ?? openSteering();
  const ownsSteering = options.steering === undefined;
  const pulse = options.pulse ?? openPulse();
  const ownsPulse = options.pulse === undefined;
  const depth = options.depth ?? openDepth();
  const ownsDepth = options.depth === undefined;
  const still = options.still ?? openStill();
  const ownsStill = options.still === undefined;
  // Local only, and never sent anywhere: the five firsts of the onboarding
  // contract, measured from the moment the session opens.
  const telemetry = options.telemetry ?? openTelemetry();

  /**
   * Lets go of every held input: the steering, the Pulse, and the depth
   * gesture with them.
   *
   * The locked focus-loss rule is one rule over every device, so the four
   * sources are cleared together and always in the same place. A held Pulse is
   * dropped rather than emitted — the frame that follows is neutral, and a
   * neutral frame carries no release — a half-made depth gesture is dropped
   * with it rather than finished on the window's return, and a toggle or an
   * intent not yet spent goes the same way: the frame that follows suspends
   * the run, and a command sent after it would be a command in a state that no
   * longer admits one.
   */
  function letGo(): void {
    steering.clear();
    pulse.clear();
    depth.clear();
    still.clear();
  }

  let worker = spawn();
  let nextId = HANDSHAKE_ID;
  let nextSeq = 1;
  let paused = false;
  let closed = false;
  let restarting: Promise<void> | null = null;
  let restarts = 0;
  let latest: FrameState | null = null;
  const pair: FramePair = { previous: null, next: null, alpha: 0 };
  let recovery: Recovery | null = null;
  let objective: ObjectiveState | null = null;
  /**
   * The staged pressures as the worker last told them, and none before the
   * first `pressure_changed`. The event carries the full list after every
   * change, so this is a replacement, never a merge.
   */
  let pressures: PressureState[] = [];
  /** The queue of proposed changes as the worker last reported it. */
  let queue: QueueState = EMPTY_QUEUE;
  /** The evaluation record the run stands under, and none before the first. */
  let slate: CandidateSlate | null = null;
  /**
   * The coordinate profile the worker last answered with, and none until one is
   * asked for. It is optional by construction: nothing here requests one, so a
   * shell that never opens the inspection surface never holds a reading.
   */
  let profile: CoordinateProfile | null = null;
  /** The perturbation result the worker last answered with. */
  let perturbation: PerturbationResult | null = null;
  /** The Echo the newest committed change left, and none before one. */
  let echo: EchoHighlight | null = null;
  /**
   * The chapter the run stands in, the chapter that closed to reach it, and
   * the ending the campaign reached. All three arrive on their own events; the
   * shell holds what it was told and derives no sequence of its own.
   */
  let chapter: ChapterChanged | null = null;
  let review: ChapterChanged | null = null;
  let ending: RunCompleted | null = null;
  /**
   * The inspection the next frame will carry, and none while none is asked for.
   *
   * An inspect request rides one `InputFrame` — the protocol has no command for
   * it — so a request made between frames waits for the next one the pump sends
   * and is spent by it. One at a time: a second request made before the first
   * is carried replaces it, because what a player asked for last is what they
   * are waiting to see.
   */
  let asked: InspectRequest | null = null;
  /** Whether the newest snapshot found the run paused in Still Mode. */
  let stilled = false;
  let capturedAt = -1;
  let pumpHandle: number | null = null;
  /**
   * The frame number of the one frame carrying a depth press that is waiting to
   * be answered, and none while none is.
   *
   * The core resolves depth only on a frame that executes a step, and about
   * half the frames at the render rate execute none, so a press is offered
   * until a frame that carried it comes back having run one. Which frame that
   * was is known here and nowhere else, because frame numbers are this
   * session's: the source is told the answer to its own offer, once, whether
   * that answer is a frame event, a refusal, or a worker that went away.
   */
  let offeredAt: number | null = null;

  const notices: string[] = [];
  const observers = new Set<() => void>();
  /** Commands awaiting their one response, by correlation id. */
  const pending = new Map<
    number,
    { settle: (answer: ResponseEnvelope) => void; timer: ReturnType<typeof setTimeout> }
  >();
  /** Frames awaiting their acknowledgement, by frame number. */
  const frames = new Map<
    number,
    {
      settle: (answer: FrameEventBody | ErrorEnvelope) => void;
      timer: ReturnType<typeof setTimeout>;
      id: number;
    }
  >();
  /** Which frame a correlation id belongs to, for the refusal path. */
  const frameIds = new Map<number, number>();

  const runId = newRunId();
  let opened = false;

  function announce(): void {
    for (const observer of observers) observer();
  }

  function settleFrame(seq: number, answer: FrameEventBody | ErrorEnvelope): void {
    // Every frame this session sends is answered exactly once, here: by its
    // frame event, by a refusal, or by the restart that gave up on it. That
    // makes this the one place the depth source can be told what became of the
    // press it offered — steps that ran consume it, anything else keeps it.
    if (offeredAt === seq) {
      offeredAt = null;
      depth.settle('steps_run' in answer ? answer.steps_run : 0);
    }
    const waiting = frames.get(seq);
    if (!waiting) return;
    clearTimeout(waiting.timer);
    frames.delete(seq);
    // The acknowledged path clears the correlation too. A frame answered by
    // its event never reaches the response branch, so nothing else would, and
    // a pump running at the render rate would grow the map without bound.
    frameIds.delete(waiting.id);
    waiting.settle(answer);
  }

  function receive(message: MessageEvent<ResponseEnvelope | EventEnvelope>): void {
    const data = message.data;
    if (!data || data.v !== PROTOCOL_VERSION) return;
    if ('re' in data) {
      // A valid frame is acknowledged by its frame event and never by a
      // response; a refused one still answers here.
      const seq = frameIds.get(data.re);
      if (seq !== undefined) {
        frameIds.delete(data.re);
        if (!data.ok) settleFrame(seq, data.error);
      }
      const waiting = pending.get(data.re);
      if (waiting) {
        clearTimeout(waiting.timer);
        pending.delete(data.re);
        waiting.settle(data);
      }
      // A trap or a defect inside the worker is a fault the shell recovers
      // from, not an answer it carries on from.
      if (opened && !data.ok && data.error.code === 'internal') void fault('internal');
      return;
    }
    if (data.ev !== 'frame') {
      // The other six events of the closed set. The objective is the one the
      // chrome shows; the rest belong to the goals that own them, and the
      // telemetry reads what it needs from every one of them.
      telemetry.event(data.ev, data.body);
      if (data.ev === 'objective_changed') {
        objective = (data.body as { objective?: ObjectiveState }).objective ?? null;
        announce();
      }
      if (data.ev === 'pressure_changed') {
        pressures = (data.body as { pressures?: PressureState[] }).pressures ?? [];
        announce();
      }
      if (data.ev === 'chapter_changed') {
        // A chapter closes when the run enters the one after it, and the shell
        // knows that from the two reports rather than from any rule of its own:
        // the chapter it was told about last, and the one it is being told
        // about now. A reopened run reports the chapter it stands in, and a
        // restore can report an earlier one, so only a step of exactly one
        // forwards leaves a chapter to review.
        const entered = data.body as ChapterChanged;
        if (chapter && entered.chapter_index === chapter.chapter_index + 1) {
          review = chapter;
        }
        chapter = entered;
        announce();
      }
      if (data.ev === 'run_completed') {
        ending = data.body as RunCompleted;
        announce();
      }
      if (data.ev === 'review_ready') {
        // Four reviews ride this event and the shell reads one of them: the
        // slate, which is what the still surface lists and what a `set_focus`
        // names a position in. The other three belong to the goals that own
        // them and pass through untouched.
        const review = (data.body as ReviewReady).review;
        if (review && review.kind === 'slate') {
          slate = (review as { slate: CandidateSlate }).slate;
          announce();
        }
        if (review && review.kind === 'coordinates') {
          profile = (review as { profile: CoordinateProfile }).profile;
          announce();
        }
        if (review && review.kind === 'perturbation') {
          perturbation = (review as { result: PerturbationResult }).result;
          announce();
        }
        if (review && review.kind === 'echo') {
          echo = (review as { echo: EchoHighlight }).echo;
          announce();
        }
      }
      return;
    }
    const body = data.body as FrameEventBody;
    if (body.buffer) {
      try {
        const decoded = decodeFrameState(body.buffer);
        // The pair the renderer draws between: the snapshot before this one and
        // this one. The first snapshot of a session stands as both.
        pair.previous = latest ?? decoded;
        pair.next = decoded;
        latest = decoded;
        telemetry.observe(decoded);
        // The held readings go with the pause they were taken in: a run back
        // in `running` has moved past the window they read, so the next pause
        // asks for its own. A suspension keeps them — a blur is not a decision
        // to stop reading the Field — and the Echo stands apart, because it is
        // shown after the exit and let go of by the surface that showed it.
        if (decoded.header.mode === 'running' && (profile !== null || perturbation !== null)) {
          profile = null;
          perturbation = null;
          announce();
        }
      } catch (cause) {
        console.error('field_game shell: a render snapshot did not decode', cause);
      }
    }
    // The interpolation fraction rides every frame event, whether or not a step
    // ran: between two steps it is what carries the surface forward.
    pair.alpha = Math.min(0.999_999, Math.max(0, (body.remainder_us * 30) / 1_000_000));
    settleFrame(body.seq, body);
    readMode();
    hold(body);
    announce();
    spendIntent();
  }

  /**
   * Reads the mode off the newest snapshot, and lets go of what belongs to the
   * side of the edge the run has just left.
   *
   * **On the way in**, every direct control: movement is disabled while the run
   * is paused, and letting go is what makes that true of what was already held
   * rather than only of what arrives afterwards. A Pulse charging as the ramp
   * completes is dropped rather than banked against the exit, exactly as the
   * focus-loss rule drops one, and a bracket press waiting on a step that will
   * never come is dropped with it.
   *
   * **On the way out**, every intent not yet spent. `commit_plan` and
   * `undo_plan` are valid in `still` and nowhere else, so an intent still
   * waiting as Still Mode is left has nowhere to go — and it does not merely
   * expire, it waits: a second Escape pressed to leave one inspection would be
   * spent the moment the next one opened, ejecting the player from a pause
   * they had just asked for. The toggle is deliberately not dropped with them,
   * because a toggle pressed as the mode changes is a reversal of the ramp
   * that is changing it.
   */
  function readMode(): void {
    const stillNow = latest?.header.mode === 'still';
    if (stillNow && !stilled) {
      steering.clear();
      pulse.clear();
      depth.clear();
    }
    if (!stillNow && stilled) still.dropIntents();
    stilled = stillNow;
  }

  /**
   * Spends one waiting intent, and only while the run is paused: `commit_plan`
   * and `undo_plan` are valid in `still` and nowhere else, so a key pressed
   * anywhere else is dropped rather than sent into a refusal.
   *
   * The second Escape is read here and nowhere else, and what makes it the
   * second one is that it removed nothing: an undo answers with what remains,
   * and an undo that found nothing to remove is the press that leaves Still
   * Mode rather than the press that empties it. The queue the shell was
   * holding is what tells the two apart — an Escape that took the last entry
   * away leaves `remaining: 0` too, and that press was an undo. Leaving is
   * asked for by the one thing that moves the mode: the toggle the next frame
   * carries.
   */
  function spendIntent(): void {
    if (latest?.header.mode !== 'still') return;
    const intent = still.takeIntent();
    if (!intent) return;
    if (intent === 'commit') {
      void command('commit_plan', {}).then((answer) => {
        if (!answer.ok) return;
        // A commit clears the queue and leaves Still Mode by the mode table's
        // own committed exit, so nothing here asks for the exit as well.
        queue = { ...EMPTY_QUEUE, impulse: Number(answer.body.impulse ?? 0) };
        announce();
      });
      return;
    }
    const standing = queue.entries.length;
    void command('undo_plan', {}).then((answer) => {
      if (!answer.ok) return;
      const body = answer.body as { queue?: QueueState; remaining?: number };
      if (body.queue) queue = body.queue;
      if (body.remaining === 0 && standing === 0) still.exit();
      announce();
    });
  }

  function wire(): void {
    worker.onmessage = receive as (message: MessageEvent<unknown>) => void;
    worker.onerror = () => {
      void fault('worker_error');
    };
  }

  function send(cmd: CommandName, body: Payload): Promise<ResponseEnvelope> {
    const id = nextId++;
    if (closed) return Promise.resolve(localFault(id, 'closed'));
    const envelope: CommandEnvelope = { v: PROTOCOL_VERSION, id, cmd, body };
    return new Promise((settle) => {
      const timer = setTimeout(() => {
        pending.delete(id);
        settle(localFault(id, 'no_response'));
        void fault('no_response');
      }, RESPONSE_LIMIT_MS);
      pending.set(id, { settle, timer });
      worker.postMessage(envelope);
    });
  }

  /**
   * Sends one frame. Its acknowledgement is the frame event, so no response is
   * waited on: the elision rule means a valid frame answers with an event and
   * an invalid one with an error carrying the same correlation id.
   */
  function sendFrame(frame: InputFrame): Promise<FrameEventBody | ErrorEnvelope> {
    const id = nextId++;
    if (closed) {
      return Promise.resolve({ code: 'worker_restart', message_key: RESUMED_NOTICE, detail: null });
    }
    frameIds.set(id, frame.seq);
    telemetry.input(frame);
    // A frame that carries a press is the frame whose answer decides it. Only a
    // frame that is actually sent is noted, so a press is never spent against a
    // frame nothing ever answered.
    if (frame.depth_key !== 0) offeredAt = frame.seq;
    const settled = new Promise<FrameEventBody | ErrorEnvelope>((settle) => {
      const timer = setTimeout(() => {
        frames.delete(frame.seq);
        frameIds.delete(id);
        settle({ code: 'worker_restart', message_key: RESUMED_NOTICE, detail: { reason: 'no_frame' } });
        void fault('no_frame');
      }, RESPONSE_LIMIT_MS);
      frames.set(frame.seq, { settle, timer, id });
    });
    const envelope: CommandEnvelope = {
      v: PROTOCOL_VERSION,
      id,
      cmd: 'input_frame',
      body: frame as unknown as Payload,
    };
    worker.postMessage(envelope);
    return settled;
  }

  /**
   * Keeps the newest export the shell can restore from, refreshed once per
   * autosave interval: the same cadence the worker's own records keep, so a
   * fault costs at most one interval of unpaused play.
   */
  function hold(body: FrameEventBody): void {
    const step = latest?.header.step ?? 0;
    const interval = Math.floor(step / AUTOSAVE_STEPS);
    if (body.steps_run === 0 && capturedAt >= 0) return;
    if (interval <= capturedAt) return;
    capturedAt = interval;
    void capture();
  }

  async function capture(): Promise<void> {
    const answer = await command('export_run', {});
    if (!answer.ok) return;
    const exported = answer.body as RunExported;
    recovery = { text: exported.text, step: latest?.header.step ?? 0 };
  }

  /**
   * The restart contract. Every in-flight command fails locally with
   * `worker_restart` — an unacknowledged command is never assumed applied —
   * and the fresh worker picks the run up from the newest record the shell
   * holds.
   */
  function fault(reason: string): Promise<void> {
    if (closed) return Promise.resolve();
    if (restarting) return restarting;
    restarting = (async () => {
      console.error(`field_game shell: the worker faulted (${reason}), replacing it`);
      announce();
      stopPump();
      worker.terminate();
      for (const [id, waiting] of pending) {
        clearTimeout(waiting.timer);
        waiting.settle(localFault(id, reason));
      }
      pending.clear();
      for (const seq of [...frames.keys()]) {
        settleFrame(seq, { code: 'worker_restart', message_key: RESUMED_NOTICE, detail: { reason } });
      }
      frameIds.clear();
      if (closed) {
        restarting = null;
        return;
      }

      worker = spawn();
      wire();
      // A worker session numbers its correlation ids and its frames from the
      // start; the fresh core has accepted neither. The offer a lost frame was
      // carrying was answered with the rest of them above, and the number it
      // stood under belongs to the session that is gone.
      nextId = HANDSHAKE_ID;
      nextSeq = 1;
      offeredAt = null;
      // The snapshots the faulted worker sent belong to the session that is
      // gone. The first snapshot of the fresh one stands as both halves of the
      // pair again, so nothing is interpolated across the join.
      latest = null;
      pair.previous = null;
      pair.next = null;
      pair.alpha = 0;
      // The objective the faulted worker had told the shell about belongs to
      // the session that is gone; the fresh one raises it again as it opens.
      objective = null;
      // So do the staged pressures: a reopened run raises the list again.
      pressures = [];
      // So does the mode it was in and the queue it was holding. A restore
      // lands in `running` with the queue cleared, which is the locked answer
      // to a record taken mid-`still`, so the shell starts from the same
      // place the fresh worker does rather than from what the old one said.
      queue = EMPTY_QUEUE;
      // And so does the slate it had raised: a fresh worker assembles its own
      // on the next entry into Still Mode, and a position in a record the
      // session that raised it no longer holds names nothing.
      slate = null;
      // The session-lived readings go with it, for the same reason: a
      // coordinate profile and a perturbation result belong to the session that
      // took them, and the fresh worker takes its own when it is asked to.
      profile = null;
      perturbation = null;
      echo = null;
      // And the chapter the faulted worker had reported: the fresh one raises
      // it again as it reopens the run. A review left standing across a restart
      // would name a boundary the resumed run has not crossed, and the ending
      // is let go of for the same reason — a resumed run has not ended.
      chapter = null;
      review = null;
      ending = null;
      asked = null;
      stilled = false;
      // The notice comes after the run is back, which is the order the
      // contract puts it in: replace, restore, surface, resume. It is
      // surfaced only when a run was actually resumed: a fresh run is not a
      // resumed one, and saying so would be a false claim about state the
      // player has lost. What the shell shows a player who lost a run
      // outright is the goal that owns persistence and its recovery surface.
      if (await reopen()) notices.push(RESUMED_NOTICE);
      restarts += 1;
      restarting = null;
      startPump();
      announce();
    })();
    return restarting;
  }

  /**
   * Opens the run on the fresh worker, and reports whether the run that was
   * running came back.
   *
   * The locked contract names `init_run` in restore mode over the newest valid
   * persistence record. Those records live in IndexedDB, which the goal that
   * owns persistence adds; until then the newest record the shell can name is
   * the export it is holding, and `import_run` is the command in the closed set
   * that restores exactly from one — same run key, same branch nonce, same
   * random state. With nothing held, or with a held record the fresh worker
   * refuses, a run is opened fresh — and that is a new run, not a resumed one.
   */
  async function reopen(): Promise<boolean> {
    if (recovery) {
      const answer = await send('import_run', { text: recovery.text });
      if (answer.ok) return true;
      console.error('field_game shell: the held record did not import', answer.error);
    }
    await send('init_run', { mode: 'new', run_id: runId, form });
    return false;
  }

  function command(cmd: CommandName, body: Payload): Promise<ResponseEnvelope> {
    if (restarting) return restarting.then(() => send(cmd, body));
    return send(cmd, body);
  }

  function nextFrame(overrides: Partial<InputFrame>, stamp: number): InputFrame {
    return { ...neutralFrame(nextSeq++, stamp), pause: paused, ...overrides };
  }

  /**
   * What the next frame carries: the toggle always, and the direct controls
   * only while the run is moving.
   *
   * A paused run reads none of the three direct sources at all, so a neutral
   * frame is what a still run receives however hard the keys are held — which
   * is the goal's own rule, and is enforced twice over, because the core runs
   * no step in `still` whatever a frame asks of it. Not sampling is the point:
   * a sample taken and thrown away would still advance the keyboard's ramp and
   * consume a release edge, and a source left half spent would come out of
   * Still Mode holding something the player let go of inside it.
   */
  function carried(): Partial<InputFrame> {
    // One request per frame, spent by the frame that carries it. The core
    // answers one only in `still`, so a request made as the mode leaves is
    // dropped by the run rather than held here.
    const inspect = asked;
    asked = null;
    if (latest?.header.mode === 'still') return { ...still.sample(), inspect };
    return {
      ...steering.sample(),
      ...pulse.sample(),
      ...depth.sample(),
      ...still.sample(),
      inspect,
    };
  }

  function startPump(): void {
    if (!pumping || closed || pumpHandle !== null) return;
    const tick = (timestamp: number): void => {
      pumpHandle = requestAnimationFrame(tick);
      if (restarting) return;
      // One sample per emitted frame from each source, and the only place a
      // ramp advances or a release edge is consumed: a frame that is not sent
      // moves nothing the shell holds.
      void sendFrame(nextFrame(carried(), Math.round(timestamp * 1000)));
    };
    pumpHandle = requestAnimationFrame(tick);
  }

  function stopPump(): void {
    if (pumpHandle === null) return;
    cancelAnimationFrame(pumpHandle);
    pumpHandle = null;
  }

  wire();
  // A development preview opens its run by importing the stand-in; every other
  // build opens a new one, and does it here rather than a microtask later, so
  // the first command a session sends is still sent as the session is made.
  // The stand-in is a record rather than a new run, so the Form it stands on is
  // the one its own bytes carry and the chosen Form reaches nothing.
  const ready = (
    import.meta.env.DEV && wantsStandInRun()
      ? import('./dev-run').then((stand) => send('import_run', { text: stand.DEV_RUN_EXPORT }))
      : send('init_run', { mode: 'new', run_id: runId, form })
  )
    .then((response) => {
      if (!answeredByCore(response)) throw response;
      opened = true;
      console.info(`field_game shell: worker handshake in protocol ${response.v}`, response);
      startPump();
      return response;
    });

  // A hidden tab or a blurred window clears every held input, sends one
  // neutral frame with the pause level, and stops the pump until it returns.
  // The clear comes first and runs whether or not the level moves, so a window
  // that blurs while already hidden still lets go of what it was holding; the
  // frame that follows is neutral because nothing is held to put in it.
  const onVisibility = (): void => {
    const hidden = typeof document !== 'undefined' && document.visibilityState === 'hidden';
    if (hidden) letGo();
    setPause(hidden);
  };
  const onBlur = (): void => {
    letGo();
    setPause(true);
  };
  const onFocus = (): void => setPause(false);

  function setPause(held: boolean): void {
    if (paused === held) return;
    paused = held;
    if (held) {
      stopPump();
      letGo();
      void sendFrame(nextFrame({ pause: true }, 0));
      return;
    }
    startPump();
  }

  if (typeof window !== 'undefined') {
    window.addEventListener('blur', onBlur);
    window.addEventListener('focus', onFocus);
    document.addEventListener('visibilitychange', onVisibility);
  }

  return {
    ready,
    command,
    snapshot: () => latest,
    frames: () => pair,
    pause: setPause,
    // One frame now, outside the pump, carrying what the shell holds: the
    // steering sample is taken here for the same reason the pause level is —
    // a frame carries what is held when it is sent, and this is a frame.
    step: (steps = 1) =>
      sendFrame(nextFrame({ ...carried(), advance_steps: steps }, performanceStamp())),
    held: () => recovery,
    restarts: () => restarts,
    recovering: () => restarting !== null,
    inflight: () => pending.size + frames.size + frameIds.size,
    notices: () => [...notices],
    objective: () => objective,
    pressures: () => pressures,
    mode: () => latest?.header.mode ?? null,
    queue: () => queue,
    slate: () => slate,
    profile: () => profile,
    perturbation: () => perturbation,
    echo: () => echo,
    clearEcho() {
      if (echo === null) return;
      echo = null;
      announce();
    },
    chapter: () => chapter,
    review: () => review,
    clearReview() {
      if (review === null) return;
      review = null;
      announce();
    },
    ending: () => ending,
    inspect(request) {
      asked = request;
    },
    queuePlan: (plan) =>
      command('queue_plan', { plan }).then((answer) => {
        if (answer.ok) {
          const held = (answer.body as { queue?: QueueState }).queue;
          if (held) queue = held;
          announce();
        }
        return answer;
      }),
    undoPlan: () =>
      command('undo_plan', {}).then((answer) => {
        if (answer.ok) {
          const held = (answer.body as { queue?: QueueState }).queue;
          if (held) queue = held;
          announce();
        }
        return answer;
      }),
    telemetry: () => telemetry.marks(),
    watch(observer) {
      observers.add(observer);
      return () => observers.delete(observer);
    },
    close() {
      closed = true;
      stopPump();
      if (ownsSteering) steering.close();
      if (ownsPulse) pulse.close();
      if (ownsDepth) depth.close();
      if (ownsStill) still.close();
      for (const waiting of pending.values()) clearTimeout(waiting.timer);
      pending.clear();
      for (const waiting of frames.values()) clearTimeout(waiting.timer);
      frames.clear();
      frameIds.clear();
      if (typeof window !== 'undefined') {
        window.removeEventListener('blur', onBlur);
        window.removeEventListener('focus', onFocus);
        document.removeEventListener('visibilitychange', onVisibility);
      }
      worker.terminate();
    },
  };
}

/**
 * Whether this preview was opened on the development run.
 *
 * The condition it stands beside is `import.meta.env.DEV`, so a production
 * build folds the whole branch away and drops the dynamic import and the module
 * it names with it. A build that shipped either would fail the production-build
 * check, which reads the emitted bundle for the marker and for the stand-in's
 * own bytes.
 */
function wantsStandInRun(): boolean {
  if (typeof window === 'undefined') return false;
  return new URLSearchParams(window.location.search).has(DEV_RUN_MARKER);
}

/** The default worker: the entry named by the path the bundler reads. */
function defaultWorker(): Worker {
  // The entry is named by the path it sits at, here and for the types above,
  // because the bundler has to read that path literally to make the worker its
  // own chunk. The package name would hide it.
  return new Worker(new URL('../../../worker/src/entry.ts', import.meta.url), {
    type: 'module',
  });
}

/** A refusal the shell answers with itself, for a command the worker lost. */
function localFault(re: number, reason: string): ResponseEnvelope {
  return {
    v: PROTOCOL_VERSION,
    re,
    ok: false,
    error: { code: 'worker_restart', message_key: RESUMED_NOTICE, detail: { reason } },
  };
}

function performanceStamp(): number {
  return typeof performance === 'undefined' ? 0 : Math.round(performance.now() * 1000);
}

/**
 * True for the one answer that means a run is loaded and the module behind it
 * opened. Every fault is a session that did not open: the envelope faults are
 * answered before the core is consulted, and the rest report that the core
 * never opened or refused the run. None of them may swap in a canvas that
 * nothing can ever draw on.
 */
function answeredByCore(response: ResponseEnvelope): boolean {
  return response.ok === true;
}

/**
 * The run key: 16 lowercase hex characters, and the only nondeterministic
 * input a run ever takes.
 */
function newRunId(): string {
  const bytes = new Uint8Array(8);
  crypto.getRandomValues(bytes);
  return Array.from(bytes, (byte) => byte.toString(16).padStart(2, '0')).join('');
}

export type { FrameState, RunOpened };
export type {
  CandidateSlate,
  PlanCommand,
  Provenance,
  QueueState,
  SlateCandidate,
} from '../../../worker/src/protocol';
