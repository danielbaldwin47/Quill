//! Spell check: the `SpellChecker` trait and its enchant implementation.
//!
//! The Annotator that underlines misspellings against the desktop's dictionaries, with
//! suggestions fetched on demand. One implementation lands behind the trait, enchant, through
//! Quill's own `extern "C"` block against libenchant-2's ABI rather than the `enchant` crate;
//! `spellbook` is the trait's other door and is not built. The block is the workspace's one
//! native link — `#[link(name = "enchant-2")]`, no build script and no `pkg-config` — so building
//! or testing this crate needs `libenchant-2.so` on the machine. A tag no provider serves is an
//! ordinary `None`, which is the "no dictionary" state; nothing here panics on it.
//!
//! Like every prose Annotator it sees the prose stream, never a `#` or a `*`.

use std::ffi::{CStr, CString, c_char, c_void};
use std::ptr::NonNull;

/// The most corrections [`SpellChecker::suggest`] hands back, the context menu's section size.
pub const SUGGESTIONS: usize = 5;

/// Where a character stands in a word, as enchant's word-character rule reads it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Position {
    /// The word's first character.
    Start,
    /// Any character between the first and the last.
    Middle,
    /// The word's last character.
    End,
}

/// A dictionary the Spell check Annotator asks about one word at a time.
pub trait SpellChecker: Send {
    /// Whether the dictionary, the Personal dictionary or this process's ignored words hold
    /// `word`. A word the backend cannot answer for is held, so an error marks nothing.
    fn check(&self, word: &str) -> bool;

    /// Corrections for `word`, best first, at most [`SUGGESTIONS`] of them.
    fn suggest(&self, word: &str) -> Vec<String>;

    /// Adds `word` to the Personal dictionary, which outlives the process.
    fn add(&mut self, word: &str);

    /// Holds `word` for the rest of this process and writes nothing.
    fn ignore(&mut self, word: &str);

    /// Every installed dictionary's tag, in the backend's own listing order.
    fn languages(&self) -> Vec<String>;

    /// Whether `c` may stand at `position` in a word of this dictionary's language.
    fn is_word_character(&self, c: char, position: Position) -> bool;
}

/// The tags of every installed dictionary, listed through a throwaway broker that loads no
/// dictionary, so the main thread resolves a language without touching the worker's checker.
pub fn installed_languages() -> Vec<String> {
    Broker::new()
        .map(|broker| broker.tags())
        .unwrap_or_default()
}

/// A [`SpellChecker`] over one enchant dictionary, owning the broker that loaded it.
pub struct Enchant {
    // Freed by `Drop` before the fields drop, so before the broker that loaded it.
    dict: NonNull<ffi::EnchantDict>,
    broker: Broker,
    tag: String,
}

// SAFETY: an enchant broker and its dictionaries hold no thread-local state and are safe to use
// from any one thread at a time; `Enchant` owns both handles outright and lends neither out, and
// the mutating calls take `&mut self`, so moving it to the worker thread is sound.
unsafe impl Send for Enchant {}

impl Enchant {
    /// Loads the dictionary for `tag` (`en_US`, `de`), or `None` when no provider serves it.
    pub fn new(tag: &str) -> Option<Self> {
        let broker = Broker::new()?;
        let c_tag = CString::new(tag).ok()?;
        // SAFETY: the broker is live and `c_tag` is a NUL-terminated string that outlives the call.
        let dict = unsafe { ffi::enchant_broker_request_dict(broker.0.as_ptr(), c_tag.as_ptr()) };
        let dict = NonNull::new(dict)?;
        Some(Self {
            dict,
            broker,
            tag: tag.to_owned(),
        })
    }

    /// The tag this dictionary was loaded for.
    pub fn tag(&self) -> &str {
        &self.tag
    }
}

impl SpellChecker for Enchant {
    fn check(&self, word: &str) -> bool {
        if word.is_empty() {
            return true;
        }
        // SAFETY: the dictionary is live, and the pointer and length describe `word`'s bytes.
        let answer =
            unsafe { ffi::enchant_dict_check(self.dict.as_ptr(), word.as_ptr().cast(), len(word)) };
        // 0 is held, a positive value misspelled, a negative one an error.
        answer <= 0
    }

    fn suggest(&self, word: &str) -> Vec<String> {
        if word.is_empty() {
            return Vec::new();
        }
        let mut count = 0usize;
        // SAFETY: as in `check`; `count` is a valid place for the list's length.
        let list = unsafe {
            ffi::enchant_dict_suggest(
                self.dict.as_ptr(),
                word.as_ptr().cast(),
                len(word),
                &mut count,
            )
        };
        if list.is_null() {
            return Vec::new();
        }
        let words = (0..count)
            // SAFETY: enchant hands back `count` valid NUL-terminated strings before the null.
            .map(|i| unsafe { CStr::from_ptr(*list.add(i)) })
            .map(|word| word.to_string_lossy().into_owned())
            .take(SUGGESTIONS)
            .collect();
        // SAFETY: `list` came from this dictionary's `suggest` and is freed exactly once.
        unsafe { ffi::enchant_dict_free_string_list(self.dict.as_ptr(), list) };
        words
    }

    fn add(&mut self, word: &str) {
        if !word.is_empty() {
            // SAFETY: as in `check`.
            unsafe { ffi::enchant_dict_add(self.dict.as_ptr(), word.as_ptr().cast(), len(word)) };
        }
    }

