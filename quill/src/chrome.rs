//! Chrome: the Command registry installed as actions and accelerators, the
//! one place in the app a chord is bound (#119); the bars, menus and Palette
//! of the Chrome and menus Piece follow it (#120–#122).
//!
//! [`quill_engine::commands`] is the data; this module turns each Command
//! into a `GSimpleAction` on the window (`win.<id>`) or the application
//! (`app.quit`, `app.window.new`) and installs its chords with
//! `set_accels_for_action`. Toggles are stateful booleans and each radio
//! group is one stateful string action, so the menus' checks and radios
//! (#121) read the same actions the chords fire. A Command whose feature is
//! not built is registered disabled: its chord does nothing and its row will
//! render greyed. Two key controllers remain elsewhere and bind no chord: the
//! harness's capture-phase stamp (`harness::watch`, #64) and the Editor's
//! caret-kind controller (#107).

use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gio, glib};
use quill_engine::commands::{self, COMMANDS, Command, Kind, Scope};
use quill_engine::focus::Focus;
use quill_engine::focus::typewriter::Typewriter;
use quill_engine::settings::Choice;
use quill_engine::theme::Scheme;

use crate::window::Window;

/// What the stateful actions show: the modes as the session holds them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Modes {
    /// Focus is on.
    pub focus: bool,
    /// The Focus scope, `sentence` or `paragraph`, on or off.
    pub focus_scope: &'static str,
    /// Typewriter is on.
    pub typewriter: bool,
    /// The ground shown is the dark one, whatever `theme` says.
    pub dark: bool,
    /// The theme setting, `auto`, `light` or `dark`.
    pub theme: &'static str,
    /// The face, `duo`, `quattro` or `mono`.
    pub face: &'static str,
    /// The window fills the screen.
    pub fullscreen: bool,
}

