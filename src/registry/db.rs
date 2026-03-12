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
                category TEXT NOT NULL DEFAULT '',
                downloads INTEGER NOT NULL DEFAULT 0,
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
        let category = crate::registry::seed::server_category(&entry.owner, &entry.name);

        self.conn.execute(
            "INSERT INTO servers (owner, name, version, description, author, license, repository, command, args, transport, tools, resources, prompts, tags, category, downloads)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)
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
        let terms: Vec<&str> = query.split_whitespace().collect();

        if terms.is_empty() {
            // Empty query: return all, ordered by downloads
            return self.list_servers(1, 50).map(|(v, _)| v);
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
                    command, args, transport, tools, resources, prompts, tags, category, downloads, created_at, updated_at,
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
                    command, args, transport, tools, resources, prompts, tags, category, downloads, created_at, updated_at
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
                    command, args, transport, tools, resources, prompts, tags, category, downloads, created_at, updated_at
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
                    command, args, transport, tools, resources, prompts, tags, category, downloads, created_at, updated_at
             FROM servers ORDER BY downloads DESC LIMIT ?1 OFFSET ?2",
        )?;
        let rows = stmt.query_map(params![per_page as i64, offset as i64], row_mapper)?;

        let mut entries = Vec::new();
        for row in rows {
            entries.push(row?.into_entry());
        }
        Ok((entries, total))
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
        category: row.get(15)?,
        downloads: row.get(16)?,
        created_at: row.get(17)?,
        updated_at: row.get(18)?,
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
    #[allow(dead_code)]
    category: String,
    downloads: i64,
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
            downloads: self.downloads,
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
            downloads: 0,
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
    /// Search servers that have a specific tag (case-insensitive substring match).
    #[allow(dead_code)]
    pub fn search_by_tags(&self, tag: &str) -> Result<Vec<ServerEntry>> {
        let escaped = escape_like(tag);
        let pattern = format!("%{escaped}%");
        let mut stmt = self.conn.prepare(
            "SELECT id, owner, name, version, description, author, license, repository,
                    command, args, transport, tools, resources, prompts, tags, category, downloads, created_at, updated_at
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
            downloads,
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
            downloads: 0,
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
            downloads: 0,
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
            downloads: 0,
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
            downloads: 0,
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
            downloads: 0,
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
            downloads,
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
            downloads: 0,
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
