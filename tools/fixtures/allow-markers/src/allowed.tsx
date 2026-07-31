// Fixture: both escape markers suppress a violation on the following line.

// lexicon-check: allow-term — fixture proving the term escape marker works
export const cellIndex = 0;

export function Diagnostics() {
  // lexicon-check: allow-inline — fixture proving the inline escape marker works
  return <pre>Developer diagnostics surface only in local builds.</pre>;
}

export const notice = 'Local builds print this line.'; // lexicon-check: allow-inline — same-line marker
