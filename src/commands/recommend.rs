use crate::api::types::InstalledServers;
use crate::config::Config;
use crate::error::Result;
use crate::registry::db::Database;
use crate::registry::seed::server_category;
use std::collections::{BTreeMap, HashSet};

/// Recommend servers based on what's already installed.
///
/// Strategy: look at categories and tools of installed servers, find
/// uninstalled servers that share categories or have complementary tools.
pub fn run(limit: usize, json_output: bool) -> Result<()> {
    let installed_path = Config::installed_servers_path()?;
    let installed: InstalledServers = if installed_path.exists() {
        let content = std::fs::read_to_string(&installed_path)?;
        serde_json::from_str(&content)?
    } else {
        InstalledServers::default()
    };

    if installed.servers.is_empty() {
        if json_output {
            println!(r#"{{"recommendations":[],"reason":"no servers installed"}}"#);
        } else {
            println!("No servers installed yet. Try 'mcpreg browse' to discover servers.");
        }
        return Ok(());
    }

    let db_path = Config::db_path()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "registry.db".to_string());
    let db = Database::open(&db_path)?;
    let _ = db.seed_default_servers();

    // Build profile from installed servers
    let installed_names: HashSet<String> = installed
        .servers
        .iter()
        .map(|s| format!("{}/{}", s.owner, s.name))
        .collect();

    // Count categories of installed servers
    let mut category_counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut installed_tools: HashSet<String> = HashSet::new();

    for s in &installed.servers {
        let cat = server_category(&s.owner, &s.name).to_string();
        *category_counts.entry(cat).or_default() += 1;

        // We don't have tools on InstalledServer, but we can look them up in DB
        if let Ok(Some(entry)) = db.get_server(&s.owner, &s.name) {
            for tool in &entry.tools {
                installed_tools.insert(tool.clone());
            }
        }
    }

    // Score all non-installed servers
    let (all_servers, _) = db.list_servers(1, 1000)?;
    let mut scored: Vec<(f64, &crate::api::types::ServerEntry)> = Vec::new();

    for server in &all_servers {
        let full_name = server.full_name();
        if installed_names.contains(&full_name) {
            continue;
        }

        let mut score = 0.0;

        // Category match bonus (servers in same categories as installed ones)
        let cat = server_category(&server.owner, &server.name).to_string();
        if let Some(&count) = category_counts.get(&cat) {
            score += count as f64 * 2.0;
        }

        // Complementary tools bonus (has tools NOT in installed set)
        let new_tools: usize = server
            .tools
            .iter()
            .filter(|t| !installed_tools.contains(t.as_str()))
            .count();
        score += new_tools as f64 * 1.5;

        // Shared tools bonus (some overlap means compatibility)
        let shared_tools: usize = server
            .tools
            .iter()
            .filter(|t| installed_tools.contains(t.as_str()))
            .count();
        score += shared_tools as f64 * 0.5;

        // Popularity bonus (log scale)
        if server.downloads > 0 {
            score += (server.downloads as f64).ln();
        }

        if score > 0.0 {
            scored.push((score, server));
        }
    }

    // Sort by score descending
    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(limit);

    if json_output {
        let items: Vec<serde_json::Value> = scored
            .iter()
            .map(|(score, s)| {
                serde_json::json!({
                    "name": s.full_name(),
                    "description": s.description,
                    "category": server_category(&s.owner, &s.name),
                    "score": format!("{score:.1}"),
                    "downloads": s.downloads,
                    "tools": s.tools,
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "recommendations": items,
                "based_on": installed_names.iter().collect::<Vec<_>>(),
                "total": scored.len(),
            }))?
        );
        return Ok(());
    }

    if scored.is_empty() {
        println!("No recommendations found. You might already have everything!");
        return Ok(());
    }

    println!(
        "Recommended servers based on {} installed server(s):\n",
        installed.servers.len()
    );
    for (i, (score, server)) in scored.iter().enumerate() {
        let cat = server_category(&server.owner, &server.name);
        println!(
            "  {}. {} v{}  [{cat}]  (score: {score:.1})",
            i + 1,
            server.full_name(),
            server.version,
        );
        println!("     {}", server.description);
        if !server.tools.is_empty() {
            let new_tools: Vec<_> = server
                .tools
                .iter()
                .filter(|t| !installed_tools.contains(t.as_str()))
                .take(5)
                .cloned()
                .collect();
            if !new_tools.is_empty() {
                println!("     New tools: {}", new_tools.join(", "));
            }
        }
        println!(
            "     ⬇ {} downloads | Install: mcpreg install {}",
            server.downloads,
            server.full_name()
        );
        println!();
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recommend_no_installed() {
        // Should handle gracefully when nothing is installed
        let _ = run(5, false);
    }

    #[test]
    fn test_recommend_json_no_installed() {
        let _ = run(5, true);
    }
}
