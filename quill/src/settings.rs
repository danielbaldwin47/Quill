//! The Settings window (`Ctrl+,`): a sidebar of five panes, every row built from the Palette's table of Settings rows.
//!
//! Plain GTK4 ([ADR 0009](../../docs/adr/0009-plain-gtk4-without-libadwaita.md)):
//! a window transient for the one it was opened from, 700 × 520, a 168 px
//! sidebar — the search field over a `ListBox` of the five panes — beside a
//! stack of them (#467, the layout `prototype/settings-stub` measured). Each
//! pane is [`quill_engine::palette::SETTINGS_ROWS`] under that pane, in the
//! table's order, and each row's control comes from [`control`], the one
//! builder the Palette's settings rows read too, so the two surfaces write
//! through the same functions. What the menus hold — Preview Mode, the
//! Annotators and their Categories and Lists, Spell check — is not here (#467
//! § One pane per setting).
//!
//! No row sets a value on the session. A row writes the file
//! ([`Session::edit_settings`]) and the settings watch reads it back and puts
//! it on to every window a moment later, which is the same path a writer's own
//! edit of the file takes — so there is one way a setting reaches the page and
//! not two. What the last read of the file refused is on the Advanced pane,
//! the one place a writer is shown a refusal without a terminal.
//!
//! The window follows the file back: the window that opened it holds it
//! ([`Open`]), and every pass that puts the settings on to the windows stands
//! its rows on them again ([`Open::refresh`]), so a key pressed elsewhere or a
//! hand's edit of the file is what the rows show.

use std::cell::{Cell, RefCell};
use std::ops::RangeInclusive;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gio, glib};

use quill_engine::palette::{Control, Pane, SETTINGS_ROWS, Setting};
use quill_engine::settings::{
    Choice, Chrome, Paper, Settings, TemplateName, Theme, export_margins, export_text_sizes,
};
use quill_engine::spell::Resolved;
use quill_engine::theme::Scheme;

use crate::export_dialog::{paper_at, paper_drop_down};
use crate::session::{Session, TemplateToggle};

mod search;
mod sheet;

pub(crate) use sheet::{controls, stylesheet};

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

/// The language dropdown's first row, which writes an empty `spell_language`:
/// the desktop locale's dictionary, resolved when a Document opens.
const SYSTEM_DEFAULT: &str = "System default";
/// The row the language dropdown stands on when the language it wants has no
/// dictionary: a state rather than a choice, so choosing it writes nothing.
const NO_DICTIONARY: &str = "No dictionary installed";

/// What the "Edit settings.toml…" button does with the file's URI: hands it to
/// the desktop's default handler, or — under a test — to a stub that takes
/// down what it was handed.
type Launch = dyn Fn(&str) -> Result<(), glib::Error>;

/// What a path list — Locations or Pinned — writes when a path goes in or
/// out of it: the whole list, never one path against what the file last said.
type WritePaths = fn(&mut Settings, Vec<PathBuf>);

/// The window's size, the canvas boards' and the stub's.
const WIDTH: i32 = 700;
/// The other side of it.
const HEIGHT: i32 = 520;
/// The sidebar's width, which the field holds by [`FIELD_CHARS`].
const SIDE: i32 = 168;
/// The search field's width in characters: what keeps the sidebar at
/// [`SIDE`], since an entry asks for more than that by default.
const FIELD_CHARS: i32 = 8;
/// The magnifier's side, the Palette's own.
const MAG: i32 = 13;
/// How far in from the sidebar's edge the magnifier stands: the field's
/// margin and its padding, so it sits where a search entry's own icon would.
const MAG_INSET: i32 = 14;
/// The air the sheet puts under the field, which the magnifier is centred
/// above.
const FIELD_BELOW: i32 = 8;
/// The Typewriter anchor's width: a scale has no natural width, so it is a
/// size request.
const ANCHOR_WIDTH: i32 = 220;
/// A spin button's width in characters, which the 26 px cells either side
/// of it are sized against.
const SPIN_CHARS: i32 = 3;
/// Between a row's words and its control.
const ROW_GAP: i32 = 16;
/// Between a path and its Remove button.
const PATH_GAP: i32 = 12;
/// Between a path list and the Add… under it.
const LIST_GAP: i32 = 8;
/// What the row holding Edit settings.toml… says, the button being the
/// row's control: what the file holds that no row does.
const FILE_ROW: &str = "Shortcuts and palette";
/// The line under it.
const FILE_HINT: &str = "Shortcut rebinds and the palette file are set in the file";

/// A switch row's read and write: what it stands on as the window opens, and
/// what flipping it writes.
type Toggle = (fn(&Settings) -> bool, fn(&mut Settings, bool));

thread_local! {
    /// The pane last shown in this process, which the next open shows: held
    /// here and written nowhere, so a relaunch opens on General (#467).
    static LAST: Cell<Pane> = const { Cell::new(Pane::General) };

    /// Whether a refresh is standing the controls on the file's values, which
    /// is the file moving a control rather than a writer moving it: no row
    /// writes while it holds ([`put`]).
    static QUIET: Cell<bool> = const { Cell::new(false) };
}

/// Puts one row's edit into the file ([`Session::edit_settings`]), unless the
/// control moved because a refresh stood it on what the file already holds.
fn put(session: &Session, edit: impl FnOnce(&mut Settings)) {
    if !QUIET.with(Cell::get) {
        session.edit_settings(edit);
    }
}

/// The pane the next [`open`] shows: the one last shown in this process, and
/// General on the first open of a run.
pub(crate) fn last_pane() -> Pane {
    LAST.with(Cell::get)
}

/// Takes down that `pane` is the one shown.
fn showed(pane: Pane) {
    LAST.with(|last| last.set(pane));
}

/// One row on a pane with a control of its own: the table's setting and the
/// control that stands on it.
struct Row {
    setting: &'static Setting,
    control: gtk::Widget,
}

/// An open Settings window and its rows, held by the window that opened it as
/// that window holds its Export dialog: weakly, so closing it is the end of
/// it.
pub(crate) struct Open {
    window: glib::WeakRef<gtk::Window>,
    nav: gtk::ListBox,
    rows: Vec<Row>,
    /// The Spell check language's dropdown and the "no dictionary" line
    /// under it, whose rows hang on what the Editor resolved.
    language: Option<(gtk::DropDown, gtk::Label)>,
    /// The refused lines under their head, drawn whether or not a line is
    /// refused and shown only while one is.
    refused: Option<gtk::Box>,
}

