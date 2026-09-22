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
//!
//! Once something is typed, the Commands are followed by the Settings rows
//! no Command answers, under a SETTINGS head (#467): each carries the
//! Settings window's own live control, built by
//! [`crate::settings::control`] on the same write, so the two cannot
//! disagree. Enter on one operates its control and leaves the Palette up;
//! Enter on a jump row — the path lists and the refused lines — opens the
//! window on the row's pane.

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{cairo, gdk, glib};
use quill_engine::commands::Command;
use quill_engine::palette::{self as engine, Control, Found, Recent, Row, Setting};
use quill_engine::spell::Resolved;
use quill_engine::theme::Scheme;

use crate::chrome::{self, CHROME_FONT, Modes, OUTLINE_JUMP, RECENT_OPEN, SETTINGS_PANE};
use crate::menus;
use crate::session::Session;
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

/// A settings row (#467): taller than a Command's, for the window's
/// controls, with its pane's name in a column of its own before the label,
/// as the canvas's direction D has it.
struct SettingRule {
    height: i32,
    /// The pane's column.
    pane: i32,
}

const SETTING: SettingRule = SettingRule {
    height: 30,
    pane: 76,
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
    let setting_height = SETTING.height;
    let controls = crate::settings::controls(scheme, "popover.chrome-palette");
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
         popover.chrome-palette list > row:selected > label, popover.chrome-palette list > row:selected > box > label {{ color: white; }}\n\
         popover.chrome-palette list > row:selected > box > label.palette-keys {{ color: rgba(255, 255, 255, 0.82); }}\n\
         popover.chrome-palette label.palette-keys {{ {keys} }}\n\
         popover.chrome-palette list > row.palette-head {{\n\
         \x20 min-height: 0; margin: 0; padding: {head_top}px {head_x}px {head_bottom}px; border-radius: 0;\n\
         }}\n\
         popover.chrome-palette list > row.palette-head:first-child {{ padding-top: {head_bottom}px; }}\n\
         popover.chrome-palette list > row.palette-head label {{ {head} }}\n\
         popover.chrome-palette list > row.palette-empty {{\n\
         \x20 min-height: 0; margin: 0; padding: {empty_top}px {empty_x}px {empty_bottom}px; border-radius: 0; color: {dim};\n\
         }}\n\
         {controls}\
         popover.chrome-palette list > row.palette-setting {{ min-height: {setting_height}px; }}\n\
         popover.chrome-palette list > row.palette-setting label.palette-pane {{ color: {dim}; }}\n\
         popover.chrome-palette list > row:selected > box > label.palette-pane {{ color: rgba(255, 255, 255, 0.82); }}\n\
         popover.chrome-palette list > row:selected scale > value {{ color: rgba(255, 255, 255, 0.82); }}\n\
         popover.chrome-palette list > row:selected switch {{ background: rgba(255, 255, 255, 0.35); }}\n"
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
    /// A Settings row and its live control, which a jump row has none of.
    Setting(&'static Setting, Option<gtk::Widget>),
}

impl Item {
    /// What Enter does with the row ([`enter`]).
    fn enter(&self) -> Enter {
        match self {
            Self::Setting(setting, _) => enter(Some(setting.control)),
            _ => enter(None),
        }
    }
}

/// What Enter does with the selected row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Enter {
    /// Runs it and closes: a Command, a recent Document, a heading.
    Run,
    /// Operates its control and leaves the Palette up: a switch flips, a
    /// dropdown opens its popup, a spin button or a scale takes the keyboard.
    Operate,
    /// Closes the Palette and opens the Settings window on the row's pane.
    Open,
}

/// What Enter does with a row carrying `control`, or with a row carrying no
/// Settings row at all.
fn enter(control: Option<Control>) -> Enter {
    match control {
        None => Enter::Run,
        Some(Control::Jump) => Enter::Open,
        Some(_) => Enter::Operate,
    }
}

