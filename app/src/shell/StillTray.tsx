/**
 * The queued-change tray: the one piece of chrome Still Mode adds.
 *
 * `docs/field-framework/SPEC.md` says Still Mode makes the ports, routes,
 * boundary handles, and forecasts visible without leaving the main screen, and
 * `docs/field-framework/LEXICON.md` says how a surface may say anything at
 * all. So the division is the one the whole shell already keeps: everything
 * about the Field is drawn on the canvas by the renderer, and this shows only
 * what is not in the Field — the Impulse the run carries and the changes
 * standing in the queue, which are quantities of the run rather than places in
 * it.
 *
 * Version 2 adds one compact, focusable tool switch. It names the causal
 * physical compartment separately from the passive View, and candidate
 * buttons move only that View. Keyboard input ignores interactive elements,
 * so focusing this chrome does not leak Space or Enter into the Field.
 *
 * The mode's name remains a status region, exactly as the one objective is,
 * because entering Still Mode is the thing a player who cannot see the Field
 * settle most needs told.
 *
 * What it lists is what the worker reported: one entry per queued change, each
 * with what it costs, and the total the queue costs together. Those two numbers
 * are the prediction — and they are the same numbers a commit spends, because
 * the core arrives at both from one count, the length of the queue.
 *
 * It also carries the ranking, because the ranking is a reading of the slate
 * rather than a place on the Field: the candidates grouped by tier, each with
 * its four values drawn as the confidence ranges the comparison reads, an
 * unassigned value drawn as the absence it is, and the tolerance-sensitivity
 * flag when the record raises one. Four ranges, never one bar.
 */

import { copy } from './copy';
import type { CandidateSlate, QueueState } from './worker-client';
import type {
  PrivilegeValue,
  SlateCandidate,
  ViewDeclaration,
} from '../../../worker/src/protocol';
import type { FrameState } from '../../../worker/src/frame-state';
import type { StillTool } from './still-edits';

/** The raw form of 1: every value and every range bound is a fraction of it. */
const WHOLE = 65536;

/**
 * The four values, in the order the surface reads them, each with the catalog
 * label that names it.
 *
 * They are shown side by side and never added, averaged, or weighed against
 * one another: four readings stand beside a candidate, and the only figure
 * derived from them anywhere is the tier, which is the rank of a set under the
 * dominance relation rather than a number that stands for a candidate.
 */
type ValueName =
  | 'scale_stability'
  | 'shared_failure'
  | 'cut_impact'
  | 'boundary_sufficiency';

const VALUES: readonly [ValueName, string][] = [
  ['scale_stability', 'label.value_scale_stability'],
  ['shared_failure', 'label.value_shared_failure'],
  ['cut_impact', 'label.value_cut_impact'],
  ['boundary_sufficiency', 'label.value_boundary_sufficiency'],
];

/**
 * One value, as a bar that spans its confidence range.
 *
 * The range is what the comparison reads, so the range is what is drawn: the
 * bar runs from `low` to `high`, and the value's own place inside it is a
 * notch. An unassigned value carries no number and no range, so it is drawn as
 * an absence with a mark of its own — not as a zero, which would be a reading
 * the record does not hold.
 */
function ValueBar({ name, value }: { name: string; value: PrivilegeValue }) {
  const assigned = value.value !== null && value.low !== null && value.high !== null;
  return (
    <p className="tray-value-row" data-assigned={assigned}>
      <span className="tray-value-name">{copy(name)}</span>
      {assigned ? (
        <span
          className="tray-range"
          data-low={value.low}
          data-high={value.high}
          data-value={value.value}
          style={{
            // Percentages of the whole, which is what [0, 1] means here.
            ['--range-low' as string]: `${((value.low ?? 0) * 100) / WHOLE}%`,
            ['--range-span' as string]: `${(((value.high ?? 0) - (value.low ?? 0)) * 100) / WHOLE}%`,
            ['--range-at' as string]: `${((value.value ?? 0) * 100) / WHOLE}%`,
          }}
        >
          <span className="tray-range-span" />
          <span className="tray-range-at" />
        </span>
      ) : (
        <span className="tray-range" data-reason={value.reason ?? ''}>
          <span className="tray-value-absent">{copy('label.value_unassigned')}</span>
        </span>
      )}
    </p>
  );
}

