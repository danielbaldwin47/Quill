//! The Editor: the text surface a Document is written on.
//!
//! One `GtkTextView` subclass (ADR 0004, `docs/adr/0004-gtktextview-editor.md`).
//! There is no mirror: the widget lays the text out once, so the web app's rule
//! that no per-token style may change glyph advance width stops being an
//! invariant to hold and becomes a property of the Faces. The `GtkTextBuffer`
//! owns the text the writer edits, which is where undo, IME preedit, clipboard,
//! selection and accessibility come from.
//!
//! The type and the page are here: the Face and the size, the leading that
//! belongs to that size, the 64-character measure centred in the window, and
//! the air above and below the column. Every number comes from
//! [`quill_engine::typography`] and none of it is decided here — this file is
//! the widget the numbers are set on. The tag table, the flattened
//! Markup × Focus runs, the hand-drawn caret and the dark palette land on this
//! type in the tickets that follow.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::ops::Range;
use std::time::Duration;

use gtk::gdk;
use gtk::gio;
use gtk::glib;
use gtk::graphene;
use gtk::gsk;
use gtk::pango;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use quill_engine::annotate::live::{self, Furniture, LiveLook, LiveSpan};
use quill_engine::annotate::{self, Painted};
use quill_engine::document::{Document, Edit};
use quill_engine::focus::typewriter::{self, Glide, Hold, Typewriter};
use quill_engine::focus::{self, Focus, LineTiers};
use quill_engine::markdown;
use quill_engine::settings::Face;
use quill_engine::sync;
use quill_engine::theme::{self, Colour, Colours, Role, Scheme};
use quill_engine::typography;

use crate::caret;
use crate::flags;
use crate::ground::Ground;
use crate::tags;

/// The CSS class the Editor's type is named on.
const FACE_CLASS: &str = "quill-editor";

/// How far down the view `--caret` leaves the line the caret is on with
/// Typewriter off: the middle, the one place that does not depend on how long
/// the document is. With Typewriter on the anchor takes its place
/// ([`Editor::reveal_caret`]).
const CARET_LINE: f64 = 0.5;

/// How far below the baseline Pango reports the ink's own baseline is drawn,
/// in the pixels the widget lays out in.
///
/// [`caret::band_top`] hangs the band from the baseline, and the share it
/// hangs it by is the oracle's, measured in a browser. The two agree about the
/// type — the `caret` state and the oracle's are the same Face at the same
/// size, and the ink in both is 37 device pixels from ascender to descender —
/// and disagree about where under it the baseline falls: measured off the
/// judged shots at scale 2, the browser's is 15.25 logical pixels below the
/// top of the ink and Pango's is 14.75. Half a pixel, and it is the whole of
/// the difference between a band at 20 above the ascender and 15 below and the
/// 19 and 16 iA draws.
///
/// A pixel of granularity rather than a number about the type, most likely:
/// Pango hints the baseline onto a whole layout pixel and a browser at device
/// ratio 2 onto a whole device pixel, which is half of one. So it is a length
/// in pixels and not a share of the em — but it is *measured*, not derived,
/// and if the Type Piece ever moves the body size it is measured again. The
/// `caret` state is what says whether it is still true.
const BASELINE_DRIFT: f64 = 0.5;

/// The most rows of selection painted in one frame: `MAX_ROWS` in
/// `legacy/app/js/caret.js`.
///
/// A backstop and not the thing that keeps the paint bounded — the viewport
/// clip in [`Editor::selection`] does that, as it does in the oracle, which
/// clips before it counts. What is left for a cap is a window tall enough, or
/// a pitch small enough, that a screenful is still hundreds of rows.
const MAX_ROWS: usize = 400;

/// The scale the ladder is asked at for anything GTK lays the page out from.
///
/// A `GtkTextView`'s margins and its three leadings are logical pixels — GTK
/// applies the surface's scale factor itself, on the way to the glass — so the
/// pitch that feeds them is the ladder at scale 1. The surface's own scale is
/// for the caret alone, whose machine measures in device pixels because the
/// snap that keeps the bar's edges hard is a snap onto one of those.
const LAYOUT_SCALE: f64 = 1.0;

/// How far past the viewport the selection's rows are still built, in rows.
///
/// `M.pitch * 6` either side of the scroller in `drawSelection`. The slack is
/// what keeps a scroll from showing the seam: the rows a frame is about to
/// need are already drawn when it arrives.
const SELECTION_SLACK: f64 = 6.0;

/// How wide a bullet's dot is drawn, as a share of the em.
///
/// A typographic `•` is about a third of the em across in a text face, and
/// this is furniture standing where one would: measured off the Design
/// oracle's own list marker and rounded to a number a reader of this file can
/// hold. See [`Editor::draw_bullet`] for why it is drawn rather than set.
const BULLET: f64 = 0.3;

/// How far above the baseline the middle of a lower-case letter sits, as a
/// share of the em.
///
/// The height a bullet and a task box are centred on, because that is where a
/// reader's eye finds the middle of a line of prose: half of an x-height, and
/// an x-height is around half an em in every one of the six Faces. Nothing is
/// measured off it that a fraction of a pixel would change.
const X_HEIGHT: f64 = 0.25;

/// How wide a task box is drawn, as a share of the em.
///
/// A shade under the x-height's own square, so that the box reads as one of
/// the line's letters rather than as a panel dropped into it.
const CHECKBOX: f64 = 0.58;

/// The weight ink is set at on paper: `--ink-weight: 415` in
/// `legacy/app/css/type.css`, a little heavier than Regular because a light
/// ground eats stems. The Faces are variable, and none of the three moves a
/// glyph's advance across the weight axis (`spike/gtk4-editor/RESULTS.txt`).
pub(crate) const INK_WEIGHT: u32 = 415;

/// The OpenType features prose is set without.
///
/// Ligatures, contextual alternates and kerning all make the text narrower
/// than the cell grid it is counted on, and the measure is counted in cells.
/// One list, because Pango and GTK's stylesheet spell a feature differently
/// and a page measured with kerning and drawn without it is measured wrong.
const FEATURES: [&str; 4] = ["liga", "clig", "calt", "kern"];

/// How many frames `--scroll` holds the view where it was asked for.
const SCROLL_FRAMES: u32 = 8;

/// What the empty page says on its first line, in the oracle's words
/// (`legacy/app/css/page.css` `content: "Start writing…"`).
const PLACEHOLDER: &str = "Start writing…";

/// The most frames a `--caret` reveal is asked again for while GTK validates
/// the layout it is resolved against ([`Editor::reveal_caret`]). A bound, not
/// a duration: the hold ends the frame the row stops moving and is on the
/// glass. Measured Live on the scale-2 headless output, that is the third
/// frame on `ref/short.md` and `ref/sample.md`, and the thirteenth on the
/// 10,062-word `shots/latency/doc10k.md` at `--caret 26000` — the layout had
/// its real height by the third, and the rest is the scroll GTK animates
/// into place over 200 ms, which the hold leaves to finish once the row is
/// on the glass.
const REVEAL_FRAMES: u32 = 120;

/// How many frames running the layout has to hold still for before a reveal
/// counts it validated ([`Landing::stood`]). GTK validates a long Document a
/// stretch at a time from an idle source below the frame clock's redraw, so
/// a frame the source got no turn in reads still with validation to come;
/// one more frame of the same reading is what tells the two apart.
const STILL_FRAMES: u32 = 2;

/// A cross-fade in flight: the dim on its way from where it was to where the
/// caret has just put it.
///
/// Held whole rather than as a fade per run, because every run of one tier
/// change moves together: they left at the same frame and they arrive at the
/// same one, and a writer reading a page whose sentences arrived at slightly
/// different moments would be reading a page that shimmers.
/// `pub` because it is a field of the `pub` state struct the subclass macro
/// generates, and a narrower visibility is a `private_interfaces` warning; the
/// module it lives in is private, so nothing outside this file can name it.
pub struct Fade {
    /// The frame it started in, on the frame clock's clock ([`Editor::now`]).
    ///
    /// How long it runs is not held beside it: a fade that is running at all
    /// runs for [`theme::FADE_MS`], because the only other length
    /// [`theme::fade_ms`] answers with is no fade, and
    /// [`Editor::begin_fade`] turns that away before there is anything to hold.
    started: i64,
    /// The lines it is moving, so that a fade cut short can be settled by
    /// drawing them again rather than by guessing what colour they were on
    /// their way to.
    lines: Vec<Range<usize>>,
    /// The runs it moves, each with the colour it is showing this frame.
    runs: Vec<FadeRun>,
}

/// One run of a cross-fade: where it is, the two tiers it lies between, and
/// what is on it now.
struct FadeRun {
    /// The buffer's own offsets, taken once ([`tags::offsets_of`]).
    at: Range<i32>,
    /// The colour the tier it is leaving drew it in.
    from: Colour,
    /// The colour the tier it is arriving at draws it in.
    to: Colour,
    /// What is on those offsets now, so that the frame after can take it off
    /// again and leave one foreground tag on the run.
    shown: Colour,
}

/// A Typewriter glide in flight: the engine's ease, and the frame it started
/// in on the frame clock's clock ([`Editor::now`]), which is the one thing
/// the engine leaves to the widget.
///
/// `pub` for the reason [`Fade`] is.
#[derive(Clone, Copy)]
pub struct Travel {
    /// Where the view is going and how it gets there.
    glide: Glide,
    /// The frame it started in.
    started: i64,
}

/// The lines the bytes an edit put in lie on; none for a deletion.
///
/// Drawn again with Focus on ([`Editor::retag`]): the buffer gives an
/// inserted byte the tags of the byte before it, and that byte is the dim of
/// the sentence before when the writer types at the start of their own — so
/// the typed bytes' lines are drawn from the tiers whatever [`focus::changed`]
/// says of them, and without a fade: the tiers say the typed text is bright,
/// and it is bright at once. With Focus off the buffer's own ink is right and
/// the Markup tags are [`Edit::lines`]'s to redraw.
fn typed_lines(document: &Document, edit: &Edit) -> Option<Range<usize>> {
    let splice = &edit.splice;
    if splice.inserted == 0 {
        return None;
    }
    let first = document.place(splice.at.start).line;
    let last = document.place(splice.at.start + splice.inserted - 1).line;
    Some(first..last + 1)
}

/// The runs a tier change moved, and the two colours each moves between.
///
/// `before` and `after` are the same bytes painted under the tiers on either
/// side of the change, so they cover the same span and differ only where a
/// colour did. Every overlap of one with the other is a stretch drawn in one
/// colour before and one colour after; the ones where those are the same
/// colour are not moving and are left out, which is what keeps a fade to the
/// sentence that changed rather than the block around it.
///
/// The walk is every pair of the two lists, bounded by the runs of one block
/// of prose: [`focus::changed`] answers with the lines of the caret's block and
/// no others, so both lists are that block's runs and neither is the
/// manuscript's.
fn faded(
    before: &[Painted],
    after: &[Painted],
    offsets: impl Fn(&Range<usize>) -> Range<i32>,
) -> Vec<FadeRun> {
    let mut runs = Vec::new();
    for now in after {
        for was in before {
            let both = now.at.start.max(was.at.start)..now.at.end.min(was.at.end);
            if both.start >= both.end || was.paint.colour == now.paint.colour {
                continue;
            }
            runs.push(FadeRun {
                at: offsets(&both),
                from: was.paint.colour,
                to: now.paint.colour,
                shown: now.paint.colour,
            });
        }
    }
    runs
}

/// How many milliseconds a frame at `now` is into something that started at
/// `started`, both on the frame clock's microseconds.
///
/// A frame that arrives before the one the fade started in — which a clock
/// stepped by hand can hand out — is nothing elapsed rather than a fade run
/// backwards.
fn millis_since(started: i64, now: i64) -> u32 {
    u32::try_from((now - started).max(0) / 1_000).unwrap_or(u32::MAX)
}

mod imp {
    use std::cell::{Cell, RefCell};

    use gtk::glib;
    use gtk::subclass::prelude::*;
    use quill_engine::settings::Face;

    use quill_engine::focus::{Focus, LineTiers};

    use crate::ground::Ground;

    use super::{Fade, Travel};
    use crate::caret;
    use quill_engine::focus::typewriter::Typewriter;

    #[derive(Default)]
    pub struct Editor {
        /// The Face this Editor is set in.
        pub face: Cell<Face>,
        /// Which of the type ladder's fourteen steps the Editor is set at.
        pub step: Cell<u32>,
        /// The ground this Editor is painting on, with the table every colour
        /// it draws is read off. Held here rather than asked of the session
        /// per frame because the caret asks for it on every frame it is
        /// visible, and a `Cell` of a `Copy` table is a read and no borrow.
        pub ground: Cell<Ground>,
        /// The page as it was last laid out. Setting a margin queues another
        /// allocation, so an allocation that does not move the page must not
        /// set it again.
        pub laid_out: Cell<Option<super::Page>>,
        /// True while the buffer is being filled with a Document rather than
        /// written in. The fill is a delete and an insert like any other, and
        /// the engine's copy is already what they would produce, so the
        /// handlers watching for a keystroke have to be able to tell them
        /// apart from one.
        pub loading: Cell<bool>,
        /// The blink and the glide. The widget feeds it what happened and
        /// asks it where the bar is and how opaque; it decides neither.
        pub caret: Cell<caret::Caret>,
        /// The row band the bar spans, and the air the leading leaves over
        /// the ink inside it. Both are functions of the type, so both are
        /// kept with the type: placing the bar on the keystroke path must not
        /// cost a row measured again.
        pub pitch: Cell<u32>,
        /// How much Focus leaves lit, held here for the same reason as the
        /// ground: it is read at every draw, and it is the value the pair the
        /// session holds becomes ([`focus::Focus::at`]).
        pub focus: Cell<Focus>,
        /// What Focus lit when the tiers were last worked out, so that the
        /// next caret move can be told which lines changed and redraw only
        /// those ([`focus::changed`]). Empty with Focus off.
        pub tiers: RefCell<Vec<LineTiers>>,
        /// Whether Live is on: the markup rendered in place rather than
        /// written out. Held beside the Focus it reads like — a mode read at
        /// every draw, and a `Cell` is a read and no borrow.
        pub live: Cell<bool>,
        /// The lines Live last left unfolded, so that a caret move can fold
        /// the block it left in the same pass that unfolds the one it entered
        /// ([`Editor::refold`]). `None` with Live off, and while it has not
        /// been worked out yet.
        pub open: RefCell<Option<std::ops::Range<usize>>>,
        /// Whether a pointer button is down. The fold stands still while it
        /// is: folding takes bytes off the page, and text that moved under a
        /// held button turns a click into a drag across whatever slid past
        /// ([`Editor::released`](super::Editor::released)).
        pub held: Cell<bool>,
        /// Whether a refold was asked for while a button was down, and so is
        /// owed at the release.
        pub fold_owed: Cell<bool>,
        /// Where the fold was last committed to putting the writer, which is
        /// what every draw between two refolds paints from
        /// ([`Editor::painting`](super::Editor::painting)). The live caret
        /// would fold the page to a place the fold has not moved to yet, which
        /// is the same reflow under a held button by another road.
        pub writer: RefCell<std::ops::Range<usize>>,
        /// What Live's fold left standing in the marker cells, in the
        /// buffer's own offsets, top to bottom
        /// ([`Editor::refurnish`](super::Editor::refurnish)). Empty with Live
        /// off. Held rather than asked for at the draw because the snapshot
        /// has no Document to ask: it is worked out on the passes that have
        /// one and read by the frame that paints it.
        pub furniture: RefCell<Vec<super::Furnishing>>,
        /// How far the baseline sits below the top of the box
        /// `iter_location` answers with, which is what the bar's band is
        /// anchored to. See [`Editor::bar`], and [`caret::band_top`] for why
        /// the baseline and not the top of the box.
        pub baseline: Cell<f64>,
        /// The bar last handed to the machine, so that a relayout which moved
        /// nothing can be told from one that moved the row.
        pub bar: Cell<Option<caret::Bar>>,
        /// What last drove the widget, which is half of a move's kind.
        pub last: Cell<caret::Source>,
        /// The frame the buffer last changed in, on the frame clock's clock.
        /// A caret move in that same frame is the move that change made.
        pub edited: Cell<Option<i64>>,
        /// Whether the frame-clock callback is attached.
        pub ticking: Cell<bool>,
        /// The cross-fade in flight, if the dim is on its way from one tier to
        /// the other. `None` between fades, which is nearly always: a fade is
        /// [`quill_engine::theme::FADE_MS`] long and only a tier that moved
        /// starts one.
        pub fade: RefCell<Option<Fade>>,
        /// Whether the cross-fade's own frame-clock callback is attached. Its
        /// own rather than the caret's, so that a page fading while the caret
        /// is held still asks for frames and a caret blinking over a settled
        /// page does not repaint the text.
        pub fading: Cell<bool>,
        /// Whether this launch is `--deterministic`, which is one of the two
        /// things that leave no fade
        /// ([`quill_engine::theme::fade_ms`]).
        pub deterministic: Cell<bool>,
        /// Whether Typewriter is on, and where it holds the caret's row
        /// ([`quill_engine::focus::typewriter::hold`]).
        pub typewriter: Cell<Typewriter>,
        /// Whether `--scroll` has put the view where a judged state asked
        /// for, which an allocation must not then hold the row against
        /// ([`Editor::hold_row`]): a state naming both `--typewriter` and
        /// `--scroll` means both.
        pub pinned: Cell<bool>,
        /// The frame the pointer was last pressed or released in, on the
        /// frame clock's clock, so that a caret move can be told how long ago
        /// that was ([`quill_engine::focus::typewriter::POINTER_MS`]). `None`
        /// until the first press.
        pub pressed: Cell<Option<i64>>,
        /// Whether the edit now reaching the buffer is a task box being
        /// flipped by a press ([`Editor::press`](super::Editor::press)) rather
        /// than something the writer typed. The caret's machine stands down
        /// while it is set: the caret did not move and was not written at, so
        /// nothing holds the blink and nothing brings the row back into the
        /// band. Named apart from `ticking`, which is the frame clock's.
        pub pressing_box: Cell<bool>,
        /// The Typewriter glide in flight, if the view is on its way to where
        /// the row is held. `None` between glides, and always under
        /// `--deterministic`, which jumps instead.
        pub glide: Cell<Option<Travel>>,
        /// Whether the glide's own frame-clock callback is attached. Its own
        /// for the reason the fade's is: a page scrolling under a still caret
        /// asks for frames, and a blinking caret over a settled page does not
        /// move the view.
        pub gliding: Cell<bool>,
        /// Whether the last placement is still owed the scroll that shows
        /// where the caret went. Every placement sets it to what it asked
        /// for, so a placement that wants no reveal calls off one still
        /// waiting rather than leaving it to be paid against a page it was
        /// never asked for. See [`Editor::reveal_caret`].
        pub reveal_owed: Cell<bool>,
        /// The one-shot that brings the blink back when the quiet after a
        /// move or an edit runs out: no frame is asked for inside it, so
        /// something outside the frame clock has to ask for the one that ends
        /// it.
        pub resume: RefCell<Option<glib::SourceId>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Editor {
        const NAME: &'static str = "QuillEditor";
        type Type = super::Editor;
        type ParentType = gtk::TextView;
    }

    impl ObjectImpl for Editor {
        /// Takes the blink's one-shot off the main loop with the widget.
        ///
        /// It holds a weak reference, so left behind it would fire into
        /// nothing rather than into a freed Editor — but a source with
        /// nothing left to do does not belong on the loop, and an Editor is
        /// disposed of whenever a window closes.
        fn dispose(&self) {
            if let Some(resume) = self.resume.take() {
                resume.remove();
            }
        }
    }

    impl WidgetImpl for Editor {
        /// The measure is centred in the room there turned out to be, and the
        /// air below the column is a share of the window's height, so both are
        /// answered here rather than guessed before the window has a size.
        fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
            self.parent_size_allocate(width, height, baseline);
            self.obj().lay_out(width, height);
            // The bar's row is the row the text wrapped to, and where the text
            // wrapped is not known until there is a width to wrap it in. A
            // caret placed before that — by `--caret`, or by the Document
            // being shown — was placed on a layout GTK had not laid out yet,
            // so it is placed again here, on the one it has. The machine drops
            // a placement that moves nothing, so the allocations that do not
            // move the page cost nothing either.
            self.obj().caret_settled();
            // And the scroll that shows the row it is on, which could not be
            // resolved against a layout that did not exist yet either (#148).
            self.obj().reveal_caret();
            // And Typewriter's hold on the row, which is a share of a viewport
            // this allocation may just have changed the height of.
            self.obj().hold_row();
        }
    }

