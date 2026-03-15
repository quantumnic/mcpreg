use crate::api::client::RegistryClient;
use crate::api::types::ServerEntry;
use crate::config::Config;
use crate::error::{McpRegError, Result};
use crate::registry::db::Database;

pub async fn run(server_ref: &str, json_output: bool) -> Result<()> {
    let parts: Vec<&str> = server_ref.splitn(2, '/').collect();
    if parts.len() != 2 {
        return Err(McpRegError::Config(
            "Server reference must be in format 'owner/name'".into(),
        ));
    }
    let (owner, name) = (parts[0], parts[1]);

    // Try remote first, fall back to local DB
    let entry = match fetch_remote(owner, name).await {
        Ok(e) => e,
        Err(_) => fetch_local(owner, name)?,
    };

    if json_output {
        println!("{}", serde_json::to_string_pretty(&entry)?);
        return Ok(());
    }

    let cat = crate::registry::seed::server_category(&entry.owner, &entry.name);

    println!("╔═══════════════════════════════════════════════╗");
    println!(
        "║  {}/{} v{}",
        entry.owner, entry.name, entry.version
    );
    println!("╠═══════════════════════════════════════════════╣");
    if !entry.description.is_empty() {
        println!("║  {}", entry.description);
        println!("║");
    }
    if !entry.author.is_empty() {
        println!("║  Author:      {}", entry.author);
    }
    if !entry.license.is_empty() {
        println!("║  License:     {}", entry.license);
    }
    if !entry.repository.is_empty() {
        println!("║  Repository:  {}", entry.repository);
    }
    if !entry.homepage.is_empty() {
        println!("║  Homepage:    {}", entry.homepage);
    }
    println!("║  Category:    {cat}");
    println!("║  Transport:   {}", entry.transport);
    println!(
        "║  Command:     {} {}",
        entry.command,
        entry.args.join(" ")
    );

    let dl = crate::color::format_downloads(entry.downloads);
    if entry.stars > 0 {
        let stars = crate::color::format_stars(entry.stars);
        println!("║  Downloads:   {dl}  |  Stars: {stars}");
    } else {
        println!("║  Downloads:   {dl}");
    }

    if !entry.env.is_empty() {
        println!("║");
        println!("║  Environment variables:");
        for (key, val) in &entry.env {
            println!("║    {key} = {val}");
        }
    }

    if !entry.tools.is_empty() {
        println!("╠═══════════════════════════════════════════════╣");
        println!("║  Tools ({}):", entry.tools.len());
        for tool in &entry.tools {
            println!("║    • {tool}");
        }
    }
    if !entry.resources.is_empty() {
        println!("║  Resources ({}):", entry.resources.len());
        for res in &entry.resources {
            println!("║    • {res}");
        }
    }
    if !entry.prompts.is_empty() {
        println!("║  Prompts ({}):", entry.prompts.len());
        for prompt in &entry.prompts {
            println!("║    • {prompt}");
        }
    }
    if !entry.tags.is_empty() {
        println!("║  Tags: {}", entry.tags.join(", "));
    }

    println!("╠═══════════════════════════════════════════════╣");
    if let Some(ref created) = entry.created_at {
        println!("║  Created: {created}");
    }
    if let Some(ref updated) = entry.updated_at {
        println!("║  Updated: {updated}");
    }

    if entry.deprecated {
        println!("║");
        println!("║  ⚠️  DEPRECATED");
        if let Some(ref replacement) = entry.deprecated_by {
            println!("║     Replaced by: {replacement}");
        }
    }

    println!("╚═══════════════════════════════════════════════╝");
    println!();
    println!(
        "  Install: mcpreg install {}/{}",
        entry.owner, entry.name
    );
    println!(
        "  Badge:   https://img.shields.io/endpoint?url=<registry>/api/v1/servers/{}/{}/shield",
        entry.owner, entry.name
    );

    Ok(())
}

async fn fetch_remote(owner: &str, name: &str) -> Result<ServerEntry> {
    let config = Config::load()?;
    let client = RegistryClient::new(&config);
    client.get_server(owner, name).await
}

fn fetch_local(owner: &str, name: &str) -> Result<ServerEntry> {
    let db_path = Config::db_path()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "registry.db".to_string());
    let db = Database::open(&db_path)?;
    let _ = db.seed_default_servers();
    db.get_server(owner, name)?
        .ok_or_else(|| McpRegError::NotFound(format!("{owner}/{name}")))
}
