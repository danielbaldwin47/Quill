//! The heading outline, for Heading navigation and PDF bookmarks.
//!
//! The list of a Document's headings with their byte ranges and levels, built
//! two ways for two readers, neither on the keystroke lane. Export reads
//! [`of`], off a [rendered page](crate::render::Page), for PDF bookmarks:
//! a bookmark's words are the words on the page, and Number Headings puts
//! `1`, `1.1` in front of a heading during the render pass, so a list built
//! from the file's bytes would name headings the reader never sees. Heading
//! navigation reads [`of_blocks`], off the Document's own block index, when
//! the Palette opens on the Outline (#397): its words are the Editor's, which
//! never carry a number, and it wants no render pass to open a list.
//!
//! [`section`] answers which heading's section a caret stands in, which is
//! the row the Outline opens on.

use std::ops::Range;

use crate::document::{self, Document};
use crate::render;

/// One heading of a Document.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Heading {
    /// Its level, 1 to 6, as Markdown writes them.
    pub level: u8,
    /// What it says: from a rendered page, the number Number Headings sets in
    /// front of it included, because that is what the reader reads; from the
    /// block index, the Editor's words with the heading's own markers gone.
    pub text: String,
    /// The index of the block it is: into [`crate::render::Page::blocks`]
    /// from [`of`], which is where Export finds the page and the offset the
    /// paginator placed it at; into [`Document::blocks`] from [`of_blocks`].
    pub block: usize,
    /// The Document bytes it is: the whole block from [`of`], the words alone
    /// — after the markers, before any underline — from [`of_blocks`], so
    /// that the start is where a jump puts the caret.
    pub source: Range<usize>,
}

/// The headings of `document`, in reading order, read off its block index.
///
/// One walk over the blocks, linear in the Document. A heading inside a
/// quote, a list or a code block is none of those blocks' kind and is not
/// here; front matter never is. The words are what the Editor shows minus the
/// heading's own markers — the leading `#`s and the space after them, any
/// closing `#`s, a setext underline — with inline markup left as written,
/// because that is what the page shows.
#[must_use]
pub fn of_blocks(document: &Document) -> Vec<Heading> {
    let text = document.text();
    document
        .blocks()
        .iter()
        .enumerate()
        .filter_map(|(block, found)| {
            let document::Kind::Heading { setext } = found.kind else {
                return None;
            };
            let (level, source) = if setext {
                setext_words(text, &found.at)
            } else {
                atx_words(text, &found.at)
            };
            Some(Heading {
                level,
                text: text[source.clone()].to_string(),
                block,
                source,
            })
        })
        .collect()
}

/// The heading whose section `caret` stands in, as an index into `headings`
/// from [`of_blocks`]: the last heading at or above the caret's block, or
/// none when the caret stands above the first heading.
///
/// By block rather than by byte, so a caret anywhere on a heading's own line
/// — on its `#`, before its words — is in that heading's section.
#[must_use]
pub fn section(document: &Document, headings: &[Heading], caret: usize) -> Option<usize> {
    let block = document.block_at(caret)?;
    headings.iter().rposition(|heading| heading.block <= block)
}

/// The level and the words of the ATX heading at `at`: after the `#`s and
/// the space, before a closing run of `#`s and the line's end.
fn atx_words(text: &str, at: &Range<usize>) -> (u8, Range<usize>) {
    let line = text[at.clone()].lines().next().unwrap_or("");
    let opened = line.len() - line.trim_start().len();
    let hashes = line[opened..].bytes().take_while(|b| *b == b'#').count();
    let level = u8::try_from(hashes.clamp(1, 6)).unwrap_or(1);
    let rest = &line[opened + hashes..];
    let lead = rest.len() - rest.trim_start().len();
    let mut body = rest.trim();
    // A closing run counts only after a space; `# C#` keeps its sharp, and a
    // line of nothing but `#`s says nothing.
    let kept = body.trim_end_matches('#');
    if kept.len() < body.len() {
        if kept.is_empty() {
            body = "";
        } else if let Some(before) = kept.strip_suffix([' ', '\t']) {
            body = before.trim_end();
        }
    }
    let start = at.start + opened + hashes + lead;
    (level, start..start + body.len())
}

