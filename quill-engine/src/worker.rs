//! The shared asynchronous Annotator worker and its generation-checked spans.
//!
//! The app owns the debounce timer and drains results on idle. This module
//! owns one lazy thread per Worker; Syntax highlight and Style check run here
//! now, and Spell check joins them when it lands. Dropping the Worker closes
//! both channels, so the thread stops without making the main loop join it.
//!
//! A request names the Annotators it wants, and a paragraph is sent once and
//! answered once whichever are on: its answer carries a Category set and a List
//! set together, the set of an Annotator that was not wanted left empty. The
//! two sets are kept apart by one [`SpanStore`] each, generic over the span's
//! kind, so a store applies its own half of an answer and leaves the other.

use std::io;
use std::ops::Range;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::pos::{self, Category};
use crate::style::{self, List, Lists};

/// Quiet time after the last keystroke before the app submits changed prose.
pub const DEBOUNCE: Duration = Duration::from_millis(100);

/// Ascending, non-overlapping spans of one kind over a paragraph's prose.
pub type Spans<K> = Vec<(Range<usize>, K)>;

/// One changed paragraph, stripped to the parser's prose stream by the caller.
#[derive(Debug)]
pub struct Paragraph {
    /// Its paragraph index in this Document generation.
    pub index: usize,
    /// Prose only; result offsets address this string, not Markdown source.
    pub prose: String,
}

/// Which Annotators a request wants; a request wanting neither is not sent.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Annotators {
    /// Syntax highlight: a Category per word.
    pub syntax: bool,
    /// Style check: a List per struck phrase.
    pub style: bool,
}

/// Changed paragraphs and the viewport they were captured with.
#[derive(Debug)]
pub struct Request {
    /// The Document generation at capture, echoed unchanged in every result.
    pub generation: u64,
    /// The Annotators to run over every paragraph of this request.
    pub wanted: Annotators,
    /// Each changed paragraph once; their order is kept within each priority.
    pub paragraphs: Vec<Paragraph>,
    /// Paragraph indices currently visible, answered before the rest.
    pub viewport: Range<usize>,
}

/// One paragraph's answer, whose byte ranges refer to its requested prose.
///
/// Both span sets arrive together under the one generation; the set of an
/// Annotator the request did not want is empty.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct ParagraphResult {
    /// The Document generation at capture.
    pub generation: u64,
    /// The paragraph index from the request.
    pub paragraph: usize,
    /// Ascending, non-overlapping Category spans, one per coloured word.
    pub categories: Spans<Category>,
    /// Ascending, non-overlapping List spans, one per struck phrase.
    pub lists: Spans<List>,
}

struct Running {
    requests: Sender<Request>,
    results: Receiver<ParagraphResult>,
    thread: JoinHandle<()>,
}

/// One lazy thread, reused for every request over this Document's lifetime.
#[derive(Default)]
pub struct Worker {
    running: Option<Running>,
}

impl Worker {
    /// Queues changed prose, starting the thread on the first request.
    ///
    /// The tagger model loads on that thread's first nonempty paragraph.
    ///
    /// # Errors
    ///
    /// Returns the spawn error if the thread cannot start, or `BrokenPipe`
    /// if a previously started worker stopped unexpectedly.
    pub fn request(&mut self, request: Request) -> io::Result<()> {
        if self.running.is_none() {
            let (requests, incoming) = mpsc::channel();
            let (outgoing, results) = mpsc::channel();
            let thread = thread::Builder::new()
                .name("quill-annotators".into())
                .spawn(move || run(incoming, outgoing))?;
            self.running = Some(Running {
                requests,
                results,
                thread,
            });
        }
        self.running
            .as_ref()
            .expect("the worker was started above")
            .requests
            .send(request)
            .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "Annotator worker stopped"))
    }

    /// Takes the next answer without waiting; the app calls this on idle.
    ///
    /// # Errors
    ///
    /// `Empty` means there is no answer yet (including before first use);
    /// `Disconnected` means the started worker has stopped.
    pub fn try_recv(&self) -> Result<ParagraphResult, TryRecvError> {
        self.running
            .as_ref()
            .map_or(Err(TryRecvError::Empty), |running| {
                running.results.try_recv()
            })
    }

    /// Whether the thread has started and has not stopped.
    #[must_use]
    pub fn is_running(&self) -> bool {
        self.running
            .as_ref()
            .is_some_and(|running| !running.thread.is_finished())
    }
}

