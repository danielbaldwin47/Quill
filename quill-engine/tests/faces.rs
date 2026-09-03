//! The fonts as files: what `tools/fontbuild.py` must have written into the six
//! Faces, and what the two bundled families must still say about themselves.
//!
//! Four things nobody can see without opening a font editor, and one of them
//! is a licence condition.
//!
//! OFL 1.1 section 3 forbids a Modified Version from carrying the Reserved Font
//! Name, and Quill's Faces are Modified Versions (ADR 0007). "iA Writer" lives
//! in the `name` table, in two encodings, so the check is over the whole file
//! in both. The Quill names have to be there in its place, on every record a
//! matcher reads and on every named instance.
//!
//! And Quattro's Italic word space has to be its Roman's. iA's is a third
//! wider, which would set an italic word wider than the same word upright — the
//! bug `tools/fontgrid.py` was written for in the web app.
//!
//! And Inter and Source Serif 4 are the opposite case: unmodified upstream
//! releases, so what has to hold is that each file still names the family and
//! the style [`quill_engine::data::FAMILIES`] promises a Template and
//! fontconfig will find it under. A file swapped for another cut is a
//! Template's bold silently coming out roman.
//!
//! The files are read straight off disk rather than through a font crate: what
//! is being checked is bytes, and the engine has no business depending on a
//! parser to say so.

use std::fs;
use std::path::PathBuf;

use quill_engine::data::{self, FACES, FAMILIES};

/// The Reserved Font Name no Modified Version may carry.
const RESERVED: &str = "iA Writer";

/// The four named instances every Face carries, in `fvar` order.
const STYLES: [&str; 4] = ["Regular", "Text", "Semibold", "Bold"];

/// The word space, glyph 1 in every one of these files, as ADR 0007 records.
const SPACE_GLYPH: usize = 1;

/// What that space measures, in font units, once Quattro Italic is patched.
/// Asserted as well as compared, so that a Face whose glyphs were reordered
/// fails here rather than quietly comparing some other pair of glyphs.
const SPACE_ADVANCE: u16 = 450;

#[test]
fn no_face_carries_the_reserved_font_name() {
    for (family, file) in FACES {
        let bytes = read(file);
        let latin1: Vec<u8> = RESERVED.bytes().collect();
        let utf16: Vec<u8> = RESERVED.encode_utf16().flat_map(u16::to_be_bytes).collect();
        for encoding in [latin1, utf16] {
            assert!(
                !bytes.windows(encoding.len()).any(|w| w == encoding),
                "{file} still carries {RESERVED:?}, which OFL 1.1 section 3 forbids {family} \
                 from using"
            );
        }
    }
}

#[test]
fn every_face_names_itself_by_its_quill_name() {
    for (family, file) in FACES {
        let font = read(file);
        let names = names(&font);
        let postscript_family = family.replace(' ', "");
        let named = |id: u16| -> Vec<&String> {
            names
                .iter()
                .filter(|(name_id, _)| *name_id == id)
                .map(|(_, string)| string)
                .collect()
        };
        let expect = |id: u16, want: &str| {
            let found = named(id);
            assert!(!found.is_empty(), "{file} has no `name` record {id}");
            for string in found {
                assert_eq!(string, want, "{file}'s `name` record {id}");
            }
        };
        expect(1, family);
        expect(2, "Regular");
        expect(4, family);
        expect(6, &format!("{postscript_family}-Regular"));

        // The named instances, which is what a weight request resolves through.
        // Followed from `fvar` rather than looked for in the name table, so
        // that an instance still pointing at an iA name is a failure and not a
        // string that happens to be present.
        let instances = instances(&font);
        assert_eq!(instances.len(), STYLES.len(), "{file}'s `fvar` instances");
        for (style, (subfamily, postscript)) in STYLES.iter().zip(instances) {
            for string in named(subfamily) {
                assert_eq!(string, style, "{file}'s instance style name");
            }
            for string in named(postscript) {
                assert_eq!(
                    string,
                    &format!("{postscript_family}-{style}"),
                    "{file}'s instance PostScript name"
                );
            }
        }
    }
}

