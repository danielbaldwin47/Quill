//! Chrome: the Command registry installed as actions and accelerators, the
//! one place in the app a chord is bound (#119); the bars, menus and Palette
//! of the Chrome and menus Piece follow it (#120–#122).
//!
//! [`quill_engine::commands`] is the data; this module turns each Command
//! into a `GSimpleAction` on the window (`win.<id>`) or the application
//! (`app.quit`, `app.window.new`) and installs its chords with
//! `set_accels_for_action`. Toggles are stateful booleans and each radio
//! group is one stateful string action, so the menus' checks and radios
//! read the same actions the chords fire. A Command whose feature is
//! not built is registered disabled: its chord does nothing and its row will
//! render greyed. Two key controllers remain elsewhere and bind no chord: the
//! harness's capture-phase stamp (`harness::watch`, #64) and the Editor's
//! caret-kind controller (#107).
//!
//! The two bars are [`Bars`] (#120): a title bar above the page and a stats
//! bar below it, in the Parity oracle's proportions (`legacy/app/css/chrome.css`),
//! shown or hidden by the `chrome` setting, `--chrome` and `chrome.toggle`.
//! Their colours are the engine's table; their geometry is the constants
//! below, because a bar's height is widget geometry and not a colour. Their
//! look is a stylesheet no file holds: [`stylesheet`] builds it per ground and
//! [`crate::editor::install_type`] installs it.
//!
//! How the bars step back while a hand is typing is [`typing`] (#128): a
//! machine with no widget in it, whose opacities the window puts on the bars
//! through [`Bars::set_fade`] and whose fade is the stylesheet's transition.
//! The three menus are [`crate::menus`]' models shown through a
//! `GtkPopoverMenu` under each bar button: the Document menu under the
//! title, the View menu under the View button and `F10`, the Stats menu
//! above the stats bar. Their look is the oracle's menu rules, as constants
//! beside the bars'. The Palette (#122) is [`crate::palette`], one popover
//! over the page that `palette.open` toggles.

use std::cell::Cell;
use std::path::Path;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{cairo, gio, glib};
use quill_engine::commands::{self, COMMANDS, Command, Kind, Menu, Scope};
use quill_engine::focus::Focus;
use quill_engine::focus::typewriter::Typewriter;
use quill_engine::settings::{Choice, Chrome, FocusScope, PreviewLayout, TemplateName};
use quill_engine::shortcuts::{Chord, Refusal};
use quill_engine::stats::words;
use quill_engine::theme::{Role, Scheme};

use crate::ground::Ground;
use crate::session::{Session, TemplateToggle};
use crate::window::Window;

pub mod typing;

/// What the stateful actions show: the modes as the session holds them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Modes {
    /// Focus is on.
    pub focus: bool,
    /// The Focus scope, `sentence` or `paragraph`, on or off.
    pub focus_scope: &'static str,
    /// Typewriter is on.
    pub typewriter: bool,
    /// Live is on: the markup rendered in place.
    pub live: bool,
    /// The ground shown is the dark one, whatever `theme` says.
    pub dark: bool,
    /// The theme setting, `auto`, `light` or `dark`.
    pub theme: &'static str,
    /// The face, `duo`, `quattro` or `mono`.
    pub face: &'static str,
    /// The window fills the screen.
    pub fullscreen: bool,
    /// The two bars are shown.
    pub bars: bool,
    /// The stats bar is shown, while the bars are.
    pub stats: bool,
    /// The Library stands beside the page. Per window rather than per session,
    /// as the fullscreen above it is: the Library is the application's, the
    /// pane showing it is the window's.
    pub library: bool,
    /// The Preview pane stands beside the Editor. Per window, as the Library
    /// above it is, and remembered by nothing: the pane is closed at every
    /// launch (#263).
    pub preview: bool,
    /// The layout the pane last showed, and the one it shows now while open,
    /// `split` or `full`. The session's, unlike the pane itself.
    pub preview_layout: &'static str,
    /// The Template the page is laid out in, `modern` to
    /// `manuscript-quattro`.
    pub template: &'static str,
    /// The Template's `center_headings` toggle.
    pub center_headings: bool,
    /// Its `number_headings` toggle.
    pub number_headings: bool,
    /// Its `indent_paragraphs` toggle.
    pub indent_paragraphs: bool,
}

impl Modes {
    /// What the session shows, as the actions show it.
    ///
    /// The scope is the live one while Focus is on; off, it is the one the
    /// settings file holds, which the session restores when Focus returns.
    pub fn of(
        session: &crate::session::Session,
        fullscreen: bool,
        library: bool,
        preview: bool,
    ) -> Self {
        let (focus, focus_scope) = match session.focus() {
            Focus::On(scope) => (true, scope.as_str()),
            Focus::Off => (false, session.settings().focus_scope.as_str()),
        };
        Modes {
            focus,
            focus_scope,
            typewriter: matches!(session.typewriter(), Typewriter::On(_)),
            live: session.live(),
            dark: session.scheme() == Scheme::Dark,
            theme: session.theme().as_str(),
            face: session.face().as_str(),
            fullscreen,
            bars: session.chrome() == Chrome::Shown,
            stats: session.stats(),
            library,
            preview,
            preview_layout: session.preview_layout().as_str(),
            template: session.template().name.as_str(),
            center_headings: session.template().center_headings,
            number_headings: session.template().number_headings,
            indent_paragraphs: session.template().indent_paragraphs,
        }
    }
}

