//! Key → [`Command`] decoding and command handling.

use super::App;
use crate::{engine::diff::DiffStatus, tools::ExternalAction};
use crossterm::event::{KeyCode, KeyModifiers};

/// A user command decoded from a key press. Keeping the key map a pure
/// `(KeyCode, KeyModifiers) -> Command` function makes it unit-testable
/// without a terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Command {
    Quit,
    Rescan,
    ToggleLeftOnly,
    ToggleRightOnly,
    ToggleDifferent,
    ToggleIdentical,
    JumpNextMatching,
    JumpPrevMatching,
    CursorDown,
    CursorUp,
    PageDown,
    PageUp,
    CursorHome,
    CursorEnd,
    /// Smart open: Different → diff tool, LeftOnly/RightOnly → viewer.
    Open,
    ViewLeft,
    ViewRight,
    EditLeft,
    EditRight,
}

pub(super) fn command_for_key(code: KeyCode, modifiers: KeyModifiers) -> Option<Command> {
    // AltGr arrives as Ctrl+Alt on Windows terminals; European layouts type
    // '[' ']' '{' '}' with AltGr, so that combination counts as unmodified.
    let altgr = KeyModifiers::CONTROL | KeyModifiers::ALT;
    let modifiers = if modifiers.contains(altgr) {
        modifiers.difference(altgr)
    } else {
        modifiers
    };

    // In raw mode Ctrl+C no longer raises SIGINT — honour it as quit.
    if modifiers.contains(KeyModifiers::CONTROL) {
        return (code == KeyCode::Char('c')).then_some(Command::Quit);
    }

    // Uppercase letters arrive as Shift + char; any other modifier means the
    // combination is unbound (Alt+d must not toggle a filter).
    if modifiers.intersects(
        KeyModifiers::ALT | KeyModifiers::SUPER | KeyModifiers::HYPER | KeyModifiers::META,
    ) {
        return None;
    }

    Some(match code {
        KeyCode::Char('q') | KeyCode::Char('Q') => Command::Quit,
        KeyCode::F(5) => Command::Rescan,
        KeyCode::Char('l') | KeyCode::Char('L') => Command::ToggleLeftOnly,
        KeyCode::Char('r') | KeyCode::Char('R') => Command::ToggleRightOnly,
        KeyCode::Char('d') | KeyCode::Char('D') => Command::ToggleDifferent,
        KeyCode::Char('i') | KeyCode::Char('I') => Command::ToggleIdentical,
        KeyCode::Char('n') => Command::JumpNextMatching,
        KeyCode::Char('N') => Command::JumpPrevMatching,
        KeyCode::Down => Command::CursorDown,
        KeyCode::Up => Command::CursorUp,
        KeyCode::PageDown => Command::PageDown,
        KeyCode::PageUp => Command::PageUp,
        KeyCode::Home => Command::CursorHome,
        KeyCode::End => Command::CursorEnd,
        KeyCode::Enter => Command::Open,
        KeyCode::Char('[') => Command::ViewLeft,
        KeyCode::Char(']') => Command::ViewRight,
        KeyCode::Char('{') => Command::EditLeft,
        KeyCode::Char('}') => Command::EditRight,
        _ => return None,
    })
}

impl App {
    pub(super) fn apply(&mut self, cmd: Command) {
        match cmd {
            Command::Quit => self.should_quit = true,
            Command::Rescan => self.run_diff(),
            Command::ToggleLeftOnly => {
                self.filter.show_left_only = !self.filter.show_left_only;
                self.rebuild_filtered();
            }
            Command::ToggleRightOnly => {
                self.filter.show_right_only = !self.filter.show_right_only;
                self.rebuild_filtered();
            }
            Command::ToggleDifferent => {
                self.filter.show_different = !self.filter.show_different;
                self.rebuild_filtered();
            }
            Command::ToggleIdentical => {
                self.filter.show_identical = !self.filter.show_identical;
                self.rebuild_filtered();
            }
            Command::JumpNextMatching => self.jump_to_matching(true),
            Command::JumpPrevMatching => self.jump_to_matching(false),
            Command::CursorDown => {
                let cur = self.diff_view.selected_index().unwrap_or(0);
                self.diff_view.list_state.select(Some(self.view_rows.next(cur)));
            }
            Command::CursorUp => {
                let cur = self.diff_view.selected_index().unwrap_or(0);
                self.diff_view.list_state.select(Some(self.view_rows.prev(cur)));
            }
            Command::PageDown => {
                let cur = self.diff_view.selected_index().unwrap_or(0);
                self.diff_view
                    .list_state
                    .select(Some(self.view_rows.nth_next(cur, self.page_height)));
            }
            Command::PageUp => {
                let cur = self.diff_view.selected_index().unwrap_or(0);
                self.diff_view
                    .list_state
                    .select(Some(self.view_rows.nth_prev(cur, self.page_height)));
            }
            Command::CursorHome => {
                self.diff_view.list_state.select(Some(self.view_rows.first()));
            }
            Command::CursorEnd => {
                self.diff_view.list_state.select(Some(self.view_rows.last()));
            }
            Command::Open => {
                let action = self
                    .selected_diff_entry()
                    .and_then(|entry| self.open_action_for(entry));
                self.pending_action = action;
            }
            Command::ViewLeft => self.queue_side_action(super::external::Side::Left, ExternalAction::ViewLeft),
            Command::ViewRight => self.queue_side_action(super::external::Side::Right, ExternalAction::ViewRight),
            Command::EditLeft => self.queue_side_action(super::external::Side::Left, ExternalAction::EditLeft),
            Command::EditRight => self.queue_side_action(super::external::Side::Right, ExternalAction::EditRight),
        }
    }

