//! THROWAWAY (#464): the picked Settings window, A1 with D's search as a spike. Never merged.
//!
//! A1 is a `gtk::Window` holding a sidebar (a `ListBox`) beside a `gtk::Stack`
//! of six panes. D is the search field at the head of the sidebar: while it
//! holds text the panes give way to one flat `ListBox` of the matching rows,
//! and the rows in it are the panes' own widgets, moved out of their slots and
//! put back when the field empties, so there is one switch per setting and
//! never two to keep in step.
//!
//! What the prototype reads from the environment, so a script can shoot it
//! without driving it: `QUILL_STUB_PANE` (the pane to open on),
//! `QUILL_STUB_QUERY` (the search text), `QUILL_STUB_POPUP` (open Paper's
//! dropdown), `QUILL_STUB_KEYS=stock` (no key handling of the stub's own) and
//! `QUILL_STUB_LOG` (the focus widget and each operated row on stderr).

use std::cell::{Cell, RefCell};
use std::path::Path;
use std::rc::Rc;
use std::time::Duration;

use gtk::prelude::*;
use gtk::{gdk, glib};

use quill_engine::settings::{Chrome, Settings, TemplateName, Theme};
use quill_engine::spell::Resolved;
use quill_engine::style::List;
use quill_engine::theme::Scheme;

use super::{
    ANCHOR_DIGITS, ANCHOR_HIGH, ANCHOR_LOW, ANCHOR_STEP, ENGLISH_ONLY, anchored,
    asked_where_to_save, centered_headings, chose_language, confirmed_move, edit, export_footer,
    export_header, export_margin, export_papers, export_spin, export_text_size, export_title_page,
    followed, indented_paragraphs, language_at, language_rows, launcher, location_list,
    no_dictionary, numbered_headings, pinned_list, served_rows, showed_extensions, showed_hidden,
    style_rows, switch, unserved_now,
};
use crate::chrome::CHROME_FONT;
use crate::session::{Session, StyleToggle};

const PANES: [&str; 6] = [
    "General",
    "Library",
    "Template",
    "Export",
    "Writing tools",
    "Advanced",
];

const TEMPLATES: [(TemplateName, &str); 5] = [
    (TemplateName::Modern, "Modern"),
    (TemplateName::Classic, "Classic"),
    (TemplateName::ManuscriptMono, "Manuscript Mono"),
    (TemplateName::ManuscriptDuo, "Manuscript Duo"),
    (TemplateName::ManuscriptQuattro, "Manuscript Quattro"),
];

/// One settings row the search can find: the pane's own widget, and the slot
/// it goes back to.
struct Found {
    pane: &'static str,
    label: String,
    row: gtk::Widget,
    slot: gtk::Box,
    hint: Option<gtk::Label>,
    control: gtk::Widget,
}

/// A result that is not a control: it names a pane and jumps to it.
struct Jump {
    pane: &'static str,
    label: &'static str,
    said: String,
    warn: bool,
}

#[derive(Clone, Copy)]
enum Hit {
    Row(usize),
    Jump(usize),
}

struct State {
    found: Vec<Found>,
    jumps: Vec<Jump>,
    results: gtk::ListBox,
    outer: gtk::Stack,
    inner: gtk::Stack,
    nav: gtk::ListBox,
    search: gtk::SearchEntry,
    /// The rows standing in the results now, in the results' order.
    shown: RefCell<Vec<Hit>>,
}

struct Build {
    session: Rc<Session>,
    inner: gtk::Stack,
    found: Vec<Found>,
    jumps: Vec<Jump>,
    body: gtk::Box,
    pane: &'static str,
    first: Cell<bool>,
}

fn log(said: &str) {
    if std::env::var_os("QUILL_STUB_LOG").is_some() {
        eprintln!("stub: {said}");
    }
}

/// A path with the home folder written `~`, as the boards write it.
pub(super) fn tilde(path: &Path) -> String {
    let home = glib::home_dir();
    path.strip_prefix(&home).map_or_else(
        |_| path.display().to_string(),
        |rest| format!("~/{}", rest.display()),
    )
}

impl Build {
    fn pane(&mut self, name: &'static str) {
        let body = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .css_classes(["settings-pane"])
            .build();
        let scrolled = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .child(&body)
            .build();
        self.inner.add_named(&scrolled, Some(name));
        self.body = body;
        self.pane = name;
        self.first.set(true);
    }

    /// A small-caps group head. GTK's CSS has no `text-transform`, so the
    /// capitals are made here.
    fn head(&self, said: &str, hint: Option<&str>) {
        let line = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(10)
            .css_classes(["settings-head"])
            .build();
        if self.first.replace(false) {
            line.add_css_class("first");
        }
        line.append(
            &gtk::Label::builder()
                .label(said.to_uppercase())
                .css_classes(["settings-caps"])
                .build(),
        );
        if let Some(hint) = hint {
            line.append(
                &gtk::Label::builder()
                    .label(hint)
                    .css_classes(["settings-hint"])
                    .build(),
            );
        }
        self.body.append(&line);
    }

