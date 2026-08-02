/**
 * The shell's surfaces: the neutral notice while the worker opens, the Field
 * once it has answered, and the notice a resumed run surfaces after a worker
 * fault.
 *
 * The shell is chrome. It owns the notices, the surface element, and the
 * session behind it; the renderer owns everything drawn on that element and
 * reads nothing but the snapshots the worker sends.
 */

import { useEffect, useMemo, useState } from 'react';
import { ChapterReview } from './ChapterReview';
import { copy } from './copy';
import { FieldSurface } from './FieldSurface';
import { FormSelect } from './FormSelect';
import { Inspect, offeredPlayback } from './Inspect';
import { Objective } from './Objective';
import { PressureLine } from './PressureLine';
import { openSound, type Sound } from './sound';
import { StillTray } from './StillTray';
import type { StillTool } from './still-edits';
import {
  openCore,
  type CandidateSlate,
  type CoreClient,
  type FramePair,
  type QueueState,
} from './worker-client';
import type {
  ChapterChanged,
  CoordinateProfile,
  EchoHighlight,
  FormId,
  PerturbationResult,
  RunCompleted,
  ViewDeclaration,
} from '../../../worker/src/protocol';
import type { FrameState } from '../../../worker/src/frame-state';
import type { ObjectiveState, PressureState } from '../../../worker/src/protocol';

interface ShellProps {
  /**
   * How the worker session is started, on the Form the opening surface took.
   * Replaced in tests.
   */
  open?: (form: FormId) => CoreClient;
  /**
   * How the cues are sounded. Replaced in tests, and `null` opens none at all.
   *
   * The level it opens at is the locked `InputConfig` default, full, because
   * `input_config` does not cross the frame boundary yet — the same standing
   * carry-forward the trail intensity the renderer takes stands on. The goal
   * that adds the settings surface hands both their configured values.
   */
  sound?: (() => Sound) | null;
}

/**
 * Where a local preview reaches the session, so a whole cycle — open, run,
 * pause, export, restart, import, resume — can be driven from the page without
 * a control surface the player will never see. Developer diagnostics, and only
 * in a development build.
 */
const BRIDGE_HANDLE = 'field_game_bridge';

/**
 * The query marker a local preview draws the development stand-in Field with,
 * in place of the empty Field a run stands on until authored content arrives.
 * Development builds only: the branch that reads it, and the module it reaches,
 * are both dropped from a production build.
 */
const FIXTURE_MARKER = 'field_fixture';

/**
 * The catalog key a refusal names, and none for a developer-only fault. The
 * envelope carries it; the shell never chooses one.
 */
function noticeKey(cause: unknown): string | null {
  if (typeof cause !== 'object' || cause === null) return null;
  const held = cause as { ok?: boolean; error?: { message_key?: string | null } };
  if (held.ok !== false) return null;
  return held.error?.message_key ?? null;
}

/**
 * The candidate the authoritative passive View matches, 1-based, and 0 while
 * it matches none. Focusing never reads or mutates the causal plan queue.
 */
function focusedIn(view: ViewDeclaration | null, slate: CandidateSlate | null): number {
  if (!view || !slate) return 0;
  return (
    slate.candidates.find(
      (candidate) =>
        candidate.view.resolution === view.resolution &&
        candidate.view.window === view.window &&
        candidate.view.surround === view.surround &&
        candidate.view.inside.length === view.inside.length &&
        candidate.view.inside.every((node, place) => node === view.inside[place]),
    )?.position ?? 0
  );
}

/**
 * How a session is opened when the caller names no other way: the real worker,
 * on the Form the opening surface took.
 *
 * It stands here rather than as a default written into the parameter list
 * because the effect that opens a session names this function among the things
 * it watches. A function rebuilt on every render is a new thing to watch every
 * render, and the effect would end the run and open another one each time —
 * a whole worker and a whole `init_run` per redraw.
 */
function openSession(form: FormId): CoreClient {
  return openCore({ form });
}

