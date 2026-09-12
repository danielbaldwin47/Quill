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
//! `settings.toml`. `--library` overrides more than the one setting it names:
//! it pins the whole `[library]` table to its defaults — the fixture as the
//! one Location, nothing Pinned, dot-entries and extensions off, no
//! confirmation and no ask-where — so that a judged shot of the Library is the
//! fixture's rows and never the rows the writer's own switches would draw. **A harness launch leaves no trace**: [`Flags::is_harness`]
//! is true the moment one of these flags is given, and a session that answers
//! true to it reads no state file and writes none, so two launches of the same
//! command line are the same window twice and a bench never resizes the window
//! a writer left. **The process that was launched serves it**: such a launch
//! runs non-unique, so a judged shot cannot land in a window of the Quill the
//! writer already has open.
//!
//! `--help` is here for one reason: reading the command line is what took it
//! away. `GApplication` answered `--help` while every argument still went to
//! it, and a Quill that now called `--help` a flag it did not know would have
//! lost something a writer had.

use std::ffi::OsString;
use std::fmt;
use std::ops::RangeInclusive;
use std::path::{Path, PathBuf};

use quill_engine::commands;
use quill_engine::settings::{
    Choice, Chrome, Export, Face, FocusScope, Paper, Preview, PreviewLayout, PreviewMode, Settings,
    Stats, StatsBar, StyleCheck, SyntaxHighlight, Template as TemplateSettings, TemplateName,
    WindowState, preview_zooms, window_sizes,
};
use quill_engine::stats::Statistic;
use quill_engine::theme::Scheme;
use quill_engine::typography;

use crate::export_dialog::Format;

/// What `--help` prints: every flag, in the architecture's order.
pub const USAGE: &str = "\
Usage: quill [flags] [file...]

Judged state — the states the Gate shoots and benches at:
  --text <file>          Open <file>.
  --theme light|dark     Set the theme.
  --font duo|quattro|mono
                         Set the Face.
  --step <n>             Set the type to step <n> of the ladder, 0 to 13.
  --focus off|sentence|paragraph
                         Turn Focus off, or on at a scope.
  --typewriter           Turn Typewriter on.
  --live                 Turn Live on: the markup rendered in place.
  --syntax off|on|nouns,verbs,adjectives,adverbs,conjunctions
                         Pin Syntax highlight off, all on, or to named Categories.
  --style off|on|fillers,redundancies,cliches
                         Pin Style check off, all on, or to named Lists.
  --stats <names>|hidden Pin the stats bar to a comma list of Statistic names,
                         or hide it: words, characters, charactersNoSpaces,
                         sentences, paragraphs, readingTime.
  --chrome on|off        Show or hide the bars around the Editor.
  --caret <offset>|end   Put the caret at a byte offset, or at the end.
  --select <from>,<to>   Select from one byte offset to another.
  --scroll <fraction>    Scroll the Document, 0 at the top and 1 at the foot.
  --nocaret              Draw no caret.
  --typing               Open inside the 500 ms after a keystroke: the title
                         bar gone and the stats bar dimmed.
  --menu view|document|stats|palette
                         Open with that menu or the Palette up, its first row
                         selected.
  --library <dir>        Take the Library from the fixture tree at <dir>: a
                         copy of it, each file stamped with the mtime its
                         manifest.json names, as the one Location, with the
                         rest of [library] at its defaults.
  --sidebar              Open with the Library beside the page.
  --preview split|full|pdf-split|pdf-full
                         Open with the Preview pane beside the Editor, or in
                         place of it, showing the rendered sheet or the pages
                         Export writes, with the rest of [preview] at its
                         defaults.
  --zoom <percent>       Draw the Preview pane at that per cent of fit width,
                         which is [preview] zoom. Wants --preview.
  --template <id>        Lay the rendered page out in that Template, with the
                         rest of [template] at its defaults.
  --search <query>       Put <query> in the Library's search field and narrow
                         the list to what it finds. Wants --sidebar.
  --export-dialog pdf|html|markdown
                         Open that format's Export dialog over the page, its
                         Options expander open, once the window has painted.
  --w <px>               Open the window this wide.
  --h <px>               Open the window this tall.

Harness:
  --deterministic        Animations off, caret blink off, manual font
                         rendering, no client-side decorations.
  --measure <out.jsonl>  Print the cold start from $QUILL_T0_NS at the first
                         presented frame, and create <out.jsonl>.
  --settings <file>      Read and write settings in <file> rather than in the
                         writer's own settings.toml.
  --palette <file>       Paint the grounds from the palette in <file> for this
                         launch, whatever the palette setting names.

  --help                 Print this.

A launch carrying any flag above is the harness's rather than a writer's: it
opens in a process of its own rather than reaching a Quill already running, it
overrides the writer's settings for that launch alone, and it writes neither
settings.toml nor state.toml — except that a launch given --settings owns the
file it names, and reads and writes that one. A launch given --theme and no
--palette paints the built-in grounds, whatever the palette setting names, so a
judged shot is the same on every machine.";

/// What `--theme` takes: the two the Gate judges. `auto` is the desktop's
/// answer rather than an answer, so it is a setting and not a judged state —
/// which is why the flag names a ground and not the three-valued setting.
const THEMES: [(&str, Scheme); 2] = [("light", Scheme::Light), ("dark", Scheme::Dark)];

/// What `--chrome` takes. On the command line the chrome is on or off, which
/// is how every other switch here reads; in the file it is shown or hidden,
/// which is how a writer describes a bar.
const CHROMES: [(&str, Chrome); 2] = [("on", Chrome::Shown), ("off", Chrome::Hidden)];

/// What `--menu` takes: the three menus and the Palette, by the names
/// `shots/oracle/states.json` uses for them.
const MENUS: [(&str, Menu); 4] = [
    ("view", Menu::Bar(commands::Menu::View)),
    ("document", Menu::Bar(commands::Menu::Document)),
    ("stats", Menu::Bar(commands::Menu::Stats)),
    ("palette", Menu::Palette),
];