    impl TextViewImpl for Editor {
        /// The selection, Live's furniture and the caret, around the text.
        ///
        /// The fill goes under the glyphs and the caret over them. Never both
        /// at once: the caret is out for as long as a selection stands
        /// ([ADR 0014](../../docs/adr/0014-a-selection-is-a-fill-and-nothing-else.md)),
        /// so each layer is the other's idle frame.
        ///
        /// The fill has to be under: the ink of a held word is the ink of any
        /// other word, and a fill painted over it would tint it. The caret has
        /// to be over. A bar is several device pixels wide
        /// ([`typography::caret_width`]) and a glyph's left
        /// side bearing can be less than that, so a bar standing on a boundary
        /// meets the ink of the letter after it; drawn under, the letter is
        /// rasterised straight through the bar and the two read as one mark,
        /// which is what #147 reported as the caret "sitting on" the glyph.
        /// Both the Parity oracle and iA Writer paint theirs over the ink —
        /// `legacy/app/css/caret.css` puts `#caret-layer` above `#mirror`.
        ///
        /// This costs the layer above the text, where a future Annotator's
        /// marks were going to go; they will have to sort against the caret
        /// rather than assume the layer to themselves. GTK's own caret is not
        /// here to be drawn over either way: the stylesheet paints it
        /// transparent, so it is laid out and never seen.
        ///
        /// Both layers are snapshotted in buffer coordinates, which is what
        /// `iter_location` answers in, so nothing is translated on the way.
        fn snapshot_layer(&self, layer: gtk::TextViewLayer, snapshot: gtk::Snapshot) {
            if layer == gtk::TextViewLayer::BelowText {
                self.obj().draw_placeholder(&snapshot);
                self.obj().draw_selection_fill(&snapshot);
            }
            if layer == gtk::TextViewLayer::AboveText {
                // Over the glyphs, with the caret and under it: furniture is
                // ink standing in the cells a marker left, so it takes the
                // ink's own side of the selection's fill, and the caret is
                // drawn over it as it is drawn over a letter (#274).
                self.obj().draw_furniture(&snapshot);
                self.obj().draw_caret(&snapshot);
            }
        }
    }
}

glib::wrapper! {
    /// The Editor widget.
    pub struct Editor(ObjectSubclass<imp::Editor>)
        @extends gtk::TextView, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Scrollable;
}

/// The selection, as the boxes that draw it.
///
/// Built by [`Editor::selection`] and drawn by [`Editor::draw_selection`], in
/// device pixels like everything else the caret's layer holds.
struct Selection {
    /// One fill per display row the selection covers, top to bottom.
    ///
    /// The whole of it. A selection carries no bars at either end and no
    /// caret: see [ADR 0014](../../docs/adr/0014-a-selection-is-a-fill-and-nothing-else.md).
    rows: Vec<caret::Bar>,
}

/// One piece of furniture Live left standing, and where the buffer holds it.
///
/// [`quill_engine::annotate::live::Furniture`] as the widget can use it
/// without a Document in hand: the byte ranges are the buffer's own offsets,
/// and a link's destination is already resolved to the address a click opens.
/// Both crossings are made where there is a Document to make them
/// ([`Editor::refurnish`]), because the snapshot and the pointer have none.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Furnishing {
    /// The cells it stands in, in the buffer's offsets: a marker's, folded to
    /// its own ground so its advance is still there to stand in
    /// ([`tags`]), or a link's own words.
    at: Range<i32>,
    /// What stands there.
    what: Furnish,
}

impl Furnishing {
    /// The cells a task box stands in, in the buffer's offsets, if this is a
    /// task box.
    fn box_cells(&self) -> Option<Range<i32>> {
        let Furnish::Checkbox { box_at, .. } = &self.what else {
            return None;
        };
        Some(self.at.start + box_at.start..self.at.start + box_at.end)
    }
}

/// What one [`Furnishing`] is.
///
/// [`Furniture::Fence`] has no member here: a fence's furniture is the Well
/// the code already stands on, so there is nothing left to draw where the
/// backticks were, and nothing to click.
#[derive(Clone, Debug, Eq, PartialEq)]
enum Furnish {
    /// A bullet item's dot.
    Bullet,
    /// An ordered item's number as the source counted it, with the byte it
    /// wrote after the count: `.` or `)`.
    Number {
        /// The count the marker counts with.
        count: u32,
        /// `.` or `)`, as the source wrote it.
        delimiter: char,
    },
    /// A task item's box, ticked or not, and the `[ ]` or `[x]` a press on it
    /// rewrites — which is the one cell inside the brackets and not the whole
    /// marker, so that the item's words never move under the writer's finger.
    Checkbox {
        /// The box's own cells, counted from the first of the furnishing's.
        ///
        /// Relative, so that nothing but [`Editor::refurnish`] has to cross
        /// into the buffer's offsets: a task item's marker is ASCII from end
        /// to end — its indent, its bullet and its brackets — so the buffer
        /// counts those cells one for one with the Document's bytes and the
        /// two offsets add.
        box_at: Range<i32>,
        /// Whether it is ticked.
        checked: bool,
    },
    /// The rule a thematic break draws.
    Hairline,
    /// A link's words, and the address Ctrl+click opens.
    Link {
        /// Resolved: an inline link's own destination, or what a reference
        /// link's label is defined as ([`markdown::reference`]).
        destination: String,
    },
}

/// One display row a [`Furnishing`] runs across, in the buffer coordinates
/// `iter_location` answers in.
#[derive(Clone, Copy, Debug)]
struct Cells {
    /// The left edge of the first cell on the row.
    x: f64,
    /// The top of the row's box.
    y: f64,
    /// How far the cells reach across it.
    w: f64,
    /// The row's own box, as the view laid it out: what [`Editor::band`]
    /// reads a row's height from under Live.
    row: gdk::Rectangle,
}

impl Editor {
    /// An Editor with an empty buffer, at the default type.
    ///
    /// The Face and size a launch is actually running are handed to
    /// [`Editor::set_type`] as the window is built; these are what the widget
    /// is until then, so that it is never laid out on a size of zero.
    #[must_use]
    pub fn new() -> Self {
        let editor: Self = glib::Object::new();
        // Prose wraps, and it wraps inside a word when a word is longer than
        // the measure, because the measure is the promise and a word that
        // breaks the column breaks the page.
        editor.set_wrap_mode(gtk::WrapMode::WordChar);
        editor.add_css_class(FACE_CLASS);
        // GTK's caret is a one-pixel line at the font's height on a system
        // timer, and ours is the whole point of the Piece. It is hidden by the
        // stylesheet (`caret-color: transparent`, in [`stylesheet`]) and not
        // by `cursor-visible`: with that property off, `GtkTextView` turns
        // every `move-cursor` into a scroll of the viewport instead of a move
        // of the insert mark, and the arrows, Home, End and Shift with any of
        // them stop doing anything (#220). Its blink is off as well, on the
        // display's settings at startup, so the unseen bar asks for no frames.
        editor
            .imp()
            .step
            .set(quill_engine::settings::default_step());
        editor.watch_caret();
        editor
    }

    /// Puts the four things the caret's machine is fed onto the widget.
    ///
    /// GTK says that the insert mark moved but never what moved it, so the
    /// two controllers are here to answer that and nothing else. Neither
    /// binds a chord and neither claims an event: they run in the capture
    /// phase, before GTK's own handling turns the press into a caret move, and
    /// then let it through. They are the exception named by #119's rule that
    /// only `quill::chrome` binds keys.
    fn watch_caret(&self) {
        let clicks = gtk::GestureClick::new();
        clicks.set_propagation_phase(gtk::PropagationPhase::Capture);
        clicks.connect_pressed(glib::clone!(
            #[weak(rename_to = editor)]
            self,
            move |clicks, _, x, y| {
                editor.imp().last.set(caret::Source::Pointer);
                // The fold stands still until the button comes up: a block
                // folding under a held pointer moves the text the release will
                // be read against ([`Editor::released`]).
                editor.imp().held.set(true);
                // Stamped here, in the capture phase, so that the caret move
                // the press makes finds the press already on record.
                editor.imp().pressed.set(Some(editor.now()));
                // And the one press that is not a caret move at all: on a task
                // box, or on a link's words with Ctrl down. The capture phase
                // is where it has to be answered, because it is answered by
                // claiming the press before GTK turns it into a move.
                editor.press(clicks, x, y);
            },
        ));
        // And at the release, as the oracle stamps both (`focus.js:266-267`):
        // a drag held longer than the pointer window still ends in a nudge.
        clicks.connect_released(glib::clone!(
            #[weak(rename_to = editor)]
            self,
            move |_, _, _, _| editor.imp().pressed.set(Some(editor.now())),
        ));
        self.add_controller(clicks);

        // A hand on the wheel takes the view from a glide in flight
        // (`focus.js:268-269`): the next frame finds nothing to carry and
        // drops the tick.
        let wheel = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::VERTICAL);
        wheel.set_propagation_phase(gtk::PropagationPhase::Capture);
        wheel.connect_scroll(glib::clone!(
            #[weak(rename_to = editor)]
            self,
            #[upgrade_or]
            glib::Propagation::Proceed,
            move |_, _, _| {
                editor.imp().glide.set(None);
                glib::Propagation::Proceed
            },
        ));
        self.add_controller(wheel);

        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        keys.connect_key_pressed(glib::clone!(
            #[weak(rename_to = editor)]
            self,
            #[upgrade_or]
            glib::Propagation::Proceed,
            move |_, _, _, _| {
                editor.imp().last.set(caret::Source::Key);
                glib::Propagation::Proceed
            },
        ));
        self.add_controller(keys);

