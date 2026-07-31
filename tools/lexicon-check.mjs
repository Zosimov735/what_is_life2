#!/usr/bin/env node
//
// Approved-lexicon and copy-catalog check for the field_game workspace.
//
// Dependency-free on purpose: it runs on a bare node install before any
// package.json, bundler, or test framework exists, and later goals run it
// unchanged. See field_game/AGENTS.md for the invocation and
// docs/field-framework/LEXICON.md for the rules it enforces.
//
// It reports six kinds of violation:
//
//   prohibited-term          representational or collapsed-value wording
//   uncatalogued-text        player-facing text written inline in a component
//                            or in a page
//   unknown-copy-key         a catalog key that no entry defines
//   marker-missing-reason    an escape marker that gives no reason
//   catalog-*                the catalog is absent, malformed, or breaks a rule
//   outside-workspace-import a specifier that leaves the workspace
//
// Two line-level escape markers exist for the rare legitimate exception:
//
//   lexicon-check: allow-term   — <reason>
//   lexicon-check: allow-inline — <reason>
//
// A marker applies to its own line and to the line directly below it, and the
// reason is required: a marker without one excuses nothing and is reported.

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const TOOLS_DIR = path.dirname(fileURLToPath(import.meta.url));
const DEFAULT_WORKSPACE = path.resolve(TOOLS_DIR, '..');

/** Where the single authored copy catalog lives, relative to the workspace root. */
const CATALOG_RELATIVE = path.join('content', 'copy', 'catalog.json');

/** Extensions carrying text the lexicon rules apply to. */
const SCANNED_EXTENSIONS = new Set([
  '.ts', '.tsx', '.js', '.jsx', '.mjs', '.cjs',
  '.rs', '.json', '.md', '.css', '.html', '.toml', '.yml', '.yaml',
]);

/** Extensions whose string literals and markup can reach a player. */
const UI_EXTENSIONS = new Set(['.ts', '.tsx', '.js', '.jsx', '.mjs', '.cjs']);

/** Extensions that carry module specifiers. */
const IMPORTING_EXTENSIONS = new Set(['.ts', '.tsx', '.js', '.jsx', '.mjs', '.cjs']);

/** Directory names never scanned, wherever they appear. */
const SKIPPED_DIRECTORIES = new Set([
  '.git', 'node_modules', 'target', 'dist', 'pkg', 'wasm-pkg', 'build', '.vite',
]);

/**
 * Paths excluded from the scan, relative to the workspace root, each for a
 * stated reason. Anything added here needs a reason in AGENTS.md.
 */
const EXCLUDED_PATHS = new Map([
  [path.join('tools', 'lexicon-data.json'), 'must spell out the terms the check rejects'],
  [path.join('tools', 'fixtures'), 'holds deliberately invalid input for the check tests'],
  ['package-lock.json', 'generated'],
  ['Cargo.lock', 'generated'],
]);

/** Documents supplied verbatim by the project owner, quoted rather than authored. */
const EXCLUDED_DOCUMENTS = new Set(['SPEC.md', 'PLAN.md']);

/** The accessor every player-facing string must pass through. */
const COPY_ACCESSOR = 'copy';

/** Attributes whose value a screen reader or tooltip shows to a player. */
const DISPLAY_ATTRIBUTES = [
  'aria-label', 'aria-description', 'aria-placeholder', 'aria-valuetext',
  'aria-roledescription', 'placeholder', 'title', 'alt', 'label',
];

// ---------------------------------------------------------------------------
// Word normalisation
// ---------------------------------------------------------------------------

/**
 * Reduces a line to lowercase words, splitting identifiers on case changes and
 * on separators, so a prohibited word is found in `wordCharge`, `word_charge`
 * and plain prose alike. Whole-word matching then keeps an innocent substring
 * of a longer, unrelated word from being reported.
 */
