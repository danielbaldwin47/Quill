//! The Library beside the page: sections, rows, the search field and the
//! status line (#253), and what the field finds (#254).
//!
//! One sidebar per window, all of them showing the one Library the session
//! holds (`docs/architecture.md` § Library, § Windows). It stands left of the
//! page and pushes it right rather than covering it, which is the Parity
//! oracle's model (`legacy/app/css/files.css`: `#app { padding-left:
//! var(--lib-w) }`), at the 368 logical pixels that file measured off iA's
//! own Library.
//!
//! What it draws is the spec's rather than the oracle's where the two differ
//! (#246 § The sidebar and the Piece): a Pinned section, hidden while nothing
//! is pinned, then one section per Location headed by that folder's name with
//! its tree beneath, folders first and collapsed until they are opened, and a
//! gap between sections. The oracle has one browser Location and heads it
//! "Library". The type is the GTK UI face the bars are set in and the ground
//! is the theme's paper, so the pane belongs to the same window as the page
//! rather than to a file manager.
//!
//! Typing in the search field puts the engine's results in place of the tree:
//! the files whose names matched first, then the files whose texts did, each
//! of those with the one snippet around its match and the match marked
//! ([`quill_engine::library::Library::search`]). Enter opens the highlighted
//! hit and Esc clears the field and hands the keyboard back to the page.
//!
//! A row can be acted on as well as opened (#256): a second click or `F2` puts
//! a field in the place of its name, and the right button opens what it can do
//! — Open, Rename, Duplicate, Pin or Unpin, Move to Trash, and on a Location's
//! head, Remove from Library. The pane decides *which* row; what happens to it
//! is the window's ([`crate::window`]) and, under that, the engine's, which
//! does the disk before the pane is drawn again.
//!
//! A row can also be dragged (#257). Let go over a folder's row or a Location's
//! head it moves into that folder, asked about first where the writer asked to
//! be asked (`library.confirm_move`); let go anywhere over the Pinned section
//! it is pinned, and the section lights as the one target it is rather than the
//! row under the pointer. What each drop would do is [`crate::files::dropped`],
//! a decision over paths, and a drop it would do nothing with — a folder onto
//! itself, a file into the folder it is already in, a row already pinned — is
//! refused while the drag is still in the air.
//!
//! Its right edge is the divider (#260): a [`GRAB`]-pixel strip lying over the
//! edge, which paints nothing at all — the hairline there is the pane's own
//! border — so that the states judged at [`WIDTH`] are the pixels they were.
//! A drag on it sets the pane's width for the app, every window at once
//! ([`crate::window::Window::resize_library`]), and a double-click puts it
//! back to [`WIDTH`].
//!
//! Its measurements are `files.css`'s, as constants below; its colours are the
//! theme's roles, through [`stylesheet`], which rides with the bars' sheet so
//! that one ground change repaints both.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, SystemTime};

use gtk::prelude::*;
use gtk::{cairo, gdk, gio, glib, graphene};
use quill_engine::document::full_name;
use quill_engine::library::{Contents, File, Library, Row, Section, Snippet, Sort, View};
use quill_engine::settings::library_width;
use quill_engine::theme::{self, Colour, Role, Scheme};

use crate::chrome::{self, CHROME_FONT};
use crate::files::{self, Dropped, Onto, Standing};
use crate::ground::Ground;
use crate::tags::pixels;
use crate::window::Window;

/// The pane's width until a writer drags the divider (`files.css`
/// `--lib-w: 368px`), and the width a double-click on the divider puts back.
pub const WIDTH: i32 = 368;
/// How wide the divider is to a pointer: the last logical pixels of the pane,
/// lying over its right edge rather than beside it, so that the page stands
/// where it stood and the strip has room to be caught.
const GRAB: i32 = 6;
/// What the pointer becomes over the divider: the name a desktop's cursor
/// theme keeps its two-headed horizontal arrow under.
const RESIZE_CURSOR: &str = "col-resize";
/// The pane's own head, the title bar's height so that what the pane is called
/// and the Document's name stand on one line across the window
/// (`.lib-head { height: var(--bar-top) }`).
const HEAD_HEIGHT: i32 = chrome::TOP_HEIGHT;
/// The head's insets and the air between its buttons (`.lib-head { padding: 0
/// 6px 0 8px; gap: 2px }`).
const HEAD_LEFT: i32 = 8;
/// What the head keeps clear of the right edge.
const HEAD_RIGHT: i32 = 6;
/// Between one thing in the head and the next.
const HEAD_GAP: i32 = 2;
/// A head button's side (`#library .lib-btn { width: 26px; height: 26px }`).
const BUTTON: i32 = 26;
/// A head button's corner (`border-radius: 5px`).
const BUTTON_RADIUS: i32 = 5;
/// The panel and plus marks in the head (`I.panel`, `I.plus`, fifteen by
/// fifteen).
const MARK: i32 = 15;
/// What the pane is called, above the Locations it holds. The oracle names its
/// one browser Location here; a Location of ours is named by its own section
/// head, so what stands here is the pane.
const TITLE: &str = "Library";
/// Below the search field, before the sort row (`.lib-find { padding: 0 10px
/// 8px }`).
const FIELD_BELOW: i32 = 8;
/// Where the manuscripts are, at the status line's right. The counterpart of
/// the oracle's "In this browser": ours are plain Markdown files in the
/// writer's own folders (ADR 0002), which is the fact that decides whether a
/// writer trusts a Library with a manuscript.
const WHERE: &str = "On this device";
/// The field's height (`#lib-q { height: 26px }`).
const FIELD_HEIGHT: i32 = 26;
/// What the field keeps clear of the pane's right edge (`.lib-find { padding:
/// 0 10px }`; its left is [`PANE_LEFT`], which every left edge in the pane is).
const FIELD_PAD: i32 = 10;
/// Between the magnifier and the text (`#lib-q { padding-left: 26px }` less
/// the magnifier's own place).
const FIELD_GAP: i32 = 6;
/// The magnifier's side (`I.search`, twelve by twelve).
const MAG: i32 = 12;
/// The field's type (`#lib-q { font-size: 13px }`).
const FIELD_PX: f64 = 13.0;
/// The sort row's height (`.lib-sort { height: 26px }`).
const SORT_HEIGHT: i32 = 26;
/// The sort row's type, which the count and the status line share
/// (`font-size: 11.5px`).
const META_PX: f64 = 11.5;
/// The sort button's own padding (`#lib-sortb { padding: 0 4px }`).
const SORT_BUTTON_PAD: i32 = 4;
/// What the count keeps clear of the right edge (`padding-right: 14px`).
const SORT_RIGHT: i32 = 14;
/// A chevron's side (`I.chev`, eleven by eleven).
const CHEV: i32 = 11;
/// A row's inset from the pane's left edge (`.lib-row { padding-left: 16px }`).
const ROW_LEFT: i32 = 16;
/// The pane's one left margin: where the ink of the search field, of the sort
/// control and of a row's icon column all begins.
///
/// `files.css` reaches the same edge through three paddings, each of them
/// answering the inset its own mark is drawn with; ours are drawn to their
/// edges, so the margin is said once, here, and every one of the three is set
/// from it. The round that asked for it read the pane's left edges as
/// 10.5/10.5/14/24.5 px against the oracle's 14.5/15/16.5/20
/// (`progress/rounds/files-r3.json`, the `search` state).
const PANE_LEFT: i32 = ROW_LEFT;
/// A row's inset from the right (`padding-right: 14px`).
const ROW_RIGHT: i32 = 14;
/// Above a row's first line (`padding-top: 10px`).
const ROW_TOP: i32 = 10;
/// Below a file row's excerpt (`padding-bottom: 7px`).
const ROW_BOTTOM: i32 = 7;
/// Below a folder row (`.lib-row.folder { padding-bottom: 10px }`).
const FOLDER_BOTTOM: i32 = 10;
/// The icon column's width (`grid-template-columns: 18px`).
const ICON_COLUMN: i32 = 18;
/// Between the icon column and the name (`column-gap: 12px`).
const ICON_GAP: i32 = 12;
/// What one folder of depth indents a row by (`.lib-row.in { padding-left:
/// 32px }`, sixteen past [`ROW_LEFT`]).
const INDENT: i32 = 16;
/// A document icon's size (`I.doc`, thirteen by sixteen).
const DOC: (i32, i32) = (13, 16);
/// A folder icon's size (`I.folder`, fifteen by thirteen).
const FOLDER: (i32, i32) = (15, 13);
/// A row's name (`.lib-row { font-size: 14px }`).
const NAME_PX: f64 = 14.0;
/// A file row's date (`.lib-row .dt { font-size: 13px }`).
const DATE_PX: f64 = 13.0;
/// A folder row's name, and a section head's (`.lib-row.folder .nm`,
/// `.lib-loc { font-size: 13.5px }`).
const HEAD_PX: f64 = 13.5;
/// A file row's excerpt (`.lib-row .ex { font-size: 13.5px }`).
const EXCERPT_PX: f64 = 13.5;
/// The excerpt's leading (`line-height: 19.5px`).
const EXCERPT_LEADING: f64 = 19.5;
/// How many lines of the excerpt a row shows (`-webkit-line-clamp: 2`).
const EXCERPT_LINES: i32 = 2;
/// A section head's height (`.lib-loc { height: 26px }`).
const SECTION_HEIGHT: i32 = 26;
/// A section head's inset, and the gap between its icon and its name
/// (`.lib-loc { padding: 0 6px; margin-left: 2px; gap: 6px }` against the
/// pane's own 8 px edge).
const SECTION_LEFT: i32 = 12;
/// Between a section head's icon and its name.
const SECTION_GAP: i32 = 6;
/// The air between one section and the next. The spec asks for "a visible gap
/// between sections", which the oracle — having one Location — has nowhere to
/// show; this is the sort row's height, so a section head stands as far from
/// the section above it as the first row does from the sort rule.
const SECTION_AIR: i32 = 14;
/// The status line's height, the stats bar's own (`--bar-bottom`), so the
/// pane's foot and the page's stand on one line.
const FOOT_HEIGHT: i32 = chrome::BOTTOM_HEIGHT;
/// The status dot's side (`.lib-status .dot { width: 6px }`).
const DOT: i32 = 6;
/// How much of the ink the dot is drawn in at rest (`opacity: .45`).
const DOT_ALPHA: f64 = 0.45;
/// Between the dot and its text (`.lib-status { gap: 7px }`).
const DOT_GAP: i32 = 7;
/// Where the status line starts (`.lib-foot { padding-left: 12px }`).
const FOOT_LEFT: i32 = 12;
/// The selection's bar (`.lib-row.file.sel::after { width: 3.5px; left: 4px;
/// top: 4px; bottom: 5px }`), rounded at 2.
const BAR: Bar = Bar {
    width: 4,
    left: 4,
    top: 4,
    bottom: 5,
    radius: 2,
};

/// The accent bar down the selected row's left edge.
struct Bar {
    /// Its width. The oracle's 3.5 rounds to the whole pixel a widget is
    /// asked for in, and lands on the same two device pixels at scale 2.
    width: i32,
    /// How far in from the pane's edge it stands.
    left: i32,
    /// The air above it inside the row.
    top: i32,
    /// The air below it.
    bottom: i32,
    /// Its corner.
    radius: i32,
}

/// What marks the row of a Document whose file changed under unsaved edits.
///
/// The oracle's warning colour, which it puts on the status dot
/// (`legacy/app/css/files.css` line 212: `.lib-status[data-k="dirty"] .dot {
/// background: #e0a030 }`); the spec puts it on the row as well, so that a
/// writer scanning the pane can see which Document is waiting on them.
const WARN: &str = "#e0a030";

/// How far down the row the dot sits, so that it stands on the name's line
/// rather than at the row's top edge.
const DOT_TOP: i32 = 5;

/// The "·" the status line's words are separated by.
const SEPARATOR: &str = "·";

/// What the search field says while it is empty.
const PLACEHOLDER: &str = "Search documents";

/// How much of the accent stands behind a matched word in a snippet
/// (`.lib-row .ex mark { background: color-mix(in srgb, var(--accent) 28%,
/// transparent) }`), over the paper the row is drawn on. The words themselves
/// go to the ink the same rule sets them in (`color: var(--fg)`), which is
/// what makes a match visible in a line of grey.
const MARK_TINT: f64 = 0.28;

/// How much of the accent stands behind a row a dragged row would land on,
/// over the paper beneath it.
///
/// Stronger than a hover and weaker than a selection's bar: the light says
/// "here", and a whole Pinned section lit at hover strength would not read as
/// one target at all.
const DROP_TINT: f64 = 0.16;

/// The class a row wears while it is the target a drop would land on.
const DROP_CLASS: &str = "lib-drop";

/// How long the field waits after a keystroke before it searches.
///
/// A content search reads every shown file whose name did not match, so the
/// field answers the writer's pause rather than the writer's typing; the
/// engine's cache then holds those texts, and the query after this one reads
/// nothing ([`Contents`]). Re-armed by each keystroke, so a word typed at
/// speed is one search.
const SETTLE_MS: u64 = 150;

/// How much of a file the excerpt is taken from.
///
/// A bound rather than the whole file: the excerpt is two lines of type, and
/// a Library of a thousand Documents would otherwise read a thousand files
/// whole to draw them. Four kilobytes is more prose than two lines of even
/// the widest pane can show, and [`EXCERPT_CHARS`] is the cap on what is kept
/// of them.
const EXCERPT_BYTES: u64 = 4096;
/// How many characters of it a row keeps. The label ellipsizes at two lines
/// long before this at any width the divider can be dragged to: a pane as
/// wide as a maximized window on a 3840 px screen leaves it is 3520 logical
/// px, and two lines of it hold around a thousand characters at [`EXCERPT_PX`]
/// (#260). The cap is what stops a one-line file of 4 KB being laid out in
/// full to find that out.
const EXCERPT_CHARS: usize = 1200;

