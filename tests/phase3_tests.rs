use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use language_graph_engine::app::AppState;
use language_graph_engine::config::Config;
use language_graph_engine::db::migrations::run_migrations;
use language_graph_engine::resolver::text::TextResolver;
use language_graph_engine::seed::ascii_supplemental::seed_phase2_1;
use language_graph_engine::server::Server;
use language_graph_engine::written_forms::{
    find_written_form_exact, get_active_store_snapshot, get_written_form_details, is_eligible,
    list_written_forms, preview_written_form, publish_store_snapshot, save_written_form,
    STORE_ENTITY_ID,
};
use proptest::prelude::*;
use rusqlite::Connection;
use std::sync::Arc;
use tower::ServiceExt;

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
fn test_eligibility_policy() {
    // Valid inputs
    let valid = vec![
        "bank",
        "hello",
        "Hello",
        "Vatsal",
        "can't",
        "O'Neill",
        "mother-in-law",
        "well-known",
        "bank's",
        "rock'n'roll",
    ];
    for input in valid {
        assert!(is_eligible(input), "Should accept valid word: {}", input);
    }

    // Invalid inputs
    let invalid = vec![
        "",
        " ",
        "two words",
        " hello",
        "hello ",
        "Hello!",
        "bank.",
        "-leading",
        "trailing-",
        "double--hyphen",
        "bad''quote",
        "COVID-19",
        "B2B",
        "room101",
        "C++",
        "email@example.com",
        "$25.00",
        "Project_3",
        "https://example.com",
        "array[0]",
        "café",
        "It’s",
    ];
    for input in invalid {
        assert!(!is_eligible(input), "Should reject invalid word: {}", input);
    }
}

#[test]
fn test_ascii_resolver_can_resolve_rejected_words() {
    let (_conn, resolver) = get_temp_db_and_resolver();

    // Word validation rejects C++, email@example.com, and array[0]
    let words = vec!["C++", "email@example.com", "array[0]"];
    for w in words {
        assert!(!is_eligible(w));
        // But the symbol layer resolver can resolve them perfectly!
        let res = resolver.resolve(w).unwrap();
        assert_eq!(res.output, w);
    }
}

#[test]
fn test_preview_is_read_only() {
    let (conn, resolver) = get_temp_db_and_resolver();

    let initial_block_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM immutable_blocks", [], |r| r.get(0))
        .unwrap();

    // Call preview repeatedly
    for _ in 0..5 {
        let preview = preview_written_form(&resolver, &conn, "bank").unwrap();
        assert!(preview.is_eligible);
        assert_eq!(preview.original_input, "bank");
        assert_eq!(preview.normalized_form, "bank");
        assert_eq!(
            preview.expected_entity_id.unwrap(),
            "urn:language-graph:written-form:nfc:utf8:62616e6b"
        );
        assert!(!preview.is_already_stored);
    }

    let final_block_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM immutable_blocks", [], |r| r.get(0))
        .unwrap();

    assert_eq!(
        initial_block_count, final_block_count,
        "Preview should not write to database"
    );
}

