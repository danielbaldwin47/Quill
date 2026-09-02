//! The Palette: every Command in one popover, narrowing as the writer types
//! (#122).
//!
//! `Ctrl+K` and `Ctrl+Shift+P` open it, View › Window "All Commands…" runs
//! the same action, and `--menu palette` has it up before the first frame.
//! What it lists is [`quill_engine::palette`]: the oracle's four sections and
//! More with nothing typed, one ranked list once something is. Arrows move
//! the selection, Enter runs it and closes, Esc closes. Its look is the
//! Parity oracle's palette rules (`legacy/app/css/chrome.css`), as constants
//! beside the menus'.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{cairo, gdk, glib};
use quill_engine::commands::Command;
use quill_engine::palette::{self as engine, Row};
use quill_engine::theme::Scheme;

use crate::chrome::{self, CHROME_FONT, Modes};
use crate::menus;

/// How far down the window the panel's top sits (`.palette { top: 13vh }`).
const TOP: f64 = 0.13;
/// The panel's width (`width: min(460px, 100vw - 48px)`).
const WIDTH: i32 = 460;
/// What the panel keeps clear of the window's edges, together, when the
/// window is narrower than [`WIDTH`] plus this.
const KEEP_CLEAR: i32 = 48;
/// The panel's corner (`border-radius: 10px`).
const RADIUS: i32 = 10;

/// The search field's measure (`.palette-field { height: 42px; padding: 0
/// 13px; gap: 8px }`).
struct Field {
    height: i32,
    /// Its side padding.
    pad: i32,
    /// Between the magnifier and the entry.
    gap: i32,
}

const FIELD: Field = Field {
    height: 42,
    pad: 13,
    gap: 8,
};

/// The magnifier's side (`MAG`, thirteen by thirteen).
const MAG: i32 = 13;
/// The entry's type (`font: 15px/1`, `letter-spacing: -0.005em`).
const ENTRY_PX: f64 = 15.0;

/// The list's measure (`padding: 5px 0 6px; max-height: min(46vh, 320px)`).
struct List {
    /// Above its first row.
    top: i32,
    /// Below its last.
    bottom: i32,
    /// The most of it that shows before it scrolls.
    max: i32,
}

const LIST: List = List {
    top: 5,
    bottom: 6,
    max: 320,
};

/// A row's measure (`height: 24px; margin: 0 5px; padding: 0 9px;
/// border-radius: 5px; font-size: 12.5px`).
struct RowRule {
    height: i32,
    /// Its side margin.
    margin: i32,
    /// Its side padding.
    pad: i32,
    /// Its corner.
    radius: i32,
    /// Its type.
    px: f64,
}

const ROW: RowRule = RowRule {
    height: 24,
    margin: 5,
    pad: 9,
    radius: 5,
    px: 12.5,
};

/// A section heading: `padding: 9px 14px 3px` (the first `3px` above), the
/// menus' type ([`chrome::Head`]).
const HEAD: chrome::Head = chrome::Head {
    top: 9,
    x: 14,
    bottom: 3,
    px: 10.0,
};
/// A heading's line in pixels. A browser's normal line box for 10 px of
/// the face is 11.5, and GTK rounds a row's height up and the line to a
/// whole pixel, so the line is set a pixel and a half short to land the
/// first row where the oracle's shot has it.
const HEAD_LINE: f64 = 10.0;
/// The line shown when nothing matches (`.palette-empty { padding: 12px
/// 14px 14px }`).
struct Empty {
    top: i32,
    x: i32,
    bottom: i32,
}

const EMPTY: Empty = Empty {
    top: 12,
    x: 14,
    bottom: 14,
};

/// `length` as whole pixels: the panel's top down the window, a heading's
/// line in Pango units, the pointer's row. Rounded here and only here, in
/// the shape of `quill::tags::pixels`.
fn pixels(length: f64) -> i32 {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a window's height, a heading's line and a pointer's y are a few thousand at most"
    )]
    let whole = length.round() as i32;
    whole
}

