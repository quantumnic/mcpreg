use crate::api::client::RegistryClient;
use crate::api::types::{InstalledServer, InstalledServers};
use crate::config::Config;
use crate::error::{McpRegError, Result};
use serde_json::Value;

pub async fn run(server_ref: &str) -> Result<()> {
    let parts: Vec<&str> = server_ref.splitn(2, '/').collect();
    if parts.len() != 2 {
        return Err(McpRegError::Config(
            "Server reference must be in format 'owner/name'".into(),
        ));
    }
    let (owner, name) = (parts[0], parts[1]);

    let config = Config::load()?;
    let client = RegistryClient::new(&config);

    println!("Fetching {owner}/{name} from registry...");
    let entry = client.get_server(owner, name).await?;

    // Update installed servers list
    let installed_path = Config::installed_servers_path()?;
    let mut installed: InstalledServers = if installed_path.exists() {
        let content = std::fs::read_to_string(&installed_path)?;
        serde_json::from_str(&content)?
    } else {
        InstalledServers::default()
    };

    // Check if already installed
    if let Some(existing) = installed.servers.iter().find(|s| s.owner == owner && s.name == name) {
        println!(
            "Server {}/{} is already installed (v{}). Updating...",
            existing.owner, existing.name, existing.version
        );
        installed.servers.retain(|s| !(s.owner == owner && s.name == name));
    }

    let now = chrono_now();
    installed.servers.push(InstalledServer {
        owner: entry.owner.clone(),
        name: entry.name.clone(),
        version: entry.version.clone(),
        command: entry.command.clone(),
        args: entry.args.clone(),
        transport: entry.transport.clone(),
        installed_at: now,
    });

    let dir = Config::config_dir()?;
    std::fs::create_dir_all(&dir)?;
    std::fs::write(&installed_path, serde_json::to_string_pretty(&installed)?)?;

    // Update claude_desktop_config.json
    update_claude_config(&entry.name, &entry.command, &entry.args)?;

    println!(
        "✓ Installed {}/{} v{}\n  Command: {} {}",
        entry.owner,
        entry.name,
        entry.version,
        entry.command,
        entry.args.join(" ")
    );
    println!("  Config updated in claude_desktop_config.json");

    Ok(())
}

fn update_claude_config(name: &str, command: &str, args: &[String]) -> Result<()> {
    let config_path = Config::claude_desktop_config_path()?;

    let mut config: Value = if config_path.exists() {
        let content = std::fs::read_to_string(&config_path)?;
        serde_json::from_str(&content)?
    } else {
        serde_json::json!({})
    };

    let mcp_servers = config
        .as_object_mut()
        .ok_or_else(|| McpRegError::Config("Invalid claude config format".into()))?
        .entry("mcpServers")
        .or_insert_with(|| serde_json::json!({}));

    let server_config = serde_json::json!({
        "command": command,
        "args": args,
    });

    mcp_servers
        .as_object_mut()
        .ok_or_else(|| McpRegError::Config("Invalid mcpServers format".into()))?
        .insert(name.to_string(), server_config);

    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&config_path, serde_json::to_string_pretty(&config)?)?;

    Ok(())
}

fn chrono_now() -> String {
    // Simple ISO timestamp without chrono dependency
    let duration = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}Z", duration.as_secs())
}

#[allow(dead_code)]
pub fn parse_server_ref(server_ref: &str) -> Result<(String, String)> {
    let parts: Vec<&str> = server_ref.splitn(2, '/').collect();
    if parts.len() != 2 {
        return Err(McpRegError::Config(
            "Server reference must be in format 'owner/name'".into(),
        ));
    }
    Ok((parts[0].to_string(), parts[1].to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_server_ref_valid() {
        let (owner, name) = parse_server_ref("alice/filesystem").unwrap();
        assert_eq!(owner, "alice");
        assert_eq!(name, "filesystem");
    }

    #[test]
    fn test_parse_server_ref_invalid() {
        assert!(parse_server_ref("no-slash").is_err());
    }

    #[test]
    fn test_parse_server_ref_with_multiple_slashes() {
        let (owner, name) = parse_server_ref("org/sub/name").unwrap();
        assert_eq!(owner, "org");
        assert_eq!(name, "sub/name");
    }
}
