//! The three menus — Document, View and Stats — as `GMenu` models built from
//! the Command registry, so a section's order and a row's label are one edit
//! in `docs/shortcuts.md`.
//!
//! A model is a pure function of the registry and the modes: the rows are
//! the Commands placed in that menu, in the table's order; the View menu's
//! sections are `GMenu` sections, which `GtkPopoverMenu` draws with a
//! separator between them; a Writing tools head named in [`SUBMENU_HEADS`]
//! draws the rows of its own Command prefix as a nested submenu under it, the
//! Syntax highlight rows being the one such head today, and the Template
//! section is a nested submenu of its own
//! name. The Document menu's five `export.` rows are a nested submenu of
//! their own name too, and Print… stands in a section of its own beneath it,
//! which is the separator on each side of it. A row's action is the Command's,
//! so the check or the radio the popover draws reads the stateful action the
//! chord fires, and a Command not built yet has a disabled action, which is
//! the greyed row.
//! Document → Open Recent is the one row no Command places: a submenu of the
//! recents the caller hands over, appended after Open File… and opening each
//! Document through [`chrome::RECENT_OPEN`] (#246). The accelerator
//! label is GTK's own rendering of the first chord the Command is
//! installed with now ([`chrome::accels`]) rather than of the registry's own,
//! handed over in GTK's syntax as the row's `accel`, so a row rebound in
//! `settings.toml` is labelled the way the writer rebound it (#124).

use std::path::PathBuf;

use gtk::gio;
use gtk::prelude::*;
use quill_engine::commands::{COMMANDS, Command, Kind, Menu, Placement, VIEW_SECTIONS};
use quill_engine::document::shown_name;

use crate::chrome::{self, Modes, RECENT_OPEN};

/// The View menu's submenu heads, each keyed by the Command prefix that folds
/// under it: a head is itself a Command, and every other Command sharing its
/// prefix is one of the submenu's rows, in the table's order. Syntax
/// highlight and Style check are the two Annotators with a head, and a third
/// is a third row here and no other edit in this module (#362).
const SUBMENU_HEADS: &[(&str, &str)] = &[("syntax.", "syntax.toggle"), ("style.", "style.toggle")];
/// The section the Parity oracle heads with a label; the rest read as groups
/// between separators.
const HEADED: &str = "Typeface";
/// The section that is a submenu of its own name rather than rows between
/// separators: eight rows is a menu's worth, and a Template is picked once and
/// left (`docs/shortcuts.md` § View menu).
const SUBMENU: &str = "Template";
/// The Document menu's rows that fold into a submenu of their own name the
/// same way, by the prefix their ids share: five sinks is a menu's worth, and
/// a writer reaches for the File menu to save far oftener than to export
/// (`docs/shortcuts.md` § Document menu).
const EXPORT_PREFIX: &str = "export.";
/// What the row that opens that submenu reads.
const EXPORT_HEAD: &str = "Export";
/// The Document menu's row that stands alone between two separators:
/// printing a Document is not exporting one, so it is not in the submenu, and
/// it is not one of the file operations above it either.
const PRINT: &str = "print";
/// The Stats menu's last row, the one that hides the bar.
const HIDE_STATS: &str = "chrome.stats";
/// The row Open Recent opens under, where a writer looks for it: under the
/// other way of reaching a Document that is not in the Library.
const RECENT_AFTER: &str = "file.open";
/// What that row reads.
const RECENT_HEAD: &str = "Open Recent";
/// How many Documents it lists: the ten newest (#246, story 42). The whole of
/// the state's recents is the Palette's list, which narrows as it is typed
/// into; a menu that cannot be typed into is a short one.
const RECENT_ROWS: usize = 10;