/// What a fired Command does, given the Command.
type Handler = Rc<dyn Fn(&'static Command)>;

/// Installs the application's half: the `app.` actions.
///
/// The chords are [`install_chords`]', because what a Command is bound to is
/// the writer's `[shortcuts]` table over the registry and reading a chord is
/// `gtk::accelerator_parse`'s, which wants GTK started. Called once, before
/// the first window.
pub fn install(app: &gtk::Application, session: &Rc<Session>) {
    let fired = app.clone();
    let session = Rc::clone(session);
    register(
        app,
        Scope::App,
        Rc::new(move |command| run_app(&fired, &session, command)),
    );
}

/// Installs every Command's chords as the settings file leaves them, and
/// answers with the entries that could not be installed after all.
///
/// The chords are installed for every Command, built or not, because GTK keeps
/// them on the application and an unbuilt Command's action is disabled, which
/// is what makes its chord do nothing. Every Command is set, not only the ones
/// the file names: `set_accels_for_action` replaces what a Command was bound
/// to, so an entry taken out of the file restores its default here and an
/// empty entry leaves it bound to nothing.
///
/// The engine reads the shape of a chord and leaves the key name to GTK
/// ([`quill_engine::shortcuts`]), so this is where a chord shaped like a chord
/// that names no key GDK has becomes a refusal — the one refusal the writer
/// can only be told about from in here. What it refuses is [`installed`]'s to
/// decide.
///
/// Called at startup and again on every save, so it must be idempotent: it is,
/// because the map it installs is computed from the file on every read.
pub fn install_chords(app: &gtk::Application, session: &Session) -> Vec<Refusal> {
    let shortcuts = session.settings().shortcuts();
    let mut refusals = shortcuts.refusals;
    for command in COMMANDS {
        let Some(chords) = shortcuts.chords.get(command.id) else {
            continue;
        };
        let entry = shortcuts.entries.get(command.id).map(String::as_str);
        let (accels, refused) = installed(command, chords, entry, |chord| {
            gtk::accelerator_parse(chord.as_str()).is_some()
        });
        refusals.extend(refused);
        let accels: Vec<&str> = accels.iter().map(String::as_str).collect();
        app.set_accels_for_action(&command.action(), &accels);
    }
    refusals
}

/// What GTK is told `command` is bound to, and the refusal where the writer's
/// entry names a key GDK has none of.
///
/// One chord GDK has no key for refuses the writer's whole entry, the way every
/// refusal the engine makes refuses one, and leaves the Command on the defaults
/// the entry was meant to replace (`docs/shortcuts.md` § Rebinding). `entry` is
/// `None` for a Command the file never named, whose chords are the registry's
/// own and are held to keys GDK has by
/// `gdk_knows_every_key_name_the_table_installs`.
///
/// Split out of [`install_chords`] with `known` as a parameter because
/// `gtk::accelerator_parse` wants a display and this decision does not.
fn installed(
    command: &'static Command,
    chords: &[Chord],
    entry: Option<&str>,
    known: impl Fn(&Chord) -> bool,
) -> (Vec<String>, Option<Refusal>) {
    let refused = entry.and_then(|entry| {
        let unknown = chords.iter().find(|chord| !known(chord))?;
        Some(Refusal::unknown_key(command.id, entry, unknown))
    });
    match refused {
        Some(refusal) => (command.accels(), Some(refusal)),
        None => (chords.iter().map(Chord::to_string).collect(), None),
    }
}

/// The chords `command` is installed with now.
///
/// Asked of GTK, because [`install_chords`] put them there and a second copy
/// of the effective map is a second thing that can go stale; the registry's
/// own where there is no application yet, which is what a test without one
/// reads. This is what the menus' rows ([`crate::menus`]) and the Palette's
/// key labels ([`crate::palette`]) show, so a rebound Command is labelled the
/// way the writer rebound it.
pub fn accels(command: &Command) -> Vec<String> {
    let Some(app) = gio::Application::default().and_downcast::<gtk::Application>() else {
        return command.accels();
    };
    app.accels_for_action(&command.action())
        .into_iter()
        .map(String::from)
        .collect()
}

/// Installs the window's half: every `win.` action, set to what the session
/// shows now.
pub fn install_window(window: &Window) {
    let fired = window.downgrade();
    register(
        window,
        Scope::Win,
        Rc::new(move |command| {
            if let Some(window) = fired.upgrade() {
                run_window(&window, command);
            }
        }),
    );
    install_recent(window);
    // The compositor can fill the screen without `F11` being pressed, so the
    // check follows the window rather than the Command.
    window.connect_fullscreened_notify(|window| reflect(window, window.modes()));
    reflect(window, window.modes());
}

/// The one window action that is not a Command: opening a named recent
/// Document (#246, stories 41 and 42).
///
/// It takes the path as its target, which no [`COMMANDS`] row can — a Command
/// is one action with no parameter, or a radio's group with a fixed value —
/// and both the Open Recent submenu ([`crate::menus`]) and the Palette's
/// recents rows ([`crate::palette`]) activate it, so a recent is opened by one
/// path however it was chosen. It is registered here rather than by
/// [`register`] because it is outside the registry, and it carries no chord
/// and no menu row of its own: `file.recent` is the Command a writer reaches.
pub const RECENT_OPEN: &str = "file.recentOpen";

/// Registers [`RECENT_OPEN`] on `window`.
fn install_recent(window: &Window) {
    let action = gio::SimpleAction::new(RECENT_OPEN, Some(glib::VariantTy::STRING));
    let opened = window.downgrade();
    action.connect_activate(move |_, target| {
        let (Some(window), Some(path)) = (
            opened.upgrade(),
            target.and_then(|target| target.get::<String>()),
        ) else {
            return;
        };
        window.open_path(Path::new(&path));
    });
    window.add_action(&action);
}

/// Puts what the session now shows on to every window's stateful actions.
///
/// One pass for every window, as `repaint` and the mode keys are, because a
/// mode moving in one window moves it in all of them.
pub fn reflect_windows(app: &gtk::Application) {
    for window in app.windows() {
        if let Ok(window) = window.downcast::<Window>() {
            reflect(&window, window.modes());
        }
    }
}

/// Registers one scope's actions on `map`, each firing `run` with its
/// Command, disabled where the Command is not built.
///
/// Generic over the map so a test can hand it a `SimpleActionGroup` and
/// activate by name without a window.
pub fn register(map: &impl IsA<gio::ActionMap>, scope: Scope, run: Handler) {
    for command in COMMANDS.iter().filter(|command| command.scope == scope) {
        let action = match command.kind {
            Kind::Check => {
                gio::SimpleAction::new_stateful(command.name(), None, &false.to_variant())
            }
            Kind::Plain | Kind::Radio { .. } => gio::SimpleAction::new(command.name(), None),
        };
        let fire = Rc::clone(&run);
        action.connect_activate(move |_, _| fire(command));
        action.set_enabled(command.built);
        map.add_action(&action);
    }
    if scope != Scope::Win {
        return;
    }
    for group in commands::radio_groups() {
        let members: Vec<&'static Command> = COMMANDS
            .iter()
            .filter(|command| matches!(command.kind, Kind::Radio { group: g, .. } if g == group))
            .collect();
        let action =
            gio::SimpleAction::new_stateful(group, Some(glib::VariantTy::STRING), &"".to_variant());
        let fire = Rc::clone(&run);
        let chosen = members.clone();
        action.connect_activate(move |_, wanted| {
            let Some(wanted) = wanted.and_then(|wanted| wanted.get::<String>()) else {
                return;
            };
            let member = chosen
                .iter()
                .find(|member| matches!(member.kind, Kind::Radio { value, .. } if value == wanted));
            if let Some(member) = member {
                fire(member);
            }
        });
        action.set_enabled(members.iter().any(|member| member.built));
        map.add_action(&action);
    }
}

/// Sets the stateful actions on `map` to `modes`.
pub fn reflect(map: &impl IsA<gio::ActionMap>, modes: Modes) {
    let set = |name: &str, state: glib::Variant| {
        if let Some(action) = map.lookup_action(name).and_downcast::<gio::SimpleAction>() {
            action.set_state(&state);
        }
    };
    set("focus.toggle", modes.focus.to_variant());
    set("typewriter.toggle", modes.typewriter.to_variant());
    set("live.toggle", modes.live.to_variant());
    set("theme.toggle", modes.dark.to_variant());
    set("window.fullscreen", modes.fullscreen.to_variant());
    // The row reads "Hide Bars", so its check is on when the bars are hidden.
    set("chrome.toggle", (!modes.bars).to_variant());
    set("chrome.stats", modes.stats.to_variant());
    set("library.toggle", modes.library.to_variant());
    // Each row is its own layout's toggle, so it is ticked only while the
    // pane is open in that layout and the pane away leaves both clear: the
    // rows are states the window is in and not where Preview would open.
    let showing = |layout: &str| (modes.preview && modes.preview_layout == layout).to_variant();
    set("preview.full", showing("full"));
    set("preview.split", showing("split"));
    // With Focus off neither scope's row is ticked, as the oracle's menu
    // has it: the scope the file holds is the one Focus comes back to, not
    // a state the page is in.
    let scope = if modes.focus { modes.focus_scope } else { "" };
    set("focus_scope", scope.to_variant());
    set("theme", modes.theme.to_variant());
    set("face", modes.face.to_variant());
    set("template", modes.template.to_variant());
    set(
        "template.centerHeadings",
        modes.center_headings.to_variant(),
    );
    set(
        "template.numberHeadings",
        modes.number_headings.to_variant(),
    );
    set(
        "template.indentParagraphs",
        modes.indent_paragraphs.to_variant(),
    );
}

/// The application's Commands.
///
/// Quit walks the windows itself rather than calling `app.quit()`, because a
/// window whose Document has something to ask has to be able to keep itself —
/// and Quill — open ([`crate::window::quit`]).
fn run_app(app: &gtk::Application, session: &Rc<Session>, command: &Command) {
    match command.id {
        "app.quit" => crate::window::quit(app),
        "window.new" => crate::window::open_new(app, session),
        _ => {}
    }
}

/// The menu a Command opens, for the two that open one: `chrome.doc` the
/// Document menu under the title, `chrome.view` (`F10`) the View menu. The
/// Stats menu has no Command of its own; a click on the stats bar opens it.
pub(crate) fn opens(id: &str) -> Option<Menu> {
    match id {
        "chrome.doc" => Some(Menu::Document),
        "chrome.view" => Some(Menu::View),
        _ => None,
    }
}

/// The window's Commands, each reaching the code its key reached before
/// #119, then every window's actions set to what moved.
fn run_window(window: &Window, command: &Command) {
    match command.id {
        "font.bigger" => window.step_size(crate::window::Step::Bigger),
        "font.smaller" => window.step_size(crate::window::Step::Smaller),
        "font.reset" => window.step_size(crate::window::Step::Default),
        // The rendered page's own ladder, which the Editor's never reaches
        // and which never reaches the Editor (#263 § Zoom).
        "preview.bigger" => window.step_zoom(crate::window::Zoom::Bigger),
        "preview.smaller" => window.step_zoom(crate::window::Zoom::Smaller),
        "preview.reset" => window.step_zoom(crate::window::Zoom::Reset),
        "font.duo" => window.set_face(quill_engine::settings::Face::Duo),
        "font.quattro" => window.set_face(quill_engine::settings::Face::Quattro),
        "font.mono" => window.set_face(quill_engine::settings::Face::Mono),
        // View › Template: one Template for the app, on the rendered page
        // alone (#263 § Templates).
        "template.modern" => window.set_template(TemplateName::Modern),
        "template.classic" => window.set_template(TemplateName::Classic),
        "template.manuscriptMono" => window.set_template(TemplateName::ManuscriptMono),
        "template.manuscriptDuo" => window.set_template(TemplateName::ManuscriptDuo),
        "template.manuscriptQuattro" => window.set_template(TemplateName::ManuscriptQuattro),
        "template.centerHeadings" => window.toggle_template(TemplateToggle::CenterHeadings),
        "template.numberHeadings" => window.toggle_template(TemplateToggle::NumberHeadings),
        "template.indentParagraphs" => window.toggle_template(TemplateToggle::IndentParagraphs),
        "theme.toggle" => window.toggle_scheme(),
        "theme.light" => window.set_theme(quill_engine::settings::Theme::Light),
        "theme.dark" => window.set_theme(quill_engine::settings::Theme::Dark),
        "theme.auto" => window.set_theme(quill_engine::settings::Theme::Auto),
        "focus.toggle" => window.toggle_focus(),
        "focus.sentence" => window.set_focus_scope(FocusScope::Sentence),
        "focus.paragraph" => window.set_focus_scope(FocusScope::Paragraph),
        "focus.swap" => window.swap_focus_scope(),
        "typewriter.toggle" => window.toggle_typewriter(),
        "live.toggle" => window.toggle_live(),
        "chrome.toggle" => window.toggle_bars(),
        "library.toggle" => window.toggle_library(),
        // Two rows, each its own layout's toggle: the chord opens the pane in
        // that layout, switches an open pane to it, or closes the pane it is
        // already showing.
        "preview.full" => window.preview_to(PreviewLayout::Full),
        "preview.split" => window.preview_to(PreviewLayout::Split),
        "library.search" => window.search_library(),
        "chrome.stats" => window.toggle_stats(),
        "chrome.doc" | "chrome.view" => {
            if let Some(menu) = opens(command.id) {
                window.open_menu(menu);
            }
        }
        "file.new" => window.new_document(),
        "file.open" => window.open_file(),
        "file.openFolder" => window.add_location(),
        "file.rename" => window.rename_document(),
        "file.duplicate" => window.duplicate_document(),
        "file.delete" => window.trash_document(),
        "file.pin" => window.pin_document(),
        "file.next" => window.step_document(crate::files::Step::Next),
        "file.prev" => window.step_document(crate::files::Step::Prev),
        "file.save" => window.save(),
        "file.saveAs" => window.save_as(crate::window::After::Stay),
        "palette.open" => window.open_palette(),
        "file.recent" => window.open_recents(),
        "settings.open" => window.open_settings(),
        "shortcuts.open" => window.open_shortcuts(),
        "window.fullscreen" if window.is_fullscreen() => window.unfullscreen(),
        "window.fullscreen" => window.fullscreen(),
        "window.close" => window.close(),
        _ => {}
    }
    if let Some(app) = window.application() {
        reflect_windows(&app);
    }
}

// ---------------------------------------------------------------- the bars

/// The title bar's height at scale 1 (`chrome.css` `--bar-top`).
pub const TOP_HEIGHT: i32 = 32;
/// The stats bar's height at scale 1 (`--bar-bottom`).
pub const BOTTOM_HEIGHT: i32 = 26;
/// A bar's side padding (`.chrome .bar { padding: 0 10px }`), which is where
/// the View button's right edge stands.
const BAR_PAD: i32 = 10;
/// Where the Library toggle's left edge stands: `.lib-toggle { left: 8px }`,
/// pinned rather than padded so the title stays centred on the window, plus
/// the two pixels the frozen `bars` shot puts its icon right of that.
const LIBRARY_LEFT: i32 = 10;
/// A padding or a margin, as CSS writes a pair: vertical then horizontal.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Pad {
    pub(crate) y: i32,
    pub(crate) x: i32,
}

