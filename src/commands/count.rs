use crate::config::Config;
use crate::error::Result;
use crate::registry::db::Database;
use crate::registry::seed::server_category;

pub fn run(by: Option<&str>, json_output: bool) -> Result<()> {
    let db_path = Config::db_path()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "registry.db".to_string());

    let db = Database::open(&db_path)?;
    let _ = db.seed_default_servers();

    let group_by = by.unwrap_or("total");

    match group_by {
        "transport" => count_by_transport(&db, json_output),
        "category" => count_by_category(&db, json_output),
        "owner" => count_by_owner(&db, json_output),
        _ => count_total(&db, json_output),
    }
}

fn count_total(db: &Database, json_output: bool) -> Result<()> {
    let stats = db.stats()?;
    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "total_servers": stats.total_servers,
                "total_downloads": stats.total_downloads,
                "unique_owners": stats.unique_owners,
            }))?
        );
    } else {
        println!("{} servers, {} downloads, {} owners",
            stats.total_servers, stats.total_downloads, stats.unique_owners);
    }
    Ok(())
}

fn count_by_transport(db: &Database, json_output: bool) -> Result<()> {
    let stats = db.stats()?;
    if json_output {
        let items: Vec<serde_json::Value> = stats.transport_counts
            .iter()
            .map(|(t, c)| serde_json::json!({"transport": t, "count": c}))
            .collect();
        println!("{}", serde_json::to_string_pretty(&items)?);
    } else {
        for (transport, count) in &stats.transport_counts {
            println!("  {transport}: {count}");
        }
    }
    Ok(())
}

fn count_by_category(db: &Database, json_output: bool) -> Result<()> {
    let (servers, _) = db.list_servers(1, 1000)?;
    let mut counts: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for s in &servers {
        let cat = server_category(&s.owner, &s.name).to_string();
        *counts.entry(cat).or_default() += 1;
    }

    if json_output {
        let items: Vec<serde_json::Value> = counts
            .iter()
            .map(|(c, n)| serde_json::json!({"category": c, "count": n}))
            .collect();
        println!("{}", serde_json::to_string_pretty(&items)?);
    } else {
        for (cat, count) in &counts {
            println!("  {cat}: {count}");
        }
    }
    Ok(())
}

fn count_by_owner(db: &Database, json_output: bool) -> Result<()> {
    let (servers, _) = db.list_servers(1, 1000)?;
    let mut counts: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for s in &servers {
        *counts.entry(s.owner.clone()).or_default() += 1;
    }

    // Sort by count descending
    let mut sorted: Vec<(String, usize)> = counts.into_iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1));

    if json_output {
        let items: Vec<serde_json::Value> = sorted
            .iter()
            .map(|(o, c)| serde_json::json!({"owner": o, "count": c}))
            .collect();
        println!("{}", serde_json::to_string_pretty(&items)?);
    } else {
        for (owner, count) in &sorted {
            println!("  {owner}: {count}");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_count_total() {
        let _ = run(None, false);
    }

    #[test]
    fn test_count_by_transport() {
        let _ = run(Some("transport"), false);
    }

    #[test]
    fn test_count_by_category() {
        let _ = run(Some("category"), false);
    }

    #[test]
    fn test_count_by_owner() {
        let _ = run(Some("owner"), true);
    }
}
