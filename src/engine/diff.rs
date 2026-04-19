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

impl DiffStatus {
    // Used in integration tests (tests/engine_tests.rs); not called from binary code.
    #[allow(dead_code)]
    pub fn is_same(&self) -> bool {
        matches!(self, DiffStatus::Identical | DiffStatus::DirectoryPresent)
    }

    #[allow(dead_code)]
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
    /// Number of non-directory entries; stable after diff_structure (DirectoryPresent never changes).
    file_count: usize,
    /// Cached per-status counts updated incrementally by update_entry_status.
    count_left_only: usize,
    count_right_only: usize,
    count_different: usize,
    count_identical: usize,
}

impl DiffResult {
    /// Updates a single entry's status and adjusts the cached counts accordingly.
    pub fn update_entry_status(&mut self, idx: usize, new_status: DiffStatus) {
        match &self.entries[idx].status {
            DiffStatus::LeftOnly => self.count_left_only -= 1,
            DiffStatus::RightOnly => self.count_right_only -= 1,
            DiffStatus::Different => self.count_different -= 1,
            DiffStatus::Identical => self.count_identical -= 1,
            _ => {}
        }
        match &new_status {
            DiffStatus::LeftOnly => self.count_left_only += 1,
            DiffStatus::RightOnly => self.count_right_only += 1,
            DiffStatus::Different => self.count_different += 1,
            DiffStatus::Identical => self.count_identical += 1,
            _ => {}
        }
        self.entries[idx].status = new_status;
    }

    pub fn count_left_only(&self) -> usize { self.count_left_only }
    pub fn count_right_only(&self) -> usize { self.count_right_only }
    pub fn count_different(&self) -> usize { self.count_different }
    pub fn count_identical(&self) -> usize { self.count_identical }

    pub fn total(&self) -> usize {
        self.file_count
    }

    // Iterator accessors kept for use in tests.
    #[allow(dead_code)]
    pub fn left_only(&self) -> impl Iterator<Item = &DiffEntry> {
        self.entries.iter().filter(|e| e.status == DiffStatus::LeftOnly)
    }

    #[allow(dead_code)]
    pub fn right_only(&self) -> impl Iterator<Item = &DiffEntry> {
        self.entries.iter().filter(|e| e.status == DiffStatus::RightOnly)
    }

    #[allow(dead_code)]
    pub fn different(&self) -> impl Iterator<Item = &DiffEntry> {
        self.entries.iter().filter(|e| e.status == DiffStatus::Different)
    }

    #[allow(dead_code)]
    pub fn identical(&self) -> impl Iterator<Item = &DiffEntry> {
        self.entries.iter().filter(|e| e.status == DiffStatus::Identical)
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

        let mut file_count = 0;
        let mut count_left_only = 0;
        let mut count_right_only = 0;
        let mut count_different = 0;
        let mut count_identical = 0;
        for e in &entries {
            match &e.status {
                DiffStatus::DirectoryPresent => {}
                DiffStatus::LeftOnly => { file_count += 1; count_left_only += 1; }
                DiffStatus::RightOnly => { file_count += 1; count_right_only += 1; }
                DiffStatus::Different => { file_count += 1; count_different += 1; }
                DiffStatus::Identical => { file_count += 1; count_identical += 1; }
                _ => { file_count += 1; }
            }
        }
        DiffResult { entries, file_count, count_left_only, count_right_only, count_different, count_identical }
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
        let pending_indices: Vec<usize> = result
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| e.status == DiffStatus::Pending)
            .map(|(i, _)| i)
            .collect();
        for idx in pending_indices {
            let abs_left = left_root.join(&result.entries[idx].relative_path);
            let abs_right = right_root.join(&result.entries[idx].relative_path);
            let new_status = match self.comparator.compare(&abs_left, &abs_right) {
                Ok(CompareResult::Identical) => DiffStatus::Identical,
                Ok(CompareResult::Different) => DiffStatus::Different,
                Ok(CompareResult::Error(e)) => DiffStatus::Error(e),
                Err(e) => DiffStatus::Error(e.to_string()),
            };
            result.update_entry_status(idx, new_status);
        }
        result
    }
}
