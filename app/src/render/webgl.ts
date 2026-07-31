/**
 * The PixiJS engine: the WebGL surface the game draws on.
 *
 * PixiJS is a bundled dependency, never a fetched one, and every texture the
 * engine uses is generated here at start — a soft radial fall-off and its
 * inverse — so the production bundle stays self-contained and asks the network
 * for nothing.
 *
 * The scene graph is built once and refilled in place: one `Graphics` per band
 * of stroked work, and a pool of sprites per band of soft work. A frame at the
 * render target therefore allocates nothing but the numbers it draws with.
 */

import { Container, Graphics, Sprite, Texture, WebGLRenderer } from 'pixi.js';
import {
  BACKDROP,
  BACKDROP_EDGE,
  BOUNDARY_SHELL,
  CHARGE_CORE,
  CURRENT_BRIGHT,
  HAZE,
  OVERLOAD,
} from './palette';
import { depthScale, formRingWidth, HANDLE_KIND, type Scene } from './scene';
import { dashOf, type Engine, playbackSwell } from './engine';

/** How wide the generated fall-off textures are, in texels. */
const SOFT_TEXTURE_SIZE = 128;

/** How far the pressure reading reaches in from the edge, at its widest. */
const RIM_REACH = 0.34;

/** Starts a PixiJS engine on the surface, or rejects with why it could not. */
export async function startWebglEngine(
  canvas: HTMLCanvasElement,
  width: number,
  height: number,
): Promise<Engine> {
  const renderer = new WebGLRenderer();
  await renderer.init({
    canvas,
    width: Math.max(1, Math.round(width)),
    height: Math.max(1, Math.round(height)),
    antialias: true,
    backgroundColor: BACKDROP,
    backgroundAlpha: 1,
    // The library's console greeting stays off: the game prints nothing of its
    // own accord, and the greeting is the one place the bundle would carry an
    // address at all.
    hello: false,
    // The surface is already sized in device pixels, so the renderer takes
    // them one to one and owns no density of its own.
    resolution: 1,
    autoDensity: false,
    clearBeforeRender: true,
    powerPreference: 'high-performance',
  });
  return new WebglEngine(renderer);
}

/** A soft radial fall-off, opaque in the middle and clear at the rim. */
function softTexture(): Texture {
  const middle = SOFT_TEXTURE_SIZE / 2;
  const canvas = document.createElement('canvas');
  canvas.width = SOFT_TEXTURE_SIZE;
  canvas.height = SOFT_TEXTURE_SIZE;
  const context = canvas.getContext('2d');
  if (!context) throw new Error('No 2D context to generate a texture with');
  const fade = context.createRadialGradient(middle, middle, 0, middle, middle, middle);
  fade.addColorStop(0, 'rgba(255,255,255,1)');
  fade.addColorStop(0.35, 'rgba(255,255,255,0.42)');
  fade.addColorStop(1, 'rgba(255,255,255,0)');
  context.fillStyle = fade;
  context.fillRect(0, 0, SOFT_TEXTURE_SIZE, SOFT_TEXTURE_SIZE);
  return Texture.from(canvas);
}

/** The same, inverted: clear in the middle and opaque at the rim. */
function rimTexture(): Texture {
  const middle = SOFT_TEXTURE_SIZE / 2;
  const canvas = document.createElement('canvas');
  canvas.width = SOFT_TEXTURE_SIZE;
  canvas.height = SOFT_TEXTURE_SIZE;
  const context = canvas.getContext('2d');
  if (!context) throw new Error('No 2D context to generate a texture with');
  const fade = context.createRadialGradient(middle, middle, 0, middle, middle, middle);
  fade.addColorStop(0, 'rgba(255,255,255,0)');
  fade.addColorStop(0.55, 'rgba(255,255,255,0)');
  fade.addColorStop(1, 'rgba(255,255,255,1)');
  context.fillStyle = fade;
  context.fillRect(0, 0, SOFT_TEXTURE_SIZE, SOFT_TEXTURE_SIZE);
  return Texture.from(canvas);
}

/** A reused band of sprites: the frame shows as many as it fills. */
class SpriteBand {
  readonly container = new Container();
  private used = 0;

  constructor(private readonly texture: Texture, additive: boolean) {
    this.container.blendMode = additive ? 'add' : 'normal';
  }

  open(): void {
    this.used = 0;
  }

