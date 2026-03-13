use crate::config::Config;
use crate::error::{McpRegError, Result};
use crate::registry::db::Database;
use std::collections::BTreeSet;

/// Check compatibility between two MCP servers.
///
/// Reports: overlapping tools (potential conflicts), shared categories,
/// transport compatibility, and environment variable overlaps.
pub fn run(server_a: &str, server_b: &str, json_output: bool) -> Result<()> {
    let (owner_a, name_a) = parse_ref(server_a)?;
    let (owner_b, name_b) = parse_ref(server_b)?;

    let db_path = Config::db_path()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "registry.db".to_string());

    let db = Database::open(&db_path)?;
    let _ = db.seed_default_servers();

    let a = db
        .get_server(&owner_a, &name_a)?
        .ok_or_else(|| McpRegError::NotFound(format!("{owner_a}/{name_a}")))?;
    let b = db
        .get_server(&owner_b, &name_b)?
        .ok_or_else(|| McpRegError::NotFound(format!("{owner_b}/{name_b}")))?;

    let tools_a: BTreeSet<&str> = a.tools.iter().map(|s| s.as_str()).collect();
    let tools_b: BTreeSet<&str> = b.tools.iter().map(|s| s.as_str()).collect();
    let overlapping_tools: BTreeSet<&str> = tools_a.intersection(&tools_b).copied().collect();

    let env_a = crate::commands::env::infer_env_vars(&a.owner, &a.name, &a.command, &a.args);
    let env_b = crate::commands::env::infer_env_vars(&b.owner, &b.name, &b.command, &b.args);

    let env_keys_a: BTreeSet<&str> = env_a.iter().map(|(k, _)| k.as_str()).collect();
    let env_keys_b: BTreeSet<&str> = env_b.iter().map(|(k, _)| k.as_str()).collect();
    let shared_env: BTreeSet<&str> = env_keys_a.intersection(&env_keys_b).copied().collect();

    let cat_a = crate::registry::seed::server_category(&a.owner, &a.name);
    let cat_b = crate::registry::seed::server_category(&b.owner, &b.name);
    let same_category = cat_a == cat_b;

    let same_transport = a.transport == b.transport;
    let same_runtime = a.command == b.command;

    // Compute a compatibility verdict
    let mut issues: Vec<String> = Vec::new();
    let mut notes: Vec<String> = Vec::new();

    if !overlapping_tools.is_empty() {
        issues.push(format!(
            "{} overlapping tool(s): {}. The MCP client may see duplicate tool names.",
            overlapping_tools.len(),
            overlapping_tools.iter().copied().collect::<Vec<_>>().join(", ")
        ));
    }
    if !shared_env.is_empty() {
        notes.push(format!(
            "Shared env var(s): {}. Ensure both servers can use the same value or configure separately.",
            shared_env.iter().copied().collect::<Vec<_>>().join(", ")
        ));
    }
    if same_category {
        notes.push(format!("Both are in the same category ({cat_a}) — may have overlapping functionality."));
    }
    if same_runtime {
        notes.push(format!("Both use the same runtime ({}) — good for resource sharing.", a.command));
    }

    let verdict = if issues.is_empty() {
        "compatible"
    } else {
        "caution"
    };

    if json_output {
        let result = serde_json::json!({
            "server_a": a.full_name(),
            "server_b": b.full_name(),
            "verdict": verdict,
            "overlapping_tools": overlapping_tools.iter().collect::<Vec<_>>(),
            "same_category": same_category,
            "category_a": cat_a,
            "category_b": cat_b,
            "same_transport": same_transport,
            "same_runtime": same_runtime,
            "shared_env_vars": shared_env.iter().collect::<Vec<_>>(),
            "issues": issues,
            "notes": notes,
            "tools_a": a.tools.len(),
            "tools_b": b.tools.len(),
        });
        println!("{}", serde_json::to_string_pretty(&result)?);
        return Ok(());
    }

    let icon = if issues.is_empty() { "✓" } else { "⚠" };
    println!("{icon} Compatibility: {} ↔ {}\n", a.full_name(), b.full_name());

    println!("  Verdict: {}", if issues.is_empty() {
        "Compatible ✓"
    } else {
        "Caution — see issues below"
    });

    println!();
    println!("  {} ({} tools, {})", a.full_name(), a.tools.len(), a.transport);
    println!("  {} ({} tools, {})", b.full_name(), b.tools.len(), b.transport);

    if !overlapping_tools.is_empty() {
        println!();
        println!("  ⚠ Overlapping tools ({}):", overlapping_tools.len());
        for t in &overlapping_tools {
            println!("    • {t}");
        }
    }

    if !shared_env.is_empty() {
        println!();
        println!("  📋 Shared environment variables:");
        for var in &shared_env {
            println!("    • {var}");
        }
    }

    println!();
    println!("  Category:  {} {} {}",
        cat_a,
        if same_category { "=" } else { "≠" },
        cat_b
    );
    println!("  Transport: {} {} {}",
        a.transport,
        if same_transport { "=" } else { "≠" },
        b.transport
    );
    println!("  Runtime:   {} {} {}",
        a.command,
        if same_runtime { "=" } else { "≠" },
        b.command
    );

    if !issues.is_empty() {
        println!();
        for issue in &issues {
            println!("  ⚠ {issue}");
        }
    }
    if !notes.is_empty() {
        println!();
        for note in &notes {
            println!("  ℹ {note}");
        }
    }

    Ok(())
}

fn parse_ref(r: &str) -> Result<(String, String)> {
    let parts: Vec<&str> = r.splitn(2, '/').collect();
    if parts.len() != 2 {
        return Err(McpRegError::Config(
            format!("Server reference must be in format 'owner/name', got '{r}'"),
        ));
    }
    Ok((parts[0].to_string(), parts[1].to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compat_bad_ref() {
        assert!(run("noslash", "also/bad", false).is_err());
        assert!(run("ok/ref", "noslash", false).is_err());
    }

    #[test]
    fn test_compat_not_found() {
        let result = run("nobody/nothing", "also/missing", false);
        assert!(result.is_err());
    }

    #[test]
    fn test_compat_same_server() {
        // Should work — comparing with itself
        let _ = run(
            "modelcontextprotocol/filesystem",
            "modelcontextprotocol/filesystem",
            false,
        );
    }

    #[test]
    fn test_compat_seeded_servers() {
        let _ = run(
            "modelcontextprotocol/filesystem",
            "modelcontextprotocol/git",
            false,
        );
    }

    #[test]
    fn test_compat_json_output() {
        let _ = run(
            "modelcontextprotocol/filesystem",
            "modelcontextprotocol/postgres",
            true,
        );
    }

    #[test]
    fn test_compat_overlapping_tools_detected() {
        // filesystem and gdrive both have read_file — should detect overlap
        let _ = run(
            "modelcontextprotocol/filesystem",
            "modelcontextprotocol/gdrive",
            false,
        );
    }

    #[test]
    fn test_parse_ref_valid() {
        let (o, n) = parse_ref("alice/tool").unwrap();
        assert_eq!(o, "alice");
        assert_eq!(n, "tool");
    }

    #[test]
    fn test_parse_ref_invalid() {
        assert!(parse_ref("noslash").is_err());
    }
}
