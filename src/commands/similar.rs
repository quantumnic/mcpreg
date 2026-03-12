use crate::config::Config;
use crate::error::{McpRegError, Result};
use crate::registry::db::Database;

pub fn run(server_ref: &str, limit: usize, json_output: bool) -> Result<()> {
    let parts: Vec<&str> = server_ref.splitn(2, '/').collect();
    if parts.len() != 2 {
        return Err(McpRegError::Config(
            "Server reference must be in format 'owner/name'".into(),
        ));
    }
    let (owner, name) = (parts[0], parts[1]);

    let db_path = Config::db_path()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "registry.db".to_string());

    let db = Database::open(&db_path)?;

    // Seed if empty
    let _ = db.seed_default_servers();

    let similar = db.find_similar(owner, name, limit)?;

    if similar.is_empty() {
        if json_output {
            println!("{{\"servers\":[],\"total\":0}}");
        } else {
            println!("No similar servers found for {owner}/{name}.");
        }
        return Ok(());
    }

    if json_output {
        let resp = serde_json::json!({
            "query": format!("{owner}/{name}"),
            "servers": similar.iter().map(|(entry, score)| {
                serde_json::json!({
                    "name": entry.full_name(),
                    "description": entry.description,
                    "similarity_score": score,
                    "shared_tools": shared_tool_count_display(entry),
                })
            }).collect::<Vec<_>>(),
            "total": similar.len(),
        });
        println!("{}", serde_json::to_string_pretty(&resp)?);
        return Ok(());
    }

    println!("Servers similar to {owner}/{name}:\n");
    for (entry, score) in &similar {
        let cat = crate::registry::seed::server_category(&entry.owner, &entry.name);
        println!(
            "  {} v{} (similarity: {:.0}%)  [{cat}]",
            entry.full_name(),
            entry.version,
            score * 100.0,
        );
        println!("    {}", entry.description);
        if !entry.tools.is_empty() {
            let tools_display: Vec<_> = entry.tools.iter().take(5).cloned().collect();
            let suffix = if entry.tools.len() > 5 {
                format!(" (+{} more)", entry.tools.len() - 5)
            } else {
                String::new()
            };
            println!("    Tools: {}{}", tools_display.join(", "), suffix);
        }
        println!("    ⬇ {} downloads", entry.downloads);
        println!();
    }

    Ok(())
}

fn shared_tool_count_display(entry: &crate::api::types::ServerEntry) -> usize {
    entry.tools.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_similar_bad_ref() {
        assert!(run("noslash", 5, false).is_err());
    }

    #[test]
    fn test_similar_json() {
        // Uses seeded DB; won't panic
        let _ = run("modelcontextprotocol/filesystem", 3, true);
    }

    #[test]
    fn test_similar_not_found() {
        let _ = run("nobody/nothing", 5, false);
    }
}
