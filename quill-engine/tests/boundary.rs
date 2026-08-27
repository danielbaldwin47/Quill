//! The engine/UI boundary, held by Cargo (ADR 0008).
//!
//! `quill-engine` must never depend on `gtk`. That is what makes
//! `cargo test -p quill-engine` runnable with no display attached, and it is
//! the one architectural rule the owner, who does not read Rust, can see
//! enforced without opening the code.
//!
//! Two tests, because a manifest and a dependency graph fail differently. The
//! first reads the engine's manifest, which is where the rule would be broken
//! on purpose. The second walks `Cargo.lock` from `quill-engine` outwards,
//! which is where it would be broken by accident — through a renamed package,
//! a target-specific table or some other crate's dependency.

use std::collections::{HashMap, HashSet};
use std::fs;

/// Package names that would put GTK in the engine.
const FORBIDDEN: [&str; 5] = ["gtk", "gtk4", "gtk4-sys", "gdk4", "gsk4"];

/// The dependency tables a manifest can declare, at the top level and inside
/// each `[target.'cfg(...)']`.
const TABLES: [&str; 3] = ["dependencies", "dev-dependencies", "build-dependencies"];

#[test]
fn the_engine_manifest_does_not_depend_on_gtk() {
    for (table, name, spec) in declared_dependencies(&manifest()) {
        assert!(
            !FORBIDDEN.contains(&name.as_str()),
            "quill-engine's [{table}] lists `{name}`: the engine cannot see GTK (ADR 0008)"
        );
        if let Some(package) = spec.get("package").and_then(toml::Value::as_str) {
            assert!(
                !FORBIDDEN.contains(&package),
                "quill-engine's [{table}] lists `{name}`, which is the `{package}` crate \
                 renamed: the engine cannot see GTK (ADR 0008)"
            );
        }
    }
}

#[test]
fn nothing_the_engine_pulls_in_is_gtk() {
    let locked = locked_dependencies();
    let mut seen: HashSet<&str> = HashSet::new();
    let mut queue = vec!["quill-engine"];
    while let Some(package) = queue.pop() {
        assert!(
            !FORBIDDEN.contains(&package),
            "quill-engine reaches `{package}` through its dependencies: the engine cannot \
             see GTK (ADR 0008)"
        );
        for dependency in locked
            .get(package)
            .into_iter()
            .flatten()
            .map(String::as_str)
        {
            if seen.insert(dependency) {
                queue.push(dependency);
            }
        }
    }
    assert!(
        !seen.is_empty(),
        "quill-engine was not found in Cargo.lock, so this test proved nothing"
    );
}

fn manifest() -> toml::Table {
    read_toml(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"))
}

/// Every dependency the manifest declares, as `(table, name, spec)`, including
/// the `[target.'cfg(...)']` tables. A bare version string is presented as an
/// empty spec table so callers have one shape to read.
fn declared_dependencies(manifest: &toml::Table) -> Vec<(String, String, toml::Table)> {
    let mut declared = Vec::new();
    collect(manifest, "", &mut declared);
    if let Some(targets) = manifest.get("target").and_then(toml::Value::as_table) {
        for (cfg, tables) in targets {
            if let Some(tables) = tables.as_table() {
                collect(tables, &format!("target.'{cfg}'."), &mut declared);
            }
        }
    }
    declared
}

fn collect(tables: &toml::Table, prefix: &str, declared: &mut Vec<(String, String, toml::Table)>) {
    for table in TABLES {
        let Some(entries) = tables.get(table).and_then(toml::Value::as_table) else {
            continue;
        };
        for (name, spec) in entries {
            let spec = spec.as_table().cloned().unwrap_or_default();
            declared.push((format!("{prefix}{table}"), name.clone(), spec));
        }
    }
}

/// `Cargo.lock`'s resolved graph: package name to the names it depends on.
fn locked_dependencies() -> HashMap<String, Vec<String>> {
    let lock = read_toml(concat!(env!("CARGO_MANIFEST_DIR"), "/../Cargo.lock"));
    let packages = lock
        .get("package")
        .and_then(toml::Value::as_array)
        .expect("Cargo.lock lists packages");
    packages
        .iter()
        .filter_map(toml::Value::as_table)
        .filter_map(|package| {
            let name = package.get("name")?.as_str()?.to_owned();
            let dependencies = package
                .get("dependencies")
                .and_then(toml::Value::as_array)
                .map(|entries| {
                    entries
                        .iter()
                        .filter_map(toml::Value::as_str)
                        // An entry is `name`, or `name version`, or
                        // `name version (source)`.
                        .filter_map(|entry| entry.split_whitespace().next())
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default();
            Some((name, dependencies))
        })
        .collect()
}

fn read_toml(path: &str) -> toml::Table {
    fs::read_to_string(path)
        .unwrap_or_else(|err| panic!("{path} is readable: {err}"))
        .parse()
        .unwrap_or_else(|err| panic!("{path} is valid TOML: {err}"))
}
