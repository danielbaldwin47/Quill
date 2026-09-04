//! Writing a Document out as a PDF file.
//!
//! Export's own sink: a `cairo::PdfSurface` at the paper's size, every page of
//! it painted by [`crate::draw`], the metadata a reader's window title and
//! library read, and the heading outline as the bookmarks a reader's sidebar
//! shows. Print is the other sink for the same pages and the same drawer
//! (`docs/architecture.md` § Preview and Export), and neither knows about the
//! other.
//!
//! Pango draws the text through pangocairo rather than cairo's own toy API, so
//! the faces a Template names subset into the file and the page reads on a
//! machine that has none of them installed.

use std::path::Path;

use crate::document::Document;
use crate::draw;
use crate::outline;
use crate::paginate::{self, Geometry};
use crate::render;
use crate::template::Template;

/// What the file says made it.
const CREATOR: &str = "Quill";

/// Writes `document` to `path` as a PDF.
///
/// `paper` is the `[export]` geometry with the paper already resolved to
/// points ([`Geometry::of`]) and `size` the body size in points. The page is
/// laid out here ([`paginate::lay_out`], which Print calls too), so a caller
/// hands over a Document and gets a file.
///
/// What the furniture and the metadata say is the Document's own
/// ([`crate::draw::wording`]) rather than a caller's: the Title is the title
/// page's rule ([`paginate::Wording::title_or_name`]), the Author the front
/// matter's,
/// and the Creator [`CREATOR`]. The outline is one bookmark per heading,
/// nested by level, each pointing at the page and the offset the paginator
/// placed its heading at.
///
/// # Errors
///
/// [`cairo::Error`] when the surface cannot be created — a path in no
/// directory, or one nothing may be written to — or when a draw or the final
/// flush fails.
pub fn write(
    path: &Path,
    document: &Document,
    template: &Template,
    toggles: render::Toggles,
    paper: Geometry,
    size: f64,
) -> Result<(), cairo::Error> {
    use pango::prelude::FontMapExt;

    let context = pangocairo::FontMap::default().create_context();
    let laid = paginate::lay_out(document, template, toggles, paper, size, &context);
    let surface = cairo::PdfSurface::new(paper.width, paper.height, path)?;
    surface.set_metadata(cairo::PdfMetadata::Title, &laid.wording.title_or_name())?;
    if let Some(author) = laid.wording.author.as_deref() {
        surface.set_metadata(cairo::PdfMetadata::Author, author)?;
    }
    surface.set_metadata(cairo::PdfMetadata::Creator, CREATOR)?;
    {
        let cr = cairo::Context::new(&surface)?;
        for page in &laid.pages {
            draw::draw(&cr, page, &laid.rendered, template, &laid.frame);
            cr.show_page()?;
        }
    }
    // After the pages, because a bookmark names a page cairo has to have
    // emitted before it can point at it.
    bookmarks(
        &surface,
        &outline::of(&laid.rendered),
        &laid.pages,
        &laid.frame,
    )?;
    surface.finish();
    surface.status()
}

/// Adds one bookmark per heading, nested by level.
///
/// A heading deeper than the one before it hangs under it, and one at the same
/// level or shallower closes every entry it has outlived: a `###` after a `#`
/// with no `##` between them hangs off the `#`, because a reader's sidebar
/// shows what was written rather than what was skipped.
fn bookmarks(
    surface: &cairo::PdfSurface,
    headings: &[outline::Heading],
    pages: &[paginate::Page],
    frame: &crate::paginate::Frame,
) -> Result<(), cairo::Error> {
    let mut open: Vec<(u8, i32)> = Vec::new();
    for heading in headings {
        let Some((page, y)) = placed(pages, heading.block) else {
            continue;
        };
        while open
            .last()
            .is_some_and(|(level, _)| *level >= heading.level)
        {
            open.pop();
        }
        let parent = open.last().map_or(cairo::PDF_OUTLINE_ROOT, |(_, id)| *id);
        let id = surface.add_outline(
            parent,
            &heading.text,
            &format!("page={page} pos=[{:.2} {y:.2}]", frame.left),
            cairo::PdfOutline::OPEN,
        )?;
        open.push((heading.level, id));
    }
    Ok(())
}

