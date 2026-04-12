use crate::config::ScanConfig;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};
use walkdir::WalkDir;


/// A single entry in a folder (file or directory).
#[derive(Debug, Clone)]
pub struct Entry {
    pub is_dir: bool,
}

/// Map: relative path → Entry.
pub type EntryMap = HashMap<PathBuf, Entry>;

pub struct Scanner {
    config: ScanConfig,
}

impl Scanner {
    pub fn new(config: ScanConfig) -> Self {
        Self { config }
    }

    /// Recursively scans a folder and returns a map of entries.
    /// Access errors for individual files are skipped (graceful degradation).
    pub fn scan(&self, root: &Path) -> EntryMap {
        let mut map = EntryMap::new();

        let walker = WalkDir::new(root)
            .follow_links(self.config.follow_symlinks)
            .into_iter()
            .filter_entry(|e| !self.is_ignored(e.file_name().to_string_lossy().as_ref()));

        for entry in walker {
            // Access errors (e.g. broken symlinks, missing permissions) — skip
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };

            let absolute_path = entry.path().to_path_buf();

            // Skip the root itself
            if absolute_path == root {
                continue;
            }

            let relative_path = absolute_path
                .strip_prefix(root)
                .unwrap_or(&absolute_path)
                .to_path_buf();

            // Metadata can fail for broken symlinks
            let metadata = match entry.metadata() {
                Ok(m) => m,
                Err(_) => continue,
            };

            let is_dir = metadata.is_dir();

            map.insert(relative_path, Entry { is_dir });
        }

        map
    }

    fn is_ignored(&self, name: &str) -> bool {
        self.config
            .ignore_patterns
            .iter()
            .any(|pattern| name == pattern.as_str())
    }
}


