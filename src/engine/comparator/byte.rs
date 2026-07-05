use super::{size_precheck, CompareResult, FileComparator};
use anyhow::{Context, Result};
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
        if let Some(result) = size_precheck(a, b) {
            return Ok(result);
        }

        let mut file_a = File::open(a).with_context(|| format!("Read error {}", a.display()))?;
        let mut file_b = File::open(b).with_context(|| format!("Read error {}", b.display()))?;

        let mut buf_a = [0u8; BUF_SIZE];
        let mut buf_b = [0u8; BUF_SIZE];

        loop {
            let n_a = Self::read_chunk(&mut file_a, &mut buf_a)
                .with_context(|| format!("Read error {}", a.display()))?;
            let n_b = Self::read_chunk(&mut file_b, &mut buf_b)
                .with_context(|| format!("Read error {}", b.display()))?;

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
