/**
 * Still Mode across the worker boundary, in the domain the ramp is stated in.
 *
 * The 250 ms is real time, and the accumulator is the worker's, so this is the
 * one place both halves of the ramp can be read at once: the timestamps are
 * the ones the shell's pump would see at 60 rendered frames a second, the
 * scale falls with them, and the step count each frame comes back with is what
 * the slowdown actually is. What a core test can pin is the arithmetic; what
 * this pins is that the arithmetic reaches the frames.
 *
 * Sessions are opened one after another rather than side by side, because the
 * worker under test is the real entry and each session must be the only one
 * loading the module while it starts.
 */

import '@vitest/web-worker';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { afterEach, beforeAll, expect, inject, test, vi } from 'vitest';
import {
  neutralFrame,
  PROTOCOL_VERSION,
  type CandidateSlate,
  type CoordinateProfile,
  type EchoHighlight,
  type ErrorEnvelope,
  type EventEnvelope,
  type FrameEventBody,
  type InputFrame,
  type Payload,
  type PerturbationResult,
  type ResponseEnvelope,
} from '../src/protocol';
import {
  decodeFrameState,
  FRAME_SECTION,
  FRAME_VERSION,
  type FrameState,
} from '../src/frame-state';

const WORKER_ENTRY = new URL('../src/entry.ts', import.meta.url);
const WORKSPACE = inject('workspace');

/** One rendered frame at the 60-frames-per-second target, in microseconds. */
const FRAME_US = 16_667;

/** How long a ramp takes, in microseconds. Locked. */
const RAMP_US = 250_000;

const KEY = '0123456789abcdef';

beforeAll(() => {
  vi.stubGlobal('fetch', async (target: URL | string) => {
    const { pathname } = new URL(String(target), 'http://localhost/');
    const bytes = await readFile(path.join(WORKSPACE, pathname));
    return new Response(bytes, { headers: { 'content-type': 'application/wasm' } });
  });
});

const opened: Worker[] = [];

afterEach(() => {
  for (const worker of opened.splice(0)) worker.terminate();
});

type FrameAnswer = { frame: FrameEventBody } | { refused: ErrorEnvelope };

interface Session {
  command: (cmd: string, body: Payload) => Promise<ResponseEnvelope>;
  send: (frame: InputFrame) => Promise<FrameAnswer>;
  /** The reviews raised since the last read, oldest first. */
  reviews: () => Payload[];
  close: () => void;
}

function openSession(): Session {
  const worker = new Worker(WORKER_ENTRY, { type: 'module' });
  opened.push(worker);

  const responses = new Map<number, (answer: ResponseEnvelope) => void>();
  const frames = new Map<number, (answer: FrameAnswer) => void>();
  const reviews: Payload[] = [];
  let nextId = 1;

  worker.addEventListener('message', (message) => {
    const data = (message as MessageEvent<ResponseEnvelope | EventEnvelope>).data;
    if ('re' in data) {
      const waiting = responses.get(data.re);
      responses.delete(data.re);
      waiting?.(data);
      return;
    }
    if (data.ev === 'review_ready') {
      reviews.push((data.body as { review: Payload }).review);
      return;
    }
    if (data.ev !== 'frame') return;
    const body = data.body as FrameEventBody;
    const waiting = frames.get(body.seq);
    frames.delete(body.seq);
    waiting?.({ frame: body });
  });

  return {
    command(cmd, body) {
      const id = nextId++;
      return new Promise((settle) => {
        responses.set(id, settle);
        worker.postMessage({ v: PROTOCOL_VERSION, id, cmd, body });
      });
    },
    send(frame) {
      const id = nextId++;
      return new Promise((settle) => {
        frames.set(frame.seq, settle);
        responses.set(id, (answer) => {
          frames.delete(frame.seq);
          settle({ refused: answer.ok ? ({} as ErrorEnvelope) : answer.error });
        });
        worker.postMessage({ v: PROTOCOL_VERSION, id, cmd: 'input_frame', body: frame });
      });
    },
    reviews: () => reviews.splice(0),
    close: () => worker.terminate(),
  };
}

async function openRun(session: Session, form = 'thread'): Promise<void> {
  const answer = await session.command('init_run', { mode: 'new', run_id: KEY, form });
  expect(answer.ok).toBe(true);
}

