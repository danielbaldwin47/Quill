//! The two designed grounds: the colour table, and the rule that picks a ground.
//!
//! Ten roles are the **Design oracle**'s, measured off iA Writer for Mac
//! (`ref/ia/mac-native/VERDICTS.md` 4.2.1–4.2.13 and § Marker ink) and carried
//! by [`docs/design.md`](../../../docs/design.md) rows Paper · ink · dim,
//! Accent, Active fill, Idle fill, Markers and Link: paper, ink, the dimmed
//! grey, the accent, the selection's two fills, the markers, the link's two
//! greys and the code ground. The other four — the rule, the shadow and the
//! chrome's two texts — are the Parity oracle's, role for role out of
//! `legacy/app/css/theme.css`, until they are measured in their turn
//! (4.2.14–4.2.15 are still unknown), which is the split `design.md` § The
//! palette is a file states.
//!
//! The markers are the ink. #198 shot iA Writer at every mark kind on both
//! grounds and found no resting marker grey at all: a heading's `#`, a quote's
//! `>`, a bullet, an ordered marker, a task box, a thematic break, a fence and
//! its info string, the inline-code marks, the emphasis runs and a bare URL all
//! rest at the body's own ink. The Parity oracle's 72 % quiet and 34 % hair
//! have no counterpart there, so [`Role::Mark`] survives as a role — a writer's
//! `palette` file still names it, and the app still says which run is a marker
//! — carrying the ink's value rather than a grey of its own.
//!
//! The two grounds are not inversions of each other but two designs: light ink
//! sits 16.4:1 over paper and dark ink only 10.8:1, because pure white on black
//! glares and blooms at night, and the dimmed grey is *relatively brighter* on
//! the dark ground because dark grounds crush low-contrast detail. (`theme.css`
//! argued that asymmetry from its own two grounds and gave the first pair as
//! 15.9:1 and 11.6:1; measured grounds move both numbers and leave the gap
//! exactly where it was, which is what it was arguing for.)
//!
//! Nothing here paints. The engine cannot see a display
//! ([ADR 0008](../../../docs/adr/0008-engine-crate-without-gtk.md)), so a role
//! resolves to a [`Colour`] and the app turns that into the widget stylesheet
//! and the tag table. A colour that is a step off the page — the two fills, the
//! code ground — is an alpha here rather than the grey it flattens to, because
//! an alpha survives a palette swap where a hex does not.

use crate::settings::{Choice, Theme, choice};

choice! {
    /// Which of the two grounds is on screen.
    ///
    /// The three-valued [`Theme`] is what the writer chose; this is what
    /// [`effective`] resolves that to once the flag and the desktop have had
    /// their say. It is written into `state.toml` as `last_scheme`, which is
    /// what an `auto` launch paints while the desktop is still being asked.
    Scheme {
        /// Dark ink on paper.
        #[default]
        Light => "light",
        /// Light ink on a dark ground.
        Dark => "dark",
    }
}

impl Scheme {
    /// The setting that pins this ground.
    ///
    /// The two grounds are two of [`Theme`]'s three values, and this is the
    /// only direction that is a plain mapping: `auto` is the third, it is a
    /// question rather than a ground, and its answer comes back through
    /// [`effective`]. Both of the things that pin a ground — the `--theme`
    /// flag and the toggle, which turns what is on screen into what is in the
    /// file — spell the conversion here rather than each in their own match.
    #[must_use]
    pub const fn setting(self) -> Theme {
        match self {
            Self::Light => Theme::Light,
            Self::Dark => Theme::Dark,
        }
    }

    /// The other ground, which is what the toggle asks for.
    #[must_use]
    pub const fn other(self) -> Self {
        match self {
            Self::Light => Self::Dark,
            Self::Dark => Self::Light,
        }
    }
}

/// A colour, on its way to GTK.
///
/// Channels and opacity are 0–1 so that [`Colour::over`] is ordinary
/// arithmetic, but both constructors take the numbers CSS writes,
/// so the table below can be read against `theme.css` line for line.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Colour {
    /// Red, 0–1.
    pub red: f64,
    /// Green, 0–1.
    pub green: f64,
    /// Blue, 0–1.
    pub blue: f64,
    /// Opacity, 0–1.
    pub alpha: f64,
}

