# Coding standards

What `/code-review` holds a diff to, beyond the smell baseline the skill carries itself. `tools/gate check` already enforces `cargo fmt`, clippy at `-D warnings`, `missing_docs` on public items, the same-line reason on every `#[allow]`, the engine's `gtk` boundary and every selftest, so a review skips those. Each rule below is one a review has found by hand more than once since 2026-08-28; the commits and PRs after it are the evidence, and where a doc already carries the rule, the line here points at it.

## Comments and docs

- A doc comment sits on the item it describes: an item inserted above a documented one goes above that item's `///` block, and one inserted below it goes below the item. (16a107f, #175, #181, and `watch_edits` in `quill/src/window.rs` until #190.)
- A module's `//!` line and a struct's doc describe the module after the change; a doc that says "only when" or "never" is checked against the new path. (16a107f, 0155ef4)
- A citation — an ADR, a `REFERENCE.md` section, a ticket, "the oracle's comment" — says what the cited source says; the reviewer opens it and finds the claim. (16a107f, 4e05ed9, aa4bfef, b47d514)
- A `#N` in a doc comment is about an issue as it stands: `tools/cited-issues` lists the closed ones, and each is read as history or removed. (#121, closed, in `quill/src/chrome.rs` and `quill/src/menus.rs` until 2026-09-02)
- A number in a row or an ADR is one a script printed from a named capture, quoted as `docs/agents/tickets.md` § Tickets (Numbers) has it; a derived one says it is derived. (#169)
- A number the code implements lives in the code; a doc that mentions it names the function or file that holds it rather than restating the value. (0155ef4, 644f946, 811238c)
- An ADR touched by a change carries a dated status line in its header naming what changed it and whether the decision stands, is narrowed, or waits on a named ticket; it states what is on `main`, never what a later ticket will land. (3540268, 1167158, 33e7eb0, e8719d2)
- A row marked "still unknown" or a superseded reading stays as written; changing it is its own ticket. (#173, e8719d2)

## Shape

- Round once, at the end, in one named helper (`quill::tags::pixels` is the shape); the edges downstream are integer arithmetic on the rounded value. (6181fc6, #174; `docs/design.md` rows Caret width and Line pitch)
- Device-to-logical pixels and colour channels go through the named helper (`logical`, `channel`, `pixels`), never a bare `as`. (4e05ed9, 6181fc6)
- A predicate or walk written in two functions, or re-derived in a test, is one helper both call. (d68ba75, e15bce8, 4e05ed9, 2e79748)
- A sentinel is an enum variant; a flag is a `bool`; two integers that travel together are a named struct. (e15bce8, 0155ef4, 6893532)
- Every walk over blocks, events or spans has a stated bound and a test or bench that pins its cost; `docs/architecture.md` § Annotators gives the budget. (7d17061, 1bdfc19)
- A name is free of its neighbours: a private fn does not share a name with an imported type (`run` beside `annotate::Run`, `Caret` beside `flags::Caret`), and two parameters that mean different things do not read as a symmetric pair (`head_room`/`foot`, not `above`/`below`). (4e05ed9, d68ba75, 16a107f)

## Tests

- A test asserts the claim through the function the code uses: whole-struct equality over a tuple of fields, `resolve(...)` over a literal `Look`. (1bdfc19, 6893532)
- A test's name is the sentence it proves, in snake_case (`a_failed_write_leaves_the_writers_file_as_it_was`); a scratch file it writes carries the pid, because worktrees test concurrently. (aa4bfef)
- A behaviour the change claims lands with the test that names the case, and a claim over a range ("rises to step 2, then falls") is asserted over the whole range. (644f946, 7d17061)
- A file Quill writes is tested with the edit the docs tell the writer to make appended to what it wrote (`docs/shortcuts.md` § Rebinding over `Settings::to_toml`). (c48ec65, #44)
- A watch is tested against the process's own read and write of the watched path. (c48ec65)

## Tools and flags

- A change to what a `tools/gate` subcommand prints or exits with changes `usage()` and `docs/agents/gate.md` in the same commit. (99803a8, 563bfe8, c8025f0, b47d514, 6ad1cdc)
- An error names the case that produced it, with the exit code `gate.md` gives that case, and interpolates the flag or value once (`{flag}`) rather than spelling it. (c8025f0, 6ad1cdc, #181)
- Every flag a `dev/shots/oracle/states.json` default or `tools/harness.mjs` passes still parses after the change. (#162's `--size 20`)
- A command that shoots nothing deletes nothing, and a tool that removes tracked files has a selftest asserting what survives. (#161's sweep)
- A selftest's fixture is derived from the file it checks (`fingerprint.json`'s keys), never a copied literal. (#161, #165)

## Spec

- The ticket's Out-of-scope is the change's boundary; a fix found on the way (a stale selftest, a wrong exit code) is filed, or its commit names the ticket it belongs to. (#164's #139 fix, #162's `--size`)
- A judging ticket's brief is scope: an edit to it says what ours cannot render, or which oracle owns a row, and nothing about what the critic should weigh; a clause aimed at a Design-judged state is wrong by construction, since that critic sees a crop and no brief. (#165 round 6, #229)
