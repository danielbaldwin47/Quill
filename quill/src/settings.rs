//! The Settings window (`Ctrl+,`): writing modes and file-backed preferences.
//!
//! Plain GTK4 ([ADR 0009](../../docs/adr/0009-plain-gtk4-without-libadwaita.md)):
//! a window transient for the one it was opened from, holding a scrollable grid.
//! Its controls write the settings file. Alongside the Template, Syntax
//! highlight, Style check and Spell check toggles shared with the menus, and
//! Spell check's language, are the Typewriter anchor, Follow System, the
//! Library's own six (#246: the Locations, Pinned, and the four switches
//! nothing but this window and the file can reach), the `[export]` table's own
//! six (#290 built them: the page every export and every print is laid out on,
//! which the Export dialog offers a job's worth of and writes nothing back
//! to), and a button that hands `settings.toml` to the system editor.
//!
//! No row sets a value on the session. A row writes the file
//! ([`Session::edit_settings`]) and the settings watch reads it back and puts
//! it on to every window a moment later, which is the same path a writer's own
//! edit of the file takes — so there is one way a setting reaches the page and
//! not two. What the last read of the file refused is the label at the bottom,
//! the one place a writer is shown a refusal without a terminal.

use std::cell::RefCell;
use std::ops::RangeInclusive;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gio, glib};

use quill_engine::pos::Category;
use quill_engine::settings::{
    Paper, PreviewMode, Settings, Theme, export_margins, export_text_sizes,
};
use quill_engine::spell::Resolved;
use quill_engine::style::List;
use quill_engine::theme::Scheme;

use crate::choices;
use crate::export_dialog::{paper_at, paper_drop_down};
use crate::session::{Session, StyleToggle, SyntaxToggle, TemplateToggle};

/// The line under each Annotator's master switch. Both are English-only, for
/// different reasons — the tagger's training and the lists' own — and a writer
/// reading a French draft is owed the same sentence under either (#356).
const ENGLISH_ONLY: &str = "Supports English only for now.";

/// How near the top and the bottom of the window the Typewriter anchor may be
/// dragged. The setting itself takes any fraction (`docs/architecture.md`
/// § Settings); these are as far as a hand on the scale can put it, because a
/// caret resting in the last tenth of the window is not a rest line.
const ANCHOR_LOW: f64 = 0.2;
/// The other end of that scale.
const ANCHOR_HIGH: f64 = 0.8;
/// What one press of an arrow key on the scale moves the anchor by.
const ANCHOR_STEP: f64 = 0.01;
/// How many digits of the anchor the scale writes beside itself.
const ANCHOR_DIGITS: i32 = 2;

/// The Mode row's two rows: the mode, and what View › Panes calls it, so one
/// name for each of them.
///
/// The value beside the words as [`crate::export_dialog`]'s paper table has
/// them, so that the row a mode stands on is never a second thing to keep in
/// step ([`choices`]).
const PREVIEW_MODES: [(PreviewMode, &str); 2] =
    [(PreviewMode::Web, "Web"), (PreviewMode::Pdf, "PDF")];

/// The language dropdown's first row, which writes an empty `spell_language`:
/// the desktop locale's dictionary, resolved when a Document opens.
const SYSTEM_DEFAULT: &str = "System default";
/// The row the language dropdown stands on when the language it wants has no
/// dictionary: a state rather than a choice, so choosing it writes nothing.
const NO_DICTIONARY: &str = "No dictionary installed";

/// The Writing tools group's heading: the first row under the Export group.
const WRITING_TOOLS: i32 = 21;
/// Syntax highlight's master switch, the first row under that heading.
const SYNTAX_FIRST: i32 = WRITING_TOOLS + 1;
/// Syntax highlight's toggles: the master and its five Categories.
const SYNTAX_ROWS: usize = 6;
/// Style check's master switch, the first row under Syntax highlight's.
const STYLE_FIRST: i32 = below(SYNTAX_FIRST, SYNTAX_ROWS);
/// Style check's toggles: the master and its three Lists.
const STYLE_ROWS: usize = 4;
/// Spell check's switch, the first row under Style check's.
const SPELL_FIRST: i32 = below(STYLE_FIRST, STYLE_ROWS);

/// The window's margin and the space between its rows and its two columns.
const MARGIN: i32 = 18;
/// Between one row and the next.
const ROW_GAP: i32 = 12;
/// Between a row's label and its control.
const COLUMN_GAP: i32 = 24;

/// What the "Edit settings.toml…" button does with the file's URI: hands it to
/// the desktop's default handler, or — under a test — to a stub that takes
/// down what it was handed.
type Launch = dyn Fn(&str) -> Result<(), glib::Error>;

/// What a path list — Locations or Pinned — writes when a path goes in or
/// out of it: the whole list, never one path against what the file last said.
type WritePaths = fn(&mut Settings, Vec<PathBuf>);

