/**
 * A development-only stand-in for a populated Field.
 *
 * **Not part of the game.** Nothing here is authoritative, nothing here reaches
 * the worker or the core, and nothing here ships: the module is reached by a
 * dynamic import behind `import.meta.env.DEV`, so the production build drops it
 * along with the branch that names it.
 *
 * Why it exists: authored content arrives with a later goal, so a run today
 * stands on an empty Field and the renderer has nothing to draw. The
 * architecture document gives the renderer exactly one input — a decoded
 * `FrameState` — so the smallest honest way to drive a populated Field in
 * development is to write snapshots in the locked byte layout and read them
 * back through the shared decoder, which is what this does. The renderer cannot
 * tell these from the worker's own, and it is not asked to.
 *
 * Reach it by opening the local preview with the marker `field_fixture` in the
 * query string, which `App` reads.
 *
 * One thing to know before changing it: this is a second implementation of the
 * frame layout, and a second implementation can drift from the first. What
 * keeps it honest is that nothing here is read directly — every snapshot goes
 * out through the shared decoder, so a layout mistake here surfaces as a
 * decoder fault rather than as a quietly wrong picture. The authority on the
 * layout is neither this file nor the decoder but the cross-language hex
 * fixture the core's own test pins, which the renderer tests also read.
 */

import { decodeFrameState, type FrameState } from '../../../worker/src/frame-state';
import type { FramePair } from './worker-client';

/** The locked header width, and the section table entry width. */
const HEADER_BYTES = 32;
const ENTRY_BYTES = 8;

/** Fixed steps per second, exactly as the timing contract sets it. */
const STEP_RATE = 30;

/** Rendered frames per second, the target the same contract sets. */
const RENDER_RATE = 60;

/** Where the Field sits, in units, and where the controlled Form circles. */
const MIDDLE = 2150;

/** Where the cluster the standing View wraps sits, in units. */
const CLUSTER_MIDDLE = { x: 2150, y: 1860 };

/** How many Ports stand along the chain the bright current delivers to. */
const CHAIN = 8;

/** How many Ports stand in the cluster the standing View wraps. */
const CLUSTER = 6;

/** How many Ports stand outside both. */
const OUTLYING = 4;

/** How many Forms the player does not steer, one per layer below the first. */
const DRIFTING_FORMS = 3;

/** Which plane each group of Ports stands on. */
const CHAIN_LAYER = 0;
const CLUSTER_LAYER = 1;
const OUTLYING_LAYERS = [0, 1, 0, 2];

/** The first Node id of each group, and of the two Form-kind Nodes. */
const CHAIN_FIRST = 1;
const CLUSTER_FIRST = CHAIN_FIRST + CHAIN;
const OUTLYING_FIRST = CLUSTER_FIRST + CLUSTER;
const FORM_NODE_FIRST = OUTLYING_FIRST + OUTLYING;
const PORT_COUNT = CHAIN + CLUSTER + OUTLYING + 1 + DRIFTING_FORMS;

/** How long the Form spends on one layer before it changes depth. */
const DEPTH_PERIOD = 240;

/** How long one turn of the pressure schedule takes. */
const PRESSURE_PERIOD = 600;

interface Point {
  x: number;
  y: number;
}

/**
 * Where the chain's Ports stand: a long shallow arc, well clear of the cluster,
 * so the band the bright current draws along it reads as its own run rather
 * than as part of the standing inside.
 */
function chainAt(index: number): Point {
  return {
    x: 1500 + index * 162,
    y: 2600 - index * 36 + 44 * Math.sin(index * 1.1),
  };
}

/** Where the cluster's Ports stand: a ring the standing inside wraps. */
function clusterAt(index: number): Point {
  const angle = (index / CLUSTER) * Math.PI * 2;
  return {
    x: CLUSTER_MIDDLE.x + Math.cos(angle) * 155,
    y: CLUSTER_MIDDLE.y + Math.sin(angle) * 155,
  };
}

