# Quill

A long-form writing environment for Linux: a native GTK4 app in Rust, ported Piece by Piece from the JavaScript app that won its blind gauntlet against iA Writer. That app is now `legacy/`, kept as the **Parity oracle** every native Piece is judged against until the last one is won, except the states `docs/design.md` hands to the **Design oracle** — iA Writer for Mac as measured in `ref/ia/mac-native/` (ADR 0015). `docs/architecture.md` specifies the native app: an `/implement` session reads the sections its ticket cites (§ Context in an `/implement` session); any other Rust, packaging or repo work reads it first. Its Gate is `docs/agents/gate.md`, and `CONTEXT.md` holds the vocabulary both use.

## Repo map

- `quill/` — the app crate, the only crate that may see `gtk` ([ADR 0008](docs/adr/0008-engine-crate-without-gtk.md)).
- `quill-engine/` — the display-free half: text model, Markdown, Annotators, Library, settings, Templates, rendering. It builds and tests with no display attached, and `tests/boundary.rs` fails `cargo test` if `gtk` slips in.
- `fonts/` — the six Quill Faces and `OFL.txt` (SIL OFL 1.1), loaded privately at startup; `tools/fontbuild.py` builds them from the iA originals.
- `tools/` — the Gate's command and its helpers: `tools/gate --help` names its five subcommands and what each ends in, `docs/agents/gate.md` says which tier requires which, and a selftest beside each subcommand keeps it honest. `npm i` at the root once, for the ones that drive a browser. Two here are the legacy app's rather than the Gate's: `gauntlet.workflow.js` (its builder/critic loop) and `mirror-metrics.mjs` (its mirror/textarea drift check). Three are a ticket's: `tools/ticket <N>` (the ticket, its comments and the spec it names under Parent, fetched once to one file), `context-report <N>` (what a landed ticket's `/implement` session cost, in the one line its closing comment carries) and `context-report-selftest` (which keeps that honest against a written-out transcript).
- `legacy/` — the JavaScript app as it won, and the Parity oracle: `legacy/bin/quill` opens it from the checkout, `legacy/tools/` shoots and benches it (`npm i` inside `legacy/` too), `legacy/BRIEF.md` and `legacy/NOTES.md` describe it. It is ISC, under its own `legacy/LICENSE`.
- `ref/ia/` — iA's fonts, templates and marketing stills; `ref/ia/mac-native/` is the Design oracle as measured (`VERDICTS.md`, `NOTES.md`, the captures under `ref/ia/shots/mac-native/`), and it outranks every still. `ref/sample.md` is the shared test passage.
- `shots/` and `progress/` — judging evidence: per-Piece screenshots, round verdicts, latency JSON and the report. `shots/oracle/` is the judged states (`states.json`) and the Parity oracle frozen at them; `shots/blind/` is the critic's copy of shots committed elsewhere, ignored by git.
- `PKGBUILD` + `packaging/` — Arch package of the native binary; `README.md` § Build, install and run the native app has the build and install commands and the traps they avoid. The `.desktop` file and the icon are named for the application id, `io.github.danielbaldwin47.Quill`.
- `docs/` — `architecture.md` (the native spec), `design.md` (what the writing surface is meant to do where the Design oracle and the Parity oracle disagree — read its row before a caret, selection, palette, focus, markup or type ticket), `shortcuts.md` (the one shortcut table every menu, the Palette and the shortcuts window read), `adr/` (decisions), `agents/` (the docs that § Gate and § Agent skills point at).

Licences: GPL-3.0-or-later at the root (`LICENSE`), ISC in `legacy/`, OFL-1.1 for `fonts/`, and iA's own terms for `ref/ia/`.

Workspace 1 is the user's: a test window (GTK, browser, bench) goes to a virtual output or to workspace 5, by the commands in `legacy/BRIEF.md` § Headed windows. The same file, § Architecture, carries the glyph-advance rule every edit under `legacy/app/` obeys.

## Rust

The `rust-analyzer-lsp` plugin is installed, and its `LSP` tool arrives deferred, so it is loaded the moment Rust enters the session: the first `.rs` path in context — in the ticket, in a fork's report, in a `grep` result, in a listing — is followed by one call, `ToolSearch` with `select:LSP`, before any other tool touches the file. This holds in the main session as much as in a fork. Loaded, it answers definition, references, hover, symbols and call hierarchy for either crate.

Every question about a Rust symbol — where it is defined, who calls it, what its type is, what a module exports — goes to the LSP tool first; `grep` and `cat` are for what it cannot answer: string literals, comments, and files that are not Rust.

## Gate

Before landing native work, closing a ticket, or closing a feature: `docs/agents/gate.md` names the tier, its commands, the latency budget, the blind-judging opponent and the Feature tier's Hand test; `docs/agents/hand-tests.md` holds the ported Pieces' checklists.

An `/implement` session whose ticket has no Hand test (Ticket tier only) lands its own work: once the Gate is green and `/code-review` is done, it merges its PR (`gh pr merge --merge`) and closes the ticket. Only a Hand test hands the close to the owner.

## Context in an `/implement` session

The smart zone is about 120k tokens. A session is near 60k once this file, the ticket and the docs it names are in context, and every tool call then adds its result plus about 0.4k of reasoning that stays for the rest of the session, so the zone is held by making fewer, smaller calls (one of the twelve landed tickets stayed inside it, on 57 calls; the rest made 122–290 and ran 165k–302k).

- **Orientation is delegated to a fork.** Before the first edit, an Agent call with `subagent_type: "fork"` maps the area and returns `file:line` ranges; this context reads those ranges. The fork is the one agent type that carries the `LSP` tool — the built-in types (Explore, general-purpose) are never given it, deferred or loaded, and answer every symbol question with `grep` (measured 2026-08-29). The fork inherits this context with the tool deferred, so its prompt tells it to load the tool with `ToolSearch` and take every Rust symbol question through it (§ Rust); `grep` stays for strings, comments and files that are not Rust. Its first LSP call can come back empty while rust-analyzer is still indexing, and the second answers. Where-is-what questions go to the module map — every module opens with a `//!` line, so `grep -rn -m1 '^//!' --include='*.rs' quill quill-engine` is both crates on one screen — or to the LSP tool (§ Rust), which answers in lines where a `cat` costs the file.
- **The Gate is one call.** `tools/gate check` is the whole Commit tier in one result. While iterating: `cargo check -q --message-format=short`, `cargo test <name>`, and listings through `head` or `grep`. `tools/gate judge` and `bench` are read for their summary lines; the shots are the critic's to look at.
- **The ticket is fetched once**, with its parent spec, to a file under the job's tmp directory by `tools/ticket <N>`, and later questions are answered from that file by `sed -n` range.
- **Docs by section.** This file is already in context. `docs/agents/gate.md` for the tier the ticket names, `docs/architecture.md` for the sections the ticket cites, ADRs by number; `docs/agents/hand-tests.md` is `/to-spec`'s reading.
- **One tool per file.** A file the harness has seen through Read, Write or Edit and then changed through Bash — `sed -i`, a heredoc, `cargo fmt` — comes back into context as a diff snippet (one session paid 60 KB this way). Files opened with Bash stay with Bash; files touched with Write or Edit change through Edit, written in rustfmt's shape so `cargo fmt` changes nothing. In a worktree, files are created with Write and changed with Edit from the first edit, whatever the permission mode says about preferring Bash.
- **Worktree Bash is one plain command per call.** Once the session has entered `.claude/worktrees/`, the isolation check reads a command's shape rather than its targets and refuses heredocs, `;`-chains and `for` loops, even ones that touch only `gh` or the job's tmp directory; a sweep that needs a loop runs in a subagent before `EnterWorktree`.
- **Background agents are waited on by ending the turn.** A review agent's report arrives as a task notification, and no call brings it sooner: the task list, a `sleep`, an idle loop all answer "still running" at the price of a turn each. Whatever is independent of the report — the commit, the push, the PR body — is done in the turn that launches the agents; that turn then names what it awaits in one line and ends, and every turn that opens before the notification is that one line and nothing else.

## Agent docs

Edit `CLAUDE.md`, `CONTEXT.md`, `docs/agents/*.md`, ADRs and any other document an agent reads through `/writing-for-agents`.

## Agent skills

### Issue tracker

Issues and specs live in this repo's GitHub Issues (`danielbaldwin47/Quill`, private), driven through the `gh` CLI. See `docs/agents/issue-tracker.md`.

### Specs and tickets

`/to-spec` and `/to-tickets` read `docs/agents/tickets.md` before publishing: it sizes a spec or a ticket for one `/implement` session inside the smart zone, names the **Size** and **Reading** lines a ticket carries, and says how a spec closes.

### Triage labels

Default vocabulary: `needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`. See `docs/agents/triage-labels.md`.

### Domain docs

Single-context: `CONTEXT.md` and `docs/adr/` at the repo root, created lazily by `/domain-modeling`. See `docs/agents/domain.md`.

### Code review

`/code-review`, wherever this repo's docs say it, is `/mattpocock-skills:code-review`, invoked by that full name: a second skill named `code-review` ships with Claude Code, and the full name picks the right one.
