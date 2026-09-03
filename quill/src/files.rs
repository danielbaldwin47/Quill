//! What a window has to decide about the file behind its Document, without a
//! window to decide it in.
//!
//! Some questions come up wherever a Document meets the disk: where an
//! untitled Document's first save goes, whether a window closing has anything
//! to ask the writer first, whether opening a file should point the Library at
//! the folder it came from, which row `file.next` steps to, what letting a
//! dragged row go does and which way `file.pin` turns, and what the status
//! line says. Each is a decision over plain values — the path state
//! ([`quill_engine::disk::OnDisk`]), the Locations, the list of rows on screen,
//! whether there is any text — so each is a function here rather than a branch
//! inside a signal handler, and each is tested with no display attached.
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
/// `selected` is the folder the sidebar's selected row stands in, taken when
/// the Document was started and not when it is saved, and it goes in front of
/// the first Location: a writer who picked a folder and pressed `Ctrl+N` said
/// where this one goes. Asking beats both, because it is the writer asking.
#[must_use]
pub fn first_save_folder(selected: Option<&Path>, first: Option<&Path>, always_ask: bool) -> Where {
    if always_ask {
        return Where::Ask;
    }
    match selected.or(first) {
        Some(folder) => Where::Folder(folder.to_path_buf()),
        None => Where::Ask,
    }
}

/// Which way `file.next` and `file.prev` walk the list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// Down the list, as the sidebar shows it.
    Next,
    /// Up it.
    Prev,
}

/// The Document `step` lands on, walking `files` — the visible list in the
/// order the sidebar sorted it — from the one at `open`.
///
/// It wraps, so stepping past either end comes round rather than stopping: the
/// list is what a writer can see, and a walk of it is a walk of a ring. With
/// nothing open, or a Document no row of the list holds, Next is the first row
/// and Prev the last. The bound is one pass of `files`.
#[must_use]
pub fn stepped(files: &[PathBuf], open: Option<&Path>, step: Step) -> Option<PathBuf> {
    if files.is_empty() {
        return None;
    }
    let at = open.and_then(|open| files.iter().position(|file| file == open));
    let landed = match (at, step) {
        (Some(at), Step::Next) => (at + 1) % files.len(),
        (Some(at), Step::Prev) => (at + files.len() - 1) % files.len(),
        (None, Step::Next) => 0,
        (None, Step::Prev) => files.len() - 1,
    };
    files.get(landed).cloned()
}

/// What the status line says once a Document has gone to the system trash.
///
/// The spec's words, and transient by construction: the next thing the window
/// has to say about the file overwrites them (`Window::show_standing`), which
/// is what makes it a notice rather than a state.
#[must_use]
pub fn moved_to_trash(name: &str) -> String {
    format!("Moved {name} to Trash")
}

/// What the status line says once a Document has been moved into a folder.
///
/// The trash notice's shape ([`moved_to_trash`]), naming the folder rather than
/// the whole path it went to: the writer let the row go over another row, and
/// that row's own name is what they aimed at.
#[must_use]
pub fn moved_into(name: &str, folder: &str) -> String {
    format!("Moved {name} to {folder}")
}

/// Where a dragged row was let go.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Onto {
    /// The Pinned section: its head or any row of it, because the whole
    /// section is the one target and no row of it is a target of its own.
    Pinned,
    /// A folder's row or a Location's head, both of which are folders on disk.
    Folder(PathBuf),
}

/// What letting a dragged row go does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Dropped {
    /// The dragged path joins the Pinned list.
    Pin,
    /// It moves into this folder.
    Into(PathBuf),
}

