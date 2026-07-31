/**
 * The optional coordinate-profile surface, and the Echo.
 *
 * Three of the goal's done-when clauses are pinned here: profiles are
 * inspectable **optionally**, which means default-hidden and asked for rather
 * than pushed; ordinary play carries **no numerical dashboard**, which means
 * the surface renders nothing at all while the Field is moving; and a committed
 * change leaves **one short causal highlight**, from the catalog, naming the
 * cause plainly and carrying no number.
 */

import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, expect, test, vi } from 'vitest';
import catalog from '../../content/copy/catalog.json';
import { Inspect, nextControlled, offeredPlayback, playbackOf } from '../src/shell/Inspect';
import type {
  CoordinateProfile,
  EchoHighlight,
  InspectRequest,
  PerturbationResult,
} from '../../../field_game/worker/src/protocol';
import type { FrameState } from '../../../field_game/worker/src/frame-state';

afterEach(cleanup);

const entries = catalog.entries as Record<string, { kind: string; text: string }>;

/** The raw form of 1. */
const WHOLE = 65536;

/** A reading that carries a number. */
function at(value: number) {
  return { value, reason: null };
}

/** A reading the record could not take. */
function absent(reason: string) {
  return { value: null, reason };
}

function profileOf(overrides: Partial<CoordinateProfile> = {}): CoordinateProfile {
  return {
    view: { inside: [2, 3, 4], resolution: 1, window: 8, surround: 'adjacent' },
    step: 8,
    swap_range: at(3),
    self_support: at(WHOLE),
    throughput: {
      in_rate: 5 * WHOLE,
      out_rate: 4 * WHOLE,
      routes: [{ route: 1, rate: 5 * WHOLE }],
      shell: [{ node: 2, rate: 0 }],
    },
    upkeep_mix: { value: null, reason: 'no-upkeep' },
    reach: at(600 * WHOLE),
    input_resolution: at(2),
    horizon: at(1),
    source_trace: at(0),
    instruction_separation: null,
    turnover_tolerance: null,
    ...overrides,
  } as CoordinateProfile;
}

function echoOf(kind: EchoHighlight['kind']): EchoHighlight {
  return {
    kind,
    parameter: 2,
    excess: 28087,
    low: 28087,
    high: 28087,
    target: { t: 'route', id: 2 },
  };
}

// ---------------------------------------------------------------------------
// Optional, and default hidden
// ---------------------------------------------------------------------------

test('the surface renders nothing at all while the run is moving', () => {
  const inspect = vi.fn();
  const { container } = render(
    <Inspect mode="running" profile={profileOf()} echo={null} inspect={inspect} />,
  );
  // Ordinary play carries no numerical dashboard: not a control, not a panel,
  // and not a number — even with a profile already in hand.
  expect(container.querySelector('.inspect')).toBeNull();
  expect(container.textContent).toBe('');
  expect(inspect).not.toHaveBeenCalled();
});

test('the profile is hidden until the optional control is opened', () => {
  const inspect = vi.fn();
  const { container } = render(
    <Inspect mode="still" profile={profileOf()} echo={null} inspect={inspect} />,
  );
  // The control stands; the panel does not, and nothing has been asked for.
  const open = screen.getByRole('button', { name: entries['label.coordinate_profile'].text });
  expect(open.getAttribute('aria-expanded')).toBe('false');
  expect(container.querySelector('.inspect-panel')).toBeNull();
  expect(inspect).not.toHaveBeenCalled();
  // No coordinate number is on the page while it is closed.
  expect(screen.queryByText(entries['label.coord_swap_range'].text)).toBeNull();

  fireEvent.click(open);
  expect(open.getAttribute('aria-expanded')).toBe('true');
  expect(container.querySelector('.inspect-panel')).toBeTruthy();
  // Opening it is what asks for a reading: nothing requests one on its own.
  expect(inspect).toHaveBeenCalledWith({
    target: 'coordinates',
    kind: null,
    parameter: null,
  } satisfies InspectRequest);
});

test('the control is a button, so it is reachable by keyboard without focus management', () => {
  render(<Inspect mode="still" profile={profileOf()} echo={null} inspect={() => {}} />);
  const open = screen.getByRole('button', { name: entries['label.coordinate_profile'].text });
  expect(open.tagName).toBe('BUTTON');
  expect(open.getAttribute('type')).toBe('button');
  expect(open.hasAttribute('tabindex')).toBe(false);
});

