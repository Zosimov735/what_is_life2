/**
 * The Edge's own objectives, on the surface the shell gives them.
 *
 * `app/test/objective.test.tsx` reads the opening chapter as the onboarding
 * contract. This file asks the same structural questions of the second chapter,
 * which is the first one whose objectives are about the strategy layer: one line
 * at a time, at most six words, the detail behind `Why?` and nowhere else, and
 * nothing a player has to dismiss before play carries on — including while the
 * chapter stands in a recoverable setback.
 *
 * The ids are read off the authored chapter rather than restated, so a chapter
 * re-authored behind this test is checked as it stands rather than as it was.
 */

import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, expect, test } from 'vitest';
import catalog from '../../content/copy/catalog.json';
import chapter from '../../content/chapters/the_edge.json';
import { Objective, explanationKey } from '../src/shell/Objective';
import type { ObjectiveState } from '../../worker/src/protocol';

afterEach(cleanup);

const entries = catalog.entries as Record<string, { kind: string; text: string }>;

/** The chapter's objectives, in the authored order. */
const SEQUENCE = (chapter.objectives as { id: string }[]).map((held) => held.id);

function standing(id: string, state: ObjectiveState['state'] = 'active'): ObjectiveState {
  return { id, state, progress: 0, target: null, started_step: 0, completed_step: null };
}

test('every objective of The Edge has a catalog entry inside the word limit', () => {
  expect(SEQUENCE.length).toBeGreaterThan(1);
  for (const id of SEQUENCE) {
    const entry = entries[id];
    expect(entry, id).toBeDefined();
    expect(entry.kind).toBe('objective');
    expect(entry.text.trim().split(/\s+/).length, id).toBeLessThanOrEqual(6);
    // Imperative voice, sentence case, closing period.
    expect(entry.text.endsWith('.'), id).toBe(true);
  }
});

test('every objective of The Edge has its own explanation behind the optional control', () => {
  for (const id of SEQUENCE) {
    const key = explanationKey(id) as string;
    expect(key, id).toBe(id.replace('objective.', 'explanation.'));
    expect(entries[key], key).toBeDefined();
    expect(entries[key].kind).toBe('explanation');
  }
});

test('The Edge adds no surface a player has to dismiss', () => {
  // "No opening exposition and no modal tutorial" is a structural rule, so it
  // is read structurally, on every state the chapter can put the surface into.
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

test('one objective of The Edge is shown at a time', () => {
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
