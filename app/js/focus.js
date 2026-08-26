/* Focus mode (sentence / paragraph) and typewriter scrolling. Owner: focus piece. */
(function () {
  const W = window.Writer;
  let focusLines = new Set();      // line indices currently un-dimmed (or partially)
  let focusRange = null;           // {line, from, to} for sentence mode
  let lastDecorated = new Set();

  function sentenceRange(text, col) {
    // find sentence boundaries around col in a single line of text
    const ends = /[.!?…]+["')\]]*\s+|\n/g;
    let start = 0, m;
    while ((m = ends.exec(text))) {
      const e = m.index + m[0].length;
      if (col < e) return { from: start, to: e };
      start = e;
    }
    return { from: start, to: text.length };
  }
  function paragraphLines(lines, li) {
    if (!lines[li] || !lines[li].trim()) return [li];
    let a = li, b = li;
    while (a > 0 && lines[a - 1].trim()) a--;
    while (b < lines.length - 1 && lines[b + 1].trim()) b++;
    const r = []; for (let i = a; i <= b; i++) r.push(i); return r;
  }
  W.addDecorator((i, text) => {
    const mode = W.settings.focus;
    if (mode === 'off') return null;
    if (!focusLines.has(i)) return text.length ? [{ from: 0, to: text.length, cls: 'dim' }] : null;
    if (mode === 'sentence' && focusRange && focusRange.line === i) {
      const r = [];
      if (focusRange.from > 0) r.push({ from: 0, to: focusRange.from, cls: 'dim' });
      if (focusRange.to < text.length) r.push({ from: focusRange.to, to: text.length, cls: 'dim' });
      return r;
    }
    return null;
  });
  function update() {
    const mode = W.settings.focus;
    const prev = new Set([...focusLines]);
    focusLines = new Set(); focusRange = null;
    if (mode !== 'off') {
      const sel = W.selection();
      const { line, col } = W.offsetToPos(sel.start);
      const lines = W.lines();
      if (mode === 'paragraph') for (const i of paragraphLines(lines, line)) focusLines.add(i);
      else { focusLines.add(line); focusRange = { line, ...sentenceRange(lines[line], col) }; }
    }
    // rerender lines whose state changed (+ current line for sentence movement)
    const changed = new Set();
    for (const i of prev) if (!focusLines.has(i)) changed.add(i);
    for (const i of focusLines) changed.add(i);
    if (mode !== W._lastFocusMode) { W._lastFocusMode = mode; W.render(true); return; }
    W.rerenderLines(changed);
  }
  // typewriter
  function typewriter(smooth) {
    if (!W.settings.typewriter) return;
    const r = W.caretRect(); if (!r) return;
    const sc = W.el.scroller;
    const target = sc.clientHeight * W.settings.typewriterLine;
    const delta = (r.top + r.height / 2) - (sc.getBoundingClientRect().top + target);
    if (Math.abs(delta) < 1) return;
    sc.scrollTo({ top: sc.scrollTop + delta, behavior: smooth ? 'smooth' : 'auto' });
  }
  W.on('boot', () => {
    W.on('selection', () => { update(); typewriter(true); });
    W.on('settings', (k) => { if (k === 'focus' || k === 'typewriter' || k === 'typewriterLine') { update(); typewriter(false); } });
    W.on('resize', () => typewriter(false));
    W.registerCommand('focus.toggle', { title: 'Focus Mode', keys: 'Mod+D', run: () => W.setSetting('focus', W.settings.focus === 'off' ? 'sentence' : 'off') });
    W.registerCommand('focus.sentence', { title: 'Focus: Sentence', run: () => W.setSetting('focus', 'sentence') });
    W.registerCommand('focus.paragraph', { title: 'Focus: Paragraph', run: () => W.setSetting('focus', 'paragraph') });
    W.registerCommand('typewriter.toggle', { title: 'Typewriter Mode', keys: 'Mod+T', run: () => W.setSetting('typewriter', !W.settings.typewriter) });
  });
})();
