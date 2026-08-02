/**
 * The surface the Field is drawn on, and the shell's whole part in drawing it.
 *
 * React owns the element, its size, and the accessibility setting that reaches
 * the renderer. Everything drawn on the element is the renderer's, and React
 * never learns what any of it is: no simulation value crosses into a component,
 * and no component ever writes one.
 *
 * The loop here reads the newest snapshot pair once per rendered frame and
 * hands it over. It advances nothing: the worker owns the accumulator, the core
 * owns the state, and this is display.
 */

import { useEffect, useRef } from 'react';
import { create_renderer, type CandidateOutline, type PlaybackReading } from '../render';
import type { Sound } from './sound';
import { openStillEdits, type SlateReading, type StillTool } from './still-edits';
import type { CandidateSlate, FramePair, PlanCommand } from './worker-client';

/** The reduced-motion setting the platform reports, until `InputConfig` crosses. */
const REDUCED_MOTION_QUERY = '(prefers-reduced-motion: reduce)';

/**
 * Where a local preview reaches the renderer, so the scene a frame built can be
 * read while the surface is running. Developer diagnostics, and only in a
 * development build.
 */
const RENDER_HANDLE = 'field_game_render';

/** Full trail intensity, the raw `Frac` the configuration defaults to. */
const FULL_TRAILS = 65536;

interface FieldSurfaceProps {
  /** Where the newest snapshot pair is read from, once per rendered frame. */
  frames: () => FramePair;
  /**
   * Where a drag on a paused Field sends the entry it proposes, and nothing
   * when the surface takes no edits at all.
   *
   * The drags live here rather than in the chrome because a handle is a place
   * on the Field, and the Field is drawn here: the source reads the scene the
   * renderer last drew, so what a player takes hold of is the handle they can
   * see, at the place it was drawn.
   */
  queuePlan?: (plan: PlanCommand) => void;
  /**
   * Takes the newest queued entry back, and nothing when the surface takes no
   * edits at all. Walking the candidates replaces the focus it proposed, which
   * is one undo and one queue.
   */
  undoPlan?: () => void;
  /** The explicit Still Mode tool selected in the accessible chrome. */
  tool?: StillTool;
  /** Moves the passive View immediately, outside the causal queue. */
  setFocus?: (slateOrdinal: number, position: number) => void;
  /**
   * The evaluation record the run stands under, and none before the first
   * slate is assembled.
   *
   * The outlines are drawn from it: a slate record crosses on demand rather
   * than per frame, so the shell holds it and the renderer is handed the
   * insides. Where each of those Nodes stands is still the snapshot's.
   */
  slate?: CandidateSlate | null;
  /** The queue as the worker last reported it, for the focus it proposes. */
  focused?: number;
  /**
   * The playback reading the shell offers the renderer, and null while none
   * stands.
   *
   * It crosses exactly as the slate does — held by the shell, handed over
   * between frames — and the offer is gated where the record is held: only
   * from the opt-in inspection surface, only while the run is still or in a
   * ramp, and null in ordinary play, so a moving Field never carries one.
   */
  playback?: PlaybackReading | null;
  /**
   * What sounds the cues, and nothing when the shell opened no sound. It reads
   * the same snapshot the renderer draws, from the same loop, so what is heard
   * and what is seen are the same frame.
   */
  sound?: Sound | null;
}

