# Coding standards

What `/code-review` holds a diff to, beyond the smell baseline the skill carries itself. `tools/gate check` already enforces `cargo fmt`, clippy at `-D warnings`, `missing_docs` on public items, the same-line reason on every `#[allow]`, the engine's `gtk` boundary and every selftest, so a review skips those. Each rule below is one a review has found by hand more than once since 2026-08-28; the commits and PRs after it are the evidence, and where a doc already carries the rule, the line here points at it.

## Comments and docs

- A doc comment sits on the item it describes: an item inserted above a documented one goes above that item's `///` block, and one inserted below it goes below the item. (dc60fb2, #175, #181, and `watch_edits` in `quill/src/window.rs` until #190.)
- A module's `//!` line and a struct's doc describe the module after the change; a doc that says "only when" or "never" is checked against the new path. (dc60fb2, 3d91cdb)
- A citation — an ADR, a `REFERENCE.md` section, a ticket, "the oracle's comment" — says what the cited source says; the reviewer opens it and finds the claim. (dc60fb2, 513a9b6, 63deb6f, e439bb6)
- A number the code implements lives in the code; a doc that mentions it names the function or file that holds it rather than restating the value. (3d91cdb, b78c425, ea8ef8f)
- An ADR touched by a change carries a dated status line in its header naming what changed it and whether the decision stands, is narrowed, or waits on a named ticket; it states what is on `main`, never what a later ticket will land. (4a6a8af, 6ff8e46, c2969cf, 697cc37)
- A row marked "still unknown" or a superseded reading stays as written; changing it is its own ticket. (#173, 697cc37)

## Shape

- Round once, at the end, in one named helper (`quill::tags::pixels` is the shape); the edges downstream are integer arithmetic on the rounded value. (9116691, #174; `docs/design.md` rows Caret width and Line pitch)
- Device-to-logical pixels and colour channels go through the named helper (`logical`, `channel`, `pixels`), never a bare `as`. (513a9b6, 9116691)
- A predicate or walk written in two functions, or re-derived in a test, is one helper both call. (031072a, 296188b, 513a9b6, 91ea16d)
- A sentinel is an enum variant; a flag is a `bool`; two integers that travel together are a named struct. (296188b, 3d91cdb, 31ba642)
- Every walk over blocks, events or spans has a stated bound and a test or bench that pins its cost; `docs/architecture.md` § Annotators gives the budget. (30a6dce, c90fd22)
- A name is free of its neighbours: a private fn does not share a name with an imported type (`run` beside `annotate::Run`, `Caret` beside `flags::Caret`), and two parameters that mean different things do not read as a symmetric pair (`head_room`/`foot`, not `above`/`below`). (513a9b6, 031072a, dc60fb2)

## Tests

- A test asserts the claim through the function the code uses: whole-struct equality over a tuple of fields, `resolve(...)` over a literal `Look`. (c90fd22, 31ba642)
- A test's name is the sentence it proves, in snake_case (`a_failed_write_leaves_the_writers_file_as_it_was`); a scratch file it writes carries the pid, because worktrees test concurrently. (63deb6f)
- A behaviour the change claims lands with the test that names the case, and a claim over a range ("rises to step 2, then falls") is asserted over the whole range. (b78c425, 30a6dce)

## Tools and flags

- A change to what a `tools/gate` subcommand prints or exits with changes `usage()` and `docs/agents/gate.md` in the same commit. (0cd10a1, a2db4c0, c703b17, e439bb6, 90f8b8d)
- An error names the case that produced it, with the exit code `gate.md` gives that case, and interpolates the flag or value once (`{flag}`) rather than spelling it. (c703b17, 90f8b8d, #181)
- Every flag a `shots/oracle/states.json` default or `tools/harness.mjs` passes still parses after the change. (#162's `--size 20`)
- A command that shoots nothing deletes nothing, and a tool that removes tracked files has a selftest asserting what survives. (#161's sweep)
- A selftest's fixture is derived from the file it checks (`fingerprint.json`'s keys), never a copied literal. (#161, #165)

## Spec

- The ticket's Out-of-scope is the change's boundary; a fix found on the way (a stale selftest, a wrong exit code) is filed, or its commit names the ticket it belongs to. (#164's #139 fix, #162's `--size`)
- A judging ticket's brief is scope: an edit to it says what ours cannot render, and nothing about what the critic should weigh. (#165 round 6)
