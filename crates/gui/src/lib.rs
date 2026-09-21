//! Small, independently tested GUI presentation rules.

use std::{
    collections::{BTreeMap, HashMap},
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

/// Builds the Sources pane hierarchy from resolved DAT file paths and resolved
/// configured roots without reading any directory from disk.
pub fn build_source_tree(
    paths: &[PathBuf],
    roots: &[PathBuf],
    outside_title: &str,
) -> Vec<SourceTreeNode> {
    let mut root_folders = roots
        .iter()
        .cloned()
        .map(|path| SourceTreeFolder {
            path,
            ..SourceTreeFolder::default()
        })
        .collect::<Vec<_>>();
    let mut outside = SourceTreeFolder::default();

    for (source_index, path) in paths.iter().enumerate() {
        let matching_root = roots
            .iter()
            .enumerate()
            .filter(|(_, root)| path.starts_with(root))
            .max_by_key(|(_, root)| root.components().count());
        match matching_root {
            Some((root_index, root)) => {
                if let Ok(relative) = path.strip_prefix(root) {
                    insert_path(&mut root_folders[root_index], relative, source_index);
                }
            }
            None => insert_components(
                &mut outside,
                display_components_without_root(path),
                source_index,
            ),
        }
    }

    let root_names = roots.iter().map(|root| root_name(root)).fold(
        HashMap::<String, usize>::new(),
        |mut counts, name| {
            *counts.entry(name).or_default() += 1;
            counts
        },
    );
    let mut tree = Vec::new();
    for (root, folder) in roots.iter().zip(root_folders) {
        if folder.folders.is_empty() && folder.dats.is_empty() {
            continue;
        }
        let name = root_name(root);
        let title = if root_names.get(&name).copied().unwrap_or_default() > 1 {
            root.display().to_string()
        } else {
            name
        };
        tree.push(SourceTreeNode::Folder {
            path: root.clone(),
            title,
            children: folder_children(folder),
        });
    }
    if !outside.folders.is_empty() || !outside.dats.is_empty() {
        tree.push(SourceTreeNode::Folder {
            path: PathBuf::from("__outside_dat_roots__"),
            title: outside_title.to_owned(),
            children: folder_children(outside),
        });
    }
    tree
}

fn insert_path(folder: &mut SourceTreeFolder, path: &Path, source_index: usize) {
    insert_components(folder, display_components(path), source_index);
}

fn insert_components(folder: &mut SourceTreeFolder, components: Vec<String>, source_index: usize) {
    let Some((file, folders)) = components.split_last() else {
        return;
    };
    let mut node = folder;
    for name in folders {
        let child = node.folders.entry(name.clone()).or_default();
        if child.path.as_os_str().is_empty() {
            child.path = node.path.join(name);
        }
        node = child;
    }
    node.dats
        .entry(file.clone())
        .or_default()
        .push(source_index);
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

fn display_components_without_root(path: &Path) -> Vec<String> {
    path.components()
        .filter_map(|component| match component {
            Component::Prefix(_) | Component::RootDir | Component::CurDir => None,
            Component::Normal(value) => Some(value.to_string_lossy().into_owned()),
            Component::ParentDir => Some("..".to_owned()),
        })
        .collect()
}

fn root_name(root: &Path) -> String {
    root.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| root.display().to_string())
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
    fn source_tree_uses_two_configured_roots_without_drive_nodes() {
        let paths = vec![
            PathBuf::from("C:/DATs/Nintendo/NES.dat"),
            PathBuf::from("C:/DATs/Arcade.xml"),
            PathBuf::from("D:/TOSEC/Amiga.dat"),
        ];
        let roots = vec![PathBuf::from("C:/DATs"), PathBuf::from("D:/TOSEC")];

        let tree = build_source_tree(&paths, &roots, "Outside DAT roots");

        assert_eq!(tree.len(), 2);
        let SourceTreeNode::Folder {
            title, children, ..
        } = &tree[0]
        else {
            panic!("expected configured root");
        };
        assert_eq!(title, "DATs");
        assert!(matches!(
            children[0],
            SourceTreeNode::Dat { source_index: 1 }
        ));
        assert!(matches!(children[1], SourceTreeNode::Folder { .. }));
    }

    #[test]
    fn source_tree_assigns_a_dat_to_the_longest_matching_root() {
        let paths = vec![PathBuf::from("C:/DATs/Nintendo/NES.dat")];
        let roots = vec![PathBuf::from("C:/DATs"), PathBuf::from("C:/DATs/Nintendo")];

        let tree = build_source_tree(&paths, &roots, "Outside DAT roots");

        assert_eq!(tree.len(), 1);
        let SourceTreeNode::Folder {
            title, children, ..
        } = &tree[0]
        else {
            panic!("expected configured root");
        };
        assert_eq!(title, "Nintendo");
        assert!(matches!(
            children[0],
            SourceTreeNode::Dat { source_index: 0 }
        ));
    }

    #[test]
    fn source_tree_groups_unmatched_paths_without_a_drive_node() {
        let paths = vec![PathBuf::from("C:/Other/Nintendo/NES.dat")];

        let tree = build_source_tree(&paths, &[PathBuf::from("D:/DATs")], "Outside DAT roots");

        assert_eq!(tree.len(), 1);
        let SourceTreeNode::Folder {
            title, children, ..
        } = &tree[0]
        else {
            panic!("expected outside group");
        };
        assert_eq!(title, "Outside DAT roots");
        let SourceTreeNode::Folder { title, .. } = &children[0] else {
            panic!("expected first path folder");
        };
        assert_eq!(title, "Other");
    }

    #[test]
    fn source_tree_puts_all_sources_outside_when_roots_are_empty() {
        let paths = vec![PathBuf::from("C:/DATs/NES.dat")];

        let tree = build_source_tree(&paths, &[], "Outside DAT roots");

        assert_eq!(tree.len(), 1);
        assert!(matches!(
            &tree[0],
            SourceTreeNode::Folder { title, .. } if title == "Outside DAT roots"
        ));
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
