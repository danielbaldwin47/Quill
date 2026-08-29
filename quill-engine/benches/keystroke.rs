//! What one keystroke costs the engine, at two Document sizes.
//!
//! Informational rather than a test, as the Markup spec says: `cargo test` has
//! the correctness, and the Gate's own number is `tools/gate bench`, which
//! measures the whole path from a real key to a presented frame. What this
//! answers is the one question that path cannot separate out — whether the
//! block re-parse and the flatten under it are **flat across Document size**.
//! Two sizes an order of magnitude apart, and the shape of the pair is the
//! answer; the absolute numbers are this machine's.
//!
//! Run it with `cargo bench -p quill-engine`. No harness and no dependency:
//! the whole of it is [`Instant`] around the call the keystroke path makes,
//! because a criterion run is a minute and a tenth of a millisecond is not a
//! number that needs one.

use std::hint::black_box;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use quill_engine::document::Document;

/// How many keystrokes each measurement is the middle of.
const KEYS: usize = 200;

fn main() {
    println!("keystroke: the block re-parse, the flatten and the splice, per key");
    for words in [1_000, 55_000] {
        let path = write(words);
        // Two carets. At the end there is nothing under the edit to move, so
        // the number is the re-parse and the flatten alone. In the middle every
        // block, span and run below the caret is rebased by the byte delta —
        // an integer add rather than a parse, but one per entry, and half a
        // Document of them. Whether *that* is flat is the question a bench at
        // one caret cannot answer.
        for (caret, at) in [("end", None), ("middle", Some(()))] {
            let mut doc = Document::open(&path).expect("the bench writes its own Document");
            let bytes = doc.text().len();
            let cursor = |doc: &Document| {
                if at.is_some() {
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
        std::fs::remove_file(&path).ok();
    }
}

/// A Document of about `words` words, on disk, and where it landed.
///
/// Prose in the shape the Editor is for: paragraphs with Markup in them, so
/// that the re-parse has spans to find and the flatten has runs to make.
fn write(words: usize) -> PathBuf {
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