/// What dropping `dragged` onto `onto` does, or `None` where it does nothing
/// and the drag is refused while it is still in the air.
///
/// A file's row is no target at all, so it is not one of [`Onto`]'s cases. Of
/// the two that are: the Pinned section takes anything not pinned already, and
/// a folder takes anything that is not already in it and is not the folder
/// itself or a folder above it — a folder cannot be moved inside its own tree,
/// and `starts_with` is both of those refusals at once.
#[must_use]
pub fn dropped(dragged: &Path, onto: &Onto, pinned: &[PathBuf]) -> Option<Dropped> {
    match onto {
        Onto::Pinned => (!pinned.iter().any(|held| held == dragged)).then_some(Dropped::Pin),
        Onto::Folder(folder) => {
            let stays = folder.starts_with(dragged) || dragged.parent() == Some(folder.as_path());
            (!stays).then(|| Dropped::Into(folder.clone()))
        }
    }
}

/// What `file.pin` does: the path it acts on, and whether that path is being
/// pinned rather than unpinned.
///
/// The Library's selected row first, because the row a writer is looking at is
/// the one they mean, and the open Document where the pane is shut or has
/// nothing selected, so the Command works either way. A path already Pinned is
/// unpinned, which is the Command's other half. `None` where there is neither a
/// row nor a file, which is an untitled Document with the pane shut.
#[must_use]
pub fn pinning(
    selected: Option<&Path>,
    open: Option<&Path>,
    pinned: &[PathBuf],
) -> Option<(PathBuf, bool)> {
    let path = selected.or(open)?;
    Some((path.to_path_buf(), !pinned.iter().any(|held| held == path)))
}

