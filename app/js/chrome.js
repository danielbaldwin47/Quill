/* Chrome: the two bars, the menus, the command palette. Owner: chrome piece.
 *
 * The rule this file follows: a writing app should show the writer the name of
 * what they are writing, how much of it there is, and nothing else — until they
 * ask. So there are exactly two bars, three menus and one palette, and while the
 * keys are moving the top bar goes away and the counts stop moving.
 *
 * Contracts with other pieces (do not break):
 *   #doc-title      files.js writes the document name into this element's text.
 *   #chrome-top .bar  files.js inserts its library toggle as the first child.
 *   The stats scan is scheduled on idle, never on the keystroke path (latency).
 */
(function () {
  const W = window.Writer;
  const root = document.documentElement;

  // ---------------------------------------------------------------- icons
  const svg = (w, h, body) =>
    `<svg width="${w}" height="${h}" viewBox="0 0 ${w} ${h}" fill="none" aria-hidden="true">${body}</svg>`;
  const CHEV = svg(9, 9, '<path d="M1.6 3.3 4.5 6.1 7.4 3.3" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round"/>');
  const TICK = svg(10, 8, '<path d="M1 4.2 3.6 6.8 9 1.4" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/>');
  const MAG = svg(13, 13, '<circle cx="5.6" cy="5.6" r="4.1" stroke="currentColor" stroke-width="1.4"/><path d="M8.7 8.7 11.8 11.8" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"/>');
  // The view icon is a little block of text whose lit rows are the rows focus
  // mode keeps lit: off = an even block, sentence = one row, paragraph = three.
  function rowsIcon(mode) {
    const on = mode === 'off' ? [0.5, 0.5, 0.5, 0.5] :
               mode === 'sentence' ? [0.22, 1, 0.22, 0.22] : [0.22, 1, 1, 1];
    const w = [15, 15, 15, 9];
    let b = '';
    for (let i = 0; i < 4; i++) b += `<rect x="0" y="${i * 3.4}" width="${w[i]}" height="1.6" rx=".8" opacity="${on[i]}"/>`;
    return `<span class="icon-rows">${svg(15, 12, b)}</span>`;
  }

  // ------------------------------------------------------------ shortcuts
  const MOD = W.isMac ? '⌘' : 'Ctrl+';
  function keyLabel(keys) {
    let k = Array.isArray(keys) ? keys[0] : keys;
    if (!k) return '';
    const parts = k.split('+');
    const key = parts.pop();
    let out = '';
    if (W.isMac) {
      if (parts.includes('Ctrl')) out += '⌃';
      if (parts.includes('Alt')) out += '⌥';
      if (parts.includes('Shift')) out += '⇧';
      if (parts.includes('Mod') || parts.includes('Meta')) out += '⌘';
    } else {
      if (parts.includes('Mod') || parts.includes('Ctrl')) out += 'Ctrl+';
      if (parts.includes('Alt')) out += 'Alt+';
      if (parts.includes('Shift')) out += 'Shift+';
    }
    const named = { ArrowUp: '↑', ArrowDown: '↓', ArrowLeft: '←', ArrowRight: '→', Enter: '↩', Escape: 'Esc', '=': '+', Space: 'Space' };
    return out + (named[key] || key);
  }

  // ---------------------------------------------------------------- stats
  // 238 wpm: Brysbaert 2019, the meta-analysis of silent reading of English
  // non-fiction. (iA's own bar implies ~200; a writer is better served by the
  // honest number than by a flattering one.)
  const WPM = 238;
  const WORD = /[^\s]+/g;
  const HASLETTER = /[\p{L}\p{N}]/u;
  const SENT = /[.!?…]['"”’)\]]*(\s|$)/gu;

  function count(text) {
    let words = 0;
    const m = text.match(WORD);
    if (m) for (const t of m) if (HASLETTER.test(t)) words++;
    let sentences = 0;
    SENT.lastIndex = 0;
    while (SENT.exec(text)) sentences++;
    // a last, unpunctuated sentence still counts if there is anything in it
    const tail = text.slice(text.search(/[^\s]*$/));
    if (words && !/[.!?…]['"”’)\]]*\s*$/.test(text.trimEnd())) sentences++;
    let paras = 0, inP = false;
    for (const line of text.split('\n')) {
      if (line.trim()) { if (!inP) { paras++; inP = true; } } else inP = false;
    }
    const nospace = text.replace(/\s/g, '').length;
    return { words, chars: text.length, nospace, sentences, paras, seconds: Math.round(words / WPM * 60), tail };
  }
  const n = (v) => v.toLocaleString();
  function readTime(seconds) {
    if (seconds < 45) return 'under a minute';
    const mins = Math.round(seconds / 60);
    if (mins < 60) return mins + ' min';
    const h = Math.floor(mins / 60), m = mins % 60;
    return h + ' h' + (m ? ' ' + m + ' min' : '');
  }
  const FIELDS = [
    { id: 'words', label: 'Words', get: (s) => [n(s.words), s.words === 1 ? 'word' : 'words'] },
    { id: 'chars', label: 'Characters', get: (s) => [n(s.chars), 'characters'] },
    { id: 'nospace', label: 'Characters Without Spaces', get: (s) => [n(s.nospace), 'without spaces'] },
    { id: 'sentences', label: 'Sentences', get: (s) => [n(s.sentences), s.sentences === 1 ? 'sentence' : 'sentences'] },
    { id: 'paras', label: 'Paragraphs', get: (s) => [n(s.paras), s.paras === 1 ? 'paragraph' : 'paragraphs'] },
    { id: 'read', label: 'Reading Time', get: (s) => [readTime(s.seconds), 'read'] },
  ];
  const DEFAULT_FIELDS = 'words,chars,read';
  const shown = () => String(W.settings.stats == null ? DEFAULT_FIELDS : W.settings.stats).split(',').filter(Boolean);

  // ------------------------------------------------------------ menu core
  let openPanel = null;      // {el, close}
  function closePanel(refocus) {
    if (!openPanel) return;
    const p = openPanel; openPanel = null;
    p.el.remove();
    if (p.anchor) p.anchor.setAttribute('aria-expanded', 'false');
    root.dataset.menu = 'off';
    if (refocus !== false && W.el.input) W.el.input.focus();
  }
  W.closeChromePanels = closePanel;

  /* items: {label, keys, cmd, checked, run, flush} | {sep:true} | {head:'…'} */
  function menu(items, opts) {
    const wasOpen = openPanel && openPanel.key === opts.key;
    closePanel(false);
    if (wasOpen) { if (W.el.input) W.el.input.focus(); return; }
    const el = document.createElement('div');
    el.className = 'menu' + (opts.place === 'above' ? ' up' : '');
    el.setAttribute('role', 'menu');
    const rows = [];
    for (const it of items) {
      if (it.sep) { const d = document.createElement('div'); d.className = 'sep'; el.appendChild(d); continue; }
      if (it.head) { const d = document.createElement('div'); d.className = 'head'; d.textContent = it.head; el.appendChild(d); continue; }
      const cmd = it.cmd ? W.commands().find((c) => c.id === it.cmd) : null;
      if (it.cmd && !cmd) continue;                     // a piece that is not loaded
      const r = document.createElement('div');
      r.className = 'row' + (it.flush || it.checked === undefined ? ' flush' : '');
      r.setAttribute('role', it.checked === undefined ? 'menuitem' : 'menuitemcheckbox');
      if (it.checked !== undefined) r.setAttribute('aria-checked', it.checked ? 'true' : 'false');
      const label = it.label || (cmd && cmd.title) || '';
      const keys = it.keys !== undefined ? it.keys : (cmd && cmd.keys);
      r.innerHTML = (it.checked === undefined ? '' : `<span class="tick">${TICK}</span>`) +
        `<span class="label"></span>` + (keys ? `<span class="keys">${keyLabel(keys)}</span>` : '');
      r.querySelector('.label').textContent = label;
      r.addEventListener('mouseenter', () => { for (const q of rows) q.classList.remove('on'); r.classList.add('on'); });
      r.addEventListener('mouseleave', () => r.classList.remove('on'));
      r.addEventListener('click', () => { closePanel(); if (it.run) it.run(); else if (it.cmd) W.run(it.cmd); });
      rows.push(r);
      el.appendChild(r);
    }
    document.getElementById('overlays').appendChild(el);
    root.dataset.menu = 'on';

    // position: under (or over) the anchor, right- or left-aligned, always inside
    const m = el.getBoundingClientRect();
    const pad = 6;
    let x = opts.x, y;
    if (opts.align === 'right') x = opts.x - m.width;
    x = Math.max(pad, Math.min(x, innerWidth - m.width - pad));
    y = opts.place === 'above' ? opts.y - m.height : opts.y;
    y = Math.max(pad, Math.min(y, innerHeight - m.height - pad));
    el.style.left = Math.round(x) + 'px';
    el.style.top = Math.round(y) + 'px';
    if (opts.anchor) opts.anchor.setAttribute('aria-expanded', 'true');
    openPanel = { el, key: opts.key, anchor: opts.anchor, rows, sel: -1 };
    return el;
  }

  // keyboard for an open menu
  function menuKeys(e) {
    if (!openPanel || !openPanel.rows) return false;
    const p = openPanel, rows = p.rows;
    const move = (d) => {
      p.sel = (p.sel + d + rows.length) % rows.length;
      rows.forEach((r, i) => r.classList.toggle('on', i === p.sel));
    };
    if (e.key === 'Escape') { closePanel(); return true; }
    if (e.key === 'ArrowDown') { move(1); return true; }
    if (e.key === 'ArrowUp') { move(-1); return true; }
    if (e.key === 'Enter' || e.key === ' ') { const r = rows[p.sel]; if (r) r.click(); else closePanel(); return true; }
    return false;
  }

  // ------------------------------------------------------------------ boot
  W.on('boot', () => {
    const top = document.getElementById('chrome-top');
    const bottom = document.getElementById('chrome-bottom');
    const overlays = document.getElementById('overlays');

    top.innerHTML =
      `<div class="bar">` +
        `<button class="doc-title" id="doc-title-btn" aria-haspopup="menu" aria-expanded="false" title="Document">` +
          `<span class="name" id="doc-title">Untitled</span><span class="dot"></span><span class="caretdown">${CHEV}</span>` +
        `</button>` +
        `<span class="side right"><button class="view" id="view-btn" aria-haspopup="menu" aria-expanded="false" title="View"></button></span>` +
      `</div>`;
    bottom.innerHTML = `<div class="bar" id="stats-bar" role="group" aria-label="Document statistics"></div>`;

    const titleBtn = document.getElementById('doc-title-btn');
    const viewBtn = document.getElementById('view-btn');
    const statsBar = document.getElementById('stats-bar');

    // ---------------------------------------------------------- the counts
    let last = count('');
    function paint() {
      const sel = W.selection();
      const selected = sel.end > sel.start;
      const s = selected ? count(W.getText().slice(sel.start, sel.end)) : last;
      const parts = [];
      if (selected) parts.push('<span class="stat sel-label">Selection</span>');
      for (const id of shown()) {
        const f = FIELDS.find((x) => x.id === id); if (!f) continue;
        const [v, l] = f.get(s);
        parts.push(`<span class="stat"><b>${v}</b> ${l}</span>`);
      }
      statsBar.innerHTML = parts.join('');
    }
    // The scan is O(document) — it never runs on the keystroke path. It runs when
    // the main thread is idle, and at the latest when typing stops. [latency]
    let queued = false;
    const idle = window.requestIdleCallback || ((fn) => setTimeout(fn, 180));
    function recount(now) {
      if (now) { last = count(W.getText()); paint(); return; }
      if (queued) return;
      queued = true;
      idle(() => { queued = false; last = count(W.getText()); paint(); }, { timeout: 500 });
    }

    // ------------------------------------------------------ hide while typing
    // Two clocks. The counts go quiet the moment you type and catch up 500 ms
    // after you stop; the title bar leaves altogether and comes back a beat
    // later, or the instant you reach for the mouse.
    let tA = 0, tB = 0;
    function typingNow() {
      root.dataset.typing = 'on';
      if (!openPanel) root.dataset.quiet = 'on';
      clearTimeout(tA); clearTimeout(tB);
      tA = setTimeout(() => { root.dataset.typing = 'off'; recount(true); }, 500);
      tB = setTimeout(() => { root.dataset.quiet = 'off'; }, 1400);
    }
    function wake() {
      if (root.dataset.quiet === 'on' || root.dataset.typing === 'on') {
        clearTimeout(tA); clearTimeout(tB);
        root.dataset.typing = 'off'; root.dataset.quiet = 'off';
        recount();
      }
    }
    W.on('change', () => { typingNow(); recount(); });
    W.on('selection', () => { if (root.dataset.typing !== 'on') paint(); });
    W.on('settings', (k) => {
      if (k === 'focus' || k === 'typewriter') viewBtn.innerHTML = rowsIcon(W.settings.focus) + `<span class="chev">${CHEV}</span>`;
      if (k === 'stats' || k === 'statsBar') { root.dataset.stats = W.settings.statsBar === false ? 'off' : 'on'; paint(); }
    });
    addEventListener('pointermove', wake, { passive: true });
    addEventListener('pointerdown', wake, { passive: true });

    // A rule under the top bar and over the stats bar, but only while there is
    // text on the other side of it.
    let rafS = 0;
    function scrolled() {
      rafS = 0;
      const sc = W.el.scroller;
      const over = sc.scrollTop > 2;
      const under = sc.scrollTop + sc.clientHeight < sc.scrollHeight - 2;
      root.dataset.scrolled = over && under ? 'both' : over ? 'over' : under ? 'under' : 'none';
      if (over && under) root.dataset.scrolled = 'under';   // both rules on
    }
    W.on('scroll', () => { if (!rafS) rafS = requestAnimationFrame(scrolled); });
    W.on('render', () => { if (!rafS) rafS = requestAnimationFrame(scrolled); });
    W.on('resize', () => { if (!rafS) rafS = requestAnimationFrame(scrolled); });

    // ------------------------------------------------------------- the menus
    function docMenu() {
      menu([
        { cmd: 'file.new' }, { cmd: 'file.open' }, { cmd: 'library.toggle' },
        { sep: true },
        { cmd: 'file.save' }, { cmd: 'file.saveAs' }, { cmd: 'file.rename' }, { cmd: 'file.duplicate' },
        { sep: true },
        { cmd: 'file.export' },
      ], { key: 'doc', anchor: titleBtn, x: titleBtn.getBoundingClientRect().left, y: titleBtn.getBoundingClientRect().bottom + 4, align: 'left' });
    }
    function viewMenu() {
      const f = W.settings.focus, r = viewBtn.getBoundingClientRect();
      menu([
        { label: f === 'off' ? 'Enable Focus Mode' : 'Disable Focus Mode', keys: 'Mod+D', flush: true, run: () => W.run('focus.toggle') },
        { label: 'Sentence', checked: f === 'sentence', run: () => W.setSetting('focus', 'sentence') },
        { label: 'Paragraph', checked: f === 'paragraph', run: () => W.setSetting('focus', 'paragraph') },
        { label: 'Typewriter', keys: 'Mod+T', checked: !!W.settings.typewriter, run: () => W.run('typewriter.toggle') },
        { sep: true },
        { head: 'Typeface' },
        { label: 'Duo', checked: W.settings.font === 'duo', run: () => W.setSetting('font', 'duo') },
        { label: 'Quattro', checked: W.settings.font === 'quattro', run: () => W.setSetting('font', 'quattro') },
        { label: 'Mono', checked: W.settings.font === 'mono', run: () => W.setSetting('font', 'mono') },
        { sep: true },
        { label: 'Dark Mode', keys: 'Mod+Shift+L', checked: isDark(), run: () => W.run('theme.toggle') },
        { label: 'Statistics', checked: W.settings.statsBar !== false, run: () => W.setSetting('statsBar', W.settings.statsBar === false) },
        { label: 'Hide Both Bars', keys: 'Mod+Shift+H', flush: true, run: () => W.setSetting('showChrome', false) },
        { label: 'All Commands…', keys: 'Mod+K', flush: true, run: () => W.run('palette.open') },
      ], { key: 'view', anchor: viewBtn, x: r.right, y: r.bottom + 4, align: 'right' });
    }
    function statsMenu(x, y) {
      const cur = shown();
      const items = FIELDS.map((f) => ({
        label: f.label, checked: cur.includes(f.id),
        run: () => {
          const set = new Set(shown());
          set.has(f.id) ? set.delete(f.id) : set.add(f.id);
          W.setSetting('stats', FIELDS.filter((x) => set.has(x.id)).map((x) => x.id).join(','));
          paint();
        },
      }));
      items.push({ sep: true }, { label: 'Hide Statistics', flush: true, run: () => W.setSetting('statsBar', false) });
      menu(items, { key: 'stats', x, y, align: 'right', place: 'above' });
    }
    function isDark() {
      const t = W.settings.theme;
      return t === 'dark' || (t === 'auto' && matchMedia('(prefers-color-scheme: dark)').matches);
    }

    titleBtn.addEventListener('click', (e) => { e.stopPropagation(); docMenu(); });
    viewBtn.addEventListener('click', (e) => { e.stopPropagation(); viewMenu(); });
    statsBar.addEventListener('click', (e) => { e.stopPropagation(); statsMenu(Math.min(e.clientX + 90, innerWidth - 8), bottom.getBoundingClientRect().top - 4); });
    statsBar.addEventListener('contextmenu', (e) => { e.preventDefault(); e.stopPropagation(); statsMenu(Math.min(e.clientX + 90, innerWidth - 8), bottom.getBoundingClientRect().top - 4); });
    addEventListener('pointerdown', (e) => { if (openPanel && !openPanel.el.contains(e.target)) closePanel(); });
    addEventListener('resize', () => closePanel(false));
    addEventListener('keydown', (e) => { if (openPanel && menuKeys(e)) { e.preventDefault(); e.stopPropagation(); } }, true);

    // ----------------------------------------------------------- the palette
    function palette() {
      if (openPanel && openPanel.key === 'palette') { closePanel(); return; }
      closePanel(false);
      const el = document.createElement('div');
      el.className = 'palette';
      el.innerHTML = `<div class="palette-field">${MAG}<input type="text" spellcheck="false" placeholder="Search commands"></div><ul class="palette-list"></ul>`;
      overlays.appendChild(el);
      root.dataset.menu = 'on';
      const inp = el.querySelector('input'), list = el.querySelector('ul');
      let items = [], sel = 0;

      // subsequence match: "sen" finds "Focus: Sentence", "tw" finds "Typewriter"
      function score(title, q) {
        if (!q) return { s: 0, hit: null };
        const t = title.toLowerCase();
        const i = t.indexOf(q);
        if (i >= 0) return { s: (i === 0 ? 0 : 1) + i / 100, hit: [[i, i + q.length]] };
        let k = 0, hit = [];
        for (let j = 0; j < t.length && k < q.length; j++) if (t[j] === q[k]) { hit.push([j, j + 1]); k++; }
        return k === q.length ? { s: 6 + hit[0][0] / 100, hit } : null;
      }
      function mark(title, hit) {
        if (!hit) return esc(title);
        let out = '', at = 0;
        for (const [a, b] of hit) { out += esc(title.slice(at, a)) + '<i>' + esc(title.slice(a, b)) + '</i>'; at = b; }
        return out + esc(title.slice(at));
      }
      const esc = (s) => s.replace(/[&<>]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;' }[c]));

      function render() {
        const q = inp.value.trim().toLowerCase();
        items = [];
        for (const c of W.commands()) {
          if (!c.title) continue;
          const m = score(c.title, q);
          if (!m) continue;
          items.push({ c, s: m.s, hit: m.hit });
        }
        items.sort((a, b) => a.s - b.s || a.c.title.localeCompare(b.c.title));
        if (sel >= items.length) sel = Math.max(0, items.length - 1);
        list.innerHTML = items.length ? items.map((it, i) =>
          `<li class="${i === sel ? 'on' : ''}" data-i="${i}"><span class="label">${mark(it.c.title, it.hit)}</span>` +
          (it.c.keys ? `<span class="keys">${keyLabel(it.c.keys)}</span>` : '') + `</li>`).join('')
          : `<li class="palette-empty">Nothing matches “${esc(inp.value.trim())}”</li>`;
        const on = list.querySelector('li.on'); if (on) on.scrollIntoView({ block: 'nearest' });
      }
      function run(i) { const it = items[i]; closePanel(); if (it) W.run(it.c.id); }
      inp.addEventListener('input', () => { sel = 0; render(); });
      inp.addEventListener('keydown', (e) => {
        if (e.key === 'Escape') { e.preventDefault(); closePanel(); }
        else if (e.key === 'ArrowDown') { e.preventDefault(); sel = items.length ? (sel + 1) % items.length : 0; render(); }
        else if (e.key === 'ArrowUp') { e.preventDefault(); sel = items.length ? (sel - 1 + items.length) % items.length : 0; render(); }
        else if (e.key === 'Enter') { e.preventDefault(); run(sel); }
        e.stopPropagation();
      });
      list.addEventListener('click', (e) => { const li = e.target.closest('li[data-i]'); if (li) run(+li.dataset.i); });
      list.addEventListener('mousemove', (e) => { const li = e.target.closest('li[data-i]'); if (li && +li.dataset.i !== sel) { sel = +li.dataset.i; render(); } });
      openPanel = { el, key: 'palette' };
      render(); inp.focus();
    }

    // ------------------------------------------------------------- commands
    W.registerCommand('palette.open', { title: 'All Commands…', keys: ['Mod+K', 'Mod+Shift+P'], run: palette });
    W.registerCommand('chrome.toggle', { title: 'Show / Hide Bars', keys: 'Mod+Shift+H', run: () => W.setSetting('showChrome', !W.settings.showChrome) });
    W.registerCommand('chrome.stats', { title: 'Show / Hide Statistics', run: () => W.setSetting('statsBar', W.settings.statsBar === false) });
    W.registerCommand('chrome.view', { title: 'View Menu', run: viewMenu });
    W.registerCommand('chrome.doc', { title: 'Document Menu', run: docMenu });
    W.registerCommand('font.duo', { title: 'Typeface: Duo', run: () => W.setSetting('font', 'duo') });
    W.registerCommand('font.quattro', { title: 'Typeface: Quattro', run: () => W.setSetting('font', 'quattro') });
    W.registerCommand('font.mono', { title: 'Typeface: Mono', run: () => W.setSetting('font', 'mono') });
    W.registerCommand('font.bigger', { title: 'Bigger Text', keys: ['Mod+=', 'Mod++'], run: () => W.setSetting('fontSize', Math.min(40, W.settings.fontSize + 1)) });
    W.registerCommand('font.smaller', { title: 'Smaller Text', keys: 'Mod+-', run: () => W.setSetting('fontSize', Math.max(10, W.settings.fontSize - 1)) });
    W.registerCommand('font.reset', { title: 'Default Text Size', keys: 'Mod+0', run: () => W.setSetting('fontSize', 20) });

    // ----------------------------------------------------------------- start
    viewBtn.innerHTML = rowsIcon(W.settings.focus) + `<span class="chev">${CHEV}</span>`;
    root.dataset.stats = W.settings.statsBar === false ? 'off' : 'on';
    root.dataset.typing = 'off'; root.dataset.quiet = 'off'; root.dataset.menu = 'off';
    recount(true);
    W.on('ready', () => { recount(true); scrolled(); });

    // Screenshot / deep-link hook: ?open=view|document|stats|palette
    const want = new URLSearchParams(location.search).get('open');
    if (want) setTimeout(() => {
      if (want === 'view') viewMenu();
      else if (want === 'document') docMenu();
      else if (want === 'palette') palette();
      else if (want === 'stats') { const r = bottom.getBoundingClientRect(); statsMenu(Math.min(innerWidth - 40, r.width * 0.86), r.top - 4); }
    }, 60);
  });
})();
