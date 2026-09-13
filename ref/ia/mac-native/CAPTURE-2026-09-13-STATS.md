# Mac captures on the original rig — #381, the bar at the foot of the window

The counts, and the bar they stand on, shot on the **built-in screen of the original 14-inch M1
MacBook Pro**. No committed capture showed either: `NOTES.md` § The rig never turned the bar on,
neither `NOTES.md` nor `VERDICTS.md` contained the word, and the only picture of it was a 2023
marketing still, which is not evidence.

**The first thing the frames say is that iA has no stats bar.** What stands at the foot of the
window is the **Toolbar** — a format bar of twelve labels, `Body`, `Heading 1 ⌃`, `List ⌃`,
`Blockquote`, `Bold`, `Italic`, `Strikethrough`, `Link`, `Wikilink`, `Footnote`, `Table`, `TOC` —
with the counts as a **thirteenth group at its right end**, a popup rather than a label.

**And on this machine it was not there at all.** `View > Toolbar` was found on **Fade In/Out** when
the rig first read it, and under that setting, with the pointer away from the foot of the window,
the bar is gone — paper to the window's edge, resting and typing alike. Whether Fade In/Out is what
the app *ships* on or what this Mac had been left on, one machine cannot say, and the rig changed
the setting itself before the states were shot.

So the question #381 asked — what does iA's stats bar look like — has an answer the chrome spec has
to decide about rather than copy: **iA does not keep one.**

All numbers are **device pixels** at backing scale 2 unless labelled pt. The region is the whole
window, `[0, 33, 1512, 949]` in logical points.
[The manifest](capture-2026-09-13-stats.json) names all 19 frames, their originals, both hashes and
the Toolbar setting each was shot under; `rig/measure_stats_381.py` re-reads every number below off
them.

## Rig and method

Mono, System — Default, Normal text size, limit 64, Library and Preview hidden, Focus, Syntax, Style
Check and Authors off, window `{0,33,1512,982}`, `ref/sample.md`.

**Every state was shot with the bar and again with `View > Toolbar > Hide`** and nothing else
changed. The control is what makes the gutter measurable at all: with one frame there is no way to
say where the page's last row would have fallen if the bar were not under it.

**`Stats Only` is not shot, and could not be.** `View > Toolbar` offers a second pair — `Default`
and `Stats Only`, the bar reduced to its counts, which is the nearest thing the app has to Quill's
stats bar. Four ways of pressing it — `click menu item` with the menu closed, the same with the menu
walked open, `perform action "AXPress"`, and an arrow-key walk of the open menu — **all report
success, leave `Default` checked, and change no pixel of the bar**. #354's Style Check lists at
least moved the frame when clicked; this one does nothing to read. Every state here therefore wears
the `Default` bar, which is what the app ships and what #381 asked for, and `Stats Only` is the one
thing the ticket named that is still unshot.

## The bar

| | light | dark |
|---|---|---|
| Height, rule to the window's foot | **80 px = 40 pt** — the rule at 1818, the window's last row at 1897 | the same |
| Ground | **the paper itself** — `#f7f7f7` | **`#1a1a1a`** |
| Rule above it | **2 px = 1 pt**, `#dbdbdb` | `#2e2e2e` |
| Counts' ink at rest | **`#191919`** — the body ink | **`#cccccc`** — the body ink |
| Counts on hover | **the accent**, `#36bffa` read off the glyph | not shot |
| Labels | **twelve**, with the counts at **x 2825 … 2973** | the same |

The reader prints the bar's **difference band**, 69 px on light, which is where the two frames stop differing rather than where the bar stops: its last rows are paper either way. The bar's box is the rule to the window's foot, and both edges are in the data — `rule_above` puts the rule at rows 1818–1819 and the frame's last row is 1897.

**Counting the bar's ink.** `measure_stats_381.py` finds **fourteen** groups of ink in the band, not thirteen: a pop-up's chevron separates from its own label by more than the 30 px the reader joins across, so `Heading 1 ⌃` reads as two. The bar carries **twelve labels and the counts**.

**The bar's ground is the paper.** Not a tint, not a translucency over the text: the median of the
band is `#f7f7f7` and `#1a1a1a` exactly, the § 4.2 papers. Only the hairline separates it from the
page.

**The counts are body ink, not a quiet tier.** `#191919` on light and `#cccccc` on dark — the same
values the running text carries (4.2.3, 4.2.4). Where Quill's chrome reads as a dimmer register,
iA's bar reads at full strength and lets the fade carry the quietness instead.

**Hover lifts the counts to the accent.** The pointer resting on them turns the label blue rather
than darkening it — the opposite direction from the Parity oracle, which lifts a dimmed figure to
body ink.