        let buffer = self.buffer();
        buffer.connect_insert_text(glib::clone!(
            #[weak(rename_to = editor)]
            self,
            move |_, _, _| editor.caret_edit_began(),
        ));
        buffer.connect_delete_range(glib::clone!(
            #[weak(rename_to = editor)]
            self,
            move |_, _, _| editor.caret_edit_began(),
        ));
        // After, and it is the only handler here that waits for another. The
        // window's own `changed` splices the Document and retags the lines the
        // edit touched, and a retag can put a run in another Face — so a
        // column read before it is a column measured on advances the writer
        // never sees. The rest of these read the text alone and do not care.
        let watcher = self.downgrade();
        buffer.connect_closure(
            "changed",
            true,
            glib::closure_local!(move |_: gtk::TextBuffer| {
                if let Some(editor) = watcher.upgrade() {
                    editor.caret_edited();
                }
            }),
        );
        buffer.connect_mark_set(glib::clone!(
            #[weak(rename_to = editor)]
            self,
            move |buffer, _, mark| {
                let insert = mark == &buffer.get_insert();
                if insert {
                    editor.caret_moved();
                }
                // The other end moves on its own through a shift-drag and a
                // shift-arrow, and it is half of what there is to draw.
                if insert || mark == &buffer.selection_bound() {
                    editor.caret_selected();
                }
            },
        ));
    }

    /// Sets the Editor in `face` at `step` of the type ladder and lays the
    /// page out again.
    ///
    /// Everything downstream of the size moves with it: a size without the
    /// leading that belongs to it is half a decision, and a leading without
    /// the measure that belongs to it is the other half. A step names all
    /// three at once, which is the point of a ladder measured off the app
    /// rather than a curve fitted to it.
    pub fn set_type(&self, face: Face, step: u32) {
        self.imp().face.set(face);
        self.imp().step.set(step);
        self.restyle();
    }

    /// The ground this Editor opens on.
    ///
    /// Set while the window is being built and before it is realised, so that
    /// the first frame is already in the right colours: a dark desktop that
    /// saw one light frame has seen a flash, and a flash is the one thing
    /// #39's story 4 is about. Nothing is redrawn here because nothing has
    /// been drawn — the switch afterwards is [`Editor::set_ground`], which has
    /// a Document to retag and a frame to invalidate.
    pub fn open_on(&self, ground: Ground) {
        self.imp().ground.set(ground);
    }

    /// How much Focus leaves lit when this Editor opens.
    ///
    /// Set beside [`Editor::open_on`] and for the same reason: `--focus` names
    /// a state the first frame is meant to show, and a frame that showed the
    /// undimmed page first would be a flash of the wrong one. Nothing is
    /// redrawn here because nothing has been drawn — the tiers themselves are
    /// worked out by [`Editor::show_document`], which is the first draw and the
    /// first thing with a caret to work them out from.
    pub fn open_focused_on(&self, focus: Focus) {
        self.imp().focus.set(focus);
    }

    /// Whether Live is on when this Editor opens.
    ///
    /// Beside [`Editor::open_focused_on`] and for its reason: `--live` names a
    /// state the first frame is meant to show, and a frame that showed the
    /// markers before folding them would be a flash of the wrong page. The
    /// fold itself is worked out by [`Editor::show_document`], which is the
    /// first draw and the first thing with a caret to work it out from.
    pub fn open_live_on(&self, live: bool) {
        self.imp().live.set(live);
    }

    /// Everything but the text that decides how this Editor draws.
    ///
    /// `tiers` is borrowed rather than read from the Editor here, because every
    /// caller already holds the borrow: the tiers are worked out and drawn in
    /// one breath, and a second borrow inside a draw is a second chance for
    /// them to be the tiers of a different caret.
    ///
    /// Live's half is the writer the fold was last committed at
    /// ([`Editor::refold`]) rather than the caret as the buffer now holds it:
    /// a draw that folded to a caret the fold has not moved to yet would take
    /// bytes off the page between the fold's own two passes, which is the
    /// reflow under a held button that [`Editor::released`] exists to prevent.
    fn painting<'a>(&self, tiers: &'a [LineTiers]) -> tags::Painting<'a> {
        let writer = self.imp().writer.borrow().clone();
        self.painting_at(&writer, tiers)
    }

    /// [`Editor::painting`], for a writer whose place the buffer does not hold
    /// yet.
    ///
    /// The same reason [`Editor::retier_at`] takes one: a Document is drawn
    /// before its cursor is placed, and Live's fold is a judgement about where
    /// the writer is, so the place it opens at is named rather than read.
    fn painting_at<'a>(&self, at: &Range<usize>, tiers: &'a [LineTiers]) -> tags::Painting<'a> {
        tags::Painting {
            face: self.imp().face.get(),
            colours: self.colours(),
            focus: self.imp().focus.get(),
            tiers,
            live: self.imp().live.get().then_some(tags::Writer {
                start: at.start,
                end: at.end,
            }),
        }
    }

    /// Works out the tiers for the caret where it now is, and answers with the
    /// lines whose tier changed.
    ///
    /// The whole of Focus on the keystroke path: the engine reads the caret's
    /// block and nothing else ([`focus::tiers`]), and the answer is compared
    /// with the one before it so that a move redraws the lines it moved the
    /// dim across and no others. With Focus off there is nothing to work out
    /// and nothing changes, which is what leaves the Focus-off page byte for
    /// byte the page it was before Focus existed.
    fn retier(&self, document: &Document) -> Vec<Range<usize>> {
        let at = self.caret_bytes(document);
        self.retier_at(document, &at)
    }

    /// The writer's selection as the buffer holds it, in the Document's
    /// bytes: an empty range is the caret.
    fn caret_bytes(&self, document: &Document) -> Range<usize> {
        let buffer = self.buffer();
        let (from, to) = buffer.selection_bounds().unwrap_or_else(|| {
            let at = buffer.iter_at_mark(&buffer.get_insert());
            (at, at)
        });
        tags::offset_of(document, &from)..tags::offset_of(document, &to)
    }

    /// The tiers for a caret at `at`, by line, as Focus stands: none while it
    /// is off.
    fn tiers_at(&self, document: &Document, at: &Range<usize>) -> Vec<LineTiers> {
        let focus = self.imp().focus.get();
        focus::tiers_by_line(document, &focus::tiers(document, at, focus))
    }

    /// [`Editor::retier`], for a caret the buffer does not hold yet.
    ///
    /// A Document is drawn before its cursor is placed, because placing one
    /// measures a column and a column is measured on the advances the draw
    /// puts there. So the caret it opens at is named rather than read.
    fn retier_at(&self, document: &Document, at: &Range<usize>) -> Vec<Range<usize>> {
        if matches!(self.imp().focus.get(), Focus::Off) {
            return Vec::new();
        }
        let after = self.tiers_at(document, at);
        let moved = focus::changed(&self.imp().tiers.borrow(), &after);
        self.imp().tiers.replace(after);
        moved
    }

    /// Draws the lines the caret's move took the dim off and put it on.
    ///
    /// The caret feed Focus listens to. A move that changed no line's tier —
    /// which is most of them, since a sentence is many keystrokes wide — draws
    /// nothing at all.
    pub fn refocus(&self, document: &Document) {
        self.refold(document);
        let before = self.imp().tiers.borrow().clone();
        let moved = self.retier(document);
        if moved.is_empty() {
            // No tier moved, so nothing is drawn again and a fade already in
            // flight is left alone: a caret walking along the sentence it is
            // in is most of what a caret does, and settling the fade on each
            // step of it would be a fade that never finished.
            return;
        }
        self.settle_fade(document);
        self.redraw(document, &moved);
        self.begin_fade(document, &before, &moved);
    }

    /// Folds the block the writer left and unfolds the one they are in, in one
    /// pass.
    ///
    /// The caret feed Live listens to, beside the one Focus listens to, and
    /// the two are drawn in the same breath for the same reason
    /// [`Editor::retag`] draws the edit and the dim together: a fold and an
    /// unfold that arrived a frame apart would be a flash of a page with two
    /// blocks open, or none.
    ///
    /// Nothing but the two blocks is drawn, which is what keeps Live on the
    /// keystroke path: a caret walking along the sentence it is in leaves the
    /// same lines open and draws nothing at all.
    ///
    /// Nothing at all is drawn while a pointer button is down: the fold is put
    /// on the slate and paid at the release ([`Editor::released`]), because a
    /// block folding under a held button moves the text out from under the
    /// pointer and GTK reads the release's own coordinates. A key press is not
    /// a held button and folds at once, as it always did.
    fn refold(&self, document: &Document) {
        if !self.imp().live.get() {
            return;
        }
        if self.imp().held.get() {
            self.imp().fold_owed.set(true);
            return;
        }
        let at = self.caret_bytes(document);
        let now = self.open_lines(document, &at);
        // Committed whether or not the open lines moved, because it is what
        // every draw until the next refold folds to: an edit that left the
        // same lines open still moved the bytes the fold is written in.
        self.imp().writer.replace(at);
        let was = self.imp().open.replace(Some(now.clone()));
        let Some(lines) = refolded(was, now) else {
            return;
        };
        self.redraw(document, &lines);
        // The block the writer left is furnished and the one they entered is
        // not, so the furniture moves with the fold and in the same pass, for
        // the reason the fold moves in one: two frames apart is a flash of a
        // dot standing on the `-` it replaced.
        self.refurnish(document);
    }

    /// A pointer button came up: the fold moves now, if it was asked to while
    /// the button was down.
    ///
    /// The other half of [`Editor::refold`]'s standing still. The writer's
    /// range at the release is the selection if the press turned into a drag,
    /// so every block the drag crossed unfolds together and none of them moved
    /// under the pointer on the way.
    ///
    /// Heard by the window, from a legacy event controller rather than a
    /// gesture: the press on a task box claims its sequence and so does GTK's
    /// own selection drag, a claimed sequence denies every other gesture on
    /// the widget, and a denied gesture's `released` never fires — which would
    /// leave the fold held down for good.
    pub fn released(&self, document: &Document) {
        self.imp().held.set(false);
        if self.imp().fold_owed.take() {
            self.refold(document);
        }
    }

    /// The lines of the parts the writer's range reaches: what Live leaves
    /// unfolded.
    ///
    /// Live's part rather than the Document's block ([`live::part_at`]), which
    /// is the same thing for everything but a list: a list is one block and
    /// many parts, and a caret walking from one item to the next left the
    /// block it was in, so a fold keyed on the block saw nothing change and
    /// neither item was drawn again. Wider than the fold moved would only cost
    /// a redraw — a line drawn again is drawn as Live now says it is — but
    /// narrower leaves a marker behind, and the block is narrower nowhere and
    /// wider only here.
    ///
    /// A part starts at a marker, which is the start of a line, so no line
    /// belongs to two parts and the lines below are the part's own.
    fn open_lines(&self, document: &Document, at: &Range<usize>) -> Range<usize> {
        let text = document.text().len();
        let part = |offset: usize| live::part_at(document, offset.min(text));
        let start = part(at.start).map_or(0, |part| part.start);
        let end = part(at.end).map_or(text, |part| part.end);
        let first = document.place(start).line;
        let last = document.place(end.saturating_sub(1).max(start)).line;
        first..last + 1
    }

    /// What a press does when it lands on Live's furniture.
    ///
    /// Two presses are not caret moves. One on a task item's marker cells
    /// flips its box; Ctrl and one on a link's words opens the destination
    /// through the desktop's default handler. Both claim the gesture, so GTK
    /// never turns the press into a move: a caret landing in the item would
    /// unfold it, and the writer who ticked a box would be left looking at the
    /// `[x]` they meant to be shown as a tick. Every other press is left
    /// entirely alone and places the caret as it always did, which unfolds the
    /// block under it.
    ///
    /// The whole answer is read off the buffer's offsets and the furnishings
    /// already worked out ([`Editor::refurnish`]), because a gesture is handed
    /// a position and no Document.
    ///
    /// A box press is an edit the caret did not make, so the caret's machine
    /// stands down for it: the `pressing_box` flag is up across the edit and
    /// [`Editor::caret_edit_began`] and [`Editor::caret_edited`] return while
    /// it is. The window's own handlers are untouched — the splice, the retag,
    /// the furniture and autosave all run.
    fn press(&self, clicks: &gtk::GestureClick, x: f64, y: f64) {
        if !self.imp().live.get() {
            return;
        }
        let (bx, by) =
            self.window_to_buffer_coords(gtk::TextWindowType::Widget, buffer_px(x), buffer_px(y));
        let Some(at) = self.iter_at_location(bx, by).map(|at| at.offset()) else {
            return;
        };
        let ctrl = clicks
            .current_event_state()
            .contains(gdk::ModifierType::CONTROL_MASK);
        let furniture = self.imp().furniture.borrow();
        let Some(standing) = furniture.iter().find(|standing| standing.at.contains(&at)) else {
            return;
        };
        // Cloned and the borrow dropped before anything is done about it: a
        // press on a box edits the buffer, the edit is spliced and retagged,
        // and the retag asks for the furniture again.
        let what = standing.what.clone();
        let box_at = standing.box_cells();
        drop(furniture);
        match what {
            Furnish::Checkbox { checked, .. } if !ctrl => {
                let Some(box_at) = box_at else { return };
                // The caret's machine stands down for the edit this makes: it
                // is the box's, not the writer's, and the caret is wherever it
                // already was, most likely on another block entirely.
                self.imp().pressing_box.set(true);
                tick(&self.buffer(), &box_at, !checked);
                self.imp().pressing_box.set(false);
            }
            Furnish::Link { destination } if ctrl => {
                open(
                    &destination,
                    self.root().and_downcast::<gtk::Window>().as_ref(),
                );
            }
            _ => return,
        }
        clicks.set_state(gtk::EventSequenceState::Claimed);
    }

    /// Works out what stands in the cells Live's fold emptied.
    ///
    /// The one pass that turns [`Furniture`] into a [`Furnishing`]: the
    /// Document's bytes into the buffer's own offsets, a task box's brackets
    /// out of the marker they sit in, and a reference link's label into the
    /// address it is defined as. All three want the Document, and the two
    /// things that read the answer have none — a snapshot is handed a frame
    /// clock and a gesture a pointer position — so the crossing is made here,
    /// on the passes that are already holding one, and the answer is kept.
    ///
    /// Bounded by the page on the glass ([`Editor::furnished`]), which is the
    /// bound the keystroke lane wants: a furnishing is held at an offset the
    /// buffer counts, and an edit anywhere moves every offset below it, so the
    /// list is thrown away and worked out again on every edit and every fold
    /// move. Paid only with Live on: `live/folded` is the one judged state that
    /// pays it, and `tools/regimes.mjs`'s `live_end_of_draft` regime launches
    /// `--live`, so the bench pins what it costs a keystroke.
    ///
    /// The viewport moves without the buffer moving, so a scroll and a resize
    /// are feeds of their own: `quill::window`'s `watch_furniture` calls this
    /// again for the rows they brought in.
    pub(crate) fn refurnish(&self, document: &Document) {
        let mut furniture = Vec::new();
        if self.imp().live.get() {
            let buffer = self.buffer();
            let at = self.caret_bytes(document);
            let over = self.furnished(document);
            furniture = standing(document, &at, &over)
                .into_iter()
                .map(|(at, what)| Furnishing {
                    at: tags::offsets_of(&buffer, document, &at),
                    what,
                })
                .collect();
        }
        if *self.imp().furniture.borrow() != furniture {
            self.imp().furniture.replace(furniture);
            self.queue_draw();
        }
    }

    /// The Document bytes [`Editor::refurnish`] works furniture out over: the
    /// rows [`Editor::seen`] holds, which are the rows
    /// [`Editor::draw_furniture`] draws.
    ///
    /// The whole Document before the widget has an allocation to read a
    /// viewport off — opening one, where the view is still nothing by nothing.
    /// That pass is the cost of opening a Document rather than of a keystroke,
    /// and it is what leaves the first frame furnished all the way down: an
    /// allocation is not a signal this widget hears, so a page bounded to a
    /// viewport of nothing would stay bare until the first edit.
    fn furnished(&self, document: &Document) -> Range<usize> {
        if self.visible_rect().height() <= 0 {
            return 0..document.text().len();
        }
        let buffer = self.buffer();
        let seen = self.seen();
        let start = tags::offset_of(document, &buffer.iter_at_offset(seen.start));
        let end = tags::offset_of(document, &buffer.iter_at_offset(seen.end));
        start..end.max(start)
    }

    /// Draws each of `lines` again in the tiers the Editor now holds, inside
    /// one `freeze_notify`.
    ///
    /// The one way a stretch of this Editor is drawn again, so that the caret's
    /// feed, the edit's feed and a cross-fade being settled all put the same
    /// page on the screen. Batched because applying a tag emits the buffer's
    /// `notify::` and nothing else: the handlers that splice the engine's copy
    /// of the text are not listening for any of this, and the text has not
    /// moved for them to hear about.
    fn redraw(&self, document: &Document, lines: &[Range<usize>]) {
        let tiers = self.imp().tiers.borrow();
        let painting = self.painting(&tiers);
        let buffer = self.buffer();
        let batch = buffer.freeze_notify();
        for at in lines {
            tags::retag(&buffer, document, painting, at);
        }
        drop(batch);
    }

    /// Moves this Editor's own painting to `ground`.
    ///
    /// Every colour on screen is read off the table at the moment it is drawn,
    /// so the switch is nothing more than changing which table is read and then
    /// making everything read it again: the tags, because a colour is baked
    /// into the tag the buffer is carrying; and the caret and the selection,
    /// because those are painted in the snapshot and a snapshot is only taken
    /// when the widget is invalidated.
    ///
    /// **The paper is not here.** The window's ground and the ink under
    /// untagged text are the stylesheet's, the stylesheet belongs to the
    /// display rather than to a widget, and `window::reset` reloads it once
    /// beside the call to this — so this method on its own leaves an Editor
    /// retagged on the old paper, and is not the whole switch. The two
    /// together are, and they are one pass because nothing is drawn between
    /// them.
    ///
    /// A whole-Document retag rather than an incremental path, because there
    /// is no incremental question to ask: every run on screen changes colour
    /// at once. #39 § Implementation Decisions (Switch) has the spike's
    /// measurement and the decision that followed from it; the same
    /// whole-Document pass is what [`Editor::show_document`] already pays to
    /// open a Document at all.
    ///
    /// Inside one `freeze_notify`, which batches the buffer's `notify::` and
    /// nothing else — applying a tag emits no `changed`, so the handlers that
    /// splice the engine's copy of the text are not listening for any of this
    /// and the text has not moved for them to hear about.
    pub fn set_ground(&self, ground: Ground, document: &Document) {
        self.imp().ground.set(ground);
        let buffer = self.buffer();
        let batch = buffer.freeze_notify();
        let tiers = self.imp().tiers.borrow();
        tags::apply(&buffer, document, self.painting(&tiers));
        drop(tiers);
        drop(batch);
        // The caret takes the new accent on its next frame, and the selection
        // its new fill, because both are read inside `snapshot`.
        self.queue_draw();
    }

    /// The leading, the air above the column, and then the column.
    fn restyle(&self) {
        // The Italic is a Face of its own, so the tags that ask for it have to
        // be moved to the new one; the rest of the type is CSS the widget
        // picks up on its own.
        tags::set_face(&self.buffer(), self.imp().face.get());
        let pitch = typography::pitch(self.imp().step.get(), LAYOUT_SCALE);
        let leading = typography::leading(pitch, self.row_height());
        // The caret's band and its unit, kept with the type: a bar placed on
        // the keystroke path must not cost a row measured all over again.
        self.imp().pitch.set(pitch);
        self.imp().baseline.set(self.row_baseline());
        self.tell_caret(|caret| caret.resize(self.em()));
        self.set_pixels_above_lines(signed(leading.above));
        self.set_pixels_inside_wrap(signed(leading.inside_wrap));
        self.set_pixels_below_lines(signed(leading.below));
        self.set_top_margin(signed(typography::page_top(pitch)));
        // The cell a heading's markers hang by moves with the size, so the
        // page is laid out from scratch rather than compared with the last
        // one: the column can be the same width at two sizes, and the hang
        // never is.
        self.imp().laid_out.set(None);
        self.lay_out(self.width(), self.height());
        // The bar is as wide and as tall as the type, so it is cut again with
        // it rather than left at the last size until the caret next moves.
        self.caret_settled();
    }

    /// Centres the measure in a view this wide and leaves the page its air.
    ///
    /// A view with no size yet is not laid out at all: `size_allocate` asks
    /// again the moment there is a size, and a column centred in nothing would
    /// only have to be undone.
    fn lay_out(&self, width: i32, height: i32) {
        if width <= 0 || height <= 0 {
            return;
        }
        let cell = typography::cell(self.imp().face.get(), self.imp().step.get());
        let page = Page {
            column: typography::column(unsigned(width), cell),
            bottom: signed(typography::page_bottom(unsigned(height))),
        };
        if self.imp().laid_out.get() == Some(page) {
            return;
        }
        self.imp().laid_out.set(Some(page));
        let side = signed(page.column.side);
        self.set_left_margin(side);
        self.set_right_margin(side);
        self.set_bottom_margin(page.bottom);
        // The heading markers hang into this container's gutter, so they are
        // re-hung with it: both halves of the pair move, the measure's edge
        // with the window and the marker run with the type.
        self.hang();
    }

    /// Hangs the heading markers into the gutter of the page as last laid out.
    ///
    /// Its own pass because three things move it and only one of them is the
    /// window: the measure's edge with the allocation, the marker run with the
    /// type, and — since Live sets a heading larger than the body — the run
    /// again when Live is switched on or off. A page that has never been laid
    /// out has no gutter to hang into and nothing to redraw.
    fn hang(&self) {
        let Some(page) = self.imp().laid_out.get() else {
            return;
        };
        tags::hang_markers(
            &self.buffer(),
            self.imp().step.get(),
            page.column,
            std::array::from_fn(|level| self.marker_advance(level as u8 + 1)),
            &self.colours(),
        );
    }

    /// How tall one row of ink is, in the Face and size now set.
    ///
    /// Laid out on a description built here rather than read off the widget:
    /// the stylesheet that will draw this same text is applied when GTK next
    /// recomputes style, which is after this is asked, so a layout that took
    /// the widget's word for it would be measuring the desktop theme's font.
    /// The description and the stylesheet are two spellings of one type —
    /// Pango and GTK's CSS name a feature and a weight differently — and both
    /// are spelled from [`FEATURES`] and [`INK_WEIGHT`], because a row
    /// measured with kerning and drawn without it is the wrong row, and the
    /// leading is built on this number.
    fn row_height(&self) -> u32 {
        unsigned(self.body_layout().pixel_size().1)
    }

    /// How far the baseline sits below the top of one row of body type, in the
    /// pixels the widget lays out in.
    ///
    /// The anchor [`caret::band_top`] takes, measured off the same layout the
    /// row's height is, because a baseline measured on one layout and a row
    /// measured on another are two rows.
    fn row_baseline(&self) -> f64 {
        f64::from(self.body_layout().baseline()) / f64::from(pango::SCALE)
    }

    /// One row of body type, laid out to be measured.
    fn body_layout(&self) -> pango::Layout {
        self.measured("Ag")
    }

    /// How far a level-`level` heading's markers advance: its `#`s and the one
    /// space after them, in the pixels this widget lays out in.
    ///
    /// **Measured, not counted off [`typography::cell`]**, because the two are
    /// not the same number and which one is right changes under the app. Pango
    /// rounds a glyph's advance to a whole pixel when font metrics are hinted
    /// and does not when they are not, and this app is both: a `--deterministic`
    /// launch pins hinting on in [`harness::determine`], and a writer's own
    /// launch takes whatever the desktop and the display's scale settle on. At
    /// the default step that is 13 px a cell in a judged shot and 12.798 in the
    /// app.
    ///
    /// A hang is handed straight back by these very glyphs — the heading's row
    /// starts one marker run left of the body column and Pango then advances
    /// that run before the first word — so a hang that is not what they will
    /// actually advance puts the heading's words off the column, and the six
    /// levels on to as many columns. Round 10 of the Markup Piece was lost to
    /// exactly that, and counting the cell either way would only have moved
    /// which of the two builds was wrong (#167).
    ///
    /// Asking the layout is the best either can do, and it is not the same
    /// answer for both. Hinted, the advance is already whole and the six
    /// levels land on one column exactly. Unhinted it is fractional, and a
    /// `left-margin` is an `i32`, so the rounding leaves a residual of up to
    /// half a logical pixel — one device pixel at scale 2, and no integer
    /// margin can beat it. That residual is the same disagreement ADR 0016
    /// leaves open for the Typography Piece.
    ///
    /// The markers are marker ink at [`Weight::Regular`](quill_engine::annotate::Weight),
    /// which is what body type is set at, so the body's own description
    /// measures them.
    ///
    /// With Live on they are the heading's own size instead
    /// ([`tags::LADDER`]), because that is what the layout will advance them
    /// by: the `#`s carry the heading's scale whether they are on the page or
    /// folded to transparent ink, and one `heading-<level>` tag cannot hang
    /// two ways. So under Live every heading of a level hangs by the scaled
    /// run, and with Live off nothing here moves.
    fn marker_advance(&self, level: u8) -> i32 {
        let mut run = "#".repeat(usize::from(level));
        run.push(' ');
        let layout = self.measured_at(&run, self.heading_scale(level));
        // The logical width, rounded once here, as every other horizontal
        // length this widget sets is.
        let width = f64::from(layout.size().0) / f64::from(pango::SCALE);
        #[expect(
            clippy::cast_possible_truncation,
            reason = "seven cells of one type size, which is under a hundred pixels"
        )]
        let whole = width.round().max(0.0) as i32;
        whole
    }

    /// `text` laid out in the Face, size and features body type is set in, for
    /// measuring rather than for drawing.
    ///
    /// The description is built here rather than read off the widget because
    /// GTK recomputes style after this is asked, and a layout that took the
    /// widget's word for it would be measuring the desktop theme's font.
    fn measured(&self, text: &str) -> pango::Layout {
        self.measured_at(text, 1.0)
    }

    /// [`Editor::measured`], with the em multiplied by `scale`.
    ///
    /// The one thing measured off the body's size rather than at it is a
    /// heading's markers under Live, and they are measured through the same
    /// description for the reason [`Editor::measured`] builds one: a run
    /// measured on the desktop theme's font is the wrong run at any size.
    fn measured_at(&self, text: &str, scale: f64) -> pango::Layout {
        let layout = self.create_pango_layout(Some(text));
        layout.set_font_description(Some(&body_font(
            self.imp().face.get(),
            typography::em(self.imp().step.get()) * scale,
        )));
        let features = pango::AttrList::new();
        features.insert(pango::AttrFontFeatures::new(&pango_features()));
        layout.set_attributes(Some(&features));
        layout
    }

    /// How much larger than the body a heading of `level` is drawn: the Live
    /// ladder while Live is on, and the body's own size while it is off.
    fn heading_scale(&self, level: u8) -> f64 {
        if !self.imp().live.get() {
            return 1.0;
        }
        tags::LADDER
            .get(usize::from(level).saturating_sub(1))
            .copied()
            .unwrap_or(1.0)
    }

    /// Shows `document`, marked up, with the caret at its start.
    ///
    /// The Markup is derived and drawn in the same breath as the text, because
    /// a frame showing the prose before its structure is a flash of the wrong
    /// page. Whole-Document on open is the architecture's cold-start cost; the
    /// keystroke path retags by the block.
    ///
    /// Filling the buffer is itself an edit as far as GTK is concerned, and
    /// the Document being shown is already the Document those edits would
    /// produce, so [`Editor::loading`] is true across it and the handlers on
    /// the buffer stand down.
    pub fn show_document(&self, document: &Document) {
        let buffer = self.buffer();
        self.imp().loading.set(true);
        buffer.set_text(document.text());
        self.imp().loading.set(false);
        // The caret this Document opens at, worked out before the draw rather
        // than read off the buffer after it: with Focus on the first sentence
        // is what the first frame lights, and a page drawn before its tiers
        // were known would be a flash of the whole thing bright.
        self.retier_at(document, &(0..0));
        let tiers = self.imp().tiers.borrow();
        tags::apply(&buffer, document, self.painting_at(&(0..0), &tiers));
        drop(tiers);
        // The fold this Document opened at, remembered for the same reason the
        // tiers are: the first caret move has to know which block to fold, and
        // every draw before it has to know where the fold stands.
        self.imp().writer.replace(0..0);
        self.imp().open.replace(
            self.imp()
                .live
                .get()
                .then(|| self.open_lines(document, &(0..0))),
        );
        buffer.place_cursor(&buffer.start_iter());
        // After the cursor, because a furnishing is a judgement about the
        // block the caret is not in and the caret is only where this Document
        // opens once the buffer has been told.
        self.refurnish(document);
    }

    /// Whether the buffer is being filled rather than written in.
    pub(crate) fn loading(&self) -> bool {
        self.imp().loading.get()
    }

    /// Draws the lines an edit changed: the ones whose Markup moved, the ones
    /// the dim moved across, and with Focus on the line the typed bytes lie
    /// on; no others.
    ///
    /// Called from the buffer's `changed`, after the text has moved and after
    /// the engine's copy has been spliced to match it, because a tag is put on
    /// by the line and the byte index within it and both of those have to be
    /// the ones the writer can now see.
    pub fn retag(&self, document: &Document, edit: &Edit) {
        // An edit moves the caret as well as the text, so the tiers are worked
        // out again here rather than left to the caret's own feed: the lines
        // the edit changed and the lines the dim moved across are drawn in the
        // same pass. A line named by both is drawn twice, which is a tag taken
        // off and put back on the same bytes and not a second look on them.
        //
        // What the edit moved is not what `retier` answers: that compares the
        // tiers' bytes as they stood, and a keystroke inside the lit sentence
        // moves every byte after it, so every line read as moved and the
        // sentence's last byte faded from dim to bright on each key (#224).
        // The tiers from before are carried across the edit first, and with
        // Focus off there are none to carry: the tiers on hand are the last
        // ones Focus lit, left where they were for the reason `retier_at` has.
        let on = matches!(self.imp().focus.get(), Focus::On(_));
        let was = self.imp().tiers.borrow().clone();
        if on {
            let at = self.caret_bytes(document);
            self.imp().tiers.replace(self.tiers_at(document, &at));
        }
        // Every edit settles the fade, whether or not a tier moved with it: a
        // fade holds the buffer's offsets, and an edit is the one thing that
        // moves the text out from under them. A writer typing through their own
        // full stop therefore sees the fade the keystroke started run until the
        // next keystroke and then arrive, rather than a stretch of an earlier
        // sentence left stranded half-way between the two tiers.
        self.settle_fade(document);
        // An edit moves the fold as well as the dim: a writer who types a
        // blank line has left one block for another, and the lines the fold
        // was last open over have moved under the splice. Asked before the
        // edit's own redraw, because the redraw folds to the writer the fold
        // last committed ([`Editor::painting`]) and the splice has just moved
        // that writer; the block the caret left is folded in the same pass
        // either way ([`Editor::refold`]).
        self.refold(document);
        self.redraw(document, std::slice::from_ref(&edit.lines));
        // And the furniture, whether or not the fold moved: every furnishing
        // below the splice is held at an offset the edit has just moved.
        // Before the Focus half returns, because Live is on or off on its own.
        self.refurnish(document);
        if !on {
            return;
        }
        let after = self.imp().tiers.borrow();
        let before = focus::rebased(document, &was, &edit.splice, &after);
        let moved = focus::changed(&before, &after);
        drop(after);
        self.redraw(document, typed_lines(document, edit).as_slice());
        self.redraw(document, &moved);
        self.begin_fade(document, &before, &moved);
    }

    /// How much Focus leaves lit from now on, with the page redrawn to say so.
    ///
    /// The whole Document rather than the lines a tier moved across, which is
    /// what separates this from [`Editor::refocus`]: the tiers only ever name
    /// the caret's block ([`focus::tiers`]), and every other block is dim by
    /// not being named at all, so a Focus that has just been switched on or off
    /// has changed the colour of blocks no tier mentions. The same
    /// whole-Document pass [`Editor::set_scheme`] pays for the same reason.
    ///
    /// Nothing cross-fades here. A fade is the dim moving between sentences as
    /// the writer writes; a key that switches Focus on or off is the writer
    /// asking for another page, and the oracle's fade is on the focus moving
    /// rather than on the mode arriving.
    pub fn set_focus(&self, focus: Focus, document: &Document) {
        self.imp().focus.set(focus);
        // Any fade is dropped rather than settled: the draw below is the whole
        // Document, which takes every tag off and puts back the ones the new
        // Focus asks for, interim colours included.
        self.imp().fade.take();
        self.retier(document);
        let buffer = self.buffer();
        let batch = buffer.freeze_notify();
        let tiers = self.imp().tiers.borrow();
        tags::apply(&buffer, document, self.painting(&tiers));
        drop(tiers);
        drop(batch);
        self.queue_draw();
        self.settle();
    }

    /// Whether the markup is rendered in place from now on, with the page
    /// redrawn to say so.
    ///
    /// The whole Document rather than the lines a fold moved across, and for
    /// [`Editor::set_focus`]'s reason: every block but the writer's changes
    /// when Live arrives, and the fold only ever names the writer's own.
    ///
    /// The hang moves with it. A heading's `#`s advance further at the
    /// heading's Live size than at the body's, and the gutter is hung by
    /// exactly what they advance ([`tags::hang_markers`]), so the six levels
    /// are measured again before the draw that will use them.
    pub fn set_live(&self, live: bool, document: &Document) {
        self.imp().live.set(live);
        self.hang();
        let at = self.caret_bytes(document);
        self.imp().writer.replace(at.clone());
        self.imp()
            .open
            .replace(live.then(|| self.open_lines(document, &at)));
        let buffer = self.buffer();
        let batch = buffer.freeze_notify();
        let tiers = self.imp().tiers.borrow();
        tags::apply(&buffer, document, self.painting_at(&at, &tiers));
        drop(tiers);
        drop(batch);
        // Everything Live left standing arrives with it and goes with it:
        // this fills the furniture on the way on and empties it on the way
        // off, so a page with Live off carries none of it.
        self.refurnish(document);
        self.queue_draw();
        // A heading's row is the size of its type, so the rows under the caret
        // have moved: the bar is re-cut on the page as it now stands.
        self.caret_settled();
        self.settle();
    }

    /// How long a cross-fade in this Editor runs for.
    ///
    /// Asked at the moment a fade starts rather than held, because both halves
    /// of the answer can move under a running app: a desktop that asks for
    /// reduced motion mid-session turns `gtk-enable-animations` off, and GTK
    /// is what fills that from the portal. A display with no settings at all is
    /// a process that will not draw a frame, so it is read as no animations.
    fn fade_length(&self) -> u32 {
        theme::fade_ms(self.imp().deterministic.get(), Self::animations())
    }

    /// Whether the desktop allows animations now: `gtk-enable-animations`,
    /// read at the moment it is asked for, for the reason
    /// [`Editor::fade_length`] gives.
    fn animations() -> bool {
        gtk::Settings::default().is_some_and(|settings| settings.is_gtk_enable_animations())
    }

    /// Whether a Typewriter move glides rather than jumps: the same two
    /// answers that decide whether a tier change fades
    /// ([`theme::animated`]).
    fn glides(&self) -> bool {
        theme::animated(self.imp().deterministic.get(), Self::animations())
    }

    /// Starts the cross-fade the tiers that just moved ask for.
    ///
    /// `before` is the tiers as they were and the Editor now holds the tiers as
    /// they are, so the same bytes painted under each are the two ends of the
    /// fade. The target has already been drawn by the caller, which is what
    /// makes a fade something that can be dropped at any frame: the page is
    /// already right, and the fade is only holding the old colour on top of it
    /// for [`quill_engine::theme::FADE_MS`].
    fn begin_fade(&self, document: &Document, before: &[LineTiers], moved: &[Range<usize>]) {
        if self.fade_length() == 0 || moved.is_empty() {
            return;
        }
        let focus = self.imp().focus.get();
        let colours = self.colours();
        let buffer = self.buffer();
        let after = self.imp().tiers.borrow();
        let mut runs = Vec::new();
        for lines in moved {
            let at = document.line_bytes(lines.start).start
                ..document.line_bytes(lines.end.saturating_sub(1)).end;
            let spans = document.spans_in(&at);
            runs.append(&mut faded(
                &annotate::paint_in(&spans, &at, before, focus, &colours),
                &annotate::paint_in(&spans, &at, &after, focus, &colours),
                |at| tags::offsets_of(&buffer, document, at),
            ));
        }
        drop(after);
        if runs.is_empty() {
            return;
        }
        self.imp().fade.replace(Some(Fade {
            started: self.now(),
            lines: moved.to_vec(),
            runs,
        }));
        self.start_fade();
    }

    /// Ends a cross-fade wherever it had got to, with its lines on the page as
    /// they are meant to end.
    ///
    /// Drawn again rather than recoloured to the target, because this is the
    /// path taken when the ground has moved under the fade — a keystroke, a
    /// second tier change — and a draw asks the engine what those lines look
    /// like now instead of trusting a colour worked out for a page that has
    /// since changed. A line number the edit has moved is a line drawn as it
    /// stands, which is never wrong, only wasted.
    fn settle_fade(&self, document: &Document) {
        let Some(fade) = self.imp().fade.take() else {
            return;
        };
        self.redraw(document, &fade.lines);
    }

    /// Attaches the cross-fade's frame-clock callback, if it is not already
    /// attached.
    ///
    /// The idle rule the caret's tick follows ([`Editor::start_tick`]): the
    /// source stands only while a fade is in flight, and the frame that
    /// arrives at the target is the frame that drops it. So an Editor with a
    /// settled page asks the frame clock for nothing on Focus's account.
    fn start_fade(&self) {
        self.keep_ticking(|editor| &editor.imp().fading, Self::advance_fade);
    }

    /// Attaches a frame-clock callback that calls `advance` with each frame's
    /// time until it says no more frames are wanted, unless `attached` says
    /// one already is. The fade and the glide are each one of these, with
    /// their own flag, so that neither asks for the other's frames.
    fn keep_ticking(&self, attached: fn(&Self) -> &Cell<bool>, advance: fn(&Self, i64) -> bool) {
        if attached(self).replace(true) {
            return;
        }
        self.add_tick_callback(move |editor, clock| {
            if advance(editor, clock.frame_time()) {
                return glib::ControlFlow::Continue;
            }
            attached(editor).set(false);
            glib::ControlFlow::Break
        });
    }

    /// Runs `step` on each of the next `frames` frames, stopping early the
    /// frame it answers true: the hold `--scroll` keeps, the one Typewriter
    /// keeps on an allocation and the one a `--caret` reveal keeps all outlast
    /// the scroll GTK makes of its own while the layout is validated
    /// ([`Editor::scroll_to`], [`Editor::hold_row`], [`Editor::reveal_caret`]).
    fn over_frames(&self, frames: u32, step: impl Fn(&Self) -> bool + 'static) {
        let left = Cell::new(frames);
        self.add_tick_callback(move |editor, _| {
            left.set(left.get().saturating_sub(1));
            if step(editor) || left.get() == 0 {
                glib::ControlFlow::Break
            } else {
                glib::ControlFlow::Continue
            }
        });
    }

    /// Puts the frame at `now` of the cross-fade on the page, and says whether
    /// another frame is wanted.
    ///
    /// One foreground tag stays on each run, because each frame takes off the
    /// colour it put on last ([`tags::recolour`]). A frame whose colour rounds
    /// to the one already showing is skipped, which is most frames of a fade
    /// between two close greys: the tag work of a fade is the number of
    /// distinguishable colours between its ends, not the number of frames it
    /// is drawn over.
    fn advance_fade(&self, now: i64) -> bool {
        let mut slot = self.imp().fade.borrow_mut();
        let Some(fade) = slot.as_mut() else {
            return false;
        };
        let elapsed = millis_since(fade.started, now);
        let arrived = elapsed >= theme::FADE_MS;
        let buffer = self.buffer();
        let batch = buffer.freeze_notify();
        for run in &mut fade.runs {
            let colour = Colour::fade(run.from, run.to, elapsed);
            if colour == run.shown {
                continue;
            }
            tags::recolour(&buffer, &run.at, run.shown, colour);
            run.shown = colour;
        }
        drop(batch);
        if arrived {
            *slot = None;
        }
        !arrived
    }

    /// The frame the widget is in, in the microseconds the machine counts.
    ///
    /// The frame clock's own time and never a system clock, so that the blink
    /// and the glide are functions of the frame they will be seen in. A widget
    /// with no surface yet has no clock; before the first frame everything
    /// happens at zero, which is one instant, which is what "before the first
    /// frame" means to the machine.
    fn now(&self) -> i64 {
        self.frame_clock().map_or(0, |clock| clock.frame_time())
    }

    /// Tells the caret's machine what happened, and keeps what it made of it.
    ///
    /// The machine is `Copy` and lives in a `Cell`, so every event is a read,
    /// a change and a write back; this is that, once, so that the four places
    /// that feed it say only what they are feeding it.
    fn tell_caret(&self, said: impl FnOnce(&mut caret::Caret)) {
        let mut caret = self.imp().caret.get();
        said(&mut caret);
        self.imp().caret.set(caret);
    }

    /// The table this Editor paints from: the held ground's colours.
    fn colours(&self) -> Colours {
        self.imp().ground.get().colours
    }

    /// One em in the device pixels the machine measures its gates in.
    fn em(&self) -> f64 {
        typography::em(self.imp().step.get()) * self.scale()
    }

    /// The surface's scale factor, which is the machine's unit.
    ///
    /// Everything in a [`caret::Bar`] is device pixels, because the snap that
    /// keeps the bar's edges hard is a snap onto one of those. Never zero: a
    /// widget with no surface yet is told it is at scale 1.
    fn scale(&self) -> f64 {
        f64::from(self.scale_factor().max(1))
    }

    /// The bar at the caret now, in device pixels.
    ///
    /// The column comes from `iter_location`, which answers in the buffer
    /// coordinates the layer is snapshotted in, and is the advance boundary
    /// between two cells. The bar is centred on it, half its width each side,
    /// by [`caret::left`]. See [ADR 0013](../../docs/adr/0013-caret-on-the-advance-boundary.md)
    /// for the 0.07 em that used to be added here and what measuring iA Writer
    /// itself said about it, and `docs/design.md` row Caret column for the
    /// centring, which sharpens that ADR: its bar put this edge *on* the
    /// boundary, and the Design oracle's is 3 px before it.
    ///
    /// The band is the pitch, not the glyph, and it hangs from the row's
    /// baseline: `iter_location` gives the top of the box, the baseline is the
    /// same distance below it on every row, and [`caret::band_top`] carries
    /// the share of the pitch that goes above it. Anchoring to the box instead
    /// is what `caret.js`'s header warns against in as many words: a band on
    /// the box is centred on the font's em box rather than on the ink, and
    /// reads as top-heavy against the letters.
    ///
    /// The distance holds on every row, wrapped or not, because of the
    /// three-way leading split of ADR 0004: `pixels-inside-wrap` carries all
    /// the air between two rows of a paragraph and `pixels-above-lines` plus
    /// `pixels-below-lines` carry all of it between two paragraphs, so no row
    /// has more air inside its own box than any other.
    ///
    /// `None` before the type has been set, when there is no band to speak of.
    fn bar(&self) -> Option<caret::Bar> {
        let pitch = self.imp().pitch.get();
        if pitch == 0 {
            return None;
        }
        let buffer = self.buffer();
        let row = self.iter_location(&buffer.iter_at_mark(&buffer.get_insert()));
        let step = self.imp().step.get();
        let scale = self.scale();
        let (y, h) = self.band(&row);
        let w = f64::from(typography::caret_width(step, scale));
        Some(caret::Bar {
            x: caret::left(f64::from(row.x()) * scale, w),
            y,
            w,
            h,
        })
    }

    /// The band of the row `row` is a character's box on, in device pixels:
    /// the y every mark on that row takes, and the height they all take.
    ///
    /// One function because the registration is the whole of the answer. The
    /// caret and the selection's fill rows are boxes that have to agree to the
    /// pixel — the `selection` state is judged on exactly that — and two sites
    /// computing one band from the same three numbers are two chances to
    /// disagree. `caret.js` has the same rule and keeps it the same way: one
    /// `dy` at `:185`, read by the fill and the caret alike.
    ///
    /// The caller has already checked the pitch: a zero pitch here is a band
    /// of no height, which draws nothing rather than wrongly.
    /// The top is snapped here rather than left to the machine, which snaps
    /// the caret's own on the way in ([`caret::Caret::moved`]). The fill rows
    /// never go through the machine, and a band left on a
    /// fraction of a device pixel is rasterised a row taller than it was cut,
    /// with a pale row standing in for the fraction — which is exactly the
    /// disagreement this function exists to prevent, one end of the band at a
    /// time.
    ///
    /// The whole rectangle rather than its top, because under Live a heading's
    /// row is not the pitch tall and the rectangle is where the view says how
    /// tall it is ([`Editor::row_pitch`]).
    fn band(&self, row: &gdk::Rectangle) -> (f64, f64) {
        let scale = self.scale();
        let (pitch, baseline) = self.row_pitch(row);
        (
            caret::snap(
                caret::band_top(f64::from(row.y()) + baseline + BASELINE_DRIFT, pitch) * scale,
            ),
            pitch * scale,
        )
    }

    /// The pitch and the baseline of the row `row` is a character's box on, in
    /// logical pixels.
    ///
    /// The body row's pair, cached with the type, on every row of a page with
    /// Live off — which is every row every judged state was won on, and the
    /// reason the answer is gated on the mode rather than taken from the view
    /// throughout. `iter_location` answers with the ink's own height, and a
    /// row set in another Face at the same size — inline code in the Mono
    /// Face, an italic run — need not be the body's to the pixel; a band cut
    /// from that would move a state Live never reaches.
    ///
    /// With Live on the view is asked, because the ladder ([`tags::LADDER`])
    /// sets a heading larger than the body and the row it stands on is taller
    /// by exactly as much as its ink is: the leading is a widget property, so
    /// the same air sits above and below a row whatever it holds
    /// ([`typography::leading`]), and only the ink between them grows. Every
    /// other row under Live is the body's ink and comes back out of
    /// [`row_band`] as the cached pair, so it is untouched.
    fn row_pitch(&self, row: &gdk::Rectangle) -> (f64, f64) {
        let pitch = f64::from(self.imp().pitch.get());
        let baseline = self.imp().baseline.get();
        if !self.imp().live.get() {
            return (pitch, baseline);
        }
        // The body row's ink, from the numbers already in hand rather than
        // from a fresh `Editor::row_height`: the leading split the pitch into
        // the ink and the air above and below it, so the air taken off the
        // pitch is the ink back again — and a band is cut on the paint path,
        // once for the caret and once for every row of a selection, which is
        // no place to lay out a row of type.
        let air = f64::from(self.pixels_above_lines() + self.pixels_below_lines());
        row_band(pitch, baseline, pitch - air, f64::from(row.height()))
    }

    /// The selection as the boxes that draw it, in device pixels.
    ///
    /// `None` when there is nothing selected, before the type has been set, or
    /// before the view has been laid out — a selection is a fill of the
    /// container ([`typography::Column`]), and a view with no size yet has no
    /// container to fill.
    ///
    /// The walk is display lines rather than logical ones, because a wrapped
    /// paragraph is as many bands as it has rows on the glass. A row is
    /// measured from an iterator's own box and never from one sitting on a
    /// wrap: that one offset is both the end of one row and the start of the
    /// next, and which of the two `iter_location` answers for is not ours to
    /// decide. A character's own box is on one row and only one.
    ///
    /// A selection is a fill of the **container**, not of the ink it covers
    /// (`docs/design.md` rows Selection, Held newline and Multi-row fill; the
    /// Design oracle's `09-dark` and `10-newline-only`). So only the two ends
    /// are measured off glyphs: the first row starts at the anchor and the
    /// last stops at the focus. Every edge between them is
    /// [`typography::Column`]'s — a row the selection entered from above is
    /// filled from the container's left edge, and a row it runs past to the
    /// container's right edge. The interior rows of a multi-row selection are
    /// both, and are filled edge to edge whatever their ink. A selection
    /// inside one row, holding no row end, is the anchor to the focus and
    /// nothing more.
    ///
    /// Only what can be seen is built. `drawSelection` clips to the viewport
    /// with six rows of slack either side and counts nothing outside it toward
    /// its cap, and this does the same at both ends: the walk begins at the
    /// first row inside the band rather than at the selection's own start, and
    /// stops at the first row past it. A select-all is then a screenful of
    /// work wherever the view is sitting, instead of a rectangle for every row
    /// of the document — and it is drawn, which a walk that spent its cap
    /// above the fold would not be.
    ///
    /// Rows are the whole of what is built: neither end is bracketed by a bar
    /// and the caret is out while the selection stands (ADR 0014). So a
    /// selection clipped at one end draws exactly what a selection clipped at
    /// neither does, and the walk owes nothing to where it stopped.
    fn selection(&self) -> Option<Selection> {
        let (start, end) = self.buffer().selection_bounds()?;
        let pitch = f64::from(self.imp().pitch.get());
        if pitch == 0.0 {
            return None;
        }
        let column = self.imp().laid_out.get()?.column;
        let container_left = f64::from(column.left);
        let container_right = f64::from(column.right);
        let scale = self.scale();
        let view = self.visible_rect();
        let top = f64::from(view.y()) - pitch * SELECTION_SLACK;
        let bottom = f64::from(view.y() + view.height()) + pitch * SELECTION_SLACK;
        let mut rows: Vec<caret::Bar> = Vec::new();
        let mut at = start;
        if let Some(seen) = self.iter_at_location(0, buffer_px(top.max(0.0)))
            && seen > at
        {
            at = seen;
        }
        if at >= end {
            // The whole of it is above the band. Nothing to build, and no end
            // to bracket: the walk below would read the row `end` sits on as
            // the selection's last and bar it, which is a mark on a row the
            // writer is not looking at.
            return None;
        }
        // `at <= end`, not `<`: a selection that ends where a row begins ends
        // *on* that row, and its closing bar belongs at that row's left edge
        // rather than out at the end of the row above. Selecting through a
        // line's newline and no further is exactly this, and it is what the
        // oracle draws — `rows.set(0, { l: lr.left, r: lr.left })` when a line
        // contributes no rectangles, with `lastEdge` taken from it. The row is
        // empty, so it costs a zero-width fill that [`draw_box`] drops.
        while at <= end && rows.len() < MAX_ROWS {
            let mut stop = at;
            // The iterator is the answer here, never the boolean. Neither of
            // GTK's two display-line moves reports whether it moved: each
            // reports whether the place it arrived at is something other than
            // the buffer's end iterator, and the last row of a Document ends
            // exactly there. Read as "it did not move", that false put `stop`
            // back at the row's start, and the row then measured no width and
            // never satisfied `stop >= end` — so the Document's last row drew
            // no fill and the walk left no bar on it, whatever the selection
            // really ended at. Every row but the last one was painted (#146).
            //
            // A move that has nowhere to go leaves the iterator alone, which
            // is the empty row and the row a selection starts at the end of,
            // and both of those want `stop == at` anyway.
            self.forward_display_line_end(&mut stop);
            stop = stop.max(at).min(end);
            let box_of_first = self.iter_location(&at);
            if f64::from(box_of_first.y()) > bottom {
                break;
            }
            // A row the selection entered from above is filled from the
            // container's left edge rather than from its first glyph: the
            // writer held everything on it, gutter included. `at > start` is
            // that test and it survives the clip at the top of the band —
            // a row the walk began at because the rows above are off the
            // glass was still entered from above.
            let entered_from_above = at > start;
            let left = if entered_from_above {
                container_left
            } else {
                f64::from(box_of_first.x())
            };
            let mut right = left;
            if stop > at {
                let mut last = stop;
                last.backward_char();
                let glyph = self.iter_location(&last);
                right = f64::from(glyph.x() + glyph.width());
            }
            // A row the selection runs past is filled to the container's right
            // edge, not to its own last glyph. `stop < end` is that test: the
            // row's end is inside the selection, so whatever the row holds
            // after its ink — a newline with no advance, or a wrap with
            // nothing at all — is held too, and the fill stands for it. An
            // empty row inside a selection is filled the same way, edge to
            // edge, without ink of its own to measure.
            if stop < end {
                right = container_right;
            }
            let (y, h) = self.band(&box_of_first);
            let x = caret::snap(left * scale);
            let row = caret::Bar {
                x,
                y,
                w: (caret::snap(right * scale) - x).max(0.0),
                h,
            };
            rows.push(row);
            if stop >= end {
                break;
            }
            // The same false, for the same reason: stepping onto a last row
            // that is empty — a Document ending in a newline — arrives at the
            // end iterator and is reported as a failure to step. `next <= at`
            // is the honest stop, and it is the one a move with nowhere to go
            // leaves behind.
            let mut next = at;
            self.forward_display_line(&mut next);
            if next <= at {
                break;
            }
            at = next;
        }
        if rows.is_empty() {
            return None;
        }
        // The fill and nothing else. Where the oracle sets a bar at each end
        // (`setEdge` in `legacy/app/js/caret.js`) and the port inherited both,
        // iA Writer for Mac — measured off the owner's captures of the running
        // app — draws no bar at either end of a selection and no caret while
        // one stands. ADR 0014 carries the measurements and what they cost.
        Some(Selection { rows })
    }

    /// A change to the buffer is starting.
    ///
    /// Stamped where the change starts rather than where it ends, because the
    /// mark moves inside the change and GTK does not promise which of
    /// `mark-set` and `changed` a listener hears first. What the caret path
    /// needs is only that the stamp is down before the move arrives.
    ///
    /// Filling the buffer with a Document is a delete and an insert like any
    /// other and is not a writer's edit, so this stands down for it with the
    /// rest of the handlers watching this buffer. A task box flipped by a
    /// press is the second edit the caret did not make, and stands down here
    /// for the same reason ([`Editor::press`]).
    fn caret_edit_began(&self) {
        if self.loading() || self.imp().pressing_box.get() {
            return;
        }
        self.imp().edited.set(Some(self.now()));
    }

    /// The insert mark moved: tell the machine where to, and draw it there.
    fn caret_moved(&self) {
        let Some(bar) = self.bar() else {
            return;
        };
        let now = self.now();
        let kind = caret::kind(self.imp().last.get(), self.imp().edited.get() == Some(now));
        self.place_bar(bar, kind, now);
        self.keep_in_band(bar);
    }

    /// Keeps the caret's row where the modes say as the writer moves it.
    ///
    /// Which rule holds the row is the engine's
    /// ([`typewriter::hold`]): Typewriter's anchor, the pointer band for a
    /// moment after a click, the edge band with Focus on and Typewriter off,
    /// and with both off the caret ticket's band — `scroll-padding: 10vh 0
    /// 28vh` in `legacy/app/css/page.css`, which [`Editor::keep_in_margins`]
    /// applies.
    ///
    /// [`caret::Source::App`] is out under every rule: a launch flag, a
    /// restored position or a Command is put where it was asked for rather
    /// than travelled to. A judged state naming both `--caret` and `--scroll`
    /// means both, and a view that chased the caret would shoot a different
    /// passage from the one it was asked for. `--typewriter`'s first frame is
    /// [`Editor::reveal_caret`]'s, which puts the row at the anchor without a
    /// move to follow.
    fn keep_in_band(&self, bar: caret::Bar) {
        let last = self.imp().last.get();
        if last == caret::Source::App {
            return;
        }
        let since_press = self
            .imp()
            .pressed
            .get()
            .map(|pressed| millis_since(pressed, self.now()));
        match self.hold(since_press) {
            Hold::Free => {
                if last == caret::Source::Key {
                    self.keep_in_margins(bar);
                }
            }
            hold => self.follow(bar, hold, self.glides()),
        }
    }

    /// Which rule holds the caret's row now, `since_press` milliseconds after
    /// the pointer was last pressed or released.
    fn hold(&self, since_press: Option<u32>) -> Hold {
        typewriter::hold(
            self.imp().typewriter.get(),
            self.imp().focus.get() != Focus::Off,
            since_press,
        )
    }

    /// Puts the row where the modes now hold it, without a glide: the
    /// oracle's rule for a setting that changed (`focus.js:274-276`), so that
    /// Typewriter switched on is seen to hold the row and Focus switched on
    /// keeps its lit sentence off the edges from the first frame.
    fn settle(&self) {
        if self.imp().laid_out.get().is_none() {
            return;
        }
        let Some(bar) = self.bar() else {
            return;
        };
        match self.hold(None) {
            Hold::Free => {}
            hold => self.follow(bar, hold, false),
        }
    }

    /// Re-holds the caret's row at the anchor after an allocation, with
    /// Typewriter on.
    ///
    /// The anchor is a share of the viewport, and an allocation is what
    /// changes the viewport: a window resized, or the chrome stepping back
    /// and giving the page its height, which is how a `--typewriter` launch
    /// arrives at its first still frame. The row did not travel, so the view
    /// jumps rather than glides, as [`Editor::caret_settled`] places the bar
    /// without a glide.
    ///
    /// Nothing is done before the layout can say how far it scrolls: a text
    /// view validates its lines after the allocation, so at the allocation
    /// the adjustment's upper is still no more than its page, and a target
    /// clamped against it would pin the view at the top. The hold is asked
    /// for again on the frames after, for as long as `--scroll` holds its own
    /// ([`SCROLL_FRAMES`]), and taken on the first of them the layout has
    /// been validated by; measured on `ref/sample.md` that is the frame after
    /// the first, and [`Editor::reveal_caret`] has the first.
    fn hold_row(&self) {
        let Typewriter::On(anchor) = self.imp().typewriter.get() else {
            return;
        };
        if self.imp().pinned.get() || self.hold_row_now(anchor) {
            return;
        }
        self.over_frames(SCROLL_FRAMES, move |editor| editor.hold_row_now(anchor));
    }

    /// Holds the row at `anchor` now, and says whether the layout was far
    /// enough along to.
    fn hold_row_now(&self, anchor: f64) -> bool {
        let validated = self
            .vadjustment()
            .is_some_and(|adjustment| adjustment.upper() > adjustment.page_size());
        if !validated {
            return false;
        }
        if let Some(bar) = self.bar() {
            self.follow(bar, Hold::Anchor(anchor), false);
        }
        true
    }

    /// Keeps the caret's row inside the scroll band with both modes off.
    ///
    /// [`Editor::keep_in_band`] calls it only for a move a key made, which is
    /// the oracle's own rule rather than a narrowing of it. The band is
    /// `scroll-padding` on the scroller, and `scroll-padding` is spent by the
    /// browser's caret-into-view — which runs on typing and on cursor keys,
    /// and not on a click, whose target the hand could already see. So a
    /// click low on the page does not jump it, and the first key pressed
    /// afterwards brings the row into the band.
    ///
    /// The row comes from the bar the machine was just handed rather than a
    /// second `iter_location`, because this is on the keystroke path; it is
    /// read in the adjustment's coordinate by [`Editor::row_of`]. The target
    /// is not clamped by the engine, and does not need to be: a
    /// `GtkAdjustment` holds itself inside its own ends.
    fn keep_in_margins(&self, bar: caret::Bar) {
        let Some(adjustment) = self.vadjustment() else {
            // Not in a scroller: there is nowhere for the band to move to.
            return;
        };
        let (row_top, row_height) = self.row_of(bar);
        if let Some(target) = typography::band_target(
            row_top,
            row_height,
            adjustment.value(),
            adjustment.page_size(),
        ) {
            adjustment.set_value(target);
        }
    }

    /// The caret's row as the vertical adjustment counts it: `(top, height)`
    /// in logical pixels from the top of the page.
    ///
    /// The bar is device pixels in buffer coordinates, which is what
    /// `iter_location` answers in and what the layers are drawn in; the
    /// adjustment counts logical pixels from the top of the page, which is
    /// the page's top margin above the buffer's first row
    /// ([`typography::page_top`]). Measured with `--typewriter` on
    /// `ref/sample.md`: the bar the machine held was 148 device pixels above
    /// where the shot drew it, at scale 2 with a pitch of 37 — the two pitches
    /// of air the page opens with. So the row comes back through the scale
    /// and down by the margin, and a band or a hold reads it where the writer
    /// sees it.
    fn row_of(&self, bar: caret::Bar) -> (f64, f64) {
        let scale = self.scale();
        (bar.y / scale + f64::from(self.top_margin()), bar.h / scale)
    }

    /// Moves the view to where `hold` keeps the caret's row, gliding there
    /// when `glides` says the move is one to be seen and jumping otherwise.
    ///
    /// The target is clamped to the adjustment's ends here rather than left
    /// to the adjustment, because a glide is measured from where it leaves
    /// to where it lands: one that aimed past the end of the page would spend
    /// its last frames easing toward a place the view had already stopped at.
    /// A glide already in flight is retargeted from where it has got to
    /// ([`Glide::retarget`]), so a caret moving mid-glide bends the travel
    /// rather than restarting it.
    fn follow(&self, bar: caret::Bar, hold: Hold, glides: bool) {
        let Some(adjustment) = self.vadjustment() else {
            // Not in a scroller: there is nowhere for the row to be held.
            return;
        };
        let (row_top, row_height) = self.row_of(bar);
        let Some(target) = typewriter::target(
            row_top,
            row_height,
            adjustment.value(),
            adjustment.page_size(),
            hold,
        ) else {
            return;
        };
        let end = (adjustment.upper() - adjustment.page_size()).max(adjustment.lower());
        let target = target.clamp(adjustment.lower(), end);
        if !glides {
            self.imp().glide.set(None);
            adjustment.set_value(target);
            return;
        }
        let now = self.now();
        let glide = match self.imp().glide.get() {
            Some(scroll) => {
                scroll
                    .glide
                    .retarget(millis_since(scroll.started, now), target, row_height)
            }
            None => Glide::new(adjustment.value(), target, row_height),
        };
        self.imp().glide.set(Some(Travel {
            glide,
            started: now,
        }));
        self.start_glide();
    }

    /// Starts the frame-clock callback that carries a glide, unless one is
    /// already attached: the shape of [`Editor::start_fade`].
    fn start_glide(&self) {
        self.keep_ticking(|editor| &editor.imp().gliding, Self::advance_glide);
    }

    /// Puts the view where the glide is at `now`, and says whether another
    /// frame is wanted.
    fn advance_glide(&self, now: i64) -> bool {
        let Some(scroll) = self.imp().glide.get() else {
            return false;
        };
        let Some(adjustment) = self.vadjustment() else {
            self.imp().glide.set(None);
            return false;
        };
        let elapsed = millis_since(scroll.started, now);
        adjustment.set_value(scroll.glide.at(elapsed));
        if scroll.glide.arrived(elapsed) {
            self.imp().glide.set(None);
            return false;
        }
        true
    }

    /// Turns Typewriter on or off, and says where it holds the row.
    ///
    /// The row is put where the modes now hold it ([`Editor::settle`]): on
    /// brings it to the anchor from wherever it is, so that `Ctrl+T` is seen
    /// to do something, and off leaves it to Focus's edge band or to nothing.
    pub fn set_typewriter(&self, typewriter: Typewriter) {
        self.imp().typewriter.set(typewriter);
        self.settle();
    }

    /// The bar is where it was, but the page under it moved.
    ///
    /// A relayout or a change of type puts the same caret at a new place on
    /// the glass, and nothing travelled to get there — the row did. So it is
    /// put there, whatever the hand was last doing: a window resized just
    /// after a word jump must not glide the caret across the new column.
    fn caret_settled(&self) {
        let Some(bar) = self.bar() else {
            return;
        };
        // A relayout that did not move the bar is not news. The machine drops
        // a repeated position but holds the blink on for it either way, which
        // is right for the keystroke that sends the same position twice and
        // wrong here: a window being dragged is allocated on every frame, and
        // the caret would never blink again while a hand was on its edge.
        if self.imp().bar.get() == Some(bar) {
            return;
        }
        self.place_bar(bar, caret::Move::FollowsEdit, self.now());
    }

    /// Hands the machine the bar the layout says is there now.
    fn place_bar(&self, bar: caret::Bar, kind: caret::Move, now: i64) {
        self.imp().bar.set(Some(bar));
        self.tell_caret(|caret| caret.moved(bar, kind, now));
        self.queue_draw();
        self.ask_for_frames();
    }

    /// The buffer changed: the blink is held, and the bar goes to the column
    /// the edit left the caret at.
    ///
    /// The move is taken from here rather than left to `mark-set`, which is
    /// emitted for a mark something *moved* — a click, a cursor key,
    /// `place_cursor` — and not for the insert mark being carried along by an
    /// insertion at its own position. Typing is the second kind and nothing
    /// else is, so a caret that waited for `mark-set` sat wherever it had last
    /// been put while the words went out from under it. The blink was held,
    /// the machine was told, and the bar did not move: the one path the judged
    /// states could not show, because every one of them is a still.
    ///
    /// [`caret::Caret::edited`] comes first because it is what puts the move
    /// about to be made inside the edit-snap window, so the bar is put at the
    /// new column rather than travelling to it, and because the frames the
    /// placement asks for are asked for on what the machine knows by then.
    ///
    /// A task box flipped by a press stands down here as it does in
    /// [`Editor::caret_edit_began`]: the bytes that changed are not the ones
    /// the caret sits on, and a bar told otherwise would take the view back to
    /// a row the writer never left ([`Editor::press`]).
    fn caret_edited(&self) {
        if self.loading() || self.imp().pressing_box.get() {
            return;
        }
        let now = self.now();
        self.tell_caret(|caret| caret.edited(now));
        self.caret_moved();
        // Typing over a selection replaces it, and the marks it collapses can
        // land on the same offsets they were already on — which is a `changed`
        // and no `mark-set` at all, the same gap the placement above closes.
        self.caret_selected();
    }

    /// The buffer's selection opened, changed or collapsed.
    ///
    /// Whether there is one at all is the whole of what the machine is told:
    /// the caret goes out while it stands, and the shape of what is held is
    /// read from the buffer again at every frame it is drawn in, because a
    /// reflow moves the bands without touching either mark.
    fn caret_selected(&self) {
        let now = self.now();
        let held = self.buffer().has_selection();
        self.tell_caret(|caret| caret.selected(held, now));
        self.queue_draw();
        self.ask_for_frames();
    }

    /// Asks for frames while the machine wants them, and for the one frame
    /// that ends the quiet when it does not.
    ///
    /// One of the two places [`caret::Caret::wants_tick`] is read and the tick
    /// source stands or falls by it; the other is the callback's own tail in
    /// [`Editor::start_tick`], which keeps the source while it stays true and
    /// drops it when it does not. Nothing else decides, so a `--deterministic`
    /// window, whose caret is frozen on, asks the frame clock for nothing at
    /// all while it is idle.
    fn ask_for_frames(&self) {
        let caret = self.imp().caret.get();
        if caret.wants_tick() {
            self.start_tick();
        } else if let Some(when) = caret.resumes_at() {
            self.wake_at(when);
        }
    }

    /// Attaches the frame-clock callback, if it is not already attached.
    ///
    /// Every frame it is attached for is a frame in which the blink or the
    /// glide has moved, which is why it may redraw on all of them — and why
    /// it is not attached at all while a hand is typing, when the caret is
    /// simply put where the glyph is and held on.
    fn start_tick(&self) {
        if self.imp().ticking.replace(true) {
            return;
        }
        self.add_tick_callback(|editor, clock| {
            editor.tell_caret(|caret| caret.tick(clock.frame_time()));
            editor.queue_draw();
            let caret = editor.imp().caret.get();
            if caret.wants_tick() {
                return glib::ControlFlow::Continue;
            }
            editor.imp().ticking.set(false);
            if let Some(when) = caret.resumes_at() {
                editor.wake_at(when);
            }
            glib::ControlFlow::Break
        });
    }

    /// Asks for one frame at `when`, on the frame clock's clock.
    ///
    /// The quiet after a move or an edit is the one stretch in which the
    /// caret has something coming and wants no frames until it comes, so it
    /// is the one place a timer belongs — as it does in the oracle, whose
    /// `setTimeout` covers the same 480 ms. All this one does is put the tick
    /// source back; whether the quiet is really over is settled by the frame
    /// times the callback is then handed, so a timer that fires early costs a
    /// frame rather than a wrong blink, and a window with no frames at all
    /// cannot be woken into a loop.
    fn wake_at(&self, when: i64) {
        if self.imp().resume.borrow().is_some() {
            return;
        }
        let left = u64::try_from(when - self.now()).unwrap_or(0);
        let id = glib::timeout_add_local_once(
            Duration::from_micros(left),
            glib::clone!(
                #[weak(rename_to = editor)]
                self,
                move || {
                    editor.imp().resume.take();
                    editor.start_tick();
                },
            ),
        );
        self.imp().resume.replace(Some(id));
    }

    /// Paints the bar the machine says is there, and nothing else.
    ///
    /// Back into the widget's own pixels on the way out. The x the machine
    /// hands back is on a whole device pixel, which on a scale-2 output is
    /// every half of a logical one — which is the point of having snapped it
    /// there rather than here.
    fn draw_caret(&self, snapshot: &gtk::Snapshot) {
        let now = self.now();
        let caret = self.imp().caret.get();
        let alpha = caret.alpha(now);
        if alpha <= 0.0 {
            return;
        }
        draw_box(
            snapshot,
            &paint(&self.colours(), Role::Accent, alpha),
            caret.rect(now),
            self.scale(),
        );
    }

    /// Paints the selection, under the glyphs: the fill, which is all of it.
    ///
    /// One [`Editor::selection`] walk per frame, on the one layer. The walk is
    /// bounded by the visible band and `MAX_ROWS`, and with no selection open
    /// it is a `selection_bounds()?` and a return — which is every frame the
    /// writer is only typing.
    fn draw_selection_fill(&self, snapshot: &gtk::Snapshot) {
        let Some(selection) = self.selection() else {
            return;
        };
        let scale = self.scale();
        let fill = selection_fill(&self.colours(), self.imp().caret.get().focused());
        for row in &selection.rows {
            draw_box(snapshot, &fill, *row, scale);
        }
    }

    /// Paints what Live left standing in the cells its fold emptied.
    ///
    /// One pass over the furnishings the last Document pass worked out
    /// ([`Editor::refurnish`]), clipped to the offsets the viewport holds, so
    /// a manuscript of a thousand list items pays for the dozen rows on the
    /// glass. With Live off the list is empty, which is a borrow and a return.
    ///
    /// Body ink and not a tint of it: `docs/design.md` row Markers rests every
    /// mark at the body's own ink, and a bullet standing where a `-` stood is
    /// the same mark drawn another way. The link rule is the exception the
    /// spec names — the accent, the one colour the writer already reads as the
    /// app speaking rather than the page.
    fn draw_furniture(&self, snapshot: &gtk::Snapshot) {
        let furniture = self.imp().furniture.borrow();
        if furniture.is_empty() {
            return;
        }
        let seen = self.seen();
        let colours = self.colours();
        for standing in furniture.iter() {
            if standing.at.end < seen.start || standing.at.start > seen.end {
                continue;
            }
            match &standing.what {
                Furnish::Bullet => self.draw_bullet(snapshot, &standing.at, &colours),
                Furnish::Number { count, delimiter } => {
                    self.draw_number(snapshot, &standing.at, &colours, *count, *delimiter);
                }
                Furnish::Checkbox { checked, .. } => {
                    if let Some(box_at) = standing.box_cells() {
                        self.draw_checkbox(snapshot, &box_at, &colours, *checked);
                    }
                }
                Furnish::Hairline => self.draw_hairline(snapshot, &standing.at, &colours),
                // A link's rule is a `GtkTextTag` and not a box drawn here
                // ([`tags::link_rule`]), because the words it stands under are
                // the one piece of furniture that is not in a marker's cells:
                // they sit after the folded `[`, and `iter_location` answers
                // for the bytes as if nothing on the line were invisible, so
                // it cannot say where they are. Pango can, and an underline is
                // what Pango is for.
                Furnish::Link { .. } => {}
            }
        }
    }

    /// The buffer offsets the viewport holds, with a row of slack at each end.
    ///
    /// The clip [`Editor::draw_furniture`] reads, and it is offsets rather
    /// than pixels because a furnishing is held at an offset: asking the view
    /// for the two ends once is one lookup, and asking every furnishing for
    /// its rectangle is one lookup each.
    ///
    /// A point off the text — a y above the first row or below the last, and
    /// any x left of the centred column, x = 0 among them — is not over a
    /// character, and GTK answers nothing for it; the whole Document is the
    /// honest reading of that, and the walk below drops what turns out to be
    /// off the glass.
    fn seen(&self) -> Range<i32> {
        let view = self.visible_rect();
        let slack = f64::from(self.imp().pitch.get()) / self.scale();
        let top = f64::from(view.y()) - slack;
        let bottom = f64::from(view.y() + view.height()) + slack;
        let first = self
            .iter_at_location(0, buffer_px(top.max(0.0)))
            .map_or(0, |at| at.offset());
        let last = self
            .iter_at_location(0, buffer_px(bottom))
            .map_or(i32::MAX, |at| at.offset());
        first..last
    }

    /// A bullet item's dot, in the cell its marker's `-`, `*` or `+` stood in.
    ///
    /// A drawn disc rather than a `•` laid out, because the six Faces are the
    /// writer's choice and the dot is not: a glyph would be a different size
    /// and a different weight in each of them, and this is furniture the app
    /// draws rather than type the writer set.
    ///
    /// Centred on the x-height rather than on the box, which is where a
    /// reader's eye puts a bullet: half an x-height above the baseline, and
    /// the baseline is where [`Editor::bar`] takes it from.
    fn draw_bullet(&self, snapshot: &gtk::Snapshot, at: &Range<i32>, colours: &Colours) {
        let Some(cell) = self.cells(at) else {
            return;
        };
        let em = self.em() / self.scale();
        let side = (em * BULLET).max(1.0);
        let x = cell.x + (cell.w - side) / 2.0;
        let y = cell.y + self.imp().baseline.get() - em * X_HEIGHT - side / 2.0;
        // Already in the widget's own pixels — the em above is divided by the
        // scale — so the narrowing is [`logical`] at a scale of one, which is
        // what [`Editor::draw_number`] hands its own two lengths through.
        let side_px = logical(side, 1.0);
        let rect = graphene::Rect::new(logical(x, 1.0), logical(y, 1.0), side_px, side_px);
        snapshot.push_rounded_clip(&gsk::RoundedRect::from_rect(rect, side_px / 2.0));
        snapshot.append_color(&paint(colours, Role::Mark, 1.0), &rect);
        snapshot.pop();
    }

    /// An ordered item's `count` and the `delimiter` the source closed it
    /// with, in the cells its marker stood in.
    ///
    /// The delimiter is carried rather than assumed, because both of
    /// CommonMark's are a writer's own choice: a list written `1)` reads `1)`
    /// folded, where a drawn `.` would be the app rewriting the page.
    ///
    /// The one piece of furniture that is type: a number is read, so it is
    /// laid out in the page's own face at the page's own size, through the
    /// widget's context rather than a description built here, because these
    /// glyphs stand beside the item's words and have to be the same ink.
    ///
    /// Hung on the baseline rather than dropped from the top of the box: the
    /// two layouts are the same type and the offset is nothing, but a heading
    /// or a Face whose metrics disagree would put the number off the row, and
    /// a baseline is the one line every row agrees on.
    fn draw_number(
        &self,
        snapshot: &gtk::Snapshot,
        at: &Range<i32>,
        colours: &Colours,
        count: u32,
        delimiter: char,
    ) {
        let Some(cell) = self.cells(at) else {
            return;
        };
        let layout = self.create_pango_layout(Some(&format!("{count}{delimiter}")));
        let baseline = f64::from(layout.baseline()) / f64::from(pango::SCALE);
        snapshot.save();
        snapshot.translate(&graphene::Point::new(
            logical(cell.x, 1.0),
            logical(cell.y + self.imp().baseline.get() - baseline, 1.0),
        ));
        snapshot.append_layout(&layout, &paint(colours, Role::Mark, 1.0));
        snapshot.restore();
    }

    /// A task item's box, empty or ticked, centred in the cells its `[ ]` or
    /// `[x]` stood in.
    ///
    /// Four rules and, when it is ticked, a fill inside them. Drawn as the
    /// caret and the selection are — device pixels, snapped, back through
    /// [`draw_box`] — because a box whose sides land on a fraction of a device
    /// pixel is a box with two grey sides and two black ones.
    fn draw_checkbox(
        &self,
        snapshot: &gtk::Snapshot,
        at: &Range<i32>,
        colours: &Colours,
        checked: bool,
    ) {
        let Some(cell) = self.cells(at) else {
            return;
        };
        let scale = self.scale();
        let em = self.em();
        let side = caret::snap(em * CHECKBOX).max(1.0);
        let rule = (em / 16.0).round().max(1.0);
        let middle = cell.y + self.imp().baseline.get() - em / scale * X_HEIGHT;
        let x = caret::snap((cell.x + cell.w / 2.0) * scale - side / 2.0);
        let y = caret::snap(middle * scale - side / 2.0);
        let side = side.max(rule * 3.0);
        let ink = paint(colours, Role::Mark, 1.0);
        for edge in [
            caret::Bar {
                x,
                y,
                w: side,
                h: rule,
            },
            caret::Bar {
                x,
                y: y + side - rule,
                w: side,
                h: rule,
            },
            caret::Bar {
                x,
                y,
                w: rule,
                h: side,
            },
            caret::Bar {
                x: x + side - rule,
                y,
                w: rule,
                h: side,
            },
        ] {
            draw_box(snapshot, &ink, edge, scale);
        }
        if !checked {
            return;
        }
        let inset = caret::snap(rule * 2.0);
        draw_box(
            snapshot,
            &ink,
            caret::Bar {
                x: x + inset,
                y: y + inset,
                w: side - inset * 2.0,
                h: side - inset * 2.0,
            },
            scale,
        );
    }

    /// The rule a thematic break draws, across the measure, where its `---`
    /// stood.
    ///
    /// The measure and not the marker's own cells: a break is the width of the
    /// page it breaks. The row's own band gives it its height, which is the
    /// band the caret and the selection take on that row, so the rule sits on
    /// the line's middle however the type is set.
    fn draw_hairline(&self, snapshot: &gtk::Snapshot, at: &Range<i32>, colours: &Colours) {
        let Some(cell) = self.cells(at) else {
            return;
        };
        let Some(page) = self.imp().laid_out.get() else {
            return;
        };
        let scale = self.scale();
        let (top, height) = self.band(&cell.row);
        let rule = scale.max(1.0);
        let x = caret::snap(f64::from(page.column.left) * scale);
        let right = caret::snap(f64::from(page.column.right) * scale);
        draw_box(
            snapshot,
            &paint(colours, Role::Rule, 1.0),
            caret::Bar {
                x,
                y: caret::snap(top + (height - rule) / 2.0),
                w: (right - x).max(0.0),
                h: rule,
            },
            scale,
        );
    }

    /// The cells the buffer offsets `at` are drawn in.
    ///
    /// One row, because every furnishing that is drawn here stands in a
    /// **block-leading** marker's cells: a bullet, a number, a task box and a
    /// thematic break are all the first thing on their block's first row, and
    /// none of them is long enough to wrap.
    ///
    /// That is also what makes `iter_location` safe to ask. It answers for an
    /// offset as though nothing on the line were invisible — measured on
    /// #274's own passage, where the offsets after a folded `](…)` all came
    /// back at the end of the row — and a block-leading marker has nothing
    /// folded before it, so its own cells are the ones GTK says they are.
    /// `iter_at_location`, which the press reads, has no such trouble.
    fn cells(&self, at: &Range<i32>) -> Option<Cells> {
        if at.end <= at.start {
            return None;
        }
        let buffer = self.buffer();
        let head = self.iter_location(&buffer.iter_at_offset(at.start));
        let mut last = buffer.iter_at_offset(at.end);
        last.backward_char();
        let tail = self.iter_location(&last);
        Some(Cells {
            x: f64::from(head.x()),
            y: f64::from(head.y()),
            w: f64::from((tail.x() + tail.width() - head.x()).max(0)),
            row: head,
        })
    }

    /// The empty page's one whisper: [`PLACEHOLDER`] on the first line, where
    /// the first glyph will land, in the grey Focus dims prose to.
    ///
    /// The Parity oracle sets it there (`legacy/app/css/page.css`, the
    /// `#mirror` rule for a Document with nothing in it) and it is gone the
    /// moment there is a character in the Document. It is the chrome Piece's
    /// to paint — #43 moved `empty` there — and the Editor's to place, because
    /// only the Editor knows where its first line is. Under the glyphs, so
    /// the caret is drawn over it like over any text.
    fn draw_placeholder(&self, snapshot: &gtk::Snapshot) {
        let buffer = self.buffer();
        if buffer.char_count() != 0 {
            return;
        }
        let at = self.iter_location(&buffer.start_iter());
        let layout = self.create_pango_layout(Some(PLACEHOLDER));
        snapshot.save();
        // Buffer coordinates are whole logical pixels, which `f32` holds.
        snapshot.translate(&graphene::Point::new(
            logical(f64::from(at.x()), 1.0),
            logical(f64::from(at.y()), 1.0),
        ));
        snapshot.append_layout(&layout, &paint(&self.colours(), Role::InkDim, 1.0));
        snapshot.restore();
    }

    /// The mode `--deterministic` and `--nocaret` asked for.
    ///
    /// A whole machine rather than a mode set on the one there is, because
    /// this is read before the first frame and a machine that has been in
    /// another mode has been keeping the wrong kind of state.
    pub fn set_mode(&self, mode: caret::Mode) {
        // The one place the whole widget hears what kind of launch this is, so
        // the cross-fade reads its half of the answer from here rather than
        // being handed the flags a second time.
        self.imp()
            .deterministic
            .set(matches!(mode, caret::Mode::Deterministic));
        self.imp().caret.set(caret::Caret::new(mode));
        self.tell_caret(|caret| caret.resize(self.em()));
    }

    /// The window this Editor is in became active, or stopped being.
    ///
    /// The one place the ghost is decided. The caret's machine drops to its
    /// ghost alpha with the blink stopped, and the selection's fill swaps to
    /// the idle band, because a window that has lost focus should
    /// say where the writer was without shouting it. Both return on focus.
    ///
    /// The machine's own flag is the whole of the state: the selection is
    /// painted here rather than by GTK, so there is nothing left for a CSS
    /// class to reach — [`Editor::draw_selection`] asks
    /// [`caret::Caret::focused`] the same question the caret's alpha does.
    pub fn set_active(&self, active: bool) {
        let now = self.now();
        self.tell_caret(|caret| caret.focus(active, now));
        self.queue_draw();
        self.ask_for_frames();
    }

    /// The app is about to place the caret itself, with no hand behind it.
    ///
    /// [`caret::Source`] is sticky: a controller sets it and nothing clears
    /// it, so once the writer has pressed a key every later placement would
    /// read as the writer's own. The places that move the caret without a hand
    /// — the launch flags, and later a restored position or a Command — say so
    /// here, which is what lets [`Editor::keep_in_band`] trust the answer
    /// rather than only being right until the first keystroke.
    fn placing(&self) {
        self.imp().last.set(caret::Source::App);
    }

    /// Selects `from` to `to`, in UTF-8 bytes, as `--select` asked.
    ///
    /// The insert mark goes to `to` and the bound to `from`, which is where a
    /// drag or a Shift+arrow leaves them: the selection is GTK's own from
    /// here, painted beneath the glyphs, and the caret is at the end the hand
    /// was moving.
    pub fn select(&self, document: &Document, from: u64, to: u64) {
        self.placing();
        let buffer = self.buffer();
        let bound = tags::iter_at(&buffer, document, byte_offset(from));
        let insert = tags::iter_at(&buffer, document, byte_offset(to));
        buffer.select_range(&insert, &bound);
    }

    /// Puts the caret where `--caret` asked for it, and shows where it went.
    ///
    /// The harness names an offset in UTF-8 bytes, because that is what a
    /// Document is measured in everywhere else, and it is reached through the
    /// Document's own line table by [`tags::iter_at`] — the one byte-to-iter
    /// mapping the app has.
    ///
    /// `reveal` scrolls the view to the caret, because a caret the harness
    /// cannot see is not the state it asked for: a bench typing at the end of
    /// a draft has to be looking at the end of the draft. It is false when
    /// `--scroll` has said where the view goes, since a state that names both
    /// means both, and a judged shot of a passage the opponent is not showing
    /// is not a comparison.
    pub fn place_caret(&self, document: &Document, caret: flags::Caret, reveal: bool) {
        self.placing();
        let buffer = self.buffer();
        let at = match caret {
            flags::Caret::End => buffer.end_iter(),
            flags::Caret::At(offset) => tags::iter_at(&buffer, document, byte_offset(offset)),
        };
        buffer.place_cursor(&at);
        self.imp().reveal_owed.set(reveal);
        if reveal {
            self.reveal_caret();
        }
    }

    /// Scrolls the view to the caret, but never before the page is laid out.
    ///
    /// GTK resolves a `scroll_to_mark` against the layout the view has, and
    /// until the first `size_allocate` it has none: the request is queued as
    /// a pending scroll and flushed when the view is next validated. On a
    /// Document shorter than the viewport that flush never happens — the
    /// scroll it asks for is a scroll there is no room to make — and the
    /// validation it is holding up is the one that draws the text. What
    /// reaches the glass is then anything from a sliver of one row to
    /// nothing at all: #148's own shot has 92x29 of ink in it, the shot on
    /// its triage comment none, and measured here the window is `#F9F9F9`
    /// edge to edge. A long Document was never hit by it because there the
    /// scroll does move, which flushes the queue and lets the validation
    /// through.
    ///
    /// So the reveal waits for a size the way [`Editor::lay_out`] does, and
    /// `size_allocate` asks again the moment there is one. Measured on a
    /// Document of three paragraphs, `--caret 0`, `--caret 10` and
    /// `--caret end` each leave the window on the same ink `--scroll 0`
    /// leaves it on, and `ref/sample.md` does not move.
    ///
    /// One request is not enough on a long Document, though (#221). GTK
    /// resolves it against the layout it has, and on 10,062 words the
    /// paragraphs above the caret are still at their estimated heights — one
    /// row each, where they wrap to a dozen. As they validate, the view keeps
    /// its first visible paragraph where it was and the caret's row moves
    /// down under the ones between growing: launched Live at `--caret 26000`
    /// the bar was drawn at buffer y 13156 with the viewport held at
    /// 2578–3478, and nothing asked again. Every keystroke does, which is why
    /// no bench regime ever saw it; an idle window blinked off the glass and
    /// presented no frame. So the reveal is asked again on each frame after,
    /// the way [`Editor::scroll_to`] holds its own, until the row has held
    /// still from one frame to the next and is on the glass
    /// ([`Editor::reveal_settled`]), or [`REVEAL_FRAMES`] run out.
    fn reveal_caret(&self) {
        if !self.imp().reveal_owed.get() || self.imp().laid_out.get().is_none() {
            return;
        }
        self.imp().reveal_owed.set(false);
        self.ask_reveal();
        let landing = Cell::new(Landing::default());
        self.over_frames(REVEAL_FRAMES, move |editor| editor.reveal_settled(&landing));
    }

    /// Asks GTK for the scroll that shows the caret's row.
    ///
    /// With Typewriter on the first frame shows the row where every frame
    /// after will hold it, so `--typewriter` is the state and not a jump from
    /// the top to it; the frames after are [`Editor::hold_row`]'s, which
    /// places the row itself, and the reveal's re-asks defer to it there.
    fn ask_reveal(&self) {
        let line = match self.imp().typewriter.get() {
            Typewriter::On(anchor) if self.hold_row_now(anchor) => return,
            Typewriter::On(anchor) => anchor,
            Typewriter::Off => CARET_LINE,
        };
        self.scroll_to_mark(&self.buffer().get_insert(), 0.0, true, 0.0, line);
    }

    /// One frame of the reveal's hold: says whether the caret's row has
    /// landed, and asks again when it has not.
    ///
    /// Landed is two things read off the same frame. The layout's height and
    /// the bar's top in buffer pixels are what validation moves, and
    /// `landing` carries the last frame's pair and how many frames running it
    /// has held ([`Landing::stood`]); once it has held for [`STILL_FRAMES`],
    /// the paragraphs above the caret have their real heights and a scroll
    /// resolved now stays resolved. And the row is on the glass
    /// ([`on_glass`]), which is what the reveal was for. A Document shorter
    /// than its viewport passes both on the third frame, wherever the view
    /// is, so #148's guarantee is asked for twice more and not moved.
    fn reveal_settled(&self, landing: &Cell<Landing>) -> bool {
        let Some(adjustment) = self.vadjustment() else {
            // Not in a scroller: there is nowhere for the row to be shown.
            return true;
        };
        if let Some(bar) = self.bar() {
            let stood = landing.get().stood(adjustment.upper(), bar.y);
            landing.set(stood);
            let view = (adjustment.value(), adjustment.page_size());
            if stood.still >= STILL_FRAMES && on_glass(self.row_of(bar), view) {
                return true;
            }
        }
        // Before the type is set there is no bar to read, and the frame is
        // spent asking anyway.
        self.ask_reveal();
        false
    }

    /// Scrolls the Document to `fraction` of its length, 0 at the top.
    ///
    /// The fraction is a fraction of the scroll, not of the text: at 0 the
    /// view is at the top of the page, showing the air above the first row,
    /// which is where the oracle puts it and is not where the first row is.
    ///
    /// Held for [`SCROLL_FRAMES`] rather than set once. A view GTK has not
    /// laid out yet cannot say how far it can go, and GTK makes a scroll of
    /// its own on the way there — the one that keeps the caret on screen when
    /// the Editor takes focus, resolved while the layout is validated and so
    /// after this is read. Holding the view for a few frames outlasts it, and
    /// is over long before the harness, which waits for a still window, takes
    /// its shot.
    pub fn scroll_to(&self, fraction: f64) {
        let Some(adjustment) = self.vadjustment() else {
            // Not in a scroller: there is nowhere to scroll to.
            return;
        };
        let fraction = fraction.clamp(0.0, 1.0);
        self.imp().pinned.set(true);
        self.over_frames(SCROLL_FRAMES, move |_| {
            let room = adjustment.upper() - adjustment.lower() - adjustment.page_size();
            adjustment.set_value(adjustment.lower() + room.max(0.0) * fraction);
            false
        });
    }

    /// The block the Editor's top edge falls in at scroll `offset`, as a
    /// scroll-sync rule reads a driving pane ([`quill_engine::sync`], #270).
    ///
    /// One block rather than the whole page, because one is all
    /// [`sync::follow_top_block`] reads of its driver — whichever block the
    /// top edge is in, and how far into it — and a manuscript's index is
    /// thousands of blocks long. The view is asked which block that is
    /// (`line_at_y`) rather than walked to it, so a wheel event costs two row
    /// rectangles however long the Document is. The row is asked for by y
    /// alone: the Editor centres its column in a left margin, so every x this
    /// side of the text — x = 0 included — is off the line, and the lookups
    /// that take an x answer nothing there.
    #[must_use]
    pub fn top_block(&self, document: &Document, offset: f64) -> Option<sync::Block> {
        let y = offset - f64::from(self.top_margin());
        let (at, _) = self.line_at_y(buffer_px(y.max(0.0)));
        let key = document.block_at(tags::offset_of(document, &at))?;
        self.block_row(document, key)
    }

    /// Where block `key` stands in the Editor's scroll coordinate: the answer
    /// a scroll-sync rule reads of the Editor as the following pane (#270).
    ///
    /// The rectangle is the view's own — `iter_location` at the block's first
    /// byte and its last — taken in buffer coordinates and put in the
    /// adjustment's by the top margin, which is the page's air above the first
    /// row and the one difference between the two ([`Editor::row_of`] says the
    /// same of the caret's).
    #[must_use]
    pub fn block_row(&self, document: &Document, key: usize) -> Option<sync::Block> {
        // The index tiles the Document, so the end of the text names its last
        // block: a key past that one is a page laid out before an edit shrank
        // the Document, and asking for it would panic.
        if key > document.block_at(document.text().len())? {
            return None;
        }
        let block = document.block(key);
        let buffer = self.buffer();
        let from = tags::iter_at(&buffer, document, block.at.start);
        let mut to = tags::iter_at(&buffer, document, block.at.end);
        if to.offset() > from.offset() {
            // A block ends where the next one begins, and that byte is on the
            // next block's first row: the foot of this one is the row its own
            // last character is on.
            to.backward_char();
        }
        let head = self.iter_location(&from);
        let foot = self.iter_location(&to);
        let margin = f64::from(self.top_margin());
        let top = f64::from(head.y()) + margin;
        let bottom = f64::from(foot.y()) + f64::from(foot.height()) + margin;
        Some(sync::Block::new(key, top, (bottom - top).max(0.0)))
    }

    /// Every block of `document` as a vertical range in the Editor's scroll
    /// coordinate: the Editor's side of the top-block rule when the Preview is
    /// the pane being scrolled (#270).
    ///
    /// The whole page here, because a follower is asked for a block the driver
    /// names and any block can be named. It walks the index, so it belongs to
    /// a wheel or a scrollbar and never to the keystroke lane: what an edit
    /// drives is the caret rule, which asks for one row.
    #[must_use]
    pub fn block_rows(&self, document: &Document) -> Vec<sync::Block> {
        let Some(last) = document.block_at(document.text().len()) else {
            return Vec::new();
        };
        (0..=last)
            .filter_map(|key| self.block_row(document, key))
            .collect()
    }

    /// Where the caret's row stands down the Editor's viewport: 0 at the top
    /// edge, 1 at the foot.
    ///
    /// The fraction [`sync::follow_caret`] puts the caret's block at down the
    /// Preview, so the block being written stays where the eye already is
    /// (#270). `None` before the type has been set or outside a scroller,
    /// where there is no row and no viewport to place it in.
    #[must_use]
    pub fn caret_fraction(&self) -> Option<f64> {
        let adjustment = self.vadjustment()?;
        let viewport = adjustment.page_size();
        if viewport <= 0.0 {
            return None;
        }
        let (top, _) = self.row_of(self.bar()?);
        Some(((top - adjustment.value()) / viewport).clamp(0.0, 1.0))
    }
}

