// THROWAWAY (#464): shoots the Settings stub on the Gate's headless stage, never on workspace 1.
//   node prototype/settings-stub/shoot.mjs            every pane and the search list, light and dark
//   node prototype/settings-stub/shoot.mjs light export   one theme, one shot
// The stub opens itself (QUILL_STUB_OPEN) on the pane or query the environment names, so no key is
// sent and a stolen focus costs a caret, not the shot.
import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(HERE, '../..');
const { openStage, APP_ID, DIALOG_APP_ID, classPattern } = await import(path.join(ROOT, 'tools/harness.mjs'));
const OUT = path.join(HERE, 'shots');
const TMP = fs.mkdtempSync(path.join(os.tmpdir(), 'quill-stub-'));
const BIN = path.join(ROOT, 'target/release/quill');
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const hypr = (args) => execFileSync('hyprctl', args, { encoding: 'utf8' });
const lua = (src) => hypr(['repl', src]);

const SHOTS = [
  ['general', { QUILL_STUB_PANE: 'general' }],
  ['library', { QUILL_STUB_PANE: 'library' }],
  ['template', { QUILL_STUB_PANE: 'template' }],
  ['export', { QUILL_STUB_PANE: 'export' }],
  ['export-popup', { QUILL_STUB_PANE: 'export', QUILL_STUB_POPUP: '1' }],
  ['writing', { QUILL_STUB_PANE: 'writing' }],
  ['advanced', { QUILL_STUB_PANE: 'advanced' }],
  ['search-head', { QUILL_STUB_QUERY: 'head' }],
  ['search-l', { QUILL_STUB_QUERY: 'li' }],
];
const themes = process.argv[2] && process.argv[2] !== 'all' ? [process.argv[2]] : ['light', 'dark'];
const only = process.argv[3];

const home = os.homedir();
const settings = (theme) => {
  const file = path.join(TMP, `settings-${theme}.toml`);
  fs.writeFileSync(file, [
    `theme = "${theme}"`,
    '',
    '[library]',
    `locations = ["${home}/Documents/Manuscripts", "${home}/Documents/Notes", "${home}/Dropbox/Letters"]`,
    `pinned = ["${home}/Documents/Manuscripts/The storm/Opening.md", "${home}/Documents/Notes/Harbour lights.md"]`,
    'confirm_move = true',
    '',
    '[shortcuts]',
    '"library.toggle" = ["<Super>l"]',
    '',
  ].join('\n'));
  return file;
};

fs.mkdirSync(OUT, { recursive: true });
const stage = await openStage({ root: ROOT });
try {
  const { output, workspace } = stage;
  const rules = path.join(TMP, 'rules.lua');
  fs.writeFileSync(rules, [
    'QUILL_GATE_RULES = QUILL_GATE_RULES or {}',
    'local function add(c, t) t.match = { class = c } QUILL_GATE_RULES[#QUILL_GATE_RULES + 1] = hl.window_rule(t) end',
    `for _, c in ipairs({ "${classPattern(APP_ID)}", "^(${DIALOG_APP_ID})$" }) do`,
    `  add(c, { name = "quill-stub-ws-" .. c, workspace = "${workspace} silent" })`,
    '  add(c, { name = "quill-stub-float-" .. c, float = true })',
    '  add(c, { name = "quill-stub-plain-" .. c, no_anim = true, border_size = 0, rounding = 0, no_shadow = true, no_blur = true, no_dim = true, opacity = "1.0 1.0", tag = "-default-opacity" })',
    'end',
    '',
  ].join('\n'));
  lua(`dofile("${rules}") return "rules"`);

  for (const theme of themes) {
    for (const [name, env] of SHOTS) {
      if (only && only !== name) continue;
      for (const key of Object.keys(process.env)) if (key.startsWith('QUILL_STUB_')) delete process.env[key];
      Object.assign(process.env, env, { QUILL_STUB_OPEN: '1' });
      const ours = await stage.launch(BIN, ['--deterministic', '--w', '1120', '--h', '720', '--theme', theme,
        '--font', 'mono', '--text', 'ref/short.md', '--settings', settings(theme)]);
      let dlg = null;
      for (let waited = 0; waited < 6000 && !dlg; waited += 200) {
        await sleep(200);
        dlg = JSON.parse(hypr(['clients', '-j'])).find((c) => c.pid === ours.child.pid && c.address !== ours.address);
      }
      if (!dlg) { console.log(`${theme} ${name}: the Settings window never appeared`); stage.kill(ours.child); continue; }
      if (dlg.workspace.id !== workspace) { console.log('ABORT: a window is off the stage', dlg.workspace.id); stage.kill(ours.child); break; }
      const held = await stage.focused(dlg.address, DIALOG_APP_ID);
      await sleep(900);
      const file = path.join(OUT, `${name}-${theme}.png`);
      const [x, y] = dlg.at;
      const [w, h] = dlg.size;
      // The popup is a surface of its own and hangs past the window's right edge, so that shot takes the room for it.
      execFileSync('grim', ['-g', name.includes('popup') ? `${x},${y} ${w + 160}x${h}` : `${x},${y} ${w}x${h}`, file]);
      console.log(`${file} ${w}x${h}${held ? '' : ' (unfocused)'}`);
      stage.kill(ours.child);
      await sleep(400);
    }
  }
} finally {
  stage.close();
  fs.rmSync(TMP, { recursive: true, force: true });
}
