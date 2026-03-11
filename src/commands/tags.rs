use crate::config::Config;
use crate::error::Result;
use crate::registry::db::Database;
use crate::registry::seed::server_category;

pub fn run(json_output: bool) -> Result<()> {
    let db_path = Config::db_path()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "registry.db".to_string());

    let db = Database::open(&db_path)?;

    // Seed if empty
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

    let (servers, _) = db.list_servers(1, 1000)?;

    // Group by category
    let mut by_cat: std::collections::BTreeMap<String, Vec<String>> = std::collections::BTreeMap::new();
    for s in &servers {
        let cat = server_category(&s.owner, &s.name).to_string();
        by_cat.entry(cat).or_default().push(s.full_name());
    }

    if json_output {
        let cats: Vec<serde_json::Value> = by_cat
            .iter()
            .map(|(cat, names)| {
                serde_json::json!({
                    "category": cat,
                    "count": names.len(),
                    "servers": names,
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "categories": cats,
                "total_categories": cats.len(),
                "total_servers": servers.len(),
            }))?
        );
        return Ok(());
    }

    println!("MCP Server Categories:\n");
    for (cat, names) in &by_cat {
        println!("  {cat} ({} servers)", names.len());
        for name in names {
            println!("    • {name}");
        }
        println!();
    }
    println!("{} categories, {} servers total", by_cat.len(), servers.len());

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tags_runs_without_panic() {
        let _ = run(false);
    }
}
