# Development material

What Quill is judged against, and the evidence of the judging. None of it ships in the package.

Quill began as a JavaScript app, built Piece by Piece and judged blind against iA Writer's own
screenshots until a harsh critic picked it for every Piece. That app is `dev/legacy/`, and it is now
the **Parity oracle**: the native GTK4 app is ported Piece by Piece, and each Piece is judged blind
before it lands (`docs/agents/gate.md`) — against the Parity oracle, or where `docs/design.md` says
so against the **Design oracle**, iA Writer for Mac as measured in `dev/ref/ia/mac-native/`.
`docs/architecture.md` is the native design; `CONTEXT.md` is the vocabulary.

```
dev/legacy/         the JavaScript app as it won, and the Parity oracle (bin/quill, app/, tools/, BRIEF.md, NOTES.md)
dev/ref/ia/         iA Writer reference: screenshots, fonts, templates, spec sheet, sources
                    mac-native/  the Design oracle as measured; its captures are under dev/ref/ia/shots/mac-native/
dev/ref/sample.md   the shared test passage;  dev/ref/short.md  the one that fits in a window
dev/ref/spell.md    the Spell check passage;  dev/ref/spell/  the en_US fixture dictionary the Gate and
                    the engine tests read through ENCHANT_CONFIG_DIR, never the machine's
dev/progress/       state, per-round verdicts, latency report, generated live page
dev/shots/          every round's screenshots, blind pairs, and the states the oracle is shot at
```

Paths inside `dev/legacy/app/`, `dev/legacy/tools/shoot.mjs` and
`dev/shots/oracle/library/manifest.json` still name `ref/…` and `shots/…` at the root, from before
these folders moved under `dev/`: every frozen oracle's fingerprint hashes those bytes, so they were
left as they were.

## Building the package

`README.md` § Install has the three commands. What they avoid:

- `--packagelist` names the package *this* checkout builds, so an older build left in the directory
  cannot be installed in its place — a glob matches every build ever made here, and pacman refuses
  two files of one package with `duplicate target`. The filter drops the `quill-writer-debug` split
  package.
- `makepkg -f` rewrites the tracked `pkgver=` line. `--packagelist` reads that line, which is why the
  restore comes last; left in place, it blocks the next `git pull`.
- The version carries the commit it was built from — `quill-writer 0.1.0.r126.g92d3c1e` is
  `92d3c1e` — so `pacman -Q quill-writer` says which build is installed. Build from the checkout you
  mean to test: a branch's work is not in a package built from `main`.

`makepkg` needs the network only to fetch crates. Runtime needs `gtk4`, `enchant` and
`hunspell-en_us`; building or testing from the checkout needs `enchant` too, because Quill links
`libenchant-2` itself.

```
cargo run -p quill -- dev/ref/sample.md    # the Faces resolve from fonts/ in this tree
tools/gate check                           # what every commit must pass: format, lints, the suite
```

`tools/gate check` is the Gate's Commit tier: formatting, the rule that every `#[allow(...)]`
carries its reason on the same line, clippy on `-D warnings`, the whole suite with no display
attached, then every tool's selftest — stopping at the first failure and ending in
`gate check: pass` or `gate check: fail (<step>)`.

Data files — the six Quill Faces, Inter and Source Serif 4, their licences, and the three Style check
lists under `data/style/` — resolve from one directory: `$QUILL_DATA_DIR` if it is set, else the path
the package build compiled in (`/usr/share/quill`), else this checkout. Templates are compiled into
the binary (`quill-engine/templates/`).

## Run the Parity oracle

No framework, no build step: `dev/legacy/app/index.html` + `dev/legacy/app/css/*.css` +
`dev/legacy/app/js/*.js`, with iA Writer Duo / Quattro / Mono bundled (SIL OFL 1.1). Needs Chromium
(`/usr/bin/chromium`) and Node ≥ 20.

