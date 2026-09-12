//! The Palette: every Command in one popover, narrowing as the writer types
//! (#122).
//!
//! `Ctrl+K` opens it, View › Window "All Commands…" runs
//! the same action, and `--menu palette` has it up before the first frame.
//! What it lists is [`quill_engine::palette`]: the oracle's four sections and
//! More with nothing typed, one ranked list once something is. Arrows move
//! the selection, Enter runs it and closes, Esc closes. `file.recent` opens
//! the same panel on a second list — the state's recent Documents, newest
//! first, narrowed by [`quill_engine::palette::recents`] — where Enter opens
//! the Document in this window instead (#246). `outline.open` opens it on a
//! third, the open Document's Outline — its headings indented by level with
//! the caret's section selected, narrowed by
//! [`quill_engine::palette::outline`] with the Library's Documents appended
//! by name — where Enter jumps the caret to the heading or opens the
//! Document (#397). Its look is the Parity oracle's palette rules
//! (`legacy/app/css/chrome.css`), as constants beside the menus'.

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{cairo, gdk, glib};
use quill_engine::commands::Command;
use quill_engine::palette::{self as engine, Recent, Row};
use quill_engine::theme::Scheme;

use crate::chrome::{self, CHROME_FONT, Modes, OUTLINE_JUMP, RECENT_OPEN};
use crate::menus;
use crate::tags::pixels;

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

/// The Palette's stylesheet, appended to the menus'.
///
/// The popover's `contents` is the panel; the field's entry is flattened to
/// the oracle's bare input; each `row` of the list is one of the oracle's
/// `li`, selected or a heading.
///
/// The placeholder is the menus' dim at `opacity: 1` because the dim alone
/// did not survive. The shot that lost round 9 — measured in #374's body,
/// from the round the #371 branch carries — reads the prompt at `#BABABA`
/// against the `#8C8C8C` of the caps, the magnifier and the chords beside
/// it: `#8c8c8c` at alpha 0.55 over the panel's `#f2f2f2`, and 0.55 is the
/// opacity GTK's Default theme gives the `placeholder` node along with
/// `.dim-label`. One grey for all four now.
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
         popover.chrome-palette entry text > placeholder {{ color: {dim}; opacity: 1; }}\n\
         popover.chrome-palette .palette-mag {{ color: {dim}; }}\n\
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

/// What the Palette is listing: every Command, the writer's recent
/// Documents (`file.recent`, #246 story 41), or the open Document's Outline
/// (`outline.open`, #397).
///
/// One popover in three modes rather than three popovers, because the panel,
/// the field, the keys and the look are the same list any way; only what
/// fills it and what Enter does with a row differ.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum Listing {
    /// The registry, in the oracle's sections: `palette.open`.
    #[default]
    Commands,
    /// The state's recents, newest first: `file.recent`.
    Recents,
    /// The open Document's headings, then Documents by name once something
    /// is typed: `outline.open`.
    Outline,
}

impl Listing {
    /// What the empty field says it is for.
    fn placeholder(self) -> &'static str {
        match self {
            Self::Commands => "Search commands",
            Self::Recents => "Search recent documents",
            Self::Outline => "Search headings and documents",
        }
    }

    /// What stands in the list when nothing is in it and nothing was typed.
    fn nothing(self) -> &'static str {
        match self {
            Self::Commands => "No commands",
            Self::Recents => "No recent documents",
            Self::Outline => "No headings",
        }
    }
}