    /// A label, an optional line under it, and the control at the right.
    fn setting(&mut self, label: &str, hint: Option<&str>, control: &impl IsA<gtk::Widget>) {
        self.first.set(false);
        let row = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(16)
            .hexpand(true)
            .css_classes(["settings-row"])
            .build();
        let words = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .hexpand(true)
            .valign(gtk::Align::Center)
            .build();
        words.append(
            &gtk::Label::builder()
                .label(label)
                .halign(gtk::Align::Start)
                .build(),
        );
        let hint = hint.map(|hint| {
            let hint = gtk::Label::builder()
                .label(hint)
                .halign(gtk::Align::Start)
                .ellipsize(gtk::pango::EllipsizeMode::End)
                .css_classes(["settings-hint"])
                .build();
            words.append(&hint);
            row.add_css_class("tall");
            hint
        });
        control.set_valign(gtk::Align::Center);
        control.set_halign(gtk::Align::End);
        row.append(&words);
        row.append(control);
        self.slotted(label, row.upcast(), hint, control.clone().upcast());
    }

    /// A check or a radio, whose label is its own.
    fn mark(&mut self, label: &str, check: &gtk::CheckButton) {
        self.first.set(false);
        check.set_label(Some(label));
        check.set_hexpand(true);
        let row = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .hexpand(true)
            .css_classes(["settings-check"])
            .build();
        row.append(check);
        self.slotted(label, row.upcast(), None, check.clone().upcast());
    }

    fn slotted(
        &mut self,
        label: &str,
        row: gtk::Widget,
        hint: Option<gtk::Label>,
        control: gtk::Widget,
    ) {
        let slot = gtk::Box::new(gtk::Orientation::Vertical, 0);
        slot.append(&row);
        self.body.append(&slot);
        self.found.push(Found {
            pane: self.pane,
            label: label.to_owned(),
            row,
            slot,
            hint,
            control,
        });
    }

    fn switch(
        &mut self,
        label: &str,
        hint: Option<&str>,
        on: bool,
        write: impl Fn(&mut Settings, bool) + 'static,
    ) {
        let control = switch(&self.session, on, write);
        self.setting(label, hint, &control);
    }
}

