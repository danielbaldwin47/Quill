// The stage a native window is judged on: an output of the Gate's own, the rules that pin Quill to
// it, the launch, the capture, and the teardown that hands the owner's desktop back exactly as it
// was found.
//
//   import { openStage, quillArgv } from './harness.mjs'
//
// `tools/gate judge` shoots on this stage; `tools/gate bench`
// ([#64](https://github.com/danielbaldwin47/Quill/issues/64)) will type on it. Neither owns the
// recipe, because a bench number and a judged shot are only comparable if the window they came
// from was the same window — same output, same scale, same rules, same environment. The recipe
// itself is `docs/research/native-harness.md` on `research/native-harness`, measured rather than
// read; what is here is that document turned into code, and the "why" comments below are the
// measurements that decided each line.
//
// WHY A DEDICATED OUTPUT. The owner's panel is scale 1.5, so a 1440x900 logical window is 2160x1350
// device pixels and GTK lays glyphs out through `wp_fractional_scale_v1`; neither the size nor the
// glyph positions of a judged shot survive that. A created headless output can be told to be
// 3200x2000 at integer scale 2 — 1600x1000 logical, room for the widest judged state plus margin —
// and Hyprland composites and presents it at 60 Hz without a pixel reaching the panel.
//
// WHY `grim -T`. `grim -g` captures a region of the *output*, layers included: on this machine that
// is Omarchy's bar and any notification that happens to fire, and the first region capture taken
// during the research came back with a crash toast painted across the document. `-T` captures the
// toplevel's own buffer through `ext_foreign_toplevel_image_capture_source_manager_v1` — no layer,
// no bar, no cursor, no compositor rounding, shadow or opacity rule. Its identifier is not in
// `hyprctl clients`; it comes from the protocol, and the only way to read it is to make grim
// enumerate the list and parse the trace ([`toplevels`]).
//
// ONE STAGE AT A TIME. The rules live in a table belonging to the compositor rather than to this
// process, which is what lets a teardown reach rules a crashed run left behind — and what means two
// stages open at once would take each other's rules down. A judged shot is a serial thing anyway
// (one window, one keyboard focus, one shutter), so this is a constraint rather than a limitation;
// it is written down because the table's name makes it invisible.
//
// WHY THE OWNER IS RESTORED, NOT MERELY LEFT ALONE. A window can only have keyboard focus if its
// monitor has it, so shooting a focused state means moving focus off the owner's window for as long
// as the shot takes. That is unavoidable; leaving it moved is not. The stage records the focused
// window, the active workspace and the pointer before it does anything, and puts all three back on
// the way out — on exit, on error and on a signal, which is why the teardown is a trap and not the
// last line of a function.
import { execFileSync, spawn } from 'node:child_process';
import crypto from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

// The `GtkApplication` application-id, which is the xdg-toplevel `app_id`, which is what Hyprland
// reports as a window's `class` and what `ext_foreign_toplevel_handle_v1` reports as its `app_id`.
// One string, three names for it; `quill/src/main.rs` holds the original.
export const APP_ID = 'io.github.danielbaldwin47.Quill';

// The stage's own numbers. The mode is the widest judged state (1440 logical) plus margin, at the
// integer scale every judged shot is taken at; the window sits at `MARGIN` from the top left so
// that no edge of it is an edge of the output.
const MODE = '3200x2000@60';
const SCALE = 2;
const MARGIN = { x: 80, y: 50 };

// How long a launch has to put a window on the compositor before the launch is called failed, and
// how often that is checked. Ten seconds is far past a native app's cold start (the research
// measured a *Python* GTK app at 200 ms) and short enough that a binary which never maps a window
// fails while an agent is still watching.
const MAP_TIMEOUT_MS = 10_000;

// How long a created output has to turn up in `hyprctl monitors` before the stage gives up on
// naming it. See where it is used: an output nobody named is the one thing here that can be left
// behind.
const CREATE_TIMEOUT_MS = 1_000;
const POLL_MS = 50;

// What a window is given between mapping and the shutter. GTK maps, then paints, then the
// compositor presents; a capture taken inside that has caught a half-drawn frame. The pair check in
// [`Stage.shoot`] is what actually proves the window has settled — this is only how long is waited
// before asking the question the first time.
const SETTLE_MS = 400;

// How many times a shot is asked for before the window is called unsteady. Each attempt is two
// captures that have to be the same bytes; a window that cannot manage that in five attempts is
// animating something, and the shot would be a coin toss rather than a judged state.
const STEADY_TRIES = 5;

