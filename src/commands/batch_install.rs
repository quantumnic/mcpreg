use crate::api::client::RegistryClient;
use crate::api::types::{InstalledServer, InstalledServers};
use crate::config::Config;
use crate::error::{McpRegError, Result};

/// Install multiple MCP servers in one go.
pub async fn run(servers: &[String], dry_run: bool) -> Result<()> {
    if servers.is_empty() {
        return Err(McpRegError::Validation("No servers specified".into()));
    }

    let quiet = std::env::var("MCPREG_QUIET").is_ok();

    // Validate all references first
    let mut refs: Vec<(&str, &str)> = Vec::new();
    for s in servers {
        let parts: Vec<&str> = s.splitn(2, '/').collect();
        if parts.len() != 2 {
            return Err(McpRegError::Config(format!(
                "Invalid server reference '{s}'. Must be owner/name"
            )));
        }
        refs.push((parts[0], parts[1]));
    }

    if dry_run {
        println!("Would install {} servers:", refs.len());
        for (owner, name) in &refs {
            println!("  • {owner}/{name}");
        }
        return Ok(());
    }

    let config = Config::load()?;
    let client = RegistryClient::new(&config);

    // Load installed servers
    let installed_path = Config::installed_servers_path()?;
    let mut installed: InstalledServers = if installed_path.exists() {
        let content = std::fs::read_to_string(&installed_path)?;
        serde_json::from_str(&content)?
    } else {
        InstalledServers::default()
    };

    let mut ok_count = 0usize;
    let mut fail_count = 0usize;

    for (owner, name) in &refs {
        if !quiet {
            print!("Installing {owner}/{name}...");
        }
        match client.get_server(owner, name).await {
            Ok(entry) => {
                // Remove old entry if upgrading
                installed.servers.retain(|s| !(s.owner == *owner && s.name == *name));
                installed.servers.push(InstalledServer {
                    owner: entry.owner.clone(),
                    name: entry.name.clone(),
                    version: entry.version.clone(),
                    command: entry.command.clone(),
                    args: entry.args.clone(),
                    transport: entry.transport.clone(),
                    installed_at: crate::commands::install::chrono_now_public(),
                });
                ok_count += 1;
                if !quiet {
                    println!(" ✓ v{}", entry.version);
                }
            }
            Err(e) => {
                fail_count += 1;
                if !quiet {
                    println!(" ✗ {e}");
                }
            }
        }
    }

    // Persist
    let dir = Config::config_dir()?;
    std::fs::create_dir_all(&dir)?;
    std::fs::write(&installed_path, serde_json::to_string_pretty(&installed)?)?;

    println!(
        "\nBatch install complete: {} succeeded, {} failed (of {} total)",
        ok_count,
        fail_count,
        refs.len()
    );

    if fail_count > 0 {
        Err(McpRegError::Registry(format!("{fail_count} server(s) failed to install")))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_batch_install_empty_list() {
        let result = run(&[], false).await;
        assert!(result.is_err());
        let msg = format!("{}", result.unwrap_err());
        assert!(msg.contains("No servers"));
    }

    #[tokio::test]
    async fn test_batch_install_bad_ref() {
        let result = run(&["no-slash".to_string()], false).await;
        assert!(result.is_err());
        let msg = format!("{}", result.unwrap_err());
        assert!(msg.contains("owner/name"));
    }

    #[tokio::test]
    async fn test_batch_install_dry_run() {
        let result = run(
            &["alice/foo".to_string(), "bob/bar".to_string()],
            true,
        ).await;
        assert!(result.is_ok());
    }
}