/// The margins a page was laid out with, in the pixels GTK takes.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct Page {
    /// The text container the margins were counted off: its
    /// [`side`](typography::Column::side) is `left-margin` and `right-margin`,
    /// which are the same because the measure is centred, and its gutter is
    /// what a heading's markers hang into (ADR 0016). The whole `Column` is
    /// kept rather than the one edge, so that two windows agreeing on the
    /// measure but not on the gutter are two layouts and not one.
    column: typography::Column,
    /// `bottom-margin`: the air below the last row of the Document.
    bottom: i32,
}

/// What Live leaves standing on the bytes `over` of `document`, for a writer at
/// `at`, each with the Document bytes of the cells it stands in.
///
/// [`Editor::refurnish`] without the buffer: the whole of the answer that can
/// be worked out from the text, so that the widget's one crossing into the
/// offsets GTK counts is the last step and everything before it is testable
/// with no display attached.
///
/// Bounded by `over`, through [`annotate::live::spans_in`], which widens it to
/// the blocks it touches: this runs on the keystroke lane, so it pays for the
/// page on the glass and not for the manuscript.
fn standing(
    document: &Document,
    at: &Range<usize>,
    over: &Range<usize>,
) -> Vec<(Range<usize>, Furnish)> {
    let text = document.text();
    let spans = annotate::live::spans_in(document, at, over);
    // The one whole-document question Live asks, asked once for the page rather
    // than once for each link: resolving a reference label is a parse of the
    // file ([`markdown::references`]), and a page can hold a dozen of them.
    // Nothing is parsed at all when no reference link is standing, which is
    // every page that writes its addresses inline.
    let defined = if spans.iter().any(|span| labelled(text, span)) {
        markdown::references(text)
    } else {
        BTreeMap::new()
    };
    spans
        .into_iter()
        .filter_map(|span| {
            let LiveLook::Furniture(furniture) = span.look else {
                return None;
            };
            let what = Furnish::of(text, &span.at, &furniture, &defined)?;
            Some((cells_of(text, &span.at, &what), what))
        })
        .collect()
}