// ---------- the command line a judged state opens ours with ----------

// The native app's arguments for one resolved judged state.
//
// Offsets need no conversion here: `--caret` and `--select` take UTF-8 bytes from the start of the
// Document, which is the form `shots/oracle/states.json` writes them in. (`tools/gate oracle` has
// to convert, because the browser shooter counts characters.)
//
// `scale` and `active` are not here and never will be: the first is the output's, and the second
// is keyboard focus, which is the compositor's to give and not a flag the app could honour.
export function quillArgv(root, flags) {
  const argv = ['--deterministic', '--w', String(flags.w), '--h', String(flags.h)];
  argv.push('--theme', flags.theme, '--font', flags.font, '--size', String(flags.size));
  argv.push('--focus', flags.focus, '--chrome', flags.chrome);
  if (flags.typewriter) argv.push('--typewriter');
  if (flags.nocaret) argv.push('--nocaret');
  // An empty Document has no passage, and so has no offset into one either.
  if (flags.text) {
    argv.push('--text', path.join(root, flags.text));
    if (flags.caret !== null && flags.caret !== undefined) argv.push('--caret', String(flags.caret));
    if (flags.select) argv.push('--select', flags.select.join(','));
  }
  if (flags.scroll) argv.push('--scroll', String(flags.scroll));
  return argv;
}

// The environment a judged launch is made in.
//
// `GDK_SCALE` is exported globally by Omarchy and would force a 2x buffer the compositor then
// rescales; the output's own scale is the one that should decide. The renderer is pinned because
// `cairo` differs from `gl` by 8,633 pixels on the same window (`gl`, `ngl` and `vulkan` are
// byte-identical to each other). The backend is pinned so an `XDG_SESSION_TYPE` surprise cannot
// put the window on XWayland, where `grim -T` would be capturing a different toplevel entirely.
// Accessibility is off because an at-spi bus that is not there costs a launch two seconds.
//
// No `FONTCONFIG_FILE`. The research's checklist asks for one listing only the repo's fonts, and
// [ADR 0007](../docs/adr/0007-quill-faces-renamed-and-private.md) has since retired exactly that:
// the Faces are added to the running fontconfig with `FcConfigAppFontAddDir` before GTK
// initialises, from `<repo>/fonts` in a development build, so a judged shot is already set in the
// repository's own files. Setting the variable would replace the user's whole fontconfig for the
// process and every child, which is the thing that ADR rejected. What is left — a system font
// drawing a glyph the Faces do not have — is what `--deterministic`'s pinned rendering and the
// judged passages between them keep out.
export function launchEnv(env = process.env) {
  const out = { ...env, GSK_RENDERER: 'gl', GDK_BACKEND: 'wayland', GTK_A11Y: 'none' };
  delete out.GDK_SCALE;
  return out;
}

// ---------- the window rules ----------

// `appId` as the anchored Lua pattern `hl.window_rule`'s `class` match wants.
//
// The dots are the point: `class` is a regex, so an unescaped `io.github...` would also match an
// `ioXgithub...` nobody has — and, less amusingly, the escaping has to survive being written into
// Lua source, where `\\.` is what a single backslash before a dot is spelt.
export function classPattern(appId) {
  return `^${appId.replace(/\./g, '\\\\.')}$`;
}

// The Lua that pins ours to the stage, as the text of a file `hyprctl repl` is told to `dofile`.
//
// Every rule is scoped to the application id, so nothing the owner has open is touched, and every
// handle is kept in one compositor-global table, because a rule outlives the process that added it
// and [`RULES_OFF`] is the only thing that can take it away again.
//
// `size` and `move` are here rather than set once for the stage because the judged states are not
// all one size — `page/narrow` is 960 wide — and a rule applies at the moment a window maps.
//
// The last two lines are Omarchy's, undone: it tags every window `default-opacity` and composites
// it at 98.5%, so a shot taken through the compositor is a shot of a slightly transparent app.
// `grim -T` reads the client's own buffer and so is already past that, but a rule set that only
// works because of how it happens to be captured is a trap for whoever changes the capture.
//
// `no_initial_focus` is added only for a state shot without keyboard focus: it stops ours taking
// focus as it maps, so the parking window put there first keeps it.
export function rulesLua(appId, { workspace, w, h, x = MARGIN.x, y = MARGIN.y, initialFocus = true }) {
  const lines = [
    'QUILL_GATE_RULES = QUILL_GATE_RULES or {}',
    `local C = "${classPattern(appId)}"`,
    'local function rule(t) t.match = { class = C } QUILL_GATE_RULES[#QUILL_GATE_RULES + 1] = hl.window_rule(t) end',
    `rule({ name = "quill-gate-workspace", workspace = "${workspace} silent" })`,
    'rule({ name = "quill-gate-float", float = true })',
    `rule({ name = "quill-gate-size", size = "${w} ${h}" })`,
    `rule({ name = "quill-gate-move", move = "${x} ${y}" })`,
    'rule({ name = "quill-gate-decoration", no_anim = true, border_size = 0, rounding = 0,'
      + ' no_shadow = true, no_blur = true, no_dim = true, opacity = "1.0 1.0", tag = "-default-opacity" })',
  ];
  if (!initialFocus) lines.push('rule({ name = "quill-gate-no-initial-focus", no_initial_focus = true })');
  return `${lines.join('\n')}\n`;
}

