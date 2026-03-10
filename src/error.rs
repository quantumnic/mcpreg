use std::fmt;

#[derive(Debug)]
pub enum McpRegError {
    Io(std::io::Error),
    Reqwest(reqwest::Error),
    Sqlite(rusqlite::Error),
    TomlParse(toml::de::Error),
    TomlSerialize(toml::ser::Error),
    SerdeJson(serde_json::Error),
    Config(String),
    Registry(String),
    NotFound(String),
    Auth(String),
    Manifest(String),
}

impl fmt::Display for McpRegError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "IO error: {e}"),
            Self::Reqwest(e) => write!(f, "HTTP error: {e}"),
            Self::Sqlite(e) => write!(f, "Database error: {e}"),
            Self::TomlParse(e) => write!(f, "TOML parse error: {e}"),
            Self::TomlSerialize(e) => write!(f, "TOML serialize error: {e}"),
            Self::SerdeJson(e) => write!(f, "JSON error: {e}"),
            Self::Config(msg) => write!(f, "Config error: {msg}"),
            Self::Registry(msg) => write!(f, "Registry error: {msg}"),
            Self::NotFound(msg) => write!(f, "Not found: {msg}"),
            Self::Auth(msg) => write!(f, "Auth error: {msg}"),
            Self::Manifest(msg) => write!(f, "Manifest error: {msg}"),
        }
    }
}

impl std::error::Error for McpRegError {}

impl From<std::io::Error> for McpRegError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<reqwest::Error> for McpRegError {
    fn from(e: reqwest::Error) -> Self {
        Self::Reqwest(e)
    }
}

impl From<rusqlite::Error> for McpRegError {
    fn from(e: rusqlite::Error) -> Self {
        Self::Sqlite(e)
    }
}

impl From<toml::de::Error> for McpRegError {
    fn from(e: toml::de::Error) -> Self {
        Self::TomlParse(e)
    }
}

impl From<toml::ser::Error> for McpRegError {
    fn from(e: toml::ser::Error) -> Self {
        Self::TomlSerialize(e)
    }
}

impl From<serde_json::Error> for McpRegError {
    fn from(e: serde_json::Error) -> Self {
        Self::SerdeJson(e)
    }
}

pub type Result<T> = std::result::Result<T, McpRegError>;
