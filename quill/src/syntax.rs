//! The prose Annotators' source mapping and retained, block-relative spans.
//!
//! Syntax highlight, Style check and Spell check read the same prose, are
//! answered by the same Worker and are mapped back to the same source, so one
//! module holds their settings, their span stores and the one request that
//! names which of the three a paragraph is wanted for (#356, #401).

use std::ops::Range;

use quill_engine::annotate::{Mark, Span};
use quill_engine::document::{Document, Edit, Splice};
use quill_engine::markdown;
use quill_engine::pos::{Categories, Category};
use quill_engine::settings::{StyleCheck, SyntaxHighlight};
use quill_engine::spell::Misspelling;
use quill_engine::style::List;
use quill_engine::worker::{
    Annotators, Edit as Dictionary, Paragraph, ParagraphResult, Request, SpanStore, Worker,
};

use crate::session::StyleToggle;

type Tokens = Vec<(Range<usize>, Category)>;
type Struck = Vec<(Range<usize>, List)>;

#[derive(Default)]
struct ParagraphState {
    length: usize,
    categories: SpanStore<Category>,
    lists: SpanStore<List>,
    misspellings: SpanStore<Misspelling>,
    dirty: bool,
    mapping: Mapping,
}

#[derive(Default)]
struct Mapping {
    fragments: Vec<Fragment>,
}

struct Fragment {
    prose: Range<usize>,
    source: Range<usize>,
}

impl Mapping {
    fn extract(source: &str, resolved: &[Span], origin: usize) -> (String, Self) {
        let mut prose = String::new();
        let mut fragments = Vec::new();
        // The Document supplies reference context without reparsing the file.
        let links: Vec<_> = resolved
            .iter()
            .filter(|span| matches!(span.mark, Mark::Link | Mark::Image))
            .map(|span| span.at.start - origin)
            .collect();
        // One pass over one dirty block's prose events; only references add
        // the lookup cost stated by prose_with_links. Request tests pin scope.
        let mut previous = 0;
        for fragment in markdown::prose_with_links(source, &links) {
            // Inline delimiters join a split word; omitted code, URLs and
            // line breaks separate words rather than inventing a larger one.
            let gap = &source[previous..fragment.at.start];
            if !prose.is_empty() && !gap.chars().all(|c| "*_~[]".contains(c)) {
                prose.push(' ');
            }
            let start = prose.len();
            prose.push_str(fragment.text);
            previous = fragment.at.end;
            fragments.push(Fragment {
                prose: start..prose.len(),
                source: fragment.at,
            });
        }
        (prose, Self { fragments })
    }

    /// Generic over what the span carries: a Category and a List are mapped
    /// back to the source the same way, and a phrase split by emphasis is
    /// struck in as many pieces as a word split by emphasis is coloured.
    fn source_spans<K: Copy>(&self, tokens: Vec<(Range<usize>, K)>) -> Vec<(Range<usize>, K)> {
        let mut out = Vec::new();
        let mut first = 0;
        // Ascending tokens and fragments: O(tokens + fragments + output),
        // confined to one answered block, including words split by emphasis.
        for (token, category) in tokens {
            while self
                .fragments
                .get(first)
                .is_some_and(|f| f.prose.end <= token.start)
            {
                first += 1;
            }
            for fragment in self.fragments[first..]
                .iter()
                .take_while(|f| f.prose.start < token.end)
            {
                let start = token.start.max(fragment.prose.start);
                let end = token.end.min(fragment.prose.end);
                if start < end {
                    out.push((
                        fragment.source.start + start - fragment.prose.start
                            ..fragment.source.start + end - fragment.prose.start,
                        category,
                    ));
                }
            }
        }
        out
    }
}

/// The state read by every paint and driven by Window's idle work.
#[derive(Default)]
pub struct Syntax {
    settings: SyntaxHighlight,
    style: StyleCheck,
    /// The `spell_check` setting.
    spell: bool,
    /// The dictionary edits a new worker is handed before anything else: the
    /// language, then the words ignored under it. An Add is the dictionary's
    /// own file and is not kept.
    dictionary: Vec<Dictionary>,
    paragraphs: Vec<ParagraphState>,
    worker: Worker,
    outstanding: usize,
}

impl Syntax {
    /// Syntax highlight's master switch, including before a worker has ever
    /// been started.
    pub(crate) fn enabled(&self) -> bool {
        self.settings.enabled
    }

    /// Which Annotators the writer has on, as a request names them.
    ///
    /// The per-Annotator answer: the Editor arms its wake on any, and a
    /// paragraph is sent once and tagged once whichever of the three is wanted.
    pub(crate) fn wanted(&self) -> Annotators {
        Annotators {
            syntax: self.settings.enabled,
            style: self.style.enabled,
            spell: self.spell,
        }
    }

    /// Whether any state is kept at all: no Annotator on keeps none.
    ///
    /// The Editor's arming question, because the wake and the drain feed all
    /// three: Style check or Spell check alone on is asynchronous work to
    /// schedule.
    pub(crate) fn working(&self) -> bool {
        any(self.wanted())
    }

    /// Whether `list`'s spans are painted.
    ///
    /// Its own switch under the master, and independent of what the store
    /// holds: the matcher emits every List whatever the toggles say, so
    /// switching one off is a repaint and never a re-match (#356).
    pub(crate) fn paints(&self, list: List) -> bool {
        self.style.enabled && StyleToggle::List(list).of(&self.style)
    }

    /// The table's category mask, independent of retained tokens.
    pub(crate) fn categories(&self) -> Categories {
        if !self.enabled() {
            return Categories::NONE;
        }
        [
            (self.settings.nouns, Category::Nouns),
            (self.settings.verbs, Category::Verbs),
            (self.settings.adjectives, Category::Adjectives),
            (self.settings.adverbs, Category::Adverbs),
            (self.settings.conjunctions, Category::Conjunctions),
        ]
        .into_iter()
        .filter_map(|(on, category)| on.then_some(category))
        .collect()
    }

