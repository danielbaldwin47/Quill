//! The Settings window's stylesheet: Quill's own CSS for every control the window holds, per scheme.
//!
//! Copied from the stub on `prototype/settings-stub` (`f7118af`), whose README
//! measured each value against the canvas on the real toolkit at scale 2. The
//! colours are per-scheme literals, as the menus' are ([`crate::chrome::menu_ink`]),
//! not theme Roles, so a palette file does not reach them. [`stylesheet`] is
//! appended to the chrome's sheet, which is installed display-wide and reloads
//! with the ground, so every open Settings window follows a scheme change.
//!
//! Nothing here reads the icon theme: the dropdown's chevron is Quill's own file
//! under [`quill_engine::data::icons`], and the checks' tick is GTK's own
//! built-in resource, which ships with the toolkit.

use gtk::glib;
use quill_engine::theme::Scheme;

use crate::chrome::CHROME_FONT;

/// A scheme's colours, as the placeholder each stands in for in [`SHEET`] and
/// the literal it is replaced by.
struct Skin {
    pairs: [(&'static str, &'static str); 22],
}

const LIGHT: Skin = Skin {
    pairs: [
        ("@ink@", "#191919"),
        ("@dim@", "#8c8c8c"),
        ("@bg@", "#f7f7f7"),
        ("@rule@", "rgba(0,0,0,0.10)"),
        ("@sw_off@", "#d2d2d2"),
        ("@sw_on@", "#0a94d6"),
        ("@knob@", "#ffffff"),
        ("@knob_bd@", "rgba(0,0,0,0.08)"),
        ("@btn_bg@", "#fcfcfc"),
        ("@btn_bd@", "rgba(0,0,0,0.16)"),
        ("@field@", "#fcfcfc"),
        ("@accent@", "#0a94d6"),
        ("@check_bd@", "rgba(0,0,0,0.28)"),
        ("@list_bg@", "#fcfcfc"),
        ("@danger@", "#c4483c"),
        ("@track@", "#dcdcdc"),
        ("@side@", "#eaebeb"),
        ("@pill@", "#d8d9d9"),
        ("@pill_ink@", "#191919"),
        ("@menu@", "#f2f2f2"),
        ("@menu_bd@", "rgba(0,0,0,0.12)"),
        ("@hover@", "#f0f0f0"),
    ],
};

const DARK: Skin = Skin {
    pairs: [
        ("@ink@", "#cccccc"),
        ("@dim@", "#7e7e7e"),
        ("@bg@", "#1a1a1a"),
        ("@rule@", "rgba(255,255,255,0.10)"),
        ("@sw_off@", "#3d3d3d"),
        ("@sw_on@", "#0a84c8"),
        ("@knob@", "#e8e8e8"),
        ("@knob_bd@", "rgba(0,0,0,0.3)"),
        ("@btn_bg@", "#262626"),
        ("@btn_bd@", "rgba(255,255,255,0.15)"),
        ("@field@", "#222222"),
        ("@accent@", "#0a84c8"),
        ("@check_bd@", "rgba(255,255,255,0.30)"),
        ("@list_bg@", "#151515"),
        ("@danger@", "#cf807e"),
        ("@track@", "#383838"),
        ("@side@", "#141615"),
        ("@pill@", "#393b3a"),
        ("@pill_ink@", "#e0e0e0"),
        ("@menu@", "#2e2e2e"),
        ("@menu_bd@", "rgba(255,255,255,0.13)"),
        ("@hover@", "#2e2e2e"),
    ],
};

/// The checks' tick: GTK's own resource, as the menus' is, recoloured white.
/// The dropdowns' popups tick their row with it too.
pub(super) const TICK: &str = "resource:///org/gtk/libgtk/theme/Default/assets/check-symbolic.svg";

/// The window's rules, each colour a placeholder a [`Skin`] fills in.
///
/// The values are the stub README's findings: the switch is 36 × 18 with a
/// 14 px knob, since a `GtkSwitch` is two sliders wide; the spin button is
/// 22 px inside a 1 px border with 26 px cells; the dropdown's popover sets its
/// own font, which it does not inherit; a path row under a top border is 31 px,
/// since borders add to `min-height`; the scale's knob overhangs its 3 px
/// trough by margins that do not halve.
const SHEET: &str = r#"
window.settings { background-color: @bg@; color: @ink@; font-family: @font@; font-size: 13px; }
window.settings scrolledwindow, window.settings viewport, window.settings stack { background: none; }
window.settings .settings-side { background-color: @side@; border-right: 1px solid @rule@; padding-top: 13px; }
window.settings .settings-side-head, window.settings .settings-caps {
  font-size: 10.5px; font-weight: 600; letter-spacing: 0.63px; color: @dim@;
}
window.settings .settings-side-head { padding: 0 17px 8px; }

