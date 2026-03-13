use crate::config::Config;
use crate::error::Result;
use crate::registry::db::Database;

pub fn run(min_shared: usize, limit: usize, json_output: bool) -> Result<()> {
    let db_path = Config::db_path()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "registry.db".to_string());

    let db = Database::open(&db_path)?;

    // Seed if empty so graph has data
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

    // Build edges: pairs of servers sharing tools
    let mut edges: Vec<(String, String, Vec<String>, usize)> = Vec::new();

    for i in 0..servers.len() {
        if edges.len() >= limit {
            break;
        }
        let tools_i: std::collections::HashSet<&str> =
            servers[i].tools.iter().map(|s| s.as_str()).collect();
        if tools_i.is_empty() {
            continue;
        }

        for j in (i + 1)..servers.len() {
            let tools_j: std::collections::HashSet<&str> =
                servers[j].tools.iter().map(|s| s.as_str()).collect();
            let shared: Vec<String> = tools_i
                .intersection(&tools_j)
                .map(|s| s.to_string())
                .collect();

            if shared.len() >= min_shared {
                let count = shared.len();
                edges.push((
                    servers[i].full_name(),
                    servers[j].full_name(),
                    shared,
                    count,
                ));
                if edges.len() >= limit {
                    break;
                }
            }
        }
    }

    // Sort by shared count desc
    edges.sort_by(|a, b| b.3.cmp(&a.3));

    if json_output {
        // Collect unique nodes
        let mut nodes: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        for (a, b, _, _) in &edges {
            nodes.insert(a.clone());
            nodes.insert(b.clone());
        }

        let edges_json: Vec<serde_json::Value> = edges
            .iter()
            .map(|(a, b, tools, count)| {
                serde_json::json!({
                    "server_a": a,
                    "server_b": b,
                    "shared_tools": tools,
                    "shared_count": count,
                })
            })
            .collect();

        let result = serde_json::json!({
            "nodes": nodes,
            "edges": edges_json,
            "total_edges": edges_json.len(),
            "total_nodes": nodes.len(),
        });
        println!("{}", serde_json::to_string_pretty(&result)?);
        return Ok(());
    }

    if edges.is_empty() {
        println!("No tool-sharing connections found (min_shared={min_shared}).");
        return Ok(());
    }

    // Collect unique nodes
    let mut nodes: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for (a, b, _, _) in &edges {
        nodes.insert(a.clone());
        nodes.insert(b.clone());
    }

    println!(
        "Tool-sharing graph: {} connections between {} servers (min_shared={min_shared})\n",
        edges.len(),
        nodes.len()
    );

    for (a, b, shared, count) in &edges {
        println!("  {a}  ←→  {b}  ({count} shared)");
        let display: Vec<&str> = shared.iter().take(6).map(|s| s.as_str()).collect();
        let suffix = if shared.len() > 6 {
            format!(" (+{} more)", shared.len() - 6)
        } else {
            String::new()
        };
        println!("    Tools: {}{suffix}", display.join(", "));
    }

    Ok(())
}
