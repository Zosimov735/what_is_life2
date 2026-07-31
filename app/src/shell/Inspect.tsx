/**
 * The optional coordinate-profile surface, and the Echo one committed change
 * leaves.
 *
 * Two rules govern this file, and they pull in opposite directions until the
 * division below is read:
 *
 * - **Ordinary play carries no numerical dashboard.** `docs/field-framework/
 *   LEXICON.md`'s writing rules are the whole of it: no collapsed value is ever
 *   shown, and detail sits behind an optional control. So nothing here renders
 *   while the run is moving — the component returns null outside the still
 *   surface's own modes — and no number reaches the page until a player opens
 *   the surface.
 * - **Profiles are inspectable.** The framework's readings exist to be read, so
 *   there is a way in, and it is the `Why?` shape the objective already uses: a
 *   button, in the document's own order, keyboard reachable because it is a
 *   button rather than because anything here manages focus.
 *
 * The two meet at a control that is closed by default and asks for nothing
 * until it is opened. A session whose player never opens it never takes a
 * reading at all — the request is what makes the worker compute one, and no
 * request is sent on its own.
 *
 * **Why this stands beside the tray rather than inside it.**
 * `docs/field-framework/ARCHITECTURE.md` locks the queued-change tray as
 * unfocusable, because a key that moved focus is the one thing that could take
 * Space and Enter out from under the player. That lock is about the tray, and
 * it stands untouched: this is its own region, exactly as `Why?` is its own
 * control beside the one objective, and the Still Mode key source already
 * leaves every key aimed at an interactive element entirely alone.
 *
 * **What is shown.** Ten readings, separately, each with the label the
 * framework gives it and the unit the record carries. No figure folds several
 * into one, no reading is ranked against another, and an unassigned reading is
 * drawn as the absence it is with the record's stated reason — never as a zero,
 * which would be a reading the record does not hold.
 */

import { useEffect, useState } from 'react';
import { copy } from './copy';
import type { PlaybackReading } from '../render';
import type {
  CoordinateProfile,
  CoordinateReading,
  EchoHighlight,
  InspectRequest,
  PerturbationResult,
} from '../../../worker/src/protocol';
import type { FrameState } from '../../../worker/src/frame-state';

/** The raw form of 1: every `Frac` is a fraction of it. */
const WHOLE = 65536;

/** The modes the still surface stands through. */
const SHOWN_IN: readonly string[] = ['ramp_in', 'still', 'ramp_out'];

/** The catalog key an Echo of one kind is worded by. */
const ECHO_COPY: Readonly<Record<string, string>> = {
  'route-removal': 'notice.echo_route_removal',
  'boundary-severance': 'notice.echo_boundary_severance',
  'component-substitution': 'notice.echo_component_substitution',
  evaluation: 'notice.echo_evaluation',
};

/**
 * How a reading is written out, per coordinate.
 *
 * The unit is the coordinate's own — FRAMEWORK.md declares each one — and the
 * record carries the raw integer. A count is a count, a fraction is a fraction
 * of the whole, a distance is in the declared distance unit, and a rate is
 * Charge per step. Nothing is converted into anything else's unit, because
 * there is no unit the ten share.
 */
type Unit = 'count' | 'fraction' | 'distance' | 'steps' | 'rate';

const COORDINATES: readonly [keyof CoordinateProfile, string, Unit][] = [
  ['swap_range', 'label.coord_swap_range', 'count'],
  ['self_support', 'label.coord_self_support', 'fraction'],
  ['reach', 'label.coord_reach', 'distance'],
  ['input_resolution', 'label.coord_input_resolution', 'count'],
  ['horizon', 'label.coord_horizon', 'steps'],
  ['source_trace', 'label.coord_source_trace', 'fraction'],
];

/** A raw value in its own unit, as text. */
function written(value: number, unit: Unit): string {
  if (unit === 'fraction') return (value / WHOLE).toFixed(3);
  if (unit === 'distance') return (value / WHOLE).toFixed(0);
  if (unit === 'rate') return (value / WHOLE).toFixed(2);
  return String(value);
}

/**
 * The playback reading one perturbation result offers, and null when the
 * result offers none.
 *
 * The played sample is the one with the largest excess, smallest sample number
 * on equal excesses — the sample the Echo already names. A result whose
 * samples carry no excess at all — an unassigned reading — offers nothing: a
 * playback of no reading would be motion standing for nothing.
 */