/// How much of `name` a rename field selects when it opens: everything before
/// the extension, in characters, which is what a GTK field's positions count
/// in.
///
/// The oracle's rule (`legacy/app/js/files.js` `startRename`, which selects
/// `value.replace(EXT, '').length`): a writer renaming a Document is renaming
/// the name and not the `.md`, and typing over the selection keeps it. A name
/// with no dot in it is selected whole.
#[must_use]
pub fn stem_chars(name: &str) -> i32 {
    let stem = name.rsplit_once('.').map_or(name, |(stem, _)| stem);
    i32::try_from(stem.chars().count()).unwrap_or(i32::MAX)
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

/// A minute in seconds: the first step [`ago`] coarsens a save's age to.
const MINUTE: u64 = 60;
/// An hour in seconds: the step after that.
const HOUR: u64 = 60 * MINUTE;
/// A day in seconds: the last step, which every older save is said in.
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
    let root = staging().join(name);
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

/// The folder this launch stages a fixture Library into.
///
/// Named for the process, so that two launches at once — a shoot and a judge —
/// stage into folders of their own.
fn staging() -> PathBuf {
    std::env::temp_dir().join(format!("quill-library-{}", std::process::id()))
}

/// Takes the staged copy away again.
///
/// Called as Quill goes down: the copy is this launch's own, nothing outside
/// it reads it, and a `$TMPDIR` left holding a tree per judged run is a tree
/// per judged run nobody clears. A launch that staged nothing has nothing
/// here to remove.
pub fn unstage() {
    std::fs::remove_dir_all(staging()).ok();
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

/// How deep a value of the manifest may nest before it is refused, which is
/// what bounds the read.
const NESTING: usize = 16;

/// A value of the manifest's JSON, as much of it as [`mtimes`] reads.
#[derive(Debug, PartialEq, Eq)]
enum Value {
    /// A `{}` and what it names, in the order the file names them.
    Table(Vec<(String, Value)>),
    /// A whole number, which is what a stamp is.
    Whole(u64),
    /// Anything else — a string, a list, a fraction, `true`, `false`, `null` —
    /// read to its end and stepped over.
    Passed,
}

/// A reader over the manifest's JSON.
///
/// A parser of its own rather than a dependency: the `quill` crate carries no
/// JSON reader, and the one shape this reads — a table of tables of numbers —
/// is a few lines of recursive descent. It is not a general JSON reader and
/// does not claim to be one; what it will not read, it refuses.
struct Json<'a> {
    /// What is left to read.
    rest: &'a str,
}

impl Json<'_> {
    /// The next letter, with any whitespace in front of it stepped over.
    fn peek(&mut self) -> Option<char> {
        self.rest = self.rest.trim_start();
        self.rest.chars().next()
    }

    /// Takes `letter` where it is next, answering whether it was there.
    fn take(&mut self, letter: char) -> bool {
        if self.peek() == Some(letter) {
            self.rest = &self.rest[letter.len_utf8()..];
            return true;
        }
        false
    }

    /// One value, `depth` tables deep.
    fn value(&mut self, depth: usize) -> Option<Value> {
        match self.peek()? {
            '{' => self.table(depth),
            '[' => self.list(depth),
            '"' => self.text().map(|_| Value::Passed),
            '-' | '0'..='9' => self.number(),
            _ => self.word(),
        }
    }

    /// A `{}` and every name in it.
    fn table(&mut self, depth: usize) -> Option<Value> {
        if depth == 0 || !self.take('{') {
            return None;
        }
        let mut named = Vec::new();
        if self.take('}') {
            return Some(Value::Table(named));
        }
        loop {
            let name = self.text()?;
            if !self.take(':') {
                return None;
            }
            named.push((name, self.value(depth - 1)?));
            if self.take(',') {
                continue;
            }
            if self.take('}') {
                return Some(Value::Table(named));
            }
            return None;
        }
    }

    /// A `[]` and everything in it, stepped over.
    fn list(&mut self, depth: usize) -> Option<Value> {
        if depth == 0 || !self.take('[') {
            return None;
        }
        if self.take(']') {
            return Some(Value::Passed);
        }
        loop {
            self.value(depth - 1)?;
            if self.take(',') {
                continue;
            }
            if self.take(']') {
                return Some(Value::Passed);
            }
            return None;
        }
    }

    /// A string, with its escapes read.
    fn text(&mut self) -> Option<String> {
        if !self.take('"') {
            return None;
        }
        let mut held = String::new();
        loop {
            let letter = self.next_letter()?;
            match letter {
                '"' => return Some(held),
                '\\' => held.push(self.escaped()?),
                other => held.push(other),
            }
        }
    }

    /// What a `\` in a string stands for.
    fn escaped(&mut self) -> Option<char> {
        match self.next_letter()? {
            'n' => Some('\n'),
            't' => Some('\t'),
            'r' => Some('\r'),
            'b' => Some('\u{8}'),
            'f' => Some('\u{c}'),
            'u' => {
                let digits = self.rest.get(..4)?;
                self.rest = &self.rest[4..];
                char::from_u32(u32::from_str_radix(digits, 16).ok()?)
            }
            // `"`, `\` and `/`, and nothing else is written this way.
            other => Some(other),
        }
    }

    /// The next letter, whitespace and all.
    fn next_letter(&mut self) -> Option<char> {
        let mut letters = self.rest.chars();
        let letter = letters.next()?;
        self.rest = letters.as_str();
        Some(letter)
    }

    /// A number. A whole one is a stamp; anything else is stepped over.
    fn number(&mut self) -> Option<Value> {
        let end = self
            .rest
            .find(|letter: char| !matches!(letter, '0'..='9' | '-' | '+' | '.' | 'e' | 'E'))
            .unwrap_or(self.rest.len());
        let (digits, rest) = self.rest.split_at(end);
        self.rest = rest;
        Some(digits.parse().map_or(Value::Passed, Value::Whole))
    }

    /// `true`, `false` or `null`, stepped over.
    fn word(&mut self) -> Option<Value> {
        let end = self
            .rest
            .find(|letter: char| !letter.is_ascii_alphabetic())
            .unwrap_or(self.rest.len());
        if end == 0 {
            return None;
        }
        self.rest = &self.rest[end..];
        Some(Value::Passed)
    }
}