    /// Jumps to the next (`forward = true`) or previous entry whose status matches
    /// the discriminant of the currently selected entry (defaults to `Different`).
    fn jump_to_matching(&mut self, forward: bool) {
        let cur = self.diff_view.selected_index().unwrap_or(0);
        let target = self
            .selected_diff_entry()
            .map(|e| std::mem::discriminant(&e.status))
            .unwrap_or_else(|| std::mem::discriminant(&DiffStatus::Different));
        let entries = self
            .diff_result
            .as_ref()
            .map(|r| r.entries.as_slice())
            .unwrap_or_default();
        let idx = if forward {
            self.view_rows.next_matching(entries, cur, |e| {
                std::mem::discriminant(&e.status) == target
            })
        } else {
            self.view_rows.prev_matching(entries, cur, |e| {
                std::mem::discriminant(&e.status) == target
            })
        };
        self.diff_view.list_state.select(Some(idx));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NONE: KeyModifiers = KeyModifiers::NONE;

    #[test]
    fn letter_keys_map_case_insensitively() {
        for (lower, upper, expected) in [
            ('q', 'Q', Command::Quit),
            ('l', 'L', Command::ToggleLeftOnly),
            ('r', 'R', Command::ToggleRightOnly),
            ('d', 'D', Command::ToggleDifferent),
            ('i', 'I', Command::ToggleIdentical),
        ] {
            assert_eq!(command_for_key(KeyCode::Char(lower), NONE), Some(expected));
            assert_eq!(command_for_key(KeyCode::Char(upper), NONE), Some(expected));
            // Terminals report uppercase letters with the Shift modifier set.
            assert_eq!(
                command_for_key(KeyCode::Char(upper), KeyModifiers::SHIFT),
                Some(expected)
            );
        }
    }

    #[test]
    fn jump_keys_are_case_sensitive() {
        assert_eq!(command_for_key(KeyCode::Char('n'), NONE), Some(Command::JumpNextMatching));
        assert_eq!(command_for_key(KeyCode::Char('N'), NONE), Some(Command::JumpPrevMatching));
    }

    #[test]
    fn navigation_and_tool_keys_map() {
        assert_eq!(command_for_key(KeyCode::F(5), NONE), Some(Command::Rescan));
        assert_eq!(command_for_key(KeyCode::Down, NONE), Some(Command::CursorDown));
        assert_eq!(command_for_key(KeyCode::Up, NONE), Some(Command::CursorUp));
        assert_eq!(command_for_key(KeyCode::PageDown, NONE), Some(Command::PageDown));
        assert_eq!(command_for_key(KeyCode::PageUp, NONE), Some(Command::PageUp));
        assert_eq!(command_for_key(KeyCode::Home, NONE), Some(Command::CursorHome));
        assert_eq!(command_for_key(KeyCode::End, NONE), Some(Command::CursorEnd));
        assert_eq!(command_for_key(KeyCode::Enter, NONE), Some(Command::Open));
        assert_eq!(command_for_key(KeyCode::Char('['), NONE), Some(Command::ViewLeft));
        assert_eq!(command_for_key(KeyCode::Char(']'), NONE), Some(Command::ViewRight));
        assert_eq!(command_for_key(KeyCode::Char('{'), NONE), Some(Command::EditLeft));
        assert_eq!(command_for_key(KeyCode::Char('}'), NONE), Some(Command::EditRight));
    }

    #[test]
    fn ctrl_c_quits() {
        assert_eq!(
            command_for_key(KeyCode::Char('c'), KeyModifiers::CONTROL),
            Some(Command::Quit)
        );
    }

    #[test]
    fn other_modified_combinations_are_unbound() {
        assert_eq!(command_for_key(KeyCode::Char('d'), KeyModifiers::CONTROL), None);
        assert_eq!(command_for_key(KeyCode::Char('q'), KeyModifiers::ALT), None);
        assert_eq!(command_for_key(KeyCode::Down, KeyModifiers::CONTROL), None);
        assert_eq!(command_for_key(KeyCode::Enter, KeyModifiers::ALT), None);
    }

    #[test]
    fn altgr_counts_as_unmodified() {
        // Windows terminals report AltGr as Ctrl+Alt.
        let altgr = KeyModifiers::CONTROL | KeyModifiers::ALT;
        assert_eq!(command_for_key(KeyCode::Char('['), altgr), Some(Command::ViewLeft));
        assert_eq!(
            command_for_key(KeyCode::Char('}'), altgr | KeyModifiers::SHIFT),
            Some(Command::EditRight)
        );
    }

    #[test]
    fn unknown_keys_map_to_none() {
        assert_eq!(command_for_key(KeyCode::Char('x'), NONE), None);
        assert_eq!(command_for_key(KeyCode::Esc, NONE), None);
        assert_eq!(command_for_key(KeyCode::F(1), NONE), None);
    }
}