/// A bar button's padding (`.chrome button { padding: 4px 6px }`).
const BUTTON_PAD: Pad = Pad { y: 4, x: 6 };
/// A bar button's corner (`border-radius: 5px`).
const BUTTON_RADIUS: i32 = 5;
/// The title's type. `.doc-title` asks for `500 13px`, but `.chrome button`
/// is the more specific rule and says `font: inherit`, so the oracle sets its
/// title at the page's default 16 px, regular, and the frozen shot measures
/// so; `letter-spacing: -0.005em` is the one part of the title's rule that
/// survives.
const TITLE_PX: f64 = 16.0;
/// Between the title and the chevron that shows when its menu is open
/// (`.doc-title { gap: 4px }`). The chevron is invisible at rest and still
/// takes its space, which is why the oracle's title sits a little left of
/// centre.
const TITLE_GAP: i32 = 4;
/// The stats' type: `font: 11px/1`, tabular figures.
const STAT_PX: f64 = 11.0;
/// Between the stats (`.bar { gap: 15px }`).
const STAT_GAP: i32 = 15;
/// Between the View button's icon and its chevron (`gap: 2px`).
const CHEVRON_GAP: i32 = 2;
/// What the chevron adds to that gap of its own (`.chev { margin-left: 1px }`).
const CHEVRON_MARGIN: i32 = 1;
/// The title's chevron while its menu is open (`.caretdown` at `.55`).
const CHEVRON_OPEN: f64 = 0.55;
/// The UI face, in the order `chrome.css` `--chrome-font` names it.
pub(crate) const CHROME_FONT: &str = "\"Adwaita Sans\", \"Inter\", \"Noto Sans\", sans-serif";
/// How long the bars take to fade (`.chrome { transition: opacity .28s ease }`).
/// Under `--deterministic` GTK's animations are off and this is zero.
const FADE_MS: u32 = 280;

/// The fade this display runs: [`FADE_MS`], or none where
/// `gtk-enable-animations` is off — `--deterministic` turns it off
/// ([`crate::harness::determine`]), as does a desktop that asks for no
/// motion — so that a shot of the typing state is the state and not a frame
/// of the way there.
fn fade_ms() -> u32 {
    // A test builds the sheet with no GTK to ask; a shipped sheet always has
    // one, since the display is what the sheet is installed on.
    if !gtk::is_initialized() {
        return FADE_MS;
    }
    let animated =
        gtk::Settings::default().is_none_or(|settings| settings.is_gtk_enable_animations());
    if animated { FADE_MS } else { 0 }
}
/// The reading pace the stats bar counts at (`chrome.js` `WPM`).
const WORDS_PER_MINUTE: f64 = 238.0;

/// A bar button's hover ground (`--hit`), per scheme. Not in the engine's
/// table: it is a widget's ground and no annotator paints it.
const fn hit(scheme: Scheme) -> &'static str {
    match scheme {
        Scheme::Light => "rgba(0, 0, 0, 0.06)",
        Scheme::Dark => "rgba(255, 255, 255, 0.09)",
    }
}

// --------------------------------------------------------------- the menus

/// A menu's narrowest (`.menu { min-width: 178px }`). Its widest, `max-width:
/// 300px`, has no GTK CSS to go in: a label's ellipsis is what a long row
/// would need, and no row of the table's is.
const MENU_MIN_WIDTH: i32 = 178;
/// A menu's padding above its first row and below its last (`padding: 4px 0`).
const MENU_PAD: i32 = 4;
/// A menu's corner (`border-radius: 6px`).
const MENU_RADIUS: i32 = 6;
/// A menu's type (`font: 12.5px/1`).
const MENU_PX: f64 = 12.5;
/// Below the button a menu opens under (`menu()`: the button's bottom plus
/// four).
const MENU_GAP_BELOW: i32 = 4;
/// Above the stats bar the Stats menu opens over (its top less three).
const MENU_GAP_ABOVE: i32 = 3;
/// A row's height (`.menu .row { height: 19px }`).
const ROW_HEIGHT: i32 = 19;
/// A row's side margin inside the menu (`margin: 0 4px`).
const ROW_MARGIN: i32 = 4;
/// A row's padding on the right (`padding: 0 8px 0 19px`); the left of it
/// is [`TICK_COLUMN`], which a row with no tick (`.flush`) does without.
const ROW_PAD: i32 = 8;
/// The column a ticked row's label stands in from: the `19px` of the row's
/// padding, which the tick sits inside.
const TICK_COLUMN: i32 = 19;
/// A row's corner (`border-radius: 4px`).
const ROW_RADIUS: i32 = 4;

/// The tick's place in its column.
struct Tick {
    /// Its inset from the row's edge (`.tick { left: 4px }`).
    left: i32,
    /// How wide its ink stands: the width of the Parity oracle's own tick,
    /// which `legacy/app/js/chrome.js` draws as an `svg(10, 8, …)`, and the
    /// width [`Tick::glyph`] is sized for.
    width: i32,
    /// The icon size GTK draws [`TICK_GLYPH`] at to lay that much ink down.
    ///
    /// GTK sizes a `-gtk-icon-source` by its icon size and not by the node's
    /// `min-width`, and at its default of 16 px the check came out wider and
    /// heavier than the oracle's — twice its ink, and darker than any label in
    /// the menu, which is what round 5's critic gave the View menu away for
    /// (`progress/rounds/chrome-r5.json`). The glyph's ink is a fixed share of
    /// its icon, so 13 px is the icon size at which it stands [`Tick::width`]
    /// wide.
    glyph: i32,
}

impl Tick {
    /// Where the icon's box stands from the row's edge.
    ///
    /// GTK centres the glyph's ink in its icon box, and the box is the wider
    /// of the two, so the box starts half the difference left of where the
    /// ink is meant to start — which is what keeps the label column where
    /// [`TICK_COLUMN`] puts it whatever size the glyph is drawn at.
    fn inset(&self) -> f64 {
        f64::from(self.left) - f64::from(self.glyph - self.width) / 2.0
    }
}

/// The tick: four in from the row's edge (`chrome.css` `.tick { left: 4px }`),
/// as wide as the oracle's own (`chrome.js`'s `TICK`).
const TICK: Tick = Tick {
    left: 4,
    width: 10,
    glyph: 13,
};

/// A chord label's distance from its row's label, and its type.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Keys {
    pub(crate) gap: i32,
    pub(crate) px: f64,
}

/// The chord's type and its distance from the label, in a menu row and a
/// Palette row alike (`.keys { margin-left: 16px; font-size: 11.5px }`).
pub(crate) const KEYS: Keys = Keys { gap: 16, px: 11.5 };

/// A separator: one pixel high, `margin: 4px 8px`.
const SEP_MARGIN: Pad = Pad { y: 4, x: 8 };

/// A section heading's padding and type.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Head {
    pub(crate) top: i32,
    pub(crate) x: i32,
    pub(crate) bottom: i32,
    pub(crate) px: f64,
}

impl Head {
    /// The heading's `letter-spacing: .06em`, as the length GTK's CSS takes.
    pub(crate) fn tracking(self) -> f64 {
        0.06 * self.px
    }

    /// The heading's type, as the declarations the menus' and the Palette's
    /// heading rules share: `weight 600`, tracked, upper case set by the
    /// label, in the dim ink.
    pub(crate) fn type_declarations(self, dim: &str) -> String {
        let px = self.px;
        let tracking = self.tracking();
        format!("font-size: {px}px; font-weight: 600; letter-spacing: {tracking}px; color: {dim};")
    }
}

/// A menu's section heading (`padding: 6px 10px 3px; font-size: 10px`).
const HEAD: Head = Head {
    top: 6,
    x: 10,
    bottom: 3,
    px: 10.0,
};

/// `letter-spacing: -0.005em` of a `px` type, as the length GTK's CSS takes:
/// the title's and the Palette's field's.
pub(crate) fn tracking(px: f64) -> f64 {
    -0.005 * px
}

/// The chord label's declarations, the same in a menu row's `accelerator`
/// and a Palette row's keys label: the dim ink, [`KEYS`]' type and gap.
pub(crate) fn keys_declarations(dim: &str) -> String {
    let Keys { gap, px } = KEYS;
    format!("color: {dim}; font-size: {px}px; margin-left: {gap}px;")
}
/// GTK's own check glyph, the one shape its theme ships; recoloured to the
/// row's ink where the oracle draws its tick path.
const TICK_GLYPH: &str = "resource:///org/gtk/libgtk/theme/Default/assets/check-symbolic.svg";

