# Specs and tickets

How `/to-spec` sizes a spec and `/to-tickets` sizes a ticket, so that each lands in one `/implement` session inside the smart zone (`CLAUDE.md` § Context in an `/implement` session has the arithmetic). Both skills read this file before publishing. A landed ticket's closing comment carries the `tools/context-report <N>` line (peak context, tool calls and each subagent's peak), and `/to-tickets` runs the report on the last few landed tickets before sizing, so the numbers here are checked against tickets rather than remembered.

## Specs

A `/to-spec` issue carries the `spec` label. Size it when it is created: a spec that fits one `/implement` session (the smart zone, including tests, `/code-review` and the Gate) keeps `ready-for-agent` and opens with one line saying so ("One-session spec: …" and the reason); one too large for a session loses `ready-for-agent` and is split by `/to-tickets` in a fresh session, its child tickets carrying the label and naming the spec under Parent. No Piece spec has yet fit one session: #36 ran as one and peaked at 334k over 211 calls.

Either way the spec closes on the owner's `hand test: pass`, by `docs/agents/gate.md` § Feature tier. The agent's last comment on a spec ends with a **Hand test** section: the install command, then the numbered "do X, see Y" steps written out in full (the `docs/agents/hand-tests.md` checklist merged with the spec's additions), so the owner tests from that comment alone. The last child ticket names itself as the one that writes it. A failed step is commented on the spec and returns it to the agent.

## Tickets

`/to-tickets` sizes every ticket for one `/implement` session. Each ticket carries a **Size** line and a **Reading** line naming the spec sections and ADRs the session needs by heading — and, for a ticket touching the writing surface, the `docs/design.md` row it builds to — and what it can skip (the parent spec whole, `legacy/`).

**The Size line quotes a landed shape, not a constant.** A ticket's estimate has run about 3× over: the Markup children sized at 50–65 tool calls landed at 122–247 calls and 165k–302k peak (#86–#91, #102); a judge-only ticket is a session by itself (#89: 138 calls, 179k); a ticket that must land a bench run is more than one (#65: 290 calls, #91: 247); and only tickets with two modules and no Gate run have landed inside the 120k zone (#72, #73, #79: 52–61 calls, 90–111k). So the line gives the rubric estimate, then the nearest landed ticket and its `context-report` numbers — "about 45 tool calls by the rubric; the landed shape is #102 (127 calls, 169k peak)" — and a ticket whose nearest shape is over 200 calls is two tickets.

What drives calls, and the bounds a ticket inside the zone has kept:

- Modules touched: one or two, by the module map; three is the ceiling, and more is two tickets, or a prefactor ticket first.
- Source read to do the work: about 30 KB, large files by the orientation fork's ranges; a ticket that needs a 50 KB module read whole is two tickets, the engine-only half first.
- Source written: about 30 KB in total.
- Pieces named: at most one, and the judge or bench run is most of what the ticket does — pair it with small changes only. A judge run costs 15–20k of context and a Piece takes two or three rounds; a bench series is one launch per regime per attempt. "Pieces: none" tickets are the cheap ones. A ticket that touches the keystroke path without naming latency still runs the headline regime once, as a measurement, and says so.
- Tests: `cargo test` runs with no display (`docs/agents/gate.md` § Commit tier), so an acceptance criterion asserts a pure function, a model or a file, and reads the widget from a `--deterministic` shot; a criterion that needs a window and no keystroke is a Hand test step; one that needs a keystroke is a keys assertion (`docs/agents/gate.md` § Ticket tier), because a still cannot show what only happens under a hand.
- Files shared with a sibling: two `ready-for-agent` tickets that edit the same module land in sequence, the second Blocked-by the first — #162 and #164 each spent twelve to fifteen calls merging a sibling that landed on `typography.rs` or `oracle.mjs` mid-session.
- Gate tooling the ticket's judge needs (an unfrozen oracle, a state key no tool serves, a `mac-native` opponent no tool serves yet — `tools/gate judge <piece>` exits 3 for each) is its own ticket, before the first judged child.
