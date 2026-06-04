use language_graph_engine::db::migrations::run_migrations;
use language_graph_engine::db::repository::Repository;
use language_graph_engine::error::Error;
use language_graph_engine::seed::lowercase_latin::{seed_lowercase_latin, COLLECTION_ENTITY_ID};
use rusqlite::Connection;

fn get_temp_db() -> Connection {
    let conn = Connection::open_in_memory().expect("Failed to open in-memory SQLite");
    run_migrations(&conn).expect("Failed to run migrations");
    conn
}

#[test]
fn test_db_migration_succeeds() {
    let conn = Connection::open_in_memory().expect("Open db");
    let res = run_migrations(&conn);
    assert!(res.is_ok(), "Migrations failed: {:?}", res.err());

    // Verify tables exist
    let tables = vec![
        "immutable_blocks",
        "entities",
        "entity_heads",
        "collections",
        "collection_snapshots",
        "collection_snapshot_members",
        "active_collection_snapshots",
    ];

    for table in tables {
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
                [table],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "Table {} does not exist", table);
    }
}

#[test]
fn test_foreign_key_enforcement() {
    let conn = get_temp_db();

    // Insert an entity head pointing to a non-existent entity and revision CID
    let res = conn.execute(
        "INSERT INTO entity_heads (entity_id, revision_cid, updated_at) 
         VALUES ('urn:fake-entity', 'bafyreibfake', datetime('now'))",
        [],
    );
    assert!(
        res.is_err(),
        "Foreign key constraint should have failed for entity_heads insert"
    );
}

#[test]
fn test_initial_seeding_metrics() {
    let mut conn = get_temp_db();
    let snap_cid = seed_lowercase_latin(&mut conn).expect("Seeding failed");

    let repo = Repository::new(&conn);

    // 3. Initial seeding creates exactly 26 stable grapheme entities
    let entity_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM entities WHERE kind='grapheme'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(entity_count, 26);

    // 4. Initial seeding creates exactly 26 active seeded revision heads
    let head_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM entity_heads", [], |row| row.get(0))
        .unwrap();
    assert_eq!(head_count, 26);

    // 5. Initial seeding creates expected revision blocks (26 revisions + 1 snapshot = 27 blocks)
    let block_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM immutable_blocks", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(block_count, 27);

    // 6. Initial seeding creates exactly one lowercase alphabet collection entity
    let coll_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM collections", [], |row| row.get(0))
        .unwrap();
    assert_eq!(coll_count, 1);

    // 7. Initial seeding creates exactly one active published alphabet snapshot
    let active_snap = repo
        .get_active_snapshot_cid(COLLECTION_ENTITY_ID)
        .unwrap()
        .expect("Active snapshot CID");
    assert_eq!(active_snap, snap_cid);

    // 8. The active snapshot has exactly 26 ordered members
    let members = repo.get_snapshot_members(&snap_cid).unwrap();
    assert_eq!(members.len(), 26);

    // 9. The ordered members correspond exactly to a through z
    for i in 0..26 {
        let ch = (b'a' + i) as char;
        let expected_entity = format!("urn:language-graph:grapheme:nfc:{:04x}", ch as u32);
        assert_eq!(members[i as usize].position, (i + 1) as i32);
        assert_eq!(members[i as usize].entity_id, expected_entity);
    }
}

#[test]
fn test_seeding_idempotency() {
    let mut conn = get_temp_db();
    let snap_cid1 = seed_lowercase_latin(&mut conn).expect("First seed");
    let snap_cid2 = seed_lowercase_latin(&mut conn).expect("Second seed");
    assert_eq!(
        snap_cid1, snap_cid2,
        "Idempotent seeding must return identical snapshot CID"
    );

    // Assert counts are unchanged
    let entity_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM entities", [], |row| row.get(0))
        .unwrap();
    assert_eq!(entity_count, 26);

    let head_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM entity_heads", [], |row| row.get(0))
        .unwrap();
    assert_eq!(head_count, 26);

    let block_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM immutable_blocks", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(block_count, 27);
}

#[test]
fn test_databases_consistency() {
    let mut conn1 = get_temp_db();
    let mut conn2 = get_temp_db();

    let snap_cid1 = seed_lowercase_latin(&mut conn1).unwrap();
    let snap_cid2 = seed_lowercase_latin(&mut conn2).unwrap();

    assert_eq!(
        snap_cid1, snap_cid2,
        "Independent databases must yield identical snapshot CID"
    );
}

#[test]
fn test_database_tampering_integrity_error() {
    let mut conn = get_temp_db();
    let _snap_cid = seed_lowercase_latin(&mut conn).unwrap();

    let repo = Repository::new(&conn);
    let members = repo.get_snapshot_members(_snap_cid.as_str()).unwrap();
    let rev_a_cid = &members[0].revision_cid;

    // Tamper with the bytes of 'a' block in the database
    conn.execute(
        "UPDATE immutable_blocks SET bytes = x'123456' WHERE cid = ?1",
        [rev_a_cid],
    )
    .unwrap();

    // Now attempting to retrieve grapheme revision 'a' must trigger integrity check error
    let res = repo.get_grapheme_revision(rev_a_cid);
    assert!(res.is_err());
    let err_msg = format!("{:?}", res.err().unwrap());
    assert!(
        err_msg.contains("Integrity check failed"),
        "Error: {}",
        err_msg
    );
}

#[test]
fn test_conflicting_canonical_entity_error() {
    let mut conn = get_temp_db();

    // Insert entity for 'a' with conflicting canonical_key or label
    conn.execute(
        "INSERT INTO entities (entity_id, kind, canonical_key, label, created_at)
         VALUES ('urn:language-graph:grapheme:nfc:0061', 'grapheme', 'a', 'Conflicting Label', datetime('now'))",
        [],
    ).unwrap();

    let res = seed_lowercase_latin(&mut conn);
    assert!(
        res.is_err(),
        "Seeding must fail due to conflicting canonical entity info"
    );
    assert!(matches!(res.unwrap_err(), Error::IntegrityError(_)));
}

#[test]
fn test_transaction_safety_on_seeding_failure() {
    let mut conn = get_temp_db();

    // Pre-insert a conflict for letter 'm'
    conn.execute(
        "INSERT INTO entities (entity_id, kind, canonical_key, label, created_at)
         VALUES ('urn:language-graph:grapheme:nfc:006d', 'grapheme', 'm', 'Conflict on M', datetime('now'))",
        [],
    ).unwrap();

    // Run seeding - it will fail on letter 'm'
    let res = seed_lowercase_latin(&mut conn);
    assert!(res.is_err(), "Seeding should have failed on M");

    // Since seeding failed, the entire transaction must have rolled back.
    // Verify that NO items for other letters exist in heads or collections
    // (the pre-inserted 'urn:language-graph:grapheme:nfc:006d' is still there, but nothing else).
    let head_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM entity_heads", [], |row| row.get(0))
        .unwrap();
    assert_eq!(
        head_count, 0,
        "No heads should have been seeded due to transaction rollback"
    );

    let snapshot_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM collection_snapshots", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(snapshot_count, 0, "No snapshot should have been created");

    let block_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM immutable_blocks", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(block_count, 0, "No blocks should have been committed");
}
