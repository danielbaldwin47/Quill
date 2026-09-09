//! HTML export: a standalone page, and the body fragment Copy as HTML carries.
//!
//! The body of both is the parser's own HTML for the Document, read through
//! [`crate::markdown`]'s one option set, so an export interprets the file
//! exactly as the Editor, the Preview and Stats do. The front matter reaches
//! neither: it is metadata rather than prose, which is why `markdown::options`
//! parses it as a block of its own, and [`events_without_front_matter`] drops
//! that block whole.
//!
//! [`page`] wraps that body in a head and a `<style>` generated from the
//! current [`crate::template`]. No font is embedded: each face is named by its
//! family with one of CSS's three generic families behind it, so a reader
//! without the Quill Faces still gets a page set the way the Template
//! describes it.
//!
//! The stylesheet says in CSS what [`crate::render`] says in Pango — the same
//! Template numbers, the same three toggles, the same two indents
//! ([`render::INDENT`] and [`render::FIRST_LINE`]). That the same design can be
//! written twice is ADR 0005's own rule, that every property a Template carries
//! is one Pango can style and CSS can express; the two do not drift because
//! both read the one Template rather than each other.
//!
//! Annotator marks never reach here. An export shows the Document, not the
//! Editor's styling (`docs/architecture.md` § Preview and Export).

use pulldown_cmark::{Event, Tag, TagEnd};

use crate::document::Document;
use crate::markdown;
use crate::render::{self, Toggles};
use crate::template::{Face, Palette, Paragraphs, Template};

/// The room the page leaves around its text block, as CSS writes a `padding`.
///
/// A Template owns typography alone and Export's margin is a paper's, so
/// neither of them answers what an HTML file leaves at its edges; this is the
/// file's own. A padding rather than a margin because the measure is set as a
/// `max-width` on the same element: padding sits outside that width, so the
/// text block stays exactly the Template's measure at any window size.
const PADDING: &str = "2rem 1rem";

/// The room a code block's ground leaves around its text.
///
/// The Template names no such number — [`crate::render`] reports the Well
/// ground as the block's own bounds and the widget fills them — so this is the
/// stylesheet's, and it is here to keep the ground off the glyphs.
const CODE_PADDING: &str = "0.5rem";

/// Every heading element, as one selector list.
const HEADINGS: &str = "h1, h2, h3, h4, h5, h6";

/// A standalone HTML page for `document`, set in `template` under `toggles`.
///
/// The head carries the charset and the Document's name; the `<style>` is
/// generated from the Template; the body is what [`fragment`] answers.
#[must_use]
pub fn page(document: &Document, template: &Template, toggles: Toggles) -> String {
    format!(
        "<!DOCTYPE html>\n\
         <html>\n\
         <head>\n\
         <meta charset=\"utf-8\">\n\
         <title>{title}</title>\n\
         <style>\n{style}</style>\n\
         </head>\n\
         <body>\n{body}</body>\n\
         </html>\n",
        title = escape(&document.name()),
        style = stylesheet(template, toggles),
        body = fragment(document),
    )
}

/// The body of `document` alone: no head, no stylesheet, no wrapper.
///
/// What Copy as HTML puts on the clipboard, for pasting into something that
/// brings its own styling. It begins at the first block's own tag.
#[must_use]
pub fn fragment(document: &Document) -> String {
    let mut body = String::new();
    pulldown_cmark::html::push_html(&mut body, events_without_front_matter(document.text()));
    body
}

/// The events of `text` with the metadata block dropped, start to end.
///
/// Named for the block the parser reports rather than for the fields inside
/// it: this is the whole block leaving the stream, and nothing here reads what
/// it holds.
fn events_without_front_matter(text: &str) -> impl Iterator<Item = Event<'_>> {
    let mut inside = false;
    markdown::events(text).filter_map(move |(event, _)| match event {
        Event::Start(Tag::MetadataBlock(_)) => {
            inside = true;
            None
        }
        Event::End(TagEnd::MetadataBlock(_)) => {
            inside = false;
            None
        }
        _ if inside => None,
        _ => Some(event),
    })
}

