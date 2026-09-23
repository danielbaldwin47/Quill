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
//
// WHY A SHOT PROVES ITS OWN CARET. Focus read back from the compositor answers for the compositor.
// The app's side of it is a separate asynchronous chain — `wl_keyboard.enter`, GTK's `is-active`,
// `set_active(true)`, a queued draw, the next frame callback — and `watch_active` seeds it inactive,
// so the first committed frames carry the ghost caret. An unfocused caret asks for no ticks, so a
// window still on that frame is perfectly still: `steady()`, `SETTLE_MS` and the read-back all pass
// on it, and `grim -T` hands back the ghost. `theme-r2` lost `theme/dark` exactly there, on a caret
// measured `#124b5e` — the accent at `GHOST` 0.3 over the dark ground — and the critic spent its
// whole verdict calling the frame a design failure. So the glass is asked one more question
// ([`carriesAccent`]): a judged shot is `--deterministic`, its blink frozen at alpha 1.0, and
// `Role::Accent` is one flat colour on both grounds, so an active state that draws a caret must
// hold at least one pixel of it and a ghosted frame never does.
import { execFileSync, spawn, spawnSync } from 'node:child_process';
import crypto from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

import { overlaid } from './crop.mjs';
import { decodePng } from './keys-assert.mjs';

// The `GtkApplication` application-id, which is the xdg-toplevel `app_id`, which is what Hyprland
// reports as a window's `class` and what `ext_foreign_toplevel_handle_v1` reports as its `app_id`.
// One string, three names for it; `quill/src/main.rs` holds the original.
export const APP_ID = 'io.github.danielbaldwin47.Quill';

// The `app_id` a window of ours that is not the application window carries.
//
// GTK gives a `GtkApplicationWindow` the application id and every other `GtkWindow` the program
// name, which for this binary is `quill`. The Export dialog `--export-dialog` opens and the
// Settings window `--pane` opens are plain `gtk::Window`s (ADR 0009, plain GTK), so each maps as a
// toplevel of its own under this second class — which is what lets the stage rule it apart from
// the window it stands over ([`rulesLua`]) and what makes it findable in the toplevel list.
// Measured on the stage, not read out of GTK: a `--export-dialog` or `--pane` shoot says so at
// once if it ever moves, because there is no second toplevel to find and the shot refuses rather
// than coming back without the window in it.
export const DIALOG_APP_ID = 'quill';

// The stage's own numbers. The mode is the widest judged state (1440 logical) plus margin, at the
// integer scale every judged shot is taken at; the window sits at `MARGIN` from the top left so
// that no edge of it is an edge of the output.
const MODE = '3200x2000@60';
// The mode's size alone, which is what `hyprctl monitors` answers with and so what it is checked
// against once the compositor has applied it.
const MODE_SIZE = MODE.split('@')[0];
const SCALE = 2;
const MARGIN = { x: 80, y: 50 };

// How long a launch has to put a window on the compositor before the launch is called failed, and
// how often that is checked. Ten seconds is far past a native app's cold start (the research
// measured a *Python* GTK app at 200 ms) and short enough that a binary which never maps a window
// fails while an agent is still watching.
const MAP_TIMEOUT_MS = 10_000;

// How long a created output has to turn up in `hyprctl monitors` before the stage gives up on it.
// `output create` answers before the monitor is listed, and a second of asking is far past the
// millisecond it actually takes.
const CREATE_TIMEOUT_MS = 1_000;
const POLL_MS = 50;

// The display is one lock. Two stages open at once take each other's rules down (ONE STAGE AT A
// TIME, above), and two sessions each asking Hyprland for an output hung it on 2026-09-02 — so a
// stage holds this file, through `flock(2)` on a descriptor of this process, from before it asks
// the compositor for anything until `close`. A second `shoot`, `judge`, `keys` or `bench` waits
// its turn rather than refusing, because the turn comes: a `bench --all` is about six minutes and
// a judge lets go before its critics run. Twenty minutes is past every run there is, and a lock
// held longer than that is a run that has hung, which the refusal names by the line the holder
// wrote into the file. Under `$XDG_RUNTIME_DIR`, which is per user and gone at logout.
const LOCK = path.join(process.env.XDG_RUNTIME_DIR || os.tmpdir(), 'quill-gate-display.lock');
const LOCK_WAIT_S = 1200;

// What a window is given between mapping and the shutter. GTK maps, then paints, then the
// compositor presents; a capture taken inside that has caught a half-drawn frame. The pair check in
// [`Stage.shoot`] is what actually proves the window has settled — this is only how long is waited
// before asking the question the first time.
const SETTLE_MS = 400;

// How many times a shot is asked for before the window is called unsteady. Each attempt is two
// captures that have to be the same bytes; a window that cannot manage that in five attempts is
// animating something, and the shot would be a coin toss rather than a judged state.
const STEADY_TRIES = 5;

// How many times focus is dispatched and read back before the window is called unfocusable. A bench
// refuses to type at that point, so this is deliberately more than a shot needs: the cost of one
// more try is a `hyprctl` call, and the cost of giving up too early is a run that did not happen.
const FOCUS_TRIES = 10;

// `Role::Accent` as `quill-engine/src/theme.rs` paints it, and the one colour a lit caret is made
// of. It is written here rather than solved because there is nothing in a single shot to solve it
// out of: the ghost and the bar are the same hue, and only their alpha differs. It is the same
// value on both grounds — `docs/design.md` row Accent, measured off the Design oracle in VERDICTS
// 4.2.7–4.2.8 — which is what lets one number stand for a shot in either theme. `tools/gate check`
// holds it to `theme.rs`'s own two rows, because this repository has twice had a colour copied into
// `tools/` outlive the palette it came from (`tools/keys-assert.mjs`, #197).
export const ACCENT = { r: 0x00, g: 0xbf, b: 0xff };
export const ACCENT_HEX = `#${[ACCENT.r, ACCENT.g, ACCENT.b].map((v) => v.toString(16).padStart(2, '0')).join('')}`;

