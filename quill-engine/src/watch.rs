//! A file saved under a running Quill: what a save arrives on, and the rule
//! that keeps a line Quill cannot apply from being said twice.
//!
//! [`Watch`] watches the *directory* a file sits in rather than the file
//! itself, because a save is a write to a temporary file and a rename over the
//! top — an editor's, and Quill's own ([`crate::settings::Settings::write_to`])
//! — and a watch on the path alone follows the file it was pointed at out of
//! the way. Events are gathered up for [`DEBOUNCE`] first, so the two halves of
//! one save are one save. Two paths are watched today: the settings file, and
//! the palette file it names (`docs/architecture.md` § Settings); the Documents
//! and the Library add theirs later (the File handling spec) through
//! [`Watch::add`], which is why what is being listened for is a set rather than
//! the one path the settings watch needs, and why [`Watch::remove`] exists — a
//! `palette` line edited or taken out drops the file it named.
//!
//! The palette file is what a desktop theme tool writes, and such a tool does
//! not save into the directory: it removes the directory and moves a fresh one
//! into place, or re-points a link at another. A watch on the directory dies
//! with it, so the directory *above* it is watched too, non-recursively, and an
//! event there naming the directory re-resolves the file and arms the watch
//! again — with the file's new version sent on, because a replaced directory
//! is an edit and not a bystander. A directory that is not there yet is the
//! same shape: [`Watch::add`] says so ([`Placed::NoDirectory`]) rather than
//! polling for it, and the caller adds the file again when it has reason to.
//!
//! A Location is the other kind of subject: a folder tree, added with
//! [`Watch::add_tree`] and watched recursively, every settled event under it
//! sent on — the architecture's "the Documents and the Library join the same
//! watch" (`docs/architecture.md` § Settings). A version is not consulted for
//! one, because a `stat` tells a save from an open but not a file created from
//! a file renamed away, and what reads the path is the Library, which re-stats
//! it as it patches its tree ([`crate::library::Library::patch`]). What arrives
//! is the path under the root as the root was added, so a caller tells a tree's
//! event from a file's by the prefix.
//!
//! The app owns the [`Watch`] and drains its [`Receiver`] from its own main
//! loop: the engine has no `glib` (ADR 0008), and the thread the debouncer
//! answers on is not one a window can be touched from. What arrives is the path
//! the file was added by, so the app tells the settings file from the palette
//! file by comparing against what it asked for.
//!
//! [`unsaid`] is the other half of a re-read — what a version of a file has to
//! say that the version before it did not already say — because a writer who
//! saves the same refused line twice should hear about it once.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, SystemTime};

use notify::{RecommendedWatcher, RecursiveMode};
use notify_debouncer_mini::{DebounceEventResult, DebouncedEventKind, Debouncer, new_debouncer};

/// How long the events of one save are gathered up before they are one save.
///
/// A save is at least a create and a rename, and an editor writing in two
/// steps is more; 100 ms is long enough to hold them together and short
/// enough that a writer who saved sees the page follow.
pub const DEBOUNCE: Duration = Duration::from_millis(100);

/// Whether a path handed to [`Watch::add`] or [`Watch::add_tree`] is being
/// listened for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[must_use = "a path whose directory is not there is not being listened for"]
pub enum Placed {
    /// The file's directory — or the tree's root — is there and watched: what
    /// happens under it will arrive.
    Listening,
    /// The file's directory is not there, so there is nothing to watch yet.
    /// The watch does not poll for it; the caller adds the file again when it
    /// has reason to think the directory has appeared — the app, on the
    /// settings watch's next event — and meanwhile, if the directory above it
    /// is there, the directory's arrival is seen there and arms the watch. A
    /// tree whose root is not a folder is this same answer, and is added again
    /// the same way.
    NoDirectory,
}

/// What tells one version of a file from the one before it: how long it is
/// and when it was written, read with a `stat` and never by opening it.
///
/// Here because `notify` reports a file being *opened* as readily as one
/// being written — an editor re-reading the file, a hand's `cat`, and Quill's
/// own re-read of a save — and an open is not a save. A file whose length and
/// write time have not moved since it was last sent on has not been saved,
/// whatever was done to it; and the question is asked with a `stat` because
/// opening the file to look would be one more open for the watch to report.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Version {
    /// The file's length in bytes.
    len: u64,
    /// When it was last written, where the file system says.
    modified: Option<SystemTime>,
}

/// The version of the file at `path`, or `None` where there is no file.
fn version(path: &Path) -> Option<Version> {
    fs::metadata(path).ok().map(|metadata| Version {
        len: metadata.len(),
        modified: metadata.modified().ok(),
    })
}

/// Where a file added by one path is on the file system now, as the watcher's
/// events will name it.
///
/// Resolved once and put back together, because an event's path is the watched
/// directory's with a name joined to it: a relative path, or one through a
/// symbolic link, would never be equal to a path reported for the same file.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Place {
    /// The file, under its directory resolved — `None` while the directory is
    /// not there.
    file: Option<PathBuf>,
    /// The file's directory as an event on the directory *above* it names it,
    /// which is how the directory's being replaced is seen — `None` where
    /// there is no directory above to resolve.
    directory: Option<PathBuf>,
}

