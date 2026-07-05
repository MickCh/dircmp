use dircmp::engine::{
    comparator::{
        byte::ByteComparator,
        hash::HashComparator,
        metadata::MetadataComparator,
        text::TextComparator,
        CompareResult, FileComparator,
    },
    diff::{DiffEngine, DiffEntry, DiffFilter, DiffStatus},
    scanner::Scanner,
};
use dircmp::config::ScanConfig;
use std::fs;
use std::path::{Path, PathBuf};
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
    let (left_map, _) = scanner.scan(left.path());
    let (right_map, _) = scanner.scan(right.path());

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
    let (left_map, _) = scanner.scan(left.path());
    let (right_map, _) = scanner.scan(right.path());

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
    let (left_map, _) = scanner.scan(left.path());
    let (right_map, _) = scanner.scan(right.path());

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
    let (left_map, _) = scanner.scan(left.path());
    let (right_map, _) = scanner.scan(right.path());

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
    let (map, _) = scanner.scan(left.path());

    assert!(!map.keys().any(|p: &PathBuf| p.to_string_lossy().contains(".git")));
    assert!(map.keys().any(|p: &PathBuf| p.to_string_lossy().contains("a.txt")));
    let _ = right;
}

#[test]
fn root_matching_ignore_pattern_is_still_scanned() {
    // Comparing e.g. two `node_modules` folders directly: the ignore pattern
    // must not filter out the scan root itself, only entries inside it.
    let base = tempdir().unwrap();
    let root = base.path().join("node_modules");
    fs::create_dir(&root).unwrap();
    fs::write(root.join("a.txt"), b"hello").unwrap();
    fs::create_dir(root.join("node_modules")).unwrap();
    fs::write(root.join("node_modules").join("nested.txt"), b"ignored").unwrap();

    let config = ScanConfig {
        ignore_patterns: vec!["node_modules".to_string()],
        follow_symlinks: false,
    };
    let scanner = Scanner::new(config);
    let (map, _) = scanner.scan(&root);

    assert!(map.keys().any(|p: &PathBuf| p.to_string_lossy().contains("a.txt")));
    assert!(!map.keys().any(|p: &PathBuf| p.to_string_lossy().contains("nested.txt")));
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
    let (left_map, _) = scanner.scan(left.path());
    let (right_map, _) = scanner.scan(right.path());

    let comparator = HashComparator::new();
    let engine = DiffEngine::new(&comparator);
    let result = engine.diff(&left_map, &right_map, left.path(), right.path());

    assert_eq!(result.total(), 3);

    let filter = DiffFilter { show_identical: false, ..DiffFilter::default() };
    let filtered: Vec<_> = result.entries.iter().filter(|e| filter.matches(e)).collect();
    assert_eq!(filtered.len(), 2);
    assert!(filtered.iter().all(|e| e.status.has_difference()));
}

// ---------------------------------------------------------------------------
// ByteComparator
// ---------------------------------------------------------------------------

#[test]
fn byte_comparator_identical() {
    let dir = tempdir().unwrap();
    let a = dir.path().join("a.bin");
    let b = dir.path().join("b.bin");
    fs::write(&a, b"binary data").unwrap();
    fs::write(&b, b"binary data").unwrap();

    let cmp = ByteComparator::new();
    assert_eq!(cmp.compare(&a, &b).unwrap(), CompareResult::Identical);
}

#[test]
fn byte_comparator_different_content_same_size() {
    let dir = tempdir().unwrap();
    let a = dir.path().join("a.bin");
    let b = dir.path().join("b.bin");
    fs::write(&a, b"AAAA").unwrap();
    fs::write(&b, b"BBBB").unwrap();

    let cmp = ByteComparator::new();
    assert_eq!(cmp.compare(&a, &b).unwrap(), CompareResult::Different);
}