## The gutter above it — there is none

| state | last row's ink | the rule | between them |
|---|---:|---:|---:|
| light, scroll 0, the bar showing | 1817 | 1818 | **1 px** |
| light, scroll 0, the bar hidden | 1817 | — | — |

**The page runs to the rule.** At the document top the last visible row's ink ends **one device
pixel** above the hairline, and the control shows the same row in the same place with no bar under
it — so the bar does not push the text up, does not mask it, and leaves no padding. The text simply
scrolls under it.

That is the measurement the chrome spec wanted: Quill keeps **10 … 19 px** of paper there and the
Parity oracle **31 … 34**. The Design oracle keeps **one**.

## The end of a draft

| | measured |
|---|---:|
| Last row's ink, scrolled to the end | row 898 |
| The rule | row 1818 |
| Air between them | **920 px = 460 pt** |

And the zero: **C3**, an empty document, carries `0 Words` in the same `#191919` as a full one — the
counts do not go quiet when there is nothing to count, and the figure is body ink whatever it
reads. Quill's own `0` is `#4A4A4A`, the heaviest ink on that screen; iA's is the body's, under a
bar that is not there at all unless asked for.

**460 pt of air below the last row**, in a 949 pt window: **48.5 % of the view**. Quill's
`--page-bottom` is 30 vh and the Parity oracle's Typewriter counterpart 62 vh; the oracle's own
figure is between them.

**The rule stands whether or not anything continues under it.** It is at rows 1818–1819 in the
document-top frame and in the document-end frame alike, so it is the bar's own edge rather than a
"more below" signal. (In the dark document-end frame the difference band starts at 1831 because the
bar's own rows match the paper there; the rule is read off the light pair, where it separates.)

## The counts' menu

A click on `188 Words ⌃` opens a popup of **ten counts, each showing its value**, with a single ✓
against the one displayed:

`941 Characters` · `752 Without Spaces` · **✓ 188 Words** · `16 Sentences` ·
`00:00:56 Reading Time` · `00:01:26 Speaking Time` · `0 of 0 Tasks` · `0% Human` · `0% AI` ·
`0% Reference`

**It is a choice of one, not a set of toggles**: the bar shows a single count and the menu picks
which. The popup is taller than the screen leaves room for and carries a chevron; a second frame
with it scrolled holds the last rows.

## A selection standing

With the first sentence held the counts read **`13 Words`** — the selection's own count, not the
document's 188 — and the figure carries the **selection fill** behind it, `#cbedf7`, which is how a
reader knows it is counting the selection rather than the page.

## What the bar does while the keys move

**Under the app's own setting it is not there.** `Fade In/Out` with the pointer away: no labels at
rest, no labels mid-burst. The bar does not dim while typing, because there is no bar.

**Pinned with `Always Show` it does not dim either.** A frame taken while the keys move reads the
same groups and the same `#191919` counts as at rest. The Parity oracle dims its bar to
0.38 while typing; iA's does not move.

**The counts update after a pause of about 1.2 s.** Typing `wordcountprobe` took 0.144 s; the
counts' own strip, sampled through Quartz at 10 Hz, first changed **1.176 s after the last key
landed**. Every later sample of the four-second window differs from the one before the keys, which says the count had moved and stayed moved; it does **not** say the bar went on settling, and the window ended before anything could. They do not tick per keystroke.

## Typewriter

**C7 was re-shot as a pair**, because the question is where the last row rests against the bar with
the caret at the window's centre, and one frame cannot say where that row would have fallen with
no bar under it. **It rests exactly where it does at rest: one device pixel above the rule.** The
bar itself is unchanged — same groups, same `#cccccc` counts. Typewriter moves the caret line and
nothing at the foot of the window.

The Parity oracle's Typewriter counterpart is `--page-bottom: 62vh`; the oracle keeps no gutter
under Typewriter any more than it does at rest.

## What this leaves the chrome spec

Everything above is the chrome spec's own decision rather than a `docs/design.md` row — the chrome
is outside [ADR 0015](../../../docs/adr/0015-the-design-oracle-outranks-the-parity-oracle.md)'s reach —
but it is measured the same way, and a `chrome` state may name a `mac-native` opponent once the
spec has decided. `defaults.font` in `shots/oracle/states.json` is `duo` and these frames are Mono,
so a state that names one names Mono and is re-shot first (#344's shape).

The three numbers the spec asked for and now has: the bar is **40 pt** on the paper under a **1 pt**
rule; the gutter above it is **1 px**; and the end of a draft keeps **460 pt** of air. The decision
the frames do not make for it is the first one: whether Quill keeps a stats bar at all, when the
Design oracle does not.