/// The stylesheet `template` and `toggles` come to.
///
/// Both palettes: the light one plain, so a reader who has expressed no
/// preference gets it, and the dark one under `prefers-color-scheme`.
fn stylesheet(template: &Template, toggles: Toggles) -> String {
    let sizes = &template.sizes;
    let rhythm = &template.rhythm;
    let indented = toggles.indent_paragraphs || template.paragraphs == Paragraphs::Indented;
    let centred = toggles.center_headings;
    // `render::Pass::space`, in CSS: an indented Template marks the break with
    // the indent, so nothing stands between two paragraphs.
    let between = if indented {
        0.0
    } else {
        rhythm.paragraph_spacing
    };

    let mut css = format!(
        ":root {{\n  color-scheme: light dark;\n  font-size: {base}pt;\n{light}}}\n\n\
         @media (prefers-color-scheme: dark) {{\n  :root {{\n{dark}  }}\n}}\n\n",
        base = sizes.base,
        light = variables(&template.light, "  "),
        dark = variables(&template.dark, "    "),
    );

    css.push_str(&format!(
        "body {{\n  \
           box-sizing: content-box;\n  \
           max-width: {measure}rem;\n  \
           margin: 0 auto;\n  \
           padding: {PADDING};\n  \
           background: var(--paper);\n  \
           color: var(--ink);\n  \
           font-family: {body};\n  \
           font-size: 1rem;\n  \
           line-height: {line_height};\n\
         }}\n\n",
        measure = rhythm.measure,
        body = stack(
            &template.faces.body,
            prose_generic(&template.faces.body.family)
        ),
        line_height = rhythm.line_height,
    ));

    // The three arms of `render::Pass::space`, in its own order: a heading's
    // own space wins on both sides of it, so the rule that opens the space
    // before a heading is written last and takes two headings in a row.
    css.push_str(&format!(
        "body > * {{\n  margin: 0;\n}}\n\n\
         body > * + * {{\n  margin-top: {between}rem;\n}}\n\n\
         body > :is({HEADINGS}) + * {{\n  margin-top: {after}rem;\n}}\n\n\
         body > * + :is({HEADINGS}) {{\n  margin-top: {before}rem;\n}}\n\n",
        after = rhythm.space_after_heading,
        before = rhythm.space_before_heading,
    ));

    css.push_str(&format!(
        "body > :is({HEADINGS}) {{\n  font-family: {heading};\n  font-weight: {weight};\n}}\n\n",
        heading = stack(
            &template.faces.heading,
            prose_generic(&template.faces.heading.family),
        ),
        weight = template.headings.weight,
    ));
    for (index, multiple) in sizes.headings.iter().enumerate() {
        css.push_str(&format!(
            "h{level} {{\n  font-size: {multiple}rem;\n}}\n\n",
            level = index + 1,
        ));
    }
    if centred {
        css.push_str(&format!(
            "body > :is({HEADINGS}) {{\n  text-align: center;\n}}\n\n"
        ));
    }

    css.push_str(&format!(
        // A code face is monospace by the role it is put to, whatever the
        // Template names it.
        "code, pre {{\n  font-family: {code};\n  font-size: {size}rem;\n  \
           background: var(--code-ground);\n}}\n\n\
         pre {{\n  padding: {CODE_PADDING};\n  overflow-x: auto;\n}}\n\n\
         pre > code {{\n  padding: 0;\n  background: none;\n}}\n\n",
        code = stack(&template.faces.code, "monospace"),
        size = sizes.code,
    ));

    css.push_str(&format!(
        // A quotation carries no bar: it is indented and set in body ink
        // (`template::Palette`). A list hangs its marker in the same indent.
        "blockquote {{\n  margin-left: {indent}rem;\n}}\n\n\
         ul, ol {{\n  padding-left: {indent}rem;\n}}\n\n\
         hr {{\n  border: 0;\n  border-top: 1px solid var(--muted);\n}}\n\n\
         a {{\n  color: var(--link);\n}}\n\n",
        indent = render::INDENT,
    ));

    if indented {
        css.push_str(&format!(
            // The first paragraph after a heading is never indented: there is
            // nothing above it the indent could tell it from.
            "body > p {{\n  text-indent: {first}rem;\n}}\n\n\
             body > :is({HEADINGS}) + p {{\n  text-indent: 0;\n}}\n\n",
            first = render::FIRST_LINE,
        ));
    }
    if toggles.number_headings {
        css.push_str(&numbering());
    }
    css
}

/// The five colour roles of `palette` as custom properties, each line behind
/// `indent`.
fn variables(palette: &Palette, indent: &str) -> String {
    let roles = [
        ("paper", palette.paper),
        ("ink", palette.ink),
        ("muted", palette.muted),
        ("link", palette.link),
        ("code-ground", palette.code_ground),
    ];
    roles
        .into_iter()
        .map(|(role, colour)| format!("{indent}--{role}: {};\n", colour.to_hex()))
        .collect()
}

