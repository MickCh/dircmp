use crate::config::ScanConfig;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};
use walkdir::WalkDir;

/// Reprezentacja pojedynczego wpisu w folderze (plik lub katalog).
#[derive(Debug, Clone)]
pub struct Entry {
    /// Ścieżka relatywna względem folderu bazowego.
    pub relative_path: PathBuf,
    /// Absolutna ścieżka.
    pub absolute_path: PathBuf,
    /// Czy to katalog?
    pub is_dir: bool,
    /// Rozmiar pliku w bajtach (0 dla katalogów).
    pub size: u64,
}

/// Mapa: ścieżka relatywna → Entry.
pub type EntryMap = HashMap<PathBuf, Entry>;

pub struct Scanner {
    config: ScanConfig,
}

impl Scanner {
    pub fn new(config: ScanConfig) -> Self {
        Self { config }
    }

    /// Skanuje folder rekurencyjnie i zwraca mapę wpisów.
    /// Błędy dostępu do pojedynczych plików są pomijane (graceful degradation).
    pub fn scan(&self, root: &Path) -> EntryMap {
        let mut map = EntryMap::new();

        let walker = WalkDir::new(root)
            .follow_links(self.config.follow_symlinks)
            .into_iter()
            .filter_entry(|e| !self.is_ignored(e.file_name().to_string_lossy().as_ref()));

        for entry in walker {
            // Błędy dostępu (np. symlinki, brak uprawnień) – pomijamy
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };

            let absolute_path = entry.path().to_path_buf();

            // Pomiń sam korzeń
            if absolute_path == root {
                continue;
            }

            let relative_path = absolute_path
                .strip_prefix(root)
                .unwrap_or(&absolute_path)
                .to_path_buf();

            // Metadata może się nie udać przy broken symlinks
            let metadata = match entry.metadata() {
                Ok(m) => m,
                Err(_) => continue,
            };

            let is_dir = metadata.is_dir();
            let size = if is_dir { 0 } else { metadata.len() };

            map.insert(
                relative_path.clone(),
                Entry {
                    relative_path,
                    absolute_path,
                    is_dir,
                    size,
                },
            );
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


