/**
 * The one visible objective, the optional `Why?`, and the local telemetry.
 *
 * What is under test is the onboarding contract's own chrome rules: one
 * objective visible at a time, at most six words, detail behind `Why?` and
 * nowhere else, nothing modal, and every string from the catalog.
 */

import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, expect, test, vi } from 'vitest';
import catalog from '../../content/copy/catalog.json';
import chapter from '../../content/chapters/the_pull.json';
import { Objective, explanationKey } from '../src/shell/Objective';
import { openTelemetry, TELEMETRY_MARKS } from '../src/shell/telemetry';
import type { FrameState } from '../../../field_game/worker/src/frame-state';
import type { ObjectiveState } from '../../../field_game/worker/src/protocol';

afterEach(cleanup);

const entries = catalog.entries as Record<string, { kind: string; text: string }>;

/**
 * The nine objectives of the opening chapter, in the authored order: the six of
 * the opening sequence, and the three the chapter's back half adds — the depth
 * it asks for, the optional test, and the moving current it closes on.
 *
 * The ids are read off the authored chapter rather than restated, so a chapter
 * re-authored behind this test is checked as it stands rather than as it was.
 */
const SEQUENCE = (chapter.objectives as { id: string }[]).map((held) => held.id);

function standing(id: string, state: ObjectiveState['state'] = 'active'): ObjectiveState {
  return { id, state, progress: 0, target: null, started_step: 0, completed_step: null };
}

test('every objective of the sequence has a catalog entry inside the word limit', () => {
  for (const id of SEQUENCE) {
    const entry = entries[id];
    expect(entry, id).toBeDefined();
    expect(entry.kind).toBe('objective');
    expect(entry.text.trim().split(/\s+/).length, id).toBeLessThanOrEqual(6);
    // Imperative voice, sentence case, closing period.
    expect(entry.text.endsWith('.'), id).toBe(true);
  }
});

test('every objective has its own explanation behind the optional control', () => {
  for (const id of SEQUENCE) {
    const key = explanationKey(id);
    expect(key, id).toBe(id.replace('objective.', 'explanation.'));
    expect(entries[key as string], key as string).toBeDefined();
    expect(entries[key as string].kind).toBe('explanation');
  }
});

test('one objective is shown at a time, and its text comes from the catalog', () => {
  const { rerender } = render(<Objective objective={standing(SEQUENCE[0])} />);
  expect(screen.getByText(entries[SEQUENCE[0]].text)).toBeTruthy();
  for (const other of SEQUENCE.slice(1)) {
    expect(screen.queryByText(entries[other].text)).toBeNull();
  }
  rerender(<Objective objective={standing(SEQUENCE[3])} />);
  expect(screen.getByText(entries[SEQUENCE[3]].text)).toBeTruthy();
  expect(screen.queryByText(entries[SEQUENCE[0]].text)).toBeNull();
});

test('nothing is shown before an objective is offered', () => {
  const { container } = render(<Objective objective={null} />);
  expect(container.textContent).toBe('');
  cleanup();
  const hidden = render(<Objective objective={standing('', 'hidden')} />);
  expect(hidden.container.textContent).toBe('');
});

test('the detail sits behind Why? and is reachable from the keyboard', () => {
  render(<Objective objective={standing(SEQUENCE[2])} />);
  const control = screen.getByRole('button', { name: entries['label.why'].text });
  // Nothing is shown until it is asked for, and nothing is modal: the control
  // is a button in the document's own order, so it takes focus by tabbing.
  expect(screen.queryByText(entries['explanation.the_pull.release_pulse'].text)).toBeNull();
  expect(control.getAttribute('aria-expanded')).toBe('false');
  control.focus();
  expect(document.activeElement).toBe(control);

  fireEvent.click(control);
  expect(screen.getByText(entries['explanation.the_pull.release_pulse'].text)).toBeTruthy();
  expect(control.getAttribute('aria-expanded')).toBe('true');
  fireEvent.click(control);
  expect(screen.queryByText(entries['explanation.the_pull.release_pulse'].text)).toBeNull();
});

