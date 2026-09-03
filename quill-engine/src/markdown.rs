//! The Markdown parser, behind one shared set of options.
//!
//! Every part of Quill that reads Markdown — the Editor's keystroke path, the
//! Preview, Export, Stats, the outline — parses through this module, so a
//! Document is never interpreted two ways. It emits two streams: the token
//! stream the Markup Annotator marks up, and the *prose stream* the other three
//! Annotators consume, which is the `Text` events with Markup, code spans,
//! fenced code, URLs and front matter removed.

use std::ops::Range;

use pulldown_cmark::{Event, LinkType, OffsetIter, Options, Parser, Tag};

/// The one option set Quill reads Markdown with.
///
/// Tables, footnotes, strikethrough and task lists are the GitHub-flavoured
/// constructs a writer expects to work. Smart punctuation is **off**, and that
/// is the load-bearing one: it would rewrite `--` and `"` on the way past, so
/// the Preview would show characters the file does not contain and the Editor
/// would style bytes that are not there. The file's bytes are what the writer
/// sees.
#[must_use]
pub fn options() -> Options {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);
    // Front matter is not prose and not a thematic break. Without this a file
    // opening with `---` is read as a rule and a heading, so the Editor would
    // style a writer's metadata as text and Spell check would read it.
    options.insert(Options::ENABLE_YAML_STYLE_METADATA_BLOCKS);
    options
}

/// The events of `text`, each with the byte range it was parsed from.
///
/// The ranges are absolute UTF-8 byte offsets into `text`. Style from the
/// range and never from the event's own string: an [`Event::Text`] holds a
/// `CowStr` that pulldown-cmark rewrites wherever it unescapes a `\*` or
/// resolves an entity, so its length can differ from its range's. The range
/// still names the original bytes, which is what the Editor needs.
#[must_use]
pub fn events(text: &str) -> OffsetIter<'_> {
    Parser::new_ext(text, options()).into_offset_iter()
}

/// One stretch of the writer's words, and the bytes it was read from.
///
/// The text is the *source* slice rather than the event's own string, for the
/// reason [`events`] gives: the two differ wherever the parser resolved an
/// entity or dropped an escape, and an Annotator that reports a misspelling has
/// to name bytes the Document actually holds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Prose<'a> {
    /// Absolute UTF-8 bytes from the start of the source.
    pub at: Range<usize>,
    /// The source bytes themselves.
    pub text: &'a str,
}

/// The prose stream of `text`: the writer's words, with the Markup gone.
///
/// Syntax highlight, Style check and Spell check read this and never the
/// Document, so that none of them has to know what a `#` is. What is left out
/// is everything that is not the writer's prose: the delimiters (which are
/// never a `Text` event to begin with), code spans and fenced or indented code
/// (`Event::Code` and the text inside a code block), front matter, and URLs.
///
/// URLs need saying twice, because they arrive two ways. An inline link's
/// destination is part of its tag and never a `Text` event, so it falls out for
/// free; an **autolink**'s destination *is* its text, so it is suppressed here
/// by name. Without that, `<https://example.org>` would reach Spell check as a
/// word.
///
/// The words come out in fragments, split wherever a construct interrupted
/// them, because that is what the parser reports and joining them would invent
/// bytes that are not in the file. What to do at a fragment's edge is the
/// consuming Annotator's judgement, not this function's.
#[must_use]
pub fn prose(text: &str) -> Vec<Prose<'_>> {
    let mut runs = Vec::new();
    // One flag per open tag, because `Event::End` does not carry enough to
    // recognise an autolink again, and a `usize` alone could not be unwound.
    let mut open: Vec<bool> = Vec::new();
    let mut hidden = 0usize;
    for (event, at) in events(text) {
        match &event {
            Event::Start(tag) => {
                let hides = hides_prose(tag);
                open.push(hides);
                hidden += usize::from(hides);
            }
            Event::End(_) => {
                hidden -= usize::from(open.pop().unwrap_or(false));
            }
            Event::Text(_) if hidden == 0 => runs.push(Prose {
                text: &text[at.clone()],
                at,
            }),
            _ => {}
        }
    }
    runs
}