/// Whether `span` is a link whose destination is a reference label rather than
/// an address, and so wants the file's definitions to resolve it ([`address`]).
fn labelled(text: &str, span: &LiveSpan) -> bool {
    let LiveLook::Furniture(Furniture::Link { destination }) = &span.look else {
        return false;
    };
    text[..destination.start].ends_with('[')
}

impl Furnish {
    /// What the `furniture` Live put on the bytes `at` of `text` stands for.
    ///
    /// `None` twice: a fence's furniture is the Well the code already stands
    /// on, so there is nothing left to draw where the backticks were; and a
    /// link whose destination resolves to nothing has nothing to open, so it
    /// keeps its words and takes no rule. `defined` is the file's link-reference
    /// definitions, which is what a reference link resolves through.
    fn of(
        text: &str,
        at: &Range<usize>,
        furniture: &Furniture,
        defined: &BTreeMap<String, String>,
    ) -> Option<Self> {
        Some(match furniture {
            Furniture::Bullet => Self::Bullet,
            Furniture::Number { count, delimiter } => Self::Number {
                count: *count,
                delimiter: *delimiter,
            },
            Furniture::Hairline => Self::Hairline,
            Furniture::Fence => return None,
            Furniture::Checkbox { checked } => {
                let box_at = brackets(text, at)?;
                Self::Checkbox {
                    box_at: cell(box_at.start - at.start)..cell(box_at.end - at.start),
                    checked: *checked,
                }
            }
            Furniture::Link { destination } => Self::Link {
                destination: address(text, destination, defined)?,
            },
        })
    }
}