// ---------------------------------------------------------------------------
// What the panel shows
// ---------------------------------------------------------------------------

test('the panel shows the ten readings separately and folds none of them together', () => {
  const { container } = render(
    <Inspect mode="still" profile={profileOf()} echo={null} inspect={() => {}} />,
  );
  fireEvent.click(screen.getByRole('button', { name: entries['label.coordinate_profile'].text }));
  for (const key of [
    'label.coord_swap_range',
    'label.coord_self_support',
    'label.coord_throughput_in',
    'label.coord_throughput_out',
    'label.coord_upkeep_mix',
    'label.coord_reach',
    'label.coord_input_resolution',
    'label.coord_horizon',
    'label.coord_source_trace',
    'label.coord_instruction_separation',
    'label.coord_turnover_tolerance',
  ]) {
    expect(screen.getByText(entries[key].text), key).toBeTruthy();
  }
  // Eleven rows for the ten readings — Throughput is one coordinate holding
  // two magnitudes, so it takes two rows — and no twelfth: there is no row for
  // a figure that stands for the profile as a whole, because there is no such
  // figure in the record.
  expect(container.querySelectorAll('.inspect-row').length).toBe(11);
  // The panel is announced when it changes, exactly as the tray's own status
  // regions are.
  const panel = container.querySelector('.inspect-panel');
  expect(panel?.getAttribute('role')).toBe('status');
  expect(panel?.getAttribute('aria-live')).toBe('polite');
});

test('an unassigned reading is drawn as an absence with its reason, never as a zero', () => {
  const { container } = render(
    <Inspect
      mode="still"
      profile={profileOf({ horizon: absent('window-too-short') })}
      echo={null}
      inspect={() => {}}
    />,
  );
  fireEvent.click(screen.getByRole('button', { name: entries['label.coordinate_profile'].text }));
  const rows = [...container.querySelectorAll('.inspect-row')];
  const horizon = rows.find((row) =>
    row.textContent?.startsWith(entries['label.coord_horizon'].text),
  );
  expect(horizon?.getAttribute('data-assigned')).toBe('false');
  expect(horizon?.textContent).toContain(entries['label.value_unassigned'].text);
  expect(horizon?.querySelector('.inspect-value')?.getAttribute('data-reason')).toBe(
    'window-too-short',
  );
  // The two replay-based readings stand null until they are asked for, and
  // read as absences rather than zeros while they do.
  const separation = rows.find((row) =>
    row.textContent?.startsWith(entries['label.coord_instruction_separation'].text),
  );
  expect(separation?.getAttribute('data-assigned')).toBe('false');
});

test('the replay-based coordinates are a second request, not part of the first', () => {
  const inspect = vi.fn();
  render(<Inspect mode="still" profile={profileOf()} echo={null} inspect={inspect} />);
  fireEvent.click(screen.getByRole('button', { name: entries['label.coordinate_profile'].text }));
  inspect.mockClear();
  fireEvent.click(screen.getByRole('button', { name: entries['label.coordinate_replays'].text }));
  expect(inspect).toHaveBeenCalledWith({
    target: 'coordinates_full',
    kind: null,
    parameter: null,
  } satisfies InspectRequest);
});

// ---------------------------------------------------------------------------
// The Echo
// ---------------------------------------------------------------------------

test('a committed change leaves one short line from the catalog, and no number', () => {
  const { container } = render(
    <Inspect mode="running" profile={null} echo={echoOf('route-removal')} inspect={() => {}} />,
  );
  const echo = container.querySelector('.echo');
  expect(echo?.textContent).toBe(entries['notice.echo_route_removal'].text);
  // One line, never a report wall, and never a reading: the record carries the
  // excess and its range, and the surface names the cause.
  expect(container.querySelectorAll('.echo').length).toBe(1);
  expect(echo?.textContent).not.toMatch(/[0-9]/);
  expect(echo?.getAttribute('role')).toBe('status');
});

