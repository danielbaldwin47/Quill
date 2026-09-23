//! The Library beside the page: the Organizer and the File List on grounds of
//! their own (#253, #441), and what the Filter field finds (#254).
//!
//! One sidebar per window, all of them showing the one Library the session
//! holds (`docs/architecture.md` § Library, § Windows). It stands left of the
//! page and pushes it right rather than covering it, at the 360 points the
//! Design oracle's pane measures (`dev/ref/ia/mac-native/NOTES.md` § State 28).
//!
//! Two columns, as ADR 0020 has them: the Organizer at [`ORGANIZER`] points
//! in the narrowest pane and half of any width dragged past it (#459), and the
//! File List beside it showing one Location's tree — folders
//! first and closed until they are opened, expanding in place — under that
//! Location's name. Each stands on its own ground a step off the paper, and
//! a 1 px rule down the Organizer's right edge closes the one against the
//! other (#448's round 13). The type is the
//! GTK UI face the bars are set in, sized to the capture's ink heights (#441
//! § Type), so the pane belongs to the same window as the page rather than to
//! a file manager.
//!
//! The Organizer chooses what the File List shows (#445): a Location's tree, a
//! pinned folder's, or the Documents last opened, flat and newest first, the
//! open Document left where it is whichever is chosen. Pinned lists the pins
//! of every Location, and is the one place in the column a dragged row can be
//! let go.
//!
//! Typing in the Filter field puts the engine's results in place of the tree:
//! the files whose names matched first, then the files whose texts did, each
//! of those with the one snippet around its match, set as plainly as any
//! excerpt ([`quill_engine::library::Library::search`]); the Sort pill reads
//! Search Relevance and is off while the query stands. Enter opens the highlighted
//! hit and Esc clears the field and hands the keyboard back to the page.
//!
//! A row can be acted on as well as opened (#256): a second click or `F2` puts
//! a field in the place of its name, and the right button opens what it can do
//! — Open, Rename, Duplicate, Pin or Unpin, Move to Trash, and on a Location's
//! head, Remove from Library. The pane decides *which* row; what happens to it
//! is the window's ([`crate::window`]) and, under that, the engine's, which
//! does the disk before the pane is drawn again.
//!
//! A row can also be dragged (#257). Let go over a folder's row or the File
//! List's head it moves into that folder, asked about first where the writer
//! asked to be asked (`library.confirm_move`); let go over the Organizer's
//! Pinned section it is pinned, and the section lights as the one target it is. What each drop would
//! do is [`crate::files::dropped`], a decision over paths, and a drop it would
//! do nothing with — a folder onto itself, a file into the folder it is already
//! in, a row already pinned — is refused while the drag is still in the air.
//!
//! Its right edge is the divider (#260): a [`GRAB`]-pixel strip lying over the
//! edge, which paints nothing at all — the File List's ground giving way to the
//! paper is the edge. A drag on it sets the pane's width for the app, every
//! window at once ([`crate::window::Window::resize_library`]), between 360 and
//! 500 points with the Organizer holding its width, and a double-click puts it
//! back to [`WIDTH`].
//!
//! Its measurements are State 28's, as the stub checked them on GTK (#437),
//! in constants below; its grounds and its grey are the theme's roles and its
//! other inks the capture's, through [`stylesheet`], which rides with the bars'
//! sheet so that one ground change repaints both.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, PoisonError, mpsc};
use std::time::{Duration, SystemTime};

use gtk::prelude::*;
use gtk::{cairo, gdk, gio, glib, graphene};
use quill_engine::document::full_name;
use quill_engine::library::{Contents, File, Found, Library, Row, Snippet, Sort, View};
use quill_engine::mark::Glyph;
use quill_engine::settings::{self, Choice, Mark, Order, ShowDate, library_width};
use quill_engine::theme::{Colour, PaneInks, Role, Scheme};

use crate::chrome::{self, CHROME_FONT};
use crate::files::{self, Dropped, Onto};
use crate::ground::Ground;
use crate::tags::pixels;
use crate::window::Window;

/// The pane's width until a writer drags the divider (`dev/ref/ia/mac-native/NOTES.md`
/// § State 28, *Pane, total*), and the width a double-click on the divider puts back.
pub const WIDTH: i32 = 360;
/// The Organizer's width at the pane's [`WIDTH`]: State 28's 129.5 points, at
/// the whole point a widget is asked for in. A drag on the divider widens it by
/// half of what it widens the pane ([`organizer_width`], #459).
const ORGANIZER: i32 = 130;
/// The Organizer's section heads, bold (#441 § Type: cap 16 device px).
const ORG_HEAD_PX: f64 = 11.0;
/// Where a head's ink begins across the column (16.5 points, #441 § The
/// Organizer).
const ORG_HEAD_LEFT: i32 = 16;
/// The air under a head's line, which leaves 8.5 points between its ink and
/// the top of the first row under it: the line keeps the rest under its
/// baseline, as the stub measured it on GTK (#437).
const ORG_HEAD_GAP: i32 = 5;
/// The air above every head but the first, between one section and the next.
const ORG_SECTION_AIR: i32 = 18;
/// An Organizer row's height, and the current Location's pill's; the pitch is
/// assumed until #440.
const ORG_ROW: i32 = 32;
/// How far in from either side of the column the pill stands, which leaves it
/// 110 points wide.
const ORG_PILL_INSET: i32 = 10;
/// The pill's corner.
const ORG_PILL_RADIUS: f64 = 5.5;
/// Where a row's icon begins across the column.
const ORG_ICON_LEFT: i32 = 20;
/// Where a row's text begins across the column.
const ORG_TEXT_LEFT: i32 = 42;
/// The air a name keeps from the pill's right end before it is ellipsised.
const ORG_TEXT_END: i32 = 6;
/// An Organizer row's type (#441 § Type).
const ORG_ROW_PX: f64 = 13.0;
/// The current Location's label on its pill, bold (#441 § Type).
const ORG_PILL_PX: f64 = 14.0;
/// The Recents mark's box.
const CLOCK: (i32, i32) = (14, 12);
/// The Organizer's first section head.
const LOCATIONS: &str = "Locations";
/// The Organizer's second section head.
const PINNED: &str = "Pinned";
/// The Organizer's third section head, and its one row.
const RECENTS: &str = "Recents";
/// What an empty Pinned section says in its place.
const NOTHING_PINNED: &str = "Pin a document or folder from its row menu";
/// What the Sort pill reads over Recents, whose order is the order they were
/// opened in and no sort's.
const LAST_OPENED: &str = "Sort by Last Opened";
/// What the Sort pill reads while a query stands, whose order is the engine's
/// relevance and no sort's.
const SEARCH_RELEVANCE: &str = "Sort by Search Relevance";

/// The action group the Sort pill's items fire: the pane's own, as `row.` is
/// ([`Sidebar::install_row_actions`]), and no Command's.
const PILL_GROUP: &str = "lib";
/// The pill's radio over `[library] sort`.
const SORT_ACTION: &str = "sort";
/// The pill's radio over `[library] order`.
const ORDER_ACTION: &str = "order";
/// The pill's check over `[library] pin_folders`.
const PIN_FOLDERS_ACTION: &str = "pin-folders";
/// The pill's radio over `[library] show_date`.
const SHOW_DATE_ACTION: &str = "show-date";
/// The pill's check over `[library] show_excerpts`.
const SHOW_EXCERPTS_ACTION: &str = "show-excerpts";
/// The pill's radio over `[library] mark`.
const MARK_ACTION: &str = "mark";
/// How wide the divider is to a pointer: the last logical pixels of the pane,
/// lying over its right edge rather than beside it, so that the page stands
/// where it stood and the strip has room to be caught.
const GRAB: i32 = 6;
/// What the pointer becomes over the divider: the name a desktop's cursor
/// theme keeps its two-headed horizontal arrow under.
const RESIZE_CURSOR: &str = "col-resize";
/// A column's head, the title bar's height, so that the Location's name and
/// the Document's stand on one line across the window.
const HEAD_HEIGHT: i32 = chrome::TOP_HEIGHT;
/// The head's insets and the air between its buttons (`.lib-head { padding: 0
/// 6px 0 8px; gap: 2px }`).
pub(crate) const HEAD_LEFT: i32 = 8;
/// What the head keeps clear of the right edge.
const HEAD_RIGHT: i32 = 6;
/// Between one thing in the head and the next.
const HEAD_GAP: i32 = 2;
/// A head button's side (`#library .lib-btn { width: 26px; height: 26px }`).
pub(crate) const BUTTON: i32 = 26;
/// A head button's corner (`border-radius: 5px`).
const BUTTON_RADIUS: i32 = 5;
/// The panel and plus marks in the head (`I.panel`, `I.plus`, fifteen by
/// fifteen).
const MARK: i32 = 15;
/// What the File List's head says where the Library has no Location to name.
const TITLE: &str = "Library";
/// The band the Sort pill stands in under the head (#441 § The Sort pill and
/// its menu).
const SORT_BAND: i32 = 33;
/// The Sort pill's least width and its height. It grows past the width to its
/// label: `Sort by Date Modified ⌄` is 161 points in Adwaita Sans (#437).
const SORT_PILL: (i32, i32) = (145, 25);
/// The Sort pill's type, which sets its capitals 17 device pixels tall.
const SORT_PX: f64 = 12.0;
/// The Sort pill's padding before its label and after its chevron.
const SORT_PAD: (i32, i32) = (12, 10);
/// Between the Sort pill's label and its chevron.
const SORT_GAP: i32 = 3;
/// A chevron's side (`I.chev`, eleven by eleven).
const CHEV: i32 = 11;
/// The Filter capsule's height (#441 § Search: 212 × 26 points).
const FILTER_HEIGHT: i32 = 26;
/// The Filter field's type, which sets its capitals 19 device pixels tall.
const FILTER_PX: f64 = 13.0;
/// Above the capsule, under its rule, and below it, over the pane's foot.
const FILTER_AIR: i32 = 7;
/// The capsule's inset from the File List's left edge: 9.5 points, at the
/// whole point the stub landed on the oracle's pixel with (#437).
const FILTER_LEFT: i32 = 10;
/// The capsule's inset from the pane's right edge.
const FILTER_RIGHT: i32 = 9;
/// The magnifier's side in the capsule.
const FILTER_ICON: i32 = 13;
/// The magnifier's inset from the capsule's rounded end.
const FILTER_ICON_LEFT: i32 = 8;
/// Between the magnifier and the prompt.
const FILTER_GAP: i32 = 4;
/// What the field's text keeps clear of the capsule's right end.
const FILTER_END: i32 = 10;
/// The magnifier as it is drawn (`I.search`, twelve by twelve), which
/// [`magnifier_icon`] scales to the side it is asked for.
const MAG: i32 = 12;
/// Above the File List's first row.
const LIST_TOP: i32 = 8;
/// A file row with its excerpt (State 28: 136 device pixels).
const ROW_PITCH: i32 = 68;
/// A folder row, and a file row with no excerpt to show (64 device pixels).
const FOLDER_PITCH: i32 = 32;
/// A page icon's inset from the File List's left edge.
const ICON_LEFT: i32 = 22;
/// How much further left a folder icon stands than a page, for the bearing its
/// wider mark leaves: an inset of 19 points.
const FOLDER_BEARING: i32 = 3;
/// Where a name's ink begins: 40.5 points, asked for at 40 so that the ink
/// lands on the oracle's 81 device pixels (#437).
const NAME_LEFT: i32 = 40;
/// Above a file row's name. A row with no excerpt stands it a point higher.
const NAME_TOP: i32 = 8;
/// Above a file row's page icon, a point under the name's top. A row with no
/// excerpt stands it a point higher.
const ICON_TOP: i32 = 9;
/// Between a name and the excerpt under it.
const EXCERPT_TOP: i32 = 1;
/// What the date's label keeps clear of the right edge, which stands its ink
/// 17.5 points in (#437).
const DATE_RIGHT: i32 = 15;
/// The least air between a name and its date.
const DATE_GAP: i32 = 12;
/// What a folder's chevron keeps clear of the right edge.
const CHEVRON_RIGHT: i32 = 16;
/// What the separator between two rows keeps clear of the right edge; its left
/// is the name's, [`NAME_LEFT`].
const SEPARATOR_RIGHT: i32 = 14;
/// What one folder of depth indents a row by.
const INDENT: i32 = 12;
/// A page icon's size in its row.
const DOC: (i32, i32) = (12, 15);
/// The page icon's width as it is drawn, which [`document_icon`] scales to the
/// width it is asked for.
const DOC_DRAWN: i32 = 13;
/// A folder icon's size (`I.folder`, fifteen by thirteen).
const FOLDER: (i32, i32) = (15, 13);
/// A row's type: the name, the date, the excerpt and a folder's name are one
/// size and differ only in ink, with capitals 20 device pixels tall and an
/// x-height of 15, the oracle's own (#441 § Type).
const ROW_PX: f64 = 13.5;
/// The title over the File List, bold, with capitals 22 device pixels tall.
const TITLE_PX: f64 = 15.0;
/// The excerpt's leading: 36 device pixels between its two lines in State
/// 28's frames (#448's round 12).
const EXCERPT_LEADING: f64 = 18.0;
/// How many lines of the excerpt a row shows, clipped at the last with no
/// ellipsis.
const EXCERPT_LINES: i32 = 2;
/// The type of the band's words above the Filter field.
const META_PX: f64 = 11.5;
/// Above and below a line of the band.
const BAND_AIR: i32 = 4;
/// Between the band's words.
const BAND_GAP: i32 = 7;
/// A button of the band's padding either side of its word.
const OFFER_PAD: i32 = 4;
/// What the band says of a Document changed on disk under unsaved edits,
/// before the two words it offers.
const CHANGED: &str = "Changed on disk";
/// The dot's side (`.lib-status .dot { width: 6px }`), which marks the row of
/// a file changed on disk.
const DOT: i32 = 6;
/// The selection's bar: 3 points with rounded ends, 8 points in from the File
/// List's left edge and 2 from the row's top and bottom, as State 28's frames
/// stand it — device columns 276…281 and 128 of a row's 136 rows (#448's
/// round 12; #441 § The selected row and the Selection Mark).
const BAR: Bar = Bar {
    width: 3,
    left: 8,
    top: 2,
    bottom: 2,
    radius: 2,
};

