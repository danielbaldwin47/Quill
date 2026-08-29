//! Focus: which bytes are bright, which are near, and which are dim.
//!
//! Focus dims everything the writer is not in. There are three tiers, not two:
//! the **bright** tier is the sentence — or the paragraph — the caret is in, the
//! **near** tier is the sentence either side of it, and everything else is
//! **dim**. The near tier is the refinement [ADR 0006] keeps: iA drops the
//! neighbouring sentences to the same grey as text three paragraphs away, and
//! the one thing writers complain about is losing the sentence they were
//! building on.
//!
//! This module is the whole of Focus that can be computed without a window
//! (#112). From a Document, where the caret is and how much to light, it hands
//! back byte ranges; the tickets after it paint them — the flattening takes a
//! tier (#126), the Editor retags (#113), the change cross-fades (#114).
//!
//! The rules are the Parity oracle's, `legacy/app/js/focus.js` lines 20-38 and
//! 110-165, ported case for case: [`sentences`] is its `TERM` and `isRealBreak`,
//! and [`tiers`] is its `update`. Two things read differently here, both because
//! the native side has a real block index (`docs/architecture.md` § Text model)
//! where the oracle had a regex over lines:
//!
//! - **A paragraph is a block** from that index. The oracle's `paragraphBounds`
//!   starts a fresh paragraph at every list item and every quote line, so a list
//!   is one paragraph per item there and one paragraph here. #113 judges the
//!   shots that would show it.
//! - **The near tier stops at the block's edges** — `focus.js:147-160` searches
//!   inside `paragraphBounds` only, and this searches inside the block's lines.
//!   The parent spec (#40) said otherwise; the oracle is right and this is the
//!   correction.
//!
//! Focus is on the keystroke path (#41), so the cost has to be flat in the size
//! of the Document. [`tiers`] reads the caret's block and, for the blank-line
//! rule, the two lines above it — [`window`] is that range, and nothing outside
//! it is looked at.

use std::collections::BTreeMap;
use std::ops::Range;

use crate::document::Document;
use crate::settings::FocusScope;

/// How much Focus leaves lit.
///
/// The two settings ADR 0006 keeps independent — Focus on or off, and its scope
/// — arrive here as one value, because there is nothing for a scope to mean
/// while Focus is off. [`Focus::Off`] yields no tiers at all, so the caller's
/// off path is its ordinary path with nothing to draw.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Focus {
    /// Focus is off: every byte is drawn as it would be without it.
    #[default]
    Off,
    /// Focus is on, at this scope.
    On(FocusScope),
}

/// The bytes Focus lights, in the two tiers above dim.
///
/// Everything the two lists leave out is dim, which is why there is no third
/// list: dim is the page, and these are the holes cut in it.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Tiers {
    /// The bright ranges. Absolute UTF-8 bytes from the start of the Document.
    pub bright: Vec<Range<usize>>,
    /// The near ranges. Absolute UTF-8 bytes from the start of the Document.
    pub near: Vec<Range<usize>>,
}

/// One line's share of the [`Tiers`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LineTiers {
    /// The line, counted from 0.
    pub line: usize,
    /// The bytes of that line in each tier, clipped to it.
    pub tiers: Tiers,
}

