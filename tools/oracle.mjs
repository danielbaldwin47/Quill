// The Parity oracle, frozen: the JavaScript app in legacy/ shot at a Piece's judged states.
//
//   tools/gate oracle <piece>            freeze the Piece's judged states, or say it is unchanged
//   tools/gate oracle <piece> --force    shoot them again whatever the fingerprint says
//
// An agent judging a Piece finds its opponent already on disk. The judged states are
// `shots/oracle/states.json` (the `defaults` plus each state's overrides, decided in issue #24);
// the shots land at `shots/oracle/<piece>/<state>.png` and are committed, so a judging session
// never has to run a browser to have something to judge against, and the owner can see that the
// frozen shot is the one this command would still produce.
//
// WHEN IT SHOOTS AGAIN. Beside the shots is `fingerprint.json`: what produced them — legacy/app's
// js, css and html hashed the way `legacy/tools/latency.mjs` fingerprints the same build, plus
// `legacy/tools/shoot.mjs` itself, plus the passages, plus the resolved flags of every state. A
// re-run with the same fingerprint shoots nothing and says so; anything different re-shoots and
// says which of the four moved. The last two are there because a shot taken at 1440x900 of the
// passage as it read last month is no longer the judged state once the state says 960 or the
// passage gains a line, and neither of those is visible in the app.
//
// A PIECE IS FROZEN WHOLE OR NOT AT ALL. `files`' `library`, `sidebar` and `search` are flags no
// tool under legacy/ serves yet; they wait for the File handling spec, as states.json says. A
// Piece with such a state is reported by name and fails — that Piece only — before a browser is
// launched, so a half-frozen opponent never sits on disk waiting to be judged as if it were whole.
// The rule needs no list kept here: a state may name only flags the defaults name.
//
// Offsets in states.json are UTF-8 bytes from the start of the passage, which is the form the
// native app's --caret and --select take. shoot.mjs counts characters, so they are converted here
// against the state's own passage.
import { execFileSync, spawn } from 'node:child_process';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

// The one hash of a legacy/ build, shared with the bench that stamps its numbers with it, and the
// two smaller stamps that live beside it.
import { appFiles, gitHead, hashApp, sha256 } from './fingerprint.mjs';

// The port legacy/bin/quill opens the app on, and the same way of moving it.
const PORT = +(process.env.QUILL_PORT || 4173);

// ---------- the judged states ----------
export function readStates(root) {
  return JSON.parse(fs.readFileSync(path.join(root, 'shots/oracle/states.json'), 'utf8'));
}

// The Piece's states, each one the defaults with its own overrides on top, in the order the file
// lists them — which is the order they are shot and reported in.
export function resolveStates(states, piece) {
  const pieces = states.pieces || {};
  if (!Object.prototype.hasOwnProperty.call(pieces, piece)) {
    throw new Error(`${piece}: no Piece by that name has judged states (shots/oracle/states.json names ${Object.keys(pieces).join(', ')})`);
  }
  return Object.entries(pieces[piece]).map(([name, overrides]) => ({ name, flags: { ...states.defaults, ...overrides } }));
}

// The flags in this state that this tool cannot serve: the ones the defaults do not name. The
// defaults are the state vocabulary — a state that reaches past them is asking for a flag the
// harness has not learnt yet.
export function unservable(defaults, flags) {
  return Object.keys(flags).filter((k) => !Object.prototype.hasOwnProperty.call(defaults, k)).sort();
}

// ---------- offsets ----------
// A UTF-8 byte offset into `text` as the character offset shoot.mjs wants. An offset that lands
// inside a character is a bug in the state, not something to round.
export function byteToChar(text, byte) {
  const bytes = Buffer.from(text, 'utf8');
  if (byte > bytes.length) throw new Error(`offset ${byte} is past the end of the passage (${bytes.length} bytes)`);
  const head = bytes.subarray(0, byte);
  const back = Buffer.from(head.toString('utf8'), 'utf8');
  if (!back.equals(head)) throw new Error(`offset ${byte} is not on a character boundary of the passage`);
  return head.toString('utf8').length;
}

