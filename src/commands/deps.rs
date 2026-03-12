use crate::config::Config;
use crate::error::{McpRegError, Result};
use crate::registry::db::Database;

/// Show dependencies and requirements for an MCP server (runtime, env vars, transport).
pub fn run(server_ref: &str, json_output: bool) -> Result<()> {
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
    let _ = db.seed_default_servers();

    let entry = db
        .get_server(owner, name)?
        .ok_or_else(|| McpRegError::NotFound(format!("{owner}/{name}")))?;

    // Check runtime availability
    let runtime = detect_runtime(&entry.command);
    let env_vars = crate::commands::env::infer_env_vars(&entry.owner, &entry.name, &entry.command, &entry.args);

    if json_output {
        let v = serde_json::json!({
            "server": entry.full_name(),
            "runtime": {
                "command": &entry.command,
                "available": runtime.available,
                "version": runtime.version,
            },
            "transport": &entry.transport,
            "environment_variables": env_vars.iter().map(|(k, v)| serde_json::json!({"name": k, "description": v})).collect::<Vec<_>>(),
            "tools_count": entry.tools.len(),
            "resources_count": entry.resources.len(),
            "prompts_count": entry.prompts.len(),
        });
        println!("{}", serde_json::to_string_pretty(&v)?);
        return Ok(());
    }

    println!("{} — dependencies\n", entry.full_name());
    println!("  Runtime:");
    let status = if runtime.available { "✓" } else { "✗" };
    let ver = runtime
        .version
        .as_deref()
        .map(|v| format!(" ({v})"))
        .unwrap_or_default();
    println!("    {status} {} {ver}", entry.command);
    if !runtime.available {
        println!("      ⚠ Not found in PATH. Install it to use this server.");
    }

    println!("\n  Transport: {}", entry.transport);

    if env_vars.is_empty() {
        println!("\n  Environment: No known env vars required.");
    } else {
        println!("\n  Environment variables:");
        for (var, desc) in &env_vars {
            // Check if set
            let set = std::env::var(var).is_ok();
            let marker = if set { "✓" } else { "○" };
            println!("    {marker} {var}");
            println!("      {desc}");
        }
    }

    println!(
        "\n  Capabilities: {} tools, {} resources, {} prompts",
        entry.tools.len(),
        entry.resources.len(),
        entry.prompts.len()
    );

    Ok(())
}

struct RuntimeInfo {
    available: bool,
    version: Option<String>,
}

fn detect_runtime(command: &str) -> RuntimeInfo {
    let version_flag = match command {
        "python" | "python3" => "--version",
        _ => "--version",
    };

    match std::process::Command::new(command)
        .arg(version_flag)
        .output()
    {
        Ok(output) if output.status.success() => {
            let ver = String::from_utf8_lossy(&output.stdout)
                .trim()
                .to_string();
            let ver = if ver.is_empty() {
                String::from_utf8_lossy(&output.stderr).trim().to_string()
            } else {
                ver
            };
            RuntimeInfo {
                available: true,
                version: if ver.is_empty() { None } else { Some(ver) },
            }
        }
        _ => RuntimeInfo {
            available: false,
            version: None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deps_bad_ref() {
        assert!(run("noslash", false).is_err());
    }

    #[test]
    fn test_detect_runtime_exists() {
        // "echo" should be available on any system
        let info = detect_runtime("echo");
        // echo --version may or may not work, but at least it won't panic
        let _ = info;
    }
}
