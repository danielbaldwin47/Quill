//! The Palette's list: the registry in the oracle's four sections and More,
//! and the match that narrows it as the writer types.
//!
//! With nothing typed the Palette is a map of the app in the order a writer
//! meets it (`legacy/app/js/chrome.js`, `SECTIONS`); typing turns it into one
//! ranked list. Both are pure functions of the registry, so the popover only
//! draws what it is handed.
//!
//! The match on a title is the oracle's, ported from `chrome.js`; the match
//! on a radio Command's group below it is Quill's own (#229), so that
//! `theme` reaches Follow System, whose title holds no word of the query.

use std::borrow::Cow;
use std::path::{Path, PathBuf};

use crate::commands::{COMMANDS, Command, Kind};
use crate::document::shown_name;
use crate::outline::Heading;

/// The oracle's four sections, by the registry's ids in the oracle's order
/// and membership (`chrome.js:364-368`; the oracle's one `file.export` is
/// `export.pdf` here, and the other four Export Commands, which the oracle has
/// no row for, fall to [`MORE`] with the rest of the table).
pub const SECTIONS: [(&str, &[&str]); 4] = [
    (
        "Write",
        &[
            "focus.toggle",
            "focus.sentence",
            "focus.paragraph",
            "typewriter.toggle",
        ],
    ),
    (
        "Text",
        &[
            "font.duo",
            "font.quattro",
            "font.mono",
            "font.bigger",
            "font.smaller",
            "font.reset",
        ],
    ),
    (
        "View",
        &[
            "theme.toggle",
            "theme.light",
            "theme.dark",
            "theme.auto",
            "chrome.stats",
            "chrome.toggle",
            "library.toggle",
            "library.search",
        ],
    ),
    (
        "Document",
        &[
            "file.new",
            "file.open",
            "file.openFolder",
            "file.save",
            "file.saveAs",
            "file.rename",
            "file.duplicate",
            "export.pdf",
            "file.next",
            "file.prev",
            "file.follow",
            "outline.open",
            "file.delete",
        ],
    ),
];

/// The fifth section: every registry Command in none of the four, in the
/// table's order. The oracle sorts these by title; the table's order is the
/// one the menus already read in.
pub const MORE: &str = "More";

/// How a row matched the query, best first: the oracle's three tiers on the
/// title (`chrome.js` ranks a prefix over a substring over a subsequence),
/// then Quill's own on the radio group, below them all.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Tier {
    /// The title starts with the query.
    Prefix,
    /// The title contains the query further in.
    Inside,
    /// The title has the query's letters in order, with gaps.
    Letters,
    /// The title did not match at all and the Command's radio group did
    /// (#229, not the oracle's). Last, so an invisible match never outranks
    /// a visible one.
    Group,
}

/// Where a match stands in the ranked list, lowest first: its [`Tier`],
/// then the byte the match begins at, so within a tier the earlier match is
/// the higher row.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Rank(Tier, usize);

/// One row of the list: its Command and, when a query matched it, the byte
/// ranges of the title it matched, which the row sets heavier.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    /// The Command the row runs.
    pub command: &'static Command,
    /// The byte ranges of its title the query matched; none at rest.
    pub hits: Vec<(usize, usize)>,
}

/// The list at rest: the four sections then More, every Command once.
#[must_use]
pub fn sections() -> Vec<(&'static str, Vec<&'static Command>)> {
    let mut placed: Vec<&'static str> = Vec::new();
    let mut out: Vec<(&'static str, Vec<&'static Command>)> = SECTIONS
        .iter()
        .map(|(head, ids)| {
            let rows: Vec<&'static Command> = ids
                .iter()
                .filter_map(|id| COMMANDS.iter().find(|command| command.id == *id))
                .collect();
            placed.extend(rows.iter().map(|command| command.id));
            (*head, rows)
        })
        .collect();
    let rest: Vec<&'static Command> = COMMANDS
        .iter()
        .filter(|command| !placed.contains(&command.id))
        .collect();
    if !rest.is_empty() {
        out.push((MORE, rest));
    }
    out
}

