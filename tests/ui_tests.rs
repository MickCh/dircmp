use dircmp::engine::diff::{DiffEntry, DiffStatus};
use dircmp::ui::panel::{build_view_rows, first_entry, last_entry, next_entry, prev_entry, ViewRow};
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn file_entry(path: &str, status: DiffStatus) -> DiffEntry {
    DiffEntry { relative_path: PathBuf::from(path), status, is_dir: false }
}

fn dir_entry(path: &str) -> DiffEntry {
    DiffEntry { relative_path: PathBuf::from(path), status: DiffStatus::DirectoryPresent, is_dir: true }
}

fn all_match(_: &DiffEntry) -> bool { true }

fn header_labels(rows: &[ViewRow]) -> Vec<String> {
    rows.iter()
        .filter_map(|r| if let ViewRow::FolderHeader(s) = r { Some(s.clone()) } else { None })
        .collect()
}

fn entry_indices(rows: &[ViewRow]) -> Vec<usize> {
    rows.iter()
        .filter_map(|r| if let ViewRow::Entry(i) = r { Some(*i) } else { None })
        .collect()
}

// ---------------------------------------------------------------------------
// build_view_rows — basic structure
// ---------------------------------------------------------------------------

#[test]
fn build_rows_empty_input() {
    let rows = build_view_rows(&[], all_match);
    assert!(rows.is_empty());
}

#[test]
fn build_rows_root_level_files_get_dot_header() {
    let entries = vec![
        file_entry("a.txt", DiffStatus::Identical),
        file_entry("b.txt", DiffStatus::Different),
    ];
    let rows = build_view_rows(&entries, all_match);

    assert_eq!(header_labels(&rows), vec!["./".to_string()]);
    assert_eq!(entry_indices(&rows), vec![0, 1]);
}

#[test]
fn build_rows_subdirectory_files_get_dir_header() {
    let entries = vec![
        file_entry("src/main.rs", DiffStatus::Identical),
        file_entry("src/lib.rs", DiffStatus::Different),
    ];
    let rows = build_view_rows(&entries, all_match);

    assert_eq!(header_labels(&rows), vec!["src/".to_string()]);
    assert_eq!(entry_indices(&rows), vec![0, 1]);
}

#[test]
fn build_rows_multiple_directories_emit_separate_headers() {
    let entries = vec![
        file_entry("a.txt", DiffStatus::Identical),
        file_entry("src/lib.rs", DiffStatus::Different),
        file_entry("tests/foo.rs", DiffStatus::LeftOnly),
    ];
    let rows = build_view_rows(&entries, all_match);

    assert_eq!(header_labels(&rows), vec!["./", "src/", "tests/"]);
    assert_eq!(entry_indices(&rows), vec![0, 1, 2]);
}

#[test]
fn build_rows_directory_entries_are_skipped() {
    let entries = vec![
        dir_entry("src"),
        file_entry("src/lib.rs", DiffStatus::Identical),
    ];
    let rows = build_view_rows(&entries, all_match);

    // Directory entry (index 0) must not appear as ViewRow::Entry
    assert!(!entry_indices(&rows).contains(&0));
    assert!(entry_indices(&rows).contains(&1));
}

#[test]
fn build_rows_filter_excludes_non_matching_entries() {
    let entries = vec![
        file_entry("a.txt", DiffStatus::Identical),
        file_entry("b.txt", DiffStatus::Different),
        file_entry("c.txt", DiffStatus::Identical),
    ];
    // Show only Different
    let rows = build_view_rows(&entries, |e| e.status == DiffStatus::Different);

    assert_eq!(entry_indices(&rows), vec![1]);
    // Still one header (./), but only one entry
    assert_eq!(header_labels(&rows), vec!["./".to_string()]);
}

#[test]
fn build_rows_filter_removes_header_when_no_entries_pass() {
    let entries = vec![file_entry("a.txt", DiffStatus::Identical)];
    let rows = build_view_rows(&entries, |e| e.status == DiffStatus::Different);

    assert!(rows.is_empty());
}

#[test]
fn build_rows_stores_original_indices() {
    // With a directory entry at index 0, the first file entry should be index 1.
    let entries = vec![
        dir_entry("sub"),
        file_entry("sub/x.rs", DiffStatus::LeftOnly),
    ];
    let rows = build_view_rows(&entries, all_match);
    assert_eq!(entry_indices(&rows), vec![1]);
}

// ---------------------------------------------------------------------------
// Navigation — next_entry / prev_entry / first_entry / last_entry
// ---------------------------------------------------------------------------

/// Builds a simple rows vec: Header, Entry(0), Header, Entry(1)
fn two_entry_rows() -> Vec<ViewRow> {
    vec![
        ViewRow::FolderHeader("./".to_string()),
        ViewRow::Entry(0),
        ViewRow::FolderHeader("sub/".to_string()),
        ViewRow::Entry(1),
    ]
}

#[test]
fn next_entry_advances_past_headers() {
    let rows = two_entry_rows();
    // From Entry(0) at index 1, next should jump over the FolderHeader to Entry(1) at index 3.
    assert_eq!(next_entry(&rows, 1), 3);
}

#[test]
fn next_entry_at_last_stays() {
    let rows = two_entry_rows();
    assert_eq!(next_entry(&rows, 3), 3);
}

#[test]
fn prev_entry_retreats_past_headers() {
    let rows = two_entry_rows();
    // From Entry(1) at index 3, prev should jump over FolderHeader to Entry(0) at index 1.
    assert_eq!(prev_entry(&rows, 3), 1);
}

#[test]
fn prev_entry_at_first_stays() {
    let rows = two_entry_rows();
    assert_eq!(prev_entry(&rows, 1), 1);
}

#[test]
fn first_entry_skips_leading_header() {
    let rows = two_entry_rows();
    assert_eq!(first_entry(&rows), 1);
}

#[test]
fn last_entry_skips_trailing_header() {
    // Trailing header after last entry
    let rows = vec![
        ViewRow::FolderHeader("./".to_string()),
        ViewRow::Entry(0),
        ViewRow::Entry(1),
        ViewRow::FolderHeader("sub/".to_string()),
    ];
    assert_eq!(last_entry(&rows), 2);
}

#[test]
fn first_and_last_entry_on_empty_rows() {
    let rows: Vec<ViewRow> = vec![];
    assert_eq!(first_entry(&rows), 0);
    assert_eq!(last_entry(&rows), 0);
}

#[test]
fn first_and_last_entry_single_entry() {
    let rows = vec![ViewRow::FolderHeader("./".to_string()), ViewRow::Entry(0)];
    assert_eq!(first_entry(&rows), 1);
    assert_eq!(last_entry(&rows), 1);
}
