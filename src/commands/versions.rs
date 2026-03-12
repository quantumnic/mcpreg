use crate::config::Config;
use crate::error::{McpRegError, Result};
use crate::registry::db::Database;

/// Show version history for a server.
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

    let entry = db
        .get_server(owner, name)?
        .ok_or_else(|| McpRegError::NotFound(format!("{owner}/{name}")))?;

    let versions = db.get_version_history(owner, name)?;

    if json_output {
        let v = serde_json::json!({
            "server": entry.full_name(),
            "current_version": entry.version,
            "versions": versions.iter().map(|(ver, ts)| serde_json::json!({
                "version": ver,
                "published_at": ts,
            })).collect::<Vec<_>>(),
            "total": versions.len(),
        });
        println!("{}", serde_json::to_string_pretty(&v)?);
        return Ok(());
    }

    println!("{} — version history\n", entry.full_name());
    println!("  Current: v{}\n", entry.version);

    if versions.is_empty() {
        println!("  No version history recorded.");
        return Ok(());
    }

    println!("  {:<15} PUBLISHED", "VERSION");
    println!("  {}", "─".repeat(40));
    for (ver, ts) in &versions {
        let marker = if *ver == entry.version { " ← current" } else { "" };
        println!("  v{ver:<13} {ts}{marker}");
    }
    println!("\n  {} version(s) total.", versions.len());

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_versions_bad_ref() {
        assert!(run("noslash", false).is_err());
    }

    #[test]
    fn test_versions_not_found() {
        // Will open DB but not find the server
        let result = run("nobody/nothing", false);
        assert!(result.is_err());
    }
}
