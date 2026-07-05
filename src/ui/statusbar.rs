use crate::{
    config::ToolsConfig,
    engine::diff::{DiffEntry, DiffFilter, DiffResult, DiffStatus},
    ui::{theme::Theme, AppState},
};
use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

/// Everything the status bar needs to render one frame, borrowed from `App`.
pub struct StatusBarContext<'a> {
    pub diff: Option<&'a DiffResult>,
    pub comparator_name: &'a str,
    pub state: &'a AppState,
    pub filter: &'a DiffFilter,
    pub tools: &'a ToolsConfig,
    pub selected: Option<&'a DiffEntry>,
    pub scan_errors: usize,
    pub status_message: Option<&'a str>,
}

pub struct StatusBar;

impl StatusBar {
    pub fn render(frame: &mut Frame, area: Rect, ctx: &StatusBarContext) {
        let stats = if let Some(msg) = ctx.status_message {
            format!(" ⚠ {msg}")
        } else {
            match ctx.state {
                AppState::Idle => " Press F5 to compare folders".to_string(),
                AppState::Scanning => " ⏳ Scanning…".to_string(),
                AppState::Comparing { done, total } => {
                    let pct = if *total > 0 { done * 100 / total } else { 0 };
                    format!(" ⏳ Comparing files… ({}/{}) {}%", done, total, pct)
                }
                AppState::Ready => {
                    if let Some(result) = ctx.diff {
                        let errors_str = if ctx.scan_errors > 0 {
                            format!("  ⚠ {} scan errors", ctx.scan_errors)
                        } else {
                            String::new()
                        };
                        format!(
                            " Entries: {}  Different: {}  Left only: {}  Right only: {}  Identical: {}  | {}{}",
                            result.total(),
                            result.count_different(),
                            result.count_left_only(),
                            result.count_right_only(),
                            result.count_identical(),
                            ctx.comparator_name,
                            errors_str,
                        )
                    } else {
                        " Ready".to_string()
                    }
                }
            }
        };

        // Compute per-action availability based on selected entry.
        let has_left = matches!(
            ctx.selected.map(|e| &e.status),
            Some(
                DiffStatus::LeftOnly
                    | DiffStatus::Different
                    | DiffStatus::Identical
                    | DiffStatus::TypeConflict
                    | DiffStatus::Error(_)
            )
        );
        let has_right = matches!(
            ctx.selected.map(|e| &e.status),
            Some(
                DiffStatus::RightOnly
                    | DiffStatus::Different
                    | DiffStatus::Identical
                    | DiffStatus::TypeConflict
                    | DiffStatus::Error(_)
            )
        );
        let can_enter = match ctx.selected.map(|e| &e.status) {
            Some(DiffStatus::Different) => ctx.tools.diff_tool.is_some(),
            Some(DiffStatus::LeftOnly | DiffStatus::RightOnly) => ctx.tools.viewer.is_some(),
            _ => false,
        };
        fn key(active: bool) -> Style {
            if active { Theme::statusbar_key() } else { Theme::statusbar_inactive_key() }
        }
        fn label(active: bool) -> Style {
            if active { Theme::statusbar() } else { Theme::statusbar_inactive_key() }
        }

        let filter = ctx.filter;
        let mut key_spans = vec![
            Span::styled(" F5", Theme::statusbar_key()),
            Span::styled(":Scan", Theme::statusbar()),
            Span::styled("  l", key(filter.show_left_only)),
            Span::styled(":►", key(filter.show_left_only)),
            Span::styled("  r", key(filter.show_right_only)),
            Span::styled(":◄", key(filter.show_right_only)),
            Span::styled("  d", key(filter.show_different)),
            Span::styled(":≠", key(filter.show_different)),
            Span::styled("  i", key(filter.show_identical)),
            Span::styled(":=", key(filter.show_identical)),
            Span::styled("  ↑↓", Theme::statusbar_key()),
            Span::styled(":Navigate", Theme::statusbar()),
            Span::styled("  n/N", Theme::statusbar_key()),
            Span::styled(":Next/Prev", Theme::statusbar()),
            Span::styled("  Home/End", Theme::statusbar_key()),
            Span::styled(":Jump", Theme::statusbar()),
        ];

        if ctx.tools.diff_tool.is_some() {
            key_spans.push(Span::styled("  Enter", key(can_enter)));
            key_spans.push(Span::styled(":Diff", label(can_enter)));
        }

        if ctx.tools.viewer.is_some() {
            key_spans.push(Span::styled("  [", key(has_left)));
            key_spans.push(Span::styled("/", Theme::statusbar_inactive_key()));
            key_spans.push(Span::styled("]", key(has_right)));
            key_spans.push(Span::styled(":View", label(has_left || has_right)));
        }

        if ctx.tools.editor.is_some() {
            key_spans.push(Span::styled("  {", key(has_left)));
            key_spans.push(Span::styled("/", Theme::statusbar_inactive_key()));
            key_spans.push(Span::styled("}", key(has_right)));
            key_spans.push(Span::styled(":Edit", label(has_left || has_right)));
        }

        key_spans.push(Span::styled("  q", Theme::statusbar_key()));
        key_spans.push(Span::styled(":Quit ", Theme::statusbar()));

        let keys = Line::from(key_spans);
        let content = Line::from(vec![Span::styled(stats, Theme::statusbar())]);
        let paragraph = Paragraph::new(vec![content, keys]).style(Theme::statusbar());
        frame.render_widget(paragraph, area);
    }
}