test('the chapter back half adds no surface a player has to dismiss', () => {
  // The objectives the chapter's back half adds — the depth, the optional
  // test, and the moving current — stand in exactly the chrome the first one
  // does: one line, one optional control, and nothing that covers the field or
  // has to be dismissed before play carries on. "No opening exposition and no
  // modal tutorial" is a structural rule, so it is read structurally, on every
  // state the chapter can put the surface into.
  for (const id of SEQUENCE.slice(6)) {
    for (const state of ['active', 'failed_recoverable'] as ObjectiveState['state'][]) {
      const { container, unmount } = render(<Objective objective={standing(id, state)} />);
      const surface = container.querySelector('.objective');
      expect(surface, id).toBeTruthy();
      // One line, and one control beside it.
      expect(surface?.querySelectorAll('p').length, id).toBe(1);
      expect(container.querySelectorAll('button').length, id).toBe(1);
      // Nothing modal, by any of the shapes a modal takes.
      expect(container.querySelector('dialog'), id).toBeNull();
      expect(container.querySelector('[aria-modal]'), id).toBeNull();
      expect(container.querySelector('[role="dialog"]'), id).toBeNull();
      // And the detail is not on the surface until it is asked for.
      const key = explanationKey(id) as string;
      expect(screen.queryByText(entries[key].text), id).toBeNull();
      unmount();
    }
  }
});

test('a setback is legible without colour, and is not a failure the run ends on', () => {
  const { container } = render(<Objective objective={standing(SEQUENCE[5], 'failed_recoverable')} />);
  const surface = container.querySelector('.objective');
  expect(surface?.getAttribute('data-state')).toBe('failed_recoverable');
  // The objective still reads, and the sequence still stands on it.
  expect(screen.getByText(entries[SEQUENCE[5]].text)).toBeTruthy();
});

// ---------------------------------------------------------------------------
// Telemetry
// ---------------------------------------------------------------------------

/** A decoded snapshot carrying exactly what a mark reads. */
function snapshot(step: number, cues: number[], flow: number): FrameState {
  return {
    header: { step } as FrameState['header'],
    forms: [],
    ports: [],
    routes: flow > 0 ? ([{ route: 1, flow }] as unknown as FrameState['routes']) : [],
    currents: [],
    inside: [],
    pressures: [],
    cues: cues.map((kind) => ({ kind, name: null, a: 0, b: 0 })),
    camera: null,
  } as unknown as FrameState;
}

test('the five firsts are recorded, once each, with the time and the step', () => {
  let clock = 1_000;
  const marks: string[] = [];
  const recorder = openTelemetry({
    now: () => clock,
    onMark: (mark) => marks.push(mark),
  });
  vi.spyOn(console, 'info').mockImplementation(() => {});

  expect(recorder.reached()).toBe(0);
  clock = 1_400;
  recorder.input({ steer_x: 0, steer_y: 0, pulse_held: false, pulse_release: false, depth_key: 0 });
  expect(recorder.reached()).toBe(0);
  recorder.input({ steer_x: 900, steer_y: 0, pulse_held: false, pulse_release: false, depth_key: 0 });
  expect(recorder.marks().first_input).toEqual({ ms: 400, step: 0 });

  clock = 2_000;
  recorder.observe(snapshot(30, [1], 0));
  expect(recorder.marks().first_pulse).toEqual({ ms: 1_000, step: 30 });

  clock = 3_000;
  recorder.observe(snapshot(60, [], 12));
  expect(recorder.marks().first_route).toEqual({ ms: 2_000, step: 60 });

  clock = 4_000;
  recorder.event('objective_changed', { objective: { state: 'failed_recoverable' } });
  expect(recorder.marks().first_collapse).toEqual({ ms: 3_000, step: 60 });

  clock = 5_000;
  recorder.event('checkpoint_written', { anchor: {} });
  expect(recorder.marks().first_anchor).toEqual({ ms: 4_000, step: 60 });

  // Each mark is the first and stays it.
  clock = 9_000;
  recorder.observe(snapshot(90, [1], 12));
  recorder.event('checkpoint_written', { anchor: {} });
  expect(recorder.marks().first_pulse?.ms).toBe(1_000);
  expect(recorder.marks().first_anchor?.ms).toBe(4_000);

  expect(marks).toEqual([...TELEMETRY_MARKS]);
  expect(recorder.reached()).toBe(TELEMETRY_MARKS.length);
});

test('the telemetry reaches nothing outside the session it is measuring', () => {
  // Offline operation is locked: the game performs no network request of any
  // kind after its own files load, telemetry endpoints named among the things
  // that do not exist. Nothing here may reach for one.
  const reached: string[] = [];
  vi.stubGlobal('fetch', (target: unknown) => {
    reached.push(String(target));
    return Promise.reject(new Error('no request is made'));
  });
  vi.spyOn(console, 'info').mockImplementation(() => {});
  const recorder = openTelemetry({ now: () => 0 });
  recorder.input({ steer_x: 1, steer_y: 0, pulse_held: false, pulse_release: false, depth_key: 0 });
  recorder.observe(snapshot(1, [1], 4));
  recorder.event('checkpoint_written', { anchor: {} });
  expect(reached).toEqual([]);
  vi.unstubAllGlobals();
});
