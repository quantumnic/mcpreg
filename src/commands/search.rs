use crate::api::client::RegistryClient;
use crate::config::Config;
use crate::error::Result;

pub async fn run(query: &str, json_output: bool, category: Option<&str>) -> Result<()> {
    let config = Config::load()?;
    let client = RegistryClient::new(&config);
    let response = client.search(query).await?;

    // Client-side category filter (server may not support it)
    let servers: Vec<_> = if let Some(cat) = category {
        let cat_lower = cat.to_lowercase();
        response
            .servers
            .into_iter()
            .filter(|s| {
                let server_cat = crate::registry::seed::server_category(&s.owner, &s.name).to_lowercase();
                server_cat.contains(&cat_lower)
            })
            .collect()
    } else {
        response.servers
    };

    if json_output {
        let resp = crate::api::types::SearchResponse {
            total: servers.len(),
            servers,
        };
        println!("{}", serde_json::to_string_pretty(&resp)?);
        return Ok(());
    }

    if servers.is_empty() {
        println!("No servers found for '{query}'");
        if let Some(cat) = category {
            println!("  (filtered by category '{cat}')");
        }
        return Ok(());
    }

    println!("Found {} server(s) matching '{query}':\n", servers.len());
    for server in &servers {
        println!(
            "  {} v{} — {}",
            server.full_name(),
            server.version,
            server.description
        );
        if !server.tools.is_empty() {
            let tools_display: Vec<_> = server.tools.iter().take(5).cloned().collect();
            let suffix = if server.tools.len() > 5 {
                format!(" (+{} more)", server.tools.len() - 5)
            } else {
                String::new()
            };
            println!("    Tools: {}{}", tools_display.join(", "), suffix);
        }
        println!(
            "    ⬇ {} downloads | {} | {}",
            server.downloads, server.license, server.transport
        );
        println!();
    }
    Ok(())
}
