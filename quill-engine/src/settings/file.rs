//! One small file, read whole and written whole.
//!
//! Both of Quill's files — the writer's `settings.toml` and the state Quill
//! leaves for its next launch — are read once and rewritten in full, so the
//! only interesting question is what a half-finished write leaves behind. The
//! answer is nothing: the bytes go to a temporary file in the same directory
//! and are renamed over the destination, which on a POSIX filesystem is atomic.
//! A reader either sees the old file or the new one, never a truncated file and
//! never a settings file that lost a writer's preferences to a full disk.
//!
//! The temporary file is hidden and carries this process's id, so two Quills
//! writing at once cannot write through the same temporary; the loser of the
//! rename simply loses, which is what "last writer wins" means.
//!
//! The reading here is TOML's; the writing is any file's, and a Document's
//! save ([`crate::disk`]) is the third caller of [`write`], because a
//! Document's save asks that same question of a half-finished write and must
//! have that same answer.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// Reads `path`, or `None` when there is no file there yet.
///
/// A missing file is not an error: it is what every first launch finds, and
/// what a writer who deleted the file to start again means.
///
/// # Errors
///
/// Returns the underlying [`io::Error`] for anything else — an unreadable file,
/// a directory in the file's place, bytes that are not UTF-8.
pub fn read(path: &Path) -> io::Result<Option<String>> {
    match fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err),
    }
}

/// The table in `text`, or `None` and one note when it is not TOML at all.
///
/// `instead` finishes that note by saying what Quill is doing about it, which
/// is the only thing the two files differ in: the writer's settings fall back
/// to the defaults, and state starts fresh.
pub fn parse(text: &str, instead: &str) -> (Option<toml::Table>, Vec<String>) {
    match text.parse::<toml::Table>() {
        Ok(table) => (Some(table), Vec::new()),
        Err(err) => (
            None,
            vec![format!("is not TOML ({}); {instead}", reason(text, &err))],
        ),
    }
}

/// Why `text` is not TOML, on one line: the line the error is on, the reason
/// the parser gives, and the text it points at.
///
/// The error's own rendering is a position, the offending line quoted with a
/// caret under it, and then the reason — four lines, and a log line that
/// wraps four times reads as a crash. Its first line alone was tried and was a
/// position with no reason: `TOML parse error at line 7, column 1` for a key
/// named twice, which a writer reads as "line 7 is wrong" and then finds
/// nothing wrong with line 7 on its own. The reason is `duplicate key` with no
/// key in it, so the text under the caret is quoted after it, which for that
/// error is the key. Only the first line of the reason, because the parser
/// hands some of them a second line saying what it expected instead.
fn reason(text: &str, err: &toml::de::Error) -> String {
    let message = err.message().lines().next().unwrap_or_default();
    let Some(span) = err.span() else {
        return message.to_owned();
    };
    let start = span.start.min(text.len());
    let line = text[..start].matches('\n').count() + 1;
    let at = &text[start..span.end.min(text.len())];
    if at.is_empty() || at.contains('\n') {
        format!("line {line}: {message}")
    } else {
        format!("line {line}: {message} at {at}")
    }
}

/// The table `path` holds: `None` for a file that is not there, cannot be read,
/// or is not TOML, with one note for each of the last two.
pub fn read_table(path: &Path, instead: &str) -> (Option<toml::Table>, Vec<String>) {
    match read(path) {
        Ok(None) => (None, Vec::new()),
        Ok(Some(text)) => parse(&text, instead),
        Err(err) => (None, vec![format!("cannot be read ({err}); {instead}")]),
    }
}

/// Writes `contents` to `path` without ever leaving a partial file there.
///
/// Creates the directory the file lives in, writes a temporary file beside it,
/// flushes it to the disk, puts on it the permissions the file being replaced
/// had, and renames it over `path`. A failure at any step removes the
/// temporary and leaves whatever was at `path` untouched.
///
/// The permissions are carried over because this writes the writer's
/// Documents as well as Quill's own two files (`docs/architecture.md`
/// § Documents and files, "renamed over the original with its permissions
/// preserved"): a Document the writer made read-only, or shared with a group,
/// is the same file after a save.
///
/// # Errors
///
/// Returns the underlying [`io::Error`] when the directory cannot be made or
/// the file cannot be written or renamed.
pub fn write(path: &Path, contents: &str) -> io::Result<()> {
    let Some(directory) = path.parent() else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{} is not a file Quill can write", path.display()),
        ));
    };
    fs::create_dir_all(directory)?;
    let temporary = temporary(path);
    let replacing = fs::metadata(path)
        .map(|metadata| metadata.permissions())
        .ok();
    let written = write_and_flush(&temporary, contents)
        .and_then(|()| match replacing {
            Some(permissions) => fs::set_permissions(&temporary, permissions),
            None => Ok(()),
        })
        .and_then(|()| fs::rename(&temporary, path));
    if written.is_err() {
        // Best effort: the write has already failed, and a temporary nobody
        // can remove is not worth a second error nobody can act on.
        fs::remove_file(&temporary).ok();
    }
    written
}

