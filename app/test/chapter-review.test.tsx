/**
 * The chapter review and the campaign's ending, as chrome.
 *
 * What is under test is the division the goal turns on: the shell renders what
 * the worker reported and holds no chapter rule of its own. So the tests drive
 * the two events — `chapter_changed` and `run_completed` — through the client
 * and read what the surface shows, and they check that every string it shows
 * came from the catalog by a key the worker named.
 */

import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, expect, test } from 'vitest';
import catalog from '../../content/copy/catalog.json';
import { ChapterReview } from '../src/shell/ChapterReview';
import type { ChapterChanged, RunCompleted } from '../../worker/src/protocol';

afterEach(cleanup);

const entries = catalog.entries as Record<string, { kind: string; text: string }>;

/** The eight chapters of the closed set, as their catalog keys. */
const CHAPTERS = [
  'the_pull',
  'the_edge',
  'the_loop',
  'the_echo',
  'the_mesh',
  'the_break',
  'the_rewrite',
  'the_quiet_edge',
];

function closed(index: number): ChapterChanged {
  return { chapter_index: index, title_key: `chapter.${CHAPTERS[index]}` };
}

function ended(index: number): RunCompleted {
  return {
    ending_id: `ending.${CHAPTERS[index]}`,
    chapter_index: index,
    continuation_unlocked: true,
  };
}

test('every chapter of the campaign carries a name and an ending in the catalog', () => {
  for (const id of CHAPTERS) {
    const name = entries[`chapter.${id}`];
    expect(name, id).toBeDefined();
    expect(name.kind).toBe('chapter');
    const ending = entries[`ending.${id}`];
    expect(ending, id).toBeDefined();
    expect(ending.kind).toBe('ending');
  }
});

test('nothing stands before a chapter has closed', () => {
  const { container } = render(
    <ChapterReview review={null} ending={null} clearReview={() => {}} />,
  );
  expect(container.querySelector('.review')).toBeNull();
});

test('the review names the chapter that closed, from the catalog', () => {
  render(<ChapterReview review={closed(0)} ending={null} clearReview={() => {}} />);
  expect(screen.getByText(entries['label.chapter_review'].text)).toBeTruthy();
  expect(screen.getByText(entries['chapter.the_pull'].text)).toBeTruthy();
  // The detail is behind the optional control and nowhere else.
  expect(screen.queryByText(entries['explanation.chapter_review'].text)).toBeNull();
  fireEvent.click(screen.getByRole('button', { name: entries['label.why'].text }));
  expect(screen.getByText(entries['explanation.chapter_review'].text)).toBeTruthy();
});

test('the review is let go of by its own control', () => {
  let cleared = 0;
  render(
    <ChapterReview
      review={closed(2)}
      ending={null}
      clearReview={() => {
        cleared += 1;
      }}
    />,
  );
  expect(screen.getByText(entries['chapter.the_loop'].text)).toBeTruthy();
  fireEvent.click(screen.getByRole('button', { name: entries['label.continue'].text }));
  expect(cleared).toBe(1);
});

test('the ending stands in place of the review, on the key the worker named', () => {
  render(<ChapterReview review={closed(6)} ending={ended(7)} clearReview={() => {}} />);
  expect(screen.getByText(entries['label.ending'].text)).toBeTruthy();
  expect(screen.getByText(entries['ending.the_quiet_edge'].text)).toBeTruthy();
  // One surface at a time: the chapter review does not stand beside it.
  expect(screen.queryByText(entries['label.chapter_review'].text)).toBeNull();
  expect(screen.queryByText(entries['chapter.the_rewrite'].text)).toBeNull();
});

test('the review shows no reading, no ranking, and no objective list', () => {
  const { container } = render(
    <ChapterReview review={closed(1)} ending={null} clearReview={() => {}} />,
  );
  const shown = container.textContent ?? '';
  // No figure of any kind: a review is not a report wall, and nothing about a
  // completed chapter is folded into a number.
  expect(/\d/.test(shown)).toBe(false);
  // And no objective is stacked beside another: one objective is visible at a
  // time, and the objective line is the surface that shows it.
  for (const [key, entry] of Object.entries(entries)) {
    if (entry.kind !== 'objective') continue;
    expect(shown.includes(entry.text), key).toBe(false);
  }
});
