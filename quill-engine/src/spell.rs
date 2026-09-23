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
use std::ops::Range;
use std::ptr::NonNull;

use crate::document::Splice;

/// The most corrections [`SpellChecker::suggest`] hands back, the context menu's section size.
pub const SUGGESTIONS: usize = 5;

/// A misspelled word's span kind in the worker's answer; the range is the whole mark.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Misspelling;

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

/// The words of `prose`, as ascending byte ranges into it, by `is_word_character`.
///
/// A word starts at a character admitted at [`Position::Start`], runs on over every character
/// admitted at [`Position::Middle`], and gives back whatever trailing characters are refused at
/// [`Position::End`], so the dictionary's apostrophe and hyphen rules decide what a word is:
/// `don't` is one word where `'` may stand mid-word and two where it may not, and a closing `'`
/// stays outside. Two kinds of token are dropped rather than checked, as the Design oracle drops
/// them (`dev/ref/ia/mac-native/NOTES.md` § State 26 § What is marked, and `docs/design.md` row *What a
/// spell mark covers*): one containing a digit (`2b`, `Q3`), since no dictionary spells it, and an
/// **all-caps** one (`DRAFFT`), since an acronym or a shout is not a spelling a dictionary can
/// answer for. A CamelCase token is a word like any other — it has lower-case letters in it.
pub fn words(prose: &str, is_word_character: impl Fn(char, Position) -> bool) -> Vec<Range<usize>> {
    let chars: Vec<(usize, char)> = prose.char_indices().collect();
    let end_of = |at: usize| chars.get(at).map_or(prose.len(), |&(offset, _)| offset);
    let mut words = Vec::new();
    let mut at = 0;
    while at < chars.len() {
        if !is_word_character(chars[at].1, Position::Start) {
            at += 1;
            continue;
        }
        let start = at;
        let mut next = at + 1;
        while next < chars.len() && is_word_character(chars[next].1, Position::Middle) {
            next += 1;
        }
        let mut last = next - 1;
        while last > start && !is_word_character(chars[last].1, Position::End) {
            last -= 1;
        }
        let word = chars[start].0..end_of(last + 1);
        if checkable(&prose[word.clone()]) {
            words.push(word);
        }
        at = next;
    }
    words
}

/// Whether `token` is a word a dictionary is asked about at all.
///
/// The two skips [`words`] names: a token carrying a digit, and one whose letters are all capitals.
/// "All capitals" is *no lower-case letter and at least one upper-case* rather than "every
/// character is upper-case", so `DRAFFT`'s own apostrophes and hyphens do not save it and a token
/// of no letters at all — left by a dictionary whose `is_word_character` admits more than letters —
/// is not mistaken for a shout.
fn checkable(token: &str) -> bool {
    if token.chars().any(char::is_numeric) {
        return false;
    }
    let shouted = token.chars().any(char::is_uppercase) && !token.chars().any(char::is_lowercase);
    !shouted
}

/// The words of `prose` that `checker` does not hold, as ascending byte ranges into it.
pub fn misspelled(prose: &str, checker: &dyn SpellChecker) -> Vec<Range<usize>> {
    words(prose, |c, position| checker.is_word_character(c, position))
        .into_iter()
        .filter(|word| !checker.check(&prose[word.clone()]))
        .collect()
}

/// The one misspelling span to leave unmarked while the writer is still typing its word, or `None`.
///
/// `spans` are a paragraph's misspellings, `caret` the caret's byte offset into `prose`, the
/// same paragraph, and `typed` where the last edit left a word being typed ([`typed_to`]), or
/// `None` once a deletion or a caret move has come since. Only a caret still standing where a
/// word character was just typed withholds anything, so a caret moved into a misspelling, or a
/// Backspace into one, leaves its wave standing (#401 § The tokeniser and the rules).
///
/// While the character before that caret is a word character, the span the caret stands inside
/// or at the end of is withheld — and so is a span the word being typed has grown past, since the
/// spans may predate the last keystroke. A caret at a span's start or outside every word
/// withholds nothing. The rule is a fixed one, letters, digits, apostrophes and hyphens, because
/// the Editor applies it at paint, where no dictionary is at hand.
pub fn withheld(
    spans: &[Range<usize>],
    caret: usize,
    prose: &str,
    typed: Option<usize>,
) -> Option<Range<usize>> {
    if typed != Some(caret) {
        return None;
    }
    let before = prose.get(..caret)?;
    let word_start = before
        .char_indices()
        .rev()
        .take_while(|&(_, c)| is_typed_word_character(c))
        .last()?
        .0;
    spans
        .iter()
        .rev()
        .find(|span| span.start < caret && span.end > word_start)
        .cloned()
}

