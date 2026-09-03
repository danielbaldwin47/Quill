//! Templates: named typographic designs for Preview and Export.
//!
//! A Template names faces, sizes, rhythm and light and dark palettes, and is
//! independent of the Editor's own Face and size. [`crate::render`] applies one
//! to a rendered Document; the Editor never reads one.
//!
//! Each built-in is one TOML file under `quill-engine/templates/`, compiled in
//! with `include_str!` and parsed with `serde`
//! ([ADR 0005](https://github.com/danielbaldwin47/Quill/blob/main/docs/adr/0005-native-templates.md)).
//! They are compiled in rather than read out of the data directory that carries
//! the Faces: a Template is not something this version lets a writer replace,
//! so a build that finds no `templates/` directory should still render. The
//! files stay files, in the format a later user-Template effort can read
//! unchanged from XDG config.
//!
//! Every property here is one Pango can style and CSS can express, which is
//! ADR 0005's rule: Preview and PDF read the numbers through Pango, and HTML
//! export generates a stylesheet from the same ones, so the two cannot drift.
//! Sizes are absolute — a Template's base is in points and does not follow the
//! Editor's size ladder — and the Quill-wide toggles (Center Headings, Number
//! Headings, Indent Paragraphs) apply on top of any Template rather than
//! living in one.
//!
//! A Template owns typography only. Page size, margins, the title page, the
//! header and the footer are Export's, styled by whichever Template is
//! current.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::theme::{Colour, Scheme};

/// Every built-in Template, by id, in the order a menu lists them.
pub const IDS: [&str; 5] = [
    "modern",
    "classic",
    "manuscript-mono",
    "manuscript-duo",
    "manuscript-quattro",
];

/// The Template a writer who has never chosen one gets.
pub const DEFAULT: &str = IDS[0];

/// The files behind [`IDS`], in the same order.
///
/// Kept beside the ids rather than in a table of pairs so that the two arrays
/// have to be the same length for the crate to build; that they describe the
/// same Template at the same index is what `every_built_in_names_itself`
/// checks.
const SOURCES: [&str; IDS.len()] = [
    include_str!("../templates/modern.toml"),
    include_str!("../templates/classic.toml"),
    include_str!("../templates/manuscript-mono.toml"),
    include_str!("../templates/manuscript-duo.toml"),
    include_str!("../templates/manuscript-quattro.toml"),
];

/// Why a Template could not be had.
///
/// One case today, because the files are compiled in: a malformed built-in is a
/// build the tests do not let out of the door, and it panics where a writer's
/// own Template would one day carry a note instead.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// No built-in carries this id.
    Unknown(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unknown(id) => {
                write!(f, "no Template named {id}: there is ")?;
                for (index, known) in IDS.iter().enumerate() {
                    let separator = match index {
                        0 => "",
                        _ if index + 1 == IDS.len() => " and ",
                        _ => ", ",
                    };
                    write!(f, "{separator}{known}")?;
                }
                Ok(())
            }
        }
    }
}

/// One typographic design.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct Template {
    /// What the settings file and the Command rows call it.
    ///
    /// The only name a Template carries: what the View menu, the Palette and
    /// the Settings window call it is the Command's own title
    /// (`quill::commands`), so a second one here would be a second place to
    /// change it.
    pub id: String,
    /// How one paragraph is told from the next.
    pub paragraphs: Paragraphs,
    /// The three families a page is set in.
    pub faces: Faces,
    /// The type sizes, from one base.
    pub sizes: Sizes,
    /// The vertical rhythm and the measure.
    pub rhythm: Rhythm,
    /// How a heading is set.
    pub headings: Headings,
    /// The colours on a light ground.
    pub light: Palette,
    /// The colours on a dark ground.
    pub dark: Palette,
}

impl Template {
    /// The palette for `scheme`.
    #[must_use]
    pub const fn palette(&self, scheme: Scheme) -> &Palette {
        match scheme {
            Scheme::Light => &self.light,
            Scheme::Dark => &self.dark,
        }
    }
}

/// The families a page is set in.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct Faces {
    /// Paragraphs, lists and quotations.
    pub body: Face,
    /// Headings.
    pub heading: Face,
    /// Inline code and fenced code.
    pub code: Face,
}

