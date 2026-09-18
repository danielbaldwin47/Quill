//! The Selection Mark's glyphs: the feather and the fountain pen, read from the
//! SVG files the data directory ships and filled on a cairo context.
//!
//! `[library] mark` names one of three marks, and two of them are drawings
//! (#441 § The selected row and the Selection Mark); the third, the bar, is the
//! pane's stylesheet and has no file. Each drawing ships twice — tall, for the
//! row that carries an excerpt, and short, for the row that does not — because
//! at the short row's height a feather's rachis and the gaps beside it are
//! under a pixel; the short file is the same glyph with fewer, wider features.
//!
//! The files are SVG so that any editor opens and redraws them, but only the
//! subset they are written in is read: a `viewBox`, and `d` attributes of
//! absolute `M`, `L`, `C` and `Z` with every number spaced apart, all filled
//! even-odd in whatever source the caller set. That is a page of code where an
//! SVG renderer would be a dependency, and a file redrawn outside the subset is
//! refused by name rather than drawn wrong.

use std::path::{Path, PathBuf};

use crate::data;
use crate::settings::Mark;

/// One glyph: its box, and the shapes that fill it.
#[derive(Clone, Debug, PartialEq)]
pub struct Glyph {
    /// The `viewBox`'s width and height, which every segment is measured in.
    size: (f64, f64),
    /// Every path of the file, one after the other.
    segments: Vec<Segment>,
}

/// One step of a path, in the `viewBox`'s units.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Segment {
    Move(f64, f64),
    Line(f64, f64),
    Curve([f64; 6]),
    Close,
}

/// The name of the file a mark is drawn from, tall or short, or `None` for the
/// bar.
#[must_use]
pub fn file_name(mark: Mark, short: bool) -> Option<&'static str> {
    match (mark, short) {
        (Mark::Bar, _) => None,
        (Mark::Feather, false) => Some("feather.svg"),
        (Mark::Feather, true) => Some("feather-short.svg"),
        (Mark::Pen, false) => Some("pen.svg"),
        (Mark::Pen, true) => Some("pen-short.svg"),
    }
}

/// Where the file a mark is drawn from sits, or `None` for the bar.
#[must_use]
pub fn file(mark: Mark, short: bool) -> Option<PathBuf> {
    file_name(mark, short).map(|name| data::marks().join(name))
}

impl Glyph {
    /// Reads one glyph file. The error names the file and what in it is
    /// outside the subset.
    ///
    /// # Errors
    ///
    /// When the file cannot be read or is not the subset this module reads.
    pub fn read(path: &Path) -> Result<Self, String> {
        std::fs::read_to_string(path)
            .map_err(|error| error.to_string())
            .and_then(|svg| Self::parse(&svg))
            .map_err(|error| format!("{}: {error}", path.display()))
    }

    /// Parses the text of one glyph file.
    ///
    /// # Errors
    ///
    /// When there is no four-number `viewBox`, no path, or a path command or
    /// number outside the subset.
    pub fn parse(svg: &str) -> Result<Self, String> {
        let view = attributes(svg, "viewBox")
            .into_iter()
            .next()
            .ok_or("no viewBox")?;
        let size = match numbers(view)?[..] {
            [_, _, width, height] if width > 0.0 && height > 0.0 => (width, height),
            _ => return Err(format!("viewBox \"{view}\" is not four numbers of a box")),
        };
        let mut segments = Vec::new();
        for d in attributes(svg, "d") {
            path(d, &mut segments)?;
        }
        if segments.is_empty() {
            return Err("no path".to_owned());
        }
        Ok(Self { size, segments })
    }

    /// How wide the glyph is drawn when it is drawn `height` tall.
    #[must_use]
    pub fn width_at(&self, height: f64) -> f64 {
        height * self.size.0 / self.size.1
    }

