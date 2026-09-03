//! A Document on disk: its path state, the save that replaces the file, the
//! name the first save derives, and the line diff of a conflict.
//!
//! A Document is a path ([`crate::document`]), and this module is what happens
//! at that path. It holds a Document beside the version of the file Quill last
//! wrote or read, so a watch event can be answered with a `stat` rather than a
//! read: the file the Document last saved has not changed, and any other
//! version has. `docs/architecture.md` § Documents and files sets the rest —
//! the save goes to a temporary file in the same directory and is renamed over
//! the original with its permissions preserved, a change on disk to a clean
//! Document reloads it in place keeping the caret's block, and a change under
//! unsaved edits is a conflict.
//!
//! The four states are [`OnDisk`], and every transition is a method: [`Filed::save`]
//! and [`Filed::save_in`] write, [`Filed::noticed`] classifies a watch event,
//! and [`Filed::reload`] and [`Filed::keep`] are the two ways out of a conflict.
//! The timer that calls them, the dialogs and the rendering of [`diff`] are the
//! app's (#251, #255); everything here is plain data for it to read.

use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::document::{Document, UNTITLED};
use crate::library;
use crate::settings::file;
use crate::watch::{self, Version};

/// The extension a derived name is given when the first line does not already
/// end in one the Library lists.
const DEFAULT_EXTENSION: &str = "md";

/// How much of the first line a derived name keeps, in characters.
///
/// The oracle's cap: `legacy/app/js/files.js` `deriveTitle` slices at 80.
const TITLE_CHARS: usize = 80;

/// How many suffixed names a collision tries before it gives up.
///
/// The oracle's cap: `legacy/app/js/files.js` `uniqueName` counts to 999.
const SUFFIXES: usize = 999;

/// Where a Document stands with the file behind it.
///
/// A sentinel is an enum variant, so "no file yet" and "the file moved under
/// us" are states rather than a pair of booleans the app has to combine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OnDisk {
    /// No file yet. The first save derives a name from the first line and
    /// makes one; until then the Document lives only in the window.
    Untitled,
    /// A file holding what Quill last wrote or read. Autosave writes it.
    Named,
    /// The file changed under unsaved edits. Autosave pauses, and the writer
    /// chooses: [`Filed::reload`] takes the disk's text, [`Filed::keep`] writes
    /// the Editor's over it.
    ChangedOnDisk,
    /// The file is gone. Autosave pauses, and [`Filed::save`] or
    /// [`Filed::keep`] recreates it.
    DeletedOnDisk,
}

/// What a save did, for a caller that must know why nothing was written.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[must_use = "a save that wrote nothing leaves the writer's text only in the window"]
pub enum Saved {
    /// The file holds the Document's text, and its version is recorded.
    Written,
    /// Nothing was written: the Document is untitled, so the caller chooses
    /// the folder and calls [`Filed::save_in`].
    NeedsAFolder,
    /// Nothing was written: the file changed under the Document, and only
    /// [`Filed::reload`] or [`Filed::keep`] resolves that.
    Refused,
}

/// Where the caret was when a reload replaced the text under it.
///
/// The two travel together: the app puts the caret back by the block, and the
/// offset is what it had before, clamped into the new text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Kept {
    /// The block the caret was in before the reload, in the block index the
    /// Document had then, or `None` where the Document had no block there.
    pub block: Option<usize>,
    /// The caret's byte offset, clamped to the reloaded text and to a
    /// character boundary.
    pub caret: usize,
}

/// What a watch event on the Document's file turned out to be.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[must_use = "the event is classified so that the app can answer it"]
pub enum Noticed {
    /// The file is the version Quill last wrote or read: Quill's own save, or
    /// an open, and nothing to do.
    Unchanged,
    /// The Document was clean, so the disk's text is in it now.
    Reloaded(Kept),
    /// The file changed under unsaved edits: [`OnDisk::ChangedOnDisk`].
    Changed,
    /// The file is gone: [`OnDisk::DeletedOnDisk`].
    Deleted,
}

/// One line of a [`diff`], as the app renders it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Line {
    /// A line both texts have.
    Same(String),
    /// A line the first text has and the second does not.
    Removed(String),
    /// A line the second text has and the first does not.
    Added(String),
}

/// A Document and the file it is: everything about the path, in one place.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Filed {
    document: Document,
    state: OnDisk,
    /// The version of the file Quill last wrote or read, which is what a watch
    /// event is compared against; `None` while there is no file.
    saved: Option<Version>,
}

impl Default for Filed {
    /// The same as [`Filed::untitled`], so that a window built before it has
    /// been handed a Document holds an untitled one rather than a `None`.
    fn default() -> Self {
        Self::untitled()
    }
}

impl Filed {
    /// A Document with no file behind it and nothing written in it.
    #[must_use]
    pub fn untitled() -> Self {
        Self {
            document: Document::untitled(),
            state: OnDisk::Untitled,
            saved: None,
        }
    }