/// Resolves `given` now.
fn locate(given: &Path) -> Place {
    let directory = match given.parent() {
        Some(directory) if !directory.as_os_str().is_empty() => directory,
        _ => Path::new("."),
    };
    let file = given.file_name().and_then(|name| {
        directory
            .canonicalize()
            .ok()
            .map(|directory| directory.join(name))
    });
    let above = directory
        .parent()
        .filter(|above| !above.as_os_str().is_empty())
        .and_then(|above| above.canonicalize().ok());
    let directory = match (above, directory.file_name()) {
        (Some(above), Some(name)) => Some(above.join(name)),
        _ => None,
    };
    Place { file, directory }
}

/// One file being listened for.
struct Subject {
    /// Where it is now.
    place: Place,
    /// The version of it last sent on.
    sent: Option<Version>,
}

/// One folder tree being listened for, root and all.
struct Tree {
    /// Its root as the watcher's events will name it — `None` while there is
    /// no folder there.
    root: Option<PathBuf>,
}

/// Resolves the root of a tree added as `given`, which is `None` where there is
/// no folder there: a tree is a root and everything under it, so a path that is
/// not a directory is not a tree.
fn root_of(given: &Path) -> Option<PathBuf> {
    let root = given.canonicalize().ok()?;
    root.is_dir().then_some(root)
}

/// What the watch knows, shared between the thread that owns the [`Watch`] and
/// the debouncer's, which is where a directory replaced is seen and where the
/// watcher has to be reached to arm it again.
struct Inner {
    /// The debouncer, whose thread and whose watches end with it; `None` once
    /// the [`Watch`] is dropped, which is what ends them.
    debouncer: Option<Debouncer<RecommendedWatcher>>,
    /// The files listened for, by the path each was added by.
    subjects: BTreeMap<PathBuf, Subject>,
    /// The folder trees listened for, by the path each was added by.
    trees: BTreeMap<PathBuf, Tree>,
    /// The directories handed to the watcher and how each is watched: every
    /// subject's own, resolved, and the one above it, non-recursively; every
    /// tree's root recursively.
    watched: BTreeMap<PathBuf, RecursiveMode>,
    /// Where a save is sent on.
    sender: Sender<PathBuf>,
}

impl Inner {
    /// Every directory that has to be watched and how: a tree's root
    /// recursively, and every subject's own directory and the one above it
    /// non-recursively, unless a tree's root already covers it — one directory
    /// is one watch, and the recursive one is the one that sees more.
    ///
    /// The walk is one pass over the subjects, each asked against the roots,
    /// and there are as many roots as the writer has Locations.
    fn wanted(&self) -> BTreeMap<PathBuf, RecursiveMode> {
        let roots: Vec<&Path> = self
            .trees
            .values()
            .filter_map(|tree| tree.root.as_deref())
            .collect();
        let mut wanted: BTreeMap<PathBuf, RecursiveMode> = roots
            .iter()
            .map(|root| (root.to_path_buf(), RecursiveMode::Recursive))
            .collect();
        let directories = self.subjects.values().flat_map(|subject| {
            let own = subject.place.file.as_deref().and_then(Path::parent);
            let above = subject.place.directory.as_deref().and_then(Path::parent);
            own.into_iter().chain(above)
        });
        for directory in directories {
            if roots.iter().any(|root| directory.starts_with(root)) {
                continue;
            }
            wanted.insert(directory.to_path_buf(), RecursiveMode::NonRecursive);
        }
        wanted
    }

    /// Watches every directory the subjects and the trees want, each in the
    /// mode that wants it, and no other; answers with the first error the
    /// watcher gave, having tried them all. A directory the watcher refused is
    /// not remembered as watched, so the next add tries it again rather than
    /// taking the refusal as final.
    fn sync(&mut self) -> Result<(), notify::Error> {
        let wanted = self.wanted();
        let Some(debouncer) = self.debouncer.as_mut() else {
            return Ok(());
        };
        let watcher = debouncer.watcher();
        let mut watched = BTreeMap::new();
        for (path, mode) in &self.watched {
            if wanted.get(path) == Some(mode) {
                watched.insert(path.clone(), *mode);
            } else {
                // A directory that was removed under the watch is already
                // unwatched; one whose mode moved is watched again below.
                let _ = watcher.unwatch(path);
            }
        }
        let mut first = Ok(());
        for (path, mode) in &wanted {
            if watched.contains_key(path) {
                continue;
            }
            match watcher.watch(path, *mode) {
                Ok(()) => {
                    watched.insert(path.clone(), *mode);
                }
                Err(err) => first = first.and(Err(err)),
            }
        }
        self.watched = watched;
        first
    }

