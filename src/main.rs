mod app;
mod config;
mod engine;
mod ui;

use anyhow::{bail, Context, Result};
use app::App;
use config::Config;
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::{io, path::PathBuf};

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();

    let (left_path, right_path, config_path) = parse_args(&args)?;

    if !left_path.exists() {
        bail!("Lewy folder nie istnieje: {}", left_path.display());
    }
    if !left_path.is_dir() {
        bail!("Lewa ścieżka nie jest folderem: {}", left_path.display());
    }
    if !right_path.exists() {
        bail!("Prawy folder nie istnieje: {}", right_path.display());
    }
    if !right_path.is_dir() {
        bail!("Prawa ścieżka nie jest folderem: {}", right_path.display());
    }

    let config_path = config_path.unwrap_or_else(Config::default_path);
    let config = Config::load(&config_path)
        .context("Błąd ładowania konfiguracji")?;

    // Inicjalizacja terminala
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Uruchom aplikację
    let result = run_app(&mut terminal, left_path, right_path, config);

    // Przywróć terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    left_path: PathBuf,
    right_path: PathBuf,
    config: Config,
) -> Result<()> {
    let mut app = App::new(left_path, right_path, config);
    app.run(terminal)
}

fn parse_args(args: &[String]) -> Result<(PathBuf, PathBuf, Option<PathBuf>)> {
    let mut left = None;
    let mut right = None;
    let mut config = None;
    let mut i = 1;

    while i < args.len() {
        match args[i].as_str() {
            "--config" | "-c" => {
                i += 1;
                if i >= args.len() {
                    bail!("Brakuje ścieżki po --config");
                }
                config = Some(PathBuf::from(&args[i]));
            }
            "--help" | "-h" => {
                print_usage();
                std::process::exit(0);
            }
            arg if !arg.starts_with('-') => {
                if left.is_none() {
                    left = Some(PathBuf::from(arg));
                } else if right.is_none() {
                    right = Some(PathBuf::from(arg));
                } else {
                    bail!("Za dużo argumentów");
                }
            }
            arg => bail!("Nieznana opcja: {}", arg),
        }
        i += 1;
    }

    let left = left.ok_or_else(|| anyhow::anyhow!("Brakuje lewego folderu\n{}", usage_str()))?;
    let right = right.ok_or_else(|| anyhow::anyhow!("Brakuje prawego folderu\n{}", usage_str()))?;

    Ok((left, right, config))
}

fn usage_str() -> &'static str {
    "Użycie: dircmp <lewy_folder> <prawy_folder> [--config <plik>]"
}

fn print_usage() {
    println!("{}", usage_str());
    println!();
    println!("Opcje:");
    println!("  -c, --config <plik>   Ścieżka do pliku konfiguracji TOML");
    println!("  -h, --help            Wyświetl tę pomoc");
    println!();
    println!("Klawisze:");
    println!("  F5          Uruchom ponowne porównanie");
    println!("  F           Przełącz filtr (wszystkie / tylko różnice)");
    println!("  ↑ / ↓       Nawigacja po wierszach");
    println!("  PgUp/PgDn   Przewijanie");
    println!("  Home/End    Skocz na początek/koniec listy");
    println!("  q           Wyjście");
}
