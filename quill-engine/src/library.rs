//! The Library: the folder trees Quill has been pointed at — its Locations —
//! and the paths Pinned beside them.
//!
//! In memory only — walked when a Location is added and again on launch,
//! watched with `notify`, never persisted as an index, so nothing is left in
//! the writer's folder and there is no stale index to delete
//! ([ADR 0002](../../docs/adr/0002-plain-markdown-documents.md)). Settings
//! remember the Locations and the Pinned paths, and hand them here as plain
//! paths; state remembers recents and per-Document caret positions. The Library
//! is shared by all windows.
//!
//! A tree holds every folder under a Location and every file named `.md`,
//! `.markdown`, `.txt` or `.text`, dot-entries among them: a dot-entry is held
//! and flagged ([`File::hidden`], [`Folder::hidden`]), and the show-hidden
//! setting is answered where the tree is read rather than where it is built, so
//! turning it on is another [`Library::shown`] and not another walk.
//!
//! A Location joins the settings watch as a tree
//! ([`crate::watch::Watch::add_tree`]) — the architecture's "the Documents and
//! the Library join the same watch" — and the app hands every path that arrives
//! under a Location's root to [`Library::patch`], which re-stats it and moves
//! the one row it names. The Library owns no watch itself: the engine has no
//! `glib` (ADR 0008), so draining the receiver is the app's.
//!
//! Search over the shown tree is one function, [`Library::search`]: a fuzzy
//! matcher over names — the query's letters in order with gaps, scored by
//! tightness — and a matcher over contents that wants every word of the query
//! somewhere in the file's text. That text is read live from disk and held for
//! the session by write time in [`Contents`], so a second query over a tree
//! nothing has touched reads nothing. [`files`] is the seam search reads the
//! shown tree through, so it answers what the writer can see, in their order.
//!
//! [`files`]: Library::files

use std::cmp::{Ordering, Reverse};
use std::collections::BTreeMap;
use std::fs;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// The file names the Library lists, without their dot.
///
/// The Library spec's four. `legacy/app/js/files.js` also allowed `mdown`,
/// which the spec drops; the comparison is case-insensitive there and here.
const EXTENSIONS: [&str; 4] = ["md", "markdown", "txt", "text"];

/// Whether `path` is a file the Library lists, whatever the case of its
/// extension.
fn listed(path: &Path) -> bool {
    let Some(extension) = path.extension().and_then(|extension| extension.to_str()) else {
        return false;
    };
    EXTENSIONS
        .iter()
        .any(|listed| extension.eq_ignore_ascii_case(listed))
}

/// When `path` was last written, where the file system says, and `None` where
/// it will not say.
fn modified(path: &Path) -> Option<SystemTime> {
    fs::metadata(path)
        .ok()
        .and_then(|metadata| metadata.modified().ok())
}

/// The display name of `path`: its last component, held rather than derived
/// because the sort and the sidebar both ask for it. A name that is not UTF-8
/// is held as `to_string_lossy` writes it, which is what a sidebar would draw.
fn name_of(path: &Path) -> String {
    path.file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned()
}

/// One file under a Location, as the tree holds it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct File {
    /// Where it is, under the Location's root as the root was added.
    path: PathBuf,
    /// Its name on disk, extension and all.
    name: String,
    /// When it was last written; `None` where the file system would not say,
    /// which sorts last under [`Sort::Date`].
    modified: Option<SystemTime>,
    /// Whether it is hidden: its own name, or any folder between it and the
    /// Location's root, begins with a dot.
    hidden: bool,
}

impl File {
    /// Where it is.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Its name on disk, extension and all.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// When it was last written.
    #[must_use]
    pub fn modified(&self) -> Option<SystemTime> {
        self.modified
    }

    /// Whether it is hidden — held either way, and shown only when the view
    /// says so ([`View::show_hidden`]).
    #[must_use]
    pub fn hidden(&self) -> bool {
        self.hidden
    }

    /// What a sort compares it by.
    fn key(&self) -> Key<'_> {
        Key {
            name: &self.name,
            modified: self.modified,
        }
    }
}

/// One folder under a Location — or the Location's root itself — and everything
/// the walk found in it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Folder {
    /// Where it is, under the Location's root as the root was added.
    path: PathBuf,
    /// Its name on disk.
    name: String,
    /// When it was last written.
    modified: Option<SystemTime>,
    /// Whether it is hidden: its own name, or any folder between it and the
    /// Location's root, begins with a dot.
    hidden: bool,
    /// The folders in it, unsorted: an order is a view's, not a tree's.
    folders: Vec<Folder>,
    /// The listed files in it, unsorted, for the same reason.
    files: Vec<File>,
}

impl Folder {
    /// Where it is.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Its name on disk.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// When it was last written.
    #[must_use]
    pub fn modified(&self) -> Option<SystemTime> {
        self.modified
    }

    /// Whether it is hidden — held either way, and shown only when the view
    /// says so ([`View::show_hidden`]).
    #[must_use]
    pub fn hidden(&self) -> bool {
        self.hidden
    }

    /// What a sort compares it by.
    fn key(&self) -> Key<'_> {
        Key {
            name: &self.name,
            modified: self.modified,
        }
    }

    /// The child folder named `name`, where the walk found one.
    fn folder(&self, name: &std::ffi::OsStr) -> Option<&Folder> {
        self.folders
            .iter()
            .find(|folder| folder.path.file_name() == Some(name))
    }

    /// The child file named `name`, where the walk found one.
    fn file(&self, name: &std::ffi::OsStr) -> Option<&File> {
        self.files
            .iter()
            .find(|file| file.path.file_name() == Some(name))
    }
}

/// One folder the writer pointed Quill at, and the tree walked from it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Location {
    /// Its root, as the writer named it.
    root: PathBuf,
    /// Everything under the root, walked.
    tree: Folder,
}

impl Location {
    /// The folder the writer pointed Quill at, which is what heads its section.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }
}

/// How the files of a folder are ordered.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Sort {
    /// By when each was last written, newest first. The default, and the
    /// oracle's.
    #[default]
    Date,
    /// By name, case-insensitively.
    Name,
}

/// What the tree is read through: the settings that decide what is shown and
/// in what order, which the writer moves without the tree being walked again.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct View {
    /// Whether dot-entries are shown; they are held either way.
    pub show_hidden: bool,
    /// The order files and folders are drawn in.
    pub sort: Sort,
}

impl View {
    /// Whether an entry flagged `hidden` is drawn.
    fn shows(&self, hidden: bool) -> bool {
        self.show_hidden || !hidden
    }
}

/// What a sort compares: the display name and the write time of one entry,
/// travelling together because either sort falls back on the other.
#[derive(Clone, Copy, Debug)]
struct Key<'a> {
    /// The entry's name on disk.
    name: &'a str,
    /// When it was last written.
    modified: Option<SystemTime>,
}

