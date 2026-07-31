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
  BOUNDARY_SHELL,
  CHARGE_CORE,
  CURRENT_BRIGHT,
  HAZE,
  OVERLOAD,
  toneText,
  type Tone,
} from './palette';
import { depthScale, formRingWidth, HANDLE_KIND, type Scene } from './scene';
import { dashOf, playbackSwell, type Engine } from './engine';

/** How far the pressure reading reaches in from the edge, at its widest. */
const RIM_REACH = 0.34;

function alphaText(tone: Tone, alpha: number): string {
  const held = alpha < 0 ? 0 : alpha > 1 ? 1 : alpha;
  return `rgba(${(tone >> 16) & 0xff},${(tone >> 8) & 0xff},${tone & 0xff},${held.toFixed(3)})`;
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
    this.currents(scene);
    this.routes(scene);
    this.boundary(scene);
    this.ports(scene);
    this.trails(scene);
    this.forms(scene);
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
      if (!mark.active) continue;
      // Scaled by the plane it stands on, so a current below reads as a
      // smaller pool further away rather than as a dimmer one here.
      const reach =
        Math.max(scene.width, scene.height) * (0.5 + 0.3 * mark.strength) * depthScale(mark.depth);
      const wash = context.createRadialGradient(
        scene.width / 2,
        scene.height / 2,
        0,
        scene.width / 2,
        scene.height / 2,
        reach,
      );
      // The phase counter drives visuals and nothing else, so it is what makes
      // an active current read as running rather than as standing — and it is a
      // beat, so reduced motion silences it along with every other one.
      const beat = scene.reducedMotion ? 1 : 0.85 + 0.15 * Math.sin(mark.phase * Math.PI * 2);
      const weight =
        mark.alpha * (mark.bright ? 0.16 : 0.07) * (0.4 + 0.6 * mark.strength) * beat;
      wash.addColorStop(0, alphaText(mark.tone, weight));
      wash.addColorStop(1, alphaText(mark.tone, 0));
      context.fillStyle = wash;
      context.fillRect(0, 0, scene.width, scene.height);

      // The path the current runs, at the width a Node has to stand within: a
      // wide soft band, and a bright line down the middle of it.
      const pairs = mark.points.length / 2;
      if (pairs < 2) continue;
      context.lineCap = 'round';
      context.lineJoin = 'round';
      for (const pass of [
        { width: mark.width, alpha: mark.alpha * (mark.bright ? 0.15 : 0.08) * beat },
        {
          width: Math.max(1, mark.width * 0.2),
          alpha: mark.alpha * (mark.bright ? 0.95 : 0.5) * beat,
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
      context.setLineDash(mark.preview ? [6, 8] : []);
      context.strokeStyle = alphaText(mark.tone, mark.alpha);
      context.lineWidth = mark.width;
      context.beginPath();
      context.moveTo(mark.x1, mark.y1);
      context.lineTo(mark.x2, mark.y2);
      context.stroke();
    }
    context.setLineDash([]);
  }

  private boundary(scene: Scene): void {
    // The candidates first, so the standing boundary reads over them: they are
    // proposals about where a boundary could stand, and the one that does
    // stand is the one a player must be able to find at a glance.
    this.strokeHulls(scene.candidates);
    this.strokeHulls(scene.boundaries);
  }

  /** One pool of closed hulls, drawn in the register each mark declares. */
  private strokeHulls(pool: Scene['boundaries']): void {
    const context = this.context;
    for (let place = 0; place < pool.count; place += 1) {
      const mark = pool.items[place];
      const pairs = mark.points.length / 2;
      if (pairs === 0) continue;
      // A hull with no area — one member, two, or three in a line — is drawn
      // as a capsule of the boundary's own width, so the reading holds. Both
      // engines take the same marks and treat these two cases the same way.
      if (pairs <= 2) {
        context.lineCap = 'round';
        context.strokeStyle = alphaText(mark.tone, mark.alpha);
        context.lineWidth = mark.width * 2;
        if (pairs === 1) {
          context.beginPath();
          context.arc(mark.points[0], mark.points[1], mark.width, 0, Math.PI * 2);
          context.fill();
          context.stroke();
        } else {
          context.beginPath();
          context.moveTo(mark.points[0], mark.points[1]);
          context.lineTo(mark.points[2], mark.points[3]);
          context.stroke();
        }
        continue;
      }
      context.beginPath();
      context.moveTo(mark.points[0], mark.points[1]);
      for (let point = 2; point < mark.points.length; point += 2) {
        context.lineTo(mark.points[point], mark.points[point + 1]);
      }
      context.closePath();
      // A proposed boundary and a candidate outline are outlines and nothing
      // more: no region under either, because no region has been declared —
      // the inside each draws is one a commit would adopt, and a commit that is
      // refused adopts none of it.
      if (!mark.proposed && mark.candidate === 0) {
        context.fillStyle = alphaText(mark.tone, mark.alpha * 0.07);
        context.fill();
        // A wide, faint pass under the dashes, so the boundary reads as an edge
        // with a hue of its own rather than as one more Route.
        context.strokeStyle = alphaText(mark.tone, mark.alpha * 0.12);
        context.lineWidth = mark.width * 7;
        context.lineJoin = 'round';
        context.stroke();
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
      context.globalCompositeOperation = 'lighter';
      const bloom = context.createRadialGradient(mark.x, mark.y, 0, mark.x, mark.y, mark.bloom);
      bloom.addColorStop(0, alphaText(mark.tone, 0.5 * mark.charge * mark.alpha));
      bloom.addColorStop(1, alphaText(mark.tone, 0));
      context.fillStyle = bloom;
      context.fillRect(mark.x - mark.bloom, mark.y - mark.bloom, mark.bloom * 2, mark.bloom * 2);

      if (mark.delivered > 0) {
        context.strokeStyle = alphaText(CURRENT_BRIGHT, 0.85 * mark.delivered);
        context.lineWidth = Math.max(1, mark.radius * 0.3);
        context.beginPath();
        context.arc(mark.x, mark.y, mark.radius * (1.6 + 1.4 * (1 - mark.delivered)), 0, Math.PI * 2);
        context.stroke();
      }

      context.globalCompositeOperation = 'source-over';
      context.fillStyle = alphaText(mark.tone, mark.alpha);
      this.nodeShape(mark.x, mark.y, mark.radius, mark.kind);
      context.fill();

      if (mark.reserve > 0) {
        context.strokeStyle = alphaText(CHARGE_CORE, 0.6 * mark.alpha);
        context.lineWidth = Math.max(1, mark.radius * 0.22);
        context.beginPath();
        context.arc(mark.x, mark.y, mark.radius * 0.55, -Math.PI / 2, -Math.PI / 2 + Math.PI * 2 * mark.reserve);
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
        context.strokeStyle = alphaText(BOUNDARY_SHELL, 0.75);
        context.lineWidth = Math.max(1, mark.radius * 0.18);
        context.beginPath();
        context.arc(mark.x, mark.y, mark.radius * 1.9, 0, Math.PI * 2);
        context.stroke();
      }
    }
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

      context.globalCompositeOperation = 'lighter';
      const reach = mark.radius * (mark.controlled ? 4.2 : 2.8);
      const halo = context.createRadialGradient(mark.x, mark.y, 0, mark.x, mark.y, reach);
      halo.addColorStop(0, alphaText(mark.ringTone, (mark.controlled ? 0.45 : 0.3) * mark.alpha));
      halo.addColorStop(1, alphaText(mark.ringTone, 0));
      context.fillStyle = halo;
      context.fillRect(mark.x - reach, mark.y - reach, reach * 2, reach * 2);
      context.globalCompositeOperation = 'source-over';

      context.beginPath();
      const turn = mark.controlled ? mark.heading : 0;
      for (let corner = 0; corner < mark.sides; corner += 1) {
        const angle = turn + (corner / mark.sides) * Math.PI * 2;
        const px = mark.x + Math.cos(angle) * mark.radius;
        const py = mark.y + Math.sin(angle) * mark.radius;
        if (corner === 0) context.moveTo(px, py);
        else context.lineTo(px, py);
      }
      context.closePath();
      context.fillStyle = alphaText(mark.bodyTone, 0.9 * mark.alpha);
      context.fill();
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

  private particles(scene: Scene): void {
    const context = this.context;
    context.globalCompositeOperation = 'lighter';
    for (let place = 0; place < scene.particles.count; place += 1) {
      const mark = scene.particles.items[place];
      context.fillStyle = alphaText(mark.tone, mark.alpha);
      context.beginPath();
      context.arc(mark.x, mark.y, mark.radius, 0, Math.PI * 2);
      context.fill();
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
   * Route's end, a diamond on a Boundary vertex. Both engines draw the same
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
    const weight = scene.rim.level * (scene.rim.crisis ? 0.7 : 0.42) * (0.75 + 0.25 * scene.rim.beat);
    wash.addColorStop(0, alphaText(scene.rim.tone, 0));
    wash.addColorStop(1, alphaText(scene.rim.tone, weight));
    context.fillStyle = wash;
    context.fillRect(0, 0, scene.width, scene.height);
  }
}
