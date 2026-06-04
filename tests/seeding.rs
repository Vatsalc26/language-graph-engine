use rusqlite::Connection;
use language_graph_engine::db::migrations::run_migrations;
use language_graph_engine::seed::lowercase_latin::{seed_lowercase_latin, COLLECTION_ENTITY_ID};

#[test]
fn test_initial_seeding_correctness() {
    let mut conn = Connection::open_in_memory().expect("Failed to open in-memory SQLite");
    run_migrations(&conn).expect("Failed to run migrations");

    // Run seeding
    let snap_cid = seed_lowercase_latin(&mut conn).expect("First seeding failed");
    assert!(!snap_cid.is_empty(), "Snapshot CID is empty");

    // 1. Initial seeding creates exactly 26 stable grapheme entities
    let entity_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM entities WHERE kind = 'grapheme'",
        [],
        |row| row.get(0)
    ).unwrap();
    assert_eq!(entity_count, 26, "Did not seed exactly 26 entities");

    // 2. Initial seeding creates exactly 26 current grapheme revision heads
    let head_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM entity_heads",
        [],
        |row| row.get(0)
    ).unwrap();
    assert_eq!(head_count, 26, "Did not seed exactly 26 entity heads");

    // 3. Initial seeding creates one active lowercase alphabet snapshot
    let active_snap_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM active_collection_snapshots WHERE collection_entity_id = ?1",
        [COLLECTION_ENTITY_ID],
        |row| row.get(0)
    ).unwrap();
    assert_eq!(active_snap_count, 1, "Did not seed exactly 1 active collection snapshot");

    // 4. Running seeding twice is idempotent and does not duplicate entities or snapshots
    let snap_cid_second = seed_lowercase_latin(&mut conn).expect("Second seeding failed");
    assert_eq!(snap_cid, snap_cid_second, "Second seeding produced a different snapshot CID!");

    let entity_count_2: i64 = conn.query_row(
        "SELECT COUNT(*) FROM entities WHERE kind = 'grapheme'",
        [],
        |row| row.get(0)
    ).unwrap();
    assert_eq!(entity_count_2, 26, "Entity count changed after second seeding");

    let snap_count_2: i64 = conn.query_row(
        "SELECT COUNT(*) FROM collection_snapshots WHERE collection_entity_id = ?1",
        [COLLECTION_ENTITY_ID],
        |row| row.get(0)
    ).unwrap();
    assert_eq!(snap_count_2, 1, "Snapshot count changed after second seeding");
}

#[test]
fn test_seeding_integrity_error_on_conflict() {
    let mut conn = Connection::open_in_memory().expect("Failed to open in-memory SQLite");
    run_migrations(&conn).expect("Failed to run migrations");

    // Manually insert a conflicting entity for 'a' before seeding
    conn.execute(
        "INSERT INTO entities (entity_id, kind, canonical_key, label, created_at)
         VALUES ('urn:language-graph:grapheme:nfc:0061', 'grapheme', 'a', 'conflicting label', datetime('now'))",
        [],
    ).unwrap();

    // Now seeding should fail due to conflicting entity label for 'a'
    let result = seed_lowercase_latin(&mut conn);
    assert!(result.is_err(), "Seeding should have failed due to conflicting entity label");
    let err_msg = format!("{:?}", result.err().unwrap());
    assert!(err_msg.contains("Conflicting label for entity"), "Error message was: {}", err_msg);
}

#[test]
fn test_seeding_integrity_error_on_head_conflict() {
    let mut conn = Connection::open_in_memory().expect("Failed to open in-memory SQLite");
    run_migrations(&conn).expect("Failed to run migrations");

    // Seed first
    seed_lowercase_latin(&mut conn).expect("Initial seeding failed");

    // Manually update the head of 'a' to a different CID
    // First, let's create a fake block so the foreign key constraint is satisfied
    conn.execute(
        "INSERT INTO immutable_blocks (cid, codec, multihash_algorithm, block_kind, bytes, stored_at)
         VALUES ('bagybeifakecid123', 'dag-cbor', 'sha2-256', 'grapheme_revision', x'00', datetime('now'))",
        [],
    ).unwrap();

    // Now disable foreign key check temporarily or use the fake CID
    conn.execute(
        "UPDATE entity_heads SET revision_cid = 'bagybeifakecid123' WHERE entity_id = 'urn:language-graph:grapheme:nfc:0061'",
        [],
    ).unwrap();

    // Seeding again should fail because the head of 'a' has a conflicting value
    let result = seed_lowercase_latin(&mut conn);
    assert!(result.is_err(), "Seeding should have failed due to conflicting head");
    let err_msg = format!("{:?}", result.err().unwrap());
    assert!(err_msg.contains("Conflicting head revision for entity"), "Error message was: {}", err_msg);
}
