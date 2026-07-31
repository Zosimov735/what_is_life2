/**
 * The locked graphite direction, as numbers.
 *
 * `docs/field-framework/SPEC.md` fixes the direction: a dark graphite
 * atmosphere carrying only currents, geometric Forms, trails, and soft
 * particles, and depicting nothing. Everything a viewer reads is therefore
 * carried by hue, brightness, motion, and size — never by text, and never by a
 * figure standing for a quantity.
 *
 * One hue per reading, so two readings never compete for the same one:
 *
 * | Reading | Hue |
 * |---|---|
 * | Charge | warm amber |
 * | Current | pale cyan, bright; slate blue, ordinary |
 * | The controlled Form | near-white, the brightest thing on the surface |
 * | Every other Form | cool graphite |
 * | Route | graphite, warmed by flow, red under overload |
 * | Boundary | pale violet |
 * | Pressure | one hue per pressure of the closed set |
 *
 * Colours are technical values rather than player-facing copy, so they are
 * written here rather than authored in the catalog.
 */

/** A colour as the 24-bit integer both engines take. */
export type Tone = number;

/** The backdrop, matching the shell's own graphite surface. */
export const BACKDROP: Tone = 0x0b0c0e;

/** The backdrop at the edge of the surface, which the middle is lifted out of. */
export const BACKDROP_EDGE: Tone = 0x06070a;

/** The haze a layer behind the camera's own is read through. */
export const HAZE: Tone = 0x11151b;

/**
 * What a mark on another plane is mixed toward. A plane further off loses
 * contrast rather than light: mixing toward the backdrop would take it away
 * altogether, and a Field whose deeper layers cannot be seen at all says
 * nothing about depth.
 */
export const HAZE_TINT: Tone = 0x3c4552;

/** Charge, from the least a Node can hold to the most. */
export const CHARGE_LOW: Tone = 0x6b4f2a;
export const CHARGE_HIGH: Tone = 0xffd39a;
export const CHARGE_CORE: Tone = 0xffa93a;

/** A Node that holds more than its own threshold. */
export const OVERLOAD: Tone = 0xe0684f;

/** A Port, before any Charge lifts it. */
export const PORT_BASE: Tone = 0x4e5865;

/** A Route, and the states its record can report. */
export const ROUTE_BASE: Tone = 0x39424e;
export const ROUTE_FLOW: Tone = 0xa9c2d6;
export const ROUTE_CUT_QUEUED: Tone = 0x6b5f4a;

/**
 * The queue's own register: what a proposed change would put on the Field, and
 * what a standing Route the queue would move reads as while it stands.
 *
 * A preview is deliberately a different hue from anything committed rather than
 * a dimmer version of it — nothing here has been applied, a refused commit
 * applies none of it, and a reading that only differed in strength would say
 * the change had half happened.
 */
export const ROUTE_PROPOSED: Tone = 0x67d6b0;
export const ROUTE_MOVE_QUEUED: Tone = 0x4f9c85;

/** The two currents: the one the opening objective names, and every other. */
export const CURRENT_BRIGHT: Tone = 0xcdf0ff;
export const CURRENT_PLAIN: Tone = 0x4a7f9c;

/** The controlled Form: the brightest mark the surface carries. */
export const FORM_CONTROLLED: Tone = 0xf4f8ff;
export const FORM_CONTROLLED_RING: Tone = 0x9fd2ff;

/** Every Form the player does not steer. */
export const FORM_PLAIN: Tone = 0x76828f;

/** The standing View's boundary, and the members standing on its shell. */
export const BOUNDARY: Tone = 0xc3a4ff;
export const BOUNDARY_SHELL: Tone = 0xe0d0ff;

/** The boundary a queued change proposes, in the queue's own register. */
export const BOUNDARY_PROPOSED: Tone = 0x67d6b0;

/**
 * A candidate of the standing slate, and the one the queue proposes adopting.
 *
 * Both sit in the boundary's own family and below it in weight: a candidate is
 * a proposal about where a boundary could stand, so it reads as the same kind
 * of thing as the standing boundary and never as brightly. Nothing here ranks
 * them — the slate this goal emits is unranked, and a tone that said otherwise
 * would be a reading the record does not carry.
 */
export const CANDIDATE: Tone = 0x8f86b8;
export const CANDIDATE_FOCUSED: Tone = 0xe6dcff;

/** One hue per pressure, in the closed set's own order. */
export const PRESSURE_TONES: readonly Tone[] = [
  0x4f7fa8, // Drain
  0x9080c4, // Noise
  0xe0684f, // Fracture
  0x3fa8a0, // Flood
  0xd9a441, // Interference
  0x9aa6b2, // Drift
];

/**
 * The Still Mode surface: what a paused Field offers to be taken hold of.
 *
 * Three handles, three hues, and three shapes, so what a handle is answers
 * without a word for it: a Port keeps the Port's own hue, a Route's two ends
 * take the hue a Route carries flow in, and a Boundary vertex takes the
 * boundary's. The forecast strip is drawn in the boundary's hue too, because
 * what it reads is the standing View's own forward reading.
 */
export const HANDLE_PORT: Tone = 0x9fb2c8;
export const HANDLE_ROUTE: Tone = 0xa9c2d6;
export const HANDLE_BOUNDARY: Tone = 0xe0d0ff;
export const FORECAST: Tone = 0xc3a4ff;

/** The hue a cue that reports something wrong opens with. */
export const CUE_HARD: Tone = 0xff8a6b;

/** The hue every other cue opens with. */
export const CUE_SOFT: Tone = 0xbfe9ff;

/** Mixes two tones, `amount` 0 giving the first and 1 the second. */
export function mixTone(from: Tone, to: Tone, amount: number): Tone {
  const held = amount < 0 ? 0 : amount > 1 ? 1 : amount;
  const red = Math.round(((from >> 16) & 0xff) + (((to >> 16) & 0xff) - ((from >> 16) & 0xff)) * held);
  const green = Math.round(((from >> 8) & 0xff) + (((to >> 8) & 0xff) - ((from >> 8) & 0xff)) * held);
  const blue = Math.round((from & 0xff) + ((to & 0xff) - (from & 0xff)) * held);
  return (red << 16) | (green << 8) | blue;
}

/** The same tone as the `rgb()` text a 2D context takes. */
export function toneText(tone: Tone): string {
  return `rgb(${(tone >> 16) & 0xff},${(tone >> 8) & 0xff},${tone & 0xff})`;
}