  next(): Sprite {
    if (this.used === this.container.children.length) {
      const made = new Sprite(this.texture);
      made.anchor.set(0.5);
      this.container.addChild(made);
    }
    const sprite = this.container.children[this.used] as Sprite;
    sprite.visible = true;
    this.used += 1;
    return sprite;
  }

  close(): void {
    for (let place = this.used; place < this.container.children.length; place += 1) {
      this.container.children[place].visible = false;
    }
  }
}

/** Places one soft sprite: middle, reach in pixels, tone, and how strong. */
function placeSoft(sprite: Sprite, x: number, y: number, reach: number, tone: number, alpha: number): void {
  sprite.x = x;
  sprite.y = y;
  sprite.width = reach * 2;
  sprite.height = reach * 2;
  sprite.tint = tone;
  sprite.alpha = alpha < 0 ? 0 : alpha > 1 ? 1 : alpha;
}

/** Strokes a dashed line, which the graphics surface has no primitive for. */
function dashed(
  graphics: Graphics,
  x1: number,
  y1: number,
  x2: number,
  y2: number,
  dash: number,
  gap: number,
): void {
  const span = Math.hypot(x2 - x1, y2 - y1);
  if (span <= 0) return;
  const stepX = (x2 - x1) / span;
  const stepY = (y2 - y1) / span;
  for (let along = 0; along < span; along += dash + gap) {
    const end = Math.min(along + dash, span);
    graphics.moveTo(x1 + stepX * along, y1 + stepY * along);
    graphics.lineTo(x1 + stepX * end, y1 + stepY * end);
  }
}

/**
 * The engine itself, exported so a test can drive it without a GPU.
 *
 * `startWebglEngine` is how the renderer builds one and is the only way the
 * shell ever reaches it; what this export adds is a seam for a test to hand in
 * a renderer of its own and read what the engine drew. Nothing about the
 * engine changes: the scene graph is PixiJS's own, and its `Graphics` and
 * `Sprite` objects record geometry without a context behind them, so the draw
 * calls a scene produces can be compared against the other engine's without
 * either one being asked to rasterize anything.
 */
export class WebglEngine implements Engine {
  readonly kind = 'webgl' as const;

  private readonly stage = new Container();
  private readonly softFade = softTexture();
  private readonly rimFade = rimTexture();

  private readonly backdrop = new Graphics();
  private readonly deepHaze = new Graphics();
  private readonly currentWash: SpriteBand;
  private readonly bands = new Graphics();
  private readonly routes = new Graphics();
  private readonly boundary = new Graphics();
  private readonly bloom: SpriteBand;
  private readonly ports = new Graphics();
  private readonly trails = new Graphics();
  private readonly halos: SpriteBand;
  private readonly forms = new Graphics();
  private readonly particles: SpriteBand;
  private readonly nearHaze = new Graphics();
  private readonly settle = new Graphics();
  private readonly handles = new Graphics();
  private readonly playbackMarks = new Graphics();
  private readonly forecast = new Graphics();
  private readonly cues = new Graphics();
  private readonly rim: Sprite;

  constructor(private readonly renderer: WebGLRenderer) {
    this.currentWash = new SpriteBand(this.softFade, true);
    this.bloom = new SpriteBand(this.softFade, true);
    this.halos = new SpriteBand(this.softFade, true);
    this.particles = new SpriteBand(this.softFade, true);
    this.rim = new Sprite(this.rimFade);
    this.rim.anchor.set(0.5);
    this.rim.blendMode = 'add';

    this.bands.blendMode = 'add';
    this.trails.blendMode = 'add';
    this.cues.blendMode = 'add';

    this.stage.addChild(
      this.backdrop,
      this.deepHaze,
      this.currentWash.container,
      this.bands,
      this.routes,
      this.boundary,
      this.bloom.container,
      this.ports,
      this.trails,
      this.halos.container,
      this.forms,
      this.particles.container,
      this.nearHaze,
      this.settle,
      this.handles,
      this.playbackMarks,
      this.forecast,
      this.cues,
      this.rim,
    );
  }

  resize(width: number, height: number): void {
    this.renderer.resize(Math.max(1, Math.round(width)), Math.max(1, Math.round(height)));
  }

