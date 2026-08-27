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

Round 2. Files owned: `tools/latency.mjs`, `app/js/core.js` (perf only), `bin/quill`.
Probes and raw runs in `shots/latency/` (`probes/*.mjs` are small single-purpose measurements;
`r2-*.json` are the runs quoted in `progress/latency-report.md`).

**Numbers** (Chromium 151, 13600K, real 10,062-word document, app fingerprint `19b86f97e10b203d`,
every keystroke accounted for in all 36 headless and all 21 compositor runs). The application's own
work, nothing animating: **5.7 ms p50 / 15.0 p99** to a produced frame, 4.8 ms to a committed one,
5.5 ms of main thread per keystroke of which **814 µs is Quill's own code** — inside the ≤5 ms /
≤16 ms bar, 99.7 % of keystrokes within one 60 Hz frame. On the user's own Hyprland compositor at
60 Hz: **23.6 / 37.7 ms**, of which 9.3 ms is Chromium and the rest is the wait for the display.
Startup from a shell with the document in the profile: **375 ms** to the first frame, 322 ms to a
usable editor. A 4× slower CPU: p50 12.6–20.3 ms. A 55k-word manuscript costs 16.9 ms of main
thread per keystroke — over one frame, and the report says so.

### What round 2 changed in `app/js/core.js` (perf only, backward compatible)

1. **Line elements no longer carry `data-i`.** Keeping the index attribute truthful meant
   rewriting every element below an inserted line on every Enter — 1.14 ms of attribute writes
   and style invalidations in a 3,285-line document, on the one keystroke that is already the
   most expensive. Nothing read the attribute (checked across `app/` and `tools/`);
   **`Writer.lineIndexOf(el)`** is there if a piece ever needs element → index.
2. **A changed line is tokenised once per keystroke, not twice.** The incremental branch used to
   `buildLine()` (which fills) and then `fillLine()` the same elements again after the contexts
   were recomputed. New elements now go in empty and are filled once, after `recomputeCtx`.