    /// Fills the glyph on `cr` in the source already set, `height` tall, with
    /// its box's top left corner at the origin.
    pub fn draw(&self, cr: &cairo::Context, height: f64) {
        let scale = height / self.size.1;
        cr.new_path();
        for segment in &self.segments {
            match *segment {
                Segment::Move(x, y) => cr.move_to(x * scale, y * scale),
                Segment::Line(x, y) => cr.line_to(x * scale, y * scale),
                Segment::Curve([x1, y1, x2, y2, x, y]) => cr.curve_to(
                    x1 * scale,
                    y1 * scale,
                    x2 * scale,
                    y2 * scale,
                    x * scale,
                    y * scale,
                ),
                Segment::Close => cr.close_path(),
            }
        }
        cr.set_fill_rule(cairo::FillRule::EvenOdd);
        // A failed fill is a context already in error, which the widget that
        // handed it over reports; there is nothing for a glyph to add.
        let _ = cr.fill();
    }
}

/// Every value of the attribute `name` in `svg`, in order.
///
/// An attribute is `name="` after whitespace, so `d` is not found inside
/// `id="…"`.
fn attributes<'a>(svg: &'a str, name: &str) -> Vec<&'a str> {
    let key = format!("{name}=\"");
    let mut found = Vec::new();
    let mut from = 0;
    while let Some(at) = svg[from..].find(&key) {
        let start = from + at;
        let value = start + key.len();
        let Some(length) = svg[value..].find('"') else {
            break;
        };
        if svg[..start].ends_with(char::is_whitespace) {
            found.push(&svg[value..value + length]);
        }
        from = value + length + 1;
    }
    found
}

/// The numbers of an attribute, spaced or comma-separated.
fn numbers(text: &str) -> Result<Vec<f64>, String> {
    text.split(|c: char| c.is_whitespace() || c == ',')
        .filter(|word| !word.is_empty())
        .map(|word| {
            word.parse()
                .map_err(|_| format!("\"{word}\" is not a number"))
        })
        .collect()
}

