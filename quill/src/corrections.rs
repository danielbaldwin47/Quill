//! Spell check's corrections: the section at the top of the context menu for
//! the misspelled word at the caret, and the `spell` action group its rows
//! fire.
//!
//! The section is built at two moments and no other: a secondary press on the
//! Editor, captured before the text view's own handling, and the keyboard's
//! chord for the same menu (Menu, `Shift+F10`), captured the same way — so a
//! menu opened either way reads the same word. A caret move never builds it,
//! which keeps [`quill_engine::spell::SpellChecker::suggest`], the one call the
//! main thread makes on the shared checker, off the keystroke lane (#401
//! § Corrections). What it is built from is the Editor's to find
//! ([`crate::editor::Editor::correct`]); this module is the model and the
//! actions, and needs no display.
//!
//! The section opens at the top of a menu of the Editor's own that repeats
//! GTK's rows beneath it ([`menu`]), because GTK only ever appends an extra
//! menu at the bottom. A word that is not misspelled opens GTK's menu, as
//! GTK draws it.

use gtk::gio;
use gtk::glib;
use gtk::prelude::*;

/// The action group's name on the Editor.
pub const GROUP: &str = "spell";
/// Replaces the word with the suggestion the row carries as its target.
pub const REPLACE: &str = "replace";
/// Writes the word into the Personal dictionary.
pub const ADD: &str = "add";
/// Leaves the word unmarked for the rest of this process.
pub const IGNORE: &str = "ignore";

/// What the Add row reads (`CONTEXT.md`, Spell check).
pub const ADD_LABEL: &str = "Add to Dictionary";
/// What the Ignore row reads.
pub const IGNORE_LABEL: &str = "Ignore";

/// The section for `word`: up to [`quill_engine::spell::SUGGESTIONS`]
/// Suggestion rows, then Add to Dictionary, then Ignore.
///
/// A Suggestion row carries the suggestion as its target and the other two
/// carry `word`, so an action fired from the menu needs nothing remembered
/// about which word it was built for. An empty `word` is an empty model.
pub fn section(word: &str, suggestions: &[String]) -> gio::Menu {
    let menu = gio::Menu::new();
    if word.is_empty() {
        return menu;
    }
    for suggestion in suggestions.iter().take(quill_engine::spell::SUGGESTIONS) {
        menu.append_item(&row(suggestion, REPLACE, suggestion));
    }
    menu.append_item(&row(ADD_LABEL, ADD, word));
    menu.append_item(&row(IGNORE_LABEL, IGNORE, word));
    menu
}

/// GTK's own context menu for a text view as GTK 4.22 draws it — Cut, Copy,
/// Paste and Delete, then Undo and Redo, then Select All and Insert Emoji —
/// each row on the text view's own action and accelerator label.
const EDITING: &[&[(&str, &str, Option<&str>)]] = &[
    &[
        ("Cu_t", "clipboard.cut", None),
        ("_Copy", "clipboard.copy", None),
        ("_Paste", "clipboard.paste", None),
        ("_Delete", "selection.delete", None),
    ],
    &[
        ("_Undo", "text.undo", Some("<Control>z")),
        ("_Redo", "text.redo", Some("<Shift><Control>z")),
    ],
    &[
        ("Select _All", "selection.select-all", None),
        (
            "Insert _Emoji",
            "misc.insert-emoji",
            Some("<Control>semicolon"),
        ),
    ],
];

/// The whole menu a misspelled word opens: its section on top, then GTK's
/// own rows beneath it.
///
/// `GtkTextView`'s extra menu is only ever appended below Insert Emoji, and
/// the section belongs above Cut, Copy and Paste (#401 § Further Notes, Hand
/// test step 3), so a misspelled word is given this menu in GTK's place. The
/// rows fire the text view's own actions, so each does and greys exactly as
/// it does in GTK's menu. A word that is not misspelled never reaches here:
/// GTK's own menu opens for it.
pub fn menu(word: &str, suggestions: &[String]) -> gio::Menu {
    let menu = gio::Menu::new();
    menu.append_section(None, &section(word, suggestions));
    for rows in EDITING {
        let part = gio::Menu::new();
        for (label, action, accel) in *rows {
            let item = gio::MenuItem::new(Some(label), Some(action));
            if let Some(accel) = accel {
                item.set_attribute_value("accel", Some(&accel.to_variant()));
            }
            part.append_item(&item);
        }
        menu.append_section(None, &part);
    }
    menu
}

/// One row firing `spell.<action>` with `target`.
fn row(label: &str, action: &str, target: &str) -> gio::MenuItem {
    let item = gio::MenuItem::new(Some(label), None);
    item.set_action_and_target_value(
        Some(&format!("{GROUP}.{action}")),
        Some(&target.to_variant()),
    );
    item
}

/// The three actions, each handing its string target to the closure it
/// belongs to: `replace` the suggestion, `add` and `ignore` the word.
pub fn actions(
    replace: impl Fn(&str) + 'static,
    add: impl Fn(&str) + 'static,
    ignore: impl Fn(&str) + 'static,
) -> gio::SimpleActionGroup {
    let group = gio::SimpleActionGroup::new();
    group.add_action(&action(REPLACE, replace));
    group.add_action(&action(ADD, add));
    group.add_action(&action(IGNORE, ignore));
    group
}