// ---------- the command line a state is shot with ----------
export function shootArgv(root, flags, out, url) {
  const argv = ['--out', out, '--url', url, '--w', String(flags.w), '--h', String(flags.h), '--dpr', String(flags.scale)];
  argv.push('--theme', flags.theme, '--font', flags.font, '--size', String(flags.size));
  argv.push('--focus', flags.focus, '--chrome', flags.chrome, '--active', flags.active ? 'on' : 'off');
  if (flags.typewriter) argv.push('--typewriter');
  if (flags.nocaret) argv.push('--nocaret');
  if (flags.typing) argv.push('--typing');
  if (flags.menu) argv.push('--menu', flags.menu);
  if (flags.text) {
    const passage = fs.readFileSync(path.join(root, flags.text), 'utf8');
    argv.push('--text', flags.text);
    if (flags.caret !== null && flags.caret !== undefined) argv.push('--caret', String(byteToChar(passage, flags.caret)));
    if (flags.select) argv.push('--select', flags.select.map((b) => byteToChar(passage, b)).join(','));
  }
  if (flags.scroll) argv.push('--scroll', String(flags.scroll));
  return argv;
}

// ---------- what produced the shots ----------
export function fingerprint(root, resolved) {
  const git = gitHead(root);
  return {
    _about: 'What produced the shots beside this file. `tools/gate oracle <piece>` re-shoots when any of it moves; git_head only says where it was taken and is not compared.',
    app: hashApp(root),
    shoot: sha256(fs.readFileSync(path.join(root, 'legacy/tools/shoot.mjs'))).slice(0, 16),
    git_head: git,
    passages: Object.fromEntries([...new Set(resolved.map((s) => s.flags.text).filter(Boolean))].sort()
      .map((p) => [p, sha256(fs.readFileSync(path.join(root, p))).slice(0, 16)])),
    states: Object.fromEntries(resolved.map((s) => [s.name, s.flags])),
  };
}

// Why these shots are not the shots this run would take — or null if they are. `have` is the
// states whose png is actually on disk.
export function freezeReason(was, now, have) {
  if (!was) return 'nothing frozen here yet';
  if (was.app?.sha256 !== now.app.sha256 || was.app?.files !== now.app.files) return 'legacy/app changed';
  if (was.shoot !== now.shoot) return 'legacy/tools/shoot.mjs changed';
  if (JSON.stringify(was.passages) !== JSON.stringify(now.passages)) return 'the passage changed';
  if (JSON.stringify(was.states) !== JSON.stringify(now.states)) return 'the judged states changed';
  const missing = Object.keys(now.states).filter((s) => !have.includes(s));
  if (missing.length) return `a shot is missing (${missing.join(', ')})`;
  return null;
}

// ---------- the document server ----------
// Something answering on the port is not proof that it is Quill, as legacy/bin/quill says — and
// for the oracle, being Quill is not proof either. Sibling worktrees run their own servers, and a
// shot taken from one of those would sit under a fingerprint naming this checkout's app. So the
// app on the port is hashed exactly as the fingerprint hashes it and must be the same app, file
// for file; anything else is a port to step past, not a server to reuse.
async function servesThisApp(port, root, want) {
  try {
    const h = crypto.createHash('sha256');
    for (const f of appFiles(root)) {
      const r = await fetch(`http://localhost:${port}/${f.replace(/^legacy\/app\//, '')}`, { signal: AbortSignal.timeout(1000) });
      if (!r.ok) return false;
      h.update(`${f}:${sha256(Buffer.from(await r.arrayBuffer()))}\n`);
    }
    return h.digest('hex').slice(0, 16) === want;
  } catch { return false; }
}

async function portBusy(port) {
  try { await fetch(`http://localhost:${port}/`, { signal: AbortSignal.timeout(1000) }); return true; } catch { return false; }
}

