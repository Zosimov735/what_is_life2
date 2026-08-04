import { useEffect, useMemo, useRef, useState } from 'react';
import type { CriterionReading, FormId, Surround, ViewDeclaration } from '../../../worker/src/protocol';
import type { FrameState } from '../../../worker/src/frame-state';
import type { CoreClient } from './worker-client';
import type { RegimeId } from './Atlas';
import { AnalysisCoordinator } from './analysis-client';
import {
  archiveRecords,
  holdoutSuites,
  recordFromExport,
  removeArchiveRecord,
  sealedHoldoutSuite,
  storeArchiveRecord,
  storeHoldoutSuite,
  type ArchiveRecord,
  type HoldoutSuite,
} from './archive';
import { copy } from './copy';
import {
  BENCHES,
  DEFAULT_OBSERVATION,
  INSTRUMENTS,
  INTERVENTIONS,
  LIVE_INTERVENTIONS,
  scenarioFrom,
  type BenchId,
  type AnalysisResult,
  type AnalysisTask,
  type DivergenceResult,
  type EnsembleResult,
  type InheritanceResult,
  type InstrumentId,
  type InstrumentReading,
  type InterventionId,
  type InterventionPlan,
  type LensSensorPacket,
  type ObservationProtocol,
  type RenewalInventory,
  type RenewalResult,
} from './experiment';
import {
  compileOpenField,
  DEFAULT_OPEN_FIELD,
  OPEN_FIELD_LAWSETS,
  runOpenField,
  withLawset,
  type CompiledOpenField,
  type OpenFieldDraft,
  type OpenFieldLawsetId,
  type OpenFieldRun,
} from './open-field';

interface ExperimentLabProps {
  client: CoreClient;
  frame: FrameState;
  regime: RegimeId;
  form: FormId;
  view: ViewDeclaration | null;
  onClose: () => void;
}

const SURROUNDS: readonly Surround[] = ['adjacent', 'double', 'whole'];

const TOOL_TARGET: Readonly<Record<InterventionId, 'route' | 'network' | 'supply' | 'node' | 'input' | 'component' | 'boundary' | 'regime'>> = {
  blade: 'route',
  clamp: 'route',
  scramble: 'network',
  decoy: 'supply',
  delay: 'input',
  replace: 'component',
  breach: 'boundary',
  transplant: 'regime',
};

const REPLACEMENT_TRANSFERS = [
  ['kind', 1],
  ['position', 2],
  ['open_state', 4],
  ['capacity_upkeep', 8],
  ['stored_charge', 16],
  ['routes', 32],
  ['membership', 64],
] as const;

function catalog(prefix: string, value: string, suffix = ''): string {
  return `${prefix}.${value}${suffix}`;
}

function fixed(raw: number): string {
  return (raw / 256).toLocaleString('en-US', { maximumFractionDigits: 1 });
}

function measured(raw: number): string {
  return (raw / 65_536).toLocaleString('en-US', { maximumFractionDigits: 2 });
}

function percent(raw: number): string {
  return `${(raw * 100 / 65_536).toLocaleString('en-US', { maximumFractionDigits: 1 })}%`;
}

function BenchTabs({ active, onSelect }: { active: BenchId; onSelect: (bench: BenchId) => void }) {
  return (
    <nav className="lab-tabs" aria-label={copy('lab.tabs')}>
      {BENCHES.map((bench) => (
        <button
          key={bench}
          type="button"
          className="lab-tab"
          aria-current={active === bench ? 'page' : undefined}
          onClick={() => onSelect(bench)}
        >
          {copy(catalog('lab', bench))}
        </button>
      ))}
    </nav>
  );
}

function Meter({ value, maximum, tone = 'mint' }: { value: number; maximum: number; tone?: 'mint' | 'amber' | 'violet' }) {
  return (
    <span className="lab-meter" data-tone={tone} aria-hidden="true">
      <span style={{ width: `${Math.min(100, Math.max(0, value * 100 / Math.max(1, maximum)))}%` }} />
    </span>
  );
}

function ObserveBench({
  form,
  protocol,
  setProtocol,
  reading,
  lensPacket,
  sampleLens,
}: {
  form: FormId;
  protocol: ObservationProtocol;
  setProtocol: (protocol: ObservationProtocol) => void;
  reading: InstrumentReading | null;
  lensPacket: LensSensorPacket | null;
  sampleLens: () => void;
}) {
  const samples = reading?.samples ?? [];
  const upkeepPurposes = ['boundary', 'repair', 'replacement', 'movement', 'reserve'] as const;
  const maxSample = Math.max(1, ...samples.map((sample) => Math.abs(sample)));
  const lens = form === 'lens' && lensPacket ? {
    sensorRadius: lensPacket.sensor_radius / 65_536,
    sensedNodes: lensPacket.node_ids,
    sensedRoutes: lensPacket.route_ids,
    horizon: lensPacket.horizon,
    points: lensPacket.points,
  } : null;
  return (
    <section className="lab-workspace lab-observe" aria-labelledby="lab-title">
      <div className="lab-controls">
        <p className="lab-kicker">{copy('lab.passive_protocol')}</p>
        <h2 id="lab-title">{copy('lab.observe')}</h2>
        <label>
          <span>{copy('lab.instrument')}</span>
          <select
            value={protocol.instrument}
            onChange={(event) => setProtocol({ ...protocol, instrument: event.target.value as InstrumentId })}
          >
            {INSTRUMENTS.map((instrument) => (
              <option value={instrument} key={instrument}>{copy(catalog('instrument', instrument))}</option>
            ))}
          </select>
        </label>
        <label>
          <span>{copy('label.measurement_grain')}</span>
          <select
            value={protocol.resolution}
            onChange={(event) => setProtocol({ ...protocol, resolution: Number(event.target.value) })}
          >
            {[1, 2, 4, 8, 16, 32].map((grain) => <option value={grain} key={grain}>{grain}</option>)}
          </select>
        </label>
        <label>
          <span>{copy('label.analysis_window')}</span>
          <input
            type="range"
            min="15"
            max="180"
            step="15"
            value={protocol.window}
            onChange={(event) => setProtocol({ ...protocol, window: Number(event.target.value) })}
          />
          <output>{protocol.window} {copy('unit.steps')}</output>
        </label>
        <label>
          <span>{copy('label.comparison_neighborhood')}</span>
          <select
            value={protocol.surround}
            onChange={(event) => setProtocol({ ...protocol, surround: event.target.value as Surround })}
          >
            {SURROUNDS.map((surround) => (
              <option value={surround} key={surround}>{copy(catalog('label', `surround_${surround}`))}</option>
            ))}
          </select>
        </label>
        <p className="lab-disclosure">{copy('lab.observe_disclosure')}</p>
        {form === 'lens' ? (
          <button type="button" className="lab-primary" onClick={sampleLens}>
            {copy('lab.sample_lens')}
          </button>
        ) : null}
      </div>
      <div className="lab-results">
        <header className="lab-result-head">
          <div>
            <p>{copy(catalog('instrument', protocol.instrument))}</p>
            <h3>{protocol.instrument === 'response_lag'
              ? `${measured(reading?.primary ?? 0)} ${copy('unit.steps')}`
              : measured(reading?.primary ?? 0)}</h3>
          </div>
          <dl>
            <div><dt>{copy('lab.samples')}</dt><dd>{samples.length}</dd></div>
            <div><dt>{copy('lab.minimum')}</dt><dd>{measured(reading?.secondary ?? 0)}</dd></div>
            <div><dt>{copy('lab.agreement')}</dt><dd>{percent(reading?.agreement ?? 0)}</dd></div>
          </dl>
        </header>
        <ol className="lab-sample-list">
          {samples.map((sample, place) => (
            <li key={place}>
              <span>{protocol.instrument === 'maintenance_allocation'
                ? copy(catalog('upkeep', upkeepPurposes[place] ?? 'boundary'))
                : String(place + 1).padStart(2, '0')}</span>
              <Meter value={Math.abs(sample)} maximum={maxSample} />
              <output>{measured(sample)}</output>
            </li>
          ))}
        </ol>
        {lens ? (
          <section className="lab-lens-forecast">
            <header><div><p>{copy('lab.lens_local_forecast')}</p><h3>{lens.horizon} {copy('unit.steps')}</h3></div><dl><div><dt>{copy('lab.sensor_radius')}</dt><dd>{lens.sensorRadius}</dd></div><div><dt>{copy('lab.sensed_nodes')}</dt><dd>{lens.sensedNodes.length}</dd></div></dl></header>
            <div className="lab-forecast-band">
              {lens.points.map((point) => {
                const maximum = Math.max(1, ...lens.points.map((held) => held.high));
                return <span key={point.step} title={String(point.step)}><i style={{ bottom: `${point.low * 100 / maximum}%`, height: `${(point.high - point.low) * 100 / maximum}%` }} /><b style={{ bottom: `${point.expected * 100 / maximum}%` }} /></span>;
              })}
            </div>
            <p className="lab-disclosure">{copy('lab.lens_disclosure')}</p>
          </section>
        ) : null}
      </div>
    </section>
  );
}

