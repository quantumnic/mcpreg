use crate::api::types::ServerEntry;
use crate::config::Config;
use crate::error::{McpRegError, Result};
use crate::registry::db::Database;
use serde::{Deserialize, Serialize};

/// A portable bundle of MCP servers that can be shared and imported.
#[derive(Debug, Serialize, Deserialize)]
pub struct ServerBundle {
    /// Bundle format version
    pub bundle_version: String,
    /// Human-readable name for this bundle
    pub name: String,
    /// Optional description
    pub description: String,
    /// Who created this bundle
    pub author: String,
    /// When the bundle was created
    pub created_at: String,
    /// The servers in this bundle
    pub servers: Vec<BundleEntry>,
}

/// A single server entry in a bundle.
#[derive(Debug, Serialize, Deserialize)]
pub struct BundleEntry {
    pub owner: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub transport: String,
    pub tools: Vec<String>,
    pub tags: Vec<String>,
}

impl From<&ServerEntry> for BundleEntry {
    fn from(s: &ServerEntry) -> Self {
        Self {
            owner: s.owner.clone(),
            name: s.name.clone(),
            version: s.version.clone(),
            description: s.description.clone(),
            transport: s.transport.clone(),
            tools: s.tools.clone(),
            tags: s.tags.clone(),
        }
    }
}

/// Create a bundle from a list of server references.
pub fn run_create(
    bundle_name: &str,
    server_refs: &[String],
    description: Option<&str>,
    author: Option<&str>,
    output: Option<&str>,
    json_output: bool,
) -> Result<()> {
    if server_refs.is_empty() {
        return Err(McpRegError::Config(
            "At least one server reference is required".into(),
        ));
    }

    let db_path = Config::db_path()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "registry.db".to_string());
    let db = Database::open(&db_path)?;
    let _ = db.seed_default_servers();

    let mut entries = Vec::new();
    let mut not_found = Vec::new();

    for server_ref in server_refs {
        let parts: Vec<&str> = server_ref.splitn(2, '/').collect();
        if parts.len() != 2 {
            eprintln!("⚠ Invalid reference (expected owner/name): {server_ref}");
            continue;
        }
        match db.get_server(parts[0], parts[1])? {
            Some(server) => entries.push(BundleEntry::from(&server)),
            None => not_found.push(server_ref.clone()),
        }
    }

    if !not_found.is_empty() && !json_output {
        for nf in &not_found {
            eprintln!("⚠ Server not found: {nf}");
        }
    }

    if entries.is_empty() {
        return Err(McpRegError::Config("No valid servers found for bundle".into()));
    }

    let now = crate::commands::install::chrono_now_public();
    let bundle = ServerBundle {
        bundle_version: "1".into(),
        name: bundle_name.to_string(),
        description: description.unwrap_or("").to_string(),
        author: author.unwrap_or("mcpreg").to_string(),
        created_at: now,
        servers: entries,
    };

    let json = serde_json::to_string_pretty(&bundle)?;

    if let Some(path) = output {
        std::fs::write(path, &json)?;
        if !json_output {
            println!("✓ Bundle '{}' saved to {path}", bundle.name);
            println!("  {} server(s) bundled", bundle.servers.len());
        }
    } else {
        println!("{json}");
    }

    Ok(())
}