/// Orders two names case-insensitively, without a lower-cased copy of either.
fn folded(one: &str, other: &str) -> Ordering {
    one.chars()
        .flat_map(char::to_lowercase)
        .cmp(other.chars().flat_map(char::to_lowercase))
}

/// Orders two entries under `sort`: [`Sort::Name`] is case-insensitive, and
/// [`Sort::Date`] is newest first with an unreadable write time last. Either
/// falls back on the other, so that the order is total and two files saved in
/// the same second do not swap between two queries.
fn order(sort: Sort, one: Key<'_>, other: Key<'_>) -> Ordering {
    match sort {
        Sort::Name => folded(one.name, other.name).then_with(|| other.modified.cmp(&one.modified)),
        Sort::Date => other
            .modified
            .cmp(&one.modified)
            .then_with(|| folded(one.name, other.name)),
    }
}

/// One entry of a shown tree, at its depth under the section's root.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Row<'a> {
    /// A folder, drawn before any file beside it, with its own rows beneath.
    Folder {
        /// The folder.
        folder: &'a Folder,
        /// How many folders lie between it and the section's root.
        depth: usize,
    },
    /// A file.
    File {
        /// The file.
        file: &'a File,
        /// How many folders lie between it and the section's root.
        depth: usize,
    },
}

impl<'a> Row<'a> {
    /// Where the entry is. It outlives the row, which is the tree's rather
    /// than the row's.
    #[must_use]
    pub fn path(&self) -> &'a Path {
        match self {
            Row::Folder { folder, .. } => &folder.path,
            Row::File { file, .. } => &file.path,
        }
    }

    /// Its name on disk.
    #[must_use]
    pub fn name(&self) -> &'a str {
        match self {
            Row::Folder { folder, .. } => &folder.name,
            Row::File { file, .. } => &file.name,
        }
    }

    /// How many folders lie between it and the section's root.
    #[must_use]
    pub fn depth(&self) -> usize {
        match self {
            Row::Folder { depth, .. } | Row::File { depth, .. } => *depth,
        }
    }
}

/// One Location as the sidebar draws it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Section<'a> {
    /// The Location's root, which heads the section.
    pub root: &'a Path,
    /// What is shown beneath it, in the order it is drawn.
    pub rows: Vec<Row<'a>>,
}

/// Whether a path handed to [`Library::pin`] was taken.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[must_use = "a path under no Location is not pinned"]
pub enum Pin {
    /// The path is under a Location, and is pinned at the end of the list.
    Held,
    /// The path is under no Location, so there is no tree to answer it from
    /// and it was not pinned.
    Outside,
}

/// What a query matched in a file.
///
/// Ordered the way results are: a name hit before a content hit, whatever
/// either scored.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Hit {
    /// The file's name, matched fuzzily.
    Name,
    /// The file's text, matched by every word of the query.
    Content,
}

/// The text around a content match, and where the match sits inside it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snippet {
    /// The window of the file's flattened text the match falls in, opened and
    /// closed with `…` where it was cut into.
    text: String,
    /// Where the match is in `text`, in bytes.
    at: Range<usize>,
}

impl Snippet {
    /// The window of text the match falls in.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Where the match is in [`text`](Snippet::text), in bytes, which is what
    /// a sidebar draws marked.
    #[must_use]
    pub fn at(&self) -> Range<usize> {
        self.at.clone()
    }

    /// The matched text itself.
    #[must_use]
    pub fn matched(&self) -> &str {
        &self.text[self.at.clone()]
    }
}

/// One file a query matched, and what matched in it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Found<'a> {
    /// The file, as the tree holds it.
    file: &'a File,
    /// What matched.
    hit: Hit,
    /// How well it matched: higher is better, comparable within a [`Hit`] and
    /// not across one, because a name hit is listed first whatever either
    /// scored.
    score: i32,
    /// The text around the first content match; `None` for a name hit, which
    /// is drawn as the name it matched.
    snippet: Option<Snippet>,
}

impl<'a> Found<'a> {
    /// The file that matched.
    #[must_use]
    pub fn file(&self) -> &'a File {
        self.file
    }

    /// What matched in it.
    #[must_use]
    pub fn hit(&self) -> Hit {
        self.hit
    }

    /// How well it matched.
    #[must_use]
    pub fn score(&self) -> i32 {
        self.score
    }

    /// The text around the first content match, for a content hit.
    #[must_use]
    pub fn snippet(&self) -> Option<&Snippet> {
        self.snippet.as_ref()
    }
}

/// The file texts search has read, held for the session.
///
/// A file is read when it is first searched and again when the tree says its
/// write time has moved, so a query over a tree nothing has touched reads
/// nothing and a file the file system will not date is read once. The app
/// keeps one beside the [`Library`] and hands it to every
/// [`search`](Library::search); dropping it is forgetting every text.
#[derive(Clone, Debug, Default)]
pub struct Contents {
    /// The flattened text of each file read, under the path it was read from.
    texts: BTreeMap<PathBuf, Cached>,
    /// How many times a file has been read from disk, which is what pins the
    /// cost of a search.
    reads: usize,
}

impl Contents {
    /// A cache holding nothing.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// How many times it has gone to disk since it was made.
    #[must_use]
    pub fn reads(&self) -> usize {
        self.reads
    }

    /// The flattened text of `file`, read where the cache does not already
    /// hold it at the write time the tree last saw. A file that cannot be read
    /// is held as no text, so an unreadable file is not read again either.
    fn text(&mut self, file: &File) -> &str {
        let held = self
            .texts
            .get(file.path())
            .is_some_and(|cached| cached.modified == file.modified());
        if !held {
            self.reads += 1;
            let text = flatten(&fs::read_to_string(file.path()).unwrap_or_default());
            self.texts.insert(
                file.path().to_path_buf(),
                Cached {
                    modified: file.modified(),
                    text,
                },
            );
        }
        &self.texts[file.path()].text
    }
}

/// One file's text as [`Contents`] holds it.
#[derive(Clone, Debug)]
struct Cached {
    /// The write time the text was read at.
    modified: Option<SystemTime>,
    /// The text, flattened.
    text: String,
}

/// The Locations Quill is showing and the paths pinned beside them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Library {
    /// The Locations, in the order they were added, which is the order their
    /// sections are drawn in.
    locations: Vec<Location>,
    /// The pinned paths, in the order they were pinned.
    pinned: Vec<PathBuf>,
}

