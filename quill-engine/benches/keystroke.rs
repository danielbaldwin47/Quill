//! What one keystroke costs the engine, at three Document sizes, and what the
//! cold parse of the bench's 10,000-word Document costs before the first frame.
//!
//! Informational rather than a test, as the Markup spec says: `cargo test` has
//! the correctness, and the Gate's own number is `tools/gate bench`, which
//! measures the whole path from a real key to a presented frame. What this
//! answers is the one question that path cannot separate out — whether the
//! block re-parse and the flatten under it are **flat across Document size**.
//! Three sizes, the largest fifty-five times the smallest, and the shape of the
//! series is the answer; the absolute numbers are this machine's.
//!
//! Run it with `cargo bench -p quill-engine`. No harness and no dependency:
//! the whole of it is [`Instant`] around the call the keystroke path makes,
//! because a criterion run is a minute and a tenth of a millisecond is not a
//! number that needs one.

use std::hint::black_box;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use quill_engine::annotate::{markup, paint};
use quill_engine::document::Document;
use quill_engine::focus::{Focus, tiers, tiers_by_line};
use quill_engine::settings::FocusScope;
use quill_engine::theme::{Colours, Scheme};

/// How many keystrokes each measurement is the middle of.
const KEYS: usize = 200;

fn main() {
    println!("keystroke: the block re-parse, the flatten and the splice, per key");
    for words in [1_000, 10_000, 55_000] {
        let path = draft_of(words);
        // Two carets. At the end there is nothing under the edit to move, so
        // the number is the re-parse and the flatten alone. In the middle every
        // block, span and run below the caret is rebased by the byte delta —
        // an integer add rather than a parse, but one per entry, and half a
        // Document of them. Whether *that* is flat is the question a bench at
        // one caret cannot answer.
        for (caret, halfway) in [("end", false), ("middle", true)] {
            let mut doc = Document::open(&path).expect("the bench writes its own Document");
            let bytes = doc.text().len();
            let cursor = |doc: &Document| {
                if halfway {
                    doc.line_bytes(doc.place(doc.text().len() / 2).line).end - 1
                } else {
                    doc.text().len()
                }
            };

            // Warm the allocator and the parser's own caches, so that the first
            // key of the measurement is not the only slow one in it.
            for _ in 0..KEYS {
                let at = cursor(&doc);
                black_box(doc.insert(at, "x"));
            }

            let mut each = Vec::with_capacity(KEYS);
            for _ in 0..KEYS {
                let at = cursor(&doc);
                let started = Instant::now();
                black_box(doc.insert(at, "x"));
                each.push(started.elapsed());
            }
            each.sort_unstable();

            println!(
                "  {words:>6} words ({bytes:>7} bytes), caret at the {caret:<6}: \
                 median {:>7.1} µs, worst {:>7.1} µs, mean {:>7.1} µs",
                micros(each[each.len() / 2]),
                micros(each[each.len() - 1]),
                micros(each.iter().sum::<Duration>() / u32::try_from(each.len()).unwrap_or(1)),
            );
        }
        focus_flatten(words, &path);
        std::fs::remove_file(&path).ok();
    }
    cold_parse();
}

/// What the whole-Document parse costs before the first frame.
///
/// Cold start keeps this parse on purpose — the block index it builds is what
/// makes the first keystroke as flat as the thousandth — so its cost is part
/// of the ≤ 250 ms the Gate holds cold start to, and this is the number to
/// read when that budget moves. The Document is `tools/gate bench`'s own,
/// `dev/shots/latency/doc10k.md`, so that the figure is the one the harness pays;
/// a checkout without it prints so rather than measuring something else.
fn cold_parse() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../dev/shots/latency/doc10k.md");
    let Ok(text) = std::fs::read_to_string(&path) else {
        println!("cold parse: dev/shots/latency/doc10k.md is not in this checkout");
        return;
    };
    let words = text.split_whitespace().count();
    let bytes = text.len();

    // Ten opens, not two hundred: a cold start happens once per launch, and the
    // spread of ten says whether the median is to be believed.
    const OPENS: usize = 10;
    let mut each = Vec::with_capacity(OPENS);
    for _ in 0..OPENS {
        let started = Instant::now();
        black_box(Document::open(&path).expect("the harness's own Document opens"));
        each.push(started.elapsed());
    }
    each.sort_unstable();

    println!("cold parse: Document::open on dev/shots/latency/doc10k.md, before the first frame");
    println!(
        "  {words:>6} words ({bytes:>7} bytes), whole Document       : \
         median {:>7.1} µs, worst {:>7.1} µs, mean {:>7.1} µs",
        micros(each[each.len() / 2]),
        micros(each[each.len() - 1]),
        micros(each.iter().sum::<Duration>() / u32::try_from(each.len()).unwrap_or(1)),
    );
}

