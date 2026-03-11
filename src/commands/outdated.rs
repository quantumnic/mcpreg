use crate::api::types::InstalledServers;
use crate::config::Config;
use crate::error::Result;

pub fn run(json_output: bool) -> Result<()> {
    let path = Config::installed_servers_path()?;
    if !path.exists() {
        if json_output {
            println!("{{\"outdated\":[],\"total\":0}}");
        } else {
            println!("No servers installed.");
        }
        return Ok(());
    }

    let content = std::fs::read_to_string(&path)?;
    let installed: InstalledServers = serde_json::from_str(&content)?;

    if installed.servers.is_empty() {
        if json_output {
            println!("{{\"outdated\":[],\"total\":0}}");
        } else {
            println!("No servers installed.");
        }
        return Ok(());
    }

    // Compare against local DB (offline check)
    let db_path = Config::db_path()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "registry.db".to_string());

    let db = crate::registry::db::Database::open(&db_path)?;
    let _ = db.seed_default_servers();

    let mut outdated = Vec::new();

    for server in &installed.servers {
        if let Ok(Some(registry_entry)) = db.get_server(&server.owner, &server.name) {
            let ordering = crate::compare_versions(&registry_entry.version, &server.version);
            if ordering == std::cmp::Ordering::Greater {
                outdated.push((server, registry_entry));
            }
        }
    }

    if json_output {
        let items: Vec<serde_json::Value> = outdated
            .iter()
            .map(|(installed, latest)| {
                serde_json::json!({
                    "name": installed.full_name(),
                    "installed_version": installed.version,
                    "latest_version": latest.version,
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "outdated": items,
                "total": outdated.len(),
                "checked": installed.servers.len(),
            }))?
        );
        return Ok(());
    }

    if outdated.is_empty() {
        println!("All {} installed server(s) are up to date. ✓", installed.servers.len());
        return Ok(());
    }

    println!("Outdated servers:\n");
    println!("  {:<35} {:<12} LATEST", "SERVER", "INSTALLED");
    println!("  {}", "─".repeat(65));
    for (installed, latest) in &outdated {
        println!(
            "  {:<35} {:<12} {}",
            installed.full_name(),
            installed.version,
            latest.version,
        );
    }
    println!(
        "\n{} of {} server(s) can be updated. Run 'mcpreg update' to update all.",
        outdated.len(),
        installed.servers.len()
    );

    Ok(())
}
