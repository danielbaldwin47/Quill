//! `print`, `Ctrl+P`: the Document on the system's printer, through a
//! `GtkPrintOperation` with a Quill tab of its own.
//!
//! Print is the second sink of the one page the engine lays out (`#282`,
//! `docs/architecture.md` § Preview and Export): the PDF writer draws the
//! pages on a cairo surface of its own, and this draws the same pages on the
//! surface the print system hands over, so a print and an export of one
//! Document are the same page in two places.
//!
//! What the writer chooses here is one job's worth. The operation opens on
//! `[export]` — its paper and its margin seed the default page setup, and the
//! Quill tab is the Export dialog's own options widget
//! ([`crate::export_dialog::Options`]) — but the print dialog's Page Setup
//! wins for that job, and nothing is written back to the settings file. There
//! is no confirmation after a print: nothing was written to a file to open.
//!
//! No Print Plain Text: the map's Out of scope.

use std::cell::RefCell;
use std::rc::Rc;

use gtk::glib;
use gtk::prelude::*;

use quill_engine::paginate::{self, Geometry, Laid};
use quill_engine::settings::{Choice, Export, Paper, locale_paper};
use quill_engine::template::Template;
use quill_engine::{draw, template};

use crate::export_dialog::{Chosen, Depth, Options};
use crate::window::Window;

/// What the operation's own tab of the print dialog is called.
const TAB: &str = "Quill";

/// The pages of one print job, and everything the drawer needs to put one of
/// them on paper.
///
/// Built once on `begin-print` and read on every `draw-page`, because the
/// print system asks for a page at a time and laying the Document out again
/// for each of them would lay it out once per page.
struct Job {
    /// The Template the pages were laid out in.
    template: Template,
    /// The Document on the paper the dialog settled on.
    laid: Laid,
}

/// Runs the print operation over `window`.
///
/// Blocking, as `gtk_print_operation_run` is: the dialog runs its own loop and
/// this returns when the job has been sent or the writer has cancelled it. A
/// failure is one line on stderr, as every other thing the app cannot write
/// is.
pub(crate) fn open(window: &Window) {
    let Some(session) = window.session() else {
        return;
    };
    let (export, seed) = {
        let settings = session.settings();
        let template = session.template();
        (
            settings.export.clone(),
            Chosen::of(&settings.export, &template),
        )
    };

    let operation = gtk::PrintOperation::new();
    operation.set_default_page_setup(Some(&page_setup(&export)));
    // The paper's own corner at the origin and distances in points, which is
    // the frame the drawer paints in ([`quill_engine::draw::draw`]): the same
    // numbers the PDF writer's surface is set up with, so the two sinks
    // cannot drift.
    operation.set_use_full_page(true);
    operation.set_unit(gtk::Unit::Points);
    // The dialog's Page Setup is what the spec means by the dialog's own
    // choice winning for the job, so the writer is offered it.
    operation.set_embed_page_setup(true);
    operation.set_job_name(&window.document().name());
    operation.set_custom_tab_label(Some(TAB));

    // Built before the dialog is, and held, because the tab's widget is asked
    // for once and read back once and the two moments are not the same.
    let options = Rc::new(Options::new(&seed, Depth::Page));
    let chosen = Rc::new(RefCell::new(seed));
    let job: Rc<RefCell<Option<Job>>> = Rc::new(RefCell::new(None));

    operation.connect_create_custom_widget(glib::clone!(
        #[strong]
        options,
        move |_| Some(options.widget().clone().upcast())
    ));
    // What the tab says is read here rather than on `begin-print`, because
    // this is the last moment the widget is still standing.
    operation.connect_custom_widget_apply(glib::clone!(
        #[strong]
        options,
        #[strong]
        chosen,
        move |_, _| {
            chosen.replace(options.chosen());
        }
    ));
    operation.connect_begin_print(glib::clone!(
        #[weak]
        window,
        #[strong]
        chosen,
        #[strong]
        job,
        move |operation, context| {
            let laid = lay_out(&window, context, &chosen.borrow());
            let pages = laid.as_ref().map_or(0, |job| job.laid.pages.len());
            // Never nought: the print system takes a page count of nought as
            // a job it cannot start.
            operation.set_n_pages(i32::try_from(pages).unwrap_or(i32::MAX).max(1));
            job.replace(laid);
        }
    ));
    operation.connect_draw_page(glib::clone!(
        #[strong]
        job,
        move |_, context, at| {
            let job = job.borrow();
            let Some(job) = job.as_ref() else {
                return;
            };
            let Some(page) = usize::try_from(at)
                .ok()
                .and_then(|at| job.laid.pages.get(at))
            else {
                return;
            };
            draw::draw(
                &context.cairo_context(),
                page,
                &job.laid.rendered,
                &job.template,
                &job.laid.frame,
            );
        }
    ));

    if let Err(err) = operation.run(gtk::PrintOperationAction::PrintDialog, Some(window)) {
        eprintln!("quill: cannot print: {err}");
    }
}

/// The Document laid out for `context`'s page setup under `chosen`.
///
/// The page setup is the dialog's final one and not the seed: a writer who
/// changed the paper or the margins in Page Setup is printing on what they
/// named there.
fn lay_out(window: &Window, context: &gtk::PrintContext, chosen: &Chosen) -> Option<Job> {
    let session = window.session()?;
    let setup = context.page_setup();
    let paper = setup_geometry(
        setup.paper_width(gtk::Unit::Points),
        setup.paper_height(gtk::Unit::Points),
        [
            setup.top_margin(gtk::Unit::Points),
            setup.bottom_margin(gtk::Unit::Points),
            setup.left_margin(gtk::Unit::Points),
            setup.right_margin(gtk::Unit::Points),
        ],
        chosen,
    );
    let built = template::named(session.template().name.as_str());
    // The print context's own Pango context: the Faces are in this process's
    // fontconfig ([`crate::fonts::load_private`]), so any context finds them,
    // and the paginator sets it to lay out in points itself.
    let pango = context.create_pango_context();
    let document = window.document();
    let laid = paginate::lay_out(
        &document,
        &built,
        chosen.toggles,
        paper,
        f64::from(chosen.export.text_size),
        &pango,
    );
    drop(document);
    Some(Job {
        template: built,
        laid,
    })
}

