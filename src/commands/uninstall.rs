use crate::api::types::InstalledServers;
use crate::config::Config;
use crate::error::{McpRegError, Result};
use serde_json::Value;

pub fn run(server_ref: &str) -> Result<()> {
    let parts: Vec<&str> = server_ref.splitn(2, '/').collect();
    if parts.len() != 2 {
        return Err(McpRegError::Config(
            "Server reference must be in format 'owner/name'".into(),
        ));
    }
    let (owner, name) = (parts[0], parts[1]);

    let installed_path = Config::installed_servers_path()?;
    if !installed_path.exists() {
        return Err(McpRegError::NotFound(format!(
            "{owner}/{name} is not installed"
        )));
    }

    let content = std::fs::read_to_string(&installed_path)?;
    let mut installed: InstalledServers = serde_json::from_str(&content)?;

    let before = installed.servers.len();
    installed
        .servers
        .retain(|s| !(s.owner == owner && s.name == name));

    if installed.servers.len() == before {
        return Err(McpRegError::NotFound(format!(
            "{owner}/{name} is not installed"
        )));
    }

    std::fs::write(&installed_path, serde_json::to_string_pretty(&installed)?)?;

    // Remove from claude_desktop_config.json
    remove_from_claude_config(name)?;

    println!("✓ Uninstalled {owner}/{name}");
    println!("  Removed from claude_desktop_config.json");
    Ok(())
}

fn remove_from_claude_config(name: &str) -> Result<()> {
    let config_path = Config::claude_desktop_config_path()?;
    if !config_path.exists() {
        return Ok(());
    }

    let content = std::fs::read_to_string(&config_path)?;
    let mut config: Value = serde_json::from_str(&content)?;

    if let Some(obj) = config.as_object_mut() {
        if let Some(mcp) = obj.get_mut("mcpServers") {
            if let Some(mcp_obj) = mcp.as_object_mut() {
                mcp_obj.remove(name);
            }
        }
    }

    std::fs::write(&config_path, serde_json::to_string_pretty(&config)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uninstall_not_installed() {
        // Should fail gracefully when nothing is installed
        let result = run("nobody/nothing");
        assert!(result.is_err());
    }
}
