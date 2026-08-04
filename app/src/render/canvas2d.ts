/**
 * The Canvas2D engine: the fallback the renderer engages when WebGL will not
 * start.
 *
 * It carries the same drawing responsibilities as the PixiJS engine — the
 * controlled Form, currents, Charge, trails, Routes, Ports, the boundary, the
 * depth haze, and the pressure cues — at reduced fidelity: soft gradients
 * rather than shaders, fewer particles, and no generated textures. A player who
 * lands here can still read the Field.
 *
 * It draws the same `Scene` the other engine does, so nothing about what the
 * surface means changes with the engine behind it.
 */

import {
  PHYSICAL_COMPARTMENT_SHELL,
  CHARGE_CORE,
  CHARGE_SPARK,
  CURRENT_BRIGHT,
  FORM_KEYLINE,
  GRAPHITE_GLOW,
  HAZE,
  INTERVENTION_BREACH,
  INTERVENTION_CLAMP,
  INTERVENTION_DECOY,
  INTERVENTION_DELAY,
  INTERVENTION_SCRAMBLE,
  OVERLOAD,
  ROUTE_AUTOMATION,
  ROUTE_AUTOMATION_DISABLED,
  ROUTE_SEPARATION,
  toneText,
  type Tone,
} from './palette';
import {
  ASSEMBLY_DRAFT_FIELD,
  COUPLING_EFFECT,
  depthScale,
  formRingWidth,
  HANDLE_KIND,
  type PolicyMark,
  type RouteMark,
  type Scene,
} from './scene';
import { dashOf, playbackSwell, type Engine } from './engine';

/** How far the pressure reading reaches in from the edge, at its widest. */
const RIM_REACH = 0.34;

/** Stable presentation noise. It adds texture without adding hidden state. */
function hash01(value: number): number {
  const wave = Math.sin(value * 12.9898 + 78.233) * 43_758.5453;
  return wave - Math.floor(wave);
}

function alphaText(tone: Tone, alpha: number): string {
  const held = alpha < 0 ? 0 : alpha > 1 ? 1 : alpha;
  return `rgba(${(tone >> 16) & 0xff},${(tone >> 8) & 0xff},${tone & 0xff},${held.toFixed(3)})`;
}

function policySegments(action: PolicyMark['action']): number {
  switch (action) {
    case 'seek_supply':
    case 'seek_port':
    case 'seek_signal':
      return 3;
    case 'change_depth':
    case 'set_route':
      return 2;
    case 'couple':
    case 'set_interface':
      return 4;
    case 'emit_signal':
      return 6;
    case 'use_ability':
      return 5;
    default:
      return 1;
  }
}

/** Starts a Canvas2D engine, or reports why it could not. */
export function startCanvas2dEngine(canvas: HTMLCanvasElement): Engine {
  const context = canvas.getContext('2d');
  if (!context) {
    throw new Error('The surface gave no 2D context');
  }
  return new Canvas2dEngine(canvas, context);
}

class Canvas2dEngine implements Engine {
  readonly kind = 'canvas2d' as const;

  constructor(
    private readonly canvas: HTMLCanvasElement,
    private readonly context: CanvasRenderingContext2D,
  ) {}

  resize(width: number, height: number): void {
    this.canvas.width = Math.max(1, Math.round(width));
    this.canvas.height = Math.max(1, Math.round(height));
  }

  draw(scene: Scene): void {
    const context = this.context;
    context.setTransform(1, 0, 0, 1, 0, 0);
    context.globalCompositeOperation = 'source-over';
    context.globalAlpha = 1;
    this.backdrop(scene);

    this.haze(scene, true);
    this.medium(scene);
    this.currents(scene);
    this.supplyLinks(scene);
    this.routes(scene);
    this.boundary(scene);
    this.ports(scene);
    this.trails(scene);
    this.forms(scene);
    this.policies(scene);
    this.assemblyDraft(scene);
    this.coupling(scene);
    this.particles(scene);
    this.haze(scene, false);
    this.settle(scene);
    this.handles(scene);
    this.playback(scene);
    this.forecast(scene);
    this.cues(scene);
    this.rim(scene);

    context.globalCompositeOperation = 'source-over';
    context.globalAlpha = 1;
  }

  dispose(): void {
    this.context.setTransform(1, 0, 0, 1, 0, 0);
    this.context.clearRect(0, 0, this.canvas.width, this.canvas.height);
  }

  private backdrop(scene: Scene): void {
    const context = this.context;
    context.fillStyle = toneText(scene.backdrop);
    context.fillRect(0, 0, scene.width, scene.height);
    if (scene.quality.backdropEtching) {
      context.globalCompositeOperation = 'lighter';
      context.strokeStyle = alphaText(GRAPHITE_GLOW, 0.1);
      context.lineWidth = Math.max(1, scene.dpr);
      const gap = Math.max(62, Math.min(scene.width, scene.height) * 0.085);
      for (let x = -scene.height; x < scene.width + scene.height; x += gap) {
        context.beginPath();
        context.moveTo(x, 0);
        context.lineTo(x + scene.height * 0.42, scene.height);
        context.stroke();
      }
      context.globalCompositeOperation = 'source-over';
    }
    if (!scene.quality.softMarks) return;
    const lift = context.createRadialGradient(
      scene.width / 2,
      scene.height / 2,
      0,
      scene.width / 2,
      scene.height / 2,
      Math.max(scene.width, scene.height) * 0.75,
    );
    lift.addColorStop(0, alphaText(0x1b2028, 0.55));
    lift.addColorStop(1, alphaText(0x05060a, 0.9));
    context.fillStyle = lift;
    context.fillRect(0, 0, scene.width, scene.height);

    // A living substrate beneath the causal marks: quiet motes, branching
    // microfilaments, and layered boundarys at the Field edge. All positions
    // are deterministic functions of the viewport and simulation clock.
    context.globalCompositeOperation = 'lighter';
    const pulseClock = scene.reducedMotion ? 0 : scene.clock * 0.012;
    for (let index = 0; index < 92; index += 1) {
      const x = hash01(index * 2.17 + 4) * scene.width;
      const y = hash01(index * 5.31 + 9) * scene.height;
      const pulse = 0.55 + 0.45 * Math.sin(pulseClock + index * 1.73);
      const warm = index % 7 === 0;
      context.fillStyle = alphaText(warm ? CHARGE_SPARK : CURRENT_BRIGHT, (0.025 + hash01(index + 13) * 0.08) * pulse);
      context.beginPath();
      context.arc(x, y, Math.max(0.45, scene.dpr * (0.35 + hash01(index * 3) * 0.75)), 0, Math.PI * 2);
      context.fill();
    }

    context.lineCap = 'round';
    context.lineJoin = 'round';
    for (let branch = 0; branch < 13; branch += 1) {
      const startX = hash01(branch * 7.1) * scene.width;
      const startY = hash01(branch * 3.9 + 2) * scene.height;
      const span = scene.width * (0.12 + hash01(branch + 22) * 0.18);
      const liftY = (hash01(branch + 31) - 0.5) * scene.height * 0.22;
      context.strokeStyle = alphaText(branch % 4 === 0 ? CHARGE_CORE : GRAPHITE_GLOW, branch % 4 === 0 ? 0.055 : 0.095);
      context.lineWidth = Math.max(0.45, scene.dpr * (branch % 4 === 0 ? 0.55 : 0.38));
      context.beginPath();
      context.moveTo(startX, startY);
      context.bezierCurveTo(startX + span * 0.3, startY + liftY, startX + span * 0.72, startY - liftY * 0.45, startX + span, startY + liftY * 0.16);
      context.stroke();
    }

    for (const side of [-1, 1]) {
      for (let layer = 0; layer < 4; layer += 1) {
        const inset = (10 + layer * 17) * scene.dpr;
        context.strokeStyle = alphaText(side < 0 ? PHYSICAL_COMPARTMENT_SHELL : CURRENT_BRIGHT, 0.035 + layer * 0.012);
        context.lineWidth = Math.max(0.6, scene.dpr * (1.45 - layer * 0.2));
        context.beginPath();
        for (let step = 0; step <= 28; step += 1) {
          const y = (step / 28) * scene.height;
          const ripple = Math.sin(step * 0.62 + layer * 0.8 + pulseClock) * (7 + layer * 2) * scene.dpr;
          const x = side < 0 ? inset + ripple : scene.width - inset - ripple;
          if (step === 0) context.moveTo(x, y);
          else context.lineTo(x, y);
        }
        context.stroke();
      }
    }
    context.globalCompositeOperation = 'source-over';
  }

  /** The planes behind the camera's own first, the planes in front of it last. */
  private haze(scene: Scene, behind: boolean): void {
    const context = this.context;
    for (let place = 0; place < scene.hazes.count; place += 1) {
      const mark = scene.hazes.items[place];
      if (behind !== mark.depth > 0) continue;
      context.fillStyle = alphaText(mark.tone, behind ? mark.alpha : mark.alpha * 0.5);
      context.fillRect(0, 0, scene.width, scene.height);
    }
  }

  private medium(scene: Scene): void {
    const mark = scene.medium;
    if (!mark.active) return;
    const context = this.context;
    const gap = scene.quality.backdropEtching ? 74 : 104;
    const length = 20 + Math.min(42, mark.speed * 2.5);
    context.globalCompositeOperation = 'lighter';
    context.strokeStyle = alphaText(mark.tone, mark.alpha);
    context.lineWidth = Math.max(1, scene.dpr * (0.65 + mark.drag));
    context.lineCap = 'round';
    if (mark.collisionRadius > 0) {
      context.setLineDash([4, 8]);
      context.strokeStyle = alphaText(mark.tone, mark.alpha * 1.45);
      for (let place = 0; place < scene.ports.count; place += 1) {
        const port = scene.ports.items[place];
        if (port.kind === 3) continue;
        context.beginPath();
        context.arc(port.x, port.y, mark.collisionRadius * scene.camera.zoom * depthScale(port.depth), 0, Math.PI * 2);
        context.stroke();
      }
      context.setLineDash([]);
    }
    for (let y = -gap; y < scene.height + gap; y += gap) {
      for (let x = -gap; x < scene.width + gap; x += gap) {
        const shift = (mark.offset + ((x / gap + y / gap) % 3) * gap / 3) % gap;
        const sx = x + mark.dx * shift;
        const sy = y + mark.dy * shift;
        context.beginPath();
        context.moveTo(sx, sy);
        context.lineTo(sx + mark.dx * length, sy + mark.dy * length);
        context.stroke();
      }
    }
    context.globalCompositeOperation = 'source-over';
  }

