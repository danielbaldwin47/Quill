# Quill on Windows and macOS: a try-out build, not a port

Research for [#466](https://github.com/danielbaldwin47/Quill/issues/466), under the map
[#2](https://github.com/danielbaldwin47/Quill/issues/2). Collected 2026-09-18 against `main` at
`45af783`. Scope, from the owner: development stays on Linux; Windows and macOS are only for trying
the app out. So this leads with the cheapest route to a runnable build on each, and prefers
cfg-gating a Linux-only seam off over building a stand-in.

Every claim is **READ** (in this repo, at the `file:line` given), **QUOTED** or **CHECKED** (a
primary source, with its URL, fetched on the date above), or **INFERRED** (reasoned from those and
not run). Nothing here was compiled for either OS: no Windows or macOS machine was used, and no
Rust target was installed. That is the main limit on this document.

## Answer

1. **Route.** One manually triggered GitHub Actions workflow with two jobs, each uploading an
   artifact: `windows-latest` with MSYS2 (`msys2/setup-msys2`, UCRT64 packages, the GNU Rust
   toolchain), and `macos-latest` (arm64) with Homebrew. Both package sets already carry GTK
   4.24.0 and Pango 1.58.2, above the `v4_22` floor Quill asks for (`Cargo.toml:14`). The repo has
   no `.github/workflows/` today, so this is its first workflow.
2. **Cross-compiling from Linux** is possible for Windows (Fedora's `mingw64-gtk4` in a container)
   and not realistic for macOS (it needs Apple's SDK and a Darwin GTK stack nobody packages). The
   Windows runner costs the same effort and also produces the DLL bundle, so cross-compiling buys
   nothing here.
3. **Seams.** There is no web view: the Preview is Pango, so the WebKitGTK question does not
   arise. One seam stops the app crate compiling on Windows (`quill/src/fonts.rs:31`, a
   `std::os::unix` import) and the same module does nothing useful on either OS, because Pango
   does not read fontconfig there by default. Two seams are worth gating off: enchant (Spell check)
   and the settings portal (Follow System). Everything else — XDG paths, PDF export, Print, the
   file dialogs, the file watch — is cross-platform code already.
4. **A gated try-out build lacks** Spell check and Follow System, and nothing else by design.
   Preview, Export (PDF and HTML), Print, the Library, Syntax highlight and Style check all stay.
5. **Effort.** About one Ticket-sized change (a portable font loader, two cfg gates, one test
   gate) plus one workflow file. INFERRED, since nothing was built.
6. **The Settings window** is unconstrained as it stands: a plain `gtk::Window` with a title and
   no header bar gets the native frame on both OSs. The constraints only bite if chrome adopts a
   `GtkHeaderBar` (§ Chrome).
7. **The Gate's harness is Linux-only** (Hyprland, uinput). No judged shot, `gate keys` or
   `gate bench` runs on either OS; a try-out build is looked at by hand.

## Build routes

### Windows

QUOTED from the gtk-rs book, <https://gtk-rs.org/gtk4-rs/stable/latest/book/installation_windows.html>,
which gives three routes:

| Route | Rust toolchain | GTK comes from |
|---|---|---|
| gvsbuild | `stable-msvc` | `gvsbuild build gtk4`, then `PKG_CONFIG_PATH`, `Path` and `Lib` pointed at `C:\gtk-build\gtk\x64\release` |
| Manual MSVC build | `stable-msvc` | `meson setup builddir --prefix=C:/gnome …` on GTK's own tree |
| MSYS2 | `stable-gnu` | `pacman -S mingw-w64-x86_64-gtk4 mingw-w64-x86_64-gettext mingw-w64-x86_64-libxml2 mingw-w64-x86_64-librsvg mingw-w64-x86_64-pkgconf mingw-w64-x86_64-gcc` |

gtk.org names the same two sources, MSYS2 and gvsbuild, and gives the UCRT package name,
`mingw-w64-ucrt-x86_64-gtk4` (QUOTED, <https://www.gtk.org/docs/installations/windows>). It says
nothing about cross-compiling.

**MSYS2 is the route for Quill**, for a reason specific to Quill: the two C libraries Quill links by
hand are MSYS2 packages too. CHECKED against the MSYS2 package API
(`https://packages.msys2.org/api/search?query=…`): `mingw-w64-gtk4` 4.24.0-1, `mingw-w64-pango`
1.58.2-1, `mingw-w64-enchant` 2.8.21-1, `mingw-w64-fontconfig` 2.18.3-1.

gvsbuild ships a prebuilt zip per release — `GTK4_Gvsbuild_2026.8.0_x64.zip`, 2026-08-08 (CHECKED,
`gh api repos/wingtk/gvsbuild/releases/latest`) — which its README describes as "GTK4, Cairo,
PyGObject, Pycairo, GtkSourceView5, adwaita-icon-theme, and all of their dependencies", provided
untested (QUOTED, <https://github.com/wingtk/gvsbuild>). It has `enchant.py` and `fontconfig.py`
build recipes (CHECKED, `gvsbuild/projects/`), but the README does not list either in the zip, so
the MSVC route means building enchant from source on the runner unless Spell check is gated off.
The zip's GTK version was not checked.

### macOS

QUOTED from the gtk-rs book, <https://gtk-rs.org/gtk4-rs/stable/latest/book/installation_macos.html>:
rustup, Homebrew, then `brew install gtk4 libadwaita meson desktop-file-utils`. Quill needs only
`gtk4` of those (ADR 0009), plus `enchant` if Spell check stays.

gtk.org's own page recommends gtk-osx with jhbuild and `gtk-mac-bundler` instead, and does not
mention Homebrew (QUOTED, <https://www.gtk.org/docs/installations/macos>). That is the route for
shipping an `.app` bundle, which is out of scope.

CHECKED against the Homebrew formula API (`https://formulae.brew.sh/api/formula/<name>.json`):
`gtk4` 4.24.0, `pango` 1.58.2 (depends on `fontconfig`), `enchant` 2.8.21. The macOS bottles are
arm64 only (`arm64_sequoia`, `arm64_tahoe`, `arm64_golden_gate`); there is no Intel macOS bottle,
so an Intel Mac builds GTK from source or is not served.

### Cross-compiling from Linux

- **To Windows: possible, not worth it.** Fedora packages the MinGW GTK stack: `mingw64-gtk4` is
  4.22.2 on Fedora 45 and 4.24.0 on Rawhide; Fedora 44's 4.21.0 is below Quill's floor (CHECKED,
  <https://packages.fedoraproject.org/pkgs/mingw-gtk4/mingw64-gtk4/>). `mingw64-enchant2` exists
  too (CHECKED, the package page answers 200). The build would be a Fedora container, the
  `x86_64-pc-windows-gnu` Rust target and a cross `pkg-config`. INFERRED: it works, but it is a
  container to maintain for a result the Windows runner gives directly. Arch has no MinGW GTK4 in
  its official repositories as far as this research looked (not verified beyond that).
- **To macOS: no.** INFERRED: linking needs Apple's SDK (osxcross), and no distribution packages
  GTK4 for a Darwin cross target, so the whole stack would be cross-built by hand.

### GitHub Actions

CHECKED, <https://docs.github.com/en/actions/reference/runners/github-hosted-runners>:
`windows-latest` and `windows-2025` are x64, 2 CPU, 8 GB; `macos-latest`, `macos-15` and
`macos-26` are arm64 (M1), 3 CPU, 7 GB; `macos-15-intel` and `macos-26-intel` exist.

The repo is private, so minutes are billed: Windows 2-core $0.010 per minute, macOS $0.062 per
minute, Linux 2-core $0.006 (CHECKED,
<https://docs.github.com/en/billing/reference/actions-runner-pricing>). Make the workflow
`workflow_dispatch` only, never `push`; a cold macOS build of the GTK-rs stack at ten minutes is
about $0.62.

The shape of the two jobs (INFERRED, not run):

- **Windows.** `msys2/setup-msys2` with `msystem: UCRT64` and `install:` the UCRT64 `gtk4`,
  `pkgconf`, `gcc` and `rust` packages; `cargo build --release -p quill` in the `msys2 {0}` shell.
  The artifact is `quill.exe`, every DLL `ldd` lists under `/ucrt64/bin`,
  `share/glib-2.0/schemas/gschemas.compiled`, and the repo's `fonts/` and `data/`.
- **macOS.** `brew install gtk4`, `cargo build --release -p quill`. The artifact is the bare binary
  plus `fonts/` and `data/`; the person trying it runs `brew install gtk4` themselves, because the
  binary links Homebrew's dylibs by absolute path. INFERRED: `#[link]` without a build script finds
  nothing under `/opt/homebrew/lib` on its own, so a build that keeps enchant or fontconfig needs
  `RUSTFLAGS="-L /opt/homebrew/lib"`.
- **Both.** A plain `cargo build` compiles the checkout's path in as the data directory
  (READ, `quill-engine/src/data.rs:132-136`), which on a runner is a path that will not exist on
  the tester's machine. `$QUILL_DATA_DIR` wins over it (READ, `data.rs:84-86, 120-130`), so the
  artifact carries a two-line launcher (`.cmd`, `.command`) that sets it to the unpacked folder.

## The seams

| # | Seam | Where | Try-out build |
|---|---|---|---|
| 1 | Private fonts through fontconfig | `quill/src/fonts.rs:31, 113, 207-231` | Must change: portable loader |
| 2 | enchant, linked by hand | `quill-engine/src/spell.rs:508-534` | Gate off |
| 3 | Settings portal over D-Bus | `quill/src/portal.rs:69`, `main.rs:125` | Gate off |
| 4 | Web view | none | Nothing to do |
| 5 | XDG paths | `quill-engine/src/settings/xdg.rs:24-48` | Leave |
| 6 | PDF export | `quill-engine/src/pdf.rs`, `Cargo.toml:23` | Leave |
| 7 | Print | `quill/src/print.rs:70, 146` | Leave |
| 8 | File watch | `quill-engine/src/watch.rs`, `notify` | Leave; tests need one gate |
| 9 | `cfg(unix)` and `std::os::unix` | four sites, three in tests | One test gate |
| 10 | Locale reads | `spell.rs:288-296`, `settings.rs:491` | Leave |

### 1. Private fonts (`quill/src/fonts.rs`)

READ. `fonts.rs:31` imports `std::os::unix::ffi::{OsStrExt, OsStringExt}` with no cfg, so **the app
crate does not compile for Windows**; it is the only such line outside test code in either crate.
`fonts.rs:207-231` is a hand-written `#[link(name = "fontconfig")] extern "C"` block, and
`load_private` (`:113`) hands the Faces directory to `FcConfigAppFontAddDir`, then asks fontconfig
to match each family (`:125-136`). `main.rs:115` reports a failure and carries on.

What breaks at run time, on both OSs: Pango only reads fontconfig when fontconfig is its backend.
`pangocairo-fontmap.c` picks CoreText first on macOS and win32 first on Windows, and falls to `fc`
only when `PANGOCAIRO_BACKEND` says so (CHECKED,
<https://raw.githubusercontent.com/GNOME/pango/main/pango/pangocairo-fontmap.c>, lines 77-90); GTK
itself builds against `pangowin32` on Windows (CHECKED, GTK `meson.build:937`). So even where
fontconfig links, the directory goes to a library nobody is reading, the Faces are absent, and the
Editor renders in a system font. The Faces are the point of the app, so this seam is the one that
cannot simply be gated off.

Cheapest fix: `pango_font_map_add_font_file`, "Loads a font file with one or more fonts into the
`PangoFontMap`", since Pango 1.56, with no backend restriction documented (QUOTED,
<https://docs.gtk.org/Pango/method.FontMap.add_font_file.html>). The `pango` crate Quill already
uses binds it as `FontMapExt::add_font_file` behind its `v1_56` feature (READ,
`pango-0.22.8/src/auto/font_map.rs:22-25`). Under `#[cfg(not(target_os = "linux"))]`,
`load_private` becomes a loop over `data::FACES` and `data::FAMILIES` calling it on
`pangocairo::FontMap::default()`, and the fontconfig block moves under `#[cfg(target_os = "linux")]`.
About twenty lines. INFERRED: untested on either OS, and whether the per-file call honours Quill's
"each Italic is a family of its own" naming (`fonts.rs:19-22`) the way fontconfig does is the first
thing to look at in a try-out build.

The zero-code alternative is launching with `PANGOCAIRO_BACKEND=fc` on macOS, where `fonts.rs`
already compiles. INFERRED and riskier: it swaps the font backend under all of GTK.

### 2. enchant (`quill-engine/src/spell.rs`)

READ. `spell.rs:508` is `#[link(name = "enchant-2")]` on a hand-declared `extern "C"` block (no
build script, no `pkg-config`; the module doc at `:5-8` says so), so **the engine needs
`libenchant-2` at link time on every OS**. Both package managers have it (§ Build routes), so it
can link. It is still the seam to gate off, because linking is the small half: on Windows a bundled
enchant also needs a provider DLL and dictionaries nobody installs, so Spell check would find "No
dictionary installed" anyway (INFERRED).

The gate: put `mod ffi`, `Broker` and the body of `Enchant` under `#[cfg(target_os = "linux")]`
(or a default-on cargo feature `spell`), and give the other side `Enchant::new` returning `None`
and `installed_languages()` returning an empty `Vec`. Only three call sites reach them:
`quill-engine/src/worker.rs:296`, `quill/src/settings.rs:312` and `quill/src/editor.rs:993`. The
app already has the empty state: the Settings row reads `No dictionary installed`
(`quill/src/settings.rs:72, 602`). `main.rs:65-96` (the `libenchant` log handler) is harmless
either way. The tests under `quill-engine/tests/spell_*.rs` need the same cfg.

**Lacks: Spell check, entirely.**

### 3. The settings portal (`quill/src/portal.rs`)

READ. It is `gio` D-Bus, not a D-Bus crate (`portal.rs:23-25`), so it compiles everywhere `gio`
does. `Portal::open` (`:69`) calls `gio::bus_get_sync(BusType::Session)`, which the module leaves
unbounded on purpose (`:50-55`); every failure is `None` (`:10-16`) and the launch falls back to
`last_scheme`.

What differs: on Windows GLib answers a session-bus request by autolaunching one —
`gdbusaddress.c:1208-1213` calls `_g_dbus_win32_get_session_address_dbus_launch`, and
`gdbus-tool.c:2604` is the `g_win32_run_session_bus` it spawns (CHECKED,
<https://raw.githubusercontent.com/GNOME/glib/main/gio/gdbusaddress.c>). So an ungated Windows
launch starts a bus daemon to ask a portal that is not there. On macOS the launchd lookup is a
`TODO` (`gdbusaddress.c:1249`), so the call just fails.

The gate: `#[cfg(not(target_os = "linux"))]` on a `Portal::open` that returns `None`. The tests
(`portal.rs:196` on) use `gio::TestDBus`, which wants a `dbus-daemon`; they take the same cfg.

There is no stand-in to build, because GTK has none either: the macOS GDK backend reports
`gtk-interface-reduced-motion`, `gtk-xft-dpi`, `gtk-font-name` and a few more, and no colour
scheme (CHECKED, `gdk/macos/gdkmacosdisplay-settings.c:133-176`), and a code search of GNOME/gtk
for `gtk-interface-color-scheme` finds it under `gdk/wayland` and `gdk/android` only (CHECKED,
`gh api search/code`).

**Lacks: Follow System.** The writer picks a ground by hand, which the Settings window already
allows.

### 4. The Preview's web view

There is none. READ: `grep -ri webkit` over both crates and `Cargo.lock` finds nothing, and
`quill/src/preview.rs:1-10` says what the Preview is — "a scrollable sheet that snapshots the
page's Pango layouts", laid out by `quill_engine::render`. WebKitGTK having no Windows port
therefore costs Quill nothing, and the Preview needs no gate. It is worth holding on to: a later
move to a web view for the Preview would be the one change that ends Windows try-outs.

### 5. XDG paths (`quill-engine/src/settings/xdg.rs`)

READ. `xdg.rs:24-48`: `$XDG_CONFIG_HOME` or `~/.config`, `$XDG_STATE_HOME` or `~/.local/state`,
through `std::env::home_dir()`. INFERRED: on Windows that is `%USERPROFILE%\.config\quill`, on
macOS `~/.config/quill`. Unidiomatic on both and entirely functional; leave it. The atomic write
(`settings/file.rs:120-127`) is `fs::set_permissions` then `fs::rename` over the target, both of
which `std` implements on Windows.

### 6. PDF export (`quill-engine/src/pdf.rs`)

READ. The surface is cairo's own PDF backend (`Cargo.toml:23`, features `pdf` and `v1_16`), which
is part of cairo on every OS; the Homebrew and MSYS2 cairo builds were not inspected for it
(INFERRED: on, as it is cairo's default). Only the tests shell out — `pdfinfo`, `pdftotext`,
`pdftohtml`, `pdftoppm` at `pdf.rs:144-307` — and `:144` already skips when poppler is absent.
The exported page depends on seam 1: without the Faces loaded the PDF is set in a fallback font.

### 7. Print (`quill/src/print.rs`)

READ. `gtk::PrintOperation` at `print.rs:70`, run with `PrintDialog` at `:146`. CHECKED in GTK's
`gtk/print/meson.build`: Windows has its own `gtkprintoperation-win32.c`; every other OS, macOS
included, gets the Unix dialog (`os_unix = not os_win32`, GTK `meson.build:156`). INFERRED: it
opens the native dialog on Windows and GTK's own dialog over CUPS on macOS, and Quill's custom tab
may be placed differently in the Windows dialog. No gate.

### 8. The file watch (`quill-engine/src/watch.rs`, `notify`)

READ, from `cargo tree -p quill-engine --target …`: `notify` 8.2 resolves to `inotify` on Linux,
`windows-sys` on Windows and `fsevent-sys` on macOS. It builds on all three. INFERRED: the rules in
`watch.rs` were written against inotify's event shapes (a save as a rename over the top, a
directory swapped by symlink at `:771-813`), and FSEvents and `ReadDirectoryChangesW` report
renames differently, so Reload-on-outside-save and the Library's live tree are where a try-out
build is most likely to misbehave without failing to build. The Omarchy theme watch has nothing to
watch on either OS.

### 9. `cfg(unix)` and `std::os::unix`

READ, all four sites:

- `quill/src/fonts.rs:31` — live code, seam 1.
- `quill-engine/src/disk.rs:801-804` — a test, already `#[cfg(unix)]`.
- `quill-engine/src/watch.rs:540` — `use std::os::unix::fs::symlink;` at the top of `mod tests`,
  not gated, so **the engine's tests do not compile on Windows**. The symlink tests (`:807-813`)
  and the `:723` permissions test want `#[cfg(unix)]`. macOS is unix and unaffected.
- `quill-engine/src/watch.rs:723` — `PermissionsExt` inside one test.

A try-out build runs `cargo build`, not `cargo test`, so none of the test sites block it.

### 10. Locale

READ. `spell.rs:288-296` reads `LC_ALL`, `LC_MESSAGES`, `LANG`; `settings.rs:491` reads `LC_ALL`,
`LC_PAPER`, `LANG` for the default paper. Windows sets none of them, so the paper falls to the
code's default and a writer in the US picks Letter by hand. No gate.

### Not seams

`gtk::UriLauncher` (`preview.rs:922`, `editor.rs:4346`), `gtk::AlertDialog` and `gtk::FileDialog`
are GTK's portable API. `harness.rs` sets `GtkSettings` and calls no process. No file under
`quill/src` spawns a command.

## Does `quill-engine` build on both today?

INFERRED: **yes on macOS, yes on Windows under MSYS2, given the C libraries — and its tests do not
compile on Windows.** "Display-free" is not "C-free". Its platform-bound dependencies, from
`quill-engine/Cargo.toml` and `cargo tree`:

| Dependency | Bound to | Windows (MSYS2) | macOS (Homebrew) |
|---|---|---|---|
| `cairo-rs`, `pango`, `pangocairo` (and `glib`, `gobject`, `gio` `-sys` under them) | the C libraries, found by `pkg-config` | packaged | packaged, as `gtk4` dependencies |
| `libenchant-2`, by `#[link]` in `spell.rs:508` | the C library, found by the linker's own search path | `mingw-w64-enchant` | `enchant`, plus `-L /opt/homebrew/lib` |
| `notify`, `notify-debouncer-mini` | a per-OS backend, chosen by cargo | `windows-sys` | `fsevent-sys` |
| `harper-brill`, `aho-corasick`, `pulldown-cmark`, `serde`, `toml` | nothing | — | — |

With seam 2 gated, the engine's only native needs are cairo and Pango, which GTK brings anyway.

## Chrome: what constrains the Settings window

READ. The Settings window is `gtk::Window::builder().title("Settings").transient_for(parent)`
around a `ScrolledWindow` and a `Grid` (`quill/src/settings.rs:158-171`). No file under `quill/src`
mentions `HeaderBar`, `set_titlebar`, `WindowControls`, a menubar or a named icon; the one
`set_decorated(false)` is the harness's (`window.rs:347`).

- **Decorations.** INFERRED: a window with no titlebar widget gets the OS's own frame on Windows
  and macOS, so today's windows look native at the edge with no work. The constraint is on a
  future `GtkHeaderBar`: it is client-side decoration everywhere, which on Windows means GTK-drawn
  buttons in place of the system's, and on macOS means opting in to the real traffic lights through
  `GtkHeaderBar:use-native-controls`, added in GTK 4.18 ("Headerbars can use native window controls
  on macOS", QUOTED, GTK `NEWS`; the property is documented at `gtk/gtkheaderbar.c:624-638`). A
  Settings window that keeps a plain title bar stays portable for free.
- **Widgets and CSS.** GTK draws every widget itself on every OS, so Quill's stylesheet and plain
  widgets carry over unchanged. Two things leak from the OS: `gtk-font-name` comes from the system
  (CHECKED, both GDK backends set it), so any chrome text whose CSS names no family changes face;
  and named icons need an icon theme that Homebrew's `gtk4` does not pull in (it depends on
  `hicolor-icon-theme` only). Quill uses none today. Keep it that way, or ship the glyphs as Quill
  does the tick (`chrome.rs:935`).
- **File dialogs.** `gtk::FileDialog` (`settings.rs:885`, `export_dialog.rs:713`) goes through
  `GtkFileChooserNative`, which has a win32 and a quartz implementation (CHECKED,
  `gtk/gtkfiledialog.c:876-917`, `gtk/meson.build:673, 706`). So the Add Location and Export
  pickers are the OS's own, and nothing custom can be put inside them on those OSs. The Export
  dialog's options already live in Quill's own window, which is the portable shape.
- **Shortcuts.** INFERRED: `docs/shortcuts.md` is `Ctrl` chords, and GTK on macOS does not map
  `Ctrl` to `Cmd`, so a Mac try-out is driven with `Ctrl`. A settings window that shows chords
  shows `Ctrl` there too.
- **libadwaita** is packaged on both, so ADR 0009's "a single libadwaita widget wanted later"
  stays open; it adds a DLL set to the Windows bundle and nothing else.

## The Gate

Linux-only, by construction: the shots and `gate keys` run under Hyprland and inject through
uinput, and the oracles are pixels from this machine's font rendering. Nothing of the Gate runs on
Windows or macOS, and a try-out build carries no verdict.

## The work, if it is wanted

1. Portable font loading under `cfg(not(target_os = "linux"))` with `pango` feature `v1_56`
   (seam 1).
2. `cfg` gates on enchant (seam 2) and `Portal::open` (seam 3), with their tests.
3. `#[cfg(unix)]` on the symlink and permissions tests in `watch.rs` (seam 9).
4. `.github/workflows/tryout.yml`, `workflow_dispatch` only, two jobs, two artifacts with a
   launcher that sets `QUILL_DATA_DIR`.

Steps 1-3 are one Ticket-tier change that the Linux Gate can hold green, since every gate is a
no-op on Linux. Step 4 is where the unknowns are: the first run of each job is the first time any
of this document's INFERRED lines meets a compiler.
