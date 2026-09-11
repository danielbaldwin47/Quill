//! The Command registry: `docs/shortcuts.md` as data, one [`Command`] per id.
//!
//! The table is the source and this module is its copy: a test parses the
//! file and refuses any disagreement, so a row edited in one place and not
//! the other goes red on the next `tools/gate check`. Chords are written here
//! as the table writes them (`Ctrl+Shift+L`); [`accel`] turns one into the
//! syntax GTK installs, and [`by_chord`] answers which Command a chord
//! reaches. [`RESERVED`] and [`OFF_LIMITS`] are the table's two chord lists
//! that bind nothing, held so the settings validator can refuse them.

/// Which action map a Command's action lives on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    /// On the window, as `win.<id>`.
    Win,
    /// On the application, as `app.<id>`.
    App,
}

impl Scope {
    /// The prefix GTK action names carry for this scope.
    pub fn prefix(self) -> &'static str {
        match self {
            Scope::Win => "win",
            Scope::App => "app",
        }
    }
}

/// One of the three menus a Command's row can be in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Menu {
    /// The title button's menu.
    Document,
    /// The View button's menu, `F10`.
    View,
    /// The stats bar's menu.
    Stats,
}

impl Menu {
    /// The menu's name, as `docs/shortcuts.md` heads its table and the
    /// `Ctrl+?` window titles the section holding it
    /// ([`crate::shortcuts::sections`]).
    #[must_use]
    pub const fn title(self) -> &'static str {
        match self {
            Self::Document => "Document",
            Self::View => "View",
            Self::Stats => "Stats",
        }
    }
}

/// The three menus in the table's order, which is the order the `Ctrl+?`
/// window puts their sections in.
pub const MENUS: [Menu; 3] = [Menu::Document, Menu::View, Menu::Stats];

/// How a Command's action holds state, which is how its menu row is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Fires and holds nothing.
    Plain,
    /// A boolean state; the row carries a check.
    Check,
    /// One value of a group: the group is a stateful string action named
    /// `group`, and activating this Command sets it to `value`.
    Radio {
        /// The group action's name, a settings key where one exists.
        group: &'static str,
        /// The value this member sets.
        value: &'static str,
    },
}

/// A row a Command has in a menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Placement {
    /// The menu the row is in.
    pub menu: Menu,
    /// The View menu's section the row is in; the other menus have none.
    pub section: Option<&'static str>,
    /// The row's label, which is the Command's title except where the table
    /// gives the row its own.
    pub label: &'static str,
}

/// One row of `docs/shortcuts.md`, or one id of a row that carries several.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Command {
    /// The stable name: the TOML key, the Palette's identity, the action name
    /// after its scope's prefix.
    pub id: &'static str,
    /// What the Palette calls it, and the menu row's label unless the
    /// placement says otherwise.
    pub title: &'static str,
    /// Where the action lives.
    pub scope: Scope,
    /// How the action holds state.
    pub kind: Kind,
    /// The chord the menu labels first, then the aliases that fire it and
    /// are never labelled, all in the table's syntax.
    pub chords: &'static [&'static str],
    /// Its menu rows; empty for a Palette-only Command.
    pub placements: &'static [Placement],
    /// Whether the feature behind it is built: an unbuilt Command is
    /// registered disabled, so its chord does nothing and its row is grey.
    pub built: bool,
}

impl Command {
    /// The action's name on its map: the id, except that `app.quit` is
    /// `quit` on the application, whose prefix its id already carries.
    pub fn name(&self) -> &'static str {
        let prefix = self.scope.prefix();
        match self
            .id
            .strip_prefix(prefix)
            .and_then(|rest| rest.strip_prefix('.'))
        {
            Some(rest) => rest,
            None => self.id,
        }
    }

    /// The action name with its scope in front, as an accelerator or a menu
    /// row names it: `win.focus.toggle`, `app.quit`, `app.window.new`.
    pub fn action(&self) -> String {
        format!("{}.{}", self.scope.prefix(), self.name())
    }

    /// What a row or a Palette entry activates: a radio member fires its
    /// group's action (`win.face`) with its value as the target, and any
    /// other Command its own action with none.
    pub fn action_and_target(&self) -> (String, Option<&'static str>) {
        match self.kind {
            Kind::Radio { group, value } => {
                (format!("{}.{group}", self.scope.prefix()), Some(value))
            }
            Kind::Plain | Kind::Check => (self.action(), None),
        }
    }

    /// The chord the menu labels, in the table's syntax.
    pub fn default(&self) -> Option<&'static str> {
        self.chords.first().copied()
    }

    /// The chords that fire it and are never labelled.
    pub fn aliases(&self) -> &'static [&'static str] {
        self.chords.get(1..).unwrap_or(&[])
    }

    /// The default followed by the aliases, in the table's syntax.
    pub fn chords(&self) -> impl Iterator<Item = &'static str> {
        self.chords.iter().copied()
    }

    /// The same chords in GTK's accelerator syntax, the default first.
    ///
    /// A chord the converter does not know is left out rather than installed
    /// wrong; the registry test holds every chord in the table to converting.
    pub fn accels(&self) -> Vec<String> {
        self.chords().filter_map(accel).collect()
    }

    /// Whether the Command has no menu row: the table's "Palette only" ids.
    pub fn hidden(&self) -> bool {
        self.placements.is_empty()
    }
}

