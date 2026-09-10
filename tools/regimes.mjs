// The fifteen latency regimes and the typist behind them: what a bench types, for both benches.
//
//   node tools/regimes.mjs                                  the fifteen regimes, one line each
//   node tools/regimes.mjs prose_end_of_draft --keys 300    the regime and every step it types
//   node tools/regimes.mjs revision --uinput                the plan line tools/uinput-keys.py reads
//
// `tools/gate bench` measures the native app and `legacy/tools/latency.mjs` measures the Parity
// oracle, and their numbers can only be compared if both type the same keys in the same order. So
// the regime definitions, the passages, the mixes, the shift table and the labels live here and
// both import them; neither owns a copy. The streams are seeded per regime name, so every session
// types an identical stream and the spread between sessions is the machine, not the script.
//
// Nothing here opens a browser, a window or a display: it is definitions and pure functions, and
// `--plan <regime>` on the legacy bench prints what this module produces so the two stay honest.
import { pathToFileURL } from 'node:url';

// The seeded generator behind every stream here, and the one a bench's bootstrap resamples with:
// one definition, so a seed means the same thing in the keys typed and in the intervals reported.
export function mulberry32(a) { return function () { a |= 0; a = a + 0x6D2B79F5 | 0; let t = Math.imul(a ^ a >>> 15, 1 | a); t = t + Math.imul(t ^ t >>> 7, 61 | t) ^ t; return ((t ^ t >>> 14) >>> 0) / 4294967296; }; }
export function hash32(s) { let h = 2166136261; for (let i = 0; i < s.length; i++) { h ^= s.charCodeAt(i); h = Math.imul(h, 16777619); } return h >>> 0; }

// ---------- the typist ----------
// Real writing, not one repeated sentence: capitals, commas, quotes, apostrophes, dashes,
// sentence ends, paragraph breaks, and the corrections everybody makes while drafting.
const PROSE = `The letter arrived on a Tuesday, unsigned, folded twice, and pushed under the door before anyone was awake. Marguerite read it standing up, still holding the kettle. "Not today," she said, to nobody in particular; the room, which had heard worse, said nothing back. She counted the reasons to go — there were four, and two of them were the same reason wearing a different coat — and then she put the kettle down and went anyway.
`;
const MARKDOWN = `## A note on measurement

The *fastest* editor is the one that **never** makes you wait for a character, and the only way to know is to count them. Read the frame back at presentation:

\`\`\`js
const t0 = event.timeStamp;
requestAnimationFrame(() => report(performance.now() - t0));
\`\`\`

> A number without a keystroke count is a number about nothing.

`;
const LETTERS = 'the quick brown fox jumps over the lazy dog while alice considers the pleasure of making a daisy chain ';
export const PASTE_TEXT = 'There is no such thing as a small delay in a text editor: the eye notices a tenth of a frame, and the hand notices before the eye does.\n\n';