function InterveneBench({
  frame,
  plan,
  setPlan,
  stage,
  staged,
}: {
  frame: FrameState;
  plan: InterventionPlan;
  setPlan: (plan: InterventionPlan) => void;
  stage: () => void;
  staged: boolean;
}) {
  const targetKind = TOOL_TARGET[plan.tool];
  const targets = targetKind === 'route'
    ? frame.routes.map((route) => route.route)
    : targetKind === 'supply' || targetKind === 'input'
      ? frame.currents.map((current) => current.id)
    : targetKind === 'node' || targetKind === 'component'
      ? frame.ports.map((port) => port.node)
      : [];
  return (
    <section className="lab-workspace lab-intervene" aria-labelledby="lab-title">
      <div className="lab-controls">
        <p className="lab-kicker">{copy('lab.typed_causal_edit')}</p>
        <h2 id="lab-title">{copy('lab.intervene')}</h2>
        <div className="lab-tool-grid" role="radiogroup" aria-label={copy('lab.tool')}>
          {INTERVENTIONS.map((tool) => (
            <button
              type="button"
              role="radio"
              aria-checked={plan.tool === tool}
              key={tool}
              onClick={() => setPlan({
                ...plan,
                tool,
                target: 0,
                scope: LIVE_INTERVENTIONS.includes(tool) ? plan.scope : 'replay',
              })}
            >
              <span>{copy(catalog('intervention', tool))}</span>
              <small>{copy(catalog('intervention', tool, '.subtitle'))}</small>
            </button>
          ))}
        </div>
      </div>
      <div className="lab-results lab-plan">
        <header className="lab-result-head">
          <div>
            <p>{copy(catalog('intervention', plan.tool, '.subtitle'))}</p>
            <h3>{copy(catalog('intervention', plan.tool))}</h3>
          </div>
          <span className="lab-status" data-ready={staged}>{copy(staged ? 'lab.staged' : 'lab.draft')}</span>
        </header>
        <div className="lab-parameter-grid">
          {targets.length > 0 ? (
            <label>
              <span>{copy(catalog('target', targetKind))}</span>
              <select value={plan.target} onChange={(event) => setPlan({ ...plan, target: Number(event.target.value) })}>
                <option value="0">{copy('lab.automatic_target')}</option>
                {targets.map((target) => <option value={target} key={target}>{target}</option>)}
              </select>
            </label>
          ) : (
            <div className="lab-fixed-target">
              <span>{copy(catalog('target', targetKind))}</span>
              <strong>{copy('lab.current_causal_target')}</strong>
            </div>
          )}
          {plan.tool === 'decoy' ? (
            <label>
              <span>{copy('target.node')}</span>
              <select value={plan.receiver} onChange={(event) => setPlan({ ...plan, receiver: Number(event.target.value) })}>
                <option value="0">{copy('lab.automatic_target')}</option>
                {frame.ports.map((port) => <option value={port.node} key={port.node}>{port.node}</option>)}
              </select>
            </label>
          ) : null}
          {plan.tool === 'replace' ? (
            <fieldset className="lab-transfer-policy">
              <legend>{copy('lab.transfer_policy')}</legend>
              {REPLACEMENT_TRANSFERS.map(([name, bit]) => (
                <label key={name}>
                  <input
                    type="checkbox"
                    checked={(plan.transferMask & bit) !== 0}
                    onChange={() => setPlan({ ...plan, transferMask: plan.transferMask ^ bit })}
                  />
                  <span>{copy(catalog('lab.transfer', name))}</span>
                </label>
              ))}
            </fieldset>
          ) : null}
          {plan.tool === 'transplant' ? (
            <label>
              <span>{copy('lab.destination_regime')}</span>
              <select value={plan.destination} onChange={(event) => setPlan({ ...plan, destination: event.target.value as RegimeId })}>
                {(['open_field', 'periodic_transport', 'crowded_medium', 'vestige_pressure', 'holdout_atmosphere'] as RegimeId[]).map((id) => (
                  <option value={id} key={id}>{copy(catalog('regime', id))}</option>
                ))}
              </select>
            </label>
          ) : null}
          <label>
            <span>{copy('lab.scope')}</span>
            <select value={plan.scope} onChange={(event) => setPlan({ ...plan, scope: event.target.value as InterventionPlan['scope'] })}>
              <option value="replay">{copy('lab.scope.replay')}</option>
              {LIVE_INTERVENTIONS.includes(plan.tool) ? (
                <option value="live">{copy('lab.scope.live')}</option>
              ) : null}
            </select>
          </label>
          {plan.scope === 'replay' ? (
            <label>
              <span>{copy('lab.onset')}</span>
              <input type="number" min="0" max="900" value={plan.onset} onChange={(event) => setPlan({ ...plan, onset: Number(event.target.value) })} />
            </label>
          ) : null}
          {plan.tool !== 'blade' && plan.tool !== 'transplant' ? (
            <label>
              <span>{copy('lab.duration')}</span>
              <input type="number" min="1" max="1800" value={plan.duration} onChange={(event) => setPlan({ ...plan, duration: Number(event.target.value) })} />
            </label>
          ) : null}
          {plan.tool !== 'blade' && plan.tool !== 'delay' && plan.tool !== 'replace' && plan.tool !== 'transplant' ? (
            <label>
              <span>{copy(catalog('intervention', plan.tool, '.amount'))}</span>
              <input type="range" min="5" max="95" step="5" value={plan.amount} onChange={(event) => setPlan({ ...plan, amount: Number(event.target.value) })} />
              <output>{plan.amount}{copy('unit.percent')}</output>
            </label>
          ) : null}
        </div>
        <p className="lab-disclosure">{copy(catalog('intervention', plan.tool, '.disclosure'))}</p>
        <button type="button" className="lab-primary" onClick={stage}>{copy('lab.stage_intervention')}</button>
      </div>
    </section>
  );
}

function DivergenceBench({ result, run }: { result: DivergenceResult | null; run: () => void }) {
  const maximum = Math.max(1, ...(result?.points.flatMap((point) => [point.baseline, point.changed]) ?? [1]));
  return (
    <section className="lab-workspace" aria-labelledby="lab-title">
      <div className="lab-controls">
        <p className="lab-kicker">{copy('lab.common_randomness')}</p>
        <h2 id="lab-title">{copy('lab.divergence')}</h2>
        <p className="lab-description">{copy('lab.divergence_description')}</p>
        <button type="button" className="lab-primary" onClick={run}>{copy('lab.run_paired_replay')}</button>
      </div>
      <div className="lab-results">
        {result ? (
          <>
            <div className="lab-traces" aria-label={copy('lab.paired_traces')}>
              {result.points.map((point) => (
                <span key={point.step} className="lab-trace-column">
                  <i style={{ height: `${point.baseline * 100 / maximum}%` }} data-trace="base" />
                  <i style={{ height: `${point.changed * 100 / maximum}%` }} data-trace="changed" />
                </span>
              ))}
            </div>
            <ol className="lab-timeline">
              <li><time>{result.firstStep}</time><span>{copy('lab.timeline.intervention')}</span></li>
              <li><time>{result.outletFloorStep}</time><span>{copy('lab.timeline.outlet')}</span></li>
              <li><time>{result.reserveFloorStep}</time><span>{copy('lab.timeline.reserve')}</span></li>
              <li><time>{result.criterionStep}</time><span>{copy('lab.timeline.criterion')}</span></li>
            </ol>
            <p className="lab-disclosure">{copy('lab.first_divergence_disclosure')}</p>
          </>
        ) : <p className="lab-empty">{copy('lab.no_replay')}</p>}
      </div>
    </section>
  );
}

function EnsembleTable({
  result,
  sealed = false,
  selectedSeed = null,
  onSelect,
}: {
  result: EnsembleResult;
  sealed?: boolean;
  selectedSeed?: number | null;
  onSelect?: (seed: number) => void;
}) {
  const maximum = Math.max(1, result.high);
  const selectable = onSelect !== undefined;
  return (
    <>
      <header className="lab-result-head lab-summary">
        <dl>
          <div><dt>{copy('lab.pass_fraction')}</dt><dd>{result.passCount}/{result.trials.length}</dd></div>
          <div><dt>{copy('lab.median')}</dt><dd>{result.median}</dd></div>
          <div><dt>{copy('lab.observed_range')}</dt><dd>{result.low}-{result.high}</dd></div>
        </dl>
      </header>
      <ol className="lab-trials">
        {result.trials.map((trial, place) => {
          const contents = (
            <>
              <span>{String(place + 1).padStart(2, '0')}</span>
              <code>{sealed ? copy('lab.sealed_seed') : trial.seed.toString(16).slice(0, 8)}</code>
              <Meter value={trial.value} maximum={maximum} tone={trial.passed ? 'mint' : 'amber'} />
              <output>{trial.value}</output>
              <small>{copy(catalog('failure', trial.failure))}</small>
            </>
          );
          return (
            <li
              key={trial.seed}
              data-passed={trial.passed}
              data-selectable={selectable || undefined}
              data-selected={selectable && selectedSeed === trial.seed ? true : undefined}
            >
              {selectable ? (
                <button
                  type="button"
                  className="lab-trial-select"
                  aria-pressed={selectedSeed === trial.seed}
                  onClick={() => onSelect?.(trial.seed)}
                >
                  {contents}
                </button>
              ) : contents}
            </li>
          );
        })}
      </ol>
    </>
  );
}

function EnsembleBench({ result, run }: { result: EnsembleResult | null; run: () => void }) {
  return (
    <section className="lab-workspace" aria-labelledby="lab-title">
      <div className="lab-controls">
        <p className="lab-kicker">{copy('lab.descriptive_experiment')}</p>
        <h2 id="lab-title">{copy('lab.ensemble')}</h2>
        <p className="lab-description">{copy('lab.ensemble_description')}</p>
        <button type="button" className="lab-primary" onClick={run}>{copy('lab.compile_twelve')}</button>
      </div>
      <div className="lab-results">
        {result ? <EnsembleTable result={result} /> : <p className="lab-empty">{copy('lab.no_ensemble')}</p>}
        <p className="lab-disclosure">{copy('lab.range_disclosure')}</p>
      </div>
    </section>
  );
}

