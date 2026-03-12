use crate::api::types::InstalledServers;
use crate::config::Config;
use crate::error::{McpRegError, Result};
use serde::{Deserialize, Serialize};

/// Backup format — includes installed servers + pinned state + config snapshot.
#[derive(Debug, Serialize, Deserialize)]
pub struct McpRegBackup {
    pub version: String,
    pub created_at: String,
    pub installed: InstalledServers,
    pub claude_config: Option<serde_json::Value>,
}

/// Export a complete backup of mcpreg state.
pub fn run_backup(output: Option<&str>) -> Result<()> {
    let installed_path = Config::installed_servers_path()?;
    let installed: InstalledServers = if installed_path.exists() {
        let content = std::fs::read_to_string(&installed_path)?;
        serde_json::from_str(&content)?
    } else {
        InstalledServers::default()
    };

    // Read claude_desktop_config.json if it exists
    let claude_config = Config::claude_desktop_config_path()
        .ok()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|c| serde_json::from_str(&c).ok());

    let now = crate::commands::install::chrono_now_public();

    let backup = McpRegBackup {
        version: env!("CARGO_PKG_VERSION").to_string(),
        created_at: now,
        installed,
        claude_config,
    };

    let json = serde_json::to_string_pretty(&backup)?;

    if let Some(path) = output {
        std::fs::write(path, &json)?;
        println!("✓ Backup saved to {path}");
        println!(
            "  {} server(s) backed up",
            backup.installed.servers.len()
        );
    } else {
        println!("{json}");
    }

    Ok(())
}

/// Restore from a backup file.
pub fn run_restore(input: &str, dry_run: bool) -> Result<()> {
    let content = std::fs::read_to_string(input)
        .map_err(|_| McpRegError::Config(format!("Cannot read backup file: {input}")))?;

    let backup: McpRegBackup = serde_json::from_str(&content)
        .map_err(|e| McpRegError::Config(format!("Invalid backup format: {e}")))?;

    println!(
        "Backup from {} (mcpreg v{})",
        backup.created_at, backup.version
    );
    println!(
        "  {} server(s) to restore",
        backup.installed.servers.len()
    );

    if backup.installed.servers.is_empty() {
        println!("Nothing to restore.");
        return Ok(());
    }

    for server in &backup.installed.servers {
        println!(
            "  • {}/{} v{} ({})",
            server.owner, server.name, server.version, server.transport
        );
    }

    if dry_run {
        println!("\nDry run — no changes made.");
        return Ok(());
    }

    // Write installed servers
    let installed_path = Config::installed_servers_path()?;
    let dir = Config::config_dir()?;
    std::fs::create_dir_all(&dir)?;
    std::fs::write(
        &installed_path,
        serde_json::to_string_pretty(&backup.installed)?,
    )?;

    // Restore claude_desktop_config.json if present in backup
    if let Some(claude_config) = &backup.claude_config {
        if let Ok(config_path) = Config::claude_desktop_config_path() {
            if let Some(parent) = config_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(
                &config_path,
                serde_json::to_string_pretty(claude_config)?,
            )?;
            println!("  ✓ Restored claude_desktop_config.json");
        }
    }

    println!(
        "\n✓ Restored {} server(s)",
        backup.installed.servers.len()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backup_format_roundtrip() {
        let backup = McpRegBackup {
            version: "0.8.0".into(),
            created_at: "2025-01-01T00:00:00Z".into(),
            installed: InstalledServers::default(),
            claude_config: Some(serde_json::json!({"mcpServers": {}})),
        };
        let json = serde_json::to_string(&backup).unwrap();
        let back: McpRegBackup = serde_json::from_str(&json).unwrap();
        assert_eq!(back.version, "0.8.0");
        assert!(back.installed.servers.is_empty());
    }

    #[test]
    fn test_backup_no_servers() {
        // Should work even when nothing is installed
        let _ = run_backup(None);
    }

    #[test]
    fn test_restore_invalid_file() {
        let result = run_restore("/nonexistent/file.json", true);
        assert!(result.is_err());
    }

    #[test]
    fn test_restore_dry_run() {
        // Create a temp backup
        let backup = McpRegBackup {
            version: "0.8.0".into(),
            created_at: "2025-01-01T00:00:00Z".into(),
            installed: InstalledServers::default(),
            claude_config: None,
        };
        let dir = std::env::temp_dir();
        let path = dir.join("mcpreg_test_backup.json");
        std::fs::write(&path, serde_json::to_string(&backup).unwrap()).unwrap();

        let result = run_restore(path.to_str().unwrap(), true);
        assert!(result.is_ok());

        let _ = std::fs::remove_file(&path);
    }
}
