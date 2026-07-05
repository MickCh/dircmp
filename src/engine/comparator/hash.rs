use super::{size_precheck, CompareResult, FileComparator};
use anyhow::{Context, Result};
use std::{io::Read, path::Path};

#[derive(Default)]
pub struct HashComparator;

impl HashComparator {
    pub fn new() -> Self {
        Self
    }

    fn hash_file(path: &Path) -> Result<blake3::Hash> {
        let mut file = std::fs::File::open(path)
            .with_context(|| format!("Read error {}", path.display()))?;
        let mut hasher = blake3::Hasher::new();
        let mut buf = [0u8; 131072]; // 128 KB chunks
        loop {
            let n = file
                .read(&mut buf)
                .with_context(|| format!("Read error {}", path.display()))?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
        }
        Ok(hasher.finalize())
    }
}

impl FileComparator for HashComparator {
    fn compare(&self, a: &Path, b: &Path) -> Result<CompareResult> {
        if let Some(result) = size_precheck(a, b) {
            return Ok(result);
        }

        let hash_a = Self::hash_file(a)?;
        let hash_b = Self::hash_file(b)?;

        if hash_a == hash_b {
            Ok(CompareResult::Identical)
        } else {
            Ok(CompareResult::Different)
        }
    }

    fn name(&self) -> &str {
        "hash (BLAKE3)"
    }
}
