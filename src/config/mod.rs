use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub comparison: ComparisonConfig,
    #[serde(default)]
    pub ui: UiConfig,
    pub scan: ScanConfig,
    #[serde(default)]
    pub tools: ToolsConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ComparisonConfig {
    pub strategy: ComparisonStrategy,
    pub text: TextComparisonConfig,
    /// Use parallel file comparison (SSD-optimised).
    /// On Linux the value is auto-detected from the filesystem at startup;
    /// on other platforms this setting is used directly.
    /// Default: true (assumes SSD, which is the common case today).
    #[serde(default = "default_true")]
    pub parallel: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ComparisonStrategy {
    Hash,
    Metadata,
    Byte,
    Text,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TextComparisonConfig {
    pub ignore_whitespace: bool,
    pub ignore_case: bool,
}

// Reserved for future UI settings (e.g. theme selection, column widths).
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct UiConfig {}

/// A tool command: either a whitespace-separated string (e.g. `"nvim -d"`) or an explicit
/// argument list (e.g. `["my tool", "-d"]`). Use the array form when the program path
/// contains spaces.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum ToolCommand {
    String(String),
    Args(Vec<String>),
}

impl ToolCommand {
    /// Returns the program name and its pre-configured arguments.
    pub fn program_and_args(&self) -> (&str, Vec<&str>) {
        match self {
            ToolCommand::String(s) => {
                let mut parts = s.split_whitespace();
                let program = parts.next().unwrap_or("");
                (program, parts.collect())
            }
            ToolCommand::Args(v) => {
                let program = v.first().map(|s| s.as_str()).unwrap_or("");
                (program, v[1..].iter().map(|s| s.as_str()).collect())
            }
        }
    }
}

impl<'de> serde::Deserialize<'de> for ToolCommand {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = ToolCommand;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "a string or array of strings")
            }
            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<ToolCommand, E> {
                Ok(ToolCommand::String(v.to_owned()))
            }
            fn visit_seq<A: serde::de::SeqAccess<'de>>(self, mut seq: A) -> Result<ToolCommand, A::Error> {
                let mut args = Vec::new();
                while let Some(s) = seq.next_element::<String>()? {
                    args.push(s);
                }
                Ok(ToolCommand::Args(args))
            }
        }
        d.deserialize_any(Visitor)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct ToolsConfig {
    pub diff_tool: Option<ToolCommand>,
    pub viewer: Option<ToolCommand>,
    pub editor: Option<ToolCommand>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ScanConfig {
    pub ignore_patterns: Vec<String>,
    pub follow_symlinks: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            comparison: ComparisonConfig {
                strategy: ComparisonStrategy::Hash,
                text: TextComparisonConfig {
                    ignore_whitespace: true,
                    ignore_case: false,
                },
                parallel: true,
            },
            ui: UiConfig {},
            scan: ScanConfig {
                ignore_patterns: vec![
                    ".git".to_string(),
                    ".DS_Store".to_string(),
                    "Thumbs.db".to_string(),
                    "node_modules".to_string(),
                ],
                follow_symlinks: false,
            },
            tools: ToolsConfig::default(),
        }
    }
}

impl Config {
    /// Loads configuration from a file. Returns an error if the file is missing or invalid.
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Cannot read config file: {}", path.display()))?;
        let config: Self = toml::from_str(&content)
            .with_context(|| format!("Failed to parse config file: {}", path.display()))?;
        Ok(config)
    }

    /// Loads configuration from a file. If the file does not exist, creates it with default settings.
    pub fn load_or_create(path: &Path) -> Result<Self> {
        if !path.exists() {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("Cannot create config directory: {}", parent.display()))?;
            }
            std::fs::write(path, Self::default_template())
                .with_context(|| format!("Cannot write default config: {}", path.display()))?;
            eprintln!("Created default config: {}", path.display());
        }
        Self::load(path)
    }

    /// Returns the default path to the configuration file.
    /// Linux/macOS: ~/.config/dircmp/config.toml
    /// Windows:     %APPDATA%\dircmp\config.toml
    pub fn default_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("dircmp")
            .join("config.toml")
    }

    /// Returns the default configuration file contents (TOML with comments).
    fn default_template() -> &'static str {
        include_str!("../../config.toml")
    }
}
