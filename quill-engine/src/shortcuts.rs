//! The `[shortcuts]` table: what the writer rebound, checked and overlaid on
//! the registry's defaults.
//!
//! `settings.toml` writes a Command id against a list of chords in GTK's
//! accelerator syntax (`docs/shortcuts.md` § Rebinding); the registry
//! ([`crate::commands`]) writes its defaults in the table's syntax
//! (`Ctrl+Shift+L`), which [`crate::commands::accel`] turns into GTK's. The
//! two meet in [`Chord`], the one form everything here compares in: GTK
//! syntax, the modifiers in a fixed order, a one-character key lowercased.
//! Every comparison below — the off-limits and reserved lists, the duplicate
//! check, the overlay — is equality on that form.
//!
//! Only the *shape* of a chord is read here: `<Modifier>` groups, then one key
//! token. Whether GDK has a key by that name is `gtk::accelerator_parse`'s
//! answer when the app installs it, and the engine may not ask GTK anything
//! (ADR 0008), so a chord of the right shape naming a key that is not one
//! leaves this module accepted and is refused at install.

use std::collections::BTreeMap;
use std::fmt;

use crate::commands::{self, COMMANDS, Command, OFF_LIMITS, RESERVED};

/// A modifier a chord can carry, in the order [`Chord`] writes them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Modifier {
    Control,
    Alt,
    Shift,
    Super,
    Meta,
    Hyper,
}

impl Modifier {
    /// The modifier a tag between angle brackets names, GTK's other spellings
    /// included, or `None` for a tag that names no modifier at all.
    fn named(tag: &str) -> Option<Self> {
        match tag {
            "Control" | "Ctrl" | "Primary" => Some(Self::Control),
            "Alt" | "Mod1" => Some(Self::Alt),
            "Shift" => Some(Self::Shift),
            "Super" => Some(Self::Super),
            "Meta" => Some(Self::Meta),
            "Hyper" => Some(Self::Hyper),
            _ => None,
        }
    }

    /// The tag [`Chord`] writes it as.
    fn tag(self) -> &'static str {
        match self {
            Self::Control => "Control",
            Self::Alt => "Alt",
            Self::Shift => "Shift",
            Self::Super => "Super",
            Self::Meta => "Meta",
            Self::Hyper => "Hyper",
        }
    }
}

/// A chord in GTK's accelerator syntax, in the one form this module compares.
///
/// Written `<Control><Alt><Shift><Super><Meta><Hyper>key` — each modifier at
/// most once and in that order — with a one-character key lowercased, because
/// GTK reads the modifiers in any order and `<Control>L` and `<Control>l` are
/// one chord to it. Longer key names are left as written, because GDK's own
/// names carry their case (`Page_Down`, `F9`). Two chords are the same chord
/// when they are equal, which is what makes the duplicate check and the
/// off-limits and reserved lists string comparisons.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Chord {
    /// The canonical text, built from the two below, so that equality on the
    /// whole struct is equality on what GTK would install.
    text: String,
    /// The modifiers it carries, deduplicated and in [`Modifier`]'s order.
    modifiers: Vec<Modifier>,
}

