// Does `tools/design-rows` list docs/design.md's rows? — the lister's own test.
//
//   node tools/design-rows-selftest.mjs
//
// The tool exists so a ticket finds a row by name without grepping the table
// (docs/agents/context.md § What the steps cost when skipped), so what it is
// held to is the file it reads: the rows are counted and named here by a second
// reading of docs/design.md, written independently of the tool's own, and the
// two must agree. Nothing here is a copied literal (CODING_STANDARDS.md
// § Tools and flags), so a row added to design.md needs no edit in this file.
// A pure function of one file and one command; no window, no browser.

import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const TOOL = path.join(ROOT, 'tools/design-rows');

let cases = 0;
let failures = 0;
function ok(name, body) {
  cases++;
  try {
    body();
    console.log(`design-rows selftest: ${name} ok`);
  } catch (e) {
    failures++;
    console.log(`design-rows selftest: ${name} FAILED`);
    console.log(String(e.message).split('\n').map((l) => `    ${l}`).join('\n'));
  }
}

const out = execFileSync(TOOL, { encoding: 'utf8' });
const printed = out.split('\n').filter((l) => /^\d+: /.test(l));
const ending = out.trimEnd().split('\n').at(-1);

// The rows as this file reads them: a pipe line that is neither the header (the
// line above a `|---|` delimiter) nor a delimiter, in a table of design.md.
const lines = fs.readFileSync(path.join(ROOT, 'docs/design.md'), 'utf8').split('\n');
const derived = lines.flatMap((line, i) => {
  const isRow = line.startsWith('|') && !/^\|[\s|:-]+\|\s*$/.test(line);
  const isHeader = isRow && /^\|[\s|:-]+\|\s*$/.test(lines[i + 1] ?? '');
  return isRow && !isHeader ? [{ name: line.split('|')[1].trim(), line: i + 1 }] : [];
});

ok('the ending line counts the rows design.md holds', () => {
  assert.ok(derived.length > 0, 'design.md has no table rows to list');
  assert.equal(ending, `design-rows: ${derived.length} rows`);
});

ok('every row design.md holds is printed, at its own line number', () => {
  assert.deepEqual(printed, derived.map((r) => `${r.line}: ${r.name}`));
});

ok("the table's own first row is in the output, and its header is not", () => {
  // Both names come out of the file: the two lines either side of the first
  // delimiter are the header and the first row, whatever they say today.
  const delimiter = lines.findIndex((l) => /^\|[\s|:-]+\|\s*$/.test(l));
  assert.notEqual(delimiter, -1, 'design.md has no table');
  const header = lines[delimiter - 1].split('|')[1].trim();
  const first = lines[delimiter + 1].split('|')[1].trim();
  assert.ok(printed.some((l) => l.endsWith(`: ${first}`)), `the row \`${first}\` is not in the output`);
  assert.ok(!printed.some((l) => l.endsWith(`: ${header}`)), `the header \`${header}\` is listed as a row`);
});

console.log(`design-rows selftest: ${cases - failures} of ${cases} ok`);
process.exit(failures === 0 ? 0 : 1);
