//! One Document per window.
//!
//! A window owns the Document it shows. That ownership is the structure the
//! rest of the file work hangs off: autosave, the reload when the file changes
//! underneath, the caret position remembered per Document and the engine's own
//! copy of the text all belong to the Document a window holds, not to a
//! `GtkTextBuffer` that has forgotten where its text came from.
//!
//! Windows belong to the application, so a file opened while Quill is running
//! joins the running instance instead of starting a second one, and closing one
//! window leaves the others alone.
//!
//! A window also opens in the shape the last session left — or the shape the
//! flags name — and takes its own shape down on the way out, which is the whole
//! of what state is for so far. Its position is not part of that: GTK4 gives a
//! client no way to ask where its window is or to put it back, so where a
//! window opens is the compositor's, on Wayland and on X11 alike.

use std::cell::Ref;
use std::io;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gdk, gio, glib};
use quill_engine::commands;
use quill_engine::disk::{Filed, Kept, Line, Noticed, OnDisk, Saved, first_save_name};
use quill_engine::document::{Document, full_name};
use quill_engine::focus::Focus;
use quill_engine::outline;
use quill_engine::settings::{
    Chrome, PreviewLayout, PreviewMode, Settings, WindowState, library_width,
};
use quill_engine::spell::Resolved;
use quill_engine::stats::Statistic;
use quill_engine::sync;

use crate::caret;
use crate::chrome;
use crate::conflict;
use crate::corrections;
use crate::files::{self, Leaving, Standing, Where};
use crate::flags;
use crate::ground::Ground;
use crate::harness;
use crate::menus;
use crate::preview::DialogOverride;
use crate::session::{Session, StyleToggle, SyntaxToggle, TemplateToggle};
use crate::tags;

/// How long after the last keystroke autosave writes the Document out.
///
/// `docs/architecture.md` § Documents and files: "after one second of idle".
const AUTOSAVE: Duration = Duration::from_secs(1);

/// How long after the last keystroke the rendered page catches up.
///
/// #263 § Implementation Decisions, "Refresh": an edit arms this, every edit
/// after it re-arms it, and the render pass runs once when it fires. Long
/// enough that a burst of typing renders nothing and short enough that a
/// writer looking up finds the page already there.
const REFRESH: Duration = Duration::from_millis(200);

/// How much one press of `preview.bigger` or `preview.smaller` moves the zoom,
/// in percentage points ([`Window::step_zoom`]).
const ZOOM_STEP: u32 = 10;

/// How often the status line is drawn again while it is saying how long ago
/// the last save was, in seconds.
///
/// Half a minute, so that "Saved · 1 min ago" is on screen within thirty
/// seconds of being true. It is the only line in the app that ages on its own.
const STATUS_TICK: u32 = 30;

/// How wide the rename dialog is, and how much air stands around the one field
/// in it ([`Window::rename_dialog`]).
///
/// Wide enough for a file name and no wider: it is a rename, not a form, and
/// it opens only where the pane that would have held the field is shut.
const DIALOG_WIDTH: i32 = 320;
/// The dialog's inset, and the air around the field in it.
const DIALOG_PAD: i32 = 12;

/// The black laid over the Editor while a Preview-driving Export dialog
/// stands in Split.
///
/// Hyprland's default modal dim is the source of this strength; moving it to
/// the Editor preserves the old visual hierarchy while leaving the pane readable.
const EXPORT_EDITOR_DIM: f64 = 0.5;

/// Whether a Document has already opened with no dictionary for Spell check
/// and said so ([`Window::say_no_dictionary`]).
static SAID_NO_DICTIONARY: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// The non-targetable black wash an Export dialog puts over the Editor.
fn editor_scrim() -> gtk::DrawingArea {
    let scrim = gtk::DrawingArea::builder()
        .hexpand(true)
        .vexpand(true)
        .visible(false)
        .can_target(false)
        .build();
    scrim.set_draw_func(|_, cr, width, height| {
        cr.set_source_rgba(0.0, 0.0, 0.0, EXPORT_EDITOR_DIM);
        cr.rectangle(0.0, 0.0, f64::from(width), f64::from(height));
        let _ = cr.fill();
    });
    scrim
}

mod imp {
    use std::cell::{Cell, OnceCell, RefCell};
    use std::path::PathBuf;
    use std::rc::Rc;

    use gtk::prelude::*;
    use gtk::subclass::prelude::*;
    use gtk::{ScrolledWindow, glib};
    use quill_engine::disk::Filed;
    use quill_engine::document::Edit;
    use quill_engine::settings::Settings;

    use super::{DialogPreviewState, Follows};
    use crate::chrome::Bars;
    use crate::chrome::typing::Typing;
    use crate::editor::Editor;
    use crate::flags;
    use crate::palette::Palette;
    use crate::preview::Preview;
    use crate::session::Session;
    use crate::sidebar::Sidebar;

    #[derive(Default)]
    pub struct Window {
        /// The one Document this window shows, and the file behind it.
        pub filed: RefCell<Filed>,
        /// Whether the buffer holds edits the file does not: what autosave
        /// has something to do about, and what tells a change on disk from a
        /// conflict ([`Filed::noticed`]).
        pub dirty: Cell<bool>,
        /// When the last edit went through the buffer, on the same monotonic
        /// clock the typing machine reads.
        pub edited: Cell<i64>,
        /// The one autosave timer, armed by the first edit of a burst.
        pub saving: RefCell<Option<glib::SourceId>>,
        /// When this window last wrote its Document out, on the same monotonic
        /// clock; `None` until it has written one, which is what the status
        /// line says "All changes saved" for rather than "Saved · just now".
        pub wrote: Cell<Option<i64>>,
        /// The timer that keeps "Saved · 2 min ago" true, armed by the first
        /// write and taken down when the window goes.
        pub ticking: RefCell<Option<glib::SourceId>>,
        /// Set once the writer has answered the prompt a close asked, so the
        /// close that follows the answer goes through instead of asking again.
        pub answered: Cell<bool>,
        /// The folder this window's untitled Document was started in — the
        /// sidebar's selected row when `file.new` ran — which its first save
        /// goes into ([`crate::files::first_save_folder`]). Taken then rather
        /// than at the save, because by then the writer may have clicked
        /// somewhere else; cleared by every Document that follows.
        pub new_in: RefCell<Option<PathBuf>>,
        /// The settings and state this window was opened from and will be
        /// remembered in.
        pub session: RefCell<Option<Rc<Session>>>,
        pub editor: Editor,
        /// What scrolls the Editor: kept because Full hides it and puts it
        /// back, and because the sync ticket reads its vertical adjustment
        /// (#270).
        pub scroller: OnceCell<ScrolledWindow>,
        /// The Editor's scroller with the Export-dialog scrim over it: the
        /// left child of [`Window::pair`], hidden whole in Preview Full.
        pub editor_frame: OnceCell<gtk::Overlay>,
        /// The app-drawn dim over the Editor while an Export dialog drives a
        /// Split pane. It takes no input.
        pub editor_scrim: OnceCell<gtk::DrawingArea>,
        /// The Editor's scroller and the Preview side by side, between the
        /// bars: what the divider divides, and what divides itself evenly
        /// until a writer has dragged it.
        pub pair: OnceCell<gtk::Box>,
        /// The rendered page beside the Editor, hidden until `preview.full`
        /// or `preview.split` opens it.
        pub preview: Preview,
        /// Whether the Preview pane is open in this window. Per window like
        /// the Library's pane and, unlike it, never remembered: the pane is
        /// closed at every launch (#263).
        pub previewing: Cell<bool>,
        /// Whether an Export dialog opened this window's pane and is holding it
        /// in Split for as long as it stands.
        ///
        /// The window's own and not the session's: the layout is one value for
        /// the app and a dialog never writes a setting, so a **Save as
        /// defaults** pressed over a pane the dialog opened composes the file
        /// with the layout the session had all along (#293 § The dialogs drive
        /// the pane).
        pub dialog_split: Cell<bool>,
        /// What **Save as defaults** wrote while an Export dialog stood over
        /// this window's pane, so the close lays the pane out at what was saved
        /// in one step.
        ///
        /// The written file reaches [`Session::running`] only when the watch
        /// reads it back, which is after the close; this is the same settings
        /// the write composed ([`Session::edit_settings`]), kept for the drop
        /// and dropped with it.
        pub dialog_saved: RefCell<Option<Settings>>,
        /// The lifecycle state that answers whether the Editor scrim is shown.
        pub(super) dialog_preview: RefCell<DialogPreviewState>,
        /// The one Export dialog standing over this window, so a second
        /// command presents it instead of opening another.
        pub export_dialog: RefCell<Option<glib::WeakRef<gtk::Window>>>,
        /// The one Preview refresh timer, armed by the first edit of a burst
        /// and re-armed by every edit after it, so the render pass runs once
        /// when the writer stops rather than once per keystroke.
        pub refresh: RefCell<Option<glib::SourceId>>,
        /// Set while one pane's scroll is being put on to the other, so the
        /// follower's own `value-changed` is read as the answer it is rather
        /// than as a writer scrolling it back.
        pub syncing: Cell<bool>,
        /// The rule that last placed the Preview, re-applied after a refresh
        /// (`Window::refollow`).
        pub follows: Cell<Follows>,
        /// Whether a furniture pass is owed on the next frame, so that the many
        /// scrolls one frame brings ask for one pass and not one each
        /// (`Window::arm_refurnish`).
        pub furnish_owed: Cell<bool>,
        /// The title bar above the Editor and the stats bar below it.
        pub bars: Bars,
        /// The Library beside the page, hidden until `library.toggle` shows
        /// it. One per window, all of them showing the session's one Library.
        pub sidebar: Sidebar,
        /// The Palette over the page, built once the window is, because a
        /// popover is parented on its window.
        pub palette: OnceCell<Palette>,
        /// How far the bars have stepped back from the last keystroke.
        pub typing: Cell<Typing>,
        /// The one timer the typing machine has armed, if one is coming.
        pub wake: RefCell<Option<glib::SourceId>>,
        /// Whether a count of the held run is already waiting on the next
        /// frame, so a drag that moves the selection a dozen times inside one
        /// frame counts it once ([`super::Window::arm_selection_count`]).
        pub selecting: Cell<bool>,
        /// Syntax's burst debounce and bounded idle-priority result wake.
        pub syntax_wake: RefCell<Option<glib::SourceId>>,
        /// Sleeps between result drains, so an empty worker cannot spin GTK.
        pub syntax_drain: RefCell<Option<glib::SourceId>>,
        /// What the edit now going through the buffer changed, left here by
        /// the handler that spliced the Document for the one that retags.
        pub pending: RefCell<Option<Edit>>,
        /// The menu or the Palette `--menu` asked for, held until the window
        /// is on the compositor: a popup opened over a toplevel that is not
        /// mapped yet keeps the toplevel from ever mapping.
        pub flagged: Cell<Option<flags::Menu>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Window {
        const NAME: &'static str = "QuillWindow";
        type Type = super::Window;
        type ParentType = gtk::ApplicationWindow;
    }

    impl ObjectImpl for Window {
        fn constructed(&self) {
            self.parent_constructed();
            let window = self.obj();
            let scroller = ScrolledWindow::builder()
                .hexpand(true)
                .vexpand(true)
                // Prose wraps, so there is nothing to scroll to sideways.
                .hscrollbar_policy(gtk::PolicyType::Never)
                .child(&self.editor)
                .build();
            // A column, the bars taking their space above and below the page
            // as the oracle's do (`chrome.css` `.chrome { flex: none }`): a
            // bar fading takes its ink away and leaves its space.
            // The Editor and the rendered page side by side, the Preview
            // right of the Editor and hidden until it is asked for, with the
            // bars spanning both: the split is inside the column and not
            // beside it, so the title bar and the stats bar are the window's
            // and not one pane's (`ref/ia/mac-native/NOTES.md` § State 16).
            let pair = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            let editor_frame = gtk::Overlay::new();
            editor_frame.set_child(Some(&scroller));
            let scrim = super::editor_scrim();
            editor_frame.add_overlay(&scrim);
            pair.append(&editor_frame);
            pair.append(self.preview.widget());
            let column = gtk::Box::new(gtk::Orientation::Vertical, 0);
            column.append(self.bars.top());
            column.append(&pair);
            column.append(self.bars.bottom());
            self.bars.follow(&scroller);
            let _ = self.scroller.set(scroller.clone());
            let _ = self.editor_frame.set(editor_frame);
            let _ = self.editor_scrim.set(scrim);
            let _ = self.pair.set(pair);
            // The Library stands left of that column and pushes it right
            // rather than covering it, which is the oracle's model: the page
            // keeps its own centring and is given a narrower window
            // (`files.css` `#app { padding-left: var(--lib-w) }`).
            let beside = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            beside.append(self.sidebar.widget());
            beside.append(&column);
            window.set_child(Some(&beside));
            let _ = self.palette.set(Palette::new(&beside));
        }

        fn dispose(&self) {
            self.obj().cancel_syntax();
            self.bars.dispose();
            self.sidebar.dispose();
            if let Some(palette) = self.palette.get() {
                palette.dispose();
            }
        }
    }

    impl WidgetImpl for Window {}
    impl WindowImpl for Window {}
    impl ApplicationWindowImpl for Window {}
}

/// Which rule last placed the Preview's scroll.
///
/// A refresh lays the page out again and has to put the pane back
/// (`Window::refollow`); which of the two sync rules puts it back is whichever
/// one last moved it, because that is the move the pane is standing where it
/// is for.
#[derive(Clone, Copy, Default, Eq, PartialEq)]
pub enum Follows {
    /// The caret rule (`Window::follow_caret`): an edit or a caret move put
    /// the caret's block where the eye already was.
    #[default]
    Caret,
    /// The top-block rule (`Window::follow_editor`): a wheel or a scrollbar
    /// over either pane left the two in step.
    TopBlock,
}

glib::wrapper! {
    /// A window, and the Document it shows.
    pub struct Window(ObjectSubclass<imp::Window>)
        @extends gtk::ApplicationWindow, gtk::Window, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Native,
                    gtk::Root, gtk::ShortcutManager, gio::ActionGroup, gio::ActionMap;
}

impl Window {
    /// A window of `app` showing `filed`, in the shape `session` remembers.
    fn new(app: &gtk::Application, filed: Filed, session: &Rc<Session>) -> Self {
        let window: Self = glib::Object::builder().property("application", app).build();
        window.imp().session.replace(Some(Rc::clone(session)));
        window.open_at(session.opening());
        if session.flags().deterministic {
            // No client-side decorations, which the Gate asks for and a judged
            // shot needs twice over: the window is exactly the size `--w` and
            // `--h` name, and nothing of the desktop's title bar is in the
            // frame to be compared against the opponent's.
            window.set_decorated(false);
        }
        // Before the first Document is shown, so that there is no window whose
        // buffer can be typed into without the engine hearing about it.
        window.watch_edits();
        window.watch_escape();
        // And before the type, since setting the type places the bar: a
        // machine in the wrong mode would have blinked once before the flags
        // that said not to were read.
        window
            .imp()
            .editor
            .set_mode(caret::Mode::from_flags(session.flags()));
        // And before the Document, because showing one draws every tag it
        // carries and a tag is drawn in a colour: the ground has to be settled
        // before the first thing painted on it.
        let ground = session.ground();
        window.imp().editor.open_on(ground);
        window.imp().bars.set_ground(ground);
        // And with it, because `--focus` names a state the first frame is
        // meant to show: the tiers are worked out inside the same draw that
        // puts the Document on the page.
        window.imp().editor.open_focused_on(session.focus());
        window.imp().bars.set_focus(session.focus());
        // And Live with them, for the same reason: `--live` names a state the
        // first frame is meant to show, and the fold is worked out inside the
        // same draw that puts the Document on the page.
        window.imp().editor.open_live_on(session.live());
        // Install the tables while the buffer is empty; showing the Document
        // below resets the worker and schedules its first viewport request.
        // Both Annotators, because either one alone is work to schedule.
        window.set_syntax(session.syntax().clone());
        window.set_style(session.style().clone());
        let (spell, language) = spelling(session);
        window.set_spell(spell, &language);
        // The bars stand or not before the Document is shown, so the page is
        // laid out once, at the height it will keep.
        window
            .imp()
            .bars
            .set_shown(session.chrome() == Chrome::Shown);
        window.imp().bars.set_stats_shown(session.stats_shown());
        window
            .imp()
            .bars
            .set_statistics(chrome::checked_set(&session.stats().show));
        window
            .imp()
            .bars
            .set_menus_grabbing(!session.flags().deterministic);
        window
            .palette()
            .set_grabbing(!session.flags().deterministic);
        // And Typewriter with them, so that `--typewriter`'s first frame holds
        // the caret's row at the anchor rather than travelling to it.
        window.imp().editor.set_typewriter(session.typewriter());
        // The Library beside the page before the Document is shown, for the
        // reason the bars are: the page is laid out once, at the width it will
        // keep, rather than reflowing under the first frame.
        window.show_library(session.flags().sidebar);
        window.imp().sidebar.attach(&window);
        // The Preview pane with it, and for the same reason: the Editor is
        // laid out once, at the width it will keep. The pane is open only
        // where `--preview` asked for it — nothing else opens one, because
        // nothing remembers one (#263).
        window.imp().preview.attach(&window);
        window.watch_sync();
        window.watch_furniture();
        window.show_preview(session.flags().preview.is_some());
        if let Some(query) = session.flags().search.as_deref() {
            window.imp().sidebar.set_query(query);
        }
        window.set_filed(filed);
        // After the Document, whose showing counts it, and before the first
        // frame: `--typing` is the chrome inside the window after a
        // keystroke, and a shot of it is the first frame.
        window
            .imp()
            .typing
            .set(chrome::typing::Typing::from_flags(session.flags()));
        window.settle();
        window.watch_pointer();
        window
            .imp()
            .editor
            .set_type(session.settings().face, session.step());
        chrome::install_window(&window);
        window.imp().editor.grab_focus();
        window.watch_active();
        window.watch_corrections();
        // A window is remembered as it closes rather than at shutdown, so that
        // the last window a writer sized is the first one the next launch
        // reads, whichever of its windows they closed first. What its Document
        // has to say about closing is [`Window::closing`]'s.
        window.connect_close_request(Self::closing);
        window
    }

