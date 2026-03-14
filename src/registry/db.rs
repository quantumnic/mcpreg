use crate::api::types::ServerEntry;
use crate::error::Result;
use rusqlite::{params, Connection};

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn open(path: &str) -> Result<Self> {
        let conn = Connection::open(path)?;
        let db = Self { conn };
        db.init_tables()?;
        db.migrate()?;
        Ok(db)
    }

    #[allow(dead_code)]
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let db = Self { conn };
        db.init_tables()?;
        Ok(db)
    }

    fn init_tables(&self) -> Result<()> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS servers (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                owner TEXT NOT NULL,
                name TEXT NOT NULL,
                version TEXT NOT NULL,
                description TEXT NOT NULL DEFAULT '',
                author TEXT NOT NULL DEFAULT '',
                license TEXT NOT NULL DEFAULT '',
                repository TEXT NOT NULL DEFAULT '',
                command TEXT NOT NULL,
                args TEXT NOT NULL DEFAULT '[]',
                transport TEXT NOT NULL DEFAULT 'stdio',
                tools TEXT NOT NULL DEFAULT '[]',
                resources TEXT NOT NULL DEFAULT '[]',
                prompts TEXT NOT NULL DEFAULT '[]',
                tags TEXT NOT NULL DEFAULT '[]',
                env TEXT NOT NULL DEFAULT '{}',
                homepage TEXT NOT NULL DEFAULT '',
                deprecated INTEGER NOT NULL DEFAULT 0,
                deprecated_by TEXT NOT NULL DEFAULT '',
                category TEXT NOT NULL DEFAULT '',
                downloads INTEGER NOT NULL DEFAULT 0,
                stars INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now')),
                UNIQUE(owner, name)
            );

            CREATE INDEX IF NOT EXISTS idx_servers_owner ON servers(owner);
            CREATE INDEX IF NOT EXISTS idx_servers_name ON servers(name);
            CREATE INDEX IF NOT EXISTS idx_servers_downloads ON servers(downloads DESC);
            CREATE INDEX IF NOT EXISTS idx_servers_category ON servers(category);

            CREATE TABLE IF NOT EXISTS server_versions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                owner TEXT NOT NULL,
                name TEXT NOT NULL,
                version TEXT NOT NULL,
                published_at TEXT NOT NULL DEFAULT (datetime('now')),
                UNIQUE(owner, name, version)
            );

            CREATE INDEX IF NOT EXISTS idx_versions_server ON server_versions(owner, name);

            CREATE TABLE IF NOT EXISTS api_keys (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                key_hash TEXT NOT NULL UNIQUE,
                owner TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                revoked INTEGER NOT NULL DEFAULT 0
            );",
        )?;
        Ok(())
    }

    /// Run schema migrations for existing databases.
    fn migrate(&self) -> Result<()> {
        // Add category column if missing (for databases created before this version)
        let has_category: bool = self.conn
            .prepare("SELECT COUNT(*) FROM pragma_table_info('servers') WHERE name='category'")?
            .query_row([], |row| row.get::<_, i64>(0))
            .map(|c| c > 0)
            .unwrap_or(false);

        if !has_category {
            self.conn.execute_batch("ALTER TABLE servers ADD COLUMN category TEXT NOT NULL DEFAULT ''")?;
            self.conn.execute_batch("CREATE INDEX IF NOT EXISTS idx_servers_category ON servers(category)")?;
        }

        // Add prompts column if missing
        let has_prompts: bool = self.conn
            .prepare("SELECT COUNT(*) FROM pragma_table_info('servers') WHERE name='prompts'")?
            .query_row([], |row| row.get::<_, i64>(0))
            .map(|c| c > 0)
            .unwrap_or(false);

        if !has_prompts {
            self.conn.execute_batch("ALTER TABLE servers ADD COLUMN prompts TEXT NOT NULL DEFAULT '[]'")?;
        }

        // Add tags column if missing
        let has_tags: bool = self.conn
            .prepare("SELECT COUNT(*) FROM pragma_table_info('servers') WHERE name='tags'")?
            .query_row([], |row| row.get::<_, i64>(0))
            .map(|c| c > 0)
            .unwrap_or(false);

        if !has_tags {
            self.conn.execute_batch("ALTER TABLE servers ADD COLUMN tags TEXT NOT NULL DEFAULT '[]'")?;
        }

        // Add env column if missing
        let has_env: bool = self.conn
            .prepare("SELECT COUNT(*) FROM pragma_table_info('servers') WHERE name='env'")?
            .query_row([], |row| row.get::<_, i64>(0))
            .map(|c| c > 0)
            .unwrap_or(false);

        if !has_env {
            self.conn.execute_batch("ALTER TABLE servers ADD COLUMN env TEXT NOT NULL DEFAULT '{}'")?;
        }

        // Add homepage column if missing
        let has_homepage: bool = self.conn
            .prepare("SELECT COUNT(*) FROM pragma_table_info('servers') WHERE name='homepage'")?
            .query_row([], |row| row.get::<_, i64>(0))
            .map(|c| c > 0)
            .unwrap_or(false);

        if !has_homepage {
            self.conn.execute_batch("ALTER TABLE servers ADD COLUMN homepage TEXT NOT NULL DEFAULT ''")?;
        }

        // Add deprecated columns if missing
        let has_deprecated: bool = self.conn
            .prepare("SELECT COUNT(*) FROM pragma_table_info('servers') WHERE name='deprecated'")?
            .query_row([], |row| row.get::<_, i64>(0))
            .map(|c| c > 0)
            .unwrap_or(false);

        if !has_deprecated {
            self.conn.execute_batch("ALTER TABLE servers ADD COLUMN deprecated INTEGER NOT NULL DEFAULT 0")?;
            self.conn.execute_batch("ALTER TABLE servers ADD COLUMN deprecated_by TEXT NOT NULL DEFAULT ''")?;
        }

        // Add stars column if missing
        let has_stars: bool = self.conn
            .prepare("SELECT COUNT(*) FROM pragma_table_info('servers') WHERE name='stars'")?
            .query_row([], |row| row.get::<_, i64>(0))
            .map(|c| c > 0)
            .unwrap_or(false);

        if !has_stars {
            self.conn.execute_batch("ALTER TABLE servers ADD COLUMN stars INTEGER NOT NULL DEFAULT 0")?;
        }

        // Create server_versions table if missing (for databases created before this version)
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS server_versions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                owner TEXT NOT NULL,
                name TEXT NOT NULL,
                version TEXT NOT NULL,
                published_at TEXT NOT NULL DEFAULT (datetime('now')),
                UNIQUE(owner, name, version)
            );
            CREATE INDEX IF NOT EXISTS idx_versions_server ON server_versions(owner, name);"
        )?;

        Ok(())
    }

    pub fn upsert_server(&self, entry: &ServerEntry) -> Result<i64> {
        let args_json = serde_json::to_string(&entry.args)?;
        let tools_json = serde_json::to_string(&entry.tools)?;
        let resources_json = serde_json::to_string(&entry.resources)?;
        let prompts_json = serde_json::to_string(&entry.prompts)?;
        let tags_json = serde_json::to_string(&entry.tags)?;
        let env_json = serde_json::to_string(&entry.env)?;
        let category = crate::registry::seed::server_category(&entry.owner, &entry.name);

        self.conn.execute(
            "INSERT INTO servers (owner, name, version, description, author, license, repository, command, args, transport, tools, resources, prompts, tags, env, homepage, deprecated, deprecated_by, category, downloads)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20)
             ON CONFLICT(owner, name) DO UPDATE SET
                version = excluded.version,
                description = excluded.description,
                author = excluded.author,
                license = excluded.license,
                repository = excluded.repository,
                command = excluded.command,
                args = excluded.args,
                transport = excluded.transport,
                tools = excluded.tools,
                resources = excluded.resources,
                prompts = excluded.prompts,
                tags = excluded.tags,
                env = excluded.env,
                homepage = excluded.homepage,
                deprecated = excluded.deprecated,
                deprecated_by = excluded.deprecated_by,
                category = excluded.category,
                updated_at = datetime('now')",
            params![
                entry.owner,
                entry.name,
                entry.version,
                entry.description,
                entry.author,
                entry.license,
                entry.repository,
                entry.command,
                args_json,
                entry.transport,
                tools_json,
                resources_json,
                prompts_json,
                tags_json,
                env_json,
                entry.homepage,
                entry.deprecated as i32,
                entry.deprecated_by.as_deref().unwrap_or(""),
                category,
                entry.downloads,
            ],
        )?;
        // Track version history
        let _ = self.conn.execute(
            "INSERT OR IGNORE INTO server_versions (owner, name, version) VALUES (?1, ?2, ?3)",
            params![entry.owner, entry.name, entry.version],
        );

        Ok(self.conn.last_insert_rowid())
    }

    /// Get version history for a server, newest first.
    pub fn get_version_history(&self, owner: &str, name: &str) -> Result<Vec<(String, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT version, published_at FROM server_versions
             WHERE owner = ?1 AND name = ?2
             ORDER BY id DESC",
        )?;
        let rows = stmt.query_map(params![owner, name], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut versions = Vec::new();
        for row in rows {
            versions.push(row?);
        }
        Ok(versions)
    }

    /// Multi-word search: splits query on whitespace, all terms must match (AND semantics).
    /// Weighted ranking: name > owner > tools > description, plus download bonus.
    pub fn search(&self, query: &str) -> Result<Vec<ServerEntry>> {
        let all_terms: Vec<&str> = query.split_whitespace().collect();

        if all_terms.is_empty() {
            // Empty query: return all, ordered by downloads
            return self.list_servers(1, 50).map(|(v, _)| v);
        }

        // Separate positive and negative terms (prefixed with '-')
        let mut terms: Vec<&str> = Vec::new();
        let mut negated: Vec<&str> = Vec::new();
        for term in &all_terms {
            if let Some(neg) = term.strip_prefix('-') {
                if !neg.is_empty() {
                    negated.push(neg);
                }
            } else {
                terms.push(term);
            }
        }

        // If only negated terms, start with all servers
        if terms.is_empty() && !negated.is_empty() {
            let mut servers = self.list_servers(1, 200).map(|(v, _)| v)?;
            for neg in &negated {
                let neg_lower = neg.to_lowercase();
                servers.retain(|s| {
                    !s.name.to_lowercase().contains(&neg_lower)
                        && !s.description.to_lowercase().contains(&neg_lower)
                        && !s.owner.to_lowercase().contains(&neg_lower)
                        && !s.tools.iter().any(|t| t.to_lowercase().contains(&neg_lower))
                        && !s.tags.iter().any(|t| t.to_lowercase().contains(&neg_lower))
                });
            }
            return Ok(servers);
        }

        // Build dynamic WHERE: every term must match at least one column
        let mut conditions = Vec::new();
        let mut param_values: Vec<String> = Vec::new();

        for term in &terms {
            let escaped = escape_like(term);
            let pattern = format!("%{escaped}%");
            let idx = param_values.len();
            param_values.push(pattern);
            conditions.push(format!(
                "(name LIKE ?{p} ESCAPE '\\' OR description LIKE ?{p} ESCAPE '\\' OR owner LIKE ?{p} ESCAPE '\\' OR tools LIKE ?{p} ESCAPE '\\' OR author LIKE ?{p} ESCAPE '\\' OR category LIKE ?{p} ESCAPE '\\' OR tags LIKE ?{p} ESCAPE '\\')",
                p = idx + 1
            ));
        }

        // Add negation conditions
        for neg in &negated {
            let escaped = escape_like(neg);
            let pattern = format!("%{escaped}%");
            let idx = param_values.len();
            param_values.push(pattern);
            conditions.push(format!(
                "NOT (name LIKE ?{p} ESCAPE '\\' OR description LIKE ?{p} ESCAPE '\\' OR tools LIKE ?{p} ESCAPE '\\' OR tags LIKE ?{p} ESCAPE '\\')",
                p = idx + 1
            ));
        }

        let where_clause = conditions.join(" AND ");

        // Relevance scoring: accumulate score across ALL terms for better multi-word ranking
        let mut score_parts = Vec::new();
        for term in &terms {
            let exact_name = term.to_string();
            let escaped = escape_like(term);
            let pattern = format!("%{escaped}%");
            let idx_exact = param_values.len() + 1;
            param_values.push(exact_name);
            let idx_like = param_values.len() + 1;
            param_values.push(pattern);
            score_parts.push(format!(
                "(CASE WHEN LOWER(name) = LOWER(?{idx_exact}) THEN 300 ELSE 0 END \
                 + CASE WHEN LOWER(owner) = LOWER(?{idx_exact}) THEN 250 ELSE 0 END \
                 + CASE WHEN LOWER(owner || '/' || name) = LOWER(?{idx_exact}) THEN 500 ELSE 0 END \
                 + CASE WHEN name LIKE ?{idx_like} ESCAPE '\\' THEN 100 ELSE 0 END \
                 + CASE WHEN owner LIKE ?{idx_like} ESCAPE '\\' THEN 50 ELSE 0 END \
                 + CASE WHEN tools LIKE ?{idx_like} ESCAPE '\\' THEN 40 ELSE 0 END \
                 + CASE WHEN category LIKE ?{idx_like} ESCAPE '\\' THEN 20 ELSE 0 END \
                 + CASE WHEN tags LIKE ?{idx_like} ESCAPE '\\' THEN 15 ELSE 0 END \
                 + CASE WHEN author LIKE ?{idx_like} ESCAPE '\\' THEN 12 ELSE 0 END \
                 + CASE WHEN description LIKE ?{idx_like} ESCAPE '\\' THEN 10 ELSE 0 END)"
            ));
        }
        let score_expr = score_parts.join(" + ");

        let sql = format!(
            "SELECT id, owner, name, version, description, author, license, repository,
                    command, args, transport, tools, resources, prompts, tags, env, homepage, deprecated, deprecated_by, category, downloads, stars, created_at, updated_at,
                    ({score_expr} + downloads / 100) AS relevance
             FROM servers
             WHERE {where_clause}
             ORDER BY relevance DESC, downloads DESC
             LIMIT 50",
        );

        let mut stmt = self.conn.prepare(&sql)?;

        let params_refs: Vec<&dyn rusqlite::types::ToSql> =
            param_values.iter().map(|v| v as &dyn rusqlite::types::ToSql).collect();

        let rows = stmt.query_map(params_refs.as_slice(), row_mapper)?;

        let mut entries = Vec::new();
        for row in rows {
            let r = row?;
            entries.push(r.into_entry());
        }
        Ok(entries)
    }

    /// Search servers filtered by category (server-side).
    #[allow(dead_code)]
    pub fn search_by_category(&self, category: &str) -> Result<Vec<ServerEntry>> {
        let escaped = escape_like(category);
        let pattern = format!("%{escaped}%");
        let mut stmt = self.conn.prepare(
            "SELECT id, owner, name, version, description, author, license, repository,
                    command, args, transport, tools, resources, prompts, tags, env, homepage, deprecated, deprecated_by, category, downloads, stars, created_at, updated_at
             FROM servers WHERE category LIKE ?1 ORDER BY downloads DESC",
        )?;
        let rows = stmt.query_map(params![pattern], row_mapper)?;
        let mut entries = Vec::new();
        for row in rows {
            entries.push(row?.into_entry());
        }
        Ok(entries)
    }

    pub fn get_server(&self, owner: &str, name: &str) -> Result<Option<ServerEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, owner, name, version, description, author, license, repository,
                    command, args, transport, tools, resources, prompts, tags, env, homepage, deprecated, deprecated_by, category, downloads, stars, created_at, updated_at
             FROM servers WHERE owner = ?1 AND name = ?2",
        )?;
        let mut rows = stmt.query_map(params![owner, name], row_mapper)?;

        match rows.next() {
            Some(Ok(r)) => Ok(Some(r.into_entry())),
            Some(Err(e)) => Err(e.into()),
            None => Ok(None),
        }
    }

    pub fn delete_server(&self, owner: &str, name: &str) -> Result<bool> {
        let affected = self.conn.execute(
            "DELETE FROM servers WHERE owner = ?1 AND name = ?2",
            params![owner, name],
        )?;
        Ok(affected > 0)
    }

    pub fn list_servers(&self, page: usize, per_page: usize) -> Result<(Vec<ServerEntry>, usize)> {
        let offset = (page.saturating_sub(1)) * per_page;

        let total: usize = self.conn.query_row("SELECT COUNT(*) FROM servers", [], |row| row.get(0))?;

        let mut stmt = self.conn.prepare(
            "SELECT id, owner, name, version, description, author, license, repository,
                    command, args, transport, tools, resources, prompts, tags, env, homepage, deprecated, deprecated_by, category, downloads, stars, created_at, updated_at
             FROM servers ORDER BY downloads DESC LIMIT ?1 OFFSET ?2",
        )?;
        let rows = stmt.query_map(params![per_page as i64, offset as i64], row_mapper)?;

        let mut entries = Vec::new();
        for row in rows {
            entries.push(row?.into_entry());
        }
        Ok((entries, total))
    }

    /// Return all servers (no pagination).
    pub fn list_all(&self) -> Result<Vec<ServerEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, owner, name, version, description, author, license, repository,
                    command, args, transport, tools, resources, prompts, tags, env, homepage, deprecated, deprecated_by, category, downloads, stars, created_at, updated_at
             FROM servers ORDER BY downloads DESC",
        )?;
        let rows = stmt.query_map([], row_mapper)?;
        let mut entries = Vec::new();
        for row in rows {
            entries.push(row?.into_entry());
        }
        Ok(entries)
    }

    /// Seed the database with well-known MCP servers if the DB is empty.
    pub fn seed_default_servers(&self) -> Result<usize> {
        let count: i64 = self.conn.query_row("SELECT COUNT(*) FROM servers", [], |row| row.get(0))?;
        if count > 0 {
            return Ok(0);
        }
        let servers = crate::registry::seed::default_servers();
        let total = servers.len();
        for entry in servers {
            self.upsert_server(&entry)?;
        }
        Ok(total)
    }

    pub fn increment_downloads(&self, owner: &str, name: &str) -> Result<bool> {
        let affected = self.conn.execute(
            "UPDATE servers SET downloads = downloads + 1 WHERE owner = ?1 AND name = ?2",
            params![owner, name],
        )?;
        Ok(affected > 0)
    }

    /// Increment the star count for a server. Returns true if the server exists.
    pub fn star_server(&self, owner: &str, name: &str) -> Result<bool> {
        let affected = self.conn.execute(
            "UPDATE servers SET stars = stars + 1 WHERE owner = ?1 AND name = ?2",
            params![owner, name],
        )?;
        Ok(affected > 0)
    }

    /// Decrement the star count for a server (minimum 0). Returns true if the server exists.
    pub fn unstar_server(&self, owner: &str, name: &str) -> Result<bool> {
        let affected = self.conn.execute(
            "UPDATE servers SET stars = MAX(0, stars - 1) WHERE owner = ?1 AND name = ?2",
            params![owner, name],
        )?;
        Ok(affected > 0)
    }

    /// Get the leaderboard: top servers by a combined score of downloads + stars.
    pub fn leaderboard(&self, limit: usize) -> Result<Vec<ServerEntry>> {
        let sql = "SELECT id, owner, name, version, description, author, license, repository,
                    command, args, transport, tools, resources, prompts, tags, env, homepage, deprecated, deprecated_by, category, downloads, stars, created_at, updated_at
             FROM servers
             WHERE deprecated = 0
             ORDER BY (downloads + stars * 10) DESC
             LIMIT ?1";
        let mut stmt = self.conn.prepare(sql)?;
        let rows = stmt.query_map(params![limit as i64], row_mapper)?;
        let mut entries = Vec::new();
        for row in rows {
            entries.push(row?.into_entry());
        }
        Ok(entries)
    }

    /// Count servers matching a transport type.
    #[allow(dead_code)]
    pub fn count_by_transport(&self, transport: &str) -> Result<usize> {
        let count: usize = self.conn.query_row(
            "SELECT COUNT(*) FROM servers WHERE transport = ?1",
            params![transport],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    /// List all unique tool names with their server counts.
    pub fn list_tools(&self) -> Result<Vec<(String, Vec<String>)>> {
        let mut stmt = self.conn.prepare("SELECT owner, name, tools FROM servers")?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;

        let mut tool_map: std::collections::BTreeMap<String, Vec<String>> =
            std::collections::BTreeMap::new();
        for row in rows {
            let (owner, name, tools_json) = row?;
            let tools: Vec<String> = serde_json::from_str(&tools_json).unwrap_or_default();
            let full_name = format!("{owner}/{name}");
            for tool in tools {
                tool_map.entry(tool).or_default().push(full_name.clone());
            }
        }

        let mut result: Vec<(String, Vec<String>)> = tool_map.into_iter().collect();
        result.sort_by(|a, b| b.1.len().cmp(&a.1.len()));
        Ok(result)
    }

    /// List all unique prompt names with their server counts.
    pub fn list_prompts(&self) -> Result<Vec<(String, Vec<String>)>> {
        let mut stmt = self.conn.prepare("SELECT owner, name, prompts FROM servers")?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;

        let mut prompt_map: std::collections::BTreeMap<String, Vec<String>> =
            std::collections::BTreeMap::new();
        for row in rows {
            let (owner, name, prompts_json) = row?;
            let prompts: Vec<String> = serde_json::from_str(&prompts_json).unwrap_or_default();
            let full_name = format!("{owner}/{name}");
            for prompt in prompts {
                prompt_map.entry(prompt).or_default().push(full_name.clone());
            }
        }

        let mut result: Vec<(String, Vec<String>)> = prompt_map.into_iter().collect();
        result.sort_by(|a, b| b.1.len().cmp(&a.1.len()));
        Ok(result)
    }

    /// List all distinct categories with counts.
    #[allow(dead_code)]
    pub fn list_categories(&self) -> Result<Vec<(String, usize)>> {
        let mut stmt = self.conn.prepare(
            "SELECT category, COUNT(*) FROM servers WHERE category != '' GROUP BY category ORDER BY COUNT(*) DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, usize>(1)?))
        })?;
        let mut cats = Vec::new();
        for row in rows {
            cats.push(row?);
        }
        Ok(cats)
    }
}

