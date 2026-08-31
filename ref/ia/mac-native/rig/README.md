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

## Running it

Needs Python with `Pillow`, `numpy` and `pyobjc-framework-Quartz`, and the terminal must hold both
**Screen Recording** (or `screencapture` returns a black frame) and **Accessibility** (or `osascript`
cannot reach the app). Paths are relative to the repository root:

    python3 rig/run_theme.py <output-dir> dark

Set `IA_SAMPLE` to shoot a passage other than `ref/sample.md`.

Three cautions learned the hard way, all recorded in `../NOTES.md`:

- **A capture carries the display's profile**, so a frame shot today does not hold the same numbers
  as one shot for states 1–16 unless it is converted (`colour.py`). Every new capture goes through
  `normalise()` and then `check()`, which fails unless the paper and the body ink land back on the
  values the § 4.2 rows hold.

- **`screencapture -l <windowid>` returns a black frame** for an occluded window on macOS 27, so the
  deactivated states (6 and 7) close Finder's windows and activate Finder instead — focus moves and
  nothing is drawn over the editor.
- **Measure boundaries off a selection fill, never off a character count**, and turn Style Check off
  before shooting anything. Reading a style-check marker as a selection is the mistake
  [#154](https://github.com/danielbaldwin47/Quill/issues/154) exists to stop repeating.
