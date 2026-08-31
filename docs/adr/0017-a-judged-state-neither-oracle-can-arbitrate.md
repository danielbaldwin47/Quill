# A judged state neither oracle can arbitrate is measured, not judged

A judged state whose subject neither the Design oracle nor the Parity oracle holds is answered by
arithmetic off ours' own pixels, and the Piece takes that answer exactly as it takes a critic's.
`shots/oracle/states.json` says so with `assert` where such a state would have carried `opponent`,
and `tools/assert-state.mjs` holds the rules. Decided from
[#147](https://github.com/danielbaldwin47/Quill/issues/147) and the round it could not win.

## Context

A blind round asks a fresh critic which of two images does a thing better. The question presumes
two images that both do the thing. Until now that presumption held: `legacy/` is the app the port
is porting, so it has every state the port has, and
[ADR 0015](0015-the-design-oracle-outranks-the-parity-oracle.md) added a second opponent for the
rows where iA Writer for Mac and `legacy/` disagree.

`caret/unfocused` is the first state where it fails, and it fails from both ends at once.

**The Design oracle has not got the state.** iA Writer for Mac draws no caret at all when its window
deactivates — `ref/ia/mac-native/VERDICTS.md` 0013.8, zero accent pixels across a 14-frame burst in
both themes. `docs/design.md` row *Caret on window deactivation* departs from it on purpose and
keeps the Parity oracle's 30 % ghost, because a tiling desktop shows the active window less plainly
than macOS does and the ghost says where the writer was. There is no capture to crop, and there
never will be.

**The Parity oracle has the state, but not the design.** `docs/design.md` row *Caret column* moved
the bar to the centre of the advance boundary, which is the Design oracle's own geometry
(VERDICTS 0013.1–0013.3) and which #147 built. `legacy/` still puts the bar's left edge there. So
the pair put in front of a critic is our ghost at the new column against `legacy/`'s ghost at the
column that row overruled — and in `progress/rounds/caret-r8.json` the critic picked `legacy/`, on
the five device pixels of paper between the preceding glyph and the bar that the centring gives up.
That reading is not wrong about the pixels. It is answering a question the Gate had already decided
elsewhere, and answering it the other way.

[ADR 0013](0013-caret-on-the-advance-boundary.md) § Consequences saw the shape of this coming — *"a
blind round the oracle can only win on the geometry, because the geometry is what it is being asked
to differ on. The owner's ask is the tiebreak the round cannot supply"* — and ADR 0015 then made
losses real again, but only for states that can take a Design oracle opponent. This one cannot, so
without a decision it stays a coin-flip on a settled question, forever, and
`docs/agents/gate.md`'s "a Piece once won is never lost" blocks every caret ticket behind it.

## Decision

**Such a state names `assert` instead of an opponent, and is measured.** The judge shoots it, runs
the named rule over the pixels, and pushes the answer into the round as a state won or lost, with
the measurement where a critic's reasoning would be. The Piece verdict is unchanged: it is ours only
when every state is, and `tools/gate oracle` passes an asserted state over exactly as it passes over
one carrying `opponent`.

**The bar for using it is that neither oracle holds the subject** — not that a round was lost, and
not that a critic was wrong. A state either oracle can be paired against stays a blind pair, whether
or not ours is currently winning it.

**`caret/unfocused` names `ghost`**, which shoots the state a second time with the window active and
holds two facts:

- the bar keeps its column, its width and its band — deactivating a window moves nothing, so a
  ghost in a different place is a defect;
- the ghost is the lit bar at `alpha` over the paper, `alpha` being the 0.3 the state names because
  `docs/design.md` is where it is decided.

The alpha is **solved out of the two shots** — the share of the way the ghost lies from the paper
towards the lit bar, per channel, dropping any channel where the two barely separate — rather than
compared against a colour written down in the tool. This repo has twice had a pinned hex outlive the
palette it was copied from, and `tools/keys-assert.mjs`'s own constants have drifted from
`quill-engine/src/theme.rs` today.

The third fact, that the blink has stopped, is deliberately not here: a still cannot hold it, and it
does not need to. `--deterministic` freezes the blink on, and `Caret::alpha` returning `GHOST` three
cycles into the quiet is asserted display-free in `quill/src/caret.rs`.

## Considered options

**Leave `caret/unfocused` a blind pair and take the loss each round.** It is the honest reading of
gate.md as written, and it blocks #147, #168, #169 and every later caret ticket on a verdict that
says nothing about whether ours got better. Rejected: the Gate exists to catch regressions the owner
would see, and this catches a decision the owner already made.

**Re-freeze the Parity oracle from a patched `legacy/`.** Then the pair would compare like with
like. Rejected outright: the Parity oracle is what the port is judged against precisely because
nobody edits it, and a patched oracle is not an oracle.

**Drop the state.** The ghost would then be held by nothing but a Hand test. Rejected: it is a real
behaviour that a regression could take away silently, which is what a judged state is for.

**Move it to `tools/gate keys`.** That is the existing arithmetic-off-the-glass condition and it
already runs for this Piece. Rejected on a mechanism: `keys` opens ours **Live**, deliberately,
because the caret machine is what a typed condition tests — so the lit reference frame would be
caught at whatever the blink was doing. The ghost needs `--deterministic`, which is the judge's
launch, not `keys`'.

## Consequences

**`tools/gate judge` now answers a state one of two ways**, and the round says which: a state
carrying `assert` records the rule and what it measured, and `refSource` names no opponent for it.
The round's `opponent` word gains `asserted` for a Piece with no opponent at all; a Piece holding an
asserted state beside a paired one is `mixed`, which the caret is from round 9 on.

**"A Piece once won is never lost" is unweakened.** `wonBefore` reads any round carrying an
`opponent` word, and an asserted round carries one, so a Piece that was won still blocks on a loss —
including a loss of an asserted state, which is a measurement and so is a real regression.

**The judged `caret` states are now three different kinds**: `caret` against the Design oracle,
`selection` against the Parity oracle until [#168](https://github.com/danielbaldwin47/Quill/issues/168)
gives it a crop, and `unfocused` asserted. That is the widest a Piece is expected to get.

**This narrows ADR 0015 § Consequences**, whose "rows a hand shows are `keys` assertions or
Hand-test steps" ends *"The deactivated caret stays a still, because the ghost stays and the
`unfocused` judged state already holds it."* The state does still hold it, and it is still a still —
what changes is that nobody is asked to prefer it to anything.

**A second asserted state is a design question, not a free extension.** The rules live in one file
with one map, and adding to it means arguing that neither oracle holds that subject either.
