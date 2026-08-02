/**
 * The opening selection: the eight Forms, their promises, and what choosing
 * one does.
 *
 * Every string the surface shows is compared against the authored catalog
 * bytes rather than written here, so an assertion that passes means the surface
 * read the catalog rather than that the two happen to agree.
 */

import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, expect, test, vi } from 'vitest';
import catalog from '../../content/copy/catalog.json';
import { FORM_IDS, PROTOCOL_VERSION, type FormId } from '../../worker/src/protocol';
import type { CommandEnvelope, ResponseEnvelope } from '../../worker/src/protocol';
import { App } from '../src/shell/App';
import { FormSelect } from '../src/shell/FormSelect';
import type { CoreClient } from '../src/shell/worker-client';

/**
 * A 2D context that answers everything and draws nothing. The test environment
 * carries no canvas implementation, and the surface behind the selection mounts
 * one as soon as a Form is taken.
 */
function quietContext(): CanvasRenderingContext2D {
  const held: Record<string, unknown> = {};
  const gradient = { addColorStop: () => {} };
  return new Proxy(held, {
    get(target, property) {
      const name = String(property);
      if (name === 'createRadialGradient' || name === 'createLinearGradient') {
        return () => gradient;
      }
      return name in target ? target[name] : () => undefined;
    },
    set(target, property, value) {
      target[String(property)] = value;
      return true;
    },
  }) as unknown as CanvasRenderingContext2D;
}

beforeEach(() => {
  const quiet = quietContext();
  vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockImplementation(((kind: string) =>
    kind === '2d' ? quiet : null) as typeof HTMLCanvasElement.prototype.getContext);
});

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

/** A worker that records that it was started, and answers nothing. */
class RecordingWorker {
  static opened: RecordingWorker[] = [];
  onmessage: ((message: MessageEvent<ResponseEnvelope>) => void) | null = null;
  onerror: ((failure: unknown) => void) | null = null;
  readonly sent: CommandEnvelope[] = [];

  constructor() {
    RecordingWorker.opened.push(this);
  }

  postMessage(command: CommandEnvelope): void {
    this.sent.push(command);
  }

  terminate(): void {}
}

/** The authored name and promise of one Form. */
function named(form: FormId): { name: string; promise: string } {
  const entries = catalog.entries as Record<string, { kind: string; text: string }>;
  return { name: entries[`form.${form}`].text, promise: entries[`promise.${form}`].text };
}

/** A client that answers nothing, for a shell under test past the selection. */
function stubClient(): CoreClient {
  const ready: Promise<ResponseEnvelope> = Promise.resolve({
    v: PROTOCOL_VERSION,
    re: 1,
    ok: true,
    body: {},
  });
  return {
    ready,
    command: async () => ({ v: PROTOCOL_VERSION, re: 0, ok: true, body: {} }),
    snapshot: () => null,
    frames: () => ({ previous: null, next: null, alpha: 0 }),
    pause: () => {},
    step: async () => ({ seq: 0, steps_run: 0, remainder_us: 0, dropped: false }),
    held: () => null,
    restarts: () => 0,
    recovering: () => false,
    inflight: () => 0,
    notices: () => [],
    objective: () => null,
    pressures: () => [],
    mode: () => null,
    queue: () => ({ entries: [], cost_total: 0, impulse: 0, impulse_after: 0 }),
    slate: () => null,
    profile: () => null,
    perturbation: () => null,
    echo: () => null,
    clearEcho: () => {},
    chapter: () => null,
    review: () => null,
    clearReview: () => {},
    ending: () => null,
    inspect: () => {},
    queuePlan: async () => ({ v: PROTOCOL_VERSION, re: 0, ok: true, body: {} }),
    undoPlan: async () => ({ v: PROTOCOL_VERSION, re: 0, ok: true, body: {} }),
    telemetry: () => ({}),
    watch: () => () => {},
    close: () => {},
  };
}

test('the eight Forms stand in the closed set order, each with one promise', () => {
  render(<FormSelect onChoose={() => {}} />);

  // The group is what it behaves as: one choice to be made, one tab stop, the
  // arrows moving inside it.
  expect(screen.getByRole('radiogroup')).toBeTruthy();

  // The instruction is the catalog's, and it is the only line that is not a
  // Form: no opening exposition stands in front of the choice.
  expect(screen.getByText(catalog.entries['instruction.choose_form'].text)).toBeTruthy();

  const offered = screen.getAllByRole('radio');
  expect(offered).toHaveLength(FORM_IDS.length);
  FORM_IDS.forEach((form, place) => {
    const { name, promise } = named(form);
    // The order is the closed set's own, read off the surface rather than
    // asserted about the data.
    expect(offered[place].textContent).toBe(`${name}${promise}`);
    expect(screen.getByText(name)).toBeTruthy();
    expect(screen.getByText(promise)).toBeTruthy();
  });
});

