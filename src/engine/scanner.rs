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

    /// Recursively scans a folder and returns a map of entries plus a count of
    /// skipped entries (permission errors, broken symlinks, etc.).
    pub fn scan(&self, root: &Path) -> (EntryMap, usize) {
        let mut map = EntryMap::new();
        let mut errors: usize = 0;

        // depth 0 exempts the root itself from ignore patterns — filtering
        // applies to entries *inside* the compared trees, not to the user's
        // choice of root (comparing two `node_modules` folders directly must
        // not yield an empty scan).
        let walker = WalkDir::new(root)
            .follow_links(self.config.follow_symlinks)
            .into_iter()
            .filter_entry(|e| {
                e.depth() == 0 || !self.is_ignored(e.file_name().to_string_lossy().as_ref())
            });

        for entry in walker {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => { errors += 1; continue; }
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
                Err(_) => { errors += 1; continue; }
            };

            let is_dir = metadata.is_dir();

            map.insert(relative_path, Entry { is_dir });
        }

        (map, errors)
    }

    // Matches only the last path segment (filename or directory name), not a full
    // or relative path. Patterns like "target/debug" will never match — use "debug".
    fn is_ignored(&self, name: &str) -> bool {
        self.config
            .ignore_patterns
            .iter()
            .any(|pattern| name == pattern.as_str())
    }
}


