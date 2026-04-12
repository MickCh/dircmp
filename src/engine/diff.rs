use crate::engine::{
    comparator::{CompareResult, FileComparator},
    scanner::EntryMap,
};
use std::path::PathBuf;

/// Comparison status of a single entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffStatus {
    /// File is awaiting content comparison (phase 2).
    Pending,
    /// File/directory exists only in the left folder.
    LeftOnly,
    /// File/directory exists only in the right folder.
    RightOnly,
    /// Files are identical.
    Identical,
    /// Directory exists on both sides.
    DirectoryPresent,
    /// Files differ in content.
    Different,
    /// One side is a file, the other is a directory with the same name.
    TypeConflict,
    /// Error occurred during comparison.
    Error(String),
}

#[allow(dead_code)]
impl DiffStatus {
    pub fn is_same(&self) -> bool {
        matches!(self, DiffStatus::Identical | DiffStatus::DirectoryPresent)
    }

    pub fn has_difference(&self) -> bool {
        !self.is_same()
    }
}

/// Comparison result for a single entry.
#[derive(Debug, Clone)]
pub struct DiffEntry {
    pub relative_path: PathBuf,
    pub status: DiffStatus,
    pub is_dir: bool,
}

/// Full comparison result for two folder trees.
#[derive(Debug, Default)]
pub struct DiffResult {
    pub entries: Vec<DiffEntry>,
}

impl DiffResult {
    pub fn left_only(&self) -> impl Iterator<Item = &DiffEntry> {
        self.entries.iter().filter(|e| e.status == DiffStatus::LeftOnly)
    }

    pub fn right_only(&self) -> impl Iterator<Item = &DiffEntry> {
        self.entries.iter().filter(|e| e.status == DiffStatus::RightOnly)
    }

    pub fn different(&self) -> impl Iterator<Item = &DiffEntry> {
        self.entries.iter().filter(|e| e.status == DiffStatus::Different)
    }

    pub fn identical(&self) -> impl Iterator<Item = &DiffEntry> {
        self.entries.iter().filter(|e| e.status == DiffStatus::Identical)
    }

    pub fn total(&self) -> usize {
        self.entries
            .iter()
            .filter(|e| !matches!(e.status, DiffStatus::DirectoryPresent))
            .count()
    }
}

/// Full diff engine — used in integration tests and future use cases.
pub struct DiffEngine<'a> {
    comparator: &'a dyn FileComparator,
}

#[allow(dead_code)]
impl<'a> DiffEngine<'a> {
    pub fn new(comparator: &'a dyn FileComparator) -> Self {
        Self { comparator }
    }

    /// Phase 1: builds the diff structure without reading file contents.
    /// Files present on both sides are assigned status `Pending`.
    pub fn diff_structure(left: &EntryMap, right: &EntryMap) -> DiffResult {
        let mut entries = Vec::new();

        let mut all_paths: Vec<PathBuf> = left.keys().cloned().collect();
        for path in right.keys() {
            if !left.contains_key(path) {
                all_paths.push(path.clone());
            }
        }
        all_paths.sort();

        for path in all_paths {
            let diff_entry = match (left.get(&path), right.get(&path)) {
                (Some(l), None) => DiffEntry {
                    relative_path: path,
                    status: DiffStatus::LeftOnly,
                    is_dir: l.is_dir,
                },
                (None, Some(r)) => DiffEntry {
                    relative_path: path,
                    status: DiffStatus::RightOnly,
                    is_dir: r.is_dir,
                },
                (Some(l), Some(r)) => {
                    let status = if l.is_dir != r.is_dir {
                        DiffStatus::TypeConflict
                    } else if l.is_dir {
                        DiffStatus::DirectoryPresent
                    } else {
                        DiffStatus::Pending
                    };
                    DiffEntry {
                        relative_path: path,
                        status,
                        is_dir: l.is_dir,
                    }
                }
                (None, None) => unreachable!(),
            };
            entries.push(diff_entry);
        }

        DiffResult { entries }
    }

    /// Full comparison (used in tests).
    pub fn diff(
        &self,
        left: &EntryMap,
        right: &EntryMap,
        left_root: &std::path::Path,
        right_root: &std::path::Path,
    ) -> DiffResult {
        let mut result = Self::diff_structure(left, right);
        for entry in &mut result.entries {
            if entry.status == DiffStatus::Pending {
                let abs_left = left_root.join(&entry.relative_path);
                let abs_right = right_root.join(&entry.relative_path);
                entry.status = match self.comparator.compare(&abs_left, &abs_right) {
                    Ok(CompareResult::Identical) => DiffStatus::Identical,
                    Ok(CompareResult::Different) => DiffStatus::Different,
                    Ok(CompareResult::Error(e)) => DiffStatus::Error(e),
                    Err(e) => DiffStatus::Error(e.to_string()),
                };
            }
        }
        result
    }
}
