use crate::api::types::InstalledServers;
use crate::config::Config;
use crate::error::{McpRegError, Result};
use crate::registry::db::Database;
use crate::registry::seed::server_category;

/// Explain why a particular server would be useful, based on installed servers.
pub fn run(server_ref: &str, json_output: bool) -> Result<()> {
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

    let target = db
        .get_server(owner, name)?
        .ok_or_else(|| McpRegError::NotFound(format!("{owner}/{name} not found in registry")))?;

    let installed_path = Config::installed_servers_path()?;
    let installed: InstalledServers = if installed_path.exists() {
        let content = std::fs::read_to_string(&installed_path)?;
        serde_json::from_str(&content)?
    } else {
        InstalledServers::default()
    };

    let target_cat = server_category(&target.owner, &target.name);
    let mut reasons: Vec<String> = Vec::new();

    // Check if already installed
    let already_installed = installed
        .servers
        .iter()
        .any(|s| s.owner == owner && s.name == name);

    if already_installed {
        reasons.push("You already have this server installed.".into());
    }

    // Category siblings
    let category_siblings: Vec<String> = installed
        .servers
        .iter()
        .filter(|s| {
            let cat = server_category(&s.owner, &s.name);
            cat == target_cat && !(s.owner == owner && s.name == name)
        })
        .map(|s| s.full_name())
        .collect();

    if !category_siblings.is_empty() {
        reasons.push(format!(
            "Same category ({target_cat}) as your installed: {}",
            category_siblings.join(", ")
        ));
    }

    // Shared tools with installed servers
    for s in &installed.servers {
        if s.owner == owner && s.name == name {
            continue;
        }
        if let Ok(Some(entry)) = db.get_server(&s.owner, &s.name) {
            let shared: Vec<_> = target
                .tools
                .iter()
                .filter(|t| entry.tools.contains(t))
                .cloned()
                .collect();
            if !shared.is_empty() {
                reasons.push(format!(
                    "Shares {} tool(s) with {}: {}",
                    shared.len(),
                    entry.full_name(),
                    shared.join(", ")
                ));
            }
        }
    }

    // Unique tools this server adds
    let mut all_installed_tools: std::collections::HashSet<String> =
        std::collections::HashSet::new();
    for s in &installed.servers {
        if let Ok(Some(entry)) = db.get_server(&s.owner, &s.name) {
            for t in &entry.tools {
                all_installed_tools.insert(t.clone());
            }
        }
    }

    let new_tools: Vec<_> = target
        .tools
        .iter()
        .filter(|t| !all_installed_tools.contains(t.as_str()))
        .cloned()
        .collect();

    if !new_tools.is_empty() {
        reasons.push(format!(
            "Adds {} new tool(s) you don't have: {}",
            new_tools.len(),
            new_tools.join(", ")
        ));
    }

    // Popularity
    if target.downloads >= 10000 {
        reasons.push(format!(
            "Popular server with {} downloads",
            target.downloads
        ));
    }

    if json_output {
        let result = serde_json::json!({
            "server": target.full_name(),
            "version": target.version,
            "description": target.description,
            "category": target_cat,
            "already_installed": already_installed,
            "reasons": reasons,
            "tools": target.tools,
            "new_tools": new_tools,
        });
        println!("{}", serde_json::to_string_pretty(&result)?);
        return Ok(());
    }

    println!("Why install {}?\n", target.full_name());
    println!("  {} v{}", target.full_name(), target.version);
    println!("  {}", target.description);
    println!("  Category: {target_cat}");
    println!("  ⬇ {} downloads\n", target.downloads);

    if reasons.is_empty() {
        println!("  No specific reasons found based on your installed servers.");
        println!("  Try it out: mcpreg install {}", target.full_name());
    } else {
        println!("  Reasons:");
        for reason in &reasons {
            println!("    • {reason}");
        }
    }
    println!();

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_why_no_panic_on_missing() {
        // The function requires config paths which may not exist in test,
        // so we just verify it doesn't panic
        let result = super::run("nobody/nothing", false);
        // Should be Err (not found or config issue) but not panic
        assert!(result.is_err());
    }
}
