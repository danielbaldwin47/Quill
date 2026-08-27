//! The Library: the folder tree Quill has been pointed at, plus recents.
//!
//! In memory only — walked on launch, watched with `notify`, never persisted as
//! an index, so nothing is left in the writer's folder and there is no stale
//! index to delete. Config remembers the location; state remembers recents and
//! per-Document caret positions. The Library is shared by all windows.
