//! The table behind a drop-down: a value, and the words a writer reads for it.
//!
//! A [`gtk::DropDown`] answers a row by its position and not by a name, so
//! every setting offered as a list needs a table pairing what the file holds
//! with what the row says, and the walk between the two — the value's row, and
//! the row's value — is the same walk whatever the setting is. It is here once
//! rather than beside each table: the Export dialog's paper list
//! ([`crate::export_dialog`]) and the Settings window's Mode row
//! ([`crate::settings`]) are two tables through one pair of functions.
//!
//! A table is `&[(T, &str)]` in the order the rows stand in, and the first row
//! is what a value or an index the table does not carry falls back to, so a
//! drop-down never answers with something the file cannot hold.

/// A drop-down over `offered`, standing on `value` before any handler is
/// connected, so that building the row is not a write.
pub(crate) fn drop_down<T: Copy + PartialEq>(offered: &[(T, &str)], value: T) -> gtk::DropDown {
    let words: Vec<&str> = offered.iter().map(|(_, words)| *words).collect();
    let drop_down = gtk::DropDown::from_strings(&words);
    drop_down.set_selected(index_of(offered, value));
    drop_down
}

/// The value `offered`'s row `index` names, and the default for a row the table
/// does not reach — which is what `GTK_INVALID_LIST_POSITION` is.
pub(crate) fn at<T: Copy + Default>(offered: &[(T, &str)], index: u32) -> T {
    usize::try_from(index)
        .ok()
        .and_then(|index| offered.get(index))
        .map_or_else(T::default, |(value, _)| *value)
}

/// Which row of `offered` `value` stands on, and the first row for a value the
/// table does not carry.
fn index_of<T: Copy + PartialEq>(offered: &[(T, &str)], value: T) -> u32 {
    let found = offered.iter().position(|(offered, _)| *offered == value);
    u32::try_from(found.unwrap_or_default()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The rows a table offers, standing for values that are not their own
    /// positions, so that a walk that answered with the index would be caught.
    const OFFERED: [(u8, &str); 3] = [(7, "seven"), (8, "eight"), (9, "nine")];

    /// A value is the row it stands on and back again, and neither a value nor
    /// a row the table does not carry answers with anything else.
    #[test]
    fn a_value_is_the_row_it_stands_on_and_what_the_table_lacks_falls_to_its_first() {
        for (value, _) in OFFERED {
            assert_eq!(at(&OFFERED, index_of(&OFFERED, value)), value, "{value}");
        }
        assert_eq!(
            index_of(&OFFERED, 42),
            0,
            "a value the table does not carry stands on its first row"
        );
        assert_eq!(
            at(&OFFERED, u32::MAX),
            u8::default(),
            "and a row past the end of the table is the value's own default"
        );
    }
}