  /**
   * A current carries no path, so what it can honestly say is that it stands
   * on a layer and how strongly: a wash of its own tone over that layer's
   * plane. Where it runs is the band the delivered Ports draw.
   */
  private currents(scene: Scene): void {
    const context = this.context;
    context.globalCompositeOperation = 'lighter';
    for (let place = 0; place < scene.currents.count; place += 1) {
      const mark = scene.currents.items[place];
      if (!mark.active && !mark.delayed) continue;
      // Scaled by the plane it stands on, so a current below reads as a
      // smaller pool further away rather than as a dimmer one here.
      const reach =
        Math.max(scene.width, scene.height) * (0.5 + 0.3 * mark.strength) * depthScale(mark.depth);
      // The causal on/off window is already represented by mark alpha. This
      // small phase beat is presentation only and reduced motion silences it.
      const beat = scene.reducedMotion ? 1 : 0.85 + 0.15 * Math.sin(mark.phase * Math.PI * 2);
      if (scene.quality.currentWash && mark.active) {
        const wash = context.createRadialGradient(
          scene.width / 2,
          scene.height / 2,
          0,
          scene.width / 2,
          scene.height / 2,
          reach,
        );
        const weight =
          mark.alpha * (mark.bright ? 0.1 : 0.045) * (0.4 + 0.6 * mark.strength) * beat;
        wash.addColorStop(0, alphaText(mark.tone, weight));
        wash.addColorStop(1, alphaText(mark.tone, 0));
        context.fillStyle = wash;
        context.fillRect(0, 0, scene.width, scene.height);
      }

      // The path the current runs, at the width a Node has to stand within: a
      // wide soft band, and a bright line down the middle of it.
      const pairs = mark.points.length / 2;
      if (pairs < 2) continue;
      context.lineCap = 'round';
      context.lineJoin = 'round';
      for (const pass of [
        { width: mark.width, alpha: mark.alpha * (mark.bright ? 0.11 : 0.065) * beat },
        {
          width: Math.max(1, mark.width * 0.1),
          alpha: mark.alpha * (mark.bright ? 0.62 : 0.4) * beat,
        },
      ]) {
        context.strokeStyle = alphaText(mark.tone, pass.alpha);
        context.lineWidth = pass.width;
        context.beginPath();
        context.moveTo(mark.points[0], mark.points[1]);
        for (let point = 1; point < pairs; point += 1) {
          context.lineTo(mark.points[point * 2], mark.points[point * 2 + 1]);
        }
        context.stroke();
      }

      // The Current reads as a braid of living transport rather than a flat
      // ribbon. Fine filaments remain inside the authoritative delivery band.
      const fibers = scene.quality.id === 'high' ? 7 : 4;
      for (let fiber = 0; fiber < fibers; fiber += 1) {
        const centered = fiber - (fibers - 1) / 2;
        context.strokeStyle = alphaText(
          fiber % 3 === 0 && mark.bright ? CHARGE_SPARK : mark.tone,
          mark.alpha * (0.08 + (fiber % 2) * 0.055) * beat,
        );
        context.lineWidth = Math.max(0.55, scene.dpr * (fiber % 3 === 0 ? 0.75 : 0.45));
        for (let point = 1; point < pairs; point += 1) {
          const x1 = mark.points[(point - 1) * 2];
          const y1 = mark.points[(point - 1) * 2 + 1];
          const x2 = mark.points[point * 2];
          const y2 = mark.points[point * 2 + 1];
          const length = Math.max(1, Math.hypot(x2 - x1, y2 - y1));
          const nx = -(y2 - y1) / length;
          const ny = (x2 - x1) / length;
          context.beginPath();
          for (let sample = 0; sample <= 12; sample += 1) {
            const along = sample / 12;
            const wave = Math.sin(along * Math.PI * 3 + fiber * 1.27 + mark.phase * Math.PI * 2);
            const offset = centered * mark.width * 0.11 + wave * mark.width * 0.045;
            const x = x1 + (x2 - x1) * along + nx * offset;
            const y = y1 + (y2 - y1) * along + ny * offset;
            if (sample === 0) context.moveTo(x, y);
            else context.lineTo(x, y);
          }
          context.stroke();
        }
      }
      if (mark.stress > 0) {
        const stressBeat =
          mark.stressCrisis && !scene.reducedMotion ? 0.78 + 0.22 * Math.sin(scene.clock * 0.55) : 1;
        context.strokeStyle = alphaText(mark.stressTone, mark.stress * 0.48 * stressBeat);
        context.lineWidth = Math.max(1, mark.width * 0.1);
        context.beginPath();
        context.moveTo(mark.points[0], mark.points[1]);
        for (let point = 1; point < pairs; point += 1) {
          context.lineTo(mark.points[point * 2], mark.points[point * 2 + 1]);
        }
        context.stroke();
      }
      if (mark.diverted) {
        context.setLineDash([Math.max(4, mark.width * 0.22), Math.max(5, mark.width * 0.3)]);
        context.strokeStyle = alphaText(INTERVENTION_DECOY, 0.9 * mark.alpha);
        context.lineWidth = Math.max(1.5, mark.width * 0.11);
        context.beginPath();
        context.moveTo(mark.points[0], mark.points[1]);
        for (let point = 1; point < pairs; point += 1) {
          context.lineTo(mark.points[point * 2], mark.points[point * 2 + 1]);
        }
        context.stroke();
        context.setLineDash([]);
      }
      if (mark.delayed) {
        this.holdBars(mark.points, mark.width, INTERVENTION_DELAY, 0.95 * mark.alpha);
      }
    }
    context.globalCompositeOperation = 'source-over';
  }

  private routes(scene: Scene): void {
    const context = this.context;
    context.lineCap = 'round';
    for (let place = 0; place < scene.routes.count; place += 1) {
      const mark = scene.routes.items[place];
      // A queued change is drawn broken, whichever kind it is: what the queue
      // would do never reads as something the Field already holds.
      context.setLineDash(
        mark.preview
          ? [6, 8]
          : !mark.automationEnabled || mark.transferOutcome === 'closed'
            ? [1.5, 6]
            : mark.gated
              ? [2, 7]
              : mark.transferOutcome === 'source_starved'
                ? [2, 9]
                : mark.transferOutcome === 'destination_headroom'
                  ? [10, 4]
                  : [],
      );
      context.strokeStyle = alphaText(ROUTE_SEPARATION, 0.82 * mark.alpha);
      context.lineWidth = mark.width + Math.max(2, scene.dpr * 2.2);
      context.beginPath();
      context.moveTo(mark.x1, mark.y1);
      context.lineTo(mark.x2, mark.y2);
      context.stroke();

      if (mark.scrambled) {
        context.setLineDash([4, 5]);
        context.strokeStyle = alphaText(INTERVENTION_SCRAMBLE, 0.92 * mark.alpha);
        context.lineWidth = Math.max(1, mark.width * 0.42);
        context.beginPath();
        context.moveTo(mark.x1, mark.y1);
        context.lineTo(mark.x2, mark.y2);
        context.stroke();
        context.setLineDash([]);
      }
      if (mark.clamped) {
        this.routeClamp(mark.x1, mark.y1, mark.x2, mark.y2, mark.width, mark.alpha);
      }
      if (mark.stress > 0) {
        const stressBeat =
          mark.stressCrisis && !scene.reducedMotion ? 0.78 + 0.22 * Math.sin(scene.clock * 0.55) : 1;
        context.strokeStyle = alphaText(mark.stressTone, mark.stress * 0.34 * stressBeat);
        context.lineWidth = mark.width * (2.4 + mark.stress);
        context.beginPath();
        context.moveTo(mark.x1, mark.y1);
        context.lineTo(mark.x2, mark.y2);
        context.stroke();
      }
      if (scene.quality.routeGlow && !mark.preview && mark.flow > 0) {
        context.strokeStyle = alphaText(CHARGE_SPARK, mark.alpha * 0.12 * mark.flow);
        context.lineWidth = mark.width * 3.6;
        context.beginPath();
        context.moveTo(mark.x1, mark.y1);
        context.lineTo(mark.x2, mark.y2);
        context.stroke();
      }
      context.strokeStyle = alphaText(mark.tone, mark.alpha);
      context.lineWidth = mark.width;
      context.beginPath();
      context.moveTo(mark.x1, mark.y1);
      context.lineTo(mark.x2, mark.y2);
      context.stroke();

      if (mark.gated && !mark.preview) {
        this.routeGates(mark.x1, mark.y1, mark.x2, mark.y2, mark.tailOpen, mark.headOpen, mark.width, mark.alpha);
      } else if (!mark.preview && mark.automationEnabled) {
        this.routeDirection(mark.x1, mark.y1, mark.x2, mark.y2, mark.width, mark.alpha, mark.flow);
      }
      this.routeAutomationControl(mark);
      this.routeTransferConstraint(mark);

      if (!mark.preview && mark.flow > 0.02) {
        const dx = mark.x2 - mark.x1;
        const dy = mark.y2 - mark.y1;
        const span = Math.max(1, Math.hypot(dx, dy));
        const nx = -dy / span;
        const ny = dx / span;
        const count = Math.max(3, Math.min(14, Math.floor(span / 42)));
        const drift = scene.reducedMotion ? 0 : (scene.clock * (0.018 + mark.flow * 0.025)) % 1;
        context.strokeStyle = alphaText(CHARGE_SPARK, mark.alpha * (0.3 + mark.flow * 0.42));
        context.lineWidth = Math.max(0.7, scene.dpr * 0.7);
        context.beginPath();
        for (let tick = 0; tick < count; tick += 1) {
          const along = (tick / count + drift) % 1;
          const x = mark.x1 + dx * along;
          const y = mark.y1 + dy * along;
          const reach = Math.max(2.5, mark.width * (0.85 + mark.flow));
          context.moveTo(x - nx * reach, y - ny * reach);
          context.lineTo(x + nx * reach, y + ny * reach);
        }
        context.stroke();
      }
    }
    context.setLineDash([]);
  }