function toWords(line) {
  const words = [];
  for (const chunk of line.split(/[^A-Za-z0-9]+/)) {
    if (!chunk) continue;
    const parts = chunk.split(/(?<=[a-z0-9])(?=[A-Z])|(?<=[A-Z])(?=[A-Z][a-z])/);
    for (const part of parts) {
      if (part) words.push(part.toLowerCase());
    }
  }
  return words;
}

function countWords(text) {
  return text.trim().split(/\s+/).filter(Boolean).length;
}

function escapeForRegExp(text) {
  return text.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

// ---------------------------------------------------------------------------
// Source tokenising
// ---------------------------------------------------------------------------

/**
 * Splits a source file into string literals, comments, and a masked copy in
 * which every string body and comment body is blanked out. The masked copy
 * keeps the original length and line breaks, so offsets and line numbers stay
 * usable, and markup can be scanned without tripping over text that merely
 * looks like markup inside a string.
 */
function tokenizeSource(text) {
  const masked = text.split('');
  const strings = [];
  const comments = [];
  const blank = (index) => {
    if (masked[index] !== '\n') masked[index] = ' ';
  };

  let index = 0;
  let line = 1;
  const length = text.length;

  while (index < length) {
    const char = text[index];
    const next = text[index + 1];

    if (char === '\n') {
      line += 1;
      index += 1;
      continue;
    }

    // Line comment, including the Rust doc-comment forms.
    if (char === '/' && next === '/') {
      const start = index;
      while (index < length && text[index] !== '\n') blank(index++);
      comments.push({ value: text.slice(start, index), line });
      continue;
    }

    // Block comment.
    if (char === '/' && next === '*') {
      const start = index;
      const startLine = line;
      blank(index++);
      blank(index++);
      while (index < length && !(text[index] === '*' && text[index + 1] === '/')) {
        if (text[index] === '\n') line += 1;
        blank(index++);
      }
      blank(index++);
      blank(index++);
      comments.push({ value: text.slice(start, index), line: startLine });
      continue;
    }

    // An apostrophe in prose — `Don't` — follows a letter or a digit, where no
    // string literal may open. Reading it as a quote would blank the rest of
    // the line and hide the markup around it, so it is left where it stands.
    const isApostrophe = char === "'" && /[A-Za-z0-9]/.test(text[index - 1] ?? '');

    if (!isApostrophe && (char === '"' || char === "'" || char === '`')) {
      const quote = char;
      const startLine = line;
      const openIndex = index;
      index += 1; // step past the opening quote
      let value = '';
      while (index < length) {
        const inner = text[index];
        if (inner === '\\') {
          value += text.slice(index, index + 2);
          blank(index++);
          blank(index++);
          continue;
        }
        if (inner === quote) break;
        if (inner === '\n') {
          // Only a template literal legitimately spans lines; treat an
          // unterminated ordinary quote as ending at the line break.
          if (quote !== '`') break;
          line += 1;
        }
        value += inner;
        blank(index++);
      }
      if (text[index] === quote) index += 1;

      const before = text.slice(Math.max(0, openIndex - 80), openIndex);
      strings.push({
        value: quote === '`' ? value.replace(/\$\{[^}]*\}/g, ' ') : value,
        raw: value,
        quote,
        line: startLine,
        before,
      });
      continue;
    }

    index += 1;
  }

  return { masked: masked.join(''), strings, comments };
}

/**
 * Markup text nodes: a run of literal text between two of `< > { }`. A run
 * ends at a brace as well as at a bracket, so text written beside an
 * expression — `<h2>Objective: {copy('…')}</h2>` — is read as two parts and
 * the authored half is still seen.
 *
 * A run is markup text when the bracket before it closes a tag or the bracket
 * after it opens a closing tag. Requiring one of those keeps ordinary code —
 * a comparison, a type parameter, the body of an object literal — out of the
 * scan, since none of it sits against a tag.
 *
 * Telling a tag from a generic argument list needs the character before the
 * `<` as well as the one after it: a tag opens after a break in the code,
 * while `Array<number>` opens against the identifier it belongs to. Without
 * that, two members of a type body — one ending at `>`, the next opening at
 * `<` — read as a tag pair with the member name between them as its text.
 *
 * A run that ends at a closing brace is the one exception. Child text ends at
 * the next tag or at the brace that opens an expression, never at the brace
 * that closes one, so that shape is the tail of an expression rather than
 * text: the `: null` of `{cond ? <El /> : null}` reads as a text node
 * otherwise.
 */
