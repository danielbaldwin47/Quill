//! Documents: text and, later, the block index.
//!
//! A Document is one Markdown file on disk, and the file is the only source of
//! truth. The engine keeps its own `String` copy of the text, spliced from the
//! buffer's edit signals; the block index and the splice arrive with the
//! keystroke path. Today a Document is the two things a window needs to show
//! one: a path and its text.

use std::borrow::Cow;
use std::io;
use std::path::{Path, PathBuf};

/// What a window titles a Document that is not on disk yet.
pub const UNTITLED: &str = "Untitled";

/// Where a byte offset is, in the two numbers a `GtkTextIter` is set from.
///
/// The app reaches a byte with `set_line` and then `set_line_index`, never by
/// counting characters from the top of the buffer, because that is O(document)
/// per lookup and there is one lookup per span.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Place {
    /// The line the byte is on, counted from 0.
    pub line: usize,
    /// How many bytes into that line it is, counted from the line's first byte.
    pub index: usize,
}

/// One Markdown file, plus the engine's copy of its text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Document {
    path: Option<PathBuf>,
    text: String,
    /// The byte each line starts at, ascending, always beginning with 0.
    ///
    /// Built once when the text is read. The ticket that lands the keystroke
    /// path makes it incremental; until then a Document's text never changes
    /// after it is opened.
    lines: Vec<usize>,
}

impl Default for Document {
    fn default() -> Self {
        Self {
            path: None,
            text: String::new(),
            lines: vec![0],
        }
    }
}

impl Document {
    /// A Document with no file behind it and nothing written in it.
    #[must_use]
    pub fn untitled() -> Self {
        Self::default()
    }

    /// Reads `path` from disk.
    ///
    /// # Errors
    ///
    /// Returns the underlying [`io::Error`] when the file cannot be read, so
    /// the caller decides what a writer sees.
    pub fn open(path: &Path) -> io::Result<Self> {
        let text = std::fs::read_to_string(path)?;
        Ok(Self {
            lines: line_starts(&text),
            text,
            path: Some(path.to_path_buf()),
        })
    }

    /// The file this Document is, or `None` while it is untitled.
    #[must_use]
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// The Document's text.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Where `offset` is, as the line and the byte index within it.
    ///
    /// An offset past the end of the text lands at the end, and an offset
    /// inside a character lands after that character: the Annotators only ever
    /// name boundaries, but `--caret` names a byte an owner chose, and neither
    /// is worth refusing a launch over.
    #[must_use]
    pub fn place(&self, offset: usize) -> Place {
        let mut offset = offset.min(self.text.len());
        while !self.text.is_char_boundary(offset) {
            offset += 1;
        }
        // The last line start at or before `offset`; `lines` always has a 0.
        let line = self.lines.partition_point(|&start| start <= offset) - 1;
        Place {
            line,
            index: offset - self.lines[line],
        }
    }

    /// The name to show for this Document: its file name, or [`UNTITLED`].
    #[must_use]
    pub fn title(&self) -> Cow<'_, str> {
        self.path
            .as_deref()
            .and_then(Path::file_name)
            .map_or(Cow::Borrowed(UNTITLED), std::ffi::OsStr::to_string_lossy)
    }
}

