# Quill

A long-form writing environment for Linux: a native GTK4 app in Rust, ported Piece by Piece from the JavaScript app that won its blind gauntlet against iA Writer. That app is now `legacy/`, kept as the **Parity oracle** every native Piece is judged against until the last one is won, except the states `docs/design.md` hands to the **Design oracle** — iA Writer for Mac as measured in `ref/ia/mac-native/` (ADR 0015). `docs/architecture.md` specifies the native app: an `/implement` session reads the sections its ticket cites (§ Context in an `/implement` session); any other Rust, packaging or repo work reads it first. Its Gate is `docs/agents/gate.md`, and `CONTEXT.md` holds the vocabulary both use.

## Repo map

- `quill/` — the app crate, the only crate that may see `gtk` ([ADR 0008](docs/adr/0008-engine-crate-without-gtk.md)).
- `quill-engine/` — the display-free half: text model, Markdown, Annotators, Library, settings, Templates, rendering. It builds and tests with no display attached, and `tests/boundary.rs` fails `cargo test` if `gtk` slips in.
- `fonts/` — the six Quill Faces, Inter and Source Serif 4 (the two Template families), and their OFL licences (SIL OFL 1.1), all loaded privately at startup; `tools/fontbuild.py` builds the Faces from the iA originals.
- `tools/` — the Gate's command and its helpers: `tools/gate --help` names its five subcommands and what each ends in, `docs/agents/gate.md` says which tier requires which, and a selftest beside each subcommand keeps it honest. `npm i` at the root once, for the ones that drive a browser. Two here are the legacy app's rather than the Gate's: `gauntlet.workflow.js` (its builder/critic loop) and `mirror-metrics.mjs` (its mirror/textarea drift check). Three are a ticket's: `tools/ticket <N>` (§ Context in an `/implement` session), `context-report <N>` (what a landed ticket's `/implement` session cost, in the one line its closing comment carries) and `context-report-selftest` (which keeps that honest against a written-out transcript); `run-cost <transcript>...` is what any agent run cost in API dollars, read from a Claude Code or Codex transcript, with `run-cost-selftest` beside it. `ink-coverage.mjs` reads how much ink a judged crop lays down against the oracle's crop, for a rasterisation question a critic should not be spent on (#212).
- `legacy/` — the JavaScript app as it won, and the Parity oracle: `legacy/bin/quill` opens it from the checkout, `legacy/tools/` shoots and benches it (`npm i` inside `legacy/` too), `legacy/BRIEF.md` and `legacy/NOTES.md` describe it. It is ISC, under its own `legacy/LICENSE`.
- `ref/ia/` — iA's fonts, templates and marketing stills; `ref/ia/mac-native/` is the Design oracle as measured (`VERDICTS.md`, `NOTES.md`, the captures under `ref/ia/shots/mac-native/`), and it outranks every still. `ref/sample.md` is the shared test passage, and `ref/short.md` the one that fits inside a window.
- `shots/` and `progress/` — judging evidence: per-Piece screenshots, round verdicts, latency JSON and the report. `shots/oracle/` is the judged states (`states.json`) and the Parity oracle frozen at them, or the `mac-native` crop a state names; `shots/blind/` is the critic's copy of shots committed elsewhere, ignored by git.
- `PKGBUILD` + `packaging/` — Arch package of the native binary; `README.md` § Build, install and run the native app has the build and install commands and the traps they avoid. The `.desktop` file and the icon are named for the application id, `io.github.danielbaldwin47.Quill`.
- `docs/` — `architecture.md` (the native spec), `design.md` (what the writing surface is meant to do where the Design oracle and the Parity oracle disagree — read its row before a caret, selection, palette, focus, markup or type ticket, `grep -n '^| <Row> |' docs/design.md`, and its § Adding or changing a row before a spec), `shortcuts.md` (the one shortcut table every menu, the Palette and the shortcuts window read), `adr/` (decisions), `agents/` (the docs that § Gate and § Agent skills point at).

Licences: GPL-3.0-or-later at the root (`LICENSE`), ISC in `legacy/`, OFL-1.1 for `fonts/`, Apache-2.0 for the bundled `harper-brill` tagger (`packaging/harper-brill-LICENSE`), and iA's own terms for `ref/ia/`.

Workspace 1 is the user's: a test window (GTK, browser, bench) goes to a virtual output or to workspace 5, by the commands in `legacy/BRIEF.md` § Headed windows. The same file, § Architecture, carries the glyph-advance rule every edit under `legacy/app/` obeys.

## Rust

The `rust-analyzer-lsp` plugin is installed, and its `LSP` tool arrives deferred, so it is loaded the moment Rust enters the session: the first `.rs` path in context — in the ticket, in a fork's report, in a `grep` result, in a listing — is followed by one call, `ToolSearch` with `select:LSP`, before any other tool touches the file. Loaded, it answers definition, references, hover, symbols and call hierarchy for either crate.

Every question about a Rust symbol — where it is defined, who calls it, what its type is, what a module exports — goes to the LSP tool first; `grep` and `cat` are for what it cannot answer: string literals, comments, and files that are not Rust.

## Gate

Before landing native work, closing a ticket, or closing a feature: `docs/agents/gate.md` names the tier, its commands, the latency budget, the blind-judging opponent and the Feature tier's Hand test; `docs/agents/hand-tests.md` holds the ported Pieces' checklists.

An `/implement` session whose ticket has no Hand test (Ticket tier only) lands its own work: once the Gate is green and `/code-review` is done, it merges its PR (`gh pr merge --merge`; the body's `Closes #N` closes the ticket) and posts the `tools/context-report` line with `gh issue comment`. Only a Hand test hands the close to the owner.

## Context in an `/implement` session

The smart zone is about 120k tokens. A session is near 60k once this file, the ticket and the docs it names are in context, and every tool call then adds about 0.25k of result and about 1.2k of reasoning that stays for the rest of the session — the reasoning is two thirds of a session's peak, and ending the turn does not release it (measured over #111, #113, #115 and #168 on 2026-09-01) — so the zone is held by making fewer calls, not smaller ones (one of the twelve landed tickets stayed inside it, on 57 calls; the rest made 122–290 and ran 165k–302k).

- **Orientation is delegated to a fork.** Before the first edit, an Agent call with `subagent_type: "fork"` maps the area and returns `file:line` ranges; this context reads those ranges. The rule holds after a compaction and through a hand-fix pass (one session that let it lapse made 45 greps and compacted twice); a ticket naming one module reads that module by `sed -n` range and skips the fork (#229, the cheapest session). The fork is the one agent type that carries the `LSP` tool — the built-in types (Explore, general-purpose) are never given it, deferred or loaded, and answer every symbol question with `grep` (measured 2026-08-29). The fork inherits this context with the tool deferred, so its prompt tells it to load the tool with `ToolSearch` and take every Rust symbol question through it (§ Rust); `grep` stays for strings, comments and files that are not Rust. Its first LSP call can come back empty while rust-analyzer is still indexing, and the second answers. Where-is-what questions go to the module map — every module opens with a `//!` line, so `grep -rn -m1 '^//!' --include='*.rs' quill quill-engine` is both crates on one screen — or to the LSP tool (§ Rust), which answers in lines where a `cat` costs the file.
- **The Gate is one call.** `tools/gate check` is the whole Commit tier in one result. While iterating: `cargo clippy --all-targets -q --message-format=short -- -D warnings` (the Gate's own step, under a second on a checked tree, so a lint is red here rather than one `gate check` later), `cargo test <name>`, and listings through `head` or `grep`. A judged Piece is `tools/gate shoot` in the foreground to look, then `tools/gate judge` in the background (`run_in_background`, the turn ended; it outlasts the foreground cap — `docs/agents/gate.md` § Ticket tier, "As run"), and `bench` in the foreground; each is read for its summary lines, and the shots are the critic's to look at.
- **The ticket is fetched once**, with its parent spec, to two files under the job's tmp directory by `tools/ticket <N>` (the spec beside the ticket, each with a heading index), and later questions are answered from those files by `sed -n` range; `tools/ticket --graph <M>` is a spec's children on one screen.
- **Docs by section.** This file is already in context. `docs/agents/gate.md` for the tier the ticket names, `docs/architecture.md` for the sections the ticket cites, ADRs by number; `docs/agents/hand-tests.md` is `/to-spec`'s reading.
- **One tool per file.** A file the harness has seen through Read, Write or Edit and then changed through Bash — `sed -i`, a heredoc, the `cargo fmt` that `tools/gate check` now applies — comes back into context as a diff snippet (one session paid 60 KB this way). Files opened with Bash stay with Bash; files touched with Read, Write or Edit change through Edit, written in rustfmt's shape so `cargo fmt` changes nothing — a `PreToolUse` hook (`.claude/hooks/edited-files-guard.sh`) refuses an in-place `sed` on such a file. In a worktree, files are created with Write and changed with Edit from the first edit, whatever the permission mode says about preferring Bash.
- **Worktrees are `docs/agents/worktree.md`**: what the isolation check refuses after `EnterWorktree`, how an orchestrator and its forks share them, and how one is left.
- **A turn ends by replying with no tool call — that ending is the only wait.** A background agent's report, and a backgrounded command's exit, arrive as task notifications only after the reply finishes without a tool use; every tool call — even a no-op `echo` or a log `tail` — continues the same turn. The turn that launches the work does everything independent of the report — the commit, the push, the PR body — then names what it awaits in one line, and that line is the reply's end; every turn that opens before the notification is that one line and nothing else. A `SendMessage` to a running agent is answered by a later notification of its own, so a notification silent on it is the agent's earlier reply, and the work stays the agent's.

## Agent docs

Edit `CLAUDE.md`, `CONTEXT.md`, `docs/agents/*.md`, ADRs and any other document an agent reads through `/writing-for-agents`.

## Agent skills

### Issue tracker

Issues and specs live in this repo's GitHub Issues (`danielbaldwin47/Quill`, private), driven through the `gh` CLI. See `docs/agents/issue-tracker.md`.

### Specs and tickets

`/to-spec` and `/to-tickets` read `docs/agents/tickets.md` before publishing: it sizes a spec or a ticket for one `/implement` session inside the smart zone, names the **Size** and **Reading** lines a ticket carries, and says how a spec closes.

### Implementing a spec

`/implement-spec` reads `docs/agents/implement-spec.md` before its first step.

### Model comparison

Running one ticket on two or more models and comparing the results — the ticket's extra sections, a worktree per model, `tools/run-cost` on the transcripts, the review pair, the verdict — is `docs/agents/model-comparison.md`.

### Triage labels

Default vocabulary: `needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`. See `docs/agents/triage-labels.md`.
Default vocabulary: `needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`. A capture ticket (the one a spec names under **Waits on captures**, `docs/agents/tickets.md`) carries `ready-for-capture` in place of `ready-for-agent`: its work is a capture on the Mac. See `docs/agents/triage-labels.md`.
### Domain docs

Single-context: `CONTEXT.md` and `docs/adr/` at the repo root, created lazily by `/domain-modeling`. See `docs/agents/domain.md`.

### Code review

`/code-review`, wherever this repo's docs say it, is `/mattpocock-skills:code-review`, invoked by that full name: a second skill named `code-review` ships with Claude Code, and the full name picks the right one.