/** Where the outlying Ports stand. */
function outlyingAt(index: number): Point {
  const places: Point[] = [
    { x: 1560, y: 1760 },
    { x: 2790, y: 1930 },
    { x: 1720, y: 2980 },
    { x: 2860, y: 2760 },
  ];
  return places[index % places.length];
}

/** Where the controlled Form stands at a step, and how fast it is going. */
function steeredAt(step: number): { pos: Point; vel: Point } {
  const turn = step * 0.011;
  const reach = 470 + 60 * Math.sin(step * 0.004);
  return {
    pos: { x: MIDDLE + Math.cos(turn) * reach, y: 2230 + Math.sin(turn) * reach * 0.74 },
    vel: { x: -Math.sin(turn) * reach * 0.011, y: Math.cos(turn) * reach * 0.74 * 0.011 },
  };
}

/**
 * Where one of the Forms the player does not steer stands. They drift close
 * together, one per layer, so the planes below read as a series receding rather
 * than as one Form that happens to be small.
 */
function driftingAt(index: number, step: number): { pos: Point; vel: Point } {
  const turn = step * 0.006 + 2 + index * 1.4;
  return {
    pos: {
      x: 2700 + index * 60 + Math.cos(turn) * (170 + index * 40),
      y: 2520 + index * 40 + Math.sin(turn) * (130 + index * 30),
    },
    vel: { x: -Math.sin(turn) * 1.1, y: Math.cos(turn) * 0.9 },
  };
}

/**
 * Every Form at a step: the one the player steers, and one standing on each of
 * the three layers below the shallowest.
 */
function formsAt(step: number): {
  id: number;
  ordinal: number;
  layer: number;
  controlled: boolean;
  motion: { pos: Point; vel: Point };
}[] {
  const forms = [
    { id: 1, ordinal: 4, layer: depthAt(step), controlled: true, motion: steeredAt(step) },
  ];
  for (let index = 0; index < DRIFTING_FORMS; index += 1) {
    forms.push({
      id: index + 2,
      ordinal: index + 1,
      layer: index + 1,
      controlled: false,
      motion: driftingAt(index, step),
    });
  }
  return forms;
}

/**
 * The layer the controlled Form stands on, and so the layer the camera reads
 * from: it changes depth on a turn, and the second Form and the second current
 * stand deeper than both, so a plane below is always there to be read.
 */
function depthAt(step: number): number {
  return Math.floor(step / DEPTH_PERIOD) % 2;
}

/** A Node's stored Charge at a step, as the Q0.16 fraction the record holds. */
function chargeAt(node: number, step: number): number {
  if (node < CLUSTER_FIRST) {
    // The travelling rise the bright current delivers: each Port along the
    // chain gains a little after the one before it, so the band moves.
    const index = node - CHAIN_FIRST;
    // A current delivers every step it is active, and Drain pulls the other
    // way, so each Port along the chain rises and falls on its own phase.
    const wave = Math.cos(step * 0.075 - index * 0.72);
    return Math.round(900 + 420 * Math.sin(step * 0.11 - index * 0.5) + Math.max(0, wave) ** 3 * 2400);
  }
  if (node < OUTLYING_FIRST) {
    const index = node - CLUSTER_FIRST;
    return Math.round(2200 + 1500 * (1 + Math.sin(step * 0.02 + index)) + index * 260);
  }
  if (node < FORM_NODE_FIRST) {
    const index = node - OUTLYING_FIRST;
    // The third outlying Node holds more than its own threshold.
    return index === 2 ? 9000 : Math.round(260 + 180 * index);
  }
  return Math.round(1200 + 900 * (1 + Math.sin(step * 0.03)));
}

/** The Q0.8 flow a Route carries at a step. */
function flowAt(route: number, step: number): number {
  if (route >= 9) return 0;
  return Math.round(60 + 90 * (1 + Math.sin(step * 0.05 + route)));
}

