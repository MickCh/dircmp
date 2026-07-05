use crate::ui::theme::Theme;
use ratatui::{
    layout::{Alignment, Rect},
    text::Line,
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

/// Renders a small centered message box on top of the current frame
/// (used for transient states: scanning, idle hint).
pub fn render(frame: &mut Frame, message: &str) {
    let area = frame.area();
    if area.width == 0 || area.height == 0 {
        return;
    }
    let msg_len = message.len() as u16;
    let width = (msg_len.min(area.width.saturating_sub(4)) + 4).min(area.width);
    let height = 3u16.min(area.height);
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    let popup_area = Rect::new(x, y, width, height);

    frame.render_widget(Clear, popup_area);
    let block = Block::default().borders(Borders::ALL).style(Theme::overlay());
    let paragraph = Paragraph::new(Line::from(message.to_string()))
        .block(block)
        .alignment(Alignment::Center);
    frame.render_widget(paragraph, popup_area);
}
