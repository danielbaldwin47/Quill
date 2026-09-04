//! Export: the two Commands that need no dialog, and the confirmation every
//! file export ends in.
//!
//! Quick Export writes the Document as a PDF beside its own file with the
//! `[export]` defaults and the `[template]` toggles the Preview is showing,
//! and Copy as HTML puts the body's markup on the clipboard. Neither asks
//! anything, which is the whole of why they are here and not in the dialog
//! this module gains later: the two of them are the engine's writers called
//! with what the settings file already says.
//!
//! The engine does the writing (ADR 0008): [`quill_engine::pdf::write`] lays
//! the Document out on paper and [`quill_engine::html::fragment`] answers the
//! body, and what is left here is where the file goes, what the writer is
//! told, and the one action a desktop notification can fire.
//!
//! [`confirm`] is that telling, and it is shared: every file export ends in
//! it, quick or from a dialog, and Print and Copy as HTML never do — nothing
//! was written for a writer to open.

use std::path::{Path, PathBuf};

use gtk::prelude::*;
use gtk::{gio, glib};

use quill_engine::document::full_name;
use quill_engine::paginate::Geometry;
use quill_engine::settings::{Choice, Export};
use quill_engine::{draw, html, pdf, render, template};

use crate::files;
use crate::window::Window;

/// What a Quick Export writes.
const PDF: &str = "pdf";

/// The `app.` action the notification's Open button fires, taking the exported
/// file's path as its target.
///
/// On the application rather than a window, because a notification outlives
/// the window that sent it — a writer can click Open after closing the
/// Document — and with a target, which no [`quill_engine::commands`] row can
/// carry, so it is registered here as `chrome::RECENT_OPEN` is registered
/// beside the window's Commands.
pub const OPEN: &str = "export.open";

/// What the notification's button says.
const OPEN_LABEL: &str = "Open";

/// The id every export's notification is sent under, so a second export
/// replaces the first one's banner rather than stacking another beneath it.
const NOTIFICATION: &str = "exported";

/// What the Open button does with the exported file's URI: hands it to the
/// desktop's default handler, or — under a test — to a stub that takes down
/// what it was handed. The settings file's "Edit settings.toml…" button is the
/// same indirection for the same reason ([`crate::settings`]).
type Launch = dyn Fn(&str) -> Result<(), glib::Error>;

/// The name an export of a Document called `name` is written under.
///
/// The Document's own name and the format's extension, which is what both
/// Quick Export and the dialog's file-name field say: a Document that has
/// never been saved is `Untitled`
/// ([`quill_engine::document::Document::name`]), so its export is
/// `Untitled.pdf`.
#[must_use]
pub(crate) fn file_name(name: &str, extension: &str) -> String {
    format!("{name}.{extension}")
}

/// Where an export of the Document called `name`, whose file is `path`, goes:
/// beside that file, under [`file_name`].
fn beside(path: &Path, name: &str, extension: &str) -> PathBuf {
    path.with_file_name(file_name(name, extension))
}

/// The paper the `[export]` table asks for, in the points a page is laid out
/// in.
///
/// `auto` is resolved here rather than kept, so a writer who carries a laptop
/// across an ocean exports on the paper their desktop now names
/// ([`Export::paper_size`]).
fn geometry(export: &Export) -> Geometry {
    let (width, height) = export.paper_size();
    Geometry {
        width,
        height,
        margin: export.margin_points(),
        header: export.header,
        footer: export.footer,
        title_page: export.title_page,
    }
}

/// `export.quick`, `Ctrl+Shift+P`: the Document as a PDF beside its own file,
/// with no dialog and over whatever was there before.
///
/// An untitled Document has nothing to stand beside, so it is asked to save
/// first ([`Window::save_before_export`]) and the export happens when the file
/// is there — which is this function again, now that the Document has a path.
///
/// A write that fails is one line on stderr, as every other file the app
/// writes is, and no [`confirm`]: the confirmation is the one thing that says
/// a file is there, so it never says so about a file that is not.
pub(crate) fn quick(window: &Window) {
    let Some(session) = window.session() else {
        return;
    };
    let Some(path) = window.path() else {
        window.save_before_export();
        return;
    };
    let settings = session.settings();
    let document = window.document();
    let target = beside(&path, &document.name(), PDF);
    let written = pdf::write(
        &target,
        &document,
        &template::named(settings.template.name.as_str()),
        render::Toggles::of(&settings.template),
        geometry(&settings.export),
        f64::from(settings.export.text_size),
        &draw::wording(&document),
    );
    drop(document);
    drop(settings);
    if let Err(err) = written {
        eprintln!("quill: cannot export {}: {err}", target.display());
        return;
    }
    confirm(window, &target);
}

/// `export.copyHtml`, `Ctrl+Shift+C`: the Document's body as HTML on the
/// clipboard, as text.
///
/// The fragment and not the page ([`html::fragment`]): unstyled, with no front
/// matter and no `<style>`, so a paste into a plain field shows the markup and
/// a paste into an editor that takes HTML shows the Document. Nothing was
/// written, so no confirmation follows.
pub(crate) fn copy_html(window: &Window) {
    let document = window.document();
    let fragment = html::fragment(&document);
    drop(document);
    window.clipboard().set_text(&fragment);
}

/// What a writer is told once a file export has happened: the words on the
/// status line at the foot of the Library, and the same words as a desktop
/// notification carrying an Open button.
///
/// Both, rather than either: a machine with no notification daemon shows the
/// line alone and the export has still happened, and a writer who was looking
/// somewhere else has the banner. The line is transient the way the trash
/// notice is — the next thing the window has to say about the file overwrites
/// it ([`files::exported`]).
///
/// Called after every file export and after nothing else: Print writes no file
/// a writer could open, and [`copy_html`] writes none at all.
pub(crate) fn confirm(window: &Window, path: &Path) {
    let words = files::exported(&full_name(path));
    window.notice(&words);
    let Some(app) = window.application() else {
        return;
    };
    app.send_notification(Some(NOTIFICATION), &notification(&words, path));
}