/// The temporary file [`write`] writes through, beside its destination.
///
/// Named from the destination and this process's id rather than at random, so
/// that a test can stand in its way and watch the write fail.
#[must_use]
pub fn temporary(path: &Path) -> PathBuf {
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    path.with_file_name(format!(".{name}.{}.tmp", std::process::id()))
}

/// Writes the bytes and gets them onto the disk, so that the rename swaps a
/// whole file rather than one the kernel has not finished with.
fn write_and_flush(path: &Path, contents: &str) -> io::Result<()> {
    let mut file = fs::File::create(path)?;
    file.write_all(contents.as_bytes())?;
    file.sync_all()
}

/// An empty directory of one test's own, named for it.
///
/// Every test here writes real files, because the thing under test is what
/// happens to real files. They each get a directory of their own so that they
/// still run in any order and all at once.
#[cfg(test)]
pub fn scratch(name: &str) -> PathBuf {
    let directory = std::env::temp_dir().join(format!("quill-{name}-{}", std::process::id()));
    fs::remove_dir_all(&directory).ok();
    fs::create_dir_all(&directory).expect("makes its own scratch directory");
    directory
}

/// What is in `directory`, sorted, so that anything left behind shows up.
#[cfg(test)]
pub fn names_in(directory: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(directory)
        .expect("reads a directory a test made")
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_file_reads_as_nothing_rather_than_an_error() {
        let path = scratch("missing").join("settings.toml");
        let read = read(&path).expect("a missing file is not a failure");
        assert!(read.is_none());
    }

    #[test]
    fn what_was_written_is_what_is_read_back() {
        let path = scratch("round_trip").join("settings.toml");
        write(&path, "size = 20\n").expect("writes into a directory it makes itself");
        let read = read(&path).expect("reads what it wrote");
        assert_eq!(read.as_deref(), Some("size = 20\n"));
    }

    #[test]
    fn a_second_write_replaces_the_first_and_leaves_no_temporary_behind() {
        let directory = scratch("replace");
        let path = directory.join("settings.toml");
        write(&path, "size = 20\n").expect("writes the first file");
        write(&path, "size = 22\n").expect("writes over the first file");
        let read = read(&path).expect("reads the second write");
        assert_eq!(read.as_deref(), Some("size = 22\n"));
        assert_eq!(names_in(&directory), ["settings.toml"]);
    }

    #[test]
    fn a_write_that_fails_leaves_the_old_file_whole() {
        let directory = scratch("failure");
        let path = directory.join("settings.toml");
        write(&path, "size = 20\n").expect("writes the file that must survive");
        // A directory where the temporary file goes: the write cannot make its
        // file, which is the shape a full disk or a read-only home has, and
        // the destination must come through it untouched.
        fs::create_dir(temporary(&path)).expect("stands in the way of the temporary");
        write(&path, "size = 22\n").expect_err("cannot write through a directory");
        let read = read(&path).expect("the old file is still there");
        assert_eq!(read.as_deref(), Some("size = 20\n"));
    }

    #[test]
    fn a_file_that_is_not_toml_is_no_table_and_one_line_about_it() {
        let (table, notes) = parse("theme = = = dark\n", "using the defaults");
        assert!(table.is_none());
        assert_eq!(notes.len(), 1, "{notes:?}");
        assert!(notes[0].starts_with("is not TOML"), "{notes:?}");
        assert!(notes[0].ends_with("using the defaults"), "{notes:?}");
        assert!(!notes[0].contains('\n'), "one line, not a stack: {notes:?}");
    }

    #[test]
    fn a_missing_file_is_no_table_and_nothing_to_say() {
        let path = scratch("read_table_missing").join("state.toml");
        let (table, notes) = read_table(&path, "starting fresh");
        assert!(table.is_none());
        assert!(notes.is_empty(), "a first launch is not news: {notes:?}");
    }

    #[test]
    fn the_temporary_sits_beside_its_destination() {
        // The rename is atomic only within one filesystem, so the temporary
        // has to be in the destination's own directory.
        let path = Path::new("/home/writer/.config/quill/settings.toml");
        assert_eq!(temporary(path).parent(), path.parent());
        assert_ne!(temporary(path), path.to_path_buf());
    }
}
