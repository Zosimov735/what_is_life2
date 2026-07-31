/**
 * The render snapshot's decoder.
 *
 * `docs/field-framework/ARCHITECTURE.md` freezes the snapshot's layout — a
 * 32-byte header at fixed offsets, an 8-byte section table entry per section,
 * and one record layout per section kind — so the encoder in the core and this
 * decoder cannot disagree. A fixture the core's own test pins holds them
 * together across the two languages.
 *
 * Everything here is render-only. The values are the one-way lossy conversions
 * the document locks, and nothing decoded here is ever sent back: the
 * authoritative state stays inside the core.
 */

/** The four ASCII bytes every snapshot opens with. */
export const FRAME_MAGIC = 'FGF1';

/** The snapshot format's own version. */
export const FRAME_VERSION = 1;

/** The header's width in bytes; the section table starts right after it. */
export const FRAME_HEADER_BYTES = 32;

/** One section table entry: kind, pad, count, offset. */
export const FRAME_SECTION_ENTRY_BYTES = 8;

/** The widest snapshot the timing contract allows across the boundary. */
export const FRAME_BUFFER_CAP = 32 * 1024;

/** The ten locked section kinds. */
export const FRAME_SECTION = {
  forms: 1,
  ports: 2,
  routes: 3,
  currents: 4,
  view: 5,
  pressures: 6,
  cues: 7,
  camera: 8,
  overlay: 9,
  currentPath: 10,
} as const;

/** The most path points one current carries into a frame. */
export const FRAME_PATH_POINTS_CAP = 32;

/** The six modes, in the order the header's byte numbers them. */
export const FRAME_MODES = [
  'running',
  'ramp_in',
  'still',
  'ramp_out',
  'suspended',
  'ended',
] as const;

export type FrameMode = (typeof FRAME_MODES)[number];

/** The header, field by field. */
export interface FrameHeader {
  version: number;
  /** Bit 0 still surface visible, bit 1 time dropped, bit 2 reduced motion. */
  flags: number;
  stillVisible: boolean;
  dropped: boolean;
  reducedMotion: boolean;
  step: number;
  /** Q0.16, saturating: full speed reads 65535. */
  timeScale: number;
  mode: FrameMode;
  cameraLayer: number;
  impulse: number;
  chapterIndex: number;
  objectiveOrdinal: number;
  sectionCount: number;
}

export interface FrameForm {
  id: number;
  formOrdinal: number;
  layer: number;
  controlled: boolean;
  focus: boolean;
  pulseCharging: boolean;
  /** A linked Form standing further from the Form it follows than its separation admits. */
  separated: boolean;
  x: number;
  y: number;
  vx: number;
  vy: number;
  /** Q0.16 of the stored-Charge cap. */
  charge: number;
  /** Q8.8 units. */
  radius: number;
}

export interface FramePort {
  node: number;
  kind: number;
  /** The layer this Node stands on; a Form's own Node carries its Form's. */
  layer: number;
  open: boolean;
  overloaded: boolean;
  member: boolean;
  shell: boolean;
  /**
   * Whether a queued `reshape_boundary` or `set_focus` would make this Node a
   * member. Read beside `member` it says whether the change would take the Node
   * in, leave it out, or leave it where it stands; no frame raises it while the
   * queue proposes no View.
   */
  proposedMember: boolean;
  /** Q0.16 of the stored-Charge cap. */
  charge: number;
  /** Units times 16. */
  x: number;
  y: number;
  /** Q0.16 of this Node's own threshold. */
  reserve: number;
}

export interface FrameRoute {
  route: number;
  tail: number;
  head: number;
  /** Q0.8 of the Route's own capacity. */
  flow: number;
  /**
   * 0 standing, 1 cut queued, 2 overloaded, 3 move queued, 4 proposed.
   *
   * The last two are previews: 3 marks a standing Route an entry would move an
   * end of, and 4 marks a record that is a proposal rather than a Route — one
   * per queued `connect` and per queued `redirect`, past the standing Routes,
   * carrying the endpoints the change proposes.
   */
  status: number;
  /** Steps since the Route formed, saturating. */
  age: number;
}

