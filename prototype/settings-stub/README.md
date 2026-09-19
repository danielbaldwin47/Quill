# Settings window stub — A1 with D's search (#464). Throwaway, never merged.

The question: can plain GTK4 with Quill's own CSS build the picked design
(`prototype/settings-window/` on branch `prototype/settings-window`), and what does the
HTML canvas get wrong? Answer: yes, all of it, with the differences listed under Findings.

## What was built

- `quill/src/settings/stub.rs` — the window, the search spike and the stylesheet. `settings::open`
  calls it; the old grid builder is cut out of `quill/src/settings.rs`, whose write functions
  (`switch`, `export_spin`, `export_papers`, `location_list`, …) the stub reuses, so every row
  writes the key it wrote before. `chrome::stylesheet` appends `settings::stylesheet(scheme)`, so
  the CSS reloads with the ground, light and dark. `window.rs` opens Settings by itself under
  `QUILL_STUB_OPEN`.
- **A1**: a `gtk::Window` (700x520), a 168 px sidebar beside a `gtk::Stack` of six panes. Rows
  #463 moved out are gone (Preview mode, Syntax highlight and its categories, the Style check and
  Spell check switches). Two rows are new to Settings and write through the same
  `Session::edit_settings`: Hide Bars (`chrome`) and the Template radios (`template.name`).
- **Sidebar: a `ListBox`, not a `StackSidebar`.** A `StackSidebar` owns its rows, so the search
  field cannot sit in its column without a wrapper, its selection cannot be told apart from an
  activation (the stub leaves the search on a click, not on a selection), and its rows come with
  the theme's `.sidebar` separators to undo. Six labels in a `ListBox` is less code than the undoing.
- **D**: a `GtkSearchEntry` at the head of the sidebar. While it holds text the outer `Stack`
  shows one flat `ListBox`. **The result rows are the panes' own widgets**: each settings row lives
  in a slot box in its pane, is moved into the result row while it matches, and is moved back when
  the field empties — one switch per setting, no second state. Two things cannot be the same
  widget: Locations, Pinned and the refused lines are jump rows (as the board draws them), and
  the board's single "Template" dropdown in D does not exist — the five radios are found one by one.
  A check or radio keeps its label on its own right, so in results the mark sits left of the
  label, not at the row's right end as `DQueryLight.png` draws it.

## Launch and shoot

    cargo build --release
    node prototype/settings-stub/shoot.mjs                 # 18 shots into shots/, light and dark
    node prototype/settings-stub/shoot.mjs dark export     # one
    FIELD=1 node prototype/settings-stub/keys.mjs palette h e a d Down Return Escape

Both scripts run on the Gate's headless output (`tools/harness.mjs` `openStage`), never on
workspace 1, and send no key to open the window: the binary reads `QUILL_STUB_OPEN`,
`QUILL_STUB_PANE`, `QUILL_STUB_QUERY`, `QUILL_STUB_POPUP`, `QUILL_STUB_KEYS=stock|palette`,
`QUILL_STUB_LOG`. By hand: `Ctrl+,` in any `cargo run`.

## Findings: where GTK differs from the canvas

Verified on the 2x headless stage against the boards unless marked.

1. **Switch is 36x18, not 32x18.** `GtkSwitch` is exactly two slider boxes wide; a 14 px knob
   with 2 px of air is 18 + 18. The spec takes 36x18 (or a 12 px knob for 32x16).
2. **No `text-transform`.** The small-caps heads are upper-cased in Rust; `letter-spacing` must
   be px (0.63px), GTK has no `em` there worth trusting. Size 10.5px/600 matches the board.
