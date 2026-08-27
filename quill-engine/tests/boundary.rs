//! The engine/UI boundary, held by Cargo (ADR 0008).
//!
//! `quill-engine` must never depend on `gtk`. That is what makes
//! `cargo test -p quill-engine` runnable with no display attached, and it is
//! the one architectural rule the owner, who does not read Rust, can see
//! enforced without opening the code. This test reads the manifest on every
//! commit so the rule cannot be crossed by accident.

use std::fs;

/// Dependency names and renamed packages that would pull GTK into the engine.
const FORBIDDEN: [&str; 5] = ["gtk", "gtk4", "gtk4-sys", "gdk4", "gsk4"];

#[test]
fn the_engine_manifest_does_not_depend_on_gtk() {
    let manifest = manifest();
    for table in ["dependencies", "dev-dependencies", "build-dependencies"] {
        for (name, spec) in deps(&manifest, table) {
            assert!(
                !FORBIDDEN.contains(&name.as_str()),
                "quill-engine's [{table}] lists `{name}`: the engine cannot see GTK (ADR 0008)"
            );
            if let Some(package) = spec.get("package").and_then(toml::Value::as_str) {
                assert!(
                    !FORBIDDEN.contains(&package),
                    "quill-engine's [{table}] lists `{name}`, which is the `{package}` \
                     crate renamed: the engine cannot see GTK (ADR 0008)"
                );
            }
        }
    }
}

#[test]
fn the_engine_may_still_depend_on_pango_and_cairo() {
    // Layout is not a widget: Preview and PDF need Pango and cairo in the
    // engine. This test states the boundary's other half, so a future reader
    // does not "fix" the one above by banning every GNOME crate.
    let manifest = manifest();
    let names: Vec<String> = deps(&manifest, "dependencies")
        .map(|(name, _)| name)
        .collect();
    for allowed in ["pango", "cairo-rs"] {
        assert!(
            names.iter().any(|name| name == allowed),
            "quill-engine should still depend on `{allowed}`; it has {names:?}"
        );
    }
}

fn manifest() -> toml::Table {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml");
    fs::read_to_string(path)
        .expect("quill-engine/Cargo.toml is readable")
        .parse()
        .expect("quill-engine/Cargo.toml is valid TOML")
}

/// The `(name, spec)` pairs of one dependency table, with bare version strings
/// presented as empty tables so callers have one shape to read.
fn deps(manifest: &toml::Table, table: &str) -> impl Iterator<Item = (String, toml::Table)> {
    manifest
        .get(table)
        .and_then(toml::Value::as_table)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(|(name, spec)| {
            let spec = spec.as_table().cloned().unwrap_or_default();
            (name, spec)
        })
}
