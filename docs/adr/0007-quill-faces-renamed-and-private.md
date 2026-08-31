# The Faces are renamed derivatives of the iA fonts, loaded privately

*Extended on 2026-08-31 by [#95](https://github.com/danielbaldwin47/Quill/issues/95): the script
freezes every glyph's advance across the `wght` axis as well, five of the six Faces having varied
one, so that setting a run bold leaves it the width it was. The decision — renamed derivatives, built
by one committed script, loaded privately — stands.*

Quill ships six font files built from the iA Writer variable fonts in `ref/ia/fonts` by one committed
script, renamed inside their `name` tables to **Quill Duo**, **Quill Quattro** and **Quill Mono**, each
Italic a family of its own (**Quill Duo Italic** and so on), and loads them into the process's own
fontconfig at startup rather than installing them. Settled in
[#16](https://github.com/danielbaldwin47/Quill/issues/16).

## Considered options

The spike ([ADR 0004](0004-gtktextview-editor.md)) relabelled the files at match time with a
`<match target="scan">` rule under a generated `FONTCONFIG_FILE`. That fixes the matching bug but not
the licence: the OFL FAQ counts a format conversion or any table edit as a Modified Version, and §3
forbids one from carrying the Reserved Font Name "iA Writer", which lives in the file's `name` table
where a scan rule never reaches. It also replaces the user's fontconfig for the whole process and every
child, and a Flatpak sandbox ships its own. Installing into `/usr/share/fonts` needs no code but puts
the Faces in every application's font picker and moves under Flatpak. Generating the files in
`build.rs` or the PKGBUILD makes `python-fonttools` a build dependency and the Gate's frozen shots
depend on a rebuild.

The Italic files could become an `Italic` style of one family now that the `name` table is under our
control; the separate-family form is what the spike proved against the "Regular" instance name each
Italic file carries, and the Editor selects Faces from its own table by name anyway.

## Consequences

**The rename is written into the files.** `tools/fontbuild.py` (`python-fonttools`, a tool-time
dependency only) reads the six `*V*.ttf` files from `ref/ia/fonts`, rewrites family, subfamily, full,
PostScript and `fvar` instance names with the prefix `Quill` held in one constant, and applies the
Quattro Italic patch that `tools/fontgrid.py` made for the web app: the space (glyph 1) advance 600 →
450 units to match the Roman, with its `gvar` entry frozen. It then freezes every glyph's advance
across the `wght` axis, by zeroing the advance phantom point of each `gvar` tuple that varies on it,
so that a run of text keeps its width when it is set bold ([#95](https://github.com/danielbaldwin47/Quill/issues/95)).
Outputs are committed under `fonts/` at the repo root, and the script is a pure function of its
inputs: a rebuild that changes nothing changes no bytes.

**Fonts are private to the process.** Startup calls `FcConfigAppFontAddDir` on the current fontconfig
with the font directory (`/usr/share/quill/fonts` from the package, `<repo>/fonts` in a development
build) before GTK initialises. A headless test asserts `Quill Duo` resolves to the shipped file. The
`<match target="scan">` rule and `FONTCONFIG_FILE` from the spike go away, which retires the first of
ADR 0004's five constraints.

**Attribution ships twice.** `fonts/OFL.txt` carries the iA and IBM Plex copyright lines, the licence
and a line naming the modifications; the package installs it beside the fonts and under
`/usr/share/licenses/quill/`. The About dialog credits the Faces as modified versions of iA Writer Duo,
Quattro and Mono by Information Architects, based on IBM Plex, under the SIL OFL 1.1.

**The prefix is one constant.** The pending name-collision check on "Quill" can rename every Face by
changing it and regenerating.