const PUNCT = `.,;:'"!?-()`;
function charStep(ch, mdMode) {
  if (ch === '\n') return { press: 'Enter', label: 'enter', keydowns: 1 };
  if (ch === ' ') return { press: 'Space', label: 'space', keydowns: 1 };
  if (/[a-z]/.test(ch)) return { press: ch, label: 'letter', keydowns: 1 };
  if (/[A-Z]/.test(ch)) return { press: ch, label: 'capital', keydowns: 1 };
  if (/[*_#`>\[\]]/.test(ch)) return { press: ch, label: 'markdown', keydowns: 1 };
  if (ch === '—') return { press: '-', label: 'punct', keydowns: 1 };          // no em-dash key on a US layout
  if (PUNCT.includes(ch)) return { press: ch, label: 'punct', keydowns: 1 };
  return { press: ch, label: mdMode ? 'markdown' : 'punct', keydowns: 1 };
}
const BACKSPACE = { press: 'Backspace', label: 'backspace', keydowns: 1 };
const UNDO = { press: 'Control+z', label: 'undo', keydowns: 2, labels: ['modifier', 'undo'] };
const PASTE = { press: 'Control+v', label: 'paste', keydowns: 2, labels: ['modifier', 'paste'] };
const SELLEFT = { press: 'Shift+ArrowLeft', label: 'nav', keydowns: 2, labels: ['modifier', 'nav'] };

// Each generator returns exactly `n` steps (a step is one key press; a chord is one step with two
// keydowns). Seeded per regime name, so every session types an identical stream and the spread
// between sessions is the machine, not the script.
export function script(mix, n, seed) {
  const rnd = mulberry32(seed);
  const out = [];
  const push = (s) => { if (out.length < n) out.push(s); };
  if (mix === 'letters') {
    for (let i = 0; out.length < n; i++) push(charStep(LETTERS[i % LETTERS.length]));
  } else if (mix === 'prose' || mix === 'paste') {
    let i = 0, since = 0, sincePaste = 0;
    while (out.length < n) {
      push(charStep(PROSE[i++ % PROSE.length]));
      since++; sincePaste++;
      if (mix === 'paste' && sincePaste > 24) { push(PASTE); sincePaste = 0; since = 0; continue; }
      if (since > 34 + Math.floor(rnd() * 26)) {                 // a typo, noticed and fixed
        const k = 1 + Math.floor(rnd() * 3);
        for (let j = 0; j < k; j++) push(BACKSPACE);
        since = 0;
      }
    }
  } else if (mix === 'newlines') {
    // Paragraph churn: short lines and Enter, in the middle of the document. Every Enter changes
    // the line count, which is the branch render() treats differently from typing inside a line.
    let i = 0;
    while (out.length < n) {
      const len = 4 + Math.floor(rnd() * 6);
      for (let j = 0; j < len; j++) push(charStep(PROSE[i++ % PROSE.length]));
      push(charStep('\n'));
    }
  } else if (mix === 'revision') {
    // Write a word, select it back, replace it; undo now and then. Selection-heavy editing.
    let i = 0;
    while (out.length < n) {
      const len = 4 + Math.floor(rnd() * 5);
      for (let j = 0; j < len; j++) { const c = PROSE[i++ % PROSE.length]; push(charStep(/[a-zA-Z]/.test(c) ? c.toLowerCase() : 'e')); }
      for (let j = 0; j < len; j++) push(SELLEFT);
      for (let j = 0; j < len; j++) push(charStep('abcdefgh'[j % 8]));
      push(charStep(' '));
      if (rnd() < 0.25) push(UNDO);
    }
  } else if (mix === 'fences') {
    // Open a fenced code block in the middle of the document and close it again, over and over.
    // The third backtick changes the context of every line below it to the end of the document;
    // the first backspace changes them all back. It is the most expensive thing a single
    // keystroke can ask a Markdown editor to do, and it is one key.
    while (out.length < n) {
      for (let j = 0; j < 3; j++) push(charStep('`'));
      for (let j = 0; j < 3; j++) push(BACKSPACE);
    }
  } else if (mix === 'markdown') {
    let i = 0;
    while (out.length < n) push(charStep(MARKDOWN[i++ % MARKDOWN.length], true));
  } else throw new Error('unknown mix ' + mix);
  return out.slice(0, n);
}
export function labelsOf(steps) {             // one label per keydown, in order
  const out = [];
  for (const s of steps) { if (s.labels) out.push(...s.labels); else out.push(s.label); }
  return out;
}
export const TEXT_LABELS = new Set(['letter', 'capital', 'space', 'punct', 'markdown', 'enter', 'backspace', 'undo', 'paste']);
// A real keyboard has no "A" key: it has Shift and "a", and Chromium sees TWO keydowns for one
// character. CDP's Input.dispatchKeyEvent does not — it sets the modifier on the one event. So a
// uinput run has its own keydown accounting, and this is the same US-layout shift table that
// tools/uinput-keys.py types from (kb_layout = us on this machine).
const SHIFTED_CHARS = new Set('!@#$%^&*()_+{}:"~|<>?'.split(''));
export const needsShift = (ch) => /[A-Z]/.test(ch) || SHIFTED_CHARS.has(ch);
export const pressChar = (st) => (st.press === 'Space' ? ' ' : st.press === 'Enter' ? '\n' : st.press === 'Backspace' ? '\b' : st.press);

// ---------- the fifteen regimes ----------
// `pace` is the wait between keystrokes: 90 ms is about 133 wpm, and three regimes fix their own
// pace because that is the thing they measure.
export const DEFAULT_PACE = 90;
// Keys measured per regime. 300 at 90 ms is 27 seconds of typing, which is enough samples for a
// p99 with an interval and short enough that fifteen regimes are one sitting.
export const DEFAULT_KEYS = 300;
// Keys typed into a freshly loaded page before the trace starts: the first keystrokes pay for lazy
// compilation and first touch of the editing machinery, and no writer types only 300 keys.
export const WARMUP_KEYS = 25;
// One refresh interval on the 60 Hz output the bench measures on. GDK will not begin a frame until
// this long after the last presentation, so a frame presented inside this window of a keystroke
// takes the slot that keystroke's own frame needed.
export const REFRESH_MS = 1000 / 60;
// How long a paced regime's pause is, and the constraint that picks the number: a pause must not
// end within one `REFRESH_MS` *after* any timer the app arms from the last keystroke, because the
// key that ends the pause then measures the chrome's return rather than typing. Only *after*
// matters: a timer that fires while the burst is already typing paints between two keys 90 ms
// apart, and one that fires after the pause ends has already lost the slot to the key.
// The keystroke-armed timers are `Typing::STATS_MS` and `Typing::TITLE_MS`
// (`quill/src/chrome/typing.rs`) and `AUTOSAVE` and the Preview's `REFRESH` (`quill/src/window.rs`);
// `tools/bench-selftest.mjs` reads all four out of the app and holds this value to them, so the
// margins live there rather than in a table copied over here. The app's periodic timers — the
// harness's drain, the status line's tick — are nobody's to clear: they repeat from the window's
// creation rather than from a keystroke, so their frames can land beside any key at any pause. 1,700 ms clears the nearest of them, `TITLE_MS`, by 300 ms and
// `AUTOSAVE` by 700, and is still a writer's think-pause. 1,400 was the old value: it *was*
// `TITLE_MS`, so eleven keys of three hundred read 13.5–16.9 ms against a 16 ms budget and the
// regime passed or failed on which side of a refresh the collision landed, on an unchanged build
// (#349, from #327 round 3). Any new value is one that selftest stays green on.
export const PAUSE_MS = 1700;
export function regimes(pace = DEFAULT_PACE) {
  return [
    // Plain writing at the end of a draft — the most common case there is, and the one quoted.
    { name: 'prose_end_of_draft',    mix: 'prose',    where: 'end',    pace, focus: 'off' },
    { name: 'prose_middle_of_draft', mix: 'prose',    where: 'middle', pace, focus: 'off' },
    { name: 'prose_focus_sentence',  mix: 'prose',    where: 'middle', pace, focus: 'sentence' },
    { name: 'paragraph_breaks',      mix: 'newlines', where: 'middle', pace, focus: 'off' },
    { name: 'revision',              mix: 'revision', where: 'middle', pace, focus: 'off' },
    { name: 'markdown_syntax',       mix: 'markdown', where: 'middle', pace, focus: 'off' },
    { name: 'fence_flip',            mix: 'fences',   where: 'middle', pace, focus: 'off' },
    { name: 'paste_blocks',          mix: 'paste',    where: 'end',    pace, focus: 'off' },
    { name: 'letters_only_r1',       mix: 'letters',  where: 'middle', pace, focus: 'off' },
    { name: 'bursts_and_pauses',     mix: 'prose',    where: 'end',    pace, focus: 'off', pauseEvery: 25, pauseMs: PAUSE_MS },
    { name: 'fast_typist',           mix: 'prose',    where: 'middle', pace: 45,   focus: 'off' },
    // Unpaced: two keys land in every 16.7 ms frame and queue behind each other, so a per-keystroke
    // uinput → presented figure grows by construction and can never clear a 5 ms mean. Run and
    // recorded like the rest, never scored — the oracle scored the eleven paced regimes and kept
    // this one out of its table (progress/latency-report.md, #41).
    { name: 'saturation_stress',     mix: 'prose',    where: 'end',    pace: 0,    focus: 'off', scored: false },
    // The headline regime's own typing with Live on, so the two lines are read against each other:
    // the fold runs on the keystroke lane beside the Markup Annotator, over the block the caret is
    // in and the one it left, and this is what says whether it stays inside the budget (#273). Only
    // the native bench reads `live`; the Parity oracle has no Live and types this as more prose.
    { name: 'live_end_of_draft',     mix: 'prose',    where: 'end',    pace, focus: 'off', live: true },
    // The headline regime's own typing with the Preview open in Split, so the two lines are read
    // against each other the way Live's are: an edit re-arms the 200 ms refresh timer and drives
    // the rendered page by the caret rule, and this is what says the pane costs the keystroke
    // nothing (#270). Only the native bench reads `preview`; the Parity oracle has no rendered page
    // and types this as more prose.
    { name: 'preview',               mix: 'prose',    where: 'end',    pace, focus: 'off', preview: 'split' },
    // Worker results land between keys, under the headline budget. Only the native bench reads
    // `syntax`; the Parity oracle has no Syntax highlight and types this as more prose.
    { name: 'syntax',                mix: 'prose',    where: 'end',    pace, focus: 'off', syntax: 'on' },
  ];
}

/// Whether a regime, by name, is held to the budget. Every regime is unless its definition says
/// `scored: false`; a name the fifteen do not include is scored, so a misspelling cannot exempt a run.
export const scoredRegime = (name) => !regimes().some((r) => r.name === name && r.scored === false);

// ---------- one regime, written out ----------
// The definition a bench acts on, then every step it will type. Two benches producing the same
// text here are typing the same thing; that is the whole check.
export function formatPlan(r, keys) {
  const steps = script(r.mix, keys, hash32(r.name));
  const out = [];
  out.push(`regime  ${r.name}`);
  out.push(`  mix          ${r.mix}`);
  out.push(`  caret        ${r.where === 'end' ? 'end of the document' : 'middle of the document, at the first line break past half way'}`);
  out.push(`  pace         ${r.pace === 0 ? 'no wait between keystrokes (as fast as the driver types)' : r.pace + ' ms between keystrokes'}`);
  out.push(`  focus        ${r.focus || 'off'}`);
  out.push(`  live         ${r.live ? 'on: the markup rendered in place' : 'off'}`);
  out.push(`  syntax       ${r.syntax || 'off'}`);
  out.push(`  preview      ${r.preview ? `open in ${r.preview}: the rendered page beside the Editor` : 'closed'}`);
  out.push(`  pauses       ${r.pauseEvery ? `every ${r.pauseEvery} keys, ${r.pauseMs || PAUSE_MS} ms` : 'none'}`);
  out.push(`  seed         ${hash32(r.name)}`);
  out.push(`  warm-up      ${WARMUP_KEYS} letter keys, outside the measurement`);
  out.push(`  keys         ${steps.length} steps, ${steps.reduce((a, s) => a + s.keydowns, 0)} keydowns`);
  out.push('');
  for (let i = 0; i < steps.length; i++) {
    const s = steps[i];
    out.push(`${String(i).padStart(6)}  ${s.press.padEnd(16)}${s.label}${s.labels ? '  [' + s.labels.join(' ') + ']' : ''}`);
  }
  return out.join('\n');
}

// ---------- the plan tools/uinput-keys.py reads ----------
// One JSON line, one key press per step, typed through /dev/uinput. A chord is a press like any
// other: `Control+z` and `Shift+ArrowLeft` go over as they are spelled here and the injector holds
// the modifier down inside the same packet, so a chord is one `write(2)`, one stamp and one timed
// key — and the modifier's own keydown reaches the app as a key nobody wrote.
export const UINPUT_PLAN = { hold_ms: 12, settle_ms: 1500, chunk: 25 };
// What the injector's US layout can say. The tables themselves live in `tools/uinput-keys.py`,
// which is the one that has to press them; this is the same shape asked as a question, so a regime
// reaching for a key that has no spelling is caught before a window is ever opened.
const MODIFIERS = new Set(['Shift', 'Control']);
const NAMED = new Set(['Enter', 'Backspace', 'Space', 'Tab',
  'ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown', 'Home', 'End']);
const typeable = (c) => c.length === 1 && /[\x08\x0a\x20-\x7e]/.test(c);
export const expressible = (st) => {
  const c = pressChar(st);
  if (typeable(c)) return true;                 // checked first, so a literal '+' is a key
  const parts = c.split('+');
  return parts.length > 1
    && parts.slice(0, -1).every((m) => MODIFIERS.has(m))
    && (NAMED.has(parts[parts.length - 1]) || typeable(parts[parts.length - 1]));
};
export function uinputPlan(steps, pace, { pauseEvery = 0, pauseMs = 0 } = {}) {
  const at = steps.findIndex((st) => !expressible(st));
  const usable = at < 0 ? steps : steps.slice(0, at);
  const keys = usable.map((st, i) => {
    const key = { press: pressChar(st) };
    // The pause replaces the pace after every `pauseEvery`-th key rather than being added to it,
    // which is what the bench's CDP path does: a burst is 25 keys and then the writer thinking,
    // and the key after the thinking is the one `bursts_and_pauses` exists to time.
    if (pauseEvery && (i + 1) % pauseEvery === 0) key.pause_ms = pauseMs || PAUSE_MS;
    return key;
  });
  return {
    // A real keyboard cannot type with no gap at all, so the saturation regime's pace of 0 becomes
    // the injector's floor of 8 ms — the same substitution the legacy bench makes.
    plan: { pace_ms: pace || 8, ...UINPUT_PLAN, keys },
    keys: usable.length,
    first_unexpressible: at < 0 ? null : { at, press: steps[at].press, label: steps[at].label },
  };
}

// ---------- run directly ----------
// The plan line goes to stdout alone, so it can be piped straight into the injector; everything
// said about it goes to stderr.
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const flags = {}, rest = [];
  for (let i = 2; i < process.argv.length; i++) {
    const a = process.argv[i];
    if (!a.startsWith('--')) { rest.push(a); continue; }
    const k = a.slice(2), v = process.argv[i + 1];
    if (v === undefined || v.startsWith('--')) flags[k] = true; else { flags[k] = v; i++; }
  }
  const all = regimes();
  if (!rest.length) {
    for (const r of all) {
      console.log(`${r.name.padEnd(24)}${r.mix.padEnd(10)}caret ${r.where.padEnd(8)}${String(r.pace).padStart(3)} ms`
        + `${r.focus && r.focus !== 'off' ? '  focus ' + r.focus : ''}${r.pauseEvery ? `  pause ${r.pauseMs} ms every ${r.pauseEvery} keys` : ''}`);
    }
  } else {
    const r = all.find((x) => x.name === rest[0]);
    if (!r) { console.error(`unknown regime ${rest[0]} (have: ${all.map((x) => x.name).join(', ')})`); process.exit(2); }
    const keys = +(flags.keys === undefined || flags.keys === true ? DEFAULT_KEYS : flags.keys);
    if (flags.uinput) {
      const steps = script(r.mix, keys, hash32(r.name));
      const { plan, keys: n, first_unexpressible: bad } = uinputPlan(steps, r.pace);
      console.error(`regime ${r.name}: ${steps.length} steps, ${n} of them a key tools/uinput-keys.py can express today.`);
      if (bad) console.error(`first it cannot: step ${bad.at}, ${bad.press} (${bad.label}) — a chord is not a character.`);
      console.log(JSON.stringify(plan));
    } else {
      console.log(formatPlan(r, keys));
    }
  }
}