/// The byte each line of `text` starts at.
///
/// Lines are split on `\n` alone, which covers `\n` and `\r\n` both: the `\r`
/// of a CRLF file belongs to the line it ends, and `GtkTextIter` counts it
/// there too, so the two agree on every offset a writer can reach.
fn line_starts(text: &str) -> Vec<usize> {
    let mut starts = vec![0];
    starts.extend(
        text.bytes()
            .enumerate()
            .filter(|&(_, byte)| byte == b'\n')
            .map(|(at, _)| at + 1),
    );
    starts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn untitled_has_no_path_and_no_text() {
        let doc = Document::untitled();
        assert_eq!(doc.path(), None);
        assert_eq!(doc.text(), "");
        assert_eq!(doc.title(), UNTITLED);
    }

    #[test]
    fn open_reads_the_file_and_remembers_its_path() {
        let path = write_temp("open_reads", "# The Lighthouse\n\nThe lamp had been lit.\n");
        let doc = Document::open(&path).expect("reads the file just written");
        assert_eq!(doc.text(), "# The Lighthouse\n\nThe lamp had been lit.\n");
        assert_eq!(doc.path(), Some(path.as_path()));
        assert_eq!(doc.title(), "quill-engine-open_reads.md");
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn open_of_a_missing_file_is_an_error_not_an_empty_document() {
        let path = std::env::temp_dir().join("quill-no-such-document.md");
        std::fs::remove_file(&path).ok();
        let err = Document::open(&path).expect_err("the file does not exist");
        assert_eq!(err.kind(), io::ErrorKind::NotFound);
    }

    #[test]
    fn an_empty_file_is_a_document_with_a_title() {
        let path = write_temp("empty_file", "");
        let doc = Document::open(&path).expect("reads the empty file");
        assert_eq!(doc.text(), "");
        assert_eq!(doc.title(), "quill-engine-empty_file.md");
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn an_untitled_document_has_one_empty_line() {
        let doc = Document::untitled();
        assert_eq!(doc.place(0), Place { line: 0, index: 0 });
    }

    #[test]
    fn every_byte_of_a_short_document_lands_on_the_line_it_is_written_on() {
        let path = write_temp("places", "# One\ntwo\n\nfour\n");
        let doc = Document::open(&path).expect("reads the file just written");
        for (offset, line, index) in [
            (0, 0, 0),
            (2, 0, 2),
            (5, 0, 5),
            (6, 1, 0),
            (9, 1, 3),
            (10, 2, 0),
            (11, 3, 0),
            (15, 3, 4),
            (16, 4, 0),
        ] {
            assert_eq!(
                doc.place(offset),
                Place { line, index },
                "byte {offset} is not on line {line} at index {index}"
            );
        }
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn an_offset_past_the_end_lands_at_the_end_rather_than_panicking() {
        let path = write_temp("past_the_end", "abc\n");
        let doc = Document::open(&path).expect("reads the file just written");
        assert_eq!(doc.place(9_999), doc.place(doc.text().len()));
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn the_em_dash_in_the_sample_passage_shifts_bytes_without_shifting_lines() {
        let doc = Document::open(Path::new("../ref/sample.md"))
            .expect("the shared test passage is in the repo");
        let text = doc.text();
        let dash = text.find('—').expect("the sample passage has an em dash");
        let place = doc.place(dash);

        // The em dash is three UTF-8 bytes, so every byte after it on the line
        // sits three further along than its character count would suggest.
        let line = text
            .lines()
            .nth(place.line)
            .expect("the line the dash is on");
        assert_eq!(&line[place.index..place.index + 3], "—");
        assert!(
            line.chars().count() < line.len(),
            "the line must be the multi-byte case, or this test proves nothing"
        );
        assert_eq!(
            doc.place(dash + 3),
            Place {
                line: place.line,
                index: place.index + 3
            },
            "the byte after the dash is still on the same line"
        );
        for inside in 1..3 {
            assert_eq!(
                doc.place(dash + inside),
                doc.place(dash + 3),
                "a byte inside the dash lands after it, never inside it"
            );
        }
    }

    #[test]
    fn a_heading_span_reaches_the_line_and_index_the_app_sets_an_iter_from() {
        let doc = Document::open(Path::new("../shots/oracle/markup.md"))
            .expect("the judged Markup passage is in the repo");
        let spans = crate::annotate::markup(doc.text());
        let first = spans.first().expect("the passage opens with a heading");
        assert_eq!(
            doc.place(first.at.start),
            Place { line: 0, index: 0 },
            "the first heading's marker starts the file"
        );
        let second = doc.place(spans[2].at.start).line;
        assert_eq!(second, 4, "`## What the sea keeps` is the fifth line");
    }

    fn write_temp(stem: &str, text: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("quill-engine-{stem}.md"));
        std::fs::write(&path, text).expect("writes to the temp directory");
        path
    }
}
