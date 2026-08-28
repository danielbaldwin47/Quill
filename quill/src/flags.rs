//! The command line: everything a launch is, parsed before the first frame.
//!
//! One struct, read once in `main` and handed to everything that builds a
//! window. Two kinds of flag live in it (`docs/architecture.md`, "Command-line
//! flags"). The **judged state** is what the Gate shoots and benches at: the
//! passage, the theme, the Face, the size, Focus, Typewriter, the chrome, the
//! caret, the selection, the scroll and the window's size. The **harness**
//! flags are `--deterministic` and `--measure`, which have no setting behind
//! them because they are the Gate's and not a writer's.
//!
//! Every flag parses from this commit, so an agent writing `tools/gate judge`
//! or `tools/gate bench` can rely on the whole set while the Pieces that give
//! some of them an effect are still to come. `--text`, `--w`, `--h` and
//! `--font` act today; the rest are held here and read by the Piece ticket
//! that lands each one. A flag is never rejected for being early.
//!
//! Three rules the rest of the app rests on. **Nothing is written back**: a
//! flag overrides the writer's settings for this launch and never reaches
//! `settings.toml`. **A harness launch leaves no trace**: [`Flags::any`] is
//! true the moment one of these flags is given, and a session that answers
//! true to it reads no state file and writes none, so two launches of the same
//! command line are the same window twice and a bench never resizes the window
//! a writer left. **The process that was launched serves it**: such a launch
//! runs non-unique, so a judged shot cannot land in a window of the Quill the
//! writer already has open.

use std::ffi::OsString;
use std::fmt;
use std::ops::RangeInclusive;
use std::path::{Path, PathBuf};

use quill_engine::settings::{
    Chrome, Face, FocusScope, Settings, Theme, WindowState, type_sizes, window_sizes,
};

/// What `--help` prints: every flag, in the architecture's order.
pub const USAGE: &str = "\
Usage: quill [flags] [file...]

Judged state — what the Gate shoots and benches, and what a writer may
override for one launch:
  --text <file>          Open <file>.
  --theme light|dark     Set the theme.
  --font duo|quattro|mono
                         Set the Face.
  --size <px>            Set the type size.
  --focus off|sentence|paragraph
                         Turn Focus off, or on at a scope.
  --typewriter           Turn Typewriter on.
  --chrome on|off        Show or hide the bars around the Editor.
  --caret <offset>|end   Put the caret at a byte offset, or at the end.
  --select <from>,<to>   Select from one byte offset to another.
  --scroll <fraction>    Scroll the Document, 0 at the top and 1 at the foot.
  --nocaret              Draw no caret.
  --w <px>               Open the window this wide.
  --h <px>               Open the window this tall.

Harness:
  --deterministic        Animations off, caret blink off, manual font
                         rendering, no client-side decorations.
  --measure <out.jsonl>  Print the cold start from $QUILL_T0_NS at the first
                         presented frame, and create <out.jsonl>.

  --help                 Print this.

A flag overrides the writer's settings for this launch alone and is never
written back to settings.toml.";

/// What `--theme` takes: the two the Gate judges. `auto` is the desktop's
/// answer rather than an answer, so it is a setting and not a judged state.
const THEMES: [(&str, Theme); 2] = [("light", Theme::Light), ("dark", Theme::Dark)];

/// What `--font` takes: the three Faces, under the names the file uses. The
/// test below holds this list to [`Face::VALUES`].
const FONTS: [(&str, Face); 3] = [
    ("duo", Face::Duo),
    ("quattro", Face::Quattro),
    ("mono", Face::Mono),
];

/// What `--chrome` takes. On the command line the chrome is on or off, which
/// is how every other switch here reads; in the file it is shown or hidden,
/// which is how a writer describes a bar.
const CHROMES: [(&str, Chrome); 2] = [("on", Chrome::Shown), ("off", Chrome::Hidden)];

/// What `--focus` takes: one flag for the two settings behind it.
const FOCUSES: [(&str, Focus); 3] = [
    ("off", Focus::Off),
    ("sentence", Focus::Sentence),
    ("paragraph", Focus::Paragraph),
];

/// What `--focus` asks for: Focus off, or on at one of its two scopes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Focus {
    /// Everything at full ink.
    Off,
    /// On, lighting the caret's sentence.
    Sentence,
    /// On, lighting the caret's paragraph.
    Paragraph,
}

/// Where `--caret` puts the caret.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Caret {
    /// At a byte offset from the start of the Document.
    At(u64),
    /// At the end of the Document.
    End,
}

