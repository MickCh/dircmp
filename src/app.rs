use crate::{
    config::Config,
    engine::{
        comparator::{create_comparator, CompareResult, FileComparator},
        diff::{DiffEngine, DiffEntry, DiffStatus},
        scanner::Scanner,
    },
    platform,
    ui::{
        layout::AppLayout,
        panel::{
            build_view_rows, first_entry, last_entry, next_entry, prev_entry, DiffView, ViewRow,
        },
        statusbar::StatusBar,
        theme::Theme,
    },
};
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{
    backend::Backend,
    layout::{Alignment, Rect},
    style::{Color, Style},
    text::Line,
    widgets::{Block, Borders, Clear, Paragraph},
    Terminal,
};
use std::{path::{Path, PathBuf}, sync::mpsc};

// ---------------------------------------------------------------------------
// Background comparison channel
// ---------------------------------------------------------------------------

enum CompareMsg {
    Result { path: PathBuf, status: DiffStatus },
    Done,
}

// ---------------------------------------------------------------------------
// App state
// ---------------------------------------------------------------------------

pub enum AppState {
    Idle,
    Scanning,
    /// Files are being compared in a background thread.
    Comparing { done: usize, total: usize },
    Ready,
}

// ---------------------------------------------------------------------------
// Diff filter
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffFilter {
    All,
    DifferencesOnly,
}

impl DiffFilter {
    pub fn matches(&self, entry: &DiffEntry) -> bool {
        match self {
            DiffFilter::All => true,
            // Pending entries are shown in DifferencesOnly mode — we don't
            // know yet whether they are identical or different.
            DiffFilter::DifferencesOnly => entry.status.has_difference(),
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            DiffFilter::All => "All",
            DiffFilter::DifferencesOnly => "Differences only",
        }
    }

    pub fn toggle(self) -> Self {
        match self {
            DiffFilter::All => DiffFilter::DifferencesOnly,
            DiffFilter::DifferencesOnly => DiffFilter::All,
        }
    }
}

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------

pub struct App {
    config: Config,
    left_root: PathBuf,
    right_root: PathBuf,
    diff_view: DiffView,
    diff_result: Option<crate::engine::diff::DiffResult>,
    filtered_entries: Vec<DiffEntry>,
    view_rows: Vec<ViewRow>,
    filter: DiffFilter,
    comparator_name: String,
    should_quit: bool,
    state: AppState,
    /// Receiver for background comparison results.
    compare_rx: Option<mpsc::Receiver<CompareMsg>>,
}

impl App {
    pub fn new(left_path: PathBuf, right_path: PathBuf, mut config: Config) -> Self {
        // Auto-detect disk type on Linux; override config if successful.
        if let Some(rotational) = platform::is_rotational(&left_path) {
            config.comparison.parallel = !rotational;
        }

        let comparator = create_comparator(&config.comparison);
        let comparator_name = comparator.name().to_string();

        Self {
            config,
            left_root: left_path,
            right_root: right_path,
            diff_view: DiffView::new(),
            diff_result: None,
            filtered_entries: Vec::new(),
            view_rows: Vec::new(),
            filter: DiffFilter::All,
            comparator_name,
            should_quit: false,
            state: AppState::Idle,
            compare_rx: None,
        }
    }

    // -----------------------------------------------------------------------
    // Diff phases
    // -----------------------------------------------------------------------

    /// Phase 1: filesystem scan + structure diff (no file reading). Fast.
    fn scan_and_build_structure(&mut self) {
        // Cancel any ongoing background comparison.
        self.compare_rx = None;

        let scanner = Scanner::new(self.config.scan.clone());
        let left_map = scanner.scan(&self.left_root);
        let right_map = scanner.scan(&self.right_root);

        let result = DiffEngine::diff_structure(&left_map, &right_map);
        self.diff_result = Some(result);
        self.rebuild_filtered();
    }