/// Opens the window.
#[expect(clippy::too_many_lines, reason = "a throwaway prototype (#464)")]
pub(super) fn open(parent: &gtk::Window, session: &Rc<Session>, spelling: Option<&Resolved>) {
    let inner = gtk::Stack::builder().hexpand(true).vexpand(true).build();
    let window = gtk::Window::builder()
        .title("Settings")
        .transient_for(parent)
        .destroy_with_parent(true)
        .default_width(700)
        .default_height(520)
        .css_classes(["settings"])
        .build();
    let mut build = Build {
        session: Rc::clone(session),
        inner: inner.clone(),
        found: Vec::new(),
        jumps: Vec::new(),
        body: gtk::Box::new(gtk::Orientation::Vertical, 0),
        pane: PANES[0],
        first: Cell::new(true),
    };

    // General.
    build.pane("General");
    let follow = gtk::Switch::new();
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
    build.setting(
        "Follow System",
        Some("Light and dark follow the desktop"),
        &follow,
    );
    build.switch(
        "Hide Bars",
        Some("The title and status bars fade while typing"),
        session.chrome() == Chrome::Hidden,
        |settings, on| {
            settings.chrome = if on { Chrome::Hidden } else { Chrome::Shown };
        },
    );
    let anchor = gtk::Scale::with_range(
        gtk::Orientation::Horizontal,
        ANCHOR_LOW,
        ANCHOR_HIGH,
        ANCHOR_STEP,
    );
    anchor.set_digits(ANCHOR_DIGITS);
    anchor.set_draw_value(true);
    anchor.set_value_pos(gtk::PositionType::Left);
    anchor.set_width_request(220);
    anchor.set_value(session.settings().typewriter_anchor);
    anchor.connect_value_changed(glib::clone!(
        #[strong]
        session,
        move |anchor| {
            let value = anchor.value();
            session.edit_settings(|settings| anchored(settings, value));
        }
    ));
    build.setting("Typewriter anchor", None, &anchor);

    // Library.
    build.pane("Library");
    let library = session.settings().library.clone();
    build.head("Locations", None);
    build.body.append(&location_list(&window, session));
    build.jumps.push(Jump {
        pane: "Library",
        label: "Locations",
        said: format!("{} folders", library.locations.len()),
        warn: false,
    });
    build.head("Pinned", None);
    build.body.append(&pinned_list(session));
    build.jumps.push(Jump {
        pane: "Library",
        label: "Pinned",
        said: format!("{} files", library.pinned.len()),
        warn: false,
    });
    build.head("Files", None);
    build.switch(
        "Show hidden folders",
        None,
        library.show_hidden,
        showed_hidden,
    );
    build.switch(
        "Show file extensions",
        None,
        library.show_extensions,
        showed_extensions,
    );
    build.switch(
        "Confirm before moving files",
        None,
        library.confirm_move,
        confirmed_move,
    );
    build.switch(
        "Always ask where to save",
        None,
        library.ask_where_to_save,
        asked_where_to_save,
    );

    // Template.
    build.pane("Template");
    let template = session.template().clone();
    build.head("Template", None);
    let mut leader: Option<gtk::CheckButton> = None;
    for (name, label) in TEMPLATES {
        let radio = gtk::CheckButton::builder().name("radio").build();
        if let Some(leader) = &leader {
            radio.set_group(Some(leader));
        } else {
            leader = Some(radio.clone());
        }
        radio.set_active(name == template.name);
        radio.connect_toggled(glib::clone!(
            #[strong]
            session,
            move |radio| {
                if radio.is_active() {
                    session.edit_settings(|settings| settings.template.name = name);
                }
            }
        ));
        build.mark(label, &radio);
    }
    build.head("Layout", None);
    build.switch(
        "Center headings",
        None,
        template.center_headings,
        centered_headings,
    );
    build.switch(
        "Number headings",
        None,
        template.number_headings,
        numbered_headings,
    );
    build.switch(
        "Indent paragraphs",
        None,
        template.indent_paragraphs,
        indented_paragraphs,
    );

    // Export.
    build.pane("Export");
    let export = session.settings().export.clone();
    let papers = export_papers(session, export.paper);
    build.setting("Paper", None, &papers);
    let margin = export_spin(
        session,
        export.margin,
        &quill_engine::settings::export_margins(),
        export_margin,
    );
    margin.set_width_chars(3);
    margin.set_max_width_chars(3);
    build.setting("Margin (mm)", None, &margin);
    let size = export_spin(
        session,
        export.text_size,
        &quill_engine::settings::export_text_sizes(),
        export_text_size,
    );
    size.set_width_chars(3);
    size.set_max_width_chars(3);
    build.setting("Text size (pt)", None, &size);
    build.switch("Title page", None, export.title_page, export_title_page);
    build.switch("Header", None, export.header, export_header);
    build.switch("Footer", None, export.footer, export_footer);

    // Writing tools.
    build.pane("Writing tools");
    build.head("Style check lists", Some(ENGLISH_ONLY));
    for (toggle, label, on) in style_rows(session) {
        if toggle == StyleToggle::Enabled {
            continue;
        }
        debug_assert!(matches!(
            toggle,
            StyleToggle::List(List::Fillers | List::Redundancies | List::Cliches)
        ));
        let check = gtk::CheckButton::builder().active(on).build();
        check.connect_toggled(glib::clone!(
            #[strong]
            session,
            move |check| {
                let on = check.is_active();
                session.edit_settings(|settings| toggle.set(&mut settings.style_check, on));
            }
        ));
        build.mark(label, &check);
    }
    build.head("Spell check", None);
    language(&mut build, spelling);

    // Advanced.
    build.pane("Advanced");
    let button = gtk::Button::builder().label("Edit settings.toml…").build();
    let launch = launcher();
    button.connect_clicked(glib::clone!(
        #[strong]
        session,
        move |_| edit(session.settings_path(), launch.as_ref())
    ));
    build.setting(
        "Shortcuts and palette",
        Some("Shortcut rebinds and the palette file are set in the file"),
        &button,
    );
    let unapplied = session.unapplied();
    if !unapplied.is_empty() {
        build.head("Not applied from settings.toml", None);
        let refused = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .css_classes(["settings-refused"])
            .build();
        for line in &unapplied {
            refused.append(
                &gtk::Label::builder()
                    .label(line)
                    .halign(gtk::Align::Start)
                    .xalign(0.0)
                    .wrap(true)
                    .wrap_mode(gtk::pango::WrapMode::WordChar)
                    .selectable(true)
                    .build(),
            );
        }
        build.body.append(&refused);
        build.jumps.push(Jump {
            pane: "Advanced",
            label: "Not applied from settings.toml",
            said: format!(
                "{} line{}",
                unapplied.len(),
                if unapplied.len() == 1 { "" } else { "s" }
            ),
            warn: true,
        });
    }

    // The sidebar: its head, the search field, the six panes.
    let side = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .width_request(168)
        .css_classes(["settings-side"])
        .build();
    side.append(
        &gtk::Label::builder()
            .label("SETTINGS")
            .halign(gtk::Align::Start)
            .css_classes(["settings-side-head"])
            .build(),
    );
    let search = gtk::SearchEntry::builder()
        .placeholder_text("Search")
        .width_chars(8)
        .max_width_chars(8)
        .build();
    side.append(&search);
    let nav = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::Browse)
        .css_classes(["settings-nav"])
        .build();
    for name in PANES {
        nav.append(
            &gtk::Label::builder()
                .label(name)
                .halign(gtk::Align::Start)
                .build(),
        );
    }
    side.append(&nav);

    let results = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::Browse)
        .css_classes(["settings-results"])
        .build();
    let outer = gtk::Stack::new();
    outer.add_named(&inner, Some("panes"));
    outer.add_named(
        &gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .child(&results)
            .build(),
        Some("results"),
    );

    let root = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    root.append(&side);
    root.append(&outer);
    window.set_child(Some(&root));

    let state = Rc::new(State {
        found: build.found,
        jumps: build.jumps,
        results,
        outer,
        inner,
        nav,
        search,
        shown: RefCell::new(Vec::new()),
    });
    wire(&window, &state);

    let at = std::env::var("QUILL_STUB_PANE")
        .ok()
        .and_then(|want| {
            PANES
                .iter()
                .position(|name| name.to_lowercase().starts_with(&want.to_lowercase()))
        })
        .unwrap_or(0);
    state.nav.select_row(
        state
            .nav
            .row_at_index(i32::try_from(at).unwrap_or(0))
            .as_ref(),
    );
    window.present();
    if let Ok(query) = std::env::var("QUILL_STUB_QUERY") {
        state.search.grab_focus();
        state.search.set_text(&query);
        state.search.set_position(-1);
    }
    if std::env::var_os("QUILL_STUB_POPUP").is_some() {
        glib::timeout_add_local_once(Duration::from_millis(400), move || {
            papers.activate();
        });
    }
}

