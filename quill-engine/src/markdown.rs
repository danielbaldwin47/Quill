//! The Markdown parser, behind one shared set of options.
//!
//! Every part of Quill that reads Markdown — the Editor's keystroke path, the
//! Preview, Export, Stats, the outline — parses through this module, so a
//! Document is never interpreted two ways. It emits two streams: the token
//! stream the Markup Annotator marks up, and the *prose stream* the other three
//! Annotators consume, which is the `Text` events with Markup, code spans,
//! fenced code, URLs and front matter removed.

use pulldown_cmark::{Event, OffsetIter, Options, Parser};

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

/// Whether `event` covers source bytes that are content rather than markup.
///
/// The parsers report a span for a *construct*, which covers its delimiters,
/// and separate spans for the content inside it; no parser reports the
/// delimiters on their own. So Quill derives them by subtraction, and this is
/// the predicate the subtraction is done against: text, code and raw HTML are
/// the three events whose bytes the writer typed as themselves.
pub(crate) fn is_content(event: &Event<'_>) -> bool {
    matches!(
        event,
        Event::Text(_) | Event::Code(_) | Event::InlineHtml(_) | Event::Html(_)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_option_set_is_the_four_gfm_constructs_and_never_smart_punctuation() {
        let options = options();
        for (name, wanted) in [
            ("tables", Options::ENABLE_TABLES),
            ("footnotes", Options::ENABLE_FOOTNOTES),
            ("strikethrough", Options::ENABLE_STRIKETHROUGH),
            ("task lists", Options::ENABLE_TASKLISTS),
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
}
