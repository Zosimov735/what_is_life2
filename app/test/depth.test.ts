/**
 * Depth, from a device to the frame the worker reads.
 *
 * The contracts under test are the ones `docs/field-framework/ARCHITECTURE.md`
 * locks — `wheel` as the raw delta sum since the previous frame clamped to
 * [−3000, 3000], `depth_key` as −1, 0, or +1 — and the ones this goal is done
 * by: the two-finger vertical gesture is the wheel and reaches depth through the
 * same path a mouse does, a bracket press names one change and no more, and a
 * wheel over the play surface is consumed so the page never scrolls while a
 * wheel over the surrounding chrome is left entirely alone.
 *
 * Everything about when a gesture becomes a depth change — the accumulated
 * threshold, the cooldown, the deferral past a frame that runs no step — is the
 * core's, and is read against a real core in `depth-run.test.ts`.
 */

import { afterEach, expect, test, vi } from 'vitest';
import { neutralFrame, type InputFrame } from '../../worker/src/protocol';
import { DEV_RUN_EXPORT } from '../src/shell/dev-run';
import {
  DEPTH_BINDINGS,
  openDepth,
  PLAY_SURFACE,
  WHEEL_LIMIT,
  WHEEL_LINE_PX,
  WHEEL_PAGE_PX,
  type Depth,
} from '../src/shell/depth';

const opened: Depth[] = [];

afterEach(() => {
  for (const source of opened.splice(0)) source.close();
  document.body.innerHTML = '';
  vi.restoreAllMocks();
});

/** A depth source over the window, with the surface the shell's own. */
function depth(): Depth {
  const source = openDepth();
  opened.push(source);
  return source;
}

/** The play surface, as the shell builds it: a canvas of class `field`. */
function surface(): HTMLCanvasElement {
  const canvas = document.createElement('canvas');
  canvas.className = 'field';
  document.body.append(canvas);
  return canvas;
}

/** Everything that is not the play surface: a notice beside it. */
function chrome(): HTMLElement {
  const notice = document.createElement('p');
  notice.className = 'notice';
  document.body.append(notice);
  return notice;
}

/** Turns the wheel over an element, and answers with the event that stood. */
function turn(
  over: Element,
  deltaY: number,
  extra: { deltaMode?: number; ctrlKey?: boolean; metaKey?: boolean; deltaX?: number } = {},
): WheelEvent {
  const event = new WheelEvent('wheel', {
    deltaY,
    bubbles: true,
    cancelable: true,
    ...extra,
  });
  over.dispatchEvent(event);
  return event;
}

/** Presses or releases a key by its code. */
function press(code: string, down = true, extra: Record<string, unknown> = {}): void {
  window.dispatchEvent(
    Object.assign(new Event(down ? 'keydown' : 'keyup'), { code, repeat: false, ...extra }),
  );
}

/** One frame's worth of depth, as the whole `InputFrame` the pump sends. */
function framed(source: Depth, seq = 1): InputFrame {
  return { ...neutralFrame(seq, seq * 16_667), ...source.sample() };
}

// ---------------------------------------------------------------------------
// The wheel, and the page that never scrolls
// ---------------------------------------------------------------------------

test('a wheel over the play surface is consumed and carried', () => {
  const source = depth();
  const event = turn(surface(), 120);

  expect(event.defaultPrevented).toBe(true);
  expect(source.sample()).toEqual({ wheel: 120, depth_key: 0 });
});

test('a wheel over the surrounding chrome scrolls the page and carries nothing', () => {
  // The one case SPEC.md's test plan names by itself: the page must never
  // scroll under the surface, and must still scroll everywhere else.
  const source = depth();
  const event = turn(chrome(), 120);

  expect(event.defaultPrevented).toBe(false);
  expect(source.sample()).toEqual({ wheel: 0, depth_key: 0 });
});

test('a sideways wheel over the surface is swallowed and read as nothing', () => {
  // A shift-held wheel and a two-finger sideways swipe both arrive as `deltaX`
  // with no `deltaY`. Neither is a depth change, and both are consumed anyway:
  // left to the platform they would rubber-band the page or take a browser back
  // through its history under a player who was steering.
  const source = depth();
  const event = turn(surface(), 0, { deltaX: 300 });

  expect(event.defaultPrevented).toBe(true);
  expect(source.sample()).toEqual({ wheel: 0, depth_key: 0 });
});