/// Why a command line could not be read: one line, naming the flag.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Error(String);

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Everything one launch of Quill was asked for.
///
/// Every judged-state and harness flag is an `Option` or a `bool`, so that
/// "not given" and "given" stay different things: a flag nobody wrote leaves
/// the writer's setting exactly where it was.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Flags {
    /// The Document `--text` names.
    pub text: Option<PathBuf>,
    /// The theme `--theme` names.
    pub theme: Option<Theme>,
    /// The Face `--font` names.
    pub font: Option<Face>,
    /// The type size `--size` names, in pixels.
    pub size: Option<u32>,
    /// What `--focus` asks of Focus.
    pub focus: Option<Focus>,
    /// Whether `--typewriter` turned Typewriter on.
    pub typewriter: bool,
    /// What `--chrome` asks of the bars around the Editor.
    pub chrome: Option<Chrome>,
    /// Where `--caret` puts the caret.
    pub caret: Option<Caret>,
    /// The byte offsets `--select` selects between.
    pub select: Option<(u64, u64)>,
    /// How far down the Document `--scroll` scrolls, 0 to 1.
    pub scroll: Option<f64>,
    /// Whether `--nocaret` asked for no caret at all.
    pub nocaret: bool,
    /// The window width `--w` names, in pixels.
    pub width: Option<u32>,
    /// The window height `--h` names, in pixels.
    pub height: Option<u32>,
    /// Whether `--deterministic` asked for the Gate's settings.
    pub deterministic: bool,
    /// The file `--measure` writes its capture into.
    pub measure: Option<PathBuf>,
    /// Whether `--help` was asked for. Not one of the harness's flags: it
    /// prints and stops.
    pub help: bool,
    /// The Documents named with no flag in front of them. Not one of the
    /// harness's either: a file on the command line is a writer opening a
    /// Document, and it reaches the Quill they already have running.
    pub files: Vec<PathBuf>,
}

impl Flags {
    /// Reads a command line, without the program's own name.
    ///
    /// # Errors
    ///
    /// One [`Error`], one line long, naming the flag: a flag Quill does not
    /// know, a flag with nothing after it, or a value outside what the flag
    /// takes.
    pub fn parse<A: IntoIterator<Item = OsString>>(args: A) -> Result<Self, Error> {
        let mut flags = Self::default();
        let mut args = args.into_iter();
        while let Some(argument) = args.next() {
            // Owned rather than borrowed, so that an argument that turns out
            // to be a file name can still be moved into `files` below. An
            // argument that is not UTF-8 matches no flag, which is right: a
            // flag is spelled in ASCII and a file name need not be.
            let name = argument.to_string_lossy().into_owned();
            let flag = name.as_str();
            match flag {
                "--text" => flags.text = Some(file(&mut args, flag)?),
                "--theme" => flags.theme = Some(one_of(flag, &text(&mut args, flag)?, &THEMES)?),
                "--font" => flags.font = Some(one_of(flag, &text(&mut args, flag)?, &FONTS)?),
                "--size" => flags.size = Some(whole(flag, &text(&mut args, flag)?, &type_sizes())?),
                "--focus" => flags.focus = Some(one_of(flag, &text(&mut args, flag)?, &FOCUSES)?),
                "--typewriter" => flags.typewriter = true,
                "--chrome" => flags.chrome = Some(one_of(flag, &text(&mut args, flag)?, &CHROMES)?),
                "--caret" => flags.caret = Some(caret(flag, &text(&mut args, flag)?)?),
                "--select" => flags.select = Some(select(flag, &text(&mut args, flag)?)?),
                "--scroll" => flags.scroll = Some(fraction(flag, &text(&mut args, flag)?)?),
                "--nocaret" => flags.nocaret = true,
                "--w" => flags.width = Some(whole(flag, &text(&mut args, flag)?, &window_sizes())?),
                "--h" => {
                    flags.height = Some(whole(flag, &text(&mut args, flag)?, &window_sizes())?)
                }
                "--deterministic" => flags.deterministic = true,
                "--measure" => flags.measure = Some(file(&mut args, flag)?),
                "--help" => flags.help = true,
                _ if flag.starts_with('-') && flag != "-" => {
                    return Err(Error(format!("{flag}: not a flag Quill knows")));
                }
                _ => flags.files.push(PathBuf::from(argument)),
            }
        }
        Ok(flags)
    }

