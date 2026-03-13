use crate::config::Config;
use crate::error::Result;
use crate::registry::db::Database;

pub fn run(json_output: bool) -> Result<()> {
    let db_path = Config::db_path()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "registry.db".to_string());

    let db_exists = std::path::Path::new(&db_path).exists();
    let config_path = Config::config_path()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "(unknown)".to_string());
    let config_exists = std::path::Path::new(&config_path).exists();

    let (server_count, owner_count, total_downloads) = if db_exists {
        let db = Database::open(&db_path)?;
        let _ = db.seed_default_servers();
        let stats = db.stats()?;
        (stats.total_servers, stats.unique_owners, stats.total_downloads)
    } else {
        (0, 0, 0)
    };

    let installed_path = Config::installed_servers_path()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    let installed_count = std::fs::read_to_string(&installed_path)
        .ok()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .and_then(|v| v.as_object().map(|o| o.len()))
        .unwrap_or(0);

    if json_output {
        let v = serde_json::json!({
            "version": env!("CARGO_PKG_VERSION"),
            "db_path": db_path,
            "db_exists": db_exists,
            "config_path": config_path,
            "config_exists": config_exists,
            "registry": {
                "servers": server_count,
                "owners": owner_count,
                "total_downloads": total_downloads,
            },
            "installed_servers": installed_count,
        });
        println!("{}", serde_json::to_string_pretty(&v)?);
        return Ok(());
    }

    println!("╔══════════════════════════════════════════════════╗");
    println!("║               mcpreg status                     ║");
    println!("╚══════════════════════════════════════════════════╝");
    println!();
    println!("  Version:      {}", env!("CARGO_PKG_VERSION"));
    println!("  Database:     {} {}", db_path, if db_exists { "✓" } else { "✗ (not found)" });
    println!("  Config:       {} {}", config_path, if config_exists { "✓" } else { "✗" });
    println!();
    println!("  Registry:");
    println!("    Servers:    {server_count}");
    println!("    Owners:     {owner_count}");
    println!("    Downloads:  {total_downloads}");
    println!("    Installed:  {installed_count}");
    println!();

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_does_not_panic() {
        // May fail due to missing DB, that's OK
        let _ = run(false);
    }

    #[test]
    fn test_status_json() {
        let _ = run(true);
    }
}