/// One family, and where its italic lives.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct Face {
    /// The family fontconfig is asked for.
    pub family: String,
    /// The family the italic cut is in, when it is a family of its own.
    ///
    /// The Quill Faces are cut that way (ADR 0007): their italic files declare
    /// themselves roman, so asking `Quill Duo` for an italic style would get a
    /// slanted Roman and `Quill Duo Italic` has to be asked for by name.
    /// `None` — Inter, Source Serif 4 — means the family carries its own
    /// italic and a style request reaches it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub italic: Option<String>,
}

/// The type sizes, from one base.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct Sizes {
    /// The body size, in points.
    pub base: f64,
    /// H1 to H6 as multiples of [`Sizes::base`].
    pub headings: [f64; 6],
    /// Code as a multiple of [`Sizes::base`].
    pub code: f64,
}

impl Sizes {
    /// The size of a heading at `level`, in points.
    ///
    /// Levels run 1 to 6 as Markdown writes them; anything outside that is the
    /// body size, which is what an H7 nobody can write would be.
    #[must_use]
    pub fn heading(&self, level: u8) -> f64 {
        let scale = self
            .headings
            .get(usize::from(level).wrapping_sub(1))
            .copied()
            .unwrap_or(1.0);
        self.base * scale
    }
}

/// The vertical rhythm and the measure, all of it in ems of the base size.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct Rhythm {
    /// The line pitch as a multiple of the base size.
    pub line_height: f64,
    /// How wide a line of text is allowed to run.
    pub measure: f64,
    /// The space between two paragraphs. Nothing, when the paragraphs are
    /// indented: the indent is what marks the break.
    pub paragraph_spacing: f64,
    /// The space above a heading.
    pub space_before_heading: f64,
    /// The space below a heading.
    pub space_after_heading: f64,
}

/// How a heading is set.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct Headings {
    /// Where a heading sits in the measure, when Center Headings is off.
    pub alignment: Alignment,
    /// Its weight, as Pango and CSS both number weights.
    pub weight: u16,
}

/// Where a heading sits in the measure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Alignment {
    /// Ranged left with the body.
    Left,
    /// Centred in the measure, which is what the Design oracle's own Modern
    /// does (`ref/ia/mac-native/NOTES.md` § State 16).
    Center,
}

/// How one paragraph is told from the next.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Paragraphs {
    /// By a space between them.
    Spaced,
    /// By a first-line indent, the first paragraph after a heading excepted.
    Indented,
}

/// The colours of one ground.
///
/// Five roles, and no bar colour for a quotation: a quotation is indented and
/// set in body ink.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
pub struct Palette {
    /// The page.
    #[serde(with = "hex")]
    pub paper: Colour,
    /// Body text and headings.
    #[serde(with = "hex")]
    pub ink: Colour,
    /// A rule, and anything set quieter than the body.
    #[serde(with = "hex")]
    pub muted: Colour,
    /// A link's words.
    #[serde(with = "hex")]
    pub link: Colour,
    /// The ground under inline and fenced code.
    #[serde(with = "hex")]
    pub code_ground: Colour,
}

/// The built-in Template `id` names.
///
/// # Errors
///
/// [`Error::Unknown`] when no built-in carries the id, which is what a settings
/// file naming a Template from a newer Quill — or a typo — comes to.
///
/// # Panics
///
/// Panics when a compiled-in file is not the TOML this module parses. Every one
/// of them is read by `every_built_in_names_itself`, so that is a failing test
/// rather than something a writer meets.
pub fn built_in(id: &str) -> Result<Template, Error> {
    let index = IDS
        .iter()
        .position(|known| *known == id)
        .ok_or_else(|| Error::Unknown(id.to_owned()))?;
    Ok(toml::from_str(SOURCES[index])
        .unwrap_or_else(|err| panic!("the compiled-in Template {id} parses: {err}")))
}

/// A colour as a Template file writes it: `#rrggbb`.
///
/// [`crate::theme::Colour`] is not `serde`'s to derive — it is four floats, and
/// a file names a hex string — so the pair of functions `#[serde(with = ...)]`
/// wants lives here.
mod hex {
    use serde::de::Error as _;
    use serde::{Deserialize, Deserializer, Serializer};

    use crate::theme::Colour;

