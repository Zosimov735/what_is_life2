/**
 * The renderer: the one thing that draws the Field, and the one interface the
 * shell reaches it through.
 *
 * `docs/field-framework/ARCHITECTURE.md` locks the interface so the Canvas2D
 * fallback is a drop-in for the WebGL one — `create_renderer(canvas, kind)`
 * answering with `resize(w, h)`, `render(prev, next, alpha)`,
 * `set_motion_profile(profile)`, and `dispose()`. Those names are the
 * document's own, spelled as the document spells them, so the contract and the
 * code read alike.
 *
 * What the renderer is, exactly: a pure function of the snapshots it has been
 * handed, the viewport, and the motion profile. It owns no authoritative state,
 * runs no timer that advances anything, reads no WASM module and no worker
 * internal, and never writes back. Its whole input is the decoded `FrameState`
 * pair the shell passes in; its whole output is pixels. Losing everything it
 * holds — a fallback engaging, a worker restarting — costs a few frames of
 * trail and nothing else.
 */

import { startCanvas2dEngine } from './canvas2d';
import { Ephemera } from './history';
import {
  createScene,
  projectScene,
  FULL_MOTION,
  type CandidateOutline,
  type MotionProfile,
  type PlaybackReading,
  type Scene,
  type Viewport,
} from './scene';
import type { Engine, RendererKind } from './engine';
import type { FrameState } from '../../../worker/src/frame-state';

export type {
  CandidateOutline,
  HandleMark,
  MotionProfile,
  PlaybackReading,
  Scene,
  Viewport,
} from './scene';
export { HANDLE_KIND } from './scene';
export type { RendererKind } from './engine';

/** The renderer handle, exactly as the architecture document locks it. */
export interface Renderer {
  /** Sizes the surface, in CSS pixels. */
  resize(width: number, height: number): void;
  /** Draws one frame between the two most recent snapshots. */
  render(previous: FrameState, next: FrameState, alpha: number): void;
  /** Takes the reduced-motion and trail settings. */
  set_motion_profile(profile: MotionProfile): void;
  /**
   * Takes the candidates of the standing slate, each as its inside and whether
   * the queue proposes it.
   *
   * A slate record crosses on demand rather than per frame — the frame carries
   * only what the renderer needs every frame — so the shell holds it and hands
   * it over here, the same way it hands over the motion profile. What the
   * outlines are drawn through is still the snapshot: the members name Nodes,
   * and where a Node stands is the frame's own.
   */
  set_candidates(candidates: readonly CandidateOutline[]): void;
  /**
   * Takes one perturbation result's compact playback series, or null to take
   * it away.
   *
   * The reading crosses exactly as the slate does — held by the shell, handed
   * over here, never in a frame — and it is drawn as motion in the Field: the
   * member marks swell and fall with the series, the base series beside it
   * when the kind carries one, and nothing resembling a chart. The setter
   * redraws, because a result arrives between frames and a still run runs no
   * step.
   */
  set_playback(reading: PlaybackReading | null): void;
  /** Releases the surface and everything drawn on it. */
  dispose(): void;
  /** Which engine is drawing, and none before one has started. */
  kind(): RendererKind | null;
  /** Settles with the engine that started, and rejects when neither would. */
  ready: Promise<RendererKind>;
  /** The scene the newest frame drew. A diagnostic, and what tests read. */
  scene(): Scene;
}

/**
 * The test-injection seam, and nothing else.
 *
 * The locked interface is two arguments — `create_renderer(canvas, kind)` — and
 * it stays two: this parameter is defaulted, every caller in the shell omits
 * it, and what it exists for is letting a test force the WebGL path to fail on
 * a machine where it would otherwise succeed. Nothing about the renderer's
 * behaviour changes when it is left out.
 */
export interface RendererOptions {
  startWebgl?: (canvas: HTMLCanvasElement, width: number, height: number) => Promise<Engine>;
  /** Whether this surface could carry a WebGL context at all. */
  probeWebgl?: () => boolean;
}

/**
 * Whether a WebGL context can be had, asked of a throwaway surface rather than
 * of the one the game draws on. A surface that has been asked for one context
 * kind cannot later give another, so probing the real surface would be the very
 * thing that stops the fallback from engaging on it.
 */
function webglAvailable(): boolean {
  try {
    const probe = document.createElement('canvas');
    return (probe.getContext('webgl2') ?? probe.getContext('webgl')) !== null;
  } catch {
    return false;
  }
}

/**
 * Starts a renderer on a surface.
 *
 * `kind` names the engine wanted. `"webgl"` starts the PixiJS engine and falls
 * back to Canvas2D when it will not start — the initialization-failure path —
 * and `"canvas2d"` goes straight there. The handle answers immediately either
 * way; `ready` settles when an engine is behind it.
 */
