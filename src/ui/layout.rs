use ratatui::layout::{Constraint, Direction, Layout, Rect};

pub struct AppLayout {
    pub left_panel: Rect,
    pub right_panel: Rect,
    pub statusbar: Rect,
}

impl AppLayout {
    pub fn compute(area: Rect) -> Self {
        let vertical = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(0),
                Constraint::Length(2),
            ])
            .split(area);

        let panels = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(50),
                Constraint::Percentage(50),
            ])
            .split(vertical[0]);

        Self {
            left_panel: panels[0],
            right_panel: panels[1],
            statusbar: vertical[1],
        }
    }
}
