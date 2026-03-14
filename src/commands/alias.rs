use crate::config;
use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct AliasStore {
    #[serde(default)]
    pub aliases: BTreeMap<String, String>,
}

impl AliasStore {
    /// Load alias store from the config directory.
    pub fn load() -> Result<Self> {
        let path = Self::store_path()?;
        if !path.exists() {
            return Ok(Self::default());
        }
        let data = std::fs::read_to_string(&path)
            .map_err(crate::error::McpRegError::Io)?;
        let store: AliasStore = serde_json::from_str(&data)
            .unwrap_or_default();
        Ok(store)
    }

    /// Save alias store to disk.
    pub fn save(&self) -> Result<()> {
        let path = Self::store_path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(crate::error::McpRegError::Io)?;
        }
        let data = serde_json::to_string_pretty(self)
            .map_err(|e| crate::error::McpRegError::Validation(format!("JSON error: {e}")))?;
        std::fs::write(&path, data)
            .map_err(crate::error::McpRegError::Io)?;
        Ok(())
    }

    fn store_path() -> Result<std::path::PathBuf> {
        let dir = config::Config::config_dir()?;
        Ok(dir.join("aliases.json"))
    }

    /// Set an alias mapping short_name → owner/name.
    pub fn set(&mut self, alias: &str, target: &str) {
        self.aliases.insert(alias.to_string(), target.to_string());
    }

    /// Remove an alias.
    pub fn remove(&mut self, alias: &str) -> bool {
        self.aliases.remove(alias).is_some()
    }

    /// Resolve an alias to its target, or return the original string.
    #[allow(dead_code)]
    pub fn resolve(&self, name: &str) -> String {
        self.aliases.get(name).cloned().unwrap_or_else(|| name.to_string())
    }
}

/// Run the alias command: list, set, or remove aliases.
pub fn run_alias(action: Option<String>, alias: Option<String>, target: Option<String>) -> Result<()> {
    let action = action.as_deref().unwrap_or("list");

    match action {
        "list" | "ls" => {
            let store = AliasStore::load()?;
            if store.aliases.is_empty() {
                println!("No aliases configured.");
                println!("  Use: mcpreg alias set <shortname> <owner/name>");
            } else {
                println!("Configured aliases:");
                for (alias, target) in &store.aliases {
                    println!("  {alias} → {target}");
                }
            }
        }
        "set" | "add" => {
            let alias = alias.ok_or_else(|| {
                crate::error::McpRegError::Validation("Missing alias name. Usage: mcpreg alias set <name> <owner/server>".into())
            })?;
            let target = target.ok_or_else(|| {
                crate::error::McpRegError::Validation("Missing target. Usage: mcpreg alias set <name> <owner/server>".into())
            })?;
            if !target.contains('/') {
                return Err(crate::error::McpRegError::Validation(
                    format!("Target must be in owner/name format, got: {target}")
                ));
            }
            let mut store = AliasStore::load()?;
            store.set(&alias, &target);
            store.save()?;
            println!("Alias set: {alias} → {target}");
        }
        "remove" | "rm" | "delete" => {
            let alias = alias.ok_or_else(|| {
                crate::error::McpRegError::Validation("Missing alias name. Usage: mcpreg alias remove <name>".into())
            })?;
            let mut store = AliasStore::load()?;
            if store.remove(&alias) {
                store.save()?;
                println!("Alias removed: {alias}");
            } else {
                println!("Alias not found: {alias}");
            }
        }
        other => {
            return Err(crate::error::McpRegError::Validation(
                format!("Unknown alias action: {other}. Use: list, set, remove")
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alias_store_default_empty() {
        let store = AliasStore::default();
        assert!(store.aliases.is_empty());
    }

    #[test]
    fn test_alias_set_and_resolve() {
        let mut store = AliasStore::default();
        store.set("fs", "modelcontextprotocol/filesystem");
        assert_eq!(store.resolve("fs"), "modelcontextprotocol/filesystem");
    }

    #[test]
    fn test_alias_resolve_passthrough() {
        let store = AliasStore::default();
        assert_eq!(store.resolve("unknown/server"), "unknown/server");
    }

    #[test]
    fn test_alias_remove() {
        let mut store = AliasStore::default();
        store.set("db", "org/sqlite");
        assert!(store.remove("db"));
        assert!(!store.remove("db"));
        assert_eq!(store.resolve("db"), "db");
    }

    #[test]
    fn test_alias_overwrite() {
        let mut store = AliasStore::default();
        store.set("fs", "old/server");
        store.set("fs", "new/server");
        assert_eq!(store.resolve("fs"), "new/server");
        assert_eq!(store.aliases.len(), 1);
    }

    #[test]
    fn test_alias_store_serde_roundtrip() {
        let mut store = AliasStore::default();
        store.set("a", "owner/alpha");
        store.set("b", "owner/beta");
        let json = serde_json::to_string(&store).unwrap();
        let back: AliasStore = serde_json::from_str(&json).unwrap();
        assert_eq!(back.aliases.len(), 2);
        assert_eq!(back.resolve("a"), "owner/alpha");
    }

    #[test]
    fn test_alias_ordering_is_alphabetical() {
        let mut store = AliasStore::default();
        store.set("z", "owner/z");
        store.set("a", "owner/a");
        store.set("m", "owner/m");
        let keys: Vec<&String> = store.aliases.keys().collect();
        assert_eq!(keys, vec!["a", "m", "z"]);
    }
}