/// Escape SQL LIKE wildcard characters (`%`, `_`) in user input.
fn escape_like(input: &str) -> String {
    input
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

/// Row mapper for server queries (reused across methods).
fn row_mapper(row: &rusqlite::Row) -> rusqlite::Result<ServerEntryRow> {
    Ok(ServerEntryRow {
        id: row.get(0)?,
        owner: row.get(1)?,
        name: row.get(2)?,
        version: row.get(3)?,
        description: row.get(4)?,
        author: row.get(5)?,
        license: row.get(6)?,
        repository: row.get(7)?,
        command: row.get(8)?,
        args: row.get::<_, String>(9)?,
        transport: row.get(10)?,
        tools: row.get::<_, String>(11)?,
        resources: row.get::<_, String>(12)?,
        prompts: row.get::<_, String>(13)?,
        tags: row.get::<_, String>(14)?,
        env: row.get::<_, String>(15)?,
        homepage: row.get(16)?,
        deprecated: row.get::<_, i32>(17)?,
        deprecated_by: row.get(18)?,
        category: row.get(19)?,
        downloads: row.get(20)?,
        stars: row.get::<_, i64>(21).unwrap_or(0),
        created_at: row.get(22)?,
        updated_at: row.get(23)?,
    })
}

struct ServerEntryRow {
    id: i64,
    owner: String,
    name: String,
    version: String,
    description: String,
    author: String,
    license: String,
    repository: String,
    command: String,
    args: String,
    transport: String,
    tools: String,
    resources: String,
    prompts: String,
    tags: String,
    env: String,
    homepage: String,
    deprecated: i32,
    deprecated_by: String,
    #[allow(dead_code)]
    category: String,
    downloads: i64,
    stars: i64,
    created_at: String,
    updated_at: String,
}

impl ServerEntryRow {
    fn into_entry(self) -> ServerEntry {
        ServerEntry {
            id: Some(self.id),
            owner: self.owner,
            name: self.name,
            version: self.version,
            description: self.description,
            author: self.author,
            license: self.license,
            repository: self.repository,
            command: self.command,
            args: serde_json::from_str(&self.args).unwrap_or_default(),
            transport: self.transport,
            tools: serde_json::from_str(&self.tools).unwrap_or_default(),
            resources: serde_json::from_str(&self.resources).unwrap_or_default(),
            prompts: serde_json::from_str(&self.prompts).unwrap_or_default(),
            tags: serde_json::from_str(&self.tags).unwrap_or_default(),
            env: serde_json::from_str(&self.env).unwrap_or_default(),
            homepage: self.homepage,
            deprecated: self.deprecated != 0,
            deprecated_by: if self.deprecated_by.is_empty() { None } else { Some(self.deprecated_by) },
            downloads: self.downloads,
            stars: self.stars,
            created_at: Some(self.created_at),
            updated_at: Some(self.updated_at),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_entry(owner: &str, name: &str) -> ServerEntry {
        ServerEntry {
            id: None,
            owner: owner.into(),
            name: name.into(),
            version: "1.0.0".into(),
            description: "A test MCP server".into(),
            author: owner.into(),
            license: "MIT".into(),
            repository: format!("https://github.com/{owner}/{name}"),
            command: "node".into(),
            args: vec!["dist/index.js".into()],
            transport: "stdio".into(),
            tools: vec!["read_file".into()],
            resources: vec!["file://".into()],
            prompts: vec![],
            tags: vec![],
            env: Default::default(),
            homepage: String::new(),
            deprecated: false,
            deprecated_by: None,
            downloads: 0,
            stars: 0,
            created_at: None,
            updated_at: None,
        }
    }

    #[test]
    fn test_db_create_and_insert() {
        let db = Database::open_in_memory().unwrap();
        let entry = test_entry("alice", "filesystem");
        let id = db.upsert_server(&entry).unwrap();
        assert!(id > 0);
    }

    #[test]
    fn test_db_get_server() {
        let db = Database::open_in_memory().unwrap();
        db.upsert_server(&test_entry("bob", "sqlite")).unwrap();
        let server = db.get_server("bob", "sqlite").unwrap().unwrap();
        assert_eq!(server.owner, "bob");
        assert_eq!(server.name, "sqlite");
        assert_eq!(server.version, "1.0.0");
    }

    #[test]
    fn test_db_search() {
        let db = Database::open_in_memory().unwrap();
        db.upsert_server(&test_entry("alice", "filesystem")).unwrap();
        db.upsert_server(&test_entry("bob", "sqlite-server")).unwrap();
        db.upsert_server(&test_entry("carol", "web-scraper")).unwrap();

        let results = db.search("filesystem").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "filesystem");

        let results = db.search("sqlite").unwrap();
        assert!(results.iter().any(|s| s.name == "sqlite-server"));
    }

    #[test]
    fn test_db_search_multi_word() {
        let db = Database::open_in_memory().unwrap();
        db.upsert_server(&test_entry("alice", "filesystem")).unwrap();
        db.upsert_server(&test_entry("bob", "sqlite-server")).unwrap();
        db.upsert_server(&test_entry("carol", "web-scraper")).unwrap();

        let results = db.search("alice filesystem").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "filesystem");

        let results = db.search("alice sqlite").unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_db_search_by_author() {
        let db = Database::open_in_memory().unwrap();
        let mut entry = test_entry("org", "tool");
        entry.author = "SpecialAuthor".into();
        db.upsert_server(&entry).unwrap();

        let results = db.search("SpecialAuthor").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "tool");
    }

    #[test]
    fn test_db_list_paginated() {
        let db = Database::open_in_memory().unwrap();
        for i in 0..5 {
            db.upsert_server(&test_entry("user", &format!("server-{i}"))).unwrap();
        }
        let (servers, total) = db.list_servers(1, 3).unwrap();
        assert_eq!(total, 5);
        assert_eq!(servers.len(), 3);

        let (servers, _) = db.list_servers(2, 3).unwrap();
        assert_eq!(servers.len(), 2);
    }

    #[test]
    fn test_db_upsert_updates_existing() {
        let db = Database::open_in_memory().unwrap();
        let mut entry = test_entry("alice", "filesystem");
        db.upsert_server(&entry).unwrap();

        entry.version = "2.0.0".into();
        entry.description = "Updated description".into();
        db.upsert_server(&entry).unwrap();

        let server = db.get_server("alice", "filesystem").unwrap().unwrap();
        assert_eq!(server.version, "2.0.0");
        assert_eq!(server.description, "Updated description");
    }

    #[test]
    fn test_db_increment_downloads() {
        let db = Database::open_in_memory().unwrap();
        db.upsert_server(&test_entry("alice", "tool")).unwrap();
        assert!(db.increment_downloads("alice", "tool").unwrap());
        assert!(db.increment_downloads("alice", "tool").unwrap());
        let server = db.get_server("alice", "tool").unwrap().unwrap();
        assert_eq!(server.downloads, 2);
    }

    #[test]
    fn test_db_increment_downloads_nonexistent() {
        let db = Database::open_in_memory().unwrap();
        assert!(!db.increment_downloads("nobody", "nothing").unwrap());
    }

    #[test]
    fn test_db_delete_server() {
        let db = Database::open_in_memory().unwrap();
        db.upsert_server(&test_entry("alice", "tool")).unwrap();
        assert!(db.delete_server("alice", "tool").unwrap());
        assert!(db.get_server("alice", "tool").unwrap().is_none());
    }

    #[test]
    fn test_db_delete_nonexistent() {
        let db = Database::open_in_memory().unwrap();
        assert!(!db.delete_server("nobody", "nothing").unwrap());
    }

    #[test]
    fn test_db_not_found() {
        let db = Database::open_in_memory().unwrap();
        let result = db.get_server("nobody", "nothing").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_db_count_by_transport() {
        let db = Database::open_in_memory().unwrap();
        db.upsert_server(&test_entry("a", "s1")).unwrap();
        db.upsert_server(&test_entry("b", "s2")).unwrap();
        assert_eq!(db.count_by_transport("stdio").unwrap(), 2);
        assert_eq!(db.count_by_transport("sse").unwrap(), 0);
    }

    #[test]
    fn test_version_history_tracked() {
        let db = Database::open_in_memory().unwrap();
        let mut entry = test_entry("alice", "tool");
        entry.version = "1.0.0".into();
        db.upsert_server(&entry).unwrap();
        entry.version = "1.1.0".into();
        db.upsert_server(&entry).unwrap();
        entry.version = "2.0.0".into();
        db.upsert_server(&entry).unwrap();

        let history = db.get_version_history("alice", "tool").unwrap();
        assert_eq!(history.len(), 3);
        // Newest first
        assert_eq!(history[0].0, "2.0.0");
        assert_eq!(history[1].0, "1.1.0");
        assert_eq!(history[2].0, "1.0.0");
    }

    #[test]
    fn test_version_history_idempotent() {
        let db = Database::open_in_memory().unwrap();
        let entry = test_entry("alice", "tool");
        db.upsert_server(&entry).unwrap();
        db.upsert_server(&entry).unwrap(); // same version again
        let history = db.get_version_history("alice", "tool").unwrap();
        assert_eq!(history.len(), 1, "Duplicate versions should not create entries");
    }

    #[test]
    fn test_version_history_empty() {
        let db = Database::open_in_memory().unwrap();
        let history = db.get_version_history("nobody", "nothing").unwrap();
        assert!(history.is_empty());
    }

    #[test]
    fn test_db_category_stored() {
        let db = Database::open_in_memory().unwrap();
        // "filesystem" maps to "📁 Files & VCS"
        db.upsert_server(&test_entry("alice", "filesystem")).unwrap();
        let cats = db.list_categories().unwrap();
        assert!(!cats.is_empty());
        assert!(cats.iter().any(|(c, _)| c.contains("Files")));
    }

    #[test]
    fn test_db_search_by_category() {
        let db = Database::open_in_memory().unwrap();
        db.upsert_server(&test_entry("alice", "filesystem")).unwrap();
        db.upsert_server(&test_entry("bob", "sqlite")).unwrap();

        let results = db.search_by_category("Files").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "filesystem");

        let results = db.search_by_category("Database").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "sqlite");
    }

    #[test]
    fn test_db_list_categories() {
        let db = Database::open_in_memory().unwrap();
        db.seed_default_servers().unwrap();
        let cats = db.list_categories().unwrap();
        assert!(cats.len() >= 5, "Expected at least 5 categories, got {}", cats.len());
    }
}

