/**
 * The React render test: the loading surface is the catalog's notice, and the
 * field replaces it once the worker has answered — and the readiness gate that
 * decides when it has.
 */

import { act, cleanup, render, screen, type RenderResult } from '@testing-library/react';
import { readdirSync, readFileSync } from 'node:fs';
import path from 'node:path';
import type { ReactElement } from 'react';
import { afterEach, beforeEach, expect, inject, test, vi } from 'vitest';
import catalog from '../../content/copy/catalog.json';
import { FRAME_VERSION, type FrameState } from '../../worker/src/frame-state';
import type {
  CommandEnvelope,
  EventEnvelope,
  ObjectiveState,
  ResponseEnvelope,
} from '../../worker/src/protocol';
import { PROTOCOL_VERSION } from '../../worker/src/protocol';
import { App } from '../src/shell/App';
import { PLAY_SURFACE } from '../src/shell/depth';
import { openCore, type CoreClient } from '../src/shell/worker-client';

/**
 * A 2D context that answers everything and draws nothing. The test environment
 * carries no canvas implementation, so without one the surface the shell mounts
 * has no engine behind it and the renderer reports a fault that has nothing to
 * do with what these tests are about.
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
      return name in target ? target[name] : () => undefined;
    },
    set(target, property, value) {
      target[String(property)] = value;
      return true;
    },
  }) as unknown as CanvasRenderingContext2D;
}

beforeEach(() => {
  const quiet = quietContext();
  vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockImplementation(((kind: string) =>
    kind === '2d' ? quiet : null) as typeof HTMLCanvasElement.prototype.getContext);
});

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

/** A client that answers nothing, for the parts of the shell under test. */
function stubClient(ready: Promise<ResponseEnvelope>, close = () => {}): CoreClient {
  return {
    ready,
    command: async () => ({ v: PROTOCOL_VERSION, re: 0, ok: true, body: {} }),
    snapshot: () => null,
    frames: () => ({ previous: null, next: null, alpha: 0 }),
    pause: () => {},
    step: async () => ({ seq: 0, steps_run: 0, remainder_us: 0, dropped: false }),
    held: () => null,
    restarts: () => 0,
    recovering: () => false,
    inflight: () => 0,
    notices: () => [],
    objective: () => null,
    pressures: () => [],
    mode: () => null,
    queue: () => ({ entries: [], cost_total: 0, impulse: 0, impulse_after: 0 }),
    slate: () => null,
    view: () => null,
    profile: () => null,
    perturbation: () => null,
    echo: () => null,
    clearEcho: () => {},
    chapter: () => null,
    review: () => null,
    clearReview: () => {},
    ending: () => null,
    inspect: () => {},
    queuePlan: async () => ({ v: PROTOCOL_VERSION, re: 0, ok: true, body: {} }),
    setFocus: async () => ({ v: PROTOCOL_VERSION, re: 0, ok: true, body: {} }),
    undoPlan: async () => ({ v: PROTOCOL_VERSION, re: 0, ok: true, body: {} }),
    telemetry: () => ({}),
    watch: () => () => {},
    close,
  };
}

/** A client whose answer arrives when the test decides it does. */
function heldClient(): { client: CoreClient; answer: (response: ResponseEnvelope) => void } {
  let answer: (response: ResponseEnvelope) => void = () => {};
  const ready = new Promise<ResponseEnvelope>((settle) => {
    answer = settle;
  });
  return { client: stubClient(ready), answer };
}

/**
 * Renders the shell and takes the opening selection.
 *
 * The selection stands in front of every other surface, because a session is
 * opened on a Form: a test about anything else has to choose one first, and
 * the Form it chooses is named from the catalog rather than written here.
 */
function opened(element: ReactElement): RenderResult {
  const held = render(element);
  act(() => {
    screen.getByRole('radio', { name: new RegExp(catalog.entries['form.thread'].text) }).click();
  });
  return held;
}

