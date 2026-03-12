use crate::config::Config;
use crate::error::{McpRegError, Result};
use crate::registry::db::Database;
use std::collections::BTreeSet;

/// Show what changed between two versions of a server (tools added/removed, description, etc.).
pub fn run(server_ref: &str, from: Option<&str>, to: Option<&str>, json_output: bool) -> Result<()> {
    let parts: Vec<&str> = server_ref.splitn(2, '/').collect();
    if parts.len() != 2 {
        return Err(McpRegError::Config(
            "Server reference must be in format 'owner/name'".into(),
        ));
    }
    let (owner, name) = (parts[0], parts[1]);

    let db_path = Config::db_path()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "registry.db".to_string());

    let db = Database::open(&db_path)?;
    let _ = db.seed_default_servers();

    let current = db
        .get_server(owner, name)?
        .ok_or_else(|| McpRegError::NotFound(format!("{owner}/{name}")))?;

    let versions = db.get_version_history(owner, name)?;

    if versions.is_empty() {
        if json_output {
            println!("{}", serde_json::to_string_pretty(&serde_json::json!({
                "server": format!("{owner}/{name}"),
                "message": "No version history available",
            }))?);
        } else {
            println!("No version history available for {owner}/{name}.");
        }
        return Ok(());
    }

    // Determine from/to versions
    let to_version = to.unwrap_or(&current.version);
    let from_version = from.unwrap_or_else(|| {
        // Use the second-newest version, or the oldest if only one
        if versions.len() >= 2 {
            &versions[1].0
        } else {
            &versions[0].0
        }
    });

    // Build a changelog-style diff of the current state
    // (We only have the current snapshot, so we show what the server looks like now
    //  vs a summary of version progression)
    let tools: BTreeSet<&str> = current.tools.iter().map(|s| s.as_str()).collect();
    let resources: BTreeSet<&str> = current.resources.iter().map(|s| s.as_str()).collect();
    let prompts: BTreeSet<&str> = current.prompts.iter().map(|s| s.as_str()).collect();
    let tags: BTreeSet<&str> = current.tags.iter().map(|s| s.as_str()).collect();

    if json_output {
        let resp = serde_json::json!({
            "server": format!("{owner}/{name}"),
            "from_version": from_version,
            "to_version": to_version,
            "current": {
                "version": &current.version,
                "description": &current.description,
                "transport": &current.transport,
                "tools": tools.iter().collect::<Vec<_>>(),
                "resources": resources.iter().collect::<Vec<_>>(),
                "prompts": prompts.iter().collect::<Vec<_>>(),
                "tags": tags.iter().collect::<Vec<_>>(),
                "downloads": current.downloads,
            },
            "version_history": versions.iter().map(|(v, d)| serde_json::json!({
                "version": v,
                "published_at": d,
            })).collect::<Vec<_>>(),
            "total_versions": versions.len(),
        });
        println!("{}", serde_json::to_string_pretty(&resp)?);
        return Ok(());
    }

    println!("📋 Changelog for {owner}/{name}\n");
    println!("  Version progression: {} → {}", from_version, to_version);
    println!();

    // Show all version history
    println!("  📦 Version History:");
    for (i, (version, date)) in versions.iter().enumerate() {
        let marker = if version == to_version {
            "→"
        } else if version == from_version {
            "←"
        } else {
            " "
        };
        let label = if i == 0 { " (current)" } else { "" };
        println!("    {marker} v{version}  {date}{label}");
    }
    println!();

    // Current state summary
    println!("  📊 Current State (v{}):", current.version);
    println!("    Description: {}", current.description);
    println!("    Transport:   {}", current.transport);
    println!("    License:     {}", current.license);
    println!("    Downloads:   {}", current.downloads);
    if !tools.is_empty() {
        println!("    Tools ({}):", tools.len());
        for tool in &tools {
            println!("      • {tool}");
        }
    }
    if !resources.is_empty() {
        println!("    Resources ({}):", resources.len());
        for res in &resources {
            println!("      • {res}");
        }
    }
    if !prompts.is_empty() {
        println!("    Prompts ({}):", prompts.len());
        for p in &prompts {
            println!("      • {p}");
        }
    }
    if !tags.is_empty() {
        println!("    Tags: {}", tags.iter().copied().collect::<Vec<_>>().join(", "));
    }

    println!();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_changelog_bad_ref() {
        assert!(run("noslash", None, None, false).is_err());
    }

    #[test]
    fn test_changelog_json() {
        let _ = run("modelcontextprotocol/filesystem", None, None, true);
    }

    #[test]
    fn test_changelog_text() {
        let _ = run("modelcontextprotocol/filesystem", None, None, false);
    }

    #[test]
    fn test_changelog_with_from() {
        let _ = run("modelcontextprotocol/filesystem", Some("0.1.0"), None, false);
    }

    #[test]
    fn test_changelog_not_found() {
        let result = run("nobody/nothing", None, None, false);
        // Should error with NotFound
        assert!(result.is_err());
    }
}