    /// Whether this launch is the harness's rather than a writer's.
    ///
    /// True the moment any judged-state or harness flag was given. Asked
    /// rather than listed, so that a flag added to the struct cannot be
    /// forgotten here: everything but `--help`, which prints and stops, and
    /// the files, which are a writer opening a Document.
    #[must_use]
    pub fn any(&self) -> bool {
        Self {
            help: false,
            files: Vec::new(),
            ..self.clone()
        } != Self::default()
    }

    /// The settings this launch runs on: the writer's, with every flag that
    /// names one over the top. Nothing here reaches `settings.toml`.
    #[must_use]
    pub fn over(&self, mut settings: Settings) -> Settings {
        if let Some(theme) = self.theme {
            settings.theme = theme;
        }
        if let Some(face) = self.font {
            settings.face = face;
        }
        if let Some(size) = self.size {
            settings.size = size;
        }
        // `--focus off` says nothing about the scope, so the writer's is left
        // where it is: turning Focus back on is their scope again.
        match self.focus {
            Some(Focus::Off) => settings.focus = false,
            Some(Focus::Sentence) => {
                settings.focus = true;
                settings.focus_scope = FocusScope::Sentence;
            }
            Some(Focus::Paragraph) => {
                settings.focus = true;
                settings.focus_scope = FocusScope::Paragraph;
            }
            None => {}
        }
        if self.typewriter {
            settings.typewriter = true;
        }
        if let Some(chrome) = self.chrome {
            settings.chrome = chrome;
        }
        settings
    }

    /// The shape the window opens at: the one handed in, at the size `--w` and
    /// `--h` name.
    #[must_use]
    pub fn shape(&self, mut opening: WindowState) -> WindowState {
        if let Some(width) = self.width {
            opening.width = width;
        }
        if let Some(height) = self.height {
            opening.height = height;
        }
        opening
    }

    /// The Documents this launch opens, `--text` first.
    #[must_use]
    pub fn documents(&self) -> Vec<&Path> {
        self.text
            .iter()
            .chain(&self.files)
            .map(PathBuf::as_path)
            .collect()
    }
}

/// The argument after `flag`, whatever it is.
fn value<A: Iterator<Item = OsString>>(args: &mut A, flag: &str) -> Result<OsString, Error> {
    args.next()
        .ok_or_else(|| Error(format!("{flag}: needs a value after it")))
}

/// The argument after `flag`, as a file name.
fn file<A: Iterator<Item = OsString>>(args: &mut A, flag: &str) -> Result<PathBuf, Error> {
    Ok(PathBuf::from(value(args, flag)?))
}

/// The argument after `flag`, as text. A value that is not UTF-8 is not one of
/// the words any flag here takes.
fn text<A: Iterator<Item = OsString>>(args: &mut A, flag: &str) -> Result<String, Error> {
    value(args, flag)?
        .into_string()
        .map_err(|value| not(flag, &value.to_string_lossy(), "text"))
}

/// A value that is not what the flag takes, in the shape a settings note uses:
/// what was written, and what the flag takes instead.
fn not(flag: &str, found: &str, wanted: &str) -> Error {
    Error(format!("{flag}: \"{found}\" is not {wanted}"))
}

/// One of the values a flag takes, by the name it is written under.
fn one_of<T: Copy>(flag: &str, written: &str, values: &[(&str, T)]) -> Result<T, Error> {
    values
        .iter()
        .find(|(name, _)| *name == written)
        .map(|&(_, value)| value)
        .ok_or_else(|| {
            let names: Vec<&str> = values.iter().map(|&(name, _)| name).collect();
            not(flag, written, &format!("one of {}", names.join(", ")))
        })
}

/// A whole number inside `range`.
fn whole(flag: &str, written: &str, range: &RangeInclusive<u32>) -> Result<u32, Error> {
    written
        .parse::<u32>()
        .ok()
        .filter(|number| range.contains(number))
        .ok_or_else(|| {
            let wanted = format!("a whole number from {} to {}", range.start(), range.end());
            not(flag, written, &wanted)
        })
}

/// A number from 0 to 1.
fn fraction(flag: &str, written: &str) -> Result<f64, Error> {
    written
        .parse::<f64>()
        .ok()
        .filter(|number| (0.0..=1.0).contains(number))
        .ok_or_else(|| not(flag, written, "a number from 0 to 1"))
}

/// A byte offset, or the end of the Document.
fn caret(flag: &str, written: &str) -> Result<Caret, Error> {
    if written == "end" {
        return Ok(Caret::End);
    }
    written
        .parse::<u64>()
        .map(Caret::At)
        .map_err(|_| not(flag, written, "a byte offset or end"))
}