/** One frame at the render rate, with an optional toggle on it. */
function rendered(seq: number, toggle = false): InputFrame {
  return { ...neutralFrame(seq, seq * FRAME_US), toggle_still: toggle };
}

/** The frame body, or a thrown refusal. */
function answered(answer: FrameAnswer): FrameEventBody {
  if ('refused' in answer) throw new Error(`a frame was refused: ${JSON.stringify(answer)}`);
  return answer.frame;
}

/** What one sent frame produced: the step count, and the snapshot when it carried one. */
interface Sent {
  ran: number;
  remainder: number;
  snapshot: FrameState | null;
}

async function send(session: Session, frame: InputFrame): Promise<Sent> {
  const body = answered(await session.send(frame));
  return {
    ran: body.steps_run,
    remainder: body.remainder_us,
    snapshot: body.buffer ? decodeFrameState(body.buffer) : null,
  };
}

/**
 * Plays frames at the render rate until one comes back carrying a snapshot.
 *
 * The first frame of a session reads a gap of zero and so runs no step, and a
 * frame that ran no step and changed no mode carries no buffer — the locked
 * rule about when one rides along. So a test that wants a snapshot asks for
 * frames until one brings it.
 */
async function firstSnapshot(session: Session, from: number): Promise<FrameState> {
  for (let seq = from; seq < from + 8; seq += 1) {
    const one = await send(session, rendered(seq));
    if (one.snapshot) return one.snapshot;
  }
  throw new Error('no frame carried a snapshot');
}

/**
 * Plays frames at the render rate until one comes back in a mode the caller
 * named, and answers with everything the frames produced.
 */
async function playUntil(
  session: Session,
  mode: string,
  from: number,
  cap = 40,
): Promise<{ sent: Sent[]; seq: number; snapshot: FrameState }> {
  const sent: Sent[] = [];
  let held: FrameState | null = null;
  let seq = from;
  for (let index = 0; index < cap; index += 1) {
    const one = await send(session, rendered(seq));
    sent.push(one);
    if (one.snapshot) held = one.snapshot;
    seq += 1;
    if (held?.header.mode === mode) return { sent, seq, snapshot: held };
  }
  throw new Error(`the run never reached ${mode}`);
}

test('the field slows over the locked 250 ms and then stands still', async () => {
  const session = openSession();
  await openRun(session);

  // Ten frames of ordinary play, so the accumulator is running as it would be.
  let seq = 1;
  for (; seq <= 10; seq += 1) await send(session, rendered(seq));

  const toggled = await send(session, rendered(seq, true));
  seq += 1;
  expect(toggled.snapshot?.header.mode).toBe('ramp_in');

  const { sent, seq: after, snapshot } = await playUntil(session, 'still', seq);
  expect(snapshot.header.mode).toBe('still');
  expect(snapshot.header.timeScale).toBe(0);

  // The ramp takes the locked span of real time, which at the render rate is
  // fifteen frames and a fraction of a sixteenth.
  const frames = sent.length;
  expect(frames).toBe(Math.ceil(RAMP_US / FRAME_US));

  // And it is a slowdown rather than a stop: the ramp runs steps, but fewer
  // than the same span of ordinary play would.
  const ran = sent.reduce((total, one) => total + one.ran, 0);
  expect(ran).toBeGreaterThan(0);
  expect(ran).toBeLessThan(Math.round((frames * FRAME_US * 30) / 1_000_000));

  // A still run stands: the frames that follow run nothing and interpolate
  // across nothing, so the surface holds exactly where it stopped.
  const holding = await send(session, rendered(after));
  expect(holding.ran).toBe(0);
  expect(holding.remainder).toBe(0);
  session.close();
});