impl Open {
    /// The window, while it is open.
    pub(crate) fn window(&self) -> Option<gtk::Window> {
        self.window.upgrade()
    }

    /// Shows `pane`, as picking it in the sidebar does: a Palette jump row or
    /// `--pane` reaching a window already open.
    pub(crate) fn show(&self, pane: Pane) {
        select(&self.nav, pane);
    }

    /// Stands every row whose control no longer shows what `settings` holds
    /// on the value it holds, writing nothing ([`QUIET`]): `Ctrl+Shift+H`, a
    /// Template picked from the Palette or a hand's edit of the file reaching
    /// the window. A row that already shows it — the echo of the window's own
    /// write, a drag on the anchor among them — is not touched, so the pane,
    /// its scroll and the focus stay where they are.
    ///
    /// The Spell check language is stood on the file's language and on
    /// `spelling`, what the parent's Editor has resolved it to by now, and
    /// the refused lines on the last read of the file; the search lists the
    /// refused lines only while they are shown.
    pub(crate) fn refresh(&self, session: &Session, spelling: Option<&Resolved>) {
        if self.window().is_none() {
            return;
        }
        let settings = session.running();
        let scheme = session.scheme();
        QUIET.with(|quiet| quiet.set(true));
        if let Some((language, said)) = &self.language {
            stand_language(language, said, &settings.spell_language, spelling);
        }
        if let Some(refused) = &self.refused {
            fill_refused(refused, session);
        }
        for row in self.rows.iter() {
            let Some(now) =
                shown(&row.control).and_then(|shown| moved(&settings, scheme, row.setting, shown))
            else {
                continue;
            };
            stand_on(&row.control, now);
        }
        QUIET.with(|quiet| quiet.set(false));
    }
}

/// What a row's control stands on, in the terms the refresh compares a pane
/// with the settings in.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Standing {
    /// A switch, or a Template's radio.
    On(bool),
    /// The Paper dropdown.
    Paper(Paper),
    /// A spin button.
    Whole(u32),
    /// The Typewriter anchor's scale.
    Fraction(f64),
}

/// What `setting`'s control stands on under `settings` with `scheme` on
/// screen, or `None` for a row the refresh leaves as it opened: the Spell
/// check language, whose rows hang on what the Editor resolved, and a row
/// with no control. Dark Mode reads the ground on screen, which under
/// Follow System is the desktop's and in no key.
fn standing(settings: &Settings, scheme: Scheme, setting: &Setting) -> Option<Standing> {
    let standing = match (setting.key?, setting.control) {
        ("chrome", _) => Standing::On(settings.chrome == Chrome::Hidden),
        ("template.name", Control::Radio { value }) => {
            Standing::On(settings.template.name.as_str() == value)
        }
        ("theme", _) if setting.command == Some("theme.toggle") => {
            Standing::On(scheme == Scheme::Dark)
        }
        ("theme", _) => Standing::On(settings.theme == Theme::Auto),
        ("typewriter_anchor", _) => Standing::Fraction(settings.typewriter_anchor),
        ("export.paper", _) => Standing::Paper(settings.export.paper),
        ("export.margin", _) => Standing::Whole(settings.export.margin),
        ("export.text_size", _) => Standing::Whole(settings.export.text_size),
        (key, Control::Switch) => Standing::On(toggle(key)?.0(settings)),
        _ => return None,
    };
    Some(standing)
}

/// What `setting`'s control, showing `shown`, is to be stood on now that the
/// launch runs `settings`, or `None` where it shows that already. The anchor
/// is the same within half a step, since the scale rounds what it is given.
fn moved(
    settings: &Settings,
    scheme: Scheme,
    setting: &Setting,
    shown: Standing,
) -> Option<Standing> {
    let now = standing(settings, scheme, setting)?;
    let same = match (now, shown) {
        (Standing::Fraction(now), Standing::Fraction(shown)) => {
            (now - shown).abs() < ANCHOR_STEP / 2.0
        }
        _ => now == shown,
    };
    (!same).then_some(now)
}

/// What `control` shows, read off the widget.
fn shown(control: &gtk::Widget) -> Option<Standing> {
    if let Some(switch) = control.downcast_ref::<gtk::Switch>() {
        Some(Standing::On(switch.is_active()))
    } else if let Some(radio) = control.downcast_ref::<gtk::CheckButton>() {
        Some(Standing::On(radio.is_active()))
    } else if let Some(spin) = control.downcast_ref::<gtk::SpinButton>() {
        u32::try_from(spin.value_as_int()).ok().map(Standing::Whole)
    } else if let Some(scale) = control.downcast_ref::<gtk::Scale>() {
        Some(Standing::Fraction(scale.value()))
    } else {
        let papers = control.downcast_ref::<gtk::DropDown>()?;
        Some(Standing::Paper(paper_at(papers.selected())))
    }
}

/// Stands `control` on `now`. A radio is only ever switched on: its group
/// switches the one it leaves off.
fn stand_on(control: &gtk::Widget, now: Standing) {
    match now {
        Standing::On(on) => {
            if let Some(switch) = control.downcast_ref::<gtk::Switch>() {
                switch.set_active(on);
            } else if let Some(radio) = control.downcast_ref::<gtk::CheckButton>()
                && on
            {
                radio.set_active(true);
            }
        }
        Standing::Whole(value) => {
            if let Some(spin) = control.downcast_ref::<gtk::SpinButton>() {
                spin.set_value(f64::from(value));
            }
        }
        Standing::Fraction(value) => {
            if let Some(scale) = control.downcast_ref::<gtk::Scale>() {
                scale.set_value(value);
            }
        }
        Standing::Paper(paper) => {
            if let Some(papers) = control.downcast_ref::<gtk::DropDown>() {
                let count = papers.model().map_or(0, |model| model.n_items());
                if let Some(at) = (0..count).find(|at| paper_at(*at) == paper) {
                    papers.set_selected(at);
                }
            }
        }
    }
}

