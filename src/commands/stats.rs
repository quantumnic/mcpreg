use crate::config::Config;
use crate::error::Result;
use crate::registry::db::Database;

pub fn run(json_output: bool) -> Result<()> {
    let db_path = Config::db_path()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "registry.db".to_string());

    let db = Database::open(&db_path)?;

    match db.seed_default_servers() {
        Ok(0) => {}
        Ok(n) => {
            if !json_output {
                eprintln!("ℹ  Seeded {n} default servers into local registry.");
            }
        }
        Err(e) => {
            if !json_output {
                eprintln!("⚠  Could not seed defaults: {e}");
            }
        }
    }

    let stats = db.stats()?;

    if json_output {
        let v = serde_json::json!({
            "total_servers": stats.total_servers,
            "total_downloads": stats.total_downloads,
            "unique_owners": stats.unique_owners,
            "avg_tools": stats.avg_tools,
            "top_servers": stats.top_servers.iter().map(|(n, d)| serde_json::json!({"name": n, "downloads": d})).collect::<Vec<_>>(),
            "transports": stats.transport_counts.iter().map(|(t, c)| serde_json::json!({"transport": t, "count": c})).collect::<Vec<_>>(),
        });
        println!("{}", serde_json::to_string_pretty(&v)?);
        return Ok(());
    }

    println!("╔══════════════════════════════════════════╗");
    println!("║         mcpreg Registry Stats            ║");
    println!("╚══════════════════════════════════════════╝");
    println!();
    println!("  Total servers:    {}", stats.total_servers);
    println!("  Total downloads:  {}", stats.total_downloads);
    println!("  Unique owners:    {}", stats.unique_owners);
    println!("  Avg tools/server: {:.1}", stats.avg_tools);
    println!();

    if !stats.top_servers.is_empty() {
        println!("  Top 5 by downloads:");
        for (i, (name, downloads)) in stats.top_servers.iter().enumerate() {
            println!("    {}. {} ({} downloads)", i + 1, name, downloads);
        }
        println!();
    }

    if !stats.transport_counts.is_empty() {
        println!("  Transports:");
        for (transport, count) in &stats.transport_counts {
            println!("    {transport}: {count} server(s)");
        }
    }

    Ok(())
}
