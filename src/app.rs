use std::sync::{Arc, Mutex};
use rusqlite::Connection;
use crate::config::Config;
use crate::error::Error;
use crate::db::migrations::run_migrations;
use crate::seed::lowercase_latin::seed_lowercase_latin;
use crate::resolver::text::TextResolver;

pub struct AppStateInner {
    pub conn: Connection,
    pub resolver: TextResolver,
    pub config: Config,
}

#[derive(Clone)]
pub struct AppState(pub Arc<Mutex<AppStateInner>>);

impl AppState {
    pub fn new(config: Config) -> Result<Self, Error> {
        // Ensure data directory exists
        if let Some(parent) = config.db_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| Error::NotFoundError(format!("Failed to create DB directory: {:?}", e)))?;
        }

        let mut conn = Connection::open(&config.db_path)?;
        
        // Run migrations
        run_migrations(&conn)?;

        // Seed data
        let active_snap_cid = seed_lowercase_latin(&mut conn)?;
        println!("Database successfully seeded. Active snapshot CID: {}", active_snap_cid);

        // Load resolver
        let resolver = TextResolver::load(&conn)?;

        Ok(Self(Arc::new(Mutex::new(AppStateInner {
            conn,
            resolver,
            config,
        }))))
    }
}
