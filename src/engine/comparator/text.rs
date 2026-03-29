use super::{CompareResult, FileComparator};
use anyhow::Result;
use std::{fs, path::Path};

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

    fn normalize(&self, content: &str) -> String {
        let mut result = if self.ignore_case {
            content.to_lowercase()
        } else {
            content.to_string()
        };

        if self.ignore_whitespace {
            // Normalizuj białe znaki: usuń wiodące/końcowe, zastąp wielokrotne spacje jedną
            result = result
                .lines()
                .map(|line| {
                    line.split_whitespace()
                        .collect::<Vec<_>>()
                        .join(" ")
                })
                .collect::<Vec<_>>()
                .join("\n");
        }

        result
    }
}

impl FileComparator for TextComparator {
    fn compare(&self, a: &Path, b: &Path) -> Result<CompareResult> {
        let content_a = match fs::read_to_string(a) {
            Ok(c) => c,
            Err(e) => return Ok(CompareResult::Error(format!("Read error {}: {}", a.display(), e))),
        };
        let content_b = match fs::read_to_string(b) {
            Ok(c) => c,
            Err(e) => return Ok(CompareResult::Error(format!("Read error {}: {}", b.display(), e))),
        };

        let norm_a = self.normalize(&content_a);
        let norm_b = self.normalize(&content_b);

        if norm_a == norm_b {
            Ok(CompareResult::Identical)
        } else {
            Ok(CompareResult::Different)
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
        let a = cmp.normalize("hello   world\n  foo  ");
        let b = cmp.normalize("hello world\nfoo");
        assert_eq!(a, b);
    }

    #[test]
    fn normalize_case() {
        let cmp = TextComparator::new(false, true);
        let a = cmp.normalize("Hello World");
        let b = cmp.normalize("hello world");
        assert_eq!(a, b);
    }
}