/// The Palette's stylesheet, appended to the menus'.
///
/// The popover's `contents` is the panel; the field's entry is flattened to
/// the oracle's bare input; each `row` of the list is one of the oracle's
/// `li`, selected or a heading.
pub fn stylesheet(scheme: Scheme) -> String {
    let chrome::MenuInk {
        ground,
        border,
        ink,
        dim,
        selected,
        shadow,
    } = chrome::menu_ink(scheme);
    // The menus' shadow with its wide blur drawn in three quarters: GTK
    // spreads a 24 px blur 27 px past the panel's edge, over the judged
    // caret 22 px beside it (`--caret 403`), and the Gate reads a lit caret
    // by its exact accent (`tools/harness.mjs`, `carriesAccent`), which one
    // part in 255 of shadow takes away. At 18 px the tail is paper before
    // the caret and the shadow reads the same to the eye.
    let shadow = shadow.replace("24px", "18px").replace("30px", "22px");
    let tracking = chrome::tracking(ENTRY_PX);
    let RowRule {
        height: row_height,
        margin: row_margin,
        pad: row_pad,
        radius: row_radius,
        px: row_px,
    } = ROW;
    let keys = chrome::keys_declarations(dim);
    let chrome::Head {
        top: head_top,
        x: head_x,
        bottom: head_bottom,
        ..
    } = HEAD;
    let head = HEAD.type_declarations(dim);
    let Empty {
        top: empty_top,
        x: empty_x,
        bottom: empty_bottom,
    } = EMPTY;
    format!(
        "popover.chrome-palette {{ font-family: {CHROME_FONT}; font-size: {row_px}px; }}\n\
         popover.chrome-palette > contents {{\n\
         \x20 background-color: {ground}; color: {ink};\n\
         \x20 border: 1px solid {border}; border-radius: {RADIUS}px;\n\
         \x20 box-shadow: {shadow}; padding: 0;\n\
         }}\n\
         popover.chrome-palette entry {{\n\
         \x20 background: none; border: none; box-shadow: none; outline: none;\n\
         \x20 padding: 0; margin: 0; min-height: 0; min-width: 0;\n\
         \x20 font-size: {ENTRY_PX}px; letter-spacing: {tracking}px;\n\
         \x20 color: {ink}; caret-color: {ink};\n\
         }}\n\
         popover.chrome-palette entry text > placeholder {{ color: {dim}; }}\n\
         popover.chrome-palette .palette-mag, popover.chrome-palette .palette-rule {{ color: {dim}; }}\n\
         popover.chrome-palette .palette-rule {{ color: {border}; }}\n\
         popover.chrome-palette scrolledwindow, popover.chrome-palette list {{ background: none; }}\n\
         popover.chrome-palette list > row {{\n\
         \x20 min-height: {row_height}px; margin: 0 {row_margin}px; padding: 0 {row_pad}px;\n\
         \x20 border-radius: {row_radius}px; color: {ink}; background: none;\n\
         }}\n\
         popover.chrome-palette list > row:hover {{ background: none; }}\n\
         popover.chrome-palette list > row.dim label {{ color: {dim}; }}\n\
         popover.chrome-palette list > row:selected, popover.chrome-palette list > row:selected:hover {{\n\
         \x20 background-color: {selected};\n\
         }}\n\
         popover.chrome-palette list > row:selected label {{ color: white; }}\n\
         popover.chrome-palette list > row:selected label.palette-keys {{ color: rgba(255, 255, 255, 0.82); }}\n\
         popover.chrome-palette label.palette-keys {{ {keys} }}\n\
         popover.chrome-palette list > row.palette-head {{\n\
         \x20 min-height: 0; margin: 0; padding: {head_top}px {head_x}px {head_bottom}px; border-radius: 0;\n\
         }}\n\
         popover.chrome-palette list > row.palette-head:first-child {{ padding-top: {head_bottom}px; }}\n\
         popover.chrome-palette list > row.palette-head label {{ {head} }}\n\
         popover.chrome-palette list > row.palette-empty {{\n\
         \x20 min-height: 0; margin: 0; padding: {empty_top}px {empty_x}px {empty_bottom}px; border-radius: 0; color: {dim};\n\
         }}\n"
    )
}

