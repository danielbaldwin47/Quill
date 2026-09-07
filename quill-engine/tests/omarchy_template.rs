//! The Omarchy template, rendered as Omarchy renders it, is a palette file the
//! engine reads without a note.
//!
//! `packaging/quill.toml.tpl` is written in Omarchy's template contract: a
//! `{{ key }}` token becomes the theme's colour for that key, and a
//! `{{ mix a b N% }}` token becomes Omarchy's blend of two keys, each channel
//! `round(a * (1 - N/100) + b * N/100)`. Omarchy renders it on every
//! `omarchy theme set` (`omarchy-theme-set-templates`, `add_template_value`
//! and `mix_color`), and the rendered file is what a writer's `palette` names.
//!
//! This test re-states those two rules over a theme's `colors.toml` checked in
//! beside it — the owner's gruvbox-dark theme as Omarchy wrote it — and asks
//! the engine to read the result: no note, every role present under the table
//! the theme's `mode` names, the other ground untouched, and each role the
//! colour its mapping promises: the thirteen #159 § The template names, and
//! `link_rule`, which that list left out and the template chose for itself
//! (#238), plus the five Syntax roles (#312). The rules are a
//! re-statement, so the Hand test on the owner's desktop (#159) is the check
//! that they are Omarchy's; what this test guards is the template drifting
//! from the roles or from the contract without anyone noticing.

use std::collections::BTreeMap;
use std::fs;

use quill_engine::theme::{Colour, Palette, Role, Scheme};

/// The template the package installs and the README's copy step names.
const TEMPLATE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../packaging/quill.toml.tpl");

/// One Omarchy theme's `colors.toml`, as `omarchy theme set` leaves it under
/// `~/.local/state/omarchy/current/theme/`.
const SAMPLE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/omarchy-colors.toml");

/// Which theme key, or which blend, each role takes: the thirteen #159 § The
/// template names, and `link_rule` as the template's own choice, because the
/// spec's list left it out (#238), plus the five Syntax roles (#312).
/// A role missing here fails the test below
/// by name.
const MAPPING: [(Role, Source); 19] = [
    (Role::Paper, Source::Key("background")),
    (Role::Ink, Source::Key("foreground")),
    (Role::InkDim, Source::Key("dark_foreground")),
    (Role::Mark, Source::Key("muted")),
    (Role::Accent, Source::Key("accent")),
    (Role::Link, Source::Key("accent")),
    (Role::LinkRule, Source::Mix("accent", "background", 50)),
    (Role::Selection, Source::Key("selection")),
    (
        Role::SelectionIdle,
        Source::Mix("selection", "background", 50),
    ),
    (Role::CodeBg, Source::Mix("background", "foreground", 6)),
    (Role::Rule, Source::Key("muted")),
    (Role::Shadow, Source::Key("darker_background")),
    (Role::ChromeFg, Source::Key("dark_foreground")),
    (Role::ChromeFgStrong, Source::Key("foreground")),
    (Role::SyntaxNoun, Source::Key("red")),
    (Role::SyntaxVerb, Source::Key("blue")),
    (Role::SyntaxAdjective, Source::Key("brown")),
    (Role::SyntaxAdverb, Source::Key("magenta")),
    (Role::SyntaxConjunction, Source::Key("green")),
];

