//! The Settings window (`Ctrl+,`): the rows that have no menu home (#125).
//!
//! Plain GTK4 ([ADR 0009](../../docs/adr/0009-plain-gtk4-without-libadwaita.md)):
//! a window transient for the one it was opened from, holding one grid. Every
//! other setting a writer can move is a menu row or a chord; these four are
//! what is left — the Typewriter anchor, which nothing else can move at all,
//! Follow System, the Spell-check language the Spell check spec will fill in,
//! and a button that hands `settings.toml` to the system editor.
//!
//! No row sets a value on the session. A row writes the file
//! ([`Session::edit_settings`]) and the settings watch reads it back and puts
//! it on to every window a moment later, which is the same path a writer's own
//! edit of the file takes — so there is one way a setting reaches the page and
//! not two. What the last read of the file refused is the label at the bottom,
//! the one place a writer is shown a refusal without a terminal.

use std::path::Path;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gio, glib};

use quill_engine::settings::{Settings, Theme};
use quill_engine::shortcuts::Refusal;
use quill_engine::theme::Scheme;

use crate::session::Session;

/// How near the top and the bottom of the window the Typewriter anchor may be
/// dragged. The setting itself takes any fraction (`docs/architecture.md`
/// § Settings); these are as far as a hand on the scale can put it, because a
/// caret resting in the last tenth of the window is not a rest line.
const ANCHOR_LOW: f64 = 0.2;
/// The other end of that scale.
const ANCHOR_HIGH: f64 = 0.8;
/// What one press of an arrow key on the scale moves the anchor by.
const ANCHOR_STEP: f64 = 0.01;
/// How many digits of the anchor the scale writes beside itself.
const ANCHOR_DIGITS: i32 = 2;

/// What the Spell-check row says for as long as the Spell check spec has not
/// landed.
const SPELL_TOOLTIP: &str = "Spell check: not yet built";
/// The one entry its dropdown carries meanwhile, so that the row is a row.
const SPELL_LANGUAGE: &str = "System default";

/// The window's margin and the space between its rows and its two columns.
const MARGIN: i32 = 18;
/// Between one row and the next.
const ROW_GAP: i32 = 12;
/// Between a row's label and its control.
const COLUMN_GAP: i32 = 24;

/// What the "Edit settings.toml…" button does with the file's URI: hands it to
/// the desktop's default handler, or — under a test — to a stub that takes
/// down what it was handed.
type Launch = dyn Fn(&str) -> Result<(), glib::Error>;

/// Opens the Settings window over `parent`.
///
/// Built on every open and dropped when it closes, as the shortcuts window is,
/// so that every row opens showing what the file says now.
pub fn open(parent: &gtk::Window, session: &Rc<Session>) {
    let grid = gtk::Grid::builder()
        .row_spacing(ROW_GAP)
        .column_spacing(COLUMN_GAP)
        .margin_top(MARGIN)
        .margin_bottom(MARGIN)
        .margin_start(MARGIN)
        .margin_end(MARGIN)
        .build();

    let anchor = gtk::Scale::with_range(
        gtk::Orientation::Horizontal,
        ANCHOR_LOW,
        ANCHOR_HIGH,
        ANCHOR_STEP,
    );
    anchor.set_digits(ANCHOR_DIGITS);
    // The number beside the slider, so that a writer reading the row and a
    // writer reading the file are reading the same value.
    anchor.set_draw_value(true);
    anchor.set_hexpand(true);
    // Set before the handler is connected, so that opening the window is not
    // itself a write.
    anchor.set_value(session.settings().typewriter_anchor);
    anchor.connect_value_changed(glib::clone!(
        #[strong]
        session,
        move |anchor| {
            let value = anchor.value();
            session.edit_settings(|settings| anchored(settings, value));
        }
    ));
    row(&grid, 0, "Typewriter anchor", &anchor);

    let follow = gtk::Switch::builder().halign(gtk::Align::End).build();
    follow.set_active(session.settings().theme == Theme::Auto);
    follow.connect_active_notify(glib::clone!(
        #[strong]
        session,
        move |follow| {
            let on = follow.is_active();
            let scheme = session.scheme();
            session.edit_settings(|settings| followed(settings, on, scheme));
        }
    ));
    row(&grid, 1, "Follow System", &follow);

    let language = gtk::DropDown::from_strings(&[SPELL_LANGUAGE]);
    language.set_halign(gtk::Align::End);
    language.set_sensitive(false);
    language.set_tooltip_text(Some(SPELL_TOOLTIP));
    row(&grid, 2, "Spell-check language", &language);

    let button = gtk::Button::builder()
        .label("Edit settings.toml…")
        .halign(gtk::Align::End)
        .build();
    let launch = launcher();
    button.connect_clicked(glib::clone!(
        #[strong]
        session,
        move |_| edit(session.settings_path(), launch.as_ref())
    ));
    row(&grid, 3, "Keyboard shortcuts", &button);

    if let Some(said) = refused(&session.refusals()) {
        let label = gtk::Label::builder()
            .label(said)
            .halign(gtk::Align::Start)
            .wrap(true)
            .build();
        grid.attach(&label, 0, 4, 2, 1);
    }

    gtk::Window::builder()
        .title("Settings")
        .transient_for(parent)
        .destroy_with_parent(true)
        .child(&grid)
        .build()
        .present();
}

