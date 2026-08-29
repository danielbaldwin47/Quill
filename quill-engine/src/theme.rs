//! The two designed grounds: the colour table, and the rule that picks a ground.
//!
//! Every value is the Parity oracle's, role for role out of
//! `legacy/app/css/theme.css`. The two grounds are not inversions of each other
//! but two designs: light ink sits 16.2:1 over paper and dark ink only 10.8:1,
//! because pure white on black glares and blooms at night, and the dimmed grey
//! is *relatively brighter* on the dark ground because dark grounds crush
//! low-contrast detail.
//!
//! Nothing here paints. The engine cannot see a display
//! ([ADR 0008](../../../docs/adr/0008-engine-crate-without-gtk.md)), so a role
//! resolves to a [`Colour`] and the app turns that into the widget stylesheet
//! and the tag table. Colours that are a step off another colour — Focus's near
//! tier, the resting marker grey — are computed here from [`Colour::lift`] and
//! [`Colour::over`] rather than written down twice.

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

/// A colour, on its way to GTK.
///
/// Channels and opacity are 0–1 so that [`Colour::over`] and [`Colour::lift`]
/// are ordinary arithmetic, but both constructors take the numbers CSS writes,
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
    /// literal in the table below, so a mistyped colour is a build failure
    /// rather than something a writer discovers.
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
    /// out, which is how a role is flattened onto the page: `theme.css` gives
    /// `--code-bg` as `#eeeeee` and `--selection` as `#c2eafa` over light paper
    /// this way, and the resting marker grey is the marker at 72 % of itself.
    #[must_use]
    pub const fn over(fg: Self, bg: Self, amount: f64) -> Self {
        Self {
            red: mix(fg.red, bg.red, amount),
            green: mix(fg.green, bg.green, amount),
            blue: mix(fg.blue, bg.blue, amount),
            alpha: bg.alpha,
        }
    }

    /// `from` moved `amount` of the way toward `toward`.
    ///
    /// The oracle's `color-mix(in srgb, …)` (`legacy/app/css/focus.css:28`) in
    /// one function. Focus's near tier is the ground's dimmed grey lifted
    /// [`near_lift`] of the way back toward its ink, so the tier is computed
    /// from the two greys rather than being a third grey to keep in step.
    #[must_use]
    pub const fn lift(from: Self, toward: Self, amount: f64) -> Self {
        Self {
            red: mix(toward.red, from.red, amount),
            green: mix(toward.green, from.green, amount),
            blue: mix(toward.blue, from.blue, amount),
            alpha: mix(toward.alpha, from.alpha, amount),
        }
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
    /// Markdown's syntax markers.
    Mark,
    /// The caret: the same blue on both grounds, because it is the one
    /// instrument the writer watches.
    Accent,
    /// Link text.
    Link,
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
    pub const ALL: [Self; 13] = [
        Self::Paper,
        Self::Ink,
        Self::InkDim,
        Self::Mark,
        Self::Accent,
        Self::Link,
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
    selection: Colour,
    selection_idle: Colour,
    chrome_fg: Colour,
    chrome_fg_strong: Colour,
    code_bg: Colour,
    rule: Colour,
    shadow: Colour,
}

impl Colours {
    /// Paper: `theme.css`'s `:root`.
    const LIGHT: Self = Self {
        paper: Colour::from_hex("#f9f9f9"),
        ink: Colour::from_hex("#1c1c1c"),
        ink_dim: Colour::from_hex("#cccccc"),
        mark: Colour::from_hex("#7a7a7a"),
        accent: Colour::from_hex("#00b5ff"),
        link: Colour::from_hex("#0b7cba"),
        selection: Colour::rgba(0, 181, 255, 0.22),
        selection_idle: Colour::rgba(28, 28, 28, 0.10),
        chrome_fg: Colour::from_hex("#8c8c8c"),
        chrome_fg_strong: Colour::from_hex("#4a4a4a"),
        code_bg: Colour::rgba(0, 0, 0, 0.045),
        rule: Colour::rgba(0, 0, 0, 0.10),
        shadow: Colour::rgba(0, 0, 0, 0.18),
    };

    /// The dark ground: `theme.css`'s `:root[data-theme="dark"]`.
    const DARK: Self = Self {
        paper: Colour::from_hex("#1a1a1a"),
        ink: Colour::from_hex("#cccccc"),
        ink_dim: Colour::from_hex("#707070"),
        mark: Colour::from_hex("#808080"),
        accent: Colour::from_hex("#00b5ff"),
        link: Colour::from_hex("#4cc5ff"),
        selection: Colour::rgba(0, 181, 255, 0.23),
        selection_idle: Colour::rgba(204, 204, 204, 0.12),
        chrome_fg: Colour::from_hex("#7e7e7e"),
        chrome_fg_strong: Colour::from_hex("#bdbdbd"),
        code_bg: Colour::rgba(255, 255, 255, 0.06),
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

/// How far Focus's near tier climbs from the ground's dimmed grey back toward
/// its ink.
///
/// `legacy/app/css/focus.css:18` and `:21`. The dark ground needs the longer
/// climb because a dark ground crushes low-contrast detail, so the same step
/// reads as less of one.
#[must_use]
pub const fn near_lift(scheme: Scheme) -> f64 {
    match scheme {
        Scheme::Light => 0.15,
        Scheme::Dark => 0.26,
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

    /// Every role on both grounds, as `legacy/app/css/theme.css` gives it: the
    /// opaque ones as the hex it writes, the translucent ones as the CSS the
    /// app will emit, which is that `rgba()` with its opacity spelled in full.
    const ORACLE: [(Scheme, Role, &str); 26] = [
        (Scheme::Light, Role::Paper, "#f9f9f9"),
        (Scheme::Light, Role::Ink, "#1c1c1c"),
        (Scheme::Light, Role::InkDim, "#cccccc"),
        (Scheme::Light, Role::Mark, "#7a7a7a"),
        (Scheme::Light, Role::Accent, "#00b5ff"),
        (Scheme::Light, Role::Link, "#0b7cba"),
        (Scheme::Light, Role::Selection, "rgba(0, 181, 255, 0.22)"),
        (Scheme::Light, Role::SelectionIdle, "rgba(28, 28, 28, 0.1)"),
        (Scheme::Light, Role::ChromeFg, "#8c8c8c"),
        (Scheme::Light, Role::ChromeFgStrong, "#4a4a4a"),
        (Scheme::Light, Role::CodeBg, "rgba(0, 0, 0, 0.045)"),
        (Scheme::Light, Role::Rule, "rgba(0, 0, 0, 0.1)"),
        (Scheme::Light, Role::Shadow, "rgba(0, 0, 0, 0.18)"),
        (Scheme::Dark, Role::Paper, "#1a1a1a"),
        (Scheme::Dark, Role::Ink, "#cccccc"),
        (Scheme::Dark, Role::InkDim, "#707070"),
        (Scheme::Dark, Role::Mark, "#808080"),
        (Scheme::Dark, Role::Accent, "#00b5ff"),
        (Scheme::Dark, Role::Link, "#4cc5ff"),
        (Scheme::Dark, Role::Selection, "rgba(0, 181, 255, 0.23)"),
        (
            Scheme::Dark,
            Role::SelectionIdle,
            "rgba(204, 204, 204, 0.12)",
        ),
        (Scheme::Dark, Role::ChromeFg, "#7e7e7e"),
        (Scheme::Dark, Role::ChromeFgStrong, "#bdbdbd"),
        (Scheme::Dark, Role::CodeBg, "rgba(255, 255, 255, 0.06)"),
        (Scheme::Dark, Role::Rule, "rgba(255, 255, 255, 0.1)"),
        (Scheme::Dark, Role::Shadow, "rgba(0, 0, 0, 0.55)"),
    ];

    /// A colour as `#rrggbb`, which is how the oracle writes the opaque ones.
    fn hex(colour: Colour) -> String {
        format!(
            "#{:02x}{:02x}{:02x}",
            channel(colour.red),
            channel(colour.green),
            channel(colour.blue)
        )
    }

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
                hex(colour)
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
    /// Ink over paper is 16.2:1 on the light ground but only 10.8:1 on the
    /// dark one, which is deliberate: pure white on black glares and blooms at
    /// night, so the dark ink is `#cccccc` rather than `#ffffff`. The floors
    /// below are the rule each ground actually holds to; the exact ratios are
    /// asserted beside them so that a colour edited by hand fails here.
    #[test]
    fn ink_and_marker_stay_legible_on_both_grounds() {
        for (scheme, ink_floor, ink_ratio, mark_ratio) in [
            (Scheme::Light, 12.0, 16.19, 4.08),
            (Scheme::Dark, 10.5, 10.84, 4.41),
        ] {
            let colours = Colours::of(scheme);
            let paper = colours.colour(Role::Paper);
            let ink = contrast(colours.colour(Role::Ink), paper);
            let mark = contrast(colours.colour(Role::Mark), paper);
            assert!(ink >= ink_floor, "{scheme:?} ink over paper is {ink:.2}:1");
            assert!(mark >= 4.0, "{scheme:?} mark over paper is {mark:.2}:1");
            assert!((ink - ink_ratio).abs() < 0.01, "{scheme:?} ink {ink:.2}:1");
            assert!(
                (mark - mark_ratio).abs() < 0.01,
                "{scheme:?} mark {mark:.2}:1"
            );
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

    #[test]
    fn the_near_tier_and_the_resting_marker_are_computed_rather_than_written() {
        for (scheme, ink_near) in [(Scheme::Light, "#b2b2b2"), (Scheme::Dark, "#888888")] {
            let colours = Colours::of(scheme);
            let near = Colour::lift(
                colours.colour(Role::InkDim),
                colours.colour(Role::Ink),
                near_lift(scheme),
            );
            assert_eq!(hex(near), ink_near, "{scheme:?} ink-near");
        }

        let light = Colours::of(Scheme::Light);
        let resting = Colour::over(light.colour(Role::Mark), light.colour(Role::Paper), 0.72);
        assert_eq!(hex(resting), "#9e9e9e", "the resting marker grey");
    }

    /// The flattenings `theme.css` states in its own comments, which is the
    /// check that [`Colour::over`] composites the way a browser does.
    #[test]
    fn a_translucent_role_flattens_onto_the_page_the_way_the_oracle_says() {
        for (scheme, role, flattened) in [
            (Scheme::Light, Role::CodeBg, "#eeeeee"),
            (Scheme::Light, Role::Selection, "#c2eafa"),
            (Scheme::Dark, Role::Selection, "#143e4f"),
        ] {
            let colours = Colours::of(scheme);
            let colour = colours.colour(role);
            let over = Colour::over(colour, colours.colour(Role::Paper), colour.alpha);
            assert_eq!(hex(over), flattened, "{scheme:?} {role:?} over the page");
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