    /// Changes masks without discarding categories; only a master change
    /// moves the work.
    pub(crate) fn configure(&mut self, settings: SyntaxHighlight, document: &Document) -> bool {
        if self.settings == settings {
            return false;
        }
        let was = self.wanted();
        self.settings = settings;
        self.rewant(was, document);
        true
    }

    /// The same for Style check's table: a List switched off is a repaint, and
    /// only the master joining or leaving the request changes the work.
    pub(crate) fn configure_style(&mut self, style: StyleCheck, document: &Document) -> bool {
        if self.style == style {
            return false;
        }
        let was = self.wanted();
        self.style = style;
        self.rewant(was, document);
        true
    }

    /// The same for the `spell_check` setting: joining asks for every
    /// paragraph again, and leaving keeps the other two Annotators' spans and
    /// takes the wave off at the repaint that follows.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "the Editor's setter calls it from #409")
    )]
    pub(crate) fn configure_spell(&mut self, on: bool, document: &Document) -> bool {
        if self.spell == on {
            return false;
        }
        let was = self.wanted();
        self.spell = on;
        self.rewant(was, document);
        true
    }

    /// Answers a master switch without taking the other Annotator down with it.
    ///
    /// The pair shares one index and one store walk, so a [`Syntax::reset`]
    /// here empties the store of the Annotator that did not move as well as
    /// the one that did, and the page loses every mark until the next
    /// keystroke dirties a paragraph and asks again (#356).
    ///
    /// Only the first arrival and the last departure reset: the first has no
    /// index to keep, the last has no work to hold. Between them a joiner
    /// dirties every paragraph, because a request carries only the Annotators
    /// wanted when it was made and the spans it wants were never matched; a
    /// leaver needs nothing: painting reads the master switch, so its spans
    /// are simply unpainted, and they are held until the next answer for
    /// their paragraph arrives with an empty set for a kind nobody wants.
    /// That is the kept-spans rule the Annotators already follow — the marks
    /// a writer can see outlive the switch and go when newer ones land.
    fn rewant(&mut self, was: Annotators, document: &Document) {
        let wanted = self.wanted();
        if wanted == was {
            return;
        }
        if !any(was) || !self.working() {
            self.reset(document);
            return;
        }
        // Whole index on a master join only, never on a keystroke or a List:
        // the next request re-extracts every block's prose, the cost a master
        // enable already paid through reset.
        if (wanted.syntax && !was.syntax)
            || (wanted.style && !was.style)
            || (wanted.spell && !was.spell)
        {
            for entry in &mut self.paragraphs {
                entry.dirty = true;
            }
        }
    }

    /// Whether a paragraph is waiting to be asked about.
    ///
    /// One pass over the block headers, on a table change only: the wake is
    /// worth arming for work that exists, and a Category or a List moves no
    /// paragraph, so it must not restart a keystroke's debounce (#356).
    pub(crate) fn asking(&self) -> bool {
        self.working() && self.paragraphs.iter().any(|entry| entry.dirty)
    }

    /// Isolates a new or reloaded Document, even if its generation repeats.
    pub(crate) fn reset(&mut self, document: &Document) {
        self.worker = Worker::default();
        // A new worker knows no language: the edits that still hold go to it
        // first, so a reopened Document or a master's first arrival checks
        // against the list the writer left.
        for edit in self.dictionary.clone() {
            if let Err(error) = self.worker.edit(edit) {
                eprintln!("Spell check worker: {error}");
            }
        }
        self.outstanding = 0;
        self.paragraphs.clear();
        if self.working() {
            // Whole index only on open/master enable, never on a keystroke.
            self.paragraphs = document
                .blocks()
                .into_iter()
                .map(|b| ParagraphState {
                    length: b.at.len(),
                    dirty: true,
                    ..Default::default()
                })
                .collect();
        }
    }

    /// Carries kept colours and strikes over the splice before any Editor
    /// redraw.
    pub(crate) fn edited(&mut self, document: &Document, edit: &Edit) {
        if !self.working() {
            return;
        }
        let count = block_count(document);
        let old_end = edit.scope.blocks.end + self.paragraphs.len() - count;
        let first = edit.scope.blocks.start;
        let mut replacements: Vec<_> = edit
            .scope
            .blocks
            .clone()
            .map(|index| ParagraphState {
                length: document.block(index).at.len(),
                dirty: true,
                ..Default::default()
            })
            .collect();
        // Only replaced blocks' tokens move. Unchanged suffix blocks retain
        // relative ranges; Vec::splice moves their headers on split/join only.
        // Structural-edit and long-document tests pin both scopes.
        //
        // Both stores are carried by the same walk, once each: a strike keeps
        // its place over a keystroke exactly as a colour does, so the marks a
        // writer can see never flash off between an edit and its answer.
        let kept = &self.paragraphs[first..old_end];
        carry(
            document,
            edit,
            kept,
            &mut replacements,
            first,
            |paragraph| paragraph.categories.spans(),
            |target| target.categories.spans_mut(),
        );
        carry(
            document,
            edit,
            kept,
            &mut replacements,
            first,
            |paragraph| paragraph.lists.spans(),
            |target| target.lists.spans_mut(),
        );
        carry(
            document,
            edit,
            kept,
            &mut replacements,
            first,
            |paragraph| paragraph.misspellings.spans(),
            |target| target.misspellings.spans_mut(),
        );
        self.paragraphs.splice(first..old_end, replacements);
    }

    fn request(&mut self, document: &Document, viewport: Range<usize>) -> Option<Request> {
        if !self.working() {
            return None;
        }
        // On debounce/idle only: one pass over block headers, parsing only
        // dirty blocks. Dirty survives submission until a current answer lands.
        let paragraphs = self
            .paragraphs
            .iter_mut()
            .enumerate()
            .filter_map(|(index, entry)| {
                if !entry.dirty {
                    return None;
                }
                let at = document.block(index).at;
                let source = &document.text()[at.clone()];
                let (prose, mapping) = Mapping::extract(source, &document.spans_in(&at), at.start);
                entry.mapping = mapping;
                Some(Paragraph { index, prose })
            })
            .collect::<Vec<_>>();
        (!paragraphs.is_empty()).then_some(Request {
            generation: document.generation(),
            wanted: self.wanted(),
            paragraphs,
            viewport,
        })
    }

    /// Submits the same guarded request path the headless tests exercise.
    pub(crate) fn submit(&mut self, document: &Document, viewport: Range<usize>) {
        if let Some(request) = self.request(document, viewport) {
            let count = request.paragraphs.len();
            match self.worker.request(request) {
                Ok(()) => self.outstanding += count,
                Err(error) => eprintln!("Syntax highlight worker: {error}"),
            }
        }
    }

    /// Forwards a dictionary edit — an Add, an Ignore or a language — and asks
    /// for the whole Document again, viewport first.
    ///
    /// The worker applies the edit before the request sent after it, so the
    /// re-request checks against the list the edit just changed (#401 § Who
    /// holds the checker). The language and the words ignored under it are
    /// kept for [`Syntax::reset`] to hand the next worker. With Spell check off
    /// the edit still goes, so the dictionary is right when it comes on, and
    /// nothing is asked.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "the Editor and the corrections menu call it from #409 and #411"
        )
    )]
    pub(crate) fn edit_dictionary(
        &mut self,
        edit: Dictionary,
        document: &Document,
        viewport: Range<usize>,
    ) {
        match &edit {
            Dictionary::Language(_) if self.dictionary.first() != Some(&edit) => {
                self.dictionary = vec![edit.clone()];
            }
            Dictionary::Ignore(_) => self.dictionary.push(edit.clone()),
            Dictionary::Language(_) | Dictionary::Add(_) => {}
        }
        if let Err(error) = self.worker.edit(edit) {
            eprintln!("Spell check worker: {error}");
        }
        if !self.spell {
            return;
        }
        for entry in &mut self.paragraphs {
            entry.dirty = true;
        }
        self.submit(document, viewport);
    }

    fn accept(&mut self, mut result: ParagraphResult, document: &Document) -> Option<Range<usize>> {
        let paragraph = result.paragraph;
        let entry = self.paragraphs.get_mut(paragraph)?;
        // One answer, three stores: each takes out its own spans and all read
        // the same generation, so a stale answer changes none and a current
        // one lands as one redraw of the paragraph.
        let generation = document.generation();
        let categories = entry.categories.apply(&mut result, generation, |spans| {
            entry.mapping.source_spans(spans)
        });
        let lists = entry.lists.apply(&mut result, generation, |spans| {
            entry.mapping.source_spans(spans)
        });
        let misspellings = entry.misspellings.apply(&mut result, generation, |spans| {
            entry.mapping.source_spans(spans)
        });
        if !categories && !lists && !misspellings {
            return None;
        }
        entry.dirty = false;
        let at = document.block(paragraph).at;
        Some(document.place(at.start).line..document.place(at.end.saturating_sub(1)).line + 1)
    }

    /// Takes at most eight paragraphs per idle wake, keeping GTK responsive.
    pub(crate) fn drain(&mut self, document: &Document) -> Vec<Range<usize>> {
        let mut lines = Vec::new();
        for _ in 0..8 {
            let result = match self.worker.try_recv() {
                Ok(result) => result,
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.outstanding = 0;
                    eprintln!("Syntax highlight worker stopped before answering");
                    break;
                }
            };
            self.outstanding = self.outstanding.saturating_sub(1);
            if let Some(at) = self.accept(result, document) {
                lines.push(at);
            }
        }
        lines
    }

    /// Whether an idle wake is still owed.
    pub(crate) fn pending(&self) -> bool {
        self.outstanding > 0
    }

    /// Absolute tokens in the drawn range; disabled draws allocate nothing.
    pub(crate) fn spans_in(&self, document: &Document, at: &Range<usize>) -> Tokens {
        if !self.enabled() {
            return Vec::new();
        }
        self.stored_in(document, at, |entry| entry.categories.spans())
    }

    /// Absolute List spans in the drawn range, every List the store holds.
    ///
    /// The painting asks [`Syntax::paints`] which of them to strike: a List
    /// switched off keeps its spans here and loses its tag alone.
    pub(crate) fn lists_in(&self, document: &Document, at: &Range<usize>) -> Struck {
        if !self.style.enabled {
            return Vec::new();
        }
        self.stored_in(document, at, |entry| entry.lists.spans())
    }

    /// What Style check draws over the drawn range: the enabled Lists' spans,
    /// merged where only whitespace lies between two of them.
    ///
    /// The two things the mark is made of read this rather than
    /// [`Syntax::lists_in`] — the ink, through
    /// [`quill_engine::annotate::Annotated`], and the rule, through
    /// [`crate::tags`] — so a merged mark is one colour under one line.
    ///
    /// The merge is the Design oracle's: two struck phrases either side of one
    /// space are ruled straight through, `down only` in the capture's passage,
    /// and a rule per span would draw two with a gap (#354,
    /// `ref/ia/mac-native/VERDICTS.md` § The Style Check mark, rule extent).
    /// The merged range keeps the earlier span's List, so switching that List
    /// off takes the joined rule with it and leaves the later span its own.
    pub(crate) fn struck_in(&self, document: &Document, at: &Range<usize>) -> Struck {
        let mut out: Struck = Vec::new();
        for (span, list) in self.lists_in(document, at) {
            if !self.paints(list) {
                continue;
            }
            match out.last_mut() {
                Some((last, _))
                    if document
                        .text()
                        .get(last.end..span.start)
                        .is_some_and(|between| {
                            !between.is_empty() && between.chars().all(char::is_whitespace)
                        }) =>
                {
                    last.end = span.end;
                }
                _ => out.push((span, list)),
            }
        }
        out
    }

    /// [`Syntax::struck_in`]'s spans with their Lists dropped, which is what
    /// both halves of the mark ask for.
    ///
    /// All three Lists draw the same mark (#356 § The mark), so the ink and the
    /// rule want the extents alone; reading them through one method keeps the
    /// two from drifting apart from each other or from the merge above.
    pub(crate) fn struck_ranges_in(
        &self,
        document: &Document,
        at: &Range<usize>,
    ) -> Vec<Range<usize>> {
        self.struck_in(document, at)
            .into_iter()
            .map(|(span, _)| span)
            .collect()
    }

    /// The words Spell check marks over the drawn range, and none while it is
    /// off.
    ///
    /// Every word the dictionary refused, the one the caret stands in among
    /// them: the store is complete, and the caret rule is the Editor's, applied
    /// at paint (#401 § The tokeniser and the rules).
    pub(crate) fn misspelled_in(
        &self,
        document: &Document,
        at: &Range<usize>,
    ) -> Vec<Range<usize>> {
        if !self.spell {
            return Vec::new();
        }
        self.stored_in(document, at, |entry| entry.misspellings.spans())
            .into_iter()
            .map(|(span, _)| span)
            .collect()
    }

    /// The retained spans one store holds over the drawn range, in the
    /// Document's own bytes.
    fn stored_in<K: Copy>(
        &self,
        document: &Document,
        at: &Range<usize>,
        store: impl Fn(&ParagraphState) -> &[(Range<usize>, K)],
    ) -> Vec<(Range<usize>, K)> {
        if at.is_empty() {
            return Vec::new();
        }
        let Some(first) = document.block_at(at.start) else {
            return Vec::new();
        };
        let last = document.block_at(at.end.saturating_sub(1)).unwrap_or(first);
        let mut tokens = Vec::new();
        // Only drawn blocks, binary-searching each block's sorted tokens:
        // O(drawn blocks * log(tokens per block) + intersecting tokens).
        for index in first..=last {
            let Some(entry) = self.paragraphs.get(index) else {
                continue;
            };
            let origin = document.block(index).at.start;
            let spans = store(entry);
            let first = spans.partition_point(|(span, _)| origin + span.end <= at.start);
            for (span, carried) in spans[first..]
                .iter()
                .take_while(|(span, _)| origin + span.start < at.end)
            {
                tokens.push((origin + span.start..origin + span.end, *carried));
            }
        }
        tokens
    }
}