/// The months a date is named in, January first.
///
/// Spelled here rather than taken from the locale so that a judged shot reads
/// the same on a machine set to any language, as the rest of the chrome does.
const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];
/// The days a recent date is named by, Sunday first.
const DAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

/// The sidebar's stylesheet, appended to the bars' ([`crate::chrome::stylesheet`]).
///
/// Every colour the pane draws is a role of the theme's table, so a ground
/// change is a stylesheet change and nothing else: the paper it stands on, the
/// ink its names are set in, the grey of its dates and excerpts, the hairline
/// between its rows and the accent down the selected one.
#[must_use]
pub fn stylesheet(ground: Ground) -> String {
    let Ground { scheme, colours } = ground;
    let paper = colours.colour(Role::Paper).to_hex();
    let ink = colours.colour(Role::Ink).to_hex();
    let dim = colours.colour(Role::ChromeFg).to_hex();
    let rule = colours.colour(Role::Rule).to_css();
    let accent = colours.colour(Role::Accent).to_hex();
    let drop = Colour::over(
        colours.colour(Role::Accent),
        colours.colour(Role::Paper),
        DROP_TINT,
    )
    .to_hex();
    // The row a click would take, and the row the open Document is on: the
    // oracle's `--lib-hover` and `--lib-sel`, which are steps off whatever
    // ground they land on rather than colours of their own, and so are ink on
    // the light ground and paper on the dark one.
    let (hit, selected) = match scheme {
        Scheme::Light => ("rgba(0, 0, 0, 0.035)", "rgba(0, 0, 0, 0.02)"),
        Scheme::Dark => ("rgba(255, 255, 255, 0.045)", "rgba(255, 255, 255, 0.028)"),
    };
    let Bar {
        width: bar_width,
        left: bar_left,
        top: bar_top,
        bottom: bar_bottom,
        radius: bar_radius,
    } = BAR;
    format!(
        ".library {{\n\
         \x20 background-color: {paper}; color: {ink};\n\
         \x20 border-right: 1px solid {rule};\n\
         \x20 font-family: {CHROME_FONT}; font-size: {NAME_PX}px;\n\
         }}\n\
         .library .lib-sort {{ border-bottom: 1px solid {rule}; }}\n\
         .library .lib-foot {{ border-top: 1px solid {rule}; }}\n\
         .library .lib-rule {{ background-color: {rule}; }}\n\
         .library label.lib-name {{ font-size: {NAME_PX}px; color: {ink}; }}\n\
         .library label.lib-head {{ font-size: {HEAD_PX}px; font-weight: 500; color: {ink}; }}\n\
         .library label.lib-date {{\n\
         \x20 font-size: {DATE_PX}px; color: {dim}; font-feature-settings: \"tnum\";\n\
         }}\n\
         .library label.lib-excerpt {{ font-size: {EXCERPT_PX}px; color: {dim}; }}\n\
         .library label.lib-meta {{\n\
         \x20 font-size: {META_PX}px; color: {dim}; font-feature-settings: \"tnum\";\n\
         }}\n\
         .library .lib-icon {{ color: {dim}; }}\n\
         .library .lib-folder-icon {{ color: {accent}; }}\n\
         .library .lib-changed {{ color: {WARN}; }}\n\
         .library button.lib-btn {{\n\
         \x20 background: none; border: none; box-shadow: none; outline: none;\n\
         \x20 min-height: 0; min-width: 0; padding: 0;\n\
         \x20 border-radius: {BUTTON_RADIUS}px; color: {dim};\n\
         }}\n\
         .library button.lib-btn:hover {{ background-color: {hit}; color: {ink}; }}\n\
         .library button.lib-offer {{\n\
         \x20 font-size: {META_PX}px; padding: 0 {SORT_BUTTON_PAD}px;\n\
         }}\n\
         .library button.lib-sortb {{\n\
         \x20 background: none; border: none; box-shadow: none; outline: none;\n\
         \x20 min-height: 0; min-width: 0; padding: 0 {SORT_BUTTON_PAD}px;\n\
         \x20 border-radius: 4px; color: {dim};\n\
         }}\n\
         .library button.lib-sortb:hover {{ background-color: {hit}; color: {ink}; }}\n\
         .library entry {{\n\
         \x20 background: none; border: none; box-shadow: none; outline: none;\n\
         \x20 padding: 0; margin: 0; min-height: 0; min-width: 0;\n\
         \x20 font-size: {FIELD_PX}px; color: {ink}; caret-color: {accent};\n\
         }}\n\
         .library entry text > placeholder {{ color: {dim}; }}\n\
         .library entry.lib-rename {{ font-size: {NAME_PX}px; }}\n\
         .library scrolledwindow, .library list {{ background: none; }}\n\
         .library list > row {{\n\
         \x20 background: none; padding: 0; min-height: 0; outline: none;\n\
         }}\n\
         .library list > row:hover {{ background-color: {hit}; }}\n\
         .library list > row:selected {{ background-color: {selected}; }}\n\
         .library list > row:selected label.lib-name {{ color: {ink}; }}\n\
         .library list > row:selected .lib-icon {{ color: {accent}; }}\n\
         .library .lib-bar {{\n\
         \x20 background: none; border-radius: {bar_radius}px;\n\
         \x20 min-width: {bar_width}px; margin: {bar_top}px 0 {bar_bottom}px {bar_left}px;\n\
         }}\n\
         .library list > row:selected .lib-bar {{ background-color: {accent}; }}\n\
         .library list > row.{DROP_CLASS} {{ background-color: {drop}; }}\n"
    )
}

/// What every row of one refresh is drawn against.
///
/// The three things that are the same for all of them and none of which the
/// tree holds: the setting a name is shown under, the clock a date is measured
/// from, and what was read of the files.
struct Drawing<'a> {
    /// Whether a name keeps its extension (`library.show_extensions`).
    extensions: bool,
    /// Now, as the dates are said against it. `None` where the clock could not
    /// be asked, which is a row with no date rather than no row.
    now: Option<&'a glib::DateTime>,
    /// The head of every shown file, by path.
    read: &'a BTreeMap<PathBuf, Head>,
    /// What a matched word in a snippet is marked with, over the paper.
    mark: Colour,
    /// What a matched word itself is set in, out of the excerpt's grey.
    marked: Colour,
}

/// One file's row, and what it is drawn from.
///
/// The tree's rows and a search's results are the same row built from
/// different places: the tree knows how deep the file lies, a result knows
/// what the query matched in it.
struct FileRow<'a> {
    /// The file's name on disk, extension and all.
    name: &'a str,
    /// Where it is.
    path: &'a Path,
    /// How many folders below its section it lies; a result is drawn flat.
    depth: usize,
    /// When it was last written.
    modified: Option<SystemTime>,
    /// The snippet around a content hit's match, drawn in place of the file's
    /// own excerpt and with the match marked. A tree row and a name hit have
    /// none, and show the excerpt.
    snippet: Option<&'a Snippet>,
}

/// One row of the list, and what it stands for.
///
/// Clonable so that the rows of one section can be taken out of the list
/// ([`Sidebar::drawn_since`]) and handed a drop target between them: every
/// field of it is a handle on the one widget or a path.
#[derive(Clone)]
struct Listed {
    row: gtk::ListBoxRow,
    /// The file or folder it draws.
    path: PathBuf,
    /// Whether it is a folder, which opens and closes rather than opening a
    /// Document.
    folder: bool,
    /// The label its name is drawn in, which a rename hides and puts a field
    /// in the place of ([`Sidebar::start_rename`]).
    name: gtk::Label,
    /// The dot that says this row's file changed on disk. Shown only while
    /// this is the row of the Document the window holds and that Document is
    /// in a conflict ([`Sidebar::set_conflicted`]).
    dot: gtk::DrawingArea,
}

/// Where a drag on the divider began: the pane's width then, and where the
/// pointer stood in the window's own pixels, which is what the width the drag
/// is asking for is measured off ([`Sidebar::dragged`]).
#[derive(Clone, Copy)]
struct Grab {
    /// The pane's width when the drag began, in logical pixels.
    width: i32,
    /// Where the pointer was across the window, in logical pixels.
    at: f64,
}

/// The Library beside the page.
#[derive(Clone)]
pub struct Sidebar {
    /// The pane and the divider over its right edge: what stands beside the
    /// page ([`Sidebar::widget`]) and what is shown and hidden, since an
    /// overlay whose pane is away would still take the divider's room.
    frame: gtk::Overlay,
    root: gtk::Box,
    /// The divider: a strip of [`GRAB`] pixels on the pane's right edge with
    /// nothing in it and nothing drawn, which a drag widens the pane by and a
    /// double-click puts back to [`WIDTH`].
    divider: gtk::Box,
    /// What the pane's head calls it: the one Location's folder, where that is
    /// the whole Library, and [`TITLE`] where there is more than one thing
    /// under it.
    title: gtk::Label,
    entry: gtk::Entry,
    sort_label: gtk::Label,
    count: gtk::Label,
    list: gtk::ListBox,
    status: gtk::Label,
    /// "· Reload · Keep" beside the status line, hidden until there is a
    /// conflict to resolve.
    offer: gtk::Box,
    reload: gtk::Button,
    keep: gtk::Button,
    /// Whether the Document the window holds is changed on disk, which is what
    /// the dot on its row says.
    conflicted: Rc<Cell<bool>>,
    /// The window this sidebar belongs to, so a row can open a Document in
    /// it. Weak, because the window owns the sidebar.
    window: Rc<RefCell<Option<glib::WeakRef<Window>>>>,
    /// The rows now drawn, top to bottom, for the highlight and the arrows.
    rows: Rc<RefCell<Vec<Listed>>>,
    /// The section heads now drawn, each with the Location it names, so that a
    /// right-click on one can offer to drop that Location. The Pinned head
    /// names no Location and is not among them.
    heads: Rc<RefCell<Vec<(gtk::ListBoxRow, PathBuf)>>>,
    /// The folders the writer has opened. Everything else is closed, which is
    /// what the spec asks a section to open at.
    expanded: Rc<RefCell<BTreeSet<PathBuf>>>,
    /// What the list is sorted by. Date, newest first, until a writer says
    /// otherwise; not a setting, because nothing persists it yet.
    sort: Rc<Cell<Sort>>,
    /// The Document the window is showing, whose row is the highlighted one.
    open: Rc<RefCell<Option<PathBuf>>>,
    /// The pane's own head. Where the Library is one Location with nothing
    /// Pinned beside it, the head is that Location's header as well
    /// ([`Sidebar::head_as_location`]).
    head: gtk::Box,
    /// The Location the head is heading, or `None` where it is only saying
    /// Library. Taken down by each [`Sidebar::refresh`].
    header: Rc<RefCell<Option<PathBuf>>>,
    /// The head's drop target while it is a Location's header, kept so that it
    /// can be taken off again when it stops being one.
    head_drop: Rc<RefCell<Option<gtk::DropTarget>>>,
    /// The menu the right button opens over a row.
    ///
    /// Parented once and handed a model as it opens, which is the shape the
    /// bars' menus have ([`crate::chrome::Bars`]). A popover built per click
    /// and unparented as that click closes it is out of the widget tree before
    /// the item pressed has resolved its action, and nothing the menu offers
    /// ever runs.
    menu: gtk::PopoverMenu,
    /// The menu the right button opens over the pane's own head, which is the
    /// one Location's header where the Library has only the one
    /// ([`Sidebar::head_as_location`]). Kept for the same reason [`Sidebar::menu`] is.
    head_menu: gtk::PopoverMenu,
    /// The Locations whose trees are folded away, which a click on a section
    /// head puts one into and takes it out of again. Held for as long as the
    /// window is and no longer: nothing persists it, because what a writer
    /// folded to see past is not what they want the Library to open at.
    folded: Rc<RefCell<BTreeSet<PathBuf>>>,
    /// The file texts search has read, kept for as long as the window is, so
    /// that a query over a tree nothing has touched reads nothing.
    contents: Rc<RefCell<Contents>>,
    /// What each shown file's first lines said, by path, so that a refresh
    /// over files nothing has written reads nothing ([`Sidebar::refresh`]).
    read: Rc<RefCell<BTreeMap<PathBuf, Head>>>,
    /// The keystroke the field is waiting out before it searches
    /// ([`SETTLE_MS`]).
    settle: Rc<RefCell<Option<glib::SourceId>>>,
}

impl Default for Sidebar {
    fn default() -> Self {
        Self::new()
    }
}

