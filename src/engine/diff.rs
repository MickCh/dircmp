use crate::engine::{
    comparator::{CompareResult, FileComparator},
    scanner::EntryMap,
};
use std::path::PathBuf;

/// Status pojedynczego wpisu po porównaniu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffStatus {
    /// Plik oczekuje na porównanie zawartości (faza 2).
    Pending,
    /// Plik/folder istnieje tylko w lewym folderze.
    LeftOnly,
    /// Plik/folder istnieje tylko w prawym folderze.
    RightOnly,
    /// Pliki są identyczne.
    Identical,
    /// Katalog istnieje po obu stronach.
    DirectoryPresent,
    /// Pliki różnią się zawartością.
    Different,
    /// Jeden jest plikiem, drugi katalogiem o tej samej nazwie.
    TypeConflict,
    /// Błąd podczas porównywania.
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

/// Wynik porównania pojedynczego wpisu.
#[derive(Debug, Clone)]
pub struct DiffEntry {
    pub relative_path: PathBuf,
    pub status: DiffStatus,
    pub is_dir: bool,
}

/// Pełny wynik porównania dwóch folderów.
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
        self.entries.len()
    }
}

/// Pełny komparator – używany w testach integracyjnych i przyszłych zastosowaniach.
pub struct DiffEngine<'a> {
    comparator: &'a dyn FileComparator,
}

#[allow(dead_code)]
impl<'a> DiffEngine<'a> {
    pub fn new(comparator: &'a dyn FileComparator) -> Self {
        Self { comparator }
    }

    /// Faza 1: buduje strukturę diff bez odczytywania plików.
    /// Pliki istniejące po obu stronach otrzymują status `Pending`.
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

    /// Pełne porównanie (używane w testach).
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
