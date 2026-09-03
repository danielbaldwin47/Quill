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
use std::time::Duration;

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

/// What the status line at the foot of the sidebar has to say about the file.
///
/// The states the File handling spec's status line shows, rather than a string
/// built wherever the file is written: a window knows which of these it is in
/// and [`said`] is the one place the words live.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Standing {
    /// Nothing has been written from this window yet.
    AtRest,
    /// A write is going out now.
    Saving,
    /// The file holds what the writer typed, this long ago.
    Saved(Duration),
    /// The file changed under unsaved edits: the writer chooses Reload or
    /// Keep, which the sidebar offers beside these words.
    Changed,
    /// The file is gone. Save or Keep recreates it.
    Deleted,
}

/// The status line at rest: the file is on disk as the writer left it.
const AT_REST: &str = "All changes saved";

/// A minute, an hour and a day in seconds: how coarsely [`ago`] says a save's
/// age as it gets older.
const MINUTE: u64 = 60;
const HOUR: u64 = 60 * MINUTE;
const DAY: u64 = 24 * HOUR;

/// What the status line says while the Document is `standing`.
///
/// The conflict's line reads "Changed on disk · Reload · Keep" on screen: the
/// two words after these are buttons the sidebar stands beside the label
/// ([`crate::sidebar::Sidebar::set_offer`]), because they are clicked rather
/// than read.
#[must_use]
pub fn said(standing: Standing) -> String {
    match standing {
        Standing::AtRest => AT_REST.to_string(),
        Standing::Saving => "Saving…".to_string(),
        Standing::Saved(since) => format!("Saved · {}", ago(since)),
        Standing::Changed => "Changed on disk".to_string(),
        Standing::Deleted => "Deleted on disk".to_string(),
    }
}