    /// Reads `path` and records the version read.
    ///
    /// # Errors
    ///
    /// Returns the underlying [`io::Error`] when the file cannot be read, as
    /// [`Document::open`] does.
    pub fn open(path: &Path) -> io::Result<Self> {
        let document = Document::open(path)?;
        Ok(Self {
            document,
            state: OnDisk::Named,
            saved: watch::version(path),
        })
    }

    /// Takes over a Document already opened elsewhere.
    ///
    /// A Document with no path is [`OnDisk::Untitled`]; one with a path is
    /// [`OnDisk::Named`] against the file as it is now, so a Document opened
    /// and then handed here is not a conflict.
    #[must_use]
    pub fn around(document: Document) -> Self {
        let saved = document.path().and_then(watch::version);
        let state = match (document.path(), saved) {
            (None, _) => OnDisk::Untitled,
            (Some(_), None) => OnDisk::DeletedOnDisk,
            (Some(_), Some(_)) => OnDisk::Named,
        };
        Self {
            document,
            state,
            saved,
        }
    }

    /// The Document itself.
    #[must_use]
    pub fn document(&self) -> &Document {
        &self.document
    }

    /// The Document itself, to edit.
    pub fn document_mut(&mut self) -> &mut Document {
        &mut self.document
    }

    /// Where the Document stands with its file.
    #[must_use]
    pub fn state(&self) -> OnDisk {
        self.state
    }

    /// The file this Document is, or `None` while it is untitled.
    #[must_use]
    pub fn path(&self) -> Option<&Path> {
        self.document.path()
    }

    /// Whether autosave may write this Document.
    ///
    /// It pauses in both conflict states, where writing is exactly what the
    /// writer has not chosen yet.
    #[must_use]
    pub fn autosaves(&self) -> bool {
        matches!(self.state, OnDisk::Untitled | OnDisk::Named)
    }

    /// Writes the Document's text over its file, recording the new version.
    ///
    /// Recreates the file of a [`OnDisk::DeletedOnDisk`] Document. Writes
    /// nothing for an untitled Document, which has no folder to write into, or
    /// for one whose file changed underneath it; the answer says which.
    ///
    /// # Errors
    ///
    /// Returns the underlying [`io::Error`] when the file cannot be written or
    /// renamed; the file is then as it was.
    pub fn save(&mut self) -> io::Result<Saved> {
        match self.state {
            OnDisk::Untitled => Ok(Saved::NeedsAFolder),
            OnDisk::ChangedOnDisk => Ok(Saved::Refused),
            OnDisk::Named | OnDisk::DeletedOnDisk => self.write_over_the_file(Over::OurVersion),
        }
    }

    /// Saves an untitled Document into `folder` under the name its first line
    /// derives ([`first_save_name`], [`unique_in`]), and fixes that name.
    ///
    /// The name is derived once, at the first save: a Document that already
    /// has a file ignores `folder` and saves where it is, so editing the first
    /// line later never renames the file.
    ///
    /// # Errors
    ///
    /// Returns the underlying [`io::Error`] when the file cannot be written or
    /// renamed.
    pub fn save_in(&mut self, folder: &Path) -> io::Result<Saved> {
        if self.state != OnDisk::Untitled {
            return self.save();
        }
        let path = unique_in(folder, &first_save_name(self.document.text()));
        self.saved = Some(wrote(&path, self.document.text())?);
        self.document.set_path(path);
        self.state = OnDisk::Named;
        Ok(Saved::Written)
    }

    /// Writes the Editor's text over the file, whatever happened to it: the
    /// Keep half of a conflict, and what recreates a deleted file.
    ///
    /// # Errors
    ///
    /// Returns the underlying [`io::Error`] when the file cannot be written or
    /// renamed.
    pub fn keep(&mut self) -> io::Result<Saved> {
        if self.state == OnDisk::Untitled {
            return Ok(Saved::NeedsAFolder);
        }
        self.write_over_the_file(Over::Anything)
    }