export interface FrameCurrent {
  id: number;
  layer: number;
  active: boolean;
  bright: boolean;
  phase: number;
  /** Q0.16 of the locked current-strength cap. */
  strength: number;
  /**
   * The current's own path, in units: the points the frame's flat array holds
   * for it, already resolved out of `path_offset` and `point_count`. An
   * authored path longer than the cap arrives decimated — both endpoints kept,
   * the interior on an even stride — which the document locks so the encoder
   * and the renderer draw the same shape.
   */
  path: { x: number; y: number }[];
  /** The band a Node has to stand within to receive, in whole units. */
  width: number;
}

/** The six pressures of the closed set, in the order the ordinal byte numbers them. */
export const FRAME_PRESSURE_IDS = [
  'drain',
  'noise',
  'fracture',
  'flood',
  'interference',
  'drift',
] as const;

export type FramePressureId = (typeof FRAME_PRESSURE_IDS)[number];

/** The four stages a pressure passes through, in the order the byte numbers them. */
export const FRAME_PRESSURE_STAGES = ['signal', 'pressure', 'crisis', 'resolution'] as const;

export type FramePressureStage = (typeof FRAME_PRESSURE_STAGES)[number];

/** What a pressure is aimed at, in the order the byte numbers it. */
export const FRAME_TARGET_KINDS = ['none', 'node', 'route', 'layer'] as const;

export type FrameTargetKind = (typeof FRAME_TARGET_KINDS)[number];

export interface FramePressure {
  /** The closed pressure set's own order. */
  ordinal: number;
  stage: FramePressureStage;
  targetKind: FrameTargetKind;
  queued: boolean;
  /** Q0.16 of the pressure's own range. */
  level: number;
  target: number;
}

/** The closed cue set, in the order the kind byte numbers it from 1. */
export const FRAME_CUE_KINDS = [
  'pulse_emitted',
  'charge_gathered',
  'port_opened',
  'route_formed',
  'route_cut',
  'break_occurred',
  'anchor_written',
  'collapse',
  'recovery',
  'depth_changed',
  'objective_complete',
  'interference_pushed',
] as const;

export type FrameCueKind = (typeof FRAME_CUE_KINDS)[number];

export interface FrameCue {
  /** The raw kind byte, 1 through 12. */
  kind: number;
  /** The kind's name, and none for a kind this build does not name. */
  name: FrameCueKind | null;
  a: number;
  b: number;
}

export interface FrameCamera {
  x: number;
  y: number;
  zoom: number;
  targetLayer: number;
}

/**
 * One window step of the Still Mode overlay: the low and high of the standing
 * View's baseline `q_I` envelope there.
 *
 * The section is present only while the run is `still`, and carries however
 * many window steps the envelope holds — which is none until the goal that
 * replays baselines computes one. An empty overlay is therefore a real
 * reading, and a different reading from a frame that carries no overlay
 * section at all: the first says the surface stands with nothing to show, and
 * the second says the run is not still.
 */
export interface FrameOverlayStep {
  lo: number;
  hi: number;
}

/** One decoded snapshot. Sections a build does not write come back empty. */
export interface FrameState {
  header: FrameHeader;
  forms: FrameForm[];
  ports: FramePort[];
  routes: FrameRoute[];
  currents: FrameCurrent[];
  /** One flag per Port record: a member of the standing inside. */
  inside: boolean[];
  pressures: FramePressure[];
  cues: FrameCue[];
  /** The camera the snapshot names, and none when it names none. */
  camera: FrameCamera | null;
  /**
   * The Still Mode overlay, and none at all on a frame that carries no
   * overlay section — which is every frame of a run that is not `still`.
   */
  overlay: FrameOverlayStep[] | null;
}

/** What a snapshot that cannot be read reports. */
export class FrameStateError extends Error {}