/// The cells `what` stands in, of a furniture span over the bytes `at`.
///
/// The span itself for a checkbox and for a link — a press anywhere on a task
/// item's marker flips its box, and a link's words are the words — and the
/// marker's ink alone for the other three.
/// [`quill_engine::annotate::Mark::BulletMarker`] is measured from the start
/// of the line, indent and all, so that the app can hang the item by its
/// width; a dot drawn in the first cell of *that* would stand out in a nested
/// item's indent rather than where its `-` was.
fn cells_of(text: &str, at: &Range<usize>, what: &Furnish) -> Range<usize> {
    match what {
        Furnish::Checkbox { .. } | Furnish::Link { .. } => at.clone(),
        _ => ink(text, at),
    }
}

/// A count of cells inside one marker run, as the buffer counts them.
///
/// The run is ASCII, so its bytes and its cells are the same count: see
/// [`Furnish::Checkbox`] for why the box is held relative to the marker at
/// all.
fn cell(count: usize) -> i32 {
    i32::try_from(count).unwrap_or(0)
}

/// `at` with the whitespace at either end of it taken off.
fn ink(text: &str, at: &Range<usize>) -> Range<usize> {
    let Some(run) = text.get(at.clone()) else {
        return at.clone();
    };
    let start = at.start + (run.len() - run.trim_start().len());
    start..start + run.trim().len()
}