/// Carries one store's spans across `edit`'s splice into `replacements`.
///
/// Both stores go through this walk, so a Category and a List are rebased by
/// the same rule and split across the same block boundaries; only the store
/// read and the store written differ.
fn carry<K: Copy>(
    document: &Document,
    edit: &Edit,
    kept: &[ParagraphState],
    replacements: &mut [ParagraphState],
    first: usize,
    from: impl Fn(&ParagraphState) -> &[(Range<usize>, K)],
    into: impl Fn(&mut ParagraphState) -> &mut Vec<(Range<usize>, K)>,
) {
    let mut origin = edit.scope.bytes.start;
    for paragraph in kept {
        for (at, carried) in from(paragraph) {
            let at = rebased(&(origin + at.start..origin + at.end), &edit.splice);
            if at.is_empty() {
                continue;
            }
            let Some(start) = document.block_at(at.start) else {
                continue;
            };
            let end = document.block_at(at.end.saturating_sub(1)).unwrap_or(start);
            for index in start.max(first)..=end.min(edit.scope.blocks.end.saturating_sub(1)) {
                if let Some(target) = replacements.get_mut(index - first) {
                    let block = document.block(index).at;
                    into(target).push((
                        at.start.max(block.start) - block.start
                            ..at.end.min(block.end) - block.start,
                        *carried,
                    ));
                }
            }
        }
        origin += paragraph.length;
    }
}