    /// Replaces the Document's text with the file's: the Reload half of a
    /// conflict, and what a clean Document does by itself.
    ///
    /// `caret` is where the caret was, and the answer says where to put it
    /// back.
    ///
    /// # Errors
    ///
    /// Returns the underlying [`io::Error`] when the file cannot be read, and
    /// [`io::ErrorKind::InvalidInput`] for an untitled Document, which has no
    /// file to reload from.
    pub fn reload(&mut self, caret: usize) -> io::Result<Kept> {
        let Some(path) = self.document.path().map(Path::to_path_buf) else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "an untitled Document has no file to reload from",
            ));
        };
        let text = fs::read_to_string(&path)?;
        let block = self.document.block_at(caret);
        self.document.reload(text);
        self.saved = watch::version(&path);
        self.state = OnDisk::Named;
        Ok(Kept {
            block,
            caret: within(self.document.text(), caret),
        })
    }

    /// Answers a watch event on the Document's file.
    ///
    /// `dirty` is whether the Editor holds edits the file does not: a clean
    /// Document reloads in place, and a dirty one enters
    /// [`OnDisk::ChangedOnDisk`] for the writer to resolve. A file that is
    /// gone is [`OnDisk::DeletedOnDisk`] however clean the Document is, and a
    /// clean Document whose file cannot be re-read is treated as changed
    /// rather than reloaded, because the text in the window is then the only
    /// copy of it Quill has.
    pub fn noticed(&mut self, dirty: bool, caret: usize) -> Noticed {
        let Some(path) = self.document.path().map(Path::to_path_buf) else {
            return Noticed::Unchanged;
        };
        let Some(now) = watch::version(&path) else {
            self.saved = None;
            self.state = OnDisk::DeletedOnDisk;
            return Noticed::Deleted;
        };
        if self.state == OnDisk::Named && Some(now) == self.saved {
            return Noticed::Unchanged;
        }
        if dirty {
            self.state = OnDisk::ChangedOnDisk;
            return Noticed::Changed;
        }
        match self.reload(caret) {
            Ok(kept) => Noticed::Reloaded(kept),
            Err(_) => {
                self.state = OnDisk::ChangedOnDisk;
                Noticed::Changed
            }
        }
    }

    /// Follows the file to `path`, where something outside Quill moved it.
    ///
    /// The version is read again at the new path, because a move that crossed
    /// a device rewrote the file; a file that is not there is a delete under
    /// another name.
    pub fn moved_to(&mut self, path: &Path) {
        self.document.set_path(path.to_path_buf());
        self.saved = watch::version(path);
        self.state = match (self.saved, self.state) {
            (None, _) => OnDisk::DeletedOnDisk,
            (Some(_), OnDisk::ChangedOnDisk) => OnDisk::ChangedOnDisk,
            (Some(_), _) => OnDisk::Named,
        };
    }

    /// Follows a rename made outside Quill, given every path one drain of the
    /// watch answered for; answers whether the Document followed.
    ///
    /// A rename arrives as two paths in the one drain — the path the file left
    /// and the path it took ([`crate::watch::Watch::add_tree`]) — so the pair
    /// is looked for there and nowhere else. This Document's file has to be
    /// gone, and one other path of the batch has to be a file that is there
    /// and is this one: the same file name, which is a Document moved into
    /// another folder, or the length and write time this Document last wrote
    /// or read, which is one renamed where it stood.
    ///
    /// Asked before [`Filed::noticed`], which sees only the path that is gone
    /// and would call it deleted (#246, story 29). The bound is one `stat` per
    /// path of the batch.
    pub fn followed(&mut self, batch: &[PathBuf]) -> bool {
        let Some(path) = self.document.path().map(Path::to_path_buf) else {
            return false;
        };
        if watch::version(&path).is_some() {
            return false;
        }
        let was = self.saved;
        let took = batch.iter().find(|arrived| {
            if **arrived == path || !library::listed(arrived) {
                return false;
            }
            match watch::version(arrived) {
                None => false,
                now => arrived.file_name() == path.file_name() || now == was,
            }
        });
        let Some(took) = took.cloned() else {
            return false;
        };
        self.moved_to(&took);
        true
    }

    /// The file's text as it is now, which a conflict shows against the
    /// Editor's.
    ///
    /// # Errors
    ///
    /// Returns the underlying [`io::Error`] when the file cannot be read, and
    /// [`io::ErrorKind::InvalidInput`] for an untitled Document.
    pub fn text_on_disk(&self) -> io::Result<String> {
        let Some(path) = self.document.path() else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "an untitled Document has no file to read",
            ));
        };
        fs::read_to_string(path)
    }

    /// The line diff of the file's text against the Editor's: what Reload
    /// would take away (`Removed`) and what it would lose of the window's text
    /// (`Added`).
    ///
    /// # Errors
    ///
    /// As [`Filed::text_on_disk`].
    pub fn against_disk(&self) -> io::Result<Vec<Line>> {
        Ok(diff(&self.text_on_disk()?, self.document.text()))
    }

    /// Writes the text over the file and records what the file then is.
    ///
    /// `over` says what the write may stand on. Under [`Over::OurVersion`] the
    /// file is re-stated first: a file that moved since Quill last wrote or
    /// read it is a conflict, and the write is refused rather than clobbering
    /// it — the watch's own event for that change arrives a moment later and
    /// finds the Document already in [`OnDisk::ChangedOnDisk`].
    fn write_over_the_file(&mut self, over: Over) -> io::Result<Saved> {
        let Some(path) = self.document.path().map(Path::to_path_buf) else {
            return Ok(Saved::NeedsAFolder);
        };
        if over == Over::OurVersion
            && self.state == OnDisk::Named
            && watch::version(&path) != self.saved
        {
            self.state = OnDisk::ChangedOnDisk;
            return Ok(Saved::Refused);
        }
        self.saved = Some(wrote(&path, self.document.text())?);
        self.state = OnDisk::Named;
        Ok(Saved::Written)
    }
}

