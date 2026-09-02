//! The three menus — Document, View and Stats — as `GMenu` models built from
//! the Command registry, so a section's order and a row's label are one edit
//! in `docs/shortcuts.md` (#121).
//!
//! A model is a pure function of the registry and the modes: the rows are
//! the Commands placed in that menu, in the table's order; the View menu's
//! six sections are `GMenu` sections, which `GtkPopoverMenu` draws with a
//! separator between them; the Syntax highlight rows are a nested submenu
//! under their head. A row's action is the Command's, so the check or the
//! radio the popover draws reads the stateful action the chord fires, and a
//! Command not built yet has a disabled action, which is the greyed row. The
//! accelerator label is GTK's own rendering of the first chord, handed over
//! in GTK's syntax as the row's `accel`.

use gtk::gio;
use gtk::prelude::*;
use quill_engine::commands::{COMMANDS, Command, Menu, Placement, VIEW_SECTIONS};

use crate::chrome::Modes;

/// The head of the Syntax highlight submenu, whose five rows are the
/// `syntax.` Commands that follow it in the table.
const SYNTAX_HEAD: &str = "syntax.toggle";
/// The section the Parity oracle heads with a label; the rest read as groups
/// between separators.
const HEADED: &str = "Typeface";
/// The Stats menu's last row, the one that hides the bar.
const HIDE_STATS: &str = "chrome.stats";

/// The model for `menu`, its labels read the way `modes` says they read now.
#[must_use]
pub fn model(menu: Menu, modes: &Modes) -> gio::Menu {
    let model = gio::Menu::new();
    match menu {
        Menu::Document => {
            for (command, placement) in rows(menu) {
                model.append_item(&item(command, placement, modes));
            }
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
                model.append_section(heading.as_deref(), &view_section(&rows, modes));
            }
        }
    }
    model
}

/// The View menu's rows of one section, the Syntax highlight rows folded
/// into their submenu.
fn view_section(rows: &[(&'static Command, &'static Placement)], modes: &Modes) -> gio::Menu {
    let section = gio::Menu::new();
    let mut folded = false;
    for (command, placement) in rows {
        if command.id == SYNTAX_HEAD {
            let submenu = gio::Menu::new();
            submenu.append_item(&item(command, placement, modes));
            let kinds = gio::Menu::new();
            for (member, placement) in rows.iter().filter(|(member, _)| is_syntax_kind(member)) {
                kinds.append_item(&item(member, placement, modes));
            }
            submenu.append_section(None, &kinds);
            section.append_item(&gio::MenuItem::new_submenu(Some(placement.label), &submenu));
            folded = true;
        } else if !(folded && is_syntax_kind(command)) {
            section.append_item(&item(command, placement, modes));
        }
    }
    section
}

/// A `syntax.` Command other than the head: one of the submenu's rows.
fn is_syntax_kind(command: &Command) -> bool {
    command.id != SYNTAX_HEAD && command.id.starts_with("syntax.")
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
    if let Some(accel) = command.accels().first() {
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

    /// The Document menu is the table's ten rows in the table's order, New
    /// Document first and Quit last.
    #[test]
    fn the_document_menu_is_the_tables_rows_in_the_tables_order() {
        let labels = labels(&model(Menu::Document, &Modes::default()));
        assert_eq!(labels, placed(Menu::Document));
        assert_eq!(labels.first().map(String::as_str), Some("New Document"));
        assert_eq!(labels.last().map(String::as_str), Some("Quit"));
    }

    /// The View menu is six sections with a separator between each pair,
    /// Focus first and All Commands… last; every placed row is there once,
    /// with the pairs read for the modes given.
    #[test]
    fn the_view_menu_is_six_sections_focus_first_and_all_commands_last() {
        let model = model(Menu::View, &resting());
        let rows = rows_of(model.upcast_ref());
        assert_eq!(rows.iter().filter(|row| row.is_none()).count(), 5);
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
        let model = model(Menu::View, &Modes::default());
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

    /// The Syntax highlight rows are a submenu under their head: the head's
    /// own check first, then the five kinds, and none of them loose in the
    /// section.
    #[test]
    fn the_syntax_rows_are_a_submenu_under_their_head() {
        let model = model(Menu::View, &Modes::default());
        let tools = model.item_link(2, "section").expect("Writing tools");
        let loose: Vec<String> = (0..tools.n_items())
            .filter_map(|i| {
                tools
                    .item_attribute_value(i, "label", Some(glib::VariantTy::STRING))
                    .and_then(|value| value.get::<String>())
            })
            .collect();
        assert_eq!(loose, ["Syntax Highlight", "Style Check", "Spell Check"]);
        let submenu = tools.item_link(0, "submenu").expect("the submenu");
        let inside: Vec<String> = rows_of(&submenu)
            .into_iter()
            .flatten()
            .map(|row| row.label)
            .collect();
        assert_eq!(
            inside,
            [
                "Syntax Highlight",
                "Nouns",
                "Verbs",
                "Adjectives",
                "Adverbs",
                "Conjunctions"
            ]
        );
    }

    /// The Stats menu is the six radios, a separator, then Hide Statistics —
    /// the table's § Stats menu order, though the registry names
    /// `chrome.stats` first.
    #[test]
    fn the_stats_menu_is_the_six_fields_then_hide_statistics() {
        let model = model(Menu::Stats, &Modes::default());
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
            let drawn: Vec<Row> = rows_of(model(menu, &modes).upcast_ref())
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
                    command.accels().first().cloned(),
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
}
