/**
 * The one visible objective, and the optional control that explains it.
 *
 * `docs/field-framework/LEXICON.md` governs what this may show: one objective
 * at a time, never two stacked; at most six words, counted in the catalog
 * rather than here; no opening exposition and no modal tutorial; and detail
 * behind `Why?` and nowhere else. Nothing is written inline — the objective's
 * own id is its copy-catalog key, and the explanation's key is the same name
 * under the `explanation` kind.
 *
 * The control is chrome and stays that way: a button, in the document's own
 * order, reachable by keyboard because it is a button rather than because
 * anything here manages focus. Opening it neither pauses the run nor covers the
 * surface — the field keeps moving behind it, which is what makes it not a
 * modal.
 */

import { useState } from 'react';
import { copy } from './copy';
import type { ObjectiveState } from '../../../worker/src/protocol';

/** The copy-catalog kind an objective's own explanation stands under. */
const EXPLANATION_PREFIX = 'explanation.';

/** The copy-catalog kind an objective's text stands under. */
const OBJECTIVE_PREFIX = 'objective.';

/**
 * Where an objective's explanation stands in the catalog: the same name under
 * the `explanation` kind. The core derives it the same way.
 */
export function explanationKey(objectiveId: string): string | null {
  if (!objectiveId.startsWith(OBJECTIVE_PREFIX)) return null;
  return `${EXPLANATION_PREFIX}${objectiveId.slice(OBJECTIVE_PREFIX.length)}`;
}

interface ObjectiveProps {
  /** The objective the run stands on, and none before one is offered. */
  objective: ObjectiveState | null;
}

export function Objective({ objective }: ObjectiveProps) {
  const [explaining, setExplaining] = useState(false);

  if (!objective || objective.id === '' || objective.state === 'hidden') return null;
  const explanation = explanationKey(objective.id);

  return (
    <div className="objective" data-state={objective.state}>
      {/* One objective, announced when it changes and never stacked. */}
      <p className="objective-line" role="status" aria-live="polite">
        {copy(objective.id)}
      </p>
      {explanation ? (
        <button
          type="button"
          className="objective-why"
          aria-expanded={explaining}
          onClick={() => setExplaining((held) => !held)}
        >
          {copy('label.why')}
        </button>
      ) : null}
      {explaining && explanation ? <p className="objective-detail">{copy(explanation)}</p> : null}
    </div>
  );
}
