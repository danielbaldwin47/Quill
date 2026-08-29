//! Quill's engine: everything that can be tested without a display.
//!
//! One module per concept in `docs/architecture.md`. This crate has no
//! dependency on `gtk` (ADR 0008, `docs/adr/0008-engine-crate-without-gtk.md`),
//! so the engine/UI boundary is held by Cargo rather than by convention and
//! `cargo test -p quill-engine` is the headless commit-tier test the Gate
//! requires. Every type both crates share is defined here.
//!
//! Most modules below are still stubs: they name the concept and the shape the
//! feature tickets fill in, so a later agent adds to a place that already
//! exists rather than inventing one. [`data`] is not one of them: every other
//! module reads the files Quill ships through it.

#![warn(missing_docs)]

pub mod annotate;
pub mod data;
pub mod document;
pub mod focus;
pub mod library;
pub mod markdown;
pub mod outline;
pub mod pos;
pub mod render;
pub mod settings;
pub mod spell;
pub mod stats;
pub mod style;
pub mod template;
pub mod theme;
pub mod typography;