/// The model for `menu`, its labels read the way `modes` says they read now
/// and its Open Recent submenu the Documents in `recents`, newest first.
///
/// `recents` is handed over rather than read from the session because a model
/// is a pure function of what it is given, which is what lets a test build
/// every menu with no session and no display.
#[must_use]
pub fn model(menu: Menu, modes: &Modes, recents: &[PathBuf]) -> gio::Menu {
    let model = gio::Menu::new();
    match menu {
        Menu::Document => {
            // Three sections, so the popover draws a separator on each side
            // of Print…: the file operations and the Export submenu, then
            // Print… alone, then the two rows that end the menu.
            let exports = gio::Menu::new();
            for (command, placement) in rows(menu).filter(|(command, _)| is_export(command)) {
                exports.append_item(&item(command, placement, modes));
            }
            let files = gio::Menu::new();
            let printing = gio::Menu::new();
            let closing = gio::Menu::new();
            let mut printed = false;
            let mut opened = false;
            for (command, placement) in rows(menu) {
                if is_export(command) {
                    // The row that opens the submenu stands where the first
                    // of its rows stands in the table.
                    if !opened {
                        files.append_item(&gio::MenuItem::new_submenu(Some(EXPORT_HEAD), &exports));
                        opened = true;
                    }
                    continue;
                }
                if command.id == PRINT {
                    printing.append_item(&item(command, placement, modes));
                    printed = true;
                    continue;
                }
                let into = if printed { &closing } else { &files };
                into.append_item(&item(command, placement, modes));
                // A writer who has opened nothing yet gets no row rather than
                // an empty one that opens on nothing.
                if command.id == RECENT_AFTER && !recents.is_empty() {
                    let submenu = recent_menu(recents);
                    into.append_item(&gio::MenuItem::new_submenu(Some(RECENT_HEAD), &submenu));
                }
            }
            model.append_section(None, &files);
            model.append_section(None, &printing);
            model.append_section(None, &closing);
        }
        Menu::Stats => {
            // The table's § Stats menu: the radios, then "the last row hides
            // the bar". The registry keeps `chrome.stats` where the table
            // first names it, under View › Window, so its Stats row is
            // taken out of the table's order here and put last, in a
            // section of its own so a separator stands before it.
            let fields = gio::Menu::new();
            let hide = gio::Menu::new();
            for (command, placement) in rows(menu) {
                let into = if command.id == HIDE_STATS {
                    &hide
                } else {
                    &fields
                };
                into.append_item(&item(command, placement, modes));
            }
            model.append_section(None, &fields);
            model.append_section(None, &hide);
        }
        Menu::View => {
            for section in VIEW_SECTIONS {
                let rows: Vec<_> = rows(menu)
                    .filter(|(_, placement)| placement.section == Some(section))
                    .collect();
                let heading = (section == HEADED).then(|| section.to_uppercase());
                let built = if section == SUBMENU {
                    template_section(section, &rows, modes)
                } else {
                    view_section(&rows, modes)
                };
                model.append_section(heading.as_deref(), &built);
            }
        }
    }
    model
}

/// The Open Recent submenu: the [`RECENT_ROWS`] newest Documents, each under
/// the name the top bar would show it by and each opening it in this window
/// through [`RECENT_OPEN`], the one window action outside the registry.
fn recent_menu(recents: &[PathBuf]) -> gio::Menu {
    let menu = gio::Menu::new();
    let action = format!("win.{RECENT_OPEN}");
    for path in recents.iter().take(RECENT_ROWS) {
        let row = gio::MenuItem::new(Some(&mnemonic_free(&shown_name(path))), None);
        let target = path.to_string_lossy().into_owned();
        row.set_action_and_target_value(Some(&action), Some(&target.to_variant()));
        menu.append_item(&row);
    }
    menu
}

/// `name` as a menu label: every `_` doubled.
///
/// A GMenu label is read for a mnemonic, so a single underscore is taken as
/// the marker in front of the letter to underline and is not drawn at all —
/// `sea_storm` would read as "seastorm" with the `s` underlined. The rows
/// built from a Command's own label say what they mean and are left as they
/// are written; a row named after a writer's file has to be escaped, because
/// the writer named it and not us.
fn mnemonic_free(name: &str) -> String {
    name.replace('_', "__")
}