fn action(name: &str, then: impl Fn(&str) + 'static) -> gio::SimpleAction {
    let action = gio::SimpleAction::new(name, Some(glib::VariantTy::STRING));
    action.connect_activate(move |_, target| {
        if let Some(target) = target.and_then(|target| target.get::<String>()) {
            then(&target);
        }
    });
    action
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use super::*;

    /// Every row of `model` as (label, action, target).
    fn rows(model: &gio::Menu) -> Vec<(String, String, String)> {
        (0..model.n_items())
            .map(|i| {
                let string = |attribute: &str| {
                    model
                        .item_attribute_value(i, attribute, Some(glib::VariantTy::STRING))
                        .and_then(|value| value.get::<String>())
                        .unwrap_or_default()
                };
                (string("label"), string("action"), string("target"))
            })
            .collect()
    }

    fn owned(words: &[&str]) -> Vec<String> {
        words.iter().map(|word| (*word).to_owned()).collect()
    }

    /// Six suggestions give five rows, then Add to Dictionary, then Ignore,
    /// each firing its own action with its own target.
    #[test]
    fn a_word_with_six_suggestions_is_five_rows_then_add_then_ignore() {
        let suggestions = owned(&[
            "received", "relieved", "receive", "recited", "reviled", "rived",
        ]);
        let model = section("recieved", &suggestions);
        let replace = |word: &str| (word.to_owned(), "spell.replace".to_owned(), word.to_owned());
        assert_eq!(
            rows(&model),
            vec![
                replace("received"),
                replace("relieved"),
                replace("receive"),
                replace("recited"),
                replace("reviled"),
                (
                    "Add to Dictionary".to_owned(),
                    "spell.add".to_owned(),
                    "recieved".to_owned()
                ),
                (
                    "Ignore".to_owned(),
                    "spell.ignore".to_owned(),
                    "recieved".to_owned()
                ),
            ]
        );
    }

    /// A dictionary with nothing to offer still offers the word's two rows.
    #[test]
    fn a_word_with_no_suggestions_is_add_then_ignore() {
        let labels: Vec<_> = rows(&section("quillworthy", &[]))
            .into_iter()
            .map(|(label, ..)| label)
            .collect();
        assert_eq!(labels, ["Add to Dictionary", "Ignore"]);
    }

    /// The menu a misspelled word opens: its section first, then GTK's own
    /// rows in GTK's order and on GTK's actions — the section above Cut.
    #[test]
    fn the_section_stands_above_cut_copy_and_paste() {
        let model = menu("recieved", &owned(&["received"]));
        let sections: Vec<Vec<(String, String)>> = (0..model.n_items())
            .map(|i| {
                let section = model
                    .item_link(i, "section")
                    .expect("every item is a section");
                (0..section.n_items())
                    .map(|j| {
                        let string = |attribute: &str| {
                            section
                                .item_attribute_value(j, attribute, Some(glib::VariantTy::STRING))
                                .and_then(|value| value.get::<String>())
                                .unwrap_or_default()
                        };
                        (string("label"), string("action"))
                    })
                    .collect()
            })
            .collect();
        let pairs = |rows: &[(&str, &str)]| -> Vec<(String, String)> {
            rows.iter()
                .map(|(label, action)| ((*label).to_owned(), (*action).to_owned()))
                .collect()
        };
        assert_eq!(
            sections,
            [
                pairs(&[
                    ("received", "spell.replace"),
                    ("Add to Dictionary", "spell.add"),
                    ("Ignore", "spell.ignore"),
                ]),
                pairs(&[
                    ("Cu_t", "clipboard.cut"),
                    ("_Copy", "clipboard.copy"),
                    ("_Paste", "clipboard.paste"),
                    ("_Delete", "selection.delete"),
                ]),
                pairs(&[("_Undo", "text.undo"), ("_Redo", "text.redo")]),
                pairs(&[
                    ("Select _All", "selection.select-all"),
                    ("Insert _Emoji", "misc.insert-emoji"),
                ]),
            ]
        );
    }

    #[test]
    fn an_empty_word_is_an_empty_model() {
        assert_eq!(section("", &owned(&["a"])).n_items(), 0);
    }

    /// Each action hands its target to its own closure and no other.
    #[test]
    fn each_action_hands_its_target_to_its_own_closure() {
        let heard = Rc::new(RefCell::new(Vec::new()));
        let hear = |name: &'static str| {
            let heard = Rc::clone(&heard);
            move |word: &str| heard.borrow_mut().push(format!("{name} {word}"))
        };
        let group = actions(hear("replace"), hear("add"), hear("ignore"));
        group.activate_action(REPLACE, Some(&"received".to_variant()));
        group.activate_action(ADD, Some(&"Teh".to_variant()));
        group.activate_action(IGNORE, Some(&"accomodate".to_variant()));
        assert_eq!(
            *heard.borrow(),
            ["replace received", "add Teh", "ignore accomodate"]
        );
    }
}