/// What `--preview` takes: where the pane opens and what it draws there, in
/// one word.
///
/// The pane has two settings under it, `[preview] layout` and `[preview]
/// mode`, and one flag names both because a shot of the pane is a shot of the
/// pair — `pdf-split` is the page column beside the Editor, `split` the
/// rendered sheet in the same place. It is a table here rather than
/// [`choice`] over a settings [`Choice`] because neither enum's vocabulary is
/// these four words: `PreviewLayout::VALUES` is what the settings file writes
/// under `layout`, and widening it to carry the mode would change the file.
const PREVIEWS: [(&str, (PreviewLayout, PreviewMode)); 4] = [
    ("split", (PreviewLayout::Split, PreviewMode::Web)),
    ("full", (PreviewLayout::Full, PreviewMode::Web)),
    ("pdf-split", (PreviewLayout::Split, PreviewMode::Pdf)),
    ("pdf-full", (PreviewLayout::Full, PreviewMode::Pdf)),
];

/// What `--menu` opens before the first frame, its first row selected: one
/// of the bars' three menus, or the Palette.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Menu {
    /// A menu under a bar button: the View menu (`F10`), the Document menu
    /// under the title, or the Stats menu above the stats bar.
    Bar(commands::Menu),
    /// The Palette, which `Ctrl+K` opens.
    Palette,
}

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
    /// The ground `--theme` names.
    ///
    /// A [`Scheme`] rather than a [`Theme`], because the flag pins what is on
    /// screen: it is read straight into `quill_engine::theme::effective` as
    /// the flag that beats both the setting and the desktop, and `auto` is not
    /// something it can say.
    pub theme: Option<Scheme>,
    /// The Face `--font` names. A Face and not a font: the flag is spelled
    /// the way `docs/architecture.md` spells it, and the word stops there
    /// (`CONTEXT.md`).
    pub face: Option<Face>,
    /// The step of the type ladder `--step` names.
    pub step: Option<u32>,
    /// What `--focus` asks of Focus.
    pub focus: Option<Focus>,
    /// Whether `--typewriter` turned Typewriter on.
    pub typewriter: bool,
    /// Whether `--live` turned Live on.
    pub live: bool,
    /// The whole Syntax highlight table pinned by `--syntax`.
    pub syntax: Option<SyntaxHighlight>,
    /// The whole Style check table pinned by `--style`.
    pub style: Option<StyleCheck>,
    /// The whole Stats table pinned by `--stats`.
    pub stats: Option<Stats>,
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
    /// Whether `--typing` asked for the chrome as it is inside the 500 ms
    /// after a keystroke: the title bar gone, the stats bar dimmed, held
    /// there ([`crate::chrome::typing::Typing::from_flags`]).
    pub typing: bool,
    /// What `--menu` asked to have open before the first frame.
    pub menu: Option<Menu>,
    /// The fixture tree `--library` names, which stands in for the writer's
    /// Locations for this launch.
    ///
    /// The folder as it was named on the command line until the launch has
    /// copied it ([`crate::files::stage`]); the copy from then on, because a
    /// judged shot walks a tree whose mtimes it stamped and never the one in
    /// the checkout.
    pub library: Option<PathBuf>,
    /// Whether `--sidebar` asked for the Library beside the page.
    pub sidebar: bool,
    /// The query `--search` puts in the Library's search field.
    pub search: Option<String>,
    /// Where `--preview` asked the pane to open and what it asked it to draw
    /// there: the rendered sheet, or the pages Export writes.
    ///
    /// The pair the flag's word names and not two flags' worth of answer
    /// ([`PREVIEWS`]), because `pdf-split` is one word and means both. The
    /// pane's being open at all is this flag having been given: nothing else
    /// opens one, because the pane is closed at every launch (#263).
    pub preview: Option<(PreviewLayout, PreviewMode)>,
    /// What `--zoom` pinned `[preview] zoom` to, per cent of fit width.
    ///
    /// A state that wants two pages of the column in one window says so with
    /// this ([`Flags::over`] pins the rest of the table around it); a writer's
    /// launch never names it.
    pub zoom: Option<u32>,
    /// The Template `--template` names, which the rendered page is laid out
    /// in for this launch.
    pub template: Option<TemplateName>,
    /// The Export dialog `--export-dialog` asks to be opened over the page,
    /// its Options expander open, once the window has painted its first frame
    /// ([`crate::window::Window::open_flagged_export`]). The dialog's being
    /// open at all is this flag having been given: nothing else opens one
    /// unasked.
    pub export_dialog: Option<Format>,
    /// The window width `--w` names, in pixels.
    pub width: Option<u32>,
    /// The window height `--h` names, in pixels.
    pub height: Option<u32>,
    /// Whether `--deterministic` asked for the Gate's settings: the
    /// rendering [`crate::harness`] pins, and Typewriter off unless
    /// `--typewriter` is given, so the writer's file reaches no judged shot
    /// ([`Flags::over`]).
    pub deterministic: bool,
    /// The file `--measure` writes its capture into.
    pub measure: Option<PathBuf>,
    /// The settings file `--settings` names, which this launch reads and
    /// writes instead of the writer's own. One of the harness's flags like
    /// the rest, so a launch carrying it is still its own process and still
    /// leaves `state.toml` alone; what it moves is where `settings.toml` is
    /// ([`crate::session::Session::settings_path`]).
    pub settings: Option<PathBuf>,
    /// The palette file `--palette` names, laid over the built-in grounds
    /// for this launch in place of whatever the `palette` setting names
    /// ([`Flags::over`]): a palette previewed without editing the writer's
    /// file, and the way a `--deterministic` shot shows one applied.
    pub palette: Option<PathBuf>,
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
    /// know, a flag with nothing after it, a value outside what the flag
    /// takes, or `--search` with no `--sidebar` beside it.
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
                "--font" => flags.face = Some(choice(flag, &text(&mut args, flag)?)?),
                "--step" => {
                    flags.step = Some(whole(flag, &text(&mut args, flag)?, &typography::steps())?);
                }
                // The type is a ladder of fourteen steps rather than a range
                // of pixels, so a size in pixels no longer names a state the
                // Gate can shoot. Refused by name rather than left to the arm
                // below, which would say only that Quill does not know it.
                "--size" => {
                    return Err(Error(format!(
                        "{flag}: the type is a ladder now, so use --step <n>, 0 to 13"
                    )));
                }
                "--focus" => flags.focus = Some(one_of(flag, &text(&mut args, flag)?, &FOCUSES)?),
                "--typewriter" => flags.typewriter = true,
                "--live" => flags.live = true,
                "--syntax" => flags.syntax = Some(syntax(flag, &text(&mut args, flag)?)?),
                "--style" => flags.style = Some(style(flag, &text(&mut args, flag)?)?),
                "--stats" => flags.stats = Some(stats_table(flag, &text(&mut args, flag)?)?),
                "--chrome" => flags.chrome = Some(one_of(flag, &text(&mut args, flag)?, &CHROMES)?),
                "--caret" => flags.caret = Some(caret(flag, &text(&mut args, flag)?)?),
                "--select" => flags.select = Some(select(flag, &text(&mut args, flag)?)?),
                "--scroll" => flags.scroll = Some(fraction(flag, &text(&mut args, flag)?)?),
                "--nocaret" => flags.nocaret = true,
                "--typing" => flags.typing = true,
                "--menu" => flags.menu = Some(one_of(flag, &text(&mut args, flag)?, &MENUS)?),
                "--library" => flags.library = Some(file(&mut args, flag)?),
                "--sidebar" => flags.sidebar = true,
                "--search" => flags.search = Some(text(&mut args, flag)?),
                "--preview" => {
                    flags.preview = Some(one_of(flag, &text(&mut args, flag)?, &PREVIEWS)?);
                }
                "--zoom" => {
                    flags.zoom = Some(whole(flag, &text(&mut args, flag)?, &preview_zooms())?);
                }
                "--template" => flags.template = Some(choice(flag, &text(&mut args, flag)?)?),
                "--export-dialog" => {
                    let written = text(&mut args, flag)?;
                    flags.export_dialog = Some(Format::parse(&written).ok_or_else(|| {
                        not(
                            flag,
                            &written,
                            &format!("one of {}", Format::VALUES.join(", ")),
                        )
                    })?);
                }
                "--w" => flags.width = Some(whole(flag, &text(&mut args, flag)?, &window_sizes())?),
                "--h" => {
                    flags.height = Some(whole(flag, &text(&mut args, flag)?, &window_sizes())?)
                }
                "--deterministic" => flags.deterministic = true,
                "--measure" => flags.measure = Some(file(&mut args, flag)?),
                "--settings" => flags.settings = Some(file(&mut args, flag)?),
                "--palette" => flags.palette = Some(file(&mut args, flag)?),
                "--help" => flags.help = true,
                _ if flag.starts_with('-') && flag != "-" => {
                    return Err(Error(format!("{flag}: not a flag Quill knows")));
                }
                _ => flags.files.push(PathBuf::from(argument)),
            }
        }
        // A query with no pane to type it into would shoot a window with the
        // Library away and the search unrun, and call it a search: refused
        // here for the reason a fixture that is not there is
        // ([`crate::files::stage`]).
        if flags.search.is_some() && !flags.sidebar {
            return Err(Error(
                "--search: --sidebar too: there is no field to type a query into".to_string(),
            ));
        }
        Ok(flags)
    }

    /// Whether this launch is the harness's rather than a writer's.
    ///
    /// True the moment any judged-state or harness flag was given, which is
    /// what makes the launch its own process, leaves the state file unread and
    /// unwritten, and keeps a judged shot out of a writer's window.
    ///
    /// Asked of the whole struct rather than of a list of fields, so that a
    /// flag added above cannot be forgotten here. That is safe in one
    /// direction only: every flag in `docs/architecture.md` is the harness's,
    /// and anything added that is *not* — as `--help` and the files are not —
    /// belongs in the two exceptions below or every launch becomes the
    /// harness's.
    #[must_use]
    pub fn is_harness(&self) -> bool {
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
        if let Some(scheme) = self.theme {
            settings.theme = scheme.setting();
        }
        if let Some(face) = self.face {
            settings.face = face;
        }
        if let Some(step) = self.step {
            settings.step = step;
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
        // A judged state names every mode it is shot in, and Typewriter's
        // flag has no `off`: a `--deterministic` launch without it is shot
        // with Typewriter off, whatever the file says — the writer's own,
        // where `--settings` did not point the launch at another.
        if self.typewriter {
            settings.typewriter = true;
        } else if self.deterministic {
            settings.typewriter = false;
        }
        // Live is pinned the same way and for the same reason: every judged
        // state but `live/folded` is shot with the markers on the page, and a
        // writer who turned Live on in their own file would otherwise be
        // shooting a folded page at every one of them.
        if self.live {
            settings.live = true;
        } else if self.deterministic {
            settings.live = false;
        }
        // Pin the whole table, so the writer's Category choices reach no shot.
        if let Some(syntax) = &self.syntax {
            settings.syntax_highlight = syntax.clone();
        } else if self.deterministic {
            settings.syntax_highlight = SyntaxHighlight::default();
        }
        // Style check is pinned whole for the same reason: a writer who left
        // Clichés off would otherwise shoot a page with two Lists struck on
        // every state that names `on`.
        if let Some(style) = &self.style {
            settings.style_check = style.clone();
        } else if self.deterministic {
            settings.style_check = StyleCheck::default();
        }
        // Stats is pinned whole for the third time over: a writer who had
        // checked Sentences, or hidden the bar altogether, would otherwise
        // shoot a different row of cells at every state that carries the
        // chrome. There is no `off` to pin it with, so a `--deterministic`
        // launch that names no `--stats` reads the table's own defaults —
        // the three cells every judged state was frozen at (#387).
        if let Some(stats) = &self.stats {
            settings.stats = stats.clone();
        } else if self.deterministic {
            settings.stats = Stats::default();
        }
        if let Some(chrome) = self.chrome {
            settings.chrome = chrome;
        }
        // The palette rides on the ground: `--palette` names the file for this
        // launch, and `--theme` on its own pins the built-in table for the
        // ground it names, whatever the writer's file says (`docs/design.md`
        // § The palette is a file), so that a judged shot is the same on
        // every machine. The two together are a palette previewed on a pinned
        // ground.
        if let Some(palette) = &self.palette {
            settings.palette = Some(palette.clone());
        } else if self.theme.is_some() {
            settings.palette = None;
        }
        // The fixture stands in for the writer's Locations, and for nothing
        // else of theirs: a judged shot walks the tree the flag named and no
        // folder of the machine it is run on, and pins nothing, because the
        // Pinned section is a state the fixture would have to carry. The four
        // `[library]` booleans are pinned off with them, because each of them
        // is rows in the shot — the dot-folder the fixture holds, every name's
        // extension — and a writer who turned one on in their own settings
        // would otherwise be shooting a different Library than round 6 judged.
        // The sort is Date already: the pane opens at it and no setting
        // carries it (`crate::sidebar::Sidebar`).
        if let Some(library) = &self.library {
            settings.library.locations = vec![library.clone()];
            settings.library.pinned = Vec::new();
            settings.library.show_hidden = false;
            settings.library.show_extensions = false;
            settings.library.confirm_move = false;
            settings.library.ask_where_to_save = false;
        }
        // A judged shot of the pane names where it opens and nothing else
        // about it, so the rest of the table is pinned to its defaults with
        // it: a writer who reads at 140 % is not shooting a different Piece
        // than the one that was judged. The pane's being open is the window's
        // ([`crate::window::Window::new`]), and absent under `--deterministic`
        // there is no pane to pin.
        // The mode is set beside the layout rather than left to the reset,
        // because the word the flag took names both: `--preview pdf-split` is
        // a shot of the page column and `--preview split` a shot of the sheet,
        // and a writer whose file remembers `mode = "pdf"` shoots neither by
        // accident.
        if let Some((layout, mode)) = self.preview {
            settings.preview = Preview::default();
            settings.preview.layout = layout;
            settings.preview.mode = mode;
        } else if self.deterministic {
            settings.preview = Preview::default();
        }
        // The zoom after the reset above, because it is the one key of the
        // table a state may name for itself: `preview/pdf-full` stands two
        // pages in one window with it, and every other state takes the
        // default fit width the reset just put back (#293).
        if let Some(zoom) = self.zoom {
            settings.preview.zoom = zoom;
        }
        // The Template the same way, and the three toggles with it: each of
        // them is the shape of every heading and every paragraph in the shot.
        if let Some(name) = self.template {
            settings.template = TemplateSettings::default();
            settings.template.name = name;
        } else if self.deterministic {
            settings.template = TemplateSettings::default();
        }
        // `[export]` has no flag of its own: the geometry is not a state the
        // Gate shoots, it is the rows the Export dialog opens on, so the whole
        // table is pinned to its defaults under every judged shot and there is
        // no half of this that reads a flag. A writer whose file asks for a
        // title page at 14 pt on Legal would otherwise be shooting different
        // rows than the ones that were judged.
        //
        // `paper` is pinned past the default, because the default is not one
        // paper: `auto` is resolved through the host's locale at the moment it
        // is asked for (`quill_engine::settings::locale_paper`), so the row the
        // dialog draws would read one thing under `LANG=en_US` and another
        // under `LANG=de_DE`. A4 names the same paper on every machine, which
        // is what a judged shot needs of it.
        if self.deterministic {
            settings.export = Export::default();
            settings.export.paper = Paper::A4;
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

/// One of the values a setting takes, under the name the file writes it as.
///
/// The three flags below this one have domains of their own — `--theme` leaves
/// out `auto`, `--chrome` says on and off where the file says shown and hidden,
/// `--focus` is one flag over two settings — but a Face is a Face, and the
/// engine already knows how to read one.
fn choice<C: Choice>(flag: &str, written: &str) -> Result<C, Error> {
    C::parse(written).ok_or_else(|| not(flag, written, &format!("one of {}", C::VALUES.join(", "))))
}

/// Pins every Category as well as the master switch, including for `off`.
fn syntax(flag: &str, written: &str) -> Result<SyntaxHighlight, Error> {
    let mut syntax = SyntaxHighlight::default();
    if written == "off" {
        return Ok(syntax);
    }
    syntax.enabled = true;
    if written == "on" {
        return Ok(syntax);
    }
    syntax.nouns = false;
    syntax.verbs = false;
    syntax.adjectives = false;
    syntax.adverbs = false;
    syntax.conjunctions = false;
    for category in written.split(',') {
        match category {
            "nouns" => syntax.nouns = true,
            "verbs" => syntax.verbs = true,
            "adjectives" => syntax.adjectives = true,
            "adverbs" => syntax.adverbs = true,
            "conjunctions" => syntax.conjunctions = true,
            _ => {
                return Err(not(
                    flag,
                    written,
                    "off, on, or comma-separated nouns, verbs, adjectives, adverbs, conjunctions",
                ));
            }
        }
    }
    Ok(syntax)
}

/// Pins every List as well as the master switch, including for `off`.
fn style(flag: &str, written: &str) -> Result<StyleCheck, Error> {
    let mut style = StyleCheck::default();
    if written == "off" {
        return Ok(style);
    }
    style.enabled = true;
    if written == "on" {
        return Ok(style);
    }
    style.fillers = false;
    style.redundancies = false;
    style.cliches = false;
    for list in written.split(',') {
        match list {
            "fillers" => style.fillers = true,
            "redundancies" => style.redundancies = true,
            "cliches" => style.cliches = true,
            _ => {
                return Err(not(
                    flag,
                    written,
                    "off, on, or comma-separated fillers, redundancies, cliches",
                ));
            }
        }
    }
    Ok(style)
}

/// Pins which Statistics the bar shows, or hides the bar.
///
/// No `off`: it is the master switch everywhere else on this command line, and
/// there is no master switch here — hiding the bar and checking nothing are
/// two different states, and `off` would have to be one of them and read as
/// the other. `hidden` says the one and the empty list says the other, so
/// neither spelling is ever a claim about both.
///
/// A name repeated is one cell, as the table's reader has it.
///
/// Named for the table where `syntax` and `style` beside it are named for
/// their flag: `Stats` is a type this module imports, and a private fn does
/// not share a name with one (`CODING_STANDARDS.md` § Shape).
fn stats_table(flag: &str, written: &str) -> Result<Stats, Error> {
    let mut stats = Stats::default();
    if written == "hidden" {
        stats.bar = StatsBar::Hidden;
        return Ok(stats);
    }
    stats.show = Vec::new();
    // The empty list is the bar shown with no cells, which is what unchecking
    // every Statistic leaves; splitting it would look for a Statistic named "".
    if written.is_empty() {
        return Ok(stats);
    }
    for name in written.split(',') {
        let Some(statistic) = Statistic::from_name(name) else {
            let names: Vec<&str> = Statistic::ALL.iter().map(|s| s.name()).collect();
            return Err(not(
                flag,
                written,
                &format!("hidden, or comma-separated {}", names.join(", ")),
            ));
        };
        if !stats.show.contains(&statistic) {
            stats.show.push(statistic);
        }
    }
    Ok(stats)
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
    use super::*;

    /// Every flag `docs/architecture.md` names, with a value it takes and —
    /// where it has a domain — one it does not.
    const FLAGS: [(&str, &str, Option<&str>); 29] = [
        ("--text", "ref/sample.md", None),
        ("--theme", "dark", Some("purple")),
        ("--font", "mono", Some("comic")),
        ("--step", "6", Some("14")),
        ("--focus", "paragraph", Some("all")),
        ("--typewriter", "", None),
        ("--live", "", None),
        ("--syntax", "nouns,adverbs", Some("pronouns")),
        ("--style", "fillers,cliches", Some("jargon")),
        ("--stats", "words,sentences", Some("syllables")),
        // The file says shown and hidden; the flag says on and off.
        ("--chrome", "off", Some("shown")),
        ("--caret", "end", Some("middle")),
        ("--select", "10,20", Some("10")),
        ("--scroll", "0.25", Some("2")),
        ("--nocaret", "", None),
        ("--typing", "", None),
        ("--menu", "view", Some("file")),
        ("--library", "shots/oracle/library", None),
        ("--sidebar", "", None),
        // With the flag it wants beside it: a query and no pane to type it
        // into is refused ([`Flags::parse`]).
        ("--search", "sea --sidebar", None),
        ("--preview", "split", Some("beside")),
        ("--template", "classic", Some("gothic")),
        ("--export-dialog", "pdf", Some("docx")),
        ("--w", "1440", Some("0")),
        ("--h", "900", Some("tall")),
        ("--deterministic", "", None),
        ("--measure", "out.jsonl", None),
        ("--settings", "settings.toml", None),
        ("--palette", "quill.toml", None),
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
            assert!(flags.is_harness(), "`{line}` is a launch of the harness's");
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
    fn a_query_with_no_sidebar_to_type_it_into_is_refused() {
        let err = parse("--library shots/oracle/library --search sea")
            .expect_err("a query with the Library away");
        let said = err.to_string();
        assert_eq!(
            said,
            "--search: --sidebar too: there is no field to type a query into"
        );
        assert!(!said.contains('\n'), "one line, not a stack: {said}");
        assert!(parse("--search sea --sidebar").is_ok());
    }

    #[test]
    fn the_whole_judged_state_and_the_harness_parse_together() {
        let flags = parse(
            "--text ref/sample.md --theme dark --font mono --step 6 --focus paragraph \
             --typewriter --live --chrome off --caret end --select 10,20 --scroll 0.25 --nocaret \
             --typing --menu palette --library shots/oracle/library --sidebar --search sea \
             --preview full --template classic --export-dialog pdf \
             --w 1440 --h 900 --deterministic --measure out.jsonl \
             --palette quill.toml",
        )
        .expect("every flag at once");
        assert_eq!(
            flags.library.as_deref(),
            Some(Path::new("shots/oracle/library"))
        );
        assert!(flags.sidebar);
        assert_eq!(flags.search.as_deref(), Some("sea"));
        assert_eq!(flags.preview, Some((PreviewLayout::Full, PreviewMode::Web)));
        assert_eq!(flags.template, Some(TemplateName::Classic));
        assert_eq!(flags.export_dialog, Some(Format::Pdf));
        assert!(flags.typing);
        assert_eq!(flags.menu, Some(Menu::Palette));
        assert_eq!(flags.text.as_deref(), Some(Path::new("ref/sample.md")));
        assert_eq!(flags.theme, Some(Scheme::Dark));
        assert_eq!(flags.face, Some(Face::Mono));
        assert_eq!(flags.step, Some(6));
        assert_eq!(flags.focus, Some(Focus::Paragraph));
        assert!(flags.typewriter);
        assert!(flags.live);
        assert_eq!(flags.chrome, Some(Chrome::Hidden));
        assert_eq!(flags.caret, Some(Caret::End));
        assert_eq!(flags.select, Some((10, 20)));
        assert_eq!(flags.scroll, Some(0.25));
        assert!(flags.nocaret);
        assert_eq!((flags.width, flags.height), (Some(1440), Some(900)));
        assert!(flags.deterministic);
        assert_eq!(flags.measure.as_deref(), Some(Path::new("out.jsonl")));
        assert_eq!(flags.palette.as_deref(), Some(Path::new("quill.toml")));
        assert!(!flags.help);
    }

    #[test]
    fn the_step_the_gates_harness_writes_is_the_ladders_default() {
        // `shots/oracle/states.json` says `"step": 5`, and every judged state
        // is launched with it.
        let flags = parse("--step 5").expect("the harness's own step");
        assert_eq!(flags.step, Some(quill_engine::settings::default_step()));
    }

    #[test]
    fn a_size_in_pixels_is_refused_by_name_and_told_the_flag_that_replaced_it() {
        // The ladder took the pixels away, so a command line still writing
        // them is answered with the flag it wanted rather than with "not a
        // flag Quill knows", which would send a reader looking for a typo.
        let err = parse("--size 20").expect_err("the type is a ladder now");
        assert_eq!(
            err.to_string(),
            "--size: the type is a ladder now, so use --step <n>, 0 to 13"
        );
    }

    #[test]
    fn a_flag_quill_does_not_know_is_one_line_naming_it() {
        let err = parse("--frobnicate").expect_err("there is no such flag");
        assert_eq!(err.to_string(), "--frobnicate: not a flag Quill knows");
    }

    #[test]
    fn a_flag_with_nothing_after_it_says_what_is_missing() {
        for flag in [
            "--text",
            "--theme",
            "--step",
            "--measure",
            "--syntax",
            "--style",
            "--stats",
        ] {
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
        assert!(!flags.is_harness(), "nothing was asked for");
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
            !flags.is_harness(),
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
        assert!(flags.is_harness());
    }

    #[test]
    fn help_prints_and_stops_rather_than_launching_the_harness() {
        let flags = parse("--help").expect("--help is a flag");
        assert!(flags.help);
        assert!(!flags.is_harness(), "there is nothing to serve");
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
    fn the_font_flag_takes_every_face_the_file_takes() {
        for value in Face::VALUES {
            let flags = parse(&format!("--font {value}")).expect("a Face the file names");
            assert_eq!(flags.face.map(Face::as_str), Some(*value));
        }
    }

    /// A judged state is shot with Typewriter off unless it says
    /// `--typewriter`, whatever the writer's file holds; a live launch
    /// without the flag keeps the file's.
    #[test]
    fn a_deterministic_launch_without_typewriter_runs_with_it_off() {
        let mut writers = Settings::default();
        writers.typewriter = true;
        let judged = parse("--deterministic")
            .expect("one flag")
            .over(writers.clone());
        assert!(!judged.typewriter);
        let live = parse("--theme dark")
            .expect("one flag")
            .over(writers.clone());
        assert!(live.typewriter);
        let asked = parse("--deterministic --typewriter")
            .expect("two flags")
            .over(writers);
        assert!(asked.typewriter);
    }

    /// The same rule for Live, and the reason is the same: `live/folded` is
    /// the one judged state shot with the markers folded away, and every other
    /// one is shot with them on the page whatever a writer's own file says.
    #[test]
    fn a_deterministic_launch_without_live_runs_with_it_off() {
        let mut writers = Settings::default();
        writers.live = true;
        let judged = parse("--deterministic")
            .expect("one flag")
            .over(writers.clone());
        assert!(!judged.live);
        let writers_own = parse("--theme dark")
            .expect("one flag")
            .over(writers.clone());
        assert!(writers_own.live);
        let asked = parse("--deterministic --live")
            .expect("two flags")
            .over(writers);
        assert!(asked.live);
    }

    #[test]
    fn syntax_pins_the_whole_table_over_the_writers_choices() {
        let mut writers = Settings::default();
        writers.syntax_highlight.enabled = true;
        writers.syntax_highlight.nouns = false;
        writers.syntax_highlight.adverbs = false;
        assert_eq!(parse("").unwrap().over(writers.clone()), writers);
        for flags in ["--syntax off", "--deterministic"] {
            assert_eq!(
                parse(flags).unwrap().over(writers.clone()).syntax_highlight,
                SyntaxHighlight::default()
            );
        }
        let mut all = SyntaxHighlight::default();
        all.enabled = true;
        assert_eq!(
            parse("--syntax on")
                .unwrap()
                .over(writers.clone())
                .syntax_highlight,
            all
        );
        assert_eq!(
            parse("--syntax nouns,verbs,adjectives,adverbs,conjunctions")
                .unwrap()
                .over(writers.clone())
                .syntax_highlight,
            all
        );
        all.verbs = false;
        all.adjectives = false;
        all.conjunctions = false;
        assert_eq!(
            parse("--deterministic --syntax nouns,adverbs")
                .unwrap()
                .over(writers)
                .syntax_highlight,
            all
        );
        for bad in ["nouns,", ",adverbs", "on,nouns", "nouns,pronouns"] {
            let error = parse(&format!("--syntax {bad}")).unwrap_err().to_string();
            assert!(error.starts_with("--syntax: "));
            assert!(!error.contains('\n'));
        }
    }

    #[test]
    fn style_pins_the_whole_table_over_the_writers_choices() {
        let mut writers = Settings::default();
        writers.style_check.enabled = true;
        writers.style_check.fillers = false;
        writers.style_check.cliches = false;
        assert_eq!(parse("").unwrap().over(writers.clone()), writers);
        for flags in ["--style off", "--deterministic"] {
            assert_eq!(
                parse(flags).unwrap().over(writers.clone()).style_check,
                StyleCheck::default()
            );
        }
        let mut all = StyleCheck::default();
        all.enabled = true;
        assert_eq!(
            parse("--style on")
                .unwrap()
                .over(writers.clone())
                .style_check,
            all
        );
        assert_eq!(
            parse("--style fillers,redundancies,cliches")
                .unwrap()
                .over(writers.clone())
                .style_check,
            all
        );
        all.redundancies = false;
        assert_eq!(
            parse("--deterministic --style fillers,cliches")
                .unwrap()
                .over(writers)
                .style_check,
            all
        );
        for bad in ["fillers,", ",cliches", "on,fillers", "fillers,jargon"] {
            let error = parse(&format!("--style {bad}")).unwrap_err().to_string();
            assert!(error.starts_with("--style: "));
            assert!(!error.contains('\n'));
        }
    }

    #[test]
    fn stats_pins_the_whole_table_over_the_writers_choices() {
        let mut writers = Settings::default();
        writers.stats.show = vec![Statistic::Paragraphs];
        writers.stats.bar = StatsBar::Hidden;
        assert_eq!(parse("").unwrap().over(writers.clone()), writers);
        // No flag under `--deterministic` is the defaults: the three cells
        // every judged state carrying the chrome was frozen at.
        assert_eq!(
            parse("--deterministic")
                .unwrap()
                .over(writers.clone())
                .stats,
            Stats::default()
        );
        let two = parse("--stats words,sentences")
            .unwrap()
            .over(writers.clone())
            .stats;
        assert_eq!(two.show, [Statistic::Words, Statistic::Sentences]);
        assert_eq!(two.bar, StatsBar::Shown, "naming Statistics shows the bar");
        // `hidden` hides the bar and leaves the checked set at the defaults,
        // there being no set to read off a command line that named none.
        let hidden = parse("--deterministic --stats hidden")
            .unwrap()
            .over(writers.clone())
            .stats;
        assert_eq!(hidden.bar, StatsBar::Hidden);
        assert_eq!(hidden.show, Stats::default().show);
        // The cells are laid out in `Statistic::ALL`'s order whatever order
        // the flag named them in, so the flag keeps the order it was given
        // and the bar is what sorts it.
        let named = parse("--stats readingTime,words").unwrap().stats.unwrap();
        assert_eq!(named.show, [Statistic::ReadingTime, Statistic::Words]);
        let twice = parse("--stats words,words").unwrap().stats.unwrap();
        assert_eq!(twice.show, [Statistic::Words], "a repeat is one cell");
        // The empty list is the bar shown with no cells — what unchecking
        // every Statistic leaves, and the only spelling that reaches it.
        let empty = Flags::parse(["--stats", ""].iter().map(OsString::from))
            .unwrap()
            .stats
            .unwrap();
        assert!(empty.show.is_empty());
        assert_eq!(
            empty.bar,
            StatsBar::Shown,
            "an empty bar is not a hidden one"
        );
        for bad in ["words,", ",words", "off", "on", "words,syllables"] {
            let error = parse(&format!("--stats {bad}")).unwrap_err().to_string();
            assert!(error.starts_with("--stats: "), "{error}");
            assert!(!error.contains('\n'));
        }
    }

    /// The fixture is the whole of the Library for a judged launch: the one
    /// Location, and the four switches that decide which rows are drawn and
    /// under what names answered no, whatever the writer turned on in their
    /// own file. Round 6 of the `files` Piece was shot on a `settings.toml`
    /// with all four off, and a writer flipping one is not a change to the
    /// pixels the Piece is judged at.
    #[test]
    fn the_library_flag_pins_the_whole_table_the_judged_shot_reads() {
        let mut writers = Settings::default();
        writers.library.locations = vec![PathBuf::from("/home/writer/Writing")];
        writers.library.pinned = vec![PathBuf::from("/home/writer/Writing/sea-storm.md")];
        writers.library.show_hidden = true;
        writers.library.show_extensions = true;
        writers.library.confirm_move = true;
        writers.library.ask_where_to_save = true;
        let judged = parse("--library shots/oracle/library")
            .expect("one flag")
            .over(writers.clone());
        assert_eq!(
            judged.library.locations,
            vec![PathBuf::from("shots/oracle/library")],
            "the fixture, and no folder of the writer's"
        );
        assert!(judged.library.pinned.is_empty());
        assert!(!judged.library.show_hidden, "the dot-folder stays hidden");
        assert!(!judged.library.show_extensions);
        assert!(!judged.library.confirm_move);
        assert!(!judged.library.ask_where_to_save);
        assert_eq!(
            parse("--sidebar")
                .expect("one flag")
                .over(writers.clone())
                .library,
            writers.library,
            "a launch that named no fixture is the writer's Library exactly"
        );
    }

    /// A judged shot of the Preview names where the pane opens and which
    /// Template is on the page, and the rest of both tables is pinned to its
    /// defaults with it: a writer reading at 140 % in Manuscript Duo with
    /// numbered headings is not shooting the state that was judged. Absent
    /// under `--deterministic` they are pinned all the same — closed and
    /// `modern` — so every state judged before Preview existed shoots as it
    /// did.
    #[test]
    fn the_preview_and_template_flags_pin_the_whole_tables_the_judged_shot_reads() {
        let mut writers = Settings::default();
        writers.preview.layout = PreviewLayout::Full;
        writers.preview.mode = PreviewMode::Pdf;
        writers.preview.zoom = 140;
        writers.template.name = TemplateName::ManuscriptDuo;
        writers.template.center_headings = false;
        writers.template.number_headings = true;
        writers.template.indent_paragraphs = true;

        let judged = parse("--preview split --template classic")
            .expect("two flags")
            .over(writers.clone());
        assert_eq!(judged.preview.layout, PreviewLayout::Split);
        assert_eq!(
            judged.preview.mode,
            PreviewMode::Web,
            "a writer who reads in PDF mode is not shooting `preview/split`"
        );
        assert_eq!(judged.preview.zoom, Settings::default().preview.zoom);
        assert_eq!(judged.template.name, TemplateName::Classic);
        assert_eq!(judged.template, {
            let mut wanted = TemplateSettings::default();
            wanted.name = TemplateName::Classic;
            wanted
        });

        let bare = parse("--deterministic")
            .expect("one flag")
            .over(writers.clone());
        assert_eq!(
            (bare.preview, bare.template),
            (Preview::default(), TemplateSettings::default()),
            "a judged shot that names neither is shot with neither"
        );
        assert!(
            parse("--deterministic")
                .expect("one flag")
                .preview
                .is_none(),
            "and with no pane at all: the table says where one would open"
        );

        let live = parse("--theme dark")
            .expect("one flag")
            .over(writers.clone());
        assert_eq!(
            (live.preview, live.template),
            (writers.preview, writers.template),
            "a writer's launch that named neither is theirs exactly"
        );
    }

    /// Every word `--preview` offers opens the pane, and each names the pair a
    /// shot of the pane is a shot of: the table it stands the pane in is
    /// [`PREVIEWS`] itself, walked here rather than written out a second time.
    /// A fifth word is refused with the module's usual message, and a word
    /// `--help` names that nothing parses would be a state the Gate could
    /// write and never shoot.
    #[test]
    fn preview_takes_every_word_the_usage_offers() {
        for (word, (layout, mode)) in PREVIEWS {
            let flags = parse(&format!("--preview {word}")).expect("a word --help names");
            assert_eq!(flags.preview, Some((layout, mode)), "--preview {word}");
            assert!(
                USAGE.contains(word),
                "--help names every word the flag takes, and not {word}"
            );
            let settings = flags.over(Settings::default());
            assert_eq!(
                (settings.preview.layout, settings.preview.mode),
                (layout, mode),
                "--preview {word} reaches the table the pane reads"
            );
        }

        // A word that is a mode on its own is not one of the four: the flag
        // takes the pair or nothing.
        let err = parse("--preview pdf").expect_err("`pdf` names no layout");
        let said = err.to_string();
        assert_eq!(
            said,
            "--preview: \"pdf\" is not one of split, full, pdf-split, pdf-full"
        );
        assert!(!said.contains('\n'), "one line, not a stack: {said}");
    }

    /// `--zoom` is the one key of `[preview]` a judged state names for itself:
    /// it stands over the pinning `--preview` does, so that `preview/pdf-full`
    /// holds two pages of the column in one window, and a state that does not
    /// name it is shot at fit width. A per cent the setting does not take is
    /// refused in the module's one line.
    #[test]
    fn zoom_pins_the_panes_own_zoom_over_the_rest_of_the_table() {
        let flags = parse("--preview pdf-full --zoom 50").expect("a per cent in the range");
        assert_eq!(flags.zoom, Some(50));
        assert_eq!(
            flags.over(Settings::default()).preview.zoom,
            50,
            "the table the column lays its pages out at"
        );
        assert_eq!(
            parse("--preview pdf-full")
                .expect("the pane without a zoom")
                .over(Settings::default())
                .preview
                .zoom,
            Preview::default().zoom,
            "a state that names no zoom is shot at fit width"
        );

        let zooms = preview_zooms();
        let past = zooms.end() + 1;
        let err = parse(&format!("--zoom {past}")).expect_err("a per cent past the range");
        let said = err.to_string();
        assert_eq!(
            said,
            format!(
                "--zoom: \"{past}\" is not a whole number from {} to {}",
                zooms.start(),
                zooms.end()
            )
        );
        assert!(!said.contains('\n'), "one line, not a stack: {said}");
        assert!(USAGE.contains("--zoom"), "--help names the flag");
    }

    /// Every word the flag offers in `--help` opens a dialog, and the three
    /// words are the three formats: a fourth is refused by the table test
    /// above, and a word `--help` names that nothing parses would be a state
    /// the Gate could write and never shoot.
    #[test]
    fn export_dialog_takes_every_word_the_usage_offers() {
        for value in Format::VALUES {
            let flags = parse(&format!("--export-dialog {value}")).expect("a format --help names");
            assert!(flags.export_dialog.is_some(), "--export-dialog {value}");
            assert!(
                USAGE.contains(value),
                "--help offers every word the flag takes: {value}"
            );
        }
        assert_eq!(
            parse("--export-dialog pdf")
                .expect("one flag")
                .export_dialog,
            Some(Format::Pdf)
        );
        assert_eq!(
            parse("--export-dialog html")
                .expect("one flag")
                .export_dialog,
            Some(Format::Html)
        );
        assert_eq!(
            parse("--export-dialog markdown")
                .expect("one flag")
                .export_dialog,
            Some(Format::Markdown)
        );
        assert!(
            Flags::default().export_dialog.is_none(),
            "a launch that named none opens none"
        );
    }

    /// The `export/dialog` state is the `[export]` table drawn as rows, so the
    /// whole table is pinned to its defaults under `--deterministic` — and
    /// `paper` past its default, because `auto` is the host's locale and a
    /// judged shot is the same on every machine.
    #[test]
    fn export_is_pinned_to_its_defaults_under_every_judged_state() {
        let mut writers = Settings::default();
        writers.export.paper = Paper::Legal;
        writers.export.margin = 35;
        writers.export.text_size = 14;
        writers.export.title_page = true;
        writers.export.header = true;
        writers.export.footer = true;

        let judged = parse("--deterministic")
            .expect("one flag")
            .over(writers.clone());
        assert_eq!(judged.export, {
            let mut wanted = Export::default();
            wanted.paper = Paper::A4;
            wanted
        });
        assert_eq!(
            judged.export.paper,
            Paper::A4,
            "one paper, not the locale's"
        );
        assert_ne!(
            Export::default().paper,
            Paper::A4,
            "and past the default, which is the locale's"
        );

        let live = parse("--theme dark")
            .expect("one flag")
            .over(writers.clone());
        assert_eq!(
            live.export, writers.export,
            "a writer's launch is their own table exactly"
        );
    }

    #[test]
    fn a_flag_overrides_the_setting_it_matches_and_leaves_the_rest_alone() {
        let writers = Settings::default();
        let flags =
            parse("--theme dark --font mono --step 6 --focus paragraph --typewriter --chrome off")
                .expect("six settings overridden");
        let launched = flags.over(writers.clone());
        assert_eq!(launched.theme, quill_engine::settings::Theme::Dark);
        assert_eq!(launched.face, Face::Mono);
        assert_eq!(launched.step, 6);
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

    /// `--palette` names the file for the launch; `--theme` on its own pins
    /// the built-ins for its ground, so the Gate's shots never see a writer's
    /// palette; the two together are a palette previewed on a pinned ground;
    /// and any other flag leaves the setting where it was.
    #[test]
    fn the_palette_flag_lays_its_file_over_the_setting_and_theme_alone_pins_the_built_ins() {
        let mut writers = Settings::default();
        writers.palette = Some(PathBuf::from("/theme/quill.toml"));
        let previewed = parse("--palette quill.toml")
            .expect("one flag")
            .over(writers.clone());
        assert_eq!(previewed.palette.as_deref(), Some(Path::new("quill.toml")));
        let pinned = parse("--theme light")
            .expect("one flag")
            .over(writers.clone());
        assert_eq!(
            pinned.palette, None,
            "the setting is not read under --theme"
        );
        let both = parse("--theme light --palette quill.toml")
            .expect("two flags")
            .over(writers.clone());
        assert_eq!(both.palette.as_deref(), Some(Path::new("quill.toml")));
        let live = parse("--font mono")
            .expect("one flag")
            .over(writers.clone());
        assert_eq!(
            live.palette, writers.palette,
            "any other flag leaves the setting alone"
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