/// A menu's colours per scheme (`chrome.css` `--menu-*`): its ground, its
/// border, its ink, its dim ink for chords and headings, the selected row's
/// ground and its shadow. Not in the engine's table, as `hit` is not.
pub(crate) struct MenuInk {
    pub(crate) ground: &'static str,
    pub(crate) border: &'static str,
    pub(crate) ink: &'static str,
    pub(crate) dim: &'static str,
    pub(crate) selected: &'static str,
    pub(crate) shadow: &'static str,
}

pub(crate) const fn menu_ink(scheme: Scheme) -> MenuInk {
    match scheme {
        Scheme::Light => MenuInk {
            ground: "#f2f2f2",
            border: "rgba(0, 0, 0, 0.12)",
            ink: "#1d1d1d",
            dim: "#8c8c8c",
            selected: "#0a94d6",
            shadow: "0 1px 1px rgba(0, 0, 0, 0.07), 0 8px 24px rgba(0, 0, 0, 0.15)",
        },
        Scheme::Dark => MenuInk {
            ground: "#2e2e2e",
            border: "rgba(255, 255, 255, 0.13)",
            ink: "#e8e8e8",
            dim: "#949494",
            selected: "#0a84c8",
            shadow: "0 1px 1px rgba(0, 0, 0, 0.35), 0 10px 30px rgba(0, 0, 0, 0.5)",
        },
    }
}

/// The menus' stylesheet, appended to the bars'.
///
/// GTK draws a menu row as a `modelbutton` with a `check` or `radio` before
/// its label and its chord in an `accelerator` after it, the selected row
/// `:selected`, a section's heading a `label.title` and the gap between
/// sections a `separator`; every rule here restyles one of those to the
/// oracle's measurement.
///
/// The heading's rule sets `opacity: 1` as well as its colour, because the
/// platform theme draws a `label.title` in a popover menu at about 0.55 and
/// colour alone does not undo that: [`menu_ink`]'s `dim` came out at #BABABA
/// on the menu's #F2F2F2, lighter than the disabled rows the heading is there
/// to organise, which is what round 5's critic gave the View menu away for.
fn menu_stylesheet(scheme: Scheme) -> String {
    let MenuInk {
        ground,
        border,
        ink,
        dim,
        selected,
        shadow,
    } = menu_ink(scheme);
    let Tick {
        width: tick_width,
        glyph: tick_glyph,
        ..
    } = TICK;
    // The tick's column: the row's padding less where the icon's box stands,
    // then the box, then what is left of the column.
    let tick_inset = TICK.inset();
    let tick_before = tick_inset - f64::from(ROW_PAD);
    let tick_after = f64::from(TICK_COLUMN) - tick_inset - f64::from(tick_glyph);
    let keys = keys_declarations(dim);
    let Pad { y: sep_y, x: sep_x } = SEP_MARGIN;
    let Head {
        top: head_top,
        x: head_x,
        bottom: head_bottom,
        ..
    } = HEAD;
    let head = HEAD.type_declarations(dim);
    format!(
        "popover.chrome-menu {{ font-family: {CHROME_FONT}; font-size: {MENU_PX}px; }}\n\
         popover.chrome-menu > contents {{\n\
         \x20 background-color: {ground}; color: {ink};\n\
         \x20 border: 1px solid {border}; border-radius: {MENU_RADIUS}px;\n\
         \x20 box-shadow: {shadow}; padding: {MENU_PAD}px 0;\n\
         \x20 min-width: {MENU_MIN_WIDTH}px;\n\
         }}\n\
         popover.chrome-menu modelbutton {{\n\
         \x20 min-height: {ROW_HEIGHT}px; min-width: 0;\n\
         \x20 margin: 0 {ROW_MARGIN}px; padding: 0 {ROW_PAD}px;\n\
         \x20 border-radius: {ROW_RADIUS}px; color: {ink};\n\
         }}\n\
         popover.chrome-menu modelbutton:disabled {{ color: {dim}; }}\n\
         popover.chrome-menu modelbutton:selected {{ background-color: {selected}; color: white; }}\n\
         popover.chrome-menu modelbutton:selected accelerator {{ color: rgba(255, 255, 255, 0.8); }}\n\
         popover.chrome-menu accelerator {{ {keys} }}\n\
         popover.chrome-menu modelbutton check, popover.chrome-menu modelbutton radio {{\n\
         \x20 min-width: {tick_glyph}px; min-height: {tick_glyph}px;\n\
         \x20 -gtk-icon-size: {tick_glyph}px;\n\
         \x20 margin: 0 {tick_after}px 0 {tick_before}px; padding: 0;\n\
         \x20 border: none; background: none; box-shadow: none; transform: none;\n\
         \x20 -gtk-icon-source: none; color: {ink};\n\
         }}\n\
         popover.chrome-menu modelbutton check:checked, popover.chrome-menu modelbutton radio:checked {{\n\
         \x20 -gtk-icon-source: -gtk-recolor(url(\"{TICK_GLYPH}\"));\n\
         }}\n\
         popover.chrome-menu modelbutton:selected check, popover.chrome-menu modelbutton:selected radio {{ color: white; }}\n\
         popover.chrome-menu modelbutton arrow {{ min-width: {tick_width}px; min-height: {tick_width}px; opacity: 0.5; }}\n\
         popover.chrome-menu separator {{ min-height: 1px; margin: {sep_y}px {sep_x}px; background: {border}; }}\n\
         popover.chrome-menu label.title {{\n\
         \x20 padding: {head_top}px {head_x}px {head_bottom}px; {head}\n\
         \x20 opacity: 1;\n\
         }}\n"
    )
}

/// The bars' stylesheet, loaded with the type's ([`crate::editor::install_type`])
/// so that a ground change reloads both together.
///
/// Everything the bars draw takes its colour from here, the icons and the
/// rules through the widget's CSS `color`, so a scheme change is a stylesheet
/// change and nothing else. The greys are the ground's table; the menus'
/// literals and the hit colour are the scheme's ([`menu_ink`], [`hit`]).
pub fn stylesheet(ground: Ground) -> String {
    let Ground { scheme, colours } = ground;
    let fg = colours.colour(Role::ChromeFg).to_hex();
    let strong = colours.colour(Role::ChromeFgStrong).to_hex();
    let rule = colours.colour(Role::Rule).to_css();
    let hit = hit(scheme);
    let Pad { y: pad_y, x: pad_x } = BUTTON_PAD;
    let tracking = tracking(TITLE_PX);
    let fade_ms = fade_ms();
    let (title_faded, stats_faded) = (typing::TITLE_FADED, typing::STATS_FADED);
    format!(
        ".chrome {{ color: {fg}; transition: opacity {fade_ms}ms ease; }}\n\
         .chrome-top.faded {{ opacity: {title_faded}; }}\n\
         .chrome-bottom.faded {{ opacity: {stats_faded}; }}\n\
         .chrome button {{\n\
         \x20 background: none; border: none; box-shadow: none; outline: none;\n\
         \x20 text-shadow: none; -gtk-icon-shadow: none;\n\
         \x20 min-height: 0; min-width: 0;\n\
         \x20 padding: {pad_y}px {pad_x}px; border-radius: {BUTTON_RADIUS}px;\n\
         \x20 color: {fg};\n\
         }}\n\
         .chrome button:hover {{ background-color: {hit}; }}\n\
         .chrome button:active, .chrome button.open {{ background-color: {hit}; color: {strong}; }}\n\
         .chrome label.chrome-title {{\n\
         \x20 font-family: {CHROME_FONT}; font-size: {TITLE_PX}px; font-weight: normal;\n\
         \x20 letter-spacing: {tracking}px;\n\
         }}\n\
         .chrome label.chrome-stat {{\n\
         \x20 font-family: {CHROME_FONT}; font-size: {STAT_PX}px;\n\
         \x20 font-feature-settings: \"tnum\"; color: {fg};\n\
         }}\n\
         .chrome .chrome-rule {{ color: {rule}; }}\n{}{}",
        menu_stylesheet(scheme),
        crate::palette::stylesheet(scheme)
    ) + &crate::sidebar::stylesheet(ground)
}

/// The two bars: the title bar above the page and the stats bar below it.
///
/// Both are plain boxes in a column with the Editor's scrolled window rather
/// than a header bar or a menubar (ADR 0009), so that the page sits between
/// them the way the oracle's does and hiding them gives the page the whole
/// window. The title bar carries the Library toggle, the title button and
/// the View button; the stats bar the count. The buttons fire their
/// Commands' actions by name, so an action not yet built does nothing
/// without greying the button.
#[derive(Clone)]
pub struct Bars {
    top: gtk::Overlay,
    bottom: gtk::Overlay,
    title: gtk::Label,
    stats: [gtk::Label; 3],
    /// The hairline under the title bar, shown once the page has scrolled
    /// past its top (`#chrome-top::after`).
    over: gtk::DrawingArea,
    /// The hairline above the stats bar, shown while the page continues
    /// below it (`#chrome-bottom::before`).
    under: gtk::DrawingArea,
    /// The View button's rows, lit the way Focus lights them.
    rows: gtk::DrawingArea,
    /// The Library toggle at the title bar's left, which steps aside while the
    /// Library is standing beside the page and carrying a toggle of its own
    /// ([`crate::sidebar`]), as the oracle's does.
    library: gtk::Button,
    /// The three menus, each under or over the button that opens it:
    /// Document, View, Stats, in [`Menu`]'s order.
    menus: [gtk::PopoverMenu; 3],
    /// Whether the bars are shown at all, and whether the stats bar is
    /// while they are.
    shown: Rc<Cell<Shown>>,
    focus: Rc<Cell<Focus>>,
    /// The ground the counts are inked for; the rest of the bars take theirs
    /// from the stylesheet.
    ground: Rc<Cell<Ground>>,
    /// The three counts the stats bar shows, kept so a ground change can
    /// re-ink them.
    counts: Rc<Cell<[(usize, &'static str); 3]>>,
}

/// The bars' two switches: whether they are shown at all, and whether the
/// stats bar is while they are.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Shown {
    bars: bool,
    stats: bool,
}

/// One answer for each bar: the title bar's above the page, the stats bar's
/// below it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Edges {
    /// The title bar's.
    pub top: bool,
    /// The stats bar's.
    pub bottom: bool,
}