/// Opens the Settings window over `parent`.
///
/// Built on every open and dropped when it closes, as the shortcuts window is,
/// so that every row opens showing the current effective setting. `spelling`
/// is what the parent window's Spell check language last resolved to, for the
/// "no dictionary" line.
pub fn open(parent: &gtk::Window, session: &Rc<Session>, spelling: Option<&Resolved>) {
    let grid = gtk::Grid::builder()
        .row_spacing(ROW_GAP)
        .column_spacing(COLUMN_GAP)
        .margin_top(MARGIN)
        .margin_bottom(MARGIN)
        .margin_start(MARGIN)
        .margin_end(MARGIN)
        .build();

    let anchor = gtk::Scale::with_range(
        gtk::Orientation::Horizontal,
        ANCHOR_LOW,
        ANCHOR_HIGH,
        ANCHOR_STEP,
    );
    anchor.set_digits(ANCHOR_DIGITS);
    // The number beside the slider, so that a writer reading the row and a
    // writer reading the file are reading the same value.
    anchor.set_draw_value(true);
    anchor.set_hexpand(true);
    // Set before the handler is connected, so that opening the window is not
    // itself a write.
    anchor.set_value(session.settings().typewriter_anchor);
    anchor.connect_value_changed(glib::clone!(
        #[strong]
        session,
        move |anchor| {
            let value = anchor.value();
            session.edit_settings(|settings| anchored(settings, value));
        }
    ));
    row(&grid, 0, "Typewriter anchor", &anchor);

    let follow = gtk::Switch::builder().halign(gtk::Align::End).build();
    follow.set_active(session.settings().theme == Theme::Auto);
    follow.connect_active_notify(glib::clone!(
        #[strong]
        session,
        move |follow| {
            let on = follow.is_active();
            let scheme = session.scheme();
            session.edit_settings(|settings| followed(settings, on, scheme));
        }
    ));
    row(&grid, 1, "Follow System", &follow);

    // Built before the rows so that the Add… dialog has a window to open
    // over; nothing is on screen until it is presented at the end.
    let window = gtk::Window::builder()
        .title("Settings")
        .transient_for(parent)
        .destroy_with_parent(true)
        .child(
            &gtk::ScrolledWindow::builder()
                .hscrollbar_policy(gtk::PolicyType::Never)
                .propagate_natural_height(true)
                .max_content_height(parent.height().max(240))
                .child(&grid)
                .build(),
        )
        .build();

    row(&grid, 2, "Locations", &location_list(&window, session));
    row(&grid, 3, "Pinned", &pinned_list(session));
    let library = session.settings().library.clone();
    row(
        &grid,
        4,
        "Show hidden folders",
        &switch(session, library.show_hidden, showed_hidden),
    );
    row(
        &grid,
        5,
        "Show file extensions",
        &switch(session, library.show_extensions, showed_extensions),
    );
    row(
        &grid,
        6,
        "Confirm before moving files",
        &switch(session, library.confirm_move, confirmed_move),
    );
    row(
        &grid,
        7,
        "Always ask where to save",
        &switch(session, library.ask_where_to_save, asked_where_to_save),
    );

    // The Preview pane's own group: the mode View › Panes' two rows write,
    // which is the pane's and not one window's
    // ([`crate::window::Window::set_preview_mode`]). Each group below the
    // Library's rows carries a heading.
    group_heading(&grid, 8, "Preview");
    row(
        &grid,
        9,
        "Mode",
        &preview_modes(session, session.preview_mode()),
    );

    // The Template's three toggles, the same keys View › Template's checks
    // flip: a row moved here is the file moving every open pane
    // ([`crate::window::reapply`]), and a check flipped there is the file
    // moving this row the next time the window is opened (#263).
    let template = session.template().clone();
    group_heading(&grid, 10, "Template");
    row(
        &grid,
        11,
        "Center headings",
        &switch(session, template.center_headings, centered_headings),
    );
    row(
        &grid,
        12,
        "Number headings",
        &switch(session, template.number_headings, numbered_headings),
    );
    row(
        &grid,
        13,
        "Indent paragraphs",
        &switch(session, template.indent_paragraphs, indented_paragraphs),
    );

    // The `[export]` table, which #282 built: the page every export and every
    // print is laid out on. The Export dialog offers one job's worth of the
    // same table, and the print dialog's Quill tab all of it but the paper,
    // and neither writes anything back — so this group and Save as defaults
    // are the two ways a default moves.
    let export = session.settings().export.clone();
    group_heading(&grid, 14, "Export");
    row(&grid, 15, "Paper", &export_papers(session, export.paper));
    row(
        &grid,
        16,
        "Margin (mm)",
        &export_spin(session, export.margin, &export_margins(), export_margin),
    );
    row(
        &grid,
        17,
        "Text size (pt)",
        &export_spin(
            session,
            export.text_size,
            &export_text_sizes(),
            export_text_size,
        ),
    );
    row(
        &grid,
        18,
        "Title page",
        &switch(session, export.title_page, export_title_page),
    );
    row(
        &grid,
        19,
        "Header",
        &switch(session, export.header, export_header),
    );
    row(
        &grid,
        20,
        "Footer",
        &switch(session, export.footer, export_footer),
    );

    group_heading(&grid, WRITING_TOOLS, "Writing tools");
    annotator_group(
        &grid,
        session,
        syntax_rows(session),
        SyntaxToggle::Enabled,
        SYNTAX_FIRST,
        ENGLISH_ONLY,
        |settings, toggle: SyntaxToggle, on| toggle.set(&mut settings.syntax_highlight, on),
    );
    // Style check's four under Syntax highlight's six, in the same group and
    // the same shape: the master a switch with its own line of text under it,
    // the three Lists checks. Its own line, because the two Annotators are
    // English-only for different reasons — the tagger's, and the lists' (#356).
    annotator_group(
        &grid,
        session,
        style_rows(session),
        StyleToggle::Enabled,
        STYLE_FIRST,
        ENGLISH_ONLY,
        |settings, toggle: StyleToggle, on| toggle.set(&mut settings.style_check, on),
    );
    // Spell check's under Style check's, in a shape of its own: a switch, the
    // language, and a line only when that language has no dictionary. The
    // dictionaries are listed as the window opens, so one installed since the
    // last open is offered on this one.
    let below_spell = spell_group(
        &grid,
        session,
        SPELL_FIRST,
        &quill_engine::spell::installed_languages(),
        spelling,
    );

    let button = gtk::Button::builder()
        .label("Edit settings.toml…")
        .halign(gtk::Align::End)
        .build();
    let launch = launcher();
    button.connect_clicked(glib::clone!(
        #[strong]
        session,
        move |_| edit(session.settings_path(), launch.as_ref())
    ));
    row(&grid, below_spell, "Keyboard shortcuts", &button);

    if let Some(said) = refused(&session.unapplied()) {
        let label = gtk::Label::builder()
            .label(said)
            .halign(gtk::Align::Start)
            .wrap(true)
            .build();
        grid.attach(&label, 0, below_spell + 1, 2, 1);
    }

    window.present();
}

/// A group's heading, across both columns: the label in bold with a row's air
/// above it, so the rows under it read as one group.
///
/// Bold by a Pango attribute rather than a CSS class: Quill's own stylesheet
/// dresses the chrome by the `chrome-*` classes its widgets carry
/// ([`crate::editor::install_type`] holds the provider), and this grid carries none
/// of them, so a class named here would style nothing and leave the heading
/// looking like a row.
fn group_heading(grid: &gtk::Grid, at: i32, said: &str) {
    let bold = gtk::pango::AttrList::new();
    bold.insert(gtk::pango::AttrInt::new_weight(gtk::pango::Weight::Bold));
    let label = gtk::Label::builder()
        .label(said)
        .halign(gtk::Align::Start)
        .margin_top(ROW_GAP)
        .attributes(&bold)
        .build();
    grid.attach(&label, 0, at, 2, 1);
}

/// One row of the grid: its label in the first column, its control in the
/// second.
fn row(grid: &gtk::Grid, at: i32, label: &str, control: &impl IsA<gtk::Widget>) {
    let label = gtk::Label::builder()
        .label(label)
        .halign(gtk::Align::Start)
        .build();
    grid.attach(&label, 0, at, 1, 1);
    grid.attach(control, 1, at, 1, 1);
}

/// What dragging the anchor scale writes.
fn anchored(settings: &mut Settings, anchor: f64) {
    settings.typewriter_anchor = anchor;
}

/// What Follow System writes: `auto` switched on, and the ground on screen
/// switched off.
///
/// Off writes a ground rather than nothing, because the switch is the writer
/// saying *stop following*, and the answer to "which ground, then" is the one
/// they are looking at ([`Scheme::setting`], which is the same conversion the
/// `Ctrl+Shift+L` toggle makes).
fn followed(settings: &mut Settings, on: bool, scheme: Scheme) {
    settings.theme = if on { Theme::Auto } else { scheme.setting() };
}

