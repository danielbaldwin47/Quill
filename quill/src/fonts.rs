//! The Faces: Quill Duo, Quill Quattro and Quill Mono, private to this process.
//!
//! The six files under the data directory's `fonts/` are renamed derivatives of
//! the iA Writer typefaces, built by `tools/fontbuild.py`
//! (`docs/adr/0007-quill-faces-renamed-and-private.md`). Startup hands the
//! directory to fontconfig with `FcConfigAppFontAddDir`, so the Faces exist for
//! Quill and for nothing else: they never appear in another application's font
//! picker, a writer installs nothing, and a development build renders in them
//! straight out of the checkout. The spike's `FONTCONFIG_FILE` and its
//! `match target="scan"` rule are gone with the rename.
//!
//! Each Italic is a family of its own. The files declare themselves roman —
//! subfamily "Regular", the OS/2 italic bit clear — so asking for "Quill Duo"
//! in an italic style would get a slanted Roman rather than the italic cut. The
//! Editor asks for "Quill Duo Italic" by name instead.
//!
//! This is the only fontconfig in Quill. It is a handful of C calls, declared
//! here rather than pulled in as a crate: the whole of what Quill wants from
//! fontconfig is "take this directory" and "what would you draw this family
//! from?".

use std::ffi::{CStr, CString, OsString, c_char, c_int, c_uchar};
use std::fmt;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};
use std::ptr::null_mut;

use quill_engine::data;

/// Why the Faces are not available.
///
/// Neither case stops Quill: a writer with the wrong typeface can still open
/// and save their Document, and a message they can read beats a window that
/// silently renders in the system sans.
#[derive(Debug)]
pub enum FaceError {
    /// There is no Faces directory to read.
    Directory(PathBuf),
    /// The directory was taken, but a Face is not in it: fontconfig answered
    /// the family name with something else, or with nothing at all.
    Missing {
        /// The family that was asked for.
        wanted: &'static str,
        /// What fontconfig offered instead.
        instead: Option<Match>,
    },
}

impl fmt::Display for FaceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Directory(path) => {
                write!(f, "no Faces directory at {}", path.display())
            }
            Self::Missing {
                wanted,
                instead: Some(found),
            } => write!(
                f,
                "no Face named {wanted}: text will be set in {} ({})",
                found.family.to_string_lossy(),
                found.file.display()
            ),
            Self::Missing {
                wanted,
                instead: None,
            } => write!(f, "no Face named {wanted}, and fontconfig offered nothing"),
        }
    }
}

/// What fontconfig answered when asked for a family.
#[derive(Debug)]
pub struct Match {
    /// The family it matched: the one that was asked for when the Face is
    /// there, and whatever it fell back to when it is not.
    pub family: OsString,
    /// The file it would draw the glyphs from.
    pub file: PathBuf,
}

/// Adds `fonts` to this process's fontconfig and checks the six Faces answer.
///
/// Call this before GTK initialises. Pango builds its font map from the current
/// fontconfig the first time it lays anything out, and a Face added after that
/// is a Face the first frame did not have.
///
/// # Errors
///
/// [`FaceError::Directory`] if fontconfig will not take the directory,
/// [`FaceError::Missing`] if it takes it but a Face is not in it.
pub fn load_private(fonts: &Path) -> Result<(), FaceError> {
    // fontconfig takes a directory that does not exist, an empty one and a
    // plain file all the same way, and says yes to each: `FcConfigAppFontAddDir`
    // reports whether it could record the directory, not whether it found a
    // font. So the honest "there is nothing here" is asked before the call, and
    // "the Faces are not in it" is asked after, by matching.
    if !fonts.is_dir() {
        return Err(FaceError::Directory(fonts.to_path_buf()));
    }
    let directory = CString::new(fonts.as_os_str().as_bytes())
        .map_err(|_| FaceError::Directory(fonts.to_path_buf()))?;
    // SAFETY: `directory` is a NUL-terminated string that outlives the call. A
    // null config is fontconfig's own way of naming the current one, which is
    // the one Pango will read.
    let added = unsafe { FcConfigAppFontAddDir(null_mut(), directory.as_ptr().cast()) };
    if added == 0 {
        return Err(FaceError::Directory(fonts.to_path_buf()));
    }
    for (wanted, _) in data::FACES {
        let instead = matched(wanted);
        // fontconfig always answers something, so "did it answer?" proves
        // nothing; the question is whether it answered with the Face's own
        // name.
        if instead.as_ref().is_none_or(|found| found.family != *wanted) {
            return Err(FaceError::Missing { wanted, instead });
        }
    }
    Ok(())
}