/// The counter rules Number Headings comes to.
///
/// [`render::Numbering`](crate::render) is what these mirror: an H1 is the
/// Document's own name and stands bare, the numbering starts under it at H2,
/// and each level resets the ones below it. The number is followed by a space,
/// as the rendered page sets it.
fn numbering() -> String {
    let mut css = String::from("body {\n  counter-reset: h2 h3 h4 h5 h6;\n}\n\n");
    for level in 2..=6_u8 {
        let deeper: Vec<String> = ((level + 1)..=6).map(|below| format!("h{below}")).collect();
        let reset = if deeper.is_empty() {
            String::new()
        } else {
            format!("\n  counter-reset: {};", deeper.join(" "))
        };
        let number: Vec<String> = (2..=level)
            .map(|above| format!("counter(h{above})"))
            .collect();
        css.push_str(&format!(
            "h{level} {{\n  counter-increment: h{level};{reset}\n}}\n\n\
             h{level}::before {{\n  content: {} \" \";\n}}\n\n",
            number.join(" \".\" "),
        ));
    }
    css
}

/// `face` as a CSS `font-family`: the family it names, then `generic` behind
/// it for the reader who does not have that family.
fn stack(face: &Face, generic: &str) -> String {
    format!("\"{}\", {generic}", face.family)
}

/// The generic family a prose face falls back to.
///
/// CSS's three generics over the families the built-in Templates name: the
/// Quill Faces (`fonts/`) are cut on one cell, so `monospace`; Source Serif 4
/// is a serif; Inter is a sans, and so is anything a later Template names,
/// because a sans is what a reader's browser sets prose in.
///
/// The family's name is what is read because it is all a stylesheet has to go
/// on: an exported page is read on a machine that has none of these fonts, and
/// the name with a generic after it is the whole of the font stack there. A
/// loaded face's own metrics never reach it.
fn prose_generic(family: &str) -> &'static str {
    if family.starts_with("Quill ") {
        "monospace"
    } else if family.contains("Serif") {
        "serif"
    } else {
        "sans-serif"
    }
}

