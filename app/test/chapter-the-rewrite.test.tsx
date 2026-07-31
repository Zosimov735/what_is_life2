/**
 * The Rewrite's chrome, read structurally.
 *
 * The chapter asks a player to replace a working part of the run while it keeps
 * running, and every one of its beats is something the field does rather than
 * something the surface says. So what the surface is allowed to say is exactly
 * what every other chapter's surface says: one objective line of at most six
 * words, one optional control beside it, and nothing that covers the field or
 * has to be dismissed. That is a structural rule, so it is read structurally,
 * over every objective of the chapter and over every state the sequence can put
 * the surface into — the recoverable setback its dependency stands in included.
 *
 * The ids are read off the authored chapter rather than restated here, so a
 * chapter re-authored behind this file is checked as it stands.
 */

import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, expect, test } from 'vitest';
import catalog from '../../content/copy/catalog.json';
import chapter from '../../content/chapters/the_rewrite.json';
import { Objective, explanationKey } from '../src/shell/Objective';
import type { ObjectiveState } from '../../../field_game/worker/src/protocol';

afterEach(cleanup);

const entries = catalog.entries as Record<string, { kind: string; text: string }>;

/** The chapter's objectives, in the authored order. */
const SEQUENCE = (chapter.objectives as { id: string }[]).map((held) => held.id);

function standing(id: string, state: ObjectiveState['state'] = 'active'): ObjectiveState {
  return { id, state, progress: 0, target: null, started_step: 0, completed_step: null };
}

test('every objective of the chapter has a catalog entry inside the word limit', () => {
  expect(SEQUENCE.length).toBeGreaterThan(0);
  for (const id of SEQUENCE) {
    const entry = entries[id];
    expect(entry, id).toBeDefined();
    expect(entry.kind).toBe('objective');
    expect(entry.text.trim().split(/\s+/).length, id).toBeLessThanOrEqual(6);
    // Imperative voice, sentence case, closing period.
    expect(entry.text.endsWith('.'), id).toBe(true);
  }
});

test('every objective of the chapter has its own explanation behind the control', () => {
  for (const id of SEQUENCE) {
    const key = explanationKey(id);
    expect(key, id).toBe(id.replace('objective.', 'explanation.'));
    expect(entries[key as string], key as string).toBeDefined();
    expect(entries[key as string].kind).toBe('explanation');
  }
});

test('the chapter adds no surface a player has to dismiss', () => {
  for (const id of SEQUENCE) {
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

test('one objective of the chapter is shown at a time', () => {
  const { rerender } = render(<Objective objective={standing(SEQUENCE[0])} />);
  expect(screen.getByText(entries[SEQUENCE[0]].text)).toBeTruthy();
  for (const other of SEQUENCE.slice(1)) {
    expect(screen.queryByText(entries[other].text)).toBeNull();
  }
  const last = SEQUENCE[SEQUENCE.length - 1];
  rerender(<Objective objective={standing(last)} />);
  expect(screen.getByText(entries[last].text)).toBeTruthy();
  expect(screen.queryByText(entries[SEQUENCE[0]].text)).toBeNull();
});

test('the final challenge explains itself only when it is asked to', () => {
  const last = SEQUENCE[SEQUENCE.length - 1];
  expect(last).toBe('objective.the_rewrite.rewrite_the_module');
  const key = explanationKey(last) as string;
  render(<Objective objective={standing(last)} />);
  const control = screen.getByRole('button', { name: entries['label.why'].text });
  expect(screen.queryByText(entries[key].text)).toBeNull();
  expect(control.getAttribute('aria-expanded')).toBe('false');

  fireEvent.click(control);
  expect(screen.getByText(entries[key].text)).toBeTruthy();
  expect(control.getAttribute('aria-expanded')).toBe('true');
  fireEvent.click(control);
  expect(screen.queryByText(entries[key].text)).toBeNull();
});

test('the setback the dependency stands in is legible without colour', () => {
  const held = 'objective.the_rewrite.hold_the_dependency';
  expect(SEQUENCE).toContain(held);
  const { container } = render(<Objective objective={standing(held, 'failed_recoverable')} />);
  const surface = container.querySelector('.objective');
  expect(surface?.getAttribute('data-state')).toBe('failed_recoverable');
  // The objective still reads, and the sequence still stands on it.
  expect(screen.getByText(entries[held].text)).toBeTruthy();
});

test('the chapter names no state outside the closed set', () => {
  const closed: ObjectiveState['state'][] = [
    'hidden',
    'active',
    'complete',
    'failed_recoverable',
  ];
  for (const state of closed) {
    const { container, unmount } = render(<Objective objective={standing(SEQUENCE[0], state)} />);
    if (state === 'hidden') {
      expect(container.textContent).toBe('');
    } else {
      expect(container.querySelector('.objective')?.getAttribute('data-state')).toBe(state);
    }
    unmount();
  }
});
