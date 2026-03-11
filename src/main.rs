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
struct Cli {
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
    /// Update all installed MCP servers
    Update,
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
    /// Run diagnostics and check your setup
    Doctor,
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
        } => commands::search::run(&query, json, category.as_deref(), &sort, limit, compact, offline, min_downloads).await,
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
        Commands::Update => run_update().await,
        Commands::Init { path } => commands::init::run(path.as_deref()),
        Commands::Validate { manifest, json } => {
            commands::validate::run(manifest.as_deref(), json)
        }
        Commands::Stats { json } => commands::stats::run(json),
        Commands::Config { action, key, value } => {
            commands::config_cmd::run(&action, key.as_deref(), value.as_deref())
        }
        Commands::Doctor => commands::doctor::run(),
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

async fn run_update() -> error::Result<()> {
    let path = config::Config::installed_servers_path()?;
    if !path.exists() {
        println!("No servers installed.");
        return Ok(());
    }

    let content = std::fs::read_to_string(&path)?;
    let installed: api::types::InstalledServers = serde_json::from_str(&content)?;

    if installed.servers.is_empty() {
        println!("No servers installed.");
        return Ok(());
    }

    println!("Checking {} server(s) for updates...\n", installed.servers.len());

    let cfg = config::Config::load()?;
    let client = api::client::RegistryClient::new(&cfg);
    let mut updated = 0;

    for server in &installed.servers {
        match client.get_server(&server.owner, &server.name).await {
            Ok(entry) => {
                if compare_versions(&entry.version, &server.version) == std::cmp::Ordering::Greater {
                    println!(
                        "  ↑ {}/{}: {} → {}",
                        server.owner, server.name, server.version, entry.version
                    );
                    updated += 1;
                    if let Err(e) = commands::install::run(&server.full_name()).await {
                        eprintln!("    Failed to update: {e}");
                    }
                } else {
                    println!(
                        "  ✓ {}/{} is up to date (v{})",
                        server.owner, server.name, server.version
                    );
                }
            }
            Err(e) => {
                eprintln!("  ✗ {}/{}: {e}", server.owner, server.name);
            }
        }
    }

    println!("\n{updated} server(s) updated.");
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
        // Handles versions like "2024.11.0"
        assert_eq!(compare_versions("2025.1.0", "2024.11.0"), std::cmp::Ordering::Greater);
    }

    #[test]
    fn test_compare_versions_partial() {
        // Graceful with incomplete versions
        assert_eq!(compare_versions("1.0", "1.0.0"), std::cmp::Ordering::Equal);
        assert_eq!(compare_versions("2", "1.9.9"), std::cmp::Ordering::Greater);
    }
}
