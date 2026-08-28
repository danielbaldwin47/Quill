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
use quill_engine::settings::WindowState;

use crate::harness;
use crate::session::Session;

mod imp {
    use std::cell::RefCell;
    use std::rc::Rc;

    use gtk::prelude::*;
    use gtk::subclass::prelude::*;
    use gtk::{ScrolledWindow, glib};
    use quill_engine::document::Document;

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
        window.set_document(document);
        window
            .imp()
            .editor
            .set_type(session.settings().face, session.size());
        window.install_size_steps();
        window.imp().editor.grab_focus();
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

    /// Installs Bigger Text, Smaller Text and Default Text Size.
    ///
    /// On the window rather than on the application because that is what
    /// `docs/shortcuts.md` names them: `font.bigger` with `win.` in front. The
    /// size they move is the session's, though, so stepping it in one window
    /// steps it in every window — a writer has one pair of eyes.
    fn install_size_steps(&self) {
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
        ]);
    }

    /// Steps the type size one pixel, or back to the default one.
    ///
    /// The step stops at the ends of [`quill_engine::settings::type_sizes`]
    /// rather than wrapping or refusing: a writer holding the key down means
    /// "as big as it goes", and it is the same range `size` in the file and
    /// `--size` on the command line are held to, because it is the same
    /// question asked three ways.
    fn step_size(&self, step: Step) {
        let Some(session) = self.imp().session.borrow().clone() else {
            return;
        };
        let steps = quill_engine::settings::type_sizes();
        let wanted = match step {
            Step::Bigger => session.size().saturating_add(1),
            Step::Smaller => session.size().saturating_sub(1),
            Step::Default => quill_engine::settings::default_size(),
        };
        let size = wanted.clamp(*steps.start(), *steps.end());
        if size == session.size() {
            return;
        }
        session.set_size(size);
        if let Some(app) = self.application() {
            reset_type(&app, session.settings().face, size);
        }
    }

    /// Takes `document` as this window's own and shows it.
    fn set_document(&self, document: Document) {
        self.imp().document.replace(document);
        let document = self.imp().document.borrow();
        self.set_title(Some(&document.title()));
        self.imp().editor.show_document(&document);
    }
}

/// Which way Bigger Text, Smaller Text and Default Text Size move.
#[derive(Clone, Copy)]
enum Step {
    Bigger,
    Smaller,
    Default,
}

/// Sets every open window's Editor in `face` at `size`.
///
/// The stylesheet first and once, because it belongs to the display rather
/// than to a window; then each Editor, because the leading and the measure are
/// laid out per widget.
fn reset_type(app: &gtk::Application, face: quill_engine::settings::Face, size: u32) {
    crate::editor::install_type(face, size);
    for window in app.windows() {
        if let Ok(window) = window.downcast::<Window>() {
            window.imp().editor.set_type(face, size);
        }
    }
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