test('a wheel carrying a platform modifier is left to the platform', () => {
  const source = depth();
  const zoom = turn(surface(), -240, { ctrlKey: true });
  const other = turn(surface(), -240, { metaKey: true });

  expect(zoom.defaultPrevented).toBe(false);
  expect(other.defaultPrevented).toBe(false);
  expect(source.sample()).toEqual({ wheel: 0, depth_key: 0 });
});

test('the deltas of one frame sum, and both signs reach the frame as they arrived', () => {
  // Two fingers on a trackpad report many small deltas between two rendered
  // frames. The sign is the platform's own — whichever way the player's system
  // reads as downward is the way that goes deeper — so nothing here inverts it.
  const source = depth();
  const canvas = surface();
  for (const delta of [40, 30, 30, -20]) turn(canvas, delta);
  expect(source.sample().wheel).toBe(80);

  for (const delta of [-40, -30, -10]) turn(canvas, delta);
  expect(source.sample().wheel).toBe(-80);

  // And a frame that saw no wheel at all carries none.
  expect(source.sample().wheel).toBe(0);
});

test('a delta in lines or in pages reaches the frame in the same unit as one in px', () => {
  // A wheel reports its delta in one of three units and which one is the
  // platform's business. The locked trigger is a distance, so a notch has to be
  // worth the same wherever it came from.
  const source = depth();
  const canvas = surface();

  turn(canvas, 3, { deltaMode: 1 });
  expect(source.sample().wheel).toBe(3 * WHEEL_LINE_PX);

  turn(canvas, 2, { deltaMode: 2 });
  expect(source.sample().wheel).toBe(2 * WHEEL_PAGE_PX);
});

test('a sub-pixel gesture loses nothing to the whole units the frame carries', () => {
  const source = depth();
  const canvas = surface();
  // A third of a pixel a frame: nothing crosses for two frames, and the third
  // carries the whole unit the three of them made.
  for (const expected of [0, 0, 1, 0, 0, 1]) {
    turn(canvas, 1 / 3);
    expect(source.sample().wheel).toBe(expected);
  }
});

test('one frame carries at most the locked clamp', () => {
  const source = depth();
  const canvas = surface();
  turn(canvas, 12_000);
  expect(source.sample().wheel).toBe(WHEEL_LIMIT);

  turn(canvas, -12_000);
  expect(source.sample().wheel).toBe(-WHEEL_LIMIT);

  // The excess is dropped rather than carried into the frame after it: one
  // flick is one gesture.
  expect(source.sample().wheel).toBe(0);
});

// ---------------------------------------------------------------------------
// The bracket keys
// ---------------------------------------------------------------------------

test('the bracket keys name the two directions, one press at a time', () => {
  const source = depth();

  press('BracketRight');
  expect(source.sample().depth_key).toBe(1);
  // One frame carries the offer at a time: the frame that carries a press is
  // the frame whose answer decides it.
  expect(source.sample().depth_key).toBe(0);
  expect(source.held()).toBe(1);

  // A step took it, so it is spent — holding the key names nothing more.
  source.settle(1);
  expect(source.sample().depth_key).toBe(0);
  expect(source.sample().depth_key).toBe(0);

  press('BracketRight', false);
  press('BracketLeft');
  expect(source.sample().depth_key).toBe(-1);
  source.settle(1);
  expect(source.sample().depth_key).toBe(0);
});

test('the bindings are the ones the core opens a run with', () => {
  // The core owns the default bindings and writes them into every run it opens;
  // this source names the same two codes for the two directions, because
  // `InputConfig` does not cross the frame boundary yet. Read here off bytes
  // the core itself wrote, so the two cannot drift apart unnoticed.
  const bindings = (JSON.parse(DEV_RUN_EXPORT) as {
    payload: { input_config: { bindings: Record<string, string> } };
  }).payload.input_config.bindings;

  expect(DEPTH_BINDINGS[bindings.ascend]).toBe(-1);
  expect(DEPTH_BINDINGS[bindings.descend]).toBe(1);
  expect(DEPTH_BINDINGS).toEqual({ BracketLeft: -1, BracketRight: 1 });
});

test('a press a stepless frame carried is offered again until a step takes it', () => {
  // The core resolves depth only on a frame that executes a step, and holds
  // nothing of a press a stepless frame carried — so the offer stands here
  // until an answer says a step ran. Everything this source holds is spent into
  // the frames it fills, which is what keeps an export taken between the two a
  // record a restore can carry on from.
  const source = depth();
  press('BracketRight');

  for (let frame = 0; frame < 3; frame += 1) {
    expect(source.sample().depth_key).toBe(1);
    // A frame that ran no step did not consume it, so the next one offers it.
    source.settle(0);
  }

  // A refused frame, or one a replaced worker never answered, counts as no step
  // and keeps the press for the same reason.
  expect(source.sample().depth_key).toBe(1);
  source.settle(0);

  expect(source.sample().depth_key).toBe(1);
  source.settle(1);
  expect(source.sample().depth_key).toBe(0);

  // A settle for a frame that carried nothing changes nothing at all.
  source.settle(1);
  expect(source.sample().depth_key).toBe(0);
});

