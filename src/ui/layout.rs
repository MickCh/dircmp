use ratatui::layout::{Constraint, Direction, Layout, Rect};

pub struct AppLayout {
    /// One-line header showing left and right root paths.
    pub header: Rect,
    /// Main content area — the unified diff list.
    pub main: Rect,
    /// Two-line status bar at the bottom.
    pub statusbar: Rect,
}

impl AppLayout {
    pub fn compute(area: Rect) -> Self {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Min(0),
                Constraint::Length(2),
            ])
            .split(area);

        Self {
            header: chunks[0],
            main: chunks[1],
            statusbar: chunks[2],
        }
    }
}