/// Appends the segments of one `d` attribute.
fn path(d: &str, segments: &mut Vec<Segment>) -> Result<(), String> {
    // A command letter is a word of its own, and a comma is a space.
    let mut spaced = String::with_capacity(d.len());
    for c in d.chars() {
        match c {
            ',' => spaced.push(' '),
            c if c.is_ascii_alphabetic() => {
                spaced.push(' ');
                spaced.push(c);
                spaced.push(' ');
            }
            c => spaced.push(c),
        }
    }
    let words: Vec<&str> = spaced.split_whitespace().collect();
    let mut command = None;
    let mut at = 0;
    while at < words.len() {
        let word = words[at];
        if let Some(letter) = word.chars().next().filter(char::is_ascii_alphabetic) {
            match letter {
                'M' | 'L' | 'C' => command = Some(letter),
                'Z' => {
                    segments.push(Segment::Close);
                    command = None;
                }
                _ => {
                    return Err(format!(
                        "path command {letter} is outside the subset: absolute M, L, C and Z"
                    ));
                }
            }
            at += 1;
            continue;
        }
        let (letter, arity) = match command {
            Some(letter @ ('M' | 'L')) => (letter, 2),
            Some(letter) => (letter, 6),
            None => return Err(format!("\"{word}\" has no path command before it")),
        };
        let values = words
            .get(at..at + arity)
            .ok_or_else(|| format!("{letter} wants {arity} numbers"))?
            .iter()
            .map(|word| {
                word.parse::<f64>()
                    .map_err(|_| format!("\"{word}\" is not a number"))
            })
            .collect::<Result<Vec<f64>, String>>()?;
        segments.push(match letter {
            'M' => {
                // Numbers after a move's first pair are lines, as SVG reads them.
                command = Some('L');
                Segment::Move(values[0], values[1])
            }
            'L' => Segment::Line(values[0], values[1]),
            _ => Segment::Curve([
                values[0], values[1], values[2], values[3], values[4], values[5],
            ]),
        });
        at += arity;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The heights the pane draws a mark at, in device pixels at scale 1 and
    /// 2: the 68 and 32 point rows less their separator and the bar's two
    /// 6 point insets.
    const HEIGHTS: [(bool, f64); 4] = [(false, 55.0), (false, 110.0), (true, 19.0), (true, 38.0)];

    /// Against the checkout rather than `data::marks()`, for the reason the
    /// data module's own tests are: an exported `$QUILL_DATA_DIR` must not turn
    /// this into a test of somebody else's files.
    fn checkout_file(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("the engine sits inside its workspace")
            .join("data")
            .join("marks")
            .join(name)
    }

    /// Each glyph loads and fills its box: ink reaches the top and the bottom
    /// row, nothing lands right of the width it is drawn at, and it is a
    /// drawing rather than a filled rectangle or a speck.
    #[test]
    fn every_glyph_loads_and_rasterises_at_the_tall_and_the_short_height() {
        for mark in [Mark::Feather, Mark::Pen] {
            for (short, height) in HEIGHTS {
                let name = file_name(mark, short).expect("a drawing");
                let glyph = Glyph::read(&checkout_file(name)).expect("a glyph in the subset");
                let aspect = glyph.width_at(1.0);
                assert!(
                    (0.12..=0.25).contains(&aspect),
                    "{name}: about a fifth as wide as it is tall, not {aspect}"
                );
                let width = glyph.width_at(height).ceil();
                // Two pixels of air right of the box, to see ink that spills.
                let (w, h) = (width as i32 + 2, height as i32);
                let mut surface =
                    cairo::ImageSurface::create(cairo::Format::ARgb32, w, h).expect("a surface");
                {
                    let cr = cairo::Context::new(&surface).expect("a context");
                    cr.set_source_rgba(0.0, 0.0, 0.0, 1.0);
                    glyph.draw(&cr, height);
                }
                let stride = usize::try_from(surface.stride()).expect("a stride");
                let data = surface.data().expect("the pixels");
                let alpha = |x: i32, y: i32| {
                    let at = usize::try_from(y).unwrap() * stride + usize::try_from(x).unwrap() * 4;
                    u32::from_ne_bytes(data[at..at + 4].try_into().unwrap()) >> 24
                };
                let row_ink = |y: i32| (0..w).map(|x| alpha(x, y)).max().unwrap_or(0);
                assert!(row_ink(0) > 0, "{name} at {height}: ink on the top row");
                assert!(
                    row_ink(h - 1) > 0,
                    "{name} at {height}: ink on the bottom row"
                );
                let spill = (0..h).map(|y| alpha(w - 1, y)).max().unwrap_or(0);
                assert_eq!(spill, 0, "{name} at {height}: ink right of its width");
                let inked = (0..h)
                    .flat_map(|y| (0..w).map(move |x| (x, y)))
                    .filter(|&(x, y)| alpha(x, y) > 127)
                    .count();
                let share = inked as f64 / (width * height);
                assert!(
                    (0.2..0.8).contains(&share),
                    "{name} at {height}: {share} of its box inked"
                );
            }
        }
    }

    #[test]
    fn the_bar_is_the_stylesheets_and_has_no_file() {
        assert_eq!(file(Mark::Bar, false), None);
        assert_eq!(file(Mark::Bar, true), None);
        assert_eq!(
            file(Mark::Pen, true),
            Some(data::marks().join("pen-short.svg"))
        );
    }

    #[test]
    fn a_file_outside_the_subset_is_refused_by_what_is_outside_it() {
        let box_ = r#"<svg viewBox="0 0 10 50">"#;
        assert_eq!(
            Glyph::parse(&format!(r#"{box_}<path d="m 1 1 L 2 2 Z"/></svg>"#)),
            Err("path command m is outside the subset: absolute M, L, C and Z".to_owned())
        );
        assert_eq!(
            Glyph::parse(r#"<svg><path d="M 1 1 Z"/></svg>"#),
            Err("no viewBox".to_owned())
        );
        assert_eq!(
            Glyph::parse(&format!(r#"{box_}<path d="M 1 1 C 2 2 Z"/></svg>"#)),
            Err("C wants 6 numbers".to_owned())
        );
        assert_eq!(
            Glyph::parse(&format!("{box_}</svg>")),
            Err("no path".to_owned())
        );
        let parsed = Glyph::parse(&format!(
            r#"{box_}<path id="x" d="M1,1 2 2 C 1 1 2 2 3 3 Z"/></svg>"#
        ))
        .expect("the subset");
        assert_eq!(
            parsed.segments,
            [
                Segment::Move(1.0, 1.0),
                Segment::Line(2.0, 2.0),
                Segment::Curve([1.0, 1.0, 2.0, 2.0, 3.0, 3.0]),
                Segment::Close,
            ]
        );
    }
}
