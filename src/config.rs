use crate::error::{McpRegError, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const DEFAULT_REGISTRY: &str = "https://registry.mcpreg.dev";

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    pub registry_url: String,
    pub api_key: Option<String>,
    pub install_dir: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            registry_url: DEFAULT_REGISTRY.to_string(),
            api_key: None,
            install_dir: None,
        }
    }
}

impl Config {
    pub fn config_dir() -> Result<PathBuf> {
        let home = dirs::home_dir().ok_or_else(|| McpRegError::Config("Cannot find home directory".into()))?;
        Ok(home.join(".mcpreg"))
    }

    pub fn config_path() -> Result<PathBuf> {
        Ok(Self::config_dir()?.join("config.toml"))
    }

    pub fn load() -> Result<Self> {
        let path = Self::config_path()?;
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(&path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }

    #[allow(dead_code)]
    pub fn save(&self) -> Result<()> {
        let dir = Self::config_dir()?;
        std::fs::create_dir_all(&dir)?;
        let content = toml::to_string_pretty(self)?;
        std::fs::write(Self::config_path()?, content)?;
        Ok(())
    }

    pub fn installed_servers_path() -> Result<PathBuf> {
        Ok(Self::config_dir()?.join("installed.json"))
    }

    pub fn claude_desktop_config_path() -> Result<PathBuf> {
        let home = dirs::home_dir().ok_or_else(|| McpRegError::Config("Cannot find home directory".into()))?;

        #[cfg(target_os = "macos")]
        let path = home.join("Library/Application Support/Claude/claude_desktop_config.json");
        #[cfg(target_os = "linux")]
        let path = home.join(".config/claude/claude_desktop_config.json");
        #[cfg(target_os = "windows")]
        let path = home.join("AppData/Roaming/Claude/claude_desktop_config.json");

        Ok(path)
    }

    pub fn db_path() -> Result<PathBuf> {
        Ok(Self::config_dir()?.join("registry.db"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.registry_url, "https://registry.mcpreg.dev");
        assert!(config.api_key.is_none());
    }

    #[test]
    fn test_config_roundtrip() {
        let config = Config {
            registry_url: "http://localhost:3000".into(),
            api_key: Some("test-key".into()),
            install_dir: None,
        };
        let serialized = toml::to_string_pretty(&config).unwrap();
        let deserialized: Config = toml::from_str(&serialized).unwrap();
        assert_eq!(deserialized.registry_url, "http://localhost:3000");
        assert_eq!(deserialized.api_key.unwrap(), "test-key");
    }

    #[test]
    fn test_config_dir_is_under_home() {
        let dir = Config::config_dir().unwrap();
        assert!(dir.to_string_lossy().contains(".mcpreg"));
    }
}
