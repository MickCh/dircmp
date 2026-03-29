use crate::{
    engine::diff::{DiffEntry, DiffStatus},
    ui::theme::Theme,
};
use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState},
    Frame,
};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelSide {
    Left,
    Right,
}

pub struct Panel {
    pub side: PanelSide,
    pub root: PathBuf,
    pub list_state: ListState,
}

impl Panel {
    pub fn new(side: PanelSide, root: PathBuf) -> Self {
        let mut list_state = ListState::default();
        list_state.select(Some(0));
        Self { side, root, list_state }
    }

    pub fn selected_index(&self) -> Option<usize> {
        self.list_state.selected()
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect, entries: &[DiffEntry], is_active: bool) {
        let border_style = if is_active {
            Theme::panel_border_active()
        } else {
            Theme::panel_border_inactive()
        };

        let title = format!(
            " {} ",
            self.root.display()
        );

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(Span::styled(title, Theme::panel_title()));

        let items: Vec<ListItem> = entries
            .iter()
            .map(|entry| self.make_list_item(entry))
            .collect();

        let list = List::new(items)
            .block(block)
            .highlight_style(Theme::selected());

        frame.render_stateful_widget(list, area, &mut self.list_state);
    }

    fn make_list_item(&self, entry: &DiffEntry) -> ListItem<'static> {
        let (style, prefix) = self.entry_style_and_prefix(entry);

        let name = entry.relative_path.display().to_string();
        let dir_indicator = if entry.is_dir { "/" } else { "" };

        let size_str = match self.side {
            PanelSide::Left => entry
                .size_left
                .map(format_size)
                .unwrap_or_else(|| "---".to_string()),
            PanelSide::Right => entry
                .size_right
                .map(format_size)
                .unwrap_or_else(|| "---".to_string()),
        };

        // Budujemy dwa spany: prefiks+nazwa i rozmiar, żeby wyrównać do szerokości panelu
        let name_part = format!("{} {}{}", prefix, name, dir_indicator);
        let size_part = format!(" {:>10}", size_str);

        let name_style = if entry.is_dir { Theme::dir() } else { style };

        ListItem::new(Line::from(vec![
            Span::styled(name_part, name_style),
            Span::styled(size_part, Theme::identical()),
        ]))
    }

    fn entry_style_and_prefix(&self, entry: &DiffEntry) -> (Style, &'static str) {
        match &entry.status {
            DiffStatus::LeftOnly => match self.side {
                PanelSide::Left => (Theme::left_only(), "◄"),
                PanelSide::Right => (Theme::left_only(), " "),
            },
            DiffStatus::RightOnly => match self.side {
                PanelSide::Left => (Theme::right_only(), " "),
                PanelSide::Right => (Theme::right_only(), "►"),
            },
            DiffStatus::Different => (Theme::different(), "≠"),
            DiffStatus::Identical => (Theme::identical(), "="),
            DiffStatus::DirectoryPresent => (Theme::dir(), " "),
            DiffStatus::TypeConflict => (Theme::type_conflict(), "!"),
            DiffStatus::Error(_) => (Theme::error(), "✗"),
        }
    }
}

fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}