/// The Spell check language row and the line under it, as `spell_group` had
/// them, less the switch #463 moved to the View menu.
fn language(build: &mut Build, spelling: Option<&Resolved>) {
    let session = Rc::clone(&build.session);
    let installed = quill_engine::spell::installed_languages();
    let current = Rc::new(RefCell::new(session.settings().spell_language.clone()));
    let (rows, selected) = language_rows(&installed, &current.borrow(), spelling);
    let words: Vec<&str> = rows.iter().map(String::as_str).collect();
    let language = gtk::DropDown::from_strings(&words);
    language.set_selected(selected);
    build.setting("Language", None, &language);

    let said = gtk::Label::builder()
        .halign(gtk::Align::Start)
        .xalign(0.0)
        .wrap(true)
        .css_classes(["settings-hint"])
        .build();
    let wanted = match spelling {
        Some(Resolved::Missing { wanted }) => Some(wanted.clone()),
        _ => None,
    };
    said.set_visible(wanted.is_some());
    said.set_label(&wanted.as_deref().map(no_dictionary).unwrap_or_default());
    build.body.append(&said);

    let rows = Rc::new(RefCell::new(rows));
    language.connect_selected_notify(move |language| {
        let Some(chosen) = language_at(&rows.borrow(), language.selected()) else {
            return;
        };
        current.replace(chosen.clone());
        session.edit_settings(|settings| chose_language(settings, chosen));
        if let Some(at) = served_rows(&mut rows.borrow_mut())
            && let Some(model) = language.model().and_downcast::<gtk::StringList>()
        {
            model.remove(at);
        }
        let wanted = unserved_now(session.spell(), &current.borrow(), &installed);
        said.set_visible(wanted.is_some());
        said.set_label(&wanted.as_deref().map(no_dictionary).unwrap_or_default());
    });
}

/// Puts every row the results hold back in its pane.
fn restore(state: &State) {
    for hit in state.shown.borrow_mut().drain(..) {
        if let Hit::Row(at) = hit {
            let found = &state.found[at];
            if let Some(line) = found.row.parent().and_downcast::<gtk::Box>() {
                line.remove(&found.row);
            }
            found.slot.append(&found.row);
            if let Some(hint) = &found.hint {
                hint.set_visible(true);
            }
        }
    }
    state.results.remove_all();
}

/// Swaps the panes for the rows matching `query`, or back.
fn filter(state: &State, query: &str) {
    restore(state);
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        state.outer.set_visible_child_name("panes");
        return;
    }
    let matches =
        |pane: &str, label: &str| format!("{pane} {label}").to_lowercase().contains(&query);
    let group = |pane: &str| {
        gtk::Label::builder()
            .label(pane)
            .width_request(92)
            .xalign(0.0)
            .css_classes(["settings-group"])
            .build()
    };
    let mut shown = Vec::new();
    for (at, found) in state.found.iter().enumerate() {
        if !matches(found.pane, &found.label) {
            continue;
        }
        let line = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        line.append(&group(found.pane));
        found.slot.remove(&found.row);
        line.append(&found.row);
        if let Some(hint) = &found.hint {
            hint.set_visible(false);
        }
        state.results.append(&line);
        shown.push(Hit::Row(at));
    }
    for (at, jump) in state.jumps.iter().enumerate() {
        if !matches(jump.pane, jump.label) {
            continue;
        }
        let line = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        line.append(&group(jump.pane));
        line.append(
            &gtk::Label::builder()
                .label(jump.label)
                .hexpand(true)
                .xalign(0.0)
                .build(),
        );
        let said = gtk::Label::builder()
            .label(format!("{} ›", jump.said))
            .css_classes(["settings-jump"])
            .build();
        if jump.warn {
            said.add_css_class("warn");
        }
        line.append(&said);
        state.results.append(&line);
        shown.push(Hit::Jump(at));
    }
    state.shown.replace(shown);
    state
        .results
        .select_row(state.results.row_at_index(0).as_ref());
    state.outer.set_visible_child_name("results");
}