test('no Form is offered as the one to take', () => {
  const { container } = render(<FormSelect onChoose={() => {}} />);

  for (const offered of screen.getAllByRole('radio')) {
    // Nothing marks a Form as chosen, current, recommended, or unavailable.
    expect(offered.getAttribute('aria-selected')).toBeNull();
    expect(offered.getAttribute('aria-current')).toBeNull();
    expect(offered.getAttribute('aria-pressed')).toBeNull();
    // A radio group with nothing checked: the choice has not been made, and no
    // option stands as the one to take.
    expect(offered.getAttribute('aria-checked')).toBe('false');
    expect((offered as HTMLButtonElement).disabled).toBe(false);
    expect(offered.className).toBe('opening-form');
  }

  // Quantified chassis promises may name their limits, but none receives an
  // editorial badge or selection state that makes it the prescribed choice.
  expect(container.querySelector('[data-recommended], [data-current]')).toBeNull();
});

test('the arrow keys move through the Forms, and one tab stop holds them all', () => {
  render(<FormSelect onChoose={() => {}} />);
  const offered = screen.getAllByRole('radio') as HTMLButtonElement[];
  const stops = () => offered.filter((held) => held.tabIndex === 0);

  // One tab stop for the group, which is what keeps the eight from standing
  // between the page and the first playable frame.
  expect(stops()).toHaveLength(1);
  expect(stops()[0]).toBe(offered[0]);

  const list = offered[0].closest('ul') as HTMLUListElement;
  fireEvent.keyDown(list, { key: 'ArrowDown' });
  expect(document.activeElement).toBe(offered[1]);
  expect(stops()).toEqual([offered[1]]);

  fireEvent.keyDown(list, { key: 'ArrowRight' });
  fireEvent.keyDown(list, { key: 'ArrowUp' });
  expect(document.activeElement).toBe(offered[1]);

  fireEvent.keyDown(list, { key: 'End' });
  expect(document.activeElement).toBe(offered[FORM_IDS.length - 1]);
  // The ends hold: the list does not wrap past either of them.
  fireEvent.keyDown(list, { key: 'ArrowDown' });
  expect(document.activeElement).toBe(offered[FORM_IDS.length - 1]);
  fireEvent.keyDown(list, { key: 'Home' });
  expect(document.activeElement).toBe(offered[0]);
  fireEvent.keyDown(list, { key: 'ArrowUp' });
  expect(document.activeElement).toBe(offered[0]);
});

test('a Form is taken by keyboard as well as by pointer', () => {
  const taken: FormId[] = [];
  render(<FormSelect onChoose={(form) => taken.push(form)} />);
  const offered = screen.getAllByRole('radio') as HTMLButtonElement[];
  const list = offered[0].closest('ul') as HTMLUListElement;

  fireEvent.keyDown(list, { key: 'ArrowDown' });
  fireEvent.keyDown(list, { key: 'ArrowDown' });
  // The Forms are buttons, so Enter and Space activate them without anything
  // here reading a key.
  (document.activeElement as HTMLButtonElement).click();
  expect(taken).toEqual([FORM_IDS[2]]);

  fireEvent.click(offered[FORM_IDS.length - 1]);
  expect(taken).toEqual([FORM_IDS[2], FORM_IDS[FORM_IDS.length - 1]]);
});

test('no session is opened until a Form is taken, and it opens on that Form', async () => {
  const opened: FormId[] = [];
  const client = stubClient();
  const { container } = render(
    <App
      open={(form) => {
        opened.push(form);
        return client;
      }}
      sound={null}
    />,
  );

  // Nothing is started while the choice stands: the Form is part of
  // `init_run`, so a session opened before the choice would be a run opened on
  // a Form nobody named.
  expect(opened).toEqual([]);
  expect(container.querySelector('canvas')).toBeNull();

  const { name } = named('vault');
  await act(async () => {
    screen.getByRole('radio', { name: new RegExp(name) }).click();
  });

  expect(opened).toEqual(['vault']);
  // And the selection is gone once it is answered: the surface stands before
  // the run and never over it.
  expect(screen.queryByText(catalog.entries['instruction.choose_form'].text)).toBeNull();
  expect(container.querySelector('canvas')).not.toBeNull();
});

test('the choice opens one session, and a redrawn surface does not open another', async () => {
  // The shell the game runs takes no `open`: it uses its own, and the effect
  // that starts a session names that function among the things it watches. A
  // function rebuilt on every render would therefore end the run and open a
  // new one every time anything on the surface changed, which is a whole
  // worker and a whole `init_run` per redraw. One worker is the assertion.
  RecordingWorker.opened.length = 0;
  vi.stubGlobal('Worker', RecordingWorker);
  vi.spyOn(console, 'info').mockImplementation(() => {});

  render(<App sound={null} />);
  await act(async () => {
    screen.getByRole('radio', { name: new RegExp(named('thread').name) }).click();
  });

  expect(RecordingWorker.opened).toHaveLength(1);
  // And it is opened on the Form that was taken, once.
  const started = RecordingWorker.opened[0].sent.filter((held) => held.cmd === 'init_run');
  expect(started).toHaveLength(1);
  expect(started[0].body.form).toBe('thread');
});
