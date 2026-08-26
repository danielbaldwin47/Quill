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
    fontSize: 18,           // px
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

  function buildLine(i, text) {
    const el = document.createElement('div');
    el.className = 'line';
    el.dataset.i = i;
    fillLine(el, i, text);
    return el;
  }
  function fillLine(el, i, text) {
    el.textContent = '';
    const ctx = lineCtx[i] || null;
    let tokens = Writer.tokenizeLine(text, ctx, i);
    // apply decorations by splitting tokens
    let decos = [];
    for (const d of decorators) { const r = d(i, text); if (r && r.length) decos = decos.concat(r); }
    if (decos.length) tokens = applyDecorations(tokens, decos);
    if (el.dataset.lc !== undefined) delete el.dataset.lc;
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
  function recomputeCtx(from) {
    let ctx = from > 0 ? lineCtx[from - 1] : null;
    let i = from;
    for (; i < lines.length; i++) {
      const next = Writer.lineContext(ctx, lines[i], i);
      // ctx object for THIS line = state entering the line; store it
      const entering = ctx;
      const changed = !sameCtx(lineCtx[i], entering);
      lineCtx[i] = entering;
      ctx = next;
      if (!changed && i >= from + 1) { /* stable from here if the following ctx is unchanged too */
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

  let renderScheduled = false;
  Writer.render = function render(full) {
    renderScheduled = false;
    const text = input.value;
    const newLines = text.split('\n');
    if (full || !lineEls.length) {
      lines = newLines; lineCtx = []; recomputeCtx(0);
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
      // replace middle
      const before = lineEls.slice(0, a), after = lineEls.slice(oldN - b);
      const frag = document.createDocumentFragment();
      const mid = [];
      for (let i = a; i < a + newMid; i++) { const el = buildLine(i, newLines[i]); mid.push(el); frag.appendChild(el); }
      for (let i = 0; i < oldMid; i++) lineEls[a + i].remove();
      if (after.length) mirror.insertBefore(frag, after[0]); else mirror.appendChild(frag);
      lineEls = before.concat(mid, after);
      // renumber & fix contexts forward
      const stableAt = recomputeCtx(a);
      for (let i = a + newMid; i < lineEls.length; i++) {
        lineEls[i].dataset.i = i;
        if (i < stableAt) fillLine(lineEls[i], i, lines[i]);
      }
      // A line's tokenization may depend on ctx computed from previous lines; mid lines were built before ctx was updated
      for (let i = a; i < a + newMid; i++) fillLine(lineEls[i], i, lines[i]);
    }
    input.style.height = mirror.offsetHeight + 'px';
    Writer.emit('render');
  };
  Writer.rerenderLines = function (indices) {
    for (const i of indices) if (lineEls[i]) fillLine(lineEls[i], i, lines[i]);
  };
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
    Writer.emit('selection');
  };
  Writer.selection = () => ({ start: input.selectionStart, end: input.selectionEnd, dir: input.selectionDirection });
  Writer.setSelection = (s, e = s) => { input.setSelectionRange(s, e); Writer.emit('selection'); };
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
  Writer.registerCommand = (id, cmd) => { commands.set(id, { id, ...cmd }); return cmd; };
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
  function onKeydown(e) {
    const ks = keyString(e);
    for (const c of commands.values()) {
      if (!c.keys) continue;
      const list = Array.isArray(c.keys) ? c.keys : [c.keys];
      for (const k of list) if (Writer.normKeys(k) === ks) { e.preventDefault(); c.run(e); return; }
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
    input.addEventListener('input', () => { Writer.render(false); Writer.emit('change', { source: 'input' }); Writer.emit('selection'); });
    document.addEventListener('selectionchange', () => { if (document.activeElement === input) Writer.emit('selection'); });
    input.addEventListener('keydown', onKeydown);
    input.addEventListener('keyup', () => Writer.emit('selection'));
    input.addEventListener('mouseup', () => Writer.emit('selection'));
    input.addEventListener('focus', () => Writer.emit('focus'));
    input.addEventListener('blur', () => Writer.emit('blur'));
    scroller.addEventListener('scroll', () => Writer.emit('scroll'), { passive: true });
    window.addEventListener('resize', () => { input.style.height = mirror.offsetHeight + 'px'; Writer.emit('resize'); });
    // Fonts may load after first layout; re-sync heights when they do.
    if (document.fonts) document.fonts.addEventListener('loadingdone', () => { input.style.height = mirror.offsetHeight + 'px'; Writer.emit('resize'); });
    Writer.emit('boot');
    if (!input.value.length) {
      // files.js restores the last document on boot; if nothing was restored, start empty.
    }
    Writer.render(true);
    input.focus();
    window.__quillReady = performance.now();
    Writer.emit('ready');
  };
})();