    /// Listens for `given`, placed where it is now.
    fn add(&mut self, given: &Path) -> Result<Placed, notify::Error> {
        let place = locate(given);
        let placed = match place.file {
            Some(_) => Placed::Listening,
            None => Placed::NoDirectory,
        };
        let sent = place.file.as_deref().and_then(version);
        self.subjects
            .insert(given.to_path_buf(), Subject { place, sent });
        self.sync()?;
        Ok(placed)
    }

    /// Listens for the tree rooted at `given`, resolved where it is now.
    fn add_tree(&mut self, given: &Path) -> Result<Placed, notify::Error> {
        let root = root_of(given);
        let placed = match root {
            Some(_) => Placed::Listening,
            None => Placed::NoDirectory,
        };
        self.trees.insert(given.to_path_buf(), Tree { root });
        self.sync()?;
        Ok(placed)
    }

    /// Acts on what the debouncer has gathered.
    fn heard(&mut self, events: DebounceEventResult) {
        // An error the watcher itself reports is dropped: the settings watch is
        // never fatal (`docs/architecture.md` § Settings), and a watch that has
        // stopped answering leaves a Quill running on the settings it read.
        let Ok(events) = events else {
            return;
        };
        for event in events {
            // A file still being written is `AnyContinuous` and is not sent on:
            // a run of writes is one save, and the `Any` that ends it is the
            // one worth reading the file at.
            if event.kind == DebouncedEventKind::Any {
                self.heard_at(&event.path);
            }
        }
    }

    /// Acts on one settled path: a subject's file saved, a subject's directory
    /// replaced — named from above, or itself moved or removed — or a path
    /// under a tree, whatever happened to it.
    ///
    /// The temporary file a save is written through is in the same directory
    /// and is reported like anything else there, so what is sent on for a
    /// subject is what was asked for rather than what moved. A tree sends the
    /// path that moved, under the root as the root was added: the Library is
    /// told which of its rows to look at again, and a caller tells a tree's
    /// event from a subject's by the prefix.
    fn heard_at(&mut self, path: &Path) {
        let saved: Vec<PathBuf> = self
            .subjects
            .iter()
            .filter(|(_, subject)| subject.place.file.as_deref() == Some(path))
            .map(|(given, _)| given.clone())
            .collect();
        for given in saved {
            self.check(&given);
        }
        let replaced: Vec<PathBuf> = self
            .subjects
            .iter()
            .filter(|(_, subject)| {
                let own = subject.place.file.as_deref().and_then(Path::parent);
                subject.place.directory.as_deref() == Some(path) || own == Some(path)
            })
            .map(|(given, _)| given.clone())
            .collect();
        for given in replaced {
            self.replace(&given);
        }
        let under: Vec<PathBuf> = self
            .trees
            .iter()
            .filter_map(|(given, tree)| {
                let rest = path.strip_prefix(tree.root.as_deref()?).ok()?;
                Some(if rest.as_os_str().is_empty() {
                    given.clone()
                } else {
                    given.join(rest)
                })
            })
            .collect();
        for moved in under {
            let _ = self.sender.send(moved);
        }
    }

    /// Sends `given` on if its version has moved since it was last sent.
    fn check(&mut self, given: &Path) {
        let Some(subject) = self.subjects.get_mut(given) else {
            return;
        };
        // A version already sent on is an open, not a save ([`Version`]).
        let now = subject.place.file.as_deref().and_then(version);
        if now == subject.sent {
            return;
        }
        subject.sent = now;
        let _ = self.sender.send(given.to_path_buf());
    }

    /// Resolves `given` again after its directory moved, arms the watch on
    /// whatever is there now, and sends the file on if it changed with it.
    ///
    /// The old directory is unwatched by name whether or not it still exists:
    /// a directory moved away keeps its watch, and that watch would report a
    /// stranger's saves under the old path.
    fn replace(&mut self, given: &Path) {
        let Some(subject) = self.subjects.get_mut(given) else {
            return;
        };
        let stale = subject
            .place
            .file
            .as_deref()
            .and_then(Path::parent)
            .map(Path::to_path_buf);
        subject.place = locate(given);
        if let (Some(stale), Some(debouncer)) = (stale, self.debouncer.as_mut()) {
            let _ = debouncer.watcher().unwatch(&stale);
            self.watched.remove(&stale);
        }
        let _ = self.sync();
        self.check(given);
    }
}

/// A watch on one or more files, which stops when it is dropped.
pub struct Watch {
    /// Everything, shared with the debouncer's thread.
    inner: Arc<Mutex<Inner>>,
}

