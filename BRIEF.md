# Quill — builder & critic brief

Quill is a long-form writing environment meant to beat iA Writer. Web app, no framework, no build step:
`app/index.html` + `app/css/*.css` + `app/js/*.js`. Serve with `node tools/serve.mjs 4173` (usually already running at http://localhost:4173/).

## Architecture (read app/js/core.js first)
- `<textarea id="input">` holds the text and native selection (transparent glyphs, on top).
- `<div id="mirror">` renders the same text line-by-line with markup spans (visible glyphs, beneath). Same font metrics ⇒ glyphs align exactly.
  HARD RULE: no style may change glyph advance widths (no font-size change per token, no letter-spacing, no padding inside lines, no different font family unless it is metric-compatible). Weight/italic are fine with iA Writer fonts (all weights share widths). Color, opacity, decoration, background are fine. A vertical shift between lines is fine only if applied to both `#mirror .line` and… no — the textarea can't get per-line spacing, so per-line spacing is NOT allowed either. Paragraph spacing therefore = blank lines (as in iA Writer).
- Plugins: `Writer.tokenizeLine` (markup.js), `Writer.addDecorator` (focus.js), `Writer.on(...)`, `Writer.registerCommand`, `Writer.caretRect()`, `Writer.setSetting(k,v)`.
- Settings persist in localStorage `quill.settings`; document autosaves to `quill.doc`.

## File ownership (one builder per piece; edit only your files, note anything else in NOTES.md)
| piece | files |
|---|---|
| page | app/css/page.css |
| type | app/css/type.css, app/fonts/** |
| caret | app/js/caret.js, app/css/caret.css |
| focus | app/js/focus.js, app/css/focus.css |
| theme | app/css/theme.css, app/js/theme.js |
| markup | app/js/markup.js, app/css/markup.css |
| chrome | app/js/chrome.js, app/css/chrome.css |
| files | app/js/files.js, app/css/files.css |
| latency | tools/latency.mjs, app/js/core.js (perf-only changes), bin/quill launcher |
If you truly need a change in core.js or index.html, make it minimal, backward compatible, and record it in NOTES.md under your piece.

## Tools
- Screenshot: `node tools/shoot.mjs --out shots/<piece>/x.png --w 1440 --h 900 --dpr 2 --theme light|dark --font duo|quattro|mono --size 18 --focus off|sentence|paragraph [--typewriter] --chrome on|off --text <file.md> --caret "<needle>"|N|end [--mouse] [--typing] [--select a,b] [--nocaret]`
- Latency: `node tools/latency.mjs --runs 10 --keys 300 --text <file.md> --json out.json`
- Blind pair: `node tools/blind.mjs pair <piece> <ours.png> <theirs.png>` → shots/blind/<piece>/A.png,B.png. `node tools/blind.mjs reveal <piece>` prints which is ours. Critics NEVER run reveal and NEVER read shots outside shots/blind/<piece>/.
- Reference: `ref/ia/REFERENCE.md` (spec sheet, quotes, screenshot table with transcribed text), `ref/ia/shots/`, `ref/ia/fonts/`, `ref/ia/templates/`.

## Standards
- Match the iA reference's viewport for comparisons (same pixel size as the reference image; use --w/--h/--dpr to hit it). Render the same passage the reference shows (transcribed in REFERENCE.md) so the critic compares design, not content.
- Everything must work in both themes and all three fonts. Never break typing latency: no per-keystroke full re-render, no layout thrash, no heavy DOM.
- Keep the app instant: no network fonts (fonts are local woff2/ttf in app/fonts), no frameworks, no build step.

## Fair comparison protocol
Many ia.net / App Store images are marketing composites (headline + partial window). Before pairing:
1. Crop the reference to just the app/editor region: `node tools/crop.mjs <ref> <out.png> x y w h [scale]`.
2. Render ours at exactly the crop's pixel size (`--w` = crop width / dpr, `--h` = crop height / dpr, `--dpr 2` for Mac shots, 1 for Windows) showing the same passage, same theme, same mode, caret in the same place.
3. Pair with `node tools/blind.mjs pair <piece> <ours.png> <crop.png>`.
Prefer the clean full-window screenshots (ianet-mac-*-support.webp, msstore-win-*.png) over composites when they show your piece.

## Headed windows (real browser, not headless)
Never open test windows on Hyprland workspace 1 — the user works there. Launch through Hyprland's exec rules so the window lands elsewhere silently, e.g.
`hyprctl dispatch exec "[workspace 2 silent] chromium --app=http://localhost:4173/ --class=quill-test"`
(if `hyprctl` is unavailable, do not open headed windows at all — use headless).