/// The table's rows under `pane`, in the table's order: the pane's own rows,
/// top to bottom.
fn rows_of(pane: Pane) -> impl Iterator<Item = &'static Setting> {
    SETTINGS_ROWS
        .iter()
        .filter(move |setting| setting.pane == pane)
}

/// The small-caps head a pane draws above `setting`, where one starts a group
/// there: the Library's three groups, the Template's two, and the refused
/// lines.
fn head(setting: &Setting) -> Option<&'static str> {
    match setting.label {
        "Show hidden folders" => Some("Files"),
        "Modern" => Some("Template"),
        "Center headings" => Some("Layout"),
        "Locations" | "Pinned" | "Not applied from settings.toml" => Some(setting.label),
        _ => None,
    }
}

/// The line under a row's words, where it has one.
fn hint(setting: &Setting) -> Option<&'static str> {
    match setting.control {
        Control::Button => Some(FILE_HINT),
        _ if setting.command == Some("theme.auto") => Some("Light and dark follow the desktop"),
        _ => None,
    }
}

/// Opens the Settings window over `parent` on the pane last shown.
///
/// Built on every open and dropped when it closes, as the shortcuts window is,
/// so that every row opens showing the current effective setting, and stood
/// on the settings again by [`Open::refresh`] while it is open. `spelling`
/// is what the parent window's Spell check language last resolved to, for the
/// "no dictionary" line.
pub fn open(parent: &gtk::Window, session: &Rc<Session>, spelling: Option<&Resolved>) -> Open {
    open_on(parent, session, spelling, last_pane())
}

/// Opens the Settings window over `parent` on `pane`, as [`open`] does.
pub fn open_on(
    parent: &gtk::Window,
    session: &Rc<Session>,
    spelling: Option<&Resolved>,
    pane: Pane,
) -> Open {
    // Built before the rows so that the Add… dialog has a window to open
    // over; nothing is on screen until it is presented at the end.
    let window = gtk::Window::builder()
        .title("Settings")
        .transient_for(parent)
        .destroy_with_parent(true)
        .default_width(WIDTH)
        .default_height(HEIGHT)
        .css_classes(["settings"])
        .build();

    let panes = gtk::Stack::builder().hexpand(true).vexpand(true).build();
    let mut rows = Vec::new();
    let mut places = Vec::new();
    let mut language = None;
    let mut refused = None;
    for each in Pane::ALL {
        let body = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .css_classes(["settings-pane"])
            .build();
        let mut first = true;
        let mut leader: Option<gtk::CheckButton> = None;
        let mut drawn = Vec::new();
        for setting in rows_of(each) {
            let Some((block, control)) = block(&window, session, spelling, setting) else {
                continue;
            };
            // The refused lines carry their head inside them, so that it
            // goes with them while no line is refused; they are the one jump
            // row with no key, Locations and Pinned being jump rows too.
            let refused_lines = setting.key.is_none() && setting.control == Control::Jump;
            if let Some(said) = head(setting).filter(|_| !refused_lines) {
                body.append(&group_head(said, first));
            }
            first = false;
            body.append(&block);
            match (setting.key, setting.control) {
                (None, Control::Jump) => refused = block.clone().downcast::<gtk::Box>().ok(),
                (Some("spell_language"), _) => {
                    language = control
                        .as_ref()
                        .and_then(|c| c.clone().downcast::<gtk::DropDown>().ok())
                        .zip(block.last_child().and_downcast::<gtk::Label>());
                }
                _ => {}
            }
            drawn.push((setting, block));
            // The Template's five radios are one group, the first its leader.
            if let Some(radio) = control
                .as_ref()
                .and_then(|c| c.downcast_ref::<gtk::CheckButton>())
            {
                match &leader {
                    Some(leader) => radio.set_group(Some(leader)),
                    None => leader = Some(radio.clone()),
                }
            }
            if let Some(control) = control {
                rows.push(Row { setting, control });
            }
        }
        let scroller = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .child(&body)
            .build();
        places.extend(drawn.into_iter().map(|(setting, block)| search::Place {
            setting,
            block,
            scroller: scroller.clone(),
        }));
        panes.add_named(&scroller, Some(each.name()));
    }

    let nav = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::Browse)
        .css_classes(["settings-nav"])
        .build();
    for each in Pane::ALL {
        nav.append(
            &gtk::Label::builder()
                .label(each.name())
                .halign(gtk::Align::Start)
                .build(),
        );
    }
    nav.connect_row_selected(glib::clone!(
        #[strong]
        panes,
        move |_, row| {
            let Some(shown) = row
                .and_then(|row| usize::try_from(row.index()).ok())
                .and_then(|at| Pane::ALL.get(at))
            else {
                return;
            };
            panes.set_visible_child_name(shown.name());
            showed(*shown);
        }
    ));

    let side = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .width_request(SIDE)
        .css_classes(["settings-side"])
        .build();
    side.append(
        &gtk::Label::builder()
            .label("SETTINGS")
            .halign(gtk::Align::Start)
            .css_classes(["settings-side-head"])
            .build(),
    );
    let (field, entry) = field();
    side.append(&field);
    side.append(&nav);

    let root = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    root.append(&side);
    root.append(&search::wire(&window, &entry, &panes, &nav, places));
    window.set_child(Some(&root));

    select(&nav, pane);
    window.present();
    Open {
        window: window.downgrade(),
        nav,
        rows,
        language,
        refused,
    }
}

/// Selects `pane`'s row in the sidebar, which shows the pane.
fn select(nav: &gtk::ListBox, pane: Pane) {
    let at = Pane::ALL.iter().position(|each| *each == pane).unwrap_or(0);
    nav.select_row(nav.row_at_index(i32::try_from(at).unwrap_or(0)).as_ref());
}

