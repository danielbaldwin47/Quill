# Which spell-check backend fits a GTK4 Rust Editor?

Research for [#6](https://github.com/danielbaldwin47/Quill/issues/6). Sources are upstream code, man pages, crates.io/GitLab metadata and `pacman` on the dev machine; each claim is cited. Retrieved 2026-08-27.

## Question

Spell check must use the system dictionaries the user already has, underline misspellings in the Editor, and offer suggestions on right-click; it is a toggle, on by default. Compare `enchant` bindings, `hunspell` bindings, `spellbook` (pure-Rust Hunspell port) and `libspelling` (GTK4-native, GNOME Text Editor) on dictionary discovery, packaging weight on Arch and Flatpak, suggestion quality, and how each hooks into the Editor. Recommend one.

## Options

| | System dictionary discovery | Suggestions | Arch weight | Flatpak (org.gnome.Platform) | Editor hook | Rust binding health | License |
|---|---|---|---|---|---|---|---|
| **enchant** | Yes — the desktop's own mechanism: user dir + `XDG_DATA_DIRS/hunspell`, provider ordering, aspell/nuspell/voikko/hspell too | Delegates to hunspell (REP/n-gram/KEY/MAP/PHONE) | 279 KiB, already installed | **In the runtime** (enchant 2.8.19), zero bundling | None — we draw the squiggle and build the menu | `enchant` 0.3.0 / `enchant-sys` 0.2.1, both **2021-06-27**, unmaintained; `enchant` has real bugs | libenchant LGPL-2.1+; crates LGPL-2.1+ |
| **hunspell direct** | **None** — constructor takes explicit `.aff`/`.dic` paths, no search, no error return | Same engine, plus `analyze`/`stem`/`generate` | 1.63 MiB | In the runtime via freedesktop-sdk (1.7.3) | None | `hunspell-rs` 0.4.0 / `hunspell-sys` 0.3.1, **2022-09/10**, unmaintained; needs libclang at build time | MPL-1.1 / GPL-2+ / LGPL-2.1+; crates permissive |
| **spellbook** | **None** — `Dictionary::new(aff: &str, dic: &str)`, `no_std`, we locate and read the files | Full Nuspell-equivalent suggester, minus `PHONE` | Zero runtime deps (2 crates) | Reads the runtime's `/usr/share/hunspell` fine | None | **0.4.2, 2026-06-03, actively maintained**, 15 reverse deps | MPL-2.0 |
| **libspelling** | Via enchant | Via enchant | 287 KiB **+ 5.8 MiB gtksourceview5** | **Not in the runtime** — must be bundled | Squiggle + correction menu, **only for a `GtkSourceBuffer`** | `libspelling` 0.5.0 (2026-05-24), official GNOME bindings, healthy | LGPL-2.1+ |

## Findings

### The user may have no dictionary at all

On this Arch machine: `enchant 2.8.15-2` and `hunspell 1.7.3-1` are installed, `hunspell-en_us` and `libspelling` are not. `/usr/share/hunspell` and `/usr/share/myspell` do not exist, and `enchant-lsmod-2 -list-dicts` returns nothing. `expac -S '%n: %D' | grep hunspell-dictionary` finds **zero** hard dependencies on Arch — only Firefox lists one as an optdepend. `hunspell-en_us` is 2.13 MiB installed (558 KiB download); `hunspell-en_gb` is 13.5 MiB.

This is a backend-independent problem: every option here needs a dictionary present. It is a PKGBUILD decision (`depends` vs `optdepends` on `hunspell-en_us`) plus a graceful "no dictionary" state in the Editor, not a reason to pick one library over another.

The Flatpak side is the reverse. `org.gnome.Platform` pulls freedesktop-sdk's `components/dictionaries.bst` (LibreOffice `libreoffice-26.8.0.3`), which installs to `/usr/share/hunspell` and symlinks `/usr/share/myspell`. English lands directly in `/usr/share/hunspell`; **every other language goes to `/usr/share/locale/<lang>/hunspell/` with a symlink**, and `share/locale` is split into the `org.gnome.Platform.Locale` extension — so non-English dictionaries exist only if the user's locale subset includes that language, and the symlinks dangle otherwise (`files/dictionaries/install_dicts.py:103`). enchant degrades correctly (`s_fileExists` uses `G_FILE_TEST_EXISTS`, which follows symlinks). Any hand-rolled discovery must replicate that check.

### enchant: discovery is the whole product

Ordering files, in increasing precedence (`lib/provider.vala:42-59`, `lib/broker.vala:199-206`): `/usr/share/enchant-2/enchant.ordering`, `/etc/enchant-2/enchant.ordering`, then `$ENCHANT_CONFIG_DIR` or `~/.config/enchant/enchant.ordering`. Arch ships `*:hunspell,nuspell,aspell` with `en_US:aspell,hunspell,nuspell` — so on Arch, **an aspell dictionary satisfies en_US before a hunspell one does**, without any code change on our side.

Hunspell dictionary directories (`providers/enchant_hunspell.cpp:211-221`) are exactly: `~/.config/enchant/hunspell`, then `$dir/hunspell` for each `XDG_DATA_DIRS` entry. A tag matches only when both `<tag>.dic` and `<tag>.aff` exist. Two things that are widely believed and are false: `/usr/share/myspell/dicts` is **not** searched (Debian works only because its dictionaries land in `/usr/share/hunspell`), and `DICPATH` does **not** work with enchant — `NEWS` for 2.6.6 says so explicitly, and `--with-hunspell-dir` was removed in 2.6.5 "along with all the other `--with-PROVIDER-dir` options, which did nothing".

Personal wordlists come free and persistent (`man 5 enchant`; `lib/dict.vala:46-56,178-228`): `~/.config/enchant/en_US.dic` for added words, `~/.config/enchant/en_US.exc` for words the dictionary accepts but the user rejects. `enchant_dict_add()` writes the file; `enchant_dict_add_to_session()` does not. The format is documented as compatible with Hunspell/Nuspell/Ispell, so a user's existing `~/.hunspell_en_US` merges in. That covers both "Add to Dictionary" and an "Ignore permanently" the other options have no equivalent for. One trap: `enchant_dict_store_replacement()` has an **empty body** (`lib/dict.vala:241`) — do not build a "learn my corrections" feature on it.

Missing backend `.so` files produce stderr warnings, not failures — on this machine four such warnings appear before the one working provider. A GUI app must suppress or capture that noise.

### The enchant crates are the weak link, not enchant

`enchant-sys` 0.2.1 is four lines of `pkg_config` plus ~120 lines of hand-written `extern "C"` against opaque types — no bindgen, no libclang. That is why a five-year-old crate still links against enchant 2.8.15. `skyspell` 6.0.0 shipped 2026-08-20 still depending on `enchant ^0.3.0`, so it does work.

The safe `enchant` 0.3.0 wrapper has genuine defects. `is_added` and `is_removed` call the FFI function and **discard the `c_int`**, returning `()` — both are unusable as written. `Broker`/`Dict` hold an `Rc`, so they are `!Send`: the checker cannot move to a worker thread, which matters directly for our keystroke latency budget. Four useful functions are absent from the FFI, including `enchant_dict_get_extra_word_characters` and `enchant_dict_is_word_character` — the per-dictionary apostrophe/hyphen rules a prose tokenizer wants.

All three problems are fixed by depending on `enchant-sys` (or inlining its `extern "C"` block) and writing our own ~200-line safe wrapper against a frozen C ABI.

### hunspell direct buys nothing we need

The C++ API is `Hunspell(const char* affpath, const char* dpath)` with **no search path, no language-tag resolution and no error return**. A wrong path prints `Hash Manager Error` to stderr and yields a live handle where every word is misspelled (`hashmgr.cxx:102-109`), and `hunspell-rs::new()` returns `Hunspell`, not `Result` — a silent-failure trap. The library has no wordlist persistence at all; `-p` is a feature of the `hunspell(1)` tool, not the library. `hunspell-sys` runs bindgen 0.61 at build time, so **libclang is required on every build machine**, including the Flatpak build; the alternative `bundled` feature (on by default in `hunspell-rs`) statically links a vendored **hunspell 1.7.1** while a fresher 1.7.3 sits unused in the runtime. Route B means reimplementing `enchant_hunspell.cpp` and losing the other providers.

### spellbook is the healthiest crate and does suggest

Correcting a premise in the ticket: spellbook is **not** check-only. `Dictionary::suggest` has existed since v0.2.0 and the README states it "should behave the same as Nuspell's `suggest`" minus phonetic suggestions. `src/suggester.rs` is 1,489 lines implementing REP edits, MAP/diacritic swaps, KEY neighbours, TRY substitution, splitting and ngram similarity; `add`/`remove_stem` give an in-memory personal dictionary. It parses real `.aff`/`.dic` files. Benchmarks (`docs/compare.md`): `earth` 66 ns vs Hunspell's 212 ns; peak heap 2.19 MB vs 3.16 MB.

It is the only option here that is actually maintained — 0.4.2 on 2026-06-03, commits through 2026-07-11, 15 reverse dependencies, 39k recent downloads against enchant's 491 — and being pure Rust it is trivially `Send`, needs no pkg-config or libclang, and is MPL-2.0.

What it does not do is discovery: `Dictionary::new(aff, dic)` takes two `&str` and there is no `std::fs` in the crate. We would own path search, tag matching, the dangling-symlink check, encoding (the parser takes `&str`; many non-English dictionaries are ISO-8859-x, and spellbook's own tests pull in `encoding_rs` + `chardetng`), and wordlist file I/O. Stated risks upstream: alpha, breaking API changes expected, "some dictionaries which use complex compounding directives may work less well". It is written by a Helix maintainer but **Helix does not ship it** — integration is still an open draft PR (#15910).

### libspelling is out for our Editor

The valuable half — `SpellingTextBufferAdapter`, which maintains a region tree, applies a `GtkTextTag` with `PANGO_UNDERLINE_ERROR` and builds the five-correction `GMenuModel` — takes a **`GtkSourceBuffer`**, not a `GtkTextBuffer`. It calls `gtk_source_buffer_get_loading()` and reads `def:misspelled-word` from the GtkSourceView style scheme. Quill's Editor renders Markdown markup inline under a hard no-advance-width-change rule; it is not a GtkSourceView code buffer, so the adapter is unusable and we would be left with `SpellingChecker` (`check_word` / `list_corrections` / `add_word` / `ignore_word`) — i.e. enchant with a GObject FFI hop per word.

Also note: libspelling never installs the menu itself; the app calls `gtk_text_view_set_extra_menu()` and `gtk_widget_insert_action_group()`. And `SpellingProviderClass` lives in a non-installed private header, so a custom backend cannot be plugged in without patching the library.

Cost: on Arch it pulls `gtksourceview5` (5.8 MiB) for a 287 KiB library, and it is **not in `org.gnome.Platform`** — `elements/core-deps/libspelling.bst` is used only by GNOME OS apps, so a Flatpak must bundle it. The Rust bindings themselves are fine (`libspelling` 0.5.0, official GNOME World/Rust namespace, gtk4 0.11 / sourceview5 0.11); the blocker is the GtkSourceBuffer coupling, not the bindings.

### Licensing is a non-issue for us

ADR 0003 puts Quill under GPL-3.0-or-later. LGPL-2.1+ (enchant, libspelling) and hunspell's tri-license are all compatible with that; the usual worry about enchant's LGPL crates — its special exception runs *downward*, permitting non-LGPL spelling providers, and grants nothing to the application — does not bite a GPL-3 app. Dictionaries are separately licensed (Arch's `hunspell-en_us` is BSD-3-Clause-Modification OR LGPL-2.1); if we ever bundle one, audit per language.

### Hooking into the Editor

No option except libspelling draws anything, so the integration work is ours in all three remaining cases and does not discriminate between them:

- **If the Editor is a `GtkTextView`** — one `GtkTextTag` with `underline: PANGO_UNDERLINE_ERROR` (plus `underline-rgba`), applied over misspelled ranges as the buffer changes; `gtk_text_view_set_extra_menu()` with a `GMenu` whose corrections section is rebuilt on right-click from the word under the pointer, backed by a `GActionGroup` inserted on the widget. This is exactly what libspelling does internally and is worth copying in shape.
- **If the Editor is custom-drawn** — the underline is a `PangoAttrList` entry (`pango_attr_underline_new(PANGO_UNDERLINE_ERROR)`) on the layout for the affected byte range, or our own squiggle in the `snapshot` vfunc; the menu is a `GtkPopoverMenu` from a `GMenu` we build, positioned at the click. Word ranges come from our own tokenizer, which must skip Markup spans and code, and should respect the dictionary's extra word characters.

Either way the checker wants to be off the main thread given the ≤5 ms keystroke budget: check the visible viewport first, debounce, and never block a keystroke on a dictionary load (spellbook quotes ~100 ms to build a heavy dictionary; hunspell/enchant load lazily but comparably).

## Recommendation

**Use enchant, through our own `extern "C"` declarations rather than the `enchant` crate.**

The ticket's hard requirement is "the system dictionaries the user already has", and enchant is the only option that answers it completely: it is the mechanism the rest of the Linux desktop already uses, it honours `~/.config/enchant`, it respects the user's provider ordering (so an aspell-only user is served on Arch without a line of our code), and it hands us persistent personal-word and exclude lists that "Add to Dictionary" and "Ignore" need. Suggestion quality is hunspell's, unmodified. It is 279 KiB, already installed here, and already inside `org.gnome.Platform` with no bundling. libspelling would add the squiggle and menu for free but only for a `GtkSourceBuffer`, which our Editor will not be, and it costs 6 MiB on Arch plus a bundled module in Flatpak for a `SpellingChecker` we can call directly.

Skip the `enchant` 0.3.0 wrapper. It is unmaintained since 2021, `is_added`/`is_removed` are broken, and its `Rc` makes the checker `!Send`, which conflicts with running checks off the keystroke path. `enchant-sys` is ~120 lines of hand-written FFI against a frozen ABI; vendoring that block and writing our own safe wrapper (~200 lines) fixes all three problems, lets us expose `enchant_dict_get_extra_word_characters` and `enchant_dict_is_word_character` for tokenization, and keeps the checker on a worker thread.

Design the seam as a small internal `SpellChecker` trait — `check(word) -> bool`, `suggest(word) -> Vec<String>`, `add(word)`, `ignore(word)`, `languages()`. That is the escape hatch: if the C dependency ever becomes a burden, **spellbook** drops in behind the same trait, reading the very same `.aff`/`.dic` files, at the cost of writing ~150 lines of discovery that mirrors `enchant_hunspell.cpp`.

## Risks

- **No dictionary on a stock Arch install.** Verified here: enchant and hunspell present, zero dictionaries, `enchant-lsmod-2 -list-dicts` empty. The Editor must show a clear "no dictionary installed" state rather than underlining everything or nothing silently, and the PKGBUILD must at least `optdepends` on `hunspell-en_us` (2.13 MiB). This is the single most likely way spell check ships broken.
- **The binding is unmaintained.** Mitigated by owning the FFI, but it means we carry the wrapper. The ABI has been stable for five years, and upstream enchant is healthy (2.8.20 on 2026-08-25), so the exposure is small but real.
- **stderr noise.** libenchant emits `libenchant-WARNING` lines for every provider `.so` it cannot dlopen. Must be routed through a GLib log handler before the app ships, or it pollutes journald on every launch.
- **Non-English dictionaries in Flatpak** live behind the `.Locale` extension with dangling symlinks in `/usr/share/hunspell` when the locale subset excludes them. enchant handles this correctly; if we ever swap to spellbook, the discovery code must check symlink targets or it will construct an empty dictionary.
- **Latency.** Checking must be viewport-first, debounced and off-thread. Dictionary load is tens to ~100 ms and must never land on a keystroke. This wants a bench alongside the existing latency Gate.
- **Tokenization is ours.** Word boundaries must skip Markup characters, code spans and URLs, and honour per-dictionary word characters, or the Editor underlines its own syntax. This is more work than the backend choice itself.
- **Premise correction for downstream tickets.** spellbook *does* implement suggestions (since v0.2.0, Nuspell-equivalent minus `PHONE`); any plan written on "spellbook is check-only" needs revising. Conversely, Helix does not yet ship spellbook — its integration PR is still an open draft.
