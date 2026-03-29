use ratatui::style::{Color, Modifier, Style};

pub struct Theme;

impl Theme {
    // Statusy diff
    pub fn left_only() -> Style {
        Style::default().fg(Color::Red)
    }

    pub fn right_only() -> Style {
        Style::default().fg(Color::Green)
    }

    pub fn different() -> Style {
        Style::default().fg(Color::Yellow)
    }

    pub fn identical() -> Style {
        Style::default().fg(Color::Gray)
    }

    pub fn type_conflict() -> Style {
        Style::default().fg(Color::Magenta)
    }

    pub fn error() -> Style {
        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
    }

    // UI elementy
    pub fn selected() -> Style {
        Style::default()
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD)
    }

    pub fn statusbar() -> Style {
        Style::default().bg(Color::DarkGray).fg(Color::White)
    }

    pub fn statusbar_key() -> Style {
        Style::default()
            .bg(Color::DarkGray)
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    }

    pub fn folder_header() -> Style {
        Style::default()
            .bg(Color::DarkGray)
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    }

    pub fn header_path() -> Style {
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD)
    }
}
