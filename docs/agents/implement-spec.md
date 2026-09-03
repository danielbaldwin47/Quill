# How `/implement-spec` runs in this repo

`/implement-spec` reads this before its first step; where a line here and the skill's differ, this line holds.

- **The graph is `tools/ticket --graph <M>`**: every child's state, blockers, Size and Reading lines on one screen. A fork is handed its ticket number and runs `tools/ticket <N>` itself.
- **Every agent is a fork** — the exploration agent and each implementer — unless the owner names a model: the fork is the one agent type carrying the `LSP` tool (`CLAUDE.md` § Context in an `/implement` session).
- **Worktrees** are `docs/agents/worktree.md` § An orchestrator and its forks: the orchestrator stays in its checkout and makes each ticket's worktree off the spec branch by absolute path.
- **The orchestrator merges each ticket branch itself**, `git merge` in the spec worktree; a merger subagent is a context hop and nothing else. A hedge in a fork's report ("rename if review objects") is settled at that merge rather than left for `/code-review`.
- **Forks never bench.** A sibling's build skews the numbers, and the display lock serialises shoots, not builds; the orchestrator runs `tools/gate bench` once, on the merged spec branch.
- **A fork's report ending in "awaiting the judge"** is a pause the judge's exit wakes: the fork resumes on its own, and a `Monitor` on the judge's pid is for the orchestrator's own reading.
- **On close, one context line per ticket** from the fork's task notification (`tool_uses`, `subagent_tokens`), since `tools/context-report` finds no `/implement-spec` session.
- **A branch merged only into the spec branch** is deleted with `git merge-base --is-ancestor <b> <spec> && git branch -D <b>`; `-d` refuses it.