test('the notice comes from the catalog and the field replaces it', async () => {
  const { client, answer } = heldClient();
  // Read from the catalog rather than written here: the check forbids inline
  // player-facing text in the shell, so a hardcoded duplicate would pass this
  // test while failing the workspace. Comparing against the authored bytes is
  // what makes the assertion mean the surface reads the catalog.
  const notice = catalog.entries['notice.preparing'].text;

  const { container } = opened(<App open={() => client} />);

  expect(screen.getByText(notice)).toBeTruthy();
  expect(container.querySelector('canvas')).toBeNull();

  await act(async () => {
    answer({
      v: PROTOCOL_VERSION,
      re: 1,
      ok: false,
      error: {
        code: 'content_invalid',
        message_key: 'notice.content_invalid',
        detail: null,
      },
    });
  });

  expect(container.querySelector('canvas')).not.toBeNull();
  expect(screen.queryByText(notice)).toBeNull();
  // The surface the shell mounts is the surface the depth source reads its
  // wheel events over. The two name it separately — one builds the element, the
  // other decides which events are the play surface's — so the naming is
  // asserted here rather than left to agree by habit.
  expect(container.querySelector('canvas')?.matches(PLAY_SURFACE)).toBe(true);
  expect(container.querySelector(PLAY_SURFACE)).toBe(container.querySelector('canvas'));
});

test('a worker that does not open leaves the notice standing', async () => {
  const notice = catalog.entries['notice.preparing'].text;
  const reported = vi.spyOn(console, 'error').mockImplementation(() => {});
  const client = stubClient(Promise.reject(new Error('no core')));

  const { container } = opened(<App open={() => client} />);
  await act(async () => {});

  expect(screen.getByText(notice)).toBeTruthy();
  expect(container.querySelector('canvas')).toBeNull();
  expect(reported).toHaveBeenCalledOnce();
  reported.mockRestore();
});

test('the worker session is ended when the shell goes away', () => {
  const { client } = heldClient();
  let ended = 0;
  const { unmount } = opened(<App open={() => ({ ...client, close: () => { ended += 1; } })} />);

  unmount();

  expect(ended).toBe(1);
});

test('the objective the worker reports reaches the surface', async () => {
  // The wiring this closes: the client's own `watch` fires, the shell reads
  // `objective()` from it, and the objective surface shows the catalog text for
  // the key the worker named. Nothing between them holds an objective of its
  // own.
  let announce: () => void = () => {};
  let standing: ObjectiveState | null = null;
  const client: CoreClient = {
    ...stubClient(Promise.resolve({ v: PROTOCOL_VERSION, re: 1, ok: true, body: {} })),
    objective: () => standing,
    watch: (observer) => {
      announce = observer;
      return () => {};
    },
  };
  const { container } = opened(<App open={() => client} sound={null} />);
  await act(async () => {});
  expect(container.querySelector('canvas')).toBeTruthy();

  // Nothing is shown before the worker offers one.
  expect(screen.queryByText(catalog.entries['objective.the_pull.follow_current'].text)).toBeNull();

  standing = {
    id: 'objective.the_pull.follow_current',
    state: 'active',
    progress: 0,
    target: null,
    started_step: 0,
    completed_step: null,
  };
  act(() => announce());
  await screen.findByText(catalog.entries['objective.the_pull.follow_current'].text);
  expect(screen.getByRole('button', { name: catalog.entries['label.why'].text })).toBeTruthy();

  // And the surface follows the worker to the next one, never stacking two.
  standing = { ...standing, id: 'objective.the_pull.open_port' };
  act(() => announce());
  await screen.findByText(catalog.entries['objective.the_pull.open_port'].text);
  expect(screen.queryByText(catalog.entries['objective.the_pull.follow_current'].text)).toBeNull();

  // The sequence ending clears the line: the hidden shape shows nothing.
  standing = { ...standing, id: '', state: 'hidden' };
  act(() => announce());
  expect(screen.queryByText(catalog.entries['objective.the_pull.open_port'].text)).toBeNull();
});

test('a session the core refuses surfaces the notice the envelope names', async () => {
  // Content that does not validate is the one refusal this goal makes
  // reachable, and its `message_key` is locked. The shell shows what the
  // envelope names and never chooses a notice of its own.
  const refused = {
    v: PROTOCOL_VERSION,
    re: 1,
    ok: false as const,
    error: { code: 'content_invalid' as const, message_key: 'notice.content_invalid', detail: null },
  };
  const client = stubClient(Promise.reject(refused));
  vi.spyOn(console, 'error').mockImplementation(() => {});
  opened(<App open={() => client} sound={null} />);
  await screen.findByText(catalog.entries['notice.content_invalid'].text);
});