function HoldoutBench({ result, suite, suites, seal, run, retire }: {
  result: EnsembleResult | null;
  suite: HoldoutSuite | null;
  suites: HoldoutSuite[];
  seal: () => void;
  run: () => void;
  retire: () => void;
}) {
  const [selectedSeed, setSelectedSeed] = useState<number | null>(null);
  const firstFailure = result?.trials.find((trial) => !trial.passed);
  const selectedTrial = result?.trials.find((trial) => trial.seed === selectedSeed)
    ?? firstFailure
    ?? result?.trials[0]
    ?? null;
  const selectedPlace = selectedTrial && result
    ? result.trials.findIndex((trial) => trial.seed === selectedTrial.seed)
    : -1;
  const traceMaximum = Math.max(1, ...(selectedTrial?.trace ?? [1]));
  const runnable = suite?.status === 'sealed';
  const weakestRoute = selectedTrial?.criterion?.routes.reduce<CriterionReading['routes'][number] | null>(
    (weakest, route) => !weakest || route.mean - route.floor < weakest.mean - weakest.floor
      ? route
      : weakest,
    null,
  ) ?? null;
  const weakestComponent = selectedTrial?.criterion?.components.reduce<CriterionReading['components'][number] | null>(
    (weakest, component) => !weakest || component.margin < weakest.margin
      ? component
      : weakest,
    null,
  ) ?? null;
  return (
    <section className="lab-workspace lab-holdout" data-sealed={suite !== null} aria-labelledby="lab-title">
      <div className="lab-controls">
        <p className="lab-kicker">{copy('lab.hands_off')}</p>
        <h2 id="lab-title">{copy('lab.holdout')}</h2>
        {suite ? (
          <div className="lab-control-seal">
            <span>{copy('lab.control_withdrawn')}</span>
            <code>{suite.hiddenSuiteVersionHash.slice(0, 12)}</code>
          </div>
        ) : null}
        <dl className="lab-criteria">
          <div><dt>{copy('lab.control_mode')}</dt><dd>{copy('lab.control.hands_off')}</dd></div>
          <div><dt>{copy('lab.trials')}</dt><dd>8</dd></div>
          <div><dt>{copy('lab.required_passes')}</dt><dd>7</dd></div>
          <div><dt>{copy('lab.editing')}</dt><dd>{copy(suite ? 'lab.locked' : 'lab.editing_open')}</dd></div>
          <div><dt>{copy('lab.suite_status')}</dt><dd>{copy(catalog('lab.holdout_status', suite?.status ?? 'draft'))}</dd></div>
          <div><dt>{copy('lab.suite_version')}</dt><dd><code>{suite?.hiddenSuiteVersionHash?.slice(0, 12) ?? '-'}</code></dd></div>
        </dl>
        {!suite ? (
          <button type="button" className="lab-primary" onClick={seal}>{copy('lab.seal_scenario')}</button>
        ) : runnable ? (
          <button type="button" className="lab-primary" onClick={run}>{copy('lab.run_holdout')}</button>
        ) : suite.status !== 'retired' ? (
          <button type="button" className="lab-primary" onClick={retire}>{copy('lab.retire_suite')}</button>
        ) : null}
        {suite && runnable ? (
          <button type="button" className="lab-secondary" onClick={retire}>{copy('lab.retire_suite')}</button>
        ) : null}
        <ol className="lab-suite-register">
          {suites.slice(0, 6).map((held) => (
            <li key={held.id} data-status={held.status}>
              <code>{held.id.slice(-10)}</code>
              <span>{copy(catalog('lab.holdout_status', held.status))}</span>
              <strong>{held.passed === null ? '-' : `${held.passed}/${held.trials}`}</strong>
            </li>
          ))}
        </ol>
      </div>
      <div className="lab-results">
        {result ? (
          <>
            <EnsembleTable
              result={result}
              sealed
              selectedSeed={selectedTrial?.seed ?? null}
              onSelect={setSelectedSeed}
            />
            {selectedTrial ? (
              <div className="lab-holdout-preview" data-passed={selectedTrial.passed}>
                <header>
                  <div>
                    <p>{copy('lab.recorded_trace')}</p>
                    <h3>{copy('lab.sealed_condition')} {String(selectedPlace + 1).padStart(2, '0')}</h3>
                  </div>
                  <strong>{copy(selectedTrial.passed ? 'lab.pass' : 'lab.fail')}</strong>
                </header>
                <div className="lab-traces lab-single-trace" aria-label={copy('lab.recorded_trace')}>
                  {selectedTrial.trace.map((value, step) => (
                    <span key={step} className="lab-trace-column" title={`${copy('lab.step')} ${step + 1}: ${value}`}>
                      <i style={{ height: `${value * 100 / traceMaximum}%` }} data-trace="selected" />
                    </span>
                  ))}
                </div>
                <dl>
                  <div><dt>{copy('lab.condition')}</dt><dd>{copy('lab.sealed_condition')} {String(selectedPlace + 1).padStart(2, '0')}</dd></div>
                  <div><dt>{copy('lab.outcome')}</dt><dd>{copy(selectedTrial.passed ? 'lab.pass' : 'lab.fail')}</dd></div>
                  <div><dt>{copy('lab.failure_reason')}</dt><dd>{copy(catalog('failure', selectedTrial.failure))}</dd></div>
                  <div><dt>{copy('lab.trailing_throughput')}</dt><dd>{selectedTrial.value}</dd></div>
                  <div><dt>{copy('lab.criterion')}</dt><dd>{copy('lab.criterion.vector')} / {copy(catalog('criterion.status', selectedTrial.criterionStatus))}</dd></div>
                  <div><dt>{copy('criterion.route')}</dt><dd>{weakestRoute ? `${measured(weakestRoute.mean)} / ${measured(weakestRoute.floor)} ${copy('unit.cu_per_step')}` : '-'}</dd></div>
                  <div><dt>{copy('criterion.component')}</dt><dd>{weakestComponent ? `${measured(weakestComponent.margin)} ${copy('unit.cu')}` : '-'}</dd></div>
                  <div><dt>{copy('criterion.leakage')}</dt><dd>{selectedTrial.criterion?.leakage.ratio === null || selectedTrial.criterion?.leakage.ratio === undefined ? '-' : `${percent(selectedTrial.criterion.leakage.ratio)} / ${percent(selectedTrial.criterion.leakage.ceiling)}`}</dd></div>
                  <div><dt>{copy('lab.branch_point')}</dt><dd>{copy('lab.last_anchor')}</dd></div>
                </dl>
                <p>{copy('lab.trace_preview_disclosure')}</p>
              </div>
            ) : null}
          </>
        ) : <p className="lab-empty">{copy(suite ? 'lab.holdout_ready' : 'lab.holdout_unsealed')}</p>}
      </div>
    </section>
  );
}

