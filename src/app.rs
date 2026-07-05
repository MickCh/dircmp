use crate::{
    config::Config,
    engine::{
        comparator::create_comparator,
        diff::{DiffEngine, DiffEntry, DiffFilter, DiffResult, DiffStatus},
        pipeline::{self, CompareMsg, ScanMsg},
    },
    ui::{
        layout::AppLayout,
        panel::{DiffView, ViewRow, ViewRows},
        statusbar::{StatusBar, StatusBarContext},
        theme::Theme,
        AppState,
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
use std::{path::PathBuf, sync::{mpsc, Arc}};

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
// Commands
// ---------------------------------------------------------------------------

/// A user command decoded from a key press. Keeping the key map a pure
/// `KeyCode -> Command` function makes it unit-testable without a terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Command {
    Quit,
    Rescan,
    ToggleLeftOnly,
    ToggleRightOnly,
    ToggleDifferent,
    ToggleIdentical,
    JumpNextMatching,
    JumpPrevMatching,
    CursorDown,
    CursorUp,
    PageDown,
    PageUp,
    CursorHome,
    CursorEnd,
    /// Smart open: Different → diff tool, LeftOnly/RightOnly → viewer.
    Open,
    ViewLeft,
    ViewRight,
    EditLeft,
    EditRight,
}

fn command_for_key(code: KeyCode) -> Option<Command> {
    Some(match code {
        KeyCode::Char('q') | KeyCode::Char('Q') => Command::Quit,
        KeyCode::F(5) => Command::Rescan,
        KeyCode::Char('l') | KeyCode::Char('L') => Command::ToggleLeftOnly,
        KeyCode::Char('r') | KeyCode::Char('R') => Command::ToggleRightOnly,
        KeyCode::Char('d') | KeyCode::Char('D') => Command::ToggleDifferent,
        KeyCode::Char('i') | KeyCode::Char('I') => Command::ToggleIdentical,
        KeyCode::Char('n') => Command::JumpNextMatching,
        KeyCode::Char('N') => Command::JumpPrevMatching,
        KeyCode::Down => Command::CursorDown,
        KeyCode::Up => Command::CursorUp,
        KeyCode::PageDown => Command::PageDown,
        KeyCode::PageUp => Command::PageUp,
        KeyCode::Home => Command::CursorHome,
        KeyCode::End => Command::CursorEnd,
        KeyCode::Enter => Command::Open,
        KeyCode::Char('[') => Command::ViewLeft,
        KeyCode::Char(']') => Command::ViewRight,
        KeyCode::Char('{') => Command::EditLeft,
        KeyCode::Char('}') => Command::EditRight,
        _ => return None,
    })
}

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------

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
            state: AppState::Idle,
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
    // Diff phases
    // -----------------------------------------------------------------------

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
    /// diff structure and kicks off phase-2 comparison. Returns `true` if the
    /// scan just completed (view needs a full rebuild).
    fn poll_scan(&mut self) -> bool {
        let msg = match &self.scan_rx {
            Some(rx) => match rx.try_recv() {
                Ok(m) => m,
                Err(mpsc::TryRecvError::Empty) => return false,
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.scan_rx = None;
                    self.state = AppState::Ready;
                    self.status_message = Some("Scan thread crashed unexpectedly".to_string());
                    return false;
                }
            },
            None => return false,
        };

        let ScanMsg::Done { left_map, right_map, errors } = msg;
        self.scan_rx = None;
        self.scan_errors = errors;
        let result = DiffEngine::diff_structure(&left_map, &right_map);
        self.diff_result = Some(result);
        self.rebuild_filtered();
        self.start_background_comparison();
        true
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

    /// Full re-scan triggered by F5.
    pub fn run_diff(&mut self) {
        self.state = AppState::Scanning;
        self.start_background_scan();
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
                            result.update_entry_status(idx, status);
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

    // -----------------------------------------------------------------------
    // Filtered view
    // -----------------------------------------------------------------------

    fn rebuild_filtered(&mut self) {
        let old_idx = self.diff_view.selected_index();

        self.view_rows = if let Some(result) = &self.diff_result {
            let filter = &self.filter;
            ViewRows::build(&result.entries, |e| filter.matches(e))
        } else {
            ViewRows::default()
        };

        // Preserve cursor position, clamped to valid range.
        // Guard: if all entries are filtered out, deselect and bail — idx=0 on an
        // empty view_rows would be an out-of-bounds selection.
        let idx = if self.view_rows.is_empty() {
            self.diff_view.list_state.select(None);
            return;
        } else {
            old_idx
                .map(|i| i.min(self.view_rows.len().saturating_sub(1)))
                .unwrap_or_else(|| self.view_rows.first())
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
        &mut self,
        terminal: &mut Terminal<B>,
        action: ExternalAction,
    ) -> Result<()>
    where
        B::Error: Send + Sync + 'static,
    {
        suspend_tui(terminal)?;
        let result = self.run_external_command(&action);
        resume_tui(terminal)?;
        self.status_message = result?;
        Ok(())
    }

    fn run_external_command(&self, action: &ExternalAction) -> Result<Option<String>> {
        let tools = &self.config.tools;
        let msg = match action {
            ExternalAction::Diff { left, right } => {
                if let Some(cmd) = &tools.diff_tool {
                    launch_cmd(cmd, &[left, right])?
                } else {
                    None
                }
            }
            ExternalAction::ViewLeft(p) | ExternalAction::ViewRight(p) => {
                if let Some(cmd) = &tools.viewer {
                    launch_cmd(cmd, &[p])?
                } else {
                    None
                }
            }
            ExternalAction::EditLeft(p) | ExternalAction::EditRight(p) => {
                if let Some(cmd) = &tools.editor {
                    launch_cmd(cmd, &[p])?
                } else {
                    None
                }
            }
        };
        Ok(msg)
    }

    // -----------------------------------------------------------------------
    // Main loop
    // -----------------------------------------------------------------------

    pub fn run<B: Backend + std::io::Write>(&mut self, terminal: &mut Terminal<B>) -> Result<()>
    where
        B::Error: Send + Sync + 'static,
    {
        self.state = AppState::Scanning;
        self.start_background_scan();

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
                if let Some(cmd) = command_for_key(key.code) {
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
    // Command handling
    // -----------------------------------------------------------------------

    fn apply(&mut self, cmd: Command) {
        match cmd {
            Command::Quit => self.should_quit = true,
            Command::Rescan => self.run_diff(),
            Command::ToggleLeftOnly => {
                self.filter.show_left_only = !self.filter.show_left_only;
                self.rebuild_filtered();
            }
            Command::ToggleRightOnly => {
                self.filter.show_right_only = !self.filter.show_right_only;
                self.rebuild_filtered();
            }
            Command::ToggleDifferent => {
                self.filter.show_different = !self.filter.show_different;
                self.rebuild_filtered();
            }
            Command::ToggleIdentical => {
                self.filter.show_identical = !self.filter.show_identical;
                self.rebuild_filtered();
            }
            Command::JumpNextMatching => self.jump_to_matching(true),
            Command::JumpPrevMatching => self.jump_to_matching(false),
            Command::CursorDown => {
                let cur = self.diff_view.selected_index().unwrap_or(0);
                self.diff_view.list_state.select(Some(self.view_rows.next(cur)));
            }
            Command::CursorUp => {
                let cur = self.diff_view.selected_index().unwrap_or(0);
                self.diff_view.list_state.select(Some(self.view_rows.prev(cur)));
            }
            Command::PageDown => {
                let cur = self.diff_view.selected_index().unwrap_or(0);
                self.diff_view
                    .list_state
                    .select(Some(self.view_rows.nth_next(cur, self.page_height)));
            }
            Command::PageUp => {
                let cur = self.diff_view.selected_index().unwrap_or(0);
                self.diff_view
                    .list_state
                    .select(Some(self.view_rows.nth_prev(cur, self.page_height)));
            }
            Command::CursorHome => {
                self.diff_view.list_state.select(Some(self.view_rows.first()));
            }
            Command::CursorEnd => {
                self.diff_view.list_state.select(Some(self.view_rows.last()));
            }
            Command::Open => {
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
            Command::ViewLeft => self.queue_side_action(Side::Left, ExternalAction::ViewLeft),
            Command::ViewRight => self.queue_side_action(Side::Right, ExternalAction::ViewRight),
            Command::EditLeft => self.queue_side_action(Side::Left, ExternalAction::EditLeft),
            Command::EditRight => self.queue_side_action(Side::Right, ExternalAction::EditRight),
        }
    }

    /// Queues an external action on the selected entry's left or right path.
    fn queue_side_action(&mut self, side: Side, make: fn(PathBuf) -> ExternalAction) {
        let Some(entry) = self.selected_diff_entry() else { return };
        let root = match side {
            Side::Left => &self.left_root,
            Side::Right => &self.right_root,
        };
        let path = root.join(&entry.relative_path);
        self.pending_action = Some(make(path));
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
            self.view_rows.next_matching(entries, cur, |e| {
                std::mem::discriminant(&e.status) == target
            })
        } else {
            self.view_rows.prev_matching(entries, cur, |e| {
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
            &StatusBarContext {
                diff: self.diff_result.as_ref(),
                comparator_name: &self.comparator_name,
                state: &self.state,
                filter: &self.filter,
                tools: &self.config.tools,
                selected: self.selected_diff_entry(),
                scan_errors: self.scan_errors,
                status_message: self.status_message.as_deref(),
            },
        );

        // Overlay for transient states
        match &self.state {
            AppState::Scanning => render_overlay(frame, " ⏳ Scanning folders… "),
            AppState::Idle => render_overlay(frame, " Press F5 to start comparison "),
            AppState::Comparing { .. } | AppState::Ready => {}
        }
    }
}

/// Which side of the comparison a path-based action targets.
#[derive(Clone, Copy)]
enum Side {
    Left,
    Right,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn suspend_tui<B: Backend + std::io::Write>(terminal: &mut Terminal<B>) -> Result<()>
where
    B::Error: Send + Sync + 'static,
{
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    Ok(())
}

fn resume_tui<B: Backend + std::io::Write>(terminal: &mut Terminal<B>) -> Result<()>
where
    B::Error: Send + Sync + 'static,
{
    enable_raw_mode()?;
    execute!(terminal.backend_mut(), EnterAlternateScreen)?;
    terminal.clear()?;
    Ok(())
}

fn launch_cmd(tool: &crate::config::ToolCommand, args: &[&PathBuf]) -> Result<Option<String>> {
    let (program, pre_args) = tool.program_and_args();
    if program.is_empty() {
        return Err(anyhow::anyhow!("empty command"));
    }
    let mut cmd = std::process::Command::new(program);
    for arg in pre_args {
        cmd.arg(arg);
    }
    for path in args {
        cmd.arg(path);
    }
    let status = cmd.status()?;
    if status.success() {
        Ok(None)
    } else {
        Ok(Some(format!("Tool exited with {status}")))
    }
}

fn render_overlay(frame: &mut ratatui::Frame, message: &str) {
    let area = frame.area();
    if area.width == 0 || area.height == 0 {
        return;
    }
    let msg_len = message.len() as u16;
    let width = (msg_len.min(area.width.saturating_sub(4)) + 4).min(area.width);
    let height = 3u16.min(area.height);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letter_keys_map_case_insensitively() {
        for (lower, upper, expected) in [
            ('q', 'Q', Command::Quit),
            ('l', 'L', Command::ToggleLeftOnly),
            ('r', 'R', Command::ToggleRightOnly),
            ('d', 'D', Command::ToggleDifferent),
            ('i', 'I', Command::ToggleIdentical),
        ] {
            assert_eq!(command_for_key(KeyCode::Char(lower)), Some(expected));
            assert_eq!(command_for_key(KeyCode::Char(upper)), Some(expected));
        }
    }

    #[test]
    fn jump_keys_are_case_sensitive() {
        assert_eq!(command_for_key(KeyCode::Char('n')), Some(Command::JumpNextMatching));
        assert_eq!(command_for_key(KeyCode::Char('N')), Some(Command::JumpPrevMatching));
    }

    #[test]
    fn navigation_and_tool_keys_map() {
        assert_eq!(command_for_key(KeyCode::F(5)), Some(Command::Rescan));
        assert_eq!(command_for_key(KeyCode::Down), Some(Command::CursorDown));
        assert_eq!(command_for_key(KeyCode::Up), Some(Command::CursorUp));
        assert_eq!(command_for_key(KeyCode::PageDown), Some(Command::PageDown));
        assert_eq!(command_for_key(KeyCode::PageUp), Some(Command::PageUp));
        assert_eq!(command_for_key(KeyCode::Home), Some(Command::CursorHome));
        assert_eq!(command_for_key(KeyCode::End), Some(Command::CursorEnd));
        assert_eq!(command_for_key(KeyCode::Enter), Some(Command::Open));
        assert_eq!(command_for_key(KeyCode::Char('[')), Some(Command::ViewLeft));
        assert_eq!(command_for_key(KeyCode::Char(']')), Some(Command::ViewRight));
        assert_eq!(command_for_key(KeyCode::Char('{')), Some(Command::EditLeft));
        assert_eq!(command_for_key(KeyCode::Char('}')), Some(Command::EditRight));
    }

    #[test]
    fn unknown_keys_map_to_none() {
        assert_eq!(command_for_key(KeyCode::Char('x')), None);
        assert_eq!(command_for_key(KeyCode::Esc), None);
        assert_eq!(command_for_key(KeyCode::F(1)), None);
    }
}