/// The lock, poisoned or not: a thread that panicked while holding it has
/// left the map as it was, and a watch that fell silent for good would be a
/// worse outcome than one that reads a map mid-edit.
fn lock(inner: &Mutex<Inner>) -> MutexGuard<'_, Inner> {
    inner.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Watch {
    /// Watches `path`, and answers with the receiver every save of it arrives
    /// on.
    ///
    /// The path arrives as it was handed here rather than as the event
    /// reported it, so a caller can compare it against the path it asked for.
    ///
    /// # Errors
    ///
    /// Returns the watcher's own error where the directory holding `path` is
    /// not there or cannot be watched: the first file is the settings file,
    /// whose directory the app has made by then.
    pub fn on(path: &Path) -> Result<(Self, Receiver<PathBuf>), notify::Error> {
        let (sender, receiver) = mpsc::channel();
        let inner = Arc::new(Mutex::new(Inner {
            debouncer: None,
            subjects: BTreeMap::new(),
            trees: BTreeMap::new(),
            watched: BTreeMap::new(),
            sender,
        }));
        let heard = Arc::clone(&inner);
        let debouncer = new_debouncer(DEBOUNCE, move |events: DebounceEventResult| {
            lock(&heard).heard(events);
        })?;
        lock(&inner).debouncer = Some(debouncer);
        let mut watch = Self { inner };
        match watch.add(path)? {
            Placed::Listening => Ok((watch, receiver)),
            Placed::NoDirectory => {
                Err(notify::Error::path_not_found().add_path(path.to_path_buf()))
            }
        }
    }

    /// Listens for `path` as well, on the same receiver, and says whether it
    /// is being listened for yet.
    ///
    /// Adding a path already listened for places it again, which is how a
    /// caller retries a [`Placed::NoDirectory`].
    ///
    /// # Errors
    ///
    /// Returns the watcher's own error where a directory cannot be watched.
    pub fn add(&mut self, path: &Path) -> Result<Placed, notify::Error> {
        lock(&self.inner).add(path)
    }

    /// Listens for everything under the folder `path` as well, on the same
    /// receiver, and says whether it is being listened for yet.
    ///
    /// This is what a Location joins the watch by: the folder and everything
    /// beneath it, a file created, written, renamed or deleted anywhere under
    /// it arriving as the path it happened to, under `path` as `path` was
    /// handed here. Adding a tree already listened for resolves its root
    /// again, which is how a caller retries a [`Placed::NoDirectory`].
    ///
    /// # Errors
    ///
    /// Returns the watcher's own error where the folder cannot be watched.
    pub fn add_tree(&mut self, path: &Path) -> Result<Placed, notify::Error> {
        lock(&self.inner).add_tree(path)
    }

    /// Stops listening for `path`; a directory stays watched only while a file
    /// in it is listened for.
    pub fn remove(&mut self, path: &Path) {
        let mut inner = lock(&self.inner);
        inner.subjects.remove(path);
        let _ = inner.sync();
    }

    /// Stops listening for the tree rooted at `path`, which a Location removed
    /// from the Library does.
    pub fn remove_tree(&mut self, path: &Path) {
        let mut inner = lock(&self.inner);
        inner.trees.remove(path);
        let _ = inner.sync();
    }
}

impl Drop for Watch {
    /// Takes the debouncer out from under the shared state so that it is
    /// dropped — and its thread and watches ended — even though the debouncer's
    /// own closure still holds the state; taken under the lock and dropped
    /// outside it, so that a callback waiting for the lock is never waited on
    /// in turn.
    fn drop(&mut self) {
        let debouncer = lock(&self.inner).debouncer.take();
        drop(debouncer);
    }
}

/// What a version of a file has to say, and what the version after it is
/// compared against.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Unsaid {
    /// The lines to log now, in the order they were read.
    pub lines: Vec<String>,
    /// Every line this version carried, which is what the next read hands
    /// back as `said`.
    pub said: BTreeSet<String>,
}

