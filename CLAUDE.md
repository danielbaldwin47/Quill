# Quill

A long-form writing environment for Linux: a native GTK4 app in Rust, ported Piece by Piece from the JavaScript app that won its blind gauntlet against iA Writer. That app is now `legacy/`, kept as the **Parity oracle** every native Piece is judged against until the last one is won. `docs/architecture.md` specifies the native app: read it before any Rust, packaging or repo work. Its Gate is `docs/agents/gate.md`, and `CONTEXT.md` holds the vocabulary both use.

## Repo map

- `quill/` — the app crate, the only crate that may see `gtk` ([ADR 0008](docs/adr/0008-engine-crate-without-gtk.md)). `main.rs` reads the command line, loads the Faces and opens the settings before any window, because every flag is applied before the first frame.
- `quill-engine/` — the display-free half: text model, Markdown, Annotators, Library, settings, Templates, rendering. It builds and tests with no display attached, and `tests/boundary.rs` fails `cargo test` if `gtk` slips in.
- `fonts/` — the six Quill Faces and `OFL.txt` (SIL OFL 1.1), loaded privately at startup; `tools/fontbuild.py` builds them from the iA originals.
- `tools/` — the Gate's command and its helpers. `tools/gate check` is the Commit tier in one line (`tools/gate --help`), and `tools/gate-fixture/selftest` keeps it honest against a crate outside the workspace whose defects it must go red on. The rest: `blind.mjs`/`thumb.mjs` (blind A/B pairs), `progress.mjs` (builds `progress/index.html`), `regimes.mjs` (the twelve latency regimes and the typist, typed by both benches), `uinput-keys.py` (real keys through `/dev/uinput`), `idle-check.py` (is anybody at this machine), `fontbuild.py`/`fontgrid.py` (the Faces), `mkdoc.mjs` (builds the 10k-word bench corpus). Two here are the legacy app's rather than the Gate's: `gauntlet.workflow.js` (its builder/critic loop) and `mirror-metrics.mjs` (its mirror/textarea drift check). `npm i` at the root once, for the three that drive a browser.
- `legacy/` — the JavaScript app as it won, and the Parity oracle: `legacy/bin/quill` opens it from the checkout, `legacy/tools/` shoots and benches it (`npm i` inside `legacy/` too), `legacy/BRIEF.md` and `legacy/NOTES.md` describe it. It is ISC, under its own `legacy/LICENSE`.
- `ref/ia/` — the iA Writer screenshots, fonts and templates every comparison is judged against; `ref/sample.md` is the shared test passage.
- `shots/` and `progress/` — judging evidence: per-Piece screenshots, blind pairs, round verdicts, latency JSON and the report. `shots/oracle/` holds the judged states the Parity oracle is shot at; the frozen shots themselves land there with the Gate tooling ([#19](https://github.com/danielbaldwin47/Quill/issues/19)) and are regenerated only when `legacy/` changes.
- `PKGBUILD` + `packaging/` — Arch package of the native binary: `makepkg -f` then `sudo pacman -U quill-[0-9]*.pkg.tar.zst` (the glob keeps the `-debug` split package out). The `.desktop` file and the icon are named for the application id, `io.github.danielbaldwin47.Quill`.
- `docs/` — `architecture.md` (the native spec), `shortcuts.md` (the one shortcut table every menu, the Palette and the shortcuts window read), `adr/` (decisions), `agents/` (the Gate, issue tracker, triage labels and domain-doc rules for the skills below).

Licences: GPL-3.0-or-later at the root (`LICENSE`), ISC in `legacy/`, OFL-1.1 for `fonts/`, and iA's own terms for `ref/ia/`.

Hard rule in `legacy/app/`: no per-token style may change glyph advance width, or the mirror and textarea drift apart (bold/italic are safe; iA fonts share widths across weights).

Hard rule for any test window (GTK, browser, bench): it opens on a virtual output — `hyprctl output create headless`, what `legacy/bin/quill --measure` does by default — or, when it must be on the real monitor, on workspace 5 with `[workspace 5 silent]`. Workspace 1 is the user's live workspace. The Hyprland 0.56 commands are in `legacy/BRIEF.md` § Headed windows; without `hyprctl`, run headless.

## Gate

Before landing native work, closing a ticket, or closing a feature: `docs/agents/gate.md` names the tier, its commands, the latency budget, the blind-judging opponent and the Hand test checklists.

An `/implement` session whose ticket has no Hand test (Ticket tier only) lands its own work: once the Gate is green and `/code-review` is done, it merges its PR (`gh pr merge --merge`) and closes the ticket. Only a Hand test hands the close to the owner.

## Agent docs

Edit `CLAUDE.md`, `CONTEXT.md`, `docs/agents/*.md`, ADRs and any other document an agent reads through `/writing-for-agents`.

## Agent skills

### Issue tracker

Issues and specs live in this repo's GitHub Issues (`danielbaldwin47/Quill`, private), driven through the `gh` CLI. See `docs/agents/issue-tracker.md`.

### Specs

A `/to-spec` issue carries the `spec` label. Size it when it is created: a spec that fits one `/implement` session (the smart zone, about 120k tokens, including tests, `/code-review` and the Gate) keeps `ready-for-agent` and opens with one line saying so ("One-session spec: …" and the reason); one too large for a session loses `ready-for-agent` and is split by `/to-tickets` in a fresh session, its child tickets carrying the label and naming the spec under Parent. Either way the spec closes on the owner's `hand test: pass`. The agent's last comment on a spec ends with a **Hand test** section: the install command, then the numbered "do X, see Y" steps written out in full (the `docs/agents/gate.md` checklist merged with the spec's additions), so the owner tests from that comment alone. The owner's `hand test: pass` with the `pacman -Q quill` output closes the spec; a failed step is commented on the spec and returns it to the agent.

### Triage labels

Default vocabulary: `needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`. See `docs/agents/triage-labels.md`.

### Domain docs

Single-context: `CONTEXT.md` and `docs/adr/` at the repo root, created lazily by `/domain-modeling`. See `docs/agents/domain.md`.
