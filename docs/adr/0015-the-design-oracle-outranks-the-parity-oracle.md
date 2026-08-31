# The Design oracle outranks the Parity oracle where the design doc says so

iA Writer for Mac running natively is the **Design oracle**, evidenced by `ref/ia/mac-native/`
(captured and measured under #154, PRs #155 and #156). `legacy/` stays the **Parity oracle**: what
the port ports, and the opponent the Gate judges against. Where the two disagree on the writing
surface, `docs/design.md` decides per behaviour, and its decision may be neither side's. A judged
state that follows such a decision names a `mac-native` capture as its opponent rather than the
Parity oracle's frozen shot. Decided 2026-08-30 with the owner, closing the question ADR 0014 left
open.

## What changed the answer

`ref/ia/REFERENCE.md` was built from marketing frames, and three of its readings were wrong on the
running app: a right side bearing read as a caret nudge, a style-check marker read as an inset
selection bar, and iPad touch handles read as a Mac selection (ADRs 0013, 0014). #154 ran the app
across fifteen states and the verdict table contradicted nine more claims and measured what no frame
could — the blink, the size ladder, the container. The port, meanwhile, had reached exactly the
Pieces those rows touch, with eight child tickets open on premises the captures overturn.

Until now nothing in the repo distinguished a Design-oracle value from a Parity-oracle one: every
constant's comment cites `caret.js` or `theme.css`, and the Gate's opponent is `legacy/` until it is
deleted, so a deliberate departure from `legacy/` could never win a blind round. A Piece once won is
never lost, which turns that into a blocked ticket.

## Considered options

**`legacy/` decides everything until the port is done; iA afterwards.** Cheapest for the port, and
builds the caret, the palette, the tiers and the container twice.

**iA decides everything.** Deletes `legacy/` now and judges every state against a `mac-native` crop.
Loses the latency bar, which only `legacy/` provides, and forces chrome and settings — a Linux GTK
app's own — toward macOS.

**A row per behaviour, the Gate told which opponent a state has.** Chosen. The design doc is the one
place the "what is Quill's caret meant to do" question has an answer; the Gate keeps its shape and
gains a per-state opponent; the reach is bounded to the writing surface.

## Consequences

**`docs/design.md` is the source of truth for the writing surface's intent.** A ticket touching a
row reads the row; an engine constant that follows a row cites the capture that measured it rather
than `legacy/`'s CSS.

**The Gate gains a per-state opponent.** `shots/oracle/states.json` may name a `mac-native` crop for
a state; ours is shot in Mono at the Design oracle's default size for that state so cells compare.
The tooling is its own ticket before the first re-judge. States with no such entry judge against
`legacy/` as before, and the latency verdict is unchanged.

**Rows a hand shows are not judged by a still.** Blink cadence, typing suppression and the
deactivated caret are `keys` assertions or Hand-test steps.

**Open children of #38–#40 that touch a row are amended before they run**, so nothing is built to
`legacy/`'s number and again to the oracle's.

**`CONTEXT.md` names both oracles.** *Design oracle* and *Parity oracle* are the two terms; "the
reference" and "the vision" are retired.

**The reach stops at the writing surface.** Chrome, Library, Preview, file handling and every
setting's switch are Quill's; the Preview captures wait for Preview's spec.