impl Library {
    /// An empty Library: no Location, nothing pinned.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The Library the settings describe, every Location walked now and every
    /// pinned path that no Location holds dropped.
    ///
    /// The settings are read by the caller and arrive here as plain paths, so
    /// that the model is testable with no settings file and the settings module
    /// stays the one place a file is parsed.
    #[must_use]
    pub fn open(locations: &[PathBuf], pinned: &[PathBuf]) -> Self {
        let mut library = Self::new();
        for root in locations {
            library.add_location(root);
        }
        for path in pinned {
            let _ = library.pin(path);
        }
        library
    }

    /// The Locations, in the order they were added.
    #[must_use]
    pub fn locations(&self) -> &[Location] {
        &self.locations
    }

    /// Adds `root` as a Location and walks it, answering whether it was added.
    ///
    /// A folder that is already a Location, and a path that is not a folder,
    /// are both refused and change nothing. Walking is one `read_dir` per
    /// folder in the tree ([`Library::patch`] keeps it that way afterwards).
    pub fn add_location(&mut self, root: &Path) -> bool {
        if !root.is_dir() || self.locations.iter().any(|location| location.root == root) {
            return false;
        }
        let hidden = name_of(root).starts_with('.');
        let mut reads = 0;
        let tree = walk(root, hidden, &mut reads);
        self.locations.push(Location {
            root: root.to_path_buf(),
            tree,
        });
        true
    }

    /// Drops the Location rooted at `root` and its tree, answering whether
    /// there was one. Nothing on disk is touched, and a pinned path under it
    /// stays pinned, answering nothing until the Location comes back.
    pub fn remove_location(&mut self, root: &Path) -> bool {
        let before = self.locations.len();
        self.locations.retain(|location| location.root != root);
        self.locations.len() != before
    }

    /// The Location holding `path`, which is the one whose root it is under.
    fn holder(&self, path: &Path) -> Option<&Location> {
        self.locations
            .iter()
            .find(|location| path.starts_with(&location.root))
    }

    /// The row at `path` — a folder or a file — where a Location's tree holds
    /// one. A Location's own root answers as a folder row at depth zero.
    #[must_use]
    pub fn at(&self, path: &Path) -> Option<Row<'_>> {
        let location = self.holder(path)?;
        let rest = path.strip_prefix(&location.root).ok()?;
        descend(&location.tree, rest)
    }

    /// Every Location as a section, in the order they were added, each read
    /// through `view`.
    ///
    /// The walk is over the rows shown, and each folder shown is sorted once.
    #[must_use]
    pub fn shown(&self, view: &View) -> Vec<Section<'_>> {
        self.locations
            .iter()
            .map(|location| Section {
                root: &location.root,
                rows: rows_of(&location.tree, view, 0),
            })
            .collect()
    }

    /// What is shown beneath the folder at `path`, in the order it is drawn;
    /// empty where no Location holds a folder there.
    #[must_use]
    pub fn rows(&self, path: &Path, view: &View) -> Vec<Row<'_>> {
        match self.at(path) {
            Some(Row::Folder { folder, .. }) => rows_of(folder, view, 0),
            _ => Vec::new(),
        }
    }

    /// The pinned paths, in the order they were pinned.
    #[must_use]
    pub fn pinned(&self) -> &[PathBuf] {
        &self.pinned
    }

    /// Pins `path`, at the end of the list, and says whether it was taken: a
    /// path under no Location is refused, because Pinned answers from the
    /// Locations' trees and there would be no tree to answer it from. Pinning
    /// a path already pinned leaves it where it is.
    pub fn pin(&mut self, path: &Path) -> Pin {
        if self.at(path).is_none() {
            return Pin::Outside;
        }
        if !self.pinned.iter().any(|pinned| pinned == path) {
            self.pinned.push(path.to_path_buf());
        }
        Pin::Held
    }

    /// Unpins `path`, answering whether it was pinned.
    pub fn unpin(&mut self, path: &Path) -> bool {
        let before = self.pinned.len();
        self.pinned.retain(|pinned| pinned != path);
        self.pinned.len() != before
    }

    /// The Pinned section: each pinned path as a row of its own at depth zero,
    /// in the order it was pinned, a pinned folder followed by its subtree from
    /// the Location's tree — the same rows [`Library::rows`] gives, one deeper.
    ///
    /// A pinned entry is drawn whether or not it is hidden, because pinning it
    /// was the writer saying so; what is beneath a pinned folder is read
    /// through `view` like anything else. A pinned path no Location holds any
    /// more answers nothing.
    #[must_use]
    pub fn pinned_rows(&self, view: &View) -> Vec<Row<'_>> {
        let mut rows = Vec::new();
        for path in &self.pinned {
            let Some(row) = self.at(path) else {
                continue;
            };
            rows.push(row);
            if let Row::Folder { folder, .. } = row {
                rows.extend(rows_of(folder, view, 1));
            }
        }
        rows
    }

    /// Every shown file of every Location, in the order the sections draw them.
    ///
    /// The seam search reads the shown tree through, so that a search answers
    /// what the writer can see and in the order they see it.
    pub fn files(&self, view: &View) -> impl Iterator<Item = &File> {
        let mut files = Vec::new();
        for section in self.shown(view) {
            for row in section.rows {
                if let Row::File { file, .. } = row {
                    files.push(file);
                }
            }
        }
        files.into_iter()
    }

    /// Every shown file `query` matched, name hits first and each kind by
    /// score.
    ///
    /// The name matcher is fuzzy: the query's letters, whitespace dropped, in
    /// order with gaps, case-insensitive and scored by tightness, so `sst`
    /// finds `sea-storm.md` and ranks it above a looser name. The content
    /// matcher wants every whitespace-separated word of the query as a
    /// case-insensitive substring of the file's text, read live from disk
    /// through `contents` and flattened the way a row shows it, and marks the
    /// first match in a [`Snippet`]. A file whose name matched is a name hit
    /// and its text is left unread. An empty query matches nothing, which is
    /// the sidebar drawing its tree rather than a result list.
    ///
    /// The bound is one pass over the shown files, and one read of each file
    /// whose name did not match and whose write time `contents` has not read
    /// at already — so a second query over an untouched tree reads nothing.
    #[must_use]
    pub fn search(&self, query: &str, view: &View, contents: &mut Contents) -> Vec<Found<'_>> {
        let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
        if words.is_empty() {
            return Vec::new();
        }
        let letters: String = words.concat();
        let mut found = Vec::new();
        for file in self.files(view) {
            if let Some(score) = fuzzy(&letters, &file.name.to_lowercase()) {
                found.push(Found {
                    file,
                    hit: Hit::Name,
                    score,
                    snippet: None,
                });
                continue;
            }
            let text = contents.text(file);
            let Some(at) = earliest_match(&words, text) else {
                continue;
            };
            // Earlier is better, and a content score is never above zero, so
            // the two kinds never share a scale — which is why they are sorted
            // by kind first.
            let score = -i32::try_from(at.start).unwrap_or(i32::MAX);
            let snippet = Some(windowed(text, &at));
            found.push(Found {
                file,
                hit: Hit::Content,
                score,
                snippet,
            });
        }
        // A stable sort, so files that scored alike stay in the view's order.
        found.sort_by_key(|found| (found.hit, Reverse(found.score)));
        found
    }

    /// Takes in what happened at `path` — a watch event, under the Location it
    /// names — and answers whether the tree moved, which is the app's signal to
    /// draw the sidebar again.
    ///
    /// One `stat` of `path`, and one `read_dir` per folder only where a folder
    /// appeared there and has to be walked in; a rename arrives as two paths
    /// and is two patches, the old one gone and the new one added.
    pub fn patch(&mut self, path: &Path) -> bool {
        let Some(location) = self
            .locations
            .iter_mut()
            .find(|location| path.starts_with(&location.root))
        else {
            return false;
        };
        let Ok(rest) = path.strip_prefix(&location.root) else {
            return false;
        };
        patch_at(&mut location.tree, rest)
    }
}