/** The modes the tray stands through: the two ramps and the pause between. */
const SHOWN_IN: readonly string[] = ['ramp_in', 'still', 'ramp_out'];

interface StillTrayProps {
  /** The mode the newest snapshot reports, and none before the first. */
  mode: FrameState['header']['mode'] | null;
  /** The queue as the worker last reported it. */
  queue: QueueState;
  /** The Impulse the newest snapshot's header carries. */
  impulse: number;
  /** The evaluation record the run stands under, and none before the first. */
  slate?: CandidateSlate | null;
  /** The authoritative passive View, and none while no reading is available. */
  view?: ViewDeclaration | null;
  /** The candidate the active passive View matches, 1-based, and 0 otherwise. */
  focused?: number;
  /** The explicit active Still Mode tool. */
  tool?: StillTool;
  /** Selects the causal or passive tool. */
  setTool?: (tool: StillTool) => void;
  /** Immediately moves the passive View to one candidate. */
  setFocus?: (position: number) => void;
}

/**
 * What a candidate is called: the name of the source it came from, from the
 * catalog.
 *
 * The first provenance is the one that put the entry in the slate; a later one
 * reached the same View and merged onto it, which is a second reason for the
 * same candidate rather than a second candidate, so the tray shows the first.
 *
 * Beside the name stand the four values as ranges and the tier as the group
 * the candidate is listed under. What never stands there is a figure that
 * folds the four into one: there is no such figure in the record, and this
 * surface adds none.
 */
function candidateKey(source: string): string {
  return `label.candidate_${source}`;
}

/**
 * The slate grouped by tier, ascending, with assembly order kept inside each.
 *
 * A slate the core has not compared — a deficient one — carries tier 0 for
 * every candidate, which is the number no tier has; that slate lists nothing
 * at all, so this is only ever asked of a compared one.
 */
export function tiersOf(slate: CandidateSlate): [number, SlateCandidate[]][] {
  const found = new Map<number, SlateCandidate[]>();
  for (const candidate of slate.candidates) {
    const held = found.get(candidate.tier);
    if (held) held.push(candidate);
    else found.set(candidate.tier, [candidate]);
  }
  return [...found.entries()].sort((first, second) => first[0] - second[0]);
}

