// Fixture: conditional rendering and typed calls carry no text of their own.
import { useState } from 'react';
import { copy } from './copy';

export function Ternary({ ready }: { ready: boolean }) {
  return <div>{ready ? <p>{copy('objective.follow_current')}</p> : null}</div>;
}

export function Optional({ ready }: { ready: boolean }) {
  return <div>{ready ? <p>{copy('objective.follow_current')}</p> : undefined}</div>;
}

export function Guarded({ isOpen }: { isOpen: boolean }) {
  return <div>{isOpen && <p>{copy('objective.follow_current')}</p>}</div>;
}

export function Either({ ready }: { ready: boolean }) {
  return (
    <div>
      {ready ? (
        <p>{copy('objective.follow_current')}</p>
      ) : (
        <p>{copy('objective.follow_current')}</p>
      )}
    </div>
  );
}

export function Counted() {
  const [count, setCount] = useState<number>(0);
  const registry = new Map<string, number>();
  const step = count > 3 ? 2 : 1;
  return (
    <ul style={{ opacity: 1 }} onClick={() => setCount(count + step)}>
      {[...registry.keys()].map((key) => (
        <li key={key}>{copy('objective.follow_current')}</li>
      ))}
    </ul>
  );
}

export function Wrapped() {
  return (
    <>
      {copy('objective.follow_current')}
    </>
  );
}
