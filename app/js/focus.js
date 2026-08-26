/* Focus mode (sentence / paragraph) and typewriter scrolling. Owner: focus piece.
 *
 * Three tiers of ink, not two:
 *   bright — the active sentence (or the active paragraph in paragraph mode)
 *   near   — sentence mode only: the sentence just before and just after the active one.
 *            iA drops those to the same grey as text three paragraphs away, which is the one
 *            thing writers complain about: you lose the sentence you are building on.
 *            A single quiet step keeps it legible without pulling the eye off the active line.
 *   dim    — everything else (#cccccc on light paper, #707070 on dark: iA's measured values)
 *
 * Typewriter scrolling is a retargetable rAF ease rather than `scroll-behavior: smooth`, so
 * successive caret moves blend instead of stuttering, and pointer-driven caret moves only
 * nudge the line back into a comfortable band instead of yanking the whole page (the
 * "screen jumping vertically" that iA's own support page warns about).
 */
(function () {
  const W = window.Writer;

  // ---------- sentence & paragraph geometry ----------

  // Candidate terminator: . ! ? … possibly repeated, optional closing quotes/brackets, then space.
  const TERM = /[.!?…]+["'”’»)\]]*(\s+|$)/g;
  // Words that end in a period but do not end a sentence.
  const ABBR = /(?:^|[\s("'“‘[])(?:mr|mrs|ms|dr|prof|rev|sr|jr|st|vs|etc|eg|e\.g|ie|i\.e|cf|al|fig|no|nos|vol|ch|pp|approx|dept|est|inc|ltd|co|univ|apt|min|max|jan|feb|mar|apr|jun|jul|aug|sep|sept|oct|nov|dec|mon|tue|wed|thu|fri|sat|sun)\.$/i;
  // A lone initial: "J. R. R. Tolkien".
  const INITIAL = /(?:^|[\s("'“‘[])[A-Za-z]\.$/;
  // Ordered-list / numbered-heading marker: "# 1. Down the Rabbit Hole".
  const ORDINAL = /^[#>\s\-*+]*\d+\.$/;

  function isRealBreak(text, before, after) {
    const head = text.slice(0, before);
    if (ABBR.test(head) || INITIAL.test(head) || ORDINAL.test(head)) return false;
    const rest = text.slice(after);
    if (!rest) return true;
    // A sentence does not restart on a lowercase letter ("... at 5 p.m. and then left").
    const c = rest.match(/^\S/);
    return !c || !/[a-z]/.test(c[0]);
  }

  // All sentence spans of one line, covering the whole line.
  function sentences(text) {
    const out = [];
    let start = 0, m;
    TERM.lastIndex = 0;
    while ((m = TERM.exec(text))) {
      const before = m.index + m[0].replace(/\s+$/, '').length;   // end of the punctuation
      const after = m.index + m[0].length;                        // start of the next sentence
      if (after >= text.length) break;
      if (!isRealBreak(text, before, after)) continue;
      out.push({ from: start, to: after });
      start = after;
    }
    out.push({ from: start, to: text.length });
    return out;
  }
  function sentenceIndex(spans, col) {
    for (let i = 0; i < spans.length; i++) if (col < spans[i].to) return i;
    return spans.length - 1;
  }

  // A line that starts its own block (heading, quote, list item, fence, table row, rule).
  const BLOCK = /^\s{0,3}(?:#{1,6}(\s|$)|>|[-*+][ \t]|\d+[.)][ \t]|```|~~~|\||-{3,}$|={3,}$)/;
  const isBlock = (s) => BLOCK.test(s);

  function paragraphBounds(lines, li) {
    if (!lines[li] || !lines[li].trim()) return [li, li];
    let a = li, b = li;
    while (a > 0 && lines[a - 1].trim() && !isBlock(lines[a])) a--;
    while (b < lines.length - 1 && lines[b + 1].trim() && !isBlock(lines[b + 1])) b++;
    return [a, b];
  }

  // ---------- focus state ----------

  // tiers: 2 = bright, 1 = near, 0 = dim
  let mode = 'off';
  let bright = [];          // [{line,from,to}]
  let near = [];            // [{line,from,to}]
  let tier = new Map();     // line -> highest tier present on that line
  let sig = '';             // change signature, so typing inside the active sentence is free
  let quiet = true;         // suppress the cross-fade for the first paint / a swapped document

  function spansForLine(i, len) {
    if (mode === 'off' || !len) return null;
    const b = [], n = [];
    for (const r of bright) if (r.line === i) b.push(r);
    for (const r of near) if (r.line === i) n.push(r);
    if (!b.length && !n.length) return [{ from: 0, to: len, cls: 'dim' }];
    const marked = [];
    for (const r of b) marked.push({ from: Math.max(0, r.from), to: Math.min(len, r.to), cls: '' });
    for (const r of n) marked.push({ from: Math.max(0, r.from), to: Math.min(len, r.to), cls: 'near' });
    marked.sort((x, y) => x.from - y.from);
    const out = [];
    let pos = 0;
    for (const s of marked) {
      if (s.to <= pos) continue;
      if (s.from > pos) out.push({ from: pos, to: s.from, cls: 'dim' });
      if (s.cls) out.push({ from: Math.max(s.from, pos), to: s.to, cls: s.cls });
      pos = s.to;
    }
    if (pos < len) out.push({ from: pos, to: len, cls: 'dim' });
    return out;
  }
  W.addDecorator((i, text) => spansForLine(i, text.length));

  function computeTiers() {
    const t = new Map();
    for (const r of near) t.set(r.line, Math.max(t.get(r.line) || 0, 1));
    for (const r of bright) t.set(r.line, 2);
    return t;
  }

  function update(opts) {
    const next = W.settings.focus;
    const modeChanged = next !== mode;
    mode = next;
    const prevTier = tier;
    const prevBright = bright, prevNear = near;
    bright = []; near = [];

    if (mode !== 'off') {
      const lines = W.lines();
      const sel = W.selection();
      const { line, col } = W.offsetToPos(sel.start);
      if (mode === 'paragraph') {
        const [a, b] = paragraphBounds(lines, line);
        for (let i = a; i <= b; i++) bright.push({ line: i, from: 0, to: lines[i].length });
      } else if (sel.end > sel.start) {
        // A live selection is one focused span: while you are marking a phrase, the phrase
        // is what you are working on.
        const s = W.offsetToPos(sel.start), e = W.offsetToPos(sel.end);
        for (let i = s.line; i <= e.line; i++) {
          bright.push({ line: i, from: i === s.line ? s.col : 0, to: i === e.line ? e.col : lines[i].length });
        }
      } else if (!(lines[line] || '').trim()) {
        // Caret parked on a blank line — a new thought. Nothing is active, but do not black
        // the page out: keep the sentence you just finished readable behind you.
        for (let i = line - 1; i >= 0; i--) {
          if (!lines[i].trim()) { if (i < line - 1) break; else continue; }
          const s = sentences(lines[i]);
          near.push({ line: i, from: s[s.length - 1].from, to: lines[i].length }); break;
        }
      } else {
        const spans = sentences(lines[line] || '');
        const k = sentenceIndex(spans, col);
        bright.push({ line, from: spans[k].from, to: spans[k].to });
        const [pa, pb] = paragraphBounds(lines, line);
        // previous sentence: same line, else the last sentence of the line above in this block
        if (k > 0) near.push({ line, from: spans[k - 1].from, to: spans[k - 1].to });
        else for (let i = line - 1; i >= pa; i--) {
          if (!lines[i].length) continue;
          const s = sentences(lines[i]);
          near.push({ line: i, from: s[s.length - 1].from, to: lines[i].length }); break;
        }
        // next sentence: same line, else the first sentence of the line below in this block
        if (k < spans.length - 1) near.push({ line, from: spans[k + 1].from, to: spans[k + 1].to });
        else for (let i = line + 1; i <= pb; i++) {
          if (!lines[i].length) continue;
          const s = sentences(lines[i]);
          near.push({ line: i, from: 0, to: s[0].to }); break;
        }
      }
    }
    tier = computeTiers();

    let nsig = mode;
    for (const r of bright) nsig += '|' + r.line + ':' + r.from + '-' + r.to;
    nsig += '#';
    for (const r of near) nsig += '|' + r.line + ':' + r.from + '-' + r.to;
    if (!modeChanged && nsig === sig && !(opts && opts.force)) return;
    sig = nsig;

    if (modeChanged) { W.render(true); flash(prevTier, tier); return; }

    const changed = new Set();
    for (const r of prevBright) changed.add(r.line);
    for (const r of prevNear) changed.add(r.line);
    for (const r of bright) changed.add(r.line);
    for (const r of near) changed.add(r.line);
    W.rerenderLines(changed);
    flash(prevTier, tier);
  }

  // Cross-fade the lines whose tier changed. fillLine() rewrites className, so these
  // one-shot classes clear themselves on that line's next render.
  function flash(prev, now) {
    if (quiet) { quiet = false; return; }
    const seen = new Set();
    prev.forEach((_, i) => seen.add(i));
    now.forEach((_, i) => seen.add(i));
    for (const i of seen) {
      const a = prev.get(i) || 0, b = now.get(i) || 0;
      if (a === b) continue;
      const el = W.lineEl(i); if (!el) continue;
      el.classList.remove('fx-in', 'fx-out');
      void el.offsetWidth;
      el.classList.add(b > a ? 'fx-in' : 'fx-out');
    }
  }

  // ---------- typewriter scrolling ----------

  let raf = 0, pointerAt = -1e9, instantNext = true;

  function reducedMotion() {
    return !!(window.matchMedia && window.matchMedia('(prefers-reduced-motion: reduce)').matches);
  }
  function cancel() { if (raf) { cancelAnimationFrame(raf); raf = 0; } }

  function glide(sc, top, dur) {
    top = Math.max(0, Math.min(top, sc.scrollHeight - sc.clientHeight));
    cancel();
    const from = sc.scrollTop, dist = top - from;
    if (Math.abs(dist) < 0.5) return;
    if (dur <= 0 || reducedMotion()) { sc.scrollTop = top; return; }
    const t0 = performance.now();
    const step = (t) => {
      const k = Math.min(1, (t - t0) / dur);
      const e = 1 - Math.pow(1 - k, 3);          // ease-out cubic: leaves fast, lands soft
      sc.scrollTop = from + dist * e;
      raf = k < 1 ? requestAnimationFrame(step) : 0;
    };
    raf = requestAnimationFrame(step);
  }

  function typewriter(animate) {
    const sc = W.el.scroller; if (!sc) return;
    const tw = !!W.settings.typewriter;
    if (!tw && W.settings.focus === 'off') return;
    const r = W.caretRect(); if (!r) return;
    const y = r.top - sc.getBoundingClientRect().top + r.height / 2;   // caret centre in the viewport
    const h = sc.clientHeight;
    const lh = r.height || 24;

    let want;
    if (tw) {
      const pos = W.settings.typewriterLine;
      const anchor = h * (typeof pos === 'number' ? pos : 0.5);
      if (performance.now() - pointerAt < 400) {
        // Clicking somewhere should not throw the page across the screen. Pull the line in
        // only if it sits near an edge, and then only as far as the nearest band edge.
        const lo = Math.max(lh * 1.5, h * 0.18), hi = Math.min(h - lh * 1.5, h * 0.82);
        if (y >= lo && y <= hi) return;
        want = y < lo ? lo : hi;
      } else want = anchor;
    } else {
      // Focus mode without typewriter: keep the active line off the edges, nothing more.
      const lo = Math.max(lh * 2, h * 0.15), hi = Math.min(h - lh * 2, h * 0.85);
      if (y >= lo && y <= hi) return;
      want = y < lo ? lo : hi;
    }

    const delta = y - want;
    if (Math.abs(delta) < 0.5) return;
    const target = sc.scrollTop + delta;
    if (!animate || instantNext) { instantNext = false; glide(sc, target, 0); return; }
    const d = Math.abs(delta);
    // One line of travel (you just started a new line) is the common case: keep it quick.
    const dur = d <= lh * 1.6 ? 120 : Math.min(280, 110 + Math.sqrt(d) * 7);
    glide(sc, target, dur);
  }

  // ---------- wiring ----------

  W.on('boot', () => {
    const input = W.el.input, sc = W.el.scroller;
    input.addEventListener('pointerdown', () => { pointerAt = performance.now(); cancel(); });
    input.addEventListener('pointerup', () => { pointerAt = performance.now(); });
    // Never fight a reader who is scrolling by hand.
    sc.addEventListener('wheel', cancel, { passive: true });
    sc.addEventListener('touchstart', cancel, { passive: true });

    W.on('selection', () => { update(); typewriter(true); });
    W.on('change', (e) => { if (!e || e.source !== 'input') { instantNext = true; quiet = true; } });
    W.on('settings', (k) => {
      if (k === 'focus' || k === 'typewriter' || k === 'typewriterLine') {
        update({ force: k === 'focus' });
        instantNext = true; typewriter(false);
      }
    });
    W.on('resize', () => { instantNext = true; typewriter(false); });
    W.on('ready', () => { quiet = true; update({ force: true }); instantNext = true; typewriter(false); });

    let lastOn = W.settings.focus === 'off' ? 'sentence' : W.settings.focus;
    W.registerCommand('focus.toggle', {
      title: 'Focus Mode', keys: 'Mod+D',
      run: () => {
        if (W.settings.focus === 'off') W.setSetting('focus', lastOn);
        else { lastOn = W.settings.focus; W.setSetting('focus', 'off'); }
      }
    });
    W.registerCommand('focus.sentence', { title: 'Focus: Sentence', run: () => { lastOn = 'sentence'; W.setSetting('focus', 'sentence'); } });
    W.registerCommand('focus.paragraph', { title: 'Focus: Paragraph', run: () => { lastOn = 'paragraph'; W.setSetting('focus', 'paragraph'); } });
    W.registerCommand('typewriter.toggle', { title: 'Typewriter Mode', keys: 'Mod+T', run: () => W.setSetting('typewriter', !W.settings.typewriter) });
  });

  // exposed for other pieces / tests
  W.focus = { sentences, paragraphBounds };
})();