#[test]
fn byte_comparator_different_sizes_fast_path() {
    let dir = tempdir().unwrap();
    let a = dir.path().join("a.bin");
    let b = dir.path().join("b.bin");
    fs::write(&a, b"short").unwrap();
    fs::write(&b, b"much longer content here").unwrap();

    let cmp = ByteComparator::new();
    assert_eq!(cmp.compare(&a, &b).unwrap(), CompareResult::Different);
}

#[test]
fn byte_comparator_empty_files() {
    let dir = tempdir().unwrap();
    let a = dir.path().join("a.bin");
    let b = dir.path().join("b.bin");
    fs::write(&a, b"").unwrap();
    fs::write(&b, b"").unwrap();

    let cmp = ByteComparator::new();
    assert_eq!(cmp.compare(&a, &b).unwrap(), CompareResult::Identical);
}

// ---------------------------------------------------------------------------
// MetadataComparator
// ---------------------------------------------------------------------------

#[test]
fn metadata_comparator_different_sizes() {
    let dir = tempdir().unwrap();
    let a = dir.path().join("a.txt");
    let b = dir.path().join("b.txt");
    fs::write(&a, b"short").unwrap();
    fs::write(&b, b"longer content").unwrap();

    let cmp = MetadataComparator::new();
    assert_eq!(cmp.compare(&a, &b).unwrap(), CompareResult::Different);
}

#[test]
fn metadata_comparator_same_size_and_mtime() {
    let dir = tempdir().unwrap();
    let a = dir.path().join("a.txt");
    let b = dir.path().join("b.txt");
    fs::write(&a, b"hello").unwrap();
    fs::write(&b, b"hello").unwrap();

    // Pin both files to the same mtime so the comparator sees them as identical.
    // File::set_modified is stable since Rust 1.75 (MSRV here is 1.85+).
    let fixed_mtime =
        std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000);
    fs::File::options().write(true).open(&a).unwrap().set_modified(fixed_mtime).unwrap();
    fs::File::options().write(true).open(&b).unwrap().set_modified(fixed_mtime).unwrap();

    let cmp = MetadataComparator::new();
    assert_eq!(cmp.compare(&a, &b).unwrap(), CompareResult::Identical);
}

// ---------------------------------------------------------------------------
// TextComparator::compare()
// ---------------------------------------------------------------------------

#[test]
fn text_comparator_identical_files() {
    let dir = tempdir().unwrap();
    let a = dir.path().join("a.txt");
    let b = dir.path().join("b.txt");
    fs::write(&a, "hello world\n").unwrap();
    fs::write(&b, "hello world\n").unwrap();

    let cmp = TextComparator::new(false, false);
    assert_eq!(cmp.compare(&a, &b).unwrap(), CompareResult::Identical);
}

#[test]
fn text_comparator_different_files() {
    let dir = tempdir().unwrap();
    let a = dir.path().join("a.txt");
    let b = dir.path().join("b.txt");
    fs::write(&a, "foo\n").unwrap();
    fs::write(&b, "bar\n").unwrap();

    let cmp = TextComparator::new(false, false);
    assert_eq!(cmp.compare(&a, &b).unwrap(), CompareResult::Different);
}

#[test]
fn text_comparator_whitespace_normalization() {
    let dir = tempdir().unwrap();
    let a = dir.path().join("a.txt");
    let b = dir.path().join("b.txt");
    fs::write(&a, "hello   world\n  foo  \n").unwrap();
    fs::write(&b, "hello world\nfoo\n").unwrap();

    let cmp = TextComparator::new(true, false);
    assert_eq!(cmp.compare(&a, &b).unwrap(), CompareResult::Identical);
}

#[test]
fn text_comparator_case_normalization() {
    let dir = tempdir().unwrap();
    let a = dir.path().join("a.txt");
    let b = dir.path().join("b.txt");
    fs::write(&a, "Hello World\n").unwrap();
    fs::write(&b, "hello world\n").unwrap();

    let cmp = TextComparator::new(false, true);
    assert_eq!(cmp.compare(&a, &b).unwrap(), CompareResult::Identical);
}