impl Default for Bars {
    fn default() -> Self {
        Self::new()
    }
}

impl Bars {
    /// Builds both bars, empty and on the light ground.
    #[must_use]
    pub fn new() -> Self {
        let focus = Rc::new(Cell::new(Focus::Off));
        let ground = Rc::new(Cell::new(Ground::default()));

        let library = button(&[], icon(15, 15, library_icon), Some("win.library.toggle"));
        library.set_margin_start(LIBRARY_LEFT);

        let title = gtk::Label::builder()
            .css_classes(["chrome-title"])
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .single_line_mode(true)
            // The oracle centres the title's 16 px em box (`line-height: 1`)
            // and GTK centres the face's taller ascent-plus-descent, which
            // puts the baseline two pixels lower; the margin lifts it back.
            .margin_bottom(4)
            .build();
        // The chevron the Document menu shows while it is open (`.caretdown`
        // at `.55`), at no ink otherwise and taking its space throughout.
        let title_chevron = icon(9, 9, |area, cr| chevron_icon(area, cr, 1.0));
        title_chevron.set_margin_start(TITLE_GAP);
        title_chevron.set_opacity(0.0);
        let title_content = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        title_content.append(&title);
        title_content.append(&title_chevron);
        let title_button = button(&["chrome-title"], title_content, Some("win.chrome.doc"));

        let lit = Rc::clone(&focus);
        let rows = icon(15, 12, move |area, cr| rows_icon(area, cr, lit.get()));
        let chevron = icon(9, 9, |area, cr| chevron_icon(area, cr, 0.5));
        chevron.set_margin_start(CHEVRON_MARGIN);
        // A pixel lower than centred, where the frozen shot has it.
        chevron.set_margin_top(1);
        let view_content = gtk::Box::new(gtk::Orientation::Horizontal, CHEVRON_GAP);
        view_content.append(&rows);
        view_content.append(&chevron);
        let view = button(&[], view_content, Some("win.chrome.view"));
        view.set_margin_end(BAR_PAD);

        let top_bar = gtk::CenterBox::builder()
            .height_request(TOP_HEIGHT)
            .hexpand(true)
            .start_widget(&library)
            .center_widget(&title_button)
            .end_widget(&view)
            .build();
        let over = rule(gtk::Align::End);
        let top = bar(&["chrome", "chrome-top"], &top_bar, &over);

        let stats: [gtk::Label; 3] = std::array::from_fn(|_| {
            gtk::Label::builder()
                .css_classes(["chrome-stat"])
                .use_markup(true)
                .valign(gtk::Align::Center)
                .build()
        });
        let line = gtk::Box::new(gtk::Orientation::Horizontal, STAT_GAP);
        // Half a pixel up, for the reason the title's margin gives.
        line.set_margin_bottom(1);
        for stat in &stats {
            line.append(stat);
        }
        // No Command opens the Stats menu — the oracle's bar opens it on a
        // click and nothing else does — so the click asks the window
        // directly, the way `chrome.doc` and `chrome.view` reach it.
        let stats_button = button(&[], line, None);
        stats_button.connect_clicked(|button| {
            if let Some(window) = button.root().and_downcast::<Window>() {
                window.open_menu(Menu::Stats);
            }
        });
        let bottom_bar = gtk::CenterBox::builder()
            .height_request(BOTTOM_HEIGHT)
            .hexpand(true)
            .center_widget(&stats_button)
            .build();
        let under = rule(gtk::Align::Start);
        let bottom = bar(&["chrome", "chrome-bottom"], &bottom_bar, &under);

        let menus = [
            popover(&title_button, gtk::PositionType::Bottom, gtk::Align::Start),
            popover(&view, gtk::PositionType::Bottom, gtk::Align::End),
            popover(&stats_button, gtk::PositionType::Top, gtk::Align::Center),
        ];
        // The Document menu's chevron shows while its menu is up.
        let chevron_up = title_chevron.clone();
        menus[0].connect_show(move |_| chevron_up.set_opacity(CHEVRON_OPEN));
        let chevron_down = title_chevron.clone();
        menus[0].connect_closed(move |_| chevron_down.set_opacity(0.0));

        let bars = Self {
            top,
            bottom,
            title,
            stats,
            over,
            under,
            rows,
            library,
            menus,
            shown: Rc::new(Cell::new(Shown {
                bars: true,
                stats: true,
            })),
            focus,
            ground,
            counts: Rc::new(Cell::new([(0, "words"), (0, "characters"), (0, "read")])),
        };
        bars.set_count("");
        bars
    }

    /// Opens `menu` on `model`, its first row selected, over or under the
    /// button that owns it; a menu already up is closed first, so a second
    /// `F10` toggles.
    pub fn open_menu(&self, menu: Menu, model: &gio::Menu) {
        let popover = &self.menus[slot(menu)];
        if popover.is_visible() {
            popover.popdown();
            return;
        }
        for other in &self.menus {
            other.popdown();
        }
        popover.set_menu_model(Some(model));
        popover.popup();
    }

    /// Whether an open menu takes the keyboard and closes on `Esc` or a
    /// click outside it, which is what a menu does; `false` under
    /// `--deterministic`, where the compositor hands the window its focus
    /// after the menu is up and a menu holding a grab would be dismissed by
    /// that, before the shot.
    pub fn set_menus_grabbing(&self, grabbing: bool) {
        for menu in &self.menus {
            menu.set_autohide(grabbing);
        }
    }

    /// Closes whichever menu is up, for the Palette opening over it.
    pub fn close_menus(&self) {
        for menu in &self.menus {
            menu.popdown();
        }
    }

    /// The menu that is open now, if one is.
    #[must_use]
    pub fn open(&self) -> Option<Menu> {
        [Menu::Document, Menu::View, Menu::Stats]
            .into_iter()
            .find(|menu| self.menus[slot(*menu)].is_visible())
    }

    /// Shows the stats bar, or hides it, while the bars are shown:
    /// `chrome.stats`.
    pub fn set_stats_shown(&self, shown: bool) {
        let bars = self.shown.get().bars;
        self.shown.set(Shown { bars, stats: shown });
        self.apply_shown();
    }

    /// Whether the stats bar is shown while the bars are.
    #[must_use]
    pub fn stats_shown(&self) -> bool {
        self.shown.get().stats
    }

    /// Takes the menus off their buttons, for the buttons' disposal: a
    /// popover is a child GTK does not take down with its parent.
    pub fn dispose(&self) {
        for menu in &self.menus {
            menu.unparent();
        }
    }

    /// The title bar, to put above the page.
    #[must_use]
    pub fn top(&self) -> &gtk::Widget {
        self.top.upcast_ref()
    }

    /// The stats bar, to put below the page.
    #[must_use]
    pub fn bottom(&self) -> &gtk::Widget {
        self.bottom.upcast_ref()
    }

    /// Shows both bars, or hides both: `--chrome`, the `chrome` setting and
    /// `Ctrl+Shift+H`. Hidden bars take no space, so the page has the
    /// window.
    pub fn set_shown(&self, shown: bool) {
        let stats = self.shown.get().stats;
        self.shown.set(Shown { bars: shown, stats });
        self.apply_shown();
    }

    /// Whether the bars are shown.
    #[must_use]
    pub fn shown(&self) -> bool {
        self.top.is_visible()
    }

    /// Puts the two switches on the widgets: the title bar follows the
    /// bars, the stats bar follows both.
    fn apply_shown(&self) {
        let Shown { bars, stats } = self.shown.get();
        self.top.set_visible(bars);
        self.bottom.set_visible(bars && stats);
    }

    /// Names the Document in the title button.
    pub fn set_title(&self, title: &str) {
        self.title.set_text(title);
    }

    /// The title button's text.
    #[must_use]
    pub fn title(&self) -> glib::GString {
        self.title.text()
    }

    /// Steps the bars back or brings them forward, to the opacities the
    /// typing machine gives: a bar under 1 wears `faded`, and the stylesheet
    /// says what `faded` is and how long the way there takes. A class already
    /// on or already off is left alone, so a keystroke inside the window
    /// changes nothing on the widget.
    pub fn set_fade(&self, title: f64, stats: f64) {
        for (bar, alpha) in [(&self.top, title), (&self.bottom, stats)] {
            let faded = alpha < 1.0;
            if bar.has_css_class("faded") != faded {
                if faded {
                    bar.add_css_class("faded");
                } else {
                    bar.remove_css_class("faded");
                }
            }
        }
    }

    /// Which bars are stepped back.
    #[must_use]
    pub fn faded(&self) -> Edges {
        Edges {
            top: self.top.has_css_class("faded"),
            bottom: self.bottom.has_css_class("faded"),
        }
    }

    /// Counts `text` into the stats bar: words, characters and reading time,
    /// the oracle's default three fields.
    ///
    /// Counted as the Document is shown, and again on idle 500 ms after the
    /// last keystroke of a run ([`typing::Typing::takes_recount`]); never on
    /// the keystroke path.
    pub fn set_count(&self, text: &str) {
        let words = words(text);
        let characters = text.chars().count();
        self.counts.set([
            (words, if words == 1 { "word" } else { "words" }),
            (characters, "characters"),
            (words, "read"),
        ]);
        self.ink_counts();
    }

