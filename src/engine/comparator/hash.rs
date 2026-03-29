use super::{CompareResult, FileComparator};
use anyhow::Result;
use sha2::{Digest, Sha256};
use std::{fs, path::Path};

#[derive(Default)]
pub struct HashComparator;

impl HashComparator {
    pub fn new() -> Self {
        Self
    }

    fn hash_file(path: &Path) -> Result<Vec<u8>> {
        let bytes = fs::read(path)?;
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        Ok(hasher.finalize().to_vec())
    }
}

impl FileComparator for HashComparator {
    fn compare(&self, a: &Path, b: &Path) -> Result<CompareResult> {
        let hash_a = match Self::hash_file(a) {
            Ok(h) => h,
            Err(e) => return Ok(CompareResult::Error(format!("Błąd odczytu {}: {}", a.display(), e))),
        };
        let hash_b = match Self::hash_file(b) {
            Ok(h) => h,
            Err(e) => return Ok(CompareResult::Error(format!("Błąd odczytu {}: {}", b.display(), e))),
        };

        if hash_a == hash_b {
            Ok(CompareResult::Identical)
        } else {
            Ok(CompareResult::Different)
        }
    }

    fn name(&self) -> &str {
        "hash (SHA-256)"
    }
}
