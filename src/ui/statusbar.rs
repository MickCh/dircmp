use crate::{
    app::{AppState, DiffFilter},
    engine::diff::DiffResult,
    ui::theme::Theme,
};
use ratatui::{
    layout::Rect,
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

pub struct StatusBar;

impl StatusBar {
    pub fn render(
        frame: &mut Frame,
        area: Rect,
        diff: Option<&DiffResult>,
        comparator_name: &str,
        state: &AppState,
        filter: &DiffFilter,
    ) {
        let stats = match state {
            AppState::Idle => " Press F5 to compare folders".to_string(),
            AppState::Scanning => " ⏳ Scanning…".to_string(),
            AppState::Comparing { done, total } => {
                let pct = if *total > 0 { done * 100 / total } else { 0 };
                format!(" ⏳ Comparing files… ({}/{}) {}%", done, total, pct)
            }
            AppState::Ready => {
                if let Some(result) = diff {
                    format!(
                        " Entries: {}  Different: {}  Left only: {}  Right only: {}  Identical: {}  | {} | {}",
                        result.total(),
                        result.different().count(),
                        result.left_only().count(),
                        result.right_only().count(),
                        result.identical().count(),
                        comparator_name,
                        filter.label(),
                    )
                } else {
                    " Ready".to_string()
                }
            }
        };

        let keys = Line::from(vec![
            Span::styled(" F5", Theme::statusbar_key()),
            Span::styled(":Compare", Theme::statusbar()),
            Span::styled("  F", Theme::statusbar_key()),
            Span::styled(":Filter", Theme::statusbar()),
            Span::styled("  ↑↓", Theme::statusbar_key()),
            Span::styled(":Navigate", Theme::statusbar()),
            Span::styled("  Home/End", Theme::statusbar_key()),
            Span::styled(":Jump", Theme::statusbar()),
            Span::styled("  q", Theme::statusbar_key()),
            Span::styled(":Quit ", Theme::statusbar()),
        ]);

        let content = Line::from(vec![Span::styled(stats, Theme::statusbar())]);
        let paragraph = Paragraph::new(vec![content, keys]).style(Theme::statusbar());
        frame.render_widget(paragraph, area);
    }
}
