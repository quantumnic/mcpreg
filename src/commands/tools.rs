use crate::config::Config;
use crate::error::Result;
use crate::registry::db::Database;

/// List all unique tools across the registry, with which servers provide them.
pub fn run(query: Option<&str>, limit: Option<usize>, json_output: bool) -> Result<()> {
    let db_path = Config::db_path()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "registry.db".to_string());

    let db = Database::open(&db_path)?;
    let _ = db.seed_default_servers();

    let mut all_tools = db.list_tools()?;

    // Filter by query if provided
    if let Some(q) = query {
        let q_lower = q.to_lowercase();
        all_tools.retain(|(name, _)| name.to_lowercase().contains(&q_lower));
    }

    let total = all_tools.len();
    let limit = limit.unwrap_or(100).min(500);
    all_tools.truncate(limit);

    if json_output {
        let items: Vec<serde_json::Value> = all_tools
            .iter()
            .map(|(tool, servers)| {
                serde_json::json!({
                    "tool": tool,
                    "server_count": servers.len(),
                    "servers": servers,
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "tools": items,
                "total": total,
            }))?
        );
        return Ok(());
    }

    if all_tools.is_empty() {
        println!("No tools found.");
        return Ok(());
    }

    println!("Tools across the MCP registry:\n");
    println!("  {:<30} SERVERS", "TOOL");
    println!("  {}", "─".repeat(60));
    for (tool, servers) in &all_tools {
        let server_list: Vec<_> = servers.iter().take(3).cloned().collect();
        let suffix = if servers.len() > 3 {
            format!(" (+{} more)", servers.len() - 3)
        } else {
            String::new()
        };
        println!(
            "  {:<30} {}{}",
            tool,
            server_list.join(", "),
            suffix,
        );
    }
    println!("\n{total} tool(s) total.");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tools_runs() {
        let _ = run(None, None, false);
    }

    #[test]
    fn test_tools_with_filter() {
        let _ = run(Some("read"), None, false);
    }

    #[test]
    fn test_tools_json() {
        let _ = run(None, Some(5), true);
    }
}
