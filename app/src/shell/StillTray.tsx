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
  FormId,
  PrivilegeValue,
  SlateCandidate,
  Surround,
  ViewDeclaration,
} from '../../../worker/src/protocol';
import type { FrameState } from '../../../worker/src/frame-state';
import type { StillTool } from './still-edits';
import knot from '../../../content/forms/knot.json';

/** The raw form of 1: every value and every range bound is a fraction of it. */
const WHOLE = 65536;
const JUNCTION = knot.abilities[0];

function chargeUnits(raw: number): string {
  return (raw / WHOLE).toLocaleString('en-US');
}

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
  /** Current authoritative physical members, read from frame flags. */
  physicalMembers?: number;
  /** Membership a queued physical edit would install, or zero when none stands. */
  proposedPhysicalMembers?: number;
  /** Current exposed physical members. */
  exposedPhysicalMembers?: number;
  /** Raw Q0.16 leakage per exposed external contact per simulation step. */
  leakPerExposedContactPerStep?: number;
  /** Opens the paused run's laboratory workspace. */
  openLab?: () => void;
  /** Whether the laboratory workspace currently covers the Field. */
  labOpen?: boolean;
  /** The commissioned chassis, used to expose its local material action. */
  form?: FormId;
  /** Stages one Knot junction at the controlled Form's current position. */
  deployJunction?: () => void;
}

const SURROUND_KEYS: Readonly<Record<Surround, string>> = {
  adjacent: 'label.surround_adjacent',
  double: 'label.surround_double',
  whole: 'label.surround_whole',
};

