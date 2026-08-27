//! Rendering a whole Document with Pango, for Preview, PDF and HTML.
//!
//! One layout pass from the current [`crate::template`] produces both the
//! layouts the Preview widget snapshots and the pages the PDF surface draws, so
//! Print and Export to PDF are one path. HTML export is the parser's HTML plus
//! the CSS the Template generates, inlined. Annotator marks never reach here:
//! Preview and Export show the Document, not the Editor's styling.
//!
//! This is why the engine may depend on `pango` and `cairo` — layout is not a
//! widget — while `gtk` stays out (ADR 0008).
