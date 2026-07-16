//! Per-frame drawing: paths header, diff list, status bar and overlays.

use super::App;
use crate::ui::{
    layout::AppLayout,
    overlay,
    panel::fit,
    statusbar::{StatusBar, StatusBarContext},
    theme::Theme,
    AppState,
};
use ratatui::widgets::Paragraph;

impl App {
    pub(super) fn render(&mut self, frame: &mut ratatui::Frame) {
        let layout = AppLayout::compute(frame.area());
        self.page_height = layout.main.height as usize;

        // Paths header — both sides truncated to their column so a long left
        // path cannot push the right one out of alignment.
        let half = (layout.header.width as usize).saturating_sub(3) / 2;
        let header_text = format!(
            " {}  {}",
            fit(&self.left_root.display().to_string(), half),
            fit(&self.right_root.display().to_string(), half),
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
                tool_keys: self.tool_key_hints(),
                scan_errors: self.scan_errors,
                status_message: self.status_message.as_deref(),
            },
        );

        // Overlay for transient states
        match &self.state {
            AppState::Scanning => overlay::render(frame, " ⏳ Scanning folders… "),
            AppState::Comparing { .. } | AppState::Ready => {}
        }
    }
}