/// How long ago a save was, as the status line says it.
///
/// The spec's shape ("Saved · 2 min ago"), coarsening as it ages: seconds are
/// "just now", then whole minutes, then hours, then days. A writer reading the
/// foot of the Library is asking whether their work is on disk, not what
/// second it landed.
fn ago(since: Duration) -> String {
    let seconds = since.as_secs();
    if seconds < MINUTE {
        return "just now".to_string();
    }
    if seconds < HOUR {
        return format!("{} min ago", seconds / MINUTE);
    }
    if seconds < DAY {
        return format!("{} hr ago", seconds / HOUR);
    }
    format!("{} d ago", seconds / DAY)
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

/// How deep [`stage`] copies. The fixture is two folders deep and this is the
/// bound on a tree that is not: a copy is the harness's own folder, and a walk
/// with no end would be a launch that never draws a frame.
const STAGE_DEPTH: usize = 8;

/// Copies the fixture Library at `fixture` to a folder of this process's own,
/// stamping each file with the mtime `manifest.json` names, and answers where
/// the copy is.
///
/// What `--library` is for: a judged shot of the Library has to be the same
/// Library on every machine, and git carries no mtimes, so a fresh checkout
/// would sort the fixture by whenever it was cloned. The copy is stamped
/// rather than the checkout because a shot must not write to the repository,
/// and because two runs at once — a shoot and a judge — would otherwise stamp
/// one tree twice.
///
/// # Errors
///
/// One line, naming what was missing: a fixture folder that is not there, a
/// `manifest.json` that is not beside it, or a copy that could not be made.
pub fn stage(fixture: &Path) -> Result<PathBuf, String> {
    if !fixture.is_dir() {
        return Err(format!("{}: no such fixture Library", fixture.display()));
    }
    let manifest = fixture.join(MANIFEST);
    let read = std::fs::read_to_string(&manifest)
        .map_err(|err| format!("{}: {err}", manifest.display()))?;
    let name = fixture
        .file_name()
        .ok_or_else(|| format!("{}: has no folder name", fixture.display()))?;
    let root = std::env::temp_dir()
        .join(format!("quill-library-{}", std::process::id()))
        .join(name);
    std::fs::remove_dir_all(&root).ok();
    copy(fixture, &root, STAGE_DEPTH)?;
    for (path, seconds) in mtimes(&read) {
        let file = root.join(path);
        let stamped = std::time::UNIX_EPOCH + std::time::Duration::from_secs(seconds);
        std::fs::OpenOptions::new()
            .write(true)
            .open(&file)
            .and_then(|file| file.set_modified(stamped))
            .map_err(|err| format!("{}: {err}", file.display()))?;
    }
    Ok(root)
}

/// A path named inside the fixture, as it stands in the copy [`stage`] made.
///
/// `--text` names a Document of the fixture's by its place in the checkout;
/// the Library the launch walks is the copy, and a Document opened from
/// outside it would be a window whose file no row of the sidebar holds.
#[must_use]
pub fn restaged(fixture: &Path, root: &Path, path: &Path) -> PathBuf {
    match path.strip_prefix(fixture) {
        Ok(inside) => root.join(inside),
        Err(_) => path.to_path_buf(),
    }
}

/// The fixture's stamps, beside its files.
const MANIFEST: &str = "manifest.json";

/// The mtimes `manifest.json` names, each a path under the fixture and the
/// epoch second it is stamped with.
///
/// Read by hand rather than through a JSON parser: the file is the Gate's own,
/// its shape is `{"mtimes": {"<path>": <seconds>}}`, and the app crate carries
/// no JSON reader for one fixture to justify. Anything it cannot read is
/// nothing stamped, which the Date sort shows at once.
fn mtimes(read: &str) -> Vec<(String, u64)> {
    let Some(table) = read.split_once("\"mtimes\"").and_then(|(_, rest)| {
        let open = rest.find('{')?;
        let close = rest[open..].find('}')?;
        Some(&rest[open + 1..open + close])
    }) else {
        return Vec::new();
    };
    table
        .split(',')
        .filter_map(|entry| {
            let (path, seconds) = entry.split_once(':')?;
            let path = path.trim().trim_matches('"');
            let seconds = seconds.trim().parse().ok()?;
            Some((path.to_string(), seconds))
        })
        .collect()
}

/// Copies the tree at `from` to `to`, `depth` folders deep.
fn copy(from: &Path, to: &Path, depth: usize) -> Result<(), String> {
    std::fs::create_dir_all(to).map_err(|err| format!("{}: {err}", to.display()))?;
    if depth == 0 {
        return Ok(());
    }
    let entries = std::fs::read_dir(from).map_err(|err| format!("{}: {err}", from.display()))?;
    for entry in entries {
        let entry = entry.map_err(|err| format!("{}: {err}", from.display()))?;
        let (source, target) = (entry.path(), to.join(entry.file_name()));
        if source.is_dir() {
            copy(&source, &target, depth - 1)?;
        } else {
            std::fs::copy(&source, &target)
                .map_err(|err| format!("{}: {err}", source.display()))?;
        }
    }
    Ok(())
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
    fn the_status_line_names_which_of_the_five_the_file_is_in() {
        assert_eq!(said(Standing::AtRest), "All changes saved");
        assert_eq!(said(Standing::Saving), "Saving…");
        assert_eq!(
            said(Standing::Saved(Duration::from_secs(2 * MINUTE))),
            "Saved · 2 min ago"
        );
        assert_eq!(said(Standing::Changed), "Changed on disk");
        assert_eq!(said(Standing::Deleted), "Deleted on disk");
    }

    #[test]
    fn a_save_ages_from_just_now_through_minutes_and_hours_to_days() {
        assert_eq!(ago(Duration::from_secs(0)), "just now");
        assert_eq!(ago(Duration::from_secs(MINUTE - 1)), "just now");
        assert_eq!(ago(Duration::from_secs(MINUTE)), "1 min ago");
        assert_eq!(ago(Duration::from_secs(HOUR - 1)), "59 min ago");
        assert_eq!(ago(Duration::from_secs(HOUR)), "1 hr ago");
        assert_eq!(ago(Duration::from_secs(DAY - 1)), "23 hr ago");
        assert_eq!(ago(Duration::from_secs(DAY)), "1 d ago");
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

    #[test]
    fn the_manifest_is_read_as_the_paths_and_the_seconds_it_names() {
        // Derived from `shots/oracle/library/manifest.json`, prose and all.
        let read = "{\n  \"_about\": \"The mtime each file is stamped with.\",\n  \
                    \"mtimes\": {\n    \".archive/old-draft.md\": 1740819600,\n    \
                    \"Drafts/closing.md\": 1740906000,\n    \"sea-storm.md\": 1741942800\n  }\n}\n";
        assert_eq!(
            mtimes(read),
            vec![
                (".archive/old-draft.md".to_string(), 1_740_819_600),
                ("Drafts/closing.md".to_string(), 1_740_906_000),
                ("sea-storm.md".to_string(), 1_741_942_800),
            ]
        );
        assert!(mtimes("{}").is_empty(), "no table is nothing stamped");
    }

    #[test]
    fn staging_copies_the_tree_and_stamps_it_from_the_manifest() {
        let fixture = scratch("fixture");
        fs::create_dir_all(fixture.join("Drafts")).expect("the subfolder");
        fs::write(fixture.join("sea-storm.md"), "# The storm\n").expect("a Document");
        fs::write(fixture.join("Drafts/opening.md"), "# Opening\n").expect("another");
        fs::write(
            fixture.join(MANIFEST),
            "{\"mtimes\": {\"sea-storm.md\": 1741942800, \"Drafts/opening.md\": 1741856400}}",
        )
        .expect("the manifest");

        let root = stage(&fixture).expect("the fixture is copied");
        assert_ne!(root, fixture, "the copy is not the checkout");
        assert_eq!(
            fs::read_to_string(root.join("sea-storm.md")).expect("the copy holds the Document"),
            "# The storm\n"
        );
        let stamped = fs::metadata(root.join("Drafts/opening.md"))
            .and_then(|of| of.modified())
            .expect("the copy is stamped");
        assert_eq!(
            stamped
                .duration_since(std::time::UNIX_EPOCH)
                .expect("after the epoch")
                .as_secs(),
            1_741_856_400
        );
        // And the Document `--text` names in the checkout is the one in the
        // copy, so the sidebar's highlight has a row to land on.
        assert_eq!(
            restaged(&fixture, &root, &fixture.join("sea-storm.md")),
            root.join("sea-storm.md")
        );
        assert_eq!(
            restaged(&fixture, &root, Path::new("/tmp/elsewhere.md")),
            Path::new("/tmp/elsewhere.md")
        );
        fs::remove_dir_all(&fixture).ok();
        fs::remove_dir_all(root.parent().expect("a temp folder of its own")).ok();
    }

    #[test]
    fn a_fixture_that_is_not_there_is_one_line_naming_it() {
        let missing = std::env::temp_dir().join(format!("quill-nothing-{}", std::process::id()));
        let refused = stage(&missing).expect_err("nothing to copy");
        assert!(refused.ends_with("no such fixture Library"), "{refused}");
        assert!(!refused.contains('\n'), "one line, not a stack: {refused}");

        // A folder with no manifest is refused by the file it is missing.
        let bare = scratch("bare");
        let refused = stage(&bare).expect_err("nothing to stamp from");
        assert!(refused.contains(MANIFEST), "{refused}");
        assert!(!refused.contains('\n'), "one line, not a stack: {refused}");
        fs::remove_dir_all(&bare).ok();
    }
}