export function StillTray({
  mode,
  queue,
  impulse,
  slate = null,
  view = null,
  focused = 0,
  tool = 'view',
  setTool = () => {},
  setFocus = () => {},
}: StillTrayProps) {
  if (!mode || !SHOWN_IN.includes(mode)) return null;

  return (
    <div className="tray" data-mode={mode} data-tool={tool}>
      {/* Announced when it changes, exactly as the one objective is: entering
          the mode is the thing a player who cannot see the surface settle
          most needs told, and the tray is the only place it is said. */}
      <p className="tray-name" role="status" aria-live="polite">
        {copy('label.still_mode')}
      </p>
      <div className="tray-tools" role="group" aria-label={copy('label.still_mode')}>
        <button
          type="button"
          className="tray-tool"
          data-tool="view"
          aria-pressed={tool === 'view'}
          onClick={() => setTool('view')}
        >
          <span>{copy('label.observation_view')}</span>
          <small>{copy('label.passive_free')}</small>
        </button>
        <button
          type="button"
          className="tray-tool"
          data-tool="compartment"
          aria-pressed={tool === 'compartment'}
          onClick={() => setTool('compartment')}
        >
          <span>{copy('label.physical_compartment')}</span>
          <small>{copy('label.causal_paid')}</small>
        </button>
      </div>
      <button
        type="button"
        className="tray-clear-view"
        disabled={!slate || tool !== 'view' || mode !== 'still' || !view || view.inside.length === 0}
        onClick={() => setFocus(0)}
      >
        {copy('label.clear_view')}
      </button>
      <p className="tray-reading">
        <span className="tray-term">{copy('label.impulse')}</span>
        <span className="tray-value">{impulse}</span>
      </p>
      <p className="tray-reading">
        <span className="tray-term">{copy('label.queue')}</span>
        <span className="tray-value">{queue.entries.length}</span>
      </p>
      {/* What the queue would spend, which is what a commit does spend. */}
      <p className="tray-reading" data-total="cost">
        <span className="tray-term">{copy('label.cost')}</span>
        <span className="tray-value">{queue.cost_total}</span>
      </p>
      {/* The candidates the standing slate holds, in presentation order, each
          named by the source it came from. A deficient slate is not compared
          and not adopted from, so it lists nothing. */}
      {slate && !slate.deficient ? (
        <>
          <p className="tray-reading">
            <span className="tray-term">{copy('label.candidates')}</span>
            <span className="tray-value">{slate.candidates.length}</span>
          </p>
          {/* The active passive View, said when it changes, exactly as the
              mode's own status is. The region stands whenever the list does,
              empty while no candidate matches, because a live region mounted
              mid-walk announces nothing. */}
          <p className="tray-focus" role="status" aria-live="polite">
            {focused > 0
              ? copy(
                  candidateKey(
                    slate.candidates.find((held) => held.position === focused)?.provenance[0]
                      ?.source ?? 'standing',
                  ),
                )
              : ''}
          </p>
          {/* The ranking, as tier grouping: the nondominated set stands first,
              then the set that stands once it is removed, and so on. The
              candidates inside a tier keep assembly order, because the ranking
              partitions the slate and never reorders within a tier. Nothing
              here is a combined figure — a tier is the rank of a set under the
              dominance relation, and the four values stand separately beside
              each candidate. */}
          {tiersOf(slate).map(([tier, held]) => (
            <ul className="tray-candidates" key={tier} data-tier={tier}>
              <li className="tray-tier">
                <span className="tray-tier-name">{copy('label.tier')}</span>
                <span className="tray-value">{tier}</span>
              </li>
              {held.map((candidate) => (
                <li
                  key={candidate.position}
                  className="tray-candidate"
                  data-position={candidate.position}
                  data-tier={candidate.tier}
                  data-focused={candidate.position === focused}
                >
                  <button
                    type="button"
                    className="tray-candidate-select"
                    aria-pressed={candidate.position === focused}
                    disabled={tool !== 'view' || mode !== 'still'}
                    onClick={() => setFocus(candidate.position)}
                  >
                    <span className="tray-mark" />
                    <span className="tray-candidate-name">
                      {copy(candidateKey(candidate.provenance[0]?.source ?? 'standing'))}
                    </span>
                  </button>
                  <span className="tray-values">
                    {VALUES.map(([key, name]) => (
                      <ValueBar
                        key={key}
                        name={name}
                        value={candidate.privilege[key] as PrivilegeValue}
                      />
                    ))}
                  </span>
                </li>
              ))}
            </ul>
          ))}
          {/* The tolerance-sensitivity flag: a warning that the ranking beside
              it turns on the declared tolerance rather than on the readings
              alone. It is a flag and not a figure. */}
          {slate.sensitivity?.flag ? (
            <p className="tray-sensitivity" role="status" aria-live="polite">
              {copy('label.sensitivity')}
            </p>
          ) : null}
        </>
      ) : null}
      <ul className="tray-queue">
        {queue.entries.map((entry) => (
          <li
            key={entry.position}
            className="tray-entry"
            data-conflict={entry.conflict}
            data-op={entry.plan.op}
          >
            <span className="tray-mark" />
            <span className="tray-entry-cost">{entry.cost}</span>
          </li>
        ))}
      </ul>
    </div>
  );
}
