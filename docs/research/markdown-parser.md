# Which Rust Markdown parser gives token spans for inline styling?

Research for [#4](https://github.com/danielbaldwin47/Quill/issues/4). Blocks [#10](https://github.com/danielbaldwin47/Quill/issues/10) (Editor spike) and [#11](https://github.com/danielbaldwin47/Quill/issues/11) (architecture spec).

## Question

The Editor styles Markup inline: `#`, `*`, `[`, `](`, list bullets, code fences and `[^1]` are drawn dimmer than the prose they wrap, in the same glyph advance widths. That needs a source span for every token *including the delimiters*, over CommonMark + GFM + footnotes (`docs/adr/0002`). Which of `pulldown-cmark`, `comrak` and `markdown-rs` provides it; does re-parsing the whole Document on every keystroke fit the budget in `progress/latency-report.md`, or is a scoped strategy needed; and how do Preview and Export reuse the same parser?

## Recommendation

**`pulldown-cmark`, driven block-scoped in the Editor and whole-document in Preview and Export.**

Three findings drive this, and the first is the one that reframes the question.

**No parser gives delimiter-only spans.** All three report a span for the *construct* (which covers its delimiters) and separate spans for the *content* inside it. The delimiter bytes are the set difference, and Quill must compute it in every case. comrak has exactly one exception — `NodeTaskItem::symbol_sourcepos`, the span of the `x` in `[x]`. So span fidelity does not separate the candidates the way the ticket assumed; what separates them is the *shape* of what they hand back.

**pulldown-cmark hands back the right shape.** `into_offset_iter()` yields `(Event, Range<usize>)` — byte ranges into the source, from a pull event stream with no tree. That is three things Quill needs at once: byte offsets the Editor can use directly (comrak reports 1-based line/byte-column, needing a line-start table; markdown-rs gives offsets but only hung off a tree); a token stream that `#11`'s annotator boundary — Syntax highlight, Style check and Spell check layered over the same tokens — can be defined against without inventing one; and a parser that can be restarted over a byte sub-range, which is what makes the scoped strategy below a few lines rather than a rewrite.

**Full re-parse per keystroke is the wrong shape even where it fits.** It costs O(document) against an O(1) budget. It fits comfortably at 10k words and fails at 55k — the size Quill exists to serve.

Runner-up is **comrak**, on the strength of verified full-GFM conformance; take it if the spike finds pulldown-cmark's autolink gap (below) worse than the arena-per-parse cost. **markdown-rs is rejected**: the token stream that actually names delimiters is private.

## Comparison

| | `pulldown-cmark` 0.13.4 | `comrak` 0.54.0 | `markdown` (markdown-rs) 1.0.0 |
|---|---|---|---|
| **Span API** | `Parser::into_offset_iter()` → `(Event, Range<usize>)` [1] | `Ast::sourcepos: Sourcepos { start, end: LineColumn }` on every node [2] | `Node::position: Option<Position>` with `Point { line, column, offset }` [3] |
| **Unit** | **byte range into source** | 1-based line + **byte** column within the line (`sourcepos_chars` opt-in for char columns, 0.52.0) | 1-based line/column **and** 0-based byte offset |
| **To absolute byte offsets** | already there | build a line-start table, `line_starts[line-1] + column - 1` | already there |
| **Inline spans** | yes, every event | yes; "all known sourcepos issues" fixed in 0.47.0, 1 open sourcepos issue (#536, wikilinks) [4] | yes, every inline node |
| **Delimiter-only span** | no — derive by subtraction | no, **except** `NodeTaskItem::symbol_sourcepos` (the `[x]` symbol) [5] | no — derive by subtraction |
| **Output model** | pull event stream, no tree | arena-allocated AST (`typed-arena`) | mdast tree |
| **Lower-level token stream** | *is* the API | n/a | **private** — `mod event;` is not `pub`; #183: "we can't get the intermediate Events because they're private" [6] |
| **Tables / task lists / strikethrough** | `ENABLE_TABLES` / `ENABLE_TASKLISTS` / `ENABLE_STRIKETHROUGH` | yes | `gfm_table` / `gfm_task_list_item` / `gfm_strikethrough` |
| **Footnotes** | `ENABLE_FOOTNOTES`, GFM `[^id]` by default; `ENABLE_OLD_FOOTNOTES` for the legacy syntax | yes, plus inline footnotes | `gfm_footnote_definition`, `gfm_label_start_footnote` |
| **Front matter** | `ENABLE_YAML_STYLE_METADATA_BLOCKS` | yes | `frontmatter` |
| **GFM conformance** | **no full-GFM claim**; README targets "100% compliance with the CommonMark spec" plus named extras | **670/670** against `github/cmark-gfm`'s `spec.txt`; a port of cmark-gfm | claims "100% to CommonMark", "100% GFM", 2300+ tests |
| **Known extension gap** | **GFM bare-URL / `www.` autolinking absent** (#494); no sub-range for a link's URL/title (#441) or a fence info string (#271) [7] | inherits cmark-gfm bugs by design (its README says so) | open #218: quadratic parsing in GFM tables |
| **First-party speed numbers** | none published; README claims "a bare minimum of allocation and copying"; `bench/` holds pathological-input regression benches only | none vs. other crates; README: "Comrak is not and will not be the fastest: some alternatives do not construct an AST" | **none anywhere**; `benches/bench.rs` self-times `to_html` on its own readme |
| **License** | MIT | BSD-2-Clause | MIT |
| **GPL-3.0-or-later compatible** (`docs/adr/0003`) | yes | yes | yes |
| **Maturity** | 0.13.4, 2026-05-20; **rustdoc and mdBook depend on it** [8] | 0.54.0, 2026-07-12; used by crates.io's backend, `deno_doc` | 1.0.0, 2025-04-23; **no commits since**; 1.0.0 notes: "Nothing changed since the last alpha" |

## Benchmark: not run

**No Rust toolchain on this machine** — `pacman -Q rust cargo` reports neither installed, and there is no `~/.cargo`. Nothing here is a measured parse time, and this file does not pretend otherwise. Per the standard `progress/latency-report.md` sets, every number below is labelled *cited* or *arithmetic*.

The only first-party figure any of the three publishes is a comrak **self**-comparison (*cited*, comrak CHANGELOG 0.45.0, aarch64, hyperfine, stdin-to-stdout over the concatenated Pro Git book): 88.1 ± 1.9 ms at 0.44.0 → 67.0 ± 1.2 ms at 0.45.0. That is a whole process against an unstated corpus size, so it converts to nothing useful about a 53 KB Document and is recorded only to show the ground is bare. There is no published throughput number for pulldown-cmark and none at all for markdown-rs.

**This is the measurement the spike (#10) should take, because it is the one that decides the fallback.** The fixture is already in the repo: `shots/latency/doc10k.md`, 9,953 words / 54,898 bytes / 431 lines — though it currently contains no table, footnote or strikethrough, so a GFM section must be added before it can benchmark extension coverage honestly. Criterion, three inputs (2k / 10k / 55k words), two operations each: a whole-document parse, and a single-block re-parse after a one-character edit mid-document.

**Pass condition: the block-scoped re-parse is flat across all three document sizes and under 200 µs at 55k words.** Flatness is the property being bought; the absolute number is secondary.

## Why per-keystroke full re-parse does not fit

The bar (`progress/latency-report.md` §5, from REFERENCE §5.2/§5.3): **≤ 5 ms average and ≤ 16 ms worst case**, keystroke → committed frame. Quill is at **2.43 ms mean and 15.61 ms worst** on the 10,062-word Document. The mean has 2.57 ms of headroom; **the worst case has 0.39 ms.**

For scale, today's entire JS `input` handler — the mirror diff, the tokenizer *and* the decorators — costs **726 µs** per keystroke at 10k words (§7), and `app_cost` never exceeded 5.30 ms across all 9,618 keystrokes in §6's table.

The decisive number is not at 10k, though. It is §11's scaling:

| document | main thread per key | `to_commit` worst |
|---|---|---|
| 2,112 words | 2.40 ms | 11.29 |
| 10,062 words | 3.90 ms | 13.65 |
| 26,841 words | 7.18 ms | 15.25 |
| **53,684 words** | **11.69 ms** | **18.67 — over the bar** |

A 55k-word manuscript already misses the ≤ 16 ms worst case, and the reason is that too much per-keystroke work is proportional to document length. **A full re-parse is exactly that kind of work, and it is the kind Quill can least afford to add**: 53 KB at 10k words, 288 KB at 55k. A parser fast enough to be invisible at 10k is five to six times more expensive at 55k, on a keystroke that has already run out of budget — and comrak allocates a fresh arena AST for the whole Document each time, producing per-keystroke garbage proportional to the manuscript.

Two honest qualifications. First, the native app has no Chromium: §6 shows every worst-case miss is `scheduler_wait`, not application work, so the native Editor starts with materially more real budget than these numbers imply. Second, the argument above does not depend on that — it is about the *shape* of the cost, not its constant. A design whose per-keystroke cost grows with manuscript length fails at long-form length whatever the toolkit, and long-form is the product.

**So: block-scoped.** It is also what the app already does. `app/js/markup.js` carries the rule in its header — "no work per keystroke beyond re-tokenising the lines that actually changed" — and threads a per-line context (`inFence`, `inFront`) forward so a line can be tokenised alone.

## The scoped strategy

**Blocks, not lines.** Line-scoping is what `markup.js` does today and it is subtly wrong for CommonMark: a paragraph continuation line, a lazy blockquote, and a setext underline all change the meaning of the line *above*. A CommonMark block is the smallest unit that re-parses correctly in isolation.

1. Keep a block index: `(byte range, kind)` per top-level block, plus the container context entering it (open fence, front matter, list depth).
2. On an edit, find the block containing it. Widen to neighbours when the edit touches a blank line, a fence marker, a list marker or a setext underline — the four ways a block boundary moves.
3. Re-parse that byte slice with a fresh `Parser`. **Rebase every yielded `Range` by adding the slice's start offset** — this is the whole trick, and it works because `into_offset_iter()` returns plain byte ranges.
4. Splice the resulting style runs into the index; shift the byte ranges of all following blocks by the edit delta. An integer add, not a re-parse.
5. Derive Markup spans by subtracting the inner `Text` ranges from the enclosing container's range, per construct.

**One construct is genuinely non-local**: link reference definitions and footnote definitions can be written anywhere and referenced anywhere. Resolve those on an idle whole-document pass — the same deferred pattern §13 already proves correct — never on the keystroke path. **The Editor does not need resolution to style them**: dimming `[^1]` or `[text][ref]` is a lexical judgement, so the fallback never blocks a frame.

The one-off whole-document parse on open stays whole-document. It is a cold-start cost, inside the 370 ms budget, not a keystroke cost.

**Note for the spike**: `Event::Text` holds a `CowStr` that becomes owned or inlined wherever pulldown-cmark unescapes a `\*` or resolves an entity, so `text.len()` can differ from `range.len()`. The `Range` still names the original source bytes — which is what the Editor needs — but the Editor must style from the range and never assume the event's string is a byte-for-byte slice of the Document.

## How Preview and Export reuse it

One `quill-markdown` module owns the parser. It exposes the token stream over a byte range, with offsets absolute into the Document, and **one shared `Options` value** — the single most important detail here, because it is what guarantees the Editor never dims a `~~` that Preview renders literally.

- **Editor** — runs it block-scoped and turns events into style runs. The annotators of `#11` (Syntax highlight, Style check, Spell check) subscribe to the same stream filtered to `Text` events, which is precisely how they see prose and never Markup characters: the offset iterator separates the two for free, so no annotator needs its own notion of what a token is.
- **Preview** — same parser, same `Options`, whole document, events fed to `pulldown_cmark::html::push_html`. Preview reuses the *parser*, not the Editor's block index; it re-renders on idle, not per keystroke.
- **Export** — HTML export is Preview's HTML with a print stylesheet; PDF is that HTML through the toolkit's paginate/print path. **The Markdown copy is the file's bytes, verbatim** — `docs/adr/0002` makes the file the only source of truth, so there is nothing to re-serialize. That removes the one capability comrak has that pulldown-cmark lacks (`format_commonmark`), and with it the round-trip fidelity risk a re-serializer would introduce.

## Known gap this decision accepts

pulldown-cmark does not do GFM's bare-URL/`www.` autolinking (#494), and `app/js/markup.js` does it today — the Parity oracle will show it. Quill runs its own linkifier over `Text` event ranges, which is exactly what `markup.js` already does and is cheap given the offsets. It must run in `quill-markdown`, once, so the Editor, Preview and Export agree; a linkifier applied in the Editor alone would be a parity bug in the other direction. If the spike finds this unpleasant, comrak is the fallback and full GFM conformance is what it buys.

## Sources

Primary sources only — docs.rs, crate source, crates.io metadata, repository issue threads.

1. https://docs.rs/pulldown-cmark/latest/pulldown_cmark/struct.Parser.html · https://docs.rs/pulldown-cmark/latest/pulldown_cmark/struct.OffsetIter.html
2. https://docs.rs/comrak/latest/comrak/nodes/struct.Ast.html · https://docs.rs/comrak/latest/comrak/nodes/struct.Sourcepos.html · https://docs.rs/comrak/latest/comrak/nodes/struct.LineColumn.html
3. https://raw.githubusercontent.com/wooorm/markdown-rs/main/src/mdast.rs · https://raw.githubusercontent.com/wooorm/markdown-rs/main/src/unist.rs
4. https://raw.githubusercontent.com/kivikakk/comrak/main/CHANGELOG.md (0.47.0) · https://github.com/kivikakk/comrak/issues/536
5. https://docs.rs/comrak/latest/comrak/nodes/struct.NodeTaskItem.html
6. https://raw.githubusercontent.com/wooorm/markdown-rs/main/src/lib.rs · https://github.com/wooorm/markdown-rs/issues/183 · https://github.com/wooorm/markdown-rs/issues/32
7. https://github.com/pulldown-cmark/pulldown-cmark/issues/494 · https://github.com/pulldown-cmark/pulldown-cmark/issues/441 · https://github.com/pulldown-cmark/pulldown-cmark/issues/271
8. https://github.com/rust-lang/mdBook/blob/master/Cargo.toml · https://github.com/rust-lang/rust/pull/113440

Also: https://docs.rs/pulldown-cmark/latest/pulldown_cmark/struct.Options.html · https://docs.rs/pulldown-cmark/latest/pulldown_cmark/enum.CowStr.html · https://github.com/pulldown-cmark/pulldown-cmark/blob/main/README.md · https://raw.githubusercontent.com/kivikakk/comrak/main/README.md · https://raw.githubusercontent.com/kivikakk/comrak/main/COPYING · https://raw.githubusercontent.com/wooorm/markdown-rs/main/src/configuration.rs · https://raw.githubusercontent.com/wooorm/markdown-rs/main/readme.md · https://github.com/wooorm/markdown-rs/issues/218 · https://crates.io/api/v1/crates/pulldown-cmark · https://crates.io/api/v1/crates/comrak · https://crates.io/api/v1/crates/markdown