window.settings entry.search {
  min-height: 22px; margin: 0 8px 8px; padding: 0 6px; border: 1px solid @btn_bd@; border-radius: 5px;
  background: @field@; color: @ink@; box-shadow: none; outline: none; font-size: 12px;
}
window.settings entry.search:focus-within { border-color: @accent@; }
window.settings entry.search > image { -gtk-icon-size: 12px; color: @dim@; margin: 0 4px 0 0; }
window.settings entry.search > image.right { margin: 0; }
window.settings entry.search > text { margin-left: 17px; }
window.settings entry.search > text > placeholder { color: @dim@; }
window.settings .settings-mag { color: @dim@; }

window.settings list.settings-nav { background: none; }
window.settings list.settings-nav > row {
  min-height: 28px; margin: 0 8px 1px; padding: 0 9px; border-radius: 5px; color: @ink@; background: none; outline: none;
}
window.settings list.settings-nav > row:hover { background-color: alpha(@pill@, 0.5); }
window.settings list.settings-nav > row:selected { background-color: @pill@; color: @pill_ink@; font-weight: 600; }
window.settings list.settings-nav > row:focus-visible { box-shadow: inset 0 0 0 1px @accent@; }

window.settings .settings-pane { padding: 18px 28px; }
window.settings .settings-head { margin-top: 14px; padding-bottom: 4px; min-height: 22px; }
window.settings .settings-head.first { margin-top: 0; }
window.settings .settings-hint { font-size: 11.5px; color: @dim@; }
window.settings .settings-row { min-height: 32px; }
window.settings .settings-row.tall { min-height: 46px; }
window.settings .settings-check { min-height: 28px; }

window.settings button {
  min-height: 22px; min-width: 0; padding: 0 10px; border: 1px solid @btn_bd@; border-radius: 5px;
  background: @btn_bg@; color: @ink@; box-shadow: none; text-shadow: none; outline: none; font-weight: normal;
}
window.settings button:hover { background: @hover@; }
window.settings button:focus-visible { border-color: @accent@; }
window.settings button.settings-small { min-height: 20px; }

window.settings dropdown > button > box { border-spacing: 8px; }
window.settings dropdown arrow {
  min-width: 10px; min-height: 10px; -gtk-icon-size: 10px; margin: 6px 0;
  -gtk-icon-source: -gtk-recolor(url("@chevron@"));
}
window.settings dropdown popover { font-family: @font@; font-size: 13px; }
window.settings dropdown popover > contents {
  background-color: @menu@; color: @ink@; border: 1px solid @menu_bd@; border-radius: 8px; padding: 4px 0;
  box-shadow: 0 1px 1px rgba(0,0,0,0.07), 0 8px 24px rgba(0,0,0,0.15);
}
window.settings dropdown popover listview { background: none; color: @ink@; margin: 0; padding: 0; }
window.settings dropdown popover scrolledwindow { margin: 0; padding: 0; }
window.settings dropdown popover listview > row {
  min-height: 24px; margin: 0 4px; padding: 0 8px; border-radius: 5px; outline: none; background: none;
}
window.settings dropdown popover listview > row:hover,
window.settings dropdown popover listview > row:focus-visible { background-color: @accent@; color: white; }
window.settings dropdown popover listview > row image { -gtk-icon-size: 10px; min-width: 10px; }

window.settings switch {
  min-width: 0; min-height: 0; padding: 0; margin: 0; border: none; border-radius: 9px;
  background: @sw_off@; box-shadow: none; outline: none;
}
window.settings switch:checked { background: @sw_on@; }
window.settings switch:focus-visible { outline: 2px solid alpha(@accent@, 0.45); outline-offset: 1px; }
window.settings switch > slider {
  min-width: 14px; min-height: 14px; margin: 2px; border: none; border-radius: 50%; background: @knob@;
  box-shadow: 0 0 0 1px @knob_bd@, 0 1px 2px rgba(0,0,0,0.2);
}
window.settings switch > image { color: transparent; -gtk-icon-size: 1px; min-width: 0; min-height: 0; }