/// Whether a request naming `wanted` would be sent at all.
fn any(wanted: Annotators) -> bool {
    wanted.syntax || wanted.style || wanted.spell
}

fn block_count(document: &Document) -> usize {
    document
        .block_at(document.text().len())
        .map_or(0, |index| index + 1)
}

fn rebased(at: &Range<usize>, splice: &Splice) -> Range<usize> {
    let shift = |offset: usize| {
        if offset < splice.at.start {
            offset
        } else if offset <= splice.at.end {
            splice.at.start + splice.inserted
        } else {
            offset - splice.at.len() + splice.inserted
        }
    };
    shift(at.start)..shift(at.end)
}

#[cfg(test)]
mod tests {
    use super::*;
    use quill_engine::annotate::{self, Annotated};
    use quill_engine::focus::Focus;
    use quill_engine::theme::{Colours, Scheme};

    fn document(text: &str) -> Document {
        let mut document = Document::untitled();
        document.insert(0, text);
        document
    }

    fn enabled(document: &Document) -> Syntax {
        let mut syntax = Syntax::default();
        let mut settings = SyntaxHighlight::default();
        settings.enabled = true;
        syntax.configure(settings, document);
        syntax
    }

    fn answer(syntax: &mut Syntax, document: &Document) {
        let request = syntax.request(document, 0..1).unwrap();
        for paragraph in request.paragraphs {
            syntax.accept(
                ParagraphResult {
                    generation: request.generation,
                    paragraph: paragraph.index,
                    categories: quill_engine::pos::categories(&paragraph.prose),
                    lists: Vec::new(),
                    misspellings: Vec::new(),
                },
                document,
            );
        }
    }