    /// Opens in the shape a session left.
    fn open_at(&self, shape: &WindowState) {
        self.set_default_size(
            i32::try_from(shape.width).unwrap_or(i32::MAX),
            i32::try_from(shape.height).unwrap_or(i32::MAX),
        );
        if shape.maximized {
            self.maximize();
        }
        if shape.fullscreen {
            self.fullscreen();
        }
    }

    /// The shape this window is in now.
    ///
    /// `default_size` rather than the allocation: it is the size a window would
    /// go back to from maximized or full screen, which is the one worth
    /// remembering, and GTK keeps it up to date as the writer drags an edge.
    ///
    /// It starts from the shape the window was opened in rather than from
    /// nothing, so that a state file written by a newer Quill keeps the keys
    /// this one does not know: the window is that remembered window, opened
    /// again, and what was said about it is still said about it.
    fn shape(&self, opened_in: &WindowState) -> WindowState {
        let (width, height) = self.default_size();
        let mut shape = opened_in.clone();
        shape.width = u32::try_from(width).unwrap_or(shape.width);
        shape.height = u32::try_from(height).unwrap_or(shape.height);
        shape.maximized = self.is_maximized();
        shape.fullscreen = self.is_fullscreen();
        shape
    }

    /// Takes this window's shape — and its caret — down for the next launch.
    fn remember(&self) {
        self.leave_caret();
        // Set the moment the window is built, so this is every window; the
        // `Option` is there because a `GObject` is constructed before anyone
        // can hand it anything.
        if let Some(session) = self.imp().session.borrow().as_ref() {
            session.remember(self.shape(session.opening()));
        }
    }

    /// What the session shows in this window, as the Commands' stateful
    /// actions show it.
    pub(crate) fn modes(&self) -> chrome::Modes {
        match self.imp().session.borrow().as_ref() {
            Some(session) => chrome::Modes::of(
                session,
                self.is_fullscreen(),
                self.imp().sidebar.is_shown(),
                self.imp().previewing.get(),
            ),
            None => chrome::Modes::default(),
        }
    }

    /// Steps the type size one rung of the ladder, or back to the default
    /// one.
    ///
    /// The step stops at the ends of [`quill_engine::settings::type_steps`]
    /// rather than wrapping or refusing: a writer holding the key down means
    /// "as big as it goes", and it is the same range `step` in the file and
    /// `--step` on the command line are held to, because it is the same
    /// question asked three ways.
    pub(crate) fn step_size(&self, direction: Step) {
        let Some(session) = self.session() else {
            return;
        };
        let ladder = quill_engine::settings::type_steps();
        let wanted = match direction {
            Step::Bigger => session.step().saturating_add(1),
            Step::Smaller => session.step().saturating_sub(1),
            Step::Default => quill_engine::settings::default_step(),
        };
        let step = wanted.clamp(*ladder.start(), *ladder.end());
        if step == session.step() {
            return;
        }
        session.set_step(step);
        // Written as the key is pressed, as every value the settings watch
        // can read back is: a live value the file does not carry is one the
        // next save — a writer's, or the watch re-reading Quill's own — puts
        // back to what the file says.
        session.store_settings();
        let face = session.face();
        if let Some(app) = self.application() {
            reset(&app, &session, |window, _| {
                window.imp().editor.set_type(face, step);
            });
        }
    }

    /// Toggles the ground, for this launch and for the next.
    ///
    /// `docs/shortcuts.md`'s `theme.toggle` row, reached through its action
    /// (`chrome`). The session decides which ground the toggle lands on — it
    /// is the one holding what `auto` resolved to — and writes the setting as
    /// the key is pressed.
    pub(crate) fn toggle_scheme(&self) {
        let Some(session) = self.session() else {
            return;
        };
        session.toggle_scheme();
        // Written now rather than on the way out, for the reason the size is
        // ([`Window::step_size`]).
        session.store_settings();
        if let Some(app) = self.application() {
            repaint(&app, &session);
        }
    }

    /// Switches Focus off, or back on at the scope it left.
    ///
    /// `docs/shortcuts.md`'s `focus.toggle` row, reached through its action
    /// (`chrome`). The session holds which scope that is and writes it on the
    /// way out, as it does the ground.
    pub(crate) fn toggle_focus(&self) {
        self.refocus_windows(|session| session.toggle_focus());
    }

    /// Swaps Sentence and Paragraph, switching Focus on if it was off.
    ///
    /// `docs/shortcuts.md`'s `focus.swap` row.
    pub(crate) fn swap_focus_scope(&self) {
        self.refocus_windows(Session::swap_focus_scope);
    }

    /// Puts Focus on at `scope`.
    ///
    /// `docs/shortcuts.md`'s `focus.sentence` and `focus.paragraph` rows, the
    /// View menu's Focus radios. The session applies ADR 0006's rule that a
    /// pick switches Focus on.
    pub(crate) fn set_focus_scope(&self, scope: quill_engine::settings::FocusScope) {
        self.refocus_windows(move |session| session.set_focus_scope(scope));
    }

    /// Sets the page in `face`, in every window.
    ///
    /// `docs/shortcuts.md`'s `font.duo`, `font.quattro` and `font.mono` rows,
    /// the View menu's Typeface radios. The same pass a size step is: the
    /// session remembers the face and writes it on the way out, and every
    /// Editor is re-set at the step this launch is reading at.
    pub(crate) fn set_face(&self, face: quill_engine::settings::Face) {
        let Some(session) = self.session() else {
            return;
        };
        if face == session.face() {
            return;
        }
        session.set_face(face);
        session.store_settings();
        let step = session.step();
        if let Some(app) = self.application() {
            reset(&app, &session, |window, _| {
                window.imp().editor.set_type(face, step);
            });
        }
    }

    /// Lays the rendered page out in `name`, in every window.
    ///
    /// `docs/shortcuts.md`'s five `template.*` radio rows, the View › Template
    /// submenu. One Template for the app (ADR 0005), so the session remembers
    /// it, writes it as the row is picked — the way a face is written — and
    /// every open pane lays its Document out again.
    pub(crate) fn set_template(&self, name: quill_engine::settings::TemplateName) {
        let Some(session) = self.session() else {
            return;
        };
        if name == session.template().name {
            return;
        }
        session.set_template(name);
        self.relay_out(&session);
    }

    /// Flips one of the Template's three toggles, in every window.
    ///
    /// `docs/shortcuts.md`'s `template.centerHeadings`,
    /// `template.numberHeadings` and `template.indentParagraphs` rows, and the
    /// same three the Settings window's switches write: one Template for the
    /// app, so a toggle moves every pane.
    pub(crate) fn toggle_template(&self, toggle: TemplateToggle) {
        let Some(session) = self.session() else {
            return;
        };
        session.toggle_template(toggle);
        self.relay_out(&session);
    }

    /// Writes what the Template picks moved and lays every open pane out
    /// again.
    ///
    /// The Editor is untouched: a Template is the rendered page's and the
    /// `font.*` ladder is the Editor's, and neither reaches the other
    /// (#263 § Zoom).
    fn relay_out(&self, session: &Session) {
        // Written as the row is picked, for the reason a size step is
        // ([`Window::step_size`]).
        session.store_settings();
        let Some(app) = self.application() else {
            return;
        };
        for window in windows(&app) {
            window.refresh_preview();
        }
    }

    /// Sets the theme, and repaints on the ground it names.
    ///
    /// `docs/shortcuts.md`'s `theme.light`, `theme.dark` and `theme.auto`
    /// rows, the Palette's three. The session decides the ground — `auto` is
    /// the desktop's last answer — and writes the setting on the way out.
    pub(crate) fn set_theme(&self, theme: quill_engine::settings::Theme) {
        let Some(session) = self.session() else {
            return;
        };
        session.set_theme(theme);
        session.store_settings();
        if let Some(app) = self.application() {
            repaint(&app, &session);
        }
    }

    /// Turns Typewriter on or off.
    ///
    /// `docs/shortcuts.md`'s `typewriter.toggle` row. The session
    /// remembers the value, and every window's Editor is told, for the reason
    /// [`Window::refocus_windows`] tells them all: on brings the caret's row
    /// to the anchor, off leaves the view where it is.
    pub(crate) fn toggle_typewriter(&self) {
        self.move_windows(Session::toggle_typewriter, |window, typewriter| {
            window.imp().editor.set_typewriter(typewriter);
        });
    }

    /// Turns Live on or off, in every window.
    ///
    /// `docs/shortcuts.md`'s `live.toggle` row. A per-app mode as Focus is:
    /// the session remembers it and writes it as the key is pressed, and every
    /// window folds or unfolds together, because a writer has one pair of eyes
    /// ([`Window::move_windows`]).
    pub(crate) fn toggle_live(&self) {
        self.move_windows(Session::toggle_live, |window, live| {
            let document = window.document();
            window.imp().editor.set_live(live, &document);
        });
    }

    /// Moves one Syntax highlight key and applies the full table to every Editor.
    pub(crate) fn toggle_syntax(&self, toggle: SyntaxToggle) {
        self.move_windows(
            |session| session.toggle_syntax(toggle),
            |window, _| {
                if let Some(session) = window.session() {
                    window.set_syntax(session.syntax().clone());
                }
            },
        );
    }

    /// Moves one Style check key and applies the full table to every Editor.
    ///
    /// The master and the three Lists take the same path, because a List is a
    /// repaint of spans already held and the master is a request to send or
    /// not: [`Window::set_style`] tells the two apart ([`crate::syntax`]).
    pub(crate) fn toggle_style(&self, toggle: StyleToggle) {
        self.move_windows(
            |session| session.toggle_style(toggle),
            |window, _| {
                if let Some(session) = window.session() {
                    window.set_style(session.style().clone());
                }
            },
        );
    }

    /// Turns Spell check on or off, in every window.
    ///
    /// Off drops every wave at once and sends no Spell request; on asks for
    /// the Document again ([`Window::set_spell`]).
    pub(crate) fn toggle_spell(&self) {
        self.move_windows(Session::toggle_spell, |window, _| {
            if let Some(session) = window.session() {
                let (spell, language) = spelling(&session);
                window.set_spell(spell, &language);
            }
        });
    }

    /// Moves Focus the way `move_it` says, and puts the answer on every window.
    ///
    /// The two Focus keys differ only in what they ask the session for, so what
    /// they do with the answer is written once.
    fn refocus_windows(&self, move_it: impl Fn(&Session) -> Focus) {
        self.move_windows(move_it, |window, focus| {
            let document = window.document();
            window.imp().editor.set_focus(focus, &document);
            window.imp().bars.set_focus(focus);
        });
    }

    /// Moves a mode the way `move_it` says, stores it, and puts the answer on
    /// every window with `apply`.
    ///
    /// Written once for the three mode keys: a writer has one pair of eyes,
    /// and a mode moving in one window moves it in all of them, as the ground
    /// and the type size do. The settings are written as the key is pressed
    /// rather than only on the way out, so a session that never gets to shut
    /// down cleanly still leaves the writer reading the way they chose to
    /// read.
    fn move_windows<T: Copy>(&self, move_it: impl Fn(&Session) -> T, apply: impl Fn(&Window, T)) {
        let Some(session) = self.session() else {
            return;
        };
        let moved = move_it(&session);
        session.store_settings();
        let Some(app) = self.application() else {
            return;
        };
        for window in app.windows() {
            let Ok(window) = window.downcast::<Window>() else {
                continue;
            };
            apply(&window, moved);
        }
    }

    /// The Document this window shows.
    ///
    /// A [`Ref`] of the Document inside the [`Filed`], so that every reader of
    /// the text goes on reading it the way it always did and only the file's
    /// half of the state is new.
    pub(crate) fn document(&self) -> Ref<'_, Document> {
        Ref::map(self.imp().filed.borrow(), Filed::document)
    }

    /// The file this window's Document is, or `None` while it is untitled.
    pub(crate) fn path(&self) -> Option<PathBuf> {
        self.imp().filed.borrow().path().map(Path::to_path_buf)
    }

    /// Takes `filed` as this window's own and shows it.
    ///
    /// Everything a new Document brings with it: the buffer, the two titles,
    /// the count, a clean slate for autosave, the watch on its file and its
    /// place at the front of the recents.
    fn set_filed(&self, filed: Filed) {
        // Where the caret stands in the Document being left, before it is
        // gone: what opening that Document again comes back to.
        self.leave_caret();
        self.imp().filed.replace(filed);
        self.imp().dirty.set(false);
        // Where the last `file.new` was going is nothing to this Document.
        self.imp().new_in.replace(None);
        // A Document this window has not written yet, whatever it wrote of the
        // one before it.
        self.imp().wrote.set(None);
        self.shown();
        // The sidebar's highlight follows the Document this window shows, so
        // that the row a writer opened is the row they can see they are in.
        self.imp().sidebar.set_open(self.path().as_deref());
        self.show_standing();
        self.say_no_dictionary();
        let (Some(path), Some(session)) = (self.path(), self.session()) else {
            return;
        };
        session.watch_document(&path);
        session.opened_at(&path);
        // Back to the sentence the writer left this Document in (#246, story
        // 43), revealed rather than merely placed, since the block it is in
        // may be a screen down.
        if let Some(offset) = session.caret(&path) {
            self.imp()
                .editor
                .place_caret(&self.document(), flags::Caret::At(offset), true);
        }
    }

    /// Takes down where the caret stands in the Document this window is
    /// showing, for the state file
    /// ([`crate::session::Session::left_caret`]).
    ///
    /// Called as a Document is left rather than as it is edited: the caret
    /// moves on every arrow key and the state is written once, on the way out.
    fn leave_caret(&self) {
        let (Some(path), Some(session)) = (self.path(), self.session()) else {
            return;
        };
        session.left_caret(
            &path,
            u64::try_from(self.caret_offset()).unwrap_or_default(),
        );
    }

    /// Puts the Document this window holds on to the page and the bars.
    ///
    /// Split from [`Window::set_filed`] because a reload shows the same
    /// Document again rather than taking a new one.
    fn shown(&self) {
        self.cancel_syntax();
        let document = self.document();
        self.show_title(&document);
        // A Document arriving carries no selection with it, and the `mark-set`
        // the fill fires is skipped while the Editor is loading, so the bar is
        // told here rather than left reading the last Document's run.
        self.imp().bars.set_selection(None);
        self.imp().bars.set_count(document.text());
        self.imp().editor.show_document(&document);
        drop(document);
        self.arm_syntax();
    }

    /// Applies the session's Syntax table; category-only changes reuse spans.
    pub(crate) fn set_syntax(&self, settings: quill_engine::settings::SyntaxHighlight) {
        self.imp().editor.set_syntax(settings, &self.document());
        self.rearm();
    }

    /// Applies the session's Style check table; a List alone reuses spans.
    pub(crate) fn set_style(&self, style: quill_engine::settings::StyleCheck) {
        self.imp().editor.set_style(style, &self.document());
        self.rearm();
    }

    /// Applies the `spell_check` and `spell_language` settings; a language
    /// that did not move is not resolved again.
    pub(crate) fn set_spell(&self, on: bool, language: &str) {
        self.imp().editor.set_spell(on, language, &self.document());
        self.rearm();
    }

    /// Starts or stops the one wake both Annotators share.
    ///
    /// Armed whenever either is on, and not only as the first arrives: a
    /// master joining the other has paragraphs to match and no keystroke
    /// coming to ask for them. A table change that dirties nothing — a
    /// Category or a List — is a repaint the Editor has already done, and
    /// arming for it would push a pending keystroke's re-match back by
    /// another debounce, so the wake follows the work rather than the table.
    fn rearm(&self) {
        if !self.imp().editor.annotating() {
            self.cancel_syntax();
        } else if self.imp().editor.asking() {
            self.arm_syntax();
        }
    }

    fn cancel_syntax(&self) {
        if let Some(source) = self.imp().syntax_wake.take() {
            source.remove();
        }
        if let Some(source) = self.imp().syntax_drain.take() {
            source.remove();
        }
    }

    fn arm_syntax(&self) {
        if !self.imp().editor.annotating() {
            return;
        }
        if let Some(source) = self.imp().syntax_wake.take() {
            source.remove();
        }
        let weak = self.downgrade();
        self.imp()
            .syntax_wake
            .replace(Some(glib::timeout_add_local_once(
                quill_engine::worker::DEBOUNCE,
                move || {
                    let Some(window) = weak.upgrade() else {
                        return;
                    };
                    window.imp().syntax_wake.take();
                    window.imp().editor.submit_syntax(&window.document());
                    window.arm_syntax_drain();
                },
            )));
    }

    fn arm_syntax_drain(&self) {
        if self.imp().syntax_drain.borrow().is_some() {
            return;
        }
        let weak = self.downgrade();
        // A timed source at idle priority: at most eight paragraphs per wake,
        // never a continuously-ready idle source while the model is loading.
        self.imp()
            .syntax_drain
            .replace(Some(glib::timeout_add_local_full(
                Duration::from_millis(16),
                glib::Priority::DEFAULT_IDLE,
                move || {
                    let Some(window) = weak.upgrade() else {
                        return glib::ControlFlow::Break;
                    };
                    if window.imp().editor.drain_syntax(&window.document()) {
                        glib::ControlFlow::Continue
                    } else {
                        window.imp().syntax_drain.take();
                        glib::ControlFlow::Break
                    }
                },
            )));
    }

    /// Puts the Document's name on the window and on the top bar.
    ///
    /// The name without its extension, which is what the top bar shows (#246,
    /// story 48) and what a `.md` writer reads as the title. Its own step,
    /// because a save that derived a name moves the titles and nothing else
    /// the page is showing.
    fn show_title(&self, document: &Document) {
        self.set_title(Some(&document.name()));
        self.imp().bars.set_title(&document.name());
    }

    /// The session this window was opened from.
    pub(crate) fn session(&self) -> Option<Rc<Session>> {
        self.imp().session.borrow().clone()
    }

    // ------------------------------------------------- the file on disk

