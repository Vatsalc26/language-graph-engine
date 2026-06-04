use crate::error::Error;
use rusqlite::Connection;

pub fn run_migrations(conn: &Connection) -> Result<(), Error> {
    // Enable foreign keys
    conn.execute("PRAGMA foreign_keys = ON;", [])?;

    // Create tables in order
    conn.execute(
        "CREATE TABLE IF NOT EXISTS immutable_blocks (
            cid TEXT PRIMARY KEY,
            codec TEXT NOT NULL,
            multihash_algorithm TEXT NOT NULL,
            block_kind TEXT NOT NULL,
            bytes BLOB NOT NULL,
            stored_at TEXT NOT NULL
        );",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS entities (
            entity_id TEXT PRIMARY KEY,
            kind TEXT NOT NULL,
            canonical_key TEXT NOT NULL UNIQUE,
            label TEXT NOT NULL,
            created_at TEXT NOT NULL
        );",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS entity_heads (
            entity_id TEXT PRIMARY KEY,
            revision_cid TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            FOREIGN KEY (entity_id) REFERENCES entities(entity_id),
            FOREIGN KEY (revision_cid) REFERENCES immutable_blocks(cid)
        );",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS collections (
            collection_entity_id TEXT PRIMARY KEY,
            canonical_key TEXT NOT NULL UNIQUE,
            label TEXT NOT NULL,
            created_at TEXT NOT NULL
        );",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS collection_snapshots (
            snapshot_cid TEXT PRIMARY KEY,
            collection_entity_id TEXT NOT NULL,
            created_at TEXT NOT NULL,
            FOREIGN KEY (snapshot_cid) REFERENCES immutable_blocks(cid),
            FOREIGN KEY (collection_entity_id) REFERENCES collections(collection_entity_id)
        );",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS collection_snapshot_members (
            snapshot_cid TEXT NOT NULL,
            position INTEGER NOT NULL,
            entity_id TEXT NOT NULL,
            revision_cid TEXT NOT NULL,
            PRIMARY KEY (snapshot_cid, position),
            UNIQUE (snapshot_cid, entity_id),
            FOREIGN KEY (snapshot_cid) REFERENCES collection_snapshots(snapshot_cid),
            FOREIGN KEY (entity_id) REFERENCES entities(entity_id),
            FOREIGN KEY (revision_cid) REFERENCES immutable_blocks(cid)
        );",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS active_collection_snapshots (
            collection_entity_id TEXT PRIMARY KEY,
            snapshot_cid TEXT NOT NULL,
            activated_at TEXT NOT NULL,
            FOREIGN KEY (collection_entity_id) REFERENCES collections(collection_entity_id),
            FOREIGN KEY (snapshot_cid) REFERENCES collection_snapshots(snapshot_cid)
        );",
        [],
    )?;

    Ok(())
}
