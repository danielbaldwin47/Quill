//! Both prose Annotators' source mapping and retained, block-relative spans.
//!
//! Syntax highlight and Style check read the same prose, are answered by the
//! same Worker and are mapped back to the same source, so one module holds both
//! settings tables, both span stores and the one request that names which of
//! the two a paragraph is wanted for (#356).

use std::ops::Range;

use quill_engine::annotate::{Mark, Span};
use quill_engine::document::{Document, Edit, Splice};
use quill_engine::markdown;
use quill_engine::pos::{Categories, Category};
use quill_engine::settings::{StyleCheck, SyntaxHighlight};
use quill_engine::style::List;
use quill_engine::worker::{Annotators, Paragraph, ParagraphResult, Request, SpanStore, Worker};

type Tokens = Vec<(Range<usize>, Category)>;
type Struck = Vec<(Range<usize>, List)>;

#[derive(Default)]
struct ParagraphState {
    length: usize,
    categories: SpanStore<Category>,
    lists: SpanStore<List>,
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
    /// The per-Annotator answer: the Editor arms its wake on either, and a
    /// paragraph is sent once and tagged once whichever of the two is wanted.
    pub(crate) fn wanted(&self) -> Annotators {
        Annotators {
            syntax: self.settings.enabled,
            style: self.style.enabled,
        }
    }

    /// Whether any state is kept at all: neither Annotator on keeps none.
    fn working(&self) -> bool {
        let wanted = self.wanted();
        wanted.syntax || wanted.style
    }

    /// Whether `list`'s spans are painted.
    ///
    /// Its own switch under the master, and independent of what the store
    /// holds: the matcher emits every List whatever the toggles say, so
    /// switching one off is a repaint and never a re-match (#356).
    pub(crate) fn paints(&self, list: List) -> bool {
        self.style.enabled
            && match list {
                List::Fillers => self.style.fillers,
                List::Redundancies => self.style.redundancies,
                List::Cliches => self.style.cliches,
            }
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

    /// Changes masks without discarding categories; a master change resets work.
    pub(crate) fn configure(&mut self, settings: SyntaxHighlight, document: &Document) -> bool {
        if self.settings == settings {
            return false;
        }
        let was = self.wanted();
        self.settings = settings;
        if self.wanted() != was {
            self.reset(document);
        }
        true
    }

    /// The same for Style check's table: a List switched off is a repaint, and
    /// only the master joining or leaving the request resets the work.
    #[allow(dead_code)] // the Editor calls it when #364 wires the table to the Annotator
    pub(crate) fn configure_style(&mut self, style: StyleCheck, document: &Document) -> bool {
        if self.style == style {
            return false;
        }
        let was = self.wanted();
        self.style = style;
        if self.wanted() != was {
            self.reset(document);
        }
        true
    }

    /// Isolates a new or reloaded Document, even if its generation repeats.
    pub(crate) fn reset(&mut self, document: &Document) {
        self.worker = Worker::default();
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

    fn accept(&mut self, mut result: ParagraphResult, document: &Document) -> Option<Range<usize>> {
        let paragraph = result.paragraph;
        let entry = self.paragraphs.get_mut(paragraph)?;
        // One answer, both stores: each takes out its own spans and both read
        // the same generation, so a stale answer changes neither and a current
        // one lands as one redraw of the paragraph.
        let generation = document.generation();
        let categories = entry.categories.apply(&mut result, generation, |spans| {
            entry.mapping.source_spans(spans)
        });
        let lists = entry.lists.apply(&mut result, generation, |spans| {
            entry.mapping.source_spans(spans)
        });
        if !categories && !lists {
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
    use quill_engine::annotate;
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
                style: true
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
                style: true
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
            &tokens,
            syntax.categories(),
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
