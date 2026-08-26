# NOTES — cross-piece changes and gotchas

Each builder appends under its piece heading when it touches a file it does not own.

## focus

Round 1. No files outside `app/js/focus.js` + `app/css/focus.css` were edited. Two contracts
this piece now depends on — please keep them:

* **theme.css `--fg-dim`** is the focus-mode dimmed ink (iA's measured #cccccc light /
  #707070 dark). focus.css reads it directly instead of re-deriving a grey, so the theme
  stays the single source of truth (including `prefers-contrast` and print). focus.css also
  publishes `--ink-dim` and `--ink-near` on `#mirror`; nothing else should redefine those.
* **page.css `--page-top` / `--page-bottom` under `[data-typewriter="on"]`** must stay ≥ ~50vh
  each, or the first and last lines cannot reach the typewriter line and the caret sticks to
  an edge. The current 52vh / 62vh is right.
* Dimming is applied as decorator classes `.dim` / `.near` on spans inside `#mirror .line`.
  markup.css rules of equal specificity are overridden because focus.css is linked last in
  index.html — keep that order.
* focus.js adds one-shot `fx-in` / `fx-out` classes to `#mirror .line` after re-filling it.
  They rely on core's `fillLine()` rewriting `el.className`, which clears them automatically.
  If fillLine ever stops resetting className, they must be removed explicitly.

New: sentence focus has a third tier (`near`) for the sentence either side of the active one,
and Focus Mode without Typewriter now keeps the active line inside a 15–85% band rather than
letting it sit on an edge. Typewriter uses a retargetable rAF ease (not `scroll-behavior`),
and a pointer-driven caret move only nudges to the nearest band edge instead of re-centring.

Screenshots/passages for this piece live in `shots/focus/`.

## theme

Round 1. Only `app/css/theme.css` + `app/js/theme.js` were edited. theme.css is the single
source of truth for colour; nothing else should hard-code a hex that varies by ground.

**Measured, not invented.** Re-measured here with ImageMagick from the lossless App Store
PNGs (`appstore-mac-01`, `-04`, `-06`, `-08`) and `ianet-mac-dark-focus-sentence-support`:

| role | light | dark |
|---|---|---|
| paper / ground | `#f9f9f9` | `#1a1a1a` |
| ink | `#1c1c1c` | `#cccccc` |
| focus-dim | `#cccccc` | `#707070` |
| caret / accent | `#00b5ff` | `#00b5ff` |
| selection fill | — | `#003e4c` (ours composites to `#143e4f`) |
| hairline | `#e1e1e1` (content-block border) | `#313131` (toolbar pill outline) |

The two palettes are deliberately *not* inversions: light ink is 15.9:1 over paper, dark ink
only 11.6:1, and the dim grey is relatively far brighter on dark (3.4:1) than on light (1.5:1)
because dark grounds crush low-contrast detail. Both facts are iA's, and both are kept.

**Tokens published** (please consume rather than redefine):
`--bg --fg --fg-dim --mark --accent --link --selection --selection-idle --chrome-bg
--chrome-fg --chrome-fg-strong --code-bg --rule --shadow`.
New this round: **`--link`** (light `#0b7cba`, dark `#4cc5ff`) — `--accent` is the *caret* blue
and is far too pale for link text on paper (2.3:1). markup.css already reads
`var(--link, var(--accent))`. **`--chrome-fg-strong`** for a document title; `--chrome-fg` for
everything else. **`--shadow`** for popover shadows.

**Cross-file colour rules that live in theme.css** (colour only, no layout):
* `::selection` globally, so the palette/bars select in the same blue as the editor.
* `#input:not(:focus)::selection` → `--selection-idle`; this out-specifies page.css's
  `#input::selection`, on purpose — the selection goes quiet when the window is not focused.
* `--chrome-bg: transparent` on both grounds. Measured: iA's toolbar is the *same* colour as
  the editor with no divider (appstore-mac-08, column scan finds no boundary), so the bars sit
  on the paper rather than on a panel. chrome.css can still paint its own scrim if it needs one.

**Attributes theme.js sets on `<html>`:**
* `data-theme-shift="on"` for ~260 ms around a switch. theme.css uses it to cross-fade
  background-color/color over 160 ms on `html, body, #mirror, #mirror span, .chrome, .caret`.
  It is never set on first paint or while typing. If a piece must not animate on a theme
  switch, opt out with `:root[data-theme-shift="on"] <your selector> { transition: none }`.
* `data-appearance="light|dark"` — the *resolved* ground, for scripts only. **CSS must not key
  off it**: the ground is chosen by `data-theme` + a `prefers-color-scheme` media query so the
  first frame is already correct (verified: stored dark on a light OS paints dark on frame 0).
  Also `Writer.appearance()` and a `Writer.on('appearance', fn)` event.

**Shortcut:** `theme.toggle` is now iA's own appearance key — `⌃⌘N` on Mac, `Ctrl+Alt+N`
elsewhere. `Mod+Shift+L` is kept as an alias so nothing that learned it breaks. Note that
`Writer.normKeys` cannot express `Ctrl+Mod+N` off Mac (it yields `Ctrl+Ctrl+N`), so the key
list is chosen at registration from `Writer.isMac`.

**For markup:** measured, iA renders emphasis `*` and heading `#` at *full ink*, not grey
(appstore-mac-01 asterisk = `#1c1c1c`, mac-09 hash = ink). Only metadata recedes — wikilink
brackets and URLs at `#707070` on dark. Ours greys all inline marks to `--mark`; that is a
deliberate departure and it reads well, but it is the one visible difference from the
reference in the round-1 pair, so it is worth a second opinion.

Verification: `shots/theme/themecheck.mjs` (boot on both grounds, live system-follow, toggle,
shortcut, first-paint flash check). Probe/state shots and the passage in `shots/theme/`.
Pair: `r1-ours.png` / `r1-theirs.png`, light panel over dark panel, 1970×1520.
Crops — light `appstore-mac-01-light-editor-hero.png` at `404 743 1970 760`;
dark `appstore-mac-08-dark-markdown-heading.png` at `186 769 1970 760`. Ours rendered at
`--w 985 --h 380 --dpr 2 --size 28.25 --font duo --chrome off --caret earth`, which reproduces
iA's wrap and its 34.1 px cell exactly.

## page

Round 1. Changed only `app/css/page.css`. No changes to any file I do not own.

Depends on tokens declared elsewhere (read, never redefined here):
* `--measure` (type.css) — the page centres it. Expects the 64ch default; if type
  ever changes it, the page geometry follows automatically.
* `--line-pitch` (type.css) — the page's top margin is `2 * var(--line-pitch)` and
  `#mirror .line`'s min-height is now `var(--line-pitch, calc(var(--line-height)*1em))`
  instead of the unitless-floor form. The old fallback is kept, so type is free to
  drop `--line-pitch` again without breaking the page.
* `--bg`, `--fg`, `--fg-dim`, `--rule`, `--selection` (theme.css). Measured iA paper
  is #f9f9f9 (App Store PNGs) / #f7f7f7 (ia.net webp) light and #1a1a1a dark —
  theme.css already matches; the page does not set a background of its own.

Behaviour other pieces should know about:
* `#page`'s top padding and `#input`'s `top` are both `var(--page-top)`; they must
  stay equal or the mirror and the textarea come apart vertically. Both are set in
  one place.
* Typewriter mode: `:root[data-typewriter="on"]` swaps `--page-top` to 52vh and
  `--page-bottom` to 62vh so focus.js can put any line — the first and the last
  included — on the typewriter line. Without typewriter the page opens with the
  text high in the window instead of the old 40vh/60vh slab.
* `#scroller` now sets `scroll-padding: 10vh 0 28vh`. Chromium honours it for the
  textarea's own caret scrolling, so typing at the end of a document comes to rest
  at ~69% of the window height (measured) rather than on the bottom edge.
* `scrollbar-gutter: stable both-edges` keeps the column exactly centred whether or
  not the document is long enough to scroll (measured: left 336.0 / right 336.0 in
  both cases at 1440x900).
* Empty document: `#mirror:has(> .line:only-child > br:only-child)::after` prints a
  faint "Start writing…" on the first line. It relies on core.js rendering an empty
  line as a lone `<br>` inside `.line`. If that ever changes, the placeholder just
  stops appearing — nothing else breaks. Measured no typing-latency cost
  (to_present p50 8.97/9.16/9.45 ms with it, 9.19/9.21/9.53 ms without).

Pair for the critic: shots/page/r1-ours.png vs shots/page/r1-theirs.png
(crop of ianet-mac-light-window-focus-syntax-italic.webp at x=114 y=152 w=2922
h=1764 — the editor area of the window, title bar excluded; ours rendered at
1461x882 @2x, light, Duo, 20px, Focus: Sentence, chrome off, caret after
"garden.", passage in shots/page/passage.md).

## chrome

Round 1. Files owned: `app/js/chrome.js`, `app/css/chrome.css`.

Changes outside my files (minimal, backward compatible):

* **`tools/shoot.mjs` — added `--scroll <px|"needle">`.** Reference captures are of
  scrolled documents (the iA stats-bar shot is cut mid-line at the top); without a
  scroll control no piece can reproduce that state. A number sets `scrollTop`; a
  string scrolls the line containing it to the top of the viewport. Absent, nothing
  changes. Used as `--scroll 120` for the chrome pair.

Contracts I depend on / preserved for other pieces:

* `#doc-title` is still a plain text element — `files.js` writes the document name
  into its `textContent`. It now lives inside a `<button class="doc-title">` that
  opens the Document menu, so writing text into `#doc-title` keeps working.
* `files.js` inserts `#lib-toggle` as the first child of `#chrome-top .bar`.
  chrome.css pins that first child at `left: 8px` (absolute) so the title stays
  centred in the window whether the library button is there or not.
* The stats scan stays off the keystroke path (the `requestIdleCallback` the
  latency piece added is kept, plus a 500 ms "you stopped typing" catch-up).
  Selection stats repaint at most once per frame.
* New settings keys (free-form, persisted by core): `stats` (comma list of visible
  stat ids) and `statsBar` (false hides the bottom bar). Neither is in core's
  DEFAULTS; chrome.js reads them defensively.
* `?open=view|document|stats|palette` opens that panel on load. It is how the
  menu/palette states get screenshot without a `--click` flag in shoot.mjs.
* Commands registered with `hidden: true` are kept out of the palette list
  (`palette.open`, `chrome.view`, `chrome.doc`). Any piece may use the flag.

## files

Round 1 built the Library (`app/js/files.js` + `app/css/files.css`). Things it
touches outside its own two files — all additive, all guarded:

* **`app/index.html` — untouched.** The Library injects itself: `files.js`
  appends `<aside id="library">` to `#app` on boot, and `files.css` gives
  `#app` a `padding-left: var(--lib-w)` only under `:root[data-library="open"]`.
  The page keeps its own centring; it is simply handed a narrower window.
  **The Library is closed by default**, so no other piece's screenshots change
  unless they ask for it (`Writer.run('library.toggle')`, ⇧⌘L, or a seeded state).
* **`#chrome-top` (chrome piece):** files.js adds one button, `#lib-toggle`, to
  the top bar — into `.side.left` if that exists, otherwise as the bar's first
  child positioned absolutely at the left so the centred title stays centred.
  It stands down entirely if the bar already contains
  `[data-cmd="library.toggle"]`, so chrome can take the button over at any time
  and nothing needs to change here.
* **`#doc-title` (chrome piece):** files.js writes the open document's name into
  it (`querySelector('#doc-title .name, #doc-title, .doc-title .name')`), as
  chrome.js's own comment expects.
* **`#mirror .line` (core/markup):** a decorator marks any line whose whole text
  is the name of another document in the Library with class `docref`, and
  files.css gives it a background + `box-shadow` ring — **no padding, no font
  change**, so glyph advances are untouched and mirror/textarea stay aligned.
  ⌘/Ctrl-click (or ⌘⏎) on such a line opens that document. The full re-render
  this needs runs only when the set of document names changes, never on the
  keystroke path.
* **`tools/shoot.mjs`:** added `--state seed.json`, which merges
  `{localStorageKey: value}` into localStorage in the same init script that
  already sets `quill.settings`. Needed to screenshot a populated Library;
  useful to anyone who wants to shoot a stored state. Default behaviour is
  unchanged when the flag is absent.
* **Storage keys:** `quill.lib` (library state + the documents kept in this
  browser) and `quill.doc` / `.sel` / `.id`, still written as a crash-safety
  mirror of the open document so core's boot comment stays true. A directory
  handle picked with the File System Access API lives in IndexedDB `quill/kv`.

## markup

**core.js — fence/front-matter state never reached the tokenizer (1-line fix).**
`recomputeCtx(from)` bailed out as soon as two consecutive lines had an "unchanged"
context, and on a *full* render `lineCtx` is empty, so every line looked unchanged and
the walk stopped at line 1. Result: every line after the first two was tokenised with
`ctx = null`, so a fenced code block's body was never seen as code. Added an optional
`force` argument, passed only from the full-render branch:
`function recomputeCtx(from, force)` · `if (!force && !changed && …)` ·
`lineCtx = []; recomputeCtx(0, true);`. Incremental renders are untouched.

**Why heading/quote markers do not hang in the margin (iA Writer hangs "# " 2 cells,
"## " 3, "> " 2).** Hanging needs a per-line horizontal shift of the mirror line, and
the textarea underneath cannot be shifted per line. Shifting only the mirror would
leave the native selection paint and click-to-position 2–3 cells off on exactly the
lines people click most (headings), which the BRIEF forbids ("never break glyph
alignment between #input and #mirror"). markup buys the same calm text image with
contrast instead of position: marks in a quiet grey, heading text bold at full ink,
and a margin rule for block quotes / a full-width hairline for `---`, both drawn with
absolutely positioned pseudo-elements that take no part in layout.

**Mono is NOT metric-compatible with Duo/Quattro** (type.css's comment above the
italic/bold note says it is). Duo and Quattro widen m/M/w/W to 1.5 cells; Mono keeps
every glyph at 0.6 em, so a `font-family` swap for code spans would move every glyph
after the first `m` on the line. markup.css therefore never changes family — code is
distinguished by ground, not by alphabet. (Weight and italic *are* safe: measured
ascent/descent/advances are identical across all four styles of each family.)

**tools/mirror-metrics.mjs** (new): proves the rule. For every line of a document it
lays the same text out twice — once with markup spans (the real mirror line), once as
plain text — in all three faces, and reports any line whose height or last-glyph
position differs. Run it after touching anything that styles the mirror:
`node tools/mirror-metrics.mjs [file.md …]`. Currently 0 drift in duo/quattro/mono.
(The mirror-vs-textarea total height differs by ~0.3 px per line in *all* documents,
markup or not — sub-pixel rounding of the fractional line pitch, not a markup issue.)

**core.js `DEFAULTS.fontSize: 18` overrides type.css's `--font-size: 20px`** for every
fresh profile (`applySettings` always writes the inline custom property). Until the
type/latency owners reconcile the two, comparison shots need an explicit `--size 20`
or they render 10 % small. Nothing changed for this; flagging it.

## latency

Round 1. Files owned: `tools/latency.mjs`, `app/js/core.js` (perf only), `bin/quill`.
New files added: `tools/mkdoc.mjs` (builds the benchmark corpus), `shots/latency/doc*.md`,
`progress/latency.json`, `progress/latency-report.md`.

**Numbers first** (headless Chromium 151, 13600K, 10,062-word Markdown document, 300 keystrokes
per regime, every keystroke accounted for): keystroke → frame presented **p50 9.8 ms, p95 18.4,
p99 19.1**; keystroke → frame committed p50 8.3; 89 % of keystrokes are on screen within one
60 Hz frame, 100 % within two. Startup: first frame showing the whole document **104 ms**
(editor ready at 77 ms). Full method, the real-display run and the caveats are in
`progress/latency-report.md`; raw runs in `shots/latency/r1-*.json`.

### Changes in `app/js/core.js` (perf only, backward compatible)

1. **The textarea is sized by a `ResizeObserver`, not by a measurement inside the keystroke.**
   `render()` used to run `input.style.height = mirror.offsetHeight + 'px'` on every input event.
   That read forces a synchronous layout of the whole document in the middle of the keystroke,
   and the write that follows dirties it again. The observer does the same job out of the frame's
   own layout pass and only fires when the height really changed. Full renders (boot, `setText`,
   font change) still sync synchronously, so anything that measures straight after still sees the
   right height. `Writer.syncHeight()` is exposed if a piece ever needs to force it.
   *Verified*: `#input.offsetHeight === #mirror.offsetHeight` after boot, after typing new lines
   and after a font-size change, in all three faces and both grounds.
2. **`selection` is emitted once per real change, not three times per keystroke.** Chrome raises
   `input`, `selectionchange` and `keyup` for a single keypress and core forwarded all three;
   every listener on that event measures layout (caret rect, typewriter). Measured on the live
   app: **3.02 → 1.02 `selection` events per keystroke, and 12.07 → 6.08 layout-reading DOM calls
   per keystroke** (`Element.getBoundingClientRect` + `Range.getClientRects`, counted by patching
   both). Text changes still force an emit, because the offsets can be unchanged while the line
   is not. New: `Writer.emitSelection(force)` — use it instead of `Writer.emit('selection')` if
   you change the selection yourself; `Writer.setSelection` already does.
3. **Writes before reads.** During an `input` event the `render` event is now emitted *after*
   `change` and `selection`, so the pieces that write DOM (focus dimming) all run before the
   piece that measures it (the caret). One layout flush per keystroke instead of two. Every other
   caller of `Writer.render()` still gets `render` synchronously, in the same place as before.
4. **Line renumbering only when the line count changed**, and only for indices whose value
   actually differs — `el.dataset.i = i` on every line below the caret is a same-value
   `setAttribute` storm, and the shortcut table is no longer rebuilt and re-sorted per keydown
   (a plain character with no modifier now looks at nothing).

A/B of *only* those four changes, same everything else, two snapshot servers, 200 keys each:
main-thread work per keystroke 6.05 → 5.35 ms, style+layout+paint 2439 → 2171 µs, present p50
10.12 → 9.46 ms, p90 17.25 → 16.52, keystrokes on screen within one frame 88.5 % → 91.5 %.

### Findings for other pieces (nothing was changed in your files except the one noted)

* **`app/js/chrome.js`** — mid-round I moved the word count off the keystroke path
  (`requestIdleCallback`, 400 ms timeout): scanning 53 KB with `/[\p{L}\p{N}'’]+/gu` cost
  **~0.53 ms per keystroke**, and the bars are hidden while you type, so it was invisible work.
  The chrome rewrite that landed later does the same thing with its own `recount(now)`/`idle()`
  and a 500 ms catch-up — that is kept, and it is the right shape. Please keep any future
  document-wide scan (readability, style check, outline) on that side of the line.
* **Focus mode costs about 1.2 ms of main thread per keystroke** (event processing 2.27 → 3.41 ms
  p50, style+layout+paint 2418 → 3022 µs). It does not change the presented latency at 133 wpm
  (present p50 9.82 → 9.80 ms) because there is frame budget to spare, but it is the single most
  expensive listener in the app. Most of it is `update()` running on every selection: the
  `JSON.stringify` signature and the re-scan of the line's sentences.
* **The caret is placed twice per keystroke** — once on `selection`, once on `render` — costing
  roughly 100 µs and one extra `getClientRects`. Coalescing the two into one `queueMicrotask`
  inside `caret.js` would remove it; it is your file, so it is left alone.
* **`page.css`'s `#mirror:has(> .line:only-child > br:only-child)` placeholder** was measured for
  a `:has()` invalidation cost on the keystroke path and there is none worth reporting (present
  p50 within run-to-run noise, as the page piece also measured).
* **Nothing in the app may scan the whole document synchronously on `change`.** At 10k words a
  full-text pass is ~0.4–0.6 ms; three of them and the keystroke misses its frame.

### `bin/quill`

`chromium --app` launcher: starts `tools/serve.mjs` if the port is dead, uses its own profile
(`~/.cache/quill/browser`), and `--measure` times *shell exec → first frame with the document*
over CDP. Two things to know:

* **It refuses to open a measured window** unless the compositor can place it off the active
  workspace (BRIEF.md "Headed windows"). This Hyprland build rejects both
  `hyprctl dispatch exec "[workspace 2 silent] …"` and `hyprctl keyword windowrule …`
  ("keyword can't work with non-legacy parsers"), and chromium's Wayland `app_id` is derived
  from the URL (`chrome-localhost__-Default`), not from `--class`, so a class rule would not
  match it either. Override with `QUILL_HEADED_OK=1` only if the user's workspace is free.
  The one real-display dataset in the report was taken before that rule was published; the
  window has been closed and none has been opened since.
* Cold start on the real display, warm profile, 10,062-word document open:
  **395 ms and 511 ms** from `exec` to pixels, of which ~155 ms is the chromium process spawn.

### Re-running

`node tools/latency.mjs` (defaults: `shots/latency/doc10k.md`, 12 startup runs, 300 keys × 5
regimes, 90 ms pacing) writes the full JSON. `--doc`, `--keys`, `--pace`, `--runs`, `--quick`,
`--url` (point it at a snapshot server for an A/B), `--json out.json`. `tools/mkdoc.mjs` rebuilds
the corpus from a Project Gutenberg text file.

## type

Round 1. Owner files: `app/css/type.css`, `app/fonts/**`, plus `tools/fontgrid.py` (new, see below).

**What the numbers came from.** Everything below is measured off the reference shots with
advance-fitting (fit the Duo cell width across a whole line, then read the pitch off the ink
bands), not read off REFERENCE.md, because REFERENCE.md's em estimates come from x-height and
are ~2% low. Fitted results (2x captures, halved to CSS px):

| capture | em | pitch | ratio |
|---|---|---|---|
| ianet-mac-dark-typewriter-support | 20.00 | 36.5 | 1.83 |
| appstore-mac-09 | 24.17 | 40.0 | 1.66 |
| appstore-mac-01 / -08 (the pair shots) | 27.65 | 46.25 | 1.67 |
| ianet-mac-light-focus-paragraph-alice | 28.25 | 47.1 | 1.67 |

So iA's leading is liquid, as they say it is: `pitch = 1.30 x size + 10.4px` fits three of the
four inside a third of a pixel. That is what `--line-pitch` computes now (rounded to whole px,
clamped 1.52-2.0 em at the extremes of Mod +/-). Default size moved 18 -> 20px, the size their
own window capture is set at.

**Changes outside my files (minimal, backward compatible):**
- `app/js/core.js`: `DEFAULTS.fontSize` 18 -> 20. One line, no behaviour change.

**For other pieces:**
- `--line-pitch` is now the *exact used line-height* in px (already rounded), so anything that
  needs the pitch should read it — page.css already does. `--line-height` is kept only as a
  unitless floor for `min-height: calc(var(--line-height) * 1em)`; it is deliberately lower
  (1.5) than the real ratio at every size in range so a blank line can never come out taller
  than a written one. Do not use it as the pitch.
  This mattered: while I was working, `#mirror .line { min-height: var(--line-pitch) }` was
  reading the *unrounded* pitch while `line-height` used the rounded one, which made every
  line 0.4px too tall and walked #mirror off #input. Rounding now happens once, in
  `--line-pitch` itself.
- Ink weight is `--ink-weight` (415 on paper, 400 on dark). Measured ink per line against iA at
  27.65px: their glyphs carry ~4% more ink than a flat 400 on light (macOS darkens stems on a
  light ground) and land within 1% of 400 on dark. Advances are bit-identical at 400 and 415 in
  all three faces, so this is metric-safe. After the change ours is within 1% of iA on both
  themes.
- `--measure` is declared in both type.css and page.css at `64ch`; page.css loads later and
  wins. Fine as long as both stay 64ch — that is iA's published default line-length limit.

**Fonts: switched to the variable cuts, and one of them is patched.**
- `app/fonts/*/iAWriter*V.woff2` + `-V-Italic.woff2` replace the four static faces per family.
  Two files instead of four, ~100KB instead of ~173KB, and the upright file covers 400 *and*
  700 — so bold no longer costs a second font fetch mid-keystroke.
- `iAWriterQuattroV-Italic-grid.woff2` is a MODIFIED copy. iA's Quattro Italic gives the word
  space 600 units where the Roman gives it 450; every italic word therefore slid the rest of
  the line 0.15 em out from under the textarea (~1 character after six words). `tools/fontgrid.py`
  resets that one advance to 450 and empties the space glyph's (contourless) gvar entry so it
  stays 450 at every weight. Nothing else is touched, and the true italic is preserved.
  **OFL note:** a public build must ship this under a family name that does not contain the
  reserved name "iA Writer".
- Remaining known advance divergences, all bold/bold-italic only and all left alone because
  fixing them would either collide ink or freeze the glyph's weight axis: Quattro `f`/`t`
  +150 units in Bold Italic; `!` +8 in Bold Italic (Duo and Quattro); Mono `j` +19 in Bold;
  Mono `%` is 667 in Regular and 600 in every other style (it is off the monospace grid in iA's
  own font). `node tools/mirror-metrics.mjs` reports 0 drift on ordinary prose in all three faces.
- The italic face is warmed at boot by one clipped invisible glyph (`html::after`). Without it
  `document.fonts.check('italic ...')` is false after boot, so the first `*word*` you type
  fetches a font mid-keystroke: the word blinks (faces are `font-display: block`) and, while the
  fallback stands in, that line lays out a pixel taller than its neighbours.

**Deliberately not done:**
- *Hanging markers / hanging punctuation.* iA hangs `# ` and `> ` in the left margin. That needs
  a per-line horizontal shift, and a textarea cannot indent one line differently from the next,
  so the mirror's heading text would sit a cell and a half away from the caret. Not metric-safe
  in this architecture; markup.css reaches the same conclusion and buys the calm text image with
  contrast instead.
- *Liquid weight* (iA varies weight with size as well as leading). The variable fonts support it
  and it is the obvious next step, but `--font-size` reaches CSS as a length, and `font-weight`
  needs a number — `calc(10px / 1px)` is not a thing. It wants a numeric `--font-size-n` from
  core.js; not worth a core change this round.

**For the markup piece (not a request, just an observation from the pair):** iA renders the
`*` around an emphasised word in full body ink, not grey. In the blind pair that is the single
most visible difference between our light shot and theirs. Ours reads calmer; theirs reads more
"the characters are really there". Worth a deliberate decision rather than a default.

**Pair:** `shots/type/r1-ours.png` (965x350 @2x, light, Duo, 27.65px, caret after "earth") vs
`shots/type/r1-theirs.png` = `crop.mjs appstore-mac-01-light-editor-hero.png 406 749 1930 700`.
Crop origin chosen so the text block registers exactly on ours, i.e. the two images differ only
in typography, not in page margins (those belong to the page piece; the reference window is
wider than any viewport we could match). Dark equivalent saved beside it:
`r1-ours-dark.png` vs `crop.mjs appstore-mac-08-dark-markdown-heading.png 188 775 1930 700`.
Reproduce the passage with `shots/type/passage.md`; `shots/type/specimen.md` is the
bold/italic/blank-line specimen used for the metric checks.

## caret

Round 1. Files touched: `app/js/caret.js`, `app/css/caret.css` (both owned). No
edits to `core.js`, `index.html` or any other piece's file. Three things reach
outside the caret's own elements, all from `app/css/caret.css`:

1. **`#input::selection` is forced transparent.** The native textarea selection
   paints *above* `#mirror`, so it veils the glyphs — measured on the old build:
   selected ink went from `#1c1c1c` to `#153d4d`. We draw the highlight
   ourselves instead, underneath the ink, where iA puts it. The rule is written
   as `#input::selection, #input:not(:focus)::selection` so it matches the
   specificity of theme.css's idle-selection rule and wins on load order
   (caret.css is linked after theme.css — please keep it that way).
   `--selection` / `--selection-idle` are still the source of truth for the
   colour; theme.css just no longer reaches the editor through `::selection`.

2. **`#sel-layer` is inserted by caret.js as the first child of `#page`.** It is
   `position:absolute; inset:0; z-index:0`, so it paints below `#mirror` (first
   in tree order among the positioned children) and above nothing else. Nothing
   in `#page` needed changing; if the page piece ever gives `#mirror` a negative
   z-index or makes `#page` a stacking context with a background, ping caret.

3. **`@media print { #sel-layer { display: none } }`** — theme.css already drops
   `#caret-layer` for print and the highlight has to go with it.

For the theme piece, not urgent: dark `--selection` is
`rgba(0,181,255,.23)`, which lands on `#143e4f` over `#1a1a1a`. iA's measured
value is `#003e4c` — same green and blue, but with the red channel pulled to 0,
which an alpha tint over a grey ground cannot do. Now that the highlight paints
*under* the ink, an opaque dark-theme value is safe if you want the exact match.

For the page piece, tiny: the empty-document placeholder starts in the first
cell, so the caret's stem sits on the `S` of "Start writing…". iA has no
placeholder there. Starting it one cell in, or dropping it to 12 % opacity,
would keep the two out of each other's way.

