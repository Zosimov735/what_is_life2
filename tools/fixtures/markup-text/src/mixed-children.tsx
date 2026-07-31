// Fixture: literal text written beside an expression is still uncatalogued.
import { copy } from './copy';

export function Panel({ target }: { target: string }) {
  return (
    <section>
      <h2>Objective: {copy('objective.follow_current')}</h2>
      <button>Open the {target}</button>
    </section>
  );
}