  draw(scene: Scene): void {
    this.drawBackdrop(scene);
    this.drawHaze(scene);
    this.drawCurrents(scene);
    this.drawRoutes(scene);
    this.drawBoundary(scene);
    this.drawPorts(scene);
    this.drawTrails(scene);
    this.drawForms(scene);
    this.drawParticles(scene);
    this.drawSettle(scene);
    this.drawHandles(scene);
    this.drawPlayback(scene);
    this.drawForecast(scene);
    this.drawCues(scene);
    this.drawRim(scene);
    this.renderer.render({ container: this.stage });
  }

  dispose(): void {
    this.stage.destroy({ children: true });
    this.softFade.destroy(true);
    this.rimFade.destroy(true);
    this.renderer.destroy();
  }

  private drawBackdrop(scene: Scene): void {
    this.backdrop
      .clear()
      .rect(0, 0, scene.width, scene.height)
      .fill({ color: BACKDROP_EDGE, alpha: 1 });
    // A soft lift in the middle, so the surface reads as depth rather than as
    // a flat sheet.
    const reach = Math.max(scene.width, scene.height) * 0.62;
    this.backdrop
      .circle(scene.width / 2, scene.height / 2, reach)
      .fill({ color: BACKDROP, alpha: 0.72 });
  }

  private drawHaze(scene: Scene): void {
    this.deepHaze.clear();
    this.nearHaze.clear();
    for (let place = 0; place < scene.hazes.count; place += 1) {
      const mark = scene.hazes.items[place];
      const target = mark.depth > 0 ? this.deepHaze : this.nearHaze;
      target
        .rect(0, 0, scene.width, scene.height)
        .fill({ color: mark.tone, alpha: mark.depth > 0 ? mark.alpha : mark.alpha * 0.5 });
    }
  }

  private drawCurrents(scene: Scene): void {
    this.currentWash.open();
    const bands = this.bands.clear();
    for (let place = 0; place < scene.currents.count; place += 1) {
      const mark = scene.currents.items[place];
      if (!mark.active) continue;
      // The phase counter drives visuals and nothing else, so it is what makes
      // an active current read as running rather than as standing — and it is a
      // beat, so reduced motion silences it along with every other one.
      const beat = scene.reducedMotion ? 1 : 0.85 + 0.15 * Math.sin(mark.phase * Math.PI * 2);
      // Scaled by the plane it stands on, so a current below reads as a
      // smaller pool further away rather than as a dimmer one here.
      const reach =
        Math.max(scene.width, scene.height) * (0.5 + 0.3 * mark.strength) * depthScale(mark.depth);
      placeSoft(
        this.currentWash.next(),
        scene.width / 2,
        scene.height / 2,
        reach,
        mark.tone,
        mark.alpha * (mark.bright ? 0.2 : 0.09) * (0.4 + 0.6 * mark.strength) * beat,
      );

      // The path the current runs, at the width a Node has to stand within: a
      // wide soft band, and a bright line down the middle of it.
      const pairs = mark.points.length / 2;
      if (pairs < 2) continue;
      for (const pass of [
        { width: mark.width, alpha: mark.alpha * (mark.bright ? 0.15 : 0.08) * beat },
        {
          width: Math.max(1, mark.width * 0.2),
          alpha: mark.alpha * (mark.bright ? 0.95 : 0.5) * beat,
        },
      ]) {
        bands.moveTo(mark.points[0], mark.points[1]);
        for (let point = 1; point < pairs; point += 1) {
          bands.lineTo(mark.points[point * 2], mark.points[point * 2 + 1]);
        }
        bands.stroke({ width: pass.width, color: mark.tone, alpha: pass.alpha, cap: 'round', join: 'round' });
      }
    }
    this.currentWash.close();
  }

  private drawRoutes(scene: Scene): void {
    const routes = this.routes.clear();
    for (let place = 0; place < scene.routes.count; place += 1) {
      const mark = scene.routes.items[place];
      // A queued change is drawn broken, whichever kind it is: what the queue
      // would do never reads as something the Field already holds.
      if (mark.preview) {
        dashed(routes, mark.x1, mark.y1, mark.x2, mark.y2, 6, 8);
      } else {
        routes.moveTo(mark.x1, mark.y1).lineTo(mark.x2, mark.y2);
      }
      routes.stroke({ width: mark.width, color: mark.tone, alpha: mark.alpha, cap: 'round' });
    }
  }

