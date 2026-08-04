import type { FramePulsePreview } from '../../../worker/src/frame-state';
import { copy } from './copy';

const fixed = (raw: number): string => (raw / 65_536).toLocaleString('en-US', {
  maximumFractionDigits: 2,
});

export function PulsePreview({ preview }: { preview: FramePulsePreview | null }) {
  if (!preview) return null;
  const hasEffect = preview.gathered > 0
    || preview.reserveReleased > 0
    || preview.openedPorts > 0
    || preview.displacedPressures > 0;
  return (
    <aside
      className="pulse-preview"
      data-connected={hasEffect}
      aria-live="polite"
      aria-label={copy('pulse.preview')}
    >
      <header>
        <span className="pulse-key">{copy('input.coupling_key')}</span>
        <span>{copy('pulse.coupling')}</span>
        <strong>{fixed(preview.radius)} {copy('unit.field_units')}</strong>
      </header>
      <div className="pulse-effects">
        {!hasEffect ? <span className="pulse-empty">{copy('pulse.no_connection')}</span> : null}
        {preview.gathered > 0 ? (
          <span><b>{copy('pulse.transfer')}</b> {fixed(preview.gathered)} {copy('unit.cu')}</span>
        ) : null}
        {preview.reserveReleased > 0 ? (
          <span><b>{copy('pulse.reserve_release')}</b> {fixed(preview.reserveReleased)} {copy('unit.cu')}</span>
        ) : null}
        {preview.openedPorts > 0 ? (
          <span><b>{copy('pulse.ports_opened')}</b> {preview.openedPorts}</span>
        ) : null}
        {preview.displacedPressures > 0 ? (
          <span><b>{copy('pulse.disturbances_displaced')}</b> {preview.displacedPressures}</span>
        ) : null}
      </div>
      <p>{copy('pulse.release_e')}</p>
    </aside>
  );
}
