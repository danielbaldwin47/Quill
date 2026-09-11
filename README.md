# Quill

A long-form writing environment for Linux, built to beat iA Writer and judged blind against iA Writer's
own screenshots until a harsh critic picked ours for every piece.

The app that won those rounds is a plain web app, and it is still here, in `legacy/`. It is now the
**Parity oracle**: a native GTK4 app in Rust is being ported Piece by Piece at the root of this repo,
and every Piece is judged blind before it lands (`docs/agents/gate.md`) — against the Parity oracle,
or where `docs/design.md` says so against the **Design oracle**, iA Writer for Mac as measured in
`ref/ia/mac-native/`. `docs/architecture.md` is the native design; `CONTEXT.md` is the vocabulary.

## Build, install and run the native app

```
makepkg -f                                # builds the Rust workspace from this checkout
sudo pacman -U "$(makepkg --packagelist | grep -v '/quill-debug-')"
git restore PKGBUILD                      # makepkg rewrote pkgver; put it back

quill                                     # an empty Document
quill ref/sample.md                       # a Document open
```

`--packagelist` names the package *this* checkout builds, so an older build left in the directory
cannot be installed in its place — a glob matches every build ever made here, and pacman refuses two
files of one package with `duplicate target`. The filter drops the `quill-debug` split package. And
`makepkg -f` rewrites the tracked `pkgver=` line: `--packagelist` reads that line, which is why the
restore comes last, and left in place it sits in the working tree as a local change that blocks the
next `git pull`.

The package's name carries the commit it was built from — `quill-0.1.0.r126.g92d3c1e` is `92d3c1e` —
so `pacman -Q quill` says which build is installed. Build from the checkout you mean to test: a
branch's work is not in a package built from `main`.

`makepkg` needs the network only to fetch crates; the build itself runs offline. Runtime needs `gtk4`
and `enchant`, plus `hunspell-en_us` for spell checking. The application menu launches it too: the
`.desktop` file and the icon install under the application id `io.github.danielbaldwin47.Quill`.

From the checkout, without installing:

```
cargo run -p quill -- ref/sample.md       # the Faces resolve from fonts/ in this tree
cargo test                                # the whole suite, no display attached
tools/gate check                          # what every commit must pass: format, lints, that suite
```

`tools/gate check` is the Gate's Commit tier (`docs/agents/gate.md`): formatting, the rule that every
`#[allow(...)]` carries its reason on the same line, clippy on `-D warnings`, then the whole suite with no
display in its environment — stopping at the first failure, and ending in one line the owner can read,
`gate check: pass` or `gate check: fail (<step>)`.

Data files — the six Quill Faces, Inter and Source Serif 4, their `OFL` licences, and the three
Style check lists under `data/style/` — resolve from one directory: `$QUILL_DATA_DIR` if it is set,
else the path the package build compiled in (`/usr/share/quill`), else this checkout. Templates are
not data files: they are compiled into the binary (`quill-engine/templates/`).

### Theme Quill with the desktop

On Omarchy, two steps and Quill follows `omarchy theme set` from then on, with no relaunch:

```
cp /usr/share/quill/quill.toml.tpl ~/.config/omarchy/themed/    # or packaging/quill.toml.tpl from this checkout
palette = "~/.local/state/omarchy/current/theme/quill.toml"      # the palette line in ~/.config/quill/settings.toml
```

The second is a line to set, not to append: Quill writes `settings.toml` with an empty `palette = ""`
on its first launch, and TOML refuses a key named twice. The template is in Omarchy's own contract, so
`omarchy theme set` renders it into the current theme's directory beside every other app's, writing only
the ground the theme's `mode` names; the other ground stays Quill's own.

Any tool that writes TOML can theme Quill the same way: the file holds a `[light]` and a `[dark]` table
whose keys are the twenty roles in `docs/design.md` § The palette is a file (`paper`, `ink`, `accent`,
…) and whose values are `#rrggbb` or `#rrggbbaa`; whatever it leaves out stays the built-in, and
`quill --theme light|dark` shows the built-in ground whatever the file says.

## Run the Parity oracle

The JavaScript app: no framework, no build step, `legacy/app/index.html` + `legacy/app/css/*.css` +
`legacy/app/js/*.js`, with iA Writer Duo / Quattro / Mono bundled (SIL OFL 1.1, github.com/iaolo/iA-Fonts).

```
(cd legacy && npm i)                      # once, for its tooling (playwright-core)

legacy/bin/quill                          # own window, no browser chrome (starts the local server)
legacy/bin/quill --fresh                  # brand-new profile: a true first run
node legacy/tools/serve.mjs 4173          # or just serve it and open http://localhost:4173/
node legacy/tools/smoke.mjs               # integration check against the running server
```

Requires Chromium (`/usr/bin/chromium`) and Node ≥ 20.

Keys: `Ctrl+K` command palette · `Ctrl+D` focus (sentence/paragraph) · `Ctrl+T` typewriter · `Ctrl+Shift+L` dark/light ·
`Ctrl+O/S/N` open/save/new · `Ctrl+=`/`Ctrl+-` text size · Library from the top-left, or the palette.

## How it was judged

