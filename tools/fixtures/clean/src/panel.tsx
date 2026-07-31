// Fixture: every player-facing string is resolved through the copy catalog.
import { copy } from './copy';

const MODE = 'still_mode';

export function ObjectivePanel() {
  return (
    <section aria-label={copy('label.still_mode')} data-mode={MODE}>
      <p>{copy('objective.follow_current')}</p>
    </section>
  );
}