/// One line of the Commands listing, top to bottom.
#[derive(Debug)]
enum Line {
    /// A section's head.
    Head(&'static str),
    /// A Command.
    Command(Row),
    /// A Settings row no Command answers.
    Setting(Found),
}

/// The Commands listing for `query`: [`engine::list`] as it always was, and
/// once something is typed the Settings rows it matches under
/// [`engine::SETTINGS`] — the ones no Command answers, since those are
/// listed as the Commands they are (#467).
fn lines(query: &str) -> Vec<Line> {
    let mut lines = Vec::new();
    for (head, group) in engine::list(query) {
        lines.extend(head.map(Line::Head));
        lines.extend(group.into_iter().map(Line::Command));
    }
    let found: Vec<Found> = engine::settings(query)
        .into_iter()
        .filter(Found::in_palette)
        .collect();
    if !found.is_empty() {
        lines.push(Line::Head(engine::SETTINGS));
        lines.extend(found.into_iter().map(Line::Setting));
    }
    lines
}

/// What the settings rows are built from, handed in by the window as it
/// opens the Palette, which knows no Session otherwise, and dropped as the
/// panel closes: the session every control writes through, and what the
/// window's Spell check language last resolved to.
pub struct Hand {
    pub session: Rc<Session>,
    pub spelling: Option<Resolved>,
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
    /// What the settings rows are built from, as of the last opening on the
    /// Commands; dropped as the panel closes.
    hand: Rc<RefCell<Option<Hand>>>,
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
            // Unfocusable itself, and not `can_focus(false)`, which would
            // bar a settings row's control from the keyboard too.
            .focusable(false)
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
            hand: Rc::new(RefCell::new(None)),
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
        // Esc inside a settings row's control gives the keyboard back to the
        // field rather than closing the panel; a dropdown's own popup, which
        // sits under the row, is left to close itself.
        let controls = gtk::EventControllerKey::new();
        controls.set_propagation_phase(gtk::PropagationPhase::Capture);
        let escaped = self.clone();
        controls.connect_key_pressed(move |_, key, _, _| {
            if key != gdk::Key::Escape || escaped.popped() {
                return glib::Propagation::Proceed;
            }
            escaped.entry.grab_focus_without_selecting();
            glib::Propagation::Stop
        });
        self.scroller.add_controller(controls);

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
            closed.hand.replace(None);
        });
    }

    /// Opens the Palette over `window`'s page with the map at rest and its
    /// first row selected, or closes it if it is up: `palette.open`.
    ///
    /// The entry takes the keyboard when the popover holds a grab, which is
    /// how a writer opens it; under `--deterministic` it holds none and the
    /// keyboard stays on the page, so the caret behind the panel is lit in
    /// the shot as the oracle's is.
    ///
    /// `hand` is what the settings rows a query finds are built from; with
    /// none, they are left out.
    pub fn toggle(&self, window: &gtk::Window, modes: Modes, hand: Option<Hand>) {
        if self.popover.is_visible() {
            self.close();
            return;
        }
        self.hand.replace(hand);
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

    /// Puts `query` in the field of the Palette that is up, which narrows
    /// the list as typing it would: `--query`, set in code because under
    /// `--deterministic` the field cannot take the keyboard.
    pub fn fill(&self, query: &str) {
        self.entry.set_text(query);
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
                let lines = lines(query);
                let hand = self.hand.borrow();
                for line in lines {
                    match line {
                        Line::Head(head) => {
                            if head != engine::SETTINGS || hand.is_some() {
                                self.list.append(&heading(head));
                            }
                        }
                        Line::Command(row) => {
                            let widget = item(&row, &modes);
                            self.list.append(&widget);
                            rows.push((widget, Item::Command(row.command)));
                        }
                        Line::Setting(found) => {
                            let Some(hand) = hand.as_ref() else {
                                continue;
                            };
                            let control = crate::settings::control(
                                &hand.session,
                                found.setting,
                                hand.spelling.as_ref(),
                            );
                            let widget = self.setting(&found, control.as_ref());
                            self.list.append(&widget);
                            rows.push((widget, Item::Setting(found.setting, control)));
                        }
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

    /// One Settings row: its pane's name, its label with the letters a query
    /// matched set heavier, and its live `control` at the right end — or,
    /// for a jump row, where Enter takes the writer.
    ///
    /// The row takes no keyboard itself but its control can, for a spin
    /// button or a scale to be stepped; a click on a switch, a dropdown or a
    /// button leaves the keyboard in the field, and so does a pick from a
    /// dropdown's popup.
    fn setting(&self, found: &Found, control: Option<&gtk::Widget>) -> gtk::ListBoxRow {
        let pane = gtk::Label::builder()
            .label(found.setting.pane.name())
            .xalign(0.0)
            .width_request(SETTING.pane)
            .css_classes(["palette-pane"])
            .build();
        let label = gtk::Label::builder()
            .label(marked(found.setting.label, &found.hits))
            .use_markup(true)
            .xalign(0.0)
            .hexpand(true)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .css_classes(["palette-label"])
            .build();
        let line = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        line.append(&pane);
        line.append(&label);
        match control {
            Some(control) => {
                control.set_valign(gtk::Align::Center);
                if !control.is::<gtk::SpinButton>() && !control.is::<gtk::Scale>() {
                    control.set_focus_on_click(false);
                }
                if let Some(popup) = control.downcast_ref::<gtk::DropDown>().and_then(popup_of) {
                    let back = self.entry.clone();
                    // A pick or an Esc closes the popup and puts the keyboard
                    // back on the dropdown's button; it goes to the field
                    // once the popup has done so.
                    popup.connect_closed(move |_| {
                        let back = back.clone();
                        glib::idle_add_local_once(move || {
                            back.grab_focus_without_selecting();
                        });
                    });
                }
                line.append(control);
            }
            None => {
                let opens = gtk::Label::builder()
                    .label("Opens Settings")
                    .css_classes(["palette-keys"])
                    .build();
                line.append(&opens);
            }
        }
        gtk::ListBoxRow::builder()
            .css_classes(["palette-row", "palette-setting"])
            .focusable(false)
            .child(&line)
            .build()
    }

    /// Whether a settings row's dropdown has its popup up.
    fn popped(&self) -> bool {
        self.rows.borrow().iter().any(|(_, item)| match item {
            Item::Setting(_, Some(control)) => control
                .downcast_ref::<gtk::DropDown>()
                .is_some_and(has_popup_up),
            _ => false,
        })
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
    ///
    /// A settings row is not run: Enter operates its control and the
    /// Palette stays ([`operate`]), or, on a jump row, closes it and opens
    /// the Settings window on the row's pane through [`SETTINGS_PANE`].
    fn run_selected(&self) {
        let Some(item) = self.selected_item() else {
            return;
        };
        let (action, target) = match (&item, item.enter()) {
            (Item::Setting(_, Some(control)), Enter::Operate) => {
                operate(control);
                return;
            }
            (Item::Setting(setting, _), Enter::Open) => (
                format!("win.{SETTINGS_PANE}"),
                Some(setting.pane.name().to_variant()),
            ),
            (Item::Setting(..), _) => return,
            (Item::Command(command), _) => {
                if !command.built {
                    return;
                }
                let (action, target) = command.action_and_target();
                (action, target.map(ToVariant::to_variant))
            }
            (Item::Recent(path), _) => {
                let target = path.to_string_lossy().into_owned();
                (format!("win.{RECENT_OPEN}"), Some(target.to_variant()))
            }
            (Item::Heading(start), _) => (format!("win.{OUTLINE_JUMP}"), Some(start.to_variant())),
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

/// Operates a settings row's control as Enter does: a switch flips, a
/// dropdown opens its popup, a button is clicked, and a spin button or a scale
/// takes the keyboard, which Esc gives back to the field.
fn operate(control: &gtk::Widget) {
    if let Some(switch) = control.downcast_ref::<gtk::Switch>() {
        switch.set_active(!switch.is_active());
    } else if let Some(drop_down) = control.downcast_ref::<gtk::DropDown>() {
        drop_down.emit_activate();
    } else if let Some(button) = control.downcast_ref::<gtk::Button>() {
        button.emit_clicked();
    } else {
        control.grab_focus();
    }
}

/// Whether `drop_down`'s popup is up.
fn has_popup_up(drop_down: &gtk::DropDown) -> bool {
    popup_of(drop_down).is_some_and(|popup| popup.is_visible())
}

/// `drop_down`'s popup: the popover among its children.
fn popup_of(drop_down: &gtk::DropDown) -> Option<gtk::Popover> {
    let mut child = drop_down.first_child();
    while let Some(widget) = child {
        if let Ok(popup) = widget.clone().downcast::<gtk::Popover>() {
            return Some(popup);
        }
        child = widget.next_sibling();
    }
    None
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
/// writer rebound in `settings.toml` reads as they rebound it, written for a
/// label ([`quill_engine::commands::label_accel`]) so that a `Shift` chord on
/// a symbol shows the key the writer presses.
fn key_label(command: &Command) -> Option<String> {
    let accel = quill_engine::commands::label_accel(&chrome::accels(command).into_iter().next()?);
    let (key, modifiers) = gtk::accelerator_parse(&accel)?;
    Some(gtk::accelerator_get_label(key, modifiers).to_string())
}

/// The magnifier (`chrome.js` `MAG`): a circle and a handle, stroked 1.4.
pub(crate) fn magnifier_icon(area: &gtk::DrawingArea, cr: &cairo::Context) {
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

    /// The listing's lines as the kinds the Palette draws them as, the
    /// settings rows by label.
    fn shape(query: &str) -> Vec<String> {
        lines(query)
            .into_iter()
            .map(|line| match line {
                Line::Head(head) => format!("head {head}"),
                Line::Command(_) => "command".to_owned(),
                Line::Setting(found) => format!("setting {}", found.setting.label),
            })
            .collect()
    }

    #[test]
    fn with_nothing_typed_the_listing_is_the_commands_row_for_row() {
        let mut expected = Vec::new();
        for (head, group) in engine::list("") {
            expected.extend(head.map(|head| format!("head {head}")));
            expected.extend(group.iter().map(|_| "command".to_owned()));
        }
        assert_eq!(shape(""), expected);
        assert!(!shape("").contains(&format!("head {}", engine::SETTINGS)));
    }

    #[test]
    fn title_finds_the_commands_then_title_page_as_a_switch_under_settings() {
        let shape = shape("title");
        let head = shape
            .iter()
            .position(|line| *line == format!("head {}", engine::SETTINGS))
            .expect("a SETTINGS head");
        // No Command's title holds "title" today; whatever does is above.
        assert!(shape[..head].iter().all(|line| line == "command"));
        assert_eq!(shape[head + 1], "setting Title page");
        let Some(Line::Setting(found)) = lines("title").into_iter().nth(head + 1) else {
            panic!("no settings row after the head");
        };
        assert_eq!(found.setting.control, Control::Switch);
    }

    #[test]
    fn manuscript_finds_the_three_templates_as_commands_and_no_settings_row() {
        let lines = lines("manuscript");
        let commands: Vec<&str> = lines
            .iter()
            .filter_map(|line| match line {
                Line::Command(row) => Some(row.command.id),
                _ => None,
            })
            .collect();
        for id in [
            "template.manuscriptMono",
            "template.manuscriptDuo",
            "template.manuscriptQuattro",
        ] {
            assert!(commands.contains(&id), "{id} in {commands:?}");
        }
        assert!(
            lines
                .iter()
                .all(|line| !matches!(line, Line::Setting(_) | Line::Head(_))),
            "{:?}",
            lines
        );
    }

    #[test]
    fn enter_runs_a_command_and_closes() {
        assert_eq!(enter(None), Enter::Run);
    }

    #[test]
    fn enter_operates_a_control_and_stays() {
        for control in [
            Control::Switch,
            Control::Dropdown,
            Control::Spin,
            Control::Scale,
            Control::Button,
        ] {
            assert_eq!(enter(Some(control)), Enter::Operate, "{control:?}");
        }
    }

    #[test]
    fn enter_on_a_jump_row_closes_and_opens_settings() {
        assert_eq!(enter(Some(Control::Jump)), Enter::Open);
    }

    #[test]
    fn the_settings_rows_take_the_windows_controls_and_not_its_frame() {
        for scheme in [Scheme::Light, Scheme::Dark] {
            let sheet = stylesheet(scheme);
            for rule in [
                "popover.chrome-palette switch {",
                "popover.chrome-palette spinbutton {",
                "popover.chrome-palette scale > trough {",
                "popover.chrome-palette dropdown popover > contents {",
            ] {
                assert!(sheet.contains(rule), "{scheme:?}: {rule}");
            }
            assert!(!sheet.contains("window.settings"), "{scheme:?}");
            assert!(!sheet.contains("popover.chrome-palette {\n  background"));
            assert!(!sheet.contains("settings-"), "{scheme:?}");
        }
    }
}
