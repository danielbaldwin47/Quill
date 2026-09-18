# Settings window shape — a throwaway prototype (#464)

The question: what shape does the Settings window take, and how is it grouped
(danielbaldwin47/Quill#464)? Today's window is one scrolling `gtk::Grid` with no CSS
(`today/`, shot from `main` at 45af783). The rows on every board are exactly the ones
#463 leaves in Settings; Preview mode, Syntax highlight and the Style/Spell switches
are gone to the View menu.

Directions, each in light and dark, 1 CSS px = 1 logical px:

- **A1** a separate window, sidebar of six panes (Stack + StackSidebar); every pane drawn once, plus the window floating over the main window.
- **A2** a separate window, switcher on top (Stack + StackSwitcher, no GtkHeaderBar); five tabs, Advanced folded into General.
- **B1** a centred sheet over a dimmed Editor (Overlay), text tabs, boxed rows (a ListBox with CSS).
- **B2** a 360 px side pane over the Editor (Overlay + Revealer), drill-in from a root list.
- **C1 / C2** one scrolling window regrouped under six headings: C1 in stock GTK4 as sampled from `today/`, C2 the identical rows, order, widgets and width with Quill's CSS. Shown unrolled; the dashed line is where a 640 px window ends.
- **D** a Palette-style searchable list of every row.

Grouping proposed: General (Follow System, Hide Bars, Typewriter anchor) · Library
(Locations, Pinned, Files' four switches) · Template (the five, then Layout's three) ·
Export · Writing tools (Style check lists, Spell check Language) · Advanced
(Edit settings.toml…, the refused lines).

Only plain GTK4 widgets are drawn: Switch, DropDown, SpinButton, Scale, CheckButton,
Button, ListBox, Stack, Overlay, Revealer. No libadwaita, no GtkHeaderBar.

`python3 gen.py` rewrites every `.dc.html` and `canvas.json` from the skins and rows at
the head of the script; `node shoot.mjs` renders every board to `shots/<board>.png`
(`node shoot.mjs A1ExportDark.dc.html` for one) with the repo's `playwright-core` and
headless `/usr/bin/chromium`. Font stacks inside `style="…"` are single-quoted: a
double-quoted font name ends the attribute and silently drops the rest.

Nothing here ships; the pick is recorded on #464.
