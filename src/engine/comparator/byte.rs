use super::{CompareResult, FileComparator};
use anyhow::Result;
use std::{fs::File, io::Read, path::Path};

const BUF_SIZE: usize = 65536; // 64 KB

#[derive(Default)]
pub struct ByteComparator;

impl ByteComparator {
    pub fn new() -> Self {
        Self
    }

    /// Fills `buf` as much as possible, returning bytes read (0 = EOF).
    fn read_chunk(reader: &mut impl Read, buf: &mut [u8]) -> std::io::Result<usize> {
        let mut total = 0;
        while total < buf.len() {
            match reader.read(&mut buf[total..])? {
                0 => break,
                n => total += n,
            }
        }
        Ok(total)
    }
}

impl FileComparator for ByteComparator {
    fn compare(&self, a: &Path, b: &Path) -> Result<CompareResult> {
        // Quick size check — avoids opening files when sizes differ.
        if let (Ok(ma), Ok(mb)) = (std::fs::metadata(a), std::fs::metadata(b)) {
            if ma.len() != mb.len() {
                return Ok(CompareResult::Different);
            }
            if ma.len() == 0 {
                return Ok(CompareResult::Identical);
            }
        }

        let mut file_a = match File::open(a) {
            Ok(f) => f,
            Err(e) => return Ok(CompareResult::Error(format!("Read error {}: {}", a.display(), e))),
        };
        let mut file_b = match File::open(b) {
            Ok(f) => f,
            Err(e) => return Ok(CompareResult::Error(format!("Read error {}: {}", b.display(), e))),
        };

        let mut buf_a = [0u8; BUF_SIZE];
        let mut buf_b = [0u8; BUF_SIZE];

        loop {
            let n_a = match Self::read_chunk(&mut file_a, &mut buf_a) {
                Ok(n) => n,
                Err(e) => return Ok(CompareResult::Error(format!("Read error {}: {}", a.display(), e))),
            };
            let n_b = match Self::read_chunk(&mut file_b, &mut buf_b) {
                Ok(n) => n,
                Err(e) => return Ok(CompareResult::Error(format!("Read error {}: {}", b.display(), e))),
            };

            if n_a != n_b || buf_a[..n_a] != buf_b[..n_b] {
                return Ok(CompareResult::Different);
            }
            if n_a == 0 {
                return Ok(CompareResult::Identical);
            }
        }
    }

    fn name(&self) -> &str {
        "byte-by-byte"
    }
}