/**
 * Decodes one snapshot. A buffer that is not one — wrong marker, wrong
 * version, too short, or a section reaching past its end — is refused rather
 * than read as if it were.
 */
export function decodeFrameState(buffer: ArrayBuffer): FrameState {
  const bytes = new Uint8Array(buffer);
  if (bytes.length < FRAME_HEADER_BYTES) {
    throw new FrameStateError(`a snapshot is at least ${FRAME_HEADER_BYTES} bytes`);
  }
  const marker = String.fromCharCode(...bytes.subarray(0, 4));
  if (marker !== FRAME_MAGIC) {
    throw new FrameStateError(`a snapshot opens with ${FRAME_MAGIC}, not ${marker}`);
  }
  const view = new DataView(buffer);
  const version = view.getUint16(4, true);
  if (version !== FRAME_VERSION) {
    throw new FrameStateError(`a snapshot of version ${version} is not version ${FRAME_VERSION}`);
  }

  const flags = view.getUint16(6, true);
  const modeAt = bytes[14];
  if (modeAt >= FRAME_MODES.length) {
    throw new FrameStateError(`a snapshot names no mode ${modeAt}`);
  }
  const header: FrameHeader = {
    version,
    flags,
    stillVisible: (flags & 1) !== 0,
    dropped: (flags & 2) !== 0,
    reducedMotion: (flags & 4) !== 0,
    step: view.getUint32(8, true),
    timeScale: view.getUint16(12, true),
    mode: FRAME_MODES[modeAt],
    cameraLayer: bytes[15],
    impulse: bytes[16],
    chapterIndex: bytes[17],
    objectiveOrdinal: view.getUint16(18, true),
    sectionCount: bytes[20],
  };

  const decoded: FrameState = {
    header,
    forms: [],
    ports: [],
    routes: [],
    currents: [],
    inside: [],
    pressures: [],
    cues: [],
    camera: null,
    overlay: null,
  };
  const table = FRAME_HEADER_BYTES + header.sectionCount * FRAME_SECTION_ENTRY_BYTES;
  if (bytes.length < table) {
    throw new FrameStateError('a snapshot holds the section table it counts');
  }

  // A current's record names where its own points start in the frame's one
  // flat point array, and the array is a section of its own. The two are read
  // together after the table has been walked, because the table does not
  // promise which of them comes first.
  let currentsAt = -1;
  let currentsCount = 0;
  let pathAt = -1;
  let pathCount = 0;

  for (let place = 0; place < header.sectionCount; place += 1) {
    const at = FRAME_HEADER_BYTES + place * FRAME_SECTION_ENTRY_BYTES;
    const kind = bytes[at];
    const count = view.getUint16(at + 2, true);
    const offset = view.getUint32(at + 4, true);
    switch (kind) {
      case FRAME_SECTION.forms:
        decoded.forms = readForms(view, offset, count, bytes.length);
        break;
      case FRAME_SECTION.ports:
        decoded.ports = readPorts(view, offset, count, bytes.length);
        break;
      case FRAME_SECTION.routes:
        decoded.routes = readRoutes(view, offset, count, bytes.length);
        break;
      case FRAME_SECTION.currents:
        currentsAt = offset;
        currentsCount = count;
        break;
      case FRAME_SECTION.currentPath:
        pathAt = offset;
        pathCount = count;
        break;
      case FRAME_SECTION.view:
        decoded.inside = readInside(bytes, offset, count);
        break;
      case FRAME_SECTION.pressures:
        decoded.pressures = readPressures(view, offset, count, bytes.length);
        break;
      case FRAME_SECTION.cues:
        decoded.cues = readCues(view, offset, count, bytes.length);
        break;
      case FRAME_SECTION.camera:
        decoded.camera = readCamera(view, offset, count, bytes.length);
        break;
      case FRAME_SECTION.overlay:
        decoded.overlay = readOverlay(view, offset, count, bytes.length);
        break;
      default:
        // A section kind this build does not name is passed over rather than
        // guessed at: the goals that own the remaining kinds extend this
        // decoder with them.
        break;
    }
  }
  if (currentsAt >= 0) {
    const points = readPathPoints(view, pathAt, pathCount, bytes.length);
    decoded.currents = readCurrents(view, currentsAt, currentsCount, bytes.length, points);
  }

  // One flag per Port record: the bitset is 32 bytes whatever the Port count,
  // and the bits past the last record stand for nothing.
  decoded.inside = decoded.inside.slice(0, decoded.ports.length);
  return decoded;
}