test('a press made while an earlier one waits takes a frame of its own', () => {
  const source = depth();
  press('BracketRight');
  expect(source.sample().depth_key).toBe(1);

  // Two presses are two changes: the second does not queue behind the first,
  // and the first is still the frame it went out on.
  press('BracketRight', false);
  press('BracketLeft');
  expect(source.sample().depth_key).toBe(-1);
  source.settle(1);
  expect(source.sample().depth_key).toBe(0);
});

test('both brackets held ask for nothing', () => {
  const source = depth();
  press('BracketRight');
  press('BracketLeft');
  expect(source.held()).toBe(2);
  expect(source.sample().depth_key).toBe(0);
  expect(source.sample().depth_key).toBe(0);

  // And letting one of them go does not resolve the other: the direction is
  // named by a press, and the press that stands was answered already.
  press('BracketLeft', false);
  expect(source.sample().depth_key).toBe(0);
});

test('the platform key repeat and the platform shortcuts are not depth changes', () => {
  const source = depth();
  press('BracketRight', true, { repeat: true });
  expect(source.sample().depth_key).toBe(0);
  expect(source.held()).toBe(0);

  press('BracketRight', true, { metaKey: true });
  expect(source.sample().depth_key).toBe(0);
  press('BracketRight', true, { ctrlKey: true });
  expect(source.sample().depth_key).toBe(0);
  press('BracketRight', true, { altKey: true });
  expect(source.sample().depth_key).toBe(0);

  // A key the bindings do not name is not one either.
  press('KeyQ');
  expect(source.sample().depth_key).toBe(0);
});

test('a press and a release between two frames still carries its direction', () => {
  const source = depth();
  press('BracketRight');
  press('BracketRight', false);
  expect(source.held()).toBe(0);
  expect(source.sample().depth_key).toBe(1);
});

// ---------------------------------------------------------------------------
// Focus loss, and the frame the pump sends
// ---------------------------------------------------------------------------

test('letting go drops the gesture rather than finishing it', () => {
  const source = depth();
  const canvas = surface();
  turn(canvas, 400);
  press('BracketRight');

  source.clear();
  expect(source.held()).toBe(0);
  expect(source.sample()).toEqual({ wheel: 0, depth_key: 0 });

  // And the source is still listening: what is dropped is what was held, not
  // the ability to hold anything again.
  turn(canvas, 60);
  expect(source.sample().wheel).toBe(60);
});

test('a closed source hears nothing more, and outlives nothing it held', () => {
  const source = openDepth();
  const canvas = surface();
  // Held at the moment it closes: a gesture part made and a press not yet
  // taken. Neither may be carried into whatever session opens next.
  turn(canvas, 400);
  press('BracketRight');
  source.close();
  expect(source.held()).toBe(0);
  expect(source.sample()).toEqual({ wheel: 0, depth_key: 0 });

  const event = turn(canvas, 400);
  press('BracketLeft');
  expect(event.defaultPrevented).toBe(false);
  expect(source.sample()).toEqual({ wheel: 0, depth_key: 0 });
});

test('the play surface the source reads is the one the shell mounts', () => {
  // The selector is exported so the shell's own surface can be asserted against
  // it, which `shell.test.tsx` does on the element React mounts. Here it is the
  // predicate itself: this element is the play surface and that one is not.
  expect(surface().matches(PLAY_SURFACE)).toBe(true);
  expect(chrome().matches(PLAY_SURFACE)).toBe(false);
});

test('the two fields reach the frame in the shape the protocol declares', () => {
  const source = depth();
  turn(surface(), 200);
  press('BracketLeft');

  const frame = framed(source, 4);
  expect(frame.wheel).toBe(200);
  expect(frame.depth_key).toBe(-1);
  // Every declared field is present, and the depth fields are integers inside
  // the locked ranges whatever the devices reported.
  expect(Number.isInteger(frame.wheel)).toBe(true);
  expect(Math.abs(frame.wheel)).toBeLessThanOrEqual(WHEEL_LIMIT);
  expect([-1, 0, 1]).toContain(frame.depth_key);
  expect(JSON.parse(JSON.stringify(frame))).toEqual(frame);
});
