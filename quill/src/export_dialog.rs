//! The Export dialog: the three Commands that ask before they write, and the
//! options widget the expander holds.
//!
//! GTK 4's file dialog takes no extra widgets, so the dialog is Quill's own
//! (ADR 0009, plain GTK): a file-name field, a button naming the folder that
//! opens the system's folder chooser, an **Options** expander that is shut
//! until asked for, and an Export button Enter fires. What the writer chooses
//! is one job's worth — every open seeds from the settings file again — so
//! nothing here is remembered, and **Save as defaults** is the one way a
//! choice reaches `[export]`.
//!
//! The engine does the writing (ADR 0008): [`quill_engine::pdf::write`] lays
//! the Document out on paper, [`quill_engine::html::page`] answers a styled
//! page, and Markdown is the Document's own bytes ([`copy`]) — Duplicate
//! Document to a folder the writer names, which is why the Document does not
//! follow the file.
//!
//! [`Options`] is a widget of its own rather than part of the dialog because
//! Print puts the same one in a `GtkPrintOperation` tab, where there is no
//! dialog around it; [`Chosen`] is what it reads back, and the one shape both
//! an export and a write to `[export]` are made from.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gdk, gio, glib};

use quill_engine::document::full_name;
use quill_engine::render::Toggles;
use quill_engine::settings::{Choice, Export, Paper, Template, export_text_sizes};
use quill_engine::{draw, html, pdf, template};

use crate::export::{confirm, file_name, geometry};
use crate::files;
use crate::session::Session;
use crate::window::Window;

/// How wide a dialog opens, in logical pixels.
///
/// Wider than the rename dialog's, because the widest thing here is a folder's
/// name rather than a file's.
const DIALOG_WIDTH: i32 = 420;

/// The gap around and between the dialog's rows.
const PAD: i32 = 12;

/// The gap between an options row's label and its control.
const COLUMN_GAP: i32 = 24;

/// What the shut expander says.
const OPTIONS: &str = "Options";

/// What the button Enter fires says.
const EXPORT: &str = "Export";

/// What the button that writes `[export]` says.
const SAVE_DEFAULTS: &str = "Save as defaults";

/// The overwrite confirm's two answers, the first of them its default.
const REPLACE: &str = "Replace";

/// The answer that writes nothing.
const CANCEL: &str = "Cancel";

/// The papers the dropdown offers, in the order it offers them, each with the
/// words it is offered under.
///
/// The order is [`Paper`]'s own, so that the row a writer picks and the value
/// written to `[export]` cannot drift; `auto` is first because it is the
/// default and it is what most writers never change.
const PAPERS: [(Paper, &str); 4] = [
    (Paper::Auto, "Automatic"),
    (Paper::A4, "A4"),
    (Paper::Letter, "Letter"),
    (Paper::Legal, "Legal"),
];

/// Which file an Export dialog writes, and so what the dialog is called, what
/// the seeded name ends in, and which writer runs.
///
/// `pub` where the rest of this module is `pub(crate)`, and it reaches no
/// further for it — this is a binary crate — because [`crate::flags::Flags`]
/// carries one as a public field for `--export-dialog`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Format {
    /// The Document laid out on paper, through the engine's PDF writer.
    Pdf,
    /// The Document as a standalone styled page.
    Html,
    /// The Document's own bytes, front matter and all.
    Markdown,
}

impl Format {
    /// The words `--export-dialog` takes, in the order `--help` prints them.
    pub(crate) const VALUES: [&'static str; 3] = ["pdf", "html", "markdown"];

    /// The format `written` names, or `None` for a word that names none.
    ///
    /// The Gate's `--export-dialog` is the one caller: a format is not a
    /// setting and is never written to a file, so this is a flag's spelling
    /// rather than [`quill_engine::settings::Choice`].
    pub(crate) fn parse(written: &str) -> Option<Self> {
        match written {
            "pdf" => Some(Self::Pdf),
            "html" => Some(Self::Html),
            "markdown" => Some(Self::Markdown),
            _ => None,
        }
    }

    /// What the seeded file name ends in.
    fn extension(self) -> &'static str {
        match self {
            Self::Pdf => "pdf",
            Self::Html => "html",
            Self::Markdown => "md",
        }
    }