/// The tiers for a caret — or a selection — at `at`, under `focus`.
///
/// `at` is the writer's selection as the buffer holds it: an empty range is a
/// caret, and a range with room in it is a live selection, which is the bright
/// span on its own (`focus.js:128-134`). While you are marking a phrase, the
/// phrase is what you are working on.
///
/// A caret on a blank line lights nothing and keeps the last sentence of the
/// block above it near (`focus.js:135-141`), which is what the writer sees on
/// every Enter with Focus on: the page does not black out behind a new thought.
#[must_use]
pub fn tiers(doc: &Document, at: &Range<usize>, focus: Focus) -> Tiers {
    let Focus::On(scope) = focus else {
        return Tiers::default();
    };
    let text = doc.text();
    let caret = at.start.min(text.len());
    let Some(block) = doc.block_at(caret).map(|at| doc.blocks()[at].at.clone()) else {
        return Tiers::default();
    };
    let read = window(doc, at, focus);
    let body = trimmed(
        text,
        &(block.start.max(read.start)..block.end.min(read.end)),
    );

    if scope == FocusScope::Paragraph {
        let bright = if body.is_empty() {
            Vec::new()
        } else {
            only(body)
        };
        return Tiers {
            bright,
            near: Vec::new(),
        };
    }
    if at.end > at.start {
        return Tiers {
            bright: only(caret..at.end.min(text.len())),
            near: Vec::new(),
        };
    }

    let line = doc.place(caret).line;
    let here = content(doc, line);
    if text[here.clone()].trim().is_empty() {
        return Tiers {
            bright: Vec::new(),
            near: behind(doc, line),
        };
    }

    let spans = sentences(&text[here.clone()]);
    let index = caret.saturating_sub(here.start).min(here.end - here.start);
    let this = sentence_at(&spans, index);
    let bright = only(here.start + spans[this].start..here.start + spans[this].end);

    // The near tier never leaves the block: the sentence before the first one of
    // a paragraph is in another thought, not a neighbouring one.
    let first = doc.place(body.start).line;
    let last = doc.place(last_byte(&body)).line;
    let mut near = Vec::new();
    if this > 0 {
        near.push(here.start + spans[this - 1].start..here.start + spans[this - 1].end);
    } else {
        for above in (first..line).rev() {
            let at = content(doc, above);
            if at.is_empty() {
                continue;
            }
            let begins = sentences(&text[at.clone()])
                .last()
                .map_or(0, |span| span.start);
            near.push(at.start + begins..at.end);
            break;
        }
    }
    if this + 1 < spans.len() {
        near.push(here.start + spans[this + 1].start..here.start + spans[this + 1].end);
    } else {
        for below in (line + 1)..=last {
            let at = content(doc, below);
            if at.is_empty() {
                continue;
            }
            let ends = sentences(&text[at.clone()])
                .first()
                .map_or(0, |span| span.end);
            near.push(at.start..at.start + ends);
            break;
        }
    }
    Tiers { bright, near }
}

/// [`Tiers`] split at the Document's line starts, ascending by line.
///
/// This is the shape the flattening consumes: it works a line at a time, and a
/// run that straddled a line start would have to be cut there anyway. Line
/// terminators are left out — an unlit newline is one the writer cannot see.
///
/// Lines with nothing on them are left out too. With Focus on they are wholly
/// dim, and with Focus off there are no tiers at all, so the caller's off path
/// and its all-dim path are one path with nothing to do.
#[must_use]
pub fn tiers_by_line(doc: &Document, tiers: &Tiers) -> Vec<LineTiers> {
    let mut lines: BTreeMap<usize, Tiers> = BTreeMap::new();
    for (ranges, bright) in [(&tiers.bright, true), (&tiers.near, false)] {
        for at in ranges {
            for line in doc.place(at.start).line..=doc.place(last_byte(at)).line {
                let on = content(doc, line);
                let clipped = at.start.max(on.start)..at.end.min(on.end);
                if clipped.start >= clipped.end {
                    continue;
                }
                let tiers = lines.entry(line).or_default();
                if bright {
                    tiers.bright.push(clipped);
                } else {
                    tiers.near.push(clipped);
                }
            }
        }
    }
    lines
        .into_iter()
        .map(|(line, tiers)| LineTiers { line, tiers })
        .collect()
}

/// The bytes [`tiers`] reads to answer for `at`, and the only ones it looks at.
///
/// The caret's block, widened to whole lines, and — when the caret is on a blank
/// line, where the rule reaches behind it — the two lines above. Two blocks at
/// the worst, whatever the Document weighs, which is what keeps Focus off the
/// keystroke budget (#41). A selection is its own window: the writer asked for
/// those bytes to be lit, so they are what there is to read.
fn window(doc: &Document, at: &Range<usize>, focus: Focus) -> Range<usize> {
    let Focus::On(scope) = focus else {
        return 0..0;
    };
    let text = doc.text();
    let caret = at.start.min(text.len());
    let Some(block) = doc.block_at(caret).map(|at| doc.blocks()[at].at.clone()) else {
        return 0..0;
    };
    if scope == FocusScope::Sentence && at.end > at.start {
        return whole_lines(doc, &(caret..at.end.min(text.len())));
    }
    let mut read = whole_lines(doc, &block);
    if scope == FocusScope::Sentence {
        let line = doc.place(caret).line;
        if text[content(doc, line)].trim().is_empty() {
            // What [`behind`] reads, and no more: the line above, and the line
            // above that one only when the first was blank too.
            let above = content(doc, line.saturating_sub(1));
            let reach = if text[above.clone()].trim().is_empty() {
                content(doc, line.saturating_sub(2)).start
            } else {
                above.start
            };
            read.start = read.start.min(reach);
        }
    }
    read
}