const fn place(menu: Menu, section: Option<&'static str>, label: &'static str) -> Placement {
    Placement {
        menu,
        section,
        label,
    }
}

const fn row(
    id: &'static str,
    title: &'static str,
    scope: Scope,
    kind: Kind,
    chords: &'static [&'static str],
    placements: &'static [Placement],
    built: bool,
) -> Command {
    Command {
        id,
        title,
        scope,
        kind,
        chords,
        placements,
        built,
    }
}

const DOC: Menu = Menu::Document;
const VIEW: Menu = Menu::View;
const STATS: Menu = Menu::Stats;

/// The View menu's sections in the table's order, separators between them.
pub const VIEW_SECTIONS: [&str; 7] = [
    "Focus",
    "Panes",
    "Writing tools",
    "Typeface",
    "Template",
    "Appearance",
    "Window",
];

/// Every Command, in the table's order: the Document menu, the View menu
/// section by section, the Stats menu, then the Palette-only rows.
///
/// `chrome.stats` sits where the table first names it, under View › Window,
/// and carries its Stats menu row as a second placement.
#[rustfmt::skip]
pub const COMMANDS: &[Command] = &[
    // Document menu.
    row("file.new", "New Document", Scope::Win, Kind::Plain, &["Ctrl+N"], &[place(DOC, None, "New Document")], true),
    row("window.new", "New Window", Scope::App, Kind::Plain, &["Ctrl+Shift+N"], &[place(DOC, None, "New Window")], true),
    row("file.open", "Open File…", Scope::Win, Kind::Plain, &["Ctrl+O"], &[place(DOC, None, "Open File…")], true),
    row("file.save", "Save", Scope::Win, Kind::Plain, &["Ctrl+S"], &[place(DOC, None, "Save")], true),
    row("file.saveAs", "Save As…", Scope::Win, Kind::Plain, &["Ctrl+Shift+S"], &[place(DOC, None, "Save As…")], true),
    row("file.rename", "Rename Document…", Scope::Win, Kind::Plain, &["F2"], &[place(DOC, None, "Rename Document…")], true),
    row("file.duplicate", "Duplicate Document", Scope::Win, Kind::Plain, &[], &[place(DOC, None, "Duplicate Document")], true),
    row("export.pdf", "PDF…", Scope::Win, Kind::Plain, &["Ctrl+Shift+E"], &[place(DOC, None, "PDF…")], true),
    row("export.html", "HTML…", Scope::Win, Kind::Plain, &[], &[place(DOC, None, "HTML…")], true),
    row("export.markdown", "Markdown…", Scope::Win, Kind::Plain, &[], &[place(DOC, None, "Markdown…")], true),
    row("export.quick", "Quick Export PDF", Scope::Win, Kind::Plain, &["Ctrl+Shift+P"], &[place(DOC, None, "Quick Export PDF")], true),
    row("export.copyHtml", "Copy as HTML", Scope::Win, Kind::Plain, &["Ctrl+Shift+C"], &[place(DOC, None, "Copy as HTML")], true),
    row("print", "Print…", Scope::Win, Kind::Plain, &["Ctrl+P"], &[place(DOC, None, "Print…")], true),
    row("window.close", "Close Window", Scope::Win, Kind::Plain, &["Ctrl+W"], &[place(DOC, None, "Close Window")], true),
    row("app.quit", "Quit", Scope::App, Kind::Plain, &["Ctrl+Q"], &[place(DOC, None, "Quit")], true),
    // View › Focus.
    row("focus.toggle", "Enable Focus Mode / Disable Focus Mode", Scope::Win, Kind::Check, &["Ctrl+D"], &[place( VIEW, Some("Focus"), "Enable Focus Mode / Disable Focus Mode", )], true),
    row("focus.sentence", "Sentence", Scope::Win, Kind::Radio { group: "focus_scope", value: "sentence", }, &[], &[place(VIEW, Some("Focus"), "Sentence")], true),
    row("focus.paragraph", "Paragraph", Scope::Win, Kind::Radio { group: "focus_scope", value: "paragraph", }, &[], &[place(VIEW, Some("Focus"), "Paragraph")], true),
    row("focus.swap", "Switch Focus Scope", Scope::Win, Kind::Plain, &["Ctrl+Shift+D"], &[], true),
    row("typewriter.toggle", "Typewriter", Scope::Win, Kind::Check, &["Ctrl+T"], &[place(VIEW, Some("Focus"), "Typewriter")], true),
    row("live.toggle", "Live", Scope::Win, Kind::Check, &["Ctrl+L"], &[place(VIEW, Some("Focus"), "Live")], true),
    // View › Panes.
    row("library.toggle", "Show Library / Hide Library", Scope::Win, Kind::Check, &["Ctrl+E", "F9"], &[place(VIEW, Some("Panes"), "Show Library / Hide Library")], true),
    row("preview.full", "Preview Full", Scope::Win, Kind::Check, &["Ctrl+R"], &[place(VIEW, Some("Panes"), "Preview Full")], true),
    row("preview.split", "Preview Split", Scope::Win, Kind::Check, &["Ctrl+Shift+R"], &[place(VIEW, Some("Panes"), "Preview Split")], true),
    // The two modes the pane draws in are one radio group whose value is the
    // `[preview]` table's `mode`; the two layouts above are its checks.
    row("preview.web", "Web", Scope::Win, Kind::Radio { group: "preview_mode", value: "web", }, &[], &[place(VIEW, Some("Panes"), "Web")], true),
    row("preview.pdf", "PDF", Scope::Win, Kind::Radio { group: "preview_mode", value: "pdf", }, &[], &[place(VIEW, Some("Panes"), "PDF")], true),
    // View › Writing tools.
    row("syntax.toggle", "Syntax Highlight", Scope::Win, Kind::Check, &[], &[place(VIEW, Some("Writing tools"), "Syntax Highlight")], true),
    row("syntax.nouns", "Nouns", Scope::Win, Kind::Check, &[], &[place(VIEW, Some("Writing tools"), "Nouns")], true),
    row("syntax.verbs", "Verbs", Scope::Win, Kind::Check, &[], &[place(VIEW, Some("Writing tools"), "Verbs")], true),
    row("syntax.adjectives", "Adjectives", Scope::Win, Kind::Check, &[], &[place(VIEW, Some("Writing tools"), "Adjectives")], true),
    row("syntax.adverbs", "Adverbs", Scope::Win, Kind::Check, &[], &[place(VIEW, Some("Writing tools"), "Adverbs")], true),
    row("syntax.conjunctions", "Conjunctions", Scope::Win, Kind::Check, &[], &[place(VIEW, Some("Writing tools"), "Conjunctions")], true),
    row("style.toggle", "Style Check", Scope::Win, Kind::Check, &[], &[place(VIEW, Some("Writing tools"), "Style Check")], true),
    row("style.fillers", "Fillers", Scope::Win, Kind::Check, &[], &[place(VIEW, Some("Writing tools"), "Fillers")], true),
    row("style.redundancies", "Redundancies", Scope::Win, Kind::Check, &[], &[place(VIEW, Some("Writing tools"), "Redundancies")], true),
    row("style.cliches", "Clichés", Scope::Win, Kind::Check, &[], &[place(VIEW, Some("Writing tools"), "Clichés")], true),
    row("spell.toggle", "Spell Check", Scope::Win, Kind::Check, &[], &[place(VIEW, Some("Writing tools"), "Spell Check")], false),
    // View › Typeface.
    row("font.duo", "Duo", Scope::Win, Kind::Radio { group: "face", value: "duo", }, &[], &[place(VIEW, Some("Typeface"), "Duo")], true),
    row("font.quattro", "Quattro", Scope::Win, Kind::Radio { group: "face", value: "quattro", }, &[], &[place(VIEW, Some("Typeface"), "Quattro")], true),
    row("font.mono", "Mono", Scope::Win, Kind::Radio { group: "face", value: "mono", }, &[], &[place(VIEW, Some("Typeface"), "Mono")], true),
    // View › Template. The five Templates are one radio group whose value is
    // the `[template]` table's `name`; the three toggles are its other keys.
    row("template.modern", "Modern", Scope::Win, Kind::Radio { group: "template", value: "modern", }, &[], &[place(VIEW, Some("Template"), "Modern")], true),
    row("template.classic", "Classic", Scope::Win, Kind::Radio { group: "template", value: "classic", }, &[], &[place(VIEW, Some("Template"), "Classic")], true),
    row("template.manuscriptMono", "Manuscript Mono", Scope::Win, Kind::Radio { group: "template", value: "manuscript-mono", }, &[], &[place(VIEW, Some("Template"), "Manuscript Mono")], true),
    row("template.manuscriptDuo", "Manuscript Duo", Scope::Win, Kind::Radio { group: "template", value: "manuscript-duo", }, &[], &[place(VIEW, Some("Template"), "Manuscript Duo")], true),
    row("template.manuscriptQuattro", "Manuscript Quattro", Scope::Win, Kind::Radio { group: "template", value: "manuscript-quattro", }, &[], &[place(VIEW, Some("Template"), "Manuscript Quattro")], true),
    row("template.centerHeadings", "Center Headings", Scope::Win, Kind::Check, &[], &[place(VIEW, Some("Template"), "Center Headings")], true),
    row("template.numberHeadings", "Number Headings", Scope::Win, Kind::Check, &[], &[place(VIEW, Some("Template"), "Number Headings")], true),
    row("template.indentParagraphs", "Indent Paragraphs", Scope::Win, Kind::Check, &[], &[place(VIEW, Some("Template"), "Indent Paragraphs")], true),
    // View › Appearance.
    row("theme.toggle", "Dark Mode", Scope::Win, Kind::Check, &["Ctrl+Shift+L", "Alt+Shift+N"], &[place(VIEW, Some("Appearance"), "Dark Mode")], true),
    row("font.bigger", "Bigger Text", Scope::Win, Kind::Plain, &["Ctrl+="], &[place(VIEW, Some("Appearance"), "Bigger Text")], true),
    row("font.smaller", "Smaller Text", Scope::Win, Kind::Plain, &["Ctrl+-"], &[place(VIEW, Some("Appearance"), "Smaller Text")], true),
    row("font.reset", "Default Text Size", Scope::Win, Kind::Plain, &["Ctrl+0"], &[place(VIEW, Some("Appearance"), "Default Text Size")], true),
    row("preview.bigger", "Bigger Preview Text", Scope::Win, Kind::Plain, &["Ctrl+Shift+="], &[place(VIEW, Some("Appearance"), "Bigger Preview Text")], true),
    row("preview.smaller", "Smaller Preview Text", Scope::Win, Kind::Plain, &["Ctrl+Shift+-"], &[place(VIEW, Some("Appearance"), "Smaller Preview Text")], true),
    row("preview.reset", "Default Preview Size", Scope::Win, Kind::Plain, &["Ctrl+Shift+0"], &[place(VIEW, Some("Appearance"), "Default Preview Size")], true),
    // View › Window.
    row("chrome.stats", "Statistics", Scope::Win, Kind::Check, &[], &[ place(VIEW, Some("Window"), "Statistics"), place(STATS, None, "Hide Statistics"), ], true),
    row("chrome.toggle", "Hide Bars / Show Bars", Scope::Win, Kind::Check, &["Ctrl+Shift+H"], &[place(VIEW, Some("Window"), "Hide Bars / Show Bars")], true),
    row("window.fullscreen", "Full Screen", Scope::Win, Kind::Check, &["F11"], &[place(VIEW, Some("Window"), "Full Screen")], true),
    row("settings.open", "Settings…", Scope::Win, Kind::Plain, &["Ctrl+,"], &[place(VIEW, Some("Window"), "Settings…")], true),
    row("shortcuts.open", "Keyboard Shortcuts", Scope::Win, Kind::Plain, &["Ctrl+?"], &[place(VIEW, Some("Window"), "Keyboard Shortcuts")], true),
    row("palette.open", "All Commands…", Scope::Win, Kind::Plain, &["Ctrl+K"], &[place(VIEW, Some("Window"), "All Commands…")], true),
    // Stats menu.
    row("stats.words", "Words", Scope::Win, Kind::Radio { group: "stats", value: "words", }, &[], &[place(STATS, None, "Words")], false),
    row("stats.characters", "Characters", Scope::Win, Kind::Radio { group: "stats", value: "characters", }, &[], &[place(STATS, None, "Characters")], false),
    row("stats.charactersNoSpaces", "Characters Without Spaces", Scope::Win, Kind::Radio { group: "stats", value: "charactersNoSpaces", }, &[], &[place(STATS, None, "Characters Without Spaces")], false),
    row("stats.sentences", "Sentences", Scope::Win, Kind::Radio { group: "stats", value: "sentences", }, &[], &[place(STATS, None, "Sentences")], false),
    row("stats.paragraphs", "Paragraphs", Scope::Win, Kind::Radio { group: "stats", value: "paragraphs", }, &[], &[place(STATS, None, "Paragraphs")], false),
    row("stats.readingTime", "Reading Time", Scope::Win, Kind::Radio { group: "stats", value: "readingTime", }, &[], &[place(STATS, None, "Reading Time")], false),
    // Palette and keyboard only.
    row("library.search", "Find a Document…", Scope::Win, Kind::Plain, &["Ctrl+Shift+O"], &[], true),
    row("file.next", "Next Document", Scope::Win, Kind::Plain, &["Ctrl+Page Down"], &[], true),
    row("file.prev", "Previous Document", Scope::Win, Kind::Plain, &["Ctrl+Page Up"], &[], true),
    row("file.follow", "Open Linked Document", Scope::Win, Kind::Plain, &["Ctrl+Enter"], &[], false),
    row("file.recent", "Open Recent…", Scope::Win, Kind::Plain, &[], &[], true),
    row("file.openFolder", "Add Location…", Scope::Win, Kind::Plain, &[], &[], true),
    row("file.delete", "Move to Trash", Scope::Win, Kind::Plain, &[], &[], true),
    row("file.pin", "Pin / Unpin", Scope::Win, Kind::Plain, &[], &[], true),
    row("theme.light", "Light Theme", Scope::Win, Kind::Radio { group: "theme", value: "light", }, &[], &[], true),
    row("theme.dark", "Dark Theme", Scope::Win, Kind::Radio { group: "theme", value: "dark", }, &[], &[], true),
    row("theme.auto", "Follow System", Scope::Win, Kind::Radio { group: "theme", value: "auto", }, &[], &[], true),
    row("chrome.doc", "Document Menu", Scope::Win, Kind::Plain, &[], &[], true),
    row("chrome.view", "View Menu", Scope::Win, Kind::Plain, &["F10"], &[], true),
];

