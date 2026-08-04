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
 * | Physical compartment | warm mineral grey |
 * | Observation View | pale violet |
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

/** A low mineral glint used for etched field structure and dormant paths. */
export const GRAPHITE_GLOW: Tone = 0x28323a;

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
export const CHARGE_SPARK: Tone = 0xffe6b8;

/** A Node that holds more than its own threshold. */
export const OVERLOAD: Tone = 0xe0684f;

/** A Port, before any Charge lifts it. */
export const PORT_BASE: Tone = 0x4e5865;

/** A Route, and the states its record can report. */
export const ROUTE_BASE: Tone = 0x39424e;
export const ROUTE_FLOW: Tone = 0xa9c2d6;
/** A dark keyline that keeps a Route legible where it crosses a Current. */
export const ROUTE_SEPARATION: Tone = 0x080b0e;
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
/** Local automation register: active throttle and a positively disabled gate. */
export const ROUTE_AUTOMATION: Tone = 0x52b7ad;
export const ROUTE_AUTOMATION_DISABLED: Tone = 0x6f7782;

/** Frozen local-policy execution: quiet, applied, waiting, and refused. */
export const POLICY_IDLE: Tone = 0x7d8995;
export const POLICY_ACTIVE: Tone = 0x62d8bd;
export const POLICY_WAIT: Tone = 0xd7ad63;
export const POLICY_BLOCKED: Tone = 0xe77a62;
export const POLICY_TARGET: Tone = 0xa8eee0;
export const POLICY_PREVIEW: Tone = 0xf2c86e;

/**
 * Design-authority assembly preview. Gold is the accepted candidate, while
 * the graphite origin remains visibly prior/noncausal. Material and Current
 * retain their physical families inside the shared bracket grammar.
 */
export const ASSEMBLY_DRAFT: Tone = 0xf0cf78;
export const ASSEMBLY_DRAFT_ORIGIN: Tone = 0x75808b;
export const ASSEMBLY_DRAFT_MATERIAL: Tone = 0x9edfc8;
export const ASSEMBLY_DRAFT_CURRENT: Tone = 0xaddcf0;

/** Paid intervention register: one causal operation, one stable local hue. */
export const INTERVENTION_CLAMP: Tone = 0x72b7d6;
export const INTERVENTION_SCRAMBLE: Tone = 0xd987b3;
export const INTERVENTION_DECOY: Tone = 0x5ed6b3;
export const INTERVENTION_DELAY: Tone = 0xe6b35c;
export const INTERVENTION_BREACH: Tone = 0xf07b62;

/** The two currents: the one the opening objective names, and every other. */
export const CURRENT_BRIGHT: Tone = 0xcdf0ff;
export const CURRENT_PLAIN: Tone = 0x4a7f9c;
export const CURRENT_GLASS: Tone = 0x7ed8f2;

/** The controlled Form: the brightest mark the surface carries. */
export const FORM_CONTROLLED: Tone = 0xf4f8ff;
export const FORM_CONTROLLED_RING: Tone = 0x9fd2ff;
export const FORM_FACET: Tone = 0xffffff;
export const FORM_FACET_SHADOW: Tone = 0x202936;
export const FORM_KEYLINE: Tone = 0x05070a;

/** Every Form the player does not steer. */
export const FORM_PLAIN: Tone = 0x76828f;

/** The material edge that determines physical membership and leakage. */
export const PHYSICAL_COMPARTMENT: Tone = 0xb8b3aa;
export const PHYSICAL_COMPARTMENT_SHELL: Tone = 0xe7e0d4;

/** A queued causal compartment edit, in the queue's mint register. */
export const PHYSICAL_COMPARTMENT_PROPOSED: Tone = 0x67d6b0;

/** The passive aperture that determines what the instruments measure. */
export const OBSERVATION_VIEW: Tone = 0xc3a4ff;

/** Embodied Renewal stock, distinguished by the work each material can perform. */
export const MATERIAL_JUNCTION: Tone = 0x72d6aa;
export const MATERIAL_BOUNDARY: Tone = 0xd7cbb8;
export const MATERIAL_CONDUCTOR: Tone = 0x79c9d5;

/** Compatibility names for consumers that have not adopted the split yet. */
export const BOUNDARY: Tone = OBSERVATION_VIEW;
export const BOUNDARY_SHELL: Tone = PHYSICAL_COMPARTMENT_SHELL;
export const BOUNDARY_PROPOSED: Tone = PHYSICAL_COMPARTMENT_PROPOSED;

/**
 * A candidate of the standing slate, and the one the active View matches.
 *
 * Both sit in the observation family and below the active View in weight: a
 * candidate is a possible measurement aperture, never a material edge.
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
 * take the hue a Route carries flow in, and a Compartment vertex takes the
 * boundary's. The forecast strip is drawn in the boundary's hue too, because
 * what it reads is the standing View's own forward reading.
 */
export const HANDLE_PORT: Tone = 0x9fb2c8;
export const HANDLE_ROUTE: Tone = 0xa9c2d6;
export const HANDLE_BOUNDARY: Tone = PHYSICAL_COMPARTMENT_SHELL;
export const FORECAST: Tone = OBSERVATION_VIEW;

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