    /// The stats bar's three cells, as `("188", "words")`.
    #[must_use]
    pub fn count(&self) -> [(String, &'static str); 3] {
        let [(words, word), (characters, characters_label), (_, read)] = self.counts.get();
        [
            (grouped(words), word),
            (grouped(characters), characters_label),
            (reading_time(words), read),
        ]
    }

    /// Lights the View button's rows the way Focus lights the page.
    pub fn set_focus(&self, focus: Focus) {
        self.focus.set(focus);
        self.rows.queue_draw();
    }

    /// Shows or hides the title bar's Library toggle.
    ///
    /// Hidden while the sidebar stands beside the page, because the pane's own
    /// head carries the toggle that shuts it and two of them in one frame is
    /// one too many; shown again the moment the pane goes, which is the
    /// oracle's arrangement (`files.js`, `.lib-head`).
    pub fn set_library_toggle_shown(&self, shown: bool) {
        self.library.set_visible(shown);
    }

    /// Re-inks the numbers for `ground`. The rest of the bars follow the
    /// stylesheet on their own.
    pub fn set_ground(&self, ground: Ground) {
        self.ground.set(ground);
        self.ink_counts();
    }

    /// Follows the page's scrolling: the hairlines stand where the page
    /// continues past a bar (`chrome.js` `over`/`under`).
    pub fn follow(&self, scroller: &gtk::ScrolledWindow) {
        let adjustment = scroller.vadjustment();
        let (over, under) = (self.over.clone(), self.under.clone());
        adjustment.connect_value_changed(move |adjustment| scrolled(adjustment, &over, &under));
        let (over, under) = (self.over.clone(), self.under.clone());
        adjustment.connect_changed(move |adjustment| scrolled(adjustment, &over, &under));
        scrolled(&adjustment, &self.over, &self.under);
    }

    /// Which hairlines are shown: the one under the title bar, the one above
    /// the stats bar.
    #[must_use]
    pub fn rules(&self) -> Edges {
        Edges {
            top: self.over.is_visible(),
            bottom: self.under.is_visible(),
        }
    }

    /// Writes the counts into the labels: the number strong and the label in
    /// the chrome's grey (`.stat b`).
    fn ink_counts(&self) {
        let strong = self
            .ground
            .get()
            .colours
            .colour(Role::ChromeFgStrong)
            .to_hex();
        for (label, (number, name)) in self.stats.iter().zip(self.count()) {
            let number = glib::markup_escape_text(&number);
            label.set_markup(&format!(
                "<span weight=\"500\" foreground=\"{strong}\">{number}</span> {name}"
            ));
        }
    }
}

/// A bar: its content with a hairline laid over one edge.
fn bar(classes: &[&str], content: &impl IsA<gtk::Widget>, rule: &gtk::DrawingArea) -> gtk::Overlay {
    let overlay = gtk::Overlay::builder()
        .css_classes(classes)
        .child(content)
        .build();
    overlay.add_overlay(rule);
    overlay
}

/// A bar button showing `child`, firing `action` when clicked.
///
/// Fired by name rather than bound with `set_action_name`, because GTK greys
/// a button whose action is disabled and a Command not built yet is exactly
/// that: the oracle's buttons stand at full ink whatever is behind them.
fn button(
    classes: &[&str],
    child: impl IsA<gtk::Widget>,
    action: Option<&'static str>,
) -> gtk::Button {
    let button = gtk::Button::builder()
        .css_classes(classes)
        .child(&child)
        .valign(gtk::Align::Center)
        .can_focus(false)
        .focus_on_click(false)
        .build();
    if let Some(action) = action {
        button.connect_clicked(move |button| {
            // A Command not built yet has a disabled action, and GTK answers
            // a disabled action with `false`; that is the click doing
            // nothing.
            let _ = button.activate_action(action, None);
        });
    }
    button
}

/// The menu under (or, for the Stats menu, over) `button`, empty until it is
/// opened on a model.
///
/// No arrow, as the oracle's menus have none; `NESTED` so the Syntax
/// highlight submenu opens inside the same popover rather than beside it.
/// While it is up the button reads as pressed (`.open`).
fn popover(
    button: &gtk::Button,
    position: gtk::PositionType,
    align: gtk::Align,
) -> gtk::PopoverMenu {
    let popover =
        gtk::PopoverMenu::from_model_full(&gio::Menu::new(), gtk::PopoverMenuFlags::NESTED);
    popover.add_css_class("chrome-menu");
    popover.set_has_arrow(false);
    popover.set_position(position);
    popover.set_halign(align);
    let gap = match position {
        gtk::PositionType::Top => MENU_GAP_ABOVE,
        _ => MENU_GAP_BELOW,
    };
    popover.set_offset(0, gap);
    popover.set_parent(button);
    // The first row selected, which is what the oracle shows once a menu is
    // open and the down arrow pressed. Selected by its state flag rather
    // than by taking the keyboard focus, which stays on the page so that
    // the caret is still lit behind the menu; GTK moves the flag with the
    // pointer and the arrow keys from there. Set once the popup is on the
    // compositor, because GTK builds the rows as it maps.
    popover.connect_map(|popover| {
        if let Some(row) = first_row(popover.upcast_ref()) {
            row.set_state_flags(gtk::StateFlags::SELECTED, false);
        }
    });
    let opened = button.clone();
    popover.connect_show(move |_| opened.add_css_class("open"));
    let closed = button.clone();
    popover.connect_closed(move |_| closed.remove_css_class("open"));
    popover
}

/// The first row GTK built for a menu's model: its first `modelbutton`, in
/// drawing order.
fn first_row(widget: &gtk::Widget) -> Option<gtk::Widget> {
    if widget.css_name() == "modelbutton" {
        return Some(widget.clone());
    }
    let mut child = widget.first_child();
    while let Some(next) = child {
        if let Some(row) = first_row(&next) {
            return Some(row);
        }
        child = next.next_sibling();
    }
    None
}

/// Which of the three popovers is `menu`'s.
const fn slot(menu: Menu) -> usize {
    match menu {
        Menu::Document => 0,
        Menu::View => 1,
        Menu::Stats => 2,
    }
}

/// A bar's hairline (`transform: scaleY(.5)` of a 1 px rule), in the rule's
/// colour, hidden until the page scrolls under it.
fn rule(edge: gtk::Align) -> gtk::DrawingArea {
    hairline("chrome-rule", edge, false)
}

/// A hairline one device pixel high along `edge` of the widget it is put
/// in, taking its colour from `class`'s CSS `color`: the bars' rules and the
/// Palette's line under its field.
pub(crate) fn hairline(class: &str, edge: gtk::Align, visible: bool) -> gtk::DrawingArea {
    let rule = gtk::DrawingArea::builder()
        .css_classes([class])
        .content_height(1)
        .hexpand(true)
        .valign(edge)
        .visible(visible)
        .can_target(false)
        .build();
    rule.set_draw_func(move |area, cr, width, height| {
        // Half a logical pixel is one device pixel at the Gate's scale, and
        // with no antialiasing it is exactly one row.
        cr.set_antialias(cairo::Antialias::None);
        source(area, cr, 1.0);
        let top = if edge == gtk::Align::Start {
            0.0
        } else {
            f64::from(height) - 0.5
        };
        cr.rectangle(0.0, top, f64::from(width), 0.5);
        let _ = cr.fill();
    });
    rule
}

/// An icon `width` by `height` logical pixels, drawn by `draw`, which takes
/// its ink from the widget's CSS colour through [`source`].
pub(crate) fn icon(
    width: i32,
    height: i32,
    draw: impl Fn(&gtk::DrawingArea, &cairo::Context) + 'static,
) -> gtk::DrawingArea {
    let area = gtk::DrawingArea::builder()
        .content_width(width)
        .content_height(height)
        .halign(gtk::Align::Center)
        .valign(gtk::Align::Center)
        .can_target(false)
        .build();
    area.set_draw_func(move |area, cr, _, _| draw(area, cr));
    area
}

/// Sets the source to the widget's CSS `color` at `alpha` of it, which is how
/// an icon or a rule takes the stylesheet's colour without being told it.
pub(crate) fn source(area: &gtk::DrawingArea, cr: &cairo::Context, alpha: f64) {
    // GTK 4.10 deprecated `color()` for reading the style through a snapshot;
    // a draw function has no snapshot, and the property it reads is the one
    // the stylesheet sets.
    #[allow(deprecated)] // The one way to read CSS `color` inside a draw function.
    let colour = area.color();
    cr.set_source_rgba(
        f64::from(colour.red()),
        f64::from(colour.green()),
        f64::from(colour.blue()),
        f64::from(colour.alpha()) * alpha,
    );
}

/// The Library toggle's icon (`files.js` `I.panel`): a 16-unit panel drawn
/// at 15 px, a rounded frame with a divider a third of the way across. Half
/// a pixel down from centred, which is where the frozen `bars` shot has it
/// beside the View button (round 2 read the two a device row apart).
fn library_icon(area: &gtk::DrawingArea, cr: &cairo::Context) {
    source(area, cr, 1.0);
    cr.translate(0.0, 0.5);
    cr.scale(15.0 / 16.0, 15.0 / 16.0);
    cr.set_line_width(1.2);
    rounded(cr, 1.6, 2.6, 12.8, 10.8, 2.2);
    let _ = cr.stroke();
    cr.move_to(6.4, 2.6);
    cr.line_to(6.4, 13.4);
    let _ = cr.stroke();
}

/// The View button's icon (`chrome.js` `rowsIcon`): four rows of text, lit
/// the way Focus lights the page — an even block with Focus off, one row for
/// a sentence, three for a paragraph.
fn rows_icon(area: &gtk::DrawingArea, cr: &cairo::Context, focus: Focus) {
    let lit = match focus {
        Focus::Off => [0.5, 0.5, 0.5, 0.5],
        Focus::On(FocusScope::Sentence) => [0.22, 1.0, 0.22, 0.22],
        Focus::On(FocusScope::Paragraph) => [0.22, 1.0, 1.0, 1.0],
    };
    let widths = [15.0, 15.0, 15.0, 9.0];
    for (row, (width, alpha)) in widths.into_iter().zip(lit).enumerate() {
        source(area, cr, alpha);
        rounded(cr, 0.0, 3.4 * row as f64, width, 1.6, 0.8);
        let _ = cr.fill();
    }
}

/// A chevron (`chrome.js` `CHEV`) at `alpha` of the ink: half beside the
/// View icon (`.chev { opacity: .5 }`), whole beside the title.
fn chevron_icon(area: &gtk::DrawingArea, cr: &cairo::Context, alpha: f64) {
    source(area, cr, alpha);
    cr.set_line_width(1.3);
    cr.set_line_cap(cairo::LineCap::Round);
    cr.set_line_join(cairo::LineJoin::Round);
    cr.move_to(1.6, 3.3);
    cr.line_to(4.5, 6.1);
    cr.line_to(7.4, 3.3);
    let _ = cr.stroke();
}

/// A rectangle with corners of `radius`, as a path.
fn rounded(cr: &cairo::Context, x: f64, y: f64, width: f64, height: f64, radius: f64) {
    use std::f64::consts::FRAC_PI_2;
    cr.new_sub_path();
    cr.arc(x + width - radius, y + radius, radius, -FRAC_PI_2, 0.0);
    cr.arc(
        x + width - radius,
        y + height - radius,
        radius,
        0.0,
        FRAC_PI_2,
    );
    cr.arc(
        x + radius,
        y + height - radius,
        radius,
        FRAC_PI_2,
        2.0 * FRAC_PI_2,
    );
    cr.arc(
        x + radius,
        y + radius,
        radius,
        2.0 * FRAC_PI_2,
        3.0 * FRAC_PI_2,
    );
    cr.close_path();
}

/// Shows each hairline where the page continues past its bar
/// (`chrome.js:262-272`): `over` once scrolled more than two pixels past
/// the top, `under` while more than two pixels of page remain below.
fn scrolled(adjustment: &gtk::Adjustment, over: &gtk::DrawingArea, under: &gtk::DrawingArea) {
    let value = adjustment.value();
    over.set_visible(value > 2.0);
    under.set_visible(value + adjustment.page_size() < adjustment.upper() - 2.0);
}

/// `n` with thousands separated, as `toLocaleString` writes it.
fn grouped(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, digit) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

/// How long `words` take to read at [`WORDS_PER_MINUTE`], as the oracle's
/// `readTime` writes it: under 45 seconds is `< 1 min`, then whole minutes,
/// then hours and minutes.
fn reading_time(words: usize) -> String {
    let seconds = (words as f64 / WORDS_PER_MINUTE * 60.0).round();
    if seconds < 45.0 {
        return "< 1 min".to_owned();
    }
    let minutes = (seconds / 60.0).round() as u64;
    if minutes < 60 {
        return format!("{minutes} min");
    }
    let (hours, minutes) = (minutes / 60, minutes % 60);
    if minutes == 0 {
        format!("{hours} h")
    } else {
        format!("{hours} h {minutes} min")
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use quill_engine::shortcuts::Chord;

    use super::*;

    /// A map with one scope's actions on it, and the ids fired through it.
    fn map(scope: Scope) -> (gio::SimpleActionGroup, Rc<RefCell<Vec<&'static str>>>) {
        let fired = Rc::new(RefCell::new(Vec::new()));
        let map = gio::SimpleActionGroup::new();
        let log = Rc::clone(&fired);
        register(
            &map,
            scope,
            Rc::new(move |command| log.borrow_mut().push(command.id)),
        );
        (map, fired)
    }

    #[test]
    fn every_chord_in_the_table_reaches_its_action_by_name() {
        let (map, fired) = map(Scope::Win);
        for command in COMMANDS
            .iter()
            .filter(|command| command.scope == Scope::Win)
        {
            for chord in command.chords() {
                let reached = commands::by_chord(chord).expect(chord);
                assert_eq!(reached.id, command.id, "{chord}");
                fired.borrow_mut().clear();
                map.activate_action(reached.id, None);
                let expected: &[&str] = if command.built { &[command.id] } else { &[] };
                assert_eq!(
                    fired.borrow().as_slice(),
                    expected,
                    "{chord} -> {}",
                    command.id
                );
            }
        }
    }

    #[test]
    fn an_alias_reaches_the_same_action_as_its_labelled_chord() {
        for (alias, labelled) in [
            ("F9", "Ctrl+E"),
            ("Alt+Shift+N", "Ctrl+Shift+L"),
            ("Ctrl+Shift+P", "Ctrl+K"),
        ] {
            let by_alias = commands::by_chord(alias).expect(alias);
            let by_default = commands::by_chord(labelled).expect(labelled);
            assert_eq!(by_alias.id, by_default.id);
            assert_eq!(by_default.default(), Some(labelled));
            assert!(by_alias.aliases().contains(&alias));
        }
    }

    /// Whether GTK would install this accelerator, asked the way a test with
    /// no display can ask it.
    ///
    /// `gtk::accelerator_parse` needs a display, so the two halves of an
    /// accelerator are checked apart: the modifier tags against the names GTK
    /// gives them, the key name through GDK's own table, which
    /// `gtk::gdk::Key::from_name` reads without one. The halves are
    /// [`Chord`]'s, which is the one walk over `<Tag>` groups there is.
    fn installs(chord: &Chord) -> bool {
        chord
            .modifiers()
            .all(|tag| ["Control", "Shift", "Alt"].contains(&tag))
            && gtk::gdk::Key::from_name(chord.key())
                .is_some_and(|key| key != gtk::gdk::Key::VoidSymbol)
    }

    #[test]
    fn gdk_knows_every_key_name_the_table_installs() {
        for command in COMMANDS {
            let accels = command.accels();
            assert_eq!(accels.len(), command.chords().count(), "{}", command.id);
            for accel in accels {
                let chord =
                    Chord::parse(&accel).unwrap_or_else(|| panic!("{}: {accel}", command.id));
                assert!(installs(&chord), "{}: {accel}", command.id);
            }
        }
    }

    /// Every chord `docs/shortcuts.md` names — the Commands' and the two
    /// lists' — is one GTK can install, once the engine has read its shape.
    ///
    /// The engine's half of this is
    /// `quill_engine::shortcuts::Chord`'s: it says the chord has the shape of
    /// a chord, and says nothing about the key name, which is GDK's to know
    /// (ADR 0008). This is the other half, over the same chords.
    #[test]
    fn gtk_installs_every_chord_the_shape_parser_accepts_here() {
        let table = COMMANDS
            .iter()
            .flat_map(Command::chords)
            .chain(commands::RESERVED.iter().copied())
            .chain(commands::OFF_LIMITS.iter().copied());
        let mut seen = 0;
        for written in table {
            let accel = commands::accel(written).unwrap_or_else(|| panic!("{written}"));
            let chord = Chord::parse(&accel).unwrap_or_else(|| panic!("{written} is {accel}"));
            assert!(installs(&chord), "{written} is {chord}");
            seen += 1;
        }
        let bound: usize = COMMANDS
            .iter()
            .map(|command| command.chords().count())
            .sum();
        assert_eq!(
            seen,
            bound + commands::RESERVED.len() + commands::OFF_LIMITS.len()
        );
    }

    /// One chord GDK has no key for refuses the writer's whole entry: the
    /// Command keeps the defaults the entry was meant to replace, and the
    /// refusal quotes the entry as it was written rather than the one chord.
    #[test]
    fn a_chord_naming_no_key_refuses_the_whole_entry_and_leaves_the_defaults() {
        let (settings, notes) = quill_engine::settings::Settings::parse(
            "[shortcuts]\n\"library.toggle\" = [\"F9\", \"<Control>frobnicate\"]\n",
        );
        assert_eq!(notes, Vec::<String>::new(), "the fixture reads cleanly");
        let shortcuts = settings.shortcuts();
        assert_eq!(shortcuts.refusals, Vec::new(), "the engine took the shape");
        let command = commands::by_id("library.toggle").expect("the registry has it");
        let (accels, refused) = installed(
            command,
            &shortcuts.chords[command.id],
            shortcuts.entries.get(command.id).map(String::as_str),
            installs,
        );
        assert_eq!(accels, command.accels());
        let refused = refused.expect("the entry names a key GDK has none of");
        assert_eq!(refused.id, "library.toggle");
        assert_eq!(
            refused.line,
            "\"library.toggle\" = [\"F9\", \"<Control>frobnicate\"]"
        );
        assert!(refused.reason.contains("names no key"), "{refused}");
    }

    /// A Command the file never named is installed with the registry's own,
    /// and nothing about it is refused.
    #[test]
    fn a_command_the_file_left_alone_is_installed_with_its_defaults() {
        let shortcuts = quill_engine::settings::Settings::default().shortcuts();
        let command = commands::by_id("library.toggle").expect("the registry has it");
        let (accels, refused) = installed(
            command,
            &shortcuts.chords[command.id],
            shortcuts.entries.get(command.id).map(String::as_str),
            installs,
        );
        assert_eq!(accels, command.accels());
        assert_eq!(refused, None);
    }

    /// The four Commands the File handling spec's second ticket builds are
    /// out of the disabled set: their rows are live and each fires.
    #[test]
    fn the_four_file_commands_are_enabled_and_fire() {
        let (window, fired) = map(Scope::Win);
        for id in ["file.open", "file.save", "file.saveAs"] {
            assert!(window.is_action_enabled(id), "{id}");
            window.activate_action(id, None);
        }
        assert_eq!(
            fired.borrow().as_slice(),
            ["file.open", "file.save", "file.saveAs"]
        );
        let (app, app_fired) = map(Scope::App);
        assert!(app.is_action_enabled("window.new"));
        app.activate_action("window.new", None);
        assert_eq!(app_fired.borrow().as_slice(), ["window.new"]);
    }

    #[test]
    fn a_disabled_commands_activation_returns_without_effect() {
        let (map, fired) = map(Scope::Win);
        assert!(!commands::by_id("export.open").unwrap().built);
        assert!(!map.is_action_enabled("export.open"));
        map.activate_action("export.open", None);
        // The Stats menu's fields are the Stats spec's (#30), so the whole
        // radio group is disabled.
        map.activate_action("stats", Some(&"words".to_variant()));
        assert!(fired.borrow().is_empty());
        map.activate_action("focus.toggle", None);
        assert_eq!(fired.borrow().as_slice(), ["focus.toggle"]);
    }

    /// A radio group whose members are built fires the member the value
    /// names: the View menu's Typeface and Focus radios, the Palette's three
    /// themes.
    #[test]
    fn a_built_radio_group_fires_the_member_its_value_names() {
        let (map, fired) = map(Scope::Win);
        for group in ["face", "focus_scope", "theme"] {
            assert!(map.is_action_enabled(group), "{group} has built members");
        }
        map.activate_action("face", Some(&"mono".to_variant()));
        map.activate_action("focus_scope", Some(&"paragraph".to_variant()));
        map.activate_action("theme", Some(&"auto".to_variant()));
        map.activate_action("face", Some(&"serif".to_variant()));
        assert_eq!(
            fired.borrow().as_slice(),
            ["font.mono", "focus.paragraph", "theme.auto"],
            "a value no member carries fires nothing"
        );
    }

    /// The eight View › Template Commands are built: the radio group fires
    /// the Template the value names, and each of the three toggles fires its
    /// own Command (#271).
    #[test]
    fn the_eight_template_commands_are_built_and_fire() {
        let (map, fired) = map(Scope::Win);
        assert!(map.is_action_enabled("template"), "the radio group");
        for value in TemplateName::VALUES {
            map.activate_action("template", Some(&(*value).to_variant()));
        }
        let toggles = [
            "template.centerHeadings",
            "template.numberHeadings",
            "template.indentParagraphs",
        ];
        for id in toggles {
            assert!(map.is_action_enabled(id), "{id}");
            map.activate_action(id, None);
        }
        assert_eq!(
            fired.borrow().as_slice(),
            [
                "template.modern",
                "template.classic",
                "template.manuscriptMono",
                "template.manuscriptDuo",
                "template.manuscriptQuattro",
                "template.centerHeadings",
                "template.numberHeadings",
                "template.indentParagraphs",
            ]
        );
    }

    /// The modes the two reflect tests read, with the Preview pane's pair as
    /// the case asks for them.
    fn preview_modes(preview: bool, preview_layout: &'static str) -> Modes {
        Modes {
            focus: true,
            focus_scope: "paragraph",
            typewriter: false,
            live: true,
            dark: true,
            theme: "auto",
            face: "quattro",
            fullscreen: false,
            bars: false,
            stats: false,
            library: true,
            preview,
            preview_layout,
            template: "classic",
            center_headings: true,
            number_headings: true,
            indent_paragraphs: false,
        }
    }

    #[test]
    fn the_stateful_actions_show_the_modes_they_are_given() {
        let (map, _) = map(Scope::Win);
        reflect(&map, preview_modes(true, "full"));
        let state = |name: &str| map.action_state(name).unwrap();
        assert_eq!(state("focus.toggle").get::<bool>(), Some(true));
        assert_eq!(state("chrome.stats").get::<bool>(), Some(false));
        // "Hide Bars" is ticked when the bars are hidden.
        assert_eq!(state("chrome.toggle").get::<bool>(), Some(true));
        assert_eq!(state("typewriter.toggle").get::<bool>(), Some(false));
        assert_eq!(state("live.toggle").get::<bool>(), Some(true));
        assert_eq!(state("theme.toggle").get::<bool>(), Some(true));
        assert_eq!(
            state("focus_scope").get::<String>().as_deref(),
            Some("paragraph")
        );
        assert_eq!(state("theme").get::<String>().as_deref(), Some("auto"));
        assert_eq!(state("face").get::<String>().as_deref(), Some("quattro"));
        assert_eq!(state("library.toggle").get::<bool>(), Some(true));
        assert_eq!(
            state("preview.full").get::<bool>(),
            Some(true),
            "the pane is open and Full is what it shows"
        );
        assert_eq!(state("preview.split").get::<bool>(), Some(false));
        assert_eq!(
            state("template").get::<String>().as_deref(),
            Some("classic")
        );
        assert_eq!(state("template.centerHeadings").get::<bool>(), Some(true));
        assert_eq!(state("template.numberHeadings").get::<bool>(), Some(true));
        assert_eq!(
            state("template.indentParagraphs").get::<bool>(),
            Some(false)
        );
    }

    /// With the pane away neither Preview row is ticked, whatever layout the
    /// pane last showed: the rows are the layout on screen and there is none
    /// (#263).
    #[test]
    fn the_preview_rows_are_both_clear_with_the_pane_away() {
        let (map, _) = map(Scope::Win);
        for layout in ["full", "split"] {
            reflect(&map, preview_modes(false, layout));
            let state = |name: &str| map.action_state(name).unwrap();
            assert_eq!(state("preview.full").get::<bool>(), Some(false));
            assert_eq!(state("preview.split").get::<bool>(), Some(false));
        }
    }

    #[test]
    fn the_bars_key_reaches_its_action_now_that_the_bars_are_built() {
        let (map, fired) = map(Scope::Win);
        assert!(map.is_action_enabled("chrome.toggle"));
        map.activate_action("chrome.toggle", None);
        assert_eq!(fired.borrow().as_slice(), ["chrome.toggle"]);
    }

    /// `F10` opens the View menu: the chord reaches `chrome.view`, whose
    /// Command opens that menu and no other; `chrome.doc` opens the
    /// Document menu; a Command that opens no menu answers none.
    #[test]
    fn f10_reaches_the_command_that_opens_the_view_menu() {
        let (map, fired) = map(Scope::Win);
        let reached = commands::by_chord("F10").expect("F10 is in the table");
        assert_eq!(reached.id, "chrome.view");
        map.activate_action(reached.id, None);
        assert_eq!(fired.borrow().as_slice(), ["chrome.view"]);
        assert_eq!(opens("chrome.view"), Some(Menu::View));
        assert_eq!(opens("chrome.doc"), Some(Menu::Document));
        assert_eq!(opens("chrome.stats"), None);
    }

    /// The bars are the oracle's height (`chrome.css` `--bar-top`,
    /// `--bar-bottom`), and their sheet is set in the table's greys for each
    /// ground.
    #[test]
    fn the_bars_are_the_oracles_height_and_take_the_tables_greys() {
        assert_eq!((TOP_HEIGHT, BOTTOM_HEIGHT), (32, 26));
        for scheme in [Scheme::Light, Scheme::Dark] {
            let ground = Ground::of(scheme);
            let sheet = stylesheet(ground);
            let colours = ground.colours;
            for role in [Role::ChromeFg, Role::ChromeFgStrong] {
                let hex = colours.colour(role).to_hex();
                assert!(
                    sheet.contains(&hex),
                    "{scheme:?}: {role:?} {hex} in\n{sheet}"
                );
            }
            let rule = colours.colour(Role::Rule).to_css();
            assert!(sheet.contains(&rule), "{scheme:?}: the rule {rule}");
            assert!(sheet.contains(&format!("{TITLE_PX}px")));
            assert!(sheet.contains(&format!("{STAT_PX}px")));
        }
    }

    /// The stats bar's three cells for `ref/sample.md` are the numbers the
    /// oracle's frozen `bars` shot shows, and its empty Document's are the
    /// `empty` shot's.
    #[test]
    fn the_count_is_the_oracles_for_the_sample_and_for_nothing() {
        let sample =
            std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../ref/sample.md"))
                .expect("ref/sample.md");
        // The count itself is `quill_engine::stats`' test; this one holds
        // the two cells the bar makes of it.
        assert_eq!(sample.chars().count(), 961);
        assert_eq!(reading_time(words(&sample)), "1 min");
        assert_eq!(words(""), 0);
        assert_eq!(reading_time(0), "< 1 min");
    }

    /// The typing state's opacities are in the sheet as the two `faded`
    /// rules, on the transition the whole chrome shares.
    #[test]
    fn the_sheet_carries_the_typing_states_opacities() {
        let sheet = stylesheet(Ground::default());
        assert!(sheet.contains(".chrome-top.faded { opacity: 0; }"));
        assert!(sheet.contains(".chrome-bottom.faded { opacity: 0.38; }"));
        assert!(sheet.contains("transition: opacity"));
    }

    #[test]
    fn numbers_are_grouped_and_times_written_the_way_the_oracle_writes_them() {
        assert_eq!(grouped(0), "0");
        assert_eq!(grouped(961), "961");
        assert_eq!(grouped(1_234), "1,234");
        assert_eq!(grouped(1_234_567), "1,234,567");
        // 176 words are 44 seconds at 238 a minute; 177 are 45.
        assert_eq!(reading_time(176), "< 1 min");
        assert_eq!(reading_time(177), "1 min");
        assert_eq!(reading_time(2_380), "10 min");
        assert_eq!(reading_time(14_280), "1 h");
        assert_eq!(reading_time(15_470), "1 h 5 min");
    }
}
