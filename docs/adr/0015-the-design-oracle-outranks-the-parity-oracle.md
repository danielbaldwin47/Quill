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
across fifteen states; `VERDICTS.md` contradicts further claims in every section it covers and
measures what no frame could — the blink, the size ladder, the container. The port, meanwhile, had
reached exactly the Pieces those rows touch, with children of #38–#40 open on premises the captures
overturn.

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

**The reach is the writing surface**: type, page, markup rendering, caret and selection, the two
palettes, Focus and Typewriter. Chrome, the Library, Preview, file handling and every setting's
*switch* stay Quill's own; the Preview captures (`mac-native-16-*`) wait for Preview's spec.

**The Gate gains a per-state opponent.** `shots/oracle/states.json` may name a `mac-native` crop for
a state; ours is shot in Mono at the Design oracle's default size for that state so cells compare.
States with no such entry judge against `legacy/` as before, and the latency verdict is unchanged.
Once `legacy/` is deleted the opponent for every state is a `mac-native` crop, replacing the
marketing-frame `reference` crop `docs/agents/gate.md` § Ticket tier and `states.json` name today.
The tooling — the per-state key, the crop, the Mono shot, and those two edits — is one Gate ticket
before the first re-judge.

**Rows a hand shows are `keys` assertions or Hand-test steps**: blink cadence and typing
suppression. The deactivated caret stays a still, because the ghost stays and the `unfocused` judged
state already holds it.

**ADR 0006's near tier is superseded.** Its § Consequences paragraph "The two Quill refinements
stay" loses its first refinement — "the neighbouring sentences sit one shade above the far dim (the
'near' tier)" — and Focus has one dim tier per theme. The second refinement, the edge band, and the
independence of Focus and Typewriter stand.

**The open children of #38–#40 that touch a row are amended before they run**: #110, #113, #114,
#115, #126, #129, #147. Amended means the triage adds a comment naming the row and the
`VERDICTS.md` evidence the ticket now builds to, and its Reading line gains `docs/design.md`.
#111, #148 and #150 touch no row and run as written.

**`CONTEXT.md` names both oracles.** *Design oracle* and *Parity oracle* are the two terms.
