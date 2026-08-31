# Quill's writing surface: what it takes from the Design oracle

iA Writer for Mac running natively is the **Design oracle**; `legacy/` is the **Parity oracle**, what
the port ports. Where the two disagree on the writing surface, the row here decides, and a row may
decide a third thing. The evidence behind every row is `ref/ia/mac-native/VERDICTS.md` (cited by
row number) and the captures under `ref/ia/shots/mac-native/`; the principle and the Gate rule are
[ADR 0015](adr/0015-the-design-oracle-outranks-the-parity-oracle.md). Decided 2026-08-30 with the
owner, from the #154 captures.

**Reach.** The writing surface: type, page, markup rendering, caret and selection, the two palettes,
Focus and Typewriter. Chrome, the Library, Preview, file handling and every setting's *switch* stay
Quill's own; the Preview captures (`mac-native-16-*`) wait for Preview's spec.

**How a departure is judged.** A judged state whose behaviour follows a row here cannot win against
the Parity oracle's frozen shot. Such a state names its opponent in `shots/oracle/states.json`: a
crop of a `mac-native` capture, with ours shot in Mono at the Design oracle's default size so cells
and gutters compare one-to-one. A row only a hand shows — blink, typing, deactivation — is a `keys`
assertion or a Hand-test step, never a still. The tooling for a per-state opponent is its own ticket
before the first re-judge (`docs/agents/gate.md` § Ticket tier).

## Rows

| Behaviour | Design oracle | Parity oracle | **Quill** | Why · evidence |
|---|---|---|---|---|
| Caret column | 6 px bar **centred** on the advance boundary, 3 px each side | left edge 0.07 em past the boundary | **centred on the boundary** | 0013.1–0013.3; `01-caret-offset-*`. Cures #147's "sits on the leading glyph" |
| Caret width | quantised **5 / 6 / 8 / 10 px** over the 14 sizes (0.080–0.186 em) | `round(0.155 em)`, min 2 px | **the oracle's ladder**, NOTES § 11 | 3.5.5; `11-*` |
| Caret height | the line pitch, within 1 px | the pitch | the pitch | 3.5.6 — agreed |
| Blink | **1.000 s**, 0.516 on / 0.484 off, ~90 ms ramps; **held solid while typing**, resumes **0.633 s** after the last key | 1.055 s, 470/85/445/55; resumes 480 ms after | **the oracle's numbers**; `caret.rs` cites `blink-idle.tsv` and `blink-typing.tsv` | 3.5.8; `04-*`, `05-*` |
| Caret on window deactivation | **gone** | 30 % ghost, no blink | **the ghost** — Quill's own | A tiling desktop shows the active window less plainly than macOS; the ghost says where the writer was. Departs from both oracles on purpose; 0013.8 |
| Selection | fill only, no end bars, no caret while it stands | same since #108 | fill only | ADR 0014 — agreed |
| Active fill | `#113d52` dark, `#ccedf8` light | accent at 0.22–0.23 alpha | **the oracle's hexes** | 4.2.9, 4.2.11 |
| Idle fill | `#464646` dark, `#dcdcdc` light — ink over paper at ≈ 0.25 / 0.12 alpha | ink at 0.12 / 0.10 alpha | **ink-alpha at the oracle's strength** | 4.2.10, 0014.9; `07-*`. Alpha survives a palette swap where a hex does not |
| Held newline | fills to the **container's right edge** | 0.5 em stub | **to the container's edge** | Found-here table; `10-newline-only` |
| Multi-row fill | interior rows span the **whole container**; last row from its left edge | ink extent per row | **the container** | `09-dark` |
| Paper · ink · dim | light `#f7f7f7` · `#191919` · `#c6c4c2`; dark `#1a1a1a` · `#cccccc` · `#707070` | `#f9f9f9` · `#1c1c1c` · `#cccccc`; dark as the oracle | **the oracle's hexes** | 4.2.1–4.2.6; `12-*`, `01-*`, `13-*` |
| Accent | `#00bfff`, **both themes** | `#00b5ff`, both | **`#00bfff`** | 4.2.7–4.2.8 |
| Dim tiers | **one** per theme; Sentence and Paragraph share it | three: bright, near (dim lifted 15 % / 26 % toward ink, Sentence only), dim | **one** | 4.2.12; `13-*`. Near was invented for the JavaScript app; it reads as blur |
| Typewriter | a value of the Focus-scope popup | independent boolean | **independent** (ADR 0006 stands); the surface — caret line at the window's vertical centre — matches | Found-here; `15-dark`. A switch, so Quill's |
| What hangs | **headings only**, `#…` at level + 1 cells; `>`, `-`, `1.` sit on the body column | nothing (a textarea cannot) | **headings only** | 4.1.10–4.1.13; `14-gutters`, `14-blocks` |
| Text container | 64-cell measure + **7-cell gutter each side**, 78 cells, centred in the window | the 64 measure centred inside a CSS gutter clamp | **78 cells, centred** — [ADR 0016](adr/0016-the-text-container-is-78-cells.md) | 4.1.8–4.1.9, Found-here; `09-select-all`. 7 cells is what `###### ` needs |
| Text sizes | **14 steps**, em 14.50 … 62.58 pt, default **21.33 pt** | integer px 10–40, default 20 | **the oracle's ladder** | 4.1.2; NOTES § 11 |
| Line pitch | `pitch / em` falls **1.732 → 1.374** over the ladder, 1.711 at the default | `clamp(1.30·em + 10.4 px, 1.52·em, 2·em)` | **fitted to the oracle's curve** | 4.1.3–4.1.4 |
| Measure | 64 cells default; 72 and 80 offered | 64 | 64 | 4.1.1 — agreed |
| Column | centred in the window | centred | centred | 4.1.8 — agreed |

## The palette is a file

Every colour Quill paints is a `Role` in `quill-engine::theme`, and the built-in light and dark
tables are the Design oracle's hexes above. A writer who wants the desktop's colours sets one key,
`palette = "<path>"`, in `settings.toml`, and Quill loads a file of `[light]` and `[dark]` tables
keyed by `Role`; a slot the file omits is the built-in for that scheme, a file with one scheme serves
both, and the file is watched like `settings.toml`. `theme = auto|light|dark` and the portal still
choose the scheme; `--theme` pins the built-ins, and an external palette is never judged. Chrome
colours join the `Role`s when chrome lands (#43), and the file tolerates keys it does not know.

Omarchy themes an app through a template, `~/.config/omarchy/themed/<file>.tpl` (stock ones live
in `/usr/share/omarchy/default/themed/`), rendered from the theme's `colors.toml` into
`~/.local/state/omarchy/current/theme/<file>` on every `omarchy theme set`. Quill ships
`quill.toml.tpl` under `packaging/`, mapping `background`, `foreground`, `dark_foreground`, `muted`,
`accent`, `selection` and a `mix` of selection and background onto the `Role`s, and its README gives
the one `palette =` line. matugen renders the same file through its own template.

## Changing a row

A row changes on evidence from the running Mac app — a capture under `ref/ia/shots/mac-native/`
with its measurement in `NOTES.md` — or on the owner's decision recorded in the row's Why column.
A marketing frame changes nothing: three readings of one were wrong (ADRs 0013, 0014).