/// What a pane shows for `setting`, and the control on it: its row, or for
/// Locations and Pinned the path list under their head, or for the refused
/// lines the lines under theirs, hidden while none is refused, neither with a
/// control the refresh stands; `None` for a row [`control`] has no control
/// for.
fn block(
    window: &gtk::Window,
    session: &Rc<Session>,
    spelling: Option<&Resolved>,
    setting: &'static Setting,
) -> Option<(gtk::Widget, Option<gtk::Widget>)> {
    match (setting.key, setting.control) {
        (Some("library.locations"), _) => Some((location_list(window, session).upcast(), None)),
        (Some("library.pinned"), _) => Some((pinned_list(session).upcast(), None)),
        (None, Control::Jump) => Some((refused_lines(session, setting.label).upcast(), None)),
        (Some("spell_language"), _) => {
            let language = control(session, setting, spelling)?;
            let said = language_line(language.downcast_ref()?, session, spelling);
            let block = gtk::Box::new(gtk::Orientation::Vertical, 0);
            block.append(&setting_row(setting, &language));
            block.append(&said);
            Some((block.upcast(), Some(language)))
        }
        (_, Control::Radio { .. }) => {
            let radio = control(session, setting, spelling)?;
            Some((
                mark_row(setting, radio.downcast_ref()?).upcast(),
                Some(radio),
            ))
        }
        _ => {
            let control = control(session, setting, spelling)?;
            Some((setting_row(setting, &control).upcast(), Some(control)))
        }
    }
}

/// The control `setting`'s row carries, standing on the effective value and
/// writing through the row's own write function: the one builder the
/// window's rows and the Palette's settings rows both read (#467), keyed by
/// the table.
///
/// `None` for a row with no control of its own here: the path lists and the
/// refused lines, which the window draws whole. A Template's radio comes
/// back alone; the window puts the five in one group ([`open_on`]).
pub(crate) fn control(
    session: &Rc<Session>,
    setting: &Setting,
    spelling: Option<&Resolved>,
) -> Option<gtk::Widget> {
    if setting.control == Control::Button {
        return Some(edit_button(session).upcast());
    }
    let control: gtk::Widget = match setting.key? {
        "theme" if setting.command == Some("theme.toggle") => {
            switch(session, session.scheme() == Scheme::Dark, darkened).upcast()
        }
        "theme" => follow_switch(session).upcast(),
        "typewriter_anchor" => anchor_scale(session).upcast(),
        "spell_language" => language(
            session,
            &quill_engine::spell::installed_languages(),
            spelling,
        )
        .upcast(),
        "export.paper" => {
            let paper = session.settings().export.paper;
            export_papers(session, paper).upcast()
        }
        "export.margin" => {
            let margin = session.settings().export.margin;
            export_spin(session, margin, &export_margins(), export_margin).upcast()
        }
        "export.text_size" => {
            let size = session.settings().export.text_size;
            export_spin(session, size, &export_text_sizes(), export_text_size).upcast()
        }
        "chrome" => switch(session, session.chrome() == Chrome::Hidden, hid_bars).upcast(),
        "template.name" => {
            let Control::Radio { value } = setting.control else {
                return None;
            };
            let name = TemplateName::parse(value)?;
            template_radio(session, session.template().name == name, name).upcast()
        }
        key => {
            let (read, write) = toggle(key)?;
            switch(session, read(&session.running()), write).upcast()
        }
    };
    Some(control)
}

/// The read and write of the switch row writing `key`, for every switch but
/// Follow System, whose write needs the ground on screen.
fn toggle(key: &str) -> Option<Toggle> {
    let toggle: Toggle = match key {
        "library.show_hidden" => (|settings| settings.library.show_hidden, showed_hidden),
        "library.show_extensions" => (
            |settings| settings.library.show_extensions,
            showed_extensions,
        ),
        "library.confirm_move" => (|settings| settings.library.confirm_move, confirmed_move),
        "library.ask_where_to_save" => (
            |settings| settings.library.ask_where_to_save,
            asked_where_to_save,
        ),
        "template.center_headings" => (
            |settings| settings.template.center_headings,
            centered_headings,
        ),
        "template.number_headings" => (
            |settings| settings.template.number_headings,
            numbered_headings,
        ),
        "template.indent_paragraphs" => (
            |settings| settings.template.indent_paragraphs,
            indented_paragraphs,
        ),
        "export.title_page" => (|settings| settings.export.title_page, export_title_page),
        "export.header" => (|settings| settings.export.header, export_header),
        "export.footer" => (|settings| settings.export.footer, export_footer),
        _ => return None,
    };
    Some(toggle)
}

/// A small-caps group head. GTK's CSS has no `text-transform`, so the
/// capitals are made here; `first` is a head at the top of its pane, which
/// takes no air above it.
fn group_head(said: &str, first: bool) -> gtk::Box {
    let line = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .css_classes(["settings-head"])
        .build();
    if first {
        line.add_css_class("first");
    }
    line.append(
        &gtk::Label::builder()
            .label(said.to_uppercase())
            .css_classes(["settings-caps"])
            .build(),
    );
    line
}

/// One row of a pane: `setting`'s words, with its [`hint`] under them, and
/// `control` at the right end.
fn setting_row(setting: &Setting, control: &gtk::Widget) -> gtk::Box {
    let row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(ROW_GAP)
        .hexpand(true)
        .css_classes(["settings-row"])
        .build();
    let words = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .hexpand(true)
        .valign(gtk::Align::Center)
        .build();
    // The button's row is named for what the file holds; the button says what
    // it does.
    let label = if setting.control == Control::Button {
        FILE_ROW
    } else {
        setting.label
    };
    words.append(
        &gtk::Label::builder()
            .label(label)
            .halign(gtk::Align::Start)
            .build(),
    );
    if let Some(hint) = hint(setting) {
        words.append(
            &gtk::Label::builder()
                .label(hint)
                .halign(gtk::Align::Start)
                .ellipsize(gtk::pango::EllipsizeMode::End)
                .css_classes(["settings-hint"])
                .build(),
        );
        row.add_css_class("tall");
    }
    control.set_valign(gtk::Align::Center);
    control.set_halign(gtk::Align::End);
    row.append(&words);
    row.append(control);
    row
}

/// A radio's row: the radio with `setting`'s words as its own label, so a
/// click on the words picks it, at the left of the row as the stub drew it.
fn mark_row(setting: &Setting, radio: &gtk::CheckButton) -> gtk::Box {
    radio.set_label(Some(setting.label));
    radio.set_hexpand(true);
    let row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .hexpand(true)
        .css_classes(["settings-check"])
        .build();
    row.append(radio);
    row
}

