use crate::config::Config;
use crate::error::{McpRegError, Result};
use crate::registry::db::Database;
use crate::registry::seed::server_category;

/// Deep inspection of a single server — combines info, score, config snippet,
/// and similar servers in one view.
pub fn run(server_ref: &str, json_output: bool) -> Result<()> {
    let parts: Vec<&str> = server_ref.splitn(2, '/').collect();
    if parts.len() != 2 {
        return Err(McpRegError::Config(
            "Server reference must be in format 'owner/name'".into(),
        ));
    }
    let (owner, name) = (parts[0], parts[1]);

    let db_path = Config::db_path()?;
    let db = Database::open(db_path.to_str().unwrap_or("registry.db"))?;
    let _ = db.seed_default_servers();

    let entry = match db.get_server(owner, name)? {
        Some(e) => e,
        None => return Err(McpRegError::NotFound(format!("{owner}/{name}"))),
    };

    let category = server_category(owner, name).to_string();

    // Score computation (inline, mirrors the API /score logic)
    let score_result = compute_score(&entry);

    // Similar servers
    let similar = find_similar(&db, owner, name, &entry);

    // Config snippet
    let config_key = format!("{owner}-{name}");
    let mut config_snippet = serde_json::json!({
        "command": entry.command,
        "args": entry.args,
    });
    if entry.transport != "stdio" {
        config_snippet["transport"] = serde_json::json!(entry.transport);
    }
    if !entry.env.is_empty() {
        config_snippet["env"] = serde_json::json!(entry.env);
    }

    if json_output {
        let output = serde_json::json!({
            "server": {
                "full_name": entry.full_name(),
                "version": entry.version,
                "description": entry.description,
                "author": entry.author,
                "license": entry.license,
                "repository": entry.repository,
                "homepage": entry.homepage,
                "transport": entry.transport,
                "command": entry.command,
                "args": entry.args,
                "tools": entry.tools,
                "resources": entry.resources,
                "prompts": entry.prompts,
                "tags": entry.tags,
                "downloads": entry.downloads,
                "deprecated": entry.deprecated,
                "deprecated_by": entry.deprecated_by,
                "category": category,
            },
            "score": {
                "score": score_result.score,
                "max_score": score_result.max_score,
                "percentage": score_result.percentage,
                "grade": score_result.grade,
                "checks": score_result.checks,
            },
            "config_snippet": {
                "mcpServers": { &config_key: config_snippet }
            },
            "similar": similar,
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
        return Ok(());
    }

    // ── Pretty display ──
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║  {} v{}", entry.full_name(), entry.version);
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();

    if !entry.description.is_empty() {
        println!("  {}", entry.description);
        println!();
    }

    // Metadata table
    println!("  ┌─────────────┬──────────────────────────────────────────");
    if !entry.author.is_empty() {
        println!("  │ Author      │ {}", entry.author);
    }
    if !entry.license.is_empty() {
        println!("  │ License     │ {}", entry.license);
    }
    println!("  │ Category    │ {category}");
    println!("  │ Transport   │ {}", entry.transport);
    println!(
        "  │ Command     │ {} {}",
        entry.command,
        entry.args.join(" ")
    );
    println!("  │ Downloads   │ {}", format_downloads(entry.downloads));
    if !entry.repository.is_empty() {
        println!("  │ Repository  │ {}", entry.repository);
    }
    if !entry.homepage.is_empty() {
        println!("  │ Homepage    │ {}", entry.homepage);
    }
    println!("  └─────────────┴──────────────────────────────────────────");

    if entry.deprecated {
        println!();
        println!("  ⚠️  DEPRECATED");
        if let Some(ref replacement) = entry.deprecated_by {
            println!("     Replaced by: {replacement}");
        }
    }

    // Tools
    if !entry.tools.is_empty() {
        println!();
        println!("  🔧 Tools ({}):", entry.tools.len());
        for tool in &entry.tools {
            println!("     • {tool}");
        }
    }

    // Resources
    if !entry.resources.is_empty() {
        println!();
        println!("  📦 Resources ({}):", entry.resources.len());
        for res in &entry.resources {
            println!("     • {res}");
        }
    }

    // Prompts
    if !entry.prompts.is_empty() {
        println!();
        println!("  💬 Prompts ({}):", entry.prompts.len());
        for prompt in &entry.prompts {
            println!("     • {prompt}");
        }
    }

    // Tags
    if !entry.tags.is_empty() {
        println!();
        println!("  🏷️  Tags: {}", entry.tags.join(", "));
    }

    // Score
    println!();
    println!(
        "  📊 Quality Score: {}/{} ({}%) — Grade: {}",
        score_result.score, score_result.max_score, score_result.percentage, score_result.grade
    );
    let bar_len = 30;
    let filled = (score_result.percentage as usize * bar_len) / 100;
    let empty = bar_len - filled;
    println!(
        "     [{}{}]",
        "█".repeat(filled),
        "░".repeat(empty)
    );
    for check in &score_result.checks {
        let pass = check["pass"].as_bool().unwrap_or(false);
        let icon = if pass { "✓" } else { "✗" };
        let name = check["check"].as_str().unwrap_or("?");
        let points = check["points"].as_u64().unwrap_or(0);
        let max_points = check["max_points"].as_u64().unwrap_or(0);
        println!("     {icon} {name} ({points}/{max_points})");
    }

    // Config snippet
    println!();
    println!("  ⚙️  Claude Desktop Config:");
    println!("  ─────────────────────────");
    let pretty_config = serde_json::json!({
        "mcpServers": { &config_key: config_snippet }
    });
    for line in serde_json::to_string_pretty(&pretty_config)?.lines() {
        println!("     {line}");
    }

    // Similar servers
    if !similar.is_empty() {
        println!();
        println!("  🔗 Similar Servers:");
        for s in &similar {
            let sim_name = s["full_name"].as_str().unwrap_or("?");
            let shared = s["shared_tools"].as_u64().unwrap_or(0);
            let sim_cat = s["category"].as_str().unwrap_or("?");
            println!("     • {sim_name} ({shared} shared tools, {sim_cat})");
        }
    }

    // Install hint
    println!();
    println!("  Install: mcpreg install {}/{}", owner, name);
    println!();

    Ok(())
}

fn format_downloads(n: i64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

struct ScoreCheck {
    name: String,
    pass: bool,
    points: u32,
    max_points: u32,
}

struct ScoreResult {
    score: u32,
    max_score: u32,
    percentage: u32,
    grade: String,
    checks: Vec<serde_json::Value>,
}

fn compute_score(entry: &crate::api::types::ServerEntry) -> ScoreResult {
    let checks = vec![
        ScoreCheck {
            name: "Has description".into(),
            pass: !entry.description.is_empty(),
            points: if !entry.description.is_empty() { 15 } else { 0 },
            max_points: 15,
        },
        ScoreCheck {
            name: "Has author".into(),
            pass: !entry.author.is_empty(),
            points: if !entry.author.is_empty() { 10 } else { 0 },
            max_points: 10,
        },
        ScoreCheck {
            name: "Has license".into(),
            pass: !entry.license.is_empty(),
            points: if !entry.license.is_empty() { 10 } else { 0 },
            max_points: 10,
        },
        ScoreCheck {
            name: "Has repository".into(),
            pass: !entry.repository.is_empty(),
            points: if !entry.repository.is_empty() { 10 } else { 0 },
            max_points: 10,
        },
        ScoreCheck {
            name: "Has tools".into(),
            pass: !entry.tools.is_empty(),
            points: if !entry.tools.is_empty() { 20 } else { 0 },
            max_points: 20,
        },
        ScoreCheck {
            name: "Has 3+ tools".into(),
            pass: entry.tools.len() >= 3,
            points: if entry.tools.len() >= 3 { 10 } else { 0 },
            max_points: 10,
        },
        ScoreCheck {
            name: "Has tags".into(),
            pass: !entry.tags.is_empty(),
            points: if !entry.tags.is_empty() { 5 } else { 0 },
            max_points: 5,
        },
        ScoreCheck {
            name: "Has resources or prompts".into(),
            pass: !entry.resources.is_empty() || !entry.prompts.is_empty(),
            points: if !entry.resources.is_empty() || !entry.prompts.is_empty() {
                10
            } else {
                0
            },
            max_points: 10,
        },
        ScoreCheck {
            name: "Not deprecated".into(),
            pass: !entry.deprecated,
            points: if !entry.deprecated { 10 } else { 0 },
            max_points: 10,
        },
    ];

    let score: u32 = checks.iter().map(|c| c.points).sum();
    let max_score: u32 = checks.iter().map(|c| c.max_points).sum();
    let percentage = if max_score > 0 {
        (score * 100) / max_score
    } else {
        0
    };

    let grade = match percentage {
        90..=100 => "A",
        75..=89 => "B",
        60..=74 => "C",
        40..=59 => "D",
        _ => "F",
    }
    .to_string();

    let check_json: Vec<serde_json::Value> = checks
        .iter()
        .map(|c| {
            serde_json::json!({
                "check": c.name,
                "pass": c.pass,
                "points": c.points,
                "max_points": c.max_points,
            })
        })
        .collect();

    ScoreResult {
        score,
        max_score,
        percentage,
        grade,
        checks: check_json,
    }
}

fn find_similar(
    db: &Database,
    owner: &str,
    name: &str,
    entry: &crate::api::types::ServerEntry,
) -> Vec<serde_json::Value> {
    let all = match db.list_all() {
        Ok(a) => a,
        Err(_) => return vec![],
    };

    let my_tools: std::collections::HashSet<&str> =
        entry.tools.iter().map(|s| s.as_str()).collect();
    let my_cat = server_category(owner, name).to_string();

    let mut scored: Vec<(usize, &crate::api::types::ServerEntry)> = Vec::new();

    for s in &all {
        if s.owner == owner && s.name == name {
            continue;
        }
        if s.deprecated {
            continue;
        }

        let shared: usize = s.tools.iter().filter(|t| my_tools.contains(t.as_str())).count();
        let cat = server_category(&s.owner, &s.name).to_string();
        let cat_bonus = if cat == my_cat { 1 } else { 0 };

        let total = shared + cat_bonus;
        if total > 0 {
            scored.push((total, s));
        }
    }

    scored.sort_by(|a, b| b.0.cmp(&a.0));
    scored.truncate(5);

    scored
        .iter()
        .map(|(shared, s)| {
            let cat = server_category(&s.owner, &s.name).to_string();
            serde_json::json!({
                "full_name": s.full_name(),
                "shared_tools": shared,
                "category": cat,
                "downloads": s.downloads,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_downloads_units() {
        assert_eq!(format_downloads(0), "0");
        assert_eq!(format_downloads(999), "999");
        assert_eq!(format_downloads(1_000), "1.0K");
        assert_eq!(format_downloads(52_000), "52.0K");
        assert_eq!(format_downloads(1_000_000), "1.0M");
        assert_eq!(format_downloads(2_500_000), "2.5M");
    }

    #[test]
    fn test_compute_score_full() {
        let entry = crate::api::types::ServerEntry {
            id: None,
            owner: "test".into(),
            name: "server".into(),
            version: "1.0.0".into(),
            description: "A test server".into(),
            author: "Test Author".into(),
            license: "MIT".into(),
            repository: "https://github.com/test/server".into(),
            command: "node".into(),
            args: vec!["server.js".into()],
            transport: "stdio".into(),
            tools: vec!["read".into(), "write".into(), "list".into()],
            resources: vec!["file://".into()],
            prompts: vec![],
            tags: vec!["test".into()],
            env: Default::default(),
            homepage: String::new(),
            deprecated: false,
            deprecated_by: None,
            downloads: 1000,
            created_at: None,
            updated_at: None,
        };
        let result = compute_score(&entry);
        assert_eq!(result.max_score, 100);
        assert!(result.score >= 80, "Expected high score for complete entry, got {}", result.score);
        assert_eq!(result.grade, "A");
    }

    #[test]
    fn test_compute_score_minimal() {
        let entry = crate::api::types::ServerEntry {
            id: None,
            owner: "test".into(),
            name: "bare".into(),
            version: "0.1.0".into(),
            description: String::new(),
            author: String::new(),
            license: String::new(),
            repository: String::new(),
            command: "node".into(),
            args: vec![],
            transport: "stdio".into(),
            tools: vec![],
            resources: vec![],
            prompts: vec![],
            tags: vec![],
            env: Default::default(),
            homepage: String::new(),
            deprecated: true,
            deprecated_by: None,
            downloads: 0,
            created_at: None,
            updated_at: None,
        };
        let result = compute_score(&entry);
        assert_eq!(result.score, 0);
        assert_eq!(result.grade, "F");
    }

    #[test]
    fn test_inspect_bad_ref() {
        let result = run("noowner", false);
        assert!(result.is_err());
    }
}