/// Chords with no Command yet, held so nothing else takes them
/// (`docs/shortcuts.md` § Reserved chords).
pub const RESERVED: &[&str] = &[
    "Ctrl+1",
    "Ctrl+2",
    "Ctrl+3",
    "Ctrl+4",
    "Ctrl+5",
    "Ctrl+6",
    "Ctrl+B",
    "Ctrl+I",
    "Ctrl+F",
    "Ctrl+H",
    "Ctrl+G",
    "Ctrl+Shift+G",
];

/// Chords `GtkTextView` takes before a window shortcut sees them, so a
/// Command bound to one is dead (`docs/shortcuts.md` § Off-limits chords).
pub const OFF_LIMITS: &[&str] = &[
    "Ctrl+A",
    "Ctrl+X",
    "Ctrl+C",
    "Ctrl+V",
    "Ctrl+Shift+V",
    "Ctrl+Z",
    "Ctrl+Shift+Z",
    "Ctrl+←",
    "Ctrl+→",
    "Ctrl+Shift+←",
    "Ctrl+Shift+→",
    "Ctrl+Backspace",
    "Ctrl+Delete",
    "Ctrl+Shift+U",
    "Ctrl+.",
    "Ctrl+;",
    "Shift+F10",
    "Menu",
];

/// The Command with this id.
pub fn by_id(id: &str) -> Option<&'static Command> {
    COMMANDS.iter().find(|command| command.id == id)
}

