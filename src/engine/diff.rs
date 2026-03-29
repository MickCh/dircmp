use crate::engine::{
    comparator::{CompareResult, FileComparator},
    scanner::EntryMap,
};
use std::path::PathBuf;

/// Status pojedynczego wpisu po porównaniu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffStatus {
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
    /// Rozmiar po lewej (None jeśli nie istnieje).
    pub size_left: Option<u64>,
    /// Rozmiar po prawej (None jeśli nie istnieje).
    pub size_right: Option<u64>,
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

    pub fn differences_count(&self) -> usize {
        self.entries.iter().filter(|e| e.status.has_difference()).count()
    }
}

/// Silnik diffowania – łączy wyniki skanera i komparatora.
pub struct DiffEngine<'a> {
    comparator: &'a dyn FileComparator,
}

impl<'a> DiffEngine<'a> {
    pub fn new(comparator: &'a dyn FileComparator) -> Self {
        Self { comparator }
    }

    /// Porównuje dwie mapy wpisów i zwraca wynik.
    pub fn diff(
        &self,
        left: &EntryMap,
        right: &EntryMap,
        left_root: &std::path::Path,
        right_root: &std::path::Path,
    ) -> DiffResult {
        let mut entries = Vec::new();

        // Zbierz wszystkie unikalne ścieżki
        let mut all_paths: Vec<PathBuf> = left.keys().cloned().collect();
        for path in right.keys() {
            if !left.contains_key(path) {
                all_paths.push(path.clone());
            }
        }
        all_paths.sort();

        for path in all_paths {
            let left_entry = left.get(&path);
            let right_entry = right.get(&path);

            let diff_entry = match (left_entry, right_entry) {
                (Some(l), None) => DiffEntry {
                    relative_path: path,
                    status: DiffStatus::LeftOnly,
                    size_left: Some(l.size),
                    size_right: None,
                    is_dir: l.is_dir,
                },
                (None, Some(r)) => DiffEntry {
                    relative_path: path,
                    status: DiffStatus::RightOnly,
                    size_left: None,
                    size_right: Some(r.size),
                    is_dir: r.is_dir,
                },
                (Some(l), Some(r)) => {
                    let status = if l.is_dir != r.is_dir {
                        DiffStatus::TypeConflict
                    } else if l.is_dir {
                        // Katalogi – istnieją po obu stronach, nie porównujemy zawartości
                        DiffStatus::DirectoryPresent
                    } else {
                        let abs_left = left_root.join(&path);
                        let abs_right = right_root.join(&path);
                        match self.comparator.compare(&abs_left, &abs_right) {
                            Ok(CompareResult::Identical) => DiffStatus::Identical,
                            Ok(CompareResult::Different) => DiffStatus::Different,
                            Ok(CompareResult::Error(e)) => DiffStatus::Error(e),
                            Err(e) => DiffStatus::Error(e.to_string()),
                        }
                    };
                    DiffEntry {
                        relative_path: path,
                        status,
                        size_left: Some(l.size),
                        size_right: Some(r.size),
                        is_dir: l.is_dir,
                    }
                }
                (None, None) => unreachable!(),
            };

            entries.push(diff_entry);
        }

        DiffResult { entries }
    }
}
