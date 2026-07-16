//! Background diff phases: spawning the scan/comparison pipeline and polling
//! its channels. No comparison logic lives here — that stays in `engine`.

use super::App;
use crate::{
    engine::{
        comparator::create_comparator,
        diff::DiffEngine,
        pipeline::{self, CompareMsg, ScanMsg},
    },
    ui::AppState,
};
use std::sync::mpsc;

impl App {
    /// Full re-scan triggered at startup and by F5.
    pub fn run_diff(&mut self) {
        self.state = AppState::Scanning;
        self.start_background_scan();
    }

    /// Phase 1: kicks off the background scan of both folder trees.
    fn start_background_scan(&mut self) {
        self.compare_rx = None;
        self.status_message = None;
        self.scan_rx = Some(pipeline::spawn_scan(
            self.config.scan.clone(),
            self.left_root.clone(),
            self.right_root.clone(),
        ));
    }

    /// Checks whether the background scan has finished. When done, builds the
    /// diff structure, rebuilds the view and kicks off phase-2 comparison.
    pub(super) fn poll_scan(&mut self) {
        let msg = match &self.scan_rx {
            Some(rx) => match rx.try_recv() {
                Ok(m) => m,
                Err(mpsc::TryRecvError::Empty) => return,
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.scan_rx = None;
                    self.state = AppState::Ready;
                    self.status_message = Some("Scan thread crashed unexpectedly".to_string());
                    return;
                }
            },
            None => return,
        };

        let ScanMsg::Done { left_map, right_map, errors } = msg;
        self.scan_rx = None;
        self.scan_errors = errors;
        let result = DiffEngine::diff_structure(&left_map, &right_map);
        self.diff_result = Some(result);
        self.rebuild_filtered();
        self.start_background_comparison();
    }

    /// Phase 2: hands the Pending entries to the background comparison pipeline.
    fn start_background_comparison(&mut self) {
        let result = self
            .diff_result
            .as_ref()
            .expect("diff_result set by poll_scan before phase 2");
        let comparator = create_comparator(&self.config.comparison);

        match pipeline::spawn_comparison(
            result,
            &self.left_root,
            &self.right_root,
            comparator,
            self.rayon_pool.clone(),
            self.config.comparison.parallel,
        ) {
            Some((rx, total)) => {
                self.state = AppState::Comparing { done: 0, total };
                self.compare_rx = Some(rx);
            }
            None => self.state = AppState::Ready,
        }
    }

    /// Drains pending messages from the comparison channel (up to MAX_MSGS_PER_POLL).
    /// Returns `true` if any entry was updated (view needs refresh).
    pub(super) fn poll_comparisons(&mut self) -> bool {
        if self.compare_rx.is_none() {
            return false;
        }

        // Process at most this many messages per call so the UI can refresh
        // between batches and show incremental progress.
        const MAX_MSGS_PER_POLL: usize = 2000;

        let mut changed = false;
        let mut count = 0;

        loop {
            if count >= MAX_MSGS_PER_POLL {
                break;
            }
            let msg = match self.compare_rx.as_ref().unwrap().try_recv() {
                Ok(m) => m,
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.compare_rx = None;
                    self.state = AppState::Ready;
                    changed = true;
                    break;
                }
            };

            match msg {
                CompareMsg::Result { path, status } => {
                    if let Some(result) = &mut self.diff_result
                        && let Some(idx) = result.find_by_path(&path)
                    {
                        result.update_entry_status(idx, status);
                        changed = true;
                    }
                    if let AppState::Comparing { done, .. } = &mut self.state {
                        *done += 1;
                    }
                    count += 1;
                }
                CompareMsg::Done => {
                    self.compare_rx = None;
                    self.state = AppState::Ready;
                    changed = true;
                    // count is not incremented here: Done always breaks the loop
                    // regardless of MAX_MSGS_PER_POLL, so its value would go unread.
                    break;
                }
            }
        }

        if changed {
            // Rebuilding view_rows iterates all entries (O(n)); throttle to avoid
            // doing it on every 16 ms frame while the channel is producing fast.
            let channel_done = self.compare_rx.is_none();
            let elapsed = self.last_rebuild.elapsed();
            if channel_done || elapsed >= std::time::Duration::from_millis(200) {
                self.rebuild_filtered();
                self.last_rebuild = std::time::Instant::now();
            }
        }

        changed
    }
}
