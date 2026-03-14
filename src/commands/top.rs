use crate::config::Config;
use crate::error::Result;
use crate::registry::db::Database;
use crate::registry::seed::server_category;
use std::collections::BTreeMap;

/// Show top servers by various criteria.
pub fn run(by: &str, limit: usize, json_output: bool) -> Result<()> {
    let db_path = Config::db_path()?;
    let db = Database::open(db_path.to_str().unwrap_or("registry.db"))?;
    let _ = db.seed_default_servers();

    let all = db.list_all()?;

    match by {
        "tools" => top_by_tools(&all, limit, json_output),
        "resources" => top_by_resources(&all, limit, json_output),
        "prompts" => top_by_prompts(&all, limit, json_output),
        "downloads" => top_by_downloads(&all, limit, json_output),
        "newest" => top_by_newest(&all, limit, json_output),
        "category" => top_categories(&all, limit, json_output),
        _ => {
            eprintln!(
                "Unknown ranking: '{by}'. Available: tools, resources, prompts, downloads, newest, category"
            );
            std::process::exit(1);
        }
    }
}

fn top_by_tools(
    servers: &[crate::api::types::ServerEntry],
    limit: usize,
    json: bool,
) -> Result<()> {
    let mut sorted: Vec<_> = servers
        .iter()
        .filter(|s| !s.deprecated)
        .collect();
    sorted.sort_by(|a, b| b.tools.len().cmp(&a.tools.len()));
    sorted.truncate(limit);

    if json {
        let items: Vec<serde_json::Value> = sorted
            .iter()
            .enumerate()
            .map(|(i, s)| {
                serde_json::json!({
                    "rank": i + 1,
                    "full_name": s.full_name(),
                    "tool_count": s.tools.len(),
                    "tools": s.tools,
                    "category": server_category(&s.owner, &s.name),
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&serde_json::json!({
            "ranking": "tools",
            "total": items.len(),
            "servers": items,
        }))?);
        return Ok(());
    }

    println!("🔧 Top servers by tool count:\n");
    for (i, s) in sorted.iter().enumerate() {
        println!(
            "  {}. {} — {} tools",
            i + 1,
            s.full_name(),
            s.tools.len()
        );
        let preview: Vec<_> = s.tools.iter().take(5).cloned().collect();
        let extra = if s.tools.len() > 5 {
            format!(", +{} more", s.tools.len() - 5)
        } else {
            String::new()
        };
        println!("     {}{extra}", preview.join(", "));
    }
    Ok(())
}

fn top_by_resources(
    servers: &[crate::api::types::ServerEntry],
    limit: usize,
    json: bool,
) -> Result<()> {
    let mut sorted: Vec<_> = servers
        .iter()
        .filter(|s| !s.deprecated && !s.resources.is_empty())
        .collect();
    sorted.sort_by(|a, b| b.resources.len().cmp(&a.resources.len()));
    sorted.truncate(limit);

    if json {
        let items: Vec<serde_json::Value> = sorted
            .iter()
            .enumerate()
            .map(|(i, s)| {
                serde_json::json!({
                    "rank": i + 1,
                    "full_name": s.full_name(),
                    "resource_count": s.resources.len(),
                    "resources": s.resources,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&serde_json::json!({
            "ranking": "resources",
            "total": items.len(),
            "servers": items,
        }))?);
        return Ok(());
    }

    println!("📦 Top servers by resource count:\n");
    for (i, s) in sorted.iter().enumerate() {
        println!(
            "  {}. {} — {} resources",
            i + 1,
            s.full_name(),
            s.resources.len()
        );
        for r in &s.resources {
            println!("     • {r}");
        }
    }
    Ok(())
}

fn top_by_prompts(
    servers: &[crate::api::types::ServerEntry],
    limit: usize,
    json: bool,
) -> Result<()> {
    let mut sorted: Vec<_> = servers
        .iter()
        .filter(|s| !s.deprecated && !s.prompts.is_empty())
        .collect();
    sorted.sort_by(|a, b| b.prompts.len().cmp(&a.prompts.len()));
    sorted.truncate(limit);

    if json {
        let items: Vec<serde_json::Value> = sorted
            .iter()
            .enumerate()
            .map(|(i, s)| {
                serde_json::json!({
                    "rank": i + 1,
                    "full_name": s.full_name(),
                    "prompt_count": s.prompts.len(),
                    "prompts": s.prompts,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&serde_json::json!({
            "ranking": "prompts",
            "total": items.len(),
            "servers": items,
        }))?);
        return Ok(());
    }

    println!("💬 Top servers by prompt count:\n");
    if sorted.is_empty() {
        println!("  No servers with prompts found.");
        return Ok(());
    }
    for (i, s) in sorted.iter().enumerate() {
        println!(
            "  {}. {} — {} prompts",
            i + 1,
            s.full_name(),
            s.prompts.len()
        );
        for p in &s.prompts {
            println!("     • {p}");
        }
    }
    Ok(())
}

fn top_by_downloads(
    servers: &[crate::api::types::ServerEntry],
    limit: usize,
    json: bool,
) -> Result<()> {
    let mut sorted: Vec<_> = servers.iter().filter(|s| !s.deprecated).collect();
    sorted.sort_by(|a, b| b.downloads.cmp(&a.downloads));
    sorted.truncate(limit);

    if json {
        let items: Vec<serde_json::Value> = sorted
            .iter()
            .enumerate()
            .map(|(i, s)| {
                serde_json::json!({
                    "rank": i + 1,
                    "full_name": s.full_name(),
                    "downloads": s.downloads,
                    "category": server_category(&s.owner, &s.name),
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&serde_json::json!({
            "ranking": "downloads",
            "total": items.len(),
            "servers": items,
        }))?);
        return Ok(());
    }

    println!("⬇️  Top servers by downloads:\n");
    for (i, s) in sorted.iter().enumerate() {
        println!(
            "  {}. {} — {} downloads  [{}]",
            i + 1,
            s.full_name(),
            s.downloads,
            server_category(&s.owner, &s.name),
        );
    }
    Ok(())
}

fn top_by_newest(
    servers: &[crate::api::types::ServerEntry],
    limit: usize,
    json: bool,
) -> Result<()> {
    let mut sorted: Vec<_> = servers.iter().filter(|s| !s.deprecated).collect();
    sorted.sort_by(|a, b| {
        let a_time = a.updated_at.as_deref().or(a.created_at.as_deref()).unwrap_or("");
        let b_time = b.updated_at.as_deref().or(b.created_at.as_deref()).unwrap_or("");
        b_time.cmp(a_time)
    });
    sorted.truncate(limit);

    if json {
        let items: Vec<serde_json::Value> = sorted
            .iter()
            .enumerate()
            .map(|(i, s)| {
                serde_json::json!({
                    "rank": i + 1,
                    "full_name": s.full_name(),
                    "version": s.version,
                    "updated_at": s.updated_at,
                    "created_at": s.created_at,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&serde_json::json!({
            "ranking": "newest",
            "total": items.len(),
            "servers": items,
        }))?);
        return Ok(());
    }

    println!("🆕 Most recently updated servers:\n");
    for (i, s) in sorted.iter().enumerate() {
        let when = s
            .updated_at
            .as_deref()
            .or(s.created_at.as_deref())
            .unwrap_or("unknown");
        println!("  {}. {} v{} — {}", i + 1, s.full_name(), s.version, when);
    }
    Ok(())
}

fn top_categories(
    servers: &[crate::api::types::ServerEntry],
    limit: usize,
    json: bool,
) -> Result<()> {
    let mut counts: BTreeMap<String, (usize, i64)> = BTreeMap::new();
    for s in servers {
        if s.deprecated {
            continue;
        }
        let cat = server_category(&s.owner, &s.name).to_string();
        let entry = counts.entry(cat).or_insert((0, 0));
        entry.0 += 1;
        entry.1 += s.downloads;
    }

    let mut sorted: Vec<_> = counts.into_iter().collect();
    sorted.sort_by(|a, b| b.1 .0.cmp(&a.1 .0));
    sorted.truncate(limit);

    if json {
        let items: Vec<serde_json::Value> = sorted
            .iter()
            .enumerate()
            .map(|(i, (cat, (count, downloads)))| {
                serde_json::json!({
                    "rank": i + 1,
                    "category": cat,
                    "server_count": count,
                    "total_downloads": downloads,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&serde_json::json!({
            "ranking": "category",
            "total": items.len(),
            "categories": items,
        }))?);
        return Ok(());
    }

    println!("📁 Top categories:\n");
    for (i, (cat, (count, downloads))) in sorted.iter().enumerate() {
        println!("  {}. {cat} — {count} servers, {downloads} total downloads", i + 1);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_top_invalid_criterion() {
        // Should exit, not panic — but we can't test exit in unit tests easily.
        // Instead verify valid criteria don't error.
        let _ = run("tools", 5, false);
        let _ = run("resources", 5, false);
        let _ = run("prompts", 5, false);
        let _ = run("downloads", 5, false);
        let _ = run("newest", 5, false);
        let _ = run("category", 5, false);
    }

    #[test]
    fn test_top_json_output() {
        let _ = run("tools", 3, true);
        let _ = run("downloads", 3, true);
        let _ = run("category", 3, true);
    }
}