/// The sidebar's search field: a plain entry, the Palette's drawn magnifier
/// over its left end and Quill's own ✕ at its right while it holds text, so
/// nothing in it comes from the icon theme (#467 § Icons are Quill's own);
/// and the entry, which [`search::wire`] wires.
fn field() -> (gtk::Overlay, gtk::Entry) {
    let entry = gtk::Entry::builder()
        .placeholder_text("Search")
        .width_chars(FIELD_CHARS)
        .max_width_chars(FIELD_CHARS)
        .css_classes(["search"])
        .build();
    let clear = quill_icon(quill_engine::data::CLEAR);
    entry.connect_changed(move |entry| {
        let gone = entry.text().is_empty();
        entry.set_secondary_icon_gicon((!gone).then_some(&clear));
    });
    entry.connect_icon_release(|entry, at| {
        if at == gtk::EntryIconPosition::Secondary {
            entry.set_text("");
        }
    });
    let mag = crate::chrome::icon(MAG, MAG, crate::palette::magnifier_icon);
    mag.add_css_class("settings-mag");
    mag.set_halign(gtk::Align::Start);
    mag.set_margin_start(MAG_INSET);
    mag.set_margin_bottom(FIELD_BELOW);
    let field = gtk::Overlay::builder().child(&entry).build();
    field.add_overlay(&mag);
    (field, entry)
}

/// One of Quill's own icons under [`quill_engine::data::icons`], as the
/// `GIcon` an image or an entry takes; a name ending `-symbolic.svg` is
/// recoloured to the widget's CSS `color`.
fn quill_icon(name: &str) -> gio::FileIcon {
    gio::FileIcon::new(&gio::File::for_path(quill_engine::data::icons().join(name)))
}

/// Follow System: on while the theme follows the desktop.
fn follow_switch(session: &Rc<Session>) -> gtk::Switch {
    let follow = gtk::Switch::new();
    follow.set_active(session.settings().theme == Theme::Auto);
    follow.connect_active_notify(glib::clone!(
        #[strong]
        session,
        move |follow| {
            let on = follow.is_active();
            let scheme = session.scheme();
            put(&session, |settings| followed(settings, on, scheme));
        }
    ));
    follow
}

/// The Typewriter anchor's scale, its value written at its left.
fn anchor_scale(session: &Rc<Session>) -> gtk::Scale {
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
    anchor.set_value_pos(gtk::PositionType::Left);
    anchor.set_width_request(ANCHOR_WIDTH);
    // Set before the handler is connected, so that opening the window is not
    // itself a write.
    anchor.set_value(session.settings().typewriter_anchor);
    anchor.connect_value_changed(glib::clone!(
        #[strong]
        session,
        move |anchor| {
            let value = anchor.value();
            put(&session, |settings| anchored(settings, value));
        }
    ));
    anchor
}

/// Edit settings.toml…: hands the file to the desktop's editor.
fn edit_button(session: &Rc<Session>) -> gtk::Button {
    let button = gtk::Button::builder().label("Edit settings.toml…").build();
    let launch = launcher();
    button.connect_clicked(glib::clone!(
        #[strong]
        session,
        move |_| edit(session.settings_path(), launch.as_ref())
    ));
    button
}

/// The Spell check language dropdown over the `installed` dictionaries,
/// standing on the language the file names before its handler is connected,
/// so opening it is not a write.
fn language(
    session: &Rc<Session>,
    installed: &[String],
    spelling: Option<&Resolved>,
) -> gtk::DropDown {
    let current = session.settings().spell_language.clone();
    let (rows, selected) = language_rows(installed, &current, spelling);
    let words: Vec<&str> = rows.iter().map(String::as_str).collect();
    let language = gtk::DropDown::from_strings(&words);
    language.set_selected(selected);
    ticked(&language);
    language.connect_selected_notify(glib::clone!(
        #[strong]
        session,
        move |language| {
            // A refresh stands the rows as well as the pick, so the rows are
            // read off the dropdown rather than kept beside it.
            let mut rows = words_of(language);
            let Some(chosen) = language_at(&rows, language.selected()) else {
                return;
            };
            if QUIET.with(Cell::get) {
                return;
            }
            put(&session, |settings| chose_language(settings, chosen));
            // The "No dictionary installed" row was a state, and the state
            // has moved on: it goes, and it is after every other row, so the
            // row stood on keeps its place.
            if let Some(at) = served_rows(&mut rows)
                && let Some(model) = language.model().and_downcast::<gtk::StringList>()
            {
                model.remove(at);
            }
        }
    ));
    language
}

/// Stands the language dropdown on `wanted`, the file's language, by what
/// `spelling` resolved it to — its rows as well as its pick, since the "No
/// dictionary installed" row is a state — and the line under it with them.
fn stand_language(
    language: &gtk::DropDown,
    said: &gtk::Label,
    wanted: &str,
    spelling: Option<&Resolved>,
) {
    let installed = quill_engine::spell::installed_languages();
    let (rows, selected) = language_rows(&installed, wanted, spelling);
    if words_of(language) != rows
        && let Some(model) = language.model().and_downcast::<gtk::StringList>()
    {
        let words: Vec<&str> = rows.iter().map(String::as_str).collect();
        model.splice(0, model.n_items(), &words);
    }
    if language.selected() != selected {
        language.set_selected(selected);
    }
    show_unserved(said, missing(spelling));
}

/// The language `spelling` wanted and found no dictionary for, if that is
/// what it resolved to.
fn missing(spelling: Option<&Resolved>) -> Option<String> {
    match spelling {
        Some(Resolved::Missing { wanted }) => Some(wanted.clone()),
        _ => None,
    }
}

/// The "no dictionary" line under the language row, shown only while it
/// holds: what `spelling` resolved as the window opened, and after a choice
/// here what that choice resolves to by the same ladder, since the Editor
/// hears of it only once the settings watch has read the file back (#401's
/// Hand test, step 12). Spell check's own state is the session's, the switch
/// having left the window for the View menu.
fn language_line(
    language: &gtk::DropDown,
    session: &Rc<Session>,
    spelling: Option<&Resolved>,
) -> gtk::Label {
    let said = gtk::Label::builder()
        .halign(gtk::Align::Start)
        .xalign(0.0)
        .wrap(true)
        .css_classes(["settings-hint"])
        .build();
    show_unserved(&said, missing(spelling));
    let installed = quill_engine::spell::installed_languages();
    language.connect_selected_notify(glib::clone!(
        #[strong]
        session,
        #[strong]
        said,
        move |language| {
            let Some(chosen) = language_at(&words_of(language), language.selected()) else {
                return;
            };
            show_unserved(&said, unserved_now(session.spell(), &chosen, &installed));
        }
    ));
    said
}

