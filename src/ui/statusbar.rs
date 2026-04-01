use crate::{
    app::{AppState, DiffFilter},
    config::ToolsConfig,
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
        tools: &ToolsConfig,
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

        let mut key_spans = vec![
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
        ];
        if tools.diff_tool.is_some() {
            key_spans.push(Span::styled("  Enter", Theme::statusbar_key()));
            key_spans.push(Span::styled(":Diff", Theme::statusbar()));
        }
        if tools.viewer.is_some() {
            key_spans.push(Span::styled("  v/V", Theme::statusbar_key()));
            key_spans.push(Span::styled(":View", Theme::statusbar()));
        }
        if tools.editor.is_some() {
            key_spans.push(Span::styled("  e/E", Theme::statusbar_key()));
            key_spans.push(Span::styled(":Edit", Theme::statusbar()));
        }
        key_spans.push(Span::styled("  q", Theme::statusbar_key()));
        key_spans.push(Span::styled(":Quit ", Theme::statusbar()));
        let keys = Line::from(key_spans);

        let content = Line::from(vec![Span::styled(stats, Theme::statusbar())]);
        let paragraph = Paragraph::new(vec![content, keys]).style(Theme::statusbar());
        frame.render_widget(paragraph, area);
    }
}