impl Chord {
    /// The chord `text` names, or `None` when `text` is not the shape of one.
    ///
    /// The shape is `<Modifier>` groups, each naming a modifier GTK knows,
    /// then one key token with no angle bracket or space in it. The key name
    /// itself is not checked here: `<Control>frobnicate` has the shape of a
    /// chord and is refused when the app hands it to `gtk::accelerator_parse`.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let mut modifiers: Vec<Modifier> = Vec::new();
        let mut rest = text;
        while let Some(after) = rest.strip_prefix('<') {
            let (tag, tail) = after.split_once('>')?;
            let modifier = Modifier::named(tag)?;
            if !modifiers.contains(&modifier) {
                modifiers.push(modifier);
            }
            rest = tail;
        }
        if rest.is_empty() || rest.contains(['<', '>']) || rest.contains(char::is_whitespace) {
            return None;
        }
        modifiers.sort_unstable();
        let mut written = String::new();
        for modifier in &modifiers {
            written.push('<');
            written.push_str(modifier.tag());
            written.push('>');
        }
        written.push_str(&key(rest));
        Some(Self {
            text: written,
            modifiers,
        })
    }

    /// The chord as GTK's accelerator syntax, which is what the app installs.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.text
    }

    /// The modifier tags it carries, GTK's spelling of each, in the order
    /// [`Chord`] writes them.
    pub fn modifiers(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.modifiers.iter().map(|modifier| modifier.tag())
    }

    /// The key token, the one half of a chord this module does not check: a
    /// caller with GDK to hand asks it whether there is a key by that name.
    #[must_use]
    pub fn key(&self) -> &str {
        self.text
            .rsplit_once('>')
            .map_or(self.text.as_str(), |(_, key)| key)
    }

    /// Whether the chord carries `modifier`.
    fn carries(&self, modifier: Modifier) -> bool {
        self.modifiers.contains(&modifier)
    }
}

impl fmt::Display for Chord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.text)
    }
}

/// The key token as [`Chord`] writes it: one character is lowercased, because
/// GTK reads `<Control>L` and `<Control>l` as one chord; a longer name is left
/// as written, because GDK's names carry their own case.
fn key(token: &str) -> String {
    let mut characters = token.chars();
    match (characters.next(), characters.next()) {
        (Some(only), None) => only.to_lowercase().to_string(),
        _ => token.to_owned(),
    }
}

/// One `[shortcuts]` entry that could not be applied, ready to log.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refusal {
    /// The entry as the file writes it: `"library.toggle" = ["<Super>l"]`.
    pub line: String,
    /// The Command id the entry names, whether or not the registry has it.
    pub id: String,
    /// Why none of the entry was applied, one sentence.
    pub reason: String,
}

impl Refusal {
    /// The refusal for a chord that has the shape of one and names no key.
    ///
    /// The app's to make rather than this module's: the shape is all that is
    /// read here, and whether GDK has a key by that name is
    /// `gtk::accelerator_parse`'s answer at install (the `//!` above).
    /// `entry` is the writer's whole line as [`Shortcuts::entries`] kept it,
    /// so a refusal made at install quotes exactly what a refusal made here
    /// would, and it refuses the whole entry as every refusal here does.
    #[must_use]
    pub fn unknown_key(id: &str, entry: &str, chord: &Chord) -> Self {
        Self {
            line: entry.to_owned(),
            id: id.to_owned(),
            reason: format!("{chord} names no key on this keyboard"),
        }
    }
}

impl fmt::Display for Refusal {
    /// The entry and why it was refused, in one line: what the app warns
    /// under `quill-settings` and what the Settings window shows at the
    /// bottom (`quill::settings::refused`), which are the same sentence said
    /// in two places.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.line, self.reason)
    }
}

/// Every Command in the registry with the chords it is bound to, in id order;
/// empty for a Command bound to none.
pub type Bound = BTreeMap<&'static str, Vec<Chord>>;

/// What a `[shortcuts]` table came to: the chords to install, and the entries
/// that were refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shortcuts {
    /// The chords to install, the file's entries over the registry's
    /// defaults.
    pub chords: Bound,
    /// The line every entry the file bound is written as, by the Command it
    /// names, so that a refusal made after this module has had its say — a
    /// chord GDK has no key for, which only the app can find out
    /// (`quill::chrome::install_chords`) — quotes the writer's whole entry.
    /// A Command the file left alone is not in here.
    pub entries: BTreeMap<&'static str, String>,
    /// One per refused entry, in the file's order.
    pub refusals: Vec<Refusal>,
}

/// Reads a `[shortcuts]` table: the chords every Command has once the file is
/// over the defaults, and one refusal per entry that could not be applied.
#[must_use]
pub fn read(table: &toml::Table) -> Shortcuts {
    let (accepted, refusals) = validate(table);
    let entries = accepted
        .keys()
        .filter_map(|id| table.get(*id).map(|value| (*id, line(id, value))))
        .collect();
    Shortcuts {
        chords: effective(COMMANDS, &accepted),
        entries,
        refusals,
    }
}

