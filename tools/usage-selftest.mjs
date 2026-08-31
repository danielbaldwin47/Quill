// Is `tools/gate --help` still true? — the help's own test.
//
//   node tools/usage-selftest.mjs
//
// CLAUDE.md § Repo map promises that the help names every subcommand and what
// each ends in, and five review passes (0cd10a1, a2db4c0, c703b17, e439bb6,
// 90f8b8d) found it behind. So it is read here, on every commit, against the
// two things it describes: the `case` arm in tools/gate that dispatches each
// subcommand, and the `gate <command> ${...}: <ending>` lines the subcommand's
// script prints. docs/agents/gate.md is held to naming each subcommand too.
// A pure function of three files; no window, no browser.

import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const GATE = path.join(ROOT, 'tools/gate');

let cases = 0;
let failures = 0;
function ok(name, body) {
  cases++;
  try {
    body();
    console.log(`usage selftest: ${name} ok`);
  } catch (e) {
    failures++;
    console.log(`usage selftest: ${name} FAILED`);
    console.log(String(e.message).split('\n').map((l) => `    ${l}`).join('\n'));
  }
}

const help = execFileSync(GATE, ['--help'], { encoding: 'utf8' });
const script = fs.readFileSync(GATE, 'utf8');
const gateMd = fs.readFileSync(path.join(ROOT, 'docs/agents/gate.md'), 'utf8');

// The subcommands are the arms of the last `case`: `  name) shift; ...`.
const commands = [...script.matchAll(/^\s+([a-z]+)\) shift;/gm)].map((m) => m[1]);

// The help's paragraph for one command: from its own line to the next
// command's, whitespace collapsed so a wrapped ending reads as one.
function paragraph(command) {
  const lines = help.split('\n');
  const start = lines.findIndex((l) => l.startsWith(`  ${command} `) || l === `  ${command}`);
  assert.notEqual(start, -1, `the help has no entry for \`${command}\``);
  let end = lines.findIndex((l, i) => i > start && /^  [a-z]+( |$)/.test(l));
  if (end === -1) end = lines.length;
  return lines.slice(start, end).join(' ').replace(/\s+/g, ' ');
}

// What a subcommand's script prints as its last line: the word after the colon
// in a `console.log(\`gate <command> ${piece}: <word>` — stdout, where the
// endings go; the `say` lines on the way there are stderr and are not read. A
// template there (judge's `${winner}`) is the help's `ours|theirs` and is not
// a word to look for, and `this Piece was won in round N` is the line above a
// verdict rather than one.
function endings(command) {
  const source = fs.readFileSync(path.join(ROOT, `tools/${command}.mjs`), 'utf8');
  const words = new Set();
  for (const m of source.matchAll(/console\.log\(`gate [a-z]+(?: \$\{[a-z]+\}| [a-z]+)?: ([a-z]+)/g)) {
    if (m[1] !== 'this') words.add(m[1]);
  }
  return [...words].sort();
}

ok('the case arm dispatches at least the five subcommands the repo map names', () => {
  for (const c of ['check', 'oracle', 'bench', 'judge', 'keys']) assert.ok(commands.includes(c), c);
});

ok('every subcommand dispatched has its paragraph in the help', () => {
  for (const c of commands) paragraph(c);
});

ok('every subcommand dispatched is named in docs/agents/gate.md', () => {
  for (const c of commands) assert.match(gateMd, new RegExp(`\`tools/gate ${c}\\b`), `gate.md never says \`tools/gate ${c}\``);
});

for (const c of commands.filter((c) => fs.existsSync(path.join(ROOT, `tools/${c}.mjs`)))) {
  ok(`the help names every ending tools/${c}.mjs prints`, () => {
    const p = paragraph(c);
    for (const word of endings(c)) {
      assert.match(p, new RegExp(`(: |\\.\\.\\. )${word}\\b`), `\`gate ${c} <piece>: ${word}\` is printed but the help never says so`);
    }
  });
}

console.log(`usage selftest: ${cases - failures} of ${cases} ok`);
process.exit(failures === 0 ? 0 : 1);