/// `text` with the four characters HTML reads as markup written as entities.
fn escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            _ => escaped.push(character),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::template;

    /// The shared test passage, as the Document a writer has open.
    fn sample() -> Document {
        Document::open(Path::new("../ref/sample.md")).expect("the shared test passage is here")
    }

    /// An untitled Document holding `text`.
    fn holding(text: &str) -> Document {
        let mut document = Document::untitled();
        document.reload(text.to_string());
        document
    }

    /// The built-in `id`, which every test here sets its Document in.
    fn built(id: &str) -> Template {
        template::built_in(id).expect("a built-in Template")
    }

    /// What stands between `<body>` and `</body>`.
    fn body_of(page: &str) -> &str {
        let start = page.find("<body>\n").expect("a body") + "<body>\n".len();
        let end = page.find("</body>").expect("a body's end");
        &page[start..end]
    }

    #[test]
    fn the_samples_body_is_the_parsers_html_and_carries_no_markup() {
        let page = page(&sample(), &built("modern"), Toggles::default());
        let body = body_of(&page);
        assert!(body.starts_with("<h1>The Lighthouse</h1>"), "{body}");
        assert!(body.contains("<strong>bottle</strong>"), "{body}");
        assert!(body.contains("<em>"), "{body}");
        assert!(body.contains("<li>the good knife</li>"), "{body}");
        for markup in ["# ", "## ", "**", "- ", "*Nothing"] {
            assert!(!body.contains(markup), "{markup} is still in {body}");
        }
    }

    #[test]
    fn the_front_matter_reaches_neither_the_page_nor_the_fragment() {
        let document = holding("---\ntitle: Kept out\nauthor: Her\n---\n\n# Heading\n\nProse.\n");
        let page = page(&document, &built("modern"), Toggles::default());
        assert!(!page.contains("Kept out"), "{page}");
        assert!(!page.contains("author"), "{page}");
        assert_eq!(
            fragment(&document),
            "<h1>Heading</h1>\n<p>Prose.</p>\n",
            "the fragment begins at the first block's tag",
        );
    }

    #[test]
    fn the_head_carries_the_charset_and_the_documents_name() {
        let page = page(&sample(), &built("modern"), Toggles::default());
        assert!(page.starts_with("<!DOCTYPE html>\n<html>\n"), "{page}");
        assert!(page.contains("<meta charset=\"utf-8\">"), "{page}");
        assert!(page.contains("<title>sample</title>"), "{page}");
    }

    #[test]
    fn a_name_with_markup_in_it_is_escaped_into_the_title() {
        assert_eq!(escape("a <b> & \"c\""), "a &lt;b&gt; &amp; &quot;c&quot;");
    }

    #[test]
    fn the_stylesheet_names_the_templates_families_and_sizes() {
        let page = page(&sample(), &built("modern"), Toggles::default());
        assert!(
            page.contains("font-family: \"Inter\", sans-serif;"),
            "{page}"
        );
        assert!(
            page.contains("font-family: \"Quill Mono\", monospace;"),
            "{page}"
        );
        assert!(page.contains("font-size: 16pt;"), "{page}");
        assert!(page.contains("font-size: 2rem;"), "{page}");
        assert!(page.contains("max-width: 34rem;"), "{page}");
        assert!(page.contains("line-height: 1.5;"), "{page}");
        assert!(page.contains("font-weight: 700;"), "{page}");
    }

    #[test]
    fn the_classic_serif_falls_back_to_serif_and_its_body_size_is_its_own() {
        let page = page(&sample(), &built("classic"), Toggles::default());
        assert!(
            page.contains("font-family: \"Source Serif 4\", serif;"),
            "{page}"
        );
        assert!(page.contains("font-size: 17pt;"), "{page}");
    }

    #[test]
    fn both_palettes_are_there_and_the_dark_one_is_under_the_media_query() {
        let page = page(&sample(), &built("modern"), Toggles::default());
        let query = page
            .find("@media (prefers-color-scheme: dark)")
            .expect("the media query");
        let light = page.find("--paper: #fcfcfc;").expect("the light paper");
        let dark = page.find("--paper: #101010;").expect("the dark paper");
        assert!(light < query, "the light palette stands plain");
        assert!(query < dark, "the dark palette stands under the query");
        for role in ["--ink", "--muted", "--link", "--code-ground"] {
            assert_eq!(page.matches(&format!("{role}: ")).count(), 2, "{role}");
        }
    }

    #[test]
    fn the_fragment_is_the_body_alone() {
        let fragment = fragment(&sample());
        assert!(fragment.starts_with("<h1>"), "{fragment}");
        for furniture in ["<style", "<html", "<head", "<body", "<!DOCTYPE"] {
            assert!(!fragment.contains(furniture), "{furniture} in {fragment}");
        }
    }

    #[test]
    fn number_headings_writes_the_counter_rules_and_nothing_writes_them_otherwise() {
        let on = Toggles {
            number_headings: true,
            ..Toggles::default()
        };
        let numbered = page(&sample(), &built("modern"), on);
        assert!(
            numbered.contains("counter-reset: h2 h3 h4 h5 h6;"),
            "{numbered}"
        );
        assert!(numbered.contains("counter-increment: h2;"), "{numbered}");
        assert!(
            numbered.contains("h2::before {\n  content: counter(h2) \" \";\n}"),
            "{numbered}"
        );
        assert!(
            numbered.contains("h3::before {\n  content: counter(h2) \".\" counter(h3) \" \";\n}"),
            "{numbered}"
        );
        assert!(
            !numbered.contains("counter-increment: h1;"),
            "an H1 stands bare"
        );
        let plain = page(&sample(), &built("modern"), Toggles::default());
        assert!(!plain.contains("counter-"), "{plain}");
    }

    #[test]
    fn center_headings_is_the_only_heading_alignment_input() {
        let ranged = page(&sample(), &built("classic"), Toggles::default());
        assert!(!ranged.contains("text-align: center;"), "{ranged}");
        let on = Toggles {
            center_headings: true,
            ..Toggles::default()
        };
        let centred = page(&sample(), &built("classic"), on);
        assert!(centred.contains("text-align: center;"), "{centred}");
        let modern_off = page(&sample(), &built("modern"), Toggles::default());
        assert!(!modern_off.contains("text-align: center;"), "{modern_off}");
    }

    #[test]
    fn indent_paragraphs_indents_them_and_closes_the_space_between_them() {
        let spaced = page(&sample(), &built("modern"), Toggles::default());
        assert!(!spaced.contains("text-indent"), "{spaced}");
        assert!(spaced.contains("margin-top: 1rem;"), "{spaced}");
        let on = Toggles {
            indent_paragraphs: true,
            ..Toggles::default()
        };
        let indented = page(&sample(), &built("modern"), on);
        assert!(
            indented.contains("body > p {\n  text-indent: 1.5rem;\n}"),
            "{indented}"
        );
        assert!(
            indented.contains("+ p {\n  text-indent: 0;\n}"),
            "the first paragraph after a heading is not indented"
        );
        assert!(indented.contains("margin-top: 0rem;"), "{indented}");
    }

    #[test]
    fn an_indented_template_indents_with_the_toggle_off() {
        let page = page(&sample(), &built("classic"), Toggles::default());
        assert!(page.contains("text-indent: 1.5rem;"), "{page}");
        assert!(page.contains("margin-top: 0rem;"), "{page}");
    }

    #[test]
    fn every_built_in_template_makes_a_page() {
        for id in template::IDS {
            let page = page(&sample(), &built(id), Toggles::default());
            assert!(page.starts_with("<!DOCTYPE html>"), "{id}");
            assert!(page.ends_with("</html>\n"), "{id}");
            assert!(page.contains("@media (prefers-color-scheme: dark)"), "{id}");
            assert!(
                body_of(&page).starts_with("<h1>The Lighthouse</h1>"),
                "{id}"
            );
        }
    }
}