/// Walks `directory` into a [`Folder`]: every folder under it and every listed
/// file, dot-entries held and flagged.
///
/// The bound is one `read_dir` per folder in the tree and one `stat` per entry
/// in it, counted in `reads`, which is what pins the cost. A folder that cannot
/// be read lists nothing rather than failing the walk, and a symbolic link to a
/// folder is not walked into: a link back up the tree would be a walk with no
/// end. A link to a file is listed like the file it names.
fn walk(directory: &Path, hidden: bool, reads: &mut usize) -> Folder {
    *reads += 1;
    let mut folder = Folder {
        path: directory.to_path_buf(),
        name: name_of(directory),
        modified: modified(directory),
        hidden,
        folders: Vec::new(),
        files: Vec::new(),
    };
    let Ok(entries) = fs::read_dir(directory) else {
        return folder;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = name_of(&path);
        let hidden = hidden || name.starts_with('.');
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_dir() {
            folder.folders.push(walk(&path, hidden, reads));
        } else if listed(&path) {
            folder.files.push(File {
                modified: modified(&path),
                path,
                name,
                hidden,
            });
        }
    }
    folder
}

/// The row at `rest` under `folder`, where the tree holds one; `rest` empty is
/// the folder itself.
fn descend<'a>(folder: &'a Folder, rest: &Path) -> Option<Row<'a>> {
    let mut here = folder;
    let mut components = rest.components();
    loop {
        let Some(component) = components.next() else {
            return Some(Row::Folder {
                folder: here,
                depth: 0,
            });
        };
        let name = component.as_os_str();
        if let Some(below) = here.folder(name) {
            here = below;
            continue;
        }
        let file = here.file(name)?;
        return components
            .next()
            .is_none()
            .then_some(Row::File { file, depth: 0 });
    }
}

/// What is shown under `folder`, at `depth` and deeper: the folders it holds
/// before the files beside them, each folder followed by its own rows, each
/// group in `view`'s order.
///
/// The walk is over the shown entries, and sorts each shown folder once.
fn rows_of<'a>(folder: &'a Folder, view: &View, depth: usize) -> Vec<Row<'a>> {
    let mut folders: Vec<&Folder> = folder
        .folders
        .iter()
        .filter(|below| view.shows(below.hidden))
        .collect();
    folders.sort_by(|one, other| order(view.sort, one.key(), other.key()));
    let mut files: Vec<&File> = folder
        .files
        .iter()
        .filter(|file| view.shows(file.hidden))
        .collect();
    files.sort_by(|one, other| order(view.sort, one.key(), other.key()));
    let mut rows = Vec::new();
    for below in folders {
        rows.push(Row::Folder {
            folder: below,
            depth,
        });
        rows.extend(rows_of(below, view, depth + 1));
    }
    rows.extend(files.into_iter().map(|file| Row::File { file, depth }));
    rows
}

/// Moves the tree under `folder` to agree with the disk at `rest`, answering
/// whether anything moved.
fn patch_at(folder: &mut Folder, rest: &Path) -> bool {
    let mut components = rest.components();
    let Some(component) = components.next() else {
        // The Location's own root. What happened inside it arrives as its own
        // path, so the only question here is whether the root is still there;
        // a folder already held is never walked again, because reading a
        // folder is itself an event under the watch — `notify`'s inotify mask
        // carries `OPEN` — and a walk answering its own events would have no
        // end.
        if folder.path.is_dir() {
            return false;
        }
        let moved = !folder.folders.is_empty() || !folder.files.is_empty();
        folder.folders.clear();
        folder.files.clear();
        return moved;
    };
    let name = component.as_os_str().to_os_string();
    let rest = components.as_path();
    if !rest.as_os_str().is_empty()
        && let Some(below) = folder
            .folders
            .iter_mut()
            .find(|below| below.path.file_name() == Some(&name))
    {
        return patch_at(below, rest);
    }
    // Either the entry itself changed, or the folder between it and here is
    // one the tree has never seen: both are answered by looking at what is at
    // that name now, which walks a new folder in with everything under it.
    patch_entry(folder, &name)
}

/// Moves the entry named `name` under `folder` to agree with the disk,
/// answering whether anything moved.
fn patch_entry(folder: &mut Folder, name: &std::ffi::OsStr) -> bool {
    let path = folder.path.join(name);
    let hidden = folder.hidden || name.to_string_lossy().starts_with('.');
    let metadata = fs::metadata(&path);
    match metadata {
        Ok(metadata) if metadata.is_dir() => {
            if folder.folder(name).is_some() {
                // A folder the tree already holds; what happened inside it
                // arrives as its own path.
                return false;
            }
            drop_entry(folder, name);
            let mut reads = 0;
            folder.folders.push(walk(&path, hidden, &mut reads));
            true
        }
        Ok(metadata) if listed(&path) => {
            let modified = metadata.modified().ok();
            if let Some(file) = folder
                .files
                .iter_mut()
                .find(|file| file.path.file_name() == Some(name))
            {
                let moved = file.modified != modified;
                file.modified = modified;
                return moved;
            }
            drop_entry(folder, name);
            folder.files.push(File {
                path,
                name: name.to_string_lossy().into_owned(),
                modified,
                hidden,
            });
            true
        }
        // Gone, or a file whose name the Library does not list — a rename from
        // `notes.md` to `notes.rs` is the second, and leaves no row either.
        _ => drop_entry(folder, name),
    }
}

/// Takes the entry named `name` out of `folder`, answering whether one was
/// there.
fn drop_entry(folder: &mut Folder, name: &std::ffi::OsStr) -> bool {
    let before = folder.folders.len() + folder.files.len();
    folder
        .folders
        .retain(|below| below.path.file_name() != Some(name));
    folder
        .files
        .retain(|file| file.path.file_name() != Some(name));
    folder.folders.len() + folder.files.len() != before
}