/// The entries that can be installed, and one refusal for each that cannot.
///
/// Entries are read in the file's order, because the duplicate rule is
/// first-wins: a chord an earlier entry took refuses the later one, which is
/// what `legacy/app/js/core.js` `buildKeymap` does over the whole keymap. The
/// check is against the chords this file took, not against the defaults an
/// untouched Command still has, so the rule a writer can see in their own file
/// is the whole of it.
#[must_use]
pub fn validate(table: &toml::Table) -> (Bound, Vec<Refusal>) {
    let mut accepted: Bound = BTreeMap::new();
    let mut refusals = Vec::new();
    let mut taken: BTreeMap<Chord, &'static str> = BTreeMap::new();
    for (id, value) in table {
        match entry(id, value, &taken) {
            Ok((command, chords)) => {
                for chord in &chords {
                    taken.insert(chord.clone(), command);
                }
                accepted.insert(command, chords);
            }
            Err(reason) => refusals.push(Refusal {
                line: line(id, value),
                id: id.clone(),
                reason,
            }),
        }
    }
    (accepted, refusals)
}

/// The chords every Command in `defaults` should have with `accepted` over the
/// top: the entry's chords where the file gave one, and the Command's own
/// otherwise.
///
/// Computed on every read rather than kept, which is what makes an entry taken
/// out of the file restore that Command's default and an empty entry unbind
/// it: nothing is remembered between two reads.
#[must_use]
pub fn effective(defaults: &[Command], accepted: &Bound) -> Bound {
    defaults
        .iter()
        .map(|command| {
            let chords = accepted.get(command.id).cloned().unwrap_or_else(|| {
                command
                    .accels()
                    .iter()
                    .filter_map(|accel| Chord::parse(accel))
                    .collect()
            });
            (command.id, chords)
        })
        .collect()
}

/// One section of the `Ctrl+?` window: a menu, or the Commands with no menu.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Section {
    /// What the window's switcher calls it.
    pub title: &'static str,
    /// Its groups, in the table's order.
    pub groups: Vec<Group>,
}

/// One group of a [`Section`]: a View menu subsection, or the whole of a
/// section that has no subsections.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Group {
    /// The heading over its rows, and `None` for the one group of a section
    /// that has no subsections, whose heading is the section's own title.
    pub title: Option<&'static str>,
    /// Its rows, in the table's order.
    pub shortcuts: Vec<Shortcut>,
}

/// One row of a [`Group`]: a Command, and the chord it is on now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shortcut {
    /// The Command's title as `docs/shortcuts.md` writes it, the pairs whole
    /// (`Show Library / Hide Library`): this window is the table rather than
    /// a menu, and has no window's state to read half a pair against.
    pub title: &'static str,
    /// The chord the row is labelled with, in GTK's accelerator syntax, and
    /// empty for a Command on none. The aliases are left out for the reason
    /// the menus leave them out: a row shows the chord it is labelled by.
    pub accelerator: String,
}

/// The section for the Commands with no menu row, titled as
/// `docs/shortcuts.md` heads their table.
const KEYBOARD_ONLY: &str = "Palette and keyboard only";

/// The `Ctrl+?` window's sections for `chords`: every Command in the registry,
/// grouped the way the menus group it, each on the chord that map leaves it.
///
/// A pure function of the effective map ([`effective`]), which is what makes
/// the window show what the writer's file came to rather than the defaults,
/// and lets it be built again on every open rather than kept. A Command placed
/// in two menus has a row in both, as it has a row in both menus.
#[must_use]
pub fn sections(chords: &Bound) -> Vec<Section> {
    commands::MENUS
        .iter()
        .map(|menu| Section {
            title: menu.title(),
            groups: groups(*menu, chords),
        })
        .chain(std::iter::once(Section {
            title: KEYBOARD_ONLY,
            groups: vec![Group {
                title: None,
                shortcuts: rows(COMMANDS.iter().filter(|command| command.hidden()), chords),
            }],
        }))
        .collect()
}