#[test]
fn every_bundled_family_names_itself_and_its_style() {
    for (family, style, file) in FAMILIES {
        let names = names(&read(file));
        for (id, want) in [(1, family), (2, style)] {
            let found: Vec<&String> = names
                .iter()
                .filter(|(name_id, _)| *name_id == id)
                .map(|(_, string)| string)
                .collect();
            assert!(!found.is_empty(), "{file} has no `name` record {id}");
            for string in found {
                assert_eq!(string, want, "{file}'s `name` record {id}");
            }
        }
    }
}

#[test]
fn the_quattro_italic_word_space_is_its_romans() {
    let roman = advance(&read("QuillQuattro.ttf"), SPACE_GLYPH);
    let italic = advance(&read("QuillQuattroItalic.ttf"), SPACE_GLYPH);
    assert_eq!(roman, SPACE_ADVANCE, "Quattro's word space moved");
    assert_eq!(
        italic, roman,
        "an italic Quattro word would set wider than the same word upright"
    );
}

/// One font file, read from the data directory this build resolves to.
fn read(file: &str) -> Vec<u8> {
    let path: PathBuf = data::fonts().join(file);
    fs::read(&path).unwrap_or_else(|err| {
        panic!(
            "{} is readable: {err}. `python3 tools/fontbuild.py` writes the Faces; Inter and \
             Source Serif 4 are committed as their releases ship them",
            path.display()
        )
    })
}

/// Every `name` record in the file, as (name_id, string).
///
/// Platform 3 stores UTF-16BE and platform 1 stores one byte per character;
/// both decode to the ASCII the Quill names are written in.
fn names(font: &[u8]) -> Vec<(u16, String)> {
    let start = table(font, b"name");
    let count = be16(font, start + 2) as usize;
    let strings = start + be16(font, start + 4) as usize;
    (0..count)
        .map(|index| {
            let record = start + 6 + index * 12;
            let platform = be16(font, record);
            let name_id = be16(font, record + 6);
            let length = be16(font, record + 8) as usize;
            let at = strings + be16(font, record + 10) as usize;
            let bytes = &font[at..at + length];
            let string = if platform == 3 {
                let units: Vec<u16> = bytes
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|pair| u16::from_be_bytes(*pair))
                    .collect();
                String::from_utf16_lossy(&units)
            } else {
                bytes.iter().map(|&byte| byte as char).collect()
            };
            (name_id, string)
        })
        .collect()
}

/// Each `fvar` named instance, as the `name` IDs of its style and its
/// PostScript name.
///
/// `fvar`'s header is version (4 bytes), the offset to the axes, a reserved
/// field, then the axis and instance counts and sizes; an instance is its style
/// name ID, flags, one 16.16 coordinate per axis, and the PostScript name ID.
fn instances(font: &[u8]) -> Vec<(u16, u16)> {
    let fvar = table(font, b"fvar");
    let axes = be16(font, fvar + 8);
    let count = be16(font, fvar + 12) as usize;
    let size = be16(font, fvar + 14) as usize;
    let first =
        fvar + be16(font, fvar + 4) as usize + axes as usize * be16(font, fvar + 10) as usize;
    (0..count)
        .map(|index| {
            let instance = first + index * size;
            (
                be16(font, instance),
                be16(font, instance + 4 + 4 * axes as usize),
            )
        })
        .collect()
}

/// A glyph's advance width, in font units.
fn advance(font: &[u8], glyph: usize) -> u16 {
    let hmtx = table(font, b"hmtx");
    be16(font, hmtx + glyph * 4)
}

/// Where `tag` begins in `font`, from the table directory.
fn table(font: &[u8], tag: &[u8; 4]) -> usize {
    let count = be16(font, 4) as usize;
    (0..count)
        .map(|index| 12 + index * 16)
        .find(|record| &font[*record..*record + 4] == tag)
        .map(|record| be32(font, record + 8) as usize)
        .unwrap_or_else(|| panic!("the file has no `{}` table", String::from_utf8_lossy(tag)))
}

fn be16(bytes: &[u8], at: usize) -> u16 {
    u16::from_be_bytes([bytes[at], bytes[at + 1]])
}

fn be32(bytes: &[u8], at: usize) -> u32 {
    u32::from_be_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}
