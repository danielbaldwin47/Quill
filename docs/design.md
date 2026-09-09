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
| Caret on window deactivation | **gone** | 30 % ghost, no blink (`caret.css`) | **the ghost, at alpha 0.3** — the Parity oracle's, kept; the `unfocused` judged state stays a still, but it is **asserted** rather than paired ([ADR 0017](adr/0017-a-judged-state-neither-oracle-can-arbitrate.md)): the bar keeps its column, width and band lit or ghosted, and the ghost is the lit bar at 0.3 over the paper (`caret::GHOST` in `quill/src/caret.rs` holds the app's copy) | A tiling desktop shows the active window less plainly than macOS; the ghost says where the writer was. Departs from the Design oracle on purpose; 0013.8. Neither oracle holds a deactivated caret at the Caret column row's column, so no critic can arbitrate it — agreed |
| Selection | fill only, no end bars, no caret while it stands | fill plus a bar at each end (`caret.css` `.sel-edge`) | **fill only**, since ADR 0014 | 0014.1–0014.3; `07-*` … `10-*` |
| Active fill | `#113d52` dark, `#ccedf8` light | accent at low alpha (`theme.css`) | **the oracle's hexes** | 4.2.9, 4.2.11 |
| Idle fill | `#464646` dark, `#dcdcdc` light — ink over paper at ≈ 0.25 / 0.12 alpha | ink at a lower alpha (`theme.css`) | **ink over paper at alpha 0.247 dark / 0.122 light** — the alphas that flatten to the oracle's hexes on the built-in paper | 4.2.11, 0014.9; `07-*`. Alpha survives a palette swap where a hex does not |
| Held newline | fills to the **container's right edge** | a half-em stub (`caret.js`) | **to the container's edge** | found-here; `10-newline-only` |
| Multi-row fill | interior rows span the **whole container**; last row from its left edge | ink extent per row (`caret.js`) | **the container** | `09-dark` |
| Paper · ink · dim | light `#f7f7f7` · `#191919` · `#c6c4c2`; dark `#1a1a1a` · `#cccccc` · `#707070` | marketing-frame readings, light off by a few units (`theme.css`) | **the oracle's hexes** | 4.2.1–4.2.6; `12-*`, `01-*`, `13-*` |
| Accent | `#00bfff`, **both themes** | one blue on both grounds, off by a few units (`theme.css`) | **`#00bfff`** | 4.2.7–4.2.8 |
| Markers | **the body's own ink** at every mark kind, both grounds: a heading's `#`s, a quote's `>`, a bullet, an ordered marker, a task box, a thematic break, a fence and its info string, the inline-code marks, the emphasis runs and a bare URL. No quiet tier, no hair tier | three tiers: `--md-mark-quiet` at 72 % for a heading's `#`s, a quote's `>`, fences, code marks and URLs; the full `--mark` for bullets, task boxes and a fence's info string; `--md-hair` at 34 % for a thematic break (`markup.css`) | **the ink** — `mark` carries `ink`'s value on both built-in grounds and stays a role of its own for a writer's `palette` file. No quiet or hair role is added: the oracle shows no ladder to add one for — agreed | 4.2 § Marker ink; `17-*-marks`. #198 measured before porting, which is the whole of ADR 0015; the ladder was the Parity oracle's and has no counterpart here |
| Link | the **words** are body ink; the `[`, `]`, `(`, `)` and the **destination** are `#7a7a78` dark / `#b5b3b0` light, just above the dim tier; a 4 px rule under the **destination only**, `#545452` / `#d5d3d1`, the same under a full-ink URL as under a quieted one | the words in a blue of their own, the brackets and destination in the marker grey, the rule at 35 % of the blue (`markup.css`) | **the oracle's three values**, and its shape: a link draws nothing itself, `link` is the plumbing's grey, `link_rule` is the hairline, and the rule goes under the destination and nothing else | 4.2.13; `17-*-marks`, read a row at a time in NOTES § Found here. **One departure, knowing**: the oracle rules a bare URL over its whole length, and ours rules an angle autolink not at all — the rule is a mark of the destination and an autolink has none. Quill does not linkify a bare URL either (`markdown.rs` § `options` enables no autolink extension), so the form the oracle was measured on does not arise; a ticket that linkifies bare URLs takes the rule with it |
| Code ground | `#252525` dark, `#eeeeee` light — one ground, inline and fenced alike | 6 % white / 4.5 % black (`theme.css`) | **a wash rather than a hex**, at the alphas that land on the oracle's two greys over each ground's own paper (`quill-engine/src/theme.rs` holds them, as it holds the idle fill's) — an alpha survives a palette swap where a hex does not | found-here; `17-*-marks` |
| Dim tiers | **one** per theme; Sentence and Paragraph share it | three: bright, near (Sentence only), dim (`focus.js`, `focus.css`) | **one** — ADR 0015 supersedes ADR 0006's near tier | 4.2.12; `13-*`. Near was invented for the JavaScript app; it reads as blur |
| Typewriter | a value of the Focus-scope popup | independent boolean (`core.js`) | **independent** — ADR 0006 stands on independence; the surface, caret line at the window's vertical centre, matches | found-here; `15-dark`. A switch, so Quill's |
| What hangs | **headings only hang out** into the gutter, `#…` at level + 1 cells; `>`, `-`, `1.` sit on the body column, and no quote rule. A wrapped list item's or quote's **continuation rows hang in**, under the item's own first word: `- ` and `> ` anchor them 2 cells past the body column and `123. ` 5 — the marker run's own advance rather than a count per kind — and a wrapped quote carries **no second `>`** | nothing (a textarea cannot); a 2 px quote rule (`markup.css`) | **the oracle's, in both directions**: a heading hangs out by its measured marker run so its words land on the body column, and a list item or a quote leaves its marker on that column and hangs its wrapped rows in by its own run. No quote rule. `quill/src/tags.rs` sets each as a `(left margin, indent)` pair and measures the run off a layout rather than counting it off the cell, which is what counting lost round 10 | 4.1.10–4.1.13; `14-gutters`, `14-blocks` for the first row, `19-{light,dark}-wrapped-markers` and their `-h6` pair for the rows under it. `ref/ia/mac-native/CAPTURE-2026-09-09.md` § #241 and NOTES § State 14 read continuation ink at 726–729 and 804–805 against a body advance of 673 — 2 × 25.6 and 5 × 25.6 past it — on both grounds and under `###### ` alike. #241, and the first use of § Adding or changing a row: the interim rule this replaces held the continuations on the body column until a capture decided it |
| Text container | 64-cell measure + **7-cell gutter each side**, 78 cells, centred in the window | the 64 measure centred inside a viewport-relative gutter (`page.css`) | **78 cells, centred** — [ADR 0016](adr/0016-the-text-container-is-78-cells.md) | 4.1.8–4.1.9, found-here; `09-select-all`. 7 cells is what `###### ` needs |
| Text sizes | **14 steps** (0–13), em 14.50 … 62.58 pt, default **step 5 = 21.33 pt** | integer px, default 20 (`type.css`, `chrome.js`) | **the oracle's ladder**: a new key `step` (0–13) replaces `size` in `settings.toml`, `--step` replaces `--size`, and `states.json` carries the step; an old `size` in px is read once, mapped to the step whose em is the nearest **at or above** it, and rewritten as `step`. At or above rather than nearest, so that no writer's type shrinks on an upgrade they did not ask for and the old default, 20 px, lands on the new one: nearest would send it to step 4, whose em is 19.25 (#162). A step's em in logical px is the ladder's pt value (macOS points are logical px): the default is 21.33 logical px, 42.67 device px at scale 2 | 4.1.2; NOTES § 11 |
| Line pitch | `pitch / em` falls **1.732 → 1.374** over the ladder, 1.711 at the default | a linear clamp (`type.css`) | **the ladder's pitch per step**, NOTES § 11, scaled as the caret width is (`round(value × scale / 2)`), within 1 device px at every step at scale 2; a fitted curve only if the ladder is ever extended | 4.1.3–4.1.4 |
| Block spacing | a blank Markdown line is **exactly one empty line**: a paragraph break is 2 × pitch, and a heading carries **no margin above or below it**, so heading to paragraph is 2 × pitch too. Heading-to-heading is 74 against body's 73, the taller line box and nothing else | the same rule in its own words — a blank line is exactly one pitch with no extra paragraph margin, and nothing pads or borders a line (`page.css`) | **one pitch per blank line, headings on the body grid, no per-tag spacing** — `leading()` splits `pitch − row` the same way for every paragraph and a heading tag carries only its weight and its hang. Agreed: both oracles hold the one rule, so air above a heading that differs from the Parity oracle's is the Line pitch row's arithmetic and not a gap of its own | 4.1.5, 4.1.6, found-here; NOTES § State 14, off `14-dark-markup` and `17-dark-marks`. Measured before porting, the way row Markers was |
| Page top | the first line box **164 device px** below the editor's top edge at the default step, a heading's first ink 185 (NOTES § The page top, off `03-dark-caret-empty-document` for the box and `01-dark-caret-midword`, `01-light-caret-midword`, `08-dark-selection-inline`, `14-dark-markup` and `17-dark-marks` for the ink) — and the editor's top edge is the window's, iA's text running to the frame under a title bar that draws nothing | 2 × pitch, from an 85 pt reading of iA of unrecorded provenance (`page.css` `--page-top`) | **2 × pitch, kept** — `page_top()` holds it, and it lands under the oracle's 164. The measurement is not yet portable: how much of the 164 is room for the title bar drawn over iA's text has no counterpart in a window whose chrome is opaque, and no capture at a second step is at the document top, so whether it scales with the pitch is unread. `page_top()` moves when one capture settles both, which is #231 | found-here; NOTES § The page top. ADR 0015 gives the space to the Design oracle and asks that "an engine constant that follows a row cites the capture that measured it" — this one has no capture to cite yet |
| Measure | 64 cells default; 72 and 80 offered | 64 | 64 | 4.1.1 — agreed |
| Column | centred in the window | centred | centred | 4.1.8 — agreed |

## The palette is a file

Every colour Quill paints becomes a `Role` in `quill-engine::theme` (#110 moves the last literal
constants there). Ten roles take the Design oracle's values above — paper, ink, dim, accent, active
and idle selection, the marks, the link's two greys and the code ground; the rest (rules, shadow,
chrome) keep the Parity oracle's until they are measured (VERDICTS 4.2.14–4.2.15 are still
unknown, and 4.2.13's wikilink brackets, content-block chip, autocomplete popup and library list
with them).