/// Where `splice` leaves a word being typed in `text`, the text after it: the end of what it
/// put in when the last character it put in is a word character, and `None` for a deletion or
/// an insertion ending in anything else.
///
/// What arms [`withheld`]: typing a word character is the one edit that hides a wave.
pub fn typed_to(text: &str, splice: &Splice) -> Option<usize> {
    let end = splice.at.start + splice.inserted;
    let last = text.get(splice.at.start..end)?.chars().next_back()?;
    is_typed_word_character(last).then_some(end)
}

/// Where a misspelling at `span`, in the text as it stood before `splice`, lies in `text`, the
/// text after it, or `None` once it is no longer a verdict on the same word.
///
/// A mark is carried across an edit until the worker's answer lands, and a mark carried onto a
/// word whose letters changed paints a verdict nobody gave: a stale `wou` grown by `l`, `d` and
/// a space waved a correctly spelled `would` for the length of one debounce (#401 Hand test).
/// So the splice kills it when it takes out a byte of the word, or leaves a word character
/// against either of its edges — typed or pasted there, or brought there by deleting what lay
/// between. Anything else leaves the word as it was, and the mark keeps exactly its letters: a
/// space typed after it does not widen the wave under that space while the answer is on its
/// way (#401 Hand test), which the colours' carry, growing a span over what is typed at its end,
/// would.
pub fn carried(span: &Range<usize>, splice: &Splice, text: &str) -> Option<Range<usize>> {
    let at = &splice.at;
    if at.start < span.end && at.end > span.start {
        return None;
    }
    let joined = |c: Option<char>| c.is_some_and(is_typed_word_character);
    if at.start == span.end && joined(text.get(at.start..).and_then(|after| after.chars().next())) {
        return None;
    }
    let before = text.get(..at.start + splice.inserted);
    if at.end == span.start && joined(before.and_then(|before| before.chars().next_back())) {
        return None;
    }
    if span.end <= at.start {
        return Some(span.clone());
    }
    let shift = |offset: usize| offset - at.len() + splice.inserted;
    Some(shift(span.start)..shift(span.end))
}

/// The characters [`withheld`] reads as the word under the caret.
fn is_typed_word_character(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '\'' | '’' | '-')
}

/// The dictionary a wanted language resolves to among the installed ones.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Resolved {
    /// The wanted tag itself is installed.
    Exact(String),
    /// Another tag of the wanted language stands in for it.
    Fallback {
        /// The installed tag to load.
        tag: String,
        /// The tag that was asked for.
        wanted: String,
    },
    /// No installed dictionary serves the language: the "no dictionary" state.
    Missing {
        /// The tag that was asked for, for Settings to name.
        wanted: String,
    },
}

impl Resolved {
    /// The tag to load, or `None` in the "no dictionary" state.
    pub fn tag(&self) -> Option<&str> {
        match self {
            Self::Exact(tag) | Self::Fallback { tag, .. } => Some(tag),
            Self::Missing { .. } => None,
        }
    }

    /// The tag that was asked for.
    pub fn wanted(&self) -> &str {
        match self {
            Self::Exact(wanted) | Self::Fallback { wanted, .. } | Self::Missing { wanted } => {
                wanted
            }
        }
    }
}

/// Resolves `wanted` against `installed`, listed in the backend's own order: the exact tag, else
/// the bare language (`en` for `en_US`), else the first installed tag of that language, else
/// [`Resolved::Missing`].
pub fn resolve<S: AsRef<str>>(wanted: &str, installed: &[S]) -> Resolved {
    let installed = || installed.iter().map(AsRef::as_ref);
    if installed().any(|tag| tag == wanted) {
        return Resolved::Exact(wanted.to_owned());
    }
    let language = language_of(wanted);
    let fallback = installed()
        .find(|&tag| tag == language)
        .or_else(|| installed().find(|&tag| language_of(tag) == language));
    match fallback {
        Some(tag) => Resolved::Fallback {
            tag: tag.to_owned(),
            wanted: wanted.to_owned(),
        },
        None => Resolved::Missing {
            wanted: wanted.to_owned(),
        },
    }
}

/// Resolves the `spell_language` setting against `installed`: empty is System default, the tag
/// [`locale_tag`] reads through `locale`; any other value is the tag itself; then [`resolve`].
pub fn resolve_setting<S: AsRef<str>>(
    language: &str,
    installed: &[S],
    locale: impl Fn(&str) -> Option<String>,
) -> Resolved {
    if language.is_empty() {
        resolve(&locale_tag(locale), installed)
    } else {
        resolve(language, installed)
    }
}

/// A tag's bare language: `en` from `en_US` or `en_US-large`.
fn language_of(tag: &str) -> &str {
    tag.split(['_', '-']).next().unwrap_or(tag)
}