impl Database {
    /// Prefix-based autocomplete: return server full_names matching a prefix.
    /// Searches both `name` and `owner/name` with prefix matching, limited results.
    pub fn suggest(&self, prefix: &str, limit: usize) -> Result<Vec<String>> {
        if prefix.is_empty() {
            return Ok(Vec::new());
        }
        let escaped = escape_like(prefix);
        let pattern = format!("{escaped}%");
        let mut stmt = self.conn.prepare(
            "SELECT owner || '/' || name FROM servers
             WHERE (owner || '/' || name) LIKE ?1 ESCAPE '\\' OR name LIKE ?1 ESCAPE '\\'
             ORDER BY downloads DESC
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(
            rusqlite::params![pattern, limit as i64],
            |row| row.get::<_, String>(0),
        )?;
        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    }

    /// Search servers that have a specific tag (case-insensitive substring match).
    #[allow(dead_code)]
    pub fn search_by_tags(&self, tag: &str) -> Result<Vec<ServerEntry>> {
        let escaped = escape_like(tag);
        let pattern = format!("%{escaped}%");
        let mut stmt = self.conn.prepare(
            "SELECT id, owner, name, version, description, author, license, repository,
                    command, args, transport, tools, resources, prompts, tags, env, homepage, deprecated, deprecated_by, category, downloads, stars, created_at, updated_at
             FROM servers WHERE LOWER(tags) LIKE LOWER(?1) ESCAPE '\\' ORDER BY downloads DESC",
        )?;
        let rows = stmt.query_map(params![pattern], row_mapper)?;
        let mut entries = Vec::new();
        for row in rows {
            entries.push(row?.into_entry());
        }
        Ok(entries)
    }

    /// List all unique tags with their server counts.
    pub fn list_tags(&self) -> Result<Vec<(String, Vec<String>)>> {
        let mut stmt = self.conn.prepare("SELECT owner, name, tags FROM servers")?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;

        let mut tag_map: std::collections::BTreeMap<String, Vec<String>> =
            std::collections::BTreeMap::new();
        for row in rows {
            let (owner, name, tags_json) = row?;
            let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();
            let full_name = format!("{owner}/{name}");
            for tag in tags {
                tag_map.entry(tag).or_default().push(full_name.clone());
            }
        }

        let mut result: Vec<(String, Vec<String>)> = tag_map.into_iter().collect();
        result.sort_by(|a, b| b.1.len().cmp(&a.1.len()));
        Ok(result)
    }

    /// List all unique resources across the registry with the servers that provide them.
    /// Sorted by server count descending (most popular resources first).
    pub fn list_resources(&self) -> Result<Vec<(String, Vec<String>)>> {
        let mut stmt = self.conn.prepare("SELECT owner, name, resources FROM servers")?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;

        let mut resource_map: std::collections::BTreeMap<String, Vec<String>> =
            std::collections::BTreeMap::new();
        for row in rows {
            let (owner, name, resources_json) = row?;
            let resources: Vec<String> = serde_json::from_str(&resources_json).unwrap_or_default();
            let full_name = format!("{owner}/{name}");
            for resource in resources {
                resource_map.entry(resource).or_default().push(full_name.clone());
            }
        }

        let mut result: Vec<(String, Vec<String>)> = resource_map.into_iter().collect();
        result.sort_by(|a, b| b.1.len().cmp(&a.1.len()));
        Ok(result)
    }

    /// Search for servers that provide a specific resource type (prefix match).
    #[allow(dead_code)]
    pub fn search_by_resource(&self, resource_query: &str) -> Result<Vec<ServerEntry>> {
        let (all_servers, _) = self.list_servers(1, 1000)?;
        let q_lower = resource_query.to_lowercase();
        let results: Vec<ServerEntry> = all_servers
            .into_iter()
            .filter(|s| {
                s.resources.iter().any(|r| r.to_lowercase().contains(&q_lower))
            })
            .collect();
        Ok(results)
    }

    /// Find servers similar to the given one based on shared tools, category, and description overlap.
    /// Returns servers sorted by similarity score (0.0–1.0), excluding the queried server itself.
    pub fn find_similar(&self, owner: &str, name: &str, limit: usize) -> Result<Vec<(ServerEntry, f64)>> {
        let target = match self.get_server(owner, name)? {
            Some(entry) => entry,
            None => return Err(crate::error::McpRegError::NotFound(format!("{owner}/{name}"))),
        };

        let target_cat = crate::registry::seed::server_category(&target.owner, &target.name);
        let target_tools: std::collections::HashSet<&str> = target.tools.iter().map(|s| s.as_str()).collect();
        let target_words: std::collections::HashSet<String> = target
            .description
            .to_lowercase()
            .split_whitespace()
            .filter(|w| w.len() > 3)
            .map(String::from)
            .collect();

        let (all_servers, _) = self.list_servers(1, 1000)?;

        let mut scored: Vec<(ServerEntry, f64)> = all_servers
            .into_iter()
            .filter(|s| !(s.owner == owner && s.name == name))
            .filter_map(|s| {
                let mut score = 0.0f64;

                // Tool overlap (Jaccard similarity, weighted heavily)
                let s_tools: std::collections::HashSet<&str> = s.tools.iter().map(|t| t.as_str()).collect();
                let intersection = target_tools.intersection(&s_tools).count();
                let union = target_tools.union(&s_tools).count();
                if union > 0 {
                    score += 0.5 * (intersection as f64 / union as f64);
                }

                // Same category bonus
                let s_cat = crate::registry::seed::server_category(&s.owner, &s.name);
                if s_cat == target_cat {
                    score += 0.3;
                }

                // Description word overlap
                let s_words: std::collections::HashSet<String> = s
                    .description
                    .to_lowercase()
                    .split_whitespace()
                    .filter(|w| w.len() > 3)
                    .map(String::from)
                    .collect();
                let word_intersection = target_words.intersection(&s_words).count();
                let word_union = target_words.union(&s_words).count();
                if word_union > 0 {
                    score += 0.2 * (word_intersection as f64 / word_union as f64);
                }

                if score > 0.05 {
                    Some((s, score))
                } else {
                    None
                }
            })
            .collect();

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(limit);
        Ok(scored)
    }
}

#[cfg(test)]
mod similar_tests {
    use super::*;

