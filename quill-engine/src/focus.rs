//! Focus: which bytes are bright and which are dim.
//!
//! Focus dims everything the writer is not in. There are two tiers: the
//! **bright** tier is the sentence — or the paragraph — the caret is in, and
//! everything else is **dim**. There is one dim tier per ground and both scopes
//! share it (`docs/design.md` § Dim tiers). The near tier this module was
//! written with — the sentence either side, a step above dim — is withdrawn by
//! [ADR 0015]: it was invented for the JavaScript app, the Design oracle has
//! nothing like it, and against `mac-native` it reads as blur rather than as a
//! thought held open. [ADR 0006]'s status note carries the narrowing.
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
//! - **A caret on a blank line lights the sentence above it**, brightly. The
//!   oracle held that sentence one tier down (`focus.js:135-141`), and with the
//!   near tier gone the choice is between the writer's last thought and the
//!   whole page going grey on every Enter. The rule was there to stop the page
//!   blacking out behind a new thought, so it keeps the tier that still says
//!   that.
//!
//! Focus is on the keystroke path (#41), so the cost has to be flat in the size
//! of the Document. [`tiers`] reads the caret's block and, for the blank-line
//! rule, the two lines above it — [`reach`] is that range, and nothing outside
//! it is looked at.

use std::collections::BTreeMap;
use std::ops::Range;

use crate::document::{Document, Splice};
use crate::settings::{FocusScope, Settings};

pub mod typewriter;

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

impl Focus {
    /// The two values [`Settings`] keeps apart, read as the one value Focus is.
    ///
    /// [`Settings::focus_scope`] outlives Focus being switched off — it is what
    /// the shortcut restores — so the scope is only a scope while
    /// [`Settings::focus`] is true, and this is the one place the pair becomes
    /// the enum the rest of Focus is written against.
    ///
    /// [`Settings`]: crate::settings::Settings
    /// [`Settings::focus`]: crate::settings::Settings::focus
    /// [`Settings::focus_scope`]: crate::settings::Settings::focus_scope
    #[must_use]
    pub fn of(settings: &Settings) -> Self {
        Self::at(settings.focus, settings.focus_scope)
    }

    /// The same pair, where it is held as two values rather than read off a
    /// [`Settings`].
    ///
    /// The app keeps Focus and its scope live while it runs, because the keys
    /// move them and the scope has to outlive Focus being switched off; this is
    /// how that pair becomes the enum, so that there is one rule for what the
    /// two mean together and not one per place they are kept.
    ///
    /// [`Settings`]: crate::settings::Settings
    #[must_use]
    pub const fn at(on: bool, scope: FocusScope) -> Self {
        if on { Self::On(scope) } else { Self::Off }
    }
}

/// Which of the two tiers a stretch of bytes is in.
///
/// The flattening resolves this against a Markup mark to reach one colour
/// ([`crate::annotate::colour`]), which is the whole of what Focus does to the
/// page.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash)]
pub enum Tier {
    /// What Focus lights: drawn as it would be with Focus off. Usually what the
    /// writer is in, though on a blank line it is the thought behind them.
    #[default]
    Bright,
    /// Everything else, drawn in the ground's dimmed grey.
    Dim,
}

/// The bytes Focus lights.
///
/// Everything the list leaves out is dim, which is why there is no second list:
/// dim is the page, and these are the holes cut in it.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Tiers {
    /// The bright ranges. Absolute UTF-8 bytes from the start of the Document.
    pub bright: Vec<Range<usize>>,
}

/// One line's share of the [`Tiers`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LineTiers {
    /// The line, counted from 0.
    pub line: usize,
    /// The bytes of that line Focus lights, clipped to it.
    pub tiers: Tiers,
}