/// The dictionary tag the process locale wants: the first set of `LC_ALL`, `LC_MESSAGES` and
/// `LANG`, read through `var`, up to any `.` or `@` (`en_US` from `en_US.UTF-8`). `C`, `POSIX`
/// and no value at all want `en_US`, the dictionary the package depends on.
pub fn locale_tag(var: impl Fn(&str) -> Option<String>) -> String {
    let value = ["LC_ALL", "LC_MESSAGES", "LANG"]
        .into_iter()
        .filter_map(var)
        .find(|value| !value.is_empty())
        .unwrap_or_default();
    let tag = value.split(['.', '@']).next().unwrap_or_default();
    match tag {
        "" | "C" | "POSIX" => "en_US".to_owned(),
        tag => tag.to_owned(),
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Letters and digits anywhere, and `'` and `-` between two of them.
    fn joining(c: char, position: Position) -> bool {
        c.is_alphanumeric() || (position == Position::Middle && matches!(c, '\'' | '-'))
    }

    /// Letters and digits alone.
    fn splitting(c: char, _: Position) -> bool {
        c.is_alphanumeric()
    }

    fn texts<'a>(prose: &'a str, spans: &[Range<usize>]) -> Vec<&'a str> {
        spans.iter().map(|span| &prose[span.clone()]).collect()
    }

    #[test]
    fn an_apostrophe_or_hyphen_joins_a_word_only_where_the_rule_admits_it_mid_word() {
        let prose = "don't well-known";
        assert_eq!(
            texts(prose, &words(prose, joining)),
            ["don't", "well-known"]
        );
        assert_eq!(
            texts(prose, &words(prose, splitting)),
            ["don", "t", "well", "known"]
        );
    }

    #[test]
    fn a_token_with_a_digit_or_all_in_capitals_is_dropped_and_camel_case_is_kept() {
        let prose = "2b ships in Q3 as v3, DRAFFT and CamelCase.";
        assert_eq!(
            texts(prose, &words(prose, joining)),
            ["ships", "in", "as", "and", "CamelCase"]
        );
    }

    /// The all-caps skip is the letters' case and not the token's every character: a hyphen or an
    /// apostrophe inside a shout is no lower-case letter, and a token of neither case is a word.
    #[test]
    fn a_shout_is_dropped_through_its_punctuation_and_a_caseless_token_is_not() {
        let prose = "DRAFFT-TWO and DON'T and \u{4e2d}\u{6587}";
        assert_eq!(
            texts(prose, &words(prose, joining)),
            ["and", "and", "\u{4e2d}\u{6587}"]
        );
    }

    #[test]
    fn a_trailing_apostrophe_refused_at_the_end_stays_outside_the_word() {
        let prose = "the writers' notes";
        assert_eq!(
            texts(prose, &words(prose, joining)),
            ["the", "writers", "notes"]
        );
    }

    #[test]
    fn the_caret_at_a_misspelling_s_end_withholds_that_span_alone() {
        let prose = "Teh misteak";
        let spans = [0..3, 4..11];
        assert_eq!(withheld(&spans, 11, prose, Some(11)), Some(4..11));
        assert_eq!(withheld(&spans, 7, prose, Some(7)), Some(4..11));
        assert_eq!(withheld(&spans, 3, prose, Some(3)), Some(0..3));
    }

    #[test]
    fn a_space_a_span_s_start_or_no_span_withholds_nothing() {
        let prose = "misteak and more";
        let spans = [Range { start: 0, end: 7 }];
        assert_eq!(withheld(&spans, 8, "misteak  and more", Some(8)), None);
        assert_eq!(withheld(&spans, 0, prose, Some(0)), None);
        assert_eq!(withheld(&spans, 11, prose, Some(11)), None);
        assert_eq!(withheld(&[], 7, prose, Some(7)), None);
    }

    #[test]
    fn a_word_typed_past_its_stale_span_is_still_withheld() {
        let spans = [Range { start: 0, end: 6 }];
        let typed = typed_to(
            "misteak",
            &Splice {
                at: 6..6,
                inserted: 1,
            },
        );
        assert_eq!(typed, Some(7));
        assert_eq!(withheld(&spans, 7, "misteak", typed), Some(0..6));
    }

    #[test]
    fn a_caret_moved_into_a_misspelling_not_typed_withholds_nothing() {
        let prose = "the comittee met";
        let spans = [Range { start: 4, end: 12 }];
        // Placed at its end by a click or an arrow: no edit armed the rule.
        assert_eq!(withheld(&spans, 12, prose, None), None);
        // Typed there, then moved away and back: the move disarmed it.
        assert_eq!(withheld(&spans, 8, prose, Some(12)), None);
    }

    #[test]
    fn a_backspace_into_a_misspelling_leaves_its_wave_standing() {
        // ` misteak` typed, then Space, then Backspace over the space.
        let space = typed_to(
            "misteak ",
            &Splice {
                at: 7..7,
                inserted: 1,
            },
        );
        assert_eq!(space, None, "a space arms nothing");
        let backspace = typed_to(
            "misteak",
            &Splice {
                at: 7..8,
                inserted: 0,
            },
        );
        assert_eq!(backspace, None, "a deletion arms nothing");
        let spans = [Range { start: 0, end: 7 }];
        assert_eq!(withheld(&spans, 7, "misteak", backspace), None);
    }

    #[test]
    fn the_ladder_takes_the_exact_tag_then_the_language_then_none() {
        let installed = ["en_US-large", "en_GB", "de_DE"];
        assert_eq!(
            resolve("en_GB", &installed),
            Resolved::Exact("en_GB".into())
        );
        assert_eq!(
            resolve("en_US", &installed),
            Resolved::Fallback {
                tag: "en_US-large".into(),
                wanted: "en_US".into()
            }
        );
        assert_eq!(resolve("de", &installed).tag(), Some("de_DE"));
        let missing = resolve("fr_FR", &installed);
        assert_eq!(missing.tag(), None);
        assert_eq!(missing.wanted(), "fr_FR");
    }

    #[test]
    fn an_empty_setting_resolves_the_locale_s_tag_and_any_other_names_its_own() {
        let installed = ["en_US", "de_DE"];
        let lang = |value: &'static str| move |var: &str| (var == "LANG").then(|| value.to_owned());
        assert_eq!(
            resolve_setting("", &installed, lang("de_DE.UTF-8")),
            Resolved::Exact("de_DE".into())
        );
        assert_eq!(
            resolve_setting("en_US", &installed, lang("de_DE.UTF-8")),
            Resolved::Exact("en_US".into())
        );
        assert_eq!(
            resolve_setting("", &installed, lang("xx_XX.UTF-8")),
            Resolved::Missing {
                wanted: "xx_XX".into()
            }
        );
    }

    #[test]
    fn a_mark_is_carried_only_while_its_word_keeps_its_letters_and_never_widens() {
        let insert = |at: usize, inserted: usize| Splice {
            at: at..at,
            inserted,
        };
        let delete = |at: Range<usize>| Splice { at, inserted: 0 };
        // `wou` marked, then `l` typed against its end: another word.
        assert_eq!(carried(&(0..3), &insert(3, 1), "woul"), None);
        // A space after it stays out of the mark; one before it moves it.
        assert_eq!(carried(&(0..3), &insert(3, 1), "wou "), Some(0..3));
        assert_eq!(carried(&(0..3), &insert(0, 3), "So wou"), Some(3..6));
        // A letter typed before it, inside it, or taken out of it.
        assert_eq!(carried(&(0..3), &insert(0, 1), "swou"), None);
        assert_eq!(carried(&(0..3), &insert(1, 1), "wxou"), None);
        assert_eq!(carried(&(0..3), &delete(2..3), "wo"), None);
        // Deleting the space after it joins the next word to it; at the
        // text's end, or before a full stop, it is the same word.
        assert_eq!(carried(&(0..3), &delete(3..4), "woux"), None);
        assert_eq!(carried(&(0..3), &delete(3..4), "wou"), Some(0..3));
        assert_eq!(carried(&(0..3), &delete(3..4), "wou."), Some(0..3));
        // Edits elsewhere on the line, before and after it.
        assert_eq!(carried(&(4..7), &insert(0, 1), "Xa teh"), Some(5..8));
        assert_eq!(carried(&(0..3), &delete(4..6), "teh b"), Some(0..3));
    }

    #[test]
    fn the_bare_language_outranks_a_regional_tag_of_it() {
        assert_eq!(resolve("en_AU", &["en_GB", "en"]).tag(), Some("en"));
    }

    #[test]
    fn the_locale_tag_is_the_first_set_variable_up_to_its_codeset() {
        let only = |name: &'static str, value: &'static str| {
            move |var: &str| (var == name).then(|| value.to_owned())
        };
        assert_eq!(locale_tag(only("LANG", "en_US.UTF-8")), "en_US");
        assert_eq!(locale_tag(only("LC_MESSAGES", "de_DE@euro")), "de_DE");
        assert_eq!(locale_tag(only("LC_ALL", "C")), "en_US");
        assert_eq!(locale_tag(only("LANG", "POSIX")), "en_US");
        assert_eq!(locale_tag(|_| None), "en_US");
        let all = |var: &str| match var {
            "LC_ALL" => Some(String::new()),
            "LC_MESSAGES" => Some("fr_FR.UTF-8".into()),
            _ => Some("de_DE.UTF-8".into()),
        };
        assert_eq!(locale_tag(all), "fr_FR");
    }
}