function ArchiveBench({
  scenarioId,
  records,
  exportRun,
  reopen,
  remove,
}: {
  scenarioId: string;
  records: ArchiveRecord[];
  exportRun: () => void;
  reopen: (record: ArchiveRecord) => void;
  remove: (record: ArchiveRecord) => void;
}) {
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [compareId, setCompareId] = useState<string | null>(null);
  const selected = records.find((record) => record.id === selectedId) ?? records[0] ?? null;
  const compared = records.find((record) => record.id === compareId) ?? null;
  const lineages = useMemo(() => {
    const grouped = new Map<string, ArchiveRecord[]>();
    for (const record of records) {
      const held = grouped.get(record.runId) ?? [];
      held.push(record);
      grouped.set(record.runId, held);
    }
    return [...grouped.entries()].map(([runId, branches]) => ({
      runId,
      branches: branches.sort((left, right) =>
        left.branchNonce - right.branchNonce || left.createdAt - right.createdAt),
    }));
  }, [records]);

  return (
    <section className="lab-workspace" aria-labelledby="lab-title">
      <div className="lab-controls">
        <p className="lab-kicker">{copy('lab.reproducible_record')}</p>
        <h2 id="lab-title">{copy('lab.archive')}</h2>
        <p className="lab-description">{copy('lab.archive_description')}</p>
        <button type="button" className="lab-primary" onClick={exportRun}>{copy('lab.archive_current')}</button>
      </div>
      <div className="lab-results lab-archive-list">
        <div><span>{copy('lab.scenario_spec')}</span><code>{scenarioId}</code><strong>{copy('lab.recorded')}</strong></div>
        <div><span>{copy('lab.generator_spec')}</span><code>{copy('lab.immutable')}</code><strong>{copy('lab.recorded')}</strong></div>
        <div><span>{copy('lab.initial_state')}</span><code>{copy('lab.embodied')}</code><strong>{copy('lab.recorded')}</strong></div>
        <div><span>{copy('lab.controls')}</span><code>{copy('lab.open_loop')}</code><strong>{copy('lab.recorded')}</strong></div>
        <div><span>{copy('lab.interventions')}</span><code>{copy('lab.typed')}</code><strong>{copy('lab.recorded')}</strong></div>
        <div><span>{copy('lab.evidence')}</span><code>{copy('lab.measurements')}</code><strong>{copy('lab.ready')}</strong></div>
        {lineages.length > 0 ? (
          <div className="lab-lineage-surface">
            <header>
              <span>{copy('lab.lineage')}</span>
              <small>{records.length} {copy('lab.records')}</small>
            </header>
            {lineages.map((lineage) => (
              <section className="lab-lineage-run" key={lineage.runId}>
                <h3>{lineage.runId.slice(0, 12)}</h3>
                <ol>
                  {lineage.branches.map((record) => (
                    <li key={record.id}>
                      <time dateTime={new Date(record.createdAt).toISOString()}>
                        {new Date(record.createdAt).toLocaleDateString(undefined, { month: 'short', day: 'numeric' })}
                      </time>
                      <button
                        type="button"
                        className="lab-lineage-record"
                        data-selected={selected?.id === record.id}
                        aria-pressed={selected?.id === record.id}
                        style={{ gridColumn: Math.min(6, record.branchNonce + 2) }}
                        onClick={() => setSelectedId(record.id)}
                      >
                        <span>{copy('lab.branch')} {record.branchNonce}</span>
                        <strong>{record.form}</strong>
                        <small>{record.regime} / {copy('lab.step')} {record.step}</small>
                      </button>
                    </li>
                  ))}
                </ol>
              </section>
            ))}
          </div>
        ) : <p className="lab-empty">{copy('lab.no_archive_records')}</p>}
        {selected ? (
          <section className="lab-lineage-inspector">
            <header>
              <div><p>{copy('lab.selected_branch')}</p><h3>{selected.runId.slice(0, 12)} / {selected.branchNonce}</h3></div>
              <code>{selected.generatorHash.slice(0, 16)}</code>
            </header>
            <dl>
              <div><dt>{copy('lab.regime')}</dt><dd>{selected.regime}</dd></div>
              <div><dt>{copy('lab.form')}</dt><dd>{selected.form}</dd></div>
              <div><dt>{copy('lab.control_contract')}</dt><dd>{selected.control}</dd></div>
              <div><dt>{copy('lab.lawset')}</dt><dd>{selected.lawsetVersion ?? '-'}</dd></div>
              <div><dt>{copy('lab.protocol')}</dt><dd>{selected.protocolVersion ?? '-'}</dd></div>
              <div><dt>{copy('lab.rng')}</dt><dd>{selected.rngAlgorithm ?? '-'}</dd></div>
              <div><dt>{copy('lab.analysis_protocol')}</dt><dd><code>{selected.analysisProtocolHash ?? '-'}</code></dd></div>
              <div><dt>{copy('lab.evidence')}</dt><dd>{selected.evidence.length}</dd></div>
            </dl>
            <label className="lab-compare-select">
              <span>{copy('lab.compare_record')}</span>
              <select value={compareId ?? ''} onChange={(event) => setCompareId(event.target.value || null)}>
                <option value="">{copy('lab.compare_none')}</option>
                {records.filter((record) => record.id !== selected.id).map((record) => (
                  <option key={record.id} value={record.id}>
                    {record.runId.slice(0, 8)} / {copy('lab.branch')} {record.branchNonce} / {copy('lab.step')} {record.step}
                  </option>
                ))}
              </select>
            </label>
            {compared ? (
              <section className="lab-record-comparison">
                <header><span>{copy('lab.comparison')}</span><code>{compared.runId.slice(0, 12)}</code></header>
                <dl>
                  <div data-match={selected.generatorHash === compared.generatorHash}><dt>{copy('lab.generator_spec')}</dt><dd>{copy(selected.generatorHash === compared.generatorHash ? 'lab.same' : 'lab.different')}</dd></div>
                  <div data-match={selected.scenarioHash === compared.scenarioHash}><dt>{copy('lab.scenario_spec')}</dt><dd>{copy(selected.scenarioHash === compared.scenarioHash ? 'lab.same' : 'lab.different')}</dd></div>
                  <div data-match={selected.embodiedStateHash === compared.embodiedStateHash}><dt>{copy('lab.initial_state')}</dt><dd>{copy(selected.embodiedStateHash === compared.embodiedStateHash ? 'lab.same' : 'lab.different')}</dd></div>
                  <div data-match={selected.controlHash === compared.controlHash}><dt>{copy('lab.controls')}</dt><dd>{copy(selected.controlHash === compared.controlHash ? 'lab.same' : 'lab.different')}</dd></div>
                  <div><dt>{copy('lab.step_delta')}</dt><dd>{selected.step - compared.step}</dd></div>
                  <div><dt>{copy('lab.evidence_delta')}</dt><dd>{selected.evidence.reduce((sum, item) => sum + item.passed, 0) - compared.evidence.reduce((sum, item) => sum + item.passed, 0)}</dd></div>
                </dl>
              </section>
            ) : null}
            <div className="lab-evidence-vector">
              {selected.evidence.map((evidence, index) => (
                <div key={`${evidence.artifact}:${index}`} data-passed={evidence.passed >= evidence.trials}>
                  <span>{copy(catalog('lab.evidence_kind', evidence.kind))}</span>
                  <strong>{evidence.passed}/{evidence.trials}</strong>
                  <code>{evidence.artifact.slice(0, 18)}</code>
                </div>
              ))}
            </div>
            <footer>
              <button type="button" className="lab-primary" onClick={() => reopen(selected)}>{copy('lab.reopen_branch')}</button>
              <button type="button" onClick={() => remove(selected)}>{copy('lab.remove_record')}</button>
            </footer>
          </section>
        ) : null}
      </div>
    </section>
  );
}

function RenewalBench({ inventory, result, run }: { inventory: RenewalInventory | null; result: RenewalResult[] | null; run: () => void }) {
  const passed = result?.filter((trial) => trial.passed).length ?? 0;
  return (
    <section className="lab-workspace" aria-labelledby="lab-title">
      <div className="lab-controls">
        <p className="lab-kicker">{copy('lab.local_rules_only')}</p>
        <h2 id="lab-title">{copy('lab.renewal')}</h2>
        <p className="lab-description">{copy('lab.renewal_description')}</p>
        <button type="button" className="lab-primary" onClick={run}>{copy('lab.run_renewal')}</button>
      </div>
      <div className="lab-results">
        <section className="lab-renewal-inventory">
          <header><div><p>{copy('lab.embodied_inventory')}</p><h3>{inventory?.materials.filter((material) => !material.claimed).length ?? 0} / {inventory?.materials.length ?? 0}</h3></div><dl><div><dt>{copy('lab.local_signals')}</dt><dd>{inventory?.signals.length ?? 0}</dd></div><div><dt>{copy('lab.step')}</dt><dd>{inventory?.step ?? 0}</dd></div></dl></header>
          <div className="lab-material-rail">
            {inventory?.materials.map((material) => (
              <div key={material.material} data-kind={material.kind} data-claimed={material.claimed}>
                <i aria-hidden="true" />
                <code>M{material.material}</code>
                <strong>{copy(catalog('material', material.kind))}</strong>
                <span>L{material.layer} · {Math.round(material.x / 65_536)}, {Math.round(material.y / 65_536)}</span>
              </div>
            ))}
          </div>
          <ol className="lab-signal-register">
            {inventory?.signals.map((signal) => (
              <li key={signal.signal}>
                <code>S{signal.signal}</code>
                <span>{signal.source} → {signal.target}</span>
                <Meter value={signal.expires_step - (inventory?.step ?? 0)} maximum={Math.max(1, signal.expires_step - signal.emitted_step)} tone="violet" />
                <output>{signal.expires_step - (inventory?.step ?? 0)}</output>
              </li>
            ))}
          </ol>
        </section>
        {result ? (
          <>
            <header className="lab-result-head"><div><p>{copy('lab.pass_fraction')}</p><h3>{passed}/{result.length}</h3></div></header>
            <div className="lab-provenance-strip">
              <span>{copy('lab.control_contract')}<code>{result[0]?.controlContract}</code></span>
              <span>{copy('lab.generator_spec')}<code>{result[0]?.generatorHash.slice(0, 16)}</code></span>
              <span>{copy('lab.scenario_spec')}<code>{result[0]?.scenarioHash.slice(0, 16)}</code></span>
              <span>{copy('lab.initial_state')}<code>{result[0]?.embodiedStateHash.slice(0, 16)}</code></span>
            </div>
            <ol className="lab-renewal-trials">
              {result.map((trial, place) => (
                <li key={trial.seed} data-passed={trial.passed}>
                  <span>{String(place + 1).padStart(2, '0')}</span>
                  <dl>
                    <div><dt>{copy('lab.detected')}</dt><dd>{trial.detectedAt}</dd></div>
                    <div><dt>{copy('lab.recruited')}</dt><dd>{trial.recruitedAt}</dd></div>
                    <div><dt>{copy('lab.reconnected')}</dt><dd>{trial.reconnectedAt}</dd></div>
                    <div><dt>{copy('lab.recovered')}</dt><dd>{trial.recoveredAt}</dd></div>
                    <div><dt>{copy('lab.resource_cost')}</dt><dd>{trial.resourceCost}</dd></div>
                    <div><dt>{copy('lab.material_cost')}</dt><dd>{trial.materialCost}</dd></div>
                    <div><dt>{copy('lab.material_ids')}</dt><dd>{trial.materialIds.join(', ') || copy('lab.none')}</dd></div>
                    <div><dt>{copy('lab.signal_id')}</dt><dd>{trial.signalId ?? copy('lab.none')}</dd></div>
                    <div><dt>{copy('lab.rebuilt_routes')}</dt><dd>{trial.rebuiltRoutes.join(', ') || copy('lab.none')}</dd></div>
                    <div><dt>{copy('lab.reconnection')}</dt><dd>{trial.reconnection}{copy('unit.percent')}</dd></div>
                    <div><dt>{copy('lab.failed_node')}</dt><dd>{trial.failedNode}</dd></div>
                    <div><dt>{copy('lab.replacement_node')}</dt><dd>{trial.replacementNode ?? copy('lab.none')}</dd></div>
                  </dl>
                </li>
              ))}
            </ol>
          </>
        ) : <p className="lab-empty">{copy('lab.no_renewal')}</p>}
        <p className="lab-disclosure">{copy('lab.renewal_disclosure')}</p>
      </div>
    </section>
  );
}