/// The `[ ]` or `[x]` inside the marker cells `at`.
///
/// A task item's furniture covers the bullet and the box together — one box
/// stands where both did — and the writer's state is the one byte between the
/// brackets, so the press that flips it wants the brackets and not the marker.
/// Read out of the text rather than carried by the Annotator, which marks the
/// box as one span and has no reason to cut it in half.
fn brackets(text: &str, at: &Range<usize>) -> Option<Range<usize>> {
    let marker = text.get(at.clone())?;
    let open = marker.find('[')?;
    let close = marker[open..].find(']')? + open;
    Some(at.start + open..at.start + close + 1)
}

/// The address the link destination written at `at` opens.
///
/// Three shapes reach here. An inline `[words](https://…)` writes the address
/// itself, with a title after it to cut off and angle brackets to strip. A
/// reference `[words][label]` writes a label instead, and the definition
/// somewhere else in the file is what says where it goes — `defined`, the map
/// [`markdown::references`] builds once for the page ([`labelled`] is the same
/// test made before the map is asked for); the byte before the destination
/// tells the two apart, `(` for an address and `[` for a label. A label nothing
/// defines opens nothing, which is [`None`].
fn address(text: &str, at: &Range<usize>, defined: &BTreeMap<String, String>) -> Option<String> {
    let written = text.get(at.clone())?.trim();
    if written.is_empty() {
        return None;
    }
    if text[..at.start].ends_with('[') {
        return defined.get(&written.to_lowercase()).cloned();
    }
    if let Some(bracketed) = written.strip_prefix('<') {
        return Some(bracketed[..bracketed.find('>')?].to_owned());
    }
    Some(written.split_whitespace().next()?.to_owned())
}

/// Flips the task box at the buffer offsets `box_at` to `checked`.
///
/// **Through the buffer**, which is what the Document's edit path is from a
/// widget: the engine's copy of the text is spliced from this buffer's own
/// `insert-text` and `delete-range` (`quill::window`), undo is
/// `GtkTextBuffer`'s, and autosave is armed by its `changed`. An edit made
/// anywhere else would be a change the writer could not undo and the file
/// would never see.
///
/// One byte inside the brackets rather than the whole box, so that nothing on
/// the line moves under the writer's finger, and both halves inside one
/// `begin_user_action`, so that undo takes the tick off in one press.
///
/// The caller raises `pressing_box` around this call: the edit is the box's
/// and not the caret's, and the caret's machine would otherwise read it as a
/// keystroke and glide the view back to whatever row the caret stands on
/// ([`Editor::press`]).
fn tick(buffer: &gtk::TextBuffer, box_at: &Range<i32>, checked: bool) {
    let (cells, state) = flip(box_at, checked);
    let mut from = buffer.iter_at_offset(cells.start);
    let mut to = buffer.iter_at_offset(cells.end);
    if to <= from {
        return;
    }
    buffer.begin_user_action();
    buffer.delete(&mut from, &mut to);
    buffer.insert(&mut from, state);
    buffer.end_user_action();
}

/// The lines a fold that has just moved from `was` to `now` draws again, or
/// `None` where it moved nowhere.
///
/// The whole of the decision [`Editor::refold`] makes once it knows both, kept
/// out of the method because a buffer is what the rest of that pass needs and
/// this needs nothing: the block the writer left is drawn folded and the one
/// they entered unfolded, and a fold that left the same lines open draws
/// nothing, which is most keystrokes.
fn refolded(was: Option<Range<usize>>, now: Range<usize>) -> Option<Vec<Range<usize>>> {
    if was.as_ref() == Some(&now) {
        return None;
    }
    let mut lines = Vec::with_capacity(2);
    lines.extend(was);
    lines.push(now);
    Some(lines)
}

/// The cells a press rewrites inside the box at `box_at`, and what it writes
/// there.
///
/// The whole of the rewrite that can be said without a buffer, which is why it
/// is here: gtk4-rs will not make a `GtkTextBuffer` before GTK is initialised,
/// and GTK will not initialise without a display, so `tools/gate check` can
/// assert this and the hand test asserts the rest of [`tick`].
fn flip(box_at: &Range<i32>, checked: bool) -> (Range<i32>, &'static str) {
    (
        box_at.start + 1..box_at.end - 1,
        if checked { "x" } else { " " },
    )
}

/// Opens `destination` through the desktop's default handler for it.
///
/// [`gtk::UriLauncher`] rather than the `gio` call the Settings window's
/// "Open settings.toml" uses, for the reason the Preview's own opener has it
/// (`quill::preview`): this is a URI out of a Document and not a file Quill
/// wrote, so it goes through the portal, which is what asks the writer before
/// a strange scheme is handed to anything.
fn open(destination: &str, window: Option<&gtk::Window>) {
    gtk::UriLauncher::new(destination).launch(window, gio::Cancellable::NONE, |_| {});
}

/// A byte offset a flag named, as the Document counts them.
fn byte_offset(bytes: u64) -> usize {
    usize::try_from(bytes).unwrap_or(usize::MAX)
}

/// A view or pointer coordinate as the buffer counts them: whole pixels.
///
/// The rounding every crossing into `GtkTextView`'s own coordinates makes, done
/// once and in one place (`CODING_STANDARDS.md` § Shape): a gesture, a scroll
/// offset and a viewport edge all arrive as fractions of a logical pixel, and
/// `window_to_buffer_coords` and `iter_at_location` count in whole ones. The
/// cast saturates in Rust, so a coordinate no window could hold clamps rather
/// than wrapping.
fn buffer_px(length: f64) -> i32 {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a coordinate inside one window, and the cast saturates either way"
    )]
    let whole = length.round() as i32;
    whole
}

/// A length in device pixels, in the widget's own pixels, as `graphene` takes
/// it.
///
/// The narrowing is the one place it belongs: a snapshot is drawn in `f32`,
/// and a caret's four lengths are three digits at most, so nothing here is
/// near what an `f32` stops counting exactly.
fn logical(device: f64, scale: f64) -> f32 {
    (device / scale) as f32
}

/// A channel of a `quill_engine::theme::Colour` as `gdk` takes it: 0 to 1
/// either way, so the narrowing loses nothing a screen could show.
fn channel(value: f64) -> f32 {
    value as f32
}

/// The colour the selection's fill is painted in, which is the whole of the
/// selection's paint since ADR 0014 took the end bars off it.
///
/// A window that is not active keeps saying what is held, in the paler of the
/// two bands `legacy/app/css/theme.css` sets — the fill is the only mark left
/// to say it with, now that the free caret is out for as long as a selection
/// stands and no bar brackets either end.
///
/// A function of the table and the one flag so that it can be checked without
/// a window: `unfocused` is shot with nothing selected, so no judged state
/// carries the idle fill and the swap is only ever true here.
fn selection_fill(colours: &Colours, focused: bool) -> gdk::RGBA {
    if focused {
        paint(colours, Role::Selection, 1.0)
    } else {
        paint(colours, Role::SelectionIdle, 1.0)
    }
}

/// One role's colour in `colours`, at `alpha` of the alpha the table gives it.
///
/// Read from the ground's table rather than written out here, because the
/// table carries every role and a second copy of a number the critic reads is
/// a second thing to keep true. The roles that reach here are the ones the
/// layers around the glyphs paint: the accent the caret and a link's rule are
/// cut from, the selection's two fills, the mark ink Live's furniture stands
/// in, and the hairline a thematic break draws.
fn paint(colours: &Colours, role: Role, alpha: f64) -> gdk::RGBA {
    let colour = colours.colour(role);
    gdk::RGBA::new(
        channel(colour.red),
        channel(colour.green),
        channel(colour.blue),
        channel(colour.alpha * alpha),
    )
}

/// Draws one box of the caret's layer, back in the widget's own pixels.
///
/// A box with no area is not drawn: an empty row of a selection is a real
/// place in the text — the end of a line whose newline is not held — and it
/// has nothing to paint.
fn draw_box(snapshot: &gtk::Snapshot, colour: &gdk::RGBA, bar: caret::Bar, scale: f64) {
    if bar.w <= 0.0 || bar.h <= 0.0 {
        return;
    }
    snapshot.append_color(
        colour,
        &graphene::Rect::new(
            logical(bar.x, scale),
            logical(bar.y, scale),
            logical(bar.w, scale),
            logical(bar.h, scale),
        ),
    );
}

/// A count of pixels as a GTK widget takes it.
fn signed(count: u32) -> i32 {
    i32::try_from(count).unwrap_or(i32::MAX)
}

/// A GTK widget's own measurement, as the typography takes it. A negative
/// dimension is a widget with no size, which is no room at all.
fn unsigned(pixels: i32) -> u32 {
    u32::try_from(pixels).unwrap_or(0)
}

/// The features as CSS names them, for the stylesheet.
fn css_features() -> String {
    FEATURES
        .map(|feature| format!("\"{feature}\" 0"))
        .join(", ")
}

/// The features as Pango names them, for a layout being measured.
fn pango_features() -> String {
    FEATURES.map(|feature| format!("{feature}=0")).join(",")
}

/// The description body text is laid out with.
///
/// A Face is asked for by family name and never by file: fontconfig already
/// holds the six that [`crate::fonts::load_private`] gave it. The Roman is
/// named explicitly upright and at Regular before the weight axis is set,
/// because a description that leaves either open is one a missing Face can be
/// resolved into obliquely (ADR 0004, ADR 0007). Sizes are absolute pixels;
/// points appear nowhere.
///
/// `em` is the ladder's own value in logical pixels and is fractional — the
/// default step's is 21.33 — so it is handed to Pango as it stands. Rounding
/// it to the whole pixel the type used to be named in would put the page a
/// third of a pixel off the Design oracle at the default and further at other
/// steps, which is the difference the oracles are frozen at.
fn body_font(face: Face, em: f64) -> pango::FontDescription {
    let mut font = pango::FontDescription::new();
    font.set_family(face.family());
    font.set_style(pango::Style::Normal);
    font.set_weight(pango::Weight::Normal);
    font.set_variations(Some(&format!("wght={INK_WEIGHT}")));
    font.set_absolute_size(em * f64::from(pango::SCALE));
    font
}

thread_local! {
    /// The one stylesheet the type is named in, held so that a size step
    /// reloads it rather than stacking another provider on the display behind
    /// it. GTK is one thread, so one place is enough.
    static TYPE_STYLE: RefCell<Option<gtk::CssProvider>> = const { RefCell::new(None) };
}

/// The theme GTK draws its own widgets with: GTK 4's built-in one, whose light
/// and dark variants `gtk-interface-color-scheme` picks between, rather than
/// whatever the desktop is set to ([`install_type`]).
const GTK_THEME: &str = "Default";

/// Names the type on the display: the Face, the size, the paper and the ink.
///
/// The base font goes through CSS rather than through the buffer's tags so
/// that a blank line, which carries no tag, sits on exactly the same metrics
/// as a written one — a page whose empty lines are a different height is not a
/// page. It is installed once, before the first window, and reloaded whenever
/// the writer steps the size.
pub fn install_type(ground: Ground, face: Face, step: u32) {
    let Some(display) = gtk::gdk::Display::default() else {
        // No display: nothing to style, and nothing that will draw text.
        return;
    };
    // The widgets GTK draws for itself — the Settings window's rows, the
    // `Ctrl+?` window, a dialog — are drawn by GTK's theme, which is the
    // desktop's: on a desktop set to `Adwaita-dark` they came out light on
    // dark over the paper the stylesheet below paints. So the theme is pinned
    // to GTK's own and its variant follows the ground, as the stylesheet does.
    if let Some(settings) = gtk::Settings::default() {
        settings.set_gtk_theme_name(Some(GTK_THEME));
        settings.set_gtk_interface_color_scheme(match ground.scheme {
            Scheme::Light => gtk::InterfaceColorScheme::Light,
            Scheme::Dark => gtk::InterfaceColorScheme::Dark,
        });
    }
    TYPE_STYLE.with_borrow_mut(|held| {
        let provider = held.get_or_insert_with(|| {
            let provider = gtk::CssProvider::new();
            gtk::style_context_add_provider_for_display(
                &display,
                &provider,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
            provider
        });
        // The bars' sheet rides with the type's, so the two grounds — the
        // page's and the chrome's — change in the one reload.
        let mut sheet = stylesheet(&ground.colours, face, typography::em(step));
        sheet.push_str(&crate::chrome::stylesheet(ground));
        provider.load_from_string(&sheet);
    });
}

/// The stylesheet [`install_type`] loads.
///
/// The ink is named on the widget and on its text node both. A `GtkTextView`
/// draws untagged text in the colour of one of the two depending on how it
/// came by its attributes, and a page whose ink is the desktop theme's is not
/// this page at all: on a dark desktop it is white on the oracle's paper, and
/// there is nothing to read.
///
/// The selection rule is here to take GTK's ground away and nothing else. The
/// band belongs to [`Editor::selection`], which cuts it off the same band the
/// caret is cut from so the two register exactly; a ground painted here as
/// well would be a second, differently rounded rectangle under it. The
/// `color` stays, for the same reason the ink is named twice above: clearing
/// the ground alone leaves the selected glyphs to the desktop theme.
///
/// The two caret colours are how GTK's own caret is hidden. `cursor-visible`
/// would hide it too, but a `GtkTextView` with that property off answers
/// `move-cursor` by scrolling the viewport rather than moving the insert
/// mark, and the keyboard can then move nothing (#220). Transparent, the bar
/// is laid out and blinked like any other and never lands a pixel; the blink
/// itself is turned off at startup so that it asks for no frames either. The
/// secondary colour is the split caret a bidirectional line shows.
fn stylesheet(colours: &Colours, face: Face, em: f64) -> String {
    let paper = colours.colour(Role::Paper).to_hex();
    let ink = colours.colour(Role::Ink).to_hex();
    format!(
        "window {{ background-color: {paper}; }}\n\
         textview.{FACE_CLASS}, textview.{FACE_CLASS} text {{\n\
         \x20 background-color: {paper};\n\
         \x20 color: {ink};\n\
         \x20 font-family: \"{family}\";\n\
         \x20 font-size: {em}px;\n\
         \x20 font-style: normal;\n\
         \x20 font-weight: {INK_WEIGHT};\n\
         \x20 font-feature-settings: {features};\n\
         \x20 caret-color: transparent;\n\
         \x20 -gtk-secondary-caret-color: transparent;\n\
         }}\n\
         textview.{FACE_CLASS} text selection {{\n\
         \x20 background-color: transparent;\n\
         \x20 color: {ink};\n\
         }}\n",
        family = face.family(),
        features = css_features()
    )
}

impl Default for Editor {
    fn default() -> Self {
        Self::new()
    }
}

/// Where the layout stood on the last frame of a reveal's hold, and for how
/// many frames running it has stood there ([`Editor::reveal_settled`]).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Landing {
    /// The vertical adjustment's upper: the layout's height in logical pixels.
    upper: f64,
    /// The bar's top in buffer device pixels ([`Editor::bar`]).
    bar_top: f64,
    /// Frames running the pair above has been read unchanged. Zero on the
    /// first reading and on any frame that moved either.
    still: u32,
}

impl Landing {
    /// The landing after a frame that read `upper` and `bar_top`.
    fn stood(self, upper: f64, bar_top: f64) -> Self {
        let still = if self.upper == upper && self.bar_top == bar_top {
            self.still + 1
        } else {
            0
        };
        Self {
            upper,
            bar_top,
            still,
        }
    }
}

/// Whether the row `(top, height)` lies whole inside the viewport
/// `(top, height)`, both counted as the vertical adjustment counts them
/// ([`Editor::row_of`]). A row over either edge is not shown, and a reveal
/// that left one there has not landed ([`Editor::reveal_settled`]).
/// The pitch and the baseline of a row whose ink is `ink` tall, on a page
/// whose body row is `body` tall at `pitch` and `baseline`. All in logical
/// pixels.
///
/// The rule [`Editor::row_pitch`] is: a row is the ink it holds plus the air
/// the leading puts above and below it, and that air is a widget property and
/// so is the same on every row. A row of taller ink is therefore taller by
/// exactly the ink's own excess, and no more — an H1 at 1.6 is *not* 1.6
/// pitches, because the air did not grow with the type.
///
/// The baseline moves with the ink because Pango's `scale` is a multiplier on
/// the font's size and a font's ascent is a fraction of its size, so the
/// distance from the top of the ink down to the baseline grows in the same
/// proportion the ink does. Taken from the ink's own height rather than from
/// the ladder's rung, so nothing here has to know which level the row is or
/// whether the rung was rounded on the way to a whole pixel.
///
/// A row of the body's own ink comes back as the pair it was given, exactly:
/// `ink / body` is one, and one times the baseline is the baseline. That is
/// what leaves every row but a heading's where it was.
fn row_band(pitch: f64, baseline: f64, body: f64, ink: f64) -> (f64, f64) {
    if body <= 0.0 || ink <= 0.0 {
        return (pitch, baseline);
    }
    (pitch + ink - body, baseline * (ink / body))
}