  private supplyLinks(scene: Scene): void {
    const context = this.context;
    context.globalCompositeOperation = 'lighter';
    context.lineCap = 'round';
    for (let place = 0; place < scene.supplyLinks.count; place += 1) {
      const mark = scene.supplyLinks.items[place];
      const dx = mark.x2 - mark.x1;
      const dy = mark.y2 - mark.y1;
      const length = Math.max(1, Math.hypot(dx, dy));
      const nx = -dy / length;
      const ny = dx / length;
      for (let filament = -1; filament <= 1; filament += 1) {
        const bow = filament * Math.min(12 * scene.dpr, length * 0.18);
        context.strokeStyle = alphaText(
          filament === 0 ? CHARGE_SPARK : mark.tone,
          mark.alpha * (filament === 0 ? 0.9 : 0.5),
        );
        context.lineWidth = Math.max(0.6, mark.width * (filament === 0 ? 1 : 0.58));
        context.beginPath();
        context.moveTo(mark.x1, mark.y1);
        context.bezierCurveTo(
          mark.x1 + dx * 0.33 + nx * bow,
          mark.y1 + dy * 0.33 + ny * bow,
          mark.x1 + dx * 0.67 + nx * bow,
          mark.y1 + dy * 0.67 + ny * bow,
          mark.x2,
          mark.y2,
        );
        context.stroke();
      }
      for (let bead = 0; bead < 3; bead += 1) {
        const along = (mark.phase + bead / 3) % 1;
        context.fillStyle = alphaText(CHARGE_SPARK, mark.alpha * Math.sin(Math.PI * along));
        context.beginPath();
        context.arc(
          mark.x1 + dx * along,
          mark.y1 + dy * along,
          Math.max(1, mark.width * 1.4),
          0,
          Math.PI * 2,
        );
        context.fill();
      }
    }
    context.globalCompositeOperation = 'source-over';
  }

  private boundary(scene: Scene): void {
    // Candidates first; the material compartment and active View then remain
    // legible over every possible observation aperture.
    this.strokeHulls(scene.candidates, scene);
    this.strokeHulls(scene.boundaries, scene);
    this.strokeHulls(scene.assemblyBoundaries, scene);
  }

  /** One pool of closed hulls, drawn in the register each mark declares. */
  private strokeHulls(pool: Scene['boundaries'], scene: Scene): void {
    const context = this.context;
    for (let place = 0; place < pool.count; place += 1) {
      const mark = pool.items[place];
      const pairs = mark.points.length / 2;
      if (pairs === 0) continue;
      // A hull with no area — one member, two, or three in a line — is drawn
      // as a capsule of the hull's own width, so the reading holds. Its dash
      // register still applies: a small View or proposal must not turn into the
      // same solid object as a small physical compartment.
      if (pairs <= 2) {
        context.lineCap = 'round';
        context.setLineDash(dashOf(mark));
        context.strokeStyle = alphaText(mark.tone, mark.alpha);
        context.lineWidth = mark.width;
        if (pairs === 1) {
          context.beginPath();
          context.arc(mark.points[0], mark.points[1], mark.width, 0, Math.PI * 2);
          if (mark.role === 'compartment' && !mark.proposed) {
            context.fillStyle = alphaText(mark.tone, mark.alpha * 0.07);
            context.fill();
          }
          context.stroke();
        } else {
          context.beginPath();
          context.moveTo(mark.points[0], mark.points[1]);
          context.lineTo(mark.points[2], mark.points[3]);
          context.stroke();
        }
        if (mark.stress > 0) {
          context.setLineDash([]);
          context.strokeStyle = alphaText(mark.stressTone, mark.stress * 0.5);
          context.lineWidth = mark.width * (1.5 + mark.stress);
          context.beginPath();
          if (pairs === 1) context.arc(mark.points[0], mark.points[1], mark.width * 1.2, 0, Math.PI * 2);
          else {
            context.moveTo(mark.points[0], mark.points[1]);
            context.lineTo(mark.points[2], mark.points[3]);
          }
          context.stroke();
        }
        context.setLineDash([]);
        continue;
      }
      context.beginPath();
      context.moveTo(mark.points[0], mark.points[1]);
      for (let point = 2; point < mark.points.length; point += 2) {
        context.lineTo(mark.points[point], mark.points[point + 1]);
      }
      context.closePath();
      // Only the committed physical compartment owns a material interior.
      // The passive View and candidates remain apertures/outlines.
      if (mark.role === 'compartment' && !mark.proposed) {
        context.fillStyle = alphaText(mark.tone, mark.alpha * 0.07);
        context.fill();
        // A broad low-alpha shoulder gives the compartment physical weight.
        context.strokeStyle = alphaText(mark.tone, mark.alpha * 0.12);
        context.lineWidth = mark.width * 2.4;
        context.lineJoin = 'round';
        context.stroke();
        if (mark.stress > 0) {
          const stressBeat =
            mark.stressCrisis && !scene.reducedMotion ? 0.78 + 0.22 * Math.sin(scene.clock * 0.55) : 1;
          context.strokeStyle = alphaText(mark.stressTone, mark.stress * 0.42 * stressBeat);
          context.lineWidth = mark.width * (1.4 + 1.8 * mark.stress);
          context.stroke();
        }
      }
      // One dash pattern per register, from the rule both engines read.
      context.setLineDash(dashOf(mark));
      context.strokeStyle = alphaText(mark.tone, mark.alpha);
      context.lineWidth = mark.width;
      context.stroke();
      context.setLineDash([]);
    }
  }

  private ports(scene: Scene): void {
    const context = this.context;
    for (let place = 0; place < scene.ports.count; place += 1) {
      const mark = scene.ports.items[place];
      if (scene.quality.softMarks) {
        context.globalCompositeOperation = 'lighter';
        const bloom = context.createRadialGradient(mark.x, mark.y, 0, mark.x, mark.y, mark.bloom);
        bloom.addColorStop(0, alphaText(mark.tone, 0.5 * mark.charge * mark.alpha));
        bloom.addColorStop(1, alphaText(mark.tone, 0));
        context.fillStyle = bloom;
        context.fillRect(mark.x - mark.bloom, mark.y - mark.bloom, mark.bloom * 2, mark.bloom * 2);
      }

      if (mark.delivered > 0) {
        context.strokeStyle = alphaText(CURRENT_BRIGHT, 0.85 * mark.delivered);
        context.lineWidth = Math.max(1, mark.radius * 0.3);
        context.beginPath();
        context.arc(mark.x, mark.y, mark.radius * (1.6 + 1.4 * (1 - mark.delivered)), 0, Math.PI * 2);
        context.stroke();
      }

      context.globalCompositeOperation = 'source-over';
      context.strokeStyle = alphaText(0x050608, 0.5 * mark.alpha);
      context.lineWidth = Math.max(1, mark.radius * 0.16);
      const dormant = mark.kind === 0 && !mark.open;
      context.fillStyle = alphaText(dormant ? 0x06070a : mark.tone, dormant ? 0.74 * mark.alpha : mark.alpha);
      this.nodeShape(mark.x, mark.y, mark.radius, mark.kind);
      context.fill();
      context.stroke();

      if (dormant) {
        context.strokeStyle = alphaText(mark.tone, 0.88 * mark.alpha);
        context.lineWidth = Math.max(1, mark.radius * 0.18);
        context.beginPath();
        for (let gate = 0; gate < 3; gate += 1) {
          const angle = gate * Math.PI * 2 / 3 - Math.PI / 2;
          context.arc(mark.x, mark.y, mark.radius * 1.18, angle + 0.16, angle + Math.PI * 0.48);
        }
        context.stroke();
      }

      // Ports are layered structures, not dots. A translucent boundary and
      // a charged nucleus make capacity, delivery, and dormancy legible even
      // before a special-state ring appears.
      context.globalCompositeOperation = 'lighter';
      context.strokeStyle = alphaText(mark.tone, (0.22 + mark.charge * 0.34) * mark.alpha);
      context.lineWidth = Math.max(0.55, scene.dpr * 0.7);
      context.beginPath();
      context.arc(mark.x, mark.y, mark.radius * 1.42, 0, Math.PI * 2);
      context.stroke();
      context.fillStyle = alphaText(
        dormant ? mark.tone : CHARGE_SPARK,
        (dormant ? 0.18 : 0.12 + mark.charge * 0.48) * mark.alpha,
      );
      context.beginPath();
      context.arc(mark.x, mark.y, Math.max(1, mark.radius * (0.13 + mark.charge * 0.16)), 0, Math.PI * 2);
      context.fill();
      context.globalCompositeOperation = 'source-over';

      if (mark.stress > 0) {
        const stressBeat =
          mark.stressCrisis && !scene.reducedMotion ? 0.78 + 0.22 * Math.sin(scene.clock * 0.55) : 1;
        context.strokeStyle = alphaText(mark.stressTone, mark.stress * 0.72 * stressBeat);
        context.lineWidth = Math.max(1, mark.radius * 0.2);
        context.beginPath();
        context.arc(mark.x, mark.y, mark.radius * 1.55, -Math.PI * 0.8, Math.PI * 0.7);
        context.stroke();
      }

      if (mark.reserve > 0) {
        context.strokeStyle = alphaText(CHARGE_CORE, 0.6 * mark.alpha);
        context.lineWidth = Math.max(1, mark.radius * 0.22);
        context.beginPath();
        context.arc(mark.x, mark.y, mark.radius * 0.55, -Math.PI / 2, -Math.PI / 2 + Math.PI * 2 * mark.reserve);
        context.stroke();
      }
      if (mark.reserveDelta !== 0) {
        const banking = mark.reserveDelta > 0;
        const strength = Math.min(1, Math.abs(mark.reserveDelta) * 10);
        context.strokeStyle = alphaText(banking ? CURRENT_BRIGHT : CHARGE_SPARK, (0.38 + 0.5 * strength) * mark.alpha);
        context.lineWidth = Math.max(1, mark.radius * (banking ? 0.14 : 0.24));
        context.beginPath();
        context.arc(mark.x, mark.y, mark.radius * (banking ? 0.78 : 1.08), 0, Math.PI * 2);
        context.stroke();
      }
      if (mark.overloaded) {
        context.strokeStyle = alphaText(OVERLOAD, 0.95);
        context.lineWidth = Math.max(1, mark.radius * 0.35);
        context.beginPath();
        context.arc(mark.x, mark.y, mark.radius * 1.35, 0, Math.PI * 2);
        context.stroke();
      }
      if (mark.shell) {
        context.strokeStyle = alphaText(PHYSICAL_COMPARTMENT_SHELL, 0.75);
        context.lineWidth = Math.max(1, mark.radius * 0.18);
        context.beginPath();
        context.arc(mark.x, mark.y, mark.radius * 1.9, 0, Math.PI * 2);
        context.stroke();
      }
      if (mark.decoyReceiver) {
        context.setLineDash([3, 4]);
        context.strokeStyle = alphaText(INTERVENTION_DECOY, 0.95);
        context.lineWidth = Math.max(1, mark.radius * 0.22);
        context.beginPath();
        context.arc(mark.x, mark.y, mark.radius * 2.35, 0, Math.PI * 2);
        context.stroke();
        context.setLineDash([]);
      }
      if (mark.breached) {
        context.strokeStyle = alphaText(INTERVENTION_BREACH, 0.96);
        context.lineWidth = Math.max(1.5, mark.radius * 0.28);
        context.beginPath();
        context.arc(mark.x, mark.y, mark.radius * 2.05, Math.PI * 0.08, Math.PI * 0.72);
        context.stroke();
      }
    }
  }

