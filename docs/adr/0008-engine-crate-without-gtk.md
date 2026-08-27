# The engine is a crate that cannot see GTK

Native Quill is a Cargo workspace of two crates: `quill-engine` (text model, Markdown token stream,
Annotators, Library index, settings, Templates, the Pango/cairo renderer) and `quill` (the GTK
application). `quill-engine` has no dependency on the `gtk` crate, so the engine/UI boundary is held
by Cargo rather than by convention, and `cargo test -p quill-engine` is the headless commit-tier test
the Gate requires. Settled in [#11](https://github.com/danielbaldwin47/Quill/issues/11).

## Considered options

One crate with modules is simpler but leaves the boundary as a rule an agent can cross without
noticing, and the owner, who does not read Rust, cannot check it. A third crate for the parser
(`quill-markdown`, as the parser research sketched) adds a `Cargo.toml` for a module with one consumer.

## Consequences

The engine may depend on `pango` and `cairo` (Preview and PDF are layout, not widgets) and on the
system libraries it wraps (enchant); everything that needs a window, a buffer or an event lives in
`quill`. Any type both crates share is defined in the engine.
