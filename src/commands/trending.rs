use crate::config::Config;
use crate::error::Result;
use crate::registry::db::Database;
use crate::registry::seed::server_category;

/// Show trending / top servers by download count with optional filters.
pub fn run(
    limit: usize,
    category: Option<&str>,
    transport: Option<&str>,
    json_output: bool,
) -> Result<()> {
    let db_path = Config::db_path()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "registry.db".to_string());

    let db = Database::open(&db_path)?;

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

    // Sort by downloads descending (trending = most popular)
    servers.sort_by(|a, b| b.downloads.cmp(&a.downloads));

    // Apply category filter
    if let Some(cat) = category {
        let cat_lower = cat.to_lowercase();
        servers.retain(|s| {
            server_category(&s.owner, &s.name)
                .to_lowercase()
                .contains(&cat_lower)
        });
    }

    // Apply transport filter
    if let Some(t) = transport {
        let t_lower = t.to_lowercase();
        servers.retain(|s| s.transport.to_lowercase() == t_lower);
    }

    servers.truncate(limit);

    if json_output {
        let items: Vec<serde_json::Value> = servers
            .iter()
            .enumerate()
            .map(|(i, s)| {
                serde_json::json!({
                    "rank": i + 1,
                    "name": s.full_name(),
                    "description": s.description,
                    "downloads": s.downloads,
                    "category": server_category(&s.owner, &s.name),
                    "transport": s.transport,
                    "tools_count": s.tools.len(),
                    "version": s.version,
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "trending": items,
                "total": items.len(),
            }))?
        );
        return Ok(());
    }

    if servers.is_empty() {
        println!("No servers found matching the given filters.");
        return Ok(());
    }

    println!("🔥 Trending MCP Servers\n");

    // Find the widest download number for alignment
    let max_dl_width = servers
        .first()
        .map(|s| format_downloads(s.downloads).len())
        .unwrap_or(0);

    for (i, server) in servers.iter().enumerate() {
        let rank = i + 1;
        let cat = server_category(&server.owner, &server.name);
        let dl = format_downloads(server.downloads);
        let medal = match rank {
            1 => "🥇",
            2 => "🥈",
            3 => "🥉",
            _ => "  ",
        };
        println!(
            "  {medal} {rank:>2}. {:<40} {:>width$} ⬇  [{cat}]",
            format!("{} v{}", server.full_name(), server.version),
            dl,
            width = max_dl_width,
        );
        println!("       {}", server.description);
        if !server.tools.is_empty() {
            let count = server.tools.len();
            let preview: Vec<_> = server.tools.iter().take(4).cloned().collect();
            let suffix = if count > 4 {
                format!(" +{} more", count - 4)
            } else {
                String::new()
            };
            println!("       Tools: {}{}", preview.join(", "), suffix);
        }
        println!();
    }

    Ok(())
}

/// Format a download count with thousands separator.
fn format_downloads(n: i64) -> String {
    if n < 1_000 {
        return n.to_string();
    }
    let s = n.to_string();
    let mut result = String::new();
    for (i, ch) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(ch);
    }
    result.chars().rev().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_downloads() {
        assert_eq!(format_downloads(0), "0");
        assert_eq!(format_downloads(999), "999");
        assert_eq!(format_downloads(1_000), "1,000");
        assert_eq!(format_downloads(52_000), "52,000");
        assert_eq!(format_downloads(1_234_567), "1,234,567");
    }

    #[test]
    fn test_trending_runs_json() {
        // Should not panic even if DB doesn't exist yet
        let _ = run(5, None, None, true);
    }

    #[test]
    fn test_trending_with_category() {
        let _ = run(3, Some("database"), None, true);
    }

    #[test]
    fn test_trending_with_transport() {
        let _ = run(3, None, Some("stdio"), false);
    }
}