    /// A keystroke went through the buffer: the autosave clock restarts.
    ///
    /// On the keystroke path, so it does the least the machine allows — two
    /// `Cell`s and, once per burst of typing, one timer. The timer is not
    /// taken down and put back on every key: when it fires it asks how long
    /// ago the last key was and arms itself for the rest of the second where
    /// the writer is still typing, which is the shape [`Window::settle`] uses
    /// for the bars. Nothing here touches the disk.
    fn edited(&self) {
        self.imp().dirty.set(true);
        self.imp().edited.set(Self::now());
        if self.imp().saving.borrow().is_some() {
            return;
        }
        self.arm_autosave(AUTOSAVE);
    }

    /// Arms the autosave timer for `left` from now.
    fn arm_autosave(&self, left: Duration) {
        let id = glib::timeout_add_local_once(
            left,
            glib::clone!(
                #[weak(rename_to = window)]
                self,
                move || {
                    window.imp().saving.take();
                    let since = Self::now() - window.imp().edited.get();
                    let waited = Duration::from_micros(u64::try_from(since).unwrap_or(0));
                    match AUTOSAVE.checked_sub(waited) {
                        Some(left) if !left.is_zero() => window.arm_autosave(left),
                        _ => window.autosave(),
                    }
                },
            ),
        );
        self.imp().saving.replace(Some(id));
    }

    /// Writes the Document out wherever that can be done without asking.
    ///
    /// The four places autosave runs from — a second of idle, the window
    /// losing the keyboard, a Library action, and closing or quitting
    /// (`docs/architecture.md` § Documents and files) — all arrive here.
    /// Silent by construction: a Document with no file and nowhere to put it
    /// is left for the close prompt to ask about, because a timer a second
    /// after a keystroke is not a question the writer asked.
    fn autosave(&self) {
        self.flush();
    }

    /// Saves what can be saved without asking, and answers whether the file
    /// now holds what the writer typed.
    ///
    /// False for the two Documents nobody but the writer can settle: an
    /// untitled one with text and no Location to put it in, and one whose file
    /// changed underneath it.
    fn flush(&self) -> bool {
        if !self.imp().filed.borrow().autosaves() {
            return false;
        }
        if !self.imp().dirty.get() {
            return true;
        }
        if self.path().is_some() {
            return self.write(None);
        }
        if self.document().text().is_empty() {
            // Nothing typed: there is nothing a file would hold.
            return true;
        }
        match self.first_save_folder() {
            Where::Folder(folder) => self.write(Some(&folder)),
            Where::Ask => false,
        }
    }

    /// Autosave's third moment: a Library action, which writes the Document
    /// out before the action moves the folder under it.
    ///
    /// Story 20's "on any Library action" (`docs/architecture.md` § Documents
    /// and files): every row operation, every open and every Location added
    /// goes through here first. The answer is [`Window::flush`]'s — whether
    /// the file holds what the writer typed — which [`Window::open_path`] and
    /// [`Window::new_document`] read to decide whether the Document being
    /// replaced can be replaced at all; a rename or a pin does not step on the
    /// window's text and ignores it.
    fn library_action(&self) -> bool {
        self.flush()
    }

    /// Whether this window may put its Document on disk at all.
    ///
    /// A launch of the harness's may not: a bench types into `ref/sample.md`,
    /// and the passage every Piece is judged on has to be the same bytes when
    /// it has finished. The same rule the settings file is defended by
    /// ([`Session::is_harness`]), for the same reason.
    fn writes(&self) -> bool {
        !self.session().is_some_and(|session| session.is_harness())
    }

    /// Where this window's first save goes ([`crate::files::first_save_folder`]).
    fn first_save_folder(&self) -> Where {
        let Some(session) = self.session() else {
            return Where::Ask;
        };
        files::first_save_folder(
            self.imp().new_in.borrow().as_deref(),
            session.first_location().as_deref(),
            session.always_asks(),
        )
    }

    /// Writes the Document — into `folder` under a derived name where it has
    /// no file yet, over its own file where it has one — and takes in
    /// everything the write moved.
    ///
    /// Answers whether the file holds the Editor's text now. A write that
    /// cannot happen is one line on stderr, as every other file here is.
    fn write(&self, folder: Option<&Path>) -> bool {
        if !self.writes() {
            return false;
        }
        self.imp()
            .sidebar
            .set_status(&files::said(Standing::Saving));
        let written = match folder {
            Some(folder) => self.imp().filed.borrow_mut().save_in(folder),
            None => self.imp().filed.borrow_mut().save(),
        };
        self.written(written)
    }

    /// Writes the Editor's text over the file whatever the file now holds:
    /// the Keep half of a conflict, and what recreates a deleted file.
    ///
    /// The one write allowed to stand on someone else's change, because it
    /// happens only where the writer has looked at what it would do
    /// ([`crate::conflict`]) and said yes.
    fn keep_over_the_file(&self) -> bool {
        if !self.writes() {
            return false;
        }
        let written = self.imp().filed.borrow_mut().keep();
        self.written(written)
    }

    /// Takes in what a write answered, and says whether the file holds the
    /// Editor's text now.
    ///
    /// A write that cannot happen is one line on stderr, as every other file
    /// error here is.
    fn written(&self, written: io::Result<Saved>) -> bool {
        match written {
            Ok(Saved::Written) => {
                self.write_landed();
                true
            }
            Ok(Saved::NeedsAFolder | Saved::Refused) => false,
            Err(err) => {
                match self.path() {
                    Some(path) => eprintln!("quill: cannot save {}: {err}", path.display()),
                    None => eprintln!("quill: cannot save: {err}"),
                }
                false
            }
        }
    }

    /// Takes in a write that happened: the file is what the window holds, so
    /// the titles follow the name a first save derived, the file joins the
    /// watch and the recents, and autosave has nothing left to do.
    fn write_landed(&self) {
        self.imp().dirty.set(false);
        self.imp().wrote.set(Some(Self::now()));
        self.show_standing();
        self.tick_standing();
        self.show_title(&self.document());
        let (Some(path), Some(session)) = (self.path(), self.session()) else {
            return;
        };
        session.watch_document(&path);
        // The recents and not the Locations: story 45 gives a Location to a
        // file the writer opened, and a Save As into a folder they picked once
        // is not a folder they asked Quill to watch (#246).
        session.wrote_at(&path);
    }

    /// `file.save`: what the writer asked for, which may be a dialog.
    ///
    /// The one save that is allowed to ask: an untitled Document with nowhere
    /// to go, or a writer who asked to always be asked, gets the Save As
    /// dialog rather than nothing happening.
    pub(crate) fn save(&self) {
        let state = self.imp().filed.borrow().state();
        match state {
            // A file that is gone comes back: `Filed::save` recreates it.
            OnDisk::Named | OnDisk::DeletedOnDisk => {
                self.write(None);
            }
            OnDisk::Untitled => match self.first_save_folder() {
                Where::Folder(folder) => {
                    self.write(Some(&folder));
                }
                Where::Ask => self.save_as(After::Stay),
            },
            // Save over a file that moved under the Document is Keep, and
            // Keep is not done blind: the writer sees what it would do first.
            OnDisk::ChangedOnDisk => self.keep_over_disk(),
        }
    }

    /// Gives an untitled Document a file so that `export.quick` has something
    /// to stand a PDF beside, and exports once it has one.
    ///
    /// [`Window::save`]'s own decision, so that a Quick Export asks exactly
    /// what a save would ask: the first-save folder where the Library names
    /// one, and the Save As dialog where it does not. A save that does not
    /// happen — a cancelled dialog, a write that failed — exports nothing.
    pub(crate) fn save_before_export(&self) {
        match self.first_save_folder() {
            Where::Folder(folder) => {
                if self.write(Some(&folder)) {
                    crate::export::quick(self);
                }
            }
            Where::Ask => self.save_as(After::Export),
        }
    }

    /// Puts `words` on the status line at the foot of the Library over
    /// whatever it now says.
    ///
    /// A notice rather than a state: the next [`Window::show_standing`] takes
    /// it back down, which is what the trash notice is and what a file
    /// export's confirmation is ([`crate::export::confirm`]).
    pub(crate) fn notice(&self, words: &str) {
        self.imp().sidebar.set_status(words);
    }

    /// Says, once per process, that Spell check has no dictionary for the
    /// language it wants: the first Document to open in that state carries
    /// the Settings window's line as a notice, and no Document after it does.
    ///
    /// Process-wide rather than per window, because the notice is a window's
    /// and a writer opening a second file has already been told (#401 § The
    /// "no dictionary" state).
    fn say_no_dictionary(&self) {
        let Some(Resolved::Missing { wanted }) = self.imp().editor.spell_resolution() else {
            return;
        };
        if !SAID_NO_DICTIONARY.swap(true, std::sync::atomic::Ordering::Relaxed) {
            self.notice(&crate::settings::no_dictionary(&wanted));
        }
    }

    /// `file.saveAs`: the writer names the file, and the Document is that file
    /// from then on.
    ///
    /// `after` is what to do once it is written, which is how the Save button
    /// of a close prompt gets its window closed only when the save it asked
    /// for actually happened.
    pub(crate) fn save_as(&self, after: After) {
        let dialog = gtk::FileDialog::new();
        dialog.set_title("Save As");
        let document = self.document();
        dialog.set_initial_name(Some(
            &document
                .path()
                .and_then(|path| path.file_name())
                .map_or_else(
                    || first_save_name(document.text()),
                    |name| name.to_string_lossy().into_owned(),
                ),
        ));
        drop(document);
        if let Some(folder) = self.save_folder() {
            dialog.set_initial_folder(Some(&gio::File::for_path(folder)));
        }
        dialog.save(
            Some(self),
            None::<&gio::Cancellable>,
            glib::clone!(
                #[weak(rename_to = window)]
                self,
                move |answer| {
                    let Some(path) = answer.ok().and_then(|file| file.path()) else {
                        // Cancelled, or a place with no local path: the
                        // Document stays where it was, and a window that was
                        // waiting on the save stays open.
                        return;
                    };
                    if !window.write_as(&path) {
                        return;
                    }
                    match after {
                        After::Stay => {}
                        After::Close => window.leave(),
                        After::Export => crate::export::quick(&window),
                    }
                },
            ),
        );
    }

    /// The folder a dialog opens in: the Document's own, else the Library's
    /// first Location.
    fn save_folder(&self) -> Option<PathBuf> {
        match self.path() {
            Some(path) => path.parent().map(Path::to_path_buf),
            None => self.session()?.first_location(),
        }
    }

    /// Makes `path` this Document's file and writes it there.
    ///
    /// `moved_to` then `keep`, which is what Save As is: the Document follows
    /// the path the writer named, and its text is written over whatever is
    /// there — including nothing, which is the ordinary case.
    fn write_as(&self, path: &Path) -> bool {
        if !self.writes() {
            return false;
        }
        self.imp().filed.borrow_mut().moved_to(path);
        let kept = self.imp().filed.borrow_mut().keep();
        self.written(kept)
    }

    /// `file.open`: the writer picks a file and this window shows it.
    ///
    /// The Document it was showing is written out first, because opening
    /// another Document is a Library action and a Library action flushes
    /// autosave. Where that Document cannot be settled without asking — an
    /// untitled one with text, a conflicted one — the file opens in a window
    /// of its own instead, so nothing the writer typed is stepped on.
    pub(crate) fn open_file(&self) {
        let dialog = gtk::FileDialog::new();
        dialog.set_title("Open File");
        if let Some(folder) = self.save_folder() {
            dialog.set_initial_folder(Some(&gio::File::for_path(folder)));
        }
        dialog.open(
            Some(self),
            None::<&gio::Cancellable>,
            glib::clone!(
                #[weak(rename_to = window)]
                self,
                move |answer| {
                    let Some(path) = answer.ok().and_then(|file| file.path()) else {
                        return;
                    };
                    window.open_path(&path);
                },
            ),
        );
    }

    /// Shows the Document at `path` in this window, in place of the one it
    /// holds.
    ///
    /// Every way of opening a file arrives here — a row clicked, Enter on the
    /// highlighted row, a search hit, `file.open`, Open Recent, `file.next`
    /// and `file.prev` — and each of them opens in this window (#246, the File
    /// handling spec's "Single click opens in this window" and "Enter opening
    /// in this window"). `window.new` is the one Command that opens a window.
    pub(crate) fn open_path(&self, path: &Path) {
        let filed = match Filed::open(path) {
            Ok(filed) => filed,
            Err(err) => {
                eprintln!("quill: cannot open {}: {err}", path.display());
                return;
            }
        };
        self.replace_with(filed, |_| {});
    }

    // ------------------------------------------------- the Library's rows

    /// `file.new`: an empty Document in this window, to be written into the
    /// folder the Library's selected row stands in.
    ///
    /// Where the Document this window holds cannot be settled without asking —
    /// an untitled one with text, a conflicted one — the writer is asked first,
    /// as [`Window::open_path`] does, so nothing they typed is stepped on.
    ///
    /// The keyboard goes to the page: `Ctrl+N` is most often pressed while the
    /// writer is picking the folder in the Library, and a blank page whose
    /// first keystroke lands in the search field is a Document they have to
    /// click into before they can write it.
    pub(crate) fn new_document(&self) {
        let folder = self.imp().sidebar.selected_folder();
        self.replace_with(Filed::untitled(), move |window| {
            // After the Document is in place, because taking one in clears
            // where the last `file.new` was going ([`Window::set_filed`]).
            window.imp().new_in.replace(folder.clone());
            window.focus_editor();
        });
    }

    /// Puts `filed` in this window in place of the Document it holds, and runs
    /// `then` once it is there.
    ///
    /// The one way a Document is replaced, which is what keeps opening one
    /// from opening a window: `window.new` is the only Command that opens a
    /// window (#246). The Document being replaced is written out first
    /// wherever that can be done without asking, which is autosave's Library
    /// action ([`Window::library_action`]); where it cannot — an untitled
    /// Document with text and nowhere to put it, or one whose file changed or
    /// went under it — the writer is asked, because the text leaves the window
    /// either way.
    fn replace_with(&self, filed: Filed, then: impl Fn(&Self) + 'static) {
        // A launch of the harness's is never asked anything, as a close is not
        // ([`Window::closing`]): there is no writer at the keyboard to answer,
        // and a dialog over a judged shot is not the state the Gate asked for.
        if self.library_action() || !self.writes() {
            self.set_filed(filed);
            then(self);
            return;
        }
        self.ask_before_replacing(filed, then);
    }

    /// Asks the writer what to do with a Document that cannot be put down
    /// silently, and shows `filed` in its place once they have answered.
    ///
    /// The prompt a close puts up ([`Window::ask_before_leaving`]), for the
    /// same two Documents and for the same reason: Save writes what can be
    /// written, Discard lets the text go, and Cancel leaves the window on the
    /// Document it holds.
    fn ask_before_replacing(&self, filed: Filed, then: impl Fn(&Self) + 'static) {
        let dialog = gtk::AlertDialog::builder()
            .modal(true)
            .message(format!(
                "Save changes to {} before opening another Document?",
                self.document().name()
            ))
            .detail("Your changes will be lost if you don't save them.")
            .buttons(["Save", "Discard", "Cancel"])
            .default_button(0)
            .cancel_button(2)
            .build();
        dialog.choose(
            Some(self),
            None::<&gio::Cancellable>,
            glib::clone!(
                #[weak(rename_to = window)]
                self,
                move |answer| {
                    match answer {
                        Ok(0) if window.keeping() => {}
                        // Nowhere to write it: the writer names the file, and
                        // the Document they asked for waits to be asked for
                        // again rather than standing on an unsaved one.
                        Ok(0) => return window.save_as(After::Stay),
                        Ok(1) => {}
                        // Cancel, `Esc`, or a dialog that could not be shown:
                        // the window stays on the Document it holds, which is
                        // the answer that loses nothing.
                        _ => return,
                    }
                    window.set_filed(filed);
                    then(&window);
                },
            ),
        );
    }

    /// The file a row operation acts on: the Library's selected row where the
    /// pane has one, and the Document this window holds otherwise.
    ///
    /// The context menu selects the row under the pointer before it opens, so
    /// this is also the row that was right-clicked.
    fn target(&self) -> Option<PathBuf> {
        self.imp().sidebar.selected_file().or_else(|| self.path())
    }

    /// `file.rename` and `F2`: the row's name becomes a field to type in, or —
    /// where the pane is shut, or has no file row selected — a small dialog
    /// does.
    pub(crate) fn rename_document(&self) {
        if self.imp().sidebar.is_shown() && self.imp().sidebar.start_rename() {
            return;
        }
        if let Some(path) = self.target() {
            self.rename_dialog(&path);
        }
    }

    /// Renames the file at `path` to `typed` and follows it: the disk first
    /// ([`quill_engine::library::Library::rename`]), then this window where
    /// what moved is the Document it holds, then every window's pane.
    pub(crate) fn rename_path(&self, path: &Path, typed: &str) {
        let Some(session) = self.session() else {
            return;
        };
        self.library_action();
        let to = match session.rename_file(path, typed) {
            Ok(to) => to,
            Err(err) => {
                eprintln!("quill: cannot rename {}: {err}", path.display());
                return;
            }
        };
        self.follow_to(&session, path, &to);
        self.redraw_library();
    }

    /// The rename dialog: one field, for the writer who pressed `F2` with the
    /// Library shut.
    ///
    /// Enter renames and Esc leaves it alone, which is what the field in the
    /// row does; there are no buttons, because there is one thing to say.
    fn rename_dialog(&self, path: &Path) {
        let name = full_name(path);
        let dialog = gtk::Window::builder()
            .title("Rename Document")
            .transient_for(self)
            .modal(true)
            .resizable(false)
            .default_width(DIALOG_WIDTH)
            .build();
        let column = gtk::Box::new(gtk::Orientation::Vertical, DIALOG_PAD);
        column.set_margin_top(DIALOG_PAD);
        column.set_margin_bottom(DIALOG_PAD);
        column.set_margin_start(DIALOG_PAD);
        column.set_margin_end(DIALOG_PAD);
        let entry = gtk::Entry::new();
        entry.set_text(&name);
        entry.select_region(0, files::stem_chars(&name));
        column.append(&entry);
        dialog.set_child(Some(&column));

        let asked = path.to_path_buf();
        entry.connect_activate(glib::clone!(
            #[weak(rename_to = window)]
            self,
            #[weak]
            dialog,
            move |entry| {
                let typed = entry.text().trim().to_string();
                dialog.close();
                if !typed.is_empty() {
                    window.rename_path(&asked, &typed);
                }
            }
        ));
        let keys = gtk::EventControllerKey::new();
        keys.connect_key_pressed(glib::clone!(
            #[weak]
            dialog,
            #[upgrade_or]
            glib::Propagation::Proceed,
            move |_, key, _, _| {
                if key == gdk::Key::Escape {
                    dialog.close();
                    return glib::Propagation::Stop;
                }
                glib::Propagation::Proceed
            }
        ));
        dialog.add_controller(keys);
        dialog.present();
    }