    /// What the dialog's title bar says.
    fn title(self) -> &'static str {
        match self {
            Self::Pdf => "Export PDF",
            Self::Html => "Export HTML",
            Self::Markdown => "Export Markdown",
        }
    }

    /// How much of the page this format's expander offers, and `None` where it
    /// has no expander at all.
    ///
    /// Markdown carries no options because nothing about the page reaches a
    /// Markdown file: it is the bytes, and paper has no say over them.
    fn depth(self) -> Option<Depth> {
        match self {
            Self::Pdf => Some(Depth::Page),
            Self::Html => Some(Depth::Toggles),
            Self::Markdown => None,
        }
    }
}

/// How much of the page an [`Options`] widget offers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Depth {
    /// The whole page: paper, text size, the three Template toggles and the
    /// three pieces of furniture. What a PDF and Print are laid out with.
    Page,
    /// The three Template toggles alone, which is all an HTML page has: it is
    /// laid out by the browser and has no paper.
    Toggles,
}

/// Everything one export is laid out with: the `[export]` table's own fields
/// and the three `[template]` toggles beside them.
///
/// One struct rather than a pair, because every consumer wants both: the
/// geometry a page is laid out on and the toggles the blocks are laid out
/// with travel together from the widget to the writer.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Chosen {
    /// The paper a page is laid out on, `auto` still unresolved.
    pub(crate) paper: Paper,
    /// The margin on every side, in whole millimetres.
    ///
    /// Not offered by the widget — the expander holds what one job changes,
    /// and a margin is a house style — but carried, so that Save as defaults
    /// writes the whole table rather than a hole in it.
    pub(crate) margin: u32,
    /// The size the body is set at, in whole points.
    pub(crate) text_size: u32,
    /// Whether the export opens with a title page.
    pub(crate) title_page: bool,
    /// Whether each page carries the Document's name in its top margin.
    pub(crate) header: bool,
    /// Whether each page carries its number in its bottom margin.
    pub(crate) footer: bool,
    /// The three Template toggles as this export lays the blocks out.
    pub(crate) toggles: Toggles,
}

impl Chosen {
    /// What a dialog opens on: the `[export]` table as the file has it and the
    /// `[template]` toggles as the writer is reading them now.
    ///
    /// Every open, rather than the last export's: an export is one job, and a
    /// title page turned on for a submission is not a thing a writer wants
    /// back on the next draft.
    pub(crate) fn of(export: &Export, template: &Template) -> Self {
        Self {
            paper: export.paper,
            margin: export.margin,
            text_size: export.text_size,
            title_page: export.title_page,
            header: export.header,
            footer: export.footer,
            toggles: Toggles::of(template),
        }
    }

    /// Puts this choice into `export`, leaving everything else in the table
    /// alone.
    ///
    /// A write over an `[export]` the file already holds rather than a fresh
    /// one, so that the keys the table carries but this struct does not know
    /// survive the write ([`Export`]'s own unknown keys).
    ///
    /// The toggles are not written: they are `[template]`'s, and the Settings
    /// window and View › Template are what move them.
    pub(crate) fn written_into(&self, export: &mut Export) {
        export.paper = self.paper;
        export.margin = self.margin;
        export.text_size = self.text_size;
        export.title_page = self.title_page;
        export.header = self.header;
        export.footer = self.footer;
    }
}

/// The options widget: one grid of rows, and the controls to read back.
///
/// The controls are held rather than looked up again, because a `gtk::Grid`
/// answers a child by its position and a position is not a name. A control the
/// depth left out is `None`, and [`Chosen::seed`] answers for it — an HTML
/// export still has a paper somewhere behind it, and Print's tab still has the
/// margin the page setup was seeded from.
pub(crate) struct Options {
    /// The rows, which is what a dialog or a print tab puts on screen.
    grid: gtk::Grid,
    /// What the widget was built on: the answer for every control the depth
    /// did not build.
    seed: Chosen,
    /// The paper dropdown, over [`PAPERS`].
    paper: Option<gtk::DropDown>,
    /// The body size in points, over [`export_text_sizes`].
    text_size: Option<gtk::SpinButton>,
    /// Centre every heading.
    center_headings: gtk::Switch,
    /// Number the headings under the title.
    number_headings: gtk::Switch,
    /// Indent a paragraph's first line.
    indent_paragraphs: gtk::Switch,
    /// Open with a title page.
    title_page: Option<gtk::Switch>,
    /// The Document's name in the top margin.
    header: Option<gtk::Switch>,
    /// The page number in the bottom margin.
    footer: Option<gtk::Switch>,
}

