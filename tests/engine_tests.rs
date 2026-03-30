use dircmp::engine::{
    comparator::{hash::HashComparator, CompareResult, FileComparator},
    diff::{DiffEngine, DiffStatus},
    scanner::Scanner,
};
use dircmp::config::ScanConfig;
use dircmp::app::DiffFilter;
use std::fs;
use std::path::PathBuf;
use tempfile::tempdir;

fn default_scan_config() -> ScanConfig {
    ScanConfig {
        ignore_patterns: vec![],
        follow_symlinks: false,
    }
}

#[test]
fn identical_folders() {
    let left = tempdir().unwrap();
    let right = tempdir().unwrap();

    fs::write(left.path().join("a.txt"), b"hello").unwrap();
    fs::write(right.path().join("a.txt"), b"hello").unwrap();

    let scanner = Scanner::new(default_scan_config());
    let left_map = scanner.scan(left.path());
    let right_map = scanner.scan(right.path());

    let comparator = HashComparator::new();
    let engine = DiffEngine::new(&comparator);
    let result = engine.diff(&left_map, &right_map, left.path(), right.path());

    assert_eq!(result.total(), 1);
    assert_eq!(result.identical().count(), 1);
    assert_eq!(result.entries.iter().filter(|e| e.status.has_difference()).count(), 0);
}

#[test]
fn different_file_content() {
    let left = tempdir().unwrap();
    let right = tempdir().unwrap();

    fs::write(left.path().join("a.txt"), b"hello").unwrap();
    fs::write(right.path().join("a.txt"), b"world").unwrap();

    let scanner = Scanner::new(default_scan_config());
    let left_map = scanner.scan(left.path());
    let right_map = scanner.scan(right.path());

    let comparator = HashComparator::new();
    let engine = DiffEngine::new(&comparator);
    let result = engine.diff(&left_map, &right_map, left.path(), right.path());

    assert_eq!(result.different().count(), 1);
    let entry = result.different().next().unwrap();
    assert_eq!(entry.status, DiffStatus::Different);
}

#[test]
fn left_only_file() {
    let left = tempdir().unwrap();
    let right = tempdir().unwrap();

    fs::write(left.path().join("only_left.txt"), b"data").unwrap();

    let scanner = Scanner::new(default_scan_config());
    let left_map = scanner.scan(left.path());
    let right_map = scanner.scan(right.path());

    let comparator = HashComparator::new();
    let engine = DiffEngine::new(&comparator);
    let result = engine.diff(&left_map, &right_map, left.path(), right.path());

    assert_eq!(result.left_only().count(), 1);
    assert_eq!(result.right_only().count(), 0);
}

#[test]
fn right_only_file() {
    let left = tempdir().unwrap();
    let right = tempdir().unwrap();

    fs::write(right.path().join("only_right.txt"), b"data").unwrap();

    let scanner = Scanner::new(default_scan_config());
    let left_map = scanner.scan(left.path());
    let right_map = scanner.scan(right.path());

    let comparator = HashComparator::new();
    let engine = DiffEngine::new(&comparator);
    let result = engine.diff(&left_map, &right_map, left.path(), right.path());

    assert_eq!(result.right_only().count(), 1);
    assert_eq!(result.left_only().count(), 0);
}

#[test]
fn ignore_patterns() {
    let left = tempdir().unwrap();
    let right = tempdir().unwrap();

    fs::write(left.path().join("a.txt"), b"hello").unwrap();
    fs::write(left.path().join(".git"), b"ignored").unwrap();

    let config = ScanConfig {
        ignore_patterns: vec![".git".to_string()],
        follow_symlinks: false,
    };
    let scanner = Scanner::new(config);
    let map = scanner.scan(left.path());

    assert!(!map.keys().any(|p: &PathBuf| p.to_string_lossy().contains(".git")));
    assert!(map.keys().any(|p: &PathBuf| p.to_string_lossy().contains("a.txt")));
    let _ = right;
}

#[test]
fn hash_comparator_identical() {
    let dir = tempdir().unwrap();
    let a = dir.path().join("a.txt");
    let b = dir.path().join("b.txt");
    fs::write(&a, b"same content").unwrap();
    fs::write(&b, b"same content").unwrap();

    let cmp = HashComparator::new();
    assert_eq!(cmp.compare(&a, &b).unwrap(), CompareResult::Identical);
}

#[test]
fn hash_comparator_different() {
    let dir = tempdir().unwrap();
    let a = dir.path().join("a.txt");
    let b = dir.path().join("b.txt");
    fs::write(&a, b"content A").unwrap();
    fs::write(&b, b"content B").unwrap();

    let cmp = HashComparator::new();
    assert_eq!(cmp.compare(&a, &b).unwrap(), CompareResult::Different);
}

#[test]
fn filter_differences_only() {
    let left = tempdir().unwrap();
    let right = tempdir().unwrap();

    fs::write(left.path().join("same.txt"), b"identical").unwrap();
    fs::write(right.path().join("same.txt"), b"identical").unwrap();
    fs::write(left.path().join("diff.txt"), b"left").unwrap();
    fs::write(right.path().join("diff.txt"), b"right").unwrap();
    fs::write(left.path().join("left_only.txt"), b"only").unwrap();

    let scanner = Scanner::new(default_scan_config());
    let left_map = scanner.scan(left.path());
    let right_map = scanner.scan(right.path());

    let comparator = HashComparator::new();
    let engine = DiffEngine::new(&comparator);
    let result = engine.diff(&left_map, &right_map, left.path(), right.path());

    assert_eq!(result.total(), 3);

    let filter = DiffFilter { show_identical: false, ..DiffFilter::default() };
    let filtered: Vec<_> = result.entries.iter().filter(|e| filter.matches(e)).collect();
    assert_eq!(filtered.len(), 2);
    assert!(filtered.iter().all(|e| e.status.has_difference()));
}
