# Specs and tickets

How `/to-spec` sizes a spec and `/to-tickets` sizes a ticket, so that each lands in one `/implement` session inside the smart zone (`CLAUDE.md` § Context in an `/implement` session has the arithmetic). Both skills read this file before publishing. A landed ticket's closing comment carries the `tools/context-report <N>` line (peak context, tool calls and each subagent's peak), and `/to-tickets` reads the last few before sizing, so the numbers here are checked against tickets rather than remembered.

## Specs

A `/to-spec` issue carries the `spec` label. Size it when it is created: a spec that fits one `/implement` session (the smart zone, including tests, `/code-review` and the Gate) keeps `ready-for-agent` and opens with one line saying so ("One-session spec: …" and the reason); one too large for a session loses `ready-for-agent` and is split by `/to-tickets` in a fresh session, its child tickets carrying the label and naming the spec under Parent.

Either way the spec closes on the owner's `hand test: pass`, by `docs/agents/gate.md` § Feature tier. The agent's last comment on a spec ends with a **Hand test** section: the install command, then the numbered "do X, see Y" steps written out in full (the `docs/agents/hand-tests.md` checklist merged with the spec's additions), so the owner tests from that comment alone. A failed step is commented on the spec and returns it to the agent.

## Tickets

`/to-tickets` sizes every ticket for one `/implement` session: about 70 tool calls (`CLAUDE.md` § Context in an `/implement` session has what the first landed tickets made). Each ticket carries a **Size** line estimating that from what drives calls, and a **Reading** line naming the spec sections and ADRs the session needs by heading and what it can skip (the parent spec whole, `legacy/`):

- Modules touched: one to three, by the module map; more is two tickets, or a prefactor ticket first.
- Source read to do the work: about 30 KB; a ticket that needs a crate read whole is two tickets.
- Source written: about 30 KB in total.
- Pieces named: at most one. A judge or bench run costs 15–20k of context; "Pieces: none" tickets are the cheap ones.
