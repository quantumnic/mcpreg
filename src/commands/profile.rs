use crate::error::{McpRegError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// A named collection of MCP servers that can be saved, loaded, and shared.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    /// Profile name
    pub name: String,
    /// Human-readable description
    pub description: String,
    /// List of server references (owner/name)
    pub servers: Vec<String>,
    /// When this profile was created (Unix timestamp)
    pub created_at: u64,
    /// Optional metadata (e.g. author, use-case tags)
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct ProfileStore {
    profiles: Vec<Profile>,
}

fn profiles_path() -> Result<PathBuf> {
    let dir = crate::config::Config::config_dir()?;
    Ok(dir.join("profiles.json"))
}

fn load_store() -> Result<ProfileStore> {
    let path = profiles_path()?;
    if !path.exists() {
        return Ok(ProfileStore::default());
    }
    let data = fs::read_to_string(&path).map_err(McpRegError::Io)?;
    serde_json::from_str(&data).map_err(|e| McpRegError::Validation(e.to_string()))
}

fn save_store(store: &ProfileStore) -> Result<()> {
    let path = profiles_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(McpRegError::Io)?;
    }
    let data = serde_json::to_string_pretty(store)
        .map_err(|e| McpRegError::Validation(e.to_string()))?;
    fs::write(&path, data).map_err(McpRegError::Io)?;
    Ok(())
}

fn load_installed() -> Result<Vec<String>> {
    let path = crate::config::Config::installed_servers_path()?;
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = fs::read_to_string(&path).map_err(McpRegError::Io)?;
    let installed: crate::api::types::InstalledServers =
        serde_json::from_str(&content).map_err(|e| McpRegError::Validation(e.to_string()))?;
    Ok(installed.servers.iter().map(|s| s.full_name()).collect())
}

