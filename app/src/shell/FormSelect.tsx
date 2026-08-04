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
import type { RegimeId } from './Atlas';
import thread from '../../../content/forms/thread.json';
import ring from '../../../content/forms/ring.json';
import relay from '../../../content/forms/relay.json';
import vault from '../../../content/forms/vault.json';
import lens from '../../../content/forms/lens.json';
import knot from '../../../content/forms/knot.json';
import wake from '../../../content/forms/wake.json';
import chorus from '../../../content/forms/chorus.json';

interface FormSelectProps {
  regime?: RegimeId;
  /** Takes the Form the player chose. The session opens on it. */
  onChoose: (form: FormId) => void;
  /** Returns to the Atlas before a run has been established. */
  onBack?: () => void;
}

interface FormContract {
  id: FormId;
  route_reach: number;
  route_capacity: number;
  forecast_depth: number;
  upkeep_rate: number;
  capacity: number;
  reserve: number;
  steer_scale: number;
}

const CONTRACTS = Object.fromEntries(
  [thread, ring, relay, vault, lens, knot, wake, chorus].map((form) => [form.id, form]),
) as unknown as Record<FormId, FormContract>;

const STATUS: Record<FormId, 'complete' | 'partial' | 'pending'> = {
  thread: 'complete',
  ring: 'complete',
  relay: 'complete',
  vault: 'complete',
  lens: 'complete',
  knot: 'complete',
  wake: 'complete',
  chorus: 'complete',
};

const units = (raw: number): string => (raw / 65_536).toLocaleString('en-US');

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

function formKey(kind: 'form' | 'ability' | 'status', form: FormId): string {
  if (kind === 'status') return `form.status.${STATUS[form]}`;
  return `${kind}.${form}`;
}

interface FormGlyphProps {
  form: FormId;
  selected?: boolean;
  rail?: boolean;
}