/// One menu's groups: the View menu's six subsections, and one group holding
/// the whole of a menu that has none.
fn groups(menu: commands::Menu, chords: &Bound) -> Vec<Group> {
    if menu == commands::Menu::View {
        return commands::VIEW_SECTIONS
            .iter()
            .map(|section| Group {
                title: Some(section),
                shortcuts: rows(placed(menu, Some(section)), chords),
            })
            .collect();
    }
    vec![Group {
        title: None,
        shortcuts: rows(placed(menu, None), chords),
    }]
}

/// The Commands with a row in `menu` under `section`, in the table's order.
fn placed(
    menu: commands::Menu,
    section: Option<&'static str>,
) -> impl Iterator<Item = &'static Command> {
    COMMANDS.iter().filter(move |command| {
        command
            .placements
            .iter()
            .any(|placement| placement.menu == menu && placement.section == section)
    })
}

/// One row per Command in the order they come, each labelled with the first
/// chord `chords` leaves it on.
fn rows<'a>(commands: impl Iterator<Item = &'a Command>, chords: &Bound) -> Vec<Shortcut> {
    commands
        .map(|command| Shortcut {
            title: command.title,
            accelerator: chords
                .get(command.id)
                .and_then(|chords| chords.first())
                .map_or_else(String::new, |chord| chord.as_str().to_owned()),
        })
        .collect()
}

/// One entry: the Command it names and the chords to install, or the one
/// reason none of them can be.
fn entry(
    id: &str,
    value: &toml::Value,
    taken: &BTreeMap<Chord, &'static str>,
) -> Result<(&'static str, Vec<Chord>), String> {
    let command =
        commands::by_id(id).ok_or_else(|| "there is no Command with this id".to_owned())?;
    let written = value
        .as_array()
        .ok_or_else(|| "this key takes a list of chords".to_owned())?;
    let mut chords: Vec<Chord> = Vec::new();
    for written in written {
        let text = written
            .as_str()
            .ok_or_else(|| format!("{written} is not a chord: it is not even text"))?;
        let chord = Chord::parse(text).ok_or_else(|| {
            format!("{text} is not a chord: <Modifier> groups, then one key name")
        })?;
        if listed(OFF_LIMITS, &chord) {
            return Err(format!("{chord} is off limits: GtkTextView takes it first"));
        }
        if listed(RESERVED, &chord) {
            return Err(format!(
                "{chord} is reserved for a Command Quill has not shipped"
            ));
        }
        if chord.carries(Modifier::Super) {
            return Err(format!("{chord} belongs to the compositor"));
        }
        if chord.carries(Modifier::Control) && chord.carries(Modifier::Alt) {
            return Err(format!("{chord} belongs to the desktop"));
        }
        let holder = taken
            .get(&chord)
            .copied()
            .or_else(|| chords.contains(&chord).then_some(command.id));
        if let Some(holder) = holder {
            return Err(format!(
                "{chord} is already bound to {holder} earlier in the file"
            ));
        }
        chords.push(chord);
    }
    Ok((command.id, chords))
}

/// Whether one of the registry's chord lists, which are in the table's syntax,
/// holds this chord.
fn listed(list: &[&str], chord: &Chord) -> bool {
    list.iter()
        .filter_map(|written| commands::accel(written))
        .filter_map(|accel| Chord::parse(&accel))
        .any(|listed| listed == *chord)
}

