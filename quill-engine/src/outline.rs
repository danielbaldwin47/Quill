//! The heading outline, for Heading navigation and PDF bookmarks.
//!
//! The list of a Document's headings with their byte ranges and levels. Like
//! Stats it is a whole-Document pass that runs on idle after the synchronous
//! keystroke lane. Export reads it for PDF bookmarks; a Preview table of
//! contents is a Preview feature built on the same list.