// The Lua that takes every rule this harness added back off again, and says how many it took.
//
// Safe to run when there are none, which is what makes it safe to run from a signal handler that
// does not know how far the stage got.
export const RULES_OFF =
  'local t = QUILL_GATE_RULES or {} local n = #t'
  + ' for _, r in ipairs(t) do r:set_enabled(false) end'
  + ' QUILL_GATE_RULES = {} return "off " .. n';

// ---------- the toplevel list ----------

// The toplevels the compositor is showing, parsed out of a `WAYLAND_DEBUG=1` trace.
//
// Not an interface, and the research says so: it is `grim` narrating its own protocol traffic. It
// is used because `ext_foreign_toplevel_handle_v1.identifier` is the only name `grim -T` answers
// to and no tool on the machine prints it. The parse is deliberately shallow — object id, event
// name, one string argument — so that a change in grim's colouring or timestamps cannot break it;
// if the interface itself moves, the fallback named in the research is a small
// `ext_foreign_toplevel_list_v1` client of our own.
export function parseToplevels(trace) {
  const plain = trace.replace(/\[[0-9;]*m/g, '');
  const event = /ext_foreign_toplevel_handle_v1#(\d+)\.(identifier|app_id|title)\("((?:[^"\\]|\\.)*)"\)/;
  const seen = new Map();
  for (const line of plain.split('\n')) {
    const m = event.exec(line);
    if (!m) continue;
    const handle = seen.get(m[1]) || {};
    handle[m[2] === 'app_id' ? 'appId' : m[2]] = m[3];
    seen.set(m[1], handle);
  }
  return [...seen.values()]
    .filter((h) => h.identifier)
    .map((h) => ({ id: h.identifier, appId: h.appId ?? null, title: h.title ?? null }));
}

// The one toplevel of `appId` that is in `after` and was not in `before`.
//
// Which window is ours cannot be asked of the toplevel list — it carries no pid — so it is
// answered by watching one appear. That also answers it when the owner already has Quill open, and
// when the stage's own parking window is up: both were there before.
export function appeared(before, after, appId) {
  const was = new Set(before.filter((t) => t.appId === appId).map((t) => t.id));
  const now = after.filter((t) => t.appId === appId && !was.has(t.id));
  if (now.length === 1) return now[0];
  if (now.length === 0) return null;
  throw new Error(`${now.length} windows of ${appId} appeared at once; the harness cannot tell which is the one it launched`);
}

// ---------- the PNG ----------

// The pixel size in a PNG's header, which is the one thing about a shot that can be checked
// without judging it: a judged state is `w`x`h` logical at scale 2, and anything else is a window
// the rules did not pin.
export function pngSize(buf) {
  if (buf.length < 24 || buf.readUInt32BE(0) !== 0x89504e47) throw new Error('not a PNG');
  if (buf.toString('latin1', 12, 16) !== 'IHDR') throw new Error('a PNG whose first chunk is not IHDR');
  return { w: buf.readUInt32BE(16), h: buf.readUInt32BE(20) };
}

// ---------- the compositor ----------

function hyprctl(args, { json = false } = {}) {
  const out = execFileSync('hyprctl', args, { encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] });
  return json ? JSON.parse(out) : out.trim();
}

// `hyprctl eval` prints "ok" whatever the Lua returned; `hyprctl repl` prints the value. Everything
// here goes through `repl` so that a rule set or a teardown can say what it did rather than that it
// was asked.
function lua(source) {
  return hyprctl(['repl', source]);
}