/// The entry as the file writes it, for the log line that names it.
fn line(id: &str, value: &toml::Value) -> String {
    format!("{} = {value}", toml::Value::String(id.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One `[shortcuts]` table with every kind of entry in it, accepted and
    /// refused, in the order the duplicate rule reads them.
    const TABLE: &str = "\
[shortcuts]
\"library.toggle\" = [\"F9\"]
\"theme.toggle\" = []
\"style.toggle\" = [\"<Control><Shift>k\"]
\"file.new\" = [\"<Control>N\", \"<Alt>n\"]
\"libary.toggle\" = [\"<Control>b\"]
\"file.save\" = [\"<Control\"]
\"stats.words\" = \"F8\"
\"file.saveAs\" = [\"<Control>a\"]
\"focus.toggle\" = [\"<Control>p\"]
\"typewriter.toggle\" = [\"<Super>t\"]
\"chrome.toggle\" = [\"<Control><Alt>h\"]
\"spell.toggle\" = [\"F9\"]
";

    /// The `[shortcuts]` table of a settings file, read as the app reads it.
    fn table(text: &str) -> toml::Table {
        let (settings, notes) = crate::settings::Settings::parse(text);
        assert_eq!(notes, Vec::<String>::new(), "{text}");
        settings.shortcuts
    }

    /// The chords a Command has by default, as [`Chord`] writes them.
    fn defaults(id: &str) -> Vec<Chord> {
        commands::by_id(id)
            .expect(id)
            .accels()
            .iter()
            .filter_map(|accel| Chord::parse(accel))
            .collect()
    }

    /// The chords the texts name.
    fn chords(texts: &[&str]) -> Vec<Chord> {
        texts.iter().filter_map(|text| Chord::parse(text)).collect()
    }

    /// A refusal the app makes at install reads like one this module made: the
    /// writer's whole entry, written the way the writer wrote it.
    ///
    /// Held against a refusal of this module's over an entry of the same shape
    /// rather than against a copy of the line, so that the two cannot drift
    /// apart. A Command the file never named is not in [`Shortcuts::entries`]
    /// at all, so there is no entry to quote and nothing at install to refuse.
    #[test]
    fn a_chord_that_names_no_key_is_refused_in_the_writers_own_words() {
        // An entry this module refuses itself, for the line it writes.
        let refusable = "\"library.toggle\" = [\"F9\", \"<Super>l\"]";
        let refused = validate(&table(&format!("[shortcuts]\n{refusable}\n")))
            .1
            .remove(0);
        assert_eq!(refused.line, refusable);
        // The same entry with a chord only the app can refuse: the shape is
        // one this module accepts, so the line it kept is what the app quotes.
        let installable = "\"library.toggle\" = [\"F9\", \"<Control>frobnicate\"]";
        let kept = read(&table(&format!("[shortcuts]\n{installable}\n")));
        assert_eq!(kept.refusals, Vec::new());
        assert_eq!(kept.entries.get("theme.toggle"), None);
        let chord = Chord::parse("<Control>frobnicate").expect("the shape of a chord");
        let unknown =
            Refusal::unknown_key("library.toggle", &kept.entries["library.toggle"], &chord);
        assert_eq!(unknown.id, refused.id);
        assert_eq!(unknown.line, installable, "the whole entry, not the chord");
        assert!(unknown.reason.contains(chord.as_str()), "{unknown:?}");
    }

    #[test]
    fn every_kind_of_entry_lands_as_the_table_writes_it() {
        let read = read(&table(TABLE));
        // Accepted: replaced, unbound, given a chord it never had, and given
        // a second one.
        for (id, expected) in [
            ("library.toggle", vec!["F9"]),
            ("theme.toggle", vec![]),
            ("style.toggle", vec!["<Control><Shift>k"]),
            ("file.new", vec!["<Control>n", "<Alt>n"]),
        ] {
            assert_eq!(read.chords[id], chords(&expected), "{id}");
        }
        // Refused: one line each, in the file's order, and the Command keeps
        // the chords it had.
        let refused = [
            (
                "libary.toggle",
                "\"libary.toggle\" = [\"<Control>b\"]",
                "there is no Command with this id",
            ),
            (
                "file.save",
                "\"file.save\" = [\"<Control\"]",
                "<Control is not a chord: <Modifier> groups, then one key name",
            ),
            (
                "stats.words",
                "\"stats.words\" = \"F8\"",
                "this key takes a list of chords",
            ),
            (
                "file.saveAs",
                "\"file.saveAs\" = [\"<Control>a\"]",
                "<Control>a is off limits: GtkTextView takes it first",
            ),
            (
                "focus.toggle",
                "\"focus.toggle\" = [\"<Control>p\"]",
                "<Control>p is reserved for a Command Quill has not shipped",
            ),
            (
                "typewriter.toggle",
                "\"typewriter.toggle\" = [\"<Super>t\"]",
                "<Super>t belongs to the compositor",
            ),
            (
                "chrome.toggle",
                "\"chrome.toggle\" = [\"<Control><Alt>h\"]",
                "<Control><Alt>h belongs to the desktop",
            ),
            (
                "spell.toggle",
                "\"spell.toggle\" = [\"F9\"]",
                "F9 is already bound to library.toggle earlier in the file",
            ),
        ];
        assert_eq!(
            read.refusals.len(),
            refused.len(),
            "{:#?}",
            read.refusals.iter().map(|r| &r.line).collect::<Vec<_>>()
        );
        for (refusal, (id, line, reason)) in read.refusals.iter().zip(refused) {
            assert_eq!(refusal.id, id);
            assert_eq!(refusal.line, line, "{id}");
            assert_eq!(refusal.reason, reason, "{id}");
            if let Some(command) = commands::by_id(id) {
                assert_eq!(read.chords[command.id], defaults(id), "{id} keeps its own");
            }
        }
    }

    #[test]
    fn a_command_taken_out_of_the_table_has_its_default_back() {
        let bound = read(&table(TABLE));
        assert_eq!(bound.chords["library.toggle"], chords(&["F9"]));
        let without = TABLE.replace("\"library.toggle\" = [\"F9\"]\n", "");
        let read = read(&table(&without));
        assert_eq!(read.chords["library.toggle"], defaults("library.toggle"));
        // And the entry that was refused for taking `F9` after it is accepted
        // now that nothing took it first.
        assert_eq!(read.chords["spell.toggle"], chords(&["F9"]));
    }

    #[test]
    fn a_chord_bound_twice_stays_with_the_entry_that_asked_first() {
        let read = read(&table(
            "[shortcuts]\n\"style.toggle\" = [\"<Control><Shift>k\"]\n\"spell.toggle\" = [\"<Control><Shift>K\", \"<Control><Shift>K\"]\n",
        ));
        assert_eq!(read.chords["style.toggle"], chords(&["<Control><Shift>k"]));
        assert_eq!(read.chords["spell.toggle"], defaults("spell.toggle"));
        assert_eq!(
            read.refusals.iter().map(|r| &r.reason).collect::<Vec<_>>(),
            ["<Control><Shift>k is already bound to style.toggle earlier in the file"]
        );
    }

    #[test]
    fn a_table_with_nothing_in_it_is_the_registrys_own_chords() {
        let read = read(&toml::Table::new());
        assert_eq!(read.refusals, Vec::new());
        assert_eq!(read.chords.len(), COMMANDS.len());
        for command in COMMANDS {
            assert_eq!(
                read.chords[command.id],
                defaults(command.id),
                "{}",
                command.id
            );
        }
    }

    #[test]
    fn every_chord_the_shortcuts_table_names_is_the_shape_of_one() {
        let written = COMMANDS
            .iter()
            .flat_map(Command::chords)
            .chain(RESERVED.iter().copied())
            .chain(OFF_LIMITS.iter().copied());
        let mut seen = 0;
        for chord in written {
            let accel = commands::accel(chord).unwrap_or_else(|| panic!("{chord}"));
            assert!(Chord::parse(&accel).is_some(), "{chord} is {accel}");
            seen += 1;
        }
        let bound: usize = COMMANDS
            .iter()
            .map(|command| command.chords().count())
            .sum();
        assert_eq!(seen, bound + RESERVED.len() + OFF_LIMITS.len());
    }

    #[test]
    fn a_chord_is_the_same_chord_however_the_writer_ordered_it() {
        assert_eq!(
            Chord::parse("<Shift><Control>L"),
            Chord::parse("<Primary><Shift>l")
        );
        assert_eq!(
            Chord::parse("<Control><Shift>l").map(|chord| chord.as_str().to_owned()),
            Some("<Control><Shift>l".to_owned())
        );
        // A key name longer than a character keeps its case, because GDK's
        // names do.
        assert_eq!(
            Chord::parse("<Control>Page_Down").map(|chord| chord.as_str().to_owned()),
            Some("<Control>Page_Down".to_owned())
        );
        for text in [
            "",
            "<Control>",
            "<Control>a b",
            "<Frobnicate>k",
            "<Control<a",
        ] {
            assert_eq!(Chord::parse(text), None, "{text:?}");
        }
    }

    /// The settings file the Gate judges the chrome Piece with, read here
    /// rather than written out again, so that the window this models and the
    /// shots the critics are shown are the same rebinding.
    const FIXTURE: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../quill/tests/fixtures/rebind.toml"
    );

    /// Every row the `Ctrl+?` window would show, title and accelerator, in the
    /// order the sections put them in.
    fn labelled(sections: &[Section]) -> Vec<(&'static str, String)> {
        sections
            .iter()
            .flat_map(|section| section.groups.iter())
            .flat_map(|group| group.shortcuts.iter())
            .map(|shortcut| (shortcut.title, shortcut.accelerator.clone()))
            .collect()
    }

    /// The window under the rebinding fixture: `F9` for the Library, the chord
    /// the fixture gave Dark Mode, and every other Command on its own default
    /// or on nothing.
    #[test]
    fn the_window_shows_the_fixtures_chords_and_every_other_commands_default() {
        let text = std::fs::read_to_string(FIXTURE).expect(FIXTURE);
        let rebinding = table(&text);
        let shortcuts = read(&rebinding);
        assert_eq!(shortcuts.refusals, Vec::new());
        let rows = labelled(&sections(&shortcuts.chords));
        // The rows are found by title below, which only says what it means
        // while no two Commands share one.
        let titles: std::collections::BTreeSet<&str> =
            COMMANDS.iter().map(|command| command.title).collect();
        assert_eq!(titles.len(), COMMANDS.len());
        for command in COMMANDS {
            let expected = match rebinding.get(command.id) {
                Some(entry) => entry.as_array().expect(command.id)[0]
                    .as_str()
                    .expect(command.id)
                    .to_owned(),
                None => command.accels().first().cloned().unwrap_or_default(),
            };
            let shown: Vec<&String> = rows
                .iter()
                .filter(|(title, _)| *title == command.title)
                .map(|(_, accelerator)| accelerator)
                .collect();
            assert!(!shown.is_empty(), "{} has no row", command.id);
            for accelerator in shown {
                assert_eq!(*accelerator, expected, "{}", command.id);
            }
        }
        assert_eq!(
            rows.len(),
            COMMANDS.len() + 1,
            "one row each, and `chrome.stats` in both the menus it is placed in"
        );
        assert_eq!(
            rows.iter()
                .find(|(title, _)| *title == commands::by_id("library.toggle").unwrap().title),
            Some(&("Show Library / Hide Library", "F9".to_owned()))
        );
    }

    /// The sections are the menus, the View menu's are its six subsections,
    /// and the Commands with no menu row are the last section of all.
    #[test]
    fn the_sections_are_the_menus_and_the_view_menus_groups_its_subsections() {
        let sections = sections(&read(&toml::Table::new()).chords);
        assert_eq!(
            sections
                .iter()
                .map(|section| section.title)
                .collect::<Vec<_>>(),
            ["Document", "View", "Stats", KEYBOARD_ONLY]
        );
        let view = &sections[1];
        assert_eq!(
            view.groups
                .iter()
                .map(|group| group.title)
                .collect::<Vec<_>>(),
            commands::VIEW_SECTIONS.map(Some)
        );
        for section in sections.iter().filter(|section| section.title != "View") {
            assert_eq!(section.groups.len(), 1, "{}", section.title);
            assert_eq!(section.groups[0].title, None, "{}", section.title);
        }
        // A Command on no chord is a row all the same, with nothing in the
        // accelerator: the window is the whole table.
        assert_eq!(
            labelled(&sections)
                .iter()
                .find(|(title, _)| *title == commands::by_id("file.duplicate").unwrap().title),
            Some(&("Duplicate Document", String::new()))
        );
    }
}
