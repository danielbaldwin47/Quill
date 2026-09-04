//! The heading outline, for Heading navigation and PDF bookmarks.
//!
//! The list of a Document's headings with their byte ranges and levels. Like
//! Stats it is a whole-Document pass that runs on idle after the synchronous
//! keystroke lane. Export reads it for PDF bookmarks; a Preview table of
//! contents is a Preview feature built on the same list.
//!
//! It is read off a [rendered page](crate::render::Page) rather than off the
//! Document's own block index, because a bookmark's words are the words on the
//! page: Number Headings puts `1`, `1.1` in front of a heading during the
//! render pass, and a list built from the file's bytes would name headings the
//! reader never sees. The rendered block carries the Document bytes it came
//! from, so [`Heading::source`] is still what Heading navigation moves the
//! caret to.

use std::ops::Range;

use crate::render;

/// One heading of a rendered Document.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Heading {
    /// Its level, 1 to 6, as Markdown writes them.
    pub level: u8,
    /// What it says, as the page has it: the number Number Headings sets in
    /// front of it included, because that is what the reader reads.
    pub text: String,
    /// The index into [`crate::render::Page::blocks`] of the block it is,
    /// which is where Export finds the page and the offset the paginator
    /// placed it at.
    pub block: usize,
    /// The Document bytes it was rendered from.
    pub source: Range<usize>,
}

/// The headings of `page`, in reading order.
///
/// One walk of the page's blocks and of each heading's layouts, so the pass is
/// linear in the Document's blocks and nothing here runs on the keystroke lane.
#[must_use]
pub fn of(page: &render::Page) -> Vec<Heading> {
    page.blocks
        .iter()
        .enumerate()
        .filter_map(|(block, rendered)| {
            let render::Kind::Heading { level } = rendered.kind else {
                return None;
            };
            Some(Heading {
                level,
                text: said(rendered),
                block,
                source: rendered.source.clone(),
            })
        })
        .collect()
}

/// What `block`'s layouts say, run together.
///
/// A heading is one layout in every Template Quill ships; the join is here so
/// that one which wrapped a heading over two layouts would still name itself
/// whole rather than by its first half.
fn said(block: &render::Block) -> String {
    block
        .layouts
        .iter()
        .map(|placed| placed.layout.text().to_string())
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::Document;
    use crate::render::Toggles;
    use crate::template;

    /// Two levels, a paragraph between them, and a heading that is the last
    /// block: enough for an outline to be nested and to end on a heading.
    const FIXTURE: &str = "# The Lighthouse\n\n\
                           A paragraph.\n\n\
                           ## What the sea keeps\n\n\
                           Another paragraph.\n\n\
                           ### The rule\n";

    /// `text` rendered at the default Template, on a context with no display
    /// behind it.
    fn rendered(text: &str, toggles: Toggles) -> render::Page {
        use pango::prelude::FontMapExt;

        let mut document = Document::untitled();
        document.reload(text.to_string());
        render::render(
            &document,
            &template::built_in("modern").expect("a built-in Template"),
            toggles,
            400.0,
            100,
            &pangocairo::FontMap::default().create_context(),
        )
    }

    #[test]
    fn every_heading_is_an_entry_at_the_level_it_was_written() {
        let page = rendered(FIXTURE, Toggles::default());
        let found = of(&page);
        assert_eq!(
            found.iter().map(|entry| entry.level).collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
        assert_eq!(
            found
                .iter()
                .map(|entry| entry.text.as_str())
                .collect::<Vec<_>>(),
            vec!["The Lighthouse", "What the sea keeps", "The rule"]
        );
    }

    #[test]
    fn an_entry_names_the_rendered_block_and_the_bytes_it_came_from() {
        let page = rendered(FIXTURE, Toggles::default());
        for entry in of(&page) {
            let block = &page.blocks[entry.block];
            assert_eq!(block.kind, render::Kind::Heading { level: entry.level });
            assert_eq!(block.source, entry.source);
            assert!(FIXTURE[entry.source.clone()].contains(entry.text.as_str()));
        }
    }

    /// The bookmark's words are the page's words, which is the whole reason
    /// the outline is read off the render pass.
    #[test]
    fn number_headings_puts_the_number_in_the_entrys_text() {
        let toggles = Toggles {
            number_headings: true,
            ..Toggles::default()
        };
        let found = of(&rendered(FIXTURE, toggles));
        assert_eq!(found[0].text, "The Lighthouse");
        assert!(
            found[1].text.starts_with('1'),
            "the first second-level heading is numbered 1: {}",
            found[1].text
        );
    }

    #[test]
    fn a_document_with_no_headings_has_no_outline() {
        assert_eq!(of(&rendered("A paragraph.\n", Toggles::default())), vec![]);
    }
}