async function documentServer(root, appSha) {
  let port = PORT;
  if (await servesThisApp(port, root, appSha)) return { url: `http://localhost:${port}/`, stop() {} };
  while (await portBusy(port)) {
    process.stderr.write(`gate oracle: port ${port} is in use by something that is not this checkout's app, trying ${port + 1}\n`);
    port++;
  }
  // Losing the race for the port is the case worth catching: between the scan above and the bind
  // below, somebody else's server can take it, and then the port answers, this child is dead, and
  // the shots would come from a stranger. So the child is watched, and what finally answers is
  // put through the same check as a server that was already there.
  const child = spawn('node', [path.join(root, 'legacy/tools/serve.mjs'), String(port)], { cwd: root, stdio: 'ignore' });
  let died = null;
  child.on('error', (e) => { died = e.message; });
  child.on('exit', (code, signal) => { died = `it exited (${signal || `code ${code}`})`; });
  for (let i = 0; i < 100 && died === null; i++) {
    if (await portBusy(port)) {
      if (await servesThisApp(port, root, appSha)) return { url: `http://localhost:${port}/`, stop() { child.kill(); } };
      break;
    }
    await new Promise((r) => setTimeout(r, 50));
  }
  child.kill();
  throw new Error(`could not start the document server on port ${port}${died ? `: ${died}` : ''}`);
}

// ---------- the command ----------
// What the command is stays in tools/gate's own usage, which calls itself the one copy of that;
// this says the shape and where the description lives.
function usage(where = process.stderr) {
  where.write(`usage: tools/gate oracle <piece> [--force]

  The Pieces with judged states are the keys of "pieces" in
  shots/oracle/states.json. What the command does: tools/gate --help
`);
}

// Everything that can go wrong once a Piece has been named ends in the one line tools/gate
// promises for every command it has: a stack trace where that line should be is the command
// breaking that promise, whatever went wrong underneath it.
async function main(argv) {
  const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
  process.chdir(root);                       // states.json's paths are the repo's, and so is shoot.mjs's --text

  let piece = null;
  let force = false;
  for (const a of argv) {
    if (a === '--force') force = true;
    else if (a === '-h' || a === '--help') { usage(process.stdout); return 0; }
    else if (a.startsWith('-')) { process.stderr.write(`gate oracle: ${a}: not a flag this command has\n`); usage(); return 2; }
    else if (piece === null) piece = a;
    else { process.stderr.write('gate oracle: one Piece at a time\n'); usage(); return 2; }
  }
  if (piece === null) { usage(); return 2; }

  try { return await freeze(root, piece, force); }
  catch (e) {
    process.stderr.write(`gate oracle ${piece}: ${e.message}\n`);
    console.log(`gate oracle ${piece}: fail (the Piece could not be frozen)`);
    return 1;
  }
}

