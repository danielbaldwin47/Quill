# Quill

A long-form writing environment: a transparent `<textarea>` over a `<div id="mirror">` that renders the same text with markup styling in identical font metrics. No framework, no build step. `README.md` has the results and how to run; `BRIEF.md` the architecture, per-piece file ownership and tool flags; `NOTES.md` the log of cross-piece changes.

## Repo map

- `app/` — the editor as served. `index.html` loads `css/` and `js/` in fixed order; `js/core.js` is the engine every other module plugs into (`Writer.*`), and `js/{markup,caret,focus,theme,chrome,files}.js` each own one piece with a matching stylesheet in `css/`. `fonts/` holds iA Writer Duo/Quattro/Mono (OFL).
- `bin/quill` — launcher: starts `tools/serve.mjs` if the port isn't already serving Quill, opens Chromium in app mode; `--measure` runs the latency bench.
- `tools/` — `serve.mjs` (static server), `shoot.mjs`/`crop.mjs`/`blind.mjs`/`thumb.mjs` (screenshots and blind A/B pairs), `latency.mjs` (keystroke and cold-start bench), `smoke.mjs` (integration check), `progress.mjs` (builds `progress/index.html`), `gauntlet.workflow.js` (builder/critic loop per piece).
- `ref/ia/` — the iA Writer screenshots, fonts and templates every comparison is judged against; `ref/sample.md` is the shared test passage.
- `shots/` and `progress/` — judging evidence: per-piece screenshots, blind pairs, round verdicts, latency JSON and the report.
- `PKGBUILD` + `packaging/` — Arch package; `makepkg -f` then `pacman -U`.
- `docs/agents/` — issue tracker, triage labels and domain-doc rules for the engineering skills (below).

Hard rule in `app/`: no per-token style may change glyph advance width, or the mirror and textarea drift apart (bold/italic are safe; iA fonts share widths across weights).

The native Rust rewrite is specified in `docs/architecture.md`; read it before any Rust, packaging or repo-migration work. Its Gate is `docs/agents/gate.md`.

## Gate

Before landing native work, closing a ticket, or closing a feature: `docs/agents/gate.md` names the tier, its commands, the latency budget, the blind-judging opponent and the Hand test checklists.

An `/implement` session whose ticket has no Hand test (Ticket tier only) lands its own work: once the Gate is green and `/code-review` is done, it merges its PR (`gh pr merge --merge`) and closes the ticket. Only a Hand test hands the close to the owner.

## Agent docs

Edit `CLAUDE.md`, `CONTEXT.md`, `docs/agents/*.md`, ADRs and any other document an agent reads through `/writing-for-agents`.

## Agent skills

### Issue tracker

Issues and specs live in this repo's GitHub Issues (`danielbaldwin47/Quill`, private), driven through the `gh` CLI. See `docs/agents/issue-tracker.md`.

### Specs

A `/to-spec` issue carries the `spec` label. Size it when it is created: a spec that fits one `/implement` session (the smart zone, about 120k tokens, including tests, `/code-review` and the Gate) keeps `ready-for-agent` and opens with one line saying so ("One-session spec: …" and the reason); one too large for a session loses `ready-for-agent` and is split by `/to-tickets` in a fresh session, its child tickets carrying the label and naming the spec under Parent. Either way the spec closes on the owner's `hand test: pass`. The agent's last comment on a spec ends with a **Hand test** section: the install command, then the numbered "do X, see Y" steps written out in full (the `docs/agents/gate.md` checklist merged with the spec's additions), so the owner tests from that comment alone. The owner's `hand test: pass` with the `pacman -Q quill` output closes the spec; a failed step is commented on the spec and returns it to the agent.

### Triage labels

Default vocabulary: `needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`. See `docs/agents/triage-labels.md`.

### Domain docs

Single-context: `CONTEXT.md` and `docs/adr/` at the repo root, created lazily by `/domain-modeling`. See `docs/agents/domain.md`.