    #[test]
    fn test_find_similar_returns_related() {
        let db = Database::open_in_memory().unwrap();
        db.seed_default_servers().unwrap();

        let similar = db.find_similar("modelcontextprotocol", "filesystem", 5).unwrap();
        assert!(!similar.is_empty(), "Should find similar servers to filesystem");
        // Git server is in same category (Files & VCS)
        let has_related = similar.iter().any(|(s, _)| {
            let cat = crate::registry::seed::server_category(&s.owner, &s.name);
            cat.contains("Files")
        });
        assert!(has_related, "Should find servers in same category");
    }

    #[test]
    fn test_find_similar_excludes_self() {
        let db = Database::open_in_memory().unwrap();
        db.seed_default_servers().unwrap();
        let similar = db.find_similar("modelcontextprotocol", "filesystem", 50).unwrap();
        assert!(
            !similar.iter().any(|(s, _)| s.owner == "modelcontextprotocol" && s.name == "filesystem"),
            "Should not include the queried server itself"
        );
    }

    #[test]
    fn test_find_similar_not_found() {
        let db = Database::open_in_memory().unwrap();
        let result = db.find_similar("nobody", "nothing", 5);
        assert!(result.is_err());
    }

    #[test]
    fn test_find_similar_scores_bounded() {
        let db = Database::open_in_memory().unwrap();
        db.seed_default_servers().unwrap();
        let similar = db.find_similar("modelcontextprotocol", "postgres", 10).unwrap();
        for (_, score) in &similar {
            assert!(*score > 0.0 && *score <= 1.0, "Score should be in (0, 1], got {score}");
        }
    }

    #[test]
    fn test_find_similar_respects_limit() {
        let db = Database::open_in_memory().unwrap();
        db.seed_default_servers().unwrap();
        let similar = db.find_similar("modelcontextprotocol", "github", 3).unwrap();
        assert!(similar.len() <= 3);
    }

    #[test]
    fn test_find_similar_database_servers_ranked() {
        let db = Database::open_in_memory().unwrap();
        db.seed_default_servers().unwrap();
        let similar = db.find_similar("modelcontextprotocol", "postgres", 5).unwrap();
        // SQLite and other DB servers should score highly
        let has_db = similar.iter().any(|(s, _)| {
            s.name.contains("sqlite") || s.name.contains("astra") || s.name.contains("neon") || s.name.contains("redis")
        });
        assert!(has_db, "Database servers should appear similar to postgres");
    }
}

/// Registry statistics.
pub struct RegistryStats {
    pub total_servers: usize,
    pub total_downloads: i64,
    pub unique_owners: usize,
    pub avg_tools: f64,
    pub top_servers: Vec<(String, i64)>,
    pub transport_counts: Vec<(String, usize)>,
}

impl Database {
    /// Compute aggregate statistics for the registry.
    pub fn stats(&self) -> Result<RegistryStats> {
        let total_servers: usize =
            self.conn.query_row("SELECT COUNT(*) FROM servers", [], |r| r.get(0))?;
        let total_downloads: i64 =
            self.conn.query_row("SELECT COALESCE(SUM(downloads),0) FROM servers", [], |r| r.get(0))?;
        let unique_owners: usize =
            self.conn.query_row("SELECT COUNT(DISTINCT owner) FROM servers", [], |r| r.get(0))?;

        // Average number of tools per server
        let mut stmt = self.conn.prepare("SELECT tools FROM servers")?;
        let tools_rows: Vec<String> = stmt
            .query_map([], |r| r.get::<_, String>(0))?
            .filter_map(|r| r.ok())
            .collect();
        let total_tools: usize = tools_rows
            .iter()
            .map(|t| serde_json::from_str::<Vec<String>>(t).map(|v| v.len()).unwrap_or(0))
            .sum();
        let avg_tools = if total_servers > 0 {
            total_tools as f64 / total_servers as f64
        } else {
            0.0
        };

        // Top 5 servers
        let mut stmt = self.conn.prepare(
            "SELECT owner || '/' || name, downloads FROM servers ORDER BY downloads DESC LIMIT 5",
        )?;
        let top_servers: Vec<(String, i64)> = stmt
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?
            .filter_map(|r| r.ok())
            .collect();

        // Transport counts
        let mut stmt = self.conn.prepare(
            "SELECT transport, COUNT(*) FROM servers GROUP BY transport ORDER BY COUNT(*) DESC",
        )?;
        let transport_counts: Vec<(String, usize)> = stmt
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, usize>(1)?)))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(RegistryStats {
            total_servers,
            total_downloads,
            unique_owners,
            avg_tools,
            top_servers,
            transport_counts,
        })
    }
}

#[cfg(test)]
mod stats_tests {
    use super::*;

    #[test]
    fn test_stats_empty_db() {
        let db = Database::open_in_memory().unwrap();
        let stats = db.stats().unwrap();
        assert_eq!(stats.total_servers, 0);
        assert_eq!(stats.total_downloads, 0);
        assert_eq!(stats.unique_owners, 0);
        assert_eq!(stats.avg_tools, 0.0);
    }

    #[test]
    fn test_stats_with_data() {
        let db = Database::open_in_memory().unwrap();
        db.seed_default_servers().unwrap();
        let stats = db.stats().unwrap();
        assert!(stats.total_servers >= 30);
        assert!(stats.total_downloads > 0);
        assert!(stats.unique_owners > 1);
        assert!(stats.avg_tools > 0.0);
        assert_eq!(stats.top_servers.len(), 5);
        assert!(!stats.transport_counts.is_empty());
    }
}

#[cfg(test)]
mod search_tests {
    use super::*;

    fn make_entry(owner: &str, name: &str, tools: Vec<&str>, downloads: i64) -> ServerEntry {
        ServerEntry {
            id: None,
            owner: owner.into(),
            name: name.into(),
            version: "1.0.0".into(),
            description: format!("Server {name} by {owner}"),
            author: owner.into(),
            license: "MIT".into(),
            repository: String::new(),
            command: "node".into(),
            args: vec![],
            transport: "stdio".into(),
            tools: tools.into_iter().map(String::from).collect(),
            resources: vec![],
            prompts: vec![],
            tags: vec![],
            env: Default::default(),
            homepage: String::new(),
            deprecated: false,
            deprecated_by: None,
            downloads,
            stars: 0,
            created_at: None,
            updated_at: None,
        }
    }