impl Colour {
    /// The colour CSS writes as `rgba(red, green, blue, alpha)`: channels 0–255,
    /// opacity 0–1.
    #[must_use]
    pub const fn rgba(red: u8, green: u8, blue: u8, alpha: f64) -> Self {
        Self {
            red: red as f64 / 255.0,
            green: green as f64 / 255.0,
            blue: blue as f64 / 255.0,
            alpha,
        }
    }

    /// The colour CSS writes as `#rrggbb`, fully opaque.
    ///
    /// # Panics
    ///
    /// Panics when `hex` is not a `#` and six hex digits. Every call is a
    /// literal in the table below, evaluated where it is written, so a mistyped
    /// colour is a build failure rather than something a writer discovers. A
    /// colour that arrives out of a file — a Template's own, when ADR 0005
    /// gives Templates a palette — wants a reader that can fail instead, and
    /// there is none yet because nothing reads one.
    #[must_use]
    pub const fn from_hex(hex: &str) -> Self {
        let digits = hex.as_bytes();
        assert!(
            digits.len() == 7 && digits[0] == b'#',
            "a colour is written `#rrggbb`"
        );
        Self::rgba(
            byte(digits[1], digits[2]),
            byte(digits[3], digits[4]),
            byte(digits[5], digits[6]),
            1.0,
        )
    }

    /// `fg` laid over `bg` at `amount` coverage.
    ///
    /// `amount` stands in for `fg`'s own opacity and the ground's is what comes
    /// out, which is how a role is flattened onto the page: the code ground and
    /// the idle selection are both washes, and this is what lands them on the
    /// greys `docs/design.md` measured over each ground's own paper.
    ///
    /// `bg` is a ground and not a second translucent role — every caller lays a
    /// role on the page — so its opacity passes through rather than being
    /// composited with `fg`'s.
    #[must_use]
    pub const fn over(fg: Self, bg: Self, amount: f64) -> Self {
        Self {
            red: mix(fg.red, bg.red, amount),
            green: mix(fg.green, bg.green, amount),
            blue: mix(fg.blue, bg.blue, amount),
            alpha: bg.alpha,
        }
    }

    /// The colour as `#rrggbb`, which is [`Colour::from_hex`] read backwards
    /// and the form the oracles write an opaque role in.
    ///
    /// The opacity is dropped rather than spelled, because the two callers of
    /// this both carry one of their own: a `GtkTextTag`'s colour is keyed by
    /// `(colour, alpha)` and a flattened role has already spent its alpha on
    /// the ground it was flattened onto. A translucent role that has not been
    /// flattened wants [`Colour::to_css`], which keeps it.
    #[must_use]
    pub fn to_hex(self) -> String {
        format!(
            "#{:02x}{:02x}{:02x}",
            channel(self.red),
            channel(self.green),
            channel(self.blue)
        )
    }

    /// The colour as CSS: `rgba(249, 249, 249, 1)`.
    ///
    /// The one text form the widget stylesheet and the tag table are both built
    /// from, so a role reaches GTK spelled the same way whichever asks for it.
    #[must_use]
    pub fn to_css(self) -> String {
        format!(
            "rgba({}, {}, {}, {})",
            channel(self.red),
            channel(self.green),
            channel(self.blue),
            (self.alpha.clamp(0.0, 1.0) * 1000.0).round() / 1000.0
        )
    }
}

/// `a` at `amount` of the way, the rest `b`.
const fn mix(a: f64, b: f64, amount: f64) -> f64 {
    amount * a + (1.0 - amount) * b
}

/// The byte two hex digits spell.
const fn byte(high: u8, low: u8) -> u8 {
    digit(high) * 16 + digit(low)
}

/// What one hex digit is worth.
const fn digit(digit: u8) -> u8 {
    match digit {
        b'0'..=b'9' => digit - b'0',
        b'a'..=b'f' => digit - b'a' + 10,
        b'A'..=b'F' => digit - b'A' + 10,
        _ => panic!("a colour is written `#rrggbb`, in hex"),
    }
}

