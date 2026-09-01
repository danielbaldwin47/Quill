//! One module per concept, each saying which concept it is.
//!
//! The scaffold's promise is that a later agent fills a module in rather than
//! deciding where it goes. That only holds if the modules stay declared and
//! stay documented, so this test names every one and fails if one loses its
//! `//!` comment.

use std::fs;
use std::path::PathBuf;

/// The engine's concepts, in `docs/architecture.md`'s order: its sixteen
/// modules, then `data`, which the "Fonts and data files" section adds and
/// every other module reads its files through.
const MODULES: [&str; 17] = [
    "document",
    "markdown",
    "annotate",
    "focus",
    "library",
    "settings",
    "template",
    "render",
    "stats",
    "outline",
    "spell",
    "pos",
    "style",
    "typography",
    "theme",
    "commands",
    "data",
];

#[test]
fn every_module_exists_and_names_its_concept() {
    for module in MODULES {
        let source = fs::read_to_string(src().join(format!("{module}.rs")))
            .unwrap_or_else(|err| panic!("quill-engine/src/{module}.rs is readable: {err}"));
        let first = source.lines().next().unwrap_or_default();
        assert!(
            first.starts_with("//!"),
            "quill-engine/src/{module}.rs must open with a `//!` comment naming its concept, \
             not `{first}`"
        );
    }
}

#[test]
fn lib_declares_every_module() {
    let lib =
        fs::read_to_string(src().join("lib.rs")).expect("quill-engine/src/lib.rs is readable");
    for module in MODULES {
        assert!(
            lib.contains(&format!("pub mod {module};")),
            "quill-engine/src/lib.rs does not declare `pub mod {module};`"
        );
    }
}

fn src() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/src"))
}
