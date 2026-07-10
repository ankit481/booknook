//! Filesystem browsing for the sidebar.
//!
//! This module has no idea that an App or a terminal exists. It only knows
//! how to turn a directory path into a sorted list of entries, and how to
//! tell a markdown file from anything else.

use std::fs;
use std::path::{Path, PathBuf};

/// One entry in a directory listing.
pub(crate) struct Entry {
    pub(crate) path: PathBuf,
    pub(crate) name: String,
    pub(crate) is_dir: bool,
}

/// List a directory's contents, folders first, with both groups sorted
/// alphabetically. This is the same convention NeoTree and most file
/// explorers use. Entries that cannot be read, because of permission
/// errors or races with the filesystem, are silently skipped rather than
/// failing the whole listing.
pub(crate) fn list_dir(dir: &Path) -> Vec<Entry> {
    let Ok(read_dir) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut entries: Vec<Entry> = read_dir
        .filter_map(Result::ok)
        .map(|e| {
            let path = e.path();
            Entry {
                is_dir: path.is_dir(),
                name: e.file_name().to_string_lossy().into_owned(),
                path,
            }
        })
        .collect();
    entries.sort_by_key(|e| (!e.is_dir, e.name.to_lowercase()));
    entries
}

pub(crate) fn is_markdown(path: &Path) -> bool {
    matches!(path.extension().and_then(|ext| ext.to_str()), Some("md") | Some("markdown"))
}