/// What Enter, Space or a click on a result row does to the control in it.
fn operate(control: &gtk::Widget) {
    if let Some(switch) = control.downcast_ref::<gtk::Switch>() {
        switch.set_active(!switch.is_active());
    } else if let Some(check) = control.downcast_ref::<gtk::CheckButton>() {
        // A radio is only ever switched on.
        check.set_active(check.widget_name() == "radio" || !check.is_active());
    } else if let Some(button) = control.downcast_ref::<gtk::Button>() {
        button.emit_clicked();
    } else if control.is::<gtk::DropDown>() {
        control.activate();
    } else {
        // A spin button or a scale: the keys that move it are its own.
        control.grab_focus();
    }
}

fn wire(window: &gtk::Window, state: &Rc<State>) {
    state.nav.connect_row_selected(glib::clone!(
        #[strong]
        state,
        move |_, row| {
            let Some(row) = row else { return };
            let name = PANES[usize::try_from(row.index()).unwrap_or(0)];
            state.inner.set_visible_child_name(name);
        }
    ));
    // A click on a pane's name leaves the search; selection alone does not,
    // so the results can stand while the sidebar still says where you were.
    state.nav.connect_row_activated(glib::clone!(
        #[strong]
        state,
        move |_, _| state.search.set_text("")
    ));
    state.search.connect_search_changed(glib::clone!(
        #[strong]
        state,
        move |search| filter(&state, &search.text())
    ));
    state.search.connect_stop_search(glib::clone!(
        #[strong]
        state,
        move |search| {
            log("stop-search");
            search.set_text("");
            filter(&state, "");
        }
    ));
    state.results.connect_row_activated(glib::clone!(
        #[strong]
        state,
        move |_, row| {
            let hit = state
                .shown
                .borrow()
                .get(usize::try_from(row.index()).unwrap_or(usize::MAX))
                .copied();
            match hit {
                Some(Hit::Row(at)) => {
                    log(&format!("operate {}", state.found[at].label));
                    operate(&state.found[at].control);
                }
                Some(Hit::Jump(at)) => {
                    let pane = state.jumps[at].pane;
                    log(&format!("jump {pane}"));
                    state.search.set_text("");
                    filter(&state, "");
                    let index = PANES.iter().position(|name| *name == pane).unwrap_or(0);
                    state.nav.select_row(
                        state
                            .nav
                            .row_at_index(i32::try_from(index).unwrap_or(0))
                            .as_ref(),
                    );
                }
                None => {}
            }
        }
    ));
    // Typing anywhere in the window goes to the field: stock GtkSearchEntry.
    state.search.set_key_capture_widget(Some(window));

    if std::env::var_os("QUILL_STUB_LOG").is_some() {
        window.connect_focus_widget_notify(|window| {
            let said = gtk::prelude::RootExt::focus(window).map_or_else(
                || "none".to_owned(),
                |widget| {
                    let parent = widget
                        .parent()
                        .map(|parent| {
                            format!("{} {:?}", parent.type_().name(), parent.css_classes())
                        })
                        .unwrap_or_default();
                    let index = widget
                        .downcast_ref::<gtk::ListBoxRow>()
                        .map(|row| format!(" #{}", row.index()))
                        .unwrap_or_default();
                    format!("{}{index} in {parent}", widget.type_().name())
                },
            );
            eprintln!("stub: focus {said}");
        });
    }

    if std::env::var("QUILL_STUB_KEYS").as_deref() == Ok("stock") {
        return;
    }
    // The Palette's shape: the focus stays in the field, Down and Up move the
    // selection, Enter operates the selected row.
    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    keys.connect_key_pressed(glib::clone!(
        #[strong]
        state,
        move |_, key, _, _| {
            let step = match key {
                gdk::Key::Down => 1,
                gdk::Key::Up => -1,
                _ => return glib::Propagation::Proceed,
            };
            if state.shown.borrow().is_empty() {
                return glib::Propagation::Proceed;
            }
            let last = i32::try_from(state.shown.borrow().len()).unwrap_or(1) - 1;
            let now = state.results.selected_row().map_or(0, |row| row.index());
            let next = (now + step).clamp(0, last);
            state
                .results
                .select_row(state.results.row_at_index(next).as_ref());
            log(&format!("select {next}"));
            glib::Propagation::Stop
        }
    ));
    state.search.add_controller(keys);
    // Esc from anywhere in the window: GtkSearchEntry's `stop-search` fires
    // only while the field itself holds the keyboard, and its key capture
    // does not forward Esc. Clears the search first, closes the window second.
    let escape = gtk::EventControllerKey::new();
    escape.set_propagation_phase(gtk::PropagationPhase::Capture);
    escape.connect_key_pressed(glib::clone!(
        #[strong]
        state,
        #[weak]
        window,
        #[upgrade_or]
        glib::Propagation::Proceed,
        move |_, key, _, _| {
            if key != gdk::Key::Escape {
                return glib::Propagation::Proceed;
            }
            if state.search.text().is_empty() {
                log("escape: close");
                window.close();
            } else {
                log("escape: clear");
                state.search.set_text("");
                filter(&state, "");
                state.search.grab_focus();
            }
            glib::Propagation::Stop
        }
    ));
    window.add_controller(escape);
    state.search.connect_activate(glib::clone!(
        #[strong]
        state,
        move |_| {
            // Not `row.activate()`: that takes the keyboard out of the field
            // and on to the row, and the next letter typed rebuilds the list
            // under it.
            if let Some(row) = state.results.selected_row() {
                state.results.emit_by_name::<()>("row-activated", &[&row]);
            }
        }
    ));
}

