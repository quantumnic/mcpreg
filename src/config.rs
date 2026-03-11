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

    /// Load config from file, then overlay environment variables.
    /// - `MCPREG_REGISTRY_URL` overrides `registry_url`
    /// - `MCPREG_API_KEY` overrides `api_key`
    /// - `MCPREG_INSTALL_DIR` overrides `install_dir`
    pub fn load() -> Result<Self> {
        let path = Self::config_path()?;
        let mut config = if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            toml::from_str::<Config>(&content)?
        } else {
            Self::default()
        };

        // Environment variable overrides
        if let Ok(url) = std::env::var("MCPREG_REGISTRY_URL") {
            config.registry_url = url;
        }
        if let Ok(key) = std::env::var("MCPREG_API_KEY") {
            config.api_key = Some(key);
        }
        if let Ok(dir) = std::env::var("MCPREG_INSTALL_DIR") {
            config.install_dir = Some(dir);
        }

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
        assert!(config.install_dir.is_none());
    }

    #[test]
    fn test_config_roundtrip() {
        let config = Config {
            registry_url: "http://localhost:3000".into(),
            api_key: Some("test-key".into()),
            install_dir: Some("/opt/mcp".into()),
        };
        let serialized = toml::to_string_pretty(&config).unwrap();
        let deserialized: Config = toml::from_str(&serialized).unwrap();
        assert_eq!(deserialized.registry_url, "http://localhost:3000");
        assert_eq!(deserialized.api_key.unwrap(), "test-key");
        assert_eq!(deserialized.install_dir.unwrap(), "/opt/mcp");
    }

    #[test]
    fn test_config_dir_is_under_home() {
        let dir = Config::config_dir().unwrap();
        assert!(dir.to_string_lossy().contains(".mcpreg"));
    }

    #[test]
    fn test_env_var_overrides() {
        // Set env vars, load, check they override
        std::env::set_var("MCPREG_REGISTRY_URL", "http://test:9999");
        std::env::set_var("MCPREG_API_KEY", "env-key-123");
        let config = Config::load().unwrap();
        assert_eq!(config.registry_url, "http://test:9999");
        assert_eq!(config.api_key.as_deref(), Some("env-key-123"));
        // Cleanup
        std::env::remove_var("MCPREG_REGISTRY_URL");
        std::env::remove_var("MCPREG_API_KEY");
    }
}
