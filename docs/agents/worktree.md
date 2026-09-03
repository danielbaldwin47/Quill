# Worktrees

How a session works inside a `.claude/worktrees/` checkout: what the Bash isolation check refuses once `EnterWorktree` has run, how an `/implement-spec` orchestrator and its forks share worktrees, what a worktree holds for `node_modules`, and how one is left. `CLAUDE.md` § Context in an `/implement` session points here.

## What the isolation check refuses

Once the session has entered a worktree, the isolation check reads a command's shape rather than its targets (measured over #160–#187 on 2026-08-30/31 and PR #240 on 2026-09-02). Refused: a chain ending in a heredoc (`git commit -F - <<'EOF'`), a foreground `sleep`, a `for` loop, a `$VAR`, `$(...)` or glob argument to any command other than `bash <script>` or `python3 <script>` (`gh api … -F sub_issue_id=$ID` — "a value computed at runtime inside a construct too complex to verify"), and a `cd` or `git -C` into a sibling worktree, reported as "the shared checkout". Refused separately, by the permission classifier: `git checkout -- .`, as destructive (#227) — working changes move to a fresh branch by a commit, `git checkout -b <new> origin/main` and `git cherry-pick`, and `git branch -f` moves a branch only once the worktree sitting on it has left it.

Passes: `;` and `&&` chains of plain commands with every path spelled out (`git fetch -q origin main; git log …`, `git add -A && git commit -q -F <file>`); a loop or a computed value written to the job's tmp directory with Write and run as one command, `bash <script>` or `python3 <script>` (39 `gh api` calls in one call, 2026-08-30); a commit message, PR body or comment written there with Write and passed by `-F` or `--body-file`; a sibling worktree read with `git worktree list`, `ls`, `find` or `git log <branch>`. A general-purpose subagent spawned after entering keeps Bash under the same rule; an Explore subagent can lose Bash entirely.

## An orchestrator and its forks

The orchestrator stays in the checkout it opened in. It makes each worktree by absolute path — `git worktree add /home/diggle/repos/quill/.claude/worktrees/<name> -b <branch> <base>`, since a relative add from inside a worktree nests the new one under it — and reads, merges and removes by absolute path. Each fork enters its own with `EnterWorktree path=`; a fork refused there ("the repository root, not an isolated worktree", #44) works by absolute path under the worktree the orchestrator made, which the guard allows while the orchestrator is in the checkout. An orchestrator that entered a worktree itself costs every fork its Edit and Write into the fork's own worktree, its `cd <wt> &&` and its `git -C <wt>`, because the guard keys on the orchestrator's worktree and the fork inherits it: #159's six forks made 129 calls through scripts under the job's tmp directory that way.

The first Bash call of a session that may have opened inside another ticket's worktree is `pwd; git branch --show-current`. The main session's cwd persists between Bash calls and a subagent's resets, so a `cd` into a subdirectory is written into the command it serves.

## `node_modules`

`.claude/settings.json` `worktree.symlinkDirectories` links `node_modules` and `legacy/node_modules` into every worktree `EnterWorktree` makes, which is why `git status --short` shows those two as untracked there. A worktree made with `git worktree add` has neither: `tools/` runs anyway, Node resolving `node_modules` upward to the checkout's, and `legacy/node_modules` is linked before `tools/gate oracle` — the one subcommand that runs `legacy/tools/` — with the `ln -s` its refusal prints.

## Leaving one

`ExitWorktree` with `keep`, then `git worktree remove <path>`. A refusal there is the dirty check: shots, logs and round files are committed or deleted first, and `--force` is passed only when `git status --short` shows nothing but the two `node_modules` links. Cleanup runs as its own chain, after every `gh` call that cannot be undone has returned. A worktree holding a branch the owner will check out in `~/repos/quill` frees it first with `git -C <path> checkout --detach`. A branch merged only into a spec branch is deleted as `docs/agents/implement-spec.md` says.
