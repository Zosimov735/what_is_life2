/**
 * The Quiet Edge's chrome, read structurally — and its endings, read as copy.
 *
 * The finale adds one thing no earlier chapter had: it closes on an ending that
 * names the Form the run opened on and the continuity choice it made, so the
 * one `ending.the_quiet_edge` line becomes a family of twenty-four. That family
 * is authored copy like every other string, and the surface that shows it is the
 * same corner surface every other ending uses, so what this file checks is that
 * every key the chapter's own marks can resolve to is authored, of the right
 * kind, and renders through `ChapterReview` without a report wall.
 *
 * The ids and the marks are read off the authored chapter rather than restated
 * here, so a chapter re-authored behind this file is checked as it stands.
 */

import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, expect, test } from 'vitest';
import catalog from '../../content/copy/catalog.json';
import chapter from '../../content/chapters/the_quiet_edge.json';
import manifest from '../../content/manifest.json';
import { Objective, explanationKey } from '../src/shell/Objective';
import { ChapterReview } from '../src/shell/ChapterReview';
import type { ObjectiveState } from '../../worker/src/protocol';

afterEach(cleanup);

const entries = catalog.entries as Record<string, { kind: string; text: string }>;

/** The chapter's objectives, in the authored order. */
const SEQUENCE = (chapter.objectives as { id: string }[]).map((held) => held.id);

/** The marks the chapter's own endings are named by, in the authored order. */
const MARKS = (chapter.ending_marks as { mark: string }[]).map((held) => held.mark);

/** The Forms the manifest authors, in play order. */
const FORMS = manifest.forms as string[];

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

test('the placeholder chapter left no copy behind it', () => {
  // The chapter that stood in this slot authored two keys, and nothing refers
  // to either of them now.
  expect(entries['objective.the_quiet_edge.hold_the_current']).toBeUndefined();
  expect(entries['explanation.the_quiet_edge.hold_the_current']).toBeUndefined();
});

test('every ending the chapter can resolve to is authored, and no two are alike', () => {
  expect(MARKS.length).toBeGreaterThan(1);
  const reached: string[] = [];
  for (const form of FORMS) {
    for (const mark of MARKS) {
      const key = `${chapter.ending_key}.${form}.${mark}`;
      const entry = entries[key];
      expect(entry, key).toBeDefined();
      expect(entry.kind, key).toBe('ending');
      expect(entry.text.trim().length, key).toBeGreaterThan(0);
      reached.push(key);
    }
  }
  expect(reached.length).toBe(FORMS.length * MARKS.length);
  expect(new Set(reached).size).toBe(reached.length);
  // Distinct copy, not just distinct keys: an ending that named what this run
  // built and then said the same thing to every run would name nothing.
  const wording = reached.map((key) => entries[key].text);
  expect(new Set(wording).size).toBe(wording.length);
});

test('the ending surface shows one line and one control, whichever ending is reached', () => {
  for (const form of FORMS) {
    for (const mark of MARKS) {
      const key = `${chapter.ending_key}.${form}.${mark}`;
      const { container, unmount } = render(
        <ChapterReview
          review={null}
          ending={{ chapter_index: 7, continuation_unlocked: true, ending_id: key }}
          clearReview={() => {}}
        />,
      );
      const surface = container.querySelector('.review');
      expect(surface, key).toBeTruthy();
      expect(surface?.getAttribute('data-kind')).toBe('ending');
      // The label, the ending's own line, and nothing else until it is asked.
      expect(surface?.querySelectorAll('p').length, key).toBe(2);
      expect(container.querySelectorAll('button').length, key).toBe(1);
      expect(container.querySelector('dialog'), key).toBeNull();
      expect(container.querySelector('[aria-modal]'), key).toBeNull();
      expect(container.querySelector('[role="dialog"]'), key).toBeNull();
      expect(screen.getByText(entries[key].text), key).toBeTruthy();
      unmount();
    }
  }
});

test('the ending explains itself only when it is asked to', () => {
  const key = `${chapter.ending_key}.${FORMS[0]}.${MARKS[0]}`;
  render(
    <ChapterReview
      review={null}
      ending={{ chapter_index: 7, continuation_unlocked: true, ending_id: key }}
      clearReview={() => {}}
    />,
  );
  const control = screen.getByRole('button', { name: entries['label.why'].text });
  expect(screen.queryByText(entries['explanation.ending'].text)).toBeNull();
  fireEvent.click(control);
  expect(screen.getByText(entries['explanation.ending'].text)).toBeTruthy();
  fireEvent.click(control);
  expect(screen.queryByText(entries['explanation.ending'].text)).toBeNull();
});

test('the chapter adds no surface a player has to dismiss', () => {
  for (const id of SEQUENCE) {
    for (const state of ['active', 'failed_recoverable'] as ObjectiveState['state'][]) {
      const { container, unmount } = render(<Objective objective={standing(id, state)} />);
      const surface = container.querySelector('.objective');
      expect(surface, id).toBeTruthy();
      expect(surface?.querySelectorAll('p').length, id).toBe(1);
      expect(container.querySelectorAll('button').length, id).toBe(1);
      expect(container.querySelector('dialog'), id).toBeNull();
      expect(container.querySelector('[aria-modal]'), id).toBeNull();
      expect(container.querySelector('[role="dialog"]'), id).toBeNull();
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

test('the hands-off challenge explains itself only when it is asked to', () => {
  const last = SEQUENCE[SEQUENCE.length - 1];
  expect(last).toBe('objective.the_quiet_edge.let_it_run');
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

test('the setback an unclosed arm stands in is legible without colour', () => {
  const held = 'objective.the_quiet_edge.close_the_ring';
  expect(SEQUENCE).toContain(held);
  const { container } = render(<Objective objective={standing(held, 'failed_recoverable')} />);
  const surface = container.querySelector('.objective');
  expect(surface?.getAttribute('data-state')).toBe('failed_recoverable');
  expect(screen.getByText(entries[held].text)).toBeTruthy();
});