/// The rows a dropdown over strings stands on, in its order.
fn words_of(drop_down: &gtk::DropDown) -> Vec<String> {
    let Some(model) = drop_down.model() else {
        return Vec::new();
    };
    (0..model.n_items())
        .filter_map(|at| model.item(at).and_downcast::<gtk::StringObject>())
        .map(|word| word.string().to_string())
        .collect()
}

/// Gives a dropdown's popup Quill's own rows: the word, then a column holding
/// the tick on the row stood on, where the stock rows put the tick straight
/// after the word (the stub's README).
fn ticked(drop_down: &gtk::DropDown) {
    let factory = gtk::SignalListItemFactory::new();
    let owner = drop_down.downgrade();
    factory.connect_setup(move |_, item| {
        let Some(item) = item.downcast_ref::<gtk::ListItem>() else {
            return;
        };
        let line = gtk::Box::new(gtk::Orientation::Horizontal, LIST_GAP);
        line.append(
            &gtk::Label::builder()
                .halign(gtk::Align::Start)
                .hexpand(true)
                .build(),
        );
        let tick = gtk::Image::from_gicon(&gio::FileIcon::new(&gio::File::for_uri(sheet::TICK)));
        line.append(&tick);
        item.set_child(Some(&line));
        // The tick moves with the pick while the popup stands.
        if let Some(owner) = owner.upgrade() {
            let (item, tick) = (item.downgrade(), tick.downgrade());
            owner.connect_selected_notify(move |owner| {
                if let (Some(item), Some(tick)) = (item.upgrade(), tick.upgrade()) {
                    stand(&tick, item.position() == owner.selected());
                }
            });
        }
    });
    let owner = drop_down.downgrade();
    factory.connect_bind(move |_, item| {
        let Some(item) = item.downcast_ref::<gtk::ListItem>() else {
            return;
        };
        let Some(line) = item.child() else {
            return;
        };
        if let Some(label) = line.first_child().and_downcast::<gtk::Label>()
            && let Some(word) = item.item().and_downcast::<gtk::StringObject>()
        {
            label.set_label(&word.string());
        }
        if let (Some(tick), Some(owner)) = (line.last_child(), owner.upgrade()) {
            stand(&tick, item.position() == owner.selected());
        }
    });
    drop_down.set_list_factory(Some(&factory));
}

/// Shows the tick or hides it, keeping its column either way.
fn stand(tick: &impl IsA<gtk::Widget>, on: bool) {
    tick.set_opacity(if on { 1.0 } else { 0.0 });
}

/// The refused lines of the last read of the settings file under `said`,
/// drawn always and shown only while a line is refused, so that an open
/// window follows a file that gains or loses one ([`Open::refresh`]).
fn refused_lines(session: &Session, said: &str) -> gtk::Box {
    let slot = gtk::Box::new(gtk::Orientation::Vertical, 0);
    slot.append(&group_head(said, false));
    let lines = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .css_classes(["settings-refused"])
        .build();
    lines.append(
        &gtk::Label::builder()
            .halign(gtk::Align::Start)
            .xalign(0.0)
            .wrap(true)
            .wrap_mode(gtk::pango::WrapMode::WordChar)
            .selectable(true)
            .build(),
    );
    slot.append(&lines);
    fill_refused(&slot, session);
    slot
}

/// Says in `slot` what the last read of the file refused, and shows it only
/// while something was.
fn fill_refused(slot: &gtk::Box, session: &Session) {
    let said = refused(&session.unapplied());
    if let Some(label) = slot
        .last_child()
        .and_then(|lines| lines.first_child())
        .and_downcast::<gtk::Label>()
    {
        label.set_label(said.as_deref().unwrap_or_default());
    }
    slot.set_visible(said.is_some());
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

/// The wanted tag with no dictionary, for Spell check `on` and `language`,
/// with the locale read from this process.
fn unserved_now(on: bool, language: &str, installed: &[String]) -> Option<String> {
    unserved(on, language, installed, |name| std::env::var(name).ok())
}

/// The tag `language` wants when no installed dictionary serves it and Spell
/// check is `on`; `None` when a dictionary serves it or Spell check is off,
/// since the state is Spell check's and not the language's.
///
/// The same ladder [`crate::editor::Editor`] resolves a Document's language
/// by, [`quill_engine::spell::resolve_setting`], with the locale read through
/// `locale`.
fn unserved(
    on: bool,
    language: &str,
    installed: &[String],
    locale: impl Fn(&str) -> Option<String>,
) -> Option<String> {
    if !on {
        return None;
    }
    match quill_engine::spell::resolve_setting(language, installed, locale) {
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

/// What choosing a language writes.
fn chose_language(settings: &mut Settings, language: String) {
    settings.spell_language = language;
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
            put(&session, |settings| write(settings, on));
        }
    ));
    switch
}

