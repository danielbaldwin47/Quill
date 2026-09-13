# The Mac rig

What shot and measured the captures in `ref/ia/shots/mac-native/`. It is the Mac counterpart of
`shots/caret/ia/wine/`, and it keeps that rig's vocabulary — *band*, *bar*, *fill*, *ink*,
`gap<`/`gap>`, *solid*, *cut* — so the two evidence sets can be put in one table.

It measures with Pillow rather than ImageMagick, because this machine has none, and it reads regions
in **logical points** (the way `screencapture -R` takes them) while every number it prints is in
**device pixels** at the display's backing scale.

| file | what it is |
|---|---|
| `iarig.py` | the reading: ground, bands, bars, fills, ink runs, gaps, and the burst that beats the blink |
| `fast.py` | the same readings off a numpy array, for the sweeps — `iarig`'s per-pixel scans are far too slow for three frames a size |
| `states.py` | the driver: `reset()` lays the passage down fresh and puts the caret at Home, so no state depends on the one before it |
| `drv.sh` | one verb per invocation for the menus, keys and window bounds |
| `run_theme.py` | states 1, 2, 3, 6, 7, 8 and 10 for whichever appearance the app is in — run once per theme |
| `run_caret.py` | the first pass at the caret and selection states, kept because its numbers are quoted in NOTES.md |
| `sizes.py` | state 11: walks the Text Size menu from the bottom until the geometry stops moving |
| `blink.py` | samples the caret's own pixels through Quartz at ~100 Hz — `screencapture` cannot time a blink |
| `typeblink.py` | state 5: the same sampler on its own thread while keystrokes go in, so frames and keys share one clock |
| `rows.py` | row bands taken from an unselected frame and read back out of a selected one, because a multi-row band has no gaps to split on |
| `caretat.py`, `selN.sh` | put the caret at a known offset, or hold a known number of cells |
| `run_markup.py` | state 17: every mark kind at rest, one frame per ground, plus a caret-on-heading control |
| `marks.py` | a glyph run's ink — the colour furthest from the paper it holds at least six times |
| `inks.py` | a line's runs grouped by the ink each carries, so a change of ink prints as one row |
| `colour.py`, `display.icc` | a capture put back into the profile the committed captures were taken in |
| `run_narrow.py` | #344, state 22: the Editor at seventeen window widths, text sizes and line-length limits — plain, three selection fills and select-all each, and the advance fitted over the fills. `--remeasure` re-reads the numbers off the committed frames without shooting |
| `sweep_narrow.py` | #344: bisects the window width the type changes at. The pitch alone separates the size classes, so one frame a width is enough; its frames are scratch, and the two sides of each break are shot as states by `run_narrow.py` |
| `run_templates.py` | #343, state 23: two passages in all four Preview Templates, each with an Editor control frame, for the first-line indent and the em |
| `manifest_2026_09_10.py`, `measure_2026_09_10.py` | the 2026-09-10 manifest, and the reader that verifies it and re-reads every number in [`../CAPTURE-2026-09-10.md`](../CAPTURE-2026-09-10.md) off the frames |
| `run_style.py` | #354, state 24: the Style Check mark on both grounds, per list, under Focus, under Syntax highlight and over a selection — each state shot twice, once with Style Check off, so every reading is a difference between two frames. The four list items carry no check the menu can be asked for, so it measures a list's state instead: a click is kept only if the frame moved the way the click should move it |
| `measure_style_354.py`, `manifest_style_354.py` | #354's reader — the mark's colour, thickness and position, read in the columns a row's control frame leaves blank between two glyphs, plus the face's own strikeout metrics straight out of the `OS/2` table — and the manifest beside it |
| `run_spell_400.py` | #400, state 26: the misspelling mark on both grounds, under Focus, under Syntax and over a selection, plus the word being typed, the autocorrect run and the correction menu. #354's shape — every state shot twice, once with Check Spelling While Typing off — and three things learned the hard way: the switch alone does not re-check a document already on the screen and `Check Document Now` finds the *next* misspelling rather than all of them, so the passage is pasted again under each setting; the pill over a word being typed takes Escape and ⌘↑, so a mouse click ends the word; and the mark is dotted, so aiming a right-click at the midpoint of a row's differing columns hits a word in between |
| `measure_spell_400.py`, `manifest_400.py` | #400's reader — the mark's colour, dots, thickness and place against the baseline, read only where its ground is clean, with each mark named by the word it stands under and the coverage over a selection solved on the raw frames — and the manifest beside it |
| `display-calibrated-2025-12-15.icc` | the DisplayCAL profile the built-in display carries now, so the two can be compared rather than assumed equal |

## Running it

Needs Python with `Pillow`, `numpy` and `pyobjc-framework-Quartz` — on this machine they live in
`.venv-rig/` at the repository root, not in the system Python — and the terminal must hold both
**Screen Recording** (or `screencapture` returns a black frame) and **Accessibility** (or `osascript`
cannot reach the app). Paths are relative to the repository root:

    python3 rig/run_theme.py <output-dir> dark

Set `IA_SAMPLE` to shoot a passage other than `ref/sample.md`.

Six cautions learned the hard way, all recorded in `../NOTES.md`, and three more for a Syntax
state in [`../CAPTURE-ORIGINAL-MBP.md` § Repeating the run](../CAPTURE-ORIGINAL-MBP.md#repeating-the-run):

- **A capture carries the display's profile**, so a frame shot today does not hold the same numbers
  as one shot for states 1–16 unless it is converted (`colour.py`). Every new capture goes through
  `normalise()` and then `check()`, which fails unless the paper and the body ink land back on the
  values the § 4.2 rows hold. `check()` covers neutrals only: it passes on a frame whose saturated
  colours are clamped, which is what the caret does — see the report above.

- **`screencapture -l <windowid>` returns a black frame** for an occluded window on macOS 27, so the
  deactivated states (6 and 7) close Finder's windows and activate Finder instead — focus moves and
  nothing is drawn over the editor.
- **Measure boundaries off a selection fill, never off a character count**, and turn Style Check off
  before shooting anything but state 24, which is the state of Style Check itself. Reading a
  style-check marker as a selection is the mistake
  [#154](https://github.com/danielbaldwin47/Quill/issues/154) exists to stop repeating.
- **`screencapture` will not write a dotted filename.** A scratch frame named `.scratch-00.png`
  fails with "cannot write file to intended destination"; the same name without the dot writes.
- **The Focus menu's item names are read lazily, and the first read of a session can be stale** —
  it came back with `Enable Style Sheck` and `Other` where every later read gives
  `Enable Style Check` and `Reference`. Read the menu before trusting a name.
- **Do not paste over the app's own sample documents.** `states.reset()` is Select-All then paste;
  make a scratch document with File -> New in Library first.