export function playbackOf(result: PerturbationResult | null): PlaybackReading | null {
  if (!result) return null;
  let taken: { excess: number; at: number } | null = null;
  for (let at = 0; at < result.samples.length; at += 1) {
    const excess = result.samples[at].excess;
    if (excess === null) continue;
    if (!taken || excess > taken.excess) taken = { excess, at };
  }
  if (!taken) return null;
  const sample = result.samples[taken.at];
  if (sample.series.length === 0) return null;
  return {
    members: result.view.inside,
    series: sample.series,
    base: sample.base_series,
  };
}

/** The modes a playback reading may stand in. */
const PLAYED_IN: readonly string[] = ['ramp_in', 'still', 'ramp_out'];

/**
 * The reading the shell offers the renderer for the mode the run stands in:
 * the held result's playback while the run is still or in a ramp, and null in
 * ordinary play — a moving Field never carries one. The offer lives in this
 * module because this surface is where a result is asked for: playback is
 * offered only from the opt-in surface, which is what makes it optional the
 * same way the profile is.
 */
export function offeredPlayback(
  mode: FrameState['header']['mode'] | null,
  result: PerturbationResult | null,
): PlaybackReading | null {
  if (!mode || !PLAYED_IN.includes(mode)) return null;
  return playbackOf(result);
}

/**
 * One reading: its own label, and either its number in its own unit or the
 * absence the record holds instead.
 */
function Reading({
  name,
  unit,
  reading,
}: {
  name: string;
  unit: Unit;
  reading: { value: number | null; reason: string | null } | null;
}) {
  const assigned = reading != null && reading.value !== null;
  return (
    <p className="inspect-row" data-assigned={assigned} data-unit={unit}>
      <span className="inspect-name">{copy(name)}</span>
      <span className="inspect-value" data-reason={reading?.reason ?? ''}>
        {assigned ? written(reading.value as number, unit) : copy('label.value_unassigned')}
      </span>
    </p>
  );
}

interface InspectProps {
  /** The mode the newest snapshot reports, and none before the first. */
  mode: FrameState['header']['mode'] | null;
  /** The profile the worker last answered with, and none until one is asked for. */
  profile: CoordinateProfile | null;
  /** The Echo the newest committed change left, and none before one. */
  echo: EchoHighlight | null;
  /** Asks for one inspection, carried by the next frame. */
  inspect: (request: InspectRequest) => void;
  /** Forgets the standing Echo once it has been shown. */
  clearEcho?: () => void;
  /**
   * The Forms the newest snapshot carries, in the frame's own order, and none
   * before the first snapshot. The Handoff control reads which stand and which
   * carries control from here and from nowhere else.
   */
  forms?: FrameState['forms'];
}

/**
 * The Form a Handoff would move control to: the next one in ascending
 * identifier order, wrapping past the end.
 *
 * One control rather than a list of them. Which Forms a chapter places is the
 * chapter's business and their identifiers mean nothing a player reads, so the
 * surface offers the move rather than a menu of destinations — and alternation
 * between two patterns, which is what a Handoff is for, is one press either
 * way. Answers none where fewer than two Forms stand, which is what keeps the
 * control off every chapter that places one.
 */
export function nextControlled(forms: FrameState['forms'] | undefined): number | null {
  if (!forms || forms.length < 2) return null;
  const ids = forms.map((form) => form.id).sort((first, second) => first - second);
  const held = forms.find((form) => form.controlled);
  if (!held) return ids[0];
  const place = ids.indexOf(held.id);
  return ids[(place + 1) % ids.length];
}