/// What a write is allowed to stand on.
///
/// A sentinel rather than a `bool`, because the two writes are different acts:
/// one is autosave, which may never lose someone else's change, and the other
/// is the writer having looked at that change and chosen theirs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Over {
    /// Only the version this Document last wrote or read. Anything else is a
    /// conflict and the write is refused.
    OurVersion,
    /// Whatever the file holds now: the Keep half of a conflict, and what
    /// recreates a file that is gone.
    Anything,
}

/// Writes `text` over `path` and reads back the version it now is.
///
/// The write is [`file::write`]'s: a temporary beside the destination, the
/// destination's permissions put on it, and a rename over the destination, so
/// the folder never holds anything of Quill's for longer than the write takes
/// (ADR 0002).
fn wrote(path: &Path, text: &str) -> io::Result<Version> {
    file::write(path, text)?;
    watch::version(path).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("{} was written and is not there", path.display()),
        )
    })
}

/// `caret` inside `text`: clamped to its length and back to a character
/// boundary, so a reload that shortened the text still lands somewhere.
fn within(text: &str, caret: usize) -> usize {
    let mut caret = caret.min(text.len());
    while !text.is_char_boundary(caret) {
        caret -= 1;
    }
    caret
}

/// The file name an untitled Document's first save gives it.
///
/// The oracle's rule (`legacy/app/js/files.js` `deriveTitle`, `safeName` and
/// `dispName`): the first non-empty line without its heading hashes, list or
/// quote marker and inline Markup, whitespace collapsed, cut to
/// [`TITLE_CHARS`]; then [`named`] makes a file name of what is left.
#[must_use]
pub fn first_save_name(text: &str) -> String {
    named(&derived_title(text))
}

/// `typed` as a file name.
///
/// The characters no file name may hold dropped, leading dots dropped, and
/// [`UNTITLED`] where nothing survives; a name that already ends in an
/// extension the Library lists keeps it, and everything else is given `.md`.
///
/// What a rename types and what a first line derives are the same question
/// asked twice ([`crate::library::Library::rename`]), so both come through
/// here.
#[must_use]
pub fn named(typed: &str) -> String {
    let name = safe_name(typed);
    if library::listed(Path::new(&name)) {
        name
    } else {
        format!("{name}.{DEFAULT_EXTENSION}")
    }
}

/// `name` in `folder`, suffixed until nothing there has it.
///
/// The oracle's suffix (`legacy/app/js/files.js` `uniqueName`): `The Lamp.md`,
/// then `The Lamp 2.md`, and on to [`SUFFIXES`], compared without case as the
/// oracle compares. Reads the folder once, so the walk is its entries plus at
/// most [`SUFFIXES`] lookups; a folder that cannot be read is an empty one,
/// and the write that follows reports what is really wrong with it.
#[must_use]
pub fn unique_in(folder: &Path, name: &str) -> PathBuf {
    let taken: HashSet<String> = fs::read_dir(folder)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().to_lowercase())
        .collect();
    if !taken.contains(&name.to_lowercase()) {
        return folder.join(name);
    }
    let (stem, extension) = split_extension(name);
    for nth in 2..=SUFFIXES {
        let candidate = format!("{stem} {nth}{extension}");
        if !taken.contains(&candidate.to_lowercase()) {
            return folder.join(candidate);
        }
    }
    folder.join(name)
}

/// `name` as its stem and its extension with the dot, where the extension is
/// one the Library lists; everything else is all stem.
pub(crate) fn split_extension(name: &str) -> (&str, &str) {
    if !library::listed(Path::new(name)) {
        return (name, "");
    }
    match name.rfind('.') {
        Some(dot) => name.split_at(dot),
        None => (name, ""),
    }
}

/// The first line of `text` as a title: hashes, list or quote marker and
/// inline Markup gone, whitespace collapsed, cut to [`TITLE_CHARS`].
fn derived_title(text: &str) -> String {
    let Some(line) = text.lines().find(|line| !line.trim().is_empty()) else {
        return String::new();
    };
    let line = strip_marker(unheaded(line));
    let bare: String = line.chars().filter(|c| !"*_`~[]".contains(*c)).collect();
    let collapsed = bare.split_whitespace().collect::<Vec<_>>().join(" ");
    collapsed.chars().take(TITLE_CHARS).collect::<String>()
}

/// `line` without a leading ATX heading marker: one to six hashes and the
/// whitespace after them, which is what makes them a heading. The leading
/// whitespace goes either way, so that a heading and a plain line answer the
/// same shape.
///
/// The one reading of a heading marker in the engine: a derived name
/// ([`derived_title`]) and a search's excerpt
/// ([`crate::library::Library::search`]) both drop it, and they drop the same
/// thing.
pub(crate) fn unheaded(line: &str) -> &str {
    let rest = line.trim_start();
    let hashes = rest.chars().take_while(|letter| *letter == '#').count();
    if (1..=6).contains(&hashes) {
        let after = &rest[hashes..];
        if after.starts_with(char::is_whitespace) {
            return after.trim_start();
        }
    }
    rest
}

