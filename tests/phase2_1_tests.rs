use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use language_graph_engine::app::AppState;
use language_graph_engine::config::Config;
use language_graph_engine::content::cid::compute_cid;
use language_graph_engine::content::encoding::to_dag_cbor;
use language_graph_engine::db::migrations::run_migrations;
use language_graph_engine::db::repository::Repository;
use language_graph_engine::model::GraphemeRevision;
use language_graph_engine::resolver::text::TextResolver;
use language_graph_engine::seed::ascii_supplemental::{
    seed_phase2_1, PROFILE_2_1_ENTITY_ID, SUPPLEMENTAL_COLLECTION_ENTITY_ID,
};
use language_graph_engine::seed::lowercase_latin::COLLECTION_ENTITY_ID as LOW_COL_ID;
use language_graph_engine::seed::phase2::{
    seed_phase2, DIGITS_COLLECTION_ENTITY_ID, PROFILE_ENTITY_ID as PROFILE_2_ENTITY_ID,
    PUNCTUATION_COLLECTION_ENTITY_ID, UPPERCASE_COLLECTION_ENTITY_ID,
    WHITESPACE_COLLECTION_ENTITY_ID,
};
use language_graph_engine::server::Server;
use proptest::prelude::*;
use rusqlite::Connection;
use tower::ServiceExt;
use unicode_normalization::UnicodeNormalization;

// Golden CIDs
const GOLDEN_LOW_SNAP_CID: &str = "bafyreib4ivpoazb5skkr7yvfelvoowz6sxxncdsjewvxawyedm5tikeshm";
const GOLDEN_PROFILE_2_SNAP_CID: &str =
    "bafyreic5acpnm6zr4cp6jl3xm425kwft77qegml2mhxftrwclkelnqplry";
const GOLDEN_PROFILE_2_1_SNAP_CID: &str =
    "bafyreidfdj3hw7gv5rt7bpsfkrkhuptprcjlwzpaq3yektnztec4caqdn4";
const GOLDEN_SUPPLEMENTAL_SNAP_CID: &str =
    "bafyreiaczeqz45ypyr53lbmyyar3ppquj2zusctubs4wqmhaqxxxnjl6zm";

fn get_temp_db() -> Connection {
    let conn = Connection::open_in_memory().expect("Failed to open in-memory SQLite");
    run_migrations(&conn).expect("Failed to run migrations");
    conn
}

fn make_revision(ch: char, script: &str, case: &str) -> GraphemeRevision {
    let surface_form: String = ch.to_string().nfc().collect();
    let scalar_val = ch as u32;
    let scalar_str = format!("U+{:04X}", scalar_val);
    let hex_id = format!("{:04x}", scalar_val);
    let entity_id = format!("urn:language-graph:grapheme:nfc:{}", hex_id);

    GraphemeRevision {
        schema: "language-graph/grapheme-revision/v1".to_string(),
        entity_id,
        kind: "grapheme".to_string(),
        surface_form: surface_form.clone(),
        normalized_form: surface_form.clone(),
        normalization: "NFC".to_string(),
        unicode_scalars: vec![scalar_str],
        script: script.to_string(),
        case: case.to_string(),
        previous_revision_cid: None,
    }
}

#[test]
fn test_regression_phase1_and_phase2_cids_unchanged() {
    let mut conn = get_temp_db();

    // Seeding Phase 2 yields the same Phase 2 profile CID
    let p2_cid = seed_phase2(&mut conn).unwrap();
    assert_eq!(p2_cid, GOLDEN_PROFILE_2_SNAP_CID);

    let repo = Repository::new(&conn);
    let low_snap_cid = repo.get_active_snapshot_cid(LOW_COL_ID).unwrap().unwrap();
    assert_eq!(low_snap_cid, GOLDEN_LOW_SNAP_CID);
}