/// The tiers for a caret — or a selection — at `at`, under `focus`.
///
/// `at` is the writer's selection as the buffer holds it: an empty range is a
/// caret, and a range with room in it is a live selection, which is the bright
/// span on its own (`focus.js:128-134`). While you are marking a phrase, the
/// phrase is what you are working on.
///
/// A caret on a blank line lights the last sentence of the block above it
/// (`focus.js:135-141` holds it one tier down, which there is no longer), so
/// that on every Enter with Focus on the page does not black out behind a new
/// thought.
#[must_use]
pub fn tiers(doc: &Document, at: &Range<usize>, focus: Focus) -> Tiers {
    let Focus::On(scope) = focus else {
        return Tiers::default();
    };
    let text = doc.text();
    let caret = at.start.min(text.len());
    let Some(block) = block_of(doc, caret) else {
        return Tiers::default();
    };
    let read = reach(doc, at, focus);
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
        return Tiers { bright };
    }
    if at.end > at.start {
        return Tiers {
            bright: only(caret..at.end.min(text.len())),
        };
    }

    let line = doc.place(caret).line;
    let here = content(doc, line);
    if text[here.clone()].trim().is_empty() {
        return Tiers {
            bright: behind(doc, line),
        };
    }

    let spans = sentences(&text[here.clone()]);
    let index = caret.saturating_sub(here.start).min(here.end - here.start);
    let this = sentence_at(&spans, index);
    Tiers {
        bright: only(here.start + spans[this].start..here.start + spans[this].end),
    }
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
    for at in &tiers.bright {
        for line in doc.place(at.start).line..=doc.place(last_byte(at)).line {
            let on = content(doc, line);
            let clipped = at.start.max(on.start)..at.end.min(on.end);
            if clipped.start >= clipped.end {
                continue;
            }
            lines.entry(line).or_default().bright.push(clipped);
        }
    }
    lines
        .into_iter()
        .map(|(line, tiers)| LineTiers { line, tiers })
        .collect()
}

/// The lines drawn differently now that the tiers are `after` and not `before`.
///
/// Ascending, and no two of them touching — the runs of consecutive lines whose
/// share of the tiers is not what it was. This is what the Editor retags on a
/// caret move (#113), in the shape [`crate::document::Edit::lines`] hands the
/// same retag after an edit (#90), and the reason a caret move costs the
/// sentence it left and the one it entered rather than a page: a line missing
/// from both lists is wholly dim in both and has nothing to redraw.
/// `a_one_character_caret_move_retags_at_most_two_lines` is that cost, over
/// the shared passage.
///
/// Two disjoint runs rather than one range covering both, because a caret that
/// jumps the length of a manuscript changes the tiers of the block it left and
/// the block it landed in and of nothing in between, and redrawing what lies
/// between them would put the cost of a click in proportion to how far it went.
///
/// Bounded by [`reach`] on each side: each list covers at most the lines of one
/// block, so there are at most two runs and they are as long as those blocks.
#[must_use]
pub fn changed(before: &[LineTiers], after: &[LineTiers]) -> Vec<Range<usize>> {
    let mut out: Vec<Range<usize>> = Vec::new();
    let (mut was, mut now) = (before.iter().peekable(), after.iter().peekable());
    loop {
        let line = match (was.peek(), now.peek()) {
            (None, None) => break,
            // A line lit in one and unlit in the other is a line that changed.
            (Some(one), None) => one.line,
            (None, Some(two)) => two.line,
            (Some(one), Some(two)) => one.line.min(two.line),
        };
        let one = was.next_if(|on| on.line == line);
        let two = now.next_if(|on| on.line == line);
        if one.map(|on| &on.tiers) == two.map(|on| &on.tiers) {
            continue;
        }
        match out.last_mut() {
            Some(run) if run.end == line => run.end = line + 1,
            _ => out.push(line..line + 1),
        }
    }
    out
}