export function App({ open = openSession, sound = openSound }: ShellProps) {
  /**
   * The Form the player took, and none before the opening surface is answered.
   *
   * Nothing opens a session before this stands: the choice is part of
   * `init_run`, so the first command a session sends already carries it and no
   * run is ever opened on a Form the player did not name.
   */
  const [chosen, setChosen] = useState<FormId | null>(null);
  const [client, setClient] = useState<CoreClient | null>(null);
  const [ready, setReady] = useState(false);
  const [recovering, setRecovering] = useState(false);
  const [fixture, setFixture] = useState<(() => FramePair) | null>(null);
  const [sounding, setSounding] = useState<Sound | null>(null);
  const [objective, setObjective] = useState<ObjectiveState | null>(null);
  const [pressures, setPressures] = useState<PressureState[]>([]);
  const [refused, setRefused] = useState<string | null>(null);
  const [mode, setMode] = useState<FrameState['header']['mode'] | null>(null);
  const [queue, setQueue] = useState<QueueState | null>(null);
  const [impulse, setImpulse] = useState(0);
  const [slate, setSlate] = useState<CandidateSlate | null>(null);
  const [view, setView] = useState<ViewDeclaration | null>(null);
  const [tool, setTool] = useState<StillTool>('view');
  // The two session-lived readings the shell holds: a coordinate profile is
  // taken only when the inspection surface asks for one, and an Echo arrives at
  // Still Mode exit after a committed change.
  const [profile, setProfile] = useState<CoordinateProfile | null>(null);
  const [perturbation, setPerturbation] = useState<PerturbationResult | null>(null);
  const [echo, setEcho] = useState<EchoHighlight | null>(null);
  // The campaign's own two: the chapter a transition closed, and the ending a
  // completed campaign reached. Both are the worker's reports, held here and
  // rendered from the catalog; no sequencing lives in the shell.
  const [review, setReview] = useState<ChapterChanged | null>(null);
  const [ending, setEnding] = useState<RunCompleted | null>(null);

  useEffect(() => {
    if (!chosen) return;
    const session = open(chosen);
    setClient(session);
    let watching = true;
    const unwatch = session.watch(() => {
      if (!watching) return;
      setRecovering(session.recovering());
      // One objective at a time, and the shell learns which from the worker
      // rather than from anything it holds itself. The staged pressures
      // arrive the same way, on their own event.
      setObjective(session.objective());
      setPressures(session.pressures());
      // The same for the Still Mode surface: the mode is the worker's, the
      // queue is the core's, and the Impulse rides the frame header. Nothing
      // here is derived from a key the shell saw a player press.
      setMode(session.mode());
      setQueue(session.queue());
      setImpulse(session.snapshot()?.header.impulse ?? 0);
      // The slate is the worker's too: it rides `review_ready` and the shell
      // holds the one last raised. So do the coordinate profile and the Echo,
      // which ride the same event and are held the same way.
      setSlate(session.slate());
      setView(session.view?.() ?? null);
      setProfile(session.profile());
      setPerturbation(session.perturbation());
      setEcho(session.echo());
      // The chapter that closed and the ending the campaign reached ride
      // `chapter_changed` and `run_completed`, exactly as the objective rides
      // its own event.
      setReview(session.review());
      setEnding(session.ending());
    });
    session.ready.then(
      () => {
        if (watching) setReady(true);
      },
      (cause: unknown) => {
        console.error('field_game shell: the worker session did not open', cause);
        // A session that never opened has one thing to say, and the error
        // envelope names which: `message_key` is a catalog key for a fault a
        // player is shown and null for a developer-only one. Content that does
        // not validate is the fault this goal makes reachable.
        const key = noticeKey(cause);
        if (watching && key) setRefused(key);
      },
    );
    if (import.meta.env.DEV) {
      (globalThis as Record<string, unknown>)[BRIDGE_HANDLE] = session;
    }
    return () => {
      watching = false;
      unwatch();
      if (import.meta.env.DEV) {
        delete (globalThis as Record<string, unknown>)[BRIDGE_HANDLE];
      }
      session.close();
    };
  }, [open, chosen]);

  // The sound stands for the life of the shell rather than of one surface: it
  // holds the audio context, and a context rebuilt with every surface would
  // ask the autoplay policy for a fresh gesture each time.
  useEffect(() => {
    if (!sound) return;
    const opened = sound();
    setSounding(opened);
    return () => {
      setSounding(null);
      opened.close();
    };
  }, [sound]);

  // A completed ranking is heard. It is the one cue the sound module cannot
  // read off a snapshot: a ranking is a record the worker raises rather than a
  // place on the Field, so the ordinal of the record the run stands under is
  // what is handed over, and the module sounds a fresh one once.
  useEffect(() => {
    sounding?.ranked(slate?.ordinal ?? null);
  }, [sounding, slate]);

  useEffect(() => {
    if (!import.meta.env.DEV || typeof window === 'undefined') return;
    if (!new URLSearchParams(window.location.search).has(FIXTURE_MARKER)) return;
    let wanted = true;
    void import('./dev-frames').then((stand) => {
      if (wanted) setFixture(() => stand.fixtureFrames);
    });
    return () => {
      wanted = false;
    };
  }, []);

  // `C` and `V` select the same two tools as the focusable buttons. Interactive
  // chrome keeps its own keys; these bindings apply only to the Field surface.
  useEffect(() => {
    if (mode !== 'still' || typeof window === 'undefined') return;
    const choose = (event: KeyboardEvent): void => {
      const target = event.target as HTMLElement | null;
      if (target?.closest('button, input, select, textarea, [contenteditable="true"]')) return;
      if (event.code === 'KeyC') setTool('compartment');
      else if (event.code === 'KeyV') setTool('view');
      else return;
      if (event.cancelable) event.preventDefault();
    };
    window.addEventListener('keydown', choose);
    return () => window.removeEventListener('keydown', choose);
  }, [mode]);

  // The playback offer is memoized on its two inputs, deliberately: the offer
  // is an object, the surface hands it to the renderer on identity, and the
  // renderer opens a fresh reading from its own start. An offer rebuilt every
  // render would restart the playback clock at the render rate and the loop
  // would never leave its first window step.
  const playback = useMemo(() => offeredPlayback(mode, perturbation), [mode, perturbation]);

  // The opening selection stands before there is a session at all, which is
  // what "before `init_run`" means: no worker is started, no run is opened, and
  // the first command the session ever sends carries the Form that was taken.
  if (!chosen) {
    return <FormSelect onChoose={setChosen} />;
  }
  if (refused) {
    return <p className="notice">{copy(refused)}</p>;
  }
  if (!ready || !client) {
    return <p className="notice">{copy('notice.preparing')}</p>;
  }
  return (
    <>
      <FieldSurface
        frames={fixture ?? client.frames}
        sound={sounding}
        queuePlan={client.queuePlan}
        undoPlan={client.undoPlan}
        tool={tool}
        setFocus={(slateOrdinal, position) => {
          void client.setFocus?.(slateOrdinal, position);
        }}
        slate={slate}
        focused={focusedIn(view, slate)}
        playback={playback}
      />
      <Objective objective={objective} />
      <PressureLine pressures={pressures} />
      <Inspect
        mode={mode}
        profile={profile}
        echo={echo}
        inspect={client.inspect}
        clearEcho={client.clearEcho}
        forms={client.snapshot()?.forms}
      />
      <StillTray
        mode={mode}
        queue={queue ?? client.queue()}
        impulse={impulse}
        slate={slate}
        view={view}
        focused={focusedIn(view, slate)}
        tool={tool}
        setTool={setTool}
        setFocus={(position) => {
          if (slate) void client.setFocus?.(slate.ordinal, position);
        }}
      />
      <ChapterReview review={review} ending={ending} clearReview={client.clearReview} />
      {recovering ? <p className="notice">{copy('notice.run_resumed')}</p> : null}
    </>
  );
}