  private routeClamp(
    x1: number,
    y1: number,
    x2: number,
    y2: number,
    width: number,
    alpha: number,
  ): void {
    this.crossBars(x1, y1, x2, y2, width, INTERVENTION_CLAMP, alpha);
  }

  private routeAutomationControl(mark: RouteMark): void {
    if (mark.preview || (mark.automationEnabled && !mark.automationLimited)) return;
    const span = Math.hypot(mark.x2 - mark.x1, mark.y2 - mark.y1);
    if (span <= 0) return;
    const tx = (mark.x2 - mark.x1) / span;
    const ty = (mark.y2 - mark.y1) / span;
    const nx = -ty;
    const ny = tx;
    const reach = Math.max(3.5, mark.width * (mark.automationEnabled ? 1.45 : 2.35));
    const center = span * 0.24;
    const separation = Math.max(2.5, mark.width * 1.1);
    const bars = mark.automationEnabled ? [0] : [-separation, separation];
    const context = this.context;
    context.setLineDash([]);
    context.strokeStyle = alphaText(
      mark.automationEnabled ? ROUTE_AUTOMATION : ROUTE_AUTOMATION_DISABLED,
      Math.max(0.5, mark.alpha),
    );
    context.lineWidth = Math.max(1, mark.width * 0.58);
    context.beginPath();
    for (const offset of bars) {
      const x = mark.x1 + tx * (center + offset);
      const y = mark.y1 + ty * (center + offset);
      context.moveTo(x - nx * reach, y - ny * reach);
      context.lineTo(x + nx * reach, y + ny * reach);
    }
    context.stroke();
  }

  /** Endpoint and midline bars identify the physical term limiting acceptance. */
  private routeTransferConstraint(mark: RouteMark): void {
    if (mark.preview || ![
      'source_starved',
      'destination_headroom',
      'capacity_throttled',
    ].includes(mark.transferOutcome)) return;
    const dx = mark.x2 - mark.x1;
    const dy = mark.y2 - mark.y1;
    const span = Math.hypot(dx, dy);
    if (span <= 0) return;
    const tx = dx / span;
    const ty = dy / span;
    const nx = -ty;
    const ny = tx;
    const along = mark.transferOutcome === 'source_starved'
      ? 0.08
      : mark.transferOutcome === 'destination_headroom'
        ? 0.92
        : 0.5;
    const x = mark.x1 + dx * along;
    const y = mark.y1 + dy * along;
    const reach = Math.max(4, mark.width * 1.8);
    const separation = mark.transferOutcome === 'destination_headroom'
      ? Math.max(2, mark.width * 0.65)
      : 0;
    this.context.setLineDash([]);
    this.context.strokeStyle = alphaText(mark.tone, Math.max(0.62, mark.alpha));
    this.context.lineWidth = Math.max(1, mark.width * 0.52);
    this.context.beginPath();
    for (const offset of separation > 0 ? [-separation, separation] : [0]) {
      this.context.moveTo(x + tx * offset - nx * reach, y + ty * offset - ny * reach);
      this.context.lineTo(x + tx * offset + nx * reach, y + ty * offset + ny * reach);
    }
    this.context.stroke();
  }

  private routeGates(
    x1: number,
    y1: number,
    x2: number,
    y2: number,
    tailOpen: boolean,
    headOpen: boolean,
    width: number,
    alpha: number,
  ): void {
    const span = Math.hypot(x2 - x1, y2 - y1);
    if (span <= 0) return;
    const nx = -(y2 - y1) / span;
    const ny = (x2 - x1) / span;
    const reach = Math.max(4, width * 2.4);
    const context = this.context;
    context.setLineDash([]);
    context.strokeStyle = alphaText(CURRENT_BRIGHT, Math.max(0.32, alpha));
    context.lineWidth = Math.max(1, width * 0.72);
    context.beginPath();
    if (!tailOpen) {
      context.moveTo(x1 - nx * reach, y1 - ny * reach);
      context.lineTo(x1 + nx * reach, y1 + ny * reach);
    }
    if (!headOpen) {
      context.moveTo(x2 - nx * reach, y2 - ny * reach);
      context.lineTo(x2 + nx * reach, y2 + ny * reach);
    }
    context.stroke();
  }

  private routeDirection(
    x1: number,
    y1: number,
    x2: number,
    y2: number,
    width: number,
    alpha: number,
    flow: number,
  ): void {
    const span = Math.hypot(x2 - x1, y2 - y1);
    if (span <= 0) return;
    const tx = (x2 - x1) / span;
    const ty = (y2 - y1) / span;
    const nx = -ty;
    const ny = tx;
    const x = x1 + (x2 - x1) * 0.62;
    const y = y1 + (y2 - y1) * 0.62;
    const reach = Math.max(3.5, width * 1.65);
    const back = Math.max(4.5, width * 2.2);
    const context = this.context;
    context.setLineDash([]);
    context.strokeStyle = alphaText(CHARGE_SPARK, alpha * (0.32 + flow * 0.48));
    context.lineWidth = Math.max(0.8, width * 0.42);
    context.beginPath();
    context.moveTo(x - tx * back + nx * reach, y - ty * back + ny * reach);
    context.lineTo(x, y);
    context.lineTo(x - tx * back - nx * reach, y - ty * back - ny * reach);
    context.stroke();
  }

  private crossBars(
    x1: number,
    y1: number,
    x2: number,
    y2: number,
    width: number,
    tone: Tone,
    alpha: number,
  ): void {
    const span = Math.hypot(x2 - x1, y2 - y1);
    if (span <= 0) return;
    const tx = (x2 - x1) / span;
    const ty = (y2 - y1) / span;
    const nx = -ty;
    const ny = tx;
    const middleX = (x1 + x2) / 2;
    const middleY = (y1 + y2) / 2;
    const reach = Math.max(5, width * 1.45);
    const gap = Math.max(3, width * 0.9);
    const context = this.context;
    context.strokeStyle = alphaText(tone, 0.96 * alpha);
    context.lineWidth = Math.max(1.5, width * 0.34);
    context.beginPath();
    for (const shift of [-gap, gap]) {
      const x = middleX + tx * shift;
      const y = middleY + ty * shift;
      context.moveTo(x - nx * reach, y - ny * reach);
      context.lineTo(x + nx * reach, y + ny * reach);
    }
    context.stroke();
  }

  private holdBars(points: number[], width: number, tone: Tone, alpha: number): void {
    if (points.length < 4) return;
    this.crossBars(
      points[0],
      points[1],
      points[points.length - 2],
      points[points.length - 1],
      width,
      tone,
      alpha,
    );
  }

  /** One shape per Node kind, so what a Node is reads without a word for it. */
  private nodeShape(x: number, y: number, radius: number, kind: number): void {
    const context = this.context;
    context.beginPath();
    if (kind === 1) {
      context.rect(x - radius, y - radius, radius * 2, radius * 2);
      return;
    }
    if (kind === 2) {
      context.moveTo(x, y - radius * 1.2);
      context.lineTo(x + radius * 1.2, y);
      context.lineTo(x, y + radius * 1.2);
      context.lineTo(x - radius * 1.2, y);
      context.closePath();
      return;
    }
    context.arc(x, y, radius, 0, Math.PI * 2);
  }

  /** Begins a closed path from pooled device-pixel pairs. */
  private polygon(points: readonly number[]): void {
    const context = this.context;
    context.beginPath();
    context.moveTo(points[0], points[1]);
    for (let point = 2; point < points.length; point += 2) {
      context.lineTo(points[point], points[point + 1]);
    }
    context.closePath();
  }

  private trails(scene: Scene): void {
    const context = this.context;
    context.globalCompositeOperation = 'lighter';
    context.lineCap = 'round';
    context.lineJoin = 'round';
    for (let place = 0; place < scene.trails.count; place += 1) {
      const mark = scene.trails.items[place];
      const pairs = mark.points.length / 2;
      if (pairs < 2) continue;
      for (let point = 1; point < pairs; point += 1) {
        const along = point / (pairs - 1);
        context.strokeStyle = alphaText(mark.tone, mark.alpha * along * along);
        context.lineWidth = Math.max(0.5, mark.width * along);
        context.beginPath();
        context.moveTo(mark.points[(point - 1) * 2], mark.points[(point - 1) * 2 + 1]);
        context.lineTo(mark.points[point * 2], mark.points[point * 2 + 1]);
        context.stroke();
      }
    }
    context.globalCompositeOperation = 'source-over';
  }

