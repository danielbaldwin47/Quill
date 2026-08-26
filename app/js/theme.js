/* Light / dark / follow-the-system. Owner: theme piece.
 *
 * The ground itself is chosen in CSS (theme.css): `data-theme` is stamped on
 * <html> by the inline bootstrap in index.html before the first paint, and
 * `auto` resolves through a prefers-color-scheme media query. So there is
 * nothing here on the critical path and nothing to flash. This file only
 * adds the switching: the shortcut, the commands, and a 160 ms cross-fade
 * so the change reads as a lamp being turned down rather than a strobe.
 */
(function () {
  const W = window.Writer;
  const root = document.documentElement;
  const mq = window.matchMedia('(prefers-color-scheme: dark)');

  // The ground actually on screen right now: 'light' | 'dark'.
  const resolve = () => (W.settings.theme === 'auto' ? (mq.matches ? 'dark' : 'light') : W.settings.theme);
  W.appearance = resolve;

  let shiftTimer = 0;
  function armShift() {
    root.dataset.themeShift = 'on';
    clearTimeout(shiftTimer);
    shiftTimer = setTimeout(() => { delete root.dataset.themeShift; shiftTimer = 0; }, 260);
  }

  let last = null;
  function publish(animate) {
    const now = resolve();
    if (now === last) return;
    if (animate && last !== null) armShift();
    last = now;
    root.dataset.appearance = now;      // for scripts; CSS never depends on it
    W.emit('appearance', now);
  }

  function set(theme) {
    if (W.settings.theme !== theme) armShift();
    W.setSetting('theme', theme);
    publish(false);
  }

  W.on('boot', () => {
    publish(false);
    W.on('settings', (k) => { if (k === 'theme') publish(false); });
    // Follow the system live while on 'auto' (macOS/Windows sundown, Night Shift, etc).
    const onSystem = () => publish(true);
    if (mq.addEventListener) mq.addEventListener('change', onSystem);
    else if (mq.addListener) mq.addListener(onSystem);

    // iA Writer's own appearance key is ⌃⌘N; Ctrl+Alt+N is its Windows/Linux twin.
    W.registerCommand('theme.toggle', {
      title: 'Dark or Light Appearance',
      keys: W.isMac ? ['Ctrl+Mod+N', 'Mod+Shift+L'] : ['Ctrl+Alt+N', 'Mod+Shift+L'],
      run: () => set(resolve() === 'dark' ? 'light' : 'dark'),
    });
    W.registerCommand('theme.auto', { title: 'Appearance: Follow System', run: () => set('auto') });
    W.registerCommand('theme.light', { title: 'Appearance: Light', run: () => set('light') });
    W.registerCommand('theme.dark', { title: 'Appearance: Dark', run: () => set('dark') });
  });
})();
