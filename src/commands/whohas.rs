use crate::config::Config;
use crate::error::Result;
use clap::Args;

#[derive(Args)]
pub struct WhohasArgs {
    /// Resource type to search for (e.g. "file://", "postgres://")
    pub resource: String,

    /// Maximum results to display
    #[arg(short, long, default_value = "20")]
    pub limit: usize,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

pub async fn run(args: &WhohasArgs, config: &Config) -> Result<()> {
    let url = format!(
        "{}/api/v1/search?q=&resource={}&limit={}",
        config.registry_url.trim_end_matches('/'),
        &args.resource,
        args.limit
    );

    let resp = reqwest::get(&url).await?;
    let body: serde_json::Value = resp.json().await?;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&body)?);
        return Ok(());
    }

    let empty = vec![]; let servers = body["servers"].as_array().unwrap_or(&empty);
    if servers.is_empty() {
        println!("No servers found providing resource type '{}'.", args.resource);
        return Ok(());
    }

    println!("🔍 Servers providing '{}' resources:\n", args.resource);
    for s in servers {
        let name = s["owner"].as_str().unwrap_or("?");
        let sname = s["name"].as_str().unwrap_or("?");
        let desc = s["description"].as_str().unwrap_or("");
        let dl = s["downloads"].as_i64().unwrap_or(0);
        let resources: Vec<&str> = s["resources"]
            .as_array()
            .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
            .unwrap_or_default();

        println!("  {name}/{sname}  ⬇ {dl}");
        if !desc.is_empty() {
            let short = if desc.len() > 60 { &desc[..60] } else { desc };
            println!("    {short}");
        }
        let matching: Vec<&&str> = resources.iter()
            .filter(|r| r.to_lowercase().contains(&args.resource.to_lowercase()))
            .collect();
        if !matching.is_empty() {
            println!("    Resources: {}", matching.iter().map(|r| **r).collect::<Vec<_>>().join(", "));
        }
        println!();
    }

    println!("Total: {} servers", body["total"].as_u64().unwrap_or(0));
    Ok(())
}