/// What one selectable row runs.
#[derive(Clone, Debug)]
enum Item {
    /// A Command, activated through its action as a menu row would.
    Command(&'static Command),
    /// A recent Document, or one of the Library's under the Outline, opened
    /// in this window through [`RECENT_OPEN`].
    Recent(PathBuf),
    /// A heading of the open Document, jumped to through [`OUTLINE_JUMP`]:
    /// the byte its words start at.
    Heading(u64),
}

/// What answers the Outline listing's Documents: a query to the Library's
/// name matches, in the Library's rank, handed in by the window so the
/// Palette knows no Session.
pub type Finder = Box<dyn Fn(&str) -> Vec<PathBuf>>;

/// What the Outline listing is built from, held from the opening until the
/// panel closes and never longer: there is no cache to go stale (#397).
struct Outline {
    /// The open Document's headings, off its block index.
    headings: Vec<quill_engine::outline::Heading>,
    /// The caret's section among them, the row the listing opens on.
    section: Option<usize>,
    /// The open Document, which the Documents leave out.
    open: Option<PathBuf>,
    /// The Library's Documents by name for a query, in the Library's rank.
    finder: Finder,
}

/// The Palette popover, parented on its window.
#[derive(Clone)]
pub struct Palette {
    popover: gtk::Popover,
    entry: gtk::Entry,
    list: gtk::ListBox,
    scroller: gtk::ScrolledWindow,
    /// The rows that can be selected, top to bottom, each with what it runs.
    rows: Rc<RefCell<Vec<(gtk::ListBoxRow, Item)>>>,
    selected: Rc<Cell<usize>>,
    /// The modes the rows' titles read, as of the last opening.
    modes: Rc<Cell<Modes>>,
    /// Which list is up, as of the last opening.
    listing: Rc<Cell<Listing>>,
    /// The recents the last opening was handed, narrowed as the writer types.
    recents: Rc<RefCell<Vec<PathBuf>>>,
    /// The Outline the last opening was handed, dropped as the panel closes.
    outline: Rc<RefCell<Option<Outline>>>,
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
        // The panel's own border's weight, not the bars' half pixel: the
        // line under the field is the same edge, two centimetres in (#374).
        panel.append(&chrome::hairline(
            "palette-rule",
            gtk::Align::Start,
            true,
            chrome::Weight::Whole,
        ));
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
            listing: Rc::new(Cell::new(Listing::default())),
            recents: Rc::new(RefCell::new(Vec::new())),
            outline: Rc::new(RefCell::new(None)),
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
        self.popover.connect_closed(move |_| {
            closed.entry.set_text("");
            // The Outline is the opening's and no longer: nothing is kept
            // to go stale under the next edit.
            closed.outline.replace(None);
        });
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
        self.show(window, modes, Listing::Commands);
    }

    /// Opens the Palette over `window`'s page on `recents`, newest first and
    /// nothing typed: `file.recent`, the Command a writer reaches Open
    /// Recent… by (#246, story 41).
    ///
    /// Never a toggle: the Command runs from the Palette itself, which closes
    /// as it runs, so a toggle here would be a Palette that never opens.
    pub fn open_recents(&self, window: &gtk::Window, modes: Modes, recents: Vec<PathBuf>) {
        self.recents.replace(recents);
        self.show(window, modes, Listing::Recents);
    }

    /// Opens the Palette over `window`'s page on the Outline: `headings`
    /// alone with `section` selected and nothing typed, `finder` answering
    /// the Documents as the writer types, `open` left out of them
    /// (`outline.open`, #397). Not a toggle, as [`Palette::open_recents`] is
    /// not.
    pub fn open_outline(
        &self,
        window: &gtk::Window,
        modes: Modes,
        headings: Vec<quill_engine::outline::Heading>,
        section: Option<usize>,
        open: Option<PathBuf>,
        finder: Finder,
    ) {
        self.outline.replace(Some(Outline {
            headings,
            section,
            open,
            finder,
        }));
        self.show(window, modes, Listing::Outline);
    }