3. **The re-tokenising below an edit is bounded by what is on screen.** When the context entering
   the lines below an edit changes — typing ``` opens a fenced block and changes every line to the
   end of the document — the old code re-filled all of them inside the keystroke: **26 ms** in a
   55k-word manuscript, three 60 Hz frames, from one keypress. Now `AHEAD = 64` lines (more than a
   screenful at any font size) are filled inside the keystroke and the rest is caught up in
   animation frames, 400 lines at a time. **`Writer.flushPending()`** forces it. Measured with
   `shots/latency/probes/micro2.mjs`, `Writer.render(false)` alone, 55k-word document:
   opening a fence **26.16 → 3.33 ms**; Enter **2.70 → 1.93 ms**; a plain character 1.95 → 1.89 ms.
   Nothing visible is ever stale: the catch-up only ever covers lines below the fold, and the
   mirror's text still equals the textarea's, line for line, at every point
   (`shots/latency/probes/correctness.mjs`).
4. `dataset.lc`, which nothing wrote or read, is no longer touched on every fill.

### Findings for other pieces (no files of yours were changed)

* **caret.js — the caret glides 62 ms behind the letter at normal writing speed, and the glide is
  an animation that runs while you type.** `GLIDE_X = 62`, `SNAP_MS = 60`: two keystrokes more
  than 60 ms apart glide, and 133 wpm is 90 ms apart, so at any ordinary writing speed *every*
  keystroke glides. Measured (`shots/latency/probes/caret-glide.mjs`, keydown → the caret's box
  has stopped moving): **57.7 ms p50 at 90 ms pacing**, 7.7 ms at 45 ms pacing (there it snaps, as
  designed). The glyph itself is on screen in ~5–10 ms. Two consequences:
  1. What a writer watches — the caret — is the slowest thing on the screen while they type.
     A 30–40 ms glide would still read as "the same caret moving" and would halve that.
  2. **An animation is a frame clock.** `document.getAnimations()` while typing shows
     `transform on caret` running in **343 of 446 samples** (plus the chrome bars' opacity
     fades at the edges of a burst). With something animating, a keystroke's update is scheduled
     at the next BeginFrame instead of producing a frame on demand — which is why one frame per
     keystroke is marked "dropped, affecting smoothness" (see the report, §9) and why the
     application's own cost can only be measured with `prefers-reduced-motion: reduce`
     (`node tools/latency.mjs --reduced-motion`). caret.css and chrome.css both honour reduced
     motion — thank you, that is what makes the honest measurement possible.
* **chrome.js — the word count is ~4.9 ms of main-thread work per keystroke at 55k words**
  (`FireIdleCallback` in the trace). It is idle-scheduled, which is right, and it is invisible at
  10k words; at 55k it is the second-largest main-thread item in the run and it lands between
  keystrokes at 133 wpm. An incremental count (adjust by the words in the edited line) would
  remove it entirely.
* **files.js autosave is cheap and correctly debounced.** Instrumented `Storage.setItem` over a
  2,500-keystroke session with pauses: see `autosave` in `shots/latency/r2-long.json`. It fires
  400 ms after you stop, never during a burst.
* **Nothing may scan the whole document synchronously on `change`.** Unchanged from round 1.

### Round-1 changes, still in place

ResizeObserver instead of a layout read inside the keystroke; one `selection` event per keystroke
instead of three; writes before reads; no attribute storm on keydown. See the round-1 notes below
in the report (§8) for the A/B.

### The bench

`node tools/latency.mjs` — every regime types **real prose**: capitals, punctuation, quotes,
Enter, Backspace, undo, paste, select-and-replace and Markdown syntax, each keystroke labelled by
kind so Enter and a fence-opening backtick are reported separately instead of averaged away.
Twelve regimes, three sessions, bootstrap intervals on p50/p99, CPU throttling (`--throttle 4`),
a long session (`--long 2500`), true cold start with a new browser process (`--coldstart 10
[--fresh]`), and an idle frame-production control. A fresh page per regime, so the round-1
double-probe bug is now impossible. `bin/quill --measure` opens the app on a **virtual Hyprland
output** (`hyprctl output create headless`) so a real compositor really presents the frames while
nothing appears on the user's screen — Hyprland 0.56 needs the new Lua dispatcher for placement:
`hyprctl repl 'return hl.dispatch(hl.dsp.exec_cmd("[workspace N silent] …"))'`.

### Round 3 — what changed in `app/js/core.js`

Two of these are correctness fixes, not perf. They were found by rebuilding the deferred-render
correctness probe (`shots/latency/probes/correctness.mjs`), and they are **visible to markup**:

1. **`recomputeCtx(from)` seeded the walk from the wrong line.** `lineCtx[i]` is the context
   *entering* line i, so a walk starting at `from` must begin from the context *leaving* line
   `from-1` — `Writer.lineContext(lineCtx[from-1], lines[from-1], from-1)`, not `lineCtx[from-1]`.
   With the old seed the line just above the edit was skipped, so **pressing Enter at the end of
   a ` ```js ` line left every line below it tokenised as prose, permanently.** One line changed.
2. **`lineCtx` is now spliced with `lineEls`** when a keystroke changes the line count. It is
   indexed by line, and `recomputeCtx` decides where to stop by comparing what it computes for
   line *i* against `lineCtx[i]`; if the array is not moved with the lines, those comparisons are
   against the wrong lines. It stopped early on a false match and left the array short at the
   tail, so the last line of the document kept a null context.
   (Round 2's correctness probe found its fence with `lines().findIndex(l => l.startsWith('```js'))`
   — which matches the *document's own* fenced block near the top — so it never checked a single
   deferred line and saw neither bug.)
3. **`AHEAD` is measured, not asserted.** Round 2 hard-coded 64 lines and called it "more than a
   screenful at any font size". It is not, in a tall window at a small size. `computeAhead()` now
   divides the scroller's height by a line's `min-height` (exactly one line pitch — the shortest a
   line can be) and adds a margin, at boot, on resize and on a settings change. 32 lines at
   1440×900 / 20 px; ~58 on the user's 4K panel at 14 px. `Writer.aheadLines()` reports it.
