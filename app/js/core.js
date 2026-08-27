/* Quill core — textarea + mirror editor engine.
 * Owner: core (do not edit from piece builders without noting it in NOTES.md).
 *
 * Model: the <textarea id="input"> holds the text and the native selection.
 * The <div id="mirror"> renders the same text, line by line, with markup styling,
 * using identical font metrics so glyphs line up exactly. The textarea sits on
 * top with transparent text; the mirror below supplies the visible glyphs.
 *
 * Plugins register:
 *   Writer.tokenizeLine = (text, ctx) => [{text, cls}]   (markup.js)
 *   Writer.addDecorator(fn)  fn(lineIndex, lineText) => [{from,to,cls}]  (focus.js)
 *   Writer.on(event, fn): 'change' | 'selection' | 'render' | 'scroll' | 'settings' | 'boot'
 *   Writer.registerCommand(id, {title, keys, run})
 */
(function () {
  const Writer = (window.Writer = {});
  const listeners = {};
  Writer.on = (ev, fn) => ((listeners[ev] ||= []).push(fn), fn);
  Writer.off = (ev, fn) => { const l = listeners[ev]; if (l) l.splice(l.indexOf(fn) >>> 0, 1); };
  Writer.emit = (ev, ...a) => { const l = listeners[ev]; if (l) for (const f of l) f(...a); };

  // ---------- settings ----------
  const DEFAULTS = {
    theme: 'auto',          // auto | light | dark
    font: 'duo',            // duo | quattro | mono
    fontSize: 20,           // px — measured iA default (see app/css/type.css)
    focus: 'off',           // off | sentence | paragraph
    typewriter: false,
    typewriterLine: 0.5,    // 0..1 fraction of viewport height where the caret line rests
    spellcheck: false,
    showChrome: true,
  };
  let settings = { ...DEFAULTS };
  try { Object.assign(settings, JSON.parse(localStorage.getItem('quill.settings') || '{}')); } catch (e) {}
  Writer.settings = settings;
  Writer.setSetting = (k, v) => {
    if (settings[k] === v) return;
    settings[k] = v;
    try { localStorage.setItem('quill.settings', JSON.stringify(settings)); } catch (e) {}
    applySettings();
    Writer.emit('settings', k, v);
  };
  function applySettings() {
    const root = document.documentElement;
    root.dataset.theme = settings.theme;
    root.dataset.font = settings.font;
    root.dataset.focus = settings.focus;
    root.dataset.typewriter = settings.typewriter ? 'on' : 'off';
    root.dataset.chrome = settings.showChrome ? 'on' : 'off';
    root.style.setProperty('--font-size', settings.fontSize + 'px');
    if (input) input.spellcheck = !!settings.spellcheck;
  }

  // ---------- elements ----------
  let scroller, page, mirror, input, caretLayer;
  Writer.el = {};

  // ---------- rendering ----------
  let lines = [];            // current text split by \n
  let lineEls = [];          // one element per line, in order
  let lineCtx = [];          // per-line tokenizer context (e.g. fence state) computed sequentially
  const decorators = [];
  Writer.addDecorator = (fn) => { decorators.push(fn); return fn; };
  Writer.tokenizeLine = (text) => [{ text, cls: '' }];   // replaced by markup.js
  Writer.lineContext = (prevCtx, text) => prevCtx;        // replaced by markup.js (returns ctx for NEXT line)

  // A line element carries no index attribute. Nothing reads one (checked across app/ and
  // tools/), and keeping it truthful means rewriting every element below an inserted line on
  // every Enter — 1.14 ms of attribute writes and style invalidations in a 3,285-line document.
  // Use Writer.lineIndexOf(el) if you ever need to go the other way.
  function newLineEl() { const el = document.createElement('div'); el.className = 'line'; return el; }
  function buildLine(i, text) { const el = newLineEl(); fillLine(el, i, text); return el; }
  function fillLine(el, i, text) {
    el.textContent = '';
    const ctx = lineCtx[i] || null;
    let tokens = Writer.tokenizeLine(text, ctx, i);
    // apply decorations by splitting tokens
    let decos = [];
    for (const d of decorators) { const r = d(i, text); if (r && r.length) decos = decos.concat(r); }
    if (decos.length) tokens = applyDecorations(tokens, decos);
    if (ctx && ctx.lineClass) el.className = 'line ' + ctx.lineClass; else el.className = 'line';
    if (tokens.lineClass) el.className += ' ' + tokens.lineClass;
    if (!text.length) { el.appendChild(document.createElement('br')); return; }
    const frag = document.createDocumentFragment();
    for (const t of tokens) {
      if (!t.text) continue;
      if (!t.cls) { frag.appendChild(document.createTextNode(t.text)); continue; }
      const s = document.createElement('span');
      s.className = t.cls;
      s.textContent = t.text;
      frag.appendChild(s);
    }
    el.appendChild(frag);
  }
  function applyDecorations(tokens, decos) {
    // split tokens at decoration boundaries and add classes
    const cuts = new Set();
    for (const d of decos) { cuts.add(d.from); cuts.add(d.to); }
    const out = []; let pos = 0;
    for (const t of tokens) {
      let start = pos, end = pos + t.text.length;
      let segs = [start];
      for (const c of cuts) if (c > start && c < end) segs.push(c);
      segs.sort((a, b) => a - b); segs.push(end);
      for (let k = 0; k < segs.length - 1; k++) {
        const a = segs[k], b = segs[k + 1];
        let cls = t.cls || '';
        for (const d of decos) if (d.from <= a && d.to >= b) cls += ' ' + d.cls;
        out.push({ text: t.text.slice(a - start, b - start), cls: cls.trim() });
      }
      pos = end;
    }
    out.lineClass = tokens.lineClass;
    return out;
  }

  // Recompute per-line context from line `from` forward; returns first index where ctx unchanged & stable (or lines.length)
  //
  // `lineCtx[i]` is the context ENTERING line i, so the walk has to start from the context
  // LEAVING line from-1 — that is `lineContext(lineCtx[from-1], lines[from-1])`, not `lineCtx[from-1]`.
  // Seeding with the latter skipped the effect of the line just above the edit, so an edit whose
  // first changed line is the one AFTER a fence opener (press Enter at the end of ```js, the most
  // ordinary thing there is) left every line below tokenised as prose, for good. Found by
  // shots/latency/probes/correctness.mjs; round 2's version of that probe searched for its own
  // fence with findIndex() and matched the document's existing one, so it never saw this.
  function recomputeCtx(from, force) {
    let ctx = from > 0 ? Writer.lineContext(lineCtx[from - 1] || null, lines[from - 1], from - 1) : null;
    let i = from;
    for (; i < lines.length; i++) {
      const next = Writer.lineContext(ctx, lines[i], i);
      // ctx object for THIS line = state entering the line; store it
      const entering = ctx;
      const changed = !sameCtx(lineCtx[i], entering);
      lineCtx[i] = entering;
      ctx = next;
      if (!force && !changed && i >= from + 1) { /* stable from here if the following ctx is unchanged too */
        if (sameCtx(lineCtx[i + 1], ctx)) return i + 1;
      }
    }
    lineCtx.length = lines.length;
    return i;
  }
  function sameCtx(a, b) {
    if (a === b) return true;
    if (!a || !b) return (!a || !Object.keys(a).length) && (!b || !Object.keys(b).length);
    const ka = Object.keys(a), kb = Object.keys(b);
    if (ka.length !== kb.length) return false;
    for (const k of ka) if (a[k] !== b[k]) return false;
    return true;
  }

  // ---------- bounded re-tokenising ----------
  // How far below an edit the mirror is brought up to date inside the keystroke. Round 2 asserted
  // "64 lines is more than a screenful at any font size"; it is not, at the small end of the size
  // range in a tall window. So measure it instead: a screenful is the scroller's height divided by
  // the shortest a line can be (its min-height is exactly one line pitch), plus a margin. Nothing
  // the reader can see is ever stale — and if they scroll into the catch-up range before it has
  // run, the scroll handler fills what came into view before the frame is painted.
  // The remainder is filled in animation frames under a time budget (never a fixed line count, so
  // one catch-up frame cannot blow past a frame's worth of work), or at once (Writer.flushPending).
  let AHEAD = 64;
  const CHUNK_BUDGET_MS = 4, CHUNK_MIN = 32;
  function computeAhead() {
    try {
      const probe = lineEls[0] || mirror.firstElementChild;
      const pitch = probe ? parseFloat(getComputedStyle(probe).minHeight) : 0;
      const h = (scroller && scroller.clientHeight) || window.innerHeight || 900;
      AHEAD = pitch > 4 ? Math.max(24, Math.ceil(h / pitch) + 8) : 64;
    } catch (e) { AHEAD = 64; }
  }
  Writer.aheadLines = () => AHEAD;
  let pendFrom = -1, pendTo = -1, pendRaf = 0;
  function addPending(from, to) {
    if (pendFrom < 0) { pendFrom = from; pendTo = to; }
    else { if (from < pendFrom) pendFrom = from; if (to > pendTo) pendTo = to; }
  }
  function schedulePending() {
    if (pendFrom < 0 || pendRaf) return;
    pendRaf = requestAnimationFrame(function step() {
      pendRaf = 0;
      // Whatever the reader is actually looking at, first — the AHEAD window follows the edit,
      // and the eye need not be there (undo, a command, a paste, or simply having scrolled away).
      // An animation frame runs BEFORE the frame's style/layout/paint, so this still lands in the
      // first frame after the keystroke: nothing stale is ever painted, and the keystroke itself
      // pays nothing for it.
      flushVisible();
      if (pendFrom < 0) return;
      const stop = Math.min(pendTo, lineEls.length);
      const t0 = performance.now();
      let i = pendFrom;
      for (; i < stop; i++) {
        if (lineEls[i]) fillLine(lineEls[i], i, lines[i]);
        if (i - pendFrom >= CHUNK_MIN && (i - pendFrom) % 32 === 0 && performance.now() - t0 >= CHUNK_BUDGET_MS) { i++; break; }
      }
      pendFrom = i;
      if (pendFrom < pendTo) pendRaf = requestAnimationFrame(step); else pendFrom = pendTo = -1;
    });
  }
  // The lines on screen right now, by binary search on offsetTop (12 reads on a 3,000-line
  // document, and only ever from a scroll or a flush — never from a keystroke).
  function visibleRange() {
    if (!lineEls.length) return [0, -1];
    const mTop = mirror.getBoundingClientRect().top;
    const vh = window.innerHeight || 900;
    const firstAtOrAfter = (y) => {                 // first line whose bottom edge is at or below y
      let lo = 0, hi = lineEls.length - 1, ans = lineEls.length - 1;
      while (lo <= hi) {
        const m = (lo + hi) >> 1, el = lineEls[m];
        if (!el) { lo = m + 1; continue; }
        if (mTop + el.offsetTop + el.offsetHeight >= y) { ans = m; hi = m - 1; } else lo = m + 1;
      }
      return ans;
    };
    return [firstAtOrAfter(0), firstAtOrAfter(vh)];
  }
  Writer.visibleRange = visibleRange;
  // Catch up whatever scrolled into view while a catch-up was still in flight. Idempotent: the
  // contiguous pending range is left alone, so the lines are simply filled with the right tokens
  // sooner. Bounded by the viewport, so it can never be more than a screenful of work.
  function flushVisible() {
    if (pendFrom < 0) return;
    const [v0, v1] = visibleRange();
    const a = Math.max(v0, pendFrom), b = Math.min(v1, pendTo - 1, lineEls.length - 1);
    for (let i = a; i <= b; i++) if (lineEls[i]) fillLine(lineEls[i], i, lines[i]);
    if (a <= pendFrom && b >= pendFrom) pendFrom = Math.min(b + 1, pendTo);
    if (pendFrom >= pendTo) { pendFrom = pendTo = -1; if (pendRaf) { cancelAnimationFrame(pendRaf); pendRaf = 0; } }
  }
  Writer.flushVisible = flushVisible;
  function flushPending() {
    if (pendFrom < 0) return;
    if (pendRaf) { cancelAnimationFrame(pendRaf); pendRaf = 0; }
    const end = Math.min(pendTo, lineEls.length);
    for (let i = pendFrom; i < end; i++) if (lineEls[i]) fillLine(lineEls[i], i, lines[i]);
    pendFrom = pendTo = -1;
  }
  Writer.flushPending = flushPending;
  Writer.pendingLines = () => (pendFrom < 0 ? 0 : Math.max(0, pendTo - pendFrom));

  let lastHeight = -1;
  function syncHeight(h) { if (h !== lastHeight) { lastHeight = h; input.style.height = h + 'px'; } }
  Writer.syncHeight = () => syncHeight(mirror.offsetHeight);

  // 'render' is where listeners measure the page (the caret asks for its rect). Emitting it while
  // other listeners still have DOM writes ahead of them costs one extra layout per keystroke, so the
  // input path defers it until every writer has run. Everyone else gets it synchronously as before.
  let deferRenderEvent = false, pendingRenderEvent = false;

  Writer.render = function render(full) {
    const text = input.value;
    const newLines = text.split('\n');
    if (full || !lineEls.length) {
      pendFrom = pendTo = -1; if (pendRaf) { cancelAnimationFrame(pendRaf); pendRaf = 0; }
      lines = newLines; lineCtx = []; recomputeCtx(0, true);
      mirror.textContent = '';
      const frag = document.createDocumentFragment();
      lineEls = newLines.map((t, i) => { const el = buildLine(i, t); frag.appendChild(el); return el; });
      mirror.appendChild(frag);
    } else {
      // common prefix / suffix diff on lines
      let a = 0, oldN = lines.length, newN = newLines.length;
      const min = Math.min(oldN, newN);
      while (a < min && lines[a] === newLines[a]) a++;
      let b = 0;
      while (b < min - a && lines[oldN - 1 - b] === newLines[newN - 1 - b]) b++;
      const oldMid = oldN - a - b, newMid = newN - a - b;
      lines = newLines;
      // Replace the middle. The new elements go in empty: a line's tokens depend on the context
      // computed from the lines above it, which is only correct after recomputeCtx below, so
      // filling them here would mean tokenising and rebuilding every edited line twice per
      // keystroke — which is exactly what this used to do.
      const anchor = lineEls[oldN - b] || null;
      const frag = document.createDocumentFragment();
      const mid = [];
      for (let i = 0; i < newMid; i++) { const el = newLineEl(); mid.push(el); frag.appendChild(el); }
      for (let i = 0; i < oldMid; i++) lineEls[a + i].remove();
      if (anchor) mirror.insertBefore(frag, anchor); else mirror.appendChild(frag);
      if (newMid === oldMid) { for (let k = 0; k < newMid; k++) lineEls[a + k] = mid[k]; }
      else if (newMid < 8000) lineEls.splice(a, oldMid, ...mid);      // spread has an argument limit
      else lineEls = lineEls.slice(0, a).concat(mid, lineEls.slice(a + oldMid));
      // lineCtx is indexed by line, and recomputeCtx decides where to stop by comparing the
      // context it computes for line i against lineCtx[i]. If the line count changed and lineCtx
      // was not moved with it, those comparisons are against the wrong lines: the walk can stop
      // early on a false match and leave lineCtx short at the tail, so the last line of the
      // document keeps a null context. Move it exactly as lineEls moves.
      if (newMid !== oldMid) {
        const blanks = new Array(newMid);
        if (newMid < 8000) lineCtx.splice(a, oldMid, ...blanks);
        else lineCtx = lineCtx.slice(0, a).concat(blanks, lineCtx.slice(a + oldMid));
      }
      // Move any catch-up still in flight by the number of lines this edit added or removed.
      if (pendFrom >= 0) { const d = newN - oldN; if (pendFrom >= a) pendFrom += d; if (pendTo >= a) pendTo += d; }
      const stableAt = Math.min(recomputeCtx(a), lineEls.length);
      for (let i = a; i < a + newMid; i++) fillLine(lineEls[i], i, lines[i]);
      // Lines below the edit only need re-tokenising when the context entering them changed —
      // usually nothing at all, but typing ``` opens a fenced block and changes every line to the
      // end of the document (26 ms of re-tokenising in a 55k-word manuscript: three frames). Do
      // what the reader can see now, hand the rest to animation frames.
      const near = Math.min(stableAt, a + newMid + AHEAD);
      for (let i = a + newMid; i < near; i++) fillLine(lineEls[i], i, lines[i]);
      if (stableAt > near) addPending(near, stableAt);
      schedulePending();
    }
    // The textarea is sized to the mirror. Reading mirror.offsetHeight here forces a synchronous
    // layout of the whole document inside the keystroke, and writing input.style.height afterwards
    // dirties it again — two full layouts per keypress on a long document. A ResizeObserver on the
    // mirror does the same job from the frame's own layout pass (see boot), so the keystroke path
    // does no measuring at all. Full renders (boot, setText, font change) still sync immediately,
    // because callers may measure right away.
    if (full) syncHeight(mirror.offsetHeight);
    if (deferRenderEvent) pendingRenderEvent = true; else Writer.emit('render');
  };
  Writer.rerenderLines = function (indices) {
    for (const i of indices) if (lineEls[i]) fillLine(lineEls[i], i, lines[i]);
  };
  // el -> index, for anything that used to read the line's data-i attribute.
  Writer.lineIndexOf = (el) => lineEls.indexOf(el);
  Writer.lines = () => lines;
  Writer.lineEl = (i) => lineEls[i];
  Writer.lineCount = () => lines.length;

  // ---------- positions ----------
  Writer.getText = () => input.value;
  Writer.setText = (t, opts = {}) => {
    input.value = t;
    if (opts.caret != null) input.setSelectionRange(opts.caret, opts.caret);
    else input.setSelectionRange(0, 0);
    Writer.render(true);
    Writer.emit('change', { source: opts.source || 'set' });
    emitSelection(true);
  };
  Writer.selection = () => ({ start: input.selectionStart, end: input.selectionEnd, dir: input.selectionDirection });
  Writer.setSelection = (s, e = s) => { input.setSelectionRange(s, e); emitSelection(true); };
  // One keystroke raises three selection notifications in Chrome (input, selectionchange, keyup) and
  // every listener on it measures layout. Fire once per real change instead: same events, a third of
  // the work. `force` is for text changes, where the offsets can be identical but the line is not.
  let selS = -1, selE = -1, selD = '';
  function emitSelection(force) {
    const s = input.selectionStart, e = input.selectionEnd, d = input.selectionDirection;
    if (!force && s === selS && e === selE && d === selD) return;
    selS = s; selE = e; selD = d;
    Writer.emit('selection');
  }
  Writer.emitSelection = emitSelection;
  // offset -> {line, col}
  Writer.offsetToPos = (off) => {
    let i = 0, acc = 0;
    for (; i < lines.length; i++) { const l = lines[i].length + 1; if (acc + l > off) break; acc += l; }
    if (i >= lines.length) { i = lines.length - 1; acc -= lines[i].length + 1; }
    return { line: i, col: off - acc };
  };
  Writer.posToOffset = (line, col) => { let acc = 0; for (let i = 0; i < line; i++) acc += lines[i].length + 1; return acc + col; };
  // DOM rect of a text offset in the mirror (relative to viewport)
  Writer.offsetRect = (off, preferEnd) => {
    const { line, col } = Writer.offsetToPos(off);
    const el = lineEls[line]; if (!el) return null;
    if (!lines[line].length) { const r = el.getBoundingClientRect(); return new DOMRect(r.left, r.top, 0, r.height); }
    // walk text nodes
    const walker = document.createTreeWalker(el, NodeFilter.SHOW_TEXT);
    let n, acc = 0;
    const range = document.createRange();
    while ((n = walker.nextNode())) {
      const len = n.data.length;
      if (acc + len >= col) {
        const k = col - acc;
        // At a soft-wrap boundary a collapsed range is ambiguous; measure a char box
        if (k < len) { range.setStart(n, k); range.setEnd(n, k + 1); const rs = range.getClientRects(); const r = rs[0]; if (r) return new DOMRect(r.left, r.top, 0, r.height); }
        range.setStart(n, k); range.setEnd(n, k);
        const rs = range.getClientRects(); const r = rs[preferEnd ? rs.length - 1 : 0] || range.getBoundingClientRect();
        return new DOMRect(r.left, r.top, 0, r.height);
      }
      acc += len;
    }
    const r = el.getBoundingClientRect(); return new DOMRect(r.left, r.top, 0, r.height);
  };
  Writer.caretRect = () => Writer.offsetRect(input.selectionDirection === 'backward' ? input.selectionStart : input.selectionEnd, true);

  // ---------- commands & keys ----------
  const commands = new Map();
  let keymap = null;   // normalised key string -> command, built lazily (keydown is a hot path)
  Writer.registerCommand = (id, cmd) => { commands.set(id, { id, ...cmd }); keymap = null; return cmd; };
  Writer.commands = () => [...commands.values()];
  Writer.run = (id, ...a) => { const c = commands.get(id); if (c) return c.run(...a); };
  const isMac = /Mac|iPhone|iPad/.test(navigator.platform);
  Writer.isMac = isMac;
  function keyString(e) {
    const parts = [];
    if (e.ctrlKey) parts.push('Ctrl'); if (e.metaKey) parts.push('Meta'); if (e.altKey) parts.push('Alt'); if (e.shiftKey) parts.push('Shift');
    let k = e.key; if (k.length === 1) k = k.toUpperCase(); if (k === ' ') k = 'Space';
    parts.push(k); return parts.join('+');
  }
  // 'Mod' = Meta on mac, Ctrl elsewhere
  Writer.normKeys = (keys) => keys.split('+').map(p => p === 'Mod' ? (isMac ? 'Meta' : 'Ctrl') : p).sort((a, b) => order(a) - order(b)).join('+');
  const ORDER = { Ctrl: 0, Meta: 1, Alt: 2, Shift: 3 };
  function order(p) { return p in ORDER ? ORDER[p] : 9; }
  function buildKeymap() {
    keymap = new Map();
    for (const c of commands.values()) {
      if (!c.keys) continue;
      const list = Array.isArray(c.keys) ? c.keys : [c.keys];
      for (const k of list) if (!keymap.has(Writer.normKeys(k))) keymap.set(Writer.normKeys(k), c);
    }
  }
  function onKeydown(e) {
    // Plain typing must not pay for the shortcut table: no modifier, single character -> nothing to look up.
    if (e.ctrlKey || e.metaKey || e.altKey || e.key.length > 1) {
      if (!keymap) buildKeymap();
      const c = keymap.get(keyString(e));
      if (c) { e.preventDefault(); c.run(e); return; }
    }
    Writer.emit('keydown', e);
  }

  // ---------- boot ----------
  Writer.boot = function boot() {
    scroller = document.getElementById('scroller');
    page = document.getElementById('page');
    mirror = document.getElementById('mirror');
    input = document.getElementById('input');
    caretLayer = document.getElementById('caret-layer');
    Object.assign(Writer.el, { scroller, page, mirror, input, caretLayer });
    applySettings();
    // The keystroke path, in one pass: mirror writes -> 'change' -> 'selection' (focus writes) ->
    // 'render' (caret measures). Writes first, reads last, so the frame lays out exactly once.
    input.addEventListener('input', () => {
      deferRenderEvent = true;
      Writer.render(false);
      deferRenderEvent = false;
      Writer.emit('change', { source: 'input' });
      emitSelection(true);
      if (pendingRenderEvent) { pendingRenderEvent = false; Writer.emit('render'); }
    });
    document.addEventListener('selectionchange', () => { if (document.activeElement === input) emitSelection(false); });
    input.addEventListener('keydown', onKeydown);
    input.addEventListener('keyup', () => emitSelection(false));
    input.addEventListener('mouseup', () => emitSelection(false));
    input.addEventListener('focus', () => Writer.emit('focus'));
    input.addEventListener('blur', () => Writer.emit('blur'));
    scroller.addEventListener('scroll', () => { if (pendFrom >= 0) flushVisible(); Writer.emit('scroll'); }, { passive: true });
    window.addEventListener('resize', () => { computeAhead(); syncHeight(mirror.offsetHeight); Writer.emit('resize'); });
    Writer.on('settings', computeAhead);
    // Fonts may load after first layout; re-sync heights when they do.
    if (document.fonts) document.fonts.addEventListener('loadingdone', () => { syncHeight(mirror.offsetHeight); Writer.emit('resize'); });
    // Keep the textarea as tall as the mirror without ever measuring from a keystroke: the observer
    // is served out of the frame's own layout, and only fires when the height really changed.
    if (window.ResizeObserver) new ResizeObserver((entries) => {
      const e = entries[entries.length - 1];
      const box = e.borderBoxSize && e.borderBoxSize[0];
      syncHeight(box ? box.blockSize : e.contentRect.height);
    }).observe(mirror);
    Writer.emit('boot');
    if (!input.value.length) {
      // files.js restores the last document on boot; if nothing was restored, start empty.
    }
    Writer.render(true);
    computeAhead();
    input.focus();
    window.__quillReady = performance.now();
    Writer.emit('ready');
  };
})();