/// One Annotator's rows of the Writing tools group: the master's switch at
/// `first` with `hint` on the line under it, then one check per kind or List,
/// in the order the rows come.
///
/// Both Annotators draw the same four shapes and write their tables the same
/// way, so they draw through one helper and a third will too; what differs is
/// the table each `write` reaches and where the group starts (#356).
fn annotator_group<T: Copy + PartialEq + 'static>(
    grid: &gtk::Grid,
    session: &Rc<Session>,
    rows: impl IntoIterator<Item = (T, &'static str, bool)>,
    master: T,
    first: i32,
    hint: &str,
    write: impl Fn(&mut Settings, T, bool) + Copy + 'static,
) {
    for (index, (toggle, label, on)) in rows.into_iter().enumerate() {
        if toggle == master {
            row(
                grid,
                first,
                label,
                &switch(session, on, move |settings, on| {
                    write(settings, toggle, on);
                }),
            );
            let said = gtk::Label::builder()
                .label(hint)
                .halign(gtk::Align::Start)
                .build();
            grid.attach(&said, 0, first + 1, 2, 1);
        } else {
            let check = gtk::CheckButton::builder()
                .halign(gtk::Align::End)
                .active(on)
                .build();
            check.connect_toggled(glib::clone!(
                #[strong]
                session,
                move |check| {
                    let on = check.is_active();
                    session.edit_settings(|settings| write(settings, toggle, on));
                }
            ));
            row(
                grid,
                first + 1 + i32::try_from(index).unwrap(),
                label,
                &check,
            );
        }
    }
}

/// The first row under an [`annotator_group`] of `rows` toggles starting at
/// `first`: the master and the line under it take two rows, and each other
/// toggle one.
#[expect(
    clippy::cast_possible_wrap,
    clippy::cast_possible_truncation,
    reason = "a group of a handful of rows; `i32::try_from` is not const"
)]
const fn below(first: i32, rows: usize) -> i32 {
    first + rows as i32 + 1
}

/// Spell check's rows of the Writing tools group from `first`: its switch,
/// the language dropdown over the `installed` dictionaries, and the "no
/// dictionary" line when `spelling` found none. Returns the first row under
/// them, so the rows after the group stand on the next row and none is empty.
///
/// A helper of its own rather than [`annotator_group`]: Spell check has no
/// kinds to check, and has a dropdown and a line that comes and goes instead
/// (#401 § Settings window).
fn spell_group(
    grid: &gtk::Grid,
    session: &Rc<Session>,
    first: i32,
    installed: &[String],
    spelling: Option<&Resolved>,
) -> i32 {
    let checking = switch(session, session.spell(), spell_checked);
    row(grid, first, "Spell check", &checking);

    let current = Rc::new(RefCell::new(session.settings().spell_language.clone()));
    let (rows, selected) = language_rows(installed, &current.borrow(), spelling);
    let words: Vec<&str> = rows.iter().map(String::as_str).collect();
    let language = gtk::DropDown::from_strings(&words);
    language.set_halign(gtk::Align::End);
    // Stood on before the handler is connected, so opening the window is not
    // itself a write.
    language.set_selected(selected);
    row(grid, first + 1, "Language", &language);

    // Attached whatever the state, and shown only while it holds: a row with
    // nothing visible in it takes no space, so the rows below stand still.
    let said = gtk::Label::builder()
        .halign(gtk::Align::Start)
        .wrap(true)
        .build();
    show_unserved(
        &said,
        match spelling {
            Some(Resolved::Missing { wanted }) => Some(wanted.clone()),
            _ => None,
        },
    );
    grid.attach(&said, 0, first + 2, 2, 1);

    // What the window opened on is the Editor's resolution; a choice made here
    // is resolved here, by the same ladder over the same listing, because the
    // Editor hears of it only once the settings watch has read the file back
    // (#412's Hand test: the line outlived the dictionary that ended it).
    let installed = Rc::new(installed.to_vec());
    let rows = Rc::new(RefCell::new(rows));
    language.connect_selected_notify(glib::clone!(
        #[strong]
        session,
        #[strong]
        current,
        #[strong]
        installed,
        #[strong]
        rows,
        #[strong]
        checking,
        #[strong]
        said,
        move |language| {
            let Some(chosen) = language_at(&rows.borrow(), language.selected()) else {
                return;
            };
            current.replace(chosen.clone());
            session.edit_settings(|settings| chose_language(settings, chosen));
            // The "No dictionary installed" row was a state, and the state
            // has moved on: it goes, and it is after every other row, so the
            // row stood on keeps its place.
            if let Some(at) = served_rows(&mut rows.borrow_mut())
                && let Some(model) = language.model().and_downcast::<gtk::StringList>()
            {
                model.remove(at);
            }
            show_unserved(
                &said,
                unserved_now(&checking, &current.borrow(), &installed),
            );
        }
    ));
    checking.connect_active_notify(move |checking| {
        show_unserved(&said, unserved_now(checking, &current.borrow(), &installed));
    });
    first + 3
}

/// The wanted tag with no dictionary, for Spell check as `checking` and
/// `language` stand now, with the locale read from this process.
fn unserved_now(checking: &gtk::Switch, language: &str, installed: &[String]) -> Option<String> {
    unserved(checking.is_active(), language, installed, |name| {
        std::env::var(name).ok()
    })
}

/// The tag `language` wants when no installed dictionary serves it and Spell
/// check is `on`; `None` when a dictionary serves it or Spell check is off,
/// since the state is Spell check's and not the language's.
///
/// The ladder [`crate::editor::Editor`] resolves a Document's language by:
/// empty is the locale's tag, read through `locale`.
fn unserved(
    on: bool,
    language: &str,
    installed: &[String],
    locale: impl Fn(&str) -> Option<String>,
) -> Option<String> {
    if !on {
        return None;
    }
    let wanted = if language.is_empty() {
        quill_engine::spell::locale_tag(locale)
    } else {
        language.to_owned()
    };
    match quill_engine::spell::resolve(&wanted, installed) {
        Resolved::Missing { wanted } => Some(wanted),
        _ => None,
    }
}

/// Puts the "no dictionary" line for `wanted` under the dropdown, or takes it
/// away.
fn show_unserved(said: &gtk::Label, wanted: Option<String>) {
    said.set_visible(wanted.is_some());
    said.set_label(&wanted.as_deref().map(no_dictionary).unwrap_or_default());
}

/// Takes the [`NO_DICTIONARY`] row out of `rows`, and says where it stood.
fn served_rows(rows: &mut Vec<String>) -> Option<u32> {
    let at = rows.iter().position(|row| row == NO_DICTIONARY)?;
    rows.remove(at);
    u32::try_from(at).ok()
}

/// The language dropdown's rows and the one it stands on: System default,
/// then every installed tag in the listing's own order.
///
/// A language with no dictionary stands on a last row saying so; an explicit
/// language the listing lacks but another dictionary serves stands on a last
/// row naming it, so the dropdown never reads as a choice the writer did not
/// make.
fn language_rows(
    installed: &[String],
    language: &str,
    spelling: Option<&Resolved>,
) -> (Vec<String>, u32) {
    let mut rows: Vec<String> = std::iter::once(SYSTEM_DEFAULT.to_owned())
        .chain(installed.iter().cloned())
        .collect();
    let at = if matches!(spelling, Some(Resolved::Missing { .. })) {
        rows.push(NO_DICTIONARY.to_owned());
        rows.len() - 1
    } else if language.is_empty() {
        0
    } else if let Some(at) = installed.iter().position(|tag| tag == language) {
        at + 1
    } else {
        rows.push(language.to_owned());
        rows.len() - 1
    };
    (rows, u32::try_from(at).unwrap_or(0))
}