/// The View menu's rows of one section, each [`SUBMENU_HEADS`] head's rows
/// folded into its submenu.
fn view_section(rows: &[(&'static Command, &'static Placement)], modes: &Modes) -> gio::Menu {
    section_with_heads(SUBMENU_HEADS, rows, modes)
}

/// The same against a head table of the caller's, which is how the tests read
/// a second head without a second Annotator's Commands.
///
/// A head draws its own check first, then its rows in the table's order; a row
/// whose head has not been drawn yet, and a row under no head at all, stays
/// where the Commands table put it.
fn section_with_heads(
    heads: &'static [(&'static str, &'static str)],
    rows: &[(&'static Command, &'static Placement)],
    modes: &Modes,
) -> gio::Menu {
    let section = gio::Menu::new();
    let mut folded: Vec<&'static str> = Vec::new();
    for (command, placement) in rows {
        if let Some((prefix, _)) = heads.iter().find(|(_, head)| command.id == *head) {
            let submenu = gio::Menu::new();
            submenu.append_item(&item(command, placement, modes));
            let kinds = gio::Menu::new();
            for (member, placement) in rows
                .iter()
                .filter(|(member, _)| folds_under(member, heads) == Some(*prefix))
            {
                kinds.append_item(&item(member, placement, modes));
            }
            submenu.append_section(None, &kinds);
            section.append_item(&gio::MenuItem::new_submenu(Some(placement.label), &submenu));
            folded.push(prefix);
        } else if !folds_under(command, heads).is_some_and(|prefix| folded.contains(&prefix)) {
            section.append_item(&item(command, placement, modes));
        }
    }
    section
}

/// The [`SUBMENU`] section as one row that opens a submenu: the five Template
/// radios, a separator, then the three toggles that bend the one chosen.
///
/// A Writing tools submenu hangs off a head row that is itself a Command
/// ([`SUBMENU_HEADS`]); this one has no head — the section's own name is the
/// row — because a Template is not a thing to switch on and off.
fn template_section(
    name: &'static str,
    rows: &[(&'static Command, &'static Placement)],
    modes: &Modes,
) -> gio::Menu {
    let section = gio::Menu::new();
    let submenu = gio::Menu::new();
    let templates = gio::Menu::new();
    let toggles = gio::Menu::new();
    for (command, placement) in rows {
        let into = if matches!(command.kind, Kind::Radio { .. }) {
            &templates
        } else {
            &toggles
        };
        into.append_item(&item(command, placement, modes));
    }
    submenu.append_section(None, &templates);
    submenu.append_section(None, &toggles);
    section.append_item(&gio::MenuItem::new_submenu(Some(name), &submenu));
    section
}

/// An `export.` Command: one of the Export submenu's rows.
fn is_export(command: &Command) -> bool {
    command.id.starts_with(EXPORT_PREFIX)
}

/// The prefix of the `heads` head `command` folds under: the prefix it shares
/// with a head without being that head. A head itself, and a Command of no
/// head's prefix, fold under nothing.
fn folds_under(
    command: &Command,
    heads: &'static [(&'static str, &'static str)],
) -> Option<&'static str> {
    heads
        .iter()
        .find(|(prefix, head)| command.id != *head && command.id.starts_with(*prefix))
        .map(|(prefix, _)| *prefix)
}

/// The Commands placed in `menu`, each with that placement, in the table's
/// order.
fn rows(menu: Menu) -> impl Iterator<Item = (&'static Command, &'static Placement)> {
    COMMANDS.iter().flat_map(move |command| {
        command
            .placements
            .iter()
            .filter(move |placement| placement.menu == menu)
            .map(move |placement| (command, placement))
    })
}

/// One row: the placement's label, the Command's action and its first
/// chord.
fn item(command: &Command, placement: &Placement, modes: &Modes) -> gio::MenuItem {
    let item = gio::MenuItem::new(Some(&label(command, placement, modes)), None);
    let (action, target) = command.action_and_target();
    item.set_action_and_target_value(Some(&action), target.map(ToVariant::to_variant).as_ref());
    if let Some(accel) = chrome::accels(command).first() {
        item.set_attribute_value("accel", Some(&accel.to_variant()));
    }
    item
}

