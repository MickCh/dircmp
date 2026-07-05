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
    /// Index into the `DiffResult::entries` slice (no owned copy stored here).
    Entry(usize),
}

/// The filtered, header-decorated row list of the diff view.
///
/// Owns the invariant that cursor navigation lands only on `Entry` rows —
/// all movement helpers skip `FolderHeader` rows.
#[derive(Default)]
pub struct ViewRows(Vec<ViewRow>);

impl ViewRows {
    /// Builds the row list from `entries`.
    ///
    /// `matches` controls which entries are visible (filter predicate).
    /// `ViewRow::Entry(i)` stores the *original index* into `entries` so the
    /// render path can look up the entry without cloning it.
    pub fn build<F>(entries: &[DiffEntry], matches: F) -> Self
    where
        F: Fn(&DiffEntry) -> bool,
    {
        let mut rows: Vec<ViewRow> = Vec::new();
        let mut current_parent: Option<PathBuf> = None;

        for (i, entry) in entries.iter().enumerate() {
            if entry.is_dir || !matches(entry) {
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

            rows.push(ViewRow::Entry(i));
        }

        Self(rows)
    }

    /// Wraps a raw row list. Intended for tests that need row layouts
    /// `build` would never produce (e.g. trailing headers).
    pub fn from_rows(rows: Vec<ViewRow>) -> Self {
        Self(rows)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn get(&self, idx: usize) -> Option<&ViewRow> {
        self.0.get(idx)
    }

    pub fn as_slice(&self) -> &[ViewRow] {
        &self.0
    }

    /// Returns the next index pointing to an Entry row, or `current` if already at the last one.
    pub fn next(&self, current: usize) -> usize {
        let rows = &self.0;
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
    pub fn prev(&self, current: usize) -> usize {
        let rows = &self.0;
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

    /// Advances `n` Entry rows forward in a single pass; stops at the last Entry if fewer remain.
    pub fn nth_next(&self, current: usize, n: usize) -> usize {
        let mut count = 0;
        let mut result = current;
        for (i, row) in self.0.iter().enumerate().skip(current + 1) {
            if matches!(row, ViewRow::Entry(_)) {
                result = i;
                count += 1;
                if count == n {
                    return i;
                }
            }
        }
        result
    }

    /// Moves `n` Entry rows backward in a single pass; stops at the first Entry if fewer remain.
    pub fn nth_prev(&self, current: usize, n: usize) -> usize {
        let rows = &self.0;
        let mut count = 0;
        let mut result = current;
        let mut i = current;
        while i > 0 {
            i -= 1;
            if matches!(rows[i], ViewRow::Entry(_)) {
                result = i;
                count += 1;
                if count == n {
                    return i;
                }
            }
        }
        result
    }

    /// Returns the index of the first Entry row, or 0 if there are none.
    pub fn first(&self) -> usize {
        self.0
            .iter()
            .position(|r| matches!(r, ViewRow::Entry(_)))
            .unwrap_or(0)
    }

    /// Returns the index of the last Entry row, or 0 if there are none.
    pub fn last(&self) -> usize {
        self.0
            .iter()
            .rposition(|r| matches!(r, ViewRow::Entry(_)))
            .unwrap_or(0)
    }

    /// Returns the next index pointing to an Entry row whose `DiffEntry` satisfies `pred`,
    /// or `current` if no such row is found after the current position.
    pub fn next_matching<F>(&self, entries: &[DiffEntry], current: usize, pred: F) -> usize
    where
        F: Fn(&DiffEntry) -> bool,
    {
        let rows = &self.0;
        let mut i = current + 1;
        while i < rows.len() {
            if let ViewRow::Entry(idx) = &rows[i]
                && pred(&entries[*idx])
            {
                return i;
            }
            i += 1;
        }
        current
    }

    /// Returns the previous index pointing to an Entry row whose `DiffEntry` satisfies `pred`,
    /// or `current` if no such row is found before the current position.
    pub fn prev_matching<F>(&self, entries: &[DiffEntry], current: usize, pred: F) -> usize
    where
        F: Fn(&DiffEntry) -> bool,
    {
        let rows = &self.0;
        if current == 0 {
            return current;
        }
        let mut i = current - 1;
        loop {
            if let ViewRow::Entry(idx) = &rows[i]
                && pred(&entries[*idx])
            {
                return i;
            }
            if i == 0 {
                break;
            }
            i -= 1;
        }
        current
    }
}

pub struct DiffView {
    pub list_state: ListState,
    /// First visible row – managed manually to enable virtual scrolling.
    scroll_offset: usize,
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
        Self { list_state, scroll_offset: 0 }
    }

    pub fn selected_index(&self) -> Option<usize> {
        self.list_state.selected()
    }

    pub fn render(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        rows: &ViewRows,
        entries: &[DiffEntry],
    ) {
        let height = area.height as usize;
        let n = rows.len();

        if height == 0 || n == 0 {
            return;
        }

        let total_width = area.width as usize;
        let symbol_width = 5;
        let name_total = total_width.saturating_sub(symbol_width);
        let left_col = name_total / 2;
        let right_col = name_total - left_col;

        // Clamp selected to a valid row.
        let selected = self.list_state.selected().unwrap_or(0).min(n - 1);

        // Adjust scroll_offset so that selected stays within the visible window.
        if selected < self.scroll_offset {
            self.scroll_offset = selected;
        } else if selected >= self.scroll_offset + height {
            self.scroll_offset = selected + 1 - height;
        }

        let start = self.scroll_offset;
        let end = (start + height).min(n);

        // Build ListItems ONLY for the visible window (~terminal height rows,
        // typically 40–60), not for the entire list (potentially 35 000+ rows).
        let items: Vec<ListItem> = rows.as_slice()[start..end]
            .iter()
            .map(|row| match row {
                ViewRow::FolderHeader(path) => make_header_item(path, total_width),
                ViewRow::Entry(idx) => make_entry_item(&entries[*idx], left_col, right_col),
            })
            .collect();

        // Use a temporary ListState with the selection expressed relative to
        // our window so ratatui highlights the correct row.
        let mut render_state = ListState::default();
        render_state.select(Some(selected - start));

        let list = List::new(items).highlight_style(Theme::selected());
        frame.render_stateful_widget(list, area, &mut render_state);
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