3. **SpinButton: CSS reaches all of it** (frame, 26 px cells, the divider, 24 px height). What it
   does not reach: the − and + are `GtkImage`s of `value-decrease-symbolic`/`value-increase-symbolic`
   from the icon theme (1 px strokes, not the board's 1.3), and `-gtk-icon-source` does not apply
   to them. The text cell needs `width-chars = 3`, not 2: two digits at `tnum` overflow 2 chars.
4. **DropDown button: reached**, but the arrow is a builtin icon drawn *stretched to its node's
   box*: a `url()` image fills the 10x22 allocation, so the chevron needs `margin: 6px 0` to stay
   square. `-gtk-recolor(url("data:…"))` draws nothing (it loads through a `GFile`); a plain
   `url("data:…")` works but is rasterised at 1x and soft at 2x. The stub writes the SVG to a temp
   file; the real thing is a gresource, a filled path (recolor fills strokes into wedges).
5. **DropDown popover: reached** (ground, border, 8 px radius, shadow, 24 px rows, accent hover),
   but it does not inherit the window's `font-size` — set it on `dropdown popover` again. The tick
   of the default factory sits straight after the label ("A4✓"); a tick column wants a list
   factory of our own. It is a separate surface and may hang past the window's edge.
6. **Scale: reached.** 3 px trough, 14 px knob with `margin: -6px -7px -5px` (11 px of overhang
   does not halve), value at the left by `set_value_pos`. Width is a `width_request`, not CSS.
7. **Check and radio: reached.** 12 px + 1 px border = the board's 14. The radio's dot is a
   `radial-gradient`; the tick is GTK's own `check-symbolic.svg` resource, recoloured.
8. **Row heights are `min-height`, borders add to them**: a path row with a `border-top` is
   `min-height: 31px` to stay 32. `overflow: hidden` for the boxed list's radius is
   `set_overflow`, not CSS.
9. **The search field widens the sidebar** to 224 px until `width-chars`/`max-width-chars` = 8.
   With it the sidebar is 168 and everything below the head sits 33 px lower than the board,
   which has no field: the spec should draw it.
10. Fonts: Adwaita Sans at 13px matches the board's Inter to the pixel; heads sit 1 px lower.
11. Colours are the boards' literals per `Scheme` (as `menu_ink` does), not theme `Role`s.
12. `line-height` was not needed; hint rows are `min-height: 46px`.

## Findings: the keyboard in the search list (`keys.mjs`, logged from the app)

- **Typing anywhere reaches the field**: `set_key_capture_widget(window)` is stock and works
  from a sidebar row, a result row or a switch.
- **Stock focus does the rest only by accident.** When the panes hide, GTK moves focus from the
  pane's first control to result row 0, and every further letter rebuilds the list and refocuses
  row 0. From there Down/Up walk rows (selection follows in Browse mode), Space and Enter fire
  `row-activated` (the stub toggles the switch/check, opens the dropdown, focuses a spin/scale),
  Tab enters the row's control where Space works natively. Verified: `number_headings`, `header`
  flipped in the file.
- **What fights:** (a) with focus *in the field*, Down leaves for the sidebar's first row, not the
  results (keynav goes to the next widget below), and Space/Enter there leave the search;
  (b) `stop-search` (Esc) fires only while the field has focus — key capture does not forward Esc,
  so Esc on a row or a switch does nothing; (c) `row.activate()` drags focus on to the row;
  (d) Space in the field types a space, so **Enter is the only operate key while typing**.
- **The Palette shape fixes all four** (default mode): a capture-phase key controller on the field
  for Down/Up (selection moves, focus stays), `activate` → emit `row-activated`, and a window-level
  capture controller for Esc (clear first, close second). Verified: Down, Enter twice, Up, Esc, Esc.
- After a dropdown pick focus lands on the dropdown's button, not the field; typing still reaches
  the field through key capture.

## Would not survive Windows/macOS as is

- The spin glyphs, the search magnifier and clear icon come from the icon theme
  (Adwaita symbolic): bundle them or draw our own.
- The chevron's temp file (`/tmp`, `file://`): make it a gresource.
- `tilde()` uses `glib::home_dir` and `/`; fine on macOS, wrong-looking on Windows.
- "Edit settings.toml…" is `gio::AppInfo::launch_default_for_uri`, as today.
- `CHROME_FONT` falls through to `sans-serif` unless Inter is loaded (it is, privately).
- `shoot.mjs`/`keys.mjs` are Hyprland-only. Nothing else is Linux-only: no libadwaita, no
  HeaderBar, no portal.

## Not verified

- By hand: nothing was clicked or dragged; pointer hover states, the Add… dialog, Remove, the scale
  drag and the Language dropdown's no-dictionary line were not exercised in the stub.
- Hide Bars and the Template radios write the file; that every open window follows was not watched.
- In the dropdown popup, an injected Down did not move and Enter picked row 0 ("auto"): the
  injector addresses the window, not the popup surface, so this says nothing about real keys.
- `cargo test` was not run (clippy `--all-targets` compiles the tests; the rows-layout tests
  still describe the old grid).
- A live scheme change with the window open, 1x outputs, and a window resize.
