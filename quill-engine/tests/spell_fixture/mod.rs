//! The temporary copy of the fixture dictionary, `dev/ref/spell/`, shared by the test binaries that
//! load a dictionary.
//!
//! `ENCHANT_CONFIG_DIR` is process-wide and enchant reads it when a broker is made, so every
//! test in a binary that includes this module first calls [`fixture`], which copies the fixture
//! and sets the variable once, before the first broker. That is why those tests live in test
//! binaries of their own: the variable is set while no other test in the process can be reading
//! the environment.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// The temporary copy of `dev/ref/spell/`, made and pointed at once per test process.
pub fn fixture() -> &'static Path {
    static COPY: OnceLock<PathBuf> = OnceLock::new();
    COPY.get_or_init(|| {
        let copy = std::env::temp_dir().join(format!("quill-spell-{}", std::process::id()));
        let _ = fs::remove_dir_all(&copy);
        let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../dev/ref/spell/hunspell");
        let target = copy.join("hunspell");
        fs::create_dir_all(&target).expect("the copy's hunspell directory is made");
        for name in ["en_US.aff", "en_US.dic"] {
            fs::copy(source.join(name), target.join(name)).expect("the fixture pair copies");
        }
        // SAFETY: set once, under the `OnceLock`, before any broker reads it; a binary including
        // this module reads the environment only from tests that wait on this lock first.
        unsafe { std::env::set_var("ENCHANT_CONFIG_DIR", &copy) };
        copy
    })
}