/// The Palette popover, parented on its window.
#[derive(Clone)]
pub struct Palette {
    popover: gtk::Popover,
    entry: gtk::Entry,
    list: gtk::ListBox,
    scroller: gtk::ScrolledWindow,
    /// The rows that can be selected, top to bottom, each with its Command.
    rows: Rc<RefCell<Vec<(gtk::ListBoxRow, &'static Command)>>>,
    selected: Rc<Cell<usize>>,
    /// The modes the rows' titles read, as of the last opening.
    modes: Rc<Cell<Modes>>,
}

impl Palette {
    /// Builds the Palette under `window`, closed.
    pub fn new(window: &impl IsA<gtk::Widget>) -> Self {
        let Field {
            height: field_height,
            pad: field_pad,
            gap: field_gap,
        } = FIELD;
        let mag = chrome::icon(MAG, MAG, magnifier_icon);
        mag.add_css_class("palette-mag");
        // A plain entry rather than GTK's search entry, whose own magnifier
        // and clear button sit inside the field where the oracle has only
        // the text; the magnifier stands beside it here.
        let entry = gtk::Entry::builder()
            .placeholder_text("Search commands")
            .hexpand(true)
            .valign(gtk::Align::Center)
            .build();
        let field = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(field_gap)
            // The oracle's 42 includes the half-pixel rule under it; the
            // hairline below takes the last pixel of it here.
            .height_request(field_height - 1)
            .margin_start(field_pad)
            .margin_end(field_pad)
            .build();
        field.append(&mag);
        field.append(&entry);

        let List {
            top: list_top,
            bottom: list_bottom,
            max: list_max,
        } = LIST;
        let list = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::Single)
            .activate_on_single_click(true)
            .can_focus(false)
            .margin_top(list_top)
            .margin_bottom(list_bottom)
            .build();
        let scroller = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .propagate_natural_height(true)
            .max_content_height(list_max)
            .child(&list)
            .build();

        let panel = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            // Inside the panel's one-pixel border on either side.
            .width_request(WIDTH - 2)
            .build();
        panel.append(&field);
        panel.append(&chrome::hairline("palette-rule", gtk::Align::Start, true));
        panel.append(&scroller);

        let popover = gtk::Popover::builder()
            .css_classes(["chrome-palette"])
            .has_arrow(false)
            .position(gtk::PositionType::Bottom)
            .child(&panel)
            .build();
        popover.set_parent(window);

        let palette = Self {
            popover,
            entry,
            list,
            scroller,
            rows: Rc::new(RefCell::new(Vec::new())),
            selected: Rc::new(Cell::new(0)),
            modes: Rc::new(Cell::new(Modes::default())),
        };
        palette.wire();
        palette
    }

    /// The entry's text narrows the list; the keys move, run and close.
    fn wire(&self) {
        let typed = self.clone();
        self.entry.connect_changed(move |entry| {
            typed.render(&entry.text());
        });
        let entered = self.clone();
        self.entry.connect_activate(move |_| entered.run_selected());
        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        let pressed = self.clone();
        keys.connect_key_pressed(move |_, key, _, _| match key {
            gdk::Key::Down => {
                pressed.step(1);
                glib::Propagation::Stop
            }
            gdk::Key::Up => {
                pressed.step(-1);
                glib::Propagation::Stop
            }
            gdk::Key::Escape => {
                pressed.close();
                glib::Propagation::Stop
            }
            _ => glib::Propagation::Proceed,
        });
        self.entry.add_controller(keys);

        let clicked = self.clone();
        self.list.connect_row_activated(move |_, row| {
            let at = clicked
                .rows
                .borrow()
                .iter()
                .position(|(candidate, _)| candidate == row);
            if let Some(at) = at {
                clicked.selected.set(at);
                clicked.run_selected();
            }
        });
        // The pointer selects the row under it, as the oracle's does.
        let motion = gtk::EventControllerMotion::new();
        let hovered = self.clone();
        motion.connect_motion(move |_, _, y| {
            let Some(row) = hovered.list.row_at_y(pixels(y)) else {
                return;
            };
            let at = hovered
                .rows
                .borrow()
                .iter()
                .position(|(candidate, _)| *candidate == row);
            if let Some(at) = at
                && at != hovered.selected.get()
            {
                hovered.select(at);
            }
        });
        self.list.add_controller(motion);
        let closed = self.clone();
        self.popover
            .connect_closed(move |_| closed.entry.set_text(""));
    }