    /// `file.duplicate`: the file beside itself as `X copy`
    /// ([`quill_engine::library::Library::duplicate`]).
    pub(crate) fn duplicate_document(&self) {
        let (Some(session), Some(path)) = (self.session(), self.target()) else {
            return;
        };
        self.library_action();
        if let Err(err) = session.duplicate_file(&path) {
            eprintln!("quill: cannot duplicate {}: {err}", path.display());
            return;
        }
        self.redraw_library();
    }

    /// `file.delete`, which the registry calls Move to Trash.
    pub(crate) fn trash_document(&self) {
        if let Some(path) = self.target() {
            self.trash_path(&path);
        }
    }

    /// Puts the file at `path` in the system trash and says so at the foot of
    /// the Library.
    ///
    /// No prompt: the desktop's trash is the undo that the oracle's twelve
    /// second banner was (`legacy/app/js/files.js` `del`). GIO's trash rather
    /// than a delete, because a writer looking for it will look there — and
    /// GIO is the app's, not the engine's (ADR 0008), so the engine is told
    /// once the file has gone ([`quill_engine::library::Library::trashed`]).
    pub(crate) fn trash_path(&self, path: &Path) {
        let Some(session) = self.session() else {
            return;
        };
        self.library_action();
        if let Err(err) = gio::File::for_path(path).trash(None::<&gio::Cancellable>) {
            eprintln!("quill: cannot move {} to the trash: {err}", path.display());
            return;
        }
        session.trashed_file(path);
        self.redraw_library();
        // After the redraw, because what the pane has just been told about the
        // file is the last thing it should say about it.
        self.imp()
            .sidebar
            .set_status(&files::moved_to_trash(&full_name(path)));
    }

    /// Moves the file at `path` into `folder`, which is what letting a row go
    /// over a folder's row or a Location's head does.
    ///
    /// `library.confirm_move` is off by default, and then the drop is the move.
    /// With it on, a stock `GtkAlertDialog` — the shape the close prompt is
    /// asked in ([`Window::ask_before_leaving`]) — stands between the drop and
    /// the disk, and Cancel leaves the file where it is.
    pub(crate) fn move_path(&self, path: &Path, folder: &Path) {
        let Some(session) = self.session() else {
            return;
        };
        if !session.settings().library.confirm_move {
            self.move_now(path, folder);
            return;
        }
        let dialog = gtk::AlertDialog::builder()
            .modal(true)
            .message(format!(
                "Move {} to {}?",
                full_name(path),
                full_name(folder)
            ))
            .detail("The file moves on disk.")
            .buttons(["Move", "Cancel"])
            .default_button(0)
            .cancel_button(1)
            .build();
        let from = path.to_path_buf();
        let into = folder.to_path_buf();
        dialog.choose(
            Some(self),
            None::<&gio::Cancellable>,
            glib::clone!(
                #[weak(rename_to = window)]
                self,
                move |answer| {
                    // Cancel, `Esc`, or a dialog that could not be shown: the
                    // file stays, which is the answer that moves nothing.
                    if matches!(answer, Ok(0)) {
                        window.move_now(&from, &into);
                    }
                }
            ),
        );
    }

    /// The move itself: the disk first
    /// ([`quill_engine::library::Library::move_to`]), then this window where
    /// what moved is the Document it holds, then every window's pane, and a
    /// notice at the foot of this one.
    fn move_now(&self, path: &Path, folder: &Path) {
        let Some(session) = self.session() else {
            return;
        };
        self.library_action();
        let to = match session.move_file(path, folder) {
            Ok(to) => to,
            Err(err) => {
                eprintln!(
                    "quill: cannot move {} to {}: {err}",
                    path.display(),
                    folder.display()
                );
                return;
            }
        };
        self.follow_to(&session, path, &to);
        self.redraw_library();
        // After the redraw, for the reason [`Window::trash_path`] gives.
        self.imp()
            .sidebar
            .set_status(&files::moved_into(&full_name(path), &full_name(folder)));
    }

    /// Follows the Document this window holds from `from` to `to`, where a
    /// row operation moved the file it was showing.
    ///
    /// The two operations that move a file — a rename and a move into another
    /// folder — leave the same four things behind: the Document takes the new
    /// path, the titles follow the name, the pane highlights the row where it
    /// went, and the watch listens for it there. A window showing something
    /// else, and a file that did not move, leave nothing behind at all.
    fn follow_to(&self, session: &Session, from: &Path, to: &Path) {
        if to == from || self.path().as_deref() != Some(from) {
            return;
        }
        self.imp().filed.borrow_mut().moved_to(to);
        self.shown();
        self.imp().sidebar.set_open(Some(to));
        session.watch_document(to);
    }

    /// `file.pin`, which the registry calls Pin / Unpin: the Library's selected
    /// row, or the Document this window holds where the pane has none
    /// ([`crate::files::pinning`]).
    pub(crate) fn pin_document(&self) {
        let Some(session) = self.session() else {
            return;
        };
        // Copied out, because pinning borrows the Library again to change it.
        let pinned = session.library().pinned().to_vec();
        let selected = self.imp().sidebar.selected_path();
        let Some((path, pin)) =
            files::pinning(selected.as_deref(), self.path().as_deref(), &pinned)
        else {
            return;
        };
        self.set_pinned(&path, pin);
    }

    /// Pins the row at `path`, or unpins it: the Library's Pinned list, and
    /// `[library].pinned` in the settings file behind it.
    pub(crate) fn set_pinned(&self, path: &Path, pinned: bool) {
        let Some(session) = self.session() else {
            return;
        };
        self.library_action();
        let moved = if pinned {
            session.pin(path)
        } else {
            session.unpin(path)
        };
        if moved {
            self.redraw_library();
        }
    }

    /// Drops the Location at `path` from the Library, which touches nothing on
    /// disk ([`Session::remove_location`]).
    pub(crate) fn drop_location(&self, path: &Path) {
        if let Some(session) = self.session() {
            session.remove_location(path);
        }
        self.redraw_library();
    }

    /// `file.openFolder`, which the registry calls Add Location…: the writer
    /// picks a folder and the Library shows it from then on.
    pub(crate) fn add_location(&self) {
        let dialog = gtk::FileDialog::new();
        dialog.set_title("Add Location");
        if let Some(folder) = self.save_folder() {
            dialog.set_initial_folder(Some(&gio::File::for_path(folder)));
        }
        dialog.select_folder(
            Some(self),
            None::<&gio::Cancellable>,
            glib::clone!(
                #[weak(rename_to = window)]
                self,
                move |answer| {
                    let Some(location) = answer.ok().and_then(|file| file.path()) else {
                        return;
                    };
                    window.library_action();
                    if let Some(session) = window.session() {
                        session.add_location(&location);
                    }
                    window.redraw_library();
                }
            ),
        );
    }

    /// `file.next` and `file.prev`: the Document `step` lands on, walking the
    /// list the Library is showing in the order it is showing it
    /// ([`crate::files::stepped`]).
    pub(crate) fn step_document(&self, step: files::Step) {
        let listed = self.imp().sidebar.listed_files();
        if let Some(path) = files::stepped(&listed, self.path().as_deref(), step) {
            self.open_path(&path);
        }
    }

    /// Draws the Library again in every window, which is what a row operation
    /// leaves behind ([`relist`]).
    fn redraw_library(&self) {
        if let Some(app) = self.application() {
            relist(&app);
        }
    }

    /// The caret's byte offset into the Document, which is what a reload puts
    /// back and what a conflict is measured from.
    fn caret_offset(&self) -> usize {
        let buffer = self.imp().editor.buffer();
        let at = buffer.iter_at_mark(&buffer.get_insert());
        tags::offset_of(&self.document(), &at)
    }

    /// Something happened to this window's file: the Document answers it.
    ///
    /// A clean Document takes the disk's text and keeps the caret in the block
    /// it was in; a Document with unsaved edits enters
    /// [`OnDisk::ChangedOnDisk`] and autosave pauses for it
    /// ([`quill_engine::disk::Filed::autosaves`]), the row takes a dot and the
    /// status line offers Reload and Keep ([`Window::show_standing`]). A file
    /// that is gone is the other conflict, and says so.
    fn heard(&self) {
        let caret = self.caret_offset();
        let dirty = self.imp().dirty.get();
        let noticed = self.imp().filed.borrow_mut().noticed(dirty, caret);
        match noticed {
            Noticed::Unchanged => {}
            Noticed::Reloaded(kept) => self.reloaded(kept),
            // The two conflicts: nothing is written and nothing is thrown
            // away, and the status line asks the writer which it is to be.
            Noticed::Changed | Noticed::Deleted => self.show_standing(),
        }
    }

    /// Shows a Document the disk replaced, with the caret back in the block it
    /// was in.
    fn reloaded(&self, kept: Kept) {
        self.imp().dirty.set(false);
        self.shown();
        let document = self.document();
        // The block index came from the Document as it was, and the disk's
        // version may be shorter: a block that is no longer there leaves the
        // caret at the offset it had, clamped. Counting the blocks walks them,
        // which a reload can afford and a keystroke could not.
        let at = kept
            .block
            .filter(|block| *block < document.blocks().len())
            .map_or(kept.caret, |block| document.block(block).at.start);
        self.imp().editor.place_caret(
            &document,
            flags::Caret::At(u64::try_from(at).unwrap_or(u64::MAX)),
            true,
        );
        drop(document);
        self.show_standing();
    }

    /// Where this window's file stands, as the status line says it.
    fn standing(&self) -> Standing {
        match self.imp().filed.borrow().state() {
            OnDisk::ChangedOnDisk => Standing::Changed,
            OnDisk::DeletedOnDisk => Standing::Deleted,
            OnDisk::Untitled | OnDisk::Named => match self.imp().wrote.get() {
                Some(wrote) => Standing::Saved(Duration::from_micros(
                    u64::try_from(Self::now() - wrote).unwrap_or(0),
                )),
                None => Standing::AtRest,
            },
        }
    }

    /// Says where the file stands, at the foot of the Library and on the row:
    /// the words, the two of them the writer can click, and the dot.
    fn show_standing(&self) {
        let standing = self.standing();
        let conflict = standing == Standing::Changed;
        let sidebar = &self.imp().sidebar;
        sidebar.set_status(&files::said(standing));
        sidebar.set_offer(conflict);
        sidebar.set_conflicted(conflict);
    }

    /// Keeps "Saved · 2 min ago" true as the minutes pass.
    ///
    /// One timer per window, armed by the first write and left running: it
    /// draws a label every [`STATUS_TICK`] seconds and stops itself when the
    /// window it belongs to is gone.
    fn tick_standing(&self) {
        if self.imp().ticking.borrow().is_some() {
            return;
        }
        let ticking = self.downgrade();
        let id = glib::timeout_add_seconds_local(STATUS_TICK, move || {
            let Some(window) = ticking.upgrade() else {
                return glib::ControlFlow::Break;
            };
            window.show_standing();
            glib::ControlFlow::Continue
        });
        self.imp().ticking.replace(Some(id));
    }

    /// The Reload of "Changed on disk · Reload · Keep": what taking the disk's
    /// text would do to the window, before it is done.
    pub(crate) fn reload_from_disk(&self) {
        let read = self.imp().filed.borrow().text_on_disk();
        let text = match read {
            Ok(text) => text,
            Err(err) => {
                eprintln!("quill: cannot read the file: {err}");
                return;
            }
        };
        let lines = quill_engine::disk::diff(self.document().text(), &text);
        self.show_diff(conflict::Choice::Reload, &lines);
    }

    /// The Keep of it: what writing the window's text over the file would do
    /// to the file.
    pub(crate) fn keep_over_disk(&self) {
        let against = self.imp().filed.borrow().against_disk();
        let lines = match against {
            Ok(lines) => lines,
            Err(err) => {
                eprintln!("quill: cannot read the file: {err}");
                return;
            }
        };
        self.show_diff(conflict::Choice::Keep, &lines);
    }

    /// Opens the diff view for `choice` over this window; its Accept does it.
    fn show_diff(&self, choice: conflict::Choice, lines: &[Line]) {
        let Some(session) = self.session() else {
            return;
        };
        conflict::open(
            self.upcast_ref(),
            session.ground(),
            choice,
            lines,
            glib::clone!(
                #[weak(rename_to = window)]
                self,
                move || window.accepted(choice)
            ),
        );
    }

    /// The writer accepted the diff: the conflict is resolved the way it said.
    fn accepted(&self, choice: conflict::Choice) {
        match choice {
            conflict::Choice::Reload => self.reload(),
            conflict::Choice::Keep => {
                self.keep_over_the_file();
            }
        }
    }

    /// Takes the disk's text, with the caret back in the block it was in.
    fn reload(&self) {
        let caret = self.caret_offset();
        let kept = self.imp().filed.borrow_mut().reload(caret);
        match kept {
            Ok(kept) => self.reloaded(kept),
            Err(err) => eprintln!("quill: cannot reload: {err}"),
        }
    }

    /// Answers `close-request`: the window goes, or asks the writer first.
    ///
    /// [`crate::files::leaving`] decides which. A prompt keeps the window
    /// (`Stop`) and closes it again once it has an answer, which is the only
    /// way GTK lets a close be undone.
    fn closing(&self) -> glib::Propagation {
        // A launch of the harness's is never asked anything: there is no
        // writer at the keyboard to answer, and a dialog over a judged shot is
        // not the state the Gate asked for.
        if self.imp().answered.get() || !self.writes() {
            return self.go();
        }
        let state = self.imp().filed.borrow().state();
        let has_text = !self.document().text().is_empty();
        match files::leaving(state, has_text) {
            Leaving::Go => self.go(),
            // The write has to have happened. A failed one — a folder that
            // cannot be written, a file that moved under the Document since
            // ([`quill_engine::disk::Saved::Refused`]) — is exactly the case
            // the prompt is for, so it asks rather than closing on a file that
            // does not hold what the writer typed.
            Leaving::Flush if self.flush() => self.go(),
            Leaving::Flush | Leaving::Ask => {
                self.ask_before_leaving(Self::leave);
                glib::Propagation::Stop
            }
        }
    }

    /// The window is going: its shape is taken down and the close proceeds.
    fn go(&self) -> glib::Propagation {
        if let Some(armed) = self.imp().saving.take() {
            armed.remove();
        }
        if let Some(ticking) = self.imp().ticking.take() {
            ticking.remove();
        }
        if let Some(refresh) = self.imp().refresh.take() {
            refresh.remove();
        }
        self.remember();
        glib::Propagation::Proceed
    }

    /// Whether closing this window would put a question to the writer, which
    /// is what a quit has to know before it closes anything ([`quit`]).
    fn would_ask(&self) -> bool {
        if self.imp().answered.get() || !self.writes() {
            return false;
        }
        let state = self.imp().filed.borrow().state();
        let has_text = !self.document().text().is_empty();
        matches!(files::leaving(state, has_text), Leaving::Ask)
    }

    /// Closes the window without asking again, whatever the Document is in.
    fn leave(&self) {
        self.imp().answered.set(true);
        self.close();
    }

    /// The Save button of the prompt a close asked: what it means for the
    /// Document standing in the way.
    ///
    /// Keep for a Document the disk has moved under — Save there is the writer
    /// saying their text is the one to have, which is exactly Keep, and a
    /// window on its way out has no room left to show them a diff — and
    /// autosave's own flush for everything else.
    fn keeping(&self) -> bool {
        let state = self.imp().filed.borrow().state();
        match state {
            OnDisk::ChangedOnDisk | OnDisk::DeletedOnDisk => self.keep_over_the_file(),
            OnDisk::Untitled | OnDisk::Named => self.flush(),
        }
    }

    /// Asks the writer what to do with a Document that cannot be closed
    /// silently: Save, Discard, Cancel, with Cancel keeping the window.
    ///
    /// `answered` is what Save and Discard lead to — the window leaving, for a
    /// close, and the next window being asked, for a quit ([`quit`]) — and
    /// Cancel leads nowhere, which is what makes it the answer that loses
    /// nothing.
    fn ask_before_leaving(&self, answered: impl Fn(&Self) + 'static) {
        let dialog = gtk::AlertDialog::builder()
            .modal(true)
            .message(format!(
                "Save changes to {} before closing?",
                self.document().name()
            ))
            .detail("Your changes will be lost if you don't save them.")
            .buttons(["Save", "Discard", "Cancel"])
            .default_button(0)
            .cancel_button(2)
            .build();
        dialog.choose(
            Some(self),
            None::<&gio::Cancellable>,
            glib::clone!(
                #[weak(rename_to = window)]
                self,
                move |answer| match answer {
                    Ok(0) if window.keeping() => answered(&window),
                    // Nowhere to flush to: the writer names the file, and the
                    // window closes when they have.
                    Ok(0) => window.save_as(After::Close),
                    Ok(1) => answered(&window),
                    // Cancel, `Esc`, or a dialog that could not be shown: the
                    // window stays, which is the answer that loses nothing.
                    _ => {}
                },
            ),
        );
    }

    /// Hides the two bars, or shows them again.
    ///
    /// `docs/shortcuts.md`'s `chrome.toggle` row. The session remembers the
    /// value and every window's bars follow, for the reason
    /// [`Window::move_windows`] gives.
    pub(crate) fn toggle_bars(&self) {
        self.move_windows(Session::toggle_chrome, |window, chrome| {
            window.imp().bars.set_shown(chrome == Chrome::Shown);
        });
    }

    /// `library.toggle`: the Library stands beside the page, or steps out of
    /// it.
    ///
    /// Per window rather than per session — a writer looking something up in
    /// one window has not asked for the pane in the others — and so not one of
    /// [`Window::move_windows`]'s modes, and nothing the settings file holds.
    pub(crate) fn toggle_library(&self) {
        self.show_library(!self.imp().sidebar.is_shown());
    }

