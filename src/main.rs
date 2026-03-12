mod api;
mod commands;
mod config;
mod error;
mod fuzzy;
mod registry;

use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(
    name = "mcpreg",
    version,
    about = "Open source registry and marketplace for MCP servers",
    long_about = "mcpreg — search, install, publish, and manage MCP (Model Context Protocol) servers.\n\nLike npm or crates.io, but for MCP servers. Self-hostable."
)]
pub struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Clone, ValueEnum, Debug, Default)]
pub enum SortOrder {
    /// Sort by relevance / downloads (default)
    #[default]
    Downloads,
    /// Sort alphabetically by name
    Name,
    /// Sort by most recently updated
    Updated,
}

#[derive(Subcommand)]
enum Commands {
    /// Search for MCP servers in the registry
    Search {
        /// Search query (supports multiple words for AND matching)
        query: String,
        /// Output results as JSON
        #[arg(long)]
        json: bool,
        /// Filter results by category (e.g. "database", "search", "browser")
        #[arg(short, long)]
        category: Option<String>,
        /// Sort order: downloads (default), name, updated
        #[arg(short, long, value_enum, default_value_t = SortOrder::Downloads)]
        sort: SortOrder,
        /// Maximum number of results to show
        #[arg(short = 'n', long)]
        limit: Option<usize>,
        /// Compact one-line-per-result output
        #[arg(long)]
        compact: bool,
        /// Search the local database instead of the remote registry
        #[arg(long)]
        offline: bool,
        /// Only show servers with at least this many downloads
        #[arg(long)]
        min_downloads: Option<i64>,
        /// Filter by tool name (only show servers that expose this tool)
        #[arg(short = 't', long)]
        tool: Option<String>,
        /// Filter by transport type (stdio, sse, streamable-http)
        #[arg(long)]
        transport: Option<String>,
    },
    /// Install an MCP server and add it to claude_desktop_config.json
    Install {
        /// Server reference (owner/name)
        server: String,
    },
    /// Uninstall an MCP server and remove it from claude_desktop_config.json
    Uninstall {
        /// Server reference (owner/name)
        server: String,
    },
    /// Publish an MCP server to the registry
    Publish {
        /// Path to mcpreg.toml manifest (default: ./mcpreg.toml)
        #[arg(short, long)]
        manifest: Option<String>,
    },
    /// List installed MCP servers
    List {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Show detailed information about an MCP server
    Info {
        /// Server reference (owner/name)
        server: String,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Browse all servers in the registry (paginated, with categories)
    Browse {
        /// Page number (default: 1)
        #[arg(short, long, default_value = "1")]
        page: usize,
        /// Results per page (default: 20)
        #[arg(short = 'n', long, default_value = "20")]
        per_page: usize,
        /// Filter by category (e.g. "database", "search", "browser")
        #[arg(short, long)]
        category: Option<String>,
        /// Sort order: downloads (default), name, updated
        #[arg(short, long, value_enum, default_value_t = SortOrder::Downloads)]
        sort: SortOrder,
        /// Only show servers with at least this many downloads
        #[arg(long)]
        min_downloads: Option<i64>,
    },
    /// List all categories with server counts
    Tags {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Export installed servers as a claude_desktop_config.json snippet
    Export {
        /// Output file path (prints to stdout if not given)
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Update installed MCP servers (all or a specific one)
    Update {
        /// Optional server reference (owner/name) to update a single server
        server: Option<String>,
    },
    /// Initialize a new mcpreg.toml manifest for your MCP server project
    Init {
        /// Directory to create mcpreg.toml in (default: current directory)
        #[arg(short, long)]
        path: Option<String>,
    },
    /// Validate an mcpreg.toml manifest
    Validate {
        /// Path to mcpreg.toml manifest (default: ./mcpreg.toml)
        #[arg(short, long)]
        manifest: Option<String>,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Show registry statistics (total servers, downloads, top servers)
    Stats {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Find servers similar to a given server (by tools, category, description)
    Similar {
        /// Server reference (owner/name)
        server: String,
        /// Maximum number of results (default: 5)
        #[arg(short = 'n', long, default_value = "5")]
        limit: usize,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Discover a random MCP server
    Random {
        /// Filter by category (e.g. "database")
        #[arg(short, long)]
        category: Option<String>,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Quick server counts (by total, transport, category, or owner)
    Count {
        /// Group by: total, transport, category, owner
        #[arg(default_value = "total")]
        by: String,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Show which installed servers have newer versions available
    Outdated {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Compare two MCP servers side-by-side (tools, resources, prompts)
    Compare {
        /// First server reference (owner/name)
        server_a: String,
        /// Second server reference (owner/name)
        server_b: String,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// View or update mcpreg configuration
    Config {
        /// Action: show, get, set, path
        #[arg(default_value = "show")]
        action: String,
        /// Config key (for get/set)
        key: Option<String>,
        /// Config value (for set)
        value: Option<String>,
    },
    /// Find which servers provide a specific tool
    Which {
        /// Tool name to search for (e.g. "read_file")
        tool: String,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Run diagnostics and check your setup
    Doctor,
    /// Show environment variables needed by an MCP server
    Env {
        /// Server reference (owner/name)
        server: String,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Pin an installed server to prevent auto-update
    Pin {
        /// Server reference (owner/name)
        server: String,
    },
    /// Unpin an installed server to allow auto-update
    Unpin {
        /// Server reference (owner/name)
        server: String,
    },
    /// List all pinned servers
    Pinned,
    /// Generate shell completions
    Completions {
        /// Shell to generate completions for
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
    /// Start the self-hosted registry server
    Serve {
        /// Bind address (default: 0.0.0.0:3000)
        #[arg(short, long, default_value = "0.0.0.0:3000")]
        bind: String,
        /// Database path (default: ~/.mcpreg/registry.db)
        #[arg(short, long)]
        db: Option<String>,
    },
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Search {
            query,
            json,
            category,
            sort,
            limit,
            compact,
            offline,
            min_downloads,
            tool,
            transport,
        } => commands::search::run(&query, json, category.as_deref(), &sort, limit, compact, offline, min_downloads, tool.as_deref(), transport.as_deref()).await,
        Commands::Install { server } => commands::install::run(&server).await,
        Commands::Uninstall { server } => commands::uninstall::run(&server),
        Commands::Publish { manifest } => commands::publish::run(manifest.as_deref()).await,
        Commands::List { json } => commands::list::run(json),
        Commands::Info { server, json } => commands::info::run(&server, json).await,
        Commands::Browse {
            page,
            per_page,
            category,
            sort,
            min_downloads,
        } => commands::browse::run(page, per_page, category.as_deref(), &sort, min_downloads),
        Commands::Tags { json } => commands::tags::run(json),
        Commands::Export { output } => commands::export::run(output.as_deref()),
        Commands::Similar { server, limit, json } => commands::similar::run(&server, limit, json),
        Commands::Random { category, json } => commands::random::run(category.as_deref(), json),
        Commands::Count { by, json } => commands::count::run(Some(&by), json),
        Commands::Outdated { json } => commands::outdated::run(json),
        Commands::Update { server } => run_update(server.as_deref()).await,
        Commands::Init { path } => commands::init::run(path.as_deref()),
        Commands::Validate { manifest, json } => {
            commands::validate::run(manifest.as_deref(), json)
        }
        Commands::Stats { json } => commands::stats::run(json),
        Commands::Compare { server_a, server_b, json } => {
            commands::compare::run(&server_a, &server_b, json)
        }
        Commands::Config { action, key, value } => {
            commands::config_cmd::run(&action, key.as_deref(), value.as_deref())
        }
        Commands::Which { tool, json } => commands::which::run(&tool, json),
        Commands::Doctor => commands::doctor::run(),
        Commands::Env { server, json } => commands::env::run(&server, json).await,
        Commands::Pin { server } => commands::pin::run_pin(&server),
        Commands::Unpin { server } => commands::pin::run_unpin(&server),
        Commands::Pinned => commands::pin::run_list(),
        Commands::Completions { shell } => commands::completions::run(shell),
        Commands::Serve { bind, db } => {
            let db_path = match db {
                Some(p) => p,
                None => config::Config::db_path()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|_| "registry.db".to_string()),
            };
            registry::server::run_server(&bind, &db_path).await
        }
    };

    if let Err(e) = result {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

/// Compare two semver version strings. Returns Ordering.
fn compare_versions(a: &str, b: &str) -> std::cmp::Ordering {
    let parse = |v: &str| -> (u64, u64, u64) {
        let parts: Vec<&str> = v.split('.').collect();
        let major = parts.first().and_then(|p| p.parse().ok()).unwrap_or(0);
        let minor = parts.get(1).and_then(|p| p.parse().ok()).unwrap_or(0);
        let patch = parts.get(2).and_then(|p| p.parse().ok()).unwrap_or(0);
        (major, minor, patch)
    };
    parse(a).cmp(&parse(b))
}

async fn run_update(target: Option<&str>) -> error::Result<()> {
    let path = config::Config::installed_servers_path()?;
    if !path.exists() {
        println!("No servers installed.");
        return Ok(());
    }

    let content = std::fs::read_to_string(&path)?;

    // Detect pinned servers (backward-compatible)
    let pinned_set: std::collections::HashSet<String> = {
        if let Ok(pinned) = serde_json::from_str::<commands::pin::PinnedInstalledServers>(&content) {
            pinned.servers.iter()
                .filter(|s| s.pinned)
                .map(|s| format!("{}/{}", s.owner, s.name))
                .collect()
        } else {
            std::collections::HashSet::new()
        }
    };

    let installed: api::types::InstalledServers = serde_json::from_str(&content)?;

    if installed.servers.is_empty() {
        println!("No servers installed.");
        return Ok(());
    }

    // Filter to a specific server if requested
    let servers_to_check: Vec<&api::types::InstalledServer> = if let Some(target_ref) = target {
        let parts: Vec<&str> = target_ref.splitn(2, '/').collect();
        if parts.len() != 2 {
            return Err(error::McpRegError::Config(
                "Server reference must be in format 'owner/name'".into(),
            ));
        }
        let (t_owner, t_name) = (parts[0], parts[1]);
        let filtered: Vec<_> = installed
            .servers
            .iter()
            .filter(|s| s.owner == t_owner && s.name == t_name)
            .collect();
        if filtered.is_empty() {
            return Err(error::McpRegError::NotFound(format!(
                "{t_owner}/{t_name} is not installed"
            )));
        }
        filtered
    } else {
        installed.servers.iter().collect()
    };

    println!(
        "Checking {} server(s) for updates...\n",
        servers_to_check.len()
    );

    let cfg = config::Config::load()?;
    let client = api::client::RegistryClient::new(&cfg);
    let mut updated = 0;
    let mut skipped_pinned = 0;

    for server in &servers_to_check {
        let full_name = server.full_name();

        // Skip pinned servers unless explicitly targeted
        if target.is_none() && pinned_set.contains(&full_name) {
            println!("  📌 {} is pinned at v{} (skipping)", full_name, server.version);
            skipped_pinned += 1;
            continue;
        }

        match client.get_server(&server.owner, &server.name).await {
            Ok(entry) => {
                if compare_versions(&entry.version, &server.version) == std::cmp::Ordering::Greater
                {
                    println!(
                        "  ↑ {}: {} → {}",
                        full_name, server.version, entry.version
                    );
                    updated += 1;
                    if let Err(e) = commands::install::run(&full_name).await {
                        eprintln!("    Failed to update: {e}");
                    }
                } else {
                    println!(
                        "  ✓ {} is up to date (v{})",
                        full_name, server.version
                    );
                }
            }
            Err(e) => {
                eprintln!("  ✗ {}: {e}", full_name);
            }
        }
    }

    println!("\n{updated} server(s) updated.");
    if skipped_pinned > 0 {
        println!("{skipped_pinned} server(s) skipped (pinned).");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compare_versions_equal() {
        assert_eq!(compare_versions("1.0.0", "1.0.0"), std::cmp::Ordering::Equal);
    }

    #[test]
    fn test_compare_versions_greater() {
        assert_eq!(compare_versions("2.0.0", "1.0.0"), std::cmp::Ordering::Greater);
        assert_eq!(compare_versions("1.1.0", "1.0.0"), std::cmp::Ordering::Greater);
        assert_eq!(compare_versions("1.0.1", "1.0.0"), std::cmp::Ordering::Greater);
    }

    #[test]
    fn test_compare_versions_less() {
        assert_eq!(compare_versions("1.0.0", "2.0.0"), std::cmp::Ordering::Less);
    }

    #[test]
    fn test_compare_versions_date_style() {
        assert_eq!(compare_versions("2025.1.0", "2024.11.0"), std::cmp::Ordering::Greater);
    }

    #[test]
    fn test_compare_versions_partial() {
        assert_eq!(compare_versions("1.0", "1.0.0"), std::cmp::Ordering::Equal);
        assert_eq!(compare_versions("2", "1.9.9"), std::cmp::Ordering::Greater);
    }

    #[test]
    fn test_cli_parses_completions() {
        use clap::CommandFactory;
        // Verify the CLI definition is valid (catches typos in arg definitions)
        Cli::command().debug_assert();
    }
}