test('the scale falls through the ramp and rises back through the exit', async () => {
  const session = openSession();
  await openRun(session);

  let seq = 1;
  await send(session, rendered(seq, true));
  seq += 1;

  // Every snapshot the entry ramp carries reports a scale no higher than the
  // one before it, and the last of them is the pause itself.
  const falling: number[] = [];
  const entry = await playUntil(session, 'still', seq);
  for (const one of entry.sent) {
    if (one.snapshot?.header.mode === 'ramp_in') falling.push(one.snapshot.header.timeScale);
  }
  expect(falling.length).toBeGreaterThan(1);
  for (let place = 1; place < falling.length; place += 1) {
    expect(falling[place]).toBeLessThan(falling[place - 1]);
  }
  seq = entry.seq;

  await send(session, rendered(seq, true));
  seq += 1;
  const rising: number[] = [];
  const exit = await playUntil(session, 'running', seq);
  for (const one of exit.sent) {
    if (one.snapshot?.header.mode === 'ramp_out') rising.push(one.snapshot.header.timeScale);
  }
  expect(rising.length).toBeGreaterThan(1);
  for (let place = 1; place < rising.length; place += 1) {
    expect(rising[place]).toBeGreaterThan(rising[place - 1]);
  }
  expect(exit.snapshot.header.timeScale).toBe(65_535);
  session.close();
});

test('the still frame carries the overlay section, and the moving frame does not', async () => {
  const session = openSession();
  await openRun(session);

  const first = await firstSnapshot(session, 1);
  expect(first.overlay).toBeNull();
  expect(first.header.stillVisible).toBe(false);

  await send(session, rendered(4, true));
  const { snapshot } = await playUntil(session, 'still', 5);
  expect(snapshot.header.stillVisible).toBe(true);
  // The overlay stands, carrying the standing candidate's baseline envelope:
  // one low-and-high pair per step of the clamped window. The replays draw
  // nothing, so each pair is a point.
  expect(snapshot.overlay).not.toBeNull();
  expect(snapshot.overlay?.length).toBeGreaterThan(0);
  for (const step of snapshot.overlay ?? []) {
    expect(step.hi).toBeGreaterThanOrEqual(step.lo);
  }
  session.close();
});

test('the three queued-change commands answer only while the run is still', async () => {
  const session = openSession();
  await openRun(session);

  const running = await session.command('undo_plan', {});
  expect(running.ok).toBe(false);
  if (!running.ok) expect(running.error.code).toBe('state');

  await send(session, rendered(1, true));
  await playUntil(session, 'still', 2);

  const undone = await session.command('undo_plan', {});
  expect(undone.ok).toBe(true);
  if (undone.ok) {
    expect(undone.body.remaining).toBe(0);
    expect(undone.body.queue).toEqual({
      entries: [],
      cost_total: 0,
      impulse: 3,
      impulse_after: 3,
    });
  }

  // A commit is an exit, and the exit is a ramp like any other.
  const committed = await session.command('commit_plan', {});
  expect(committed.ok).toBe(true);
  if (committed.ok) expect(committed.body.applied).toBe(0);
  session.close();
});

test('a run driven into Still Mode exports a record that restores into running', async () => {
  const session = openSession();
  await openRun(session);
  await send(session, rendered(1, true));
  await playUntil(session, 'still', 2);

  const exported = await session.command('export_run', {});
  expect(exported.ok).toBe(true);
  const text = exported.ok ? (exported.body.text as string) : '';
  session.close();

  const fresh = openSession();
  const imported = await fresh.command('import_run', { text });
  expect(imported.ok).toBe(true);
  // The locked mid-still restore rule, read from the other side: a record
  // carries no mode, so the run it opens is a moving one.
  const after = await firstSnapshot(fresh, 1);
  expect(after.header.mode).toBe('running');
  expect(after.header.timeScale).toBe(65_535);
  expect(after.overlay).toBeNull();
  fresh.close();
});

test('the decoder reads an overlay section the encoder wrote', () => {
  // A filled section and an empty one are read by the one decoder path. The
  // buffer is built by hand to the locked layout rather than taken from a run,
  // so the layout is pinned on its own terms and a core-side change to what
  // fills the envelope cannot silently move it.
  const header = 32;
  const table = 8;
  const records = 2;
  const bytes = new Uint8Array(header + table + records * 8);
  const view = new DataView(bytes.buffer);
  bytes.set([0x46, 0x47, 0x46, 0x31], 0);
  view.setUint16(4, FRAME_VERSION, true);
  view.setUint16(6, 1, true);
  view.setUint16(12, 0, true);
  bytes[14] = 2;
  bytes[20] = 1;
  bytes[header] = FRAME_SECTION.overlay;
  view.setUint16(header + 2, records, true);
  view.setUint32(header + 4, header + table, true);
  view.setFloat32(header + table, 0.5, true);
  view.setFloat32(header + table + 4, 1.5, true);
  view.setFloat32(header + table + 8, 0.25, true);
  view.setFloat32(header + table + 12, 2.25, true);

  const decoded = decodeFrameState(bytes.buffer);
  expect(decoded.header.mode).toBe('still');
  expect(decoded.header.stillVisible).toBe(true);
  expect(decoded.overlay).toEqual([
    { lo: 0.5, hi: 1.5 },
    { lo: 0.25, hi: 2.25 },
  ]);
});