export function create_renderer(
  canvas: HTMLCanvasElement,
  kind: RendererKind,
  options: RendererOptions = {},
): Renderer {
  const probe = options.probeWebgl ?? webglAvailable;
  const scene = createScene();
  const ephemera = new Ephemera();

  let engine: Engine | null = null;
  let disposed = false;
  let profile: MotionProfile = FULL_MOTION;
  /** The standing slate's candidates, as the shell last handed them over. */
  let candidates: readonly CandidateOutline[] = [];
  /** The playback reading the shell last handed over, and none between. */
  let playback: PlaybackReading | null = null;
  /**
   * The playback position, in window steps, and the last instant it advanced.
   *
   * A still run runs no step, so the scene's simulated clock stands exactly
   * when playback is wanted; the playback clock is real time instead — one
   * window step per 1/30 s, the locked rate — accumulated across the draws
   * the shell asks for and cleared with the reading. The renderer still runs
   * no timer of its own: the clock advances only inside a draw.
   */
  let playbackClock = 0;
  let playbackAt: number | null = null;
  let viewport: Viewport = { width: 0, height: 0, dpr: 1 };
  // The newest pair, kept so a surface that resizes or an engine that arrives
  // mid-flight redraws at once. Three fields rather than one object, so a frame
  // at the render target allocates nothing here.
  let heldPrevious: FrameState | null = null;
  let heldNext: FrameState | null = null;
  let heldAlpha = 0;

  const density = (): number =>
    typeof globalThis.devicePixelRatio === 'number' && globalThis.devicePixelRatio > 0
      ? globalThis.devicePixelRatio
      : 1;

  function adopt(started: Engine): RendererKind {
    if (disposed) {
      started.dispose();
      return started.kind;
    }
    engine = started;
    if (viewport.width > 0 && viewport.height > 0) {
      started.resize(viewport.width, viewport.height);
    }
    // A surface that arrived mid-flight draws the newest frame at once rather
    // than waiting for the next one.
    redraw();
    return started.kind;
  }

  async function start(): Promise<RendererKind> {
    if (kind === 'webgl' && probe()) {
      try {
        const startWebgl = options.startWebgl ?? (await import('./webgl')).startWebglEngine;
        const width = viewport.width > 0 ? viewport.width : 1;
        const height = viewport.height > 0 ? viewport.height : 1;
        return adopt(await startWebgl(canvas, width, height));
      } catch (cause) {
        console.error('field_game render: the WebGL engine did not start, falling back', cause);
      }
    }
    return adopt(startCanvas2dEngine(canvas));
  }

  function draw(previous: FrameState, next: FrameState, alpha: number): void {
    if (!engine || viewport.width <= 0 || viewport.height <= 0) return;
    ephemera.observe(previous, next);
    // One window step per 1/30 s while a reading is held, looping; the modulo
    // is the scene's, because the scene knows whether reduced motion holds the
    // last step instead.
    if (playback && playback.series.length > 0) {
      const now = typeof performance !== 'undefined' ? performance.now() : 0;
      if (playbackAt !== null) {
        playbackClock += Math.max(0, now - playbackAt) * (30 / 1000);
      }
      playbackAt = now;
    } else {
      playbackAt = null;
    }
    projectScene(
      scene,
      previous,
      next,
      alpha,
      viewport,
      profile,
      ephemera,
      candidates,
      playback,
      playbackClock,
    );
    engine.draw(scene);
  }

  function redraw(): void {
    if (heldPrevious && heldNext) draw(heldPrevious, heldNext, heldAlpha);
  }

  const ready = start();

  return {
    resize(width, height) {
      const dpr = density();
      viewport = {
        width: Math.max(1, Math.round(width * dpr)),
        height: Math.max(1, Math.round(height * dpr)),
        dpr,
      };
      engine?.resize(viewport.width, viewport.height);
      redraw();
    },
    render(previous, next, alpha) {
      heldPrevious = previous;
      heldNext = next;
      heldAlpha = alpha;
      draw(previous, next, alpha);
    },
    set_motion_profile(next) {
      profile = next;
    },
    set_candidates(next) {
      candidates = next;
      // A slate arrives between frames — a still run runs no step, so no
      // snapshot need follow it — and the outlines have to reach the surface
      // all the same.
      redraw();
    },
    set_playback(next) {
      playback = next;
      // A fresh reading plays from its own start, and a reading taken away
      // takes its clock with it.
      playbackClock = 0;
      playbackAt = null;
      redraw();
    },
    dispose() {
      disposed = true;
      heldPrevious = null;
      heldNext = null;
      ephemera.clear();
      engine?.dispose();
      engine = null;
    },
    kind: () => engine?.kind ?? null,
    ready,
    scene: () => scene,
  };
}