/// A channel as the 0–255 CSS writes.
fn channel(value: f64) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// Every colour the app asks a ground for.
///
/// A role rather than a colour at the call site, so a feature names what it is
/// painting and each ground answers for itself. Adding one here without adding
/// it to [`Colours::colour`] does not build.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Role {
    /// The page.
    Paper,
    /// Body text.
    Ink,
    /// Text Focus has dimmed.
    InkDim,
    /// Markdown's syntax markers. The Design oracle rests every one of them at
    /// the body's ink, so this is [`Role::Ink`]'s value on both built-in
    /// grounds; it stays a role of its own because a writer's `palette` file
    /// may still set the markers apart from the prose.
    Mark,
    /// The caret: the same blue on both grounds, because it is the one
    /// instrument the writer watches.
    Accent,
    /// A link's plumbing: its `[`, `]`, `(` and `)` and the destination between
    /// them. The link's *words* are the writer's and take [`Role::Ink`]; this
    /// is the grey the Design oracle quiets the machinery around them to.
    Link,
    /// The hairline under a link's words.
    LinkRule,
    /// The selection.
    Selection,
    /// The selection while the window is not focused.
    SelectionIdle,
    /// The bars' text.
    ChromeFg,
    /// The bars' text where it carries weight.
    ChromeFgStrong,
    /// The ground behind code.
    CodeBg,
    /// Hairlines and block borders.
    Rule,
    /// What a raised surface casts.
    Shadow,
}

impl Role {
    /// Every role, in the order `theme.css` declares them.
    ///
    /// A role added to [`Role`] belongs here too. [`Colours`] does not build
    /// without a field for it and [`Colours::colour`] does not build without an
    /// arm, so the table stays total either way; this list is the one place
    /// kept by hand, and what a role missing from it costs is the tests below
    /// quietly stopping short of it.
    pub const ALL: [Self; 14] = [
        Self::Paper,
        Self::Ink,
        Self::InkDim,
        Self::Mark,
        Self::Accent,
        Self::Link,
        Self::LinkRule,
        Self::Selection,
        Self::SelectionIdle,
        Self::ChromeFg,
        Self::ChromeFgStrong,
        Self::CodeBg,
        Self::Rule,
        Self::Shadow,
    ];
}

/// One ground's colour for every [`Role`].
///
/// Read through [`Colours::colour`], which is total: the table cannot answer a
/// role with a fallback, because there is no fallback to answer with.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Colours {
    paper: Colour,
    ink: Colour,
    ink_dim: Colour,
    mark: Colour,
    accent: Colour,
    link: Colour,
    link_rule: Colour,
    selection: Colour,
    selection_idle: Colour,
    chrome_fg: Colour,
    chrome_fg_strong: Colour,
    code_bg: Colour,
    rule: Colour,
    shadow: Colour,
}

impl Colours {
    /// Paper: the Design oracle's ten, then `theme.css`'s `:root` for the rest.
    ///
    /// The idle fill and the code ground are alphas rather than the greys they
    /// flatten to, because an alpha survives a palette swap where a hex does
    /// not: over this paper they land on the oracle's own `#dcdcdc` and
    /// `#eeeeee`, and over a writer's paper they land wherever that paper puts
    /// them (`design.md` rows Idle fill and Code ground).
    const LIGHT: Self = Self {
        paper: Colour::from_hex("#f7f7f7"),
        ink: Colour::from_hex("#191919"),
        ink_dim: Colour::from_hex("#c6c4c2"),
        mark: Colour::from_hex("#191919"),
        accent: Colour::from_hex("#00bfff"),
        link: Colour::from_hex("#b5b3b0"),
        link_rule: Colour::from_hex("#d5d3d1"),
        selection: Colour::from_hex("#ccedf8"),
        selection_idle: Colour::rgba(25, 25, 25, 0.122),
        chrome_fg: Colour::from_hex("#8c8c8c"),
        chrome_fg_strong: Colour::from_hex("#4a4a4a"),
        code_bg: Colour::rgba(0, 0, 0, 0.036),
        rule: Colour::rgba(0, 0, 0, 0.10),
        shadow: Colour::rgba(0, 0, 0, 0.18),
    };