/// The tiers as they were before an edit, carried into the text after it.
///
/// `before` holds bytes of the text as it was; `doc` is the text now, and
/// `splice` is what the edit did between the two. Every range is moved across
/// the splice — what lay after it slides by what the edit added or took, what
/// lay inside the bytes it took collapses to where they were — and the bytes
/// the edit brought in are drawn as `after`, the tiers read from the text now,
/// draws them: text the writer has just typed has no earlier colour to fade
/// from, so it is bright at once where the sentence it joined is bright.
///
/// This is what [`changed`] compares against after a keystroke. Compared
/// against `before` as it stands, a byte typed inside the lit sentence turned
/// `a..b` into `a..b + 1` and shifted every range below it, so the line read
/// as moved and the cross-fade carried the sentence's last byte from dim to
/// bright on every keystroke (#224, found by the owner's hand).
///
/// Bounded as [`changed`] is: each list holds the lines of one block at most
/// ([`reach`]), and the walk is once over each and then a sort of what they
/// held, so a keystroke costs the caret's block and not the manuscript.
#[must_use]
pub fn rebased(
    doc: &Document,
    before: &[LineTiers],
    splice: &Splice,
    after: &[LineTiers],
) -> Vec<LineTiers> {
    let came_in = splice.at.start..splice.at.start + splice.inserted;
    let carry = |offset: usize| {
        if offset <= splice.at.start {
            offset
        } else if offset >= splice.at.end {
            offset - splice.at.len() + splice.inserted
        } else {
            splice.at.start
        }
    };
    let mut bright: Vec<Range<usize>> = Vec::new();
    for was in before.iter().flat_map(|on| &on.tiers.bright) {
        let moved = carry(was.start)..carry(was.end);
        // Split around the bytes that came in: those are `after`'s to say.
        bright.push(moved.start..moved.end.min(came_in.start));
        bright.push(moved.start.max(came_in.end)..moved.end);
    }
    for now in after.iter().flat_map(|on| &on.tiers.bright) {
        bright.push(now.start.max(came_in.start)..now.end.min(came_in.end));
    }
    bright.retain(|at| at.start < at.end);
    bright.sort_by_key(|at| at.start);
    // Joined back up, so that a range split around the typed bytes and filled
    // in again reads as the one range it is.
    let mut joined: Vec<Range<usize>> = Vec::with_capacity(bright.len());
    for at in bright {
        match joined.last_mut() {
            Some(last) if at.start <= last.end => last.end = last.end.max(at.end),
            _ => joined.push(at),
        }
    }
    tiers_by_line(doc, &Tiers { bright: joined })
}

/// The one tier a stretch of bytes is drawn in, for a property that has only
/// one.
///
/// [`crate::annotate::paint`] cuts its runs at every tier boundary, which is
/// the right answer for the ink. A paragraph property — a code block's well —
/// belongs to whole lines and cannot be cut, so it needs the block's tier as a
/// single answer, and a block Focus lights any of is a block the writer is in.
/// With Focus off there are no tiers and the answer is the untiered one,
/// [`Tier::Bright`], as it is throughout the flattening.
///
/// One pass over the bright ranges, which [`reach`] bounds to the caret's block
/// however long the Document is: one range in Sentence scope, and one per line
/// of that block in Paragraph. `a_paragraph_property_costs_the_caret_s_block`
/// pins it.
#[must_use]
pub fn tier_in(tiers: &[LineTiers], focus: Focus, at: &Range<usize>) -> Tier {
    if matches!(focus, Focus::Off) {
        return Tier::Bright;
    }
    let lit = tiers
        .iter()
        .flat_map(|on| &on.tiers.bright)
        .any(|bright| bright.start < at.end && at.start < bright.end);
    if lit { Tier::Bright } else { Tier::Dim }
}

/// The bytes [`tiers`] reads to answer for `at`, and the only ones it looks at.
///
/// The caret's block, widened to whole lines, and — when the caret is parked on
/// a blank line — the lines [`behind_lines`] names above it. Two blocks at the
/// worst, whatever the Document weighs, which is what keeps Focus off the
/// keystroke budget (#41). A selection is its own reach: the writer asked for
/// those bytes to be lit, so they are what there is to read.
fn reach(doc: &Document, at: &Range<usize>, focus: Focus) -> Range<usize> {
    let Focus::On(scope) = focus else {
        return 0..0;
    };
    let text = doc.text();
    let caret = at.start.min(text.len());
    let Some(block) = block_of(doc, caret) else {
        return 0..0;
    };
    if scope == FocusScope::Sentence && at.end > at.start {
        return whole_lines(doc, &(caret..at.end.min(text.len())));
    }
    let mut read = whole_lines(doc, &block);
    if scope == FocusScope::Sentence {
        let line = doc.place(caret).line;
        if text[content(doc, line)].trim().is_empty() {
            for above in behind_lines(doc, line) {
                read.start = read.start.min(content(doc, above).start);
            }
        }
    }
    read
}

