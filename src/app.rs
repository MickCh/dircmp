use crate::{
    config::Config,
    engine::{
        comparator::create_comparator,
        diff::{DiffEngine, DiffEntry},
        scanner::Scanner,
    },
    ui::{
        layout::AppLayout,
        panel::{Panel, PanelSide},
        statusbar::StatusBar,
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

/// Aktywny filtr wyświetlanych wpisów.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffFilter {
    /// Pokaż wszystko.
    All,
    /// Tylko wpisy z różnicami (LeftOnly, RightOnly, Different, TypeConflict, Error).
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
    Error(String),
}

pub struct App {
    config: Config,
    left_panel: Panel,
    right_panel: Panel,
    active_panel: PanelSide,
    diff_result: Option<crate::engine::diff::DiffResult>,
    /// Przefiltrowane wpisy – aktualizowane po każdej zmianie filtru/wyników.
    filtered_entries: Vec<DiffEntry>,
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
            left_panel: Panel::new(PanelSide::Left, left_path),
            right_panel: Panel::new(PanelSide::Right, right_path),
            active_panel: PanelSide::Left,
            diff_result: None,
            filtered_entries: Vec::new(),
            filter: DiffFilter::All,
            comparator_name,
            should_quit: false,
            state: AppState::Idle,
        }
    }

    /// Odświeża `filtered_entries` na podstawie aktualnego wyniku i filtra.
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

        // Resetuj zaznaczenie żeby nie wyjść poza zakres
        let max = self.filtered_entries.len().saturating_sub(1);
        let clamp = |idx: usize| idx.min(max);
        let li = self.left_panel.selected_index().map(clamp).unwrap_or(0);
        let ri = self.right_panel.selected_index().map(clamp).unwrap_or(0);
        self.left_panel.list_state.select(Some(li));
        self.right_panel.list_state.select(Some(ri));
    }

    /// Uruchamia porównanie folderów. Błędy są przechwytywane i wyświetlane w UI.
    pub fn run_diff(&mut self) {
        self.state = AppState::Scanning;

        let scanner = Scanner::new(self.config.scan.clone());
        let left_map = scanner.scan(&self.left_panel.root.clone());
        let right_map = scanner.scan(&self.right_panel.root.clone());

        let comparator = create_comparator(&self.config.comparison);
        let engine = DiffEngine::new(comparator.as_ref());

        let result = engine.diff(
            &left_map,
            &right_map,
            &self.left_panel.root.clone(),
            &self.right_panel.root.clone(),
        );

        self.diff_result = Some(result);
        self.rebuild_filtered();
        self.state = AppState::Ready;
    }

    /// Główna pętla aplikacji.
    pub fn run<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
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
        // Jeśli jesteśmy w stanie błędu, dowolny klawisz go kasuje
        if matches!(self.state, AppState::Error(_)) {
            self.state = AppState::Ready;
            return;
        }

        // Kopiujemy potrzebne wartości zanim pożyczymy self mutowalnie
        let entry_count = self.filtered_entries.len();
        let step = self.config.ui.panel_scroll_step;

        match code {
            KeyCode::Char('q') | KeyCode::Char('Q') => {
                self.should_quit = true;
            }
            KeyCode::F(5) => {
                self.run_diff();
            }
            KeyCode::Tab => {
                self.active_panel = match self.active_panel {
                    PanelSide::Left => PanelSide::Right,
                    PanelSide::Right => PanelSide::Left,
                };
            }
            KeyCode::Char('f') | KeyCode::Char('F') => {
                self.filter = self.filter.toggle();
                self.rebuild_filtered();
            }
            KeyCode::Down => {
                let max = entry_count.saturating_sub(1);
                match self.active_panel {
                    PanelSide::Left => {
                        let cur = self.left_panel.selected_index().unwrap_or(0);
                        self.left_panel.list_state.select(Some((cur + 1).min(max)));
                    }
                    PanelSide::Right => {
                        let cur = self.right_panel.selected_index().unwrap_or(0);
                        self.right_panel.list_state.select(Some((cur + 1).min(max)));
                    }
                }
            }
            KeyCode::Up => {
                match self.active_panel {
                    PanelSide::Left => {
                        let cur = self.left_panel.selected_index().unwrap_or(0);
                        self.left_panel.list_state.select(Some(cur.saturating_sub(1)));
                    }
                    PanelSide::Right => {
                        let cur = self.right_panel.selected_index().unwrap_or(0);
                        self.right_panel.list_state.select(Some(cur.saturating_sub(1)));
                    }
                }
            }
            KeyCode::PageDown => {
                let max = entry_count.saturating_sub(1);
                match self.active_panel {
                    PanelSide::Left => {
                        let cur = self.left_panel.selected_index().unwrap_or(0);
                        self.left_panel.list_state.select(Some((cur + step).min(max)));
                    }
                    PanelSide::Right => {
                        let cur = self.right_panel.selected_index().unwrap_or(0);
                        self.right_panel.list_state.select(Some((cur + step).min(max)));
                    }
                }
            }
            KeyCode::PageUp => {
                match self.active_panel {
                    PanelSide::Left => {
                        let cur = self.left_panel.selected_index().unwrap_or(0);
                        self.left_panel.list_state.select(Some(cur.saturating_sub(step)));
                    }
                    PanelSide::Right => {
                        let cur = self.right_panel.selected_index().unwrap_or(0);
                        self.right_panel.list_state.select(Some(cur.saturating_sub(step)));
                    }
                }
            }
            KeyCode::Home => {
                match self.active_panel {
                    PanelSide::Left => self.left_panel.list_state.select(Some(0)),
                    PanelSide::Right => self.right_panel.list_state.select(Some(0)),
                }
            }
            KeyCode::End => {
                let last = entry_count.saturating_sub(1);
                match self.active_panel {
                    PanelSide::Left => self.left_panel.list_state.select(Some(last)),
                    PanelSide::Right => self.right_panel.list_state.select(Some(last)),
                }
            }
            _ => {}
        }
    }

    fn render(&mut self, frame: &mut ratatui::Frame) {
        let layout = AppLayout::compute(frame.area());

        // Synchronizuj zaznaczenie między panelami
        match self.active_panel {
            PanelSide::Left => {
                if let Some(idx) = self.left_panel.selected_index() {
                    self.right_panel.list_state.select(Some(idx));
                }
            }
            PanelSide::Right => {
                if let Some(idx) = self.right_panel.selected_index() {
                    self.left_panel.list_state.select(Some(idx));
                }
            }
        }

        let entries = self.filtered_entries.as_slice();

        self.left_panel.render(
            frame,
            layout.left_panel,
            entries,
            self.active_panel == PanelSide::Left,
        );

        self.right_panel.render(
            frame,
            layout.right_panel,
            entries,
            self.active_panel == PanelSide::Right,
        );

        StatusBar::render(
            frame,
            layout.statusbar,
            self.diff_result.as_ref(),
            &self.comparator_name,
            &self.state,
            &self.filter,
        );

        // Nakładka na środku ekranu dla stanów specjalnych
        match &self.state {
            AppState::Scanning => {
                render_overlay(frame, " ⏳ Skanowanie folderów… ");
            }
            AppState::Error(msg) => {
                let msg = msg.clone();
                render_overlay(frame, &format!(" ✗ {}  [dowolny klawisz] ", msg));
            }
            AppState::Idle => {
                render_overlay(frame, " Naciśnij F5 aby rozpocząć porównanie ");
            }
            AppState::Ready => {}
        }
    }
}