function span(offset: number, count: number, width: number, length: number): void {
  if (offset + count * width > length) {
    throw new FrameStateError('a section reaches past the end of the snapshot');
  }
}

function readForms(view: DataView, offset: number, count: number, length: number): FrameForm[] {
  span(offset, count, 24, length);
  const found: FrameForm[] = [];
  for (let place = 0; place < count; place += 1) {
    const at = offset + place * 24;
    const flags = view.getUint8(at + 3);
    found.push({
      id: view.getUint8(at),
      formOrdinal: view.getUint8(at + 1),
      layer: view.getUint8(at + 2),
      controlled: (flags & 1) !== 0,
      focus: (flags & 2) !== 0,
      pulseCharging: (flags & 4) !== 0,
      separated: (flags & 8) !== 0,
      x: view.getFloat32(at + 4, true),
      y: view.getFloat32(at + 8, true),
      vx: view.getFloat32(at + 12, true),
      vy: view.getFloat32(at + 16, true),
      charge: view.getUint16(at + 20, true),
      radius: view.getUint16(at + 22, true),
    });
  }
  return found;
}

function readPorts(view: DataView, offset: number, count: number, length: number): FramePort[] {
  span(offset, count, 16, length);
  const found: FramePort[] = [];
  for (let place = 0; place < count; place += 1) {
    const at = offset + place * 16;
    const flags = view.getUint8(at + 5);
    found.push({
      node: view.getUint32(at, true),
      kind: view.getUint8(at + 4),
      layer: view.getUint8(at + 14),
      open: (flags & 1) !== 0,
      overloaded: (flags & 2) !== 0,
      member: (flags & 4) !== 0,
      shell: (flags & 8) !== 0,
      proposedMember: (flags & 16) !== 0,
      charge: view.getUint16(at + 6, true),
      x: view.getUint16(at + 8, true),
      y: view.getUint16(at + 10, true),
      reserve: view.getUint16(at + 12, true),
    });
  }
  return found;
}

function readRoutes(view: DataView, offset: number, count: number, length: number): FrameRoute[] {
  span(offset, count, 16, length);
  const found: FrameRoute[] = [];
  for (let place = 0; place < count; place += 1) {
    const at = offset + place * 16;
    found.push({
      route: view.getUint32(at, true),
      tail: view.getUint32(at + 4, true),
      head: view.getUint32(at + 8, true),
      flow: view.getUint8(at + 12),
      status: view.getUint8(at + 13),
      age: view.getUint16(at + 14, true),
    });
  }
  return found;
}

function readCurrents(
  view: DataView,
  offset: number,
  count: number,
  length: number,
  points: { x: number; y: number }[],
): FrameCurrent[] {
  span(offset, count, 12, length);
  const found: FrameCurrent[] = [];
  for (let place = 0; place < count; place += 1) {
    const at = offset + place * 12;
    const flags = view.getUint8(at + 3);
    const pathAt = view.getUint16(at + 8, true);
    const pathCount = view.getUint8(at + 10);
    if (pathCount > FRAME_PATH_POINTS_CAP) {
      throw new FrameStateError(`a current carries ${pathCount} path points, past the cap`);
    }
    if (pathAt + pathCount > points.length) {
      throw new FrameStateError('a current path reaches past the point array');
    }
    found.push({
      id: view.getUint16(at, true),
      layer: view.getUint8(at + 2),
      active: (flags & 1) !== 0,
      bright: (flags & 2) !== 0,
      phase: view.getUint16(at + 4, true),
      strength: view.getUint16(at + 6, true),
      path: points.slice(pathAt, pathAt + pathCount),
      width: view.getUint8(at + 11),
    });
  }
  return found;
}