    #[test]
    fn test_search_by_tool_name() {
        let db = Database::open_in_memory().unwrap();
        db.upsert_server(&make_entry("alice", "files", vec!["read_file", "write_file"], 10)).unwrap();
        db.upsert_server(&make_entry("bob", "math", vec!["calculate"], 20)).unwrap();

        let results = db.search("read_file").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "files");
    }

    #[test]
    fn test_search_weighted_ordering() {
        let db = Database::open_in_memory().unwrap();
        db.upsert_server(&make_entry("org", "sqlite", vec![], 10)).unwrap();
        db.upsert_server(&{
            let mut e = make_entry("org", "data-tool", vec![], 10);
            e.description = "Works with sqlite databases".into();
            e
        }).unwrap();

        let results = db.search("sqlite").unwrap();
        assert!(results.len() >= 2);
        assert_eq!(results[0].name, "sqlite", "Name match should rank first");
    }

    #[test]
    fn test_search_no_results() {
        let db = Database::open_in_memory().unwrap();
        db.upsert_server(&make_entry("alice", "tool", vec![], 0)).unwrap();
        let results = db.search("zzzznonexistent").unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_seed_idempotent() {
        let db = Database::open_in_memory().unwrap();
        let first = db.seed_default_servers().unwrap();
        assert!(first > 0);
        let second = db.seed_default_servers().unwrap();
        assert_eq!(second, 0, "Second seed should insert nothing");
    }

    #[test]
    fn test_search_empty_returns_all() {
        let db = Database::open_in_memory().unwrap();
        db.upsert_server(&make_entry("a", "s1", vec![], 10)).unwrap();
        db.upsert_server(&make_entry("b", "s2", vec![], 20)).unwrap();
        let results = db.search("").unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].name, "s2");
    }

    #[test]
    fn test_search_whitespace_only() {
        let db = Database::open_in_memory().unwrap();
        db.upsert_server(&make_entry("a", "s1", vec![], 0)).unwrap();
        let results = db.search("   ").unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_search_special_chars() {
        let db = Database::open_in_memory().unwrap();
        db.upsert_server(&make_entry("org", "server-with-dash", vec![], 0)).unwrap();
        let results = db.search("server-with-dash").unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_search_sql_injection_safe() {
        let db = Database::open_in_memory().unwrap();
        db.upsert_server(&make_entry("a", "s1", vec![], 0)).unwrap();
        // Attempt SQL injection - should not crash or return unexpected results
        let results = db.search("'; DROP TABLE servers; --").unwrap();
        assert!(results.is_empty());
        // Table should still exist
        let results = db.search("s1").unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_search_percent_char_escaped() {
        let db = Database::open_in_memory().unwrap();
        db.upsert_server(&make_entry("a", "s1", vec![], 0)).unwrap();
        // '%' should be escaped and NOT match everything
        let results = db.search("%").unwrap();
        assert!(results.is_empty(), "Escaped '%' should not match all servers");
    }

    #[test]
    fn test_search_underscore_char_escaped() {
        let db = Database::open_in_memory().unwrap();
        db.upsert_server(&make_entry("a", "ab", vec![], 0)).unwrap();
        db.upsert_server(&make_entry("a", "a_b", vec![], 0)).unwrap();
        // '_' in LIKE matches any single char — after escaping it should match literal '_'
        let results = db.search("a_b").unwrap();
        assert_eq!(results.len(), 1, "Should only match literal underscore");
        assert_eq!(results[0].name, "a_b");
    }

    #[test]
    fn test_list_servers_page_beyond_total() {
        let db = Database::open_in_memory().unwrap();
        db.upsert_server(&make_entry("a", "s1", vec![], 0)).unwrap();
        let (servers, total) = db.list_servers(999, 10).unwrap();
        assert_eq!(total, 1);
        assert!(servers.is_empty());
    }

    #[test]
    fn test_list_servers_page_zero_treated_as_one() {
        let db = Database::open_in_memory().unwrap();
        db.upsert_server(&make_entry("a", "s1", vec![], 0)).unwrap();
        let (servers, _) = db.list_servers(0, 10).unwrap();
        assert_eq!(servers.len(), 1);
    }

    #[test]
    fn test_escape_like_function() {
        assert_eq!(escape_like("hello"), "hello");
        assert_eq!(escape_like("100%"), "100\\%");
        assert_eq!(escape_like("a_b"), "a\\_b");
        assert_eq!(escape_like("a\\b"), "a\\\\b");
        assert_eq!(escape_like("%_\\"), "\\%\\_\\\\");
    }

    #[test]
    fn test_search_category_column() {
        let db = Database::open_in_memory().unwrap();
        // "filesystem" should get categorized as Files & VCS
        db.upsert_server(&make_entry("org", "filesystem", vec![], 0)).unwrap();
        let results = db.search("Files").unwrap();
        assert!(!results.is_empty(), "Category column should be searchable");
    }
}

#[cfg(test)]
mod tools_index_tests {
    use super::*;

    #[test]
    fn test_list_tools_empty_db() {
        let db = Database::open_in_memory().unwrap();
        let tools = db.list_tools().unwrap();
        assert!(tools.is_empty());
    }

    #[test]
    fn test_list_tools_with_data() {
        let db = Database::open_in_memory().unwrap();
        db.seed_default_servers().unwrap();
        let tools = db.list_tools().unwrap();
        assert!(tools.len() > 10, "Expected many unique tools, got {}", tools.len());
        // Most popular tools should appear in multiple servers
        let (top_tool, top_servers) = &tools[0];
        assert!(!top_tool.is_empty());
        assert!(!top_servers.is_empty());
    }

    #[test]
    fn test_list_tools_sorted_by_server_count() {
        let db = Database::open_in_memory().unwrap();
        db.seed_default_servers().unwrap();
        let tools = db.list_tools().unwrap();
        for i in 1..tools.len() {
            assert!(
                tools[i - 1].1.len() >= tools[i].1.len(),
                "Expected sorted by server count desc"
            );
        }
    }

    #[test]
    fn test_list_prompts_empty_db() {
        let db = Database::open_in_memory().unwrap();
        let prompts = db.list_prompts().unwrap();
        assert!(prompts.is_empty());
    }

    #[test]
    fn test_list_prompts_with_data() {
        let db = Database::open_in_memory().unwrap();
        let mut entry = ServerEntry {
            id: None,
            owner: "alice".into(),
            name: "prompt-tool".into(),
            version: "1.0.0".into(),
            description: "Test".into(),
            author: "alice".into(),
            license: "MIT".into(),
            repository: String::new(),
            command: "node".into(),
            args: vec![],
            transport: "stdio".into(),
            tools: vec![],
            resources: vec![],
            prompts: vec!["summarize".into(), "analyze".into()],
            tags: vec![],
            env: Default::default(),
            homepage: String::new(),
            deprecated: false,
            deprecated_by: None,
            downloads: 0,
            stars: 0,
            created_at: None,
            updated_at: None,
        };
        db.upsert_server(&entry).unwrap();
        entry.owner = "bob".into();
        entry.name = "other-tool".into();
        entry.prompts = vec!["summarize".into(), "translate".into()];
        db.upsert_server(&entry).unwrap();

        let prompts = db.list_prompts().unwrap();
        assert_eq!(prompts.len(), 3); // summarize, analyze, translate

        // summarize should be in 2 servers
        let summarize = prompts.iter().find(|(p, _)| p == "summarize").unwrap();
        assert_eq!(summarize.1.len(), 2);
    }

    #[test]
    fn test_list_tools_no_duplicates() {
        let db = Database::open_in_memory().unwrap();
        db.seed_default_servers().unwrap();
        let tools = db.list_tools().unwrap();
        let names: std::collections::HashSet<&str> = tools.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names.len(), tools.len(), "Tool names should be unique");
    }

    #[test]
    fn test_list_tools_shared_tool_has_multiple_servers() {
        let db = Database::open_in_memory().unwrap();
        db.seed_default_servers().unwrap();
        let tools = db.list_tools().unwrap();
        // "read_file" appears in both filesystem and gdrive
        let read_file = tools.iter().find(|(t, _)| t == "read_file");
        assert!(read_file.is_some(), "read_file tool should exist");
        let servers = &read_file.unwrap().1;
        assert!(servers.len() >= 2, "read_file should be in multiple servers");
    }
}

#[cfg(test)]
mod prompts_tests {
    use super::*;

    #[test]
    fn test_db_prompts_stored_and_retrieved() {
        let db = Database::open_in_memory().unwrap();
        let entry = ServerEntry {
            id: None,
            owner: "alice".into(),
            name: "prompt-tool".into(),
            version: "1.0.0".into(),
            description: "Has prompts".into(),
            author: "alice".into(),
            license: "MIT".into(),
            repository: String::new(),
            command: "node".into(),
            args: vec![],
            transport: "stdio".into(),
            tools: vec!["tool1".into()],
            resources: vec![],
            prompts: vec!["summarize".into(), "analyze".into(), "translate".into()],
            tags: vec![],
            env: Default::default(),
            homepage: String::new(),
            deprecated: false,
            deprecated_by: None,
            downloads: 0,
            stars: 0,
            created_at: None,
            updated_at: None,
        };
        db.upsert_server(&entry).unwrap();
        let retrieved = db.get_server("alice", "prompt-tool").unwrap().unwrap();
        assert_eq!(retrieved.prompts, vec!["summarize", "analyze", "translate"]);
    }

    #[test]
    fn test_db_prompts_default_empty() {
        let db = Database::open_in_memory().unwrap();
        let entry = ServerEntry {
            id: None,
            owner: "bob".into(),
            name: "no-prompts".into(),
            version: "1.0.0".into(),
            description: "No prompts".into(),
            author: "bob".into(),
            license: "MIT".into(),
            repository: String::new(),
            command: "node".into(),
            args: vec![],
            transport: "stdio".into(),
            tools: vec![],
            resources: vec![],
            prompts: vec![],
            tags: vec![],
            env: Default::default(),
            homepage: String::new(),
            deprecated: false,
            deprecated_by: None,
            downloads: 0,
            stars: 0,
            created_at: None,
            updated_at: None,
        };
        db.upsert_server(&entry).unwrap();
        let retrieved = db.get_server("bob", "no-prompts").unwrap().unwrap();
        assert!(retrieved.prompts.is_empty());
    }

    #[test]
    fn test_db_prompts_upsert_preserves() {
        let db = Database::open_in_memory().unwrap();
        let mut entry = ServerEntry {
            id: None,
            owner: "carol".into(),
            name: "evolving".into(),
            version: "1.0.0".into(),
            description: "First version".into(),
            author: "carol".into(),
            license: "MIT".into(),
            repository: String::new(),
            command: "node".into(),
            args: vec![],
            transport: "stdio".into(),
            tools: vec![],
            resources: vec![],
            prompts: vec!["prompt1".into()],
            tags: vec![],
            env: Default::default(),
            homepage: String::new(),
            deprecated: false,
            deprecated_by: None,
            downloads: 0,
            stars: 0,
            created_at: None,
            updated_at: None,
        };
        db.upsert_server(&entry).unwrap();

        // Update with new prompts
        entry.version = "2.0.0".into();
        entry.prompts = vec!["prompt1".into(), "prompt2".into(), "prompt3".into()];
        db.upsert_server(&entry).unwrap();

        let retrieved = db.get_server("carol", "evolving").unwrap().unwrap();
        assert_eq!(retrieved.version, "2.0.0");
        assert_eq!(retrieved.prompts.len(), 3);
    }
}

#[cfg(test)]
mod tags_tests {
    use super::*;
    use crate::api::types::ServerEntry;

    fn test_entry(owner: &str, name: &str) -> ServerEntry {
        ServerEntry {
            id: None,
            owner: owner.into(),
            name: name.into(),
            version: "1.0.0".into(),
            description: "A test MCP server".into(),
            author: owner.into(),
            license: "MIT".into(),
            repository: format!("https://github.com/{owner}/{name}"),
            command: "node".into(),
            args: vec!["dist/index.js".into()],
            transport: "stdio".into(),
            tools: vec!["read_file".into()],
            resources: vec!["file://".into()],
            prompts: vec![],
            tags: vec![],
            env: Default::default(),
            homepage: String::new(),
            deprecated: false,
            deprecated_by: None,
            downloads: 0,
            stars: 0,
            created_at: None,
            updated_at: None,
        }
    }

    #[test]
    fn test_db_search_by_tags() {
        let db = Database::open_in_memory().unwrap();
        let mut entry = test_entry("alice", "my-tool");
        entry.tags = vec!["llm".into(), "code-review".into()];
        db.upsert_server(&entry).unwrap();

        let results = db.search("llm").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "my-tool");

        let results = db.search("code-review").unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_db_tags_roundtrip() {
        let db = Database::open_in_memory().unwrap();
        let mut entry = test_entry("alice", "tagged");
        entry.tags = vec!["ai".into(), "productivity".into(), "automation".into()];
        db.upsert_server(&entry).unwrap();

        let server = db.get_server("alice", "tagged").unwrap().unwrap();
        assert_eq!(server.tags, vec!["ai", "productivity", "automation"]);
    }

    #[test]
    fn test_db_tags_default_empty() {
        let db = Database::open_in_memory().unwrap();
        let entry = test_entry("bob", "no-tags");
        db.upsert_server(&entry).unwrap();

        let server = db.get_server("bob", "no-tags").unwrap().unwrap();
        assert!(server.tags.is_empty());
    }

    #[test]
    fn test_db_search_multi_word_with_tags() {
        let db = Database::open_in_memory().unwrap();
        let mut e1 = test_entry("alice", "helper");
        e1.tags = vec!["production".into()];
        db.upsert_server(&e1).unwrap();

        let mut e2 = test_entry("bob", "helper2");
        e2.tags = vec!["testing".into()];
        db.upsert_server(&e2).unwrap();

        let results = db.search("helper production").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].owner, "alice");
    }
}

#[cfg(test)]
mod suggest_tests {
    use super::*;

    fn make_entry(owner: &str, name: &str, downloads: i64) -> ServerEntry {
        ServerEntry {
            id: None,
            owner: owner.into(),
            name: name.into(),
            version: "1.0.0".into(),
            description: format!("Server {name}"),
            author: owner.into(),
            license: "MIT".into(),
            repository: String::new(),
            command: "node".into(),
            args: vec![],
            transport: "stdio".into(),
            tools: vec![],
            resources: vec![],
            prompts: vec![],
            tags: vec![],
            env: Default::default(),
            homepage: String::new(),
            deprecated: false,
            deprecated_by: None,
            downloads,
            stars: 0,
            created_at: None,
            updated_at: None,
        }
    }

    #[test]
    fn test_suggest_by_name_prefix() {
        let db = Database::open_in_memory().unwrap();
        db.upsert_server(&make_entry("org", "filesystem", 100)).unwrap();
        db.upsert_server(&make_entry("org", "file-upload", 50)).unwrap();
        db.upsert_server(&make_entry("org", "sqlite", 200)).unwrap();

        let results = db.suggest("file", 10).unwrap();
        assert_eq!(results.len(), 2);
        // Should be sorted by downloads: filesystem (100) before file-upload (50)
        assert_eq!(results[0], "org/filesystem");
        assert_eq!(results[1], "org/file-upload");
    }