/// `line` without a leading list or block quote marker and the whitespace
/// after it.
fn strip_marker(line: &str) -> &str {
    let mut chars = line.chars();
    match (chars.next(), chars.next()) {
        (Some('-' | '*' | '+' | '>'), Some(space)) if space.is_whitespace() => {
            line[1..].trim_start()
        }
        _ => line,
    }
}

/// `title` as a file name: the characters no file name may hold dropped,
/// leading dots dropped, and [`UNTITLED`] where nothing survives.
fn safe_name(title: &str) -> String {
    let kept: String = title
        .chars()
        .filter(|c| {
            !matches!(c, '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|') && !c.is_control()
        })
        .collect();
    let kept = kept.trim_start_matches('.').trim();
    if kept.is_empty() {
        UNTITLED.to_string()
    } else {
        kept.to_string()
    }
}

/// The lines of `after` against `before`, in order, for the app to render.
///
/// A conflict is nearly always a few lines in a long file, so the common head
/// and tail are matched off first and the longest common subsequence is found
/// over what is left: the walk is one cell per pair of *differing* lines,
/// `head..tail` of one text by `head..tail` of the other, and two texts that
/// differ in one line cost their length in comparisons rather than its square.
#[must_use]
pub fn diff(before: &str, after: &str) -> Vec<Line> {
    let old: Vec<&str> = before.lines().collect();
    let new: Vec<&str> = after.lines().collect();
    let head = old
        .iter()
        .zip(&new)
        .take_while(|(one, two)| one == two)
        .count();
    let tail = old[head..]
        .iter()
        .rev()
        .zip(new[head..].iter().rev())
        .take_while(|(one, two)| one == two)
        .count();

    let mut lines: Vec<Line> = old[..head]
        .iter()
        .map(|l| Line::Same((*l).to_string()))
        .collect();
    lines.extend(between(
        &old[head..old.len() - tail],
        &new[head..new.len() - tail],
    ));
    lines.extend(
        old[old.len() - tail..]
            .iter()
            .map(|l| Line::Same((*l).to_string())),
    );
    lines
}