test('a run being resumed after a worker fault surfaces the catalog notice', async () => {
  const { client, answer } = heldClient();
  let recovering = false;
  let announce: () => void = () => {};
  const watching: CoreClient = {
    ...client,
    recovering: () => recovering,
    watch: (observer) => {
      announce = observer;
      return () => {};
    },
  };

  const { container } = opened(<App open={() => watching} />);
  await act(async () => {
    answer({ v: PROTOCOL_VERSION, re: 1, ok: true, body: { protocol: PROTOCOL_VERSION } });
  });
  expect(container.querySelector('canvas')).not.toBeNull();
  const resumed = catalog.entries['notice.run_resumed'].text;
  expect(screen.queryByText(resumed)).toBeNull();

  await act(async () => {
    recovering = true;
    announce();
  });
  expect(screen.getByText(resumed)).toBeTruthy();

  await act(async () => {
    recovering = false;
    announce();
  });
  expect(screen.queryByText(resumed)).toBeNull();
});

/** A worker that records what it was sent and answers when the test says so. */
class RecordingWorker {
  static opened: RecordingWorker[] = [];
  onmessage: ((message: MessageEvent<ResponseEnvelope | EventEnvelope>) => void) | null = null;
  onerror: ((failure: unknown) => void) | null = null;
  readonly sent: CommandEnvelope[] = [];

  constructor() {
    RecordingWorker.opened.push(this);
  }

  postMessage(command: CommandEnvelope): void {
    this.sent.push(command);
  }

  terminate(): void {}

  answer(response: ResponseEnvelope): void {
    this.onmessage?.({ data: response } as MessageEvent<ResponseEnvelope>);
  }

  raise(event: EventEnvelope): void {
    this.onmessage?.({ data: event } as MessageEvent<EventEnvelope>);
  }
}

/** Opens a real client over a recorded worker and reports how `ready` settles. */
async function gate(response: ResponseEnvelope): Promise<'ready' | 'refused' | 'pending'> {
  RecordingWorker.opened.length = 0;
  vi.stubGlobal('Worker', RecordingWorker);
  vi.spyOn(console, 'info').mockImplementation(() => {});

  const client = openCore({ form: 'thread' });
  let outcome: 'ready' | 'refused' | 'pending' = 'pending';
  client.ready.then(
    () => {
      outcome = 'ready';
    },
    () => {
      outcome = 'refused';
    },
  );

  RecordingWorker.opened[0].answer(response);
  await new Promise((settle) => {
    setTimeout(settle, 0);
  });
  client.close();
  return outcome;
}

test('the first command is init_run, on the Form the session was opened with', () => {
  RecordingWorker.opened.length = 0;
  vi.stubGlobal('Worker', RecordingWorker);

  openCore({ form: 'vault' }).close();

  const [handshake] = RecordingWorker.opened[0].sent;
  expect(handshake.v).toBe(PROTOCOL_VERSION);
  expect(handshake.id).toBe(1);
  expect(handshake.cmd).toBe('init_run');
  expect(handshake.body.mode).toBe('new');
  expect(handshake.body.run_id).toMatch(/^[0-9a-f]{16}$/);
  // The choice is part of the first command a session ever sends, which is
  // what makes it part of the run rather than something applied to one.
  expect(handshake.body.form).toBe('vault');
});

test('setFocus is a top-level free command and the shell adopts only the returned View', async () => {
  RecordingWorker.opened.length = 0;
  vi.stubGlobal('Worker', RecordingWorker);
  const initial = { inside: [2, 3], resolution: 1, window: 45, surround: 'adjacent' as const };
  const selected = { inside: [3], resolution: 2, window: 30, surround: 'double' as const };
  const client = openCore({ form: 'thread', pump: false });
  const worker = RecordingWorker.opened[0];

  worker.answer({
    v: PROTOCOL_VERSION,
    re: 1,
    ok: true,
    body: { protocol: PROTOCOL_VERSION, view: initial },
  });
  await client.ready;
  expect(client.view?.()).toEqual(initial);

  const moved = client.setFocus?.(7, 2);
  expect(worker.sent[1]).toEqual({
    v: PROTOCOL_VERSION,
    id: 2,
    cmd: 'set_focus',
    body: { slate_ordinal: 7, position: 2 },
  });
  expect(client.queue().entries).toEqual([]);
  worker.answer({ v: PROTOCOL_VERSION, re: 2, ok: true, body: { view: selected } });
  await moved;
  expect(client.view?.()).toEqual(selected);
  expect(client.queue().entries).toEqual([]);
  client.close();
});

