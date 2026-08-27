//! Settings: what the writer chose, and what the app observed.
//!
//! Config is one TOML file at `$XDG_CONFIG_HOME/quill/settings.toml`; state
//! lives under `$XDG_STATE_HOME/quill/`. Missing keys take defaults and unknown
//! keys are kept, so an older Quill never destroys a newer file. The file is
//! watched like a Document: a saved edit applies without a restart, and a line
//! that cannot be applied is logged once and skipped, never fatal.
