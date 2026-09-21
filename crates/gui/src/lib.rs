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

/// Formats a source title with its DAT verification progress.
///
/// A loaded DAT has a known total and starts at zero found ROMs until it has
/// been verified. A DAT that failed to load has no known total, so its title is
/// returned without an invented count.
pub fn source_title(name: &str, total: Option<usize>, found: Option<usize>) -> String {
    match total {
        Some(total) => format!("{name} ({}/{total})", found.unwrap_or(0)),
        None => name.to_owned(),
    }
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
    fn source_title_starts_at_zero_until_verified() {
        assert_eq!(
            source_title("Example DAT", Some(340), None),
            "Example DAT (0/340)"
        );
    }

    #[test]
    fn source_title_shows_found_count() {
        assert_eq!(
            source_title("Example DAT", Some(340), Some(12)),
            "Example DAT (12/340)"
        );
    }

    #[test]
    fn source_title_omits_counts_when_dat_did_not_load() {
        assert_eq!(source_title("missing.dat", None, None), "missing.dat");
    }
}
