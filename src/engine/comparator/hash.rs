use super::{CompareResult, FileComparator};
use anyhow::Result;
use sha2::{Digest, Sha256};
use std::{io::Read, path::Path};

#[derive(Default)]
pub struct HashComparator;

impl HashComparator {
    pub fn new() -> Self {
        Self
    }

    fn hash_file(path: &Path) -> Result<Vec<u8>> {
        let mut file = std::fs::File::open(path)?;
        let mut hasher = Sha256::new();
        let mut buf = [0u8; 65536]; // 64 KB chunks
        loop {
            let n = file.read(&mut buf)?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
        }
        Ok(hasher.finalize().to_vec())
    }
}

impl FileComparator for HashComparator {
    fn compare(&self, a: &Path, b: &Path) -> Result<CompareResult> {
        // Size check: different sizes → definitely different, skip hashing entirely.
        match (std::fs::metadata(a), std::fs::metadata(b)) {
            (Ok(ma), Ok(mb)) => {
                if ma.len() != mb.len() {
                    return Ok(CompareResult::Different);
                }
                if ma.len() == 0 {
                    return Ok(CompareResult::Identical);
                }
            }
            _ => {} // metadata unavailable — fall through to hashing
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
        "hash (SHA-256)"
    }
}