export function Inspect({ mode, profile, echo, inspect, clearEcho, forms }: InspectProps) {
  const [open, setOpen] = useState(false);
  const [explaining, setExplaining] = useState(false);
  const handing = nextControlled(forms);

  // The surface asks for the eight recorded-window coordinates when it opens,
  // and again whenever the run pauses with it open — a fresh pause is a fresh
  // window, and a reading of the previous one would be a reading of a Field
  // that has moved since.
  useEffect(() => {
    if (!open || mode !== 'still') return;
    inspect({ target: 'coordinates', kind: null, parameter: null });
  }, [open, mode, inspect]);

  // The Echo is shown once, on the surface that follows the commit, and is let
  // go of when the next pause begins.
  useEffect(() => {
    if (!echo || !mode || !SHOWN_IN.includes(mode)) return;
    clearEcho?.();
  }, [echo, mode, clearEcho]);

  const wording = echo ? ECHO_COPY[echo.kind] : undefined;

  return (
    <>
      {/* The one short causal highlight, and never a report wall: one line,
          from the catalog, naming the cause plainly. The record carries the
          numbers; the surface does not show them. */}
      {wording ? (
        <p className="echo" role="status" aria-live="polite">
          {copy(wording)}
        </p>
      ) : null}
      {/* The Handoff: control moves to another Form, in Still Mode, at once
          and for nothing. Its own region beside the tray, one plain button, so
          it is keyboard reachable because it is a button rather than because
          anything here manages focus — and the detail sits behind `Why?`,
          exactly as the objective's does. It stands only where a run holds
          several Forms; a chapter that places one never shows it. */}
      {mode && SHOWN_IN.includes(mode) && handing !== null ? (
        <div className="handoff">
          <button
            type="button"
            className="handoff-move"
            onClick={() => inspect({ target: 'handoff', kind: null, parameter: handing })}
          >
            {copy('label.handoff')}
          </button>
          <button
            type="button"
            className="handoff-why"
            aria-expanded={explaining}
            onClick={() => setExplaining((held) => !held)}
          >
            {copy('label.why')}
          </button>
          {explaining ? <p className="handoff-detail">{copy('explanation.handoff')}</p> : null}
        </div>
      ) : null}
      {mode && SHOWN_IN.includes(mode) ? (
        <div className="inspect" data-open={open}>
          <button
            type="button"
            className="inspect-open"
            aria-expanded={open}
            onClick={() => setOpen((held) => !held)}
          >
            {copy('label.coordinate_profile')}
          </button>
          {open ? (
            <div className="inspect-panel" role="status" aria-live="polite">
              <p className="inspect-detail">{copy('explanation.coordinate_profile')}</p>
              {profile ? (
                <>
                  {COORDINATES.map(([key, name, unit]) => (
                    <Reading
                      key={key}
                      name={name}
                      unit={unit}
                      reading={profile[key] as CoordinateReading}
                    />
                  ))}
                  {/* Throughput is one coordinate holding two magnitudes and
                      an itemized identity, so it takes two rows — the Charge
                      entering per step, and the Charge leaving — each named
                      from the catalog; the identity is in the record beside
                      them. Two rows for one reading, never one figure for
                      two. */}
                  <Reading
                    name="label.coord_throughput_in"
                    unit="rate"
                    reading={{ value: profile.throughput.in_rate, reason: null }}
                  />
                  <Reading
                    name="label.coord_throughput_out"
                    unit="rate"
                    reading={{ value: profile.throughput.out_rate, reason: null }}
                  />
                  {/* Upkeep Mix is five shares that sum to one, so it is written
                      as the five and never as one of them. */}
                  <p className="inspect-row" data-assigned={profile.upkeep_mix.value !== null}>
                    <span className="inspect-name">{copy('label.coord_upkeep_mix')}</span>
                    <span className="inspect-value" data-reason={profile.upkeep_mix.reason ?? ''}>
                      {profile.upkeep_mix.value
                        ? profile.upkeep_mix.value
                            .map((share) => (share / WHOLE).toFixed(2))
                            .join(' ')
                        : copy('label.value_unassigned')}
                    </span>
                  </p>
                  <Reading
                    name="label.coord_instruction_separation"
                    unit="fraction"
                    reading={profile.instruction_separation}
                  />
                  <Reading
                    name="label.coord_turnover_tolerance"
                    unit="fraction"
                    reading={profile.turnover_tolerance}
                  />
                  {/* The two replay-based coordinates cost eight replays each,
                      so they are taken only when they are asked for — the split
                      the analysis budget locks, read as a second control. */}
                  <button
                    type="button"
                    className="inspect-replays"
                    onClick={() =>
                      inspect({ target: 'coordinates_full', kind: null, parameter: null })
                    }
                  >
                    {copy('label.coordinate_replays')}
                  </button>
                </>
              ) : null}
            </div>
          ) : null}
        </div>
      ) : null}
    </>
  );
}