struct Skin {
    pairs: [(&'static str, &'static str); 22],
}

const LIGHT: Skin = Skin {
    pairs: [
        ("@ink@", "#191919"),
        ("@dim@", "#8c8c8c"),
        ("@bg@", "#f7f7f7"),
        ("@rule@", "rgba(0,0,0,0.10)"),
        ("@sw_off@", "#d2d2d2"),
        ("@sw_on@", "#0a94d6"),
        ("@knob@", "#ffffff"),
        ("@knob_bd@", "rgba(0,0,0,0.08)"),
        ("@btn_bg@", "#fcfcfc"),
        ("@btn_bd@", "rgba(0,0,0,0.16)"),
        ("@field@", "#fcfcfc"),
        ("@accent@", "#0a94d6"),
        ("@check_bd@", "rgba(0,0,0,0.28)"),
        ("@list_bg@", "#fcfcfc"),
        ("@danger@", "#c4483c"),
        ("@track@", "#dcdcdc"),
        ("@side@", "#eaebeb"),
        ("@pill@", "#d8d9d9"),
        ("@pill_ink@", "#191919"),
        ("@menu@", "#f2f2f2"),
        ("@menu_bd@", "rgba(0,0,0,0.12)"),
        ("@hover@", "#f0f0f0"),
    ],
};

const DARK: Skin = Skin {
    pairs: [
        ("@ink@", "#cccccc"),
        ("@dim@", "#7e7e7e"),
        ("@bg@", "#1a1a1a"),
        ("@rule@", "rgba(255,255,255,0.10)"),
        ("@sw_off@", "#3d3d3d"),
        ("@sw_on@", "#0a84c8"),
        ("@knob@", "#e8e8e8"),
        ("@knob_bd@", "rgba(0,0,0,0.3)"),
        ("@btn_bg@", "#262626"),
        ("@btn_bd@", "rgba(255,255,255,0.15)"),
        ("@field@", "#222222"),
        ("@accent@", "#0a84c8"),
        ("@check_bd@", "rgba(255,255,255,0.30)"),
        ("@list_bg@", "#151515"),
        ("@danger@", "#cf807e"),
        ("@track@", "#383838"),
        ("@side@", "#141615"),
        ("@pill@", "#393b3a"),
        ("@pill_ink@", "#e0e0e0"),
        ("@menu@", "#2e2e2e"),
        ("@menu_bd@", "rgba(255,255,255,0.13)"),
        ("@hover@", "#2e2e2e"),
    ],
};

const TICK: &str = "resource:///org/gtk/libgtk/theme/Default/assets/check-symbolic.svg";
/// The dropdown's chevron as a symbolic SVG: a filled shape, because
/// `-gtk-recolor` fills every path and a stroked one comes out a wedge. It is
/// written to a file because `-gtk-recolor` loads through a `GFile`, which a
/// `data:` URL is not; a plain `url("data:…")` works but is rasterised at 1x
/// and scaled, so it is soft on a 2x output. The real thing is a gresource.
const CHEVRON: &str = "<svg xmlns='http://www.w3.org/2000/svg' width='16' height='16' viewBox='0 0 10 10'><path d='M1.55 3.55 L2.45 2.65 L5 5.2 L7.55 2.65 L8.45 3.55 L5 7 Z'/></svg>";

