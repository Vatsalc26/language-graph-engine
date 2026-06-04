use language_graph_engine::db::migrations::run_migrations;
use language_graph_engine::lexicon_import::esdb::{classify_word, Classification};
use language_graph_engine::lexicon_import::importer::{
    analyze_esdb_file, import_eligible_words, list_deferred_entries,
};
use language_graph_engine::resolver::text::TextResolver;
use language_graph_engine::seed::ascii_supplemental::seed_phase2_1;
use language_graph_engine::written_forms::{
    get_written_form_details, save_written_form, STORE_ENTITY_ID,
};
use rusqlite::Connection;

fn get_temp_db() -> Connection {
    let mut conn = Connection::open_in_memory().expect("Failed to open in-memory SQLite");
    run_migrations(&conn).expect("Failed to run migrations");
    seed_phase2_1(&mut conn).expect("Failed to seed Phase 2.1");
    conn
}

fn get_temp_db_and_resolver() -> (Connection, TextResolver) {
    let conn = get_temp_db();
    let resolver = TextResolver::load(&conn).expect("Load resolver");
    (conn, resolver)
}

#[test]
fn test_esdb_classification() {
    assert_eq!(classify_word("bank"), Classification::Eligible);
    assert_eq!(classify_word("can't"), Classification::Eligible);
    assert_eq!(classify_word("mother-in-law"), Classification::Eligible);

    assert!(matches!(
        classify_word("café"),
        Classification::Deferred {
            reason_code: "unsupported_non_ascii",
            ..
        }
    ));
    assert!(matches!(
        classify_word("ice cream"),
        Classification::Deferred {
            reason_code: "contains_space_or_multiword",
            ..
        }
    ));
    assert!(matches!(
        classify_word("U.S."),
        Classification::Deferred {
            reason_code: "abbreviation_or_special_form",
            ..
        }
    ));
    assert!(matches!(
        classify_word("COVID-19"),
        Classification::Deferred {
            reason_code: "contains_digits_or_alphanumeric_structure",
            ..
        }
    ));
    assert!(matches!(
        classify_word(""),
        Classification::Deferred {
            reason_code: "malformed_or_empty",
            ..
        }
    ));
}

#[test]
fn test_dry_run_analysis_is_read_only() {
    let (conn, _resolver) = get_temp_db_and_resolver();

    let initial_block_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM immutable_blocks", [], |r| r.get(0))
        .unwrap();

    let fixture = b"bank\ncan't\ncaf\xc3\xa9\nice cream\nCOVID-19\n";

    let report = analyze_esdb_file(&conn, fixture, None).unwrap();
    assert_eq!(report.entries_read, 5);
    assert_eq!(report.eligible_new_words, 2);
    assert_eq!(report.deferred_entries, 3);

    let final_block_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM immutable_blocks", [], |r| r.get(0))
        .unwrap();

    assert_eq!(
        initial_block_count, final_block_count,
        "Dry run analysis must be completely read-only"
    );
}

#[test]
fn test_import_correctness_and_idempotency() {
    let (mut conn, resolver) = get_temp_db_and_resolver();

    // 1. Pre-save a word manually
    let manually_saved = save_written_form(&resolver, &mut conn, "bank").unwrap();
    assert_eq!(manually_saved.status, "Created");

    let fixture = b"bank\ncan't\nmother-in-law\ncaf\xc3\xa9\nice cream\n";

    // 2. Perform import (Run 1)
    let report = import_eligible_words(&mut conn, &resolver, fixture, None).unwrap();
    assert_eq!(report.eligible_new_words, 2); // can't, mother-in-law
    assert_eq!(report.eligible_existing_words_to_reuse, 1); // bank (reused)
    assert_eq!(report.deferred_entries, 2); // café, ice cream
    assert!(report.snapshot_cid.is_some());
    assert!(report.manifest_cid.is_some());

    // Verify bank is reused and details contain both Manual and ESDB attestations
    let details = get_written_form_details(&conn, "bank").unwrap().unwrap();
    assert_eq!(details.revision_cid, manually_saved.revision_cid);
    let attestations = details.attestations.unwrap();
    assert_eq!(attestations.len(), 2);
    assert_eq!(
        attestations[0],
        "ESDB English (US) rel-2026.02.25 Default Wordlist"
    );
    assert_eq!(attestations[1], "Manually saved");

    // 3. Re-run identical import (Run 2)
    let report2 = import_eligible_words(&mut conn, &resolver, fixture, None).unwrap();
    assert_eq!(report2.eligible_new_words, 0); // All already stored
    assert_eq!(report2.eligible_existing_words_to_reuse, 3); // bank, can't, mother-in-law
    assert_eq!(
        report2.snapshot_cid, report.snapshot_cid,
        "Snapshot CID must remain identical"
    );

    // 4. Re-run identical import (Run 3) to test manifest CID determinism under identical DB state
    let report3 = import_eligible_words(&mut conn, &resolver, fixture, None).unwrap();
    assert_eq!(report3.eligible_new_words, 0);
    assert_eq!(report3.eligible_existing_words_to_reuse, 3);
    assert_eq!(report3.snapshot_cid, report2.snapshot_cid);
    assert_eq!(
        report3.manifest_cid, report2.manifest_cid,
        "Manifest CID must remain identical for identical input and database state"
    );

    // 5. Verify batch records and deferred entries pagination
    let import_id: String = conn.query_row(
        "SELECT import_id FROM lexicon_import_batches WHERE status = 'completed' AND import_id != 'manual' ORDER BY completed_at DESC LIMIT 1",
        [],
        |r| r.get(0)
    ).unwrap();

    let deferred = list_deferred_entries(&conn, &import_id, 10, 0).unwrap();
    assert_eq!(deferred.len(), 2); // café, ice cream
    assert_eq!(deferred[0].reason_code, "unsupported_non_ascii");
}

#[test]
fn test_import_rollback_on_failure() {
    let (mut conn, resolver) = get_temp_db_and_resolver();

    // Verify initial state
    let initial_store_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM written_form_store_members WHERE store_entity_id = ?1",
            [STORE_ENTITY_ID],
            |r| r.get(0),
        )
        .unwrap();

    // Trigger failure by introducing count mismatch
    let fixture = b"bank\n";
    let res = import_eligible_words(&mut conn, &resolver, fixture, Some(109902));
    assert!(res.is_err(), "Must error on expected count check mismatch");

    // Verify that the import did not write any entries to the store
    let final_store_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM written_form_store_members WHERE store_entity_id = ?1",
            [STORE_ENTITY_ID],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        initial_store_count, final_store_count,
        "No entries should be added on failure"
    );

    // The batch row should record status = failed
    let failed_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM lexicon_import_batches WHERE status = 'failed'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(failed_count, 1);
}
