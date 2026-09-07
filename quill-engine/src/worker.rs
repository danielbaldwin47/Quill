//! The shared asynchronous Annotator worker and its generation-checked spans.
//!
//! The app owns the debounce timer and drains results on idle. This module
//! owns one lazy thread per Worker; Syntax highlight runs here now, and Style
//! check and Spell check join it when they land. Dropping the Worker closes
//! both channels, so the thread stops without making the main loop join it.

use std::io;
use std::ops::Range;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::pos::{self, Category};

/// Quiet time after the last keystroke before the app submits changed prose.
pub const DEBOUNCE: Duration = Duration::from_millis(100);

/// One changed paragraph, stripped to the parser's prose stream by the caller.
#[derive(Debug)]
pub struct Paragraph {
    /// Its paragraph index in this Document generation.
    pub index: usize,
    /// Prose only; result offsets address this string, not Markdown source.
    pub prose: String,
}

/// Changed paragraphs and the viewport they were captured with.
#[derive(Debug)]
pub struct Request {
    /// The Document generation at capture, echoed unchanged in every result.
    pub generation: u64,
    /// Each changed paragraph once; their order is kept within each priority.
    pub paragraphs: Vec<Paragraph>,
    /// Paragraph indices currently visible, answered before the rest.
    pub viewport: Range<usize>,
}

/// One paragraph's answer, whose byte ranges refer to its requested prose.
#[derive(Debug, PartialEq, Eq)]
pub struct ParagraphResult {
    /// The Document generation at capture.
    pub generation: u64,
    /// The paragraph index from the request.
    pub paragraph: usize,
    /// Ascending, non-overlapping Category spans, one per coloured word.
    pub spans: Vec<(Range<usize>, Category)>,
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
    for request in requests {
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
                    spans: pos::categories(&paragraph.prose),
                };
                if results.send(result).is_err() {
                    return;
                }
            }
        }
    }
}

/// One paragraph's last accepted spans, kept while a replacement is pending.
///
/// The receiver owns paragraph indexing and chooses the stored coordinates
/// through `apply`'s mapping. The app stores source-relative spans here and
/// rebases them when its paragraph is edited.
#[derive(Default)]
pub struct SpanStore {
    spans: Vec<(Range<usize>, Category)>,
}

impl SpanStore {
    /// Replaces this paragraph's spans only when its generation is current.
    /// Returns whether the result was accepted; stale answers change nothing.
    /// Mapping runs only for an accepted answer, against its captured prose.
    pub fn apply(
        &mut self,
        result: ParagraphResult,
        generation: u64,
        map: impl FnOnce(Vec<(Range<usize>, Category)>) -> Vec<(Range<usize>, Category)>,
    ) -> bool {
        if result.generation != generation {
            return false;
        }
        self.spans = map(result.spans);
        true
    }

    /// The last accepted spans, or none while a paragraph awaits its first.
    #[must_use]
    pub fn spans(&self) -> &[(Range<usize>, Category)] {
        &self.spans
    }

    /// Retained spans for source rebasing; preserve ascending, disjoint ranges.
    pub fn spans_mut(&mut self) -> &mut Vec<(Range<usize>, Category)> {
        &mut self.spans
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(generation: u64, prose: &str) -> Request {
        Request {
            generation,
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
        let mut store = SpanStore::default();
        worker.request(request(0, "Alice reads.")).unwrap();
        assert!(store.apply(receive(&worker), 0, std::convert::identity));
        assert_eq!(
            store.spans(),
            &[(0..5, Category::Nouns), (6..11, Category::Verbs)]
        );

        worker.request(request(1, "Alice walks.")).unwrap();
        worker.request(request(2, "Alice sleeps.")).unwrap();
        let stale = receive(&worker);
        assert_eq!(stale.generation, 1);
        assert!(!store.apply(stale, 2, |_| panic!("stale prose must not be mapped")));
        assert_eq!(
            store.spans(),
            &[(0..5, Category::Nouns), (6..11, Category::Verbs)]
        );
        assert!(store.apply(receive(&worker), 2, std::convert::identity));
        assert_eq!(
            store.spans(),
            &[(0..5, Category::Nouns), (6..12, Category::Verbs)]
        );

        worker.request(request(3, "123.")).unwrap();
        assert!(store.apply(receive(&worker), 3, std::convert::identity));
        assert!(
            store.spans().is_empty(),
            "a current empty answer removes old colours"
        );
    }

    #[test]
    fn viewport_paragraphs_arrive_first_in_their_given_order() {
        let mut worker = Worker::default();
        worker
            .request(Request {
                generation: 7,
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