    fn striking(document: &Document, lists: [bool; 3]) -> Syntax {
        let mut syntax = Syntax::default();
        syntax.configure_style(style(lists), document);
        syntax
    }

    fn style(lists: [bool; 3]) -> StyleCheck {
        let mut style = StyleCheck::default();
        style.enabled = true;
        [style.fillers, style.redundancies, style.cliches] = lists;
        style
    }

    #[test]
    fn style_check_alone_is_asked_for_and_its_spans_kept_beside_the_categories() {
        let document = document("Basically Alice reads.");
        let mut syntax = striking(&document, [true; 3]);
        assert_eq!(
            syntax.wanted(),
            Annotators {
                syntax: false,
                style: true,
                spell: false
            }
        );
        let request = syntax.request(&document, 0..1).unwrap();
        assert_eq!(request.wanted, syntax.wanted());
        // One answer carries both sets under one generation, and each store
        // takes out its own: a paragraph is sent once and tagged once.
        assert!(
            syntax
                .accept(
                    ParagraphResult {
                        generation: request.generation,
                        paragraph: 0,
                        categories: vec![(10..15, Category::Nouns)],
                        lists: vec![(0..9, List::Fillers)],
                        misspellings: Vec::new(),
                    },
                    &document
                )
                .is_some()
        );
        let whole = 0..document.text().len();
        assert_eq!(syntax.lists_in(&document, &whole), [(0..9, List::Fillers)]);
        // Syntax highlight is off, so its spans are kept and not painted.
        assert!(syntax.spans_in(&document, &whole).is_empty());
        assert!(syntax.request(&document, 0..1).is_none());
    }

    /// One List on under the master paints that List and no other.
    ///
    /// The other side of the tag's name (`tags::style_mark_name`): a switch
    /// wired to the wrong field would paint a List whose check is off, and the
    /// `style` Piece shoots only two of the eight combinations, so the six it
    /// never sees are caught here (#356).
    #[test]
    fn one_list_on_is_that_list_and_no_other_painted() {
        let document = document("Basically Alice reads.");
        for (index, only) in List::ALL.into_iter().enumerate() {
            let mut lists = [false; 3];
            lists[index] = true;
            let syntax = striking(&document, lists);
            for list in List::ALL {
                assert_eq!(
                    syntax.paints(list),
                    list == only,
                    "{only:?} alone on, asked about {list:?}",
                );
            }
        }
    }

    #[test]
    fn a_list_switched_off_keeps_its_spans_and_leaves_the_painting() {
        let document = document("Basically Alice reads.");
        let mut syntax = striking(&document, [true; 3]);
        let request = syntax.request(&document, 0..1).unwrap();
        syntax.accept(
            ParagraphResult {
                generation: request.generation,
                paragraph: 0,
                lists: vec![(0..9, List::Fillers)],
                ..ParagraphResult::default()
            },
            &document,
        );
        assert!(syntax.paints(List::Fillers));
        assert!(syntax.configure_style(style([false, true, true]), &document));
        let whole = 0..document.text().len();
        assert_eq!(syntax.lists_in(&document, &whole), [(0..9, List::Fillers)]);
        assert!(!syntax.paints(List::Fillers));
        assert!(syntax.paints(List::Cliches));
        // A List is a repaint and never a re-match: nothing is asked again.
        assert!(syntax.request(&document, 0..1).is_none());
        // The master is: switching it off drops the state, and on again
        // asks for every paragraph anew.
        assert!(syntax.configure_style(StyleCheck::default(), &document));
        assert!(syntax.lists_in(&document, &whole).is_empty());
        assert!(syntax.request(&document, 0..1).is_none());
        assert!(syntax.configure_style(style([false, true, true]), &document));
        assert!(syntax.request(&document, 0..1).is_some());
    }

    #[test]
    fn both_annotators_on_send_one_request_and_fill_both_stores() {
        let document = document("Basically Alice reads.");
        let mut syntax = enabled(&document);
        assert!(syntax.configure_style(style([true; 3]), &document));
        assert_eq!(
            syntax.wanted(),
            Annotators {
                syntax: true,
                style: true,
                spell: false
            }
        );
        let request = syntax.request(&document, 0..1).unwrap();
        assert_eq!(request.paragraphs.len(), 1);
        syntax.accept(
            ParagraphResult {
                generation: request.generation,
                paragraph: 0,
                categories: vec![(10..15, Category::Nouns)],
                lists: vec![(0..9, List::Fillers)],
                misspellings: Vec::new(),
            },
            &document,
        );
        let whole = 0..document.text().len();
        assert_eq!(
            syntax.spans_in(&document, &whole),
            [(10..15, Category::Nouns)]
        );
        assert_eq!(syntax.lists_in(&document, &whole), [(0..9, List::Fillers)]);
        assert!(syntax.request(&document, 0..1).is_none());
    }

    #[test]
    fn a_master_joining_keeps_the_others_spans_and_asks_for_its_own() {
        let document = document("Basically Alice reads.");
        let mut syntax = enabled(&document);
        answer(&mut syntax, &document);
        let whole = 0..document.text().len();
        let coloured = syntax.spans_in(&document, &whole);
        assert!(!coloured.is_empty());
        // Style check joins under Syntax highlight: the colours stay on the
        // page, and the strikes are asked for without waiting on a keystroke.
        assert!(syntax.configure_style(style([true; 3]), &document));
        assert_eq!(syntax.spans_in(&document, &whole), coloured);
        // The joiner has work, so the wake is armed for it; a List alone has
        // none, and must not restart a keystroke's debounce.
        assert!(syntax.asking());
        let request = syntax.request(&document, 0..1).unwrap();
        syntax.accept(
            ParagraphResult {
                generation: request.generation,
                paragraph: 0,
                categories: vec![(10..15, Category::Nouns)],
                lists: vec![(0..9, List::Fillers)],
                misspellings: Vec::new(),
            },
            &document,
        );
        assert_eq!(syntax.lists_in(&document, &whole), [(0..9, List::Fillers)]);
        assert!(!syntax.asking());
        assert!(syntax.configure_style(style([false, true, true]), &document));
        assert!(!syntax.asking());
        // The one that leaves takes nothing with it: the strikes outlive
        // Syntax highlight's departure, and its own spans are merely unpainted.
        assert!(syntax.configure(SyntaxHighlight::default(), &document));
        assert_eq!(syntax.lists_in(&document, &whole), [(0..9, List::Fillers)]);
        assert!(syntax.spans_in(&document, &whole).is_empty());
    }

