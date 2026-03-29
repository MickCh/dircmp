use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub comparison: ComparisonConfig,
    pub ui: UiConfig,
    pub scan: ScanConfig,
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

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct UiConfig {
    pub panel_scroll_step: usize,
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
            ui: UiConfig {
                panel_scroll_step: 3,
            },
            scan: ScanConfig {
                ignore_patterns: vec![
                    ".git".to_string(),
                    ".DS_Store".to_string(),
                    "Thumbs.db".to_string(),
                    "node_modules".to_string(),
                ],
                follow_symlinks: false,
            },
        }
    }
}

impl Config {
    /// Ładuje konfigurację z pliku. Jeśli plik nie istnieje, zwraca domyślną konfigurację.
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Cannot read config file: {}", path.display()))?;
        let config: Self = toml::from_str(&content)
            .with_context(|| format!("Failed to parse config file: {}", path.display()))?;
        Ok(config)
    }

    /// Zwraca domyślną ścieżkę do pliku konfiguracji (~/.config/dircmp/config.toml).
    pub fn default_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("dircmp")
            .join("config.toml")
    }
}