/// Two byte offsets with a comma between them.
fn select(flag: &str, written: &str) -> Result<(u64, u64), Error> {
    let wanted = "two byte offsets separated by a comma";
    let (from, to) = written
        .split_once(',')
        .ok_or_else(|| not(flag, written, wanted))?;
    let from = from
        .parse::<u64>()
        .map_err(|_| not(flag, written, wanted))?;
    let to = to.parse::<u64>().map_err(|_| not(flag, written, wanted))?;
    Ok((from, to))
}

#[cfg(test)]
mod tests {
    use quill_engine::settings::Choice;

    use super::*;

    /// Every flag `docs/architecture.md` names, with a value it takes and —
    /// where it has a domain — one it does not.
    const FLAGS: [(&str, &str, Option<&str>); 15] = [
        ("--text", "ref/sample.md", None),
        ("--theme", "dark", Some("purple")),
        ("--font", "mono", Some("comic")),
        ("--size", "24", Some("0")),
        ("--focus", "paragraph", Some("all")),
        ("--typewriter", "", None),
        // The file says shown and hidden; the flag says on and off.
        ("--chrome", "off", Some("shown")),
        ("--caret", "end", Some("middle")),
        ("--select", "10,20", Some("10")),
        ("--scroll", "0.25", Some("2")),
        ("--nocaret", "", None),
        ("--w", "1440", Some("0")),
        ("--h", "900", Some("tall")),
        ("--deterministic", "", None),
        ("--measure", "out.jsonl", None),
    ];

    /// A command line, written as it would be typed.
    fn parse(line: &str) -> Result<Flags, Error> {
        Flags::parse(line.split_whitespace().map(OsString::from))
    }

    #[test]
    fn every_flag_takes_its_value_and_refuses_what_is_not_one() {
        for (flag, good, bad) in FLAGS {
            let line = format!("{flag} {good}");
            let flags = parse(&line).unwrap_or_else(|err| panic!("`{line}`: {err}"));
            assert!(flags.any(), "`{line}` is a launch of the harness's");
            assert!(
                flags.files.is_empty(),
                "`{line}` left an argument over: {flags:?}"
            );

            let Some(bad) = bad else { continue };
            let line = format!("{flag} {bad}");
            let err = parse(&line).expect_err(&format!("`{line}` is not a value {flag} takes"));
            let said = err.to_string();
            assert!(said.starts_with(&format!("{flag}: ")), "{said}");
            assert!(
                said.contains(bad),
                "the line quotes what was written: {said}"
            );
            assert!(!said.contains('\n'), "one line, not a stack: {said}");
        }
    }

    #[test]
    fn the_whole_judged_state_and_the_harness_parse_together() {
        let flags = parse(
            "--text ref/sample.md --theme dark --font mono --size 24 --focus paragraph \
             --typewriter --chrome off --caret end --select 10,20 --scroll 0.25 --nocaret \
             --w 1440 --h 900 --deterministic --measure out.jsonl",
        )
        .expect("every flag at once");
        assert_eq!(flags.text.as_deref(), Some(Path::new("ref/sample.md")));
        assert_eq!(flags.theme, Some(Theme::Dark));
        assert_eq!(flags.font, Some(Face::Mono));
        assert_eq!(flags.size, Some(24));
        assert_eq!(flags.focus, Some(Focus::Paragraph));
        assert!(flags.typewriter);
        assert_eq!(flags.chrome, Some(Chrome::Hidden));
        assert_eq!(flags.caret, Some(Caret::End));
        assert_eq!(flags.select, Some((10, 20)));
        assert_eq!(flags.scroll, Some(0.25));
        assert!(flags.nocaret);
        assert_eq!((flags.width, flags.height), (Some(1440), Some(900)));
        assert!(flags.deterministic);
        assert_eq!(flags.measure.as_deref(), Some(Path::new("out.jsonl")));
        assert!(!flags.help);
    }

    #[test]
    fn a_flag_quill_does_not_know_is_one_line_naming_it() {
        let err = parse("--frobnicate").expect_err("there is no such flag");
        assert_eq!(err.to_string(), "--frobnicate: not a flag Quill knows");
    }

    #[test]
    fn a_flag_with_nothing_after_it_says_what_is_missing() {
        for flag in ["--text", "--theme", "--size", "--measure"] {
            let err = parse(flag).expect_err("nothing follows it");
            assert_eq!(err.to_string(), format!("{flag}: needs a value after it"));
        }
    }