/// The list for `query`: the map at rest, section by section, when the
/// query is blank; otherwise one ranked list under no heading, the rows
/// ordered by [`Rank`] and then by title. A row whose title misses is still
/// listed when its radio group matches, at [`Tier::Group`] below every title
/// match and in the registry's order, since it has no title offset to rank
/// by and no highlight to show.
#[must_use]
pub fn list(query: &str) -> Vec<(Option<&'static str>, Vec<Row>)> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return sections()
            .into_iter()
            .map(|(head, commands)| {
                let rows = commands
                    .into_iter()
                    .map(|command| Row {
                        command,
                        hits: Vec::new(),
                    })
                    .collect();
                (Some(head), rows)
            })
            .collect();
    }
    // Each row beside the key it sorts on: its rank, then its title to
    // break a tie. A group match has no title to break the tie with, so it
    // takes the empty string and a stable sort leaves those rows in the
    // order the registry gave them.
    let mut ranked: Vec<((Rank, &'static str), Row)> = COMMANDS
        .iter()
        .filter_map(|command| {
            if let Some((rank, hits)) = score(command.title, &query) {
                return Some(((rank, command.title), Row { command, hits }));
            }
            score_group(command, &query).map(|rank| {
                (
                    (rank, ""),
                    Row {
                        command,
                        hits: Vec::new(),
                    },
                )
            })
        })
        .collect();
    ranked.sort_by_key(|(key, _)| *key);
    vec![(None, ranked.into_iter().map(|(_, row)| row).collect())]
}

/// One row of the Palette's recents list: a Document the writer opened
/// before, as `file.recent` lists it (#246, story 41).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Recent<'a> {
    /// The Document the row opens.
    pub path: &'a Path,
    /// What the row reads: the file name without its extension, the name the
    /// top bar shows it under ([`shown_name`]).
    pub name: Cow<'a, str>,
    /// The byte ranges of that name the query matched; none at rest.
    pub hits: Vec<(usize, usize)>,
}

/// The recents list for `query`: `opened` as it stands, newest first, when
/// the query is blank; otherwise the rows whose name matched, by [`Rank`].
///
/// The match is [`score`] over the name alone, the same rule the Commands
/// are narrowed by, so `sst` finds `sea-storm.md` here as it does in the
/// Library's search. Within a rank the sort is stable, so two names matched
/// alike stay newest first, which is the order the writer was handed them in.
#[must_use]
pub fn recents<'a>(opened: &'a [PathBuf], query: &str) -> Vec<Recent<'a>> {
    let query = query.trim().to_lowercase();
    let named = opened.iter().map(|path| Recent {
        path: path.as_path(),
        name: shown_name(path),
        hits: Vec::new(),
    });
    if query.is_empty() {
        return named.collect();
    }
    let mut ranked: Vec<(Rank, Recent<'a>)> = named
        .filter_map(|row| {
            let (rank, hits) = score(&row.name, &query)?;
            Some((rank, Recent { hits, ..row }))
        })
        .collect();
    ranked.sort_by_key(|(rank, _)| *rank);
    ranked.into_iter().map(|(_, row)| row).collect()
}

/// The head the Outline listing draws over its Documents.
pub const DOCUMENTS: &str = "Documents";

/// The most Documents the Outline listing appends under [`DOCUMENTS`].
pub const DOCUMENTS_CAP: usize = 10;