/// The mtimes `manifest.json` names, each a path under the fixture and the
/// epoch second it is stamped with, in the order the file names them.
///
/// A manifest this cannot read is nothing stamped, which the Date sort shows
/// at once.
fn mtimes(read: &str) -> Vec<(String, u64)> {
    let mut json = Json { rest: read };
    let Some(Value::Table(named)) = json.value(NESTING) else {
        return Vec::new();
    };
    let Some((_, Value::Table(mtimes))) = named.into_iter().find(|(name, _)| name == "mtimes")
    else {
        return Vec::new();
    };
    mtimes
        .into_iter()
        .filter_map(|(path, stamp)| match stamp {
            Value::Whole(seconds) => Some((path, seconds)),
            _ => None,
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
            first_save_folder(None, Some(folder), false),
            Where::Folder(folder.to_path_buf())
        );
    }

    #[test]
    fn with_no_location_the_first_save_asks() {
        assert_eq!(first_save_folder(None, None, false), Where::Ask);
    }

    #[test]
    fn always_ask_asks_even_with_a_location() {
        assert_eq!(
            first_save_folder(None, Some(Path::new("/home/writer/Notes")), true),
            Where::Ask
        );
    }

    #[test]
    fn the_selected_rows_folder_takes_the_first_save_before_the_first_location() {
        let selected = Path::new("/home/writer/Notes/Drafts");
        let first = Path::new("/home/writer/Notes");
        assert_eq!(
            first_save_folder(Some(selected), Some(first), false),
            Where::Folder(selected.to_path_buf())
        );
        // And a writer who asked to be asked is still asked.
        assert_eq!(
            first_save_folder(Some(selected), Some(first), true),
            Where::Ask
        );
    }

    #[test]
    fn next_and_prev_walk_the_visible_list_and_come_round_at_its_ends() {
        let files: Vec<PathBuf> = ["a.md", "b.md", "c.md"]
            .iter()
            .map(|name| PathBuf::from("/notes").join(name))
            .collect();
        let at = |name: &str| PathBuf::from("/notes").join(name);
        assert_eq!(
            stepped(&files, Some(&at("a.md")), Step::Next),
            Some(at("b.md"))
        );
        assert_eq!(
            stepped(&files, Some(&at("c.md")), Step::Next),
            Some(at("a.md"))
        );
        assert_eq!(
            stepped(&files, Some(&at("b.md")), Step::Prev),
            Some(at("a.md"))
        );
        assert_eq!(
            stepped(&files, Some(&at("a.md")), Step::Prev),
            Some(at("c.md"))
        );
        // A Document no row holds, and an untitled one, start at either end.
        assert_eq!(stepped(&files, None, Step::Next), Some(at("a.md")));
        assert_eq!(
            stepped(&files, Some(Path::new("/elsewhere.md")), Step::Prev),
            Some(at("c.md"))
        );
        assert_eq!(stepped(&[], None, Step::Next), None);
    }

    #[test]
    fn a_rename_field_opens_with_the_name_selected_and_not_the_extension() {
        assert_eq!(stem_chars("sea-storm.md"), 9);
        assert_eq!(stem_chars("notes.tar.md"), 9);
        assert_eq!(stem_chars("README"), 6);
        // Characters, not bytes: a GTK field counts positions in characters.
        assert_eq!(stem_chars("œuvre.md"), 5);
    }

    #[test]
    fn the_move_notice_names_the_document_and_the_folder_it_went_into() {
        assert_eq!(
            moved_into("Sea storm.md", "Drafts"),
            "Moved Sea storm.md to Drafts"
        );
    }

    #[test]
    fn a_row_let_go_over_the_pinned_section_pins_it_unless_it_is_pinned_already() {
        let held = PathBuf::from("/w/Drafts");
        let loose = PathBuf::from("/w/Sea storm.md");
        let pinned = vec![held.clone()];
        assert_eq!(dropped(&loose, &Onto::Pinned, &pinned), Some(Dropped::Pin));
        assert_eq!(dropped(&held, &Onto::Pinned, &pinned), None);
    }

    #[test]
    fn a_row_let_go_over_a_folder_moves_into_it_and_never_into_itself_or_where_it_is() {
        let drafts = PathBuf::from("/w/Drafts");
        let file = PathBuf::from("/w/Sea storm.md");
        assert_eq!(
            dropped(&file, &Onto::Folder(drafts.clone()), &[]),
            Some(Dropped::Into(drafts.clone()))
        );
        // Already in it, on itself, and into its own subtree: nothing to do,
        // and the drag is refused before it lands.
        assert_eq!(
            dropped(
                &PathBuf::from("/w/Drafts/Sea storm.md"),
                &Onto::Folder(drafts.clone()),
                &[]
            ),
            None
        );
        assert_eq!(dropped(&drafts, &Onto::Folder(drafts.clone()), &[]), None);
        assert_eq!(
            dropped(&drafts, &Onto::Folder(PathBuf::from("/w/Drafts/Old")), &[]),
            None
        );
    }

    #[test]
    fn pin_acts_on_the_selected_row_before_the_open_document_and_turns_it_the_other_way() {
        let row = PathBuf::from("/w/Drafts");
        let open = PathBuf::from("/w/Sea storm.md");
        let pinned = vec![row.clone()];
        assert_eq!(
            pinning(Some(&row), Some(&open), &pinned),
            Some((row.clone(), false)),
            "the selected row is pinned, so the Command unpins it"
        );
        assert_eq!(
            pinning(None, Some(&open), &pinned),
            Some((open, true)),
            "with no row, the open Document"
        );
        assert_eq!(pinning(None, None, &pinned), None);
    }

    #[test]
    fn the_trash_notice_names_the_document_that_went() {
        assert_eq!(
            moved_to_trash("sea-storm.md"),
            "Moved sea-storm.md to Trash"
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

    /// The manifest the Gate actually stamps from, read from the checkout
    /// rather than copied into a literal here (`CODING_STANDARDS.md` § Tools).
    fn judged_manifest() -> String {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("the crate sits inside the workspace")
            .join("shots/oracle/library")
            .join(MANIFEST);
        fs::read_to_string(&path).unwrap_or_else(|err| panic!("{}: {err}", path.display()))
    }

    #[test]
    fn the_manifest_is_read_as_the_paths_and_the_seconds_it_names() {
        let read = judged_manifest();
        let named = mtimes(&read);

        // Every stamp the file writes, and no other: each pair is the file's
        // own `"<path>": <seconds>` line, and there are as many pairs as the
        // `mtimes` table has lines with a stamp on them.
        let table = read
            .split_once("\"mtimes\"")
            .expect("the manifest names its mtimes")
            .1;
        let lines = table.lines().filter(|line| line.contains("\": 1")).count();
        assert_eq!(named.len(), lines, "one pair per stamped line");
        assert!(lines >= 3, "the fixture stamps its files: {lines}");
        for (path, seconds) in &named {
            assert!(
                read.contains(&format!("\"{path}\": {seconds}")),
                "{path} is stamped {seconds} in the file"
            );
        }

        assert!(mtimes("{}").is_empty(), "no table is nothing stamped");
        assert!(mtimes("not json").is_empty(), "and nothing it cannot read");
    }

    #[test]
    fn a_stamp_is_read_past_the_prose_the_lists_and_the_escapes_around_it() {
        let read = r#"{
            "_about": "a note with a \" and a \\ in it",
            "_seen": [1, {"deep": 2}, true, null, 1.5],
            "mtimes": {"a\/b.md": 1740819600, "half.md": 1.5}
        }"#;
        assert_eq!(
            mtimes(read),
            vec![("a/b.md".to_string(), 1_740_819_600)],
            "the whole seconds are stamps and nothing else is"
        );
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

        // And the copy goes with the launch that made it.
        unstage();
        assert!(!root.exists(), "the staged Library is gone");
        assert!(!staging().exists(), "and so is the folder it stood in");
        fs::remove_dir_all(&fixture).ok();
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