/// The last sentence of the block above a caret parked on a blank line.
///
/// The oracle steps over one blank line and no more (`focus.js:136-140`): two
/// blank lines behind you and the thought they separate is far enough back to
/// go dim with everything else.
fn behind(doc: &Document, line: usize) -> Vec<Range<usize>> {
    let text = doc.text();
    for step in 1..=2 {
        let Some(above) = line.checked_sub(step) else {
            break;
        };
        let at = content(doc, above);
        if text[at.clone()].trim().is_empty() {
            continue;
        }
        let start = sentences(&text[at.clone()])
            .last()
            .map_or(0, |span| span.start);
        return only(at.start + start..at.end);
    }
    Vec::new()
}

/// The sentence spans of one line, covering it end to end.
///
/// `focus.js`'s `sentences`: a terminator run, then any closing quotes or
/// brackets, then whitespace or the end of the line. The spans tile, so the
/// caret is always in one of them and typing between two sentences never falls
/// through a gap.
fn sentences(text: &str) -> Vec<Range<usize>> {
    /// The characters that can end a sentence.
    const TERMINATORS: [char; 4] = ['.', '!', '?', '…'];
    /// The closing quotes and brackets that belong to the sentence they follow.
    const CLOSERS: [char; 7] = ['"', '\'', '”', '’', '»', ')', ']'];

    let mut spans = Vec::new();
    let mut start = 0;
    let mut scan = 0;
    while let Some(here) = text[scan..].chars().next() {
        if !TERMINATORS.contains(&here) {
            scan += here.len_utf8();
            continue;
        }
        let stop = run(text, scan, &TERMINATORS);
        let before = run(text, stop, &CLOSERS);
        let after = before + text[before..].len() - text[before..].trim_start().len();
        if after == before && after < text.len() {
            // The punctuation is inside a word — "3.5", or a URL — not after one.
            scan = stop;
            continue;
        }
        // A terminator with nothing after it needs no break: the final span
        // already runs to the end of the line.
        if after >= text.len() {
            break;
        }
        if is_real_break(text, before, after) {
            spans.push(start..after);
            start = after;
        }
        scan = after;
    }
    spans.push(start..text.len());
    spans
}

/// The end of the run of `of` characters that starts at `from`.
fn run(text: &str, from: usize, of: &[char]) -> usize {
    let mut end = from;
    while let Some(c) = text[end..].chars().next() {
        if !of.contains(&c) {
            break;
        }
        end += c.len_utf8();
    }
    end
}

/// Whether the punctuation ending at `before`, with the next word at `after`,
/// really ends a sentence. `focus.js`'s `isRealBreak`.
fn is_real_break(text: &str, before: usize, after: usize) -> bool {
    let head = &text[..before];
    if is_abbreviation(head) || is_initial(head) || is_ordinal(head) {
        return false;
    }
    // A sentence does not restart on a lowercase letter: "at 5 p.m. and then
    // she left" is one sentence, whatever the full stops say.
    match text[after..].chars().next() {
        None => true,
        Some(c) => c.is_whitespace() || !c.is_ascii_lowercase(),
    }
}

/// Whether `head` ends in a word that ends in a period without ending a
/// sentence — `focus.js`'s `ABBR`.
fn is_abbreviation(head: &str) -> bool {
    /// The words that end in a period and do not end a sentence. May is a word
    /// as well as a month, so the months are here without it.
    const ABBREVIATIONS: &[&str] = &[
        "mr", "mrs", "ms", "dr", "prof", "rev", "sr", "jr", "st", "vs", "etc", "eg", "e.g", "ie",
        "i.e", "cf", "al", "fig", "no", "nos", "vol", "ch", "pp", "approx", "dept", "est", "inc",
        "ltd", "co", "univ", "apt", "min", "max", "jan", "feb", "mar", "apr", "jun", "jul", "aug",
        "sep", "sept", "oct", "nov", "dec", "mon", "tue", "wed", "thu", "fri", "sat", "sun",
    ];

    let Some(head) = head.strip_suffix('.') else {
        return false;
    };
    ABBREVIATIONS.iter().any(|abbreviation| {
        let Some(at) = head.len().checked_sub(abbreviation.len()) else {
            return false;
        };
        head.is_char_boundary(at)
            && head[at..].eq_ignore_ascii_case(abbreviation)
            && opens_a_word(&head[..at])
    })
}