/** Every Port's place at a step, in units, in Node id order. */
function portPlaces(step: number): Point[] {
  const places: Point[] = [];
  for (let index = 0; index < CHAIN; index += 1) places.push(chainAt(index));
  for (let index = 0; index < CLUSTER; index += 1) places.push(clusterAt(index));
  for (let index = 0; index < OUTLYING; index += 1) places.push(outlyingAt(index));
  for (const form of formsAt(step)) places.push(form.motion.pos);
  return places;
}

/** The Routes, tail to head, in the order their ids run. */
const ROUTES: { tail: number; head: number }[] = [
  // Across the cluster rather than around it, so what is connected to what and
  // where the boundary sits stay two separate readings on the surface.
  { tail: CLUSTER_FIRST, head: CLUSTER_FIRST + 2 },
  { tail: CLUSTER_FIRST + 2, head: CLUSTER_FIRST + 4 },
  { tail: CLUSTER_FIRST + 4, head: CLUSTER_FIRST },
  { tail: CLUSTER_FIRST + 1, head: CLUSTER_FIRST + 3 },
  { tail: CLUSTER_FIRST + 3, head: CLUSTER_FIRST + 5 },
  { tail: CLUSTER_FIRST + 5, head: CLUSTER_FIRST + 1 },
  { tail: CLUSTER_FIRST, head: OUTLYING_FIRST },
  { tail: CLUSTER_FIRST + 3, head: OUTLYING_FIRST + 1 },
  { tail: CHAIN_FIRST + 2, head: CHAIN_FIRST + 3 },
  { tail: CHAIN_FIRST + 4, head: CHAIN_FIRST + 5 },
  { tail: OUTLYING_FIRST, head: OUTLYING_FIRST + 2 },
  { tail: OUTLYING_FIRST + 1, head: OUTLYING_FIRST + 3 },
];

/** Which plane a Node stands on; a Form's own Node carries its Form's. */
function portLayer(node: number, step: number): number {
  if (node < CLUSTER_FIRST) return CHAIN_LAYER;
  if (node < OUTLYING_FIRST) return CLUSTER_LAYER;
  if (node < FORM_NODE_FIRST) return OUTLYING_LAYERS[node - OUTLYING_FIRST];
  return formsAt(step)[node - FORM_NODE_FIRST].layer;
}

/** Which Node ids stand inside the authoritative physical compartment. */
function isMember(node: number): boolean {
  return node >= CLUSTER_FIRST && node < OUTLYING_FIRST;
}

/** A deliberately different passive View, so the V2 split is visible. */
function isViewed(node: number): boolean {
  return (
    (node >= CLUSTER_FIRST && node < CLUSTER_FIRST + 3) || node === OUTLYING_FIRST + 1
  );
}

/** Which members have a Route crossing out of the inside. */
function isShell(node: number): boolean {
  if (!isMember(node)) return false;
  return ROUTES.some(
    (route) =>
      (route.tail === node && !isMember(route.head)) ||
      (route.head === node && !isMember(route.tail)),
  );
}

/** How long one turn of the Pulse the stand-in scripts takes. */
const PULSE_PERIOD = 60;

/** How many steps of that turn the scripted Pulse spends charging. */
const PULSE_CHARGING = 24;

/** The locked reach of an empty release, raw: 8 units. */
const PULSE_RADIUS_BASE = 8 * 65536;

/** What each raw unit of charge adds to the locked reach. */
const PULSE_RADIUS_PER_CHARGE = 184;

/** The locked share a held step adds to the charge, raw. */
const PULSE_CHARGE_STEP = 2048;

/**
 * How far the controlled Form's Pulse has reached at a step, as the Q8.8 units
 * the record carries.
 *
 * Scripted, like everything else here — but scripted on the locked rule rather
 * than beside it: the charge is the locked share per held step and the reach is
 * `524288 + 184 × charge`, so the picture the stand-in draws is the picture the
 * core would send for the same hold. The reach holds through the step the
 * release lands on, which is the step the emitted reach belongs to.
 */