/** The flat point array, in units: the records hold units times 16 per axis. */
function readPathPoints(
  view: DataView,
  offset: number,
  count: number,
  length: number,
): { x: number; y: number }[] {
  if (offset < 0 || count === 0) return [];
  span(offset, count, 4, length);
  const found: { x: number; y: number }[] = [];
  for (let place = 0; place < count; place += 1) {
    const at = offset + place * 4;
    found.push({ x: view.getUint16(at, true) / 16, y: view.getUint16(at + 2, true) / 16 });
  }
  return found;
}

function readPressures(
  view: DataView,
  offset: number,
  count: number,
  length: number,
): FramePressure[] {
  span(offset, count, 12, length);
  const found: FramePressure[] = [];
  for (let place = 0; place < count; place += 1) {
    const at = offset + place * 12;
    const ordinal = view.getUint8(at);
    const stage = view.getUint8(at + 1);
    const targetKind = view.getUint8(at + 2);
    if (ordinal >= FRAME_PRESSURE_IDS.length) {
      throw new FrameStateError(`a snapshot names no pressure ${ordinal}`);
    }
    if (stage >= FRAME_PRESSURE_STAGES.length) {
      throw new FrameStateError(`a snapshot names no stage ${stage}`);
    }
    if (targetKind >= FRAME_TARGET_KINDS.length) {
      throw new FrameStateError(`a snapshot names no target kind ${targetKind}`);
    }
    found.push({
      ordinal,
      stage: FRAME_PRESSURE_STAGES[stage],
      targetKind: FRAME_TARGET_KINDS[targetKind],
      queued: view.getUint8(at + 3) !== 0,
      level: view.getUint16(at + 4, true),
      target: view.getUint32(at + 8, true),
    });
  }
  return found;
}

function readCues(view: DataView, offset: number, count: number, length: number): FrameCue[] {
  span(offset, count, 8, length);
  const found: FrameCue[] = [];
  for (let place = 0; place < count; place += 1) {
    const at = offset + place * 8;
    const kind = view.getUint8(at);
    found.push({
      kind,
      // The kinds are numbered from 1; a kind past the closed set is carried as
      // an unnamed one rather than refused, because a cue the renderer cannot
      // name is a cue it can pass over.
      name: FRAME_CUE_KINDS[kind - 1] ?? null,
      a: view.getUint16(at + 2, true),
      b: view.getUint32(at + 4, true),
    });
  }
  return found;
}

function readCamera(
  view: DataView,
  offset: number,
  count: number,
  length: number,
): FrameCamera | null {
  span(offset, count, 16, length);
  if (count === 0) return null;
  return {
    x: view.getFloat32(offset, true),
    y: view.getFloat32(offset + 4, true),
    zoom: view.getFloat32(offset + 8, true),
    targetLayer: view.getUint8(offset + 12),
  };
}

/**
 * The Still Mode overlay: one low and high per window step, in the same
 * one-way `f32` every other derived quantity crosses as.
 */
function readOverlay(
  view: DataView,
  offset: number,
  count: number,
  length: number,
): FrameOverlayStep[] {
  span(offset, count, 8, length);
  const found: FrameOverlayStep[] = [];
  for (let place = 0; place < count; place += 1) {
    const at = offset + place * 8;
    found.push({ lo: view.getFloat32(at, true), hi: view.getFloat32(at + 4, true) });
  }
  return found;
}

/** The 32-byte bitset: bit i is set when Port record i is a member. */
function readInside(bytes: Uint8Array, offset: number, count: number): boolean[] {
  span(offset, count, 32, bytes.length);
  if (count === 0) return [];
  const found: boolean[] = [];
  for (let place = 0; place < 256; place += 1) {
    found.push((bytes[offset + (place >> 3)] & (1 << (place & 7))) !== 0);
  }
  return found;
}