/// Whether `head` ends in a lone initial — the `J.`, `R.` and `R.` of
/// "J. R. R. Tolkien". `focus.js`'s `INITIAL`.
fn is_initial(head: &str) -> bool {
    let Some(head) = head.strip_suffix('.') else {
        return false;
    };
    match head.chars().next_back() {
        Some(letter) if letter.is_ascii_alphabetic() => {
            opens_a_word(&head[..head.len() - letter.len_utf8()])
        }
        _ => false,
    }
}

/// Whether `head` is a list's or a numbered heading's marker — the `1.` of
/// "# 1. Down the Rabbit Hole". `focus.js`'s `ORDINAL`.
fn is_ordinal(head: &str) -> bool {
    let Some(head) = head.strip_suffix('.') else {
        return false;
    };
    let before = head.trim_end_matches(|c: char| c.is_ascii_digit());
    before.len() < head.len()
        && before
            .chars()
            .all(|c| c.is_whitespace() || matches!(c, '#' | '>' | '-' | '*' | '+'))
}

/// Whether a word starting where `head` ends starts a word rather than
/// continuing one — `focus.js`'s `(?:^|[\s("'“‘[])`.
fn opens_a_word(head: &str) -> bool {
    /// The opening quotes and brackets a word may start against.
    const OPENERS: [char; 6] = ['(', '"', '\'', '“', '‘', '['];

    match head.chars().next_back() {
        None => true,
        Some(c) => c.is_whitespace() || OPENERS.contains(&c),
    }
}

/// The span `index` bytes into a line is in.
///
/// A caret on a sentence's last byte belongs to that sentence, and a caret on
/// the boundary belongs to the one it is about to type into.
fn sentence_at(spans: &[Range<usize>], index: usize) -> usize {
    spans
        .iter()
        .position(|span| index < span.end)
        .unwrap_or(spans.len() - 1)
}

/// The bytes of `line` without its line terminator.
fn content(doc: &Document, line: usize) -> Range<usize> {
    let at = doc.line_bytes(line);
    let end = at.start + doc.text()[at.clone()].trim_end_matches(['\n', '\r']).len();
    at.start..end
}

/// `at` widened to the whole lines it touches, their terminators left out.
fn whole_lines(doc: &Document, at: &Range<usize>) -> Range<usize> {
    let start = doc.line_bytes(doc.place(at.start).line).start;
    let end = content(doc, doc.place(last_byte(at)).line).end;
    start..end.max(start)
}

/// `at` without the blank at its end: a block's range runs to the newline that
/// closes it, and there is nothing to light in a newline.
fn trimmed(text: &str, at: &Range<usize>) -> Range<usize> {
    if at.start >= at.end {
        return at.start..at.start;
    }
    at.start..at.start + text[at.clone()].trim_end().len()
}

/// One range as the list a tier holds.
///
/// `vec![a..b]` reads as an attempt to build a `Vec` out of a range rather than
/// one holding a range, and clippy is right to ask which was meant.
fn only(at: Range<usize>) -> Vec<Range<usize>> {
    std::iter::once(at).collect()
}

