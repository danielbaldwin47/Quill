# Quill

A long-form writing environment for Linux: a native GTK4 app in Rust, ported Piece by Piece from the JavaScript app that won its blind gauntlet against iA Writer. That app is now `legacy/`, kept as the **Parity oracle** every native Piece is judged against until the last one is won. `docs/architecture.md` specifies the native app: read it before any Rust, packaging or repo work. Its Gate is `docs/agents/gate.md`, and `CONTEXT.md` holds the vocabulary both use.

## Repo map

- `quill/` — the app crate, the only crate that may see `gtk` ([ADR 0008](docs/adr/0008-engine-crate-without-gtk.md)). `main.rs` reads the command line, loads the Faces and opens the settings before any window, because every flag is applied before the first frame.
- `quill-engine/` — the display-free half: text model, Markdown, Annotators, Library, settings, Templates, rendering. It builds and tests with no display attached, and `tests/boundary.rs` fails `cargo test` if `gtk` slips in.
- `fonts/` — the six Quill Faces and `OFL.txt` (SIL OFL 1.1), loaded privately at startup; `tools/fontbuild.py` builds them from the iA originals.
- `tools/` — the Gate's command and its helpers. `tools/gate check` is the Commit tier in one line, `tools/gate oracle <piece>` freezes the Parity oracle from `legacy/`, and `tools/gate judge <piece>` shoots ours at that Piece's judged states, pairs each blind against the oracle, runs a critic per pair and records the round (`tools/gate --help`); `tools/gate-fixture/selftest` keeps the first honest against a crate outside the workspace whose defects it must go red on, `oracle-selftest.mjs` keeps the second honest against the judged states with no browser, and `judge-selftest.mjs` keeps the third honest with no window and no critic. The rest: `harness.mjs` (the headless output, window rules, launch, capture and teardown that a judged shot and a bench both run on), `critic.md` (the blind critic's prompt, and the only thing a critic is ever told), `blind.mjs`/`thumb.mjs` (blind A/B pairs, under `shots/blind/<piece>/<state>/` with their keys outside the repo at `$XDG_STATE_HOME/quill/blind-keys/`), `progress.mjs` (builds `progress/index.html`), `regimes.mjs` (the twelve latency regimes and the typist, typed by both benches), `fingerprint.mjs` (which build of `legacy/app` a shot or a number is of, hashed the one way for both), `uinput-keys.py` (real keys through `/dev/uinput`), `idle-check.py` (is anybody at this machine), `fontbuild.py`/`fontgrid.py` (the Faces), `mkdoc.mjs` (builds the 10k-word bench corpus). Two here are the legacy app's rather than the Gate's: `gauntlet.workflow.js` (its builder/critic loop) and `mirror-metrics.mjs` (its mirror/textarea drift check). Three are a ticket's rather than the Gate's: `tools/ticket <N>` (the ticket, its comments and the spec it names under Parent, fetched once to one file), `context-report <N>` (what a landed ticket's `/implement` session cost, in the one line its closing comment carries) and `context-report-selftest` (which keeps that honest against a written-out transcript). `npm i` at the root once, for the three that drive a browser.
- `legacy/` — the JavaScript app as it won, and the Parity oracle: `legacy/bin/quill` opens it from the checkout, `legacy/tools/` shoots and benches it (`npm i` inside `legacy/` too), `legacy/BRIEF.md` and `legacy/NOTES.md` describe it. It is ISC, under its own `legacy/LICENSE`.
- `ref/ia/` — the iA Writer screenshots, fonts and templates every comparison is judged against; `ref/sample.md` is the shared test passage.
- `shots/` and `progress/` — judging evidence: per-Piece screenshots, round verdicts, latency JSON and the report. Blind pairs are not among it: `shots/blind/` is two copies of shots already committed on either side, so it is written for the critic and ignored by git. `shots/oracle/` holds the judged states (`states.json`) and the Parity oracle frozen at them by `tools/gate oracle <piece>`: one committed shot per state, beside the fingerprint of the `legacy/` build and the shooter that produced it, re-shot only when that fingerprint or the states move.
- `PKGBUILD` + `packaging/` — Arch package of the native binary; `README.md` § Build, install and run has the build and install commands and the traps they avoid. The `.desktop` file and the icon are named for the application id, `io.github.danielbaldwin47.Quill`.
- `docs/` — `architecture.md` (the native spec), `shortcuts.md` (the one shortcut table every menu, the Palette and the shortcuts window read), `adr/` (decisions), `agents/` (the Gate, the ported Pieces' Hand test checklists, issue tracker, triage labels and domain-doc rules for the skills below).

Licences: GPL-3.0-or-later at the root (`LICENSE`), ISC in `legacy/`, OFL-1.1 for `fonts/`, and iA's own terms for `ref/ia/`.

Hard rule in `legacy/app/`: no per-token style may change glyph advance width, or the mirror and textarea drift apart (bold/italic are safe; iA fonts share widths across weights).

Hard rule for any test window (GTK, browser, bench): it opens on a virtual output — `hyprctl output create headless`, what `legacy/bin/quill --measure` does by default — or, when it must be on the real monitor, on workspace 5 with `[workspace 5 silent]`. Workspace 1 is the user's live workspace. The Hyprland 0.56 commands are in `legacy/BRIEF.md` § Headed windows; without `hyprctl`, run headless.

## Rust

The `rust-analyzer-lsp` plugin is installed, so for any Rust in either crate the LSP tool answers definition, references, hover, symbols and call hierarchy. Reach for it before a `grep` for a symbol or a `cat` of a file to find one.

## Gate

Before landing native work, closing a ticket, or closing a feature: `docs/agents/gate.md` names the tier, its commands, the latency budget, the blind-judging opponent and the Feature tier's Hand test; `docs/agents/hand-tests.md` holds the ported Pieces' checklists.

An `/implement` session whose ticket has no Hand test (Ticket tier only) lands its own work: once the Gate is green and `/code-review` is done, it merges its PR (`gh pr merge --merge`) and closes the ticket. Only a Hand test hands the close to the owner.

## Context in an `/implement` session

The smart zone is about 120k tokens. A session is near 60k once this file, the ticket and the docs it names are in context, and every tool call then adds its result plus about 0.4k of reasoning that stays for the rest of the session, so the zone is held by making fewer, smaller calls (the first eight landed tickets made 120–230 and ran 190k–280k).

- **Orientation is delegated.** Before the first edit, an Explore agent maps the area and returns `file:line` ranges; this context reads those ranges. Where-is-what questions go to the module map — every module opens with a `//!` line, so `grep -rn -m1 '^//!' --include='*.rs' quill quill-engine` is both crates on one screen — or to the LSP tool (§ Rust), which answers in lines where a `cat` costs the file.
- **The Gate is one call.** `tools/gate check` is the whole Commit tier in one result. While iterating: `cargo check -q --message-format=short`, `cargo test <name>`, and listings through `head` or `grep`. `tools/gate judge` and `bench` are read for their summary lines; the shots are the critic's to look at.
- **The ticket is fetched once**, with its parent spec, to a file under the job's tmp directory by `tools/ticket <N>`, and later questions are answered from that file by `sed -n` range.
- **Docs by section.** This file is already in context. `docs/agents/gate.md` for the tier the ticket names, `docs/architecture.md` for the sections the ticket cites, ADRs by number; `docs/agents/hand-tests.md` is `/to-spec`'s reading.
- **One tool per file.** A file the harness has seen through Read, Write or Edit and then changed through Bash — `sed -i`, a heredoc, `cargo fmt` — comes back into context as a diff snippet (one session paid 60 KB this way). Files opened with Bash stay with Bash; files touched with Write or Edit change through Edit, written in rustfmt's shape so `cargo fmt` changes nothing.

## Agent docs

Edit `CLAUDE.md`, `CONTEXT.md`, `docs/agents/*.md`, ADRs and any other document an agent reads through `/writing-for-agents`.

## Agent skills

### Issue tracker

Issues and specs live in this repo's GitHub Issues (`danielbaldwin47/Quill`, private), driven through the `gh` CLI. See `docs/agents/issue-tracker.md`.

### Specs

A `/to-spec` issue carries the `spec` label. Size it when it is created: a spec that fits one `/implement` session (the smart zone, about 120k tokens, including tests, `/code-review` and the Gate) keeps `ready-for-agent` and opens with one line saying so ("One-session spec: …" and the reason); one too large for a session loses `ready-for-agent` and is split by `/to-tickets` in a fresh session, its child tickets carrying the label and naming the spec under Parent. Either way the spec closes on the owner's `hand test: pass`. The agent's last comment on a spec ends with a **Hand test** section: the install command, then the numbered "do X, see Y" steps written out in full (the `docs/agents/hand-tests.md` checklist merged with the spec's additions), so the owner tests from that comment alone. The owner's `hand test: pass` with the `pacman -Q quill` output closes the spec; a failed step is commented on the spec and returns it to the agent.

### Tickets

`/to-tickets` sizes every ticket for one `/implement` session inside the smart zone (§ Context in an `/implement` session): about 70 tool calls, where the first eight landed tickets ran 120–230. Each ticket carries a **Size** line estimating that from what drives calls, and a **Reading** line naming the spec sections and ADRs the session needs by heading and what it can skip (the parent spec whole, `legacy/`):

- Modules touched: one to three, by the module map; more is two tickets, or a prefactor ticket first.
- Source read to do the work: about 30 KB; a ticket that needs a crate read whole is two tickets.
- Source written: about 30 KB in total.
- Pieces named: at most one. A judge or bench run costs 15–20k of context; "Pieces: none" tickets are the cheap ones.

A landed ticket's closing comment carries the `tools/context-report <N>` line (peak context, tool calls and each subagent's peak), and `/to-tickets` reads the last few before sizing, so the numbers above are checked against tickets rather than remembered.

### Triage labels

Default vocabulary: `needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`. See `docs/agents/triage-labels.md`.

### Domain docs

Single-context: `CONTEXT.md` and `docs/adr/` at the repo root, created lazily by `/domain-modeling`. See `docs/agents/domain.md`.

### Code review

`/code-review`, wherever this repo's docs say it, is `/mattpocock-skills:code-review`, invoked by that full name: a second skill named `code-review` ships with Claude Code, and the full name picks the right one.
