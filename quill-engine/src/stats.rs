//! Stats: word count, character count and reading time.
//!
//! Computed for the whole Document or for the selection, over the prose stream
//! so Markup never counts as a word. A whole-Document pass runs on idle after
//! the synchronous keystroke lane, never inside it.
