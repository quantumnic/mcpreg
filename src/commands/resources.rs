use crate::api::client::RegistryClient;
use crate::config::Config;
use crate::error::Result;
use clap::Args;

#[derive(Args)]
pub struct ResourcesArgs {
    /// Filter resources by name
    #[arg(short, long)]
    pub query: Option<String>,

    /// Maximum number of resources to display
    #[arg(short, long, default_value = "30")]
    pub limit: usize,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

pub async fn run(args: &ResourcesArgs, config: &Config) -> Result<()> {
    let client = RegistryClient::new(config);
    let mut url = format!("{}/api/v1/resources?limit={}", config.registry_url.trim_end_matches('/'), args.limit);
    if let Some(ref q) = args.query {
        url.push_str(&format!("&q={q}"));
    }

    let resp = reqwest::get(&url).await?;
    let body: serde_json::Value = resp.json().await?;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&body)?);
        return Ok(());
    }

    let empty = vec![]; let resources = body["resources"].as_array().unwrap_or(&empty);
    if resources.is_empty() {
        println!("No resources found.");
        return Ok(());
    }

    println!("📦 Resources across the registry:\n");
    for item in resources {
        let resource = item["resource"].as_str().unwrap_or("?");
        let count = item["server_count"].as_u64().unwrap_or(0);
        let servers: Vec<&str> = item["servers"]
            .as_array()
            .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
            .unwrap_or_default();

        println!("  {} ({} server{})", resource, count, if count == 1 { "" } else { "s" });
        for s in servers.iter().take(5) {
            println!("    └─ {s}");
        }
        if servers.len() > 5 {
            println!("    └─ ... and {} more", servers.len() - 5);
        }
    }

    println!("\nTotal: {} unique resource types", body["total"].as_u64().unwrap_or(0));
    let _ = client;
    Ok(())
}