/// Save the currently installed servers as a named profile.
pub fn run_save(name: &str, description: Option<&str>) -> Result<()> {
    let server_names = load_installed()?;
    if server_names.is_empty() {
        eprintln!("⚠ No installed servers found. Install some servers first.");
        return Ok(());
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let profile = Profile {
        name: name.to_string(),
        description: description.unwrap_or("").to_string(),
        servers: server_names.clone(),
        created_at: now,
        metadata: HashMap::new(),
    };

    let mut store = load_store()?;

    // Replace if a profile with this name already exists
    store.profiles.retain(|p| p.name != name);
    store.profiles.push(profile);
    save_store(&store)?;

    println!("✓ Profile '{name}' saved with {} servers:", server_names.len());
    for s in &server_names {
        println!("  • {s}");
    }
    Ok(())
}

/// List all saved profiles.
pub fn run_list(json: bool) -> Result<()> {
    let store = load_store()?;

    if json {
        let output = serde_json::to_string_pretty(&store.profiles)
            .map_err(|e| McpRegError::Validation(e.to_string()))?;
        println!("{output}");
        return Ok(());
    }

    if store.profiles.is_empty() {
        println!("No saved profiles. Use 'mcpreg profile save <name>' to create one.");
        return Ok(());
    }

    println!("📋 Saved profiles:\n");
    for p in &store.profiles {
        let desc = if p.description.is_empty() {
            String::new()
        } else {
            format!(" — {}", p.description)
        };
        println!("  {} ({} servers){}", p.name, p.servers.len(), desc);
        for s in &p.servers {
            println!("    • {s}");
        }
        println!();
    }
    Ok(())
}

/// Show details of a specific profile.
pub fn run_show(name: &str, json: bool) -> Result<()> {
    let store = load_store()?;
    let profile = store.profiles.iter().find(|p| p.name == name);

    match profile {
        Some(p) => {
            if json {
                let output = serde_json::to_string_pretty(p)
                    .map_err(|e| McpRegError::Validation(e.to_string()))?;
                println!("{output}");
            } else {
                println!("📦 Profile: {}", p.name);
                if !p.description.is_empty() {
                    println!("   {}", p.description);
                }
                println!("   {} servers:", p.servers.len());
                for s in &p.servers {
                    println!("    • {s}");
                }
            }
            Ok(())
        }
        None => {
            eprintln!("Profile '{name}' not found.");
            std::process::exit(1);
        }
    }
}

/// Delete a saved profile.
pub fn run_delete(name: &str) -> Result<()> {
    let mut store = load_store()?;
    let before = store.profiles.len();
    store.profiles.retain(|p| p.name != name);

    if store.profiles.len() == before {
        eprintln!("Profile '{name}' not found.");
        std::process::exit(1);
    }

    save_store(&store)?;
    println!("✓ Profile '{name}' deleted.");
    Ok(())
}

/// Apply a profile: install all servers in the profile that aren't already installed.
pub async fn run_apply(name: &str, dry_run: bool) -> Result<()> {
    let store = load_store()?;
    let profile = store.profiles.iter().find(|p| p.name == name);

    let profile = match profile {
        Some(p) => p,
        None => {
            eprintln!("Profile '{name}' not found.");
            std::process::exit(1);
        }
    };

    let installed = load_installed()?;
    let installed_set: std::collections::HashSet<&str> =
        installed.iter().map(|s| s.as_str()).collect();
    let mut to_install: Vec<&str> = Vec::new();
    let mut already: Vec<&str> = Vec::new();

    for server in &profile.servers {
        if installed_set.contains(server.as_str()) {
            already.push(server);
        } else {
            to_install.push(server);
        }
    }

    if to_install.is_empty() {
        println!("✓ All {} servers from profile '{name}' are already installed.", profile.servers.len());
        return Ok(());
    }

    println!("Profile '{}': {} servers to install, {} already installed", name, to_install.len(), already.len());

    if dry_run {
        println!("\n[dry-run] Would install:");
        for s in &to_install {
            println!("  • {s}");
        }
        return Ok(());
    }

    for server in &to_install {
        println!("\n→ Installing {server}...");
        if let Err(e) = crate::commands::install::run(server).await {
            eprintln!("  ⚠ Failed to install {server}: {e}");
        }
    }

    println!("\n✓ Profile '{name}' applied.");
    Ok(())
}

/// Export a profile to a standalone JSON file.
pub fn run_export(name: &str, output: Option<&str>) -> Result<()> {
    let store = load_store()?;
    let profile = store.profiles.iter().find(|p| p.name == name);

    let profile = match profile {
        Some(p) => p,
        None => {
            eprintln!("Profile '{name}' not found.");
            std::process::exit(1);
        }
    };

    let json = serde_json::to_string_pretty(profile)
        .map_err(|e| McpRegError::Validation(e.to_string()))?;

    match output {
        Some(path) => {
            fs::write(path, &json).map_err(McpRegError::Io)?;
            println!("✓ Profile '{name}' exported to {path}");
        }
        None => {
            println!("{json}");
        }
    }
    Ok(())
}

/// Import a profile from a JSON file.
pub fn run_import(path: &str) -> Result<()> {
    let data = fs::read_to_string(path).map_err(McpRegError::Io)?;
    let profile: Profile =
        serde_json::from_str(&data).map_err(|e| McpRegError::Validation(e.to_string()))?;

    let mut store = load_store()?;
    // Replace if exists
    store.profiles.retain(|p| p.name != profile.name);
    let name = profile.name.clone();
    let count = profile.servers.len();
    store.profiles.push(profile);
    save_store(&store)?;

    println!("✓ Profile '{name}' imported with {count} servers.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profile_serialization() {
        let profile = Profile {
            name: "test".into(),
            description: "Test profile".into(),
            servers: vec!["org/tool-a".into(), "org/tool-b".into()],
            created_at: 1700000000,
            metadata: HashMap::new(),
        };
        let json = serde_json::to_string(&profile).unwrap();
        let deserialized: Profile = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, "test");
        assert_eq!(deserialized.servers.len(), 2);
        assert_eq!(deserialized.created_at, 1700000000);
    }

    #[test]
    fn test_profile_store_default() {
        let store = ProfileStore::default();
        assert!(store.profiles.is_empty());
    }

    #[test]
    fn test_profile_with_metadata() {
        let mut metadata = HashMap::new();
        metadata.insert("author".into(), "test-user".into());
        metadata.insert("use-case".into(), "data-science".into());

        let profile = Profile {
            name: "data-science".into(),
            description: "Data science stack".into(),
            servers: vec!["org/postgres".into()],
            created_at: 0,
            metadata,
        };

        let json = serde_json::to_string(&profile).unwrap();
        let deserialized: Profile = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.metadata.get("author").unwrap(), "test-user");
    }

    #[test]
    fn test_profile_store_roundtrip() {
        let store = ProfileStore {
            profiles: vec![
                Profile {
                    name: "web".into(),
                    description: "Web dev".into(),
                    servers: vec!["org/fetch".into()],
                    created_at: 100,
                    metadata: HashMap::new(),
                },
                Profile {
                    name: "db".into(),
                    description: "Database".into(),
                    servers: vec!["org/postgres".into(), "org/sqlite".into()],
                    created_at: 200,
                    metadata: HashMap::new(),
                },
            ],
        };
        let json = serde_json::to_string(&store).unwrap();
        let deserialized: ProfileStore = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.profiles.len(), 2);
        assert_eq!(deserialized.profiles[1].servers.len(), 2);
    }
}
