//! Application layer: the [`App`] struct and its main event loop.
//!
//! Behaviour is split across submodules, each extending `impl App`:
//! - `command` – key → `Command` map and command handling
//! - `background` – background scan/comparison spawning and channel polling
//! - `external` – external tool policy, queuing and terminal handoff
//! - `render` – per-frame drawing

mod background;
mod command;
mod external;
mod render;

use crate::{
    config::Config,
    engine::{
        comparator::create_comparator,
        diff::{DiffEntry, DiffFilter, DiffResult},
        pipeline::{CompareMsg, ScanMsg},
    },
    tools::ExternalAction,
    ui::{
        panel::{DiffView, ViewRow, ViewRows},
        AppState,
    },
};
use anyhow::Result;
use command::command_for_key;
use crossterm::event::{self, Event, KeyEventKind};
use ratatui::{backend::Backend, Terminal};
use std::{
    path::PathBuf,
    sync::{mpsc, Arc},
};

pub struct App {
    config: Config,
    left_root: PathBuf,
    right_root: PathBuf,
    diff_view: DiffView,
    diff_result: Option<DiffResult>,
    view_rows: ViewRows,
    filter: DiffFilter,
    comparator_name: String,
    should_quit: bool,
    state: AppState,
    /// Receiver for background scan results.
    scan_rx: Option<mpsc::Receiver<ScanMsg>>,
    /// Receiver for background comparison results.
    compare_rx: Option<mpsc::Receiver<CompareMsg>>,
    /// Height of the main list area (updated each frame, used for page navigation).
    page_height: usize,
    /// Timestamp of the last rebuild_filtered() call; used to throttle rebuilds.
    last_rebuild: std::time::Instant,
    /// External tool action to execute after the current frame.
    pending_action: Option<ExternalAction>,
    /// Shared rayon pool for parallel file comparison — created once, reused across F5 rescans.
    rayon_pool: Arc<rayon::ThreadPool>,
    /// Total scan errors (skipped entries) from the last scan.
    scan_errors: usize,
    /// Transient message shown in the status bar (e.g. non-zero tool exit).
    status_message: Option<String>,
}

impl App {
    pub fn new(left_path: PathBuf, right_path: PathBuf, config: Config) -> Self {
        let comparator = create_comparator(&config.comparison);
        let comparator_name = comparator.name().to_string();

        let num_threads = rayon::current_num_threads().min(8);
        let rayon_pool = Arc::new(
            rayon::ThreadPoolBuilder::new()
                .num_threads(num_threads)
                .build()
                .expect("rayon pool"),
        );

        Self {
            config,
            left_root: left_path,
            right_root: right_path,
            diff_view: DiffView::new(),
            diff_result: None,
            view_rows: ViewRows::default(),
            filter: DiffFilter::default(),
            comparator_name,
            should_quit: false,
            state: AppState::Scanning,
            scan_rx: None,
            compare_rx: None,
            page_height: 40,
            last_rebuild: std::time::Instant::now(),
            pending_action: None,
            rayon_pool,
            scan_errors: 0,
            status_message: None,
        }
    }

    // -----------------------------------------------------------------------
    // Main loop
    // -----------------------------------------------------------------------

    pub fn run<B: Backend + std::io::Write>(&mut self, terminal: &mut Terminal<B>) -> Result<()>
    where
        B::Error: Send + Sync + 'static,
    {
        self.run_diff();

        loop {
            self.poll_scan();
            // Drain comparison results before rendering so the frame is fresh.
            self.poll_comparisons();

            terminal.draw(|frame| self.render(frame))?;

            // Use a shorter timeout while scanning or comparing to keep the UI responsive.
            let timeout = if self.scan_rx.is_some() || self.compare_rx.is_some() {
                std::time::Duration::from_millis(16)
            } else {
                std::time::Duration::from_millis(100)
            };

            if event::poll(timeout)?
                && let Event::Key(key) = event::read()?
                && key.kind == KeyEventKind::Press
            {
                self.status_message = None;
                if let Some(cmd) = command_for_key(key.code, key.modifiers) {
                    self.apply(cmd);
                }
            }

            if let Some(action) = self.pending_action.take() {
                self.launch_external(terminal, action)?;
            }

            if self.should_quit {
                break;
            }
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Selection & filtered view
    // -----------------------------------------------------------------------

    fn selected_diff_entry(&self) -> Option<&DiffEntry> {
        let idx = self.diff_view.selected_index()?;
        match self.view_rows.get(idx)? {
            ViewRow::Entry(entry_idx) => self.diff_result.as_ref()?.entries.get(*entry_idx),
            ViewRow::FolderHeader(_) => None,
        }
    }

    fn rebuild_filtered(&mut self) {
        let old_idx = self.diff_view.selected_index();
        // Remember the selected entry's path so the cursor follows the entry
        // (not the row number) when rows appear or disappear mid-comparison.
        let old_path = self.selected_diff_entry().map(|e| e.relative_path.clone());

        self.view_rows = if let Some(result) = &self.diff_result {
            let filter = &self.filter;
            ViewRows::build(&result.entries, |e| filter.matches(e))
        } else {
            ViewRows::default()
        };

        // Guard: if all entries are filtered out, deselect and bail — idx=0 on an
        // empty view_rows would be an out-of-bounds selection.
        if self.view_rows.is_empty() {
            self.diff_view.list_state.select(None);
            return;
        }

        let entries = self
            .diff_result
            .as_ref()
            .map(|r| r.entries.as_slice())
            .unwrap_or_default();
        let idx = old_path
            .and_then(|path| {
                self.view_rows
                    .find_entry_row(entries, |e| e.relative_path == path)
            })
            // Entry no longer visible: fall back to the old row index, clamped.
            .or_else(|| old_idx.map(|i| i.min(self.view_rows.len() - 1)))
            .unwrap_or_else(|| self.view_rows.first());
        self.diff_view.list_state.select(Some(idx));
    }
}