/// List the contents of a bundle file.
pub fn run_inspect(path: &str, json_output: bool) -> Result<()> {
    let content = std::fs::read_to_string(path)
        .map_err(|_| McpRegError::Config(format!("Cannot read bundle file: {path}")))?;

    let bundle: ServerBundle = serde_json::from_str(&content)
        .map_err(|e| McpRegError::Config(format!("Invalid bundle format: {e}")))?;

    if json_output {
        println!("{}", serde_json::to_string_pretty(&bundle)?);
        return Ok(());
    }

    println!("📦 Bundle: {}", bundle.name);
    if !bundle.description.is_empty() {
        println!("   {}", bundle.description);
    }
    println!("   Author: {}", bundle.author);
    println!("   Created: {}", bundle.created_at);
    println!("   Servers: {}\n", bundle.servers.len());

    for (i, entry) in bundle.servers.iter().enumerate() {
        println!(
            "  {}. {}/{} v{} — {}",
            i + 1,
            entry.owner,
            entry.name,
            entry.version,
            entry.description,
        );
        if !entry.tools.is_empty() {
            let tools_display: Vec<_> = entry.tools.iter().take(5).cloned().collect();
            let suffix = if entry.tools.len() > 5 {
                format!(" (+{} more)", entry.tools.len() - 5)
            } else {
                String::new()
            };
            println!("     Tools: {}{suffix}", tools_display.join(", "));
        }
        if !entry.tags.is_empty() {
            println!("     Tags: {}", entry.tags.join(", "));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bundle_roundtrip() {
        let bundle = ServerBundle {
            bundle_version: "1".into(),
            name: "test-bundle".into(),
            description: "A test bundle".into(),
            author: "tester".into(),
            created_at: "2025-01-01T00:00:00Z".into(),
            servers: vec![BundleEntry {
                owner: "org".into(),
                name: "tool".into(),
                version: "1.0.0".into(),
                description: "A tool".into(),
                transport: "stdio".into(),
                tools: vec!["read".into(), "write".into()],
                tags: vec!["utility".into()],
            }],
        };

        let json = serde_json::to_string(&bundle).unwrap();
        let back: ServerBundle = serde_json::from_str(&json).unwrap();
        assert_eq!(back.name, "test-bundle");
        assert_eq!(back.servers.len(), 1);
        assert_eq!(back.servers[0].tools.len(), 2);
    }

    #[test]
    fn test_bundle_empty_servers_error() {
        let result = run_create("empty", &[], None, None, None, false);
        assert!(result.is_err());
    }

    #[test]
    fn test_bundle_inspect_missing_file() {
        let result = run_inspect("/nonexistent/file.json", false);
        assert!(result.is_err());
    }

    #[test]
    fn test_bundle_inspect_invalid_json() {
        let dir = std::env::temp_dir();
        let path = dir.join("mcpreg_test_bad_bundle.json");
        std::fs::write(&path, "not json").unwrap();
        let result = run_inspect(path.to_str().unwrap(), false);
        assert!(result.is_err());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_bundle_inspect_valid() {
        let bundle = ServerBundle {
            bundle_version: "1".into(),
            name: "inspect-test".into(),
            description: "".into(),
            author: "test".into(),
            created_at: "2025-06-01T00:00:00Z".into(),
            servers: vec![BundleEntry {
                owner: "acme".into(),
                name: "gadget".into(),
                version: "2.0.0".into(),
                description: "A gadget".into(),
                transport: "sse".into(),
                tools: vec![],
                tags: vec![],
            }],
        };
        let dir = std::env::temp_dir();
        let path = dir.join("mcpreg_test_inspect_bundle.json");
        std::fs::write(&path, serde_json::to_string(&bundle).unwrap()).unwrap();
        let result = run_inspect(path.to_str().unwrap(), false);
        assert!(result.is_ok());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_bundle_inspect_json_output() {
        let bundle = ServerBundle {
            bundle_version: "1".into(),
            name: "json-test".into(),
            description: "desc".into(),
            author: "a".into(),
            created_at: "2025-01-01".into(),
            servers: vec![],
        };
        let dir = std::env::temp_dir();
        let path = dir.join("mcpreg_test_inspect_json.json");
        std::fs::write(&path, serde_json::to_string(&bundle).unwrap()).unwrap();
        let result = run_inspect(path.to_str().unwrap(), true);
        assert!(result.is_ok());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_bundle_create_with_seeded_data() {
        let servers = vec![
            "modelcontextprotocol/filesystem".to_string(),
            "modelcontextprotocol/git".to_string(),
        ];
        let dir = std::env::temp_dir();
        let outpath = dir.join("mcpreg_test_create_bundle.json");
        let result = run_create(
            "dev-tools",
            &servers,
            Some("Development tools"),
            Some("tester"),
            Some(outpath.to_str().unwrap()),
            false,
        );
        // Should succeed with seeded data
        if result.is_ok() {
            let content = std::fs::read_to_string(&outpath).unwrap();
            let bundle: ServerBundle = serde_json::from_str(&content).unwrap();
            assert_eq!(bundle.name, "dev-tools");
            assert!(!bundle.servers.is_empty());
        }
        let _ = std::fs::remove_file(&outpath);
    }

    #[test]
    fn test_bundle_entry_from_server_entry() {
        let server = crate::api::types::ServerEntry {
            id: None,
            owner: "alice".into(),
            name: "magic".into(),
            version: "3.0.0".into(),
            description: "Magic server".into(),
            author: "alice".into(),
            license: "MIT".into(),
            repository: "".into(),
            command: "node".into(),
            args: vec![],
            transport: "stdio".into(),
            tools: vec!["spell".into()],
            resources: vec![],
            prompts: vec![],
            tags: vec!["magic".into()],
            env: Default::default(),
            homepage: "".into(),
            deprecated: false,
            deprecated_by: None,
            downloads: 100,
            stars: 5,
            created_at: None,
            updated_at: None,
        };
        let entry = BundleEntry::from(&server);
        assert_eq!(entry.owner, "alice");
        assert_eq!(entry.name, "magic");
        assert_eq!(entry.tools, vec!["spell"]);
    }
}
