/**
 * The surface the Field is drawn on, and the shell's whole part in drawing it.
 *
 * React owns the element, its size, and the accessibility setting that reaches
 * the renderer. Everything drawn on the element is the renderer's. On pointer
 * inspection React receives one explicit authoritative reading for the hit
 * target; that display value never writes back into simulation.
 *
 * The loop here reads the newest snapshot pair once per rendered frame and
 * hands it over. It advances nothing: the worker owns the accumulator, the core
 * owns the state, and this is display.
 */

import { useEffect, useRef, useState } from 'react';
import {
  create_renderer,
  type CandidateOutline,
  type EngineeringTransitionCompanion,
  type PlaybackReading,
  type Scene,
} from '../render';
import { copy } from './copy';
import type { Sound } from './sound';
import { openStillEdits, type SlateReading, type StillTool } from './still-edits';
import type { CandidateSlate, FramePair, PlanCommand } from './worker-client';
import type { RegimeId } from './regimes';
import type {
  EngineeringAssemblyPreview,
  LocalAction,
  LocalCondition,
  PolicyPreview,
} from '../../../worker/src/protocol';
import './field-shell.css';

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

const units = (raw: number): string => (raw / 65_536).toLocaleString('en-US', {
  maximumFractionDigits: 2,
});

interface FieldIntervention {
  kind: 'clamp' | 'scramble' | 'decoy' | 'decoy_receiver' | 'delay' | 'breach';
  remaining: number;
  capture_fraction?: number;
  current?: number;
  original_capacity?: number;
  original_coefficient?: number;
  probability?: number;
  receiver?: number;
}

const percent = (raw: number): string => `${(raw * 100 / 65_536).toFixed(1)}%`;

const catalog = (prefix: string, value: string): string => `${prefix}.${value}`;

function interventionSummary(reading: FieldIntervention): string {
  const duration = `${reading.remaining} ${copy('unit.steps')}`;
  switch (reading.kind) {
    case 'clamp':
      return `${copy('intervention.clamp')} · ${units(reading.original_capacity ?? 0)} ${copy('unit.cu_per_step')} ${copy('field.inspect.standing')} · ${duration}`;
    case 'scramble':
      return `${copy('intervention.scramble')} · ${percent(reading.probability ?? 0)} · ${duration}`;
    case 'decoy':
      return `${copy('intervention.decoy')} → ${reading.receiver ?? '-'} · ${percent(reading.capture_fraction ?? 0)} · ${duration}`;
    case 'decoy_receiver':
      return `${copy('intervention.decoy')} ← ${reading.current ?? '-'} · ${percent(reading.capture_fraction ?? 0)} · ${duration}`;
    case 'delay':
      return `${copy('intervention.delay')} · ${duration}`;
    case 'breach':
      return `${copy('intervention.breach')} · ${percent(reading.original_coefficient ?? 0)} ${copy('field.inspect.standing')} · ${duration}`;
  }
}

const interventionRow = (readings: FieldIntervention[]) => readings.length > 0 ? (
  <div>
    <dt>{copy('field.inspect.interventions')}</dt>
    <dd>{readings.map(interventionSummary).join(' / ')}</dd>
  </div>
) : null;

export type PolicyOutcome =
  | 'idle'
  | 'held'
  | 'applied'
  | 'no_target'
  | 'target_unavailable'
  | 'wrong_layer'
  | 'out_of_range'
  | 'no_effect'
  | 'cooldown'
  | 'capacity_reached'
  | 'unavailable';

interface PolicyInspection {
  automated: boolean;
  policy_action: LocalAction | null;
  policy_cooldown: number;
  policy_outcome: PolicyOutcome;
  policy_rule: number;
  policy_target_kind: 'none' | 'node' | 'route' | 'current' | 'signal';
  policy_target: number | null;
  policy_timer: number;
  policy_capabilities: {
    actions: LocalAction['kind'][];
    attached_routes: Array<{ capacity: number; outgoing: boolean; route: number }>;
    conditions: LocalCondition['kind'][];
    coupling_radius_max: number;
    mobile: boolean;
    route_weight_max: number;
    sensor_radius_max: number;
    signal_strength_max: number;
  };
}

