//! The diff view: what Reload or Keep would do, shown before it is done.
//!
//! A file that changed under unsaved edits is a conflict, and the two ways out
//! of it each throw one of the two texts away: [`Choice::Reload`] takes the
//! disk's, [`Choice::Keep`] writes the window's over the file. Neither is
//! offered blind. [`open`] draws the engine's line diff
//! (`quill_engine::disk::diff`) of what the text will become — every line kept,
//! taken away or brought in — and one Accept button does it.
//!
//! Stock widgets in the shape the Settings and shortcuts windows are built in
//! ([`crate::settings::open`]): a `GtkWindow` transient over the window it
//! belongs to, built on every open and dropped when it closes, so a view is
//! never showing a diff of a file as it was a minute ago.
//!
//! What it draws is [`rows`], a function of the diff alone, which is what this
//! module can be tested through with no display attached.

use gtk::glib;
use gtk::prelude::*;
use quill_engine::disk::Line;
use quill_engine::theme::{Colour, Role};

use crate::ground::Ground;

/// The tag on a line the file will gain.
const ADDED: &str = "added";
/// The tag on a line the file will lose.
const REMOVED: &str = "removed";

/// What each kind of line is marked with, as a diff is marked anywhere.
const SAME_MARK: &str = "  ";
const REMOVED_MARK: &str = "- ";
const ADDED_MARK: &str = "+ ";

/// How strongly a changed line's tint is laid over the paper.
///
/// A wash rather than a colour: the line is text to be read, and the mark in
/// front of it already says which kind it is.
const TINT: f64 = 0.14;

/// The view's size when it opens, in logical pixels: wide enough for a line of
/// prose without wrapping, tall enough for a screenful of it.
const VIEW_WIDTH: i32 = 720;
const VIEW_HEIGHT: i32 = 520;

/// The air around the view's parts, and between them.
const MARGIN: i32 = 12;
const GAP: i32 = 8;

/// Which way out of a conflict the view is showing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Choice {
    /// The disk's text replaces what is in the window.
    Reload,
    /// What is in the window is written over the file.
    Keep,
}

impl Choice {
    /// What the view is called.
    fn title(self) -> &'static str {
        match self {
            Self::Reload => "Reload from disk",
            Self::Keep => "Keep this version",
        }
    }

    /// The sentence over the diff: what Accept will do.
    fn said(self) -> &'static str {
        match self {
            Self::Reload => "Accept replaces what is in this window with the file on disk.",
            Self::Keep => "Accept writes what is in this window over the file on disk.",
        }
    }
}

/// One line of the view: what it says and which tag tints it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shown {
    /// The line, behind the mark that says what the diff makes of it.
    pub text: String,
    /// The tag that tints it, or `None` for a line both texts have.
    pub tag: Option<&'static str>,
}

/// The lines the view draws for `lines`.
///
/// Marked and tinted both, so that the diff still reads as one for a writer
/// who cannot tell the two tints apart.
#[must_use]
pub fn rows(lines: &[Line]) -> Vec<Shown> {
    lines
        .iter()
        .map(|line| match line {
            Line::Same(text) => Shown {
                text: format!("{SAME_MARK}{text}"),
                tag: None,
            },
            Line::Removed(text) => Shown {
                text: format!("{REMOVED_MARK}{text}"),
                tag: Some(REMOVED),
            },
            Line::Added(text) => Shown {
                text: format!("{ADDED_MARK}{text}"),
                tag: Some(ADDED),
            },
        })
        .collect()
}

