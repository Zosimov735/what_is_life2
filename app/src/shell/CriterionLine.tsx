import { copy } from './copy';
import type { FrameState } from '../../../worker/src/frame-state';
import type { CriterionReading } from '../../../worker/src/protocol';

interface CriterionLineProps {
  criterion: CriterionReading | null;
  mode: FrameState['header']['mode'] | null;
}

const units = (raw: number): string => (raw / 65_536).toLocaleString('en-US', {
  maximumFractionDigits: 2,
});

const percent = (raw: number | null): string => raw === null
  ? copy('criterion.unbounded')
  : `${Math.round((raw / 65_536) * 100)}%`;

function statusKey(reading: CriterionReading): string {
  if (reading.status === 'passed') return 'criterion.status.passed';
  if (reading.status === 'failed') return 'criterion.status.failed';
  if (!reading.ready) return 'criterion.status.observing';
  if (!reading.all_metrics_met) return 'criterion.status.recovering';
  return reading.hands_off
    ? 'criterion.status.hands_off'
    : 'criterion.status.steady';
}

export function CriterionLine({ criterion, mode }: CriterionLineProps) {
  if (!criterion || mode !== 'running') return null;

  const weakestRoute = criterion.routes.reduce<(typeof criterion.routes)[number] | null>(
    (weakest, route) => !weakest || route.minimum - route.floor < weakest.minimum - weakest.floor
      ? route
      : weakest,
    null,
  );
  const weakestComponent = criterion.components.reduce<(typeof criterion.components)[number] | null>(
    (weakest, component) => !weakest || component.margin < weakest.margin
      ? component
      : weakest,
    null,
  );
  const requiredWindow = weakestRoute?.window_steps ?? 0;
  const windowProgress = Math.min(criterion.observed_steps, requiredWindow);

  return (
    <aside
      className="criterion-line"
      data-status={criterion.status}
      data-met={criterion.all_metrics_met}
      aria-live="polite"
    >
      <header>
        <span>{copy('criterion.title')}</span>
        <strong>{copy(statusKey(criterion))}</strong>
      </header>
      <dl>
        <div data-met={weakestRoute?.met ?? false}>
          <dt>{copy('criterion.route')}</dt>
          <dd>
            {weakestRoute
              ? `${units(weakestRoute.minimum)} / ${units(weakestRoute.floor)} ${copy('unit.cu_per_step')}`
              : copy('criterion.none')}
          </dd>
        </div>
        <div data-met={weakestComponent?.met ?? false}>
          <dt>{copy('criterion.component')}</dt>
          <dd>
            {weakestComponent
              ? `${units(weakestComponent.margin)} ${copy('unit.cu')}`
              : copy('criterion.none')}
          </dd>
        </div>
        <div data-met={criterion.leakage.met}>
          <dt>{copy('criterion.leakage')}</dt>
          <dd>{percent(criterion.leakage.ratio)} / {percent(criterion.leakage.ceiling)}</dd>
        </div>
        <div data-met={criterion.hands_off && criterion.all_metrics_met}>
          <dt>{criterion.ready ? copy('criterion.hands_off') : copy('criterion.window')}</dt>
          <dd>
            {criterion.ready
              ? `${criterion.hands_off_remaining} ${copy('criterion.steps')}`
              : `${windowProgress} / ${requiredWindow}`}
          </dd>
        </div>
      </dl>
    </aside>
  );
}
