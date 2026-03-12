use crate::api::types::InstalledServers;
use crate::config::Config;
use crate::error::Result;
use crate::registry::db::Database;
use crate::registry::seed::server_category;
use std::collections::BTreeMap;

/// Quick one-liner overview of registry state.
pub fn run(json_output: bool) -> Result<()> {
    let db_path = Config::db_path()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "registry.db".to_string());

    let db = Database::open(&db_path)?;
    let _ = db.seed_default_servers();

    let stats = db.stats()?;
    let (servers, _) = db.list_servers(1, 1000)?;

    // Category breakdown
    let mut by_cat: BTreeMap<String, usize> = BTreeMap::new();
    for s in &servers {
        let cat = server_category(&s.owner, &s.name).to_string();
        *by_cat.entry(cat).or_default() += 1;
    }

    // Transport breakdown
    let mut by_transport: BTreeMap<String, usize> = BTreeMap::new();
    for s in &servers {
        *by_transport.entry(s.transport.clone()).or_default() += 1;
    }

    // Top 3 servers
    let mut sorted = servers.clone();
    sorted.sort_by(|a, b| b.downloads.cmp(&a.downloads));
    let top3: Vec<_> = sorted.iter().take(3).collect();

    // Installed count
    let installed_count = Config::installed_servers_path()
        .ok()
        .and_then(|p| {
            if p.exists() {
                std::fs::read_to_string(&p).ok()
            } else {
                None
            }
        })
        .and_then(|c| serde_json::from_str::<InstalledServers>(&c).ok())
        .map(|i| i.servers.len())
        .unwrap_or(0);

    // Total unique tools
    let tools = db.list_tools()?;
    let total_tools = tools.len();

    // Total unique prompts
    let prompts = db.list_prompts()?;
    let total_prompts = prompts.len();

    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "total_servers": stats.total_servers,
                "total_downloads": stats.total_downloads,
                "unique_owners": stats.unique_owners,
                "unique_tools": total_tools,
                "unique_prompts": total_prompts,
                "installed": installed_count,
                "avg_tools_per_server": format!("{:.1}", stats.avg_tools),
                "categories": by_cat,
                "transports": by_transport,
                "top_3": top3.iter().map(|s| serde_json::json!({
                    "name": s.full_name(),
                    "downloads": s.downloads,
                })).collect::<Vec<_>>(),
            }))?
        );
        return Ok(());
    }

    println!("╔═══════════════════════════════════════════════╗");
    println!("║           mcpreg Registry Summary             ║");
    println!("╠═══════════════════════════════════════════════╣");
    println!("║  Servers: {:<6}  Owners: {:<6}  Installed: {} ║",
        stats.total_servers, stats.unique_owners, installed_count);
    println!("║  Tools:   {:<6}  Prompts: {:<5}  Avg tools: {:.1} ║",
        total_tools, total_prompts, stats.avg_tools);
    println!("║  Downloads: {:<35}║", format_number(stats.total_downloads));
    println!("╠═══════════════════════════════════════════════╣");

    // Top 3
    println!("║  🏆 Top servers:                              ║");
    for (i, s) in top3.iter().enumerate() {
        let medal = match i {
            0 => "🥇",
            1 => "🥈",
            2 => "🥉",
            _ => "  ",
        };
        println!(
            "║  {medal} {:<30} {:>8} ⬇ ║",
            s.full_name(),
            format_number(s.downloads)
        );
    }
    println!("╠═══════════════════════════════════════════════╣");

    // Categories (compact)
    println!("║  Categories:                                  ║");
    for (cat, count) in &by_cat {
        println!("║    {cat:<34} {count:>3}  ║");
    }
    println!("╚═══════════════════════════════════════════════╝");

    Ok(())
}

fn format_number(n: i64) -> String {
    if n < 1_000 {
        return n.to_string();
    }
    let s = n.to_string();
    let mut result = String::new();
    for (i, ch) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(ch);
    }
    result.chars().rev().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_number() {
        assert_eq!(format_number(42), "42");
        assert_eq!(format_number(1_234), "1,234");
        assert_eq!(format_number(999_999), "999,999");
    }

    #[test]
    fn test_summary_json() {
        let _ = run(true);
    }

    #[test]
    fn test_summary_text() {
        let _ = run(false);
    }
}
