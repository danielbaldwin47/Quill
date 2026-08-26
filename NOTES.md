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
New files: `tools/mkdoc.mjs` (builds the benchmark corpus), `shots/latency/doc*.md`,
`progress/latency.json`, `progress/latency-report.md`.

**Numbers** (Chromium 151, 13600K, real 10,062-word Markdown document, 300 keystrokes per regime,
every keystroke accounted for, headless with a 60 Hz frame clock — see below): keystroke → frame
presented **p50 10.8 ms, p95 18.5, p99 19.1**, worst 20.2; on a real 60 Hz panel (earlier build)
13.8 / 29.1. Application cost with the display clock removed: **4.2 ms p50**, 2.9 ms to committed
frame, **4.9 ms of main thread per keystroke** — inside the ≤ 5 ms bar, and only 0.85 ms of it is
Quill's own code (the biggest single cost is Chrome inserting the character into a 53 KB textarea).
Startup: first frame showing the whole document **100 ms**, editor ready 71 ms. Estimated
keyboard-to-photon ≈ 32 ms, i.e. the Sublime/TextEdit bracket. Method, all regimes, document-size
scaling (2 k → 55 k words) and the caveats: `progress/latency-report.md`; raw runs in
`shots/latency/r1-*.json` and `shots/latency/size-doc*.json`.

### The one thing everyone touching this app should know

**Anything that animates while the writer types puts every keystroke on the display's clock.**
With no animation running, the compositor produces a frame on demand; with one running, a
keystroke waits for the next tick. Measured on this build, same page, same keys, the only
difference being a CSS rule that keeps the caret blinking during typing:

| caret while typing | present p50 | p90 | p99 | commit p50 |
|---|---|---|---|---|
| blink suppressed (what the app does) | 3.28 | 4.15 | 11.68 | 2.57 |
| blink left running | 9.40 | 16.48 | 18.22 | 8.90 |

On a real 60 Hz panel that tick is unavoidable, so this is not free latency — but it is exactly why
a headless benchmark with nothing animating reports ~6 ms less than any panel can deliver.
`tools/latency.mjs` therefore keeps a 1 px composited animation running by default (`--clock on`)
so headless models a panel, and `--clock off` measures the application's own cost. Both are
reported and never quoted as each other. caret.js's existing behaviour (blink off on each
keystroke, back after 480 ms of quiet) is right on both counts, visual and physical — please keep it.

### Changes in `app/js/core.js` (perf only, backward compatible)

1. **The textarea is sized by a `ResizeObserver`, not by a measurement inside the keystroke.**
   `render()` used to run `input.style.height = mirror.offsetHeight + 'px'` on every input event:
   that read forces a synchronous layout of the whole document mid-keystroke and the write after it
   dirties layout again. The observer does the same job out of the frame's own layout pass and only
   fires when the height really changed. Full renders (boot, `setText`, font change) still sync
   synchronously, so anything that measures immediately afterwards still sees the right height;
   `Writer.syncHeight()` forces it if a piece ever needs to. *Verified*: `#input.offsetHeight ===
   #mirror.offsetHeight` after boot, after typing new lines and after a font-size change, in all
   three faces and both grounds.
2. **`selection` is emitted once per real change, not three times per keystroke.** Chrome raises
   `input`, `selectionchange` and `keyup` for a single keypress and core forwarded all three; every
   listener on that event measures layout (caret rect, typewriter). Measured on the live app:
   **3.02 → 1.02 `selection` events per keystroke and 12.07 → 6.08 layout-reading DOM calls**
   (`Element.getBoundingClientRect` + `Range.getClientRects`, counted by patching both). Text
   changes still force an emit, because the offsets can be unchanged while the line is not.
   New: **`Writer.emitSelection(force)`** — use it instead of `Writer.emit('selection')` when you
   move the selection yourself; `Writer.setSelection` already does.
