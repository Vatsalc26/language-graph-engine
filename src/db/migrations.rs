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

    conn.execute(
        "CREATE TABLE IF NOT EXISTS text_profiles (
            profile_entity_id TEXT PRIMARY KEY,
            canonical_key TEXT NOT NULL UNIQUE,
            label TEXT NOT NULL,
            created_at TEXT NOT NULL
        );",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS text_profile_snapshots (
            snapshot_cid TEXT PRIMARY KEY,
            profile_entity_id TEXT NOT NULL,
            created_at TEXT NOT NULL,
            FOREIGN KEY (snapshot_cid) REFERENCES immutable_blocks(cid),
            FOREIGN KEY (profile_entity_id) REFERENCES text_profiles(profile_entity_id)
        );",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS text_profile_snapshot_collections (
            profile_snapshot_cid TEXT NOT NULL,
            position INTEGER NOT NULL,
            collection_entity_id TEXT NOT NULL,
            collection_snapshot_cid TEXT NOT NULL,
            PRIMARY KEY (profile_snapshot_cid, position),
            UNIQUE (profile_snapshot_cid, collection_entity_id),
            FOREIGN KEY (profile_snapshot_cid) REFERENCES text_profile_snapshots(snapshot_cid),
            FOREIGN KEY (collection_entity_id) REFERENCES collections(collection_entity_id),
            FOREIGN KEY (collection_snapshot_cid) REFERENCES collection_snapshots(snapshot_cid)
        );",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS active_text_profile_snapshots (
            profile_entity_id TEXT PRIMARY KEY,
            snapshot_cid TEXT NOT NULL,
            activated_at TEXT NOT NULL,
            FOREIGN KEY (profile_entity_id) REFERENCES text_profiles(profile_entity_id),
            FOREIGN KEY (snapshot_cid) REFERENCES text_profile_snapshots(snapshot_cid)
        );",
        [],
    )?;

    // Phase 3 Schema Additions
    conn.execute(
        "CREATE TABLE IF NOT EXISTS written_forms (
            entity_id TEXT PRIMARY KEY,
            surface_form TEXT NOT NULL,
            normalized_surface_form TEXT NOT NULL COLLATE BINARY UNIQUE,
            form_class TEXT NOT NULL,
            created_at TEXT NOT NULL,
            FOREIGN KEY (entity_id) REFERENCES entities(entity_id)
        );",
        [],
    )?;

    conn.execute(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_written_forms_normalized_surface
         ON written_forms(normalized_surface_form COLLATE BINARY);",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS written_form_components (
            written_form_revision_cid TEXT NOT NULL,
            position INTEGER NOT NULL,
            symbol_entity_id TEXT NOT NULL,
            symbol_revision_cid TEXT NOT NULL,
            surface_form TEXT NOT NULL,
            PRIMARY KEY (written_form_revision_cid, position),
            FOREIGN KEY (written_form_revision_cid) REFERENCES immutable_blocks(cid),
            FOREIGN KEY (symbol_entity_id) REFERENCES entities(entity_id),
            FOREIGN KEY (symbol_revision_cid) REFERENCES immutable_blocks(cid)
        );",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS written_form_stores (
            store_entity_id TEXT PRIMARY KEY,
            canonical_key TEXT NOT NULL UNIQUE,
            label TEXT NOT NULL,
            store_kind TEXT NOT NULL,
            admission_policy TEXT NOT NULL,
            created_at TEXT NOT NULL
        );",
        [],
    )?;

    // Seed exactly one initial store metadata row
    conn.execute(
        "INSERT OR IGNORE INTO written_form_stores (store_entity_id, canonical_key, label, store_kind, admission_policy, created_at)
         VALUES (
             'urn:language-graph:store:english-natural-language-written-forms',
             'english-natural-language-written-forms',
             'English Natural-Language Written Forms',
             'written-form-store',
             'ascii-letters-with-internal-apostrophe-or-hyphen-v1',
             datetime('now')
         );",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS written_form_store_members (
            store_entity_id TEXT NOT NULL,
            written_form_entity_id TEXT NOT NULL,
            admitted_revision_cid TEXT NOT NULL,
            added_at TEXT NOT NULL,
            status TEXT NOT NULL,
            PRIMARY KEY (store_entity_id, written_form_entity_id),
            FOREIGN KEY (store_entity_id) REFERENCES written_form_stores(store_entity_id),
            FOREIGN KEY (written_form_entity_id) REFERENCES written_forms(entity_id),
            FOREIGN KEY (admitted_revision_cid) REFERENCES immutable_blocks(cid)
        );",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS written_form_store_snapshots (
            snapshot_cid TEXT PRIMARY KEY,
            store_entity_id TEXT NOT NULL,
            created_at TEXT NOT NULL,
            FOREIGN KEY (snapshot_cid) REFERENCES immutable_blocks(cid),
            FOREIGN KEY (store_entity_id) REFERENCES written_form_stores(store_entity_id)
        );",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS written_form_store_snapshot_members (
            snapshot_cid TEXT NOT NULL,
            position INTEGER NOT NULL,
            written_form_entity_id TEXT NOT NULL,
            written_form_revision_cid TEXT NOT NULL,
            PRIMARY KEY (snapshot_cid, position),
            UNIQUE (snapshot_cid, written_form_entity_id),
            FOREIGN KEY (snapshot_cid) REFERENCES written_form_store_snapshots(snapshot_cid),
            FOREIGN KEY (written_form_entity_id) REFERENCES written_forms(entity_id),
            FOREIGN KEY (written_form_revision_cid) REFERENCES immutable_blocks(cid)
        );",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS active_written_form_store_snapshots (
            store_entity_id TEXT PRIMARY KEY,
            snapshot_cid TEXT NOT NULL,
            activated_at TEXT NOT NULL,
            FOREIGN KEY (store_entity_id) REFERENCES written_form_stores(store_entity_id),
            FOREIGN KEY (snapshot_cid) REFERENCES written_form_store_snapshots(snapshot_cid)
        );",
        [],
    )?;

    Ok(())
}
