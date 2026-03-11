use crate::api::types::InstalledServers;
use crate::config::Config;
use crate::error::Result;

pub fn run(json_output: bool) -> Result<()> {
    let path = Config::installed_servers_path()?;

    if !path.exists() {
        if json_output {
            println!("{}", serde_json::to_string_pretty(&InstalledServers::default())?);
        } else {
            println!("No MCP servers installed yet.");
            println!("Use 'mcpreg install owner/server-name' to install one.");
        }
        return Ok(());
    }

    let content = std::fs::read_to_string(&path)?;
    let installed: InstalledServers = serde_json::from_str(&content)?;

    if json_output {
        println!("{}", serde_json::to_string_pretty(&installed)?);
        return Ok(());
    }

    if installed.servers.is_empty() {
        println!("No MCP servers installed.");
        return Ok(());
    }

    println!("Installed MCP servers:\n");
    for server in &installed.servers {
        println!(
            "  {} v{}",
            server.full_name(),
            server.version
        );
        println!(
            "    Command: {} {}",
            server.command,
            server.args.join(" ")
        );
        println!("    Transport: {}", server.transport);
        println!();
    }
    println!("Total: {} server(s)", installed.servers.len());

    Ok(())
}
