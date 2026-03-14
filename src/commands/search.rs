use crate::api::client::RegistryClient;
use crate::config::Config;
use crate::error::Result;
use crate::registry::db::Database;
use crate::SortOrder;

#[allow(clippy::too_many_arguments)]
pub async fn run(
    query: &str,
    json_output: bool,
    category: Option<&str>,
    sort: &SortOrder,
    limit: Option<usize>,
    compact: bool,
    offline: bool,
    regex_mode: bool,
    verbose: bool,
    min_downloads: Option<i64>,
    tool_filter: Option<&str>,
    transport_filter: Option<&str>,
    author_filter: Option<&str>,
    owner_filter: Option<&str>,
    tag_filter: Option<&str>,
    license_filter: Option<&str>,
) -> Result<()> {
    // Regex mode always uses local DB
    let mut servers = if regex_mode {
        let db_path = Config::db_path()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| "registry.db".to_string());
        let db = Database::open(&db_path)?;
        let _ = db.seed_default_servers();
        db.search_regex(query)?
    } else if offline {
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

        // Use OR search if query contains pipe character
        if query.contains('|') {
            db.search_any(query)?
        } else {
            db.search(query)?
        }
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

    // Min downloads filter
    if let Some(min) = min_downloads {
        servers.retain(|s| s.downloads >= min);
    }

    // Tool name filter
    if let Some(tool) = tool_filter {
        let tool_lower = tool.to_lowercase();
        servers.retain(|s| {
            s.tools.iter().any(|t| t.to_lowercase().contains(&tool_lower))
        });
    }

    // Transport filter
    if let Some(transport) = transport_filter {
        let t_lower = transport.to_lowercase();
        servers.retain(|s| s.transport.to_lowercase() == t_lower);
    }

    // Author filter
    if let Some(author) = author_filter {
        let author_lower = author.to_lowercase();
        servers.retain(|s| s.author.to_lowercase().contains(&author_lower));
    }

    // Owner filter
    if let Some(owner) = owner_filter {
        let owner_lower = owner.to_lowercase();
        servers.retain(|s| s.owner.to_lowercase().contains(&owner_lower));
    }

    // Tag filter
    if let Some(tag) = tag_filter {
        let tag_lower = tag.to_lowercase();
        servers.retain(|s| {
            s.tags.iter().any(|t| t.to_lowercase().contains(&tag_lower))
        });
    }

    // License filter
    if let Some(license) = license_filter {
        let lic_lower = license.to_lowercase();
        servers.retain(|s| s.license.to_lowercase().contains(&lic_lower));
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
        SortOrder::Stars => {
            servers.sort_by(|a, b| b.stars.cmp(&a.stars));
        }
    }

    // Apply limit
    if let Some(n) = limit {
        servers.truncate(n);
    }

    if json_output {
        let resp = crate::api::types::SearchResponse {
            total: servers.len(),
            servers,
            suggestions: None,
        };
        println!("{}", serde_json::to_string_pretty(&resp)?);
        return Ok(());
    }

    if servers.is_empty() {
        println!("No servers found for '{query}'");
        if let Some(cat) = category {
            println!("  (filtered by category '{cat}')");
        }

        // Fuzzy suggestions
        if !query.is_empty() {
            suggest_similar(query, offline).await;
        }

        return Ok(());
    }

    if compact {
        for (i, server) in servers.iter().enumerate() {
            if verbose {
                println!(
                    "#{:<3} {} v{} — {} (⬇ {})",
                    i + 1,
                    server.full_name(),
                    server.version,
                    server.description,
                    server.downloads,
                );
            } else {
                println!(
                    "{} v{} — {} (⬇ {})",
                    server.full_name(),
                    server.version,
                    server.description,
                    server.downloads,
                );
            }
        }
    } else {
        println!("Found {} server(s) matching '{query}':\n", servers.len());
        for (i, server) in servers.iter().enumerate() {
            let cat = crate::registry::seed::server_category(&server.owner, &server.name);
            if verbose {
                println!(
                    "  #{} {} v{} — {}  [{}]",
                    i + 1,
                    server.full_name(),
                    server.version,
                    server.description,
                    cat,
                );
            } else {
                println!(
                    "  {} v{} — {}  [{}]",
                    server.full_name(),
                    server.version,
                    server.description,
                    cat,
                );
            }
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
            if server.deprecated {
                let replacement = server.deprecated_by.as_deref().unwrap_or("unknown");
                println!("    ⚠️  DEPRECATED — replaced by {replacement}");
            }
            if verbose {
                println!(
                    "    ⬇ {} downloads | {} | {} | {} tools | {} tags",
                    server.downloads, server.license, server.transport,
                    server.tools.len(), server.tags.len(),
                );
            } else {
                println!(
                    "    ⬇ {} downloads | {} | {}",
                    server.downloads, server.license, server.transport
                );
            }
            println!();
        }
    }
    Ok(())
}

/// Show fuzzy suggestions when search returns no results.
async fn suggest_similar(query: &str, offline: bool) {
    let all_names = if offline {
        let db_path = Config::db_path()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| "registry.db".to_string());
        match Database::open(&db_path) {
            Ok(db) => {
                let (servers, _) = db.list_servers(1, 1000).unwrap_or_default();
                servers.iter().map(|s| s.full_name()).collect::<Vec<_>>()
            }
            Err(_) => return,
        }
    } else {
        // For remote, try to fetch all
        let config = match Config::load() {
            Ok(c) => c,
            Err(_) => return,
        };
        let client = RegistryClient::new(&config);
        match client.search("").await {
            Ok(resp) => resp.servers.iter().map(|s| s.full_name()).collect(),
            Err(_) => return,
        }
    };

    let suggestions = crate::fuzzy::suggest(query, &all_names, 3);
    if !suggestions.is_empty() {
        println!("\n  Did you mean?");
        for (name, _) in &suggestions {
            println!("    • {name}");
        }
    }
}
