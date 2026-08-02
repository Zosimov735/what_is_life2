/**
 * What the two engines have in common.
 *
 * `docs/field-framework/ARCHITECTURE.md` locks the renderer's own interface so
 * the Canvas2D fallback is a drop-in for the WebGL one. This is the seam behind
 * that interface: both engines take the same scene, are sized the same way, and
 * are disposed the same way, and the renderer picks between them without
 * anything above it learning which one it got.
 *
 * Sizes here are device pixels. The renderer converts once, from the CSS pixels
 * its own `resize` takes, so no engine and no mark carries two units.
 */

import type { Scene } from './scene';

/** The two engines, named as the locked interface names them. */
export type RendererKind = 'webgl' | 'canvas2d';

export interface Engine {
  readonly kind: RendererKind;
  /** Sizes the surface, in device pixels. */
  resize(width: number, height: number): void;
  /** Draws one scene. */
  draw(scene: Scene): void;
  /** Releases everything the engine holds. */
  dispose(): void;
}

/**
 * The dash pattern one hull mark is drawn with: on-length, off-length, or an
 * empty array for the solid material compartment.
 *
 * Three registers are told apart without relying on colour: the committed
 * compartment is solid and heavy, its proposed edit is short-dashed, the View
 * is long-dashed and thin, and a candidate is sparse. Both engines read this,
 * so parity is a fact about the code rather than a thing to keep checking.
 *
 * A candidate's own sparseness is its tier: the nondominated set is drawn
 * closest to solid and each tier below it is drawn sparser, so the ranking
 * reads on the Field the way it reads in the tray — as a grouping, without a
 * figure and without a colour. A slate no comparison ran on carries tier 0 and
 * takes the same pattern as tier 1, because an uncompared candidate is not a
 * ranked one.
 */
/**
 * The playback ring's radius for one modulation factor, shared by both
 * engines so the swell is the same swell: the member's own mark radius,
 * standing a quarter out at the reading's floor and walking out by a further
 * 1.1 radii to its ceiling. Motion, not measurement — the factor is already
 * normalized against the reading's own range.
 */
export function playbackSwell(radius: number, factor: number): number {
  return radius * (1.25 + 1.1 * factor);
}

export function dashOf(mark: {
  role: 'compartment' | 'view' | 'candidate';
  proposed: boolean;
  candidate: number;
  tier: number;
}): number[] {
  if (mark.candidate > 0) return [2, 4 + 4 * Math.max(1, mark.tier)];
  if (mark.role === 'compartment') return mark.proposed ? [4, 6] : [];
  return [12, 8];
}