    #[test]
    fn typing_carries_strikes_over_the_splice_with_the_colours() {
        let mut document = document("Basically Alice reads.");
        let mut syntax = enabled(&document);
        syntax.configure_style(style([true; 3]), &document);
        let request = syntax.request(&document, 0..1).unwrap();
        syntax.accept(
            ParagraphResult {
                generation: request.generation,
                paragraph: 0,
                categories: vec![(10..15, Category::Nouns)],
                lists: vec![(0..9, List::Fillers)],
                misspellings: Vec::new(),
            },
            &document,
        );
        let edit = document.insert(document.text().len(), " Bob walks.");
        syntax.edited(&document, &edit);
        let whole = 0..document.text().len();
        assert_eq!(syntax.lists_in(&document, &whole), [(0..9, List::Fillers)]);
        assert_eq!(
            syntax.spans_in(&document, &whole),
            [(10..15, Category::Nouns)]
        );
    }

    fn spelling(document: &Document) -> Syntax {
        let mut syntax = Syntax::default();
        syntax.configure_spell(true, document);
        syntax
    }

    #[test]
    fn spell_check_alone_asks_for_spell_only_and_all_three_off_ask_for_nothing() {
        let document = document("Teh cat reads.");
        let mut syntax = spelling(&document);
        assert_eq!(
            syntax.wanted(),
            Annotators {
                syntax: false,
                style: false,
                spell: true
            }
        );
        assert!(syntax.working());
        let request = syntax.request(&document, 0..1).unwrap();
        assert_eq!(request.wanted, syntax.wanted());
        assert!(syntax.configure_spell(false, &document));
        assert!(!syntax.working());
        assert!(syntax.request(&document, 0..1).is_none());
        assert!(Syntax::default().request(&document, 0..1).is_none());
    }

    #[test]
    fn three_sets_land_from_one_answer_under_one_generation() {
        let document = document("Basically Teh cat reads.");
        let mut syntax = enabled(&document);
        syntax.configure_style(style([true; 3]), &document);
        syntax.configure_spell(true, &document);
        let request = syntax.request(&document, 0..1).unwrap();
        assert_eq!(request.paragraphs.len(), 1);
        let answer = |generation| ParagraphResult {
            generation,
            paragraph: 0,
            categories: vec![(14..17, Category::Nouns)],
            lists: vec![(0..9, List::Fillers)],
            misspellings: vec![(10..13, Misspelling)],
        };
        let whole = 0..document.text().len();
        // A stale answer changes none of the three.
        assert!(
            syntax
                .accept(answer(request.generation + 1), &document)
                .is_none()
        );
        assert!(syntax.misspelled_in(&document, &whole).is_empty());
        assert!(
            syntax
                .accept(answer(request.generation), &document)
                .is_some()
        );
        assert_eq!(
            syntax.spans_in(&document, &whole),
            [(14..17, Category::Nouns)]
        );
        assert_eq!(syntax.lists_in(&document, &whole), [(0..9, List::Fillers)]);
        assert_eq!(
            syntax.misspelled_in(&document, &whole),
            std::slice::from_ref(&(10..13))
        );
        assert!(syntax.request(&document, 0..1).is_none());
    }

    #[test]
    fn misspelled_in_answers_the_painting_only_while_spell_check_is_on() {
        let mut document = document("Teh cat reads.");
        let mut syntax = enabled(&document);
        syntax.configure_spell(true, &document);
        let request = syntax.request(&document, 0..1).unwrap();
        syntax.accept(
            ParagraphResult {
                generation: request.generation,
                paragraph: 0,
                misspellings: vec![(0..3, Misspelling)],
                ..ParagraphResult::default()
            },
            &document,
        );
        // Carried over a keystroke like a colour, so the wave never flashes
        // off between an edit and its answer.
        let edit = document.insert(0, "So ");
        syntax.edited(&document, &edit);
        let whole = 0..document.text().len();
        assert_eq!(
            syntax.misspelled_in(&document, &whole),
            std::slice::from_ref(&(3..6))
        );
        // Leaving under Syntax highlight keeps the spans and paints none.
        assert!(syntax.configure_spell(false, &document));
        assert!(syntax.misspelled_in(&document, &whole).is_empty());
        assert!(syntax.configure_spell(true, &document));
        assert_eq!(
            syntax.misspelled_in(&document, &whole),
            std::slice::from_ref(&(3..6))
        );
    }

