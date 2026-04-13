use ratatui::style::{Color, Modifier, Style};

// Catppuccin Mocha palette
const SURFACE0: Color = Color::Rgb(49, 50, 68);     // #313244
const OVERLAY0: Color = Color::Rgb(108, 112, 134);  // #6c7086
const TEXT: Color = Color::Rgb(205, 214, 244);      // #cdd6f4
const BLUE: Color = Color::Rgb(137, 180, 250);      // #89b4fa
const LAVENDER: Color = Color::Rgb(180, 190, 254);  // #b4befe
const YELLOW: Color = Color::Rgb(249, 226, 175);    // #f9e2af
const RED: Color = Color::Rgb(243, 139, 168);       // #f38ba8
const MAUVE: Color = Color::Rgb(203, 166, 247);     // #cba6f7

pub struct Theme;

impl Theme {
    // Diff statuses
    pub fn left_only() -> Style {
        Style::default().fg(YELLOW)
    }

    pub fn right_only() -> Style {
        Style::default().fg(YELLOW)
    }

    pub fn different() -> Style {
        Style::default().fg(RED)
    }

    pub fn identical() -> Style {
        Style::default().fg(TEXT)
    }

    pub fn type_conflict() -> Style {
        Style::default().fg(MAUVE)
    }

    pub fn error() -> Style {
        Style::default().fg(RED).add_modifier(Modifier::BOLD)
    }

    pub fn pending() -> Style {
        Style::default().fg(OVERLAY0)
    }

    // UI elements
    pub fn selected() -> Style {
        // Dark purple-tinted background, lavender text
        Style::default()
            .bg(Color::Rgb(52, 48, 82))
            .fg(LAVENDER)
            .add_modifier(Modifier::BOLD)
    }

    pub fn statusbar() -> Style {
        Style::default().bg(SURFACE0).fg(TEXT)
    }

    pub fn statusbar_key() -> Style {
        Style::default()
            .bg(SURFACE0)
            .fg(BLUE)
            .add_modifier(Modifier::BOLD)
    }

    pub fn statusbar_inactive_key() -> Style {
        Style::default().bg(SURFACE0).fg(OVERLAY0)
    }

    pub fn folder_header() -> Style {
        // Dark navy-tinted background, blue text
        Style::default()
            .bg(Color::Rgb(30, 48, 80))
            .fg(BLUE)
            .add_modifier(Modifier::BOLD)
    }

    pub fn header_path() -> Style {
        Style::default()
            .bg(Color::Rgb(30, 48, 80))
            .fg(BLUE)
            .add_modifier(Modifier::BOLD)
    }

}
