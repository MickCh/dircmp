//! External tool actions: the Enter/`[`/`]`/`{`/`}` policy, action queuing
//! and the terminal handoff around launching a tool.

use super::App;
use crate::{
    engine::diff::{DiffEntry, DiffStatus},
    tools::{self, ExternalAction},
    ui::statusbar::ToolKeyHints,
};
use anyhow::Result;
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::Backend, Terminal};
use std::path::PathBuf;

/// Which side of the comparison a path-based action targets.
#[derive(Clone, Copy)]
pub(super) enum Side {
    Left,
    Right,
}

/// Whether an entry with `status` has something to act on for `side`. The
/// single policy behind both `queue_side_action` and the `[`/`]`/`{`/`}`
/// status-bar hints.
fn side_available(status: &DiffStatus, side: Side) -> bool {
    match status {
        DiffStatus::LeftOnly => matches!(side, Side::Left),
        DiffStatus::RightOnly => matches!(side, Side::Right),
        DiffStatus::Different
        | DiffStatus::Identical
        | DiffStatus::TypeConflict
        | DiffStatus::Error(_) => true,
        DiffStatus::Pending | DiffStatus::DirectoryPresent => false,
    }
}

impl App {
    /// The single place encoding what Enter does for a given entry: Different →
    /// diff tool, LeftOnly/RightOnly → viewer. Returns `None` when the entry has
    /// no smart-open action or the required tool is not configured. Used both to
    /// queue the action and to light up the status-bar hint, so the two cannot
    /// drift apart.
    pub(super) fn open_action_for(&self, entry: &DiffEntry) -> Option<ExternalAction> {
        let tools = &self.config.tools;
        match &entry.status {
            DiffStatus::Different if tools.diff_tool.is_some() => Some(ExternalAction::Diff {
                left: self.left_root.join(&entry.relative_path),
                right: self.right_root.join(&entry.relative_path),
            }),
            DiffStatus::LeftOnly if tools.viewer.is_some() => Some(ExternalAction::ViewLeft(
                self.left_root.join(&entry.relative_path),
            )),
            DiffStatus::RightOnly if tools.viewer.is_some() => Some(ExternalAction::ViewRight(
                self.right_root.join(&entry.relative_path),
            )),
            _ => None,
        }
    }

    /// Queues an external action on the selected entry's left or right path.
    /// Ignored when the entry has no file on that side or the required tool is
    /// not configured — the latter guard keeps an unbound key from suspending
    /// the terminal for a guaranteed no-op (visible as a screen flicker).
    pub(super) fn queue_side_action(&mut self, side: Side, make: fn(PathBuf) -> ExternalAction) {
        let Some(entry) = self.selected_diff_entry() else { return };
        if !side_available(&entry.status, side) {
            return;
        }
        let root = match side {
            Side::Left => &self.left_root,
            Side::Right => &self.right_root,
        };
        let action = make(root.join(&entry.relative_path));
        if !tools::action_tool_configured(&self.config.tools, &action) {
            return;
        }
        self.pending_action = Some(action);
    }

    /// Availability of the tool key hints for the current selection, passed to
    /// the status bar (which only displays them — the policy lives here).
    pub(super) fn tool_key_hints(&self) -> ToolKeyHints {
        let status = self.selected_diff_entry().map(|e| &e.status);
        let has_left = status.is_some_and(|s| side_available(s, Side::Left));
        let has_right = status.is_some_and(|s| side_available(s, Side::Right));
        let enter_enabled = self
            .selected_diff_entry()
            .is_some_and(|e| self.open_action_for(e).is_some());
        ToolKeyHints {
            diff_tool_configured: self.config.tools.diff_tool.is_some(),
            viewer_configured: self.config.tools.viewer.is_some(),
            editor_configured: self.config.tools.editor.is_some(),
            enter_enabled,
            has_left,
            has_right,
        }
    }

    pub(super) fn launch_external<B: Backend + std::io::Write>(
        &mut self,
        terminal: &mut Terminal<B>,
        action: ExternalAction,
    ) -> Result<()>
    where
        B::Error: Send + Sync + 'static,
    {
        suspend_tui(terminal)?;
        let result = tools::run_action(&self.config.tools, &action);
        resume_tui(terminal)?;
        // A tool that fails to launch (missing binary, bad config) is reported
        // in the status bar; it must not abort the whole session.
        self.status_message = match result {
            Ok(msg) => msg,
            Err(e) => Some(format!("Tool failed: {e:#}")),
        };
        Ok(())
    }
}

fn suspend_tui<B: Backend + std::io::Write>(terminal: &mut Terminal<B>) -> Result<()>
where
    B::Error: Send + Sync + 'static,
{
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    Ok(())
}

fn resume_tui<B: Backend + std::io::Write>(terminal: &mut Terminal<B>) -> Result<()>
where
    B::Error: Send + Sync + 'static,
{
    enable_raw_mode()?;
    execute!(terminal.backend_mut(), EnterAlternateScreen)?;
    terminal.clear()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn side_available_matches_entry_side() {
        // One-sided entries only expose their own side.
        assert!(side_available(&DiffStatus::LeftOnly, Side::Left));
        assert!(!side_available(&DiffStatus::LeftOnly, Side::Right));
        assert!(side_available(&DiffStatus::RightOnly, Side::Right));
        assert!(!side_available(&DiffStatus::RightOnly, Side::Left));
        // Two-sided entries expose both.
        for status in [
            DiffStatus::Different,
            DiffStatus::Identical,
            DiffStatus::TypeConflict,
            DiffStatus::Error(String::new()),
        ] {
            assert!(side_available(&status, Side::Left));
            assert!(side_available(&status, Side::Right));
        }
        // Not yet compared / header-only entries expose neither.
        for status in [DiffStatus::Pending, DiffStatus::DirectoryPresent] {
            assert!(!side_available(&status, Side::Left));
            assert!(!side_available(&status, Side::Right));
        }
    }
}