impl Options {
    /// Builds the widget on `seed`, offering as much of the page as `depth`
    /// says.
    ///
    /// Every control is set before anything is connected to it, which is what
    /// the Settings window's rows do and for the same reason: building a
    /// widget is not a writer changing something.
    pub(crate) fn new(seed: &Chosen, depth: Depth) -> Self {
        let grid = gtk::Grid::builder()
            .row_spacing(PAD)
            .column_spacing(COLUMN_GAP)
            .build();
        let mut at = 0;
        let (paper, text_size) = match depth {
            Depth::Page => {
                let paper = paper_drop_down(seed.paper);
                row(&grid, &mut at, "Paper", &paper);
                let sizes = export_text_sizes();
                let text_size = gtk::SpinButton::with_range(
                    f64::from(*sizes.start()),
                    f64::from(*sizes.end()),
                    1.0,
                );
                text_size.set_value(f64::from(seed.text_size));
                row(&grid, &mut at, "Text size", &text_size);
                (Some(paper), Some(text_size))
            }
            Depth::Toggles => (None, None),
        };
        let center_headings = switch(seed.toggles.center_headings);
        row(&grid, &mut at, "Center headings", &center_headings);
        let number_headings = switch(seed.toggles.number_headings);
        row(&grid, &mut at, "Number headings", &number_headings);
        let indent_paragraphs = switch(seed.toggles.indent_paragraphs);
        row(&grid, &mut at, "Indent paragraphs", &indent_paragraphs);
        let (title_page, header, footer) = match depth {
            Depth::Page => {
                let title_page = switch(seed.title_page);
                row(&grid, &mut at, "Title page", &title_page);
                let header = switch(seed.header);
                row(&grid, &mut at, "Header", &header);
                let footer = switch(seed.footer);
                row(&grid, &mut at, "Footer", &footer);
                (Some(title_page), Some(header), Some(footer))
            }
            Depth::Toggles => (None, None, None),
        };
        Self {
            grid,
            seed: seed.clone(),
            paper,
            text_size,
            center_headings,
            number_headings,
            indent_paragraphs,
            title_page,
            header,
            footer,
        }
    }

    /// The widget itself, to be put in an expander or a print tab.
    pub(crate) fn widget(&self) -> &gtk::Grid {
        &self.grid
    }

    /// What the widget now says, with the seed answering for every control it
    /// does not carry.
    pub(crate) fn chosen(&self) -> Chosen {
        let mut chosen = self.seed.clone();
        if let Some(paper) = &self.paper {
            chosen.paper = paper_at(paper.selected());
        }
        if let Some(text_size) = &self.text_size {
            chosen.text_size =
                u32::try_from(text_size.value_as_int()).unwrap_or(self.seed.text_size);
        }
        chosen.toggles = Toggles {
            center_headings: self.center_headings.is_active(),
            number_headings: self.number_headings.is_active(),
            indent_paragraphs: self.indent_paragraphs.is_active(),
        };
        if let Some(title_page) = &self.title_page {
            chosen.title_page = title_page.is_active();
        }
        if let Some(header) = &self.header {
            chosen.header = header.is_active();
        }
        if let Some(footer) = &self.footer {
            chosen.footer = footer.is_active();
        }
        chosen
    }
}

/// One row of the options grid: its label at the left, its control at the
/// right, and `at` moved on to the next row.
fn row(grid: &gtk::Grid, at: &mut i32, label: &str, control: &impl IsA<gtk::Widget>) {
    let label = gtk::Label::builder()
        .label(label)
        .halign(gtk::Align::Start)
        .build();
    grid.attach(&label, 0, *at, 1, 1);
    grid.attach(control, 1, *at, 1, 1);
    *at += 1;
}