/// What one matched letter of a fuzzy name match is worth before its gap is
/// charged.
const MATCH: i32 = 1;
/// What the name's first letter is worth, matched.
const START: i32 = 12;
/// What a letter just after a separator is worth: the writer typing the
/// initials of a hyphenated name is the case the score is shaped for.
const BOUNDARY: i32 = 8;
/// What a letter just after the one before it is worth: an unbroken run is
/// what tightness means.
const ADJACENT: i32 = 10;
/// What one skipped character costs.
const GAP: i32 = 1;
/// The most a single gap is charged, so one long gap does not sink a name
/// that is otherwise tight.
const GAP_CAP: i32 = 6;

/// Whether `letter` opens a word in a file name.
fn separator(letter: char) -> bool {
    matches!(letter, '-' | '_' | '.' | '/') || letter.is_whitespace()
}

/// What matching a query letter at `index` of `letters` scores, having matched
/// the letter before it at `before` — `None` where it is the query's first.
fn scored(letters: &[char], before: Option<usize>, index: usize) -> i32 {
    let gap = match before {
        Some(before) => index - before - 1,
        None => index,
    };
    let bonus = if gap == 0 && before.is_some() {
        ADJACENT
    } else if index == 0 {
        START
    } else if separator(letters[index - 1]) {
        BOUNDARY
    } else {
        0
    };
    let charged = i32::try_from(gap).unwrap_or(i32::MAX).min(GAP_CAP);
    MATCH + bonus - GAP * charged
}

/// How tightly `query` — lowercase, no whitespace — falls through `name`, also
/// lowercase, or `None` where the query's letters are not all there in order.
///
/// Every letter scores [`MATCH`]; the name's first scores [`START`], one just
/// after a separator [`BOUNDARY`], one just after the letter before it
/// [`ADJACENT`], and every character skipped costs [`GAP`] up to [`GAP_CAP`].
/// The score is the best-scoring way of laying the query through the name, not
/// the leftmost, because the leftmost cannot tell `sea-storm` from
/// `sunset-stories`: both take `sst` in the same span, and only the tighter
/// laying says which one the writer meant.
///
/// The bound is the query's letters times the square of the name's, over a
/// file name and nothing longer.
fn fuzzy(query: &str, name: &str) -> Option<i32> {
    let letters: Vec<char> = name.chars().collect();
    // The best score for the query so far, laid so that its last letter is the
    // name's letter at each index; `None` where it cannot end there.
    let mut previous: Vec<Option<i32>> = Vec::new();
    for (step, wanted) in query.chars().enumerate() {
        let mut here: Vec<Option<i32>> = vec![None; letters.len()];
        for (index, letter) in letters.iter().enumerate() {
            if *letter != wanted {
                continue;
            }
            if step == 0 {
                here[index] = Some(scored(&letters, None, index));
                continue;
            }
            let mut top: Option<i32> = None;
            for (before, had) in previous.iter().enumerate().take(index) {
                let Some(had) = had else {
                    continue;
                };
                let score = had + scored(&letters, Some(before), index);
                top = Some(top.map_or(score, |held: i32| held.max(score)));
            }
            here[index] = top;
        }
        previous = here;
        if previous.iter().all(Option::is_none) {
            return None;
        }
    }
    previous.into_iter().flatten().max()
}

/// The file's text as a row shows it: a heading's `#` markers and the
/// emphasis marks `*_`, a backtick and `~` dropped, every run of whitespace
/// one space. A hit is then always something the writer can see in the
/// snippet, which is what `legacy/app/js/files.js`'s `excerpt` flattened the
/// text for.
fn flatten(text: &str) -> String {
    let mut flat = String::with_capacity(text.len());
    for line in text.lines() {
        for letter in unheaded(line).chars() {
            if matches!(letter, '*' | '_' | '`' | '~') {
                continue;
            }
            if letter.is_whitespace() {
                if !flat.is_empty() && !flat.ends_with(' ') {
                    flat.push(' ');
                }
            } else {
                flat.push(letter);
            }
        }
        if !flat.is_empty() && !flat.ends_with(' ') {
            flat.push(' ');
        }
    }
    flat.truncate(flat.trim_end().len());
    flat
}

