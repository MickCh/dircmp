use super::{CompareResult, FileComparator};
use anyhow::Result;
use std::{io::Read, path::Path};

#[derive(Default)]
pub struct HashComparator;

impl HashComparator {
    pub fn new() -> Self {
        Self
    }

    fn hash_file(path: &Path) -> Result<blake3::Hash> {
        let mut file = std::fs::File::open(path)?;
        let mut hasher = blake3::Hasher::new();
        let mut buf = [0u8; 131072]; // 128 KB chunks
        loop {
            let n = file.read(&mut buf)?;
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
        // Size check: different sizes → definitely different, skip hashing entirely.
        // metadata unavailable — fall through to hashing
        if let (Ok(ma), Ok(mb)) = (std::fs::metadata(a), std::fs::metadata(b)) {
            if ma.len() != mb.len() {
                return Ok(CompareResult::Different);
            }
            if ma.len() == 0 {
                return Ok(CompareResult::Identical);
            }
        }

        let hash_a = match Self::hash_file(a) {
            Ok(h) => h,
            Err(e) => return Ok(CompareResult::Error(format!("Read error {}: {}", a.display(), e))),
        };
        let hash_b = match Self::hash_file(b) {
            Ok(h) => h,
            Err(e) => return Ok(CompareResult::Error(format!("Read error {}: {}", b.display(), e))),
        };

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