function findMarkupText(masked) {
  const brackets = [];
  let previousAngle = -1;

  // `<` opens a tag when a name follows it and nothing it could belong to
  // precedes it. An identifier, a closing bracket, or a closing parenthesis in
  // front of it makes it a generic argument list or a comparison instead.
  const opensTag = (index) =>
    masked[index] === '<' &&
    /[A-Za-z/]/.test(masked[index + 1] ?? '') &&
    !/[A-Za-z0-9_$)\]]/.test(masked[index - 1] ?? '');

  for (let index = 0; index < masked.length; index += 1) {
    const char = masked[index];
    if (char !== '<' && char !== '>' && char !== '{' && char !== '}') continue;

    // `>` closes a tag when the angle bracket before it opened one. `=>` is an
    // arrow, and a lone `a > b` has no opening bracket behind it to close.
    const closesTag =
      char === '>' &&
      masked[index - 1] !== '=' &&
      previousAngle !== -1 &&
      opensTag(previousAngle);

    brackets.push({
      index,
      closesTag,
      opensClosingTag: char === '<' && masked[index + 1] === '/',
      closesExpression: char === '}',
    });
    if (char === '<' || char === '>') previousAngle = index;
  }

  const found = [];
  for (let position = 1; position < brackets.length; position += 1) {
    const opening = brackets[position - 1];
    const closing = brackets[position];
    const opensChildren = opening.closesTag && !closing.closesExpression;
    if (!opensChildren && !closing.opensClosingTag) continue;

    const start = opening.index + 1;
    const raw = masked.slice(start, closing.index);
    const trimmed = raw.trim();
    if (!trimmed) continue;
    const offset = start + raw.indexOf(trimmed[0]);
    found.push({
      text: trimmed,
      line: masked.slice(0, offset).split('\n').length,
    });
  }
  return found;
}

/**
 * Text nodes in a page. A page carries the document title and nothing else a
 * player reads (AGENTS.md), so `title` is passed over along with the two
 * elements whose bodies are code, `script` and `style`.
 *
 * Only `<!--` opens a comment here. The `//` of a URL in an attribute opens
 * nothing, which is why a page is read by this reader rather than by the
 * source tokeniser.
 */
function findHtmlText(text) {
  const passedOver = /^<\s*(script|style|title)\b/i;
  const found = [];

  /** Records the text run beginning at `start`, and returns where it ends. */
  const takeText = (start) => {
    const nextTag = text.indexOf('<', start);
    const end = nextTag === -1 ? text.length : nextTag;
    const raw = text.slice(start, end);
    const trimmed = raw.trim();
    if (trimmed) {
      const offset = start + raw.indexOf(trimmed[0]);
      found.push({ text: trimmed, line: text.slice(0, offset).split('\n').length });
    }
    return end;
  };

  let index = 0;
  while (index < text.length) {
    if (text.startsWith('<!--', index)) {
      const end = text.indexOf('-->', index + 4);
      index = end === -1 ? text.length : takeText(end + 3);
      continue;
    }

    if (text[index] !== '<') {
      index = takeText(index);
      continue;
    }

    const tagEnd = text.indexOf('>', index);
    if (tagEnd === -1) break;

    const element = passedOver.exec(text.slice(index, tagEnd + 1));
    if (element) {
      const closing = new RegExp(`</\\s*${element[1]}\\s*>`, 'i').exec(text.slice(tagEnd));
      index = closing ? takeText(tagEnd + closing.index + closing[0].length) : text.length;
      continue;
    }

    index = takeText(tagEnd + 1);
  }

  return found;
}