/// A switch of the options grid, set to `on` before anything is connected.
fn switch(on: bool) -> gtk::Switch {
    let switch = gtk::Switch::builder().halign(gtk::Align::End).build();
    switch.set_active(on);
    switch
}

/// The paper dropdown, standing on `paper`.
///
/// The Settings window's Export group offers the same rows from the same
/// table, so that a paper named there and a paper named here are the one list
/// (#290).
pub(crate) fn paper_drop_down(paper: Paper) -> gtk::DropDown {
    let words: Vec<&str> = PAPERS.iter().map(|(_, words)| *words).collect();
    let drop_down = gtk::DropDown::from_strings(&words);
    drop_down.set_selected(index_of(paper));
    drop_down
}

/// The paper the dropdown's `index`-th row names, and the default for an index
/// [`PAPERS`] does not reach — which is what `GTK_INVALID_LIST_POSITION` is.
pub(crate) fn paper_at(index: u32) -> Paper {
    usize::try_from(index)
        .ok()
        .and_then(|index| PAPERS.get(index))
        .map_or_else(Paper::default, |(paper, _)| *paper)
}

/// Which of the dropdown's rows `paper` stands on.
fn index_of(paper: Paper) -> u32 {
    let found = PAPERS.iter().position(|(offered, _)| *offered == paper);
    u32::try_from(found.unwrap_or_default()).unwrap_or_default()
}

/// The one line the overwrite confirm asks.
///
/// One line and two answers: the writer named a file that is already there,
/// and the only two things to do about it are to write over it or not to.
fn replacing(name: &str) -> String {
    format!("Replace {name}?")
}

/// The folder a dialog opens on: the Document's own, else the Library's first
/// Location, else the writer's home.
///
/// Home rather than nothing, because the button names a folder and an untitled
/// Document in a launch with no Location still has to name one.
fn opening_folder(path: Option<&Path>, location: Option<PathBuf>) -> PathBuf {
    path.and_then(Path::parent)
        .map(Path::to_path_buf)
        .or(location)
        .unwrap_or_else(glib::home_dir)
}

/// Markdown export: the Document's bytes at `path`, front matter and all.
///
/// The text and not the file on disk, so that a Document with unsaved changes
/// exports what the writer is looking at; the Document's own path is not
/// touched, which is the whole difference between this and Save As.
fn copy(text: &str, path: &Path) -> std::io::Result<()> {
    std::fs::write(path, text.as_bytes())
}

/// `export.pdf`, `export.html` and `export.markdown`: the dialog for `format`,
/// over the window whose Document it writes.
///
/// The expander is shut, which is what a writer opens it on: Options is what
/// one job changes and most jobs change nothing.
pub(crate) fn open(window: &Window, format: Format) {
    opened(window, format, false);
}

/// The same dialog with its Options expander already open: `--export-dialog`,
/// and the `export/dialog` judged state alone (ADR 0017).
///
/// A still cannot open an expander, so the state that shows what is inside one
/// is launched with it open. Nothing else calls this, and no writer can reach
/// it: the flag is the harness's.
pub(crate) fn open_expanded(window: &Window, format: Format) {
    opened(window, format, true);
}

