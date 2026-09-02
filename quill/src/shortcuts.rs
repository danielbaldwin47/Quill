//! The `Ctrl+?` window: every Command with the chord it is on now (#125).
//!
//! What it lists is [`quill_engine::shortcuts::sections`] — the registry
//! grouped as the menus are, over the writer's `[shortcuts]` table — so the
//! window is the whole of `docs/shortcuts.md` and not only the Commands that
//! have a chord. It is built on every open and dropped when it closes, which
//! is what makes a rebinding saved while Quill is running show the next time
//! the writer asks for it.
//!
//! `GtkShortcutsWindow` is GTK's own table of sections, groups and rows, and
//! it draws a chord the way the platform draws one. It is deprecated in 4.18
//! with nothing to replace it before GTK 5, which is why the two functions
//! below carry `#[allow(deprecated)]`.

use gtk::prelude::*;

use quill_engine::shortcuts::{Group, Section};

/// Opens the shortcuts window over `parent`, listing `sections`.
///
/// Transient and modal for the reason the Settings window is: it is a window
/// about the window it was opened from, and it goes away with it.
#[allow(deprecated)] // GtkShortcutsWindow: deprecated in 4.18, no replacement before GTK 5
pub fn open(parent: &gtk::Window, sections: &[Section]) {
    let window = gtk::ShortcutsWindow::builder()
        .transient_for(parent)
        .modal(true)
        .destroy_with_parent(true)
        .build();
    for section in sections {
        window.add_section(&built(section));
    }
    window.present();
}

/// One section of the window: its title in the switcher, and a group per
/// heading under it.
#[allow(deprecated)] // GtkShortcutsWindow: deprecated in 4.18, no replacement before GTK 5
fn built(section: &Section) -> gtk::ShortcutsSection {
    let built = gtk::ShortcutsSection::builder()
        .title(section.title)
        .section_name(name(section.title))
        .build();
    for group in &section.groups {
        built.add_group(&grouped(group));
    }
    built
}

/// One group of a section: its heading, and a row per Command.
#[allow(deprecated)] // GtkShortcutsWindow: deprecated in 4.18, no replacement before GTK 5
fn grouped(group: &Group) -> gtk::ShortcutsGroup {
    let built = gtk::ShortcutsGroup::builder()
        .title(group.title.unwrap_or_default())
        .build();
    for shortcut in &group.shortcuts {
        // A Command with no chord is a row with an empty accelerator, so that
        // the window is the whole table. GTK draws that as nothing, and warns
        // once that it measured the nothing at a negative width; the warning
        // is `GtkShortcutLabel`'s own — its box is empty and its spacing is
        // subtracted from it — and there is no property on
        // `GtkShortcutsShortcut` to put anything in its place.
        built.add_shortcut(
            &gtk::ShortcutsShortcut::builder()
                .title(shortcut.title)
                .accelerator(&shortcut.accelerator)
                .build(),
        );
    }
    built
}

/// The name GTK holds a section by, which has to tell one section from
/// another and is never read by a writer: the title, lowercased, with its
/// spaces closed up.
fn name(title: &str) -> String {
    title.to_lowercase().replace(' ', "-")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every section's GTK name is its own, which is the whole of what GTK
    /// asks of it.
    #[test]
    fn no_two_sections_are_held_by_the_same_name() {
        let defaults = quill_engine::settings::Settings::default().shortcuts();
        let sections = quill_engine::shortcuts::sections(&defaults.chords);
        let names: std::collections::BTreeSet<String> =
            sections.iter().map(|section| name(section.title)).collect();
        assert_eq!(names.len(), sections.len());
        assert!(names.contains("palette-and-keyboard-only"), "{names:?}");
    }
}
