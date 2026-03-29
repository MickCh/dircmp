pub mod byte;
pub mod hash;
pub mod metadata;
pub mod text;

use crate::config::{ComparisonConfig, ComparisonStrategy};
use anyhow::Result;
use std::path::Path;

/// Wynik porównania dwóch plików.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompareResult {
    /// Pliki są identyczne.
    Identical,
    /// Pliki różnią się.
    Different,
    /// Nie można porównać (np. błąd odczytu, różne typy).
    Error(String),
}

/// Trait definiujący interfejs komparatora plików.
/// Każda strategia porównywania implementuje ten trait.
pub trait FileComparator: Send + Sync {
    /// Porównuje dwa pliki i zwraca wynik.
    fn compare(&self, a: &Path, b: &Path) -> Result<CompareResult>;

    /// Nazwa strategii (do wyświetlania w UI).
    fn name(&self) -> &str;
}

/// Tworzy komparator na podstawie konfiguracji.
pub fn create_comparator(config: &ComparisonConfig) -> Box<dyn FileComparator> {
    match config.strategy {
        ComparisonStrategy::Hash => Box::new(hash::HashComparator::new()),
        ComparisonStrategy::Metadata => Box::new(metadata::MetadataComparator::new()),
        ComparisonStrategy::Byte => Box::new(byte::ByteComparator::new()),
        ComparisonStrategy::Text => Box::new(text::TextComparator::new(
            config.text.ignore_whitespace,
            config.text.ignore_case,
        )),
    }
}
