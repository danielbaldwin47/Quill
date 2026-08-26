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
