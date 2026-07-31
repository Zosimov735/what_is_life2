// Fixture: technical string literals and developer diagnostics are not
// player-facing copy, so they stay inline.
import { ObjectivePanel } from './panel';

const CONTENT_TYPE = 'application/json';
const TRAIL_SHADOW = '0 0 4px rgba(0, 0, 0, 0.4)';
const SPEC_URL = 'https://example.invalid/field-framework';

export function readFrame(depth: number) {
  if (depth < 0) {
    throw new Error('Depth index below the first layer');
  }
  console.warn('Depth change ignored during Still Mode');
  return { CONTENT_TYPE, TRAIL_SHADOW, SPEC_URL, ObjectivePanel };
}
