use crate::api::types::InstalledServers;
use crate::config::Config;
use crate::error::Result;
use serde_json::json;

pub fn run(output_path: Option<&str>) -> Result<()> {
    let path = Config::installed_servers_path()?;

    if !path.exists() {
        println!("No servers installed. Nothing to export.");
        return Ok(());
    }

    let content = std::fs::read_to_string(&path)?;
    let installed: InstalledServers = serde_json::from_str(&content)?;

    if installed.servers.is_empty() {
        println!("No servers installed. Nothing to export.");
        return Ok(());
    }

    // Build mcpServers config object
    let mut mcp_servers = serde_json::Map::new();
    for server in &installed.servers {
        let mut server_config = serde_json::Map::new();
        server_config.insert("command".into(), json!(server.command));
        server_config.insert("args".into(), json!(server.args));
        mcp_servers.insert(server.name.clone(), serde_json::Value::Object(server_config));
    }

    let config = json!({ "mcpServers": mcp_servers });
    let output = serde_json::to_string_pretty(&config)?;

    match output_path {
        Some(path) => {
            std::fs::write(path, &output)?;
            println!("✓ Exported {} server(s) to {path}", installed.servers.len());
        }
        None => {
            println!("{output}");
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_export_no_servers() {
        // Should not panic when no installed.json exists
        let _ = run(None);
    }

    #[test]
    fn test_export_to_file() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("export.json");
        // Will print "no servers" but not error
        let _ = run(Some(out.to_str().unwrap()));
    }
}
