/**
 * The command surface, across the worker boundary.
 *
 * Everything here is read the way the shell reads it: protocol envelopes over
 * `postMessage`, with no direct reach into the module. What is under test is
 * the whole surface this goal completes — the pause level and the step hook of
 * `input_frame`, `export_run`, `import_run` with its locked validation, the two
 * restores over the session's records, the render snapshot's decoder, and the
 * malformed answer every command gives.
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
  FORM_IDS,
  neutralFrame,
  PROTOCOL_VERSION,
  SAVE_VERSION,
  type ErrorEnvelope,
  type EventEnvelope,
  type FrameEventBody,
  type InputFrame,
  type Payload,
  type ResponseEnvelope,
} from '../src/protocol';
import {
  decodeFrameState,
  FrameStateError,
  FRAME_HEADER_BYTES,
  FRAME_MAGIC,
  FRAME_VERSION,
} from '../src/frame-state';

const WORKER_ENTRY = new URL('../src/entry.ts', import.meta.url);
const WORKSPACE = inject('workspace');

/** One rendered frame at the 60-frames-per-second target, in microseconds. */
const FRAME_US = 16_667;

/** How many completed steps one autosave interval spans. */
const AUTOSAVE_STEPS = 900;

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
  command: (cmd: string, body: unknown) => Promise<ResponseEnvelope>;
  raw: (message: unknown) => Promise<ResponseEnvelope>;
  send: (frame: InputFrame) => Promise<FrameAnswer>;
  close: () => void;
}