/// The diff of two texts that share no first and no last line: a longest
/// common subsequence over `old.len() * new.len()` cells, walked back into
/// removed, added and same lines in order.
fn between(old: &[&str], new: &[&str]) -> Vec<Line> {
    let stride = new.len() + 1;
    let mut common = vec![0usize; (old.len() + 1) * stride];
    for one in (0..old.len()).rev() {
        for two in (0..new.len()).rev() {
            common[one * stride + two] = if old[one] == new[two] {
                common[(one + 1) * stride + two + 1] + 1
            } else {
                common[(one + 1) * stride + two].max(common[one * stride + two + 1])
            };
        }
    }

    let mut lines = Vec::new();
    let (mut one, mut two) = (0, 0);
    while one < old.len() && two < new.len() {
        if old[one] == new[two] {
            lines.push(Line::Same(old[one].to_string()));
            one += 1;
            two += 1;
        } else if common[(one + 1) * stride + two] >= common[one * stride + two + 1] {
            lines.push(Line::Removed(old[one].to_string()));
            one += 1;
        } else {
            lines.push(Line::Added(new[two].to_string()));
            two += 1;
        }
    }
    lines.extend(old[one..].iter().map(|l| Line::Removed((*l).to_string())));
    lines.extend(new[two..].iter().map(|l| Line::Added((*l).to_string())));
    lines
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc::RecvTimeoutError;
    use std::time::Duration;

    use super::*;
    use crate::watch::Watch;

    /// Long enough for a write of this test's own to come back through the
    /// watcher, as `watch`'s own tests wait.
    const WAIT: Duration = Duration::from_millis(500);

    /// An empty directory of this test's own, named for the test and this
    /// process, because worktrees test concurrently.
    fn scratch(named: &str) -> PathBuf {
        let directory =
            std::env::temp_dir().join(format!("quill-disk-{named}-{}", std::process::id()));
        fs::remove_dir_all(&directory).ok();
        fs::create_dir_all(&directory).expect("makes its own scratch directory");
        directory
    }

    /// What is in `folder`, sorted, so anything left behind shows up.
    fn names_in(folder: &Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(folder)
            .expect("reads a directory this test made")
            .map(|entry| {
                entry
                    .expect("reads an entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        names.sort();
        names
    }

    /// A Document of `text` saved into `folder`.
    fn saved_in(folder: &Path, text: &str) -> Filed {
        let mut filed = Filed::untitled();
        filed.document_mut().reload(text.to_string());
        assert_eq!(
            filed.save_in(folder).expect("writes the first save"),
            Saved::Written
        );
        filed
    }

    #[test]
    fn a_first_line_names_the_file_and_an_empty_document_is_untitled() {
        assert_eq!(
            first_save_name("# The Lamp\n\nIt had been lit.\n"),
            "The Lamp.md"
        );
        assert_eq!(first_save_name(""), "Untitled.md");
        assert_eq!(first_save_name("\n\n   \n"), "Untitled.md");
        assert_eq!(first_save_name("- *A list* item\n"), "A list item.md");
        assert_eq!(first_save_name("> quoted\n"), "quoted.md");
    }

    #[test]
    fn a_name_never_holds_a_character_a_file_name_may_not() {
        assert_eq!(first_save_name("a/b:c*d?e\"f<g>h|i\n"), "abcdefghi.md");
        assert_eq!(first_save_name("...hidden\n"), "hidden.md");
        assert_eq!(first_save_name("///\n"), "Untitled.md");
    }

    #[test]
    fn a_first_line_that_is_already_a_file_name_keeps_its_extension() {
        assert_eq!(first_save_name("notes.txt\n"), "notes.txt");
        assert_eq!(first_save_name("chapter.markdown\n"), "chapter.markdown");
        assert_eq!(first_save_name("version 1.2\n"), "version 1.2.md");
    }

    #[test]
    fn a_name_taken_in_the_folder_gets_the_oracles_suffix() {
        let folder = scratch("collision");
        assert_eq!(
            unique_in(&folder, "The Lamp.md"),
            folder.join("The Lamp.md")
        );
        fs::write(folder.join("The Lamp.md"), "one").expect("writes the first");
        assert_eq!(
            unique_in(&folder, "The Lamp.md"),
            folder.join("The Lamp 2.md")
        );
        fs::write(folder.join("the lamp 2.md"), "two").expect("writes the second");
        assert_eq!(
            unique_in(&folder, "The Lamp.md"),
            folder.join("The Lamp 3.md")
        );
    }

    #[test]
    fn the_name_holds_when_the_first_line_is_edited_later() {
        let folder = scratch("fixed-name");
        let mut filed = saved_in(&folder, "# The Lamp\n");
        assert_eq!(filed.path(), Some(folder.join("The Lamp.md").as_path()));

        filed.document_mut().reload("# The Window\n".to_string());
        assert_eq!(filed.save().expect("saves again"), Saved::Written);
        assert_eq!(filed.path(), Some(folder.join("The Lamp.md").as_path()));
        assert_eq!(names_in(&folder), ["The Lamp.md"]);
    }

    #[test]
    fn a_save_leaves_the_folder_holding_the_one_file() {
        let folder = scratch("nothing-beside");
        let filed = saved_in(&folder, "# The Lamp\n\nIt had been lit.\n");
        assert_eq!(names_in(&folder), ["The Lamp.md"]);
        assert_eq!(
            fs::read_to_string(filed.path().expect("has a file")).expect("reads it back"),
            "# The Lamp\n\nIt had been lit.\n"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_save_keeps_the_permissions_the_file_had() {
        use std::os::unix::fs::PermissionsExt;

        let folder = scratch("permissions");
        let path = folder.join("one.md");
        fs::write(&path, "one\n").expect("writes the file");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).expect("locks it down");

        let mut filed = Filed::open(&path).expect("opens it");
        filed.document_mut().reload("two\n".to_string());
        assert_eq!(filed.save().expect("saves"), Saved::Written);

        let mode = fs::metadata(&path).expect("stats it").permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
        assert_eq!(names_in(&folder), ["one.md"]);
    }

    #[test]
    fn a_failed_save_leaves_the_file_as_it_was_and_no_temporary() {
        let folder = scratch("failed-save");
        let path = folder.join("one.md");
        fs::write(&path, "one\n").expect("writes the file");
        let mut filed = Filed::open(&path).expect("opens it");
        filed.document_mut().reload("two\n".to_string());
        // A directory where the temporary must go: the write fails, and the
        // file it would have replaced is untouched.
        fs::create_dir(file::temporary(&path)).expect("stands in the temporary's way");

        assert!(filed.save().is_err());
        assert_eq!(fs::read_to_string(&path).expect("still there"), "one\n");
        assert_eq!(filed.state(), OnDisk::Named);
    }

    #[test]
    fn an_untitled_document_asks_for_a_folder_before_it_can_be_saved() {
        let mut filed = Filed::untitled();
        assert_eq!(filed.state(), OnDisk::Untitled);
        assert_eq!(filed.save().expect("answers"), Saved::NeedsAFolder);
        assert_eq!(filed.keep().expect("answers"), Saved::NeedsAFolder);
    }

    #[test]
    fn an_outside_write_to_a_clean_document_reloads_it_and_keeps_the_carets_block() {
        let folder = scratch("clean-reload");
        let path = folder.join("one.md");
        fs::write(&path, "# One\n\nThe lamp.\n").expect("writes the file");
        let mut filed = Filed::open(&path).expect("opens it");
        let caret = filed
            .document()
            .text()
            .find("lamp")
            .expect("finds the word");
        let block = filed.document().block_at(caret);

        fs::write(&path, "# One\n\nThe lamp had been lit.\n").expect("writes from outside");
        let noticed = filed.noticed(false, caret);

        assert_eq!(noticed, Noticed::Reloaded(Kept { block, caret }));
        assert_eq!(filed.state(), OnDisk::Named);
        assert_eq!(filed.document().text(), "# One\n\nThe lamp had been lit.\n");
    }

    #[test]
    fn an_outside_write_under_unsaved_edits_refuses_the_save_until_reload_or_keep() {
        let folder = scratch("conflict");
        let path = folder.join("one.md");
        fs::write(&path, "one\n").expect("writes the file");
        let mut filed = Filed::open(&path).expect("opens it");
        filed.document_mut().reload("ours\n".to_string());

        fs::write(&path, "theirs\n").expect("writes from outside");
        assert_eq!(filed.noticed(true, 0), Noticed::Changed);
        assert_eq!(filed.state(), OnDisk::ChangedOnDisk);
        assert!(!filed.autosaves());
        assert_eq!(filed.save().expect("answers"), Saved::Refused);
        assert_eq!(fs::read_to_string(&path).expect("untouched"), "theirs\n");

        assert_eq!(
            filed.reload(0).expect("reloads"),
            Kept {
                block: Some(0),
                caret: 0
            }
        );
        assert_eq!(filed.document().text(), "theirs\n");
        assert_eq!(filed.state(), OnDisk::Named);
        assert!(filed.autosaves());
    }

    #[test]
    fn a_save_on_to_a_file_that_moved_since_is_refused_and_is_a_conflict() {
        let folder = scratch("restat");
        let path = folder.join("one.md");
        fs::write(&path, "one\n").expect("writes the file");
        let mut filed = Filed::open(&path).expect("opens it");
        filed.document_mut().reload("ours\n".to_string());

        // The write from outside, and no watch event yet: the save is the
        // first thing that looks at the file, and it looks before it writes.
        fs::write(&path, "theirs and longer\n").expect("writes from outside");
        assert_eq!(filed.save().expect("answers"), Saved::Refused);
        assert_eq!(
            fs::read_to_string(&path).expect("reads it back"),
            "theirs and longer\n",
            "the file is as the other writer left it"
        );
        assert_eq!(filed.state(), OnDisk::ChangedOnDisk);
        assert!(!filed.autosaves());

        // And the event, when it arrives, finds the conflict already standing.
        assert_eq!(filed.noticed(true, 0), Noticed::Changed);
        assert_eq!(filed.keep().expect("keeps ours"), Saved::Written);
        assert_eq!(fs::read_to_string(&path).expect("reads it back"), "ours\n");
    }

    #[test]
    fn an_outside_rename_in_one_drain_is_followed_rather_than_read_as_a_delete() {
        let folder = scratch("followed");
        let from = folder.join("one.md");
        let to = folder.join("two.md");
        fs::write(&from, "one\n").expect("writes the file");
        let mut filed = Filed::open(&from).expect("opens it");

        fs::rename(&from, &to).expect("renames it from outside");
        // What one drain of the watch answers for: the path the file left and
        // the path it took.
        assert!(filed.followed(&[from.clone(), to.clone()]), "it followed");
        assert_eq!(filed.path(), Some(to.as_path()));
        assert_eq!(filed.state(), OnDisk::Named);
        assert_eq!(filed.noticed(false, 0), Noticed::Unchanged);

        // A drain that holds no file this one could be is the delete it was.
        let gone = folder.join("three.md");
        fs::rename(&to, &gone).expect("moves it away again");
        fs::write(
            folder.join("elsewhere.md"),
            "a longer line of another file\n",
        )
        .expect("writes another file");
        assert!(
            !filed.followed(&[to.clone(), folder.join("elsewhere.md")]),
            "nothing in the drain is this file"
        );
        assert_eq!(filed.path(), Some(to.as_path()));
        assert_eq!(filed.noticed(false, 0), Noticed::Deleted);
        assert_eq!(filed.state(), OnDisk::DeletedOnDisk);
        fs::remove_dir_all(&folder).ok();
    }

    #[test]
    fn keep_writes_the_editors_text_over_the_file() {
        let folder = scratch("keep");
        let path = folder.join("one.md");
        fs::write(&path, "one\n").expect("writes the file");
        let mut filed = Filed::open(&path).expect("opens it");
        filed.document_mut().reload("ours\n".to_string());
        fs::write(&path, "theirs\n").expect("writes from outside");
        assert_eq!(filed.noticed(true, 0), Noticed::Changed);

        assert_eq!(filed.keep().expect("keeps ours"), Saved::Written);
        assert_eq!(filed.state(), OnDisk::Named);
        assert_eq!(fs::read_to_string(&path).expect("reads it back"), "ours\n");
        assert_eq!(filed.noticed(false, 0), Noticed::Unchanged);
    }

    #[test]
    fn an_outside_rename_follows_the_file() {
        let folder = scratch("rename");
        let path = folder.join("one.md");
        fs::write(&path, "one\n").expect("writes the file");
        let mut filed = Filed::open(&path).expect("opens it");

        let moved = folder.join("two.md");
        fs::rename(&path, &moved).expect("renames it from outside");
        filed.moved_to(&moved);

        assert_eq!(filed.path(), Some(moved.as_path()));
        assert_eq!(filed.state(), OnDisk::Named);
        assert_eq!(filed.noticed(false, 0), Noticed::Unchanged);
    }

    #[test]
    fn an_outside_delete_is_recreated_by_the_next_save() {
        let folder = scratch("delete");
        let path = folder.join("one.md");
        fs::write(&path, "one\n").expect("writes the file");
        let mut filed = Filed::open(&path).expect("opens it");
        filed.document_mut().reload("ours\n".to_string());

        fs::remove_file(&path).expect("deletes it from outside");
        assert_eq!(filed.noticed(false, 0), Noticed::Deleted);
        assert_eq!(filed.state(), OnDisk::DeletedOnDisk);
        assert!(!filed.autosaves());

        assert_eq!(filed.save().expect("recreates the file"), Saved::Written);
        assert_eq!(filed.state(), OnDisk::Named);
        assert_eq!(fs::read_to_string(&path).expect("reads it back"), "ours\n");
        assert_eq!(names_in(&folder), ["one.md"]);
    }

    #[test]
    fn the_watch_reports_the_write_the_document_then_notices() {
        let folder = scratch("watched");
        let path = folder.join("one.md");
        fs::write(&path, "one\n").expect("writes the file");
        let mut filed = Filed::open(&path).expect("opens it");
        let (_watch, events) = Watch::on(&path).expect("watches the file");

        fs::write(&path, "one and two\n").expect("writes from outside");

        assert_eq!(events.recv_timeout(WAIT), Ok(path.clone()));
        assert_eq!(
            filed.noticed(false, 0),
            Noticed::Reloaded(Kept {
                block: Some(0),
                caret: 0
            })
        );
        assert_eq!(filed.document().text(), "one and two\n");

        // And Quill's own save is a version the Document already knows, so the
        // event it raises is not a change.
        assert_eq!(filed.save().expect("saves"), Saved::Written);
        while events.recv_timeout(WAIT).is_ok() {}
        assert_eq!(events.recv_timeout(WAIT), Err(RecvTimeoutError::Timeout));
        assert_eq!(filed.noticed(false, 0), Noticed::Unchanged);
    }

    #[test]
    fn the_line_diff_lists_the_added_and_the_removed_lines() {
        assert_eq!(
            diff(
                "# One\n\nThe lamp.\nThe end.\n",
                "# One\n\nThe lamp had been lit.\nThe end.\n"
            ),
            vec![
                Line::Same("# One".to_string()),
                Line::Same(String::new()),
                Line::Removed("The lamp.".to_string()),
                Line::Added("The lamp had been lit.".to_string()),
                Line::Same("The end.".to_string()),
            ]
        );
        assert_eq!(diff("one\n", "one\n"), vec![Line::Same("one".to_string())]);
        assert_eq!(diff("", "one\n"), vec![Line::Added("one".to_string())]);
        assert_eq!(diff("one\n", ""), vec![Line::Removed("one".to_string())]);
    }

    #[test]
    fn the_line_diff_walks_only_what_lies_between_the_common_head_and_tail() {
        // The bound the doc states: a one-line change in a long file costs the
        // file in comparisons, not its square, so this returns at once.
        let before: String = (0..20_000).map(|nth| format!("line {nth}\n")).collect();
        let after = before.replace("line 10000\n", "line ten thousand\n");
        let lines = diff(&before, &after);

        assert_eq!(lines.len(), 20_001);
        assert_eq!(lines[10_000], Line::Removed("line 10000".to_string()));
        assert_eq!(lines[10_001], Line::Added("line ten thousand".to_string()));
    }

    #[test]
    fn a_conflicts_diff_reads_the_file_against_the_editors_text() {
        let folder = scratch("against-disk");
        let path = folder.join("one.md");
        fs::write(&path, "one\n").expect("writes the file");
        let mut filed = Filed::open(&path).expect("opens it");
        filed.document_mut().reload("ours\n".to_string());
        fs::write(&path, "theirs\n").expect("writes from outside");

        assert_eq!(filed.text_on_disk().expect("reads the file"), "theirs\n");
        assert_eq!(
            filed.against_disk().expect("diffs it"),
            vec![
                Line::Removed("theirs".to_string()),
                Line::Added("ours".to_string()),
            ]
        );
    }
}
