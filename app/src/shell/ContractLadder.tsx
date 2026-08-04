import { useEffect, useMemo, useRef, useState } from 'react';
import type {
  ContractCatalog,
  ContractCatalogEntry,
  ContractCriterion,
} from '../../../worker/src/protocol';
import { copy } from './copy';
import './contract-ladder.css';

const WHOLE = 65_536;

function fixed(value: number): string {
  return (value / WHOLE).toLocaleString('en-US', { maximumFractionDigits: 2 });
}

function criterionValue(criterion: ContractCriterion): string {
  if (criterion.metric === 'stored_charge') return `${fixed(criterion.threshold)} ${copy('unit.cu')}`;
  if (criterion.metric === 'accepted_flow') {
    return `${fixed(criterion.threshold)} ${copy('unit.cu_per_step')}`;
  }
  if (criterion.metric === 'leakage_ratio') {
    return `${(criterion.threshold * 100 / WHOLE).toLocaleString('en-US', {
      maximumFractionDigits: 1,
    })}${copy('unit.percent')}`;
  }
  return `${criterion.threshold.toLocaleString('en-US')} ${copy('unit.steps')}`;
}

function sourceLabel(criterion: ContractCriterion): string {
  const label = copy(`contract.source.${criterion.source.kind}`);
  return criterion.source.id === null ? label : `${label} ${criterion.source.id}`;
}

function statusKey(contract: ContractCatalogEntry, activeId: string | null): string {
  if (contract.id === activeId) return 'contract.ladder.active';
  return `contract.ladder.${contract.status}`;
}

interface ContractLadderProps {
  catalog: ContractCatalog;
  activeId: string | null;
  onOpen: (contract: ContractCatalogEntry) => Promise<boolean>;
  onReturn?: () => Promise<boolean>;
}