/// One row of the grid: its label in the first column, its control in the
/// second.
fn row(grid: &gtk::Grid, at: i32, label: &str, control: &impl IsA<gtk::Widget>) {
    let label = gtk::Label::builder()
        .label(label)
        .halign(gtk::Align::Start)
        .build();
    grid.attach(&label, 0, at, 1, 1);
    grid.attach(control, 1, at, 1, 1);
}

/// What dragging the anchor scale writes.
fn anchored(settings: &mut Settings, anchor: f64) {
    settings.typewriter_anchor = anchor;
}

/// What Follow System writes: `auto` switched on, and the ground on screen
/// switched off.
///
/// Off writes a ground rather than nothing, because the switch is the writer
/// saying *stop following*, and the answer to "which ground, then" is the one
/// they are looking at ([`Scheme::setting`], which is the same conversion the
/// `Ctrl+Shift+L` toggle makes).
fn followed(settings: &mut Settings, on: bool, scheme: Scheme) {
    settings.theme = if on { Theme::Auto } else { scheme.setting() };
}

/// What the window says at the bottom about the last read of the settings
/// file, and `None` where it refused nothing.
///
/// One line per refused entry, each the entry as the writer wrote it and why
/// none of it was applied — the same sentence the app warns under
/// `quill-settings`, which is [`Refusal`]'s own.
fn refused(refusals: &[Refusal]) -> Option<String> {
    (!refusals.is_empty()).then(|| {
        refusals
            .iter()
            .map(Refusal::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    })
}

/// Hands the settings file to `launch`, as the URI a handler is asked for.
///
/// A file that has no URI and a desktop that has no handler for it are one
/// line on stderr each: the writer asked for an editor and there is none,
/// which is the desktop's to fix and not Quill's to fail on.
fn edit(path: &Path, launch: &Launch) {
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

    use quill_engine::settings::Chrome;

    use super::*;
    use crate::flags::Flags;

    /// A launch reading and writing a settings file of this test's own.
    fn launched(name: &str) -> (Rc<Session>, std::path::PathBuf) {
        let path = std::env::temp_dir().join(format!("quill-{name}-{}.toml", std::process::id()));
        std::fs::remove_file(&path).ok();
        let session = Session::open(
            Flags {
                settings: Some(path.clone()),
                ..Flags::default()
            },
            None,
        );
        (session, path)
    }

    /// Dragging the scale writes the anchor to the file, and writes it beside
    /// what the writer had already changed with a key.
    #[test]
    fn the_anchor_row_writes_the_anchor_the_scale_was_dragged_to() {
        let (session, path) = launched("anchor");
        assert_eq!(session.toggle_chrome(), Chrome::Hidden);
        session.edit_settings(|settings| anchored(settings, ANCHOR_LOW));
        let (written, notes) = Settings::read_from(&path);
        assert_eq!(notes, Vec::<String>::new());
        assert!(
            (written.typewriter_anchor - ANCHOR_LOW).abs() < f64::EPSILON,
            "{}",
            written.typewriter_anchor
        );
        assert_eq!(written.chrome, Chrome::Hidden, "and what the key moved");
        std::fs::remove_file(&path).ok();
    }

    /// Follow System writes `auto` switched on, and the ground on screen
    /// switched off.
    #[test]
    fn follow_system_writes_auto_and_switching_it_off_pins_the_ground_on_screen() {
        let (session, path) = launched("follow");
        session.edit_settings(|settings| followed(settings, true, session.scheme()));
        let (written, notes) = Settings::read_from(&path);
        assert_eq!(notes, Vec::<String>::new());
        assert_eq!(written.theme, Theme::Auto);
        assert_eq!(
            written
                .to_toml()
                .lines()
                .find(|line| line.starts_with("theme")),
            Some("theme = \"auto\"")
        );
        session.edit_settings(|settings| followed(settings, false, Scheme::Dark));
        let (written, _) = Settings::read_from(&path);
        assert_eq!(written.theme, Theme::Dark);
        std::fs::remove_file(&path).ok();
    }

    /// The button hands the desktop the settings file, as a URI naming the
    /// file this launch is running on.
    #[test]
    fn the_button_hands_the_desktop_the_settings_file_as_a_uri() {
        let handed: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
        let taken = Rc::clone(&handed);
        let launch: Box<Launch> = Box::new(move |uri| {
            taken.borrow_mut().push(uri.to_owned());
            Ok(())
        });
        let path = std::env::temp_dir().join("quill settings.toml");
        edit(&path, launch.as_ref());
        assert_eq!(
            handed.borrow().as_slice(),
            [format!(
                "file://{}",
                path.display().to_string().replace(' ', "%20")
            )],
            "the path, escaped as a URI"
        );
    }

    /// A refused entry is shown with its reason, and nothing is shown where
    /// the file refused nothing.
    #[test]
    fn a_refused_line_is_shown_with_its_reason_and_a_clean_file_shows_none() {
        assert_eq!(refused(&[]), None);
        let refusals = [
            Refusal {
                line: "\"library.toggle\" = [\"<Super>l\"]".to_owned(),
                id: "library.toggle".to_owned(),
                reason: "<Super>l belongs to the compositor".to_owned(),
            },
            Refusal {
                line: "\"libary.toggle\" = [\"<Control>b\"]".to_owned(),
                id: "libary.toggle".to_owned(),
                reason: "there is no Command with this id".to_owned(),
            },
        ];
        assert_eq!(
            refused(&refusals),
            Some(
                "\"library.toggle\" = [\"<Super>l\"]: <Super>l belongs to the compositor\n\
                 \"libary.toggle\" = [\"<Control>b\"]: there is no Command with this id"
                    .to_owned()
            )
        );
    }
}
