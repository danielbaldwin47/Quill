# How `/implement-spec` runs in this repo

Where a line here and the skill's differ, this line holds.

- **The graph is `tools/ticket --graph <M>`**: every child's state, blockers, Size and Reading lines on one screen. A fork is handed its ticket number and runs `tools/ticket <N>` itself.
- **Every agent is a fork** — the exploration agent and each implementer — unless the owner names a model: the fork is the one agent type carrying the `LSP` tool (`CLAUDE.md` § Context in an `/implement` session).
- **Worktrees** are `docs/agents/worktree.md` § An orchestrator and its forks: the orchestrator stays in its checkout, makes each ticket's worktree off the spec branch by absolute path, and addresses one with `git -C <absolute path>` (§ An orchestrator and its forks).
- **The orchestrator merges each ticket branch itself**, `git merge` in the spec worktree; a merger subagent is a context hop and nothing else. A hedge in a fork's report ("rename if review objects") is settled at that merge rather than left for `/code-review`. After every merge and before its commit, `tools/gate check`: a semantic conflict survives a clean textual merge (#263's `band` signature; the one orchestrator that tested after each of eleven merges hit no surprise).
- **An implementer's test command runs under `timeout`** (`timeout 900 cargo test …`): a blocking-wait patch hung `cargo test` for 2 h 39 m at 0 % CPU on #376 while its agent waited on it.
- **A headed Hand test precedes the review pair.** Once every ticket is merged, one fork runs the spec's **Hand test** steps against the built binary through the `run` skill and reports each step pass or fail; a failed step goes back to its ticket's implementer before `/code-review` starts. Both review axes read a diff and never run the app: #263 passed review and failed five of ten steps on the owner's build, and #356's toggle blanked both Annotators.
- **The orchestrator runs `tools/gate bench` once, on the merged spec branch.** The bench is its alone because a sibling's build skews the numbers, and the display lock serialises shoots, not builds.
- **A fork's report ending in "awaiting the judge"** is a pause the judge's exit wakes: the fork resumes on its own, and a `Monitor` on the judge's pid is for the orchestrator's own reading.
- **On close, one context line per ticket** from the fork's task notification (`tool_uses`, `subagent_tokens`), since `tools/context-report` finds no `/implement-spec` session.
- **A branch merged only into the spec branch** is deleted with `git merge-base --is-ancestor <b> <spec> && git branch -D <b>`; `-d` refuses it.
