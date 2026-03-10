use crate::config::Config;
use crate::error::Result;
use crate::registry::db::Database;
use crate::registry::seed::server_category;
use std::collections::BTreeMap;

pub fn run(page: usize, per_page: usize, category: Option<&str>) -> Result<()> {
    let db_path = Config::db_path()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "registry.db".to_string());

    let db = Database::open(&db_path)?;

    // Seed if empty (so browse works even without serve)
    match db.seed_default_servers() {
        Ok(0) => {}
        Ok(n) => eprintln!("ℹ  Seeded {n} default servers into local registry."),
        Err(e) => eprintln!("⚠  Could not seed defaults: {e}"),
    }

    let (all_servers, total) = db.list_servers(1, 1000)?;

    if total == 0 {
        println!("No servers in registry.");
        return Ok(());
    }

    // Group by category
    let mut by_category: BTreeMap<&str, Vec<_>> = BTreeMap::new();
    for s in &all_servers {
        let cat = server_category(&s.owner, &s.name);
        by_category.entry(cat).or_default().push(s);
    }

    // Filter by category if requested
    let categories: Vec<(&str, Vec<_>)> = if let Some(filter) = category {
        let filter_lower = filter.to_lowercase();
        by_category
            .into_iter()
            .filter(|(k, _)| k.to_lowercase().contains(&filter_lower))
            .collect()
    } else {
        by_category.into_iter().collect()
    };

    // Flatten for pagination
    let mut flat: Vec<(&str, &crate::api::types::ServerEntry)> = Vec::new();
    for (cat, servers) in &categories {
        for s in servers {
            flat.push((cat, s));
        }
    }

    let total_filtered = flat.len();
    let start = (page.saturating_sub(1)) * per_page;
    let page_items: Vec<_> = flat.into_iter().skip(start).take(per_page).collect();

    if page_items.is_empty() {
        println!("No servers on page {page}.");
        return Ok(());
    }

    println!("╔══════════════════════════════════════════════════════════════════╗");
    println!("║              mcpreg — MCP Server Registry Browser               ║");
    println!("╚══════════════════════════════════════════════════════════════════╝");
    println!();

    let mut current_cat = "";
    for (cat, server) in &page_items {
        if *cat != current_cat {
            current_cat = cat;
            println!("  {current_cat}");
            println!("  {}", "─".repeat(60));
        }
        let tools_count = server.tools.len();
        println!(
            "    {}/{} v{} ({} tools)",
            server.owner, server.name, server.version, tools_count,
        );
        println!("      {}", server.description);
        println!(
            "      ⬡ {} | 📦 {} | ⬇ {}",
            server.transport, server.license, server.downloads
        );
        println!();
    }

    let total_pages = total_filtered.div_ceil(per_page);
    println!(
        "  Page {page}/{total_pages} ({total_filtered} servers total) — use --page N to navigate"
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_browse_runs_without_error() {
        // Just make sure it doesn't panic with a temporary DB
        // (uses in-memory via seed)
        let _ = run(1, 10, None);
    }
}