// ---------------------------------------------------------------------------
// TypeConflict (file on one side, directory on the other)
// ---------------------------------------------------------------------------

#[test]
fn type_conflict_file_vs_directory() {
    let left = tempdir().unwrap();
    let right = tempdir().unwrap();

    // Left: regular file named "thing"
    fs::write(left.path().join("thing"), b"data").unwrap();
    // Right: directory named "thing"
    fs::create_dir(right.path().join("thing")).unwrap();

    let scanner = Scanner::new(default_scan_config());
    let (left_map, _) = scanner.scan(left.path());
    let (right_map, _) = scanner.scan(right.path());

    let comparator = HashComparator::new();
    let engine = DiffEngine::new(&comparator);
    let result = engine.diff(&left_map, &right_map, left.path(), right.path());

    let conflict = result.entries.iter().find(|e| e.relative_path.to_str() == Some("thing"));
    assert!(conflict.is_some(), "expected an entry for 'thing'");
    assert_eq!(conflict.unwrap().status, DiffStatus::TypeConflict);
}

// ---------------------------------------------------------------------------
// Scanner — nested directories
// ---------------------------------------------------------------------------

#[test]
fn scanner_recurses_into_subdirectories() {
    let dir = tempdir().unwrap();
    fs::create_dir(dir.path().join("sub")).unwrap();
    fs::write(dir.path().join("root.txt"), b"root").unwrap();
    fs::write(dir.path().join("sub").join("nested.txt"), b"nested").unwrap();

    let scanner = Scanner::new(default_scan_config());
    let (map, _) = scanner.scan(dir.path());

    assert!(map.contains_key(Path::new("root.txt")));
    assert!(map.contains_key(Path::new("sub/nested.txt")));
    // The sub-directory itself should be present as a directory entry.
    assert!(map.get(Path::new("sub")).is_some_and(|e| e.is_dir));
}

// ---------------------------------------------------------------------------
// DiffFilter — all toggle combinations
// ---------------------------------------------------------------------------

fn make_entry(status: DiffStatus) -> DiffEntry {
    DiffEntry {
        relative_path: PathBuf::from("file.txt"),
        status,
        is_dir: false,
    }
}

#[test]
fn filter_all_enabled_passes_everything() {
    let filter = DiffFilter::default();
    assert!(filter.matches(&make_entry(DiffStatus::LeftOnly)));
    assert!(filter.matches(&make_entry(DiffStatus::RightOnly)));
    assert!(filter.matches(&make_entry(DiffStatus::Different)));
    assert!(filter.matches(&make_entry(DiffStatus::Identical)));
}

#[test]
fn filter_left_only_toggle() {
    let filter = DiffFilter { show_left_only: false, ..DiffFilter::default() };
    assert!(!filter.matches(&make_entry(DiffStatus::LeftOnly)));
    assert!(filter.matches(&make_entry(DiffStatus::RightOnly)));
    assert!(filter.matches(&make_entry(DiffStatus::Different)));
    assert!(filter.matches(&make_entry(DiffStatus::Identical)));
}

#[test]
fn filter_right_only_toggle() {
    let filter = DiffFilter { show_right_only: false, ..DiffFilter::default() };
    assert!(filter.matches(&make_entry(DiffStatus::LeftOnly)));
    assert!(!filter.matches(&make_entry(DiffStatus::RightOnly)));
}

#[test]
fn filter_different_toggle() {
    let filter = DiffFilter { show_different: false, ..DiffFilter::default() };
    assert!(!filter.matches(&make_entry(DiffStatus::Different)));
    assert!(filter.matches(&make_entry(DiffStatus::Identical)));
}

#[test]
fn filter_pending_always_passes() {
    // Pending entries are always shown (catch-all arm in matches())
    let filter = DiffFilter {
        show_left_only: false,
        show_right_only: false,
        show_different: false,
        show_identical: false,
    };
    assert!(filter.matches(&make_entry(DiffStatus::Pending)));
}