/// One row of the Palette's Outline listing (#397, `outline.open`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outlined<'a> {
    /// A heading of the open Document, which Enter jumps to.
    Heading {
        /// The byte its words start at, where the jump puts the caret.
        start: usize,
        /// Its level, 1 to 6, which the row indents by.
        level: u8,
        /// Its words, as the Editor shows them.
        text: &'a str,
        /// The byte ranges of those words the query matched; none at rest.
        hits: Vec<(usize, usize)>,
    },
    /// The head over the Documents, [`DOCUMENTS`]: neither selected nor run.
    Head(&'static str),
    /// A Document of the Library by name, which Enter opens; drawn as a
    /// recents row is.
    Document(Recent<'a>),
    /// The dim line standing where the headings would be, for a Document
    /// with none: neither selected nor run.
    NoHeadings,
}

/// The Outline listing for one query: its rows and which of the rows that
/// can be selected is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutlineList<'a> {
    /// The rows, top to bottom, heads and the dim line among them.
    pub rows: Vec<Outlined<'a>>,
    /// The index, among the rows that can be selected — headings and
    /// Documents, top to bottom — of the selected one.
    pub selected: usize,
}

/// The Outline listing for `query`: with nothing typed, `headings` alone in
/// reading order with `section` — the caret's, from
/// [`crate::outline::section`] — selected, or the first row when the caret
/// stands above every heading; once something is typed, the headings it
/// matched by [`score`], ranked, then under [`DOCUMENTS`] the first
/// [`DOCUMENTS_CAP`] of `documents` — the Library's name matches, in the
/// Library's own rank — that are not `open`, the first matched heading
/// selected. A Document with no headings lists [`Outlined::NoHeadings`]
/// where they would stand, and its Documents beneath as ever.
#[must_use]
pub fn outline<'a>(
    headings: &'a [Heading],
    section: Option<usize>,
    documents: &'a [PathBuf],
    open: Option<&Path>,
    query: &str,
) -> OutlineList<'a> {
    let query = query.trim().to_lowercase();
    let entry = |heading: &'a Heading, hits| Outlined::Heading {
        start: heading.source.start,
        level: heading.level,
        text: &heading.text,
        hits,
    };
    let mut rows = Vec::new();
    if headings.is_empty() {
        rows.push(Outlined::NoHeadings);
    } else if query.is_empty() {
        rows.extend(headings.iter().map(|heading| entry(heading, Vec::new())));
    } else {
        let mut ranked: Vec<(Rank, Outlined<'a>)> = headings
            .iter()
            .filter_map(|heading| {
                let (rank, hits) = score(&heading.text, &query)?;
                Some((rank, entry(heading, hits)))
            })
            .collect();
        ranked.sort_by_key(|(rank, _)| *rank);
        rows.extend(ranked.into_iter().map(|(_, row)| row));
    }
    if !query.is_empty() {
        let mut listed = documents
            .iter()
            .filter(|path| open.is_none_or(|open| open != path.as_path()))
            .take(DOCUMENTS_CAP)
            .map(|path| {
                Outlined::Document(Recent {
                    path,
                    name: shown_name(path),
                    hits: Vec::new(),
                })
            })
            .peekable();
        if listed.peek().is_some() {
            rows.push(Outlined::Head(DOCUMENTS));
            rows.extend(listed);
        }
    }
    let selected = if query.is_empty() {
        section.unwrap_or(0)
    } else {
        0
    };
    OutlineList { rows, selected }
}

/// Quill's own fallback (#229): `query`, already lower-cased, against a radio
/// Command's group, by the same substring-or-letters rule the title uses, so
/// `theme` reaches Follow System and `stats` the counts `commands.rs` groups
/// under it. Every hit ranks [`Tier::Group`], below every title match, and
/// carries no highlight, the group being nowhere on screen. A Command with
/// no group never matches.
#[must_use]
fn score_group(command: &Command, query: &str) -> Option<Rank> {
    let Kind::Radio { group, .. } = command.kind else {
        return None;
    };
    score(group, query).map(|_| Rank(Tier::Group, 0))
}