  private forms(scene: Scene): void {
    const context = this.context;
    for (let place = 0; place < scene.forms.count; place += 1) {
      const mark = scene.forms.items[place];
      const pairs = mark.outline.length / 2;
      if (pairs < 3) continue;

      if (scene.quality.softMarks) {
        context.globalCompositeOperation = 'lighter';
        const reach = mark.radius * (mark.controlled ? 3.8 : 2.4);
        const halo = context.createRadialGradient(mark.x, mark.y, 0, mark.x, mark.y, reach);
        halo.addColorStop(0, alphaText(mark.ringTone, (mark.controlled ? 0.32 : 0.18) * mark.alpha));
        halo.addColorStop(1, alphaText(mark.ringTone, 0));
        context.fillStyle = halo;
        context.fillRect(mark.x - reach, mark.y - reach, reach * 2, reach * 2);
        context.globalCompositeOperation = 'source-over';
      }

      if (mark.controlled || scene.quality.formFacets > 1) {
        context.globalCompositeOperation = 'lighter';
        context.save();
        context.translate(mark.x, mark.y);
        context.rotate(mark.heading);
        for (let filament = -2; filament <= 2; filament += 1) {
          const spread = filament * mark.radius * 0.17;
          const reach = mark.radius * (1.35 + Math.abs(filament) * 0.12 + mark.formOrdinal * 0.015);
          context.strokeStyle = alphaText(mark.ringTone, (0.14 + (2 - Math.abs(filament)) * 0.03) * mark.alpha);
          context.lineWidth = Math.max(0.55, mark.radius * 0.018);
          context.beginPath();
          context.moveTo(-mark.radius * 0.32, spread * 0.2);
          context.bezierCurveTo(-reach * 0.54, spread * 1.4, -reach * 0.82, -spread * 0.7, -reach, spread * 0.36);
          context.stroke();
        }
        context.restore();
        context.setLineDash([Math.max(2, mark.radius * 0.08), Math.max(3, mark.radius * 0.12)]);
        for (let boundary = 0; boundary < 2; boundary += 1) {
          context.strokeStyle = alphaText(mark.ringTone, (0.18 - boundary * 0.035) * mark.alpha);
          context.lineWidth = Math.max(0.55, scene.dpr * 0.65);
          context.beginPath();
          context.arc(mark.x, mark.y, mark.radius * (1.34 + boundary * 0.22), mark.heading + boundary, mark.heading + Math.PI * (1.42 + boundary * 0.16));
          context.stroke();
        }
        context.setLineDash([]);
        context.globalCompositeOperation = 'source-over';
      }

      this.polygon(mark.outline);
      context.fillStyle = alphaText(mark.bodyTone, 0.9 * mark.alpha);
      context.fill();

      if (scene.quality.formFacets > 0) {
        const step = scene.quality.formFacets === 1 ? 2 : 1;
        for (let point = 0; point < pairs; point += step) {
          const next = (point + 1) % pairs;
          context.beginPath();
          context.moveTo(mark.x, mark.y);
          context.lineTo(mark.outline[point * 2], mark.outline[point * 2 + 1]);
          context.lineTo(mark.outline[next * 2], mark.outline[next * 2 + 1]);
          context.closePath();
          context.fillStyle = alphaText(
            point % 2 === 0 ? mark.facetTone : mark.facetShadow,
            (mark.controlled ? 0.13 : 0.08) * mark.alpha,
          );
          context.fill();
        }
        this.polygon(mark.core);
        context.fillStyle = alphaText(mark.facetShadow, 0.72 * mark.alpha);
        context.fill();
        context.strokeStyle = alphaText(mark.facetTone, (mark.controlled ? 0.48 : 0.24) * mark.alpha);
        context.lineWidth = Math.max(1, mark.radius * 0.035);
        context.stroke();
      }

      if (scene.quality.formFacets > 0) {
        context.globalCompositeOperation = 'lighter';
        const spokes = Math.min(pairs, mark.controlled ? 9 : 6);
        for (let spoke = 0; spoke < spokes; spoke += 1) {
          const point = Math.floor((spoke / spokes) * pairs);
          context.strokeStyle = alphaText(mark.facetTone, (mark.controlled ? 0.16 : 0.09) * mark.alpha);
          context.lineWidth = Math.max(0.5, scene.dpr * 0.55);
          context.beginPath();
          context.moveTo(mark.x, mark.y);
          context.lineTo(mark.outline[point * 2], mark.outline[point * 2 + 1]);
          context.stroke();
        }
        const orbit = mark.radius * (0.58 + 0.06 * (mark.formOrdinal % 3));
        context.strokeStyle = alphaText(mark.ringTone, 0.2 * mark.alpha);
        context.beginPath();
        context.ellipse(mark.x, mark.y, orbit, orbit * 0.42, mark.heading + mark.formOrdinal * 0.34, 0, Math.PI * 2);
        context.stroke();
        const orbitPhase = scene.reducedMotion ? mark.formOrdinal : scene.clock * 0.035 + mark.formOrdinal;
        context.fillStyle = alphaText(CHARGE_SPARK, (0.34 + mark.charge * 0.45) * mark.alpha);
        context.beginPath();
        context.arc(
          mark.x + Math.cos(orbitPhase) * orbit,
          mark.y + Math.sin(orbitPhase) * orbit * 0.42,
          Math.max(1, mark.radius * 0.045),
          0,
          Math.PI * 2,
        );
        context.fill();

        if (mark.formOrdinal === 0) {
          context.strokeStyle = alphaText(mark.facetTone, 0.34 * mark.alpha);
          context.lineWidth = Math.max(0.65, scene.dpr * 0.7);
          for (const side of [-1, 1]) {
            context.beginPath();
            for (let sample = 0; sample <= 18; sample += 1) {
              const along = sample / 18;
              const y = mark.y - mark.radius * 0.64 + along * mark.radius * 1.28;
              const x = mark.x + Math.sin(along * Math.PI * 3.2) * mark.radius * 0.19 * side;
              if (sample === 0) context.moveTo(x, y);
              else context.lineTo(x, y);
            }
            context.stroke();
          }
          context.strokeStyle = alphaText(mark.ringTone, 0.28 * mark.alpha);
          context.beginPath();
          for (let rung = 2; rung < 18; rung += 3) {
            const along = rung / 18;
            const y = mark.y - mark.radius * 0.64 + along * mark.radius * 1.28;
            const offset = Math.sin(along * Math.PI * 3.2) * mark.radius * 0.19;
            context.moveTo(mark.x - offset, y);
            context.lineTo(mark.x + offset, y);
          }
          context.stroke();
        }
        context.globalCompositeOperation = 'source-over';
      }

      this.polygon(mark.outline);
      if (mark.stress > 0) {
        const stressBeat =
          mark.stressCrisis && !scene.reducedMotion ? 0.78 + 0.22 * Math.sin(scene.clock * 0.55) : 1;
        context.strokeStyle = alphaText(mark.stressTone, mark.stress * 0.5 * stressBeat);
        context.lineWidth = formRingWidth(mark) * (2 + mark.stress);
        context.stroke();
      }
      this.polygon(mark.outline);
      const selected = mark.controlled || mark.focus;
      context.strokeStyle = alphaText(FORM_KEYLINE, (selected ? 0.92 : 0.62) * mark.alpha);
      context.lineWidth = formRingWidth(mark) + Math.max(1, scene.dpr * (selected ? 1.5 : 0.7));
      context.stroke();
      this.polygon(mark.outline);
      context.strokeStyle = alphaText(mark.ringTone, mark.alpha);
      context.lineWidth = formRingWidth(mark);
      context.stroke();

      context.beginPath();
      context.arc(mark.x, mark.y, mark.radius * (0.2 + 0.42 * mark.charge), 0, Math.PI * 2);
      context.fillStyle = alphaText(mark.tone, mark.alpha);
      context.fill();

      if (mark.focus) {
        context.beginPath();
        context.arc(mark.x, mark.y, mark.radius * 1.55, 0, Math.PI * 2);
        context.strokeStyle = alphaText(mark.ringTone, 0.6 * mark.alpha);
        context.lineWidth = Math.max(1, mark.radius * 0.09);
        context.stroke();
      }
      if (mark.controlled) {
        context.beginPath();
        context.arc(mark.x, mark.y, mark.radius * 1.24, 0, Math.PI * 2);
        context.strokeStyle = alphaText(mark.ringTone, 0.34 * mark.alpha);
        context.lineWidth = Math.max(1, mark.radius * 0.055);
        context.stroke();
        if (mark.charge > 0.001) {
          context.strokeStyle = alphaText(CHARGE_SPARK, (0.48 + 0.42 * mark.charge) * mark.alpha);
          context.lineWidth = Math.max(1.2, mark.radius * 0.075);
          context.beginPath();
          context.arc(
            mark.x,
            mark.y,
            mark.radius * 1.38,
            -Math.PI / 2,
            -Math.PI / 2 + Math.PI * 2 * mark.charge,
          );
          context.stroke();
        }
      }
      if (mark.pulseCharging) {
        const beat = scene.reducedMotion ? 1 : 0.6 + 0.4 * Math.sin(scene.clock * 0.9);
        context.beginPath();
        context.arc(mark.x, mark.y, mark.radius * 0.85 * beat, 0, Math.PI * 2);
        context.strokeStyle = alphaText(CHARGE_CORE, 0.75 * mark.alpha);
        context.lineWidth = Math.max(1, mark.radius * 0.14);
        context.stroke();
      }
    }
  }