test('a chapter transition replaces the cached View and forgets a missing authoritative reading', async () => {
  RecordingWorker.opened.length = 0;
  vi.stubGlobal('Worker', RecordingWorker);
  const initial = { inside: [2, 3, 4], resolution: 1, window: 45, surround: 'adjacent' as const };
  const entered = { inside: [2, 3], resolution: 2, window: 30, surround: 'double' as const };
  const client = openCore({ form: 'thread', pump: false });
  const worker = RecordingWorker.opened[0];

  worker.answer({
    v: PROTOCOL_VERSION,
    re: 1,
    ok: true,
    body: { protocol: PROTOCOL_VERSION, view: initial },
  });
  await client.ready;
  worker.raise({
    v: PROTOCOL_VERSION,
    ev: 'chapter_changed',
    step: 0,
    body: { chapter_index: 0, title_key: 'title.the_pull', view: initial },
  });
  worker.raise({
    v: PROTOCOL_VERSION,
    ev: 'chapter_changed',
    step: 120,
    body: { chapter_index: 1, title_key: 'title.the_edge', view: entered },
  });
  expect(client.view?.()).toEqual(entered);

  // A transition from an older producer that omitted the new field may not
  // leave the previous chapter's View masquerading as the current one.
  worker.raise({
    v: PROTOCOL_VERSION,
    ev: 'chapter_changed',
    step: 240,
    body: { chapter_index: 2, title_key: 'title.the_loop' },
  });
  expect(client.view?.()).toBeNull();
  client.close();
});

test('readiness is a loaded run, and nothing short of one', async () => {
  // The one answer that means a run is loaded and the module behind it opened.
  expect(
    await gate({
      v: PROTOCOL_VERSION,
      re: 1,
      ok: true,
      body: { protocol: PROTOCOL_VERSION },
    }),
  ).toBe('ready');

  // Every fault is a session that did not open. A protocol fault is answered
  // before the core is ever consulted; an internal fault means it never
  // opened; the rest report a run the core refused. None of them is a loaded
  // core, so none may swap in a canvas that nothing can ever draw on — the
  // content fault included, which opening a new run can no longer produce.
  for (const code of ['protocol', 'internal', 'save_corrupt', 'content_invalid'] as const) {
    expect(await gate({ v: PROTOCOL_VERSION, re: 1, ok: false, error: { code, message_key: null, detail: null } })).toBe(
      'refused',
    );
  }
});

test('the surface hands the sound the same snapshots it draws', async () => {
  const { client, answer } = heldClient();
  const heard: number[] = [];
  const sound = {
    observe: (next: FrameState) => heard.push(next.header.step),
    ranked: () => {},
    setLevel: () => {},
    state: () => 'idle' as const,
    scheduled: () => 0,
    close: () => {},
  };

  // Animation frames the test hands out, so the surface's own loop runs once.
  const waiting: FrameRequestCallback[] = [];
  vi.stubGlobal('requestAnimationFrame', (run: FrameRequestCallback) => {
    waiting.push(run);
    return waiting.length;
  });
  vi.stubGlobal('cancelAnimationFrame', () => {});

  /** A snapshot with nothing in it but its own step: the renderer draws it. */
  const frame = (step: number): FrameState => ({
    header: {
      version: FRAME_VERSION,
      flags: 0,
      stillVisible: false,
      dropped: false,
      reducedMotion: false,
      step,
      timeScale: 65_535,
      mode: 'running',
      cameraLayer: 0,
      impulse: 3,
      chapterIndex: 0,
      objectiveOrdinal: 0,
      sectionCount: 0,
      leakPerExposedContactPerStep: 0,
    },
    forms: [],
    ports: [],
    routes: [],
    currents: [],
    inside: [],
    pressures: [],
    cues: [],
    camera: null,
    overlay: null,
  });
  const drawing: CoreClient = {
    ...client,
    frames: () => ({ previous: frame(4), next: frame(5), alpha: 0 }),
  };

  opened(<App open={() => drawing} sound={() => sound} />);
  await act(async () => {
    answer({ v: PROTOCOL_VERSION, re: 1, ok: true, body: { protocol: PROTOCOL_VERSION } });
  });
  await act(async () => {
    for (const run of waiting.splice(0)) run(0);
  });

  // What is heard and what is seen are the same frame, read from the same loop.
  expect(heard).toEqual([5]);
});