    /// Phase 2: spawn a background thread that compares Pending entries one
    /// by one and sends results through an mpsc channel.
    fn start_background_comparison(&mut self) {
        let left_root = self.left_root.clone();
        let right_root = self.right_root.clone();

        let to_compare: Vec<(PathBuf, PathBuf, PathBuf)> = self
            .diff_result
            .as_ref()
            .unwrap()
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
            self.state = AppState::Ready;
            return;
        }

        let total = to_compare.len();
        self.state = AppState::Comparing { done: 0, total };
        let (tx, rx) = mpsc::channel();
        let comparator = create_comparator(&self.config.comparison);
        let parallel = self.config.comparison.parallel;

        std::thread::spawn(move || {
            if parallel {
                use rayon::prelude::*;
                let par_tx = tx.clone();
                to_compare.par_iter().for_each_with(par_tx, |tx, (rel, left, right)| {
                    let status = compare_one(comparator.as_ref(), left, right);
                    let _ = tx.send(CompareMsg::Result { path: rel.clone(), status });
                });
            } else {
                for (rel, left, right) in &to_compare {
                    let status = compare_one(comparator.as_ref(), left, right);
                    if tx.send(CompareMsg::Result { path: rel.clone(), status }).is_err() {
                        return;
                    }
                }
            }
            let _ = tx.send(CompareMsg::Done);
        });

