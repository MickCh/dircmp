mod app;
mod config;
mod engine;
mod platform;
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
        bail!("Left folder does not exist: {}", left_path.display());
    }
    if !left_path.is_dir() {
        bail!("Left path is not a directory: {}", left_path.display());
    }
    if !right_path.exists() {
        bail!("Right folder does not exist: {}", right_path.display());
    }
    if !right_path.is_dir() {
        bail!("Right path is not a directory: {}", right_path.display());
    }

    let config_path = config_path.unwrap_or_else(Config::default_path);
    let config = Config::load(&config_path)
        .context("Failed to load configuration")?;

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_app(&mut terminal, left_path, right_path, config);

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
                    bail!("Missing path after --config");
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
                    bail!("Too many arguments");
                }
            }
            arg => bail!("Unknown option: {}", arg),
        }
        i += 1;
    }

    let left = left.ok_or_else(|| anyhow::anyhow!("Missing left folder\n{}", usage_str()))?;
    let right = right.ok_or_else(|| anyhow::anyhow!("Missing right folder\n{}", usage_str()))?;

    Ok((left, right, config))
}

fn usage_str() -> &'static str {
    "Usage: dircmp <left_folder> <right_folder> [--config <file>]"
}

fn print_usage() {
    println!("{}", usage_str());
    println!();
    println!("Options:");
    println!("  -c, --config <file>   Path to TOML configuration file");
    println!("  -h, --help            Show this help");
    println!();
    println!("Keys:");
    println!("  F5          Re-run comparison");
    println!("  F           Toggle filter (all / differences only)");
    println!("  ↑ / ↓       Navigate rows");
    println!("  PgUp/PgDn   Scroll");
    println!("  Home/End    Jump to first/last entry");
    println!("  q           Quit");
}
