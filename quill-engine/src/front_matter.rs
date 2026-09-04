//! The front matter a Document may open with, as far as Export reads it.
//!
//! A file whose first line is `---` opens with a metadata block, which
//! [`crate::markdown`] already parses and hides from the prose stream. Export
//! reads three keys out of it and no others: `title` names the title page and
//! the PDF's Title, `author` its byline and the PDF's Author, `date` the line
//! beneath them. Everything else in the block is the writer's own and is
//! carried nowhere.
//!
//! This is a reader and nothing more. Nothing is ever written back into the
//! block ([ADR 0002](../../docs/adr/0002-plain-markdown-documents.md)): the
//! file is the document, and what Quill has learned about a Document lives
//! beside it or not at all. It is also not YAML — a block is read line by
//! line, `key: value`, with a quoted value read as the same text a bare one is
//! — because the three keys Export uses are plain scalars and a YAML
//! dependency would buy nothing but the ways a block can be wrong.

use pulldown_cmark::{Event, Tag, TagEnd};

use crate::markdown;

/// What a Document's front matter says about the Document.
///
/// Each key is `None` when the block does not name it, when the block names it
/// with nothing after the colon, and when there is no block at all: a Document
/// with no front matter and one whose front matter is empty are the same
/// Document to Export.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FrontMatter {
    /// What the Document is called, for the title page and the PDF's Title.
    pub title: Option<String>,
    /// Who wrote it, for the title page and the PDF's Author.
    pub author: Option<String>,
    /// The date it carries, for the title page.
    pub date: Option<String>,
}

impl FrontMatter {
    /// Whether the block named nothing Export reads.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.title.is_none() && self.author.is_none() && self.date.is_none()
    }
}

/// The keys Export reads, in the order the title page sets them.
const KEYS: [&str; 3] = ["title", "author", "date"];

/// The front matter of `text`, or the empty [`FrontMatter`] when it has none.
///
/// Only the first block is read, because only the first is front matter: a
/// `---` further down a file is a thematic break, and the parser says so.
#[must_use]
pub fn read(text: &str) -> FrontMatter {
    let mut found = FrontMatter::default();
    let mut inside = false;
    for (event, _) in markdown::events(text) {
        match event {
            Event::Start(Tag::MetadataBlock(_)) => inside = true,
            Event::End(TagEnd::MetadataBlock(_)) => break,
            Event::Text(block) if inside => {
                for line in block.lines() {
                    take(&mut found, line);
                }
            }
            _ => {}
        }
    }
    found
}

/// Reads one `key: value` line into the key it names, if it names one of
/// [`KEYS`] and that key is still unset.
///
/// The first of two lines with one key wins, so that a writer who has written
/// `title` twice reads the same Document twice rather than one that depends on
/// which line the reader saw last.
fn take(found: &mut FrontMatter, line: &str) {
    let Some((key, value)) = line.split_once(':') else {
        return;
    };
    let value = unquoted(value.trim());
    if value.is_empty() {
        return;
    }
    let slot = match KEYS.iter().position(|known| *known == key.trim()) {
        Some(0) => &mut found.title,
        Some(1) => &mut found.author,
        Some(2) => &mut found.date,
        _ => return,
    };
    if slot.is_none() {
        *slot = Some(value.to_owned());
    }
}

/// `value` with one pair of matching quotes taken off it.
///
/// A writer quotes a value to keep a colon or a leading `#` inside it, and
/// means the text between the quotes; a value with a quote at one end only is
/// text that happens to start or end with a quote, and is left whole.
fn unquoted(value: &str) -> &str {
    for quote in ['"', '\''] {
        if let Some(inside) = value
            .strip_prefix(quote)
            .and_then(|v| v.strip_suffix(quote))
        {
            return inside;
        }
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A block naming all three, and a line the reader passes over.
    const FIXTURE: &str = "---\n\
                           title: The Lighthouse\n\
                           author: A. Writer\n\
                           date: 2026-09-04\n\
                           keywords: sea, storm\n\
                           ---\n\
                           \n\
                           The lamp turned all night.\n";

    #[test]
    fn the_three_keys_are_read_from_the_block_and_nothing_else_is() {
        assert_eq!(
            read(FIXTURE),
            FrontMatter {
                title: Some("The Lighthouse".to_owned()),
                author: Some("A. Writer".to_owned()),
                date: Some("2026-09-04".to_owned()),
            }
        );
    }

    #[test]
    fn a_document_with_no_block_names_nothing() {
        let found = read("# The Lighthouse\n\nThe lamp turned all night.\n");
        assert_eq!(found, FrontMatter::default());
        assert!(found.is_empty());
    }

    /// A `---` that is not the first thing in the file is a thematic break,
    /// and a break is not front matter however it is written.
    #[test]
    fn a_rule_further_down_the_file_is_not_front_matter() {
        assert_eq!(
            read("The lamp turned all night.\n\n---\n\ntitle: Not a title\n"),
            FrontMatter::default()
        );
    }

    #[test]
    fn a_quoted_value_and_a_bare_one_read_the_same() {
        let bare = read("---\ntitle: The Lighthouse\n---\n");
        let quoted = read("---\ntitle: \"The Lighthouse\"\n---\n");
        let single = read("---\ntitle: 'The Lighthouse'\n---\n");
        assert_eq!(bare.title.as_deref(), Some("The Lighthouse"));
        assert_eq!(quoted, bare);
        assert_eq!(single, bare);
    }

    /// A colon inside a quoted value belongs to the value: the split is at the
    /// first colon, and what follows it is one string however many colons it
    /// holds.
    #[test]
    fn a_value_keeps_the_colons_inside_it() {
        assert_eq!(
            read("---\ntitle: \"The Lighthouse: a life\"\n---\n")
                .title
                .as_deref(),
            Some("The Lighthouse: a life")
        );
    }

    /// A quote at one end only is a character of the value, not a wrapper.
    #[test]
    fn a_value_quoted_at_one_end_is_left_whole() {
        assert_eq!(
            read("---\nauthor: \"A. Writer\n---\n").author.as_deref(),
            Some("\"A. Writer")
        );
    }

    #[test]
    fn a_key_with_nothing_after_the_colon_names_nothing() {
        let found = read("---\ntitle:\nauthor:   \ndate: 2026-09-04\n---\n");
        assert_eq!(found.title, None);
        assert_eq!(found.author, None);
        assert_eq!(found.date.as_deref(), Some("2026-09-04"));
        assert!(!found.is_empty());
    }

    /// A line with no colon, and a key Export does not read, are both passed
    /// over rather than being read as the key before them.
    #[test]
    fn a_line_that_is_no_pair_is_passed_over() {
        assert_eq!(
            read("---\nnot a pair\nsubtitle: a life\n---\n"),
            FrontMatter::default()
        );
    }

    #[test]
    fn the_first_of_two_lines_with_one_key_is_the_one_read() {
        assert_eq!(
            read("---\ntitle: First\ntitle: Second\n---\n")
                .title
                .as_deref(),
            Some("First")
        );
    }
}
