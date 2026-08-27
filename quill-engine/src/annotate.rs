//! The Annotator trait, its spans, and flattening them into runs.
//!
//! An Annotator turns a byte range of a Document into spans, each a byte range
//! and a mark. Four exist: Markup, Syntax highlight ([`crate::pos`]), Style
//! check ([`crate::style`]) and Spell check ([`crate::spell`]). Markup runs
//! synchronously on the keystroke; the other three run on a worker thread
//! against a Document generation, and a result computed against a stale
//! generation is discarded.
//!
//! Overlapping `GtkTextTag`s override colour by priority rather than blending,
//! so Markup × Focus × Syntax highlight are flattened here into
//! non-overlapping runs carrying one precomputed colour each. Decorations
//! (underlines) stay separate: underline and colour are different properties,
//! so those overlaps are safe.
//!
//! All offsets are UTF-8 bytes from the start of the Document, because that is
//! what the parser emits.