fn on_glass(row: (f64, f64), view: (f64, f64)) -> bool {
    let (row_top, row_height) = row;
    let (view_top, view_height) = view;
    row_top >= view_top && row_top + row_height <= view_top + view_height
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fold_draws_the_lines_it_left_and_the_lines_it_entered_and_nothing_when_it_stood_still() {
        assert_eq!(refolded(Some(4..5), 4..5), None, "the same lines are open");
        assert_eq!(refolded(Some(4..5), 7..9), Some(vec![4..5, 7..9]));
        assert_eq!(
            refolded(None, 7..9),
            Some(std::iter::once(7..9).collect::<Vec<_>>()),
            "the first fold has no lines to close"
        );
    }

    /// The empty page's words are the oracle's, read from the rule that sets
    /// them, so the two sides of `chrome/empty` say the same thing.
    #[test]
    fn the_placeholder_is_the_oracles_words() {
        let css = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../legacy/app/css/page.css"
        ))
        .expect("legacy/app/css/page.css");
        assert!(
            css.contains(&format!("content: \"{PLACEHOLDER}\";")),
            "page.css sets a different placeholder than {PLACEHOLDER:?}"
        );
    }

    /// A keystroke inside the lit sentence starts no fade: the tiers from
    /// before it, carried across it, paint the same bytes the same colour as
    /// the tiers read after it. Against the tiers as they stood, the byte the
    /// keystroke pushed past the range's end faded from dim to bright, which
    /// is what the owner saw (#224).
    #[test]
    fn a_keystroke_inside_the_lit_sentence_fades_nothing() {
        use quill_engine::settings::FocusScope;
        let focus = Focus::On(FocusScope::Sentence);
        let colours = Ground::default().colours;
        let mut doc = Document::untitled();
        doc.insert(0, "A first thought. A second one.\n");
        let was = focus::tiers_by_line(&doc, &focus::tiers(&doc, &(8..8), focus));
        let edit = doc.insert(8, "x");
        let after = focus::tiers_by_line(&doc, &focus::tiers(&doc, &(9..9), focus));
        let before = focus::rebased(&doc, &was, &edit.splice, &after);
        let line = doc.line_bytes(0);
        let spans = doc.spans_in(&line);
        let painted =
            |tiers: &[LineTiers]| annotate::paint_in(&spans, &line, tiers, focus, &colours);
        let runs = faded(&painted(&before), &painted(&after), buffer_offsets);
        assert!(
            runs.is_empty(),
            "the typed byte and the sentence around it are bright on both sides"
        );
        let stale = faded(&painted(&was), &painted(&after), buffer_offsets);
        assert!(
            matches!(stale.as_slice(), [FadeRun { at, .. }] if *at == (17..18)),
            "the defect, kept so this test is known to see it: judged against \
             the tiers as they stood, the sentence's last byte fades in; got {:?}",
            stale.iter().map(|run| run.at.clone()).collect::<Vec<_>>()
        );
    }

    /// A full stop typed into a sentence still moves the dim, and so still
    /// fades: the half the caret is not in goes from bright to dim; taken out
    /// again, that half comes back the other way. The one keystroke that is
    /// meant to fade, kept beside the one that is not.
    #[test]
    fn a_full_stop_typed_into_the_lit_sentence_fades_the_half_it_cut_off() {
        use quill_engine::settings::FocusScope;
        let focus = Focus::On(FocusScope::Sentence);
        let colours = Ground::default().colours;
        let mut doc = Document::untitled();
        doc.insert(0, "A first thought Another one.\n");
        let line = 0..doc.text().len();
        let ends = |runs: &[FadeRun]| {
            runs.iter()
                .map(|run| (run.at.clone(), run.from == run.to))
                .collect::<Vec<_>>()
        };

        let was = focus::tiers_by_line(&doc, &focus::tiers(&doc, &(15..15), focus));
        let edit = doc.insert(15, ".");
        let after = focus::tiers_by_line(&doc, &focus::tiers(&doc, &(16..16), focus));
        let before = focus::rebased(&doc, &was, &edit.splice, &after);
        let spans = doc.spans_in(&line);
        let painted =
            |tiers: &[LineTiers]| annotate::paint_in(&spans, &line, tiers, focus, &colours);
        let cut = faded(&painted(&before), &painted(&after), buffer_offsets);
        assert_eq!(
            ends(&cut),
            [(17..29, false)],
            "`Another one.` was lit and is dim now; `A first thought.` fades nowhere"
        );

        let was = after;
        let edit = doc.delete(15..16);
        let after = focus::tiers_by_line(&doc, &focus::tiers(&doc, &(15..15), focus));
        let before = focus::rebased(&doc, &was, &edit.splice, &after);
        let spans = doc.spans_in(&line);
        let painted =
            |tiers: &[LineTiers]| annotate::paint_in(&spans, &line, tiers, focus, &colours);
        let joined = faded(&painted(&before), &painted(&after), buffer_offsets);
        assert_eq!(
            ends(&joined),
            [(16..28, false)],
            "the full stop gone, `Another one.` is the caret's sentence again and lights"
        );
    }

    /// Buffer offsets for a test with no buffer: the Document's own bytes,
    /// which is what a one-line ASCII passage's buffer offsets are.
    fn buffer_offsets(at: &Range<usize>) -> Range<i32> {
        let offset = |byte: usize| i32::try_from(byte).expect("a line's bytes fit an i32");
        offset(at.start)..offset(at.end)
    }

    /// A reveal has landed only when the whole row is on the glass.
    ///
    /// The numbers are #221's: the bar drawn at buffer y 13156 at scale 2,
    /// which [`Editor::row_of`] reads as 6578 logical pixels from the page's
    /// top, with the viewport held at 2578–3478 — the row GTK's validation
    /// carried off the bottom. A row straddling either edge is not shown
    /// either, and the row of a Document shorter than its viewport is, from
    /// wherever the view is.
    #[test]
    fn a_row_is_on_the_glass_only_when_the_viewport_holds_all_of_it() {
        let view = (2578.0, 900.0);
        assert!(!on_glass((6578.0, 18.5), view), "#221's row read as shown");
        assert!(!on_glass((2570.0, 18.5), view), "a row over the top edge");
        assert!(
            !on_glass((3470.0, 18.5), view),
            "a row over the bottom edge"
        );
        assert!(on_glass((2578.0, 18.5), view), "a row at the very top");
        assert!(on_glass((3000.0, 18.5), view), "a row in the middle");
        assert!(
            on_glass((60.0, 37.0), (0.0, 900.0)),
            "a short Document's row"
        );
    }

    /// The layout has stood still only for the frames it read the same, and
    /// a frame that moved either reading starts the count over.
    ///
    /// The readings are the first frames of #221's Live run at
    /// `--caret 26000`: the height and the bar both moved twice as GTK
    /// validated, then held.
    #[test]
    fn a_landing_counts_the_frames_the_layout_has_held_still_for() {
        let landing = Landing::default().stood(5425.0, 6626.0);
        assert_eq!(
            landing.still, 0,
            "the first reading has nothing to hold against"
        );
        let landing = landing.stood(18232.0, 26312.0);
        assert_eq!(landing.still, 0, "the height and the bar both moved");
        let landing = landing.stood(29691.0, 26312.0);
        assert_eq!(landing.still, 0, "the height moved with the bar still");
        let landing = landing.stood(29691.0, 26312.0);
        assert_eq!(landing.still, 1, "one frame the same");
        let landing = landing.stood(29691.0, 26312.0);
        assert_eq!(
            landing.still, 2,
            "two frames the same, which is STILL_FRAMES"
        );
        assert_eq!(STILL_FRAMES, 2, "the test's count is the constant's");
        let landing = landing.stood(29691.0, 26400.0);
        assert_eq!(landing.still, 0, "a bar that moved starts the count over");
    }

    /// The ladder's ems are fractional, and the type is set at them.
    ///
    /// Both places the size is named — GTK's CSS and Pango's description —
    /// take the fraction, and the Editor rounded it to a whole pixel only
    /// while `--size` counted in pixels. At the default step that rounding was
    /// 21.33 to 21, which is a page a third of a pixel narrower per em than
    /// the one the oracles are frozen at.
    #[test]
    fn the_type_is_set_at_the_ladders_fractional_em() {
        let em = typography::em(quill_engine::settings::default_step());
        assert!((em - 21.33).abs() < 0.001, "the default step's em is {em}");
        assert!(
            stylesheet(&Ground::default().colours, Face::Duo, em).contains("font-size: 21.33px"),
            "the stylesheet named the type at a whole pixel"
        );
        let described = f64::from(body_font(Face::Duo, em).size());
        let wanted = em * f64::from(pango::SCALE);
        assert!(
            (described - wanted).abs() <= 1.0,
            "Pango was given {described} rather than {wanted}"
        );
    }

    /// GTK paints no selection ground at all, and still paints the glyphs in
    /// the page's own ink.
    ///
    /// The band is ours now, drawn in the same pass and off the same band as
    /// the bar (§ the snapshot), because that is the only way the fill, the
    /// two end bars and the caret can be guaranteed to register. What is left
    /// for the stylesheet is to get GTK out of the way without letting it take
    /// the ink with it: a `selection` rule that only clears the ground would
    /// leave `color` to the desktop theme's selected-text colour, which on a
    /// dark desktop is white on our paper.
    #[test]
    fn the_stylesheet_leaves_the_selection_ground_to_us_and_keeps_the_ink() {
        for scheme in [Scheme::Light, Scheme::Dark] {
            let css = sheet(scheme);
            let rule = css
                .split_once(&format!("textview.{FACE_CLASS} text selection"))
                .expect("no selection rule at all")
                .1;
            let rule = rule.split_once('}').expect("unclosed selection rule").0;
            assert!(
                rule.contains("background-color: transparent"),
                "{scheme:?}: GTK is still painting a selection ground under ours:\n{css}"
            );
            assert!(
                rule.contains(&format!(
                    "color: {}",
                    Ground::of(scheme).colours.colour(Role::Ink).to_hex()
                )),
                "{scheme:?}: selected glyphs are not in the page's ink:\n{css}"
            );
            assert_eq!(
                css.matches("selection").count(),
                1,
                "{scheme:?}: the idle swap is the fill's now, not the stylesheet's:\n{css}"
            );
        }
    }

    /// Each ground's own paper and ink, and nothing of the other's.
    ///
    /// The window and the text view are named separately because they are two
    /// surfaces: the paper is the one the writer sees past the measure, and a
    /// stylesheet that dressed only one of them would put a light frame around
    /// a dark page. Read off the text rather than off a screen, because that
    /// is a pure function of the scheme and needs no display to check
    /// ([#110](https://github.com/danielbaldwin47/Quill/issues/110)).
    ///
    /// The values themselves are the engine's to pin (`theme.rs` § `ORACLE`,
    /// off `docs/design.md`'s Paper · ink · dim row) and are read from the
    /// table here rather than written out again: after this ticket
    /// `grep -n '#[0-9a-f]\{6\}' quill/src` finds nothing, and a second copy
    /// of a number the critic reads is a second thing to keep true.
    #[test]
    fn each_ground_is_dressed_in_its_own_paper_and_its_own_ink() {
        for scheme in [Scheme::Light, Scheme::Dark] {
            let colours = Ground::of(scheme).colours;
            let paper = colours.colour(Role::Paper).to_hex();
            let ink = colours.colour(Role::Ink).to_hex();
            let other = Ground::of(scheme.other())
                .colours
                .colour(Role::Paper)
                .to_hex();
            let css = sheet(scheme);
            assert_eq!(
                css.matches(&format!("background-color: {paper}")).count(),
                2,
                "{scheme:?}: the window and the text are not both on {paper}:\n{css}"
            );
            assert!(
                css.contains(&format!("color: {ink}")),
                "{scheme:?}: the ink is not {ink}:\n{css}"
            );
            assert!(
                !css.contains(&other),
                "{scheme:?}: the other ground's paper {other} is in this one:\n{css}"
            );
        }
    }

    /// The stylesheet for `scheme` at the default type, which is what every
    /// judged state is shot at.
    fn sheet(scheme: Scheme) -> String {
        stylesheet(
            &Ground::of(scheme).colours,
            Face::Duo,
            typography::em(quill_engine::settings::default_step()),
        )
    }

    /// The band swaps when the window goes.
    ///
    /// Held here because no shot can hold it, which is why the stylesheet used
    /// to hold it: `caret/unfocused` is shot with `select: null`, so the one
    /// judged state that is not active is also the one state with no band to
    /// be idle. The swap moved from the stylesheet to the paint when the
    /// selection became ours, and the check moves with it.
    #[test]
    fn the_selection_goes_idle_with_the_window() {
        for scheme in [Scheme::Light, Scheme::Dark] {
            let colours = Ground::of(scheme).colours;
            let fill = selection_fill(&colours, true);
            let idle = selection_fill(&colours, false);
            assert_eq!(
                fill,
                paint(&colours, Role::Selection, 1.0),
                "{scheme:?}: an active window is the oracle's active fill"
            );
            assert_ne!(fill, idle, "{scheme:?}: the fill did not go idle");
            assert_eq!(idle, paint(&colours, Role::SelectionIdle, 1.0));
        }
        assert_ne!(
            selection_fill(&Ground::of(Scheme::Light).colours, true),
            selection_fill(&Ground::of(Scheme::Dark).colours, true),
            "the two grounds were designed one fill each"
        );
    }

    /// The page the numbers below were read off: `--deterministic --font duo
    /// --step 5`, the step every judged state is shot at, measured from the
    /// running app through `iter_location`. The body's ink is 28 logical
    /// pixels in a pitch of 37, so the leading splits 9 pixels of air over it
    /// (4 above, 5 below), and its baseline sits 22 pixels down. Live sets an
    /// H1 at [`tags::LADDER`]'s 1.6, which Pango lays out as 45 pixels of ink
    /// — 44.8 rounded — and an H2 at 1.4 as 40.
    const PAGE: (f64, f64, f64) = (37.0, 22.0, 28.0);

    /// Every row but a heading's is the pitch the type was cut at, to the bit:
    /// the caret Piece is won on those bands and a band that moved by a
    /// rounding would be a state lost to arithmetic nobody asked for.
    #[test]
    fn a_row_of_body_ink_is_the_pitch_and_the_baseline_it_was_cut_with() {
        let (pitch, baseline, body) = PAGE;
        assert_eq!(row_band(pitch, baseline, body, body), (pitch, baseline));
    }

    /// A heading's row is taller by the excess of its own ink and by nothing
    /// else: the air above and below it is a widget property and did not grow
    /// with the type. So an H1 is 54 rather than the 59.2 that 1.6 pitches
    /// would be, and the acceptance line's "1.6 × the pitch" is the ink's
    /// ladder rung and not the row's.
    #[test]
    fn a_heading_row_is_taller_by_its_inks_own_excess() {
        let (pitch, baseline, body) = PAGE;
        let air = pitch - body;
        for ink in [45.0, 40.0, 34.0] {
            let (row, _) = row_band(pitch, baseline, body, ink);
            assert_eq!(row, ink + air, "a row of {ink} px of ink keeps the air");
            assert!(
                row > pitch && row < ink / body * pitch,
                "{ink}: taller than the pitch, and short of the ink's own multiple of it"
            );
        }
    }

    /// The baseline moves down the row in the proportion the ink grew, so the
    /// band keeps its eleven-sixteenths above the letters rather than riding
    /// up toward the row above ([`caret::band_top`]).
    #[test]
    fn a_heading_rows_baseline_moves_with_its_ink() {
        let (pitch, baseline, body) = PAGE;
        let (_, deep) = row_band(pitch, baseline, body, 45.0);
        assert!((deep - baseline * (45.0 / body)).abs() < f64::EPSILON);
        assert!(
            deep > baseline && deep < 45.0,
            "the baseline is further down a taller row and still inside its ink"
        );
    }

    /// A page with no type set yet, and a row the view has not laid out, are
    /// both the body's pair: there is no rectangle to read and the band the
    /// caller draws is the one it would have drawn before Live existed.
    #[test]
    fn a_row_with_no_ink_falls_back_to_the_body() {
        let (pitch, baseline, body) = PAGE;
        assert_eq!(row_band(pitch, baseline, body, 0.0), (pitch, baseline));
        assert_eq!(row_band(pitch, baseline, 0.0, 45.0), (pitch, baseline));
    }

    // Live's furniture, headless. Every judgement about it is made on the
    // Document's own bytes ([`standing`]), and the widget's one step is the
    // crossing into the offsets GTK counts — which cannot be tested here,
    // because gtk4-rs asserts that GTK is initialised before it will make even
    // a `GtkTextBuffer`, and `gtk::init` needs a display. The passages below
    // are ASCII, so the two counts are the same and a byte offset is a cell.

    /// A Document holding `text`, with its Markup already derived.
    fn document(text: &str) -> Document {
        let mut document = Document::untitled();
        document.insert(0, text);
        document
    }

    /// Everything Live leaves standing on `text` with the writer at `at`, as
    /// the Editor keeps it: [`Editor::refurnish`] without the widget, over the
    /// whole passage rather than over one viewport of it.
    fn furniture(text: &str, at: &Range<usize>) -> Vec<Furnishing> {
        standing(&document(text), at, &(0..text.len()))
            .into_iter()
            .map(|(at, what)| Furnishing {
                at: buffer_offsets(&at),
                what,
            })
            .collect()
    }

    /// A page with one of each kind of furniture on it, and a caret parked in
    /// the heading so that nothing below it is the writer's own block.
    const PASSAGE: &str = "# Title\n\n- the rope\n\n1. Untie the skiff.\n\n- [ ] scrub the lens\n\n---\n\n\
         A [link](https://example.org/book) to it.\n\n```rust\nlet x = 1;\n```\n";

    #[test]
    fn a_task_boxs_brackets_are_found_inside_the_marker_that_carries_them() {
        let text = "- [ ] scrub the lens\n";
        assert_eq!(
            brackets(text, &(0..5)),
            Some(2..5),
            "the furniture covers the bullet and the box together; the press \
             wants the box"
        );
        let nested = "  - [x] wind the clock\n";
        assert_eq!(brackets(nested, &(0..7)), Some(4..7), "indent and all");
        assert_eq!(
            brackets("- the rope\n", &(0..2)),
            None,
            "a bullet with no box has no brackets to find"
        );
    }

    #[test]
    fn the_cells_a_task_box_press_rewrites_are_the_one_between_its_brackets() {
        let text = "Chores\n\n- [ ] scrub the lens\n";
        let box_at = furniture(text, &(0..0))
            .first()
            .and_then(Furnishing::box_cells)
            .expect("the page holds one task item");
        assert_eq!(
            box_at,
            10..13,
            "the box's own cells, found inside the marker cells the bullet and \
             the box share"
        );

        assert_eq!(
            flip(&box_at, true),
            (11..12, "x"),
            "one cell is rewritten and it is the one between the brackets, so \
             nothing on the line moves under the writer's finger"
        );
        assert_eq!(
            flip(&box_at, false),
            (11..12, " "),
            "and the press flips back"
        );
        assert_eq!(
            &text[11..12],
            " ",
            "the cells `flip` names are where the source wrote the state"
        );
    }

    #[test]
    fn a_links_words_open_the_address_the_source_wrote_or_the_one_it_defined() {
        /// The bytes an inline link writes its destination in: the
        /// [`Mark::Url`](quill_engine::annotate::Mark::Url) span, as the
        /// Markup Annotator hands it over.
        fn written(text: &str) -> Range<usize> {
            text.find('(').expect("a destination is opened") + 1
                ..text.rfind(')').expect("a destination is closed")
        }

        for (source, opens, why) in [
            (
                "A [link](https://example.org/book) to it.\n",
                Some("https://example.org/book"),
                "an inline destination is the address itself",
            ),
            (
                "A [link](https://example.org \"The Book\") to it.\n",
                Some("https://example.org"),
                "the title after the address is not part of it",
            ),
            (
                "A [link](<https://example.org/the book>) to it.\n",
                Some("https://example.org/the book"),
                "angle brackets hold an address with a space in it",
            ),
        ] {
            assert_eq!(
                address(source, &written(source), &markdown::references(source)).as_deref(),
                opens,
                "{why}"
            );
        }

        let referenced = "A [link][Book] here.\n\n[book]: https://example.org/book\n";
        let label = referenced.find("Book").expect("the label is written once");
        assert_eq!(
            address(
                referenced,
                &(label..label + 4),
                &markdown::references(referenced)
            )
            .as_deref(),
            Some("https://example.org/book"),
            "a reference link writes a label, not an address, and the \
             definition elsewhere in the file says where it goes"
        );
        let undefined = "A [link][Nowhere] here.\n";
        let label = undefined
            .find("Nowhere")
            .expect("the label is written once");
        assert_eq!(
            address(
                undefined,
                &(label..label + 7),
                &markdown::references(undefined)
            ),
            None,
            "a label nothing defines opens nothing"
        );
    }

    #[test]
    fn every_kind_of_marker_stands_in_its_own_cells_and_a_fence_stands_in_none() {
        let standing = furniture(PASSAGE, &(0..0));
        let kinds: Vec<Furnish> = standing.iter().map(|one| one.what.clone()).collect();
        assert_eq!(
            kinds,
            [
                Furnish::Bullet,
                Furnish::Number {
                    count: 1,
                    delimiter: '.'
                },
                Furnish::Checkbox {
                    box_at: 2..5,
                    checked: false
                },
                Furnish::Hairline,
                Furnish::Link {
                    destination: "https://example.org/book".to_owned()
                },
            ],
            "one furnishing per marker, in the order the page writes them, and \
             the fenced block's two fences draw nothing: the Well is already \
             under the code"
        );
    }

    #[test]
    fn a_press_finds_the_furniture_it_landed_on_by_the_offset_alone() {
        let standing = furniture(PASSAGE, &(0..0));
        let on = |at: i32| {
            standing
                .iter()
                .find(|one| one.at.contains(&at))
                .map(|one| one.what.clone())
        };
        let box_at = i32::try_from(PASSAGE.find("[ ]").expect("the page holds one task item"))
            .expect("the page fits an i32");

        assert!(
            matches!(on(box_at), Some(Furnish::Checkbox { .. })),
            "a press on the box itself"
        );
        assert!(
            matches!(on(box_at - 2), Some(Furnish::Checkbox { .. })),
            "and one on the bullet the box swallowed: the whole marker is the \
             target, because that is what the box stands in"
        );
        assert_eq!(
            on(box_at + 4),
            None,
            "a press on the item's first word is a caret move like any other"
        );

        let word = i32::try_from(PASSAGE.find("link").expect("the page holds one link"))
            .expect("the page fits an i32");
        assert!(
            matches!(on(word), Some(Furnish::Link { .. })),
            "a link's words are the target, and its destination is folded away"
        );
    }

    #[test]
    fn the_writers_own_block_is_furnished_with_nothing() {
        let item = PASSAGE
            .find("the rope")
            .expect("the page holds one bullet item");
        let standing = furniture(PASSAGE, &(item..item));
        assert!(
            !standing.iter().any(|one| one.what == Furnish::Bullet),
            "the caret is in the bullet item, so its marker is on the page and \
             nothing stands in its cells: {standing:?}"
        );
        assert!(
            standing.iter().any(|one| one.what == Furnish::Hairline),
            "and every other block is furnished as it was"
        );
    }
}
