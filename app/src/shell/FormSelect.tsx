/**
 * The opening selection: the eight starting Forms, and the one promise each of
 * them makes.
 *
 * This is the first surface a run stands behind, and it is the only one that
 * stands before there is a run at all — the Form the player takes is part of
 * `init_run`, so the session opens on the choice rather than the choice
 * arriving after it.
 *
 * What governs the wording is `docs/field-framework/LEXICON.md`. Three of its
 * rules decide almost everything here:
 *
 * - **No collapsed value is ever shown.** So there are no numbers here at all:
 *   no figure, no bar, and nothing that folds several readings into one. What
 *   each Form authors is a parameter of the Field, and the surface says what
 *   the parameter does in words instead of showing what it is set to.
 * - **Copy names what the player can do.** Each Form's line is its authored
 *   promise from the catalog and nothing else — no comparison between Forms, no
 *   mark on one of them, and no order but the closed set's own.
 * - **Nothing player-facing is written inline.** Every string here is a catalog
 *   key: the instruction, the eight names, and the eight promises.
 *
 * The order is `FORM_IDS`, which is the closed set's own order and is not a
 * ranking. No Form is offered as the one to take, nothing is preselected, and
 * the surface asks for a choice rather than confirming a default.
 *
 * **Keyboard.** Every Form is a button, so the whole surface works with Tab and
 * Enter without anything here managing focus. The arrow keys are the addition:
 * one tab stop for the group and the arrows moving between the eight, which is
 * how a list of choices is expected to behave and what keeps the tab order from
 * running to eight stops before the first playable frame.
 *
 * That behaviour has a name, and the surface takes it: the group is a
 * `radiogroup` and each Form is a `radio` inside it — one tab stop, the arrows
 * moving between the options, and exactly one choice to be made. None is
 * checked, because none has been taken: the surface asks for a choice rather
 * than confirming a default, and a choice opens the run rather than marking the
 * option.
 */

import { useEffect, useRef, useState } from 'react';
import { copy } from './copy';
import { FORM_IDS, type FormId } from '../../../worker/src/protocol';

interface FormSelectProps {
  /** Takes the Form the player chose. The session opens on it. */
  onChoose: (form: FormId) => void;
}

/** Which key moves the roving focus, and by how much. */
const MOVES: Readonly<Record<string, number>> = {
  ArrowDown: 1,
  ArrowRight: 1,
  ArrowUp: -1,
  ArrowLeft: -1,
};

/**
 * Where one Form's name and its one promise stand in the catalog: the same
 * name under the two kinds, exactly as an objective and its explanation do.
 * The keys are derived rather than listed, so a Form of the closed set cannot
 * be given a surface entry that names nothing.
 */
function wording(form: FormId): { name: string; promise: string } {
  const name = `form.${form}`;
  const promise = `promise.${form}`;
  return { name: copy(name), promise: copy(promise) };
}

export function FormSelect({ onChoose }: FormSelectProps) {
  const [at, setAt] = useState(0);
  /** Whether the focus is the surface's to move, so a mounted page does not steal it. */
  const roving = useRef(false);
  const buttons = useRef<(HTMLButtonElement | null)[]>([]);

  useEffect(() => {
    if (!roving.current) return;
    buttons.current[at]?.focus();
  }, [at]);

  function onKeyDown(event: React.KeyboardEvent<HTMLUListElement>) {
    const move = MOVES[event.key];
    const last = FORM_IDS.length - 1;
    let wanted = at;
    if (move !== undefined) wanted = Math.min(last, Math.max(0, at + move));
    else if (event.key === 'Home') wanted = 0;
    else if (event.key === 'End') wanted = last;
    else return;
    event.preventDefault();
    roving.current = true;
    setAt(wanted);
  }

  return (
    <div className="opening">
      <p className="opening-line" id="opening-line">
        {copy('instruction.choose_form')}
      </p>
      <ul
        className="opening-forms"
        role="radiogroup"
        aria-labelledby="opening-line"
        onKeyDown={onKeyDown}
      >
        {FORM_IDS.map((form, place) => {
          const held = wording(form);
          return (
            <li key={form} role="presentation">
              <button
                type="button"
                role="radio"
                // Nothing is checked: the choice has not been made, and taking
                // one opens the run rather than marking the option.
                aria-checked={false}
                className="opening-form"
                // One tab stop for the group; the arrows move inside it.
                tabIndex={place === at ? 0 : -1}
                ref={(element) => {
                  buttons.current[place] = element;
                }}
                onFocus={() => setAt(place)}
                onClick={() => onChoose(form)}
              >
                <span className="opening-form-name">{held.name}</span>
                <span className="opening-form-promise">{held.promise}</span>
              </button>
            </li>
          );
        })}
      </ul>
    </div>
  );
}
