use super::{CompareResult, FileComparator};
use anyhow::Result;
use std::{fs, path::Path};

#[derive(Default)]
pub struct ByteComparator;

impl ByteComparator {
    pub fn new() -> Self { Self }
}

impl FileComparator for ByteComparator {
    fn compare(&self, a: &Path, b: &Path) -> Result<CompareResult> {
        let bytes_a = match fs::read(a) {
            Ok(b) => b,
            Err(e) => return Ok(CompareResult::Error(format!("Błąd odczytu {}: {}", a.display(), e))),
        };
        let bytes_b = match fs::read(b) {
            Ok(b) => b,
            Err(e) => return Ok(CompareResult::Error(format!("Błąd odczytu {}: {}", b.display(), e))),
        };

        if bytes_a == bytes_b {
            Ok(CompareResult::Identical)
        } else {
            Ok(CompareResult::Different)
        }
    }

    fn name(&self) -> &str {
        "bajt po bajcie"
    }
}