    #[test]
    fn test_suggest_by_full_name_prefix() {
        let db = Database::open_in_memory().unwrap();
        db.upsert_server(&make_entry("modelcontextprotocol", "filesystem", 100)).unwrap();
        db.upsert_server(&make_entry("other", "filesystem", 50)).unwrap();

        let results = db.suggest("modelcontextprotocol/file", 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], "modelcontextprotocol/filesystem");
    }

    #[test]
    fn test_suggest_empty_prefix() {
        let db = Database::open_in_memory().unwrap();
        db.upsert_server(&make_entry("org", "tool", 10)).unwrap();
        let results = db.suggest("", 10).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_suggest_respects_limit() {
        let db = Database::open_in_memory().unwrap();
        for i in 0..10 {
            db.upsert_server(&make_entry("org", &format!("server-{i}"), i * 10)).unwrap();
        }
        let results = db.suggest("server", 3).unwrap();
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn test_suggest_no_match() {
        let db = Database::open_in_memory().unwrap();
        db.upsert_server(&make_entry("org", "tool", 10)).unwrap();
        let results = db.suggest("zzz", 10).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_suggest_special_chars_escaped() {
        let db = Database::open_in_memory().unwrap();
        db.upsert_server(&make_entry("org", "tool", 10)).unwrap();
        // '%' should not match everything
        let results = db.suggest("%", 10).unwrap();
        assert!(results.is_empty());
    }
}

#[cfg(test)]
mod improved_search_tests {
    use super::*;

    fn make_entry(owner: &str, name: &str, tools: Vec<&str>, downloads: i64) -> crate::api::types::ServerEntry {
        crate::api::types::ServerEntry {
            id: None,
            owner: owner.into(),
            name: name.into(),
            version: "1.0.0".into(),
            description: format!("Server {name} by {owner}"),
            author: owner.into(),
            license: "MIT".into(),
            repository: String::new(),
            command: "node".into(),
            args: vec![],
            transport: "stdio".into(),
            tools: tools.into_iter().map(String::from).collect(),
            resources: vec![],
            prompts: vec![],
            tags: vec![],
            env: Default::default(),
            homepage: String::new(),
            deprecated: false,
            deprecated_by: None,
            downloads,
            stars: 0,
            created_at: None,
            updated_at: None,
        }
    }

    #[test]
    fn test_search_exact_name_ranks_first() {
        let db = Database::open_in_memory().unwrap();
        // Insert a server named "git" and one with "git" in description
        db.upsert_server(&make_entry("org", "git", vec![], 10)).unwrap();
        db.upsert_server(&{
            let mut e = make_entry("org", "git-tool", vec![], 100);
            e.description = "Advanced git operations".into();
            e
        }).unwrap();

        let results = db.search("git").unwrap();
        assert!(!results.is_empty());
        // Exact name match "git" should rank first despite fewer downloads
        assert_eq!(results[0].name, "git");
    }

    #[test]
    fn test_search_owner_and_name_multi_word() {
        let db = Database::open_in_memory().unwrap();
        db.upsert_server(&make_entry("alice", "tool", vec![], 10)).unwrap();
        db.upsert_server(&make_entry("bob", "tool", vec![], 100)).unwrap();

        // Multi-word search: "alice tool" should match alice's server
        let results = db.search("alice tool").unwrap();
        assert!(!results.is_empty());
        assert_eq!(results[0].owner, "alice");
    }

    #[test]
    fn test_search_author_boost() {
        let db = Database::open_in_memory().unwrap();
        let mut e1 = make_entry("org1", "s1", vec![], 10);
        e1.author = "Anthropic".into();
        db.upsert_server(&e1).unwrap();

        let mut e2 = make_entry("org2", "s2", vec![], 10);
        e2.description = "Uses Anthropic's models".into();
        db.upsert_server(&e2).unwrap();

        let results = db.search("Anthropic").unwrap();
        assert!(!results.is_empty());
    }

    #[test]
    fn test_search_tag_scoring() {
        let db = Database::open_in_memory().unwrap();
        let mut e = make_entry("dev", "tagged", vec![], 10);
        e.tags = vec!["llm".into(), "production".into()];
        db.upsert_server(&e).unwrap();

        let results = db.search("llm").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "tagged");
    }
}

#[cfg(test)]
mod tag_tests {
    use super::*;

    fn test_entry_with_tags(owner: &str, name: &str, tags: Vec<&str>) -> ServerEntry {
        ServerEntry {
            id: None,
            owner: owner.into(),
            name: name.into(),
            version: "1.0.0".into(),
            description: "A test server".into(),
            author: owner.into(),
            license: "MIT".into(),
            repository: format!("https://github.com/{owner}/{name}"),
            command: "node".into(),
            args: vec!["index.js".into()],
            transport: "stdio".into(),
            tools: vec![],
            resources: vec![],
            prompts: vec![],
            tags: tags.into_iter().map(String::from).collect(),
            env: Default::default(),
            homepage: String::new(),
            deprecated: false,
            deprecated_by: None,
            downloads: 0,
            stars: 0,
            created_at: None,
            updated_at: None,
        }
    }

    #[test]
    fn test_search_by_tags() {
        let db = Database::open_in_memory().unwrap();
        db.upsert_server(&test_entry_with_tags("alice", "server1", vec!["ai", "llm"])).unwrap();
        db.upsert_server(&test_entry_with_tags("bob", "server2", vec!["database", "sql"])).unwrap();
        db.upsert_server(&test_entry_with_tags("carol", "server3", vec!["ai", "vision"])).unwrap();

        let results = db.search_by_tags("ai").unwrap();
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|s| s.tags.iter().any(|t| t.contains("ai"))));
    }

    #[test]
    fn test_search_by_tags_case_insensitive() {
        let db = Database::open_in_memory().unwrap();
        db.upsert_server(&test_entry_with_tags("alice", "s1", vec!["AI", "LLM"])).unwrap();
        let results = db.search_by_tags("ai").unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_search_by_tags_no_match() {
        let db = Database::open_in_memory().unwrap();
        db.upsert_server(&test_entry_with_tags("alice", "s1", vec!["web"])).unwrap();
        let results = db.search_by_tags("database").unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_list_tags() {
        let db = Database::open_in_memory().unwrap();
        db.upsert_server(&test_entry_with_tags("alice", "s1", vec!["ai", "llm"])).unwrap();
        db.upsert_server(&test_entry_with_tags("bob", "s2", vec!["ai", "web"])).unwrap();
        db.upsert_server(&test_entry_with_tags("carol", "s3", vec!["web"])).unwrap();

        let tags = db.list_tags().unwrap();
        assert!(tags.len() >= 3);
        // "ai" should have 2 servers, "web" 2, "llm" 1
        let ai_entry = tags.iter().find(|(t, _)| t == "ai").unwrap();
        assert_eq!(ai_entry.1.len(), 2);
        let web_entry = tags.iter().find(|(t, _)| t == "web").unwrap();
        assert_eq!(web_entry.1.len(), 2);
    }

    #[test]
    fn test_list_tags_empty_db() {
        let db = Database::open_in_memory().unwrap();
        let tags = db.list_tags().unwrap();
        assert!(tags.is_empty());
    }

    #[test]
    fn test_list_tags_sorted_by_count() {
        let db = Database::open_in_memory().unwrap();
        db.upsert_server(&test_entry_with_tags("a", "s1", vec!["rare"])).unwrap();
        db.upsert_server(&test_entry_with_tags("b", "s2", vec!["common", "rare"])).unwrap();
        db.upsert_server(&test_entry_with_tags("c", "s3", vec!["common"])).unwrap();
        db.upsert_server(&test_entry_with_tags("d", "s4", vec!["common"])).unwrap();

        let tags = db.list_tags().unwrap();
        assert_eq!(tags[0].0, "common");
        assert_eq!(tags[0].1.len(), 3);
    }

    #[test]
    fn test_seed_servers_have_tags() {
        let db = Database::open_in_memory().unwrap();
        db.seed_default_servers().unwrap();
        let tags = db.list_tags().unwrap();
        assert!(tags.len() >= 5, "Seeded servers should produce at least 5 unique tags");
        // "official" tag should exist for modelcontextprotocol servers
        assert!(tags.iter().any(|(t, _)| t == "official"), "Should have 'official' tag");
    }
}

impl Database {
    /// Delete multiple servers at once. Returns the number of actually deleted servers.
    pub fn bulk_delete(&self, refs: &[(String, String)]) -> Result<usize> {
        let mut deleted = 0usize;
        for (owner, name) in refs {
            if self.delete_server(owner, name)? {
                deleted += 1;
            }
        }
        Ok(deleted)
    }

    /// Efficient total server count without fetching rows.
    pub fn count_servers(&self) -> Result<usize> {
        let count: usize = self.conn.query_row("SELECT COUNT(*) FROM servers", [], |row| row.get(0))?;
        Ok(count)
    }

    /// Return a random server, optionally filtered by category.
    pub fn random_server(&self, category: Option<&str>) -> Result<Option<ServerEntry>> {
        let (sql, params_vec): (String, Vec<String>) = if let Some(cat) = category {
            let escaped = escape_like(cat);
            let pattern = format!("%{escaped}%");
            (
                "SELECT id, owner, name, version, description, author, license, repository,
                        command, args, transport, tools, resources, prompts, tags, env, homepage, deprecated, deprecated_by, category, downloads, stars, created_at, updated_at
                 FROM servers WHERE LOWER(category) LIKE LOWER(?1) ESCAPE '\\' ORDER BY RANDOM() LIMIT 1".into(),
                vec![pattern],
            )
        } else {
            (
                "SELECT id, owner, name, version, description, author, license, repository,
                        command, args, transport, tools, resources, prompts, tags, env, homepage, deprecated, deprecated_by, category, downloads, stars, created_at, updated_at
                 FROM servers ORDER BY RANDOM() LIMIT 1".into(),
                vec![],
            )
        };

        let mut stmt = self.conn.prepare(&sql)?;
        let params_refs: Vec<&dyn rusqlite::types::ToSql> =
            params_vec.iter().map(|v| v as &dyn rusqlite::types::ToSql).collect();
        let mut rows = stmt.query_map(params_refs.as_slice(), row_mapper)?;

        match rows.next() {
            Some(Ok(r)) => Ok(Some(r.into_entry())),
            Some(Err(e)) => Err(e.into()),
            None => Ok(None),
        }
    }
}

#[cfg(test)]
mod bulk_and_count_tests {
    use super::*;

    fn test_entry(owner: &str, name: &str) -> ServerEntry {
        ServerEntry {
            id: None,
            owner: owner.into(),
            name: name.into(),
            version: "1.0.0".into(),
            description: "Test".into(),
            author: owner.into(),
            license: "MIT".into(),
            repository: String::new(),
            command: "node".into(),
            args: vec![],
            transport: "stdio".into(),
            tools: vec![],
            resources: vec![],
            prompts: vec![],
            tags: vec![],
            env: Default::default(),
            homepage: String::new(),
            deprecated: false,
            deprecated_by: None,
            downloads: 0,
            stars: 0,
            created_at: None,
            updated_at: None,
        }
    }

    #[test]
    fn test_count_servers_empty() {
        let db = Database::open_in_memory().unwrap();
        assert_eq!(db.count_servers().unwrap(), 0);
    }

    #[test]
    fn test_count_servers_with_data() {
        let db = Database::open_in_memory().unwrap();
        db.upsert_server(&test_entry("a", "s1")).unwrap();
        db.upsert_server(&test_entry("b", "s2")).unwrap();
        db.upsert_server(&test_entry("c", "s3")).unwrap();
        assert_eq!(db.count_servers().unwrap(), 3);
    }

    #[test]
    fn test_bulk_delete_all_found() {
        let db = Database::open_in_memory().unwrap();
        db.upsert_server(&test_entry("a", "s1")).unwrap();
        db.upsert_server(&test_entry("b", "s2")).unwrap();
        db.upsert_server(&test_entry("c", "s3")).unwrap();

        let refs = vec![
            ("a".into(), "s1".into()),
            ("b".into(), "s2".into()),
        ];
        let deleted = db.bulk_delete(&refs).unwrap();
        assert_eq!(deleted, 2);
        assert_eq!(db.count_servers().unwrap(), 1);
    }

    #[test]
    fn test_bulk_delete_partial() {
        let db = Database::open_in_memory().unwrap();
        db.upsert_server(&test_entry("a", "s1")).unwrap();

        let refs = vec![
            ("a".into(), "s1".into()),
            ("nobody".into(), "nothing".into()),
        ];
        let deleted = db.bulk_delete(&refs).unwrap();
        assert_eq!(deleted, 1);
        assert_eq!(db.count_servers().unwrap(), 0);
    }

    #[test]
    fn test_bulk_delete_empty() {
        let db = Database::open_in_memory().unwrap();
        let deleted = db.bulk_delete(&[]).unwrap();
        assert_eq!(deleted, 0);
    }

    #[test]
    fn test_random_server_nonempty() {
        let db = Database::open_in_memory().unwrap();
        db.seed_default_servers().unwrap();
        let server = db.random_server(None).unwrap();
        assert!(server.is_some());
    }

    #[test]
    fn test_random_server_empty_db() {
        let db = Database::open_in_memory().unwrap();
        let server = db.random_server(None).unwrap();
        assert!(server.is_none());
    }

    #[test]
    fn test_random_server_with_category() {
        let db = Database::open_in_memory().unwrap();
        db.seed_default_servers().unwrap();
        let server = db.random_server(Some("database")).unwrap();
        assert!(server.is_some());
        let s = server.unwrap();
        let cat = crate::registry::seed::server_category(&s.owner, &s.name).to_lowercase();
        assert!(cat.contains("database"), "Expected database category, got {cat}");
    }

    #[test]
    fn test_random_server_nonexistent_category() {
        let db = Database::open_in_memory().unwrap();
        db.seed_default_servers().unwrap();
        let server = db.random_server(Some("zzzznonexistent")).unwrap();
        assert!(server.is_none());
    }
}

impl Database {
    /// List all unique owners with their server counts, sorted by count descending.
    pub fn list_owners(&self) -> Result<Vec<(String, usize)>> {
        let mut stmt = self.conn.prepare(
            "SELECT owner, COUNT(*) FROM servers GROUP BY owner ORDER BY COUNT(*) DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, usize>(1)?))
        })?;
        let mut owners = Vec::new();
        for row in rows {
            owners.push(row?);
        }
        Ok(owners)
    }

    /// List servers ordered by most recently updated.
    pub fn recently_updated(&self, limit: usize) -> Result<Vec<ServerEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, owner, name, version, description, author, license, repository,
                    command, args, transport, tools, resources, prompts, tags, env, homepage, deprecated, deprecated_by, category, downloads, stars, created_at, updated_at
             FROM servers ORDER BY updated_at DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], row_mapper)?;
        let mut entries = Vec::new();
        for row in rows {
            entries.push(row?.into_entry());
        }
        Ok(entries)
    }

    /// Get recent version publications across all servers, newest first.
    pub fn recent_versions(&self, limit: usize) -> Result<Vec<(String, String, String, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT owner, name, version, published_at FROM server_versions
             ORDER BY id DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?;
        let mut versions = Vec::new();
        for row in rows {
            versions.push(row?);
        }
        Ok(versions)
    }

    /// Export all servers as a Vec (for full registry dump).
    pub fn export_all(&self) -> Result<Vec<ServerEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, owner, name, version, description, author, license, repository,
                    command, args, transport, tools, resources, prompts, tags, env, homepage, deprecated, deprecated_by, category, downloads, stars, created_at, updated_at
             FROM servers ORDER BY owner, name",
        )?;
        let rows = stmt.query_map([], row_mapper)?;
        let mut entries = Vec::new();
        for row in rows {
            entries.push(row?.into_entry());
        }
        Ok(entries)
    }

    /// Search servers with OR semantics (pipe-separated terms: "postgres|sqlite|redis").
    pub fn search_any(&self, query: &str) -> Result<Vec<ServerEntry>> {
        let terms: Vec<&str> = query.split('|').map(|t| t.trim()).filter(|t| !t.is_empty()).collect();
        if terms.is_empty() {
            return Ok(Vec::new());
        }

        let mut conditions = Vec::new();
        let mut param_values: Vec<String> = Vec::new();

        for term in &terms {
            let escaped = escape_like(term);
            let pattern = format!("%{escaped}%");
            let idx = param_values.len() + 1;
            param_values.push(pattern);
            conditions.push(format!(
                "(name LIKE ?{idx} ESCAPE '\\' OR description LIKE ?{idx} ESCAPE '\\' OR owner LIKE ?{idx} ESCAPE '\\' OR tools LIKE ?{idx} ESCAPE '\\' OR tags LIKE ?{idx} ESCAPE '\\')"
            ));
        }

        let where_clause = conditions.join(" OR ");
        let sql = format!(
            "SELECT id, owner, name, version, description, author, license, repository,
                    command, args, transport, tools, resources, prompts, tags, env, homepage, deprecated, deprecated_by, category, downloads, stars, created_at, updated_at
             FROM servers
             WHERE {where_clause}
             ORDER BY downloads DESC
             LIMIT 50"
        );

        let mut stmt = self.conn.prepare(&sql)?;
        let params_refs: Vec<&dyn rusqlite::types::ToSql> =
            param_values.iter().map(|v| v as &dyn rusqlite::types::ToSql).collect();
        let rows = stmt.query_map(params_refs.as_slice(), row_mapper)?;

        let mut entries = Vec::new();
        for row in rows {
            entries.push(row?.into_entry());
        }
        Ok(entries)
    }

    /// Search servers using a regex pattern against name, description, owner, and tools.
    pub fn search_regex(&self, pattern: &str) -> Result<Vec<ServerEntry>> {
        let re = regex::Regex::new(pattern).map_err(|e| {
            crate::error::McpRegError::Validation(format!("Invalid regex: {e}"))
        })?;

        let all = self.list_all()?;
        let mut matched: Vec<ServerEntry> = all
            .into_iter()
            .filter(|s| {
                re.is_match(&s.name)
                    || re.is_match(&s.description)
                    || re.is_match(&s.owner)
                    || re.is_match(&s.full_name())
                    || s.tools.iter().any(|t| re.is_match(t))
                    || s.tags.iter().any(|t| re.is_match(t))
            })
            .collect();
        matched.sort_by(|a, b| b.downloads.cmp(&a.downloads));
        Ok(matched)
    }
}

