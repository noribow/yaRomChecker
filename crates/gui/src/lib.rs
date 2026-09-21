//! Small, independently tested GUI presentation rules.

use std::{
    collections::BTreeMap,
    path::{Component, Path, PathBuf},
};

/// A node in the configured source tree. Folder nodes come only from path
/// components that prefix at least one configured DAT path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SourceTreeNode {
    Folder {
        path: PathBuf,
        title: String,
        children: Vec<SourceTreeNode>,
    },
    Dat {
        source_index: usize,
    },
}

#[derive(Default)]
struct SourceTreeFolder {
    path: PathBuf,
    folders: BTreeMap<String, SourceTreeFolder>,
    dats: BTreeMap<String, Vec<usize>>,
}

/// Builds the Sources pane hierarchy from resolved DAT file paths without
/// reading any directory from disk.
pub fn build_source_tree(paths: &[PathBuf]) -> Vec<SourceTreeNode> {
    let mut root = SourceTreeFolder::default();
    for (source_index, path) in paths.iter().enumerate() {
        let components = display_components(path);
        let Some((file, folders)) = components.split_last() else {
            continue;
        };
        let mut node = &mut root;
        for folder in folders {
            let child = node.folders.entry(folder.clone()).or_default();
            if child.path.as_os_str().is_empty() {
                child.path = node.path.join(folder);
            }
            node = child;
        }
        node.dats
            .entry(file.clone())
            .or_default()
            .push(source_index);
    }
    folder_children(root)
}

fn display_components(path: &Path) -> Vec<String> {
    path.components()
        .filter_map(|component| match component {
            Component::Prefix(prefix) => Some(prefix.as_os_str().to_string_lossy().into_owned()),
            Component::RootDir => None,
            Component::Normal(value) => Some(value.to_string_lossy().into_owned()),
            Component::CurDir => None,
            Component::ParentDir => Some("..".to_owned()),
        })
        .collect()
}

fn folder_children(folder: SourceTreeFolder) -> Vec<SourceTreeNode> {
    let mut named = Vec::new();
    for (title, child) in folder.folders {
        named.push((
            title.clone(),
            SourceTreeNode::Folder {
                path: child.path.clone(),
                title,
                children: folder_children(child),
            },
        ));
    }
    for (title, indexes) in folder.dats {
        for source_index in indexes {
            named.push((title.clone(), SourceTreeNode::Dat { source_index }));
        }
    }
    named.sort_by(|left, right| left.0.cmp(&right.0));
    named.into_iter().map(|(_, node)| node).collect()
}

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
    fn source_tree_supports_mixed_children_and_multiple_roots() {
        let paths = vec![
            PathBuf::from("C:/DATs/Nintendo/NES.dat"),
            PathBuf::from("C:/DATs/Arcade.xml"),
            PathBuf::from("D:/TOSEC/Amiga.dat"),
        ];

        let tree = build_source_tree(&paths);

        assert_eq!(tree.len(), 2);
        let SourceTreeNode::Folder {
            title, children, ..
        } = &tree[0]
        else {
            panic!("expected drive folder");
        };
        assert_eq!(title, "C:");
        let SourceTreeNode::Folder { children, .. } = &children[0] else {
            panic!("expected DATs folder");
        };
        assert!(matches!(
            children[0],
            SourceTreeNode::Dat { source_index: 1 }
        ));
        assert!(matches!(children[1], SourceTreeNode::Folder { .. }));
    }

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