// How many times a shot whose caret came back ghosted is re-settled and re-captured before the
// shot is refused. The repaint is one activation notify away, so a shot that has not lit by the
// third pass — each of them a `SETTLE_MS` wait and two full captures — is not racing, and going
// round again would only turn a refusal into a slower refusal.
const LIT_TRIES = 3;

// How much of a launch's stdout is kept. The one line anything reads from it is the cold start,
// printed at the first frame; this is far past that and small enough that a chatty binary cannot
// grow a long run's memory.
const SAID_MAX = 64 * 1024;

// The workspace a window takes when it has to be on the real monitor rather than on a headless
// output: CLAUDE.md's hard rule for every test window, and never workspace 1, which is the owner's
// live one.
export const PANEL_WORKSPACE = 5;

// How long every real keyboard and pointer on the machine has to have been silent before the panel
// may be taken. `tools/idle-check.py`'s own default, said again here because it is this harness that
// decides when somebody is at the machine and the number belongs beside that decision.
export const PANEL_IDLE_S = 8;

// How long the panel's workspace switch is given to show up in `hyprctl activeworkspace`.
const SWITCH_TIMEOUT_MS = 1_500;

// ---------- the command line a judged state opens ours with ----------

// The native app's arguments for one resolved judged state.
//
// Offsets need no conversion here: `--caret` and `--select` take UTF-8 bytes from the start of the
// Document, which is the form `dev/shots/oracle/states.json` writes them in. (`tools/gate oracle` has
// to convert, because the browser shooter counts characters.)
//
// `scale` and `active` are not here and never will be: the first is the output's, and the second
// is keyboard focus, which is the compositor's to give and not a flag the app could honour.
//
// `live` drops `--deterministic`, and only `tools/gate keys` asks for it. A judged still wants the
// blink frozen on and the glide taken out, so that two shots of one state are the same bytes; a
// condition that types wants the caret machine running, because the machine is the thing it is
// there to test. Everything else about the state is unchanged, so the two commands open the same
// document at the same size in the same theme.
export function quillArgv(root, flags, { live = false } = {}) {
  const argv = live ? [] : ['--deterministic'];
  argv.push('--w', String(flags.w), '--h', String(flags.h));
  argv.push('--theme', flags.theme, '--font', flags.font, '--step', String(flags.step));
  argv.push('--focus', flags.focus, '--chrome', flags.chrome);
  if (flags.typewriter) argv.push('--typewriter');
  // Live is passed only when the state names it, and absent it is pinned off by `--deterministic`
  // (`Flags::over`), which is what leaves every state judged before Live existed the page it was.
  // `flags.live` is the app's Live mode and has nothing to do with this function's own `live`
  // option above, which is `tools/gate keys` asking for a launch that is not `--deterministic`.
  if (flags.live) argv.push('--live');
  if (flags.syntax) argv.push('--syntax', flags.syntax);
  if (flags.style) argv.push('--style', flags.style);
  if (flags.spell) argv.push('--spell', flags.spell);
  if (flags.stats) argv.push('--stats', flags.stats);
  if (flags.nocaret) argv.push('--nocaret');
  // The two chrome states the bars alone do not reach: `--typing` is the chrome stepped back, and
  // `--menu` is one popover open with its first row selected. Both are states the app is put in
  // before its first frame, so they are flags and not a script of keystrokes.
  if (flags.typing) argv.push('--typing');
  if (flags.menu) argv.push('--menu', flags.menu);
  // What the Palette's field holds as it opens: `chrome/palette-settings` types a query that lists
  // settings rows under the Commands. A state's own key and never a default's, as `pane` is, and
  // only beside `menu: palette`, since the app refuses it anywhere else.
  if (flags.query) argv.push('--query', flags.query);
  // The Library the `files` states open. `--library` is the fixture folder — joined to the root the
  // way `--text` is, because ours is launched from wherever the harness happens to be — `--sidebar`
  // shows the pane, and `--search` puts the query in its field. The app copies the tree and stamps
  // it from the fixture's `manifest.json`, so the Date sort is the same on every machine.
  if (flags.library) argv.push('--library', path.join(root, flags.library));
  if (flags.sidebar) argv.push('--sidebar');
  if (flags.search) argv.push('--search', flags.search);
  // The Preview pane the `preview` states open, and the Template its page is laid out in.
  // `--preview` is the layout — Split divides the pair, Full puts the page where the Editor's
  // scroller was — and it is named only by a state that wants the pane, because the pane's being
  // open is never remembered and a state that says nothing about it is shot with no pane at all.
  // `--template` is named by every state, off the defaults: it pins the whole `[template]` table,
  // and the shape of a heading is the shape of one whatever Piece the shot is of.
  if (flags.preview) argv.push('--preview', flags.preview);
  // `--zoom` is the one key of `[preview]` a state names for itself, over the pinning `--preview`
  // does: `preview/pdf-full` is shot at 75 % so that two pages of the column stand in one window
  // and the air between them is a thing a still can hold.
  if (flags.zoom) argv.push('--zoom', String(flags.zoom));
  if (flags.template) argv.push('--template', flags.template);
  // The Export dialog the `export` states open, named by its format. A still cannot pull an
  // expander open, so the flag opens the dialog with its Options already showing; the dialog is a
  // surface of its own over the page, which is why the state is shot with the caret away
  // ([`wantsLitCaret`]) — the keyboard is the dialog's while it is up.
  if (flags.export) argv.push('--export-dialog', flags.export);
  // The Settings window the `settings` states open, on the pane named. It is shot as the Export
  // dialog is, a second toplevel over the page ([`opensSecondWindow`]). The key is a state's own and
  // never a default's: a key in `defaults` is in every state's flags, and so in every Piece's
  // fingerprint.
  if (flags.pane) argv.push('--pane', flags.pane);
  // An empty Document has no passage, and so has no offset into one either.
  if (flags.text) {
    argv.push('--text', path.join(root, flags.text));
    if (flags.caret !== null && flags.caret !== undefined) argv.push('--caret', String(flags.caret));
    if (flags.select) argv.push('--select', flags.select.join(','));
  }
  // Named whenever the state names it, 0 included: a state whose view is at the top of the
  // page says so with `--scroll 0`, and a falsy check would drop exactly that state and
  // shoot the app wherever it happened to have scrolled itself instead.
  if (flags.scroll !== null && flags.scroll !== undefined) argv.push('--scroll', String(flags.scroll));
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
//
// `ENCHANT_CONFIG_DIR` is the fixture dictionary's copy when `spell` names one, and every launch
// through a stage names one (`Stage.launch`), whatever `--spell` says: enchant searches that
// directory's `hunspell/` before the system's, so no shot reads the machine's dictionaries, and an
// Add during a keys run writes the copy, never the checkout.
export function launchEnv(env = process.env, spell = null) {
  const out = { ...env, GSK_RENDERER: 'gl', GDK_BACKEND: 'wayland', GTK_A11Y: 'none' };
  delete out.GDK_SCALE;
  if (spell) out.ENCHANT_CONFIG_DIR = spell;
  return out;
}

// A fresh copy of `dev/ref/spell/` under `tmp`, one per launch, so a word one launch added is not in
// the dictionary the next launch reads.
export function spellFixture(root, tmp) {
  const copy = fs.mkdtempSync(path.join(tmp, 'spell-'));
  fs.cpSync(path.join(root, 'dev/ref/spell'), copy, { recursive: true });
  return copy;
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
  const decoration = 'no_anim = true, border_size = 0, rounding = 0, no_shadow = true,'
    + ' no_blur = true, no_dim = true, opacity = "1.0 1.0", tag = "-default-opacity"';
  const lines = [
    'QUILL_GATE_RULES = QUILL_GATE_RULES or {}',
    `local C = "${classPattern(appId)}"`,
    `local D = "${classPattern(DIALOG_APP_ID)}"`,
    'local function rule(t) t.match = { class = C } QUILL_GATE_RULES[#QUILL_GATE_RULES + 1] = hl.window_rule(t) end',
    'local function dialog(t) t.match = { class = D } QUILL_GATE_RULES[#QUILL_GATE_RULES + 1] = hl.window_rule(t) end',
    `rule({ name = "quill-gate-workspace", workspace = "${workspace} silent" })`,
    'rule({ name = "quill-gate-float", float = true })',
    `rule({ name = "quill-gate-size", size = "${w} ${h}" })`,
    `rule({ name = "quill-gate-move", move = "${x} ${y}" })`,
    `rule({ name = "quill-gate-decoration", ${decoration} })`,
    // The dialog a `--export-dialog` state opens, and the Settings window a `--pane` state opens,
    // is a toplevel of its own under a second class
    // ([`DIALOG_APP_ID`]), and it is ruled separately for two reasons. It wants what ours wants:
    // the stage's workspace, so it is not put on the owner's, and Omarchy's compositing undone. And
    // it must not be given ours' size and position — a dialog is placed by the compositor against
    // the window it is transient for, and where it lands is the thing `export/dialog` measures.
    `dialog({ name = "quill-gate-dialog-workspace", workspace = "${workspace} silent" })`,
    'dialog({ name = "quill-gate-dialog-float", float = true })',
    `dialog({ name = "quill-gate-dialog-decoration", ${decoration} })`,
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

/// How many pixels of `accent` a shot holds, exactly — no threshold and no neighbourhood.
///
/// Exact is the whole point. Every near-accent pixel in a judged shot is the accent under an alpha:
/// the ghost caret at `GHOST` 0.3, a selection fill at .22, the antialiased column at either end of
/// the bar. A tolerance wide enough to be kind would let the ghost in, which is the one thing this
/// is asked to keep out. The bar's own core is flat — 444 px of it in every shot this was measured
/// on — so exactness costs nothing.
export function accentPixels(png) {
  let held = 0;
  for (let i = 0; i < png.w * png.h; i += 1) {
    const at = i * png.ch;
    if (png.data[at] === ACCENT.r && png.data[at + 1] === ACCENT.g && png.data[at + 2] === ACCENT.b) held += 1;
  }
  return held;
}

/// Whether a capture holds a caret painted at full alpha.
///
/// The one question `steady()` cannot answer, and the only one asked of a shot's pixels here: the
/// harness judges nothing, it only refuses to hand a critic a frame ours had not finished waking
/// up into. Measured on the four shots #197 came out of — the ghosted `theme/dark` capture holds
/// 444 px of `#124b5e` and none of the accent, and the three clean ones hold 444 px of the accent.
export function carriesAccent(buf) {
  return accentPixels(decodePng(buf)) > 0;
}

/// Whether a shot of `argv` must show a lit caret.
///
/// Read off the command line rather than taken as a flag, because `shoot` is handed a command line
/// and a state is only ever the flags in it. Ours paints no caret under `--nocaret`, and paints a
/// selection's band instead of a bar under `--select` — `caret/selection` is 444 px short of an
/// accent by design, and `mac-native` 03 and 04 have no accent pixel in the frame either. Every
/// other active state draws the bar, an empty Document included: `page/empty` is the state #166
/// lost to the ghost.
///
/// A Full pane is the fourth way out, and the only one that is not about the caret at all: Full
/// puts the rendered page where the Editor's scroller was, so there is no Editor on the glass to
/// draw a bar and no accent pixel to wait for. Both words that open one are read — `full`, the
/// rendered sheet, and `pdf-full`, the page column — because what the pane draws there makes no
/// difference to the Editor being gone. It is read here rather than taken as the state's `nocaret`
/// because the state has not asked for a caret to be left out — the layout has none to leave.
/// Either Split keeps the Editor beside the page and is held to a lit frame like any other state.
///
/// `--deterministic` is a condition in its own right and not a detail of the ways out: it is what
/// freezes the blink on, and [`quillArgv`] drops it for a Live launch, where the caret is meant to
/// be dark half the time. A Live shot has no lit frame to insist on, so it is shot the way it
/// always was.
///
/// The list is the flags ours has today. `chrome/view-menu` and `chrome/palette` name `--menu`,
/// which `quill/src/main.rs` has not grown yet, so `judge chrome` refuses on the flag long before
/// it reaches here; the session that builds `--menu` decides whether a popover leaves the Editor
/// drawing its bar, and adds a way out here if it does not.
export function wantsLitCaret(argv, { active = true } = {}) {
  const preview = argv.includes('--preview') ? argv[argv.indexOf('--preview') + 1] : null;
  return active
    && argv.includes('--deterministic')
    && !argv.includes('--nocaret')
    && !argv.includes('--select')
    && !(preview === 'full' || preview === 'pdf-full');
}

/// Whether a launch of `argv` puts a second toplevel of ours over the page: the Export dialog
/// `--export-dialog` opens, or the Settings window `--pane` opens. Read off the command line for the
/// reason [`wantsLitCaret`] is.
export function opensSecondWindow(argv) {
  return argv.includes('--export-dialog') || argv.includes('--pane');
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
  return activeWindow().address;
}

// The focused window's address and class, which is what a bench has to be sure of before it writes
// a key to `/dev/uinput`: nothing an application can ask for, and the only honest answer to "where
// will this key land". Exported because refusing to type is the bench's decision, not the stage's.
export function activeWindow() {
  try {
    const active = hyprctl(['activewindow', '-j'], { json: true });
    return { address: active.address || null, class: active.class || null };
  } catch {
    return { address: null, class: null };
  }
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

// The workspace one monitor is showing, by id, or null when there is no such monitor.
//
// Asked of `hyprctl monitors` rather than of `hyprctl activeworkspace`, which answers for whichever
// monitor holds focus and so cannot say what the panel is showing while something else is focused.
function monitorWorkspace(name) {
  const found = monitors().find((m) => m.name === name);
  return found ? found.activeWorkspace.id : null;
}

// Puts keyboard focus on a monitor by name, so that a workspace dispatch lands on that one.
function focusMonitor(name) {
  return lua(`return hl.dispatch(hl.dsp.focus{monitor=hl.get_monitor("${name}")})`);
}

// Puts a workspace up on the panel, and answers with whatever Hyprland said.
//
// Hyprland 0.56 dropped the old string dispatcher: `hyprctl dispatch workspace N` is now parsed as
// Lua, fails, and changes nothing — which would leave a window measuring on a workspace nobody is
// looking at, the one thing the panel mode exists to avoid. `hl.dsp.focus{workspace=N}` is the form
// that works (`dev/legacy/bin/quill:168-185` records the search that found it), and every caller reads
// the switch back rather than trusting this return.
function gotoWorkspace(id) {
  return lua(`return hl.dispatch(hl.dsp.focus{workspace=${id}})`);
}

// The monitors of `list` that are a panel somebody could be looking at.
//
// Hyprland substitutes a headless output called `FALLBACK` when every real output is asleep — a
// DisplayPort monitor in standby drops the link — and the Gate's own stage is a `HEADLESS-N`.
// Neither of them scans out to anything, which is the one thing the panel mode is for.
export function physicalMonitors(list) {
  return list.filter((m) => !/^(HEADLESS|FALLBACK)/.test(String(m.name ?? '')));
}

// Why the panel cannot be taken for a run, or `null` when it can.
//
// Pure, and asked of a monitor list rather than of the compositor, because these are the refusals
// that most need to be right and least can be rehearsed: this is the mode that puts a window in
// front of somebody. `tools/bench-selftest.mjs` checks them by handing this the machines that
// cannot be arranged on demand — one with its panel asleep, one with the owner sitting on the
// workspace the run wants.
//
// "In use" is asked of the panel's own `activeWorkspace`, never of `hyprctl activeworkspace`, which
// answers for whichever monitor holds focus. On a machine with a stray headless output focused the
// two disagree, and the global answer would clear a workspace that is in fact the one on screen —
// then bring the run up on the headless output, which scans out to nothing. That is the exact
// failure this mode exists to prevent.
//
// The idle check is not here. It is a question about the last few seconds rather than about the
// machine's shape, so it is asked once, by `openPanelStage`, immediately before the switch.
export function panelRefusal({ monitors: list, workspace }) {
  const panel = physicalMonitors(list)[0];
  if (!panel) {
    return 'no physical output is connected (Hyprland is on FALLBACK — the panel is asleep), '
      + 'and there is nothing to measure scan-out on';
  }
  if (workspace === 1) return "workspace 1 is the owner's live workspace";
  if (workspace === panel.activeWorkspace?.id) {
    return `workspace ${workspace} is the one in use on ${panel.name}`;
  }
  return null;
}

// The same question, asked of this machine.
export function panelBlocked({ workspace = PANEL_WORKSPACE } = {}) {
  if (!compositorAvailable()) return 'there is no compositor to put a window on';
  return panelRefusal({ monitors: monitors(), workspace });
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

// The trail a stage says the one thing it has to say on the way in to — that it is waiting for
// the display — when its caller gave it no `say` of its own.
const stderr = (line) => process.stderr.write(`${line}\n`);

// ---------- choosing what to run ----------

// A refusal of the command line, thrown by [`chosenList`]: the message is the line the command
// prints above its usage.
export class Refusal extends Error {}

// What a command that runs over several of a kind — `gate shoot` over Pieces, `gate bench` over
// regimes — was told to run: `--all` for every name `universe()` answers, `--<flag> a,b` for the
// names listed, or neither, when the name given as an argument (or the default) is the caller's
// own to resolve. `all` and `listed` are what the command line carried — whether `--all` was
// given, and the flag's value or null — and `named` is the argument, for a command that counts it
// against the flags. Answers `{ flag, names }`: the flag that chose and the names it chose, both
// null when neither flag was given. Throws a `Refusal` for a list with no names in it, and for two
// of them saying which.
export function chosenList({ command, flag, listOf, what, all, listed, named = null, universe }) {
  const names = listed === null ? null : String(listed).split(',').map((s) => s.trim()).filter(Boolean);
  if (names !== null && !names.length) throw new Refusal(`${command}: ${flag} takes ${listOf} separated by commas`);
  const asked = [named, all ? '--all' : null, names ? flag : null].filter(Boolean);
  if (asked.length > 1) throw new Refusal(`${command}: ${asked.join(' and ')} both say ${what}; pick one`);
  if (all) return { flag: '--all', names: universe() };
  if (names) return { flag, names };
  return { flag: null, names: null };
}

// The wall clock in nanoseconds, which is what the app reads `$QUILL_T0_NS` as: `date +%s%N`'s
// scale. `Date.now()` alone would quantise the cold start to the millisecond, so the origin and the
// sub-millisecond offset are added instead.
function nowRealtimeNs() {
  return String(BigInt(Math.round((performance.timeOrigin + performance.now()) * 1e6)));
}

// ---------- the stage ----------

// The trap both stages are torn down by. `process.on('exit')` runs after an uncaught throw and after
// a normal return, and only synchronous work survives it — which everything in a `close` is. The
// signals are separate because Node does not run exit handlers for them by default: they end the
// process outright, and a stage that ended that way is exactly the one that would leave an output
// behind, or leave the owner looking at a workspace they did not choose.
//
// Answers with the way to take the handlers off again, so that a `close` which has just run is not
// a process that still holds four listeners for the rest of the run.
function armTeardown(close) {
  const onExit = () => { try { close(); } catch { /* teardown is best effort by then */ } };
  const onSignal = (sig) => { onExit(); process.exit(sig === 'SIGINT' ? 130 : 143); };
  process.on('exit', onExit);
  process.on('SIGINT', onSignal);
  process.on('SIGTERM', onSignal);
  process.on('SIGHUP', onSignal);
  return () => {
    process.removeListener('exit', onExit);
    process.removeListener('SIGINT', onSignal);
    process.removeListener('SIGTERM', onSignal);
    process.removeListener('SIGHUP', onSignal);
  };
}

// Takes the display lock, and answers with the descriptor that holds it. `flock` is handed the
// descriptor itself (as its fd 3) rather than the path, so the lock belongs to the open file
// description this process holds: it is released when `close` closes the descriptor or when the
// process ends, and nothing this process spawns inherits it, because Node opens files
// close-on-exec and passes a child only the stdio it is given. The file is opened without
// truncating, so the holder's line can be read before it is overwritten with this run's.
//
// `say` is the caller's trail, because a wait is something the owner should be able to see from
// the log while it is happening, and the one line here is the holder's own description of itself,
// read through the descriptor this process holds: the file it opened is the file it waits on.
function holdDisplay(say) {
  const fd = fs.openSync(LOCK, fs.constants.O_RDWR | fs.constants.O_CREAT, 0o600);
  const holder = () => {
    try {
      const buf = Buffer.alloc(fs.fstatSync(fd).size);
      const read = fs.readSync(fd, buf, 0, buf.length, 0);
      return buf.toString('utf8', 0, read).trim() || 'a run that wrote no name';
    } catch { return 'a run that wrote no name'; }
  };
  const take = (...args) => {
    const r = spawnSync('flock', [...args, '3'], { stdio: ['ignore', 'ignore', 'ignore', fd] });
    if (r.error) { fs.closeSync(fd); throw new Error(`the display lock needs flock(1), which would not run: ${r.error.message}`); }
    return r.status === 0;
  };
  if (!take('-n')) {
    say(`the display is held by ${holder()}; waiting`);
    if (!take('-w', String(LOCK_WAIT_S))) {
      const who = holder();
      fs.closeSync(fd);
      throw new Error(`the display has been held for ${LOCK_WAIT_S}s by ${who}`);
    }
  }
  fs.ftruncateSync(fd);
  fs.writeSync(fd, `pid ${process.pid}: gate ${path.basename(process.argv[1] || '', '.mjs')} ${process.argv.slice(2).join(' ')}`.trimEnd(), 0);
  return fd;
}

// Everything both stages undo, and the order it has to be undone in.
//
// The windows go first, because a window on an output that has just been removed is a window
// Hyprland has moved somewhere the owner can see. `restore` is the one step the two stages do not
// share — the headless one removes the output it made, the panel one puts back the workspace it
// took — and it runs before the pointer and the focus, which are the owner's own and go back last.
// The display lock goes last of all, so the next run in line finds the compositor as this one
// found it.
//
// Idempotent, and says nothing the second time: the trap calls this on paths that may already have.
function closeStage({ state, owner, tmp, disarm }, restore) {
  if (state.closed) return;
  state.closed = true;
  for (const child of state.children) { try { child.kill('SIGKILL'); } catch { /* already gone */ } }
  state.children.clear();
  try { lua(RULES_OFF); } catch { /* no compositor left to tell */ }
  try { restore(); } catch { /* ditto */ }
  if (owner.cursor) { try { movePointer(owner.cursor); } catch { /* ditto */ } }
  if (owner.window) { try { focusWindow(owner.window); } catch { /* ditto */ } }
  try { fs.rmSync(tmp, { recursive: true, force: true }); } catch { /* ditto */ }
  if (state.lock !== null) { try { fs.closeSync(state.lock); } catch { /* the exit releases it anyway */ } state.lock = null; }
  disarm();
}

// Somewhere on the stage that no window of ours covers: the pointer is parked here so that
// `follow_mouse` cannot hand focus to whatever happens to be under it. The window is at MARGIN and
// is at most the output's logical size less that, so the far corner is always clear of it.
function parkingCorner(monitor) {
  const logical = {
    w: Math.round(monitor.width / monitor.scale), h: Math.round(monitor.height / monitor.scale),
  };
  return { x: monitor.x + logical.w - 4, y: monitor.y + logical.h - 4 };
}

// Opens the stage: a headless output at the judged scale, on a workspace of its own, with the
// owner's focus and pointer recorded so that [`Stage.close`] can put them back. The owner's
// workspace is not among them because this stage never changes it — the window goes to the new
// output's own workspace, which nothing is displaying. [`openPanelStage`] is the one that does.
//
// The display lock is taken first, before the owner's focus and pointer are read, so that what
// `close` puts back is what the owner had when this run's turn came rather than when it joined
// the queue. The teardown is registered before the output is created, not after, so that a failure
// inside this function is still torn down; `close` is idempotent and says nothing the second time.
//
// The mode and the scale are not arguments. They are the judged states' own — 3200x2000 at integer
// scale 2 is what `w`x`h`x2 is asserted against in [`Stage.shoot`] — so a caller that could pass a
// different scale could only pass one that every capture then refuses.
//
// `say` takes the one line a wait for the display produces; a caller with a trail of its own
// (`tools/judge.mjs`, `tools/shoot.mjs`) passes it, and the default is stderr.
export async function openStage({ root, appId = APP_ID, say = stderr } = {}) {
  const lock = holdDisplay(say);
  const owner = { window: activeWindowAddress(), cursor: cursorPosition() };
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'quill-gate-'));

  const state = { output: null, workspace: null, lock, closed: false, children: new Set() };
  let disarm = () => {};

  const close = () => closeStage({ state, owner, tmp, disarm: () => disarm() }, () => {
    if (state.output) hyprctl(['output', 'remove', state.output]);
    state.output = null;
  });
  disarm = armTeardown(close);

  // Named for this run's pid, so that an output a killed run left behind says whose it was, and so
  // that two runs can never mistake each other's for their own. It is `state.output` from the
  // moment it is asked for: `output create` answers before the monitor is in `hyprctl monitors`,
  // and one that turns up late is still one `close` removes.
  const output = `quill-gate-${process.pid}`;
  hyprctl(['output', 'create', 'headless', output]);
  state.output = output;
  let monitor = null;
  for (let waited = 0; waited < CREATE_TIMEOUT_MS && !monitor; waited += POLL_MS) {
    monitor = monitors().find((m) => m.name === output) || null;
    if (!monitor) await sleep(POLL_MS);
  }
  if (!monitor) {
    close();
    throw new Error(`asked Hyprland for headless output ${output} and it never appeared; a leftover of an earlier run is any \`quill-gate-*\` in \`hyprctl monitors\`, removed with \`hyprctl output remove <name>\``);
  }

  lua(`hl.monitor({output="${output}", mode="${MODE}", position="auto", scale=${SCALE}}) return "set"`);
  // Read back rather than read once. `hl.monitor` answers before Hyprland has applied the mode, and
  // a headless output is created at 1920x1080 before it is told otherwise — so the obvious single
  // read returns the size the output had a moment ago. That was wrong twice: it put a mode into
  // every bench's fingerprint that the run was not taken at, and it computed `corner` from the
  // wrong logical size, parking the pointer on top of the window instead of clear of it.
  monitor = null;
  for (let waited = 0; waited < CREATE_TIMEOUT_MS; waited += POLL_MS) {
    monitor = monitors().find((m) => m.name === output) || null;
    if (monitor && `${monitor.width}x${monitor.height}` === MODE_SIZE && monitor.scale === SCALE) break;
    await sleep(POLL_MS);
  }
  if (!monitor) { close(); throw new Error(`${output} disappeared while it was being configured`); }
  if (`${monitor.width}x${monitor.height}` !== MODE_SIZE) { close(); throw new Error(`${output} came up at ${monitor.width}x${monitor.height}, not the ${MODE_SIZE} every judged shot and every bench is taken at`); }
  if (monitor.scale !== SCALE) { close(); throw new Error(`${output} came up at scale ${monitor.scale}, not ${SCALE}: a judged shot at a fractional scale is not the judged state`); }
  state.workspace = monitor.activeWorkspace.id;

  return new Stage({ root, appId, tmp, state, monitor, corner: parkingCorner(monitor), close });
}

// Opens the stage on the physical panel: the one measurement a headless output cannot give.
//
// A Wayland surface on a workspace nobody is displaying gets no frame callbacks, so a number about
// scan-out can only be taken with the window genuinely on screen — and `dev/legacy/BRIEF.md` forbids
// doing that to somebody who is working. So this refuses on every count it can before it changes
// anything: no compositor, no panel awake, the owner's own workspace, the workspace they have up on
// the panel, and finally a machine whose keyboard and pointer have not been silent. Nothing is
// created, shown or typed until all five have passed.
//
// The workspace that was up is restored by `Stage.close`, which the trap runs on a normal return,
// on a throw and on a signal — so a run interrupted half way through gives the owner their screen
// back rather than leaving them on the bench's.
//
// The result is not comparable with a headless one and is never a Gate condition: the panel is
// fractional-scale, so the window's buffer is not the judged stage's. `tools/gate bench` marks it
// informational, and the mode and scale it was taken at go into the result's fingerprint.
export async function openPanelStage({
  root, appId = APP_ID, workspace = PANEL_WORKSPACE, idle = PANEL_IDLE_S, say = stderr,
} = {}) {
  const blocked = panelBlocked({ workspace });
  if (blocked) throw new Error(blocked);

  // The display lock comes after the refusals that cost nothing and before the one that has to be
  // asked last: a run that waited twenty minutes for its turn is refused for the owner's workspace
  // no differently, and the idle check below is only worth anything asked after the wait.
  const lock = holdDisplay(say);

  // Asked here and not a moment earlier. An idle check with a `cargo build` or a compositor call
  // after it is a check of a machine that was empty a minute ago, and the point of it is that the
  // owner is not at the keyboard *now*.
  //
  // Its own stderr is the reason given, rather than a guess at one: `tools/idle-check.py` exits
  // non-zero both for "somebody is there" and for "there are no input devices I can read", and
  // telling the owner they are at a keyboard they are not at is a refusal they cannot act on.
  try {
    execFileSync('python3', [path.join(root, 'tools/idle-check.py'), String(idle)], {
      stdio: ['ignore', 'ignore', 'pipe'],
    });
  } catch (e) {
    const why = String(e.stderr || '').trim().split('\n').pop().replace(/^idle-check: /, '');
    fs.closeSync(lock);
    throw new Error("not taking the owner's screen: "
      + (why || `something used this machine in the last ${idle}s`));
  }

  const monitor = physicalMonitors(monitors())[0];
  const owner = {
    window: activeWindowAddress(),
    cursor: cursorPosition(),
    // The panel's own workspace, not the focused monitor's, for the same reason `panelRefusal` asks
    // it that way: this is what gets put back, and putting back the wrong monitor's workspace is
    // its own way of leaving the owner somewhere they did not choose.
    workspace: monitor.activeWorkspace.id,
  };
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'quill-gate-panel-'));
  const state = { output: null, workspace, lock, closed: false, children: new Set() };
  let disarm = () => {};

  // The workspace goes back as this stage's own step, which puts it before the pointer and the
  // focus: focusing the owner's window is itself a workspace switch, and restoring in the other
  // order would leave them on the bench's.
  const close = () => closeStage({ state, owner, tmp, disarm: () => disarm() }, () => {
    try { focusMonitor(monitor.name); } catch { /* the switch below is the part that matters */ }
    gotoWorkspace(owner.workspace);
  });
  disarm = armTeardown(close);

  // The panel is focused first because `hl.dsp.focus{workspace=N}` acts on whichever monitor holds
  // focus, and on a machine with a stray headless output that is not this one. Best effort: the
  // read-back is the authority, and it asks the panel rather than the compositor at large.
  try { focusMonitor(monitor.name); } catch { /* the read-back will say if it mattered */ }
  gotoWorkspace(workspace);
  let up = null;
  for (let waited = 0; waited < SWITCH_TIMEOUT_MS; waited += POLL_MS) {
    up = monitorWorkspace(monitor.name);
    if (up === workspace) break;
    await sleep(POLL_MS);
  }
  if (up !== workspace) {
    close();
    throw new Error(`workspace ${workspace} would not come up on ${monitor.name} (it is showing `
      + `${up}); refusing to measure a window nobody is looking at`);
  }

  return new Stage({ root, appId, tmp, state, monitor, corner: parkingCorner(monitor), close });
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
    // Stamped here and nowhere else, because "immediately before the exec" is the whole meaning of
    // the number: enumerating the toplevels above costs a process and a Wayland round trip, and a
    // t0 taken before that would put both of them inside the app's cold start. Every launch carries
    // it; only a launch under `--measure` reads it.
    const spell = spellFixture(this.root, this.tmp);
    const env = { ...launchEnv(process.env, spell), QUILL_T0_NS: nowRealtimeNs() };
    const child = spawn(bin, argv, { cwd: this.root, env, stdio: ['ignore', 'pipe', 'pipe'] });
    this.state.children.add(child);
    let stderr = '';
    child.stderr.on('data', (b) => { stderr += b; });
    // Kept rather than ignored because `--measure` says its cold start on stdout, and a bench that
    // could not read it would have to time the launch from outside and measure the wrong thing.
    // A listener from here rather than from the caller: the line is printed at the first frame,
    // which is before this function has a window to hand back. Capped, because every launch is
    // listened to and a long-running one that found something to say every frame would otherwise
    // grow this without limit; the lines worth reading are the first ones.
    let stdout = '';
    child.stdout.on('data', (b) => { if (stdout.length < SAID_MAX) stdout += b; });
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
        return {
          child, toplevel, address: window.address, geometry: { at: window.at, size: window.size },
          said: () => stdout,
        };
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

  /// The second toplevel a launch puts on the stage ([`opensSecondWindow`]): the Export dialog or
  /// the Settings window over the page.
  ///
  /// Found by the process rather than by the class, because the class is shared with every other
  /// Quill and the process is this launch's alone: the one client of ours' pid that is not ours is
  /// the dialog. Its toplevel identifier is then the one in the list under [`DIALOG_APP_ID`] with
  /// the same title, which is the only name `grim -T` answers to ([`toplevels`]).
  ///
  /// `at` is where the compositor put it, in the device pixels of ours' own capture: the offset of
  /// its top left from ours', doubled by the scale every judged shot is taken at. Nothing here
  /// places the window — Hyprland centres it against the window it is transient for, and that
  /// placement is what the `export/dialog` rule measures.
  async dialogOver(ours) {
    for (let waited = 0; waited < MAP_TIMEOUT_MS; waited += POLL_MS) {
      await sleep(POLL_MS);
      const client = clients().find((c) => c.pid === ours.child.pid && c.address !== ours.address);
      if (!client) continue;
      const toplevel = toplevels().find((t) => t.appId === DIALOG_APP_ID && t.title === client.title);
      if (!toplevel) continue;
      return {
        address: client.address,
        appId: DIALOG_APP_ID,
        toplevel,
        at: [
          (client.at[0] - ours.geometry.at[0]) * SCALE,
          (client.at[1] - ours.geometry.at[1]) * SCALE,
        ],
      };
    }
    throw new Error(`the launch opened no second window within ${MAP_TIMEOUT_MS / 1000}s, and the state`
      + ` names a dialog or a Settings pane: the shot would be the page with nothing over it`);
  }

  kill(child) {
    if (!child) return;
    this.state.children.delete(child);
    try { child.kill('SIGTERM'); } catch { /* already gone */ }
  }

  /// Puts keyboard focus on `address`, and answers with whether it took.
  ///
  /// The pointer comes over first: focus follows the mouse on this desktop, and a pointer still on
  /// the owner's panel takes focus straight back off whatever was focused here. Read back rather
  /// than assumed, and tried more than once, because the compositor answers the dispatch before it
  /// has finished acting on it — and because the whole point of asking is that a bench about to
  /// write real keys to `/dev/uinput` must not take the owner's word for where they will land. A
  /// shot asks for its own reason: an unfocused Quill paints the ghost caret, which is a judged
  /// state of its own and so is never a shot that merely looks wrong — and for its parking window,
  /// because focus that never left the owner is the arrangement `shoot`'s contract forbids.
  /// `appId` is the class the window is expected under, which is ours' unless the window asked for
  /// is the dialog over it ([`DIALOG_APP_ID`]).
  async focused(address, appId = this.appId) {
    movePointer(this.corner);
    for (let tries = 0; tries < FOCUS_TRIES; tries += 1) {
      focusWindow(address);
      if (this.holds(address, appId)) return true;
      await sleep(POLL_MS);
    }
    return false;
  }

  /// Whether keyboard focus is still on `address`, and on a window of ours.
  ///
  /// The one question a bench asks between chunks, so it is one `hyprctl` call and no dispatch. The
  /// class is checked as well as the address because the address alone would be satisfied by an
  /// address the compositor has since given to something else.
  holds(address, appId = this.appId) {
    const active = activeWindow();
    return active.address === address && active.class === appId;
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
  ///
  /// It refuses rather than writes when focus never took, when two captures never agreed, when the
  /// window was captured at the wrong size, or when a determined state that draws a caret came back
  /// with a ghosted one. `judge` is the only caller today; the last check is here rather than there
  /// so that a caller which does not exist yet inherits it, because a frame ours had not woken into
  /// is the wrong frame to measure as much as it is the wrong frame to judge.
  ///
  /// Every one of them refuses by throwing, which `judge`'s own `main` turns into `refused (...)`
  /// and exit 3 — the ending `docs/agents/gate.md` § Blind judging gives a run that judged nothing.
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
        // ours hold focus — `no_initial_focus` keeps it from taking any as it maps. Read back, as
        // ours is below: a dispatch the compositor loses leaves focus where it was, on the owner's
        // window, which is the one arrangement the contract above forbids. The shot would very likely still be right — ours is unfocused either way — but
        // "very likely" is not something the Gate can vouch for, so it refuses instead (#186).
        parked = await this.launch(bin, quillArgv(this.root, PARKING));
        if (!(await this.focused(parked.address))) {
          throw new Error(`keyboard focus never took on the parking window (${parked.address}); the shot would be taken with the owner's window still focused`);
        }
      }

      ours = await this.launch(bin, argv);
      // A state that opens a dialog is two toplevels rather than one: the page, and the surface the
      // compositor places over it. Both are waited for here, because everything below — the focus
      // read-back, the shutter, the frame that is written — is about the pair rather than about
      // ours alone. Read out of `argv` for the reason [`wantsLitCaret`] is: the flags a state names
      // are what the state is, and a second channel saying the same thing could disagree with them.
      const over = opensSecondWindow(argv) ? await this.dialogOver(ours) : null;
      // Read back rather than dispatched and hoped for. The compositor answers the dispatch before
      // it has finished acting on it, and an unfocused Quill still paints — it paints the ghost
      // caret the `unfocused` state is judged on. `steady()` cannot catch that: it proves two
      // captures agree, and a page with nothing on it but a ghost is agreed from its first frame,
      // so the shot that loses the race is the stillest one. #166 lost `page/empty` this way, at
      // 0.3 alpha, while `page/light` and `page/narrow` won — their text layout cost enough frames
      // for the activation notify to land.
      //
      // The window focus is read back on is the dialog where there is one: presenting the
      // transient gives it the keyboard, so insisting on ours' own address would reject the
      // arrangement the app asks for. The Editor under it draws the ghost caret by rights, which
      // is why such a state names `--nocaret` and asks for no lit bar below.
      const keyboard = over ?? ours;
      if (active && !(await this.focused(keyboard.address, keyboard.appId))) {
        throw new Error(`keyboard focus never took on ours (${keyboard.address}); the shot would be ghosted`);
      }
      // Settled, captured, and then asked whether the caret in it is lit — and settled and captured
      // again while it is not. The read-back above closed the compositor's half of this race (#186)
      // and could not close the app's: the activation repaint is frames behind the `wl_keyboard`
      // enter, and the frame it is behind is a still one. Each pass is a fresh `SETTLE_MS` and a
      // fresh pair of captures, so a repaint that landed between them is picked up here rather than
      // waited for by a longer sleep.
      const lit = wantsLitCaret(argv, { active });
      let png = null;
      for (let attempt = 0; attempt < LIT_TRIES; attempt += 1) {
        await sleep(SETTLE_MS);
        png = await this.steady(ours.toplevel.id);
        // The dialog's own buffer over the page's, which is the one frame a writer sees: each
        // toplevel is captured on its own because `grim -T` reads a toplevel's own buffer, and the
        // region capture that would take both at once is the one the research ruled out
        // ([`overlaid`]). Steadied on its own too — a dialog still laying itself out is a moving
        // window whether or not the page under it has stopped.
        if (over) png = overlaid(png, await this.steady(over.toplevel.id), over.at);
        if (!lit || carriesAccent(png)) break;
        png = null;
      }
      if (png === null) {
        throw new Error(`the caret came back ghosted in ${LIT_TRIES} settled captures: no pixel of the accent `
          + `${ACCENT_HEX} is in the frame, so ours had not painted itself active by the shutter and the `
          + 'shot would be judged on the ghost');
      }
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
  w: 1440, h: 900, theme: 'light', font: 'duo', step: 5, focus: 'off', chrome: 'on',
  typewriter: false, nocaret: false, text: null, caret: null, select: null, scroll: 0,
};
