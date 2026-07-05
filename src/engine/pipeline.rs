//! Background execution of the two diff phases.
//!
//! Phase 1 (scan) and phase 2 (content comparison) each run in their own
//! thread and report back through an `mpsc` channel, so the UI layer only
//! polls receivers — it contains no comparison logic of its own.

use crate::{
    config::ScanConfig,
    engine::{
        comparator::FileComparator,
        diff::{compare_entry, DiffResult, DiffStatus},
        scanner::{EntryMap, Scanner},
    },
};
use std::{
    path::{Path, PathBuf},
    sync::{mpsc, Arc},
};

/// Message from the background scan thread (phase 1).
pub enum ScanMsg {
    Done {
        left_map: EntryMap,
        right_map: EntryMap,
        /// Total skipped entries (permission errors, broken symlinks, …).
        errors: usize,
    },
}

/// Message from the background comparison thread (phase 2).
pub enum CompareMsg {
    Result { path: PathBuf, status: DiffStatus },
    Done,
}

/// Phase 1: scans both folder trees in a background thread (each side in
/// parallel via `rayon::join`). The result arrives on the returned receiver.
pub fn spawn_scan(
    config: ScanConfig,
    left_root: PathBuf,
    right_root: PathBuf,
) -> mpsc::Receiver<ScanMsg> {
    let (tx, rx) = mpsc::channel();

    std::thread::spawn(move || {
        let scanner = Scanner::new(config);
        let ((left_map, left_errors), (right_map, right_errors)) =
            rayon::join(|| scanner.scan(&left_root), || scanner.scan(&right_root));
        let _ = tx.send(ScanMsg::Done {
            left_map,
            right_map,
            errors: left_errors + right_errors,
        });
    });

    rx
}

/// Phase 2: compares all `Pending` entries of `result` in a background thread
/// and streams per-file results through the returned receiver.
///
/// Returns `None` when there is nothing to compare; otherwise the receiver
/// plus the number of queued files (for progress display).
pub fn spawn_comparison(
    result: &DiffResult,
    left_root: &Path,
    right_root: &Path,
    comparator: Box<dyn FileComparator>,
    pool: Arc<rayon::ThreadPool>,
    parallel: bool,
) -> Option<(mpsc::Receiver<CompareMsg>, usize)> {
    let to_compare: Vec<(PathBuf, PathBuf, PathBuf)> = result
        .entries
        .iter()
        .filter(|e| e.status == DiffStatus::Pending)
        .map(|e| {
            let rel = e.relative_path.clone();
            let abs_left = left_root.join(&rel);
            let abs_right = right_root.join(&rel);
            (rel, abs_left, abs_right)
        })
        .collect();

    if to_compare.is_empty() {
        return None;
    }

    let total = to_compare.len();
    let (tx, rx) = mpsc::channel();

    std::thread::spawn(move || {
        if parallel {
            use rayon::prelude::*;
            let par_tx = tx.clone();
            pool.install(|| {
                to_compare.par_iter().for_each_with(par_tx, |tx, (rel, left, right)| {
                    let status = compare_entry(comparator.as_ref(), left, right);
                    let _ = tx.send(CompareMsg::Result { path: rel.clone(), status });
                });
            });
        } else {
            for (rel, left, right) in &to_compare {
                let status = compare_entry(comparator.as_ref(), left, right);
                if tx.send(CompareMsg::Result { path: rel.clone(), status }).is_err() {
                    return;
                }
            }
        }
        let _ = tx.send(CompareMsg::Done);
    });

    Some((rx, total))
}