test('a queued change crosses the bridge, previews in the frame, and commits', async () => {
  // The whole path in one place: a plan body over the protocol, the queue the
  // response carries, the preview the next frame draws it as, and the commit
  // that spends exactly what the queue predicted.
  const session = openSession();
  await openRun(session);

  await send(session, rendered(1, true));
  const { seq, snapshot } = await playUntil(session, 'still', 2);
  expect(snapshot.header.impulse).toBe(3);
  const standing = snapshot.routes.length;

  const queued = await session.command('queue_plan', { plan: { op: 'cut', route: 1 } });
  expect(queued.ok).toBe(true);
  if (queued.ok) {
    expect(queued.body.queue).toEqual({
      entries: [{ position: 0, plan: { op: 'cut', route: 1 }, cost: 1, conflict: false }],
      cost_total: 1,
      impulse: 3,
      impulse_after: 2,
    });
  }

  const reshaped = await session.command('queue_plan', {
    plan: { op: 'reshape_compartment', members: [2, 3] },
  });
  expect(reshaped.ok).toBe(true);
  if (reshaped.ok) {
    expect(reshaped.body.queue).toMatchObject({
      entries: [
        { position: 0, plan: { op: 'cut', route: 1 }, cost: 1, conflict: false },
        {
          position: 1,
          plan: { op: 'reshape_compartment', members: [2, 3] },
          cost: 1,
          conflict: false,
        },
      ],
      cost_total: 2,
      impulse: 3,
      impulse_after: 1,
    });
  }
  const predicted = reshaped.ok ? (reshaped.body.queue as { cost_total: number }).cost_total : 0;
  expect(predicted).toBe(2);

  // An entry that cannot stand is refused and is not queued.
  const refused = await session.command('queue_plan', { plan: { op: 'cut', route: 1 } });
  expect(refused.ok).toBe(false);
  if (!refused.ok) expect(refused.error.code).toBe('not_found');

  // The frame draws the queue: the Route a cut would take reads as queued, and
  // the membership the reshape proposes stands beside the standing one.
  const preview = await firstSnapshot(session, seq);
  expect(preview.routes.filter((route) => route.status === 1).map((route) => route.route)).toEqual([
    1,
  ]);
  expect(preview.ports.filter((port) => port.proposedMember).map((port) => port.node)).toEqual([
    2, 3,
  ]);
  expect(preview.ports.filter((port) => port.member).map((port) => port.node)).toEqual([2, 3, 4]);
  const viewBeforeCommit = preview.ports
    .filter((_port, place) => preview.inside[place])
    .map((port) => port.node);

  const committed = await session.command('commit_plan', {});
  expect(committed.ok).toBe(true);
  if (committed.ok) {
    expect(committed.body.applied).toBe(2);
    // What the queue predicted is what the commit spent.
    expect(committed.body.impulse).toBe(3 - predicted);
  }

  const after = await firstSnapshot(session, seq + 8);
  expect(after.routes).toHaveLength(standing - 1);
  expect(after.routes.every((route) => route.status !== 1)).toBe(true);
  expect(after.ports.filter((port) => port.member).map((port) => port.node)).toEqual([2, 3]);
  expect(after.ports.every((port) => !port.proposedMember)).toBe(true);
  expect(after.ports.filter((_port, place) => after.inside[place]).map((port) => port.node)).toEqual(
    viewBeforeCommit,
  );
  expect(after.header.impulse).toBe(1);
  session.close();
});