#[test]
fn test_deterministic_new_symbol_cids() {
    // Assert on deterministic CIDs of a few key supplemental symbols
    let test_cases = vec![
        (
            '#',
            "Common",
            "none",
            "bafyreiajj25zb3zic6pcu7655fsk4a7mvclwgiof3i44eovx2zxodioo7m",
        ),
        (
            '$',
            "Common",
            "none",
            "bafyreidh4oihmqyykdz7dwsvy4yndkg7ihw4hf3jcn3h2z46fqr5pldznu",
        ),
        (
            '\\',
            "Common",
            "none",
            "bafyreigsg5oxzm26o4v35tpamxbbg2xrgc65a32x44zxpawxyqssata6cm",
        ),
        (
            '`',
            "Common",
            "none",
            "bafyreibvmqwgeanqyomvln2zs2ixjw2jystszzpccukyzcz433nsdmyvke",
        ),
        (
            '~',
            "Common",
            "none",
            "bafyreicx2xpvp2d4gzijs7q3irpd56ymjooalnyzwvkpypaeblzkoken4e",
        ),
    ];

    for (ch, script, case, expected_cid) in test_cases {
        let rev = make_revision(ch, script, case);
        let bytes = to_dag_cbor(&rev).unwrap();
        let cid = compute_cid(&bytes).unwrap();
        assert_eq!(cid.to_string(), expected_cid, "CID mismatch for '{}'", ch);
    }
}

#[test]
fn test_collection_snapshot_cids() {
    let mut conn = get_temp_db();
    seed_phase2_1(&mut conn).unwrap();
    let repo = Repository::new(&conn);

    let supp_snap = repo
        .get_active_snapshot_cid(SUPPLEMENTAL_COLLECTION_ENTITY_ID)
        .unwrap()
        .unwrap();
    assert_eq!(supp_snap, GOLDEN_SUPPLEMENTAL_SNAP_CID);

    let supp_members = repo.get_snapshot_members(&supp_snap).unwrap();
    assert_eq!(supp_members.len(), 21);

    // First member is '#'
    assert_eq!(
        supp_members[0].entity_id,
        "urn:language-graph:grapheme:nfc:0023"
    );
    // Last member is '~'
    assert_eq!(
        supp_members[20].entity_id,
        "urn:language-graph:grapheme:nfc:007e"
    );
}

#[test]
fn test_text_profile_snapshot_cid() {
    let mut conn = get_temp_db();
    let profile_cid = seed_phase2_1(&mut conn).unwrap();
    assert_eq!(profile_cid, GOLDEN_PROFILE_2_1_SNAP_CID);

    let repo = Repository::new(&conn);
    let active_profile_cid = repo
        .get_active_profile_snapshot_cid(PROFILE_2_1_ENTITY_ID)
        .unwrap()
        .unwrap();
    assert_eq!(active_profile_cid, GOLDEN_PROFILE_2_1_SNAP_CID);

    let profile = repo.get_profile_snapshot(&active_profile_cid).unwrap();
    assert_eq!(profile.schema, "language-graph/text-profile-snapshot/v1");
    assert_eq!(profile.profile_entity_id, PROFILE_2_1_ENTITY_ID);
    assert_eq!(profile.kind, "written-text-profile");
    assert_eq!(profile.label, "Printable ASCII Text Profile");
    assert_eq!(profile.collections.len(), 6);

    // Verify ordering
    assert_eq!(profile.collections[0].collection_entity_id, LOW_COL_ID);
    assert_eq!(
        profile.collections[1].collection_entity_id,
        UPPERCASE_COLLECTION_ENTITY_ID
    );
    assert_eq!(
        profile.collections[2].collection_entity_id,
        DIGITS_COLLECTION_ENTITY_ID
    );
    assert_eq!(
        profile.collections[3].collection_entity_id,
        WHITESPACE_COLLECTION_ENTITY_ID
    );
    assert_eq!(
        profile.collections[4].collection_entity_id,
        PUNCTUATION_COLLECTION_ENTITY_ID
    );
    assert_eq!(
        profile.collections[5].collection_entity_id,
        SUPPLEMENTAL_COLLECTION_ENTITY_ID
    );
}

#[test]
fn test_seeding_idempotency() {
    let mut conn = get_temp_db();
    let cid1 = seed_phase2_1(&mut conn).unwrap();
    let cid2 = seed_phase2_1(&mut conn).unwrap();
    assert_eq!(cid1, cid2);

    let block_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM immutable_blocks", [], |row| {
            row.get(0)
        })
        .unwrap();
    // Phase 2 blocks: 80
    // Phase 2.1 additional graphemes: 21
    // Phase 2.1 collection snapshot: 1
    // Phase 2.1 profile snapshot: 1
    // Total: 103 blocks
    assert_eq!(block_count, 103);
}