  private policies(scene: Scene): void {
    const context = this.context;
    context.globalCompositeOperation = 'lighter';
    context.lineCap = 'round';
    context.lineJoin = 'round';
    for (let place = 0; place < scene.policyCandidates.count; place += 1) {
      const candidate = scene.policyCandidates.items[place];
      context.setLineDash([Math.max(2, scene.dpr * 2), Math.max(4, scene.dpr * 4)]);
      context.strokeStyle = alphaText(candidate.tone, candidate.alpha);
      context.lineWidth = Math.max(0.75, scene.dpr * (candidate.selected ? 1.2 : 0.8));
      context.beginPath();
      context.arc(candidate.x, candidate.y, candidate.radius, 0, Math.PI * 2);
      context.stroke();
    }
    for (let place = 0; place < scene.policies.count; place += 1) {
      const mark = scene.policies.items[place];
      const beat = scene.reducedMotion ? 1 : 0.9 + 0.1 * Math.sin(mark.phase * Math.PI * 2);
      if (mark.sensorReach > mark.radius * 1.3) {
        context.setLineDash([Math.max(2, scene.dpr * 2), Math.max(4, scene.dpr * 5)]);
        context.strokeStyle = alphaText(mark.tone, mark.alpha * 0.26);
        context.lineWidth = Math.max(0.55, scene.dpr * 0.65);
        context.beginPath();
        context.arc(mark.x, mark.y, mark.sensorReach, 0, Math.PI * 2);
        context.stroke();
      }
      if (mark.reach > mark.radius * 1.3) {
        context.setLineDash([
          Math.max(mark.preview ? 5 : 3, scene.dpr * (mark.preview ? 5 : 3.5)),
          Math.max(mark.preview ? 4 : 6, scene.dpr * (mark.preview ? 4 : 7)),
        ]);
        context.strokeStyle = alphaText(mark.tone, mark.alpha * 0.18);
        context.lineWidth = Math.max(0.65, scene.dpr * 0.7);
        context.beginPath();
        context.arc(mark.x, mark.y, mark.reach, 0, Math.PI * 2);
        context.stroke();
      }

      if (mark.hasTarget) {
        const dx = mark.targetX - mark.x;
        const dy = mark.targetY - mark.y;
        const length = Math.max(1, Math.hypot(dx, dy));
        const tx = dx / length;
        const ty = dy / length;
        const nx = -ty;
        const ny = tx;
        const startX = mark.x + tx * mark.radius;
        const startY = mark.y + ty * mark.radius;
        const endX = mark.targetX - tx * mark.targetRadius;
        const endY = mark.targetY - ty * mark.targetRadius;
        context.setLineDash([Math.max(4, scene.dpr * 4), Math.max(5, scene.dpr * 5)]);
        context.strokeStyle = alphaText(mark.targetTone, mark.alpha * 0.48);
        context.lineWidth = Math.max(0.8, scene.dpr);
        context.beginPath();
        context.moveTo(startX, startY);
        context.lineTo(endX, endY);
        context.stroke();
        context.setLineDash([]);

        const arrowX = startX + (endX - startX) * 0.68;
        const arrowY = startY + (endY - startY) * 0.68;
        const back = Math.max(4, scene.dpr * 5);
        const wing = Math.max(3, scene.dpr * 3.5);
        context.strokeStyle = alphaText(mark.targetTone, mark.alpha * 0.72);
        context.beginPath();
        context.moveTo(arrowX - tx * back + nx * wing, arrowY - ty * back + ny * wing);
        context.lineTo(arrowX, arrowY);
        context.lineTo(arrowX - tx * back - nx * wing, arrowY - ty * back - ny * wing);
        context.stroke();

        context.strokeStyle = alphaText(mark.targetTone, mark.alpha * 0.82);
        context.lineWidth = Math.max(1, scene.dpr * 1.15);
        context.beginPath();
        for (let corner = 0; corner < 4; corner += 1) {
          const angle = corner * Math.PI / 2 + mark.phase * 0.35;
          context.arc(
            mark.targetX,
            mark.targetY,
            mark.targetRadius * beat,
            angle - Math.PI * 0.16,
            angle + Math.PI * 0.16,
          );
        }
        context.stroke();
      }

      context.setLineDash([]);
      context.strokeStyle = alphaText(mark.tone, mark.alpha);
      context.lineWidth = Math.max(1.1, scene.dpr * 1.35);
      context.beginPath();
      const segments = policySegments(mark.action);
      for (let segment = 0; segment < segments; segment += 1) {
        const center = -Math.PI / 2 + (segment / segments) * Math.PI * 2 + mark.phase * 0.22;
        const half = segments === 1 ? Math.PI * 0.76 : (Math.PI / segments) * 0.52;
        context.arc(mark.x, mark.y, mark.radius * beat, center - half, center + half);
      }
      context.stroke();

      if (mark.action === 'emit_signal') {
        context.strokeStyle = alphaText(mark.tone, mark.alpha * (0.42 - mark.phase * 0.2));
        context.lineWidth = Math.max(0.7, scene.dpr * 0.8);
        context.beginPath();
        context.arc(mark.x, mark.y, mark.radius * (1.28 + mark.phase * 0.24), 0, Math.PI * 2);
        context.stroke();
      }
      if (
        mark.outcome === 'no_target'
        || mark.outcome === 'target_unavailable'
        || mark.outcome === 'wrong_layer'
        || mark.outcome === 'out_of_range'
        || mark.outcome === 'capacity_reached'
        || mark.action === 'couple' && mark.outcome === 'no_effect'
        || mark.outcome === 'unavailable'
      ) {
        const cut = mark.radius * 0.42;
        context.strokeStyle = alphaText(mark.tone, mark.alpha * 0.72);
        context.lineWidth = Math.max(0.9, scene.dpr);
        context.beginPath();
        context.moveTo(mark.x - cut, mark.y - cut);
        context.lineTo(mark.x + cut, mark.y + cut);
        context.moveTo(mark.x + cut, mark.y - cut);
        context.lineTo(mark.x - cut, mark.y + cut);
        context.stroke();
      }
    }
    context.setLineDash([]);
    context.globalCompositeOperation = 'source-over';
  }

