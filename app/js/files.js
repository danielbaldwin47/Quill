/* File handling: autosave, open/save via File System Access API, library. Owner: files piece. */
(function () {
  const W = window.Writer;
  const KEY = 'quill.doc';
  let handle = null, saveT = null, dirty = false;
  function title() {
    const first = (W.getText().split('\n').find(l => l.trim()) || '').replace(/^#+\s*/, '').trim();
    return handle ? handle.name : (first || 'Untitled');
  }
  function persist() {
    try { localStorage.setItem(KEY, W.getText()); localStorage.setItem(KEY + '.sel', String(W.selection().start)); } catch (e) {}
    const t = document.getElementById('doc-title'); if (t) t.textContent = title() + (dirty && handle ? ' •' : '');
  }
  async function save(as) {
    if (!('showSaveFilePicker' in window)) { download(); return; }
    if (!handle || as) handle = await window.showSaveFilePicker({ suggestedName: title().replace(/[\\/:*?"<>|]/g, '') + '.md', types: [{ description: 'Markdown', accept: { 'text/markdown': ['.md', '.markdown', '.txt'] } }] }).catch(() => null);
    if (!handle) return;
    const w = await handle.createWritable(); await w.write(W.getText()); await w.close();
    dirty = false; persist();
  }
  async function open() {
    if (!('showOpenFilePicker' in window)) { const i = document.createElement('input'); i.type = 'file'; i.accept = '.md,.markdown,.txt'; i.onchange = async () => { const f = i.files[0]; if (f) W.setText(await f.text(), { source: 'open' }); }; i.click(); return; }
    const [h] = await window.showOpenFilePicker({ types: [{ description: 'Markdown', accept: { 'text/markdown': ['.md', '.markdown', '.txt'] } }] }).catch(() => []);
    if (!h) return; handle = h; const f = await h.getFile(); W.setText(await f.text(), { source: 'open' }); dirty = false; persist();
  }
  function download() {
    const a = document.createElement('a'); a.href = URL.createObjectURL(new Blob([W.getText()], { type: 'text/markdown' })); a.download = title() + '.md'; a.click();
  }
  W.on('boot', () => {
    try { const t = localStorage.getItem(KEY); if (t != null) { W.el.input.value = t; const s = +localStorage.getItem(KEY + '.sel') || 0; W.el.input.setSelectionRange(s, s); } } catch (e) {}
    W.on('change', () => { dirty = true; clearTimeout(saveT); saveT = setTimeout(persist, 150); });
    W.on('ready', persist);
    W.registerCommand('file.new', { title: 'New Document', keys: 'Mod+N', run: () => { handle = null; W.setText(''); persist(); } });
    W.registerCommand('file.open', { title: 'Open…', keys: 'Mod+O', run: open });
    W.registerCommand('file.save', { title: 'Save', keys: 'Mod+S', run: () => save(false) });
    W.registerCommand('file.saveAs', { title: 'Save As…', keys: 'Mod+Shift+S', run: () => save(true) });
    W.registerCommand('file.export', { title: 'Download .md', run: download });
    // drag & drop
    document.addEventListener('dragover', e => e.preventDefault());
    document.addEventListener('drop', async e => { e.preventDefault(); const f = e.dataTransfer.files[0]; if (f) { handle = null; W.setText(await f.text(), { source: 'open' }); } });
  });
})();