/// The banner [`confirm`] sends: the words as its title, and one button that
/// fires [`OPEN`] with the file's path.
fn notification(words: &str, path: &Path) -> gio::Notification {
    let notification = gio::Notification::new(words);
    notification.add_button_with_target_value(
        OPEN_LABEL,
        &format!("app.{OPEN}"),
        Some(&path.to_string_lossy().to_variant()),
    );
    notification
}

/// Registers [`OPEN`] on `app`, so the notification's button has something to
/// fire.
///
/// Outside the Command registry, as `chrome::RECENT_OPEN` is: it takes the
/// path as its target, carries no chord and stands in no menu.
pub fn install(app: &gtk::Application) {
    let launch = launcher();
    let action = gio::SimpleAction::new(OPEN, Some(glib::VariantTy::STRING));
    action.connect_activate(move |_, target| {
        let Some(path) = target.and_then(|target| target.get::<String>()) else {
            return;
        };
        open(Path::new(&path), launch.as_ref());
    });
    app.add_action(&action);
}

/// Hands the exported file to `launch`, as the URI a handler is asked for.
///
/// A file that has no URI and a desktop with no handler for a PDF are one line
/// on stderr each: the file was written either way, and which application
/// opens it is the desktop's to say.
fn open(path: &Path, launch: &Launch) {
    if let Err(err) = glib::filename_to_uri(path, None).and_then(|uri| launch(&uri)) {
        eprintln!("quill: {}: cannot be opened ({err})", path.display());
    }
}

/// The desktop's default handler for a URI: what the button does outside a
/// test.
fn launcher() -> Box<Launch> {
    Box::new(|uri| gio::AppInfo::launch_default_for_uri(uri, gio::AppLaunchContext::NONE))
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use quill_engine::document::UNTITLED;
    use quill_engine::settings::Paper;

    use super::*;

    /// A Document's export is named for the Document, not for its file: the
    /// name the top bar shows with the format's extension after it.
    #[test]
    fn an_export_is_named_for_the_document_and_the_format() {
        assert_eq!(file_name("The Lighthouse", PDF), "The Lighthouse.pdf");
        assert_eq!(file_name(UNTITLED, PDF), "Untitled.pdf");
        assert_eq!(file_name("Sea storm", "html"), "Sea storm.html");
    }

    /// Quick Export stands the file beside the Document, in the Document's own
    /// folder, whatever the Document's own extension was.
    #[test]
    fn a_quick_export_stands_beside_the_document() {
        assert_eq!(
            beside(
                Path::new("/tmp/drafts/The Lighthouse.md"),
                "The Lighthouse",
                PDF
            ),
            Path::new("/tmp/drafts/The Lighthouse.pdf")
        );
        assert_eq!(
            beside(Path::new("/tmp/drafts/notes"), "notes", PDF),
            Path::new("/tmp/drafts/notes.pdf")
        );
    }

    /// An `[export]` table of the defaults with `edit` applied.
    ///
    /// The table carries a private field for the keys it does not know, so the
    /// app crate cannot write one out as a literal and edits a default
    /// instead.
    fn export(edit: impl FnOnce(&mut Export)) -> Export {
        let mut export = Export::default();
        edit(&mut export);
        export
    }

    /// The `[export]` table becomes a page geometry: the paper's own size in
    /// points, the margin converted from millimetres, and the three switches
    /// carried straight through.
    #[test]
    fn the_export_table_becomes_the_page_geometry() {
        let export = export(|export| {
            export.paper = Paper::A4;
            export.footer = true;
        });
        let paper = geometry(&export);
        assert_eq!((paper.width, paper.height), Paper::A4.size());
        assert_eq!(paper.margin, export.margin_points());
        assert_eq!(
            (paper.header, paper.footer, paper.title_page),
            (false, true, false)
        );
        assert_eq!(
            geometry(&Export::default()).width,
            Paper::default().size().0,
            "auto is resolved to a size, never left as auto"
        );
    }

    /// The notification is built from the status line's own words, and its
    /// one button fires the action the application registers.
    ///
    /// `gio::Notification` answers nothing it was built with, so what is
    /// asserted is the pair that has to agree with something else: the words,
    /// which are the status line's ([`files::exported`]), and the detailed
    /// action, which is [`install`]'s name in the `app.` scope.
    #[test]
    fn the_notification_says_the_status_lines_words_and_fires_the_apps_open_action() {
        let path = Path::new("/tmp/drafts/The Lighthouse.pdf");
        let words = files::exported(&full_name(path));
        assert_eq!(words, "Exported The Lighthouse.pdf");
        assert_eq!(format!("app.{OPEN}"), "app.export.open");
        let _ = notification(&words, path);
    }

    /// The Open button hands the desktop the exported file, as a URI naming
    /// the file that was written.
    #[test]
    fn the_open_button_hands_the_desktop_the_exported_file_as_a_uri() {
        let handed: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
        let taken = Rc::clone(&handed);
        let launch: Box<Launch> = Box::new(move |uri| {
            taken.borrow_mut().push(uri.to_owned());
            Ok(())
        });
        let path = std::env::temp_dir().join("The Lighthouse.pdf");
        open(&path, launch.as_ref());
        assert_eq!(
            handed.borrow().as_slice(),
            [format!(
                "file://{}",
                path.display().to_string().replace(' ', "%20")
            )]
        );
    }
}