    pub fn serialize<S: Serializer>(colour: &Colour, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&colour.to_hex())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Colour, D::Error> {
        let text = String::deserialize(deserializer)?;
        Colour::parse(&text)
            .ok_or_else(|| D::Error::custom(format!("{text} is not a colour: write it `#rrggbb`")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_built_in_names_itself() {
        for id in IDS {
            let template = built_in(id).expect("a built-in id");
            assert_eq!(template.id, id, "{id}'s file names another Template");
        }
    }

    #[test]
    fn every_property_survives_a_write_and_a_reparse() {
        for id in IDS {
            let template = built_in(id).expect("a built-in id");
            let written = toml::to_string(&template).expect("a Template is TOML");
            let reparsed: Template = toml::from_str(&written).unwrap_or_else(|err| {
                panic!("{id} reparses from what it writes: {err}\n{written}")
            });
            assert_eq!(reparsed, template, "{id} lost a property on the way out");
        }
    }

    #[test]
    fn modern_is_inter_on_the_measured_papers() {
        let modern = built_in("modern").expect("a built-in id");
        assert_eq!(modern.faces.body.family, "Inter");
        assert_eq!(modern.faces.heading.family, "Inter");
        // Inter carries its own italic, so nothing names one for it.
        assert_eq!(modern.faces.body.italic, None);
        // `ref/ia/mac-native/NOTES.md` § State 16.
        assert_eq!(modern.dark.paper, Colour::from_hex("#101010"));
        assert_eq!(modern.light.paper, Colour::from_hex("#ffffff"));
        assert_eq!(modern.headings.alignment, Alignment::Center);
        assert_eq!(DEFAULT, "modern");
    }

    #[test]
    fn classic_is_source_serif_4() {
        let classic = built_in("classic").expect("a built-in id");
        assert_eq!(classic.faces.body.family, "Source Serif 4");
        assert_eq!(classic.paragraphs, Paragraphs::Indented);
        assert_eq!(
            classic.rhythm.paragraph_spacing, 0.0,
            "indented paragraphs are not spaced as well"
        );
    }

    #[test]
    fn the_manuscripts_are_the_quill_faces_at_the_editors_size() {
        for (id, family) in [
            ("manuscript-mono", "Quill Mono"),
            ("manuscript-duo", "Quill Duo"),
            ("manuscript-quattro", "Quill Quattro"),
        ] {
            let manuscript = built_in(id).expect("a built-in id");
            let italic = format!("{family} Italic");
            for face in [
                &manuscript.faces.body,
                &manuscript.faces.heading,
                &manuscript.faces.code,
            ] {
                assert_eq!(face.family, family, "{id} is one face throughout");
                // Each Italic is a family of its own (ADR 0007).
                assert_eq!(
                    face.italic.as_deref(),
                    Some(italic.as_str()),
                    "{id}'s italic"
                );
            }
            // 16 pt is the 21.33 px em of step 5 at 96 dpi: the Editor's own
            // default size.
            assert_eq!(manuscript.sizes.base, 16.0);
            // Headings bold at body size.
            assert_eq!(manuscript.sizes.headings, [1.0; 6]);
            assert_eq!(manuscript.headings.weight, 700);
        }
    }

    #[test]
    fn a_heading_scale_is_read_by_level() {
        let sizes = built_in("modern").expect("a built-in id").sizes;
        assert_eq!(sizes.heading(1), sizes.base * sizes.headings[0]);
        assert_eq!(sizes.heading(6), sizes.base * sizes.headings[5]);
        // A level Markdown cannot write is the body size rather than a panic.
        assert_eq!(sizes.heading(0), sizes.base);
        assert_eq!(sizes.heading(7), sizes.base);
    }

    #[test]
    fn an_unknown_id_is_an_error_naming_it() {
        let err = built_in("nightfall").expect_err("there is no Template called nightfall");
        assert_eq!(err, Error::Unknown("nightfall".to_owned()));
        let said = err.to_string();
        assert!(said.contains("nightfall"), "{said}");
        assert!(said.contains("manuscript-quattro"), "{said}");
    }

    #[test]
    fn a_colour_that_is_not_written_in_hex_is_refused() {
        let text = SOURCES[0].replace("#101010", "midnight");
        let err = toml::from_str::<Template>(&text).expect_err("`midnight` is not a colour");
        assert!(err.to_string().contains("midnight"), "{err}");
    }
}
