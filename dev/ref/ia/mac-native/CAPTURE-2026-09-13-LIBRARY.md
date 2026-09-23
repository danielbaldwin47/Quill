# Mac captures on the original rig — #379, the Library pane

The pane, its list, its rows, its excerpts, its search field and its two bars, shot on the
**built-in screen of the original 14-inch M1 MacBook Pro**.

No committed capture showed the Library at all: `NOTES.md` § The rig reads *Library hidden* for
every state, and `VERDICTS.md` 4.2.13 has the library list down as **still unknown**. Quill's own
pane is drawn to `dev/legacy/app/css/files.css`, whose numbers the JavaScript app measured off iA's
once — second-hand, and never checked against a frame.

**The pane is two columns and three grounds**, where Quill's is one column on one paper; **the
selected row is a 3 pt accent bar**, not a fill; and **the search field is at the foot**, with its
prompt at **`#7e7e7e`** where Quill's lands at `#BCBCBC` — the one-line bug that filed this ticket,
answered with a number.

All numbers are **device pixels** at backing scale 2 unless labelled pt. The region is the whole
window, `[0, 33, 1512, 949]` in logical points.
[The manifest](capture-2026-09-13-library.json) names all 20 frames, their originals, both hashes
and the state each was shot in, and it is built from the frames on disk rather than from a list, so
a frame shot and not written down cannot slip out of it. Every colour, width, pitch and inset
below is `rig/measure_library_379.py`'s output — the reader was rewritten for this, because the
first version of it read the light rows only and left the prompt, the separators and every dark
value to a scratch probe that nothing committed.

## Rig and method

Mono, System — Default, Normal text size, limit 64, Preview hidden, Focus, Typewriter, Syntax, Style
Check and Authors off, window `{0,33,1512,982}`, the Library shown.

The Library settings were taken **as found**, and they are what #379 asks for: Organizer with
Favorites, Smart Folders and Hashtags on; Files with Sort bar, Filter bar and Text excerpts on and
*Pin folders to top* off; Sort by Date Modified, Newest on Top; Navigation **Tree**.

**The fixture was added, not swapped in.** The Library holds `dev/shots/oracle/library/` without its
`manifest.json` — six files, a `Drafts` folder with two, `.archive` with one — beside the four
documents it already had, which are in every frame. Nothing already in the Library was moved,
renamed or removed; the copy is `rig/`'s to add and to take away again, and it was taken away when
the run ended — the Library holds exactly what it held before.

**L1 and the O1 drag have controls**; the rest each change what the pane holds, so a second frame of
the same pane is not a thing that exists. The pane's own width is read off its ground rather than
off the L1 pair: showing the pane reflows the page, so the pair differs nearly everywhere, and a box
drawn round that difference is the window.

## The pane

| | light | dark |
|---|---|---|
| Pane, total | **360 pt** — device columns 0 … 719 | the same |
| **Organizer**, the left column | **129.5 pt**, ground **`#eaebeb`** | ground **`#1a1c1b`** |
| **File List**, the right column | **230.5 pt**, ground **`#fcfcfc`** | ground **`#151515`** |
| The page beside it | `#f7f7f7` | `#1a1a1a` |
| The divider | **2 px**, a change of ground rather than a drawn rule | the same |

**Three grounds, not one.** On light the Organizer is *darker* than the paper and the File List
*lighter* than it; on dark the Organizer sits at the paper and the List goes darker still. Quill
draws the whole pane on the page's own paper.

**360 pt against Quill's 368.** Close, and not the same — and the 368 was second-hand.

**The pane drags, and the first two answers to that were wrong.** Dragged from the column the
pane's own ground gives way at — **360 pt** — it goes to **500 pt** and comes back to 360. The
second pass dragged from 368 pt, 8 pt past the edge; the third took the divider from the L1 pair's
difference, which is nearly the whole window because showing the pane reflows the page, and so
dragged from **1275 pt**, the middle of the text. Neither touched the divider, and both reported
"it does not drag" from a drag that never happened.

## A file row

| | measured |
|---|---|
| Pitch, excerpts on | **136 px = 68 pt** |
| Pitch, excerpts off | **64 px = 32 pt** |
| Name ink | **`#191919`** light, **`#b9b9b9`** dark |
| Date ink | **`#999999`** light, **`#757575`** dark |
| Excerpt ink | **the same as the date** |
| Separator | **`#ededed`** light, **`#212121`** dark; it runs x 340 … 691, **inset 74 px from the list's left edge and 19 px from its right** |
| Excerpt | **two lines**, the file's title run into its first words |

**The date and the excerpt share one grey**, and it is **darker** than the editor's own dim tier —
`#c6c4c2` light and `#707070` dark, VERDICTS 4.2.5 and 4.2.6. The `files` round-6 critic preferred
a **paler** pane to the editor's chrome; the oracle goes the other way, and the pane reads *more*
present than the chrome beside it rather than less.

**A folder row** carries a folder icon, its name, no date and no excerpt, and a `›` chevron at the
list's right edge.

## The selected row is a bar, not a fill

**A 6 px = 3 pt accent bar at the File List's left edge**, `#36bffa`, running the row's full
height — 128 px with excerpts on, 56 px without. There is no fill and no tint: the selected row's
ground is the list's own.