/// What Focus costs on top: the tiers for a caret, and the flattening painted
/// over them.
///
/// The second thing on the keystroke path that has to stay flat across Document
/// size, and it runs on every caret move as well as every key, so #117 has a
/// before number for it. [`tiers`] reads the caret's block alone and is flat by
/// construction; [`paint`] here paints the **whole** Document, which is the
/// pessimal shape and not the one the Editor will use — #113 paints the lines
/// it is showing. The gap between the two sizes is therefore the number to
/// read: it says what painting the whole page would cost if the Editor ever did.
///
/// [`tiers`]: quill_engine::focus::tiers
/// [`paint`]: quill_engine::annotate::paint
fn focus_flatten(words: usize, path: &Path) {
    let doc = Document::open(path).expect("the bench writes its own Document");
    let text = doc.text();
    let (bytes, spans) = (text.len(), markup(text));
    let colours = Colours::of(Scheme::Light);
    let focus = Focus::On(FocusScope::Sentence);

    // A caret that keeps moving, because a caret that sits still would measure
    // one set of tiers over and over and the writer's never does.
    let mut caret = bytes / 2;
    let step = |caret: &mut usize| {
        *caret = if *caret + 1 >= bytes {
            bytes / 2
        } else {
            *caret + 1
        };
        *caret
    };
    for _ in 0..KEYS {
        let at = step(&mut caret);
        black_box(tiers_by_line(&doc, &tiers(&doc, &(at..at), focus)));
    }

    let mut each = Vec::with_capacity(KEYS);
    for _ in 0..KEYS {
        let at = step(&mut caret);
        let started = Instant::now();
        let by_line = tiers_by_line(&doc, &tiers(&doc, &(at..at), focus));
        black_box(paint(&spans, bytes, &by_line, focus, &colours));
        each.push(started.elapsed());
    }
    each.sort_unstable();

    println!(
        "  {words:>6} words ({bytes:>7} bytes), Focus sentence, whole page: \
         median {:>7.1} µs, worst {:>7.1} µs, mean {:>7.1} µs",
        micros(each[each.len() / 2]),
        micros(each[each.len() - 1]),
        micros(each.iter().sum::<Duration>() / u32::try_from(each.len()).unwrap_or(1)),
    );
}

/// Writes a Document of about `words` words, and says where it landed.
///
/// Prose in the shape the Editor is for: paragraphs with Markup in them, so
/// that the re-parse has spans to find and the flatten has runs to make. It
/// goes to disk because [`Document::open`] is the only way in — the engine
/// builds a Document from a file, never from a string a caller is holding.
fn draft_of(words: usize) -> PathBuf {
    let para = "The lamp *was* lit at the head of the stair, and the keeper \
                wrote **nothing** of it in the log that night.\n\n";
    let each = para.split_whitespace().count();
    let mut text = String::from("# The Lighthouse\n\n");
    for _ in 0..words.div_ceil(each) {
        text.push_str(para);
    }
    let path = std::env::temp_dir().join(format!("quill-bench-{words}.md"));
    std::fs::write(&path, text).expect("writes to the temp directory");
    path
}

/// `span` as microseconds, to print.
fn micros(span: Duration) -> f64 {
    span.as_secs_f64() * 1_000_000.0
}
