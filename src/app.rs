use crate::{
    config::Config,
    engine::{
        comparator::create_comparator,
        diff::{DiffEngine, DiffEntry},
        scanner::Scanner,
    },
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
use std::path::PathBuf;

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

/// Active display filter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffFilter {
    All,
    DifferencesOnly,
}

impl DiffFilter {
    pub fn matches(&self, entry: &DiffEntry) -> bool {
        match self {
            DiffFilter::All => true,
            DiffFilter::DifferencesOnly => entry.status.has_difference(),
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            DiffFilter::All => "Wszystkie",
            DiffFilter::DifferencesOnly => "Tylko różnice",
        }
    }

    pub fn toggle(self) -> Self {
        match self {
            DiffFilter::All => DiffFilter::DifferencesOnly,
            DiffFilter::DifferencesOnly => DiffFilter::All,
        }
    }
}

pub enum AppState {
    Idle,
    Scanning,
    Ready,
}

pub struct App {
    config: Config,
    left_root: PathBuf,
    right_root: PathBuf,
    diff_view: DiffView,
    diff_result: Option<crate::engine::diff::DiffResult>,
    /// Filtered entries – rebuilt whenever filter or results change.
    filtered_entries: Vec<DiffEntry>,
    /// View rows derived from filtered_entries (includes folder headers).
    view_rows: Vec<ViewRow>,
    filter: DiffFilter,
    comparator_name: String,
    should_quit: bool,
    state: AppState,
}

impl App {
    pub fn new(left_path: PathBuf, right_path: PathBuf, config: Config) -> Self {
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
        }
    }

    /// Rebuilds `filtered_entries` and `view_rows` from current diff result and filter.
    fn rebuild_filtered(&mut self) {
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

        let idx = first_entry(&self.view_rows);
        self.diff_view
            .list_state
            .select(if self.view_rows.is_empty() { None } else { Some(idx) });
    }

    /// Runs the folder comparison and updates state.
    pub fn run_diff(&mut self) {
        self.state = AppState::Scanning;

        let scanner = Scanner::new(self.config.scan.clone());
        let left_map = scanner.scan(&self.left_root.clone());
        let right_map = scanner.scan(&self.right_root.clone());

        let comparator = create_comparator(&self.config.comparison);
        let engine = DiffEngine::new(comparator.as_ref());

        let result = engine.diff(&left_map, &right_map, &self.left_root, &self.right_root);

        self.diff_result = Some(result);
        self.rebuild_filtered();
        self.state = AppState::Ready;
    }

    /// Main application loop. Starts comparison immediately on launch.
    pub fn run<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
        self.run_diff();

        loop {
            terminal.draw(|frame| self.render(frame))?;

            if event::poll(std::time::Duration::from_millis(100))? {
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
                let idx = first_entry(&self.view_rows);
                self.diff_view.list_state.select(Some(idx));
            }
            KeyCode::End => {
                let idx = last_entry(&self.view_rows);
                self.diff_view.list_state.select(Some(idx));
            }
            _ => {}
        }
    }

    fn render(&mut self, frame: &mut ratatui::Frame) {
        let layout = AppLayout::compute(frame.area());

        // Paths header
        let half = (layout.header.width as usize).saturating_sub(2) / 2;
        let header_text = format!(
            " {:<half$}  {}",
            self.left_root.display(),
            self.right_root.display(),
            half = half
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
            AppState::Scanning => render_overlay(frame, " ⏳ Skanowanie folderów… "),
            AppState::Idle => render_overlay(frame, " Naciśnij F5 aby rozpocząć porównanie "),
            AppState::Ready => {}
        }
    }
}
