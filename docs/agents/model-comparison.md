# Model comparison

How one ticket is run on two or more models and the results compared: the same ticket, a worktree and a branch per model, one PR each left unmerged, the runs costed from their transcripts, the branches reviewed on the same two axes, and a verdict the owner lands. `CLAUDE.md` § Agent skills points here; the first run was #302 (2026-09-06: Claude Opus 5, GPT-6 Astra, GPT-5.6 Sol), and its comments on the ticket are the worked example.

The orchestrating session is a Claude Code session in the main checkout. It writes the ticket, launches the Claude side, costs every side, runs the review, and writes the verdict. The other models run in their own CLIs, driven by the owner, and reach the orchestrator through their PR and their transcript on disk.

## 1. The ticket

Written as `docs/agents/tickets.md` says, off an orientation fork's `file:line` report, with two sections the ticket would not otherwise carry. Both models read the ticket and nothing else of this doc, so the ticket carries everything a run must do the same way.

**Comparison report** — the PR body ends with these eight items, in this order:

1. **Time.** `date -Is` at the first tool call and at the PR's opening, and the minutes between.
2. **Tokens and calls.** Tool calls made and peak context if the session can read it; input, cache-write, cache-read and output tokens for an implementation that can see its own usage, otherwise "the orchestrator reads them from the transcript".
3. **Estimated API cost.** Tokens × the model's first-party list price, naming the prices used.
4. **Runs.** How many times build, test, clippy, `tools/gate check` and `tools/gate shoot` ran, and how many failed before the last green one.
5. **Diff shape.** `git diff --stat <base>`, tests added or changed, whether every docs line the ticket names was changed.
6. **Decisions.** What the ticket left open and what was decided, a sentence each; anything the ticket says that was found wrong.
7. **Trouble.** Refused commands, retries, wrong turns, reading that turned out unneeded — the calls a better ticket would have saved.
8. **Confidence.** What was verified headed against what only a test holds, and any known gap.

**Landing** — the branch is `ticket-<N>-<model>` off the parent's branch; the PR targets that branch, carries `Closes #<N>` and the report, and is not merged and gets no closing comment: the owner picks one after the review. Where the ticket touches a Hand test, the winner's merge re-posts the affected steps on the parent (`docs/agents/hand-tests.md`).

An implementer cannot see its own usage, so item 2 and 3 on the Claude side say so and the orchestrator fills them in (§ 3). A model that reports its own cost reports a snapshot taken before its last calls; the transcript is the figure.

## 2. The runs

**The Claude side** is a subagent, not a fork: `Agent` with `subagent_type: "general-purpose"` and `model:` set, on a worktree the orchestrator makes by absolute path off the parent's branch (`docs/agents/worktree.md` § An orchestrator and its forks). Its prompt carries the orientation fork's report inline — the subagent has no `LSP` tool and would spend calls re-deriving it — plus the worktree rules (absolute paths, `cd <worktree> &&` on every command, no `EnterWorktree`), the Gate commands and the isolation guard's refusals, the commit trailer for its model, and the order to open the PR and stop. Its first command is `date -Is`.

**The other side** is the owner's: the same ticket number as the whole first prompt, a worktree named the same way, the CLI's own orientation. Notes from the owner on what the session was told and where it was interrupted go in the verdict.

**Fairness** is the same ticket, the same base commit and the same brief; an orientation the Claude side is handed inline is one the other side has to earn, and the verdict says so.

## 3. Costing

`tools/run-cost <transcript>...` reads a run's transcripts and prints per model the API calls, tool calls, peak context, wall-clock, the four token counts and the cost at list price; its doc says which files are which and what each shape can and cannot answer. A Claude subagent's transcript is `~/.claude/projects/<project dir>/<session>/subagents/agent-<id>.jsonl`, the id from the `Agent` result; a Codex run is the `rollout-*.jsonl` files under `~/.codex/sessions/<date>/` written during the run, the main thread plus one file per sub-agent it spawned — cost them together, since the sub-agents are the run's. Claude prices are in the tool; another model's are passed as `--price IN,CACHED,OUT` from its own page, and the readout on the PR names the source.

The readout goes on each PR as a comment, so the figure lives with the code, and the verdict's table quotes it. Output on the Claude side is a floor (thinking is not in the transcript); price per token differs by model, so the verdict states both the dollar figure and the token volume, which is the like-for-like number.

## 4. The Gate and the shots

`tools/gate check` in each worktree, in the background, read for its exit code; a run's own report of green is not the measurement. Where the ticket re-shoots an asserted state, the stills are compared by hash across branches (an identical shape shoots identical bytes under `--deterministic`), and `git diff --stat <base> -- shots progress` on each branch says whether the evidence was committed — two of three runs on #302 left the old still in place, and the third committed the still without the reference its own rule reads.

## 5. The review

`/code-review` on every branch: one agent per branch per axis, all from one brief file the orchestrator writes, so the four (or six) reports are comparable. The brief names the worktrees, the base, the standards sources, the smell baseline the skill carries, and — for the Spec axis — the traps the ticket's own reading found (a guard that ignores an argument, a close path, a docs line, an assertion that would still pass on the old behaviour), so every reviewer checks the same things. Reports come back under 400 words each and are attached to the verdict whole.

## 6. The verdict

One comment on the ticket: what the branches share; a table of the measured figures (time, calls, peak, tokens, price per token, cost, diff shape, findings per axis, evidence committed); where they differ, by mechanism; what every branch missed, which is the ticket's defect rather than a model's; then the pick, the fixes it needs, and what to port from the others. The review reports follow under `<details>`. A second run later is an addendum comment with the table extended, so the ticket carries the whole experiment.

The owner lands it: the pick's PR merges into the parent's branch, the others close with `gh pr close --delete-branch`, the worktrees are removed and their local branches deleted (`docs/agents/worktree.md` § Leaving one), and the affected Hand test steps go back to the parent.

## What #302 taught

- A ticket criterion that names a widget ("a `bool` on the window the headless tests can read") is unmeetable headless; name the display-free seam.
- A criterion that names a colour edge is checked against the palette first: at the light theme the column's surround and the paper are one colour, and "the dialog found against the pane's paper" cost every run a shoot to discover.
- Every run swallowed a second Export command for a different format, because the ticket's sentence said "presents the standing dialog"; a sentence that admits a literal reading gets it from every model.
- A `git push` fails in every CLI when the user-level `credential.helper` names a removed `gh` path; `gh auth setup-git` rewrites it.
- A model that spawns its own orientation and review agents (Codex did, both runs) costs 18–36 extra calls and fixes its Standards findings before the PR; it is measured as part of the run.