/// What fontconfig would draw `family` from, after the usual substitutions.
///
/// `None` only when fontconfig has no fonts at all; normally it answers with
/// its own fallback, which is why callers compare the family that comes back.
#[must_use]
pub fn matched(family: &str) -> Option<Match> {
    let wanted = CString::new(family).ok()?;
    // SAFETY: every pointer here comes from fontconfig and goes straight back
    // to it. Both patterns are destroyed on every path out, and the strings are
    // copied before the pattern that owns them is freed.
    unsafe {
        let pattern = FcPatternCreate();
        if pattern.is_null() {
            return None;
        }
        FcPatternAddString(pattern, FC_FAMILY.as_ptr(), wanted.as_ptr().cast());
        FcConfigSubstitute(null_mut(), pattern, FC_MATCH_PATTERN);
        FcDefaultSubstitute(pattern);
        let mut result: c_int = 0;
        let found = FcFontMatch(null_mut(), pattern, &raw mut result);
        FcPatternDestroy(pattern);
        if found.is_null() || result != FC_RESULT_MATCH {
            return None;
        }
        let answer = string(found, FC_FAMILY).zip(string(found, FC_FILE));
        FcPatternDestroy(found);
        answer.map(|(family, file)| Match {
            family,
            file: PathBuf::from(file),
        })
    }
}

/// The first value of `pattern`'s `object` property, copied out.
///
/// # Safety
///
/// `pattern` must be a live pattern from fontconfig.
unsafe fn string(pattern: *mut FcPattern, object: &CStr) -> Option<OsString> {
    let mut value: *mut c_uchar = null_mut();
    // SAFETY: the caller's pattern is live, and fontconfig writes a pointer it
    // owns into `value`, valid until the pattern is destroyed. Nothing here
    // outlives that: the bytes are copied before returning.
    unsafe {
        if FcPatternGetString(pattern, object.as_ptr(), 0, &raw mut value) != FC_RESULT_MATCH
            || value.is_null()
        {
            return None;
        }
        Some(OsString::from_vec(
            CStr::from_ptr(value.cast::<c_char>()).to_bytes().to_vec(),
        ))
    }
}

/// fontconfig's `FC_FAMILY`.
const FC_FAMILY: &CStr = c"family";
/// fontconfig's `FC_FILE`.
const FC_FILE: &CStr = c"file";
/// `FcMatchPattern`: substitute the rules that apply to what was asked for.
const FC_MATCH_PATTERN: c_int = 0;
/// `FcResultMatch`.
const FC_RESULT_MATCH: c_int = 0;

/// fontconfig's `FcConfig`, only ever held as a pointer.
enum FcConfig {}
/// fontconfig's `FcPattern`, only ever held as a pointer.
enum FcPattern {}

#[allow(non_snake_case)] // fontconfig's own names, so they can be looked up.
#[link(name = "fontconfig")]
unsafe extern "C" {
    fn FcConfigAppFontAddDir(config: *mut FcConfig, dir: *const c_uchar) -> c_int;
    fn FcConfigSubstitute(config: *mut FcConfig, pattern: *mut FcPattern, kind: c_int) -> c_int;
    fn FcDefaultSubstitute(pattern: *mut FcPattern);
    fn FcFontMatch(
        config: *mut FcConfig,
        pattern: *mut FcPattern,
        result: *mut c_int,
    ) -> *mut FcPattern;
    fn FcPatternAddString(
        pattern: *mut FcPattern,
        object: *const c_char,
        value: *const c_uchar,
    ) -> c_int;
    fn FcPatternCreate() -> *mut FcPattern;
    fn FcPatternDestroy(pattern: *mut FcPattern);
    fn FcPatternGetString(
        pattern: *mut FcPattern,
        object: *const c_char,
        index: c_int,
        value: *mut *mut c_uchar,
    ) -> c_int;
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;

    use super::*;

    /// One test, not six: `FcConfigAppFontAddDir` changes the process's own
    /// fontconfig, and `cargo test` runs its tests in parallel threads.
    #[test]
    fn the_faces_resolve_to_the_shipped_files() {
        assert!(
            std::env::var_os("FONTCONFIG_FILE").is_none(),
            "$FONTCONFIG_FILE is set, so this test would prove nothing: ADR 0007 retired the \
             override the spike used, and the Faces have to be found without it"
        );

        let fonts = data::fonts();
        load_private(&fonts).unwrap_or_else(|err| panic!("the Faces load from {fonts:?}: {err}"));

        for (family, file) in data::FACES {
            let found = matched(family).expect("fontconfig answers");
            assert_eq!(
                found.family,
                OsStr::new(family),
                "fontconfig matched {family} with something else"
            );
            assert_eq!(
                found.file,
                fonts.join(file),
                "{family} came from a font Quill does not ship"
            );
        }
    }

    #[test]
    fn a_directory_that_is_not_there_is_said_so() {
        let err = load_private(Path::new("/nowhere/quill/fonts"))
            .expect_err("there are no Faces in a directory that does not exist");
        assert!(matches!(err, FaceError::Directory(_)), "got {err}");
    }
}
