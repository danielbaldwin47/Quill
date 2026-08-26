/* Custom caret. Owner: caret piece.
 * Hides the native caret (css) and draws our own in #caret-layer, positioned via Writer.caretRect().
 */
(function () {
  const W = window.Writer;
  let caret, layer, page, input, blinkTimer, visible = true;
  function place() {
    if (!caret) return;
    const r = W.caretRect();
    if (!r) return;
    const pr = page.getBoundingClientRect();
    caret.style.transform = `translate(${r.left - pr.left}px, ${r.top - pr.top}px)`;
    caret.style.height = r.height + 'px';
    // restart blink on movement
    caret.classList.remove('blink'); void caret.offsetWidth; caret.classList.add('blink');
    caret.classList.toggle('hidden', document.activeElement !== input);
  }
  W.on('boot', () => {
    layer = W.el.caretLayer; page = W.el.page; input = W.el.input;
    caret = document.createElement('div'); caret.className = 'caret blink'; layer.appendChild(caret);
    W.on('selection', place); W.on('render', place); W.on('resize', place); W.on('settings', place);
    W.on('focus', place); W.on('blur', place);
  });
  W.placeCaret = place;
})();
