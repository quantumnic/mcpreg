use crate::api;
use crate::commands;
use crate::config::Config;
use crate::error::{McpRegError, Result};

/// Update installed MCP servers to their latest versions.
pub async fn run(target: Option<&str>, dry_run: bool) -> Result<()> {
    let path = Config::installed_servers_path()?;
    if !path.exists() {
        println!("No servers installed.");
        return Ok(());
    }

    let content = std::fs::read_to_string(&path)?;

    // Detect pinned servers (backward-compatible)
    let pinned_set: std::collections::HashSet<String> = {
        if let Ok(pinned) =
            serde_json::from_str::<commands::pin::PinnedInstalledServers>(&content)
        {
            pinned
                .servers
                .iter()
                .filter(|s| s.pinned)
                .map(|s| format!("{}/{}", s.owner, s.name))
                .collect()
        } else {
            std::collections::HashSet::new()
        }
    };

    let installed: api::types::InstalledServers = serde_json::from_str(&content)?;

    if installed.servers.is_empty() {
        println!("No servers installed.");
        return Ok(());
    }

    // Filter to a specific server if requested
    let servers_to_check: Vec<&api::types::InstalledServer> = if let Some(target_ref) = target {
        let parts: Vec<&str> = target_ref.splitn(2, '/').collect();
        if parts.len() != 2 {
            return Err(McpRegError::Config(
                "Server reference must be in format 'owner/name'".into(),
            ));
        }
        let (t_owner, t_name) = (parts[0], parts[1]);
        let filtered: Vec<_> = installed
            .servers
            .iter()
            .filter(|s| s.owner == t_owner && s.name == t_name)
            .collect();
        if filtered.is_empty() {
            return Err(McpRegError::NotFound(format!(
                "{t_owner}/{t_name} is not installed"
            )));
        }
        filtered
    } else {
        installed.servers.iter().collect()
    };

    if dry_run {
        println!(
            "Checking {} server(s) for updates (dry run)...\n",
            servers_to_check.len()
        );
    } else {
        println!(
            "Checking {} server(s) for updates...\n",
            servers_to_check.len()
        );
    }

    let cfg = Config::load()?;
    let client = api::client::RegistryClient::new(&cfg);
    let mut updated = 0;
    let mut skipped_pinned = 0;
    let mut up_to_date = 0;

    for server in &servers_to_check {
        let full_name = server.full_name();

        // Skip pinned servers unless explicitly targeted
        if target.is_none() && pinned_set.contains(&full_name) {
            println!(
                "  📌 {} is pinned at v{} (skipping)",
                full_name, server.version
            );
            skipped_pinned += 1;
            continue;
        }

        match client.get_server(&server.owner, &server.name).await {
            Ok(entry) => {
                if crate::compare_versions(&entry.version, &server.version)
                    == std::cmp::Ordering::Greater
                {
                    if dry_run {
                        println!(
                            "  ↑ {}: {} → {} (would update)",
                            full_name, server.version, entry.version
                        );
                    } else {
                        println!(
                            "  ↑ {}: {} → {}",
                            full_name, server.version, entry.version
                        );
                        if let Err(e) = commands::install::run(&full_name).await {
                            eprintln!("    Failed to update: {e}");
                        }
                    }
                    updated += 1;
                } else {
                    println!(
                        "  ✓ {} is up to date (v{})",
                        full_name, server.version
                    );
                    up_to_date += 1;
                }
            }
            Err(e) => {
                eprintln!("  ✗ {}: {e}", full_name);
            }
        }
    }

    println!();
    if dry_run {
        println!(
            "{updated} server(s) would be updated, {up_to_date} up to date."
        );
    } else {
        println!("{updated} server(s) updated, {up_to_date} up to date.");
    }
    if skipped_pinned > 0 {
        println!("{skipped_pinned} server(s) skipped (pinned).");
    }
    Ok(())
}