/// Opens the diff view for `choice` over `parent`, with `accept` behind its
/// Accept button.
///
/// `lines` is the diff of the text as it is against the text `choice` would
/// leave, so what the view shows is what the writer is agreeing to and nothing
/// else. Accept runs `accept` and the view closes; Cancel closes it and the
/// conflict stands, which is the answer that loses nothing.
pub fn open(
    parent: &gtk::Window,
    ground: Ground,
    choice: Choice,
    lines: &[Line],
    accept: impl Fn() + 'static,
) {
    let buffer = gtk::TextBuffer::new(None);
    tint(&buffer, ADDED, ground.colours.colour(Role::Accent));
    tint(&buffer, REMOVED, ground.colours.colour(Role::InkDim));
    fill(&buffer, lines);

    let said = gtk::Label::builder()
        .label(choice.said())
        .xalign(0.0)
        .wrap(true)
        .build();
    let view = gtk::TextView::builder()
        .buffer(&buffer)
        .editable(false)
        .cursor_visible(false)
        .monospace(true)
        .left_margin(MARGIN)
        .right_margin(MARGIN)
        .top_margin(MARGIN)
        .bottom_margin(MARGIN)
        .build();
    let scroller = gtk::ScrolledWindow::builder()
        .hexpand(true)
        .vexpand(true)
        .child(&view)
        .build();

    let cancel = gtk::Button::with_label("Cancel");
    let accepted = gtk::Button::with_label("Accept");
    accepted.add_css_class("suggested-action");
    let buttons = gtk::Box::new(gtk::Orientation::Horizontal, GAP);
    buttons.set_halign(gtk::Align::End);
    buttons.append(&cancel);
    buttons.append(&accepted);

    let column = gtk::Box::new(gtk::Orientation::Vertical, GAP);
    column.set_margin_top(MARGIN);
    column.set_margin_bottom(MARGIN);
    column.set_margin_start(MARGIN);
    column.set_margin_end(MARGIN);
    column.append(&said);
    column.append(&scroller);
    column.append(&buttons);

    let window = gtk::Window::builder()
        .title(choice.title())
        .transient_for(parent)
        .destroy_with_parent(true)
        .modal(true)
        .default_width(VIEW_WIDTH)
        .default_height(VIEW_HEIGHT)
        .child(&column)
        .build();
    cancel.connect_clicked(glib::clone!(
        #[weak]
        window,
        move |_| window.close()
    ));
    accepted.connect_clicked(glib::clone!(
        #[weak]
        window,
        move |_| {
            accept();
            window.close();
        }
    ));
    window.present();
}

/// Puts [`rows`] into `buffer`, each tinted by the tag it takes.
fn fill(buffer: &gtk::TextBuffer, lines: &[Line]) {
    for shown in rows(lines) {
        let from = buffer.end_iter().offset();
        let mut at = buffer.end_iter();
        // The newline is inside the tagged run, so a tinted line is tinted to
        // the end of the line rather than to the end of its last word.
        buffer.insert(&mut at, &format!("{}\n", shown.text));
        if let Some(tag) = shown.tag {
            buffer.apply_tag_by_name(tag, &buffer.iter_at_offset(from), &buffer.end_iter());
        }
    }
}

/// Adds the tag named `name` to `buffer`, washing the line behind it in
/// `colour`.
fn tint(buffer: &gtk::TextBuffer, name: &str, colour: Colour) {
    let wash = Colour {
        alpha: TINT,
        ..colour
    };
    let tag = gtk::TextTag::builder()
        .name(name)
        .background(wash.to_css())
        .build();
    buffer.tag_table().add(&tag);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rows_mark_what_each_line_is_and_leave_the_kept_ones_plain() {
        let lines = vec![
            Line::Same("first".to_string()),
            Line::Removed("second".to_string()),
            Line::Added("third".to_string()),
        ];
        assert_eq!(
            rows(&lines),
            vec![
                Shown {
                    text: "  first".to_string(),
                    tag: None,
                },
                Shown {
                    text: "- second".to_string(),
                    tag: Some(REMOVED),
                },
                Shown {
                    text: "+ third".to_string(),
                    tag: Some(ADDED),
                },
            ]
        );
    }

    #[test]
    fn two_texts_that_are_the_same_draw_no_marked_line() {
        let lines = quill_engine::disk::diff("one\ntwo\n", "one\ntwo\n");
        assert!(rows(&lines).iter().all(|shown| shown.tag.is_none()));
    }
}