/// The geometry a paper of `width` by `height` points with `margins` — top,
/// bottom, left and right, in points — is paginated under, with `chosen`'s
/// furniture on it.
///
/// A Quill page has one margin and a page setup has four, so the one is the
/// largest of the four: the text then stands inside every margin the writer
/// named, rather than inside the narrowest of them and over one of the others.
fn setup_geometry(width: f64, height: f64, margins: [f64; 4], chosen: &Chosen) -> Geometry {
    Geometry {
        width,
        height,
        margin: margins.into_iter().fold(0.0, f64::max),
        header: chosen.export.header,
        footer: chosen.export.footer,
        title_page: chosen.export.title_page,
    }
}

/// What the operation opens on: the paper `[export]` names, as GTK asks for it
/// ([`paper_name`]), and the margin it names on all four sides
/// ([`Export::margin_points`]).
///
/// One margin on all four sides, because that is the page `[export]` describes
/// and the page a PDF export is laid out on.
fn page_setup(export: &Export) -> gtk::PageSetup {
    let margin = export.margin_points();
    let setup = gtk::PageSetup::new();
    setup.set_paper_size(&gtk::PaperSize::new(Some(paper_name(export.paper))));
    setup.set_top_margin(margin, gtk::Unit::Points);
    setup.set_bottom_margin(margin, gtk::Unit::Points);
    setup.set_left_margin(margin, gtk::Unit::Points);
    setup.set_right_margin(margin, gtk::Unit::Points);
    setup
}

/// What GTK calls `paper`: the PWG standard name of the size, which is how a
/// `GtkPaperSize` is asked for and how a printer names the tray it is in.
///
/// `Auto` is resolved through the desktop's locale here, as an export resolves
/// it ([`Geometry::of`]), so a print and an export of the same Document open on
/// the same paper. [`locale_paper`] answers a size and never
/// `Auto`, so the one recursive arm below terminates.
fn paper_name(paper: Paper) -> &'static str {
    match paper {
        Paper::Auto => paper_name(locale_paper()),
        Paper::A4 => "iso_a4",
        Paper::Letter => "na_letter",
        Paper::Legal => "na_legal",
    }
}

#[cfg(test)]
mod tests {
    use quill_engine::render::Toggles;

    use super::*;
    use crate::export::edited_defaults;

    /// A [`Chosen`] with every piece of furniture on, so that a geometry's
    /// switches are read from it rather than from a default that says no to
    /// all three.
    fn chosen() -> Chosen {
        Chosen {
            export: edited_defaults(|export| {
                export.paper = Paper::A4;
                export.title_page = true;
                export.header = true;
                export.footer = true;
            }),
            toggles: Toggles::default(),
        }
    }

    /// The paper and the margins the print dialog settled on become the
    /// paginator's geometry, the one margin being the largest of the four.
    #[test]
    fn the_page_setups_paper_and_margins_become_the_geometry() {
        let laid = setup_geometry(595.0, 842.0, [56.0, 56.0, 72.0, 20.0], &chosen());
        assert_eq!(
            laid,
            Geometry {
                width: 595.0,
                height: 842.0,
                margin: 72.0,
                header: true,
                footer: true,
                title_page: true,
            }
        );
    }

    /// The furniture on the page is the tab's and not the page setup's: a
    /// writer who turned the footer off in the Quill tab prints no page
    /// numbers however the margins were named.
    #[test]
    fn the_furniture_is_the_tabs_own() {
        let chosen = Chosen {
            export: edited_defaults(|export| export.paper = Paper::A4),
            ..chosen()
        };
        let laid = setup_geometry(612.0, 792.0, [36.0; 4], &chosen);
        assert!(!laid.header && !laid.footer && !laid.title_page);
        assert_eq!(laid.margin, 36.0);
    }

    /// The default `[export]` seeds the page setup with the locale's own paper
    /// and a 20 mm margin, which is what a PDF export of the same Document is
    /// laid out on.
    #[test]
    fn the_default_export_table_seeds_the_page_setup() {
        let export = Export::default();
        let name = paper_name(export.paper);
        assert_eq!(name, paper_name(locale_paper()));
        assert!(
            ["iso_a4", "na_letter"].contains(&name),
            "the locale's paper is one GTK names: {name}"
        );
        assert_eq!(export.margin, 20, "the default margin is 20 mm");
    }

    /// A paper named in `[export]` is the paper GTK is asked for, whatever the
    /// rest of the table says.
    ///
    /// The margin the setup stands on is [`Export::margin_points`] itself
    /// ([`page_setup`]), which no test here converts a second time; a
    /// `GtkPageSetup` cannot be built without a display, so the setup's own
    /// four sides are the Hand test's.
    #[test]
    fn a_named_paper_seeds_the_page_setup() {
        for (paper, name) in [
            (Paper::A4, "iso_a4"),
            (Paper::Letter, "na_letter"),
            (Paper::Legal, "na_legal"),
        ] {
            let export = edited_defaults(|export| {
                export.paper = paper;
                export.margin = 25;
            });
            assert_eq!(paper_name(export.paper), name, "{paper:?}");
        }
    }
}