3. **Writes before reads.** During an `input` event the `render` event is now emitted after
   `change` and `selection`, so the pieces that write DOM (focus dimming) run before the piece that
   measures it (the caret): one layout flush per keystroke instead of two. Every other caller of
   `Writer.render()` still gets `render` synchronously, exactly as before.
4. **No attribute storm.** `el.dataset.i` is rewritten only when the line count actually changed
   and only where the value differs (a same-value `setAttribute` still costs a style
   invalidation), and the shortcut table is no longer rebuilt and re-sorted on every keydown — a
   plain character with no modifier now looks at nothing.

A/B of *only* those four changes, identical in every other file, two snapshot servers, 250 keys
each, display clock off so the frame cadence does not hide the difference: main-thread work per
keystroke **4.84 → 4.17 ms**, style+layout+paint 2000 → 1859 µs, present p99 **9.01 → 5.93 ms**,
p90 4.27 → 3.97, on screen within one frame 99.6 → 100 %.

### Findings for other pieces (no files of yours were changed)

* **chrome.js** — mid-round I moved the word count off the keystroke path with
  `requestIdleCallback`; scanning 53 KB with `/[\p{L}\p{N}'’]+/gu` cost **~0.53 ms per keystroke**
  and the bars are hidden while you type, so it was invisible work. The chrome rewrite that landed
  later does the same thing with its own `recount(now)`/`idle()` and a 500 ms catch-up — kept, and
  it is the right shape. Please keep any future document-wide scan (style check, outline,
  readability) on that side of the line.
* **Focus mode costs ~0.5 ms of main thread per keystroke** (4.89 → 5.43 ms busy; event processing
  2.3 → 3.4 ms p50). It does not change presented latency at 133 wpm — the frame budget absorbs it —
  but it is the most expensive listener in the app, and most of it is `update()` running on every
  selection (the `JSON.stringify` signature and the per-line sentence re-scan).
* **The caret is placed twice per keystroke** — once on `selection`, once on `render` — about
  100 µs and one extra `getClientRects`. Coalescing them into one `queueMicrotask` in caret.js
  would remove it; it is your file, so it is left alone.
* **page.css's `:has()` placeholder** was checked for an invalidation cost on the keystroke path;
  there is none outside run-to-run noise.
* **Nothing may scan the whole document synchronously on `change`.** At 10k words a full-text pass
  is ~0.4–0.6 ms; three of them and the keystroke misses its frame. At 55k words the whole
  keystroke already costs 12.9 ms of a 16.7 ms frame.

### `bin/quill`

`chromium --app` launcher: starts `tools/serve.mjs` if the port is dead, own profile under
`~/.cache/quill/browser`, `--fresh` for a true first run, `--measure` times *shell exec → first
frame with the document* over CDP (155 ms of that is the chromium process spawn; 395 and 511 ms to
pixels in two runs with the 10k-word document).

**It refuses to open a measured window** unless the compositor can place it off the active
workspace (BRIEF.md "Headed windows"). This Hyprland build rejects both
`hyprctl dispatch exec "[workspace 2 silent] …"` and `hyprctl keyword windowrule …`
("keyword can't work with non-legacy parsers"), and chromium's Wayland `app_id` comes from the URL
(`chrome-localhost__-Default`) rather than `--class`, so a class rule cannot match it either.
Override with `QUILL_HEADED_OK=1` only when the user's workspace is free. The single real-display
dataset in the report was taken before that rule was published; that window was closed and none has
been opened since.

### Re-running

`node tools/latency.mjs` (defaults: `shots/latency/doc10k.md`, 12 startup runs, 5 regimes × 300
keys, 90 ms pacing, 60 Hz clock). Flags: `--clock off`, `--doc`, `--keys`, `--pace`, `--runs`,
`--quick`, `--url` (point at a snapshot server for an A/B), `--snapshot <dir>` (fingerprint that
tree instead of `app/`), `--json out.json`. `tools/mkdoc.mjs` rebuilds the corpus from a Project
Gutenberg text file. Note that the app is fingerprinted into every result file: other builders
edit the same tree while a run is in flight, so a number without a fingerprint is a number about
nothing in particular.
