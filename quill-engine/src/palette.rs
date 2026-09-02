//! The Palette's list: the registry in the oracle's four sections and More,
//! and the match that narrows it as the writer types.
//!
//! With nothing typed the Palette is a map of the app in the order a writer
//! meets it (`legacy/app/js/chrome.js`, `SECTIONS`); typing turns it into one
//! ranked list. Both are pure functions of the registry, so the popover only
//! draws what it is handed.

use crate::commands::{COMMANDS, Command};

/// The oracle's four sections, by the registry's ids in the oracle's order
/// and membership (`chrome.js:364-368`; the oracle's `file.export` is
/// `export.open` here).
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
            "export.open",
            "file.next",
            "file.prev",
            "file.follow",
            "file.delete",
        ],
    ),
];

/// The fifth section: every registry Command in none of the four, in the
/// table's order. The oracle sorts these by title; the table's order is the
/// one the menus already read in.
pub const MORE: &str = "More";

/// How a title matched the query, best first (`chrome.js` ranks a prefix
/// over a substring over a subsequence).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Tier {
    /// The title starts with the query.
    Prefix,
    /// The title contains the query further in.
    Inside,
    /// The title has the query's letters in order, with gaps.
    Letters,
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
/// ordered by [`Rank`] and then by title.
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
    let mut ranked: Vec<(Rank, Row)> = COMMANDS
        .iter()
        .filter_map(|command| {
            score(command.title, &query).map(|(rank, hits)| (rank, Row { command, hits }))
        })
        .collect();
    ranked.sort_by(|a, b| {
        a.0.cmp(&b.0)
            .then_with(|| a.1.command.title.cmp(b.1.command.title))
    });
    vec![(None, ranked.into_iter().map(|(_, row)| row).collect())]
}

/// The oracle's match (`chrome.js:344-352`): `query`, already lower-cased,
/// against `title`. A substring ranks first, at its offset; failing that the
/// query's letters in order anywhere in the title, "sen" finding "Sentence"
/// and "tw" "Typewriter"; failing that, no row. The hits are byte ranges of
/// `title`.
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
}