const SHEET: &str = r#"
window.settings { background-color: @bg@; color: @ink@; font-family: @font@; font-size: 13px; }
window.settings scrolledwindow, window.settings viewport, window.settings stack { background: none; }
window.settings .settings-side { background-color: @side@; border-right: 1px solid @rule@; padding-top: 13px; }
window.settings .settings-side-head, window.settings .settings-caps {
  font-size: 10.5px; font-weight: 600; letter-spacing: 0.63px; color: @dim@;
}
window.settings .settings-side-head { padding: 0 17px 8px; }

window.settings entry.search {
  min-height: 22px; margin: 0 8px 8px; padding: 0 6px; border: 1px solid @btn_bd@; border-radius: 5px;
  background: @field@; color: @ink@; box-shadow: none; outline: none; font-size: 12px;
}
window.settings entry.search:focus-within { border-color: @accent@; }
window.settings entry.search > image { -gtk-icon-size: 12px; color: @dim@; margin: 0 4px 0 0; }
window.settings entry.search > text > placeholder { color: @dim@; }

window.settings list.settings-nav { background: none; }
window.settings list.settings-nav > row {
  min-height: 28px; margin: 0 8px 1px; padding: 0 9px; border-radius: 5px; color: @ink@; background: none; outline: none;
}
window.settings list.settings-nav > row:hover { background-color: alpha(@pill@, 0.5); }
window.settings list.settings-nav > row:selected { background-color: @pill@; color: @pill_ink@; font-weight: 600; }
window.settings list.settings-nav > row:focus-visible { box-shadow: inset 0 0 0 1px @accent@; }

window.settings .settings-pane { padding: 18px 28px; }
window.settings .settings-head { margin-top: 14px; padding-bottom: 4px; min-height: 22px; }
window.settings .settings-head.first { margin-top: 0; }
window.settings .settings-hint { font-size: 11.5px; color: @dim@; }
window.settings .settings-row { min-height: 32px; }
window.settings .settings-row.tall { min-height: 46px; }
window.settings .settings-check { min-height: 28px; }

window.settings button {
  min-height: 22px; min-width: 0; padding: 0 10px; border: 1px solid @btn_bd@; border-radius: 5px;
  background: @btn_bg@; color: @ink@; box-shadow: none; text-shadow: none; outline: none; font-weight: normal;
}
window.settings button:hover { background: @hover@; }
window.settings button:focus-visible { border-color: @accent@; }
window.settings button.settings-small { min-height: 20px; }

window.settings dropdown > button > box { border-spacing: 8px; }
window.settings dropdown arrow {
  min-width: 10px; min-height: 10px; -gtk-icon-size: 10px; margin: 6px 0;
  -gtk-icon-source: -gtk-recolor(url("@chevron@"));
}
window.settings dropdown popover { font-family: @font@; font-size: 13px; }
window.settings dropdown popover > contents {
  background-color: @menu@; color: @ink@; border: 1px solid @menu_bd@; border-radius: 8px; padding: 4px 0;
  box-shadow: 0 1px 1px rgba(0,0,0,0.07), 0 8px 24px rgba(0,0,0,0.15);
}
window.settings dropdown popover listview { background: none; color: @ink@; margin: 0; padding: 0; }
window.settings dropdown popover scrolledwindow { margin: 0; padding: 0; }
window.settings dropdown popover listview > row {
  min-height: 24px; margin: 0 4px; padding: 0 8px; border-radius: 5px; outline: none; background: none;
}
window.settings dropdown popover listview > row:hover,
window.settings dropdown popover listview > row:focus-visible { background-color: @accent@; color: white; }
window.settings dropdown popover listview > row image { -gtk-icon-size: 10px; min-width: 10px; }

window.settings switch {
  min-width: 0; min-height: 0; padding: 0; margin: 0; border: none; border-radius: 9px;
  background: @sw_off@; box-shadow: none; outline: none;
}
window.settings switch:checked { background: @sw_on@; }
window.settings switch:focus-visible { outline: 2px solid alpha(@accent@, 0.45); outline-offset: 1px; }
window.settings switch > slider {
  min-width: 14px; min-height: 14px; margin: 2px; border: none; border-radius: 50%; background: @knob@;
  box-shadow: 0 0 0 1px @knob_bd@, 0 1px 2px rgba(0,0,0,0.2);
}
window.settings switch > image { color: transparent; -gtk-icon-size: 1px; min-width: 0; min-height: 0; }

window.settings spinbutton {
  min-height: 22px; padding: 0; border: 1px solid @btn_bd@; border-radius: 5px; background: @field@; color: @ink@;
  box-shadow: none; outline: none; font-feature-settings: "tnum";
}
window.settings spinbutton:focus-within { border-color: @accent@; }
window.settings spinbutton > text {
  min-width: 0; min-height: 0; padding: 0 0 0 8px; background: none; border: none; box-shadow: none; outline: none;
}
window.settings spinbutton > button {
  min-width: 26px; min-height: 22px; padding: 0; margin: 0; border: none; border-left: 1px solid @btn_bd@;
  border-radius: 0; background: none; -gtk-icon-size: 12px;
}
window.settings spinbutton > button:hover { background: @hover@; }
window.settings spinbutton > button:last-child { border-radius: 0 4px 4px 0; }

