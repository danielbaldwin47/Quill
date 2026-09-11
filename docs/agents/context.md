# Context: what an `/implement` session costs

The measurements behind `CLAUDE.md` § Context in an `/implement` session. The steps live there; this file is why they are shaped as they are, and it is re-measured rather than argued with.

## The budget

The smart zone is about 120k tokens. A session is near 60k once `CLAUDE.md`, the ticket and the docs it names are in context. Every tool call then adds about 0.25k of result and about 1.2k of reasoning that stays for the rest of the session; the reasoning is two thirds of a session's peak, and ending the turn does not release it (measured over #111, #113, #115 and #168 on 2026-09-01). So the zone is held by making fewer calls, not smaller ones: of the twelve tickets landed by 2026-09-01, one stayed inside it on 57 calls, and the rest made 122–290 calls and ran 165k–302k.

## What the steps cost when skipped

Measured over eighteen sessions on 2026-09-10 and 2026-09-11, in a retrospective on 2026-09-11:

- **Orientation inline instead of in a fork.** None of eight `/implement` sessions forked before its first edit; inline orientation cost 25k–61k tokens and up to 30 calls each, and #319 peaked at 370k. The fork's report costs this context a page. A session that let the rule lapse mid-way made 45 greps and compacted twice.
- **The LSP tool loaded and unused.** Twelve sessions paid the `ToolSearch` call and made zero LSP calls; four got the empty first answer while rust-analyzer indexed and fell back to `grep` for good. A fork does get the tool (re-measured 2026-09-11: `sleep 20`, and the second call answered); the built-in agent types (Explore, general-purpose, `claude`) and a custom agent definition never do.
- **Worktree refusals.** Twenty-six refusals in eight sessions, every one of three shapes: a `$VAR`, `$(…)` or heredoc inside a chain; a script run with a runtime-computed path; `git` in a compound command. Each cost one to three retries, and one session wrote nine shell scripts to get around them. `docs/agents/worktree.md` § What the isolation check refuses is the full list.
- **A file changed through Bash after Edit.** One session paid 60 KB of diff snippets; over the eight sessions, 52 python3 or node rewrites of files the session had already opened with Edit, 27 of them in #319.
- **Section reads of unwrapped files.** A two-hit `grep` over `progress/state.json` returned 25k characters; `docs/design.md` row listings 11–16k; eleven lines of `docs/agents/gate.md` § Latency 9k. `tools/gate brief <piece>` and `tools/design-rows` are the readers.
- **The landing sequence by hand.** Six to nine calls per ticket, run five times in two days, with the same two steps failing each time (an untracked `shots/latency/bench-*.json` blocking `git worktree remove`; a branch already deleted by `--delete-branch`). `tools/land` is that sequence.
