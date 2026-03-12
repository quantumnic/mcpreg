use crate::config::Config;
use crate::error::Result;
use crate::registry::db::Database;

pub fn run(category: Option<&str>, json_output: bool) -> Result<()> {
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

    let (mut servers, _) = db.list_servers(1, 1000)?;

    if let Some(cat) = category {
        let cat_lower = cat.to_lowercase();
        servers.retain(|s| {
            let server_cat =
                crate::registry::seed::server_category(&s.owner, &s.name).to_lowercase();
            server_cat.contains(&cat_lower)
        });
    }

    if servers.is_empty() {
        if json_output {
            println!("{{\"error\":\"no servers found\"}}");
        } else {
            println!("No servers found.");
        }
        return Ok(());
    }

    // Simple pseudo-random selection using timestamp
    let idx = {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos() as usize;
        nanos % servers.len()
    };

    let server = &servers[idx];

    if json_output {
        println!("{}", serde_json::to_string_pretty(server)?);
        return Ok(());
    }

    let cat = crate::registry::seed::server_category(&server.owner, &server.name);
    println!("🎲 Random pick:\n");
    println!("  {}/{} v{}  [{cat}]", server.owner, server.name, server.version);
    println!("  {}", server.description);
    if !server.tools.is_empty() {
        let tools_display: Vec<_> = server.tools.iter().take(8).cloned().collect();
        let suffix = if server.tools.len() > 8 {
            format!(" (+{} more)", server.tools.len() - 8)
        } else {
            String::new()
        };
        println!("  Tools: {}{}", tools_display.join(", "), suffix);
    }
    if !server.prompts.is_empty() {
        println!("  Prompts: {}", server.prompts.join(", "));
    }
    println!(
        "  {} | {} | ⬇ {} downloads",
        server.transport, server.license, server.downloads
    );
    println!("\n  Install: mcpreg install {}/{}", server.owner, server.name);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_random_runs_without_panic() {
        let _ = run(None, false);
    }

    #[test]
    fn test_random_json_runs_without_panic() {
        let _ = run(None, true);
    }

    #[test]
    fn test_random_with_category() {
        let _ = run(Some("database"), false);
    }
}