/// The destination `label` is defined as, if `text` defines it.
///
/// The one whole-document question about a link, and the reason it is asked
/// here rather than in the Annotator: a link-reference definition can stand
/// anywhere in the file, and the Markup Annotator reads a line at a time
/// ([`crate::annotate::Mark::DefinitionLabel`] says so in as many words). A
/// reference link's [`crate::annotate::Mark::Url`] is its *label* rather than
/// an address, so the app resolves it through this before it opens anything.
///
/// The lookup case-folds, which is what the CommonMark spec asks for and what
/// pulldown-cmark's own map does. The parser collects every definition as it
/// builds its tree, so this parses `text` once and reads the map it left.
#[must_use]
pub fn reference(text: &str, label: &str) -> Option<String> {
    let parser = Parser::new_ext(text, options());
    let defined = parser.reference_definitions().get(label)?;
    Some(defined.dest.to_string())
}

/// Whether the text inside `tag` is something other than the writer's prose.
fn hides_prose(tag: &Tag<'_>) -> bool {
    matches!(
        tag,
        Tag::CodeBlock(_)
            | Tag::MetadataBlock(_)
            | Tag::Link {
                link_type: LinkType::Autolink | LinkType::Email,
                ..
            }
    )
}

#[cfg(test)]
mod tests {
    use pulldown_cmark::Event;

    use super::*;

    #[test]
    fn the_option_set_is_the_gfm_constructs_and_front_matter_never_smart_punctuation() {
        let options = options();
        for (name, wanted) in [
            ("tables", Options::ENABLE_TABLES),
            ("footnotes", Options::ENABLE_FOOTNOTES),
            ("strikethrough", Options::ENABLE_STRIKETHROUGH),
            ("task lists", Options::ENABLE_TASKLISTS),
            // The fifth, and the one #37's list did not name: front matter is
            // not a thematic break and not prose, and every reader of this
            // module has to agree about that or the Editor would style a
            // writer's metadata as text and Preview would render it.
            ("front matter", Options::ENABLE_YAML_STYLE_METADATA_BLOCKS),
        ] {
            assert!(options.contains(wanted), "{name} is not enabled");
        }
        assert!(
            !options.contains(Options::ENABLE_SMART_PUNCTUATION),
            "smart punctuation would rewrite the writer's own bytes"
        );
    }

    #[test]
    fn smart_punctuation_being_off_leaves_the_writers_quotes_and_dashes_alone() {
        let source = "She said \"no\" -- twice.\n";
        let text: String = events(source)
            .filter(|(event, _)| matches!(event, Event::Text(_)))
            .map(|(event, _)| match event {
                Event::Text(run) => run.to_string(),
                _ => unreachable!(),
            })
            .collect();
        assert!(
            text.contains("\"no\"") && text.contains("--"),
            "the parser rewrote the source's punctuation: {text}"
        );
    }

    #[test]
    fn every_range_is_an_absolute_byte_offset_into_the_source() {
        let source = "# The Lighthouse\n\nThe lamp had been lit.\n";
        for (event, at) in events(source) {
            assert!(
                at.end <= source.len(),
                "{event:?} names bytes past the end of the source"
            );
            assert!(
                source.is_char_boundary(at.start) && source.is_char_boundary(at.end),
                "{event:?} names a range that splits a character"
            );
        }
    }

    #[test]
    fn an_entitys_range_is_longer_than_the_character_the_event_resolved_it_to() {
        let source = "Tom &amp; Jerry\n";
        let (event, at) = events(source)
            .find(|(event, _)| matches!(event, Event::Text(run) if run.as_ref() == "&"))
            .expect("the entity resolves to a text event");
        let Event::Text(run) = &event else {
            unreachable!()
        };
        assert_eq!(
            &source[at.clone()],
            "&amp;",
            "the range names the source bytes"
        );
        assert!(
            run.len() < at.len(),
            "this is the case an Annotator must never measure by the event's own length: \
             {run:?} is {} bytes against a range of {}",
            run.len(),
            at.len()
        );
    }