  /** Accepted Design assembly changes, never a causal Field transition. */
  private assemblyDraft(scene: Scene): void {
    const context = this.context;
    context.save();
    context.globalCompositeOperation = 'lighter';
    context.lineCap = 'round';
    context.lineJoin = 'round';
    const transition = scene.engineeringTransition;
    if (transition.active && transition.operation === 'full_contract_reset') {
      const span = Math.min(scene.width, scene.height);
      const travel = transition.stage === 'reconstruct'
        ? (1 - transition.phase) * span * 0.08
        : transition.stage === 'settle'
          ? (1 - transition.phase) * span * 0.025
          : 0;
      const inset = span * 0.035 + travel;
      context.strokeStyle = alphaText(
        transition.authorityTone,
        transition.stage === 'preview' ? 0.34 : 0.62,
      );
      context.lineWidth = Math.max(1.2, scene.dpr * 1.45);
      context.strokeRect(inset, inset, Math.max(1, scene.width - inset * 2), Math.max(1, scene.height - inset * 2));
      const inner = inset + span * 0.012;
      context.strokeStyle = alphaText(
        transition.tone,
        transition.stage === 'preview' ? 0.2 : 0.38,
      );
      context.lineWidth = Math.max(0.8, scene.dpr);
      context.strokeRect(inner, inner, Math.max(1, scene.width - inner * 2), Math.max(1, scene.height - inner * 2));
    }
    for (let place = 0; place < scene.assemblyDrafts.count; place += 1) {
      const mark = scene.assemblyDrafts.items[place];
      const radius = mark.radius;
      const bracket = radius * 1.18;
      const corner = Math.max(scene.dpr * 3, radius * 0.28);
      if (mark.displaced) {
        context.setLineDash([Math.max(3, scene.dpr * 3), Math.max(5, scene.dpr * 5)]);
        context.strokeStyle = alphaText(mark.originTone, mark.alpha * 0.52);
        context.lineWidth = Math.max(0.8, scene.dpr * 0.9);
        context.beginPath();
        context.moveTo(mark.fromX, mark.fromY);
        context.lineTo(mark.x, mark.y);
        context.stroke();
        context.setLineDash([]);
        context.strokeStyle = alphaText(mark.originTone, mark.alpha * 0.7);
        context.lineWidth = Math.max(0.8, scene.dpr);
        context.beginPath();
        context.arc(mark.fromX, mark.fromY, mark.fromRadius, 0, Math.PI * 2);
        context.moveTo(mark.fromX - mark.fromRadius * 0.52, mark.fromY);
        context.lineTo(mark.fromX + mark.fromRadius * 0.52, mark.fromY);
        context.stroke();
      }

      context.strokeStyle = alphaText(mark.tone, mark.alpha);
      context.lineWidth = Math.max(1, scene.dpr * 1.25);
      context.beginPath();
      context.moveTo(mark.x - bracket, mark.y - bracket + corner);
      context.lineTo(mark.x - bracket, mark.y - bracket);
      context.lineTo(mark.x - bracket + corner, mark.y - bracket);
      context.moveTo(mark.x + bracket - corner, mark.y - bracket);
      context.lineTo(mark.x + bracket, mark.y - bracket);
      context.lineTo(mark.x + bracket, mark.y - bracket + corner);
      context.moveTo(mark.x + bracket, mark.y + bracket - corner);
      context.lineTo(mark.x + bracket, mark.y + bracket);
      context.lineTo(mark.x + bracket - corner, mark.y + bracket);
      context.moveTo(mark.x - bracket + corner, mark.y + bracket);
      context.lineTo(mark.x - bracket, mark.y + bracket);
      context.lineTo(mark.x - bracket, mark.y + bracket - corner);
      context.stroke();

      context.setLineDash([Math.max(4, scene.dpr * 4), Math.max(3, scene.dpr * 3)]);
      context.strokeStyle = alphaText(mark.tone, mark.alpha * 0.76);
      context.lineWidth = Math.max(0.8, scene.dpr);
      context.beginPath();
      context.arc(mark.x, mark.y, radius, 0, Math.PI * 2);
      context.stroke();
      context.setLineDash([]);

      if ((mark.fields & (ASSEMBLY_DRAFT_FIELD.charge | ASSEMBLY_DRAFT_FIELD.reserve | ASSEMBLY_DRAFT_FIELD.amount | ASSEMBLY_DRAFT_FIELD.leakage)) !== 0) {
        const beforeAngle = -Math.PI / 2 + Math.PI * 2 * mark.beforeLevel;
        const afterAngle = -Math.PI / 2 + Math.PI * 2 * mark.afterLevel;
        context.strokeStyle = alphaText(mark.originTone, mark.alpha * 0.72);
        context.lineWidth = Math.max(1, radius * 0.09);
        context.beginPath();
        context.arc(mark.x, mark.y, radius * 0.76, -Math.PI / 2, beforeAngle);
        context.stroke();
        context.strokeStyle = alphaText(mark.tone, mark.alpha);
        context.lineWidth = Math.max(1.2, radius * 0.13);
        context.beginPath();
        context.arc(mark.x, mark.y, radius * 0.9, -Math.PI / 2, afterAngle);
        context.stroke();
      }

      if ((mark.fields & ASSEMBLY_DRAFT_FIELD.interface) !== 0) {
        context.strokeStyle = alphaText(mark.tone, mark.alpha);
        context.lineWidth = Math.max(1.1, radius * 0.1);
        context.beginPath();
        if (mark.open) {
          context.moveTo(mark.x - radius * 0.62, mark.y);
          context.lineTo(mark.x - radius * 0.15, mark.y);
          context.moveTo(mark.x + radius * 0.15, mark.y);
          context.lineTo(mark.x + radius * 0.62, mark.y);
        } else {
          context.moveTo(mark.x - radius * 0.58, mark.y);
          context.lineTo(mark.x + radius * 0.58, mark.y);
          context.moveTo(mark.x, mark.y - radius * 0.42);
          context.lineTo(mark.x, mark.y + radius * 0.42);
        }
        context.stroke();
      }

      if ((mark.fields & ASSEMBLY_DRAFT_FIELD.blanks) !== 0) {
        const count = Math.max(1, Math.min(8, mark.count));
        context.strokeStyle = alphaText(mark.tone, mark.alpha * 0.9);
        context.lineWidth = Math.max(1, scene.dpr);
        for (let index = 0; index < count; index += 1) {
          const angle = -Math.PI / 2 + (index - (count - 1) / 2) * 0.3;
          context.beginPath();
          context.moveTo(mark.x + Math.cos(angle) * radius * 0.42, mark.y + Math.sin(angle) * radius * 0.42);
          context.lineTo(mark.x + Math.cos(angle) * radius * 0.68, mark.y + Math.sin(angle) * radius * 0.68);
          context.stroke();
        }
      }

      if (mark.kind === 'material') {
        context.strokeStyle = alphaText(mark.tone, mark.alpha * 0.92);
        context.lineWidth = Math.max(1, scene.dpr);
        context.beginPath();
        context.moveTo(mark.x, mark.y - radius * 0.48);
        context.lineTo(mark.x + radius * 0.48, mark.y);
        context.lineTo(mark.x, mark.y + radius * 0.48);
        context.lineTo(mark.x - radius * 0.48, mark.y);
        context.closePath();
        context.stroke();
        for (let index = 0; index < mark.count; index += 1) {
          const angle = index * 2.39996;
          const reach = radius * (0.12 + 0.055 * index);
          context.fillStyle = alphaText(mark.tone, mark.alpha * (0.86 - index * 0.06));
          context.beginPath();
          context.arc(
            mark.x + Math.cos(angle) * reach,
            mark.y + Math.sin(angle) * reach,
            Math.max(1, radius * 0.055),
            0,
            Math.PI * 2,
          );
          context.fill();
        }
      }

      if (mark.kind === 'current') {
        const angle = -Math.PI / 2 + Math.PI * 2 * mark.phase;
        context.strokeStyle = alphaText(mark.tone, mark.alpha);
        context.lineWidth = Math.max(1.1, radius * 0.1);
        context.beginPath();
        context.moveTo(mark.x, mark.y);
        context.lineTo(mark.x + Math.cos(angle) * radius * 0.62, mark.y + Math.sin(angle) * radius * 0.62);
        if (!mark.active) {
          context.moveTo(mark.x - radius * 0.46, mark.y + radius * 0.46);
          context.lineTo(mark.x + radius * 0.46, mark.y - radius * 0.46);
        }
        context.stroke();
      }

      if (mark.kind === 'route') {
        context.strokeStyle = alphaText(mark.tone, mark.alpha);
        context.lineWidth = Math.max(1.1, radius * 0.1);
        context.beginPath();
        context.moveTo(mark.x - radius * 0.58, mark.y);
        context.lineTo(mark.x + radius * 0.58, mark.y);
        context.moveTo(mark.x + radius * 0.18, mark.y - radius * 0.22);
        context.lineTo(mark.x + radius * 0.58, mark.y);
        context.lineTo(mark.x + radius * 0.18, mark.y + radius * 0.22);
        context.stroke();
      }

      if ((mark.fields & ASSEMBLY_DRAFT_FIELD.layer) !== 0) {
        context.strokeStyle = alphaText(mark.tone, mark.alpha * 0.9);
        context.lineWidth = Math.max(1, scene.dpr);
        context.beginPath();
        context.moveTo(mark.x + bracket * 1.16, mark.y - radius * 0.5);
        context.lineTo(mark.x + bracket * 1.16, mark.y + radius * 0.5);
        context.moveTo(mark.x + bracket * 1.3, mark.y - radius * 0.28);
        context.lineTo(mark.x + bracket * 1.3, mark.y + radius * 0.72);
        context.stroke();
      }

      if ((mark.fields & ASSEMBLY_DRAFT_FIELD.members) !== 0 && mark.kind === 'physical_compartment') {
        context.strokeStyle = alphaText(mark.tone, mark.alpha * 0.85);
        context.lineWidth = Math.max(1, scene.dpr);
        context.beginPath();
        context.moveTo(mark.x - radius * 0.45, mark.y - radius * 0.22);
        context.lineTo(mark.x + radius * 0.45, mark.y - radius * 0.22);
        context.moveTo(mark.x - radius * 0.45, mark.y + radius * 0.22);
        context.lineTo(mark.x + radius * 0.45, mark.y + radius * 0.22);
        context.stroke();
      }

      if (mark.companion && scene.engineeringTransition.active) {
        const transition = scene.engineeringTransition;
        const lockRadius = radius * (1.34 + (transition.stage === 'settle'
          ? (1 - transition.phase) * 0.42
          : 0));
        if (transition.stage === 'deenergize') {
          const contraction = radius * (1.82 - transition.phase * 0.72);
          context.strokeStyle = alphaText(transition.originTone, mark.alpha * (0.9 - transition.phase * 0.36));
          context.lineWidth = Math.max(1, scene.dpr * 1.2);
          context.beginPath();
          for (let segment = 0; segment < 8; segment += 1) {
            const angle = segment * Math.PI / 4;
            context.moveTo(
              mark.fromX + Math.cos(angle) * contraction,
              mark.fromY + Math.sin(angle) * contraction,
            );
            context.lineTo(
              mark.fromX + Math.cos(angle) * contraction * 0.72,
              mark.fromY + Math.sin(angle) * contraction * 0.72,
            );
          }
          context.stroke();
        } else if (transition.stage === 'reconstruct') {
          const sweep = Math.PI * 0.08 + Math.PI * 0.38 * transition.phase;
          context.strokeStyle = alphaText(transition.tone, mark.alpha * 0.9);
          context.lineWidth = Math.max(1.2, scene.dpr * 1.45);
          context.beginPath();
          for (let corner = 0; corner < 4; corner += 1) {
            const angle = corner * Math.PI / 2 - Math.PI / 4;
            context.arc(mark.x, mark.y, lockRadius, angle - sweep, angle + sweep);
          }
          context.stroke();
        } else if (
          transition.stage === 'settle'
          || transition.stage === 'locked'
          || transition.stage === 'preview'
        ) {
          const authorityAlpha = transition.stage === 'preview' ? 0.56 : 0.9;
          context.lineWidth = Math.max(1.2, scene.dpr * 1.5);
          context.beginPath();
          if (mark.compatibility === 'hard_refusal') {
            const cross = lockRadius * 0.7;
            context.strokeStyle = alphaText(OVERLOAD, mark.alpha * 0.92);
            context.moveTo(mark.x - cross, mark.y - cross);
            context.lineTo(mark.x + cross, mark.y + cross);
            context.moveTo(mark.x + cross, mark.y - cross);
            context.lineTo(mark.x - cross, mark.y + cross);
            for (let corner = 0; corner < 4; corner += 1) {
              const angle = corner * Math.PI / 2;
              context.arc(mark.x, mark.y, lockRadius, angle - Math.PI * 0.09, angle + Math.PI * 0.09);
            }
          } else if (mark.compatibility === 'adaptation_required') {
            context.setLineDash([
              Math.max(3, scene.dpr * 3),
              Math.max(4, scene.dpr * 4),
            ]);
            context.strokeStyle = alphaText(transition.tone, mark.alpha * authorityAlpha);
            context.arc(mark.x, mark.y, lockRadius, 0, Math.PI * 2);
          } else if (mark.compatibility === 'retained_unchanged') {
            context.strokeStyle = alphaText(
              transition.authorityTone,
              mark.alpha * authorityAlpha,
            );
            context.arc(mark.x, mark.y, lockRadius, 0, Math.PI * 2);
            context.moveTo(mark.x + lockRadius * 0.82, mark.y);
            context.arc(mark.x, mark.y, lockRadius * 0.82, 0, Math.PI * 2);
          } else {
            context.strokeStyle = alphaText(
              transition.authorityTone,
              mark.alpha * authorityAlpha,
            );
            for (let corner = 0; corner < 4; corner += 1) {
              const angle = corner * Math.PI / 2;
              context.arc(mark.x, mark.y, lockRadius, angle - Math.PI * 0.16, angle + Math.PI * 0.16);
            }
          }
          context.stroke();
          context.setLineDash([]);
          if (transition.stage !== 'preview' && mark.compatibility !== 'hard_refusal') {
            context.fillStyle = alphaText(transition.authorityTone, mark.alpha * 0.72);
            context.beginPath();
            context.arc(mark.x, mark.y, Math.max(scene.dpr * 1.25, radius * 0.08), 0, Math.PI * 2);
            context.fill();
          }
        }
      }
    }
    context.restore();
  }

  private coupling(scene: Scene): void {
    const field = scene.coupling;
    if (!field.active || field.radius <= 0) return;
    const context = this.context;
    context.globalCompositeOperation = 'lighter';
    context.lineCap = 'round';
    context.setLineDash([
      Math.max(5 * scene.dpr, field.radius * 0.045),
      Math.max(4 * scene.dpr, field.radius * 0.028),
    ]);
    context.strokeStyle = alphaText(field.tone, field.alpha * (field.connected ? 0.82 : 0.42));
    context.lineWidth = Math.max(1, scene.dpr * (field.connected ? 1.45 : 0.9));
    context.beginPath();
    context.arc(field.x, field.y, field.radius, 0, Math.PI * 2);
    context.stroke();
    context.setLineDash([]);

    const sweep = -Math.PI / 2 + field.phase * Math.PI * 2;
    context.strokeStyle = alphaText(field.tone, field.alpha);
    context.lineWidth = Math.max(1.5, scene.dpr * 2.1);
    context.beginPath();
    context.arc(field.x, field.y, field.radius, sweep, sweep + Math.PI * (0.24 + 0.28 * field.charge));
    context.stroke();
    const ticks = 20;
    context.strokeStyle = alphaText(field.tone, field.alpha * 0.48);
    context.lineWidth = Math.max(0.7, scene.dpr * 0.75);
    context.beginPath();
    for (let tick = 0; tick < ticks; tick += 1) {
      const angle = (tick / ticks) * Math.PI * 2;
      const reach = (tick % 5 === 0 ? 5 : 2) * scene.dpr;
      context.moveTo(field.x + Math.cos(angle) * (field.radius - reach), field.y + Math.sin(angle) * (field.radius - reach));
      context.lineTo(field.x + Math.cos(angle) * (field.radius + reach), field.y + Math.sin(angle) * (field.radius + reach));
    }
    context.stroke();

    for (let place = 0; place < scene.couplingTargets.count; place += 1) {
      const target = scene.couplingTargets.items[place];
      const dx = target.x - field.x;
      const dy = target.y - field.y;
      const length = Math.max(1, Math.hypot(dx, dy));
      const nx = -dy / length;
      const ny = dx / length;
      const bow = Math.min(22 * scene.dpr, length * 0.12) * (place % 2 === 0 ? 1 : -1);
      context.strokeStyle = alphaText(target.tone, target.alpha * 0.5);
      context.lineWidth = Math.max(0.8, scene.dpr);
      context.beginPath();
      context.moveTo(field.x, field.y);
      context.bezierCurveTo(
        field.x + dx * 0.33 + nx * bow,
        field.y + dy * 0.33 + ny * bow,
        field.x + dx * 0.67 + nx * bow,
        field.y + dy * 0.67 + ny * bow,
        target.x,
        target.y,
      );
      context.stroke();

      const lockBeat = scene.reducedMotion ? 1 : 0.9 + 0.1 * Math.sin(scene.clock * 0.42 + place);
      context.strokeStyle = alphaText(target.tone, target.alpha);
      context.lineWidth = Math.max(1.2, scene.dpr * 1.5);
      context.beginPath();
      for (let corner = 0; corner < 4; corner += 1) {
        const angle = corner * Math.PI / 2 + target.phase * Math.PI * 0.5;
        context.arc(target.x, target.y, target.radius * lockBeat, angle - Math.PI * 0.18, angle + Math.PI * 0.18);
      }
      context.stroke();

      if ((target.effects & COUPLING_EFFECT.gather) !== 0) {
        for (let bead = 0; bead < 3; bead += 1) {
          const along = 1 - ((field.phase * 1.8 + bead / 3) % 1);
          context.fillStyle = alphaText(CHARGE_SPARK, target.alpha * Math.sin(Math.PI * along));
          context.beginPath();
          context.arc(field.x + dx * along, field.y + dy * along, Math.max(1.1, scene.dpr * 1.45), 0, Math.PI * 2);
          context.fill();
        }
      }
      if ((target.effects & COUPLING_EFFECT.open) !== 0) {
        context.strokeStyle = alphaText(CURRENT_BRIGHT, target.alpha * 0.78);
        context.lineWidth = Math.max(1, scene.dpr);
        context.beginPath();
        context.arc(target.x, target.y, target.radius * 0.56, 0, Math.PI * 2);
        context.stroke();
      }
      if ((target.effects & COUPLING_EFFECT.suppress) !== 0) {
        const tx = dx / length;
        const ty = dy / length;
        context.strokeStyle = alphaText(target.tone, target.alpha * 0.85);
        context.lineWidth = Math.max(1, scene.dpr);
        context.beginPath();
        for (let push = 0; push < 2; push += 1) {
          const at = target.radius * (0.85 + push * 0.35);
          const px = target.x + tx * at;
          const py = target.y + ty * at;
          context.moveTo(px - tx * 5 * scene.dpr + nx * 4 * scene.dpr, py - ty * 5 * scene.dpr + ny * 4 * scene.dpr);
          context.lineTo(px, py);
          context.lineTo(px - tx * 5 * scene.dpr - nx * 4 * scene.dpr, py - ty * 5 * scene.dpr - ny * 4 * scene.dpr);
        }
        context.stroke();
      }
    }
    context.globalCompositeOperation = 'source-over';
  }

