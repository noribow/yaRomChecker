//! Small, independently tested GUI presentation rules.

/// Whether the member table is useful for a selected set.
///
/// A single loose file is represented completely by its set row. Archives and
/// multi-file sets need the member table even when an archive currently has
/// only one discovered member.
pub fn should_show_member_pane(member_count: usize, has_archive_member: bool) -> bool {
    has_archive_member || member_count > 1
}

/// Formats a set count, leaving it blank until verification has produced a result.
pub fn set_count_text(verified: bool, count: usize) -> String {
    if verified {
        count.to_string()
    } else {
        String::new()
    }
}

/// Joins the DAT's authoritative member-name list to optional scan hits.
/// The returned index points into `hits`, while a missing index represents an
/// unscanned or unmatched DAT member whose detail cells must remain blank.
pub fn dat_member_rows(
    dat_names: &[String],
    hits: &[(String, String)],
) -> Vec<(String, Option<usize>)> {
    dat_names
        .iter()
        .map(|name| {
            let hit = hits
                .iter()
                .position(|(dat_name, _entry_path)| dat_name == name);
            (name.clone(), hit)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hides_members_for_one_loose_file() {
        assert!(!should_show_member_pane(1, false));
    }

    #[test]
    fn shows_members_for_archive_or_multi_file_set() {
        assert!(should_show_member_pane(1, true));
        assert!(should_show_member_pane(2, false));
    }

    #[test]
    fn set_counts_are_blank_before_verify_and_include_zero_after_verify() {
        assert_eq!(set_count_text(false, 0), "");
        assert_eq!(set_count_text(false, 4), "");
        assert_eq!(set_count_text(true, 0), "0");
        assert_eq!(set_count_text(true, 4), "4");
    }

    #[test]
    fn member_rows_include_dat_names_without_scan_hits() {
        let names = vec!["track-1.bin".to_owned(), "track-2.bin".to_owned()];
        let hits = vec![("track-1.bin".to_owned(), "disc.zip/track-1.bin".to_owned())];

        assert_eq!(
            dat_member_rows(&names, &hits),
            vec![
                ("track-1.bin".to_owned(), Some(0)),
                ("track-2.bin".to_owned(), None)
            ]
        );
    }
}