window.settings scale { padding: 0; min-height: 14px; outline: none; }
window.settings scale > value { color: @dim@; font-size: 12px; font-feature-settings: "tnum"; margin-right: 10px; }
window.settings scale > trough {
  min-height: 3px; margin: 0; border: none; border-radius: 3px; background: @track@; box-shadow: none; outline: none;
}
window.settings scale > trough > highlight { min-height: 3px; border: none; border-radius: 3px; background: @accent@; }
window.settings scale > trough > slider {
  min-width: 14px; min-height: 14px; margin: -6px -7px -5px -7px; border: none; border-radius: 50%; background: @knob@;
  box-shadow: 0 0 0 1px @knob_bd@, 0 1px 2px rgba(0,0,0,0.25);
}
window.settings scale:focus-visible > trough > slider { box-shadow: 0 0 0 2px alpha(@accent@, 0.6); }

window.settings checkbutton { padding: 0; outline: none; border-spacing: 0; }
window.settings checkbutton > check, window.settings checkbutton > radio {
  min-width: 12px; min-height: 12px; margin: 0 9px 0 0; padding: 0; border: 1px solid @check_bd@;
  background: @field@; box-shadow: none; color: white; -gtk-icon-size: 10px; -gtk-icon-source: none;
}
window.settings checkbutton > check { border-radius: 3px; }
window.settings checkbutton > radio { border-radius: 50%; }
window.settings checkbutton > check:checked {
  background: @accent@; border-color: @accent@; -gtk-icon-source: -gtk-recolor(url("@tick@"));
}
window.settings checkbutton > radio:checked {
  border-color: @accent@; background: radial-gradient(circle closest-side, white 38%, @accent@ 46%);
}
window.settings checkbutton:focus-visible > check, window.settings checkbutton:focus-visible > radio {
  outline: 2px solid alpha(@accent@, 0.45); outline-offset: 1px;
}

window.settings .settings-paths { border: 1px solid @btn_bd@; border-radius: 6px; background-color: @list_bg@; }
window.settings .settings-paths > box { min-height: 32px; padding: 0 3px 0 10px; }
window.settings .settings-paths > box:not(:first-child) { min-height: 31px; border-top: 1px solid @rule@; }

window.settings .settings-refused {
  padding: 6px 10px; border: 1px solid @btn_bd@; border-radius: 5px;
  font-family: "Quill Mono", monospace; font-size: 11px; color: @danger@;
}

window.settings list.settings-results { background: none; padding: 6px 0; }
window.settings list.settings-results > row {
  min-height: 34px; margin: 0 6px; padding: 0 10px; border-radius: 5px; outline: none; color: @ink@; background: none;
}
window.settings list.settings-results > row .settings-row,
window.settings list.settings-results > row .settings-row.tall,
window.settings list.settings-results > row .settings-check { min-height: 0; }
window.settings list.settings-results > row:selected { background-color: @accent@; color: white; }
window.settings list.settings-results > row:focus-visible { box-shadow: inset 0 0 0 2px alpha(@ink@, 0.55); }
window.settings list.settings-results > row:selected .settings-group,
window.settings list.settings-results > row:selected .settings-jump { color: rgba(255,255,255,0.8); }
window.settings list.settings-results > row:selected switch { background: rgba(255,255,255,0.35); }
window.settings list.settings-results > row:selected checkbutton > check:not(:checked),
window.settings list.settings-results > row:selected checkbutton > radio:not(:checked) { border-color: rgba(255,255,255,0.8); }
window.settings .settings-group { font-size: 11.5px; color: @dim@; }
window.settings .settings-jump { font-size: 12px; color: @dim@; }
window.settings .settings-jump.warn { color: @danger@; }
"#;

/// Where the chevron was written, as a `file://` URI.
fn chevron() -> String {
    let path = std::env::temp_dir().join("quill-stub-chevron-symbolic.svg");
    std::fs::write(&path, CHEVRON).ok();
    glib::filename_to_uri(&path, None).map_or_else(|_| String::new(), |uri| uri.to_string())
}

/// The window's stylesheet for `scheme`, loaded with the chrome's so a ground
/// change reloads it.
pub(super) fn stylesheet(scheme: Scheme) -> String {
    let skin = match scheme {
        Scheme::Light => LIGHT,
        Scheme::Dark => DARK,
    };
    let mut sheet = SHEET
        .replace("@font@", CHROME_FONT)
        .replace("@tick@", TICK)
        .replace("@chevron@", &chevron());
    for (name, value) in skin.pairs {
        sheet = sheet.replace(name, value);
    }
    sheet
}