  private drawBoundary(scene: Scene): void {
    const boundary = this.boundary.clear();
    // The candidates first, so the standing boundary reads over them: they are
    // proposals about where a boundary could stand, and the one that does
    // stand is the one a player must be able to find at a glance.
    this.strokeHulls(boundary, scene.candidates);
    this.strokeHulls(boundary, scene.boundaries);
  }

  /** One pool of closed hulls, drawn in the register each mark declares. */
  private strokeHulls(boundary: Graphics, pool: Scene['boundaries']): void {
    for (let place = 0; place < pool.count; place += 1) {
      const mark = pool.items[place];
      const pairs = mark.points.length / 2;
      if (pairs === 0) continue;
      // A hull with no area — one member, two, or three in a line — is drawn
      // as a capsule of the boundary's own width, so the reading holds. Both
      // engines take the same marks and treat these two cases the same way.
      if (pairs <= 2) {
        if (pairs === 1) {
          boundary
            .circle(mark.points[0], mark.points[1], mark.width)
            .fill({ color: mark.tone, alpha: mark.alpha });
        } else {
          boundary.moveTo(mark.points[0], mark.points[1]);
          boundary.lineTo(mark.points[2], mark.points[3]);
          boundary.stroke({ width: mark.width * 2, color: mark.tone, alpha: mark.alpha, cap: 'round' });
        }
        continue;
      }
      // A proposed boundary and a candidate outline are outlines and nothing
      // more: no region under either, because no region has been declared —
      // the inside each draws is one a commit would adopt, and a commit that is
      // refused adopts none of it.
      if (!mark.proposed && mark.candidate === 0) {
        boundary.poly(mark.points, true).fill({ color: mark.tone, alpha: mark.alpha * 0.07 });
        // A wide, faint pass under the dashes, so the boundary reads as an edge
        // with a hue of its own rather than as one more Route.
        boundary.poly(mark.points, true);
        boundary.stroke({
          width: mark.width * 7,
          color: mark.tone,
          alpha: mark.alpha * 0.12,
          join: 'round',
        });
      }
      // One dash pattern per register, so the three readings are told apart
      // without a colour: the standing boundary, the change a queue proposes,
      // and a candidate of the slate.
      const [on, off] = dashOf(mark);
      for (let corner = 0; corner < pairs; corner += 1) {
        const next = (corner + 1) % pairs;
        dashed(
          boundary,
          mark.points[corner * 2],
          mark.points[corner * 2 + 1],
          mark.points[next * 2],
          mark.points[next * 2 + 1],
          on,
          off,
        );
      }
      boundary.stroke({ width: mark.width, color: mark.tone, alpha: mark.alpha, cap: 'round' });
    }
  }

  private drawPorts(scene: Scene): void {
    this.bloom.open();
    const ports = this.ports.clear();
    for (let place = 0; place < scene.ports.count; place += 1) {
      const mark = scene.ports.items[place];
      placeSoft(this.bloom.next(), mark.x, mark.y, mark.bloom, mark.tone, 0.5 * mark.charge * mark.alpha);

      if (mark.kind === 1) {
        ports.rect(mark.x - mark.radius, mark.y - mark.radius, mark.radius * 2, mark.radius * 2);
      } else if (mark.kind === 2) {
        ports.poly(
          [
            mark.x,
            mark.y - mark.radius * 1.2,
            mark.x + mark.radius * 1.2,
            mark.y,
            mark.x,
            mark.y + mark.radius * 1.2,
            mark.x - mark.radius * 1.2,
            mark.y,
          ],
          true,
        );
      } else {
        ports.circle(mark.x, mark.y, mark.radius);
      }
      ports.fill({ color: mark.tone, alpha: mark.alpha });

      if (mark.delivered > 0) {
        ports.circle(mark.x, mark.y, mark.radius * (1.6 + 1.4 * (1 - mark.delivered)));
        ports.stroke({
          width: Math.max(1, mark.radius * 0.3),
          color: CURRENT_BRIGHT,
          alpha: 0.85 * mark.delivered,
        });
      }
      if (mark.reserve > 0) {
        // The same sweep the other engine draws: an arc that opens with the
        // fraction held, rather than a ring that only changes brightness.
        ports.arc(
          mark.x,
          mark.y,
          mark.radius * 0.55,
          -Math.PI / 2,
          -Math.PI / 2 + Math.PI * 2 * mark.reserve,
        );
        ports.stroke({
          width: Math.max(1, mark.radius * 0.22),
          color: CHARGE_CORE,
          alpha: 0.6 * mark.alpha,
        });
      }
      if (mark.overloaded) {
        ports.circle(mark.x, mark.y, mark.radius * 1.35);
        ports.stroke({ width: Math.max(1, mark.radius * 0.35), color: OVERLOAD, alpha: 0.95 });
      }
      if (mark.shell) {
        ports.circle(mark.x, mark.y, mark.radius * 1.9);
        ports.stroke({ width: Math.max(1, mark.radius * 0.18), color: BOUNDARY_SHELL, alpha: 0.75 });
      }
    }
    this.bloom.close();
  }

