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
  PHYSICAL_COMPARTMENT_SHELL,
  CHARGE_SPARK,
  CHARGE_CORE,
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
import { dashOf, type Engine, playbackSwell } from './engine';

/** How wide the generated fall-off textures are, in texels. */
const SOFT_TEXTURE_SIZE = 128;

/** How far the pressure reading reaches in from the edge, at its widest. */
const RIM_REACH = 0.34;

/** Stable presentation noise. It adds texture without adding hidden state. */
function hash01(value: number): number {
  const wave = Math.sin(value * 12.9898 + 78.233) * 43_758.5453;
  return wave - Math.floor(wave);
}

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

/** Appends one solid or preview Route path without allocating a frame closure. */
function routePath(graphics: Graphics, mark: RouteMark): void {
  if (mark.preview) dashed(graphics, mark.x1, mark.y1, mark.x2, mark.y2, 6, 8);
  else if (!mark.automationEnabled || mark.transferOutcome === 'closed') dashed(graphics, mark.x1, mark.y1, mark.x2, mark.y2, 1.5, 6);
  else if (mark.gated) dashed(graphics, mark.x1, mark.y1, mark.x2, mark.y2, 2, 7);
  else if (mark.transferOutcome === 'source_starved') dashed(graphics, mark.x1, mark.y1, mark.x2, mark.y2, 2, 9);
  else if (mark.transferOutcome === 'destination_headroom') dashed(graphics, mark.x1, mark.y1, mark.x2, mark.y2, 10, 4);
  else graphics.moveTo(mark.x1, mark.y1).lineTo(mark.x2, mark.y2);
}

/** Endpoint and midline bars identify the physical term limiting acceptance. */
function routeTransferConstraint(graphics: Graphics, mark: RouteMark): void {
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
  for (const offset of separation > 0 ? [-separation, separation] : [0]) {
    graphics.moveTo(x + tx * offset - nx * reach, y + ty * offset - ny * reach);
    graphics.lineTo(x + tx * offset + nx * reach, y + ty * offset + ny * reach);
  }
  graphics.stroke({
    width: Math.max(1, mark.width * 0.52),
    color: mark.tone,
    alpha: Math.max(0.62, mark.alpha),
    cap: 'round',
  });
}

/** Closed endpoint gates are drawn where a dormant Route meets its Port. */
function routeGates(graphics: Graphics, mark: RouteMark): void {
  const span = Math.hypot(mark.x2 - mark.x1, mark.y2 - mark.y1);
  if (span <= 0) return;
  const nx = -(mark.y2 - mark.y1) / span;
  const ny = (mark.x2 - mark.x1) / span;
  const reach = Math.max(4, mark.width * 2.4);
  if (!mark.tailOpen) {
    graphics.moveTo(mark.x1 - nx * reach, mark.y1 - ny * reach);
    graphics.lineTo(mark.x1 + nx * reach, mark.y1 + ny * reach);
  }
  if (!mark.headOpen) {
    graphics.moveTo(mark.x2 - nx * reach, mark.y2 - ny * reach);
    graphics.lineTo(mark.x2 + nx * reach, mark.y2 + ny * reach);
  }
  graphics.stroke({
    width: Math.max(1, mark.width * 0.72),
    color: CURRENT_BRIGHT,
    alpha: Math.max(0.32, mark.alpha),
    cap: 'round',
  });
}

/** A local actuator collar distinguishes disabled and throttled Routes. */
function routeAutomationControl(graphics: Graphics, mark: RouteMark): void {
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
  for (const offset of bars) {
    const x = mark.x1 + tx * (center + offset);
    const y = mark.y1 + ty * (center + offset);
    graphics.moveTo(x - nx * reach, y - ny * reach);
    graphics.lineTo(x + nx * reach, y + ny * reach);
  }
  graphics.stroke({
    width: Math.max(1, mark.width * 0.58),
    color: mark.automationEnabled ? ROUTE_AUTOMATION : ROUTE_AUTOMATION_DISABLED,
    alpha: Math.max(0.5, mark.alpha),
    cap: 'round',
  });
}