    #[test]
    fn the_caret_goes_to_an_offset_or_to_the_end() {
        assert_eq!(parse("--caret 0").unwrap().caret, Some(Caret::At(0)));
        assert_eq!(parse("--caret 1234").unwrap().caret, Some(Caret::At(1234)));
        assert_eq!(parse("--caret end").unwrap().caret, Some(Caret::End));
        assert_eq!(
            parse("--caret -1").unwrap_err().to_string(),
            "--caret: \"-1\" is not a byte offset or end"
        );
    }

    #[test]
    fn a_selection_is_two_offsets_with_a_comma_between_them() {
        assert_eq!(parse("--select 0,0").unwrap().select, Some((0, 0)));
        for bad in ["10", "10,", "10 20", "a,b"] {
            parse(&format!("--select {bad}")).expect_err("not a selection");
        }
    }

    #[test]
    fn an_empty_command_line_carries_nothing() {
        let flags = parse("").expect("an empty command line");
        assert_eq!(flags, Flags::default());
        assert!(!flags.any(), "nothing was asked for");
        assert!(flags.documents().is_empty());
    }

    #[test]
    fn a_file_on_the_command_line_is_a_writers_launch_and_not_the_harnesss() {
        let flags = parse("one.md two.md").expect("two Documents");
        assert_eq!(
            flags.documents(),
            [Path::new("one.md"), Path::new("two.md")]
        );
        assert!(
            !flags.any(),
            "opening a Document reaches the Quill already running"
        );
    }

    #[test]
    fn the_text_flag_is_the_first_document() {
        let flags = parse("--text ref/sample.md other.md").expect("a flag and a file");
        assert_eq!(
            flags.documents(),
            [Path::new("ref/sample.md"), Path::new("other.md")]
        );
        assert!(flags.any());
    }

    #[test]
    fn help_prints_and_stops_rather_than_launching_the_harness() {
        let flags = parse("--help").expect("--help is a flag");
        assert!(flags.help);
        assert!(!flags.any(), "there is nothing to serve");
    }

    #[test]
    fn the_usage_names_every_flag() {
        for (flag, _, _) in FLAGS {
            assert!(
                USAGE.contains(&format!("{flag} ")),
                "no {flag} in the usage"
            );
        }
        assert!(USAGE.contains("--help "));
    }

    #[test]
    fn the_font_values_are_the_faces_own_names() {
        let names: Vec<&str> = FONTS.iter().map(|&(name, _)| name).collect();
        assert_eq!(names, Face::VALUES, "the flag and the file name the Faces");
    }

    #[test]
    fn a_flag_overrides_the_setting_it_matches_and_leaves_the_rest_alone() {
        let writers = Settings::default();
        let flags =
            parse("--theme dark --font mono --size 24 --focus paragraph --typewriter --chrome off")
                .expect("six settings overridden");
        let launched = flags.over(writers.clone());
        assert_eq!(launched.theme, Theme::Dark);
        assert_eq!(launched.face, Face::Mono);
        assert_eq!(launched.size, 24);
        assert!(launched.focus);
        assert_eq!(launched.focus_scope, FocusScope::Paragraph);
        assert!(launched.typewriter);
        assert_eq!(launched.chrome, Chrome::Hidden);
        assert_eq!(launched.template, writers.template, "nothing else moved");
        assert_eq!(
            Flags::default().over(writers.clone()),
            writers,
            "a launch with no flags is the writer's settings exactly"
        );
    }

    #[test]
    fn focus_off_turns_focus_off_and_leaves_the_scope_the_writer_chose() {
        // Built by hand rather than with `..Settings::default()`: the engine
        // keeps a private field for the keys it did not recognise.
        let mut writers = Settings::default();
        writers.focus = true;
        writers.focus_scope = FocusScope::Paragraph;
        let launched = parse("--focus off").unwrap().over(writers);
        assert!(!launched.focus);
        assert_eq!(launched.focus_scope, FocusScope::Paragraph);
    }

    #[test]
    fn the_window_opens_at_the_size_the_flags_name() {
        let shape = parse("--w 1440 --h 900")
            .unwrap()
            .shape(WindowState::default());
        assert_eq!((shape.width, shape.height), (1440, 900));
        assert_eq!(
            Flags::default().shape(WindowState::default()),
            WindowState::default(),
            "a launch with no size is the shape it was handed"
        );
    }
}
