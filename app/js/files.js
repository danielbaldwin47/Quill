/* File handling: Library sidebar, autosave, open/save, drag & drop. Owner: files piece.
 *
 * Two locations:
 *   device  — documents kept in this browser (localStorage). Always available.
 *   folder  — a real folder on disk via the File System Access API; files are
 *             read and written in place, so the manuscript lives where you put it.
 *
 * Naming follows iA Writer: a document with no explicit file name takes its name
 * from its first line, live. Saving to disk fixes that name into a real file.
 */
(function () {
  'use strict';
  const W = window.Writer;
  const KEY = 'quill.lib';          // library state (device docs, prefs)
  const DOCKEY = 'quill.doc';       // crash-safety mirror of the open document
  const EXT = /\.(md|markdown|mdown|txt|text)$/i;
  const DEFAULT_EXT = '.md';
  const MAX_FILES = 600, MAX_READ = 250, BIG = 400000;

  const state = {
    open: false, loc: 'device', sort: 'mtime', width: 368,
    device: [], files: [], openId: null, collapsed: {}, q: '',
    dirName: '', perm: 'prompt', dir: null, scanning: false,
  };
  let els = {}, loading = false, saveT = 0, diskT = 0, listT = 0, tickT = 0;
  let status = 'saved', lastSave = 0, undoDel = null, undoT = 0;

  // ---------- tiny helpers ----------
  const $ = (s, r) => (r || document).querySelector(s);
  const esc = (s) => String(s).replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]));
  const uid = () => Date.now().toString(36) + Math.random().toString(36).slice(2, 7);
  const words = (t) => (t.match(/[\p{L}\p{N}'’]+/gu) || []).length;
  // Counting every document's words on every autosave is the one place this
  // file could get expensive; the count is cached against the exact string.
  function wordsOf(d) { if (d._wt !== d.text) { d._wt = d.text; d._w = words(d.text || ''); } return d._w; }
  const nfmt = (n) => n.toLocaleString();

  function deriveTitle(text) {
    const line = (text || '').split('\n').find((l) => l.trim());
    if (!line) return '';
    return line.replace(/^\s*#{1,6}\s+/, '').replace(/^\s*[-*+>]\s+/, '')
      .replace(/[*_`~\[\]]/g, '').replace(/\s+/g, ' ').trim().slice(0, 80);
  }
  function safeName(s) { return s.replace(/[\\/:*?"<>|\n\r\t]/g, '').replace(/^\.+/, '').trim() || 'Untitled'; }
  function dispName(d) { return d.name || (safeName(deriveTitle(d.text) || 'Untitled') + DEFAULT_EXT); }
  function baseName(n) { return n.replace(EXT, ''); }
  function excerpt(text, n) {
    return (text || '').replace(/^\s*#{1,6}\s+/gm, '').replace(/[*_`~]/g, '')
      .replace(/\s+/g, ' ').trim().slice(0, n || 180);
  }
  function fmtDate(t) {
    if (!t) return '';
    const d = new Date(t), now = new Date();
    const same = (a, b) => a.getFullYear() === b.getFullYear() && a.getMonth() === b.getMonth() && a.getDate() === b.getDate();
    if (same(d, now)) return d.toLocaleTimeString([], { hour: 'numeric', minute: '2-digit' });
    const y = new Date(now.getTime() - 864e5);
    if (same(d, y)) return 'Yesterday';
    if (now - d < 6 * 864e5) return d.toLocaleDateString([], { weekday: 'short' });
    if (d.getFullYear() === now.getFullYear()) return d.toLocaleDateString([], { day: 'numeric', month: 'short' });
    return d.toLocaleDateString([], { day: 'numeric', month: 'short', year: '2-digit' });
  }
  function ago(t) {
    const s = Math.max(0, (Date.now() - t) / 1000);
    if (s < 8) return 'just now';
    if (s < 60) return Math.round(s) + 's ago';
    if (s < 3600) return Math.round(s / 60) + ' min ago';
    if (s < 86400) return Math.round(s / 3600) + ' h ago';
    return fmtDate(t);
  }

  // ---------- icons ----------
  const I = {
    panel: '<svg viewBox="0 0 16 16" width="15" height="15" aria-hidden="true"><rect x="1.6" y="2.6" width="12.8" height="10.8" rx="2.2" fill="none" stroke="currentColor" stroke-width="1.2"/><line x1="6.4" y1="2.6" x2="6.4" y2="13.4" stroke="currentColor" stroke-width="1.2"/></svg>',
    doc: '<svg viewBox="0 0 14 17" width="13" height="16" aria-hidden="true"><path d="M1.1 1.6a1 1 0 0 1 1-1h6L13 5.2v10.2a1 1 0 0 1-1 1H2.1a1 1 0 0 1-1-1z" fill="none" stroke="currentColor" stroke-width="1.1"/><path d="M8.1.9v3.4a1 1 0 0 0 1 1h3.5" fill="none" stroke="currentColor" stroke-width="1.1"/></svg>',
    folder: '<svg viewBox="0 0 16 14" width="15" height="13" aria-hidden="true"><path d="M.7 3.1a1.6 1.6 0 0 1 1.6-1.6h3.3l1.5 1.7h7.2a1.6 1.6 0 0 1 1.6 1.6v6.5a1.6 1.6 0 0 1-1.6 1.6H2.3A1.6 1.6 0 0 1 .7 11.3z" fill="currentColor"/></svg>',
    chev: '<svg viewBox="0 0 12 12" width="11" height="11" aria-hidden="true"><path d="M3 4.6 6 7.6l3-3" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"/></svg>',
    plus: '<svg viewBox="0 0 16 16" width="15" height="15" aria-hidden="true"><path d="M8 3v10M3 8h10" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"/></svg>',
    search: '<svg viewBox="0 0 14 14" width="12" height="12" aria-hidden="true"><circle cx="6" cy="6" r="4.3" fill="none" stroke="currentColor" stroke-width="1.3"/><path d="M9.2 9.2 12.6 12.6" stroke="currentColor" stroke-width="1.3" stroke-linecap="round"/></svg>',
    cloudoff: '<svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true"><path d="M4.4 12.5h6.9a3 3 0 0 0 .4-6 4 4 0 0 0-7.5-1.2 3.1 3.1 0 0 0 .2 7.2z" fill="none" stroke="currentColor" stroke-width="1.2"/></svg>',
  };

  // ---------- persistence ----------
  function load() {
    let s = {};
    try { s = JSON.parse(localStorage.getItem(KEY) || '{}'); } catch (e) {}
    for (const k of ['open', 'loc', 'sort', 'width', 'openId', 'dirName']) if (s[k] !== undefined) state[k] = s[k];
    state.collapsed = s.collapsed || {};
    state.device = Array.isArray(s.device) ? s.device : [];
    if (state.loc === 'folder' && !s.dirName) state.loc = 'device';
  }
  function persist() {
    try {
      localStorage.setItem(KEY, JSON.stringify({
        open: state.open, loc: state.loc, sort: state.sort, width: state.width,
        openId: state.openId, collapsed: state.collapsed, dirName: state.dirName,
        device: state.device,
      }));
      if (status === 'error') setStatus('saved');
    } catch (e) { setStatus('error'); }
  }
  // IndexedDB: the only place a directory handle can live across reloads.
  function idb() {
    return new Promise((res, rej) => {
      const r = indexedDB.open('quill', 1);
      r.onupgradeneeded = () => r.result.createObjectStore('kv');
      r.onsuccess = () => res(r.result); r.onerror = () => rej(r.error);
    });
  }
  async function idbSet(k, v) {
    const db = await idb();
    return new Promise((res, rej) => { const t = db.transaction('kv', 'readwrite'); t.objectStore('kv').put(v, k); t.oncomplete = () => res(); t.onerror = () => rej(t.error); });
  }
  async function idbGet(k) {
    const db = await idb();
    return new Promise((res, rej) => { const t = db.transaction('kv', 'readonly'); const q = t.objectStore('kv').get(k); q.onsuccess = () => res(q.result); q.onerror = () => rej(q.error); });
  }

  // ---------- document model ----------
  const docs = () => (state.loc === 'folder' ? state.files : state.device);
  const current = () => docs().find((d) => d.id === state.openId) || null;

  function sortKey(d) { return state.sort === 'name' ? dispName(d).toLowerCase() : state.sort === 'words' ? -wordsOf(d) : -(d.mtime || 0); }
  function cmp(a, b) { const x = sortKey(a), y = sortKey(b); return x < y ? -1 : x > y ? 1 : 0; }

  function entries() {
    const q = state.q.trim().toLowerCase();
    const all = docs();
    if (q) {
      const hits = [];
      for (const d of all) {
        const name = dispName(d).toLowerCase();
        const ni = name.indexOf(q);
        const text = d.text || '';
        const ti = text.toLowerCase().indexOf(q);
        if (ni < 0 && ti < 0) continue;
        hits.push({ type: 'file', doc: d, rank: ni >= 0 ? ni : 1000 + ti, snip: ti >= 0 ? snippet(text, ti, q.length) : null });
      }
      hits.sort((a, b) => a.rank - b.rank || cmp(a.doc, b.doc));
      return hits;
    }
    const roots = [], groups = new Map();
    for (const d of all) {
      if (d.folder) { if (!groups.has(d.folder)) groups.set(d.folder, []); groups.get(d.folder).push(d); }
      else roots.push({ type: 'file', doc: d });
    }
    const out = roots.slice();
    for (const [name, list] of groups) { list.sort(cmp); out.push({ type: 'folder', name, docs: list }); }
    out.sort((a, b) => {
      const ka = a.type === 'file' ? sortKey(a.doc) : gkey(a.docs), kb = b.type === 'file' ? sortKey(b.doc) : gkey(b.docs);
      return ka < kb ? -1 : ka > kb ? 1 : 0;
    });
    return out;
  }
  function gkey(list) { let best = null; for (const d of list) { const k = sortKey(d); if (best === null || k < best) best = k; } return best; }
  function snippet(text, i, len) {
    const flat = text.replace(/\s+/g, ' ');
    let j = 0, k = 0;   // map index in text -> index in flat
    for (; j < i && k < flat.length; j++) { if (/\s/.test(text[j]) && /\s/.test(text[j + 1] || '')) continue; k++; }
    const from = Math.max(0, k - 40);
    return { text: flat.slice(from, from + 150), at: k - from, len };
  }

  // ---------- library DOM ----------
  function build() {
    const aside = document.createElement('aside');
    aside.id = 'library';
    aside.setAttribute('aria-label', 'Library');
    aside.innerHTML = `
      <div class="lib-head">
        <button class="lib-btn" id="lib-hide" title="Hide Library (⇧⌘L)" aria-label="Hide Library">${I.panel}</button>
        <button class="lib-loc" id="lib-loc" aria-haspopup="menu"><span class="fold">${I.folder}</span><span class="nm"></span><span class="cv">${I.chev}</span></button>
        <span class="lib-sp"></span>
        <button class="lib-btn" id="lib-new" title="New Document (⌘N)" aria-label="New Document">${I.plus}</button>
      </div>
      <div class="lib-find">
        <span class="mag">${I.search}</span>
        <input id="lib-q" type="search" placeholder="Search documents" autocomplete="off" spellcheck="false" aria-label="Search documents">
      </div>
      <div class="lib-sort">
        <button id="lib-sortb" aria-haspopup="menu"><span class="nm"></span><span class="cv">${I.chev}</span></button>
        <span class="lib-sp"></span>
        <span class="lib-count"></span>
      </div>
      <div class="lib-list" id="lib-list" role="listbox" tabindex="-1"></div>
      <div class="lib-foot">
        <span class="lib-status" id="lib-status"><i class="dot"></i><span class="txt"></span></span>
        <span class="lib-sp"></span>
        <span class="lib-where" id="lib-where"></span>
      </div>
      <div class="lib-grip" id="lib-grip" title="Drag to resize"></div>`;
    document.getElementById('app').appendChild(aside);
    els = {
      aside, list: $('#lib-list', aside), loc: $('#lib-loc', aside), q: $('#lib-q', aside),
      sortb: $('#lib-sortb', aside), count: $('.lib-count', aside), status: $('#lib-status', aside),
      where: $('#lib-where', aside), grip: $('#lib-grip', aside),
    };

    // Toggle in the top bar. The chrome piece owns that bar, so we only add a
    // button to its left group — and stand down entirely if it already has one.
    if (!document.querySelector('#chrome-top [data-cmd="library.toggle"]')) {
      const left = document.querySelector('#chrome-top .side.left') || document.querySelector('#chrome-top .bar');
      const tog = document.createElement('button');
      tog.className = 'icon lib-toggle'; tog.id = 'lib-toggle'; tog.innerHTML = I.panel;
      tog.dataset.cmd = 'library.toggle';
      tog.title = 'Show Library (\u21e7\u2318L)'; tog.setAttribute('aria-label', 'Show Library');
      if (left && left.classList.contains('left')) left.appendChild(tog);
      else if (left) left.insertBefore(tog, left.firstChild);
      else { tog.classList.add('floating'); document.body.appendChild(tog); }
      tog.addEventListener('click', (e) => { e.stopPropagation(); toggle(); });
    }

    $('#lib-hide', aside).addEventListener('click', () => toggle(false));
    $('#lib-new', aside).addEventListener('click', () => newDoc());
    els.loc.addEventListener('click', locMenu);
    els.sortb.addEventListener('click', sortMenu);
    els.q.addEventListener('input', () => { state.q = els.q.value; renderList(); });
    els.q.addEventListener('keydown', (e) => {
      if (e.key === 'Escape') { e.stopPropagation(); if (state.q) { els.q.value = ''; state.q = ''; renderList(); } else W.el.input.focus(); }
      else if (e.key === 'Enter' || e.key === 'ArrowDown') { e.preventDefault(); const f = els.list.querySelector('.lib-row.file'); if (f) openDoc(f.dataset.id, true); }
    });
    els.list.addEventListener('click', onListClick);
    els.list.addEventListener('dblclick', (e) => { const r = e.target.closest('.lib-row.file'); if (r) startRename(r); });
    els.list.addEventListener('contextmenu', (e) => { const r = e.target.closest('.lib-row.file'); if (!r) return; e.preventDefault(); rowMenu(r, e.clientX, e.clientY); });
    els.list.addEventListener('keydown', listKeys);
    grip();
    dropZone();
  }

  function toggle(v) {
    state.open = v === undefined ? !state.open : !!v;
    document.documentElement.dataset.library = state.open ? 'open' : 'closed';
    if (state.open) { renderAll(); requestAnimationFrame(() => { W.emit('resize'); }); }
    persist();
  }

  function renderAll() { renderHead(); renderList(); renderStatus(); }

  function renderHead() {
    const nm = state.loc === 'folder' ? (state.dirName || 'Folder') : 'Library';
    $('.nm', els.loc).textContent = nm;
    els.loc.title = state.loc === 'folder' ? 'Folder on disk — click to switch' : 'Documents kept in this browser — click to connect a folder';
    els.aside.dataset.loc = state.loc;
    $('.nm', els.sortb).textContent = state.sort === 'name' ? 'Sort by Name' : state.sort === 'words' ? 'Sort by Length' : 'Sort by Date';
    const list = docs();
    const tw = list.reduce((a, d) => a + wordsOf(d), 0);
    els.count.textContent = list.length ? `${list.length} ${list.length === 1 ? 'document' : 'documents'} · ${nfmt(tw)} words` : '';
    els.where.textContent = state.loc === 'folder' ? (state.perm === 'granted' ? 'On disk' : 'Reconnect') : 'In this browser';
    els.where.className = 'lib-where' + (state.loc === 'folder' && state.perm !== 'granted' ? ' warn' : '');
  }

  function rowHTML(d, indent, snip) {
    const nm = dispName(d);
    const sel = d.id === state.openId;
    const meta = state.sort === 'words' ? nfmt(wordsOf(d)) + ' w' : fmtDate(d.mtime);
    let ex;
    if (snip) {
      const a = esc(snip.text.slice(0, snip.at)), b = esc(snip.text.slice(snip.at, snip.at + snip.len)), c = esc(snip.text.slice(snip.at + snip.len));
      ex = a + '<mark>' + b + '</mark>' + c;
    } else ex = esc(excerpt(d.text));
    return `<div class="lib-row file${sel ? ' sel' : ''}${indent ? ' in' : ''}" data-id="${esc(d.id)}" role="option" aria-selected="${sel}" tabindex="-1">`
      + `<span class="ic">${I.doc}</span><span class="nm" title="${esc(nm)}">${esc(nm)}</span><span class="dt">${esc(meta)}</span>`
      + (ex ? `<span class="ex">${ex}</span>` : `<span class="ex empty">Empty document</span>`) + `</div>`;
  }

  function renderList() {
    const es = entries();
    let html = '';
    for (const e of es) {
      if (e.type === 'file') html += rowHTML(e.doc, false, e.snip);
      else {
        const col = !!state.collapsed[e.name];
        html += `<div class="lib-row folder${col ? ' col' : ''}" data-folder="${esc(e.name)}"><span class="ic">${I.folder}</span><span class="nm">${esc(e.name)}</span><span class="dt">${e.docs.length}</span><span class="cv">${I.chev}</span></div>`;
        if (!col) for (const d of e.docs) html += rowHTML(d, true, null);
      }
    }
    if (!html) html = emptyHTML();
    els.list.innerHTML = html;
    renderHead();
  }

  function emptyHTML() {
    if (state.q) return `<div class="lib-empty"><p>No document matches <b>${esc(state.q)}</b>.</p></div>`;
    if (state.loc === 'folder' && state.perm !== 'granted')
      return `<div class="lib-empty"><p>Quill needs your permission to read <b>${esc(state.dirName)}</b> again.</p><p><button class="lib-cta" data-act="reconnect">Reconnect folder</button></p></div>`;
    if (state.loc === 'folder' && state.scanning) return `<div class="lib-empty"><p>Reading ${esc(state.dirName)}…</p></div>`;
    return `<div class="lib-empty"><p>${state.loc === 'folder' ? 'No .md or .txt files in this folder yet.' : 'No documents yet.'}</p>`
      + `<p><button class="lib-cta" data-act="new">New document</button></p>`
      + (state.loc === 'device' && window.showDirectoryPicker ? `<p class="hint">or <button class="lib-link" data-act="pick">open a folder on your disk…</button></p>` : '') + `</div>`;
  }

  function updateRow(d) {
    const row = els.list.querySelector(`.lib-row.file[data-id="${CSS.escape(String(d.id))}"]`);
    if (!row) { renderList(); return; }
    const nm = dispName(d);
    const n = $('.nm', row); if (n.textContent !== nm) { n.textContent = nm; n.title = nm; }
    const dt = $('.dt', row); const meta = state.sort === 'words' ? nfmt(wordsOf(d)) + ' w' : fmtDate(d.mtime);
    if (dt.textContent !== meta) dt.textContent = meta;
    const ex = $('.ex', row); const t = excerpt(d.text);
    if (ex) { ex.textContent = t || 'Empty document'; ex.className = 'ex' + (t ? '' : ' empty'); }
    renderHead();
  }

  function onListClick(e) {
    const act = e.target.closest('[data-act]');
    if (act) {
      const a = act.dataset.act;
      if (a === 'new') newDoc(); else if (a === 'pick') pickFolder(); else if (a === 'reconnect') reconnect();
      return;
    }
    const f = e.target.closest('.lib-row.folder');
    if (f) { const n = f.dataset.folder; state.collapsed[n] = !state.collapsed[n]; persist(); renderList(); return; }
    const r = e.target.closest('.lib-row.file');
    if (r) openDoc(r.dataset.id, true);
  }
  function listKeys(e) {
    const rows = [...els.list.querySelectorAll('.lib-row.file')];
    const i = rows.findIndex((r) => r.dataset.id === state.openId);
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault();
      const n = rows[Math.max(0, Math.min(rows.length - 1, i + (e.key === 'ArrowDown' ? 1 : -1)))];
      if (n) openDoc(n.dataset.id, false);
    } else if (e.key === 'Enter') { e.preventDefault(); W.el.input.focus(); }
    else if (e.key === 'Escape') { e.preventDefault(); W.el.input.focus(); }
  }

  // ---------- menus ----------
  function menu(x, y, items) {
    closeMenu();
    const m = document.createElement('div');
    m.className = 'lib-menu'; m.setAttribute('role', 'menu');
    m.innerHTML = items.map((it) => it.sep ? '<hr>'
      : `<button role="menuitem"${it.on ? ' class="on"' : ''}${it.danger ? ' data-danger="1"' : ''}><span>${esc(it.label)}</span>${it.hint ? `<kbd>${esc(it.hint)}</kbd>` : ''}</button>`).join('');
    document.getElementById('overlays').appendChild(m);
    const w = m.offsetWidth, h = m.offsetHeight;
    m.style.left = Math.max(8, Math.min(x, innerWidth - w - 8)) + 'px';
    m.style.top = (y + h > innerHeight - 8 ? Math.max(8, y - h) : y) + 'px';
    const real = items.filter((i) => !i.sep);
    [...m.querySelectorAll('button')].forEach((b, idx) => {
      b.addEventListener('mousedown', (e) => e.preventDefault());
      b.addEventListener('click', () => { const it = real[idx]; closeMenu(); if (it && it.run) it.run(); });
    });
    // Close on any press outside the menu — but the press that lands ON an item
    // must survive long enough to become a click, or nothing would ever run.
    const onDown = (e) => { if (!m.contains(e.target)) closeMenu(); };
    const onKey = (e) => { if (e.key === 'Escape') { e.preventDefault(); closeMenu(); W.el.input.focus(); } };
    setTimeout(() => {
      document.addEventListener('mousedown', onDown, true);
      document.addEventListener('keydown', onKey, true);
      menu.cleanup = () => { document.removeEventListener('mousedown', onDown, true); document.removeEventListener('keydown', onKey, true); };
    }, 0);
    menu.el = m;
  }
  function closeMenu() {
    if (menu.cleanup) { menu.cleanup(); menu.cleanup = null; }
    if (menu.el) { menu.el.remove(); menu.el = null; }
  }

  function locMenu() {
    const r = els.loc.getBoundingClientRect();
    const items = [{ label: 'This Browser', on: state.loc === 'device', run: () => switchLoc('device') }];
    if (state.dirName) items.push({ label: state.dirName, on: state.loc === 'folder', run: () => switchLoc('folder') });
    items.push({ sep: true });
    if (window.showDirectoryPicker) items.push({ label: 'Open Folder…', hint: '⇧⌘O', run: pickFolder });
    items.push({ label: 'Open File…', hint: '⌘O', run: openFile });
    if (state.loc === 'device') items.push({ label: 'New Collection…', run: newFolder });
    menu(r.left, r.bottom + 4, items);
  }
  function sortMenu() {
    const r = els.sortb.getBoundingClientRect();
    menu(r.left, r.bottom + 4, [
      { label: 'Sort by Date', on: state.sort === 'mtime', run: () => setSort('mtime') },
      { label: 'Sort by Name', on: state.sort === 'name', run: () => setSort('name') },
      { label: 'Sort by Length', on: state.sort === 'words', run: () => setSort('words') },
    ]);
  }
  function rowMenu(row, x, y) {
    const d = docs().find((v) => String(v.id) === row.dataset.id); if (!d) return;
    menu(x, y, [
      { label: 'Open', run: () => openDoc(d.id, true) },
      { label: 'Rename…', run: () => startRename(row) },
      { label: 'Duplicate', run: () => duplicate(d) },
      { label: 'Download .md', run: () => download(d) },
      { sep: true },
      { label: 'Delete', danger: true, run: () => del(d) },
    ]);
  }
  function setSort(s) { state.sort = s; persist(); renderList(); }

  // ---------- open / create / delete ----------
  function flushAll() { flushSave(); if (state.loc === 'folder') return writeDisk(); }
  async function openDoc(id, focusEditor) {
    const d = docs().find((v) => String(v.id) === String(id));
    if (!d || d.id === state.openId) { if (focusEditor) W.el.input.focus(); return; }
    await flushAll();
    if (d.text == null && d.handle) { try { d.text = await (await d.handle.getFile()).text(); } catch (e) { d.text = ''; } }
    state.openId = d.id;
    loading = true;
    W.setText(d.text || '', { caret: d.caret || 0, source: 'open' });
    loading = false;
    setStatus('saved');
    persist(); mirrorDoc(); renderList(); refreshRefs();
    if (focusEditor) W.el.input.focus();
  }
  function newDoc(folder) {
    flushAll();
    const d = { id: uid(), name: null, folder: folder || null, mtime: Date.now(), text: '' };
    if (state.loc === 'folder') { d.pending = true; }
    docs().unshift(d);
    state.openId = d.id; loading = true; W.setText('', { source: 'new' }); loading = false;
    persist(); renderList(); W.el.input.focus(); setStatus('saved');
  }
  function newFolder() {
    const n = prompt('Name this collection'); if (!n) return;
    newDoc(safeName(n));
  }
  function duplicate(d) {
    const copy = { id: uid(), name: d.name ? uniqueName(baseName(d.name) + ' copy' + (d.name.match(EXT) || [DEFAULT_EXT])[0]) : null, folder: d.folder, mtime: Date.now(), text: d.text || '' };
    if (state.loc === 'folder') { copy.pending = true; state.files.unshift(copy); } else state.device.unshift(copy);
    persist(); renderList();
    openDoc(copy.id, false);
  }
  function uniqueName(n) {
    const taken = new Set(docs().map((d) => dispName(d).toLowerCase()));
    if (!taken.has(n.toLowerCase())) return n;
    const b = baseName(n), e = (n.match(EXT) || [DEFAULT_EXT])[0];
    for (let i = 2; i < 999; i++) if (!taken.has((b + ' ' + i + e).toLowerCase())) return b + ' ' + i + e;
    return n;
  }
  async function del(d) {
    const list = docs(); const i = list.indexOf(d); if (i < 0) return;
    list.splice(i, 1);
    if (state.loc === 'folder' && state.dir && d.handle) {
      try { await (d.folder ? (await state.dir.getDirectoryHandle(d.folder)) : state.dir).removeEntry(d.name); } catch (e) { flash('Could not delete ' + d.name); }
      undoDel = null;
    } else { undoDel = { doc: d, at: i }; clearTimeout(undoT); undoT = setTimeout(() => { undoDel = null; renderStatus(); }, 12000); }
    if (state.openId === d.id) {
      const n = list[Math.min(i, list.length - 1)];
      state.openId = null;
      if (n) openDoc(n.id, false); else newDoc();
    }
    persist(); renderList(); renderStatus();
  }
  function undelete() {
    if (!undoDel) return;
    state.device.splice(undoDel.at, 0, undoDel.doc); undoDel = null;
    persist(); renderList(); renderStatus();
  }

  function startRename(row) {
    const d = docs().find((v) => String(v.id) === row.dataset.id); if (!d) return;
    const span = $('.nm', row); if (!span || row.querySelector('input')) return;
    const inp = document.createElement('input');
    inp.className = 'lib-rename'; inp.value = dispName(d); inp.spellcheck = false;
    span.replaceWith(inp);
    const b = inp.value.replace(EXT, '').length; inp.focus(); inp.setSelectionRange(0, b);
    const done = (ok) => {
      if (inp.__done) return; inp.__done = true;
      const v = safeName(inp.value.trim());
      if (ok && v && v !== dispName(d)) rename(d, EXT.test(v) ? v : v + DEFAULT_EXT); else renderList();
    };
    inp.addEventListener('keydown', (e) => { e.stopPropagation(); if (e.key === 'Enter') { e.preventDefault(); done(true); } else if (e.key === 'Escape') { e.preventDefault(); done(false); } });
    inp.addEventListener('blur', () => done(true));
  }
  async function rename(d, name) {
    name = uniqueName(name);
    if (state.loc === 'folder' && state.dir && d.handle) {
      try {
        const dir = d.folder ? await state.dir.getDirectoryHandle(d.folder) : state.dir;
        const h = await dir.getFileHandle(name, { create: true });
        const w = await h.createWritable(); await w.write(d.text || ''); await w.close();
        await dir.removeEntry(d.name);
        d.handle = h; d.id = (d.folder ? d.folder + '/' : '') + name;
      } catch (e) { flash('Could not rename'); renderList(); return; }
    }
    d.name = name; d.mtime = Date.now();
    if (state.openId && !docs().some((x) => x.id === state.openId)) state.openId = d.id;
    persist(); renderList(); mirrorDoc(); refreshRefs();
  }

  // ---------- saving ----------
  function setStatus(s) { status = s; if (s === 'saved') lastSave = Date.now(); renderStatus(); }
  function renderStatus() {
    if (!els.status) return;
    const t = $('.txt', els.status);
    let msg = '';
    if (undoDel) { els.status.dataset.k = 'undo'; t.innerHTML = `Deleted <b>${esc(dispName(undoDel.doc))}</b> · <button class="lib-link" id="lib-undo">Undo</button>`; const u = $('#lib-undo', els.status); if (u) u.onclick = undelete; return; }
    if (status === 'saving') msg = 'Saving…';
    else if (status === 'error') msg = 'Storage full — export your work';
    else if (status === 'dirty') msg = 'Unsaved changes';
    else msg = lastSave ? 'Saved ' + ago(lastSave) : 'All changes saved';
    els.status.dataset.k = status;
    t.textContent = msg;
  }
  function flash(m) { if (!els.status) return; const t = $('.txt', els.status); els.status.dataset.k = 'error'; t.textContent = m; clearTimeout(flash.t); flash.t = setTimeout(renderStatus, 4000); }

  function mirrorDoc() {
    const d = current();
    try {
      localStorage.setItem(DOCKEY, d ? (d.text || '') : W.getText());
      localStorage.setItem(DOCKEY + '.sel', String(W.selection().start));
      localStorage.setItem(DOCKEY + '.id', d ? String(d.id) : '');
    } catch (e) {}
    const el = document.querySelector('#doc-title .name, #doc-title, .doc-title .name');
    if (el) el.textContent = d ? dispName(d) : (deriveTitle(W.getText()) || 'Untitled');
  }

  function flushSave() {
    clearTimeout(saveT); saveT = 0;
    const d = current(); if (!d) return;
    d.text = W.getText(); d.caret = W.selection().start; d.mtime = Date.now();
    persist(); mirrorDoc();
    if (state.loc !== 'folder') setStatus('saved');
  }
  function onChange() {
    if (loading) return;
    const d = current(); if (!d) return;
    setStatus('saving');
    clearTimeout(saveT);
    saveT = setTimeout(() => { flushSave(); updateRow(d); refreshRefs(); }, 400);
    if (state.loc === 'folder') { clearTimeout(diskT); diskT = setTimeout(writeDisk, 1400); }
  }
  async function writeDisk() {
    clearTimeout(diskT); diskT = 0;
    const d = current(); if (!d || state.loc !== 'folder') return;
    d.text = W.getText(); d.mtime = Date.now();
    try {
      if (!d.handle) {
        if (!state.dir) { setStatus('dirty'); return; }
        const dir = d.folder ? await state.dir.getDirectoryHandle(d.folder, { create: true }) : state.dir;
        const name = uniqueName(d.name || safeName(deriveTitle(d.text) || 'Untitled') + DEFAULT_EXT);
        d.handle = await dir.getFileHandle(name, { create: true });
        d.name = name; d.id = (d.folder ? d.folder + '/' : '') + name; d.pending = false;
        renderList();
      }
      const w = await d.handle.createWritable();
      await w.write(d.text); await w.close();
      setStatus('saved'); mirrorDoc();
    } catch (e) { setStatus('dirty'); flash('Could not write to disk'); }
  }

  // ---------- folder (File System Access) ----------
  async function pickFolder() {
    if (!window.showDirectoryPicker) { flash('This browser cannot open folders — use Open File'); return; }
    let dir;
    try { dir = await window.showDirectoryPicker({ mode: 'readwrite', id: 'quill-library' }); } catch (e) { return; }
    state.dir = dir; state.dirName = dir.name; state.perm = 'granted'; state.loc = 'folder';
    try { await idbSet('dir', dir); } catch (e) {}
    persist(); toggle(true); await scan();
    const first = state.files[0]; if (first) openDoc(first.id, false); else newDoc();
  }
  async function reconnect() {
    if (!state.dir) { try { state.dir = await idbGet('dir'); } catch (e) {} }
    if (!state.dir) return pickFolder();
    try {
      const p = await state.dir.requestPermission({ mode: 'readwrite' });
      if (p !== 'granted') return;
      state.perm = 'granted'; await scan();
    } catch (e) { pickFolder(); }
  }
  function switchLoc(loc) {
    if (loc === state.loc) return;
    flushAll();
    state.loc = loc; state.q = ''; if (els.q) els.q.value = '';
    persist(); renderAll();
    if (loc === 'folder') { if (state.perm !== 'granted') reconnect(); else if (!state.files.length) scan(); }
    const list = docs();
    if (list.length) openDoc((list.find((d) => d.id === state.openId) || list[0]).id, false);
    refreshRefs();
  }
  async function scan() {
    if (!state.dir) return;
    state.scanning = true; renderList();
    const found = [];
    async function walk(dir, folder, depth) {
      let n = 0;
      for await (const [name, h] of dir.entries()) {
        if (name.startsWith('.')) continue;
        if (h.kind === 'directory') { if (depth < 1) await walk(h, name, depth + 1); continue; }
        if (!EXT.test(name)) continue;
        if (found.length >= MAX_FILES) return;
        found.push({ id: (folder ? folder + '/' : '') + name, name, folder, handle: h, mtime: 0, text: null });
        if (++n > MAX_FILES) return;
      }
    }
    try { await walk(state.dir, null, 0); } catch (e) { state.perm = 'prompt'; state.scanning = false; renderList(); return; }
    await Promise.all(found.map(async (d) => { try { const f = await d.handle.getFile(); d.mtime = f.lastModified; d.size = f.size; } catch (e) {} }));
    found.sort((a, b) => b.mtime - a.mtime);
    await Promise.all(found.slice(0, MAX_READ).map(async (d) => {
      try { d.text = d.size > BIG ? '' : await (await d.handle.getFile()).text(); } catch (e) { d.text = ''; }
    }));
    // keep unsaved new documents that have not hit disk yet
    const pending = state.files.filter((d) => d.pending);
    state.files = pending.concat(found);
    state.scanning = false;
    if (!state.files.some((d) => d.id === state.openId)) state.openId = state.files[0] ? state.files[0].id : null;
    renderList(); refreshRefs();
  }

  // ---------- open / download single files ----------
  async function openFile() {
    if (window.showOpenFilePicker) {
      let hs;
      try { hs = await window.showOpenFilePicker({ multiple: true, types: [{ description: 'Text & Markdown', accept: { 'text/markdown': ['.md', '.markdown', '.mdown'], 'text/plain': ['.txt', '.text'] } }] }); } catch (e) { return; }
      let first = null;
      for (const h of hs) { const d = await adopt(h); if (d && !first) first = d; }
      if (first) { toggle(true); openDoc(first.id, true); }
      return;
    }
    const i = document.createElement('input');
    i.type = 'file'; i.accept = '.md,.markdown,.txt,.text'; i.multiple = true;
    i.onchange = async () => { let first = null; for (const f of i.files) { const d = await adoptFile(f); if (!first) first = d; } if (first) { toggle(true); openDoc(first.id, true); } };
    i.click();
  }
  async function adopt(handle) {
    try {
      const f = await handle.getFile();
      const d = { id: uid(), name: safeName(f.name), folder: null, mtime: f.lastModified, text: await f.text(), handle };
      state.loc = 'device';
      const dup = state.device.find((x) => x.name === d.name && x.text === d.text);
      if (dup) return dup;
      state.device.unshift(d); persist(); renderList(); return d;
    } catch (e) { return null; }
  }
  async function adoptFile(f) {
    const d = { id: uid(), name: safeName(f.name), folder: null, mtime: f.lastModified, text: await f.text() };
    state.loc = 'device'; state.device.unshift(d); persist(); renderList(); return d;
  }
  function download(d) {
    const doc = d || current(); if (!doc) return;
    const a = document.createElement('a');
    a.href = URL.createObjectURL(new Blob([doc.text || ''], { type: 'text/markdown;charset=utf-8' }));
    a.download = dispName(doc); a.click();
    setTimeout(() => URL.revokeObjectURL(a.href), 4000);
  }
  async function saveAs() {
    const d = current(); if (!d) return;
    if (!window.showSaveFilePicker) return download(d);
    try {
      const h = await window.showSaveFilePicker({ suggestedName: dispName(d), types: [{ description: 'Markdown', accept: { 'text/markdown': ['.md', '.markdown'], 'text/plain': ['.txt'] } }] });
      const w = await h.createWritable(); await w.write(W.getText()); await w.close();
      d.handle = h; d.name = h.name; d.mtime = Date.now();
      persist(); renderList(); mirrorDoc(); setStatus('saved');
    } catch (e) {}
  }
  async function saveNow() {
    const d = current(); if (!d) return;
    flushSave();
    if (state.loc === 'folder') return writeDisk();
    if (d.handle) {
      try { const w = await d.handle.createWritable(); await w.write(d.text); await w.close(); setStatus('saved'); return; } catch (e) {}
    }
    if (window.showSaveFilePicker) return saveAs();
    download(d);
  }

  // ---------- drag & drop ----------
  function dropZone() {
    const veil = document.createElement('div');
    veil.id = 'lib-drop'; veil.innerHTML = '<div class="box">Drop text files to add them to your Library</div>';
    document.getElementById('overlays').appendChild(veil);
    let depth = 0;
    const has = (e) => e.dataTransfer && [...(e.dataTransfer.types || [])].includes('Files');
    document.addEventListener('dragenter', (e) => { if (!has(e)) return; e.preventDefault(); if (++depth === 1) veil.classList.add('on'); });
    document.addEventListener('dragover', (e) => { if (has(e)) { e.preventDefault(); e.dataTransfer.dropEffect = 'copy'; } });
    document.addEventListener('dragleave', (e) => { if (!has(e)) return; if (--depth <= 0) { depth = 0; veil.classList.remove('on'); } });
    document.addEventListener('drop', async (e) => {
      if (!has(e)) return;
      e.preventDefault(); depth = 0; veil.classList.remove('on');
      const items = [...(e.dataTransfer.items || [])];
      let first = null, n = 0;
      for (const it of items) {
        if (it.kind !== 'file') continue;
        let d = null;
        if (it.getAsFileSystemHandle) {
          try { const h = await it.getAsFileSystemHandle(); if (h && h.kind === 'file' && EXT.test(h.name)) d = await adopt(h); } catch (err) {}
        }
        if (!d) { const f = it.getAsFile(); if (f && (EXT.test(f.name) || /^text\//.test(f.type))) d = await adoptFile(f); }
        if (d) { n++; if (!first) first = d; }
      }
      if (first) { toggle(true); openDoc(first.id, true); flash(n === 1 ? 'Added ' + dispName(first) : 'Added ' + n + ' documents'); }
    });
  }

  // ---------- resize grip ----------
  function grip() {
    let x0 = 0, w0 = 0;
    els.grip.addEventListener('pointerdown', (e) => {
      e.preventDefault(); x0 = e.clientX; w0 = state.width;
      els.grip.setPointerCapture(e.pointerId);
      document.documentElement.dataset.libResize = 'on';
      const move = (ev) => {
        state.width = Math.max(260, Math.min(560, w0 + (ev.clientX - x0)));
        document.documentElement.style.setProperty('--lib-w', state.width + 'px');
      };
      const up = () => {
        els.grip.removeEventListener('pointermove', move); els.grip.removeEventListener('pointerup', up);
        delete document.documentElement.dataset.libResize; persist(); W.emit('resize');
      };
      els.grip.addEventListener('pointermove', move); els.grip.addEventListener('pointerup', up);
    });
  }

  // ---------- document references (lines that name another document) ----------
  let refNames = new Map(), refSig = '';
  function refreshRefs() {
    const m = new Map();
    let sig = '';
    for (const d of docs()) {
      if (d.id === state.openId) continue;
      const n = dispName(d);
      m.set(n.toLowerCase(), d.id); m.set(baseName(n).toLowerCase(), d.id);
      sig += n + '\n';
    }
    refNames = m;
    // A full re-render is the only way to restyle those lines, so it happens
    // only when the names themselves changed — never on the keystroke path.
    if (sig !== refSig) { refSig = sig; W.render(true); }
  }
  function refIdFor(line) {
    const t = line.trim(); if (!t || t.length > 120 || /\s{2,}/.test(t)) return null;
    return refNames.get(t.toLowerCase()) || null;
  }

  // ---------- boot ----------
  W.on('boot', () => {
    load();
    build();
    document.documentElement.dataset.library = state.open ? 'open' : 'closed';
    document.documentElement.style.setProperty('--lib-w', state.width + 'px');

    // migrate a pre-library document, or start with one empty document
    if (!state.device.length && state.loc === 'device') {
      let t = '';
      try { t = localStorage.getItem(DOCKEY) || ''; } catch (e) {}
      state.device.push({ id: uid(), name: null, folder: null, mtime: Date.now(), text: t });
      state.openId = state.device[0].id;
    }
    if (!docs().some((d) => d.id === state.openId)) state.openId = (docs()[0] || {}).id || null;
    const d = current();
    if (d) { W.el.input.value = d.text || ''; const c = Math.min(d.caret || 0, (d.text || '').length); W.el.input.setSelectionRange(c, c); }

    W.on('change', onChange);
    W.on('selection', () => { const c = current(); if (c && !loading) c.caret = W.selection().start; });
    W.on('ready', () => { mirrorDoc(); if (state.open) renderAll(); refreshRefs(); });
    window.addEventListener('beforeunload', () => { flushSave(); });
    document.addEventListener('visibilitychange', () => { if (document.visibilityState === 'hidden') flushAll(); });
    tickT = setInterval(() => { if (state.open && status === 'saved' && !undoDel) renderStatus(); }, 20000);

    // restore a previously granted folder without prompting
    if (state.loc === 'folder') {
      idbGet('dir').then(async (dir) => {
        if (!dir) { state.loc = 'device'; renderAll(); return; }
        state.dir = dir;
        let p = 'prompt';
        try { p = await dir.queryPermission({ mode: 'readwrite' }); } catch (e) {}
        state.perm = p;
        if (p === 'granted') await scan(); else renderAll();
      }).catch(() => {});
    }

    // mark lines that name another document
    W.addDecorator((i, text) => (refNames.size && refIdFor(text)) ? [{ from: 0, to: text.length, cls: 'docref' }] : null);
    // The textarea sits on top of the mirror, so the line under the pointer is
    // read from the mirror element the click passed through.
    W.el.input.addEventListener('click', (e) => {
      if (!(e.metaKey || e.ctrlKey)) return;
      const line = document.elementsFromPoint(e.clientX, e.clientY).find((el) => el.classList && el.classList.contains('line'));
      if (!line) return;
      const id = refIdFor(W.lines()[+line.dataset.i] || '');
      if (id) { e.preventDefault(); openDoc(id, true); }
    });

    W.registerCommand('library.toggle', { title: 'Show / Hide Library', keys: ['Mod+Shift+L', 'Ctrl+Meta+S'], run: () => toggle() });
    W.registerCommand('library.search', { title: 'Find a Document…', keys: ['Mod+Shift+O'], run: () => { toggle(true); els.q.focus(); els.q.select(); } });
    W.registerCommand('file.new', { title: 'New Document', keys: ['Mod+N', 'Alt+Mod+N'], run: () => { toggle(true); newDoc(); } });
    W.registerCommand('file.open', { title: 'Open File…', keys: 'Mod+O', run: openFile });
    W.registerCommand('file.openFolder', { title: 'Open Folder…', run: pickFolder });
    W.registerCommand('file.save', { title: 'Save', keys: 'Mod+S', run: saveNow });
    W.registerCommand('file.saveAs', { title: 'Save As…', keys: 'Mod+Shift+S', run: saveAs });
    W.registerCommand('file.duplicate', { title: 'Duplicate Document', run: () => { const d = current(); if (d) duplicate(d); } });
    W.registerCommand('file.rename', { title: 'Rename Document…', keys: 'F2', run: () => { toggle(true); const r = els.list.querySelector('.lib-row.file.sel'); if (r) startRename(r); } });
    W.registerCommand('file.delete', { title: 'Delete Document', run: () => { const d = current(); if (d && confirm('Delete “' + dispName(d) + '”?')) del(d); } });
    W.registerCommand('file.export', { title: 'Download .md', run: () => download(null) });
    W.registerCommand('file.next', { title: 'Next Document', keys: ['Ctrl+Meta+ArrowDown', 'Mod+Alt+ArrowDown'], run: () => step(1) });
    W.registerCommand('file.prev', { title: 'Previous Document', keys: ['Ctrl+Meta+ArrowUp', 'Mod+Alt+ArrowUp'], run: () => step(-1) });
    W.registerCommand('file.follow', { title: 'Open Linked Document', keys: 'Mod+Enter', run: () => {
      const pos = W.offsetToPos(W.selection().start); const id = refIdFor(W.lines()[pos.line] || '');
      if (id) openDoc(id, true);
    } });
  });
  function step(n) {
    const rows = docs().slice().sort(cmp);
    const i = rows.findIndex((d) => d.id === state.openId);
    const t = rows[Math.max(0, Math.min(rows.length - 1, i + n))];
    if (t) openDoc(t.id, true);
  }

  // expose a little of it for tooling / tests
  W.Files = { toggle, state, openDoc, newDoc, scan, dispName, refreshRefs };
})();