/// `line` without its Markdown heading marker, where it opens with one: up to
/// six `#` and the whitespace after them.
fn unheaded(line: &str) -> &str {
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

/// Where `word`, already lowercase, ends in `text` if it starts at `start`.
fn match_at(text: &str, start: usize, word: &str) -> Option<usize> {
    let mut wanted = word.chars().peekable();
    wanted.peek()?;
    let mut end = start;
    for letter in text[start..].chars() {
        for lowered in letter.to_lowercase() {
            if wanted.next() != Some(lowered) {
                return None;
            }
        }
        end += letter.len_utf8();
        if wanted.peek().is_none() {
            return Some(end);
        }
    }
    None
}

/// Where `word`, already lowercase, first falls in `text`, case-insensitively.
///
/// The bound is one try per character of `text`, each of at most `word`'s
/// length.
fn first_match(text: &str, word: &str) -> Option<Range<usize>> {
    text.char_indices()
        .find_map(|(start, _)| match_at(text, start, word).map(|end| start..end))
}

/// Where the earliest of `words` — all lowercase — falls in `text`, if every
/// one of them is somewhere in it. Every word or no hit: that is what "every
/// whitespace-separated word" means.
fn earliest_match(words: &[String], text: &str) -> Option<Range<usize>> {
    let mut earliest: Option<Range<usize>> = None;
    for word in words {
        let at = first_match(text, word)?;
        match &earliest {
            Some(held) if held.start <= at.start => {}
            _ => earliest = Some(at),
        }
    }
    earliest
}

/// How many characters of lead-in a snippet opens with.
const LEAD: usize = 42;
/// How many characters of the text a snippet shows.
const WINDOW: usize = 160;

/// The window of `text` around the match at `at`, and where the match sits in
/// it.
///
/// The window opens up to [`LEAD`] characters before the match, moved forward
/// to the next word so it does not open mid-word, and runs [`WINDOW`]
/// characters — the shape `legacy/app/js/files.js`'s `snippet` drew — with `…`
/// at whichever end was cut into. `text` is flattened, so a space is the only
/// whitespace a word ends at.
fn windowed(text: &str, at: &Range<usize>) -> Snippet {
    let mut from = text[..at.start]
        .char_indices()
        .rev()
        .take(LEAD)
        .last()
        .map_or(at.start, |(index, _)| index);
    if from > 0 {
        from += text[from..at.start].find(' ').map_or(0, |space| space + 1);
    }
    let to = text[from..]
        .char_indices()
        .nth(WINDOW)
        .map_or(text.len(), |(index, _)| from + index)
        .max(at.end);
    let mut window = String::new();
    if from > 0 {
        window.push('…');
    }
    let start = window.len();
    window.push_str(&text[from..to]);
    if to < text.len() {
        window.push('…');
    }
    Snippet {
        text: window,
        at: start + (at.start - from)..start + (at.end - from),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc::Receiver;
    use std::time::Duration;

    use crate::watch::{Placed, Watch};

    use super::*;

    /// How long a test waits for what it did to the disk to come back through
    /// the watch: the debounce, the batch the debouncer may hold it in, and
    /// room for a loaded machine — `watch`'s own wait.
    const WAIT: Duration = Duration::from_millis(500);

    /// An empty directory of one test's own, named for it and carrying the
    /// pid, because worktrees test concurrently.
    fn scratch(name: &str) -> PathBuf {
        let directory =
            std::env::temp_dir().join(format!("quill-library-{name}-{}", std::process::id()));
        fs::remove_dir_all(&directory).ok();
        fs::create_dir_all(&directory).expect("makes its own scratch directory");
        directory
    }

    /// Writes a file under `directory`, making the folders above it.
    fn written(directory: &Path, name: &str, text: &str) -> PathBuf {
        let path = directory.join(name);
        if let Some(above) = path.parent() {
            fs::create_dir_all(above).expect("makes the folders above the file");
        }
        fs::write(&path, text).expect("writes the file");
        path
    }

    /// Stamps `path` with a write time `seconds` after the epoch, so that a
    /// Date sort is asserted against times a test chose.
    fn stamped(path: &Path, seconds: u64) {
        let file = fs::File::options()
            .write(true)
            .open(path)
            .expect("opens the file to stamp it");
        file.set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(seconds))
            .expect("stamps the file");
    }

    /// A Library of the one Location at `root`, walked now, which is what most
    /// of these tests are.
    fn walked(root: &Path) -> Library {
        Library::open(std::slice::from_ref(&root.to_path_buf()), &[])
    }

    /// The names of every shown row of every section, in the order they are
    /// drawn.
    fn names(library: &Library, view: &View) -> Vec<String> {
        library
            .shown(view)
            .into_iter()
            .flat_map(|section| section.rows)
            .map(|row| row.name().to_string())
            .collect()
    }

    /// Everything the watch has to say before it falls silent for a wait,
    /// handed to the Library the way the app's main loop hands it, and whether
    /// the tree moved for any of it.
    fn drained(library: &mut Library, events: &Receiver<PathBuf>) -> bool {
        let mut moved = false;
        while let Ok(path) = events.recv_timeout(WAIT) {
            moved |= library.patch(&path);
        }
        moved
    }

    #[test]
    fn a_walked_location_lists_folders_first_and_only_the_four_extensions() {
        let directory = scratch("walked");
        written(&directory, "Notes/deep.md", "deep");
        written(&directory, "alpha.md", "alpha");
        written(&directory, "beta.txt", "beta");
        written(&directory, "delta.markdown", "delta");
        written(&directory, "gamma.text", "gamma");
        written(&directory, "ignored.rs", "fn main() {}");
        written(&directory, "README", "no extension");
        let library = walked(&directory);
        assert_eq!(
            names(
                &library,
                &View {
                    show_hidden: false,
                    sort: Sort::Name
                }
            ),
            [
                "Notes",
                "deep.md",
                "alpha.md",
                "beta.txt",
                "delta.markdown",
                "gamma.text"
            ]
        );
        fs::remove_dir_all(&directory).ok();
    }

    #[test]
    fn an_extension_is_listed_whatever_its_case() {
        let directory = scratch("cased");
        written(&directory, "SHOUTED.MD", "shouted");
        written(&directory, "quiet.Markdown", "quiet");
        let library = walked(&directory);
        assert_eq!(
            names(
                &library,
                &View {
                    show_hidden: false,
                    sort: Sort::Name
                }
            ),
            ["quiet.Markdown", "SHOUTED.MD"]
        );
        fs::remove_dir_all(&directory).ok();
    }

    #[test]
    fn a_dot_folder_is_absent_until_show_hidden_is_on() {
        let directory = scratch("hidden");
        written(&directory, ".git/HEAD.md", "ref");
        written(&directory, "one.md", "one");
        let library = walked(&directory);
        assert_eq!(names(&library, &View::default()), ["one.md"]);
        assert_eq!(
            names(
                &library,
                &View {
                    show_hidden: true,
                    sort: Sort::Name
                }
            ),
            [".git", "HEAD.md", "one.md"]
        );
        fs::remove_dir_all(&directory).ok();
    }

    #[test]
    fn flipping_show_hidden_re_queries_the_tree_rather_than_walking_it_again() {
        let directory = scratch("requery");
        written(&directory, ".git/HEAD.md", "ref");
        let library = walked(&directory);
        assert!(names(&library, &View::default()).is_empty());
        // The dot-folder is taken off the disk between the two queries. The
        // second still answers it, so what it read was the tree the one walk
        // built and not the disk.
        fs::remove_dir_all(directory.join(".git")).expect("takes the dot-folder away");
        assert_eq!(
            names(
                &library,
                &View {
                    show_hidden: true,
                    sort: Sort::Name
                }
            ),
            [".git", "HEAD.md"]
        );
        fs::remove_dir_all(&directory).ok();
    }

    #[test]
    fn date_sort_puts_the_newest_file_first() {
        let directory = scratch("dated");
        stamped(&written(&directory, "oldest.md", "one"), 100);
        stamped(&written(&directory, "newest.md", "two"), 300);
        stamped(&written(&directory, "middle.md", "three"), 200);
        let library = walked(&directory);
        assert_eq!(
            names(&library, &View::default()),
            ["newest.md", "middle.md", "oldest.md"]
        );
        fs::remove_dir_all(&directory).ok();
    }

    #[test]
    fn name_sort_is_case_insensitive() {
        let directory = scratch("named");
        stamped(&written(&directory, "Beta.md", "one"), 300);
        stamped(&written(&directory, "alpha.md", "two"), 200);
        stamped(&written(&directory, "Gamma.md", "three"), 100);
        let library = walked(&directory);
        assert_eq!(
            names(
                &library,
                &View {
                    show_hidden: false,
                    sort: Sort::Name
                }
            ),
            ["alpha.md", "Beta.md", "Gamma.md"]
        );
        fs::remove_dir_all(&directory).ok();
    }

    #[test]
    fn the_walk_reads_each_folder_once() {
        let directory = scratch("bound");
        written(&directory, "one.md", "one");
        written(&directory, "chapters/two.md", "two");
        written(&directory, "chapters/parts/three.md", "three");
        let mut reads = 0;
        let tree = walk(&directory, false, &mut reads);
        assert_eq!(reads, 3, "the root, `chapters` and `chapters/parts`");
        assert_eq!(tree.folders.len(), 1);
        assert_eq!(tree.files.len(), 1);
        fs::remove_dir_all(&directory).ok();
    }

    #[test]
    fn a_folder_the_tree_already_holds_is_not_walked_again() {
        let directory = scratch("unwalked");
        written(&directory, "chapters/one.md", "one");
        let mut library = walked(&directory);
        let view = View {
            show_hidden: false,
            sort: Sort::Name,
        };
        // Written behind the tree's back. A patch of a folder the tree already
        // holds reads nothing, because reading a folder is itself an event
        // under the watch, and a patch that walked again would answer its own
        // events for ever.
        written(&directory, "chapters/two.md", "two");
        assert!(!library.patch(&directory), "the Location's own root");
        assert!(!library.patch(&directory.join("chapters")), "a folder held");
        assert_eq!(names(&library, &view), ["chapters", "one.md"]);
        // The file's own path is what moves it, which is what the watch sends.
        assert!(library.patch(&directory.join("chapters/two.md")));
        assert_eq!(names(&library, &view), ["chapters", "one.md", "two.md"]);
        fs::remove_dir_all(&directory).ok();
    }

    #[test]
    fn a_file_written_renamed_and_deleted_under_the_watch_moves_the_tree() {
        let directory = scratch("watched");
        let root = directory.join("Library");
        fs::create_dir_all(&root).expect("makes the Location");
        let settings = written(&directory, "settings.toml", "theme = \"auto\"\n");
        let (mut watch, events) = Watch::on(&settings).expect("watches the settings file");
        assert_eq!(
            watch.add_tree(&root).expect("watches the Location"),
            Placed::Listening
        );
        let mut library = walked(&root);
        let view = View::default();
        assert!(names(&library, &view).is_empty(), "an empty Location");

        let one = root.join("one.md");
        fs::write(&one, "# One\n").expect("writes the file");
        assert!(drained(&mut library, &events), "the file is in the tree");
        assert_eq!(names(&library, &view), ["one.md"]);

        let two = root.join("two.md");
        fs::rename(&one, &two).expect("renames the file");
        assert!(
            drained(&mut library, &events),
            "the tree follows the rename"
        );
        assert_eq!(names(&library, &view), ["two.md"]);

        fs::remove_file(&two).expect("deletes the file");
        assert!(
            drained(&mut library, &events),
            "the tree follows the delete"
        );
        assert!(names(&library, &view).is_empty());
        fs::remove_dir_all(&directory).ok();
    }

    #[test]
    fn a_folder_created_under_the_watch_arrives_with_what_is_in_it() {
        let directory = scratch("subtree");
        let root = directory.join("Library");
        fs::create_dir_all(&root).expect("makes the Location");
        let settings = written(&directory, "settings.toml", "theme = \"auto\"\n");
        let (mut watch, events) = Watch::on(&settings).expect("watches the settings file");
        assert_eq!(
            watch.add_tree(&root).expect("watches the Location"),
            Placed::Listening
        );
        let mut library = walked(&root);
        let elsewhere = directory.join("chapters");
        written(&elsewhere, "one.md", "one");
        fs::rename(&elsewhere, root.join("chapters")).expect("moves the folder in");
        assert!(drained(&mut library, &events), "the folder is in the tree");
        assert_eq!(
            names(
                &library,
                &View {
                    show_hidden: false,
                    sort: Sort::Name
                }
            ),
            ["chapters", "one.md"]
        );
        fs::remove_dir_all(&directory).ok();
    }

    #[test]
    fn a_pinned_folder_answers_the_subtree_the_location_holds() {
        let directory = scratch("pinned");
        written(&directory, "chapters/one.md", "one");
        written(&directory, "chapters/two.md", "two");
        written(&directory, "top.md", "top");
        let chapters = directory.join("chapters");
        let library = Library::open(
            std::slice::from_ref(&directory),
            std::slice::from_ref(&chapters),
        );
        let view = View {
            show_hidden: false,
            sort: Sort::Name,
        };
        let rows = library.pinned_rows(&view);
        assert_eq!(rows.first().map(Row::path), Some(chapters.as_path()));
        let pinned: Vec<&Path> = rows.iter().skip(1).map(Row::path).collect();
        let subtree: Vec<&Path> = library
            .rows(&chapters, &view)
            .iter()
            .map(Row::path)
            .collect();
        assert_eq!(pinned, subtree);
        assert_eq!(subtree, [chapters.join("one.md"), chapters.join("two.md")]);
        fs::remove_dir_all(&directory).ok();
    }

    #[test]
    fn pinning_a_path_outside_every_location_is_refused() {
        let directory = scratch("outside");
        written(&directory, "inside/one.md", "one");
        let outside = written(&directory, "two.md", "two");
        let mut library = Library::open(&[directory.join("inside")], &[]);
        assert_eq!(library.pin(&outside), Pin::Outside);
        assert!(library.pinned().is_empty());
        assert_eq!(library.pin(&directory.join("inside/one.md")), Pin::Held);
        assert_eq!(library.pinned(), [directory.join("inside/one.md")]);
        fs::remove_dir_all(&directory).ok();
    }

    #[test]
    fn two_locations_answer_as_two_sections_in_the_order_added() {
        let directory = scratch("sections");
        written(&directory, "first/one.md", "one");
        written(&directory, "second/two.md", "two");
        let first = directory.join("first");
        let second = directory.join("second");
        let mut library = Library::open(&[first.clone(), second.clone()], &[]);
        let view = View::default();
        let roots: Vec<&Path> = library
            .shown(&view)
            .into_iter()
            .map(|section| section.root)
            .collect();
        assert_eq!(roots, [first.as_path(), second.as_path()]);

        assert!(library.remove_location(&first), "the first is a Location");
        let sections = library.shown(&view);
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].root, second.as_path());
        assert_eq!(names(&library, &view), ["two.md"]);
        fs::remove_dir_all(&directory).ok();
    }

    #[test]
    fn every_shown_file_is_answered_in_the_order_it_is_drawn() {
        let directory = scratch("files");
        written(&directory, "chapters/deep.md", "deep");
        written(&directory, ".hidden.md", "hidden");
        written(&directory, "top.md", "top");
        let library = walked(&directory);
        let view = View {
            show_hidden: false,
            sort: Sort::Name,
        };
        let shown: Vec<&str> = library.files(&view).map(File::name).collect();
        assert_eq!(shown, ["deep.md", "top.md"]);
        let everything = View {
            show_hidden: true,
            sort: Sort::Name,
        };
        let all: Vec<&str> = library.files(&everything).map(File::name).collect();
        assert_eq!(all, ["deep.md", ".hidden.md", "top.md"]);
        fs::remove_dir_all(&directory).ok();
    }

    /// The names a search answered, in the order it answered them.
    fn hits<'a>(found: &'a [Found<'a>]) -> Vec<&'a str> {
        found.iter().map(|found| found.file().name()).collect()
    }

    #[test]
    fn sst_finds_sea_storm_and_ranks_it_above_a_looser_name() {
        let directory = scratch("fuzzy");
        written(&directory, "sea-storm.md", "the tide turned");
        written(&directory, "sunset-stories.md", "the tide turned");
        written(&directory, "harbour.md", "the tide turned");
        let library = walked(&directory);
        let mut contents = Contents::new();
        let found = library.search("sst", &View::default(), &mut contents);
        assert_eq!(hits(&found), ["sea-storm.md", "sunset-stories.md"]);
        assert!(found.iter().all(|found| found.hit() == Hit::Name));
        assert!(
            found[0].score() > found[1].score(),
            "`sst` lies tighter on `sea-storm` than on `sunset-stories`"
        );
        assert!(
            found.iter().all(|found| found.snippet().is_none()),
            "a name hit is drawn as the name it matched"
        );
        assert_eq!(
            contents.reads(),
            1,
            "only `harbour.md`, whose name did not match, was read"
        );
        fs::remove_dir_all(&directory).ok();
    }

    #[test]
    fn a_query_no_name_or_text_holds_finds_nothing_and_an_empty_one_matches_nothing() {
        let directory = scratch("miss");
        written(&directory, "sea-storm.md", "the tide turned");
        let library = walked(&directory);
        let view = View::default();
        let mut contents = Contents::new();
        assert!(library.search("xyz", &view, &mut contents).is_empty());
        assert!(library.search("", &view, &mut contents).is_empty());
        assert!(library.search("   ", &view, &mut contents).is_empty());
        assert_eq!(contents.reads(), 1, "the empty queries read nothing");
        fs::remove_dir_all(&directory).ok();
    }

    #[test]
    fn every_word_of_the_query_must_be_in_the_text() {
        let directory = scratch("words");
        written(
            &directory,
            "harbour.md",
            "The lamp was lit before the sea rose.",
        );
        written(&directory, "lighthouse.md", "The sea rose.");
        let library = walked(&directory);
        let mut contents = Contents::new();
        let found = library.search("lamp sea", &View::default(), &mut contents);
        assert_eq!(hits(&found), ["harbour.md"]);
        assert_eq!(found[0].hit(), Hit::Content);
        fs::remove_dir_all(&directory).ok();
    }

    #[test]
    fn a_content_hits_snippet_opens_on_a_whole_word_and_marks_exactly_the_match() {
        let directory = scratch("snippet");
        let lead = "the tide turned and turned again ".repeat(4);
        let text = format!("# Harbour\n\n{lead}— the *sea* rose at dawn.\n");
        written(&directory, "harbour.md", &text);
        let library = walked(&directory);
        let mut contents = Contents::new();
        let found = library.search("sea", &View::default(), &mut contents);
        assert_eq!(hits(&found), ["harbour.md"]);
        let snippet = found[0].snippet().expect("a content hit carries a snippet");
        assert_eq!(snippet.matched(), "sea");
        assert_eq!(&snippet.text()[snippet.at()], "sea");
        assert!(
            snippet.text().starts_with('…'),
            "the window was cut into, so it says so"
        );
        assert!(
            snippet.text().contains("— the sea rose at dawn."),
            "the em dash and the emphasis marks are flattened, not broken: {}",
            snippet.text()
        );
        let flat = flatten(&text);
        let body = snippet.text().trim_matches('…');
        assert!(
            flat.contains(&format!(" {body}")),
            "the window opens on a whole word: {body}"
        );
        fs::remove_dir_all(&directory).ok();
    }

    #[test]
    fn a_name_hit_lists_before_a_content_hit_of_any_score() {
        let directory = scratch("kinds");
        written(
            &directory,
            "chapters-of-the-unnamed.md",
            "nothing to find here",
        );
        written(&directory, "harbour.md", "sea rose at dawn");
        let library = walked(&directory);
        let mut contents = Contents::new();
        let found = library.search("sea", &View::default(), &mut contents);
        assert_eq!(hits(&found), ["chapters-of-the-unnamed.md", "harbour.md"]);
        assert_eq!(found[0].hit(), Hit::Name);
        assert_eq!(found[1].hit(), Hit::Content);
        assert!(
            found[0].score() < found[1].score(),
            "the name hit scored below the content hit and still lists first"
        );
        fs::remove_dir_all(&directory).ok();
    }

    #[test]
    fn a_files_text_is_read_again_only_when_its_write_time_moves() {
        let directory = scratch("cache");
        let path = written(&directory, "harbour.md", "the lamp was lit");
        stamped(&path, 1_000);
        let view = View::default();
        let mut contents = Contents::new();
        let library = walked(&directory);
        assert_eq!(
            hits(&library.search("lamp", &view, &mut contents)),
            ["harbour.md"]
        );
        assert_eq!(contents.reads(), 1);
        // Rewritten behind the tree's back, at the write time it already read
        // at: the cache answers, and answers what it read.
        fs::write(&path, "the sea rose").expect("rewrites the file");
        stamped(&path, 1_000);
        assert_eq!(
            hits(&library.search("lamp", &view, &mut contents)),
            ["harbour.md"]
        );
        assert_eq!(contents.reads(), 1, "an unchanged write time reads nothing");
        // A write time the tree has seen move is a read, and the new text.
        stamped(&path, 2_000);
        let moved = walked(&directory);
        assert!(moved.search("lamp", &view, &mut contents).is_empty());
        assert_eq!(
            hits(&moved.search("sea", &view, &mut contents)),
            ["harbour.md"]
        );
        assert_eq!(contents.reads(), 2, "one read for the new write time");
        fs::remove_dir_all(&directory).ok();
    }

    #[test]
    fn a_hidden_file_is_searched_only_where_it_is_shown() {
        let directory = scratch("unshown");
        written(&directory, ".sea-storm.md", "the tide turned");
        let library = walked(&directory);
        let mut contents = Contents::new();
        assert!(
            library
                .search("sst", &View::default(), &mut contents)
                .is_empty()
        );
        let everything = View {
            show_hidden: true,
            sort: Sort::Name,
        };
        assert_eq!(
            hits(&library.search("sst", &everything, &mut contents)),
            [".sea-storm.md"]
        );
        fs::remove_dir_all(&directory).ok();
    }
}