    /// Stands the Library pane at `wanted` logical pixels wide, in this
    /// window and in every other.
    ///
    /// One width for the app, as the ground and the type size are and for the
    /// reason [`Window::move_windows`] gives: a writer has one pair of eyes,
    /// and a divider dragged in one window is the Library's width everywhere.
    /// It is not a setting, though — what a writer dragged is what Quill
    /// observed — so it is taken down in the state
    /// ([`Session::set_library_width`]) and the file is written as the drag
    /// ends ([`crate::sidebar::Sidebar::watch_divider`]).
    ///
    /// `wanted` is what the drag asked for; what the pane is given is what
    /// this window can hold ([`quill_engine::settings::library_width`]).
    pub(crate) fn resize_library(&self, wanted: i32) {
        let Some(session) = self.session() else {
            return;
        };
        let width = library_width(
            u32::try_from(wanted).unwrap_or_default(),
            u32::try_from(self.width()).unwrap_or(u32::MAX),
        );
        session.set_library_width(width);
        let Some(app) = self.application() else {
            return;
        };
        for window in windows(&app) {
            window.imp().sidebar.set_width(width);
        }
    }

    /// Stands the Library beside the page, or takes it away.
    ///
    /// The title bar's toggle goes with it: the pane's own head carries the
    /// one that shuts it while it is open.
    fn show_library(&self, shown: bool) {
        self.imp().sidebar.set_shown(shown);
        self.imp().bars.set_library_toggle_shown(!shown);
    }

    /// `preview.full` and `preview.split`: the rendered page stands where
    /// `asked` says, or steps out.
    ///
    /// Each chord owns a layout and is that layout's toggle, which is
    /// [`next_preview_pane`]'s rule: closed, the pane opens in `asked`; open
    /// in the other layout, it switches to `asked` in place with the keyboard
    /// following; open in `asked`, it closes. #263 story 4 asks for
    /// `Ctrl+Shift+R` "to switch between Split and Full … without hiding the
    /// pane first", which the middle case is.
    ///
    /// Whether the pane is open is this window's, as the Library's pane is
    /// and for the same reason, and — unlike the Library's width and unlike
    /// the layout — nothing the state or the settings file holds: the pane is
    /// closed at every launch (#263). The layout is the session's, so it
    /// moves the way the ground does ([`Window::move_windows`]) and every
    /// other window's open pane follows it; a window with no pane open is
    /// left alone, because the pane work below would take the keyboard off a
    /// Library search field to hand it to the Editor.
    pub(crate) fn preview_to(&self, asked: PreviewLayout) {
        // A window with no session has no layout to show, and
        // [`Window::apply_preview`] reads that case as Split.
        let showing = self
            .session()
            .map_or(PreviewLayout::Split, |session| session.preview_layout());
        match next_preview_pane(self.imp().previewing.get(), showing, asked) {
            PreviewPane::Closed => self.show_preview(false),
            PreviewPane::Open(layout) => {
                self.move_windows(
                    |session| session.set_preview_layout(layout),
                    |window, ()| {
                        if !window.imp().previewing.get() {
                            return;
                        }
                        window.apply_preview();
                        window.refresh_preview();
                        window.focus_pane();
                    },
                );
                // The pass above has already re-applied an open pane, this
                // window's included; a closed one is this window's to open.
                if !self.imp().previewing.get() {
                    self.show_preview(true);
                }
            }
        }
    }

    /// Stands every window's Preview pane in `mode`, and writes the key.
    ///
    /// `docs/shortcuts.md`'s `preview.web` and `preview.pdf` rows, View ›
    /// Panes' two mode rows and the Settings window's Mode row: one mode for
    /// the app, as the Template is, and not a state a window is in.
    ///
    /// The write is the one path a setting takes
    /// ([`Session::edit_settings`]), and the live value beside it is what
    /// keeps the pane and the menu's check from waiting on the watch to read
    /// that write back — the same pair a zoom step is
    /// ([`Session::set_preview_zoom`]).
    pub(crate) fn set_preview_mode(&self, mode: PreviewMode) {
        let Some(session) = self.session() else {
            return;
        };
        if mode == session.preview_mode() {
            return;
        }
        session.set_preview_mode(mode);
        session.edit_settings(|settings| settings.preview.mode = mode);
        let Some(app) = self.application() else {
            return;
        };
        for window in windows(&app) {
            // A pane that is not open lays nothing out; the mode is on it all
            // the same, so opening it later opens it in this one.
            window.imp().preview.set_mode(mode);
            window.refresh_preview();
            window.show_page_words();
        }
    }

    /// Presents the Export dialog already standing over this window.
    ///
    /// Every Export command goes through this before building a dialog, so a
    /// second chord or menu choice returns to the existing job.
    pub(crate) fn present_export_dialog(&self) -> bool {
        let standing = self
            .imp()
            .export_dialog
            .borrow()
            .as_ref()
            .and_then(glib::WeakRef::upgrade);
        if let Some(dialog) = standing {
            dialog.present();
            true
        } else {
            self.imp().export_dialog.replace(None);
            false
        }
    }

    /// Remembers the one Export dialog standing over this window.
    pub(crate) fn hold_export_dialog(&self, dialog: &gtk::Window) {
        self.imp().export_dialog.replace(Some(dialog.downgrade()));
    }

    /// Forgets the Export dialog as its one close hook runs.
    pub(crate) fn release_export_dialog(&self) {
        self.imp().export_dialog.replace(None);
    }

    /// Stands this window's Preview pane under an Export dialog that has just
    /// opened: `over`'s mode and its Options' values, the pane opened in Split
    /// where it was away, and not a line written to the settings file.
    ///
    /// The pane a dialog drives is this window's own. The mode and the values
    /// it drives it with are the pane's own override
    /// ([`crate::preview::Preview::set_dialog_override`]) rather than a
    /// setting, so `[preview] mode` and `[export]` are what the close comes
    /// back to. What was there before is answered here and handed back to
    /// [`Window::drop_dialog_preview`], because a dialog is one job and
    /// nothing about it outlives its window.
    pub(crate) fn show_dialog_preview(&self, over: &DialogOverride) -> DialogPreviewBefore {
        let imp = self.imp();
        let before = DialogPreviewBefore {
            open: imp.previewing.get(),
        };
        imp.dialog_preview.borrow_mut().open();
        imp.preview.set_dialog_override(Some(over.clone()));
        if !before.open {
            // Split, whatever layout the pane was last opened in: a Full pane
            // behind a dialog that had to open it would replace the Editor
            // the job belongs to. This window's own Split and not the session's
            // ([`imp::Window::dialog_split`]), because a dialog writes no
            // setting and **Save as defaults** composes the file from what the
            // session is running. The pane is opened here rather than through
            // [`Window::show_preview`] because the keyboard belongs to the
            // dialog, and because a pane opened by a dialog is put back by the
            // close rather than remembered.
            imp.dialog_split.set(true);
            imp.previewing.set(true);
        }
        self.apply_preview();
        self.refresh_preview();
        before
    }

    /// Takes down what **Save as defaults** wrote while a dialog stands over
    /// this window's pane, so that the close lays the pane out at it
    /// ([`Window::drop_dialog_preview`]).
    pub(crate) fn save_dialog_preview(&self, saved: Settings) {
        self.imp().dialog_saved.replace(Some(saved));
    }

    /// Lays the pane out again at what an open dialog's Options now say.
    ///
    /// One call for every control on the widget, because there is one thing
    /// any of them means ([`crate::export_dialog::Options::on_change`]).
    pub(crate) fn move_dialog_preview(&self, over: &DialogOverride) {
        self.imp().preview.set_dialog_override(Some(over.clone()));
        self.refresh_preview();
    }

    /// Puts the pane back to what `before` says the dialog opened over: the
    /// Options' values dropped, the pages re-laid at the `[export]` table that
    /// now stands, and the pane hidden again where the dialog is what opened
    /// it.
    ///
    /// **Save as defaults** wrote `[export]` before ever this runs, and what
    /// the pane comes back to is what was saved: the write is the one path a
    /// setting takes ([`Session::edit_settings`]), but the drain that reads the
    /// file back has not run yet, so the settings the write composed are what
    /// the drop lays out at ([`dropped_preview`]) rather than the running table
    /// the write is still ahead of.
    ///
    /// The keyboard is left where the closing dialog puts it, which is this
    /// window: a pane opened by a dialog never took it.
    pub(crate) fn drop_dialog_preview(&self, before: DialogPreviewBefore) {
        let imp = self.imp();
        imp.dialog_preview.borrow_mut().close();
        if let Some(scrim) = imp.editor_scrim.get() {
            scrim.set_visible(false);
        }
        imp.preview.set_dialog_override(None);
        imp.dialog_split.set(false);
        let saved = imp.dialog_saved.take();
        let Some(session) = self.session() else {
            return;
        };
        let dropped = dropped_preview(before, &session.running(), saved);
        imp.previewing.set(dropped.open);
        self.apply_preview();
        if dropped.open {
            self.refresh_preview_at(&dropped.settings);
        } else {
            // The pane is away, so the page it stood over is not on the bar.
            self.show_page_words();
        }
    }

    /// Opens the Preview pane or shuts it.
    ///
    /// Closing hands the keyboard back to the Editor, which is what Full took
    /// it from: the caret and the scroll are where the writer left them,
    /// because hiding a widget moves neither.
    fn show_preview(&self, shown: bool) {
        self.imp().previewing.set(shown);
        self.apply_preview();
        if shown {
            self.refresh_preview();
            self.focus_pane();
        } else {
            // The pane is away, so the page it stood over is not on the bar.
            self.show_page_words();
            self.focus_editor();
        }
    }

    /// Stands the Preview pane at `wanted` logical pixels wide, in this window
    /// and in every other.
    ///
    /// One width for the app, as the Library's is and for the reason
    /// [`Window::resize_library`] gives; taken down in the state rather than
    /// the settings, because what a writer dragged is what Quill observed, and
    /// written as the drag ends ([`crate::preview::Preview`]).
    pub(crate) fn resize_preview(&self, wanted: i32) {
        let Some(session) = self.session() else {
            return;
        };
        // A drag is always a width, however far left the pointer went: the
        // pane stops at the narrowest it may stand at rather than falling back
        // to the even divide, which is what a pane nobody dragged reads as.
        let width = crate::preview::pane_width(
            Some(u32::try_from(wanted).unwrap_or_default()),
            u32::try_from(self.pair_width()).unwrap_or(u32::MAX),
        );
        session.set_preview_width(width);
        let Some(app) = self.application() else {
            return;
        };
        for window in windows(&app) {
            window.apply_preview();
        }
    }

    /// Puts the pane's state — open or away, Split or Full, and how wide — on
    /// to this window's widgets.
    ///
    /// The one pass all three go through, because they are one shape: Full is
    /// the pane open with the Editor's scroller away, and a width means
    /// nothing in it.
    fn apply_preview(&self) {
        let imp = self.imp();
        let shown = imp.previewing.get();
        // A dialog that opened this window's pane holds it in Split without
        // moving the session's layout ([`Window::show_dialog_preview`]).
        let split = imp.dialog_split.get()
            || self
                .session()
                .is_none_or(|session| session.preview_layout() == PreviewLayout::Split);
        imp.preview.set_shown(shown);
        if let Some(editor_frame) = imp.editor_frame.get() {
            editor_frame.set_visible(!shown || split);
        }
        let dimmed = imp.dialog_preview.borrow_mut().apply(shown, split);
        if let Some(scrim) = imp.editor_scrim.get() {
            scrim.set_visible(dimmed);
        }
        let width = match self.session() {
            Some(session) if shown && split => crate::preview::pane_width(
                session.preview_width(),
                u32::try_from(self.pair_width()).unwrap_or(u32::MAX),
            ),
            _ => None,
        };
        if let Some(pair) = imp.pair.get() {
            // An even Split is the two halves of a homogeneous box rather
            // than a width worked out here: the pair knows how wide it is and
            // a window has not been allocated when this first runs.
            pair.set_homogeneous(shown && split && width.is_none());
        }
        imp.preview.set_width(width);
    }

    /// How wide the pair the panes divide is, which is the window less the
    /// Library.
    fn pair_width(&self) -> i32 {
        self.imp()
            .pair
            .get()
            .map_or_else(|| self.width(), WidgetExt::width)
    }

    /// Lays the Document out again in the Preview pane, where one is open.
    ///
    /// Everything the render pass reads — the Template, its toggles, the zoom
    /// and the ground — is the session's, so the pane is handed the settings
    /// rather than a copy of each ([`crate::preview::Preview::refresh`]). The
    /// settings it is handed are the ones this launch is running
    /// ([`Session::running`]) rather than the ones it last read, because a
    /// Template picked from the menu and a zoom stepped by a key are held live
    /// and written to the file, not read back out of it.
    pub(crate) fn refresh_preview(&self) {
        let Some(session) = self.session() else {
            return;
        };
        self.refresh_preview_at(&session.running());
    }

    /// The same, laid out at `settings` rather than at what the session is
    /// running.
    ///
    /// The one caller that needs the difference is the close of an Export
    /// dialog whose **Save as defaults** was pressed: what it wrote is ahead of
    /// the running table until the watch reads the file back
    /// ([`Window::drop_dialog_preview`]).
    fn refresh_preview_at(&self, settings: &Settings) {
        if !self.imp().previewing.get() {
            return;
        }
        let Some(session) = self.session() else {
            return;
        };
        let document = self.document();
        self.imp()
            .preview
            .refresh(&document, settings, session.ground().scheme);
        drop(document);
        self.show_page_words();
        // The page is a new page, so the scroll it had means nothing: the pane
        // is put back by whichever rule last placed it. For an edit that is the
        // caret's block, which is what the writer was looking at (#263
        // § Refresh); a wheel or a task box flipped by a press left the pane in
        // step with the Editor instead, and it stays there.
        self.refollow();
    }

    /// Puts the page under the column's top edge on the stats bar, and takes
    /// the cell away where there is no page there: Web mode, and a pane that
    /// is not open.
    ///
    /// Asked of the pane rather than worked out here, because which mode is
    /// showing and where it stands are both the pane's
    /// ([`crate::preview::Preview::page_words`]); the words themselves are the
    /// engine's, as the counts beside them are.
    pub(crate) fn show_page_words(&self) {
        let imp = self.imp();
        let words = imp
            .previewing
            .get()
            .then(|| imp.preview.page_words())
            .flatten();
        imp.bars.set_page(words);
    }

    /// Puts the Preview back where the rule that last placed it says, after a
    /// refresh has laid the page out again.
    ///
    /// The two rules are the whole of what moves the pane, and neither is the
    /// right answer to the other's page: putting an edit's caret rule on a pane
    /// a wheel had left in step would snap it, and a wheel's rule on a pane the
    /// caret placed would shift it for nothing.
    fn refollow(&self) {
        let imp = self.imp();
        match imp.follows.get() {
            Follows::Caret => self.follow_caret(),
            Follows::TopBlock => {
                if let Some(scroller) = imp.scroller.get() {
                    self.follow_editor(scroller.vadjustment().value());
                }
            }
        }
    }

    /// Arms the Preview's refresh, or re-arms it where an earlier keystroke of
    /// the same burst already did.
    ///
    /// The whole of what an edit costs the Preview: one timer taken down and
    /// one put up, and nothing of the render pass on the keystroke lane. A
    /// window with no pane open arms nothing at all.
    fn arm_refresh(&self) {
        if !self.imp().previewing.get() {
            return;
        }
        if let Some(armed) = self.imp().refresh.take() {
            armed.remove();
        }
        let id = glib::timeout_add_local_once(
            REFRESH,
            glib::clone!(
                #[weak(rename_to = window)]
                self,
                move || {
                    window.imp().refresh.take();
                    window.refresh_preview();
                },
            ),
        );
        self.imp().refresh.replace(Some(id));
    }

    /// Puts the Preview where the caret's block asks for it: the caret rule
    /// ([`quill_engine::sync::follow_caret`]), which is what an edit and a
    /// caret move drive.
    ///
    /// One block index and one fraction, both of them O(1): this runs on every
    /// keystroke and every arrow key, and the page it reads is whatever the
    /// last refresh laid out.
    ///
    /// Recorded as the rule now placing the pane, so that the refresh an edit
    /// arms puts it back the same way ([`Window::refollow`]).
    fn follow_caret(&self) {
        let imp = self.imp();
        if !imp.previewing.get() {
            return;
        }
        imp.follows.set(Follows::Caret);
        // `try_borrow` because a `mark-set` can arrive while the splice that
        // moved the mark still holds the Document ([`Window::watch_edits`]).
        let Ok(filed) = imp.filed.try_borrow() else {
            return;
        };
        let document = filed.document();
        let buffer = imp.editor.buffer();
        let at = buffer.iter_at_mark(&buffer.get_insert());
        let Some(caret) = document.block_at(tags::offset_of(document, &at)) else {
            return;
        };
        drop(filed);
        let Some(fraction) = imp.editor.caret_fraction() else {
            return;
        };
        self.follow_preview(imp.preview.caret_offset(caret, fraction), true);
    }

    /// Puts the Preview where the Editor's top edge asks for it: the top-block
    /// rule, which is what a wheel or a scrollbar over the Editor drives.
    ///
    /// Recorded as the rule now placing the pane, for the reason
    /// [`Window::follow_caret`] records its own.
    ///
    /// A scroll the Editor makes to put the caret's row back after a fold is
    /// not a writer scrolling and drives nothing: the row it moved is the row
    /// it left standing still, and reading it as a scroll would take the pane
    /// off the caret's block an edit had just put it on
    /// ([`crate::editor::Editor::shifting`]).
    fn follow_editor(&self, offset: f64) {
        let imp = self.imp();
        if !imp.previewing.get() || imp.syncing.get() || imp.editor.shifting() {
            return;
        }
        imp.follows.set(Follows::TopBlock);
        // `try_borrow` for the reason [`Window::follow_caret`] has it: a scroll
        // is one of the things an edit sets off, and the splice may still be
        // holding the Document when it arrives.
        let Ok(filed) = imp.filed.try_borrow() else {
            return;
        };
        let driver = imp.editor.top_block(filed.document(), offset);
        drop(filed);
        let Some(driver) = driver else {
            return;
        };
        // One block is the whole of what the rule reads of a driver: whichever
        // block the top edge falls in, and how far into it
        // ([`crate::editor::Editor::top_block`]).
        // Asked again if it does not land, as the caret rule asks: the sheet is
        // laid out after the Editor's first scroll arrives, and until it is the
        // adjustment clamps every value to the end it has been told about — an
        // Editor put where `--scroll` asked at launch drove a Preview that
        // stayed at the top.
        self.follow_preview(imp.preview.top_block_offset(&[driver], offset), true);
    }

