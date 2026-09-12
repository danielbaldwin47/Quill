#!/usr/bin/env node
// Does the edited-files guard still refuse a rewrite of an opened file? — the hook's own test.
//
//   node tools/edited-files-guard-selftest.mjs
//
// .claude/hooks/edited-files-guard.sh is a pure function of one JSON object on stdin and the
// transcript that object points at, so it is checked here without a session: a scratch directory
// standing in for the session's cwd, three files in it, and a transcript JSONL naming one of them
// as opened with Read and Edit — the same shape Claude Code writes, and the same shape the hook
// reads. Each case is one tool call as the harness would hand it over, asserted on the hook's exit
// code and on the decision it prints (nothing printed is an allow). `tools/gate check` runs this.

import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const HOOK = path.join(ROOT, '.claude/hooks/edited-files-guard.sh');

// The hook passes everything it cannot parse, and with no jq it can parse nothing: every case
// below would allow, and would prove nothing.
if (spawnSync('jq', ['--version'], { encoding: 'utf8' }).status !== 0) {
  console.log('edited-files-guard selftest: skipped (no jq)');
  process.exit(0);
}

let cases = 0;
let failures = 0;
function ok(name, body) {
  cases++;
  try {
    body();
    console.log(`edited-files-guard selftest: ${name} ok`);
  } catch (e) {
    failures++;
    console.log(`edited-files-guard selftest: ${name} FAILED`);
    console.log(String(e.message).split('\n').map((l) => `    ${l}`).join('\n'));
  }
}

// A session's cwd: a file it opened, a file it never touched, and a script to run from its path.
const cwd = fs.mkdtempSync(path.join(os.tmpdir(), 'edited-files-guard-'));
fs.mkdirSync(path.join(cwd, 'src'));
const OPENED = path.join(cwd, 'src/opened.rs');
const UNSEEN = path.join(cwd, 'src/unseen.rs');
const SCRIPT = path.join(cwd, 'rewrite.py');
fs.writeFileSync(OPENED, 'fn opened() {}\n');
fs.writeFileSync(UNSEEN, 'fn unseen() {}\n');
fs.writeFileSync(SCRIPT, 'print("a script, not a command")\n');

// The transcript, as Claude Code writes it: one JSON object per line, with the file tools' paths
// inside an assistant message's tool_use blocks. Only src/opened.rs was opened.
const transcript = path.join(cwd, 'transcript.jsonl');
const turn = (name, file) => JSON.stringify({
  type: 'assistant',
  message: { role: 'assistant', content: [{ type: 'tool_use', name, input: { file_path: file } }] },
});
fs.writeFileSync(transcript, [
  JSON.stringify({ type: 'user', message: { role: 'user', content: 'rewrite it' } }),
  turn('Read', OPENED),
  turn('Edit', OPENED),
  turn('Bash', UNSEEN), // not a file tool: a Bash call naming a path opens nothing
  '',
].join('\n'));

// One tool call through the hook: its exit code, and its decision — `allow` when it printed
// nothing — with whatever reason it gave.
function guard(command) {
  const input = JSON.stringify({
    session_id: 'selftest',
    transcript_path: transcript,
    cwd,
    hook_event_name: 'PreToolUse',
    tool_name: 'Bash',
    tool_input: { command },
  });
  const run = spawnSync('bash', [HOOK], { input, encoding: 'utf8' });
  assert.equal(run.status, 0, `the hook exits 0 whatever it decides: ${run.status}\n${run.stderr}`);
  const out = run.stdout.trim();
  if (!out) return { decision: 'allow', reason: '' };
  const { hookSpecificOutput: o } = JSON.parse(out);
  assert.equal(o.hookEventName, 'PreToolUse');
  return { decision: o.permissionDecision, reason: o.permissionDecisionReason };
}

const refuses = (command) => {
  const { decision, reason } = guard(command);
  assert.equal(decision, 'deny', `expected a refusal of: ${command}`);
  assert.match(reason, /src\/opened\.rs/, `the refusal names the file: ${reason}`);
  return reason;
};
const passes = (command) => {
  const { decision, reason } = guard(command);
  assert.equal(decision, 'allow', `expected a pass of: ${command}\ngot: ${reason}`);
};

// The shape the hook has always refused, still refused.
ok('an in-place sed on an opened file', () => {
  refuses(`sed -i 's/opened/renamed/' ${OPENED}`);
});

// The 52 rewrites of 2026-09-10: a heredoc program that reads the file, changes the text and
// writes it back. Both the absolute path and the path relative to cwd name it.
ok('a python3 heredoc writing an opened file', () => {
  const reason = refuses(`python3 - <<'PY'\np = "${OPENED}"\ns = open(p).read()\nopen(p, 'w').write(s.replace("opened", "renamed"))\nPY`);
  assert.match(reason, /change it with Edit/i, `the refusal says what to do instead: ${reason}`);
  refuses(`python3 - <<'PY'\np = "src/opened.rs"\nopen(p, "w").write("fn renamed() {}\\n")\nPY`);
  refuses(`python3 -c "open('${OPENED}', 'a').write('// more\\n')"`);
  refuses(`python3 - <<'PY'\nfrom pathlib import Path\nPath("${OPENED}").write_text("fn renamed() {}\\n")\nPY`);
  refuses(`python3 -c "print(open('${OPENED}').read())" > ${OPENED}`);
});

// A program that only reads it is not a rewrite, and the guard is about what comes back as a diff.
ok('a python3 heredoc only reading an opened file', () => {
  passes(`python3 - <<'PY'\ns = open("${OPENED}").read()\nprint(len(s))\nPY`);
  passes(`python3 -c "print(open('${OPENED}').read().count('fn'))"`);
});

// Node says it differently, and perl a third way; the hook reads all three.
ok('a node -e writing an opened file', () => {
  refuses(`node -e "const fs=require('fs'); fs.writeFileSync('${OPENED}', 'fn renamed() {}\\n')"`);
  refuses(`node - <<'JS'\nrequire('fs').writeFile('src/opened.rs', 'x', () => {});\nJS`);
  refuses(`perl -e 'open(F, ">", "${OPENED}") or die; print F "x";'`);
});

// A script named by path is a file, and the hook reads a command's text, not a file's — even when
// the script is the very rewrite it would have refused inline.
ok('a script run from a path', () => {
  passes(`python3 ${SCRIPT}`);
  passes(`python3 ${SCRIPT} ${OPENED}`);
});

// A file this session never opened is none of the hook's business.
ok('a file the session never opened', () => {
  passes(`python3 - <<'PY'\nopen("${UNSEEN}", 'w').write("fn renamed() {}\\n")\nPY`);
  passes(`sed -i 's/unseen/renamed/' ${UNSEEN}`);
});

// No inline program at all, and an interpreter that is only quoted text.
ok('a command with no inline program', () => {
  passes('cargo test --quiet');
  passes(`cat ${OPENED}`);
  passes(`grep -n fn ${OPENED}`);
  passes(`git commit -m "python3 -c \\"open('src/opened.rs','w')\\" is what we stopped doing"`);
});

fs.rmSync(cwd, { recursive: true, force: true });

if (failures === 0) {
  console.log('edited-files-guard selftest: pass');
} else {
  console.log(`edited-files-guard selftest: fail (${failures} of ${cases})`);
  process.exit(1);
}
