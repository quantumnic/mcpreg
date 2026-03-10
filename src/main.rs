mod api;
mod commands;
mod config;
mod error;
mod registry;

use clap::{Parser, Subcommand};

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

#[derive(Subcommand)]
enum Commands {
    /// Search for MCP servers in the registry
    Search {
        /// Search query
        query: String,
    },
    /// Install an MCP server and add it to claude_desktop_config.json
    Install {
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
    List,
    /// Show detailed information about an MCP server
    Info {
        /// Server reference (owner/name)
        server: String,
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
    },
    /// Update all installed MCP servers
    Update,
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
        Commands::Search { query } => commands::search::run(&query).await,
        Commands::Install { server } => commands::install::run(&server).await,
        Commands::Publish { manifest } => commands::publish::run(manifest.as_deref()).await,
        Commands::List => commands::list::run(),
        Commands::Info { server } => commands::info::run(&server).await,
        Commands::Browse { page, per_page, category } => {
            commands::browse::run(page, per_page, category.as_deref())
        }
        Commands::Update => run_update().await,
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

    let cfg = config::Config::load()?;
    let client = api::client::RegistryClient::new(&cfg);
    let mut updated = 0;

    for server in &installed.servers {
        match client.get_server(&server.owner, &server.name).await {
            Ok(entry) => {
                if entry.version != server.version {
                    println!(
                        "  ↑ {}/{}: {} → {}",
                        server.owner, server.name, server.version, entry.version
                    );
                    updated += 1;
                    // Re-install to update
                    if let Err(e) = commands::install::run(&server.full_name()).await {
                        eprintln!("    Failed to update: {e}");
                    }
                } else {
                    println!("  ✓ {}/{} is up to date (v{})", server.owner, server.name, server.version);
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
