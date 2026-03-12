use crate::api::client::RegistryClient;
use crate::config::Config;
use crate::error::Result;

/// Quick connectivity check against the configured registry.
pub async fn run(json_output: bool) -> Result<()> {
    let config = Config::load()?;
    let client = RegistryClient::new(&config);

    let start = std::time::Instant::now();
    let result = client.health().await;
    let elapsed = start.elapsed();

    match result {
        Ok(info) => {
            if json_output {
                let mut map = info.clone();
                map.insert("latency_ms".to_string(), serde_json::json!(elapsed.as_millis()));
                map.insert("registry_url".to_string(), serde_json::json!(config.registry_url));
                println!("{}", serde_json::to_string_pretty(&map)?);
            } else {
                let status = info.get("status").and_then(|v| v.as_str()).unwrap_or("unknown");
                let version = info.get("version").and_then(|v| v.as_str()).unwrap_or("?");
                let servers = info.get("servers").and_then(|v| v.as_i64()).unwrap_or(-1);

                let icon = if status == "ok" { "✓" } else { "✗" };
                println!("{icon} Registry: {}", config.registry_url);
                println!("  Status:  {status}");
                println!("  Version: {version}");
                if servers >= 0 {
                    println!("  Servers: {servers}");
                }
                println!("  Latency: {}ms", elapsed.as_millis());
            }
        }
        Err(e) => {
            if json_output {
                println!("{}", serde_json::json!({
                    "status": "error",
                    "error": e.to_string(),
                    "registry_url": config.registry_url,
                    "latency_ms": elapsed.as_millis(),
                }));
            } else {
                eprintln!("✗ Cannot reach registry: {}", config.registry_url);
                eprintln!("  Error: {e}");
                eprintln!("  Latency: {}ms", elapsed.as_millis());
            }
            return Err(e);
        }
    }

    Ok(())
}
