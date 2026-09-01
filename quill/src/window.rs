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

use std::rc::Rc;

use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gio, glib};
use quill_engine::document::Document;
use quill_engine::focus::Focus;
use quill_engine::settings::WindowState;
use quill_engine::theme::Scheme;

use crate::caret;
use crate::harness;
use crate::session::Session;
use crate::tags;

mod imp {
    use std::cell::RefCell;
    use std::rc::Rc;

    use gtk::prelude::*;
    use gtk::subclass::prelude::*;
    use gtk::{ScrolledWindow, glib};
    use quill_engine::document::{Document, Edit};

    use crate::editor::Editor;
    use crate::session::Session;

    #[derive(Default)]
    pub struct Window {
        /// The one Document this window shows.
        pub document: RefCell<Document>,
        /// The settings and state this window was opened from and will be
        /// remembered in.
        pub session: RefCell<Option<Rc<Session>>>,
        pub editor: Editor,
        /// What the edit now going through the buffer changed, left here by
        /// the handler that spliced the Document for the one that retags.
        pub pending: RefCell<Option<Edit>>,
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
            window.set_child(Some(&scroller));
        }
    }

    impl WidgetImpl for Window {}
    impl WindowImpl for Window {}
    impl ApplicationWindowImpl for Window {}
}

glib::wrapper! {
    /// A window, and the Document it shows.
    pub struct Window(ObjectSubclass<imp::Window>)
        @extends gtk::ApplicationWindow, gtk::Window, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Native,
                    gtk::Root, gtk::ShortcutManager, gio::ActionGroup, gio::ActionMap;
}