    /// Puts the Editor where the Preview's top edge asks for it: the same rule
    /// the other way round, which is what a wheel or a scrollbar over the
    /// rendered page drives.
    ///
    /// Recorded as the rule now placing the pane, for the reason
    /// [`Window::follow_caret`] records its own: the two panes are in step, and
    /// a refresh keeps them there.
    fn follow_preview_scroll(&self, offset: f64) {
        let imp = self.imp();
        if !imp.previewing.get() || imp.syncing.get() {
            return;
        }
        imp.follows.set(Follows::TopBlock);
        let Some(adjustment) = imp.scroller.get().map(|scroller| scroller.vadjustment()) else {
            return;
        };
        let Ok(filed) = imp.filed.try_borrow() else {
            return;
        };
        let follower = imp.editor.block_rows(filed.document());
        drop(filed);
        let max = (adjustment.upper() - adjustment.page_size()).max(0.0);
        let to = sync::follow_top_block(&imp.preview.blocks(), offset, &follower, max);
        imp.syncing.set(true);
        adjustment.set_value(to);
        imp.syncing.set(false);
    }

    /// Scrolls the Preview to `to`, without the scroll driving back.
    ///
    /// A page laid out again this instant is taller or shorter than the
    /// scroller has been allocated for, and `gtk::Adjustment` clamps to the
    /// end it has been told about; so a value that did not land is asked for
    /// once more on the main loop, by which time GTK has laid the sheet out.
    /// Once more and no further: a value that will not land twice is a page
    /// with nowhere to put it.
    fn follow_preview(&self, to: f64, again: bool) {
        let imp = self.imp();
        let adjustment = imp.preview.vadjustment();
        imp.syncing.set(true);
        adjustment.set_value(to);
        imp.syncing.set(false);
        if again && (adjustment.value() - to).abs() > 0.5 {
            glib::idle_add_local_once(glib::clone!(
                #[weak(rename_to = window)]
                self,
                move || window.follow_preview(to, false),
            ));
        }
    }

    /// Carries each pane's scroll to the other.
    ///
    /// Both directions off the adjustments rather than off a wheel, because a
    /// scrollbar drag, a Page key and a wheel are one thing to a follower;
    /// the guard is what keeps the answer from being read as a second
    /// question. The caret rule runs after this one for an edit — the Editor
    /// has already moved its own view by the time `mark-set` reaches
    /// [`Window::watch_edits`] — so the finer rule has the last word.
    ///
    /// One scroll runs later still and is not a rule at all: an edit under Live
    /// folds the page, and the frame after puts the caret's row back where it
    /// stood ([`crate::editor::Editor::anchor_row`]). That one is stood back
    /// from rather than followed, so the caret rule keeps the last word.
    fn watch_sync(&self) {
        let Some(scroller) = self.imp().scroller.get() else {
            return;
        };
        scroller.vadjustment().connect_value_changed(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |adjustment| window.follow_editor(adjustment.value()),
        ));
        let preview = self.imp().preview.vadjustment();
        preview.connect_value_changed(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |adjustment| {
                // The page counts up with the scroll however the scroll was
                // made, a sync's own included, so this is read before the
                // guard [`Window::follow_preview_scroll`] keeps.
                window.show_page_words();
                window.follow_preview_scroll(adjustment.value());
            },
        ));
        // A column laid out again is a new count of pages under an unmoved
        // scroll, which `changed` says and `value-changed` does not.
        preview.connect_changed(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |_| window.show_page_words(),
        ));
    }

    /// Works Live's furniture out again for the rows a scroll or a resize
    /// brought on to the glass.
    ///
    /// [`crate::editor::Editor::refurnish`] is bounded to the viewport, and the
    /// two feeds that call it — an edit and a caret move — are both about the
    /// buffer. A wheel, a scrollbar and a window pulled taller move the
    /// viewport and touch no byte, so the furniture would be worked out for
    /// rows that have scrolled off and never for the rows that arrived. The
    /// adjustment says both things: `value-changed` is the scroll, and
    /// `changed` is the page under it growing or shrinking.
    ///
    /// Live off is a borrow and a return in the Editor, so this is connected
    /// once for the window's life rather than switched with the mode.
    ///
    /// Neither feed furnishes on the spot ([`Window::arm_refurnish`]): GTK
    /// emits both from inside `size_allocate`, on a layout it has not validated
    /// yet, and the pass reads the view for the rows on the glass.
    fn watch_furniture(&self) {
        let Some(scroller) = self.imp().scroller.get() else {
            return;
        };
        let adjustment = scroller.vadjustment();
        adjustment.connect_value_changed(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |_| window.arm_refurnish(),
        ));
        adjustment.connect_changed(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |_| window.arm_refurnish(),
        ));
    }

    /// Asks for the furniture to be worked out again on the next frame, unless
    /// a pass is already owed.
    ///
    /// Two things at once. The many `value-changed` a wheel, a glide or an
    /// allocation emit become one pass, in the frame's UPDATE phase; and that
    /// phase is after GTK's own validate idle and never inside `size_allocate`,
    /// where the layout is mid-flight and the lookups the pass makes of it
    /// abort (#277). A tick armed during LAYOUT — a resize — runs on the frame
    /// after, so a resize shows one frame of rows without their furniture.
    fn arm_refurnish(&self) {
        if self.imp().furnish_owed.replace(true) {
            return;
        }
        self.imp().editor.add_tick_callback(glib::clone!(
            #[weak(rename_to = window)]
            self,
            #[upgrade_or]
            glib::ControlFlow::Break,
            move |_, _| {
                window.imp().furnish_owed.set(false);
                window.refurnish();
                glib::ControlFlow::Break
            }
        ));
    }

    /// Hands the Editor the Document so it can furnish the rows it now shows.
    ///
    /// `try_borrow` for the reason [`Window::follow_editor`] has it: a scroll
    /// is one of the things an edit sets off, and the splice may still be
    /// holding the Document when it arrives. The edit's own pass furnishes the
    /// page after it, so nothing is lost by standing back here.
    fn refurnish(&self) {
        let imp = self.imp();
        let Ok(filed) = imp.filed.try_borrow() else {
            return;
        };
        imp.editor.refurnish(filed.document());
    }

    /// Steps the rendered page's zoom, or puts it back to the default:
    /// `preview.bigger`, `preview.smaller` and `preview.reset`, and Ctrl+wheel
    /// over the pane ([`crate::preview::Preview`]).
    ///
    /// The zoom is the settings file's, so every open pane re-renders at it,
    /// the way a size step re-types every window ([`Window::step_size`]); the
    /// Editor's own `font.*` ladder never reaches the Preview and this never
    /// reaches the Editor.
    pub(crate) fn step_zoom(&self, direction: Zoom) {
        let Some(session) = self.session() else {
            return;
        };
        let zoom = stepped_zoom(session.preview_zoom(), direction);
        if zoom == session.preview_zoom() {
            return;
        }
        session.set_preview_zoom(zoom);
        // Written as the key is pressed, for the reason the size is
        // ([`Window::step_size`]).
        session.store_settings();
        let Some(app) = self.application() else {
            return;
        };
        for window in windows(&app) {
            window.refresh_preview();
        }
    }

    /// The keyboard goes wherever the pane's state says it should: to the
    /// Preview in Full, since there is no Editor on screen to type into, and
    /// to the Editor everywhere else.
    fn focus_pane(&self) {
        let full = self
            .session()
            .is_some_and(|session| session.preview_layout() == PreviewLayout::Full);
        if self.imp().previewing.get() && full {
            self.imp().preview.grab_focus();
        } else {
            self.focus_editor();
        }
    }

    /// `library.search`: the Library stands beside the page if it was away,
    /// and the keyboard goes to its search field.
    pub(crate) fn search_library(&self) {
        self.show_library(true);
        self.imp().sidebar.focus_search();
    }

    /// The keyboard goes back to the page: what Esc does in the sidebar.
    pub(crate) fn focus_editor(&self) {
        self.imp().editor.grab_focus();
    }

    /// Esc anywhere the page has the keyboard: a search still standing is
    /// dropped and the pane draws its tree again (#246 story 18).
    ///
    /// A writer searches, opens the hit and is then on the page with the pane
    /// still showing the hits; Esc is what puts the Library back, and the pane
    /// cannot hear it because the keyboard is not in the pane. The controller
    /// watches in the bubble phase, so it hears only what the widget with the
    /// keyboard let through: the Palette's field, the pane's field and a
    /// rename field each take Esc for themselves.
    fn watch_escape(&self) {
        let keys = gtk::EventControllerKey::new();
        keys.connect_key_pressed(glib::clone!(
            #[weak(rename_to = window)]
            self,
            #[upgrade_or]
            glib::Propagation::Proceed,
            move |_, key, _, _| {
                if key == gdk::Key::Escape && window.imp().sidebar.clear_query() {
                    window.focus_editor();
                    return glib::Propagation::Stop;
                }
                glib::Propagation::Proceed
            }
        ));
        self.add_controller(keys);
    }

    /// The typing machine's clock: the same monotonic microseconds the frame
    /// clock counts in, read off the system because a timer fires between
    /// frames.
    fn now() -> i64 {
        glib::monotonic_time()
    }

    /// A real keystroke went through the buffer: the chrome steps back and
    /// the autosave clock restarts.
    ///
    /// On the keystroke path, so it does as little as the machine allows —
    /// two CSS classes that are already on stay on, and one timer is
    /// re-armed. The count is not taken here; the timer asks for it. Autosave
    /// is [`Window::edited`]'s two `Cell`s and nothing else: no file is
    /// touched between keystrokes.
    ///
    /// A task box flipped by a press is an edit the caret did not make, and it
    /// drives no sync rule at all: the pane stands where the last rule put it,
    /// and the refresh this arms puts it back there rather than on the caret's
    /// block ([`Window::refollow`]).
    fn typed(&self) {
        self.edited();
        // Two timers and nothing else on the keystroke lane: autosave's, above,
        // and the rendered page's ([`Window::arm_refresh`]).
        self.arm_refresh();
        if !self.imp().editor.pressing_box() {
            self.follow_caret();
        }
        let mut typing = self.imp().typing.get();
        typing.keystroke(Self::now());
        self.imp().typing.set(typing);
        self.settle();
    }

    /// The pointer moved: the bars come back at once, whatever the timers
    /// had left to do. Nothing at rest, which is where the pointer mostly
    /// finds it.
    fn woken(&self) {
        let now = Self::now();
        let mut typing = self.imp().typing.get();
        if !typing.typing(now) {
            return;
        }
        typing.pointer();
        self.imp().typing.set(typing);
        self.settle();
    }

    /// Puts the bars where the machine says they are now, counts on idle if
    /// a count is owed, and arms the one timer for the next thing the machine
    /// will do on its own.
    fn settle(&self) {
        let now = Self::now();
        let mut typing = self.imp().typing.get();
        self.imp()
            .bars
            .set_fade(typing.title_alpha(now), typing.stats_alpha(now));
        if typing.takes_recount(now) {
            glib::idle_add_local_once(glib::clone!(
                #[weak(rename_to = window)]
                self,
                move || window.recount(),
            ));
        }
        self.imp().typing.set(typing);
        if let Some(armed) = self.imp().wake.take() {
            armed.remove();
        }
        if let Some(when) = typing.resumes_at(now) {
            let left = u64::try_from(when - now).unwrap_or(0);
            let id = glib::timeout_add_local_once(
                std::time::Duration::from_micros(left),
                glib::clone!(
                    #[weak(rename_to = window)]
                    self,
                    move || {
                        window.imp().wake.take();
                        window.settle();
                    },
                ),
            );
            self.imp().wake.replace(Some(id));
        }
    }

    /// Counts the Document into the stats bar, on idle.
    fn recount(&self) {
        let document = self.document();
        self.imp().bars.set_count(document.text());
    }

    /// Asks for the held run to be counted on the next frame, once however
    /// many times the selection has moved since the last one.
    ///
    /// A drag moves the selection bound on every motion event, and the count
    /// walks the run; coalescing to the frame clock is what keeps dragging
    /// across a long Document free, as the oracle's `requestAnimationFrame`
    /// does (`legacy/app/js/chrome.js`). Adding the callback queues the frame,
    /// so a selection that moves and then stands still is still counted.
    fn arm_selection_count(&self) {
        if self.imp().selecting.replace(true) {
            return;
        }
        self.add_tick_callback(|window, _| {
            window.imp().selecting.set(false);
            window.count_selection();
            glib::ControlFlow::Break
        });
    }

    /// Counts the run the buffer holds into the stats bar, or gives the bar
    /// back to the Document where no run is held.
    ///
    /// The buffer's own text rather than a slice of the Document: the two
    /// carry the same bytes ([`crate::editor::Editor::show_document`] fills
    /// one from the other and every edit is spliced back), and asking the
    /// buffer for the run spares both the char-to-byte arithmetic its offsets
    /// would need and a copy of the whole Document to slice.
    fn count_selection(&self) {
        let buffer = self.imp().editor.buffer();
        match buffer.selection_bounds() {
            Some((from, to)) => {
                let run = buffer.text(&from, &to, true);
                self.imp().bars.set_selection(Some(run.as_str()));
            }
            None => self.imp().bars.set_selection(None),
        }
    }

    /// Watches the pointer for the typing machine: any motion over the
    /// window is [`Window::woken`].
    fn watch_pointer(&self) {
        let motion = gtk::EventControllerMotion::new();
        motion.connect_motion(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |_, _, _| window.woken(),
        ));
        self.add_controller(motion);
    }

    /// Hides the stats bar, or shows it again: `chrome.stats`, the View
    /// menu's Statistics check and the Stats menu's last row.
    pub(crate) fn toggle_stats(&self) {
        self.move_windows(Session::toggle_stats, |window, stats| {
            window.imp().bars.set_stats_shown(stats);
        });
    }

    /// Checks one Statistic, or unchecks it: the Stats menu's six rows.
    ///
    /// The bar rebuilds its cells from the checked set, which is why the
    /// closure takes the whole set rather than the one Statistic that moved:
    /// the cells stand in [`Statistic::ALL`]'s order and not the order they
    /// were checked in. The write is [`Window::move_windows`]', as every
    /// other check's is.
    pub(crate) fn toggle_statistic(&self, statistic: Statistic) {
        self.move_windows(
            |session| {
                session.toggle_statistic(statistic);
                chrome::checked_set(&session.stats().show)
            },
            |window, checked| window.imp().bars.set_statistics(checked),
        );
    }

    /// Opens `menu` under its bar button, its rows reading the modes as they
    /// are now: `chrome.doc`, `chrome.view` (`F10`), a click on the stats
    /// bar, and `--menu`. The bars come back first
    /// ([`Window::bring_bars_back`]).
    pub(crate) fn open_menu(&self, menu: commands::Menu) {
        self.bring_bars_back();
        self.palette().close();
        // Built on every open, so Open Recent lists what has been opened
        // since the last one.
        let recents = self.session().map(|session| session.recents());
        let model = menus::model(menu, &self.modes(), recents.as_deref().unwrap_or_default());
        self.imp().bars.open_menu(menu, &model);
    }

    /// Opens the Palette over the page, or closes it: `palette.open`, which
    /// is `Ctrl+K` and View › Window "All Commands…". A
    /// menu that is up closes first, and the bars come back as they do for a
    /// menu.
    pub(crate) fn open_palette(&self) {
        self.bring_bars_back();
        self.imp().bars.close_menus();
        self.palette().toggle(self.upcast_ref(), self.modes());
    }

    /// `file.recent`: the Palette over the page on this writer's recent
    /// Documents, newest first, Enter opening the highlighted one in this
    /// window (#246, story 41).
    pub(crate) fn open_recents(&self) {
        let Some(session) = self.session() else {
            return;
        };
        self.bring_bars_back();
        self.imp().bars.close_menus();
        self.palette()
            .open_recents(self.upcast_ref(), self.modes(), session.recents());
    }

    /// `outline.open`, `Ctrl+Shift+O`: the Palette over the page on the open
    /// Document's Outline, read off the block index now and held only while
    /// the panel is up (#397). Typing narrows the headings and appends the
    /// Library's Documents by name, the open Document left out; Enter jumps
    /// through [`Window::jump_to`] or opens as a recents row does.
    ///
    /// Never a toggle, for the reason `file.recent` is not one.
    pub(crate) fn open_outline(&self) {
        let (headings, section, open) = {
            let document = self.document();
            let headings = outline::of_blocks(&document);
            let section = outline::section(&document, &headings, self.caret_offset());
            (headings, section, document.path().map(Path::to_path_buf))
        };
        // The Library is read through the sidebar's own view, so the
        // Documents fall in the order the sidebar shows them.
        let session = self.session();
        let view = session
            .as_ref()
            .map(|session| self.imp().sidebar.view(session));
        let finder: crate::palette::Finder = Box::new(move |query| {
            let (Some(session), Some(view)) = (&session, &view) else {
                return Vec::new();
            };
            session
                .library()
                .names(query, view)
                .into_iter()
                .map(|file| file.path().to_path_buf())
                .collect()
        });
        self.bring_bars_back();
        self.imp().bars.close_menus();
        self.palette().open_outline(
            self.upcast_ref(),
            self.modes(),
            headings,
            section,
            open,
            finder,
        );
    }

    /// A jump from the Outline: the caret to `offset`, the first byte of a
    /// heading's words, revealed as a placed caret is — Typewriter's anchor
    /// honoured, Preview following by the caret rule — with no selection
    /// made and the keyboard back on the page. A jump is not typing, so the
    /// bars stay where they are.
    pub(crate) fn jump_to(&self, offset: u64) {
        self.imp()
            .editor
            .place_caret(&self.document(), flags::Caret::At(offset), true);
        self.imp().editor.grab_focus();
    }

    /// Opens the Settings window over this one: `settings.open`, `Ctrl+,` and
    /// View › Window "Settings…".
    pub(crate) fn open_settings(&self) {
        let Some(session) = self.session() else {
            return;
        };
        crate::settings::open(
            self.upcast_ref(),
            &session,
            self.imp().editor.spell_resolution().as_ref(),
        );
    }

    /// Opens the shortcuts window over this one: `shortcuts.open`, `Ctrl+?`
    /// and View › Window "Keyboard Shortcuts".
    ///
    /// The table it lists is built here, from the effective map this launch is
    /// running on, so a `[shortcuts]` edit saved a moment ago is in the window
    /// that opens next.
    pub(crate) fn open_shortcuts(&self) {
        let Some(session) = self.session() else {
            return;
        };
        let shortcuts = session.settings().shortcuts();
        crate::shortcuts::open(
            self.upcast_ref(),
            &quill_engine::shortcuts::sections(&shortcuts.chords),
        );
    }

    /// Puts the typing machine at rest and the bars at full strength, for a
    /// popover about to open over them: the oracle forces both bars to
    /// opacity 1 while a menu or the Palette is up (`chrome.css`,
    /// `[data-menu="on"]`), whatever the timers had left to do.
    fn bring_bars_back(&self) {
        self.imp().typing.set(chrome::typing::Typing::new());
        self.settle();
    }

    /// The Palette, built with the window.
    fn palette(&self) -> &crate::palette::Palette {
        self.imp()
            .palette
            .get()
            .expect("the Palette is built in constructed")
    }

    /// Opens what `--menu` named, once the window has painted its first
    /// frame.
    ///
    /// Held until then rather than opened now: a popup over a toplevel the
    /// compositor has no frame of yet keeps the toplevel from ever mapping,
    /// and the shot never comes. GTK maps the widget as `present` returns,
    /// so `map` is already behind us; the frame clock's first `after-paint`
    /// is the first moment the compositor holds a frame of the window.
    fn open_flagged(&self, menu: flags::Menu) {
        self.imp().flagged.set(Some(menu));
        let Some(clock) = self.frame_clock() else {
            return;
        };
        let window = self.downgrade();
        clock.connect_after_paint(move |_| {
            let Some(window) = window.upgrade() else {
                return;
            };
            match window.imp().flagged.take() {
                Some(flags::Menu::Bar(menu)) => window.open_menu(menu),
                Some(flags::Menu::Palette) => window.open_palette(),
                Some(flags::Menu::Outline) => window.open_outline(),
                None => {}
            }
        });
    }

    /// Opens the Export dialog `--export-dialog` named, its Options expander
    /// open, once the window has painted its first frame.
    ///
    /// Held until then for the reason [`Self::open_flagged`] holds a menu: a
    /// second surface over a toplevel the compositor has no frame of yet keeps
    /// the toplevel from ever mapping, and the shot never comes.
    ///
    /// The format is carried in the callback rather than in the window,
    /// because `after-paint` runs on every frame and this is the first
    /// frame's alone: the [`Cell`] is emptied by the frame that opens the
    /// dialog, so a writer looking at a judged shot is not looking at a stack
    /// of them.
    fn open_flagged_export(&self, format: crate::export_dialog::Format) {
        let Some(clock) = self.frame_clock() else {
            return;
        };
        let window = self.downgrade();
        let asked = std::cell::Cell::new(Some(format));
        clock.connect_after_paint(move |_| {
            let (Some(window), Some(format)) = (window.upgrade(), asked.take()) else {
                return;
            };
            crate::export_dialog::open_expanded(&window, format);
        });
    }

    /// Tells the Editor whether this window has the keyboard, now and after.
    ///
    /// `is-active` is the property GTK keeps the answer in, so it is the one
    /// thing watched: the caret's ghost and the selection's idle colour are
    /// the same state, and a state read from two places is a state that can
    /// disagree with itself.
    ///
    /// Told once here as well as on every change, because a window that is
    /// never given the keyboard never notifies: the `unfocused` judged state
    /// is shot with another surface focused, and its caret has to be a ghost
    /// from the first frame rather than after a change that never comes.
    /// Losing it is also one of the four moments autosave runs at, which is
    /// why the write is here and not beside the Editor's own handlers.
    fn watch_active(&self) {
        self.imp().editor.set_active(self.is_active());
        self.connect_is_active_notify(|window| {
            window.imp().editor.set_active(window.is_active());
            // Focus loss is one of the four moments autosave runs at
            // (`docs/architecture.md` § Documents and files): a writer who
            // turned to another window has stopped typing into this one.
            if !window.is_active() {
                window.autosave();
            }
        });
    }

    /// Keeps the engine's copy of the text in step with the buffer, keystroke
    /// by keystroke, and draws what that changed.
    ///
    /// Three handlers and the order between them is the keystroke path.
    /// `insert-text` and `delete-range` are read **before** GTK's own handler,
    /// because that is the last moment at which the buffer and the Document
    /// still agree on what a byte offset means — the iterators name a place in
    /// the text the Document still has. `changed` is read after, because a tag
    /// is put on by line and byte index within the line, and both have to be
    /// the ones the writer can now see.
    ///
    /// So the splice happens before any Annotator runs, as
    /// `docs/architecture.md` § Text model requires, and the retag happens
    /// after the text has moved. What the splice worked out is carried between
    /// them in [`imp::Window::pending`].
    ///
    /// The caret's two feeds are here for the same reason the splice is: the
    /// Editor holds no Document and every one of them is a judgement about
    /// one. The pointer's release is the fourth and last of them.
    fn watch_edits(&self) {
        let buffer = self.imp().editor.buffer();

        let watcher = self.downgrade();
        buffer.connect_insert_text(move |_, at, text| {
            let Some(window) = watcher.upgrade() else {
                return;
            };
            if window.imp().editor.loading() {
                return;
            }
            let mut filed = window.imp().filed.borrow_mut();
            let document = filed.document_mut();
            let offset = tags::offset_of(document, at);
            let edit = document.insert(offset, text);
            window.imp().pending.replace(Some(edit));
        });

        let watcher = self.downgrade();
        buffer.connect_delete_range(move |_, from, to| {
            let Some(window) = watcher.upgrade() else {
                return;
            };
            if window.imp().editor.loading() {
                return;
            }
            let mut filed = window.imp().filed.borrow_mut();
            let document = filed.document_mut();
            let at = tags::offset_of(document, from)..tags::offset_of(document, to);
            let edit = document.delete(at);
            window.imp().pending.replace(Some(edit));
        });

        let watcher = self.downgrade();
        buffer.connect_changed(move |_| {
            let Some(window) = watcher.upgrade() else {
                return;
            };
            if window.imp().editor.loading() {
                return;
            }
            // Nothing pending is a change the Document was not spliced for:
            // filling the buffer with a Document it already holds.
            let Some(edit) = window.imp().pending.take() else {
                return;
            };
            let document = window.document();
            window.imp().editor.retag(&document, &edit);
            drop(document);
            window.arm_syntax();
            // A keystroke, and only a keystroke: a load is skipped above and
            // a switch fills the buffer with nothing pending.
            window.typed();
        });

        // Focus's own feed, and the Document is why it is here rather than
        // beside the Editor's other caret handlers: the Editor holds no
        // Document, and the tiers are read off one. A move the writer made
        // with a key, a click or a shift-drag all arrive as `mark-set`; the
        // ones an edit made arrive above, where the retag they share is
        // already being paid for.
        let watcher = self.downgrade();
        buffer.connect_mark_set(move |buffer, _, mark| {
            let Some(window) = watcher.upgrade() else {
                return;
            };
            if window.imp().editor.loading() {
                return;
            }
            // The other end of a selection moves on its own through a
            // shift-drag, and a selection is the bright span.
            if mark != &buffer.get_insert() && mark != &buffer.selection_bound() {
                return;
            }
            // The stats bar's own feed. Armed rather than counted here: a
            // drag moves the bound on every motion event and the count is
            // O(run), so the frame is what it is paid once per.
            window.arm_selection_count();
            // `changed` fires before this for an edit, and its retag has
            // already moved the dim: taking the Document here would be a
            // second borrow of one the splice may still hold.
            let Ok(filed) = window.imp().filed.try_borrow() else {
                return;
            };
            window.imp().editor.refocus(filed.document());
            // And the caret rule: a caret leaving a word it was typing gives
            // the word its wave.
            window.imp().editor.rewithhold(filed.document());
            drop(filed);
            // A caret move takes the caret rule, as an edit does: the block
            // being written is what the rendered page is kept on (#263
            // § Scroll sync).
            window.follow_caret();
        });

        // Live's fold waits for the button to come up ([`Editor::released`]),
        // and this is where the Editor is told that it has. A legacy
        // controller rather than a `GtkGestureClick`: a press on a task box
        // claims its sequence and GTK's own selection drag claims one too, a
        // claimed sequence denies every other gesture on the widget, and a
        // denied gesture is never told about the release — which would leave
        // the fold held down for good. A legacy controller is not a gesture
        // and is never denied. It runs in the capture phase and takes nothing:
        // the press is still GTK's to turn into a caret move.
        let releases = gtk::EventControllerLegacy::new();
        releases.set_propagation_phase(gtk::PropagationPhase::Capture);
        let watcher = self.downgrade();
        releases.connect_event(move |_, event| {
            let Some(window) = watcher.upgrade() else {
                return glib::Propagation::Proceed;
            };
            if event.event_type() == gdk::EventType::ButtonRelease {
                // As the `mark-set` handler has it: a splice may still hold
                // the Document, and this borrow is not worth waiting for.
                if let Ok(filed) = window.imp().filed.try_borrow() {
                    window.imp().editor.released(filed.document());
                }
            }
            glib::Propagation::Proceed
        });
        self.imp().editor.add_controller(releases);
    }

    /// Puts Spell check's corrections on the Editor: the `spell` action group
    /// and the two moments the section is built at ([`crate::corrections`]).
    ///
    /// Here rather than in the Editor because both halves want the Document,
    /// and Add and Ignore want the wake that drains the answer. Both
    /// controllers run in the capture phase and take nothing: the press is
    /// still GTK's to open its menu with, and the chord still GTK's binding.
    fn watch_corrections(&self) {
        let replace = self.downgrade();
        let add = self.downgrade();
        let ignore = self.downgrade();
        let actions = corrections::actions(
            move |suggestion| {
                if let Some(window) = replace.upgrade() {
                    window.imp().editor.replace(suggestion);
                }
            },
            move |word| {
                if let Some(window) = add.upgrade() {
                    window.edit_dictionary(quill_engine::worker::Edit::Add(word.to_owned()));
                }
            },
            move |word| {
                if let Some(window) = ignore.upgrade() {
                    window.edit_dictionary(quill_engine::worker::Edit::Ignore(word.to_owned()));
                }
            },
        );
        self.imp()
            .editor
            .insert_action_group(corrections::GROUP, Some(&actions));

        let presses = gtk::GestureClick::new();
        presses.set_button(gdk::BUTTON_SECONDARY);
        presses.set_propagation_phase(gtk::PropagationPhase::Capture);
        let watcher = self.downgrade();
        presses.connect_pressed(move |_, _, x, y| {
            let Some(window) = watcher.upgrade() else {
                return;
            };
            if let Ok(filed) = window.imp().filed.try_borrow() {
                window.imp().editor.correct(filed.document(), Some((x, y)));
            }
        });
        self.imp().editor.add_controller(presses);

        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        let watcher = self.downgrade();
        keys.connect_key_pressed(move |_, key, _, state| {
            let menu = key == gdk::Key::Menu
                || (key == gdk::Key::F10 && state.contains(gdk::ModifierType::SHIFT_MASK));
            if let Some(window) = watcher.upgrade().filter(|_| menu)
                && let Ok(filed) = window.imp().filed.try_borrow()
            {
                window.imp().editor.correct(filed.document(), None);
            }
            glib::Propagation::Proceed
        });
        self.imp().editor.add_controller(keys);
    }

    /// Sends an Add or an Ignore and wakes the drain for the Document it
    /// asks for again.
    fn edit_dictionary(&self, edit: quill_engine::worker::Edit) {
        if let Ok(filed) = self.imp().filed.try_borrow() {
            self.imp().editor.edit_dictionary(edit, filed.document());
        }
        self.rearm();
    }
}