/// The Command a chord in the table's syntax fires, default or alias.
pub fn by_chord(chord: &str) -> Option<&'static Command> {
    COMMANDS
        .iter()
        .find(|command| command.chords().any(|bound| bound == chord))
}

/// The radio groups the registry names, each once, in the table's order.
pub fn radio_groups() -> Vec<&'static str> {
    distinct(COMMANDS.iter().filter_map(|command| match command.kind {
        Kind::Radio { group, .. } => Some(group),
        Kind::Plain | Kind::Check => None,
    }))
}

/// A chord in the table's syntax as GTK's accelerator syntax:
/// `Ctrl+Shift+L` is `<Control><Shift>l`, `Ctrl+Page Down` is
/// `<Control>Page_Down`, `Ctrl+Shift+=` is `<Control><Shift>plus`
/// ([`shifted`]).
///
/// `None` for a key this converter has no name for, which the registry test
/// turns into a failure rather than a silently unbound chord.
pub fn accel(chord: &str) -> Option<String> {
    let mut parts = chord.split('+').collect::<Vec<_>>();
    // `Ctrl++` splits into `Ctrl`, `` and ``: the two empty tails are one
    // `+` key.
    if parts.len() >= 2 && parts[parts.len() - 1].is_empty() && parts[parts.len() - 2].is_empty() {
        parts.truncate(parts.len() - 2);
        parts.push("+");
    }
    let (key, modifiers) = parts.split_last()?;
    let mut out = String::new();
    for modifier in modifiers {
        out.push_str(match *modifier {
            "Ctrl" => "<Control>",
            "Shift" => "<Shift>",
            "Alt" => "<Alt>",
            _ => return None,
        });
    }
    let name = match shifted(key) {
        Some(name) if modifiers.contains(&"Shift") => name.to_owned(),
        _ => key_name(key)?,
    };
    out.push_str(&name);
    Some(out)
}