test('the queued-change tray stands only while the Still Mode surface is up', async () => {
  const { client, answer } = heldClient();
  // The tray's wording is read from the catalog rather than written here, for
  // the same reason the notice is: a hardcoded duplicate would pass this test
  // while failing the workspace check.
  const name = catalog.entries['label.still_mode'].text;
  const impulse = catalog.entries['label.impulse'].text;
  const queued = catalog.entries['label.queue'].text;

  let mode: FrameState['header']['mode'] | null = null;
  let watcher: () => void = () => {};
  const still: CoreClient = {
    ...client,
    mode: () => mode,
    queue: () => ({ entries: [], cost_total: 0, impulse: 3, impulse_after: 3 }),
    snapshot: () =>
      ({ header: { impulse: 3 } }) as unknown as ReturnType<CoreClient['snapshot']>,
    watch: (observer) => {
      watcher = observer;
      return () => {};
    },
  };

  const { container } = opened(<App open={() => still} />);
  await act(async () => {
    answer({ v: PROTOCOL_VERSION, re: 1, ok: true, body: { protocol: PROTOCOL_VERSION } });
  });
  expect(container.querySelector('.tray')).toBeNull();

  for (const held of ['ramp_in', 'still', 'ramp_out'] as const) {
    mode = held;
    await act(async () => watcher());
    expect(container.querySelector('.tray')?.getAttribute('data-mode')).toBe(held);
    expect(screen.getByText(name)).toBeTruthy();
    expect(screen.getByText(impulse)).toBeTruthy();
    expect(screen.getByText(queued)).toBeTruthy();
  }

  // The two explicit tools are real accessible controls. Their selected state
  // says which register a gesture affects without relying on colour.
  mode = 'still';
  await act(async () => watcher());
  const viewTool = screen.getByRole('button', {
    name: new RegExp(catalog.entries['label.observation_view'].text),
  });
  const compartmentTool = screen.getByRole('button', {
    name: new RegExp(catalog.entries['label.physical_compartment'].text),
  });
  expect(viewTool.getAttribute('aria-pressed')).toBe('true');
  expect(compartmentTool.getAttribute('aria-pressed')).toBe('false');
  act(() => compartmentTool.click());
  expect(viewTool.getAttribute('aria-pressed')).toBe('false');
  expect(compartmentTool.getAttribute('aria-pressed')).toBe('true');
  expect(container.querySelector('.tray[aria-hidden]')).toBeNull();
  const announced = container.querySelector('.tray-name');
  expect(announced?.getAttribute('role')).toBe('status');
  expect(announced?.getAttribute('aria-live')).toBe('polite');

  mode = 'running';
  await act(async () => watcher());
  expect(container.querySelector('.tray')).toBeNull();
});

test('the tray lists every queued entry with its own cost and the total', async () => {
  // What the tray predicts is what a commit spends, and the tray shows both
  // halves of that: one cost per entry, and the total the queue costs.
  const { client, answer } = heldClient();
  const cost = catalog.entries['label.cost'].text;
  let watcher: () => void = () => {};
  const still: CoreClient = {
    ...client,
    mode: () => 'still',
    queue: () => ({
      entries: [
        { position: 0, plan: { op: 'cut', route: 1 }, cost: 1, conflict: false },
        { position: 1, plan: { op: 'connect', from: 2, to: 3 }, cost: 1, conflict: true },
      ],
      cost_total: 2,
      impulse: 6,
      impulse_after: 4,
    }),
    snapshot: () =>
      ({ header: { impulse: 6 } }) as unknown as ReturnType<CoreClient['snapshot']>,
    watch: (observer) => {
      watcher = observer;
      return () => {};
    },
  };

  const { container } = opened(<App open={() => still} />);
  await act(async () => {
    answer({ v: PROTOCOL_VERSION, re: 1, ok: true, body: { protocol: PROTOCOL_VERSION } });
  });
  await act(async () => watcher());

  expect(screen.getByText(cost)).toBeTruthy();
  const total = container.querySelector('[data-total="cost"] .tray-value');
  expect(total?.textContent).toBe('2');
  const entries = [...container.querySelectorAll('.tray-entry')];
  expect(entries).toHaveLength(2);
  expect(entries.map((entry) => entry.querySelector('.tray-entry-cost')?.textContent)).toEqual([
    '1',
    '1',
  ]);
  // A conflict is carried on the entry that stands in one, and nothing about it
  // is written: the mark opens up, and the tray says the same thing to a reader
  // of the tree.
  expect(entries.map((entry) => entry.getAttribute('data-conflict'))).toEqual(['false', 'true']);
  expect(entries.map((entry) => entry.getAttribute('data-op'))).toEqual(['cut', 'connect']);
});

/** One assigned privilege value, as the record carries it. */
function reading(value: number, low: number, high: number) {
  return { value, low, high, samples: 8, reason: null };
}

