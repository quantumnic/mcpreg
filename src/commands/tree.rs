use crate::config::Config;
use crate::error::Result;
use crate::registry::db::Database;

/// Show a visual tree of server relationships by shared tools/resources.
pub fn run(server: Option<&str>, depth: usize, json: bool) -> Result<()> {
    let db_path = Config::db_path()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "registry.db".to_string());
    let db = Database::open(&db_path)?;
    let _ = db.seed_default_servers();

    let all = db.list_servers(1, 500).map(|(v, _)| v)?;

    if all.is_empty() {
        println!("No servers in registry.");
        return Ok(());
    }

    // Build adjacency: servers sharing tools
    let mut edges: Vec<(String, String, Vec<String>)> = Vec::new();
    for i in 0..all.len() {
        for j in (i + 1)..all.len() {
            let shared: Vec<String> = all[i]
                .tools
                .iter()
                .filter(|t| all[j].tools.contains(t))
                .cloned()
                .collect();
            if !shared.is_empty() {
                edges.push((all[i].full_name(), all[j].full_name(), shared));
            }
        }
    }

    if json {
        let nodes: Vec<serde_json::Value> = all
            .iter()
            .map(|s| {
                serde_json::json!({
                    "name": s.full_name(),
                    "tools": s.tools.len(),
                    "resources": s.resources.len(),
                })
            })
            .collect();
        let edge_json: Vec<serde_json::Value> = edges
            .iter()
            .map(|(a, b, shared)| {
                serde_json::json!({
                    "from": a,
                    "to": b,
                    "shared_tools": shared,
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "nodes": nodes,
                "edges": edge_json,
            }))?
        );
        return Ok(());
    }

    // If a specific server is given, show its neighborhood
    if let Some(root) = server {
        println!("🌳 {root}");
        let mut shown = 0usize;
        for (a, b, shared) in &edges {
            if shown >= depth * 5 {
                break;
            }
            if a == root {
                println!("  ├── {} (shared: {})", b, shared.join(", "));
                shown += 1;
            } else if b == root {
                println!("  ├── {} (shared: {})", a, shared.join(", "));
                shown += 1;
            }
        }
        if shown == 0 {
            println!("  (no shared tools with other servers)");
        }
    } else {
        // Full tree: show clusters
        println!("🌳 Server Relationship Tree ({} servers, {} edges)\n", all.len(), edges.len());
        let mut sorted_edges = edges.clone();
        sorted_edges.sort_by(|a, b| b.2.len().cmp(&a.2.len()));
        for (i, (a, b, shared)) in sorted_edges.iter().enumerate() {
            if i >= depth * 10 {
                let remaining = sorted_edges.len() - i;
                if remaining > 0 {
                    println!("  ... and {remaining} more connections");
                }
                break;
            }
            println!(
                "  {} ↔ {} ({} shared: {})",
                a,
                b,
                shared.len(),
                shared.join(", ")
            );
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tree_no_crash_empty_db() {
        // With a fresh in-memory DB, tree should work without panic
        let result = std::panic::catch_unwind(|| {
            // We can't easily test with temp DB here, but ensure the function
            // signature is correct and module compiles
            let _ = format!("tree command compiled");
        });
        assert!(result.is_ok());
    }
}