/// GDK's name for the keyval a US layout's Shift produces for `key`, where
/// Shift produces another one.
///
/// A chord names the keyval the press makes, not the unshifted key: GDK
/// matches a consumed modifier as a don't-care, so `Ctrl+Shift+=` arrives as
/// keyval `plus` with Shift consumed and an accelerator written
/// `<Control><Shift>equal` never matches it. Letters are their own answer and
/// are left to [`key_name`]. The table is the US layout's, as the rest of the
/// registry is.
fn shifted(key: &str) -> Option<&'static str> {
    match key {
        "1" => Some("exclam"),
        "2" => Some("at"),
        "3" => Some("numbersign"),
        "4" => Some("dollar"),
        "5" => Some("percent"),
        "6" => Some("asciicircum"),
        "7" => Some("ampersand"),
        "8" => Some("asterisk"),
        "9" => Some("parenleft"),
        "0" => Some("parenright"),
        "-" => Some("underscore"),
        "=" => Some("plus"),
        "," => Some("less"),
        "." => Some("greater"),
        ";" => Some("colon"),
        "/" => Some("question"),
        _ => None,
    }
}

/// GDK's name for a key as the table writes it.
fn key_name(key: &str) -> Option<String> {
    let named = match key {
        "=" => Some("equal"),
        "+" => Some("plus"),
        "-" => Some("minus"),
        "," => Some("comma"),
        "?" => Some("question"),
        "." => Some("period"),
        ";" => Some("semicolon"),
        "Page Down" => Some("Page_Down"),
        "Page Up" => Some("Page_Up"),
        "Enter" => Some("Return"),
        "Backspace" => Some("BackSpace"),
        "Delete" => Some("Delete"),
        "Menu" => Some("Menu"),
        "←" => Some("Left"),
        "→" => Some("Right"),
        _ => None,
    };
    if let Some(named) = named {
        return Some(named.to_owned());
    }
    let mut chars = key.chars();
    match (chars.next()?, chars.as_str()) {
        (letter, "") if letter.is_ascii_alphabetic() => {
            Some(letter.to_ascii_lowercase().to_string())
        }
        (digit, "") if digit.is_ascii_digit() => Some(digit.to_string()),
        ('F', number) if number.parse::<u8>().is_ok_and(|n| (1..=12).contains(&n)) => {
            Some(key.to_owned())
        }
        _ => None,
    }
}

