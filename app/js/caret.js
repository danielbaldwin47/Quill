/* Caret & selection. Owner: caret piece.
 *
 * The native caret and the native selection are both switched off (css); this
 * draws both, because the browser will not give us either at the right size:
 *
 *   - a native caret is a 1px hairline on the font's content box;
 *   - a native textarea selection paints *above* #mirror, so it veils the ink.
 *
 * Geometry is measured from iA Writer's own captures (ref/ia/REFERENCE.md §4.1
 * plus re-measurement here of appstore-mac-01 and msstore-win-01):
 *
 *   caret width   8.4 px @ 55.3 px  = .152 em   |  5.0 px @ 31.1 px = .161 em
 *   caret height  92.8 px, pitch 92.5           |  52.0 px, pitch 52.0
 *   caret left    +3.3 px past the cell edge    |  +2.3 px  = .060 / .073 em
 *   band          68.75 % of the pitch above the baseline, 31.25 % below
 *
 * The last one is the interesting number. A CSS line box puts the baseline at
 * 73 % of the pitch, i.e. lower than iA does; iA's band is centred closer to
 * the middle of the *ink* (cap height to descender) than to the middle of the
 * font's em box, so the caret looks balanced against the letters rather than
 * top-heavy. We anchor to the measured baseline so it holds for any font,
 * size or line-height the type piece chooses.
 *
 * The horizontal nudge is the second one: the caret's left edge lands where
 * the *ink* of the next glyph will start, not where its advance cell starts.
 * The caret marks the letter, not the slot.
 */