window.settings spinbutton {
  min-height: 22px; padding: 0; border: 1px solid @btn_bd@; border-radius: 5px; background: @field@; color: @ink@;
  box-shadow: none; outline: none; font-feature-settings: "tnum";
}
window.settings spinbutton:focus-within { border-color: @accent@; }
window.settings spinbutton > text {
  min-width: 0; min-height: 0; padding: 0 0 0 8px; background: none; border: none; box-shadow: none; outline: none;
}
window.settings spinbutton > button {
  min-width: 26px; min-height: 22px; padding: 0; margin: 0; border: none; border-left: 1px solid @btn_bd@;
  border-radius: 0; background: none; -gtk-icon-size: 12px;
}
window.settings spinbutton > button:hover { background: @hover@; }
window.settings spinbutton > button:last-child { border-radius: 0 4px 4px 0; }

window.settings scale { padding: 0; min-height: 14px; outline: none; }
window.settings scale > value { color: @dim@; font-size: 12px; font-feature-settings: "tnum"; margin-right: 10px; }
window.settings scale > trough {
  min-height: 3px; margin: 0; border: none; border-radius: 3px; background: @track@; box-shadow: none; outline: none;
}
window.settings scale > trough > highlight { min-height: 3px; margin: 0; border: none; border-radius: 3px; background: @accent@; }
window.settings scale > trough > slider {
  min-width: 14px; min-height: 14px; margin: -6px -7px -5px -7px; border: none; border-radius: 50%; background: @knob@;
  box-shadow: 0 0 0 1px @knob_bd@, 0 1px 2px rgba(0,0,0,0.25);
}
window.settings scale:focus-visible > trough > slider { box-shadow: 0 0 0 2px alpha(@accent@, 0.6); }

window.settings checkbutton { padding: 0; outline: none; border-spacing: 0; }
window.settings checkbutton > check, window.settings checkbutton > radio {
  min-width: 12px; min-height: 12px; margin: 0 9px 0 0; padding: 0; border: 1px solid @check_bd@;
  background: @field@; box-shadow: none; color: white; -gtk-icon-size: 10px; -gtk-icon-source: none;
}
window.settings checkbutton > check { border-radius: 3px; }
window.settings checkbutton > radio { border-radius: 50%; }
window.settings checkbutton > check:checked {
  background: @accent@; border-color: @accent@; -gtk-icon-source: -gtk-recolor(url("@tick@"));
}
window.settings checkbutton > radio:checked {
  border-color: @accent@; background: radial-gradient(circle closest-side, white 38%, @accent@ 46%);
}
window.settings checkbutton:focus-visible > check, window.settings checkbutton:focus-visible > radio {
  outline: 2px solid alpha(@accent@, 0.45); outline-offset: 1px;
}

window.settings .settings-paths { border: 1px solid @btn_bd@; border-radius: 6px; background-color: @list_bg@; }
window.settings .settings-paths > box { min-height: 32px; padding: 0 3px 0 10px; }
window.settings .settings-paths > box:not(:first-child) { min-height: 31px; border-top: 1px solid @rule@; }

window.settings .settings-refused {
  padding: 6px 10px; border: 1px solid @btn_bd@; border-radius: 5px;
  font-family: "Quill Mono", monospace; font-size: 11px; color: @danger@;
}

window.settings list.settings-results { background: none; padding: 6px 0; }
window.settings list.settings-results > row {
  min-height: 34px; margin: 0 6px; padding: 0 10px; border-radius: 5px; outline: none; color: @ink@; background: none;
}
window.settings list.settings-results > row:selected { background-color: @accent@; color: white; }
window.settings list.settings-results > row:selected .settings-group { color: rgba(255,255,255,0.8); }
window.settings .settings-group { font-size: 11.5px; color: @dim@; }

window.settings .settings-lit {
  border-radius: 3px; transition-property: background-color, box-shadow;
  transition-duration: @fade@; transition-timing-function: ease-out;
}
window.settings .settings-lit.settings-hit {
  background-color: alpha(@accent@, 0.16); box-shadow: 0 0 0 5px alpha(@accent@, 0.16); transition-duration: 0s;
}
"#;

/// The chevron's file as the `file://` URI a stylesheet's `url()` takes.
///
/// A file, not a `data:` URL: `-gtk-recolor` loads through a `GFile`, which a
/// `data:` URL is not, and a plain `url("data:…")` is rasterised at 1x and
/// soft on a 2x output. Empty when the path cannot be a URI, which leaves the
/// arrow undrawn rather than the sheet unparsed.
fn chevron() -> String {
    let path = quill_engine::data::icons().join(quill_engine::data::CHEVRON);
    glib::filename_to_uri(&path, None).map_or_else(|_| String::new(), |uri| uri.to_string())
}

