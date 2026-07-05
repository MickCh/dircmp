use super::{CompareResult, FileComparator};
use anyhow::{Context, Result};
use std::path::Path;

#[derive(Default)]
pub struct MetadataComparator;

impl MetadataComparator {
    pub fn new() -> Self { Self }
}

impl FileComparator for MetadataComparator {
    fn compare(&self, a: &Path, b: &Path) -> Result<CompareResult> {
        let meta_a =
            std::fs::metadata(a).with_context(|| format!("Metadata error {}", a.display()))?;
        let meta_b =
            std::fs::metadata(b).with_context(|| format!("Metadata error {}", b.display()))?;

        let size_eq = meta_a.len() == meta_b.len();
        let mtime_eq = meta_a.modified().ok() == meta_b.modified().ok();

        if size_eq && mtime_eq {
            Ok(CompareResult::Identical)
        } else {
            Ok(CompareResult::Different)
        }
    }

    fn name(&self) -> &str {
        "metadata (size + mtime)"
    }
}