/// The lines of `now` that the version of the file before it did not already
/// say, and what the version after it is compared against.
///
/// Once per distinct line per version of the file, rather than once per line
/// per run: a writer who saves the same refused line again hears nothing, and
/// one who takes it out and puts it back hears it again, because each version
/// replaces the set rather than adding to it. The same line twice in one
/// version is one line.
///
/// Both halves come back from the one call, so that a caller cannot say a line
/// twice by forgetting to take down what it said.
#[must_use]
pub fn unsaid(said: &BTreeSet<String>, now: &[String]) -> Unsaid {
    let mut lines = Vec::new();
    let mut version = BTreeSet::new();
    for line in now {
        if version.insert(line.clone()) && !said.contains(line) {
            lines.push(line.clone());
        }
    }
    Unsaid {
        lines,
        said: version,
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::unix::fs::symlink;
    use std::sync::mpsc::RecvTimeoutError;

    use crate::shortcuts::Chord;

    use super::*;

    /// How long a test waits for a save to come through: the debounce, the
    /// batch the debouncer may hold it in, and room for a loaded machine.
    const WAIT: Duration = Duration::from_millis(500);

    /// An empty directory of one test's own, named for it and carrying the
    /// pid, because worktrees test concurrently.
    fn scratch(name: &str) -> PathBuf {
        let directory =
            std::env::temp_dir().join(format!("quill-watch-{name}-{}", std::process::id()));
        fs::remove_dir_all(&directory).ok();
        fs::create_dir_all(&directory).expect("makes its own scratch directory");
        directory
    }

    /// The lines of a version of a file, as the caller of [`unsaid`] holds
    /// them.
    fn version(lines: &[&str]) -> Vec<String> {
        lines.iter().map(|line| (*line).to_string()).collect()
    }

    #[test]
    fn a_save_of_a_watched_file_arrives_on_the_receiver() {
        let directory = scratch("saved");
        let file = directory.join("settings.toml");
        fs::write(&file, "theme = \"light\"\n").unwrap();
        let (_watch, saves) = Watch::on(&file).unwrap();
        fs::write(&file, "theme = \"dark\"\n").unwrap();
        assert_eq!(saves.recv_timeout(WAIT), Ok(file));
        fs::remove_dir_all(&directory).ok();
    }

    #[test]
    fn a_save_of_a_file_that_replaces_the_watched_one_arrives_too() {
        // What every editor does, and what `settings::file::write` does: the
        // bytes go to a file beside it and are renamed over the top, so the
        // file the watch was pointed at is not the file that is there after.
        let directory = scratch("renamed");
        let file = directory.join("settings.toml");
        fs::write(&file, "theme = \"light\"\n").unwrap();
        let (_watch, saves) = Watch::on(&file).unwrap();
        let beside = directory.join(".settings.toml.tmp");
        fs::write(&beside, "theme = \"dark\"\n").unwrap();
        fs::rename(&beside, &file).unwrap();
        assert_eq!(saves.recv_timeout(WAIT), Ok(file));
        fs::remove_dir_all(&directory).ok();
    }

    #[test]
    fn two_saves_inside_the_debounce_arrive_as_one() {
        let directory = scratch("debounced");
        let file = directory.join("settings.toml");
        fs::write(&file, "theme = \"light\"\n").unwrap();
        let (_watch, saves) = Watch::on(&file).unwrap();
        fs::write(&file, "theme = \"dark\"\n").unwrap();
        std::thread::sleep(Duration::from_millis(20));
        fs::write(&file, "theme = \"auto\"\n").unwrap();
        assert_eq!(saves.recv_timeout(WAIT), Ok(file.clone()));
        assert_eq!(
            saves.recv_timeout(WAIT),
            Err(RecvTimeoutError::Timeout),
            "the second write is the same save"
        );
        fs::remove_dir_all(&directory).ok();
    }

    /// An editor re-reading the file, a `cat`, and Quill's own re-read of a
    /// save all open the file, and `notify` reports every open. None of them
    /// is a save, and a Quill that took its own re-read for one would re-read
    /// the file for as long as it ran.
    #[test]
    fn a_read_of_the_watched_file_is_not_a_save() {
        let directory = scratch("read");
        let file = directory.join("settings.toml");
        fs::write(&file, "theme = \"light\"\n").unwrap();
        let (_watch, saves) = Watch::on(&file).unwrap();
        let read = fs::read_to_string(&file).unwrap();
        assert_eq!(read, "theme = \"light\"\n");
        assert_eq!(saves.recv_timeout(WAIT), Err(RecvTimeoutError::Timeout));
        // And a save after the read is still a save.
        fs::write(&file, "theme = \"dark\"\n").unwrap();
        assert_eq!(saves.recv_timeout(WAIT), Ok(file));
        fs::remove_dir_all(&directory).ok();
    }

    #[test]
    fn a_save_of_another_file_in_the_directory_is_not_sent() {
        let directory = scratch("neighbour");
        let file = directory.join("settings.toml");
        fs::write(&file, "theme = \"light\"\n").unwrap();
        let (_watch, saves) = Watch::on(&file).unwrap();
        fs::write(directory.join("state.toml"), "last_scheme = \"dark\"\n").unwrap();
        assert_eq!(saves.recv_timeout(WAIT), Err(RecvTimeoutError::Timeout));
        fs::remove_dir_all(&directory).ok();
    }

    #[test]
    fn a_file_added_after_the_watch_is_listened_for_too() {
        let directory = scratch("added");
        let file = directory.join("settings.toml");
        let later = directory.join("a-document.md");
        fs::write(&file, "theme = \"light\"\n").unwrap();
        fs::write(&later, "# A Document\n").unwrap();
        let (mut watch, saves) = Watch::on(&file).unwrap();
        assert_eq!(watch.add(&later).unwrap(), Placed::Listening);
        fs::write(&later, "# A Document, saved\n").unwrap();
        assert_eq!(saves.recv_timeout(WAIT), Ok(later));
        fs::remove_dir_all(&directory).ok();
    }

    #[test]
    fn a_watch_that_has_been_dropped_sends_nothing() {
        let directory = scratch("dropped");
        let file = directory.join("settings.toml");
        fs::write(&file, "theme = \"light\"\n").unwrap();
        let (watch, saves) = Watch::on(&file).unwrap();
        drop(watch);
        fs::write(&file, "theme = \"dark\"\n").unwrap();
        assert!(saves.recv_timeout(WAIT).is_err());
        fs::remove_dir_all(&directory).ok();
    }

    /// The whole of a save, end to end and on the clock: a writer rebinds a
    /// Command in the file, and the chords Quill would install have changed
    /// before half a second is out.
    ///
    /// The two halves apart are [`Watch`]'s and
    /// [`crate::settings::Settings::shortcuts`]'; this is the one thing the
    /// writer is promised, which is neither of them alone.
    #[test]
    fn a_rebind_saved_to_the_file_is_in_the_effective_map_within_the_wait() {
        use crate::settings::Settings;

        let directory = scratch("rebind");
        let file = directory.join("settings.toml");
        let (settings, _) = Settings::open_at(&file);
        let (_watch, saves) = Watch::on(&file).unwrap();

        let saved = std::time::Instant::now();
        let mut rebound = settings;
        rebound.shortcuts = "\"library.toggle\" = [\"F9\"]"
            .parse()
            .expect("the entry the writer typed is TOML");
        rebound.write_to(&file).unwrap();

        assert_eq!(saves.recv_timeout(WAIT), Ok(file.clone()));
        let (read, notes) = Settings::reread(&file);
        let read = read.expect("the file the writer saved is TOML");
        assert!(saved.elapsed() < WAIT, "{:?}", saved.elapsed());
        assert_eq!(notes, Vec::<String>::new());

        let chords = read.shortcuts().chords;
        let library: Vec<&str> = chords["library.toggle"].iter().map(Chord::as_str).collect();
        assert_eq!(library, ["F9"]);
        assert!(
            !chords
                .values()
                .flatten()
                .any(|chord| chord.as_str() == "<Control>e"),
            "the chord it replaced is left bound to nothing"
        );
        fs::remove_dir_all(&directory).ok();
    }

    /// A watch opened on a settings file in `directory`, the way the app opens
    /// one, so that a palette file can be added beside it.
    fn opened(directory: &Path) -> (Watch, Receiver<PathBuf>) {
        let settings = directory.join("settings.toml");
        fs::write(&settings, "theme = \"auto\"\n").unwrap();
        Watch::on(&settings).unwrap()
    }

    /// A directory the watcher refuses — one that cannot be read, which inotify
    /// will not watch — is tried again by the next add of a file in it, so a
    /// refusal is not taken as the directory being watched.
    #[test]
    fn a_directory_the_watcher_refused_is_tried_again_on_the_next_add() {
        use std::os::unix::fs::PermissionsExt;
        let directory = scratch("refused");
        let theme = directory.join("theme");
        fs::create_dir_all(&theme).unwrap();
        let palette = theme.join("quill.toml");
        let (mut watch, saves) = opened(&directory);
        fs::set_permissions(&theme, fs::Permissions::from_mode(0o000)).unwrap();
        let refused = watch.add(&palette);
        fs::set_permissions(&theme, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(
            refused.is_err(),
            "a directory that cannot be read cannot be watched"
        );
        assert_eq!(watch.add(&palette).ok(), Some(Placed::Listening));
        fs::write(&palette, "[dark]\npaper = \"#101010\"\n").unwrap();
        assert_eq!(saves.recv_timeout(WAIT), Ok(palette));
    }

    /// A theme directory under `directory` holding a palette file that says
    /// `text`, and the path the palette will be added by.
    fn themed(directory: &Path, name: &str, text: &str) -> PathBuf {
        let theme = directory.join(name);
        fs::create_dir_all(&theme).unwrap();
        let palette = theme.join("quill.toml");
        fs::write(&palette, text).unwrap();
        palette
    }

    /// Everything the receiver has to say before it falls silent for a wait.
    fn drained(saves: &Receiver<PathBuf>) -> Vec<PathBuf> {
        let mut heard = Vec::new();
        while let Ok(path) = saves.recv_timeout(WAIT) {
            heard.push(path);
        }
        heard
    }

    #[test]
    fn a_palette_file_in_another_directory_arrives_by_the_path_it_was_added_by() {
        let directory = scratch("palette");
        let palette = themed(&directory, "theme", "[dark]\npaper = \"#101010\"\n");
        let (mut watch, saves) = opened(&directory);
        assert_eq!(watch.add(&palette).unwrap(), Placed::Listening);
        fs::write(&palette, "[dark]\npaper = \"#202020\"\n").unwrap();
        assert_eq!(saves.recv_timeout(WAIT), Ok(palette));
        fs::remove_dir_all(&directory).ok();
    }

    /// What `omarchy theme set` does: the theme directory is removed and the
    /// next one is moved into its place, so the directory the watch was on is
    /// gone and the one at its path is a stranger.
    #[test]
    fn a_directory_moved_over_the_watched_one_is_watched_in_its_turn() {
        let directory = scratch("replaced");
        let palette = themed(&directory, "theme", "[dark]\npaper = \"#101010\"\n");
        let (mut watch, saves) = opened(&directory);
        assert_eq!(watch.add(&palette).unwrap(), Placed::Listening);

        themed(&directory, "next-theme", "[light]\npaper = \"#fafafa\"\n");
        fs::remove_dir_all(directory.join("theme")).unwrap();
        fs::rename(directory.join("next-theme"), directory.join("theme")).unwrap();
        let heard = drained(&saves);
        assert!(
            !heard.is_empty() && heard.iter().all(|path| *path == palette),
            "the directory moved in is an edit of the file: {heard:?}"
        );

        fs::write(&palette, "[light]\npaper = \"#f0f0f0\"\n").unwrap();
        assert_eq!(
            saves.recv_timeout(WAIT),
            Ok(palette),
            "and the watch is on the directory that is there now"
        );
        fs::remove_dir_all(&directory).ok();
    }

    /// What a theme tool that keeps every theme and points a link at the
    /// current one does: the link is written beside and renamed over, and the
    /// directory the watch resolved to is untouched and no longer current.
    #[test]
    fn a_directory_re_pointed_through_a_link_is_followed() {
        let directory = scratch("linked");
        let first = themed(&directory, "themes/a", "[dark]\npaper = \"#101010\"\n");
        let second = themed(&directory, "themes/b", "[light]\npaper = \"#fafafa\"\n");
        symlink(first.parent().unwrap(), directory.join("theme")).unwrap();
        let palette = directory.join("theme/quill.toml");
        let (mut watch, saves) = opened(&directory);
        assert_eq!(watch.add(&palette).unwrap(), Placed::Listening);

        symlink(second.parent().unwrap(), directory.join("theme.next")).unwrap();
        fs::rename(directory.join("theme.next"), directory.join("theme")).unwrap();
        let heard = drained(&saves);
        assert!(
            !heard.is_empty() && heard.iter().all(|path| *path == palette),
            "the link re-pointed is an edit of the file: {heard:?}"
        );

        fs::write(&second, "[light]\npaper = \"#f0f0f0\"\n").unwrap();
        assert_eq!(saves.recv_timeout(WAIT), Ok(palette), "the new directory");
        fs::write(&first, "[dark]\npaper = \"#202020\"\n").unwrap();
        assert_eq!(
            saves.recv_timeout(WAIT),
            Err(RecvTimeoutError::Timeout),
            "the old directory is nobody's now"
        );
        fs::remove_dir_all(&directory).ok();
    }

    #[test]
    fn a_file_removed_from_the_watch_is_not_sent_on() {
        let directory = scratch("removed");
        let palette = themed(&directory, "theme", "[dark]\npaper = \"#101010\"\n");
        let (mut watch, saves) = opened(&directory);
        assert_eq!(watch.add(&palette).unwrap(), Placed::Listening);
        watch.remove(&palette);
        fs::write(&palette, "[dark]\npaper = \"#202020\"\n").unwrap();
        assert_eq!(saves.recv_timeout(WAIT), Err(RecvTimeoutError::Timeout));
        // The file the watch was opened on is still listened for.
        let settings = directory.join("settings.toml");
        fs::write(&settings, "theme = \"dark\"\n").unwrap();
        assert_eq!(saves.recv_timeout(WAIT), Ok(settings));
        fs::remove_dir_all(&directory).ok();
    }

    /// A Location under `directory`, with a folder in it, and the path it will
    /// be added by.
    fn located(directory: &Path) -> PathBuf {
        let root = directory.join("Library");
        fs::create_dir_all(root.join("chapters")).unwrap();
        root
    }

    #[test]
    fn a_file_created_deep_in_a_watched_tree_arrives_by_the_path_the_root_was_added_by() {
        let directory = scratch("tree");
        let root = located(&directory);
        let (mut watch, saves) = opened(&directory);
        assert_eq!(watch.add_tree(&root).unwrap(), Placed::Listening);
        let file = root.join("chapters").join("one.md");
        fs::write(&file, "# One\n").unwrap();
        assert!(drained(&saves).contains(&file), "the file that was created");
        fs::remove_dir_all(&directory).ok();
    }

    #[test]
    fn a_file_renamed_inside_a_watched_tree_arrives_as_the_path_it_left_and_the_one_it_took() {
        // Which is what a tree's caller needs: `notify` gathers events by path,
        // so a rename is two paths, and the Library takes the row off the one
        // and puts it on the other.
        let directory = scratch("moved");
        let root = located(&directory);
        let one = root.join("one.md");
        fs::write(&one, "# One\n").unwrap();
        let (mut watch, saves) = opened(&directory);
        assert_eq!(watch.add_tree(&root).unwrap(), Placed::Listening);
        let two = root.join("two.md");
        fs::rename(&one, &two).unwrap();
        let heard = drained(&saves);
        assert!(heard.contains(&one) && heard.contains(&two), "{heard:?}");
        fs::remove_dir_all(&directory).ok();
    }

    #[test]
    fn a_tree_removed_from_the_watch_is_not_sent_on() {
        let directory = scratch("dropped-tree");
        let root = located(&directory);
        let (mut watch, saves) = opened(&directory);
        assert_eq!(watch.add_tree(&root).unwrap(), Placed::Listening);
        watch.remove_tree(&root);
        fs::write(root.join("one.md"), "# One\n").unwrap();
        assert_eq!(saves.recv_timeout(WAIT), Err(RecvTimeoutError::Timeout));
        // The file the watch was opened on is still listened for.
        let settings = directory.join("settings.toml");
        fs::write(&settings, "theme = \"dark\"\n").unwrap();
        assert_eq!(saves.recv_timeout(WAIT), Ok(settings));
        fs::remove_dir_all(&directory).ok();
    }

    #[test]
    fn a_tree_whose_folder_is_not_there_is_said_so_and_added_again_later() {
        let directory = scratch("no-tree");
        let root = directory.join("Library");
        let (mut watch, saves) = opened(&directory);
        assert_eq!(watch.add_tree(&root).unwrap(), Placed::NoDirectory);
        fs::create_dir_all(&root).unwrap();
        assert_eq!(watch.add_tree(&root).unwrap(), Placed::Listening);
        let file = root.join("one.md");
        fs::write(&file, "# One\n").unwrap();
        assert!(drained(&saves).contains(&file), "the file that was created");
        fs::remove_dir_all(&directory).ok();
    }

    #[test]
    fn a_file_whose_directory_is_not_there_yet_is_said_so_and_added_again_later() {
        let directory = scratch("later");
        let palette = directory.join("theme/quill.toml");
        let (mut watch, saves) = opened(&directory);
        assert_eq!(watch.add(&palette).unwrap(), Placed::NoDirectory);
        fs::create_dir(directory.join("theme")).unwrap();
        assert_eq!(watch.add(&palette).unwrap(), Placed::Listening);
        fs::write(&palette, "[dark]\npaper = \"#101010\"\n").unwrap();
        assert_eq!(saves.recv_timeout(WAIT), Ok(palette));
        fs::remove_dir_all(&directory).ok();
    }

    /// The watch tells a save from an open by length and write time, so the
    /// same bytes saved again are a save to it; whether they are a repaint is
    /// the palette's question, answered by reading both and comparing.
    #[test]
    fn the_same_bytes_saved_twice_are_told_apart_by_the_palette_and_not_the_watch() {
        use crate::theme::Palette;

        let directory = scratch("same");
        let text = "[dark]\npaper = \"#101010\"\n";
        let palette = themed(&directory, "theme", text);
        let (mut watch, saves) = opened(&directory);
        assert_eq!(watch.add(&palette).unwrap(), Placed::Listening);
        let (before, _) = Palette::read_from(&palette);

        fs::write(&palette, text).unwrap();
        drained(&saves);
        let (again, _) = Palette::read_from(&palette);
        assert_eq!(again, before, "the same bytes are not a repaint");

        fs::write(&palette, "[dark]\npaper = \"#202020\"\n").unwrap();
        assert_eq!(saves.recv_timeout(WAIT), Ok(palette.clone()));
        let (changed, _) = Palette::read_from(&palette);
        assert_ne!(changed, before, "and a change is");
        fs::remove_dir_all(&directory).ok();
    }

    #[test]
    fn every_line_of_the_first_version_is_said() {
        let now = version(&["is not TOML", "\"library.toggle\": no such Command"]);
        let unsaid = unsaid(&BTreeSet::new(), &now);
        assert_eq!(unsaid.lines, now);
        assert_eq!(unsaid.said, now.into_iter().collect());
    }

    #[test]
    fn a_line_the_version_before_said_is_not_said_again() {
        let first = version(&["<Super>l belongs to the compositor"]);
        let said = unsaid(&BTreeSet::new(), &first).said;
        let again = unsaid(&said, &first);
        assert_eq!(again.lines, Vec::<String>::new());
        assert_eq!(again.said, said);
    }

    #[test]
    fn a_line_a_version_added_is_said_and_the_ones_beside_it_are_not() {
        let first = version(&["<Super>l belongs to the compositor"]);
        let said = unsaid(&BTreeSet::new(), &first).said;
        let second = version(&["<Super>l belongs to the compositor", "is not TOML"]);
        assert_eq!(unsaid(&said, &second).lines, version(&["is not TOML"]));
    }

    #[test]
    fn a_line_that_left_the_file_and_came_back_is_said_again() {
        let bad = version(&["<Super>l belongs to the compositor"]);
        let said = unsaid(&BTreeSet::new(), &bad).said;
        let mended = unsaid(&said, &[]);
        assert_eq!(mended.lines, Vec::<String>::new());
        assert_eq!(unsaid(&mended.said, &bad).lines, bad);
    }

    #[test]
    fn the_same_line_twice_in_one_version_is_said_once() {
        let now = version(&["is not TOML", "is not TOML"]);
        assert_eq!(
            unsaid(&BTreeSet::new(), &now).lines,
            version(&["is not TOML"])
        );
    }
}
