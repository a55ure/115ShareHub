pub mod migration;
pub mod models;
pub mod repository;

use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::MutexGuard;

#[derive(Clone)]
pub struct Database {
    conn: Arc<Mutex<rusqlite::Connection>>,
}

impl Database {
    pub fn new(path: &Path) -> Result<Self, rusqlite::Error> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let conn = rusqlite::Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON;")?;
        Ok(Database {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn run_migrations(&self) -> Result<(), rusqlite::Error> {
        let conn = self.get_conn();
        migration::run_migrations(&conn)
    }

    pub fn get_conn(&self) -> MutexGuard<'_, rusqlite::Connection> {
        self.conn.lock().expect("database connection lock poisoned")
    }
}