fn run(requests: Receiver<Request>, results: Sender<ParagraphResult>) {
    // The lists are read and compiled on the first request that wants Style
    // check and kept for the thread's life, so a session that never turns it
    // on never touches the files and a keystroke never pays for them.
    let mut lists = None;
    for request in requests {
        // Two passes over only the requested paragraphs, no sort or document
        // scan. Each paragraph is tagged once; the order test pins both groups.
        for visible in [true, false] {
            for paragraph in &request.paragraphs {
                if request.viewport.contains(&paragraph.index) != visible {
                    continue;
                }
                let (categories, struck) = answer(&paragraph.prose, request.wanted, &mut lists);
                let result = ParagraphResult {
                    generation: request.generation,
                    paragraph: paragraph.index,
                    categories,
                    lists: struck,
                };
                if results.send(result).is_err() {
                    return;
                }
            }
        }
    }
}

/// One paragraph's two span sets, compiling the lists if this is their first
/// ask. Separate from `run` so a test can watch `lists` across two answers.
fn answer(
    prose: &str,
    wanted: Annotators,
    lists: &mut Option<Lists>,
) -> (Spans<Category>, Spans<List>) {
    let categories = if wanted.syntax {
        pos::categories(prose)
    } else {
        Vec::new()
    };
    let struck = if wanted.style {
        let lists = lists.get_or_insert_with(|| {
            let mut notes = Vec::new();
            let lists = Lists::load(&mut notes);
            for note in notes {
                eprintln!("Style check: {note}");
            }
            lists
        });
        style::struck(prose, lists)
    } else {
        Vec::new()
    };
    (categories, struck)
}

/// A span kind an answer carries: [`Category`] or [`List`]. Implemented for
/// those two only, so a [`SpanStore`] takes its own half and leaves the other.
pub trait Kind: Sized {
    /// Takes this kind's spans out of an answer.
    fn take(result: &mut ParagraphResult) -> Spans<Self>;
}

impl Kind for Category {
    fn take(result: &mut ParagraphResult) -> Spans<Self> {
        std::mem::take(&mut result.categories)
    }
}

impl Kind for List {
    fn take(result: &mut ParagraphResult) -> Spans<Self> {
        std::mem::take(&mut result.lists)
    }
}

/// One paragraph's last accepted spans of one kind, kept while a replacement
/// is pending.
///
/// The receiver owns paragraph indexing and chooses the stored coordinates
/// through `apply`'s mapping. The app stores source-relative spans here and
/// rebases them when its paragraph is edited. A paragraph answering both
/// Annotators holds one store per kind, each applying the same answer.
pub struct SpanStore<K> {
    spans: Spans<K>,
}

impl<K> Default for SpanStore<K> {
    fn default() -> Self {
        Self { spans: Vec::new() }
    }
}

impl<K: Kind> SpanStore<K> {
    /// Replaces this paragraph's spans only when its generation is current.
    /// Returns whether the result was accepted; stale answers change nothing.
    /// Mapping runs only for an accepted answer, against its captured prose.
    /// Takes this store's kind out of the answer, leaving the other kind for
    /// its own store.
    pub fn apply(
        &mut self,
        result: &mut ParagraphResult,
        generation: u64,
        map: impl FnOnce(Spans<K>) -> Spans<K>,
    ) -> bool {
        if result.generation != generation {
            return false;
        }
        self.spans = map(K::take(result));
        true
    }

    /// The last accepted spans, or none while a paragraph awaits its first.
    #[must_use]
    pub fn spans(&self) -> &[(Range<usize>, K)] {
        &self.spans
    }

