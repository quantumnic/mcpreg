use crate::api::client::RegistryClient;
use crate::config::Config;
use crate::error::Result;

pub async fn run(query: &str) -> Result<()> {
    let config = Config::load()?;
    let client = RegistryClient::new(&config);
    let response = client.search(query).await?;

    if response.servers.is_empty() {
        println!("No servers found for '{query}'");
        return Ok(());
    }

    println!("Found {} server(s) matching '{query}':\n", response.total);
    for server in &response.servers {
        println!(
            "  {} v{} — {}",
            server.full_name(),
            server.version,
            server.description
        );
        if !server.tools.is_empty() {
            println!("    Tools: {}", server.tools.join(", "));
        }
        println!("    Downloads: {} | License: {}", server.downloads, server.license);
        println!();
    }
    Ok(())
}