/// The level and the words of the setext heading at `at`: every line above
/// the underline, `===` making a level 1 and `---` a level 2.
fn setext_words(text: &str, at: &Range<usize>) -> (u8, Range<usize>) {
    let block = &text[at.clone()];
    let mut lines: Vec<(usize, &str)> = Vec::new();
    let mut offset = 0;
    for line in block.split_inclusive('\n') {
        lines.push((offset, line.trim_end_matches(['\n', '\r'])));
        offset += line.len();
    }
    let underline = lines.pop().filter(|(_, line)| !line.trim().is_empty());
    let level = match underline.and_then(|(_, line)| line.trim().chars().next()) {
        Some('-') => 2,
        _ => 1,
    };
    let content: Vec<(usize, &str)> = lines
        .into_iter()
        .filter(|(_, line)| !line.trim().is_empty())
        .collect();
    let Some((first_at, first)) = content.first().copied() else {
        return (level, at.start..at.start);
    };
    let (last_at, last) = content.last().copied().unwrap_or((first_at, first));
    let start = first_at + (first.len() - first.trim_start().len());
    let end = last_at + last.trim_end().len();
    (level, at.start + start..at.start + end)
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

    /// `text` as a Document, for the block-index constructor.
    fn held(text: &str) -> Document {
        let mut document = Document::untitled();
        document.reload(text.to_string());
        document
    }

    /// The Outline of `text` as `(level, words, byte the words start at)`.
    fn outlined(text: &str) -> Vec<(u8, String, usize)> {
        of_blocks(&held(text))
            .into_iter()
            .map(|heading| (heading.level, heading.text, heading.source.start))
            .collect()
    }

    /// The shared passage, read off the block index: the two headings the
    /// Hand test names, each at the byte its words start at.
    #[test]
    fn the_sample_has_two_headings_where_their_words_start() {
        let document = Document::open(std::path::Path::new("../dev/ref/sample.md"))
            .expect("dev/ref/sample.md opens");
        let found = of_blocks(&document);
        let text = document.text();
        assert_eq!(
            found
                .iter()
                .map(|heading| (heading.level, heading.text.as_str()))
                .collect::<Vec<_>>(),
            vec![(1, "The Lighthouse"), (2, "What the sea keeps")]
        );
        assert_eq!(found[0].source, 2..16);
        assert_eq!(found[1].source, 573..591);
        for heading in &found {
            assert_eq!(&text[heading.source.clone()], heading.text);
        }
    }

    /// A setext pair is levels 1 and 2 with the underline outside the words.
    #[test]
    fn a_setext_heading_is_its_words_without_the_underline() {
        assert_eq!(
            outlined("Title\n=====\n\nSub\n---\n\nText.\n"),
            vec![(1, "Title".to_string(), 0), (2, "Sub".to_string(), 13)]
        );
    }

    /// The `#`s and the space after them go, so do closing `#`s, and inline
    /// markup inside the words stays as the page shows it.
    #[test]
    fn markers_are_stripped_and_inline_markup_is_kept() {
        assert_eq!(
            outlined("##   *Two* words ##\n\n### C#\n"),
            vec![(2, "*Two* words".to_string(), 5), (3, "C#".to_string(), 25)]
        );
    }

    /// A `#` line inside a fence and a heading inside a quote are not
    /// headings of the Document, and an empty Document has no Outline.
    #[test]
    fn what_is_not_a_heading_block_is_not_in_the_outline() {
        assert_eq!(
            outlined("```\n# not one\n```\n\n> # nor this\n\n# This\n"),
            vec![(1, "This".to_string(), 35)]
        );
        assert_eq!(outlined(""), vec![]);
        assert_eq!(outlined("A paragraph.\n"), vec![]);
    }

    /// The caret's section: none above the first heading, the heading whose
    /// own line the caret is on, the last heading inside the last section.
    #[test]
    fn the_carets_section_is_the_last_heading_at_or_above_it() {
        let document = held("Before.\n\n# One\n\nA line.\n\n## Two\n\nLast words.\n");
        let headings = of_blocks(&document);
        assert_eq!(section(&document, &headings, 3), None);
        assert_eq!(section(&document, &headings, 9), Some(0));
        assert_eq!(section(&document, &headings, 18), Some(0));
        assert_eq!(section(&document, &headings, 26), Some(1));
        assert_eq!(
            section(&document, &headings, document.text().len()),
            Some(1)
        );
    }
}
