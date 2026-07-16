use super::{CompareResult, FileComparator};
use anyhow::{Context, Result};
use std::{path::Path, time::Duration};

/// Filesystems store mtimes with different granularity (FAT/exFAT: 2 s,
/// NTFS: 100 ns, ext4: 1 ns), so a file copied across filesystems can have
/// its mtime shifted by up to 2 s. Anything within that window counts as
/// equal — the same tolerance rsync and Total Commander use.
const MTIME_TOLERANCE: Duration = Duration::from_secs(2);

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

        let mtime_a = meta_a
            .modified()
            .with_context(|| format!("Metadata error {}", a.display()))?;
        let mtime_b = meta_b
            .modified()
            .with_context(|| format!("Metadata error {}", b.display()))?;

        let size_eq = meta_a.len() == meta_b.len();
        let mtime_delta = mtime_a
            .duration_since(mtime_b)
            .unwrap_or_else(|e| e.duration());
        let mtime_eq = mtime_delta <= MTIME_TOLERANCE;

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
