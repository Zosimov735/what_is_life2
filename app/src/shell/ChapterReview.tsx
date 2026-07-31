/**
 * The two surfaces the campaign puts up between and after its chapters: the
 * review a transition leaves, and the ending a completed campaign closes on.
 *
 * **No chapter rule lives here.** Which chapter follows which, when a chapter
 * closes, what an ending is, and which ending a run reaches are all the core's,
 * and this file renders exactly what the worker reported: the `chapter_changed`
 * the run entered on, the one it entered on before that, and the `ending_id`
 * the `run_completed` event carried. Every string comes from the catalog, and
 * the two the worker names — the chapter's `title_key` and the ending's key —
 * are read by the accessor like any other.
 *
 * **It is a review, not a report wall.** `docs/field-framework/LEXICON.md`
 * allows one short line and puts detail behind `Why?`, so that is the shape: a
 * label, the chapter's own name, the optional explanation, and one control that
 * dismisses it. Nothing here is a number, nothing ranks anything, and no
 * objective is listed beside another — one objective is visible at a time, and
 * a list of them stacked would be exactly what that rule refuses.
 *
 * **It never blocks the run.** A transition is settled inside the simulation
 * and nothing waits for the player: the Field of the chapter that opened is
 * already moving behind this. So it is a corner surface with one button in the
 * document's own order, dismissed by that button, and the steering under it is
 * untouched.
 */

import { useState } from 'react';
import { copy } from './copy';
import type { ChapterChanged, RunCompleted } from '../../../worker/src/protocol';

interface ChapterReviewProps {
  /** The chapter that closed, and none until one has. */
  review: ChapterChanged | null;
  /** The ending the campaign closed on, and none before it did. */
  ending: RunCompleted | null;
  /** Forgets the standing review, which is what showing it once means. */
  clearReview: () => void;
}

export function ChapterReview({ review, ending, clearReview }: ChapterReviewProps) {
  const [explaining, setExplaining] = useState(false);

  // The ending stands in place of the review: a campaign that has ended has no
  // next chapter to have carried into, and one surface at a time is the rule
  // the objective line already keeps.
  if (ending) {
    return (
      <div className="review" data-kind="ending" role="status" aria-live="polite">
        <p className="review-label">{copy('label.ending')}</p>
        <p className="review-line">{copy(ending.ending_id)}</p>
        <button
          type="button"
          className="review-why"
          aria-expanded={explaining}
          onClick={() => setExplaining((held) => !held)}
        >
          {copy('label.why')}
        </button>
        {explaining ? <p className="review-detail">{copy('explanation.ending')}</p> : null}
      </div>
    );
  }

  if (!review) return null;

  return (
    <div className="review" data-kind="chapter" role="status" aria-live="polite">
      <p className="review-label">{copy('label.chapter_review')}</p>
      <p className="review-line">{copy(review.title_key)}</p>
      <button
        type="button"
        className="review-why"
        aria-expanded={explaining}
        onClick={() => setExplaining((held) => !held)}
      >
        {copy('label.why')}
      </button>
      {explaining ? <p className="review-detail">{copy('explanation.chapter_review')}</p> : null}
      <button
        type="button"
        className="review-continue"
        onClick={() => {
          setExplaining(false);
          clearReview();
        }}
      >
        {copy('label.continue')}
      </button>
    </div>
  );
}