  private drawTrails(scene: Scene): void {
    const trails = this.trails.clear();
    for (let place = 0; place < scene.trails.count; place += 1) {
      const mark = scene.trails.items[place];
      const pairs = mark.points.length / 2;
      if (pairs < 2) continue;
      for (let point = 1; point < pairs; point += 1) {
        const along = point / (pairs - 1);
        trails.moveTo(mark.points[(point - 1) * 2], mark.points[(point - 1) * 2 + 1]);
        trails.lineTo(mark.points[point * 2], mark.points[point * 2 + 1]);
        trails.stroke({
          width: Math.max(0.5, mark.width * along),
          color: mark.tone,
          alpha: mark.alpha * along * along,
          cap: 'round',
        });
      }
    }
  }

  private drawForms(scene: Scene): void {
    this.halos.open();
    const forms = this.forms.clear();
    for (let place = 0; place < scene.forms.count; place += 1) {
      const mark = scene.forms.items[place];
      const reach = mark.radius * (mark.controlled ? 4.2 : 2.8);
      placeSoft(
        this.halos.next(),
        mark.x,
        mark.y,
        reach,
        mark.ringTone,
        (mark.controlled ? 0.45 : 0.3) * mark.alpha,
      );

      const corners: number[] = [];
      const turn = mark.controlled ? mark.heading : 0;
      for (let corner = 0; corner < mark.sides; corner += 1) {
        const angle = turn + (corner / mark.sides) * Math.PI * 2;
        corners.push(mark.x + Math.cos(angle) * mark.radius, mark.y + Math.sin(angle) * mark.radius);
      }
      forms.poly(corners, true).fill({ color: mark.bodyTone, alpha: 0.9 * mark.alpha });
      forms.poly(corners, true);
      forms.stroke({ width: formRingWidth(mark), color: mark.ringTone, alpha: mark.alpha });

      forms
        .circle(mark.x, mark.y, mark.radius * (0.2 + 0.42 * mark.charge))
        .fill({ color: mark.tone, alpha: mark.alpha });

      if (mark.focus) {
        forms.circle(mark.x, mark.y, mark.radius * 1.55);
        forms.stroke({
          width: Math.max(1, mark.radius * 0.09),
          color: mark.ringTone,
          alpha: 0.6 * mark.alpha,
        });
      }
      if (mark.pulseCharging) {
        const beat = scene.reducedMotion ? 1 : 0.6 + 0.4 * Math.sin(scene.clock * 0.9);
        forms.circle(mark.x, mark.y, mark.radius * 0.85 * beat);
        forms.stroke({
          width: Math.max(1, mark.radius * 0.14),
          color: CHARGE_CORE,
          alpha: 0.75 * mark.alpha,
        });
      }
    }
    this.halos.close();
  }

  private drawParticles(scene: Scene): void {
    this.particles.open();
    for (let place = 0; place < scene.particles.count; place += 1) {
      const mark = scene.particles.items[place];
      placeSoft(this.particles.next(), mark.x, mark.y, mark.radius * 2.6, mark.tone, mark.alpha);
    }
    this.particles.close();
  }

  private drawCues(scene: Scene): void {
    const cues = this.cues.clear();
    for (let place = 0; place < scene.cues.count; place += 1) {
      const mark = scene.cues.items[place];
      cues.circle(mark.x, mark.y, mark.radius);
      cues.stroke({ width: mark.width, color: mark.tone, alpha: mark.alpha });
    }
  }

  /**
   * The settling a falling time scale reads as. Nothing else on the surface
   * says the Field has stopped on purpose rather than stopped being drawn.
   */
  private drawSettle(scene: Scene): void {
    this.settle.clear();
    if (scene.stillness <= 0.001) return;
    this.settle
      .rect(0, 0, scene.width, scene.height)
      .fill({ color: HAZE, alpha: 0.36 * scene.stillness });
  }