export function ContractLadder({ catalog, activeId, onOpen, onReturn }: ContractLadderProps) {
  const initial = Math.max(
    0,
    catalog.contracts.findIndex((contract) => contract.id === activeId) >= 0
      ? catalog.contracts.findIndex((contract) => contract.id === activeId)
      : catalog.contracts.findIndex((contract) => contract.available),
  );
  const [selected, setSelected] = useState(initial);
  const [opening, setOpening] = useState(false);
  const [refused, setRefused] = useState(false);
  const [returnRefused, setReturnRefused] = useState(false);
  const buttons = useRef<(HTMLButtonElement | null)[]>([]);
  const contract = catalog.contracts[selected] ?? catalog.contracts[0];
  const grades = useMemo(
    () => contract
      ? (['throughput', 'resilience', 'economy', 'complexity'] as const).map((axis) => ({
          axis,
          bands: contract.grade_bands[axis],
        }))
      : [],
    [contract],
  );

  useEffect(() => {
    const navigate = (event: KeyboardEvent): void => {
      if (event.target instanceof HTMLElement
          && event.target.closest('input, select, textarea, [contenteditable="true"]')) return;
      if (event.code === 'ArrowUp') {
        setSelected((position) => (position + catalog.contracts.length - 1) % catalog.contracts.length);
      } else if (event.code === 'ArrowDown') {
        setSelected((position) => (position + 1) % catalog.contracts.length);
      } else {
        return;
      }
      event.preventDefault();
    };
    window.addEventListener('keydown', navigate);
    return () => window.removeEventListener('keydown', navigate);
  }, [catalog.contracts.length]);

  useEffect(() => buttons.current[selected]?.focus(), [selected]);

  if (!contract) return null;

  const open = async (): Promise<void> => {
    if (!contract.available || opening) return;
    setOpening(true);
    setRefused(false);
    const accepted = await onOpen(contract);
    setOpening(false);
    setRefused(!accepted);
  };

  const resume = async (): Promise<void> => {
    if (!onReturn || opening) return;
    setOpening(true);
    setReturnRefused(false);
    const accepted = await onReturn();
    setOpening(false);
    setReturnRefused(!accepted);
  };

  return (
    <main className="contract-ladder" aria-label={copy('contract.ladder.title')}>
      <img
        className="contract-ladder-texture"
        src="/assets/number-2-field-texture.png"
        alt=""
        aria-hidden="true"
      />
      <header className="contract-ladder-heading">
        <p>{copy('contract.ladder.description')}</p>
        <h1>{copy('contract.ladder.title')}</h1>
      </header>

      <nav className="contract-ladder-index" aria-label={copy('contract.ladder.title')}>
        <ol>
          {catalog.contracts.map((entry, index) => (
            <li key={entry.id} data-status={entry.status} data-active={entry.id === activeId}>
              <button
                ref={(element) => { buttons.current[index] = element; }}
                type="button"
                aria-current={index === selected ? 'true' : undefined}
                onClick={() => { setSelected(index); setRefused(false); }}
              >
                <span>{String(entry.order).padStart(2, '0')}</span>
                <b>{copy(entry.title_key)}</b>
                <small>{copy(statusKey(entry, activeId))}</small>
              </button>
            </li>
          ))}
        </ol>
      </nav>

      <article className="contract-ladder-detail" aria-live="polite">
        <header>
          <p>{copy(statusKey(contract, activeId))}</p>
          <h2>{copy(contract.title_key)}</h2>
          <span>{copy(contract.brief_key)}</span>
        </header>

        {!contract.available ? (
          <section className="contract-ladder-lock">
            <h3>{copy('contract.ladder.missing')}</h3>
            <p>{contract.missing_prerequisites.map((id) => {
              const required = catalog.contracts.find((entry) => entry.id === id);
              return required ? copy(required.title_key) : id;
            }).join(' / ')}</p>
          </section>
        ) : null}

        <div className="contract-ladder-scroll">
          <section>
            <h3>{copy('contract.ladder.opening')}</h3>
            <dl className="contract-ladder-facts">
              <div><dt>{copy('contract.ladder.components')}</dt><dd>{contract.opening.component_count} / {contract.limits.max_components}</dd></div>
              <div><dt>{copy('contract.ladder.routes')}</dt><dd>{contract.opening.route_count} / {contract.limits.max_routes}</dd></div>
              <div><dt>{copy('contract.ladder.rules')}</dt><dd>{contract.limits.max_rules_per_component}</dd></div>
              <div><dt>{copy('contract.ladder.form')}</dt><dd>{copy(`form.${contract.opening.form}`)}</dd></div>
              <div><dt>{copy('contract.ladder.regime')}</dt><dd>{copy(`regime.${contract.opening.regime}`)}</dd></div>
              {contract.opening.supply_cycles.filter((cycle) => cycle.duty < 65_536).map((cycle) => (
                <div key={cycle.current}>
                  <dt>{copy('contract.ladder.supply_cycle')} {cycle.current}</dt>
                  <dd>{cycle.on_steps} {copy('contract.ladder.emitting')} / {cycle.period - cycle.on_steps} {copy('contract.ladder.quiet')}</dd>
                </div>
              ))}
              <div><dt>{copy('contract.ladder.minutes')}</dt><dd>{contract.commissioning.expected_minutes} {copy('unit.minutes')}</dd></div>
              <div><dt>{copy('contract.ladder.generator')}</dt><dd><code>{contract.opening.generator_spec_hash}</code></dd></div>
              <div><dt>{copy('contract.ladder.assembly')}</dt><dd><code>{contract.opening.assembly_template_hash}</code></dd></div>
            </dl>
          </section>

          <section>
            <h3>{copy('contract.ladder.criteria')}</h3>
            <ol className="contract-ladder-criteria">
              {contract.criteria.map((criterion) => (
                <li key={criterion.id}>
                  <span>{sourceLabel(criterion)}</span>
                  <b>{copy(`contract.metric.${criterion.metric}`)}</b>
                  <strong>{copy(`contract.comparison.${criterion.comparison}`)} {criterionValue(criterion)}</strong>
                  <small>{copy(`contract.aggregation.${criterion.aggregation}`)} · {criterion.window_steps} {copy('unit.steps')}</small>
                </li>
              ))}
            </ol>
          </section>

          <section>
            <h3>{copy('contract.ladder.capabilities')}</h3>
            <div className="contract-ladder-capabilities">
              <ul>{contract.capabilities.actions.map((action) => <li key={action}>{copy(`automation.action.${action}`)}</li>)}</ul>
              <ul>{contract.capabilities.conditions.map((condition) => <li key={condition}>{copy(`automation.condition.${condition}`)}</li>)}</ul>
              <ul>{contract.capabilities.hardware.map((hardware) => <li key={hardware}>{copy(`contract.hardware.${hardware}`)}</li>)}</ul>
            </div>
          </section>

          {contract.unlocks.actions.length > 0
              || contract.unlocks.conditions.length > 0
              || contract.unlocks.hardware.length > 0
              || contract.unlocks.next_contract ? (
            <section>
              <h3>{copy('contract.ladder.unlocks')}</h3>
              <div className="contract-ladder-capabilities contract-ladder-unlocks">
                <ul>{contract.unlocks.actions.map((action) => <li key={action}>{copy(`automation.action.${action}`)}</li>)}</ul>
                <ul>{contract.unlocks.conditions.map((condition) => <li key={condition}>{copy(`automation.condition.${condition}`)}</li>)}</ul>
                <ul>{contract.unlocks.hardware.map((hardware) => <li key={hardware}>{copy(`contract.hardware.${hardware}`)}</li>)}</ul>
              </div>
            </section>
          ) : null}

          <section>
            <h3>{copy('contract.ladder.qualification')}</h3>
            <dl className="contract-ladder-facts contract-ladder-qualification">
              <div><dt>{copy('contract.ladder.trials')}</dt><dd>{contract.qualification.trial_count}</dd></div>
              <div><dt>{copy('contract.ladder.duration')}</dt><dd>{contract.qualification.duration_steps} {copy('unit.steps')}</dd></div>
              <div><dt>{copy('contract.ladder.wait')}</dt><dd>{contract.commissioning.maximum_wall_wait_seconds} {copy('unit.seconds')}</dd></div>
            </dl>
          </section>

          <section>
            <h3>{copy('contract.ladder.grade_bands')}</h3>
            <div className="contract-ladder-grades">
              {grades.map(({ axis, bands }) => (
                <div key={axis}>
                  <span>{copy(`contract.grade.${axis}`)}</span>
                  <b>{bands.map((value) => Math.round(value * 100 / WHOLE)).join(' / ')}</b>
                </div>
              ))}
            </div>
          </section>
        </div>

        <footer>
          {onReturn ? <button type="button" disabled={opening} onClick={() => void resume()}>{copy('contract.ladder.return')}</button> : null}
          <button
            type="button"
            className="contract-ladder-open"
            disabled={!contract.available || opening}
            onClick={() => void open()}
          >
            {copy('contract.ladder.open')}
          </button>
          {refused ? <p role="status">{copy('contract.ladder.open_failed')}</p> : null}
          {returnRefused ? <p role="status">{copy('contract.ladder.return_failed')}</p> : null}
        </footer>
      </article>
    </main>
  );
}