        self.compare_rx = Some(rx);
    }

    /// Full re-scan triggered by F5.
    pub fn run_diff(&mut self) {
        self.state = AppState::Scanning;
        self.scan_and_build_structure();
        self.start_background_comparison();
    }

    // -----------------------------------------------------------------------
    // Background channel polling
    // -----------------------------------------------------------------------

    /// Drains all pending messages from the comparison channel.
    /// Returns `true` if any entry was updated (view needs refresh).
    fn poll_comparisons(&mut self) -> bool {
        if self.compare_rx.is_none() {
            return false;
        }

        let mut changed = false;

        loop {
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
                    if let Some(result) = &mut self.diff_result {
                        if let Some(entry) =
                            result.entries.iter_mut().find(|e| e.relative_path == path)
                        {
                            entry.status = status;
                            changed = true;
                        }
                    }
                    if let AppState::Comparing { done, .. } = &mut self.state {
                        *done += 1;
                    }
                }
                CompareMsg::Done => {
                    self.compare_rx = None;
                    self.state = AppState::Ready;
                    changed = true;
                    break;
                }
            }
        }

        if changed {
            self.rebuild_filtered();
        }

        changed
    }

    // -----------------------------------------------------------------------
    // Filtered view
    // -----------------------------------------------------------------------

    fn rebuild_filtered(&mut self) {
        let old_idx = self.diff_view.selected_index();

        self.filtered_entries = self
            .diff_result
            .as_ref()
            .map(|r| {
                r.entries
                    .iter()
                    .filter(|e| self.filter.matches(e))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();

        self.view_rows = build_view_rows(&self.filtered_entries);

        // Preserve cursor position, clamped to valid range.
        let idx = if self.view_rows.is_empty() {
            self.diff_view.list_state.select(None);
            return;
        } else {
            old_idx
                .map(|i| i.min(self.view_rows.len().saturating_sub(1)))
                .unwrap_or_else(|| first_entry(&self.view_rows))
        };
        self.diff_view.list_state.select(Some(idx));
    }

    // -----------------------------------------------------------------------
    // Main loop
    // -----------------------------------------------------------------------

    pub fn run<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
        // Show scanning overlay before the blocking filesystem scan.
        self.state = AppState::Scanning;
        terminal.draw(|frame| self.render(frame))?;

        self.scan_and_build_structure();
        self.start_background_comparison();

        loop {
            // Drain comparison results before rendering so the frame is fresh.
            self.poll_comparisons();

            terminal.draw(|frame| self.render(frame))?;

            // Use a shorter timeout while comparing to keep the UI responsive.
            let timeout = if self.compare_rx.is_some() {
                std::time::Duration::from_millis(16)
            } else {
                std::time::Duration::from_millis(100)
            };

            if event::poll(timeout)? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        self.handle_key(key.code);
                    }
                }
            }

            if self.should_quit {
                break;
            }
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Input handling
    // -----------------------------------------------------------------------

    fn handle_key(&mut self, code: KeyCode) {
        let step = self.config.ui.panel_scroll_step;

        match code {
            KeyCode::Char('q') | KeyCode::Char('Q') => {
                self.should_quit = true;
            }
            KeyCode::F(5) => {
                self.run_diff();
            }
            KeyCode::Char('f') | KeyCode::Char('F') => {
                self.filter = self.filter.toggle();
                self.rebuild_filtered();
            }
            KeyCode::Down => {
                let cur = self.diff_view.selected_index().unwrap_or(0);
                let next = next_entry(&self.view_rows, cur);
                self.diff_view.list_state.select(Some(next));
            }
            KeyCode::Up => {
                let cur = self.diff_view.selected_index().unwrap_or(0);
                let prev = prev_entry(&self.view_rows, cur);
                self.diff_view.list_state.select(Some(prev));
            }
            KeyCode::PageDown => {
                let mut idx = self.diff_view.selected_index().unwrap_or(0);
                for _ in 0..step {
                    idx = next_entry(&self.view_rows, idx);
                }
                self.diff_view.list_state.select(Some(idx));
            }
            KeyCode::PageUp => {
                let mut idx = self.diff_view.selected_index().unwrap_or(0);
                for _ in 0..step {
                    idx = prev_entry(&self.view_rows, idx);
                }
                self.diff_view.list_state.select(Some(idx));
            }
            KeyCode::Home => {
                self.diff_view.list_state.select(Some(first_entry(&self.view_rows)));
            }
            KeyCode::End => {
                self.diff_view.list_state.select(Some(last_entry(&self.view_rows)));
            }
            _ => {}
        }
    }

    // -----------------------------------------------------------------------
    // Rendering
    // -----------------------------------------------------------------------

    fn render(&mut self, frame: &mut ratatui::Frame) {
        let layout = AppLayout::compute(frame.area());

        // Paths header
        let half = (layout.header.width as usize).saturating_sub(2) / 2;
        let header_text = format!(
            " {:<half$}  {}",
            self.left_root.display(),
            self.right_root.display(),
            half = half,
        );
        frame.render_widget(
            Paragraph::new(header_text).style(Theme::header_path()),
            layout.header,
        );

        // Diff list
        self.diff_view.render(frame, layout.main, &self.view_rows);

        // Status bar
        StatusBar::render(
            frame,
            layout.statusbar,
            self.diff_result.as_ref(),
            &self.comparator_name,
            &self.state,
            &self.filter,
        );

        // Overlay for transient states
        match &self.state {
            AppState::Scanning => render_overlay(frame, " ⏳ Scanning folders… "),
            AppState::Idle => render_overlay(frame, " Press F5 to start comparison "),
            AppState::Comparing { .. } | AppState::Ready => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn compare_one(comparator: &dyn FileComparator, left: &Path, right: &Path) -> DiffStatus {
    match comparator.compare(left, right) {
        Ok(CompareResult::Identical) => DiffStatus::Identical,
        Ok(CompareResult::Different) => DiffStatus::Different,
        Ok(CompareResult::Error(e)) => DiffStatus::Error(e),
        Err(e) => DiffStatus::Error(e.to_string()),
    }
}

fn render_overlay(frame: &mut ratatui::Frame, message: &str) {
    let area = frame.area();
    let msg_len = message.len() as u16;
    let width = msg_len.min(area.width.saturating_sub(4)) + 4;
    let height = 3u16;
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    let popup_area = Rect::new(x, y, width, height);

    frame.render_widget(Clear, popup_area);
    let block = Block::default()
        .borders(Borders::ALL)
        .style(Style::default().bg(Color::DarkGray).fg(Color::White));
    let paragraph = Paragraph::new(Line::from(message.to_string()))
        .block(block)
        .alignment(Alignment::Center);
    frame.render_widget(paragraph, popup_area);
}