  private particles(scene: Scene): void {
    const context = this.context;
    context.globalCompositeOperation = 'lighter';
    for (let place = 0; place < scene.particles.count; place += 1) {
      const mark = scene.particles.items[place];
      context.fillStyle = alphaText(mark.tone, mark.alpha);
      context.beginPath();
      context.arc(mark.x, mark.y, mark.radius, 0, Math.PI * 2);
      context.fill();
      if (mark.subject === 1) {
        context.strokeStyle = alphaText(mark.tone, mark.alpha * 0.68);
        context.lineWidth = Math.max(0.6, scene.dpr * 0.72);
        context.beginPath();
        context.arc(mark.x, mark.y, mark.radius * 1.55, 0, Math.PI * 2);
        context.stroke();
        context.strokeStyle = alphaText(mark.tone, mark.alpha * 0.38);
        context.lineWidth = Math.max(0.5, scene.dpr * 0.52);
        context.beginPath();
        for (let spoke = 0; spoke < 6; spoke += 1) {
          const angle = (spoke / 6) * Math.PI * 2 + mark.id * 0.7;
          context.moveTo(mark.x + Math.cos(angle) * mark.radius * 0.65, mark.y + Math.sin(angle) * mark.radius * 0.65);
          context.lineTo(mark.x + Math.cos(angle) * mark.radius * 1.8, mark.y + Math.sin(angle) * mark.radius * 1.8);
        }
        context.stroke();
      } else if (mark.subject === 2) {
        for (let ring = 0; ring < 3; ring += 1) {
          context.strokeStyle = alphaText(mark.tone, mark.alpha * (0.6 - ring * 0.14));
          context.lineWidth = Math.max(0.5, scene.dpr * (0.75 - ring * 0.12));
          context.beginPath();
          context.arc(mark.x, mark.y, mark.radius * (1.2 + ring * 0.42), 0, Math.PI * 2);
          context.stroke();
        }
      }
    }
    context.globalCompositeOperation = 'source-over';
  }

  private cues(scene: Scene): void {
    const context = this.context;
    context.globalCompositeOperation = 'lighter';
    for (let place = 0; place < scene.cues.count; place += 1) {
      const mark = scene.cues.items[place];
      context.strokeStyle = alphaText(mark.tone, mark.alpha);
      context.lineWidth = mark.width;
      context.beginPath();
      context.arc(mark.x, mark.y, mark.radius, 0, Math.PI * 2);
      context.stroke();
    }
    context.globalCompositeOperation = 'source-over';
  }

  /**
   * The settling a falling time scale reads as. Nothing else on the surface
   * says the Field has stopped on purpose rather than stopped being drawn.
   */
  private settle(scene: Scene): void {
    if (scene.stillness <= 0.001) return;
    const context = this.context;
    context.fillStyle = alphaText(HAZE, 0.36 * scene.stillness);
    context.fillRect(0, 0, scene.width, scene.height);
  }

  /**
   * The handles a paused Field offers. One shape per kind, so what a handle
   * takes hold of reads without a word for it: a ring on a Port, a square at a
   * Route's end, a diamond on a Compartment vertex. Both engines draw the same
   * three shapes at the same three sizes.
   */
  private handles(scene: Scene): void {
    const context = this.context;
    for (let place = 0; place < scene.handles.count; place += 1) {
      const mark = scene.handles.items[place];
      context.strokeStyle = alphaText(mark.tone, mark.alpha);
      context.lineWidth = mark.width;
      context.beginPath();
      if (mark.kind === HANDLE_KIND.route) {
        context.rect(mark.x - mark.radius, mark.y - mark.radius, mark.radius * 2, mark.radius * 2);
      } else if (mark.kind === HANDLE_KIND.boundary) {
        context.moveTo(mark.x, mark.y - mark.radius);
        context.lineTo(mark.x + mark.radius, mark.y);
        context.lineTo(mark.x, mark.y + mark.radius);
        context.lineTo(mark.x - mark.radius, mark.y);
        context.closePath();
      } else {
        context.arc(mark.x, mark.y, mark.radius, 0, Math.PI * 2);
      }
      context.stroke();
    }
  }

  /**
   * The playback reading, as motion on the members themselves: one ring per
   * member mark, swelling and falling with the played step's factor, and a
   * second, fainter ring for the base series when the reading carries one —
   * drawn together, because the reading is their difference. No axis, no
   * strip, and no number: the same marks the other engine draws.
   */
  private playback(scene: Scene): void {
    const context = this.context;
    for (let place = 0; place < scene.playback.count; place += 1) {
      const mark = scene.playback.items[place];
      context.strokeStyle = alphaText(mark.tone, mark.alpha);
      context.lineWidth = Math.max(1, mark.radius * 0.2);
      context.beginPath();
      context.arc(mark.x, mark.y, playbackSwell(mark.radius, mark.factor), 0, Math.PI * 2);
      context.stroke();
      if (mark.base !== null) {
        context.strokeStyle = alphaText(mark.tone, mark.alpha * 0.45);
        context.lineWidth = Math.max(1, mark.radius * 0.1);
        context.beginPath();
        context.arc(mark.x, mark.y, playbackSwell(mark.radius, mark.base), 0, Math.PI * 2);
        context.stroke();
      }
    }
  }

  /**
   * The forecast overlay: the strip the standing View's forward reading is
   * drawn in, and the band inside it when one has been computed.
   */
  private forecast(scene: Scene): void {
    const mark = scene.forecast;
    if (mark.alpha <= 0.001) return;
    const context = this.context;

    // The strip itself: two rules and two brackets, so an overlay with nothing
    // in it still reads as a place a reading goes.
    context.strokeStyle = alphaText(mark.tone, mark.alpha * 0.34);
    context.lineWidth = Math.max(1, scene.dpr);
    context.beginPath();
    context.moveTo(mark.x, mark.y);
    context.lineTo(mark.x, mark.y + mark.height);
    context.lineTo(mark.x + mark.width, mark.y + mark.height);
    context.lineTo(mark.x + mark.width, mark.y);
    context.stroke();

    const pairs = mark.low.length / 2;
    if (pairs === 0) return;
    context.beginPath();
    context.moveTo(mark.low[0], mark.low[1]);
    for (let point = 1; point < pairs; point += 1) {
      context.lineTo(mark.low[point * 2], mark.low[point * 2 + 1]);
    }
    for (let point = pairs - 1; point >= 0; point -= 1) {
      context.lineTo(mark.high[point * 2], mark.high[point * 2 + 1]);
    }
    context.closePath();
    context.fillStyle = alphaText(mark.tone, mark.alpha * 0.22);
    context.fill();
    context.strokeStyle = alphaText(mark.tone, mark.alpha * 0.7);
    context.stroke();
  }

  private rim(scene: Scene): void {
    if (scene.rim.level <= 0) return;
    const context = this.context;
    const localWeight = scene.rim.localized ? 0.46 : 1;
    if (!scene.quality.pressureBloom) {
      const inset = Math.max(2, scene.dpr * 2);
      context.strokeStyle = alphaText(scene.rim.tone, scene.rim.level * 0.42 * localWeight);
      context.lineWidth = Math.max(1, scene.dpr * (1 + scene.rim.level * 2));
      context.strokeRect(inset, inset, Math.max(1, scene.width - inset * 2), Math.max(1, scene.height - inset * 2));
      return;
    }
    const reach = Math.max(scene.width, scene.height);
    const inner = reach * (0.66 - RIM_REACH * scene.rim.level * (0.7 + 0.3 * scene.rim.beat));
    const wash = context.createRadialGradient(
      scene.width / 2,
      scene.height / 2,
      Math.max(0, inner),
      scene.width / 2,
      scene.height / 2,
      reach * 0.78,
    );
    const weight =
      scene.rim.level *
      (scene.rim.crisis ? 0.7 : 0.42) *
      (0.75 + 0.25 * scene.rim.beat) *
      localWeight;
    wash.addColorStop(0, alphaText(scene.rim.tone, 0));
    wash.addColorStop(1, alphaText(scene.rim.tone, weight));
    context.fillStyle = wash;
    context.fillRect(0, 0, scene.width, scene.height);
  }
}
