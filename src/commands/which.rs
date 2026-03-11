use crate::config::Config;
use crate::error::Result;
use crate::registry::db::Database;

pub fn run(tool_name: &str, json_output: bool) -> Result<()> {
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

    let all_tools = db.list_tools()?;
    let tool_lower = tool_name.to_lowercase();

    // Exact match first, then fuzzy/partial matches
    let mut exact: Vec<&(String, Vec<String>)> = Vec::new();
    let mut partial: Vec<&(String, Vec<String>)> = Vec::new();

    for entry in &all_tools {
        let name_lower = entry.0.to_lowercase();
        if name_lower == tool_lower {
            exact.push(entry);
        } else if name_lower.contains(&tool_lower) || tool_lower.contains(&name_lower) {
            partial.push(entry);
        }
    }

    let matches: Vec<&(String, Vec<String>)> = if !exact.is_empty() {
        exact
    } else {
        partial
    };

    if json_output {
        let items: Vec<serde_json::Value> = matches
            .iter()
            .map(|(tool, servers)| {
                serde_json::json!({
                    "tool": tool,
                    "servers": servers,
                    "server_count": servers.len(),
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "query": tool_name,
                "matches": items,
                "total": matches.len(),
            }))?
        );
        return Ok(());
    }

    if matches.is_empty() {
        println!("No servers found providing tool '{tool_name}'.");

        // Fuzzy suggestions
        let tool_names: Vec<String> = all_tools.iter().map(|(t, _)| t.clone()).collect();
        let suggestions = crate::fuzzy::suggest(tool_name, &tool_names, 3);
        if !suggestions.is_empty() {
            println!("\n  Did you mean?");
            for (name, _) in &suggestions {
                println!("    • {name}");
            }
        }
        return Ok(());
    }

    for (tool, servers) in &matches {
        println!("Tool: {tool}");
        println!("  Provided by {} server(s):", servers.len());
        for s in servers {
            println!("    • {s}");
        }
        println!();
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_which_does_not_panic() {
        let _ = run("read_file", false);
    }

    #[test]
    fn test_which_json_does_not_panic() {
        let _ = run("nonexistent_tool_xyz", true);
    }
}