    #[test]
    fn each_dictionary_edit_is_forwarded_and_asks_for_the_whole_document_viewport_first() {
        let document = document("Teh cat.\n\nA dgo.\n\nThe end.");
        let mut syntax = spelling(&document);
        let request = syntax.request(&document, 0..3).unwrap();
        for paragraph in request.paragraphs {
            syntax.accept(
                ParagraphResult {
                    generation: request.generation,
                    paragraph: paragraph.index,
                    ..ParagraphResult::default()
                },
                &document,
            );
        }
        assert!(syntax.request(&document, 0..3).is_none());
        // A tag no provider serves, so the thread loads and writes nothing
        // whatever dictionaries this machine has.
        let language = Dictionary::Language(Some("zz_QUILL".into()));
        let ignore = Dictionary::Ignore("dgo".into());
        for edit in [
            language.clone(),
            ignore.clone(),
            Dictionary::Add("Teh".into()),
        ] {
            syntax.edit_dictionary(edit.clone(), &document, 1..2);
            assert!(syntax.worker.is_running(), "{edit:?} reached no worker");
            assert!(syntax.pending(), "{edit:?} was followed by no request");
            let again = syntax.request(&document, 1..2).unwrap();
            assert_eq!(again.paragraphs.len(), syntax.paragraphs.len());
            assert_eq!(again.viewport, 1..2);
        }
        // The language and the word ignored under it outlive the worker; the
        // Add is the dictionary's own file.
        assert_eq!(syntax.dictionary, [language.clone(), ignore]);
        syntax.reset(&document);
        assert!(syntax.worker.is_running());
        // Off, an edit still goes and nothing is asked; a new language drops
        // the words ignored under the old one.
        assert!(syntax.configure_spell(false, &document));
        syntax.edit_dictionary(Dictionary::Language(None), &document, 0..1);
        assert_eq!(syntax.dictionary, [Dictionary::Language(None)]);
        assert!(syntax.request(&document, 0..1).is_none());
    }

    #[test]
    fn master_off_never_constructs_or_sends_a_request() {
        let mut document = document("Alice reads.");
        let mut syntax = Syntax::default();
        syntax.reset(&document);
        let edit = document.insert(5, " happily");
        syntax.edited(&document, &edit);
        syntax.submit(&document, 0..1);
        assert!(syntax.request(&document, 0..1).is_none());
        assert!(syntax.paragraphs.is_empty());
        assert!(!syntax.worker.is_running());
        assert!(!syntax.pending());
    }

    #[test]
    fn disabled_nouns_stay_stored_and_paint_body_ink() {
        let document = document("Alice reads.");
        let mut syntax = enabled(&document);
        answer(&mut syntax, &document);
        let tokens = syntax.spans_in(&document, &(0..document.text().len()));
        assert!(tokens.contains(&(0..5, Category::Nouns)));
        let mut settings = syntax.settings.clone();
        settings.nouns = false;
        syntax.configure(settings, &document);
        assert_eq!(
            syntax.spans_in(&document, &(0..document.text().len())),
            tokens
        );
        assert!(syntax.request(&document, 0..1).is_none());
        let colours = Colours::of(Scheme::Light);
        let drawn = annotate::paint_tagged_in(
            &[],
            Annotated {
                tagged: &tokens,
                enabled: syntax.categories(),
                struck: &[],
            },
            &(0..document.text().len()),
            &[],
            Focus::Off,
            &colours,
        );
        assert!(!drawn.iter().any(|run| run.at.start < 5));
    }

    #[test]
    fn mapping_splits_tokens_at_markup_without_colouring_invented_bytes() {
        let source = "é**lé**phant [fox](https://example.org) `code` &amp; \\*word";
        let (prose, mapping) = Mapping::extract(source, &[], 0);
        assert!(prose.starts_with("éléphant fox"), "{prose}");
        assert!(!prose.contains("code"));
        assert!(!prose.contains("https"));
        let spans = mapping.source_spans(vec![(0.."éléphant".len(), Category::Nouns)]);
        assert_eq!(
            spans
                .iter()
                .map(|(at, _)| &source[at.clone()])
                .collect::<Vec<_>>(),
            ["é", "lé", "phant"]
        );
        assert!(
            mapping
                .fragments
                .iter()
                .all(|f| source[f.source.clone()] == prose[f.prose.clone()])
        );
        for source in [
            "```rust\nAlice reads.\n```\n",
            "    Alice reads.\n",
            "---\ntitle: Alice\n---\n",
        ] {
            assert!(Mapping::extract(source, &[], 0).0.is_empty(), "{source}");
        }
    }

    #[test]
    fn requests_remove_external_reference_labels_and_map_only_the_linked_words() {
        let document = document(
            "An introduction.\n\n[é**lé**phant][animal] reads [books][volume].\n\n[animal]: https://example.org/elephant\n[volume]: https://example.org/books\n",
        );
        let mut syntax = enabled(&document);
        let request = syntax.request(&document, 0..1).unwrap();
        let paragraph = request
            .paragraphs
            .iter()
            .find(|p| p.prose.contains("phant"))
            .unwrap();
        assert_eq!(
            paragraph.prose.split_whitespace().collect::<Vec<_>>(),
            ["éléphant", "reads", "books", "."]
        );
        assert!(
            request
                .paragraphs
                .iter()
                .all(|p| !p.prose.contains(['[', ']', '*'])
                    && !p.prose.contains("animal")
                    && !p.prose.contains("https"))
        );
        assert!(
            syntax
                .accept(
                    ParagraphResult {
                        generation: request.generation,
                        paragraph: paragraph.index,
                        categories: vec![(0.."éléphant".len(), Category::Nouns)],
                        lists: Vec::new(),
                        misspellings: Vec::new(),
                    },
                    &document
                )
                .is_some()
        );
        let spans = syntax.spans_in(&document, &(0..document.text().len()));
        assert_eq!(
            spans
                .iter()
                .map(|(at, _)| &document.text()[at.clone()])
                .collect::<Vec<_>>(),
            ["é", "lé", "phant"]
        );
    }

