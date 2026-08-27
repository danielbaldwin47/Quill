//! Spell check: the `SpellChecker` trait and its implementations.
//!
//! The Annotator that underlines misspellings against a system dictionary, with
//! suggestions fetched on demand. Two implementations are planned behind one
//! trait: enchant through Quill's own `extern "C"` declarations, and
//! `spellbook`. Dictionary loading happens on the worker thread at first use,
//! never on a keystroke or at startup, and with no dictionary installed Spell
//! check shows a "no dictionary" state rather than failing.
//!
//! Like every prose Annotator it sees the prose stream, never a `#` or a `*`.
