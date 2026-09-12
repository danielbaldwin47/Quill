//! The shared asynchronous Annotator worker and its generation-checked spans.
//!
//! The app owns the debounce timer and drains results on idle. This module
//! owns one lazy thread per Worker, which runs Syntax highlight, Style check
//! and Spell check. Dropping the Worker closes both channels, so the thread
//! stops without making the main loop join it.
//!
//! A request names the Annotators it wants, and a paragraph is sent once and
//! answered once whichever are on: its answer carries a Category set, a List
//! set and a Misspelling set together, the set of an Annotator that was not
//! wanted left empty. The sets are kept apart by one [`SpanStore`] each,
//! generic over the span's kind, so a store applies its own set of an answer
//! and leaves the others.
//!
//! Spell check's dictionary is the one piece of the thread's state the main
//! thread reads: the [`Checker`] handle, which the thread fills at first use
//! and locks per paragraph, and the main thread locks for suggestions. Add,
//! Ignore and a language change reach the thread as an [`Edit`], down the
//! channel requests take, so each is applied before the requests sent after it.

use std::io;
use std::ops::Range;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::pos::{self, Category};
use crate::spell::{self, Enchant, Misspelling, SpellChecker};
use crate::style::{self, List, Lists};

/// Quiet time after the last keystroke before the app submits changed prose.
pub const DEBOUNCE: Duration = Duration::from_millis(100);

/// Ascending, non-overlapping spans of one kind over a paragraph's prose.
pub type Spans<K> = Vec<(Range<usize>, K)>;

/// The Spell check dictionary the worker thread and the main thread share.
///
/// Empty until the thread's first use of a language, and empty for none. The
/// thread holds the lock for one paragraph at a time, never across a request,
/// so a main-thread `suggest` waits for one paragraph at most.
pub type Checker = Arc<Mutex<Option<Box<dyn SpellChecker>>>>;

/// One changed paragraph, stripped to the parser's prose stream by the caller.
#[derive(Debug)]
pub struct Paragraph {
    /// Its paragraph index in this Document generation.
    pub index: usize,
    /// Prose only; result offsets address this string, not Markdown source.
    pub prose: String,
}

/// Which Annotators a request wants; a request wanting none is not sent.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Annotators {
    /// Syntax highlight: a Category per word.
    pub syntax: bool,
    /// Style check: a List per struck phrase.
    pub style: bool,
    /// Spell check: a Misspelling per word the dictionary does not hold.
    pub spell: bool,
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

/// A change to Spell check's dictionary, applied on the worker thread before
/// any request sent after it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Edit {
    /// Adds a word to the Personal dictionary, which outlives the process.
    Add(String),
    /// Holds a word until the process ends or the language changes, and
    /// writes nothing.
    Ignore(String),
    /// The exact tag to check against, resolved on the main thread, or `None`
    /// for the "no dictionary" state. The dictionary loads at its first use.
    Language(Option<String>),
}

/// One paragraph's answer, whose byte ranges refer to its requested prose.
///
/// All three span sets arrive together under the one generation; the set of
/// an Annotator the request did not want is empty.
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
    /// Ascending, non-overlapping Misspelling spans, one per misspelled word.
    pub misspellings: Spans<Misspelling>,
}

/// What travels down the one channel, so an edit and a request keep their order.
enum Message {
    Request(Request),
    Edit(Edit),
}

struct Running {
    messages: Sender<Message>,
    results: Receiver<ParagraphResult>,
    thread: JoinHandle<()>,
}

/// One lazy thread, reused for every request over this Document's lifetime.
#[derive(Default)]
pub struct Worker {
    running: Option<Running>,
    checker: Checker,
}

impl Worker {
    /// Queues changed prose, starting the thread on the first message.
    ///
    /// The tagger model loads on that thread's first nonempty paragraph.
    ///
    /// # Errors
    ///
    /// Returns the spawn error if the thread cannot start, or `BrokenPipe`
    /// if a previously started worker stopped unexpectedly.
    pub fn request(&mut self, request: Request) -> io::Result<()> {
        self.send(Message::Request(request))
    }

    /// Queues a dictionary edit, applied before every request sent after it,
    /// starting the thread on the first message.
    ///
    /// # Errors
    ///
    /// As [`Worker::request`].
    pub fn edit(&mut self, edit: Edit) -> io::Result<()> {
        self.send(Message::Edit(edit))
    }

    /// The Spell check dictionary handle, shared with the thread from the
    /// Worker's creation; empty until a request wanting Spell check loads it.
    #[must_use]
    pub fn checker(&self) -> Checker {
        Arc::clone(&self.checker)
    }

