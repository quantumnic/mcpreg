use crate::config::Config;
use crate::error::Result;
use crate::registry::db::Database;

/// List all unique prompts across the registry.
pub fn run(query: Option<&str>, json_output: bool) -> Result<()> {
    let db_path = Config::db_path()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "registry.db".to_string());

    let db = Database::open(&db_path)?;
    let _ = db.seed_default_servers();

    let mut all_prompts = db.list_prompts()?;

    if let Some(q) = query {
        let q_lower = q.to_lowercase();
        all_prompts.retain(|(name, _)| name.to_lowercase().contains(&q_lower));
    }

    let total = all_prompts.len();

    if json_output {
        let items: Vec<serde_json::Value> = all_prompts
            .iter()
            .map(|(prompt, servers)| {
                serde_json::json!({
                    "prompt": prompt,
                    "server_count": servers.len(),
                    "servers": servers,
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "prompts": items,
                "total": total,
            }))?
        );
        return Ok(());
    }

    if all_prompts.is_empty() {
        println!("No prompts found in the registry.");
        return Ok(());
    }

    println!("Prompts across the MCP registry:\n");
    println!("  {:<30} SERVERS", "PROMPT");
    println!("  {}", "─".repeat(60));
    for (prompt, servers) in &all_prompts {
        println!(
            "  {:<30} {}",
            prompt,
            servers.join(", "),
        );
    }
    println!("\n{total} prompt(s) total.");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prompts_runs() {
        let _ = run(None, false);
    }

    #[test]
    fn test_prompts_json() {
        let _ = run(None, true);
    }
}