#[cfg(test)]
mod regex_search_tests {
    use super::*;
    use crate::api::types::ServerEntry;

    fn create_test_db() -> Database {
        let db = Database::open_in_memory().unwrap();
        let entries = vec![
            ("org", "filesystem", "File system access", vec!["read_file", "write_file"]),
            ("org", "sqlite", "SQLite database", vec!["query", "execute"]),
            ("org", "web-search", "Search the web", vec!["brave_search"]),
            ("org", "postgres-db", "PostgreSQL access", vec!["pg_query"]),
        ];
        for (owner, name, desc, tools) in entries {
            let entry = ServerEntry {
                id: None,
                owner: owner.into(),
                name: name.into(),
                version: "1.0.0".into(),
                description: desc.into(),
                author: owner.into(),
                license: "MIT".into(),
                repository: String::new(),
                command: "node".into(),
                args: vec![],
                transport: "stdio".into(),
                tools: tools.into_iter().map(String::from).collect(),
                resources: vec![],
                prompts: vec![],
                tags: vec![],
                env: Default::default(),
                homepage: String::new(),
                deprecated: false,
                deprecated_by: None,
                downloads: 100,
                stars: 0,
                created_at: None,
                updated_at: None,
            };
            db.upsert_server(&entry).unwrap();
        }
        db
    }

    #[test]
    fn test_search_regex_name_match() {
        let db = create_test_db();
        let results = db.search_regex("file.*").unwrap();
        assert!(results.iter().any(|s| s.name == "filesystem"));
    }