    /// Opens the Palette over `window`'s page with the map at rest and its
    /// first row selected, or closes it if it is up: `palette.open`.
    ///
    /// The entry takes the keyboard when the popover holds a grab, which is
    /// how a writer opens it; under `--deterministic` it holds none and the
    /// keyboard stays on the page, so the caret behind the panel is lit in
    /// the shot as the oracle's is.
    pub fn toggle(&self, window: &gtk::Window, modes: Modes) {
        if self.popover.is_visible() {
            self.close();
            return;
        }
        self.modes.set(modes);
        let top = pixels(f64::from(window.height()) * TOP);
        let width = WIDTH.min(window.width() - KEEP_CLEAR);
        if let Some(panel) = self.popover.child().and_downcast::<gtk::Box>() {
            panel.set_width_request(width - 2);
        }
        self.popover
            .set_pointing_to(Some(&gdk::Rectangle::new(0, top, window.width(), 0)));
        self.entry.set_text("");
        self.render("");
        // GTK moves the keyboard into a popover as it shows, grab or no
        // grab, and a keyboard inside a popup surface leaves the page's
        // caret a ghost. Without a grab nothing in the panel can take it, so
        // it stays on the page and the caret there stays a bar.
        let grabbing = self.popover.is_autohide();
        self.entry.set_can_focus(grabbing);
        self.entry.set_focusable(grabbing);
        self.popover.popup();
        if grabbing {
            self.entry.grab_focus();
        }
    }

    /// Closes the Palette; nothing if it is not up.
    pub fn close(&self) {
        if self.popover.is_visible() {
            self.popover.popdown();
        }
    }

    /// Whether the Palette is up.
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.popover.is_visible()
    }

    /// Whether the Palette takes the keyboard and closes on a click outside
    /// it; `false` under `--deterministic`, for the reason
    /// [`chrome::Bars::set_menus_grabbing`] gives.
    pub fn set_grabbing(&self, grabbing: bool) {
        self.popover.set_autohide(grabbing);
    }

    /// Takes the popover off the window, for the window's disposal.
    pub fn dispose(&self) {
        self.popover.unparent();
    }

    /// Fills the list for `query` and selects its first row.
    fn render(&self, query: &str) {
        while let Some(child) = self.list.first_child() {
            self.list.remove(&child);
        }
        let modes = self.modes.get();
        let mut rows = Vec::new();
        for (head, group) in engine::list(query) {
            if let Some(head) = head {
                self.list.append(&heading(head));
            }
            for row in group {
                let widget = item(&row, &modes);
                self.list.append(&widget);
                rows.push((widget, row.command));
            }
        }
        if rows.is_empty() {
            self.list.append(&nothing(query.trim()));
        }
        *self.rows.borrow_mut() = rows;
        self.select(0);
    }

    /// Moves the selection `by` rows, wrapping at either end.
    fn step(&self, by: i32) {
        let count = self.rows.borrow().len();
        if count == 0 {
            return;
        }
        let at = self.selected.get() as i32 + by;
        self.select(at.rem_euclid(count as i32) as usize);
    }

    /// Selects row `at` and scrolls it into view.
    fn select(&self, at: usize) {
        let rows = self.rows.borrow();
        let Some((row, _)) = rows.get(at) else {
            self.list.unselect_all();
            return;
        };
        self.selected.set(at);
        self.list.select_row(Some(row));
        if let Some(bounds) = row.compute_bounds(&self.list) {
            let adjustment = self.scroller.vadjustment();
            let (top, bottom) = (
                f64::from(bounds.y()),
                f64::from(bounds.y() + bounds.height()),
            );
            if top < adjustment.value() {
                adjustment.set_value(top);
            } else if bottom > adjustment.value() + adjustment.page_size() {
                adjustment.set_value(bottom - adjustment.page_size());
            }
        }
    }

    /// Runs the selected row's Command and closes; a row not built does
    /// nothing at all, as its chord does nothing, and the Palette stays up.
    fn run_selected(&self) {
        let Some(command) = self.selected_command() else {
            return;
        };
        if !command.built {
            return;
        }
        self.close();
        let (action, target) = command.action_and_target();
        let _ = self
            .popover
            .activate_action(&action, target.map(ToVariant::to_variant).as_ref());
    }

    /// The Command of the selected row, if a row is selected.
    fn selected_command(&self) -> Option<&'static Command> {
        self.rows
            .borrow()
            .get(self.selected.get())
            .map(|(_, command)| *command)
    }
}

/// A section's heading, which is neither selected nor run.
fn heading(text: &str) -> gtk::ListBoxRow {
    let label = gtk::Label::builder()
        .label(text.to_uppercase())
        .xalign(0.0)
        .build();
    // A browser's line box for 10 px type is 11.5 px; Pango's is the face's
    // ascent plus descent, 14, which would put every row below 2.5 px lower
    // than the oracle's.
    let attributes = gtk::pango::AttrList::new();
    let units = pixels(HEAD_LINE * f64::from(gtk::pango::SCALE));
    attributes.insert(gtk::pango::AttrInt::new_line_height_absolute(units));
    label.set_attributes(Some(&attributes));
    gtk::ListBoxRow::builder()
        .css_classes(["palette-head"])
        .selectable(false)
        .activatable(false)
        .can_focus(false)
        .child(&label)
        .build()
}