Nine pieces — the page, the type, cursor & caret, focus & typewriter, dark & light, markup rendering, chrome & menus,
file handling, latency. Each piece got its own builder and a separate critic with fresh context. The builder rendered
ours at the exact pixel size of a cropped iA Writer screenshot (`ref/ia/shots/`, 49 real captures from the App Store,
Microsoft Store and ia.net, transcribed passages in `ref/ia/REFERENCE.md`), showing the same passage in the same
state. `tools/blind.mjs` shuffled the pair to `A.png`/`B.png` with the key kept outside the repo; the critic saw only
those two files, picked the one a writer would rather write in, and named the biggest gap of each. Losing pieces
looped with that gap fed to the builder. Latency was judged on numbers and methodology instead of pixels.

| piece | rounds | blind verdict | margin |
|---|---|---|---|
| The page | 1 | ours | clear |
| The type | 1 | ours | clear |
| Cursor & caret | 1 | ours | clear |
| Focus & typewriter | 1 | ours | slight |
| Dark & light | 1 | ours | clear |
| Markup rendering | 1 | ours | clear |
| Chrome & menus | 1 | ours | slight |
| File handling | 1 | ours | clear |
| Latency | 4 | ours | — (numbers, see below) |

Every round's screenshots, verdicts and gaps: `progress/rounds/*.json`, `shots/<piece>/`, and the live page
(`progress/index.html`, generated by `tools/progress.mjs`). The native port is judged the same way, against these
numbers and against the oracle shot at the states in `shots/oracle/states.json`.

## Latency (measured, `progress/latency-report.md`)

iA Writer publishes no latency numbers. The bar is the best native editors: ≤ 5 ms mean / ≤ 16 ms worst app-internal
(Typometer class: Notepad++ 4.3, Sublime 8.2) and Sublime's 32.5 ± 4.0 ms keyboard-to-photon (Hume, photodiode).

* **App-internal, keystroke → committed frame: 2.43 ± 0.66 ms mean (95 % CI 2.39–2.48), 15.61 ms worst**, n = 900,
  10,062-word document, real prose with capitals/punctuation/Enter/Backspace/undo/paste, every keystroke reconciled
  against Chrome's trace and an independent in-page probe (39,129 / 39,129 accounted for).
* **Real keys through `/dev/uinput` → frame presented on the Hyprland compositor: 12.17 ± 5.01 ms**; adding the modelled
  vblank wait and cited panel response puts keyboard-to-photon at ≈ 24.5 ms, inside the Sublime bracket. The critic's
  caveat stands: this rig has no physical panel attached, so two of the three photon terms are modelled, not measured.
* **Caret settles 8.1 ms after the key** (was 57 ms: a caret glide animation that also stole 2.7 ms of frame-clock
  from every keystroke — removing it is what took the mean from 5.73 to 2.43 ms).
* Independent re-run at close-out (`shots/latency/final-check.json`, default 60 Hz frame clock, 12 regimes × 300 keys):
  app cost 1.6–2.7 ms mean per regime, worst 4.8 ms; navigation → typeable editor 76 ms with the 10k-word document, 44 ms empty.
* **Cold launch `legacy/bin/quill` → first frame showing the document: 370 ms median** (n = 12); page load inside a running
  browser ≈ 80 ms to a typeable editor.

Those are the numbers the native app has to beat: `docs/agents/gate.md` holds it to ≤ 5 ms mean, ≤ 16 ms worst and
≤ 250 ms cold start, and wins the latency Piece only by beating the oracle itself.

## Layout

```
quill/          the app crate: GtkApplication, window, editor, flags, harness
quill-engine/   the display-free half: text model, Markdown, Annotators, Library, settings, rendering
fonts/          the six Quill Faces, Inter and Source Serif 4 (private, loaded at startup) + their
                OFL licences
data/style/     the three Style check lists (fillers, redundancies, clichés) + SOURCES.md and the
                licence texts its sources require
tools/          the Gate: `gate check`, and its helpers — blind pairs, progress page, uinput keys,
                idle check, font build (`npm i` at the root once, for the three that drive a browser)
legacy/         the JavaScript app as it won, and the Parity oracle (bin/quill, app/, tools/, BRIEF.md, NOTES.md)
ref/ia/         iA Writer reference: screenshots, fonts, spec sheet, sources;  ref/sample.md  the test passage
                mac-native/  the Design oracle as measured; its captures are under ref/ia/shots/mac-native/
progress/       state, per-round verdicts, latency report, generated live page
shots/          every round's screenshots, blind pairs, and the states the oracle is shot at
docs/           architecture.md, adr/, agents/ (the Gate, issue tracker, triage, domain docs)
PKGBUILD        the Arch package;  packaging/  the .desktop file and the icon
```

Licences: the native app and everything at the root are GPL-3.0-or-later (`LICENSE`); `legacy/` is ISC
(`legacy/LICENSE`); the Faces and the iA Writer fonts are SIL OFL 1.1; the bundled `harper-brill`
tagger is Apache-2.0 (`packaging/harper-brill-LICENSE`); the Style check lists carry entries under
MIT, BSD-3-Clause and CC0-1.0 beside Quill's own (`data/style/SOURCES.md`).

## Known gaps the critic still named on winning rounds

* `#` heading markers don't hang into the margin (the textarea cannot follow a per-line shift).
* Selection shows identical bars at both ends; anchor and focus aren't distinguished — and the native
  selection drops both bars on purpose: a fill and nothing else (`docs/design.md` § Selection, ADR 0014).
* Documents live in the browser (IndexedDB/localStorage) unless you open a folder; the Library shows that honestly.

Those three are the oracle's; the first and third are a native Piece's brief.
