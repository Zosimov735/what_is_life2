/**
 * The standing pressure, named — and the optional control that explains it.
 *
 * The direct reading of a pressure is the renderer's: the rim of the surface
 * carries its hue, depth, and beat, and the audio cues carry its onset, its
 * crisis, and its resolution. This line is the concise explanation beside
 * those cues, in the exact shape the objective takes: one short line, an
 * optional `Why?`, and the longer wording behind it — never a modal, never a
 * scripted sequence, the field moving behind it the whole time.
 *
 * What it shows is the pressure the run stands under: the primary one when
 * one stands, else the furthest-advanced active one. A queued pressure shows
 * nothing — a seat requested is not a pressure standing — and an empty list
 * shows nothing at all, which is most of a run.
 */

import { useState } from 'react';
import { copy } from './copy';
import type { PressureState } from '../../../worker/src/protocol';

/** The copy-catalog key a pressure's name stands under. */
function nameKey(pressure: PressureState): string {
  return `pressure.${pressure.pressure}`;
}

/** The copy-catalog key a pressure's explanation stands under. */
function explanationKey(pressure: PressureState): string {
  return `explanation.pressure_${pressure.pressure}`;
}

/** The authoritative rule family each member of the closed pressure set modifies. */
function ruleKey(pressure: PressureState): string {
  return `pressure.rule.${pressure.pressure}`;
}

/**
 * The one pressure the line names: the primary if one is active, else the
 * active one standing at the furthest stage, ties to the closed set's order.
 */
export function surfaced(pressures: readonly PressureState[]): PressureState | null {
  const active = pressures.filter((held) => !held.queued);
  if (active.length === 0) return null;
  const primary = active.find((held) => held.primary);
  if (primary) return primary;
  const order = ['signal', 'pressure', 'crisis', 'resolution'];
  return active.reduce((best, held) =>
    order.indexOf(held.stage) > order.indexOf(best.stage) ? held : best,
  );
}

interface PressureLineProps {
  /** The staged pressures as the worker last told them. */
  pressures: readonly PressureState[];
}

export function PressureLine({ pressures }: PressureLineProps) {
  const [explaining, setExplaining] = useState(false);
  const shown = surfaced(pressures);
  if (!shown) return null;

  return (
    <div
      className="pressure-line"
      data-stage={shown.stage}
      data-pressure={shown.pressure}
      data-expanded={explaining}
    >
      <div className="pressure-summary">
        <p className="pressure-name" role="status" aria-live="polite">
          {copy(nameKey(shown))}
        </p>
        <span className="pressure-stage">{shown.stage}</span>
        <meter
          className="pressure-level"
          min={0}
          max={65_536}
          value={shown.level}
          aria-label={copy('pressure.inspect.level')}
        />
        <span className="pressure-target">
          {shown.target.t}{shown.target.id === null ? '' : ` ${shown.target.id}`}
        </span>
        <button
          type="button"
          className="objective-why"
          aria-expanded={explaining}
          onClick={() => setExplaining((held) => !held)}
        >
          {copy('label.why')}
        </button>
      </div>
      {explaining ? (
        <div className="pressure-detail">
          <p className="objective-detail">{copy(explanationKey(shown))}</p>
          <dl>
            <div><dt>{copy('pressure.inspect.stage')}</dt><dd>{shown.stage}</dd></div>
            <div><dt>{copy('pressure.inspect.level')}</dt><dd>{(shown.level * 100 / 65_536).toFixed(1)}%</dd></div>
            <div><dt>{copy('pressure.inspect.target')}</dt><dd>{shown.target.t}{shown.target.id === null ? '' : ` ${shown.target.id}`}</dd></div>
            <div><dt>{copy('pressure.inspect.onset')}</dt><dd>{shown.start_step}</dd></div>
            <div><dt>{copy('pressure.inspect.modified_rule')}</dt><dd>{copy(ruleKey(shown))}</dd></div>
          </dl>
        </div>
      ) : null}
    </div>
  );
}
