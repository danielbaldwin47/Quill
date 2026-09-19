//! The Settings window's search: the sidebar's field finds a setting and takes
//! the writer to it, and never operates one (#474).

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gdk, glib};

use quill_engine::palette::{self, Setting};

/// How long a jumped-to row's highlight takes to fade once it has held for
/// [`HOLD_MS`]: the sheet's `transition-duration`, which GTK runs at once
/// where `gtk-enable-animations` is off.
pub(super) const FADE_MS: u32 = 800;
/// How long the highlight holds at full strength before it fades; with the
/// fade, about a second.
const HOLD_MS: u64 = 200;
/// How far past a jumped-to row's edge its pane scrolls, so the highlight's
/// spread is in view with the row.
const SCROLL_PAST: f64 = 12.0;
/// The frames a jump waits for a newly shown pane to be laid out before it
/// highlights the row where it is.
const LAYOUT_FRAMES: u32 = 10;

/// The view's two children: the panes, and the results in their place.
const PANES: &str = "panes";
const RESULTS: &str = "results";

/// A setting a pane shows: its row of the table, the widget it is drawn as,
/// and the scroller of the pane it is on. Rows are compared by value, never
/// by address: the table is a `const`, so each use of it may be a copy.
pub(super) struct Place {
    pub(super) setting: &'static Setting,
    pub(super) block: gtk::Widget,
    pub(super) scroller: gtk::ScrolledWindow,
}

/// What Esc does in the window: clears a field holding text, and closes the
/// window once it holds none.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Esc {
    Clear,
    Close,
}

/// What Esc does with `text` in the field.
fn escape(text: &str) -> Esc {
    if text.is_empty() {
        Esc::Close
    } else {
        Esc::Clear
    }
}

/// Whether `text` in the field replaces the pane with results: the listing
/// finds nothing for a blank query, so blank shows the pane.
fn searching(text: &str) -> bool {
    !text.trim().is_empty()
}

/// The results for `query`: every row the listing matches, the rows a Command
/// also sets among them, that `shown` says a pane draws — the refused lines
/// are there only while a line is refused.
fn results(query: &str, shown: impl Fn(&Setting) -> bool) -> Vec<&'static Setting> {
    palette::settings(query)
        .into_iter()
        .map(|found| found.setting)
        .filter(|setting| shown(setting))
        .collect()
}

/// The selection `by` rows on from `at` among `count` results, held at either
/// end rather than wrapping; the first row when none was selected, and none
/// when there are no results.
fn stepped(at: Option<usize>, by: isize, count: usize) -> Option<usize> {
    let last = count.checked_sub(1)?;
    Some(at.map_or(0, |at| at.saturating_add_signed(by).min(last)))
}

/// The search's widgets and what it has found.
struct Search {
    entry: gtk::Entry,
    view: gtk::Stack,
    list: gtk::ListBox,
    scroller: gtk::ScrolledWindow,
    nav: gtk::ListBox,
    places: Vec<Place>,
    found: RefCell<Vec<&'static Setting>>,
    selected: Cell<Option<usize>>,
}

/// Wires the sidebar's `entry` over `panes` in `window`, `nav` being the
/// sidebar's list of panes and `places` every setting the panes draw; returns
/// the view that shows the panes or, while the field holds a query, the
/// results in their place.
///
/// Keys are the Palette's, which stock focus fights (the stub's README):
/// typing anywhere reaches the field, Down and Up move the selection while
/// focus stays in it, Enter jumps, and Esc clears a field holding text and
/// closes the window second.
pub(super) fn wire(
    window: &gtk::Window,
    entry: &gtk::Entry,
    panes: &gtk::Stack,
    nav: &gtk::ListBox,
    places: Vec<Place>,
) -> gtk::Stack {
    let list = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::Single)
        .can_focus(false)
        .css_classes(["settings-results"])
        .build();
    let scroller = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&list)
        .build();
    let view = gtk::Stack::builder().hexpand(true).vexpand(true).build();
    view.add_named(panes, Some(PANES));
    view.add_named(&scroller, Some(RESULTS));
    view.set_visible_child_name(PANES);

    let search = Rc::new(Search {
        entry: entry.clone(),
        view: view.clone(),
        list,
        scroller,
        nav: nav.clone(),
        places,
        found: RefCell::new(Vec::new()),
        selected: Cell::new(None),
    });
    let held = Rc::downgrade(&search);

    entry.connect_changed(glib::clone!(
        #[strong]
        held,
        move |entry| {
            if let Some(search) = held.upgrade() {
                search.render(&entry.text());
            }
        }
    ));
    entry.connect_activate(glib::clone!(
        #[strong]
        held,
        move |_| {
            if let Some(search) = held.upgrade() {
                search.jump_selected();
            }
        }
    ));
    let moves = gtk::EventControllerKey::new();
    moves.set_propagation_phase(gtk::PropagationPhase::Capture);
    moves.connect_key_pressed(glib::clone!(
        #[strong]
        held,
        move |_, key, _, _| {
            let by = match key {
                gdk::Key::Down => 1,
                gdk::Key::Up => -1,
                _ => return glib::Propagation::Proceed,
            };
            match held.upgrade() {
                Some(search) if !search.found.borrow().is_empty() => {
                    search.step(by);
                    glib::Propagation::Stop
                }
                _ => glib::Propagation::Proceed,
            }
        }
    ));
    entry.add_controller(moves);
    search.list.connect_row_activated(glib::clone!(
        #[strong]
        held,
        move |_, row| {
            if let (Some(search), Ok(at)) = (held.upgrade(), usize::try_from(row.index())) {
                search.jump(at);
            }
        }
    ));

    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    keys.connect_key_pressed(glib::clone!(
        #[strong]
        held,
        move |keys, key, _, state| match held.upgrade() {
            Some(search) => search.window_key(keys, key, state),
            None => glib::Propagation::Proceed,
        }
    ));
    window.add_controller(keys);
    // The window's handlers hold the search weakly; the window holds it for
    // as long as it is open, and dropping it with the window frees its rows.
    window.connect_destroy(move |_| {
        let _ = &search;
    });
    view
}

