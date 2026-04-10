use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub vault_path: String,
    pub tasks_dir: Option<String>,
    pub outputs_dir: Option<String>,
    pub projects_dir: Option<String>,
    pub code_projects_path: Option<String>,
    pub ignore_dirs: Option<Vec<String>>,
    pub stale_days: Option<i64>,
}

impl Config {
    pub fn config_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .context("Could not find config directory")?
            .join("vaultlink");
        Ok(config_dir.join("config.toml"))
    }

    pub fn load() -> Result<Self> {
        let path = Self::config_path()?;
        if !path.exists() {
            anyhow::bail!(
                "Config not found at {}. Run `vaultlink --init` to create one.",
                path.display()
            );
        }
        let content = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read config at {}", path.display()))?;
        let config: Config = toml::from_str(&content).context("Failed to parse config.toml")?;
        Ok(config)
    }

    pub fn init_default() -> Result<()> {
        let path = Self::config_path()?;
        if path.exists() {
            println!("Config already exists at {}", path.display());
            return Ok(());
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let default = Config {
            vault_path: dirs::home_dir()
                .map(|h| h.join("obsidian-vault").to_string_lossy().to_string())
                .unwrap_or_else(|| "~/obsidian-vault".to_string()),
            tasks_dir: Some("tasks".to_string()),
            outputs_dir: Some("outputs".to_string()),
            projects_dir: Some("01-projects".to_string()),
            code_projects_path: None,
            ignore_dirs: Some(vec![
                ".obsidian".to_string(),
                "templates".to_string(),
                "assets".to_string(),
            ]),
            stale_days: Some(7),
        };
        let content = toml::to_string_pretty(&default)?;
        fs::write(&path, content)?;
        println!("Created default config at {}", path.display());
        Ok(())
    }

    pub fn vault_path(&self) -> PathBuf {
        PathBuf::from(&self.vault_path)
    }

    pub fn projects_dir(&self) -> String {
        self.projects_dir
            .clone()
            .unwrap_or_else(|| "01-projects".to_string())
    }

    pub fn ignore_dirs(&self) -> Vec<String> {
        self.ignore_dirs.clone().unwrap_or_else(|| {
            vec![
                ".obsidian".to_string(),
                "templates".to_string(),
                "assets".to_string(),
            ]
        })
    }

    pub fn stale_days(&self) -> i64 {
        self.stale_days.unwrap_or(7)
    }

    pub fn code_projects_path(&self) -> Option<PathBuf> {
        self.code_projects_path
            .as_ref()
            .map(PathBuf::from)
            .filter(|p| !p.as_os_str().is_empty())
    }
}