function FormGlyph({ form, selected = false, rail = false }: FormGlyphProps) {
  const canvas = useRef<HTMLCanvasElement | null>(null);

  useEffect(() => {
    const element = canvas.current;
    const context = element?.getContext('2d');
    if (!element || !context) return;

    const width = rail ? 188 : 132;
    const height = rail ? 148 : 112;
    const ratio = Math.min(2, window.devicePixelRatio || 1);
    element.width = Math.round(width * ratio);
    element.height = Math.round(height * ratio);
    context.setTransform(ratio, 0, 0, ratio, 0, 0);
    context.clearRect(0, 0, width, height);

    const ink = selected ? '#b9f2dc' : '#ded9d0';
    const low = selected ? 'rgba(136, 223, 192, 0.34)' : 'rgba(222, 217, 208, 0.24)';
    const accent = selected ? '#88dfc0' : '#c5a45f';
    const cx = width / 2;
    const cy = height / 2;
    const scale = rail ? 1.18 : 1;

    context.lineCap = 'round';
    context.lineJoin = 'round';
    context.strokeStyle = ink;
    context.fillStyle = ink;
    context.lineWidth = 1;
    context.shadowColor = selected ? 'rgba(136, 223, 192, 0.72)' : 'rgba(222, 217, 208, 0.28)';
    context.shadowBlur = selected ? 10 : 4;

    const node = (x: number, y: number, radius = 2.2): void => {
      context.beginPath();
      context.arc(x, y, radius, 0, Math.PI * 2);
      context.fill();
      context.beginPath();
      context.arc(x, y, radius + 3.2, 0, Math.PI * 2);
      context.strokeStyle = low;
      context.stroke();
      context.strokeStyle = ink;
    };

    const ellipse = (x: number, y: number, rx: number, ry: number, rotation = 0): void => {
      context.beginPath();
      context.ellipse(x, y, rx, ry, rotation, 0, Math.PI * 2);
      context.stroke();
    };

    const line = (points: ReadonlyArray<readonly [number, number]>): void => {
      context.beginPath();
      points.forEach(([x, y], place) => place === 0 ? context.moveTo(x, y) : context.lineTo(x, y));
      context.stroke();
    };

    context.save();
    context.translate(cx, cy);
    context.scale(scale, scale);
    context.translate(-cx, -cy);

    if (form === 'thread') {
      context.beginPath();
      context.moveTo(cx - 18, cy - 42);
      context.bezierCurveTo(cx + 30, cy - 23, cx - 30, cy + 20, cx + 18, cy + 42);
      context.stroke();
      context.beginPath();
      context.moveTo(cx + 18, cy - 42);
      context.bezierCurveTo(cx - 30, cy - 23, cx + 30, cy + 20, cx - 18, cy + 42);
      context.stroke();
      for (let y = -32; y <= 32; y += 16) {
        const spread = Math.abs(y) < 9 ? 21 : 14;
        line([[cx - spread, cy + y], [cx + spread, cy + y]]);
        node(cx + (y % 32 === 0 ? -spread : spread), cy + y, 1.6);
      }
    } else if (form === 'ring') {
      ellipse(cx, cy, 38, 25, -0.12);
      ellipse(cx, cy, 27, 17, -0.12);
      ellipse(cx, cy, 14, 9, -0.12);
      node(cx + 37, cy - 5);
      node(cx - 26, cy + 15, 1.8);
      node(cx + 4, cy, 2.7);
    } else if (form === 'relay') {
      const top: [number, number] = [cx, cy - 37];
      const left: [number, number] = [cx - 35, cy + 28];
      const right: [number, number] = [cx + 35, cy + 28];
      line([top, left, right, top]);
      line([top, [cx, cy + 3], left]);
      line([[cx, cy + 3], right]);
      line([[cx + 35, cy + 28], [cx + 51, cy + 8]]);
      node(...top);
      node(...left);
      node(...right);
      node(cx, cy + 3, 2.8);
      node(cx + 51, cy + 8, 1.8);
    } else if (form === 'vault') {
      line([[cx, cy - 43], [cx + 36, cy], [cx, cy + 41], [cx - 36, cy], [cx, cy - 43]]);
      line([[cx, cy - 43], [cx, cy + 41]]);
      line([[cx - 36, cy], [cx + 36, cy]]);
      line([[cx, cy - 43], [cx - 17, cy], [cx, cy + 41]]);
      line([[cx, cy - 43], [cx + 17, cy], [cx, cy + 41]]);
      ellipse(cx, cy, 8, 8);
      node(cx, cy, 2.7);
    } else if (form === 'lens') {
      context.beginPath();
      context.moveTo(cx - 48, cy);
      context.quadraticCurveTo(cx, cy - 42, cx + 48, cy);
      context.quadraticCurveTo(cx, cy + 42, cx - 48, cy);
      context.stroke();
      line([[cx, cy - 38], [cx + 25, cy], [cx, cy + 38], [cx - 25, cy], [cx, cy - 38]]);
      ellipse(cx, cy, 13, 13);
      node(cx, cy, 3.4);
    } else if (form === 'knot') {
      for (const rotation of [0, Math.PI / 3, -Math.PI / 3]) {
        ellipse(cx, cy, 38, 16, rotation);
      }
      ellipse(cx, cy, 14, 14);
      node(cx - 34, cy, 1.7);
      node(cx + 34, cy, 1.7);
      node(cx, cy - 34, 1.7);
    } else if (form === 'wake') {
      context.beginPath();
      context.moveTo(cx + 40, cy);
      context.bezierCurveTo(cx + 4, cy - 36, cx - 19, cy - 28, cx - 46, cy - 12);
      context.stroke();
      context.beginPath();
      context.moveTo(cx + 40, cy);
      context.bezierCurveTo(cx + 4, cy + 36, cx - 19, cy + 28, cx - 46, cy + 12);
      context.stroke();
      context.beginPath();
      context.moveTo(cx + 34, cy);
      context.bezierCurveTo(cx + 3, cy - 15, cx - 17, cy - 12, cx - 43, cy - 4);
      context.stroke();
      context.beginPath();
      context.moveTo(cx + 34, cy);
      context.bezierCurveTo(cx + 3, cy + 15, cx - 17, cy + 12, cx - 43, cy + 4);
      context.stroke();
      node(cx + 40, cy, 3.2);
      node(cx - 45, cy - 12, 1.5);
      node(cx - 45, cy + 12, 1.5);
    } else {
      const points = Array.from({ length: 6 }, (_, place) => {
        const angle = (Math.PI * 2 * place) / 6 - Math.PI / 2;
        return [cx + Math.cos(angle) * 34, cy + Math.sin(angle) * 31] as const;
      });
      points.forEach((point, place) => {
        line([point, points[(place + 2) % points.length]]);
        line([point, [cx, cy]]);
        node(point[0], point[1], place % 2 === 0 ? 2.5 : 1.8);
      });
      ellipse(cx, cy, 12, 12);
      node(cx, cy, 3);
    }

    context.restore();
    context.shadowBlur = 0;
    context.strokeStyle = accent;
    context.lineWidth = 0.8;
    context.beginPath();
    context.moveTo(width * 0.22, height - 9);
    context.lineTo(width * 0.78, height - 9);
    context.stroke();
  }, [form, rail, selected]);

  return <canvas className="form-glyph" data-form={form} data-rail={rail || undefined} ref={canvas} aria-hidden="true" />;
}