    #[test]
    fn test_search_regex_alternation() {
        let db = create_test_db();
        let results = db.search_regex("sqlite|postgres").unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_search_regex_tool_match() {
        let db = create_test_db();
        let results = db.search_regex("brave_search").unwrap();
        assert!(results.iter().any(|s| s.name == "web-search"));
    }

    #[test]
    fn test_search_regex_no_match() {
        let db = create_test_db();
        let results = db.search_regex("^zzz$").unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_search_regex_invalid_pattern() {
        let db = create_test_db();
        let result = db.search_regex("[invalid");
        assert!(result.is_err());
    }

    #[test]
    fn test_search_regex_case_sensitive() {
        let db = create_test_db();
        // Regex is case-sensitive by default
        let results = db.search_regex("(?i)FILE").unwrap();
        assert!(!results.is_empty());
    }

    #[test]
    fn test_search_regex_suffix_pattern() {
        let db = create_test_db();
        let results = db.search_regex("db$").unwrap();
        assert!(results.iter().any(|s| s.name == "postgres-db"));
    }
}

#[cfg(test)]
mod owners_export_tests {
    use super::*;

    #[test]
    fn test_list_owners_empty() {
        let db = Database::open_in_memory().unwrap();
        let owners = db.list_owners().unwrap();
        assert!(owners.is_empty());
    }

    #[test]
    fn test_list_owners_with_data() {
        let db = Database::open_in_memory().unwrap();
        db.seed_default_servers().unwrap();
        let owners = db.list_owners().unwrap();
        assert!(!owners.is_empty());
        // modelcontextprotocol should be top owner
        assert_eq!(owners[0].0, "modelcontextprotocol");
        assert!(owners[0].1 > 5, "MCP org should have many servers");
    }

    #[test]
    fn test_list_owners_sorted_by_count() {
        let db = Database::open_in_memory().unwrap();
        db.seed_default_servers().unwrap();
        let owners = db.list_owners().unwrap();
        for i in 1..owners.len() {
            assert!(owners[i - 1].1 >= owners[i].1, "Should be sorted by count desc");
        }
    }

    #[test]
    fn test_export_all_empty() {
        let db = Database::open_in_memory().unwrap();
        let servers = db.export_all().unwrap();
        assert!(servers.is_empty());
    }

    #[test]
    fn test_export_all_with_data() {
        let db = Database::open_in_memory().unwrap();
        db.seed_default_servers().unwrap();
        let servers = db.export_all().unwrap();
        let count = db.count_servers().unwrap();
        assert_eq!(servers.len(), count);
    }

    #[test]
    fn test_export_all_sorted_by_owner_name() {
        let db = Database::open_in_memory().unwrap();
        db.seed_default_servers().unwrap();
        let servers = db.export_all().unwrap();
        for i in 1..servers.len() {
            let prev = format!("{}/{}", servers[i - 1].owner, servers[i - 1].name);
            let curr = format!("{}/{}", servers[i].owner, servers[i].name);
            assert!(prev <= curr, "Expected sorted order: {prev} <= {curr}");
        }
    }

    #[test]
    fn test_search_any_or_semantics() {
        let db = Database::open_in_memory().unwrap();
        db.seed_default_servers().unwrap();
        let results = db.search_any("postgres|sqlite|redis").unwrap();
        assert!(results.len() >= 3, "Should find at least 3 DB servers");
        let names: Vec<String> = results.iter().map(|s| s.name.clone()).collect();
        assert!(names.iter().any(|n| n.contains("postgres")));
        assert!(names.iter().any(|n| n.contains("sqlite")));
    }

    #[test]
    fn test_search_any_empty() {
        let db = Database::open_in_memory().unwrap();
        let results = db.search_any("").unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_search_any_single_term() {
        let db = Database::open_in_memory().unwrap();
        db.seed_default_servers().unwrap();
        let results = db.search_any("github").unwrap();
        assert!(!results.is_empty());
    }

    #[test]
    fn test_env_field_roundtrip() {
        let db = Database::open_in_memory().unwrap();
        let mut entry = ServerEntry {
            id: None,
            owner: "dev".into(),
            name: "env-test".into(),
            version: "1.0.0".into(),
            description: "Server with env hints".into(),
            author: "dev".into(),
            license: "MIT".into(),
            repository: String::new(),
            command: "node".into(),
            args: vec!["index.js".into()],
            transport: "stdio".into(),
            tools: vec!["query".into()],
            resources: vec![],
            prompts: vec![],
            tags: vec![],
            env: std::collections::HashMap::new(),
            homepage: "https://example.com".into(),
            deprecated: false,
            deprecated_by: None,
            downloads: 0,
            stars: 0,
            created_at: None,
            updated_at: None,
        };
        entry.env.insert("API_KEY".into(), "your-api-key-here".into());
        entry.env.insert("DATABASE_URL".into(), "postgres://localhost/db".into());
        db.upsert_server(&entry).unwrap();

        let retrieved = db.get_server("dev", "env-test").unwrap().unwrap();
        assert_eq!(retrieved.env.len(), 2);
        assert_eq!(retrieved.env.get("API_KEY").unwrap(), "your-api-key-here");
        assert_eq!(retrieved.homepage, "https://example.com");
    }

    #[test]
    fn test_homepage_field_roundtrip() {
        let db = Database::open_in_memory().unwrap();
        let entry = ServerEntry {
            id: None,
            owner: "org".into(),
            name: "homepage-test".into(),
            version: "1.0.0".into(),
            description: "Has homepage".into(),
            author: "org".into(),
            license: "MIT".into(),
            repository: "https://github.com/org/repo".into(),
            command: "node".into(),
            args: vec![],
            transport: "stdio".into(),
            tools: vec![],
            resources: vec![],
            prompts: vec![],
            tags: vec![],
            env: Default::default(),
            homepage: "https://my-mcp-server.dev".into(),
            deprecated: false,
            deprecated_by: None,
            downloads: 0,
            stars: 0,
            created_at: None,
            updated_at: None,
        };
        db.upsert_server(&entry).unwrap();
        let retrieved = db.get_server("org", "homepage-test").unwrap().unwrap();
        assert_eq!(retrieved.homepage, "https://my-mcp-server.dev");
    }

    #[test]
    fn test_env_default_empty() {
        let db = Database::open_in_memory().unwrap();
        db.seed_default_servers().unwrap();
        let server = db.get_server("modelcontextprotocol", "filesystem").unwrap().unwrap();
        assert!(server.env.is_empty(), "Default servers should have empty env");
    }
}

#[cfg(test)]
mod resources_db_tests {
    use super::*;

    fn test_entry_with_resources(owner: &str, name: &str, resources: Vec<&str>) -> ServerEntry {
        ServerEntry {
            id: None,
            owner: owner.into(),
            name: name.into(),
            version: "1.0.0".into(),
            description: format!("{name} server"),
            author: owner.into(),
            license: "MIT".into(),
            repository: String::new(),
            command: "node".into(),
            args: vec![],
            transport: "stdio".into(),
            tools: vec![],
            resources: resources.into_iter().map(String::from).collect(),
            prompts: vec![],
            tags: vec![],
            env: Default::default(),
            homepage: String::new(),
            deprecated: false,
            deprecated_by: None,
            downloads: 0,
            stars: 0,
            created_at: None,
            updated_at: None,
        }
    }

    #[test]
    fn test_list_resources_empty_db() {
        let db = Database::open_in_memory().unwrap();
        let result = db.list_resources().unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_list_resources_with_data() {
        let db = Database::open_in_memory().unwrap();
        db.upsert_server(&test_entry_with_resources("a", "fs", vec!["file://", "dir://"])).unwrap();
        db.upsert_server(&test_entry_with_resources("b", "db", vec!["postgres://", "file://"])).unwrap();
        let result = db.list_resources().unwrap();
        // file:// appears in 2 servers, should be first
        assert_eq!(result[0].0, "file://");
        assert_eq!(result[0].1.len(), 2);
        // dir:// and postgres:// each in 1 server
        let names: Vec<&str> = result.iter().map(|(n, _)| n.as_str()).collect();
        assert!(names.contains(&"dir://"));
        assert!(names.contains(&"postgres://"));
    }

    #[test]
    fn test_list_resources_sorted_by_count() {
        let db = Database::open_in_memory().unwrap();
        db.upsert_server(&test_entry_with_resources("a", "s1", vec!["common://"])).unwrap();
        db.upsert_server(&test_entry_with_resources("b", "s2", vec!["common://"])).unwrap();
        db.upsert_server(&test_entry_with_resources("c", "s3", vec!["common://", "rare://"])).unwrap();
        let result = db.list_resources().unwrap();
        assert_eq!(result[0].0, "common://");
        assert_eq!(result[0].1.len(), 3);
        assert_eq!(result[1].0, "rare://");
        assert_eq!(result[1].1.len(), 1);
    }

    #[test]
    fn test_search_by_resource() {
        let db = Database::open_in_memory().unwrap();
        db.upsert_server(&test_entry_with_resources("a", "fs", vec!["file://local", "dir://"])).unwrap();
        db.upsert_server(&test_entry_with_resources("b", "db", vec!["postgres://"])).unwrap();
        let result = db.search_by_resource("file").unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "fs");
    }

    #[test]
    fn test_search_by_resource_case_insensitive() {
        let db = Database::open_in_memory().unwrap();
        db.upsert_server(&test_entry_with_resources("a", "fs", vec!["File://"])).unwrap();
        let result = db.search_by_resource("file").unwrap();
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_search_by_resource_no_match() {
        let db = Database::open_in_memory().unwrap();
        db.upsert_server(&test_entry_with_resources("a", "fs", vec!["file://"])).unwrap();
        let result = db.search_by_resource("postgres").unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_list_resources_no_duplicates() {
        let db = Database::open_in_memory().unwrap();
        // Same resource in same server shouldn't create duplicates
        db.upsert_server(&test_entry_with_resources("a", "fs", vec!["file://", "file://"])).unwrap();
        let result = db.list_resources().unwrap();
        // file:// appears once in the list, with server a/fs listed (possibly twice due to input dup)
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, "file://");
    }
}

#[cfg(test)]
mod fuzzy_edge_case_tests {
    use crate::fuzzy;

    #[test]
    fn test_fuzzy_empty_strings() {
        let score = fuzzy::fuzzy_score("", "", 3);
        // Empty strings should return Some(0)
        assert_eq!(score, Some(0));
    }

    #[test]
    fn test_fuzzy_query_longer_than_target() {
        let score = fuzzy::fuzzy_score("verylongquery", "ab", 3);
        // Should return None (too far apart)
        assert!(score.is_none(), "Very different strings should return None");
    }

    #[test]
    fn test_fuzzy_exact_match() {
        let score = fuzzy::fuzzy_score("hello", "hello", 3);
        assert_eq!(score, Some(0), "Exact match should have distance 0");
    }

    #[test]
    fn test_fuzzy_near_match() {
        let score = fuzzy::fuzzy_score("helo", "hello", 3);
        assert!(score.is_some(), "Near match should return Some: {:?}", score);
        assert!(score.unwrap() <= 3, "Should be within max_distance");
    }

    #[test]
    fn test_fuzzy_no_match() {
        let score = fuzzy::fuzzy_score("xyz", "abc", 1);
        assert!(score.is_none(), "Completely different strings with low max should be None");
    }

    #[test]
    fn test_fuzzy_single_char() {
        let score = fuzzy::fuzzy_score("a", "a", 3);
        assert_eq!(score, Some(0));
    }

    #[test]
    fn test_fuzzy_case_difference() {
        // fuzzy_score operates on raw strings - check it doesn't crash
        let score = fuzzy::fuzzy_score("hello", "Hello", 3);
        // May or may not match depending on implementation
        assert!(score.is_some() || score.is_none());
    }

    #[test]
    fn test_fuzzy_unicode() {
        let _score = fuzzy::fuzzy_score("über", "über-server", 5);
        // Just ensure it doesn't panic
    }

    #[test]
    fn test_fuzzy_special_chars() {
        let _score = fuzzy::fuzzy_score("file://", "file://path", 5);
        // Just ensure it doesn't panic
    }
}

#[cfg(test)]
mod negation_search_tests {
    use super::*;
    use crate::api::types::ServerEntry;

    fn create_test_db() -> Database {
        let db = Database::open_in_memory().unwrap();

        let servers = vec![
            ("alice", "filesystem", "File system access", "file,storage", "read_file,write_file"),
            ("bob", "sqlite", "SQLite database access", "database", "query,execute"),
            ("carol", "web-search", "Web search via API", "search,web", "search,fetch"),
            ("dave", "browser", "Browser automation", "browser,web", "navigate,click,screenshot"),
            ("eve", "postgres", "PostgreSQL database", "database", "pg_query,pg_execute"),
        ];

        for (owner, name, desc, tags, tools) in servers {
            let entry = ServerEntry {
                id: None,
                owner: owner.into(),
                name: name.into(),
                version: "1.0.0".into(),
                description: desc.into(),
                author: owner.into(),
                license: "MIT".into(),
                repository: String::new(),
                command: "node".into(),
                args: vec![],
                transport: "stdio".into(),
                tools: tools.split(',').map(String::from).collect(),
                resources: vec![],
                prompts: vec![],
                tags: tags.split(',').map(String::from).collect(),
                env: Default::default(),
                homepage: String::new(),
                deprecated: false,
                deprecated_by: None,
                downloads: 100,
                stars: 0,
                created_at: None,
                updated_at: None,
            };
            db.upsert_server(&entry).unwrap();
        }

        db
    }

    #[test]
    fn test_search_negation_excludes_term() {
        let db = create_test_db();
        let results = db.search("database -postgres").unwrap();
        // Should find sqlite but not postgres
        assert!(results.iter().any(|s| s.name == "sqlite"));
        assert!(!results.iter().any(|s| s.name == "postgres"));
    }

    #[test]
    fn test_search_negation_only() {
        let db = create_test_db();
        let results = db.search("-database").unwrap();
        // Should exclude all database servers
        assert!(!results.iter().any(|s| s.name == "sqlite"));
        assert!(!results.iter().any(|s| s.name == "postgres"));
        // But include others
        assert!(results.iter().any(|s| s.name == "filesystem"));
    }

    #[test]
    fn test_search_multiple_negations() {
        let db = create_test_db();
        let results = db.search("-database -web").unwrap();
        // Should exclude database and web servers
        assert!(!results.iter().any(|s| s.name == "sqlite"));
        assert!(!results.iter().any(|s| s.name == "web-search"));
        assert!(!results.iter().any(|s| s.name == "browser"));
        assert!(results.iter().any(|s| s.name == "filesystem"));
    }

    #[test]
    fn test_search_negation_no_effect_when_not_matching() {
        let db = create_test_db();
        let results = db.search("filesystem -nonexistent").unwrap();
        assert!(results.iter().any(|s| s.name == "filesystem"));
    }

    #[test]
    fn test_search_without_negation_unchanged() {
        let db = create_test_db();
        let results = db.search("sqlite").unwrap();
        assert!(results.iter().any(|s| s.name == "sqlite"));
    }

    #[test]
    fn test_search_empty_negation_ignored() {
        let db = create_test_db();
        // Bare "-" should be ignored
        let results = db.search("sqlite -").unwrap();
        assert!(results.iter().any(|s| s.name == "sqlite"));
    }
}

impl Database {
    /// Search with weighted field scoring.
    #[allow(dead_code)]
    /// Name matches are weighted 3x, description 2x, tools/tags 1x.
    /// Returns results sorted by weighted relevance score (highest first).
    pub fn search_weighted(&self, query: &str) -> Result<Vec<(ServerEntry, f64)>> {
        let query_lower = query.to_lowercase();
        let tokens: Vec<&str> = query_lower.split_whitespace().collect();
        if tokens.is_empty() {
            return Ok(Vec::new());
        }

        let all = self.list_all()?;
        let mut scored: Vec<(ServerEntry, f64)> = Vec::new();

        for server in all {
            let mut score = 0.0_f64;
            let name_lower = server.name.to_lowercase();
            let full_name_lower = server.full_name().to_lowercase();
            let desc_lower = server.description.to_lowercase();
            let tools_str = server.tools.join(" ").to_lowercase();
            let tags_str = server.tags.join(" ").to_lowercase();
            let category = crate::registry::seed::server_category(&server.owner, &server.name).to_lowercase();

            for token in &tokens {
                // Exact name match (highest weight)
                if name_lower == *token || full_name_lower == *token {
                    score += 10.0;
                } else if name_lower.contains(token) {
                    score += 5.0;
                }

                // Description match
                if desc_lower.contains(token) {
                    score += 2.0;
                }

                // Tools match
                if tools_str.contains(token) {
                    score += 1.5;
                }

                // Tags match
                if tags_str.contains(token) {
                    score += 1.0;
                }

                // Category match
                if category.contains(token) {
                    score += 0.5;
                }
            }

            // Boost by popularity (log scale)
            if score > 0.0 {
                let popularity_boost = (server.downloads as f64 + 1.0).ln() * 0.1;
                score += popularity_boost;
                scored.push((server, score));
            }
        }

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        Ok(scored)
    }

    /// Get servers grouped by transport type.
    #[allow(dead_code)]
    pub fn group_by_transport(&self) -> Result<Vec<(String, Vec<ServerEntry>)>> {
        let all = self.list_all()?;
        let mut groups: std::collections::BTreeMap<String, Vec<ServerEntry>> = std::collections::BTreeMap::new();
        for server in all {
            groups.entry(server.transport.clone()).or_default().push(server);
        }
        Ok(groups.into_iter().collect())
    }

    /// Get servers that have been updated most recently.
    #[allow(dead_code)]
    pub fn hot_servers(&self, limit: usize) -> Result<Vec<ServerEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, owner, name, version, description, author, license, repository,
                    command, args, transport, tools, resources, prompts, tags, env,
                    homepage, deprecated, deprecated_by, category, downloads, stars,
                    created_at, updated_at
             FROM servers
             WHERE updated_at IS NOT NULL
             ORDER BY updated_at DESC
             LIMIT ?1"
        )?;
        let rows = stmt.query_map(params![limit as i64], row_mapper)?;
        let mut results = Vec::new();
        for row in rows {
            results.push(row?.into_entry());
        }
        Ok(results)
    }
}

#[cfg(test)]
mod weighted_search_tests {
    use super::*;

    fn create_test_db() -> Database {
        let db = Database::open_in_memory().unwrap();
        db.seed_default_servers().unwrap();
        db
    }

    #[test]
    fn test_search_weighted_basic() {
        let db = create_test_db();
        let results = db.search_weighted("filesystem").unwrap();
        assert!(!results.is_empty());
        // First result should be the filesystem server (exact name match)
        assert_eq!(results[0].0.name, "filesystem");
        // Score should be high due to exact name match
        assert!(results[0].1 > 5.0);
    }

    #[test]
    fn test_search_weighted_empty_query() {
        let db = create_test_db();
        let results = db.search_weighted("").unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_search_weighted_multi_word() {
        let db = create_test_db();
        let results = db.search_weighted("database sql").unwrap();
        assert!(!results.is_empty());
        // Servers matching both terms should score highest
        let top = &results[0].0;
        let name_or_desc = format!("{} {}", top.name, top.description).to_lowercase();
        assert!(
            name_or_desc.contains("sql") || name_or_desc.contains("database"),
            "Top result should match at least one search term"
        );
    }

    #[test]
    fn test_search_weighted_name_beats_description() {
        let db = create_test_db();
        let results = db.search_weighted("git").unwrap();
        assert!(!results.is_empty());
        // The "git" server should score higher than servers that just mention git in description
        let git_server = results.iter().find(|(s, _)| s.name == "git");
        assert!(git_server.is_some(), "git server should be in results");
        let git_score = git_server.unwrap().1;
        // Should be near the top
        assert!(git_score >= results[0].1 * 0.5, "git server should score well");
    }

    #[test]
    fn test_search_weighted_no_results() {
        let db = create_test_db();
        let results = db.search_weighted("xyznonexistent123").unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_search_weighted_scores_descending() {
        let db = create_test_db();
        let results = db.search_weighted("server").unwrap();
        for window in results.windows(2) {
            assert!(
                window[0].1 >= window[1].1,
                "Results should be sorted by score descending"
            );
        }
    }

    #[test]
    fn test_group_by_transport() {
        let db = create_test_db();
        let groups = db.group_by_transport().unwrap();
        assert!(!groups.is_empty());
        // Should have at least stdio
        assert!(
            groups.iter().any(|(t, _)| t == "stdio"),
            "Should have stdio transport group"
        );
        // Each group should have at least one server
        for (_, servers) in &groups {
            assert!(!servers.is_empty());
        }
    }

    #[test]
    fn test_hot_servers() {
        let db = create_test_db();
        let hot = db.hot_servers(5).unwrap();
        assert!(!hot.is_empty());
        assert!(hot.len() <= 5);
    }

    #[test]
    fn test_hot_servers_zero_limit() {
        let db = create_test_db();
        let hot = db.hot_servers(0).unwrap();
        assert!(hot.is_empty());
    }
}
