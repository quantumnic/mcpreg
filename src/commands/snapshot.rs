use crate::config::Config;
use crate::error::Result;
use crate::registry::db::Database;

/// Save the full registry state to a JSON file.
pub fn run_save(output: Option<&str>) -> Result<()> {
    let db_path = Config::db_path()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "registry.db".to_string());
    let db = Database::open(&db_path)?;

    let servers = db.export_all()?;
    let stats = db.stats()?;

    let snapshot = serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "created_at": chrono_now(),
        "stats": {
            "total_servers": stats.total_servers,
            "total_downloads": stats.total_downloads,
            "unique_owners": stats.unique_owners,
            "avg_tools": stats.avg_tools,
        },
        "servers": servers,
    });

    let json = serde_json::to_string_pretty(&snapshot)?;

    match output {
        Some(path) => {
            std::fs::write(path, &json)?;
            println!("✅ Snapshot saved to {path} ({} servers)", servers.len());
        }
        None => {
            println!("{json}");
        }
    }

    Ok(())
}

/// Restore registry state from a JSON snapshot file.
pub fn run_restore(file: &str, dry_run: bool) -> Result<()> {
    let content = std::fs::read_to_string(file)?;
    let snapshot: serde_json::Value = serde_json::from_str(&content)?;

    let servers = snapshot["servers"]
        .as_array()
        .ok_or_else(|| crate::error::McpRegError::Validation("Invalid snapshot: missing 'servers' array".into()))?;

    let entries: Vec<crate::api::types::ServerEntry> = servers
        .iter()
        .filter_map(|v| serde_json::from_value(v.clone()).ok())
        .collect();

    if dry_run {
        println!("Would restore {} servers from snapshot", entries.len());
        if let Some(created) = snapshot["created_at"].as_str() {
            println!("  Snapshot created: {created}");
        }
        if let Some(ver) = snapshot["version"].as_str() {
            println!("  mcpreg version: {ver}");
        }
        for entry in entries.iter().take(10) {
            println!("  • {} v{}", entry.full_name(), entry.version);
        }
        if entries.len() > 10 {
            println!("  ... and {} more", entries.len() - 10);
        }
        return Ok(());
    }

    let db_path = Config::db_path()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "registry.db".to_string());
    let db = Database::open(&db_path)?;

    let mut imported = 0;
    for entry in &entries {
        if db.upsert_server(entry).is_ok() {
            imported += 1;
        }
    }

    println!("✅ Restored {imported}/{} servers from snapshot", entries.len());

    Ok(())
}

/// Simple timestamp without pulling in chrono.
fn chrono_now() -> String {
    // Use a best-effort UTC timestamp
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    // Simple format: just the epoch, consumers can parse
    format!("{now}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chrono_now_returns_numeric() {
        let ts = chrono_now();
        assert!(ts.parse::<u64>().is_ok());
    }
}