  /**
   * The handles a paused Field offers. One shape per kind — a ring on a Port,
   * a square at a Route's end, a diamond on a Boundary vertex — which is the
   * same three the other engine draws, at the same three sizes.
   */
  private drawHandles(scene: Scene): void {
    const handles = this.handles.clear();
    for (let place = 0; place < scene.handles.count; place += 1) {
      const mark = scene.handles.items[place];
      if (mark.kind === HANDLE_KIND.route) {
        handles.rect(
          mark.x - mark.radius,
          mark.y - mark.radius,
          mark.radius * 2,
          mark.radius * 2,
        );
      } else if (mark.kind === HANDLE_KIND.boundary) {
        handles.poly(
          [
            mark.x,
            mark.y - mark.radius,
            mark.x + mark.radius,
            mark.y,
            mark.x,
            mark.y + mark.radius,
            mark.x - mark.radius,
            mark.y,
          ],
          true,
        );
      } else {
        handles.circle(mark.x, mark.y, mark.radius);
      }
      handles.stroke({ width: mark.width, color: mark.tone, alpha: mark.alpha });
    }
  }

  /**
   * The playback reading, as motion on the members themselves: one ring per
   * member mark, swelling and falling with the played step's factor, and a
   * second, fainter ring for the base series when the reading carries one —
   * drawn together, because the reading is their difference. The same marks,
   * the same shared swell rule, and the same absences as the other engine: no
   * axis, no strip, no number.
   */
  private drawPlayback(scene: Scene): void {
    const marks = this.playbackMarks.clear();
    for (let place = 0; place < scene.playback.count; place += 1) {
      const mark = scene.playback.items[place];
      marks
        .circle(mark.x, mark.y, playbackSwell(mark.radius, mark.factor))
        .stroke({
          width: Math.max(1, mark.radius * 0.2),
          color: mark.tone,
          alpha: mark.alpha,
        });
      if (mark.base !== null) {
        marks
          .circle(mark.x, mark.y, playbackSwell(mark.radius, mark.base))
          .stroke({
            width: Math.max(1, mark.radius * 0.1),
            color: mark.tone,
            alpha: mark.alpha * 0.45,
          });
      }
    }
  }

  /**
   * The forecast overlay: the strip the standing View's forward reading is
   * drawn in, and the band inside it when one has been computed.
   */
  private drawForecast(scene: Scene): void {
    const forecast = this.forecast.clear();
    const mark = scene.forecast;
    if (mark.alpha <= 0.001) return;

    forecast.moveTo(mark.x, mark.y);
    forecast.lineTo(mark.x, mark.y + mark.height);
    forecast.lineTo(mark.x + mark.width, mark.y + mark.height);
    forecast.lineTo(mark.x + mark.width, mark.y);
    forecast.stroke({
      width: Math.max(1, scene.dpr),
      color: mark.tone,
      alpha: mark.alpha * 0.34,
    });

    const pairs = mark.low.length / 2;
    if (pairs === 0) return;
    const band: number[] = [];
    for (let point = 0; point < pairs; point += 1) {
      band.push(mark.low[point * 2], mark.low[point * 2 + 1]);
    }
    for (let point = pairs - 1; point >= 0; point -= 1) {
      band.push(mark.high[point * 2], mark.high[point * 2 + 1]);
    }
    forecast.poly(band, true).fill({ color: mark.tone, alpha: mark.alpha * 0.22 });
    forecast.poly(band, true);
    forecast.stroke({
      width: Math.max(1, scene.dpr),
      color: mark.tone,
      alpha: mark.alpha * 0.7,
    });
  }

  private drawRim(scene: Scene): void {
    if (scene.rim.level <= 0) {
      this.rim.visible = false;
      return;
    }
    this.rim.visible = true;
    const reach =
      Math.max(scene.width, scene.height) *
      (1.6 - RIM_REACH * scene.rim.level * (0.7 + 0.3 * scene.rim.beat));
    placeSoft(
      this.rim,
      scene.width / 2,
      scene.height / 2,
      reach / 2,
      scene.rim.tone,
      scene.rim.level * (scene.rim.crisis ? 0.85 : 0.5) * (0.75 + 0.25 * scene.rim.beat),
    );
  }
}
