/**
 * The copy accessor: the one way a component reads player-facing text.
 *
 * Every string comes from `field_game/content/copy/catalog.json`, and the
 * check reads a string in a `copy('key')` call as a catalog key. Keep the call
 * shape if this module is ever replaced. Rules:
 * `docs/field-framework/LEXICON.md`.
 */

import catalog from '../../../content/copy/catalog.json';

/** One authored string with the kind whose writing rules it follows. */
interface CatalogEntry {
  kind: string;
  text: string;
}

const entries = catalog.entries as Record<string, CatalogEntry | undefined>;

/** Resolves a catalog key. An undefined key is a defect, not a fallback. */
export function copy(key: string): string {
  const entry = entries[key];
  if (!entry) {
    throw new Error(`No catalog entry defines ${key}`);
  }
  return entry.text;
}
