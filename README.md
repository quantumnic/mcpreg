# mcpreg

**Open source registry and marketplace for MCP (Model Context Protocol) servers.**

Like npm or crates.io, but for MCP servers. Search, install, publish, and self-host your own registry.

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

## Features

- 🔍 **Search** — Find MCP servers by name, description, tools, or capabilities (with `--offline` mode)
- 📦 **Install** — One command to install and configure MCP servers in `claude_desktop_config.json`
- 📤 **Publish** — Share your MCP servers with the community (with version & transport validation)
- 📋 **List** — See all installed MCP servers at a glance
- ℹ️ **Info** — View detailed server metadata, tools, resources, and prompts
- 🔄 **Update** — Keep all installed servers up to date
- 📤 **Export** — Export installed servers config as a portable JSON snippet
- 🏠 **Self-host** — Run your own private registry with the built-in axum server
- 🎯 **Prompts** — First-class support for MCP prompts in manifests, DB, and API

## Installation

### From source

```bash
git clone https://github.com/quantumnic/mcpreg
cd mcpreg
cargo install --path .
```

### From crates.io (coming soon)

```bash
cargo install mcpreg
```

## Quick Start

### Search for MCP servers

```bash
mcpreg search "filesystem"
mcpreg search "sqlite"
mcpreg search "web scraper"
mcpreg search "database" --category database --compact
mcpreg search "tools" --offline --sort name --limit 10
```

### Install a server

Installs the server and automatically adds it to your `claude_desktop_config.json`:

```bash
mcpreg install modelcontextprotocol/filesystem
```

### List installed servers

```bash
mcpreg list
```

### View server details

```bash
mcpreg info modelcontextprotocol/filesystem
```

### Publish your own server

Create an `mcpreg.toml` in your project root:

```toml
[package]
name = "my-mcp-server"
version = "1.0.0"
description = "Does something useful"
author = "yourusername"
license = "MIT"
repository = "https://github.com/you/your-server"

[server]
command = "node"
args = ["dist/index.js"]
transport = "stdio"  # or "http"

[server.env]
API_KEY = "placeholder"

[capabilities]
tools = ["read_file", "write_file"]
resources = ["file://"]
```

Then publish:

```bash
mcpreg publish
```

### Update all servers

```bash
mcpreg update
```

## Configuration

mcpreg stores its configuration in `~/.mcpreg/config.toml`:

```toml
registry_url = "https://registry.mcpreg.dev"
api_key = "your-api-key-for-publishing"
```

## Self-Hosting

Run your own private MCP server registry:

```bash
# Start the registry server
mcpreg serve --bind 0.0.0.0:3000

# With a custom database path
mcpreg serve --bind 0.0.0.0:3000 --db /path/to/registry.db
```

Then point your clients to it:

```toml
# ~/.mcpreg/config.toml
registry_url = "http://your-server:3000"
```

### REST API

| Endpoint | Method | Description |
|---|---|---|
| `GET /health` | GET | Health check |
| `GET /api/v1/search?q=query` | GET | Search servers |
| `GET /api/v1/servers` | GET | List all servers (paginated) |
| `GET /api/v1/servers/:owner/:name` | GET | Get server details |
| `POST /api/v1/publish` | POST | Publish a server (requires API key) |

### Pagination

```
GET /api/v1/servers?page=1&per_page=20
```

## Architecture

```
src/
  main.rs              # CLI entry point (clap)
  config.rs            # ~/.mcpreg/config.toml management
  error.rs             # Error types
  commands/
    search.rs          # Search registry
    install.rs         # Install + configure in claude_desktop_config.json
    publish.rs         # Publish to registry
    list.rs            # List installed servers
    info.rs            # Server details
  api/
    client.rs          # Registry HTTP client
    types.rs           # MCP manifest, server entries, API types
  registry/
    server.rs          # Axum HTTP server for self-hosting
    db.rs              # SQLite schema + queries
    routes.rs          # REST API route handlers
```

## Tech Stack

- **Rust** — Fast, safe, single binary
- **[clap](https://github.com/clap-rs/clap)** — CLI argument parsing
- **[axum](https://github.com/tokio-rs/axum)** — HTTP server for self-hosted registry
- **[reqwest](https://github.com/seanmonstar/reqwest)** — HTTP client for registry API
- **[rusqlite](https://github.com/rusqlite/rusqlite)** — SQLite for registry data
- **[serde](https://github.com/serde-rs/serde)** — Serialization/deserialization
- **[tokio](https://github.com/tokio-rs/tokio)** — Async runtime

## Development

```bash
# Run tests
cargo test

# Run clippy
cargo clippy

# Run the server locally
cargo run -- serve --bind 127.0.0.1:3000

# Search (against local or remote registry)
cargo run -- search "filesystem"
```

## Contributing

Contributions welcome! Please open an issue or PR.

1. Fork the repo
2. Create a feature branch (`git checkout -b feature/amazing`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to branch (`git push origin feature/amazing`)
5. Open a Pull Request

## License

MIT — see [LICENSE](LICENSE)