test('each Echo kind the branch can raise has its own catalog line', () => {
  for (const kind of [
    'route-removal',
    'boundary-severance',
    'component-substitution',
    'evaluation',
  ] as EchoHighlight['kind'][]) {
    cleanup();
    const { container } = render(
      <Inspect mode="running" profile={null} echo={echoOf(kind)} inspect={() => {}} />,
    );
    expect(container.querySelector('.echo')?.textContent, kind).toBeTruthy();
  }
});

test('an Echo of a kind the branch does not raise shows nothing rather than a wrong line', () => {
  const { container } = render(
    <Inspect mode="running" profile={null} echo={echoOf('window-change')} inspect={() => {}} />,
  );
  expect(container.querySelector('.echo')).toBeNull();
});

test('the Echo is let go of when the next pause begins', () => {
  const clearEcho = vi.fn();
  const { rerender } = render(
    <Inspect
      mode="running"
      profile={null}
      echo={echoOf('route-removal')}
      inspect={() => {}}
      clearEcho={clearEcho}
    />,
  );
  expect(clearEcho).not.toHaveBeenCalled();
  rerender(
    <Inspect
      mode="ramp_in"
      profile={null}
      echo={echoOf('route-removal')}
      inspect={() => {}}
      clearEcho={clearEcho}
    />,
  );
  expect(clearEcho).toHaveBeenCalledTimes(1);
});

test('every string the surface shows comes from the catalog', () => {
  for (const key of [
    'label.coordinate_profile',
    'label.coordinate_replays',
    'label.coord_throughput_in',
    'label.coord_throughput_out',
    'label.value_unassigned',
    'explanation.coordinate_profile',
    'notice.echo_route_removal',
    'notice.echo_boundary_severance',
    'notice.echo_component_substitution',
    'notice.echo_evaluation',
  ]) {
    expect(entries[key], key).toBeDefined();
  }
  // The Echo lines are notices, which the catalog holds to being short, and
  // each ends with a period like every other authored sentence.
  for (const key of Object.keys(entries)) {
    if (!key.startsWith('notice.echo_')) continue;
    expect(entries[key].kind, key).toBe('notice');
    expect(entries[key].text.endsWith('.'), key).toBe(true);
    expect(entries[key].text.split(/\s+/).length, key).toBeLessThanOrEqual(8);
  }
});

// ---------------------------------------------------------------------------
// The playback offer
// ---------------------------------------------------------------------------

function resultOf(samples: PerturbationResult['samples']): PerturbationResult {
  return {
    view: { inside: [2, 3, 4], resolution: 1, window: 8, surround: 'adjacent' },
    provenance: [],
    position: 0,
    sigma: { key: '0', ctr: '0', half: 0 },
    streams: [],
    kind: 'route-removal',
    parameter: 2,
    tau: 8192,
    reading: { value: 100, low: 0, high: 200, samples: 8, reason: null },
    samples,
    recomputed: null,
    step: 8,
  } as unknown as PerturbationResult;
}

function sampleOf(
  excess: number | null,
  series: number[],
  base: number[] | null = null,
): PerturbationResult['samples'][number] {
  return { deviation: excess, excess, series, base_deviation: null, base_series: base };
}

test('the played sample is the largest excess, smallest number on ties', () => {
  const result = resultOf([
    sampleOf(100, [1, 2, 3]),
    sampleOf(300, [4, 5, 6], [1, 1, 1]),
    sampleOf(300, [7, 8, 9]),
    sampleOf(null, [0, 0, 0]),
  ]);
  const reading = playbackOf(result);
  // Samples 2 and 3 tie at the largest excess; the smaller number plays — the
  // sample the Echo already names — and the reading carries its own base.
  expect(reading?.series).toEqual([4, 5, 6]);
  expect(reading?.base).toEqual([1, 1, 1]);
  expect(reading?.members).toEqual([2, 3, 4]);
});

test('a result with no excess offers no playback', () => {
  expect(playbackOf(resultOf([sampleOf(null, [1, 2, 3])]))).toBeNull();
  expect(playbackOf(null)).toBeNull();
});

test('the offer stands in still and the ramps, and running offers null', () => {
  const result = resultOf([sampleOf(100, [1, 2, 3])]);
  // The gate is structural: whatever record the shell holds, a moving Field
  // is offered nothing at all — the goal's own no-dashboard rule read as an
  // assertion on the offer rather than a judgement about layout.
  expect(offeredPlayback('running', result)).toBeNull();
  expect(offeredPlayback(null, result)).toBeNull();
  expect(offeredPlayback('still', result)?.series).toEqual([1, 2, 3]);
  expect(offeredPlayback('ramp_in', result)).not.toBeNull();
  expect(offeredPlayback('ramp_out', result)).not.toBeNull();
});