function leakagePercent(raw: number): string {
  return `${((raw / WHOLE) * 100).toFixed(raw === 0 ? 0 : 3)}%`;
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
  physicalMembers = 0,
  proposedPhysicalMembers = 0,
  exposedPhysicalMembers = 0,
  leakPerExposedContactPerStep = 0,
  openLab = () => {},
  labOpen = false,
  form,
  deployJunction = () => {},
}: StillTrayProps) {
  if (!mode || !SHOWN_IN.includes(mode)) return null;
  const conflicted = queue.entries.some((entry) => entry.conflict);
  const canCommit = mode === 'still'
    && queue.entries.length > 0
    && queue.cost_total <= impulse
    && !conflicted;
  const canUndo = mode === 'still' && queue.entries.length > 0;

  return (
    <div
      className="tray field-still-shell"
      data-mode={mode}
      data-tool={tool}
      data-queue={queue.entries.length > 0 ? 'queued' : 'empty'}
      data-can-commit={canCommit}
      data-can-undo={canUndo}
      data-conflicted={conflicted}
    >
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
      <section
        className="tray-mode-panel tray-physical-panel"
        data-active={tool === 'compartment'}
        aria-label={copy('label.physical_compartment')}
      >
        <p className="tray-panel-kicker">{copy('label.physical_compartment')}</p>
        <div className="tray-physical-readings">
          <p className="tray-reading">
            <span className="tray-term">{copy('label.physical_members')}</span>
            <span className="tray-value">{physicalMembers}</span>
          </p>
          <p className="tray-reading">
            <span className="tray-term">{copy('label.exposed_members')}</span>
            <span className="tray-value">{exposedPhysicalMembers}</span>
          </p>
          <p className="tray-reading">
            <span className="tray-term">{copy('label.leak_coefficient')}</span>
            <span className="tray-value tray-value-unit">
              {leakagePercent(leakPerExposedContactPerStep)}
              <small>{copy('unit.per_contact_step')}</small>
            </span>
          </p>
          {proposedPhysicalMembers > 0 ? (
            <p className="tray-reading tray-proposed-reading">
              <span className="tray-term">{copy('label.proposed_members')}</span>
              <span className="tray-value">{proposedPhysicalMembers}</span>
            </p>
          ) : null}
        </div>
        {form === 'knot' ? (
          <section className="tray-junction" aria-label={copy('ability.junction_material')}>
            <div className="tray-junction-head">
              <span>{copy('ability.junction_material')}</span>
              <strong>{copy('ability.junction_blanks')}</strong>
            </div>
            <div className="tray-junction-spec">
              <p className="tray-reading">
                <span className="tray-term">{copy('ability.junction_deployment')}</span>
                <span className="tray-value">{chargeUnits(JUNCTION.deploy_cost)} {copy('unit.cu')}</span>
              </p>
              <p className="tray-reading">
                <span className="tray-term">{copy('ability.junction_capacity')}</span>
                <span className="tray-value">{chargeUnits(JUNCTION.capacity)} {copy('unit.cu')}</span>
              </p>
              <p className="tray-reading">
                <span className="tray-term">{copy('ability.junction_upkeep')}</span>
                <span className="tray-value">{chargeUnits(JUNCTION.upkeep_rate)} {copy('unit.cu_per_step')}</span>
              </p>
            </div>
            <button
              type="button"
              className="tray-deploy-junction"
              onClick={deployJunction}
              disabled={mode !== 'still' || queue.entries.length >= 6 || impulse <= queue.cost_total}
            >
              {copy('ability.junction_deploy')}
            </button>
          </section>
        ) : null}
        <button
          type="button"
          className="tray-open-lab"
          aria-controls="field-laboratory"
          aria-expanded={labOpen}
          onClick={openLab}
          disabled={mode !== 'still'}
        >
          {copy('lab.open')}
        </button>
      </section>
      <section
        className="tray-view-panel"
        data-active={tool === 'view'}
        aria-label={copy('label.observation_view')}
      >
        <p className="tray-panel-kicker">{copy('label.view_protocol')}</p>
        <div className="tray-view-readings">
          <p className="tray-reading">
            <span className="tray-term">{copy('label.view_members')}</span>
            <span className="tray-value">{view?.inside.length ?? 0}</span>
          </p>
          <p className="tray-reading">
            <span className="tray-term">{copy('label.measurement_grain')}</span>
            <span className="tray-value">{view?.resolution ?? 0}</span>
          </p>
          <p className="tray-reading">
            <span className="tray-term">{copy('label.analysis_window')}</span>
            <span className="tray-value tray-value-unit">
              {view?.window ?? 0}
              <small>{copy('unit.steps')}</small>
            </span>
          </p>
          <p className="tray-reading">
            <span className="tray-term">{copy('label.comparison_neighborhood')}</span>
            <span className="tray-value">
              {view ? copy(SURROUND_KEYS[view.surround]) : copy('label.not_available')}
            </span>
          </p>
        </div>
        <button
          type="button"
          className="tray-clear-view"
          disabled={!slate || tool !== 'view' || mode !== 'still' || !view || view.inside.length === 0}
          onClick={() => setFocus(0)}
        >
          {copy('label.clear_view')}
        </button>
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
      </section>
      <div
        className="tray-budget tray-operation-dock"
        data-can-commit={canCommit}
        data-can-undo={canUndo}
        data-conflicted={conflicted}
      >
        <div className="tray-operation-summary">
          <p className="tray-reading">
            <span className="tray-term">{copy('label.impulse')}</span>
            <span className="tray-value">{impulse}</span>
          </p>
          <p className="tray-reading">
            <span className="tray-term">{copy('label.queue')}</span>
            <span className="tray-value">{queue.entries.length}</span>
          </p>
          <p className="tray-reading" data-total="cost">
            <span className="tray-term">{copy('label.cost')}</span>
            <span className="tray-value">{queue.cost_total}</span>
          </p>
          <meter
            className="tray-budget-meter"
            min={0}
            max={Math.max(1, impulse)}
            value={Math.min(queue.cost_total, Math.max(1, impulse))}
            aria-label={copy('label.cost')}
          />
        </div>
        <ul className="tray-queue" aria-label={copy('label.queue')}>
          {queue.entries.map((entry) => (
            <li
              key={entry.position}
              className="tray-entry"
              data-conflict={entry.conflict}
              data-op={entry.plan.op}
              aria-label={`${copy('label.queue')} ${entry.position}`}
            >
              <span className="tray-mark" />
              <span className="tray-entry-cost">{entry.cost}</span>
            </li>
          ))}
        </ul>
      </div>
      <p className="tray-guidance">
        {copy(tool === 'view' ? 'instruction.move_view' : 'instruction.shape_compartment')}
      </p>
    </div>
  );
}
