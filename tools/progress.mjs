// Build progress/index.html from progress/state.json + progress/rounds/*.json (+ progress/latency.json)
// Round file: { piece, round, winner: 'ours'|'theirs'|'tie', gap, verdict, oursShot, theirsShot, builderNote, at, latency? }
import fs from 'node:fs'; import path from 'node:path'; import { chromium } from 'playwright-core'; import { thumb } from './thumb.mjs';
// Who a round was judged against, read from the round rather than assumed, so a page showing both
// eras captions each one correctly. What a round is lives in tools/rounds.mjs, which tools/gate
// judge writes them through — the page and the judge cannot disagree about it, and the page does
// not have to import the judging command to ask.
import { opponentName as opponentOf } from './rounds.mjs';
const state = JSON.parse(fs.readFileSync('progress/state.json', 'utf8'));
const rounds = fs.readdirSync('progress/rounds').filter(f => f.endsWith('.json')).map(f => JSON.parse(fs.readFileSync('progress/rounds/' + f, 'utf8'))).sort((a, b) => (a.piece.localeCompare(b.piece)) || a.round - b.round);
const latency = fs.existsSync('progress/latency.json') ? JSON.parse(fs.readFileSync('progress/latency.json', 'utf8')) : null;
const esc = s => String(s ?? '').replace(/[&<>"]/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]));
const fmtT = iso => { try { return new Date(iso).toLocaleString('en-GB', { timeZone: 'Europe/Stockholm', hour: '2-digit', minute: '2-digit', day: '2-digit', month: 'short' }); } catch { return iso; } };
const browser = await chromium.launch({ executablePath: '/usr/bin/chromium', headless: true });
const thumbCache = new Map();
async function th(file) { if (!file || !fs.existsSync(file) || !/\.(png|jpe?g|webp)$/i.test(file)) return null; try { if (!thumbCache.has(file)) thumbCache.set(file, await thumb(file, 640, browser)); return thumbCache.get(file); } catch (e) { console.error('thumb failed', file, e.message); return null; } }
const pieces = [];
for (const p of state.pieces) {
  const rs = rounds.filter(r => r.piece === p.id);
  const last = rs[rs.length - 1];
  const won = last && last.winner === 'ours';
  pieces.push({ ...p, rounds: rs, last, won, ours: last ? await th(last.oursShot) : null, theirs: last ? await th(last.theirsShot) : null });
}
await browser.close();
const wonCount = pieces.filter(p => p.won).length;
const totalRounds = rounds.length;
const now = new Date().toISOString();
const card = p => `
<section class="piece ${p.won ? 'won' : p.last ? 'losing' : 'pending'}" id="piece-${p.id}">
  <header class="piece-head">
    <div>
      <h2>${esc(p.title)}</h2>
      <p class="what">${esc(p.what)}</p>
    </div>
    <div class="status">
      <span class="pill">${p.won ? 'Picked blind' : p.last ? 'Still loses' : 'Not judged yet'}</span>
      <span class="rounds" title="one dot per round: filled = ours picked">${p.rounds.map(r => `<i class="${r.winner === 'ours' ? 'w' : r.winner === 'tie' ? 't' : 'l'}" title="round ${r.round}: ${r.winner}"></i>`).join('')}${p.rounds.length ? `<b>${p.rounds.length}</b>` : ''}</span>
    </div>
  </header>
  ${p.last ? `
  <div class="pair">
    <figure class="${p.last.winner === 'ours' ? 'picked' : ''}"><div class="img">${p.ours ? `<img src="${p.ours}" alt="ours">` : '<div class="noimg">no screenshot</div>'}</div><figcaption>Ours${p.last.winner === 'ours' ? ' · critic picked this' : ''}</figcaption></figure>
    <figure class="${p.last.winner === 'theirs' ? 'picked' : ''}"><div class="img">${p.theirs ? `<img src="${p.theirs}" alt="${esc(opponentOf(p.last))}">` : '<div class="noimg">no reference</div>'}</div><figcaption>${esc(opponentOf(p.last))}${p.last.winner === 'theirs' ? ' · critic picked this' : ''}</figcaption></figure>
  </div>
  <div class="verdict">
    <p class="gap"><span class="lbl">Biggest gap</span>${esc(p.last.gap)}</p>
    <p class="quote">${esc(p.last.verdict)}</p>
    ${p.last.builderNote ? `<p class="note"><span class="lbl">Builder</span>${esc(p.last.builderNote)}</p>` : ''}
    <p class="meta">Round ${p.last.round} · ${fmtT(p.last.at)}</p>
  </div>` : `<p class="waiting">Waiting for the first round.</p>`}
</section>`;
const lat = latency ? `
<section class="latency">
  <h2>Measured latency</h2>
  <div class="tiles">
    <div class="tile"><span class="lbl">Keystroke → paint p50</span><b>${latency.keystroke_p50 ?? '—'}<small>ms</small></b><span class="sub">p99 ${latency.keystroke_p99 ?? '—'} ms</span></div>
    <div class="tile"><span class="lbl">Startup → editor ready</span><b>${latency.startup_ready ?? '—'}<small>ms</small></b><span class="sub">${latency.startup_note ? esc(latency.startup_note) : ''}</span></div>
    <div class="tile"><span class="lbl">Bar (iA / best native)</span><b>${latency.bar_keystroke ?? '—'}<small>ms</small></b><span class="sub">${esc(latency.bar_note || '')}</span></div>
  </div>
  <p class="meta">${esc(latency.method || '')} · ${fmtT(latency.at)}</p>
</section>` : '';
const html = `<title>Quill Gauntlet</title>
<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Newsreader:ital,opsz,wght@0,6..72,400;0,6..72,500;1,6..72,400&family=Instrument+Sans:wght@400;500;600&family=IBM+Plex+Mono:wght@400;500&display=swap">
<style>
:root{--paper:#f4f5f3;--ink:#171a1c;--ink-2:#5b6266;--ink-3:#9aa1a5;--line:#dfe2e0;--card:#ffffff;--accent:#1e7fe0;--win:#1f8f5f;--win-bg:#e6f4ec;--lose:#b9552a;--lose-bg:#f8ebe4;--pend:#8a9094;--pend-bg:#eceeed;--mono:"IBM Plex Mono",ui-monospace,Menlo,monospace;--sans:"Instrument Sans",-apple-system,"Segoe UI",system-ui,sans-serif;--serif:"Newsreader",Georgia,"Times New Roman",serif}
@media (prefers-color-scheme:dark){:root:not([data-theme="light"]){--paper:#131516;--ink:#e6e8e7;--ink-2:#a8afb2;--ink-3:#6c7377;--line:#2a2e30;--card:#1b1e20;--accent:#4c9cf0;--win:#3fbf86;--win-bg:#15281f;--lose:#e0764a;--lose-bg:#2c1d15;--pend:#8a9094;--pend-bg:#22262a}}
:root[data-theme="dark"]{--paper:#131516;--ink:#e6e8e7;--ink-2:#a8afb2;--ink-3:#6c7377;--line:#2a2e30;--card:#1b1e20;--accent:#4c9cf0;--win:#3fbf86;--win-bg:#15281f;--lose:#e0764a;--lose-bg:#2c1d15;--pend:#8a9094;--pend-bg:#22262a}
body{margin:0;background:var(--paper);color:var(--ink);font-family:var(--sans);font-size:15px;line-height:1.5}
.wrap{max-width:1180px;margin:0 auto;padding:40px 28px 80px}
.top{display:grid;grid-template-columns:1fr auto;gap:24px;align-items:end;border-bottom:1px solid var(--line);padding-bottom:24px;margin-bottom:28px}
h1{font-family:var(--serif);font-weight:500;font-size:44px;line-height:1.05;margin:0 0 6px;letter-spacing:-.01em;text-wrap:balance}
.sub-h{color:var(--ink-2);margin:0;max-width:60ch}
.score{display:flex;gap:28px;font-variant-numeric:tabular-nums}
.score div{display:flex;flex-direction:column;gap:2px}
.score b{font-family:var(--serif);font-size:40px;font-weight:400;line-height:1}
.score b em{font-style:normal;color:var(--ink-3);font-size:22px}
.lbl{display:block;font-size:11px;letter-spacing:.08em;text-transform:uppercase;color:var(--ink-3);font-weight:600;margin-bottom:4px}
.grid{display:grid;grid-template-columns:repeat(auto-fill,minmax(340px,1fr));gap:18px}
.piece{background:var(--card);border:1px solid var(--line);border-radius:6px;padding:18px 18px 16px;display:flex;flex-direction:column;gap:14px;position:relative}
.piece.won{border-color:color-mix(in srgb,var(--win) 45%,var(--line))}
.piece-head{display:flex;justify-content:space-between;gap:12px;align-items:flex-start}
.piece h2{font-family:var(--serif);font-weight:500;font-size:24px;margin:0;line-height:1.1}
.what{margin:4px 0 0;color:var(--ink-2);font-size:13px}
.status{display:flex;flex-direction:column;align-items:flex-end;gap:8px;flex:none}
.pill{font-size:11px;font-weight:600;letter-spacing:.06em;text-transform:uppercase;padding:4px 8px;border-radius:3px;white-space:nowrap}
.won .pill{background:var(--win-bg);color:var(--win)}.losing .pill{background:var(--lose-bg);color:var(--lose)}.pending .pill{background:var(--pend-bg);color:var(--pend)}
.rounds{display:flex;gap:4px;align-items:center;font-family:var(--mono);font-size:11px;color:var(--ink-3)}
.rounds i{width:8px;height:8px;border-radius:50%;border:1.5px solid var(--lose);display:inline-block}
.rounds i.w{background:var(--win);border-color:var(--win)}.rounds i.t{border-color:var(--pend)}
.rounds b{margin-left:4px;font-weight:500}
.pair{display:grid;grid-template-columns:1fr 1fr;gap:10px}
figure{margin:0}
.img{aspect-ratio:16/10;background:var(--pend-bg);border:1px solid var(--line);border-radius:3px;overflow:hidden}
.img img{width:100%;height:100%;object-fit:cover;object-position:top;display:block}
figure.picked .img{outline:2px solid var(--accent);outline-offset:1px}
figcaption{font-size:11px;color:var(--ink-3);margin-top:5px;font-family:var(--mono)}
figure.picked figcaption{color:var(--accent)}
.noimg{display:grid;place-items:center;height:100%;color:var(--ink-3);font-size:12px}
.verdict{display:flex;flex-direction:column;gap:8px;border-top:1px solid var(--line);padding-top:12px}
.verdict p{margin:0}
.gap{font-weight:500}
.quote{font-family:var(--serif);font-size:16px;line-height:1.4;color:var(--ink-2)}
.note{font-size:13px;color:var(--ink-2)}
.meta{font-family:var(--mono);font-size:11px;color:var(--ink-3)}
.waiting{color:var(--ink-3);margin:0}
.latency{margin:32px 0 0;padding:20px;border:1px solid var(--line);border-radius:6px;background:var(--card)}
.latency h2{font-family:var(--serif);font-weight:500;font-size:24px;margin:0 0 14px}
.tiles{display:grid;grid-template-columns:repeat(auto-fit,minmax(220px,1fr));gap:14px}
.tile{display:flex;flex-direction:column;gap:2px}
.tile b{font-family:var(--serif);font-weight:400;font-size:38px;line-height:1;font-variant-numeric:tabular-nums}
.tile b small{font-size:16px;color:var(--ink-3);margin-left:4px}
.tile .sub{font-size:12px;color:var(--ink-2)}
.log{margin-top:36px}
.log h2{font-family:var(--serif);font-weight:500;font-size:24px;margin:0 0 10px}
.log ol{list-style:none;margin:0;padding:0;display:flex;flex-direction:column;gap:6px}
.log li{display:grid;grid-template-columns:110px 1fr;gap:12px;font-size:14px;border-top:1px solid var(--line);padding:8px 0}
.log time{font-family:var(--mono);font-size:12px;color:var(--ink-3)}
.foot{margin-top:28px;font-family:var(--mono);font-size:11px;color:var(--ink-3)}
@media (max-width:640px){.top{grid-template-columns:1fr}.pair{grid-template-columns:1fr}}
</style>
<div class="wrap">
  <header class="top">
    <div><h1>Quill against iA Writer</h1><p class="sub-h">Nine pieces of a writing environment, each rebuilt until a fresh critic — shown ours and iA Writer's side by side, unlabeled — prefers ours. The critic is instructed to be harsh; every round records its verdict and the single biggest remaining gap.</p></div>
    <div class="score"><div><span class="lbl">Picked blind</span><b>${wonCount}<em>/${pieces.length}</em></b></div><div><span class="lbl">Rounds judged</span><b>${totalRounds}</b></div></div>
  </header>
  <div class="grid">${pieces.map(card).join('')}</div>
  ${lat}
  <section class="log"><h2>Log</h2><ol>${[...state.log, ...rounds.map(r => ({ at: r.at, msg: `${r.piece} · round ${r.round}: critic picked ${r.winner === 'ours' ? 'OURS' : r.winner === 'theirs' ? opponentOf(r) : 'neither'} — ${r.gap}` }))].sort((a, b) => a.at.localeCompare(b.at)).reverse().map(l => `<li><time>${fmtT(l.at)}</time><span>${esc(l.msg)}</span></li>`).join('')}</ol></section>
  <p class="foot">Updated ${fmtT(now)} · Screenshots at 1440×900@2x unless noted · iA Writer reference images from ia.net</p>
</div>`;
fs.writeFileSync('progress/index.html', html);
console.log(`progress: ${wonCount}/${pieces.length} won, ${totalRounds} rounds, ${(html.length / 1024).toFixed(0)}KB`);
