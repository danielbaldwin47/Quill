# Quill's writing surface: what it takes from the Design oracle

iA Writer for Mac running natively is the **Design oracle**; `legacy/` is the **Parity oracle**, what
the port ports. Where the two disagree on the writing surface, the row here decides, and a row may
decide a third thing. The evidence behind every row is `ref/ia/mac-native/VERDICTS.md` (cited by
row number; "found-here" is its closing table) and the captures under `ref/ia/shots/mac-native/`.
The principle, its reach, and how a state that follows a row is judged are
[ADR 0015](adr/0015-the-design-oracle-outranks-the-parity-oracle.md). Decided 2026-08-30 with the
owner, from the #154 captures.

## Rows

The Parity column names the behaviour and the file that holds its numbers; the numbers themselves
stay in that file. The Quill column is what Quill is to do: a row marked *agreed* is landed, and any
other row is landed when the ticket that names it closes.

| Behaviour | Design oracle | Parity oracle | **Quill** | Why · evidence |
|---|---|---|---|---|
| Caret column | 6 px bar **centred** on the advance boundary, 3 px each side | left edge a nudge past the boundary (`caret.js`) | **centred on the boundary** — sharpens ADR 0013, whose bar has its left edge there; `caret::left` is the rule and `Editor::bar()` calls it | 0013.1–0013.3; `01-caret-offset-*`. What #147 reports as "sits on the leading glyph" — agreed, and `caret/caret` is judged against `01-light-caret-midword` |
| Caret width | quantised **5 / 6 / 8 / 10 px** over the 14 sizes (0.080–0.186 em) | a fixed fraction of the em, rounded, never under 2 px (`caret.js`) | **the oracle's ladder**, one width per step, NOTES § 11. Ladder values are device px at scale 2; at any scale the bar is `round(value × scale / 2)` device px, at least 1, and an odd bar's extra pixel falls right of the boundary | 3.5.5; `11-*` |
| Caret height | the line pitch, within 1 px | the pitch | the pitch | 3.5.6 — agreed |
| Blink | **1.000 s**, 0.516 on / 0.484 off, ~90 ms ramps; **held solid while typing**, resumes **0.633 s** after the last key | a longer cycle, resuming sooner (`caret.css`, `caret.js`) | **the oracle's numbers**; `caret.rs` cites `blink-idle.tsv` and `blink-typing.tsv` | 3.5.8, found-here; `04-*`, `05-*` |
| Caret on window deactivation | **gone** | 30 % ghost, no blink (`caret.css`) | **the ghost** — the Parity oracle's, kept, so the `unfocused` judged state stays a still against it | A tiling desktop shows the active window less plainly than macOS; the ghost says where the writer was. Departs from the Design oracle on purpose; 0013.8 |
| Selection | fill only, no end bars, no caret while it stands | fill plus a bar at each end (`caret.css` `.sel-edge`) | **fill only**, since ADR 0014 | 0014.1–0014.3; `07-*` … `10-*` |
| Active fill | `#113d52` dark, `#ccedf8` light | accent at low alpha (`theme.css`) | **the oracle's hexes** | 4.2.9, 4.2.11 |
| Idle fill | `#464646` dark, `#dcdcdc` light — ink over paper at ≈ 0.25 / 0.12 alpha | ink at a lower alpha (`theme.css`) | **ink over paper at alpha 0.247 dark / 0.122 light** — the alphas that flatten to the oracle's hexes on the built-in paper | 4.2.11, 0014.9; `07-*`. Alpha survives a palette swap where a hex does not |
| Held newline | fills to the **container's right edge** | a half-em stub (`caret.js`) | **to the container's edge** | found-here; `10-newline-only` |
| Multi-row fill | interior rows span the **whole container**; last row from its left edge | ink extent per row (`caret.js`) | **the container** | `09-dark` |
| Paper · ink · dim | light `#f7f7f7` · `#191919` · `#c6c4c2`; dark `#1a1a1a` · `#cccccc` · `#707070` | marketing-frame readings, light off by a few units (`theme.css`) | **the oracle's hexes** | 4.2.1–4.2.6; `12-*`, `01-*`, `13-*` |
| Accent | `#00bfff`, **both themes** | one blue on both grounds, off by a few units (`theme.css`) | **`#00bfff`** | 4.2.7–4.2.8 |
| Dim tiers | **one** per theme; Sentence and Paragraph share it | three: bright, near (Sentence only), dim (`focus.js`, `focus.css`) | **one** — ADR 0015 supersedes ADR 0006's near tier | 4.2.12; `13-*`. Near was invented for the JavaScript app; it reads as blur |
| Typewriter | a value of the Focus-scope popup | independent boolean (`core.js`) | **independent** — ADR 0006 stands on independence; the surface, caret line at the window's vertical centre, matches | found-here; `15-dark`. A switch, so Quill's |
| What hangs | **headings only**, `#…` at level + 1 cells; `>`, `-`, `1.` sit on the body column; no quote rule | nothing (a textarea cannot); a 2 px quote rule (`markup.css`) | **headings only, no quote rule**. A wrapped list item's or quote's continuation rows: still unknown — no capture shows one; they sit on the body column until a `mac-native` capture of a wrapped item decides it | 4.1.10–4.1.13; `14-gutters`, `14-blocks` |
| Text container | 64-cell measure + **7-cell gutter each side**, 78 cells, centred in the window | the 64 measure centred inside a viewport-relative gutter (`page.css`) | **78 cells, centred** — [ADR 0016](adr/0016-the-text-container-is-78-cells.md) | 4.1.8–4.1.9, found-here; `09-select-all`. 7 cells is what `###### ` needs |
| Text sizes | **14 steps** (0–13), em 14.50 … 62.58 pt, default **step 5 = 21.33 pt** | integer px, default 20 (`type.css`, `chrome.js`) | **the oracle's ladder**: a new key `step` (0–13) replaces `size` in `settings.toml`, `--step` replaces `--size`, and `states.json` carries the step; an old `size` in px is read once, mapped to the step whose em is the nearest **at or above** it, and rewritten as `step`. At or above rather than nearest, so that no writer's type shrinks on an upgrade they did not ask for and the old default, 20 px, lands on the new one: nearest would send it to step 4, whose em is 19.25 (#162). A step's em in logical px is the ladder's pt value (macOS points are logical px): the default is 21.33 logical px, 42.67 device px at scale 2 | 4.1.2; NOTES § 11 |
| Line pitch | `pitch / em` falls **1.732 → 1.374** over the ladder, 1.711 at the default | a linear clamp (`type.css`) | **the ladder's pitch per step**, NOTES § 11, scaled as the caret width is (`round(value × scale / 2)`), within 1 device px at every step at scale 2; a fitted curve only if the ladder is ever extended | 4.1.3–4.1.4 |
| Measure | 64 cells default; 72 and 80 offered | 64 | 64 | 4.1.1 — agreed |
| Column | centred in the window | centred | centred | 4.1.8 — agreed |

## The palette is a file

Every colour Quill paints becomes a `Role` in `quill-engine::theme` (#110 moves the last literal
constants there). Six roles take the Design oracle's values above — paper, ink, dim, accent, active
and idle selection; the rest (marks, links, code ground, rules, shadow, chrome) keep the Parity
oracle's until they are measured (VERDICTS 4.2.13–4.2.15 are still unknown).

A writer who wants the desktop's colours sets one key, `palette = "<path>"`, in `settings.toml`, and
Quill loads a file of `[light]` and `[dark]` tables whose keys are the `Role` names in snake case
(`paper`, `ink`, `ink_dim`, `mark`, `accent`, `link`, `selection`, `selection_idle`, `code_bg`,
`rule`, `shadow`, `chrome_fg`, `chrome_fg_strong`); a slot the file omits, a scheme it omits, or a
file that is missing is the built-in, and unknown keys are ignored. The watch is on the file's
directory, because a desktop theme switch replaces the directory rather than rewriting the file.
`theme = auto|light|dark` and the portal still choose the scheme. The Gate judges the built-ins;
`--theme` pins them.

Omarchy renders `~/.config/omarchy/themed/<file>.tpl` (stock templates: `/usr/share/omarchy/default/themed/`)
from the theme's `colors.toml` into `~/.local/state/omarchy/current/theme/<file>` on every
`omarchy theme set`; `omarchy-theme-set-templates` is the contract. A `colors.toml` carries one
`mode`, so Quill's template writes the one table `[{{ mode }}]` and the other scheme stays the
built-in. Quill will ship `packaging/quill.toml.tpl`, mapping `background`, `foreground`,
`dark_foreground`, `muted`, `accent`, `selection` and a `{{ mix selection background N% }}` onto the
roles, and `README.md` § Build, install and run gets the two steps: copy the template into
`~/.config/omarchy/themed/`, and set `palette = "~/.local/state/omarchy/current/theme/quill.toml"`.

## Changing a row

A row changes on a `mac-native` capture with its measurement in `NOTES.md`, or on the owner's
decision recorded in the row's Why column.