/** One unassigned value: no number, no range, and the reason it carries. */
function absent(reason: string) {
  return { value: null, low: null, high: null, samples: 0, reason };
}

function profile(
  scale: ReturnType<typeof reading> | ReturnType<typeof absent>,
  shared: ReturnType<typeof reading> | ReturnType<typeof absent>,
) {
  return {
    scale_stability: scale,
    shared_failure: shared,
    cut_impact: reading(65536, 65536, 65536),
    boundary_sufficiency: reading(0, 0, 0),
  };
}

const NO_DEVIATIONS = { deviations: [0, 0, 0, 0, 0, 0, 0, 0] };

test('the tray lists candidate Views and moves the active View without queuing a causal edit', async () => {
  // A candidate is named by the source it came from, grouped under the tier the
  // ranking put it in, and read beside its four values as ranges — never as one
  // figure. The one the authoritative View matches is marked as active.
  const { client, answer } = heldClient();
  const heading = catalog.entries['label.candidates'].text;
  const standing = catalog.entries['label.candidate_standing'].text;
  const finer = catalog.entries['label.candidate_finer'].text;
  const drawn = catalog.entries['label.candidate_drawn'].text;

  const view = { inside: [2, 3], resolution: 1, window: 45, surround: 'adjacent' as const };
  const slate = {
    ordinal: 0,
    step: 12,
    deficient: false,
    deficiency_reason: null,
    window_declared: 45,
    window_effective: 45,
    dominance: [{ a: 1, b: 3 }],
    sensitivity: { flag: true, changed_at: ['double'] },
    candidates: [
      {
        position: 1,
        view,
        provenance: [{ source: 'standing' as const, detail: null }],
        tier: 1,
        privilege: profile(reading(65536, 49152, 65536), reading(32768, 32768, 32768)),
        baseline: NO_DEVIATIONS,
      },
      {
        position: 2,
        view: { ...view, inside: [3] },
        provenance: [{ source: 'finer' as const, detail: null }],
        tier: 1,
        privilege: profile(reading(32768, 16384, 49152), absent('few-samples')),
        baseline: NO_DEVIATIONS,
      },
      {
        position: 3,
        view: { ...view, inside: [5, 6] },
        provenance: [{ source: 'drawn' as const, detail: 1 }],
        tier: 2,
        privilege: profile(reading(0, 0, 0), absent('few-surround')),
        baseline: NO_DEVIATIONS,
      },
    ],
  };
  let watcher: () => void = () => {};
  let activeView = slate.candidates[2].view;
  const setFocus = vi.fn(async (slateOrdinal: number, position: number): Promise<ResponseEnvelope> => {
    activeView =
      position === 0
        ? { ...activeView, inside: [] }
        : slate.candidates[position - 1].view;
    return {
      v: PROTOCOL_VERSION,
      re: 2,
      ok: true,
      body: { view: activeView, slate_ordinal: slateOrdinal },
    };
  });
  const held: CoreClient = {
    ...client,
    mode: () => 'still',
    slate: () => slate,
    view: () => activeView,
    setFocus,
    queue: () => ({
      entries: [],
      cost_total: 0,
      impulse: 3,
      impulse_after: 3,
    }),
    snapshot: () =>
      ({ header: { impulse: 3 } }) as unknown as ReturnType<CoreClient['snapshot']>,
    watch: (observer) => {
      watcher = observer;
      return () => {};
    },
  };

  const { container } = opened(<App open={() => held} />);
  await act(async () => {
    answer({ v: PROTOCOL_VERSION, re: 1, ok: true, body: { protocol: PROTOCOL_VERSION } });
  });
  await act(async () => watcher());

  expect(screen.getByText(heading)).toBeTruthy();
  const listed = [...container.querySelectorAll('.tray-candidate')];
  expect(listed).toHaveLength(3);
  expect(listed.map((entry) => entry.querySelector('.tray-candidate-name')?.textContent)).toEqual([
    standing,
    finer,
    drawn,
  ]);
  // The candidate the passive View matches is the one marked, and only that one.
  expect(listed.map((entry) => entry.getAttribute('data-focused'))).toEqual([
    'false',
    'false',
    'true',
  ]);
  // Presentation order is assembly order, kept inside each tier, and the tiers
  // group the list ascending.
  expect(listed.map((entry) => entry.getAttribute('data-position'))).toEqual(['1', '2', '3']);
  expect(listed.map((entry) => entry.getAttribute('data-tier'))).toEqual(['1', '1', '2']);
  expect(
    [...container.querySelectorAll('.tray-candidates')].map((group) =>
      group.getAttribute('data-tier'),
    ),
  ).toEqual(['1', '2']);

  // Four readings stand beside every candidate, each drawn as the confidence
  // range the comparison reads, and never one bar for the four together.
  const first = listed[0].querySelectorAll('.tray-value-row');
  expect(first).toHaveLength(4);
  expect([...first].map((row) => row.getAttribute('data-assigned'))).toEqual([
    'true',
    'true',
    'true',
    'true',
  ]);
  const span = listed[0].querySelector('.tray-range') as HTMLElement;
  expect(span.getAttribute('data-low')).toBe('49152');
  expect(span.getAttribute('data-high')).toBe('65536');

  // An unassigned value is an honest absence: no bar, no zero, a mark and the
  // reason the record stated.
  const missing = listed[1].querySelectorAll('.tray-value-row')[1];
  expect(missing.getAttribute('data-assigned')).toBe('false');
  expect(missing.querySelector('.tray-range')?.getAttribute('data-reason')).toBe('few-samples');
  expect(missing.querySelector('.tray-value-absent')?.textContent).toBe(
    catalog.entries['label.value_unassigned'].text,
  );

  // No composite anywhere: every candidate renders exactly the four value
  // rows, one per privilege value, and no fifth reading of any kind — no
  // total, no average, no single bar for the four. The tier's own number
  // stands only on the group heading, never inside a candidate row.
  for (const entry of listed) {
    expect(entry.querySelectorAll('.tray-value-row')).toHaveLength(4);
    expect(entry.querySelector('.tray-value')).toBeNull();
  }

  // The tolerance-sensitivity flag and the accessible candidate controls.
  const flagged = container.querySelector('.tray-sensitivity');
  expect(flagged?.textContent).toBe(catalog.entries['label.sensitivity'].text);
  const candidateButtons = container.querySelectorAll('.tray-candidate-select');
  expect(candidateButtons).toHaveLength(3);
  expect(candidateButtons[2].getAttribute('aria-pressed')).toBe('true');

  // Clearing is an explicit accessible View action and remains outside the
  // causal queue. Position 0 is the protocol's passive clear operation.
  const clear = screen.getByRole('button', {
    name: catalog.entries['label.clear_view'].text,
  }) as HTMLButtonElement;
  expect(clear.disabled).toBe(false);
  await act(async () => clear.click());
  expect(setFocus).toHaveBeenLastCalledWith(0, 0);
  expect(held.queue().entries).toEqual([]);
  await act(async () => watcher());
  expect(clear.disabled).toBe(true);

  await act(async () => {
    (candidateButtons[1] as HTMLButtonElement).click();
  });
  expect(setFocus).toHaveBeenLastCalledWith(0, 2);
  expect(held.queue().entries).toEqual([]);
  await act(async () => watcher());
  expect(candidateButtons[1].getAttribute('aria-pressed')).toBe('true');
  expect(candidateButtons[2].getAttribute('aria-pressed')).toBe('false');

  // The View announcement: the active candidate is said when it changes,
  // under the same status discipline as the mode's own name.
  const announced = container.querySelector('.tray-focus');
  expect(announced?.getAttribute('role')).toBe('status');
  expect(announced?.getAttribute('aria-live')).toBe('polite');
  expect(announced?.textContent).toBe(finer);
});

