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
}
