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

/// What a `[shortcuts]` table came to: the chords to install, and the entries
/// that were refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shortcuts {
    /// Every Command in the registry with the chords it should be installed
    /// with, in id order; empty for a Command that has none.
    pub chords: BTreeMap<&'static str, Vec<Chord>>,
    /// One per refused entry, in the file's order.
    pub refusals: Vec<Refusal>,
}

/// Reads a `[shortcuts]` table: the chords every Command has once the file is
/// over the defaults, and one refusal per entry that could not be applied.
#[must_use]
pub fn read(table: &toml::Table) -> Shortcuts {
    let (accepted, refusals) = validate(table);
    Shortcuts {
        chords: effective(COMMANDS, &accepted),
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
pub fn validate(table: &toml::Table) -> (BTreeMap<&'static str, Vec<Chord>>, Vec<Refusal>) {
    let mut accepted: BTreeMap<&'static str, Vec<Chord>> = BTreeMap::new();
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
pub fn effective(
    defaults: &[Command],
    accepted: &BTreeMap<&'static str, Vec<Chord>>,
) -> BTreeMap<&'static str, Vec<Chord>> {
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
}