/// The oracle's match (`chrome.js:344-352`): `query`, already lower-cased,
/// against `title`. A substring ranks first, at its offset; failing that the
/// query's letters in order anywhere in the title, "sen" finding "Sentence"
/// and "tw" "Typewriter"; failing that, no row. The hits are byte ranges of
/// `title`.
///
/// [`score_group`] runs the same rule over a radio Command's group, which is
/// how [`list`] reaches a row this one turns away.
#[must_use]
pub fn score(title: &str, query: &str) -> Option<(Rank, Vec<(usize, usize)>)> {
    if query.is_empty() {
        return Some((Rank(Tier::Prefix, 0), Vec::new()));
    }
    // Each character of the title lower-cased beside where it starts and
    // ends in the title, so a hit can be handed back as bytes of the title.
    let chars: Vec<(usize, usize, char)> = title
        .char_indices()
        .map(|(at, c)| {
            let lower = c.to_lowercase().next().unwrap_or(c);
            (at, at + c.len_utf8(), lower)
        })
        .collect();
    let wanted: Vec<char> = query.chars().collect();
    if let Some(at) = (0..chars.len()).find(|&start| {
        chars.len() - start >= wanted.len()
            && wanted
                .iter()
                .enumerate()
                .all(|(k, c)| chars[start + k].2 == *c)
    }) {
        let end = chars[at + wanted.len() - 1].1;
        let tier = if at == 0 { Tier::Prefix } else { Tier::Inside };
        let rank = Rank(tier, at);
        return Some((rank, vec![(chars[at].0, end)]));
    }
    let mut hits = Vec::new();
    let mut k = 0;
    for (from, to, c) in &chars {
        if k < wanted.len() && *c == wanted[k] {
            hits.push((*from, *to));
            k += 1;
        }
    }
    if k < wanted.len() {
        return None;
    }
    let first = chars
        .iter()
        .position(|(from, _, _)| *from == hits[0].0)
        .unwrap_or(0);
    Some((Rank(Tier::Letters, first), hits))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_command_is_in_exactly_one_section_and_the_oracles_four_come_first() {
        let sections = sections();
        let heads: Vec<&str> = sections.iter().map(|(head, _)| *head).collect();
        assert_eq!(heads, ["Write", "Text", "View", "Document", MORE]);
        let mut listed: Vec<&str> = sections
            .iter()
            .flat_map(|(_, rows)| rows.iter().map(|command| command.id))
            .collect();
        let mut all: Vec<&str> = COMMANDS.iter().map(|command| command.id).collect();
        assert_eq!(
            listed.len(),
            all.len(),
            "a Command listed twice or not at all"
        );
        listed.sort_unstable();
        all.sort_unstable();
        assert_eq!(listed, all);
        // The oracle's membership, id for id, in its order.
        for ((head, ids), (got, rows)) in SECTIONS.iter().zip(&sections) {
            assert_eq!(head, got);
            let got_ids: Vec<&str> = rows.iter().map(|command| command.id).collect();
            assert_eq!(&got_ids, ids);
        }
    }

    #[test]
    fn more_is_the_rest_of_the_table_in_the_tables_order() {
        let sections = sections();
        let (_, more) = sections.last().unwrap();
        let ids: Vec<&str> = more.iter().map(|command| command.id).collect();
        assert!(ids.contains(&"window.fullscreen"));
        assert!(ids.contains(&"settings.open"));
        assert!(ids.contains(&"shortcuts.open"));
        assert!(ids.contains(&"syntax.toggle"));
        assert!(ids.contains(&"style.toggle"));
        assert!(ids.contains(&"style.fillers"));
        assert!(ids.contains(&"style.redundancies"));
        assert!(ids.contains(&"style.cliches"));
        assert!(ids.contains(&"spell.toggle"));
        assert!(ids.contains(&"chrome.view"));
        assert!(ids.contains(&"focus.swap"));
        let table: Vec<usize> = ids
            .iter()
            .map(|id| {
                COMMANDS
                    .iter()
                    .position(|command| command.id == *id)
                    .unwrap()
            })
            .collect();
        assert!(table.windows(2).all(|pair| pair[0] < pair[1]));
    }

    #[test]
    fn dark_lists_dark_mode_first() {
        let list = list("dark");
        assert_eq!(list.len(), 1);
        let (head, rows) = &list[0];
        assert_eq!(*head, None);
        assert_eq!(rows[0].command.id, "theme.toggle");
        assert_eq!(rows[0].hits, vec![(0, 4)]);
        assert_eq!(rows[1].command.id, "theme.dark");
    }

    #[test]
    fn a_blank_query_is_the_map_and_a_miss_is_nothing() {
        let at_rest = list("  ");
        assert_eq!(at_rest.len(), 5);
        assert_eq!(at_rest[0].0, Some("Write"));
        assert_eq!(at_rest[0].1[0].command.id, "focus.toggle");
        assert!(at_rest[0].1[0].hits.is_empty());
        let (_, none) = &list("zzzq")[0];
        assert!(none.is_empty());
    }

    #[test]
    fn the_match_is_the_oracles_substring_first_then_letters_in_order() {
        assert_eq!(
            score("Sentence", "sen"),
            Some((Rank(Tier::Prefix, 0), vec![(0, 3)]))
        );
        assert_eq!(
            score("Focus: Sentence", "sen"),
            Some((Rank(Tier::Inside, 7), vec![(7, 10)]))
        );
        assert_eq!(
            score("Typewriter", "tw"),
            Some((Rank(Tier::Letters, 0), vec![(0, 1), (4, 5)]))
        );
        assert_eq!(score("Typewriter", "xq"), None);
        // A later substring outranks an earlier scatter.
        assert!(score("Focus: Sentence", "sen").unwrap().0 < score("Typewriter", "tw").unwrap().0);
    }

    fn command(id: &str) -> &'static Command {
        COMMANDS.iter().find(|command| command.id == id).unwrap()
    }

    /// The ids of the rows `query` reached through their group. A query this
    /// side of blank is the only kind `list` ranks, so a row it lists with
    /// nothing highlighted is a row whose title matched nothing.
    fn group_rows(query: &str) -> Vec<&'static str> {
        let (_, rows) = &list(query)[0];
        rows.iter()
            .filter(|row| row.hits.is_empty())
            .map(|row| row.command.id)
            .collect()
    }

    #[test]
    fn theme_reaches_follow_system_through_its_group() {
        let (_, rows) = &list("theme")[0];
        let ids: Vec<&str> = rows.iter().map(|row| row.command.id).collect();
        // Dark Theme holds the query nearer its start than Light Theme, so
        // it leads; Follow System's title holds it nowhere, and is listed
        // for its group with nothing to highlight.
        assert_eq!(ids, ["theme.dark", "theme.light", "theme.auto"]);
        assert_eq!(rows[0].hits, vec![(5, 10)]);
        assert_eq!(rows[1].hits, vec![(6, 11)]);
        assert!(rows[2].hits.is_empty());
    }

    #[test]
    fn a_group_match_ranks_below_every_title_match_and_keeps_the_registrys_order() {
        // "Preview Full" holds the query in its title, so it outranks the two
        // rows the `preview_mode` group brings in on their group alone.
        assert!(
            score("Preview Full", "preview").unwrap().0
                < score_group(command("preview.web"), "preview").unwrap()
        );
        let (_, rows) = &list("preview")[0];
        let ids: Vec<&str> = rows.iter().map(|row| row.command.id).collect();
        let group = group_rows("preview");
        let titled = ids.len() - group.len();
        assert!(
            rows[..titled].iter().all(|row| !row.hits.is_empty()),
            "every row above the group tier matched by title"
        );
        assert_eq!(
            &ids[titled..],
            &group[..],
            "every row below them is a group match"
        );
        assert_eq!(
            group,
            ["preview.web", "preview.pdf"],
            "the group tier keeps the registry's order, not the alphabet"
        );
    }

    #[test]
    fn a_group_reaches_the_focus_scopes_and_the_faces() {
        let (_, rows) = &list("focus")[0];
        let ids: Vec<&str> = rows.iter().map(|row| row.command.id).collect();
        // `focus_scope` starts with the query; the two titles holding
        // "Focus" come first all the same.
        assert_eq!(
            ids,
            [
                "focus.toggle",
                "focus.swap",
                "focus.sentence",
                "focus.paragraph"
            ]
        );
        // The typeface group is `face`, the settings key, so `face` reaches
        // the three faces and `font` — neither title nor group — reaches
        // none of them (#229 out of scope: a keywords column).
        assert_eq!(
            group_rows("face"),
            ["font.duo", "font.quattro", "font.mono"]
        );
        let (_, rows) = &list("font")[0];
        assert!(rows.iter().all(|row| !row.command.id.starts_with("font.")));
    }

    /// The Documents a state would hand `file.recent`, newest first.
    fn opened() -> Vec<PathBuf> {
        [
            "/home/w/sea-storm.md",
            "/home/w/notes.txt",
            "/home/w/set.md",
        ]
        .iter()
        .map(PathBuf::from)
        .collect()
    }

    /// The names of the rows `query` leaves, in the order they are listed.
    fn listed(opened: &[PathBuf], query: &str) -> Vec<String> {
        recents(opened, query)
            .into_iter()
            .map(|row| row.name.into_owned())
            .collect()
    }

    /// At rest the recents are the state's order, newest first, under the
    /// name the top bar shows each Document by.
    #[test]
    fn the_recents_are_newest_first_under_the_name_without_the_extension() {
        let opened = opened();
        assert_eq!(listed(&opened, ""), ["sea-storm", "notes", "set"]);
        assert_eq!(listed(&opened, "  "), ["sea-storm", "notes", "set"]);
        let rows = recents(&opened, "");
        assert_eq!(rows[0].path, Path::new("/home/w/sea-storm.md"));
        assert!(rows[0].hits.is_empty());
    }

    /// Typing narrows them by the same rule the Commands are narrowed by:
    /// a substring first, then the letters in order, and the letters they
    /// matched are handed back to be marked.
    #[test]
    fn typing_narrows_the_recents_by_name_and_marks_what_matched() {
        let opened = opened();
        // "set" is a whole word of one name and scattered through another.
        assert_eq!(listed(&opened, "set"), ["set", "sea-storm"]);
        let rows = recents(&opened, "sto");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "sea-storm");
        assert_eq!(rows[0].hits, vec![(4, 7)]);
        assert!(listed(&opened, "zzzq").is_empty());
    }

    /// Two names matched alike stay in the order the state handed them over,
    /// so the newer of the two is the higher row.
    #[test]
    fn recents_matched_alike_stay_newest_first() {
        let opened: Vec<PathBuf> = ["/w/new/draft.md", "/w/old/draft.md"]
            .iter()
            .map(PathBuf::from)
            .collect();
        let rows = recents(&opened, "draft");
        assert_eq!(
            rows.iter().map(|row| row.path).collect::<Vec<_>>(),
            [Path::new("/w/new/draft.md"), Path::new("/w/old/draft.md")]
        );
    }

    #[test]
    fn a_command_with_no_group_is_matched_by_no_group_name() {
        assert_eq!(score_group(command("app.quit"), "theme"), None);
        assert_eq!(score_group(command("palette.open"), "face"), None);
        // A check is not a radio, whatever its id shares with a group.
        assert_eq!(score_group(command("theme.toggle"), "theme"), None);
        // So every row the group tier brings in is a radio.
        for query in ["theme", "preview", "focus", "face"] {
            for id in group_rows(query) {
                assert!(
                    matches!(command(id).kind, Kind::Radio { .. }),
                    "{id} was listed for `{query}` with no title match"
                );
            }
        }
    }

    /// The Outline of the shared passage as `of_blocks` reads it, with a
    /// third heading so a query can rank two matches.
    fn headings() -> Vec<Heading> {
        [
            (1, "The Lighthouse", 0),
            (2, "What the sea keeps", 4),
            (2, "Sea glass", 8),
        ]
        .iter()
        .map(|&(level, text, block)| Heading {
            level,
            text: text.to_string(),
            block,
            source: block..block + text.len(),
        })
        .collect()
    }

    /// Twelve Documents of the Library, the open one among them.
    fn library() -> Vec<PathBuf> {
        (1..=12)
            .map(|n| PathBuf::from(format!("/w/sea-{n}.md")))
            .collect()
    }

    /// What each row of `list` reads: a heading's words, a Document's name,
    /// a head's word, or the dim line's.
    fn read(list: &OutlineList<'_>) -> Vec<String> {
        list.rows
            .iter()
            .map(|row| match row {
                Outlined::Heading { text, .. } => (*text).to_string(),
                Outlined::Head(head) => format!("[{head}]"),
                Outlined::Document(recent) => recent.name.to_string(),
                Outlined::NoHeadings => "(no headings)".to_string(),
            })
            .collect()
    }

    /// Nothing typed lists the headings alone, in reading order, with the
    /// caret's section selected — or the first row above every heading.
    #[test]
    fn at_rest_the_outline_lists_the_headings_with_the_carets_section_selected() {
        let headings = headings();
        let library = library();
        let list = outline(&headings, Some(1), &library, None, "");
        assert_eq!(
            read(&list),
            ["The Lighthouse", "What the sea keeps", "Sea glass"]
        );
        assert_eq!(list.selected, 1);
        assert!(matches!(
            list.rows[1],
            Outlined::Heading {
                start: 4,
                level: 2,
                ..
            }
        ));
        assert_eq!(outline(&headings, None, &library, None, "  ").selected, 0);
    }

    /// Typing ranks the matched headings first, then up to ten Documents
    /// under their head, the open Document left out, the first heading
    /// selected.
    #[test]
    fn typing_ranks_the_headings_then_caps_the_documents_at_ten() {
        let headings = headings();
        let library = library();
        let list = outline(
            &headings,
            Some(2),
            &library,
            Some(Path::new("/w/sea-3.md")),
            "sea",
        );
        let rows = read(&list);
        assert_eq!(
            &rows[..3],
            ["Sea glass", "What the sea keeps", "[Documents]"]
        );
        assert_eq!(rows.len(), 3 + DOCUMENTS_CAP);
        assert!(!rows.contains(&"sea-3".to_string()));
        assert_eq!(rows[3], "sea-1");
        assert_eq!(list.selected, 0);
        let Outlined::Heading { hits, start, .. } = &list.rows[0] else {
            panic!("a heading leads");
        };
        assert_eq!((*start, hits.as_slice()), (8, &[(0, 3)][..]));
        // A query no heading matches lists the Documents alone.
        assert_eq!(
            read(&outline(&headings, None, &library, None, "zzq"))[0],
            "[Documents]"
        );
        // And one no Document matches lists the headings alone, no head.
        assert_eq!(
            read(&outline(&headings, None, &[], None, "light")),
            ["The Lighthouse"]
        );
    }

    /// A Document with no headings shows the dim line where they would be,
    /// and typing lists Documents beneath it.
    #[test]
    fn no_headings_is_one_dim_line_with_the_documents_beneath_it() {
        let library = library();
        assert_eq!(
            read(&outline(&[], None, &library, None, "")),
            ["(no headings)"]
        );
        let typed = outline(&[], None, &library[..2], None, "sea");
        assert_eq!(
            read(&typed),
            ["(no headings)", "[Documents]", "sea-1", "sea-2"]
        );
        assert_eq!(typed.selected, 0);
    }
}
