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

        Ok(())
    }

    pub fn upsert_server(&self, entry: &ServerEntry) -> Result<i64> {
        let args_json = serde_json::to_string(&entry.args)?;
        let tools_json = serde_json::to_string(&entry.tools)?;
        let resources_json = serde_json::to_string(&entry.resources)?;
        let prompts_json = serde_json::to_string(&entry.prompts)?;
        let category = crate::registry::seed::server_category(&entry.owner, &entry.name);

        self.conn.execute(
            "INSERT INTO servers (owner, name, version, description, author, license, repository, command, args, transport, tools, resources, prompts, category, downloads)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
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
                category,
                entry.downloads,
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
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
            let pattern = format!("%{term}%");
            let idx = param_values.len();
            param_values.push(pattern);
            conditions.push(format!(
                "(name LIKE ?{p} OR description LIKE ?{p} OR owner LIKE ?{p} OR tools LIKE ?{p} OR author LIKE ?{p} OR category LIKE ?{p})",
                p = idx + 1
            ));
        }

        let where_clause = conditions.join(" AND ");

        // Relevance scoring uses only the first term for simplicity
        let first_pattern = format!("%{}%", terms[0]);
        param_values.push(first_pattern);
        let score_idx = param_values.len();

        let sql = format!(
            "SELECT id, owner, name, version, description, author, license, repository,
                    command, args, transport, tools, resources, prompts, category, downloads, created_at, updated_at,
                    (CASE WHEN name LIKE ?{s} THEN 100 ELSE 0 END
                     + CASE WHEN owner LIKE ?{s} THEN 50 ELSE 0 END
                     + CASE WHEN tools LIKE ?{s} THEN 30 ELSE 0 END
                     + CASE WHEN description LIKE ?{s} THEN 10 ELSE 0 END
                     + downloads / 100) AS relevance
             FROM servers
             WHERE {where_clause}
             ORDER BY relevance DESC, downloads DESC
             LIMIT 50",
            s = score_idx,
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
        let pattern = format!("%{category}%");
        let mut stmt = self.conn.prepare(
            "SELECT id, owner, name, version, description, author, license, repository,
                    command, args, transport, tools, resources, prompts, category, downloads, created_at, updated_at
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
                    command, args, transport, tools, resources, prompts, category, downloads, created_at, updated_at
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
                    command, args, transport, tools, resources, prompts, category, downloads, created_at, updated_at
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
        category: row.get(14)?,
        downloads: row.get(15)?,
        created_at: row.get(16)?,
        updated_at: row.get(17)?,
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
    fn test_search_category_column() {
        let db = Database::open_in_memory().unwrap();
        // "filesystem" should get categorized as Files & VCS
        db.upsert_server(&make_entry("org", "filesystem", vec![], 0)).unwrap();
        let results = db.search("Files").unwrap();
        assert!(!results.is_empty(), "Category column should be searchable");
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