test('entering Still Mode raises the slate, and set_focus moves only the passive View', async () => {
  // The whole candidate path over the bridge: the record the entry raises, the
  // position a top-level `set_focus` names in it, and the independent View
  // bitset the next frame carries without a queued or committed causal edit.
  const session = openSession();
  await openRun(session);

  await send(session, rendered(1, true));
  const { seq, snapshot } = await playUntil(session, 'still', 2);
  const physicalBefore = snapshot.ports.filter((port) => port.member).map((port) => port.node);
  const viewBefore = snapshot.ports
    .filter((_port, place) => snapshot.inside[place])
    .map((port) => port.node);
  expect(physicalBefore.length).toBeGreaterThan(0);
  expect(viewBefore.length).toBeGreaterThan(0);

  const raised = session.reviews();
  expect(raised).toHaveLength(1);
  const review = raised[0] as { kind: string; slate: CandidateSlate };
  expect(review.kind).toBe('slate');
  const slate = review.slate;
  expect(slate.ordinal).toBe(0);
  expect(slate.deficient).toBe(false);
  // Two to five candidates, the standing View in seat 1, and every one of them
  // carrying why it exists.
  expect(slate.candidates.length).toBeGreaterThanOrEqual(2);
  expect(slate.candidates.length).toBeLessThanOrEqual(5);
  expect(slate.candidates[0].provenance[0].source).toBe('standing');
  expect(slate.candidates[0].view.inside).toEqual(viewBefore);
  for (const candidate of slate.candidates) {
    expect(candidate.provenance.length).toBeGreaterThan(0);
    expect(candidate.view.inside.length).toBeGreaterThan(0);
    // Ranked: every candidate stands in a tier, and carries four values that
    // are never combined into one.
    expect(candidate.tier).toBeGreaterThanOrEqual(1);
    expect(candidate.tier).toBeLessThanOrEqual(slate.candidates.length);
    for (const value of [
      candidate.privilege.scale_stability,
      candidate.privilege.shared_failure,
      candidate.privilege.cut_impact,
      candidate.privilege.boundary_sufficiency,
    ]) {
      // Either a number inside a confidence range that contains it, or no
      // number at all and the stated reason.
      if (value.value === null) {
        expect(value.low).toBeNull();
        expect(value.high).toBeNull();
        expect(value.reason).toBeTruthy();
      } else {
        expect(value.reason).toBeNull();
        expect(value.low).toBeLessThanOrEqual(value.value);
        expect(value.high).toBeGreaterThanOrEqual(value.value);
      }
    }
    expect(candidate.baseline.deviations).toHaveLength(8);
  }
  // The tiers derive from the dominance relation, which the record carries.
  for (const pair of slate.dominance) {
    const first = slate.candidates[pair.a - 1];
    const second = slate.candidates[pair.b - 1];
    expect(first.tier).toBeLessThan(second.tier);
  }
  expect(slate.sensitivity.flag).toBe(slate.sensitivity.changed_at.length > 0);

  const adopted = slate.candidates[1].view;
  const focused = await session.command('set_focus', {
    slate_ordinal: slate.ordinal,
    position: 2,
  });
  expect(focused.ok).toBe(true);
  if (!focused.ok) return;
  expect(focused.body.view).toEqual(adopted);

  // A still frame carries the moved View immediately. Physical membership,
  // proposed-physical flags, Intervention Budget, and the queue do not move.
  const after = await firstSnapshot(session, seq);
  expect(after.ports.filter((_port, place) => after.inside[place]).map((port) => port.node)).toEqual(
    adopted.inside,
  );
  expect(after.ports.filter((port) => port.member).map((port) => port.node)).toEqual(physicalBefore);
  expect(after.ports.every((port) => !port.proposedMember)).toBe(true);
  expect(after.header.impulse).toBe(snapshot.header.impulse);
  expect(session.reviews()).toEqual([]);

  // Position 0 clears only the selected members and retains the measurement
  // protocol. The physical compartment and Intervention Budget still do not
  // move, and the cleared View is visible in the next frame as an empty bitset.
  const cleared = await session.command('set_focus', {
    slate_ordinal: slate.ordinal,
    position: 0,
  });
  expect(cleared.ok).toBe(true);
  if (!cleared.ok) return;
  expect(cleared.body.view).toEqual({ ...adopted, inside: [] });
  const afterClear = await firstSnapshot(session, seq + 8);
  expect(afterClear.ports.filter((_port, place) => afterClear.inside[place])).toEqual([]);
  expect(afterClear.ports.filter((port) => port.member).map((port) => port.node)).toEqual(
    physicalBefore,
  );
  expect(afterClear.header.impulse).toBe(snapshot.header.impulse);
  session.close();
});

