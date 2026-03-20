use anyhow::Result;
use rusqlite::Connection;
use std::path::Path;
use std::sync::Mutex;

pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    pub fn open(path: &str) -> Result<Self> {
        // Ensure parent directory exists
        if let Some(parent) = Path::new(path).parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(path)?;

        // Enable WAL mode for concurrent reads
        conn.execute_batch("PRAGMA journal_mode=WAL;")?;
        conn.execute_batch("PRAGMA foreign_keys=ON;")?;

        let db = Self { conn: Mutex::new(conn) };
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS clients (
                client_id TEXT PRIMARY KEY,
                client_name TEXT,
                public_key_pem TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'pending',
                registered_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS chunks (
                id TEXT PRIMARY KEY,
                path TEXT NOT NULL,
                title TEXT NOT NULL,
                content TEXT NOT NULL,
                chunk_index INTEGER NOT NULL,
                embedding BLOB,
                source_type TEXT NOT NULL DEFAULT 'local_file',
                content_type TEXT,
                file_size INTEGER,
                modified_at TEXT,
                indexed_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS nodes (
                node_id TEXT PRIMARY KEY,
                host TEXT NOT NULL,
                port INTEGER NOT NULL,
                endpoint TEXT NOT NULL,
                capabilities TEXT,
                last_seen TEXT NOT NULL,
                healthy INTEGER NOT NULL DEFAULT 1
            );

            CREATE TABLE IF NOT EXISTS tasks (
                task_id TEXT PRIMARY KEY,
                capability_id TEXT NOT NULL,
                client_id TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'queued',
                input TEXT NOT NULL,
                result TEXT,
                error TEXT,
                progress INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_chunks_path ON chunks(path);
            CREATE INDEX IF NOT EXISTS idx_tasks_client ON tasks(client_id);
            CREATE INDEX IF NOT EXISTS idx_tasks_status ON tasks(status);
            "
        )?;
        Ok(())
    }

    // --- Client operations ---

    pub fn register_client(&self, client_id: &str, client_name: &str, public_key_pem: &str, status: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO clients (client_id, client_name, public_key_pem, status, registered_at)
             VALUES (?1, ?2, ?3, ?4, datetime('now'))",
            rusqlite::params![client_id, client_name, public_key_pem, status],
        )?;
        Ok(())
    }

    pub fn get_client(&self, client_id: &str) -> Result<Option<(String, String, String)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT client_name, public_key_pem, status FROM clients WHERE client_id = ?1"
        )?;
        let result = stmt.query_row(rusqlite::params![client_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?))
        });
        match result {
            Ok(row) => Ok(Some(row)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn approve_client(&self, client_id: &str) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        let rows = conn.execute(
            "UPDATE clients SET status = 'approved' WHERE client_id = ?1 AND status = 'pending'",
            rusqlite::params![client_id],
        )?;
        Ok(rows > 0)
    }

    pub fn reject_client(&self, client_id: &str) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        let rows = conn.execute(
            "UPDATE clients SET status = 'rejected' WHERE client_id = ?1",
            rusqlite::params![client_id],
        )?;
        Ok(rows > 0)
    }

    pub fn list_pending_clients(&self) -> Result<Vec<(String, String, String)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT client_id, client_name, registered_at FROM clients WHERE status = 'pending'"
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?))
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    // --- Chunk operations ---

    #[allow(clippy::too_many_arguments)]
    pub fn insert_chunk(
        &self,
        id: &str,
        path: &str,
        title: &str,
        content: &str,
        chunk_index: i32,
        embedding: &[u8],
        content_type: Option<&str>,
        file_size: Option<i64>,
        modified_at: Option<&str>,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO chunks (id, path, title, content, chunk_index, embedding, content_type, file_size, modified_at, indexed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, datetime('now'))",
            rusqlite::params![id, path, title, content, chunk_index, embedding, content_type, file_size, modified_at],
        )?;
        Ok(())
    }

    pub fn get_all_embeddings(&self) -> Result<Vec<(String, Vec<u8>, String, String, String)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, embedding, title, content, path FROM chunks WHERE embedding IS NOT NULL"
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Vec<u8>>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
            ))
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn chunk_count(&self) -> Result<usize> {
        let conn = self.conn.lock().unwrap();
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM chunks", [], |row| row.get(0))?;
        Ok(count as usize)
    }

    // --- Node operations ---

    pub fn upsert_node(&self, node_id: &str, host: &str, port: u16, endpoint: &str, capabilities: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO nodes (node_id, host, port, endpoint, capabilities, last_seen, healthy)
             VALUES (?1, ?2, ?3, ?4, ?5, datetime('now'), 1)",
            rusqlite::params![node_id, host, port, endpoint, capabilities],
        )?;
        Ok(())
    }

    pub fn list_healthy_nodes(&self) -> Result<Vec<(String, String, u16, String)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT node_id, host, port, endpoint FROM nodes WHERE healthy = 1"
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, u16>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    // --- Task operations (spec improvement #6: task persistence) ---

    pub fn insert_task(&self, task_id: &str, capability_id: &str, client_id: &str, input: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO tasks (task_id, capability_id, client_id, status, input, progress, created_at, updated_at)
             VALUES (?1, ?2, ?3, 'queued', ?4, 0, datetime('now'), datetime('now'))",
            rusqlite::params![task_id, capability_id, client_id, input],
        )?;
        Ok(())
    }

    pub fn update_task_status(&self, task_id: &str, status: &str, result: Option<&str>, error: Option<&str>, progress: i32) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE tasks SET status = ?2, result = ?3, error = ?4, progress = ?5, updated_at = datetime('now')
             WHERE task_id = ?1",
            rusqlite::params![task_id, status, result, error, progress],
        )?;
        Ok(())
    }

    pub fn get_task(&self, task_id: &str) -> Result<Option<TaskRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT task_id, capability_id, client_id, status, input, result, error, progress, created_at, updated_at
             FROM tasks WHERE task_id = ?1"
        )?;
        let result = stmt.query_row(rusqlite::params![task_id], |row| {
            Ok(TaskRow {
                task_id: row.get(0)?,
                capability_id: row.get(1)?,
                client_id: row.get(2)?,
                status: row.get(3)?,
                input: row.get(4)?,
                result: row.get(5)?,
                error: row.get(6)?,
                progress: row.get(7)?,
                created_at: row.get(8)?,
                updated_at: row.get(9)?,
            })
        });
        match result {
            Ok(row) => Ok(Some(row)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
}

#[derive(Debug)]
pub struct TaskRow {
    pub task_id: String,
    pub capability_id: String,
    pub client_id: String,
    pub status: String,
    pub input: String,
    pub result: Option<String>,
    pub error: Option<String>,
    pub progress: i32,
    pub created_at: String,
    pub updated_at: String,
}