// ---------------------------------------------------------------------------
// The Handoff
// ---------------------------------------------------------------------------

/** One Form of a decoded snapshot, holding nothing down. */
function formAt(id: number, controlled: boolean): FrameState['forms'][number] {
  return {
    id,
    formOrdinal: 0,
    layer: 0,
    controlled,
    focus: false,
    pulseCharging: false,
    separated: false,
    x: 2_048,
    y: 2_048,
    vx: 0,
    vy: 0,
    charge: 0,
    radius: 0,
  };
}

test('the Handoff control stands only where a run holds several Forms', () => {
  const one = render(
    <Inspect
      mode="still"
      profile={null}
      echo={null}
      inspect={() => {}}
      forms={[formAt(1, true)]}
    />,
  );
  expect(one.container.querySelector('.handoff')).toBeNull();
  cleanup();

  const several = render(
    <Inspect
      mode="still"
      profile={null}
      echo={null}
      inspect={() => {}}
      forms={[formAt(1, true), formAt(3, false)]}
    />,
  );
  expect(several.container.querySelector('.handoff')).toBeTruthy();
});

test('the Handoff control is a button, and moving control is what it asks for', () => {
  const inspect = vi.fn();
  render(
    <Inspect
      mode="still"
      profile={null}
      echo={null}
      inspect={inspect}
      forms={[formAt(1, true), formAt(3, false), formAt(5, false)]}
    />,
  );
  const move = screen.getByRole('button', { name: entries['label.handoff'].text });
  // Keyboard reachable because it is a button, exactly as the profile control
  // is: nothing here manages focus.
  expect(move.tagName).toBe('BUTTON');
  expect(move.getAttribute('type')).toBe('button');
  expect(move.hasAttribute('tabindex')).toBe(false);
  // Nothing is asked for until it is pressed.
  expect(inspect).not.toHaveBeenCalled();

  fireEvent.click(move);
  expect(inspect).toHaveBeenCalledWith({
    target: 'handoff',
    kind: null,
    parameter: 3,
  } satisfies InspectRequest);
});

test('the Handoff walks the Forms in ascending order and wraps past the end', () => {
  expect(nextControlled([formAt(1, true), formAt(3, false), formAt(5, false)])).toBe(3);
  expect(nextControlled([formAt(1, false), formAt(3, true), formAt(5, false)])).toBe(5);
  expect(nextControlled([formAt(1, false), formAt(3, false), formAt(5, true)])).toBe(1);
  // Order is the identifiers' rather than the frame's.
  expect(nextControlled([formAt(5, true), formAt(1, false), formAt(3, false)])).toBe(1);
  expect(nextControlled([formAt(1, true)])).toBeNull();
  expect(nextControlled(undefined)).toBeNull();
});

test('the Handoff detail sits behind Why? and nothing else is added to the surface', () => {
  const { container } = render(
    <Inspect
      mode="still"
      profile={null}
      echo={null}
      inspect={() => {}}
      forms={[formAt(1, true), formAt(3, false)]}
    />,
  );
  expect(container.querySelector('.handoff-detail')).toBeNull();
  const why = container.querySelector('.handoff-why') as HTMLElement;
  expect(why.getAttribute('aria-expanded')).toBe('false');
  fireEvent.click(why);
  expect(why.getAttribute('aria-expanded')).toBe('true');
  expect(container.querySelector('.handoff-detail')?.textContent).toBe(
    entries['explanation.handoff'].text,
  );
  // No modal, nothing to dismiss, and the field is not covered.
  expect(container.querySelector('[role="dialog"], [aria-modal]')).toBeNull();
});

test('a moving Field offers no Handoff control at all', () => {
  const { container } = render(
    <Inspect
      mode="running"
      profile={null}
      echo={null}
      inspect={() => {}}
      forms={[formAt(1, true), formAt(3, false)]}
    />,
  );
  expect(container.querySelector('.handoff')).toBeNull();
  expect(container.textContent).toBe('');
});
