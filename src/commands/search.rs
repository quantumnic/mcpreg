use crate::api::client::RegistryClient;
use crate::config::Config;
use crate::error::Result;
use crate::registry::db::Database;
use crate::SortOrder;

pub async fn run(
    query: &str,
    json_output: bool,
    category: Option<&str>,
    sort: &SortOrder,
    limit: Option<usize>,
    compact: bool,
    offline: bool,
) -> Result<()> {
    let mut servers = if offline {
        // Search local database directly
        let db_path = Config::db_path()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| "registry.db".to_string());

        let db = Database::open(&db_path)?;

        // Seed if empty so offline search has data
        match db.seed_default_servers() {
            Ok(0) => {}
            Ok(n) => {
                if !json_output {
                    eprintln!("ℹ  Seeded {n} default servers into local registry.");
                }
            }
            Err(e) => {
                if !json_output {
                    eprintln!("⚠  Could not seed defaults: {e}");
                }
            }
        }

        db.search(query)?
    } else {
        let config = Config::load()?;
        let client = RegistryClient::new(&config);
        let response = client.search(query).await?;
        response.servers
    };

    // Client-side category filter
    if let Some(cat) = category {
        let cat_lower = cat.to_lowercase();
        servers.retain(|s| {
            let server_cat =
                crate::registry::seed::server_category(&s.owner, &s.name).to_lowercase();
            server_cat.contains(&cat_lower)
        });
    }

    // Client-side sorting
    match sort {
        SortOrder::Name => {
            servers.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        }
        SortOrder::Updated => servers.sort_by(|a, b| {
            let a_time = a.updated_at.as_deref().unwrap_or("");
            let b_time = b.updated_at.as_deref().unwrap_or("");
            b_time.cmp(a_time)
        }),
        SortOrder::Downloads => {} // already sorted by server
    }

    // Apply limit
    if let Some(n) = limit {
        servers.truncate(n);
    }

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

    if compact {
        for server in &servers {
            println!(
                "{} v{} — {} (⬇ {})",
                server.full_name(),
                server.version,
                server.description,
                server.downloads,
            );
        }
    } else {
        println!("Found {} server(s) matching '{query}':\n", servers.len());
        for server in &servers {
            let cat = crate::registry::seed::server_category(&server.owner, &server.name);
            println!(
                "  {} v{} — {}  [{}]",
                server.full_name(),
                server.version,
                server.description,
                cat,
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
            if !server.prompts.is_empty() {
                println!("    Prompts: {}", server.prompts.join(", "));
            }
            println!(
                "    ⬇ {} downloads | {} | {}",
                server.downloads, server.license, server.transport
            );
            println!();
        }
    }
    Ok(())
}
