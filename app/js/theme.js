/* Light / dark / auto. Owner: theme piece. */
(function () {
  const W = window.Writer;
  W.on('boot', () => {
    W.registerCommand('theme.toggle', { title: 'Toggle Dark Mode', keys: 'Mod+Shift+L', run: () => {
      const dark = matchMedia('(prefers-color-scheme: dark)').matches;
      const cur = W.settings.theme === 'auto' ? (dark ? 'dark' : 'light') : W.settings.theme;
      W.setSetting('theme', cur === 'dark' ? 'light' : 'dark');
    }});
    W.registerCommand('theme.auto', { title: 'Theme: Follow System', run: () => W.setSetting('theme', 'auto') });
    W.registerCommand('theme.light', { title: 'Theme: Light', run: () => W.setSetting('theme', 'light') });
    W.registerCommand('theme.dark', { title: 'Theme: Dark', run: () => W.setSetting('theme', 'dark') });
  });
})();
