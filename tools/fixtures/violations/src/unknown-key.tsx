// Fixture: expected to fail with unknown-copy-key.
import { copy } from './copy';

export function Notice() {
  return <p>{copy('objective.missing_entry')}</p>;
}