export function FieldSurface({
  frames,
  sound,
  queuePlan,
  undoPlan,
  tool = 'view',
  setFocus,
  slate = null,
  focused = 0,
  playback = null,
}: FieldSurfaceProps) {
  const surface = useRef<HTMLCanvasElement>(null);
  // Held in a reference rather than read from the closure, so the sound
  // arriving after the first paint does not tear the surface down and build a
  // second one: the renderer's own effect depends on the surface and the frame
  // source, and on nothing else.
  const heard = useRef<Sound | null>(null);
  // The same shape, and for the same reason: a queue sink arriving after the
  // first paint must not tear the surface down and build a second one.
  const queued = useRef<((plan: PlanCommand) => void) | null>(null);
  const undone = useRef<(() => void) | null>(null);
  const activeTool = useRef<StillTool>('view');
  const focusedView = useRef<((slateOrdinal: number, position: number) => void) | null>(null);
  const carriedTool = useRef<((tool: StillTool) => void) | null>(null);
  // The slate and the candidate the authoritative View matches, held in
  // references for the same reason: they arrive between frames, and a surface
  // rebuilt on every arrival would lose the trails and the engine with them.
  const standing = useRef<CandidateSlate | null>(null);
  const focus = useRef(0);
  const carried = useRef<((candidates: readonly CandidateOutline[]) => void) | null>(null);
  // The playback reading, held the same way and for the same reason: it
  // arrives between frames, and a surface rebuilt on every arrival would lose
  // the trails and the engine with them.
  const played = useRef<PlaybackReading | null>(null);
  const carriedPlayback = useRef<((reading: PlaybackReading | null) => void) | null>(null);

  useEffect(() => {
    heard.current = sound ?? null;
  }, [sound]);

  useEffect(() => {
    queued.current = queuePlan ?? null;
  }, [queuePlan]);

  useEffect(() => {
    undone.current = undoPlan ?? null;
  }, [undoPlan]);

  useEffect(() => {
    activeTool.current = tool;
    carriedTool.current?.(tool);
  }, [tool]);

  useEffect(() => {
    focusedView.current = setFocus ?? null;
  }, [setFocus]);

  // The playback reading reaches the renderer on its own setter, which
  // redraws: a result arrives between frames, and a still run runs no step to
  // carry it.
  useEffect(() => {
    played.current = playback;
    carriedPlayback.current?.(playback);
  }, [playback]);

  // The outlines the renderer draws, rebuilt whenever the record or the focus
  // moves. A slate arrives between frames — a still run runs no step — so the
  // renderer redraws on the setter rather than waiting for a snapshot.
  useEffect(() => {
    standing.current = slate;
    focus.current = focused;
    carried.current?.(
      (slate?.candidates ?? []).map((candidate) => ({
        position: candidate.position,
        members: candidate.view.inside,
        focused: candidate.position === focused,
        tier: candidate.tier,
      })),
    );
  }, [slate, focused]);

  useEffect(() => {
    const canvas = surface.current;
    if (!canvas) return;

    const renderer = create_renderer(canvas, 'webgl');
    renderer.ready.catch((cause: unknown) => {
      console.error('field_game shell: no renderer would start on the surface', cause);
    });
    if (import.meta.env.DEV) {
      (globalThis as Record<string, unknown>)[RENDER_HANDLE] = renderer;
    }

    const motion =
      typeof window.matchMedia === 'function' ? window.matchMedia(REDUCED_MOTION_QUERY) : null;
    const carryMotion = (): void => {
      renderer.set_motion_profile({
        reducedMotion: motion?.matches ?? false,
        trailIntensity: FULL_TRAILS,
      });
    };
    carryMotion();
    motion?.addEventListener('change', carryMotion);

    const size = (): void => {
      renderer.resize(
        canvas.clientWidth || window.innerWidth,
        canvas.clientHeight || window.innerHeight,
      );
    };
    size();
    const watcher =
      typeof ResizeObserver === 'function' ? new ResizeObserver(() => size()) : null;
    watcher?.observe(canvas);
    if (!watcher) window.addEventListener('resize', size);

    // The drags a paused Field takes. The source reads the scene the renderer
    // last drew and the mode the snapshot reports, so it takes hold of exactly
    // what is on the surface and does nothing at all while the Field moves.
    const edits = openStillEdits({
      surface: canvas,
      scene: () => renderer.scene(),
      paused: () => frames().next?.header.mode === 'still',
      tool: () => activeTool.current,
      queue: (plan) => queued.current?.(plan),
      focus: (slateOrdinal, position) => focusedView.current?.(slateOrdinal, position),
      slate: (): SlateReading | null => {
        const held = standing.current;
        if (!held) return null;
        return {
          ordinal: held.ordinal,
          count: held.candidates.length,
          deficient: held.deficient,
        };
      },
      focused: () => focus.current,
      undo: () => undone.current?.(),
    });

    // The renderer is reached through the reference the effect above writes,
    // so a slate that arrived before this surface was built still reaches it.
    carriedPlayback.current = (reading) => renderer.set_playback(reading);
    carriedPlayback.current(played.current);
    carriedTool.current = (next) => renderer.set_still_tool(next);
    carriedTool.current(activeTool.current);
    carried.current = (candidates) => renderer.set_candidates(candidates);
    carried.current(
      (standing.current?.candidates ?? []).map((candidate) => ({
        position: candidate.position,
        members: candidate.view.inside,
        focused: candidate.position === focus.current,
        tier: candidate.tier,
      })),
    );

    let handle = requestAnimationFrame(function tick() {
      handle = requestAnimationFrame(tick);
      const pair = frames();
      if (!pair.previous || !pair.next) return;
      renderer.render(pair.previous, pair.next, pair.alpha);
      // The reduced-motion setting reaches the renderer and stops there: it is
      // a motion setting, and a player who asked for less movement has not
      // asked for less sound. The two channels are separate in `InputConfig`
      // and separate here.
      heard.current?.observe(pair.next);
    });

    return () => {
      cancelAnimationFrame(handle);
      carried.current = null;
      carriedPlayback.current = null;
      carriedTool.current = null;
      edits.close();
      watcher?.disconnect();
      if (!watcher) window.removeEventListener('resize', size);
      motion?.removeEventListener('change', carryMotion);
      if (import.meta.env.DEV) {
        delete (globalThis as Record<string, unknown>)[RENDER_HANDLE];
      }
      renderer.dispose();
    };
  }, [frames]);

  return <canvas className="field" ref={surface} />;
}