test('an inspect request crosses the bridge and answers with the profile', async () => {
  const session = openSession();
  await openRun(session);

  let seq = 1;
  for (; seq <= 10; seq += 1) await send(session, rendered(seq));
  await send(session, rendered(seq, true));
  seq += 1;
  const entered = await playUntil(session, 'still', seq);
  seq = entered.seq;
  session.reviews();

  // Nothing has been asked for, so nothing has been answered: a profile is
  // taken because a frame carried a request and never on its own.
  await send(session, rendered(seq));
  seq += 1;
  expect(session.reviews()).toHaveLength(0);

  // The eight recorded-window coordinates.
  await send(session, {
    ...rendered(seq),
    inspect: { target: 'coordinates', kind: null, parameter: null },
  });
  seq += 1;
  const answered = session.reviews();
  expect(answered).toHaveLength(1);
  expect((answered[0] as { kind: string }).kind).toBe('coordinates');
  const profile = (answered[0] as { profile: CoordinateProfile }).profile;
  expect(profile.swap_range).toBeDefined();
  expect(profile.instruction_separation).toBeNull();
  expect(profile.turnover_tolerance).toBeNull();
  // Ten readings, the step, and the View — and no key that folds several of
  // them into one.
  expect(Object.keys(profile).sort()).toEqual([
    'horizon',
    'input_resolution',
    'instruction_separation',
    'reach',
    'self_support',
    'source_trace',
    'step',
    'swap_range',
    'throughput',
    'turnover_tolerance',
    'upkeep_mix',
    'view',
  ]);

  // The whole profile, which the two replays pay for.
  await send(session, {
    ...rendered(seq),
    inspect: { target: 'coordinates_full', kind: null, parameter: null },
  });
  seq += 1;
  const whole = session.reviews();
  expect((whole[0] as { profile: CoordinateProfile }).profile.turnover_tolerance).not.toBeNull();

  // And one perturbation, whose defaulted parameter comes back resolved.
  await send(session, {
    ...rendered(seq),
    inspect: { target: 'perturbation', kind: 'route-removal', parameter: null },
  });
  const perturbed = session.reviews();
  expect((perturbed[0] as { kind: string }).kind).toBe('perturbation');
  const result = (perturbed[0] as { result: PerturbationResult }).result;
  expect(result.kind).toBe('route-removal');
  // A defaulted parameter comes back resolved, or the reading says why the
  // kind's own default rule could name none — never a bare null beside an
  // assigned reading.
  expect(result.parameter === null ? result.reading.reason : typeof result.parameter).toBeTruthy();

  // Boundary severance takes no parameter and always replays, so it is where
  // the compact playback record is read: one series per sample, at most w long.
  await send(session, {
    ...rendered(seq + 1),
    inspect: { target: 'perturbation', kind: 'boundary-severance', parameter: null },
  });
  const severed = (session.reviews()[0] as { result: PerturbationResult }).result;
  expect(severed.parameter).toBeNull();
  expect(severed.samples).toHaveLength(8);
  for (const sample of severed.samples) {
    expect(Array.isArray(sample.series)).toBe(true);
    expect(sample.series.length).toBeLessThanOrEqual(severed.view.window);
    expect(sample.base_series).toBeNull();
  }
  // Every stream the result used is recorded: the eight shared-baseline
  // streams its excesses are taken against stand first, then the kind's own
  // eight samples.
  expect(severed.streams).toHaveLength(16);
  expect(severed.streams[0]).toBe('candidate/0/baseline/1');
  expect(severed.streams[8]).toBe('candidate/0/perturbation/boundary-severance/1');
  session.close();
});

test('a moving run answers no inspect request at all', async () => {
  const session = openSession();
  await openRun(session);
  for (let seq = 1; seq <= 10; seq += 1) {
    await send(session, {
      ...rendered(seq),
      inspect: { target: 'coordinates_full', kind: null, parameter: null },
    });
  }
  // Ordinary play is free of readings nobody asked to see, and the core is
  // where that holds: the request is valid, and it is simply not answered.
  expect(session.reviews()).toHaveLength(0);
  session.close();
});

