// Ad-hoc inventory shots: each menu open and the Settings window, light and dark, on the Gate's
// own headless stage. Not a judged shot; nothing under the repo is written.
import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { openStage, APP_ID, classPattern } from '/home/diggle/repos/quill/tools/harness.mjs';

const ROOT = '/home/diggle/repos/quill';
const OUT = '/tmp/quill-settings-inventory';
const BIN = path.join(ROOT, 'target/release/quill');
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const hypr = (args) => execFileSync('hyprctl', args, { encoding: 'utf8' });
const lua = (src) => hypr(['repl', src]);

const only = process.argv[2] || 'all';
const stage = await openStage({ root: ROOT });
try {
  const output = stage.output;
  const workspace = stage.workspace;
  const rules = path.join(OUT, 'rules.lua');
  fs.writeFileSync(rules, [
    'QUILL_GATE_RULES = QUILL_GATE_RULES or {}',
    `local C = "${classPattern(APP_ID)}"`,
    'local function rule(t) t.match = { class = C } QUILL_GATE_RULES[#QUILL_GATE_RULES + 1] = hl.window_rule(t) end',
    `rule({ name = "quill-inv-workspace", workspace = "${workspace} silent" })`,
    'rule({ name = "quill-inv-float", float = true })',
    'local function dialog(t) t.match = { class = "^(quill)$" } QUILL_GATE_RULES[#QUILL_GATE_RULES + 1] = hl.window_rule(t) end',
    `dialog({ name = "quill-inv-dialog-workspace", workspace = "${workspace} silent" })`,
    'dialog({ name = "quill-inv-dialog-float", float = true })',
    'dialog({ name = "quill-inv-dialog-decoration", no_anim = true, border_size = 0, rounding = 0, no_shadow = true, no_blur = true, no_dim = true, opacity = "1.0 1.0", tag = "-default-opacity" })',
    'rule({ name = "quill-inv-decoration", no_anim = true, border_size = 0, rounding = 0, no_shadow = true, no_blur = true, no_dim = true, opacity = "1.0 1.0", tag = "-default-opacity" })',
    '',
  ].join('\n'));
  lua(`dofile("${rules}") return "rules"`);

  const base = (theme) => ['--deterministic', '--w', '1440', '--h', '900', '--theme', theme, '--font', 'mono',
    '--library', 'shots/oracle/library', '--text', 'ref/short.md',
    '--settings', path.join(OUT, `settings-${theme}.toml`)];
  const grim = (name) => {
    const file = path.join(OUT, `${name}.png`);
    execFileSync('grim', ['-o', output, file]);
    console.log(file);
  };

  for (const theme of ['light', 'dark']) {
    if (only === 'all' || only === 'menus') {
      for (const menu of ['document', 'view', 'stats', 'palette']) {
        const ours = await stage.launch(BIN, [...base(theme), '--menu', menu]);
        await sleep(1500);
        grim(`menu-${menu}-${theme}`);
        stage.kill(ours.child);
        await sleep(400);
      }
    }
    if (only === 'all' || only === 'settings') {
      const ours = await stage.launch(BIN, base(theme));
      await sleep(1200);
      const active = JSON.parse(hypr(['activewindow', '-j']));
      console.log('active', active.address, 'ours', ours.address);
      let sent = '';
      for (const form of [
        `hl.dsp.send_shortcut({ mods = "CTRL", key = "comma", window = "address:${ours.address}" })`,
        `hl.dsp.send_shortcut("CTRL", "comma", "address:${ours.address}")`,
      ]) {
        try { sent = lua(`return hl.dispatch(${form})`); console.log('form ok:', form, sent.trim()); break; } catch (e) { console.log('form failed:', form, String(e.stderr || e.message).slice(0, 200)); }
      }
      await sleep(1500);
      let clients = JSON.parse(hypr(['clients', '-j'])).filter((c) => c.pid === ours.child.pid);
      if (clients.length < 2 && active.address === ours.address) {
        console.log('falling back to wtype, ours holds focus');
        execFileSync('wtype', ['-M', 'ctrl', '-k', 'comma', '-m', 'ctrl']);
        await sleep(1500);
        clients = JSON.parse(hypr(['clients', '-j'])).filter((c) => c.pid === ours.child.pid);
      }
      console.log('windows of ours:', clients.map((c) => `${c.title} ${c.size} ws${c.workspace.id}`).join(' | '));
      const wrong = clients.find((c) => c.workspace.id !== workspace);
      if (wrong) { console.log('ABORT: a window is off the stage', wrong.title, wrong.workspace.id); stage.kill(ours.child); break; }
      grim(`settings-${theme}-1-top`);
      const dlg = clients.find((c) => c.address !== ours.address);
      const key = (k) => lua(`return hl.dispatch(hl.dsp.send_shortcut({ mods = "", key = "${k}", window = "address:${dlg.address}" }))`);
      key('Tab'); await sleep(300);
      for (const name of ['2-middle', '3-bottom']) {
        key('Page_Down'); await sleep(900);
        grim(`settings-${theme}-${name}`);
      }
      stage.kill(ours.child);
      await sleep(400);
    }
  }
} finally {
  stage.close();
}