    /// The passage the Markup Piece is judged on.
    fn oracle() -> String {
        std::fs::read_to_string("../shots/oracle/markup.md")
            .expect("the judged Markup passage is in the repo")
    }

    /// The prose stream of `source` as the words alone, in order.
    fn words(source: &str) -> Vec<&str> {
        prose(source).into_iter().map(|run| run.text).collect()
    }

    #[test]
    fn the_prose_stream_of_the_oracles_passage_is_the_writers_words_and_nothing_else() {
        let source = oracle();
        assert_eq!(
            words(&source),
            [
                "The Lighthouse",
                "The lamp had been lit for an hour before she noticed the boat. It was a ",
                "small thing",
                ", a dark stitch on the water, and it was ",
                "not moving",
                " the way boats move when someone is rowing them.",
                "What the sea keeps",
                "There are things the sea gives back and things it keeps, and no one has ever \
                 found the rule that decides between them.",
                "the rope, coiled and salt-stiff",
                "the good knife",
                "two oranges, for luck",
                "Untie the skiff.",
                "Push off before the tide turns.",
                "She wrote the bearing on her sleeve, ",
                ", and the log entry followed:",
                "The rest is in ",
                "the keeper's book",
                ", and the boat that was not moving grew larger in the dark.",
            ]
        );
    }

    #[test]
    fn no_markup_byte_survives_into_the_prose_stream() {
        let source = oracle();
        let words: String = words(&source).concat();
        // A hyphen is not in the list: `salt-stiff` is the writer's word, and
        // the bullet that opens its line never reaches a `Text` event at all.
        for markup in ['#', '*', '`', '>', '[', ']', '(', ')'] {
            assert!(
                !words.contains(markup),
                "the prose stream carries a {markup:?}, which an Annotator must never see"
            );
        }
        assert!(
            !words.contains("1."),
            "the prose stream carries an ordered marker, which is structure and not a word"
        );
        assert!(
            !words.contains("https"),
            "the prose stream carries a URL, which Spell check would call a misspelling"
        );
        assert!(
            !words.contains("21:40"),
            "the prose stream carries the fenced block, which is not prose"
        );
    }

    #[test]
    fn every_prose_range_names_the_source_bytes_it_was_read_from() {
        let source = oracle();
        for run in prose(&source) {
            assert_eq!(
                &source[run.at.clone()],
                run.text,
                "a prose range must name its own source bytes"
            );
        }
    }

    #[test]
    fn front_matter_is_not_prose() {
        let source = "---\ntitle: The Lighthouse\n---\n\nThe lamp was lit.\n";
        assert_eq!(words(source), ["The lamp was lit."]);
    }

    #[test]
    fn an_autolinks_url_is_not_prose_even_though_it_is_the_links_text() {
        let source = "Written up at <https://example.org/keepers-book> in full.\n";
        assert_eq!(words(source), ["Written up at ", " in full."]);
    }

    #[test]
    fn an_escapes_backslash_is_left_for_the_subtraction_to_find() {
        let source = "A \\* star\n";
        let (_, at) = events(source)
            .find(|(event, _)| matches!(event, Event::Text(run) if run.starts_with('*')))
            .expect("the escaped star is a text event");
        assert_eq!(
            &source[at.clone()],
            "* star",
            "the content range must start at the star, not at the backslash"
        );
        assert_eq!(
            &source[at.start - 1..at.start],
            "\\",
            "the backslash is uncovered, which is how subtraction marks it as Markup"
        );
    }

    #[test]
    fn a_reference_label_resolves_to_the_definitions_destination_whatever_its_case() {
        let source = "See [the book][Ref] for it.\n\n[ref]: https://example.org/book\n";
        assert_eq!(
            reference(source, "Ref").as_deref(),
            Some("https://example.org/book"),
            "the label is written one way and defined another, and CommonMark \
             folds the case of both"
        );
        assert_eq!(
            reference(source, "missing"),
            None,
            "a label nothing defines resolves to nothing, and the caller opens \
             nothing"
        );
    }
}