A writer who wants the desktop's colours sets one key, `palette = "<path>"`, in `settings.toml`, and
Quill loads a file of `[light]` and `[dark]` tables whose keys are the `Role` names in snake case
(`paper`, `ink`, `ink_dim`, `mark`, `accent`, `link`, `link_rule`, `selection`, `selection_idle`,
`code_bg`, `rule`, `shadow`, `chrome_fg`, `chrome_fg_strong`, and Syntax highlight's `syntax_noun`,
`syntax_verb`, `syntax_adjective`, `syntax_adverb`, `syntax_conjunction`); a slot the file omits, a
scheme it omits, or a file that is missing is the built-in, and unknown keys are ignored. The watch
is on the file's directory, because a desktop theme switch replaces the directory rather than
rewriting it.
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

## Adding or changing a row

A row is written or changed on a `mac-native` capture with its measurement in `NOTES.md`, or on
the owner's decision recorded in the row's Why column. A spec or ticket that needs a Design-oracle
value no capture holds files a **capture ticket** rather than reading a marketing still or guessing:
a sub-issue of #154 (the Design oracle's parent issue), shaped like #231 § What would settle it —
the states to shoot, at the rig `ref/ia/mac-native/NOTES.md` § The rig describes, and what each
settles. It carries `ready-for-human` while the owner shoots and `ready-for-agent` once the
captures are pushed, for the measuring half: the numbers into `NOTES.md`, the verdict rows into
`VERDICTS.md`, and the row here. Until it lands no row is written, the state names no `opponent`
in `states.json`, and it stays a Parity pair against `legacy/`.

A capture ticket serves a row inside [ADR 0015](adr/0015-the-design-oracle-outranks-the-parity-oracle.md)'s
reach, the writing surface. A spec wanting Design-oracle evidence outside it — the Library, Preview,
chrome — files the same ticket, and its answer is the spec's own decision rather than a row here,
unless an ADR widens the reach.
