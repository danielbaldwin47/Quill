# Shortcut precedence on Linux: GNOME HIG, then iA Writer for Windows

Quill's default shortcuts come from a fixed precedence: the GNOME Human Interface Guidelines for
anything they name (file operations, zoom, find, full screen, settings), then iA Writer for Windows,
then iA Writer for Mac with `⌘` read as `Ctrl`, then the JavaScript app. A Command earns a default
chord only if a writer flips it mid-sentence (Focus, Typewriter, Library, Preview, theme, text size,
save); everything else lives in its menu and the Palette. `Super` chords are the compositor's and
`Ctrl+Alt` chords the desktop's, so neither is ever a default. The table this rule produced is
`docs/shortcuts.md`. Settled in [#33](https://github.com/danielbaldwin47/Quill/issues/33).

## Considered options

Following iA Writer for Windows first would put the Library on `Ctrl+E` and Preview on `Ctrl+R` (kept,
since GNOME names neither) but leaves Export, Syntax highlight, Style check, Stats, Typewriter and text
size with no chord at all and dark mode on `Alt+Shift+N`; a Linux app that ignores `Ctrl+,`, `F11`,
`Ctrl+Q` and `Ctrl+0` reads as a port. Following GNOME alone would label the Library `F9`, which a
writer arriving from iA never finds. Keeping the JavaScript app's set verbatim would carry its
`Ctrl+Shift+L` collision (theme and Library both) and its `Ctrl+Alt+N` and `Ctrl+Alt+↑/↓`, which GNOME
reserves. Giving every Command a chord, as an editor would, spends memorable chords on toggles set
once a year and makes Spell check something a writer can switch off by accident.

## Consequences

**Aliases.** Where iA or GNOME uses a different chord for the same Command, a second, unlabelled chord
fires it (`F9` beside `Ctrl+E`, `Alt+Shift+N` beside `Ctrl+Shift+L`, `Ctrl+Shift+P` beside `Ctrl+K`);
the menu shows only the first.

**Rebinding.** Every Command is rebindable from a `[shortcuts]` table in `settings.toml`
([ADR 0010](0010-settings-in-toml-under-xdg.md)); an entry replaces the defaults, the file is
watched, and the `Ctrl+?` window shows what is in force. No shortcut editor in this effort.

**Reserved chords.** Chords GNOME or iA give to Commands Quill does not ship yet (print, heading
level, bold and italic, find and replace, copy as HTML) are held in the table so no later spec spends
them on something else.
