use crate::{
    engine::diff::{DiffEntry, DiffStatus},
    ui::theme::Theme,
};
use ratatui::{
    layout::Rect,
    text::{Line, Span},
    widgets::{List, ListItem, ListState},
    Frame,
};
use std::path::PathBuf;

/// A row in the unified diff view.
pub enum ViewRow {
    /// A section header showing the directory path.
    FolderHeader(String),
    /// A file entry with diff status.
    Entry(DiffEntry),
}

pub struct DiffView {
    pub list_state: ListState,
}

impl Default for DiffView {
    fn default() -> Self {
        Self::new()
    }
}

impl DiffView {
    pub fn new() -> Self {
        let mut list_state = ListState::default();
        list_state.select(Some(0));
        Self { list_state }
    }

    pub fn selected_index(&self) -> Option<usize> {
        self.list_state.selected()
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect, rows: &[ViewRow]) {
        let total = area.width as usize;
        // Symbol column: " ≠ " = 3 chars + 2 spaces = 5
        let symbol_width = 5;
        let name_total = total.saturating_sub(symbol_width);
        let left_col = name_total / 2;
        let right_col = name_total - left_col;

        let items: Vec<ListItem> = rows
            .iter()
            .map(|row| match row {
                ViewRow::FolderHeader(path) => make_header_item(path, total),
                ViewRow::Entry(entry) => make_entry_item(entry, left_col, right_col),
            })
            .collect();

        let list = List::new(items).highlight_style(Theme::selected());
        frame.render_stateful_widget(list, area, &mut self.list_state);
    }
}

fn fit(s: &str, width: usize) -> String {
    let char_count = s.chars().count();
    if char_count <= width {
        format!("{:<width$}", s, width = width)
    } else {
        let truncated: String = s.chars().take(width.saturating_sub(1)).collect();
        format!("{}…", truncated)
    }
}

fn make_header_item(path: &str, total_width: usize) -> ListItem<'static> {
    let content = format!(" {}", path);
    let padded = format!("{:<width$}", content, width = total_width);
    ListItem::new(Line::from(Span::styled(padded, Theme::folder_header())))
}

fn make_entry_item(entry: &DiffEntry, left_col: usize, right_col: usize) -> ListItem<'static> {
    let name = entry
        .relative_path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| entry.relative_path.to_string_lossy().into_owned());

    let (symbol, sym_style, left_style, right_style) = entry_styles(&entry.status);

    let left_name = match &entry.status {
        DiffStatus::RightOnly => fit("", left_col),
        _ => fit(&name, left_col),
    };
    let right_name = match &entry.status {
        DiffStatus::LeftOnly => fit("", right_col),
        _ => fit(&name, right_col),
    };

    ListItem::new(Line::from(vec![
        Span::styled(left_name, left_style),
        Span::styled(format!(" {} ", symbol), sym_style),
        Span::styled(right_name, right_style),
    ]))
}

fn entry_styles(
    status: &DiffStatus,
) -> (
    &'static str,
    ratatui::style::Style,
    ratatui::style::Style,
    ratatui::style::Style,
) {
    match status {
        DiffStatus::Identical => (
            "=",
            Theme::identical(),
            Theme::identical(),
            Theme::identical(),
        ),
        DiffStatus::Different => (
            "≠",
            Theme::different(),
            Theme::different(),
            Theme::different(),
        ),
        DiffStatus::LeftOnly => (
            "►",
            Theme::left_only(),
            Theme::left_only(),
            Theme::identical(),
        ),
        DiffStatus::RightOnly => (
            "◄",
            Theme::right_only(),
            Theme::identical(),
            Theme::right_only(),
        ),
        DiffStatus::Pending => (
            "?",
            Theme::pending(),
            Theme::pending(),
            Theme::pending(),
        ),
        DiffStatus::TypeConflict => (
            "!",
            Theme::type_conflict(),
            Theme::type_conflict(),
            Theme::type_conflict(),
        ),
        DiffStatus::Error(_) => (
            "✗",
            Theme::error(),
            Theme::error(),
            Theme::error(),
        ),
        DiffStatus::DirectoryPresent => unreachable!("directory entries become folder headers"),
    }
}

/// Builds the list of view rows from diff entries.
/// Directory entries are skipped — they're represented implicitly via file parent paths.
pub fn build_view_rows(entries: &[DiffEntry]) -> Vec<ViewRow> {
    let mut rows: Vec<ViewRow> = Vec::new();
    let mut current_parent: Option<PathBuf> = None;

    for entry in entries {
        if entry.is_dir {
            continue;
        }

        let parent = entry
            .relative_path
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_default();

        if current_parent.as_ref() != Some(&parent) {
            let header = if parent.as_os_str().is_empty() {
                "./".to_string()
            } else {
                format!("{}/", parent.display())
            };
            rows.push(ViewRow::FolderHeader(header));
            current_parent = Some(parent);
        }

        rows.push(ViewRow::Entry(entry.clone()));
    }

    rows
}

/// Returns the next index pointing to an Entry row, or `current` if already at the last one.
pub fn next_entry(rows: &[ViewRow], current: usize) -> usize {
    let mut i = current + 1;
    while i < rows.len() {
        if matches!(rows[i], ViewRow::Entry(_)) {
            return i;
        }
        i += 1;
    }
    current
}

/// Returns the previous index pointing to an Entry row, or `current` if already at the first one.
pub fn prev_entry(rows: &[ViewRow], current: usize) -> usize {
    if current == 0 {
        return current;
    }
    let mut i = current - 1;
    loop {
        if matches!(rows[i], ViewRow::Entry(_)) {
            return i;
        }
        if i == 0 {
            break;
        }
        i -= 1;
    }
    current
}

/// Returns the index of the first Entry row, or 0 if there are none.
pub fn first_entry(rows: &[ViewRow]) -> usize {
    rows.iter()
        .position(|r| matches!(r, ViewRow::Entry(_)))
        .unwrap_or(0)
}

/// Returns the index of the last Entry row, or 0 if there are none.
pub fn last_entry(rows: &[ViewRow]) -> usize {
    rows.iter()
        .rposition(|r| matches!(r, ViewRow::Entry(_)))
        .unwrap_or(0)
}
