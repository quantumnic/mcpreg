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
                downloads INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now')),
                UNIQUE(owner, name)
            );

            CREATE INDEX IF NOT EXISTS idx_servers_owner ON servers(owner);
            CREATE INDEX IF NOT EXISTS idx_servers_name ON servers(name);
            CREATE INDEX IF NOT EXISTS idx_servers_downloads ON servers(downloads DESC);

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

    pub fn upsert_server(&self, entry: &ServerEntry) -> Result<i64> {
        let args_json = serde_json::to_string(&entry.args)?;
        let tools_json = serde_json::to_string(&entry.tools)?;
        let resources_json = serde_json::to_string(&entry.resources)?;

        self.conn.execute(
            "INSERT INTO servers (owner, name, version, description, author, license, repository, command, args, transport, tools, resources)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
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
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn search(&self, query: &str) -> Result<Vec<ServerEntry>> {
        let pattern = format!("%{query}%");
        let mut stmt = self.conn.prepare(
            "SELECT id, owner, name, version, description, author, license, repository,
                    command, args, transport, tools, resources, downloads, created_at, updated_at
             FROM servers
             WHERE name LIKE ?1 OR description LIKE ?1 OR owner LIKE ?1
             ORDER BY downloads DESC
             LIMIT 50",
        )?;
        let rows = stmt.query_map(params![pattern], |row| {
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
                downloads: row.get(13)?,
                created_at: row.get(14)?,
                updated_at: row.get(15)?,
            })
        })?;

        let mut entries = Vec::new();
        for row in rows {
            let r = row?;
            entries.push(r.into_entry());
        }
        Ok(entries)
    }

    pub fn get_server(&self, owner: &str, name: &str) -> Result<Option<ServerEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, owner, name, version, description, author, license, repository,
                    command, args, transport, tools, resources, downloads, created_at, updated_at
             FROM servers WHERE owner = ?1 AND name = ?2",
        )?;
        let mut rows = stmt.query_map(params![owner, name], |row| {
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
                downloads: row.get(13)?,
                created_at: row.get(14)?,
                updated_at: row.get(15)?,
            })
        })?;

        match rows.next() {
            Some(Ok(r)) => Ok(Some(r.into_entry())),
            Some(Err(e)) => Err(e.into()),
            None => Ok(None),
        }
    }

    pub fn list_servers(&self, page: usize, per_page: usize) -> Result<(Vec<ServerEntry>, usize)> {
        let offset = (page.saturating_sub(1)) * per_page;

        let total: usize = self.conn.query_row("SELECT COUNT(*) FROM servers", [], |row| row.get(0))?;

        let mut stmt = self.conn.prepare(
            "SELECT id, owner, name, version, description, author, license, repository,
                    command, args, transport, tools, resources, downloads, created_at, updated_at
             FROM servers ORDER BY downloads DESC LIMIT ?1 OFFSET ?2",
        )?;
        let rows = stmt.query_map(params![per_page as i64, offset as i64], |row| {
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
                downloads: row.get(13)?,
                created_at: row.get(14)?,
                updated_at: row.get(15)?,
            })
        })?;

        let mut entries = Vec::new();
        for row in rows {
            let r = row?;
            entries.push(r.into_entry());
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

    #[allow(dead_code)]
    pub fn increment_downloads(&self, owner: &str, name: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE servers SET downloads = downloads + 1 WHERE owner = ?1 AND name = ?2",
            params![owner, name],
        )?;
        Ok(())
    }
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

        let results = db.search("file").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "filesystem");

        let results = db.search("server").unwrap();
        assert!(results.iter().any(|s| s.name == "sqlite-server"));
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
        db.increment_downloads("alice", "tool").unwrap();
        db.increment_downloads("alice", "tool").unwrap();
        let server = db.get_server("alice", "tool").unwrap().unwrap();
        assert_eq!(server.downloads, 2);
    }

    #[test]
    fn test_db_not_found() {
        let db = Database::open_in_memory().unwrap();
        let result = db.get_server("nobody", "nothing").unwrap();
        assert!(result.is_none());
    }
}