impl Search {
    /// Shows the results for `text` in the pane's place, the first selected,
    /// or the pane again when `text` is blank.
    fn render(&self, text: &str) {
        self.list.remove_all();
        self.selected.set(None);
        if !searching(text) {
            self.found.borrow_mut().clear();
            self.view.set_visible_child_name(PANES);
            return;
        }
        let found = results(text, |setting| {
            self.places
                .iter()
                .any(|place| place.setting == setting && place.block.is_visible())
        });
        for setting in &found {
            self.list.append(&result_row(setting));
        }
        let count = found.len();
        *self.found.borrow_mut() = found;
        self.view.set_visible_child_name(RESULTS);
        self.select(stepped(None, 0, count));
    }

    /// Moves the selection `by` rows, held at either end.
    fn step(&self, by: isize) {
        self.select(stepped(self.selected.get(), by, self.found.borrow().len()));
    }

    /// Selects result `at` and scrolls it into view.
    fn select(&self, at: Option<usize>) {
        self.selected.set(at);
        let row = at
            .and_then(|at| i32::try_from(at).ok())
            .and_then(|at| self.list.row_at_index(at));
        self.list.select_row(row.as_ref());
        if let Some(row) = row {
            scroll_to(&self.scroller, &row, 0.0);
        }
    }

    /// Jumps to the selected result, if there is one.
    fn jump_selected(&self) {
        if let Some(at) = self.selected.get() {
            self.jump(at);
        }
    }

    /// Takes the writer to result `at`: clears the field, shows its pane,
    /// scrolls its row into view and highlights it.
    fn jump(&self, at: usize) {
        let Some(setting) = self.found.borrow().get(at).copied() else {
            return;
        };
        let Some(place) = self.places.iter().find(|place| place.setting == setting) else {
            return;
        };
        self.entry.set_text("");
        super::select(&self.nav, setting.pane);
        light(&place.scroller, &place.block);
    }

    /// The window's own keys, ahead of every widget in it: Esc, and a letter
    /// typed with the field unfocused, which goes to the field.
    fn window_key(
        &self,
        keys: &gtk::EventControllerKey,
        key: gdk::Key,
        state: gdk::ModifierType,
    ) -> glib::Propagation {
        let Some(window) = keys.widget().and_downcast::<gtk::Window>() else {
            return glib::Propagation::Proceed;
        };
        // A dropdown's popup is a surface of its own, whose Esc closes it and
        // whose keys are its list's.
        let own = keys.current_event().and_then(|event| event.surface()) == window.surface();
        if !own {
            return glib::Propagation::Proceed;
        }
        if key == gdk::Key::Escape {
            match escape(&self.entry.text()) {
                Esc::Clear => self.entry.set_text(""),
                Esc::Close => window.close(),
            }
            return glib::Propagation::Stop;
        }
        // A field or a spin button's figures take their own typing.
        let editing = gtk::prelude::GtkWindowExt::focus(&window)
            .is_some_and(|focus| focus.is::<gtk::Text>() || focus.is::<gtk::Entry>());
        let chorded = state.intersects(
            gdk::ModifierType::CONTROL_MASK
                | gdk::ModifierType::ALT_MASK
                | gdk::ModifierType::SUPER_MASK,
        );
        let Some(letter) = key.to_unicode().filter(|letter| !letter.is_control()) else {
            return glib::Propagation::Proceed;
        };
        // Space on an empty field is the focused switch's or check's.
        if editing || chorded || (letter == ' ' && self.entry.text().is_empty()) {
            return glib::Propagation::Proceed;
        }
        self.entry.grab_focus_without_selecting();
        let mut at = i32::try_from(self.entry.text().chars().count()).unwrap_or(-1);
        self.entry
            .insert_text(letter.encode_utf8(&mut [0; 4]), &mut at);
        self.entry.set_position(at);
        glib::Propagation::Stop
    }
}