fn opened(window: &Window, format: Format, expanded: bool) {
    let Some(session) = window.session() else {
        return;
    };
    let seed = {
        let settings = session.settings();
        let template = session.template();
        Chosen::of(&settings.export, &template)
    };
    let dialog = gtk::Window::builder()
        .title(format.title())
        .transient_for(window)
        .modal(true)
        .destroy_with_parent(true)
        .resizable(false)
        .default_width(DIALOG_WIDTH)
        .build();
    let column = gtk::Box::new(gtk::Orientation::Vertical, PAD);
    column.set_margin_top(PAD);
    column.set_margin_bottom(PAD);
    column.set_margin_start(PAD);
    column.set_margin_end(PAD);

    let seeded = file_name(&window.document().name(), format.extension());
    let name = gtk::Entry::new();
    name.set_text(&seeded);
    name.select_region(0, files::stem_chars(&seeded));
    // Enter in the field is the Export button, not a signal of the field's
    // own: one handler then answers Enter from anywhere in the dialog, which
    // is what the writer who opened Options and typed nothing expects.
    name.set_activates_default(true);
    column.append(&name);

    let chosen_folder = Rc::new(RefCell::new(opening_folder(
        window.path().as_deref(),
        session.first_location(),
    )));
    let button = gtk::Button::with_label(&full_name(&chosen_folder.borrow()));
    button.connect_clicked(glib::clone!(
        #[weak]
        dialog,
        #[strong]
        chosen_folder,
        move |button| pick_folder(&dialog, button, &chosen_folder)
    ));
    column.append(&button);

    let options = format
        .depth()
        .map(|depth| Rc::new(Options::new(&seed, depth)));
    if let Some(options) = &options {
        column.append(&expander(&session, options, expanded));
    }

    let go = gtk::Button::builder()
        .label(EXPORT)
        .halign(gtk::Align::End)
        .receives_default(true)
        .build();
    column.append(&go);
    dialog.set_child(Some(&column));
    dialog.set_default_widget(Some(&go));

    go.connect_clicked(glib::clone!(
        #[weak(rename_to = asked)]
        dialog,
        #[weak]
        window,
        #[weak]
        name,
        #[strong]
        chosen_folder,
        move |_| {
            let typed = name.text().trim().to_string();
            if typed.is_empty() {
                return;
            }
            let target = chosen_folder.borrow().join(&typed);
            let chosen = options
                .as_ref()
                .map_or_else(|| seed.clone(), |options| options.chosen());
            if target.exists() {
                ask_to_replace(&window, &asked, target, format, chosen);
            } else {
                asked.close();
                write(&window, format, &target, &chosen);
            }
        }
    ));

    let keys = gtk::EventControllerKey::new();
    keys.connect_key_pressed(glib::clone!(
        #[weak]
        dialog,
        #[upgrade_or]
        glib::Propagation::Proceed,
        move |_, key, _, _| {
            if key == gdk::Key::Escape {
                dialog.close();
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        }
    ));
    dialog.add_controller(keys);
    dialog.present();
}

/// The **Options** expander: the widget, shut unless `expanded`, with the one
/// button that keeps what is inside it.
///
/// `expanded` is `--export-dialog`'s and nothing else's: a writer's dialog
/// opens shut ([`open`]), and the judged state opens it open because a still
/// cannot pull it.
fn expander(session: &Rc<Session>, options: &Rc<Options>, expanded: bool) -> gtk::Expander {
    let inside = gtk::Box::new(gtk::Orientation::Vertical, PAD);
    inside.set_margin_top(PAD);
    inside.append(options.widget());
    let defaults = gtk::Button::builder()
        .label(SAVE_DEFAULTS)
        .halign(gtk::Align::End)
        .build();
    defaults.connect_clicked(glib::clone!(
        #[strong]
        session,
        #[strong]
        options,
        move |_| {
            let chosen = options.chosen();
            session.edit_settings(|settings| chosen.written_into(&mut settings.export));
        }
    ));
    inside.append(&defaults);
    let expander = gtk::Expander::new(Some(OPTIONS));
    expander.set_expanded(expanded);
    expander.set_child(Some(&inside));
    expander
}

/// The folder button: the system's own chooser, as Add Location opens it, and
/// the button renamed for what came back.
fn pick_folder(dialog: &gtk::Window, button: &gtk::Button, chosen: &Rc<RefCell<PathBuf>>) {
    let picker = gtk::FileDialog::new();
    picker.set_title("Export To");
    picker.set_initial_folder(Some(&gio::File::for_path(chosen.borrow().as_path())));
    picker.select_folder(
        Some(dialog),
        None::<&gio::Cancellable>,
        glib::clone!(
            #[weak]
            button,
            #[strong(rename_to = chosen)]
            chosen,
            move |answer| {
                let Some(folder) = answer.ok().and_then(|file| file.path()) else {
                    return;
                };
                button.set_label(&full_name(&folder));
                *chosen.borrow_mut() = folder;
            }
        ),
    );
}

/// The one-line confirm a file that is already there gets, and the export it
/// leads to.
///
/// Over the dialog rather than the window, so the answer stands in front of
/// the name that provoked it; Cancel leaves the dialog open, which is what
/// makes it the answer that loses nothing — the writer types another name.
fn ask_to_replace(
    window: &Window,
    dialog: &gtk::Window,
    target: PathBuf,
    format: Format,
    chosen: Chosen,
) {
    let alert = gtk::AlertDialog::builder()
        .modal(true)
        .message(replacing(&full_name(&target)))
        .buttons([REPLACE, CANCEL])
        .default_button(0)
        .cancel_button(1)
        .build();
    let window = window.clone();
    let asked = dialog.clone();
    alert.choose(Some(dialog), None::<&gio::Cancellable>, move |answer| {
        // Cancel, `Esc`, or a confirm that could not be shown: the file on
        // disk stands and the dialog stays up.
        if matches!(answer, Ok(0)) {
            asked.close();
            write(&window, format, &target, &chosen);
        }
    });
}

/// Writes the export at `target` and confirms it.
///
/// A write that fails is one line on stderr, as every other file the app
/// writes is, and no [`confirm`]: the confirmation is the one thing that says
/// a file is there, so it never says so about a file that is not.
fn write(window: &Window, format: Format, target: &Path, chosen: &Chosen) {
    let Some(session) = window.session() else {
        return;
    };
    let built = template::named(session.template().name.as_str());
    let written = match format {
        Format::Pdf => {
            let mut export = session.settings().export.clone();
            chosen.written_into(&mut export);
            let document = window.document();
            pdf::write(
                target,
                &document,
                &built,
                chosen.toggles,
                geometry(&export),
                f64::from(export.text_size),
                &draw::wording(&document),
            )
            .map_err(|err| err.to_string())
        }
        Format::Html => {
            let document = window.document();
            let page = html::page(&document, &built, chosen.toggles);
            drop(document);
            std::fs::write(target, page).map_err(|err| err.to_string())
        }
        Format::Markdown => {
            let document = window.document();
            let copied = copy(document.text(), target);
            drop(document);
            copied.map_err(|err| err.to_string())
        }
    };
    if let Err(err) = written {
        eprintln!("quill: cannot export {}: {err}", target.display());
        return;
    }
    confirm(window, target);
}

#[cfg(test)]
mod tests {
    use quill_engine::settings::{Choice, Settings};

    use super::*;

    /// Each format names its own file and its own dialog, and only the two
    /// that lay a page out carry an expander.
    #[test]
    fn each_format_names_its_file_and_carries_the_options_a_page_has() {
        for (format, extension, title, depth) in [
            (Format::Pdf, "pdf", "Export PDF", Some(Depth::Page)),
            (Format::Html, "html", "Export HTML", Some(Depth::Toggles)),
            (Format::Markdown, "md", "Export Markdown", None),
        ] {
            assert_eq!(format.extension(), extension);
            assert_eq!(format.title(), title);
            assert_eq!(format.depth(), depth);
            assert_eq!(
                file_name("The Lighthouse", format.extension()),
                format!("The Lighthouse.{extension}")
            );
        }
    }

    /// The dialog opens on the `[export]` table and the `[template]` toggles
    /// as they now stand, and a default of each is the defaults.
    #[test]
    fn the_options_seed_from_the_export_table_and_the_current_toggles() {
        let export = Export::default();
        let template = Template::default();
        assert_eq!(
            Chosen::of(&export, &template),
            Chosen {
                paper: export.paper,
                margin: export.margin,
                text_size: export.text_size,
                title_page: export.title_page,
                header: export.header,
                footer: export.footer,
                toggles: Toggles::of(&template),
            }
        );
        let mut moved = Template::default();
        moved.number_headings = !moved.number_headings;
        assert_eq!(
            Chosen::of(&export, &moved).toggles,
            Toggles::of(&moved),
            "the toggles are the ones the writer is reading now"
        );
    }

    /// What the widget was built on comes back out of it unchanged: a choice
    /// written into a table the file already holds moves the keys it names and
    /// nothing else.
    #[test]
    fn a_seed_written_back_leaves_the_export_table_as_it_was() {
        let export = Export::default();
        let mut same = export.clone();
        Chosen::of(&export, &Template::default()).written_into(&mut same);
        assert_eq!(same, export);
    }

    /// Save as defaults writes the keys the writer moved into `[export]` and
    /// leaves every other line of the settings file as it was.
    #[test]
    fn save_as_defaults_writes_the_changed_export_keys_and_nothing_else() {
        let mut settings = Settings::default();
        let before = settings.to_toml();
        let mut chosen = Chosen::of(&settings.export, &settings.template);
        chosen.paper = Paper::Letter;
        chosen.text_size = 14;
        chosen.footer = true;
        chosen.toggles.number_headings = !chosen.toggles.number_headings;
        chosen.written_into(&mut settings.export);
        assert_eq!(
            (
                settings.export.paper,
                settings.export.text_size,
                settings.export.footer
            ),
            (Paper::Letter, 14, true)
        );
        let after = settings.to_toml();
        let moved: Vec<(&str, &str)> = before
            .lines()
            .zip(after.lines())
            .filter(|(before, after)| before != after)
            .collect();
        assert_eq!(
            moved,
            [
                ("paper = \"auto\"", "paper = \"letter\""),
                ("text_size = 12", "text_size = 14"),
                ("footer = false", "footer = true"),
            ],
            "the three keys the widget moved, and the toggles left in [template]"
        );
    }

    /// The dropdown offers every paper the setting can hold, in the setting's
    /// own order, and the row a writer picks is the paper that is written.
    #[test]
    fn every_paper_is_offered_and_the_row_picked_is_the_paper_written() {
        assert_eq!(
            PAPERS.map(|(paper, _)| paper.as_str()).as_slice(),
            Paper::VALUES
        );
        for (at, (paper, _)) in PAPERS.iter().enumerate() {
            let index = u32::try_from(at).unwrap();
            assert_eq!(index_of(*paper), index);
            assert_eq!(paper_at(index), *paper);
        }
        assert_eq!(
            paper_at(gtk::INVALID_LIST_POSITION),
            Paper::default(),
            "a dropdown standing on nothing is the default paper"
        );
    }

    /// The overwrite confirm asks about the file the writer named, in one
    /// line.
    #[test]
    fn the_overwrite_confirm_asks_about_the_file_that_is_already_there() {
        assert_eq!(
            replacing(&full_name(Path::new("/tmp/drafts/The Lighthouse.pdf"))),
            "Replace The Lighthouse.pdf?"
        );
        assert_eq!(replacing("notes.md"), "Replace notes.md?");
    }

    /// The folder button opens on the Document's own folder, falls back to the
    /// Library's first Location for an untitled Document, and to home when
    /// there is no Location either.
    #[test]
    fn the_folder_is_the_documents_own_then_the_librarys_then_home() {
        let location = PathBuf::from("/tmp/quill-locations");
        assert_eq!(
            opening_folder(
                Some(Path::new("/tmp/drafts/The Lighthouse.md")),
                Some(location.clone())
            ),
            Path::new("/tmp/drafts")
        );
        assert_eq!(opening_folder(None, Some(location.clone())), location);
        assert_eq!(opening_folder(None, None), glib::home_dir());
    }

    /// Markdown export is the Document's bytes at the path the writer named,
    /// front matter and all, and the Document's own file is not touched.
    #[test]
    fn markdown_export_copies_the_documents_bytes_and_leaves_its_file_alone() {
        let text = "---\ntitle: The Lighthouse\n---\n\n# One\n\nThe sea.\n";
        let folder = std::env::temp_dir().join(format!("quill-md-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let own = folder.join("The Lighthouse.md");
        std::fs::write(&own, text).unwrap();
        let target = folder.join("Copy.md");
        copy(text, &target).unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), text.as_bytes());
        assert_eq!(std::fs::read(&own).unwrap(), text.as_bytes());
        std::fs::remove_dir_all(&folder).ok();
    }
}