(function () {
  const W = window.Writer;

  const NUDGE_X = 0.07;    // em past the advance boundary (measured .060–.073)
  const ABOVE   = 0.6875;  // share of the line pitch above the baseline (11/16)
  const WIDTH   = 0.155;   // em
  const IDLE_MS = 480;     // quiet before the caret starts blinking again
  const SNAP_MS = 60;      // moves closer together than this snap, never lag
  const GLIDE_X = 62;      // ms, along a line
  const GLIDE_Y = 92;      // ms, between lines
  const NL_TAIL = 0.5;     // em of highlight standing in for a selected newline
  const MAX_ROWS = 400;    // hard cap on selection rectangles per paint

  let caret, layer, selLayer, page, input, mirror, scroller, probe, probeSpan;
  let edgeA, edgeB;
  const pool = [];           // .sel fill rectangles
  const M = { em: 16, pitch: 26, base: 20, w: 2.5, dpr: 1 };
  let idleTimer = 0, prev = null, prevAt = 0, hasSel = false, scrollRaf = 0;
  let baseX = 0, baseY = 0, settleTimer = 0, moving = false;

  // ---------------------------------------------------------------- metrics
  // Snapping has to happen in *viewport* coordinates: #page's own left edge is
  // rarely on a whole device pixel (centred column, reserved scrollbar gutter),
  // so rounding the offset alone still lands the caret on a half pixel and the
  // stem comes out one column wider with a grey edge.
  function snap(v) { return Math.round(v * M.dpr) / M.dpr; }

  function measure() {
    const cs = getComputedStyle(mirror);
    M.em = parseFloat(cs.fontSize) || 16;
    M.pitch = parseFloat(cs.lineHeight) || M.em * 1.6;
    M.dpr = window.devicePixelRatio || 1;
    // Whole CSS pixels. Chrome pixel-snaps a painted box's left and right edge
    // in layout units, so a 4.5px stem is rasterised 5px wide however hard you
    // snap it; asking for the integer keeps the stem the width we chose and
    // both its edges hard, at any zoom or device ratio.
    M.w = Math.max(2, Math.round(M.em * WIDTH));
    // Baseline position inside a line box, measured rather than derived: the
    // three families round their ascent differently and a wrong guess shows.
    const pr = probe.getBoundingClientRect();
    const sr = probeSpan.getBoundingClientRect();
    if (pr.height) { M.base = sr.top - pr.top; if (!parseFloat(cs.lineHeight)) M.pitch = pr.height; }
  }

  // Top of the caret band for a client rect on a given line element.
  function bandTop(lineEl, r) {
    const lr = lineEl.getBoundingClientRect();
    const k = Math.max(0, Math.floor((r.top + r.height / 2 - lr.top) / M.pitch));
    return { top: lr.top + k * M.pitch, row: k, lineTop: lr.top, lineLeft: lr.left };
  }

  // ------------------------------------------------------------------ caret
  // Rest on left/top, travel on transform. A transformed box is rasterised in
  // its layer's own space, and #page's left edge is not on a whole device pixel
  // (centred column + reserved scrollbar gutter), so a caret parked on a
  // transform picks up a grey column on each edge. left/top paints it exactly
  // where we put it; the transform is only ever a delta that ends back at zero.
  function setBase(x, y) {
    if (baseX !== x) { caret.style.left = x + 'px'; baseX = x; }
    if (baseY !== y) { caret.style.top = y + 'px'; baseY = y; }
  }
  function settle(x, y) {
    caret.style.transitionDuration = '0s';
    if (moving) { caret.style.transform = 'none'; moving = false; }
    setBase(x, y);
  }
  function moveTo(x, y, dur) {
    clearTimeout(settleTimer);
    if (!dur) { settle(x, y); return; }
    caret.style.transitionDuration = dur + 'ms';
    caret.style.transform = 'translate(' + (x - baseX) + 'px,' + (y - baseY) + 'px)';
    moving = true;
    settleTimer = setTimeout(() => settle(x, y), dur + 24);
  }

  function placeCaret(off) {
    const r = W.offsetRect(off, true);
    if (!r) return;
    const { line } = W.offsetToPos(off);
    const el = W.lineEl(line);
    if (!el) return;
    const b = bandTop(el, r);
    const pr = page.getBoundingClientRect();
    const x = snap(r.left + M.em * NUDGE_X) - pr.left;
    const y = snap(b.top + M.base - ABOVE * M.pitch) - pr.top;
    const h = snap(M.pitch);

    const now = performance.now();
    let dur = 0;
    if (prev) {
      const dx = Math.abs(x - prev.x), dy = Math.abs(y - prev.y);
      // Glide only where a glide reads as the same caret moving. Long jumps and
      // fast typing snap, so the caret is never behind the letter you just hit.
      if (dy <= M.pitch * 1.2 && dx <= M.em * 14 && now - prevAt >= SNAP_MS) {
        dur = dy > 0.5 ? GLIDE_Y : GLIDE_X;
      }
    }
    if (caret._h !== h) { caret.style.height = h + 'px'; caret._h = h; }
    if (caret._w !== M.w) { caret.style.width = M.w + 'px'; caret._w = M.w; }
    moveTo(x, y, dur);
    prev = { x, y }; prevAt = now;
  }

  function poke() {
    caret.classList.remove('blink');
    clearTimeout(idleTimer);
    idleTimer = setTimeout(() => {
      // shoot.mjs freezes the caret by pinning an inline opacity; respect it.
      if (caret.style.opacity === '') caret.classList.add('blink');
    }, IDLE_MS);
  }

  // -------------------------------------------------------------- selection
  function rangeFor(el, from, to) {
    const walk = document.createTreeWalker(el, NodeFilter.SHOW_TEXT);
    const r = document.createRange();
    let acc = 0, n, started = false;
    while ((n = walk.nextNode())) {
      const len = n.data.length;
      if (!started && acc + len >= from) { r.setStart(n, from - acc); started = true; }
      if (acc + len >= to) { if (!started) r.setStart(n, 0); r.setEnd(n, to - acc); return r; }
      acc += len;
    }
    if (!started) { r.selectNodeContents(el); return r; }
    r.setEndAfter(el.lastChild || el);
    return r;
  }

  function takeRect(i) {
    let d = pool[i];
    if (!d) { d = pool[i] = document.createElement('div'); d.className = 'sel'; selLayer.appendChild(d); }
    return d;
  }

  function drawSelection(start, end) {
    const pr = page.getBoundingClientRect();
    const vr = scroller.getBoundingClientRect();
    const top = vr.top - M.pitch * 6, bottom = vr.bottom + M.pitch * 6;
    const a = W.offsetToPos(start), b = W.offsetToPos(end);
    const lines = W.lines();
    const dy = M.base - ABOVE * M.pitch;
    let n = 0, firstEdge = null, lastEdge = null;

    for (let li = a.line; li <= b.line && n < MAX_ROWS; li++) {
      const el = W.lineEl(li);
      if (!el) continue;
      const lr = el.getBoundingClientRect();
      if (lr.bottom < top) {
        // Skip whole screenfuls without touching the DOM for each line.
        const skip = Math.floor((top - lr.bottom) / M.pitch);
        if (skip > 1) li += skip - 1;
        continue;
      }
      if (lr.top > bottom) break;

      const text = lines[li] != null ? lines[li] : '';
      const from = li === a.line ? a.col : 0;
      const to = li === b.line ? b.col : text.length;
      const rows = new Map();
      if (text.length && to > from) {
        const rects = rangeFor(el, from, to).getClientRects();
        for (let i = 0; i < rects.length; i++) {
          const cr = rects[i];
          if (!cr.height) continue;
          const k = Math.max(0, Math.floor((cr.top + cr.height / 2 - lr.top) / M.pitch));
          const cur = rows.get(k);
          if (cur) { if (cr.left < cur.l) cur.l = cr.left; if (cr.right > cur.r) cur.r = cr.right; }
          else rows.set(k, { l: cr.left, r: cr.right });
        }
      }
      if (!rows.size) rows.set(0, { l: lr.left, r: lr.left });
      const keys = [...rows.keys()].sort((p, q) => p - q);
      // A selected line break has no glyph; give it a short tail so the reader
      // can see the break is inside the selection.
      if (li < b.line) { const m = rows.get(keys[keys.length - 1]); m.r += M.em * NL_TAIL; }

      for (const k of keys) {
        if (n >= MAX_ROWS) break;
        const m = rows.get(k);
        const d = takeRect(n++);
        const x = snap(m.l) - pr.left, y = snap(lr.top + k * M.pitch + dy) - pr.top;
        d.style.left = x + 'px'; d.style.top = y + 'px';
        d.style.width = Math.max(0, snap(m.r) - snap(m.l)) + 'px';
        d.style.height = snap(M.pitch) + 'px';
        d.style.display = '';
        if (li === a.line && k === keys[0]) firstEdge = { x: x, y: y };
        if (li === b.line) lastEdge = { x: snap(m.r) - pr.left, y: y };
      }
    }
    for (let i = n; i < pool.length; i++) pool[i].style.display = 'none';

    // Two caret-width bars bracket the selection: the same instrument at both
    // ends, so the eye can see exactly which cells are held.
    setEdge(edgeA, firstEdge, -M.w);
    setEdge(edgeB, lastEdge, 0);
  }

  function setEdge(el, p, dx) {
    if (!p) { el.style.display = 'none'; return; }
    el.style.display = '';
    el.style.width = M.w.toFixed(2) + 'px';
    el.style.height = snap(M.pitch) + 'px';
    el.style.left = (p.x + dx) + 'px'; el.style.top = p.y + 'px';
  }

  function clearSelection() {
    if (!hasSel) return;
    for (let i = 0; i < pool.length; i++) pool[i].style.display = 'none';
    edgeA.style.display = 'none';
    edgeB.style.display = 'none';
  }

  // ------------------------------------------------------------------ place
  function place() {
    if (!caret) return;
    const s = W.selection();
    const focused = document.activeElement === input;
    const collapsed = s.start === s.end;
    layer.classList.toggle('idle', !focused);
    selLayer.classList.toggle('idle', !focused);

    if (collapsed) {
      clearSelection(); hasSel = false;
      caret.style.display = '';
      placeCaret(s.dir === 'backward' ? s.start : s.end);
      poke();
    } else {
      caret.style.display = 'none';
      caret.classList.remove('blink');
      clearTimeout(idleTimer);
      prev = null;
      drawSelection(s.start, s.end);
      hasSel = true;
    }
  }

  function onScroll() {
    if (!hasSel || scrollRaf) return;
    scrollRaf = requestAnimationFrame(() => { scrollRaf = 0; if (hasSel) place(); });
  }

  function remeasure() { measure(); prev = null; place(); }

  W.on('boot', () => {
    layer = W.el.caretLayer; page = W.el.page; input = W.el.input;
    mirror = W.el.mirror; scroller = W.el.scroller;

    selLayer = document.createElement('div');
    selLayer.id = 'sel-layer';
    selLayer.setAttribute('aria-hidden', 'true');
    page.insertBefore(selLayer, page.firstChild);   // paints under #mirror's ink

    probe = document.createElement('div');
    probe.className = 'caret-probe';
    probe.textContent = 'x';
    probeSpan = document.createElement('span');
    probe.appendChild(probeSpan);
    layer.appendChild(probe);

    caret = document.createElement('div');
    caret.className = 'caret';
    layer.appendChild(caret);
    edgeA = document.createElement('div'); edgeA.className = 'sel-edge'; edgeA.style.display = 'none';
    edgeB = document.createElement('div'); edgeB.className = 'sel-edge'; edgeB.style.display = 'none';
    layer.appendChild(edgeA); layer.appendChild(edgeB);

    measure();
    W.on('selection', place);
    W.on('render', place);
    W.on('change', poke);
    W.on('resize', remeasure);
    W.on('settings', remeasure);
    W.on('focus', () => { prev = null; place(); });
    W.on('blur', place);
    W.on('scroll', onScroll);
    if (document.fonts) document.fonts.addEventListener('loadingdone', remeasure);
  });

  W.placeCaret = place;
  W.caretMetrics = M;
})();