/// The window's stylesheet for `scheme`, appended to the chrome's so a ground
/// change reloads it ([`crate::chrome::stylesheet`]).
pub(crate) fn stylesheet(scheme: Scheme) -> String {
    let skin = match scheme {
        Scheme::Light => LIGHT,
        Scheme::Dark => DARK,
    };
    let mut sheet = SHEET
        .replace("@font@", CHROME_FONT)
        .replace("@tick@", TICK)
        .replace("@chevron@", &chevron())
        .replace("@fade@", &format!("{}ms", super::search::FADE_MS));
    for (name, value) in skin.pairs {
        sheet = sheet.replace(name, value);
    }
    sheet
}

/// The window's rules for its controls alone — the switch, the spin button,
/// the scale, the dropdown and its popup, the buttons and the checks — scoped
/// to `scope` instead of the window, for the Palette's settings rows, whose
/// controls are the window's own and must look it (#467).
///
/// The window's frame, its sidebar, its rows and its search results are left
/// behind: a rule is taken when its selector names a control and no
/// `settings-` class.
pub(crate) fn controls(scheme: Scheme, scope: &str) -> String {
    const CONTROLS: [&str; 5] = ["switch", "spinbutton", "scale", "dropdown", " button"];
    stylesheet(scheme)
        .split_inclusive("}\n")
        .map(str::trim_start)
        .filter(|rule| {
            let selector = rule.split('{').next().unwrap_or_default();
            !selector.contains("settings-")
                && (selector.contains("checkbutton")
                    || CONTROLS.iter().any(|control| selector.contains(control)))
        })
        .map(|rule| rule.replace("window.settings", scope))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chrome;
    use crate::ground::Ground;

    const SCHEMES: [Scheme; 2] = [Scheme::Light, Scheme::Dark];

    /// The rule `selector` opens in `sheet`, from its brace to the next.
    fn rule<'a>(sheet: &'a str, selector: &str) -> &'a str {
        let open = format!("\n{selector} {{");
        let start = sheet
            .find(&open)
            .unwrap_or_else(|| panic!("no `{selector}` rule"))
            + open.len();
        let end = start + sheet[start..].find('}').expect("an unclosed rule");
        &sheet[start..end]
    }

    #[test]
    fn the_chromes_sheet_carries_the_settings_sheet_on_both_grounds() {
        for scheme in SCHEMES {
            assert!(chrome::stylesheet(Ground::of(scheme)).contains(&stylesheet(scheme)));
        }
    }

    #[test]
    fn every_placeholder_is_filled_on_both_grounds() {
        for scheme in SCHEMES {
            assert!(!stylesheet(scheme).contains('@'), "{scheme:?}");
        }
    }

    #[test]
    fn the_grounds_differ_by_their_literals() {
        let (light, dark) = (stylesheet(Scheme::Light), stylesheet(Scheme::Dark));
        assert!(rule(&light, "window.settings").contains("background-color: #f7f7f7"));
        assert!(rule(&dark, "window.settings").contains("background-color: #1a1a1a"));
    }

    #[test]
    fn the_switch_is_two_fourteen_pixel_knobs_wide() {
        // README finding 1: "Switch is 36x18, not 32x18. `GtkSwitch` is
        // exactly two slider boxes wide; a 14 px knob with 2 px of air is 18 + 18."
        let sheet = stylesheet(Scheme::Light);
        let slider = rule(&sheet, "window.settings switch > slider");
        assert!(slider.contains("min-width: 14px; min-height: 14px; margin: 2px;"));
        assert!(rule(&sheet, "window.settings switch").contains("min-width: 0; min-height: 0;"));
    }

    #[test]
    fn the_spin_button_is_twenty_four_pixels_with_twenty_six_pixel_cells() {
        // README finding 3: "CSS reaches all of it (frame, 26 px cells, the
        // divider, 24 px height)" — 22 px inside a 1 px border.
        let sheet = stylesheet(Scheme::Light);
        let frame = rule(&sheet, "window.settings spinbutton");
        assert!(frame.contains("min-height: 22px; padding: 0; border: 1px solid"));
        let cell = rule(&sheet, "window.settings spinbutton > button");
        assert!(cell.contains("min-width: 26px; min-height: 22px;"));
        assert!(cell.contains("border-left: 1px solid"));
    }

    #[test]
    fn the_dropdowns_popover_sets_its_own_font_and_the_chevron_stays_square() {
        // README finding 5: the popover "does not inherit the window's
        // `font-size` — set it on `dropdown popover` again"; finding 4: the
        // chevron "needs `margin: 6px 0` to stay square".
        let sheet = stylesheet(Scheme::Light);
        let popover = rule(&sheet, "window.settings dropdown popover");
        assert!(popover.contains(&format!("font-family: {CHROME_FONT}; font-size: 13px;")));
        let arrow = rule(&sheet, "window.settings dropdown arrow");
        assert!(
            arrow.contains(
                "min-width: 10px; min-height: 10px; -gtk-icon-size: 10px; margin: 6px 0;"
            )
        );
        assert!(arrow.contains("-gtk-recolor(url(\"file://"));
    }

    #[test]
    fn a_path_row_under_a_border_is_one_pixel_shorter() {
        // README finding 8: "a path row with a `border-top` is
        // `min-height: 31px` to stay 32."
        let sheet = stylesheet(Scheme::Light);
        assert!(
            rule(&sheet, "window.settings .settings-paths > box").contains("min-height: 32px;")
        );
        let below = rule(
            &sheet,
            "window.settings .settings-paths > box:not(:first-child)",
        );
        assert!(below.contains("min-height: 31px; border-top: 1px solid"));
    }

    #[test]
    fn a_hint_row_is_forty_six_pixels() {
        // README finding 12: "hint rows are `min-height: 46px`."
        let sheet = stylesheet(Scheme::Light);
        assert!(rule(&sheet, "window.settings .settings-row.tall").contains("min-height: 46px;"));
    }

    #[test]
    fn the_scale_is_a_three_pixel_trough_under_a_fourteen_pixel_knob() {
        // README finding 6: "3 px trough, 14 px knob with `margin: -6px -7px
        // -5px` (11 px of overhang does not halve)".
        let sheet = stylesheet(Scheme::Light);
        assert!(rule(&sheet, "window.settings scale > trough").contains("min-height: 3px;"));
        let knob = rule(&sheet, "window.settings scale > trough > slider");
        assert!(knob.contains("min-width: 14px; min-height: 14px; margin: -6px -7px -5px -7px;"));
    }

    #[test]
    fn checks_and_radios_are_twelve_pixels_inside_a_border() {
        // README finding 7: "12 px + 1 px border = the board's 14. The
        // radio's dot is a `radial-gradient`".
        let sheet = stylesheet(Scheme::Light);
        let mark = rule(
            &sheet,
            "window.settings checkbutton > check, window.settings checkbutton > radio",
        );
        assert!(mark.contains("min-width: 12px; min-height: 12px;"));
        assert!(mark.contains("border: 1px solid"));
        let dot = rule(&sheet, "window.settings checkbutton > radio:checked");
        assert!(dot.contains("radial-gradient("));
    }

    #[test]
    fn heads_are_ten_and_a_half_pixels_semibold_and_tracked_in_pixels() {
        // README finding 2: "`letter-spacing` must be px (0.63px) … Size
        // 10.5px/600 matches the board."
        let sheet = stylesheet(Scheme::Light);
        let head = rule(
            &sheet,
            "window.settings .settings-side-head, window.settings .settings-caps",
        );
        assert!(head.contains("font-size: 10.5px; font-weight: 600; letter-spacing: 0.63px;"));
    }

    #[test]
    fn the_body_is_the_chromes_font_at_thirteen_pixels() {
        // README finding 10: "Adwaita Sans at 13px matches the board's Inter
        // to the pixel".
        let sheet = stylesheet(Scheme::Light);
        let window = rule(&sheet, "window.settings");
        assert!(window.contains(&format!("font-family: {CHROME_FONT}; font-size: 13px;")));
    }

    #[test]
    fn the_chevron_is_quills_own_file_and_the_tick_gtks_own_resource() {
        let uri = chevron();
        let (path, _) = glib::filename_from_uri(&uri).expect("the chevron is a file URI");
        assert_eq!(
            path,
            quill_engine::data::icons().join(quill_engine::data::CHEVRON)
        );
        assert!(path.is_file(), "no chevron at {}", path.display());
        assert!(!path.starts_with(std::env::temp_dir()));
        let sheet = stylesheet(Scheme::Dark);
        assert!(!sheet.contains("data:"));
        assert!(sheet.contains(&format!("url(\"{uri}\")")));
        // Every image the sheet names is the chevron or GTK's own tick: no
        // icon-theme name, which CSS would spell `-gtk-icontheme(`.
        assert!(!sheet.contains("-gtk-icontheme"));
        assert_eq!(sheet.matches("url(\"").count(), 2);
        assert!(sheet.contains(&format!("url(\"{TICK}\")")));
    }
}