    fn ignore(&mut self, word: &str) {
        if !word.is_empty() {
            // SAFETY: as in `check`.
            unsafe {
                ffi::enchant_dict_add_to_session(
                    self.dict.as_ptr(),
                    word.as_ptr().cast(),
                    len(word),
                )
            };
        }
    }

    fn languages(&self) -> Vec<String> {
        self.broker.tags()
    }

    fn is_word_character(&self, c: char, position: Position) -> bool {
        let n = match position {
            Position::Start => 0,
            Position::Middle => 1,
            Position::End => 2,
        };
        // SAFETY: the dictionary is live; the call reads a code point and a position only.
        unsafe { ffi::enchant_dict_is_word_character(self.dict.as_ptr(), u32::from(c), n) != 0 }
    }
}

impl Drop for Enchant {
    fn drop(&mut self) {
        // SAFETY: the dictionary came from this broker, which is still live, and is freed once.
        unsafe { ffi::enchant_broker_free_dict(self.broker.0.as_ptr(), self.dict.as_ptr()) };
    }
}

/// An enchant broker, freed on drop.
struct Broker(NonNull<ffi::EnchantBroker>);

impl Broker {
    fn new() -> Option<Self> {
        // SAFETY: `enchant_broker_init` takes nothing and returns a new broker or null.
        NonNull::new(unsafe { ffi::enchant_broker_init() }).map(Self)
    }

    /// Every tag the broker's providers serve, first listing kept where two providers share one.
    fn tags(&self) -> Vec<String> {
        unsafe extern "C" fn collect(
            tag: *const c_char,
            _provider_name: *const c_char,
            _provider_desc: *const c_char,
            _provider_file: *const c_char,
            user_data: *mut c_void,
        ) {
            // SAFETY: `user_data` is the `Vec` `tags` passed, alive for the whole listing, and
            // enchant hands a valid NUL-terminated tag.
            let (tags, tag) = unsafe {
                (
                    &mut *user_data.cast::<Vec<String>>(),
                    CStr::from_ptr(tag).to_string_lossy(),
                )
            };
            if !tags.iter().any(|held| *held == tag) {
                tags.push(tag.into_owned());
            }
        }
        let mut tags: Vec<String> = Vec::new();
        // SAFETY: the broker is live, `collect` matches `EnchantDictDescribeFn`, and `tags`
        // outlives the call, which runs the callback synchronously.
        unsafe {
            ffi::enchant_broker_list_dicts(
                self.0.as_ptr(),
                collect,
                (&raw mut tags).cast::<c_void>(),
            )
        };
        tags
    }
}

impl Drop for Broker {
    fn drop(&mut self) {
        // SAFETY: the broker came from `enchant_broker_init` and is freed once, after every
        // dictionary it handed out (`Enchant::drop` runs before its `broker` field drops).
        unsafe { ffi::enchant_broker_free(self.0.as_ptr()) };
    }
}

/// A word's byte length in the `ssize_t` enchant takes.
fn len(word: &str) -> isize {
    isize::try_from(word.len()).unwrap_or(isize::MAX)
}

/// libenchant-2's ABI, declared by hand from `/usr/include/enchant-2/enchant.h`: only the calls
/// the wrapper makes.
mod ffi {
    use std::ffi::{c_char, c_int, c_void};

    /// Opaque `EnchantBroker`.
    #[repr(C)]
    pub struct EnchantBroker {
        _private: [u8; 0],
    }

    /// Opaque `EnchantDict`.
    #[repr(C)]
    pub struct EnchantDict {
        _private: [u8; 0],
    }

    /// `EnchantDictDescribeFn`.
    pub type DescribeFn = unsafe extern "C" fn(
        lang_tag: *const c_char,
        provider_name: *const c_char,
        provider_desc: *const c_char,
        provider_file: *const c_char,
        user_data: *mut c_void,
    );

    #[link(name = "enchant-2")]
    unsafe extern "C" {
        pub fn enchant_broker_init() -> *mut EnchantBroker;
        pub fn enchant_broker_free(broker: *mut EnchantBroker);
        pub fn enchant_broker_request_dict(
            broker: *mut EnchantBroker,
            tag: *const c_char,
        ) -> *mut EnchantDict;
        pub fn enchant_broker_free_dict(broker: *mut EnchantBroker, dict: *mut EnchantDict);
        pub fn enchant_broker_list_dicts(
            broker: *mut EnchantBroker,
            describe: DescribeFn,
            user_data: *mut c_void,
        );
        pub fn enchant_dict_check(dict: *mut EnchantDict, word: *const c_char, len: isize)
        -> c_int;
        pub fn enchant_dict_suggest(
            dict: *mut EnchantDict,
            word: *const c_char,
            len: isize,
            out_n_suggs: *mut usize,
        ) -> *mut *mut c_char;
        pub fn enchant_dict_free_string_list(dict: *mut EnchantDict, list: *mut *mut c_char);
        pub fn enchant_dict_add(dict: *mut EnchantDict, word: *const c_char, len: isize);
        pub fn enchant_dict_add_to_session(dict: *mut EnchantDict, word: *const c_char, len: isize);
        pub fn enchant_dict_is_word_character(dict: *mut EnchantDict, uc: u32, n: usize) -> c_int;
    }
}
