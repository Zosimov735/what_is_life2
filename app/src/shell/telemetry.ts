/**
 * Local telemetry: how long a run took to reach each of the five firsts.
 *
 * **It is local and it stays local.** The game performs no network request of
 * any kind after its own files load — `docs/field-framework/ARCHITECTURE.md`
 * locks that as offline operation, telemetry endpoints named among the things
 * that do not exist — so nothing here is sent anywhere, stored anywhere, or
 * read by anything but a developer looking at the session in front of them.
 * What it is for is the onboarding contract: time to first input, first Pulse,
 * first route, first collapse, and first Anchor.
 *
 * It is shell-side measurement rather than run state, and deliberately: the
 * save payload is exactly its twelve locked keys, and a counter inside it would
 * be a thirteenth. Each first is recorded twice over — the wall-clock
 * milliseconds since the run opened, which is what "time to" means, and the
 * completed step the simulation stood at, which is the figure that repeats
 * exactly for the same run key and the same frames.
 */

import type { FrameState } from '../../../worker/src/frame-state';

/** The five firsts, in the order the onboarding contract names them. */
export const TELEMETRY_MARKS = [
  'first_input',
  'first_pulse',
  'first_route',
  'first_collapse',
  'first_anchor',
] as const;

export type TelemetryMark = (typeof TELEMETRY_MARKS)[number];

/** When one mark was reached. */
export interface Mark {
  /** Milliseconds since the run opened. */
  ms: number;
  /** The completed step the simulation stood at. */
  step: number;
}

/** Every mark a run has reached, and none for one it has not. */
export type Telemetry = Partial<Record<TelemetryMark, Mark>>;

/** The cue kinds the marks are read from, in the closed set's own numbering. */
const CUE_PULSE_EMITTED = 1;

export interface Recorder {
  /** Notes that a frame carried something the player did. */
  input: (held: { steer_x: number; steer_y: number; pulse_held: boolean; pulse_release: boolean; depth_key: number }) => void;
  /** Reads one decoded snapshot for the marks it carries. */
  observe: (snapshot: FrameState) => void;
  /** Reads one worker event for the marks it carries. */
  event: (name: string, body: Record<string, unknown>) => void;
  /** Every mark reached so far. The same shape each call, copied. */
  marks: () => Telemetry;
  /** How many marks have been reached. */
  reached: () => number;
}

export interface RecorderOptions {
  /** Where the wall clock comes from. Replaced in tests. */
  now?: () => number;
  /** Runs once per mark, as it is reached. */
  onMark?: (mark: TelemetryMark, at: Mark) => void;
}

/** Opens a recorder. The clock starts when this is called: run open. */
export function openTelemetry(options: RecorderOptions = {}): Recorder {
  const now = options.now ?? defaultClock;
  const opened = now();
  const marks: Telemetry = {};
  let step = 0;

  function reach(mark: TelemetryMark): void {
    if (marks[mark]) return;
    const at: Mark = { ms: Math.max(0, Math.round(now() - opened)), step };
    marks[mark] = at;
    // A developer diagnostic, and the only place a mark is ever surfaced.
    console.info(`field_game telemetry: ${mark} at ${at.ms} ms, step ${at.step}`);
    options.onMark?.(mark, at);
  }

  return {
    input(held) {
      const moved =
        held.steer_x !== 0 ||
        held.steer_y !== 0 ||
        held.pulse_held ||
        held.pulse_release ||
        held.depth_key !== 0;
      if (moved) reach('first_input');
    },
    observe(snapshot) {
      step = snapshot.header.step;
      if (snapshot.cues.some((cue) => cue.kind === CUE_PULSE_EMITTED)) reach('first_pulse');
      if (snapshot.routes.some((route) => route.flow > 0)) reach('first_route');
    },
    event(name, body) {
      if (name === 'checkpoint_written') reach('first_anchor');
      if (name !== 'objective_changed') return;
      const objective = body.objective as { state?: string } | undefined;
      if (objective?.state === 'failed_recoverable') reach('first_collapse');
    },
    marks: () => ({ ...marks }),
    reached: () => Object.keys(marks).length,
  };
}

function defaultClock(): number {
  return typeof performance === 'undefined' ? 0 : performance.now();
}
