//! The Markdown parser, behind one shared set of options.
//!
//! Every part of Quill that reads Markdown — the Editor's keystroke path, the
//! Preview, Export, Stats, the outline — parses through this module, so a
//! Document is never interpreted two ways. It emits two streams: the token
//! stream the Markup Annotator marks up, and the *prose stream* the other three
//! Annotators consume, which is the `Text` events with Markup, code spans,
//! fenced code, URLs and front matter removed.
