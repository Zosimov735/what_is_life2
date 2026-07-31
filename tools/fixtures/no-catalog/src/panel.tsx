// Fixture: expected to fail with catalog-missing before any key can resolve.
import { copy } from './copy';

export function Objective() {
  return <p>{copy('objective.follow_current')}</p>;
}