// Whether there is a Hyprland to put a stage on at all. A judging session on a machine without one
// should say so in a line rather than in a stack trace from the first `hyprctl`.
export function compositorAvailable() {
  try {
    hyprctl(['monitors', '-j'], { json: true });
    return true;
  } catch {
    return false;
  }
}

function monitors() { return hyprctl(['monitors', '-j'], { json: true }); }
function clients() { return hyprctl(['clients', '-j'], { json: true }); }

function activeWindowAddress() {
  try { return hyprctl(['activewindow', '-j'], { json: true }).address || null; } catch { return null; }
}

function cursorPosition() {
  const [x, y] = hyprctl(['cursorpos']).split(',').map((n) => Number(n.trim()));
  return Number.isFinite(x) && Number.isFinite(y) ? { x, y } : null;
}

function focusWindow(address) {
  // A class string is silently ignored by `hl.dsp.focus`; the window object out of `hl.get_windows`
  // is what it takes, and the address is how one is recognised.
  return lua(`for _, w in ipairs(hl.get_windows()) do if w.address == "${address}" then hl.dispatch(hl.dsp.focus{window=w}) return "focused" end end return "gone"`);
}

function movePointer({ x, y }) {
  lua(`hl.dispatch(hl.dsp.cursor.move(${Math.round(x)}, ${Math.round(y)})) return "moved"`);
}

// Every toplevel the compositor is showing, now.
function toplevels() {
  // grim exits non-zero because `__none__` is not a toplevel, which is the point: the enumeration
  // happens on the way to finding that out, and the trace is on stderr either way.
  try {
    execFileSync('grim', ['-T', '__none__', '/dev/null'], {
      encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'], env: { ...process.env, WAYLAND_DEBUG: '1' },
    });
    return [];
  } catch (e) {
    return parseToplevels(`${e.stdout || ''}${e.stderr || ''}`);
  }
}

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

// ---------- the stage ----------

// Opens the stage: a headless output at the judged scale, on a workspace of its own, with the
// owner's focus, workspace and pointer recorded so that [`Stage.close`] can put them back.
//
// The teardown is registered before the output is created, not after, so that a failure inside
// this function is still torn down; `close` is idempotent and says nothing the second time.
//
// The mode and the scale are not arguments. They are the judged states' own — 3200x2000 at integer
// scale 2 is what `w`x`h`x2 is asserted against in [`Stage.shoot`] — so a caller that could pass a
// different scale could only pass one that every capture then refuses.
export async function openStage({ root, appId = APP_ID } = {}) {
  const owner = { window: activeWindowAddress(), cursor: cursorPosition() };
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'quill-gate-'));

  const state = { output: null, workspace: null, closed: false, children: new Set() };

  // The trap. `process.on('exit')` runs after an uncaught throw and after a normal return, and only
  // synchronous work survives it — which everything in `close` is. The signals are separate
  // because Node does not run exit handlers for them by default: they end the process outright, and
  // a stage that ended that way is exactly the one that would leave an output behind.
  const onExit = () => { try { close(); } catch { /* teardown is best effort by then */ } };
  const onSignal = (sig) => { onExit(); process.exit(sig === 'SIGINT' ? 130 : 143); };
  process.on('exit', onExit);
  process.on('SIGINT', onSignal);
  process.on('SIGTERM', onSignal);
  process.on('SIGHUP', onSignal);

  function close() {
    if (state.closed) return;
    state.closed = true;
    // Order matters: the windows go first, because a window on an output that has just been removed
    // is a window Hyprland has moved somewhere the owner can see.
    for (const child of state.children) { try { child.kill('SIGKILL'); } catch { /* already gone */ } }
    state.children.clear();
    try { lua(RULES_OFF); } catch { /* no compositor left to tell */ }
    if (state.output) { try { hyprctl(['output', 'remove', state.output]); } catch { /* already gone */ } }
    state.output = null;
    if (owner.cursor) { try { movePointer(owner.cursor); } catch { /* ditto */ } }
    if (owner.window) { try { focusWindow(owner.window); } catch { /* ditto */ } }
    try { fs.rmSync(tmp, { recursive: true, force: true }); } catch { /* ditto */ }
    process.removeListener('exit', onExit);
    process.removeListener('SIGINT', onSignal);
    process.removeListener('SIGTERM', onSignal);
    process.removeListener('SIGHUP', onSignal);
  }

  const before = monitors().map((m) => m.name);
  hyprctl(['output', 'create', 'headless']);
  // Named by watching one appear, and watched for rather than looked for once. `output create`
  // answers before the monitor is in `hyprctl monitors`, and an output this function did not manage
  // to name is an output `close` cannot remove — the one way this stage can leak something onto the
  // owner's machine. A second of asking is far past the millisecond it actually takes.
  let output = null;
  for (let waited = 0; waited < CREATE_TIMEOUT_MS && !output; waited += POLL_MS) {
    output = monitors().map((m) => m.name).find((n) => !before.includes(n)) || null;
    if (!output) await sleep(POLL_MS);
  }
  if (!output) {
    close();
    throw new Error('asked Hyprland for a headless output and no new monitor appeared; if one appears now, remove it with `hyprctl output remove <name>`');
  }
  state.output = output;

  lua(`hl.monitor({output="${output}", mode="${MODE}", position="auto", scale=${SCALE}}) return "set"`);
  const monitor = monitors().find((m) => m.name === output);
  if (!monitor) { close(); throw new Error(`${output} disappeared while it was being configured`); }
  if (monitor.scale !== SCALE) { close(); throw new Error(`${output} came up at scale ${monitor.scale}, not ${SCALE}: a judged shot at a fractional scale is not the judged state`); }
  state.workspace = monitor.activeWorkspace.id;

  // Somewhere on the stage that no judged window covers: the pointer is parked here so that
  // `follow_mouse` cannot hand focus to whatever happens to be under it. The window is at MARGIN and
  // is at most the mode's logical size less that, so the far corner is always clear of it.
  const logical = { w: Math.round(monitor.width / SCALE), h: Math.round(monitor.height / SCALE) };
  const corner = { x: monitor.x + logical.w - 4, y: monitor.y + logical.h - 4 };

  return new Stage({ root, appId, tmp, state, monitor, corner, close });
}