/// The block `caret` is in, as the bytes it covers.
fn block_of(doc: &Document, caret: usize) -> Option<Range<usize>> {
    doc.block_at(caret).map(|block| doc.block(block).at)
}

/// The lines the blank-line rule may look at, above a caret parked on `line`.
///
/// The line above, and the one above that only when the first is blank too: the
/// oracle steps over one blank line and no more (`focus.js:136-140`). Two blank
/// lines behind you and the thought they separate is far enough back to go dim
/// with everything else. [`behind`] reads these and [`reach`] bounds itself by
/// them, so the rule lives in one place.
fn behind_lines(doc: &Document, line: usize) -> Vec<usize> {
    let mut lines = Vec::new();
    let Some(above) = line.checked_sub(1) else {
        return lines;
    };
    lines.push(above);
    if doc.text()[content(doc, above)].trim().is_empty() {
        lines.extend(line.checked_sub(2));
    }
    lines
}

/// The last sentence of the block above a caret parked on a blank line.
fn behind(doc: &Document, line: usize) -> Vec<Range<usize>> {
    let text = doc.text();
    for above in behind_lines(doc, line) {
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
        let stop = skip(text, scan, &TERMINATORS);
        let before = skip(text, stop, &CLOSERS);
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

/// Where the run of `of` characters starting at `from` ends.
fn skip(text: &str, from: usize, of: &[char]) -> usize {
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

/// `at` without the line terminator that closes it: a block's range runs to the
/// newline, and there is nothing to light in a newline.
///
/// Only the terminator goes. The oracle lights a line to its last byte
/// (`focus.js:127`), so the two trailing spaces of a Markdown hard break stay in
/// the tier the rest of their line is in.
fn trimmed(text: &str, at: &Range<usize>) -> Range<usize> {
    if at.start >= at.end {
        return at.start..at.start;
    }
    at.start..at.start + text[at.clone()].trim_end_matches(['\n', '\r']).len()
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

    /// What a Document lights under `focus`, as source text. Everything it
    /// leaves out is dim.
    fn lit(doc: &Document, at: Range<usize>, focus: Focus) -> Vec<&str> {
        let text = doc.text();
        tiers(doc, &at, focus)
            .bright
            .iter()
            .map(|at| &text[at.clone()])
            .collect()
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
    fn the_judged_caret_lights_its_sentence_and_dims_the_one_after_it() {
        let doc = passage();
        assert_eq!(
            lit(&doc, 403..403, Focus::On(FocusScope::Sentence)),
            [
                "She went down the spiral stair, counting the steps the way she always did, and found the coat on its hook and the lantern beside it. "
            ],
            "the caret is in the paragraph's first sentence, and it is the only one lit"
        );
        assert_eq!(
            tiers(&doc, &(403..403), Focus::On(FocusScope::Sentence)),
            Tiers {
                bright: only(353..486),
            },
            "the sentence after it, 486..520, is dim with everything else"
        );
    }

    #[test]
    fn the_judged_caret_lights_its_whole_paragraph() {
        let doc = passage();
        let bright = lit(&doc, 403..403, Focus::On(FocusScope::Paragraph));
        assert_eq!(bright.len(), 1, "one paragraph is one bright range");
        assert!(
            bright[0].starts_with("She went down the spiral stair,")
                && bright[0].ends_with("the sound of a long vowel."),
            "the whole paragraph is lit, end to end: {:?}",
            bright[0]
        );
        assert_eq!(
            tiers(&doc, &(403..403), Focus::On(FocusScope::Paragraph)),
            Tiers {
                bright: only(353..568),
            }
        );
    }

    /// The two shapes the withdrawn near tier used to tell apart: a neighbour in
    /// another block and a neighbour in this one. One dim tier answers both the
    /// same way, which is the whole of what [ADR 0015] narrowed.
    #[test]
    fn only_the_carets_own_sentence_is_lit_wherever_its_neighbours_sit() {
        let across =
            document("First thought here.\n\nSecond thought here.\n\nThird thought here.\n");
        assert_eq!(
            lit(&across, 25..25, Focus::On(FocusScope::Sentence)),
            ["Second thought here."],
            "the sentences either side are in other blocks, and dim"
        );

        let within = document("A first line here.\nA second line here.\n\nAnother block.\n");
        assert_eq!(
            lit(&within, 25..25, Focus::On(FocusScope::Sentence)),
            ["A second line here."],
            "the line above is in this block, and dim just the same"
        );
    }

    #[test]
    fn a_live_selection_is_the_bright_span() {
        let doc = passage();
        assert_eq!(
            lit(&doc, 353..372, Focus::On(FocusScope::Sentence)),
            ["She went down the s"],
            "while you are marking a phrase, the phrase is what you are working on"
        );
    }

    #[test]
    fn a_caret_on_a_blank_line_lights_the_sentence_above_it() {
        let doc = document("One thought. And then another one.\n\nNext.\n");
        assert_eq!(
            lit(&doc, 35..35, Focus::On(FocusScope::Sentence)),
            ["And then another one."],
            "the page does not black out behind a new thought"
        );
    }

    #[test]
    fn a_caret_two_blank_lines_below_a_thought_leaves_it_dim() {
        let doc = document("One thought here.\n\n\n\nFar below.\n");
        assert!(
            lit(&doc, 20..20, Focus::On(FocusScope::Sentence)).is_empty(),
            "two blank lines back is far enough to go dim with everything else"
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
            for at in &on.tiers.bright {
                assert!(
                    at.start >= line.start && at.end <= line.end,
                    "line {} has {at:?}, which leaves {line:?}",
                    on.line
                );
            }
        }
    }

    #[test]
    fn a_hard_breaks_two_spaces_stay_in_the_tier_their_line_is_in() {
        let doc = document("A first line here.  \nA second line here.\n");
        let tiers = tiers(&doc, &(5..5), Focus::On(FocusScope::Paragraph));
        let text = doc.text();
        assert_eq!(
            tiers_by_line(&doc, &tiers)
                .iter()
                .map(|on| &text[on.tiers.bright[0].clone()])
                .collect::<Vec<_>>(),
            ["A first line here.  ", "A second line here."],
            "the oracle lights a line to its last byte (`focus.js:127`), so only \
             the terminator is trimmed away"
        );
    }

    #[test]
    fn tiers_by_line_lights_one_sentence_of_a_line_of_three() {
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
            ["And then another one. "],
            "the sentences either side of the caret's are dim, so the line has one hole in it"
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
            let read = reach(&doc, &(caret..caret), focus);
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
            for at in &tiers.bright {
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
                for at in &tiers.bright {
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

    // The retag: which lines the Editor draws again when the caret moves.

    /// The lines whose tier changed when the caret went from `from` to `to`.
    fn moved(doc: &Document, from: usize, to: usize, focus: Focus) -> Vec<Range<usize>> {
        let before = tiers_by_line(doc, &tiers(doc, &(from..from), focus));
        let after = tiers_by_line(doc, &tiers(doc, &(to..to), focus));
        changed(&before, &after)
    }

    #[test]
    fn a_caret_that_does_not_leave_its_sentence_changes_no_line() {
        let doc = document("A first thought. A second one.\n\nA third.\n");
        assert_eq!(
            moved(&doc, 2, 8, Focus::On(FocusScope::Sentence)),
            Vec::<Range<usize>>::new(),
            "the same sentence is lit before and after, so nothing is redrawn"
        );
    }

    #[test]
    fn a_caret_crossing_a_full_stop_retags_only_the_line_the_two_sentences_share() {
        let doc = document("A first thought. A second one.\n\nA third.\n");
        assert_eq!(
            moved(&doc, 2, 20, Focus::On(FocusScope::Sentence)),
            vec![0..1],
            "both sentences are on line 0, and no other line's tier moved"
        );
    }

    #[test]
    fn a_caret_leaving_a_paragraph_retags_the_two_it_is_between_and_nothing_between_them() {
        let doc = document("One.\n\nTwo.\n\nThree.\n\nFour.\n");
        assert_eq!(
            moved(&doc, 0, 20, Focus::On(FocusScope::Paragraph)),
            [0..1, 6..7],
            "the block left and the block landed in, as two runs: the lines \
             between them were dim before and are dim now"
        );
    }

    #[test]
    fn a_caret_move_with_focus_off_changes_no_line() {
        let doc = document("One.\n\nTwo.\n\nThree.\n");
        assert_eq!(
            moved(&doc, 0, 12, Focus::Off),
            Vec::<Range<usize>>::new(),
            "with Focus off there are no tiers, so a move has nothing to redraw \
             and the page is the one the buffer already draws"
        );
    }

    #[test]
    fn consecutive_changed_lines_are_one_run() {
        let doc = document("A block that runs on\nover two lines.\n\nApart.\n");
        assert_eq!(
            moved(&doc, 40, 0, Focus::On(FocusScope::Paragraph)),
            [0..2, 3..4],
            "the block spans lines 0 and 1, which are one run, and the block the \
             caret left is another"
        );
    }

    #[test]
    fn a_one_character_caret_move_retags_at_most_two_lines() {
        let doc = passage();
        let text = doc.text();
        let focus = Focus::On(FocusScope::Sentence);
        let mut widest = 0;
        let mut caret = 0;
        for next in (1..text.len()).filter(|at| text.is_char_boundary(*at)) {
            let lines: usize = moved(&doc, caret, next, focus)
                .iter()
                .map(std::iter::ExactSizeIterator::len)
                .sum();
            widest = widest.max(lines);
            caret = next;
        }
        assert_eq!(
            widest, 2,
            "a one-character caret move redraws the line the sentence it left \
             is on and the line the one it entered is on, and a sentence is \
             read off one line ([`tiers`]), so two is the whole of it"
        );
    }

    // The retag after an edit: the tiers from before it, carried across it.

    /// The lines whose tier changed when `text` was typed at `caret`, judged
    /// the way the Editor judges it: the tiers from before the keystroke,
    /// carried across it, against the tiers read from the text after.
    fn moved_by_typing(
        doc: &mut Document,
        caret: usize,
        text: &str,
        focus: Focus,
    ) -> Vec<Range<usize>> {
        let before = tiers_by_line(doc, &tiers(doc, &(caret..caret), focus));
        let edit = doc.insert(caret, text);
        let landed = caret + text.len();
        let after = tiers_by_line(doc, &tiers(doc, &(landed..landed), focus));
        changed(&rebased(doc, &before, &edit.splice, &after), &after)
    }

    #[test]
    fn a_keystroke_inside_the_lit_sentence_moves_no_line() {
        let focus = Focus::On(FocusScope::Sentence);
        let text = "A first thought. A second one.\n\nA third.\n";
        assert_eq!(
            moved_by_typing(&mut document(text), 8, "x", focus),
            Vec::<Range<usize>>::new(),
            "the sentence is one byte longer and still the lit one"
        );
        assert_eq!(
            moved_by_typing(&mut document(text), 40, "x", focus),
            Vec::<Range<usize>>::new(),
            "typed at the block's last byte, where the range's end moved"
        );
        assert_eq!(
            moved_by_typing(&mut document(text), 32, "x", focus),
            Vec::<Range<usize>>::new(),
            "typed at the block's first byte, where the range's start moved"
        );
    }

    #[test]
    fn a_keystroke_at_the_judged_caret_moves_no_line_of_the_passage() {
        let mut doc = passage();
        // `shots/oracle/states.json`, `focus/sentence`: `"caret": 140`.
        assert_eq!(
            moved_by_typing(&mut doc, 140, "x", Focus::On(FocusScope::Sentence)),
            Vec::<Range<usize>>::new(),
            "the keystroke the owner's hand test types at the judged state"
        );
    }

    #[test]
    fn typing_through_a_full_stop_moves_the_line_the_sentence_split_on() {
        let mut doc = document("A first thought Another one.\n\nA third.\n");
        assert_eq!(
            moved_by_typing(&mut doc, 15, ".", Focus::On(FocusScope::Sentence)),
            vec![0..1],
            "the line was one lit sentence and is now `A first thought.` lit \
             with `Another one.` dim: a tier moved, and its line is drawn again"
        );
        let mut doc = document("One Two three.\n");
        assert_eq!(
            moved_by_typing(&mut doc, 3, ". ", Focus::On(FocusScope::Sentence)),
            vec![0..1],
            "`. ` at a sentence's end: the caret lands in `Two three.`, which \
             is lit on its own now, and `One.` behind it went dim"
        );
    }

    #[test]
    fn a_deletion_across_a_range_edge_collapses_it_and_moves_only_its_line() {
        let focus = Focus::On(FocusScope::Sentence);
        let mut doc = document("One. Two.\n\nThree.\n");
        let before = tiers_by_line(&doc, &tiers(&doc, &(6..6), focus));
        let edit = doc.delete(2..7);
        let after = tiers_by_line(&doc, &tiers(&doc, &(2..2), focus));
        assert_eq!(
            changed(&rebased(&doc, &before, &edit.splice, &after), &after),
            vec![0..1],
            "`Two.` began inside the bytes taken out, so what is left of it \
             starts where they were; the joined sentence `Onwo.` is lit whole"
        );
    }

    #[test]
    fn a_paragraph_property_costs_the_caret_s_block() {
        let doc = passage();
        let text = doc.text();
        let block = doc.blocks()[0].at.clone();
        for scope in [FocusScope::Sentence, FocusScope::Paragraph] {
            let focus = Focus::On(scope);
            let mut widest = 0;
            for caret in (0..text.len()).filter(|at| text.is_char_boundary(*at)) {
                let lines = tiers_by_line(&doc, &tiers(&doc, &(caret..caret), focus));
                widest = widest.max(lines.iter().map(|on| on.tiers.bright.len()).sum::<usize>());
                // The answer is read the way `tags::draw` reads it, once per
                // span of the range being drawn.
                let _ = tier_in(&lines, focus, &block);
            }
            assert!(
                widest <= 8,
                "{scope:?} scope left {widest} bright ranges for a paragraph \
                 property to walk, which is not one block's worth"
            );
        }
    }

    // The pair of settings, read as one value.

    #[test]
    fn focus_off_keeps_the_scope_it_will_be_switched_back_on_at() {
        let mut settings = Settings::default();
        settings.focus_scope = FocusScope::Paragraph;
        settings.focus = false;
        assert_eq!(Focus::of(&settings), Focus::Off);
        settings.focus = true;
        assert_eq!(
            Focus::of(&settings),
            Focus::On(FocusScope::Paragraph),
            "the scope was remembered across the switch, which is what the \
             shortcut restores"
        );
    }

    // A paragraph property takes one tier for the whole block.

    #[test]
    fn a_block_focus_lights_any_of_is_a_block_the_writer_is_in() {
        let doc = document("One.\n\n```\ncode\n```\n\nThree.\n");
        let focus = Focus::On(FocusScope::Paragraph);
        let fence = doc.text().find("```").expect("the fence is in the text");
        let block = fence..fence + 12;
        let inside = tiers_by_line(&doc, &tiers(&doc, &(fence + 4..fence + 4), focus));
        assert_eq!(tier_in(&inside, focus, &block), Tier::Bright);
        let elsewhere = tiers_by_line(&doc, &tiers(&doc, &(0..0), focus));
        assert_eq!(tier_in(&elsewhere, focus, &block), Tier::Dim);
        assert_eq!(
            tier_in(&[], Focus::Off, &block),
            Tier::Bright,
            "with Focus off the untiered colour is the bright arm of the table"
        );
    }
}