function pulseReachAt(step: number): number {
  const through = step % PULSE_PERIOD;
  if (through > PULSE_CHARGING) return 0;
  const charge = Math.min(65536, through * PULSE_CHARGE_STEP);
  return (PULSE_RADIUS_BASE + PULSE_RADIUS_PER_CHARGE * charge) >> 8;
}

/**
 * The cues a step raises.
 *
 * The Pulse's own four ride the turn its charging flag runs on: the flag stands
 * for the first stretch of every turn, so the release lands the step after it
 * drops. Every other release reaches something and the one between them reaches
 * nothing, because telling those two apart — by the ring, and by the sound — is
 * what the Pulse's feedback has to carry.
 */
function cuesAt(step: number): { kind: number; a: number; b: number }[] {
  const raised: { kind: number; a: number; b: number }[] = [];
  if (step % PRESSURE_PERIOD === 440) raised.push({ kind: 6, a: 0, b: CLUSTER_FIRST + 2 });
  if (step % DEPTH_PERIOD === 0 && step > 0) raised.push({ kind: 10, a: 0, b: 0 });
  if (step % PULSE_PERIOD === PULSE_CHARGING) {
    // The locked payloads: `b` is the Node the cue stands at, `a` is the
    // emitted reach for cue 1 and the gathered total in 1/16-unit ticks for
    // cue 2.
    raised.push({ kind: 1, a: pulseReachAt(step), b: FORM_NODE_FIRST });
    if (step % (PULSE_PERIOD * 2) === PULSE_CHARGING) {
      raised.push({ kind: 2, a: 320, b: FORM_NODE_FIRST });
    }
  }
  if (step % 180 === 84) raised.push({ kind: 3, a: 0, b: CHAIN_FIRST + 6 });
  if (step % 240 === 144) raised.push({ kind: 12, a: 1, b: FORM_NODE_FIRST });
  return raised;
}

/** The pressures standing at a step: one steady reading and one that turns. */
function pressuresAt(step: number): {
  ordinal: number;
  stage: number;
  targetKind: number;
  level: number;
  target: number;
}[] {
  const standing = [
    {
      ordinal: 0,
      stage: 0,
      targetKind: 0,
      level: Math.round(9000 + 5000 * (1 + Math.sin(step * 0.01))),
      target: 0,
    },
  ];
  const turn = step % PRESSURE_PERIOD;
  if (turn >= 380) {
    const stage = turn < 440 ? 1 : turn < 540 ? 2 : 3;
    const level = turn < 440 ? 22000 : turn < 540 ? 52000 : 18000;
    standing.push({ ordinal: 2, stage, targetKind: 1, level, target: CLUSTER_FIRST + 2 });
  }
  return standing;
}

