/* Markup (Markdown) tokenizer for the mirror. Owner: markup piece.
 * Writer.tokenizeLine(text, ctx, i) -> tokens [{text, cls}] (may carry .lineClass)
 * Writer.lineContext(prevCtx, text, i) -> ctx entering the NEXT line ({inFence, ...})
 *
 * Rules obeyed here:
 *  - markup never changes glyph metrics: no font-family swap, no size, no letter-spacing,
 *    no padding/margin inside a line. Only colour, weight, italic, decoration, background.
 *  - the mirror's text is character-for-character the textarea's text, so #input and #mirror
 *    stay glyph-aligned (offsets, caret, selection and click mapping all keep working).
 *  - no work per keystroke beyond re-tokenising the lines that actually changed.
 */
(function () {
  const W = window.Writer;

  const RE_FENCE = /^[ \t]{0,3}(`{3,}|~{3,})[ \t]*(.*)$/;
  const RE_HEAD = /^([ \t]{0,3}#{1,6})([ \t]+|$)(.*?)([ \t]+#+[ \t]*)?$/;
  const RE_HR = /^[ \t]{0,3}([-*_])[ \t]*(?:\1[ \t]*){2,}$/;
  const RE_QUOTE = /^[ \t]{0,3}(?:>[ \t]?)+/;
  const RE_LIST = /^([ \t]*)([-*+]|\d{1,9}[.)])([ \t]+)(\[([ xX])\][ \t]+)?/;
  const RE_INDENT_CODE = /^(?:\t| {4})[ \t]*\S/;
  const RE_TABLE = /^[ \t]{0,3}\|/;
  const RE_TABLE_SEP = /^[ \t]{0,3}\|?[ \t]*:?-{1,}:?[ \t]*(?:\|[ \t]*:?-*:?[ \t]*)*\|?[ \t]*$/;
  const RE_DEF = /^[ \t]{0,3}(\[\^?[^\]\s][^\]]*\]:)([ \t]*)/;
  const RE_FRONT_OPEN = /^---[ \t]*$/;
  const RE_FRONT_CLOSE = /^(?:---|\.\.\.)[ \t]*$/;
  const RE_FRONT_KEY = /^([ \t]*[A-Za-z_][\w .-]*:)([ \t]*)/;
  // A lone file path / media reference on its own line = content block ("chip"), as in iA Writer.
  const RE_CHIP = /^([ \t]*)((?:\.{0,2}\/|~\/|[A-Za-z]:[\\/])?[\w][\w\-.()/\\]*\.(?:png|jpe?g|gif|webp|svg|heic|pdf|mp4|m4v|mov|mp3|m4a|wav|csv|md|markdown|html?))([ \t]*)$/i;

  // ---------------- per-line context (fences, front matter) ----------------
  W.lineContext = function (prev, text, i) {
    const ctx = prev || null;
    if (ctx && ctx.inFront) return RE_FRONT_CLOSE.test(text) ? {} : ctx;
    if (ctx && ctx.inFence) {
      const c = RE_FENCE.exec(text);
      if (c && c[1][0] === ctx.fenceChar && c[1].length >= ctx.fenceLen) return {};
      return ctx;
    }
    if (i === 0 && RE_FRONT_OPEN.test(text)) return { inFront: true };
    const m = RE_FENCE.exec(text);
    if (m) return { inFence: true, fenceChar: m[1][0], fenceLen: m[1].length };
    return {};
  };

  // ---------------- inline ----------------
  // `base` is the class inherited from the enclosing block (heading, list item, quote…).
  function inlineTokens(text, base, out) {
    out = out || [];
    let i = 0; const n = text.length; let buf = '';
    const push = (t, cls) => { if (t) out.push({ text: t, cls: cls }); };
    const flush = () => { if (buf) { push(buf, base); buf = ''; } };
    const mark = base + ' md-mark md-mark-inline';
    while (i < n) {
      const c = text[i];
      if (c === '\\' && i + 1 < n && /[\\`*_{}[\]()#+\-.!~|>]/.test(text[i + 1])) {
        flush(); push(c, mark); push(text[i + 1], base); i += 2; continue;
      }
      if (c === '`') {
        const m = /^(`+)(?!`)([\s\S]*?[^`])\1(?!`)/.exec(text.slice(i));
        if (m) {
          flush();
          push(m[1], mark + ' md-code-mark'); push(m[2], base + ' md-code'); push(m[1], mark + ' md-code-mark');
          i += m[0].length; continue;
        }
      }
      if (c === '=' && text[i + 1] === '=') {
        const m = /^==(?=\S)([\s\S]*?\S)==/.exec(text.slice(i));
        if (m) { flush(); push('==', mark + ' md-hl-mark'); inlineTokens(m[1], base + ' md-hl', out); push('==', mark + ' md-hl-mark'); i += m[0].length; continue; }
      }
      if (c === '*' || c === '_') {
        const m = /^(\*\*\*|___|\*\*|__|\*|_)(?=\S)([\s\S]*?\S)\1(?![*_])/.exec(text.slice(i));
        if (m && !(c === '_' && (/\w/.test(text[i - 1] || '') || /\w/.test(text[i + m[0].length] || '')))) {
          flush();
          const cls = m[1].length === 3 ? ' md-strong md-em' : m[1].length === 2 ? ' md-strong' : ' md-em';
          push(m[1], mark);                       // the marker itself stays upright and light
          inlineTokens(m[2], base + cls, out);
          push(m[1], mark);
          i += m[0].length; continue;
        }
      }
      if (c === '~' && text[i + 1] === '~') {
        const m = /^~~(?=\S)([\s\S]*?\S)~~/.exec(text.slice(i));
        if (m) { flush(); push('~~', mark + ' md-strike'); inlineTokens(m[1], base + ' md-strike', out); push('~~', mark + ' md-strike'); i += m[0].length; continue; }
      }
      if (c === '[' && text[i + 1] === '[') {            // wikilink [[Page]]
        const m = /^\[\[([^\]|]*)(\|[^\]]*)?\]\]/.exec(text.slice(i));
        if (m) {
          flush(); push('[[', mark);
          push(m[1], base + ' md-link-text');
          if (m[2]) { push('|', mark); push(m[2].slice(1), base + ' md-link-text'); }
          push(']]', mark); i += m[0].length; continue;
        }
      }
      if (c === '[' || (c === '!' && text[i + 1] === '[')) {
        let m = /^(!?\[)((?:[^[\]\\]|\\.)*)(\]\()([^()\s]*)((?:[ \t]+"[^"]*")?)(\))/.exec(text.slice(i));
        if (m) {
          flush();
          push(m[1], mark); inlineTokens(m[2], base + ' md-link-text', out); push(m[3], mark);
          push(m[4], base + ' md-url'); push(m[5], base + ' md-url-title'); push(m[6], mark);
          i += m[0].length; continue;
        }
        m = /^(!?\[)((?:[^[\]\\]|\\.)*)(\])(\[[^\]]*\])?/.exec(text.slice(i));   // [text][ref] / [^1]
        if (m && (m[4] || /^\[\^/.test(m[0]))) {
          flush();
          push(m[1], mark); inlineTokens(m[2], base + ' md-link-text', out); push(m[3], mark);
          if (m[4]) push(m[4], mark + ' md-url');
          i += m[0].length; continue;
        }
      }
      if (c === '<') {
        const m = /^<(?:https?:\/\/[^>\s]+|[^@\s<>]+@[^@\s<>]+)>/.exec(text.slice(i));
        if (m) { flush(); push('<', mark); push(m[0].slice(1, -1), base + ' md-url md-autolink'); push('>', mark); i += m[0].length; continue; }
        const h = /^<\/?[A-Za-z][\w-]*(?:\s[^<>]*)?>|^<!--[\s\S]*?-->/.exec(text.slice(i));   // inline html / comment
        if (h) { flush(); push(h[0], base + ' md-html'); i += h[0].length; continue; }
      }
      if ((c === 'h' || c === 'w') && /^(?:https?:\/\/|www\.)[^\s<>()[\]]/.test(text.slice(i))) {
        const m = /^(?:https?:\/\/|www\.)[^\s<>()[\]]+[^\s<>()[\].,;:!?'"]/.exec(text.slice(i));
        if (m) { flush(); push(m[0], base + ' md-url md-autolink'); i += m[0].length; continue; }
      }
      if (c === '#' && (i === 0 || /[\s(]/.test(text[i - 1]))) {             // #hashtag
        const m = /^#[A-Za-z][\w/-]*/.exec(text.slice(i));
        if (m) { flush(); push(m[0], base + ' md-tag'); i += m[0].length; continue; }
      }
      buf += c; i++;
    }
    flush();
    return out;
  }
  W.inlineTokens = inlineTokens;

  function tableTokens(text, base, out) {
    out = out || [];
    let seg = '', i = 0;
    const flush = () => { if (seg) { inlineTokens(seg, base, out); seg = ''; } };
    while (i < text.length) {
      const c = text[i];
      if (c === '\\' && text[i + 1] === '|') { seg += c + text[i + 1]; i += 2; continue; }
      if (c === '|') { flush(); out.push({ text: '|', cls: (base + ' md-mark md-pipe').trim() }); i++; continue; }
      seg += c; i++;
    }
    flush();
    return out;
  }

  // ---------------- line ----------------
  W.tokenizeLine = function (text, ctx, i) {
    let t, m;

    // fenced code block body / closing fence
    if (ctx && ctx.inFence) {
      m = RE_FENCE.exec(text);
      const closing = m && m[1][0] === ctx.fenceChar && m[1].length >= ctx.fenceLen;
      t = [{ text: text, cls: closing ? 'md-mark md-fence' : 'md-codeblock' }];
      t.lineClass = 'l-code' + (closing ? ' l-code-end' : '');
      return t;
    }
    // YAML front matter
    if (ctx && ctx.inFront) {
      if (RE_FRONT_CLOSE.test(text)) { t = [{ text: text, cls: 'md-mark md-front-rule' }]; t.lineClass = 'l-front l-front-end'; return t; }
      m = RE_FRONT_KEY.exec(text);
      t = m ? [{ text: m[1], cls: 'md-meta-key' }, { text: m[2], cls: '' }, { text: text.slice(m[0].length), cls: 'md-meta-val' }]
            : [{ text: text, cls: 'md-meta-val' }];
      t.lineClass = 'l-front';
      return t;
    }
    if (i === 0 && RE_FRONT_OPEN.test(text)) { t = [{ text: text, cls: 'md-mark md-front-rule' }]; t.lineClass = 'l-front l-front-start'; return t; }

    // opening fence
    if ((m = RE_FENCE.exec(text))) {
      const lead = text.slice(0, text.indexOf(m[1]));
      t = [{ text: lead, cls: '' }, { text: m[1], cls: 'md-mark md-fence' }, { text: text.slice(lead.length + m[1].length), cls: 'md-fence-info' }];
      t.lineClass = 'l-code l-code-start';
      return t;
    }
    // heading
    if ((m = RE_HEAD.exec(text))) {
      const lvl = m[1].trim().length;
      t = [{ text: m[1], cls: 'md-mark md-hmark' }, { text: m[2], cls: 'md-mark md-hmark' }];
      inlineTokens(m[3], 'md-h', t);
      if (m[4]) t.push({ text: m[4], cls: 'md-mark md-hmark' });
      t.lineClass = 'l-h l-h' + lvl;
      return t;
    }
    // thematic break
    if (RE_HR.test(text)) { t = [{ text: text, cls: 'md-mark md-hr' }]; t.lineClass = 'l-hr'; return t; }
    // block quote
    if ((m = RE_QUOTE.exec(text))) {
      const depth = (m[0].match(/>/g) || []).length;
      t = [{ text: m[0], cls: 'md-mark md-quote-mark' }];
      const rest = text.slice(m[0].length);
      const h = RE_HEAD.exec(rest);
      if (h) {
        t.push({ text: h[1] + h[2], cls: 'md-mark md-hmark' });
        inlineTokens(h[3], 'md-quote md-h', t);
        if (h[4]) t.push({ text: h[4], cls: 'md-mark md-hmark' });
      } else inlineTokens(rest, 'md-quote', t);
      t.lineClass = 'l-quote l-quote' + Math.min(depth, 3);
      return t;
    }
    // list item / task
    if ((m = RE_LIST.exec(text))) {
      const done = m[5] && m[5] !== ' ';
      t = [{ text: m[1], cls: '' }, { text: m[2], cls: 'md-mark md-bullet' }, { text: m[3], cls: '' }];
      if (m[4]) t.push({ text: m[4], cls: 'md-mark md-task' + (done ? ' md-task-done' : '') });
      inlineTokens(text.slice(m[0].length), 'md-li' + (done ? ' md-done' : ''), t);
      t.lineClass = 'l-li' + (m[4] ? ' l-task' + (done ? ' l-task-done' : '') : '');
      return t;
    }
    // indented code
    if (RE_INDENT_CODE.test(text)) { t = [{ text: text, cls: 'md-codeblock' }]; t.lineClass = 'l-code l-code-indent'; return t; }
    // table row
    if (RE_TABLE.test(text)) {
      const sep = RE_TABLE_SEP.test(text);
      t = sep ? [{ text: text, cls: 'md-mark md-table-sep' }] : tableTokens(text, '', []);
      t.lineClass = 'l-table' + (sep ? ' l-table-sep' : '');
      return t;
    }
    // link reference / footnote definition
    if ((m = RE_DEF.exec(text))) {
      t = [{ text: m[1], cls: 'md-mark md-def' }, { text: m[2], cls: '' }];
      inlineTokens(text.slice(m[0].length), 'md-url', t);
      return t;
    }
    // content block (a lone file reference)
    if ((m = RE_CHIP.exec(text))) {
      t = [{ text: m[1], cls: '' }, { text: m[2], cls: 'md-chip' }, { text: m[3], cls: '' }];
      t.lineClass = 'l-chip';
      return t;
    }
    return inlineTokens(text, '');
  };

  // ---------------- active line ----------------
  // Marks sit at a low contrast so the prose reads first; on the line the caret is in they come
  // back up so you can see exactly what you are editing. Pure class toggle on one line element —
  // no re-tokenising, no layout, no per-keystroke cost.
  let activeEl = null, activeLine = -1, queued = false;
  function paintActive() {
    queued = false;
    if (!W.selection) return;
    const li = W.offsetToPos(W.selection().start).line;
    const el = W.lineEl(li) || null;
    if (activeEl && activeEl !== el) activeEl.classList.remove('md-here');
    activeEl = el; activeLine = li;
    // idempotent: a line rebuilt by another plugin loses the class, so always re-add
    if (el) el.classList.add('md-here');
  }
  function schedule() { if (!queued) { queued = true; requestAnimationFrame(paintActive); } }
  W.on('boot', () => {
    W.on('selection', schedule);
    W.on('render', () => { activeEl = null; activeLine = -1; schedule(); });
  });
})();
