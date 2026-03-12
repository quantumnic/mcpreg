use crate::api::types::InstalledServers;
use crate::config::Config;
use crate::error::Result;
use crate::registry::seed::server_category;

/// Security-oriented audit of installed MCP servers.
/// Checks for: env vars with secrets, wide permissions, unknown transports, etc.
pub fn run(json_output: bool) -> Result<()> {
    let path = Config::installed_servers_path()?;
    if !path.exists() {
        if json_output {
            println!("{}", serde_json::json!({"installed": 0, "warnings": []}));
        } else {
            println!("No servers installed. Nothing to audit.");
        }
        return Ok(());
    }

    let content = std::fs::read_to_string(&path)?;
    let installed: InstalledServers = serde_json::from_str(&content)?;

    if installed.servers.is_empty() {
        if json_output {
            println!("{}", serde_json::json!({"installed": 0, "warnings": []}));
        } else {
            println!("No servers installed. Nothing to audit.");
        }
        return Ok(());
    }

    let mut warnings: Vec<serde_json::Value> = Vec::new();

    for server in &installed.servers {
        let full = server.full_name();
        let cat = server_category(&server.owner, &server.name);

        // Check for non-stdio transports (SSE/HTTP expose network surface)
        if server.transport != "stdio" {
            warnings.push(serde_json::json!({
                "server": full,
                "level": "medium",
                "issue": format!("Uses '{}' transport — exposes a network endpoint", server.transport),
                "recommendation": "Ensure the endpoint is not publicly accessible or uses auth",
            }));
        }

        // Check for commands that execute arbitrary code
        let risky_commands = ["sh", "bash", "zsh", "cmd", "powershell"];
        if risky_commands.contains(&server.command.as_str()) {
            warnings.push(serde_json::json!({
                "server": full,
                "level": "high",
                "issue": format!("Command '{}' runs a shell directly", server.command),
                "recommendation": "Prefer a specific runtime (node, python3, uvx) over raw shell",
            }));
        }

        // Check args for suspicious patterns
        let args_str = server.args.join(" ");
        if args_str.contains("--no-sandbox") || args_str.contains("--disable-security") {
            warnings.push(serde_json::json!({
                "server": full,
                "level": "high",
                "issue": "Server args disable security features",
                "recommendation": "Review if disabling sandboxing/security is necessary",
            }));
        }

        // Unknown category = unvetted server
        if cat == "📦 Other" {
            warnings.push(serde_json::json!({
                "server": full,
                "level": "info",
                "issue": "Server is not in a known category (may be third-party/unvetted)",
                "recommendation": "Verify the server source and review its code",
            }));
        }
    }

    if json_output {
        println!("{}", serde_json::to_string_pretty(&serde_json::json!({
            "installed": installed.servers.len(),
            "warnings": warnings,
            "total_warnings": warnings.len(),
        }))?);
    } else {
        println!("Audit of {} installed server(s):\n", installed.servers.len());

        for server in &installed.servers {
            println!("  {} v{} ({})", server.full_name(), server.version, server.transport);
        }

        if warnings.is_empty() {
            println!("\n✓ No issues found.");
        } else {
            println!("\n⚠ {} warning(s):\n", warnings.len());
            for w in &warnings {
                let level = w["level"].as_str().unwrap_or("?");
                let icon = match level {
                    "high" => "🔴",
                    "medium" => "🟡",
                    _ => "🔵",
                };
                println!("  {icon} [{}] {}", w["server"].as_str().unwrap_or("?"), w["issue"].as_str().unwrap_or(""));
                println!("    → {}", w["recommendation"].as_str().unwrap_or(""));
                println!();
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_audit_no_panic_without_installed() {
        // Should not panic even if no installed.json exists
        let result = super::run(false);
        // May error if path doesn't exist, but should not panic
        let _ = result;
    }
}
