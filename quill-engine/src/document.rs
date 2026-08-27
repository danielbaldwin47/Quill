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

/// One Markdown file, plus the engine's copy of its text.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Document {
    path: Option<PathBuf>,
    text: String,
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
        Ok(Self {
            text: std::fs::read_to_string(path)?,
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

    /// The name to show for this Document: its file name, or [`UNTITLED`].
    #[must_use]
    pub fn title(&self) -> Cow<'_, str> {
        self.path
            .as_deref()
            .and_then(Path::file_name)
            .map_or(Cow::Borrowed(UNTITLED), std::ffi::OsStr::to_string_lossy)
    }
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

    fn write_temp(stem: &str, text: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("quill-engine-{stem}.md"));
        std::fs::write(&path, text).expect("writes to the temp directory");
        path
    }
}