    fn send(&mut self, message: Message) -> io::Result<()> {
        if self.running.is_none() {
            let (messages, incoming) = mpsc::channel();
            let (outgoing, results) = mpsc::channel();
            let checker = Arc::clone(&self.checker);
            let thread = thread::Builder::new()
                .name("quill-annotators".into())
                .spawn(move || run(incoming, outgoing, checker))?;
            self.running = Some(Running {
                messages,
                results,
                thread,
            });
        }
        self.running
            .as_ref()
            .expect("the worker was started above")
            .messages
            .send(message)
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

fn run(messages: Receiver<Message>, results: Sender<ParagraphResult>, checker: Checker) {
    // The lists are read and compiled on the first request that wants Style
    // check and kept for the thread's life, so a session that never turns it
    // on never touches the files and a keystroke never pays for them. The
    // dictionary is loaded the same way, on its first use.
    let mut lists = None;
    let mut dictionary = Dictionary {
        checker,
        language: None,
        loaded: false,
    };
    for message in messages {
        let request = match message {
            Message::Request(request) => request,
            Message::Edit(edit) => {
                dictionary.edit(edit);
                continue;
            }
        };
        if request.wanted.spell {
            dictionary.load();
        }
        // Two passes over only the requested paragraphs, no sort or document
        // scan. Each paragraph is tagged once; the order test pins both groups.
        for visible in [true, false] {
            for paragraph in &request.paragraphs {
                if request.viewport.contains(&paragraph.index) != visible {
                    continue;
                }
                let result = ParagraphResult {
                    generation: request.generation,
                    paragraph: paragraph.index,
                    ..answer(
                        &paragraph.prose,
                        request.wanted,
                        &mut lists,
                        &dictionary.checker,
                    )
                };
                if results.send(result).is_err() {
                    return;
                }
            }
        }
    }
}

/// The thread's side of the [`Checker`]: the language it was last handed and
/// whether the handle holds that language's dictionary yet.
struct Dictionary {
    checker: Checker,
    language: Option<String>,
    loaded: bool,
}

impl Dictionary {
    fn edit(&mut self, edit: Edit) {
        match edit {
            Edit::Language(language) => {
                if language != self.language {
                    self.language = language;
                    self.loaded = false;
                    // Emptied now, so a suggestion never reads the old
                    // language; dropped after the guard is released.
                    let old = lock(&self.checker).take();
                    drop(old);
                }
            }
            Edit::Add(word) => {
                self.load();
                if let Some(checker) = lock(&self.checker).as_mut() {
                    checker.add(&word);
                }
            }
            Edit::Ignore(word) => {
                self.load();
                if let Some(checker) = lock(&self.checker).as_mut() {
                    checker.ignore(&word);
                }
            }
        }
    }

    /// Fills the handle for the current language on its first use. The load
    /// runs outside the lock, so a main-thread `suggest` never waits for it.
    fn load(&mut self) {
        if self.loaded {
            return;
        }
        self.loaded = true;
        let loaded = self
            .language
            .as_deref()
            .and_then(Enchant::new)
            .map(|dictionary| Box::new(dictionary) as Box<dyn SpellChecker>);
        *lock(&self.checker) = loaded;
    }
}

/// The handle's guard; a panic under the lock leaves the dictionary usable.
fn lock(checker: &Checker) -> MutexGuard<'_, Option<Box<dyn SpellChecker>>> {
    checker.lock().unwrap_or_else(PoisonError::into_inner)
}

/// One paragraph's three span sets, compiling the lists if this is their
/// first ask. Separate from `run` so a test can watch `lists` across two
/// answers.
fn answer(
    prose: &str,
    wanted: Annotators,
    lists: &mut Option<Lists>,
    checker: &Checker,
) -> ParagraphResult {
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
    let misspellings = if wanted.spell {
        // Locked for this paragraph alone; an empty handle marks nothing.
        lock(checker)
            .as_deref()
            .map_or_else(Vec::new, |checker| spell::misspelled(prose, checker))
            .into_iter()
            .map(|word| (word, Misspelling))
            .collect()
    } else {
        Vec::new()
    };
    ParagraphResult {
        categories,
        lists: struck,
        misspellings,
        ..ParagraphResult::default()
    }
}

/// A span kind an answer carries: [`Category`], [`List`] or [`Misspelling`].
/// Implemented for those three only, so a [`SpanStore`] takes its own set and
/// leaves the others.
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

impl Kind for Misspelling {
    fn take(result: &mut ParagraphResult) -> Spans<Self> {
        std::mem::take(&mut result.misspellings)
    }
}

/// One paragraph's last accepted spans of one kind, kept while a replacement
/// is pending.
///
/// The receiver owns paragraph indexing and chooses the stored coordinates
/// through `apply`'s mapping. The app stores source-relative spans here and
/// rebases them when its paragraph is edited. A paragraph answering several
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
    /// Takes this store's kind out of the answer, leaving the other kinds for
    /// their own stores.
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
        spell: false,
    };
    const STYLE: Annotators = Annotators {
        syntax: false,
        style: true,
        spell: false,
    };
    const BOTH: Annotators = Annotators {
        syntax: true,
        style: true,
        spell: false,
    };
    const SPELL: Annotators = Annotators {
        syntax: false,
        style: false,
        spell: true,
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
        let result = answer(
            "Alice basically reads.",
            SYNTAX,
            &mut lists,
            &Checker::default(),
        );
        assert!(!result.categories.is_empty());
        assert!(result.lists.is_empty());
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
        assert!(result.misspellings.is_empty());
    }

    #[test]
    fn the_lists_are_compiled_once_however_many_style_requests_follow() {
        // Seeded with a list of the test's own: were they compiled again, the
        // shipped files would answer and `basically` would be struck too.
        let mut notes = Vec::new();
        let mut lists = Some(Lists::compile([(List::Fillers, "zzz\n")], &mut notes));
        for _ in 0..2 {
            let result = answer("zzz basically", STYLE, &mut lists, &Checker::default());
            assert_eq!(result.lists, [(0..3, List::Fillers)]);
        }
        assert!(notes.is_empty());
    }

    #[test]
    fn spell_with_no_language_answers_an_empty_set_and_loads_nothing() {
        let mut worker = Worker::default();
        worker.edit(Edit::Language(None)).unwrap();
        worker.request(wanting(2, "recieved", SPELL)).unwrap();
        let result = receive(&worker);
        assert_eq!(
            result,
            ParagraphResult {
                generation: 2,
                ..ParagraphResult::default()
            }
        );
        assert!(lock(&worker.checker()).is_none());
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
