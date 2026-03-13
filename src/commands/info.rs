use crate::api::client::RegistryClient;
use crate::config::Config;
use crate::error::{McpRegError, Result};

pub async fn run(server_ref: &str, json_output: bool) -> Result<()> {
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

    if json_output {
        println!("{}", serde_json::to_string_pretty(&entry)?);
        return Ok(());
    }

    println!("{}/{} v{}", entry.owner, entry.name, entry.version);
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    if !entry.description.is_empty() {
        println!("Description: {}", entry.description);
    }
    if !entry.author.is_empty() {
        println!("Author:      {}", entry.author);
    }
    if !entry.license.is_empty() {
        println!("License:     {}", entry.license);
    }
    if !entry.repository.is_empty() {
        println!("Repository:  {}", entry.repository);
    }
    println!("Transport:   {}", entry.transport);
    println!("Command:     {} {}", entry.command, entry.args.join(" "));
    println!("Downloads:   {}", entry.downloads);

    if !entry.tools.is_empty() {
        println!("\nTools ({}):", entry.tools.len());
        for tool in &entry.tools {
            println!("  • {tool}");
        }
    }
    if !entry.resources.is_empty() {
        println!("\nResources:");
        for res in &entry.resources {
            println!("  • {res}");
        }
    }
    if !entry.prompts.is_empty() {
        println!("\nPrompts ({}):", entry.prompts.len());
        for prompt in &entry.prompts {
            println!("  • {prompt}");
        }
    }
    if let Some(ref created) = entry.created_at {
        println!("\nCreated: {created}");
    }
    if let Some(ref updated) = entry.updated_at {
        println!("Updated: {updated}");
    }

    if entry.deprecated {
        println!("\n⚠️  DEPRECATED");
        if let Some(ref replacement) = entry.deprecated_by {
            println!("   Replaced by: {replacement}");
        }
    }

    // Installation hint
    println!("\nInstall: mcpreg install {}/{}", entry.owner, entry.name);

    Ok(())
}
