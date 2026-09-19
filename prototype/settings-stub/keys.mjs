// THROWAWAY (#464): drives the search list with keys on the headless stage and prints what the stub
// logged (QUILL_STUB_LOG) and what reached settings.toml after each key.
//   node prototype/settings-stub/keys.mjs stock|palette [key ...]
// Default keys: h e a d Down space Return Tab space Escape
import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(HERE, '../..');
const { openStage, APP_ID, DIALOG_APP_ID, classPattern } = await import(path.join(ROOT, 'tools/harness.mjs'));
const TMP = fs.mkdtempSync(path.join(os.tmpdir(), 'quill-stub-keys-'));
const BIN = path.join(ROOT, 'target/release/quill');
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const hypr = (args) => execFileSync('hyprctl', args, { encoding: 'utf8' });
const lua = (src) => hypr(['repl', src]);

const mode = process.argv[2] || 'palette';
const keys = process.argv.length > 3 ? process.argv.slice(3) : ['h', 'e', 'a', 'd', 'Down', 'space', 'Return', 'Tab', 'space', 'Escape'];
const file = path.join(TMP, 'settings.toml');
fs.writeFileSync(file, 'theme = "light"\n');
const wrote = () => fs.readFileSync(file, 'utf8').split('\n')
  .filter((l) => /center_headings|number_headings|header|paper|margin|title_page|footer/.test(l)).join(' ; ');

const stage = await openStage({ root: ROOT });
try {
  const { workspace } = stage;
  const rules = path.join(TMP, 'rules.lua');
  fs.writeFileSync(rules, [
    'QUILL_GATE_RULES = QUILL_GATE_RULES or {}',
    'local function add(c, t) t.match = { class = c } QUILL_GATE_RULES[#QUILL_GATE_RULES + 1] = hl.window_rule(t) end',
    `for _, c in ipairs({ "${classPattern(APP_ID)}", "^(${DIALOG_APP_ID})$" }) do`,
    `  add(c, { name = "quill-stub-ws-" .. c, workspace = "${workspace} silent" })`,
    '  add(c, { name = "quill-stub-float-" .. c, float = true })',
    '  add(c, { name = "quill-stub-plain-" .. c, no_anim = true, border_size = 0, rounding = 0, no_shadow = true })',
    'end',
    '',
  ].join('\n'));
  lua(`dofile("${rules}") return "rules"`);
  Object.assign(process.env, { QUILL_STUB_OPEN: '1', QUILL_STUB_LOG: '1', QUILL_STUB_KEYS: mode });
  // FIELD=1 starts with the keyboard in the search field, not on the sidebar's first row.
  if (process.env.FIELD) process.env.QUILL_STUB_QUERY = '';
  const ours = await stage.launch(BIN, ['--deterministic', '--w', '1120', '--h', '720', '--theme', 'light',
    '--font', 'mono', '--text', 'ref/short.md', '--settings', file]);
  let said = '';
  ours.child.stderr.on('data', (b) => { said += b; });
  let dlg = null;
  for (let waited = 0; waited < 6000 && !dlg; waited += 200) {
    await sleep(200);
    dlg = JSON.parse(hypr(['clients', '-j'])).find((c) => c.pid === ours.child.pid && c.address !== ours.address);
  }
  if (!dlg || dlg.workspace.id !== workspace) throw new Error('no Settings window on the stage');
  console.log('focused:', await stage.focused(dlg.address, DIALOG_APP_ID));
  await sleep(500);
  said = '';
  for (const key of keys) {
    lua(`return hl.dispatch(hl.dsp.send_shortcut({ mods = "", key = "${key}", window = "address:${dlg.address}" }))`);
    await sleep(700);
    const log = said.split('\n').filter((l) => l.startsWith('stub:')).map((l) => l.slice(6)).join(' | ');
    said = '';
    console.log(`${key.padEnd(7)} -> ${log || '(nothing logged)'}   [file: ${wrote() || 'no export/template key yet'}]`);
  }
  if (process.env.SHOT) {
    const [x, y] = dlg.at; const [w, h] = dlg.size;
    execFileSync('grim', ['-g', `${x},${y} ${w}x${h}`, process.env.SHOT]);
  }
  stage.kill(ours.child);
} finally {
  stage.close();
  fs.rmSync(TMP, { recursive: true, force: true });
}
