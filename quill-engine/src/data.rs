//! The data directory: the one place the files Quill ships are found.
//!
//! The Faces, the Templates, the Style check lists and `OFL.txt` all resolve
//! through it, so an installed package and a development build differ in one
//! path rather than in every lookup (`docs/architecture.md`, "Fonts and data
//! files"). Nothing is downloaded and nothing is searched for: there is one
//! directory, and three ways of learning where it is.
//!
//! `$QUILL_DATA_DIR` wins, so the Gate harness and anyone bisecting can point a
//! build at another tree without reinstalling. Failing that, the path the
//! package build compiled in (`/usr/share/quill`). Failing that — which is what
//! a plain `cargo build` gets — the checkout this binary was built from, so
//! `cargo run` renders in the Faces sitting in `fonts/`.
//!
//! One name does both jobs, because the package build sets the variable and the
//! compiled-in path is what it leaves behind. The corner that costs: a
//! developer with `$QUILL_DATA_DIR` exported bakes it into a plain
//! `cargo build`, so a binary built that way keeps looking there once the
//! variable is gone from the shell. `cargo` rebuilds when the variable changes,
//! so nothing goes stale — but unset it before building a binary meant to stand
//! on its own.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// The six Faces the data directory holds, as (family name, file name).
///
/// One table, because three things have to agree: `tools/fontbuild.py` writes
/// these files with these family names inside them, startup asks fontconfig for
/// these families, and the tests check that the two still describe the same six
/// Faces. Each Italic is a family of its own (ADR 0007).
pub const FACES: [(&str, &str); 6] = [
    ("Quill Duo", "QuillDuo.ttf"),
    ("Quill Duo Italic", "QuillDuoItalic.ttf"),
    ("Quill Quattro", "QuillQuattro.ttf"),
    ("Quill Quattro Italic", "QuillQuattroItalic.ttf"),
    ("Quill Mono", "QuillMono.ttf"),
    ("Quill Mono Italic", "QuillMonoItalic.ttf"),
];

/// The variable that names the data directory, read at run time here and set by
/// the package build so that [`COMPILED_IN`] has something to hold.
const VARIABLE: &str = "QUILL_DATA_DIR";

/// Where the package put its data, written into the binary by the build that
/// made it. `None` for a plain `cargo build`.
const COMPILED_IN: Option<&str> = option_env!("QUILL_DATA_DIR");

/// The data directory this binary reads from.
#[must_use]
pub fn dir() -> PathBuf {
    resolve(std::env::var_os(VARIABLE), COMPILED_IN, checkout())
}

/// The directory holding the six Faces.
#[must_use]
pub fn fonts() -> PathBuf {
    dir().join("fonts")
}

/// The precedence itself, with its three inputs handed in.
///
/// Separated from [`dir`] because the environment, the compiled-in path and the
/// checkout are each fixed for a given process, and the rule between them is
/// the part worth testing.
fn resolve(environment: Option<OsString>, compiled_in: Option<&str>, checkout: &Path) -> PathBuf {
    // An empty variable is a variable nobody meant to set: `QUILL_DATA_DIR=`
    // in a wrapper script would otherwise resolve every data file to the
    // working directory.
    environment
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| compiled_in.map(PathBuf::from))
        .unwrap_or_else(|| checkout.to_path_buf())
}

/// The workspace root of the checkout this crate was built from.
fn checkout() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("quill-engine sits inside the workspace it is built from")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_environment_wins() {
        let dir = resolve(
            Some(OsString::from("/tmp/quill-data")),
            Some("/usr/share/quill"),
            Path::new("/home/writer/quill"),
        );
        assert_eq!(dir, Path::new("/tmp/quill-data"));
    }

    #[test]
    fn the_compiled_in_path_is_honoured_when_the_build_wrote_one() {
        let dir = resolve(
            None,
            Some("/usr/share/quill"),
            Path::new("/home/writer/quill"),
        );
        assert_eq!(dir, Path::new("/usr/share/quill"));
    }

    #[test]
    fn a_plain_build_resolves_to_the_checkout() {
        let dir = resolve(None, None, Path::new("/home/writer/quill"));
        assert_eq!(dir, Path::new("/home/writer/quill"));
    }

    #[test]
    fn an_empty_variable_is_no_variable() {
        let dir = resolve(Some(OsString::new()), None, Path::new("/home/writer/quill"));
        assert_eq!(dir, Path::new("/home/writer/quill"));
    }

    #[test]
    fn the_fonts_directory_is_the_data_directory_plus_one_name() {
        assert_eq!(fonts(), dir().join("fonts"));
    }

    #[test]
    fn the_checkout_holds_the_faces_a_plain_build_will_look_for() {
        // Named against `checkout()` rather than `fonts()`, so that setting
        // `$QUILL_DATA_DIR` in a shell cannot turn this into a passing test
        // about somebody else's directory.
        for (family, file) in FACES {
            let face = checkout().join("fonts").join(file);
            assert!(
                face.is_file(),
                "no {family} at {}: `python3 tools/fontbuild.py` writes the Faces",
                face.display()
            );
        }
    }
}