/** A permanent directional notch makes an operational Route read tail to head. */
function routeDirection(graphics: Graphics, mark: RouteMark): void {
  const span = Math.hypot(mark.x2 - mark.x1, mark.y2 - mark.y1);
  if (span <= 0) return;
  const tx = (mark.x2 - mark.x1) / span;
  const ty = (mark.y2 - mark.y1) / span;
  const nx = -ty;
  const ny = tx;
  const x = mark.x1 + (mark.x2 - mark.x1) * 0.62;
  const y = mark.y1 + (mark.y2 - mark.y1) * 0.62;
  const reach = Math.max(3.5, mark.width * 1.65);
  const back = Math.max(4.5, mark.width * 2.2);
  graphics.moveTo(x - tx * back + nx * reach, y - ty * back + ny * reach);
  graphics.lineTo(x, y);
  graphics.lineTo(x - tx * back - nx * reach, y - ty * back - ny * reach);
  graphics.stroke({
    width: Math.max(0.8, mark.width * 0.42),
    color: CHARGE_SPARK,
    alpha: mark.alpha * (0.32 + mark.flow * 0.48),
    cap: 'round',
    join: 'round',
  });
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

/** Two transverse bars: the low-cost fallback shared by Clamp and Delay. */
function crossBars(
  graphics: Graphics,
  x1: number,
  y1: number,
  x2: number,
  y2: number,
  width: number,
  tone: number,
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
  for (const shift of [-gap, gap]) {
    const x = middleX + tx * shift;
    const y = middleY + ty * shift;
    graphics.moveTo(x - nx * reach, y - ny * reach).lineTo(x + nx * reach, y + ny * reach);
  }
  graphics.stroke({ width: Math.max(1.5, width * 0.34), color: tone, alpha: 0.96 * alpha, cap: 'round' });
}

/** Strokes a dashed circle for a one-member hull's non-solid register. */
function dashedCircle(
  graphics: Graphics,
  x: number,
  y: number,
  radius: number,
  dash: number,
  gap: number,
): void {
  const circumference = Math.PI * 2 * radius;
  if (circumference <= 0) return;
  for (let along = 0; along < circumference; along += dash + gap) {
    const end = Math.min(along + dash, circumference);
    const startTurn = along / radius;
    const endTurn = end / radius;
    graphics.moveTo(x + Math.cos(startTurn) * radius, y + Math.sin(startTurn) * radius);
    graphics.arc(x, y, radius, startTurn, endTurn);
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
  private readonly livingMatrix = new Graphics();
  private readonly deepHaze = new Graphics();
  private readonly medium = new Graphics();
  private readonly currentWash: SpriteBand;
  private readonly bands = new Graphics();
  private readonly supplyLinks = new Graphics();
  private readonly routes = new Graphics();
  private readonly boundary = new Graphics();
  private readonly bloom: SpriteBand;
  private readonly ports = new Graphics();
  private readonly trails = new Graphics();
  private readonly halos: SpriteBand;
  private readonly forms = new Graphics();
  private readonly policies = new Graphics();
  private readonly assemblyDraft = new Graphics();
  private readonly coupling = new Graphics();
  private readonly particles: SpriteBand;
  private readonly materialGeometry = new Graphics();
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
    this.supplyLinks.blendMode = 'add';
    this.trails.blendMode = 'add';
    this.policies.blendMode = 'add';
    this.assemblyDraft.blendMode = 'add';
    this.coupling.blendMode = 'add';
    this.cues.blendMode = 'add';

    this.stage.addChild(
      this.backdrop,
      this.livingMatrix,
      this.deepHaze,
      this.medium,
      this.currentWash.container,
      this.bands,
      this.supplyLinks,
      this.routes,
      this.boundary,
      this.bloom.container,
      this.ports,
      this.trails,
      this.halos.container,
      this.forms,
      this.policies,
      this.assemblyDraft,
      this.coupling,
      this.particles.container,
      this.materialGeometry,
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
    this.drawLivingMatrix(scene);
    this.drawHaze(scene);
    this.drawMedium(scene);
    this.drawCurrents(scene);
    this.drawSupplyLinks(scene);
    this.drawRoutes(scene);
    this.drawBoundary(scene);
    this.drawPorts(scene);
    this.drawTrails(scene);
    this.drawForms(scene);
    this.drawPolicies(scene);
    this.drawAssemblyDraft(scene);
    this.drawCoupling(scene);
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
    if (scene.quality.backdropEtching) {
      const gap = Math.max(62, Math.min(scene.width, scene.height) * 0.085);
      for (let x = -scene.height; x < scene.width + scene.height; x += gap) {
        this.backdrop
          .moveTo(x, 0)
          .lineTo(x + scene.height * 0.42, scene.height)
          .stroke({ width: Math.max(1, scene.dpr), color: GRAPHITE_GLOW, alpha: 0.1 });
      }
    }
    // A soft lift in the middle, so the surface reads as depth rather than as
    // a flat sheet.
    const reach = Math.max(scene.width, scene.height) * 0.62;
    this.backdrop
      .circle(scene.width / 2, scene.height / 2, reach)
      .fill({ color: BACKDROP, alpha: 0.72 });
  }

  private drawLivingMatrix(scene: Scene): void {
    const matrix = this.livingMatrix.clear();
    if (!scene.quality.softMarks) return;
    const pulseClock = scene.reducedMotion ? 0 : scene.clock * 0.012;

    for (let index = 0; index < 92; index += 1) {
      const x = hash01(index * 2.17 + 4) * scene.width;
      const y = hash01(index * 5.31 + 9) * scene.height;
      const pulse = 0.55 + 0.45 * Math.sin(pulseClock + index * 1.73);
      matrix.circle(x, y, Math.max(0.45, scene.dpr * (0.35 + hash01(index * 3) * 0.75)));
      matrix.fill({
        color: index % 7 === 0 ? CHARGE_SPARK : CURRENT_BRIGHT,
        alpha: (0.025 + hash01(index + 13) * 0.08) * pulse,
      });
    }

    for (let branch = 0; branch < 13; branch += 1) {
      const startX = hash01(branch * 7.1) * scene.width;
      const startY = hash01(branch * 3.9 + 2) * scene.height;
      const span = scene.width * (0.12 + hash01(branch + 22) * 0.18);
      const liftY = (hash01(branch + 31) - 0.5) * scene.height * 0.22;
      matrix.moveTo(startX, startY);
      for (let sample = 1; sample <= 18; sample += 1) {
        const along = sample / 18;
        const bend = Math.sin(along * Math.PI) * liftY;
        matrix.lineTo(startX + span * along, startY + bend * (1 - along * 0.38));
      }
      matrix.stroke({
        width: Math.max(0.45, scene.dpr * (branch % 4 === 0 ? 0.55 : 0.38)),
        color: branch % 4 === 0 ? CHARGE_CORE : GRAPHITE_GLOW,
        alpha: branch % 4 === 0 ? 0.055 : 0.095,
        cap: 'round',
      });
    }

    for (const side of [-1, 1]) {
      for (let layer = 0; layer < 4; layer += 1) {
        const inset = (10 + layer * 17) * scene.dpr;
        for (let step = 0; step <= 28; step += 1) {
          const y = (step / 28) * scene.height;
          const ripple = Math.sin(step * 0.62 + layer * 0.8 + pulseClock) * (7 + layer * 2) * scene.dpr;
          const x = side < 0 ? inset + ripple : scene.width - inset - ripple;
          if (step === 0) matrix.moveTo(x, y);
          else matrix.lineTo(x, y);
        }
        matrix.stroke({
          width: Math.max(0.6, scene.dpr * (1.45 - layer * 0.2)),
          color: side < 0 ? PHYSICAL_COMPARTMENT_SHELL : CURRENT_BRIGHT,
          alpha: 0.035 + layer * 0.012,
          cap: 'round',
        });
      }
    }
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

  private drawMedium(scene: Scene): void {
    const graphics = this.medium.clear();
    const mark = scene.medium;
    if (!mark.active) return;
    const gap = scene.quality.backdropEtching ? 74 : 104;
    const length = 20 + Math.min(42, mark.speed * 2.5);
    if (mark.collisionRadius > 0) {
      for (let place = 0; place < scene.ports.count; place += 1) {
        const port = scene.ports.items[place];
        if (port.kind === 3) continue;
        graphics
          .circle(port.x, port.y, mark.collisionRadius * scene.camera.zoom * depthScale(port.depth))
          .stroke({ width: Math.max(1, scene.dpr), color: mark.tone, alpha: mark.alpha * 1.2 });
      }
    }
    for (let y = -gap; y < scene.height + gap; y += gap) {
      for (let x = -gap; x < scene.width + gap; x += gap) {
        const shift = (mark.offset + ((x / gap + y / gap) % 3) * gap / 3) % gap;
        const sx = x + mark.dx * shift;
        const sy = y + mark.dy * shift;
        graphics
          .moveTo(sx, sy)
          .lineTo(sx + mark.dx * length, sy + mark.dy * length)
          .stroke({ width: Math.max(1, scene.dpr * (0.65 + mark.drag)), color: mark.tone, alpha: mark.alpha });
      }
    }
  }

  private drawCurrents(scene: Scene): void {
    this.currentWash.open();
    const bands = this.bands.clear();
    for (let place = 0; place < scene.currents.count; place += 1) {
      const mark = scene.currents.items[place];
      if (!mark.active && !mark.delayed) continue;
      // Phase now drives physical cadence as well as this restrained beat. The
      // frame's `emitting` flag supplies the causal on/off reading; reduced
      // motion only silences the decorative interpolation.
      const beat = scene.reducedMotion ? 1 : 0.85 + 0.15 * Math.sin(mark.phase * Math.PI * 2);
      // Scaled by the plane it stands on, so a current below reads as a
      // smaller pool further away rather than as a dimmer one here.
      const reach =
        Math.max(scene.width, scene.height) * (0.5 + 0.3 * mark.strength) * depthScale(mark.depth);
      if (scene.quality.currentWash && mark.active) {
        placeSoft(
          this.currentWash.next(),
          scene.width / 2,
          scene.height / 2,
          reach,
          mark.tone,
          mark.alpha * (mark.bright ? 0.1 : 0.045) * (0.4 + 0.6 * mark.strength) * beat,
        );
      }

      // The path the current runs, at the width a Node has to stand within: a
      // wide soft band, and a bright line down the middle of it.
      const pairs = mark.points.length / 2;
      if (pairs < 2) continue;
      for (const pass of [
        { width: mark.width, alpha: mark.alpha * (mark.bright ? 0.11 : 0.065) * beat },
        {
          width: Math.max(1, mark.width * 0.1),
          alpha: mark.alpha * (mark.bright ? 0.62 : 0.4) * beat,
        },
      ]) {
        bands.moveTo(mark.points[0], mark.points[1]);
        for (let point = 1; point < pairs; point += 1) {
          bands.lineTo(mark.points[point * 2], mark.points[point * 2 + 1]);
        }
        bands.stroke({ width: pass.width, color: mark.tone, alpha: pass.alpha, cap: 'round', join: 'round' });
      }

      const fibers = scene.quality.id === 'high' ? 7 : 4;
      for (let fiber = 0; fiber < fibers; fiber += 1) {
        const centered = fiber - (fibers - 1) / 2;
        for (let point = 1; point < pairs; point += 1) {
          const x1 = mark.points[(point - 1) * 2];
          const y1 = mark.points[(point - 1) * 2 + 1];
          const x2 = mark.points[point * 2];
          const y2 = mark.points[point * 2 + 1];
          const length = Math.max(1, Math.hypot(x2 - x1, y2 - y1));
          const nx = -(y2 - y1) / length;
          const ny = (x2 - x1) / length;
          for (let sample = 0; sample <= 12; sample += 1) {
            const along = sample / 12;
            const wave = Math.sin(along * Math.PI * 3 + fiber * 1.27 + mark.phase * Math.PI * 2);
            const offset = centered * mark.width * 0.11 + wave * mark.width * 0.045;
            const x = x1 + (x2 - x1) * along + nx * offset;
            const y = y1 + (y2 - y1) * along + ny * offset;
            if (sample === 0) bands.moveTo(x, y);
            else bands.lineTo(x, y);
          }
          bands.stroke({
            width: Math.max(0.55, scene.dpr * (fiber % 3 === 0 ? 0.75 : 0.45)),
            color: fiber % 3 === 0 && mark.bright ? CHARGE_SPARK : mark.tone,
            alpha: mark.alpha * (0.08 + (fiber % 2) * 0.055) * beat,
            cap: 'round',
            join: 'round',
          });
        }
      }
      if (mark.stress > 0) {
        const stressBeat =
          mark.stressCrisis && !scene.reducedMotion ? 0.78 + 0.22 * Math.sin(scene.clock * 0.55) : 1;
        bands.moveTo(mark.points[0], mark.points[1]);
        for (let point = 1; point < pairs; point += 1) {
          bands.lineTo(mark.points[point * 2], mark.points[point * 2 + 1]);
        }
        bands.stroke({
          width: Math.max(1, mark.width * 0.1),
          color: mark.stressTone,
          alpha: mark.stress * 0.48 * stressBeat,
          cap: 'round',
          join: 'round',
        });
      }
      if (mark.diverted) {
        for (let point = 1; point < pairs; point += 1) {
          dashed(
            bands,
            mark.points[(point - 1) * 2],
            mark.points[(point - 1) * 2 + 1],
            mark.points[point * 2],
            mark.points[point * 2 + 1],
            Math.max(4, mark.width * 0.22),
            Math.max(5, mark.width * 0.3),
          );
        }
        bands.stroke({
          width: Math.max(1.5, mark.width * 0.11),
          color: INTERVENTION_DECOY,
          alpha: 0.9 * mark.alpha,
          cap: 'round',
        });
      }
      if (mark.delayed) {
        crossBars(
          bands,
          mark.points[0],
          mark.points[1],
          mark.points[mark.points.length - 2],
          mark.points[mark.points.length - 1],
          mark.width,
          INTERVENTION_DELAY,
          mark.alpha,
        );
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
      routePath(routes, mark);
      routes.stroke({
        width: mark.width + Math.max(2, scene.dpr * 2.2),
        color: ROUTE_SEPARATION,
        alpha: 0.82 * mark.alpha,
        cap: 'round',
      });
      if (mark.stress > 0) {
        const stressBeat =
          mark.stressCrisis && !scene.reducedMotion ? 0.78 + 0.22 * Math.sin(scene.clock * 0.55) : 1;
        routePath(routes, mark);
        routes.stroke({
          width: mark.width * (2.4 + mark.stress),
          color: mark.stressTone,
          alpha: mark.stress * 0.34 * stressBeat,
          cap: 'round',
        });
      }
      if (scene.quality.routeGlow && !mark.preview && mark.flow > 0) {
        routes.moveTo(mark.x1, mark.y1).lineTo(mark.x2, mark.y2);
        routes.stroke({
          width: mark.width * 3.6,
          color: CHARGE_SPARK,
          alpha: mark.alpha * 0.12 * mark.flow,
          cap: 'round',
        });
      }
      routePath(routes, mark);
      routes.stroke({ width: mark.width, color: mark.tone, alpha: mark.alpha, cap: 'round' });
      if (mark.gated && !mark.preview) routeGates(routes, mark);
      else if (!mark.preview && mark.automationEnabled) routeDirection(routes, mark);
      routeAutomationControl(routes, mark);
      routeTransferConstraint(routes, mark);
      if (!mark.preview && mark.flow > 0.02) {
        const dx = mark.x2 - mark.x1;
        const dy = mark.y2 - mark.y1;
        const span = Math.max(1, Math.hypot(dx, dy));
        const nx = -dy / span;
        const ny = dx / span;
        const count = Math.max(3, Math.min(14, Math.floor(span / 42)));
        const drift = scene.reducedMotion ? 0 : (scene.clock * (0.018 + mark.flow * 0.025)) % 1;
        for (let tick = 0; tick < count; tick += 1) {
          const along = (tick / count + drift) % 1;
          const x = mark.x1 + dx * along;
          const y = mark.y1 + dy * along;
          const reach = Math.max(2.5, mark.width * (0.85 + mark.flow));
          routes.moveTo(x - nx * reach, y - ny * reach).lineTo(x + nx * reach, y + ny * reach);
        }
        routes.stroke({
          width: Math.max(0.7, scene.dpr * 0.7),
          color: CHARGE_SPARK,
          alpha: mark.alpha * (0.3 + mark.flow * 0.42),
          cap: 'round',
        });
      }
      if (mark.scrambled) {
        dashed(routes, mark.x1, mark.y1, mark.x2, mark.y2, 4, 5);
        routes.stroke({
          width: Math.max(1, mark.width * 0.42),
          color: INTERVENTION_SCRAMBLE,
          alpha: 0.92 * mark.alpha,
          cap: 'round',
        });
      }
      if (mark.clamped) {
        crossBars(
          routes,
          mark.x1,
          mark.y1,
          mark.x2,
          mark.y2,
          mark.width,
          INTERVENTION_CLAMP,
          mark.alpha,
        );
      }
    }
  }

  private drawSupplyLinks(scene: Scene): void {
    const links = this.supplyLinks.clear();
    for (let place = 0; place < scene.supplyLinks.count; place += 1) {
      const mark = scene.supplyLinks.items[place];
      const dx = mark.x2 - mark.x1;
      const dy = mark.y2 - mark.y1;
      const length = Math.max(1, Math.hypot(dx, dy));
      const nx = -dy / length;
      const ny = dx / length;
      for (let filament = -1; filament <= 1; filament += 1) {
        const bow = filament * Math.min(12 * scene.dpr, length * 0.18);
        for (let sample = 0; sample <= 12; sample += 1) {
          const along = sample / 12;
          const bend = Math.sin(along * Math.PI) * bow;
          const x = mark.x1 + dx * along + nx * bend;
          const y = mark.y1 + dy * along + ny * bend;
          if (sample === 0) links.moveTo(x, y);
          else links.lineTo(x, y);
        }
        links.stroke({
          width: Math.max(0.6, mark.width * (filament === 0 ? 1 : 0.58)),
          color: filament === 0 ? CHARGE_SPARK : mark.tone,
          alpha: mark.alpha * (filament === 0 ? 0.9 : 0.5),
          cap: 'round',
        });
      }
      for (let bead = 0; bead < 3; bead += 1) {
        const along = (mark.phase + bead / 3) % 1;
        links.circle(mark.x1 + dx * along, mark.y1 + dy * along, Math.max(1, mark.width * 1.4));
        links.fill({ color: CHARGE_SPARK, alpha: mark.alpha * Math.sin(Math.PI * along) });
      }
    }
  }

  private drawBoundary(scene: Scene): void {
    const boundary = this.boundary.clear();
    // Candidates first; the material compartment and active View then remain
    // legible over every possible observation aperture.
    this.strokeHulls(boundary, scene.candidates, scene);
    this.strokeHulls(boundary, scene.boundaries, scene);
    this.strokeHulls(boundary, scene.assemblyBoundaries, scene);
  }

  /** One pool of closed hulls, drawn in the register each mark declares. */
  private strokeHulls(boundary: Graphics, pool: Scene['boundaries'], scene: Scene): void {
    for (let place = 0; place < pool.count; place += 1) {
      const mark = pool.items[place];
      const pairs = mark.points.length / 2;
      if (pairs === 0) continue;
      // A hull with no area — one member, two, or three in a line — is drawn
      // as a capsule of the hull's own width, so the reading holds. Its dash
      // register still applies: a small View or proposal must not turn into the
      // same solid object as a small physical compartment.
      if (pairs <= 2) {
        const dash = dashOf(mark);
        if (pairs === 1) {
          if (mark.role === 'compartment' && !mark.proposed) {
            boundary
              .circle(mark.points[0], mark.points[1], mark.width)
              .fill({ color: mark.tone, alpha: mark.alpha * 0.07 });
          }
          if (dash.length === 0) {
            boundary.circle(mark.points[0], mark.points[1], mark.width);
          } else {
            dashedCircle(
              boundary,
              mark.points[0],
              mark.points[1],
              mark.width,
              dash[0],
              dash[1],
            );
          }
        } else {
          if (dash.length === 0) {
            boundary.moveTo(mark.points[0], mark.points[1]);
            boundary.lineTo(mark.points[2], mark.points[3]);
          } else {
            dashed(
              boundary,
              mark.points[0],
              mark.points[1],
              mark.points[2],
              mark.points[3],
              dash[0],
              dash[1],
            );
          }
        }
        boundary.stroke({ width: mark.width, color: mark.tone, alpha: mark.alpha, cap: 'round' });
        if (mark.stress > 0) {
          if (pairs === 1) boundary.circle(mark.points[0], mark.points[1], mark.width * 1.2);
          else boundary.moveTo(mark.points[0], mark.points[1]).lineTo(mark.points[2], mark.points[3]);
          boundary.stroke({
            width: mark.width * (1.5 + mark.stress),
            color: mark.stressTone,
            alpha: mark.stress * 0.5,
            cap: 'round',
          });
        }
        continue;
      }
      // Only the committed physical compartment owns a material interior.
      // The passive View and every candidate remain apertures/outlines: seeing
      // through them must never make them look like causal walls.
      if (mark.role === 'compartment' && !mark.proposed) {
        boundary.poly(mark.points, true).fill({ color: mark.tone, alpha: mark.alpha * 0.07 });
        // A broad low-alpha shoulder gives the compartment physical weight.
        boundary.poly(mark.points, true);
        boundary.stroke({
          width: mark.width * 2.4,
          color: mark.tone,
          alpha: mark.alpha * 0.16,
          join: 'round',
        });
        if (mark.stress > 0) {
          const stressBeat =
            mark.stressCrisis && !scene.reducedMotion ? 0.78 + 0.22 * Math.sin(scene.clock * 0.55) : 1;
          boundary.poly(mark.points, true);
          boundary.stroke({
            width: mark.width * (1.4 + 1.8 * mark.stress),
            color: mark.stressTone,
            alpha: mark.stress * 0.42 * stressBeat,
            join: 'round',
          });
        }
      }
      const dash = dashOf(mark);
      if (dash.length === 0) {
        boundary.poly(mark.points, true);
      } else {
        const [on, off] = dash;
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
      }
      boundary.stroke({ width: mark.width, color: mark.tone, alpha: mark.alpha, cap: 'round' });
    }
  }

  private drawPorts(scene: Scene): void {
    this.bloom.open();
    const ports = this.ports.clear();
    for (let place = 0; place < scene.ports.count; place += 1) {
      const mark = scene.ports.items[place];
      if (scene.quality.softMarks) {
        placeSoft(this.bloom.next(), mark.x, mark.y, mark.bloom, mark.tone, 0.5 * mark.charge * mark.alpha);
      }

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
      const dormant = mark.kind === 0 && !mark.open;
      ports.fill({
        color: dormant ? BACKDROP_EDGE : mark.tone,
        alpha: dormant ? 0.74 * mark.alpha : mark.alpha,
      });
      if (mark.kind === 1) ports.rect(mark.x - mark.radius, mark.y - mark.radius, mark.radius * 2, mark.radius * 2);
      else if (mark.kind === 2) {
        ports.poly([
          mark.x,
          mark.y - mark.radius * 1.2,
          mark.x + mark.radius * 1.2,
          mark.y,
          mark.x,
          mark.y + mark.radius * 1.2,
          mark.x - mark.radius * 1.2,
          mark.y,
        ], true);
      } else ports.circle(mark.x, mark.y, mark.radius);
      ports.stroke({ width: Math.max(1, mark.radius * 0.16), color: BACKDROP_EDGE, alpha: 0.5 * mark.alpha });

      if (dormant) {
        for (let gate = 0; gate < 3; gate += 1) {
          const angle = gate * Math.PI * 2 / 3 - Math.PI / 2;
          ports.arc(mark.x, mark.y, mark.radius * 1.18, angle + 0.16, angle + Math.PI * 0.48);
        }
        ports.stroke({
          width: Math.max(1, mark.radius * 0.18),
          color: mark.tone,
          alpha: 0.88 * mark.alpha,
          cap: 'round',
        });
      }

      ports.circle(mark.x, mark.y, mark.radius * 1.42);
      ports.stroke({
        width: Math.max(0.55, scene.dpr * 0.7),
        color: mark.tone,
        alpha: (0.22 + mark.charge * 0.34) * mark.alpha,
      });
      ports.circle(mark.x, mark.y, Math.max(1, mark.radius * (0.13 + mark.charge * 0.16)));
      ports.fill({
        color: dormant ? mark.tone : CHARGE_SPARK,
        alpha: (dormant ? 0.18 : 0.12 + mark.charge * 0.48) * mark.alpha,
      });

      if (mark.stress > 0) {
        const stressBeat =
          mark.stressCrisis && !scene.reducedMotion ? 0.78 + 0.22 * Math.sin(scene.clock * 0.55) : 1;
        ports.arc(mark.x, mark.y, mark.radius * 1.55, -Math.PI * 0.8, Math.PI * 0.7);
        ports.stroke({
          width: Math.max(1, mark.radius * 0.2),
          color: mark.stressTone,
          alpha: mark.stress * 0.72 * stressBeat,
        });
      }

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
      if (mark.reserveDelta !== 0) {
        const banking = mark.reserveDelta > 0;
        const strength = Math.min(1, Math.abs(mark.reserveDelta) * 10);
        ports.circle(mark.x, mark.y, mark.radius * (banking ? 0.78 : 1.08));
        ports.stroke({
          width: Math.max(1, mark.radius * (banking ? 0.14 : 0.24)),
          color: banking ? CURRENT_BRIGHT : CHARGE_SPARK,
          alpha: (0.38 + 0.5 * strength) * mark.alpha,
        });
      }
      if (mark.overloaded) {
        ports.circle(mark.x, mark.y, mark.radius * 1.35);
        ports.stroke({ width: Math.max(1, mark.radius * 0.35), color: OVERLOAD, alpha: 0.95 });
      }
      if (mark.shell) {
        ports.circle(mark.x, mark.y, mark.radius * 1.9);
        ports.stroke({
          width: Math.max(1, mark.radius * 0.18),
          color: PHYSICAL_COMPARTMENT_SHELL,
          alpha: 0.75,
        });
      }
      if (mark.decoyReceiver) {
        dashedCircle(ports, mark.x, mark.y, mark.radius * 2.35, 3, 4);
        ports.stroke({
          width: Math.max(1, mark.radius * 0.22),
          color: INTERVENTION_DECOY,
          alpha: 0.95,
        });
      }
      if (mark.breached) {
        ports.arc(mark.x, mark.y, mark.radius * 2.05, Math.PI * 0.08, Math.PI * 0.72);
        ports.stroke({
          width: Math.max(1.5, mark.radius * 0.28),
          color: INTERVENTION_BREACH,
          alpha: 0.96,
        });
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
      const pairs = mark.outline.length / 2;
      if (pairs < 3) continue;
      if (scene.quality.softMarks) {
        const reach = mark.radius * (mark.controlled ? 3.8 : 2.4);
        placeSoft(
          this.halos.next(),
          mark.x,
          mark.y,
          reach,
          mark.ringTone,
          (mark.controlled ? 0.32 : 0.18) * mark.alpha,
        );
      }

      if (mark.controlled || scene.quality.formFacets > 1) {
        const tx = Math.cos(mark.heading);
        const ty = Math.sin(mark.heading);
        const nx = -ty;
        const ny = tx;
        for (let filament = -2; filament <= 2; filament += 1) {
          const spread = filament * mark.radius * 0.17;
          const reach = mark.radius * (1.35 + Math.abs(filament) * 0.12 + mark.formOrdinal * 0.015);
          for (let sample = 0; sample <= 14; sample += 1) {
            const along = sample / 14;
            const wave = Math.sin(along * Math.PI * 1.35 + filament * 0.45) * spread * (1 - along * 0.45);
            const x = mark.x - tx * (mark.radius * 0.32 + reach * along) + nx * wave;
            const y = mark.y - ty * (mark.radius * 0.32 + reach * along) + ny * wave;
            if (sample === 0) forms.moveTo(x, y);
            else forms.lineTo(x, y);
          }
          forms.stroke({
            width: Math.max(0.55, mark.radius * 0.018),
            color: mark.ringTone,
            alpha: (0.14 + (2 - Math.abs(filament)) * 0.03) * mark.alpha,
            cap: 'round',
          });
        }
        for (let boundary = 0; boundary < 2; boundary += 1) {
          forms.arc(
            mark.x,
            mark.y,
            mark.radius * (1.34 + boundary * 0.22),
            mark.heading + boundary,
            mark.heading + Math.PI * (1.42 + boundary * 0.16),
          );
          forms.stroke({
            width: Math.max(0.55, scene.dpr * 0.65),
            color: mark.ringTone,
            alpha: (0.18 - boundary * 0.035) * mark.alpha,
            cap: 'round',
          });
        }
      }

      forms.poly(mark.outline, true).fill({ color: mark.bodyTone, alpha: 0.9 * mark.alpha });
      if (scene.quality.formFacets > 0) {
        const step = scene.quality.formFacets === 1 ? 2 : 1;
        for (let point = 0; point < pairs; point += step) {
          const next = (point + 1) % pairs;
          forms.poly(
            [
              mark.x,
              mark.y,
              mark.outline[point * 2],
              mark.outline[point * 2 + 1],
              mark.outline[next * 2],
              mark.outline[next * 2 + 1],
            ],
            true,
          );
          forms.fill({
            color: point % 2 === 0 ? mark.facetTone : mark.facetShadow,
            alpha: (mark.controlled ? 0.13 : 0.08) * mark.alpha,
          });
        }
        forms.poly(mark.core, true).fill({ color: mark.facetShadow, alpha: 0.72 * mark.alpha });
        forms.poly(mark.core, true);
        forms.stroke({
          width: Math.max(1, mark.radius * 0.035),
          color: mark.facetTone,
          alpha: (mark.controlled ? 0.48 : 0.24) * mark.alpha,
        });
      }

      if (scene.quality.formFacets > 0) {
        const spokes = Math.min(pairs, mark.controlled ? 9 : 6);
        for (let spoke = 0; spoke < spokes; spoke += 1) {
          const point = Math.floor((spoke / spokes) * pairs);
          forms.moveTo(mark.x, mark.y).lineTo(mark.outline[point * 2], mark.outline[point * 2 + 1]);
        }
        forms.stroke({
          width: Math.max(0.5, scene.dpr * 0.55),
          color: mark.facetTone,
          alpha: (mark.controlled ? 0.16 : 0.09) * mark.alpha,
        });
        const orbit = mark.radius * (0.58 + 0.06 * (mark.formOrdinal % 3));
        const orbitTurn = mark.heading + mark.formOrdinal * 0.34;
        for (let sample = 0; sample <= 24; sample += 1) {
          const angle = (sample / 24) * Math.PI * 2;
          const ox = Math.cos(angle) * orbit;
          const oy = Math.sin(angle) * orbit * 0.42;
          const x = mark.x + ox * Math.cos(orbitTurn) - oy * Math.sin(orbitTurn);
          const y = mark.y + ox * Math.sin(orbitTurn) + oy * Math.cos(orbitTurn);
          if (sample === 0) forms.moveTo(x, y);
          else forms.lineTo(x, y);
        }
        forms.stroke({ width: Math.max(0.5, scene.dpr * 0.55), color: mark.ringTone, alpha: 0.2 * mark.alpha });
        const orbitPhase = scene.reducedMotion ? mark.formOrdinal : scene.clock * 0.035 + mark.formOrdinal;
        const orbitX = Math.cos(orbitPhase) * orbit;
        const orbitY = Math.sin(orbitPhase) * orbit * 0.42;
        forms.circle(
          mark.x + orbitX * Math.cos(orbitTurn) - orbitY * Math.sin(orbitTurn),
          mark.y + orbitX * Math.sin(orbitTurn) + orbitY * Math.cos(orbitTurn),
          Math.max(1, mark.radius * 0.045),
        );
        forms.fill({ color: CHARGE_SPARK, alpha: (0.34 + mark.charge * 0.45) * mark.alpha });
        if (mark.formOrdinal === 0) {
          for (const side of [-1, 1]) {
            for (let sample = 0; sample <= 18; sample += 1) {
              const along = sample / 18;
              const y = mark.y - mark.radius * 0.64 + along * mark.radius * 1.28;
              const x = mark.x + Math.sin(along * Math.PI * 3.2) * mark.radius * 0.19 * side;
              if (sample === 0) forms.moveTo(x, y);
              else forms.lineTo(x, y);
            }
          }
          forms.stroke({ width: Math.max(0.65, scene.dpr * 0.7), color: mark.facetTone, alpha: 0.34 * mark.alpha });
          for (let rung = 2; rung < 18; rung += 3) {
            const along = rung / 18;
            const y = mark.y - mark.radius * 0.64 + along * mark.radius * 1.28;
            const offset = Math.sin(along * Math.PI * 3.2) * mark.radius * 0.19;
            forms.moveTo(mark.x - offset, y).lineTo(mark.x + offset, y);
          }
          forms.stroke({ width: Math.max(0.55, scene.dpr * 0.58), color: mark.ringTone, alpha: 0.28 * mark.alpha });
        }
      }

      if (mark.stress > 0) {
        const stressBeat =
          mark.stressCrisis && !scene.reducedMotion ? 0.78 + 0.22 * Math.sin(scene.clock * 0.55) : 1;
        forms.poly(mark.outline, true);
        forms.stroke({
          width: formRingWidth(mark) * (2 + mark.stress),
          color: mark.stressTone,
          alpha: mark.stress * 0.5 * stressBeat,
          join: 'round',
        });
      }
      forms.poly(mark.outline, true);
      const selected = mark.controlled || mark.focus;
      forms.stroke({
        width: formRingWidth(mark) + Math.max(1, scene.dpr * (selected ? 1.5 : 0.7)),
        color: FORM_KEYLINE,
        alpha: (selected ? 0.92 : 0.62) * mark.alpha,
        join: 'round',
      });
      forms.poly(mark.outline, true);
      forms.stroke({ width: formRingWidth(mark), color: mark.ringTone, alpha: mark.alpha, join: 'round' });

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
      if (mark.controlled) {
        forms.circle(mark.x, mark.y, mark.radius * 1.24);
        forms.stroke({
          width: Math.max(1, mark.radius * 0.055),
          color: mark.ringTone,
          alpha: 0.34 * mark.alpha,
        });
        const reservoir = Math.PI * 2 * mark.charge;
        if (reservoir > 0.001) {
          forms.arc(mark.x, mark.y, mark.radius * 1.38, -Math.PI / 2, -Math.PI / 2 + reservoir);
          forms.stroke({
            width: Math.max(1.2, mark.radius * 0.075),
            color: CHARGE_SPARK,
            alpha: (0.48 + 0.42 * mark.charge) * mark.alpha,
            cap: 'round',
          });
        }
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

  private drawPolicies(scene: Scene): void {
    const graphics = this.policies.clear();
    for (let place = 0; place < scene.policyCandidates.count; place += 1) {
      const candidate = scene.policyCandidates.items[place];
      dashedCircle(
        graphics,
        candidate.x,
        candidate.y,
        candidate.radius,
        Math.max(2, scene.dpr * 2),
        Math.max(4, scene.dpr * 4),
      );
      graphics.stroke({
        width: Math.max(0.75, scene.dpr * (candidate.selected ? 1.2 : 0.8)),
        color: candidate.tone,
        alpha: candidate.alpha,
      });
    }
    for (let place = 0; place < scene.policies.count; place += 1) {
      const mark = scene.policies.items[place];
      const beat = scene.reducedMotion ? 1 : 0.9 + 0.1 * Math.sin(mark.phase * Math.PI * 2);
      if (mark.sensorReach > mark.radius * 1.3) {
        dashedCircle(
          graphics,
          mark.x,
          mark.y,
          mark.sensorReach,
          Math.max(2, scene.dpr * 2),
          Math.max(4, scene.dpr * 5),
        );
        graphics.stroke({
          width: Math.max(0.55, scene.dpr * 0.65),
          color: mark.tone,
          alpha: mark.alpha * 0.26,
        });
      }
      if (mark.reach > mark.radius * 1.3) {
        dashedCircle(
          graphics,
          mark.x,
          mark.y,
          mark.reach,
          Math.max(mark.preview ? 5 : 3, scene.dpr * (mark.preview ? 5 : 3.5)),
          Math.max(mark.preview ? 4 : 6, scene.dpr * (mark.preview ? 4 : 7)),
        );
        graphics.stroke({
          width: Math.max(0.65, scene.dpr * 0.7),
          color: mark.tone,
          alpha: mark.alpha * 0.18,
        });
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
        dashed(graphics, startX, startY, endX, endY, Math.max(4, scene.dpr * 4), Math.max(5, scene.dpr * 5));
        graphics.stroke({
          width: Math.max(0.8, scene.dpr),
          color: mark.targetTone,
          alpha: mark.alpha * 0.48,
          cap: 'round',
        });
        const arrowX = startX + (endX - startX) * 0.68;
        const arrowY = startY + (endY - startY) * 0.68;
        const back = Math.max(4, scene.dpr * 5);
        const wing = Math.max(3, scene.dpr * 3.5);
        graphics
          .moveTo(arrowX - tx * back + nx * wing, arrowY - ty * back + ny * wing)
          .lineTo(arrowX, arrowY)
          .lineTo(arrowX - tx * back - nx * wing, arrowY - ty * back - ny * wing)
          .stroke({
            width: Math.max(0.8, scene.dpr),
            color: mark.targetTone,
            alpha: mark.alpha * 0.72,
            cap: 'round',
            join: 'round',
          });
        for (let corner = 0; corner < 4; corner += 1) {
          const angle = corner * Math.PI / 2 + mark.phase * 0.35;
          graphics.arc(
            mark.targetX,
            mark.targetY,
            mark.targetRadius * beat,
            angle - Math.PI * 0.16,
            angle + Math.PI * 0.16,
          );
        }
        graphics.stroke({
          width: Math.max(1, scene.dpr * 1.15),
          color: mark.targetTone,
          alpha: mark.alpha * 0.82,
          cap: 'round',
        });
      }

      const segments = policySegments(mark.action);
      for (let segment = 0; segment < segments; segment += 1) {
        const center = -Math.PI / 2 + (segment / segments) * Math.PI * 2 + mark.phase * 0.22;
        const half = segments === 1 ? Math.PI * 0.76 : (Math.PI / segments) * 0.52;
        graphics.arc(mark.x, mark.y, mark.radius * beat, center - half, center + half);
      }
      graphics.stroke({
        width: Math.max(1.1, scene.dpr * 1.35),
        color: mark.tone,
        alpha: mark.alpha,
        cap: 'round',
      });

      if (mark.action === 'emit_signal') {
        graphics.circle(mark.x, mark.y, mark.radius * (1.28 + mark.phase * 0.24));
        graphics.stroke({
          width: Math.max(0.7, scene.dpr * 0.8),
          color: mark.tone,
          alpha: mark.alpha * (0.42 - mark.phase * 0.2),
        });
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
        graphics
          .moveTo(mark.x - cut, mark.y - cut)
          .lineTo(mark.x + cut, mark.y + cut)
          .moveTo(mark.x + cut, mark.y - cut)
          .lineTo(mark.x - cut, mark.y + cut)
          .stroke({
            width: Math.max(0.9, scene.dpr),
            color: mark.tone,
            alpha: mark.alpha * 0.72,
            cap: 'round',
          });
      }
    }
  }

  /** Accepted Design assembly changes, never a causal Field transition. */
  private drawAssemblyDraft(scene: Scene): void {
    const graphics = this.assemblyDraft.clear();
    const transition = scene.engineeringTransition;
    if (transition.active && transition.operation === 'full_contract_reset') {
      const span = Math.min(scene.width, scene.height);
      const travel = transition.stage === 'reconstruct'
        ? (1 - transition.phase) * span * 0.08
        : transition.stage === 'settle'
          ? (1 - transition.phase) * span * 0.025
          : 0;
      const inset = span * 0.035 + travel;
      const width = Math.max(1.2, scene.dpr * 1.45);
      graphics.rect(inset, inset, Math.max(1, scene.width - inset * 2), Math.max(1, scene.height - inset * 2));
      graphics.stroke({
        width,
        color: transition.authorityTone,
        alpha: transition.stage === 'preview' ? 0.34 : 0.62,
      });
      graphics.rect(
        inset + span * 0.012,
        inset + span * 0.012,
        Math.max(1, scene.width - (inset + span * 0.012) * 2),
        Math.max(1, scene.height - (inset + span * 0.012) * 2),
      );
      graphics.stroke({
        width: Math.max(0.8, scene.dpr),
        color: transition.tone,
        alpha: transition.stage === 'preview' ? 0.2 : 0.38,
      });
    }
    for (let place = 0; place < scene.assemblyDrafts.count; place += 1) {
      const mark = scene.assemblyDrafts.items[place];
      const radius = mark.radius;
      const bracket = radius * 1.18;
      const corner = Math.max(scene.dpr * 3, radius * 0.28);
      if (mark.displaced) {
        dashed(
          graphics,
          mark.fromX,
          mark.fromY,
          mark.x,
          mark.y,
          Math.max(3, scene.dpr * 3),
          Math.max(5, scene.dpr * 5),
        );
        graphics.stroke({
          width: Math.max(0.8, scene.dpr * 0.9),
          color: mark.originTone,
          alpha: mark.alpha * 0.52,
          cap: 'round',
        });
        graphics.circle(mark.fromX, mark.fromY, mark.fromRadius);
        graphics.moveTo(mark.fromX - mark.fromRadius * 0.52, mark.fromY);
        graphics.lineTo(mark.fromX + mark.fromRadius * 0.52, mark.fromY);
        graphics.stroke({
          width: Math.max(0.8, scene.dpr),
          color: mark.originTone,
          alpha: mark.alpha * 0.7,
          cap: 'round',
        });
      }

      graphics.moveTo(mark.x - bracket, mark.y - bracket + corner);
      graphics.lineTo(mark.x - bracket, mark.y - bracket);
      graphics.lineTo(mark.x - bracket + corner, mark.y - bracket);
      graphics.moveTo(mark.x + bracket - corner, mark.y - bracket);
      graphics.lineTo(mark.x + bracket, mark.y - bracket);
      graphics.lineTo(mark.x + bracket, mark.y - bracket + corner);
      graphics.moveTo(mark.x + bracket, mark.y + bracket - corner);
      graphics.lineTo(mark.x + bracket, mark.y + bracket);
      graphics.lineTo(mark.x + bracket - corner, mark.y + bracket);
      graphics.moveTo(mark.x - bracket + corner, mark.y + bracket);
      graphics.lineTo(mark.x - bracket, mark.y + bracket);
      graphics.lineTo(mark.x - bracket, mark.y + bracket - corner);
      graphics.stroke({
        width: Math.max(1, scene.dpr * 1.25),
        color: mark.tone,
        alpha: mark.alpha,
        cap: 'round',
        join: 'round',
      });

      dashedCircle(
        graphics,
        mark.x,
        mark.y,
        radius,
        Math.max(4, scene.dpr * 4),
        Math.max(3, scene.dpr * 3),
      );
      graphics.stroke({
        width: Math.max(0.8, scene.dpr),
        color: mark.tone,
        alpha: mark.alpha * 0.76,
        cap: 'round',
      });

      if ((mark.fields & (ASSEMBLY_DRAFT_FIELD.charge | ASSEMBLY_DRAFT_FIELD.reserve | ASSEMBLY_DRAFT_FIELD.amount | ASSEMBLY_DRAFT_FIELD.leakage)) !== 0) {
        const beforeAngle = -Math.PI / 2 + Math.PI * 2 * mark.beforeLevel;
        const afterAngle = -Math.PI / 2 + Math.PI * 2 * mark.afterLevel;
        if (mark.beforeLevel > 0) {
          graphics.arc(mark.x, mark.y, radius * 0.76, -Math.PI / 2, beforeAngle);
          graphics.stroke({
            width: Math.max(1, radius * 0.09),
            color: mark.originTone,
            alpha: mark.alpha * 0.72,
            cap: 'round',
          });
        }
        if (mark.afterLevel > 0) {
          graphics.arc(mark.x, mark.y, radius * 0.9, -Math.PI / 2, afterAngle);
          graphics.stroke({
            width: Math.max(1.2, radius * 0.13),
            color: mark.tone,
            alpha: mark.alpha,
            cap: 'round',
          });
        }
      }

      if ((mark.fields & ASSEMBLY_DRAFT_FIELD.interface) !== 0) {
        if (mark.open) {
          graphics.moveTo(mark.x - radius * 0.62, mark.y).lineTo(mark.x - radius * 0.15, mark.y);
          graphics.moveTo(mark.x + radius * 0.15, mark.y).lineTo(mark.x + radius * 0.62, mark.y);
        } else {
          graphics.moveTo(mark.x - radius * 0.58, mark.y).lineTo(mark.x + radius * 0.58, mark.y);
          graphics.moveTo(mark.x, mark.y - radius * 0.42).lineTo(mark.x, mark.y + radius * 0.42);
        }
        graphics.stroke({
          width: Math.max(1.1, radius * 0.1),
          color: mark.tone,
          alpha: mark.alpha,
          cap: 'round',
        });
      }

      if ((mark.fields & ASSEMBLY_DRAFT_FIELD.blanks) !== 0) {
        const count = Math.max(1, Math.min(8, mark.count));
        for (let index = 0; index < count; index += 1) {
          const angle = -Math.PI / 2 + (index - (count - 1) / 2) * 0.3;
          graphics.moveTo(
            mark.x + Math.cos(angle) * radius * 0.42,
            mark.y + Math.sin(angle) * radius * 0.42,
          );
          graphics.lineTo(
            mark.x + Math.cos(angle) * radius * 0.68,
            mark.y + Math.sin(angle) * radius * 0.68,
          );
        }
        graphics.stroke({
          width: Math.max(1, scene.dpr),
          color: mark.tone,
          alpha: mark.alpha * 0.9,
          cap: 'round',
        });
      }

      if (mark.kind === 'material') {
        graphics.poly([
          mark.x, mark.y - radius * 0.48,
          mark.x + radius * 0.48, mark.y,
          mark.x, mark.y + radius * 0.48,
          mark.x - radius * 0.48, mark.y,
        ], true);
        graphics.stroke({
          width: Math.max(1, scene.dpr),
          color: mark.tone,
          alpha: mark.alpha * 0.92,
          join: 'round',
        });
        for (let index = 0; index < mark.count; index += 1) {
          const angle = index * 2.39996;
          const reach = radius * (0.12 + 0.055 * index);
          graphics.circle(
            mark.x + Math.cos(angle) * reach,
            mark.y + Math.sin(angle) * reach,
            Math.max(1, radius * 0.055),
          );
          graphics.fill({ color: mark.tone, alpha: mark.alpha * (0.86 - index * 0.06) });
        }
      }

      if (mark.kind === 'current') {
        const angle = -Math.PI / 2 + Math.PI * 2 * mark.phase;
        graphics.moveTo(mark.x, mark.y);
        graphics.lineTo(mark.x + Math.cos(angle) * radius * 0.62, mark.y + Math.sin(angle) * radius * 0.62);
        if (!mark.active) {
          graphics.moveTo(mark.x - radius * 0.46, mark.y + radius * 0.46);
          graphics.lineTo(mark.x + radius * 0.46, mark.y - radius * 0.46);
        }
        graphics.stroke({
          width: Math.max(1.1, radius * 0.1),
          color: mark.tone,
          alpha: mark.alpha,
          cap: 'round',
        });
      }

      if (mark.kind === 'route') {
        graphics
          .moveTo(mark.x - radius * 0.58, mark.y)
          .lineTo(mark.x + radius * 0.58, mark.y)
          .moveTo(mark.x + radius * 0.18, mark.y - radius * 0.22)
          .lineTo(mark.x + radius * 0.58, mark.y)
          .lineTo(mark.x + radius * 0.18, mark.y + radius * 0.22);
        graphics.stroke({
          width: Math.max(1.1, radius * 0.1),
          color: mark.tone,
          alpha: mark.alpha,
          cap: 'round',
          join: 'round',
        });
      }

      if ((mark.fields & ASSEMBLY_DRAFT_FIELD.layer) !== 0) {
        graphics.moveTo(mark.x + bracket * 1.16, mark.y - radius * 0.5);
        graphics.lineTo(mark.x + bracket * 1.16, mark.y + radius * 0.5);
        graphics.moveTo(mark.x + bracket * 1.3, mark.y - radius * 0.28);
        graphics.lineTo(mark.x + bracket * 1.3, mark.y + radius * 0.72);
        graphics.stroke({
          width: Math.max(1, scene.dpr),
          color: mark.tone,
          alpha: mark.alpha * 0.9,
          cap: 'round',
        });
      }

      if ((mark.fields & ASSEMBLY_DRAFT_FIELD.members) !== 0 && mark.kind === 'physical_compartment') {
        graphics.moveTo(mark.x - radius * 0.45, mark.y - radius * 0.22);
        graphics.lineTo(mark.x + radius * 0.45, mark.y - radius * 0.22);
        graphics.moveTo(mark.x - radius * 0.45, mark.y + radius * 0.22);
        graphics.lineTo(mark.x + radius * 0.45, mark.y + radius * 0.22);
        graphics.stroke({
          width: Math.max(1, scene.dpr),
          color: mark.tone,
          alpha: mark.alpha * 0.85,
          cap: 'round',
        });
      }

      if (mark.companion && scene.engineeringTransition.active) {
        const transition = scene.engineeringTransition;
        const lockRadius = radius * (1.34 + (transition.stage === 'settle'
          ? (1 - transition.phase) * 0.42
          : 0));
        if (transition.stage === 'deenergize') {
          const contraction = radius * (1.82 - transition.phase * 0.72);
          for (let segment = 0; segment < 8; segment += 1) {
            const angle = segment * Math.PI / 4;
            graphics
              .moveTo(
                mark.fromX + Math.cos(angle) * contraction,
                mark.fromY + Math.sin(angle) * contraction,
              )
              .lineTo(
                mark.fromX + Math.cos(angle) * contraction * 0.72,
                mark.fromY + Math.sin(angle) * contraction * 0.72,
              );
          }
          graphics.stroke({
            width: Math.max(1, scene.dpr * 1.2),
            color: transition.originTone,
            alpha: mark.alpha * (0.9 - transition.phase * 0.36),
            cap: 'round',
          });
        } else if (transition.stage === 'reconstruct') {
          const sweep = Math.PI * 0.08 + Math.PI * 0.38 * transition.phase;
          for (let corner = 0; corner < 4; corner += 1) {
            const angle = corner * Math.PI / 2 - Math.PI / 4;
            graphics.arc(mark.x, mark.y, lockRadius, angle - sweep, angle + sweep);
          }
          graphics.stroke({
            width: Math.max(1.2, scene.dpr * 1.45),
            color: transition.tone,
            alpha: mark.alpha * 0.9,
            cap: 'round',
          });
        } else if (
          transition.stage === 'settle'
          || transition.stage === 'locked'
          || transition.stage === 'preview'
        ) {
          const authorityAlpha = transition.stage === 'preview' ? 0.56 : 0.9;
          if (mark.compatibility === 'hard_refusal') {
            const cross = lockRadius * 0.7;
            graphics
              .moveTo(mark.x - cross, mark.y - cross)
              .lineTo(mark.x + cross, mark.y + cross)
              .moveTo(mark.x + cross, mark.y - cross)
              .lineTo(mark.x - cross, mark.y + cross);
            for (let corner = 0; corner < 4; corner += 1) {
              const angle = corner * Math.PI / 2;
              graphics.arc(
                mark.x,
                mark.y,
                lockRadius,
                angle - Math.PI * 0.09,
                angle + Math.PI * 0.09,
              );
            }
            graphics.stroke({
              width: Math.max(1.4, scene.dpr * 1.7),
              color: OVERLOAD,
              alpha: mark.alpha * 0.92,
              cap: 'round',
            });
          } else if (mark.compatibility === 'adaptation_required') {
            for (let segment = 0; segment < 8; segment += 1) {
              const angle = segment * Math.PI / 4;
              graphics.arc(
                mark.x,
                mark.y,
                lockRadius,
                angle - Math.PI * 0.055,
                angle + Math.PI * 0.055,
              );
            }
            graphics.stroke({
              width: Math.max(1.2, scene.dpr * 1.5),
              color: transition.tone,
              alpha: mark.alpha * authorityAlpha,
              cap: 'round',
            });
          } else if (mark.compatibility === 'retained_unchanged') {
            graphics.circle(mark.x, mark.y, lockRadius);
            graphics.circle(mark.x, mark.y, lockRadius * 0.82);
            graphics.stroke({
              width: Math.max(1, scene.dpr * 1.15),
              color: transition.authorityTone,
              alpha: mark.alpha * authorityAlpha,
            });
          } else {
            for (let corner = 0; corner < 4; corner += 1) {
              const angle = corner * Math.PI / 2;
              graphics.arc(mark.x, mark.y, lockRadius, angle - Math.PI * 0.16, angle + Math.PI * 0.16);
            }
            graphics.stroke({
              width: Math.max(1.2, scene.dpr * 1.5),
              color: transition.authorityTone,
              alpha: mark.alpha * authorityAlpha,
              cap: 'round',
            });
          }
          if (transition.stage !== 'preview' && mark.compatibility !== 'hard_refusal') {
            graphics.circle(mark.x, mark.y, Math.max(scene.dpr * 1.25, radius * 0.08));
            graphics.fill({ color: transition.authorityTone, alpha: mark.alpha * 0.72 });
          }
        }
      }
    }
  }

  private drawCoupling(scene: Scene): void {
    const graphics = this.coupling.clear();
    const field = scene.coupling;
    if (!field.active || field.radius <= 0) return;

    dashedCircle(
      graphics,
      field.x,
      field.y,
      field.radius,
      Math.max(5 * scene.dpr, field.radius * 0.045),
      Math.max(4 * scene.dpr, field.radius * 0.028),
    );
    graphics.stroke({
      width: Math.max(1, scene.dpr * (field.connected ? 1.45 : 0.9)),
      color: field.tone,
      alpha: field.alpha * (field.connected ? 0.82 : 0.42),
    });
    const sweep = -Math.PI / 2 + field.phase * Math.PI * 2;
    graphics.arc(field.x, field.y, field.radius, sweep, sweep + Math.PI * (0.24 + 0.28 * field.charge));
    graphics.stroke({
      width: Math.max(1.5, scene.dpr * 2.1),
      color: field.tone,
      alpha: field.alpha,
      cap: 'round',
    });
    const ticks = 20;
    for (let tick = 0; tick < ticks; tick += 1) {
      const angle = (tick / ticks) * Math.PI * 2;
      const outer = field.radius + (tick % 5 === 0 ? 5 : 2) * scene.dpr;
      const inner = field.radius - (tick % 5 === 0 ? 5 : 2) * scene.dpr;
      graphics
        .moveTo(field.x + Math.cos(angle) * inner, field.y + Math.sin(angle) * inner)
        .lineTo(field.x + Math.cos(angle) * outer, field.y + Math.sin(angle) * outer);
    }
    graphics.stroke({ width: Math.max(0.7, scene.dpr * 0.75), color: field.tone, alpha: field.alpha * 0.48 });

    for (let place = 0; place < scene.couplingTargets.count; place += 1) {
      const target = scene.couplingTargets.items[place];
      const dx = target.x - field.x;
      const dy = target.y - field.y;
      const length = Math.max(1, Math.hypot(dx, dy));
      const nx = -dy / length;
      const ny = dx / length;
      const bow = Math.min(22 * scene.dpr, length * 0.12) * (place % 2 === 0 ? 1 : -1);
      for (let sample = 0; sample <= 18; sample += 1) {
        const along = sample / 18;
        const bend = Math.sin(along * Math.PI) * bow;
        const x = field.x + dx * along + nx * bend;
        const y = field.y + dy * along + ny * bend;
        if (sample === 0) graphics.moveTo(x, y);
        else graphics.lineTo(x, y);
      }
      graphics.stroke({ width: Math.max(0.8, scene.dpr), color: target.tone, alpha: target.alpha * 0.5, cap: 'round' });

      const lockBeat = scene.reducedMotion ? 1 : 0.9 + 0.1 * Math.sin(scene.clock * 0.42 + place);
      for (let corner = 0; corner < 4; corner += 1) {
        const angle = corner * Math.PI / 2 + target.phase * Math.PI * 0.5;
        graphics.arc(
          target.x,
          target.y,
          target.radius * lockBeat,
          angle - Math.PI * 0.18,
          angle + Math.PI * 0.18,
        );
      }
      graphics.stroke({ width: Math.max(1.2, scene.dpr * 1.5), color: target.tone, alpha: target.alpha, cap: 'round' });

      if ((target.effects & COUPLING_EFFECT.gather) !== 0) {
        for (let bead = 0; bead < 3; bead += 1) {
          const along = 1 - ((field.phase * 1.8 + bead / 3) % 1);
          graphics.circle(field.x + dx * along, field.y + dy * along, Math.max(1.1, scene.dpr * 1.45));
          graphics.fill({ color: CHARGE_SPARK, alpha: target.alpha * Math.sin(Math.PI * along) });
        }
      }
      if ((target.effects & COUPLING_EFFECT.open) !== 0) {
        graphics.circle(target.x, target.y, target.radius * 0.56);
        graphics.stroke({ width: Math.max(1, scene.dpr), color: CURRENT_BRIGHT, alpha: target.alpha * 0.78 });
      }
      if ((target.effects & COUPLING_EFFECT.suppress) !== 0) {
        const tx = dx / length;
        const ty = dy / length;
        for (let push = 0; push < 2; push += 1) {
          const at = target.radius * (0.85 + push * 0.35);
          const px = target.x + tx * at;
          const py = target.y + ty * at;
          graphics
            .moveTo(px - tx * 5 * scene.dpr + nx * 4 * scene.dpr, py - ty * 5 * scene.dpr + ny * 4 * scene.dpr)
            .lineTo(px, py)
            .lineTo(px - tx * 5 * scene.dpr - nx * 4 * scene.dpr, py - ty * 5 * scene.dpr - ny * 4 * scene.dpr);
        }
        graphics.stroke({ width: Math.max(1, scene.dpr), color: target.tone, alpha: target.alpha * 0.85 });
      }
    }
  }

  private drawParticles(scene: Scene): void {
    this.particles.open();
    const geometry = this.materialGeometry.clear();
    for (let place = 0; place < scene.particles.count; place += 1) {
      const mark = scene.particles.items[place];
      placeSoft(this.particles.next(), mark.x, mark.y, mark.radius * 2.6, mark.tone, mark.alpha);
      if (mark.subject === 1) {
        geometry.circle(mark.x, mark.y, mark.radius * 1.55);
        geometry.stroke({ width: Math.max(0.6, scene.dpr * 0.72), color: mark.tone, alpha: mark.alpha * 0.68 });
        for (let spoke = 0; spoke < 6; spoke += 1) {
          const angle = (spoke / 6) * Math.PI * 2 + mark.id * 0.7;
          geometry
            .moveTo(mark.x + Math.cos(angle) * mark.radius * 0.65, mark.y + Math.sin(angle) * mark.radius * 0.65)
            .lineTo(mark.x + Math.cos(angle) * mark.radius * 1.8, mark.y + Math.sin(angle) * mark.radius * 1.8);
        }
        geometry.stroke({ width: Math.max(0.5, scene.dpr * 0.52), color: mark.tone, alpha: mark.alpha * 0.38 });
      } else if (mark.subject === 2) {
        for (let ring = 0; ring < 3; ring += 1) {
          geometry.circle(mark.x, mark.y, mark.radius * (1.2 + ring * 0.42));
          geometry.stroke({
            width: Math.max(0.5, scene.dpr * (0.75 - ring * 0.12)),
            color: mark.tone,
            alpha: mark.alpha * (0.6 - ring * 0.14),
          });
        }
      }
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
   * a square at a Route's end, a diamond on a Compartment vertex — which is the
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
    const localWeight = scene.rim.localized ? 0.46 : 1;
    if (!scene.quality.pressureBloom) {
      this.rim.visible = false;
      const inset = Math.max(2, scene.dpr * 2);
      this.cues.rect(inset, inset, Math.max(1, scene.width - inset * 2), Math.max(1, scene.height - inset * 2));
      this.cues.stroke({
        width: Math.max(1, scene.dpr * (1 + scene.rim.level * 2)),
        color: scene.rim.tone,
        alpha: scene.rim.level * 0.42 * localWeight,
      });
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
      scene.rim.level *
        (scene.rim.crisis ? 0.85 : 0.5) *
        (0.75 + 0.25 * scene.rim.beat) *
        localWeight,
    );
  }
}
