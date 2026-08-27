//! Syntax highlight: a part-of-speech tag per word.
//!
//! The Annotator that colours nouns, verbs, adjectives, adverbs and
//! conjunctions to show sentence texture — Quill's Syntax highlight, not code
//! highlighting. It reads the prose stream and emits one UPOS tag per word.
//! The tagger model loads on the worker thread at first use.
