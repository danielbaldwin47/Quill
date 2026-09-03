// Does `tools/ticket --graph` still print what its docstring says? — the ticket tool's own test.
//
//   node tools/ticket-selftest.mjs
//
// `tools/ticket --graph <M>` needs `gh` and the network; `--from <file>` reads the same graph
// from a file, and tools/ticket-fixture/spec-44.json is #44's three children as
// `tools/ticket --graph 44 --json` captured them on 2026-09-02, every one closed by then. Every
// expectation here is derived from that file — a child's line from its number, state and title,
// the Size and Reading lines from its body, the frontier from the states — and the cases the
// capture lacks (an open child, an open blocker, a body with no Size line, a `## Size` heading
// in place of the line, as #159's children carry) are the same fixture with one field changed, written to a scratch file carrying the pid, never a second literal.

import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const TICKET = path.join(ROOT, 'tools/ticket');
const FIXTURE = path.join(ROOT, 'tools/ticket-fixture/spec-44.json');

let cases = 0;
let failures = 0;
function ok(name, body) {
  cases++;
  try {
    body();
    console.log(`ticket selftest: ${name} ok`);
  } catch (e) {
    failures++;
    console.log(`ticket selftest: ${name} FAILED`);
    console.log(String(e.message).split('\n').map((l) => `    ${l}`).join('\n'));
  }
}

const fixture = JSON.parse(fs.readFileSync(FIXTURE, 'utf8'));

// The tool's stdout over a fixture file, as lines.
function graphLines(file, ...flags) {
  return execFileSync(TICKET, ['--graph', '--from', file, ...flags], { encoding: 'utf8' }).trimEnd().split('\n');
}

// The same, over a graph object: the fixture with a change, through a scratch file.
let scratch = 0;
function graphLinesOf(g, ...flags) {
  const file = path.join(os.tmpdir(), `ticket-selftest-${process.pid}-${scratch++}.json`);
  fs.writeFileSync(file, JSON.stringify(g));
  try {
    return graphLines(file, ...flags);
  } finally {
    fs.unlinkSync(file);
  }
}

const clone = (g) => JSON.parse(JSON.stringify(g));
const HEADER = /^#(\d+) (open|closed)  (.*)$/;

// The output cut at its child lines: one block per child, then the summary line.
function blocks(lines) {
  const out = [];
  for (const line of lines.slice(0, -1)) {
    if (HEADER.test(line)) out.push([line]);
    else out[out.length - 1].push(line);
  }
  return { blocks: out, summary: lines[lines.length - 1] };
}

// The frontier the docstring promises: open children with no open blocker.
const frontierOf = (g) => g.children.filter((c) => c.state === 'open' && !c.blocked_by.some((b) => b.state === 'open')).map((c) => `#${c.number}`);
const summaryOf = (g) => `${g.spec.number}: ${g.children.filter((c) => c.state === 'open').length} open of ${g.children.length}; frontier: ${frontierOf(g).join(', ') || 'none'}`;

const captured = graphLines(FIXTURE);

ok('the fixture is what --json prints for it, so its shape is the one the tool reads and writes', () => {
  assert.deepEqual(JSON.parse(graphLines(FIXTURE, '--json').join('\n')), fixture);
  assert.ok(fixture.children.length >= 3, 'the capture has fewer than three children');
});

ok('one `#<n> <state>  <title>` line per child, in the fixture\'s order', () => {
  const headers = captured.filter((l) => HEADER.test(l)).map((l) => l.match(HEADER));
  assert.deepEqual(headers.map((m) => [Number(m[1]), m[2], m[3]]), fixture.children.map((c) => [c.number, c.state, c.title]));
});

ok('the Size and Reading lines are the fixture bodies\' own, with the bold stripped', () => {
  const { blocks: perChild } = blocks(captured);
  fixture.children.forEach((c, i) => {
    for (const label of ['Size', 'Reading']) {
      const own = c.body.split('\n').filter((l) => l.startsWith(`**${label}**:`));
      assert.equal(own.length, 1, `#${c.number}'s body carries ${own.length} ${label} lines; the capture should carry one`);
      const printed = perChild[i].filter((l) => l.startsWith(`  ${label}:`));
      assert.equal(printed.length, 1, `#${c.number} printed ${printed.length} ${label} lines`);
      assert.equal(`**${label}**:${printed[0].slice(`  ${label}:`.length)}`, own[0]);
    }
  });
});

ok('`blocked by:` names each of a child\'s blockers with its state, and is absent where there are none', () => {
  const { blocks: perChild } = blocks(captured);
  assert.ok(fixture.children.some((c) => c.blocked_by.length > 0), 'no child in the capture has a blocker');
  fixture.children.forEach((c, i) => {
    const printed = perChild[i].filter((l) => l.startsWith('  blocked by:'));
    if (c.blocked_by.length === 0) return assert.deepEqual(printed, []);
    assert.deepEqual(printed, [`  blocked by: ${c.blocked_by.map((b) => `#${b.number} (${b.state})`).join(', ')}`]);
  });
  const unblocked = clone(fixture);
  unblocked.children[0].blocked_by = [];
  assert.deepEqual(blocks(graphLinesOf(unblocked)).blocks[0].filter((l) => l.startsWith('  blocked by:')), []);
});

ok('the last line counts the open children and names exactly the open ones with no open blocker', () => {
  assert.equal(blocks(captured).summary, summaryOf(fixture));
  // The capture is all closed; the fixture reopened three ways covers the frontier's three answers.
  const reopened = clone(fixture);
  for (const c of reopened.children) c.state = 'open';
  const withOpenBlockers = clone(reopened);
  for (const c of withOpenBlockers.children) for (const b of c.blocked_by) b.state = 'open';
  const firstOnly = clone(fixture);
  firstOnly.children[0].state = 'open';
  assert.ok(frontierOf(reopened).length === reopened.children.length, 'the reopened capture should put every child on the frontier');
  assert.ok(frontierOf(withOpenBlockers).length < reopened.children.length, 'reopening the blockers should shorten the frontier');
  assert.deepEqual(frontierOf(firstOnly), [`#${fixture.children[0].number}`]);
  for (const g of [reopened, withOpenBlockers, firstOnly]) assert.equal(blocks(graphLinesOf(g)).summary, summaryOf(g));
});

ok('a child whose body has no Size line prints `Size: —`, and its Reading line as before', () => {
  const sizeless = clone(fixture);
  const child = sizeless.children[0];
  child.body = child.body.split('\n').filter((l) => !l.startsWith('**Size**:')).join('\n');
  const [before, after] = [blocks(captured).blocks[0], blocks(graphLinesOf(sizeless)).blocks[0]];
  assert.deepEqual(after.filter((l) => l.startsWith('  Size:')), ['  Size: —']);
  assert.deepEqual(after.filter((l) => l.startsWith('  Reading:')), before.filter((l) => l.startsWith('  Reading:')));
});

ok('a `## Size` heading over one paragraph reads as the same Size line', () => {
  const headed = clone(fixture);
  const child = headed.children[0];
  child.body = child.body.split('\n').map((l) => (l.startsWith('**Size**:') ? `## Size\n\n${l.slice('**Size**:'.length).trim()}` : l)).join('\n');
  assert.deepEqual(blocks(graphLinesOf(headed)).blocks[0].filter((l) => l.startsWith('  Size:')), blocks(captured).blocks[0].filter((l) => l.startsWith('  Size:')));
});

console.log(`ticket selftest: ${cases - failures} of ${cases} ok`);
process.exit(failures === 0 ? 0 : 1);
