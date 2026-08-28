//! Where Quill's two files live.
//!
//! Config is what the writer chose and state is what the app observed
//! ([ADR 0010](../../../docs/adr/0010-settings-in-toml-under-xdg.md)), so they
//! go to the two directories the XDG base directory specification keeps apart:
//! `$XDG_CONFIG_HOME/quill/` for the first, `$XDG_STATE_HOME/quill/` for the
//! second. A writer backing up their preferences copies one directory; state
//! churning on every quit never touches it.
//!
//! The specification's own rules, kept here rather than assumed: an unset
//! variable falls back to `~/.config` and `~/.local/state`, and a variable that
//! is empty or holds a relative path is treated as unset, because a relative
//! base directory would put a writer's settings wherever Quill happened to be
//! launched from.

use std::ffi::OsString;
use std::path::PathBuf;

/// The directory Quill's own files sit in, under either base directory.
const QUILL: &str = "quill";

/// `$XDG_CONFIG_HOME/quill/`: what the writer chose.
#[must_use]
pub fn config_dir() -> PathBuf {
    base("XDG_CONFIG_HOME", ".config").join(QUILL)
}

/// `$XDG_STATE_HOME/quill/`: what the app observed.
#[must_use]
pub fn state_dir() -> PathBuf {
    base("XDG_STATE_HOME", ".local/state").join(QUILL)
}

/// The base directory `variable` names, or `$HOME/<fallback>`.
fn base(variable: &str, fallback: &str) -> PathBuf {
    resolve(std::env::var_os(variable), std::env::home_dir(), fallback)
}

/// The rule itself, with the environment handed in.
fn resolve(variable: Option<OsString>, home: Option<PathBuf>, fallback: &str) -> PathBuf {
    variable
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| home.map(|home| home.join(fallback)))
        // No home directory at all: a process that has none has nowhere better
        // to keep a file than beside itself.
        .unwrap_or_else(|| PathBuf::from(fallback))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_variable_wins_when_it_names_an_absolute_directory() {
        let base = resolve(
            Some(OsString::from("/run/user/1000/config")),
            Some(PathBuf::from("/home/writer")),
            ".config",
        );
        assert_eq!(base, PathBuf::from("/run/user/1000/config"));
    }

    #[test]
    fn an_unset_variable_falls_back_to_the_home_directory() {
        let base = resolve(None, Some(PathBuf::from("/home/writer")), ".config");
        assert_eq!(base, PathBuf::from("/home/writer/.config"));
    }

    #[test]
    fn an_empty_variable_is_no_variable() {
        let base = resolve(
            Some(OsString::new()),
            Some(PathBuf::from("/home/writer")),
            ".local/state",
        );
        assert_eq!(base, PathBuf::from("/home/writer/.local/state"));
    }

    #[test]
    fn a_relative_variable_is_no_variable() {
        // The specification says to ignore it, and it is worth ignoring: a
        // writer's settings must not depend on the directory Quill was
        // launched from.
        let base = resolve(
            Some(OsString::from("config")),
            Some(PathBuf::from("/home/writer")),
            ".config",
        );
        assert_eq!(base, PathBuf::from("/home/writer/.config"));
    }

    #[test]
    fn with_no_home_the_base_is_the_working_directory() {
        let base = resolve(None, None, ".config");
        assert_eq!(base, PathBuf::from(".config"));
    }

    #[test]
    fn config_and_state_are_two_directories_named_for_quill() {
        assert_ne!(config_dir(), state_dir());
        assert_eq!(config_dir().file_name(), Some(QUILL.as_ref()));
        assert_eq!(state_dir().file_name(), Some(QUILL.as_ref()));
    }
}