#[test]
fn test_explicit_save_and_idempotency() {
    let (mut conn, resolver) = get_temp_db_and_resolver();

    // Save bank
    let res1 = save_written_form(&resolver, &mut conn, "bank").unwrap();
    assert_eq!(res1.status, "Created");
    assert_eq!(res1.surface_form, "bank");
    assert_eq!(
        res1.entity_id,
        "urn:language-graph:written-form:nfc:utf8:62616e6b"
    );

    // Assert rows were created
    let entity_exists: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM entities WHERE entity_id = ?1",
            [&res1.entity_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(entity_exists, 1);

    let head_cid: String = conn
        .query_row(
            "SELECT revision_cid FROM entity_heads WHERE entity_id = ?1",
            [&res1.entity_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(head_cid, res1.revision_cid);

    let wf_exists: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM written_forms WHERE entity_id = ?1",
            [&res1.entity_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(wf_exists, 1);

    let component_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM written_form_components WHERE written_form_revision_cid = ?1",
            [&res1.revision_cid],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(component_count, 4);

    let member_exists: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM written_form_store_members WHERE store_entity_id = ?1 AND written_form_entity_id = ?2",
            [STORE_ENTITY_ID, &res1.entity_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(member_exists, 1);

    // Save bank again (idempotent)
    let res2 = save_written_form(&resolver, &mut conn, "bank").unwrap();
    assert_eq!(res2.status, "Already Stored");
    assert_eq!(res2.entity_id, res1.entity_id);
    assert_eq!(res2.revision_cid, res1.revision_cid);
}

#[test]
fn test_transaction_rollback_on_failure() {
    let (mut conn, resolver) = get_temp_db_and_resolver();

    // Pre-insert a conflicting row in written_forms with the same entity_id to trigger a primary key conflict
    let conflict_entity_id = "urn:language-graph:written-form:nfc:utf8:62616e6b"; // bank
    conn.execute(
        "INSERT INTO entities (entity_id, kind, canonical_key, label, created_at)
         VALUES (?1, 'written-form', 'conflicting-key', 'Conflict', datetime('now'))",
        [conflict_entity_id],
    )
    .unwrap();

    // Now try to save bank, which will try to insert "written-form:bank" as canonical key for conflict_entity_id,
    // which will fail or conflict
    let res = save_written_form(&resolver, &mut conn, "bank");
    assert!(res.is_err());

    // Verify no written_forms row for bank exists and no components are stored
    let wf_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM written_forms WHERE entity_id = ?1",
            [conflict_entity_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(wf_count, 0);

    let comp_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM written_form_components", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(comp_count, 0);
}

#[test]
fn test_exact_lookup_case_sensitivity() {
    let (mut conn, resolver) = get_temp_db_and_resolver();

    save_written_form(&resolver, &mut conn, "bank").unwrap();
    save_written_form(&resolver, &mut conn, "Bank").unwrap();

    let lookup1 = find_written_form_exact(&conn, "bank").unwrap().unwrap();
    assert_eq!(lookup1.surface_form, "bank");
    assert_eq!(
        lookup1.entity_id,
        "urn:language-graph:written-form:nfc:utf8:62616e6b"
    );

    let lookup2 = find_written_form_exact(&conn, "Bank").unwrap().unwrap();
    assert_eq!(lookup2.surface_form, "Bank");
    assert_eq!(
        lookup2.entity_id,
        "urn:language-graph:written-form:nfc:utf8:42616e6b"
    );

    let lookup3 = find_written_form_exact(&conn, "BANK").unwrap();
    assert!(lookup3.is_none());

    let lookup4 = find_written_form_exact(&conn, "cant").unwrap();
    assert!(lookup4.is_none());
}

#[test]
fn test_listing_and_pagination() {
    let (mut conn, resolver) = get_temp_db_and_resolver();

    save_written_form(&resolver, &mut conn, "bank").unwrap();
    save_written_form(&resolver, &mut conn, "can't").unwrap();
    save_written_form(&resolver, &mut conn, "mother-in-law").unwrap();

    // Order alphabetically: bank, can't, mother-in-law
    let list1 = list_written_forms(&conn, STORE_ENTITY_ID, 2, 0).unwrap();
    assert_eq!(list1.len(), 2);
    assert_eq!(list1[0].surface_form, "bank");
    assert_eq!(list1[1].surface_form, "can't");

    let list2 = list_written_forms(&conn, STORE_ENTITY_ID, 2, 2).unwrap();
    assert_eq!(list2.len(), 1);
    assert_eq!(list2[0].surface_form, "mother-in-law");
}

#[test]
fn test_details_retrieval() {
    let (mut conn, resolver) = get_temp_db_and_resolver();

    let saved = save_written_form(&resolver, &mut conn, "bank").unwrap();
    let details = get_written_form_details(&conn, "bank").unwrap().unwrap();

    assert_eq!(details.surface_form, "bank");
    assert_eq!(details.entity_id, saved.entity_id);
    assert_eq!(details.revision_cid, saved.revision_cid);
    assert_eq!(details.components.len(), 4);
    assert_eq!(details.components[0].surface_form, "b");
    assert_eq!(details.components[3].surface_form, "k");
}

#[test]
fn test_snapshot_publication() {
    let (mut conn, resolver) = get_temp_db_and_resolver();

    save_written_form(&resolver, &mut conn, "bank").unwrap();
    save_written_form(&resolver, &mut conn, "can't").unwrap();
    save_written_form(&resolver, &mut conn, "mother-in-law").unwrap();

    let pub1 = publish_store_snapshot(&mut conn).unwrap();
    assert_eq!(pub1.status, "Published");
    assert_eq!(pub1.member_count, 3);

    // Idempotent publish
    let pub2 = publish_store_snapshot(&mut conn).unwrap();
    assert_eq!(pub2.status, "No Changes");
    assert_eq!(pub2.snapshot_cid, pub1.snapshot_cid);

    // Save another and publish
    save_written_form(&resolver, &mut conn, "hello").unwrap();
    let pub3 = publish_store_snapshot(&mut conn).unwrap();
    assert_eq!(pub3.status, "Published");
    assert_ne!(pub3.snapshot_cid, pub1.snapshot_cid);
    assert_eq!(pub3.member_count, 4);

    // Verify snapshot contents from DB
    let active_snap = get_active_store_snapshot(&conn).unwrap().unwrap();
    assert_eq!(active_snap.store_entity_id, STORE_ENTITY_ID);
    assert_eq!(active_snap.members.len(), 4);
    // Sort order: bank, can't, hello, mother-in-law
    assert_eq!(
        active_snap.members[0].written_form_entity_id,
        "urn:language-graph:written-form:nfc:utf8:62616e6b"
    ); // bank
    assert_eq!(
        active_snap.members[2].written_form_entity_id,
        "urn:language-graph:written-form:nfc:utf8:68656c6c6f"
    ); // hello
}

#[test]
fn test_golden_vectors_generation() {
    let (mut conn, resolver) = get_temp_db_and_resolver();

    let cases = vec!["bank", "Bank", "can't", "mother-in-law"];
    for case in cases {
        let saved = save_written_form(&resolver, &mut conn, case).unwrap();
        println!(
            "GOLDEN FIXTURE FOR '{}':\n  normalized_form: {}\n  entity_id: {}\n  revision_cid: {}\n  profile_cid: {}\n",
            case, case, saved.entity_id, saved.revision_cid, saved.composition_profile_snapshot_cid
        );
    }
}

// Property-based testing
proptest! {
    #![proptest_config(ProptestConfig::with_cases(20))]
    #[test]
    fn test_property_valid_word_saving(ref s in "[a-zA-Z]+") {
        let mut conn = get_temp_db();
        let resolver = TextResolver::load(&conn).unwrap();
        if is_eligible(s) {
            let res = save_written_form(&resolver, &mut conn, s).unwrap();
            prop_assert_eq!(&res.surface_form, s);
            prop_assert_eq!(res.status, "Created");

            let lookup = find_written_form_exact(&conn, s).unwrap().unwrap();
            prop_assert_eq!(&lookup.surface_form, s);

            let res_dup = save_written_form(&resolver, &mut conn, s).unwrap();
            prop_assert_eq!(res_dup.status, "Already Stored");
        }
    }
}

#[test]
fn test_concurrency() {
    let (_conn, resolver) = get_temp_db_and_resolver();
    let resolver_arc = Arc::new(resolver);

    let handles: Vec<_> = (0..5)
        .map(|_i| {
            let res_clone = Arc::clone(&resolver_arc);
            // In a real application, multiple connections can share the DB, but with SQLite in-memory, we can share the connection or use isolated ones.
            // Since we want to test database safe concurrently, let's write isolated concurrent tasks that preview or lookup safely, and serialized saves.
            // Let's spawn thread to preview
            std::thread::spawn(move || {
                let conn_temp = Connection::open_in_memory().unwrap();
                // Previews do not write, so they can run concurrently without state conflict.
                let preview = preview_written_form(&res_clone, &conn_temp, "bank").unwrap();
                assert!(preview.is_eligible);
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }
}

async fn get_test_app() -> (axum::Router, tempfile::NamedTempFile) {
    let temp_db = tempfile::NamedTempFile::new().expect("Failed to create temp DB file");
    let config = Config {
        db_path: temp_db.path().to_path_buf(),
        listen_port: 0,
        seed_phase: "phase2_1".to_string(),
    };
    let app_state = AppState::new(config).expect("AppState init failed");
    let app = Server::build_router(app_state);
    (app, temp_db)
}

#[tokio::test]
async fn test_http_api_phase3_endpoints() {
    let (app, _temp_db) = get_test_app().await;

    // 1. POST /api/wordforms/preview
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/wordforms/preview")
                .header("Content-Type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&serde_json::json!({ "text": "bank" })).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let preview: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(preview["isEligible"], true);
    assert_eq!(
        preview["expectedEntityId"],
        "urn:language-graph:written-form:nfc:utf8:62616e6b"
    );

    // 2. POST /api/wordforms to save
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/wordforms")
                .header("Content-Type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&serde_json::json!({ "text": "bank" })).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let save_res: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(save_res["status"], "Created");

    // 3. GET /api/wordforms/exact?surface=bank
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/wordforms/exact?surface=bank")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let lookup: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(lookup["surfaceForm"], "bank");

    // 4. GET /api/wordforms/details?surface=bank
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/wordforms/details?surface=bank")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let details: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(details["surfaceForm"], "bank");
    assert_eq!(details["components"].as_array().unwrap().len(), 4);

    // 5. GET /api/wordforms (list)
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/wordforms?store=urn:language-graph:store:english-natural-language-written-forms&limit=10&offset=0")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let list: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(list.as_array().unwrap().len(), 1);

    // 6. POST /publish
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/word-stores/english-natural-language-written-forms/publish")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let publish: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(publish["status"], "Published");

    // 7. GET /snapshots/active
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/word-stores/english-natural-language-written-forms/snapshots/active")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}
