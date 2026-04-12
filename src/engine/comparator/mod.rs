pub mod byte;
pub mod hash;
pub mod metadata;
pub mod text;

use crate::config::{ComparisonConfig, ComparisonStrategy};
use anyhow::Result;
use std::path::Path;

/// Result of comparing two files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompareResult {
    /// Files are identical.
    Identical,
    /// Files differ.
    Different,
    /// Cannot compare (e.g. read error, type mismatch).
    Error(String),
}

/// Trait defining the file comparator interface.
/// Each comparison strategy implements this trait.
pub trait FileComparator: Send + Sync {
    /// Compares two files and returns the result.
    fn compare(&self, a: &Path, b: &Path) -> Result<CompareResult>;

    /// Strategy name (displayed in the UI).
    fn name(&self) -> &str;
}

/// Creates a comparator based on the configuration.
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