/// The last byte `at` covers, or its start when it covers none.
fn last_byte(at: &Range<usize>) -> usize {
    at.end.saturating_sub(1).max(at.start)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::Kind;
    use std::path::Path;

    /// A Document holding `text`, the way an owner's file arrives.
    fn document(text: &str) -> Document {
        let mut doc = Document::untitled();
        doc.insert(0, text);
        doc
    }

    /// The sentences of one line as the text they cover, which is what a writer
    /// would see the dim move across.
    fn spans(text: &str) -> Vec<&str> {
        sentences(text)
            .into_iter()
            .map(|at| &text[at])
            .collect::<Vec<_>>()
    }

    /// What a Document shows under `focus`, as `(bright, near)` source text.
    fn lit(doc: &Document, at: Range<usize>, focus: Focus) -> (Vec<&str>, Vec<&str>) {
        let tiers = tiers(doc, &at, focus);
        let text = doc.text();
        (
            tiers.bright.iter().map(|at| &text[at.clone()]).collect(),
            tiers.near.iter().map(|at| &text[at.clone()]).collect(),
        )
    }

    /// The shared test passage, which the judged states are shot against.
    fn passage() -> Document {
        Document::open(Path::new("../ref/sample.md"))
            .expect("the shared test passage is in the repo")
    }

    // Segmentation: the oracle's cases.

    #[test]
    fn an_abbreviation_does_not_end_a_sentence() {
        assert_eq!(
            spans("Dr. Ross came home. He was late."),
            ["Dr. Ross came home. ", "He was late."],
            "a title ending in a period is a word, not a full stop"
        );
        assert_eq!(
            spans("It rained, etc. Then it stopped."),
            ["It rained, etc. Then it stopped."],
            "the oracle's list holds `etc`, so a period after it never breaks, \
             even here where a writer did end the sentence"
        );
        assert_eq!(
            spans("Ready, e.g. Tuesday. Or not."),
            ["Ready, e.g. Tuesday. ", "Or not."],
            "an abbreviation with a period inside it is one entry, not two"
        );
    }

    #[test]
    fn an_initial_does_not_end_a_sentence() {
        assert_eq!(
            spans("J. R. R. Tolkien wrote it. Then he stopped."),
            ["J. R. R. Tolkien wrote it. ", "Then he stopped."],
            "three initials in a row are three words of one name"
        );
    }

    #[test]
    fn an_ordinal_at_a_lists_start_does_not_end_a_sentence() {
        assert_eq!(
            spans("1. Down the Rabbit Hole"),
            ["1. Down the Rabbit Hole"],
            "a list marker is a marker, not a sentence of its own"
        );
        assert_eq!(
            spans("# 1. Down the Rabbit Hole"),
            ["# 1. Down the Rabbit Hole"]
        );
        assert_eq!(
            spans("Chapter 1. Down the Rabbit Hole"),
            ["Chapter 1. ", "Down the Rabbit Hole"],
            "the marker rule is anchored at the line's start, so this is a real break"
        );
    }

    #[test]
    fn a_lowercase_word_does_not_start_a_sentence() {
        assert_eq!(
            spans("They left at 5 p.m. and then it was quiet."),
            ["They left at 5 p.m. and then it was quiet."],
            "a sentence does not restart on a lowercase letter"
        );
    }

    #[test]
    fn an_ellipsis_and_a_doubled_terminator_end_a_sentence() {
        assert_eq!(
            spans("She waited… Nothing came."),
            ["She waited… ", "Nothing came."]
        );
        assert_eq!(spans("Really?! He left."), ["Really?! ", "He left."]);
    }

    #[test]
    fn a_closing_quote_or_bracket_belongs_to_the_sentence_it_ends() {
        assert_eq!(
            spans("He said \"Stop.\" Then he left."),
            ["He said \"Stop.\" ", "Then he left."],
            "the quote closes the sentence, so it is inside it"
        );
        assert_eq!(
            spans("Er sagte «Halt.» Dann ging er."),
            ["Er sagte «Halt.» ", "Dann ging er."],
            "a guillemet closes it too"
        );
        assert_eq!(
            spans("It ended (at last.) And then it did not."),
            ["It ended (at last.) ", "And then it did not."]
        );
    }

    #[test]
    fn a_final_run_without_punctuation_is_a_sentence() {
        assert_eq!(
            spans("It ended. A thought without an end"),
            ["It ended. ", "A thought without an end"],
            "the writer is mid-sentence and it is still the sentence they are in"
        );
        assert_eq!(spans(""), [""], "an empty line is one empty sentence");
    }

    #[test]
    fn punctuation_inside_a_word_is_not_a_terminator() {
        assert_eq!(
            spans("It cost 3.50 and weighed 2.5kg. Then it broke."),
            ["It cost 3.50 and weighed 2.5kg. ", "Then it broke."],
            "a decimal point has no space after it, so it never ends anything"
        );
    }

    // The tiers.

    #[test]
    fn focus_off_lights_nothing_at_all() {
        let doc = passage();
        assert_eq!(
            tiers(&doc, &(403..403), Focus::Off),
            Tiers::default(),
            "with Focus off there is nothing to dim, so there is nothing to light"
        );
    }

    #[test]
    fn the_judged_caret_lights_its_sentence_and_the_one_after_it() {
        let doc = passage();
        let (bright, near) = lit(&doc, 403..403, Focus::On(FocusScope::Sentence));
        assert_eq!(
            bright,
            [
                "She went down the spiral stair, counting the steps the way she always did, and found the coat on its hook and the lantern beside it. "
            ],
            "the caret is in the paragraph's first sentence"
        );
        assert_eq!(
            near,
            ["The door took both hands to open. "],
            "the sentence before it is in another paragraph, so only the one after is near"
        );
        assert_eq!(
            tiers(&doc, &(403..403), Focus::On(FocusScope::Sentence)),
            Tiers {
                bright: only(353..486),
                near: only(486..520),
            }
        );
    }

    #[test]
    fn the_judged_caret_lights_its_whole_paragraph_with_nothing_near() {
        let doc = passage();
        let (bright, near) = lit(&doc, 403..403, Focus::On(FocusScope::Paragraph));
        assert_eq!(bright.len(), 1, "one paragraph is one bright range");
        assert!(
            bright[0].starts_with("She went down the spiral stair,")
                && bright[0].ends_with("the sound of a long vowel."),
            "the whole paragraph is lit, end to end: {:?}",
            bright[0]
        );
        assert!(
            near.is_empty(),
            "Paragraph scope has no near tier, so the two scopes feel distinct"
        );
        assert_eq!(
            tiers(&doc, &(403..403), Focus::On(FocusScope::Paragraph)),
            Tiers {
                bright: only(353..568),
                near: Vec::new(),
            }
        );
    }

    #[test]
    fn the_near_tier_stops_at_the_blocks_edges() {
        let doc = document("First thought here.\n\nSecond thought here.\n\nThird thought here.\n");
        let (bright, near) = lit(&doc, 25..25, Focus::On(FocusScope::Sentence));
        assert_eq!(bright, ["Second thought here."]);
        assert!(
            near.is_empty(),
            "the sentences either side are in other paragraphs, which are dim: {near:?}"
        );
    }

    #[test]
    fn the_near_tier_reaches_the_line_above_inside_one_block() {
        let doc = document("A first line here.\nA second line here.\n\nAnother block.\n");
        let (bright, near) = lit(&doc, 25..25, Focus::On(FocusScope::Sentence));
        assert_eq!(bright, ["A second line here."]);
        assert_eq!(
            near,
            ["A first line here."],
            "the two lines are one paragraph, so the line above is a neighbouring sentence"
        );
    }

    #[test]
    fn a_live_selection_is_the_bright_span() {
        let doc = passage();
        let (bright, near) = lit(&doc, 353..372, Focus::On(FocusScope::Sentence));
        assert_eq!(
            bright,
            ["She went down the s"],
            "while you are marking a phrase, the phrase is what you are working on"
        );
        assert!(near.is_empty(), "a selection has no neighbours: {near:?}");
    }

    #[test]
    fn a_caret_on_a_blank_line_lights_nothing_and_keeps_the_sentence_above_near() {
        let doc = document("One thought. And then another one.\n\nNext.\n");
        let (bright, near) = lit(&doc, 35..35, Focus::On(FocusScope::Sentence));
        assert!(
            bright.is_empty(),
            "nothing is active on a blank line: {bright:?}"
        );
        assert_eq!(
            near,
            ["And then another one."],
            "the page does not black out behind a new thought"
        );
    }

    #[test]
    fn a_caret_two_blank_lines_below_a_thought_leaves_it_dim() {
        let doc = document("One thought here.\n\n\n\nFar below.\n");
        let (bright, near) = lit(&doc, 20..20, Focus::On(FocusScope::Sentence));
        assert!(bright.is_empty(), "{bright:?}");
        assert!(
            near.is_empty(),
            "two blank lines back is far enough to go dim with everything else: {near:?}"
        );
    }

    // Splitting at the line starts.

    #[test]
    fn tiers_by_line_never_yields_a_range_that_crosses_a_line_start() {
        let doc = document("A first line here.\nA second line here.\nA third line here.\n");
        let tiers = tiers(&doc, &(25..25), Focus::On(FocusScope::Paragraph));
        assert_eq!(
            tiers.bright.len(),
            1,
            "the paragraph is one range before it is split"
        );
        let text = doc.text();
        let by_line = tiers_by_line(&doc, &tiers);
        assert_eq!(
            by_line
                .iter()
                .map(|on| (on.line, &text[on.tiers.bright[0].clone()]))
                .collect::<Vec<_>>(),
            [
                (0, "A first line here."),
                (1, "A second line here."),
                (2, "A third line here.")
            ],
            "one range per line, and no newline in any of them"
        );
        for on in &by_line {
            let line = content(&doc, on.line);
            for at in on.tiers.bright.iter().chain(&on.tiers.near) {
                assert!(
                    at.start >= line.start && at.end <= line.end,
                    "line {} has {at:?}, which leaves {line:?}",
                    on.line
                );
            }
        }
    }

    #[test]
    fn tiers_by_line_keeps_both_tiers_of_a_line_apart() {
        let doc = document("One thought. And then another one. And a third.\n");
        let tiers = tiers(&doc, &(15..15), Focus::On(FocusScope::Sentence));
        let text = doc.text();
        let by_line = tiers_by_line(&doc, &tiers);
        assert_eq!(by_line.len(), 1, "it is all one line");
        assert_eq!(
            by_line[0]
                .tiers
                .bright
                .iter()
                .map(|at| &text[at.clone()])
                .collect::<Vec<_>>(),
            ["And then another one. "]
        );
        assert_eq!(
            by_line[0]
                .tiers
                .near
                .iter()
                .map(|at| &text[at.clone()])
                .collect::<Vec<_>>(),
            ["One thought. ", "And a third."],
            "the sentence either side of the caret's is near"
        );
    }

    #[test]
    fn a_line_with_nothing_on_it_is_left_out_rather_than_named_dim() {
        let doc = document("First thought here.\n\nSecond thought here.\n");
        let tiers = tiers(&doc, &(25..25), Focus::On(FocusScope::Sentence));
        assert_eq!(
            tiers_by_line(&doc, &tiers)
                .iter()
                .map(|on| on.line)
                .collect::<Vec<_>>(),
            [2],
            "only the caret's line carries anything; the rest of the page is dim"
        );
    }

    // The keystroke path.

    #[test]
    fn a_caret_in_a_ten_thousand_word_document_reads_no_more_than_two_blocks() {
        let paragraph = "The lamp had been lit for an hour before she noticed the boat. \
             It was a small thing, a dark stitch on the water, and it was not moving. \
             She watched it for a long time.";
        let mut text = String::new();
        for _ in 0..300 {
            text.push_str(paragraph);
            text.push_str("\n\n");
        }
        let mut doc = Document::untitled();
        doc.insert(0, &text);
        assert!(
            doc.text().len() > 50_000,
            "the passage is a manuscript, not a note: {} bytes",
            doc.text().len()
        );

        let focus = Focus::On(FocusScope::Sentence);
        let mut widest = 0;
        for caret in (0..doc.text().len()).step_by(97) {
            let read = window(&doc, &(caret..caret), focus);
            let block = doc.block_at(caret).expect("the block index tiles the text");
            let here = doc.blocks()[block].at.clone();
            // The block above is the nearest one with prose in it: the blank
            // lines between two paragraphs are Gap entries of their own, and
            // reaching over them is still reaching one block back.
            let above = doc.blocks()[..block]
                .iter()
                .rev()
                .find(|block| block.kind != Kind::Gap)
                .map_or(here.start, |block| block.at.start);
            assert!(
                read.start >= above && read.end <= here.end,
                "a caret at {caret} read {read:?}, which leaves the block {here:?} and the one before it"
            );
            let tiers = tiers(&doc, &(caret..caret), focus);
            for at in tiers.bright.iter().chain(&tiers.near) {
                assert!(
                    at.start >= read.start && at.end <= read.end,
                    "a caret at {caret} lit {at:?}, which it never read"
                );
            }
            widest = widest.max(read.end - read.start);
        }
        assert!(
            widest < 2_000,
            "the widest window over a {} byte Document was {widest} bytes, which is not flat",
            doc.text().len()
        );
    }

    #[test]
    fn every_caret_in_the_shared_passage_lights_something_well_formed() {
        let doc = passage();
        let text = doc.text();
        for scope in [FocusScope::Sentence, FocusScope::Paragraph] {
            for caret in 0..=text.len() {
                if !text.is_char_boundary(caret) {
                    continue;
                }
                let tiers = tiers(&doc, &(caret..caret), Focus::On(scope));
                for at in tiers.bright.iter().chain(&tiers.near) {
                    assert!(
                        at.start < at.end && at.end <= text.len(),
                        "a caret at {caret} in {scope:?} scope lit {at:?}"
                    );
                    assert!(
                        text.is_char_boundary(at.start) && text.is_char_boundary(at.end),
                        "a caret at {caret} in {scope:?} scope lit {at:?}, which cuts a character"
                    );
                }
            }
        }
    }
}