test('a deficient slate is listed as no candidates at all', async () => {
  const { client, answer } = heldClient();
  const heading = catalog.entries['label.candidates'].text;
  let watcher: () => void = () => {};
  const held: CoreClient = {
    ...client,
    mode: () => 'still',
    slate: () => ({
      ordinal: 0,
      step: 4,
      deficient: true,
      deficiency_reason: 'no-alternative-candidate',
      window_declared: 45,
      window_effective: 4,
      dominance: [],
      sensitivity: { flag: false, changed_at: [] },
      candidates: [
        {
          position: 1,
          view: { inside: [2], resolution: 1, window: 45, surround: 'adjacent' as const },
          provenance: [{ source: 'standing' as const, detail: null }],
          // The four values are still read for the standing View of a
          // deficient slate; the tier is the number no tier has, because no
          // comparison ran.
          tier: 0,
          privilege: profile(reading(65536, 65536, 65536), reading(0, 0, 0)),
          baseline: NO_DEVIATIONS,
        },
      ],
    }),
    snapshot: () =>
      ({ header: { impulse: 3 } }) as unknown as ReturnType<CoreClient['snapshot']>,
    watch: (observer) => {
      watcher = observer;
      return () => {};
    },
  };

  const { container } = opened(<App open={() => held} />);
  await act(async () => {
    answer({ v: PROTOCOL_VERSION, re: 1, ok: true, body: { protocol: PROTOCOL_VERSION } });
  });
  await act(async () => watcher());

  // No comparison runs on a deficient slate and nothing is adopted from one,
  // so the tray offers no list to walk.
  expect(screen.queryByText(heading)).toBeNull();
  expect(container.querySelectorAll('.tray-candidate')).toHaveLength(0);
});

