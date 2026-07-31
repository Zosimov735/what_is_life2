/**
 * The standing pressure's own line: the concise explanation beside the
 * renderer's direct cues.
 *
 * What is under test is the same chrome discipline the objective keeps: one
 * line, every string from the catalog, detail behind the optional control and
 * nowhere else, nothing modal, and nothing at all for most of a run — a
 * queued pressure and an empty list both show nothing.
 */

import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, expect, test } from 'vitest';
import catalog from '../../content/copy/catalog.json';
import { PressureLine, surfaced } from '../src/shell/PressureLine';
import { PRESSURE_IDS, type PressureState } from '../../../field_game/worker/src/protocol';

afterEach(cleanup);

const entries = catalog.entries as Record<string, { kind: string; text: string }>;

function standing(
  pressure: PressureState['pressure'],
  stage: PressureState['stage'] = 'pressure',
  held: Partial<PressureState> = {},
): PressureState {
  return {
    pressure,
    stage,
    level: 30_000,
    primary: false,
    queued: false,
    start_step: 0,
    target: { t: 'none', id: null },
    displaced: null,
    bound: null,
    ...held,
  };
}

test('every pressure of the closed set has its name and its explanation in the catalog', () => {
  for (const id of PRESSURE_IDS) {
    const name = entries[`pressure.${id}`];
    expect(name, id).toBeDefined();
    expect(name.kind).toBe('pressure');
    const explanation = entries[`explanation.pressure_${id}`];
    expect(explanation, id).toBeDefined();
    expect(explanation.kind).toBe('explanation');
  }
});

test('an empty list and a queued pressure both show nothing', () => {
  const { container } = render(<PressureLine pressures={[]} />);
  expect(container.textContent).toBe('');
  cleanup();

  const waiting = render(
    <PressureLine pressures={[standing('interference', 'signal', { queued: true })]} />,
  );
  expect(waiting.container.textContent).toBe('');
});

test('an active pressure is named from the catalog, and the detail waits behind the control', () => {
  render(<PressureLine pressures={[standing('interference', 'crisis')]} />);
  expect(screen.getByText('Interference')).toBeTruthy();

  // The detail is not shown until asked for, and the asking is a button — no
  // modal, nothing covering the surface.
  const detail = entries['explanation.pressure_interference'].text;
  expect(screen.queryByText(detail)).toBeNull();
  const why = screen.getByRole('button', { name: entries['label.why'].text });
  expect(why.getAttribute('aria-expanded')).toBe('false');
  fireEvent.click(why);
  expect(screen.getByText(detail)).toBeTruthy();
  expect(why.getAttribute('aria-expanded')).toBe('true');
  // And it closes the same way.
  fireEvent.click(why);
  expect(screen.queryByText(detail)).toBeNull();
});

test('the primary pressure is the one surfaced, else the furthest advanced', () => {
  // The primary wins, whatever stage the other stands at.
  const primary = surfaced([
    standing('drain', 'crisis'),
    standing('interference', 'signal', { primary: true }),
  ]);
  expect(primary?.pressure).toBe('interference');

  // With no primary, the furthest-advanced active pressure is surfaced.
  const advanced = surfaced([standing('drain', 'signal'), standing('flood', 'crisis')]);
  expect(advanced?.pressure).toBe('flood');

  // A queued pressure is never surfaced, even primary: a seat requested is
  // not a pressure standing.
  const queuedPrimary = surfaced([
    standing('drain', 'pressure'),
    standing('interference', 'signal', { queued: true, primary: true }),
  ]);
  expect(queuedPrimary?.pressure).toBe('drain');
  expect(surfaced([standing('noise', 'signal', { queued: true })])).toBeNull();
});

test('one line at a time: two active pressures surface one name', () => {
  render(
    <PressureLine
      pressures={[standing('drain', 'pressure'), standing('fracture', 'crisis')]}
    />,
  );
  expect(screen.getByText('Fracture')).toBeTruthy();
  expect(screen.queryByText('Drain')).toBeNull();
  expect(screen.getAllByRole('status')).toHaveLength(1);
});
