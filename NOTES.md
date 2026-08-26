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
