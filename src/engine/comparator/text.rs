use super::{CompareResult, FileComparator};
use anyhow::Result;
use std::{
    fs::File,
    io::{BufRead, BufReader},
    path::Path,
};

pub struct TextComparator {
    ignore_whitespace: bool,
    ignore_case: bool,
}

impl TextComparator {
    pub fn new(ignore_whitespace: bool, ignore_case: bool) -> Self {
        Self {
            ignore_whitespace,
            ignore_case,
        }
    }

    fn normalize_line(&self, line: &str) -> String {
        let s = if self.ignore_case {
            line.to_lowercase()
        } else {
            line.to_string()
        };

        if self.ignore_whitespace {
            s.split_whitespace().collect::<Vec<_>>().join(" ")
        } else {
            s
        }
    }
}

impl FileComparator for TextComparator {
    fn compare(&self, a: &Path, b: &Path) -> Result<CompareResult> {
        let file_a = match File::open(a) {
            Ok(f) => f,
            Err(e) => return Ok(CompareResult::Error(format!("Read error {}: {}", a.display(), e))),
        };
        let file_b = match File::open(b) {
            Ok(f) => f,
            Err(e) => return Ok(CompareResult::Error(format!("Read error {}: {}", b.display(), e))),
        };

        let mut lines_a = BufReader::new(file_a).lines();
        let mut lines_b = BufReader::new(file_b).lines();

        loop {
            match (lines_a.next(), lines_b.next()) {
                (None, None) => return Ok(CompareResult::Identical),
                (Some(Err(e)), _) => {
                    return Ok(CompareResult::Error(format!("Read error {}: {}", a.display(), e)))
                }
                (_, Some(Err(e))) => {
                    return Ok(CompareResult::Error(format!("Read error {}: {}", b.display(), e)))
                }
                (Some(Ok(la)), Some(Ok(lb))) => {
                    if self.normalize_line(&la) != self.normalize_line(&lb) {
                        return Ok(CompareResult::Different);
                    }
                }
                _ => return Ok(CompareResult::Different),
            }
        }
    }

    fn name(&self) -> &str {
        match (self.ignore_whitespace, self.ignore_case) {
            (true, true) => "text (ignore whitespace, ignore case)",
            (true, false) => "text (ignore whitespace)",
            (false, true) => "text (ignore case)",
            (false, false) => "text",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_whitespace() {
        let cmp = TextComparator::new(true, false);
        let a = cmp.normalize_line("hello   world  ");
        let b = cmp.normalize_line("hello world");
        assert_eq!(a, b);
    }

    #[test]
    fn normalize_case() {
        let cmp = TextComparator::new(false, true);
        let a = cmp.normalize_line("Hello World");
        let b = cmp.normalize_line("hello world");
        assert_eq!(a, b);
    }
}