/// The accent bar down the selected row's left edge.
struct Bar {
    /// Its width.
    width: i32,
    /// How far in from the File List's edge it stands.
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
/// (`dev/legacy/app/css/files.css` line 212: `.lib-status[data-k="dirty"] .dot {
/// background: #e0a030 }`); the spec puts it on the row as well, so that a
/// writer scanning the pane can see which Document is waiting on them.
const WARN: &str = "#e0a030";

/// How far down the row the dot sits, so that it stands on the name's line
/// rather than at the row's top edge.
const DOT_TOP: i32 = 5;

/// The "·" the Reload / Keep band's words are separated by.
const SEPARATOR: &str = "·";

/// What the Filter field says while it is empty.
const PLACEHOLDER: &str = "Filter";

/// The class the Filter capsule wears while a query stands in it, which rings
/// it and shows its clear button (State 28's search frame, #448).
const LIVE_CLASS: &str = "lib-live";

/// The clear button's side at the capsule's right end.
const CLEAR: i32 = 14;

/// How much of the accent stands behind a row a dragged row would land on,
/// over the paper beneath it.
///
/// Stronger than a hover and weaker than a selection's bar: the light says
/// "here", and a whole Pinned section lit at hover strength would not read as
/// one target at all.
const DROP_TINT: f64 = 0.16;

/// The class a row wears while it is the target a drop would land on.
const DROP_CLASS: &str = "lib-drop";

/// The class the row of the Document the window holds wears, which is what
/// the bar and the Selection Mark are drawn on.
///
/// A class of its own rather than the list's selection: a click selects a
/// folder's row, and Enter and the arrows move the selection over rows that
/// are not open, while the bar marks the open Document and nothing else
/// (#441 § The selected row and the Selection Mark).
const OPEN_CLASS: &str = "lib-open";

/// The class on a File List row's own box, which [`OPEN_CLASS`] and
/// [`DROP_CLASS`] go on: the view's row around it is GTK's, and is bound to
/// whichever item scrolls into it (#460).
const ROW_CLASS: &str = "lib-row";

/// What a drop target is a place for, asked as a drag arrives over it: a
/// File List row answers for whichever folder is bound in it, and nothing for
/// a file ([`Sidebar::drop_onto`]).
type Aim = Rc<dyn Fn() -> Option<Onto>>;

/// The line inside an Organizer row that answers a click, and so the shape the
/// press's hit step is drawn on; heads and the empty-Pinned prose carry none.
const ORG_LINE_CLASS: &str = "lib-org-line";

/// The toggle that shuts the pane, which stands outside the pane's `.library`
/// root ([`Sidebar::toggle`]) and takes the head buttons' look from this.
const TOGGLE_CLASS: &str = "lib-toggle";

/// How long the field waits after a keystroke before it searches.
///
/// A content search reads every shown file whose name did not match, so the
/// field answers the writer's pause rather than the writer's typing; the
/// engine's cache then holds those texts, and the query after this one reads
/// nothing ([`Contents`]). Re-armed by each keystroke, so a word typed at
/// speed is one search. The search itself runs off the main thread
/// ([`Searching`]), so the wait only spares it work, never the window a hang.
const SETTLE_MS: u64 = 150;

/// How often the main thread looks for the search thread's answer while one is
/// owed, which is about a frame: the answer is drawn at the next look.
const ANSWER_POLL_MS: u64 = 16;

/// How much of a file the excerpt is taken from.
///
/// A bound rather than the whole file: the excerpt is two lines of type, and
/// a Library of a thousand Documents would otherwise read a thousand files
/// whole to draw them. Four kilobytes is more prose than two lines of even
/// the widest pane can show, and [`EXCERPT_CHARS`] is the cap on what is kept
/// of them.
const EXCERPT_BYTES: u64 = 4096;
/// How many characters of it a row keeps. The label ellipsizes at two lines
/// long before this at any width the divider can be dragged to: the pane
/// stops at 500 pt, which leaves the File List 300 (#459), and two lines of
/// that hold under a hundred characters of excerpt type. The cap is what the
/// label is laid out in full to find that out, once per row per rebuild:
/// 1200 characters cost about 12 ms a row, so a Location of 400 Documents
/// took five seconds to open (#441's second Hand test).
const EXCERPT_CHARS: usize = 300;

/// The months a date is named in, January first.
///
/// Spelled here rather than taken from the locale so that a judged shot reads
/// the same on a machine set to any language, as the rest of the chrome does.
const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// The sidebar's stylesheet, appended to the bars' ([`crate::chrome::stylesheet`]).
///
/// Every ground and grey the pane draws is a role of the theme's table — the
/// Organizer's ground, the File List's, the secondary grey of its dates, its
/// excerpts and its prompt, the ink its names are set in and the accent of its
/// bar — so a ground change is a stylesheet change and nothing else; its other
/// inks are [`PaneInks`], the capture's on the built-in grounds and derived
/// from the table on a writer's palette (#456).
///
/// Hover draws nothing and a selected row takes no fill (State 28, *A hovered
/// row draws nothing*), so the rules on a row's state clear GTK's own. The
/// Sort pill clears its background image as well as its colour, because GTK's
/// Default theme paints a gradient on a button that a colour alone does not
/// remove (#437). The Filter prompt stands at `opacity: 1`, because the same
/// theme halves the `placeholder` node (0.55), which left Quill's prompt at
/// `#BCBCBC` — the bug #379 was filed as.
#[must_use]
pub fn stylesheet(ground: Ground) -> String {
    let Ground { scheme, colours } = ground;
    let organizer = colours.colour(Role::OrganizerBg).to_hex();
    let list = colours.colour(Role::FileListBg).to_hex();
    let secondary = colours.colour(Role::Secondary).to_hex();
    let ink = colours.colour(Role::Ink).to_hex();
    let dim = colours.colour(Role::ChromeFg).to_hex();
    let strong = colours.colour(Role::ChromeFgStrong).to_hex();
    let accent = colours.colour(Role::Accent).to_hex();
    let drop = Colour::over(
        colours.colour(Role::Accent),
        colours.colour(Role::FileListBg),
        DROP_TINT,
    )
    .to_hex();
    // A head button under the pointer: a step off whatever ground it lands
    // on rather than a colour of its own, and so ink on the light ground and
    // paper on the dark one.
    let hit = match scheme {
        Scheme::Light => "rgba(0, 0, 0, 0.035)",
        Scheme::Dark => "rgba(255, 255, 255, 0.045)",
    };
    let inks = PaneInks::of(scheme, &colours);
    let [
        separator,
        sort_ground,
        sort_border,
        sort_ink,
        foot_rule,
        field_border,
    ] = [
        inks.separator,
        inks.sort_ground,
        inks.sort_border,
        inks.sort_ink,
        inks.foot_rule,
        inks.field_border,
    ]
    .map(Colour::to_hex);
    let [field_focus, field_clear, seam, head_rule, field_icon] = [
        inks.field_focus,
        inks.field_clear,
        inks.seam,
        inks.head_rule,
        inks.field_icon,
    ]
    .map(Colour::to_hex);
    let [org_head, org_ink, org_pill] =
        [inks.org_head, inks.org_ink, inks.org_pill].map(Colour::to_hex);
    let Bar {
        width: bar_width,
        left: bar_left,
        top: bar_top,
        bottom: bar_bottom,
        radius: bar_radius,
    } = BAR;
    let sort_radius = f64::from(SORT_PILL.1) / 2.0;
    let filter_radius = f64::from(FILTER_HEIGHT) / 2.0;
    let (sort_left, sort_right) = SORT_PAD;
    format!(
        ".library {{\n\
         \x20 background-color: {list}; color: {ink};\n\
         \x20 font-family: {CHROME_FONT}; font-size: {ROW_PX}px;\n\
         }}\n\
         .library .lib-org {{\n\
         \x20 background-color: {organizer}; box-shadow: inset -1px 0 {seam};\n\
         }}\n\
         .library .lib-head-rule {{ background-color: {head_rule}; }}\n\
         .library label.lib-org-head {{\n\
         \x20 font-size: {ORG_HEAD_PX}px; font-weight: bold; color: {org_head};\n\
         }}\n\
         .library label.lib-org-prose {{ font-size: {ORG_ROW_PX}px; color: {org_head}; }}\n\
         .library label.lib-org-row {{ font-size: {ORG_ROW_PX}px; color: {org_ink}; }}\n\
         .library .lib-org-icon {{ color: {org_ink}; }}\n\
         .library .lib-pill {{\n\
         \x20 background-color: {org_pill}; border-radius: {ORG_PILL_RADIUS}px;\n\
         }}\n\
         .library .lib-org row:active > .{ORG_LINE_CLASS} {{\n\
         \x20 background-image: linear-gradient({hit}, {hit});\n\
         \x20 border-radius: {ORG_PILL_RADIUS}px;\n\
         }}\n\
         .library .lib-pill label.lib-org-row {{\n\
         \x20 font-size: {ORG_PILL_PX}px; font-weight: bold;\n\
         }}\n\
         .library .lib-list {{ background-color: {list}; }}\n\
         .library .lib-rule {{ background-color: {separator}; }}\n\
         .library .lib-foot-rule {{ background-color: {foot_rule}; }}\n\
         .library label.lib-name {{ font-size: {ROW_PX}px; color: {ink}; }}\n\
         .library label.lib-head {{ font-size: {ROW_PX}px; color: {ink}; }}\n\
         .library label.lib-title {{\n\
         \x20 font-size: {TITLE_PX}px; font-weight: bold; color: {strong};\n\
         }}\n\
         .library label.lib-date {{\n\
         \x20 font-size: {ROW_PX}px; color: {secondary}; font-feature-settings: \"tnum\";\n\
         }}\n\
         .library label.lib-excerpt {{ font-size: {ROW_PX}px; color: {secondary}; }}\n\
         .library label.lib-meta {{ font-size: {META_PX}px; color: {secondary}; }}\n\
         .library .lib-icon {{ color: {secondary}; }}\n\
         .library .lib-folder-icon {{ color: {accent}; }}\n\
         .library .lib-changed {{ color: {WARN}; }}\n\
         .library button.lib-btn, .{TOGGLE_CLASS} button.lib-btn {{\n\
         \x20 background: none; border: none; box-shadow: none; outline: none;\n\
         \x20 min-height: 0; min-width: 0; padding: 0;\n\
         \x20 border-radius: {BUTTON_RADIUS}px; color: {dim};\n\
         }}\n\
         .library button.lib-btn:hover, .{TOGGLE_CLASS} button.lib-btn:hover {{\n\
         \x20 background-color: {hit}; color: {ink};\n\
         }}\n\
         .library button.lib-offer {{\n\
         \x20 font-size: {META_PX}px; padding: 0 {OFFER_PAD}px;\n\
         }}\n\
         .library button.lib-sortb {{\n\
         \x20 background-color: {sort_ground}; background-image: none;\n\
         \x20 border: 1px solid {sort_border}; box-shadow: none; outline: none;\n\
         \x20 min-height: 0; min-width: 0; padding: 0 {sort_right}px 0 {sort_left}px;\n\
         \x20 border-radius: {sort_radius}px; color: {sort_ink};\n\
         }}\n\
         .library button.lib-sortb label {{ font-size: {SORT_PX}px; color: {sort_ink}; }}\n\
         .library button.lib-sortb .lib-icon {{ color: {sort_ink}; }}\n\
         .library .lib-filter {{\n\
         \x20 background-color: {list}; border: 1px solid {field_border};\n\
         \x20 border-radius: {filter_radius}px;\n\
         }}\n\
         .library .lib-filter.{LIVE_CLASS} {{\n\
         \x20 border-color: {field_focus}; box-shadow: 0 0 0 2px {field_focus};\n\
         }}\n\
         .library .lib-filter .lib-clear {{ color: {field_clear}; }}\n\
         .library .lib-filter .lib-icon {{ color: {field_icon}; }}\n\
         .library entry {{\n\
         \x20 background: none; border: none; box-shadow: none; outline: none;\n\
         \x20 padding: 0; margin: 0; min-height: 0; min-width: 0;\n\
         \x20 font-size: {FILTER_PX}px; color: {ink}; caret-color: {accent};\n\
         }}\n\
         .library entry text > placeholder {{ color: {secondary}; opacity: 1; }}\n\
         .library entry.lib-rename {{ font-size: {ROW_PX}px; }}\n\
         .library scrolledwindow, .library list, .library listview {{ background: none; }}\n\
         .library list > row {{\n\
         \x20 background: none; padding: 0; min-height: 0; outline: none;\n\
         }}\n\
         .library list > row:hover {{ background: none; }}\n\
         .library list > row:selected {{ background: none; }}\n\
         .library listview > row {{\n\
         \x20 background: none; padding: 0; min-height: 0; outline: none;\n\
         }}\n\
         .library listview > row:hover {{ background: none; }}\n\
         .library listview > row:selected {{ background: none; }}\n\
         .library .lib-bar {{\n\
         \x20 background: none; border-radius: {bar_radius}px;\n\
         \x20 min-width: {bar_width}px; margin: {bar_top}px 0 {bar_bottom}px {bar_left}px;\n\
         }}\n\
         .library .{ROW_CLASS}.{OPEN_CLASS} .lib-bar {{ background-color: {accent}; }}\n\
         .library .lib-mark {{\n\
         \x20 color: transparent; margin: {bar_top}px 0 {bar_bottom}px 0;\n\
         }}\n\
         .library .{ROW_CLASS}.{OPEN_CLASS} .lib-mark {{ color: {accent}; }}\n\
         .library list > row.{DROP_CLASS}, .library .{ROW_CLASS}.{DROP_CLASS} {{\n\
         \x20 background-color: {drop};\n\
         }}\n\
         .library :drop(active) {{ box-shadow: none; outline: none; }}\n"
    )
}

/// One row the File List lists, and everything its row says but the file's
/// head, which is read when the row is first bound ([`head_of`]).
///
/// A refresh that changes the listing builds one of these for every listed
/// file and folder, which is a few strings each; the widgets are built only
/// for the rows in view, once a slot, and each is bound to whichever item
/// scrolls into it ([`Sidebar::bind`], #460).
#[derive(Clone, Debug, PartialEq)]
struct Item {
    /// The file or folder it draws.
    path: PathBuf,
    /// Whether it is a folder, which opens and closes rather than opening a
    /// Document.
    folder: bool,
    /// How many folders below its section it lies; a result is drawn flat.
    depth: usize,
    /// The name the row shows: a file's without its extension unless the
    /// writer asked for them ([`row_name`]).
    name: String,
    /// Whether a folder is open; `false` for a file.
    open: bool,
    /// The date a file's row says, where `library.show_date` asks for one and
    /// the clock could be asked.
    date: Option<String>,
    /// When the file was last written, which says whether the head held for
    /// it is still the one on disk.
    modified: Option<SystemTime>,
    /// A content hit's snippet, drawn in place of the file's own excerpt, and
    /// the bytes of it the query matched. A tree row and a name hit have none,
    /// and show the excerpt.
    snippet: Option<(String, Range<usize>)>,
    /// Whether a file's page carries a pin.
    page: Page,
}

/// The widgets of one row in view, built once and bound to whichever
/// [`Item`] the File List puts in the slot.
///
/// Both lines are built, a folder's and a file's, and the one the item is not
/// is hidden; every field is a handle on a widget or on what a drawing reads,
/// so a clone is the same slot.
#[derive(Clone)]
struct Slot {
    /// The row: the line under the bar, and the separator under both. What a
    /// drag carries and a drop lights, and what [`OPEN_CLASS`] goes on.
    root: gtk::Box,
    /// A folder's line: its icon, its name and its chevron.
    folder_line: gtk::Box,
    folder_icon: gtk::DrawingArea,
    folder_name: gtk::Label,
    chevron: gtk::DrawingArea,
    /// Whether the chevron draws open.
    chevron_open: Rc<Cell<bool>>,
    /// A file's line: the page, the name and the date over the excerpt, and
    /// the dot.
    file_line: gtk::Box,
    page: gtk::DrawingArea,
    /// Whether the page draws its pin, and where the pin stands.
    page_is: Rc<Cell<Page>>,
    pose: Rc<Cell<PinPose>>,
    body: gtk::Box,
    /// The name and the date: where a rename's field stands in for the name.
    top: gtk::Box,
    /// The label the file's name is drawn in, which a rename hides and puts
    /// a field in the place of ([`Sidebar::start_rename`]).
    title: gtk::Label,
    date: gtk::Label,
    excerpt: gtk::Inscription,
    /// The dot that says this row's file changed on disk. Shown only while
    /// this is the row of the Document the window holds and that Document is
    /// in a conflict ([`Sidebar::set_conflicted`]).
    dot: gtk::DrawingArea,
    bar: gtk::Box,
    /// The Selection Mark's glyph where one stands in for the bar, and which.
    mark: gtk::DrawingArea,
    glyph: Rc<Cell<Option<&'static Glyph>>>,
    rule: gtk::Box,
    /// Which of the pane's items is bound here, by its place in the list.
    bound: Rc<Cell<Option<usize>>>,
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
    /// What stands beside the page ([`Sidebar::widget`]): the pane with the
    /// divider over its right edge, slid in from the window's left edge and
    /// out again (#485). It is the whole overlay that slides, since an
    /// overlay whose pane is away would still take the divider's room.
    frame: gtk::Revealer,
    /// The toggle that shuts the pane ([`Sidebar::toggle`]), which stands
    /// over the window where the Organizer's head leaves room for it rather
    /// than in the head, so that the slide never carries it from under the
    /// pointer that pressed it (#485).
    toggle: gtk::Box,
    root: gtk::Box,
    /// The Organizer's column, whose width [`Sidebar::set_width`] keeps at
    /// [`organizer_width`] of the pane's.
    organizer: gtk::Box,
    /// The divider: a strip of [`GRAB`] pixels on the pane's right edge with
    /// nothing in it and nothing drawn, which a drag widens the pane by and a
    /// double-click puts back to [`WIDTH`].
    divider: gtk::Box,
    /// What the File List's head calls it: the name of the Location it shows,
    /// or [`TITLE`] where the Library has none.
    title: gtk::Label,
    entry: gtk::Entry,
    sort_label: gtk::Label,
    /// The File List: a view over [`Sidebar::store`], which builds the rows in
    /// view and binds each to an item as it scrolls in (#460).
    list: gtk::ListView,
    /// One object per listed item, in order; the item itself is
    /// [`Sidebar::items`] at the object's place.
    store: gio::ListStore,
    /// What Enter opens and a row operation acts on, which the open
    /// Document's row is and the arrows move.
    selection: gtk::SingleSelection,
    /// What each listed row says, in the order the list draws them.
    items: Rc<RefCell<Vec<Item>>>,
    /// Every row's widgets the list has asked for, bound or waiting.
    slots: Rc<RefCell<Vec<Slot>>>,
    /// Whether a file's row carries its excerpt, and what marks the open
    /// Document's row: the two settings a bind reads that no item holds.
    shown: Rc<Cell<(bool, Mark)>>,
    /// The pin being run, and where it stands, so that a page bound part-way
    /// through takes the pose the others have ([`Sidebar::run_pin`]).
    pin_now: Rc<RefCell<Option<(PathBuf, PinPose)>>>,
    /// The Sort pill, which Recents disables.
    sort_button: gtk::Button,
    /// The Organizer's rows: the three section heads and what stands under
    /// each.
    org: gtk::ListBox,
    /// Each Organizer row now drawn, with what it stands for.
    organized: Rc<RefCell<Vec<(gtk::ListBoxRow, Organized)>>>,
    /// What the File List shows, as the Organizer last chose it; `None` where
    /// the Library has no Location. Settled against the Library at every
    /// [`Sidebar::refresh`].
    showing: Rc<RefCell<Option<Showing>>>,
    /// The band's line above the Filter field: a notice, hidden while there is
    /// none ([`Sidebar::set_notice`]).
    notice: gtk::Label,
    /// "Changed on disk · Reload · Keep" in the band above the Filter field,
    /// hidden until there is a conflict to resolve.
    offer: gtk::Box,
    reload: gtk::Button,
    keep: gtk::Button,
    /// Whether the Document the window holds is changed on disk, which is what
    /// the dot on its row says.
    conflicted: Rc<Cell<bool>>,
    /// The window this sidebar belongs to, so a row can open a Document in
    /// it. Weak, because the window owns the sidebar.
    window: Rc<RefCell<Option<glib::WeakRef<Window>>>>,
    /// The Location rows now drawn, each with the Location it names, so that a
    /// right-click on one can offer to drop that Location: the Organizer's
    /// Location rows, the File List's head carrying its own
    /// ([`Sidebar::head_as_location`]).
    heads: Rc<RefCell<Vec<(gtk::ListBoxRow, PathBuf)>>>,
    /// The folders the writer has opened. Everything else is closed, which is
    /// what the spec asks a section to open at.
    expanded: Rc<RefCell<BTreeSet<PathBuf>>>,
    /// The Sort pill's menu, parented once on the pill and kept for the reason
    /// [`Sidebar::menu`] is.
    sort_menu: gtk::PopoverMenu,
    /// The pill's [`PILL_GROUP`] actions, whose states are put back to what
    /// `[library]` holds at every [`Sidebar::refresh`], so the radios and the
    /// checks read the file rather than the last click.
    sort_actions: gio::SimpleActionGroup,
    /// The Document the window is showing, whose row is the highlighted one.
    open: Rc<RefCell<Option<PathBuf>>>,
    /// The File List's head, which is the header of the Location it shows
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
    /// The query's search, run on its own thread, with the file texts it has
    /// read kept for as long as the window is, so that a query over a tree
    /// nothing has touched reads nothing.
    searching: Rc<RefCell<Searching>>,
    /// What each shown file's first lines said, by path, so that a refresh
    /// over files nothing has written reads nothing ([`Sidebar::refresh`]).
    read: Rc<RefCell<BTreeMap<PathBuf, Head>>>,
    /// What the File List's rows were last built from, so that a refresh that
    /// would build the same rows again leaves them standing and only moves the
    /// highlight ([`Sidebar::refresh`]).
    picture: Rc<RefCell<Option<Picture>>>,
    /// The keystroke the field is waiting out before it searches
    /// ([`SETTLE_MS`]).
    settle: Rc<RefCell<Option<glib::SourceId>>>,
    /// The pin a menu or a drop has just asked for, which the next
    /// [`Sidebar::refresh`] starts on the rows it draws for it.
    pinning: Rc<RefCell<Option<Pinning>>>,
    /// Each pinned page now drawn, with where its pin stands, so that a pin can
    /// be run in or out on the rows that show it ([`Sidebar::run_pin`]).
    pins: Rc<RefCell<Vec<PinnedPage>>>,
    /// Where the row being dragged was grabbed, in the pane's coordinates,
    /// which a drop reads the row's travel from.
    grabbed: Rc<Cell<Option<(f64, f64)>>>,
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
        let root = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        root.add_css_class("library");
        root.set_width_request(WIDTH);
        // The pane is exactly its width and the page takes the rest of the
        // window: said here rather than left to the children, because the list
        // inside it expands and a box takes its children's answer.
        root.set_hexpand(false);

        // Two columns, the one a change of ground from the other (ADR 0020):
        // the Organizer taking half of what the divider adds, with the toggle
        // that shuts the pane in its head, and the File List taking the rest.
        let organizer = gtk::Box::new(gtk::Orientation::Vertical, 0);
        organizer.add_css_class("lib-org");
        organizer.set_width_request(ORGANIZER);
        organizer.set_hexpand(false);
        organizer.append(&organizer_head());
        let org = gtk::ListBox::new();
        org.set_selection_mode(gtk::SelectionMode::None);
        let org_scroller = gtk::ScrolledWindow::builder()
            .vexpand(true)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .child(&org)
            .build();
        organizer.append(&org_scroller);
        root.append(&organizer);
        let file_list = gtk::Box::new(gtk::Orientation::Vertical, 0);
        file_list.add_css_class("lib-list");
        file_list.set_hexpand(true);
        root.append(&file_list);

        let title = gtk::Label::new(Some(TITLE));
        let head = head(&title);
        file_list.append(&head);

        let sort_label = gtk::Label::new(Some(sort_title(Sort::Modified)));
        let sort_button = sort_button(&sort_label);
        file_list.append(&sort_row(&sort_button));
        let head_rule = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        head_rule.add_css_class("lib-head-rule");
        head_rule.set_height_request(1);
        file_list.append(&head_rule);

        // A view rather than a box of rows: a Location of four hundred files
        // built every row's widgets at each refresh, 1–2.4 ms a row, where the
        // view builds a screenful and binds each to the item scrolled into it
        // (#460). The list's air above its first row is that row's margin
        // ([`Sidebar::bind`]), so that it scrolls away with the rows as a box's
        // margin inside a scrolled window did.
        let store = gio::ListStore::new::<glib::BoxedAnyObject>();
        let selection = gtk::SingleSelection::new(Some(store.clone()));
        selection.set_autoselect(false);
        selection.set_can_unselect(true);
        let list = gtk::ListView::new(Some(selection.clone()), None::<gtk::ListItemFactory>);
        let scroller = gtk::ScrolledWindow::builder()
            .vexpand(true)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .child(&list)
            .build();
        file_list.append(&scroller);

        // The foot: the band a notice or a conflict stands in, empty at rest,
        // over the rule and the Filter field.
        let notice = gtk::Label::new(None);
        let reload = offer_word("Reload");
        let keep = offer_word("Keep");
        let offer = offered(&reload, &keep);
        file_list.append(&band(&notice, &offer));
        let entry = gtk::Entry::builder()
            .hexpand(true)
            .has_frame(false)
            .placeholder_text(PLACEHOLDER)
            .build();
        file_list.append(&filter(&entry));

        // On the pane and not on the list: a `GtkListBox` takes only rows
        // away, so a popover parented on it is a child [`Sidebar::refresh`]
        // cannot remove and its loop over the children never ends. A popover
        // is a `GtkNative` and no layout manager gives it room, so the pane's
        // own column is unmoved by it.
        let menu = menu_popover(&root);
        let head_menu = menu_popover(&head);
        let sort_menu = menu_popover(&root);
        // The divider lies over the pane's last [`GRAB`] pixels rather than
        // standing beside them: an overlay child is given room without taking
        // any, so the page begins where it always did and a state judged at
        // [`WIDTH`] is the pixels it was. It holds nothing and is styled by
        // nothing, so there is nothing for it to draw.
        let divider = gtk::Box::new(gtk::Orientation::Vertical, 0);
        divider.set_width_request(GRAB);
        divider.set_halign(gtk::Align::End);
        divider.set_cursor_from_name(Some(RESIZE_CURSOR));
        let overlay = gtk::Overlay::new();
        overlay.set_child(Some(&root));
        overlay.add_overlay(&divider);
        // The oracle's `#library` slide, `translateX(-100%)` to `none`, with
        // `#app`'s `padding-left` in step: the Revealer gives the pane its full
        // width and shows the part of it that has come in, while its own width
        // grows with that part, so the page is given a narrower window, and
        // recentres, on every frame of the slide. It eases out cubic over
        // [`MOTION_MS`], turns back from where it stands, and does not move
        // while the window is unmapped or `gtk-enable-animations` is off,
        // which it reads for itself, so a window opening with the pane, a
        // harness shot and a desktop asking for no motion all have it at rest.
        let frame = gtk::Revealer::builder()
            .transition_type(gtk::RevealerTransitionType::SlideRight)
            .transition_duration(MOTION_MS)
            .reveal_child(false)
            .child(&overlay)
            .hexpand(false)
            .build();
        // Stood exactly where the Organizer's head would hold it, so the
        // pane at rest draws as it did with the toggle inside it.
        let toggle = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        toggle.add_css_class(TOGGLE_CLASS);
        toggle.set_height_request(HEAD_HEIGHT);
        toggle.set_margin_start(HEAD_LEFT);
        toggle.set_halign(gtk::Align::Start);
        toggle.set_valign(gtk::Align::Start);
        toggle.append(&button(panel_icon, "win.library.toggle"));
        toggle.set_visible(false);
        let sidebar = Self {
            frame,
            toggle,
            divider,
            root,
            organizer,
            head,
            title,
            entry,
            sort_label,
            list,
            store,
            selection,
            items: Rc::new(RefCell::new(Vec::new())),
            slots: Rc::new(RefCell::new(Vec::new())),
            shown: Rc::new(Cell::new((true, Mark::default()))),
            pin_now: Rc::new(RefCell::new(None)),
            sort_button: sort_button.clone(),
            org,
            organized: Rc::new(RefCell::new(Vec::new())),
            showing: Rc::new(RefCell::new(None)),
            notice,
            offer,
            reload,
            keep,
            conflicted: Rc::new(Cell::new(false)),
            window: Rc::new(RefCell::new(None)),
            heads: Rc::new(RefCell::new(Vec::new())),
            expanded: Rc::new(RefCell::new(BTreeSet::new())),
            sort_menu,
            sort_actions: gio::SimpleActionGroup::new(),
            open: Rc::new(RefCell::new(None)),
            header: Rc::new(RefCell::new(None)),
            head_drop: Rc::new(RefCell::new(None)),
            menu,
            head_menu,
            searching: Rc::new(RefCell::new(Searching::new())),
            read: Rc::new(RefCell::new(BTreeMap::new())),
            picture: Rc::new(RefCell::new(None)),
            settle: Rc::new(RefCell::new(None)),
            pinning: Rc::new(RefCell::new(None)),
            pins: Rc::new(RefCell::new(Vec::new())),
            grabbed: Rc::new(Cell::new(None)),
        };
        // The model is handed over as the menu opens, as the row menu's is
        // ([`Sidebar::popup`]), on the pane's root where the row menu stands:
        // a popover parented on the pill, or given its model before the pane's
        // [`PILL_GROUP`] is inserted, tracks every row as an action missing
        // from the start and draws each one greyed and inert (#446).
        let sorting = sidebar.clone();
        sort_button.connect_clicked(move |button| {
            let under = button
                .compute_bounds(&sorting.root)
                .map_or((0.0, 0.0), |at| (at.x(), at.y() + at.height()));
            sorting.popup(
                &pill_menu(),
                &sorting.sort_menu,
                f64::from(under.0),
                f64::from(under.1),
            );
        });
        let toggle = sidebar.toggle.clone();
        sidebar.connect_on_screen(move |on_screen| toggle.set_visible(on_screen));
        sidebar.wire();
        sidebar
    }