impl Sidebar {
    /// Builds the pane, empty and hidden.
    #[must_use]
    pub fn new() -> Self {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.add_css_class("library");
        root.set_width_request(WIDTH);
        // The pane is exactly its width and the page takes the rest of the
        // window: said here rather than left to the children, because the list
        // inside it expands and a box takes its children's answer.
        root.set_hexpand(false);

        let title = gtk::Label::new(Some(TITLE));
        let head = head(&title);
        root.append(&head);
        let entry = gtk::Entry::builder()
            .hexpand(true)
            .has_frame(false)
            .placeholder_text(PLACEHOLDER)
            .build();
        root.append(&find(&entry));

        let sort_label = gtk::Label::new(Some(sort_title(Sort::Date)));
        let count = gtk::Label::new(None);
        count.add_css_class("lib-meta");
        let sort_button = sort_button(&sort_label);
        root.append(&sort_row(&sort_button, &count));

        let list = gtk::ListBox::new();
        list.set_selection_mode(gtk::SelectionMode::Browse);
        let scroller = gtk::ScrolledWindow::builder()
            .vexpand(true)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .child(&list)
            .build();
        root.append(&scroller);

        let status = gtk::Label::new(Some(&files::said(Standing::AtRest)));
        status.add_css_class("lib-meta");
        let reload = offer_word("Reload");
        let keep = offer_word("Keep");
        let offer = offered(&reload, &keep);
        root.append(&foot(&status, &offer));

        // On the pane and not on the list: a `GtkListBox` takes only rows
        // away, so a popover parented on it is a child [`Sidebar::refresh`]
        // cannot remove and its loop over the children never ends. A popover
        // is a `GtkNative` and no layout manager gives it room, so the pane's
        // own column is unmoved by it.
        let menu = menu_popover(&root);
        let head_menu = menu_popover(&head);
        // The divider lies over the pane's last [`GRAB`] pixels rather than
        // standing beside them: an overlay child is given room without taking
        // any, so the page begins where it always did and a state judged at
        // [`WIDTH`] is the pixels it was. It holds nothing and is styled by
        // nothing, so there is nothing for it to draw.
        let divider = gtk::Box::new(gtk::Orientation::Vertical, 0);
        divider.set_width_request(GRAB);
        divider.set_halign(gtk::Align::End);
        divider.set_cursor_from_name(Some(RESIZE_CURSOR));
        let frame = gtk::Overlay::new();
        frame.set_child(Some(&root));
        frame.add_overlay(&divider);
        frame.set_hexpand(false);
        // Hidden on the frame and not on the pane: a hidden pane inside a
        // shown overlay would still leave the divider's strip beside the page.
        frame.set_visible(false);
        let sidebar = Self {
            frame,
            divider,
            root,
            head,
            title,
            entry,
            sort_label,
            count,
            list,
            status,
            offer,
            reload,
            keep,
            conflicted: Rc::new(Cell::new(false)),
            window: Rc::new(RefCell::new(None)),
            rows: Rc::new(RefCell::new(Vec::new())),
            heads: Rc::new(RefCell::new(Vec::new())),
            expanded: Rc::new(RefCell::new(BTreeSet::new())),
            sort: Rc::new(Cell::new(Sort::Date)),
            open: Rc::new(RefCell::new(None)),
            header: Rc::new(RefCell::new(None)),
            head_drop: Rc::new(RefCell::new(None)),
            menu,
            head_menu,
            folded: Rc::new(RefCell::new(BTreeSet::new())),
            contents: Rc::new(RefCell::new(Contents::new())),
            read: Rc::new(RefCell::new(BTreeMap::new())),
            settle: Rc::new(RefCell::new(None)),
        };
        let cycling = sidebar.clone();
        sort_button.connect_clicked(move |_| cycling.cycle_sort());
        sidebar.wire();
        sidebar
    }

    /// The search field's keys.
    ///
    /// Typing narrows the list once the keystrokes stop, Enter opens the
    /// highlighted hit, Esc clears the query and hands the keyboard back to
    /// the page, and Down steps into the list, where the arrows walk the rows
    /// (`legacy/app/js/files.js`, the field's `keydown`).
    fn wire(&self) {
        let typed = self.clone();
        self.entry.connect_changed(move |_| typed.settle());
        let entered = self.clone();
        self.entry
            .connect_activate(move |_| entered.open_highlighted());
        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        let pressed = self.clone();
        keys.connect_key_pressed(move |_, key, _, _| match key {
            gdk::Key::Escape => {
                pressed.clear();
                glib::Propagation::Stop
            }
            gdk::Key::Down => {
                pressed.step_into_list();
                glib::Propagation::Stop
            }
            _ => glib::Propagation::Proceed,
        });
        self.entry.add_controller(keys);
    }

    /// The widget to stand left of the page: the pane with its divider over
    /// its right edge.
    #[must_use]
    pub fn widget(&self) -> &gtk::Widget {
        self.frame.upcast_ref()
    }

