/* Markup (Markdown) tokenizer for the mirror. Owner: markup piece.
 * Writer.tokenizeLine(text, ctx, i) -> tokens [{text, cls}] (may carry .lineClass)
 * Writer.lineContext(prevCtx, text, i) -> ctx entering the NEXT line ({inFence, fenceChar} etc.)
 * Rule: markup must never change glyph metrics — only color/weight/opacity/decoration.
 */
(function () {
  const W = window.Writer;
  W.lineContext = function (prev, text) {
    const ctx = prev || {};
    const m = /^\s{0,3}(`{3,}|~{3,})/.exec(text);
    if (ctx.inFence) {
      if (m && m[1][0] === ctx.fenceChar && m[1].length >= ctx.fenceLen) return { inFence: false };
      return ctx;
    }
    if (m) return { inFence: true, fenceChar: m[1][0], fenceLen: m[1].length, lineClass: 'in-fence' };
    return ctx.lineClass ? {} : ctx;
  };
  function inlineTokens(text, base) {
    const out = [];
    let i = 0, n = text.length, buf = '';
    const flush = () => { if (buf) { out.push({ text: buf, cls: base }); buf = ''; } };
    while (i < n) {
      const c = text[i];
      if (c === '\\' && i + 1 < n) { buf += c + text[i + 1]; i += 2; continue; }
      if (c === '`') {
        const m = /^(`+)([^`]|[^`][\s\S]*?[^`])\1(?!`)/.exec(text.slice(i));
        if (m) { flush(); out.push({ text: m[1], cls: base + ' md-mark' }); out.push({ text: m[2], cls: base + ' md-code' }); out.push({ text: m[1], cls: base + ' md-mark' }); i += m[0].length; continue; }
      }
      if (c === '*' || c === '_') {
        const m = /^(\*\*\*|___|\*\*|__|\*|_)(?=\S)([\s\S]*?\S)\1(?![*_\w])/.exec(text.slice(i));
        if (m && !(c === '_' && i > 0 && /\w/.test(text[i - 1]))) {
          flush();
          const cls = m[1].length === 3 ? ' md-strong md-em' : m[1].length === 2 ? ' md-strong' : ' md-em';
          out.push({ text: m[1], cls: base + ' md-mark' });
          for (const t of inlineTokens(m[2], base + cls)) out.push(t);
          out.push({ text: m[1], cls: base + ' md-mark' });
          i += m[0].length; continue;
        }
      }
      if (c === '~' && text[i + 1] === '~') {
        const m = /^~~(?=\S)([\s\S]*?\S)~~/.exec(text.slice(i));
        if (m) { flush(); out.push({ text: '~~', cls: base + ' md-mark' }); out.push({ text: m[1], cls: base + ' md-strike' }); out.push({ text: '~~', cls: base + ' md-mark' }); i += m[0].length; continue; }
      }
      if (c === '!' && text[i + 1] === '[' || c === '[') {
        const m = /^(!?\[)([^\]]*)(\]\()([^)\s]*)(\))/.exec(text.slice(i));
        if (m) { flush(); out.push({ text: m[1], cls: base + ' md-mark' }); out.push({ text: m[2], cls: base + ' md-link-text' }); out.push({ text: m[3], cls: base + ' md-mark' }); out.push({ text: m[4], cls: base + ' md-url' }); out.push({ text: m[5], cls: base + ' md-mark' }); i += m[0].length; continue; }
      }
      if (c === '<') {
        const m = /^<(https?:\/\/[^>\s]+|[^@\s>]+@[^@\s>]+)>/.exec(text.slice(i));
        if (m) { flush(); out.push({ text: m[0], cls: base + ' md-url' }); i += m[0].length; continue; }
      }
      if (c === 'h' && /^https?:\/\/\S+/.test(text.slice(i))) {
        const m = /^https?:\/\/[^\s<>()]+/.exec(text.slice(i));
        flush(); out.push({ text: m[0], cls: base + ' md-url' }); i += m[0].length; continue;
      }
      buf += c; i++;
    }
    flush();
    return out;
  }
  W.tokenizeLine = function (text, ctx) {
    if (ctx && ctx.inFence) {
      const m = /^\s{0,3}(`{3,}|~{3,})/.exec(text);
      const t = [{ text, cls: m && m[1][0] === ctx.fenceChar ? 'md-mark md-fence' : 'md-codeblock' }];
      t.lineClass = 'l-codeblock'; return t;
    }
    let m;
    if ((m = /^\s{0,3}(`{3,}|~{3,})(.*)$/.exec(text))) { const t = [{ text: m[1], cls: 'md-mark md-fence' }, { text: m[2], cls: 'md-fence-info' }]; t.lineClass = 'l-codeblock l-fence'; return t; }
    if ((m = /^(\s{0,3}#{1,6})(\s+)(.*?)(\s+#+\s*)?$/.exec(text))) {
      const lvl = m[1].trim().length;
      const t = [{ text: m[1], cls: 'md-mark md-hmark' }, { text: m[2], cls: 'md-hmark-space' }, ...inlineTokens(m[3], 'md-h md-h' + lvl)];
      if (m[4]) t.push({ text: m[4], cls: 'md-mark' });
      t.lineClass = 'l-h l-h' + lvl; return t;
    }
    if (/^\s{0,3}([-*_])(\s*\1){2,}\s*$/.test(text)) { const t = [{ text, cls: 'md-mark md-hr' }]; t.lineClass = 'l-hr'; return t; }
    if ((m = /^(\s*>\s?)+/.exec(text))) { const t = [{ text: m[0], cls: 'md-mark md-quote-mark' }, ...inlineTokens(text.slice(m[0].length), 'md-quote')]; t.lineClass = 'l-quote'; return t; }
    if ((m = /^(\s*)([-*+]|\d{1,9}[.)])(\s+)(\[[ xX]\]\s+)?/.exec(text))) {
      const t = [{ text: m[1] + m[2], cls: 'md-mark md-bullet' }, { text: m[3], cls: '' }];
      if (m[4]) t.push({ text: m[4], cls: 'md-mark md-task' + (/x/i.test(m[4]) ? ' md-task-done' : '') });
      t.push(...inlineTokens(text.slice(m[0].length), 'md-li' + (m[4] && /x/i.test(m[4]) ? ' md-done' : '')));
      t.lineClass = 'l-li'; return t;
    }
    if (/^(\t| {4})/.test(text)) { const t = [{ text, cls: 'md-codeblock' }]; t.lineClass = 'l-codeblock l-indent'; return t; }
    if ((m = /^\s{0,3}\[\^?[^\]]+\]:\s*/.exec(text))) { return [{ text: m[0], cls: 'md-mark' }, { text: text.slice(m[0].length), cls: 'md-url' }]; }
    return inlineTokens(text, '');
  };
})();