/// Where a role's colour comes from in the theme.
#[derive(Clone, Copy)]
enum Source {
    /// The theme's colour under this key.
    Key(&'static str),
    /// Omarchy's `mix a b N%`: N per cent of `b` laid over `a`.
    Mix(&'static str, &'static str, u32),
}

#[test]
fn the_template_renders_to_a_palette_the_engine_reads_without_a_note() {
    let theme = sample();
    let rendered = render(
        &fs::read_to_string(TEMPLATE).expect("the template is readable"),
        &theme,
    );

    let (palette, notes) = Palette::parse(&rendered);
    assert!(
        notes.is_empty(),
        "the rendered template read with notes: {notes:?}\n{rendered}"
    );

    let written = written(&theme);
    let untouched = written.other();
    for role in Role::ALL {
        assert!(
            palette.colour(written, role).is_some(),
            "`{}` is not in the rendered `[{}]` table",
            role.key(),
            theme["mode"]
        );
        assert_eq!(
            palette.colour(untouched, role),
            None,
            "the template wrote `{}` for the ground the theme's `mode` did not name",
            role.key()
        );
    }
}

#[test]
fn every_role_takes_the_theme_colour_the_mapping_promises() {
    let theme = sample();
    let rendered = render(
        &fs::read_to_string(TEMPLATE).expect("the template is readable"),
        &theme,
    );
    let (palette, _) = Palette::parse(&rendered);
    let scheme = written(&theme);

    let mut promised: Vec<Role> = MAPPING.iter().map(|(role, _)| *role).collect();
    promised.sort_by_key(|role| role.key());
    let mut all = Role::ALL.to_vec();
    all.sort_by_key(|role| role.key());
    assert_eq!(promised, all, "the mapping names every role once");

    for (role, source) in MAPPING {
        let expected = match source {
            Source::Key(key) => theme[key].clone(),
            Source::Mix(a, b, percent) => mix(&theme[a], &theme[b], percent),
        };
        assert_eq!(
            palette.colour(scheme, role),
            Colour::parse(&expected),
            "`{}` is not `{expected}`",
            role.key()
        );
    }
}

/// The ground the theme's `mode` names, which is the one table the template
/// writes. A sample whose `mode` is neither is a broken fixture, not a light
/// theme.
fn written(theme: &BTreeMap<String, String>) -> Scheme {
    match theme["mode"].as_str() {
        "dark" => Scheme::Dark,
        "light" => Scheme::Light,
        other => panic!("the sample's `mode` is `{other}`, neither `dark` nor `light`"),
    }
}

/// The sample theme's string keys — `mode` and every colour — by name.
fn sample() -> BTreeMap<String, String> {
    let text = fs::read_to_string(SAMPLE).expect("the sample colors.toml is readable");
    let table: toml::Table = text.parse().expect("the sample colors.toml is TOML");
    table
        .into_iter()
        .filter_map(|(key, value)| Some((key, value.as_str()?.to_owned())))
        .collect()
}

/// Omarchy's two substitutions, re-stated: `{{ key }}` is the theme's value for
/// `key`, and `{{ mix a b N% }}` is [`mix`] of the theme's `a` and `b`. A token
/// the theme has no key for is the failure Omarchy would leave unreplaced, so
/// it is a panic naming the token rather than a silent `{{ … }}` in the output.
fn render(template: &str, theme: &BTreeMap<String, String>) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(open) = rest.find("{{") {
        out.push_str(&rest[..open]);
        let after = &rest[open + 2..];
        let close = after
            .find("}}")
            .expect("every `{{` in the template has its `}}`");
        let token = after[..close].trim();
        let words: Vec<&str> = token.split_whitespace().collect();
        let value = match words.as_slice() {
            [key] => theme
                .get(*key)
                .unwrap_or_else(|| panic!("the theme has no `{key}` for `{{{{ {token} }}}}`"))
                .clone(),
            ["mix", a, b, amount] => {
                let percent: u32 = amount
                    .strip_suffix('%')
                    .and_then(|digits| digits.parse().ok())
                    .unwrap_or_else(|| panic!("`{{{{ {token} }}}}` does not end in `N%`"));
                mix(&theme[*a], &theme[*b], percent)
            }
            _ => {
                panic!("`{{{{ {token} }}}}` is neither `{{{{ key }}}}` nor `{{{{ mix a b N% }}}}`")
            }
        };
        out.push_str(&value);
        rest = &after[close + 2..];
    }
    out.push_str(rest);
    out
}

/// Omarchy's `mix_color`: each channel `int(a * (1 - N/100) + b * N/100 + 0.5)`,
/// which for a whole-number percentage is this integer arithmetic exactly.
fn mix(a: &str, b: &str, percent: u32) -> String {
    let (a, b) = (rgb(a), rgb(b));
    let blend = |i: usize| {
        let mixed = (u32::from(a[i]) * (100 - percent) + u32::from(b[i]) * percent + 50) / 100;
        u8::try_from(mixed).expect("a blend of two channels is a channel")
    };
    format!("#{:02x}{:02x}{:02x}", blend(0), blend(1), blend(2))
}

/// The three channels of a `#rrggbb`, the only form Omarchy will mix.
fn rgb(colour: &str) -> [u8; 3] {
    let hex = colour
        .strip_prefix('#')
        .filter(|hex| hex.len() == 6)
        .unwrap_or_else(|| panic!("`{colour}` is not a `#rrggbb` Omarchy can mix"));
    let channel = |i: usize| {
        u8::from_str_radix(&hex[i..i + 2], 16).unwrap_or_else(|_| panic!("`{colour}` is not hex"))
    };
    [channel(0), channel(2), channel(4)]
}