/// What a Save As dialog leaves behind once the file is written.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum After {
    /// The window stays: `file.saveAs` on its own.
    Stay,
    /// The window closes: the Save button of the prompt a close asked.
    Close,
    /// The Document is exported: `export.quick` on an untitled Document, which
    /// asks for a file before it has anything to stand a PDF beside
    /// ([`Window::save_before_export`]).
    Export,
}

/// What the Preview pane was showing when an Export dialog opened over it,
/// and so what closing the dialog puts back (#293 § The dialogs drive the
/// pane).
///
/// Whether the pane was open at all is the window's own, as it always is, and
/// it is the whole of this: the Split a dialog stands a hidden pane in is the
/// window's too and goes with the dialog ([`imp::Window::dialog_split`]), so
/// the session's layout was never moved and has nothing to put back. The mode
/// and the geometry are not here either: those are the pane's override, dropped
/// whole ([`crate::preview::DialogOverride`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct DialogPreviewBefore {
    /// Whether this window's pane was open before the dialog opened.
    open: bool,
}

/// Whether a Preview-driving Export dialog stands over the window, which is
/// the one thing the Editor's scrim depends on beyond the pane's own layout.
///
/// Display-free so the dialog's open, layout, close and reopen transitions are
/// held by one lifecycle test. [`DialogPreviewState::apply`] answers whether
/// the scrim shows and the window puts that answer on the widget; nothing here
/// reaches the settings file.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct DialogPreviewState {
    /// Whether a PDF or HTML Export dialog currently drives the pane.
    open: bool,
}

impl DialogPreviewState {
    /// Starts one dialog's Preview override.
    fn open(&mut self) {
        self.open = true;
    }

    /// Whether the scrim covers the Editor under the pane's current
    /// visibility and layout: while a dialog stands and the Editor half is on
    /// screen beside a Split pane, and never in Full or with the pane away.
    fn apply(&self, previewing: bool, split: bool) -> bool {
        self.open && previewing && split
    }

    /// Ends the dialog, and with it the scrim.
    fn close(&mut self) {
        self.open = false;
    }
}

/// What the Preview pane comes back to when an Export dialog closes over it.
///
/// Built by [`dropped_preview`] and applied by
/// [`Window::drop_dialog_preview`]; a struct rather than a pair because the
/// pane is put back in one move.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct DroppedPreview {
    /// Whether the pane stays open: it does where it was open before the
    /// dialog, and goes away where the dialog is what opened it.
    open: bool,
    /// The settings the pane lays its pages out at.
    settings: Settings,
}

/// Where the Preview pane is left when an Export dialog closes: `before`'s pane,
/// laid out at what was saved from the dialog, or at `running` where nothing
/// was.
///
/// **Save as defaults** writes `[export]` before the close ever runs, and it
/// writes it to the file: the running settings carry it only once the watch has
/// read the file back, which is after this. So the settings that write composed
/// are what the pane goes back to, and the pane lands on what was saved in one
/// step rather than on the old table and then on the saved one (#293 § The
/// dialogs drive the pane).
///
/// Display-free, so that both cases are one headless test.
pub(crate) fn dropped_preview(
    before: DialogPreviewBefore,
    running: &Settings,
    saved: Option<Settings>,
) -> DroppedPreview {
    DroppedPreview {
        open: before.open,
        settings: saved.unwrap_or_else(|| running.clone()),
    }
}

/// Where the Preview pane stands once a Preview chord has been pressed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PreviewPane {
    /// Away, with the Editor holding the window on its own.
    Closed,
    /// On screen, in this layout.
    Open(PreviewLayout),
}

/// Where `asked`'s chord leaves the pane, given the one `showing` now.
///
/// Each chord owns a layout and is that layout's toggle: it closes the pane
/// it is already looking at and otherwise shows its own, whether that means
/// opening the pane or switching it in place. `showing` is read only while
/// the pane is open — closed, the layout is where the pane last stood
/// ([`crate::session::Session::preview_layout`]) and says nothing about what
/// the chord should do.
pub(crate) fn next_preview_pane(
    previewing: bool,
    showing: PreviewLayout,
    asked: PreviewLayout,
) -> PreviewPane {
    if previewing && showing == asked {
        PreviewPane::Closed
    } else {
        PreviewPane::Open(asked)
    }
}

/// Which way Bigger Preview Text, Smaller Preview Text and Default Preview
/// Size move the rendered page's zoom.
#[derive(Clone, Copy)]
pub(crate) enum Zoom {
    /// [`ZOOM_STEP`] points larger.
    Bigger,
    /// [`ZOOM_STEP`] points smaller.
    Smaller,
    /// Back to the Template's own sizes.
    Reset,
}

/// The zoom one press of a Preview size key leaves, as a whole percentage.
///
/// A step of [`ZOOM_STEP`] points inside the range the settings file is read
/// against ([`quill_engine::settings::preview_zooms`]), and a reset to the
/// `[preview]` table's own default rather than to a number written twice.
pub(crate) fn stepped_zoom(now: u32, direction: Zoom) -> u32 {
    let range = quill_engine::settings::preview_zooms();
    let wanted = match direction {
        Zoom::Bigger => now.saturating_add(ZOOM_STEP),
        Zoom::Smaller => now.saturating_sub(ZOOM_STEP),
        Zoom::Reset => quill_engine::settings::Preview::default().zoom,
    };
    wanted.clamp(*range.start(), *range.end())
}

/// Which way Bigger Text, Smaller Text and Default Text Size move.
#[derive(Clone, Copy)]
pub(crate) enum Step {
    /// One rung up the ladder.
    Bigger,
    /// One rung down.
    Smaller,
    /// Back to the default rung.
    Default,
}

/// Puts what the session now says on to every open window, and `each` window
/// on top of that.
///
/// The stylesheet first and once, because it belongs to the display rather
/// than to a window: the type and the ground are both named in it, so either
/// moving reloads it, and reloading it names both whichever one moved. Then
/// `each` window, because everything else — the leading, the measure, the tags
/// — is laid out per widget.
///
/// The session is asked rather than told, so that there is one answer to what
/// this launch is running: the caller has already moved it, and a second copy
/// passed alongside is a second thing that can be stale. A writer has one pair
/// of eyes, so a change in one window is a change in all of them.
///
/// The ground is asked for once here and handed to `each` window, so the
/// stylesheet and every widget in the pass are painted from the one table
/// ([`Session::ground`]).
fn reset(app: &gtk::Application, session: &Session, each: impl Fn(&Window, Ground)) {
    let ground = session.ground();
    crate::editor::install_type(ground, session.face(), session.step());
    for window in app.windows() {
        if let Ok(window) = window.downcast::<Window>() {
            each(&window, ground);
        }
    }
}

