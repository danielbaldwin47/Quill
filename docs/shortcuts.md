# Keyboard shortcuts

The one table every menu, the Palette, the `Ctrl+?` shortcuts window and the writer's `[shortcuts]`
overrides read. A feature spec that adds a Command adds its row here; a Command absent from this file
has no shortcut, no menu home and no Palette entry. The precedence rule that produced the table is
[ADR 0011](adr/0011-shortcut-precedence-on-linux.md). Settled in
[#33](https://github.com/danielbaldwin47/Quill/issues/33).

**Reading the table.** *Id* is the Command's stable name: the TOML key, the Palette's identity, and
the GTK action name with `win.` or `app.` in front (`focus.toggle` → `win.focus.toggle`). *Default*
is the chord the menu labels. *Alias* is a second chord that fires the same Command and is never
labelled; it exists only where iA Writer or GNOME uses a different chord for the same thing. A dash
means no shortcut: the Command lives in its menu and the Palette. A Command's menu is its only menu.

## Document menu

The title button in the top bar opens it.

| Id | Title | Default | Alias |
|---|---|---|---|
| `file.new` | New Document | `Ctrl+N` | |
| `window.new` | New Window | `Ctrl+Shift+N` | |
| `file.open` | Open File… | `Ctrl+O` | |
| `file.save` | Save | `Ctrl+S` | |
| `file.saveAs` | Save As… | `Ctrl+Shift+S` | |
| `file.rename` | Rename Document… | `F2` | |
| `file.duplicate` | Duplicate Document | — | |
| `export.open` | Export… | `Ctrl+Shift+E` | |
| `window.close` | Close Window | `Ctrl+W` | |
| `app.quit` | Quit | `Ctrl+Q` | |

`export.open` opens the Export dialog. The Export spec decides whether the menu shows the three
targets beneath it as `export.pdf`, `export.html`, `export.markdown` (unbound) or one item.

## View menu

The button at the right of the top bar opens it; `F10` opens it too (GNOME's primary-menu key).
Sections in this order, separators between them.

**Focus** (the order ADR 0006 fixes: toggle, the two scope radios, then Typewriter)

| Id | Title | Default | Alias |
|---|---|---|---|
| `focus.toggle` | Enable Focus Mode / Disable Focus Mode | `Ctrl+D` | |
| `focus.sentence` | Sentence (radio) | — | |
| `focus.paragraph` | Paragraph (radio) | — | |
| `focus.swap` | Switch Focus Scope (Palette only, no menu row) | `Ctrl+Shift+D` | |
| `typewriter.toggle` | Typewriter | `Ctrl+T` | |

**Panes**

| Id | Title | Default | Alias |
|---|---|---|---|
| `library.toggle` | Show Library / Hide Library | `Ctrl+E` | `F9` |
| `preview.toggle` | Show Preview / Hide Preview | `Ctrl+R` | |
| `preview.layout` | Preview Split / Preview Full (radio pair) | reserved `Ctrl+Shift+R` | |

`preview.layout`'s binding is the Preview spec's call; the chord is reserved so nothing else takes it.

**Writing tools**

| Id | Title | Default | Alias |
|---|---|---|---|
| `syntax.toggle` | Syntax Highlight (submenu head, check) | — | |
| `syntax.nouns` … `syntax.conjunctions` | Nouns, Verbs, Adjectives, Adverbs, Conjunctions (checks in the submenu) | — | |
| `style.toggle` | Style Check | — | |
| `spell.toggle` | Spell Check | — | |

These three are set once and left; a chord they could be hit by accident on is worse than a trip to
the menu. The Palette reaches them in two keystrokes.

**Typeface**

| Id | Title | Default | Alias |
|---|---|---|---|
| `font.duo` | Duo (radio) | — | |
| `font.quattro` | Quattro (radio) | — | |
| `font.mono` | Mono (radio) | — | |

**Appearance**

| Id | Title | Default | Alias |
|---|---|---|---|
| `theme.toggle` | Dark Mode | `Ctrl+Shift+L` | `Alt+Shift+N` |
| `font.bigger` | Bigger Text | `Ctrl+=` | `Ctrl++` |
| `font.smaller` | Smaller Text | `Ctrl+-` | |
| `font.reset` | Default Text Size | `Ctrl+0` | |

**Window**

| Id | Title | Default | Alias |
|---|---|---|---|
| `chrome.stats` | Statistics (check) | — | |
| `chrome.toggle` | Hide Bars / Show Bars | `Ctrl+Shift+H` | |
| `window.fullscreen` | Full Screen | `F11` | |
| `settings.open` | Settings… | `Ctrl+,` | |
| `shortcuts.open` | Keyboard Shortcuts | `Ctrl+?` | |
| `palette.open` | All Commands… | `Ctrl+K` | `Ctrl+Shift+P` |

## Stats menu

Clicking the stats bar opens it. Radios pick what the bar shows; the last row hides the bar.

| Id | Title | Default | Alias |
|---|---|---|---|
| `stats.words` | Words (radio) | — | |
| `stats.characters` | Characters (radio) | — | |
| `stats.charactersNoSpaces` | Characters Without Spaces (radio) | — | |
| `stats.sentences` | Sentences (radio) | — | |
| `stats.paragraphs` | Paragraphs (radio) | — | |
| `stats.readingTime` | Reading Time (radio) | — | |
| `chrome.stats` | Hide Statistics | — | |

## Palette and keyboard only

No menu row; the Palette lists them under their section.

| Id | Title | Default | Alias |
|---|---|---|---|
| `library.search` | Find a Document… | `Ctrl+Shift+O` | |
| `file.next` | Next Document | `Ctrl+Page Down` | |
| `file.prev` | Previous Document | `Ctrl+Page Up` | |
| `file.follow` | Open Linked Document | `Ctrl+Enter` | |
| `file.openFolder` | Open Folder as Library… | — | |
| `file.delete` | Delete Document… | — | |
| `theme.light`, `theme.dark`, `theme.auto` | Light Theme, Dark Theme, Follow System | — | |
| `chrome.doc` | Document Menu | — | |
| `chrome.view` | View Menu | `F10` | |

`Ctrl+Shift+O` is the one "jump" chord. The Heading navigation spec, in the light of the Library
spec, decides whether it opens Documents, headings or one merged list; the id may change with it.

## Reserved chords

Chords with no Command yet. A spec that ships the Command claims the chord; nothing else may.

| Chord | For |
|---|---|
| `Ctrl+P` | Print |
| `Ctrl+1` … `Ctrl+6` | Heading level 1–6 |
| `Ctrl+B`, `Ctrl+I` | Bold, Italic |
| `Ctrl+F`, `Ctrl+H`, `Ctrl+G`, `Ctrl+Shift+G` | Find, Find and Replace, Next match, Previous match |
| `Ctrl+Shift+C` | Copy as HTML |
| `Ctrl+Shift+R` | Preview Split / Full |

## Off-limits chords

`GtkTextView` handles these before a window shortcut sees the key, so a Command bound to one is
dead: `Ctrl+A`, `Ctrl+X`, `Ctrl+C`, `Ctrl+V`, `Ctrl+Shift+V`, `Ctrl+Z`, `Ctrl+Shift+Z`, `Ctrl+←`,
`Ctrl+→`, `Ctrl+Shift+←`, `Ctrl+Shift+→`, `Ctrl+Backspace`, `Ctrl+Delete`, `Ctrl+Shift+U`, `Ctrl+.`,
`Ctrl+;`, `Shift+F10`, `Menu`. Undo, redo and the clipboard are GTK's own and appear in no menu of
Quill's. `Super` chords belong to the compositor and `Ctrl+Alt` chords to the desktop; neither is
ever a default and the rebinding rules below refuse them.

## Rebinding

A `[shortcuts]` table in `settings.toml` (ADR 0010) maps a Command id to a list of chords in GTK
accelerator syntax:

```toml
[shortcuts]
"library.toggle" = ["F9", "<Control>e"]   # first entry is the one the menu labels
"theme.toggle"   = []                      # unbound
"spell.toggle"   = ["<Control><Shift>k"]   # a chord for a Command that has none by default
```

- An entry **replaces** the Command's default list, aliases included; an empty list unbinds it.
- Every Command in this file is rebindable, the unbound ones too. Reserved chords are not ids; a
  chord becomes bindable to a Command when a spec ships that Command.
- Refused, with one log line naming the entry and the reason: an unknown id, a chord that does not
  parse, a chord of the right shape naming a key the keyboard has none of, an off-limits chord, a
  `Super` or `Ctrl+Alt` chord, and a chord already taken earlier in the file (first entry keeps it).
  A refused entry leaves the default in place, whichever of these refused it and however many of its
  chords were fine: the whole entry goes, never part of it. The same line is shown at the foot of the
  Settings window (`Ctrl+,`), as is a file that is not TOML at all — one `[shortcuts]` header is the
  table; a second header with the same name is what makes a file not TOML, and Quill writes none of
  its own until there is an entry to put under it.
- Menu labels, the Palette and the `Ctrl+?` window show the effective bindings, never the defaults.
- The settings file is watched; a saved edit applies without a restart.
- The Settings window has one row, "Keyboard shortcuts: edit settings.toml", that opens the file in
  the system editor. A capture-a-keystroke editor is a later effort.