function InheritanceBench({ result, run }: { result: InheritanceResult | null; run: () => void }) {
  return (
    <section className="lab-workspace" aria-labelledby="lab-title">
      <div className="lab-controls">
        <p className="lab-kicker">{copy('lab.external_harness')}</p>
        <h2 id="lab-title">{copy('lab.inheritance')}</h2>
        <p className="lab-description">{copy('lab.inheritance_description')}</p>
        <button type="button" className="lab-primary" onClick={run}>{copy('lab.run_inheritance')}</button>
      </div>
      <div className="lab-results">
        {result ? (
          <div className="lab-inheritance">
            <header>
              <span>{copy('lab.source_state')}</span>
              <strong>{result.sourceComponents} / {result.sourceRoutes}</strong>
              <code>{result.copiedSpecification}</code>
            </header>
            <div className="lab-partition-line"><span>{copy('lab.external_copy_partition')}</span></div>
            <div className="lab-child-grid">
              {result.children.map((child) => (
                <article key={child.id} data-passed={child.passed}>
                  <p>{copy('lab.resulting_trial')} {child.id}</p>
                  <h3>{copy(child.passed ? 'lab.pass' : 'lab.fail')}</h3>
                  <dl>
                    <div><dt>{copy('lab.components')}</dt><dd>{child.inheritedComponents}</dd></div>
                    <div><dt>{copy('lab.routes')}</dt><dd>{child.inheritedRoutes}</dd></div>
                    <div><dt>{copy('lab.initial_charge')}</dt><dd>{fixed(child.initialCharge)}</dd></div>
                    <div><dt>{copy('lab.recovered')}</dt><dd>{child.recoveredAt ?? copy('lab.not_recovered')}</dd></div>
                    <div><dt>{copy('lab.criterion_margin')}</dt><dd>{child.criterionMargin}</dd></div>
                  </dl>
                  <div className="lab-inherited-identities">
                    <span>{copy('lab.components')}</span><code>{child.componentIds.join(', ')}</code>
                    <span>{copy('lab.routes')}</span><code>{child.routeIds.join(', ') || '-'}</code>
                  </div>
                </article>
              ))}
            </div>
            <p className="lab-disclosure">{copy('lab.inheritance_disclosure')}</p>
          </div>
        ) : <p className="lab-empty">{copy('lab.no_inheritance')}</p>}
      </div>
    </section>
  );
}