export type FieldInspection =
  | ({
      target: 'form'; id: number; node: number; kind: string; layer: number;
      q: number; capacity: number; vx: number; vy: number; controlled: boolean;
      ability: string; ability_available: boolean; ability_value: number;
      ability_limit: number; ability_count: number; ability_due: number;
      medium_coupling: number; medium_drag: number; medium_vx: number; medium_vy: number;
      medium_collision_radius: number; medium_collision_response: number;
    } & PolicyInspection)
  | ({
      target: 'node'; id: number; kind: string; layer: number; open: boolean;
      q: number; capacity: number; upkeep_rate: number; inflow_mean: number;
      upkeep_mix: number[]; outflow_mean: number; current_leakage: number;
      window: number; active_interventions: FieldIntervention[];
    } & PolicyInspection)
  | {
      target: 'route'; id: number; tail: number; head: number; capacity: number;
      flow: number; mean_flow: number; formed_step: number; window: number;
      requested_flow: number; accepted_flow: number;
      outcome: 'disabled' | 'closed' | 'standing' | 'capacity_throttled' |
        'source_starved' | 'destination_headroom' | 'flowing';
      enabled: boolean; capacity_limit: number; allocation_weight: number;
      controller: number; active_interventions: FieldIntervention[];
    }
  | {
      target: 'current'; id: number; active: boolean; layer: number; period: number;
      phase: number; recipients: number[]; strength: number; width: number;
      cycle_mean: number; duty: number; emitting: boolean; instantaneous_ceiling: number;
      ceiling_low: number; ceiling_high: number; variability: number;
      on_steps: number; active_interventions: FieldIntervention[];
    }
  | {
      target: 'compartment'; members: number[]; leak_fraction: number; current_leakage: number;
      active_interventions: FieldIntervention[];
    }
  | {
      target: 'view'; members: number[]; resolution: number; surround: string; window: number;
    }
  | {
      target: 'material'; id: number; kind: string; layer: number; amount: number; claimed: boolean;
    }
  | {
      target: 'cache'; id: number; form: number; layer: number; q: number; radius: number; release_in: number;
    };

type FormInspection = Extract<FieldInspection, { target: 'form' }>;

function abilitySummary(reading: FormInspection): string {
  switch (reading.ability) {
    case 'responsive_steering':
      return `${(reading.ability_value / 65_536).toFixed(1)}${copy('unit.multiplier')} ${copy('field.inspect.steering_response')}`;
    case 'local_retention':
      return copy('field.inspect.local_retention_active');
    case 'extended_commissioning':
      return `${units(reading.ability_value)} ${copy('unit.cu_per_step')} · ${units(reading.ability_limit)} ${copy('unit.field_units')}`;
    case 'reserve_discharge':
      return `${units(reading.ability_value)} / ${units(reading.ability_limit)} ${copy('unit.cu')} ${copy('field.inspect.retained')}`;
    case 'local_sensor':
      return `${units(reading.ability_value)} ${copy('unit.cu')} ${copy('field.inspect.packet')} · ${units(reading.ability_limit)} ${copy('unit.field_units')}`;
    case 'junction_deployment':
      return `${reading.ability_count} ${copy('field.inspect.blanks')} · ${units(reading.ability_value)} ${copy('unit.cu')} · ${units(reading.ability_limit)} ${copy('unit.cu')} ${copy('field.inspect.limit')}`;
    case 'conserving_cache':
      return `${reading.ability_count} ${copy('field.inspect.standing')} · ${units(reading.ability_value)} ${copy('unit.cu')} · ${units(reading.ability_limit)} ${copy('unit.field_units')}`;
    case 'control_handoff':
      return `${reading.ability_count} ${copy('field.inspect.members_count')} · ${units(reading.ability_limit)} ${copy('unit.field_units')}`;
    default:
      return copy('field.inspect.unavailable');
  }
}