**A right-clicked row is outlined** with a focus ring instead, which is a third state again.

## The Organizer

Four sections, headed **`Locations`**, **`Favorites`**, **`Smart Folders`**, **`Hashtags`** in
**`#7f8080`** light and **`#393b3a`** dark. Under Locations the current one — `☁ iCloud` — stands on a rounded grey pill.

**An empty section carries prose, not nothing**: *Drag folders and files here for quick access*
under Favorites, *Write #tags to group files* under Hashtags. Smart Folders holds `Recents`.

## The search field

**It is at the foot of the File List**, under the rows rather than over them; it reads **`Filter`**
beside a magnifier; and `Edit > Find > Filter Library...` is what puts the caret in it.

**Its prompt is `#7e7e7e`** light and **`#757575`** dark, on a field whose ground is the list's own
(`#fcfcfc` / `#161616`) rather than a well of its own. Quill's is `#BCBCBC` — the palest thing
in the pane, 1.73:1 on the paper, "a field that looks switched off", which is the report that filed
#379. The oracle's prompt is **far darker** than Quill's, not paler, and darker than the pane's own
secondary grey.

## A folder opens in place

Under `Navigation: Tree` a click on `Drafts` turns its `›` into `⌄` and puts `closing.md` and
`opening.md` **indented** below it, in the same row shape, with the rest of the list following.
There is no stepping-in, so there is no way back to find.

## The Sort control's menu

The pill under the File List's title bar — `Sort by Date Modified ⌄` — opens ✓`Date Modified`,
`Date Created`, `Name`, `Extension` | `Oldest on Top`, ✓`Newest on Top` | `Pin Folders to Top` |
`Show Date ›`, ✓`Show Text Excerpts`, `Navigation ›`. **It is the Library pane of Settings, put on
the pane** — the same switches, reachable without leaving the window.

## A row's own menu, and where a Favorite is made

`Open in New Tab`, `Open in New Window` | `Get Info`, **`Favorite`**, `Duplicate`, `Rename`,
`Move to Trash` | `Show in Finder` | `Share ›`, `Export…`, `Print ›` | `Copy ›` | `New File`,
`New Folder` | `Sort By ›`, `View Options ›`.

**Favorites are made from the row**, not dragged — though the empty section's own prose says
dragging works too.


## A hovered row draws nothing

With the pointer resting on a row, **the row is unchanged** — the pane differs from its resting
frame only at the **search field**, which lifts to `#a1d5f5` with its prompt at `#777777`. Selection
is a bar; hover is nothing.

## What the search matches, and how it orders

The query `sea` leaves **three rows** of twelve: `The Lighthouse.txt`, `sea-storm.md` and
`harbour-lights.md`. Only one of those has `sea` in its **name**; the other two have it in their
**contents**. The query `the`, which no name holds, leaves **eleven** — so the field searches
**names and contents together**.

**The sort control changes with it.** `Sort by Date Modified ⌄` becomes **`Sort by Search
Relevance ⌄`** while a query stands, and goes back when it is cleared.

**A result row is an ordinary row.** Same icon, name, date and two-line excerpt, the excerpt still
the file's opening rather than a snippet around the match, and **no mark on the match at all**.

## What could not be done from this rig

**A file could not be made a Favorite**, so the Favorites section is only ever seen carrying its
empty-state prose, and **the Favorites row is unmeasured** — which is the half of L4 that #379 asked
for. A Favorite is made from a row's own context menu, and that menu is **not in the accessibility
tree**: with it open the process reports **zero menus**, `click menu item "Favorite" of …` fails
whatever it is asked of, `perform action "AXPress"` has nothing to press, and an arrow-key walk of
the open menu left the Organizer's ink unchanged to the pixel. The menu itself is on record
(`l4-row-menu`), so *where* a Favorite is made is answered; what the section looks like with one in
it is not.

## What this capture does not measure

Named in #379's *What the spec needs* and not answered here: the **title bar over the pane** (the
Library toggle, the ‹ › history, what the title shows while the pane is open); the **Sort and Filter
bar's own** height, type, grey and count wording; whether the pane has a **status line** at its foot
beyond the search field; the **row's left inset** from the pane's edge and a **folder row's pitch**
against a file row's; and **folders-at-top against interleaved** (the app's *Pin folders to top* is
off, and `Drafts` sorted to the top on its date alone). Each is readable off the frames this branch
commits; none is read here.

## What the spec still has to decide

Every value here is #246's own decision rather than a `docs/design.md` row — the Library is outside
[ADR 0015](../../../../docs/adr/0015-the-design-oracle-outranks-the-parity-oracle.md)'s reach — but it
is measured the same way, and a `files` state may name a `mac-native` opponent once the spec has
decided. `defaults.font` in `dev/shots/oracle/states.json` is `duo` and these frames are Mono, so such
a state names Mono and is re-shot first (#344's shape).

The three the spec asked for and now has: the **prompt's grey** (`#7e7e7e`, against Quill's
`#BCBCBC`), the **secondary grey** (`#999999`, darker than the editor's dim rather than paler), and
the **section head** (`#7f8080`, over a section that names itself in prose when empty). The one it
still owns is the biggest: Quill's pane is one column and the oracle's is two, and no measurement
decides whether to follow.