function OpenFieldBench({
  draft,
  setDraft,
  compiled,
  compile,
  result,
  run,
}: {
  draft: OpenFieldDraft;
  setDraft: (draft: OpenFieldDraft) => void;
  compiled: CompiledOpenField | null;
  compile: () => void;
  result: OpenFieldRun | null;
  run: () => void;
}) {
  const update = <K extends keyof OpenFieldDraft>(key: K, value: OpenFieldDraft[K]) => {
    setDraft({ ...draft, [key]: value });
  };
  const addComponent = () => {
    const node = Math.max(0, ...draft.components.map((component) => component.node), ...draft.routes.flatMap((route) => [route.tail, route.head])) + 1;
    update('components', [...draft.components, { node, kind: 'module', layer: 0, x: 2048, y: 2048, charge: 16, open: true, upkeepRate: 0, capacity: 64 }]);
  };
  const removeComponent = (node: number) => {
    setDraft({
      ...draft,
      components: draft.components.filter((component) => component.node !== node),
      routes: draft.routes.filter((route) => route.tail !== node && route.head !== node),
      compartmentMembers: draft.compartmentMembers.filter((member) => member !== node),
    });
  };
  const addRoute = () => {
    const route = Math.max(0, ...draft.routes.map((held) => held.route)) + 1;
    const tail = draft.components[0]?.node ?? 1;
    const head = draft.components[1]?.node ?? tail;
    update('routes', [...draft.routes, { route, tail, head, capacity: 32 }]);
  };
  const addMaterial = () => {
    const material = Math.max(0, ...draft.materials.map((held) => held.material)) + 1;
    update('materials', [...draft.materials, {
      material,
      kind: 'junction_blank',
      amount: 1,
      layer: 0,
      x: 2048,
      y: 2048,
    }]);
  };
  return (
    <section className="lab-workspace lab-open-field" aria-labelledby="lab-title">
      <div className="lab-controls">
        <p className="lab-kicker">{copy('lab.fixed_lawset')}</p>
        <h2 id="lab-title">{copy('lab.open_field')}</h2>
        <p className="lab-description">{copy('lab.open_field_description')}</p>
        <label><span>{copy('lab.lawset')}</span><select value={draft.lawsetId} onChange={(event) => setDraft(withLawset(draft, event.target.value as OpenFieldLawsetId))}>{OPEN_FIELD_LAWSETS.map((id) => <option key={id} value={id}>{id}</option>)}</select></label>
        <label><span>{copy('lab.form')}</span><select value={draft.form} onChange={(event) => update('form', event.target.value as FormId)}>{(['thread', 'ring', 'relay', 'vault', 'lens', 'knot', 'wake', 'chorus'] as FormId[]).map((id) => <option key={id} value={id}>{copy(catalog('form', id))}</option>)}</select></label>
        <label><span>{copy('lab.control_mode')}</span><select value={draft.control} onChange={(event) => update('control', event.target.value as OpenFieldDraft['control'])}><option value="hands_off">{copy('lab.control.hands_off')}</option><option value="recorded_open_loop">{copy('lab.open_loop')}</option></select></label>
      </div>
      <div className="lab-results lab-open-field-editor">
        <div className="lab-editor-group"><h3>{copy('lab.field_rules')}</h3>
          <label><span>{copy('lab.supply_rate')}</span><input type="number" min="0" step="0.25" value={draft.supplyPerStep} onChange={(event) => update('supplyPerStep', Number(event.target.value))} /></label>
          <label><span>{copy('lab.supply_width')}</span><input type="number" min="8" step="8" value={draft.supplyWidth} onChange={(event) => update('supplyWidth', Number(event.target.value))} /></label>
          <label><span>{copy('lab.dissipation')}</span><input type="number" min="0" step="0.03125" value={draft.dissipationPerStep} onChange={(event) => update('dissipationPerStep', Number(event.target.value))} /></label>
          <label><span>{copy('lab.noise_fraction')}</span><input type="number" min="0" max="1" step="0.025" value={draft.conductanceNoise} onChange={(event) => update('conductanceNoise', Number(event.target.value))} /></label>
          <label><span>{copy('lab.compartment_leak')}</span><input type="number" min="0" max="1" step="0.015625" value={draft.compartmentLeak} onChange={(event) => update('compartmentLeak', Number(event.target.value))} /></label>
          <label><span>{copy('lab.route_capacity_scale')}</span><input type="number" min="0.125" max="1" step="0.125" value={draft.routeCapacityScale} onChange={(event) => update('routeCapacityScale', Number(event.target.value))} /></label>
          <label><span>{copy('lab.supply_layer')}</span><input type="number" min="0" max="7" value={draft.supplyLayer} onChange={(event) => update('supplyLayer', Number(event.target.value))} /></label>
          <label><span>{copy('lab.supply_x')}</span><input type="number" min="0" max="4095" value={draft.supplyX} onChange={(event) => update('supplyX', Number(event.target.value))} /></label>
          <label><span>{copy('lab.supply_y')}</span><input type="number" min="0" max="4095" value={draft.supplyY} onChange={(event) => update('supplyY', Number(event.target.value))} /></label>
        </div>
        <div className="lab-editor-group"><h3>{copy('lab.declared_function')}</h3>
          <label><span>{copy('lab.criterion_component_floor')}</span><input type="number" min="0" value={draft.criterionFloor} onChange={(event) => update('criterionFloor', Number(event.target.value))} /></label>
          <label><span>{copy('lab.criterion_route_floor')}</span><input type="number" min="0" step="0.25" value={draft.criterionRouteFloor} onChange={(event) => update('criterionRouteFloor', Number(event.target.value))} /></label>
          <label><span>{copy('lab.criterion_leakage_ceiling')}</span><input type="number" min="0" max="1" step="0.01" value={draft.criterionLeakageCeiling} onChange={(event) => update('criterionLeakageCeiling', Number(event.target.value))} /></label>
          <label><span>{copy('lab.criterion_window')}</span><input type="number" min="1" step="15" value={draft.criterionWindow} onChange={(event) => update('criterionWindow', Number(event.target.value))} /></label>
          <label><span>{copy('lab.criterion_failure_grace')}</span><input type="number" min="0" step="1" value={draft.criterionFailureGrace} onChange={(event) => update('criterionFailureGrace', Number(event.target.value))} /></label>
          <label><span>{copy('lab.criterion_hands_off')}</span><input type="number" min="1" step="15" value={draft.criterionDuration} onChange={(event) => update('criterionDuration', Number(event.target.value))} /></label>
          <label><span>{copy('label.analysis_window')}</span><input type="number" min="15" step="15" value={draft.observationWindow} onChange={(event) => update('observationWindow', Number(event.target.value))} /></label>
          <label><span>{copy('label.measurement_grain')}</span><select value={draft.observationResolution} onChange={(event) => update('observationResolution', Number(event.target.value))}>{[1, 2, 4, 8, 16, 32].map((grain) => <option value={grain} key={grain}>{grain}</option>)}</select></label>
          <label><span>{copy('lab.trials')}</span><input type="number" min="1" max="64" value={draft.trialCount} onChange={(event) => update('trialCount', Number(event.target.value))} /></label>
        </div>
        <div className="lab-editor-group"><h3>{copy('lab.interventions')}</h3>
          <label><span>{copy('lab.tool')}</span><select value={draft.intervention.tool} onChange={(event) => update('intervention', { ...draft.intervention, tool: event.target.value as OpenFieldDraft['intervention']['tool'], target: draft.intervention.target || draft.routes[0]?.route || 0 })}>
            <option value="none">{copy('lab.none')}</option>
            {(['blade', 'clamp', 'breach'] as const).map((tool) => <option value={tool} key={tool}>{copy(catalog('intervention', tool))}</option>)}
          </select></label>
          {draft.intervention.tool === 'blade' || draft.intervention.tool === 'clamp' ? <label><span>{copy('target.route')}</span><select value={draft.intervention.target} onChange={(event) => update('intervention', { ...draft.intervention, target: Number(event.target.value) })}>{draft.routes.length === 0 ? <option value="0">{copy('lab.none')}</option> : null}{draft.routes.map((route) => <option value={route.route} key={route.route}>{route.route}</option>)}</select></label> : null}
          {draft.intervention.tool !== 'none' ? <label><span>{copy('lab.onset')}</span><input type="number" min="0" max="1800" value={draft.intervention.onset} onChange={(event) => update('intervention', { ...draft.intervention, onset: Number(event.target.value) })} /></label> : null}
          {draft.intervention.tool === 'clamp' || draft.intervention.tool === 'breach' ? <label><span>{copy('lab.duration')}</span><input type="number" min="1" max="1800" value={draft.intervention.duration} onChange={(event) => update('intervention', { ...draft.intervention, duration: Number(event.target.value) })} /></label> : null}
          {draft.intervention.tool === 'clamp' || draft.intervention.tool === 'breach' ? <label><span>{copy('lab.amount')}</span><input type="range" min="1" max="95" step="1" value={draft.intervention.amount} onChange={(event) => update('intervention', { ...draft.intervention, amount: Number(event.target.value) })} /><output>{draft.intervention.amount}%</output></label> : null}
        </div>
        <section className="lab-topology-editor">
          <header><div><p>{copy('lab.encoded_side_information')}</p><h3>{copy('lab.component_placements')}</h3></div><button type="button" onClick={addComponent}>{copy('lab.add_component')}</button></header>
          <ol className="lab-component-drafts">
            {draft.components.map((component, place) => (
              <li key={component.node}>
                <code>{component.node}</code>
                <select value={component.kind} onChange={(event) => update('components', draft.components.map((held, index) => index === place ? { ...held, kind: event.target.value as typeof held.kind } : held))}>
                  {(['port', 'reserve', 'module'] as const).map((kind) => <option value={kind} key={kind}>{copy(catalog('node_kind', kind))}</option>)}
                </select>
                <label><span>X</span><input type="number" min="0" max="4095" value={component.x} onChange={(event) => update('components', draft.components.map((held, index) => index === place ? { ...held, x: Number(event.target.value) } : held))} /></label>
                <label><span>Y</span><input type="number" min="0" max="4095" value={component.y} onChange={(event) => update('components', draft.components.map((held, index) => index === place ? { ...held, y: Number(event.target.value) } : held))} /></label>
                <label><span>{copy('lab.layer')}</span><input type="number" min="0" max="7" value={component.layer} onChange={(event) => update('components', draft.components.map((held, index) => index === place ? { ...held, layer: Number(event.target.value) } : held))} /></label>
                <label><span>{copy('lab.initial_charge')}</span><input type="number" min="0" max="4096" value={component.charge} onChange={(event) => update('components', draft.components.map((held, index) => index === place ? { ...held, charge: Number(event.target.value) } : held))} /></label>
                <label><span>{copy('lab.capacity')}</span><input type="number" min="1" max="4096" value={component.capacity} onChange={(event) => update('components', draft.components.map((held, index) => index === place ? { ...held, capacity: Number(event.target.value) } : held))} /></label>
                <label><span>{copy('lab.upkeep')}</span><input type="number" min="0" step="0.25" value={component.upkeepRate} onChange={(event) => update('components', draft.components.map((held, index) => index === place ? { ...held, upkeepRate: Number(event.target.value) } : held))} /></label>
                <label className="lab-member-toggle"><input type="checkbox" checked={component.open} onChange={() => update('components', draft.components.map((held, index) => index === place ? { ...held, open: !held.open } : held))} /><span>{copy('lab.open_component')}</span></label>
                <label className="lab-member-toggle"><input type="checkbox" checked={draft.compartmentMembers.includes(component.node)} onChange={() => update('compartmentMembers', draft.compartmentMembers.includes(component.node) ? draft.compartmentMembers.filter((member) => member !== component.node) : [...draft.compartmentMembers, component.node].sort((left, right) => left - right))} /><span>{copy('lab.physical_member')}</span></label>
                <button type="button" className="lab-remove-draft" onClick={() => removeComponent(component.node)} aria-label={copy('lab.remove_component')}>×</button>
              </li>
            ))}
          </ol>
          <header><div><h3>{copy('lab.route_placements')}</h3></div><button type="button" onClick={addRoute}>{copy('lab.add_route')}</button></header>
          <ol className="lab-route-drafts">
            {draft.routes.map((route, place) => (
              <li key={route.route}>
                <code>{route.route}</code>
                <label><span>{copy('target.node')}</span><input type="number" min="1" value={route.tail} onChange={(event) => update('routes', draft.routes.map((held, index) => index === place ? { ...held, tail: Number(event.target.value) } : held))} /></label>
                <span>→</span>
                <label><span>{copy('target.node')}</span><input type="number" min="1" value={route.head} onChange={(event) => update('routes', draft.routes.map((held, index) => index === place ? { ...held, head: Number(event.target.value) } : held))} /></label>
                <label><span>{copy('lab.capacity')}</span><input type="number" min="1" max="256" value={route.capacity} onChange={(event) => update('routes', draft.routes.map((held, index) => index === place ? { ...held, capacity: Number(event.target.value) } : held))} /></label>
                <button type="button" className="lab-remove-draft" onClick={() => update('routes', draft.routes.filter((_, index) => index !== place))} aria-label={copy('lab.remove_route')}>×</button>
              </li>
            ))}
          </ol>
          <header><div><h3>{copy('lab.materials')}</h3></div><button type="button" onClick={addMaterial}>{copy('lab.add_material')}</button></header>
          <ol className="lab-material-drafts">
            {draft.materials.map((material, place) => (
              <li key={material.material}>
                <code>{material.material}</code>
                <select value={material.kind} onChange={(event) => update('materials', draft.materials.map((held, index) => index === place ? { ...held, kind: event.target.value as typeof held.kind } : held))}>
                  {(['junction_blank', 'boundary_blank', 'conductor'] as const).map((kind) => <option value={kind} key={kind}>{kind.replace('_', ' ')}</option>)}
                </select>
                <label><span>{copy('lab.amount')}</span><input type="number" min="1" max="65535" value={material.amount} onChange={(event) => update('materials', draft.materials.map((held, index) => index === place ? { ...held, amount: Number(event.target.value) } : held))} /></label>
                <label><span>X</span><input type="number" min="0" max="4095" value={material.x} onChange={(event) => update('materials', draft.materials.map((held, index) => index === place ? { ...held, x: Number(event.target.value) } : held))} /></label>
                <label><span>Y</span><input type="number" min="0" max="4095" value={material.y} onChange={(event) => update('materials', draft.materials.map((held, index) => index === place ? { ...held, y: Number(event.target.value) } : held))} /></label>
                <label><span>{copy('lab.layer')}</span><input type="number" min="0" max="7" value={material.layer} onChange={(event) => update('materials', draft.materials.map((held, index) => index === place ? { ...held, layer: Number(event.target.value) } : held))} /></label>
                <button type="button" className="lab-remove-draft" onClick={() => update('materials', draft.materials.filter((_, index) => index !== place))} aria-label={copy('lab.remove_material')}>×</button>
              </li>
            ))}
          </ol>
        </section>
        <button type="button" className="lab-primary" onClick={compile}>{copy('lab.compile_scenario')}</button>
        {compiled ? <div className="lab-compiled"><span>{copy('lab.compiled_experiment')}</span><strong>{compiled.experimentId}</strong><code>{compiled.scenarioHash}</code></div> : null}
        {compiled && draft.control === 'hands_off' ? <button type="button" className="lab-primary" onClick={run}>{copy('lab.run_open_field')}</button> : null}
        {result ? (
          <div className="lab-open-field-result">
            <header className="lab-result-head"><div><p>{copy('lab.pass_fraction')}</p><h3>{result.passed}/{result.trials.length}</h3></div><code>{result.experimentId}</code></header>
            <div className="lab-provenance-strip">
              <span>{copy('lab.control_contract')}<code>{result.controlContract}</code></span>
              <span>{copy('lab.generator_spec')}<code>{result.generatorHash.slice(0, 16)}</code></span>
              <span>{copy('lab.scenario_spec')}<code>{result.scenarioHash.slice(0, 16)}</code></span>
              <span>{copy('lab.initial_state')}<code>{result.embodiedStateHash.slice(0, 16)}</code></span>
            </div>
            <ol className="lab-trials">
              {result.trials.map((trial) => (
                <li key={trial.seed} data-passed={trial.passed}>
                  <code>{String(trial.seed + 1).padStart(2, '0')}</code>
                  <Meter value={trial.sustained_steps} maximum={draft.criterionDuration} tone={trial.passed ? 'mint' : 'amber'} />
                  <output>{trial.sustained_steps}</output>
                  <small>{copy(trial.passed ? 'lab.pass' : 'lab.fail')} / {copy(catalog('criterion.status', trial.criterion.status))}</small>
                </li>
              ))}
            </ol>
          </div>
        ) : null}
        <p className="lab-disclosure">{copy('lab.open_field_disclosure')}</p>
      </div>
    </section>
  );
}

