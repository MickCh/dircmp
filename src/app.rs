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
            build_view_rows, first_entry, last_entry, next_entry, next_entry_matching,
            prev_entry, prev_entry_matching, DiffView, ViewRow,
        },
        statusbar::StatusBar,
        theme::Theme,
    },
};
use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
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
// External tool actions
// ---------------------------------------------------------------------------

enum ExternalAction {
    Diff { left: PathBuf, right: PathBuf },
    ViewLeft(PathBuf),
    EditLeft(PathBuf),
    ViewRight(PathBuf),
    EditRight(PathBuf),
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

#[derive(Debug, Clone, Copy)]
pub struct DiffFilter {
    pub show_left_only: bool,
    pub show_right_only: bool,
    pub show_different: bool,
    pub show_identical: bool,
}

impl Default for DiffFilter {
    fn default() -> Self {
        Self {
            show_left_only: true,
            show_right_only: true,
            show_different: true,
            show_identical: true,
        }
    }
}

impl DiffFilter {
    pub fn matches(&self, entry: &DiffEntry) -> bool {
        match entry.status {
            DiffStatus::LeftOnly => self.show_left_only,
            DiffStatus::RightOnly => self.show_right_only,
            DiffStatus::Different => self.show_different,
            DiffStatus::Identical => self.show_identical,
            _ => true,
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
    view_rows: Vec<ViewRow>,
    filter: DiffFilter,
    comparator_name: String,
    should_quit: bool,
    state: AppState,
    /// Receiver for background comparison results.
    compare_rx: Option<mpsc::Receiver<CompareMsg>>,
    /// Height of the main list area (updated each frame, used for page navigation).
    page_height: usize,
    /// Timestamp of the last rebuild_filtered() call; used to throttle rebuilds.
    last_rebuild: std::time::Instant,
    /// External tool action to execute after the current frame.
    pending_action: Option<ExternalAction>,
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
            view_rows: Vec::new(),
            filter: DiffFilter::default(),
            comparator_name,
            should_quit: false,
            state: AppState::Idle,
            compare_rx: None,
            page_height: 40,
            last_rebuild: std::time::Instant::now(),
            pending_action: None,
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
        let (left_map, right_map) = rayon::join(
            || scanner.scan(&self.left_root),
            || scanner.scan(&self.right_root),
        );

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
                // For I/O-bound comparators (e.g. SHA-256), more than ~8 threads
                // saturates disk bandwidth without improving throughput.
                let num_threads = rayon::current_num_threads().min(8);
                let pool = rayon::ThreadPoolBuilder::new()
                    .num_threads(num_threads)
                    .build()
                    .expect("rayon pool");
                let par_tx = tx.clone();
                pool.install(|| {
                    to_compare.par_iter().for_each_with(par_tx, |tx, (rel, left, right)| {
                        let status = compare_one(comparator.as_ref(), left, right);
                        let _ = tx.send(CompareMsg::Result { path: rel.clone(), status });
                    });
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

    /// Drains pending messages from the comparison channel (up to MAX_MSGS_PER_POLL).
    /// Returns `true` if any entry was updated (view needs refresh).
    fn poll_comparisons(&mut self) -> bool {
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
                    if let Some(result) = &mut self.diff_result {
                        // entries are sorted by path (diff_structure calls all_paths.sort()),
                        // so binary search is O(log n) instead of O(n) linear scan.
                        if let Ok(idx) = result
                            .entries
                            .binary_search_by(|e| e.relative_path.cmp(&path))
                        {
                            result.entries[idx].status = status;
                            changed = true;
                        }
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

    // -----------------------------------------------------------------------
    // Filtered view
    // -----------------------------------------------------------------------

    fn rebuild_filtered(&mut self) {
        let old_idx = self.diff_view.selected_index();

        // Build view_rows directly from diff_result without cloning entries.
        // ViewRow::Entry(i) stores an index into diff_result.entries.
        let new_rows = if let Some(result) = &self.diff_result {
            let filter = &self.filter;
            build_view_rows(&result.entries, |e| filter.matches(e))
        } else {
            Vec::new()
        };
        self.view_rows = new_rows;

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
    // Selection helpers
    // -----------------------------------------------------------------------

    fn selected_diff_entry(&self) -> Option<&DiffEntry> {
        let idx = self.diff_view.selected_index()?;
        match self.view_rows.get(idx)? {
            ViewRow::Entry(entry_idx) => self.diff_result.as_ref()?.entries.get(*entry_idx),
            ViewRow::FolderHeader(_) => None,
        }
    }

    // -----------------------------------------------------------------------
    // External tool launching
    // -----------------------------------------------------------------------

    fn launch_external<B: Backend + std::io::Write>(
        &self,
        terminal: &mut Terminal<B>,
        action: ExternalAction,
    ) -> Result<()> {
        disable_raw_mode()?;
        execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

        let result = self.run_external_command(&action);

        enable_raw_mode()?;
        execute!(terminal.backend_mut(), EnterAlternateScreen)?;
        terminal.clear()?;

        result
    }

    fn run_external_command(&self, action: &ExternalAction) -> Result<()> {
        let tools = &self.config.tools;
        match action {
            ExternalAction::Diff { left, right } => {
                if let Some(cmd) = &tools.diff_tool {
                    launch_cmd(cmd, &[left, right])?;
                }
            }
            ExternalAction::ViewLeft(p) | ExternalAction::ViewRight(p) => {
                if let Some(cmd) = &tools.viewer {
                    launch_cmd(cmd, &[p])?;
                }
            }
            ExternalAction::EditLeft(p) | ExternalAction::EditRight(p) => {
                if let Some(cmd) = &tools.editor {
                    launch_cmd(cmd, &[p])?;
                }
            }
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Main loop
    // -----------------------------------------------------------------------

    pub fn run<B: Backend + std::io::Write>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
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

            if event::poll(timeout)?
                && let Event::Key(key) = event::read()?
                && key.kind == KeyEventKind::Press
            {
                self.handle_key(key.code);
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
    // Input handling
    // -----------------------------------------------------------------------

    fn handle_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('q') | KeyCode::Char('Q') => {
                self.should_quit = true;
            }
            KeyCode::F(5) => {
                self.run_diff();
            }
            KeyCode::Char('l') | KeyCode::Char('L') => {
                self.filter.show_left_only = !self.filter.show_left_only;
                self.rebuild_filtered();
            }
            KeyCode::Char('r') | KeyCode::Char('R') => {
                self.filter.show_right_only = !self.filter.show_right_only;
                self.rebuild_filtered();
            }
            KeyCode::Char('d') | KeyCode::Char('D') => {
                self.filter.show_different = !self.filter.show_different;
                self.rebuild_filtered();
            }
            KeyCode::Char('i') | KeyCode::Char('I') => {
                self.filter.show_identical = !self.filter.show_identical;
                self.rebuild_filtered();
            }
            KeyCode::Char('n') => self.jump_to_matching(true),
            KeyCode::Char('N') => self.jump_to_matching(false),
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
                for _ in 0..self.page_height {
                    idx = next_entry(&self.view_rows, idx);
                }
                self.diff_view.list_state.select(Some(idx));
            }
            KeyCode::PageUp => {
                let mut idx = self.diff_view.selected_index().unwrap_or(0);
                for _ in 0..self.page_height {
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
            KeyCode::Enter => {
                if let Some(entry) = self.selected_diff_entry() {
                    let left = self.left_root.join(&entry.relative_path);
                    let right = self.right_root.join(&entry.relative_path);
                    self.pending_action = match entry.status {
                        DiffStatus::Different => Some(ExternalAction::Diff { left, right }),
                        DiffStatus::LeftOnly => Some(ExternalAction::ViewLeft(left)),
                        DiffStatus::RightOnly => Some(ExternalAction::ViewRight(right)),
                        _ => None,
                    };
                }
            }
            KeyCode::Char('[') => {
                if let Some(entry) = self.selected_diff_entry() {
                    let path = self.left_root.join(&entry.relative_path);
                    self.pending_action = Some(ExternalAction::ViewLeft(path));
                }
            }
            KeyCode::Char(']') => {
                if let Some(entry) = self.selected_diff_entry() {
                    let path = self.right_root.join(&entry.relative_path);
                    self.pending_action = Some(ExternalAction::ViewRight(path));
                }
            }
            KeyCode::Char('{') => {
                if let Some(entry) = self.selected_diff_entry() {
                    let path = self.left_root.join(&entry.relative_path);
                    self.pending_action = Some(ExternalAction::EditLeft(path));
                }
            }
            KeyCode::Char('}') => {
                if let Some(entry) = self.selected_diff_entry() {
                    let path = self.right_root.join(&entry.relative_path);
                    self.pending_action = Some(ExternalAction::EditRight(path));
                }
            }
            _ => {}
        }
    }

    /// Jumps to the next (`forward = true`) or previous entry whose status matches
    /// the discriminant of the currently selected entry (defaults to `Different`).
    fn jump_to_matching(&mut self, forward: bool) {
        let cur = self.diff_view.selected_index().unwrap_or(0);
        let target = self
            .selected_diff_entry()
            .map(|e| std::mem::discriminant(&e.status))
            .unwrap_or_else(|| std::mem::discriminant(&DiffStatus::Different));
        let entries = self
            .diff_result
            .as_ref()
            .map(|r| r.entries.as_slice())
            .unwrap_or_default();
        let idx = if forward {
            next_entry_matching(&self.view_rows, entries, cur, |e| {
                std::mem::discriminant(&e.status) == target
            })
        } else {
            prev_entry_matching(&self.view_rows, entries, cur, |e| {
                std::mem::discriminant(&e.status) == target
            })
        };
        self.diff_view.list_state.select(Some(idx));
    }

    // -----------------------------------------------------------------------
    // Rendering
    // -----------------------------------------------------------------------

    fn render(&mut self, frame: &mut ratatui::Frame) {
        let layout = AppLayout::compute(frame.area());
        self.page_height = layout.main.height as usize;

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
        let entries = self
            .diff_result
            .as_ref()
            .map(|r| r.entries.as_slice())
            .unwrap_or_default();
        self.diff_view.render(frame, layout.main, &self.view_rows, entries);

        // Status bar
        StatusBar::render(
            frame,
            layout.statusbar,
            self.diff_result.as_ref(),
            &self.comparator_name,
            &self.state,
            &self.filter,
            &self.config.tools,
            self.selected_diff_entry(),
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

fn launch_cmd(cmd_str: &str, args: &[&PathBuf]) -> Result<()> {
    let mut parts = cmd_str.split_whitespace();
    let program = parts.next().ok_or_else(|| anyhow::anyhow!("empty command"))?;
    let mut cmd = std::process::Command::new(program);
    for part in parts {
        cmd.arg(part);
    }
    for path in args {
        cmd.arg(path);
    }
    cmd.status()?;
    Ok(())
}

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