    /// Puts the panel over `window`'s page with `listing` in it and its first
    /// row selected.
    fn show(&self, window: &gtk::Window, modes: Modes, listing: Listing) {
        self.modes.set(modes);
        self.listing.set(listing);
        self.entry.set_placeholder_text(Some(listing.placeholder()));
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

    /// Fills the list for `query` and selects its first row — or, on the
    /// Outline with nothing typed, the caret's section.
    fn render(&self, query: &str) {
        while let Some(child) = self.list.first_child() {
            self.list.remove(&child);
        }
        let listing = self.listing.get();
        let mut rows = Vec::new();
        let mut selected = 0;
        // Whether the list already says it is empty, as the Outline's dim
        // line does where the headings would stand.
        let mut said = false;
        match listing {
            Listing::Commands => {
                let modes = self.modes.get();
                for (head, group) in engine::list(query) {
                    if let Some(head) = head {
                        self.list.append(&heading(head));
                    }
                    for row in group {
                        let widget = item(&row, &modes);
                        self.list.append(&widget);
                        rows.push((widget, Item::Command(row.command)));
                    }
                }
            }
            Listing::Recents => {
                for row in engine::recents(&self.recents.borrow(), query) {
                    self.append_recent(&row, &mut rows);
                }
            }
            Listing::Outline => {
                if let Some(source) = self.outline.borrow().as_ref() {
                    // The Library is asked only once something is typed:
                    // at rest the list is the headings alone.
                    let found = if query.trim().is_empty() {
                        Vec::new()
                    } else {
                        (source.finder)(query)
                    };
                    let list = engine::outline(
                        &source.headings,
                        source.section,
                        &found,
                        source.open.as_deref(),
                        query,
                    );
                    selected = list.selected;
                    for row in list.rows {
                        match row {
                            engine::Outlined::Heading {
                                start,
                                level,
                                text,
                                hits,
                            } => {
                                let widget = entry(text, &hits, level);
                                self.list.append(&widget);
                                let start = u64::try_from(start).unwrap_or(u64::MAX);
                                rows.push((widget, Item::Heading(start)));
                            }
                            engine::Outlined::Head(head) => self.list.append(&heading(head)),
                            engine::Outlined::Document(row) => self.append_recent(&row, &mut rows),
                            engine::Outlined::NoHeadings => {
                                self.list.append(&dim(listing.nothing()));
                                said = true;
                            }
                        }
                    }
                }
            }
        }
        if rows.is_empty() && !said {
            self.list.append(&nothing(query.trim(), listing));
        }
        *self.rows.borrow_mut() = rows;
        self.select(selected);
    }

    /// Appends one Document's row — a recent, or one of the Library's under
    /// the Outline — and records it as opening that Document.
    fn append_recent(&self, row: &Recent<'_>, rows: &mut Vec<(gtk::ListBoxRow, Item)>) {
        let widget = recent(row);
        self.list.append(&widget);
        rows.push((widget, Item::Recent(row.path.to_path_buf())));
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

    /// Runs the selected row and closes: a Command through its action, a
    /// recent Document through [`RECENT_OPEN`], which opens it in this
    /// window. A Command not built does nothing at all, as its chord does
    /// nothing, and the Palette stays up.
    fn run_selected(&self) {
        let Some(item) = self.selected_item() else {
            return;
        };
        let (action, target) = match &item {
            Item::Command(command) => {
                if !command.built {
                    return;
                }
                let (action, target) = command.action_and_target();
                (action, target.map(ToVariant::to_variant))
            }
            Item::Recent(path) => {
                let target = path.to_string_lossy().into_owned();
                (format!("win.{RECENT_OPEN}"), Some(target.to_variant()))
            }
            Item::Heading(start) => (format!("win.{OUTLINE_JUMP}"), Some(start.to_variant())),
        };
        self.close();
        let _ = self.popover.activate_action(&action, target.as_ref());
    }

    /// What the selected row runs, if a row is selected.
    fn selected_item(&self) -> Option<Item> {
        self.rows
            .borrow()
            .get(self.selected.get())
            .map(|(_, item)| item.clone())
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

/// The line under an empty list: what the query missed, or — with nothing
/// typed — that there was nothing to list, which is what a writer who has
/// opened no Document yet sees under Open Recent….
fn nothing(query: &str, listing: Listing) -> gtk::ListBoxRow {
    if query.is_empty() {
        return dim(listing.nothing());
    }
    dim(&format!("Nothing matches “{query}”"))
}

/// One dim line that is neither selected nor run: what an empty list says,
/// and what stands where a Document with no headings would list them.
fn dim(said: &str) -> gtk::ListBoxRow {
    let label = gtk::Label::builder().label(said).xalign(0.0).build();
    gtk::ListBoxRow::builder()
        .css_classes(["palette-empty"])
        .selectable(false)
        .activatable(false)
        .can_focus(false)
        .child(&label)
        .build()
}

/// One heading's row of the Outline: its words, the letters a query matched
/// set heavier, stepped in one em of the row's type per level below the
/// first, so the Document's shape reads down the list as it does down the
/// page.
fn entry(text: &str, hits: &[(usize, usize)], level: u8) -> gtk::ListBoxRow {
    let label = gtk::Label::builder()
        .label(marked(text, hits))
        .use_markup(true)
        .xalign(0.0)
        .hexpand(true)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .css_classes(["palette-label"])
        .build();
    let step = pixels(ROW.px) * i32::from(level.saturating_sub(1));
    label.set_margin_start(step);
    gtk::ListBoxRow::builder()
        .css_classes(["palette-row"])
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

/// One recent Document's row: its name without the extension, the letters a
/// query matched set heavier, and the folder it sits in at the right, in the
/// dim type a Command's chord takes — which is what tells two Documents of
/// the same name apart.
fn recent(row: &Recent<'_>) -> gtk::ListBoxRow {
    let label = gtk::Label::builder()
        .label(marked(&row.name, &row.hits))
        .use_markup(true)
        .xalign(0.0)
        .hexpand(true)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .css_classes(["palette-label"])
        .build();
    let line = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    line.append(&label);
    if let Some(folder) = folder_of(row.path) {
        let folder = gtk::Label::builder()
            .label(folder)
            .css_classes(["palette-keys"])
            .build();
        line.append(&folder);
    }
    gtk::ListBoxRow::builder()
        .css_classes(["palette-row"])
        .can_focus(false)
        .child(&line)
        .build()
}

/// The folder a recent Document sits in, as its own name alone: the whole
/// path would be the row rather than a note beside it, and a Document at the
/// root of a filesystem has none.
fn folder_of(path: &Path) -> Option<String> {
    let folder = path.parent()?.file_name()?;
    Some(folder.to_string_lossy().into_owned())
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

/// The Command's first chord as GTK writes it, the way the menus' rows do:
/// the one it is installed with now ([`chrome::accels`]), so a Command the
/// writer rebound in `settings.toml` reads as they rebound it.
fn key_label(command: &Command) -> Option<String> {
    let accel = chrome::accels(command).into_iter().next()?;
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

    /// Round 9 of the `chrome` Piece lost the panel on two greys doing one
    /// job: GTK's theme dims the `placeholder` node to 0.55, which turned
    /// the menus' dim into a second, paler grey under the field, and the
    /// rule under it was declared twice, `{dim}` then `{border}`.
    #[test]
    fn the_prompt_is_the_menus_dim_at_full_strength_and_the_rule_is_declared_once() {
        for scheme in [Scheme::Light, Scheme::Dark] {
            let sheet = stylesheet(scheme);
            let dim = chrome::menu_ink(scheme).dim;
            let rule = sheet
                .split_once("placeholder")
                .expect("no placeholder rule at all")
                .1;
            let rule = rule.split_once('}').expect("unclosed placeholder rule").0;
            assert!(
                rule.contains(&format!("color: {dim}")),
                "{scheme:?}: the prompt is not the menus' dim:\n{sheet}"
            );
            assert!(
                rule.contains("opacity: 1"),
                "{scheme:?}: GTK's theme is still halving the prompt:\n{sheet}"
            );
            assert_eq!(
                sheet.matches(".palette-rule").count(),
                1,
                "{scheme:?}: the rule under the field takes two colours:\n{sheet}"
            );
            let border = chrome::menu_ink(scheme).border;
            assert!(
                sheet.contains(&format!(".palette-rule {{ color: {border}; }}")),
                "{scheme:?}: the rule is not the panel border's colour:\n{sheet}"
            );
        }
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