4. **The catch-up serves the viewport first, and is bounded by time, not by a line count.** The
   AHEAD window follows the *edit*; the reader's eye need not be there (undo, a command, a paste,
   or simply having scrolled away). The catch-up frame now fills the visible ∩ pending range
   first — an animation frame runs *before* that frame's style/layout/paint, so it still lands in
   the first frame after the keystroke — and the remainder runs under a **4 ms** budget instead of
   400 lines. Scrolling into a not-yet-caught-up range also fills what came into view, on the
   scroll event. New: `Writer.visibleRange()`, `Writer.flushVisible()`, `Writer.pendingLines()`.
   Measured: 1,608 lines deferred by one keystroke in a 55k-word manuscript are caught up in **one
   frame**, and nothing stale is ever painted (all 17 assertions in `correctness.mjs` pass).

### Round 3 — new tools

* **`tools/uinput-keys.py`** (new). Types through a real virtual keyboard on `/dev/uinput`, so keys
  travel evdev → libinput → Hyprland → Wayland → Chromium exactly as the user's own keyboard does,
  and records `CLOCK_MONOTONIC` immediately before each `write(2)`. Chromium's TimeTicks *are*
  CLOCK_MONOTONIC on Linux (verified against `clock_gettime` — see the report §3.5), so the
  kernel → browser delivery hop is now **measured** instead of excluded. `bin/quill --uinput`.
* **`tools/idle-check.py`** (new). Watches every real keyboard/pointer evdev node and exits
  non-zero if anybody touches the machine. `bin/quill --panel N` refuses to take the screen
  without it, and puts the workspace that was up back afterwards.
* **`bin/quill --panel N`** (new). The one measurement a virtual output cannot give: commit →
  scan-out on the **physical** panel. A Wayland surface on a workspace nobody is looking at gets no
  frame callbacks, so the window has to really be on screen; the idle guard above is what makes
  that acceptable.
* **`tools/latency.mjs`**: every keystroke now carries five milestones, not two — JS done, the wait
  for a BeginFrame, painted, committed, presented — and the app-internal bar is answered in the
  **mean and the worst case**, which is how REFERENCE §5.3 states it. Statistics over fewer than
  20 samples print n/mean/max and no percentiles.

### Findings for other pieces (round 3; no files of yours were changed)

* **caret.js — unchanged from round 2 and still the largest perceptible latency in the product.**
  `GLIDE_X = 62`, `SNAP_MS = 60`: at any ordinary writing speed every keystroke glides, and the
  caret settles ~58 ms after the glyph is already on screen. Against Dan Luu's 2 ms perception
  threshold that is the one number a writer's eye can actually see. A 30 ms glide would still read
  as the same caret moving.
* **chrome.js — `count()` is still a whole-document scan.** ~0.9 ms of main thread per keystroke at
  10k words and ~4.9 ms at 55k, idle-scheduled (so off the critical path, which is right) but the
  second-largest main-thread item in a large document. Adjusting the count by the words in the
  edited line would remove it.
* **page.css — the `#mirror:has(> .line:only-child > br:only-child)::after` placeholder costs
  nothing measurable.** Re-checked this round by deleting the rule at run time: 5.06 vs 5.27 ms
  mean keydown→paint, inside the run-to-run spread. Keep it.
* **`contain: layout style` on `#mirror .line` is not worth it.** It does what it says — PrePaint
  406→327 µs and Paint 669→455 µs per keystroke at 10k words, 1741→1441 and 1917→1442 at 55k — but
  the end-to-end latency does not move (10k: 5.27 → 5.34 ms mean to paint; 55k: 9.01/9.21 →
  9.10), because the dominant term is the wait for the next BeginFrame, not the paint. Measured,
  twice, and left out.