export function ExperimentLab({ client, frame, regime, form, view, onClose }: ExperimentLabProps) {
  const [bench, setBench] = useState<BenchId>('observe');
  const [observation, setObservation] = useState(DEFAULT_OBSERVATION);
  const [plan, setPlan] = useState<InterventionPlan>({
    id: `intervention-${frame.header.step}`,
    tool: 'blade',
    scope: 'replay',
    target: frame.routes[0]?.route ?? 0,
    receiver: frame.ports[0]?.node ?? 0,
    transferMask: 111,
    destination: 'crowded_medium',
    onset: 12,
    duration: 90,
    amount: 35,
  });
  const [staged, setStaged] = useState<InterventionPlan | null>(null);
  const [divergence, setDivergence] = useState<DivergenceResult | null>(null);
  const [ensemble, setEnsemble] = useState<EnsembleResult | null>(null);
  const [holdoutSuite, setHoldoutSuite] = useState<HoldoutSuite | null>(null);
  const [suites, setSuites] = useState<HoldoutSuite[]>([]);
  const [holdout, setHoldout] = useState<EnsembleResult | null>(null);
  const [renewal, setRenewal] = useState<RenewalResult[] | null>(null);
  const [renewalInventory, setRenewalInventory] = useState<RenewalInventory | null>(null);
  const [inheritance, setInheritance] = useState<InheritanceResult | null>(null);
  const [draft, setDraft] = useState<OpenFieldDraft>(() => {
    const components = frame.ports
      .filter((port) => port.kind !== 3)
      .map((port) => ({
        node: port.node,
        kind: (['port', 'reserve', 'module'] as const)[Math.min(2, port.kind)],
        layer: port.layer,
        x: Math.round(port.x / 16),
        y: Math.round(port.y / 16),
        charge: Math.round(port.charge * 4096 / 65_535),
        open: port.open,
        upkeepRate: 0,
        capacity: 64,
      }));
    const current = frame.currents[0];
    return {
      ...DEFAULT_OPEN_FIELD,
      form,
      components,
      materials: frame.materials.map((material) => ({
        material: material.material,
        kind: (['junction_blank', 'boundary_blank', 'conductor'] as const)[Math.min(2, material.kind)],
        amount: material.amount,
        layer: material.layer,
        x: Math.round(material.x / 16),
        y: Math.round(material.y / 16),
      })),
      routes: frame.routes.map((route) => ({ route: route.route, tail: route.tail, head: route.head, capacity: 32 })),
      compartmentMembers: frame.ports.filter((port) => port.member && port.kind !== 3).map((port) => port.node),
      supplyLayer: current?.layer ?? 0,
      supplyX: current?.path[0]?.x ?? 2048,
      supplyY: current?.path[0]?.y ?? 2048,
    };
  });
  const [compiled, setCompiled] = useState<CompiledOpenField | null>(null);
  const [openFieldRun, setOpenFieldRun] = useState<OpenFieldRun | null>(null);
  const [lensPacket, setLensPacket] = useState<LensSensorPacket | null>(null);
  const [instrumentReading, setInstrumentReading] = useState<InstrumentReading | null>(null);
  const [records, setRecords] = useState<ArchiveRecord[]>([]);
  const [job, setJob] = useState<{ id: number; kind: string; status: 'queued' | 'running' | 'complete' | 'cancelled'; completed: number; total: number } | null>(null);
  const jobOrdinal = useRef(0);
  const analysis = useRef<AnalysisCoordinator | null>(null);
  if (analysis.current === null) analysis.current = new AnalysisCoordinator();
  const scenario = useMemo(
    () => scenarioFrom(frame, regime, form, view, observation, staged),
    [frame, regime, form, view, observation, staged],
  );
  const handsOffScenario = useMemo(
    () => scenarioFrom(frame, regime, form, view, observation, staged, 'hands_off'),
    [frame, regime, form, view, observation, staged],
  );

  useEffect(() => {
    void archiveRecords().then(setRecords);
    void holdoutSuites().then((held) => {
      setSuites(held);
      setHoldoutSuite(
        held.find((suite) =>
          suite.scenarioId === handsOffScenario.id
          && suite.status !== 'retired',
        ) ?? null,
      );
    });
    return () => analysis.current?.cancel();
  }, []);

  useEffect(() => {
    if (!holdoutSuite || holdoutSuite.scenarioId === handsOffScenario.id) return;
    if (holdoutSuite.status !== 'sealed' && holdoutSuite.status !== 'evaluated') return;
    const contaminated: HoldoutSuite = {
      ...holdoutSuite,
      status: 'contaminated',
      updatedAt: Date.now(),
      contaminationReason: 'post_seal_scenario_change',
    };
    setHoldoutSuite(contaminated);
    void storeHoldoutSuite(contaminated).then(async () => setSuites(await holdoutSuites()));
  }, [holdoutSuite, handsOffScenario.id]);

  useEffect(() => {
    if (bench !== 'renewal') return;
    void client.command('renewal_inventory', {}).then((response) => {
      if (response.ok) setRenewalInventory(response.body as unknown as RenewalInventory);
    });
  }, [bench, client, frame.header.step]);

  useEffect(() => {
    if (bench !== 'observe') return;
    let current = true;
    const inside = view?.inside ?? frame.ports.map((port) => port.node);
    void client.command('sample_instrument', {
      inside,
      instrument: observation.instrument,
      resolution: observation.resolution,
      surround: observation.surround,
      window: observation.window,
    }).then((response) => {
      if (current && response.ok) {
        setInstrumentReading(response.body as unknown as InstrumentReading);
      }
    });
    return () => { current = false; };
  }, [bench, client, frame.header.step, observation.instrument, observation.resolution, observation.surround, observation.window, view]);

  function cancelJob() {
    analysis.current?.cancel();
    analysis.current = new AnalysisCoordinator();
    setJob((standing) => standing ? { ...standing, status: 'cancelled' } : null);
  }

  function startJob(kind: AnalysisTask) {
    analysis.current?.cancel();
    analysis.current = new AnalysisCoordinator();
    const coordinator = analysis.current;
    const id = ++jobOrdinal.current;
    setJob({ id, kind, status: 'queued', completed: 0, total: 3 });
    void (async () => {
      const contract = kind === 'holdout' || kind === 'inheritance'
        ? handsOffScenario
        : scenario;
      const analysisScenario = kind === 'holdout'
        ? { ...contract, holdoutSeed: holdoutSuite?.suiteSeed ?? 0 }
        : contract;
      const exported = await client.command('export_run', {});
      if (!exported.ok || typeof exported.body.text !== 'string') {
        throw new Error('analysis_export_failed');
      }
      const fallback = async (): Promise<AnalysisResult> => {
        const response = await client.command('run_analysis', { kind, scenario: analysisScenario });
        if (!response.ok) throw new Error(response.error.code);
        return response.body as unknown as AnalysisResult;
      };
      return coordinator.run(kind, analysisScenario, exported.body.text, fallback, (completed, total) => {
        setJob({ id, kind, status: 'running', completed, total });
      });
    })().then((result) => {
      if (kind === 'divergence') setDivergence(result as DivergenceResult);
      else if (kind === 'ensemble') setEnsemble(result as EnsembleResult);
      else if (kind === 'holdout') {
        const held = result as EnsembleResult;
        setHoldout(held);
        if (holdoutSuite?.status === 'sealed') {
          const evaluated: HoldoutSuite = {
            ...holdoutSuite,
            status: 'evaluated',
            updatedAt: Date.now(),
            passed: held.passCount,
          };
          setHoldoutSuite(evaluated);
          void storeHoldoutSuite(evaluated).then(async () => setSuites(await holdoutSuites()));
        }
      }
      else setInheritance(result as InheritanceResult);
      setJob({ id, kind, status: 'complete', completed: 3, total: 3 });
    }).catch(() => {
      setJob((standing) => standing?.id === id ? { ...standing, status: 'cancelled' } : standing);
    });
  }

  function stagePlan() {
    const targetKind = TOOL_TARGET[plan.tool];
    const fallback = targetKind === 'route'
      ? frame.routes[0]?.route
      : targetKind === 'supply' || targetKind === 'input'
        ? frame.currents[0]?.id
      : targetKind === 'node' || targetKind === 'component'
        ? frame.ports[0]?.node
        : 0;
    const resolved = { ...plan, target: plan.target || fallback || 0 };
    setStaged(resolved);
    setPlan(resolved);
    if (resolved.scope === 'live' && resolved.tool === 'blade' && resolved.target > 0) {
      void client.queuePlan({ op: 'cut', route: resolved.target });
    }
    if (resolved.scope === 'live' && resolved.tool === 'clamp' && resolved.target > 0) {
      void client.queuePlan({
        op: 'limit_route',
        route: resolved.target,
        retained_fraction: Math.max(1, Math.round((100 - resolved.amount) * 65_536 / 100)),
        duration: resolved.duration,
      });
    }
    if (resolved.scope === 'live' && resolved.tool === 'breach') {
      void client.queuePlan({
        op: 'raise_leak',
        delta: Math.max(1, Math.round(resolved.amount * 8_192 / 100)),
        duration: resolved.duration,
      });
    }
    if (resolved.scope === 'live' && resolved.tool === 'decoy' && resolved.target > 0) {
      void client.queuePlan({
        op: 'divert_supply',
        current: resolved.target,
        receiver: resolved.receiver || frame.ports[0]?.node || 0,
        capture_fraction: Math.max(1, Math.round(resolved.amount * 65_536 / 100)),
        duration: resolved.duration,
      });
    }
    if (resolved.scope === 'live' && resolved.tool === 'replace' && resolved.target > 0) {
      void client.queuePlan({
        op: 'replace_component',
        node: resolved.target,
        transfer_mask: Math.max(1, resolved.transferMask),
      });
    }
    if (resolved.scope === 'live' && resolved.tool === 'transplant') {
      void client.queuePlan({ op: 'transplant', regime: resolved.destination });
    }
    if (resolved.scope === 'live' && resolved.tool === 'delay' && resolved.target > 0) {
      void client.queuePlan({
        op: 'delay_supply',
        current: resolved.target,
        duration: resolved.duration,
      });
    }
    if (resolved.scope === 'live' && resolved.tool === 'scramble' && frame.routes.length > 0) {
      void client.queuePlan({
        op: 'scramble_routes',
        routes: frame.routes.map((route) => route.route),
        probability: Math.max(1, Math.round(resolved.amount * 65_536 / 100)),
        duration: resolved.duration,
      });
    }
  }

  function stageDivergenceInspection() {
    const resolved = scenarioFrom(frame, regime, form, view, observation, staged ?? plan);
    const target = resolved.intervention?.target ?? frame.routes[0]?.route ?? null;
    if (resolved.intervention?.tool === 'blade') {
      client.inspect({ target: 'perturbation', kind: 'route-removal', parameter: target });
    }
  }

  async function exportRun() {
    const response = await client.command('export_run', {});
    if (!response.ok || typeof response.body.text !== 'string') return;
    const evidence: ArchiveRecord['evidence'] = [];
    if (ensemble) evidence.push({ kind: 'established', passed: ensemble.passCount, trials: ensemble.trials.length, artifact: ensemble.scenarioHash });
    if (holdout) evidence.push({ kind: 'withstood', passed: holdout.passCount, trials: holdout.trials.length, artifact: holdout.scenarioHash });
    if (renewal) evidence.push({ kind: 'renewed', passed: renewal.filter((trial) => trial.passed).length, trials: renewal.length, artifact: renewal[0]?.scenarioHash ?? scenario.id });
    if (divergence) evidence.push({ kind: 'paired_effect_observed', passed: 1, trials: 1, artifact: `${divergence.scenarioHash}:${divergence.firstStep}` });
    await storeArchiveRecord(recordFromExport(
      response.body.text,
      scenario,
      evidence,
      typeof response.body.embodied_state_hash === 'string'
        ? response.body.embodied_state_hash
        : undefined,
    ));
    setRecords(await archiveRecords());
  }

  async function reopen(record: ArchiveRecord) {
    const response = await client.command('reopen_archive', { text: record.payload });
    if (response.ok) onClose();
  }

  async function remove(record: ArchiveRecord) {
    await removeArchiveRecord(record.id);
    setRecords(await archiveRecords());
  }

  async function compileDraft() {
    setCompiled(await compileOpenField(draft, client));
    setOpenFieldRun(null);
  }

  async function launchOpenField() {
    setOpenFieldRun(await runOpenField(draft, client));
  }

  async function sampleLens() {
    const response = await client.command('sample_lens', {});
    if (!response.ok) return;
    setLensPacket(response.body as unknown as LensSensorPacket);
  }

  async function runRenewalTrials() {
    analysis.current?.cancel();
    const id = ++jobOrdinal.current;
    const total = 8;
    const root = Number.parseInt(scenario.id.slice(0, 8), 16) || 1;
    const trials: RenewalResult[] = [];
    setJob({ id, kind: 'renewal', status: 'queued', completed: 0, total });
    for (let place = 0; place < total; place += 1) {
      setJob({ id, kind: 'renewal', status: 'running', completed: place, total });
      const seed = (root + place * 65_537) >>> 0;
      const response = await client.command('renewal_trial', { seed });
      if (!response.ok) break;
      const held = response.body as Record<string, unknown>;
      trials.push({
        controlContract: String(held.control_contract) as RenewalResult['controlContract'],
        embodiedStateHash: String(held.embodied_state_hash),
        generatorHash: String(held.generator_hash),
        scenarioHash: String(held.scenario_hash),
        seed: Number(held.seed),
        detectedAt: Number(held.detected_at),
        recruitedAt: Number(held.recruited_at),
        reconnectedAt: Number(held.reconnected_at),
        recoveredAt: Number(held.recovered_at),
        resourceCost: Math.round(Number(held.resource_cost) / 65_536),
        materialCost: Number(held.material_cost),
        materialIds: Array.isArray(held.material_ids) ? held.material_ids.map(Number) : [],
        rebuiltRoutes: Array.isArray(held.rebuilt_routes) ? held.rebuilt_routes.map(Number) : [],
        signalId: held.signal_id === null ? null : Number(held.signal_id),
        reconnection: Number(held.reconnection),
        failedNode: Number(held.failed_node),
        replacementNode: held.replacement_node === null ? null : Number(held.replacement_node),
        passed: Boolean(held.passed),
      });
    }
    setRenewal(trials);
    setJob({ id, kind: 'renewal', status: 'complete', completed: trials.length, total });
  }

  async function sealHoldout() {
    const identity = client.identity?.();
    if (!identity) return;
    const exported = await client.command('export_run', {});
    if (!exported.ok) return;
    const suite = await sealedHoldoutSuite(handsOffScenario, {
      ...identity,
      embodiedHash: typeof exported.body.embodied_state_hash === 'string'
        ? exported.body.embodied_state_hash
        : identity.embodiedHash,
    });
    await storeHoldoutSuite(suite);
    setHoldoutSuite(suite);
    setSuites(await holdoutSuites());
    setHoldout(null);
  }

  async function retireHoldout() {
    if (!holdoutSuite) return;
    const retired: HoldoutSuite = {
      ...holdoutSuite,
      status: 'retired',
      updatedAt: Date.now(),
    };
    await storeHoldoutSuite(retired);
    setHoldoutSuite(null);
    setSuites(await holdoutSuites());
    setHoldout(null);
  }

  return (
    <div
      id="field-laboratory"
      className="lab"
      role="dialog"
      aria-modal="true"
      aria-labelledby="lab-title"
      data-bench={bench}
      data-holdout={bench === 'holdout' ? holdoutSuite?.status ?? 'draft' : undefined}
    >
      <img className="lab-texture" src="/assets/number-2-field-texture.png" alt="" aria-hidden="true" />
      <header className="lab-header">
        <div>
          <p>{copy('lab.header')}</p>
          <h1 id="lab-title">{copy('lab.title')}</h1>
        </div>
        <dl>
          <div><dt>{copy('lab.scenario')}</dt><dd>{scenario.id}</dd></div>
          <div><dt>{copy('lab.step')}</dt><dd>{frame.header.step}</dd></div>
          <div><dt>{copy('lab.regime')}</dt><dd>{copy(catalog('regime', regime))}</dd></div>
        </dl>
        {job ? (
          <div className="lab-job" data-status={job.status}>
            <span>{copy(catalog('lab.job', job.status))}</span>
            <code>{job.kind}-{String(job.id).padStart(3, '0')}</code>
            <meter min="0" max={job.total} value={job.completed} />
            {job.status === 'queued' || job.status === 'running' ? (
              <button type="button" onClick={cancelJob}>{copy('lab.cancel_job')}</button>
            ) : null}
          </div>
        ) : null}
        <button type="button" className="lab-close" onClick={onClose}>{copy('lab.close')}</button>
      </header>
      <BenchTabs active={bench} onSelect={setBench} />
      {bench === 'observe' ? <ObserveBench form={form} protocol={observation} setProtocol={setObservation} reading={instrumentReading} lensPacket={lensPacket} sampleLens={() => void sampleLens()} /> : null}
      {bench === 'intervene' ? <InterveneBench frame={frame} plan={plan} setPlan={setPlan} stage={stagePlan} staged={staged !== null} /> : null}
      {bench === 'divergence' ? <DivergenceBench result={divergence} run={() => { stageDivergenceInspection(); startJob('divergence'); }} /> : null}
      {bench === 'ensemble' ? <EnsembleBench result={ensemble} run={() => startJob('ensemble')} /> : null}
      {bench === 'holdout' ? <HoldoutBench result={holdout} suite={holdoutSuite} suites={suites} seal={() => void sealHoldout()} run={() => startJob('holdout')} retire={() => void retireHoldout()} /> : null}
      {bench === 'archive' ? <ArchiveBench scenarioId={scenario.id} records={records} exportRun={() => void exportRun()} reopen={(record) => void reopen(record)} remove={(record) => void remove(record)} /> : null}
      {bench === 'renewal' ? <RenewalBench inventory={renewalInventory} result={renewal} run={() => void runRenewalTrials()} /> : null}
      {bench === 'inheritance' ? <InheritanceBench result={inheritance} run={() => startJob('inheritance')} /> : null}
      {bench === 'open_field' ? <OpenFieldBench draft={draft} setDraft={setDraft} compiled={compiled} compile={() => void compileDraft()} result={openFieldRun} run={() => void launchOpenField()} /> : null}
    </div>
  );
}
