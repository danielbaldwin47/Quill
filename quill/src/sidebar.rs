//! The Library beside the page: sections, rows, the search field and the
//! status line (#253).
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
//! Its measurements are `files.css`'s, as constants below; its colours are the
//! theme's roles, through [`stylesheet`], which rides with the bars' sheet so
//! that one ground change repaints both.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::SystemTime;

use gtk::prelude::*;
use gtk::{cairo, gdk, glib};
use quill_engine::library::{Row, Sort, View};
use quill_engine::theme::{Role, Scheme};

use crate::chrome::{self, CHROME_FONT};
use crate::ground::Ground;
use crate::window::Window;

/// The pane's width (`files.css` `--lib-w: 368px`).
pub const WIDTH: i32 = 368;
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
/// What the field keeps clear of the pane's edges (`.lib-find { padding: 0
/// 10px }`).
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
/// Where the sort row's text starts (`.lib-sort { padding-left: 10px }`).
const SORT_LEFT: i32 = 10;
/// What the count keeps clear of the right edge (`padding-right: 14px`).
const SORT_RIGHT: i32 = 14;
/// A chevron's side (`I.chev`, eleven by eleven).
const CHEV: i32 = 11;
/// A row's inset from the pane's left edge (`.lib-row { padding-left: 16px }`).
const ROW_LEFT: i32 = 16;
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

/// The status line at rest: the file is on disk as the writer left it.
///
/// The texts autosave moves it through — "Saving…", and what a conflict has
/// to say — are the ticket that owns those moments ([`Sidebar::set_status`]).
const AT_REST: &str = "All changes saved";

/// What the search field says while it is empty.
const PLACEHOLDER: &str = "Search documents";

/// How much of a file the excerpt is taken from.
///
/// A bound rather than the whole file: the excerpt is two lines of type, and
/// a Library of a thousand Documents would otherwise read a thousand files
/// whole to draw them. Two lines of 368 px hold well under [`EXCERPT_CHARS`]
/// characters, and the first four kilobytes of a Markdown file hold that many
/// even where every line is a marker.
const EXCERPT_BYTES: u64 = 4096;
/// How many characters of it a row keeps. The label ellipsizes at two lines
/// long before this; the cap is what stops a one-line file of 4 KB being laid
/// out in full to find that out.
const EXCERPT_CHARS: usize = 240;

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
         .library button.lib-btn {{\n\
         \x20 background: none; border: none; box-shadow: none; outline: none;\n\
         \x20 min-height: 0; min-width: 0; padding: 0;\n\
         \x20 border-radius: {BUTTON_RADIUS}px; color: {dim};\n\
         }}\n\
         .library button.lib-btn:hover {{ background-color: {hit}; color: {ink}; }}\n\
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
         .library list > row:selected .lib-bar {{ background-color: {accent}; }}\n"
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
}

/// One row of the list, and what it stands for.
struct Listed {
    row: gtk::ListBoxRow,
    /// The file or folder it draws.
    path: PathBuf,
    /// Whether it is a folder, which opens and closes rather than opening a
    /// Document.
    folder: bool,
}

/// The Library beside the page.
#[derive(Clone)]
pub struct Sidebar {
    root: gtk::Box,
    /// What the pane's head calls it: the one Location's folder, where that is
    /// the whole Library, and [`TITLE`] where there is more than one thing
    /// under it.
    title: gtk::Label,
    entry: gtk::Entry,
    sort_label: gtk::Label,
    count: gtk::Label,
    list: gtk::ListBox,
    status: gtk::Label,
    /// The window this sidebar belongs to, so a row can open a Document in
    /// it. Weak, because the window owns the sidebar.
    window: Rc<RefCell<Option<glib::WeakRef<Window>>>>,
    /// The rows now drawn, top to bottom, for the highlight and the arrows.
    rows: Rc<RefCell<Vec<Listed>>>,
    /// The folders the writer has opened. Everything else is closed, which is
    /// what the spec asks a section to open at.
    expanded: Rc<RefCell<BTreeSet<PathBuf>>>,
    /// What the list is sorted by. Date, newest first, until a writer says
    /// otherwise; not a setting, because nothing persists it yet.
    sort: Rc<Cell<Sort>>,
    /// The Document the window is showing, whose row is the highlighted one.
    open: Rc<RefCell<Option<PathBuf>>>,
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
        root.set_visible(false);

        let title = gtk::Label::new(Some(TITLE));
        root.append(&head(&title));
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