    /// The dark ground: the same ten measured, then
    /// `theme.css`'s `:root[data-theme="dark"]`.
    ///
    /// Three of the ten were already the oracle's own numbers — the dark paper,
    /// ink and dimmed grey are what `theme.css` set them to and what
    /// `VERDICTS.md` measured — and the marker is now the ink, so what moves
    /// here is the accent, the two fills, the link's two greys and the ground
    /// under code.
    const DARK: Self = Self {
        paper: Colour::from_hex("#1a1a1a"),
        ink: Colour::from_hex("#cccccc"),
        ink_dim: Colour::from_hex("#707070"),
        mark: Colour::from_hex("#cccccc"),
        accent: Colour::from_hex("#00bfff"),
        link: Colour::from_hex("#7a7a78"),
        link_rule: Colour::from_hex("#545452"),
        selection: Colour::from_hex("#113d52"),
        selection_idle: Colour::rgba(204, 204, 204, 0.247),
        chrome_fg: Colour::from_hex("#7e7e7e"),
        chrome_fg_strong: Colour::from_hex("#bdbdbd"),
        code_bg: Colour::rgba(255, 255, 255, 0.048),
        rule: Colour::rgba(255, 255, 255, 0.10),
        shadow: Colour::rgba(0, 0, 0, 0.55),
    };

    /// The colours of one ground.
    #[must_use]
    pub const fn of(scheme: Scheme) -> Self {
        match scheme {
            Scheme::Light => Self::LIGHT,
            Scheme::Dark => Self::DARK,
        }
    }

    /// What this ground paints `role` in.
    ///
    /// Total by construction: a role added to [`Role`] and not to this match is
    /// a build failure, so no feature can end up painting with a colour that
    /// was never designed for the ground it is on.
    #[must_use]
    pub const fn colour(&self, role: Role) -> Colour {
        match role {
            Role::Paper => self.paper,
            Role::Ink => self.ink,
            Role::InkDim => self.ink_dim,
            Role::Mark => self.mark,
            Role::Accent => self.accent,
            Role::Link => self.link,
            Role::LinkRule => self.link_rule,
            Role::Selection => self.selection,
            Role::SelectionIdle => self.selection_idle,
            Role::ChromeFg => self.chrome_fg,
            Role::ChromeFgStrong => self.chrome_fg_strong,
            Role::CodeBg => self.code_bg,
            Role::Rule => self.rule,
            Role::Shadow => self.shadow,
        }
    }
}