/// The paper dropdown of the Export group, over the rows the Export dialog
/// offers ([`paper_drop_down`]) and standing on `paper` before its handler is
/// connected.
///
/// The dialog's own list rather than a second one, so that a paper named here
/// and a paper named there cannot drift apart.
fn export_papers(session: &Rc<Session>, paper: Paper) -> gtk::DropDown {
    let papers = paper_drop_down(paper);
    ticked(&papers);
    papers.connect_selected_notify(glib::clone!(
        #[strong]
        session,
        move |papers| {
            let paper = paper_at(papers.selected());
            put(&session, |settings| export_paper(settings, paper));
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
    spin.set_width_chars(SPIN_CHARS);
    spin.set_max_width_chars(SPIN_CHARS);
    // Quill's − and + in place of the icon theme's: the spin button's own
    // buttons, each given an image of Quill's.
    let mut child = spin.first_child();
    while let Some(widget) = child {
        if let Some(button) = widget.downcast_ref::<gtk::Button>() {
            let glyph = if button.has_css_class("down") {
                quill_engine::data::MINUS
            } else {
                quill_engine::data::PLUS
            };
            button.set_child(Some(&gtk::Image::from_gicon(&quill_icon(glyph))));
        }
        child = widget.next_sibling();
    }
    spin.set_value(f64::from(value));
    spin.connect_value_changed(glib::clone!(
        #[strong]
        session,
        move |spin| {
            // The button was built over the range, so its value is inside it;
            // the floor is the answer for a value no `u32` can hold.
            let value = u32::try_from(spin.value_as_int()).unwrap_or(low);
            put(&session, |settings| write(settings, value));
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
    // A boxed list, its rounded corners from the widget's overflow rather
    // than CSS (the stub's README), and no box at all while it is empty.
    let list = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .hexpand(true)
        .overflow(gtk::Overflow::Hidden)
        .css_classes(["settings-paths"])
        .visible(!held.is_empty())
        .build();
    let held = Rc::new(RefCell::new(held));
    for path in held.borrow().clone() {
        let line = line(session, &list, &held, &path, write);
        list.append(&line);
    }
    (list, held)
}

/// A path with the home folder written `~`, as the canvas boards write it.
fn tilde(path: &Path) -> String {
    let home = glib::home_dir();
    path.strip_prefix(&home).map_or_else(
        |_| path.display().to_string(),
        |rest| format!("~/{}", rest.display()),
    )
}

/// One line of such a list: the path, and the button that drops it.
fn line(
    session: &Rc<Session>,
    list: &gtk::Box,
    held: &Rc<RefCell<Vec<PathBuf>>>,
    path: &Path,
    write: WritePaths,
) -> gtk::Box {
    let line = gtk::Box::new(gtk::Orientation::Horizontal, PATH_GAP);
    let label = gtk::Label::builder()
        .label(tilde(path))
        .halign(gtk::Align::Start)
        .hexpand(true)
        // A long path is cut at the front: the folder it ends in is the half
        // that says which one it is.
        .ellipsize(gtk::pango::EllipsizeMode::Start)
        .build();
    let remove = gtk::Button::builder()
        .label("Remove")
        .valign(gtk::Align::Center)
        .css_classes(["settings-small"])
        .build();
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
            list.set_visible(list.first_child().is_some());
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
                        list.set_visible(true);
                    }
                ),
            );
        }
    ));
    let column = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(LIST_GAP)
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

/// What Hide Bars writes: the key `chrome.toggle` moves, which the watch puts
/// on to every window's bars.
fn hid_bars(settings: &mut Settings, on: bool) {
    settings.chrome = if on { Chrome::Hidden } else { Chrome::Shown };
}

/// What Dark Mode writes: the ground it asks for, pinned, which stops Follow
/// System as `Ctrl+Shift+L` does ([`Scheme::setting`]).
fn darkened(settings: &mut Settings, on: bool) {
    let scheme = if on { Scheme::Dark } else { Scheme::Light };
    settings.theme = scheme.setting();
}

/// What a Template's radio writes: the name the `template.*` radios pick.
fn named_template(settings: &mut Settings, name: TemplateName) {
    settings.template.name = name;
}

/// One Template's radio, on while the page is laid out in `name` and set so
/// before its handler is connected, so opening the window is not a write;
/// picking it writes `name`, and the one it leaves writes nothing.
fn template_radio(session: &Rc<Session>, on: bool, name: TemplateName) -> gtk::CheckButton {
    let radio = gtk::CheckButton::new();
    radio.set_active(on);
    radio.connect_toggled(glib::clone!(
        #[strong]
        session,
        move |radio| {
            if radio.is_active() {
                put(&session, |settings| named_template(settings, name));
            }
        }
    ));
    radio
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

    /// The value `key`, dotted under its table, has in the file `settings`
    /// writes, as the file spells it.
    fn written(settings: &Settings, key: &str) -> Option<String> {
        let (table, name) = key.rsplit_once('.').unwrap_or(("", key));
        let mut under = String::new();
        for line in settings.to_toml().lines() {
            if let Some(head) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
                head.clone_into(&mut under);
            } else if under == table
                && let Some((said, value)) = line.split_once(" = ")
                && said == name
            {
                return Some(value.to_owned());
            }
        }
        None
    }

    /// The panes are the table's: five in the sidebar's order, each holding
    /// its rows in the table's order (the engine's table test holds the rows
    /// to none the View menu holds).
    #[test]
    fn the_panes_are_the_tables_rows() {
        let labels = |pane| {
            rows_of(pane)
                .map(|setting| setting.label)
                .collect::<Vec<_>>()
        };
        assert_eq!(
            Pane::ALL.map(Pane::name),
            ["General", "Library", "Template", "Export", "Advanced"]
        );
        assert_eq!(
            labels(Pane::General),
            [
                "Dark Mode",
                "Follow System",
                "Hide Bars",
                "Typewriter anchor",
                "Spell check language"
            ]
        );
        assert_eq!(
            labels(Pane::Library),
            [
                "Locations",
                "Pinned",
                "Show hidden folders",
                "Show file extensions",
                "Confirm before moving files",
                "Always ask where to save"
            ]
        );
        assert_eq!(
            labels(Pane::Template),
            [
                "Modern",
                "Classic",
                "Manuscript Mono",
                "Manuscript Duo",
                "Manuscript Quattro",
                "Center headings",
                "Number headings",
                "Indent paragraphs"
            ]
        );
        assert_eq!(
            labels(Pane::Export),
            [
                "Paper",
                "Margin (mm)",
                "Text size (pt)",
                "Title page",
                "Header",
                "Footer"
            ]
        );
        assert_eq!(
            labels(Pane::Advanced),
            ["Edit settings.toml…", "Not applied from settings.toml"]
        );
        // Every head stands over a row the table has.
        let heads: Vec<_> = SETTINGS_ROWS.iter().filter_map(head).collect();
        assert_eq!(
            heads,
            [
                "Locations",
                "Pinned",
                "Files",
                "Template",
                "Layout",
                "Not applied from settings.toml"
            ]
        );
    }

    /// Every switch but the ground's two and Hide Bars' reads the key it
    /// writes: flipped in the file and applied, the row reads the flip, and
    /// its write puts each value back under that key alone.
    #[test]
    fn every_switch_row_reads_and_writes_the_key_the_table_names() {
        let (session, path) = launched("switch-rows-keys");
        let switches = SETTINGS_ROWS
            .iter()
            .filter(|setting| setting.control == Control::Switch);
        let mut unbuilt = Vec::new();
        for setting in switches {
            let key = setting.key.unwrap();
            let Some((read, write)) = toggle(key) else {
                unbuilt.push(key);
                continue;
            };
            for on in [true, false] {
                let mut settings = session.settings().clone();
                write(&mut settings, on);
                assert_eq!(written(&settings, key), Some(on.to_string()), "{key}");
                session.apply(settings);
                assert_eq!(read(&session.running()), on, "{key}");
            }
        }
        assert_eq!(
            unbuilt,
            ["theme", "theme", "chrome"],
            "every other switch is built"
        );
        std::fs::remove_file(path).ok();
    }

    /// The first open of a run shows General, every later one the pane last
    /// shown, and showing one writes nothing: the settings file is as the
    /// launch left it.
    #[test]
    fn the_last_pane_starts_at_general_and_is_kept_in_the_process_alone() {
        let (_session, path) = launched("last-pane");
        let before = std::fs::read_to_string(&path).ok();
        assert_eq!(last_pane(), Pane::General);
        showed(Pane::Export);
        assert_eq!(last_pane(), Pane::Export);
        showed(Pane::Library);
        assert_eq!(last_pane(), Pane::Library);
        assert_eq!(
            std::fs::read_to_string(&path).ok(),
            before,
            "the pane is not a setting"
        );
        std::fs::remove_file(path).ok();
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

    /// Dark Mode pins the ground it asks for, which stops Follow System, and
    /// stands on the ground on screen: under Follow System that is the
    /// desktop's, so a dark desktop shows it on with `auto` in the file.
    #[test]
    fn dark_mode_pins_a_ground_and_stands_on_the_one_on_screen() {
        let (session, path) = launched("dark-mode");
        session.edit_settings(|settings| darkened(settings, true));
        assert_eq!(Settings::read_from(&path).0.theme, Theme::Dark);
        session.edit_settings(|settings| darkened(settings, false));
        assert_eq!(Settings::read_from(&path).0.theme, Theme::Light);

        let dark = row("Dark Mode");
        let follow = row("Follow System");
        let mut settings = Settings::default();
        settings.theme = Theme::Auto;
        assert_eq!(
            standing(&settings, Scheme::Dark, dark),
            Some(Standing::On(true))
        );
        assert_eq!(
            standing(&settings, Scheme::Dark, follow),
            Some(Standing::On(true)),
            "the two switches are on together"
        );
        assert_eq!(
            moved(&settings, Scheme::Light, dark, Standing::On(true)),
            Some(Standing::On(false)),
            "the desktop going light turns it off"
        );
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

    /// The table's row labelled `label`.
    fn row(label: &str) -> &'static Setting {
        SETTINGS_ROWS
            .iter()
            .find(|setting| setting.label == label)
            .unwrap()
    }

    /// Hide Bars writes the key `chrome.toggle` moves, and a Template's radio
    /// the name the `template.*` radios pick; each is read back from the file
    /// and put on to the launch, beside the checks it leaves where they were.
    #[test]
    fn hide_bars_and_the_template_name_round_trip_through_the_file() {
        let (session, path) = launched("bars-and-template");
        for on in [true, false] {
            let written = wrote(&session, &path, |settings| hid_bars(settings, on));
            let chrome = if on { Chrome::Hidden } else { Chrome::Shown };
            assert_eq!(written.chrome, chrome);
            assert_eq!(session.chrome(), chrome, "the bars follow the file");
        }
        wrote(&session, &path, |settings| {
            numbered_headings(settings, true)
        });
        for name in [TemplateName::Classic, TemplateName::ManuscriptQuattro] {
            let written = wrote(&session, &path, |settings| named_template(settings, name));
            assert_eq!(written.template.name, name);
            assert_eq!(session.template().name, name, "the page follows the file");
            assert!(written.template.number_headings, "the check stays on");
        }
        std::fs::remove_file(&path).ok();
    }

    /// The refresh stands a row again only where the settings moved from
    /// what it shows: Hide Bars under a pressed `Ctrl+Shift+H`, the radios
    /// under another Template, the anchor past half a step — and nothing
    /// where the pane already shows the settings, the echo of its own write.
    #[test]
    fn the_refresh_moves_only_the_rows_the_settings_moved_from() {
        let mut settings = Settings::default();
        let bars = row("Hide Bars");
        let modern = row("Modern");
        let classic = row("Classic");
        let anchor = row("Typewriter anchor");
        let paper = row("Paper");
        let light = Scheme::Light;
        let showing =
            |settings: &Settings, setting: &Setting| standing(settings, light, setting).unwrap();
        for setting in [bars, modern, classic, anchor, paper] {
            let shown = showing(&settings, setting);
            assert_eq!(
                moved(&settings, light, setting, shown),
                None,
                "{}",
                setting.label
            );
        }

        let before = settings.clone();
        settings.chrome = Chrome::Hidden;
        assert_eq!(
            moved(&settings, light, bars, showing(&before, bars)),
            Some(Standing::On(true))
        );
        settings.template.name = TemplateName::Classic;
        assert_eq!(
            moved(&settings, light, classic, showing(&before, classic)),
            Some(Standing::On(true))
        );
        assert_eq!(
            moved(&settings, light, modern, showing(&before, modern)),
            Some(Standing::On(false))
        );
        assert_eq!(
            moved(&settings, light, paper, showing(&before, paper)),
            None
        );

        let dragged = Standing::Fraction(settings.typewriter_anchor + ANCHOR_STEP / 4.0);
        assert_eq!(
            moved(&settings, light, anchor, dragged),
            None,
            "within the scale's rounding"
        );
        settings.typewriter_anchor += ANCHOR_STEP * 10.0;
        assert_eq!(
            moved(&settings, light, anchor, showing(&before, anchor)),
            Some(Standing::Fraction(settings.typewriter_anchor))
        );

        let language = row("Spell check language");
        assert_eq!(
            standing(&settings, light, language),
            None,
            "left as it opened"
        );
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

    /// The language dropdown writes `spell_language` alone and beside what the
    /// file already said, and the session reads it back once the file is
    /// applied.
    #[test]
    fn the_language_row_writes_its_own_key_and_reflects_after_the_file_is_applied() {
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
        std::fs::remove_file(path).unwrap();
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