/// `items` with each value kept where it first appears.
pub fn distinct<T: PartialEq>(items: impl Iterator<Item = T>) -> Vec<T> {
    let mut seen = Vec::new();
    for item in items {
        if !seen.contains(&item) {
            seen.push(item);
        }
    }
    seen
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{BTreeMap, BTreeSet};

    const DOC_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../docs/shortcuts.md");

    /// One id of one row of a Command table, as the file has it.
    #[derive(Debug)]
    struct Row {
        id: String,
        title: String,
        default: Option<String>,
        reserved: Option<String>,
        alias: Option<String>,
        menu: Option<Menu>,
        section: Option<String>,
        note: String,
    }

    /// What the file says: the Command rows in order, the reserved chords,
    /// the off-limits chords.
    struct Table {
        rows: Vec<Row>,
        reserved: Vec<String>,
        off_limits: Vec<String>,
    }

    #[derive(Clone, Copy, PartialEq)]
    enum Under {
        Commands(Option<Menu>),
        Reserved,
        OffLimits,
        Other,
    }

    fn ticks(cell: &str) -> Vec<String> {
        cell.split('`')
            .skip(1)
            .step_by(2)
            .map(str::to_owned)
            .collect()
    }

    fn cells(line: &str) -> Vec<String> {
        line.trim()
            .trim_matches('|')
            .split('|')
            .map(|cell| cell.trim().to_owned())
            .collect()
    }

    /// `Ctrl+1` … `Ctrl+6` as the six chords, anything else as itself.
    fn chord_list(cell: &str) -> Vec<String> {
        let chords = ticks(cell);
        if cell.contains('…') && chords.len() == 2 {
            let (from, to) = (&chords[0], &chords[1]);
            let stem = &from[..from.len() - 1];
            let first = from.chars().last().unwrap().to_digit(10).unwrap();
            let last = to.chars().last().unwrap().to_digit(10).unwrap();
            return (first..=last).map(|n| format!("{stem}{n}")).collect();
        }
        chords
    }

    fn parse(doc: &str) -> Table {
        let mut table = Table {
            rows: Vec::new(),
            reserved: Vec::new(),
            off_limits: Vec::new(),
        };
        let mut under = Under::Other;
        let mut section = None;
        let mut prose = String::new();
        for line in doc.lines() {
            if let Some(heading) = line.strip_prefix("## ") {
                under = match heading {
                    "Document menu" => Under::Commands(Some(Menu::Document)),
                    "View menu" => Under::Commands(Some(Menu::View)),
                    "Stats menu" => Under::Commands(Some(Menu::Stats)),
                    "Palette and keyboard only" => Under::Commands(None),
                    "Reserved chords" => Under::Reserved,
                    "Off-limits chords" => Under::OffLimits,
                    _ => Under::Other,
                };
                section = None;
                continue;
            }
            if let Some(rest) = line.strip_prefix("**") {
                section = rest.split("**").next().map(str::to_owned);
                continue;
            }
            match under {
                Under::OffLimits => {
                    prose.push_str(line);
                    prose.push(' ');
                }
                Under::Reserved if line.starts_with("| `") => {
                    table.reserved.extend(chord_list(&cells(line)[0]));
                }
                Under::Commands(menu) if line.starts_with("| `") => {
                    table
                        .rows
                        .extend(rows(&cells(line), menu, section.as_deref()));
                }
                _ => {}
            }
        }
        let dead = prose
            .split_once("dead:")
            .map(|(_, rest)| rest)
            .and_then(|rest| rest.split_once(". Undo"))
            .map(|(chords, _)| chords)
            .unwrap_or("");
        table.off_limits = ticks(dead);
        table
    }

    /// The ids one table row carries, each with its title: one, a list, or
    /// the `syntax.nouns` … `syntax.conjunctions` range whose ids are the
    /// titles lowercased after the prefix.
    fn rows(cells: &[String], menu: Option<Menu>, section: Option<&str>) -> Vec<Row> {
        let (title, note) = match cells[1].split_once(" (") {
            Some((title, note)) => (title, note.trim_end_matches(')')),
            None => (cells[1].as_str(), ""),
        };
        let ids = ticks(&cells[0]);
        let titles = title.split(", ").collect::<Vec<_>>();
        let pairs: Vec<(String, String)> = if cells[0].contains('…') {
            let prefix = ids[0].split_once('.').unwrap().0;
            titles
                .iter()
                .map(|title| {
                    (
                        format!("{prefix}.{}", title.to_lowercase()),
                        (*title).to_owned(),
                    )
                })
                .collect()
        } else if ids.len() > 1 {
            ids.into_iter()
                .zip(titles.iter().map(|t| (*t).to_owned()))
                .collect()
        } else {
            vec![(ids[0].clone(), title.to_owned())]
        };
        let default_cell = cells[2].as_str();
        let (default, reserved) = if let Some(rest) = default_cell.strip_prefix("reserved ") {
            (None, ticks(rest).into_iter().next())
        } else {
            (ticks(default_cell).into_iter().next(), None)
        };
        let alias = cells.get(3).and_then(|cell| ticks(cell).into_iter().next());
        pairs
            .into_iter()
            .map(|(id, title)| Row {
                id,
                title,
                default: default.clone(),
                reserved: reserved.clone(),
                alias: alias.clone(),
                menu,
                section: section.map(str::to_owned),
                note: note.to_owned(),
            })
            .collect()
    }

    /// Every way the registry can disagree with the file, as one line each.
    fn check(doc: &str) -> Vec<String> {
        let table = parse(doc);
        let mut faults = Vec::new();
        let ids_in_file = distinct(table.rows.iter().map(|row| row.id.as_str()));
        let ids_here: Vec<&str> = COMMANDS.iter().map(|command| command.id).collect();
        if ids_in_file != ids_here {
            faults.push(format!(
                "ids differ or are out of order: file {ids_in_file:?}, registry {ids_here:?}"
            ));
        }
        let mut rows_by_id: BTreeMap<&str, Vec<&Row>> = BTreeMap::new();
        for row in &table.rows {
            rows_by_id.entry(&row.id).or_default().push(row);
        }
        for command in COMMANDS {
            let Some(rows) = rows_by_id.get(command.id) else {
                continue;
            };
            let first = rows[0];
            if command.default() != first.default.as_deref() {
                faults.push(format!(
                    "{}: default is {:?} here, {:?} in the file",
                    command.id,
                    command.default(),
                    first.default
                ));
            }
            let aliases: Vec<&str> = first.alias.iter().map(String::as_str).collect();
            if command.aliases() != aliases.as_slice() {
                faults.push(format!(
                    "{}: aliases are {:?} here, {:?} in the file",
                    command.id,
                    command.aliases(),
                    aliases
                ));
            }
            if let Some(reserved) = &first.reserved
                && !RESERVED.contains(&reserved.as_str())
            {
                faults.push(format!(
                    "{}: the file reserves {reserved}, RESERVED lacks it",
                    command.id
                ));
            }
            if command.title != first.title {
                faults.push(format!(
                    "{}: title is {:?} here, {:?} in the file",
                    command.id, command.title, first.title
                ));
            }
            let placed: Vec<&Row> = rows
                .iter()
                .copied()
                .filter(|row| row.menu.is_some() && !row.note.contains("Palette only"))
                .collect();
            if placed.len() != command.placements.len() {
                faults.push(format!(
                    "{}: {} placements here, {} rows in the file",
                    command.id,
                    command.placements.len(),
                    placed.len()
                ));
            }
            for row in &placed {
                if !command.placements.iter().any(|p| {
                    Some(p.menu) == row.menu
                        && p.section == row.section.as_deref()
                        && p.label == row.title
                }) {
                    faults.push(format!(
                        "{}: no placement for the file's row {:?} › {:?} {:?}",
                        command.id, row.menu, row.section, row.title
                    ));
                }
                let is_radio = matches!(command.kind, Kind::Radio { .. });
                let is_check = command.kind == Kind::Check;
                if row.note.contains("radio") && !is_radio {
                    faults.push(format!("{}: the file's row is a radio", command.id));
                }
                if row.note.contains("check") && !is_check {
                    faults.push(format!("{}: the file's row is a check", command.id));
                }
            }
        }
        let sections: Vec<&str> = table
            .rows
            .iter()
            .filter(|row| row.menu == Some(Menu::View))
            .filter_map(|row| row.section.as_deref())
            .fold(Vec::new(), |mut acc, s| {
                if acc.last() != Some(&s) {
                    acc.push(s);
                }
                acc
            });
        if sections != VIEW_SECTIONS {
            faults.push(format!(
                "View sections are {VIEW_SECTIONS:?} here, {sections:?} in the file"
            ));
        }
        let reserved: BTreeSet<&str> = table.reserved.iter().map(String::as_str).collect();
        if reserved != RESERVED.iter().copied().collect() {
            faults.push(format!(
                "reserved chords are {RESERVED:?} here, {reserved:?} in the file"
            ));
        }
        let off_limits: BTreeSet<&str> = table.off_limits.iter().map(String::as_str).collect();
        if off_limits != OFF_LIMITS.iter().copied().collect() {
            faults.push(format!(
                "off-limits chords are {OFF_LIMITS:?} here, {off_limits:?} in the file"
            ));
        }
        let mut bound: BTreeMap<&str, &str> = BTreeMap::new();
        for command in COMMANDS {
            for chord in command.chords() {
                if let Some(other) = bound.insert(chord, command.id) {
                    faults.push(format!(
                        "{chord} is bound to both {other} and {}",
                        command.id
                    ));
                }
                if RESERVED.contains(&chord) {
                    faults.push(format!("{}: {chord} is reserved", command.id));
                }
                if OFF_LIMITS.contains(&chord) {
                    faults.push(format!("{}: {chord} is off limits", command.id));
                }
            }
        }
        for chord in bound
            .keys()
            .copied()
            .chain(RESERVED.iter().copied())
            .chain(OFF_LIMITS.iter().copied())
        {
            if accel(chord).is_none() {
                faults.push(format!("{chord} has no accelerator syntax"));
            }
        }
        faults
    }

    #[test]
    fn the_registry_agrees_with_docs_shortcuts_md_row_for_row() {
        let doc = std::fs::read_to_string(DOC_PATH).unwrap();
        assert!(!parse(&doc).rows.is_empty());
        let faults = check(&doc);
        assert!(faults.is_empty(), "{}", faults.join("\n"));
    }

    #[test]
    fn a_row_altered_in_the_file_fails_the_check() {
        let doc = std::fs::read_to_string(DOC_PATH).unwrap();
        let chord = doc.replace(
            "| `file.new` | New Document | `Ctrl+N` |",
            "| `file.new` | New Document | `Ctrl+M` |",
        );
        assert_ne!(chord, doc);
        assert_eq!(
            check(&chord),
            ["file.new: default is Some(\"Ctrl+N\") here, Some(\"Ctrl+M\") in the file"]
        );
        let row = doc.replace("| `spell.toggle` | Spell Check | — | |\n", "");
        assert_ne!(row, doc);
        assert!(
            check(&row)
                .iter()
                .any(|fault| fault.starts_with("ids differ"))
        );
        let reserved = doc.replace("| `Ctrl+B`, `Ctrl+I` |", "| `Ctrl+J`, `Ctrl+I` |");
        assert_ne!(reserved, doc);
        assert!(
            check(&reserved)
                .iter()
                .any(|fault| fault.starts_with("reserved chords"))
        );
    }

    #[test]
    fn a_chord_becomes_the_accelerator_gtk_installs() {
        assert_eq!(accel("Ctrl+Shift+L").as_deref(), Some("<Control><Shift>l"));
        assert_eq!(accel("Ctrl+=").as_deref(), Some("<Control>equal"));
        assert_eq!(accel("Ctrl++").as_deref(), Some("<Control>plus"));
        assert_eq!(accel("Ctrl+-").as_deref(), Some("<Control>minus"));
        assert_eq!(accel("Ctrl+0").as_deref(), Some("<Control>0"));
        assert_eq!(
            accel("Ctrl+Page Down").as_deref(),
            Some("<Control>Page_Down")
        );
        assert_eq!(accel("Alt+Shift+N").as_deref(), Some("<Alt><Shift>n"));
        assert_eq!(accel("F10").as_deref(), Some("F10"));
        assert_eq!(accel("Ctrl+?").as_deref(), Some("<Control>question"));
        assert_eq!(accel("Super+Q"), None);
        assert_eq!(accel("Ctrl+F13"), None);
    }

    #[test]
    fn a_shift_chord_names_the_keyval_shift_produces() {
        assert_eq!(
            accel("Ctrl+Shift+=").as_deref(),
            Some("<Control><Shift>plus")
        );
        assert_eq!(
            accel("Ctrl+Shift+-").as_deref(),
            Some("<Control><Shift>underscore")
        );
        assert_eq!(
            accel("Ctrl+Shift+0").as_deref(),
            Some("<Control><Shift>parenright")
        );
        assert_eq!(
            accel("Ctrl+Shift+7").as_deref(),
            Some("<Control><Shift>ampersand")
        );
        // A letter is its own keyval, and no modifier changes an unshifted
        // chord's key.
        assert_eq!(accel("Ctrl+Shift+L").as_deref(), Some("<Control><Shift>l"));
        assert_eq!(accel("Ctrl+=").as_deref(), Some("<Control>equal"));
        assert_eq!(accel("Ctrl+0").as_deref(), Some("<Control>0"));
    }

    #[test]
    fn a_chord_finds_its_command_by_default_or_alias() {
        assert_eq!(by_chord("Ctrl+E").map(|c| c.id), Some("library.toggle"));
        assert_eq!(by_chord("F9").map(|c| c.id), Some("library.toggle"));
        assert_eq!(by_chord("Ctrl+Shift+P").map(|c| c.id), Some("export.quick"));
        // A reserved chord has no Command yet, so it finds none.
        assert_eq!(by_chord("Ctrl+B"), None);
        assert_eq!(by_id("app.quit").map(Command::name), Some("quit"));
        assert_eq!(
            by_id("app.quit").map(Command::action).as_deref(),
            Some("app.quit")
        );
        assert_eq!(by_id("window.new").map(Command::name), Some("window.new"));
        assert_eq!(
            by_id("window.new").map(Command::action).as_deref(),
            Some("app.window.new")
        );
        assert_eq!(
            by_id("focus.toggle").map(Command::action).as_deref(),
            Some("win.focus.toggle")
        );
        assert_eq!(
            by_id("library.toggle").map(|c| (c.default(), c.aliases())),
            Some((Some("Ctrl+E"), ["F9"].as_slice()))
        );
        assert_eq!(
            by_id("file.duplicate").map(|c| (c.default(), c.aliases())),
            Some((None, [].as_slice()))
        );
    }

    #[test]
    fn the_hidden_commands_are_the_tables_palette_only_ids_with_chords_or_menus_elsewhere() {
        let hidden: Vec<&str> = COMMANDS
            .iter()
            .filter(|c| c.hidden())
            .map(|c| c.id)
            .collect();
        assert!(hidden.contains(&"focus.swap"));
        assert!(hidden.contains(&"chrome.view"));
        assert!(hidden.contains(&"chrome.doc"));
        assert!(!hidden.contains(&"palette.open"));
        assert_eq!(by_id("chrome.stats").unwrap().placements.len(), 2);
        assert_eq!(
            radio_groups(),
            [
                "focus_scope",
                "preview_mode",
                "face",
                "template",
                "stats",
                "theme"
            ]
        );
    }
}