export function FormSelect(props: FormSelectProps) {
  const measured = props.regime !== undefined;
  const regime = props.regime ?? 'open_field';
  const onChoose = props.onChoose;
  const onBack = props.onBack ?? (() => {});
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
    else if (event.key === 'Enter' && STATUS[FORM_IDS[at]] !== 'pending') {
      onChoose(FORM_IDS[at]);
      event.preventDefault();
      return;
    } else return;
    event.preventDefault();
    roving.current = true;
    setAt(wanted);
  }

  return (
    <div className="opening opening-measured" data-regime={regime} data-form={FORM_IDS[at]}>
      <img
        className="opening-texture"
        src="/assets/number-2-field-texture.png"
        alt=""
        aria-hidden="true"
      />
      {measured ? (
        <button type="button" className="opening-back" onClick={onBack}>
          {copy('atlas.return')}
        </button>
      ) : null}
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
                aria-checked={measured && place === at}
                className="opening-form"
                data-status={STATUS[form]}
                data-form={form}
                // One tab stop for the group; the arrows move inside it.
                tabIndex={place === at ? 0 : -1}
                ref={(element) => {
                  buttons.current[place] = element;
                }}
                onFocus={() => setAt(place)}
                onClick={() => {
                  if (measured) setAt(place);
                  else onChoose(form);
                }}
              >
                <FormGlyph form={form} selected={measured && place === at} />
                <span className="opening-form-name">{held.name}</span>
                <span className="opening-form-promise">{held.promise}</span>
              </button>
            </li>
          );
        })}
      </ul>
      {measured ? <aside className="form-contract" aria-live="polite">
        <span className="form-contract-spine" aria-hidden="true" />
        <div className="form-contract-heading">
          <FormGlyph form={FORM_IDS[at]} selected rail />
          <div>
            <p>{copy(formKey('status', FORM_IDS[at]))}</p>
            <h2>{copy(formKey('form', FORM_IDS[at]))}</h2>
            <span>{copy(formKey('ability', FORM_IDS[at]))}</span>
          </div>
        </div>
        <dl>
          <div><dt>{copy('form.contract.steering')}</dt><dd>{(CONTRACTS[FORM_IDS[at]].steer_scale / 65_536).toFixed(2)}{copy('unit.multiplier')}</dd></div>
          <div><dt>{copy('form.contract.operating_limit')}</dt><dd>{units(CONTRACTS[FORM_IDS[at]].capacity)} {copy('unit.cu')}</dd></div>
          <div><dt>{copy('form.contract.upkeep')}</dt><dd>{units(CONTRACTS[FORM_IDS[at]].upkeep_rate * 30)} {copy('unit.cu_per_second')}</dd></div>
          <div><dt>{copy('form.contract.construction_span')}</dt><dd>{units(CONTRACTS[FORM_IDS[at]].route_reach)}</dd></div>
          <div><dt>{copy('form.contract.route_capacity')}</dt><dd>{units(CONTRACTS[FORM_IDS[at]].route_capacity * 30)} {copy('unit.cu_per_second')}</dd></div>
          <div><dt>{copy('form.contract.reserve')}</dt><dd>{units(CONTRACTS[FORM_IDS[at]].reserve)} {copy('unit.cu')}</dd></div>
          <div><dt>{copy('form.contract.forecast')}</dt><dd>{CONTRACTS[FORM_IDS[at]].forecast_depth} {copy('unit.steps')}</dd></div>
        </dl>
        <button
          type="button"
          className="form-begin"
          disabled={STATUS[FORM_IDS[at]] === 'pending'}
          onClick={() => onChoose(FORM_IDS[at])}
        >
          <span className="form-begin-line" aria-hidden="true" />
          <span>{copy(STATUS[FORM_IDS[at]] === 'pending' ? 'form.pending' : 'form.begin')}</span>
          <span className="form-begin-gate" aria-hidden="true" />
        </button>
      </aside> : null}
    </div>
  );
}
