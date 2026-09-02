//! The ground a launch paints on, with the table it is painted from — the one
//! value every painter reads a colour off.
//!
//! A [`Scheme`] names one of the two grounds; a `Ground` is that ground with
//! its [`Colours`]. The two travel together because the app needs both — the
//! table for every colour it paints, the scheme for what is not in the table:
//! GTK's own light-or-dark variant, and the menus' literals — and a scheme
//! carried alongside a table it did not come from is a pair that can
//! disagree.
//!
//! The table is chosen once, where the ground is chosen
//! ([`crate::session::Session::ground`]), and handed down: every painter
//! takes a `Ground` or reads the one its widget holds, and none of them
//! builds a table of its own. That is what lets a writer's palette file lie
//! over the built-ins (`docs/design.md` § The palette is a file): the one
//! site answers [`Ground::overlaid`] rather than [`Ground::of`], and no
//! painter learns what a palette is.
//!
//! Not the engine's [`quill_engine::annotate::Ground`], which is what one run
//! is drawn on — the page or the code well — and is spelled with its module
//! where the two meet.

use quill_engine::theme::{Colours, Palette, Scheme};

/// One of the two grounds, and the colours it is painted in.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ground {
    /// Which ground this is.
    pub scheme: Scheme,
    /// Every role's colour on it.
    pub colours: Colours,
}

impl Ground {
    /// `scheme`'s ground in the built-in table — the Design oracle's values
    /// and the Parity oracle's (`docs/design.md` § Paper · ink · dim).
    ///
    /// The one place in the app that reads [`Colours::of`]: the built-ins
    /// enter here and nowhere else, so a palette laid over them is a change to
    /// what [`crate::session::Session::ground`] answers, not to a painter.
    #[must_use]
    pub const fn of(scheme: Scheme) -> Self {
        Self {
            scheme,
            colours: Colours::of(scheme),
        }
    }

    /// `scheme`'s ground with `palette`'s table for it laid over the
    /// built-ins: what a writer with a palette file paints on.
    ///
    /// Every role the file names is the file's and every role it leaves out
    /// is [`Ground::of`]'s ([`Colours::overlaid`]), so the answer is as total
    /// as the built-ins and no painter can tell the two apart. The empty
    /// palette answers exactly what [`Ground::of`] does.
    #[must_use]
    pub fn overlaid(scheme: Scheme, palette: &Palette) -> Self {
        Self {
            scheme,
            colours: Colours::overlaid(scheme, palette),
        }
    }
}

impl Default for Ground {
    /// The light ground, which is what an Editor holds before it is told
    /// which ground it opens on.
    fn default() -> Self {
        Self::of(Scheme::default())
    }
}
