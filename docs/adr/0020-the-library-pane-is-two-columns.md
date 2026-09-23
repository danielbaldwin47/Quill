# The Library pane is two columns, one Location at a time

The Library pane is an **Organizer** beside a **File List**: the Organizer names the Locations, the
Pinned Documents and folders, and the Recents; the File List shows one Location's tree at a time.
It replaces the one-column pane that stacked every Location's full tree in one scroll. The pane's
paint is iA Writer for Mac's as measured (`dev/ref/ia/mac-native/NOTES.md` § State 28), cited value by
value by the Library makeover spec rather than bound by a `docs/design.md` row, because the Library
stays outside the Design oracle's reach ([ADR 0015](0015-the-design-oracle-outranks-the-parity-oracle.md)).
Decided 2026-09-14 with the owner, from
[#435](https://github.com/danielbaldwin47/Quill/issues/435) and the thirteen directions
[#438](https://github.com/danielbaldwin47/Quill/issues/438) put in front of them.

## Context

The [Library spec](https://github.com/danielbaldwin47/Quill/issues/25) ported the Parity oracle's
sidebar: one column on the paper, every Location a spaced section carrying its whole tree, Pinned
above them, search at the top, a sort-and-count line, a saved-status line at the foot. It won its
Piece, and the round-11 critic's gap named what it still lacked — "the pane has no tonal step from
the page … B's library reads as part of the page, not a pane beside it"
(`dev/progress/rounds/files-r11.json`). With more than one Location the owner's description was "one
massive filing cabinet drawer that's not separated out very well": the writer collapses Locations
and scrolls to find a thing.

The capture ticket [#379](https://github.com/danielbaldwin47/Quill/issues/379) then measured the
Design oracle's pane for the first time (State 28, PR #427; the frames' remaining values in
[#436](https://github.com/danielbaldwin47/Quill/issues/436)): two columns on three grounds, a
129.5 pt Organizer of four sections beside a 230.5 pt File List, 360 pt in all and dragging to 500,
68 pt rows, an accent bar for the selected row and nothing for hover, `Filter` at the foot, the
Sort pill carrying the Library's settings, no document count and no status line. Its closing
question was the one no measurement decides: "Quill's pane is one column and the oracle's is two."

ADR 0015 keeps the Library Quill's own. That is not a bar to taking the oracle's shape; it says the
shape is this decision's to take, on its merits, and that the values the spec cites from State 28
are the spec's decisions rather than design rows.

## Decision

**Two columns.** The Organizer holds three sections — **Locations**, the current one marked;
**Pinned**, one list across every Location, carrying a line of prose when empty; **Recents** — and
the File List shows the current Location's tree, a folder expanding in place. Switching Locations
happens in the Organizer; the open Document does not change with it, and a File List that does not
hold the open Document shows no selection.

**iA's paint, with the polish the prototypes added.** The grounds, greys, pitch, insets, bar and
foot field are State 28's numbers. On top of them the canvas rounds settled: the file icon carries
text lines; excerpts clip at the second line with no ellipsis; the New control is two bare glyphs;
the Organizer's heads align over the row icons; one corner radius and one glyph stroke weight across
the pane; the sort control holds its width under search; the selection bar has rounded ends and
stops short of the row's separators. Quill's Organizer has three sections where iA's has four, and
the Library makeover spec ([#437](https://github.com/danielbaldwin47/Quill/issues/437)) owns every
value the rounds left open.

**Paint and behaviour together.** The search field moves to the foot and matches names and
contents together, as it already does; the Sort pill takes the Library's settings out of the
Settings window; the count and the saved-status lines go; the title bar over the pane names the
current Location.

## Considered options

- **One column with a Location head** (the prototype's row C): the current Location as a switcher
  head, Pinned as a section, the oracle's rows and foot. It reads well and costs least, but it keeps
  the Locations and the Pinned Documents a menu away instead of in view, which is the thing the
  two columns are for.
- **iA's structure in Quill's greys** (row B): two columns but the File List on the page's ground.
  That is the round-11 gap kept on purpose.
- **A dark Organizer, or the whole pane a step off the paper** (rows D, L, M): more separation
  from the page, bought by darkening light mode or lowering the list below the paper. The owner
  chose iA's three grounds, which separate the pane on light already, over both.
- **Warm grounds, a serif, an icon rail, no separators, a monospace pane** (rows H, E, F, J, K):
  each a distinct identity; each passed over for the measured pane, which the owner called "WAY
  higher in objective quality" than what shipped.

## Consequences

**The glossary changes.** A Location is no longer "shown with its full tree" beside the others; the
File List shows one at a time. Pinned is a section of the Organizer, not a list above the Locations.
Organizer and File List enter `CONTEXT.md`.

**A `files` state may name a `mac-native` opponent.** #429 kept the Piece a Parity pair because a
`mac-native` crop would judge the two columns Quill did not draw. Once the spec's stub draws them,
a state can name the crop, re-shot in Mono as ADR 0015 has it.

**The palette grows grounds.** Three grounds cannot be drawn from today's roles, so the spec adds
theme roles for the Organizer's and the File List's grounds with a derivation for palette files
that do not name them.

**The prototypes are on record.** The canvas
(https://claude.ai/code/artifact/ab5ac0d6-a5f0-4473-abc2-a05b319881e3, rows A–M) and its generator
on `prototype/library-pane`, with the owner's reference crops of iA's dark pane. Nothing on that
branch ships; the spec's GTK stub is the reference the build is measured against.