class Stage {
  constructor({ root, appId, tmp, state, monitor, corner, close }) {
    this.root = root;
    this.appId = appId;
    this.output = state.output;
    this.workspace = state.workspace;
    this.monitor = monitor;
    this.corner = corner;
    this.tmp = tmp;
    this.state = state;
    this.close = close;
  }

  /// Puts this harness's rules on the class, sized for the state about to be shot.
  rules({ w, h, initialFocus }) {
    const file = path.join(this.tmp, 'rules.lua');
    fs.writeFileSync(file, rulesLua(this.appId, { workspace: this.workspace, w, h, initialFocus }));
    lua(`dofile("${file}") return "rules"`);
  }

  /// Launches the binary and waits for the window it maps, answering with the process, its Hyprland
  /// address and its toplevel identifier.
  ///
  /// A launch that dies is told apart from a launch that is slow: the child is watched, and its
  /// stderr is kept so that the reason a binary would not open a window is on the way to the line
  /// the owner reads rather than lost.
  ///
  /// Spawned here rather than through `hl.dsp.exec_cmd("[workspace N silent] ...")`, which is what
  /// the research's recipe uses. `exec_cmd` hands the process to the compositor and answers with a
  /// Lua table, so the harness would have no pid to kill on the way out, no exit code to tell a
  /// binary that died from one that is slow, and no stderr to say why. The `[workspace N silent]`
  /// half of it is a window rule anyway, and is one here — verified: a directly spawned window
  /// lands on the stage's workspace at the pinned geometry with the owner's focus untouched.
  async launch(bin, argv) {
    const before = toplevels();
    const child = spawn(bin, argv, { cwd: this.root, env: launchEnv(), stdio: ['ignore', 'ignore', 'pipe'] });
    this.state.children.add(child);
    let stderr = '';
    child.stderr.on('data', (b) => { stderr += b; });
    let died = null;
    child.on('error', (e) => { died = e.message; });
    child.on('exit', (code, signal) => { died = signal ? `it was killed (${signal})` : `it exited with ${code}`; });

    try {
      for (let waited = 0; waited < MAP_TIMEOUT_MS; waited += POLL_MS) {
        await sleep(POLL_MS);
        if (died) break;
        const toplevel = appeared(before, toplevels(), this.appId);
        if (!toplevel) continue;
        const window = clients().find((c) => c.pid === child.pid);
        if (!window) continue;           // Hyprland has the toplevel but not yet the window
        return { child, toplevel, address: window.address, geometry: { at: window.at, size: window.size } };
      }
    } catch (e) {
      // Two windows appearing at once, or a compositor that stopped answering. The child is this
      // function's until it hands one back, so it does not outlive the throw.
      this.kill(child);
      throw e;
    }
    this.kill(child);
    const why = died ? `: ${died}` : ` within ${MAP_TIMEOUT_MS / 1000}s`;
    throw new Error(`the launch put no window on the compositor${why}${stderr.trim() ? `\n${stderr.trim()}` : ''}`);
  }