    /// Gives the pane its window, which is what a row opens a Document in,
    /// and draws the Library for the first time.
    ///
    /// Split from [`Sidebar::new`] because a window's widgets are built before
    /// it has a session to build them from ([`crate::window::Window::new`]).
    pub fn attach(&self, window: &Window) {
        self.window.replace(Some(window.downgrade()));
        // The pane opens at the width the writer last dragged it to, held to
        // what the shape this window opens at can hold: a width taken down
        // beside a wider monitor is not one this window has room for. A
        // launch of the harness's read no state file and so opens at [`WIDTH`]
        // ([`crate::session::Session::library_width`]).
        if let Some(session) = window.session() {
            self.set_width(library_width(
                session.library_width(),
                u32::try_from(window.default_width()).unwrap_or(u32::MAX),
            ));
        }
        self.watch_divider();
        let activated = self.clone();
        self.list
            .connect_row_activated(move |_, row| activated.activate(row));
        // Esc drops the query and hands the keyboard back to the page, which
        // is where a writer who came to the pane looking for a Document leaves
        // it — the same thing Esc in the field does, because a writer who
        // arrowed down into the hits is in the same search.
        let keys = gtk::EventControllerKey::new();
        let escaped = self.clone();
        keys.connect_key_pressed(move |_, key, _, _| {
            if key == gdk::Key::Escape {
                escaped.clear();
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        self.list.add_controller(keys);
        // A second click on a row renames it where it stands, and the right
        // button opens what can be done to it. Both watch the list in the
        // capture phase, so that the press they take is one the list itself
        // never sees: the first click has already opened the row, and opening
        // it again on the second would put the caret back to the top of a
        // Document the writer is only renaming.
        let doubles = gtk::GestureClick::new();
        doubles.set_button(gdk::BUTTON_PRIMARY);
        doubles.set_propagation_phase(gtk::PropagationPhase::Capture);
        let renaming = self.clone();
        doubles.connect_pressed(move |gesture, presses, _, y| {
            if presses < 2 {
                return;
            }
            gesture.set_state(gtk::EventSequenceState::Claimed);
            renaming.rename_at(y);
        });
        self.list.add_controller(doubles);
        let menued = gtk::GestureClick::new();
        menued.set_button(gdk::BUTTON_SECONDARY);
        menued.set_propagation_phase(gtk::PropagationPhase::Capture);
        let opening = self.clone();
        menued.connect_pressed(move |gesture, _, x, y| {
            gesture.set_state(gtk::EventSequenceState::Claimed);
            opening.menu_at(x, y);
        });
        self.list.add_controller(menued);
        // The pane's head is the one Location's header where the Library has
        // only the one, so the right button offers there what a section head
        // offers ([`Sidebar::head_as_location`]).
        let heading = gtk::GestureClick::new();
        heading.set_button(gdk::BUTTON_SECONDARY);
        let removing = self.clone();
        heading.connect_pressed(move |gesture, _, x, y| {
            let Some(location) = removing.header.borrow().clone() else {
                return;
            };
            gesture.set_state(gtk::EventSequenceState::Claimed);
            removing.popup(&location_menu(&location), &removing.head_menu, x, y);
        });
        self.head.add_controller(heading);
        self.install_row_actions();
        // The two words of "Changed on disk · Reload · Keep": each opens the
        // diff view of what it would do, in the window this pane belongs to.
        let reloading = self.clone();
        self.reload.connect_clicked(move |_| {
            if let Some(window) = reloading.owner() {
                window.reload_from_disk();
            }
        });
        let keeping = self.clone();
        self.keep.connect_clicked(move |_| {
            if let Some(window) = keeping.owner() {
                window.keep_over_disk();
            }
        });
        self.refresh();
    }

    /// The divider's drag and its double-click.
    ///
    /// A drag moves the pane's right edge and every window's page follows it
    /// live; the width is written to the state file as the drag ends, so a
    /// Quill that never shuts down cleanly still opens at the width the
    /// writer chose, which is why [`crate::session::Session::store_settings`]
    /// writes as a key is pressed. A double-click puts the pane back to
    /// [`WIDTH`].
    fn watch_divider(&self) {
        let grab = Rc::new(Cell::new(Grab {
            width: WIDTH,
            at: 0.0,
        }));
        // One gesture for both, and a `GtkGestureClick` rather than a
        // `GtkGestureDrag`: a drag gesture claims the press it begins on, and
        // a click gesture beside it then never sees a second press to count
        // (a double-click on the divider did nothing at all, by hand, until
        // the two were one). The press records where the pane began, every
        // move of the held pointer sets the width, and the release writes it.
        let clicks = gtk::GestureClick::new();
        clicks.set_button(gdk::BUTTON_PRIMARY);
        let pressed = self.clone();
        let taken = Rc::clone(&grab);
        clicks.connect_pressed(move |_, presses, x, _| {
            let Some(at) = pressed.pointer(x) else {
                return;
            };
            if presses >= 2 {
                // The second press of a double-click also anchors the drag it
                // is: the release that ends it then leaves the pane where the
                // double-click put it rather than where it was dragged to.
                taken.set(Grab { width: WIDTH, at });
                pressed.resize(WIDTH);
                pressed.store_width();
                return;
            }
            taken.set(Grab {
                width: pressed.root.width(),
                at,
            });
        });
        let moved = self.clone();
        let from = Rc::clone(&grab);
        clicks.connect_update(move |gesture, sequence| {
            if let Some((x, _)) = gesture.point(sequence) {
                moved.dragged(from.get(), x);
            }
        });
        // Letting go writes the width the moves arrived at and sets nothing
        // itself: a press and a release of the same click report the pointer
        // half a pixel apart under a scaled output, and a divider that
        // answered the release would shave a pixel off the pane every time it
        // was clicked.
        let ended = self.clone();
        clicks.connect_released(move |_, _, _, _| ended.store_width());
        self.divider.add_controller(clicks);
    }

    /// Where the pointer is now in the window's own pixels, given where it is
    /// in the divider's.
    ///
    /// The divider travels with the edge it is: an offset read off its own
    /// coordinates counts every pixel the pane has already grown by a second
    /// time, and the pane runs away from the pointer.
    fn pointer(&self, at: f64) -> Option<f64> {
        let window = self.owner()?;
        let point = self.divider.compute_point(&window, &place(at, 0.0))?;
        Some(f64::from(point.x()))
    }

    /// The pane's width part-way through a drag: what it was when the drag
    /// began, plus how far the pointer has travelled since.
    fn dragged(&self, grab: Grab, at: f64) {
        let Some(now) = self.pointer(at) else {
            return;
        };
        self.resize(grab.width + pixels(now - grab.at));
    }

    /// Stands every window's pane at `width` ([`Window::resize_library`]).
    fn resize(&self, width: i32) {
        if let Some(window) = self.owner() {
            window.resize_library(width);
        }
    }

    /// Writes the width the drag left the pane at to the state file.
    fn store_width(&self) {
        if let Some(session) = self.session() {
            session.store_state();
        }
    }

    /// Whether the pane is showing.
    #[must_use]
    pub fn is_shown(&self) -> bool {
        self.frame.is_visible()
    }

    /// Shows or hides the pane. The page keeps its own centring and is simply
    /// given a narrower window, as the oracle's is.
    pub fn set_shown(&self, shown: bool) {
        self.frame.set_visible(shown);
    }

    /// Stands the pane at `width` logical pixels.
    ///
    /// The width the drag arrived at, already pulled into range by the window
    /// that took the drag ([`crate::window::Window::resize_library`]); the
    /// pane asks for it and the page takes what is left, which is how the
    /// pane has always been sized.
    pub fn set_width(&self, width: u32) {
        self.root
            .set_width_request(i32::try_from(width).unwrap_or(WIDTH));
    }

    /// Puts the keyboard in the search field: the second half of
    /// `library.search`, whose first half is the window standing the pane
    /// beside the page ([`crate::window::Window::search_library`]).
    pub fn focus_search(&self) {
        self.entry.grab_focus();
    }

    /// Puts `query` in the search field and narrows the list to it, as
    /// `--search` does.
    ///
    /// Searched at once rather than after [`SETTLE_MS`]: a harness launch
    /// shoots its first frame, and what the flag asks for is a window with the
    /// results in it.
    pub fn set_query(&self, query: &str) {
        self.entry.set_text(query);
        self.entry.set_position(-1);
        self.search_now();
    }

    /// What the status line says ([`crate::files::said`]).
    pub fn set_status(&self, said: &str) {
        self.status.set_text(said);
    }

    /// Whether the status line offers Reload and Keep beside what it says.
    ///
    /// The slack in the foot goes to whichever of the two is expanding, so
    /// that the words stand beside the line they belong to and where the
    /// manuscripts are stays at the pane's edge: the label holds the slack at
    /// rest, and the offer holds it while there is a conflict.
    pub fn set_offer(&self, offered: bool) {
        self.status.set_hexpand(!offered);
        self.offer.set_hexpand(offered);
        self.offer.set_visible(offered);
    }

    /// Whether the Document the window holds is changed on disk, which puts a
    /// dot on its row.
    pub fn set_conflicted(&self, conflicted: bool) {
        self.conflicted.set(conflicted);
        self.highlight();
    }

    /// The Document the window is showing, whose row is highlighted.
    pub fn set_open(&self, path: Option<&Path>) {
        self.open.replace(path.map(Path::to_path_buf));
        self.highlight();
    }

    /// What the tree is read through: the writer's `[library] show_hidden`
    /// and the sort this pane stands at. The Palette's Outline reads the
    /// Library through the same view, so its Documents fall in the order the
    /// sidebar shows them (#397).
    pub(crate) fn view(&self, session: &crate::session::Session) -> View {
        View {
            show_hidden: session.settings().library.show_hidden,
            sort: self.sort.get(),
        }
    }

    /// Draws the Library as it is now: every section, in order, from the tree
    /// the session holds.
    ///
    /// The whole list is built rather than patched, because a watch event can
    /// have moved a row from one folder to another and the pane is bounded by
    /// what a writer can see at once, not by the tree's size.
    pub fn refresh(&self) {
        let Some(session) = self.session() else {
            return;
        };
        while let Some(child) = self.list.first_child() {
            self.list.remove(&child);
        }
        self.rows.borrow_mut().clear();
        self.heads.borrow_mut().clear();
        let library = session.library();
        let view = self.view(&session);
        let now = glib::DateTime::now_local().ok();
        self.read_heads(library.files(&view));
        let read = self.read.borrow();
        let ground = session.ground();
        let drawing = Drawing {
            extensions: session.settings().library.show_extensions,
            now: now.as_ref(),
            read: &read,
            mark: Colour::over(
                ground.colours.colour(Role::Accent),
                ground.colours.colour(Role::Paper),
                MARK_TINT,
            ),
            marked: ground.colours.colour(Role::Ink),
        };
        let shown = library.shown(&view);
        let pinned = library.pinned_rows(&view);
        // One Location and nothing pinned is one thing to name, and the head
        // names it: a pane called Library over a section called library says
        // the same word twice. Anything else is the pane over its parts.
        let parts = !pinned.is_empty() || library.locations().len() > 1;
        let alone = (!parts).then(|| shown.first()).flatten();
        self.title
            .set_text(&alone.map_or_else(|| TITLE.to_string(), section_name));
        // No section head is drawn for it, so the pane's head is its header:
        // what it is called, what its menu offers, and what a row let go over
        // it moves into (#246, stories 3 and 38).
        self.header
            .replace(alone.map(|section| section.path.to_path_buf()));
        self.head_as_location();
        let query = self.query();
        if query.is_empty() {
            self.tree(&shown, &pinned, parts, &drawing);
            let words = read.values().map(|head| head.words).sum();
            self.count.set_text(&counted(read.len(), words));
        } else {
            self.results(&query, &library, &view, &drawing);
        }
        self.highlight();
    }

    /// Reads the head of every shown file, keeping the ones already read.
    ///
    /// A row shows two lines of what its file says and the count adds up their
    /// words, so every shown file has to be read; a file whose write time has
    /// not moved since the last refresh is not read again, which is the shape
    /// [`quill_engine::library::Contents`] gives search. The bound is one read
    /// of at most [`EXCERPT_BYTES`] per shown file that has been written since
    /// the pane last drew, and what is held is one head per shown file: the
    /// map is built again from the files the view shows, so a file that has
    /// left the Library leaves the map with it.
    fn read_heads<'a>(&self, files: impl Iterator<Item = &'a File>) {
        let mut read = self.read.borrow_mut();
        let mut before = std::mem::take(&mut *read);
        for file in files {
            let head = match before.remove(file.path()) {
                Some(head) if head.modified == file.modified() => head,
                _ => Head::of(file.path(), file.modified()),
            };
            read.insert(file.path().to_path_buf(), head);
        }
    }

    /// The Library as it stands: the Pinned section, then one section per
    /// Location, or the one Location's rows alone under the pane's own head.
    fn tree(&self, shown: &[Section<'_>], pinned: &[Row<'_>], parts: bool, drawing: &Drawing<'_>) {
        let mut sections = 0;
        if !pinned.is_empty() {
            let first = self.rows.borrow().len();
            let head = self.section(None, "Pinned", sections, pinned, drawing);
            // One target, however many rows it draws: the whole section lights
            // and every row of it takes the drop (#246 story 11).
            let mut group: Vec<gtk::Widget> = vec![head.upcast()];
            group.extend(
                self.drawn_since(first)
                    .into_iter()
                    .map(|listed| listed.row.upcast()),
            );
            let group = Rc::new(group);
            let over = Rc::new(Cell::new(0));
            for row in group.iter() {
                self.drop_onto(row, &Onto::Pinned, &group, &over);
            }
            sections += 1;
        }
        for section in shown {
            let first = self.rows.borrow().len();
            if parts {
                let head = self.section(
                    Some(section.path),
                    &section_name(section),
                    sections,
                    &section.rows,
                    drawing,
                );
                self.folder_target(&head, section.path);
            } else {
                self.rows_of(&section.rows, drawing);
            }
            for listed in self.drawn_since(first) {
                if listed.folder {
                    self.folder_target(&listed.row, &listed.path);
                }
            }
            sections += 1;
        }
    }

    /// The rows drawn since the list held `first` of them: the section just
    /// built, in the order it was built.
    fn drawn_since(&self, first: usize) -> Vec<Listed> {
        self.rows.borrow()[first..].to_vec()
    }

    /// A folder's row, a Location's head or the pane's own head as a place a
    /// dragged row can be moved into, lighting alone.
    fn folder_target(&self, on: &impl IsA<gtk::Widget>, folder: &Path) -> gtk::DropTarget {
        let lit = Rc::new(vec![on.clone().upcast()]);
        let over = Rc::new(Cell::new(0));
        self.drop_onto(on, &Onto::Folder(folder.to_path_buf()), &lit, &over)
    }

    /// Puts `onto` under `row`: what letting a dragged row go there does, and
    /// which rows light while the pointer is over it.
    ///
    /// `lit` is every row of the target — the Pinned section is one target
    /// however many rows it draws — and `over` counts how many of them the
    /// pointer is inside, so that crossing from one row of a section to the
    /// next never puts the light out. A drop the Library cannot do is refused
    /// while the drag is still in the air, which is what preloading the value
    /// is for: without it the path is unreadable until the writer has let go.
    fn drop_onto(
        &self,
        on: &impl IsA<gtk::Widget>,
        onto: &Onto,
        lit: &Rc<Vec<gtk::Widget>>,
        over: &Rc<Cell<usize>>,
    ) -> gtk::DropTarget {
        let target = gtk::DropTarget::new(glib::types::Type::STRING, gdk::DragAction::MOVE);
        target.set_preload(true);

        let lighting = Rc::clone(lit);
        let light = move |allowed: bool| {
            for row in lighting.iter() {
                if allowed {
                    row.add_css_class(DROP_CLASS);
                } else {
                    row.remove_css_class(DROP_CLASS);
                }
            }
        };

        // Entering and moving both answer, because a preloaded value arrives
        // when the drag's own read of it finishes and can still be unread at
        // the first `enter`; the answers agree, so the later one is the same
        // light rather than a second one.
        let pane = self.clone();
        let asked = onto.clone();
        let entering = light.clone();
        let counted = Rc::clone(over);
        target.connect_enter(move |target, _, _| {
            counted.set(counted.get() + 1);
            let allowed = pane.would(target, &asked).is_some();
            entering(allowed);
            action(allowed)
        });

        let pane = self.clone();
        let asked = onto.clone();
        let moving = light.clone();
        target.connect_motion(move |target, _, _| {
            let allowed = pane.would(target, &asked).is_some();
            moving(allowed);
            action(allowed)
        });

        let leaving = light.clone();
        let counted = Rc::clone(over);
        target.connect_leave(move |_| {
            let left = counted.get().saturating_sub(1);
            counted.set(left);
            if left == 0 {
                leaving(false);
            }
        });

        let pane = self.clone();
        let asked = onto.clone();
        let counted = Rc::clone(over);
        target.connect_drop(move |_, value, _, _| {
            counted.set(0);
            light(false);
            let Ok(from) = value.get::<String>() else {
                return false;
            };
            let from = PathBuf::from(from);
            let Some(done) = files::dropped(&from, &asked, &pane.pinned_now()) else {
                return false;
            };
            pane.let_go(&from, &done);
            true
        });
        on.add_controller(target.clone());
        target
    }

    /// What letting the row `target` is carrying go over `onto` would do, and
    /// `None` where it would do nothing.
    fn would(&self, target: &gtk::DropTarget, onto: &Onto) -> Option<Dropped> {
        let carried = target.value()?.get::<String>().ok()?;
        files::dropped(Path::new(&carried), onto, &self.pinned_now())
    }

    /// The Pinned list as it stands, which is what a drop over the Pinned
    /// section is answered against.
    fn pinned_now(&self) -> Vec<PathBuf> {
        self.session()
            .map_or_else(Vec::new, |session| session.library().pinned().to_vec())
    }

    /// Does what the drop decided, which is the window's to do: a pin goes
    /// through the settings and a move through the engine, each after asking
    /// whatever it has to ask.
    fn let_go(&self, from: &Path, done: &Dropped) {
        let Some(window) = self.owner() else {
            return;
        };
        match done {
            Dropped::Pin => window.set_pinned(from, true),
            Dropped::Into(folder) => window.move_path(from, folder),
        }
    }

    /// Makes `row` draggable, carrying `path` as the plain text of it.
    ///
    /// A string rather than a type of Quill's own, because the drag never
    /// leaves this pane and a path is what both targets want; the icon under
    /// the pointer is the row itself, so what is being dragged is what was
    /// grabbed.
    fn drag_from(&self, row: &gtk::ListBoxRow, path: &Path) {
        let source = gtk::DragSource::new();
        source.set_actions(gdk::DragAction::MOVE);
        let carried = path.to_string_lossy().into_owned();
        source.connect_prepare(move |_, _, _| {
            Some(gdk::ContentProvider::for_value(&carried.to_value()))
        });
        let dragged = row.clone();
        source.connect_drag_begin(move |source, _| {
            source.set_icon(Some(&gtk::WidgetPaintable::new(Some(&dragged))), 0, 0);
        });
        row.add_controller(source);
    }

    /// The list narrowed to what `query` found, in the engine's order: the
    /// files whose names matched, then the files whose texts did, each of
    /// those with its snippet and the match marked in it.
    ///
    /// Flat, with no sections and no folders: what a writer asked for is the
    /// Documents that answer the query, and where each one lies is the tree's
    /// answer to a different question.
    fn results(&self, query: &str, library: &Library, view: &View, drawing: &Drawing<'_>) {
        let mut contents = self.contents.borrow_mut();
        let found = library.search(query, view, &mut contents);
        self.count
            .set_text(&narrowed(found.len(), drawing.read.len()));
        for hit in &found {
            let file = hit.file();
            let listed = self.file_row(
                &FileRow {
                    name: file.name(),
                    path: file.path(),
                    depth: 0,
                    modified: file.modified(),
                    snippet: hit.snippet(),
                },
                drawing,
            );
            self.list.append(&listed.row);
            self.rows.borrow_mut().push(listed);
        }
    }

    /// One section: its head, then the rows of it that are not inside a closed
    /// folder. The head is answered, because it is what a drop over the section
    /// lands on ([`Sidebar::drop_onto`]).
    ///
    /// A Location's head is also what folds its tree away: a click on it puts
    /// the Location in [`Sidebar::folded`] and the section draws its head
    /// alone, and the next click brings the tree back. The head says which way
    /// it stands by what is under it and by nothing drawn on the head itself,
    /// because the head's pixels are the judged `library` state's. The pane's
    /// own head is not one of these: where it is heading the one Location
    /// ([`Sidebar::head_as_location`]) it carries the buttons that shut the
    /// pane and start a Document, and a click on it is one of those.
    fn section(
        &self,
        root: Option<&Path>,
        name: &str,
        above: usize,
        rows: &[Row<'_>],
        drawing: &Drawing<'_>,
    ) -> gtk::ListBoxRow {
        let head = gtk::ListBoxRow::new();
        head.set_selectable(false);
        head.set_activatable(false);
        head.set_child(Some(&section_head(name, root.is_some())));
        if above > 0 {
            head.set_margin_top(SECTION_AIR);
        }
        if let Some(root) = root {
            self.heads
                .borrow_mut()
                .push((head.clone(), root.to_path_buf()));
            let folding = gtk::GestureClick::new();
            folding.set_button(gdk::BUTTON_PRIMARY);
            let location = root.to_path_buf();
            let pane = self.clone();
            folding.connect_pressed(move |_, _, _, _| pane.fold(&location));
            head.add_controller(folding);
        }
        self.list.append(&head);
        if !root.is_some_and(|root| self.folded.borrow().contains(root)) {
            self.rows_of(rows, drawing);
        }
        head
    }

    /// A click on a Location's head: its tree folds away, or comes back.
    fn fold(&self, location: &Path) {
        files::flipped(&mut self.folded.borrow_mut(), location);
        self.refresh();
    }

    /// The rows of one section, in the order the tree hands them over, less
    /// whatever is inside a folder that is closed.
    fn rows_of(&self, rows: &[Row<'_>], drawing: &Drawing<'_>) {
        // A closed folder takes its subtree with it: the tree arrives
        // flattened, deepest last, so everything below a closed folder is
        // everything after it that is deeper than it is.
        let mut closed: Option<usize> = None;
        for (at, row) in rows.iter().enumerate() {
            if closed.is_some_and(|depth| row.depth() > depth) {
                continue;
            }
            closed = None;
            let open = self.expanded.borrow().contains(row.path());
            let listed = match row {
                Row::Folder { folder, depth } => {
                    if !open {
                        closed = Some(*depth);
                    }
                    self.folder_row(folder.name(), row.path(), *depth, open, held(rows, at))
                }
                Row::File { file, depth } => self.file_row(
                    &FileRow {
                        name: file.name(),
                        path: row.path(),
                        depth: *depth,
                        modified: file.modified(),
                        snippet: None,
                    },
                    drawing,
                ),
            };
            self.list.append(&listed.row);
            self.rows.borrow_mut().push(listed);
        }
    }

    /// A folder: its name, how many entries it holds, and a chevron that says
    /// whether it is open.
    fn folder_row(&self, name: &str, path: &Path, depth: usize, open: bool, held: usize) -> Listed {
        let line = gtk::Box::new(gtk::Orientation::Horizontal, ICON_GAP);
        line.set_margin_start(ROW_LEFT + INDENT * i32::try_from(depth).unwrap_or(0));
        line.set_margin_end(ROW_RIGHT - SORT_BUTTON_PAD);
        line.set_margin_top(ROW_TOP);
        line.set_margin_bottom(FOLDER_BOTTOM);
        line.append(&column(icon(FOLDER, folder_icon), "lib-folder-icon"));
        let label = gtk::Label::new(Some(name));
        label.add_css_class("lib-head");
        label.set_hexpand(true);
        label.set_xalign(0.0);
        label.set_ellipsize(gtk::pango::EllipsizeMode::End);
        line.append(&label);
        let count = gtk::Label::new(Some(&held.to_string()));
        count.add_css_class("lib-meta");
        line.append(&count);
        let chevron = icon((CHEV, CHEV), move |area, cr| chevron_icon(area, cr, open));
        chevron.add_css_class("lib-icon");
        line.append(&chevron);
        self.listed(line, &label, path, true)
    }

    /// A file: its name, when it was last written, and two lines of what it
    /// says — its own beginning, or the snippet a query found in it.
    fn file_row(&self, row: &FileRow<'_>, drawing: &Drawing<'_>) -> Listed {
        let line = gtk::Box::new(gtk::Orientation::Horizontal, ICON_GAP);
        line.set_margin_start(ROW_LEFT + INDENT * i32::try_from(row.depth).unwrap_or(0));
        line.set_margin_end(ROW_RIGHT);
        line.set_margin_top(ROW_TOP);
        line.set_margin_bottom(ROW_BOTTOM);
        let mark = icon(DOC, document_icon);
        mark.set_valign(gtk::Align::Start);
        line.append(&column(mark, "lib-icon"));

        let body = gtk::Box::new(gtk::Orientation::Vertical, 0);
        body.set_hexpand(true);
        let top = gtk::Box::new(gtk::Orientation::Horizontal, ICON_GAP);
        let title = gtk::Label::new(Some(&row_name(row.name, drawing.extensions)));
        title.add_css_class("lib-name");
        title.set_hexpand(true);
        title.set_xalign(0.0);
        title.set_ellipsize(gtk::pango::EllipsizeMode::End);
        top.append(&title);
        if let (Some(modified), Some(now)) = (row.modified, drawing.now) {
            let date = gtk::Label::new(Some(&stamp(modified, now)));
            date.add_css_class("lib-date");
            top.append(&date);
        }
        body.append(&top);
        let said = match row.snippet {
            Some(snippet) => snippet.text(),
            None => drawing
                .read
                .get(row.path)
                .map_or("", |head| head.excerpt.as_str()),
        };
        if !said.is_empty() {
            let excerpt = gtk::Label::new(Some(said));
            excerpt.add_css_class("lib-excerpt");
            excerpt.set_xalign(0.0);
            excerpt.set_wrap(true);
            excerpt.set_wrap_mode(gtk::pango::WrapMode::WordChar);
            excerpt.set_attributes(Some(&marks(row.snippet, drawing)));
            excerpt.set_lines(EXCERPT_LINES);
            excerpt.set_ellipsize(gtk::pango::EllipsizeMode::End);
            body.append(&excerpt);
        }
        line.append(&body);
        self.listed(line, &title, row.path, false)
    }

    /// A row of the list: the hairline above it, the accent bar down its left
    /// edge, and the line itself.
    fn listed(&self, line: gtk::Box, name: &gtk::Label, path: &Path, folder: bool) -> Listed {
        // At the row's right edge, inside the row's own margin, and hidden
        // until the window says this Document is the one in a conflict.
        let dot = icon((DOT, DOT), warned_dot);
        dot.add_css_class("lib-changed");
        dot.set_valign(gtk::Align::Start);
        dot.set_margin_top(DOT_TOP);
        dot.set_visible(false);
        line.append(&dot);
        let bar = gtk::Box::new(gtk::Orientation::Vertical, 0);
        bar.add_css_class("lib-bar");
        bar.set_halign(gtk::Align::Start);
        // Over the row rather than beside it, as `.lib-row.file.sel::after`
        // is: a bar that takes a column of its own pushes the icon and the
        // name eight pixels right of the padding they are set from, which is
        // how the pane came to have four left edges (#254, [`PANE_LEFT`]).
        let beside = gtk::Overlay::new();
        beside.set_child(Some(&line));
        beside.add_overlay(&bar);
        let stacked = gtk::Box::new(gtk::Orientation::Vertical, 0);
        // The hairline starts under the name rather than at the pane's edge,
        // as iA's does (`.lib-row + .lib-row::before { left: 46px; right: 14px }`),
        // and is left off the first row of a section.
        let rule = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        rule.add_css_class("lib-rule");
        rule.set_height_request(1);
        rule.set_margin_start(ROW_LEFT + ICON_COLUMN + ICON_GAP);
        rule.set_margin_end(ROW_RIGHT);
        rule.set_visible(!self.at_section_head());
        stacked.append(&rule);
        stacked.append(&beside);
        let row = gtk::ListBoxRow::new();
        row.set_child(Some(&stacked));
        // Every row of the pane can be dragged: a file into a folder or onto
        // Pinned, a folder either way too.
        self.drag_from(&row, path);
        Listed {
            row,
            path: path.to_path_buf(),
            folder,
            name: name.clone(),
            dot,
        }
    }

    /// Whether the row now being built is the first of its section, and so
    /// draws no hairline above it.
    ///
    /// A list with nothing in it yet counts: the first row of a result list
    /// stands under the sort row, whose own border is already the line
    /// between them.
    fn at_section_head(&self) -> bool {
        match self.list.last_child().and_downcast::<gtk::ListBoxRow>() {
            Some(row) => !row.is_selectable(),
            None => true,
        }
    }

    /// Highlights the row of the Document the window is showing, and no row at
    /// all where it is showing something the Library does not hold; the dot
    /// goes on that same row while that Document is in a conflict.
    ///
    /// Under a query, a result list with the open Document nowhere in it
    /// highlights its first hit instead, because the highlight is what Enter
    /// opens. One walk of the rows now drawn, which is what the pane shows
    /// and not what the tree holds.
    fn highlight(&self) {
        let open = self.open.borrow();
        let conflicted = self.conflicted.get();
        let rows = self.rows.borrow();
        for listed in rows.iter() {
            let is_open = open.as_deref() == Some(listed.path.as_path());
            listed.dot.set_visible(is_open && conflicted);
        }
        let found = rows
            .iter()
            .find(|listed| open.as_deref() == Some(listed.path.as_path()))
            .or_else(|| {
                (!self.query().is_empty()).then(|| rows.iter().find(|listed| !listed.folder))?
            })
            .map(|listed| listed.row.clone());
        self.list.select_row(found.as_ref());
    }

    /// A row was clicked, or Enter was pressed on it: a folder opens or
    /// closes, a file opens in this window.
    fn activate(&self, row: &gtk::ListBoxRow) {
        let found = {
            let rows = self.rows.borrow();
            rows.iter()
                .find(|listed| listed.row == *row)
                .map(|listed| (listed.path.clone(), listed.folder))
        };
        let Some((path, folder)) = found else {
            return;
        };
        if folder {
            files::flipped(&mut self.expanded.borrow_mut(), &path);
            // The refresh builds every row again, so the row that was pressed
            // is a new widget and the keyboard would be left on the list with
            // nothing under it: the arrows go on from where the writer is.
            self.refresh();
            self.focus_row(&path);
            return;
        }
        if let Some(window) = self.owner() {
            window.open_path(&path);
        }
    }

    /// Puts the selection and the keyboard on the row at `path`, where the
    /// list still draws one.
    fn focus_row(&self, path: &Path) {
        let found = self
            .rows
            .borrow()
            .iter()
            .find(|listed| listed.path == path)
            .map(|listed| listed.row.clone());
        if let Some(row) = found {
            self.list.select_row(Some(&row));
            row.grab_focus();
        }
    }

    // ------------------------------------------------------ row operations

    /// The path of the row the pane has selected, whether file or folder.
    #[must_use]
    pub fn selected_path(&self) -> Option<PathBuf> {
        let row = self.list.selected_row()?;
        self.listed_at(&row).map(|(path, _)| path)
    }

    /// The file the selected row is, and `None` where it is a folder or there
    /// is no selected row: what a row operation acts on
    /// (`Window::target`).
    #[must_use]
    pub fn selected_file(&self) -> Option<PathBuf> {
        let row = self.list.selected_row()?;
        let (path, folder) = self.listed_at(&row)?;
        (!folder).then_some(path)
    }

    /// The folder a Document started from this pane goes into: the selected
    /// folder row itself, or the folder the selected file stands in.
    #[must_use]
    pub fn selected_folder(&self) -> Option<PathBuf> {
        let row = self.list.selected_row()?;
        let (path, folder) = self.listed_at(&row)?;
        if folder {
            Some(path)
        } else {
            path.parent().map(Path::to_path_buf)
        }
    }

    /// The files the pane is showing, top to bottom, which is the list
    /// `file.next` and `file.prev` walk ([`crate::files::stepped`]).
    ///
    /// The rows now drawn and not the tree: a file inside a closed folder is
    /// not a row a writer can step to, and under a query the list is the hits.
    #[must_use]
    pub fn listed_files(&self) -> Vec<PathBuf> {
        self.rows
            .borrow()
            .iter()
            .filter(|listed| !listed.folder)
            .map(|listed| listed.path.clone())
            .collect()
    }

    /// Puts a field in the place of the selected row's name, and answers
    /// whether there was a file row to put one in.
    ///
    /// `false` is what sends `file.rename` to the dialog instead
    /// (`Window::rename_document`): a folder row, or no row at all.
    pub fn start_rename(&self) -> bool {
        let Some(row) = self.list.selected_row() else {
            return false;
        };
        self.rename_row(&row)
    }

    /// A double click at `y`: that row is selected and its name becomes a
    /// field.
    fn rename_at(&self, y: f64) {
        let Some(row) = self.list.row_at_y(pixels(y)) else {
            return;
        };
        if row.is_selectable() {
            self.list.select_row(Some(&row));
        }
        self.rename_row(&row);
    }

    /// Puts a field in the place of `row`'s name, and answers whether it took.
    ///
    /// The oracle's field (`legacy/app/js/files.js` `startRename`): the name
    /// as it stands with everything before the extension selected, Enter
    /// renaming, Esc leaving it, and clicking away renaming — because a writer
    /// who typed a name and looked elsewhere meant the name.
    fn rename_row(&self, row: &gtk::ListBoxRow) -> bool {
        let found = {
            let rows = self.rows.borrow();
            rows.iter()
                .find(|listed| listed.row == *row)
                .filter(|listed| !listed.folder)
                .map(|listed| (listed.path.clone(), listed.name.clone()))
        };
        let Some((path, label)) = found else {
            return false;
        };
        if !label.is_visible() {
            // Already being renamed: the field is standing where the label is.
            return true;
        }
        let Some(beside) = label.parent().and_downcast::<gtk::Box>() else {
            return false;
        };
        let name = full_name(&path);
        let entry = gtk::Entry::builder()
            .text(&name)
            .hexpand(true)
            .has_frame(false)
            .build();
        entry.add_css_class("lib-rename");
        beside.insert_child_after(&entry, Some(&label));
        label.set_visible(false);
        entry.select_region(0, files::stem_chars(&name));
        entry.grab_focus();

        // One rename, however many of the three ways of ending it fire: taking
        // the field down moves the keyboard, which is itself one of them.
        let done = Rc::new(Cell::new(false));
        let ending = self.clone();
        let (ended, asked, over) = (done.clone(), path.clone(), label.clone());
        entry.connect_activate(move |entry| {
            ending.end_rename(entry, &over, &asked, &ended, true);
        });
        let escaping = self.clone();
        let (ended, asked, over) = (done.clone(), path.clone(), label.clone());
        let keys = gtk::EventControllerKey::new();
        keys.connect_key_pressed(glib::clone!(
            #[weak]
            entry,
            #[upgrade_or]
            glib::Propagation::Proceed,
            move |_, key, _, _| {
                if key == gdk::Key::Escape {
                    escaping.end_rename(&entry, &over, &asked, &ended, false);
                    return glib::Propagation::Stop;
                }
                glib::Propagation::Proceed
            }
        ));
        entry.add_controller(keys);
        let leaving = self.clone();
        let focus = gtk::EventControllerFocus::new();
        focus.connect_leave(glib::clone!(
            #[weak]
            entry,
            move |_| leaving.end_rename(&entry, &label, &path, &done, true)
        ));
        entry.add_controller(focus);
        true
    }

    /// Ends a rename: the field goes, the name comes back, and where the
    /// writer meant it the window renames the file.
    fn end_rename(
        &self,
        entry: &gtk::Entry,
        label: &gtk::Label,
        path: &Path,
        done: &Rc<Cell<bool>>,
        commit: bool,
    ) {
        if done.replace(true) {
            return;
        }
        let typed = entry.text().trim().to_string();
        if let Some(beside) = entry.parent().and_downcast::<gtk::Box>() {
            beside.remove(entry);
        }
        label.set_visible(true);
        if commit
            && !typed.is_empty()
            && let Some(window) = self.owner()
        {
            window.rename_path(path, &typed);
        }
    }

    /// The right button at (`x`, `y`): the row under the pointer is selected
    /// and the menu of what can be done to it opens where the pointer is.
    ///
    /// A section head has a menu of its own — a Location is dropped from the
    /// Library, and nothing on disk is touched — and the Pinned head, which
    /// names no Location, has none.
    fn menu_at(&self, x: f64, y: f64) {
        let Some(row) = self.list.row_at_y(pixels(y)) else {
            return;
        };
        let model = if let Some(location) = self.head_at(&row) {
            location_menu(&location)
        } else {
            let Some((path, folder)) = self.listed_at(&row) else {
                return;
            };
            self.list.select_row(Some(&row));
            row_menu(folder, self.is_pinned(&path))
        };
        // The gesture counts from the list's top left and the menu stands on
        // the pane, which the list is scrolled inside.
        let on_pane = self.list.compute_point(&self.root, &place(x, y));
        let (x, y) = on_pane.map_or((x, y), |point| (f64::from(point.x()), f64::from(point.y())));
        self.popup(&model, &self.menu, x, y);
    }

    /// Stands `model` in `over` at (`x`, `y`) of the widget `over` is parented
    /// on, as the menu of what can be done there.
    fn popup(&self, model: &gio::Menu, over: &gtk::PopoverMenu, x: f64, y: f64) {
        over.set_menu_model(Some(model));
        over.set_pointing_to(Some(&gdk::Rectangle::new(pixels(x), pixels(y), 1, 1)));
        over.popup();
    }

    /// Takes the two menus off the widgets they are parented on, for the
    /// window's disposal: a popover is a child GTK does not take down with its
    /// parent ([`crate::chrome::Bars::dispose`]).
    pub fn dispose(&self) {
        self.menu.unparent();
        self.head_menu.unparent();
    }

    /// Puts the one Location's header on the pane's own head, or takes it off
    /// again.
    ///
    /// A Library of one Location with nothing Pinned draws no section head —
    /// the pane's head is already saying that Location's name — so the head is
    /// where Remove from Library and a drop on to the Location's top level
    /// have to live, or neither is reachable at all. The target goes on and
    /// comes off as the Library changes, because one left behind would answer
    /// for a Location the pane has stopped showing.
    fn head_as_location(&self) {
        if let Some(target) = self.head_drop.take() {
            self.head.remove_controller(&target);
        }
        let Some(location) = self.header.borrow().clone() else {
            return;
        };
        self.head_drop
            .replace(Some(self.folder_target(&self.head, &location)));
    }

    /// The actions a row menu fires that no Command covers: opening the row
    /// under the pointer rather than the Document the window holds, pinning it,
    /// and dropping a Location.
    ///
    /// A `row.` group of the pane's own, beside the window's `win.file.*`,
    /// which the same menu fires for Rename, Duplicate and Move to Trash.
    fn install_row_actions(&self) {
        let actions = gio::SimpleActionGroup::new();
        let opened = gio::SimpleAction::new("open", None);
        let opening = self.clone();
        opened.connect_activate(move |_, _| opening.open_selected());
        actions.add_action(&opened);
        for (name, pinned) in [("pin", true), ("unpin", false)] {
            let action = gio::SimpleAction::new(name, None);
            let pinning = self.clone();
            action.connect_activate(move |_, _| pinning.set_pinned(pinned));
            actions.add_action(&action);
        }
        // The one action with something to say: which Location. It rides on
        // the menu item rather than in a field the right-click filled in, so
        // that what the row menu fires is all there in the model.
        let removed = gio::SimpleAction::new("remove", Some(glib::VariantTy::STRING));
        let dropping = self.clone();
        removed.connect_activate(move |_, location| {
            if let Some(location) = location.and_then(glib::Variant::str) {
                dropping.drop_location(Path::new(location));
            }
        });
        actions.add_action(&removed);
        self.root.insert_action_group("row", Some(&actions));
    }

    /// `row.open`: the selected row, opened as a click on it would.
    fn open_selected(&self) {
        if let Some(row) = self.list.selected_row() {
            self.activate(&row);
        }
    }

    /// `row.pin` and `row.unpin`, on the selected row.
    fn set_pinned(&self, pinned: bool) {
        let (Some(window), Some(path)) = (self.owner(), self.selected_path()) else {
            return;
        };
        window.set_pinned(&path, pinned);
    }

    /// `row.remove`: the Location the menu item names leaves the Library.
    fn drop_location(&self, location: &Path) {
        if let Some(window) = self.owner() {
            window.drop_location(location);
        }
    }

    /// What `row` stands for — its path, and whether it is a folder — or
    /// `None` where it is a section head.
    fn listed_at(&self, row: &gtk::ListBoxRow) -> Option<(PathBuf, bool)> {
        self.rows
            .borrow()
            .iter()
            .find(|listed| listed.row == *row)
            .map(|listed| (listed.path.clone(), listed.folder))
    }

    /// The Location `row` heads, where it is a Location's head.
    fn head_at(&self, row: &gtk::ListBoxRow) -> Option<PathBuf> {
        self.heads
            .borrow()
            .iter()
            .find(|(head, _)| head == row)
            .map(|(_, root)| root.clone())
    }

    /// Whether `path` is Pinned, which is which half of Pin or Unpin the row's
    /// menu offers.
    fn is_pinned(&self, path: &Path) -> bool {
        self.session().is_some_and(|session| {
            session
                .library()
                .pinned()
                .iter()
                .any(|pinned| pinned == path)
        })
    }

    /// Esc: the keyboard goes back to the page.
    fn leave(&self) {
        if let Some(window) = self.owner() {
            window.focus_editor();
        }
    }

    /// What the field says, with the spaces around it dropped; empty where it
    /// says nothing, which is the pane drawing its tree.
    fn query(&self) -> String {
        self.entry.text().trim().to_string()
    }

    /// Searches once the keystrokes stop, re-arming the wait at each one.
    fn settle(&self) {
        self.wake();
        let searching = self.clone();
        let waiting = glib::timeout_add_local_once(Duration::from_millis(SETTLE_MS), move || {
            searching.settle.replace(None);
            searching.refresh();
        });
        self.settle.replace(Some(waiting));
    }

    /// Draws the list for what the field says now, without waiting the
    /// keystrokes out.
    fn search_now(&self) {
        self.wake();
        self.refresh();
    }

    /// Drops the wait a keystroke armed, so that nothing searches twice.
    fn wake(&self) {
        if let Some(waiting) = self.settle.replace(None) {
            waiting.remove();
        }
    }

    /// Esc in the field or in the list: the query goes, the tree comes back,
    /// and the keyboard returns to the page (#246 story 18).
    fn clear(&self) {
        self.clear_query();
        self.leave();
    }

    /// Drops the query and draws the tree again, answering whether there was a
    /// query to drop.
    ///
    /// The answer is what tells Esc pressed on the page apart from Esc pressed
    /// on nothing, which is what the window listens for ([`crate::window`]): a
    /// writer who searched, opened a hit and is now typing is still in a
    /// search until they say otherwise, and Esc is how they say it.
    pub fn clear_query(&self) -> bool {
        if self.entry.text().is_empty() {
            return false;
        }
        self.entry.set_text("");
        self.search_now();
        true
    }

    /// Enter in the field: the highlighted hit opens, or the first row where
    /// nothing is highlighted.
    fn open_highlighted(&self) {
        let row = self
            .list
            .selected_row()
            .or_else(|| self.list.row_at_index(0));
        if let Some(row) = row {
            self.activate(&row);
        }
    }

    /// Down in the field: the keyboard steps into the list, where the arrows
    /// walk the rows.
    fn step_into_list(&self) {
        if self.list.selected_row().is_none()
            && let Some(first) = self.list.row_at_index(0)
        {
            self.list.select_row(Some(&first));
        }
        self.list.grab_focus();
    }

    /// The next sort order, and the list drawn in it.
    fn cycle_sort(&self) {
        let next = match self.sort.get() {
            Sort::Date => Sort::Name,
            Sort::Name => Sort::Date,
        };
        self.sort.set(next);
        self.sort_label.set_text(sort_title(next));
        self.refresh();
    }

    /// The window this pane belongs to, while it is still open.
    fn owner(&self) -> Option<Window> {
        self.window
            .borrow()
            .as_ref()
            .and_then(glib::WeakRef::upgrade)
    }

    /// The session behind the window, which holds the one Library.
    fn session(&self) -> Option<Rc<crate::session::Session>> {
        self.owner().and_then(|window| window.session())
    }
}

/// What a drop target answers a drag hovering over it with: the move it would
/// do, or nothing at all, which is how GTK is told to refuse the drop.
fn action(allowed: bool) -> gdk::DragAction {
    if allowed {
        gdk::DragAction::MOVE
    } else {
        gdk::DragAction::empty()
    }
}

/// How many entries the folder at `at` holds, as its row says.
///
/// What lies directly inside it and not what lies under that: the tree arrives
/// flattened and deepest last, so its own entries are the rows after it that
/// are one deeper, up to the first that is not deeper at all.
fn held(rows: &[Row<'_>], at: usize) -> usize {
    let depth = rows[at].depth();
    rows[at + 1..]
        .iter()
        .take_while(|row| row.depth() > depth)
        .filter(|row| row.depth() == depth + 1)
        .count()
}

/// A place in a widget, as `graphene` takes it.
///
/// The narrowing is where the pointer is: the pane is [`WIDTH`] wide and a
/// list of a thousand rows is tens of thousands of pixels tall, which an `f32`
/// counts exactly.
fn place(x: f64, y: f64) -> graphene::Point {
    graphene::Point::new(x as f32, y as f32)
}

/// The menu the right button opens on `over`, empty until it is opened on a
/// model.
///
/// Built once and kept, as the bars' menus are ([`crate::chrome::Bars`]):
/// `NESTED` and arrowless for the same reason, and the same `.chrome-menu`
/// look, which the one stylesheet carries
/// ([`crate::chrome::stylesheet`]).
fn menu_popover(over: &impl IsA<gtk::Widget>) -> gtk::PopoverMenu {
    let menu = gtk::PopoverMenu::from_model_full(&gio::Menu::new(), gtk::PopoverMenuFlags::NESTED);
    menu.add_css_class("chrome-menu");
    menu.set_has_arrow(false);
    menu.set_halign(gtk::Align::Start);
    menu.set_parent(over);
    menu
}

/// What a row's context menu offers.
///
/// The oracle's order (`legacy/app/js/files.js` `rowMenu`: Open, Rename…,
/// Duplicate, then Delete under a rule), with Pin or Unpin — whichever the row
/// is not — between them, and the oracle's Download dropped: a file already on
/// disk has nothing to download. A folder is opened by clicking it and has
/// neither a name a rename can give it nor a copy worth making, so all it
/// offers is Pinned.
fn row_menu(folder: bool, pinned: bool) -> gio::Menu {
    let pinning = if pinned {
        ("Unpin", "row.unpin")
    } else {
        ("Pin", "row.pin")
    };
    let menu = gio::Menu::new();
    if folder {
        menu.append(Some(pinning.0), Some(pinning.1));
        return menu;
    }
    let doing = gio::Menu::new();
    doing.append(Some("Open"), Some("row.open"));
    doing.append(Some("Rename…"), Some("win.file.rename"));
    doing.append(Some("Duplicate"), Some("win.file.duplicate"));
    doing.append(Some(pinning.0), Some(pinning.1));
    menu.append_section(None, &doing);
    let going = gio::Menu::new();
    going.append(Some("Move to Trash"), Some("win.file.delete"));
    menu.append_section(None, &going);
    menu
}

/// What a Location's head offers: the two things that are not about a file.
///
/// Adding a Location is `file.openFolder`, the Command the Palette and the
/// Document menu already run, so the head offers the writer looking at their
/// Locations the same one path to another rather than a second of its own;
/// dropping this Location stands under a rule of its own, as Move to Trash
/// does in a row's menu ([`row_menu`]).
fn location_menu(location: &Path) -> gio::Menu {
    let menu = gio::Menu::new();
    let adding = gio::Menu::new();
    adding.append(Some("Add Location…"), Some("win.file.openFolder"));
    menu.append_section(None, &adding);
    let going = gio::Menu::new();
    let row = gio::MenuItem::new(Some("Remove from Library"), None);
    let target = location.to_string_lossy().into_owned();
    row.set_action_and_target_value(Some("row.remove"), Some(&target.to_variant()));
    going.append_item(&row);
    menu.append_section(None, &going);
    menu
}

/// The pane's head: what it is called, the toggle that shuts it, and the
/// button that starts a Document in it.
///
/// Both buttons fire their Commands by name, as the bars' buttons do
/// ([`crate::chrome::Bars`]). Where the head is naming the one Location of the
/// whole Library it is that Location's header too, and carries its menu and
/// its drop target ([`Sidebar::head_location`]).
fn head(title: &gtk::Label) -> gtk::Box {
    let head = gtk::Box::new(gtk::Orientation::Horizontal, HEAD_GAP);
    head.set_height_request(HEAD_HEIGHT);
    head.set_margin_start(HEAD_LEFT);
    head.set_margin_end(HEAD_RIGHT);
    head.append(&button(panel_icon, "win.library.toggle"));
    let name = gtk::Box::new(gtk::Orientation::Horizontal, SECTION_GAP);
    name.set_margin_start(HEAD_GAP);
    name.set_hexpand(true);
    name.append(&column(icon(FOLDER, folder_icon), "lib-icon"));
    title.add_css_class("lib-head");
    title.set_xalign(0.0);
    title.set_ellipsize(gtk::pango::EllipsizeMode::End);
    name.append(title);
    head.append(&name);
    head.append(&button(plus_icon, "win.file.new"));
    head
}

/// A head button: a mark in a 26 px square that fires a Command by name.
fn button(
    draw: impl Fn(&gtk::DrawingArea, &cairo::Context) + 'static,
    action: &'static str,
) -> gtk::Button {
    let mark = icon((MARK, MARK), draw);
    let button = gtk::Button::builder()
        .child(&mark)
        .valign(gtk::Align::Center)
        .can_focus(false)
        .focus_on_click(false)
        .build();
    button.add_css_class("lib-btn");
    button.set_size_request(BUTTON, BUTTON);
    button.connect_clicked(move |button| {
        // A Command not built yet has a disabled action, and GTK answers a
        // disabled action with `false`; that is the click doing nothing.
        let _ = button.activate_action(action, None);
    });
    button
}

/// The search field, under the head.
fn find(entry: &gtk::Entry) -> gtk::Box {
    let field = gtk::Box::new(gtk::Orientation::Horizontal, FIELD_GAP);
    field.set_height_request(FIELD_HEIGHT);
    field.set_margin_start(PANE_LEFT);
    field.set_margin_end(FIELD_PAD);
    field.set_margin_bottom(FIELD_BELOW);
    let mag = icon((MAG, MAG), magnifier_icon);
    mag.add_css_class("lib-icon");
    field.append(&mag);
    field.append(entry);
    field
}

/// The sort row: what the list is ordered by, and how much of it there is.
fn sort_row(button: &gtk::Button, count: &gtk::Label) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    row.add_css_class("lib-sort");
    row.set_height_request(SORT_HEIGHT);
    button.set_margin_start(PANE_LEFT - SORT_BUTTON_PAD);
    button.set_hexpand(true);
    button.set_halign(gtk::Align::Start);
    count.set_margin_end(SORT_RIGHT);
    row.append(button);
    row.append(count);
    row
}

/// The sort control: the order it is in now, and a chevron for the rest.
fn sort_button(label: &gtk::Label) -> gtk::Button {
    let line = gtk::Box::new(gtk::Orientation::Horizontal, 2);
    line.append(label);
    let chevron = icon((CHEV, CHEV), move |area, cr| chevron_icon(area, cr, true));
    line.append(&chevron);
    let button = gtk::Button::builder().child(&line).build();
    button.add_css_class("lib-sortb");
    label.add_css_class("lib-meta");
    button
}

/// One of the two words a conflict offers: a word in the status line that
/// happens to be clicked rather than read, so it is a button with the pane's
/// own flat look on it.
fn offer_word(word: &str) -> gtk::Button {
    let button = gtk::Button::with_label(word);
    button.add_css_class("lib-btn");
    button.add_css_class("lib-offer");
    button
}

/// "· Reload · Keep", hidden until there is a conflict to resolve.
fn offered(reload: &gtk::Button, keep: &gtk::Button) -> gtk::Box {
    let offer = gtk::Box::new(gtk::Orientation::Horizontal, DOT_GAP);
    offer.set_visible(false);
    offer.set_halign(gtk::Align::Start);
    offer.append(&separator());
    offer.append(reload);
    offer.append(&separator());
    offer.append(keep);
    offer
}

/// The "·" between two of the status line's words.
fn separator() -> gtk::Label {
    let separator = gtk::Label::new(Some(SEPARATOR));
    separator.add_css_class("lib-meta");
    separator
}

/// The status line at the pane's foot.
fn foot(status: &gtk::Label, offer: &gtk::Box) -> gtk::Box {
    let foot = gtk::Box::new(gtk::Orientation::Horizontal, DOT_GAP);
    foot.add_css_class("lib-foot");
    foot.set_height_request(FOOT_HEIGHT);
    let dot = icon((DOT, DOT), dot_icon);
    dot.add_css_class("lib-icon");
    dot.set_margin_start(FOOT_LEFT);
    foot.append(&dot);
    status.set_hexpand(true);
    status.set_xalign(0.0);
    status.set_ellipsize(gtk::pango::EllipsizeMode::End);
    // An ellipsizing label still asks for its whole sentence as its natural width, and the pane
    // grows to it: the "no dictionary" notice doubled the Library's width (#415). One character
    // is its natural width now, and `hexpand` hands it the foot's slack.
    status.set_max_width_chars(1);
    foot.append(status);
    foot.append(offer);
    // Where the manuscripts are, at the foot's right (`.lib-where`), because a
    // status line that says the work is saved without saying where has not
    // said the half that matters.
    let device = gtk::Label::new(Some(WHERE));
    device.add_css_class("lib-meta");
    device.set_margin_end(ROW_RIGHT);
    foot.append(&device);
    foot
}

/// The excerpt's two lines, set on the leading the oracle gives them, and the
/// mark behind what a query matched in them.
///
/// The leading is through Pango rather than through the stylesheet: GTK's CSS
/// has no `line-height`, and two lines of 13.5 px type on their own natural
/// leading stand a pixel and a half tighter than iA's. The mark is through
/// Pango because it stands behind a range of the text and a stylesheet can
/// only reach the whole label.
fn marks(snippet: Option<&Snippet>, drawing: &Drawing<'_>) -> gtk::pango::AttrList {
    let attributes = gtk::pango::AttrList::new();
    let height = pixels(EXCERPT_LEADING * f64::from(gtk::pango::SCALE));
    attributes.insert(gtk::pango::AttrInt::new_line_height_absolute(height));
    let Some(snippet) = snippet else {
        return attributes;
    };
    let at = snippet.at();
    let (from, to) = (index(at.start), index(at.end));
    let mut ground = gtk::pango::AttrColor::new_background(
        marked(drawing.mark.red),
        marked(drawing.mark.green),
        marked(drawing.mark.blue),
    );
    ground.set_start_index(from);
    ground.set_end_index(to);
    attributes.insert(ground);
    let mut ink = gtk::pango::AttrColor::new_foreground(
        marked(drawing.marked.red),
        marked(drawing.marked.green),
        marked(drawing.marked.blue),
    );
    ink.set_start_index(from);
    ink.set_end_index(to);
    attributes.insert(ink);
    attributes
}

/// A byte offset into a snippet as Pango counts them.
fn index(at: usize) -> u32 {
    u32::try_from(at).unwrap_or(u32::MAX)
}

/// One channel of a colour as Pango marks a range in: the byte
/// [`quill_engine::theme::channel`] rounds it to, widened to the sixteen bits
/// Pango holds a channel in.
///
/// The rounding is the engine's and happens once there; the widening is exact
/// — a byte in both halves of the sixteen — and what the screen is given is
/// that byte either way.
fn marked(amount: f64) -> u16 {
    u16::from(theme::channel(amount)) * 257
}

/// A section's name: the folder's own, or the path where it has none.
fn section_name(section: &Section<'_>) -> String {
    match section.path.file_name() {
        Some(name) => name.to_string_lossy().into_owned(),
        None => section.path.display().to_string(),
    }
}

/// A section's head: the folder's name, or Pinned.
fn section_head(name: &str, folder: bool) -> gtk::Box {
    let head = gtk::Box::new(gtk::Orientation::Horizontal, SECTION_GAP);
    head.set_height_request(SECTION_HEIGHT);
    head.set_margin_start(SECTION_LEFT);
    head.set_margin_end(ROW_RIGHT);
    if folder {
        // In the grey rather than the folder colour, which the oracle keeps
        // for a folder inside a Location (`#library[data-loc="device"]
        // .lib-loc .fold { color: var(--lib-2) }`): a Location is what the
        // pane is made of, not a folder inside one.
        head.append(&column(icon(FOLDER, folder_icon), "lib-icon"));
    }
    let label = gtk::Label::new(Some(name));
    label.add_css_class("lib-head");
    label.set_xalign(0.0);
    label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    head.append(&label);
    head
}

/// An icon in the row's icon column, so that names line up whatever they are
/// marked with.
fn column(mark: gtk::DrawingArea, class: &str) -> gtk::Box {
    mark.add_css_class(class);
    let column = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    column.set_width_request(ICON_COLUMN);
    column.append(&mark);
    column
}

/// A drawing of `size`, left where the row puts it.
fn icon(
    size: (i32, i32),
    draw: impl Fn(&gtk::DrawingArea, &cairo::Context) + 'static,
) -> gtk::DrawingArea {
    chrome::icon(size.0, size.1, draw)
}

/// The name a row shows: the file's, without its extension unless the writer
/// asked for them (`library.show_extensions`, false by default).
fn row_name(name: &str, extensions: bool) -> String {
    if extensions {
        return name.to_string();
    }
    match Path::new(name).file_stem() {
        Some(stem) => stem.to_string_lossy().into_owned(),
        None => name.to_string(),
    }
}

/// The count beside the sort control: how much there is to write with.
fn counted(documents: usize, words: usize) -> String {
    let documents = if documents == 1 {
        "1 document".to_string()
    } else {
        format!("{documents} documents")
    };
    let words = if words == 1 {
        "1 word".to_string()
    } else {
        format!("{words} words")
    };
    format!("{documents} · {words}")
}

/// The count while a query narrows the list: how much of the Library answered
/// it (`files.js` `renderCount`: `${shown} of ${list.length} documents`).
fn narrowed(found: usize, of: usize) -> String {
    format!("{found} of {of} documents")
}

/// The head of one file: what its row shows of it, and how much of it there is.
///
/// Read once per refresh and shared by the row and the count, so that a Library
/// of a thousand Documents is a thousand bounded reads and not two thousand.
struct Head {
    /// Two lines of what the file says.
    excerpt: String,
    /// How many words the read holds ([`quill_engine::stats::words`], the same
    /// count the stats bar shows).
    words: usize,
    /// When the file was last written, as the read found it: what says whether
    /// a later refresh has to read it again ([`Sidebar::read_heads`]).
    modified: Option<SystemTime>,
}

impl Head {
    /// The first [`EXCERPT_BYTES`] of the file at `path`, read as prose, held
    /// against the write time `modified` the Library has for it.
    ///
    /// A file that cannot be read is an empty head rather than a missing row:
    /// the tree says the file is there and the sidebar's job is to show it,
    /// whatever a reader of it just found.
    fn of(path: &Path, modified: Option<SystemTime>) -> Self {
        let Some(read) = beginning(path) else {
            return Self {
                excerpt: String::new(),
                words: 0,
                modified,
            };
        };
        Self {
            words: quill_engine::stats::words(&read),
            excerpt: prose(&read),
            modified,
        }
    }
}

/// What the sort control reads in each order.
fn sort_title(sort: Sort) -> &'static str {
    match sort {
        Sort::Date => "Sort by Date",
        Sort::Name => "Sort by Name",
    }
}

/// The first [`EXCERPT_BYTES`] of the file at `path`, or nothing where it
/// cannot be read.
fn beginning(path: &Path) -> Option<String> {
    let file = std::fs::File::open(path).ok()?;
    let mut read = Vec::new();
    file.take(EXCERPT_BYTES).read_to_end(&mut read).ok()?;
    Some(String::from_utf8_lossy(&read).into_owned())
}

/// Two lines of what a file says, for the row beneath its name.
///
/// The file's own text with its Markdown markers taken off the front of each
/// line and its line breaks closed up, which is what the oracle shows
/// (`files.js` `rowHTML`): the row is a glance at the Document, not a
/// rendering of it. At most [`EXCERPT_CHARS`] characters are kept.
fn prose(text: &str) -> String {
    let mut said = String::new();
    for line in text.lines() {
        let line = line.trim_start_matches(['#', '>', '-', '*', '+', ' ', '\t']);
        for word in line.split_whitespace() {
            if said.chars().count() >= EXCERPT_CHARS {
                return said;
            }
            if !said.is_empty() {
                said.push(' ');
            }
            said.push_str(word);
        }
    }
    said
}

/// When a file was last written, as its row says it.
///
/// The oracle's rule (`files.js` `fmtDate`), in English rather than in the
/// machine's locale: the time of day for today, Yesterday, the weekday inside
/// a week, the month and day inside the year, and the year with it beyond
/// that.
fn stamp(modified: SystemTime, now: &glib::DateTime) -> String {
    let Ok(since) = modified.duration_since(SystemTime::UNIX_EPOCH) else {
        return String::new();
    };
    let seconds = i64::try_from(since.as_secs()).unwrap_or(i64::MAX);
    let Ok(when) = glib::DateTime::from_unix_local(seconds) else {
        return String::new();
    };
    dated(&when, now)
}

/// [`stamp`] with both dates already resolved, so a test can name them.
fn dated(when: &glib::DateTime, now: &glib::DateTime) -> String {
    let day = days(when.year(), when.month(), when.day_of_month());
    let today = days(now.year(), now.month(), now.day_of_month());
    let month = MONTHS[usize::try_from(when.month() - 1).unwrap_or(0).min(11)];
    match today - day {
        0 => {
            let hour = when.hour();
            let (twelve, half) = (hour % 12, if hour < 12 { "AM" } else { "PM" });
            let twelve = if twelve == 0 { 12 } else { twelve };
            format!("{twelve}:{:02} {half}", when.minute())
        }
        1 => "Yesterday".to_string(),
        2..=5 => DAYS[usize::try_from((day + 4).rem_euclid(7)).unwrap_or(0)].to_string(),
        _ if when.year() == now.year() => format!("{month} {}", when.day_of_month()),
        _ => format!("{month} {}, {:02}", when.day_of_month(), when.year() % 100),
    }
}

/// The day number of a civil date, counting from 1970-01-01.
///
/// Days rather than seconds, because what the row says depends on which day a
/// file was written and not on how many hours ago it was: a file written last
/// night is Yesterday at nine this morning. Howard Hinnant's `days_from_civil`.
fn days(year: i32, month: i32, day: i32) -> i64 {
    let year = i64::from(if month <= 2 { year - 1 } else { year });
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let month = i64::from(month);
    let day = i64::from(day);
    let day_of_year = (153 * (if month > 2 { month - 3 } else { month + 9 }) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

/// The document mark (`files.js` `I.doc`): a page with its corner turned.
fn document_icon(area: &gtk::DrawingArea, cr: &cairo::Context) {
    chrome::source(area, cr, 1.0);
    cr.set_line_width(1.1);
    cr.move_to(1.1, 1.6);
    cr.curve_to(1.1, 1.05, 1.55, 0.6, 2.1, 0.6);
    cr.line_to(8.1, 0.6);
    cr.line_to(13.0, 5.2);
    cr.line_to(13.0, 15.4);
    cr.curve_to(13.0, 15.95, 12.55, 16.4, 12.0, 16.4);
    cr.line_to(2.1, 16.4);
    cr.curve_to(1.55, 16.4, 1.1, 15.95, 1.1, 15.4);
    cr.close_path();
    let _ = cr.stroke();
    cr.move_to(8.1, 0.9);
    cr.line_to(8.1, 4.3);
    cr.curve_to(8.1, 4.85, 8.55, 5.3, 9.1, 5.3);
    cr.line_to(12.6, 5.3);
    let _ = cr.stroke();
}

/// The panel mark in the head (`files.js` `I.panel`), the same one the title
/// bar's Library toggle carries: a rounded frame with a divider a third of the
/// way across.
fn panel_icon(area: &gtk::DrawingArea, cr: &cairo::Context) {
    chrome::source(area, cr, 1.0);
    cr.scale(f64::from(MARK) / 16.0, f64::from(MARK) / 16.0);
    cr.set_line_width(1.2);
    cr.move_to(1.6, 4.8);
    cr.curve_to(1.6, 3.6, 2.6, 2.6, 3.8, 2.6);
    cr.line_to(12.2, 2.6);
    cr.curve_to(13.4, 2.6, 14.4, 3.6, 14.4, 4.8);
    cr.line_to(14.4, 11.2);
    cr.curve_to(14.4, 12.4, 13.4, 13.4, 12.2, 13.4);
    cr.line_to(3.8, 13.4);
    cr.curve_to(2.6, 13.4, 1.6, 12.4, 1.6, 11.2);
    cr.close_path();
    let _ = cr.stroke();
    cr.move_to(6.4, 2.6);
    cr.line_to(6.4, 13.4);
    let _ = cr.stroke();
}

/// The plus in the head (`files.js` `I.plus`): a new Document.
fn plus_icon(area: &gtk::DrawingArea, cr: &cairo::Context) {
    chrome::source(area, cr, 1.0);
    cr.scale(f64::from(MARK) / 16.0, f64::from(MARK) / 16.0);
    cr.set_line_width(1.4);
    cr.set_line_cap(cairo::LineCap::Round);
    cr.move_to(8.0, 3.0);
    cr.line_to(8.0, 13.0);
    let _ = cr.stroke();
    cr.move_to(3.0, 8.0);
    cr.line_to(13.0, 8.0);
    let _ = cr.stroke();
}

/// The folder mark (`files.js` `I.folder`): a filled tab folder.
fn folder_icon(area: &gtk::DrawingArea, cr: &cairo::Context) {
    chrome::source(area, cr, 1.0);
    cr.move_to(0.7, 3.1);
    cr.curve_to(0.7, 2.2, 1.4, 1.5, 2.3, 1.5);
    cr.line_to(5.6, 1.5);
    cr.line_to(7.1, 3.2);
    cr.line_to(14.3, 3.2);
    cr.curve_to(15.2, 3.2, 15.9, 3.9, 15.9, 4.8);
    cr.line_to(15.9, 11.3);
    cr.curve_to(15.9, 12.2, 15.2, 12.9, 14.3, 12.9);
    cr.line_to(2.3, 12.9);
    cr.curve_to(1.4, 12.9, 0.7, 12.2, 0.7, 11.3);
    cr.close_path();
    let _ = cr.fill();
}

/// A chevron (`files.js` `I.chev`), pointing down where what it opens is open
/// and right where it is closed (`.lib-row.folder.col .cv { rotate(-90deg) }`).
fn chevron_icon(area: &gtk::DrawingArea, cr: &cairo::Context, open: bool) {
    chrome::source(area, cr, 1.0);
    cr.set_line_width(1.4);
    cr.set_line_cap(cairo::LineCap::Round);
    cr.set_line_join(cairo::LineJoin::Round);
    if !open {
        cr.translate(5.5, 5.5);
        cr.rotate(-std::f64::consts::FRAC_PI_2);
        cr.translate(-5.5, -5.5);
    }
    cr.move_to(2.8, 4.2);
    cr.line_to(5.5, 6.9);
    cr.line_to(8.2, 4.2);
    let _ = cr.stroke();
}

/// The magnifier beside the search field (`files.js` `I.search`).
fn magnifier_icon(area: &gtk::DrawingArea, cr: &cairo::Context) {
    chrome::source(area, cr, 1.0);
    cr.set_line_width(1.3);
    cr.set_line_cap(cairo::LineCap::Round);
    cr.arc(5.15, 5.15, 3.7, 0.0, std::f64::consts::TAU);
    let _ = cr.stroke();
    cr.move_to(7.9, 7.9);
    cr.line_to(10.8, 10.8);
    let _ = cr.stroke();
}

/// The status dot (`.lib-status .dot`), at rest.
fn dot_icon(area: &gtk::DrawingArea, cr: &cairo::Context) {
    dot_at(area, cr, DOT_ALPHA);
}

/// The dot that marks a row whose file changed on disk: the same circle at
/// full strength, as the oracle's warning dot is (`files.css` line 212).
fn warned_dot(area: &gtk::DrawingArea, cr: &cairo::Context) {
    dot_at(area, cr, 1.0);
}

/// A [`DOT`]-wide circle in the widget's own colour, at `alpha`.
fn dot_at(area: &gtk::DrawingArea, cr: &cairo::Context, alpha: f64) {
    chrome::source(area, cr, alpha);
    let half = f64::from(DOT) / 2.0;
    cr.arc(half, half, half, 0.0, std::f64::consts::TAU);
    let _ = cr.fill();
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A date the tests measure from: a Wednesday.
    fn at(year: i32, month: i32, day: i32, hour: i32) -> glib::DateTime {
        glib::DateTime::from_local(year, month, day, hour, 0, 0.0).expect("a date")
    }

    /// What `model` offers, in drawing order and its sections flattened: each
    /// row's label and the action it fires.
    fn menu_rows(model: &gio::MenuModel) -> Vec<(String, String)> {
        let mut rows = Vec::new();
        for at in 0..model.n_items() {
            if let Some(section) = model.item_link(at, "section") {
                rows.extend(menu_rows(&section));
                continue;
            }
            let string = |attribute: &str| {
                model
                    .item_attribute_value(at, attribute, Some(glib::VariantTy::STRING))
                    .and_then(|value| value.get::<String>())
                    .unwrap_or_default()
            };
            rows.push((string("label"), string("action")));
        }
        rows
    }

    #[test]
    fn a_location_head_offers_adding_a_location_and_removing_this_one() {
        let rows = menu_rows(location_menu(Path::new("/w/Drafts")).upcast_ref());
        assert_eq!(
            rows,
            vec![
                (
                    "Add Location…".to_string(),
                    "win.file.openFolder".to_string()
                ),
                ("Remove from Library".to_string(), "row.remove".to_string()),
            ],
            "Add Location… is the Command the Palette runs, not a second path"
        );
    }

    #[test]
    fn a_rows_menu_offers_the_half_of_pin_or_unpin_the_row_is_not() {
        let pinning = |folder, pinned| {
            menu_rows(row_menu(folder, pinned).upcast_ref())
                .into_iter()
                .find(|(label, _)| label == "Pin" || label == "Unpin")
                .expect("Pin or Unpin")
        };
        assert_eq!(pinning(false, false), ("Pin".into(), "row.pin".into()));
        assert_eq!(pinning(false, true), ("Unpin".into(), "row.unpin".into()));
        // A folder has neither a name a rename can give it nor a copy worth
        // making, so pinning is the whole of its menu.
        assert_eq!(pinning(true, false), ("Pin".into(), "row.pin".into()));
        assert_eq!(menu_rows(row_menu(true, false).upcast_ref()).len(), 1);
    }

    #[test]
    fn a_row_drops_the_extension_unless_the_writer_asked_for_it() {
        assert_eq!(row_name("sea-storm.md", false), "sea-storm");
        assert_eq!(row_name("sea-storm.md", true), "sea-storm.md");
        assert_eq!(row_name("letters.txt", false), "letters");
        assert_eq!(row_name("notes", false), "notes");
    }

    #[test]
    fn the_count_says_documents_and_words_and_says_one_of_each_singly() {
        assert_eq!(counted(0, 0), "0 documents · 0 words");
        assert_eq!(counted(1, 1), "1 document · 1 word");
        assert_eq!(counted(8, 239), "8 documents · 239 words");
    }

    #[test]
    fn the_count_says_how_much_of_the_library_a_query_left() {
        assert_eq!(narrowed(2, 8), "2 of 8 documents");
        assert_eq!(narrowed(0, 8), "0 of 8 documents");
    }

    #[test]
    fn a_content_hits_snippet_is_marked_where_the_match_is() {
        // Its own folder, named for this process, because the worktrees test
        // at the same time.
        let root = std::env::temp_dir().join(format!("quill-sidebar-{}", std::process::id()));
        std::fs::create_dir_all(&root).expect("a folder to search");
        std::fs::write(root.join("harbour.md"), "The lamps. The sea was flat.\n")
            .expect("a file to find");
        let library = Library::open(std::slice::from_ref(&root), &[]);
        let view = View {
            show_hidden: false,
            sort: Sort::Date,
        };
        let mut contents = Contents::new();
        let found = library.search("sea", &view, &mut contents);
        let snippet = found
            .first()
            .expect("the file's text holds the word")
            .snippet()
            .expect("a content hit carries a snippet");
        assert_eq!(snippet.matched(), "sea");
        let read = BTreeMap::new();
        let drawing = Drawing {
            extensions: false,
            now: None,
            read: &read,
            mark: Colour::rgba(0, 191, 255, 1.0),
            marked: Colour::rgba(28, 28, 28, 1.0),
        };
        let at = snippet.at();
        let coloured = |over: &gtk::pango::AttrList, kind| {
            over.attributes()
                .into_iter()
                .filter(|attribute| attribute.type_() == kind)
                .map(|attribute| (attribute.start_index(), attribute.end_index()))
                .collect::<Vec<_>>()
        };
        // The match stands in the mark's ground and in the ink, out of the
        // grey the words either side of it are set in.
        let over = marks(Some(snippet), &drawing);
        let range = vec![(index(at.start), index(at.end))];
        assert_eq!(coloured(&over, gtk::pango::AttrType::Background), range);
        assert_eq!(coloured(&over, gtk::pango::AttrType::Foreground), range);
        let plain = marks(None, &drawing);
        assert!(
            coloured(&plain, gtk::pango::AttrType::Background).is_empty()
                && coloured(&plain, gtk::pango::AttrType::Foreground).is_empty(),
            "a row with no query is unmarked"
        );
        std::fs::remove_dir_all(&root).expect("the folder to go");
    }

    #[test]
    fn an_excerpt_is_the_prose_with_the_markers_off_and_the_lines_closed_up() {
        assert_eq!(
            prose("# The storm\n\nThe gale came up the coast\nat four in the morning.\n"),
            "The storm The gale came up the coast at four in the morning."
        );
        assert_eq!(prose("- one\n- two\n"), "one two");
        assert_eq!(prose("   \n\n"), "");
    }

    #[test]
    fn an_excerpt_stops_at_the_characters_a_row_can_show() {
        let long = "word ".repeat(EXCERPT_CHARS);
        let said = prose(&long);
        assert!(
            said.chars().count() <= EXCERPT_CHARS + "word".len(),
            "{} characters",
            said.chars().count()
        );
    }

    #[test]
    fn a_date_is_the_time_today_yesterday_the_weekday_then_the_month() {
        // 2026-03-04 was a Wednesday.
        let now = at(2026, 3, 4, 15);
        assert_eq!(dated(&at(2026, 3, 4, 9), &now), "9:00 AM");
        assert_eq!(dated(&at(2026, 3, 4, 0), &now), "12:00 AM");
        assert_eq!(dated(&at(2026, 3, 4, 13), &now), "1:00 PM");
        assert_eq!(dated(&at(2026, 3, 3, 9), &now), "Yesterday");
        assert_eq!(dated(&at(2026, 3, 1, 9), &now), "Sun");
        assert_eq!(dated(&at(2026, 1, 9, 9), &now), "Jan 9");
        assert_eq!(dated(&at(2025, 3, 14, 9), &now), "Mar 14, 25");
    }

    #[test]
    fn the_fixtures_mtimes_are_the_dates_the_oracles_shot_shows() {
        // `shots/oracle/library/manifest.json` stamps sea-storm.md at
        // 1741942800, which the frozen shot dates `Mar 14, 25`.
        let stamped = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_741_942_800);
        assert_eq!(stamp(stamped, &at(2026, 9, 3, 12)), "Mar 14, 25");
    }

    #[test]
    fn the_day_number_counts_from_the_epoch() {
        assert_eq!(days(1970, 1, 1), 0);
        assert_eq!(days(1970, 1, 2), 1);
        assert_eq!(days(2000, 3, 1), 11_017);
        assert_eq!(days(2026, 3, 4) - days(2026, 3, 3), 1);
        assert_eq!(days(2026, 1, 1) - days(2025, 12, 31), 1);
    }

    #[test]
    fn the_sheet_names_the_panes_ground_its_hairline_and_its_accent_bar() {
        let sheet = stylesheet(Ground::default());
        assert!(sheet.contains(".library {"), "{sheet}");
        assert!(sheet.contains("border-right: 1px solid"), "{sheet}");
        assert!(sheet.contains("font-size: 13.5px"), "{sheet}");
        assert!(
            sheet.contains("list > row:selected .lib-bar { background-color: #00bfff; }"),
            "{sheet}"
        );
    }
}
