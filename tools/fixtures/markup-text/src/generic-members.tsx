// Fixture: consecutive generic-typed members in a type body are declarations,
// not markup text. A member run ending at `>` and the next one opening at `<`
// look like a tag pair unless the bracket before `<` is read too.
import { copy } from './copy';

interface Listing {
  routes: Array<number>
  ports: Array<number>
  grains: Map<string, number>
}

type Pair = { first: Array<number>, second: Array<number> };

export function Panel({ routes }: Listing) {
  const counted: Pair = { first: routes, second: routes };
  return <p>{copy('objective.follow_current')}</p>;
}