#[test]
fn test_transaction_safety_on_phase2_1_failure() {
    let mut conn = get_temp_db();

    // Pre-insert a conflicting entity for number sign '#' before seeding
    conn.execute(
        "INSERT INTO entities (entity_id, kind, canonical_key, label, created_at)
         VALUES ('urn:language-graph:grapheme:nfc:0023', 'grapheme', '#', 'Conflict Number Sign', datetime('now'))",
        [],
    ).unwrap();

    let res = seed_phase2_1(&mut conn);
    assert!(res.is_err());

    // Seeding should have rolled back to Phase 2 state
    let repo = Repository::new(&conn);
    let active_profile_2_1 = repo
        .get_active_profile_snapshot_cid(PROFILE_2_1_ENTITY_ID)
        .unwrap();
    assert!(active_profile_2_1.is_none());

    // Phase 2 profile snapshot should still be there though, because it committed successfully in its own tx before Phase 2.1 ran
    let active_profile_2 = repo
        .get_active_profile_snapshot_cid(PROFILE_2_ENTITY_ID)
        .unwrap();
    assert!(active_profile_2.is_some());
}

#[test]
fn test_resolver_valid_cases() {
    let mut conn = get_temp_db();
    seed_phase2_1(&mut conn).unwrap();

    let resolver = TextResolver::load(&conn).unwrap();
    assert_eq!(resolver.cache.len(), 95);

    let test_inputs = vec![
        "Hello, World! #2026",
        "https://github.com/vatsal",
        "a+b = c-d*e/f",
        "~`@[^_`{|}~]",
        "abcdefghijklmnopqrstuvwxyz ABCDEFGHIJKLMNOPQRSTUVWXYZ 0123456789",
    ];

    for input in test_inputs {
        let res = resolver.resolve(input).unwrap();
        assert_eq!(res.input, input);
        assert_eq!(res.output, input);
        assert_eq!(res.collection_snapshot_cid, GOLDEN_PROFILE_2_1_SNAP_CID);

        // Verify traces
        for (i, step) in res.trace.iter().enumerate() {
            assert_eq!(step.position, i + 1);
            assert!(!step.entity_id.is_empty());
            assert!(!step.revision_cid.is_empty());
            assert!(!step.display_name.is_empty());
            assert!(!step.category.is_empty());
            assert!(!step.source_collection_entity_id.is_empty());
            assert!(!step.source_collection_snapshot_cid.is_empty());
        }
    }
}

#[test]
fn test_resolver_unsupported_validation_errors() {
    let mut conn = get_temp_db();
    seed_phase2_1(&mut conn).unwrap();

    let resolver = TextResolver::load(&conn).unwrap();

    let invalid_inputs = vec![
        (
            "It’s working…",
            vec![
                "’ U+2019 RIGHT SINGLE QUOTATION MARK at position 3",
                "… U+2026 HORIZONTAL ELLIPSIS at position 13",
            ],
        ),
        (
            "“Hello”",
            vec![
                "“ U+201c LEFT DOUBLE QUOTATION MARK at position 1",
                "” U+201d RIGHT DOUBLE QUOTATION MARK at position 7",
            ],
        ),
        ("Line\nbreak", vec!["\n U+000A NEWLINE at position 5"]),
        ("Tab\tcharacter", vec!["\t U+0009 TAB at position 4"]),
    ];

    for (input, expected_err_parts) in invalid_inputs {
        let res = resolver.resolve(input);
        assert!(res.is_err());
        let err_msg = res.unwrap_err().to_string();
        for part in expected_err_parts {
            assert!(
                err_msg.to_lowercase().contains(&part.to_lowercase()),
                "Error msg: '{}' missing expected part: '{}'",
                err_msg,
                part
            );
        }
    }
}