test('a committed cut leaves one Echo, raised at the exit', async () => {
  const session = openSession();
  await openRun(session);

  let seq = 1;
  for (; seq <= 12; seq += 1) await send(session, rendered(seq));
  await send(session, rendered(seq, true));
  seq += 1;
  const entered = await playUntil(session, 'still', seq);
  seq = entered.seq;
  session.reviews();

  const route = entered.snapshot.routes.find((held) => held.status === 0);
  expect(route).toBeDefined();
  const queued = await session.command('queue_plan', {
    plan: { op: 'cut', route: route?.route },
  });
  expect(queued.ok).toBe(true);
  const committed = await session.command('commit_plan', {});
  expect(committed.ok).toBe(true);
  // The commit's own review is the reassembled slate; the highlight waits.
  expect(session.reviews().map((held) => (held as { kind: string }).kind)).toEqual(['slate']);

  const left = await playUntil(session, 'running', seq);
  const raised = session.reviews().filter((held) => (held as { kind: string }).kind === 'echo');
  expect(raised).toHaveLength(1);
  const echo = (raised[0] as { echo: EchoHighlight }).echo;
  // The framework's Echo rule, carried: a committed cut of one Route reads
  // from Route removal with that Route as its parameter.
  expect(echo.kind).toBe('route-removal');
  expect(echo.parameter).toBe(route?.route);
  expect(echo.target).toEqual({ t: 'route', id: route?.route });
  expect(left.snapshot.header.mode).toBe('running');
  session.close();
});

// ---------------------------------------------------------------------------
// The Handoff
// ---------------------------------------------------------------------------

test('the frame a Handoff rides carries a fresh buffer with the flag moved', async () => {
  // Chorus stands four Forms, so a run under it can hand control on without
  // any chapter authoring a second placement.
  const session = openSession();
  await openRun(session, 'chorus');

  let seq = 1;
  for (; seq <= 10; seq += 1) await send(session, rendered(seq));
  await send(session, rendered(seq, true));
  seq += 1;
  const entered = await playUntil(session, 'still', seq);
  seq = entered.seq;
  const held = entered.snapshot.forms.find((form) => form.controlled);
  expect(held?.id).toBe(1);
  expect(entered.snapshot.forms).toHaveLength(4);
  // The entry's own slate, let go of, so what is counted below is what the
  // Handoff raised and nothing else.
  expect(session.reviews().map((one) => (one as { kind: string }).kind)).toEqual(['slate']);

  // A still frame runs no step and changes no mode, so without the Handoff's
  // own condition it would carry no buffer at all — and the renderer would go
  // on drawing the Form control had left.
  const quiet = await send(session, rendered(seq));
  seq += 1;
  expect(quiet.ran).toBe(0);
  expect(quiet.snapshot).toBeNull();

  const moved = await send(session, {
    ...rendered(seq),
    inspect: { target: 'handoff', kind: null, parameter: 3 },
  });
  seq += 1;
  expect(moved.ran).toBe(0);
  expect(moved.snapshot).not.toBeNull();
  expect(moved.snapshot?.header.mode).toBe('still');
  expect(moved.snapshot?.forms.find((form) => form.controlled)?.id).toBe(3);
  expect(moved.snapshot?.forms.filter((form) => form.controlled)).toHaveLength(1);
  // And nothing was reviewed: a Handoff moves control rather than reading a
  // View, so it raises no `review_ready` of its own.
  expect(session.reviews()).toHaveLength(0);
  session.close();
});

test('a Handoff the core refuses answers on the frame error path', async () => {
  const session = openSession();
  await openRun(session, 'chorus');

  let seq = 1;
  for (; seq <= 10; seq += 1) await send(session, rendered(seq));
  await send(session, rendered(seq, true));
  seq += 1;
  const entered = await playUntil(session, 'still', seq);
  seq = entered.seq;

  // A Form the run does not hold.
  const missing = await session.send({
    ...rendered(seq),
    inspect: { target: 'handoff', kind: null, parameter: 200 },
  });
  seq += 1;
  expect('refused' in missing && missing.refused.code).toBe('not_found');

  // And the Form already controlled: a no-op is refused rather than absorbed.
  const standing = await session.send({
    ...rendered(seq),
    inspect: { target: 'handoff', kind: null, parameter: 1 },
  });
  expect('refused' in standing && standing.refused.code).toBe('validation');
  session.close();
});