/// The ground to paint.
///
/// The flag wins, then the setting; `Auto` takes the desktop's answer, or, when
/// there is none yet, the ground this Quill left last. `--theme` pins a ground
/// so the Gate shoots both states on any desktop, and `last` — `last_scheme`
/// out of `state.toml` — is what keeps the first frame of an `auto` launch from
/// flashing the wrong ground while the portal is still being asked.
#[must_use]
pub const fn effective(
    flag: Option<Scheme>,
    setting: Theme,
    portal: Option<Scheme>,
    last: Scheme,
) -> Scheme {
    match (flag, setting) {
        (Some(flag), _) => flag,
        (None, Theme::Light) => Scheme::Light,
        (None, Theme::Dark) => Scheme::Dark,
        (None, Theme::Auto) => match portal {
            Some(desktop) => desktop,
            None => last,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every role on both grounds, as the oracle that owns it gives it: the
    /// opaque ones as the hex it writes, the translucent ones as the CSS the
    /// app will emit, which is that `rgba()` with its opacity spelled in full.
    ///
    /// Ten of the rows are `docs/design.md`'s, off the Design oracle, and the
    /// rest are `legacy/app/css/theme.css`'s; which is which is the module's
    /// header. Every row is written out here rather than derived, because a
    /// table that computes what it asserts asserts nothing — including the two
    /// marker rows, which are the ink's value said a second time rather than a
    /// reference to it, so that a hand that unpicks the two grounds fails here.
    const ORACLE: [(Scheme, Role, &str); 28] = [
        (Scheme::Light, Role::Paper, "#f7f7f7"),
        (Scheme::Light, Role::Ink, "#191919"),
        (Scheme::Light, Role::InkDim, "#c6c4c2"),
        (Scheme::Light, Role::Mark, "#191919"),
        (Scheme::Light, Role::Accent, "#00bfff"),
        (Scheme::Light, Role::Link, "#b5b3b0"),
        (Scheme::Light, Role::LinkRule, "#d5d3d1"),
        (Scheme::Light, Role::Selection, "#ccedf8"),
        (
            Scheme::Light,
            Role::SelectionIdle,
            "rgba(25, 25, 25, 0.122)",
        ),
        (Scheme::Light, Role::ChromeFg, "#8c8c8c"),
        (Scheme::Light, Role::ChromeFgStrong, "#4a4a4a"),
        (Scheme::Light, Role::CodeBg, "rgba(0, 0, 0, 0.036)"),
        (Scheme::Light, Role::Rule, "rgba(0, 0, 0, 0.1)"),
        (Scheme::Light, Role::Shadow, "rgba(0, 0, 0, 0.18)"),
        (Scheme::Dark, Role::Paper, "#1a1a1a"),
        (Scheme::Dark, Role::Ink, "#cccccc"),
        (Scheme::Dark, Role::InkDim, "#707070"),
        (Scheme::Dark, Role::Mark, "#cccccc"),
        (Scheme::Dark, Role::Accent, "#00bfff"),
        (Scheme::Dark, Role::Link, "#7a7a78"),
        (Scheme::Dark, Role::LinkRule, "#545452"),
        (Scheme::Dark, Role::Selection, "#113d52"),
        (
            Scheme::Dark,
            Role::SelectionIdle,
            "rgba(204, 204, 204, 0.247)",
        ),
        (Scheme::Dark, Role::ChromeFg, "#7e7e7e"),
        (Scheme::Dark, Role::ChromeFgStrong, "#bdbdbd"),
        (Scheme::Dark, Role::CodeBg, "rgba(255, 255, 255, 0.048)"),
        (Scheme::Dark, Role::Rule, "rgba(255, 255, 255, 0.1)"),
        (Scheme::Dark, Role::Shadow, "rgba(0, 0, 0, 0.55)"),
    ];

    /// WCAG 2.1 relative luminance.
    fn luminance(colour: Colour) -> f64 {
        fn linear(channel: f64) -> f64 {
            if channel <= 0.03928 {
                channel / 12.92
            } else {
                ((channel + 0.055) / 1.055).powf(2.4)
            }
        }

        0.2126 * linear(colour.red) + 0.7152 * linear(colour.green) + 0.0722 * linear(colour.blue)
    }

    /// WCAG 2.1 contrast ratio, which is the same either way round.
    fn contrast(one: Colour, other: Colour) -> f64 {
        let (one, other) = (luminance(one), luminance(other));
        (one.max(other) + 0.05) / (one.min(other) + 0.05)
    }

    #[test]
    fn every_role_is_the_colour_the_oracle_gives_it() {
        for (scheme, role, written) in ORACLE {
            let colour = Colours::of(scheme).colour(role);
            let ours = if written.starts_with('#') {
                colour.to_hex()
            } else {
                colour.to_css()
            };
            assert_eq!(ours, written, "{scheme:?} {role:?}");
        }
    }

    #[test]
    fn the_table_answers_for_every_role_on_both_grounds() {
        for scheme in [Scheme::Light, Scheme::Dark] {
            for role in Role::ALL {
                let named = ORACLE
                    .iter()
                    .filter(|(named, wanted, _)| *named == scheme && *wanted == role)
                    .count();
                assert_eq!(named, 1, "{scheme:?} {role:?} is named once above");
            }
        }
    }

    /// The design rule of the two grounds, and the one place the asymmetry
    /// `theme.css` argues for is pinned rather than described.
    ///
    /// Ink over paper is 16.4:1 on the light ground but only 10.8:1 on the
    /// dark one, which is deliberate: pure white on black glares and blooms at
    /// night, so the dark ink is `#cccccc` rather than `#ffffff`. The floors
    /// below are the rule each ground actually holds to; the exact ratios are
    /// asserted beside them so that a colour edited by hand fails here.
    ///
    /// The markers ride with the ink and are asserted to, because the Design
    /// oracle rests every mark kind at the body's own ink (#198): there is no
    /// marker floor left to clear. What is quieter than the prose is the link's
    /// plumbing, and the rule it holds to is that it stays *above* the tier
    /// Focus dims to — a link's destination is quiet, not out of focus — which
    /// it does on both grounds by a margin narrower on light than on dark.
    #[test]
    fn ink_and_marker_stay_legible_on_both_grounds() {
        for (scheme, ink_floor, ink_ratio, link_ratio, dim_ratio) in [
            (Scheme::Light, 12.0, 16.41, 1.95, 1.62),
            (Scheme::Dark, 10.5, 10.84, 4.05, 3.51),
        ] {
            let colours = Colours::of(scheme);
            let paper = colours.colour(Role::Paper);
            let ink = contrast(colours.colour(Role::Ink), paper);
            let link = contrast(colours.colour(Role::Link), paper);
            let dim = contrast(colours.colour(Role::InkDim), paper);
            assert!(ink >= ink_floor, "{scheme:?} ink over paper is {ink:.2}:1");
            assert!((ink - ink_ratio).abs() < 0.01, "{scheme:?} ink {ink:.2}:1");
            assert_eq!(
                colours.colour(Role::Mark),
                colours.colour(Role::Ink),
                "{scheme:?} rests its markers at the ink"
            );
            assert!(link > dim, "{scheme:?} link {link:.2}:1, dim {dim:.2}:1");
            assert!(
                (link - link_ratio).abs() < 0.01,
                "{scheme:?} link {link:.2}"
            );
            assert!((dim - dim_ratio).abs() < 0.01, "{scheme:?} dim {dim:.2}");
        }
    }

    #[test]
    fn the_flag_wins_then_the_setting_then_the_desktop_then_what_we_left() {
        for last in [Scheme::Light, Scheme::Dark] {
            for setting in [Theme::Auto, Theme::Light, Theme::Dark] {
                for portal in [None, Some(Scheme::Light), Some(Scheme::Dark)] {
                    let wanted = match setting {
                        Theme::Light => Scheme::Light,
                        Theme::Dark => Scheme::Dark,
                        Theme::Auto => portal.unwrap_or(last),
                    };
                    let ours = effective(None, setting, portal, last);
                    assert_eq!(ours, wanted, "{setting:?} {portal:?} last {last:?}");
                    for flag in [Scheme::Light, Scheme::Dark] {
                        let ours = effective(Some(flag), setting, portal, last);
                        assert_eq!(ours, flag, "--theme {flag:?} over {setting:?} {portal:?}");
                    }
                }
            }
        }
    }

    /// What the translucent roles flatten to, which is the whole reason they
    /// are alphas: `docs/design.md` rows Idle fill and Code ground measured
    /// `#dcdcdc` / `#464646` and `#eeeeee` / `#252525` off the Design oracle,
    /// and the alphas below are the ones that land each role on exactly those
    /// greys over each ground's own paper. A hex would have said the same thing
    /// once and then lied to the first writer who set `palette` to their own
    /// paper.
    ///
    /// The code ground's light value is `theme.css`'s own `#eeeeee` and its
    /// dark one is not: the Parity oracle's 6 % white lands on `#2a2a2a`, and
    /// the oracle's ground is a shade below it. Both alphas here are the ones
    /// measured, so this is also the check that [`Colour::over`] composites the
    /// way a browser does.
    #[test]
    fn a_translucent_role_flattens_onto_the_page_the_way_the_oracle_says() {
        for (scheme, role, flattened) in [
            (Scheme::Light, Role::SelectionIdle, "#dcdcdc"),
            (Scheme::Dark, Role::SelectionIdle, "#464646"),
            (Scheme::Light, Role::CodeBg, "#eeeeee"),
            (Scheme::Dark, Role::CodeBg, "#252525"),
        ] {
            let colours = Colours::of(scheme);
            let colour = colours.colour(role);
            let over = Colour::over(colour, colours.colour(Role::Paper), colour.alpha);
            assert_eq!(
                over.to_hex(),
                flattened,
                "{scheme:?} {role:?} over the page"
            );
        }
    }

    #[test]
    fn a_colour_reads_and_writes_the_way_css_does() {
        assert_eq!(Colour::from_hex("#00b5ff"), Colour::rgba(0, 181, 255, 1.0));
        assert_eq!(Colour::from_hex("#FFFFFF"), Colour::from_hex("#ffffff"));
        assert_eq!(
            Colour::from_hex("#f9f9f9").to_css(),
            "rgba(249, 249, 249, 1)"
        );
        assert_eq!(
            Colour::rgba(0, 0, 0, 0.045).to_css(),
            "rgba(0, 0, 0, 0.045)"
        );
    }

    #[test]
    fn a_scheme_is_written_the_way_the_setting_that_names_it_is() {
        assert_eq!(Scheme::Light.as_str(), "light");
        assert_eq!(Scheme::Dark.as_str(), "dark");
        assert_eq!(Scheme::parse("dark"), Some(Scheme::Dark));
        assert_eq!(
            Scheme::parse("auto"),
            None,
            "auto is a setting, not a ground"
        );
        assert_eq!(Scheme::VALUES, ["light", "dark"]);
    }
}