    #[test]
    fn requests_preserve_collapsed_shortcut_and_image_reference_context() {
        for linked in ["[books][]", "[books]", "![books][]", "![books][volume]"] {
            let document = document(&format!(
                "An introduction.\n\n{linked} matter.\n\n[books]: /x\n[volume]: /y\n"
            ));
            let mut syntax = enabled(&document);
            let request = syntax.request(&document, 0..1).unwrap();
            let paragraph = request
                .paragraphs
                .iter()
                .find(|p| p.prose.contains("books"))
                .unwrap();
            assert_eq!(
                paragraph.prose.split_whitespace().collect::<Vec<_>>(),
                ["books", "matter."],
                "{linked}"
            );
        }
        let document = document("[unknown] remains literal.");
        let mut syntax = enabled(&document);
        let request = syntax.request(&document, 0..1).unwrap();
        assert_eq!(request.paragraphs[0].prose, document.text());
    }

    #[test]
    fn resolved_links_do_not_consume_adjacent_literal_brackets() {
        for linked in ["[books][unknown]", "[books][volume][]"] {
            let source = format!("{linked} matter.\n\n[books]: /x\n[volume]: /y\n");
            let document = document(&source);
            let mut syntax = enabled(&document);
            let request = syntax.request(&document, 0..1).unwrap();
            assert_eq!(
                request.paragraphs[0].prose,
                Mapping::extract(&source, &[], 0).0,
                "{linked}"
            );
        }
    }

    #[test]
    fn stale_answers_leave_dirty_paragraphs_in_the_next_generation_request() {
        let mut document = document("Alice reads.\n\nBob walks.");
        let mut syntax = enabled(&document);
        let old = syntax.request(&document, 0..1).unwrap();
        let edit = document.insert(5, " slowly");
        syntax.edited(&document, &edit);
        assert!(
            syntax
                .accept(
                    ParagraphResult {
                        generation: old.generation,
                        paragraph: 0,
                        ..ParagraphResult::default()
                    },
                    &document
                )
                .is_none()
        );
        let new = syntax.request(&document, 0..1).unwrap();
        assert_eq!(new.paragraphs.len(), block_count(&document));
        answer(&mut syntax, &document);
        assert!(syntax.request(&document, 0..1).is_none());
    }

    #[test]
    fn typing_rebases_kept_words_and_split_join_preserves_suffix_colours() {
        let mut document = document("Alice reads.\n\nBob walks.");
        let mut syntax = enabled(&document);
        answer(&mut syntax, &document);
        let edit = document.insert(2, "x");
        syntax.edited(&document, &edit);
        assert!(
            syntax
                .spans_in(&document, &(0..document.text().len()))
                .contains(&(0..6, Category::Nouns))
        );
        let edit = document.insert(7, "\n\n");
        syntax.edited(&document, &edit);
        let bob = document.text().find("Bob").unwrap();
        assert!(
            syntax
                .spans_in(&document, &(bob..bob + 3))
                .contains(&(bob..bob + 3, Category::Nouns))
        );
        let edit = document.delete(7..9);
        syntax.edited(&document, &edit);
        let bob = document.text().find("Bob").unwrap();
        assert!(
            syntax
                .spans_in(&document, &(bob..bob + 3))
                .contains(&(bob..bob + 3, Category::Nouns))
        );
        assert_eq!(syntax.paragraphs.len(), block_count(&document));
    }

    #[test]
    fn an_end_edit_only_requests_its_paragraph_and_adjacent_blank_in_a_long_document() {
        let mut document = document("Alice reads.\n\n".repeat(2000).trim_end());
        let mut syntax = enabled(&document);
        answer(&mut syntax, &document);
        let last = syntax.paragraphs.len() - 1;
        let first_allocation = syntax.paragraphs[0].categories.spans().as_ptr();
        let edit = document.insert(document.text().len() - 2, " now");
        syntax.edited(&document, &edit);
        assert_eq!(
            syntax.paragraphs[0].categories.spans().as_ptr(),
            first_allocation
        );
        let request = syntax.request(&document, last..last + 1).unwrap();
        assert_eq!(
            request
                .paragraphs
                .iter()
                .map(|p| p.index)
                .collect::<Vec<_>>(),
            [last - 1, last]
        );
        assert_eq!(
            syntax.spans_in(&document, &(0..5)),
            [(0..5, Category::Nouns)]
        );
    }

    #[test]
    fn replacing_a_document_drops_the_old_worker_even_when_generations_match() {
        let document = document("Alice reads.");
        let mut syntax = enabled(&document);
        syntax.submit(&document, 0..1);
        assert!(syntax.worker.is_running());
        syntax.reset(&document);
        assert!(!syntax.worker.is_running());
        assert!(!syntax.pending());
        assert!(syntax.spans_in(&document, &(0..5)).is_empty());
        assert!(syntax.request(&document, 0..1).is_some());
    }

    #[test]
    fn idle_drains_are_bounded_and_stop_after_the_last_answer() {
        let document = document(&"Alice reads.\n\n".repeat(20));
        let mut syntax = enabled(&document);
        syntax.submit(&document, 0..1);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let mut accepted = 0;
        while syntax.pending() {
            assert!(std::time::Instant::now() < deadline);
            let lines = syntax.drain(&document);
            assert!(lines.len() <= 8);
            accepted += lines.len();
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert_eq!(accepted, block_count(&document));
        assert!(syntax.request(&document, 0..1).is_none());
    }

    #[test]
    fn deleting_everything_and_typing_again_preserves_cache_indexing() {
        let mut document = document("Alice reads.\n\nBob walks.");
        let mut syntax = enabled(&document);
        answer(&mut syntax, &document);
        let edit = document.delete(0..document.text().len());
        syntax.edited(&document, &edit);
        assert!(syntax.spans_in(&document, &(0..0)).is_empty());
        let edit = document.insert(0, "Alice sleeps.");
        syntax.edited(&document, &edit);
        answer(&mut syntax, &document);
        assert_eq!(
            syntax.spans_in(&document, &(0..5)),
            [(0..5, Category::Nouns)]
        );
    }
}