impl Window {
    /// A window of `app` showing `document`, in the shape `session` remembers.
    fn new(app: &gtk::Application, document: Document, session: &Rc<Session>) -> Self {
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
        window.imp().editor.open_on(session.scheme());
        // And with it, because `--focus` names a state the first frame is
        // meant to show: the tiers are worked out inside the same draw that
        // puts the Document on the page.
        window.imp().editor.open_focused_on(session.focus());
        window.set_document(document);
        window
            .imp()
            .editor
            .set_type(session.settings().face, session.step());
        window.install_commands();
        window.imp().editor.grab_focus();
        window.watch_active();
        // A window is remembered as it closes rather than at shutdown, so that
        // the last window a writer sized is the first one the next launch
        // reads, whichever of its windows they closed first.
        window.connect_close_request(|window| {
            window.remember();
            glib::Propagation::Proceed
        });
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

    /// Takes this window's shape down for the next launch.
    fn remember(&self) {
        // Set the moment the window is built, so this is every window; the
        // `Option` is there because a `GObject` is constructed before anyone
        // can hand it anything.
        if let Some(session) = self.imp().session.borrow().as_ref() {
            session.remember(self.shape(session.opening()));
        }
    }

    /// Installs Bigger Text, Smaller Text, Default Text Size and Dark Mode.
    ///
    /// On the window rather than on the application because that is what
    /// `docs/shortcuts.md` names them: `font.bigger` with `win.` in front. What
    /// they move is the session's, though — the size and the ground both — so
    /// moving either in one window moves it in every window: a writer has one
    /// pair of eyes.
    fn install_commands(&self) {
        self.add_action_entries([
            gio::ActionEntry::builder("font.bigger")
                .activate(|window: &Self, _, _| window.step_size(Step::Bigger))
                .build(),
            gio::ActionEntry::builder("font.smaller")
                .activate(|window: &Self, _, _| window.step_size(Step::Smaller))
                .build(),
            gio::ActionEntry::builder("font.reset")
                .activate(|window: &Self, _, _| window.step_size(Step::Default))
                .build(),
            gio::ActionEntry::builder("theme.toggle")
                .activate(|window: &Self, _, _| window.toggle_scheme())
                .build(),
            gio::ActionEntry::builder("focus.toggle")
                .activate(|window: &Self, _, _| window.toggle_focus())
                .build(),
            gio::ActionEntry::builder("focus.swap")
                .activate(|window: &Self, _, _| window.swap_focus_scope())
                .build(),
            gio::ActionEntry::builder("typewriter.toggle")
                .activate(|window: &Self, _, _| window.toggle_typewriter())
                .build(),
        ]);
    }

    /// Steps the type size one rung of the ladder, or back to the default
    /// one.
    ///
    /// The step stops at the ends of [`quill_engine::settings::type_steps`]
    /// rather than wrapping or refusing: a writer holding the key down means
    /// "as big as it goes", and it is the same range `step` in the file and
    /// `--step` on the command line are held to, because it is the same
    /// question asked three ways.
    fn step_size(&self, direction: Step) {
        let Some(session) = self.imp().session.borrow().clone() else {
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
        let face = session.settings().face;
        if let Some(app) = self.application() {
            reset(&app, &session, |window| {
                window.imp().editor.set_type(face, step);
            });
        }
    }

    /// Toggles the ground, for this launch and for the next.
    ///
    /// A working binding until #119 moves the Commands into the registry
    /// `docs/shortcuts.md` describes; the accelerator is that table's
    /// `theme.toggle` row, `Ctrl+Shift+L`. The session decides which ground
    /// the toggle lands on — it is the one holding what `auto` resolved to —
    /// and writes the setting on the way out.
    fn toggle_scheme(&self) {
        let Some(session) = self.imp().session.borrow().clone() else {
            return;
        };
        let scheme = session.toggle_scheme();
        if let Some(app) = self.application() {
            repaint(&app, &session, scheme);
        }
    }

    /// Switches Focus off, or back on at the scope it left.
    ///
    /// A working binding until #119 moves the Commands into the registry
    /// `docs/shortcuts.md` describes; the accelerator is that table's
    /// `focus.toggle` row, `Ctrl+D`. The session holds which scope that is and
    /// writes it on the way out, as it does the ground.
    fn toggle_focus(&self) {
        self.refocus_windows(|session| session.toggle_focus());
    }

    /// Swaps Sentence and Paragraph, switching Focus on if it was off.
    ///
    /// `docs/shortcuts.md`'s `focus.swap` row, `Ctrl+Shift+D`.
    fn swap_focus_scope(&self) {
        self.refocus_windows(Session::swap_focus_scope);
    }

    /// Turns Typewriter on or off.
    ///
    /// `docs/shortcuts.md`'s `typewriter.toggle` row, `Ctrl+T`. Nothing is
    /// redrawn and nothing scrolls: the key sets the value the session
    /// remembers, and #115 is what makes the caret's line move to it.
    fn toggle_typewriter(&self) {
        let Some(session) = self.imp().session.borrow().clone() else {
            return;
        };
        session.toggle_typewriter();
    }

    /// Moves Focus the way `move_it` says, and puts the answer on every window.
    ///
    /// The two Focus keys differ only in what they ask the session for, so what
    /// they do with the answer is written once: a writer has one pair of eyes,
    /// and Focus moving in one window moves it in all of them, as the ground
    /// and the type size do.
    fn refocus_windows(&self, move_it: impl Fn(&Session) -> Focus) {
        let Some(session) = self.imp().session.borrow().clone() else {
            return;
        };
        let focus = move_it(&session);
        let Some(app) = self.application() else {
            return;
        };
        for window in app.windows() {
            let Ok(window) = window.downcast::<Window>() else {
                continue;
            };
            let document = window.imp().document.borrow();
            window.imp().editor.set_focus(focus, &document);
        }
    }

    /// Takes `document` as this window's own and shows it.
    fn set_document(&self, document: Document) {
        self.imp().document.replace(document);
        let document = self.imp().document.borrow();
        self.set_title(Some(&document.title()));
        self.imp().editor.show_document(&document);
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
    fn watch_active(&self) {
        self.imp().editor.set_active(self.is_active());
        self.connect_is_active_notify(|window| {
            window.imp().editor.set_active(window.is_active());
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
            let mut document = window.imp().document.borrow_mut();
            let offset = tags::offset_of(&document, at);
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
            let mut document = window.imp().document.borrow_mut();
            let at = tags::offset_of(&document, from)..tags::offset_of(&document, to);
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
            let document = window.imp().document.borrow();
            window.imp().editor.retag(&document, &edit.lines);
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
            // `changed` fires before this for an edit, and its retag has
            // already moved the dim: taking the Document here would be a
            // second borrow of one the splice may still hold.
            let Ok(document) = window.imp().document.try_borrow() else {
                return;
            };
            window.imp().editor.refocus(&document);
        });
    }
}

/// Which way Bigger Text, Smaller Text and Default Text Size move.
#[derive(Clone, Copy)]
enum Step {
    Bigger,
    Smaller,
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
fn reset(app: &gtk::Application, session: &Session, each: impl Fn(&Window)) {
    crate::editor::install_type(session.scheme(), session.settings().face, session.step());
    for window in app.windows() {
        if let Ok(window) = window.downcast::<Window>() {
            each(&window);
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
/// The caller has already moved the session; this is told the ground rather
/// than asking, so that the two cannot disagree about which one it is.
pub fn repaint(app: &gtk::Application, session: &Session, scheme: Scheme) {
    reset(app, session, |window| {
        let document = window.imp().document.borrow();
        window.imp().editor.set_scheme(scheme, &document);
    });
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
        first = Some(present(app, Document::untitled(), session));
    }
    for path in documents {
        match Document::open(path) {
            Ok(document) => {
                let window = present(app, document, session);
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
            window.imp().editor.place_caret(
                &window.imp().document.borrow(),
                caret,
                scroll.is_none(),
            );
        }
        // After `--caret`, because placing the cursor collapses a selection to
        // it: a state naming both means the selection, with the caret at the
        // end `--select` leaves the insert mark on.
        if let Some((from, to)) = session.flags().select {
            window
                .imp()
                .editor
                .select(&window.imp().document.borrow(), from, to);
        }
        if let Some(scroll) = scroll {
            window.imp().editor.scroll_to(scroll);
        }
    }
    if let Some(window) = first
        && session.flags().measure.is_some()
    {
        harness::cold_start(&window);
        harness::watch(&window);
    }
}

/// Opens one window on `document`, and hands it back.
fn present(app: &gtk::Application, document: Document, session: &Rc<Session>) -> Window {
    let window = Window::new(app, document, session);
    window.present();
    window
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
        match Document::open(&path) {
            Ok(document) => {
                present(app, document, session);
            }
            Err(err) => eprintln!("quill: cannot open {}: {err}", path.display()),
        }
    }
}

/// Takes down the shape of every window still open.
///
/// Quitting outright — `Ctrl+Q`, or the desktop closing the session — destroys
/// windows without asking them to close, so shutdown asks the ones that are
/// left. A window that closed on its own is already gone from this list and is
/// remembered once.
pub fn remember_open(app: &gtk::Application) {
    for window in app.windows() {
        if let Ok(window) = window.downcast::<Window>() {
            window.remember();
        }
    }
}