// ---------------------------------------------------------------------------
// Player-facing text detection
// ---------------------------------------------------------------------------

/** Two or more letter runs of at least two letters each. */
function hasSeveralWords(text) {
  return (text.match(/[A-Za-z]{2,}/g) ?? []).length >= 2;
}

/** True when a run of characters reads as a sentence a player could be shown. */
function readsAsProse(text) {
  const trimmed = text.trim();
  if (trimmed.length < 6) return false;
  if (!/\s/.test(trimmed)) return false;
  if (!hasSeveralWords(trimmed)) return false;

  // Technical shapes: markup, code, selectors, colours, measurements, paths.
  if (/[(){}<>;=|\\]/.test(trimmed)) return false;
  if (trimmed.includes('://') || trimmed.includes('#')) return false;
  if (/[A-Za-z]\.[A-Za-z]/.test(trimmed)) return false;
  if (/^[\d\s.,%:-]+$/.test(trimmed)) return false;
  if (/\b\d+(px|rem|em|ms|fps|vh|vw)\b/.test(trimmed)) return false;

  // Authored copy is a capitalised sentence; lowercase technical phrases such
  // as "use strict" or "still mode change" are left alone.
  const startsCapitalised = /^[A-Z]/.test(trimmed);
  const endsSentence = /[A-Za-z][.!?]("|')?$/.test(trimmed);
  return startsCapitalised || endsSentence;
}

/** Developer-facing call sites whose literals never reach a player. */
function isDiagnosticContext(before) {
  return (
    /(?:new\s+)?(?:Error|TypeError|RangeError)\s*\(\s*$/.test(before) ||
    /console\.\w+\s*\(\s*$/.test(before) ||
    /\bassert\w*(?:\.\w+)?\s*\(\s*$/.test(before) ||
    /\b(?:describe|it|test|suite)\s*\(\s*$/.test(before) ||
    /\b(?:panic|unreachable|todo|expect|debug_assert)!\s*\(\s*$/.test(before)
  );
}

/** Module specifier positions. */
function isSpecifierContext(before) {
  return (
    /\bfrom\s*$/.test(before) ||
    /\bimport\s*\(?\s*$/.test(before) ||
    /\brequire\s*\(\s*$/.test(before) ||
    /\bexport\s+\*\s+from\s*$/.test(before) ||
    /\binclude_str!\s*\(\s*$/.test(before)
  );
}

function isCopyAccessorContext(before) {
  return new RegExp(`\\b${COPY_ACCESSOR}\\s*\\(\\s*$`).test(before);
}

function displayAttributeAt(before) {
  for (const attribute of DISPLAY_ATTRIBUTES) {
    if (new RegExp(`\\b${escapeForRegExp(attribute)}\\s*=\\s*$`, 'i').test(before)) {
      return attribute;
    }
  }
  return null;
}

// ---------------------------------------------------------------------------
// Escape markers
// ---------------------------------------------------------------------------

const MARKER_PATTERN = /lexicon-check:\s*allow-(term|inline)\s*(?:[—:-]\s*(.*))?$/;

/**
 * Maps each line number to the markers covering it: a marker covers its own
 * line and the line below, so it can sit above the code it excuses.
 *
 * The reason is what makes a marker a considered exception rather than a way
 * past a rule, so one written without a reason covers nothing at all and is
 * returned as a fault of its own.
 */
function collectMarkers(lines) {
  const coverage = new Map();
  const unreasoned = [];
  const add = (lineNumber, kind, reason) => {
    if (!coverage.has(lineNumber)) coverage.set(lineNumber, []);
    coverage.get(lineNumber).push({ kind, reason });
  };
  lines.forEach((text, offset) => {
    const match = text.match(MARKER_PATTERN);
    if (!match) return;
    const kind = match[1];
    const reason = (match[2] ?? '').trim();
    if (!reason) {
      unreasoned.push({ line: offset + 1, kind });
      return;
    }
    add(offset + 1, kind, reason);
    add(offset + 2, kind, reason);
  });
  return { coverage, unreasoned };
}

// ---------------------------------------------------------------------------
// File walking
// ---------------------------------------------------------------------------

function walk(root, isExcluded) {
  const found = [];
  const visit = (absolute) => {
    let entries;
    try {
      entries = fs.readdirSync(absolute, { withFileTypes: true });
    } catch {
      return;
    }
    for (const entry of entries) {
      const child = path.join(absolute, entry.name);
      if (entry.isDirectory()) {
        if (SKIPPED_DIRECTORIES.has(entry.name)) continue;
        if (isExcluded(child)) continue;
        visit(child);
        continue;
      }
      if (!entry.isFile()) continue;
      if (!SCANNED_EXTENSIONS.has(path.extname(entry.name))) continue;
      if (isExcluded(child)) continue;
      found.push(child);
    }
  };
  visit(root);
  return found.sort();
}

function isInside(root, candidate) {
  const relative = path.relative(root, candidate);
  return relative !== '' && !relative.startsWith('..') && !path.isAbsolute(relative);
}

// ---------------------------------------------------------------------------
// The check
// ---------------------------------------------------------------------------

class Report {
  constructor(workspaceRoot, repoRoot) {
    this.workspaceRoot = workspaceRoot;
    this.repoRoot = repoRoot;
    this.violations = [];
  }

  /** Workspace-relative where possible, repository-relative otherwise. */
  label(absolute) {
    const relative = path.relative(this.workspaceRoot, absolute);
    const chosen = relative.startsWith('..')
      ? path.relative(this.repoRoot, absolute)
      : relative;
    return chosen.split(path.sep).join('/');
  }

  add(code, file, line, message, extra = {}) {
    this.violations.push({ code, file: this.label(file), line, message, ...extra });
  }
}

function loadData() {
  const dataPath = path.join(TOOLS_DIR, 'lexicon-data.json');
  const data = JSON.parse(fs.readFileSync(dataPath, 'utf8'));
  const prohibited = new Map();
  for (const [category, terms] of Object.entries(data.prohibited)) {
    for (const term of terms) prohibited.set(term, category);
  }
  return { ...data, prohibitedIndex: prohibited };
}

/** Rule A — prohibited wording anywhere in a scanned file. */
function checkProhibitedTerms(report, file, lines, markers, prohibitedIndex, isMarkdown) {
  lines.forEach((text, offset) => {
    const lineNumber = offset + 1;
    if ((markers.get(lineNumber) ?? []).some((m) => m.kind === 'term')) return;

    // A term inside a Markdown code span is being named, not used.
    const scannable = isMarkdown ? text.replace(/`[^`]*`/g, ' ') : text;

    const reported = new Set();
    for (const word of toWords(scannable)) {
      if (reported.has(word)) continue;
      const category = prohibitedIndex.get(word);
      if (!category) continue;
      reported.add(word);
      report.add(
        'prohibited-term',
        file,
        lineNumber,
        `"${word}" is outside the approved vocabulary (${category}).`,
        { term: word, category },
      );
    }
  });
}

/** Rules B and D — inline player-facing text, copy keys, and module specifiers. */
function checkSource(report, file, text, lines, markers, options) {
  const extension = path.extname(file);
  const { masked, strings } = tokenizeSource(text);
  const allowsInline = (lineNumber) =>
    (markers.get(lineNumber) ?? []).some((m) => m.kind === 'inline');

  const usedKeys = [];

  for (const literal of strings) {
    const { value, before, line } = literal;

    if (isSpecifierContext(before)) {
      if (options.checkSpecifiers) {
        checkSpecifier(report, file, line, value, options.workspaceRoot);
      }
      continue;
    }

    if (isCopyAccessorContext(before)) {
      usedKeys.push({ key: value, line, file });
      continue;
    }

    if (!options.checkInlineText) continue;
    if (isDiagnosticContext(before)) continue;
    if (allowsInline(line)) continue;

    const attribute = displayAttributeAt(before);
    if (attribute) {
      if (hasSeveralWords(value) || /[A-Za-z]{3,}/.test(value)) {
        report.add(
          'uncatalogued-text',
          file,
          line,
          `${attribute} is set from an inline string; read it from the copy catalog.`,
        );
      }
      continue;
    }

    if (readsAsProse(value)) {
      report.add(
        'uncatalogued-text',
        file,
        line,
        'Inline sentence in source; move it to the copy catalog.',
      );
    }
  }

  if (options.checkInlineText && (extension === '.tsx' || extension === '.jsx')) {
    reportMarkupText(report, file, findMarkupText(masked), allowsInline);
  }

  // Rust files carry no copy, but they must not reach outside the workspace.
  if (options.checkSpecifiers && extension === '.rs') {
    lines.forEach((lineText, offset) => {
      const match = lineText.match(/include_str!\s*\(\s*"([^"]+)"/);
      if (match) checkSpecifier(report, file, offset + 1, match[1], options.workspaceRoot);
    });
  }

  return usedKeys;
}

/**
 * Reports the text nodes that read as authored wording rather than as code.
 * Markup and pages share it, so a sentence is treated the same wherever it is
 * written.
 */
function reportMarkupText(report, file, nodes, allowsInline) {
  for (const node of nodes) {
    if (allowsInline(node.line)) continue;
    if (!/[A-Za-z]{2,}/.test(node.text)) continue;
    if (/[(){}<>;=|\\]/.test(node.text)) continue;
    if (/[A-Za-z]\.[A-Za-z]/.test(node.text)) continue;
    report.add(
      'uncatalogued-text',
      file,
      node.line,
      'Markup text node written inline; read it from the copy catalog.',
    );
  }
}

/** Rule B for pages: a page carries its title and no other text a player reads. */
function checkPage(report, file, text, markers) {
  const allowsInline = (lineNumber) =>
    (markers.get(lineNumber) ?? []).some((m) => m.kind === 'inline');
  reportMarkupText(report, file, findHtmlText(text), allowsInline);
}

function checkSpecifier(report, file, line, specifier, workspaceRoot) {
  if (specifier.startsWith('.')) {
    const resolved = path.resolve(path.dirname(file), specifier);
    if (!isInside(workspaceRoot, resolved)) {
      report.add(
        'outside-workspace-import',
        file,
        line,
        `"${specifier}" resolves outside the workspace.`,
      );
    }
    return;
  }
  if (path.isAbsolute(specifier) || specifier.startsWith('file:')) {
    report.add(
      'outside-workspace-import',
      file,
      line,
      `"${specifier}" is an absolute path outside the workspace.`,
    );
  }
}

/** Rule C — the catalog exists, parses, and obeys the writing rules. */
function checkCatalog(report, catalogPath, data) {
  if (!fs.existsSync(catalogPath)) {
    report.add(
      'catalog-missing',
      catalogPath,
      0,
      'No copy catalog; every player-facing string needs one to come from.',
    );
    return null;
  }

  let catalog;
  try {
    catalog = JSON.parse(fs.readFileSync(catalogPath, 'utf8'));
  } catch (error) {
    report.add('catalog-unparsable', catalogPath, 0, error.message);
    return null;
  }

  if (typeof catalog.catalogVersion !== 'number' || typeof catalog.locale !== 'string') {
    report.add(
      'catalog-invalid',
      catalogPath,
      0,
      'The catalog needs a numeric catalogVersion and a locale.',
    );
  }
  if (!catalog.entries || typeof catalog.entries !== 'object') {
    report.add('catalog-invalid', catalogPath, 0, 'The catalog needs an entries object.');
    return null;
  }

  const lines = fs.readFileSync(catalogPath, 'utf8').split('\n');
  const lineOf = (key) => {
    const index = lines.findIndex((line) => line.includes(`"${key}"`));
    return index === -1 ? 0 : index + 1;
  };

  // The key format LEXICON.md states, and the one the core's own reader holds
  // authored keys to: the kind opens the key, and every segment after it opens
  // with a lowercase letter. The three must agree, so the strictest of them is
  // what all three are written to.
  const keyPattern = /^[a-z][a-z0-9]*(?:\.[a-z][a-z0-9_]*)+$/;

  for (const [key, entry] of Object.entries(catalog.entries)) {
    const line = lineOf(key);

    if (!keyPattern.test(key)) {
      report.add(
        'catalog-key-format',
        catalogPath,
        line,
        `"${key}" must be lowercase dotted, such as objective.follow_current.`,
      );
    }

    if (!entry || typeof entry.text !== 'string' || typeof entry.kind !== 'string') {
      report.add(
        'catalog-entry-invalid',
        catalogPath,
        line,
        `"${key}" needs a kind and a text.`,
      );
      continue;
    }

    const kindRule = data.copyKinds[entry.kind];
    if (!kindRule) {
      report.add(
        'catalog-entry-invalid',
        catalogPath,
        line,
        `"${key}" uses the unknown kind "${entry.kind}".`,
      );
      continue;
    }

    if (kindRule.maxWords && countWords(entry.text) > kindRule.maxWords) {
      report.add(
        'instruction-too-long',
        catalogPath,
        line,
        `"${key}" runs to ${countWords(entry.text)} words; the limit is ${kindRule.maxWords}.`,
      );
    }

    if (kindRule.canonicalSet) {
      const names = data.canonical[kindRule.canonicalSet];
      if (!names.includes(entry.text)) {
        report.add(
          'catalog-entry-invalid',
          catalogPath,
          line,
          `"${key}" must use a name from ${kindRule.canonicalSet}.`,
        );
      }
    }

    for (const label of data.canonical.interfaceLabels) {
      const boundary = /[A-Za-z0-9]$/.test(label) ? '(?![A-Za-z0-9])' : '';
      const pattern = new RegExp(
        `(?<![A-Za-z0-9])${escapeForRegExp(label)}${boundary}`,
        'gi',
      );
      for (const match of entry.text.matchAll(pattern)) {
        if (match[0] !== label) {
          report.add(
            'label-spelling',
            catalogPath,
            line,
            `"${key}" writes "${match[0]}"; the label is "${label}".`,
          );
        }
      }
    }
  }

  return catalog;
}

/**
 * Reads the command line. A usage fault is returned rather than exited on, so
 * the caller decides the exit code and nothing already written is cut short.
 */
function parseArguments(argv) {
  const options = { root: null, json: false, help: false, usageError: null };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === '--json') options.json = true;
    else if (argument === '--help' || argument === '-h') options.help = true;
    else if (argument === '--root' || argument.startsWith('--root=')) {
      // A missing directory is a mistake worth naming: scanning the default
      // tree instead would report a pass the caller never asked for. In the
      // two-token form the next option is a missing value, not a directory.
      const separate = argument === '--root';
      const value = separate ? argv[++index] : argument.slice(7);
      if (!value || (separate && value.startsWith('-'))) {
        options.usageError = '--root needs a directory';
        return options;
      }
      options.root = value;
    } else {
      options.usageError = `unknown argument "${argument}"`;
      return options;
    }
  }
  return options;
}

const HELP = `Approved-lexicon and copy-catalog check for the field_game workspace.

Usage:
  node field_game/tools/lexicon-check.mjs [--root <dir>] [--json]

  --root <dir>  Treat <dir> as the workspace root and scan only it. Without it
                the check scans field_game/ plus docs/field-framework/.
  --json        Emit a machine-readable report on stdout.
  --help        Show this text.

Exit codes: 0 clean, 1 violations found, 2 bad usage.

Rules: docs/field-framework/LEXICON.md. Invocation: field_game/AGENTS.md.
`;

function main() {
  const options = parseArguments(process.argv.slice(2));
  if (options.usageError) {
    process.stderr.write(`lexicon-check: ${options.usageError}\n`);
    return 2;
  }
  if (options.help) {
    process.stdout.write(HELP);
    return 0;
  }

  const data = loadData();
  const workspaceRoot = options.root
    ? path.resolve(options.root)
    : DEFAULT_WORKSPACE;
  const repoRoot = path.resolve(workspaceRoot, '..');
  const report = new Report(workspaceRoot, repoRoot);

  const isExcluded = (absolute) => {
    const relative = path.relative(workspaceRoot, absolute);
    for (const excluded of EXCLUDED_PATHS.keys()) {
      if (relative === excluded || relative.startsWith(`${excluded}${path.sep}`)) {
        return true;
      }
    }
    return false;
  };

  // The workspace is scanned in full. The framework documents are scanned for
  // wording only, and only when the check runs on the real workspace.
  const workspaceFiles = walk(workspaceRoot, isExcluded);
  const documentFiles = options.root
    ? []
    : walk(path.join(repoRoot, 'docs', 'field-framework'), () => false).filter(
        (file) => !EXCLUDED_DOCUMENTS.has(path.basename(file)),
      );

  const catalogPath = path.join(workspaceRoot, CATALOG_RELATIVE);
  const catalog = checkCatalog(report, catalogPath, data);

  const usedKeys = [];
  for (const file of [...workspaceFiles, ...documentFiles]) {
    const text = fs.readFileSync(file, 'utf8');
    const lines = text.split('\n');
    const { coverage: markers, unreasoned } = collectMarkers(lines);
    const extension = path.extname(file);
    const isDocument = documentFiles.includes(file);

    for (const marker of unreasoned) {
      report.add(
        'marker-missing-reason',
        file,
        marker.line,
        `The allow-${marker.kind} marker gives no reason, so it excuses nothing.`,
      );
    }

    checkProhibitedTerms(
      report, file, lines, markers, data.prohibitedIndex, extension === '.md',
    );

    if (isDocument) continue;

    const relative = path.relative(workspaceRoot, file);
    const isTest = /\.(test|spec)\./.test(path.basename(file));
    const isTooling = relative.split(path.sep)[0] === 'tools';

    if (UI_EXTENSIONS.has(extension) || extension === '.rs') {
      usedKeys.push(
        ...checkSource(report, file, text, lines, markers, {
          workspaceRoot,
          // Tests and tooling print developer-facing text by nature.
          checkInlineText: UI_EXTENSIONS.has(extension) && !isTest && !isTooling,
          checkSpecifiers: IMPORTING_EXTENSIONS.has(extension) || extension === '.rs',
        }),
      );
    }

    if (extension === '.html' && !isTooling) checkPage(report, file, text, markers);
  }

  if (catalog) {
    for (const used of usedKeys) {
      if (!Object.prototype.hasOwnProperty.call(catalog.entries, used.key)) {
        report.add(
          'unknown-copy-key',
          used.file,
          used.line,
          `No catalog entry defines "${used.key}".`,
        );
      }
    }
  }

  const ok = report.violations.length === 0;
  const summary = {
    ok,
    workspaceRoot,
    catalog: {
      path: report.label(catalogPath),
      entryCount: catalog ? Object.keys(catalog.entries).length : 0,
    },
    scannedFileCount: workspaceFiles.length + documentFiles.length,
    violations: report.violations,
  };

  if (options.json) {
    process.stdout.write(`${JSON.stringify(summary, null, 2)}\n`);
  } else if (ok) {
    process.stdout.write(
      `lexicon-check: ${summary.scannedFileCount} files scanned, ` +
        `${summary.catalog.entryCount} catalog entries, no violations.\n`,
    );
  } else {
    for (const violation of report.violations) {
      process.stdout.write(
        `${violation.file}:${violation.line}  ${violation.code}  ${violation.message}\n`,
      );
    }
    process.stdout.write(
      `\nlexicon-check: ${report.violations.length} violation(s). ` +
        'See docs/field-framework/LEXICON.md.\n',
    );
  }

  return ok ? 0 : 1;
}

// Set rather than exited on: `process.exit` can cut off a report still being
// written to a pipe.
process.exitCode = main();