/// The line under an empty list.
fn nothing(query: &str) -> gtk::ListBoxRow {
    let label = gtk::Label::builder()
        .label(format!("Nothing matches “{query}”"))
        .xalign(0.0)
        .build();
    gtk::ListBoxRow::builder()
        .css_classes(["palette-empty"])
        .selectable(false)
        .activatable(false)
        .can_focus(false)
        .child(&label)
        .build()
}

/// One Command's row: its title now, the letters a query matched set
/// heavier, and its first chord at the right; greyed when the Command is
/// not built.
fn item(row: &Row, modes: &Modes) -> gtk::ListBoxRow {
    let title = menus::title(row.command, modes);
    let label = gtk::Label::builder()
        .label(marked(&title, &row.hits))
        .use_markup(true)
        .xalign(0.0)
        .hexpand(true)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .css_classes(["palette-label"])
        .build();
    let line = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    line.append(&label);
    if let Some(keys) = key_label(row.command) {
        let keys = gtk::Label::builder()
            .label(keys)
            .css_classes(["palette-keys"])
            .build();
        line.append(&keys);
    }
    let widget = gtk::ListBoxRow::builder()
        .css_classes(["palette-row"])
        .can_focus(false)
        .child(&line)
        .build();
    if !row.command.built {
        widget.add_css_class("dim");
    }
    widget
}

/// The title as markup, the matched byte ranges set at weight 600
/// (`.label i { font-weight: 600 }`). The hits were found on the Command's
/// title; a paired title reads one half here, so a hit past its end is
/// dropped rather than marked.
fn marked(title: &str, hits: &[(usize, usize)]) -> String {
    let mut out = String::new();
    let mut at = 0;
    for &(from, to) in hits {
        if to > title.len() || from < at || !title.is_char_boundary(from) {
            break;
        }
        out.push_str(&glib::markup_escape_text(&title[at..from]));
        out.push_str("<span weight=\"600\">");
        out.push_str(&glib::markup_escape_text(&title[from..to]));
        out.push_str("</span>");
        at = to;
    }
    out.push_str(&glib::markup_escape_text(&title[at..]));
    out
}

/// The Command's first chord as GTK writes it, the way the menus' rows do.
fn key_label(command: &Command) -> Option<String> {
    let accel = command.accels().into_iter().next()?;
    let (key, modifiers) = gtk::accelerator_parse(&accel)?;
    Some(gtk::accelerator_get_label(key, modifiers).to_string())
}

/// The magnifier (`chrome.js` `MAG`): a circle and a handle, stroked 1.4.
fn magnifier_icon(area: &gtk::DrawingArea, cr: &cairo::Context) {
    chrome::source(area, cr, 1.0);
    cr.set_line_width(1.4);
    cr.set_line_cap(cairo::LineCap::Round);
    cr.arc(5.6, 5.6, 4.1, 0.0, std::f64::consts::TAU);
    let _ = cr.stroke();
    cr.move_to(8.7, 8.7);
    cr.line_to(11.8, 11.8);
    let _ = cr.stroke();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sheet_is_the_oracles_panel_field_and_rows() {
        let sheet = stylesheet(Scheme::Light);
        assert!(sheet.contains("border-radius: 10px"));
        assert!(sheet.contains("font-size: 15px"));
        assert!(sheet.contains("min-height: 24px"));
        assert!(sheet.contains("font-size: 11.5px"));
        assert!(sheet.contains("background-color: #0a94d6"));
        assert!(stylesheet(Scheme::Dark).contains("background-color: #2e2e2e"));
    }

    #[test]
    fn a_hit_is_set_heavier_and_a_hit_past_a_halved_title_is_dropped() {
        assert_eq!(
            marked("Dark Mode", &[(0, 4)]),
            "<span weight=\"600\">Dark</span> Mode"
        );
        assert_eq!(marked("Show Library", &[(20, 24)]), "Show Library");
        assert_eq!(marked("A & B", &[]), "A &amp; B");
    }
}