        let status = gtk::Label::new(Some(AT_REST));
        status.add_css_class("lib-meta");
        root.append(&foot(&status));

        let sidebar = Self {
            root,
            title,
            entry,
            sort_label,
            count,
            list,
            status,
            window: Rc::new(RefCell::new(None)),
            rows: Rc::new(RefCell::new(Vec::new())),
            expanded: Rc::new(RefCell::new(BTreeSet::new())),
            sort: Rc::new(Cell::new(Sort::Date)),
            open: Rc::new(RefCell::new(None)),
        };
        let cycling = sidebar.clone();
        sort_button.connect_clicked(move |_| cycling.cycle_sort());
        sidebar
    }

    /// The widget to stand left of the page.
    #[must_use]
    pub fn widget(&self) -> &gtk::Widget {
        self.root.upcast_ref()
    }

    /// Gives the pane its window, which is what a row opens a Document in,
    /// and draws the Library for the first time.
    ///
    /// Split from [`Sidebar::new`] because a window's widgets are built before
    /// it has a session to build them from ([`crate::window::Window::new`]).
    pub fn attach(&self, window: &Window) {
        self.window.replace(Some(window.downgrade()));
        let activated = self.clone();
        self.list
            .connect_row_activated(move |_, row| activated.activate(row));
        // Esc hands the keyboard back to the page, which is where a writer who
        // came to the pane looking for a Document leaves it.
        let keys = gtk::EventControllerKey::new();
        let escaped = self.clone();
        keys.connect_key_pressed(move |_, key, _, _| {
            if key == gdk::Key::Escape {
                escaped.leave();
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        self.list.add_controller(keys);
        self.refresh();
    }

    /// Whether the pane is showing.
    #[must_use]
    pub fn is_shown(&self) -> bool {
        self.root.is_visible()
    }

    /// Shows or hides the pane. The page keeps its own centring and is simply
    /// given a narrower window, as the oracle's is.
    pub fn set_shown(&self, shown: bool) {
        self.root.set_visible(shown);
    }

    /// Puts the keyboard in the search field, opening the pane if it is shut:
    /// what `library.search` does.
    pub fn focus_search(&self) {
        self.set_shown(true);
        self.entry.grab_focus();
    }

    /// Puts `query` in the search field, as `--search` does.
    pub fn set_query(&self, query: &str) {
        self.entry.set_text(query);
        self.entry.set_position(-1);
    }

    /// What the status line says.
    pub fn set_status(&self, said: &str) {
        self.status.set_text(said);
    }

    /// The Document the window is showing, whose row is highlighted.
    pub fn set_open(&self, path: Option<&Path>) {
        self.open.replace(path.map(Path::to_path_buf));
        self.highlight();
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
        let library = session.library();
        let view = View {
            show_hidden: session.settings().library.show_hidden,
            sort: self.sort.get(),
        };
        let now = glib::DateTime::now_local().ok();
        // Every shown file read once, here, for the two things a row and the
        // count both want out of it. The walk is the files the view shows and
        // at most [`EXCERPT_BYTES`] of each.
        let read: BTreeMap<PathBuf, Head> = library
            .files(&view)
            .map(|file| (file.path().to_path_buf(), Head::of(file.path())))
            .collect();
        let drawing = Drawing {
            extensions: session.settings().library.show_extensions,
            now: now.as_ref(),
            read: &read,
        };
        let mut sections = 0;
        let pinned = library.pinned_rows(&view);
        // One Location and nothing pinned is one thing to name, and the head
        // names it: a pane called Library over a section called library says
        // the same word twice. Anything else is the pane over its parts.
        let parts = !pinned.is_empty() || library.locations().len() > 1;
        if !pinned.is_empty() {
            self.section(None, "Pinned", sections, &pinned, &drawing);
            sections += 1;
        }
        self.title.set_text(TITLE);
        for section in library.shown(&view) {
            let name = match section.root.file_name() {
                Some(name) => name.to_string_lossy().into_owned(),
                None => section.root.display().to_string(),
            };
            if parts {
                self.section(Some(section.root), &name, sections, &section.rows, &drawing);
            } else {
                self.title.set_text(&name);
                self.rows_of(&section.rows, &drawing);
            }
            sections += 1;
        }
        let words = read.values().map(|head| head.words).sum();
        self.count.set_text(&counted(read.len(), words));
        self.highlight();
    }

    /// One section: its head, then the rows of it that are not inside a closed
    /// folder.
    fn section(
        &self,
        root: Option<&Path>,
        name: &str,
        above: usize,
        rows: &[Row<'_>],
        drawing: &Drawing<'_>,
    ) {
        let head = gtk::ListBoxRow::new();
        head.set_selectable(false);
        head.set_activatable(false);
        head.set_child(Some(&section_head(name, root.is_some())));
        if above > 0 {
            head.set_margin_top(SECTION_AIR);
        }
        self.list.append(&head);
        self.rows_of(rows, drawing);
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
                Row::File { file, depth } => {
                    self.file_row(file.name(), row.path(), *depth, file.modified(), drawing)
                }
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
        self.listed(line, path, true)
    }

    /// A file: its name, when it was last written, and two lines of what it
    /// says.
    fn file_row(
        &self,
        name: &str,
        path: &Path,
        depth: usize,
        modified: Option<SystemTime>,
        drawing: &Drawing<'_>,
    ) -> Listed {
        let line = gtk::Box::new(gtk::Orientation::Horizontal, ICON_GAP);
        line.set_margin_start(ROW_LEFT + INDENT * i32::try_from(depth).unwrap_or(0));
        line.set_margin_end(ROW_RIGHT);
        line.set_margin_top(ROW_TOP);
        line.set_margin_bottom(ROW_BOTTOM);
        let mark = icon(DOC, document_icon);
        mark.set_valign(gtk::Align::Start);
        line.append(&column(mark, "lib-icon"));

        let body = gtk::Box::new(gtk::Orientation::Vertical, 0);
        body.set_hexpand(true);
        let top = gtk::Box::new(gtk::Orientation::Horizontal, ICON_GAP);
        let title = gtk::Label::new(Some(&shown_name(name, drawing.extensions)));
        title.add_css_class("lib-name");
        title.set_hexpand(true);
        title.set_xalign(0.0);
        title.set_ellipsize(gtk::pango::EllipsizeMode::End);
        top.append(&title);
        if let (Some(modified), Some(now)) = (modified, drawing.now) {
            let date = gtk::Label::new(Some(&stamp(modified, now)));
            date.add_css_class("lib-date");
            top.append(&date);
        }
        body.append(&top);
        let said = drawing
            .read
            .get(path)
            .map_or("", |head| head.excerpt.as_str());
        if !said.is_empty() {
            let excerpt = gtk::Label::new(Some(said));
            excerpt.add_css_class("lib-excerpt");
            excerpt.set_xalign(0.0);
            excerpt.set_wrap(true);
            excerpt.set_wrap_mode(gtk::pango::WrapMode::WordChar);
            excerpt.set_attributes(Some(&leading()));
            excerpt.set_lines(EXCERPT_LINES);
            excerpt.set_ellipsize(gtk::pango::EllipsizeMode::End);
            body.append(&excerpt);
        }
        line.append(&body);
        self.listed(line, path, false)
    }

    /// A row of the list: the hairline above it, the accent bar down its left
    /// edge, and the line itself.
    fn listed(&self, line: gtk::Box, path: &Path, folder: bool) -> Listed {
        let bar = gtk::Box::new(gtk::Orientation::Vertical, 0);
        bar.add_css_class("lib-bar");
        let beside = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        beside.append(&bar);
        beside.append(&line);
        let stacked = gtk::Box::new(gtk::Orientation::Vertical, 0);
        // The hairline starts under the name rather than at the pane's edge,
        // as iA's does (`.lib-row + .lib-row::before { left: 46px; right: 14px }`),
        // and is left off the first row of a section.
        let rule = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        rule.add_css_class("lib-rule");
        rule.set_height_request(1);
        rule.set_margin_start(BAR.width + ROW_LEFT + ICON_COLUMN + ICON_GAP);
        rule.set_margin_end(ROW_RIGHT);
        rule.set_visible(!self.at_section_head());
        stacked.append(&rule);
        stacked.append(&beside);
        let row = gtk::ListBoxRow::new();
        row.set_child(Some(&stacked));
        Listed {
            row,
            path: path.to_path_buf(),
            folder,
        }
    }

    /// Whether the row now being built is the first of its section.
    fn at_section_head(&self) -> bool {
        self.list
            .last_child()
            .and_downcast::<gtk::ListBoxRow>()
            .is_some_and(|row| !row.is_selectable())
    }

    /// Highlights the row of the Document the window is showing, and no row at
    /// all where it is showing something the Library does not hold.
    fn highlight(&self) {
        let open = self.open.borrow();
        let found = open.as_deref().and_then(|open| {
            let rows = self.rows.borrow();
            rows.iter()
                .find(|listed| listed.path == open)
                .map(|listed| listed.row.clone())
        });
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
            let mut expanded = self.expanded.borrow_mut();
            if !expanded.remove(&path) {
                expanded.insert(path);
            }
            drop(expanded);
            self.refresh();
            return;
        }
        if let Some(window) = self.owner() {
            window.open_path(&path);
        }
    }

    /// Esc: the keyboard goes back to the page.
    fn leave(&self) {
        if let Some(window) = self.owner() {
            window.focus_editor();
        }
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

/// The pane's head: what it is called, the toggle that shuts it, and the
/// button that starts a Document in it.
///
/// Both buttons fire their Commands by name, so `file.new` — which the row
/// operations ticket builds — does nothing yet without the button being greyed,
/// which is how the bars' buttons stand ([`crate::chrome::Bars`]).
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
    field.set_margin_start(FIELD_PAD);
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
    button.set_margin_start(SORT_LEFT - SORT_BUTTON_PAD);
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

/// The status line at the pane's foot.
fn foot(status: &gtk::Label) -> gtk::Box {
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
    foot.append(status);
    // Where the manuscripts are, at the foot's right (`.lib-where`), because a
    // status line that says the work is saved without saying where has not
    // said the half that matters.
    let held = gtk::Label::new(Some(WHERE));
    held.add_css_class("lib-meta");
    held.set_margin_end(ROW_RIGHT);
    foot.append(&held);
    foot
}

/// The excerpt's two lines, set on the leading the oracle gives them.
///
/// Through Pango rather than through the stylesheet: GTK's CSS has no
/// `line-height`, and two lines of 13.5 px type on their own natural leading
/// stand a pixel and a half tighter than iA's.
fn leading() -> gtk::pango::AttrList {
    let attributes = gtk::pango::AttrList::new();
    let height = pixels(EXCERPT_LEADING * f64::from(gtk::pango::SCALE));
    attributes.insert(gtk::pango::AttrInt::new_line_height_absolute(height));
    attributes
}

/// `length` as a whole number: the excerpt's leading in Pango units. Rounded
/// here and only here, in the shape of `quill::tags::pixels`.
fn pixels(length: f64) -> i32 {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a line of type in Pango units is a few tens of thousands at most"
    )]
    let whole = length.round() as i32;
    whole
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
fn shown_name(name: &str, extensions: bool) -> String {
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
}

impl Head {
    /// The first [`EXCERPT_BYTES`] of the file at `path`, read as prose.
    ///
    /// A file that cannot be read is an empty head rather than a missing row:
    /// the tree says the file is there and the sidebar's job is to show it,
    /// whatever a reader of it just found.
    fn of(path: &Path) -> Self {
        let Some(read) = beginning(path) else {
            return Self {
                excerpt: String::new(),
                words: 0,
            };
        };
        Self {
            words: quill_engine::stats::words(&read),
            excerpt: prose(&read),
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
    said(&when, now)
}

/// [`stamp`] with both dates already resolved, so a test can name them.
fn said(when: &glib::DateTime, now: &glib::DateTime) -> String {
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
    chrome::source(area, cr, DOT_ALPHA);
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

    #[test]
    fn a_row_drops_the_extension_unless_the_writer_asked_for_it() {
        assert_eq!(shown_name("sea-storm.md", false), "sea-storm");
        assert_eq!(shown_name("sea-storm.md", true), "sea-storm.md");
        assert_eq!(shown_name("letters.txt", false), "letters");
        assert_eq!(shown_name("notes", false), "notes");
    }

    #[test]
    fn the_count_says_documents_and_words_and_says_one_of_each_singly() {
        assert_eq!(counted(0, 0), "0 documents · 0 words");
        assert_eq!(counted(1, 1), "1 document · 1 word");
        assert_eq!(counted(8, 239), "8 documents · 239 words");
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
        assert_eq!(said(&at(2026, 3, 4, 9), &now), "9:00 AM");
        assert_eq!(said(&at(2026, 3, 4, 0), &now), "12:00 AM");
        assert_eq!(said(&at(2026, 3, 4, 13), &now), "1:00 PM");
        assert_eq!(said(&at(2026, 3, 3, 9), &now), "Yesterday");
        assert_eq!(said(&at(2026, 3, 1, 9), &now), "Sun");
        assert_eq!(said(&at(2026, 1, 9, 9), &now), "Jan 9");
        assert_eq!(said(&at(2025, 3, 14, 9), &now), "Mar 14, 25");
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