/// What choosing the dropdown's row `at` writes to `spell_language`: empty for
/// System default, the tag for a dictionary, and nothing for
/// [`NO_DICTIONARY`].
fn language_at(rows: &[String], at: u32) -> Option<String> {
    let row = rows.get(usize::try_from(at).ok()?)?;
    if at == 0 {
        Some(String::new())
    } else if row == NO_DICTIONARY {
        None
    } else {
        Some(row.clone())
    }
}

/// The line under the language dropdown when `wanted` has no dictionary, and
/// the notice the first Document to open in that state carries
/// ([`crate::window::Window`]): the tag, the Arch package that serves it, and
/// where every other system is told.
pub(crate) fn no_dictionary(wanted: &str) -> String {
    format!(
        "No dictionary installed for {wanted}: install hunspell-{}, or see the \
         README's Spell check in other languages.",
        wanted.to_lowercase()
    )
}

/// What the Spell check switch writes.
fn spell_checked(settings: &mut Settings, on: bool) {
    settings.spell_check = on;
}

/// What choosing a language writes.
fn chose_language(settings: &mut Settings, language: String) {
    settings.spell_language = language;
}

/// The Writing tools rows, projected from the live table without writing it.
fn syntax_rows(session: &Session) -> [(SyntaxToggle, &'static str, bool); SYNTAX_ROWS] {
    let syntax = session.syntax();
    [
        (SyntaxToggle::Enabled, "Syntax highlight"),
        (SyntaxToggle::Category(Category::Nouns), "Nouns"),
        (SyntaxToggle::Category(Category::Verbs), "Verbs"),
        (SyntaxToggle::Category(Category::Adjectives), "Adjectives"),
        (SyntaxToggle::Category(Category::Adverbs), "Adverbs"),
        (
            SyntaxToggle::Category(Category::Conjunctions),
            "Conjunctions",
        ),
    ]
    .map(|(toggle, label)| (toggle, label, toggle.of(&syntax)))
}

/// The Style check rows, projected from the live table without writing it.
fn style_rows(session: &Session) -> [(StyleToggle, &'static str, bool); STYLE_ROWS] {
    let style = session.style();
    [
        (StyleToggle::Enabled, "Style check"),
        (StyleToggle::List(List::Fillers), "Fillers"),
        (StyleToggle::List(List::Redundancies), "Redundancies"),
        (StyleToggle::List(List::Cliches), "Clichés"),
    ]
    .map(|(toggle, label)| (toggle, label, toggle.of(&style)))
}

/// A switch that writes one setting, set to the effective value
/// now before its handler is connected, so opening the window is not a write.
fn switch(
    session: &Rc<Session>,
    on: bool,
    write: impl Fn(&mut Settings, bool) + 'static,
) -> gtk::Switch {
    let switch = gtk::Switch::builder().halign(gtk::Align::End).build();
    switch.set_active(on);
    switch.connect_active_notify(glib::clone!(
        #[strong]
        session,
        move |switch| {
            let on = switch.is_active();
            session.edit_settings(|settings| write(settings, on));
        }
    ));
    switch
}

/// The Mode dropdown of the Preview group, standing on `mode` before its
/// handler is connected so that opening the window is not a write.
///
/// The mode is one setting for the app, so the row is the same key View ›
/// Panes' two rows write and the check there reads back what is picked here,
/// once the file is applied (#299).
fn preview_modes(session: &Rc<Session>, mode: PreviewMode) -> gtk::DropDown {
    let modes = choices::drop_down(&PREVIEW_MODES, mode);
    modes.set_halign(gtk::Align::End);
    modes.connect_selected_notify(glib::clone!(
        #[strong]
        session,
        move |modes| {
            let mode = choices::at(&PREVIEW_MODES, modes.selected());
            session.edit_settings(|settings| preview_mode(settings, mode));
        }
    ));
    modes
}

/// What the Mode row writes.
fn preview_mode(settings: &mut Settings, mode: PreviewMode) {
    settings.preview.mode = mode;
}

/// The paper dropdown of the Export group, over the rows the Export dialog
/// offers ([`paper_drop_down`]) and standing on `paper` before its handler is
/// connected.
///
/// The dialog's own list rather than a second one, so that a paper named here
/// and a paper named there cannot drift apart.
fn export_papers(session: &Rc<Session>, paper: Paper) -> gtk::DropDown {
    let papers = paper_drop_down(paper);
    papers.set_halign(gtk::Align::End);
    papers.connect_selected_notify(glib::clone!(
        #[strong]
        session,
        move |papers| {
            let paper = paper_at(papers.selected());
            session.edit_settings(|settings| export_paper(settings, paper));
        }
    ));
    papers
}

/// A spin button that writes one whole number of `[export]`, held to the range
/// the settings file holds that key to and set to what the file says now
/// before its handler is connected, so opening the window is not a write.
fn export_spin(
    session: &Rc<Session>,
    value: u32,
    range: &RangeInclusive<u32>,
    write: fn(&mut Settings, u32),
) -> gtk::SpinButton {
    let low = *range.start();
    let spin = gtk::SpinButton::with_range(f64::from(low), f64::from(*range.end()), 1.0);
    spin.set_halign(gtk::Align::End);
    spin.set_value(f64::from(value));
    spin.connect_value_changed(glib::clone!(
        #[strong]
        session,
        move |spin| {
            // The button was built over the range, so its value is inside it;
            // the floor is the answer for a value no `u32` can hold.
            let value = u32::try_from(spin.value_as_int()).unwrap_or(low);
            session.edit_settings(|settings| write(settings, value));
        }
    ));
    spin
}

/// What the Paper row writes.
fn export_paper(settings: &mut Settings, paper: Paper) {
    settings.export.paper = paper;
}

/// What the Margin row writes, in whole millimetres.
fn export_margin(settings: &mut Settings, millimetres: u32) {
    settings.export.margin = millimetres;
}

/// What the Text size row writes, in whole points.
fn export_text_size(settings: &mut Settings, points: u32) {
    settings.export.text_size = points;
}

/// What the Title page row writes.
fn export_title_page(settings: &mut Settings, on: bool) {
    settings.export.title_page = on;
}

/// What the Header row writes.
fn export_header(settings: &mut Settings, on: bool) {
    settings.export.header = on;
}

/// What the Footer row writes.
fn export_footer(settings: &mut Settings, on: bool) {
    settings.export.footer = on;
}

/// The lines of a path list — Locations or Pinned — one per path with a
/// button that drops it, beside the copy of the list this window holds.
///
/// The window writes the whole list each time rather than one path against
/// what the file last said, because the file is applied a moment later by the
/// watch ([`Session::apply`]): two removes in one breath would both read the
/// settings from before either, and the second would put the first back.
fn paths(
    session: &Rc<Session>,
    held: Vec<PathBuf>,
    write: WritePaths,
) -> (gtk::Box, Rc<RefCell<Vec<PathBuf>>>) {
    let list = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(ROW_GAP)
        .hexpand(true)
        .build();
    let held = Rc::new(RefCell::new(held));
    for path in held.borrow().clone() {
        let line = line(session, &list, &held, &path, write);
        list.append(&line);
    }
    (list, held)
}

/// One line of such a list: the path, and the button that drops it.
fn line(
    session: &Rc<Session>,
    list: &gtk::Box,
    held: &Rc<RefCell<Vec<PathBuf>>>,
    path: &Path,
    write: WritePaths,
) -> gtk::Box {
    let line = gtk::Box::new(gtk::Orientation::Horizontal, COLUMN_GAP);
    let label = gtk::Label::builder()
        .label(path.display().to_string())
        .halign(gtk::Align::Start)
        .hexpand(true)
        // A long path is cut at the front: the folder it ends in is the half
        // that says which one it is.
        .ellipsize(gtk::pango::EllipsizeMode::Start)
        .build();
    let remove = gtk::Button::builder().label("Remove").build();
    let gone = path.to_path_buf();
    remove.connect_clicked(glib::clone!(
        #[strong]
        session,
        #[strong]
        held,
        #[strong]
        list,
        #[strong]
        line,
        move |_| {
            held.borrow_mut().retain(|kept| *kept != gone);
            let paths = held.borrow().clone();
            session.edit_settings(|settings| write(settings, paths));
            list.remove(&line);
        }
    ));
    line.append(&label);
    line.append(&remove);
    line
}

/// The Locations row: the folders the Library shows, and the button that adds
/// another.
fn location_list(window: &gtk::Window, session: &Rc<Session>) -> gtk::Box {
    let locations = session.settings().library.locations.clone();
    let (list, held) = paths(session, locations, located);
    let add = gtk::Button::builder()
        .label("Add…")
        .halign(gtk::Align::End)
        .build();
    add.connect_clicked(glib::clone!(
        #[strong]
        session,
        #[strong]
        held,
        #[strong]
        list,
        #[weak]
        window,
        move |_| {
            let dialog = gtk::FileDialog::new();
            dialog.set_title("Add Location");
            dialog.select_folder(
                Some(&window),
                None::<&gio::Cancellable>,
                glib::clone!(
                    #[strong]
                    session,
                    #[strong]
                    held,
                    #[strong]
                    list,
                    move |answer| {
                        let Some(root) = answer.ok().and_then(|folder| folder.path()) else {
                            return;
                        };
                        if held.borrow().contains(&root) {
                            return;
                        }
                        held.borrow_mut().push(root.clone());
                        let paths = held.borrow().clone();
                        session.edit_settings(|settings| located(settings, paths));
                        let line = line(&session, &list, &held, &root, located);
                        list.append(&line);
                    }
                ),
            );
        }
    ));
    let column = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(ROW_GAP)
        .hexpand(true)
        .build();
    column.append(&list);
    column.append(&add);
    column
}

/// The Pinned row: what sits above the Locations in the sidebar, each with a
/// button that unpins it. Nothing adds one here — a pin is made in the
/// sidebar, on the Document or the folder being pinned.
fn pinned_list(session: &Rc<Session>) -> gtk::Box {
    let pinned = session.settings().library.pinned.clone();
    let (list, _) = paths(session, pinned, held_pinned);
    list
}

/// What the Locations list writes.
fn located(settings: &mut Settings, locations: Vec<PathBuf>) {
    settings.library.locations = locations;
}

/// What the Pinned list writes.
fn held_pinned(settings: &mut Settings, pinned: Vec<PathBuf>) {
    settings.library.pinned = pinned;
}

/// What Show hidden folders writes.
fn showed_hidden(settings: &mut Settings, on: bool) {
    settings.library.show_hidden = on;
}

/// What Show file extensions writes.
fn showed_extensions(settings: &mut Settings, on: bool) {
    settings.library.show_extensions = on;
}

/// What Confirm before moving files writes.
fn confirmed_move(settings: &mut Settings, on: bool) {
    settings.library.confirm_move = on;
}

/// What Always ask where to save writes.
fn asked_where_to_save(settings: &mut Settings, on: bool) {
    settings.library.ask_where_to_save = on;
}

/// What Center headings writes: the same key `template.centerHeadings` flips.
fn centered_headings(settings: &mut Settings, on: bool) {
    TemplateToggle::CenterHeadings.set(&mut settings.template, on);
}

/// What Number headings writes: the same key `template.numberHeadings` flips.
fn numbered_headings(settings: &mut Settings, on: bool) {
    TemplateToggle::NumberHeadings.set(&mut settings.template, on);
}

/// What Indent paragraphs writes: the same key `template.indentParagraphs`
/// flips.
fn indented_paragraphs(settings: &mut Settings, on: bool) {
    TemplateToggle::IndentParagraphs.set(&mut settings.template, on);
}

/// What the window says at the bottom about the last read of the settings
/// file, and `None` where all of it applied.
///
/// The lines [`Session::unapplied`] answers, one under the other: a file that
/// is not TOML says so and that the settings on screen are the last good ones,
/// and a refused `[shortcuts]` entry is the entry as the writer wrote it and
/// why none of it was applied — the same sentences the app warns under
/// `quill-settings`.
fn refused(unapplied: &[String]) -> Option<String> {
    (!unapplied.is_empty()).then(|| unapplied.join("\n"))
}

/// Hands the settings file to `launch`, as the URI a handler is asked for.
///
/// A file that has no URI and a desktop that has no handler for it are one
/// line on stderr each: the writer asked for an editor and there is none,
/// which is the desktop's to fix and not Quill's to fail on.
fn edit(path: &Path, launch: &Launch) {
    if let Err(err) = glib::filename_to_uri(path, None).and_then(|uri| launch(&uri)) {
        eprintln!("quill: {}: cannot be opened ({err})", path.display());
    }
}

/// The desktop's default handler for a URI: what the button does outside a
/// test.
fn launcher() -> Box<Launch> {
    Box::new(|uri| gio::AppInfo::launch_default_for_uri(uri, gio::AppLaunchContext::NONE))
}

#[cfg(test)]
mod tests {
    use quill_engine::settings::{Choice, Chrome, TemplateName};
    use quill_engine::shortcuts::Refusal;

    use super::*;
    use crate::flags::Flags;

    /// A launch reading and writing a settings file of this test's own.
    fn launched(name: &str) -> (Rc<Session>, std::path::PathBuf) {
        let path = std::env::temp_dir().join(format!("quill-{name}-{}.toml", std::process::id()));
        std::fs::remove_file(&path).ok();
        let session = Session::open(
            Flags {
                settings: Some(path.clone()),
                ..Flags::default()
            },
            None,
        );
        (session, path)
    }

    /// Dragging the scale writes the anchor to the file, and writes it beside
    /// what the writer had already changed with a key.
    #[test]
    fn the_anchor_row_writes_the_anchor_the_scale_was_dragged_to() {
        let (session, path) = launched("anchor");
        assert_eq!(session.toggle_chrome(), Chrome::Hidden);
        session.edit_settings(|settings| anchored(settings, ANCHOR_LOW));
        let (written, notes) = Settings::read_from(&path);
        assert_eq!(notes, Vec::<String>::new());
        assert!(
            (written.typewriter_anchor - ANCHOR_LOW).abs() < f64::EPSILON,
            "{}",
            written.typewriter_anchor
        );
        assert_eq!(written.chrome, Chrome::Hidden, "and what the key moved");
        std::fs::remove_file(&path).ok();
    }

    /// Follow System writes `auto` switched on, and the ground on screen
    /// switched off.
    #[test]
    fn follow_system_writes_auto_and_switching_it_off_pins_the_ground_on_screen() {
        let (session, path) = launched("follow");
        session.edit_settings(|settings| followed(settings, true, session.scheme()));
        let (written, notes) = Settings::read_from(&path);
        assert_eq!(notes, Vec::<String>::new());
        assert_eq!(written.theme, Theme::Auto);
        assert_eq!(
            written
                .to_toml()
                .lines()
                .find(|line| line.starts_with("theme")),
            Some("theme = \"auto\"")
        );
        session.edit_settings(|settings| followed(settings, false, Scheme::Dark));
        let (written, _) = Settings::read_from(&path);
        assert_eq!(written.theme, Theme::Dark);
        std::fs::remove_file(&path).ok();
    }

    /// The button hands the desktop the settings file, as a URI naming the
    /// file this launch is running on.
    #[test]
    fn the_button_hands_the_desktop_the_settings_file_as_a_uri() {
        let handed: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
        let taken = Rc::clone(&handed);
        let launch: Box<Launch> = Box::new(move |uri| {
            taken.borrow_mut().push(uri.to_owned());
            Ok(())
        });
        let path = std::env::temp_dir().join("quill settings.toml");
        edit(&path, launch.as_ref());
        assert_eq!(
            handed.borrow().as_slice(),
            [format!(
                "file://{}",
                path.display().to_string().replace(' ', "%20")
            )],
            "the path, escaped as a URI"
        );
    }

    /// One row's write, read back off the file and then put on to the session
    /// the way the watch would ([`Session::apply`]), so that the row after it
    /// writes from where this one left off rather than putting it back.
    fn wrote(session: &Rc<Session>, path: &Path, edit: impl FnOnce(&mut Settings)) -> Settings {
        session.edit_settings(edit);
        let (written, notes) = Settings::read_from(path);
        assert_eq!(notes, Vec::<String>::new());
        session.apply(written.clone());
        written
    }

    /// A folder of this test's own for a Location to point at, made empty.
    fn folder(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("quill-{name}-{}", std::process::id()));
        std::fs::remove_dir_all(&path).ok();
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    /// Every Library row writes its own key into `[library]` and leaves the
    /// rows written before it alone.
    #[test]
    fn every_library_row_writes_its_key_and_keeps_the_rows_before_it() {
        let (session, path) = launched("library-rows");
        let root = folder("library-rows-in");
        let pin = root.join("draft.md");
        let written = wrote(&session, &path, |settings| {
            located(settings, vec![root.clone()]);
        });
        assert_eq!(written.library.locations, std::slice::from_ref(&root));
        let written = wrote(&session, &path, |settings| {
            held_pinned(settings, vec![pin.clone()]);
        });
        assert_eq!(written.library.pinned, [pin]);
        for (flip, reads) in [
            (showed_hidden as fn(&mut Settings, bool), 0),
            (showed_extensions, 1),
            (confirmed_move, 2),
            (asked_where_to_save, 3),
        ] {
            let written = wrote(&session, &path, |settings| flip(settings, true));
            let switches = [
                written.library.show_hidden,
                written.library.show_extensions,
                written.library.confirm_move,
                written.library.ask_where_to_save,
            ];
            assert!(switches[reads], "row {reads} wrote its own key");
            assert_eq!(
                switches.iter().filter(|on| **on).count(),
                reads + 1,
                "and left the rows before it on"
            );
            assert_eq!(
                written.library.locations,
                std::slice::from_ref(&root),
                "and the Locations row alone"
            );
        }
        std::fs::remove_file(&path).ok();
        std::fs::remove_dir_all(&root).ok();
    }

    /// Every Template row writes its own key into `[template]`, leaves the
    /// other two and the Template itself alone, and the value it wrote is the
    /// one View › Template's check reads back once the file is applied — one
    /// setting under both (#271).
    #[test]
    fn every_template_row_writes_its_key_and_the_menus_check_reads_it_back() {
        let (session, path) = launched("template-rows");
        let rows = [
            (
                centered_headings as fn(&mut Settings, bool),
                TemplateToggle::CenterHeadings,
            ),
            (numbered_headings, TemplateToggle::NumberHeadings),
            (indented_paragraphs, TemplateToggle::IndentParagraphs),
        ];
        for (flip, moved) in rows {
            for on in [true, false] {
                let written = wrote(&session, &path, |settings| flip(settings, on));
                assert_eq!(moved.of(&written.template), on, "{moved:?} wrote its key");
                assert_eq!(
                    moved.of(&session.template()),
                    on,
                    "and the menu's check reads it back"
                );
                assert_eq!(
                    written.template.name,
                    TemplateName::Modern,
                    "and the Template itself is the radios'"
                );
                for (_, still) in rows.iter().filter(|(_, other)| *other != moved) {
                    assert_eq!(
                        still.of(&written.template),
                        still.of(&session.template()),
                        "{still:?} is where the row before it left it"
                    );
                }
            }
        }
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn syntax_rows_open_from_the_live_table_without_a_write() {
        let (session, path) = launched("syntax-rows-open");
        let (settings, notes) = Settings::parse(
            "[syntax_highlight]\nenabled = false\nnouns = false\nverbs = true\n\
             adjectives = false\nadverbs = true\nconjunctions = false\nfuture = 7\n",
        );
        assert!(notes.is_empty());
        session.apply(settings);
        session.toggle_syntax(SyntaxToggle::Enabled);
        session.store_settings();
        let before = std::fs::read(&path).unwrap();
        assert!(
            !session.settings().syntax_highlight.enabled,
            "the watch has not read the Command yet"
        );
        assert_eq!(
            syntax_rows(&session),
            [
                (SyntaxToggle::Enabled, "Syntax highlight", true),
                (SyntaxToggle::Category(Category::Nouns), "Nouns", false),
                (SyntaxToggle::Category(Category::Verbs), "Verbs", true),
                (
                    SyntaxToggle::Category(Category::Adjectives),
                    "Adjectives",
                    false
                ),
                (SyntaxToggle::Category(Category::Adverbs), "Adverbs", true),
                (
                    SyntaxToggle::Category(Category::Conjunctions),
                    "Conjunctions",
                    false
                ),
            ]
        );
        assert_eq!(std::fs::read(&path).unwrap(), before);
        std::fs::remove_file(&path).unwrap();
        syntax_rows(&session);
        assert!(!path.exists(), "opening rows does not create a file either");
    }

    #[test]
    fn every_syntax_row_saves_its_own_key_and_reflects_after_the_file_is_applied() {
        let (session, path) = launched("syntax-rows-save");
        let source = "face = \"mono\"\nfuture = 4\n[template]\nnumber_headings = true\n\
            [syntax_highlight]\nenabled = false\nnouns = false\nverbs = true\n\
            adjectives = false\nadverbs = true\nconjunctions = false\nfuture_syntax = 9\n";
        let (initial, notes) = Settings::parse(source);
        assert!(notes.is_empty());
        session.apply(initial.clone());
        for ((toggle, _, was), key) in syntax_rows(&session).into_iter().zip([
            "enabled",
            "nouns",
            "verbs",
            "adjectives",
            "adverbs",
            "conjunctions",
        ]) {
            for on in [!was, was] {
                let (expected, notes) = Settings::parse(
                    &source.replace(&format!("\n{key} = {was}"), &format!("\n{key} = {on}")),
                );
                assert!(notes.is_empty());
                let written = wrote(&session, &path, |settings| {
                    toggle.set(&mut settings.syntax_highlight, on);
                });
                assert_eq!(
                    written, expected,
                    "{key}: whole file, including unknown keys"
                );
                assert_eq!(session.running(), expected);
                assert_eq!(toggle.of(&session.syntax()), on);
            }
            assert_eq!(session.running(), initial);
        }
        std::fs::remove_file(path).unwrap();
    }

    /// The four Style check rows open from the live table, in the order and
    /// under the labels the group draws them, and the line under the master
    /// says what the lists cover (#356).
    #[test]
    fn style_rows_open_from_the_live_table_without_a_write() {
        let (session, path) = launched("style-rows-open");
        let (settings, notes) = Settings::parse(
            "[style_check]\nenabled = false\nfillers = false\nredundancies = true\n\
             cliches = false\nfuture = 7\n",
        );
        assert!(notes.is_empty());
        session.apply(settings);
        session.toggle_style(StyleToggle::Enabled);
        session.store_settings();
        let before = std::fs::read(&path).unwrap();
        assert!(
            !session.settings().style_check.enabled,
            "the watch has not read the Command yet"
        );
        assert_eq!(
            style_rows(&session),
            [
                (StyleToggle::Enabled, "Style check", true),
                (StyleToggle::List(List::Fillers), "Fillers", false),
                (StyleToggle::List(List::Redundancies), "Redundancies", true),
                (StyleToggle::List(List::Cliches), "Clichés", false),
            ]
        );
        assert_eq!(
            ENGLISH_ONLY, "Supports English only for now.",
            "the line under the Style check switch"
        );
        assert_eq!(std::fs::read(&path).unwrap(), before);
        std::fs::remove_file(&path).unwrap();
        style_rows(&session);
        assert!(!path.exists(), "opening rows does not create a file either");
    }

    #[test]
    fn every_style_row_saves_its_own_key_and_reflects_after_the_file_is_applied() {
        let (session, path) = launched("style-rows-save");
        let source = "face = \"mono\"\nfuture = 4\n[template]\nnumber_headings = true\n\
            [style_check]\nenabled = false\nfillers = false\nredundancies = true\n\
            cliches = false\nfuture_style = 9\n";
        let (initial, notes) = Settings::parse(source);
        assert!(notes.is_empty());
        session.apply(initial.clone());
        for ((toggle, _, was), key) in
            style_rows(&session)
                .into_iter()
                .zip(["enabled", "fillers", "redundancies", "cliches"])
        {
            for on in [!was, was] {
                let (expected, notes) = Settings::parse(
                    &source.replace(&format!("\n{key} = {was}"), &format!("\n{key} = {on}")),
                );
                assert!(notes.is_empty());
                let written = wrote(&session, &path, |settings| {
                    toggle.set(&mut settings.style_check, on);
                });
                assert_eq!(
                    written, expected,
                    "{key}: whole file, including unknown keys"
                );
                assert_eq!(session.running(), expected);
                assert_eq!(toggle.of(&session.style()), on);
            }
            assert_eq!(session.running(), initial);
        }
        std::fs::remove_file(path).unwrap();
    }

    /// The top group lost the language row, so every group under it moved up
    /// by one and none left a gap; Spell check's rows stand under Style
    /// check's last and the button under Spell check's last, whether or not
    /// the "no dictionary" line is there.
    #[test]
    fn spell_check_s_rows_follow_style_check_s_with_no_empty_row() {
        let (session, path) = launched("spell-rows-layout");
        assert_eq!(WRITING_TOOLS, 21, "the Export group's Footer is row 20");
        assert_eq!(SYNTAX_FIRST, WRITING_TOOLS + 1);
        assert_eq!(
            syntax_rows(&session).len(),
            SYNTAX_ROWS,
            "Syntax highlight: switch, line, five checks"
        );
        assert_eq!(STYLE_FIRST, SYNTAX_FIRST + 7);
        assert_eq!(style_rows(&session).len(), STYLE_ROWS);
        assert_eq!(
            SPELL_FIRST,
            STYLE_FIRST + 5,
            "under Style check's switch, line and three checks"
        );
        std::fs::remove_file(path).ok();
    }

    /// System default, then the listing in its own order; the row the
    /// setting names is the one stood on, and a language with no dictionary
    /// stands on a row saying so that writes nothing.
    #[test]
    fn the_language_dropdown_lists_system_default_then_the_listing_in_order() {
        let installed = ["en_US", "de_DE", "en_GB"].map(str::to_owned);
        let (rows, at) = language_rows(&installed, "", None);
        assert_eq!(rows, ["System default", "en_US", "de_DE", "en_GB"]);
        assert_eq!(at, 0);
        assert_eq!(language_rows(&installed, "de_DE", None).1, 2);

        let fallback = Resolved::Fallback {
            tag: "de_DE".into(),
            wanted: "de_AT".into(),
        };
        let (rows, at) = language_rows(&installed, "de_AT", Some(&fallback));
        assert_eq!(rows.last().map(String::as_str), Some("de_AT"));
        assert_eq!(language_at(&rows, at).as_deref(), Some("de_AT"));

        let missing = Resolved::Missing {
            wanted: "xx_XX".into(),
        };
        let (rows, at) = language_rows(&installed, "", Some(&missing));
        assert_eq!(
            rows,
            [
                "System default",
                "en_US",
                "de_DE",
                "en_GB",
                "No dictionary installed"
            ]
        );
        assert_eq!(at, 4);
        assert_eq!(language_at(&rows, at), None, "a state, not a choice");
        assert_eq!(language_at(&rows, 0).as_deref(), Some(""));
        assert_eq!(language_at(&rows, 3).as_deref(), Some("en_GB"));
        assert_eq!(language_at(&rows, 9), None);
    }

    /// The line follows the resolution as the writer changes it: a language
    /// with no dictionary shows it, a served one takes it away, System default
    /// is the locale's, and Spell check off shows none. And the stale row goes
    /// with it, leaving the rows before it where they stood.
    #[test]
    fn the_no_dictionary_line_follows_a_changed_resolution() {
        let installed = ["en_US", "de_DE"].map(str::to_owned);
        let english = |_: &str| Some("en_US.UTF-8".to_owned());
        let unknown = |_: &str| Some("xx_XX.UTF-8".to_owned());
        assert_eq!(
            unserved(true, "xx_XX", &installed, english).as_deref(),
            Some("xx_XX")
        );
        assert_eq!(unserved(true, "en_US", &installed, english), None);
        assert_eq!(
            unserved(true, "de_AT", &installed, english),
            None,
            "falls back"
        );
        assert_eq!(unserved(true, "", &installed, english), None);
        assert_eq!(
            unserved(true, "", &installed, unknown).as_deref(),
            Some("xx_XX")
        );
        assert_eq!(unserved(false, "xx_XX", &installed, english), None);

        let missing = Resolved::Missing {
            wanted: "xx_XX".into(),
        };
        let (mut rows, _) = language_rows(&installed, "xx_XX", Some(&missing));
        assert_eq!(served_rows(&mut rows), Some(3));
        assert_eq!(rows, ["System default", "en_US", "de_DE"]);
        assert_eq!(served_rows(&mut rows), None, "gone once");
    }

    #[test]
    fn the_no_dictionary_line_names_the_tag_and_its_arch_package() {
        let said = no_dictionary("xx_XX");
        assert!(said.contains("xx_XX"), "{said}");
        assert!(said.contains("hunspell-xx_xx"), "{said}");
        assert!(said.contains("Spell check in other languages"), "{said}");
    }

    /// The switch writes `spell_check` and the dropdown `spell_language`, each
    /// alone and beside what the file already said, and the session reads
    /// both back once the file is applied.
    #[test]
    fn the_spell_rows_write_their_own_keys_and_reflect_after_the_file_is_applied() {
        let (session, path) = launched("spell-rows-save");
        let source = "face = \"mono\"\nfuture = 4\nspell_check = true\nspell_language = \"\"\n";
        let (initial, notes) = Settings::parse(source);
        assert!(notes.is_empty());
        session.apply(initial);
        let installed = ["en_US", "de_DE"].map(str::to_owned);
        let (rows, _) = language_rows(&installed, "", None);
        for (at, language) in [(2, "de_DE"), (0, "")] {
            let chosen = language_at(&rows, at).expect("a dictionary row");
            let written = wrote(&session, &path, |settings| chose_language(settings, chosen));
            let (expected, _) = Settings::parse(&source.replace(
                "spell_language = \"\"",
                &format!("spell_language = \"{language}\""),
            ));
            assert_eq!(written, expected, "whole file, including unknown keys");
            assert_eq!(session.settings().spell_language, language);
        }
        for on in [false, true] {
            let written = wrote(&session, &path, |settings| spell_checked(settings, on));
            assert_eq!(written.spell_check, on);
            assert_eq!(session.spell(), on);
        }
        std::fs::remove_file(path).unwrap();
    }

    /// The Mode row writes `[preview] mode` and nothing else, the dropdown's
    /// rows and the setting's values line up, and the value it wrote is the
    /// one View › Panes' check reads back once the file is applied — one
    /// setting under both (#299).
    #[test]
    fn the_preview_mode_row_writes_its_key_and_the_menus_check_reads_it_back() {
        let (session, path) = launched("preview-mode-row");
        assert_eq!(
            PREVIEW_MODES.map(|(mode, _)| mode.as_str()).as_slice(),
            PreviewMode::VALUES,
            "a labelled row for every mode the setting takes, in its own order"
        );
        let before = session.running().preview;
        for mode in [PreviewMode::Pdf, PreviewMode::Web] {
            let written = wrote(&session, &path, |settings| preview_mode(settings, mode));
            assert_eq!(written.preview.mode, mode, "the row wrote its key");
            assert_eq!(
                (written.preview.layout, written.preview.zoom),
                (before.layout, before.zoom),
                "and left the rest of [preview] alone"
            );
            assert_eq!(
                session.preview_mode(),
                mode,
                "and the menu's check reads it back"
            );
            let text = std::fs::read_to_string(&path).unwrap();
            let said = format!("mode = \"{}\"", mode.as_str());
            assert!(text.contains(&said), "the file says {said}:\n{text}");
        }
        std::fs::remove_file(&path).ok();
    }

    /// Every Export row writes its own key into `[export]`, leaves the rows
    /// written before it where they are, and the file itself says what the
    /// row wrote — the one path a setting takes (#290).
    #[test]
    fn every_export_row_writes_its_key_and_the_file_says_so() {
        let (session, path) = launched("export-rows");
        let written = wrote(&session, &path, |settings| {
            export_paper(settings, Paper::Legal);
        });
        assert_eq!(written.export.paper, Paper::Legal);
        let written = wrote(&session, &path, |settings| export_margin(settings, 25));
        assert_eq!(written.export.margin, 25);
        assert_eq!(written.export.paper, Paper::Legal, "the row before it");
        let written = wrote(&session, &path, |settings| export_text_size(settings, 14));
        assert_eq!(written.export.text_size, 14);
        for (flip, reads) in [
            (export_title_page as fn(&mut Settings, bool), 0),
            (export_header, 1),
            (export_footer, 2),
        ] {
            let written = wrote(&session, &path, |settings| flip(settings, true));
            let switches = [
                written.export.title_page,
                written.export.header,
                written.export.footer,
            ];
            assert!(switches[reads], "row {reads} wrote its own key");
            assert_eq!(
                switches.iter().filter(|on| **on).count(),
                reads + 1,
                "and left the rows before it on"
            );
            assert_eq!(
                (
                    written.export.paper,
                    written.export.margin,
                    written.export.text_size
                ),
                (Paper::Legal, 25, 14),
                "and the three rows above it alone"
            );
        }
        let text = std::fs::read_to_string(&path).unwrap();
        for said in [
            "paper = \"legal\"",
            "margin = 25",
            "text_size = 14",
            "title_page = true",
            "header = true",
            "footer = true",
        ] {
            assert!(text.contains(said), "the file says {said}:\n{text}");
        }
        std::fs::remove_file(&path).ok();
    }

    /// A Location written by the row reaches the Library as the watch reads
    /// the file back, which is what makes the row apply without a relaunch
    /// (#251 found the edit going nowhere).
    #[test]
    fn a_location_the_row_wrote_reaches_the_library_when_the_file_is_read_back() {
        let (session, path) = launched("library-live");
        let root = folder("library-live-in");
        assert!(session.library().locations().is_empty());
        wrote(&session, &path, |settings| {
            located(settings, vec![root.clone()]);
        });
        assert_eq!(
            session
                .library()
                .locations()
                .iter()
                .map(|location| location.path().to_path_buf())
                .collect::<Vec<_>>(),
            std::slice::from_ref(&root),
            "the folder the row added is a Location now"
        );
        wrote(&session, &path, |settings| located(settings, Vec::new()));
        assert!(
            session.library().locations().is_empty(),
            "and removing it drops it"
        );
        std::fs::remove_file(&path).ok();
        std::fs::remove_dir_all(&root).ok();
    }

    /// A refused entry is shown with its reason, and nothing is shown where
    /// the file refused nothing.
    #[test]
    fn a_refused_line_is_shown_with_its_reason_and_a_clean_file_shows_none() {
        assert_eq!(refused(&[]), None);
        let refusal = Refusal {
            line: "\"library.toggle\" = [\"<Super>l\"]".to_owned(),
            id: "library.toggle".to_owned(),
            reason: "<Super>l belongs to the compositor".to_owned(),
        };
        // The file's own line first, then the entry's, in the words the app
        // warns with.
        let unapplied = [
            "is not TOML (line 28: duplicate key at \"library.toggle\"); keeping the settings \
             Quill is running on"
                .to_owned(),
            refusal.to_string(),
        ];
        assert_eq!(
            refused(&unapplied),
            Some(
                "is not TOML (line 28: duplicate key at \"library.toggle\"); keeping the \
                 settings Quill is running on\n\
                 \"library.toggle\" = [\"<Super>l\"]: <Super>l belongs to the compositor"
                    .to_owned()
            )
        );
    }
}