/** Writes one snapshot in the locked layout. */
function encode(step: number): ArrayBuffer {
  const layer = depthAt(step);
  const places = portPlaces(step);
  const cues = cuesAt(step);
  const pressures = pressuresAt(step);
  const forms = formsAt(step);
  const currents = [
    {
      id: 1,
      layer: CHAIN_LAYER,
      active: true,
      bright: true,
      strength: 41000,
      width: 40,
      path: Array.from({ length: CHAIN }, (_, index) => chainAt(index)),
    },
    {
      id: 2,
      layer: 2,
      active: true,
      bright: false,
      strength: 17000,
      width: 30,
      path: [
        { x: 2500, y: 2400 },
        { x: 2760, y: 2560 },
        { x: 3020, y: 2620 },
      ],
    },
    {
      id: 3,
      layer: 3,
      active: true,
      bright: false,
      strength: 9000,
      width: 24,
      path: [
        { x: 2450, y: 2900 },
        { x: 2900, y: 2820 },
      ],
    },
  ];
  const pathPoints = currents.flatMap((current) => current.path);

  const sections: { kind: number; count: number; width: number }[] = [
    { kind: 1, count: forms.length, width: 24 },
    { kind: 2, count: PORT_COUNT, width: 16 },
    { kind: 3, count: ROUTES.length, width: 16 },
    { kind: 4, count: currents.length, width: 12 },
    { kind: 5, count: 1, width: 32 },
    { kind: 6, count: pressures.length, width: 12 },
    { kind: 7, count: cues.length, width: 8 },
    { kind: 10, count: pathPoints.length, width: 4 },
  ].filter((section) => section.count > 0);

  const table = HEADER_BYTES + sections.length * ENTRY_BYTES;
  const body = sections.reduce((total, section) => total + section.count * section.width, 0);
  const buffer = new ArrayBuffer(table + body);
  const view = new DataView(buffer);
  const bytes = new Uint8Array(buffer);

  bytes.set([0x46, 0x47, 0x46, 0x31]);
  view.setUint16(4, 2, true);
  view.setUint16(6, 0, true);
  view.setUint32(8, step, true);
  view.setUint16(12, 65535, true);
  bytes[14] = 0;
  bytes[15] = layer;
  bytes[16] = 3;
  bytes[17] = 0;
  view.setUint16(18, 0, true);
  bytes[20] = sections.length;

  let at = table;
  sections.forEach((section, place) => {
    const entry = HEADER_BYTES + place * ENTRY_BYTES;
    bytes[entry] = section.kind;
    view.setUint16(entry + 2, section.count, true);
    view.setUint32(entry + 4, at, true);
    at += section.count * section.width;
  });

  let offset = table;

  // Forms.
  for (const form of forms) {
    bytes[offset] = form.id;
    bytes[offset + 1] = form.ordinal;
    bytes[offset + 2] = form.layer;
    bytes[offset + 3] =
      (form.controlled ? 1 : 0) |
      (form.controlled && Math.floor(step / 150) % 2 === 0 ? 2 : 0) |
      (form.controlled && step % PULSE_PERIOD < PULSE_CHARGING ? 4 : 0);
    view.setFloat32(offset + 4, form.motion.pos.x, true);
    view.setFloat32(offset + 8, form.motion.pos.y, true);
    view.setFloat32(offset + 12, form.motion.vel.x, true);
    view.setFloat32(offset + 16, form.motion.vel.y, true);
    view.setUint16(
      offset + 20,
      form.controlled ? Math.round(14000 + 11000 * (1 + Math.sin(step * 0.02))) : 6000,
      true,
    );
    view.setUint16(offset + 22, form.controlled ? pulseReachAt(step) : 0, true);
    offset += 24;
  }

  // Ports.
  for (let index = 0; index < PORT_COUNT; index += 1) {
    const node = CHAIN_FIRST + index;
    const place = places[index];
    const charge = chargeAt(node, step);
    const kind =
      node >= FORM_NODE_FIRST ? 3 : node === CLUSTER_FIRST + 1 ? 1 : node === CLUSTER_FIRST + 4 ? 2 : 0;
    view.setUint32(offset, node, true);
    bytes[offset + 4] = kind;
    bytes[offset + 5] =
      1 |
      (node === OUTLYING_FIRST + 2 ? 2 : 0) |
      (isMember(node) ? 4 : 0) |
      (isShell(node) ? 8 : 0);
    view.setUint16(offset + 6, Math.min(65535, charge), true);
    view.setUint16(offset + 8, Math.round(place.x * 16), true);
    view.setUint16(offset + 10, Math.round(place.y * 16), true);
    view.setUint16(offset + 12, node >= FORM_NODE_FIRST ? 26000 : 0, true);
    bytes[offset + 14] = portLayer(node, step);
    bytes[offset + 15] = 0;
    offset += 16;
  }

  // Routes.
  ROUTES.forEach((route, index) => {
    const id = index + 1;
    view.setUint32(offset, id, true);
    view.setUint32(offset + 4, route.tail, true);
    view.setUint32(offset + 8, route.head, true);
    bytes[offset + 12] = Math.min(255, flowAt(id, step));
    bytes[offset + 13] = id === 11 ? 2 : id === 12 ? 1 : 0;
    view.setUint16(offset + 14, Math.min(65535, step), true);
    offset += 16;
  });

  // Currents, each naming where its own points start in the flat array.
  let pathAt = 0;
  for (const current of currents) {
    view.setUint16(offset, current.id, true);
    bytes[offset + 2] = current.layer;
    bytes[offset + 3] = (current.active ? 1 : 0) | (current.bright ? 2 : 0);
    view.setUint16(offset + 4, step % 1024, true);
    view.setUint16(offset + 6, current.strength, true);
    view.setUint16(offset + 8, pathAt, true);
    bytes[offset + 10] = current.path.length;
    bytes[offset + 11] = Math.min(255, current.width);
    pathAt += current.path.length;
    offset += 12;
  }

  // The passive observation View, one bit per Port record and independent of
  // the physical membership flags above.
  for (let index = 0; index < PORT_COUNT; index += 1) {
    if (isViewed(CHAIN_FIRST + index)) bytes[offset + (index >> 3)] |= 1 << (index & 7);
  }
  offset += 32;

  // Pressures.
  for (const pressure of pressures) {
    bytes[offset] = pressure.ordinal;
    bytes[offset + 1] = pressure.stage;
    bytes[offset + 2] = pressure.targetKind;
    bytes[offset + 3] = 0;
    view.setUint16(offset + 4, Math.min(65535, pressure.level), true);
    view.setUint16(offset + 6, 0, true);
    view.setUint32(offset + 8, pressure.target, true);
    offset += 12;
  }

  // Cues.
  for (const cue of cues) {
    bytes[offset] = cue.kind;
    bytes[offset + 1] = 0;
    view.setUint16(offset + 2, cue.a, true);
    view.setUint32(offset + 4, cue.b, true);
    offset += 8;
  }

  // The flat point array every current's path indexes into.
  for (const point of pathPoints) {
    view.setUint16(offset, Math.round(point.x * 16), true);
    view.setUint16(offset + 2, Math.round(point.y * 16), true);
    offset += 4;
  }

  return buffer;
}