async function freeze(root, piece, force) {
  // A Piece nobody has judged states for is a mistyped command, not a failed freeze, so it answers
  // as tools/gate answers a command it does not have: the name, the usage, and 2. A states.json
  // that will not parse is the other thing entirely — that is the freeze failing, and it ends in
  // the line the owner reads, through the catch in main().
  const states = readStates(root);
  let resolved;
  try { resolved = resolveStates(states, piece); }
  catch (e) { process.stderr.write(`gate oracle: ${e.message}\n`); return 2; }

  if (resolved.length === 0) {
    console.log(`gate oracle ${piece}: no judged states (${piece === 'latency' ? 'the latency Piece is benched, not judged' : 'nothing to freeze'})`);
    return 0;
  }

  // Every state is read before any is shot, so a Piece that cannot be frozen whole is not
  // half-frozen: the flags no tool serves yet, then the offsets, then the passages.
  const blocked = resolved.map((s) => ({ ...s, cannot: unservable(states.defaults, s.flags) })).filter((s) => s.cannot.length);
  if (blocked.length) {
    for (const s of blocked) {
      // A flag whose value is a path names a fixture nobody has built either; say so, since that
      // is the other half of why the state cannot be shot.
      const fixtures = s.cannot.map((f) => s.flags[f]).filter((v) => typeof v === 'string' && v.includes('/'));
      process.stderr.write(`gate oracle ${piece}: state ${s.name} names ${s.cannot.join(', ')}${fixtures.length ? `, and the fixture ${fixtures.join(', ')}` : ''}\n`);
    }
    process.stderr.write('gate oracle: no tool under legacy/ serves those yet; they wait for the File handling spec (shots/oracle/states.json)\n');
    console.log(`gate oracle ${piece}: fail (${blocked.length} of ${resolved.length} states name flags this tool cannot serve yet)`);
    return 1;
  }

  // The other half of freezing whole: every state's passage is read and every offset converted
  // now, so a caret that lands inside a character is found here rather than after three of the
  // Piece's four shots are already on disk.
  const dir = path.join(root, 'shots/oracle', piece);
  const shot = (s) => path.join('shots/oracle', piece, `${s.name}.png`);
  try { for (const s of resolved) shootArgv(root, s.flags, shot(s), 'http://localhost/'); }
  catch (e) {
    process.stderr.write(`gate oracle ${piece}: ${e.message}\n`);
    console.log(`gate oracle ${piece}: fail (a judged state could not be read)`);
    return 1;
  }

  // legacy/ has its own manifest, and the root `npm i` does not fill it. Said here, where it is
  // one line, rather than as shoot.mjs's module-not-found under a failed shot.
  if (!fs.existsSync(path.join(root, 'legacy/node_modules/playwright-core'))) {
    process.stderr.write('gate oracle: legacy/ has no playwright-core to shoot with; run `npm i` inside legacy/ (its manifest is its own, and the root one is not enough)\n');
    console.log(`gate oracle ${piece}: fail (legacy/ is not installed)`);
    return 1;
  }

  const now = fingerprint(root, resolved);
  const fpFile = path.join(dir, 'fingerprint.json');
  const was = fs.existsSync(fpFile) ? JSON.parse(fs.readFileSync(fpFile, 'utf8')) : null;
  const have = resolved.filter((s) => fs.existsSync(path.join(dir, `${s.name}.png`))).map((s) => s.name);
  const why = freezeReason(was, now, have);
  if (why === null && !force) {
    console.log(`gate oracle ${piece}: unchanged (${resolved.length} states already frozen)`);
    return 0;
  }

  const server = await documentServer(root, now.app.sha256);
  try {
    fs.mkdirSync(dir, { recursive: true });
    // The fingerprint goes before the shots do, not after: a run that dies half way through
    // leaves a directory of new shots and old ones, and a fingerprint still sitting there would
    // call that mixture unchanged. Without one, the next run says nothing is frozen and shoots.
    fs.rmSync(fpFile, { force: true });
    for (const s of resolved) {
      process.stderr.write(`gate oracle ${piece}: shooting ${s.name}\n`);
      // shoot.mjs's own "wrote ..." line would drown the one line the owner reads; what it says
      // when it fails is on stderr, above that line, for the agent who has to fix it.
      execFileSync('node', [path.join(root, 'legacy/tools/shoot.mjs'), ...shootArgv(root, s.flags, shot(s), server.url)], { cwd: root, stdio: ['ignore', 'ignore', 'inherit'] });
    }
    // A state that has been renamed or dropped leaves its shot behind, and a judging session would
    // pick up an opponent no judged state asks for any more. Only a shot this command took itself
    // — one the last fingerprint names — is cleared up; anything else in the directory is somebody
    // else's and is left where it is.
    for (const name of Object.keys(was?.states || {})) {
      if (!resolved.some((s) => s.name === name) && fs.existsSync(path.join(dir, `${name}.png`))) {
        process.stderr.write(`gate oracle ${piece}: ${name} is no longer a judged state, removing its shot\n`);
        fs.rmSync(path.join(dir, `${name}.png`));
      }
    }
    fs.writeFileSync(fpFile, `${JSON.stringify(now, null, 2)}\n`);
  } catch (e) {
    process.stderr.write(`gate oracle ${piece}: ${e.message}\n`);
    console.log(`gate oracle ${piece}: fail (a state could not be shot)`);
    return 1;
  } finally {
    server.stop();
  }
  console.log(`gate oracle ${piece}: froze ${resolved.length} states (${force && why === null ? '--force' : why})`);
  return 0;
}

// The exit code is set rather than taken: process.exit() can cut the last line short on its way
// down a pipe, and that line is the whole point of the command.
if (import.meta.url === pathToFileURL(process.argv[1] || '').href) {
  process.exitCode = await main(process.argv.slice(2));
}