/**
 * The structural guard over the shell's own surfaces.
 *
 * A surface shipped with no rule at all fails in the one way that is invisible
 * from inside a component test: the markup renders, the buttons answer, every
 * assertion about them passes — and no player can reach it, because the field
 * is a full-viewport canvas under `overflow: hidden` and an unplaced region
 * lands after it, below the fold. The Handoff shipped exactly that way. So the
 * stylesheet is read here as a contract rather than as decoration.
 */
const SHELL = path.join(inject('workspace'), 'app', 'src', 'shell');

/** Every class name the shell's components put on the page. */
function renderedClasses(): Set<string> {
  const found = new Set<string>();
  for (const entry of readdirSync(SHELL)) {
    if (!entry.endsWith('.tsx')) continue;
    const source = readFileSync(path.join(SHELL, entry), 'utf8');
    for (const [, listed] of source.matchAll(/className="([^"]*)"/g)) {
      for (const name of listed.split(/\s+/)) if (name) found.add(name);
    }
  }
  return found;
}

test('every class the shell renders carries a rule in the one stylesheet', () => {
  const stylesheet = readFileSync(path.join(SHELL, 'shell.css'), 'utf8');
  const rendered = [...renderedClasses()].sort();
  expect(rendered.length).toBeGreaterThan(30);

  // A rule for the class itself, not for one that merely starts with its name:
  // `.tray-value` is not a rule for `.tray`, and `.handoff-move` is not one
  // for `.handoff`.
  const unstyled = rendered.filter(
    (name) => !new RegExp(`\\.${name}(?![\\w-])`).test(stylesheet),
  );
  expect(unstyled).toEqual([]);
});

test('every surface the shell puts up is placed by the stylesheet', () => {
  const stylesheet = readFileSync(path.join(SHELL, 'shell.css'), 'utf8');
  const rendered = renderedClasses();

  // Each region the shell stands on the surface, named. A region that stops
  // being rendered fails here rather than leaving a rule with nothing under
  // it, and one that stops being placed fails here rather than in play.
  const SURFACES = [
    'notice',
    'opening',
    'field',
    'objective',
    'pressure-line',
    'tray',
    'inspect',
    'handoff',
    'echo',
    'review',
  ];
  for (const surface of SURFACES) {
    expect(rendered.has(surface), `the shell renders .${surface}`).toBe(true);
    const rule = stylesheet.match(new RegExp(`\\n\\.${surface} \\{([^}]*)\\}`));
    expect(rule, `the stylesheet holds a rule for .${surface}`).not.toBeNull();
    // The field is the surface everything else stands over, and it is the one
    // region laid out in the document rather than placed against the viewport.
    const placed = surface === 'field' ? 'width: 100vw' : 'position: fixed';
    expect(rule?.[1], `.${surface} is placed`).toContain(placed);
  }
});

test('the two Still Mode controls take clicks without their regions taking them', () => {
  const stylesheet = readFileSync(path.join(SHELL, 'shell.css'), 'utf8');
  // The region passes the field through and the interactive children do not,
  // which is the division every chrome surface here keeps.
  for (const region of ['inspect', 'handoff']) {
    const rule = stylesheet.match(new RegExp(`\\n\\.${region} \\{([^}]*)\\}`));
    expect(rule?.[1] ?? '', `.${region} lets the field through`).toContain('pointer-events: none');
  }
  for (const control of ['inspect-open', 'handoff-move', 'handoff-why', 'handoff-detail']) {
    const rule = stylesheet.match(new RegExp(`\\.${control}(?![\\w-])[^{]*\\{([^}]*)\\}`));
    expect(rule?.[1] ?? '', `.${control} takes clicks`).toContain('pointer-events: auto');
  }
  // And the Handoff's own controls answer the keyboard visibly.
  expect(stylesheet).toContain('.handoff-move:focus-visible');
  expect(stylesheet).toContain('.handoff-why:focus-visible');
});