    /// The search field's keys.
    ///
    /// Typing narrows the list once the keystrokes stop, Enter opens the
    /// highlighted hit, Esc clears the query and hands the keyboard back to
    /// the page, and Down steps into the list, where the arrows walk the rows
    /// (`dev/legacy/app/js/files.js`, the field's `keydown`).
    fn wire(&self) {
        self.list.set_factory(Some(&self.slots_factory()));
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

    /// The toggle that shuts the pane, to stand over the window's top left
    /// corner, above the pane's head: shown while any of the pane is on
    /// screen, so it stays where it was pressed through the slide either way.
    #[must_use]
    pub fn toggle(&self) -> &gtk::Widget {
        self.toggle.upcast_ref()
    }

    /// Runs `f` with whether any of the pane is on screen, each time that may
    /// have changed: as soon as it is asked in, and not until its slide out
    /// has ended.
    pub fn connect_on_screen(&self, f: impl Fn(bool) + 'static) {
        let f = Rc::new(f);
        for property in ["reveal-child", "child-revealed"] {
            let f = f.clone();
            self.frame
                .connect_notify_local(Some(property), move |frame, _| {
                    f(frame.reveals_child() || frame.is_child_revealed());
                });
        }
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
        // Enter on a row. A click is the row's own ([`Sidebar::slot`]), since a
        // view activates on a double click and would open a Document the
        // writer only meant to rename.
        let activated = self.clone();
        self.list.connect_activate(move |_, position| {
            if let Ok(at) = usize::try_from(position) {
                activated.activate(at);
            }
        });
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
        // A second click on a file's row renames it where it stands, and on a
        // folder's opens or closes it again, and the right
        // button opens what can be done to it. Both watch the list in the
        // capture phase, so that the press they take is one the list itself
        // never sees: the first click has already opened the row, and opening
        // it again on the second would put the caret back to the top of a
        // Document the writer is only renaming.
        let doubles = gtk::GestureClick::new();
        doubles.set_button(gdk::BUTTON_PRIMARY);
        doubles.set_propagation_phase(gtk::PropagationPhase::Capture);
        let renaming = self.clone();
        doubles.connect_pressed(move |gesture, presses, x, y| {
            if presses < 2 {
                return;
            }
            gesture.set_state(gtk::EventSequenceState::Claimed);
            renaming.second_press_at(x, y);
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
        self.install_pill_actions();
        // A click in the Organizer changes what the File List shows or opens a
        // pinned Document, and the right button on a Location's row offers
        // what the File List's head does.
        let chosen = self.clone();
        self.org
            .connect_row_activated(move |_, row| chosen.choose(row));
        let org_menued = gtk::GestureClick::new();
        org_menued.set_button(gdk::BUTTON_SECONDARY);
        org_menued.set_propagation_phase(gtk::PropagationPhase::Capture);
        let org_opening = self.clone();
        org_menued.connect_pressed(move |gesture, _, x, y| {
            gesture.set_state(gtk::EventSequenceState::Claimed);
            org_opening.org_menu_at(x, y);
        });
        self.org.add_controller(org_menued);
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

    /// Whether the pane is open: the state last asked for, so a pane still
    /// sliding out answers no and one still sliding in answers yes.
    #[must_use]
    pub fn is_shown(&self) -> bool {
        self.frame.reveals_child()
    }

    /// Slides the pane in or out (#485). The page keeps its own centring and
    /// is simply given a narrower window, as the oracle's is.
    ///
    /// A pane sliding out takes neither the pointer nor the keyboard, so a
    /// click in its last quarter second cannot leave the keyboard in a pane
    /// that is about to be gone.
    pub fn set_shown(&self, shown: bool) {
        self.frame.set_can_target(shown);
        self.frame.set_can_focus(shown);
        self.frame.set_reveal_child(shown);
    }

    /// Whether the keyboard is somewhere in the pane.
    #[must_use]
    pub fn holds_focus(&self) -> bool {
        self.frame
            .root()
            .and_then(|root| root.focus())
            .is_some_and(|focus| focus.is_ancestor(&self.frame))
    }

    /// Stands the pane at `width` logical pixels.
    ///
    /// The width the drag arrived at, already pulled into range by the window
    /// that took the drag ([`crate::window::Window::resize_library`]); the
    /// pane asks for it and the page takes what is left, which is how the
    /// pane has always been sized. The Organizer takes its share of it
    /// ([`organizer_width`]) and the File List the rest.
    pub fn set_width(&self, width: u32) {
        let width = i32::try_from(width).unwrap_or(WIDTH);
        self.root.set_width_request(width);
        self.organizer.set_width_request(organizer_width(width));
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

    /// Stands `words` in the band above the Filter field, or takes the line
    /// down where they are empty.
    ///
    /// A notice rather than a state — a trash, an export's confirmation, the
    /// missing dictionary — because the saved states say nothing any more
    /// (#441 § The foot and the title bar).
    pub fn set_notice(&self, words: &str) {
        self.notice.set_text(words);
        self.notice.set_visible(!words.is_empty());
    }

    /// Whether the band above the Filter field offers Reload and Keep, which it
    /// does while the Document the window holds changed on disk under unsaved
    /// edits.
    pub fn set_offer(&self, offered: bool) {
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
        // Over Recents the Document just opened is the newest of them, so the
        // list is drawn again, once the opening has been taken in.
        if self.showing.borrow().as_ref() == Some(&Showing::Recents) {
            let drawing = self.clone();
            glib::idle_add_local_once(move || drawing.refresh());
        }
        self.highlight();
    }

    /// What the tree is read through: the writer's `[library]` table
    /// ([`view_of`]). The Palette's Outline reads the Library through the same
    /// view, so its Documents fall in the order the sidebar shows them (#397).
    pub(crate) fn view(&self, session: &crate::session::Session) -> View {
        view_of(&session.settings().library)
    }

    /// Draws the Library as it is now: the Organizer, and in the File List
    /// the one section it chose, from the tree the session holds.
    ///
    /// The rows are built rather than patched, because a watch event can have
    /// moved a row from one folder to another; but only where what they would
    /// be built from moved ([`Picture`]), so that a refresh which changes
    /// nothing a row shows — a save of a Document the list does not show, a
    /// settings save the pane does not read — moves the highlight alone. What
    /// is read and sorted is the listed section's, never the whole Library's
    /// (#453).
    ///
    /// A query lists its name hits at once and its content hits when the
    /// search thread answers, which draws the list again (#454).
    pub fn refresh(&self) {
        self.draw(false);
    }

    /// [`Sidebar::refresh`], with a query's content search run here and now
    /// where `at_once`, rather than on the search thread.
    fn draw(&self, at_once: bool) {
        let Some(session) = self.session() else {
            return;
        };
        let library = session.library();
        let view = self.view(&session);
        let shown = session.settings().library.clone();
        sync_pill(&self.sort_actions, &shown);
        let locations: Vec<&Path> = library.locations().iter().map(|at| at.path()).collect();
        // Each pinned entry's own row, which is all the Organizer draws of it:
        // a folder's subtree is the File List's to sort, once it is chosen.
        let pinned: Vec<(&Path, bool)> = library
            .pinned()
            .iter()
            .filter_map(|path| library.at(path))
            .map(|row| (row.path(), matches!(row, Row::Folder { .. })))
            .collect();
        let showing = settled(self.showing.borrow().as_ref(), &locations, &pinned);
        self.showing.replace(showing.clone());
        let organizer = organized(&locations, &pinned, showing.as_ref(), shown.show_extensions);
        let same = self
            .organized
            .borrow()
            .iter()
            .map(|(_, row)| row)
            .eq(organizer.iter());
        if !same {
            self.heads.borrow_mut().clear();
            let org = self.org.clone().upcast::<gtk::Widget>();
            self.pins
                .borrow_mut()
                .retain(|(_, page, _)| !page.is_ancestor(&org));
            self.organize(&organizer);
        }
        // The File List's head names what it shows, and is the header of a
        // Location's tree alone: what its menu offers, and what a row let go
        // over it moves into (#246, stories 3 and 38).
        self.title.set_text(&match &showing {
            Some(Showing::Location(path) | Showing::Folder(path)) => place_name(path),
            Some(Showing::Recents) => RECENTS.to_string(),
            None => TITLE.to_string(),
        });
        self.header.replace(match &showing {
            Some(Showing::Location(path)) => Some(path.clone()),
            _ => None,
        });
        self.head_as_location();
        let query = self.query();
        let (label, enabled) = pill_reads(showing.as_ref(), &query, view.sort);
        self.sort_label.set_text(label);
        self.sort_button.set_sensitive(enabled);
        enable_pill(&self.sort_actions, enabled);
        let opened = match showing {
            Some(Showing::Recents) => session.recents(),
            _ => Vec::new(),
        };
        let matches = self.matches(&library, &query, view, at_once);
        let listing = listing(
            &library,
            showing.as_ref(),
            &query,
            &view,
            &opened,
            matches.as_deref(),
            &self.expanded.borrow(),
        );
        let now = glib::DateTime::now_local().ok();
        let picture = Picture::of(
            &listing,
            &query,
            &shown,
            library.pinned(),
            now.as_ref(),
            &self.expanded.borrow(),
        );
        if self.picture.borrow().as_ref() != Some(&picture) {
            prune_heads(&mut self.read.borrow_mut(), listing.files());
            let items = items_of(
                &listing,
                &shown,
                library.pinned(),
                now.as_ref(),
                &self.expanded.borrow(),
            );
            self.shown.set((shown.show_excerpts, shown.mark));
            self.relist(items);
            self.picture.replace(Some(picture));
        }
        self.highlight();
        self.start_pinning();
    }

    /// Puts `items` in the File List in place of what it held. Every slot lets
    /// go of its row, and the view binds the slots in view to the new items as
    /// it lays them out, which is all that is built (#460).
    fn relist(&self, items: Vec<Item>) {
        for slot in self.slots.borrow().iter() {
            self.unbind(slot);
        }
        let count = items.len();
        self.items.replace(items);
        let objects: Vec<glib::BoxedAnyObject> =
            (0..count).map(glib::BoxedAnyObject::new).collect();
        self.store.splice(0, self.store.n_items(), &objects);
    }

    /// Draws the Organizer's `rows`: each Location's row one of the heads its
    /// menu is found by, and the whole Pinned section, head and prose and
    /// rows, the one target a dragged row is pinned by.
    fn organize(&self, rows: &[Organized]) {
        while let Some(child) = self.org.first_child() {
            self.org.remove(&child);
        }
        let mut organized = self.organized.borrow_mut();
        organized.clear();
        let mut pinned: Vec<gtk::Widget> = Vec::new();
        let mut in_pinned = false;
        for (at, row) in rows.iter().enumerate() {
            let drawn = match row {
                Organized::Head(name) => {
                    in_pinned = *name == PINNED;
                    org_head_row(name, at > 0)
                }
                Organized::Location { path, name, on } => {
                    let drawn = org_row(org_folder(), name, *on);
                    self.heads.borrow_mut().push((drawn.clone(), path.clone()));
                    drawn
                }
                Organized::Pinned { path, name, on, .. } => {
                    let mark = match org_page(row) {
                        Some(page) => {
                            let page = self.page_mark(path, page);
                            page.add_css_class("lib-org-icon");
                            page
                        }
                        None => org_folder(),
                    };
                    org_row(mark, name, *on)
                }
                Organized::NothingPinned => org_prose(NOTHING_PINNED),
                Organized::Recents { on } => {
                    let clock = icon(CLOCK, clock_icon);
                    clock.add_css_class("lib-org-icon");
                    org_row(clock, RECENTS, *on)
                }
            };
            if in_pinned {
                pinned.push(drawn.clone().upcast());
            }
            self.org.append(&drawn);
            organized.push((drawn, row.clone()));
        }
        drop(organized);
        let lit = Rc::new(pinned);
        let over = Rc::new(Cell::new(0));
        for widget in lit.iter() {
            self.drop_onto(widget, Rc::new(|| Some(Onto::Pinned)), &lit, &over);
        }
    }

    /// A click on an Organizer row: a Location, a pinned folder or Recents
    /// fills the File List and leaves the open Document where it is; a pinned
    /// Document opens.
    fn choose(&self, row: &gtk::ListBoxRow) {
        let chosen = self
            .organized
            .borrow()
            .iter()
            .find(|(drawn, _)| drawn == row)
            .map(|(_, organized)| organized.clone());
        let showing = match chosen {
            Some(Organized::Location { path, .. }) => Showing::Location(path),
            Some(Organized::Pinned {
                path, folder: true, ..
            }) => Showing::Folder(path),
            Some(Organized::Recents { .. }) => Showing::Recents,
            Some(Organized::Pinned {
                path,
                folder: false,
                ..
            }) => {
                if let Some(window) = self.owner() {
                    window.open_path(&path);
                }
                return;
            }
            Some(Organized::Head(_) | Organized::NothingPinned) | None => return,
        };
        self.showing.replace(Some(showing));
        self.refresh();
    }

    /// The right button over the Organizer: a Location's row offers what the
    /// File List's head does.
    fn org_menu_at(&self, x: f64, y: f64) {
        let Some(location) = self
            .org
            .row_at_y(pixels(y))
            .and_then(|row| self.head_at(&row))
        else {
            return;
        };
        let on_pane = self.org.compute_point(&self.root, &place(x, y));
        let (x, y) = on_pane.map_or((x, y), |point| (f64::from(point.x()), f64::from(point.y())));
        self.popup(&location_menu(&location), &self.menu, x, y);
    }

    /// A folder's row, a Location's head or the pane's own head as a place a
    /// dragged row can be moved into, lighting alone.
    fn folder_target(&self, on: &impl IsA<gtk::Widget>, folder: &Path) -> gtk::DropTarget {
        let lit = Rc::new(vec![on.clone().upcast()]);
        let over = Rc::new(Cell::new(0));
        let folder = folder.to_path_buf();
        self.drop_onto(
            on,
            Rc::new(move || Some(Onto::Folder(folder.clone()))),
            &lit,
            &over,
        )
    }

    /// A File List row as a place a dragged row can be moved into, answering
    /// for whichever folder the list has bound in `slot`; a file's row takes
    /// nothing.
    fn slot_target(&self, slot: &Slot) {
        let lit = Rc::new(vec![slot.root.clone().upcast()]);
        let over = Rc::new(Cell::new(0));
        let items = Rc::clone(&self.items);
        let bound = Rc::clone(&slot.bound);
        let aim: Aim = Rc::new(move || {
            let items = items.borrow();
            let item = items.get(bound.get()?)?;
            item.folder.then(|| Onto::Folder(item.path.clone()))
        });
        self.drop_onto(&slot.root, aim, &lit, &over);
    }

    /// Puts what `onto` answers under `row`: what letting a dragged row go
    /// there does, and which rows light while the pointer is over it.
    ///
    /// `onto` is asked as the drag arrives rather than once, because a File
    /// List row is bound to whatever the list scrolls into it ([`Aim`]).
    /// `lit` is every row of the target — the Pinned section is one target
    /// however many rows it draws — and `over` counts how many of them the
    /// pointer is inside, so that crossing from one row of a section to the
    /// next never puts the light out. A drop the Library cannot do is refused
    /// while the drag is still in the air, which is what preloading the value
    /// is for: without it the path is unreadable until the writer has let go.
    fn drop_onto(
        &self,
        on: &impl IsA<gtk::Widget>,
        onto: Aim,
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
        let asked = Rc::clone(&onto);
        let entering = light.clone();
        let counted = Rc::clone(over);
        target.connect_enter(move |target, _, _| {
            counted.set(counted.get() + 1);
            let allowed = asked().and_then(|onto| pane.would(target, &onto)).is_some();
            entering(allowed);
            action(allowed)
        });

        let pane = self.clone();
        let asked = Rc::clone(&onto);
        let moving = light.clone();
        target.connect_motion(move |target, _, _| {
            let allowed = asked().and_then(|onto| pane.would(target, &onto)).is_some();
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
        let counted = Rc::clone(over);
        target.connect_drop(move |target, value, x, y| {
            counted.set(0);
            light(false);
            let (Ok(from), Some(asked)) = (value.get::<String>(), onto()) else {
                return false;
            };
            let from = PathBuf::from(from);
            let Some(done) = files::dropped(&from, &asked, &pane.pinned_now()) else {
                return false;
            };
            let travel = pane.travel(target, x, y);
            pane.let_go(&from, &done, travel);
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
    ///
    /// A pin comes in from the way the row travelled to the drop, `travel`.
    fn let_go(&self, from: &Path, done: &Dropped, travel: (f64, f64)) {
        let Some(window) = self.owner() else {
            return;
        };
        match done {
            Dropped::Pin => {
                self.pinning.replace(Some(Pinning {
                    path: from.to_path_buf(),
                    along: approach(Gesture::Drop { travel }),
                }));
                window.set_pinned(from, true);
                // A pin the Library already held drew nothing to run.
                self.pinning.take();
            }
            Dropped::Into(folder) => window.move_path(from, folder),
        }
    }

    /// How far and which way the row let go of at (`x`, `y`) over `target`'s
    /// widget travelled from where it was grabbed, in the pane's coordinates:
    /// nothing where either end cannot be placed.
    fn travel(&self, target: &gtk::DropTarget, x: f64, y: f64) -> (f64, f64) {
        let dropped = target
            .widget()
            .and_then(|widget| widget.compute_point(&self.root, &place(x, y)));
        match (self.grabbed.take(), dropped) {
            (Some((from_x, from_y)), Some(at)) => {
                (f64::from(at.x()) - from_x, f64::from(at.y()) - from_y)
            }
            _ => (0.0, 0.0),
        }
    }

    /// Makes `slot`'s row draggable, carrying the path of the row bound in it
    /// as the plain text of it.
    ///
    /// A string rather than a type of Quill's own, because the drag never
    /// leaves this pane and a path is what both targets want; the icon under
    /// the pointer is the row itself, so what is being dragged is what was
    /// grabbed.
    fn drag_from(&self, slot: &Slot) {
        let source = gtk::DragSource::new();
        source.set_actions(gdk::DragAction::MOVE);
        let items = Rc::clone(&self.items);
        let bound = Rc::clone(&slot.bound);
        // Where the row was grabbed, which a pin dropped from it comes in
        // along ([`Sidebar::travel`]).
        let grabbed = Rc::clone(&self.grabbed);
        let pane = self.root.downgrade();
        source.connect_prepare(move |source, x, y| {
            let carried = bound.get().and_then(|at| {
                items
                    .borrow()
                    .get(at)
                    .map(|item| item.path.to_string_lossy().into_owned())
            })?;
            let at = pane
                .upgrade()
                .zip(source.widget())
                .and_then(|(root, row)| row.compute_point(&root, &place(x, y)));
            grabbed.set(at.map(|at| (f64::from(at.x()), f64::from(at.y()))));
            Some(gdk::ContentProvider::for_value(&carried.to_value()))
        });
        let dragged = slot.root.clone();
        source.connect_drag_begin(move |source, _| {
            source.set_icon(Some(&gtk::WidgetPaintable::new(Some(&dragged))), 0, 0);
        });
        slot.root.add_controller(source);
    }

    /// A click on `slot`'s row opens what is bound there: a folder opens or
    /// closes, a file opens in this window.
    ///
    /// On the release of a single press, as a list box's row activated, and
    /// once the view has taken the press into its selection: a folder's click
    /// lists the rows again, and the view's own answer to the press would
    /// otherwise select whatever the row's place holds by then.
    fn slot_click(&self, slot: &Slot) {
        let click = gtk::GestureClick::new();
        click.set_button(gdk::BUTTON_PRIMARY);
        let pane = self.clone();
        let bound = Rc::clone(&slot.bound);
        click.connect_released(move |_, presses, _, _| {
            let Some(at) = bound.get().filter(|_| presses == 1) else {
                return;
            };
            let pane = pane.clone();
            glib::idle_add_local_once(move || pane.activate(at));
        });
        slot.root.add_controller(click);
    }

    /// The File List's factory: a [`Slot`] built for each row the view asks
    /// for, bound to the item at its place as it scrolls in and let go of as
    /// it scrolls out.
    fn slots_factory(&self) -> gtk::SignalListItemFactory {
        let factory = gtk::SignalListItemFactory::new();
        let building = self.clone();
        factory.connect_setup(move |_, object| {
            let Some(item) = object.downcast_ref::<gtk::ListItem>() else {
                return;
            };
            let slot = building.slot();
            item.set_child(Some(&slot.root));
            building.slots.borrow_mut().push(slot);
        });
        let binding = self.clone();
        factory.connect_bind(move |_, object| {
            let Some(item) = object.downcast_ref::<gtk::ListItem>() else {
                return;
            };
            let at = item
                .item()
                .and_downcast::<glib::BoxedAnyObject>()
                .map(|at| *at.borrow::<usize>());
            if let (Some(slot), Some(at)) = (binding.slot_of(item), at) {
                binding.bind(&slot, at);
            }
        });
        let unbinding = self.clone();
        factory.connect_unbind(move |_, object| {
            if let Some(slot) = object
                .downcast_ref::<gtk::ListItem>()
                .and_then(|item| unbinding.slot_of(item))
            {
                unbinding.unbind(&slot);
            }
        });
        let tearing = self.clone();
        factory.connect_teardown(move |_, object| {
            let Some(item) = object.downcast_ref::<gtk::ListItem>() else {
                return;
            };
            if let Some(slot) = tearing.slot_of(item) {
                tearing.unbind(&slot);
                tearing
                    .slots
                    .borrow_mut()
                    .retain(|held| held.root != slot.root);
            }
        });
        factory
    }

    /// The slot whose row `item` holds.
    fn slot_of(&self, item: &gtk::ListItem) -> Option<Slot> {
        let child = item.child()?;
        self.slots
            .borrow()
            .iter()
            .find(|slot| slot.root.upcast_ref::<gtk::Widget>() == &child)
            .cloned()
    }

    /// The widgets of one row, bound to nothing yet: a folder's line and a
    /// file's, the bar and the glyph over them, the separator under them, and
    /// what a click, a drag and a drop on the row do.
    fn slot(&self) -> Slot {
        // A folder: its name and a chevron that says whether it is open, on a
        // row as short as a file's with no excerpt, and no count and no date.
        let folder_line = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        // A pixel short of the pitch, the separator above the row being the
        // last of it.
        folder_line.set_height_request(FOLDER_PITCH - 1);
        let folder_mark = icon(FOLDER, folder_icon);
        folder_mark.add_css_class("lib-folder-icon");
        folder_mark.set_valign(gtk::Align::Center);
        folder_line.append(&folder_mark);
        let folder_name = gtk::Label::new(None);
        folder_name.add_css_class("lib-head");
        folder_name.set_hexpand(true);
        folder_name.set_xalign(0.0);
        folder_name.set_margin_start(NAME_LEFT - (ICON_LEFT - FOLDER_BEARING) - FOLDER.0);
        folder_name.set_ellipsize(gtk::pango::EllipsizeMode::End);
        folder_line.append(&folder_name);
        let chevron_open = Rc::new(Cell::new(false));
        let open = Rc::clone(&chevron_open);
        let chevron = icon((CHEV, CHEV), move |area, cr| {
            chevron_icon(area, cr, open.get());
        });
        chevron.add_css_class("lib-icon");
        chevron.set_margin_end(CHEVRON_RIGHT);
        folder_line.append(&chevron);

        // A file: its name, when it was last written, and two lines of what it
        // says — its own beginning, or the snippet a query found in it — on a
        // row of [`ROW_PITCH`], or of [`FOLDER_PITCH`] where it says nothing.
        let file_line = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        let page_is = Rc::new(Cell::new(Page::Plain));
        let pose = Rc::new(Cell::new(PinPose::HOME));
        let (drawn, posed) = (Rc::clone(&page_is), Rc::clone(&pose));
        let page = icon(DOC, move |area, cr| {
            let _ = cr.save();
            document_icon(area, cr);
            let _ = cr.restore();
            if drawn.get() != Page::Plain {
                pin_icon(area, cr, posed.get());
            }
        });
        page.add_css_class("lib-icon");
        page.set_valign(gtk::Align::Start);
        file_line.append(&page);
        let body = gtk::Box::new(gtk::Orientation::Vertical, 0);
        body.set_hexpand(true);
        body.set_valign(gtk::Align::Start);
        body.set_margin_start(NAME_LEFT - ICON_LEFT - DOC.0);
        body.set_margin_end(DATE_RIGHT);
        let top = gtk::Box::new(gtk::Orientation::Horizontal, DATE_GAP);
        let title = gtk::Label::new(None);
        title.add_css_class("lib-name");
        title.set_hexpand(true);
        title.set_xalign(0.0);
        title.set_ellipsize(gtk::pango::EllipsizeMode::End);
        top.append(&title);
        let date = gtk::Label::new(None);
        date.add_css_class("lib-date");
        top.append(&date);
        body.append(&top);
        // Two lines, clipped at the last with no ellipsis, in an inscription
        // rather than a label: a wrapping label answers every question about
        // its size by laying its whole text out at no width at all, a line a
        // character, which was 2.7 ms a row of the two hundred the view keeps
        // built (#460). An inscription is asked for no size but the one it is
        // given — no characters of width, so that the row's width decides where
        // the lines break, and the two lines' height — and lays its text out
        // once, at that width. It takes no pointer, so that a wheel over an
        // excerpt scrolls the List.
        let excerpt = gtk::Inscription::new(None);
        excerpt.add_css_class("lib-excerpt");
        excerpt.set_xalign(0.0);
        excerpt.set_yalign(0.0);
        excerpt.set_wrap_mode(gtk::pango::WrapMode::WordChar);
        excerpt.set_text_overflow(gtk::InscriptionOverflow::Clip);
        excerpt.set_min_chars(0);
        excerpt.set_nat_chars(0);
        excerpt.set_min_lines(0);
        excerpt.set_nat_lines(0);
        excerpt.set_height_request(pixels(EXCERPT_LEADING * f64::from(EXCERPT_LINES)));
        excerpt.set_valign(gtk::Align::Start);
        excerpt.set_can_target(false);
        excerpt.set_margin_top(EXCERPT_TOP);
        body.append(&excerpt);
        file_line.append(&body);
        // At the row's right edge, inside the row's own margin, and hidden
        // until the window says this Document is the one in a conflict.
        let dot = icon((DOT, DOT), warned_dot);
        dot.add_css_class("lib-changed");
        dot.set_valign(gtk::Align::Start);
        dot.set_margin_top(DOT_TOP);
        dot.set_visible(false);
        file_line.append(&dot);
        let lines = gtk::Box::new(gtk::Orientation::Vertical, 0);
        lines.append(&folder_line);
        lines.append(&file_line);

        // The accent bar at the row's left edge, or the Selection Mark's glyph
        // where the bar stands. The glyph takes the bar's top and bottom insets
        // from the stylesheet, its width from its own aspect at the height they
        // leave and its left edge from the bar's centre line, and draws in the
        // accent only on the open Document's row, as the bar does.
        let bar = gtk::Box::new(gtk::Orientation::Vertical, 0);
        bar.add_css_class("lib-bar");
        bar.set_halign(gtk::Align::Start);
        let glyph_drawn: Rc<Cell<Option<&'static Glyph>>> = Rc::new(Cell::new(None));
        let drawing = Rc::clone(&glyph_drawn);
        let mark = gtk::DrawingArea::new();
        mark.set_draw_func(move |area, cr, _, height| {
            if let Some(glyph) = drawing.get() {
                chrome::source(area, cr, 1.0);
                glyph.draw(cr, f64::from(height));
            }
        });
        mark.add_css_class("lib-mark");
        mark.set_halign(gtk::Align::Start);
        mark.set_visible(false);
        // Over the row rather than beside it: a bar that takes a column of its
        // own pushes the icon and the name right of the insets they are set
        // from.
        let beside = gtk::Overlay::new();
        beside.set_child(Some(&lines));
        beside.add_overlay(&bar);
        beside.add_overlay(&mark);
        // The separator starts under the name rather than at the List's edge
        // (State 28: 40.5 points in from the left and 14 from the right),
        // follows the row's indent, and closes every row, the last included,
        // so a list ends on a rule rather than in empty space (#448's round 14).
        let rule = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        rule.add_css_class("lib-rule");
        rule.set_height_request(1);
        rule.set_margin_end(SEPARATOR_RIGHT);
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.add_css_class(ROW_CLASS);
        root.append(&beside);
        root.append(&rule);
        let slot = Slot {
            root,
            folder_line,
            folder_icon: folder_mark,
            folder_name,
            chevron,
            chevron_open,
            file_line,
            page,
            page_is,
            pose,
            body,
            top,
            title,
            date,
            excerpt,
            dot,
            bar,
            mark,
            glyph: glyph_drawn,
            rule,
            bound: Rc::new(Cell::new(None)),
        };
        // Every row of the pane can be dragged: a file into a folder or onto
        // Pinned, a folder either way too; and a folder's row takes a drop.
        self.drag_from(&slot);
        self.slot_target(&slot);
        self.slot_click(&slot);
        slot
    }

    /// Binds `slot` to the item at `at`: the line its kind draws, indented for
    /// its depth, and whether it is the open Document's row.
    fn bind(&self, slot: &Slot, at: usize) {
        let Some(item) = self.items.borrow().get(at).cloned() else {
            return;
        };
        slot.bound.set(Some(at));
        // The list's air above its first row, which scrolls away with it.
        slot.root.set_margin_top(if at == 0 { LIST_TOP } else { 0 });
        let indent = INDENT * i32::try_from(item.depth).unwrap_or(0);
        slot.rule.set_margin_start(NAME_LEFT + indent);
        slot.folder_line.set_visible(item.folder);
        slot.file_line.set_visible(!item.folder);
        if item.folder {
            slot.folder_icon
                .set_margin_start(ICON_LEFT - FOLDER_BEARING + indent);
            slot.folder_name.set_text(&item.name);
            slot.chevron_open.set(item.open);
            slot.chevron.queue_draw();
            slot.glyph.set(None);
            slot.mark.set_visible(false);
            slot.bar.set_visible(true);
            self.pins
                .borrow_mut()
                .retain(|(_, area, _)| *area != slot.page);
        } else {
            self.bind_file(slot, &item, indent);
        }
        self.mark_open(slot, &item);
    }

    /// The file half of [`Sidebar::bind`]: the page and its pin, the name and
    /// the date, the excerpt, and the bar or the glyph.
    ///
    /// Show Text Excerpts off takes a file's own beginning away and leaves the
    /// short row, the bar shortening with it; a content hit keeps its snippet,
    /// which is the reason the file is in the results at all.
    fn bind_file(&self, slot: &Slot, item: &Item, indent: i32) {
        let (excerpts, mark) = self.shown.get();
        let (said, matched) = match &item.snippet {
            Some((text, at)) => (text.clone(), Some(at.clone())),
            None if excerpts => (self.excerpt_of(&item.path, item.modified), None),
            None => (String::new(), None),
        };
        let tall = !said.is_empty();
        let lift = i32::from(!tall);
        let pitch = if tall { ROW_PITCH } else { FOLDER_PITCH };
        slot.file_line.set_height_request(pitch - 1);
        slot.page.set_margin_start(ICON_LEFT + indent);
        slot.page.set_margin_top(ICON_TOP - lift);
        slot.page_is.set(item.page);
        // A page bound while its pin runs takes the pose the others have.
        slot.pose.set(match &*self.pin_now.borrow() {
            Some((path, pose)) if *path == item.path => *pose,
            _ => PinPose::HOME,
        });
        slot.page.queue_draw();
        {
            let mut pins = self.pins.borrow_mut();
            pins.retain(|(_, area, _)| *area != slot.page);
            if item.page != Page::Plain {
                pins.push((item.path.clone(), slot.page.clone(), Rc::clone(&slot.pose)));
            }
        }
        slot.body.set_margin_top(NAME_TOP - lift);
        clear_rename(slot);
        slot.title.set_text(&item.name);
        slot.date.set_text(item.date.as_deref().unwrap_or(""));
        slot.date.set_visible(item.date.is_some());
        slot.excerpt.set_visible(tall);
        if tall {
            slot.excerpt.set_text(Some(&said));
            slot.excerpt
                .set_attributes(Some(&excerpt_attributes(matched)));
        }
        // The short glyph on the short row, whether excerpts are off or this
        // file has nothing to show of itself.
        match glyph(mark, !tall) {
            Some(drawn) => {
                let (width, left) = mark_place(drawn, pitch);
                slot.mark.set_content_width(pixels(width.ceil()));
                slot.mark.set_margin_start(left);
                slot.glyph.set(Some(drawn));
                slot.mark.set_visible(true);
                slot.bar.set_visible(false);
                slot.mark.queue_draw();
            }
            None => {
                slot.glyph.set(None);
                slot.mark.set_visible(false);
                slot.bar.set_visible(true);
            }
        }
    }

    /// Lets go of whatever `slot` was bound to: its page, if it drew a pin, is
    /// no longer one [`Sidebar::run_pin`] moves.
    fn unbind(&self, slot: &Slot) {
        slot.bound.set(None);
        self.pins
            .borrow_mut()
            .retain(|(_, area, _)| *area != slot.page);
    }

    /// What the file at `path`, as written at `modified`, shows of itself:
    /// read now where the pane holds no head of it or an older one.
    fn excerpt_of(&self, path: &Path, modified: Option<SystemTime>) -> String {
        head_of(&mut self.read.borrow_mut(), path, modified)
            .0
            .excerpt
            .clone()
    }

    /// Puts [`OPEN_CLASS`] on `slot`'s row where `item` is the Document the
    /// window is showing, with the dot while that Document is in a conflict.
    fn mark_open(&self, slot: &Slot, item: &Item) {
        let open = self.open.borrow().as_deref() == Some(item.path.as_path());
        slot.dot.set_visible(open && self.conflicted.get());
        if open {
            slot.root.add_css_class(OPEN_CLASS);
        } else {
            slot.root.remove_css_class(OPEN_CLASS);
        }
    }

    /// Marks the row of the Document the window is showing, and no row at all
    /// where it is showing something the Library does not hold; the dot goes
    /// on that same row while that Document is in a conflict.
    ///
    /// The mark is [`OPEN_CLASS`] and the selection is what Enter opens: the
    /// open row is selected too, and under a query a result list with the open
    /// Document nowhere in it selects its first hit instead, with no mark on
    /// it. The class goes on the rows in view and the selection on the item,
    /// which is what the pane lists and not what the tree holds.
    fn highlight(&self) {
        let items = self.items.borrow();
        for slot in self.slots.borrow().iter() {
            match slot.bound.get().and_then(|at| items.get(at)) {
                Some(item) => self.mark_open(slot, item),
                None => {
                    slot.root.remove_css_class(OPEN_CLASS);
                    slot.dot.set_visible(false);
                }
            }
        }
        let found = {
            let open = self.open.borrow();
            items
                .iter()
                .position(|item| open.as_deref() == Some(item.path.as_path()))
                .or_else(|| {
                    (!self.query().is_empty())
                        .then(|| items.iter().position(|item| !item.folder))?
                })
        };
        drop(items);
        self.selection.set_selected(
            found
                .and_then(|at| u32::try_from(at).ok())
                .unwrap_or(gtk::INVALID_LIST_POSITION),
        );
    }

    /// The row at `at` was clicked, or Enter was pressed on it: a folder opens
    /// or closes, a file opens in this window.
    fn activate(&self, at: usize) {
        let found = self
            .items
            .borrow()
            .get(at)
            .map(|item| (item.path.clone(), item.folder));
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
            .items
            .borrow()
            .iter()
            .position(|item| item.path == path);
        if let Some(at) = found.and_then(|at| u32::try_from(at).ok()) {
            self.list.scroll_to(
                at,
                gtk::ListScrollFlags::FOCUS | gtk::ListScrollFlags::SELECT,
                None,
            );
        }
    }

    // ------------------------------------------------------ row operations

    /// The row the pane has selected, and its place in the list.
    fn selected(&self) -> Option<(usize, Item)> {
        let at = usize::try_from(self.selection.selected()).ok()?;
        let item = self.items.borrow().get(at).cloned()?;
        Some((at, item))
    }

    /// The path of the row the pane has selected, whether file or folder.
    #[must_use]
    pub fn selected_path(&self) -> Option<PathBuf> {
        self.selected().map(|(_, item)| item.path)
    }

    /// The file the selected row is, and `None` where it is a folder or there
    /// is no selected row: what a row operation acts on
    /// (`Window::target`).
    #[must_use]
    pub fn selected_file(&self) -> Option<PathBuf> {
        let (_, item) = self.selected()?;
        (!item.folder).then_some(item.path)
    }

    /// The folder a Document started from this pane goes into: the selected
    /// folder row itself, or the folder the selected file stands in.
    #[must_use]
    pub fn selected_folder(&self) -> Option<PathBuf> {
        let (_, item) = self.selected()?;
        if item.folder {
            Some(item.path)
        } else {
            item.path.parent().map(Path::to_path_buf)
        }
    }

    /// The files the pane is showing, top to bottom, which is the list
    /// `file.next` and `file.prev` walk ([`crate::files::stepped`]).
    ///
    /// The rows the list holds and not the tree: a file inside a closed folder
    /// is not a row a writer can step to, and under a query the list is the
    /// hits.
    #[must_use]
    pub fn listed_files(&self) -> Vec<PathBuf> {
        self.items
            .borrow()
            .iter()
            .filter(|item| !item.folder)
            .map(|item| item.path.clone())
            .collect()
    }

    /// Puts a field in the place of the selected row's name, and answers
    /// whether there was a file row to put one in.
    ///
    /// `false` is what sends `file.rename` to the dialog instead
    /// (`Window::rename_document`): a folder row, no row at all, or a row
    /// scrolled out of view.
    pub fn start_rename(&self) -> bool {
        self.selected().is_some_and(|(at, _)| self.rename_row(at))
    }

    /// The place in the list of the row under (`x`, `y`) of the list's own
    /// pixels, where a row is bound there.
    fn index_at(&self, x: f64, y: f64) -> Option<usize> {
        let list = self.list.upcast_ref::<gtk::Widget>();
        let slots = self.slots.borrow();
        let mut under = self.list.pick(x, y, gtk::PickFlags::DEFAULT);
        while let Some(widget) = under {
            if &widget == list {
                return None;
            }
            if let Some(slot) = slots
                .iter()
                .find(|slot| slot.root.upcast_ref::<gtk::Widget>() == &widget)
            {
                return slot.bound.get();
            }
            under = widget.parent();
        }
        None
    }

    /// A second click at (`x`, `y`) inside the double-click time: a file's row
    /// is selected and its name becomes a field; a folder's row opens or
    /// closes again, since a folder is not renamed from the list and the list
    /// itself never sees the press to do it (#441's Hand test, step 2).
    fn second_press_at(&self, x: f64, y: f64) {
        let Some(at) = self.index_at(x, y) else {
            return;
        };
        let folder = self.items.borrow().get(at).is_some_and(|item| item.folder);
        if folder {
            self.activate(at);
            return;
        }
        if let Ok(position) = u32::try_from(at) {
            self.selection.set_selected(position);
        }
        self.rename_row(at);
    }

    /// Puts a field in the place of the name of the row at `at`, and answers
    /// whether it took: a file's row, and one in view.
    ///
    /// The oracle's field (`dev/legacy/app/js/files.js` `startRename`): the name
    /// as it stands with everything before the extension selected, Enter
    /// renaming, Esc leaving it, and clicking away renaming — because a writer
    /// who typed a name and looked elsewhere meant the name.
    fn rename_row(&self, at: usize) -> bool {
        let path = self
            .items
            .borrow()
            .get(at)
            .filter(|item| !item.folder)
            .map(|item| item.path.clone());
        let label = self
            .slots
            .borrow()
            .iter()
            .find(|slot| slot.bound.get() == Some(at))
            .map(|slot| slot.title.clone());
        let (Some(path), Some(label)) = (path, label) else {
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
            // Once the field is down and whatever took the keyboard from it
            // is done: a rename lists the rows again, and a field taken down
            // because its row was bound elsewhere is inside the list's own
            // binding ([`clear_rename`]).
            let path = path.to_path_buf();
            glib::idle_add_local_once(move || window.rename_path(&path, &typed));
        }
    }

    /// The right button at (`x`, `y`): the row under the pointer is selected
    /// and the menu of what can be done to it opens where the pointer is.
    ///
    /// The File List draws no section head — its own head is the Location's
    /// header ([`Sidebar::head_as_location`]) — so every row's menu is a
    /// row's.
    fn menu_at(&self, x: f64, y: f64) {
        let Some(at) = self.index_at(x, y) else {
            return;
        };
        let Some((path, folder)) = self
            .items
            .borrow()
            .get(at)
            .map(|item| (item.path.clone(), item.folder))
        else {
            return;
        };
        if let Ok(position) = u32::try_from(at) {
            self.selection.set_selected(position);
        }
        let model = row_menu(folder, self.is_pinned(&path));
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
        self.sort_menu.unparent();
    }

    /// Puts the header of the Location the File List shows on the File List's
    /// head, or takes it off again.
    ///
    /// The File List draws no Location head of its own — its head is already
    /// saying that Location's name — so the head is where Remove from Library
    /// and a drop on to the Location's top level have to live, or neither is
    /// reachable from the List at all. The target goes on and comes off as the
    /// Library changes, because one left behind would answer for a Location
    /// the pane has stopped showing.
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

    /// The Sort pill's [`PILL_GROUP`] actions ([`pill_actions`]), each writing
    /// its `[library]` key through the session's settings write.
    ///
    /// The choice is put on to the session as it is written and every window's
    /// pane is listed again in the same frame
    /// ([`crate::session::Session::edit_library`]), so a sort lands at the
    /// click rather than after the watch has read the file back; that read
    /// finds the table already applied and re-lists nothing (#453).
    fn install_pill_actions(&self) {
        let writing = self.clone();
        pill_actions(
            &self.sort_actions,
            move |edit: &dyn Fn(&mut settings::Library)| {
                let Some(session) = writing.session() else {
                    return;
                };
                session.edit_library(|settings| edit(&mut settings.library));
                match writing.owner().and_then(|window| window.application()) {
                    Some(app) => crate::window::relist(&app),
                    None => writing.refresh(),
                }
            },
        );
        self.root
            .insert_action_group(PILL_GROUP, Some(&self.sort_actions));
    }

    /// `row.open`: the selected row, opened as a click on it would.
    fn open_selected(&self) {
        if let Some((at, _)) = self.selected() {
            self.activate(at);
        }
    }

    /// `row.pin` and `row.unpin`, on the selected row.
    ///
    /// A pin comes in down and into the paper; an unpin runs it backwards on
    /// the rows as they stand, and the Library lets go of the pin once it is
    /// out.
    fn set_pinned(&self, pinned: bool) {
        let (Some(window), Some(path)) = (self.owner(), self.selected_path()) else {
            return;
        };
        let along = approach(Gesture::Menu);
        if pinned {
            self.pinning.replace(Some(Pinning {
                path: path.clone(),
                along,
            }));
            window.set_pinned(&path, true);
            // A pin the Library already held drew nothing to run.
            self.pinning.take();
        } else if animated() {
            let unpinned = path.clone();
            self.run_pin(&path, along, false, move || {
                window.set_pinned(&unpinned, false);
            });
        } else {
            window.set_pinned(&path, false);
        }
    }

    /// A page's icon for the row of `path`: the page, or the page with a pin
    /// through it, whose pin is kept where [`Sidebar::run_pin`] finds it.
    fn page_mark(&self, path: &Path, page: Page) -> gtk::DrawingArea {
        if page == Page::Plain {
            return icon(DOC, document_icon);
        }
        let pose = Rc::new(Cell::new(PinPose::HOME));
        let posed = Rc::clone(&pose);
        let mark = icon(DOC, move |area, cr| {
            let _ = cr.save();
            document_icon(area, cr);
            let _ = cr.restore();
            pin_icon(area, cr, posed.get());
        });
        self.pins
            .borrow_mut()
            .push((path.to_path_buf(), mark.clone(), pose));
        mark
    }

    /// Starts the pin a menu or a drop asked for on the rows this refresh drew
    /// for it. Where animations are off ([`animated`]) those rows are already
    /// drawn as the pin ends.
    fn start_pinning(&self) {
        let Some(pinning) = self.pinning.take() else {
            return;
        };
        if animated() {
            self.run_pin(&pinning.path, pinning.along, true, || {});
        }
    }

    /// Runs the pin of every page drawn for `path` in along `along`, or back
    /// out of it, over [`MOTION_MS`] eased out, the Organizer's Pinned row for
    /// `path` fading with it; `done` runs on the last frame.
    ///
    /// The pages are looked for at every frame rather than once: the File
    /// List binds a row as the view lays it out, which can be after the pin
    /// has begun, and a page bound part-way through takes the pose the others
    /// stand at ([`Sidebar::pin_now`]).
    fn run_pin(
        &self,
        path: &Path,
        along: (f64, f64),
        going_in: bool,
        done: impl FnOnce() + 'static,
    ) {
        let rows: Vec<gtk::ListBoxRow> = self
            .organized
            .borrow()
            .iter()
            .filter_map(|(row, organized)| match organized {
                Organized::Pinned { path: pinned, .. } if pinned == path => Some(row.clone()),
                _ => None,
            })
            .collect();
        let pins = Rc::clone(&self.pins);
        let standing = Rc::clone(&self.pin_now);
        let pinned = path.to_path_buf();
        let pose_at = move |at: f64| {
            let pose = PinPose { along, at };
            standing.replace(Some((pinned.clone(), pose)));
            for (page, area, held) in pins.borrow().iter() {
                if *page == pinned {
                    held.set(pose);
                    area.queue_draw();
                }
            }
            for row in &rows {
                row.set_opacity(at);
            }
        };
        pose_at(if going_in { 0.0 } else { 1.0 });
        let started = Cell::new(None);
        let done = Cell::new(Some(done));
        let ended = Rc::clone(&self.pin_now);
        self.root.add_tick_callback(move |_, clock| {
            let now = clock.frame_time();
            let start = started.get().unwrap_or(now);
            started.set(Some(start));
            let eased = eased(now - start);
            pose_at(if going_in { eased } else { 1.0 - eased });
            if eased < 1.0 {
                return glib::ControlFlow::Continue;
            }
            ended.replace(None);
            if let Some(done) = done.take() {
                done();
            }
            glib::ControlFlow::Break
        });
    }

    /// `row.remove`: the Location the menu item names leaves the Library.
    fn drop_location(&self, location: &Path) {
        if let Some(window) = self.owner() {
            window.drop_location(location);
        }
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
        query_of(&self.entry.text()).to_string()
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
    /// keystrokes out, and with its content hits in it: the search runs on
    /// this thread, which is what a harness launch shooting its first frame
    /// needs ([`Sidebar::set_query`]).
    fn search_now(&self) {
        self.wake();
        self.draw(true);
    }

    /// What `query`'s content search has answered, asking it where the
    /// standing question is not this one: on this thread where `at_once`, and
    /// otherwise on the search thread, whose answer draws the list again.
    /// `None` while the answer is owed, which lists the names alone; and for an
    /// empty query, which lists no search at all.
    fn matches(
        &self,
        library: &Library,
        query: &str,
        view: View,
        at_once: bool,
    ) -> Option<Vec<Match>> {
        let mut searching = self.searching.borrow_mut();
        if query.is_empty() {
            searching.forget();
            return None;
        }
        if let Some(question) = searching.ask(query, view, library) {
            if at_once {
                searching.answer_now(&question);
            } else {
                searching.spawn(question);
                self.poll_answers(&mut searching);
            }
        }
        searching.matches(query, view).map(<[Match]>::to_vec)
    }

    /// Looks for the search thread's answer each [`ANSWER_POLL_MS`] while one
    /// is owed, and draws the list again when it comes; one poll at a time.
    fn poll_answers(&self, searching: &mut Searching) {
        if searching.polling {
            return;
        }
        searching.polling = true;
        let pane = self.clone();
        glib::timeout_add_local(Duration::from_millis(ANSWER_POLL_MS), move || {
            let (answered, owed) = {
                let mut searching = pane.searching.borrow_mut();
                let answered = searching.receive();
                let owed = searching.owed();
                searching.polling = owed;
                (answered, owed)
            };
            if answered {
                pane.refresh();
            }
            if owed {
                glib::ControlFlow::Continue
            } else {
                glib::ControlFlow::Break
            }
        });
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
        let listed = !self.items.borrow().is_empty();
        let at = self
            .selected()
            .map(|(at, _)| at)
            .or_else(|| listed.then_some(0));
        if let Some(at) = at {
            self.activate(at);
        }
    }

    /// Down in the field: the keyboard steps into the list, where the arrows
    /// walk the rows.
    fn step_into_list(&self) {
        if self.items.borrow().is_empty() {
            self.list.grab_focus();
            return;
        }
        if self.selected().is_none() {
            self.selection.set_selected(0);
        }
        self.list
            .scroll_to(self.selection.selected(), gtk::ListScrollFlags::FOCUS, None);
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
/// The oracle's order (`dev/legacy/app/js/files.js` `rowMenu`: Open, Rename…,
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

/// The File List's head: the name of the Location it shows, bold and centred
/// over the List, and New as a bare `+` at the right (#441 § The foot and the
/// title bar).
///
/// New fires its Command by name, as the bars' buttons do
/// ([`crate::chrome::Bars`]). The head is the shown Location's header too, and
/// carries its menu and its drop target ([`Sidebar::head_as_location`]).
fn head(title: &gtk::Label) -> gtk::Box {
    let head = gtk::Box::new(gtk::Orientation::Horizontal, HEAD_GAP);
    head.set_height_request(HEAD_HEIGHT);
    head.set_margin_start(HEAD_LEFT);
    head.set_margin_end(HEAD_RIGHT);
    // As wide as New, so that the name is centred on the List rather than on
    // what New leaves of it.
    let balance = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    balance.set_width_request(BUTTON);
    head.append(&balance);
    title.add_css_class("lib-title");
    title.set_hexpand(true);
    title.set_ellipsize(gtk::pango::EllipsizeMode::End);
    head.append(title);
    head.append(&button(plus_icon, "win.file.new"));
    head
}

/// The Organizer's head: room for the toggle that shuts the pane, which
/// stands over it from outside the pane ([`Sidebar::toggle`]).
fn organizer_head() -> gtk::Box {
    let head = gtk::Box::new(gtk::Orientation::Horizontal, HEAD_GAP);
    head.set_height_request(HEAD_HEIGHT);
    head
}

/// What the File List shows, as the Organizer chose it.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Showing {
    /// A Location's tree.
    Location(PathBuf),
    /// A pinned folder's tree.
    Folder(PathBuf),
    /// The Documents last opened, flat and newest first.
    Recents,
}

/// One row of the Organizer, and what a click on it does.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Organized {
    /// A section head.
    Head(&'static str),
    /// A Location, on its pill while the File List shows it.
    Location {
        path: PathBuf,
        name: String,
        on: bool,
    },
    /// A pinned Document or folder, marked while the File List shows the
    /// folder.
    Pinned {
        path: PathBuf,
        name: String,
        folder: bool,
        on: bool,
    },
    /// The prose an empty Pinned section says.
    NothingPinned,
    /// The one Recents row, marked while the File List shows the recents.
    Recents { on: bool },
}

/// The Organizer's rows for a Library of `locations` and `pinned` — each pin
/// with whether it is a folder — in section order, with what the File List is
/// `showing` marked (#441 § The Organizer).
///
/// A pinned Document's name drops its extension as the File List's rows do
/// (`library.show_extensions`), and every name is ellipsised by its row
/// ([`org_row`]); a folder's name is its whole name.
fn organized(
    locations: &[&Path],
    pinned: &[(&Path, bool)],
    showing: Option<&Showing>,
    extensions: bool,
) -> Vec<Organized> {
    let mut rows = vec![Organized::Head(LOCATIONS)];
    rows.extend(locations.iter().map(|path| Organized::Location {
        path: path.to_path_buf(),
        name: place_name(path),
        on: showing == Some(&Showing::Location(path.to_path_buf())),
    }));
    rows.push(Organized::Head(PINNED));
    if pinned.is_empty() {
        rows.push(Organized::NothingPinned);
    }
    rows.extend(pinned.iter().map(|&(path, folder)| Organized::Pinned {
        path: path.to_path_buf(),
        name: if folder {
            place_name(path)
        } else {
            row_name(&place_name(path), extensions)
        },
        folder,
        on: folder && showing == Some(&Showing::Folder(path.to_path_buf())),
    }));
    rows.push(Organized::Head(RECENTS));
    rows.push(Organized::Recents {
        on: showing == Some(&Showing::Recents),
    });
    rows
}

/// What the File List shows now: what the Organizer last `chosen`, while the
/// Library still holds it, and otherwise the first Location, or nothing where
/// there is none.
fn settled(
    chosen: Option<&Showing>,
    locations: &[&Path],
    pinned: &[(&Path, bool)],
) -> Option<Showing> {
    let held = match chosen {
        Some(Showing::Location(path)) => locations.contains(&path.as_path()),
        Some(Showing::Folder(path)) => pinned.contains(&(path.as_path(), true)),
        Some(Showing::Recents) => true,
        None => false,
    };
    if held {
        return chosen.cloned();
    }
    locations
        .first()
        .map(|path| Showing::Location(path.to_path_buf()))
}

/// The recents the Library holds, newest first: the files `opened` names that
/// a Location's tree has a row for, in the order the state keeps them.
fn recent_files<'a>(library: &'a Library, opened: &[PathBuf]) -> Vec<&'a File> {
    opened
        .iter()
        .filter_map(|path| match library.at(path) {
            Some(Row::File { file, .. }) => Some(file),
            _ => None,
        })
        .collect()
}

/// What a query over Recents finds: what it found over the Library, name hits
/// first, narrowed to the files `opened` names.
fn recent_found<'a>(
    found: Vec<(&'a File, Option<Snippet>)>,
    opened: &[PathBuf],
) -> Vec<(&'a File, Option<Snippet>)> {
    found
        .into_iter()
        .filter(|(file, _)| opened.iter().any(|path| path == file.path()))
        .collect()
}

/// What `query` lists over the Library: `matches`, the search thread's
/// answer, for each path the tree still holds; or, while the answer is owed,
/// the files whose names the query matched, which wants no file read and is
/// the answer's own first group ([`Library::names`]).
fn found_files<'a>(
    library: &'a Library,
    query: &str,
    view: &View,
    matches: Option<&[Match]>,
) -> Vec<(&'a File, Option<Snippet>)> {
    let Some(matches) = matches else {
        return library
            .names(query, view)
            .into_iter()
            .map(|file| (file, None))
            .collect();
    };
    matches
        .iter()
        .filter_map(|(path, snippet)| match library.at(path) {
            Some(Row::File { file, .. }) => Some((file, snippet.clone())),
            _ => None,
        })
        .collect()
}

/// One file a query matched, as it crosses back from the search thread: its
/// path, which the tree answers for again on the main thread, and the snippet
/// a content match carries ([`Found`]).
type Match = (PathBuf, Option<Snippet>);

/// `found` in the form that crosses a thread.
fn owned(found: &[Found<'_>]) -> Vec<Match> {
    found
        .iter()
        .map(|hit| (hit.file().path().to_path_buf(), hit.snippet().cloned()))
        .collect()
}

/// One content search asked of the search thread: the query, over the view
/// and a copy of the Library as they stood when it was asked.
#[derive(Clone)]
struct Question {
    /// Which question this is, counting up from the first; an answer carries
    /// it back, so that an answer to a question since replaced is known.
    number: u64,
    query: String,
    view: View,
    library: Library,
}

impl Question {
    /// Whether this is the question `query` over `view` and `library` asks.
    fn asks(&self, query: &str, view: View, library: &Library) -> bool {
        self.query == query && self.view == view && self.library == *library
    }

    /// Searches, reading through `contents` what the engine's search reads.
    fn answer(&self, contents: &mut Contents) -> Answer {
        Answer {
            number: self.number,
            matches: owned(&self.library.search(&self.query, &self.view, contents)),
        }
    }
}

/// What the search thread found for one [`Question`].
struct Answer {
    number: u64,
    matches: Vec<Match>,
}

/// A query's content search, run off the main thread (#454).
///
/// The engine's [`Library::search`] reads every shown file whose name did not
/// match, which over a folder of a few hundred Documents is seconds; on the
/// main thread that was a window not responding on a query's first letter.
/// Each question goes to a thread of its own with a copy of the Library, the
/// text cache goes with it behind a lock, and the answer comes back over a
/// channel the main thread polls ([`Sidebar::poll_answers`]). Questions queue
/// at the lock one at a time, so the cache keeps its write-time rule and the
/// second query over an untouched tree still reads nothing; and a question a
/// newer one replaced while it queued is never searched, which is what spares
/// a word typed letter by letter one read per letter.
struct Searching {
    /// The file texts search has read ([`Contents`]).
    contents: Arc<Mutex<Contents>>,
    /// The number of the newest question asked, which a queued thread reads to
    /// learn it has been replaced.
    newest: Arc<AtomicU64>,
    /// The question standing: the last asked, answered or not.
    asked: Option<Question>,
    /// The answer to the standing question, or to the question before it
    /// while that one's query and view are still the standing one's, so that
    /// a tree that moves under a query keeps its hits while they are found
    /// again.
    answered: Option<Answered>,
    sender: mpsc::Sender<Answer>,
    answers: mpsc::Receiver<Answer>,
    /// Whether a poll for answers is running ([`Sidebar::poll_answers`]).
    polling: bool,
}

/// An answer the pane lists, with the query and view it answered.
struct Answered {
    number: u64,
    query: String,
    view: View,
    matches: Vec<Match>,
}

impl Searching {
    /// Nothing asked and no text read.
    fn new() -> Self {
        let (sender, answers) = mpsc::channel();
        Self {
            contents: Arc::new(Mutex::new(Contents::new())),
            newest: Arc::new(AtomicU64::new(0)),
            asked: None,
            answered: None,
            sender,
            answers,
            polling: false,
        }
    }

    /// The question `query` over `view` and `library` asks, where it is not
    /// the one already standing; `None` where it is, asked or answered.
    fn ask(&mut self, query: &str, view: View, library: &Library) -> Option<Question> {
        if self
            .asked
            .as_ref()
            .is_some_and(|asked| asked.asks(query, view, library))
        {
            return None;
        }
        let question = Question {
            number: self.newest.fetch_add(1, Ordering::Relaxed) + 1,
            query: query.to_string(),
            view,
            library: library.clone(),
        };
        self.asked = Some(question.clone());
        Some(question)
    }

    /// Answers `question` on this thread.
    fn answer_now(&mut self, question: &Question) {
        let answer =
            question.answer(&mut self.contents.lock().unwrap_or_else(PoisonError::into_inner));
        self.take(answer);
    }

    /// Answers `question` on a thread of its own, sending the answer back;
    /// on this thread where no thread can be started, so that an answer is
    /// never owed forever.
    fn spawn(&mut self, question: Question) {
        let contents = Arc::clone(&self.contents);
        let newest = Arc::clone(&self.newest);
        let sender = self.sender.clone();
        let asked = question.clone();
        let started = std::thread::Builder::new()
            .name("quill-search".to_string())
            .spawn(move || {
                let mut contents = contents.lock().unwrap_or_else(PoisonError::into_inner);
                // Replaced while it waited for the cache: the newer question
                // reads whatever this one would have.
                if newest.load(Ordering::Relaxed) != question.number {
                    return;
                }
                sender.send(question.answer(&mut contents)).ok();
            });
        if started.is_err() {
            self.answer_now(&asked);
        }
    }

    /// Takes in `answer`, answering whether it answered the standing question;
    /// an answer to a question since replaced is dropped.
    fn take(&mut self, answer: Answer) -> bool {
        let Some(asked) = self
            .asked
            .as_ref()
            .filter(|asked| asked.number == answer.number)
        else {
            return false;
        };
        self.answered = Some(Answered {
            number: answer.number,
            query: asked.query.clone(),
            view: asked.view,
            matches: answer.matches,
        });
        true
    }

    /// Takes in every answer the search thread has sent, answering whether one
    /// answered the standing question. Only one can, since a question is
    /// answered once, so the rest are dropped unread.
    fn receive(&mut self) -> bool {
        let answers: Vec<Answer> = self.answers.try_iter().collect();
        answers.into_iter().any(|answer| self.take(answer))
    }

    /// Whether the standing question's answer is still to come.
    fn owed(&self) -> bool {
        self.asked.as_ref().is_some_and(|asked| {
            self.answered
                .as_ref()
                .is_none_or(|answered| answered.number != asked.number)
        })
    }

    /// The hits to list for `query` over `view`: the standing question's
    /// answer, or the last one given for the same query and view.
    fn matches(&self, query: &str, view: View) -> Option<&[Match]> {
        self.answered
            .as_ref()
            .filter(|answered| answered.query == query && answered.view == view)
            .map(|answered| answered.matches.as_slice())
    }

    /// Stands no question: the query is gone, and a question still queued is
    /// not searched.
    fn forget(&mut self) {
        if self.asked.take().is_some() {
            self.newest.fetch_add(1, Ordering::Relaxed);
        }
        self.answered = None;
    }
}

/// What one refresh lists in the File List, before a row of it is built.
enum Listing<'a> {
    /// A Location's or a pinned folder's tree, less what lies inside a closed
    /// folder.
    Tree(Vec<Row<'a>>),
    /// A search's hits or the recents, flat, each with the snippet its text
    /// matched where it did.
    Flat(Vec<(&'a File, Option<Snippet>)>),
    /// Nothing to list: a Library with no Location.
    Nothing,
}

impl<'a> Listing<'a> {
    /// The files listed, in the order they are drawn: every file whose head a
    /// row may show, and no other.
    fn files(&self) -> Vec<&'a File> {
        match self {
            Self::Tree(rows) => rows
                .iter()
                .filter_map(|row| match row {
                    Row::File { file, .. } => Some(*file),
                    Row::Folder { .. } => None,
                })
                .collect(),
            Self::Flat(files) => files.iter().map(|(file, _)| *file).collect(),
            Self::Nothing => Vec::new(),
        }
    }
}

/// What the File List lists for `showing` and `query`: the Recents `opened`
/// names, narrowed by the query where there is one; a query's hits over the
/// Library, `matches` where the search has answered and its name hits alone
/// where it has not ([`found_files`]); or the chosen tree, sorted once and
/// with what `expanded` leaves closed left out.
///
/// Only the tree listed is sorted and walked, and no file's text is read here:
/// the search reads on its own thread ([`Searching`]).
fn listing<'a>(
    library: &'a Library,
    showing: Option<&Showing>,
    query: &str,
    view: &View,
    opened: &[PathBuf],
    matches: Option<&[Match]>,
    expanded: &BTreeSet<PathBuf>,
) -> Listing<'a> {
    match showing {
        Some(Showing::Recents) if query.is_empty() => Listing::Flat(
            recent_files(library, opened)
                .into_iter()
                .map(|file| (file, None))
                .collect(),
        ),
        Some(Showing::Recents) => Listing::Flat(recent_found(
            found_files(library, query, view, matches),
            opened,
        )),
        _ if !query.is_empty() => Listing::Flat(found_files(library, query, view, matches)),
        Some(Showing::Location(path) | Showing::Folder(path)) => {
            Listing::Tree(open_rows(&library.rows(path, view), expanded))
        }
        None => Listing::Nothing,
    }
}

/// `rows` less whatever lies inside a folder `expanded` does not hold.
///
/// A closed folder takes its subtree with it: the tree arrives flattened,
/// deepest last, so everything below a closed folder is everything after it
/// that is deeper than it is.
fn open_rows<'a>(rows: &[Row<'a>], expanded: &BTreeSet<PathBuf>) -> Vec<Row<'a>> {
    let mut closed: Option<usize> = None;
    let mut open = Vec::new();
    for row in rows {
        if closed.is_some_and(|depth| row.depth() > depth) {
            continue;
        }
        closed = match row {
            Row::Folder { depth, .. } if !expanded.contains(row.path()) => Some(*depth),
            _ => None,
        };
        open.push(*row);
    }
    open
}

/// The rows `listing` lists, as the File List draws them under the
/// `[library]` table `shown`, the Pinned list `pinned` and the clock `now`, a
/// folder open where `expanded` holds it: one [`Item`] each, in order, and no
/// file read.
fn items_of(
    listing: &Listing<'_>,
    shown: &settings::Library,
    pinned: &[PathBuf],
    now: Option<&glib::DateTime>,
    expanded: &BTreeSet<PathBuf>,
) -> Vec<Item> {
    let file = |file: &File, depth: usize, snippet: Option<&Snippet>| {
        let when = match shown.show_date {
            ShowDate::Modified => file.modified(),
            ShowDate::Created => file.created(),
            ShowDate::None => None,
        };
        Item {
            path: file.path().to_path_buf(),
            folder: false,
            depth,
            name: row_name(file.name(), shown.show_extensions),
            open: false,
            date: when.zip(now).map(|(when, now)| stamp(when, now)),
            modified: file.modified(),
            snippet: snippet.map(|snippet| (snippet.text().to_string(), snippet.at())),
            page: page_of(file.path(), pinned),
        }
    };
    match listing {
        Listing::Tree(rows) => rows
            .iter()
            .map(|row| match row {
                Row::File { file: of, depth } => file(of, *depth, None),
                Row::Folder { folder, depth } => Item {
                    path: row.path().to_path_buf(),
                    folder: true,
                    depth: *depth,
                    name: folder.name().to_string(),
                    open: expanded.contains(row.path()),
                    date: None,
                    modified: folder.modified(),
                    snippet: None,
                    page: Page::Plain,
                },
            })
            .collect(),
        Listing::Flat(files) => files
            .iter()
            .map(|(of, snippet)| file(of, 0, snippet.as_ref()))
            .collect(),
        Listing::Nothing => Vec::new(),
    }
}

/// Lets go of every head in `read` of a file not among `files`, reading
/// nothing: what is held is at most one head per listed file.
fn prune_heads<'a>(read: &mut BTreeMap<PathBuf, Head>, files: impl IntoIterator<Item = &'a File>) {
    let listed: BTreeSet<&Path> = files.into_iter().map(File::path).collect();
    read.retain(|path, _| listed.contains(path.as_path()));
}

/// The head of the file at `path` as written at `modified`, and whether it
/// was read for this: a head `read` already holds of that write is not read
/// again, which is the shape [`Contents`] gives search.
///
/// Asked as a row is bound rather than for every listed file, so a Location of
/// four hundred Documents is read only as far as it is seen, one read of at
/// most [`EXCERPT_BYTES`] a row (#460).
fn head_of<'m>(
    read: &'m mut BTreeMap<PathBuf, Head>,
    path: &Path,
    modified: Option<SystemTime>,
) -> (&'m Head, bool) {
    match read.entry(path.to_path_buf()) {
        std::collections::btree_map::Entry::Occupied(held) if held.get().modified == modified => {
            (held.into_mut(), false)
        }
        std::collections::btree_map::Entry::Occupied(mut held) => {
            held.insert(Head::of(path, modified));
            (held.into_mut(), true)
        }
        std::collections::btree_map::Entry::Vacant(empty) => {
            (empty.insert(Head::of(path, modified)), true)
        }
    }
}

/// Takes down a rename's field left standing in `slot` when the list binds
/// the slot to another row, and puts the name back; the field's own ending
/// renames the file it was opened on ([`Sidebar::end_rename`]).
fn clear_rename(slot: &Slot) {
    let mut child = slot.top.first_child();
    while let Some(widget) = child {
        child = widget.next_sibling();
        if widget.is::<gtk::Entry>() {
            slot.top.remove(&widget);
        }
    }
    slot.title.set_visible(true);
}

/// Everything the File List's rows are built from, so that two refreshes can
/// be told apart without building either ([`Sidebar::refresh`]).
///
/// A file's head is read again only when its write time moves, and its write
/// time is here, so the excerpt a row shows is too. The day stands for the
/// clock, since a date is said to the day ([`stamp`]).
#[derive(Debug, PartialEq, Eq)]
struct Picture {
    rows: Vec<Pictured>,
    query: String,
    extensions: bool,
    date: ShowDate,
    excerpts: bool,
    mark: Mark,
    pinned: Vec<PathBuf>,
    day: Option<(i32, i32)>,
}

/// One row of a [`Picture`].
#[derive(Debug, PartialEq, Eq)]
struct Pictured {
    path: PathBuf,
    depth: usize,
    /// Whether a folder is open; `None` for a file.
    open: Option<bool>,
    modified: Option<SystemTime>,
    created: Option<SystemTime>,
    snippet: Option<Snippet>,
}

impl Picture {
    /// What `listing` would be built into under `query`, the `[library]`
    /// table `shown`, `pinned` and the clock `now`.
    fn of(
        listing: &Listing<'_>,
        query: &str,
        shown: &settings::Library,
        pinned: &[PathBuf],
        now: Option<&glib::DateTime>,
        expanded: &BTreeSet<PathBuf>,
    ) -> Self {
        let file = |file: &File, depth, snippet: Option<Snippet>| Pictured {
            path: file.path().to_path_buf(),
            depth,
            open: None,
            modified: file.modified(),
            created: file.created(),
            snippet,
        };
        let rows = match listing {
            Listing::Tree(rows) => rows
                .iter()
                .map(|row| match row {
                    Row::File { file: of, depth } => file(of, *depth, None),
                    Row::Folder { folder, depth } => Pictured {
                        path: row.path().to_path_buf(),
                        depth: *depth,
                        open: Some(expanded.contains(row.path())),
                        modified: folder.modified(),
                        created: folder.created(),
                        snippet: None,
                    },
                })
                .collect(),
            Listing::Flat(files) => files
                .iter()
                .map(|(of, snippet)| file(of, 0, snippet.clone()))
                .collect(),
            Listing::Nothing => Vec::new(),
        };
        Self {
            rows,
            query: query.to_string(),
            extensions: shown.show_extensions,
            date: shown.show_date,
            excerpts: shown.show_excerpts,
            mark: shown.mark,
            pinned: pinned.to_vec(),
            day: now.map(|now| (now.year(), now.day_of_year())),
        }
    }
}

/// An Organizer section head: bold, in the head grey, with a section's air
/// above it where it is not the first.
fn org_head_row(name: &str, after: bool) -> gtk::ListBoxRow {
    let label = gtk::Label::new(Some(name));
    label.add_css_class("lib-org-head");
    label.set_xalign(0.0);
    label.set_margin_start(ORG_HEAD_LEFT);
    label.set_margin_top(if after { ORG_SECTION_AIR } else { 0 });
    label.set_margin_bottom(ORG_HEAD_GAP);
    still_row(&label)
}

/// What an empty Pinned section says, wrapped to the column in the head grey.
fn org_prose(words: &str) -> gtk::ListBoxRow {
    let prose = gtk::Label::new(Some(words));
    prose.add_css_class("lib-org-prose");
    prose.set_xalign(0.0);
    prose.set_wrap(true);
    prose.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    // One character of natural width, so that the column decides where the
    // lines break rather than the sentence deciding the column's width.
    prose.set_max_width_chars(1);
    prose.set_margin_start(ORG_HEAD_LEFT);
    prose.set_margin_end(ORG_PILL_INSET);
    still_row(&prose)
}

/// An Organizer row that does nothing when clicked.
fn still_row(child: &impl IsA<gtk::Widget>) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();
    row.set_selectable(false);
    row.set_activatable(false);
    row.set_child(Some(child));
    row
}

/// An Organizer row: its icon and its name, ellipsised short of the column's
/// edge, on the pill where it is what the File List shows.
fn org_row(mark: gtk::DrawingArea, name: &str, on: bool) -> gtk::ListBoxRow {
    let line = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    line.add_css_class(ORG_LINE_CLASS);
    line.set_height_request(ORG_ROW);
    line.set_margin_start(ORG_PILL_INSET);
    line.set_margin_end(ORG_PILL_INSET);
    mark.set_margin_start(ORG_ICON_LEFT - ORG_PILL_INSET);
    mark.set_valign(gtk::Align::Center);
    let label = gtk::Label::new(Some(name));
    label.add_css_class("lib-org-row");
    label.set_xalign(0.0);
    label.set_hexpand(true);
    label.set_margin_start(ORG_TEXT_LEFT - ORG_ICON_LEFT - mark.content_width());
    label.set_margin_end(ORG_TEXT_END);
    label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    line.append(&mark);
    line.append(&label);
    if on {
        line.add_css_class("lib-pill");
    }
    let row = gtk::ListBoxRow::new();
    row.set_selectable(false);
    row.set_child(Some(&line));
    row
}

/// A folder's icon in the Organizer, in the accent as the File List's folders
/// are (#441 § Grounds and roles).
fn org_folder() -> gtk::DrawingArea {
    let mark = icon(FOLDER, folder_icon);
    mark.add_css_class("lib-folder-icon");
    mark
}

/// The Recents mark: a clock face.
fn clock_icon(area: &gtk::DrawingArea, cr: &cairo::Context) {
    chrome::source(area, cr, 1.0);
    cr.set_line_width(1.2);
    cr.set_line_cap(cairo::LineCap::Round);
    cr.arc(7.0, 6.0, 5.2, 0.0, std::f64::consts::TAU);
    let _ = cr.stroke();
    cr.move_to(7.0, 3.2);
    cr.line_to(7.0, 6.2);
    cr.line_to(9.2, 7.6);
    let _ = cr.stroke();
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

/// The Organizer's width in a pane `pane` points wide: [`ORGANIZER`] at
/// [`WIDTH`], and half of every point the divider adds past it, so a name in
/// either column gets room as the pane widens (#459). A pane narrower than
/// [`WIDTH`] leaves the Organizer at [`ORGANIZER`].
fn organizer_width(pane: i32) -> i32 {
    ORGANIZER + (pane.max(WIDTH) - WIDTH) / 2
}

/// The Filter field at the File List's foot: the rule, and under it the capsule
/// with its magnifier and its prompt (#441 § Search).
fn filter(entry: &gtk::Entry) -> gtk::Box {
    let foot = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let rule = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    rule.add_css_class("lib-foot-rule");
    rule.set_height_request(1);
    foot.append(&rule);
    let field = gtk::Box::new(gtk::Orientation::Horizontal, FILTER_GAP);
    field.add_css_class("lib-filter");
    field.set_height_request(FILTER_HEIGHT);
    // The capsule keeps the width it has at the pane's narrowest however far
    // the divider widens the pane (#441 § The Sort pill and its menu).
    field.set_width_request(WIDTH - ORGANIZER - FILTER_LEFT - FILTER_RIGHT);
    field.set_halign(gtk::Align::Start);
    field.set_margin_start(FILTER_LEFT);
    field.set_margin_end(FILTER_RIGHT);
    field.set_margin_top(FILTER_AIR);
    field.set_margin_bottom(FILTER_AIR);
    let mag = icon((FILTER_ICON, FILTER_ICON), magnifier_icon);
    mag.add_css_class("lib-icon");
    mag.set_margin_start(FILTER_ICON_LEFT);
    field.append(&mag);
    entry.set_margin_end(FILTER_END);
    field.append(entry);
    // Shown while a query stands, and a click on it empties the field, which
    // clears the search as deleting the query by hand does.
    let clear = icon((CLEAR, CLEAR), clear_icon);
    clear.add_css_class("lib-clear");
    clear.set_valign(gtk::Align::Center);
    clear.set_margin_end(FILTER_END);
    clear.set_visible(false);
    // An icon takes no pointer, so the ✕ is given it back for its click.
    clear.set_can_target(true);
    let click = gtk::GestureClick::new();
    let emptied = entry.clone();
    click.connect_released(move |_, _, _, _| emptied.set_text(""));
    clear.add_controller(click);
    field.append(&clear);
    let (ringed, shown) = (field.clone(), clear.clone());
    entry.connect_changed(move |entry| {
        let live = live(&entry.text());
        if live {
            ringed.add_css_class(LIVE_CLASS);
        } else {
            ringed.remove_css_class(LIVE_CLASS);
        }
        shown.set_visible(live);
    });
    foot.append(&field);
    foot
}

/// Whether the Filter field's `text` is a query, which is what rings the
/// capsule and shows its ✕: the same trimmed text [`Sidebar::query`] searches,
/// so a field holding only spaces reads as empty in both places.
fn live(text: &str) -> bool {
    !query_of(text).is_empty()
}

/// The query the Filter field's `text` stands for: the text without the
/// spaces around it, so a field of spaces lists what an empty one does —
/// the chosen tree, or Recents whole.
fn query_of(text: &str) -> &str {
    text.trim()
}

/// The view `[library]` reads the tree through: hidden folders, the sort, its
/// order, and whether folders stand first.
fn view_of(library: &settings::Library) -> View {
    View {
        show_hidden: library.show_hidden,
        sort: library.sort,
        order: library.order,
        pin_folders: library.pin_folders,
    }
}

/// The Sort pill's menu: iA's groups as shot, less Navigation, and the one
/// group Quill adds (#441 § The Sort pill and its menu).
///
/// Every row is a radio or a check on the pane's [`PILL_GROUP`] and none is a
/// Command, so `docs/shortcuts.md` has nothing to say about it; which row is
/// checked is its action's state ([`sync_pill`]).
fn pill_menu() -> gio::Menu {
    let menu = gio::Menu::new();
    let fields = gio::Menu::new();
    for (label, sort) in [
        ("Date Modified", Sort::Modified),
        ("Date Created", Sort::Created),
        ("Name", Sort::Name),
        ("Extension", Sort::Extension),
    ] {
        fields.append_item(&radio(label, SORT_ACTION, sort.as_str()));
    }
    menu.append_section(None, &fields);
    let orders = gio::Menu::new();
    for (label, order) in [
        ("Oldest on Top", Order::Oldest),
        ("Newest on Top", Order::Newest),
    ] {
        orders.append_item(&radio(label, ORDER_ACTION, order.as_str()));
    }
    menu.append_section(None, &orders);
    let placing = gio::Menu::new();
    placing.append(Some("Pin Folders to Top"), Some(&pill(PIN_FOLDERS_ACTION)));
    menu.append_section(None, &placing);
    let showing = gio::Menu::new();
    let dates = gio::Menu::new();
    for (label, date) in [
        ("Date Modified", ShowDate::Modified),
        ("Date Created", ShowDate::Created),
        ("None", ShowDate::None),
    ] {
        dates.append_item(&radio(label, SHOW_DATE_ACTION, date.as_str()));
    }
    showing.append_submenu(Some("Show Date"), &dates);
    showing.append(
        Some("Show Text Excerpts"),
        Some(&pill(SHOW_EXCERPTS_ACTION)),
    );
    menu.append_section(None, &showing);
    let marking = gio::Menu::new();
    let glyphs = gio::Menu::new();
    for (label, mark) in [
        ("Bar", Mark::Bar),
        ("Feather", Mark::Feather),
        ("Fountain Pen", Mark::Pen),
    ] {
        glyphs.append_item(&radio(label, MARK_ACTION, mark.as_str()));
    }
    marking.append_submenu(Some("Selection Mark"), &glyphs);
    menu.append_section(None, &marking);
    menu
}

/// The detailed name of the pill's action `action`.
fn pill(action: &str) -> String {
    format!("{PILL_GROUP}.{action}")
}

/// A radio row of the pill's menu: `label`, firing `action` with `target`.
fn radio(label: &str, action: &str, target: &str) -> gio::MenuItem {
    let item = gio::MenuItem::new(Some(label), None);
    item.set_action_and_target_value(Some(&pill(action)), Some(&target.to_variant()));
    item
}

/// The state each of the pill's actions stands at for `library`: a radio's
/// the value its key holds, a check's whether its key is on.
fn pill_states(library: &settings::Library) -> [(&'static str, glib::Variant); 6] {
    [
        (SORT_ACTION, library.sort.as_str().to_variant()),
        (ORDER_ACTION, library.order.as_str().to_variant()),
        (PIN_FOLDERS_ACTION, library.pin_folders.to_variant()),
        (SHOW_DATE_ACTION, library.show_date.as_str().to_variant()),
        (SHOW_EXCERPTS_ACTION, library.show_excerpts.to_variant()),
        (MARK_ACTION, library.mark.as_str().to_variant()),
    ]
}

/// Adds the pill's six actions to `group`, each at its `[library]` default
/// until [`sync_pill`] says what the file holds.
///
/// An activation that names a value its key has — a radio's target, or a
/// check turned over — takes that state at once, so the menu reads the click,
/// and hands `write` the edit that moves the key. Anything else moves nothing.
fn pill_actions(
    group: &gio::SimpleActionGroup,
    write: impl Fn(&dyn Fn(&mut settings::Library)) + 'static,
) {
    let write = Rc::new(write);
    for (name, state) in pill_states(&settings::Library::default()) {
        let parameter = state.str().map(|_| glib::VariantTy::STRING);
        let action = gio::SimpleAction::new_stateful(name, parameter, &state);
        let writing = Rc::clone(&write);
        action.connect_activate(move |action, parameter| {
            let value = match parameter {
                Some(value) => value.clone(),
                None => {
                    let on = action.state().and_then(|state| state.get::<bool>());
                    (!on.unwrap_or(false)).to_variant()
                }
            };
            let name = action.name();
            if !set_key(&mut settings::Library::default(), &name, &value) {
                return;
            }
            action.set_state(&value);
            writing(&|library| {
                set_key(library, &name, &value);
            });
        });
        group.add_action(&action);
    }
}

/// What the Sort pill reads over what the File List shows, and whether its
/// menu can be opened: Recents keep the order they were opened in and a query
/// the engine's relevance, and neither is a sort a writer can choose.
fn pill_reads(showing: Option<&Showing>, query: &str, sort: Sort) -> (&'static str, bool) {
    match showing {
        Some(Showing::Recents) => (LAST_OPENED, false),
        _ if !query.is_empty() => (SEARCH_RELEVANCE, false),
        _ => (sort_title(sort), true),
    }
}

/// Turns every item of the pill's menu in `group` on or off together.
fn enable_pill(group: &gio::SimpleActionGroup, enabled: bool) {
    for name in group.list_actions() {
        if let Some(action) = group
            .lookup_action(&name)
            .and_downcast::<gio::SimpleAction>()
        {
            action.set_enabled(enabled);
        }
    }
}

/// Puts what `library` holds on to the pill's actions in `group`.
fn sync_pill(group: &gio::SimpleActionGroup, library: &settings::Library) {
    for (name, state) in pill_states(library) {
        let Some(action) = group
            .lookup_action(name)
            .and_downcast::<gio::SimpleAction>()
        else {
            continue;
        };
        if action.state().as_ref() != Some(&state) {
            action.set_state(&state);
        }
    }
}

/// Puts `value` into the `[library]` key the pill's action `name` moves,
/// answering whether it did: a string the key's choice does not name, or an
/// action the pill does not have, moves nothing.
fn set_key(library: &mut settings::Library, name: &str, value: &glib::Variant) -> bool {
    match (name, value.str(), value.get::<bool>()) {
        (SORT_ACTION, Some(text), _) => Sort::parse(text).map(|sort| library.sort = sort).is_some(),
        (ORDER_ACTION, Some(text), _) => Order::parse(text)
            .map(|order| library.order = order)
            .is_some(),
        (SHOW_DATE_ACTION, Some(text), _) => ShowDate::parse(text)
            .map(|date| library.show_date = date)
            .is_some(),
        (MARK_ACTION, Some(text), _) => Mark::parse(text).map(|mark| library.mark = mark).is_some(),
        (PIN_FOLDERS_ACTION, _, Some(on)) => {
            library.pin_folders = on;
            true
        }
        (SHOW_EXCERPTS_ACTION, _, Some(on)) => {
            library.show_excerpts = on;
            true
        }
        _ => false,
    }
}

/// The Sort band under the head, the pill at its left.
fn sort_row(button: &gtk::Button) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    row.set_height_request(SORT_BAND);
    button.set_margin_start(HEAD_LEFT);
    button.set_halign(gtk::Align::Start);
    button.set_valign(gtk::Align::Start);
    button.set_size_request(SORT_PILL.0, SORT_PILL.1);
    row.append(button);
    row
}

/// The Sort pill: the field the List is sorted by, and a chevron for the rest.
fn sort_button(label: &gtk::Label) -> gtk::Button {
    let line = gtk::Box::new(gtk::Orientation::Horizontal, SORT_GAP);
    label.set_hexpand(true);
    label.set_xalign(0.0);
    line.append(label);
    let chevron = icon((CHEV, CHEV), move |area, cr| chevron_icon(area, cr, true));
    chevron.add_css_class("lib-icon");
    line.append(&chevron);
    let button = gtk::Button::builder().child(&line).build();
    button.add_css_class("lib-sortb");
    button
}

/// One of the two words a conflict offers: a word in the band that
/// happens to be clicked rather than read, so it is a button with the pane's
/// own flat look on it.
fn offer_word(word: &str) -> gtk::Button {
    let button = gtk::Button::with_label(word);
    button.add_css_class("lib-btn");
    button.add_css_class("lib-offer");
    button
}

/// "Changed on disk · Reload · Keep", hidden until there is a conflict to
/// resolve.
fn offered(reload: &gtk::Button, keep: &gtk::Button) -> gtk::Box {
    let offer = gtk::Box::new(gtk::Orientation::Horizontal, BAND_GAP);
    offer.set_visible(false);
    offer.set_margin_start(FILTER_LEFT);
    offer.set_margin_end(FILTER_RIGHT);
    offer.set_margin_top(BAND_AIR);
    offer.set_margin_bottom(BAND_AIR);
    let said = gtk::Label::new(Some(CHANGED));
    said.add_css_class("lib-meta");
    offer.append(&said);
    offer.append(&separator());
    offer.append(reload);
    offer.append(&separator());
    offer.append(keep);
    offer
}

/// The "·" between two of the band's words.
fn separator() -> gtk::Label {
    let separator = gtk::Label::new(Some(SEPARATOR));
    separator.add_css_class("lib-meta");
    separator
}

/// The band above the Filter field: a notice's line, and the offer a Document
/// changed on disk under unsaved edits makes. Each is hidden until it has
/// something to say, so that at rest nothing stands between the last row and
/// the rule (#441 § The foot and the title bar).
fn band(notice: &gtk::Label, offer: &gtk::Box) -> gtk::Box {
    let band = gtk::Box::new(gtk::Orientation::Vertical, 0);
    notice.add_css_class("lib-meta");
    notice.set_xalign(0.0);
    notice.set_ellipsize(gtk::pango::EllipsizeMode::End);
    // An ellipsizing label still asks for its whole sentence as its natural width, and the pane
    // grows to it: the "no dictionary" notice doubled the Library's width (#415). One character
    // is its natural width now.
    notice.set_max_width_chars(1);
    notice.set_margin_start(FILTER_LEFT);
    notice.set_margin_end(FILTER_RIGHT);
    notice.set_margin_top(BAND_AIR);
    notice.set_margin_bottom(BAND_AIR);
    notice.set_visible(false);
    band.append(notice);
    band.append(offer);
    band
}

/// The excerpt's two lines, set on the leading the oracle gives them, with
/// the words a query matched, `matched` bytes into the text, set bold.
///
/// Through Pango rather than through the stylesheet: GTK's CSS has no
/// `line-height`, and two lines of 13.5 px type on their own natural leading
/// stand a pixel and a half tighter than iA's. The match is bold in the
/// excerpt's own grey and nothing else, so that a writer sees why the row is
/// here while it still reads as a row; iA marks nothing, and the bold is
/// Quill's own (#455).
fn excerpt_attributes(matched: Option<Range<usize>>) -> gtk::pango::AttrList {
    let attributes = gtk::pango::AttrList::new();
    let height = pixels(EXCERPT_LEADING * f64::from(gtk::pango::SCALE));
    attributes.insert(gtk::pango::AttrInt::new_line_height_absolute(height));
    if let Some(matched) = matched {
        let mut bold = gtk::pango::AttrInt::new_weight(gtk::pango::Weight::Bold);
        bold.set_start_index(u32::try_from(matched.start).unwrap_or(u32::MAX));
        bold.set_end_index(u32::try_from(matched.end).unwrap_or(u32::MAX));
        attributes.insert(bold);
    }
    attributes
}

/// A section's name: the folder's own, or the path where it has none.
fn place_name(path: &Path) -> String {
    match path.file_name() {
        Some(name) => name.to_string_lossy().into_owned(),
        None => path.display().to_string(),
    }
}

/// How wide a glyph is drawn on a row `pitch` tall, and how far in from the
/// List's edge it stands: centred on the bar's centre line, since the glyph is
/// several times the bar's width and a shared left edge reads as pushed right
/// (#457), and never past the List's edge.
fn mark_place(glyph: &Glyph, pitch: i32) -> (f64, i32) {
    let width = glyph.width_at(f64::from(pitch - 1 - BAR.top - BAR.bottom));
    let centre = f64::from(BAR.left) + f64::from(BAR.width) / 2.0;
    (width, pixels((centre - width / 2.0).round().max(0.0)))
}

/// The glyph `mark` is drawn with, tall or short, or `None` for the bar.
///
/// The four files are read once a process. One that cannot be read is said on
/// stderr that once and leaves its rows the bar, which is a row still marked
/// rather than a pane that will not open.
fn glyph(mark: Mark, short: bool) -> Option<&'static Glyph> {
    static GLYPHS: OnceLock<Vec<((Mark, bool), Glyph)>> = OnceLock::new();
    let glyphs = GLYPHS.get_or_init(|| {
        [Mark::Feather, Mark::Pen]
            .into_iter()
            .flat_map(|drawn| [(drawn, false), (drawn, true)])
            .filter_map(|(drawn, short)| {
                let path = quill_engine::mark::file(drawn, short)?;
                Glyph::read(&path)
                    .map_err(|error| eprintln!("quill: the {} mark: {error}", drawn.as_str()))
                    .ok()
                    .map(|glyph| ((drawn, short), glyph))
            })
            .collect()
    });
    glyphs
        .iter()
        .find(|(key, _)| *key == (mark, short))
        .map(|(_, glyph)| glyph)
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

/// The head of one file: what its row shows of it.
///
/// Read at most once per refresh, so that a Library of a thousand Documents is
/// a thousand bounded reads.
struct Head {
    /// Two lines of what the file says.
    excerpt: String,
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
                modified,
            };
        };
        Self {
            excerpt: prose(&read),
            modified,
        }
    }
}

/// What the sort control reads in each order.
fn sort_title(sort: Sort) -> &'static str {
    match sort {
        Sort::Modified => "Sort by Date Modified",
        Sort::Created => "Sort by Date Created",
        Sort::Name => "Sort by Name",
        Sort::Extension => "Sort by Extension",
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

/// A file's date, as its row says it.
///
/// Today is a time in the locale's short form, this year the month and day
/// (`Mar 14`), and older the month, day and two-digit year (`Mar 14, 25`);
/// the oracle's Yesterday and weekday forms went with #441 (D7). The month
/// stays English, as the rest of the pane is.
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
///
/// Which day a file was written decides the form, not how many hours ago: a
/// file written last night is a date at nine this morning.
fn dated(when: &glib::DateTime, now: &glib::DateTime) -> String {
    if (when.year(), when.day_of_year()) == (now.year(), now.day_of_year()) {
        return short_time(when);
    }
    let month = MONTHS[usize::try_from(when.month() - 1).unwrap_or(0).min(11)];
    if when.year() == now.year() {
        format!("{month} {}", when.day_of_month())
    } else {
        format!("{month} {}, {:02}", when.day_of_month(), when.year() % 100)
    }
}

/// The hour and minute of `when` in the locale's short form.
///
/// `%p` names the half of the day only in a locale that tells the time on
/// twelve hours, so its being empty is the twenty-four-hour clock.
fn short_time(when: &glib::DateTime) -> String {
    let half = when
        .format("%p")
        .map(|half| half.to_string())
        .unwrap_or_default();
    let form = if half.trim().is_empty() {
        "%H:%M"
    } else {
        "%l:%M %p"
    };
    when.format(form)
        .map(|time| time.trim().to_string())
        .unwrap_or_default()
}

/// The page mark: a page with its corner turned and two lines of text on it,
/// drawn at the width its row asked for.
fn document_icon(area: &gtk::DrawingArea, cr: &cairo::Context) {
    chrome::source(area, cr, 1.0);
    let scale = f64::from(area.content_width()) / f64::from(DOC_DRAWN);
    cr.scale(scale, scale);
    cr.set_line_width(1.1);
    cr.move_to(3.4, 8.6);
    cr.line_to(10.6, 8.6);
    cr.move_to(3.4, 11.4);
    cr.line_to(10.6, 11.4);
    let _ = cr.stroke();
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

/// How long the pane takes to slide in or out (#485), and a pin to go in or
/// come out (#441 § The pinned icon and its animation): the oracle's `.24s`
/// (`dev/legacy/app/css/files.css` at `legacy-last`, `#library` and `#app`:
/// `transition: .24s cubic-bezier(.22, .61, .36, 1)`). Both motions ease out
/// cubic: the pin through [`eased`], the pane through GTK's Revealer.
const MOTION_MS: u32 = 240;
/// How far back along its path a pin starts from, in the page icon's own
/// units: far enough that it comes in from outside the icon's cell.
const PIN_TRAVEL: f64 = 6.0;

/// What a page's row draws: the page, or the page with a pin through it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Page {
    Plain,
    Pinned,
}

/// The page the File List's row for `path` draws, `pinned` being the
/// Library's Pinned list.
fn page_of(path: &Path, pinned: &[PathBuf]) -> Page {
    if pinned.iter().any(|pin| pin == path) {
        Page::Pinned
    } else {
        Page::Plain
    }
}

/// The page an Organizer row draws, where it draws one: a pinned Document's is
/// the pinned page, and nothing else in the Organizer is a page.
fn org_page(row: &Organized) -> Option<Page> {
    match row {
        Organized::Pinned { folder: false, .. } => Some(Page::Pinned),
        _ => None,
    }
}

/// How a pin was asked for, which is where it comes in from.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Gesture {
    /// The row's menu.
    Menu,
    /// A drop on the Pinned section, with how far and which way the row
    /// travelled from where it was grabbed, in the pane's coordinates.
    Drop { travel: (f64, f64) },
}

/// The diagonal a pin travels along going in, a unit vector with y down.
///
/// The menu's is down and to the left, into the paper from the page's folded
/// corner; a drop's is the diagonal nearest the row's own travel, a travel with
/// no sideways part or no upward part taking the menu's for that part.
fn approach(gesture: Gesture) -> (f64, f64) {
    let (x, y) = match gesture {
        Gesture::Menu => (-1.0, 1.0),
        Gesture::Drop { travel: (dx, dy) } => (
            if dx > 0.0 { 1.0 } else { -1.0 },
            if dy < 0.0 { -1.0 } else { 1.0 },
        ),
    };
    let unit = std::f64::consts::FRAC_1_SQRT_2;
    (x * unit, y * unit)
}

/// How far along its [`MOTION_MS`] a pin is `elapsed` microseconds in, eased
/// out (cubic): 0 at the start, and 1 from the end on. The oracle's curve,
/// `cubic-bezier(.22, .61, .36, 1)`, is an ease-out close to this one.
fn eased(elapsed: i64) -> f64 {
    let t = (elapsed as f64 / (i64::from(MOTION_MS) * 1000) as f64).clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

/// Whether the pin moves: GTK's animations setting, which `--deterministic`
/// turns off ([`crate::harness::determine`]) as a desktop asking for no motion
/// does, so a shot is of the pin where it ends.
fn animated() -> bool {
    gtk::is_initialized()
        && gtk::Settings::default().is_some_and(|settings| settings.is_gtk_enable_animations())
}

/// Where a page's pin stands: `at` 1 is home, through the paper, and 0 is
/// [`PIN_TRAVEL`] back along `along` and unseen.
#[derive(Clone, Copy, Debug, PartialEq)]
struct PinPose {
    along: (f64, f64),
    at: f64,
}

impl PinPose {
    const HOME: Self = Self {
        along: (0.0, 0.0),
        at: 1.0,
    };
}

/// A pinned page drawn on a row: the pinned path, its icon, and where its pin
/// stands.
type PinnedPage = (PathBuf, gtk::DrawingArea, Rc<Cell<PinPose>>);

/// A pin asked for and not yet drawn: whose, and which way it comes in.
#[derive(Clone, Debug, PartialEq)]
struct Pinning {
    path: PathBuf,
    along: (f64, f64),
}

/// A pin through the page [`document_icon`] draws: a round head over the
/// folded corner and its needle down and to the left into the paper, in the
/// row's icon ink at the pose's strength and set back along its path.
fn pin_icon(area: &gtk::DrawingArea, cr: &cairo::Context, pose: PinPose) {
    let scale = f64::from(area.content_width()) / f64::from(DOC_DRAWN);
    let back = (1.0 - pose.at) * PIN_TRAVEL;
    chrome::source(area, cr, pose.at);
    cr.scale(scale, scale);
    cr.translate(-pose.along.0 * back, -pose.along.1 * back);
    cr.set_line_width(1.1);
    cr.set_line_cap(cairo::LineCap::Round);
    cr.move_to(9.4, 4.6);
    cr.line_to(5.4, 8.6);
    let _ = cr.stroke();
    cr.arc(10.4, 3.6, 2.2, 0.0, std::f64::consts::TAU);
    let _ = cr.fill();
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

/// The magnifier in the Filter field (`files.js` `I.search`), drawn at the side
/// it is asked for.
fn magnifier_icon(area: &gtk::DrawingArea, cr: &cairo::Context) {
    chrome::source(area, cr, 1.0);
    let scale = f64::from(area.content_width()) / f64::from(MAG);
    cr.scale(scale, scale);
    cr.set_line_width(1.3);
    cr.set_line_cap(cairo::LineCap::Round);
    cr.arc(5.15, 5.15, 3.7, 0.0, std::f64::consts::TAU);
    let _ = cr.stroke();
    cr.move_to(7.9, 7.9);
    cr.line_to(10.8, 10.8);
    let _ = cr.stroke();
}

/// The Filter capsule's clear button: a disc in the widget's colour with a
/// cross cut out of it, as State 28's search frame draws it.
fn clear_icon(area: &gtk::DrawingArea, cr: &cairo::Context) {
    chrome::source(area, cr, 1.0);
    let side = f64::from(area.content_width());
    let half = side / 2.0;
    cr.arc(half, half, half, 0.0, std::f64::consts::TAU);
    let _ = cr.fill();
    cr.set_operator(cairo::Operator::Clear);
    cr.set_line_width(side * 0.12);
    cr.set_line_cap(cairo::LineCap::Round);
    let arm = side * 0.2;
    cr.move_to(half - arm, half - arm);
    cr.line_to(half + arm, half + arm);
    cr.move_to(half + arm, half - arm);
    cr.line_to(half - arm, half + arm);
    let _ = cr.stroke();
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

    #[test]
    fn the_organizer_takes_half_of_what_the_divider_adds() {
        let widths = settings::library_widths();
        let (narrowest, widest) = (*widths.start(), *widths.end());
        let narrowest = i32::try_from(narrowest).expect("a width");
        let widest = i32::try_from(widest).expect("a width");
        assert_eq!(narrowest, WIDTH, "the pane's floor is its default");
        assert_eq!(organizer_width(WIDTH), ORGANIZER, "State 28 at the floor");
        assert_eq!(organizer_width(WIDTH - 120), ORGANIZER, "never narrower");
        assert_eq!(organizer_width(WIDTH + 2), ORGANIZER + 1);
        let wide = organizer_width(widest);
        assert_eq!(wide, ORGANIZER + (widest - WIDTH) / 2);
        assert_eq!(
            widest - wide,
            WIDTH - ORGANIZER + (widest - WIDTH) / 2,
            "the File List takes the other half"
        );
    }

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
    fn a_content_hits_match_is_bold_and_nothing_else_is_marked() {
        // Its own folder, named for this process, because the worktrees test
        // at the same time.
        let root = std::env::temp_dir().join(format!("quill-sidebar-{}", std::process::id()));
        std::fs::create_dir_all(&root).expect("a folder to search");
        std::fs::write(root.join("harbour.md"), "The lamps. The sea was flat.\n")
            .expect("a file to find");
        let library = Library::open(std::slice::from_ref(&root), &[]);
        let view = View::default();
        let mut contents = Contents::new();
        let found = library.search("sea", &view, &mut contents);
        let snippet = found
            .first()
            .expect("the file's text holds the word")
            .snippet()
            .expect("a content hit carries a snippet");
        assert_eq!(snippet.matched(), "sea");
        // The match is bold over exactly its own bytes, in the excerpt's own
        // grey: the attributes carry the leading and the weight, no colour.
        let attributes = excerpt_attributes(Some(snippet.at())).attributes();
        let kinds: Vec<_> = attributes
            .iter()
            .map(|attribute| attribute.type_())
            .collect();
        assert_eq!(
            kinds,
            vec![
                gtk::pango::AttrType::AbsoluteLineHeight,
                gtk::pango::AttrType::Weight
            ]
        );
        let weight = &attributes[1];
        let span = usize::try_from(weight.start_index()).expect("an index")
            ..usize::try_from(weight.end_index()).expect("an index");
        assert_eq!(span, snippet.at());
        // Pango's bold is weight 700.
        assert_eq!(
            weight
                .downcast_ref::<gtk::pango::AttrInt>()
                .map(|bold| bold.value()),
            Some(700)
        );
        // A name hit, or any file's own beginning, carries no weight at all.
        let plain: Vec<_> = excerpt_attributes(None)
            .attributes()
            .into_iter()
            .map(|attribute| attribute.type_())
            .collect();
        assert_eq!(plain, vec![gtk::pango::AttrType::AbsoluteLineHeight]);
        std::fs::remove_dir_all(&root).expect("the folder to go");
    }

    #[test]
    fn the_pill_reads_search_relevance_and_is_off_while_a_query_stands() {
        let group = gio::SimpleActionGroup::new();
        pill_actions(&group, |_| {});
        for (query, reads, on) in [
            ("sea", SEARCH_RELEVANCE, false),
            ("", "Sort by Date Modified", true),
        ] {
            let (label, enabled) = pill_reads(None, query, Sort::Modified);
            assert_eq!((label, enabled), (reads, on), "under {query:?}");
            enable_pill(&group, enabled);
            for name in group.list_actions() {
                assert_eq!(group.is_action_enabled(&name), on, "{name} under {query:?}");
            }
        }
        assert_eq!(
            pill_reads(Some(&Showing::Recents), "sea", Sort::Modified),
            (LAST_OPENED, false)
        );
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
    fn the_excerpt_cap_outlasts_two_lines_of_the_widest_file_list() {
        // The widest File List, and the most characters two lines of it could
        // hold if every one were as narrow as an `i` (a fifth of the type size):
        // the cap stays past that, so a long excerpt still ends in an ellipsis.
        let pane = *settings::library_widths().end() as i32;
        let list = f64::from(pane - organizer_width(pane));
        let most = 2.0 * list / (ROW_PX * 0.2);
        assert!(
            EXCERPT_CHARS as f64 > most,
            "{EXCERPT_CHARS} characters against {most:.0} that two lines hold"
        );
    }

    #[test]
    fn the_filter_rings_for_a_query_and_not_for_spaces() {
        assert!(!live(""));
        assert!(!live("   "));
        assert!(!live("\t"));
        assert!(live("sea"));
        assert!(live("  sea "));
    }

    #[test]
    fn a_date_is_a_time_today_the_month_and_day_this_year_then_the_year() {
        let now = at(2026, 3, 4, 15);
        // Today: a time, on whichever clock the test's locale keeps.
        for (hour, forms) in [
            (9, ["9:00 AM", "09:00"]),
            (0, ["12:00 AM", "00:00"]),
            (13, ["1:00 PM", "13:00"]),
        ] {
            let said = dated(&at(2026, 3, 4, hour), &now);
            assert!(forms.contains(&said.as_str()), "{hour} h today said {said}");
        }
        // This year: the month and day, yesterday and last week included.
        assert_eq!(dated(&at(2026, 3, 3, 23), &now), "Mar 3");
        assert_eq!(dated(&at(2026, 3, 1, 9), &now), "Mar 1");
        assert_eq!(dated(&at(2026, 1, 9, 9), &now), "Jan 9");
        // Older: with the two-digit year, the same day a year ago too.
        assert_eq!(dated(&at(2025, 3, 4, 9), &now), "Mar 4, 25");
        assert_eq!(dated(&at(2025, 3, 14, 9), &now), "Mar 14, 25");
        assert_eq!(dated(&at(2009, 12, 31, 9), &now), "Dec 31, 09");
    }

    /// What `model` offers, submenus flattened into their rows: each row's
    /// label (a submenu's under its head, `Head › Row`), action and target,
    /// the target empty for a check.
    fn pill_rows(model: &gio::MenuModel, head: &str, into: &mut Vec<(String, String, String)>) {
        for at in 0..model.n_items() {
            let string = |attribute: &str| {
                model
                    .item_attribute_value(at, attribute, None)
                    .and_then(|value| value.str().map(String::from))
                    .unwrap_or_default()
            };
            if let Some(submenu) = model.item_link(at, "submenu") {
                pill_rows(&submenu, &format!("{} › ", string("label")), into);
                continue;
            }
            into.push((
                format!("{head}{}", string("label")),
                string("action"),
                string("target"),
            ));
        }
    }

    /// The pill's menu is iA's five groups in order, each row carrying its
    /// action and target, and the rows a writer who has never opened it sees
    /// checked are the `[library]` defaults (#446).
    #[test]
    fn the_sort_pills_menu_is_five_groups_with_the_defaults_checked() {
        let model = pill_menu();
        let groups: Vec<Vec<(String, String, String)>> = (0..model.n_items())
            .map(|at| {
                let mut rows = Vec::new();
                let section = model.item_link(at, "section").expect("a group");
                pill_rows(&section, "", &mut rows);
                rows
            })
            .collect();
        let row = |label: &str, action: &str, target: &str| {
            (
                label.to_string(),
                format!("lib.{action}"),
                target.to_string(),
            )
        };
        assert_eq!(
            groups,
            vec![
                vec![
                    row("Date Modified", "sort", "modified"),
                    row("Date Created", "sort", "created"),
                    row("Name", "sort", "name"),
                    row("Extension", "sort", "extension"),
                ],
                vec![
                    row("Oldest on Top", "order", "oldest"),
                    row("Newest on Top", "order", "newest"),
                ],
                vec![row("Pin Folders to Top", "pin-folders", "")],
                vec![
                    row("Show Date › Date Modified", "show-date", "modified"),
                    row("Show Date › Date Created", "show-date", "created"),
                    row("Show Date › None", "show-date", "none"),
                    row("Show Text Excerpts", "show-excerpts", ""),
                ],
                vec![
                    row("Selection Mark › Bar", "mark", "bar"),
                    row("Selection Mark › Feather", "mark", "feather"),
                    row("Selection Mark › Fountain Pen", "mark", "pen"),
                ],
            ]
        );
        let group = gio::SimpleActionGroup::new();
        pill_actions(&group, |_: &dyn Fn(&mut settings::Library)| {});
        let checked: Vec<&str> = groups
            .iter()
            .flatten()
            .filter(|(_, action, target)| {
                let name = action.trim_start_matches("lib.");
                let state = group.action_state(name).expect("a stateful action");
                match state.str() {
                    Some(value) => value == target,
                    None => state.get::<bool>() == Some(true),
                }
            })
            .map(|(label, _, _)| label.as_str())
            .collect();
        assert_eq!(
            checked,
            [
                "Date Modified",
                "Newest on Top",
                "Pin Folders to Top",
                "Show Date › Date Modified",
                "Show Text Excerpts",
                "Selection Mark › Bar",
            ]
        );
    }

    /// Each of the pill's actions writes its one `[library]` key, a target no
    /// key names writes nothing, and the view the pane lists through, built
    /// from the table read back, carries the field, order and placement. The
    /// popover's reach to the `lib` group needs a window, so only the Hand test
    /// covers that path.
    #[test]
    fn each_pill_item_writes_its_key_and_the_view_reads_it_back() {
        let written = Rc::new(RefCell::new(settings::Settings::default()));
        let group = gio::SimpleActionGroup::new();
        let into = Rc::clone(&written);
        pill_actions(&group, move |edit: &dyn Fn(&mut settings::Library)| {
            edit(&mut into.borrow_mut().library);
        });
        group.activate_action("sort", Some(&"name".to_variant()));
        group.activate_action("order", Some(&"oldest".to_variant()));
        group.activate_action("pin-folders", None);
        group.activate_action("show-date", Some(&"none".to_variant()));
        group.activate_action("show-excerpts", None);
        group.activate_action("mark", Some(&"pen".to_variant()));
        group.activate_action("sort", Some(&"birthday".to_variant()));
        assert_eq!(
            group
                .action_state("sort")
                .and_then(|state| state.get::<String>()),
            Some("name".to_string()),
            "a target no key names leaves the radio where it was"
        );

        let path =
            std::env::temp_dir().join(format!("quill-sidebar-pill-{}.toml", std::process::id()));
        written
            .borrow()
            .write_to(&path)
            .expect("the settings are written");
        let (read, _) = settings::Settings::read_from(&path);
        std::fs::remove_file(&path).ok();
        let library = &read.library;
        assert_eq!(
            (library.sort, library.order, library.pin_folders),
            (Sort::Name, Order::Oldest, false)
        );
        assert_eq!(
            (library.show_date, library.show_excerpts, library.mark),
            (ShowDate::None, false, Mark::Pen)
        );
        let view = view_of(library);
        assert_eq!(
            (view.sort, view.order, view.pin_folders),
            (Sort::Name, Order::Oldest, false)
        );

        sync_pill(&group, &settings::Library::default());
        assert_eq!(
            group
                .action_state("pin-folders")
                .and_then(|state| state.get::<bool>()),
            Some(true),
            "a refresh puts the checks back to what the file holds"
        );
    }

    #[test]
    fn the_fixtures_mtimes_are_the_dates_the_oracles_shot_shows() {
        // `dev/shots/oracle/library/manifest.json` stamps sea-storm.md at
        // 1741942800, which the frozen shot dates `Mar 14, 25`.
        let stamped = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_741_942_800);
        assert_eq!(stamp(stamped, &at(2026, 9, 3, 12)), "Mar 14, 25");
    }

    /// The Organizer lists its three sections in order: every Location, the
    /// one the File List shows on its pill; the pins of every Location, a
    /// Document's extension dropped; and Recents (#441 § The Organizer).
    #[test]
    fn the_organizer_lists_its_sections_in_order_with_what_the_list_shows_marked() {
        let novel = Path::new("/w/novel");
        let essays = Path::new("/w/essays");
        let draft = Path::new("/w/novel/draft.md");
        let notes = Path::new("/w/essays/notes");
        let locations = [novel, essays];
        let pinned = [(draft, false), (notes, true)];
        let on_essays = Showing::Location(essays.to_path_buf());
        assert_eq!(
            organized(&locations, &pinned, Some(&on_essays), false),
            vec![
                Organized::Head(LOCATIONS),
                Organized::Location {
                    path: novel.to_path_buf(),
                    name: "novel".into(),
                    on: false,
                },
                Organized::Location {
                    path: essays.to_path_buf(),
                    name: "essays".into(),
                    on: true,
                },
                Organized::Head(PINNED),
                Organized::Pinned {
                    path: draft.to_path_buf(),
                    name: "draft".into(),
                    folder: false,
                    on: false,
                },
                Organized::Pinned {
                    path: notes.to_path_buf(),
                    name: "notes".into(),
                    folder: true,
                    on: false,
                },
                Organized::Head(RECENTS),
                Organized::Recents { on: false },
            ]
        );
        let marked = |showing: &Showing| {
            organized(&locations, &pinned, Some(showing), false)
                .into_iter()
                .filter(|row| {
                    matches!(
                        row,
                        Organized::Location { on: true, .. }
                            | Organized::Pinned { on: true, .. }
                            | Organized::Recents { on: true }
                    )
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(
            marked(&Showing::Recents),
            vec![Organized::Recents { on: true }]
        );
        assert_eq!(
            marked(&Showing::Folder(notes.to_path_buf())),
            vec![Organized::Pinned {
                path: notes.to_path_buf(),
                name: "notes".into(),
                folder: true,
                on: true,
            }]
        );
        assert!(
            organized(&locations, &[], None, false).contains(&Organized::NothingPinned),
            "an empty Pinned section says how to pin"
        );
    }

    /// A pinned Document's page carries the pin in the File List and in the
    /// Organizer, and nothing else's does.
    #[test]
    fn a_pinned_documents_page_carries_the_pin_in_both_places() {
        let essays = Path::new("/w/essays");
        let draft = Path::new("/w/essays/draft.md");
        let notes = Path::new("/w/essays/notes");
        let other = Path::new("/w/essays/other.md");
        let pinned = [draft.to_path_buf(), notes.to_path_buf()];
        assert_eq!(page_of(draft, &pinned), Page::Pinned);
        assert_eq!(page_of(other, &pinned), Page::Plain);
        let rows = organized(&[essays], &[(draft, false), (notes, true)], None, false);
        let paged: Vec<(&Organized, Page)> = rows
            .iter()
            .filter_map(|row| org_page(row).map(|page| (row, page)))
            .collect();
        assert_eq!(
            paged,
            vec![(
                &Organized::Pinned {
                    path: draft.to_path_buf(),
                    name: "draft".into(),
                    folder: false,
                    on: false,
                },
                Page::Pinned
            )],
            "the pinned Document alone draws a page, and it is the pinned one"
        );
    }

    /// A pin comes in along the diagonal of the gesture that asked for it, and
    /// settles in 240 ms, eased out.
    #[test]
    fn a_pin_comes_in_along_its_gesture() {
        let unit = std::f64::consts::FRAC_1_SQRT_2;
        assert_eq!(
            approach(Gesture::Menu),
            (-unit, unit),
            "from the menu, down and into the paper"
        );
        assert_eq!(
            approach(Gesture::Drop {
                travel: (-40.0, 120.0)
            }),
            (-unit, unit),
            "a row dragged down and left"
        );
        assert_eq!(
            approach(Gesture::Drop {
                travel: (30.0, -200.0)
            }),
            (unit, -unit),
            "a row dragged up and right"
        );
        assert_eq!(
            approach(Gesture::Drop { travel: (0.0, 0.0) }),
            approach(Gesture::Menu),
            "a drop that went nowhere comes in as the menu's does"
        );
        assert!(eased(0).abs() < f64::EPSILON);
        assert!(eased(60_000) > 0.25, "eased out: most of the way early");
        assert!((eased(i64::from(MOTION_MS) * 1000) - 1.0).abs() < f64::EPSILON);
        assert!((eased(i64::from(MOTION_MS) * 2000) - 1.0).abs() < f64::EPSILON);
    }

    /// What the File List shows holds while the Library holds it, and falls
    /// back to the first Location once it does not.
    #[test]
    fn a_choice_the_library_no_longer_holds_falls_back_to_the_first_location() {
        let novel = Path::new("/w/novel");
        let essays = Path::new("/w/essays");
        let notes = Path::new("/w/essays/notes");
        let first = Some(Showing::Location(novel.to_path_buf()));
        assert_eq!(settled(None, &[novel, essays], &[]), first);
        let chosen = Showing::Location(essays.to_path_buf());
        assert_eq!(
            settled(Some(&chosen), &[novel, essays], &[]),
            Some(chosen.clone())
        );
        assert_eq!(settled(Some(&chosen), &[novel], &[]), first);
        let folder = Showing::Folder(notes.to_path_buf());
        assert_eq!(
            settled(Some(&folder), &[novel, essays], &[(notes, true)]),
            Some(folder.clone())
        );
        assert_eq!(settled(Some(&folder), &[novel, essays], &[]), first);
        assert_eq!(
            settled(Some(&Showing::Recents), &[novel], &[]),
            Some(Showing::Recents)
        );
        assert_eq!(settled(None, &[], &[]), None);
    }

    /// Recents are the state's recents the Library holds, newest first, and
    /// the Filter narrows them by name and by text as the Library's search
    /// does (#441 story 29).
    #[test]
    fn recents_are_the_states_newest_first_and_the_filter_narrows_them_as_search_does() {
        let root =
            std::env::temp_dir().join(format!("quill-sidebar-recents-{}", std::process::id()));
        std::fs::create_dir_all(&root).expect("a folder of recents");
        for (name, text) in [
            ("sea-wall.md", "Stone.\n"),
            ("harbour.md", "The sea was flat.\n"),
            ("lamps.md", "Dark.\n"),
            ("storm.md", "The sea rose.\n"),
        ] {
            std::fs::write(root.join(name), text).expect("a file to open");
        }
        let library = Library::open(std::slice::from_ref(&root), &[]);
        let opened = vec![
            root.join("lamps.md"),
            root.join("harbour.md"),
            root.join("sea-wall.md"),
            PathBuf::from("/nowhere/else.md"),
        ];
        let names = |files: Vec<&File>| {
            files
                .iter()
                .map(|file| file.name().to_string())
                .collect::<Vec<_>>()
        };
        assert_eq!(
            names(recent_files(&library, &opened)),
            ["lamps.md", "harbour.md", "sea-wall.md"]
        );
        let mut contents = Contents::new();
        let view = View::default();
        let matches = owned(&library.search("sea", &view, &mut contents));
        let found = recent_found(found_files(&library, "sea", &view, Some(&matches)), &opened);
        assert_eq!(
            names(found.iter().map(|(file, _)| *file).collect()),
            ["sea-wall.md", "harbour.md"],
            "the name hit first, then the text hit, and never storm.md, which was not opened"
        );
        std::fs::remove_dir_all(&root).expect("the folder to go");
    }

    /// Recents chosen under a Filter of spaces lists every recent, as under an
    /// empty one; under a word it lists the recents the word finds. Either way
    /// the pill reads Last Opened and cannot be opened (#441 step 4).
    #[test]
    fn recents_under_a_blank_filter_list_whole_and_under_a_word_narrow() {
        let root = std::env::temp_dir().join(format!(
            "quill-sidebar-recents-blank-{}",
            std::process::id()
        ));
        std::fs::remove_dir_all(&root).ok();
        std::fs::create_dir_all(&root).expect("a folder of recents");
        for (name, text) in [("sea-wall.md", "Stone.\n"), ("lamps.md", "Dark.\n")] {
            std::fs::write(root.join(name), text).expect("a file to open");
        }
        let library = Library::open(std::slice::from_ref(&root), &[]);
        let opened = vec![root.join("lamps.md"), root.join("sea-wall.md")];
        let view = View::default();
        let expanded = BTreeSet::new();
        let listed = |typed: &str| {
            let query = query_of(typed);
            let matches = owned(&library.search(query, &view, &mut Contents::new()));
            listing(
                &library,
                Some(&Showing::Recents),
                query,
                &view,
                &opened,
                Some(&matches),
                &expanded,
            )
            .files()
            .into_iter()
            .map(|file| file.name().to_string())
            .collect::<Vec<_>>()
        };
        assert_eq!(listed(""), ["lamps.md", "sea-wall.md"]);
        assert_eq!(listed("   "), listed(""), "spaces are no query");
        assert_eq!(listed(" sea "), ["sea-wall.md"]);
        for typed in ["", "   ", " sea "] {
            assert_eq!(
                pill_reads(Some(&Showing::Recents), query_of(typed), Sort::Modified),
                (LAST_OPENED, false),
                "{typed:?}"
            );
        }
        std::fs::remove_dir_all(&root).expect("the folder to go");
    }

    /// A search's answer to a query the writer has since typed past is
    /// dropped; until the standing query's answer comes its names are listed
    /// alone, and then its name hits first and its content hits after, in the
    /// engine's order (#454).
    #[test]
    fn a_superseded_answer_is_dropped_and_the_standing_one_lists_names_then_texts() {
        let root = std::env::temp_dir().join(format!("quill-sidebar-ask-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        std::fs::create_dir_all(&root).expect("a folder to search");
        std::fs::write(root.join("sea-wall.md"), "Stones.\n").expect("a name hit");
        std::fs::write(root.join("harbour.md"), "The lamps. The sea was flat.\n")
            .expect("a text hit");
        std::fs::write(root.join("storm.md"), "Rain.\n").expect("no hit");
        let library = Library::open(std::slice::from_ref(&root), &[]);
        let view = View::default();
        let listed = |searching: &Searching| {
            found_files(&library, "sea", &view, searching.matches("sea", view))
                .iter()
                .map(|(file, snippet)| (file.name().to_string(), snippet.is_some()))
                .collect::<Vec<_>>()
        };
        let answered = |searching: &Searching, question: &Question| {
            question.answer(&mut searching.contents.lock().expect("the cache"))
        };

        let mut searching = Searching::new();
        let typed_past = searching
            .ask("se", view, &library)
            .expect("a first question");
        let standing = searching
            .ask("sea", view, &library)
            .expect("a second question");
        assert!(
            searching.ask("sea", view, &library).is_none(),
            "the standing question is not asked again"
        );
        assert!(!searching.take(answered(&searching, &typed_past)));
        assert!(searching.owed());
        assert_eq!(listed(&searching), [("sea-wall.md".to_string(), false)]);
        assert!(searching.take(answered(&searching, &standing)));
        assert!(!searching.owed());
        assert_eq!(
            listed(&searching),
            [
                ("sea-wall.md".to_string(), false),
                ("harbour.md".to_string(), true)
            ]
        );
        std::fs::remove_dir_all(&root).expect("the folder to go");
    }

    /// The search thread answers the standing question over the channel, a
    /// queued question since replaced is never searched, and the second
    /// query over an untouched tree reads nothing (#454).
    #[test]
    fn the_search_thread_answers_the_standing_question_alone() {
        let root =
            std::env::temp_dir().join(format!("quill-sidebar-thread-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        std::fs::create_dir_all(&root).expect("a folder to search");
        std::fs::write(root.join("harbour.md"), "The lamps. The sea was flat.\n")
            .expect("a text hit");
        let library = Library::open(std::slice::from_ref(&root), &[]);
        let view = View::default();
        let mut searching = Searching::new();
        // Holding the cache queues the first question behind the second.
        let held = Arc::clone(&searching.contents);
        let guard = held.lock().expect("the cache");
        let first = searching.ask("lamps", view, &library).expect("a question");
        searching.spawn(first);
        let second = searching.ask("sea", view, &library).expect("a question");
        searching.spawn(second);
        drop(guard);
        let answer = searching
            .answers
            .recv_timeout(Duration::from_secs(10))
            .expect("the search thread answers");
        assert!(searching.take(answer), "the standing question's answer");
        assert!(
            searching
                .answers
                .recv_timeout(Duration::from_millis(200))
                .is_err(),
            "the replaced question is never searched"
        );
        assert_eq!(held.lock().expect("the cache").reads(), 1);
        let again = searching
            .ask("the sea", view, &library)
            .expect("a question");
        searching.spawn(again);
        let answer = searching
            .answers
            .recv_timeout(Duration::from_secs(10))
            .expect("the search thread answers");
        assert!(searching.take(answer));
        assert_eq!(
            held.lock().expect("the cache").reads(),
            1,
            "an untouched tree is not read again"
        );
        std::fs::remove_dir_all(&root).expect("the folder to go");
    }

    /// The File List's items are the listing's rows in the order the listing
    /// hands them over — a tree's folders and files at their depths, a query's
    /// hits and the recents flat — each named as its row names it (#460).
    #[test]
    fn the_lists_items_are_the_listings_rows_in_order() {
        let root = std::env::temp_dir().join(format!("quill-sidebar-items-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        std::fs::create_dir_all(root.join("inner")).expect("the Location");
        for (path, says) in [
            (root.join("sea.md"), "The sea.\n"),
            (root.join("storm.md"), "The storm.\n"),
            (root.join("inner/harbour.md"), "The harbour.\n"),
        ] {
            std::fs::write(path, says).expect("a file to list");
        }
        let library = Library::open(std::slice::from_ref(&root), &[]);
        let showing = Showing::Location(root.clone());
        let view = View::default();
        let shown = settings::Library::default();
        let open = BTreeSet::from([root.join("inner")]);
        let paths = |items: &[Item]| {
            items
                .iter()
                .map(|item| item.path.clone())
                .collect::<Vec<_>>()
        };

        let tree = listing(&library, Some(&showing), "", &view, &[], None, &open);
        let Listing::Tree(rows) = &tree else {
            panic!("a Location lists its tree");
        };
        let items = items_of(&tree, &shown, &[], None, &open);
        assert_eq!(
            items
                .iter()
                .map(|item| (item.path.clone(), item.folder, item.depth))
                .collect::<Vec<_>>(),
            rows.iter()
                .map(|row| (
                    row.path().to_path_buf(),
                    matches!(row, Row::Folder { .. }),
                    row.depth()
                ))
                .collect::<Vec<_>>()
        );
        assert_eq!(items.len(), 4, "the open folder's file is listed");
        assert!(items.iter().any(|item| item.folder && item.open));
        assert!(
            items
                .iter()
                .filter(|item| !item.folder)
                .all(|item| !item.name.ends_with(".md")),
            "{items:?}"
        );

        let hits = listing(&library, Some(&showing), "sea", &view, &[], None, &open);
        let Listing::Flat(found) = &hits else {
            panic!("a query lists its hits");
        };
        let items = items_of(&hits, &shown, &[], None, &open);
        assert_eq!(
            paths(&items),
            found
                .iter()
                .map(|(file, _)| file.path().to_path_buf())
                .collect::<Vec<_>>()
        );
        assert!(items.iter().all(|item| item.depth == 0 && !item.folder));

        let opened = vec![root.join("storm.md"), root.join("sea.md")];
        let recents = listing(
            &library,
            Some(&Showing::Recents),
            "",
            &view,
            &opened,
            None,
            &open,
        );
        assert_eq!(paths(&items_of(&recents, &shown, &[], None, &open)), opened);
        std::fs::remove_dir_all(&root).expect("the folder to go");
    }

    /// A refresh reads no head; a head is read as its row is bound, once, and
    /// not again until its file is written; and a head of a file the listing
    /// no longer holds — the other Location's — is let go of (#453, #460).
    #[test]
    fn a_head_is_read_as_its_row_is_bound_and_only_the_listed_are_held() {
        let root = std::env::temp_dir().join(format!("quill-sidebar-heads-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let (listed, other) = (root.join("listed"), root.join("other"));
        std::fs::create_dir_all(listed.join("inner")).expect("the listed Location");
        std::fs::create_dir_all(&other).expect("the other Location");
        for path in [
            listed.join("sea.md"),
            listed.join("storm.md"),
            listed.join("inner/harbour.md"),
            other.join("lamps.md"),
            other.join("wall.md"),
            other.join("tide.md"),
        ] {
            std::fs::write(path, "The sea.\n").expect("a file to list");
        }
        let library = Library::open(&[listed.clone(), other.clone()], &[]);
        let showing = Showing::Location(listed.clone());
        let closed = BTreeSet::new();
        let listing = listing(
            &library,
            Some(&showing),
            "",
            &View::default(),
            &[],
            None,
            &closed,
        );
        let items = items_of(&listing, &settings::Library::default(), &[], None, &closed);
        assert_eq!(items.len(), 3, "the listed Location's folder and two files");
        assert!(items.iter().all(|item| item.path.starts_with(&listed)));

        let mut read = BTreeMap::new();
        let lamps = other.join("lamps.md");
        assert!(head_of(&mut read, &lamps, None).1, "a head from before");
        prune_heads(&mut read, listing.files());
        assert!(read.is_empty(), "the other Location's head is let go of");

        let bound = items.iter().find(|item| !item.folder).expect("a file row");
        assert!(
            head_of(&mut read, &bound.path, bound.modified).1,
            "read as bound"
        );
        assert!(
            !head_of(&mut read, &bound.path, bound.modified).1,
            "nothing written, nothing read"
        );
        assert_eq!(read.len(), 1, "only the bound row's head is held");
        assert!(
            head_of(&mut read, &bound.path, Some(SystemTime::UNIX_EPOCH)).1,
            "a write is read again"
        );
        std::fs::remove_dir_all(&root).expect("the folder to go");
    }

    /// The pane's grounds and its grey are the theme's three roles on both
    /// grounds, and its bar is the accent (#441 § Grounds and roles).
    #[test]
    fn the_panes_grounds_and_grey_are_their_roles_and_the_bar_is_the_accent() {
        for scheme in [Scheme::Light, Scheme::Dark] {
            let ground = Ground::of(scheme);
            let sheet = stylesheet(ground);
            let hex = |role| ground.colours.colour(role).to_hex();
            for (rule, role) in [
                (".lib-org {\n  background-color: ", Role::OrganizerBg),
                (".lib-list { background-color: ", Role::FileListBg),
                (
                    "label.lib-excerpt { font-size: 13.5px; color: ",
                    Role::Secondary,
                ),
                ("placeholder { color: ", Role::Secondary),
                ("row.lib-open .lib-bar { background-color: ", Role::Accent),
                ("row.lib-open .lib-mark { color: ", Role::Accent),
            ] {
                let wanted = hex(role);
                assert!(
                    sheet.contains(&format!("{rule}{wanted};")),
                    "{scheme:?}: no `{rule}{wanted}` for {role:?} in\n{sheet}"
                );
            }
            let date = sheet
                .split_once("label.lib-date {")
                .expect("no date rule")
                .1;
            let date = date.split_once('}').expect("an unclosed date rule").0;
            assert!(
                date.contains(&format!("color: {};", hex(Role::Secondary))),
                "{scheme:?}: the date is not the secondary grey:\n{sheet}"
            );
        }
    }

    /// Every ink the pane draws follows a writer's palette (#456): a file
    /// naming only a paper, an ink and an accent moves the Location pill, the
    /// Sort pill, the Filter ring and its icons off the capture's greys and
    /// blue, to what [`PaneInks`] derives from that table.
    #[test]
    fn a_palettes_paper_ink_and_accent_reach_the_pills_the_ring_and_the_icons() {
        let (palette, notes) = quill_engine::theme::Palette::parse(
            "[light]\npaper = \"#f0e0c0\"\nink = \"#402000\"\naccent = \"#ff8000\"\n",
        );
        assert!(notes.is_empty(), "{notes:?}");
        let ground = Ground::overlaid(Scheme::Light, &palette);
        let sheet = stylesheet(ground);
        let built_in = stylesheet(Ground::of(Scheme::Light));
        let inks = PaneInks::of(Scheme::Light, &ground.colours);
        for (rule, ink) in [
            (".lib-pill {\n  background-color: ", inks.org_pill),
            (
                "button.lib-sortb label { font-size: 12px; color: ",
                inks.sort_ink,
            ),
            ("border-color: ", inks.field_focus),
            (".lib-filter .lib-clear { color: ", inks.field_clear),
            (".lib-filter .lib-icon { color: ", inks.field_icon),
        ] {
            let wanted = format!("{rule}{};", ink.to_hex());
            assert!(sheet.contains(&wanted), "no `{wanted}` in\n{sheet}");
            assert!(
                !built_in.contains(&wanted),
                "`{wanted}` is still the capture's"
            );
        }
    }

    /// Hover draws nothing and a selected row takes no fill (State 28), so the
    /// rules on a row's state only clear GTK's own; the Sort pill clears the
    /// Default theme's gradient as well as its colour (#437); the Filter prompt
    /// stands at full strength, where the theme's 0.55 left Quill's at
    /// `#BCBCBC` (#379); and no rule is drawn at the pane's edge.
    #[test]
    fn a_row_takes_no_hover_and_no_fill_and_the_theme_paints_nothing_of_its_own() {
        let sheet = stylesheet(Ground::default());
        let rule = |selector: &str| {
            sheet
                .split_once(selector)
                .unwrap_or_else(|| panic!("no `{selector}` rule in\n{sheet}"))
                .1
                .split_once('}')
                .expect("an unclosed rule")
                .0
                .trim()
                .to_string()
        };
        assert_eq!(rule("list > row:hover {"), "background: none;");
        assert_eq!(rule("list > row:selected {"), "background: none;");
        assert_eq!(rule("listview > row:hover {"), "background: none;");
        assert_eq!(rule("listview > row:selected {"), "background: none;");
        // The bar and the mark follow the open Document, never the selection a
        // click on a folder's row moves (Hand test step 2): the only rules on a
        // selected row are the Organizer's and the File List's clearing GTK's
        // own fill.
        assert_eq!(sheet.matches(":selected").count(), 2, "{sheet}");
        assert!(
            rule("button.lib-sortb {").contains("background-image: none;"),
            "{sheet}"
        );
        assert!(rule("placeholder {").contains("opacity: 1;"), "{sheet}");
        assert!(!sheet.contains("border-right"), "{sheet}");
        // A row a drag hovers takes the pane's own tint, never the Default
        // theme's inset box on the one widget holding the drop (Hand test
        // step 3).
        assert_eq!(rule(":drop(active) {"), "box-shadow: none; outline: none;");
    }

    /// An Organizer row that answers a click takes the head buttons' hit step
    /// on its pill's shape while pressed, and nothing under the pointer alone
    /// (#458).
    #[test]
    fn an_organizer_row_answers_a_press_with_the_hit_step_and_takes_no_hover() {
        for (scheme, hit) in [
            (Scheme::Light, "rgba(0, 0, 0, 0.035)"),
            (Scheme::Dark, "rgba(255, 255, 255, 0.045)"),
        ] {
            let sheet = stylesheet(Ground::of(scheme));
            let pressed = format!(".lib-org row:active > .{ORG_LINE_CLASS} {{");
            assert_eq!(sheet.matches(":active").count(), 1, "{sheet}");
            let rule = sheet
                .split_once(&pressed)
                .unwrap_or_else(|| panic!("{scheme:?}: no `{pressed}` rule in\n{sheet}"))
                .1
                .split_once('}')
                .expect("an unclosed rule")
                .0;
            assert!(
                rule.contains(&format!("linear-gradient({hit}, {hit})")),
                "{scheme:?}: {rule}"
            );
            assert!(
                rule.contains(&format!("border-radius: {ORG_PILL_RADIUS}px;")),
                "{scheme:?}: {rule}"
            );
            assert!(
                !sheet.contains(".lib-org row:hover") && !sheet.contains("lib-org-line:hover"),
                "{scheme:?}: an Organizer hover rule in\n{sheet}"
            );
        }
    }

    /// Each of the four glyphs stands centred on the bar's centre line, 9.5 px
    /// in, within a pixel, and never left of the List's edge (#457).
    #[test]
    fn every_selection_mark_is_centred_on_the_bars_centre_line() {
        let centre = f64::from(BAR.left) + f64::from(BAR.width) / 2.0;
        assert!((centre - 9.5).abs() < f64::EPSILON);
        for mark in [Mark::Feather, Mark::Pen] {
            for (short, pitch) in [(false, ROW_PITCH), (true, FOLDER_PITCH)] {
                let drawn = glyph(mark, short).expect("the mark's file reads");
                let (width, left) = mark_place(drawn, pitch);
                assert!(left >= 0, "{mark:?} short={short}: {left}");
                let off = (f64::from(left) + width / 2.0 - centre).abs();
                assert!(
                    left == 0 || off <= 0.5,
                    "{mark:?} short={short}: {width} wide at {left} is {off} off centre"
                );
            }
        }
    }
}