```
(cd dev/legacy && npm i)                  # once, for its tooling (playwright-core)

dev/legacy/bin/quill                      # own window, no browser chrome (starts the local server)
dev/legacy/bin/quill --fresh              # brand-new profile: a true first run
node dev/legacy/tools/serve.mjs 4173      # or just serve it and open http://localhost:4173/
node dev/legacy/tools/smoke.mjs           # integration check against the running server
```

## How it was judged

Nine Pieces — the page, the type, cursor & caret, focus & typewriter, dark & light, markup rendering,
chrome & menus, file handling, latency. Each got its own builder and a separate critic with fresh
context. The builder rendered ours at the exact pixel size of a cropped iA Writer screenshot
(`dev/ref/ia/shots/`, 49 captures from the App Store, Microsoft Store and ia.net, passages
transcribed in `dev/ref/ia/REFERENCE.md`), showing the same passage in the same state.
`tools/blind.mjs` shuffled the pair to `A.png`/`B.png` with the key kept outside the repo; the critic
saw only those two files, picked the one a writer would rather write in, and named the biggest gap
of each. Losing Pieces looped with that gap fed back to the builder. Latency was judged on numbers
and method instead.

| Piece | Rounds | Blind verdict | Margin |
|---|---|---|---|
| The page | 1 | ours | clear |
| The type | 1 | ours | clear |
| Cursor & caret | 1 | ours | clear |
| Focus & typewriter | 1 | ours | slight |
| Dark & light | 1 | ours | clear |
| Markup rendering | 1 | ours | clear |
| Chrome & menus | 1 | ours | slight |
| File handling | 1 | ours | clear |
| Latency | 4 | ours | — (numbers, below) |

Every round's screenshots, verdicts and gaps: `dev/progress/rounds/*.json`, `dev/shots/<piece>/`,
and the live page (`dev/progress/index.html`, generated by `tools/progress.mjs`). The native port is
judged the same way, against the oracle shot at the states in `dev/shots/oracle/states.json`.

## Latency (`dev/progress/latency-report.md`)

iA Writer publishes no latency numbers. The bar is the best native editors: ≤ 5 ms mean / ≤ 16 ms
worst app-internal (Typometer class: Notepad++ 4.3, Sublime 8.2) and Sublime's 32.5 ± 4.0 ms
keyboard-to-photon (Hume, photodiode). The legacy app measured:

- **Keystroke → committed frame: 2.43 ± 0.66 ms mean (95 % CI 2.39–2.48), 15.61 ms worst**, n = 900,
  on a 10,062-word document of real prose with capitals, punctuation, Enter, Backspace, undo and
  paste; every keystroke reconciled against Chrome's trace and an in-page probe (39,129 / 39,129).
- **Real keys through `/dev/uinput` → frame presented on Hyprland: 12.17 ± 5.01 ms**; with the
  modelled vblank wait and cited panel response, ≈ 24.5 ms keyboard-to-photon, inside the Sublime
  bracket. Two of the three photon terms are modelled: this rig has no physical panel attached.
- **Caret settles 8.1 ms after the key** (was 57 ms: a caret glide that also stole 2.7 ms of frame
  clock from every keystroke — removing it took the mean from 5.73 to 2.43 ms).
- Independent re-run at close-out (`dev/shots/latency/final-check.json`, 60 Hz, 12 regimes × 300
  keys): 1.6–2.7 ms mean per regime, worst 4.8 ms; navigation → typeable editor 76 ms with the
  10k-word document, 44 ms empty.
- **Cold launch → first frame showing the document: 370 ms median** (n = 12).

`docs/agents/gate.md` holds the native app to ≤ 5 ms mean, ≤ 16 ms worst and ≤ 250 ms cold start,
and it wins the latency Piece only by beating the oracle itself.

## Known gaps the critic still named on winning rounds

- `#` heading markers don't hang into the margin (the textarea cannot follow a per-line shift).
- Selection shows identical bars at both ends — and the native selection drops both bars on
  purpose: a fill and nothing else (`docs/design.md` § Selection, ADR 0014).
- Documents live in the browser (IndexedDB/localStorage) unless you open a folder.

Those three are the oracle's; the first and third are a native Piece's brief.