/** One snapshot of the stand-in Field, read back through the shared decoder. */
export function fixtureSnapshot(step: number): FrameState {
  return decodeFrameState(encode(Math.max(0, Math.round(step))));
}

/** The pair the stand-in hands out, and how far through the run it is. */
const pair: FramePair = { previous: null, next: null, alpha: 0 };
let elapsed = 0;
let standing = -1;

/**
 * The frame pair a development surface draws.
 *
 * It advances one step per two rendered frames — the locked step rate against
 * the render target — counted rather than clocked, so a surface the platform
 * stops rendering picks up where it left off rather than jumping. That is the
 * same shape the real pump has: a hidden tab advances nothing.
 */
export function fixtureFrames(): FramePair {
  elapsed += STEP_RATE / RENDER_RATE;
  return frameAt();
}

/**
 * Places the stand-in at a step, so a state a long way into the turn — a
 * pressure at its crisis stage, a depth change, a Route under overload — can be
 * looked at without waiting for it to come round.
 */
export function fixtureSeek(step: number): void {
  elapsed = Math.max(0, step);
  standing = -1;
  pair.previous = null;
  pair.next = null;
}

function frameAt(): FramePair {
  const step = Math.floor(elapsed);
  if (step !== standing) {
    pair.previous = pair.next ?? fixtureSnapshot(step - 1);
    pair.next = fixtureSnapshot(step);
    standing = step;
  }
  pair.alpha = elapsed - step;
  return pair;
}
