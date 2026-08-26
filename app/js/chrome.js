/* Chrome: top bar, status bar, command palette. Owner: chrome piece. */
(function () {
  const W = window.Writer;
  function stats(text) {
    const words = (text.match(/[\p{L}\p{N}'’]+/gu) || []).length;
    const chars = text.length;
    const minutes = Math.max(1, Math.round(words / 250));
    return { words, chars, minutes };
  }
  W.on('boot', () => {
    const top = document.getElementById('chrome-top');
    const bottom = document.getElementById('chrome-bottom');
    top.innerHTML = `<div class="bar"><span class="title" id="doc-title">Untitled</span><span class="spacer"></span><button class="icon" data-cmd="palette.open" title="Commands (Ctrl+K)">⌘</button></div>`;
    bottom.innerHTML = `<div class="bar"><span id="stat-words"></span><span class="sep">·</span><span id="stat-time"></span><span class="spacer"></span><span id="stat-mode"></span></div>`;
    const upd = () => {
      const s = stats(W.getText());
      document.getElementById('stat-words').textContent = s.words + (s.words === 1 ? ' word' : ' words');
      document.getElementById('stat-time').textContent = s.minutes + ' min';
      const m = []; if (W.settings.focus !== 'off') m.push('Focus'); if (W.settings.typewriter) m.push('Typewriter');
      document.getElementById('stat-mode').textContent = m.join(' · ');
    };
    W.on('change', upd); W.on('settings', upd); upd();
    top.addEventListener('click', (e) => { const b = e.target.closest('[data-cmd]'); if (b) W.run(b.dataset.cmd); });
    // hide chrome while typing, show on mouse move
    let hideT; const root = document.documentElement;
    W.on('change', () => { root.dataset.typing = 'on'; clearTimeout(hideT); });
    document.addEventListener('mousemove', () => { root.dataset.typing = 'off'; });
    // command palette
    const overlays = document.getElementById('overlays');
    let pal;
    W.registerCommand('palette.open', { title: 'Command Palette', keys: ['Mod+K', 'Mod+Shift+P'], run: () => {
      if (pal) { pal.remove(); pal = null; W.el.input.focus(); return; }
      pal = document.createElement('div'); pal.className = 'palette';
      pal.innerHTML = `<input class="palette-input" placeholder="Type a command…"><ul class="palette-list"></ul>`;
      overlays.appendChild(pal);
      const inp = pal.querySelector('input'), list = pal.querySelector('ul');
      let sel = 0, items = [];
      const render = () => {
        const q = inp.value.toLowerCase();
        items = W.commands().filter(c => c.title.toLowerCase().includes(q));
        sel = Math.min(sel, Math.max(0, items.length - 1));
        list.innerHTML = items.map((c, i) => `<li class="${i === sel ? 'sel' : ''}" data-id="${c.id}"><span>${c.title}</span><kbd>${(Array.isArray(c.keys) ? c.keys[0] : c.keys || '').replace('Mod', W.isMac ? '⌘' : 'Ctrl')}</kbd></li>`).join('');
      };
      const close = () => { pal.remove(); pal = null; W.el.input.focus(); };
      inp.addEventListener('input', render);
      inp.addEventListener('keydown', (e) => {
        if (e.key === 'Escape') { e.preventDefault(); close(); }
        else if (e.key === 'ArrowDown') { e.preventDefault(); sel = (sel + 1) % items.length; render(); }
        else if (e.key === 'ArrowUp') { e.preventDefault(); sel = (sel - 1 + items.length) % items.length; render(); }
        else if (e.key === 'Enter') { e.preventDefault(); const c = items[sel]; close(); if (c) W.run(c.id); }
      });
      list.addEventListener('click', (e) => { const li = e.target.closest('li'); if (li) { const id = li.dataset.id; close(); W.run(id); } });
      render(); inp.focus();
    }});
    W.registerCommand('chrome.toggle', { title: 'Toggle Bars', keys: 'Mod+Shift+H', run: () => W.setSetting('showChrome', !W.settings.showChrome) });
    W.registerCommand('font.duo', { title: 'Font: Duo', run: () => W.setSetting('font', 'duo') });
    W.registerCommand('font.quattro', { title: 'Font: Quattro', run: () => W.setSetting('font', 'quattro') });
    W.registerCommand('font.mono', { title: 'Font: Mono', run: () => W.setSetting('font', 'mono') });
    W.registerCommand('font.bigger', { title: 'Bigger Text', keys: 'Mod+=', run: () => W.setSetting('fontSize', Math.min(40, W.settings.fontSize + 1)) });
    W.registerCommand('font.smaller', { title: 'Smaller Text', keys: 'Mod+-', run: () => W.setSetting('fontSize', Math.max(10, W.settings.fontSize - 1)) });
  });
})();
