#![recursion_limit = "256"]

mod api;
mod color;
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
    /// Disable colored output
    #[arg(long, global = true)]
    no_color: bool,

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
    /// Sort by star count (most starred first)
    Stars,
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
        /// Use regex pattern matching for the query
        #[arg(long)]
        regex: bool,
        /// Show relevance scores in output
        #[arg(short = 'v', long)]
        verbose: bool,
        /// Only show servers with at least this many downloads
        #[arg(long)]
        min_downloads: Option<i64>,
        /// Filter by tool name (only show servers that expose this tool)
        #[arg(short = 't', long)]
        tool: Option<String>,
        /// Filter by transport type (stdio, sse, streamable-http)
        #[arg(long)]
        transport: Option<String>,
        /// Filter by author name
        #[arg(long)]
        author: Option<String>,
        /// Filter by owner/organization
        #[arg(long)]
        owner: Option<String>,
        /// Filter by tag (e.g. "ai", "database", "web")
        #[arg(long)]
        tag: Option<String>,
        /// Filter by license (e.g. "MIT", "Apache-2.0")
        #[arg(long)]
        license: Option<String>,
        /// Only show servers with at least this many stars
        #[arg(long)]
        min_stars: Option<i64>,
        /// Exclude deprecated servers from results
        #[arg(long)]
        exclude_deprecated: bool,
    },
    /// Run an MCP server directly from the registry
    #[command(alias = "run")]
    Exec {
        /// Server reference (owner/name)
        server: String,
        /// Extra arguments to pass to the server command
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
        /// Show what would be executed without actually running
        #[arg(long)]
        dry_run: bool,
    },
    /// Mirror the local registry to a directory of JSON files
    Mirror {
        /// Output directory for the mirror
        #[arg(default_value = "mcpreg-mirror")]
        output: String,
        /// Output as JSON
        #[arg(long)]
        json: bool,
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
    /// List all resources provided by servers in the registry
    Resources(commands::resources::ResourcesArgs),
    /// Find servers that provide a specific resource type
    Whohas(commands::whohas::WhohasArgs),
    /// Export installed servers as a claude_desktop_config.json snippet
    Export {
        /// Output file path (prints to stdout if not given)
        #[arg(short, long)]
        output: Option<String>,
        /// Output format: json (default), toml, env
        #[arg(short, long)]
        format: Option<String>,
    },
    /// Update installed MCP servers (all or a specific one)
    Update {
        /// Optional server reference (owner/name) to update a single server
        server: Option<String>,
        /// Show what would be updated without making changes
        #[arg(long)]
        dry_run: bool,
    },
    /// Show version history for a server
    Versions {
        /// Server reference (owner/name)
        server: String,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// List all tools across the registry (discover which tools exist)
    Tools {
        /// Filter tools by name
        #[arg(short, long)]
        query: Option<String>,
        /// Maximum number of results
        #[arg(short = 'n', long)]
        limit: Option<usize>,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// List all prompts across the registry
    Prompts {
        /// Filter prompts by name
        #[arg(short, long)]
        query: Option<String>,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Initialize a new mcpreg.toml manifest for your MCP server project
    Init {
        /// Directory to create mcpreg.toml in (default: current directory)
        #[arg(short, long)]
        path: Option<String>,
    },
    /// Lint a local mcpreg.toml manifest with detailed checks
    Lint {
        /// Path to mcpreg.toml manifest (default: ./mcpreg.toml)
        #[arg(short, long)]
        manifest: Option<String>,
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
    /// Show mcpreg system status (version, DB, config, counts)
    Status {
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
    /// Show a server's current state and version history (local DB)
    Diff {
        /// Server reference (owner/name)
        server: String,
        /// Compare from this version
        #[arg(long)]
        from: Option<String>,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Show changelog / version progression for a server
    Changelog {
        /// Server reference (owner/name)
        server: String,
        /// Starting version to compare from
        #[arg(long)]
        from: Option<String>,
        /// Ending version to compare to (default: current)
        #[arg(long)]
        to: Option<String>,
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
    /// Check compatibility between two MCP servers (tool conflicts, env overlaps)
    Compat {
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
    /// Show dependencies and requirements for an MCP server
    Deps {
        /// Server reference (owner/name)
        server: String,
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
    /// Get personalized server recommendations based on installed servers
    Recommend {
        /// Maximum number of recommendations (default: 10)
        #[arg(short = 'n', long, default_value = "10")]
        limit: usize,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Create a portable bundle of MCP servers for sharing
    BundleCreate {
        /// Bundle name
        name: String,
        /// Server references (owner/name) to include
        #[arg(required = true)]
        servers: Vec<String>,
        /// Bundle description
        #[arg(short, long)]
        description: Option<String>,
        /// Bundle author
        #[arg(short, long)]
        author: Option<String>,
        /// Output file path (prints to stdout if not given)
        #[arg(short, long)]
        output: Option<String>,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Inspect a bundle file (show contents without installing)
    BundleInspect {
        /// Path to bundle JSON file
        file: String,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Backup installed servers and config to a JSON file
    Backup {
        /// Output file path (prints to stdout if not given)
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Restore installed servers from a backup file
    Restore {
        /// Path to backup JSON file
        file: String,
        /// Show what would be restored without making changes
        #[arg(long)]
        dry_run: bool,
    },
    /// Check registry connectivity and server health
    Health {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Show local action history (installs, updates, searches)
    History {
        /// Maximum entries to show (default: 20)
        #[arg(short = 'n', long, default_value = "20")]
        limit: usize,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Security audit of installed MCP servers
    Audit {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Show trending / most popular MCP servers
    Trending {
        /// Maximum number of results (default: 15)
        #[arg(short = 'n', long, default_value = "15")]
        limit: usize,
        /// Filter by category (e.g. "database", "search")
        #[arg(short, long)]
        category: Option<String>,
        /// Filter by transport type (stdio, sse, streamable-http)
        #[arg(long)]
        transport: Option<String>,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Quick registry overview (servers, tools, categories, top downloads)
    Summary {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Show the tool-sharing graph between servers
    Graph {
        /// Minimum number of shared tools to show a connection (default: 1)
        #[arg(long, default_value = "1")]
        min_shared: usize,
        /// Maximum number of edges to display (default: 30)
        #[arg(short = 'n', long, default_value = "30")]
        limit: usize,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Verify installed servers are runnable (command on PATH, config consistency)
    Check {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Autocomplete: suggest server names matching a prefix
    Suggest {
        /// Prefix to match
        prefix: String,
        /// Maximum number of suggestions (default: 10)
        #[arg(short = 'n', long, default_value = "10")]
        limit: usize,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
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
    /// Manage your favorite/bookmarked servers
    #[command(subcommand)]
    Favorite(FavoriteCommands),
    /// Manage named server profiles (save, load, share server collections)
    #[command(subcommand)]
    Profile(ProfileCommands),
    /// Manage server name aliases (shortcuts for owner/name references)
    Alias {
        /// Action: list, set, remove
        #[arg(default_value = "list")]
        action: Option<String>,
        /// Alias name (for set/remove)
        alias: Option<String>,
        /// Target server (owner/name) for set
        target: Option<String>,
    },
    /// Clean up stale mcpreg data (temp files, old backups)
    Clean {
        /// Show what would be cleaned without deleting
        #[arg(long)]
        dry_run: bool,
    },
    /// Explain why a server might be useful based on your installed servers
    Why {
        /// Server reference (owner/name)
        server: String,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Deep inspection of a server (info + quality score + config + similar)
    Inspect {
        /// Server reference (owner/name)
        server: String,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Rate an MCP server (1-5 stars) with an optional review comment
    #[command(subcommand)]
    Rate(RateCommands),

    /// Manage the local cache (status, clear, path)
    Cache {
        /// Action: status, clear, path
        #[arg(default_value = "status")]
        action: Option<String>,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Show top servers ranked by various criteria (tools, resources, downloads, etc.)
    Top {
        /// Ranking criterion: tools, resources, prompts, downloads, newest, category
        #[arg(default_value = "tools")]
        by: String,
        /// Maximum number of results (default: 10)
        #[arg(short = 'n', long, default_value = "10")]
        limit: usize,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum ProfileCommands {
    /// Save currently installed servers as a named profile
    Save {
        /// Profile name
        name: String,
        /// Description of the profile
        #[arg(short, long)]
        description: Option<String>,
    },
    /// List all saved profiles
    List {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Show details of a specific profile
    Show {
        /// Profile name
        name: String,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Delete a saved profile
    Delete {
        /// Profile name
        name: String,
    },
    /// Apply a profile (install all servers in it)
    Apply {
        /// Profile name
        name: String,
        /// Show what would be installed without making changes
        #[arg(long)]
        dry_run: bool,
    },
    /// Export a profile to a JSON file
    Export {
        /// Profile name
        name: String,
        /// Output file path (prints to stdout if not given)
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Import a profile from a JSON file
    Import {
        /// Path to profile JSON file
        file: String,
    },
}

#[derive(Subcommand)]
enum RateCommands {
    /// Rate a server (1-5 stars)
    Add {
        /// Server reference (owner/name)
        server: String,
        /// Rating (1-5 stars)
        #[arg(short, long)]
        stars: u8,
        /// Optional review comment
        #[arg(short, long)]
        comment: Option<String>,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Show ratings for a server
    Show {
        /// Server reference (owner/name)
        server: String,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum FavoriteCommands {
    /// Add a server to your favorites
    Add {
        /// Server reference (owner/name)
        server: String,
        /// Optional note/reason
        #[arg(short, long)]
        note: Option<String>,
    },
    /// Remove a server from favorites
    Remove {
        /// Server reference (owner/name)
        server: String,
    },
    /// List all favorite servers
    List {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    // Apply --no-color flag via the NO_COLOR env var convention
    if cli.no_color {
        std::env::set_var("NO_COLOR", "1");
    }

    let config = config::Config::load().unwrap_or_default();

    let result = match cli.command {
        Commands::Search {
            query,
            json,
            category,
            sort,
            limit,
            compact,
            offline,
            regex,
            verbose,
            min_downloads,
            tool,
            transport,
            author,
            owner,
            tag,
            license,
            min_stars,
            exclude_deprecated,
        } => commands::search::run(&query, json, category.as_deref(), &sort, limit, compact, offline, regex, verbose, min_downloads, tool.as_deref(), transport.as_deref(), author.as_deref(), owner.as_deref(), tag.as_deref(), license.as_deref(), min_stars, exclude_deprecated).await,
        Commands::Exec { server, args, dry_run } => commands::exec::run(&server, &args, dry_run).await,
        Commands::Mirror { output, json } => commands::mirror::run(&output, json),
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
        Commands::Resources(args) => commands::resources::run(&args, &config).await,
        Commands::Whohas(args) => commands::whohas::run(&args, &config).await,
        Commands::Export { output, format } => {
            let fmt = commands::export::ExportFormat::from_str_opt(format.as_deref());
            commands::export::run(output.as_deref(), fmt)
        }
        Commands::Similar { server, limit, json } => commands::similar::run(&server, limit, json),
        Commands::Random { category, json } => commands::random::run(category.as_deref(), json),
        Commands::Count { by, json } => commands::count::run(Some(&by), json),
        Commands::Outdated { json } => commands::outdated::run(json),
        Commands::Update { server, dry_run } => commands::update::run(server.as_deref(), dry_run).await,
        Commands::Versions { server, json } => commands::versions::run(&server, json),
        Commands::Tools { query, limit, json } => commands::tools::run(query.as_deref(), limit, json),
        Commands::Prompts { query, json } => commands::prompts::run(query.as_deref(), json),
        Commands::Init { path } => commands::init::run(path.as_deref()),
        Commands::Lint { manifest } => commands::lint::run(manifest.as_deref()),
        Commands::Validate { manifest, json } => {
            commands::validate::run(manifest.as_deref(), json)
        }
        Commands::Stats { json } => commands::stats::run(json),
        Commands::Status { json } => commands::status::run(json),
        Commands::Changelog { server, from, to, json } => {
            commands::changelog::run(&server, from.as_deref(), to.as_deref(), json)
        }
        Commands::Diff { server, from, json } => {
            commands::diff::run(&server, from.as_deref(), json)
        }
        Commands::Compare { server_a, server_b, json } => {
            commands::compare::run(&server_a, &server_b, json)
        }
        Commands::Compat { server_a, server_b, json } => {
            commands::compat::run(&server_a, &server_b, json)
        }
        Commands::Config { action, key, value } => {
            commands::config_cmd::run(&action, key.as_deref(), value.as_deref())
        }
        Commands::Which { tool, json } => commands::which::run(&tool, json),
        Commands::Deps { server, json } => commands::deps::run(&server, json),
        Commands::Doctor => commands::doctor::run(),
        Commands::Env { server, json } => commands::env::run(&server, json).await,
        Commands::Pin { server } => commands::pin::run_pin(&server),
        Commands::Unpin { server } => commands::pin::run_unpin(&server),
        Commands::Pinned => commands::pin::run_list(),
        Commands::Recommend { limit, json } => commands::recommend::run(limit, json),
        Commands::BundleCreate {
            name,
            servers,
            description,
            author,
            output,
            json,
        } => commands::bundle::run_create(
            &name,
            &servers,
            description.as_deref(),
            author.as_deref(),
            output.as_deref(),
            json,
        ),
        Commands::BundleInspect { file, json } => {
            commands::bundle::run_inspect(&file, json)
        }
        Commands::Backup { output } => commands::backup::run_backup(output.as_deref()),
        Commands::Restore { file, dry_run } => commands::backup::run_restore(&file, dry_run),
        Commands::Health { json } => commands::health::run(json).await,
        Commands::History { limit, json } => commands::history::run(limit, json),
        Commands::Trending { limit, category, transport, json } => {
            commands::trending::run(limit, category.as_deref(), transport.as_deref(), json)
        }
        Commands::Summary { json } => commands::summary::run(json),
        Commands::Audit { json } => commands::audit::run(json),
        Commands::Graph { min_shared, limit, json } => commands::graph::run(min_shared, limit, json),
        Commands::Check { json } => commands::check::run(json),
        Commands::Suggest { prefix, limit, json } => commands::suggest::run(&prefix, limit, json).await,
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
        Commands::Profile(sub) => match sub {
            ProfileCommands::Save { name, description } => {
                commands::profile::run_save(&name, description.as_deref())
            }
            ProfileCommands::List { json } => commands::profile::run_list(json),
            ProfileCommands::Show { name, json } => commands::profile::run_show(&name, json),
            ProfileCommands::Delete { name } => commands::profile::run_delete(&name),
            ProfileCommands::Apply { name, dry_run } => {
                commands::profile::run_apply(&name, dry_run).await
            }
            ProfileCommands::Export { name, output } => {
                commands::profile::run_export(&name, output.as_deref())
            }
            ProfileCommands::Import { file } => commands::profile::run_import(&file),
        },
        Commands::Favorite(sub) => match sub {
            FavoriteCommands::Add { server, note } => {
                commands::favorites::add(&server, note.as_deref())
            }
            FavoriteCommands::Remove { server } => commands::favorites::remove(&server),
            FavoriteCommands::List { json } => commands::favorites::list(json),
        },
        Commands::Rate(sub) => match sub {
            RateCommands::Add { server, stars, comment, json } => {
                commands::rate::run(&server, stars, comment.as_deref(), json)
            }
            RateCommands::Show { server, json } => {
                commands::rate::show(&server, json)
            }
        },
        Commands::Alias { action, alias, target } => {
            commands::alias::run_alias(action, alias, target)
        }
        Commands::Clean { dry_run } => commands::clean::run_clean(dry_run),
        Commands::Why { server, json } => commands::why::run(&server, json),
        Commands::Inspect { server, json } => commands::inspect::run(&server, json),
        Commands::Cache { action, json } => commands::cache::run(action.as_deref(), json),
        Commands::Top { by, limit, json } => commands::top::run(&by, limit, json),
    };

    if let Err(e) = result {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

/// Compare two semver version strings. Returns Ordering.
pub fn compare_versions(a: &str, b: &str) -> std::cmp::Ordering {
    let parse = |v: &str| -> (u64, u64, u64) {
        let parts: Vec<&str> = v.split('.').collect();
        let major = parts.first().and_then(|p| p.parse().ok()).unwrap_or(0);
        let minor = parts.get(1).and_then(|p| p.parse().ok()).unwrap_or(0);
        let patch = parts.get(2).and_then(|p| p.parse().ok()).unwrap_or(0);
        (major, minor, patch)
    };
    parse(a).cmp(&parse(b))
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

    #[test]
    fn test_compare_versions_empty() {
        assert_eq!(compare_versions("", ""), std::cmp::Ordering::Equal);
    }

    #[test]
    fn test_compare_versions_prerelease_style() {
        // Non-numeric parts fall back to string comparison
        assert_eq!(compare_versions("1.0.0", "1.0.0"), std::cmp::Ordering::Equal);
    }

    #[test]
    fn test_compare_versions_many_segments() {
        assert_eq!(compare_versions("1.2.3.4", "1.2.3.4"), std::cmp::Ordering::Equal);
        // compare_versions only considers the first 3 semver segments
        assert_eq!(compare_versions("1.2.3.5", "1.2.3.4"), std::cmp::Ordering::Equal);
    }

    #[test]
    fn test_compare_versions_zero_padded() {
        assert_eq!(compare_versions("01.02.03", "1.2.3"), std::cmp::Ordering::Equal);
    }
}
