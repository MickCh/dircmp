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
            AppState::Idle => " Naciśnij F5 aby porównać foldery".to_string(),
            AppState::Scanning => " ⏳ Skanowanie…".to_string(),
            AppState::Comparing => " ⏳ Porównywanie plików…".to_string(),
            AppState::Ready => {
                if let Some(result) = diff {
                    format!(
                        " Wpisy: {}  Różne: {}  Lewe: {}  Prawe: {}  Identyczne: {}  | {} | {}",
                        result.total(),
                        result.different().count(),
                        result.left_only().count(),
                        result.right_only().count(),
                        result.identical().count(),
                        comparator_name,
                        filter.label(),
                    )
                } else {
                    " Gotowy".to_string()
                }
            }
        };

        let keys = Line::from(vec![
            Span::styled(" F5", Theme::statusbar_key()),
            Span::styled(":Porównaj", Theme::statusbar()),
            Span::styled("  F", Theme::statusbar_key()),
            Span::styled(":Filtr", Theme::statusbar()),
            Span::styled("  ↑↓", Theme::statusbar_key()),
            Span::styled(":Nawigacja", Theme::statusbar()),
            Span::styled("  Home/End", Theme::statusbar_key()),
            Span::styled(":Skocz", Theme::statusbar()),
            Span::styled("  q", Theme::statusbar_key()),
            Span::styled(":Wyjście ", Theme::statusbar()),
        ]);

        let content = Line::from(vec![Span::styled(stats, Theme::statusbar())]);
        let paragraph = Paragraph::new(vec![content, keys]).style(Theme::statusbar());
        frame.render_widget(paragraph, area);
    }
}
