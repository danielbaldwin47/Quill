//! What a window has to decide about the file behind its Document, without a
//! window to decide it in.
//!
//! Three questions come up wherever a Document meets the disk: where an
//! untitled Document's first save goes, whether a window closing has anything
//! to ask the writer first, and whether opening a file should point the Library
//! at the folder it came from. Each is a decision over plain values — the path
//! state ([`quill_engine::disk::OnDisk`]), the Locations, whether there is any
//! text — so each is a function here rather than a branch inside a signal
//! handler, and each is tested with no display attached.
//!
//! What the answers are *for* is [`crate::window`]: it flushes, prompts, opens
//! dialogs and shows Documents. The rules themselves are
//! `docs/architecture.md` § Documents and files and the File handling spec.

use std::path::{Path, PathBuf};

use quill_engine::disk::OnDisk;
use quill_engine::library::Library;

/// Where an untitled Document's first save goes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Where {
    /// Into this folder, under the name the first line derives.
    Folder(PathBuf),
    /// Nowhere Quill can name: the writer is asked.
    Ask,
}

/// Where the first save of an untitled Document lands.
///
/// The Library's first Location, because that is the folder a writer who has
/// pointed Quill at one is writing in; a writer who has pointed it at none is
/// asked, and so is one who asked to always be asked (`library.ask_where_to_save`).
///
/// `first` is the first Location rather than the row the sidebar has selected,
/// because there is no sidebar yet; the selected row is the sidebar ticket's
/// and goes in front of it here (#246).
pub fn first_save_folder(first: Option<&Path>, always_ask: bool) -> Where {
    match first {
        Some(folder) if !always_ask => Where::Folder(folder.to_path_buf()),
        _ => Where::Ask,
    }
}

/// What a window has to do before it can go.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Leaving {
    /// Nothing: there is no file and nothing anyone typed.
    Go,
    /// Write the Document out, and then go. No prompt: a writer who named a
    /// file once has already said what to do with what they type into it.
    Flush,
    /// Ask first — Save, Discard, Cancel — because Quill cannot answer this
    /// one: an untitled Document with text has no file to flush to, and a
    /// conflicted one has a file Quill would be overwriting someone else's
    /// change in.
    Ask,
}

/// What closing a window holding this Document has to do first.
///
/// The Chrome spec's "close discards with no prompt" is superseded here: the
/// prompt is for the two cases where writing the file is not obviously the
/// right answer, and every other close writes and goes.
#[must_use]
pub fn leaving(state: OnDisk, has_text: bool) -> Leaving {
    match state {
        OnDisk::Untitled if has_text => Leaving::Ask,
        OnDisk::Untitled => Leaving::Go,
        OnDisk::Named => Leaving::Flush,
        OnDisk::ChangedOnDisk | OnDisk::DeletedOnDisk => Leaving::Ask,
    }
}

/// The folder a Document opened at `path` adds to the Library, or `None` where
/// the Library already has a Location.
///
/// The first file a writer opens is where they write, so opening it with an
/// empty Library is what points the Library at that folder; every open after
/// that just opens, because a writer with Locations has already said which
/// folders Quill shows.
#[must_use]
pub fn location_for(library: &Library, path: &Path) -> Option<PathBuf> {
    if !library.locations().is_empty() {
        return None;
    }
    Some(path.parent()?.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::fs;

    /// A scratch directory of this test's own, named for the process so that
    /// two worktrees testing at once do not share one.
    fn scratch(name: &str) -> PathBuf {
        let directory =
            std::env::temp_dir().join(format!("quill-files-{name}-{}", std::process::id()));
        fs::remove_dir_all(&directory).ok();
        fs::create_dir_all(&directory).expect("makes its own scratch directory");
        directory
    }

    #[test]
    fn the_first_save_lands_in_the_first_location() {
        let folder = Path::new("/home/writer/Notes");
        assert_eq!(
            first_save_folder(Some(folder), false),
            Where::Folder(folder.to_path_buf())
        );
    }

    #[test]
    fn with_no_location_the_first_save_asks() {
        assert_eq!(first_save_folder(None, false), Where::Ask);
    }

    #[test]
    fn always_ask_asks_even_with_a_location() {
        assert_eq!(
            first_save_folder(Some(Path::new("/home/writer/Notes")), true),
            Where::Ask
        );
    }

    #[test]
    fn an_untitled_document_with_text_is_asked_about_and_an_empty_one_is_not() {
        assert_eq!(leaving(OnDisk::Untitled, true), Leaving::Ask);
        assert_eq!(leaving(OnDisk::Untitled, false), Leaving::Go);
    }

    #[test]
    fn a_named_document_is_flushed_however_much_is_in_it() {
        assert_eq!(leaving(OnDisk::Named, true), Leaving::Flush);
        assert_eq!(leaving(OnDisk::Named, false), Leaving::Flush);
    }

    #[test]
    fn a_conflicted_document_is_asked_about() {
        assert_eq!(leaving(OnDisk::ChangedOnDisk, true), Leaving::Ask);
        assert_eq!(leaving(OnDisk::DeletedOnDisk, true), Leaving::Ask);
    }

    #[test]
    fn an_empty_library_takes_the_folder_of_the_first_file_opened() {
        let directory = scratch("first-location");
        let path = directory.join("draft.md");
        fs::write(&path, "hello\n").expect("writes the Document");
        assert_eq!(
            location_for(&Library::new(), &path),
            Some(directory.clone())
        );
        fs::remove_dir_all(&directory).ok();
    }

    #[test]
    fn a_library_with_a_location_takes_nothing_from_an_open() {
        let directory = scratch("second-location");
        let path = directory.join("draft.md");
        fs::write(&path, "hello\n").expect("writes the Document");
        let library = Library::open(std::slice::from_ref(&directory), &[]);
        assert_eq!(location_for(&library, &path), None);
        // And a file from somewhere else is still no reason to add one.
        assert_eq!(location_for(&library, Path::new("/tmp/elsewhere.md")), None);
        fs::remove_dir_all(&directory).ok();
    }
}
