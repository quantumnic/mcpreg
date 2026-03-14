use crate::api::client::RegistryClient;
use crate::config::Config;
use crate::error::{McpRegError, Result};
use std::process::Command;

pub async fn run(server_ref: &str, extra_args: &[String], dry_run: bool) -> Result<()> {
    let parts: Vec<&str> = server_ref.splitn(2, '/').collect();
    if parts.len() != 2 {
        return Err(McpRegError::Config(
            "Server reference must be in format 'owner/name'".into(),
        ));
    }
    let (owner, name) = (parts[0], parts[1]);

    let config = Config::load()?;
    let client = RegistryClient::new(&config);
    let entry = client.get_server(owner, name).await?;

    if entry.deprecated {
        let replacement = entry.deprecated_by.as_deref().unwrap_or("unknown");
        eprintln!(
            "⚠️  Warning: {}/{} is deprecated. Consider using {replacement} instead.",
            entry.owner, entry.name
        );
    }

    let mut args: Vec<String> = entry.args.clone();
    args.extend_from_slice(extra_args);

    if dry_run {
        println!("Would execute:");
        println!("  Command:   {}", entry.command);
        println!("  Args:      {}", args.join(" "));
        println!("  Transport: {}", entry.transport);
        if !entry.env.is_empty() {
            println!("  Env vars:");
            for (k, v) in &entry.env {
                println!("    {k}={v}");
            }
        }
        return Ok(());
    }

    eprintln!(
        "▶ Running {}/{} v{} ({} transport)",
        entry.owner, entry.name, entry.version, entry.transport
    );

    let mut cmd = Command::new(&entry.command);
    cmd.args(&args);
    for (k, v) in &entry.env {
        cmd.env(k, v);
    }

    let status = cmd.status().map_err(|e| {
        McpRegError::Config(format!(
            "Failed to execute '{}': {e}. Is it installed?",
            entry.command
        ))
    })?;

    if !status.success() {
        let code = status.code().unwrap_or(-1);
        return Err(McpRegError::Config(format!(
            "Server process exited with code {code}"
        )));
    }

    Ok(())
}
