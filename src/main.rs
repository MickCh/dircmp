use anyhow::{bail, Context, Result};
use dircmp::{app::App, config::Config, platform};
use std::path::PathBuf;

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

    let mut config = match config_path {
        Some(path) => {
            if !path.exists() {
                bail!("Config file not found: {}", path.display());
            }
            Config::load(&path).with_context(|| format!("Failed to load config: {}", path.display()))?
        }
        None => Config::load_or_create(&Config::default_path()?)
            .context("Failed to load configuration")?,
    };

    // Auto-detect disk type on Linux; overrides the configured value when detection succeeds.
    if let Some(rotational) = platform::is_rotational(&left_path) {
        config.comparison.parallel = !rotational;
    }

    // ratatui::init() enables raw mode + alternate screen and installs a panic
    // hook that restores the terminal, so a crash never leaves it in raw mode.
    let mut terminal = ratatui::init();
    let result = App::new(left_path, right_path, config).run(&mut terminal);
    ratatui::restore();
    result
}

fn parse_args(args: &[String]) -> Result<(PathBuf, PathBuf, Option<PathBuf>)> {
    let mut left = None;
    let mut right = None;
    let mut config = None;
    // After a literal `--`, everything is a folder path — allows comparing
    // directories whose names start with `-`.
    let mut positional_only = false;
    let mut i = 1;

    while i < args.len() {
        let arg = args[i].as_str();
        let is_option = !positional_only && arg.starts_with('-') && arg.len() > 1;
        if is_option {
            match arg {
                "--" => positional_only = true,
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
                _ => bail!("Unknown option: {}", arg),
            }
        } else if left.is_none() {
            left = Some(PathBuf::from(arg));
        } else if right.is_none() {
            right = Some(PathBuf::from(arg));
        } else {
            bail!("Too many arguments");
        }
        i += 1;
    }

    let left = left.ok_or_else(|| anyhow::anyhow!("Missing left folder\n{}", usage_str()))?;
    let right = right.ok_or_else(|| anyhow::anyhow!("Missing right folder\n{}", usage_str()))?;

    Ok((left, right, config))
}

fn usage_str() -> &'static str {
    "Usage: dircmp [--config <file>] [--] <left_folder> <right_folder>"
}

fn print_usage() {
    println!("{}", usage_str());
    println!();
    println!("Options:");
    println!("  -c, --config <file>   Path to TOML configuration file");
    println!("  -h, --help            Show this help");
    println!("  --                    Treat the remaining arguments as folder paths");
    println!();
    println!("Keys:");
    println!("  F5          Re-run comparison");
    println!("  L           Toggle left-only filter");
    println!("  R           Toggle right-only filter");
    println!("  D           Toggle different filter");
    println!("  I           Toggle identical filter");
    println!("  n / N       Jump to next/previous entry with same status");
    println!("  ↑ / ↓       Navigate rows");
    println!("  PgUp/PgDn   Scroll");
    println!("  Home/End    Jump to first/last entry");
    println!("  q, Ctrl+C   Quit");
}
