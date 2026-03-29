use super::{CompareResult, FileComparator};
use anyhow::Result;
use std::path::Path;

#[derive(Default)]
pub struct MetadataComparator;

impl MetadataComparator {
    pub fn new() -> Self { Self }
}

impl FileComparator for MetadataComparator {
    fn compare(&self, a: &Path, b: &Path) -> Result<CompareResult> {
        let meta_a = match std::fs::metadata(a) {
            Ok(m) => m,
            Err(e) => return Ok(CompareResult::Error(format!("Metadata error {}: {}", a.display(), e))),
        };
        let meta_b = match std::fs::metadata(b) {
            Ok(m) => m,
            Err(e) => return Ok(CompareResult::Error(format!("Metadata error {}: {}", b.display(), e))),
        };

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