/// Puts a ground on to every open window, stylesheet and tags together.
///
/// The one pass the switch is: [`reset`] reloads the stylesheet the paper and
/// the chrome are named in, and each Editor re-resolves its tag table from the
/// palette, so a frame is never composed half on one ground and half on the
/// other. Both of the things that can move the ground — a writer's
/// `Ctrl+Shift+L` and a desktop the writer asked Quill to follow — arrive
/// here, because a writer has one pair of eyes and there is one way to repaint
/// what they are looking at.
///
/// The caller has already moved the session, and the ground is read back off
/// it rather than passed alongside, because the table is the session's to
/// choose ([`Session::ground`]) and a scheme handed in beside it would be a
/// second answer to the same question.
pub fn repaint(app: &gtk::Application, session: &Session) {
    reset(app, session, |window, ground| {
        let document = window.document();
        window.imp().editor.set_ground(ground, &document);
        window.imp().bars.set_ground(ground);
        // The rendered page is on the Template's paper and not the Editor's,
        // but which of the Template's two palettes it is on follows the
        // ground: a dark Editor is a dark page beside it.
        drop(document);
        window.refresh_preview();
    });
    chrome::reflect_windows(app);
}

/// The `spell_check` and `spell_language` settings as the session holds them:
/// the check live, as the Commands leave it, and the language from the file.
fn spelling(session: &Session) -> (bool, String) {
    (session.spell(), session.settings().spell_language.clone())
}

/// Puts a settings file saved while Quill is running on to every window.
///
/// The whole of what the file carries at once — the ground, the type, Focus,
/// Typewriter and the bars — rather than only what moved, because a file is
/// saved whole and the session has already been moved by it
/// ([`Session::apply`]): a value the writer left alone is set to what it
/// already held, and the writer sees one repaint rather than five.
///
/// The keys' own paths ([`repaint`], [`Window::step_size`]) stay as they are:
/// they move one thing and write the file, and this is the other direction,
/// the file moving everything.
pub fn reapply(app: &gtk::Application, session: &Session) {
    let focus = session.focus();
    let typewriter = session.typewriter();
    let live = session.live();
    let face = session.face();
    let step = session.step();
    let bars = session.chrome() == Chrome::Shown;
    reset(app, session, move |window, ground| {
        let document = window.document();
        window.imp().editor.set_type(face, step);
        window.imp().editor.set_ground(ground, &document);
        window.imp().editor.set_focus(focus, &document);
        window.imp().editor.set_typewriter(typewriter);
        window.imp().editor.set_live(live, &document);
        window.imp().bars.set_ground(ground);
        window.imp().bars.set_focus(focus);
        window.imp().bars.set_shown(bars);
        // A `[template]` or a `[preview]` key saved while Quill is running is
        // a page laid out again, and where the pane opens is a pane to stand
        // again (#271's Settings rows arrive this way).
        drop(document);
        window.apply_preview();
        window.refresh_preview();
        window.set_syntax(session.syntax().clone());
        window.set_style(session.style().clone());
        let (spell, language) = spelling(session);
        window.set_spell(spell, &language);
    });
    // The sidebar reads the `[library]` settings as it lists — hidden files,
    // extensions — and the Library itself has already been made to say what
    // the file says ([`Session::apply`]), so a settings save that moved either
    // is one re-listing (#246).
    relist(app);
    chrome::reflect_windows(app);
}

/// Opens the windows this launch asks for.
///
/// The Documents its flags name, or one untitled Document when they name none.
/// `--measure` hangs its cold start and its per-key capture on the first of
/// them, because the first window to be presented is the one whose first frame
/// is the launch's and the one a bench will type into. `--caret` moves the
/// caret after the Document is shown, since the offset it names is an offset
/// into that Document. This is the only path a launch of the harness's takes:
/// such a launch is handed no files by GTK, so [`present_files`] below is a
/// writer's alone.
pub fn present_launch(app: &gtk::Application, session: &Rc<Session>) {
    let documents = session.flags().documents();
    let mut first = None;
    if documents.is_empty() {
        first = Some(present(app, Filed::untitled(), session));
    }
    for path in documents {
        match Filed::open(path) {
            Ok(filed) => {
                let window = present(app, filed, session);
                if first.is_none() {
                    first = Some(window);
                }
            }
            Err(err) => eprintln!("quill: cannot open {}: {err}", path.display()),
        }
    }
    // After the Document is shown rather than with it: the offset `--caret`
    // names is an offset into that Document, and there is nothing to count
    // until it is in the buffer. `--scroll` comes second and wins, because a
    // state that names where the view is means it however the caret got there.
    if let Some(window) = &first {
        let scroll = session.flags().scroll;
        if let Some(caret) = session.flags().caret {
            window
                .imp()
                .editor
                .place_caret(&window.document(), caret, scroll.is_none());
        }
        // After `--caret`, because placing the cursor collapses a selection to
        // it: a state naming both means the selection, with the caret at the
        // end `--select` leaves the insert mark on.
        if let Some((from, to)) = session.flags().select {
            window.imp().editor.select(&window.document(), from, to);
        }
        if let Some(scroll) = scroll {
            window.imp().editor.scroll_to(scroll);
        }
        // Last, over a page that is already where the state put it, because
        // a popover is placed against its button and the bars are laid out
        // with the page.
        if let Some(menu) = session.flags().menu {
            window.open_flagged(menu);
        }
        // The Export dialog last of all, and for the same reason a menu is
        // held: it is a second surface over a window that has to be mapped
        // and laid out first, and its own seeded file name is read off the
        // Document that was shown above.
        if let Some(format) = session.flags().export_dialog {
            window.open_flagged_export(format);
        }
    }
    if let Some(window) = first
        && session.flags().measure.is_some()
    {
        harness::cold_start(&window);
        harness::watch(&window);
    }
}

/// Opens one window on `filed`, and hands it back.
fn present(app: &gtk::Application, filed: Filed, session: &Rc<Session>) -> Window {
    let window = Window::new(app, filed, session);
    window.present();
    window
}

/// `window.new`: an empty window, with an untitled Document in it.
pub fn open_new(app: &gtk::Application, session: &Rc<Session>) {
    present(app, Filed::untitled(), session);
}

/// Opens one window per file of an open request.
///
/// A file that cannot be read is reported and skipped, so the other files still
/// get their windows. Reporting is stderr until the Library spec decides what a
/// writer sees; the point today is that nothing fails silently.
pub fn present_files(app: &gtk::Application, files: &[gio::File], session: &Rc<Session>) {
    for file in files {
        let Some(path) = file.path() else {
            // A `gio::File` with no local path: a URI Quill cannot read as a
            // file. Saying so beats opening a window on nothing.
            eprintln!("quill: not a local file: {}", file.uri());
            continue;
        };
        match Filed::open(&path) {
            Ok(filed) => {
                present(app, filed, session);
            }
            Err(err) => eprintln!("quill: cannot open {}: {err}", path.display()),
        }
    }
}

/// Every window of this application that is one of Quill's.
///
/// The one walk every pass over the windows shares — a relist, a flush, a
/// quit — because `gtk::Application` answers its windows as GTK windows and
/// each pass wants Quill's own.
fn windows(app: &gtk::Application) -> Vec<Window> {
    app.windows()
        .into_iter()
        .filter_map(|window| window.downcast::<Window>().ok())
        .collect()
}

/// Tells the window showing `path` that something happened to its file.
///
/// Every window is asked, because two of them can show the same Document, and
/// a window showing something else is one comparison and no work.
pub fn heard(app: &gtk::Application, path: &Path) {
    for window in windows(app) {
        if window.path().as_deref() == Some(path) {
            window.heard();
        }
    }
}

/// Follows every open Document whose file was renamed outside Quill, given
/// every path one drain of the watch answered for.
///
/// A rename arrives as the path the file left and the path it took, in the one
/// drain ([`quill_engine::disk::Filed::followed`]), so this runs before the
/// paths are answered one by one: a Document that followed is then a Document
/// whose file is where it says, and the drain's own event for it finds nothing
/// changed rather than a file that is gone (#246, story 29).
pub fn followed(app: &gtk::Application, batch: &[PathBuf]) {
    if batch.is_empty() {
        return;
    }
    for window in windows(app) {
        let moved = window.imp().filed.borrow_mut().followed(batch);
        if moved {
            window.shown();
            if let (Some(path), Some(session)) = (window.path(), window.session()) {
                window.imp().sidebar.set_open(Some(&path));
                session.watch_document(&path);
            }
        }
    }
}

/// Draws the Library again in every window that is showing it.
///
/// The Library is the application's and the pane is the window's, so a tree
/// the watch has patched is one pass over the windows, the way a ground change
/// is: every sidebar reads the same tree and each redraws its own rows.
pub fn relist(app: &gtk::Application) {
    for window in windows(app) {
        window.imp().sidebar.refresh();
    }
}

/// Writes out every window's Document that can be written without asking.
///
/// The last of autosave's four moments: a Quill going down — `Ctrl+Q` having
/// closed its windows, or the desktop ending the session — leaves the file
/// holding the last keystroke.
pub fn flush_open(app: &gtk::Application) {
    for window in windows(app) {
        window.flush();
    }
}

/// Quit: every window is asked as if the writer had closed it, and Cancel
/// anywhere cancels the quit.
///
/// The questions come first and the closing after: the windows with something
/// to ask ask it one at a time, and every window closes only once the last of
/// them has been answered. A Cancel ends the walk with nothing closed, which
/// is what makes it a cancelled quit rather than a quit that stopped halfway
/// through and left the writer with the windows it had not reached yet.
/// `app.quit()` would destroy them all without asking any of them.
pub fn quit(app: &gtk::Application) {
    let mut asking: Vec<Window> = windows(app).into_iter().filter(Window::would_ask).collect();
    // Asked off the end, so the writer answers for the windows in the order
    // the application holds them.
    asking.reverse();
    ask_to_quit(app, asking);
}

/// Asks the windows of `asking` in turn, and closes every window once the last
/// of them has answered.
///
/// A window that answered Save or Discard is marked as having answered, so
/// that its own close does not ask again; one whose Save had nowhere to go is
/// in the Save As dialog, and the quit ends there, since the writer is being
/// asked something already.
fn ask_to_quit(app: &gtk::Application, asking: Vec<Window>) {
    let mut rest = asking;
    let Some(window) = rest.pop() else {
        for window in windows(app) {
            window.close();
        }
        return;
    };
    let app = app.clone();
    window.ask_before_leaving(move |window| {
        window.imp().answered.set(true);
        ask_to_quit(&app, rest.clone());
    });
}

/// Takes down the shape of every window still open.
///
/// Quitting outright — `Ctrl+Q`, or the desktop closing the session — destroys
/// windows without asking them to close, so shutdown asks the ones that are
/// left. A window that closed on its own is already gone from this list and is
/// remembered once.
pub fn remember_open(app: &gtk::Application) {
    for window in windows(app) {
        window.remember();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A page of four blocks at `scale` times the heights the other pane lays
    /// them out at, the third of which the rendered page will lack — a
    /// Document's `Gap`, which the render pass drops.
    fn page(scale: f64) -> Vec<sync::Block> {
        [
            (0, 0.0, 100.0),
            (1, 100.0, 200.0),
            (2, 300.0, 60.0),
            (3, 360.0, 240.0),
        ]
        .into_iter()
        .map(|(key, top, height)| sync::Block::new(key, top * scale, height * scale))
        .collect()
    }

    /// What [`Window::follow_editor`] hands the rule is one block and not the
    /// Editor's whole index, and the rule answers the same offset either way.
    ///
    /// The whole reason the Editor is asked for a block by
    /// [`crate::editor::Editor::top_block`] rather than walked: the top-block
    /// rule reads the driver only for whichever block its top edge is in, so a
    /// slice of that one block is the same question asked cheaply, and a
    /// keystroke never pays for a manuscript's index.
    #[test]
    fn one_block_is_the_whole_of_what_the_top_block_rule_asks_of_a_driver() {
        let driver = page(1.0);
        let follower = page(2.0);
        let max = 2000.0;
        for offset in [0.0, 50.0, 100.0, 355.0, 500.0] {
            let whole = sync::follow_top_block(&driver, offset, &follower, max);
            let one = driver
                .iter()
                .rev()
                .find(|block| block.top <= offset)
                .copied()
                .expect("the top edge is in a block");
            assert!(
                (sync::follow_top_block(&[one], offset, &follower, max) - whole).abs()
                    < f64::EPSILON,
                "at {offset} the one block the top edge is in answers what the page does"
            );
        }
    }

    /// The stepping the three Preview size keys press, on its own: ten points
    /// a press, held inside the range the settings file is read against, and a
    /// reset to the Template's own sizes (#263 § Zoom).
    #[test]
    fn the_preview_zoom_steps_by_ten_points_and_stops_at_the_ends() {
        assert_eq!(stepped_zoom(100, Zoom::Bigger), 110);
        assert_eq!(stepped_zoom(100, Zoom::Smaller), 90);
        assert_eq!(stepped_zoom(195, Zoom::Bigger), 200, "clamped, not refused");
        assert_eq!(stepped_zoom(200, Zoom::Bigger), 200);
        assert_eq!(stepped_zoom(55, Zoom::Smaller), 50);
        assert_eq!(stepped_zoom(50, Zoom::Smaller), 50);
        assert_eq!(stepped_zoom(0, Zoom::Smaller), 50, "and never below zero");
        assert_eq!(stepped_zoom(175, Zoom::Reset), 100);
        assert_eq!(stepped_zoom(50, Zoom::Reset), 100);
    }

    /// With the pane away, either chord opens it in its own layout — the
    /// layout the pane last stood in does not steer it (#263, the third Hand
    /// test round).
    #[test]
    fn a_chord_with_no_pane_opens_its_own_layout() {
        for showing in [PreviewLayout::Split, PreviewLayout::Full] {
            assert_eq!(
                next_preview_pane(false, showing, PreviewLayout::Full),
                PreviewPane::Open(PreviewLayout::Full)
            );
            assert_eq!(
                next_preview_pane(false, showing, PreviewLayout::Split),
                PreviewPane::Open(PreviewLayout::Split)
            );
        }
    }

    /// Over the other layout a chord switches the open pane rather than
    /// closing it: #263 story 4's "without hiding the pane first".
    #[test]
    fn a_chord_over_the_other_layout_switches_in_place() {
        assert_eq!(
            next_preview_pane(true, PreviewLayout::Split, PreviewLayout::Full),
            PreviewPane::Open(PreviewLayout::Full)
        );
        assert_eq!(
            next_preview_pane(true, PreviewLayout::Full, PreviewLayout::Split),
            PreviewPane::Open(PreviewLayout::Split)
        );
    }

    /// Over its own layout a chord is the way back out.
    #[test]
    fn a_chord_over_its_own_layout_closes_the_pane() {
        assert_eq!(
            next_preview_pane(true, PreviewLayout::Full, PreviewLayout::Full),
            PreviewPane::Closed
        );
        assert_eq!(
            next_preview_pane(true, PreviewLayout::Split, PreviewLayout::Split),
            PreviewPane::Closed
        );
    }

    /// The close of an Export dialog leaves the pane as it found it, laid out
    /// at the `[export]` table that stands once the dialog has gone: the one
    /// **Save as defaults** wrote where it was pressed, and the running one
    /// where it was not.
    ///
    /// The written table and not the running one, because the write reaches
    /// [`Session::running`] only when the watch reads the file back, which is
    /// after the close (#293 § The dialogs drive the pane).
    #[test]
    fn the_drop_lays_the_pane_out_at_what_save_as_defaults_wrote_and_at_the_table_otherwise() {
        use quill_engine::settings::Paper;

        // The table the dialog opened over: what the writer's file holds.
        let mut running = Settings::default();
        running.export.paper = Paper::Legal;
        running.export.text_size = 12;

        // Nothing saved: the pane goes back to that table, and stays open
        // because it was open before the dialog.
        let over_an_open_pane = DialogPreviewBefore { open: true };
        let dropped = dropped_preview(over_an_open_pane, &running, None);
        assert!(dropped.open, "a pane the dialog found open stays open");
        assert_eq!(
            dropped.settings.export, running.export,
            "the Options' values are dropped and the file's table is what is left"
        );

        // Save as defaults pressed: the pane goes back to what it wrote, in
        // one step, though the running settings are still the old table.
        let mut saved = running.clone();
        saved.export.paper = Paper::Letter;
        saved.export.text_size = 14;
        let dropped = dropped_preview(over_an_open_pane, &running, Some(saved.clone()));
        assert_eq!(
            dropped.settings.export, saved.export,
            "what was saved is what the pane comes back to"
        );
        assert_ne!(
            dropped.settings.export, running.export,
            "and not the table the running settings still hold"
        );

        // A pane the dialog itself opened goes away again, whatever was saved.
        let dropped = dropped_preview(DialogPreviewBefore { open: false }, &running, Some(saved));
        assert!(
            !dropped.open,
            "a pane the dialog opened is hidden again by the close"
        );
    }

    #[test]
    fn a_dialog_dims_only_the_editor_in_split_through_close_and_reopen() {
        let mut dialog = DialogPreviewState::default();
        assert!(
            !dialog.apply(false, true),
            "no dialog leaves the Editor clear"
        );

        dialog.open();
        assert!(dialog.apply(true, true), "Split dims the Editor");
        assert!(!dialog.apply(true, false), "Full leaves the Editor clear");
        assert!(
            dialog.apply(true, true),
            "returning to Split restores the dim"
        );

        dialog.close();
        assert!(!dialog.apply(true, true), "every close takes the dim down");

        dialog.open();
        assert!(
            dialog.apply(true, true),
            "a later dialog can dim Split again"
        );
        dialog.close();
        assert!(
            !dialog.apply(false, true),
            "closing over a hidden pane stays clear"
        );
    }
}