/// What a fired Command does, given the Command.
type Run = Rc<dyn Fn(&'static Command)>;

/// Installs the application's half: every chord in the table, and the
/// `app.` actions.
///
/// The chords are installed for every Command, built or not, because GTK
/// keeps them on the application and an unbuilt Command's action is disabled,
/// which is what makes its chord do nothing. Called once, before the first
/// window.
pub fn install(app: &gtk::Application) {
    for command in COMMANDS {
        let accels = command.accels();
        let accels: Vec<&str> = accels.iter().map(String::as_str).collect();
        app.set_accels_for_action(&command.action(), &accels);
    }
    let fired = app.clone();
    register(
        app,
        Scope::App,
        Rc::new(move |command| run_app(&fired, command)),
    );
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
    // The compositor can fill the screen without `F11` being pressed, so the
    // check follows the window rather than the Command.
    window.connect_fullscreened_notify(|window| reflect(window, window.modes()));
    reflect(window, window.modes());
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
pub fn register(map: &impl IsA<gio::ActionMap>, scope: Scope, run: Run) {
    for command in COMMANDS.iter().filter(|command| command.scope == scope) {
        let action = match command.kind {
            Kind::Check => gio::SimpleAction::new_stateful(command.id, None, &false.to_variant()),
            Kind::Plain | Kind::Radio { .. } => gio::SimpleAction::new(command.id, None),
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
    set("theme.toggle", modes.dark.to_variant());
    set("window.fullscreen", modes.fullscreen.to_variant());
    set("focus_scope", modes.focus_scope.to_variant());
    set("theme", modes.theme.to_variant());
    set("face", modes.face.to_variant());
}

/// What the session shows, as the actions show it.
pub fn modes(session: &crate::session::Session, fullscreen: bool) -> Modes {
    let (focus, focus_scope) = match session.focus() {
        Focus::On(scope) => (true, scope.as_str()),
        Focus::Off => (false, session.settings().focus_scope.as_str()),
    };
    Modes {
        focus,
        focus_scope,
        typewriter: matches!(session.typewriter(), Typewriter::On(_)),
        dark: session.scheme() == Scheme::Dark,
        theme: session.theme().as_str(),
        face: session.settings().face.as_str(),
        fullscreen,
    }
}

/// The application's Commands.
fn run_app(app: &gtk::Application, command: &Command) {
    if command.id == "app.quit" {
        app.quit();
    }
}

/// The window's Commands, each reaching the code its key reached before
/// #119, then every window's actions set to what moved.
fn run_window(window: &Window, command: &Command) {
    match command.id {
        "font.bigger" => window.step_size(crate::window::Step::Bigger),
        "font.smaller" => window.step_size(crate::window::Step::Smaller),
        "font.reset" => window.step_size(crate::window::Step::Default),
        "theme.toggle" => window.toggle_scheme(),
        "focus.toggle" => window.toggle_focus(),
        "focus.swap" => window.swap_focus_scope(),
        "typewriter.toggle" => window.toggle_typewriter(),
        "window.fullscreen" if window.is_fullscreen() => window.unfullscreen(),
        "window.fullscreen" => window.fullscreen(),
        "window.close" => window.close(),
        _ => {}
    }
    if let Some(app) = window.application() {
        reflect_windows(&app);
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;

    /// A map with every `win.` action on it, and the ids fired through it.
    fn map() -> (gio::SimpleActionGroup, Rc<RefCell<Vec<&'static str>>>) {
        let fired = Rc::new(RefCell::new(Vec::new()));
        let map = gio::SimpleActionGroup::new();
        let log = Rc::clone(&fired);
        register(
            &map,
            Scope::Win,
            Rc::new(move |command| log.borrow_mut().push(command.id)),
        );
        (map, fired)
    }

    #[test]
    fn every_chord_in_the_table_reaches_its_action_by_name() {
        let (map, fired) = map();
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

    #[test]
    fn gdk_knows_every_key_name_the_table_installs() {
        // `gtk::accelerator_parse` needs a display, so the two halves of an
        // accelerator are checked apart: the modifier tags against the three
        // GTK names, the key name through GDK's own table.
        for command in COMMANDS {
            let accels = command.accels();
            assert_eq!(accels.len(), command.chords().count(), "{}", command.id);
            for accel in accels {
                let mut rest = accel.as_str();
                while let Some(after) = rest.strip_prefix('<') {
                    let (tag, tail) = after.split_once('>').expect(&accel);
                    assert!(
                        ["Control", "Shift", "Alt"].contains(&tag),
                        "{}: {accel}",
                        command.id
                    );
                    rest = tail;
                }
                let key = gtk::gdk::Key::from_name(rest).expect(&accel);
                assert_ne!(key, gtk::gdk::Key::VoidSymbol, "{}: {accel}", command.id);
            }
        }
    }

    #[test]
    fn a_disabled_commands_activation_returns_without_effect() {
        let (map, fired) = map();
        assert!(!commands::by_id("file.open").unwrap().built);
        assert!(!map.is_action_enabled("file.open"));
        map.activate_action("file.open", None);
        map.activate_action("face", Some(&"mono".to_variant()));
        assert!(fired.borrow().is_empty());
        map.activate_action("focus.toggle", None);
        assert_eq!(fired.borrow().as_slice(), ["focus.toggle"]);
    }

    #[test]
    fn the_stateful_actions_show_the_modes_they_are_given() {
        let (map, _) = map();
        let modes = Modes {
            focus: true,
            focus_scope: "paragraph",
            typewriter: false,
            dark: true,
            theme: "auto",
            face: "quattro",
            fullscreen: false,
        };
        reflect(&map, modes);
        let state = |name: &str| map.action_state(name).unwrap();
        assert_eq!(state("focus.toggle").get::<bool>(), Some(true));
        assert_eq!(state("typewriter.toggle").get::<bool>(), Some(false));
        assert_eq!(state("theme.toggle").get::<bool>(), Some(true));
        assert_eq!(
            state("focus_scope").get::<String>().as_deref(),
            Some("paragraph")
        );
        assert_eq!(state("theme").get::<String>().as_deref(), Some("auto"));
        assert_eq!(state("face").get::<String>().as_deref(), Some("quattro"));
    }
}
