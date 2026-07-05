pub mod byte;
pub mod hash;
pub mod metadata;
pub mod text;

use crate::config::{ComparisonConfig, ComparisonStrategy};
use anyhow::Result;
use std::path::Path;

/// Result of comparing two files. Failures (unreadable file, metadata error)
/// are reported through the `anyhow::Result` returned by [`FileComparator::compare`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompareResult {
    /// Files are identical.
    Identical,
    /// Files differ.
    Different,
}

/// Trait defining the file comparator interface.
/// Each comparison strategy implements this trait.
pub trait FileComparator: Send + Sync {
    /// Compares two files and returns the result.
    fn compare(&self, a: &Path, b: &Path) -> Result<CompareResult>;

    /// Strategy name (displayed in the UI).
    fn name(&self) -> &str;
}

/// Size-based shortcut shared by content comparators: different sizes → `Different`,
/// both empty → `Identical`. Returns `None` when contents must actually be read
/// (equal non-zero sizes, or metadata unavailable).
pub(crate) fn size_precheck(a: &Path, b: &Path) -> Option<CompareResult> {
    let ma = std::fs::metadata(a).ok()?;
    let mb = std::fs::metadata(b).ok()?;
    if ma.len() != mb.len() {
        Some(CompareResult::Different)
    } else if ma.len() == 0 {
        Some(CompareResult::Identical)
    } else {
        None
    }
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