function actionLabel(action: LocalAction | null): string {
  return action ? copy(`automation.action.${action.kind}`) : copy('automation.status.no_action');
}

interface HoverInspection {
  x: number;
  y: number;
  align: 'left' | 'right';
  reading: FieldInspection;
}

interface FieldSurfaceProps {
  /** Atlas lawset whose physical parameters established this Field. */
  regime?: RegimeId;
  /** Where the newest snapshot pair is read from, once per rendered frame. */
  frames: () => FramePair;
  /** Reads exact local state only after the pointer settles on a causal object. */
  inspectField?: (target: FieldInspection['target'], id: number) => Promise<FieldInspection | null>;
  /** Pins the exact object currently under the pointer into the workbench. */
  onSelect?: (reading: FieldInspection) => void;
  /** Exposes the rendered causal projection for display-only blueprint thumbnails. */
  onSurface?: (surface: HTMLCanvasElement | null) => void;
  /** Pure Rust Design projection; never written into the simulation frame. */
  policyPreview?: PolicyPreview | null;
  /** Rust-accepted opening assembly candidate; display-only and noncausal. */
  assemblyPreview?: EngineeringAssemblyPreview | null;
  /** Rust-authored transition preview or accepted receipt projected on the real machine. */
  engineeringTransition?: EngineeringTransitionCompanion | null;
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
  regime = 'open_field',
  frames,
  sound,
  queuePlan,
  undoPlan,
  tool = 'view',
  setFocus,
  slate = null,
  focused = 0,
  playback = null,
  inspectField,
  onSelect,
  onSurface,
  policyPreview = null,
  assemblyPreview = null,
  engineeringTransition = null,
}: FieldSurfaceProps) {
  const surface = useRef<HTMLCanvasElement>(null);
  const inspector = useRef(inspectField);
  const inspectionKey = useRef('');
  const inspectionRequest = useRef(0);
  const [inspection, setInspection] = useState<HoverInspection | null>(null);
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
  const carriedPolicyPreview = useRef<((preview: PolicyPreview | null) => void) | null>(null);
  const carriedAssemblyPreview = useRef<((preview: EngineeringAssemblyPreview | null) => void) | null>(null);
  const carriedEngineeringTransition = useRef<((companion: EngineeringTransitionCompanion | null) => void) | null>(null);

  useEffect(() => {
    inspector.current = inspectField;
  }, [inspectField]);

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

  useEffect(() => {
    carriedPolicyPreview.current?.(policyPreview);
  }, [policyPreview]);

  useEffect(() => {
    carriedAssemblyPreview.current?.(assemblyPreview);
  }, [assemblyPreview]);

  useEffect(() => {
    carriedEngineeringTransition.current?.(engineeringTransition);
    sound?.transition(engineeringTransition ? {
      commitAllowed: engineeringTransition.preview.definition.commit_allowed,
      operation: engineeringTransition.preview.definition.operation,
      previewId: engineeringTransition.preview.preview_id,
      status: engineeringTransition.status,
    } : null);
  }, [engineeringTransition, sound]);

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

    const distanceToRoute = (scene: Scene, place: number, x: number, y: number): number => {
      const route = scene.routes.items[place];
      const dx = route.x2 - route.x1;
      const dy = route.y2 - route.y1;
      const length = dx * dx + dy * dy;
      const along = length === 0 ? 0 : Math.min(1, Math.max(0, ((x - route.x1) * dx + (y - route.y1) * dy) / length));
      return Math.hypot(x - (route.x1 + dx * along), y - (route.y1 + dy * along));
    };

    const onPointerMove = (event: PointerEvent): void => {
      const scene = renderer.scene();
      const bounds = canvas.getBoundingClientRect();
      const dpr = scene.dpr || 1;
      const x = (event.clientX - bounds.left) * dpr;
      const y = (event.clientY - bounds.top) * dpr;
      let target: FieldInspection['target'] | null = null;
      let id = 0;
      let nearest = Infinity;
      for (let place = 0; place < scene.forms.count; place += 1) {
        const form = scene.forms.items[place];
        const distance = Math.hypot(x - form.x, y - form.y);
        if (form.alpha > 0.05 && distance <= Math.max(16 * dpr, form.radius * 1.15) && distance < nearest) {
          target = 'form';
          id = form.form;
          nearest = distance;
        }
      }
      for (let place = 0; !target && place < scene.ports.count; place += 1) {
        const port = scene.ports.items[place];
        const distance = Math.hypot(x - port.x, y - port.y);
        if (port.alpha > 0.05 && distance <= Math.max(12 * dpr, port.radius * 1.6) && distance < nearest) {
          target = 'node';
          id = port.node;
          nearest = distance;
        }
      }
      if (!target) {
        for (let place = 0; place < scene.routes.count; place += 1) {
          const route = scene.routes.items[place];
          if (route.preview || route.alpha <= 0.05) continue;
          const distance = distanceToRoute(scene, place, x, y);
          if (distance <= Math.max(7 * dpr, route.width * 2) && distance < nearest) {
            target = 'route';
            id = route.route;
            nearest = distance;
          }
        }
      }
      if (!target) {
        for (let place = 0; place < scene.currents.count; place += 1) {
          const current = scene.currents.items[place];
          if (current.alpha <= 0.05) continue;
          if (current.points.length === 2) {
            const distance = Math.hypot(x - current.points[0], y - current.points[1]);
            if (distance <= Math.max(8 * dpr, current.width * 0.65) && distance < nearest) {
              target = 'current';
              id = current.id;
              nearest = distance;
            }
          }
          for (let point = 2; point < current.points.length; point += 2) {
            const x1 = current.points[point - 2];
            const y1 = current.points[point - 1];
            const x2 = current.points[point];
            const y2 = current.points[point + 1];
            const dx = x2 - x1;
            const dy = y2 - y1;
            const length = dx * dx + dy * dy;
            const along = length === 0 ? 0 : Math.min(1, Math.max(0, ((x - x1) * dx + (y - y1) * dy) / length));
            const distance = Math.hypot(x - (x1 + dx * along), y - (y1 + dy * along));
            if (distance <= Math.max(8 * dpr, current.width * 0.65) && distance < nearest) {
              target = 'current';
              id = current.id;
              nearest = distance;
            }
          }
        }
      }
      if (!target) {
        for (let place = 0; place < scene.particles.count; place += 1) {
          const particle = scene.particles.items[place];
          if ((particle.subject !== 1 && particle.subject !== 2) || particle.alpha <= 0.05) continue;
          const distance = Math.hypot(x - particle.x, y - particle.y);
          if (distance <= Math.max(10 * dpr, particle.radius * 1.8) && distance < nearest) {
            target = particle.subject === 2 ? 'cache' : 'material';
            id = particle.id;
            nearest = distance;
          }
        }
      }
      if (!target) {
        for (let place = scene.boundaries.count - 1; place >= 0; place -= 1) {
          const boundary = scene.boundaries.items[place];
          if (boundary.alpha <= 0.05 || boundary.points.length < 2) continue;
          if (boundary.points.length === 2) {
            const distance = Math.hypot(x - boundary.points[0], y - boundary.points[1]);
            if (distance <= Math.max(7 * dpr, boundary.width * 1.5) && distance < nearest) {
              target = boundary.role === 'view' ? 'view' : 'compartment';
              id = 0;
              nearest = distance;
            }
            continue;
          }
          for (let point = 0; point < boundary.points.length; point += 2) {
            const next = (point + 2) % boundary.points.length;
            const x1 = boundary.points[point];
            const y1 = boundary.points[point + 1];
            const x2 = boundary.points[next];
            const y2 = boundary.points[next + 1];
            const dx = x2 - x1;
            const dy = y2 - y1;
            const length = dx * dx + dy * dy;
            const along = length === 0 ? 0 : Math.min(1, Math.max(0, ((x - x1) * dx + (y - y1) * dy) / length));
            const distance = Math.hypot(x - (x1 + dx * along), y - (y1 + dy * along));
            if (distance <= Math.max(7 * dpr, boundary.width * 1.5) && distance < nearest) {
              target = boundary.role === 'view' ? 'view' : 'compartment';
              id = 0;
              nearest = distance;
            }
          }
        }
      }
      const key = target ? `${target}:${id}` : '';
      if (key === inspectionKey.current) return;
      inspectionKey.current = key;
      const request = ++inspectionRequest.current;
      if (!target || !inspector.current) {
        setInspection(null);
        return;
      }
      const cssX = event.clientX - bounds.left;
      const cssY = event.clientY - bounds.top;
      void inspector.current(target, id).then((reading) => {
        if (request !== inspectionRequest.current || !reading) return;
        setInspection({
          x: cssX,
          y: Math.max(100, Math.min(bounds.height - 100, cssY)),
          align: cssX > bounds.width * 0.62 ? 'right' : 'left',
          reading,
        });
      });
    };

    const clearInspection = (): void => {
      inspectionKey.current = '';
      inspectionRequest.current += 1;
      setInspection(null);
    };

    canvas.addEventListener('pointermove', onPointerMove);
    canvas.addEventListener('pointerleave', clearInspection);

    // The renderer is reached through the reference the effect above writes,
    // so a slate that arrived before this surface was built still reaches it.
    carriedPlayback.current = (reading) => renderer.set_playback(reading);
    carriedPolicyPreview.current = (preview) => renderer.set_policy_preview(preview);
    carriedPolicyPreview.current(policyPreview);
    carriedAssemblyPreview.current = (preview) => renderer.set_assembly_preview(preview);
    carriedAssemblyPreview.current(assemblyPreview);
    carriedEngineeringTransition.current = (companion) => renderer.set_engineering_transition(companion);
    carriedEngineeringTransition.current(engineeringTransition);
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
      carriedPolicyPreview.current = null;
      carriedAssemblyPreview.current = null;
      carriedEngineeringTransition.current = null;
      edits.close();
      canvas.removeEventListener('pointermove', onPointerMove);
      canvas.removeEventListener('pointerleave', clearInspection);
      watcher?.disconnect();
      if (!watcher) window.removeEventListener('resize', size);
      motion?.removeEventListener('change', carryMotion);
      if (import.meta.env.DEV) {
        delete (globalThis as Record<string, unknown>)[RENDER_HANDLE];
      }
      renderer.dispose();
    };
  }, [frames]);

  return (
    <div
      className="field-stage"
      data-regime={regime}
      data-inspecting={inspection ? inspection.reading.target : undefined}
      tabIndex={0}
      onPointerDown={(event) => {
        event.currentTarget.focus({ preventScroll: true });
        if (inspection) onSelect?.(inspection.reading);
      }}
    >
      <img
        className="field-texture"
        src="/assets/number-2-field-texture.png"
        alt=""
        aria-hidden="true"
      />
      <canvas
        className="field"
        ref={(node) => {
          surface.current = node;
          onSurface?.(node);
        }}
      />
      {engineeringTransition ? (
        <aside
          className="field-engineering-transition"
          data-status={engineeringTransition.status}
          data-operation={engineeringTransition.preview.definition.operation}
          aria-live="polite"
        >
          <span>{copy(`automation.transition.title.${engineeringTransition.preview.definition.operation}`)}</span>
          <strong>{copy(`automation.transition.field.${engineeringTransition.status}`)}</strong>
        </aside>
      ) : null}
      {inspection ? (
        <aside
          className="field-inspection"
          data-align={inspection.align}
          data-target={inspection.reading.target}
          style={{ left: inspection.x, top: inspection.y }}
          aria-live="polite"
        >
          <header>
            <span>{copy(catalog('field.inspect', inspection.reading.target))}</span>
            {'id' in inspection.reading ? <strong>{inspection.reading.id}</strong> : null}
          </header>
          {inspection.reading.target === 'form' ? (
            <dl>
              <div><dt>{copy('field.inspect.kind')}</dt><dd>{inspection.reading.kind}</dd></div>
              <div><dt>{copy('field.inspect.charge')}</dt><dd>{units(inspection.reading.q)} / {units(inspection.reading.capacity)} {copy('unit.cu')}</dd></div>
              <div><dt>{copy('field.inspect.velocity')}</dt><dd>{units(inspection.reading.vx)}, {units(inspection.reading.vy)} {copy('unit.field_units_per_step')}</dd></div>
              <div><dt>{copy('field.inspect.medium')}</dt><dd>{units(inspection.reading.medium_vx)}, {units(inspection.reading.medium_vy)} {copy('unit.field_units_per_step')} · {percent((inspection.reading.medium_drag * inspection.reading.medium_coupling) / 65_536)}</dd></div>
              {inspection.reading.medium_collision_radius > 0 ? <div><dt>{copy('field.inspect.collision')}</dt><dd>{units(inspection.reading.medium_collision_radius)} {copy('unit.field_units')} · {percent(inspection.reading.medium_collision_response)} {copy('field.inspect.rebound')}</dd></div> : null}
              <div><dt>{copy('field.inspect.control')}</dt><dd>{copy(inspection.reading.automated ? 'automation.status.automated' : 'automation.status.unprogrammed')}</dd></div>
              <div><dt>{copy('automation.active.label')}</dt><dd>{actionLabel(inspection.reading.policy_action)}</dd></div>
              <div><dt>{copy('automation.outcome.label')}</dt><dd>{copy(`automation.outcome.${inspection.reading.policy_outcome}`)}</dd></div>
              {inspection.reading.policy_rule >= 0 ? <div><dt>{copy('automation.rule.label')}</dt><dd>{inspection.reading.policy_rule + 1}</dd></div> : null}
              {inspection.reading.policy_target_kind !== 'none' ? <div><dt>{copy('automation.active.target')}</dt><dd>{inspection.reading.policy_target_kind} {inspection.reading.policy_target}</dd></div> : null}
              <div><dt>{copy('automation.label.runtime_timer')}</dt><dd>{inspection.reading.policy_timer} {copy('unit.steps')}</dd></div>
              <div><dt>{copy('automation.label.cooldown')}</dt><dd>{inspection.reading.policy_cooldown} {copy('unit.steps')}</dd></div>
              <div><dt>{copy('field.inspect.ability')}</dt><dd>{inspection.reading.ability.replaceAll('_', ' ')}</dd></div>
              <div><dt>{copy('field.inspect.ability_state')}</dt><dd>{copy(inspection.reading.ability_available ? 'field.inspect.ready' : 'field.inspect.unavailable')}</dd></div>
              <div><dt>{copy('field.inspect.ability_value')}</dt><dd>{abilitySummary(inspection.reading)}</dd></div>
              {inspection.reading.ability_due > 0 ? <div><dt>{copy('field.inspect.release_in')}</dt><dd>{inspection.reading.ability_due} {copy('unit.steps')}</dd></div> : null}
            </dl>
          ) : inspection.reading.target === 'node' ? (
            <dl>
              <div><dt>{copy('field.inspect.kind')}</dt><dd>{inspection.reading.kind}</dd></div>
              <div><dt>{copy('field.inspect.charge')}</dt><dd>{units(inspection.reading.q)} / {units(inspection.reading.capacity)} {copy('unit.cu')}</dd></div>
              <div><dt>{copy('field.inspect.inflow')}</dt><dd>{units(inspection.reading.inflow_mean)} {copy('unit.cu_per_step')}</dd></div>
              <div><dt>{copy('field.inspect.outflow')}</dt><dd>{units(inspection.reading.outflow_mean)} {copy('unit.cu_per_step')}</dd></div>
              <div><dt>{copy('field.inspect.upkeep')}</dt><dd>{units(inspection.reading.upkeep_rate)} {copy('unit.cu_per_step')}</dd></div>
              <div><dt>{copy('field.inspect.upkeep_mix')}</dt><dd>{inspection.reading.upkeep_mix.map((value, place) => value > 0 ? `${copy(catalog('upkeep', ['boundary', 'repair', 'replacement', 'movement', 'reserve'][place]))} ${units(value)}` : '').filter(Boolean).join(' · ') || '-'}</dd></div>
              <div><dt>{copy('field.inspect.leakage')}</dt><dd>{units(inspection.reading.current_leakage)} {copy('unit.cu_per_step')}</dd></div>
              <div><dt>{copy('field.inspect.control')}</dt><dd>{copy(inspection.reading.automated ? 'automation.status.automated' : 'automation.status.unprogrammed')}</dd></div>
              <div><dt>{copy('automation.active.label')}</dt><dd>{actionLabel(inspection.reading.policy_action)}</dd></div>
              <div><dt>{copy('automation.outcome.label')}</dt><dd>{copy(`automation.outcome.${inspection.reading.policy_outcome}`)}</dd></div>
              {inspection.reading.policy_rule >= 0 ? <div><dt>{copy('automation.rule.label')}</dt><dd>{inspection.reading.policy_rule + 1}</dd></div> : null}
              {inspection.reading.policy_target_kind !== 'none' ? <div><dt>{copy('automation.active.target')}</dt><dd>{inspection.reading.policy_target_kind} {inspection.reading.policy_target}</dd></div> : null}
              <div><dt>{copy('automation.label.runtime_timer')}</dt><dd>{inspection.reading.policy_timer} {copy('unit.steps')}</dd></div>
              <div><dt>{copy('automation.label.cooldown')}</dt><dd>{inspection.reading.policy_cooldown} {copy('unit.steps')}</dd></div>
              {interventionRow(inspection.reading.active_interventions)}
            </dl>
          ) : inspection.reading.target === 'route' ? (
            <dl>
              <div><dt>{copy('field.inspect.direction')}</dt><dd>{inspection.reading.tail} → {inspection.reading.head}</dd></div>
              <div><dt>{copy('field.inspect.capacity')}</dt><dd>{units(inspection.reading.capacity)} {copy('unit.cu_per_step')}</dd></div>
              <div><dt>{copy('automation.route.state')}</dt><dd>{copy(inspection.reading.enabled ? 'automation.route.enabled' : 'automation.route.disabled')}</dd></div>
              <div><dt>{copy('automation.label.route_limit')}</dt><dd>{units(inspection.reading.capacity_limit)} {copy('unit.cu_per_step')}</dd></div>
              <div><dt>{copy('automation.label.allocation_weight')}</dt><dd>{inspection.reading.allocation_weight}</dd></div>
              <div><dt>{copy('automation.route.controller')}</dt><dd>{inspection.reading.controller}</dd></div>
              <div><dt>{copy('field.inspect.flow')}</dt><dd>{units(inspection.reading.flow)} {copy('unit.cu_per_step')}</dd></div>
              <div><dt>{copy('field.inspect.rolling_mean')}</dt><dd>{units(inspection.reading.mean_flow)} {copy('unit.cu_per_step')}</dd></div>
              <div><dt>{copy('field.inspect.window')}</dt><dd>{inspection.reading.window} {copy('unit.steps')}</dd></div>
              {interventionRow(inspection.reading.active_interventions)}
            </dl>
          ) : inspection.reading.target === 'current' ? (
            <dl>
              <div><dt>{copy('field.inspect.state')}</dt><dd>{copy(inspection.reading.active ? 'field.inspect.active' : 'field.inspect.inactive')}</dd></div>
              <div><dt>{copy('field.inspect.emission')}</dt><dd>{copy(inspection.reading.emitting ? 'field.inspect.emitting' : 'field.inspect.quiet')}</dd></div>
              <div><dt>{copy('field.inspect.delivery_ceiling')}</dt><dd>{units(inspection.reading.instantaneous_ceiling)} {copy('unit.cu_per_step')}</dd></div>
              <div><dt>{copy('field.inspect.variability')}</dt><dd>{units(inspection.reading.ceiling_low)}–{units(inspection.reading.ceiling_high)} {copy('unit.cu_per_step')} · ±{percent(inspection.reading.variability)}</dd></div>
              <div><dt>{copy('field.inspect.cycle_mean')}</dt><dd>{units(inspection.reading.cycle_mean)} {copy('unit.cu_per_step')}</dd></div>
              <div><dt>{copy('field.inspect.width')}</dt><dd>{units(inspection.reading.width)}</dd></div>
              <div><dt>{copy('field.inspect.schedule')}</dt><dd>{inspection.reading.phase} / {inspection.reading.period} · {inspection.reading.on_steps} {copy('field.inspect.on_window')}</dd></div>
              <div><dt>{copy('field.inspect.recipients')}</dt><dd>{inspection.reading.recipients.join(', ') || '-'}</dd></div>
              {interventionRow(inspection.reading.active_interventions)}
            </dl>
          ) : inspection.reading.target === 'compartment' ? (
            <dl>
              <div><dt>{copy('field.inspect.members')}</dt><dd>{inspection.reading.members.join(', ') || '-'}</dd></div>
              <div><dt>{copy('field.inspect.leak_fraction')}</dt><dd>{percent(inspection.reading.leak_fraction)}</dd></div>
              <div><dt>{copy('field.inspect.leakage')}</dt><dd>{units(inspection.reading.current_leakage)} {copy('unit.cu_per_step')}</dd></div>
              {interventionRow(inspection.reading.active_interventions)}
            </dl>
          ) : inspection.reading.target === 'view' ? (
            <dl>
              <div><dt>{copy('field.inspect.state')}</dt><dd>{copy('field.inspect.passive')}</dd></div>
              <div><dt>{copy('field.inspect.members')}</dt><dd>{inspection.reading.members.join(', ') || '-'}</dd></div>
              <div><dt>{copy('field.inspect.surround')}</dt><dd>{inspection.reading.surround}</dd></div>
              <div><dt>{copy('field.inspect.resolution')}</dt><dd>{inspection.reading.resolution}</dd></div>
              <div><dt>{copy('field.inspect.window')}</dt><dd>{inspection.reading.window} {copy('unit.steps')}</dd></div>
            </dl>
          ) : inspection.reading.target === 'material' ? (
            <dl>
              <div><dt>{copy('field.inspect.kind')}</dt><dd>{inspection.reading.kind.replace('_', ' ')}</dd></div>
              <div><dt>{copy('field.inspect.amount')}</dt><dd>{inspection.reading.amount}</dd></div>
              <div><dt>{copy('field.inspect.layer')}</dt><dd>{inspection.reading.layer}</dd></div>
              <div><dt>{copy('field.inspect.state')}</dt><dd>{copy(inspection.reading.claimed ? 'field.inspect.claimed' : 'field.inspect.available')}</dd></div>
            </dl>
          ) : (
            <dl>
              <div><dt>{copy('field.inspect.charge')}</dt><dd>{units(inspection.reading.q)} {copy('unit.cu')}</dd></div>
              <div><dt>{copy('field.inspect.release_in')}</dt><dd>{inspection.reading.release_in} {copy('unit.steps')}</dd></div>
              <div><dt>{copy('field.inspect.delivery_radius')}</dt><dd>{units(inspection.reading.radius)} {copy('unit.field_units')}</dd></div>
              <div><dt>{copy('field.inspect.source_form')}</dt><dd>{inspection.reading.form}</dd></div>
            </dl>
          )}
        </aside>
      ) : null}
    </div>
  );
}
