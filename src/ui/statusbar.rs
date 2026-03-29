use crate::{
    app::{AppState, DiffFilter},
    engine::diff::DiffResult,
    ui::theme::Theme,
};
use ratatui::{
    layout::Rect,
    style::Style,
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
                        " Entries: {}  Different: {}  Left only: {}  Right only: {}  Identical: {}  | {}",
                        result.total(),
                        result.different().count(),
                        result.left_only().count(),
                        result.right_only().count(),
                        result.identical().count(),
                        comparator_name,
                    )
                } else {
                    " Ready".to_string()
                }
            }
        };

        fn key_style(active: bool) -> Style {
            if active { Theme::statusbar_key() } else { Theme::statusbar_inactive_key() }
        }

        let keys = Line::from(vec![
            Span::styled(" F5", Theme::statusbar_key()),
            Span::styled(":Scan", Theme::statusbar()),
            Span::styled("  l", key_style(filter.show_left_only)),
            Span::styled(":►", key_style(filter.show_left_only)),
            Span::styled("  r", key_style(filter.show_right_only)),
            Span::styled(":◄", key_style(filter.show_right_only)),
            Span::styled("  d", key_style(filter.show_different)),
            Span::styled(":≠", key_style(filter.show_different)),
            Span::styled("  i", key_style(filter.show_identical)),
            Span::styled(":=", key_style(filter.show_identical)),
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