/// The page `block` opens on, counting from 1 as a PDF does, and its top edge
/// there.
///
/// `None` when no page carries it, which a page list cut from another rendered
/// page is the only way to reach.
fn placed(pages: &[paginate::Page], block: usize) -> Option<(usize, f64)> {
    pages.iter().enumerate().find_map(|(at, page)| {
        let fragment = page
            .fragments
            .iter()
            .find(|fragment| fragment.block == block)?;
        Some((at + 1, fragment.y))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::{Export, Paper};
    use crate::template;
    use std::fs;
    use std::ops::Range;
    use std::path::PathBuf;
    use std::process::Command;

    /// The shared test passages, which are what a reader is asked to read.
    const SAMPLE: &str = "../ref/sample.md";
    const SHORT: &str = "../ref/short.md";

    /// The resolution a raster is read at: two device pixels to the point, so
    /// a hairline is a whole pixel and a half-pixel error is visible.
    const DPI: u32 = 144;

    /// Anything not the paper. The drawer paints on white, so a pixel under
    /// this carries ink of some kind.
    const INK: u8 = 250;

    /// Whether poppler is on this machine. These tests read what they wrote
    /// back through it; with none they say so in one line and pass, so
    /// `cargo test` stays green on a bare tree.
    fn poppler() -> bool {
        let found = Command::new("pdfinfo").arg("-v").output().is_ok();
        if !found {
            eprintln!(
                "skipped: poppler (pdfinfo, pdftotext, pdftoppm, pdftohtml) is not installed"
            );
        }
        found
    }

    /// The default Template, which is the one every page here is set in.
    fn modern() -> Template {
        template::built_in("modern").expect("a built-in Template")
    }

    /// The repository file `name`, as a Document.
    fn passage(name: &str) -> Document {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(name);
        Document::open(&path).expect("a passage the repository carries")
    }

    /// `text` as an untitled Document.
    fn written_out(text: &str) -> Document {
        let mut document = Document::untitled();
        document.reload(text.to_owned());
        document
    }

    /// A scratch path for `name`, with the pid in it because worktrees test
    /// concurrently.
    fn scratch(name: &str, extension: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "quill-pdf-{}-{name}.{extension}",
            std::process::id()
        ))
    }

    /// The `[export]` defaults on A4, with the furniture as asked for.
    fn a4(header: bool, footer: bool, title_page: bool) -> Geometry {
        let mut export = Export::default();
        export.paper = Paper::A4;
        Geometry {
            header,
            footer,
            title_page,
            ..Geometry::of(&export)
        }
    }

    /// The body size every page here is set at: the `[export]` default.
    fn size() -> f64 {
        f64::from(Export::default().text_size)
    }

    /// Writes `document` to a scratch PDF named `name` and answers where it
    /// went.
    fn exported(
        name: &str,
        document: &Document,
        paper: Geometry,
        toggles: render::Toggles,
    ) -> PathBuf {
        let path = scratch(name, "pdf");
        write(&path, document, &modern(), toggles, paper, size()).expect("the PDF is written");
        path
    }

    /// What `pdfinfo` gives `key`, or the empty string when it names no such
    /// key.
    fn said(path: &Path, key: &str) -> String {
        let out = Command::new("pdfinfo")
            .arg(path)
            .output()
            .expect("pdfinfo runs");
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .filter_map(|line| line.split_once(':'))
            .find(|(found, _)| found.trim() == key)
            .map_or_else(String::new, |(_, value)| value.trim().to_owned())
    }

    /// How many pages `path` has.
    fn pages_of(path: &Path) -> usize {
        said(path, "Pages").parse().expect("a page count")
    }

    /// The text of page `page` of `path`.
    fn text(path: &Path, page: usize) -> String {
        let page = page.to_string();
        let out = Command::new("pdftotext")
            .args(["-f", &page, "-l", &page])
            .arg(path)
            .arg("-")
            .output()
            .expect("pdftotext runs");
        String::from_utf8_lossy(&out.stdout).into_owned()
    }

    /// One bookmark: how deep it is nested, which page it points at, and what
    /// it says.
    #[derive(Debug, Eq, PartialEq)]
    struct Bookmark {
        level: usize,
        page: usize,
        text: String,
    }

    /// The bookmarks of `path`, in the order the reader's sidebar lists them.
    ///
    /// `pdftohtml -xml` is the poppler tool that prints the outline, as one
    /// `<outline>` per level around `<item page="N">` rows; the depth of the
    /// nesting is the level, which is what a bookmark tree is.
    fn bookmarks(path: &Path) -> Vec<Bookmark> {
        let out = Command::new("pdftohtml")
            .args(["-xml", "-stdout"])
            .arg(path)
            .output()
            .expect("pdftohtml runs");
        let xml = String::from_utf8_lossy(&out.stdout).into_owned();
        let mut found = Vec::new();
        let mut level = 0;
        for line in xml.lines() {
            let line = line.trim();
            if line == "<outline>" {
                level += 1;
            } else if line == "</outline>" {
                level -= 1;
            } else if let Some(rest) = line.strip_prefix("<item page=\"") {
                let Some((page, rest)) = rest.split_once("\">") else {
                    continue;
                };
                found.push(Bookmark {
                    level,
                    page: page.parse().expect("a page number"),
                    text: rest.trim_end_matches("</item>").to_owned(),
                });
            }
        }
        found
    }

    /// One page of a PDF, rastered: how wide it is and one grey per pixel,
    /// 0 black and 255 white.
    struct Raster {
        width: usize,
        height: usize,
        grey: Vec<u8>,
    }

    impl Raster {
        /// Page `page` of `path` at [`DPI`], through `pdftoppm`'s grey PGM.
        fn of(path: &Path, page: usize) -> Self {
            let page = page.to_string();
            let out = Command::new("pdftoppm")
                .args(["-gray", "-r", &DPI.to_string(), "-f", &page, "-l", &page])
                .arg(path)
                .output()
                .expect("pdftoppm runs");
            let bytes = out.stdout;
            // `P5\n<width> <height>\n255\n`, then one byte a pixel.
            let mut fields = Vec::new();
            let mut at = 0;
            for _ in 0..4 {
                let end = bytes[at..]
                    .iter()
                    .position(u8::is_ascii_whitespace)
                    .expect("a PGM header field")
                    + at;
                fields.push(String::from_utf8_lossy(&bytes[at..end]).into_owned());
                at = end + 1;
            }
            assert_eq!(fields[0], "P5", "pdftoppm writes a binary grey map");
            let width = fields[1].parse().expect("a raster width");
            let height = fields[2].parse().expect("a raster height");
            Self {
                width,
                height,
                grey: bytes[at..].to_vec(),
            }
        }

        /// The leftmost and rightmost columns carrying ink between the rows
        /// `band`, or `None` when that band is bare paper.
        fn ink(&self, band: Range<usize>) -> Option<(usize, usize)> {
            let mut edges: Option<(usize, usize)> = None;
            for row in band {
                for column in 0..self.width {
                    if self.grey[row * self.width + column] < INK {
                        edges = Some(match edges {
                            Some((left, right)) => (left.min(column), right.max(column)),
                            None => (column, column),
                        });
                    }
                }
            }
            edges
        }

        /// The row `points` from the paper's top edge.
        fn row(&self, points: f64) -> usize {
            (points * f64::from(DPI) / 72.0) as usize
        }

        /// The grey at `x`, `y` points from the paper's top-left corner.
        fn grey_at(&self, x: f64, y: f64) -> u8 {
            self.grey[self.row(y) * self.width + self.row(x)]
        }
    }

    #[test]
    fn the_sample_is_a4_at_twelve_points_with_the_metadata_the_rules_name() {
        if !poppler() {
            return;
        }
        let path = exported(
            "sample",
            &passage(SAMPLE),
            a4(false, false, false),
            render::Toggles::default(),
        );
        // No front matter, so the Title is the Document's name, which is the
        // file's own.
        assert_eq!(said(&path, "Title"), "sample");
        assert_eq!(said(&path, "Author"), "");
        assert_eq!(said(&path, "Creator"), CREATOR);
        let paper = said(&path, "Page size");
        assert!(paper.starts_with("595.276 x 841.89 pts (A4)"), "{paper}");
        assert!(text(&path, 1).contains("What the sea keeps"));
        fs::remove_file(&path).expect("the scratch file goes");
    }

    #[test]
    fn the_short_passage_is_one_page_and_letter_is_the_paper_it_is_asked_for() {
        if !poppler() {
            return;
        }
        let mut paper = a4(false, false, false);
        let (width, height) = Paper::Letter.size();
        paper.width = width;
        paper.height = height;
        let path = exported("short", &passage(SHORT), paper, render::Toggles::default());
        assert_eq!(pages_of(&path), 1);
        assert!(text(&path, 1).contains("The Note on the Table"));
        let said = said(&path, "Page size");
        assert!(said.starts_with("612 x 792 pts (letter)"), "{said}");
        fs::remove_file(&path).expect("the scratch file goes");
    }

    #[test]
    fn the_front_matter_names_the_title_and_the_author_the_file_carries() {
        if !poppler() {
            return;
        }
        // The `lamp` span and the link are here because the drawer paints an
        // inline code ground and a link's own ink as passes of their own, and
        // a page with neither would never walk them.
        let document = written_out(
            "---\ntitle: The Lighthouse\nauthor: A. Writer\n---\n\n\
             The `lamp` turned, and [the keeper](https://example.invalid) wrote.\n",
        );
        let path = exported(
            "front",
            &document,
            a4(false, false, false),
            render::Toggles::default(),
        );
        assert_eq!(said(&path, "Title"), "The Lighthouse");
        assert_eq!(said(&path, "Author"), "A. Writer");
        let page = text(&path, 1);
        assert!(page.contains("lamp"), "{page}");
        assert!(page.contains("the keeper"), "{page}");
        fs::remove_file(&path).expect("the scratch file goes");
    }

    /// The sample is one page of A4, so a Document that runs over one is the
    /// sample three times: the page turn, and the bookmarks that follow it,
    /// are what this measures.
    #[test]
    fn a_document_over_a_page_long_carries_a_bookmark_per_heading_on_its_own_page() {
        if !poppler() {
            return;
        }
        let sample = fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(SAMPLE))
            .expect("the sample reads");
        let document = written_out(&format!("{sample}\n{sample}\n{sample}"));
        let path = exported(
            "long",
            &document,
            a4(false, false, false),
            render::Toggles::default(),
        );
        let pages = pages_of(&path);
        assert!(pages > 1, "three samples run past one page: {pages}");
        let found = bookmarks(&path);
        assert_eq!(
            found
                .iter()
                .map(|entry| (entry.level, entry.text.as_str()))
                .collect::<Vec<_>>(),
            vec![
                (1, "The Lighthouse"),
                (2, "What the sea keeps"),
                (1, "The Lighthouse"),
                (2, "What the sea keeps"),
                (1, "The Lighthouse"),
                (2, "What the sea keeps"),
            ]
        );
        assert!(
            found.windows(2).all(|pair| pair[0].page <= pair[1].page),
            "the bookmarks run down the file: {found:?}"
        );
        assert_eq!(found.last().expect("a last bookmark").page, pages);
        for entry in &found {
            assert!(
                text(&path, entry.page).contains(&entry.text),
                "{entry:?} is on the page it points at"
            );
        }
        fs::remove_file(&path).expect("the scratch file goes");
    }

    #[test]
    fn number_headings_puts_the_number_in_the_bookmark_the_reader_sees() {
        if !poppler() {
            return;
        }
        let toggles = render::Toggles {
            number_headings: true,
            ..render::Toggles::default()
        };
        let path = exported(
            "numbered",
            &passage(SAMPLE),
            a4(false, false, false),
            toggles,
        );
        let found = bookmarks(&path);
        assert_eq!(found[0].text, "The Lighthouse");
        assert!(
            found[1].text.starts_with('1'),
            "the first second-level bookmark begins 1: {}",
            found[1].text
        );
        fs::remove_file(&path).expect("the scratch file goes");
    }

    /// A thematic break is the text block's own width and nothing else's, so
    /// its ink says where the block stands to the pixel; a paragraph's ragged
    /// right edge could not.
    ///
    /// The blank line in front of it is because a `---` on the first line of a
    /// file is front matter and not a rule at all.
    #[test]
    fn the_text_block_is_centred_on_the_paper_and_the_paper_is_white() {
        if !poppler() {
            return;
        }
        let paper = a4(false, false, false);
        let path = exported(
            "rule",
            &written_out("\n---\n"),
            paper,
            render::Toggles::default(),
        );
        let raster = Raster::of(&path, 1);
        assert_eq!(raster.grey[0], 255, "the paper is white in the corner");
        let (left, right) = raster.ink(0..raster.height).expect("the rule's ink");
        let middle = (left + right + 1) as f64 / 2.0;
        assert!(
            (middle - raster.width as f64 / 2.0).abs() <= 1.0,
            "the block is centred: {left}..{right} of {}",
            raster.width
        );
        let frame = paginate::frame(paper, &modern(), size());
        assert!(
            (left as f64 - frame.left * f64::from(DPI) / 72.0).abs() <= 1.0,
            "the block's left edge is the frame's: {left}"
        );
        fs::remove_file(&path).expect("the scratch file goes");
    }

    #[test]
    fn the_footer_stands_inside_the_bottom_margin_and_nowhere_when_it_is_off() {
        if !poppler() {
            return;
        }
        let document = passage(SHORT);
        let on = exported(
            "footer-on",
            &document,
            a4(false, true, false),
            render::Toggles::default(),
        );
        let off = exported(
            "footer-off",
            &document,
            a4(false, false, false),
            render::Toggles::default(),
        );
        let paper = a4(false, true, false);
        let lit = Raster::of(&on, 1);
        let band = lit.row(paper.height - paper.margin)..lit.height;
        // Ink found here is ink inside the bottom margin: the band is the
        // rows between the body's foot and the paper's edge.
        let (left, right) = lit.ink(band.clone()).expect("the page number's ink");
        let middle = (left + right + 1) as f64 / 2.0;
        assert!(
            (middle - lit.width as f64 / 2.0).abs() <= 1.0,
            "the page number is centred: {left}..{right}"
        );
        assert!(
            lit.ink(0..lit.row(paper.margin)).is_none(),
            "no header is printed with the header off"
        );
        let dark = Raster::of(&off, 1);
        assert_eq!(
            dark.ink(band),
            None,
            "the bottom margin is bare with the footer off"
        );
        fs::remove_file(&on).expect("the scratch file goes");
        fs::remove_file(&off).expect("the scratch file goes");
    }

    /// The Well ground under a code block cut across a page break opens on
    /// one page and closes on the next, so the middle page's ground runs the
    /// whole band with neither end of it padded.
    #[test]
    fn a_code_blocks_ground_carries_across_the_page_break() {
        if !poppler() {
            return;
        }
        let mut paper = a4(false, false, false);
        // Room for about five lines, so twelve of them run over three pages.
        paper.height = 2.0f64.mul_add(paper.margin, 90.0);
        let lines = (1..=12)
            .map(|at| format!("let step = {at};"))
            .collect::<Vec<_>>()
            .join("\n");
        let document = written_out(&format!("```rust\n{lines}\n```\n"));
        let path = exported("well", &document, paper, render::Toggles::default());
        assert!(pages_of(&path) >= 3, "twelve lines run over three pages");
        let frame = paginate::frame(paper, &modern(), size());
        let raster = Raster::of(&path, 2);
        // The right-hand end of the well, where no code line reaches.
        let x = frame.left + frame.measure - 4.0;
        let ground = raster.grey_at(x, frame.top + 2.0);
        assert!(
            (200..250).contains(&ground),
            "the ground opens the middle page: {ground}"
        );
        assert_eq!(
            raster.grey_at(x, frame.top - 4.0),
            255,
            "and stops at the band, with the margin left as paper"
        );
        fs::remove_file(&path).expect("the scratch file goes");
    }

    #[test]
    fn a_title_page_carries_its_own_lines_alone_and_the_body_starts_after_it() {
        if !poppler() {
            return;
        }
        let document = written_out(
            "---\ntitle: The Lighthouse\nauthor: A. Writer\ndate: 2026-09-04\n---\n\n\
             The lamp had been lit for an hour.\n",
        );
        let path = exported(
            "title-page",
            &document,
            a4(false, true, true),
            render::Toggles::default(),
        );
        assert_eq!(pages_of(&path), 2);
        let first = text(&path, 1);
        assert!(first.contains("The Lighthouse"), "{first}");
        assert!(first.contains("A. Writer"), "{first}");
        assert!(first.contains("2026-09-04"), "{first}");
        assert!(!first.contains("The lamp"), "{first}");
        // Unnumbered, and the body starts at 1 whatever stands before it.
        assert!(!first.contains('1'), "{first}");
        assert!(text(&path, 2).trim_end().ends_with('1'));
        fs::remove_file(&path).expect("the scratch file goes");
    }
}
