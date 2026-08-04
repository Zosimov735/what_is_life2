import { useEffect, useRef, useState } from 'react';
import { renderQuality, setRenderQuality, type RenderQuality } from '../render/quality';
import { copy } from './copy';
import { REGIMES, type RegimeEntry, type RegimeId } from './regimes';
import './atlas-form.css';

export type { RegimeId } from './regimes';

const FACTS = ['supply', 'dissipation', 'transport', 'medium', 'compartment'] as const;

const catalog = (prefix: string, value: string): string => `${prefix}.${value}`;

function regimeKey(kind: 'name' | 'status' | 'subtitle' | 'fact' | 'value', entry: RegimeEntry, fact?: string): string {
  if (kind === 'name') return `regime.${entry.id}`;
  if (kind === 'status') return `regime.status.${entry.status}`;
  if (kind === 'subtitle') return `regime.subtitle.${entry.id}`;
  if (kind === 'fact') return `regime.fact.${fact}`;
  return `regime.${entry.id}.${fact}`;
}

interface AtlasProps {
  onOpen: (regime: RegimeId) => void;
}

export function Atlas({ onOpen }: AtlasProps) {
  const [selected, setSelected] = useState(0);
  const [quality, setQuality] = useState<RenderQuality>(() => renderQuality().id);
  const markers = useRef<(HTMLButtonElement | null)[]>([]);
  const entry = REGIMES[selected];

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.code === 'ArrowLeft' || event.code === 'ArrowUp') {
        setSelected((at) => (at + REGIMES.length - 1) % REGIMES.length);
        event.preventDefault();
      } else if (event.code === 'ArrowRight' || event.code === 'ArrowDown') {
        setSelected((at) => (at + 1) % REGIMES.length);
        event.preventDefault();
      } else if (event.code === 'Enter' && entry.status === 'implemented') {
        onOpen(entry.id);
        event.preventDefault();
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [entry, onOpen]);

  useEffect(() => markers.current[selected]?.focus(), [selected]);

  return (
    <main className="atlas" data-selected={entry.id} aria-label={copy('atlas.title')}>
      <img className="atlas-texture" src="/assets/number-2-field-texture.png" alt="" aria-hidden="true" />
      <div className="atlas-topology" aria-hidden="true">
        <span className="atlas-orbit atlas-orbit-a" />
        <span className="atlas-orbit atlas-orbit-b" />
        <span className="atlas-orbit atlas-orbit-c" />
        <span className="atlas-route atlas-route-a" />
        <span className="atlas-route atlas-route-b" />
        <span className="atlas-route atlas-route-c" />
        <span className="atlas-route atlas-route-d" />
      </div>
      <header className="atlas-heading">
        <p>{copy('atlas.kicker')}</p>
        <h1>{copy('atlas.title')}</h1>
      </header>
      <div className="atlas-map" role="radiogroup" aria-label={copy('atlas.destinations')}>
        {REGIMES.map((regime, index) => (
          <button
            key={regime.id}
            ref={(element) => { markers.current[index] = element; }}
            type="button"
            role="radio"
            aria-checked={index === selected}
            className="atlas-marker"
            data-status={regime.status}
            data-regime={regime.id}
            style={{ left: `${regime.position.x}%`, top: `${regime.position.y}%` }}
            onClick={() => setSelected(index)}
          >
            <span className="atlas-marker-core"><span /></span>
            <span className="atlas-marker-name">{copy(regimeKey('name', regime))}</span>
          </button>
        ))}
      </div>
      <aside className="atlas-detail" aria-live="polite">
        <span className="atlas-contract-spine" aria-hidden="true" />
        <div className="atlas-detail-head">
          <p>{copy(regimeKey('status', entry))}</p>
          <h2>{copy(regimeKey('name', entry))}</h2>
          <span>{copy(regimeKey('subtitle', entry))}</span>
        </div>
        <dl className="atlas-contract-facts">
          {FACTS.map((fact) => (
            <div key={fact} data-fact={fact}>
              <dt>{copy(regimeKey('fact', entry, fact))}</dt>
              <dd>{copy(regimeKey('value', entry, fact))}</dd>
            </div>
          ))}
          <div data-fact="interventions">
            <dt>{copy('regime.fact.interventions')}</dt>
            <dd>{entry.interventions.length > 0 ? entry.interventions.map((tool) => copy(catalog('intervention', tool))).join(' / ') : copy('lab.none')}</dd>
          </div>
          <div className="atlas-criterion" data-fact="criterion">
            <dt>{copy('regime.fact.criterion')}</dt>
            <dd>{entry.criterion}</dd>
          </div>
        </dl>
        <button
          type="button"
          className="atlas-open"
          disabled={entry.status !== 'implemented'}
          onClick={() => onOpen(entry.id)}
        >
          <span className="atlas-open-line" aria-hidden="true" />
          <span>{copy(entry.status === 'implemented' ? 'atlas.open' : 'atlas.pending')}</span>
          <span className="atlas-open-gate" aria-hidden="true" />
        </button>
      </aside>
      <div className="atlas-quality" role="group" aria-label={copy('atlas.quality')}>
        {(['low', 'medium', 'high'] as RenderQuality[]).map((level) => (
          <button
            type="button"
            key={level}
            aria-pressed={quality === level}
            onClick={() => { setRenderQuality(level); setQuality(level); }}
          >
            {copy(catalog('atlas.quality', level))}
          </button>
        ))}
      </div>
    </main>
  );
}
