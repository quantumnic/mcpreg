use crate::config::Config;
use crate::error::{McpRegError, Result};

/// Pin an installed server to its current version (skip during `mcpreg update`).
pub fn run_pin(server_ref: &str) -> Result<()> {
    modify_pin(server_ref, true)
}

/// Unpin an installed server so `mcpreg update` can upgrade it.
pub fn run_unpin(server_ref: &str) -> Result<()> {
    modify_pin(server_ref, false)
}

/// List pinned servers.
pub fn run_list() -> Result<()> {
    let path = Config::installed_servers_path()?;
    if !path.exists() {
        println!("No servers installed.");
        return Ok(());
    }
    let content = std::fs::read_to_string(&path)?;
    let installed: PinnedInstalledServers = serde_json::from_str(&content)?;

    let pinned: Vec<_> = installed.servers.iter().filter(|s| s.pinned).collect();
    if pinned.is_empty() {
        println!("No pinned servers.");
    } else {
        println!("Pinned servers:\n");
        for s in &pinned {
            println!("  📌 {}/{} v{}", s.owner, s.name, s.version);
        }
        println!("\n{} server(s) pinned.", pinned.len());
    }
    Ok(())
}

fn modify_pin(server_ref: &str, pin: bool) -> Result<()> {
    let parts: Vec<&str> = server_ref.splitn(2, '/').collect();
    if parts.len() != 2 {
        return Err(McpRegError::Config(
            "Server reference must be in format 'owner/name'".into(),
        ));
    }
    let (owner, name) = (parts[0], parts[1]);

    let path = Config::installed_servers_path()?;
    if !path.exists() {
        return Err(McpRegError::NotFound("No servers installed".into()));
    }

    let content = std::fs::read_to_string(&path)?;
    let mut installed: PinnedInstalledServers = serde_json::from_str(&content)?;

    let server = installed
        .servers
        .iter_mut()
        .find(|s| s.owner == owner && s.name == name)
        .ok_or_else(|| McpRegError::NotFound(format!("{owner}/{name} is not installed")))?;

    server.pinned = pin;
    let version = server.version.clone();

    let serialized = serde_json::to_string_pretty(&installed)?;
    std::fs::write(&path, serialized)?;

    if pin {
        println!("📌 Pinned {owner}/{name} at v{version}");
        println!("   This server will be skipped during 'mcpreg update'.");
    } else {
        println!("📌 Unpinned {owner}/{name}");
        println!("   This server will be updated normally.");
    }

    Ok(())
}

/// Extended InstalledServer with pin support (backward-compatible).
#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct PinnedInstalledServer {
    pub owner: String,
    pub name: String,
    pub version: String,
    pub command: String,
    pub args: Vec<String>,
    pub transport: String,
    pub installed_at: String,
    #[serde(default)]
    pub pinned: bool,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Default)]
pub struct PinnedInstalledServers {
    pub servers: Vec<PinnedInstalledServer>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pinned_deserialization_backward_compat() {
        // Old format without "pinned" field should default to false
        let json = r#"{"servers": [
            {"owner": "a", "name": "b", "version": "1.0.0", "command": "node",
             "args": [], "transport": "stdio", "installed_at": "2024-01-01"}
        ]}"#;
        let installed: PinnedInstalledServers = serde_json::from_str(json).unwrap();
        assert!(!installed.servers[0].pinned);
    }

    #[test]
    fn test_pinned_serialization_roundtrip() {
        let s = PinnedInstalledServer {
            owner: "alice".into(),
            name: "tool".into(),
            version: "1.0.0".into(),
            command: "node".into(),
            args: vec!["index.js".into()],
            transport: "stdio".into(),
            installed_at: "2024-01-01T00:00:00Z".into(),
            pinned: true,
        };
        let json = serde_json::to_string(&s).unwrap();
        let deserialized: PinnedInstalledServer = serde_json::from_str(&json).unwrap();
        assert!(deserialized.pinned);
        assert_eq!(deserialized.owner, "alice");
    }
}
