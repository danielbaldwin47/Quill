//! The data directory: the one place the files Quill ships are found.
//!
//! The Faces, the Templates, the Style check lists, the Selection Mark's
//! glyphs, the Settings window's icons and `OFL.txt` all resolve
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
/// all three beside the fonts and under `/usr/share/licenses/quill-writer/`.
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

/// The directory holding the Selection Mark's glyph files, beside the Style
/// check lists and for the same reason: which four files sit there is the mark
/// module's to say.
#[must_use]
pub fn marks() -> PathBuf {
    dir().join("data").join("marks")
}

/// The icons the Settings window draws that GTK would otherwise take from
/// the icon theme: the dropdown's chevron, the spin button's minus and plus,
/// and the search field's clear.
///
/// Files rather than a compiled resource, for the reason the Selection Mark's
/// glyphs are: one data directory, so an installed Quill and a `cargo run`
/// load the same file, and no build script. Each is a filled symbolic SVG, its
/// name ending `-symbolic.svg` so that GTK recolours it to the widget's CSS
/// `color` — through `-gtk-recolor(url(…))` in a stylesheet, or a
/// `gio::FileIcon` in an image — and loads it at the output's scale.
pub const ICONS: [&str; 4] = [
    "chevron-symbolic.svg",
    "minus-symbolic.svg",
    "plus-symbolic.svg",
    "clear-symbolic.svg",
];

/// The directory holding [`ICONS`], beside the Selection Mark's glyphs.
#[must_use]
pub fn icons() -> PathBuf {
    dir().join("data").join("icons")
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
    fn the_marks_directory_is_the_data_directory_plus_two_names() {
        assert_eq!(marks(), dir().join("data").join("marks"));
    }

    #[test]
    fn the_checkout_holds_the_four_mark_glyphs() {
        // Against `checkout()` for the reason the lists are.
        for file in [
            "feather.svg",
            "feather-short.svg",
            "pen.svg",
            "pen-short.svg",
        ] {
            let path = checkout().join("data").join("marks").join(file);
            assert!(
                path.is_file(),
                "no {} in the data directory: the glyphs are committed, not built",
                path.display()
            );
        }
    }

    #[test]
    fn the_icons_directory_is_the_data_directory_plus_two_names() {
        assert_eq!(icons(), dir().join("data").join("icons"));
    }

    #[test]
    fn the_checkout_holds_every_icon_as_a_filled_symbolic_svg() {
        // Against `checkout()` for the reason the marks are.
        for file in ICONS {
            let path = checkout().join("data").join("icons").join(file);
            let svg = std::fs::read_to_string(&path).unwrap_or_else(|_| {
                panic!(
                    "no {} in the data directory: the icons are committed, not built",
                    path.display()
                )
            });
            assert!(file.ends_with("-symbolic.svg"), "{file} is not recoloured");
            assert!(svg.contains("<path d="), "{file} draws no path");
            // `-gtk-recolor` fills every path, so a stroke comes out a wedge.
            assert!(!svg.contains("stroke"), "{file} is stroked, not filled");
        }
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

    /// The strikeout rule `tools/fontbuild.py` pins, read back out of the
    /// files that ship.
    ///
    /// Pango draws a strikethrough from the face's own `OS/2` fields and takes
    /// neither a thickness nor a position from a text tag, so these two numbers
    /// are the whole of the Style check rule's geometry, and Markdown's `~~`
    /// moves with them (#354; `docs/design.md` § Rows, Style check mark). A
    /// rebuild that dropped `pin_strikeout` would pass every other test in the
    /// tree and be caught only by a judged round; this is what stands in for
    /// that round.
    #[test]
    fn every_face_carries_the_pinned_strikeout_rule() {
        for (family, file) in FACES {
            let path = checkout().join("fonts").join(file);
            let font = std::fs::read(&path).expect("a Face to read");
            let os2 = table(&font, b"OS/2");
            assert_eq!(
                (
                    be16(&font, table(&font, b"head") + 18),
                    be16(&font, os2 + 26),
                    be16(&font, os2 + 28)
                ),
                (1000, 30, 262),
                "{family} at {}: units per em, `yStrikeoutSize` and \
                 `yStrikeoutPosition` — `python3 tools/fontbuild.py` writes them",
                path.display()
            );
        }
    }

    /// Where one table of a TrueType file starts, by its four-byte tag.
    ///
    /// Twelve bytes of header, then a directory of sixteen-byte records — tag,
    /// checksum, offset, length. Enough of the format to read three numbers,
    /// and less than a font crate would cost the engine for one test.
    fn table(font: &[u8], tag: &[u8; 4]) -> usize {
        let tables = usize::from(be16(font, 4));
        (0..tables)
            .map(|at| 12 + at * 16)
            .find(|record| &font[*record..*record + 4] == tag)
            .map(|record| {
                usize::try_from(u32::from_be_bytes(
                    font[record + 8..record + 12]
                        .try_into()
                        .expect("four bytes of offset"),
                ))
                .expect("an offset inside the file")
            })
            .unwrap_or_else(|| panic!("no {} table", String::from_utf8_lossy(tag)))
    }

    /// The two pinned values and the units per em are all positive, so an
    /// unsigned read is enough: a dropped pin reads iA's own 60 and 309.
    fn be16(bytes: &[u8], at: usize) -> u16 {
        u16::from_be_bytes(bytes[at..at + 2].try_into().expect("two bytes"))
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
