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

/// The two bundled OFL families, as (family name, style, file name).
///
/// Inter 4.1 (`extras/ttf/` of the upstream release, github.com/rsms/inter) and
/// Source Serif 4.005 (`TTF/` of the Desktop release,
/// github.com/adobe-fonts/source-serif), unmodified: the four static cuts a
/// Template asks for, Regular, Italic, Bold and Bold Italic. Modern is set in
/// the first and Classic in the second ([`crate::template`]), so they load
/// privately at startup the way the Faces do and a writer installs nothing.
///
/// Unlike the Faces, one family covers all four cuts. The family name repeats
/// down the table and the style is what tells the files apart: a Template names
/// the family alone and asks Pango for the weight and the slope. Neither is a
/// Modified Version, so both keep their upstream names and file names, and
/// their licences ship beside the Faces' own as [`LICENCES`] lists.
pub const FAMILIES: [(&str, &str, &str); 8] = [
    ("Inter", "Regular", "Inter-Regular.ttf"),
    ("Inter", "Italic", "Inter-Italic.ttf"),
    ("Inter", "Bold", "Inter-Bold.ttf"),
    ("Inter", "Bold Italic", "Inter-BoldItalic.ttf"),
    ("Source Serif 4", "Regular", "SourceSerif4-Regular.ttf"),
    ("Source Serif 4", "Italic", "SourceSerif4-It.ttf"),
    ("Source Serif 4", "Bold", "SourceSerif4-Bold.ttf"),
    ("Source Serif 4", "Bold Italic", "SourceSerif4-BoldIt.ttf"),
];

/// The licences that ship in the fonts directory, one per set of files there.
///
/// `OFL.txt` is the Faces' (ADR 0007), and the other two are Inter's and Source
/// Serif's own, copied out of their releases unchanged. The package installs
/// all three beside the fonts and under `/usr/share/licenses/quill/`.
pub const LICENCES: [&str; 3] = ["OFL.txt", "OFL-Inter.txt", "OFL-SourceSerif4.txt"];

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

/// The directory holding the six Faces, the two bundled families and their
/// licences.
#[must_use]
pub fn fonts() -> PathBuf {
    dir().join("fonts")
}

/// The directory holding the three Style check lists, their `SOURCES.md` and
/// the licence texts those sources require.
///
/// A second name under the data directory rather than a second directory beside
/// it: the lists are data files like the Faces, so an installed Quill and a
/// `cargo run` find them by one path. Which three files sit there is the style
/// module's to say — this module only says where.
#[must_use]
pub fn style() -> PathBuf {
    dir().join("data").join("style")
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
    fn the_style_directory_is_the_data_directory_plus_two_names() {
        assert_eq!(style(), dir().join("data").join("style"));
    }

    #[test]
    fn the_checkout_holds_the_three_lists_and_what_licenses_them() {
        // Against `checkout()` for the reason the Faces are, and spelled out
        // here rather than read from a table: three file names one test can
        // hold is the whole of what ships, and a list renamed without its
        // `SOURCES.md` row is what this catches.
        for file in [
            "fillers.txt",
            "redundancies.txt",
            "cliches.txt",
            "SOURCES.md",
            "LICENSE-MIT-wooorm.txt",
            "LICENSE-MIT-duereg.txt",
            "LICENSE-BSD-3-Clause-proselint.txt",
            "LICENSE-CC0-plainlanguage.txt",
        ] {
            let path = checkout().join("data").join("style").join(file);
            assert!(
                path.is_file(),
                "no {} in the data directory: the lists are committed, not built",
                path.display()
            );
        }
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

    #[test]
    fn the_checkout_holds_the_bundled_families_and_every_licence() {
        // Against `checkout()` for the same reason the Faces are.
        for (family, style, file) in FAMILIES {
            let font = checkout().join("fonts").join(file);
            assert!(
                font.is_file(),
                "no {family} {style} at {}: it is committed, not built",
                font.display()
            );
        }
        for licence in LICENCES {
            let text = checkout().join("fonts").join(licence);
            assert!(
                text.is_file(),
                "no {} beside the fonts it licenses",
                text.display()
            );
        }
    }
}