/// A result: the setting's label with its pane's name dim beside it.
fn result_row(setting: &Setting) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    row.append(
        &gtk::Label::builder()
            .label(setting.label)
            .halign(gtk::Align::Start)
            .build(),
    );
    row.append(
        &gtk::Label::builder()
            .label(setting.pane.name())
            .halign(gtk::Align::Start)
            .css_classes(["settings-group"])
            .build(),
    );
    row
}

/// Scrolls `scroller` so `widget`, somewhere inside it, is in view with
/// `past` to spare beyond whichever edge it was out past; `false` while
/// `widget` is not laid out yet.
fn scroll_to(scroller: &gtk::ScrolledWindow, widget: &impl IsA<gtk::Widget>, past: f64) -> bool {
    let Some(inside) = scroller.child() else {
        return false;
    };
    let Some(bounds) = widget.compute_bounds(&inside) else {
        return false;
    };
    if bounds.height() <= 0.0 {
        return false;
    }
    let adjustment = scroller.vadjustment();
    let (top, bottom) = (
        f64::from(bounds.y()),
        f64::from(bounds.y() + bounds.height()),
    );
    if bottom > adjustment.upper() {
        return false;
    }
    if top < adjustment.value() {
        adjustment.set_value(top - past);
    } else if bottom > adjustment.value() + adjustment.page_size() {
        adjustment.set_value(bottom + past - adjustment.page_size());
    }
    true
}

/// Scrolls `block` into view in `scroller` once its pane is laid out, and
/// lights it: the highlight holds, then fades by the sheet's transition.
fn light(scroller: &gtk::ScrolledWindow, block: &gtk::Widget) {
    let waited = Cell::new(0);
    block.add_tick_callback(glib::clone!(
        #[strong]
        scroller,
        move |block, _| {
            waited.set(waited.get() + 1);
            if !scroll_to(&scroller, block, SCROLL_PAST) && waited.get() < LAYOUT_FRAMES {
                return glib::ControlFlow::Continue;
            }
            block.add_css_class("settings-lit");
            block.add_css_class("settings-hit");
            glib::timeout_add_local_once(
                std::time::Duration::from_millis(HOLD_MS),
                glib::clone!(
                    #[weak]
                    block,
                    move || block.remove_css_class("settings-hit")
                ),
            );
            glib::ControlFlow::Break
        }
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(query: &str) -> Vec<(&'static str, &'static str)> {
        results(query, |_| true)
            .into_iter()
            .map(|setting| (setting.label, setting.pane.name()))
            .collect()
    }

    #[test]
    fn marg_finds_the_margin_on_the_export_pane_alone() {
        assert_eq!(words("marg"), [("Margin (mm)", "Export")]);
    }

    #[test]
    fn hide_finds_hide_bars_though_a_command_sets_it() {
        assert!(words("hide").contains(&("Hide Bars", "General")));
    }

    #[test]
    fn a_blank_field_shows_the_pane_and_lists_nothing() {
        for blank in ["", " ", "\t"] {
            assert!(!searching(blank));
            assert!(words(blank).is_empty());
        }
        assert!(searching("marg"));
    }

    #[test]
    fn a_row_no_pane_draws_is_no_result() {
        let refused = "Not applied from settings.toml";
        assert!(words("applied").iter().any(|(label, _)| *label == refused));
        assert!(
            results("applied", |setting| setting.label != refused)
                .iter()
                .all(|setting| setting.label != refused)
        );
    }

    #[test]
    fn esc_clears_a_field_holding_text_and_closes_an_empty_one() {
        assert_eq!(escape("marg"), Esc::Clear);
        assert_eq!(escape(" "), Esc::Clear);
        assert_eq!(escape(""), Esc::Close);
    }

    #[test]
    fn down_and_up_hold_at_either_end() {
        assert_eq!(stepped(None, 0, 3), Some(0));
        assert_eq!(stepped(Some(0), 1, 3), Some(1));
        assert_eq!(stepped(Some(2), 1, 3), Some(2));
        assert_eq!(stepped(Some(0), -1, 3), Some(0));
        assert_eq!(stepped(Some(2), -1, 3), Some(1));
        assert_eq!(stepped(Some(1), 0, 0), None);
        assert_eq!(stepped(None, 1, 0), None);
    }
}