  kill(child) {
    if (!child) return;
    this.state.children.delete(child);
    try { child.kill('SIGTERM'); } catch { /* already gone */ }
  }

  /// Captures one toplevel twice and answers with the bytes, once two consecutive captures agree.
  ///
  /// Two captures rather than one is the whole determinism check that can be made from outside the
  /// app: a caret still blinking, an animation still running or a frame still being painted all show
  /// up as two different files, and none of them is visible in one.
  async steady(toplevel) {
    const a = path.join(this.tmp, 'a.png');
    const b = path.join(this.tmp, 'b.png');
    let last = null;
    for (let attempt = 0; attempt < STEADY_TRIES; attempt++) {
      execFileSync('grim', ['-T', toplevel, a]);
      execFileSync('grim', ['-T', toplevel, b]);
      const first = fs.readFileSync(a);
      const second = fs.readFileSync(b);
      if (first.equals(second)) return first;
      last = { a: crypto.createHash('sha256').update(first).digest('hex').slice(0, 12), b: crypto.createHash('sha256').update(second).digest('hex').slice(0, 12) };
      await sleep(SETTLE_MS);
    }
    throw new Error(`two consecutive captures never agreed (${last.a} then ${last.b}); the window is still moving`);
  }

  /// Shoots one judged state into `out`, and answers with the sha256 of what it wrote.
  ///
  /// `active` is keyboard focus, and it is the harness's rather than a flag, because no application
  /// can give itself focus. It is true by holding focus on ours, and false by parking focus on a
  /// second window of our own on the same stage — never by handing it back to the owner, whose
  /// window is not a prop in a judged state.
  async shoot({ bin, argv, w, h, out, active = true }) {
    this.rules({ w, h, initialFocus: active });
    let parked = null;
    let ours = null;
    try {
      // The pointer comes over first in both cases: focus follows the mouse on this desktop, and a
      // pointer still on the owner's panel takes focus back off whatever is focused here.
      movePointer(this.corner);

      if (!active) {
        // Parked first, and focused, so that at no moment between the launch and the shutter does
        // ours hold focus — `no_initial_focus` keeps it from taking any as it maps.
        parked = await this.launch(bin, quillArgv(this.root, PARKING));
        focusWindow(parked.address);
      }

      ours = await this.launch(bin, argv);
      if (active) focusWindow(ours.address);
      await sleep(SETTLE_MS);

      const png = await this.steady(ours.toplevel.id);
      const size = pngSize(png);
      const want = { w: w * SCALE, h: h * SCALE };
      if (size.w !== want.w || size.h !== want.h) {
        throw new Error(`the window was captured at ${size.w}x${size.h}, not the ${want.w}x${want.h} this state is judged at`);
      }
      fs.mkdirSync(path.dirname(out), { recursive: true });
      fs.writeFileSync(out, png);
      return crypto.createHash('sha256').update(png).digest('hex');
    } finally {
      this.kill(ours?.child);
      this.kill(parked?.child);
      lua(RULES_OFF);
    }
  }
}

// The parking window's state: an empty Quill whose only job is to be the thing keyboard focus is on
// while an unfocused state is shot. It is a Quill because a Quill is the one window this repository
// can be sure exists on the machine.
//
// Its own `w` and `h` decide nothing — the stage's rules are scoped to the class, so this window
// maps at the judged state's size and position, exactly on top of ours. That costs the shot
// nothing: `grim -T` reads a toplevel's own buffer rather than the composited output, so what is in
// front of ours is not in the capture. The two numbers are here because `quillArgv` takes a whole
// state and a state has a size; they are the judged default so that nothing surprising happens if a
// rule ever fails to apply.
const PARKING = {
  w: 1440, h: 900, theme: 'light', font: 'duo', size: 20, focus: 'off', chrome: 'on',
  typewriter: false, nocaret: false, text: null, caret: null, select: null, scroll: 0,
};