/// The row's label now: a label the table writes as a pair, `Hide Bars /
/// Show Bars`, reads the half the state calls for, the way the oracle's rows
/// do; any other label is the placement's.
#[must_use]
pub fn label(command: &Command, placement: &Placement, modes: &Modes) -> String {
    half(command, placement.label, modes)
}

/// The Command's title now, by the same rule: what the Palette lists it as.
#[must_use]
pub fn title(command: &Command, modes: &Modes) -> String {
    half(command, command.title, modes)
}

/// The half of a paired `text` the modes call for, or all of an unpaired one.
fn half(command: &Command, text: &str, modes: &Modes) -> String {
    let Some((first, second)) = text.split_once(" / ") else {
        return text.to_string();
    };
    // A pair reads whole where nothing the window holds picks a half.
    // `file.pin` acts on whichever row the Library has selected before it acts
    // on the Document — the pane's state, not the window's — so neither Pin
    // nor Unpin is the one true label and the row offers both.
    if command.id == "file.pin" {
        return text.to_string();
    }
    // The second half is the way back: Disable once Focus is on, Show once
    // the bars are hidden. A pane not built yet is closed, so its row offers
    // to open it.
    let second_now = match command.id {
        "focus.toggle" => modes.focus,
        "chrome.toggle" => !modes.bars,
        _ => false,
    };
    if second_now { second } else { first }.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use gtk::glib;
    use quill_engine::commands::{Scope, by_id};

    /// One row as the popover would draw it, read back off the model.
    #[derive(Debug, PartialEq, Eq)]
    struct Row {
        label: String,
        action: Option<String>,
        target: Option<String>,
        accel: Option<String>,
    }

    /// Every row of `model` in drawing order, sections and submenus
    /// flattened; a section boundary is a `None`.
    fn rows_of(model: &gio::MenuModel) -> Vec<Option<Row>> {
        let mut out = Vec::new();
        for i in 0..model.n_items() {
            if let Some(section) = model.item_link(i, "section") {
                if i > 0 {
                    out.push(None);
                }
                out.extend(rows_of(&section));
                continue;
            }
            if let Some(submenu) = model.item_link(i, "submenu") {
                // A submenu's rows draw inside the row that opens it, so its
                // own sections are no boundary of this model's.
                out.extend(rows_of(&submenu).into_iter().flatten().map(Some));
                continue;
            }
            let string = |attribute: &str| {
                model
                    .item_attribute_value(i, attribute, Some(glib::VariantTy::STRING))
                    .and_then(|value| value.get::<String>())
            };
            out.push(Some(Row {
                label: string("label").unwrap_or_default(),
                action: string("action"),
                target: string("target"),
                accel: string("accel"),
            }));
        }
        out
    }

    fn labels(model: &gio::Menu) -> Vec<String> {
        rows_of(model.upcast_ref())
            .into_iter()
            .flatten()
            .map(|row| row.label)
            .collect()
    }

    fn placed(menu: Menu) -> Vec<&'static str> {
        rows(menu).map(|(_, placement)| placement.label).collect()
    }

    /// The modes of a window at rest: bars up, Focus off.
    fn resting() -> Modes {
        Modes {
            bars: true,
            stats: true,
            ..Modes::default()
        }
    }

    /// The Documents a state would hand the menu, newest first.
    fn opened(count: usize) -> Vec<PathBuf> {
        (0..count)
            .map(|i| PathBuf::from(format!("/home/w/draft-{i}.md")))
            .collect()
    }

    /// The Document menu is the table's rows in the table's order, New
    /// Document first and Quit last, and with no recents it is only those.
    /// The five Export rows draw inside the submenu that stands where the
    /// first of them stands, so flattened they are still in the table's
    /// order.
    #[test]
    fn the_document_menu_is_the_tables_rows_in_the_tables_order() {
        let labels = labels(&model(Menu::Document, &Modes::default(), &[]));
        assert_eq!(labels, placed(Menu::Document));
        assert_eq!(labels.first().map(String::as_str), Some("New Document"));
        assert_eq!(labels.last().map(String::as_str), Some("Quit"));
    }

    /// The five Export rows are one submenu of their own name, standing where
    /// the first of them stands in the table, and Print… is a section of its
    /// own beneath it — a separator on each side.
    #[test]
    fn the_export_rows_are_a_submenu_and_print_stands_alone_beneath_it() {
        let model = model(Menu::Document, &Modes::default(), &[]);
        assert_eq!(model.n_items(), 3, "three sections, two separators");
        let files = model.item_link(0, "section").expect("the file operations");
        let at = files.n_items() - 1;
        let label = files
            .item_attribute_value(at, "label", Some(glib::VariantTy::STRING))
            .and_then(|value| value.get::<String>());
        assert_eq!(label.as_deref(), Some(EXPORT_HEAD), "the last row opens it");
        let submenu = files.item_link(at, "submenu").expect("the submenu");
        let inside: Vec<String> = rows_of(&submenu)
            .into_iter()
            .flatten()
            .map(|row| row.label)
            .collect();
        assert_eq!(
            inside,
            [
                "PDF…",
                "HTML…",
                "Markdown…",
                "Quick Export PDF",
                "Copy as HTML"
            ]
        );
        let printing = model.item_link(1, "section").expect("the print section");
        assert_eq!(printing.n_items(), 1, "Print… is the whole section");
        let rows = rows_of(&printing);
        let row = rows[0].as_ref().unwrap();
        assert_eq!(row.label, "Print…");
        assert_eq!(row.action.as_deref(), Some("win.print"));
        let closing = model.item_link(2, "section").expect("the closing rows");
        let after: Vec<String> = rows_of(&closing)
            .into_iter()
            .flatten()
            .map(|row| row.label)
            .collect();
        assert_eq!(after, ["Close Window", "Quit"]);
    }

    /// Print… is built and stands in the Document menu under the Export rows
    /// rather than inside the submenu, which is the separator the spec asks
    /// for; #290 built the Command behind the row.
    #[test]
    fn print_stands_built_below_the_export_rows() {
        let print = by_id(PRINT).expect(PRINT);
        assert!(print.built, "{PRINT} is not built");
        assert!(!is_export(print), "{PRINT} is inside the Export submenu");
        for id in ["export.pdf", "export.html", "export.markdown"] {
            assert!(by_id(id).expect(id).built, "{id} is not built");
        }
    }

    /// The two Commands that need no dialog are built, and each is a row of
    /// the Export submenu rather than a Palette-only id; #287 built them.
    #[test]
    fn quick_export_and_copy_as_html_are_built_in_the_export_submenu() {
        for (id, label) in [
            ("export.quick", "Quick Export PDF"),
            ("export.copyHtml", "Copy as HTML"),
        ] {
            let command = by_id(id).expect(id);
            assert!(command.built, "{id} is not built");
            assert!(is_export(command), "{id} is not an Export row");
            assert_eq!(
                command
                    .placements
                    .iter()
                    .map(|placement| (placement.menu, placement.label))
                    .collect::<Vec<_>>(),
                [(Menu::Document, label)],
                "{id} stands in the Document menu under its own label"
            );
        }
    }

    /// Open Recent is a submenu under Open File…, holding the recents by the
    /// name the top bar shows them by, each opening its own path.
    #[test]
    fn open_recent_is_a_submenu_under_open_file_naming_each_documents_path() {
        let recents = opened(2);
        let model = model(Menu::Document, &Modes::default(), &recents);
        let mut expected: Vec<String> = placed(Menu::Document)
            .into_iter()
            .map(str::to_string)
            .collect();
        let at = expected
            .iter()
            .position(|label| label == "Open File…")
            .unwrap()
            + 1;
        expected.splice(at..at, ["draft-0".to_string(), "draft-1".to_string()]);
        assert_eq!(labels(&model), expected, "the submenu's rows draw in place");
        let files = model.item_link(0, "section").expect("the file operations");
        let submenu = files.item_link(3, "submenu").expect("Open Recent");
        let rows = rows_of(&submenu);
        assert_eq!(rows.len(), 2);
        let row = rows[0].as_ref().unwrap();
        assert_eq!(row.action.as_deref(), Some("win.file.recentOpen"));
        assert_eq!(row.target.as_deref(), Some("/home/w/draft-0.md"));
        assert_eq!(row.accel, None);
    }

    /// An underscore in a file's name is drawn, not eaten as a mnemonic.
    #[test]
    fn an_underscore_in_a_recents_name_is_drawn_rather_than_underlining_a_letter() {
        let path = PathBuf::from("/home/w/sea_storm_two.md");
        let model = model(
            Menu::Document,
            &Modes::default(),
            std::slice::from_ref(&path),
        );
        let files = model.item_link(0, "section").expect("the file operations");
        let submenu = files.item_link(3, "submenu").expect("Open Recent");
        let row = rows_of(&submenu).remove(0).expect("the one recent");
        assert_eq!(
            row.label, "sea__storm__two",
            "each underscore of the name is doubled, which is one underscore drawn"
        );
        assert_eq!(row.target.as_deref(), path.to_str());
    }

    /// It lists the ten newest and no more, however many the state holds.
    #[test]
    fn open_recent_lists_the_ten_newest_and_no_more() {
        let recents = opened(25);
        let model = model(Menu::Document, &Modes::default(), &recents);
        let files = model.item_link(0, "section").expect("the file operations");
        let submenu = files.item_link(3, "submenu").expect("Open Recent");
        let names: Vec<String> = rows_of(&submenu)
            .into_iter()
            .flatten()
            .map(|row| row.label)
            .collect();
        assert_eq!(names.len(), RECENT_ROWS);
        assert_eq!(names.first().map(String::as_str), Some("draft-0"));
        assert_eq!(names.last().map(String::as_str), Some("draft-9"));
    }

    /// The View menu is one section per section of the table with a separator
    /// between each pair, Focus first and All Commands… last; every placed row
    /// is there once, with the pairs read for the modes given.
    #[test]
    fn the_view_menu_is_the_tables_sections_focus_first_and_all_commands_last() {
        let model = model(Menu::View, &resting(), &[]);
        let rows = rows_of(model.upcast_ref());
        assert_eq!(
            rows.iter().filter(|row| row.is_none()).count(),
            VIEW_SECTIONS.len() - 1
        );
        let labels: Vec<String> = rows.into_iter().flatten().map(|row| row.label).collect();
        let expected: Vec<String> = placed(Menu::View)
            .into_iter()
            .map(|label| {
                label
                    .split_once(" / ")
                    .map_or(label, |(first, _)| first)
                    .to_string()
            })
            .collect();
        assert_eq!(labels, expected);
        assert_eq!(
            labels.first().map(String::as_str),
            Some("Enable Focus Mode")
        );
        assert_eq!(labels.last().map(String::as_str), Some("All Commands…"));
    }

    /// The View menu's sections come in the table's order, and only the
    /// Typeface section carries a heading, as the oracle's does.
    #[test]
    fn the_view_sections_are_the_tables_and_typeface_alone_is_headed() {
        let model = model(Menu::View, &Modes::default(), &[]);
        assert_eq!(model.n_items(), i32::try_from(VIEW_SECTIONS.len()).unwrap());
        for (i, section) in VIEW_SECTIONS.iter().enumerate() {
            let i = i32::try_from(i).unwrap();
            let heading = model
                .item_attribute_value(i, "label", Some(glib::VariantTy::STRING))
                .and_then(|value| value.get::<String>());
            let expected = (*section == HEADED).then(|| section.to_uppercase());
            assert_eq!(heading, expected, "section {section}");
            assert!(model.item_link(i, "section").is_some(), "section {section}");
        }
    }

    /// Each Annotator's rows are a submenu under their head: the head's own
    /// check first, then its kinds or Lists, and none of them loose in the
    /// section. Spell check, which no head claims yet, stays a loose row.
    #[test]
    fn every_head_takes_its_own_rows_into_a_submenu() {
        let model = model(Menu::View, &Modes::default(), &[]);
        let tools = model.item_link(2, "section").expect("Writing tools");
        let loose: Vec<String> = (0..tools.n_items())
            .filter_map(|i| {
                tools
                    .item_attribute_value(i, "label", Some(glib::VariantTy::STRING))
                    .and_then(|value| value.get::<String>())
            })
            .collect();
        assert_eq!(loose, ["Syntax Highlight", "Style Check", "Spell Check"]);
        let inside = |at: i32| -> Vec<String> {
            let submenu = tools.item_link(at, "submenu").expect("the submenu");
            rows_of(&submenu)
                .into_iter()
                .flatten()
                .map(|row| row.label)
                .collect()
        };
        assert_eq!(
            inside(0),
            [
                "Syntax Highlight",
                "Nouns",
                "Verbs",
                "Adjectives",
                "Adverbs",
                "Conjunctions"
            ]
        );
        assert_eq!(
            inside(1),
            ["Style Check", "Fillers", "Redundancies", "Clichés"]
        );
    }

    /// A second head in the table folds its own prefix's rows under it and
    /// leaves a row of neither prefix where the Commands table put it, which
    /// is the shape a second Annotator's toggles land in (#362). Read on the
    /// Focus section, whose `focus.` rows no head claims on the page: under a
    /// table that heads them, Sentence and Paragraph draw inside the head's
    /// submenu and Typewriter and Live stay loose beneath it.
    #[test]
    fn a_second_head_folds_its_own_rows_and_leaves_a_headless_row_in_place() {
        const HEADS: &[(&str, &str)] = &[("syntax.", "syntax.toggle"), ("focus.", "focus.toggle")];
        let focus: Vec<_> = rows(Menu::View)
            .filter(|(_, placement)| placement.section == Some("Focus"))
            .collect();
        let section = section_with_heads(HEADS, &focus, &Modes::default());
        let loose: Vec<String> = (0..section.n_items())
            .filter_map(|i| {
                section
                    .item_attribute_value(i, "label", Some(glib::VariantTy::STRING))
                    .and_then(|value| value.get::<String>())
            })
            .collect();
        // The row that opens a submenu carries the placement's label whole,
        // which is why a head whose label is a pair reads as both halves; no
        // head the table names is written as a pair.
        assert_eq!(
            loose,
            [
                "Enable Focus Mode / Disable Focus Mode",
                "Typewriter",
                "Live"
            ]
        );
        let submenu = section.item_link(0, "submenu").expect("the submenu");
        let inside: Vec<String> = rows_of(&submenu)
            .into_iter()
            .flatten()
            .map(|row| row.label)
            .collect();
        assert_eq!(inside, ["Enable Focus Mode", "Sentence", "Paragraph"]);
    }

    /// The Template section is one row that opens a submenu named for it: the
    /// five Templates, then the three toggles, and none of the eight loose in
    /// the View menu.
    #[test]
    fn the_template_rows_are_a_submenu_named_for_their_section() {
        let model = model(Menu::View, &Modes::default(), &[]);
        let at = i32::try_from(
            VIEW_SECTIONS
                .iter()
                .position(|section| *section == SUBMENU)
                .expect("the table has the section"),
        )
        .unwrap();
        let section = model.item_link(at, "section").expect("the section");
        assert_eq!(section.n_items(), 1, "one row, which opens the submenu");
        let label = section
            .item_attribute_value(0, "label", Some(glib::VariantTy::STRING))
            .and_then(|value| value.get::<String>());
        assert_eq!(label.as_deref(), Some(SUBMENU));
        let submenu = section.item_link(0, "submenu").expect("the submenu");
        // Two sections inside it, so the popover draws a separator between
        // the Templates and the toggles that bend one.
        assert_eq!(submenu.n_items(), 2);
        let inside: Vec<String> = rows_of(&submenu)
            .into_iter()
            .flatten()
            .map(|row| row.label)
            .collect();
        assert_eq!(
            inside,
            [
                "Modern",
                "Classic",
                "Manuscript Mono",
                "Manuscript Duo",
                "Manuscript Quattro",
                "Center Headings",
                "Number Headings",
                "Indent Paragraphs"
            ]
        );
    }

    /// The Stats menu is the six radios, a separator, then Hide Statistics —
    /// the table's § Stats menu order, though the registry names
    /// `chrome.stats` first.
    #[test]
    fn the_stats_menu_is_the_six_fields_then_hide_statistics() {
        let model = model(Menu::Stats, &Modes::default(), &[]);
        let rows = rows_of(model.upcast_ref());
        assert_eq!(rows.iter().filter(|row| row.is_none()).count(), 1);
        let labels: Vec<String> = rows.into_iter().flatten().map(|row| row.label).collect();
        let mut expected = placed(Menu::Stats);
        expected.rotate_left(1);
        assert_eq!(labels, expected);
        assert_eq!(labels.len(), 7);
        assert_eq!(labels.first().map(String::as_str), Some("Words"));
        assert_eq!(labels.last().map(String::as_str), Some("Hide Statistics"));
    }

    /// Every row's action is its Command's, a radio's the group with the
    /// member's value as target, and its accel the first chord in GTK's
    /// syntax — from the table and nowhere else.
    #[test]
    fn every_row_names_its_commands_action_and_first_chord() {
        for menu in [Menu::Document, Menu::View, Menu::Stats] {
            let modes = resting();
            let drawn: Vec<Row> = rows_of(model(menu, &modes, &[]).upcast_ref())
                .into_iter()
                .flatten()
                .collect();
            let mut placed: Vec<_> = rows(menu).collect();
            if menu == Menu::Stats {
                placed.rotate_left(1);
            }
            assert_eq!(drawn.len(), placed.len(), "{menu:?}");
            for (row, (command, placement)) in drawn.into_iter().zip(placed) {
                assert_eq!(row.label, label(command, placement, &modes));
                let (action, target) = command.action_and_target();
                let target = target.map(String::from);
                assert_eq!(
                    row.action.as_deref(),
                    Some(action.as_str()),
                    "{}",
                    command.id
                );
                assert_eq!(row.target, target, "{}", command.id);
                assert_eq!(
                    row.accel,
                    chrome::accels(command).first().cloned(),
                    "{}",
                    command.id
                );
            }
        }
    }

    /// The paired labels read for the state: Focus on says Disable, the
    /// bars hidden say Show.
    #[test]
    fn a_paired_label_reads_the_half_the_modes_call_for() {
        let focus = by_id("focus.toggle").unwrap();
        let bars = by_id("chrome.toggle").unwrap();
        let off = Modes::default();
        let on = Modes {
            focus: true,
            bars: true,
            ..Modes::default()
        };
        assert_eq!(
            label(focus, &focus.placements[0], &off),
            "Enable Focus Mode"
        );
        assert_eq!(
            label(focus, &focus.placements[0], &on),
            "Disable Focus Mode"
        );
        assert_eq!(label(bars, &bars.placements[0], &off), "Show Bars");
        assert_eq!(label(bars, &bars.placements[0], &on), "Hide Bars");
        let quit = by_id("app.quit").unwrap();
        assert_eq!(quit.scope, Scope::App);
        assert_eq!(label(quit, &quit.placements[0], &off), "Quit");
    }

    /// `file.pin` is the one pair the modes cannot pick a half of: what it
    /// pins is the pane's selected row before it is the window's Document.
    #[test]
    fn the_pin_pair_reads_whole_because_the_modes_do_not_hold_which_row_it_means() {
        let pin = by_id("file.pin").unwrap();
        assert_eq!(title(pin, &Modes::default()), "Pin / Unpin");
        assert!(pin.placements.is_empty(), "the Palette lists it, no menu");
    }
}