    /// Retained spans for source rebasing; preserve ascending, disjoint ranges.
    pub fn spans_mut(&mut self) -> &mut Spans<K> {
        &mut self.spans
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SYNTAX: Annotators = Annotators {
        syntax: true,
        style: false,
    };
    const STYLE: Annotators = Annotators {
        syntax: false,
        style: true,
    };
    const BOTH: Annotators = Annotators {
        syntax: true,
        style: true,
    };

    fn request(generation: u64, prose: &str) -> Request {
        wanting(generation, prose, SYNTAX)
    }

    fn wanting(generation: u64, prose: &str, wanted: Annotators) -> Request {
        Request {
            generation,
            wanted,
            paragraphs: vec![Paragraph {
                index: 0,
                prose: prose.into(),
            }],
            viewport: 0..1,
        }
    }

    fn receive(worker: &Worker) -> ParagraphResult {
        worker
            .running
            .as_ref()
            .unwrap()
            .results
            .recv_timeout(Duration::from_secs(10))
            .expect("the worker answers without a display or main loop")
    }

    #[test]
    fn old_results_are_dropped_and_last_colours_survive_pending_work() {
        let mut worker = Worker::default();
        let mut store = SpanStore::<Category>::default();
        worker.request(request(0, "Alice reads.")).unwrap();
        assert!(store.apply(&mut receive(&worker), 0, std::convert::identity));
        assert_eq!(
            store.spans(),
            &[(0..5, Category::Nouns), (6..11, Category::Verbs)]
        );

        worker.request(request(1, "Alice walks.")).unwrap();
        worker.request(request(2, "Alice sleeps.")).unwrap();
        let mut stale = receive(&worker);
        assert_eq!(stale.generation, 1);
        assert!(!store.apply(&mut stale, 2, |_| panic!("stale prose must not be mapped")));
        assert_eq!(
            store.spans(),
            &[(0..5, Category::Nouns), (6..11, Category::Verbs)]
        );
        assert!(store.apply(&mut receive(&worker), 2, std::convert::identity));
        assert_eq!(
            store.spans(),
            &[(0..5, Category::Nouns), (6..12, Category::Verbs)]
        );

        worker.request(request(3, "123.")).unwrap();
        assert!(store.apply(&mut receive(&worker), 3, std::convert::identity));
        assert!(
            store.spans().is_empty(),
            "a current empty answer removes old colours"
        );
    }

    #[test]
    fn a_paragraph_wanting_both_answers_with_both_sets_under_one_generation() {
        let mut worker = Worker::default();
        worker
            .request(wanting(5, "Alice basically reads.", BOTH))
            .unwrap();
        let mut result = receive(&worker);
        assert_eq!((result.generation, result.paragraph), (5, 0));
        assert_eq!(result.lists, [(6..15, List::Fillers)]);
        assert!(
            result.categories.contains(&(0..5, Category::Nouns)),
            "the Category set answers the same paragraph: {:?}",
            result.categories
        );

        // One answer, two stores: each takes its own kind and leaves the other.
        let mut colours = SpanStore::<Category>::default();
        let mut struck = SpanStore::<List>::default();
        assert!(colours.apply(&mut result, 5, std::convert::identity));
        assert!(struck.apply(&mut result, 5, std::convert::identity));
        assert!(colours.spans().contains(&(0..5, Category::Nouns)));
        assert_eq!(struck.spans(), &[(6..15, List::Fillers)]);
    }

    #[test]
    fn syntax_alone_leaves_the_list_set_empty_and_never_reads_the_lists() {
        let mut lists = None;
        let (categories, struck) = answer("Alice basically reads.", SYNTAX, &mut lists);
        assert!(!categories.is_empty());
        assert!(struck.is_empty());
        assert!(
            lists.is_none(),
            "a writer who never turns Style check on never pays for its files"
        );
    }

    #[test]
    fn style_alone_leaves_the_category_set_empty() {
        let mut worker = Worker::default();
        worker
            .request(wanting(1, "Alice basically reads.", STYLE))
            .unwrap();
        let result = receive(&worker);
        assert_eq!(result.lists, [(6..15, List::Fillers)]);
        assert!(result.categories.is_empty());
    }

    #[test]
    fn the_lists_are_compiled_once_however_many_style_requests_follow() {
        // Seeded with a list of the test's own: were they compiled again, the
        // shipped files would answer and `basically` would be struck too.
        let mut notes = Vec::new();
        let mut lists = Some(Lists::compile([(List::Fillers, "zzz\n")], &mut notes));
        for _ in 0..2 {
            let (_, struck) = answer("zzz basically", STYLE, &mut lists);
            assert_eq!(struck, [(0..3, List::Fillers)]);
        }
        assert!(notes.is_empty());
    }

    #[test]
    fn viewport_paragraphs_arrive_first_in_their_given_order() {
        let mut worker = Worker::default();
        worker
            .request(Request {
                generation: 7,
                wanted: SYNTAX,
                paragraphs: [4, 1, 3, 0, 2]
                    .into_iter()
                    .map(|index| Paragraph {
                        index,
                        prose: "Alice reads.".into(),
                    })
                    .collect(),
                viewport: 1..4,
            })
            .unwrap();
        for paragraph in [1, 3, 2, 4, 0] {
            let result = receive(&worker);
            assert_eq!((result.generation, result.paragraph), (7, paragraph));
        }
        assert_eq!(worker.try_recv(), Err(TryRecvError::Empty));
    }

    #[test]
    fn first_request_starts_one_thread_and_the_next_reuses_it() {
        let mut worker = Worker::default();
        assert!(!worker.is_running());
        assert_eq!(worker.try_recv(), Err(TryRecvError::Empty));
        worker.request(request(0, "Alice reads.")).unwrap();
        receive(&worker);
        assert!(worker.is_running());
        let first = worker.running.as_ref().unwrap().thread.thread().id();
        worker.request(request(1, "Alice walks.")).unwrap();
        receive(&worker);
        assert_eq!(worker.running.as_ref().unwrap().thread.thread().id(), first);
    }
}