function openSession(): Session {
  const worker = new Worker(WORKER_ENTRY, { type: 'module' });
  opened.push(worker);

  const responses = new Map<number, (answer: ResponseEnvelope) => void>();
  const frames = new Map<number, (answer: FrameAnswer) => void>();
  let nextId = 1;

  worker.addEventListener('message', (message) => {
    const data = (message as MessageEvent<ResponseEnvelope | EventEnvelope>).data;
    if ('re' in data) {
      const waiting = responses.get(data.re);
      responses.delete(data.re);
      waiting?.(data);
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
    raw(message) {
      // A message the envelope rules answer before the core is consulted. Its
      // correlation is whatever the message carried, or zero.
      const id =
        typeof message === 'object' && message !== null && 'id' in message
          ? Number((message as { id: unknown }).id)
          : 0;
      const key = Number.isInteger(id) && id >= 0 ? id : 0;
      if (key >= nextId) nextId = key + 1;
      return new Promise((settle) => {
        responses.set(key, settle);
        worker.postMessage(message);
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
    close() {
      worker.terminate();
    },
  };
}

async function openRun(session: Session, key = KEY): Promise<Payload> {
  const opening = await session.command('init_run', { mode: 'new', run_id: key, form: 'thread' });
  expect(opening.ok).toBe(true);
  return opening.ok ? opening.body : {};
}

async function exportOf(session: Session): Promise<Payload> {
  const answer = await session.command('export_run', {});
  expect(answer.ok).toBe(true);
  return answer.ok ? answer.body : {};
}

function refusalOf(answer: ResponseEnvelope): ErrorEnvelope {
  expect(answer.ok).toBe(false);
  if (answer.ok) throw new Error('a refusal was expected');
  return answer.error;
}

// ---------------------------------------------------------------------------
// Export, import, and the round trip a fresh worker completes
// ---------------------------------------------------------------------------

test('an export carried to a fresh worker imports and re-exports the same bytes', async () => {
  const first = openSession();
  await openRun(first);
  await first.send({ ...neutralFrame(1, FRAME_US), advance_steps: 30 });
  const taken = await exportOf(first);
  first.close();

  // A second worker, which has never seen this run, restores it exactly.
  const second = openSession();
  const imported = await second.command('import_run', { text: taken.text });
  expect(imported.ok).toBe(true);
  if (!imported.ok) return;
  expect(imported.body.run_id).toBe(KEY);
  expect(imported.body.step).toBe(30);
  expect(imported.body.branch_nonce).toBe(0);
  expect(imported.body.view).toMatchObject({
    inside: expect.any(Array),
    resolution: expect.any(Number),
    window: expect.any(Number),
    surround: expect.any(String),
  });
  expect(imported.body).not.toHaveProperty('migrated_from');

  const again = await exportOf(second);
  second.close();

  // A third worker imports what the second exported, and the two agree byte
  // for byte: the restore is a fixed point, so the whole cycle repeats.
  const third = openSession();
  await third.command('import_run', { text: again.text });
  const third_time = await exportOf(third);
  expect(third_time.text).toBe(again.text);
  expect(third_time.sha256).toBe(again.sha256);
  expect(String(again.sha256)).toMatch(/^[0-9a-f]{64}$/);
});

test('a verified V1 import labels its transient migration and rewrites as ordinary V2', async () => {
  const { DEV_RUN_EXPORT } = await import('../../app/src/shell/dev-run');
  const session = openSession();
  const migrated = await session.command('import_run', { text: DEV_RUN_EXPORT });
  expect(migrated.ok).toBe(true);
  if (!migrated.ok) return;
  expect(migrated.body.migrated_from).toBe(1);
  expect(migrated.body.view).toMatchObject({
    inside: expect.any(Array),
    resolution: expect.any(Number),
    window: expect.any(Number),
    surround: expect.any(String),
  });

  const rewritten = await exportOf(session);
  expect(JSON.parse(String(rewritten.text)).save_version).toBe(SAVE_VERSION);
  session.close();

  const reopened = openSession();
  const ordinary = await reopened.command('import_run', { text: rewritten.text });
  expect(ordinary.ok).toBe(true);
  if (!ordinary.ok) return;
  expect(ordinary.body).not.toHaveProperty('migrated_from');
  expect(ordinary.body.view).toEqual(migrated.body.view);
  reopened.close();
});

test('an import is refused for every locked reason, and loads nothing', async () => {
  const source = openSession();
  await openRun(source);
  await source.send({ ...neutralFrame(1, FRAME_US), advance_steps: 4 });
  const file = String((await exportOf(source)).text);
  source.close();

  const refusals: [string, string, string][] = [
    ['not an object', '[]', 'import_invalid'],
    ['empty', '{}', 'import_invalid'],
    ['truncated', file.slice(0, Math.floor(file.length / 2)), 'import_invalid'],
    ['spaced', file.replace('{', '{ '), 'import_invalid'],
    ['wrong format', file.replace('field-game-run', 'field-game-save'), 'import_invalid'],
    [
      'newer version',
      file.replace(
        `,"save_version":${SAVE_VERSION}}`,
        `,"save_version":${SAVE_VERSION + 1}}`,
      ),
      'save_version',
    ],
    ['wrong digest', file.replace('"payload_sha256":"', '"payload_sha256":"0'), 'import_invalid'],
    ['out of range', file.replace('"impulse":3', '"impulse":9'), 'import_invalid'],
  ];

  const session = openSession();
  for (const [named, text, code] of refusals) {
    const answer = await session.command('import_run', { text });
    expect(refusalOf(answer).code, named).toBe(code);
  }
  // Nothing loaded, so the command that needs a run is still out of state.
  expect(refusalOf(await session.command('export_run', {})).code).toBe('state');
});

// ---------------------------------------------------------------------------
// The restores
// ---------------------------------------------------------------------------

test('a checkpoint is restored exactly, and a recovered branch takes a fresh nonce', async () => {
  const session = openSession();
  await openRun(session);
  // One autosave interval of play writes one record, which the restores name.
  await session.send({ ...neutralFrame(1, FRAME_US), advance_steps: AUTOSAVE_STEPS });
  const atInterval = await exportOf(session);

  await session.send({ ...neutralFrame(2, 2 * FRAME_US), advance_steps: 25 });
  expect(JSON.parse(String((await exportOf(session)).text)).payload.field.now.step).toBe(
    AUTOSAVE_STEPS + 25,
  );

  const anchors = JSON.parse(String(atInterval.text)).payload.anchors;
  expect(anchors).toHaveLength(1);
  expect(anchors[0].kind).toBe('auto');
  expect(anchors[0].step).toBe(AUTOSAVE_STEPS);
  const anchorId = anchors[0].anchor_id;

  const retried = await session.command('restore_checkpoint', { anchor_id: anchorId });
  expect(retried.ok).toBe(true);
  if (!retried.ok) return;
  expect(retried.body.step).toBe(AUTOSAVE_STEPS);
  expect(retried.body.branch_nonce).toBe(0);
  expect(retried.body.view).toMatchObject({
    inside: expect.any(Array),
    resolution: expect.any(Number),
    window: expect.any(Number),
    surround: expect.any(String),
  });
  expect(retried.body).not.toHaveProperty('migrated_from');

  const back = JSON.parse(String((await exportOf(session)).text)).payload;
  const recorded = JSON.parse(String(atInterval.text)).payload;
  expect(back.field.now.step).toBe(AUTOSAVE_STEPS);
  expect(back.rng).toEqual(recorded.rng);
  // The one post-restore normalization, and nothing else.
  expect(back.field.now.prev_assembly_step).toBe(AUTOSAVE_STEPS);
  expect(recorded.field.now.prev_assembly_step).toBeNull();

  const recovered = await session.command('recover_branch', { anchor_id: anchorId });
  expect(recovered.ok).toBe(true);
  if (!recovered.ok) return;
  expect(recovered.body.branch_nonce).toBe(1);
  expect(recovered.body.view).toEqual(retried.body.view);
  const branched = JSON.parse(String((await exportOf(session)).text)).payload;
  expect(branched.branch_nonce).toBe(1);
  expect(branched.rng).not.toEqual(recorded.rng);
  expect(branched.field.now).toEqual(back.field.now);

  // And the frame numbering starts over, because a restore clears what the
  // accumulator and the frame counter held.
  const resumed = await session.send({ ...neutralFrame(1, FRAME_US), advance_steps: 3 });
  expect(resumed).toHaveProperty('frame');
});

test('a restore naming no checkpoint is not found and moves nothing', async () => {
  const session = openSession();
  await openRun(session);
  await session.send({ ...neutralFrame(1, FRAME_US), advance_steps: 5 });
  for (const cmd of ['restore_checkpoint', 'recover_branch']) {
    expect(refusalOf(await session.command(cmd, { anchor_id: 77 })).code).toBe('not_found');
  }
  expect(JSON.parse(String((await exportOf(session)).text)).payload.field.now.step).toBe(5);
});

// ---------------------------------------------------------------------------
// The render snapshot
// ---------------------------------------------------------------------------

test('the decoder reads the fixture the core encoder pins', async () => {
  const hex = (
    await readFile(path.join(WORKSPACE, 'core', 'tests', 'fixtures', 'frame_state.hex'), 'utf8')
  ).trim();
  const bytes = new Uint8Array(hex.length / 2);
  for (let place = 0; place < bytes.length; place += 1) {
    bytes[place] = Number.parseInt(hex.slice(place * 2, place * 2 + 2), 16);
  }

  const decoded = decodeFrameState(bytes.buffer);
  expect(decoded.header.version).toBe(FRAME_VERSION);
  expect(decoded.header.step).toBe(1);
  expect(decoded.header.mode).toBe('running');
  expect(decoded.header.timeScale).toBe(65_535);
  expect(decoded.header.impulse).toBe(3);
  expect(decoded.header.sectionCount).toBe(7);
  expect(decoded.header.leakPerExposedContactPerStep).toBe(4_096);
  expect(decoded.header.dropped).toBe(false);
  expect(decoded.header.reducedMotion).toBe(false);

  expect(decoded.forms).toHaveLength(1);
  expect(decoded.forms[0]).toMatchObject({
    id: 1,
    formOrdinal: 2,
    layer: 0,
    controlled: true,
    focus: true,
    pulseCharging: true,
    // The fixture's Form stood at 120 by 210 with half a unit per step one way
    // and a whole unit the other, and its one step ran under a control of zero:
    // the steering phase's damper took a quarter of each before the position
    // advanced by what was left.
    x: 120.375,
    y: 209.25,
  });

  expect(decoded.ports).toHaveLength(4);
  expect(decoded.ports.map((port) => port.node)).toEqual([1, 2, 3, 4]);
  expect(decoded.ports[0]).toMatchObject({ kind: 0, open: true, member: true, x: 1600, y: 3200 });
  expect(decoded.ports[1].overloaded).toBe(true);
  expect(decoded.ports[1].member).toBe(false);
  // Every Port record names the layer its Node stands on, so the renderer
  // places it on its own plane rather than on the camera's.
  expect(decoded.ports.map((port) => port.layer)).toEqual([0, 0, 0, 1]);

  expect(decoded.routes).toHaveLength(1);
  expect(decoded.routes[0]).toMatchObject({ route: 1, tail: 1, head: 2, status: 2, age: 1 });

  expect(decoded.currents).toHaveLength(1);
  expect(decoded.currents[0]).toMatchObject({ id: 7, layer: 0, active: true, bright: true });
  expect(decoded.currents[0].strength).toBe(16_384);
  // The path the current runs along, resolved out of the frame's one flat
  // point array and handed over in units.
  expect(decoded.currents[0].path).toEqual([
    { x: 100, y: 100 },
    { x: 200, y: 300 },
  ]);
  expect(decoded.currents[0].width).toBe(16);

  // One flag per Port record, and no more: the bitset is 32 bytes whatever the
  // Port count, and the bits past the last record stand for nothing.
  expect(decoded.inside).toEqual([true, false, true, false]);
  expect(decoded.inside).toHaveLength(decoded.ports.length);
});

test('the three sections the renderer reads decode from their locked layouts', () => {
  // The core writes the five sections that stand on the Field it already
  // holds; pressures, cues, and the camera stand on state later goals own. The
  // renderer is what reads all three, so the decoder carries them now, and this
  // pins their layouts against a snapshot written to the document's own bytes.
  const layouts: { kind: number; count: number; width: number }[] = [
    { kind: 6, count: 2, width: 12 },
    { kind: 7, count: 2, width: 8 },
    { kind: 8, count: 1, width: 16 },
  ];
  const table = FRAME_HEADER_BYTES + layouts.length * 8;
  const span = layouts.reduce((total, one) => total + one.count * one.width, 0);
  const bytes = new Uint8Array(table + span);
  const view = new DataView(bytes.buffer);
  bytes.set(Array.from(FRAME_MAGIC, (character) => character.charCodeAt(0)));
  view.setUint16(4, FRAME_VERSION, true);
  bytes[20] = layouts.length;
  let at = table;
  layouts.forEach((one, place) => {
    const entry = FRAME_HEADER_BYTES + place * 8;
    bytes[entry] = one.kind;
    view.setUint16(entry + 2, one.count, true);
    view.setUint32(entry + 4, at, true);
    at += one.count * one.width;
  });

  let offset = table;
  // Two pressures: Fracture at its crisis stage against a Node, and a queued
  // Drift against a layer.
  for (const pressure of [
    { ordinal: 2, stage: 2, target: 1, queued: 0, level: 52_000, id: 7 },
    { ordinal: 5, stage: 0, target: 3, queued: 1, level: 1_024, id: 2 },
  ]) {
    bytes[offset] = pressure.ordinal;
    bytes[offset + 1] = pressure.stage;
    bytes[offset + 2] = pressure.target;
    bytes[offset + 3] = pressure.queued;
    view.setUint16(offset + 4, pressure.level, true);
    view.setUint32(offset + 8, pressure.id, true);
    offset += 12;
  }
  // Two cues: one the closed set names, and one past its end.
  for (const cue of [
    { kind: 6, a: 3, b: 11 },
    { kind: 200, a: 0, b: 0 },
  ]) {
    bytes[offset] = cue.kind;
    view.setUint16(offset + 2, cue.a, true);
    view.setUint32(offset + 4, cue.b, true);
    offset += 8;
  }
  view.setFloat32(offset, 1024.5, true);
  view.setFloat32(offset + 4, 2048.25, true);
  view.setFloat32(offset + 8, 1.5, true);
  bytes[offset + 12] = 3;

  const decoded = decodeFrameState(bytes.buffer);
  expect(decoded.pressures).toEqual([
    { ordinal: 2, stage: 'crisis', targetKind: 'node', queued: false, level: 52_000, target: 7 },
    { ordinal: 5, stage: 'signal', targetKind: 'layer', queued: true, level: 1_024, target: 2 },
  ]);
  expect(decoded.cues).toEqual([
    { kind: 6, name: 'break_occurred', a: 3, b: 11 },
    { kind: 200, name: null, a: 0, b: 0 },
  ]);
  expect(decoded.camera).toEqual({ x: 1024.5, y: 2048.25, zoom: 1.5, targetLayer: 3 });

  // A stage or a target kind the closed sets do not name is refused rather
  // than read as if it were one of them.
  const wrongStage = bytes.slice();
  wrongStage[table + 1] = 9;
  expect(() => decodeFrameState(wrongStage.buffer)).toThrow(FrameStateError);
  const wrongTarget = bytes.slice();
  wrongTarget[table + 2] = 9;
  expect(() => decodeFrameState(wrongTarget.buffer)).toThrow(FrameStateError);
});

test('a snapshot the shell cannot read is refused rather than half read', () => {
  expect(() => decodeFrameState(new Uint8Array(8).buffer)).toThrow(FrameStateError);
  const wrong = new Uint8Array(FRAME_HEADER_BYTES);
  wrong.set([0x46, 0x47, 0x46, 0x39]);
  expect(() => decodeFrameState(wrong.buffer)).toThrow(FrameStateError);
  const future = new Uint8Array(FRAME_HEADER_BYTES);
  future.set(Array.from(FRAME_MAGIC, (character) => character.charCodeAt(0)));
  new DataView(future.buffer).setUint16(4, FRAME_VERSION + 1, true);
  expect(() => decodeFrameState(future.buffer)).toThrow(FrameStateError);
});

test('a live frame decodes, and the dropped flag is the worker own accumulator', async () => {
  const session = openSession();
  await openRun(session);
  await session.send(neutralFrame(1, 1_000_000));
  const answer = await session.send(neutralFrame(2, 1_000_000 + 10_000_000));
  expect(answer).toHaveProperty('frame');
  if (!('frame' in answer)) return;
  expect(answer.frame.dropped).toBe(true);
  const decoded = decodeFrameState(answer.frame.buffer as ArrayBuffer);
  expect(decoded.header.dropped).toBe(true);
  expect(decoded.header.step).toBe(6);
  // A run stands on the authored chapter now, so the snapshot carries its
  // parts rather than the header alone.
  expect(decoded.header.sectionCount).toBeGreaterThan(0);
});

// ---------------------------------------------------------------------------
// Malformed messages, protocol version, and the closed error set
// ---------------------------------------------------------------------------

test('a message outside the envelope rules answers protocol, with the locked correlation', async () => {
  const session = openSession();
  const cases: [string, unknown, number][] = [
    ['not an object', 'init_run', 0],
    ['no version', { id: 4, cmd: 'init_run', body: {} }, 4],
    ['another version', { v: PROTOCOL_VERSION + 1, id: 5, cmd: 'init_run', body: {} }, 5],
    ['a version that is not a number', { v: String(PROTOCOL_VERSION), id: 6, cmd: 'init_run', body: {} }, 6],
    ['no correlation', { v: PROTOCOL_VERSION, cmd: 'init_run', body: {} }, 0],
    ['a correlation that is not a u32', { v: PROTOCOL_VERSION, id: -1, cmd: 'init_run', body: {} }, 0],
    ['an unknown command', { v: PROTOCOL_VERSION, id: 8, cmd: 'restore_state', body: {} }, 8],
    ['a command that is not a string', { v: PROTOCOL_VERSION, id: 9, cmd: 7, body: {} }, 9],
  ];
  for (const [named, message, re] of cases) {
    const answer = await session.raw(message);
    expect(answer.v, named).toBe(PROTOCOL_VERSION);
    expect(answer.re, named).toBe(re);
    expect(refusalOf(answer).code, named).toBe('protocol');
  }

  // A correlation that does not strictly increase is refused too.
  await session.raw({ v: PROTOCOL_VERSION, id: 40, cmd: 'init_run', body: {} });
  const repeated = await session.raw({
    v: PROTOCOL_VERSION,
    id: 40,
    cmd: 'init_run',
    body: {},
  });
  expect(refusalOf(repeated).detail).toEqual({ reason: 'correlation' });
});

test('every command refuses a body it cannot read, with the locked envelope', async () => {
  const session = openSession();
  await openRun(session);

  // A command valid only in Still Mode, which no run reaches yet.
  for (const cmd of ['queue_plan', 'undo_plan', 'commit_plan', 'set_focus']) {
    const error = refusalOf(await session.command(cmd, {}));
    expect(error.code, cmd).toBe('state');
    expect(error.detail, cmd).toEqual({ actual: 'running', expected: ['still'] });
  }

  // A command valid only before a run is loaded.
  expect(refusalOf(await session.command('init_run', { mode: 'new' })).code).toBe('state');
  expect(refusalOf(await session.command('import_run', { text: '{}' })).code).toBe('state');

  // Bodies of the wrong shape, the wrong type, and the wrong range.
  const wrong: [string, unknown, string][] = [
    ['export_run', { unexpected: 1 }, 'validation'],
    ['restore_checkpoint', {}, 'validation'],
    ['restore_checkpoint', { anchor_id: 'first' }, 'validation'],
    ['restore_checkpoint', { anchor_id: -1 }, 'validation'],
    ['restore_checkpoint', { anchor_id: 1, extra: 2 }, 'validation'],
    ['recover_branch', { anchor_id: 4294967296 }, 'validation'],
  ];
  for (const [cmd, body, code] of wrong) {
    expect(refusalOf(await session.command(cmd, body)).code, `${cmd} ${JSON.stringify(body)}`).toBe(
      code,
    );
  }
});

test('a frame refused for any locked reason answers instead of being acknowledged', async () => {
  const session = openSession();
  await openRun(session);
  await session.send({ ...neutralFrame(1, FRAME_US), advance_steps: 2 });

  const broken: [string, Partial<InputFrame> | Payload][] = [
    ['a repeated frame number', { seq: 1 }],
    ['a steering component that is never sent', { seq: 9, steer_x: -32768 }],
    ['a steering vector past its magnitude', { seq: 9, steer_x: 32000, steer_y: 32000 }],
    ['a wheel past its clamp', { seq: 9, wheel: 3001 }],
    ['a depth key outside its three values', { seq: 9, depth_key: 2 }],
    ['a step count past its cap', { seq: 9, advance_steps: 1801 }],
    ['a frame number of zero', { seq: 0 }],
    ['a pause that is not a flag', { seq: 9, pause: 1 }],
  ];
  for (const [named, overrides] of broken) {
    const answer = await session.send({ ...neutralFrame(9, 9 * FRAME_US), ...overrides } as InputFrame);
    expect(answer, named).toHaveProperty('refused');
    if ('refused' in answer) expect(answer.refused.code, named).toBe('validation');
  }

  // A key the frame's shape never declares, and a field missing from it.
  const extra = await session.send({
    ...neutralFrame(9, 9 * FRAME_US),
    bonus: 1,
  } as unknown as InputFrame);
  expect(extra).toHaveProperty('refused');
  const short = await session.command('input_frame', { seq: 9, t_us: 0 });
  expect(refusalOf(short).code).toBe('validation');

  // And the run is exactly where it was: a refused frame runs nothing.
  expect(JSON.parse(String((await exportOf(session)).text)).payload.field.now.step).toBe(2);
});

test('the pause level suspends the run and releasing it resumes the same run', async () => {
  const session = openSession();
  await openRun(session);
  await session.send({ ...neutralFrame(1, FRAME_US), advance_steps: 4 });
  const before = await exportOf(session);

  const paused = await session.send({ ...neutralFrame(2, 2 * FRAME_US), pause: true });
  expect(paused).toHaveProperty('frame');
  if ('frame' in paused) {
    expect(paused.frame.steps_run).toBe(0);
    const decoded = decodeFrameState(paused.frame.buffer as ArrayBuffer);
    expect(decoded.header.mode).toBe('suspended');
    expect(decoded.header.timeScale).toBe(0);
  }
  expect((await exportOf(session)).text).toBe(before.text);

  const released = await session.send({ ...neutralFrame(3, 3 * FRAME_US), advance_steps: 2 });
  expect(released).toHaveProperty('frame');
  if ('released' in released) return;
  const decoded = decodeFrameState(
    ('frame' in released ? released.frame.buffer : undefined) as ArrayBuffer,
  );
  expect(decoded.header.mode).toBe('running');
  expect(decoded.header.step).toBe(6);
});

test('releasing a pause owes no time, however long the pause lasted', async () => {
  const session = openSession();
  await openRun(session);
  // A frame at the render rate, then the pause, then a release ten seconds
  // later. A suspended run consumed none of that time, so the release frame is
  // the first frame again: it reads a gap of zero, runs nothing, and drops
  // nothing.
  await session.send(neutralFrame(1, 1_000_000));
  await session.send(neutralFrame(2, 1_000_000 + FRAME_US));
  const paused = await session.send({ ...neutralFrame(3, 1_000_000 + 2 * FRAME_US), pause: true });
  expect(paused).toHaveProperty('frame');

  const released = await session.send(neutralFrame(4, 1_000_000 + 10_000_000));
  expect(released).toHaveProperty('frame');
  if (!('frame' in released)) return;
  expect(released.frame.dropped).toBe(false);
  expect(released.frame.steps_run).toBe(0);
  expect(released.frame.remainder_us).toBe(0);

  // And the run picks the clock back up from there.
  const carried = await session.send(neutralFrame(5, 1_000_000 + 10_100_000));
  expect(carried).toHaveProperty('frame');
  if ('frame' in carried) {
    expect(carried.frame.steps_run).toBe(3);
    expect(carried.frame.dropped).toBe(false);
  }
});

test('two sessions agree byte for byte past the first autosave', async () => {
  // The autosave cadence writes into the payload's own metadata from step 900
  // on, so the determinism contract has to be read past that boundary.
  const played = async (): Promise<string> => {
    const session = openSession();
    await openRun(session);
    await session.send({ ...neutralFrame(1, FRAME_US), advance_steps: 1000 });
    await session.send({ ...neutralFrame(2, 2 * FRAME_US), advance_steps: 200 });
    const taken = String((await exportOf(session)).text);
    session.close();
    return taken;
  };

  const once = await played();
  const again = await played();
  expect(once).toBe(again);
  const payload = JSON.parse(once).payload;
  expect(payload.field.now.step).toBe(1200);
  expect(payload.anchors).toHaveLength(1);
  expect(payload.anchors[0].save_key).toBe(`${KEY}:auto:1`);
  expect(payload.anchors[0].step).toBe(1000);
});

test('every Form of the closed set opens on its own ordinal in the frame', async () => {
  // Two orders have to agree, and nothing but a test makes them: `FORM_IDS`,
  // which the opening surface offers the eight Forms in, and the ordinal the
  // frame's forms record carries — which the renderer reads to draw a Form's
  // own mark. They are declared in two languages, so this reads the second off
  // a real core opened on each of the first.
  for (const [place, form] of FORM_IDS.entries()) {
    const session = openSession();
    const opening = await session.command('init_run', { mode: 'new', run_id: KEY, form });
    expect(opening.ok).toBe(true);
    const answer = await session.send({ ...neutralFrame(1, FRAME_US), advance_steps: 1 });
    expect('frame' in answer, `${form} answers with a frame`).toBe(true);
    const carried = (answer as { frame: FrameEventBody }).frame;
    expect(carried.buffer, `${form} carries a render snapshot`).toBeTruthy();
    const state = decodeFrameState(carried.buffer as ArrayBuffer);
    const controlled = state.forms.find((held) => held.controlled);
    expect(controlled, `${form} stands a controlled Form`).toBeTruthy();
    expect(controlled?.formOrdinal, `${form} carries its own place`).toBe(place);
    session.close();
  }
});