#[test]
fn test_resolver_non_mutation() {
    let mut conn = get_temp_db();
    seed_phase2_1(&mut conn).unwrap();

    let resolver = TextResolver::load(&conn).unwrap();

    let get_counts = |c: &Connection| -> (i64, i64, i64, i64, i64, i64) {
        let blocks: i64 = c
            .query_row("SELECT COUNT(*) FROM immutable_blocks", [], |r| r.get(0))
            .unwrap();
        let entities: i64 = c
            .query_row("SELECT COUNT(*) FROM entities", [], |r| r.get(0))
            .unwrap();
        let heads: i64 = c
            .query_row("SELECT COUNT(*) FROM entity_heads", [], |r| r.get(0))
            .unwrap();
        let profiles: i64 = c
            .query_row("SELECT COUNT(*) FROM text_profiles", [], |r| r.get(0))
            .unwrap();
        let profile_snaps: i64 = c
            .query_row("SELECT COUNT(*) FROM text_profile_snapshots", [], |r| {
                r.get(0)
            })
            .unwrap();
        let active_snaps: i64 = c
            .query_row(
                "SELECT COUNT(*) FROM active_text_profile_snapshots",
                [],
                |r| r.get(0),
            )
            .unwrap();
        (
            blocks,
            entities,
            heads,
            profiles,
            profile_snaps,
            active_snaps,
        )
    };

    let counts_before = get_counts(&conn);

    let _ = resolver.resolve("Hello, Vatsal! +1");
    let _ = resolver.resolve("Smart quotes ’ fail.");

    let counts_after = get_counts(&conn);
    assert_eq!(
        counts_before, counts_after,
        "Resolver queries must not mutate DB tables"
    );
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]
    #[test]
    fn test_property_resolver_identity(s in "[\\x20-\\x7E]{1,100}") {
        let mut conn = get_temp_db();
        seed_phase2_1(&mut conn).unwrap();
        let resolver = TextResolver::load(&conn).unwrap();
        let res = resolver.resolve(&s).unwrap();
        assert_eq!(res.output, s);
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
async fn test_http_api_phase2_1_endpoints() {
    let (app, _temp_db) = get_test_app().await;

    // 1. GET /api/status returns 95 count and active profile snapshot CID
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let status_json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(status_json["symbolCount"], 95);
    assert_eq!(
        status_json["activeSnapshotCid"],
        GOLDEN_PROFILE_2_1_SNAP_CID
    );

    // 2. GET /api/collections returns 6 collections
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/collections")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let collections: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let arr = collections.as_array().unwrap();
    assert_eq!(arr.len(), 6);

    // 3. GET /api/profiles/active returns profile details
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/profiles/active")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let profile: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(profile["profileEntityId"], PROFILE_2_1_ENTITY_ID);

    // 4. POST /api/resolve accepts valid Phase 2.1 input with supplemental characters
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/resolve")
                .header("Content-Type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&serde_json::json!({ "text": "Hello, #Vatsal! +~`" }))
                        .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let resolve_res: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(resolve_res["output"], "Hello, #Vatsal! +~`");
}

#[test]
fn test_regression_phase2_1_specific_examples() {
    let mut conn = get_temp_db();
    seed_phase2_1(&mut conn).unwrap();

    let resolver = TextResolver::load(&conn).unwrap();
    assert_eq!(resolver.cache.len(), 95);

    let specific_cases = vec![
        r"path\to\file",
        "3+4=7",
        "C++ >= C?",
        "email@example.com",
        "array[0]",
        r#"{status: "ok"}"#,
    ];

    for input in specific_cases {
        let res = resolver.resolve(input).unwrap();
        assert_eq!(res.input, input);
        assert_eq!(res.output, input);
    }
}

fn get_db_counts(c: &Connection) -> (i64, i64, i64, i64, i64, i64) {
    let blocks: i64 = c
        .query_row("SELECT COUNT(*) FROM immutable_blocks", [], |r| r.get(0))
        .unwrap();
    let entities: i64 = c
        .query_row("SELECT COUNT(*) FROM entities", [], |r| r.get(0))
        .unwrap();
    let heads: i64 = c
        .query_row("SELECT COUNT(*) FROM entity_heads", [], |r| r.get(0))
        .unwrap();
    let profiles: i64 = c
        .query_row("SELECT COUNT(*) FROM text_profiles", [], |r| r.get(0))
        .unwrap();
    let profile_snaps: i64 = c
        .query_row("SELECT COUNT(*) FROM text_profile_snapshots", [], |r| {
            r.get(0)
        })
        .unwrap();
    let active_snaps: i64 = c
        .query_row(
            "SELECT COUNT(*) FROM active_text_profile_snapshots",
            [],
            |r| r.get(0),
        )
        .unwrap();
    (
        blocks,
        entities,
        heads,
        profiles,
        profile_snaps,
        active_snaps,
    )
}

#[test]
fn test_regression_phase2_1_keyboard_resolution() {
    let mut conn = get_temp_db();
    seed_phase2_1(&mut conn).unwrap();

    let resolver = TextResolver::load(&conn).unwrap();
    assert_eq!(resolver.cache.len(), 95);

    let specific_cases = vec![
        r"path\to\file",
        "3+4=7",
        "C++ >= C?",
        "email@example.com",
        "array[0]",
        r#"{status: "ok"}"#,
        "price=$25.00",
        "cats & dogs",
        "https://example.com/path?x=1&y=2",
    ];

    use unicode_segmentation::UnicodeSegmentation;

    for input in specific_cases {
        let counts_before = get_db_counts(&conn);
        let res = resolver.resolve(input).unwrap();
        let counts_after = get_db_counts(&conn);

        assert_eq!(res.input, input);
        assert_eq!(res.output, input);
        assert_eq!(res.collection_snapshot_cid, GOLDEN_PROFILE_2_1_SNAP_CID);
        assert_eq!(res.trace.len(), input.graphemes(true).count());
        assert_eq!(
            counts_before, counts_after,
            "Normal resolution must perform no database writes"
        );
    }
}

#[test]
fn test_rejection_unicode_operator_counterparts() {
    let mut conn = get_temp_db();
    seed_phase2_1(&mut conn).unwrap();

    let resolver = TextResolver::load(&conn).unwrap();

    let invalid_inputs = vec![
        (
            "3 × 4 = 12",
            vec!["× U+00D7 UNSUPPORTED SYMBOL at position 3"],
        ),
        (
            "12 ÷ 3 = 4",
            vec!["÷ U+00F7 UNSUPPORTED SYMBOL at position 4"],
        ),
        (
            "10 − 2 = 8",
            vec!["− U+2212 UNSUPPORTED SYMBOL at position 4"],
        ),
        ("x ≤ 5", vec!["≤ U+2264 UNSUPPORTED SYMBOL at position 3"]),
        ("x ≥ 5", vec!["≥ U+2265 UNSUPPORTED SYMBOL at position 3"]),
        ("x ≠ y", vec!["≠ U+2260 UNSUPPORTED SYMBOL at position 3"]),
        (
            "It’s working…",
            vec![
                "’ U+2019 RIGHT SINGLE QUOTATION MARK at position 3",
                "… U+2026 HORIZONTAL ELLIPSIS at position 13",
            ],
        ),
        (
            "“Hello”",
            vec![
                "“ U+201C LEFT DOUBLE QUOTATION MARK at position 1",
                "” U+201D RIGHT DOUBLE QUOTATION MARK at position 7",
            ],
        ),
        ("Hello—world", vec!["— U+2014 EM DASH at position 6"]),
        ("line1\nline2", vec!["\n U+000A NEWLINE at position 6"]),
        ("tab\tvalue", vec!["\t U+0009 TAB at position 4"]),
    ];

    for (input, expected_err_parts) in invalid_inputs {
        let counts_before = get_db_counts(&conn);
        let res = resolver.resolve(input);
        let counts_after = get_db_counts(&conn);

        assert!(res.is_err(), "Input should be rejected: {}", input);
        assert_eq!(
            counts_before, counts_after,
            "Normal resolution must perform no database writes on error"
        );

        let err_msg = res.unwrap_err().to_string();
        for part in expected_err_parts {
            assert!(
                err_msg.to_lowercase().contains(&part.to_lowercase()),
                "Error msg: '{}' missing expected part: '{}'",
                err_msg,
                part
            );
        }
    }
}

#[test]
fn test_full_printable_ascii_profile_coverage() {
    let mut conn = get_temp_db();
    seed_phase2_1(&mut conn).unwrap();

    let resolver = TextResolver::load(&conn).unwrap();
    assert_eq!(resolver.cache.len(), 95);

    // Build the string containing all 95 printable ASCII characters
    let mut all_chars = String::new();
    for code in 0x20..=0x7E {
        all_chars.push(code as u8 as char);
    }
    assert_eq!(all_chars.len(), 95);

    // Resolve through active Printable ASCII profile
    let res = resolver.resolve(&all_chars).unwrap();
    assert_eq!(res.input, all_chars);
    assert_eq!(res.output, all_chars);
    assert_eq!(res.collection_snapshot_cid, GOLDEN_PROFILE_2_1_SNAP_CID);

    // Verify cache has exactly 95 supported symbols
    for code in 0x20..=0x7E {
        let ch = code as u8 as char;
        let s = ch.to_string();
        assert!(
            resolver.cache.contains_key(&s),
            "Missing char in cache: {}",
            s
        );
    }

    // Verify no extra unsupported scalar is present
    for key in resolver.cache.keys() {
        assert_eq!(key.len(), 1, "Cache key must be single grapheme");
        let ch = key.chars().next().unwrap();
        assert!(
            ('\u{0020}'..='\u{007E}').contains(&ch),
            "Extra char in cache: {}",
            ch
        );
    }
}

#[test]
fn test_supplemental_symbols_golden_vectors() {
    let mut conn = get_temp_db();
    let seeded_profile_cid = seed_phase2_1(&mut conn).unwrap();

    // Verify Golden CIDs of the snapshots
    assert_eq!(seeded_profile_cid, GOLDEN_PROFILE_2_1_SNAP_CID);

    let repo = Repository::new(&conn);

    let active_low_snap = repo.get_active_snapshot_cid(LOW_COL_ID).unwrap().unwrap();
    assert_eq!(active_low_snap, GOLDEN_LOW_SNAP_CID);

    let active_p2_snap = repo
        .get_active_profile_snapshot_cid(PROFILE_2_ENTITY_ID)
        .unwrap()
        .unwrap();
    assert_eq!(active_p2_snap, GOLDEN_PROFILE_2_SNAP_CID);

    let active_supp_snap = repo
        .get_active_snapshot_cid(SUPPLEMENTAL_COLLECTION_ENTITY_ID)
        .unwrap()
        .unwrap();
    assert_eq!(active_supp_snap, GOLDEN_SUPPLEMENTAL_SNAP_CID);

    let active_p2_1_snap = repo
        .get_active_profile_snapshot_cid(PROFILE_2_1_ENTITY_ID)
        .unwrap()
        .unwrap();
    assert_eq!(active_p2_1_snap, GOLDEN_PROFILE_2_1_SNAP_CID);

    // Verify detailed golden vectors for all 21 supplemental symbols
    let golden_table = vec![
        (
            '#',
            "U+0023",
            "urn:language-graph:grapheme:nfc:0023",
            "bafyreiajj25zb3zic6pcu7655fsk4a7mvclwgiof3i44eovx2zxodioo7m",
        ),
        (
            '$',
            "U+0024",
            "urn:language-graph:grapheme:nfc:0024",
            "bafyreidh4oihmqyykdz7dwsvy4yndkg7ihw4hf3jcn3h2z46fqr5pldznu",
        ),
        (
            '%',
            "U+0025",
            "urn:language-graph:grapheme:nfc:0025",
            "bafyreiaovyhckcdgqs2rm34efe7vk6az6gwrijcxhshl4csebixtddwldu",
        ),
        (
            '&',
            "U+0026",
            "urn:language-graph:grapheme:nfc:0026",
            "bafyreie5cqbgjq2ydtstpanskr345or64evwksv66thzlqgml6eqbqeplq",
        ),
        (
            '*',
            "U+002A",
            "urn:language-graph:grapheme:nfc:002a",
            "bafyreiaooo3tczirtiqwhn47tnqkxbqn676zjdi4awjcdcelsoyva3x4ly",
        ),
        (
            '+',
            "U+002B",
            "urn:language-graph:grapheme:nfc:002b",
            "bafyreicdelciki475m5awn356ur6halwtfcln445h4qvjti775vej52yga",
        ),
        (
            '/',
            "U+002F",
            "urn:language-graph:grapheme:nfc:002f",
            "bafyreiftsuulldo5xivjmrecy2reta7vni3yg42ux427ktgetb6djiq5hm",
        ),
        (
            '<',
            "U+003C",
            "urn:language-graph:grapheme:nfc:003c",
            "bafyreiff3ae6gibn2nfl4eu5abnlglnwbvinysn6ntdj5mcygu77v5tlwi",
        ),
        (
            '=',
            "U+003D",
            "urn:language-graph:grapheme:nfc:003d",
            "bafyreig4vzvzckmikxapd37f2fce7leqedjht5cp6egn5z67po3ded7t5m",
        ),
        (
            '>',
            "U+003E",
            "urn:language-graph:grapheme:nfc:003e",
            "bafyreihdwvqwdmlmvijsw556bfnlr2ltzrvwnxbpfbiyd7c3d7y5jcmkzq",
        ),
        (
            '@',
            "U+0040",
            "urn:language-graph:grapheme:nfc:0040",
            "bafyreidxoor5oz3sj3jltaf7sfdttpkooacy7cmnh2ipaiqvcyrt54skd4",
        ),
        (
            '[',
            "U+005B",
            "urn:language-graph:grapheme:nfc:005b",
            "bafyreihccfu5mvgpkjzxumxhppso7mavh7cwkbbbg7mmuvjxbybt456u3q",
        ),
        (
            '\\',
            "U+005C",
            "urn:language-graph:grapheme:nfc:005c",
            "bafyreigsg5oxzm26o4v35tpamxbbg2xrgc65a32x44zxpawxyqssata6cm",
        ),
        (
            ']',
            "U+005D",
            "urn:language-graph:grapheme:nfc:005d",
            "bafyreibca6kx7wib4t3qkz5xxhxn6rekkjgdqs3a63myycqopiwovn5cmq",
        ),
        (
            '^',
            "U+005E",
            "urn:language-graph:grapheme:nfc:005e",
            "bafyreifeuinqwbpfyclxihqr6xsmyzaudlj3b27wikwxvjdpnxax5uxyiu",
        ),
        (
            '_',
            "U+005F",
            "urn:language-graph:grapheme:nfc:005f",
            "bafyreidl25gy5bawxpcgzdm2i2omdt4m5kgbhoptoie35fse2mkep74ebi",
        ),
        (
            '`',
            "U+0060",
            "urn:language-graph:grapheme:nfc:0060",
            "bafyreibvmqwgeanqyomvln2zs2ixjw2jystszzpccukyzcz433nsdmyvke",
        ),
        (
            '{',
            "U+007B",
            "urn:language-graph:grapheme:nfc:007b",
            "bafyreib3hl3wjd3la7r2eu7u72beoocelb64l73uslzgk6dlszzowyotvy",
        ),
        (
            '|',
            "U+007C",
            "urn:language-graph:grapheme:nfc:007c",
            "bafyreicbi3yy7ilvg4wr3j6m33udppwuddimecocphqy26z44pnwop2cle",
        ),
        (
            '}',
            "U+007D",
            "urn:language-graph:grapheme:nfc:007d",
            "bafyreidcqbeft3st5w4w4cngn5bnrceok6fumpdmovudeqzxdstnumefbm",
        ),
        (
            '~',
            "U+007E",
            "urn:language-graph:grapheme:nfc:007e",
            "bafyreicx2xpvp2d4gzijs7q3irpd56ymjooalnyzwvkpypaeblzkoken4e",
        ),
    ];

    let supp_snap_members = repo.get_snapshot_members(&active_supp_snap).unwrap();
    assert_eq!(supp_snap_members.len(), 21);

    for (i, &(ch, scalar, entity_id, revision_cid)) in golden_table.iter().enumerate() {
        // Assert collection membership details
        let member = &supp_snap_members[i];
        assert_eq!(member.position, (i + 1) as i32);
        assert_eq!(member.entity_id, entity_id);
        assert_eq!(member.revision_cid, revision_cid);

        // Fetch grapheme revision block details
        let rev = repo.get_grapheme_revision(revision_cid).unwrap();
        assert_eq!(rev.surface_form, ch.to_string());
        assert_eq!(rev.entity_id, entity_id);
        assert_eq!(rev.unicode_scalars, vec![scalar.to_string()]);
        assert_eq!(rev.script, "Common");
        assert_eq!(rev.case, "none");
    }
}
