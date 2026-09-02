//! A file saved under a running Quill: what a save arrives on, and the rule
//! that keeps a line Quill cannot apply from being said twice.
//!
//! [`Watch`] watches the *directory* a file sits in rather than the file
//! itself, because a save is a write to a temporary file and a rename over the
//! top — an editor's, and Quill's own ([`crate::settings::Settings::write_to`])
//! — and a watch on the path alone follows the file it was pointed at out of
//! the way. Events are gathered up for [`DEBOUNCE`] first, so the two halves of
//! one save are one save. The settings file is the only path watched today; the
//! Documents and the Library add theirs later (the File handling spec) through
//! [`Watch::add`], which is why what is being listened for is a set rather than
//! the one path the settings watch needs.
//!
//! The app owns the [`Watch`] and drains its [`Receiver`] from its own main
//! loop: the engine has no `glib` (ADR 0008), and the thread the debouncer
//! answers on is not one a window can be touched from.
//!
//! [`unsaid`] is the other half of a re-read — what a version of a file has to
//! say that the version before it did not already say — because a writer who
//! saves the same refused line twice should hear about it once.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use notify::{RecommendedWatcher, RecursiveMode};
use notify_debouncer_mini::{DebounceEventResult, DebouncedEventKind, Debouncer, new_debouncer};

/// How long the events of one save are gathered up before they are one save.
///
/// A save is at least a create and a rename, and an editor writing in two
/// steps is more; 100 ms is long enough to hold them together and short
/// enough that a writer who saved sees the page follow.
pub const DEBOUNCE: Duration = Duration::from_millis(100);

/// The files a [`Watch`] is listening for, each with the version of it last
/// sent on, as its debouncer's thread reads them.
type Listening = Arc<Mutex<BTreeMap<PathBuf, Option<Version>>>>;

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

/// A watch on one or more files, which stops when it is dropped.
pub struct Watch {
    /// The debouncer, whose thread and whose watches end with it.
    debouncer: Debouncer<RecommendedWatcher>,
    /// The files whose saves are sent on, shared with that thread.
    listening: Listening,
    /// The directories already handed to the watcher, so that a second file
    /// in one of them costs no second watch.
    directories: BTreeSet<PathBuf>,
}

impl Watch {
    /// Watches `path`, and answers with the receiver every save of it arrives
    /// on.
    ///
    /// The path arrives as it is written here rather than as the event
    /// reported it, so a caller can compare it against the path it asked for.
    ///
    /// # Errors
    ///
    /// Returns the watcher's own error where the directory holding `path`
    /// cannot be resolved or cannot be watched.
    pub fn on(path: &Path) -> Result<(Self, Receiver<PathBuf>), notify::Error> {
        let (sender, receiver) = mpsc::channel();
        let listening: Listening = Listening::default();
        let heard = Arc::clone(&listening);
        let debouncer = new_debouncer(DEBOUNCE, move |events: DebounceEventResult| {
            send(&heard, &sender, events);
        })?;
        let mut watch = Self {
            debouncer,
            listening,
            directories: BTreeSet::new(),
        };
        watch.add(path)?;
        Ok((watch, receiver))
    }

    /// Listens for `path` as well, on the same receiver.
    ///
    /// # Errors
    ///
    /// Returns the watcher's own error where the directory holding `path`
    /// cannot be resolved or cannot be watched.
    pub fn add(&mut self, path: &Path) -> Result<(), notify::Error> {
        let (directory, file) = place(path)?;
        if let Ok(mut listening) = self.listening.lock() {
            let now = version(&file);
            listening.insert(file, now);
        }
        if self.directories.insert(directory.clone()) {
            self.debouncer
                .watcher()
                .watch(&directory, RecursiveMode::NonRecursive)?;
        }
        Ok(())
    }
}

/// Sends on every watched file the events say has settled, and nothing else.
///
/// The temporary file a save is written through is in the same directory and
/// is reported like anything else there, so what is sent on is what was asked
/// for rather than what moved. A file still being written is
/// [`DebouncedEventKind::AnyContinuous`] and is not sent on: a run of writes
/// is one save, and the [`DebouncedEventKind::Any`] that ends it is the one
/// worth reading the file at.
///
/// An error the watcher itself reports is dropped: the settings watch is never
/// fatal (`docs/architecture.md` § Settings), and a watch that has stopped
/// answering leaves a Quill running on the settings it read.
fn send(listening: &Listening, sender: &Sender<PathBuf>, events: DebounceEventResult) {
    let (Ok(events), Ok(mut listening)) = (events, listening.lock()) else {
        return;
    };
    for event in events {
        if event.kind != DebouncedEventKind::Any {
            continue;
        }
        let Some(known) = listening.get_mut(&event.path) else {
            continue;
        };
        // A version already sent on is an open, not a save ([`Version`]).
        let now = version(&event.path);
        if now == *known {
            continue;
        }
        *known = now;
        let _ = sender.send(event.path);
    }
}

/// The directory to watch for `path`, and the path its saves are reported
/// under.
///
/// The directory is resolved and the file name put back on it, because an
/// event's path is the watched directory's with a name joined to it: a
/// relative path, or one through a symbolic link, would never be equal to a
/// path reported for the same file.
fn place(path: &Path) -> Result<(PathBuf, PathBuf), notify::Error> {
    let directory = match path.parent() {
        Some(directory) if !directory.as_os_str().is_empty() => directory,
        _ => Path::new("."),
    };
    let name = path
        .file_name()
        .ok_or_else(|| notify::Error::generic("names no file to watch"))?;
    let directory = directory.canonicalize()?;
    let file = directory.join(name);
    Ok((directory, file))
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
        watch.add(&later).unwrap();
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
